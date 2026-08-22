use crate::domain::{Geometry, PaneKindId, PaneState};
use crate::error::{ErrorClass, ZelperError};
use crate::zellij::{ResizeDirection, ResizeOp, ZellijBackend};

/// 1 paneの方向step resize（DD-9 grow/shrink）。
/// 各step後にgeometryを再取得し、no-op（変化なし）が2回連続なら打ち切る。
pub fn run_pane(
    backend: &dyn ZellijBackend,
    pane: &str,
    grow: bool,
    direction: ResizeDirection,
    steps: u32,
    json: bool,
) -> Result<(), ZelperError> {
    let id = PaneKindId::parse_spec(pane).ok_or_else(|| {
        ZelperError::new(ErrorClass::Usage, format!("invalid pane spec '{pane}'"))
    })?;
    let op = |dir: ResizeDirection| {
        if grow {
            ResizeOp::Grow(dir)
        } else {
            ResizeOp::Shrink(dir)
        }
    };
    let mut applied = 0u32;
    let mut noop_streak = 0u32;
    let mut last = geometry_of(backend, &id)?;
    for _ in 0..steps {
        backend.resize(Some(&id), op(direction))?;
        let now = geometry_of(backend, &id)?;
        if now == last {
            noop_streak += 1;
            if noop_streak >= 2 {
                eprintln!(
                    "zelper: warning: resize is a no-op at the current geometry; stopped after {applied} step(s)"
                );
                break;
            }
        } else {
            noop_streak = 0;
            applied += 1;
            last = now;
        }
    }
    if json {
        println!(
            "{}",
            crate::output::json::ok(serde_json::json!({
                "pane": id.as_spec(),
                "applied_steps": applied,
                "geometry": { "x": last.x, "y": last.y, "rows": last.rows, "cols": last.cols },
            }))
        );
    } else {
        println!(
            "resized {} ({} step(s) applied -> {}x{})",
            id.as_spec(),
            applied,
            last.cols,
            last.rows
        );
    }
    Ok(())
}

fn geometry_of(backend: &dyn ZellijBackend, id: &PaneKindId) -> Result<Geometry, ZelperError> {
    let panes = backend.list_panes()?;
    panes
        .iter()
        .find(|p| &p.id == id)
        .map(|p| p.geometry)
        .ok_or_else(|| {
            ZelperError::new(
                ErrorClass::VerificationFailed,
                format!("pane {} disappeared during resize", id.as_spec()),
            )
        })
}

