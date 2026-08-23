use crate::cli::FilterArgs;
use crate::domain::{PaneKindId, TargetSpec};
use crate::error::{ErrorClass, ZelperError};
use crate::output;
use crate::selector;
use crate::zellij::ZellijBackend;

pub fn run(
    backend: &dyn ZellijBackend,
    panes: &[String],
    filter: &FilterArgs,
    full: bool,
    tail: Option<usize>,
    json: bool,
) -> Result<(), ZelperError> {
    let all_panes = backend.list_panes()?;
    let spec = build_spec(backend, panes, filter)?;
    let set = selector::resolve(&spec, &all_panes)?;

    let mut results: Vec<output::json::TargetedResult<String>> = Vec::new();
    for pane in &set.panes {
        let r = backend.dump_screen(&pane.id, full);
        match r {
            Ok(content) => {
                let content = apply_tail(&content, tail);
                results.push(output::json::TargetedResult {
                    target: pane.id.as_spec(),
                    ok: true,
                    detail: Some(content),
                    error: None,
                });
            }
            Err(e) => results.push(output::json::TargetedResult {
                target: pane.id.as_spec(),
                ok: false,
                detail: None,
                error: Some(e.message().to_string()),
            }),
        }
    }

    let failures = results.iter().filter(|r| !r.ok).count();
    let exit_partial = failures > 0 && failures < results.len();

    let err = if failures == 0 {
        None
    } else {
        Some(
            ZelperError::new(
                if exit_partial {
                    ErrorClass::PartialFailure
                } else {
                    ErrorClass::OperationFailed
                },
                format!("{failures}/{} pane reads failed", results.len()),
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
            let pane = set
                .panes
                .iter()
                .find(|p| p.id.as_spec() == r.target)
                .expect("result target from set");
            if r.ok {
                println!(
                    "=== {} ({}, tab:{} {}) ===",
                    r.target, pane.title, pane.tab_id.0, pane.tab_name
                );
                if let Some(c) = &r.detail {
                    println!("{c}");
                }
            } else {
                println!(
                    "=== {} ({}) === FAILED: {}",
                    r.target,
                    pane.title,
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

/// --tail: 取得済み内容の末尾N行
fn apply_tail(content: &str, tail: Option<usize>) -> String {
    match tail {
        Some(n) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(n);
            lines[start..].join("\n")
        }
        None => content.to_string(),
    }
}

/// CLI引数からTargetSpecへ（--tabの名前解決を含む）
pub fn build_spec(
    backend: &dyn ZellijBackend,
    panes: &[String],
    filter: &FilterArgs,
) -> Result<TargetSpec, ZelperError> {
    let pane_ids: Vec<PaneKindId> = super::parse_pane_specs(panes)?;
    let tab = match &filter.tab {
        Some(raw) => {
            let tabs = backend.list_tabs()?;
            Some(selector::resolve_tab(raw, &tabs)?)
        }
        None => None,
    };
    Ok(TargetSpec {
        pane_ids,
        name: filter.name.clone(),
        command: filter.command.clone(),
        cwd: filter.cwd.clone(),
        all: filter.all,
        tab,
    })
}
