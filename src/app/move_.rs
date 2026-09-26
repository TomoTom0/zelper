//! move verb（DD-13）。5操作を1 verbに載せる（設計§1.2）:
//!
//! - pane方向移動・rotate: native `move-pane` wrap（run_pane_native）
//! - paneのtab間移動（`--to-tab`）: companion break-id（run_pane_to_tab）
//! - pane交換（`--swap-with`）: companion break-id ×2 + 順序規則（run_pane_swap）
//! - tab交換（`move tab --swap-with`）: native `move-tab` -tの隣接transposition
//!   合成（run_tab_swap）
//!
//! 対象はtiled terminal paneのみ（DD-13）: floating paneはM1実測で方向移動が
//! 座標shiftとして動作しslot swapにならないため、companion系と同一に状態変更前に
//! Preflight errorで遮断する（`remap --embed-floating`でのtiled化をhint）。
//! plugin paneもpipe経路で特定できないため同じくPreflight error。
//! 実行関数（run_*）は報告構造を返すのみで出力しない（出力はreport_*。attach.rsの
//! report分離と同一pattern。no-op warningの検証可能性のため）

use crate::companion::{self, PERMISSION_HINT};
use crate::domain::{PaneKindId, PaneState, TabId};
use crate::error::{ErrorClass, ZelperError};
use crate::zellij::{MovePaneOp, ResizeDirection, TabMoveDirection, ZellijBackend};

/// move paneの動作（main.rs dispatch由来。cli::MovePaneActionのdomain写像）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneMoveAction {
    Direction(ResizeDirection),
    Rotate,
}

/// run_pane_nativeの実行結果（報告構造）
#[derive(Debug)]
pub struct NativeOutcome {
    pub target: String,
    pub action: String,
    pub backwards: bool,
    /// 幾何比較で移動が起きなかった（edge方向移動のsilent no-op。M1実測）と
    /// 判定された場合true。warning報告 + exit 0（resizeのno-op規約に準拠）
    pub noop: bool,
}

/// run_pane_to_tabの実行結果
#[derive(Debug)]
pub struct ToTabOutcome {
    pub target: String,
    pub tab: u32,
    pub tab_name: String,
    /// 宛先==現tabでpipeなしのno-op完了
    pub noop: bool,
}

/// run_pane_swapの実行結果（両paneの移動後所属）
#[derive(Debug)]
pub struct SwapOutcome {
    pub a: String,
    pub a_tab: u32,
    pub b: String,
    pub b_tab: u32,
}

/// run_tab_swapの実行結果（両tabの移動後position）
#[derive(Debug)]
pub struct TabSwapOutcome {
    pub a: u32,
    pub a_position: u32,
    pub b: u32,
    pub b_position: u32,
}

/// `move pane`のentry（三者排他・--backwards検証後に各経路へdispatch）
pub fn run_pane(
    backend: &dyn ZellijBackend,
    pane: &str,
    action: Option<PaneMoveAction>,
    backwards: bool,
    to_tab: Option<&str>,
    swap_with: Option<&str>,
    json: bool,
) -> Result<(), ZelperError> {
    // 三者排他・ちょうど1つ（DD-1.5。2個同時はclap conflictsでも弾かれるが、
    // app層でも最終検証する。LayoutSpec::validate_exclusiveと同一pattern）
    let n = [action.is_some(), to_tab.is_some(), swap_with.is_some()]
        .iter()
        .filter(|b| **b)
        .count();
    match n {
        0 => {
            return Err(ZelperError::new(
                ErrorClass::Usage,
                "move pane requires exactly one of a direction/rotate positional, \
--to-tab, or --swap-with",
            ));
        }
        1 => {}
        _ => {
            return Err(ZelperError::new(
                ErrorClass::Usage,
                "move pane arguments are mutually exclusive: pass only one of the \
direction/rotate positional, --to-tab, or --swap-with",
            ));
        }
    }
    // --backwardsはaction=rotateとのみ併用可（clapで表現できないenum値条件のため
    // runtime検証。設計§1.2）
    if backwards && action != Some(PaneMoveAction::Rotate) {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "--backwards can only be combined with the rotate action",
        ));
    }
    if let Some(a) = action {
        let out = run_pane_native(backend, pane, a, backwards)?;
        report_native(&out, json);
    } else if let Some(tab) = to_tab {
        let out = run_pane_to_tab(backend, pane, tab)?;
        report_to_tab(&out, json);
    } else {
        let out = run_pane_swap(backend, pane, swap_with.expect("exactly one checked"))?;
        report_swap(&out, json);
    }
    Ok(())
}

