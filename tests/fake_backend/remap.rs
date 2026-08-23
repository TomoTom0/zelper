// L1/L2: remap planner matrix R1〜R10 + KDL + 実行順序（test-plan §2.7）
mod fake;
use fake::FakeBackend;
use zelper::app::remap::{PlanMode, RemapArgs, plan};
use zelper::cli::OverflowMode;
use zelper::domain::*;
use zelper::error::ErrorClass;

fn pane(id: u32, title: &str, y: u32, x: u32, cmd: Option<&str>) -> PaneState {
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
        tab_id: TabId(0),
        tab_position: 0,
        tab_name: "T".into(),
        plugin_url: None,
    }
}

fn panes(n: usize) -> Vec<PaneState> {
    (0..n)
        .map(|i| {
            pane(
                i as u32,
                &format!("p{i}"),
                (i / 3) as u32 * 10,
                (i % 3) as u32 * 10,
                Some(&format!("cmd{i}")),
            )
        })
        .collect()
}

const T: TabId = TabId(0);

// ---- planner matrix ----

#[test]
fn r1_one_pane_into_three_slots_fills_with_shells() {
    let p = plan(T, "T", &panes(1), 3, None).unwrap();
    assert_eq!(p.mode, PlanMode::Fill);
    assert_eq!(p.instances.len(), 1);
    assert_eq!(p.instances[0].assignments.len(), 1);
    assert_eq!(p.instances[0].empty_slots, 2);
    assert!(p.instances[0].assignments[0].preserved);
}

#[test]
fn r2_three_into_three_exact_preserved() {
    let p = plan(T, "T", &panes(3), 3, None).unwrap();
    assert_eq!(p.mode, PlanMode::Fill);
    assert_eq!(p.instances[0].empty_slots, 0);
    assert!(p.instances[0].assignments.iter().all(|a| a.preserved));
}

#[test]
fn r3_overflow_default_is_error_with_guidance() {
    let err = plan(T, "T", &panes(4), 3, None).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    assert!(err.message().contains("--overflow nest"));
    assert!(err.message().contains("--overflow tabs"));
}

#[test]
fn r3n_nest_preserves_all() {
    let p = plan(T, "T", &panes(4), 3, Some(OverflowMode::Nest)).unwrap();
    assert_eq!(p.mode, PlanMode::Nest);
    assert_eq!(p.instances.len(), 1);
    assert_eq!(p.instances[0].assignments.len(), 4);
    assert!(p.instances[0].assignments.iter().all(|a| a.preserved));
}

#[test]
fn r3t_tabs_first_instance_preserved_rest_recreated() {
    let p = plan(T, "T", &panes(4), 3, Some(OverflowMode::Tabs)).unwrap();
    assert_eq!(p.mode, PlanMode::Tabs);
    assert_eq!(p.instances.len(), 2);
    assert!(p.instances[0].assignments.iter().all(|a| a.preserved));
    let overflow = &p.instances[1].assignments;
    assert_eq!(overflow.len(), 1);
    assert!(!overflow[0].preserved);
    assert_eq!(overflow[0].command_argv, vec!["cmd3"]);
}

#[test]
fn r4_six_into_three_two_instances() {
    let p = plan(T, "T", &panes(6), 3, Some(OverflowMode::Tabs)).unwrap();
    assert_eq!(p.instances.len(), 2);
    assert_eq!(p.instances[1].assignments.len(), 3);
    assert_eq!(p.instances[1].empty_slots, 0);
}

#[test]
fn r5_seven_into_three_three_instances_last_partial() {
    let p = plan(T, "T", &panes(7), 3, Some(OverflowMode::Tabs)).unwrap();
    assert_eq!(p.instances.len(), 3);
    assert_eq!(p.instances[2].assignments.len(), 1);
    assert_eq!(p.instances[2].empty_slots, 2);
    assert_eq!(p.instances[2].assignments[0].command_argv, vec!["cmd6"]);
}

