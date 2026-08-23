use crate::domain::{PaneKindId, TabId};
use crate::error::{ErrorClass, ZelperError};
use crate::output;
use crate::zellij::ZellijBackend;

/// 破壊的複数対象の確認gate（DD-12）: --yes or --dry-run が必要
fn destructive_gate(
    n_targets: usize,
    empty_mode: bool,
    yes: bool,
    dry_run: bool,
) -> Result<(), ZelperError> {
    if yes && dry_run {
        eprintln!("zelper: note: --yes is ignored when --dry-run is given");
    }
    let gated = n_targets > 1 || empty_mode;
    if gated && !yes && !dry_run {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "destructive operation on {n_targets} target(s){} requires --yes or --dry-run",
                if empty_mode { " (--empty)" } else { "" }
            ),
        ));
    }
    Ok(())
}

pub fn run_pane(
    backend: &dyn ZellijBackend,
    panes: &[String],
    yes: bool,
    dry_run: bool,
    json: bool,
) -> Result<(), ZelperError> {
    let ids: Vec<PaneKindId> = super::parse_pane_specs(panes)?;
    if ids.is_empty() {
        return Err(ZelperError::new(ErrorClass::Usage, "no pane specified"));
    }
    let current = backend.list_panes()?;
    // 存在確認 + visual order
    let mut targets: Vec<_> = current
        .iter()
        .filter(|p| ids.contains(&p.id))
        .cloned()
        .collect();
    targets.sort_by_key(|p| p.visual_key());
    let missing: Vec<_> = ids
        .iter()
        .filter(|id| !current.iter().any(|p| p.id == **id))
        .map(|id| id.as_spec())
        .collect();
    if !missing.is_empty() {
        return Err(ZelperError::new(
            ErrorClass::NoTarget,
            format!("panes not found: {missing:?}"),
        ));
    }
    destructive_gate(targets.len(), false, yes, dry_run)?;

    if dry_run {
        let names: Vec<_> = targets.iter().map(|p| p.id.as_spec()).collect();
        if json {
            println!(
                "{}",
                output::json::ok(serde_json::json!({ "plan": { "close_panes": names } }))
            );
        } else {
            for n in &names {
                println!("[plan] close pane {n}");
            }
        }
        return Ok(());
    }

    let mut results: Vec<output::json::TargetedResult<String>> = Vec::new();
    for p in &targets {
        let r = backend.close_pane(&p.id);
        // postcondition: 消失確認（exit 0で無操作の検出）。
        // close呼び出し自体の失敗とpostcondition不整合は区別して報告する
        let gone = backend
            .list_panes()
            .map(|after| !after.iter().any(|q| q.id == p.id));
        let (ok, errmsg) = match (&r, gone) {
            (Ok(()), Ok(true)) => (true, None),
            (Ok(()), Ok(false)) => (
                false,
                Some("pane still present after close (silent no-op?)".to_string()),
            ),
            (Ok(()), Err(e)) => (
                false,
                Some(format!("postcondition check failed: {}", e.message())),
            ),
            (Err(e), _) => (false, Some(e.message().to_string())),
        };
        results.push(output::json::TargetedResult {
            target: p.id.as_spec(),
            ok,
            detail: None,
            error: errmsg,
        });
    }
    report(results, "removed pane", json)
}

pub fn run_tab(
    backend: &dyn ZellijBackend,
    tabs: &[String],
    empty: bool,
    yes: bool,
    dry_run: bool,
    json: bool,
) -> Result<(), ZelperError> {
    let all_tabs = backend.list_tabs()?;
    let targets: Vec<TabId> = if empty {
        // 指定されたTABSPECは解決して絞り込みに使う（解決不能ならerror。
        // 黙って捨てると削除対象が「全空tab」へ暗黙拡大するため）
        let given: Vec<TabId> = tabs
            .iter()
            .map(|raw| crate::selector::resolve_tab(raw, &all_tabs))
            .collect::<Result<_, _>>()?;
        all_tabs
            .iter()
            .filter(|t| t.is_empty() && (given.is_empty() || given.contains(&t.id)))
            .map(|t| t.id)
            .collect()
    } else {
        if tabs.is_empty() {
            return Err(ZelperError::new(ErrorClass::Usage, "no tab specified"));
        }
        let mut ids = Vec::new();
        for raw in tabs {
            ids.push(crate::selector::resolve_tab(raw, &all_tabs)?);
        }
        ids
    };
    if targets.is_empty() {
        return Err(ZelperError::new(
            ErrorClass::NoTarget,
            if empty {
                "no empty tab found"
            } else {
                "no tab matched"
            },
        ));
    }
    destructive_gate(targets.len(), empty, yes, dry_run)?;

    if dry_run {
        let names: Vec<_> = targets.iter().map(|t| t.0.to_string()).collect();
        if json {
            println!(
                "{}",
                output::json::ok(serde_json::json!({ "plan": { "close_tabs": names } }))
            );
        } else {
            for n in &names {
                println!("[plan] close tab {n}");
            }
        }
        return Ok(());
    }

    let mut results: Vec<output::json::TargetedResult<String>> = Vec::new();
    for id in &targets {
        let r = backend.close_tab(*id);
        let gone = backend
            .list_tabs()
            .map(|after| !after.iter().any(|t| t.id == *id));
        let (ok, errmsg) = match (&r, gone) {
            (Ok(()), Ok(true)) => (true, None),
            (Ok(()), Ok(false)) => (
                false,
                Some("tab still present after close (silent no-op?)".to_string()),
            ),
            (Ok(()), Err(e)) => (
                false,
                Some(format!("postcondition check failed: {}", e.message())),
            ),
            (Err(e), _) => (false, Some(e.message().to_string())),
        };
        results.push(output::json::TargetedResult {
            target: id.0.to_string(),
            ok,
            detail: None,
            error: errmsg,
        });
    }
    report(results, "removed tab", json)
}

fn report(
    results: Vec<output::json::TargetedResult<String>>,
    human_ok: &str,
    json: bool,
) -> Result<(), ZelperError> {
    let failures = results.iter().filter(|r| !r.ok).count();
    let err = if failures == 0 {
        None
    } else {
        // DD-4.3: 全失敗=5(OperationFailed)、部分失敗=6(PartialFailure)。read/sendと統一
        Some(
            ZelperError::new(
                if failures < results.len() {
                    ErrorClass::PartialFailure
                } else {
                    ErrorClass::OperationFailed
                },
                format!("{failures}/{} removals failed or unverified", results.len()),
            )
            .with_data(serde_json::json!({ "results": results })),
        )
    };
    if json {
        if failures == 0 {
            let env = serde_json::json!({
                "schema_version": output::json::SCHEMA_VERSION,
                "ok": true,
                "data": { "results": results },
            });
            println!("{env}");
            Ok(())
        } else {
            // 失敗時のenvelope（per-target結果をdataとして同梱）はmainから出力される
            Err(err.expect("failure error"))
        }
    } else {
        for r in &results {
            if r.ok {
                println!("{human_ok} {}", r.target);
            } else {
                println!(
                    "FAILED {} : {}",
                    r.target,
                    r.error.as_deref().unwrap_or("?")
                );
            }
        }
        match err {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }
}
