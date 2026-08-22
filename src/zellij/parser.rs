use crate::domain::{Geometry, PaneKindId, PaneState, SessionRef, TabId, TabState};
use crate::error::{ErrorClass, ZelperError};
use serde::Deserialize;

/// `zellij --version`出力（例: `zellij 0.44.3`）のparse
pub fn parse_version(out: &str) -> Option<(u32, u32, u32)> {
    let second = out.split_whitespace().nth(1)?;
    let mut it = second.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next()?.parse().ok()?;
    let patch = it
        .next()
        .map(|p| {
            p.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    Some((major, minor, patch))
}

/// `list-sessions -n`テキスト（例: `zelper-p1-basic [Created 10s ago]`）→ SessionRef列
pub fn parse_sessions(out: &str) -> Vec<SessionRef> {
    out.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let name = l.split_whitespace().next()?;
            if name.is_empty() {
                None
            } else {
                Some(SessionRef {
                    name: name.to_string(),
                })
            }
        })
        .collect()
}

/// new-pane stdout（`terminal_1\n`）→ PaneKindId
pub fn parse_created_pane(out: &str) -> Result<PaneKindId, ZelperError> {
    let t = out.trim();
    PaneKindId::parse_spec(t).ok_or_else(|| {
        ZelperError::new(
            ErrorClass::OperationFailed,
            format!("unexpected new-pane output: {t:?}"),
        )
    })
}

/// new-tab stdout（`1\n`）→ TabId
pub fn parse_created_tab(out: &str) -> Result<TabId, ZelperError> {
    let t = out.trim();
    t.parse::<u32>().map(TabId).map_err(|_| {
        ZelperError::new(
            ErrorClass::OperationFailed,
            format!("unexpected new-tab output: {t:?}"),
        )
    })
}

/// ---- PaneInfo（`list-panes -a --json`要素・実出力field名） ----

#[derive(Debug, Deserialize)]
pub struct PaneInfo {
    pub id: u32,
    pub is_plugin: bool,
    pub is_focused: bool,
    pub is_floating: bool,
    pub title: String,
    pub exited: bool,
    pub is_held: bool,
    pub pane_x: u32,
    pub pane_y: u32,
    pub pane_rows: u32,
    pub pane_columns: u32,
    pub is_selectable: bool,
    pub plugin_url: Option<String>,
    pub tab_id: u32,
    pub tab_position: u32,
    pub tab_name: String,
    pub pane_command: Option<String>,
    pub pane_cwd: Option<String>,
}

pub fn parse_panes(json: &str) -> Result<Vec<PaneState>, ZelperError> {
    let infos: Vec<PaneInfo> = serde_json::from_str(json).map_err(|e| {
        ZelperError::new(
            ErrorClass::OperationFailed,
            format!("failed to parse list-panes output: {e}"),
        )
    })?;
    Ok(infos
        .into_iter()
        .map(|i| PaneState {
            id: if i.is_plugin {
                PaneKindId::Plugin(i.id)
            } else {
                PaneKindId::Terminal(i.id)
            },
            title: i.title,
            is_selectable: i.is_selectable,
            is_floating: i.is_floating,
            is_focused: i.is_focused,
            exited: i.exited,
            is_held: i.is_held,
            geometry: Geometry {
                x: i.pane_x,
                y: i.pane_y,
                rows: i.pane_rows,
                cols: i.pane_columns,
            },
            command: i.pane_command,
            cwd: i.pane_cwd,
            tab_id: TabId(i.tab_id),
            tab_position: i.tab_position,
            tab_name: i.tab_name,
            plugin_url: i.plugin_url,
        })
        .collect())
}

/// ---- TabInfo（`list-tabs -a --json`要素・実出力field名） ----

#[derive(Debug, Deserialize)]
pub struct TabInfo {
    pub position: u32,
    pub name: String,
    pub active: bool,
    pub are_floating_panes_visible: bool,
    pub selectable_tiled_panes_count: u32,
    pub selectable_floating_panes_count: u32,
    pub tab_id: u32,
}

pub fn parse_tabs(json: &str) -> Result<Vec<TabState>, ZelperError> {
    let infos: Vec<TabInfo> = serde_json::from_str(json).map_err(|e| {
        ZelperError::new(
            ErrorClass::OperationFailed,
            format!("failed to parse list-tabs output: {e}"),
        )
    })?;
    Ok(infos
        .into_iter()
        .map(|i| TabState {
            id: TabId(i.tab_id),
            position: i.position,
            name: i.name,
            active: i.active,
            selectable_tiled_panes_count: i.selectable_tiled_panes_count,
            selectable_floating_panes_count: i.selectable_floating_panes_count,
            are_floating_panes_visible: i.are_floating_panes_visible,
        })
        .collect())
}
