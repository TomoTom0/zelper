// L1: backend parser fixtures（test-plan §2.3。実出力に基づく）
use zelper::domain::{PaneKindId, TabId};
use zelper::zellij::parser::{
    parse_created_pane, parse_created_tab, parse_panes, parse_panes_opt, parse_sessions,
    parse_tabs, parse_tabs_opt, parse_version,
};

// [covers:backend-parser.version-strings]
#[test]
fn version_strings() {
    assert_eq!(parse_version("zellij 0.44.3\n"), Some((0, 44, 3)));
    assert_eq!(parse_version("zellij 0.44.1"), Some((0, 44, 1)));
    assert_eq!(parse_version("zellij 0.43.2"), Some((0, 43, 2)));
    assert_eq!(parse_version("zellij 1.0.0"), Some((1, 0, 0)));
    assert_eq!(parse_version("garbage"), None);
}

// [covers:backend-parser.sessions-text-basic-form]
#[test]
fn sessions_text_parse() {
    let out = "zelper-p1-basic [Created 10s ago]\nzelper-p1-ops [Created 2m ago]\n";
    let s = parse_sessions(out);
    assert_eq!(s.len(), 2);
    assert_eq!(s[0].name, "zelper-p1-basic");
    assert_eq!(s[0].created.as_deref(), Some("10s ago"));
    assert!(!s[0].current);
    assert!(!s[0].exited);
    assert_eq!(s[1].name, "zelper-p1-ops");
}

// [covers:backend-parser.sessions-parse-flags-current-and-exited]
#[test]
fn sessions_parse_flags_current_and_exited() {
    // 実出力形式: suffixなし / (current) / (EXITED - attach to resurrect)。
    // parseは全行を返し、current/exited flagを付ける（絞り込みは呼び出し側）
    let out = "dead-one [Created 2h ago] (EXITED - attach to resurrect)\n\
               live-one [Created 1m ago] (current)\n\
               live-two [Created 3d ago]\n";
    let s = parse_sessions(out);
    assert_eq!(s.len(), 3);
    assert_eq!(s[0].name, "dead-one");
    assert!(s[0].exited);
    assert!(!s[0].current);
    assert_eq!(s[1].name, "live-one");
    assert!(!s[1].exited);
    assert!(s[1].current);
    assert_eq!(s[2].name, "live-two");
    assert!(!s[2].exited);
    assert!(!s[2].current);
    // 実行中のみへの絞り込み（resolve_session・list sessionsが行うfilter）
    let live: Vec<_> = s.into_iter().filter(|x| !x.exited).collect();
    assert_eq!(live.len(), 2);
}

