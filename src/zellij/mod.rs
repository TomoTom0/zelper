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
    fn list_panes(&self) -> Result<Vec<PaneState>, ZelperError>;
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
    fn override_layout(&self, spec: &OverrideSpec) -> Result<(), ZelperError>;
    fn go_to_tab(&self, tab: TabId) -> Result<(), ZelperError>;
    fn dump_layout(&self) -> Result<String, ZelperError>;
    fn toggle_embed_floating(&self, pane: &PaneKindId) -> Result<(), ZelperError>;
}

/// 最小サポートversion（DD-3.1: 0.44.1 = --layout-string導入）
pub const MIN_SUPPORTED: (u32, u32, u32) = (0, 44, 1);
