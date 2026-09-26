// L2: move系verb（fake backend。move.toml 22条件。TASK-88・設計§5.2）
// native系（move-pane argv・edge no-op）・companion系（--to-tab / --swap-withの
// 実行sequence・順序規則・部分失敗）・tab交換（隣接transposition合成）のrun層条件。
// 出力（JSON envelope・human 1行サマリ）はL3（cli系条件書）管轄。polling timeout系
// （to-tab-poll-timeout・swap-partial-failure）は10s deadlineを実時間で消費する
// （fake_remap r36と同一方式）
mod fake;
use fake::FakeBackend;
use std::path::Path;
use std::sync::Mutex;
use zelper::app::move_::{
    PaneMoveAction, run_pane, run_pane_native, run_pane_swap, run_pane_to_tab, run_tab_swap,
};
use zelper::domain::*;
use zelper::error::ErrorClass;
use zelper::zellij::ResizeDirection;

// ---- XDG_CACHE_HOME隔離（companion setupのwasm extract・permissions seedを
// test実envから分離。fake_remap.rsと同一構成） ----

static XDG_LOCK: Mutex<()> = Mutex::new(());

fn with_isolated_xdg<T>(f: impl FnOnce(&Path) -> T) -> T {
    let _guard = XDG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = std::env::temp_dir().join(format!("zelper-move-xdg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let prev = std::env::var("XDG_CACHE_HOME").ok();
    // safety: XDG_LOCKで直列化した範囲内でのみ一時的に上書きし、即座に復元する
    unsafe { std::env::set_var("XDG_CACHE_HOME", &dir) };
    let out = f(&dir);
    match prev {
        Some(p) => unsafe { std::env::set_var("XDG_CACHE_HOME", p) },
        None => unsafe { std::env::remove_var("XDG_CACHE_HOME") },
    }
    out
}

// ---- fixture群 ----

/// tiled なselectable terminal pane。geometryは (y, x)
fn pane_at(id: u32, title: &str, tab: u32, tab_name: &str, y: u32, x: u32) -> PaneState {
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
        command: None,
        terminal_command: None,
        cwd: None,
        tab_id: TabId(tab),
        tab_position: tab,
        tab_name: tab_name.to_string(),
        plugin_url: None,
    }
}

fn tab_at(id: u32, name: &str, active: bool) -> TabState {
    TabState {
        id: TabId(id),
        position: id,
        name: name.to_string(),
        active,
        selectable_tiled_panes_count: 1,
        selectable_floating_panes_count: 0,
        are_floating_panes_visible: true,
    }
}

/// M1/M8実測の3-slot layout（mixed orientation: 左1 pane〔左半分全体〕+ 右上/右下。
/// REVIEW-3指摘1: 実配置そのものを基準値にする）。幾何は実測値（100x30画面）:
/// A=terminal_0 (y=0,x=0, 29x50) focus・B=terminal_1 (y=0,x=50, 15x50)・
/// C=terminal_2 (y=15,x=50, 14x50)
fn m1_layout_panes() -> Vec<PaneState> {
    vec![
        m1_pane(0, "a", 0, 0, 29, 50, true),
        m1_pane(1, "b", 0, 50, 15, 50, false),
        m1_pane(2, "c", 15, 50, 14, 50, false),
    ]
}

fn m1_pane(id: u32, title: &str, y: u32, x: u32, rows: u32, cols: u32, focused: bool) -> PaneState {
    let mut p = pane_at(id, title, 0, "T0", y, x);
    p.geometry.rows = rows;
    p.geometry.cols = cols;
    p.is_focused = focused;
    p
}

/// 全paneの状態snapshot（id/title/focus/geometry。REVIEW-3指摘1: 対象paneだけで
/// なく全paneの幾何・ID・title・focusを照合する）
fn pane_snapshots(b: &FakeBackend) -> Vec<String> {
    snapshot_of(&b.state.borrow().panes)
}

fn snapshot_of(panes: &[PaneState]) -> Vec<String> {
    panes
        .iter()
        .map(|p| {
            format!(
                "{}|{}|focus={}|y={},x={},{}x{}",
                p.id.as_spec(),
                p.title,
                p.is_focused,
                p.geometry.y,
                p.geometry.x,
                p.geometry.rows,
                p.geometry.cols
            )
        })
        .collect()
}

/// M1実配置の期待snapshot構築helper: (id, title, y, x, rows, cols, focused) 列から
fn expect_snapshots(entries: &[(u32, &str, u32, u32, u32, u32, bool)]) -> Vec<String> {
    entries
        .iter()
        .map(|(id, title, y, x, rows, cols, f)| {
            format!("terminal_{id}|{title}|focus={f}|y={y},x={x},{rows}x{cols}")
        })
        .collect()
}

/// 2 tab構成（tab0にpane A=terminal_1とX=terminal_2・tab1にpane B=terminal_3と
/// Y=terminal_4。両tab非singleton）
fn two_tabs_two_panes_each() -> (Vec<PaneState>, Vec<TabState>) {
    (
        vec![
            pane_at(1, "A", 0, "T0", 0, 0),
            pane_at(2, "X", 0, "T0", 1, 0),
            pane_at(3, "B", 1, "T1", 0, 0),
            pane_at(4, "Y", 1, "T1", 1, 0),
        ],
        vec![tab_at(0, "T0", true), tab_at(1, "T1", false)],
    )
}

fn pipes(b: &FakeBackend) -> Vec<String> {
    b.calls()
        .iter()
        .filter(|c| c.starts_with("pipe "))
        .cloned()
        .collect()
}

fn move_tab_calls(b: &FakeBackend) -> Vec<String> {
    b.calls()
        .iter()
        .filter(|c| c.starts_with("move-tab "))
        .cloned()
        .collect()
}

fn tab_of(b: &FakeBackend, id: u32) -> TabId {
    b.state
        .borrow()
        .panes
        .iter()
        .find(|p| p.id == PaneKindId::Terminal(id))
        .map(|p| p.tab_id)
        .expect("pane exists")
}

// ---------------------------------------------------------------------------
// native系（方向移動・rotate）
// ---------------------------------------------------------------------------

/// 方向指定（4方位）はMovePaneOp::Directionとしてmove_paneへ渡り、argvは
/// `move-pane <dir> -p <spec>`に組み立てられる。M1実測の実配置（左1 pane +
/// 右上/右下）で全paneの幾何・ID・title・focusを照合する（REVIEW-3指摘1:
/// 対象paneだけでなく全paneを比較し、Left/Up・Right/Downを同一slot扱いしない
/// fakeになっていることを検出できる形）。M1実測との対応: C left→A（左の大pane）・
/// A right→C（右側の下段）・C up→B・B down→C
// [covers:move.pane-direction-argv]
#[test]
fn pane_direction_argv_and_full_state_swap() {
    let tabs = vec![tab_at(0, "T0", true)];
    // CASE L（M1実測）: terminal_2 left → A(terminal_0)と交換
    let b = FakeBackend::new(m1_layout_panes(), tabs.clone());
    let out = run_pane_native(
        &b,
        "2",
        PaneMoveAction::Direction(ResizeDirection::Left),
        false,
    )
    .unwrap();
    assert!(!out.noop, "swapが起きた系列でno-op報告はしない");
    assert_eq!(
        b.calls()
            .iter()
            .filter(|c| *c == "move-pane left -p terminal_2")
            .count(),
        1,
        "argv: move-pane left -p terminal_2（ちょうど1回）: {:?}",
        b.calls()
    );
    assert_eq!(
        pane_snapshots(&b),
        expect_snapshots(&[
            (0, "a", 15, 50, 14, 50, false),
            (1, "b", 0, 50, 15, 50, false),
            (2, "c", 0, 0, 29, 50, true),
        ]),
        "CASE L: A(0)とC(2)が交換（slot幾何不変・id/title不変・focusは移動pane 2へ）"
    );

    // CASE R（M1実測）: terminal_0 right → C(terminal_2)と交換（右上Bでない）
    let b = FakeBackend::new(m1_layout_panes(), tabs.clone());
    run_pane_native(
        &b,
        "0",
        PaneMoveAction::Direction(ResizeDirection::Right),
        false,
    )
    .unwrap();
    assert_eq!(
        pane_snapshots(&b),
        expect_snapshots(&[
            (0, "a", 15, 50, 14, 50, true),
            (1, "b", 0, 50, 15, 50, false),
            (2, "c", 0, 0, 29, 50, false),
        ]),
        "CASE R: A(0)とC(2)が交換（M1実測: rightの相手は右側の下段C）"
    );

    // CASE U（M1実測）: terminal_2 up → B(terminal_1)と交換・focus pane(0)不参加のためfocus不変
    let b = FakeBackend::new(m1_layout_panes(), tabs.clone());
    run_pane_native(
        &b,
        "2",
        PaneMoveAction::Direction(ResizeDirection::Up),
        false,
    )
    .unwrap();
    assert_eq!(
        pane_snapshots(&b),
        expect_snapshots(&[
            (0, "a", 0, 0, 29, 50, true),
            (1, "b", 15, 50, 14, 50, false),
            (2, "c", 0, 50, 15, 50, false),
        ]),
        "CASE U: B(1)とC(2)が同一列で交換（M1実測）・focusはAのまま"
    );

    // CASE D（M1実測）: terminal_1 down → C(terminal_2)と交換
    let b = FakeBackend::new(m1_layout_panes(), tabs.clone());
    run_pane_native(
        &b,
        "1",
        PaneMoveAction::Direction(ResizeDirection::Down),
        false,
    )
    .unwrap();
    assert_eq!(
        pane_snapshots(&b),
        expect_snapshots(&[
            (0, "a", 0, 0, 29, 50, true),
            (1, "b", 15, 50, 14, 50, false),
            (2, "c", 0, 50, 15, 50, false),
        ]),
        "CASE D: B(1)とC(2)が交換（M1実測）・focusはAのまま"
    );

    // 幾何隣接則由来（未実測方向）: terminal_1 left → 左でy重複するAと交換
    let b = FakeBackend::new(m1_layout_panes(), tabs.clone());
    run_pane_native(
        &b,
        "1",
        PaneMoveAction::Direction(ResizeDirection::Left),
        false,
    )
    .unwrap();
    assert_eq!(
        pane_snapshots(&b),
        expect_snapshots(&[
            (0, "a", 0, 50, 15, 50, false),
            (1, "b", 0, 0, 29, 50, true),
            (2, "c", 15, 50, 14, 50, false),
        ]),
        "1 left: A(0)と交換（相手がfocusならfocusは移動pane 1へ）"
    );

    // 4方位のargv形式（up/downも同一形式）
    for (dir, want) in [
        (ResizeDirection::Left, "move-pane left -p terminal_2"),
        (ResizeDirection::Right, "move-pane right -p terminal_2"),
        (ResizeDirection::Up, "move-pane up -p terminal_2"),
        (ResizeDirection::Down, "move-pane down -p terminal_2"),
    ] {
        let b = FakeBackend::new(m1_layout_panes(), tabs.clone());
        run_pane_native(&b, "2", PaneMoveAction::Direction(dir), false).unwrap();
        assert!(
            b.calls().iter().any(|c| c == want),
            "{want}: {:?}",
            b.calls()
        );
    }
}

/// rotate（forwards）: argvはbareの`move-pane -p <spec>`（方向引数なし）。
/// m8実測基準: 対象paneと視覚順序上の「次」の1つとのみ交換（全pane循環でない。
/// m8 move3: 対象0〔順序1番目〕→0↔1交換で2不変・move2: 末尾対象→先頭とwrap交換）
// [covers:move.rotate-argv]
#[test]
fn rotate_forwards_argv() {
    let tabs = vec![tab_at(0, "T0", true)];
    // 対象0（視覚順序1番目）→ 0↔1交換・2不変（m8 move3基準）
    let b = FakeBackend::new(m1_layout_panes(), tabs.clone());
    let out = run_pane_native(&b, "0", PaneMoveAction::Rotate, false).unwrap();
    assert!(!out.noop);
    assert_eq!(
        b.calls()
            .iter()
            .filter(|c| *c == "move-pane -p terminal_0")
            .count(),
        1,
        "argv: move-pane -p terminal_0: {:?}",
        b.calls()
    );
    assert_eq!(
        pane_snapshots(&b),
        expect_snapshots(&[
            (0, "a", 0, 50, 15, 50, true),
            (1, "b", 0, 0, 29, 50, false),
            (2, "c", 15, 50, 14, 50, false),
        ]),
        "0↔1交換・2不変（全pane循環shiftでない）・focusは対象0に追従"
    );

    // 対象2（視覚順序の末尾）→ wrapして先頭の0と交換（m8 move2基準）
    let b = FakeBackend::new(m1_layout_panes(), tabs);
    let out = run_pane_native(&b, "2", PaneMoveAction::Rotate, false).unwrap();
    assert!(!out.noop);
    assert_eq!(
        pane_snapshots(&b),
        expect_snapshots(&[
            (0, "a", 15, 50, 14, 50, false),
            (1, "b", 0, 50, 15, 50, false),
            (2, "c", 0, 0, 29, 50, true),
        ]),
        "末尾の対象は先頭(0)とwrap交換・1不変"
    );
}

/// rotate --backwards: argvは別action名`move-pane-backwards -p <spec>`。
/// 対象と視覚順序上の「前」の1つとのみ交換（先頭は末尾とwrap。m8 move4の対称）
// [covers:move.rotate-backwards-argv]
#[test]
fn rotate_backwards_argv() {
    let tabs = vec![tab_at(0, "T0", true)];
    // 対象2（視覚順序3番目）→ 前の1と交換・0不変
    let b = FakeBackend::new(m1_layout_panes(), tabs.clone());
    let out = run_pane_native(&b, "2", PaneMoveAction::Rotate, true).unwrap();
    assert!(!out.noop);
    assert_eq!(
        b.calls()
            .iter()
            .filter(|c| *c == "move-pane-backwards -p terminal_2")
            .count(),
        1,
        "argv: move-pane-backwards -p terminal_2: {:?}",
        b.calls()
    );
    assert_eq!(
        pane_snapshots(&b),
        expect_snapshots(&[
            (0, "a", 0, 0, 29, 50, true),
            (1, "b", 15, 50, 14, 50, false),
            (2, "c", 0, 50, 15, 50, false),
        ]),
        "1↔2交換・0不変"
    );

    // 対象0（先頭）→ wrapして末尾の2と交換
    let b = FakeBackend::new(m1_layout_panes(), tabs);
    run_pane_native(&b, "0", PaneMoveAction::Rotate, true).unwrap();
    assert_eq!(
        pane_snapshots(&b),
        expect_snapshots(&[
            (0, "a", 15, 50, 14, 50, true),
            (1, "b", 0, 50, 15, 50, false),
            (2, "c", 0, 0, 29, 50, false),
        ]),
        "先頭の対象は末尾(2)とwrap交換・1不変"
    );
}

/// edge方向移動のno-op検出: 対象paneが移動方向の端にいる系列ではmove_paneはrc=0
/// で完了するが幾何が不変のため、list-panes再取得の幾何比較でno-op warning報告
/// （exit 0相当。warningなしの成功報告でない）。M1実測のedge 4系列（EL/EU/ER/ED）
/// ＋幾何隣接則由来の端（A up/down・B right・C down）で全pane状態完全不変を照合
// [covers:move.pane-edge-noop-warning]
#[test]
fn edge_direction_is_noop_with_warning_report() {
    let tabs = vec![tab_at(0, "T0", true)];
    let initial = snapshot_of(&m1_layout_panes());
    // (target, direction): M1実測 EL/EU/ER/ED + 幾何則由来の端
    for (target, dir) in [
        ("0", ResizeDirection::Left),  // EL実測: A左端
        ("1", ResizeDirection::Up),    // EU実測: B上端
        ("2", ResizeDirection::Right), // ER実測: C右端
        ("2", ResizeDirection::Down),  // ED実測: C下端
        ("0", ResizeDirection::Up),    // 幾何則: Aの上にx重複なし
        ("0", ResizeDirection::Down),  // 幾何則: Aの下にx重複なし（tiled terminal内）
        ("1", ResizeDirection::Right), // 幾何則: Bの右にy重複なし
    ] {
        let b = FakeBackend::new(m1_layout_panes(), tabs.clone());
        let out = run_pane_native(&b, target, PaneMoveAction::Direction(dir), false).unwrap();
        assert!(out.noop, "target {target} {dir:?}: 幾何不変 = no-op報告");
        assert_eq!(
            pane_snapshots(&b),
            initial,
            "target {target} {dir:?}: edgeは状態完全不変（M1: rc=0・出力空）"
        );
    }
    // rotate単独pane tabもno-op（交換相手がいない）
    let b1 = FakeBackend::new(
        vec![pane_at(5, "solo", 0, "T0", 0, 0)],
        vec![tab_at(0, "T0", true)],
    );
    let out1 = run_pane_native(&b1, "5", PaneMoveAction::Rotate, false).unwrap();
    assert!(out1.noop, "単独pane tabのrotateはno-op");
}

/// 三者排他（action positional / --to-tab / --swap-with はちょうど1つ）と
/// --backwards×rotate制約。違反はUsageで、move_pane・companion系呼出は不発
// [covers:move.exclusive-usage]
#[test]
fn exclusive_usage_errors() {
    let (panes, tabs) = two_tabs_two_panes_each();
    let mk = || FakeBackend::new(panes.clone(), tabs.clone());
    // (1) すべて不在
    let b = mk();
    let err = run_pane(&b, "1", None, false, None, None, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Usage);
    // (2) left + --to-tab同時
    let b = mk();
    let err = run_pane(
        &b,
        "1",
        Some(PaneMoveAction::Direction(ResizeDirection::Left)),
        false,
        Some("1"),
        None,
        false,
    )
    .unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Usage);
    // (3) --swap-with + --to-tab同時
    let b = mk();
    let err = run_pane(&b, "1", None, false, Some("1"), Some("3"), false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Usage);
    // (4) --backwards + action=left（rotate以外との併用）
    let b = mk();
    let err = run_pane(
        &b,
        "1",
        Some(PaneMoveAction::Direction(ResizeDirection::Left)),
        true,
        None,
        None,
        false,
    )
    .unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Usage);
    assert!(
        err.message().contains("backwards"),
        "--backwards起因の文面: {}",
        err.message()
    );
    // いずれの系列もbackendの状態変更系呼出は不発（list-panesすら呼ばない）
    for b in [&mk(), &mk()] {
        assert!(
            !b.calls().iter().any(|c| c.starts_with("move-pane")
                || c.starts_with("pipe ")
                || c.starts_with("move-tab ")),
            "排他違反は検証前に中断: {:?}",
            b.calls()
        );
    }
}