/// `move tab`のentry
pub fn run_tab(
    backend: &dyn ZellijBackend,
    tab: &str,
    swap_with: &str,
    json: bool,
) -> Result<(), ZelperError> {
    let out = run_tab_swap(backend, tab, swap_with)?;
    report_tab_swap(&out, json);
    Ok(())
}

/// 対象paneの解決（PANESPEC単一。不在はNoTarget）
fn resolve_pane(backend: &dyn ZellijBackend, pane: &str) -> Result<PaneState, ZelperError> {
    let id = PaneKindId::parse_spec(pane).ok_or_else(|| {
        ZelperError::new(
            ErrorClass::Usage,
            format!("invalid pane spec '{pane}': expected terminal_N, plugin_N, or a number"),
        )
    })?;
    let panes = backend.list_panes()?;
    panes.iter().find(|p| p.id == id).cloned().ok_or_else(|| {
        ZelperError::new(
            ErrorClass::NoTarget,
            format!(
                "pane '{}' not found in the current session state",
                id.as_spec()
            ),
        )
    })
}

/// 実行後のpane再取得（id基準。不在は検証失敗）
fn pane_after(
    backend: &dyn ZellijBackend,
    id: &PaneKindId,
    what: &str,
) -> Result<PaneState, ZelperError> {
    let panes = backend.list_panes()?;
    panes.iter().find(|p| &p.id == id).cloned().ok_or_else(|| {
        ZelperError::new(
            ErrorClass::VerificationFailed,
            format!("pane {} disappeared during {}", id.as_spec(), what),
        )
    })
}

/// move対象paneの事前検査（DD-13: tiled terminal paneのみ。is_remap_source相当の
/// 単一pane版）。floatingはM1実測の座標shift挙動をzeller側で到達不能にするため
/// native系・companion系の双方で遮断し、tiled化のhintを含める
fn preflight_pane(p: &PaneState, path: &str) -> Result<(), ZelperError> {
    if !matches!(p.id, PaneKindId::Terminal(_)) {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "move {path} target must be a terminal pane; {} is a plugin pane (the \
companion pipe cannot address plugin panes)",
                p.id.as_spec()
            ),
        ));
    }
    if p.is_floating {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "move {path} target must be a tiled pane; {} is floating (a directional \
move shifts floating coordinates instead of swapping slots, and rotate is a no-op). use \
`zelper remap --embed-floating` to tile it first (processes preserved)",
                p.id.as_spec()
            ),
        ));
    }
    if p.exited || p.is_held {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "move {path} target {} is exited/held and cannot be moved",
                p.id.as_spec()
            ),
        ));
    }
    if !p.is_selectable {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "move {path} target {} is not selectable and cannot be moved",
                p.id.as_spec()
            ),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// native系（方向移動・rotate）
// ---------------------------------------------------------------------------

