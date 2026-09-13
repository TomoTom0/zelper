// L1: remap v2 planner matrix（test-plan §2.7 (a) R1〜R12・R20。DD-10.5/10.6）。
// plannerは純粋関数（fake backendなしで検証する。DD-10.6）。
// fail-first: 実装（TASK-39）前にfailする（test-plan §5）。
use zelper::app::remap::plan_v2;
use zelper::domain::*;
use zelper::error::ErrorClass;

const ANCHOR: TabId = TabId(0);

fn pane(id: u32, title: &str, cmd: Option<&str>) -> PaneState {
    pane_at(id, title, 0, 0, 0, id * 10, cmd)
}

/// tab_id / tab_position / y / x を指定するpane builder
fn pane_at(
    id: u32,
    title: &str,
    tab_id: u32,
    tab_position: u32,
    y: u32,
    x: u32,
    cmd: Option<&str>,
) -> PaneState {
    PaneState {
        id: PaneKindId::Terminal(id),
        title: title.to_string(),
        is_selectable: true,
        is_floating: false,
        is_focused: false,
        exited: false,
        is_held: false,
        geometry: Geometry {
            x,
            y,
            rows: 10,
            cols: 10,
        },
        command: cmd.map(|c| c.to_string()),
        cwd: Some("/w".into()),
        tab_id: TabId(tab_id),
        tab_position,
        tab_name: format!("t{tab_id}"),
        plugin_url: None,
    }
}

fn panes(n: usize) -> Vec<PaneState> {
    (0..n)
        .map(|i| pane(i as u32, &format!("p{i}"), Some(&format!("cmd{i}"))))
        .collect()
}

fn ids(plan: &zelper::app::remap::V2Plan, instance: usize) -> Vec<PaneKindId> {
    plan.instances[instance]
        .assignments
        .iter()
        .map(|a| a.pane)
        .collect()
}

// ---- R1〜R7: k = max(1, ceil(M/N)) のinstance反復 ----

// [covers:layout-planner.r1-one-pane-into-three-slots-k1-empty-two]
#[test]
fn r1_one_pane_into_three_slots() {
    let p = plan_v2(&panes(1), 3, ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 1);
    assert_eq!(p.m, 1);
    assert_eq!(p.n_slots, 3);
    assert_eq!(p.instances.len(), 1);
    assert_eq!(p.instances[0].assignments.len(), 1);
    assert_eq!(p.instances[0].assignments[0].slot, 0);
    assert_eq!(p.instances[0].assignments[0].pane, PaneKindId::Terminal(0));
    // 空2 slotは既定shell（R40で実行側・R16でKDL側を検証）
    assert_eq!(p.instances[0].empty_slots, 2);
}

// [covers:layout-planner.r2-three-into-three-no-move-no-plugin]
#[test]
fn r2_three_into_three_no_move_no_plugin() {
    // 3 pane同一tab / 3-slot: k=1・移動なし。group 0にanchor外tabのpaneが存在
    // しないためcompanion pluginは不使用（「移動なし」の実行側検証はR29）
    let p = plan_v2(&panes(3), 3, ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 1);
    assert_eq!(p.instances.len(), 1);
    assert_eq!(p.instances[0].empty_slots, 0);
    assert_eq!(
        ids(&p, 0),
        vec![
            PaneKindId::Terminal(0),
            PaneKindId::Terminal(1),
            PaneKindId::Terminal(2)
        ]
    );
}

// [covers:layout-planner.r3-four-panes-into-three-slots-k2]
#[test]
fn r3_four_into_three_k2_no_error() {
    // M>Nでもerrorにならない（v2はM>N既定error廃止。旧仕様の置換）
    let p = plan_v2(&panes(4), 3, ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 2);
    assert_eq!(p.instances.len(), 2);
    // group 0 = visual順先頭3・group 1 = 残1
    assert_eq!(ids(&p, 0).len(), 3);
    assert_eq!(
        ids(&p, 1),
        vec![PaneKindId::Terminal(3)],
        "group 1 is the 4th pane"
    );
    assert_eq!(p.instances[1].assignments[0].slot, 0);
    assert_eq!(p.instances[0].empty_slots, 0);
    assert_eq!(p.instances[1].empty_slots, 2);
}

