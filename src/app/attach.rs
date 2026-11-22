use crate::error::{ErrorClass, ZelperError};
use crate::output;
use crate::zellij::ZellijBackend;

/// 対話attach（DD-14）。TTY検証（stdin・stdout両方）はzellij起動前にfail-fast
/// する。検証と本体は`run_attach_inner`へ分離し、testはtty_ok引数で注入する
/// （設計§2.5/§5.2）
pub fn run_attach(
    backend: &dyn ZellijBackend,
    session: &str,
    json: bool,
) -> Result<(), ZelperError> {
    use std::io::IsTerminal;
    let tty_ok = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    run_attach_inner(backend, session, json, tty_ok)
}

/// attach本体。tty_ok=falseはzellij attachを起動する前にPreflight（exit 7）で
/// 拒否する（M4実測: 全fd非TTYの素attachはzellijがpanic exit 101・stdout汚染、
/// stdin=TTY/stdout=pipeではUI出力先不在でblockするため、それらに到達させる前に
/// 止める。stdout片側のみ非TTYの構成もblock防止のため拒否する）
pub fn run_attach_inner(
    backend: &dyn ZellijBackend,
    session: &str,
    json: bool,
    tty_ok: bool,
) -> Result<(), ZelperError> {
    if !tty_ok {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            "attach requires a TTY (stdin and stdout); for non-interactive session creation use `zelper add session`",
        ));
    }
    backend.attach_session()?;
    // 対話終了後のみ出力（対話中は一切出力しない。stdoutはzellijが占有する）
    println!("{}", attach_report(session, json));
    Ok(())
}

/// 対象sessionのattach clientをdetach（DD-14）。`--session NAME action detach`
/// wrapで、zellijのaction detachはclient不在でもrc=0 silent（M3 (b)実測）の
/// ため、client数を検査せずrc=0を無条件で成功扱いする（no-op成功をerrorとしない）
pub fn run_detach(
    backend: &dyn ZellijBackend,
    session: &str,
    json: bool,
) -> Result<(), ZelperError> {
    backend.detach_session()?;
    println!("{}", detach_report(session, json));
    Ok(())
}

/// attach終了後の報告文。humanは `attach session '<NAME>' ended` の1行・
/// JSONはenvelope 1通（data.session=NAME。DD-14・DD-4.2）。出力契約の検証は
/// この純粋関数単位で行う（対話中の出力がないことはrun_attach_innerの構造
/// 〔attach_session復帰後にのみ出力〕が保証する）
pub fn attach_report(session: &str, json: bool) -> String {
    if json {
        output::json::ok(serde_json::json!({ "session": session }))
    } else {
        format!("attach session '{session}' ended")
    }
}

/// detachの報告文。humanは `detached clients from session '<NAME>'` の1行・
/// JSONはenvelope 1通（data.session=NAME）
pub fn detach_report(session: &str, json: bool) -> String {
    if json {
        output::json::ok(serde_json::json!({ "session": session }))
    } else {
        format!("detached clients from session '{session}'")
    }
}
