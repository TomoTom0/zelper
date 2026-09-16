// L1: remap v2 planner matrix（test-plan §2.7 (a) R1〜R12・R20。DD-10.5/10.6 v2.2）。
// plannerは純粋関数（fake backendなしで検証する。DD-10.6）。
// TASK-75（v2.2）: plan_v2は正規形TabTemplate列（layout::normalize_tab_templates出力）
// を引数に取り、blocks × groups（group = (block b, tab t)）構造の計画を返す。
// r1〜r10の期待値はT=1後方互換（式レベル一致・DD-10.6 v2.2）により不変（回帰固定）。
use zelper::app::remap::plan_v2;
use zelper::app::remap::{V2Plan, V2TabPlan};
use zelper::domain::*;
use zelper::error::ErrorClass;
use zelper::layout::{TabTemplate, normalize_tab_templates};

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
        terminal_command: None,
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

/// bare n-slot単tab layout文字列
fn bare_layout(n: usize) -> String {
    let slots = vec!["        pane"; n].join("\n");
    format!("layout {{\n    tab {{\n{slots}\n    }}\n}}\n")
}

/// layout文字列から正規形TabTemplate列（warning列は無視）
fn templates_of(text: &str) -> Vec<TabTemplate> {
    let (t, _) = normalized(text);
    t
}

/// layout文字列から正規形TabTemplate列とwarning列
fn normalized(text: &str) -> (Vec<TabTemplate>, Vec<String>) {
    normalize_tab_templates(&zelper::layout::parse(text).unwrap()).unwrap()
}

/// T=1・N=n slotの名無し正規形TabTemplate列
fn templates_n(n: usize) -> Vec<TabTemplate> {
    templates_of(&bare_layout(n))
}

fn group(plan: &V2Plan, b: usize, t: usize) -> &V2TabPlan {
    &plan.blocks[b].groups[t]
}

fn ids(plan: &V2Plan, b: usize, t: usize) -> Vec<PaneKindId> {
    plan.blocks[b].groups[t]
        .assignments
        .iter()
        .map(|a| a.pane)
        .collect()
}

// ---- R1〜R7: k = max(1, ceil(M/S)) のblock反復 ----

// [covers:layout-planner.r1-one-pane-into-three-slots-k1-empty-two]
#[test]
fn r1_one_pane_into_three_slots() {
    let p = plan_v2(&panes(1), &templates_n(3), ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 1);
    assert_eq!(p.m, 1);
    assert_eq!(p.s_slots, 3);
    assert_eq!(p.blocks.len(), 1);
    assert_eq!(p.blocks[0].groups.len(), 1);
    assert_eq!(p.blocks[0].groups[0].assignments.len(), 1);
    assert_eq!(p.blocks[0].groups[0].assignments[0].slot, 0);
    assert_eq!(
        p.blocks[0].groups[0].assignments[0].pane,
        PaneKindId::Terminal(0)
    );
    // 空2 slotは既定shell（R40で実行側・R16でKDL側を検証）
    assert_eq!(p.blocks[0].groups[0].empty_slots, 2);
}

// [covers:layout-planner.r2-three-into-three-no-move-no-plugin]
#[test]
fn r2_three_into_three_no_move_no_plugin() {
    // 3 pane同一tab / 3-slot: k=1・移動なし。group (0,0)にanchor外tabのpaneが存在
    // しないためcompanion pluginは不使用（「移動なし」の実行側検証はR29）
    let p = plan_v2(&panes(3), &templates_n(3), ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 1);
    assert_eq!(p.blocks.len(), 1);
    assert_eq!(p.blocks[0].groups[0].empty_slots, 0);
    assert_eq!(
        ids(&p, 0, 0),
        vec![
            PaneKindId::Terminal(0),
            PaneKindId::Terminal(1),
            PaneKindId::Terminal(2)
        ]
    );
}

