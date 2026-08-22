use crate::error::{ErrorClass, ZelperError};
use crate::output;
use crate::zellij::{LayoutSpec, NewPaneSpec, NewTabSpec, ZellijBackend};

/// count>1の途中失敗時に、作成済みIDをerror messageに載せる（DD-11: rollbackしない）
fn add_partial(e: ZelperError, created: &[String]) -> ZelperError {
    if created.is_empty() {
        e
    } else {
        ZelperError::new(
            e.class().clone(),
            format!(
                "{} | created so far: {created:?} (kept; no rollback)",
                e.message()
            ),
        )
    }
}

pub fn run_pane(
    backend: &dyn ZellijBackend,
    tab: Option<&str>,
    count: u32,
    name: Option<&str>,
    cwd: Option<&std::path::Path>,
    command: &[String],
    json: bool,
) -> Result<(), ZelperError> {
    if count == 0 {
        return Err(ZelperError::new(ErrorClass::Usage, "--count must be >= 1"));
    }
    let tab_id = match tab {
        Some(raw) => {
            let tabs = backend.list_tabs()?;
            Some(crate::selector::resolve_tab(raw, &tabs)?)
        }
        None => None,
    };
    let mut created: Vec<String> = Vec::new();
    for i in 0..count {
        let pname = match (name, count) {
            (Some(n), c) if c > 1 => Some(format!("{n}-{}", i + 1)),
            (n, _) => n.map(|s| s.to_string()),
        };
        let spec = NewPaneSpec {
            tab: tab_id,
            name: pname,
            cwd: cwd.map(|p| p.to_path_buf()),
            command: command.to_vec(),
        };
        created.push(
            backend
                .new_pane(&spec)
                .map_err(|e| add_partial(e, &created))?
                .as_spec(),
        );
    }
    // postcondition: 作成IDの存在確認
    let panes = backend.list_panes()?;
    let missing: Vec<_> = created
        .iter()
        .filter(|c| !panes.iter().any(|p| p.id.as_spec() == c.as_str()))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!("created panes not found in state afterwards: {missing:?}"),
        ));
    }
    if json {
        println!(
            "{}",
            output::json::ok(serde_json::json!({ "created": created }))
        );
    } else {
        for c in &created {
            println!("created pane {c}");
        }
    }
    Ok(())
}

pub fn run_tab(
    backend: &dyn ZellijBackend,
    count: u32,
    name: Option<&str>,
    cwd: Option<&std::path::Path>,
    layout: Option<LayoutSpec>,
    command: &[String],
    json: bool,
) -> Result<(), ZelperError> {
    if count == 0 {
        return Err(ZelperError::new(ErrorClass::Usage, "--count must be >= 1"));
    }
    if let Some(l) = &layout {
        l.validate_exclusive()?;
    }
    let mut created: Vec<String> = Vec::new();
    for i in 0..count {
        let tname = match (name, count) {
            (Some(n), c) if c > 1 => Some(format!("{n}-{}", i + 1)),
            (n, _) => n.map(|s| s.to_string()),
        };
        let spec = NewTabSpec {
            name: tname,
            cwd: cwd.map(|p| p.to_path_buf()),
            layout: layout.clone(),
            command: command.to_vec(),
        };
        created.push(
            backend
                .new_tab(&spec)
                .map_err(|e| add_partial(e, &created))?
                .0
                .to_string(),
        );
    }
    let tabs = backend.list_tabs()?;
    let missing: Vec<_> = created
        .iter()
        .filter(|c| !tabs.iter().any(|t| t.id.0.to_string() == **c))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!("created tabs not found in state afterwards: {missing:?}"),
        ));
    }
    if json {
        println!(
            "{}",
            output::json::ok(serde_json::json!({ "created_tabs": created }))
        );
    } else {
        for c in &created {
            println!("created tab {c}");
        }
    }
    Ok(())
}