/// swap系の同一対象指定: pane自分自身・tab同一値はいずれもUsage（状態変更前に中断）
// [covers:move.self-target-swap-usage-error]
#[test]
fn self_target_swap_is_usage_error() {
    let (panes, tabs) = two_tabs_two_panes_each();
    let b = FakeBackend::new(panes.clone(), tabs.clone());
    let err = run_pane_swap(&b, "1", "1").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Usage);
    let err = run_pane_swap(&b, "1", "terminal_1").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Usage, "別表記の同一idも拒否");
    assert!(
        !b.calls().iter().any(|c| c.starts_with("pipe ")),
        "pipe不発: {:?}",
        b.calls()
    );

    let err = run_tab_swap(&b, "1", "1").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Usage);
    // 解決前の文字列段で弾くためbackend呼出（list-tabs）も不発
    assert!(
        !b.calls().iter().any(|c| c.starts_with("list-tabs")),
        "tab同一指定は文字列段で中断: {:?}",
        b.calls()
    );
}

/// 対象解決のerror分類: pane不在はNoTarget・tab不在はNoTarget・同名曖昧は
/// AmbiguousTarget（--to-tab宛先・tab swap対象の両方）
// [covers:move.target-resolution-not-found-and-ambiguous]
#[test]
fn target_resolution_not_found_and_ambiguous() {
    let (panes, tabs) = two_tabs_two_panes_each();
    // pane不在（native / to-tab / swap）
    let b = FakeBackend::new(panes.clone(), tabs.clone());
    let err = run_pane_native(
        &b,
        "99",
        PaneMoveAction::Direction(ResizeDirection::Left),
        false,
    )
    .unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);
    let err = run_pane_to_tab(&b, "99", "1").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);
    let err = run_pane_swap(&b, "1", "99").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);

    // --to-tab宛先: 不在 / 同名2件
    let err = run_pane_to_tab(&b, "1", "42").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);
    let dup = vec![tab_at(0, "dup", true), tab_at(1, "dup", false)];
    let b = FakeBackend::new(panes.clone(), dup);
    let err = run_pane_to_tab(&b, "1", "dup").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::AmbiguousTarget);

    // tab swap: 不在 / 同名2件
    let b = FakeBackend::new(panes.clone(), tabs.clone());
    let err = run_tab_swap(&b, "42", "0").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);
    let dup = vec![tab_at(0, "dup", true), tab_at(1, "dup", false)];
    let b = FakeBackend::new(panes, dup);
    let err = run_tab_swap(&b, "dup", "0").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::AmbiguousTarget);
}