/// TASK-99: 名前の2語目以降（空白含み名）にmarker部分文字列（EXITED・(current)・
/// `[Created`）を含む行でも、生成metadata field（exited/current/created）は名前由来
/// の部分文字列で汚染されない。根拠はzellij 0.44.3の行形式の固定性——
/// `[Created ...]`節は常時name直後・suffixは排他一択で行末（設計§2.1）。
/// TASK-98: 空白含み名5行のname期待値は完全名（marker直前の区切りASCII空白1文字
/// のみ除去）へ更新。field非汚染assert群は不変・marker不在2行は先頭wordのまま
// [covers:backend-parser.sessions-name-marker-substrings-do-not-pollute-fields]
#[test]
fn sessions_marker_substrings_do_not_pollute_fields() {
    // trailing space付きの行（s0行・d行の行末）も意図的に含む
    let out = "x EXITED [Created 2m ago]\n\
               cur (current) [Created 1m ago]\n\
               x [foo [Created 1m ago]\n\
               pre [Created 5m ago] [Created 1m ago]\n\
               y (EXITED - attach to resurrect) [Created 1m ago]\n\
               s0 [Created 1s ago] \n\
               d [Created 2m ago] (EXITED - attach to resurrect) \n\
               weird row without marker\n\
               weird [foo]\n\
               live [Created 1m ago] (current)\n\
               d2 [Created 2m ago] (EXITED - attach to resurrect)\n";
    let s = parse_sessions(out);
    assert_eq!(s.len(), 11);
    // 名2語目にEXITEDを含むrunning行 → exited=false（createdは真の生成節）。
    // name=完全名（TASK-98）
    assert_eq!(s[0].name, "x EXITED");
    assert!(!s[0].exited, "名中EXITEDのrunning行がexitedに汚染");
    assert!(!s[0].current);
    assert_eq!(s[0].created.as_deref(), Some("2m ago"));
    // 名2語目に(current)を含む行 → current=false。name=完全名（TASK-98）
    assert_eq!(s[1].name, "cur (current)");
    assert!(!s[1].current, "名中(current)行がcurrentに汚染");
    assert!(!s[1].exited);
    // 名2語目に`[`を含む行 → createdは真の生成節（名前側の節でない）。
    // name=完全名（TASK-98）
    assert_eq!(s[2].name, "x [foo");
    assert_eq!(
        s[2].created.as_deref(),
        Some("1m ago"),
        "名中`[`行のcreatedが名前側の節に汚染: {:?}",
        s[2].created
    );
    // 名前が完全な[Created ...]を含む行 → 生成節=最後尾出現が選ばれる。
    // name=完全名（名前側のmarker字面を含む。TASK-98）
    assert_eq!(s[3].name, "pre [Created 5m ago]");
    assert_eq!(s[3].created.as_deref(), Some("1m ago"));
    // 名前末尾が完全なEXITED suffix字面と一致するrunning行 → exited=false。
    // name=完全名（TASK-98）
    assert_eq!(s[4].name, "y (EXITED - attach to resurrect)");
    assert!(
        !s[4].exited,
        "名末尾EXITED字面のrunning行がexitedに汚染: {:?}",
        s[4]
    );
    assert!(!s[4].current);
    // suffixなし行のtrailing space → flags false・createdは取れる
    assert_eq!(s[5].name, "s0");
    assert!(!s[5].exited);
    assert!(!s[5].current);
    assert_eq!(s[5].created.as_deref(), Some("1s ago"));
    // 本物suffixの後の空白 → exited=true（trim_end対応）
    assert_eq!(s[6].name, "d");
    assert!(
        s[6].exited,
        "本物suffix後空白行のexitedが落ちる: {:?}",
        s[6]
    );
    assert!(!s[6].current);
    assert_eq!(s[6].created.as_deref(), Some("2m ago"));
    // `[Created `不在行（`[`不含） → created=None・flags false
    assert_eq!(s[7].name, "weird");
    assert_eq!(s[7].created, None);
    assert!(!s[7].exited);
    assert!(!s[7].current);
    // `[`を含むが`[Created `marker不在の行 → created=None（設計§4: find('[')
    // heuristicへは戻さない。fallback再導入だとcreated=Some("foo")になるため赤）
    assert_eq!(s[8].name, "weird");
    assert_eq!(
        s[8].created, None,
        "find('[') fallbackが再導入されname側の節を掴んだ: {:?}",
        s[8].created
    );
    assert!(!s[8].exited);
    assert!(!s[8].current);
    // 本物suffix行の回帰（anchor化後もtrueを取る）
    assert_eq!(s[9].name, "live");
    assert!(s[9].current);
    assert!(!s[9].exited);
    assert_eq!(s[10].name, "d2");
    assert!(s[10].exited);
    assert!(!s[10].current);
}