// [covers:layout-planner.r3-four-panes-into-three-slots-k2]
#[test]
fn r3_four_into_three_slots_k2_no_error() {
    // M>Nでもerrorにならない（v2はM>N既定error廃止。旧仕様の置換）
    let p = plan_v2(&panes(4), &templates_n(3), ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 2);
    assert_eq!(p.blocks.len(), 2);
    // block 0 = visual順先頭3・block 1 = 残1
    assert_eq!(ids(&p, 0, 0).len(), 3);
    assert_eq!(
        ids(&p, 1, 0),
        vec![PaneKindId::Terminal(3)],
        "block 1 group (1,0) is the 4th pane"
    );
    assert_eq!(p.blocks[1].groups[0].assignments[0].slot, 0);
    assert_eq!(p.blocks[0].groups[0].empty_slots, 0);
    assert_eq!(p.blocks[1].groups[0].empty_slots, 2);
}

// [covers:layout-planner.r4-six-into-three-two-full-instances]
#[test]
fn r4_six_into_three_two_full_blocks() {
    let p = plan_v2(&panes(6), &templates_n(3), ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 2);
    assert_eq!(p.blocks[0].groups[0].assignments.len(), 3);
    assert_eq!(p.blocks[1].groups[0].assignments.len(), 3);
    assert_eq!(p.blocks[1].groups[0].empty_slots, 0);
}

// [covers:layout-planner.r5-seven-into-three-third-instance-partial]
#[test]
fn r5_seven_into_three_third_block_partial() {
    let p = plan_v2(&panes(7), &templates_n(3), ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 3);
    assert_eq!(p.blocks.len(), 3);
    assert_eq!(p.blocks[2].groups[0].assignments.len(), 1);
    // 第3 blockは1 pane + 2空slot（既定shell）
    assert_eq!(p.blocks[2].groups[0].empty_slots, 2);
}

// [covers:layout-planner.r6-cross-tab-groups-follow-visual-order]
#[test]
fn r6_cross_tab_groups_follow_visual_order() {
    // 5 paneが3 tabに分散 / 3-slot: k=2。group (0,0) = visual順先頭3 pane（tab跨ぎ）
    // をanchorへ集中・group (1,0) = 残2。session全体を1度のplanで扱う例
    // （worked example #6）
    let source = vec![
        pane_at(1, "a1", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "a2", 0, 0, 0, 50, Some("cmd2")),
        pane_at(3, "b1", 1, 1, 0, 0, Some("cmd3")),
        pane_at(4, "b2", 1, 1, 0, 50, Some("cmd4")),
        pane_at(5, "c1", 2, 2, 0, 0, Some("cmd5")),
    ];
    let p = plan_v2(&source, &templates_n(3), ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 2);
    assert_eq!(
        ids(&p, 0, 0),
        vec![
            PaneKindId::Terminal(1),
            PaneKindId::Terminal(2),
            PaneKindId::Terminal(3)
        ],
        "group (0,0) = visual順先頭3（tab 0の2 pane + tab 1の1 pane。tab跨ぎ）"
    );
    assert_eq!(
        ids(&p, 1, 0),
        vec![PaneKindId::Terminal(4), PaneKindId::Terminal(5)]
    );
    // 空になったsource tabの自動closeは実行側の挙動（R37/R39で検証）
}

// [covers:layout-planner.r7-empty-session-all-slots-empty]
#[test]
fn r7_empty_session_all_slots_empty() {
    // M=0（空session）: k=1・全slot空（既定shell）
    let p = plan_v2(&[], &templates_n(3), ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 1);
    assert_eq!(p.m, 0);
    assert_eq!(p.blocks.len(), 1);
    assert_eq!(p.blocks[0].groups.len(), 1);
    assert!(p.blocks[0].groups[0].assignments.is_empty());
    assert_eq!(p.blocks[0].groups[0].empty_slots, 3);
}

// ---- R9〜R12 ----