// [covers:layout-planner.r4-six-into-three-two-full-instances]
#[test]
fn r4_six_into_three_two_full_instances() {
    let p = plan_v2(&panes(6), 3, ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 2);
    assert_eq!(p.instances[0].assignments.len(), 3);
    assert_eq!(p.instances[1].assignments.len(), 3);
    assert_eq!(p.instances[1].empty_slots, 0);
}

// [covers:layout-planner.r5-seven-into-three-third-instance-partial]
#[test]
fn r5_seven_into_three_third_instance_partial() {
    let p = plan_v2(&panes(7), 3, ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 3);
    assert_eq!(p.instances.len(), 3);
    assert_eq!(p.instances[2].assignments.len(), 1);
    // 第3 instanceは1 pane + 2空slot（既定shell）
    assert_eq!(p.instances[2].empty_slots, 2);
}

// [covers:layout-planner.r6-cross-tab-groups-follow-visual-order]
#[test]
fn r6_cross_tab_groups_follow_visual_order() {
    // 5 paneが3 tabに分散 / 3-slot: k=2。group 0 = visual順先頭3 pane（tab跨ぎ）
    // をanchorへ集中・group 1 = 残2。session全体を1度のplanで扱う例
    // （worked example #6）
    let source = vec![
        pane_at(1, "a1", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "a2", 0, 0, 0, 50, Some("cmd2")),
        pane_at(3, "b1", 1, 1, 0, 0, Some("cmd3")),
        pane_at(4, "b2", 1, 1, 0, 50, Some("cmd4")),
        pane_at(5, "c1", 2, 2, 0, 0, Some("cmd5")),
    ];
    let p = plan_v2(&source, 3, ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 2);
    assert_eq!(
        ids(&p, 0),
        vec![
            PaneKindId::Terminal(1),
            PaneKindId::Terminal(2),
            PaneKindId::Terminal(3)
        ],
        "group 0 = visual順先頭3（tab 0の2 pane + tab 1の1 pane。tab跨ぎ）"
    );
    assert_eq!(
        ids(&p, 1),
        vec![PaneKindId::Terminal(4), PaneKindId::Terminal(5)]
    );
    // 空になったsource tabの自動closeは実行側の挙動（R37/R39で検証）
}

// [covers:layout-planner.r7-empty-session-all-slots-empty]
#[test]
fn r7_empty_session_all_slots_empty() {
    // M=0（空session）: k=1・全slot空（既定shell）
    let p = plan_v2(&[], 3, ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 1);
    assert_eq!(p.m, 0);
    assert_eq!(p.instances.len(), 1);
    assert!(p.instances[0].assignments.is_empty());
    assert_eq!(p.instances[0].empty_slots, 3);
}

// ---- R9〜R12 ----

// [covers:layout-planner.r9-fewer-panes-than-slots-keeps-k1]
#[test]
fn r9_fewer_panes_than_slots_keeps_k1() {
    // M<N・layoutにcommand付きslot: plannerはk=1。空slotのbare pane正規化は
    // generator側（R16。test layout_generator.rs r9）で検証する
    let p = plan_v2(&panes(2), 3, ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 1);
    assert_eq!(p.instances[0].empty_slots, 1);
}