/// paneの方向移動・rotate（native `move-pane` wrap）。M1実測: `move-pane <dir> -p`
/// はslot幾何完全不変のpane id付け替えswapで、edge方向移動はrc=0・出力空の
/// silent no-op。そのためexit codeのみでは成否判別不能であり、実行後に
/// list-panesを再取得して対象paneの幾何を比較し、不変ならno-opとして報告する
/// （resizeのno-op報告規約に準拠。無検証の成功を返さない）
pub fn run_pane_native(
    backend: &dyn ZellijBackend,
    pane: &str,
    action: PaneMoveAction,
    backwards: bool,
) -> Result<NativeOutcome, ZelperError> {
    let state = resolve_pane(backend, pane)?;
    preflight_pane(&state, "pane")?;
    let op = match action {
        PaneMoveAction::Direction(d) => MovePaneOp::Direction(d),
        PaneMoveAction::Rotate => {
            if backwards {
                MovePaneOp::RotateBackwards
            } else {
                MovePaneOp::RotateForwards
            }
        }
    };
    backend.move_pane(&state.id, op)?;
    // postcondition: 幾何再取得比較（M8実測: 全操作でpane id・title不変。
    // pane不在はVerificationFailed）
    let after = pane_after(backend, &state.id, "move")?;
    if after.title != state.title {
        return Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!(
                "move of {} changed the pane title ({} -> {}), which native move must not",
                state.id.as_spec(),
                state.title,
                after.title
            ),
        ));
    }
    let noop = after.geometry == state.geometry;
    let action_str = match action {
        PaneMoveAction::Direction(d) => d.as_str().to_string(),
        PaneMoveAction::Rotate => "rotate".to_string(),
    };
    Ok(NativeOutcome {
        target: state.id.as_spec(),
        action: action_str,
        backwards: backwards && action == PaneMoveAction::Rotate,
        noop,
    })
}

// ---------------------------------------------------------------------------
// companion系（--to-tab / --swap-with）
// ---------------------------------------------------------------------------

/// break-id pipe 1回 + 完了polling（pollingはpane id基準でpaneが宛先tabへ移動
/// したことの確認）。move_pane_via_break_idの下請け
fn poll_pane_on_tab(
    backend: &dyn ZellijBackend,
    pane: PaneKindId,
    from: TabId,
    dest: TabId,
) -> Result<(), ZelperError> {
    companion::poll(
        || {
            let Some(panes) = backend.list_panes_lenient()? else {
                return Ok(None);
            };
            Ok(panes
                .iter()
                .any(|p| p.id == pane && p.tab_id == dest)
                .then_some(()))
        },
        || {
            ZelperError::new(
                ErrorClass::OperationFailed,
                format!(
                    "timed out waiting for pane {} to move from tab {} to tab {}: the pane \
was not moved. note: screen may be busy and its 1s list response timeout may have \
produced empty output. a piped command may be delayed and still execute server-side \
after the CLI fails, so state may change later. {PERMISSION_HINT}",
                    pane.as_spec(),
                    from.0,
                    dest.0
                ),
            )
        },
    )
}

/// break-id pipe 1回（payload = `<宛先tab_id> <単一pane_id>`）+ 完了polling。
/// remapのanchor寄せと同一命令の部分利用（設計§3.2 P1）
fn move_pane_via_break_id(
    backend: &dyn ZellijBackend,
    wasm: &std::path::Path,
    pane: &PaneState,
    dest: TabId,
) -> Result<(), ZelperError> {
    let payload = companion::break_id_payload(dest, &[pane.id]);
    backend
        .pipe_plugin(wasm, "break-id", &payload)
        .map_err(|e| {
            ZelperError::new(
                ErrorClass::OperationFailed,
                format!(
                    "break-id pipe failed for pane {} (target tab {}): {}. the pane was not \
moved yet, but a piped command may be delayed and still execute server-side after the \
CLI fails. {PERMISSION_HINT}",
                    pane.id.as_spec(),
                    dest.0,
                    e.message()
                ),
            )
        })?;
    poll_pane_on_tab(backend, pane.id, pane.tab_id, dest)
}