// [covers:layout-planner.r9-fewer-panes-than-slots-keeps-k1]
#[test]
fn r9_fewer_panes_than_slots_keeps_k1() {
    // M<N・layoutにcommand付きslot: plannerはk=1。空slotのbare pane正規化は
    // generator側（R16。test layout_generator.rs r9）で検証する
    let p = plan_v2(&panes(2), &templates_n(3), ANCHOR, "agents").unwrap();
    assert_eq!(p.k, 1);
    assert_eq!(p.blocks[0].groups[0].empty_slots, 1);
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
    let p = plan_v2(&source, &templates_n(4), ANCHOR, "agents").unwrap();
    let got: Vec<PaneKindId> = p.blocks[0].groups[0]
        .assignments
        .iter()
        .map(|a| a.pane)
        .collect();
    let expect = vec![
        PaneKindId::Terminal(1),  // (tab0, y=1, x=0)
        PaneKindId::Terminal(2),  // (tab0, y=5, x=0)
        PaneKindId::Terminal(12), // (tab1, y=0, x=0)
        PaneKindId::Terminal(11), // (tab1, y=0, x=100)
    ];
    assert_eq!(got, expect);
    // slot = visual order順に0..N-1
    let slots: Vec<usize> = p.blocks[0].groups[0]
        .assignments
        .iter()
        .map(|a| a.slot)
        .collect();
    assert_eq!(slots, vec![0, 1, 2, 3]);
}

// [covers:layout-planner.r11-zero-slot-layout-invalid-and-missing-layout-not-found]
#[test]
fn r11_zero_slot_layout_invalid_and_missing_layout_not_found() {
    // 鋳型のslot数 N_0=0（T=1全体でN_0=0）はLayoutInvalid（exit 7）
    let empty_tpl = vec![TabTemplate {
        name: None,
        focus: false,
        subtree: kdl::KdlDocument::new(),
        n_slots: 0,
        pane_focus_slot: None,
    }];
    let err = plan_v2(&panes(1), &empty_tpl, ANCHOR, "agents").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::LayoutInvalid);

    // layout解決不能はLayoutNotFound（exit 7）。現行layout::load_kdlの挙動
    let missing = LayoutRef::Name("no-such-layout-xyz".into());
    let err = zelper::layout::load_kdl(&missing).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::LayoutNotFound);
}

// [covers:layout-planner.r12-slot-count-uses-first-tab-subtree-excluding-plugin-leaves]
#[test]
fn r12_slot_count_uses_first_tab_subtree_excluding_plugin_leaves() {
    // n_slots = 鋳型subtreeの末端terminal slot数（v2.2でnormalize_tab_templates経由の
    // 数え上げへ。数え上げ規則は不変）。plugin leaf除外はbare node・pane wrapper・
    // config子node持ちの全形式（MR-32回帰）
    let three = "layout {\n    tab {\n        pane size=\"40%\"\n        pane size=\"30%\"\n        pane size=\"30%\"\n    }\n}\n";
    let tpl = templates_of(three);
    assert_eq!(tpl.len(), 1);
    assert_eq!(tpl[0].n_slots, 3);

    // pane wrapper内のplugin leaf（tab bar等）はslotを消費しない
    let with_bar = "layout {\n    tab {\n        pane size=1 borderless=true {\n            plugin location=\"zellij:tab-bar\"\n        }\n        pane\n        pane\n    }\n}\n";
    assert_eq!(templates_of(with_bar)[0].n_slots, 2);

    // bare plugin node（config子node持ちを含む）もslotを形成しない
    let bare = "layout {\n    tab {\n        plugin location=\"file:/plugins/zjstatus.wasm\" {\n            format_left \"{session_name}\"\n        }\n        pane\n    }\n}\n";
    assert_eq!(templates_of(bare)[0].n_slots, 1);

    // container・floating_panes・templateはSKIP_NODES規則で対象外
    let complex = "layout {\n    pane_template name=\"t\" {\n        pane\n    }\n    tab {\n        pane split_direction=\"vertical\" {\n            pane\n            pane command=\"htop\"\n        }\n    }\n    floating_panes {\n        pane\n    }\n}\n";
    assert_eq!(templates_of(complex)[0].n_slots, 2);
}

