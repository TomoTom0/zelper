// L1: backend parser fixtures（test-plan §2.3。実出力に基づく）
use zelper::domain::{PaneKindId, TabId};
use zelper::zellij::parser::{
    parse_created_pane, parse_created_tab, parse_panes, parse_sessions, parse_tabs, parse_version,
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
    assert_eq!(panes[0].geometry.cols, 100);
    assert!(panes[0].is_remap_source());
    assert_eq!(panes[1].id, PaneKindId::Plugin(2));
    assert!(!panes[1].is_remap_source());
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