/// paneのtab間移動（`--to-tab`。companion break-id）。実行sequence（設計§3.3）:
/// 対象・宛先解決 → preflight → 宛先==現tabならpipeなしno-op → companion setup +
/// probe（不成立は状態変更前に中断） → break-id pipe + polling（完了条件 =
/// pane.tab_id == 宛先） → postcondition（pane id不変）
pub fn run_pane_to_tab(
    backend: &dyn ZellijBackend,
    pane: &str,
    tab_raw: &str,
) -> Result<ToTabOutcome, ZelperError> {
    let state = resolve_pane(backend, pane)?;
    preflight_pane(&state, "pane --to-tab")?;
    let tabs = backend.list_tabs()?;
    let dest = crate::selector::resolve_tab(tab_raw, &tabs)?;
    let dest_name = tabs
        .iter()
        .find(|t| t.id == dest)
        .map(|t| t.name.clone())
        .unwrap_or_default();
    // 宛先==現tabはcompanion経路を一切使わずwarning付きexit 0（no-op報告。
    // resizeのno-op規約に準拠）
    if state.tab_id == dest {
        return Ok(ToTabOutcome {
            target: state.id.as_spec(),
            tab: dest.0,
            tab_name: dest_name,
            noop: true,
        });
    }
    let wasm = companion::setup_and_probe(backend)?;
    move_pane_via_break_id(backend, &wasm, &state, dest)?;
    // postcondition: pane id不変（M8基準。polling完了条件はtab所属のみのため、
    // 完了後の再取得で対象pane idが変化していないことを検証する）
    let panes_after = backend.list_panes()?;
    let Some(after) = panes_after.iter().find(|p| p.id == state.id) else {
        return Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!(
                "pane id invariant violated: pane {} is no longer present after --to-tab \
(companion moves must preserve pane ids)",
                state.id.as_spec()
            ),
        ));
    };
    if after.tab_id != dest {
        return Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!(
                "pane {} verification failed after --to-tab: expected on tab {}, found tab \
{} (pane id invariant kept)",
                state.id.as_spec(),
                dest.0,
                after.tab_id.0
            ),
        ));
    }
    let after_name = backend
        .list_tabs()?
        .iter()
        .find(|t| t.id == dest)
        .map(|t| t.name.clone())
        .unwrap_or_else(|| dest_name.clone());
    Ok(ToTabOutcome {
        target: state.id.as_spec(),
        tab: dest.0,
        tab_name: after_name,
        noop: false,
    })
}

