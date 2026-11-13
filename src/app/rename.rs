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
/// 実行順序（設計§2.8）: app層Usage検査（空文字・'/'含み・空白含み・同一名
/// ——空文字・'/'・同一名はzellijがrc=0で黙って失敗/部分適用する系列〔M4 I・
/// M95-B/C1/C2〕、空白含みはzellijが受理してもlist_sessionsの世界modelで
/// 復元不能なため、いずれも一覧取得より前にfail-fast）→ 事前list_sessions（1回の取得をNAME不在・
/// EXITED判定・NEW_NAME衝突の全判定に共用。存在確認はEXITED込み = remove
/// sessionと同一基準）→ rename呼出（action形式・DD-3.1判定はbackendが担う）→
/// 事後list_sessionsでpostcondition（旧名消失+新名running出現。rc=0黙失敗の
/// 検出）→ 報告。事前/事後一覧取得の失敗は0件へ丸めず元class/exitのまま扱う
/// （事後のみ「but failed to confirm」文面を付与——rename呼出が成功済みの
/// 可能性があるため。add session DD-14と同一pattern）
pub fn run_session(
    backend: &dyn ZellijBackend,
    name: &str,
    new_name: &str,
    json: bool,
) -> Result<(), ZelperError> {
    // app層Usage検査（§2.2/§2.3: 無駄なbackend呼出をしない。先頭'-'はclapが
    // 既に拒否済みのため検査しない。空白含み名はzellijが受理する（M95-C3）が
    // parse_sessionsが先頭wordのみを名前化するためlist_sessionsで復元不能
    // （postcondition必敗）——世界modelで表現可能な名前のみ許可する。それ以外の
    // 文字種は推測で事前拒否せず、すり抜けはpostconditionが最終防衛）
    if new_name.is_empty() {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "NEW_NAME must not be empty",
        ));
    }
    if new_name.contains('/') {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "NEW_NAME must not contain '/'",
        ));
    }
    if new_name.contains(char::is_whitespace) {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "NEW_NAME must not contain whitespace",
        ));
    }
    if name == new_name {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "NEW_NAME must differ from NAME",
        ));
    }
    // 事前一覧（§2.1: 失敗は元class/exitのまま伝播・0件へ丸めない）
    let sessions = backend.list_sessions()?;
    let Some(target) = sessions.iter().find(|s| s.name == name) else {
        // 不在はNoTarget（§2.1）。candidates=全行のname（EXITED込み——agentの
        // 修正導線をremove sessionと同一にする）。行0件は候補列でなく
        // 「no zellij sessions found」文面。単一対象のため単数形文面
        if sessions.is_empty() {
            return Err(ZelperError::new(
                ErrorClass::NoTarget,
                "no zellij sessions found",
            ));
        }
        return Err(ZelperError::with_candidates(
            ErrorClass::NoTarget,
            format!("session '{name}' not found"),
            sessions.iter().map(|s| s.name.clone()).collect(),
        ));
    };
    // EXITED行はrename不可（§2.4: server不在へのaction系呼び出しは失敗のみ
    // 〔M95-A/D実測〕のためfail-fast。存在確認〔行にあるか〕と実行可否判定
    // 〔runningか〕は別個の検査）
    if target.exited {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "cannot rename an EXITED (dead) session '{name}' (attach to resurrect it first)"
            ),
        ));
    }
    // NEW_NAME衝突（§2.2: EXITED込みで判定。既存名へのrenameはzellijがrc=0で
    // 黙って失敗する〔M4 I〕ため、事前検査が唯一の防衛）
    if sessions.iter().any(|s| s.name == new_name) {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            format!("session already exists: '{new_name}'"),
        ));
    }
    backend.rename_session(name, new_name)?;
    // 事後一覧（§2.5: 失敗は元class/exitを保持したままmessageへ文脈付与）
    let after = backend.list_sessions().map_err(|e| {
        ZelperError::new(
            e.class().clone(),
            format!(
                "renamed session '{name}' but failed to confirm: {}",
                e.message()
            ),
        )
    })?;
    // postcondition（§2.5）: (1) 旧名行の消失（RUNNING/EXITED問わず）
    // (2) 新名行が存在しexited == false。黙失敗の現れ方が複数（衝突黙失敗は
    // 旧名消失・新名不出現、no-op黙失敗は旧名残存、等）なため両条件必須
    let old_gone = !after.iter().any(|s| s.name == name);
    let new_running = after.iter().any(|s| s.name == new_name && !s.exited);
    if old_gone && new_running {
        if json {
            println!(
                "{}",
                crate::output::json::ok(
                    serde_json::json!({ "renamed_session": name, "name": new_name })
                )
            );
        } else {
            println!("renamed session '{name}' -> '{new_name}'");
        }
        Ok(())
    } else {
        Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!("rename of session '{name}' did not take effect (unchanged in list-sessions)"),
        ))
    }
}