// [covers:layout-planner.r12-multi-tab-layout-counts-first-tab-shape-only]
#[test]
fn r12_multi_tab_layout_counts_layout_total_slots() {
    // TASK-75（v2.2・期待値反転）: multi-tab layoutの反復単位はlayout全体。
    // 2-tab layout（各3 slot）の反復単位はS=6（先頭tabの3ではない）。
    // M=4はS=6に対して反復不要（k=1）で、鋳型0に3 pane・鋳型1に1 paneが割当たる
    const TWO_TAB: &str = "layout {\n    tab {\n        pane\n        pane\n        pane\n    }\n    tab {\n        pane\n        pane\n        pane\n    }\n}\n";
    let tpl = templates_of(TWO_TAB);
    assert_eq!(tpl.len(), 2);
    assert_eq!(
        tpl.iter().map(|t| t.n_slots).collect::<Vec<_>>(),
        vec![3, 3]
    );

    let p = plan_v2(&panes(4), &tpl, ANCHOR, "agents").unwrap();
    assert_eq!(p.s_slots, 6, "S = sum(N_t)");
    assert_eq!(p.k, 1, "M=4 <= S=6のため反復不要（M>N error廃止の再確認）");
    assert_eq!(p.blocks.len(), 1);
    assert_eq!(p.blocks[0].groups.len(), 2);
    assert_eq!(ids(&p, 0, 0).len(), 3);
    assert_eq!(
        ids(&p, 0, 1),
        vec![PaneKindId::Terminal(3)],
        "割当は累積slot式（割当式の本体はmulti-tab-allocation-cumulative-slots）"
    );
}

// [covers:layout-planner.r12-zellij-bare-bools-and-property-nodes-parse]
#[test]
fn r12_zellij_bare_bools_and_property_nodes_parse() {
    // zellij layoutはbare bool（borderless=true）とproperty node（cwd/start_suspended）
    // を常用する。parse時にquote正規化されること（MR-16回帰）。v2.2は正規形鋳型の
    // n_slotsとして数え上げる
    let bar_layout = "layout {\n    pane size=1 borderless=true { plugin location=\"zellij:tab-bar\" }\n    pane focus=true\n}\n";
    assert_eq!(templates_of(bar_layout)[0].n_slots, 1);

    let with_cwd = "layout {\n    cwd \"/work\"\n    pane\n}\n";
    assert_eq!(templates_of(with_cwd)[0].n_slots, 1);

    let with_suspend = "layout {\n    tab {\n        pane command=\"x\" {\n            start_suspended true\n        }\n    }\n}\n";
    assert_eq!(templates_of(with_suspend)[0].n_slots, 1);
}

// ---- R20: preflight warning ----

// [covers:layout-planner.r20-quoted-chars-in-pane-command-warn-but-do-not-fail]
#[test]
fn r20_quoted_chars_in_pane_command_warn_but_do_not_fail() {
    // terminal_commandに `"` / `\` を含むpaneがあればpreflight warning（実行は可。
    // DD-10.13。空白分割でquoteが復元できずrun一致が外れるため）
    let source = vec![
        {
            let mut p = pane(1, "ok", None);
            p.terminal_command = Some("codex exec".into());
            p
        },
        {
            let mut p = pane(2, "dq", None);
            p.terminal_command = Some("echo \"x\"".into());
            p
        },
        {
            let mut p = pane(3, "bs", None);
            p.terminal_command = Some("echo C:\\path".into());
            p
        },
    ];
    let p = plan_v2(&source, &templates_n(3), ANCHOR, "agents").unwrap();
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
    let p = plan_v2(&panes(3), &templates_n(3), ANCHOR, "agents").unwrap();
    assert!(p.warnings.is_empty());
}

