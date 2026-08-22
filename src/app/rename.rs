use crate::domain::PaneKindId;
use crate::error::{ErrorClass, ZelperError};
use crate::zellij::ZellijBackend;

pub fn run_pane(
    backend: &dyn ZellijBackend,
    pane: &str,
    name: &str,
    json: bool,
) -> Result<(), ZelperError> {
    let id = PaneKindId::parse_spec(pane).ok_or_else(|| {
        ZelperError::new(
            ErrorClass::Usage,
            format!("invalid pane spec '{pane}': expected terminal_N, plugin_N, or a number"),
        )
    })?;
    let before = backend.list_panes()?;
    if !before.iter().any(|p| p.id == id) {
        return Err(ZelperError::new(
            ErrorClass::NoTarget,
            format!("pane '{}' not found", id.as_spec()),
        ));
    }
    backend.rename_pane(&id, name)?;
    // postcondition: title反映の検証（サイレント成功の検出。DD-8）
    let after = backend.list_panes()?;
    let ok = after.iter().any(|p| p.id == id && p.title == name);
    if ok {
        if json {
            println!(
                "{}",
                crate::output::json::ok(
                    serde_json::json!({ "renamed": id.as_spec(), "name": name })
                )
            );
        } else {
            println!("renamed pane {} -> {}", id.as_spec(), name);
        }
        Ok(())
    } else {
        Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!(
                "rename of {} did not take effect (title unchanged after rename)",
                id.as_spec()
            ),
        ))
    }
}

pub fn run_tab(
    backend: &dyn ZellijBackend,
    tab: &str,
    name: &str,
    json: bool,
) -> Result<(), ZelperError> {
    let tabs = backend.list_tabs()?;
    let tid = crate::selector::resolve_tab(tab, &tabs)?;
    backend.rename_tab(tid, name)?;
    let after = backend.list_tabs()?;
    let ok = after.iter().any(|t| t.id == tid && t.name == name);
    if ok {
        if json {
            println!(
                "{}",
                crate::output::json::ok(serde_json::json!({ "renamed_tab": tid.0, "name": name }))
            );
        } else {
            println!("renamed tab {} -> {}", tid.0, name);
        }
        Ok(())
    } else {
        Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!("rename of tab {} did not take effect", tid.0),
        ))
    }
}
