pub mod parser;
pub mod process;

use crate::domain::{PaneKindId, PaneState, SessionRef, TabId, TabState};
use crate::error::ZelperError;

/// layout系の作成spec
#[derive(Debug, Clone)]
pub struct LayoutSpec {
    pub name: Option<String>,
    pub path: Option<std::path::PathBuf>,
    pub inline: Option<String>,
}

impl LayoutSpec {
    /// 3sourceの相互排他検証（DD-1.5）
    pub fn validate_exclusive(&self) -> Result<(), ZelperError> {
        let n = [
            self.name.is_some(),
            self.path.is_some(),
            self.inline.is_some(),
        ]
        .iter()
        .filter(|b| **b)
        .count();
        match n {
            0 => Err(ZelperError::new(
                crate::error::ErrorClass::Usage,
                "exactly one layout source (name / --path / --inline) is required",
            )),
            1 => Ok(()),
            _ => Err(ZelperError::new(
                crate::error::ErrorClass::Usage,
                "layout sources are mutually exclusive: pass only one of positional name / --path / --inline",
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NewPaneSpec {
    pub tab: Option<TabId>,
    pub name: Option<String>,
    pub cwd: Option<std::path::PathBuf>,
    pub command: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct NewTabSpec {
    pub name: Option<String>,
    pub cwd: Option<std::path::PathBuf>,
    pub layout: Option<LayoutSpec>,
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeDirection {
    Left,
    Right,
    Up,
    Down,
}

impl ResizeDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResizeDirection::Left => "left",
            ResizeDirection::Right => "right",
            ResizeDirection::Up => "up",
            ResizeDirection::Down => "down",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ResizeOp {
    Grow(ResizeDirection),
    Shrink(ResizeDirection),
}

/// move pane操作の種別（DD-13。設計§2.3）。directionは`ResizeDirection`（left/
/// right/up/down）を再利用し、rotateはforwards/backwardsの2値。argvは
/// `move-pane <dir> -p <spec>` / `move-pane -p <spec>`（bare = rotate forwards）/
/// `move-pane-backwards -p <spec>`（M1実測確定）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovePaneOp {
    Direction(ResizeDirection),
    RotateForwards,
    RotateBackwards,
}

/// move-tabの方向（DD-13。M2実測確定: direction positional必須・`-t <tab_id>`で
/// 非active tabをfocus不変で1 position移動）。Left/Rightのみの専用表現
/// （`ResizeDirection`流用は4値として型過大のため不採用）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabMoveDirection {
    Left,
    Right,
}

impl TabMoveDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            TabMoveDirection::Left => "left",
            TabMoveDirection::Right => "right",
        }
    }
}

#[derive(Debug, Clone)]
pub struct OverrideSpec {
    pub source: LayoutSpec,
    /// zelperは常にtrue（他tab保護。DD-10.1）
    pub apply_only_to_active_tab: bool,
    pub retain_terminal: bool,
    pub retain_plugin: bool,
}

/// typed Zellij adapter（DD-3.2）。fake実装と差し替え可能。
pub trait ZellijBackend {
    fn version(&self) -> Result<String, ZelperError>;
    fn list_sessions(&self) -> Result<Vec<SessionRef>, ZelperError>;
    fn list_tabs(&self) -> Result<Vec<TabState>, ZelperError>;
    fn list_tabs_lenient(&self) -> Result<Option<Vec<TabState>>, ZelperError>;
    /// 指定session名でlist-tabs（list sessionsのPANES概要用。対象session解決に
    /// 依らず任意の実行中sessionのtab構成を取る）
    fn list_tabs_for(&self, session: &str) -> Result<Vec<TabState>, ZelperError>;
    fn list_panes(&self) -> Result<Vec<PaneState>, ZelperError>;
    fn list_panes_lenient(&self) -> Result<Option<Vec<PaneState>>, ZelperError>;
    fn current_tab(&self) -> Result<TabState, ZelperError>;
    fn dump_screen(&self, pane: &PaneKindId, full: bool) -> Result<String, ZelperError>;
    fn write_chars(&self, pane: &PaneKindId, text: &str) -> Result<(), ZelperError>;
    fn write_bytes(&self, pane: &PaneKindId, bytes: &[u8]) -> Result<(), ZelperError>;
    fn send_keys(&self, pane: &PaneKindId, keys: &[String]) -> Result<(), ZelperError>;
    fn rename_pane(&self, pane: &PaneKindId, name: &str) -> Result<(), ZelperError>;
    fn rename_tab(&self, tab: TabId, name: &str) -> Result<(), ZelperError>;
    fn new_pane(&self, spec: &NewPaneSpec) -> Result<PaneKindId, ZelperError>;
    fn new_tab(&self, spec: &NewTabSpec) -> Result<TabId, ZelperError>;
    fn close_pane(&self, pane: &PaneKindId) -> Result<(), ZelperError>;
    fn close_tab(&self, tab: TabId) -> Result<(), ZelperError>;
    fn resize(&self, pane: Option<&PaneKindId>, op: ResizeOp) -> Result<(), ZelperError>;
    /// native move-pane系（DD-13）。argv: `move-pane <dir> -p <spec>` /
    /// `move-pane -p <spec>`（rotate forwards）/ `move-pane-backwards -p <spec>`。
    /// M1実測: edge方向移動はrc=0・出力空のsilent no-opのため、成否はcallerが
    /// list-panes再取得の幾何比較で判定する
    fn move_pane(&self, pane: &PaneKindId, op: MovePaneOp) -> Result<(), ZelperError>;
    /// native move-tab wrap（DD-13）。argv: `move-tab <left|right> -t <tab_id>`。
    /// M2実測: 1回の呼び出しで非active tabをfocus不変で1 position移動
    fn move_tab(&self, tab: TabId, direction: TabMoveDirection) -> Result<(), ZelperError>;
    fn override_layout(&self, spec: &OverrideSpec) -> Result<(), ZelperError>;
    fn go_to_tab(&self, tab: TabId) -> Result<(), ZelperError>;
    /// action focus-pane-id <PANE_ID>（DD-10.7 step 6 v2.2。PANE_ID形式は
    /// PaneKindId::as_spec()のterminal_N / plugin_N系列分離。backendはargv組み立て
    /// とResultを返す薄い実装のみで、Errのwarning化はexecute側の責務〔TR75-10〕）
    fn focus_pane(&self, pane: &PaneKindId) -> Result<(), ZelperError>;
    fn dump_layout(&self) -> Result<String, ZelperError>;
    fn toggle_embed_floating(&self, pane: &PaneKindId) -> Result<(), ZelperError>;
    /// 対話attach（DD-14）。run_interactive経由（stdio inherit×3・timeout無効。
    /// DD-3.1例外）。argvは `attach <session>` positional形式
    fn attach_session(&self) -> Result<(), ZelperError>;
    /// 対象sessionのattach clientをdetach（DD-14）。`--session NAME action detach`
    /// wrap（現行run_actionの枠）
    fn detach_session(&self) -> Result<(), ZelperError>;
    /// detached session作成（DD-14）。argvは `attach -b NAME`（positional形式。
    /// global --session形式はrandom別名sessionを作るため禁止——M5実測）。
    /// session解決に依存しないtop-level argv
    fn create_session_detached(&self, name: &str) -> Result<(), ZelperError>;
    /// sessionの完全削除（DD-11 remove session・TASK-84）。argvは
    /// `delete-session <name> -f`（top-level positional。run_actionの--session接頭
    /// なし）。kill-sessionとは終状態が同一（行+session_info完全消滅・実測M4/M5）
    /// で状態（RUNNING/死data）に依らず動作するため単一methodに統一
    fn delete_session(&self, name: &str) -> Result<(), ZelperError>;
    /// session名変更（DD-8 rename session・TASK-95）。argvは
    /// `--session <from> action rename-session <to>`（run_action_as形式・M4 H実測）。
    /// 既存session名へのrename・空文字・'/'含み名はrc=0で黙って失敗する（M4 I・
    /// M95-C1/C2）ため、衝突・validation事前検査とpostcondition検証はcallerの責務
    fn rename_session(&self, from: &str, to: &str) -> Result<(), ZelperError>;
    fn pipe_plugin(
        &self,
        path: &std::path::Path,
        name: &str,
        payload: &str,
    ) -> Result<(), ZelperError>;
    // action pipe --plugin file:<path> --name <name> -- <payload>（DD-10.3）。
    // 権限dialog pending等でblockするためtimeout必須。効果の成否は戻り値に現れない
    // （成功時に即座exit 0）ため、callerがpolling + postcondition検証で判定する
}

/// 最小サポートversion（DD-3.1/3.5。DD-10 v2改訂に伴い引き上げ）。
/// 実行時要件はzellij >=0.44.3。実証済み組合せはzellij 0.44.3 + zellij-tile 0.44.3
/// （companion plugin build）のみで、0.44.x系列を超える未来versionは受け付けない
pub const MIN_SUPPORTED: (u32, u32, u32) = (0, 44, 3);