/// TASK-98: parse_sessionsはnameを生成節より前の全体（完全名）へ復元する。
/// marker（`[Created `）直前の区切りASCII空白1文字のみを除き、名前自身の末尾空白
/// （space/tab/NBSP）と内部の複数空白は保存する。marker不在行は先頭wordへ
/// fallback（既存挙動との互換）。現行（name=split_whitespace先頭word）では
/// 復元系caseが赤（fail-first）
// [covers:backend-parser.sessions-whitespace-name-restored-to-full-name]
#[test]
fn sessions_whitespace_names_restored_to_full_name() {
    let out = "a b [Created 2s ago]\n\
               x EXITED [Created 1m ago]\n\
               y (current) [Created 1m ago] (current)\n\
               pre [Created 5m ago] [Created 1m ago]\n\
               d  x [Created 2m ago] (EXITED - attach to resurrect)\n\
               a  [Created 1m ago]\n\
               b\t [Created 1m ago]\n\
               c\u{00A0} [Created 1m ago]\n\
               x[Created 1m ago]\n\
               weird row without marker\n\
               [Created 1m ago]\n\
               live-a [Created 1m ago]\n";
    let s = parse_sessions(out);
    // 空remainder（[Created ...]で始まる行）はskipされるため12行中11件
    assert_eq!(s.len(), 11, "空remainder行がskipされていない: {s:?}");
    // 基本復元: name=完全名・created/flagsはTASK-99 anchorのまま
    assert_eq!(s[0].name, "a b");
    assert_eq!(s[0].created.as_deref(), Some("2s ago"));
    assert!(!s[0].current);
    assert!(!s[0].exited);
    // TASK-99汚染fixtureのname側: 名2語目のmarker部分文字列も名前に含まれる
    assert_eq!(s[1].name, "x EXITED");
    assert!(!s[1].exited);
    // suffixと名前末尾の同字面共存: 名中(current)と本物(current) suffixの分離
    assert_eq!(s[2].name, "y (current)");
    assert!(s[2].current);
    assert!(!s[2].exited);
    // 名前がmarker字面を含む: rfind=最後尾出現が真の生成節（名前側を含む完全名）
    assert_eq!(s[3].name, "pre [Created 5m ago]");
    assert_eq!(s[3].created.as_deref(), Some("1m ago"));
    // 内部複数空白保持 + 本物EXITED suffix
    assert_eq!(s[4].name, "d  x");
    assert!(s[4].exited);
    assert_eq!(s[4].created.as_deref(), Some("2m ago"));
    // 名前自身の末尾空白（区切りASCII空白1文字のみ除去。trim_end系抽出は全種類の
    // 空白を削除するため完全復元の反例になる）: space / tab / NBSP
    assert_eq!(s[5].name, "a ");
    assert_eq!(s[6].name, "b\t");
    assert_eq!(s[7].name, "c\u{00A0}");
    assert_eq!(s[5].created.as_deref(), Some("1m ago"));
    assert_eq!(s[6].created.as_deref(), Some("1m ago"));
    assert_eq!(s[7].created.as_deref(), Some("1m ago"));
    // 区切り不在の非正規行: marker直前の1文字を剥がさない
    assert_eq!(s[8].name, "x");
    assert_eq!(s[8].created.as_deref(), Some("1m ago"));
    assert!(!s[8].exited);
    assert!(!s[8].current);
    // marker不在fallback: 既存挙動（先頭word）との互換・created=None
    assert_eq!(s[9].name, "weird");
    assert_eq!(s[9].created, None);
    assert!(!s[9].exited);
    assert!(!s[9].current);
    // 標準行回帰: 空白なし名は現行とbit一致
    assert_eq!(s[10].name, "live-a");
    assert_eq!(s[10].created.as_deref(), Some("1m ago"));
    assert!(!s[10].exited);
    assert!(!s[10].current);
}

// [covers:backend-parser.created-ids-from-stdout]
#[test]
fn created_ids_parse() {
    assert_eq!(
        parse_created_pane("terminal_1\n").unwrap(),
        PaneKindId::Terminal(1)
    );
    assert_eq!(parse_created_tab("1\n").unwrap(), TabId(1));
    assert!(parse_created_pane("").is_err());
    assert!(parse_created_tab("abc").is_err());
}