// [covers:layout-planner.quoting-warning-covers-single-quotes-newlines-control-chars]
#[test]
fn c6_quoting_warning_covers_single_quotes_newlines_control_chars() {
    // warning対象は `"` / `\` に加え `'`・改行・制御文字（split_whitespaceで
    // argvが壊れうる文字。DD-10.13。C6回帰）
    let source = [
        (1, "sq", "bash -lc 'echo a b'"),
        (2, "nl", "echo a\nb"),
        (3, "ctl", "echo \u{1b}[0m"),
        (4, "ok", "codex exec --plan"),
    ]
    .into_iter()
    .map(|(id, title, invoked)| {
        let mut p = pane(id, title, None);
        p.terminal_command = Some(invoked.into());
        p
    })
    .collect::<Vec<_>>();
    let p = plan_v2(&source, &templates_n(4), ANCHOR, "agents").unwrap();
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

// [covers:layout-planner.run-derives-from-invoked-with]
#[test]
fn run_is_derived_from_terminal_command_not_foreground_process() {
    let mut shell_proc = pane(1, "shell", Some("vim src/main.rs"));
    shell_proc.terminal_command = None;
    let mut invoked = pane(2, "cmd", None);
    invoked.terminal_command = Some("claude --model x".into());
    let mut ordinary = pane(3, "ordinary", None);
    ordinary.terminal_command = None;
    let p = plan_v2(
        &[shell_proc, invoked, ordinary],
        &templates_n(3),
        ANCHOR,
        "agents",
    )
    .unwrap();
    let runs: Vec<_> = p.blocks[0].groups[0]
        .assignments
        .iter()
        .map(|a| a.run.clone())
        .collect();
    assert_eq!(runs[0], None);
    assert_eq!(
        runs[1],
        Some(
            vec!["claude", "--model", "x"]
                .into_iter()
                .map(String::from)
                .collect()
        )
    );
    assert_eq!(runs[2], None);
}

// ---- TASK-75（DD-10 v2.2: 正規形TabTemplate列・multi-tab配分） ----

/// ref-hetero相当のT=3 layout（N_t=9,1,2 → S=12）
const HETERO: &str = "layout {\n    tab {\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n    }\n    tab {\n        pane\n    }\n    tab {\n        pane\n        pane\n    }\n}\n";

// [covers:layout-planner.normalize-tab-templates-doc-order-and-tab-attributes]
#[test]
fn normalize_tab_templates_doc_order_and_tab_attributes() {
    // 正規形TabTemplate列構築の本体: 各tab nodeを文書順に列挙し、tab属性
    // （name・focus）はsubtree本体から除去して鋳型fieldへ保持する。
    // tab無しlayoutは1つの名無し鋳型（T=1・旧base_subtree相当）。
    // new_tab_templateはSKIP_NODES維持で鋳型化対象外
    let two_named = "layout {\n    tab name=\"T-a\" focus=true {\n        pane\n        pane\n    }\n    tab {\n        pane\n        pane\n    }\n}\n";
    let tpl = templates_of(two_named);
    assert_eq!(tpl.len(), 2, "tab node数 = 鋳型数（文書順）");
    assert_eq!(tpl[0].name.as_deref(), Some("T-a"));
    assert!(tpl[0].focus);
    assert_eq!(tpl[0].n_slots, 2);
    assert_eq!(tpl[1].name, None);
    assert!(!tpl[1].focus);
    assert_eq!(tpl[1].n_slots, 2);
    // subtree本体からtab属性（name/focus）は除去される
    for t in &tpl {
        let text = format!("{}", t.subtree);
        assert!(!text.contains("focus"), "raw: {text}");
        assert!(!text.contains("T-a"), "raw: {text}");
        assert!(
            t.subtree.nodes().iter().all(|n| n.name().value() != "tab"),
            "subtreeはtab children（tab nodeを含まない）: {text}"
        );
    }

    // tab無しlayout（layout直下pane 3つ）は1つの名無し鋳型
    let no_tab = "layout {\n    pane\n    pane\n    pane\n}\n";
    let tpl = templates_of(no_tab);
    assert_eq!(tpl.len(), 1);
    assert_eq!(tpl[0].name, None);
    assert!(!tpl[0].focus);
    assert_eq!(tpl[0].n_slots, 3);

    // new_tab_template混在layout: new_tab_templateは鋳型に含めない（T=tab数のみ）
    let with_new_tpl = "layout {\n    new_tab_template {\n        pane\n    }\n    tab {\n        pane\n        pane\n    }\n}\n";
    let tpl = templates_of(with_new_tpl);
    assert_eq!(tpl.len(), 1);
    assert_eq!(tpl[0].n_slots, 2);

    // 複数のfocus=true鋳型（tab focus）は文書順最初を採用しwarning 1件
    // （E2E要因改修・2026-09-16: zellijはdump上常にactive tabへfocus=trueを付ける
    // ため複数指定を機械的に判別できず、zelper側規則として文書順最初に確定し
    // 採用を利用者へ通知する。§5.2 (v)系列b）
    let focus_single =
        "layout {\n    tab focus=true {\n        pane\n    }\n    tab {\n        pane\n    }\n}\n";
    let (tpl, warn) = normalized(focus_single);
    assert!(tpl[0].focus);
    assert!(
        warn.is_empty(),
        "focus=true鋳型1件ならwarningなし: {warn:?}"
    );
    let focus_dual = "layout {\n    tab focus=true {\n        pane\n    }\n    tab {\n        pane\n    }\n    tab focus=true {\n        pane\n    }\n}\n";
    let (tpl, warn) = normalized(focus_dual);
    assert_eq!(
        warn.len(),
        1,
        "複数focus=true鋳型検出のwarning 1件: {warn:?}"
    );
    assert!(
        warn[0].to_lowercase().contains("focus"),
        "warning文面は複数focus検出を示す: {warn:?}"
    );
    // 採用（文書順最初のt*）はplan_v2のtab_focus_targetで検証
    // （t1-formula-level-backward-compat・final-go-to系）
    assert!(tpl.iter().filter(|t| t.focus).count() == 2);
}

// [covers:layout-planner.multi-tab-allocation-cumulative-slots]
#[test]
fn multi_tab_allocation_cumulative_slots() {
    // 累積slot式: i -> b=floor(i/S)・o=i%S -> C_tについてo in [C_t, C_t+N_t)を
    // 満たすt・s=o-C_t（visual order逐次充填）。生成tabは全block全tab（k×T）
    let tpl = templates_of(HETERO);
    assert_eq!(
        tpl.iter().map(|t| t.n_slots).collect::<Vec<_>>(),
        vec![9, 1, 2]
    );
    let source = panes(15);
    let p = plan_v2(&source, &tpl, ANCHOR, "ref-hetero").unwrap();
    assert_eq!(p.k, 2);
    assert_eq!(p.blocks.len(), 2);
    for b in 0..2 {
        assert_eq!(p.blocks[b].groups.len(), 3, "各blockのgroups長T=3");
    }
    // i=0..8 -> group (0,0) slot 0..8
    assert_eq!(
        ids(&p, 0, 0),
        (0..9).map(PaneKindId::Terminal).collect::<Vec<_>>()
    );
    let slots00: Vec<usize> = p.blocks[0].groups[0]
        .assignments
        .iter()
        .map(|a| a.slot)
        .collect();
    assert_eq!(slots00, (0..9).collect::<Vec<_>>());
    // i=9 -> (0,1) slot 0・i=10,11 -> (0,2) slot 0,1
    assert_eq!(ids(&p, 0, 1), vec![PaneKindId::Terminal(9)]);
    assert_eq!(group(&p, 0, 1).assignments[0].slot, 0);
    assert_eq!(
        ids(&p, 0, 2),
        vec![PaneKindId::Terminal(10), PaneKindId::Terminal(11)]
    );
    assert_eq!(group(&p, 0, 2).assignments[0].slot, 0);
    assert_eq!(group(&p, 0, 2).assignments[1].slot, 1);
    // i=12,13,14 -> (1,0) slot 0,1,2・(1,1)/(1,2)は割当0件
    assert_eq!(
        ids(&p, 1, 0),
        vec![
            PaneKindId::Terminal(12),
            PaneKindId::Terminal(13),
            PaneKindId::Terminal(14)
        ]
    );
    let slots10: Vec<usize> = p.blocks[1].groups[0]
        .assignments
        .iter()
        .map(|a| a.slot)
        .collect();
    assert_eq!(slots10, vec![0, 1, 2]);
    assert!(group(&p, 1, 1).assignments.is_empty());
    assert!(group(&p, 1, 2).assignments.is_empty());

    // 割当は入力並び順に依存せずvisual orderで固定（r10規則のmulti-tab拡張）
    let mut shuffled = source.clone();
    shuffled.reverse();
    let p2 = plan_v2(&shuffled, &tpl, ANCHOR, "ref-hetero").unwrap();
    assert_eq!(p2, p, "入力並びによらず同一plan");
}

// [covers:layout-planner.k-uses-total-slots]
#[test]
fn k_uses_total_slots() {
    // k = max(1, ceil(M/S))。M=0はk=1で全tab全slot空
    let tpl = templates_of(HETERO); // S=12
    for (m, want_k) in [(11usize, 1usize), (12, 1), (13, 2), (0, 1)] {
        let p = plan_v2(&panes(m), &tpl, ANCHOR, "b").unwrap();
        assert_eq!(p.k, want_k, "M={m}");
    }
    let p0 = plan_v2(&[], &tpl, ANCHOR, "b").unwrap();
    assert_eq!(p0.k, 1);
    assert_eq!(p0.m, 0);
    for (t, want) in [(0usize, 9usize), (1, 1), (2, 2)] {
        assert!(group(&p0, 0, t).assignments.is_empty());
        assert_eq!(group(&p0, 0, t).empty_slots, want, "M=0は全group空");
    }
}

// [covers:layout-planner.tab-names-from-templates-and-block-suffix]
#[test]
fn tab_names_from_templates_and_block_suffix() {
    // 命名: (0,0)=None（anchor保持）・幹はT>=2かつ鋳型名ありなら鋳型名、
    // そうでなければbase・b>=1は-<b+1>接尾。名無し鋳型のzellij自動名（Tab #N）は
    // 再現対象外でzelper命名に置換（U75-8）
    let named = "layout {\n    tab name=\"T-3x3\" {\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n    }\n    tab name=\"T-single\" {\n        pane\n    }\n    tab {\n        pane\n        pane\n    }\n}\n";
    let tpl = templates_of(named);
    let p = plan_v2(&panes(15), &tpl, ANCHOR, "ref-hetero").unwrap();
    assert_eq!(group(&p, 0, 0).name, None, "(0,0)はanchor（renameしない）");
    assert_eq!(group(&p, 0, 1).name.as_deref(), Some("T-single"));
    assert_eq!(group(&p, 0, 2).name.as_deref(), Some("ref-hetero"));
    assert_eq!(group(&p, 1, 0).name.as_deref(), Some("T-3x3-2"));
    assert_eq!(group(&p, 1, 1).name.as_deref(), Some("T-single-2"));
    assert_eq!(group(&p, 1, 2).name.as_deref(), Some("ref-hetero-2"));
}

// [covers:layout-planner.zero-slot-tab-template-invalid]
#[test]
fn zero_slot_tab_template_invalid() {
    // multi-tab内の部分的なN_t=0鋳型（plugin leafのみのtab等）はLayoutInvalid
    // （事前中断。T=1全体のN_0=0はr11が検証）
    let invalid = "layout {\n    tab {\n        pane\n        pane\n        pane\n    }\n    tab {\n        pane size=1 borderless=true {\n            plugin location=\"zellij:compact-bar\"\n        }\n    }\n}\n";
    let err = normalize_tab_templates(&zelper::layout::parse(invalid).unwrap()).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::LayoutInvalid,
        "N_t=0鋳型はnormalize時点でLayoutInvalid: {}",
        err.message()
    );
}

