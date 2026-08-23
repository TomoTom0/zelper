// L2: resize（test-plan §2.6: step実行・no-op打ち切り・equalize収束・無限ループ禁止）
mod fake;
use fake::FakeBackend;
use zelper::domain::*;
use zelper::error::ErrorClass;
use zelper::zellij::ResizeDirection;

fn pane(id: u32, title: &str, y: u32, x: u32, rows: u32, cols: u32) -> PaneState {
    PaneState {
        id: PaneKindId::Terminal(id),
        title: title.to_string(),
        is_selectable: true,
        is_floating: false,
        is_focused: false,
        exited: false,
        is_held: false,
        geometry: Geometry { x, y, rows, cols },
        command: None,
        cwd: None,
        tab_id: TabId(0),
        tab_position: 0,
        tab_name: "T".into(),
        plugin_url: None,
    }
}

#[test]
fn grow_applies_steps_with_geometry_verification() {
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0, 10, 10)], vec![]);
    zelper::app::resize::run_pane(&b, "1", true, ResizeDirection::Right, 3, false).unwrap();
    let s = b.state.borrow();
    assert_eq!(s.panes[0].geometry.cols, 40);
    let resize_calls = b
        .calls()
        .iter()
        .filter(|c| c.starts_with("resize terminal_1"))
        .count();
    assert_eq!(resize_calls, 3);
}

#[test]
fn noop_stops_early_with_warning() {
    let b = FakeBackend::new(vec![pane(1, "frozen-a", 0, 0, 10, 10)], vec![]);
    // no-opでもerrorにしない（DD-9: 打ち切り+報告）
    zelper::app::resize::run_pane(&b, "1", true, ResizeDirection::Right, 5, false).unwrap();
    let s = b.state.borrow();
    assert_eq!(s.panes[0].geometry.cols, 10);
}

#[test]
fn equalize_same_row_converges() {
    let b = FakeBackend::new(
        vec![pane(1, "a", 0, 0, 30, 10), pane(2, "b", 0, 100, 30, 30)],
        vec![],
    );
    zelper::app::resize::run_equalize(&b, &["1".into(), "2".into()], None, false).unwrap();
    let s = b.state.borrow();
    let cols: Vec<u32> = s.panes.iter().map(|p| p.geometry.cols).collect();
    assert!(
        (cols[0] as i32 - cols[1] as i32).abs() <= 1,
        "cols: {cols:?}"
    );
    assert_eq!(cols.iter().sum::<u32>(), 40); // 総面積は保存
}

#[test]
fn equalize_oscillation_terminates_no_infinite_loop() {
    // 10と15の合計25は10刻みのresizeで均等化不能（振動する）
    let b = FakeBackend::new(
        vec![pane(1, "a", 0, 0, 30, 10), pane(2, "b", 0, 100, 30, 15)],
        vec![],
    );
    // timeoutではなく正常完了すること（無限ループ禁止の保証）
    zelper::app::resize::run_equalize(&b, &["1".into(), "2".into()], None, false).unwrap();
    // 完了すればよい（幾何は不問。notesはstderr/出力側）
}

#[test]
fn equalize_missing_pane_is_error_and_cross_tab_rejected() {
    // レビュー回帰: 不在IDは黙って除外しない
    let b = FakeBackend::new(
        vec![pane(1, "a", 0, 0, 10, 10), pane(2, "b", 0, 100, 10, 30)],
        vec![],
    );
    let err =
        zelper::app::resize::run_equalize(&b, &["1".into(), "99".into()], None, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);

    // 異tab混在はerror
    let mut p1 = pane(1, "a", 0, 0, 10, 10);
    p1.tab_id = TabId(0);
    let mut p2 = pane(2, "b", 0, 100, 10, 30);
    p2.tab_id = TabId(1);
    let b2 = FakeBackend::new(vec![p1, p2], vec![]);
    let err =
        zelper::app::resize::run_equalize(&b2, &["1".into(), "2".into()], None, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
}

#[test]
fn equalize_requires_targets() {
    let b = FakeBackend::new(vec![], vec![]);
    let err = zelper::app::resize::run_equalize(&b, &[], None, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Usage);
}

#[test]
fn equalize_rejects_floating() {
    let mut p = pane(1, "f", 0, 0, 10, 10);
    p.is_floating = true;
    let b = FakeBackend::new(vec![p], vec![]);
    let err = zelper::app::resize::run_equalize(&b, &["1".into()], None, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
}