// [covers:layout-planner.r10-assignment-follows-visual-order-regardless-of-input-order]
#[test]
fn r10_assignment_follows_visual_order_regardless_of_input_order() {
    // 割当順は (tab_position, pane_y, pane_x) 昇順で固定。入力順に依存しない
    let mut source = vec![
        pane_at(11, "tab1-x100", 1, 1, 0, 100, Some("d")),
        pane_at(2, "tab0-y5", 0, 0, 5, 0, Some("b")),
        pane_at(12, "tab1-x0", 1, 1, 0, 0, Some("c")),
        pane_at(1, "tab0-y1", 0, 0, 1, 0, Some("a")),
    ];
    // 意図的にvisual orderでない並びで渡す
    source.reverse();
    let p = plan_v2(&source, 4, ANCHOR, "agents").unwrap();
    let got: Vec<PaneKindId> = p.instances[0].assignments.iter().map(|a| a.pane).collect();
    let expect = vec![
        PaneKindId::Terminal(1),  // (tab0, y=1, x=0)
        PaneKindId::Terminal(2),  // (tab0, y=5, x=0)
        PaneKindId::Terminal(12), // (tab1, y=0, x=0)
        PaneKindId::Terminal(11), // (tab1, y=0, x=100)
    ];
    assert_eq!(got, expect);
    // slot = visual order順に0..N-1
    let slots: Vec<usize> = p.instances[0].assignments.iter().map(|a| a.slot).collect();
    assert_eq!(slots, vec![0, 1, 2, 3]);
}

// [covers:layout-planner.r11-zero-slot-layout-invalid-and-missing-layout-not-found]
#[test]
fn r11_zero_slot_layout_invalid_and_missing_layout_not_found() {
    // slot数 N=0はLayoutInvalid（exit 7）
    let err = plan_v2(&panes(1), 0, ANCHOR, "agents").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::LayoutInvalid);

    // layout解決不能はLayoutNotFound（exit 7）。現行layout::load_kdlの挙動
    let missing = LayoutRef::Name("no-such-layout-xyz".into());
    let err = zelper::layout::load_kdl(&missing).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::LayoutNotFound);
}

// [covers:layout-planner.r12-slot-count-uses-first-tab-subtree-excluding-plugin-leaves]
#[test]
fn r12_slot_count_uses_first_tab_subtree_excluding_plugin_leaves() {
    // N = layout最初のtab subtreeの末端terminal slot数。
    // plugin leaf除外はbare node・pane wrapper・config子node持ちの全形式（MR-32回帰）
    let three = "layout {\n    tab {\n        pane size=\"40%\"\n        pane size=\"30%\"\n        pane size=\"30%\"\n    }\n}\n";
    let doc = zelper::layout::parse(three).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    assert_eq!(zelper::layout::count_terminal_slots(&base), 3);

    // pane wrapper内のplugin leaf（tab bar等）はslotを消費しない
    let with_bar = "layout {\n    tab {\n        pane size=1 borderless=true {\n            plugin location=\"zellij:tab-bar\"\n        }\n        pane\n        pane\n    }\n}\n";
    let doc = zelper::layout::parse(with_bar).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    assert_eq!(zelper::layout::count_terminal_slots(&base), 2);

    // bare plugin node（config子node持ちを含む）もslotを形成しない
    let bare = "layout {\n    tab {\n        plugin location=\"file:/plugins/zjstatus.wasm\" {\n            format_left \"{session_name}\"\n        }\n        pane\n    }\n}\n";
    let doc = zelper::layout::parse(bare).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    assert_eq!(zelper::layout::count_terminal_slots(&base), 1);

    // container・floating_panes・templateはSKIP_NODES規則で対象外
    let complex = "layout {\n    pane_template name=\"t\" {\n        pane\n    }\n    tab {\n        pane split_direction=\"vertical\" {\n            pane\n            pane command=\"htop\"\n        }\n    }\n    floating_panes {\n        pane\n    }\n}\n";
    let doc = zelper::layout::parse(complex).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    assert_eq!(zelper::layout::count_terminal_slots(&base), 2);
}

// [covers:layout-planner.r12-multi-tab-layout-counts-first-tab-shape-only]
#[test]
fn r12_multi_tab_layout_counts_first_tab_shape_only() {
    // multi-tab layoutは最初のtab形状を反復単位とする（2-tab layout各3 slot -> N=3）
    const TWO_TAB: &str = "layout {\n    tab {\n        pane\n        pane\n        pane\n    }\n    tab {\n        pane\n        pane\n        pane\n    }\n}\n";
    let doc = zelper::layout::parse(TWO_TAB).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    assert_eq!(zelper::layout::count_terminal_slots(&base), 3);

    // 4 paneはN=3のk=2（errorにならない。v2はM>N既定error廃止）
    let p = plan_v2(&panes(4), 3, ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 2);
}