// [covers:layout-planner.pane-focus-slot-first-focused-leaf]
#[test]
fn pane_focus_slot_first_focused_leaf() {
    // pane_focus_slot = 文書順最初のfocus=true terminal leafのslot index。
    // 複数focus=trueは文書順最初を採用しwarning（U75-1）。focus無しはNone
    let focus_mid =
        "layout {\n    tab {\n        pane\n        pane focus=true\n        pane\n    }\n}\n";
    let (tpl, warn) = normalized(focus_mid);
    assert_eq!(tpl[0].pane_focus_slot, Some(1));
    assert!(warn.is_empty(), "raw: {warn:?}");

    let focus_two = "layout {\n    tab {\n        pane focus=true\n        pane\n        pane focus=true\n    }\n}\n";
    let (tpl, warn) = normalized(focus_two);
    assert_eq!(tpl[0].pane_focus_slot, Some(0), "文書順最初を採用");
    assert_eq!(warn.len(), 1, "複数focus検出のwarning 1件: {warn:?}");
    assert!(
        warn[0].to_lowercase().contains("focus"),
        "warning文面はfocus検出を示す: {warn:?}"
    );

    let no_focus = "layout {\n    tab {\n        pane\n        pane\n        pane\n    }\n}\n";
    let (tpl, warn) = normalized(no_focus);
    assert_eq!(tpl[0].pane_focus_slot, None);
    assert!(warn.is_empty());
}