/// pane交換（`--swap-with`。companion break-id ×2）。順序規則（設計§3.3）: 移動元
/// tab自動close（E6事実1）を構造回避するため、singleton（当該tabのselectable
/// paneが当該paneのみ）側のpaneを後に移動する。両singletonは実行前にPreflight
/// error（どちらのtabも閉じて再作成が必要になりtab位置・idを保存できない）。
/// 同一tab内の交換はbreak系の機構外のためPreflight error（remapをhint）。
/// 途中失敗は移動済み/未移動を正確に報告し、atomicityは主張しない（DD-12）
pub fn run_pane_swap(
    backend: &dyn ZellijBackend,
    pane_a: &str,
    pane_b: &str,
) -> Result<SwapOutcome, ZelperError> {
    let id_a = parse_pane_spec(pane_a)?;
    let id_b = parse_pane_spec(pane_b)?;
    if id_a == id_b {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "cannot swap a pane with itself; pass a different --swap-with target",
        ));
    }
    let panes = backend.list_panes()?;
    let find = |id: PaneKindId| -> Result<PaneState, ZelperError> {
        panes.iter().find(|p| p.id == id).cloned().ok_or_else(|| {
            ZelperError::new(
                ErrorClass::NoTarget,
                format!(
                    "pane '{}' not found in the current session state",
                    id.as_spec()
                ),
            )
        })
    };
    let a = find(id_a)?;
    let b = find(id_b)?;
    preflight_pane(&a, "pane --swap-with")?;
    preflight_pane(&b, "pane --swap-with")?;
    if a.tab_id == b.tab_id {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "both panes are on tab {}; break-id only moves panes across tabs and \
zellij has no same-tab position swap mechanism. use `zelper remap` to rearrange panes \
within a tab",
                a.tab_id.0
            ),
        ));
    }
    // singleton判定（selectable基準。設計§3.3: selectable pane数==1のtab）
    let selectable_in = |tab: TabId| {
        panes
            .iter()
            .filter(|p| p.tab_id == tab && p.is_selectable)
            .count()
    };
    let a_singleton = selectable_in(a.tab_id) == 1;
    let b_singleton = selectable_in(b.tab_id) == 1;
    if a_singleton && b_singleton {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "both panes are the last selectable pane of their tab ({} and {}): either \
move would leave its source tab empty and auto-close it, so tab positions and tab ids \
cannot be preserved. use `move tab --swap-with` (note: tab names and tab id ownership \
are NOT swapped, unlike a pane swap) or `remap` instead",
                a.id.as_spec(),
                b.id.as_spec()
            ),
        ));
    }
    // 順序規則: 両非singletonはA→T_Bが先（決定論的tie-break）。片方のみsingleton
    // は非singleton側（相手tabに到着後も相手tabのpane構成が保たれる側）が先
    let (first, second) = if a_singleton { (&b, &a) } else { (&a, &b) };
    let wasm = companion::setup_and_probe(backend)?;
    // step 1: firstを相手tabへ（firstは必ず非singleton側）
    let first_dest = if first.id == a.id { b.tab_id } else { a.tab_id };
    if let Err(e) = move_pane_via_break_id(backend, &wasm, first, first_dest) {
        return Err(ZelperError::new(
            e.class().clone(),
            format!(
                "{}. partial state: neither pane has moved yet (both panes remain on their \
original tabs), but a piped command may be delayed and still execute server-side after \
the CLI fails, so state may change later",
                e.message()
            ),
        ));
    }
    // step 2: secondを相手tabへ（secondの移動元tabはstep 1の到着paneにより非空）
    let second_dest = if second.id == a.id {
        b.tab_id
    } else {
        a.tab_id
    };
    let second_from = second.tab_id;
    if let Err(e) = move_pane_via_break_id(backend, &wasm, second, second_dest) {
        return Err(ZelperError::new(
            ErrorClass::OperationFailed,
            format!(
                "pane swap failed at the second move: {}. partial state: {} moved to tab \
{}; {} was NOT moved and remains on tab {}. complete manually with `zelper move pane {} \
--to-tab {}`. a piped command may be delayed and still execute server-side after the CLI \
fails, so state may change later. atomicity is not claimed",
                e.message(),
                first.id.as_spec(),
                first_dest.0,
                second.id.as_spec(),
                second_from.0,
                second.id.as_spec(),
                second_dest.0
            ),
        ));
    }
    // postcondition: 両pane id不変・互いの元tabへ所属（M8基準）
    let after_a = pane_after(backend, &a.id, "move --swap-with")?;
    let after_b = pane_after(backend, &b.id, "move --swap-with")?;
    if after_a.tab_id != b.tab_id || after_b.tab_id != a.tab_id {
        return Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!(
                "pane swap verification failed: expected {} on tab {} and {} on tab {}, \
found {} on tab {} and {} on tab {}",
                a.id.as_spec(),
                b.tab_id.0,
                b.id.as_spec(),
                a.tab_id.0,
                a.id.as_spec(),
                after_a.tab_id.0,
                b.id.as_spec(),
                after_b.tab_id.0
            ),
        ));
    }
    Ok(SwapOutcome {
        a: a.id.as_spec(),
        a_tab: b.tab_id.0,
        b: b.id.as_spec(),
        b_tab: a.tab_id.0,
    })
}

