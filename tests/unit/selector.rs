// L1: target resolver（test-plan §2.2）
use zelper::domain::*;
use zelper::error::ErrorClass;
use zelper::selector::{resolve, resolve_single, resolve_tab};

fn pane(
    id: u32,
    title: &str,
    tab: u32,
    tab_pos: u32,
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
        cwd: Some("/tmp".into()),
        tab_id: TabId(tab),
        tab_position: tab_pos,
        tab_name: format!("tab{tab}"),
        plugin_url: None,
    }
}

fn fixtures() -> Vec<PaneState> {
    vec![
        pane(0, "shell", 1, 0, 30, 0, None),
        pane(1, "agent-a", 1, 0, 0, 0, Some("codex exec")),
        pane(2, "agent-b", 1, 0, 0, 100, Some("codex exec")),
        pane(3, "build", 2, 1, 0, 0, Some("cargo watch")),
        // non-selectable plugin pane（bar）
        PaneState {
            id: PaneKindId::Plugin(0),
            title: "tab-bar".into(),
            is_selectable: false,
            is_floating: false,
            is_focused: false,
            exited: false,
            is_held: false,
            geometry: Geometry {
                x: 0,
                y: 0,
                rows: 1,
                cols: 200,
            },
            command: None,
            cwd: None,
            tab_id: TabId(1),
            tab_position: 0,
            tab_name: "tab1".into(),
            plugin_url: Some("zellij:tab-bar".into()),
        },
        // floating pane（selectableだが対象外filter）
        PaneState {
            id: PaneKindId::Terminal(4),
            title: "float".into(),
            is_selectable: true,
            is_floating: true,
            is_focused: false,
            exited: false,
            is_held: false,
            geometry: Geometry {
                x: 0,
                y: 0,
                rows: 5,
                cols: 20,
            },
            command: Some("htop".into()),
            cwd: None,
            tab_id: TabId(2),
            tab_position: 1,
            tab_name: "tab2".into(),
            plugin_url: None,
        },
    ]
}

#[test]
fn positional_id_resolution_and_no_target() {
    let panes = fixtures();
    let spec = TargetSpec {
        pane_ids: vec![PaneKindId::Terminal(1)],
        ..Default::default()
    };
    let set = resolve(&spec, &panes).unwrap();
    assert_eq!(set.panes.len(), 1);
    assert_eq!(set.panes[0].id, PaneKindId::Terminal(1));

    let missing = TargetSpec {
        pane_ids: vec![PaneKindId::Terminal(99)],
        ..Default::default()
    };
    let err = resolve(&missing, &panes).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);
}

#[test]
fn filter_name_exact_match_and_command_partial() {
    let panes = fixtures();
    let spec = TargetSpec {
        name: Some("agent-a".into()),
        ..Default::default()
    };
    let set = resolve(&spec, &panes).unwrap();
    assert_eq!(set.panes.len(), 1);

    let spec = TargetSpec {
        command: Some("codex".into()),
        ..Default::default()
    };
    let set = resolve(&spec, &panes).unwrap();
    assert_eq!(set.panes.len(), 2);
}

#[test]
fn filter_all_selectable_terminal_only_excludes_plugin_and_floating() {
    let panes = fixtures();
    let spec = TargetSpec {
        all: true,
        ..Default::default()
    };
    let set = resolve(&spec, &panes).unwrap();
    // floating(terminal_4)はselectableだがall対象に含める仕様（DD-1.4はselectable terminal pane）
    // plugin_0はnon-selectableのため除外
    assert!(set.panes.iter().all(|p| p.is_selectable));
    assert!(
        !set.panes
            .iter()
            .any(|p| matches!(p.id, PaneKindId::Plugin(_)))
    );
}

#[test]
fn union_of_positional_and_filter() {
    let panes = fixtures();
    let spec = TargetSpec {
        pane_ids: vec![PaneKindId::Terminal(3)],
        command: Some("codex".into()),
        ..Default::default()
    };
    let set = resolve(&spec, &panes).unwrap();
    assert_eq!(set.panes.len(), 3);
}

#[test]
fn visual_order_deterministic() {
    let panes = fixtures();
    let spec = TargetSpec {
        all: true,
        ..Default::default()
    };
    let set = resolve(&spec, &panes).unwrap();
    let keys: Vec<_> = set.panes.iter().map(|p| p.visual_key()).collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

#[test]
fn single_target_ambiguous_and_ok() {
    let panes = fixtures();
    let spec = TargetSpec {
        command: Some("codex".into()),
        ..Default::default()
    };
    let err = resolve_single(&spec, &panes).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::AmbiguousTarget);
    assert_eq!(err.candidates().len(), 2);

    let spec = TargetSpec {
        pane_ids: vec![PaneKindId::Terminal(3)],
        ..Default::default()
    };
    assert!(resolve_single(&spec, &panes).is_ok());
}

#[test]
fn tab_resolution_by_id_name_ambiguous() {
    let tabs = vec![
        TabState {
            id: TabId(1),
            position: 0,
            name: "agents".into(),
            active: true,
            selectable_tiled_panes_count: 3,
            selectable_floating_panes_count: 0,
            are_floating_panes_visible: true,
        },
        TabState {
            id: TabId(2),
            position: 1,
            name: "agents".into(),
            active: false,
            selectable_tiled_panes_count: 1,
            selectable_floating_panes_count: 0,
            are_floating_panes_visible: false,
        },
        TabState {
            id: TabId(5),
            position: 2,
            name: "build".into(),
            active: false,
            selectable_tiled_panes_count: 2,
            selectable_floating_panes_count: 0,
            are_floating_panes_visible: false,
        },
    ];
    assert_eq!(resolve_tab("5", &tabs).unwrap(), TabId(5));
    assert_eq!(resolve_tab("build", &tabs).unwrap(), TabId(5));
    let err = resolve_tab("agents", &tabs).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::AmbiguousTarget);
    let err = resolve_tab("9", &tabs).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);
}

#[test]
fn empty_tab_definition() {
    let t = TabState {
        id: TabId(1),
        position: 0,
        name: "x".into(),
        active: true,
        selectable_tiled_panes_count: 0,
        selectable_floating_panes_count: 0,
        are_floating_panes_visible: false,
    };
    assert!(t.is_empty());
    let t2 = TabState {
        selectable_floating_panes_count: 1,
        ..t
    };
    assert!(!t2.is_empty());
}
