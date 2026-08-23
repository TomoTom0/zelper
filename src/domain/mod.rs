use serde::{Deserialize, Serialize};

/// zellij sessionの参照（session名。ID概念はzellijに不存在）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionRef {
    pub name: String,
}

/// zellij tab ID。**close後再利用されるため不安定キー**（取得直後に消費する運用のみ）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub u32);

/// pane ID。terminal/pluginで独立採番（DD-2）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PaneKindId {
    Terminal(u32),
    Plugin(u32),
}

impl PaneKindId {
    /// CLI/zellij表記（`terminal_3` / `plugin_1`）
    pub fn as_spec(&self) -> String {
        match self {
            PaneKindId::Terminal(n) => format!("terminal_{n}"),
            PaneKindId::Plugin(n) => format!("plugin_{n}"),
        }
    }

    /// PANESPEC表記（`3` / `terminal_3` / `plugin_1`）からのparse。bare数字はterminal_N。
    pub fn parse_spec(s: &str) -> Option<PaneKindId> {
        if let Some(rest) = s.strip_prefix("terminal_") {
            rest.parse().ok().map(PaneKindId::Terminal)
        } else if let Some(rest) = s.strip_prefix("plugin_") {
            rest.parse().ok().map(PaneKindId::Plugin)
        } else if !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
            s.parse().ok().map(PaneKindId::Terminal)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Geometry {
    pub x: u32,
    pub y: u32,
    pub rows: u32,
    pub cols: u32,
}

/// pane状態（`list-panes -a --json` 1要素の正規形）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneState {
    pub id: PaneKindId,
    pub title: String,
    pub is_selectable: bool,
    pub is_floating: bool,
    pub is_focused: bool,
    pub exited: bool,
    pub is_held: bool,
    pub geometry: Geometry,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub tab_id: TabId,
    pub tab_position: u32,
    pub tab_name: String,
    pub plugin_url: Option<String>,
}

impl PaneState {
    /// remap対象: selectable かつ tiled なterminal pane（DD-10.2）
    pub fn is_remap_source(&self) -> bool {
        self.is_selectable && !self.is_floating && matches!(self.id, PaneKindId::Terminal(_))
    }

    /// visual order用sort key（tab_position, y, x）
    pub fn visual_key(&self) -> (u32, u32, u32) {
        (self.tab_position, self.geometry.y, self.geometry.x)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabState {
    pub id: TabId,
    pub position: u32,
    pub name: String,
    pub active: bool,
    pub selectable_tiled_panes_count: u32,
    pub selectable_floating_panes_count: u32,
    pub are_floating_panes_visible: bool,
}

impl TabState {
    /// 「空tab」= selectable な pane（tiled+floating両方）が0個（DD-11）
    pub fn is_empty(&self) -> bool {
        self.selectable_tiled_panes_count == 0 && self.selectable_floating_panes_count == 0
    }
}

/// layout参照（DD-2）。positional=name、path/inlineはoption専用
#[derive(Debug, Clone)]
pub enum LayoutRef {
    Name(String),
    Path(std::path::PathBuf),
    Inline(String),
}

/// 解決済み対象集合（決定順序 = visual order）
#[derive(Debug, Clone, Default)]
pub struct TargetSet {
    pub panes: Vec<PaneState>,
}

/// 対象指定（CLI → selectorへの入力。DD-1.4）
#[derive(Debug, Clone, Default)]
pub struct TargetSpec {
    pub pane_ids: Vec<PaneKindId>,
    pub name: Option<String>,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub all: bool,
    pub tab: Option<TabId>,
}

impl TargetSpec {
    pub fn is_empty(&self) -> bool {
        self.pane_ids.is_empty()
            && self.name.is_none()
            && self.command.is_none()
            && self.cwd.is_none()
            && !self.all
            && self.tab.is_none()
    }
}