fn parse_pane_spec(pane: &str) -> Result<PaneKindId, ZelperError> {
    PaneKindId::parse_spec(pane).ok_or_else(|| {
        ZelperError::new(
            ErrorClass::Usage,
            format!("invalid pane spec '{pane}': expected terminal_N, plugin_N, or a number"),
        )
    })
}

// ---------------------------------------------------------------------------
// tab交換（native move-tab -t の隣接transposition合成）
// ---------------------------------------------------------------------------

/// tab交換（`move tab --swap-with`）。M2実測（`move-tab <dir> -t <tab_id>`は1回の
/// 呼び出しで非active tabをfocus不変で1 position移動し、隣接tabとは1回で純swap
/// 〔A0B1→B0A1〕）からの導出: 左側tab A（position i）をright×d、相手B（position
/// j > i）をleft×(d-1)の計2d-1回の呼び出しで純position交換を実現する（各呼び出し
/// はedgeを越えないためwrap〔全体rotation〕は不発火。d=1はright×1の1回）。
/// step毎にlist-tabsでid存在確認（id基準追跡）し、postconditionでposition交換・
/// id/name不変を検証する
pub fn run_tab_swap(
    backend: &dyn ZellijBackend,
    tab_a: &str,
    tab_b: &str,
) -> Result<TabSwapOutcome, ZelperError> {
    // 同一対象指定は解決前の文字列段で弾く（状態変更・backend呼出前に中断）。
    // 解決後に同一idへ至る別表記（id数値とtab名）はid比較で弾く
    if tab_a == tab_b {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "cannot swap a tab with itself; pass a different --swap-with target",
        ));
    }
    let tabs = backend.list_tabs()?;
    let a_id = crate::selector::resolve_tab(tab_a, &tabs)?;
    let b_id = crate::selector::resolve_tab(tab_b, &tabs)?;
    if a_id == b_id {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "cannot swap a tab with itself; pass a different --swap-with target",
        ));
    }
    let info = |id: TabId| -> (u32, String) {
        tabs.iter()
            .find(|t| t.id == id)
            .map(|t| (t.position, t.name.clone()))
            .expect("resolve_tab confirmed existence")
    };
    let (pos_a, name_a) = info(a_id);
    let (pos_b, name_b) = info(b_id);
    // i < jとなる向き付け（A=左側tab・B=右側tab）。d = j - i
    let (left, right, d) = if pos_a < pos_b {
        (a_id, b_id, pos_b - pos_a)
    } else {
        (b_id, a_id, pos_a - pos_b)
    };
    let ensure_exists = |id: TabId, step: usize| -> Result<(), ZelperError> {
        let now = backend.list_tabs()?;
        if now.iter().any(|t| t.id == id) {
            Ok(())
        } else {
            Err(ZelperError::new(
                ErrorClass::OperationFailed,
                format!(
                    "tab {} disappeared during the swap (at step {}); positions of the \
surviving tabs are unchanged. no rollback is performed",
                    id.0, step
                ),
            ))
        }
    };
    // 左側tabをright×d（各呼び出しは1 position移動。右端を越えないためwrap不発火）
    for step in 0..d as usize {
        ensure_exists(left, step + 1)?;
        backend.move_tab(left, TabMoveDirection::Right)?;
    }
    // 相手側をleft×(d-1)（d=1〔隣接〕はこのphaseなし・right×1の1回のみで純swap）
    for step in 0..d.saturating_sub(1) as usize {
        ensure_exists(right, d as usize + step + 1)?;
        backend.move_tab(right, TabMoveDirection::Left)?;
    }
    // postcondition: position交換・id/name不変をlist-tabsのid追跡で検証
    let after = backend.list_tabs()?;
    let find_after = |id: TabId| -> Option<(u32, String)> {
        after
            .iter()
            .find(|t| t.id == id)
            .map(|t| (t.position, t.name.clone()))
    };
    let (Some((after_a_pos, after_a_name)), Some((after_b_pos, after_b_name))) =
        (find_after(a_id), find_after(b_id))
    else {
        return Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!(
                "tab {} or {} disappeared after the swap (expected positions {} and {})",
                a_id.0, b_id.0, pos_b, pos_a
            ),
        ));
    };
    let mut diffs: Vec<String> = Vec::new();
    if after_a_pos != pos_b {
        diffs.push(format!(
            "tab {} expected position {} (swap), found {}",
            a_id.0, pos_b, after_a_pos
        ));
    }
    if after_b_pos != pos_a {
        diffs.push(format!(
            "tab {} expected position {} (swap), found {}",
            b_id.0, pos_a, after_b_pos
        ));
    }
    if after_a_name != name_a {
        diffs.push(format!(
            "tab {} name changed: {:?} -> {:?}",
            a_id.0, name_a, after_a_name
        ));
    }
    if after_b_name != name_b {
        diffs.push(format!(
            "tab {} name changed: {:?} -> {:?}",
            b_id.0, name_b, after_b_name
        ));
    }
    if !diffs.is_empty() {
        return Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!(
                "tab swap postcondition failed (tab ids are preserved): {}",
                diffs.join("; ")
            ),
        ));
    }
    Ok(TabSwapOutcome {
        a: a_id.0,
        a_position: after_a_pos,
        b: b_id.0,
        b_position: after_b_pos,
    })
}

