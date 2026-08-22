pub mod add;
pub mod list;
pub mod read;
pub mod remap;
pub mod remove;
pub mod rename;
pub mod resize;
pub mod send;

use crate::error::{ErrorClass, ZelperError};
use crate::zellij::ZellijBackend;

/// 対象session解決（DD-1.2）: --session > ZELLIJ_SESSION_NAME > 実行中1つ > error
pub fn resolve_session(
    flag: Option<&str>,
    backend: &dyn ZellijBackend,
) -> Result<String, ZelperError> {
    if let Some(name) = flag {
        return Ok(name.to_string());
    }
    if let Ok(name) = std::env::var("ZELLIJ_SESSION_NAME") {
        return Ok(name);
    }
    let sessions = backend.list_sessions()?;
    match sessions.len() {
        1 => Ok(sessions[0].name.clone()),
        0 => Err(ZelperError::new(
            ErrorClass::NoTarget,
            "no running zellij session found; pass --session or create a session first",
        )),
        _ => {
            let names = sessions.iter().map(|s| s.name.clone()).collect();
            Err(ZelperError::with_candidates(
                ErrorClass::AmbiguousTarget,
                "multiple zellij sessions running; pass --session",
                names,
            ))
        }
    }
}

/// PANE指定文字列列をPaneKindIdに変換（不正形式はusage扱い）
pub fn parse_pane_specs(specs: &[String]) -> Result<Vec<crate::domain::PaneKindId>, ZelperError> {
    specs
        .iter()
        .map(|s| {
            crate::domain::PaneKindId::parse_spec(s).ok_or_else(|| {
                ZelperError::new(
                    ErrorClass::Usage,
                    format!("invalid pane spec '{s}': expected terminal_N, plugin_N, or a number"),
                )
            })
        })
        .collect()
}