/// floating paneの遮断（native・companion双方）: Preflight exit 7。native系の
/// messageにはremap --embed-floating併用のhintを含む。move_pane・pipe・companion
/// setupはすべて不発
// [covers:move.floating-pane-preflight]
#[test]
fn floating_pane_is_preflight_error_on_all_paths() {
    let mk = || {
        let mut panes = two_tabs_two_panes_each().0;
        panes.push({
            let mut p = pane_at(5, "float", 0, "T0", 0, 50);
            p.is_floating = true;
            p
        });
        (panes, two_tabs_two_panes_each().1)
    };
    // native系: direction と rotate
    let (panes, tabs) = mk();
    let b = FakeBackend::new(panes.clone(), tabs.clone());
    let err = run_pane_native(
        &b,
        "5",
        PaneMoveAction::Direction(ResizeDirection::Left),
        false,
    )
    .unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    assert!(
        err.message().contains("embed-floating"),
        "native系のhint: {}",
        err.message()
    );
    let err = run_pane_native(&b, "5", PaneMoveAction::Rotate, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    // companion系: --to-tab と --swap-with
    let err = run_pane_to_tab(&b, "5", "1").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    let err = run_pane_swap(&b, "5", "3").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    // swapの相手側がfloatingでも同様
    let err = run_pane_swap(&b, "3", "5").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    // 状態変更系呼出はすべて不発（companion setupも不発: permissions未書込）
    with_isolated_xdg(|xdg| {
        let (panes, tabs) = mk();
        let b = FakeBackend::new(panes, tabs);
        let _ = run_pane_native(
            &b,
            "5",
            PaneMoveAction::Direction(ResizeDirection::Left),
            false,
        );
        let _ = run_pane_to_tab(&b, "5", "1");
        let _ = run_pane_swap(&b, "5", "3");
        assert!(!xdg.join("zellij").join("permissions.kdl").exists());
    });
    assert!(
        !b.calls()
            .iter()
            .any(|c| c.starts_with("move-pane") || c.starts_with("pipe ")),
        "{:?}",
        b.calls()
    );
}

/// companion系のplugin pane対象外検査: --to-tab / --swap-withの対象はterminal
/// paneに限り、plugin pane指定は状態変更前にPreflight
// [covers:move.plugin-pane-preflight]
#[test]
fn plugin_pane_is_preflight_error_on_companion_paths() {
    let (mut panes, tabs) = two_tabs_two_panes_each();
    panes.push(PaneState {
        id: PaneKindId::Plugin(0),
        title: "plugin".into(),
        is_selectable: false,
        is_floating: true,
        is_focused: false,
        exited: false,
        is_held: false,
        geometry: Geometry {
            x: 0,
            y: 0,
            rows: 5,
            cols: 5,
        },
        command: None,
        terminal_command: None,
        cwd: None,
        tab_id: TabId(0),
        tab_position: 0,
        tab_name: "T0".into(),
        plugin_url: Some("file:zelper-companion.wasm".into()),
    });
    let b = FakeBackend::new(panes, tabs);
    let err = run_pane_to_tab(&b, "plugin_0", "1").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    let err = run_pane_swap(&b, "plugin_0", "3").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    let err = run_pane_swap(&b, "3", "plugin_0").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    assert!(pipes(&b).is_empty(), "pipe不発: {:?}", b.calls());
}

// ---------------------------------------------------------------------------
// companion系（--to-tab / --swap-with）
// ---------------------------------------------------------------------------

/// --to-tabのcompanion実行一式: companion setup（permissions seed）→ pipe probe →
/// pipe break-id <宛先tab id> <単一pane id>の順で現れ、polling完了後のpaneは
/// 宛先tab所属・pane id不変
// [covers:move.to-tab-companion-sequence]
#[test]
fn to_tab_companion_sequence() {
    let (panes, tabs) = two_tabs_two_panes_each();
    let b = FakeBackend::new(panes, tabs);
    let seeded = with_isolated_xdg(|xdg| {
        let out = run_pane_to_tab(&b, "1", "T1").unwrap();
        assert!(!out.noop);
        assert_eq!(out.tab, 1);
        xdg.join("zellij").join("permissions.kdl").exists()
    });
    assert!(seeded, "companion setup（permissions seed）が先行");
    let calls = b.calls();
    let probe = calls.iter().position(|c| c.starts_with("pipe probe "));
    let brk = calls.iter().position(|c| c.starts_with("pipe break-id "));
    let (probe, brk) = (probe.expect("probe pipe"), brk.expect("break-id pipe"));
    assert!(probe < brk, "probe確認がbreak-idより先: {calls:?}");
    assert_eq!(
        calls[brk], "pipe break-id 1 terminal_1",
        "payload = 宛先tab idと単一pane id: {calls:?}"
    );
    assert_eq!(tab_of(&b, 1), TabId(1), "pane Aはtab 1所属");
    // pane id不変（M8基準）
    assert!(
        b.state
            .borrow()
            .panes
            .iter()
            .any(|p| p.id == PaneKindId::Terminal(1))
    );
}

/// 宛先==現tabのno-op: companion経路を一切使わずwarning付きexit 0で完了
// [covers:move.to-tab-noop-same-tab]
#[test]
fn to_tab_same_tab_is_noop() {
    let (panes, tabs) = two_tabs_two_panes_each();
    let b = FakeBackend::new(panes, tabs);
    let seeded = with_isolated_xdg(|xdg| {
        let out = run_pane_to_tab(&b, "1", "T0").unwrap();
        assert!(out.noop, "no-op報告");
        assert_eq!(out.tab, 0);
        xdg.join("zellij").join("permissions.kdl").exists()
    });
    assert!(!seeded, "companion setup不発");
    assert!(pipes(&b).is_empty(), "pipe不発: {:?}", b.calls());
    assert_eq!(tab_of(&b, 1), TabId(0), "pane Aはtab 0に留まる");
}

/// probe不成立の安全側中断: probe pipeの送信は成功するがcompanion pluginのtitle
/// 出現が起こらない系列（probe効果だけ抑制）では、polling timeout後に一切の状態
/// 変更（break-id pipe等）なくOperationFailed + 権限hintで中断する（条件書given
/// 「probe pipe後のtitle出現が起こらない失敗注入」との整合。REVIEW-3指摘5）
// [covers:move.probe-failure]
#[test]
fn probe_failure_aborts_before_state_change() {
    let (panes, tabs) = two_tabs_two_panes_each();
    let b = FakeBackend::new(panes, tabs);
    b.mute_pipe("probe"); // pipe送信は成功するがtitleが出現しない（効果だけ抑制）
    let err = with_isolated_xdg(|_| run_pane_to_tab(&b, "1", "T1").unwrap_err());
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
    assert!(
        err.message().to_lowercase().contains("permission"),
        "権限hint: {}",
        err.message()
    );
    assert!(
        err.message().contains("probe not observed"),
        "probe polling timeoutであること: {}",
        err.message()
    );
    let calls = b.calls();
    assert!(
        calls.iter().any(|c| c.starts_with("pipe probe")),
        "probeは試みる: {calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.starts_with("pipe break-id")),
        "break-id pipe不発: {calls:?}"
    );
    assert_eq!(tab_of(&b, 1), TabId(0), "pane Aは移動しない");
}

/// probe pipe送信自体がbackend失敗する系列（pipe CLIの非zero終了・block模擬）でも
/// 状態変更前にOperationFailed + 権限hintで中断する（move.probe-failureの送信失敗
/// 側。REVIEW-3指摘5: 効果抑制と送信失敗を別条件として分離）
// [covers:move.probe-pipe-send-failure]
#[test]
fn probe_pipe_send_failure_aborts_before_state_change() {
    let (panes, tabs) = two_tabs_two_panes_each();
    let b = FakeBackend::new(panes, tabs);
    b.inject_failure("pipe"); // 最初のpipe（= probe）の送信が失敗する
    let err = with_isolated_xdg(|_| run_pane_to_tab(&b, "1", "T1").unwrap_err());
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
    assert!(
        err.message().to_lowercase().contains("permission"),
        "権限hint: {}",
        err.message()
    );
    assert!(
        err.message().contains("probe pipe did not complete"),
        "probe送信失敗であること: {}",
        err.message()
    );
    assert!(
        !b.calls().iter().any(|c| c.starts_with("pipe break-id")),
        "break-id pipe不発: {:?}",
        b.calls()
    );
    assert_eq!(tab_of(&b, 1), TabId(0), "pane Aは移動しない");
}

/// 移動は実際に完了しているがlenient観測だけ失敗する系列（REVIEW-3指摘2）:
/// polling timeout後の再取得で移動済みが観測された場合は「実際には完了していたが
/// deadline内に確認できなかった」と報告する（未移動と断定しない。pollのC3規則に
/// よりdeadline超過後の成立は成功扱いにしないためerrorのまま）
// [covers:move.to-tab-poll-timeout]
#[test]
fn to_tab_poll_timeout_reports_accurately() {
    let (panes, tabs) = two_tabs_two_panes_each();
    let b = FakeBackend::new(panes, tabs);
    b.mute_pipe("break-id"); // pipeは成功するが効果が現れない
    let err = with_isolated_xdg(|_| run_pane_to_tab(&b, "1", "T1").unwrap_err());
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
    let msg = err.message().to_lowercase();
    assert!(msg.contains("permission"), "権限hint: {}", err.message());
    assert!(
        msg.contains("delay") || msg.contains("late"),
        "遅延実行の注意: {}",
        err.message()
    );
    assert!(
        msg.contains("the pane was not moved"),
        "未移動の正確な報告: {}",
        err.message()
    );
    assert_eq!(tab_of(&b, 1), TabId(0), "pane Aはtab 0に留まる");

    // 移動自体は反映・lenient観測だけ失敗する系列: timeout後の再取得（strict）で
    // 移動済みが見えるため「実際には完了した」と報告する
    let (panes, tabs) = two_tabs_two_panes_each();
    let b = FakeBackend::new(panes, tabs);
    b.hide_pane_in_lenient_after_break(PaneKindId::Terminal(1));
    let err = with_isolated_xdg(|_| run_pane_to_tab(&b, "1", "T1").unwrap_err());
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
    let msg = err.message();
    assert!(
        msg.contains("actually completed"),
        "移動済み観測の報告: {msg}"
    );
    assert!(
        msg.contains("not observed within the polling deadline"),
        "deadline内に観測できなかった旨: {msg}"
    );
    // 実状態: pane Aは移動済み（移動自体は成功している）
    assert_eq!(tab_of(&b, 1), TabId(1), "pane Aは実際にはtab 1へ移動済み");
}

/// companion setup失敗の事前中断: wasm extract（cache dir作成失敗）はprobe・
/// break-id pipe等の状態変更前にPreflightで中断する
// [covers:move.companion-setup-failure]
#[test]
fn companion_setup_failure_is_preflight_error() {
    let (panes, tabs) = two_tabs_two_panes_each();
    let b = FakeBackend::new(panes, tabs);
    // XDG_CACHE_HOMEに通常fileを指向させ、companion cache dirの作成を失敗させる
    let guard = XDG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let file = std::env::temp_dir().join(format!("zelper-move-notadir-{}", std::process::id()));
    std::fs::write(&file, "x").unwrap();
    let prev = std::env::var("XDG_CACHE_HOME").ok();
    // safety: XDG_LOCKで直列化した範囲内でのみ一時的に上書きし、即座に復元する
    unsafe { std::env::set_var("XDG_CACHE_HOME", &file) };
    let err = run_pane_to_tab(&b, "1", "T1");
    match prev {
        Some(p) => unsafe { std::env::set_var("XDG_CACHE_HOME", p) },
        None => unsafe { std::env::remove_var("XDG_CACHE_HOME") },
    }
    drop(guard);
    let err = err.unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    assert!(pipes(&b).is_empty(), "probe・break-id不発: {:?}", b.calls());
    assert_eq!(tab_of(&b, 1), TabId(0));
}

/// pane交換2連発の順序規則: singleton判定（当該tabのselectable pane数==1基準）で
/// 発行順序を決める。両非singletonはA→T_Bが先・AのみsingletonはB→T_Aが先・
/// BのみsingletonはA→T_Bが先。両step完了後はA∈T_B・B∈T_A（pane id不変）
// [covers:move.swap-order-singleton-rule]
#[test]
fn swap_order_follows_singleton_rule() {
    // (1) 両tab 2 pane以上（両非singleton）: A→T_Bが先（決定論的tie-break）
    {
        let (panes, tabs) = two_tabs_two_panes_each();
        let b = FakeBackend::new(panes, tabs);
        with_isolated_xdg(|_| run_pane_swap(&b, "1", "3").unwrap());
        let brks: Vec<String> = pipes(&b)
            .into_iter()
            .filter(|c| c.starts_with("pipe break-id "))
            .collect();
        assert_eq!(
            brks,
            vec![
                "pipe break-id 1 terminal_1".to_string(),
                "pipe break-id 0 terminal_3".to_string()
            ],
            "両非singletonはA先: {:?}",
            b.calls()
        );
        assert_eq!(tab_of(&b, 1), TabId(1));
        assert_eq!(tab_of(&b, 3), TabId(0));
    }
    // (2) T_AがAのみ（Aのみsingleton）: B→T_Aが先
    {
        let panes = vec![
            pane_at(1, "A", 0, "T0", 0, 0),
            pane_at(3, "B", 1, "T1", 0, 0),
            pane_at(4, "Y", 1, "T1", 1, 0),
        ];
        let tabs = vec![tab_at(0, "T0", true), tab_at(1, "T1", false)];
        let b = FakeBackend::new(panes, tabs);
        with_isolated_xdg(|_| run_pane_swap(&b, "1", "3").unwrap());
        let brks: Vec<String> = pipes(&b)
            .into_iter()
            .filter(|c| c.starts_with("pipe break-id "))
            .collect();
        assert_eq!(
            brks,
            vec![
                "pipe break-id 0 terminal_3".to_string(),
                "pipe break-id 1 terminal_1".to_string()
            ],
            "AのみsingletonはB先: {:?}",
            b.calls()
        );
        assert_eq!(tab_of(&b, 1), TabId(1));
        assert_eq!(tab_of(&b, 3), TabId(0));
    }
    // (3) T_BがBのみ（Bのみsingleton）: A→T_Bが先
    {
        let panes = vec![
            pane_at(1, "A", 0, "T0", 0, 0),
            pane_at(2, "X", 0, "T0", 1, 0),
            pane_at(3, "B", 1, "T1", 0, 0),
        ];
        let tabs = vec![tab_at(0, "T0", true), tab_at(1, "T1", false)];
        let b = FakeBackend::new(panes, tabs);
        with_isolated_xdg(|_| run_pane_swap(&b, "1", "3").unwrap());
        let brks: Vec<String> = pipes(&b)
            .into_iter()
            .filter(|c| c.starts_with("pipe break-id "))
            .collect();
        assert_eq!(
            brks,
            vec![
                "pipe break-id 1 terminal_1".to_string(),
                "pipe break-id 0 terminal_3".to_string()
            ],
            "BのみsingletonはA先: {:?}",
            b.calls()
        );
        assert_eq!(tab_of(&b, 1), TabId(1));
        assert_eq!(tab_of(&b, 3), TabId(0));
    }
}

/// 同一tab内のpane交換拒否: Preflight exit 7・messageはremapへのhintを含む
// [covers:move.swap-same-tab-preflight-error]
#[test]
fn swap_same_tab_is_preflight_error() {
    let panes = vec![
        pane_at(1, "A", 0, "T0", 0, 0),
        pane_at(2, "B", 0, "T0", 0, 100),
    ];
    let b = FakeBackend::new(panes, vec![tab_at(0, "T0", true)]);
    let seeded = with_isolated_xdg(|xdg| {
        let err = run_pane_swap(&b, "1", "2").unwrap_err();
        assert_eq!(*err.class(), ErrorClass::Preflight);
        assert!(
            err.message().contains("remap"),
            "remap hint: {}",
            err.message()
        );
        xdg.join("zellij").join("permissions.kdl").exists()
    });
    assert!(!seeded, "companion setup不発");
    assert!(pipes(&b).is_empty(), "pipe不発: {:?}", b.calls());
}

/// 両singleton tabのpane交換拒否: Preflight exit 7。hintは代替手段（move tab
/// --swap-with・remap）とtab交換ではtab名・tab idの帰属が入れ替わらない旨の
/// 差分案内を含む（R7）
// [covers:move.swap-both-singleton-preflight-error]
#[test]
fn swap_both_singleton_is_preflight_error() {
    let panes = vec![
        pane_at(1, "A", 0, "T0", 0, 0),
        pane_at(3, "B", 1, "T1", 0, 0),
    ];
    let b = FakeBackend::new(panes, vec![tab_at(0, "T0", true), tab_at(1, "T1", false)]);
    let seeded = with_isolated_xdg(|xdg| {
        let err = run_pane_swap(&b, "1", "3").unwrap_err();
        assert_eq!(*err.class(), ErrorClass::Preflight);
        let msg = err.message();
        assert!(
            msg.contains("move tab --swap-with") && msg.contains("remap"),
            "代替手段のhint: {msg}"
        );
        assert!(
            msg.contains("NOT swapped") || msg.to_lowercase().contains("not swapped"),
            "tab交換との差分案内（tab名・id帰属）: {msg}"
        );
        xdg.join("zellij").join("permissions.kdl").exists()
    });
    assert!(!seeded, "companion setup不発");
    assert!(pipes(&b).is_empty(), "pipe不発: {:?}", b.calls());
}

/// pane交換2連発の途中失敗報告: 2 step目（B→T_A）が完了しない系列では、1 step目
/// の実行済み（A moved to T_B）と未実行（B remains）を正確に区別して報告し、
/// 手動完了hint（move pane B --to-tab T_A）を含む。atomicityの主張をしない
// [covers:move.swap-partial-failure-report]
#[test]
fn swap_partial_failure_reports_moved_and_unmoved() {
    let (panes, tabs) = two_tabs_two_panes_each();
    let b = FakeBackend::new(panes, tabs);
    b.mute_break_id_nth(2); // 2 step目（B→T_A）のbreak-id効果不出現
    let err = with_isolated_xdg(|_| run_pane_swap(&b, "1", "3").unwrap_err());
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
    let msg = err.message();
    assert!(
        msg.contains("terminal_1") && msg.contains("moved to tab 1"),
        "移動済み報告（A・T_B所属）: {msg}"
    );
    assert!(
        msg.contains("terminal_3 was not moved and remains on tab 1"),
        "未移動報告（B・元tab所属。REVIEW-3指摘2: 再取得による正確な報告）: {msg}"
    );
    assert!(
        msg.contains("move pane terminal_3 --to-tab 0"),
        "手動完了hint: {msg}"
    );
    assert!(
        msg.to_lowercase().contains("atomicity is not claimed"),
        "atomicity不主張: {msg}"
    );
    // 実状態: Aは移動済み・Bは元tab
    assert_eq!(tab_of(&b, 1), TabId(1));
    assert_eq!(tab_of(&b, 3), TabId(1));
}

// ---------------------------------------------------------------------------
// tab交換（native move-tab -t の隣接transposition合成）
// ---------------------------------------------------------------------------

/// tab交換の隣接transposition合成: d=2系はRight×2（A）→Left×1（B）の計3回・
/// d=1系はRight×1の1回。完了後のpositionは交換・tab id/name不変・go-to-tab呼出0回
// [covers:move.tab-swap-argv-sequence]
#[test]
fn tab_swap_argv_sequence_and_postcondition() {
    let four_tabs = || {
        (
            Vec::<PaneState>::new(),
            vec![
                tab_at(0, "w", true),
                tab_at(1, "x", false),
                tab_at(2, "y", false),
                tab_at(3, "z", false),
            ],
        )
    };
    // d=2（position 0と2）
    {
        let (panes, tabs) = four_tabs();
        let b = FakeBackend::new(panes, tabs);
        let out = run_tab_swap(&b, "0", "2").unwrap();
        assert_eq!(
            move_tab_calls(&b),
            vec![
                "move-tab right -t 0".to_string(),
                "move-tab right -t 0".to_string(),
                "move-tab left -t 2".to_string(),
            ]
        );
        assert_eq!((out.a, out.a_position, out.b, out.b_position), (0, 2, 2, 0));
        let s = b.state.borrow();
        let by_pos: Vec<(u32, u32, &str)> = s
            .tabs
            .iter()
            .map(|t| (t.position, t.id.0, t.name.as_str()))
            .collect();
        // position交換（id 0はposition 2へ・id 2はposition 0へ）・他tab不変・name不変
        assert_eq!(
            by_pos,
            vec![(0, 2, "y"), (1, 1, "x"), (2, 0, "w"), (3, 3, "z")],
            "position交換・id/name不変: {by_pos:?}"
        );
        assert!(
            !b.calls().iter().any(|c| c.starts_with("go-to-tab")),
            "-tはfocus不変のためgo-to併用不要: {:?}",
            b.calls()
        );
    }
    // d=1（隣接・position 0と1）: right×1の1回のみ
    {
        let (panes, tabs) = four_tabs();
        let b = FakeBackend::new(panes, tabs);
        let out = run_tab_swap(&b, "0", "1").unwrap();
        assert_eq!(move_tab_calls(&b), vec!["move-tab right -t 0".to_string()]);
        assert_eq!((out.a, out.a_position, out.b, out.b_position), (0, 1, 1, 0));
    }
}

/// tab交換のpostcondition検証: move_tab呼出が成功扱いでもpositionが変化しない
/// 系列はVerificationFailed（期待/実測のposition差分を含む）
// [covers:move.tab-swap-verification-failed]
#[test]
fn tab_swap_position_mismatch_is_verification_failed() {
    let tabs = vec![
        tab_at(0, "w", true),
        tab_at(1, "x", false),
        tab_at(2, "y", false),
    ];
    let b = FakeBackend::new(Vec::new(), tabs);
    b.mute_move_tab();
    let err = run_tab_swap(&b, "0", "2").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert!(
        err.message().contains("expected position"),
        "期待/実測のposition差分: {}",
        err.message()
    );
}

/// tab交換の途中消滅（REVIEW-3指摘3）: 先行stepのmove_tabが成功した後に相手tabが
/// 消えた系列では、「生存tabの位置不変」と断定せず、完了step数（2 of 3）と現在の
/// positionを報告するOperationFailedとして中断する
// [covers:move.tab-swap-mid-vanish-operation-failed]
#[test]
fn tab_swap_mid_vanish_reports_partial_progress() {
    let tabs = vec![
        tab_at(0, "w", true),
        tab_at(1, "x", false),
        tab_at(2, "y", false),
        tab_at(3, "z", false),
    ];
    let b = FakeBackend::new(Vec::new(), tabs);
    // d=2（position 0と2の交換）: right×2完了後・left×1の存在確認で相手tab(2)が消える
    b.vanish_tab_after_move_tab(2, TabId(2));
    let err = run_tab_swap(&b, "0", "2").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
    let msg = err.message();
    assert!(
        msg.contains("after 2 of 3"),
        "完了step数の報告: {msg}"
    );
    assert!(
        msg.contains("may be changed") && !msg.contains("are unchanged"),
        "位置が変化しうる旨（不変と断定しない）: {msg}"
    );
    assert!(
        msg.contains("current positions"),
        "現在positionの添付: {msg}"
    );
    // 実状態: right×2が効いている（[0,1,2,3]→[1,0,2,3]→[1,2,0,3]の後、相手(2)消滅で
    // [1,0,3]に正規化。tab 0はposition 2から1へ詰められているが初期position 0ではない）
    let s = b.state.borrow();
    let pos_of = |id: u32| s.tabs.iter().find(|t| t.id.0 == id).map(|t| t.position);
    assert_eq!(pos_of(0), Some(1), "先行stepの移動は保持（rollbackなし）");
    assert_eq!(pos_of(2), None, "相手tabは消滅");
}

/// companion系のpostcondition検証: polling完了条件（tab所属）は成立しても、
/// 完了後のlist-panes再取得で対象pane idが変化している系列はVerificationFailed
// [covers:move.companion-postcondition-verification-failed]
#[test]
fn companion_postcondition_pane_id_violation_is_verification_failed() {
    let (panes, tabs) = two_tabs_two_panes_each();
    let b = FakeBackend::new(panes, tabs);
    // poll 1回目（n=1）で完了条件成立→直後のstrict再取得（n=2）でpane idが変化
    b.rename_moved_pane_after_break_nth(2);
    let err = with_isolated_xdg(|_| run_pane_to_tab(&b, "1", "T1").unwrap_err());
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert!(
        err.message().contains("pane id"),
        "pane id不変違反の報告: {}",
        err.message()
    );
}

/// backend呼出の非zero終了: native系move_paneとtab交換move_tabはいずれも
/// OperationFailed（exit 5相当。geometry比較・postcondition検証には進まない）
// [covers:move.backend-failure-operation-failed]
#[test]
fn backend_failure_is_operation_failed() {
    let b = FakeBackend::new(m1_layout_panes(), vec![tab_at(0, "T0", true)]);
    b.inject_failure("move-pane");
    let err = run_pane_native(
        &b,
        "1",
        PaneMoveAction::Direction(ResizeDirection::Left),
        false,
    )
    .unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);

    let tabs = vec![
        tab_at(0, "w", true),
        tab_at(1, "x", false),
        tab_at(2, "y", false),
    ];
    let b = FakeBackend::new(Vec::new(), tabs);
    b.inject_failure("move-tab");
    let err = run_tab_swap(&b, "0", "2").unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
}
