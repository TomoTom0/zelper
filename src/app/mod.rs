pub mod add;
pub mod attach;
pub mod docs;
pub mod list;
pub mod move_;
pub mod read;
pub mod remap;
pub mod remove;
pub mod rename;
pub mod resize;
pub mod send;
pub mod watch;

use crate::error::{ErrorClass, ZelperError};
use crate::zellij::ZellijBackend;

/// 対象session解決（DD-1.2）: --session（事前存在確認つき） >
/// ZELLIJ_SESSION_NAME > 実行中1つ > error。
///
/// --session明示時はlist_sessions（EXITED含む）に該当nameがあればpass-through
/// する（EXITEDも「存在」扱い: attachによるresurrectionの正規経路のため。
/// TASK-93 D3）。なければNoTarget: runningがあればcandidates（running名のみ。
/// auto解決との対称性）つき、runningなしなら作成案内文面（candidatesなし）。
/// flag分岐の一覧取得は名前の有無だけで操作を行わないため、有効名は素通しで
/// 振る舞い不変（list-sessionsが+1回。auto解決経路と同じく解決時にちょうど1回）。
/// env（ZELLIJ_SESSION_NAME）はzellijが自paneへ注入する値のため検証せず
/// pass-throughする（list-sessions呼出も行わない）
pub fn resolve_session(
    flag: Option<&str>,
    backend: &dyn ZellijBackend,
) -> Result<String, ZelperError> {
    if let Some(name) = flag {
        let sessions = backend.list_sessions()?;
        if sessions.iter().any(|s| s.name == name) {
            return Ok(name.to_string());
        }
        let running: Vec<String> = sessions
            .iter()
            .filter(|s| !s.exited)
            .map(|s| s.name.clone())
            .collect();
        if running.is_empty() {
            return Err(ZelperError::new(
                ErrorClass::NoTarget,
                format!(
                    "session '{name}' not found; no running zellij sessions, \
                     create one with 'zelper add session {name}'"
                ),
            ));
        }
        return Err(ZelperError::with_candidates(
            ErrorClass::NoTarget,
            format!("session '{name}' not found"),
            running,
        ));
    }
    if let Ok(name) = std::env::var("ZELLIJ_SESSION_NAME") {
        return Ok(name);
    }
    let sessions = backend
        .list_sessions()?
        .into_iter()
        .filter(|s| !s.exited)
        .collect::<Vec<_>>();
    match sessions.len() {
        1 => Ok(sessions[0].name.clone()),
        0 => Err(ZelperError::new(
            ErrorClass::NoTarget,
            // 0個の内訳（完全0件/EXITEDのみ）で文面を分けない（D5。NAMEはliteral）
            "no running zellij session found; create one with 'zelper add session NAME'",
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
