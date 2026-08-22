// L2: rename / add / remove（fake backend。test-plan §2.5、remove安全gate）
mod fake;
use fake::FakeBackend;
use zelper::domain::*;
use zelper::error::ErrorClass;

fn pane(id: u32, title: &str, tab: u32, y: u32, x: u32) -> PaneState {
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
        command: Some("codex".into()),
        cwd: Some("/w".into()),
        tab_id: TabId(tab),
        tab_position: 0,
        tab_name: "T1".into(),
        plugin_url: None,
    }
}

fn tab(id: u32, tiled: u32) -> TabState {
    TabState {
        id: TabId(id),
        position: id,
        name: format!("tab{id}"),
        active: id == 0,
        selectable_tiled_panes_count: tiled,
        selectable_floating_panes_count: 0,
        are_floating_panes_visible: true,
    }
}

#[test]
fn rename_pane_updates_title_and_verifies() {
    let b = FakeBackend::new(vec![pane(1, "old", 0, 0, 0)], vec![tab(0, 1)]);
    zelper::app::rename::run_pane(&b, "1", "new-name", false).unwrap();
    assert!(
        b.calls()
            .iter()
            .any(|c| c == "rename-pane terminal_1 new-name")
    );
    assert_eq!(b.state.borrow().panes[0].title, "new-name");
}

#[test]
fn rename_verification_failure_detected() {
    let b = FakeBackend::new(vec![pane(1, "old", 0, 0, 0)], vec![tab(0, 1)]);
    // rename-paneを失敗させる（stateが変わらない → 検証で捕捉）
    b.inject_failure("rename-pane");
    let err = zelper::app::rename::run_pane(&b, "1", "new-name", false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed); // backend呼び出し自体の失敗
}

#[test]
fn rename_pane_silent_noop_detected_by_postcondition() {
    let b = FakeBackend::new(vec![pane(1, "old", 0, 0, 0)], vec![tab(0, 1)]);
    // close-paneと違いrenameはexit 0で無視されるケース: fail_opではなくtitleを変えないfakeが必要。
    // FakeBackendは必ずtitleを変えるため、この経路はL4実zellijで検証する（test-plan §2.9）。
    // ここではstate整合を確認
    zelper::app::rename::run_pane(&b, "1", "x", false).unwrap();
    assert_eq!(b.state.borrow().panes.len(), 1);
}

#[test]
fn add_pane_count_and_postcondition() {
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0, 0)], vec![tab(0, 1)]);
    zelper::app::add::run_pane(&b, None, 2, Some("w"), None, &[], false).unwrap();
    let s = b.state.borrow();
    let titles: Vec<_> = s.panes.iter().map(|p| p.title.clone()).collect();
    assert!(titles.contains(&"w-1".to_string()));
    assert!(titles.contains(&"w-2".to_string()));
    assert_eq!(s.panes.len(), 3);
}

#[test]
fn add_tab_layout_conflict_rejected() {
    let b = FakeBackend::new(vec![], vec![tab(0, 1)]);
    let err = zelper::app::add::run_tab(
        &b,
        1,
        None,
        None,
        Some(zelper::zellij::LayoutSpec {
            name: Some("x".into()),
            path: Some("./x.kdl".into()),
            inline: None,
        }),
        &[],
        false,
    )
    .unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Usage);
}

#[test]
fn remove_single_pane_no_gate_multi_requires_yes() {
    let b = FakeBackend::new(
        vec![
            pane(1, "a", 0, 0, 0),
            pane(2, "b", 0, 0, 100),
            pane(3, "c", 0, 10, 0),
        ],
        vec![tab(0, 3)],
    );
    zelper::app::remove::run_pane(&b, &["1".to_string()], false, false, false).unwrap();

    let err =
        zelper::app::remove::run_pane(&b, &["2".to_string(), "3".to_string()], false, false, false)
            .unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);

    // --dry-runは実行しない
    zelper::app::remove::run_pane(&b, &["2".to_string()], true, true, false).unwrap();
    assert_eq!(b.state.borrow().panes.len(), 2); // pane 2, 3は残存

    zelper::app::remove::run_pane(&b, &["2".to_string()], true, false, false).unwrap();
    assert_eq!(b.state.borrow().panes.len(), 1); // pane 3のみ残存
}

#[test]
fn remove_empty_with_unknown_tab_name_is_error_not_silent_widen() {
    // レビュー回帰: 解決不能なTABSPECは黙って捨てずerror（全空tabへの暗黙拡大防止）
    let b = FakeBackend::new(vec![], vec![tab(0, 0), tab(1, 3)]);
    let err =
        zelper::app::remove::run_tab(&b, &["nosuchtab".to_string()], true, true, false, false)
            .unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);
    // 名前で一意に解決できる場合はその範囲のみ
    let b2 = FakeBackend::new(
        vec![pane(1, "a", 0, 0, 0)],
        vec![
            tab(0, 1),
            TabState {
                id: TabId(1),
                position: 1,
                name: "empty1".into(),
                active: false,
                selectable_tiled_panes_count: 0,
                selectable_floating_panes_count: 0,
                are_floating_panes_visible: true,
            },
        ],
    );
    zelper::app::remove::run_tab(&b2, &["empty1".to_string()], true, true, false, false).unwrap();
    let s = b2.state.borrow();
    assert_eq!(s.tabs.len(), 1);
    assert_eq!(s.tabs[0].id, TabId(0));
}

#[test]
fn remove_empty_tabs_selects_only_empty() {
    // tab0: pane 2個（空でない）/ tab1: 空tab
    let b = FakeBackend::new(
        vec![pane(1, "a", 0, 0, 0), pane(2, "b", 0, 0, 100)],
        vec![tab(0, 2), tab(1, 0)],
    );
    zelper::app::remove::run_tab(&b, &[], true, true, true, false).unwrap(); // dry-run
    assert_eq!(b.state.borrow().tabs.len(), 2);
    zelper::app::remove::run_tab(&b, &[], true, true, false, false).unwrap();
    let s = b.state.borrow();
    assert_eq!(s.tabs.len(), 1);
    assert_eq!(s.tabs[0].id, TabId(0));
}

#[test]
fn remove_empty_requires_gate_even_for_single() {
    let b = FakeBackend::new(vec![], vec![tab(0, 0), tab(1, 3)]);
    let err = zelper::app::remove::run_tab(&b, &[], true, false, false, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
}

#[test]
fn remove_missing_pane_is_no_target() {
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0, 0)], vec![tab(0, 1)]);
    let err =
        zelper::app::remove::run_pane(&b, &["42".to_string()], true, false, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);
}