// [covers:backend-parser.panes-json-full-fields-from-real-output]
#[test]
fn panes_json_parse_from_real_output_shape() {
    // Phase 1実出力（out/02-panes-multi.json）の一部を簡略化したfield構成
    let json = r#"[
      {
        "id": 1, "is_plugin": false, "is_focused": true, "is_fullscreen": false,
        "is_floating": false, "is_suppressed": false, "title": "HB1",
        "exited": false, "exit_status": null, "is_held": false,
        "pane_x": 0, "pane_content_x": 0, "pane_y": 1, "pane_content_y": 1,
        "pane_rows": 58, "pane_content_rows": 56, "pane_columns": 100, "pane_content_columns": 98,
        "cursor_coordinates_in_pane": null, "terminal_command": null,
        "plugin_url": null, "is_selectable": true, "index_in_pane_group": null,
        "tab_id": 0, "tab_position": 0, "tab_name": "Tab #1",
        "pane_command": "bash /work/hb.sh p1", "pane_cwd": "/work"
      },
      {
        "id": 2, "is_plugin": true, "is_focused": false, "is_fullscreen": false,
        "is_floating": false, "is_suppressed": false, "title": "Zellij (update available)",
        "exited": false, "exit_status": null, "is_held": false,
        "pane_x": 0, "pane_content_x": 0, "pane_y": 0, "pane_content_y": 0,
        "pane_rows": 1, "pane_content_rows": 1, "pane_columns": 200, "pane_content_columns": 200,
        "cursor_coordinates_in_pane": null, "terminal_command": null,
        "plugin_url": "zellij:tab-bar", "is_selectable": false, "index_in_pane_group": null,
        "tab_id": 0, "tab_position": 0, "tab_name": "Tab #1",
        "pane_command": null, "pane_cwd": null
      }
    ]"#;
    let panes = parse_panes(json).unwrap();
    assert_eq!(panes.len(), 2);
    assert_eq!(panes[0].id, PaneKindId::Terminal(1));
    assert_eq!(panes[0].command.as_deref(), Some("bash /work/hb.sh p1"));
    assert_eq!(panes[0].terminal_command, None);
    assert_eq!(panes[0].geometry.cols, 100);
    assert!(panes[0].is_remap_source());
    assert_eq!(panes[1].id, PaneKindId::Plugin(2));
    assert!(!panes[1].is_remap_source());
}

// [covers:backend-parser.terminal-command-field-parsed]
#[test]
fn terminal_command_field_is_parsed_independently_from_pane_command() {
    let json = r#"[
      {"id":1,"is_plugin":false,"is_focused":true,"is_floating":false,"title":"shell","exited":false,"is_held":false,"pane_x":0,"pane_y":0,"pane_rows":10,"pane_columns":10,"is_selectable":true,"plugin_url":null,"tab_id":0,"tab_position":0,"tab_name":"t","terminal_command":null,"pane_command":"vim src/main.rs","pane_cwd":"/work"},
      {"id":2,"is_plugin":false,"is_focused":false,"is_floating":false,"title":"cmd","exited":false,"is_held":false,"pane_x":10,"pane_y":0,"pane_rows":10,"pane_columns":10,"is_selectable":true,"plugin_url":null,"tab_id":0,"tab_position":0,"tab_name":"t","terminal_command":"claude --model x","pane_command":null,"pane_cwd":"/work"}
    ]"#;
    let panes = parse_panes(json).unwrap();
    assert_eq!(panes[0].terminal_command, None);
    assert_eq!(panes[0].command.as_deref(), Some("vim src/main.rs"));
    assert_eq!(
        panes[1].terminal_command.as_deref(),
        Some("claude --model x")
    );
    assert_eq!(panes[1].command, None);

    let legacy = json.replace(",\"terminal_command\":null", "");
    assert_eq!(parse_panes(&legacy).unwrap()[0].terminal_command, None);
}

// [covers:backend-parser.empty-output-parses-to-none-not-error]
#[test]
fn empty_output_is_optional_but_garbage_is_error() {
    assert!(parse_panes_opt("").unwrap().is_none());
    assert!(parse_tabs_opt("\n").unwrap().is_none());
    assert!(parse_panes_opt("garbage").is_err());
    assert!(parse_tabs_opt("garbage").is_err());
    assert!(parse_panes_opt("[]").unwrap().is_some());
    assert!(parse_tabs_opt("[]").unwrap().is_some());
}

// [covers:backend-parser.tabs-json-from-real-output]
#[test]
fn tabs_json_parse_from_real_output_shape() {
    let json = r#"[
      {
        "position": 0, "name": "Tab #1", "active": true,
        "panes_to_hide": 2, "is_fullscreen_active": false, "is_sync_panes_active": false,
        "are_floating_panes_visible": true, "other_focused_clients": null,
        "active_swap_layout_name": null, "is_swap_layout_dirty": false,
        "viewport_rows": 60, "viewport_columns": 200,
        "display_area_rows": 60, "display_area_columns": 200,
        "selectable_tiled_panes_count": 3, "selectable_floating_panes_count": 1,
        "tab_id": 0, "has_bell_notification": null, "is_flashing_bell": false
      }
    ]"#;
    let tabs = parse_tabs(json).unwrap();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].id, TabId(0));
    assert_eq!(tabs[0].selectable_tiled_panes_count, 3);
    assert!(!tabs[0].is_empty());
}