#[test]
fn r10_visual_order_assigns_source_order_to_slots() {
    let mut src = panes(3);
    // visual order: y, x 昇順に並べ替えたものがplanner入力（selectorと同じ規則）
    src.sort_by_key(|p| p.visual_key());
    let p = plan(T, "T", &src, 3, None).unwrap();
    let ids: Vec<_> = p.instances[0].assignments.iter().map(|a| a.pane).collect();
    assert_eq!(ids, src.iter().map(|s| s.id).collect::<Vec<_>>());
}

#[test]
fn shell_pane_is_bare_in_recreated_instance() {
    let mut src = panes(3);
    src.push(pane(9, "shell", 99, 0, Some("/bin/sh")));
    let p = plan(T, "T", &src, 3, Some(OverflowMode::Tabs)).unwrap();
    let overflow = &p.instances[1].assignments;
    assert!(overflow.iter().any(|a| a.command_argv.is_empty()));
}

#[test]
fn zero_slot_layout_rejected() {
    let err = plan(T, "T", &panes(1), 0, None).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::LayoutInvalid);
}

// ---- layout KDL ----

const THREE_SLOT: &str = "layout {\n    tab {\n        pane size=\"40%\"\n        pane size=\"30%\"\n        pane size=\"30%\"\n    }\n}\n";

#[test]
fn kdl_count_slots_variants() {
    let doc = zelper::layout::parse(THREE_SLOT).unwrap();
    assert_eq!(zelper::layout::count_terminal_slots(&doc), 3);

    // plugin leafは除外
    let with_plugin = "layout {\n    tab {\n        pane\n        pane {\n            plugin location=\"zellij:tab-bar\"\n        }\n    }\n}\n";
    let doc = zelper::layout::parse(with_plugin).unwrap();
    assert_eq!(zelper::layout::count_terminal_slots(&doc), 1);

    // container・floating・template
    let complex = "layout {\n    pane_template name=\"t\" {\n        pane\n    }\n    tab {\n        pane split_direction=\"vertical\" {\n            pane\n            pane command=\"htop\"\n        }\n    }\n    floating_panes {\n        pane\n    }\n}\n";
    let doc = zelper::layout::parse(complex).unwrap();
    assert_eq!(zelper::layout::count_terminal_slots(&doc), 2);
}

#[test]
fn kdl_generate_instance_injects_command() {
    let doc = zelper::layout::parse(THREE_SLOT).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    let mut cmds = std::collections::BTreeMap::new();
    cmds.insert(
        1,
        zelper::layout::SlotCommand {
            command_argv: vec!["bash".into(), "/work/hb.sh".into(), "p1".into()],
            cwd: Some("/w".into()),
        },
    );
    let kdl = zelper::layout::generate_instance_kdl(&base, "agents-2", &cmds).unwrap();
    // zellij 0.44.3はbare文字列値を拒否するため、command/cwd/argsは必ずquoteされること（S9回帰）
    assert!(kdl.contains("command=\"bash\""), "raw: {kdl}");
    assert!(kdl.contains("args \"/work/hb.sh\" \"p1\""), "raw: {kdl}");
    assert!(kdl.contains("cwd=\"/w\""), "raw: {kdl}");
    assert!(kdl.contains("tab name=\"agents-2\""));
    // 改行区切り形式（DD-3.3）: 生成物がそのまま再parse可能であること
    let reparsed = zelper::layout::parse(&kdl).unwrap();
    assert_eq!(zelper::layout::count_terminal_slots(&reparsed), 3);
}

// ---- executor（fake backend） ----

fn args(overflow: Option<OverflowMode>, dry_run: bool) -> RemapArgs<'static> {
    RemapArgs {
        layout: None,
        path: None,
        inline: Some(THREE_SLOT),
        tab: None,
        session_scope: false,
        overflow,
        embed_floating: false,
        dry_run,
        json: false,
    }
}

fn tab0(n: u32) -> TabState {
    TabState {
        id: TabId(0),
        position: 0,
        name: "T".into(),
        active: true,
        selectable_tiled_panes_count: n,
        selectable_floating_panes_count: 0,
        are_floating_panes_visible: true,
    }
}

#[test]
fn fill_mode_executes_override_with_retain_and_active_tab_only() {
    let b = FakeBackend::new(panes(3), vec![tab0(3)]);
    zelper::app::remap::run(&b, &args(None, false)).unwrap();
    let calls = b.calls();
    assert!(
        calls
            .iter()
            .any(|c| c == "override-layout active_only=true retain_t=true retain_p=true")
    );
    assert_eq!(b.state.borrow().panes.len(), 3); // paneは消えない
}