/// equalize（DD-9）。近似であることを出力に明示する。
/// 同一ROW（y一致）群はcols均等、同一COL（x一致）群はrows均等を順に適用する。
pub fn run_equalize(
    backend: &dyn ZellijBackend,
    panes: &[String],
    tab: Option<&str>,
    json: bool,
) -> Result<(), ZelperError> {
    let all = backend.list_panes()?;
    let targets: Vec<PaneState> = if !panes.is_empty() {
        let ids = super::parse_pane_specs(panes)?;
        // 存在確認: 不在IDは黙って除外せずerror（typo検出）
        let missing: Vec<String> = ids
            .iter()
            .filter(|id| !all.iter().any(|p| &p.id == *id))
            .map(|id| id.as_spec())
            .collect();
        if !missing.is_empty() {
            return Err(ZelperError::new(
                ErrorClass::NoTarget,
                format!("panes not found: {missing:?}"),
            ));
        }
        let mut v: Vec<_> = all
            .iter()
            .filter(|p| ids.contains(&p.id))
            .cloned()
            .collect();
        // 同一tab内のみ対象とする（境界をまたぐequalizeは意味をなさない）
        let tabs: Vec<_> = v.iter().map(|p| p.tab_id).collect::<Vec<_>>();
        let first = tabs[0];
        if tabs.iter().any(|t| *t != first) {
            return Err(ZelperError::new(
                ErrorClass::Preflight,
                "equalize targets must be in the same tab",
            ));
        }
        v.sort_by_key(|p| p.visual_key());
        v
    } else if let Some(raw) = tab {
        let tabs = backend.list_tabs()?;
        let tid = crate::selector::resolve_tab(raw, &tabs)?;
        all.iter()
            .filter(|p| p.tab_id == tid && p.is_remap_source())
            .cloned()
            .collect()
    } else {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "equalize requires panes or --tab",
        ));
    };
    if targets.is_empty() {
        return Err(ZelperError::new(ErrorClass::NoTarget, "no target pane"));
    }
    if targets.iter().any(|p| p.is_floating) {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            "equalize targets tiled panes only; floating panes are excluded",
        ));
    }

    let mut notes: Vec<String> = Vec::new();
    // ROW grouping（y一致）→ cols均等
    let mut by_row: Vec<(u32, Vec<PaneKindId>)> = Vec::new();
    for p in &targets {
        if let Some(entry) = by_row.iter_mut().find(|(y, _)| *y == p.geometry.y) {
            entry.1.push(p.id);
        } else {
            by_row.push((p.geometry.y, vec![p.id]));
        }
    }
    for (_, group) in &by_row {
        if group.len() > 1 {
            equalize_dimension(backend, group, true, &mut notes)?;
        }
    }
    // COL grouping（x一致）→ rows均等
    let after = backend.list_panes()?;
    let mut by_col: Vec<(u32, Vec<PaneKindId>)> = Vec::new();
    for id in targets.iter().map(|p| p.id) {
        let Some(p) = after.iter().find(|q| q.id == id) else {
            return Err(ZelperError::new(
                ErrorClass::VerificationFailed,
                format!("pane {} disappeared during equalize", id.as_spec()),
            ));
        };
        if let Some(entry) = by_col.iter_mut().find(|(x, _)| *x == p.geometry.x) {
            entry.1.push(id);
        } else {
            by_col.push((p.geometry.x, vec![id]));
        }
    }
    for (_, group) in &by_col {
        if group.len() > 1 {
            equalize_dimension(backend, group, false, &mut notes)?;
        }
    }

    let final_panes = backend.list_panes()?;
    let mut report: Vec<serde_json::Value> = Vec::new();
    for t in &targets {
        let Some(p) = final_panes.iter().find(|q| q.id == t.id) else {
            return Err(ZelperError::new(
                ErrorClass::VerificationFailed,
                format!("pane {} disappeared during equalize", t.id.as_spec()),
            ));
        };
        report.push(serde_json::json!({
            "pane": t.id.as_spec(),
            "geometry": { "x": p.geometry.x, "y": p.geometry.y, "rows": p.geometry.rows, "cols": p.geometry.cols },
        }));
    }
    if json {
        println!(
            "{}",
            crate::output::json::ok(serde_json::json!({
                "equalized": report,
                "notes": notes,
                "guarantee": "approximate",
            }))
        );
    } else {
        for r in &report {
            let g = &r["geometry"];
            println!(
                "equalized {} -> {}x{}{}",
                r["pane"].as_str().unwrap_or("?"),
                g["cols"],
                g["rows"],
                if notes.is_empty() {
                    String::new()
                } else {
                    format!(" (notes: {})", notes.join("; "))
                }
            );
        }
        println!("note: equalize is approximate; exact geometry is not guaranteed");
    }
    Ok(())
}

const MAX_ITERS: u32 = 20;

/// 1直線上のgroupを均等化。dimension=trueならcols、falseならrows。
fn equalize_dimension(
    backend: &dyn ZellijBackend,
    group: &[PaneKindId],
    cols: bool,
    notes: &mut Vec<String>,
) -> Result<(), ZelperError> {
    let total: u32 = {
        let panes = backend.list_panes()?;
        group
            .iter()
            .filter_map(|id| {
                panes.iter().find(|p| &p.id == id).map(|p| {
                    if cols {
                        p.geometry.cols
                    } else {
                        p.geometry.rows
                    }
                })
            })
            .sum()
    };
    let n = group.len() as u32;
    let target = total / n;
    let mut remainder = total % n;

    for id in group {
        let mut iters = 0u32;
        let mut prev_prev: Option<Geometry> = None;
        loop {
            if iters >= MAX_ITERS {
                notes.push(format!("{}: iteration limit reached", id.as_spec()));
                break;
            }
            let panes = backend.list_panes()?;
            let cur = panes
                .iter()
                .find(|p| &p.id == id)
                .map(|p| p.geometry)
                .ok_or_else(|| ZelperError::new(ErrorClass::VerificationFailed, "pane lost"))?;
            let have = if cols { cur.cols } else { cur.rows };
            // 端数は前のpaneから1つずつ配る
            let goal = target + if remainder > 0 { 1 } else { 0 };
            if have.abs_diff(goal) <= 1 {
                remainder = remainder.saturating_sub(1);
                break;
            }
            let op = if have < goal {
                ResizeOp::Grow(if cols {
                    ResizeDirection::Right
                } else {
                    ResizeDirection::Down
                })
            } else {
                ResizeOp::Shrink(if cols {
                    ResizeDirection::Right
                } else {
                    ResizeDirection::Down
                })
            };
            backend.resize(Some(id), op)?;
            iters += 1;
            let now = geometry_of(backend, id)?;
            if Some(now) == prev_prev {
                // 振動検出（grow/shrinkが交互に元に戻る）
                notes.push(format!(
                    "{}: no progress (resize step did not change geometry), stopped",
                    id.as_spec()
                ));
                break;
            }
            prev_prev = Some(cur);
        }
    }
    Ok(())
}
