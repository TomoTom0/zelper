use crate::domain::{PaneKindId, PaneState, TargetSet, TargetSpec};
use crate::error::{ErrorClass, ZelperError};

/// 対象解決（DD-1.4）。panesは`list-panes -a --json`由来の全体集合。
/// positional pane_idsとfilter optionの和集合を返す。順序はvisual order。
pub fn resolve(spec: &TargetSpec, panes: &[PaneState]) -> Result<TargetSet, ZelperError> {
    let mut selected: Vec<PaneState> = Vec::new();

    for id in &spec.pane_ids {
        match panes.iter().find(|p| &p.id == id) {
            Some(p) => {
                if !selected.iter().any(|s| s.id == p.id) {
                    selected.push(p.clone());
                }
            }
            None => {
                return Err(ZelperError::new(
                    ErrorClass::NoTarget,
                    format!(
                        "pane '{}' not found in the current session state",
                        id.as_spec()
                    ),
                ));
            }
        }
    }

    let matches_filter = |p: &PaneState| -> bool {
        if let Some(name) = &spec.name
            && &p.title != name
        {
            return false;
        }
        if let Some(cmd) = &spec.command {
            let hit = p
                .command
                .as_deref()
                .map(|c| c.contains(cmd.as_str()))
                .unwrap_or(false);
            if !hit {
                return false;
            }
        }
        if let Some(cwd) = &spec.cwd
            && p.cwd.as_deref() != Some(cwd.as_str())
        {
            return false;
        }
        true
    };

    // filterはopt-in: 何も指定されなければpositionalのみ。--tabのみの場合は
    // そのtabの全pane（DD-1.4の`read --tab agents`形式）
    let has_filter = spec.all
        || spec.tab.is_some()
        || spec.name.is_some()
        || spec.command.is_some()
        || spec.cwd.is_some();
    let pool: Vec<&PaneState> = if has_filter {
        let pool: Vec<&PaneState> = panes
            .iter()
            .filter(|p| p.is_selectable && matches!(p.id, PaneKindId::Terminal(_)))
            .collect();
        let mut pool = pool;
        if let Some(tab) = spec.tab {
            pool.retain(|p| p.tab_id == tab);
        }
        if !spec.all {
            pool.retain(|p| matches_filter(p));
        }
        pool
    } else {
        Vec::new()
    };

    for p in pool {
        if !selected.iter().any(|s| s.id == p.id) {
            selected.push(p.clone());
        }
    }

    if selected.is_empty() {
        return Err(ZelperError::new(
            ErrorClass::NoTarget,
            "no pane matched the given targets",
        ));
    }

    selected.sort_by_key(|p| p.visual_key());
    Ok(TargetSet { panes: selected })
}

/// 単一対象を要求する操作（rename等）用。複数ヒットはAmbiguousTarget。
pub fn resolve_single(spec: &TargetSpec, panes: &[PaneState]) -> Result<PaneState, ZelperError> {
    let set = resolve(spec, panes)?;
    if set.panes.len() == 1 {
        Ok(set.panes[0].clone())
    } else {
        let candidates = set.panes.iter().map(|p| p.id.as_spec()).collect();
        Err(ZelperError::with_candidates(
            ErrorClass::AmbiguousTarget,
            format!(
                "target matched {} panes; a single target is required",
                set.panes.len()
            ),
            candidates,
        ))
    }
}

/// --tabのTABSPEC（ID または一意な名前）解決
pub fn resolve_tab(
    raw: &str,
    tabs: &[crate::domain::TabState],
) -> Result<crate::domain::TabId, ZelperError> {
    if let Ok(id) = raw.parse::<u32>() {
        let tid = crate::domain::TabId(id);
        if tabs.iter().any(|t| t.id == tid) {
            return Ok(tid);
        }
        return Err(ZelperError::new(
            ErrorClass::NoTarget,
            format!("tab id {id} not found"),
        ));
    }
    let hits: Vec<_> = tabs.iter().filter(|t| t.name == raw).collect();
    match hits.len() {
        1 => Ok(hits[0].id),
        0 => Err(ZelperError::new(
            ErrorClass::NoTarget,
            format!("tab named '{raw}' not found"),
        )),
        n => Err(ZelperError::with_candidates(
            ErrorClass::AmbiguousTarget,
            format!("tab name '{raw}' matched {n} tabs; use the tab id"),
            tabs.iter()
                .filter(|t| t.name == raw)
                .map(|t| t.id.0.to_string())
                .collect(),
        )),
    }
}