// ---------------------------------------------------------------------------
// 報告（DD-4.3 変更系1行サマリ規約。no-opはwarning行 + exit 0）
// ---------------------------------------------------------------------------

pub fn report_native(out: &NativeOutcome, json: bool) {
    if out.noop {
        eprintln!(
            "zelper: warning: move was a no-op (the pane is already at the edge of its \
travel; geometry unchanged)"
        );
    }
    if json {
        let mut data = serde_json::json!({
            "target": out.target,
            "action": out.action,
        });
        if out.action == "rotate" {
            data["backwards"] = serde_json::json!(out.backwards);
        }
        if out.noop {
            data["noop"] = serde_json::json!(true);
        }
        println!("{}", crate::output::json::ok(data));
    } else if out.action == "rotate" {
        println!(
            "rotated {} {}",
            out.target,
            if out.backwards {
                "backwards"
            } else {
                "forwards"
            }
        );
    } else {
        println!("moved {} {}", out.target, out.action);
    }
}

pub fn report_to_tab(out: &ToTabOutcome, json: bool) {
    if out.noop {
        eprintln!(
            "zelper: warning: pane {} is already on tab {}; nothing to move (no-op)",
            out.target, out.tab
        );
    }
    if json {
        let mut data = serde_json::json!({
            "target": out.target,
            "action": "to-tab",
            "tab": out.tab,
            "tab_name": out.tab_name,
        });
        if out.noop {
            data["noop"] = serde_json::json!(true);
        }
        println!("{}", crate::output::json::ok(data));
    } else {
        println!("moved {} -> tab {} ({})", out.target, out.tab, out.tab_name);
    }
}

pub fn report_swap(out: &SwapOutcome, json: bool) {
    if json {
        println!(
            "{}",
            crate::output::json::ok(serde_json::json!({
                "action": "swap-pane",
                "panes": [
                    { "id": out.a, "tab": out.a_tab },
                    { "id": out.b, "tab": out.b_tab },
                ],
            }))
        );
    } else {
        println!("swapped {} <-> {}", out.a, out.b);
    }
}

pub fn report_tab_swap(out: &TabSwapOutcome, json: bool) {
    if json {
        println!(
            "{}",
            crate::output::json::ok(serde_json::json!({
                "action": "swap-tab",
                "tabs": [
                    { "id": out.a, "position": out.a_position },
                    { "id": out.b, "position": out.b_position },
                ],
            }))
        );
    } else {
        println!("swapped tab {} <-> tab {}", out.a, out.b);
    }
}