#[test]
fn dry_run_does_not_mutate() {
    let b = FakeBackend::new(panes(1), vec![tab0(1)]);
    zelper::app::remap::run(&b, &args(None, true)).unwrap();
    let calls = b.calls();
    assert!(!calls.iter().any(|c| c.starts_with("override-layout")));
    assert!(!calls.iter().any(|c| c.starts_with("new-tab")));
    assert!(!calls.iter().any(|c| c.starts_with("close-pane")));
}

#[test]
fn tabs_mode_sequence_close_override_newtab() {
    let b = FakeBackend::new(panes(6), vec![tab0(6)]);
    zelper::app::remap::run(&b, &args(Some(OverflowMode::Tabs), false)).unwrap();
    let calls = b.calls();
    let idx = |pred: &dyn Fn(&str) -> bool| calls.iter().position(|c| pred(c));
    let close = idx(&|c| c.starts_with("close-pane")).expect("close-pane called");
    let ov = idx(&|c| c.starts_with("override-layout")).expect("override called");
    let nt = idx(&|c| c.starts_with("new-tab")).expect("new-tab called");
    let snap = idx(&|c| c == "dump-layout").expect("snapshot");
    assert!(snap < close, "snapshot before mutation");
    assert!(close < ov, "close overflow before override");
    assert!(ov < nt, "new tab after override");
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("close-pane")).count(),
        3
    );
    assert_eq!(calls.iter().filter(|c| c.starts_with("new-tab")).count(), 1);
    // overflow 3 paneはclose済み。新tabにslot数分(3)のpaneが生成される
    assert_eq!(b.state.borrow().panes.len(), 6);
}