// [covers:layout-planner.r12-zellij-bare-bools-and-property-nodes-parse]
#[test]
fn r12_zellij_bare_bools_and_property_nodes_parse() {
    // zellij layoutはbare bool（borderless=true）とproperty node（cwd/start_suspended）
    // を常用する。parse時にquote正規化されること（MR-16回帰）
    let bar_layout = "layout {\n    pane size=1 borderless=true { plugin location=\"zellij:tab-bar\" }\n    pane focus=true\n}\n";
    let doc = zelper::layout::parse(bar_layout).unwrap();
    assert_eq!(zelper::layout::count_terminal_slots(&doc), 1);

    let with_cwd = "layout {\n    cwd \"/work\"\n    pane\n}\n";
    let doc = zelper::layout::parse(with_cwd).unwrap();
    assert_eq!(zelper::layout::count_terminal_slots(&doc), 1);

    let with_suspend = "layout {\n    tab {\n        pane command=\"x\" {\n            start_suspended true\n        }\n    }\n}\n";
    let doc = zelper::layout::parse(with_suspend).unwrap();
    assert_eq!(zelper::layout::count_terminal_slots(&doc), 1);
}

// ---- R20: preflight warning ----

// [covers:layout-planner.r20-quoted-chars-in-pane-command-warn-but-do-not-fail]
#[test]
fn r20_quoted_chars_in_pane_command_warn_but_do_not_fail() {
    // pane_commandに `"` / `\` を含むpaneがあればpreflight warning（実行は可。
    // DD-10.13。空白分割でquoteが復元できずrun一致が外れるため）
    let source = vec![
        pane(1, "ok", Some("codex exec")),
        pane(2, "dq", Some("echo \"x\"")),
        pane(3, "bs", Some("echo C:\\path")),
    ];
    let p = plan_v2(&source, 3, ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 1); // 実行は可（errorにならない）
    assert!(
        p.warnings.iter().any(|w| w.contains("terminal_2")),
        "raw: {:?}",
        p.warnings
    );
    assert!(
        p.warnings.iter().any(|w| w.contains("terminal_3")),
        "raw: {:?}",
        p.warnings
    );
    assert!(
        !p.warnings.iter().any(|w| w.contains("terminal_1")),
        "raw: {:?}",
        p.warnings
    );

    // warning対象がいなければ空
    let p = plan_v2(&panes(3), 3, ANCHOR, "agents").unwrap();
    assert!(p.warnings.is_empty());
}

// [covers:layout-planner.quoting-warning-covers-single-quotes-newlines-control-chars]
#[test]
fn c6_quoting_warning_covers_single_quotes_newlines_control_chars() {
    // warning対象は `"` / `\` に加え `'`・改行・制御文字（split_whitespaceで
    // argvが壊れうる文字。DD-10.13。C6回帰）
    let source = vec![
        pane(1, "sq", Some("bash -lc 'echo a b'")),
        pane(2, "nl", Some("echo a\nb")),
        pane(3, "ctl", Some("echo \u{1b}[0m")),
        pane(4, "ok", Some("codex exec --plan")),
    ];
    let p = plan_v2(&source, 4, ANCHOR, "agents").unwrap();
    for id in [1u32, 2, 3] {
        assert!(
            p.warnings
                .iter()
                .any(|w| w.contains(&format!("terminal_{id}"))),
            "terminal_{id} がwarning対象: raw: {:?}",
            p.warnings
        );
    }
    assert!(
        !p.warnings.iter().any(|w| w.contains("terminal_4")),
        "通常の引数区切り空白は対象外: raw: {:?}",
        p.warnings
    );
}
