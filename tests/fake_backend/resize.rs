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
        terminal_command: None,
        cwd: None,
        tab_id: TabId(0),
        tab_position: 0,
        tab_name: "T".into(),
        plugin_url: None,
    }
}

// [covers:resize.grow-applies-steps-with-geometry-verification]
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

// [covers:resize.noop-stops-early-with-warning]
#[test]
fn noop_stops_early_with_warning() {
    let b = FakeBackend::new(vec![pane(1, "frozen-a", 0, 0, 10, 10)], vec![]);
    // no-opでもerrorにしない（DD-9: 打ち切り+報告）
    zelper::app::resize::run_pane(&b, "1", true, ResizeDirection::Right, 5, false).unwrap();
    let s = b.state.borrow();
    assert_eq!(s.panes[0].geometry.cols, 10);
}

// [covers:resize.equalize-same-row-converges]
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

// [covers:resize.equalize-oscillation-terminates-no-infinite-loop]
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

// [covers:resize.equalize-missing-pane-and-cross-tab-rejected]
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

// [covers:resize.equalize-requires-targets]
#[test]
fn equalize_requires_targets() {
    let b = FakeBackend::new(vec![], vec![]);
    let err = zelper::app::resize::run_equalize(&b, &[], None, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Usage);
}

// [covers:resize.equalize-rejects-floating]
#[test]
fn equalize_rejects_floating() {
    let mut p = pane(1, "f", 0, 0, 10, 10);
    p.is_floating = true;
    let b = FakeBackend::new(vec![p], vec![]);
    let err = zelper::app::resize::run_equalize(&b, &["1".into()], None, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
}

fn tab(id: u32, name: &str, active: bool) -> TabState {
    TabState {
        id: TabId(id),
        position: id,
        name: name.to_string(),
        active,
        selectable_tiled_panes_count: 2,
        selectable_floating_panes_count: 0,
        are_floating_panes_visible: true,
    }
}

/// --tab TABSPEC（TASK-68）のfixture: tab 0（cols=10/30の2 pane・非対象）と
/// tab 1 build（cols=10/30の2 pane・対象 + 同一行のexited pane・floating pane各1件）の混在
fn mixed_tab_fixture() -> (Vec<PaneState>, Vec<TabState>) {
    let mut p3 = pane(3, "c", 0, 0, 30, 10);
    p3.tab_id = TabId(1);
    let mut p4 = pane(4, "d", 0, 100, 30, 30);
    p4.tab_id = TabId(1);
    // exited・floating（is_remap_source=false）は対象paneと同一行（y=0）に配置:
    // 誤選択時にROW groupingへ混入して呼出列・幾何が変わり「黙除外されていること」が
    // 検出できる（単独行配置だと誤選択でもresizeされず検証不能）
    let mut p5 = pane(5, "gone", 0, 200, 30, 10);
    p5.tab_id = TabId(1);
    p5.exited = true;
    let mut p6 = pane(6, "float", 0, 300, 30, 10);
    p6.tab_id = TabId(1);
    p6.is_floating = true;
    (
        vec![
            pane(1, "a", 0, 0, 30, 10),
            pane(2, "b", 0, 100, 30, 30),
            p3,
            p4,
            p5,
            p6,
        ],
        vec![tab(0, "Tab #1", true), tab(1, "build", false)],
    )
}

// [covers:resize.equalize-tab-spec-selects-target-tab-panes]
#[test]
fn equalize_tab_spec_selects_target_tab_panes() {
    // --tab指定はPANESPEC群指定と異なり対象tabのtiled pane（is_remap_source）を
    // 黙選択する: 他tab pane・同tabのexited pane・floating paneは対象外でerrorに
    // しない（algorithmの均等化検証はPANESPEC系既存条件と共通のため、ここでは対象
    // 選択の系列〔tab解決・選択・呼出列〕に絞る）
    let resize_calls = |b: &FakeBackend| -> Vec<String> {
        b.calls()
            .iter()
            .filter(|c| c.starts_with("resize "))
            .cloned()
            .collect()
    };
    let cols = |b: &FakeBackend, id: u32| {
        b.state
            .borrow()
            .panes
            .iter()
            .find(|p| p.id == PaneKindId::Terminal(id))
            .map(|p| p.geometry.cols)
            .unwrap()
    };

    // TABSPECの両形式（一意名 "build"・ID "1"）ともTabId(1)へ解決し同一の対象選択・
    // 呼出列・幾何になる（assertは両caseで共通）
    for spec in ["build", "1"] {
        let (panes, tabs) = mixed_tab_fixture();
        let b = FakeBackend::new(panes, tabs);
        zelper::app::resize::run_equalize(&b, &[], Some(spec), false)
            .unwrap_or_else(|e| panic!("TABSPEC {spec:?}: {e}"));
        // 対象2 paneは均等化（各1 stepでgoal到達）
        assert_eq!((cols(&b, 3), cols(&b, 4)), (20, 20), "TABSPEC {spec:?}");
        // 非対象pane（他tab terminal_1/2・exited terminal_5・floating terminal_6）は不変
        assert_eq!(
            (cols(&b, 1), cols(&b, 2), cols(&b, 5), cols(&b, 6)),
            (10, 30, 10, 10),
            "TABSPEC {spec:?}"
        );
        assert_eq!(
            resize_calls(&b),
            vec![
                "resize terminal_3 increase right",
                "resize terminal_4 decrease right",
            ],
            "TABSPEC {spec:?}: 対象tabの2 paneのみへ呼出（他tab・exited・floating paneへの呼出なし）"
        );
    }
}