#[test]
fn r7_failure_on_second_instance_keeps_first_applied() {
    let b = FakeBackend::new(panes(6), vec![tab0(6)]);
    b.inject_failure("new-tab");
    let err = zelper::app::remap::run(&b, &args(Some(OverflowMode::Tabs), false)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
    let calls = b.calls();
    // instance 1（override）は適用済みでrollbackしない
    assert!(calls.iter().any(|c| c.starts_with("override-layout")));
}

#[test]
fn r8_floating_panes_block_or_embed() {
    let mut p = panes(2);
    let mut f = pane(9, "float", 0, 0, Some("htop"));
    f.is_floating = true;
    p.push(f);
    let b = FakeBackend::new(p, vec![tab0(3)]);
    let err = zelper::app::remap::run(&b, &args(None, false)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    assert!(err.candidates().contains(&"terminal_9".to_string()));

    let b2 = FakeBackend::new(
        {
            let mut p = panes(2);
            let mut f = pane(9, "float", 0, 0, Some("htop"));
            f.is_floating = true;
            p.push(f);
            p
        },
        vec![tab0(3)],
    );
    let mut a = args(None, false);
    a.embed_floating = true;
    zelper::app::remap::run(&b2, &a).unwrap();
    assert!(
        b2.calls()
            .iter()
            .any(|c| c.contains("toggle-embed terminal_9"))
    );
    assert!(!b2.state.borrow().panes.iter().any(|p| p.is_floating));
}

#[test]
fn dry_run_with_embed_floating_does_not_mutate_and_includes_them_in_plan() {
    // M-3回帰: dry-run + --embed-floating はtoggleしない（DD-12非破壊）
    let mut p = panes(3);
    let mut f = pane(9, "float", 0, 0, Some("htop"));
    f.is_floating = true;
    p.push(f);
    let b = FakeBackend::new(p, vec![tab0(4)]);
    let mut a = args(None, true);
    a.embed_floating = true;
    // 3 tiled + 1 floating(embed予定) = 4 pane > 3 slot → 計画段階でoverflow errorになる
    // （floating paneがsourceに含まれている証明。含まれないならFillで成功してしまう）
    let err = zelper::app::remap::run(&b, &a).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    assert!(err.message().contains("--overflow"));
    // 状態は一切変更されていない
    assert!(!b.calls().iter().any(|c| c.contains("toggle-embed")));
    assert!(b.state.borrow().panes.iter().any(|p| p.is_floating));
    assert!(!b.calls().iter().any(|c| c.starts_with("override-layout")));
}

// ---- レビュー回帰（MR-16〜: bare bool KDL・property node・multi-tab layout N・dry-run tab切替） ----

#[test]
fn kdl_accepts_zellij_bare_bools_and_property_nodes() {
    // zellij layoutはbare bool（borderless=true）とproperty node（cwd/start_suspended）を常用する。
    // kdl crate（KDL v2）はbare boolを拒否するため、parse時にquote正規化されること
    let bar_layout = "layout {\n    pane size=1 borderless=true { plugin location=\"zellij:tab-bar\" }\n    pane focus=true\n}\n";
    let doc = zelper::layout::parse(bar_layout).unwrap();
    // bar plugin leafはslot除外、focus=trueのpaneはslot
    assert_eq!(zelper::layout::count_terminal_slots(&doc), 1);

    let with_cwd = "layout {\n    cwd \"/work\"\n    pane\n}\n";
    let doc = zelper::layout::parse(with_cwd).unwrap();
    assert_eq!(zelper::layout::count_terminal_slots(&doc), 1);

    let with_suspend = "layout {\n    tab {\n        pane command=\"x\" {\n            start_suspended true\n        }\n    }\n}\n";
    let doc = zelper::layout::parse(with_suspend).unwrap();
    assert_eq!(zelper::layout::count_terminal_slots(&doc), 1);
}

#[test]
fn multi_tab_layout_counts_first_tab_slots_only() {
    // apply-only-to-active-tabはlayoutの先頭tabのみ適用する（実機help確認済み）。
    // 2-tab layout（各3 slot）に対し4 paneならN=3としてoverflow errorになること
    const TWO_TAB: &str = "layout {\n    tab {\n        pane\n        pane\n        pane\n    }\n    tab {\n        pane\n        pane\n        pane\n    }\n}\n";
    let b = FakeBackend::new(panes(4), vec![tab0(4)]);
    let mut a = args(None, false);
    a.inline = Some(TWO_TAB);
    let err = zelper::app::remap::run(&b, &a).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    assert!(err.message().contains("--overflow"));
    assert!(!b.calls().iter().any(|c| c.starts_with("override-layout")));
}

#[test]
fn dry_run_with_tab_does_not_switch_active_tab() {
    // dry-runはgo-to-tabも行わない（DD-12の非破壊）
    let tabs = vec![
        tab0(3),
        TabState {
            id: TabId(1),
            position: 1,
            name: "b".into(),
            active: false,
            selectable_tiled_panes_count: 3,
            selectable_floating_panes_count: 0,
            are_floating_panes_visible: true,
        },
    ];
    let p0 = panes(3);
    let mut p1 = panes(3);
    for (i, p) in p1.iter_mut().enumerate() {
        p.tab_id = TabId(1);
        p.id = PaneKindId::Terminal(10 + i as u32);
    }
    let b = FakeBackend::new(p0.into_iter().chain(p1).collect(), tabs);
    let mut a = args(None, true);
    a.tab = Some("b");
    zelper::app::remap::run(&b, &a).unwrap();
    assert!(!b.calls().iter().any(|c| c.starts_with("go-to-tab")));
}

#[test]
fn tabs_mode_renames_created_tab_and_reports_partial_state_on_failure() {
    let b = FakeBackend::new(panes(6), vec![tab0(6)]);
    zelper::app::remap::run(&b, &args(Some(OverflowMode::Tabs), false)).unwrap();
    // 生成tabは rename-tab で "<layout名>-2" になる（inlineなので "remap-2"）
    assert!(b.calls().iter().any(|c| c == "rename-tab 1 remap-2"));
    let names: Vec<String> = b
        .state
        .borrow()
        .tabs
        .iter()
        .map(|t| t.name.clone())
        .collect();
    assert!(names.contains(&"remap-2".to_string()));

    // 途中失敗時の部分適用報告（R7強化）: new-tab失敗で実行済み情報がmessageに載る
    let b2 = FakeBackend::new(panes(6), vec![tab0(6)]);
    b2.inject_failure("new-tab");
    let err = zelper::app::remap::run(&b2, &args(Some(OverflowMode::Tabs), false)).unwrap_err();
    assert!(err.message().contains("partial state"));
    assert!(err.message().contains("no rollback"));
}

#[test]
fn r6_session_scope_applies_per_tab_independently() {
    let tabs = vec![
        TabState {
            id: TabId(0),
            position: 0,
            name: "a".into(),
            active: true,
            selectable_tiled_panes_count: 2,
            selectable_floating_panes_count: 0,
            are_floating_panes_visible: true,
        },
        TabState {
            id: TabId(1),
            position: 1,
            name: "b".into(),
            active: false,
            selectable_tiled_panes_count: 2,
            selectable_floating_panes_count: 0,
            are_floating_panes_visible: true,
        },
    ];
    let mut p0 = panes(2);
    for p in &mut p0 {
        p.tab_id = TabId(0);
    }
    let mut p1 = panes(2);
    for (i, p) in p1.iter_mut().enumerate() {
        p.tab_id = TabId(1);
        p.id = PaneKindId::Terminal(10 + i as u32);
    }
    let all: Vec<_> = p0.into_iter().chain(p1).collect();
    let b = FakeBackend::new(all, tabs);
    let mut a = args(None, false);
    a.session_scope = true;
    zelper::app::remap::run(&b, &a).unwrap();
    // tiled paneを持つtabのみ（tab1は空のため対象外）、かつ各tab独立
    let overrides = b
        .calls()
        .iter()
        .filter(|c| c.starts_with("override-layout"))
        .count();
    assert_eq!(overrides, 2);
}

// ---- PR#1レビュー回帰（plugin leaf slot・再作成検証・floating変更タイミング・cwd・JSON envelope） ----

#[test]
fn plugin_leaf_does_not_consume_slot_index() {
    // PR#1: --overflow tabsでplugin leaf（tab bar等）がslot indexを消費すると、
    // commandがplugin paneに注入され以降のterminal paneが1つずれる
    let layout = "layout {\n    tab {\n        pane size=1 borderless=true {\n            plugin location=\"zellij:tab-bar\"\n        }\n        pane\n        pane\n    }\n}\n";
    let doc = zelper::layout::parse(layout).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    assert_eq!(zelper::layout::count_terminal_slots(&base), 2);

    let mut cmds = std::collections::BTreeMap::new();
    cmds.insert(
        0,
        zelper::layout::SlotCommand {
            command_argv: vec!["cmdA".into()],
            cwd: None,
        },
    );
    cmds.insert(
        1,
        zelper::layout::SlotCommand {
            command_argv: vec!["cmdB".into()],
            cwd: None,
        },
    );
    let kdl = zelper::layout::generate_instance_kdl(&base, "t", &cmds).unwrap();
    let reparsed = zelper::layout::parse(&kdl).unwrap();
    let specs = zelper::layout::extract_slot_commands(&reparsed);
    assert_eq!(
        specs
            .iter()
            .map(|s| s.command_argv.clone())
            .collect::<Vec<_>>(),
        vec![vec!["cmdA".to_string()], vec!["cmdB".to_string()]],
        "raw: {kdl}"
    );
    assert!(specs.iter().all(|s| s.cwd.is_none()));

    // bare plugin node（pane wrapperなし）もslotを形成しない
    let bare = "layout {\n    tab {\n        plugin location=\"zellij:compact-bar\"\n        pane\n        pane\n    }\n}\n";
    let doc = zelper::layout::parse(bare).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    assert_eq!(zelper::layout::count_terminal_slots(&base), 2);
}

#[test]
fn plugin_config_children_do_not_consume_slot_index() {
    // 独立レビューMR-32: plugin nodeのconfig子node（zjstatusのformat_left等）が
    // has_nested判定でnest扱いになると、config node自体がslotに数えられ・inject対象に
    // なる。plugin配下はすべてそのpluginのconfigurationでありslotを形成しない
    let layout = "layout {\n    plugin location=\"file:/plugins/zjstatus.wasm\" {\n        format_left \"{session_name}\"\n        format_right \"{datetime}\"\n    }\n    pane\n    pane\n}\n";
    let doc = zelper::layout::parse(layout).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    assert_eq!(zelper::layout::count_terminal_slots(&base), 2);

    let mut cmds = std::collections::BTreeMap::new();
    cmds.insert(
        0,
        zelper::layout::SlotCommand {
            command_argv: vec!["cmdA".into()],
            cwd: None,
        },
    );
    cmds.insert(
        1,
        zelper::layout::SlotCommand {
            command_argv: vec!["cmdB".into()],
            cwd: None,
        },
    );
    let kdl = zelper::layout::generate_instance_kdl(&base, "t", &cmds).unwrap();
    // layout直下のbare plugin（config子node込み）は生成KDLから除去される:
    // 生成KDLではbase subtreeがtab配下に置かれ、tab直下のpluginは実zellijで
    // Invalid tab property errorになるため（実機検証S11）。実zellijもbare pluginを
    // 無視するため除去しても挙動は変わらない
    assert!(!kdl.contains("plugin"), "raw: {kdl}");
    assert!(!kdl.contains("format_left"), "raw: {kdl}");
    let reparsed = zelper::layout::parse(&kdl).unwrap();
    let specs = zelper::layout::extract_slot_commands(&reparsed);
    assert_eq!(
        specs
            .iter()
            .map(|s| s.command_argv.clone())
            .collect::<Vec<_>>(),
        vec![vec!["cmdA".to_string()], vec!["cmdB".to_string()]],
        "raw: {kdl}"
    );

    // pane wrapper内のplugin + config子nodeはslotを形成せず、生成KDLにも保持される
    let wrapped = "layout {\n    pane size=1 borderless=true {\n        plugin location=\"file:/plugins/zjstatus.wasm\" {\n            format_left \"{session_name}\"\n        }\n    }\n    pane\n}\n";
    let doc = zelper::layout::parse(wrapped).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    assert_eq!(zelper::layout::count_terminal_slots(&base), 1);
    let mut cmds = std::collections::BTreeMap::new();
    cmds.insert(
        0,
        zelper::layout::SlotCommand {
            command_argv: vec!["cmdA".into()],
            cwd: None,
        },
    );
    let kdl = zelper::layout::generate_instance_kdl(&base, "t", &cmds).unwrap();
    assert!(kdl.contains("format_left \"{session_name}\""), "raw: {kdl}");
    assert_eq!(
        zelper::layout::extract_slot_commands(&zelper::layout::parse(&kdl).unwrap())
            .iter()
            .map(|s| s.command_argv.clone())
            .collect::<Vec<_>>(),
        vec![vec!["cmdA".to_string()]],
        "raw: {kdl}"
    );
}

#[test]
fn childless_tab_node_does_not_consume_slot_index() {
    // 独立レビューMR-31: 子なしtab/layout nodeはwalk_slots（count）と同様に
    // injectでもslotを形成しない。leaf扱いだと以降のpaneが1つずれる
    let layout = "layout {\n    tab\n    pane\n    pane\n}\n";
    let doc = zelper::layout::parse(layout).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    assert_eq!(zelper::layout::count_terminal_slots(&base), 2);

    let mut cmds = std::collections::BTreeMap::new();
    cmds.insert(
        0,
        zelper::layout::SlotCommand {
            command_argv: vec!["cmdA".into()],
            cwd: None,
        },
    );
    cmds.insert(
        1,
        zelper::layout::SlotCommand {
            command_argv: vec!["cmdB".into()],
            cwd: None,
        },
    );
    let kdl = zelper::layout::generate_instance_kdl(&base, "t", &cmds).unwrap();
    let reparsed = zelper::layout::parse(&kdl).unwrap();
    let specs = zelper::layout::extract_slot_commands(&reparsed);
    assert_eq!(
        specs
            .iter()
            .map(|s| s.command_argv.clone())
            .collect::<Vec<_>>(),
        vec![vec!["cmdA".to_string()], vec!["cmdB".to_string()]],
        "raw: {kdl}"
    );
}

#[test]
fn shell_only_pane_keeps_cwd_in_recreated_instance() {
    // PR#1: shellのみpane（argv空）の再作成でもcwdは注入される（既定dirで起動しない）
    let doc = zelper::layout::parse(THREE_SLOT).unwrap();
    let base = zelper::layout::base_subtree(&doc);
    let mut cmds = std::collections::BTreeMap::new();
    cmds.insert(
        0,
        zelper::layout::SlotCommand {
            command_argv: vec![],
            cwd: Some("/w/proj".into()),
        },
    );
    let kdl = zelper::layout::generate_instance_kdl(&base, "t", &cmds).unwrap();
    assert!(kdl.contains("cwd=\"/w/proj\""), "raw: {kdl}");
    let reparsed = zelper::layout::parse(&kdl).unwrap();
    let specs = zelper::layout::extract_slot_commands(&reparsed);
    assert_eq!(specs[0].cwd.as_deref(), Some("/w/proj"));
    assert!(specs[0].command_argv.is_empty());
    // 他slotは素のまま
    assert!(specs[1].cwd.is_none() && specs[2].cwd.is_none());
}

#[test]
fn embed_floating_layout_error_does_not_mutate() {
    // PR#1: layout解決errorはfloating paneのtiled化より前に返る（状態変更なし）
    let mut p = panes(2);
    let mut f = pane(9, "float", 0, 0, Some("htop"));
    f.is_floating = true;
    p.push(f);
    let b = FakeBackend::new(p, vec![tab0(3)]);
    let mut a = args(None, false);
    a.embed_floating = true;
    a.inline = Some("layout {"); // 不正KDL（未close）
    let err = zelper::app::remap::run(&b, &a).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::LayoutInvalid);
    assert!(!b.calls().iter().any(|c| c.contains("toggle-embed")));
    assert!(b.state.borrow().panes.iter().any(|p| p.is_floating));
}

#[test]
fn embed_floating_plan_error_does_not_mutate() {
    // PR#1: M > N + overflow未指定のplan errorもtiled化より前（状態変更なし）
    let mut p = panes(3);
    let mut f = pane(9, "float", 0, 0, Some("htop"));
    f.is_floating = true;
    p.push(f);
    let b = FakeBackend::new(p, vec![tab0(4)]);
    let mut a = args(None, false);
    a.embed_floating = true;
    let err = zelper::app::remap::run(&b, &a).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    assert!(err.message().contains("--overflow"));
    assert!(!b.calls().iter().any(|c| c.contains("toggle-embed")));
    assert!(b.state.borrow().panes.iter().any(|p| p.is_floating));
}

#[test]
fn tabs_mode_unrestarted_command_fails_verification() {
    // PR#1: 再作成されなかったcommandは検証失敗（ok:trueのまま成功扱いにしない）
    let b = FakeBackend::new(panes(6), vec![tab0(6)]);
    b.starve_command_restart("cmd4"); // instance 1の1 paneがbare paneに化ける
    let err = zelper::app::remap::run(&b, &args(Some(OverflowMode::Tabs), false)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert!(
        err.message().contains("command not restarted"),
        "raw: {}",
        err.message()
    );
    assert!(
        err.message().contains("terminal_4"),
        "raw: {}",
        err.message()
    );
    let missing = err.data().unwrap()["missing"].as_array().unwrap();
    assert!(missing.len() == 1);
}

#[test]
fn verification_failure_json_attaches_mapping_to_error() {
    // PR#1: 検証失敗時は成功envelopeをstdoutに出さず、mapping/missingをerror.dataに
    // 載せる（mainが単一のerror envelopeとして出力する）
    let b = FakeBackend::new(panes(3), vec![tab0(3)]);
    b.drop_on_next_override(PaneKindId::Terminal(1)); // layout適用でpane 1が消える
    let mut a = args(None, false);
    a.json = true;
    let err = zelper::app::remap::run(&b, &a).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    let data = err.data().expect("mapping data attached to error");
    let mapping = data["mapping"].as_array().unwrap();
    assert!(
        mapping
            .iter()
            .any(|m| m["pane"] == "terminal_1" && m["alive"] == false),
        "raw: {data}"
    );
    assert!(
        data["missing"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m.as_str().unwrap().contains("terminal_1")),
        "raw: {data}"
    );
}
