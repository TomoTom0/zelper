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

/// rename session NAME NEW_NAME: session名変更（DD-8 rename session・TASK-95）。
/// main.rsからは対象session解離を必要としないprobe backendが渡される
/// （RenameTarget::Session例外dispatch。設計§2.7）。
///
/// TASK-95テスト工程時点のstub（テスト先行・fail-first）: signatureのみ確定し、
/// 実行順序（app層Usage検査→事前一覧→不在/EXITED/衝突検査→rename呼出→
/// 事後一覧→postcondition→報告。設計§2.8）は実装工程で置き換える。
/// classは期待class群（Usage/NoTarget/Preflight/OperationFailed/
/// VerificationFailed）と区別可能なPartialFailureを返す
pub fn run_session(
    backend: &dyn ZellijBackend,
    name: &str,
    new_name: &str,
    json: bool,
) -> Result<(), ZelperError> {
    let _ = (backend, name, new_name, json);
    Err(ZelperError::new(
        ErrorClass::PartialFailure,
        "rename session: not yet implemented",
    ))
}