// [covers:layout-planner.empty-group-planned-for-all-blocks-tabs]
#[test]
fn empty_group_planned_for_all_blocks_tabs() {
    // 割当0件のgroupもV2TabPlanとして生成され、empty_slots=N_t・nameは命名規則どおり
    let tpl = templates_of(HETERO);
    let p = plan_v2(&panes(15), &tpl, ANCHOR, "ref-hetero").unwrap();
    // block 1の割当は3件で(1,1)/(1,2)が空group
    assert_eq!(group(&p, 1, 1).assignments.len(), 0);
    assert_eq!(group(&p, 1, 1).empty_slots, 1);
    assert_eq!(group(&p, 1, 2).assignments.len(), 0);
    assert_eq!(group(&p, 1, 2).empty_slots, 2);
    // nameは命名規則どおり（-2接尾付き）
    assert_eq!(group(&p, 1, 1).name.as_deref(), Some("ref-hetero-2"));
    assert_eq!(group(&p, 1, 2).name.as_deref(), Some("ref-hetero-2"));
    // block 0の各groupに空groupなし
    for t in 0..3 {
        assert!(
            !group(&p, 0, t).assignments.is_empty(),
            "block 0のgroup (0,{t})は空でない"
        );
    }
}

// [covers:layout-planner.t1-formula-level-backward-compat]
#[test]
fn t1_formula_level_backward_compat() {
    // T=1式レベル後方互換: 割当式b=floor(i/N)・s=i%N・k=max(1,ceil(M/N))・
    // 命名は幹=baseで(0,0)=anchor・b>=1は<base>-<b+1>・tab focus復帰先はanchor。
    // 単tab node layoutの鋳型nameはT=1のため命名幹に使われない
    let single_named =
        "layout {\n    tab name=\"T\" {\n        pane\n        pane\n        pane\n    }\n}\n";
    let no_tab = "layout {\n    pane\n    pane\n    pane\n}\n";
    for text in [single_named, no_tab] {
        let tpl = templates_of(text);
        assert_eq!(tpl.len(), 1);
        let p = plan_v2(&panes(4), &tpl, ANCHOR, "agents").unwrap();
        assert_eq!(p.k, 2);
        // 割当: b=floor(i/3)・s=i%3（v2.1のinstance j式・slot式と同一）
        assert_eq!(
            ids(&p, 0, 0),
            vec![
                PaneKindId::Terminal(0),
                PaneKindId::Terminal(1),
                PaneKindId::Terminal(2)
            ]
        );
        let slots00: Vec<usize> = p.blocks[0].groups[0]
            .assignments
            .iter()
            .map(|a| a.slot)
            .collect();
        assert_eq!(slots00, vec![0, 1, 2]);
        assert_eq!(ids(&p, 1, 0), vec![PaneKindId::Terminal(3)]);
        assert_eq!(group(&p, 1, 0).assignments[0].slot, 0);
        // 命名: (0,0)=None（anchor）・block 1はbase幹の<base>-2
        // （単tab layoutの鋳型name "T" はT=1のため幹に使われない）
        assert_eq!(group(&p, 0, 0).name, None);
        assert_eq!(group(&p, 1, 0).name.as_deref(), Some("agents-2"));
        // focus=true鋳型なし: tab_focus_targetは立たない（実行はanchor復帰。
        // この系列はremap-sequence.final_go_to_honors_template_focus系列cが検証）
        assert!(!group(&p, 0, 0).tab_focus_target);
        assert!(!group(&p, 1, 0).tab_focus_target);
    }

    // focus=true付き単tab layout: t*=0しか存在せず block 0 tab 0 = anchor が
    // tab_focus_target（§2.7: focus指定の有無によらず最終go-to先はanchor＝
    // 現行のanchor復帰と同一の挙動）
    let single_focused = "layout {\n    tab name=\"T\" focus=true {\n        pane\n        pane\n        pane\n    }\n}\n";
    let tpl = templates_of(single_focused);
    let p = plan_v2(&panes(4), &tpl, ANCHOR, "agents").unwrap();
    assert!(group(&p, 0, 0).tab_focus_target, "t*=0 -> (0,0)=anchor");
    assert!(!group(&p, 1, 0).tab_focus_target, "block 0以外は立たない");
}
