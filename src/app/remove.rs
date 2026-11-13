use crate::domain::{PaneKindId, SessionRef, TabId};
use crate::error::{ErrorClass, ZelperError};
use crate::output;
use crate::zellij::ZellijBackend;

/// 破壊的複数対象の確認gate（DD-12）: --yes or --dry-run が必要。
/// always=true（remove session・TASK-84設計§2.5）は対象数・modeによらず常時gate
/// （session削除は最小構成でも全pane・tab・resurrection dataを不可逆に巻き込む
/// ため単一でもgate。remove tab --emptyの単一gate先例と同じ理由）。pane/tab経路
/// （always=false）の挙動は不変
fn destructive_gate(
    n_targets: usize,
    empty_mode: bool,
    yes: bool,
    dry_run: bool,
    always: bool,
) -> Result<(), ZelperError> {
    if yes && dry_run {
        eprintln!("zelper: note: --yes is ignored when --dry-run is given");
    }
    let gated = always || n_targets > 1 || empty_mode;
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
    destructive_gate(targets.len(), false, yes, dry_run, false)?;

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
    report(results, |r| format!("removed pane {}", r.target), json)
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
    destructive_gate(targets.len(), empty, yes, dry_run, false)?;

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
    report(results, |r| format!("removed tab {}", r.target), json)
}

/// per-target結果の報告（DD-11 remove）。humanの成功行文面はcallerがok_lineで
/// 組む（pane/tabは "{verb} {target}" 固定。sessionはstate表示〔§2.12最小拡張〕）。
/// gate/dry-run/失敗時envelope構造は不変
fn report(
    results: Vec<output::json::TargetedResult<String>>,
    ok_line: impl Fn(&output::json::TargetedResult<String>) -> String,
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
                println!("{}", ok_line(r));
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

/// remove session（TASK-84設計§2）。selectorは NAME...（1件以上）/ --all /
/// --dead / --me の4者のうちちょうど1つ（app層exactly-one検証——clap pairwise
/// conflictsと二段構え。moveの三者排他と同一pattern）。意味論は単一の完全削除
/// （session行もresurrection dataも消す。backendは常に `zellij delete-session
/// NAME -f` 1呼び出し・状態分岐なし）。
///
/// - 常時gate（§2.5: 対象数・selector種別によらず--yes/--dry-run必須）
/// - self検出と保護（§2.3: NAME経路は拒否・--allは除外+stderr warning・--meは
///   ZELLIJ_SESSION_NAME解決で明示許可）
/// - EXITED行は削除対象として受理（§2.4: 存在確認はlist_sessionsのEXITED込み）
/// - dry-runはplan形式（§2.6: dead対象はlist_tabs_for不呼出でtabs/panes=null）
/// - 事後list_sessions消失確認（§2.11: 残存はper-target失敗・一覧取得失敗は
///   元class保持で「but failed to confirm」文面）
/// - 部分失敗は継続実行+per-target報告（§2.12: VerificationFailed不使用）
// signatureはTASK-84テスト先行工程で確定済み（selector 3 flag + gate 2 + json）
#[allow(clippy::too_many_arguments)]
pub fn run_session(
    backend: &dyn ZellijBackend,
    names: &[String],
    all: bool,
    dead: bool,
    me: bool,
    yes: bool,
    dry_run: bool,
    json: bool,
) -> Result<(), ZelperError> {
    // app層exactly-one検証（§2.0）: clap pairwise conflictsは併用のみ検出する
    // ため、0個（NAME省略かつflagなし）の最終検証はここ
    let selectors = [!names.is_empty(), all, dead, me];
    if selectors.iter().filter(|b| **b).count() != 1 {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "exactly one session selector is required: NAME... / --all / --dead / --me",
        ));
    }
    // --me解決（§2.3）: ZELLIJ_SESSION_NAMEのみ（zellijが自paneへ注入する値。
    // --session flagはsession外からも指定可能で自sessionの証拠にならない）。
    // env不在はdry-runでもplan表示に解決名が必要なためPreflight
    let me_name: Option<String> = std::env::var("ZELLIJ_SESSION_NAME")
        .ok()
        .filter(|s| !s.is_empty());
    if me && me_name.is_none() {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            "remove session --me requires running inside a zellij session (ZELLIJ_SESSION_NAME is not set)",
        ));
    }
    // 事前一覧（EXITED込み = 存在確認基準。0-session環境はbackendの0件decodeでOk(空)）
    let sessions = backend.list_sessions()?;
    // 対象集合解決（state持ち回り: exited=dry-run/報告表示・current=--me注記）
    let mut targets: Vec<(String, bool, bool)> = Vec::new(); // (name, exited, current)
    if me {
        // §2.3: env設定時も他NAME対象と同一の事前存在確認（不在はNoTarget）
        let name = me_name.clone().expect("checked above");
        match sessions.iter().find(|s| s.name == name) {
            Some(s) => targets.push((name, s.exited, true)),
            None => return Err(session_not_found(&sessions, &[name])),
        }
    } else if dead {
        // §2.2: EXITED行のみ
        targets = sessions
            .iter()
            .filter(|s| s.exited)
            .map(|s| (s.name.clone(), true, false))
            .collect();
        if targets.is_empty() {
            return Err(ZelperError::new(
                ErrorClass::NoTarget,
                "no sessions to remove",
            ));
        }
    } else if all {
        // §2.2/§2.3: 全行（EXITED込み）からself除外+stderr warning通知
        // （dry-runにも反映——除外行は[plan]でなくwarning行）
        for s in &sessions {
            if me_name.as_deref() == Some(s.name.as_str()) {
                eprintln!(
                    "warning: --all excludes the current session '{}' (use 'zelper remove session --me' to remove it)",
                    s.name
                );
                continue;
            }
            targets.push((s.name.clone(), s.exited, false));
        }
        if targets.is_empty() {
            return Err(ZelperError::new(
                ErrorClass::NoTarget,
                if sessions.is_empty() {
                    "no sessions to remove".to_string()
                } else {
                    // 自sessionのみが存在する環境（§2.9表: --me案内文面）
                    format!(
                        "no sessions to remove (only the current session '{}' exists; use 'zelper remove session --me' to remove it)",
                        sessions[0].name
                    )
                },
            ));
        }
    } else {
        // NAME...経路: 重複除去（指定順保持・報告は1回）→存在確認→self検査
        let mut deduped: Vec<&str> = Vec::new();
        for n in names {
            if !deduped.contains(&n.as_str()) {
                deduped.push(n.as_str());
            }
        }
        let missing: Vec<String> = deduped
            .iter()
            .filter(|n| !sessions.iter().any(|s| s.name == **n))
            .map(|n| n.to_string())
            .collect();
        if !missing.is_empty() {
            return Err(session_not_found(&sessions, &missing));
        }
        // self検査（§2.3）: NAMEに自session（ZELLIJ_SESSION_NAME該当名）が含まれる
        // 場合は--meなしでは状態変更前に拒否（fail-fast）
        if let Some(me) = &me_name
            && deduped.contains(&me.as_str())
        {
            return Err(ZelperError::new(
                ErrorClass::Preflight,
                format!(
                    "cannot remove the current session '{me}' without --me; use 'zelper remove session --me'"
                ),
            ));
        }
        targets = deduped
            .into_iter()
            .map(|n| {
                let s = sessions
                    .iter()
                    .find(|s| s.name == n)
                    .expect("checked above");
                (n.to_string(), s.exited, false)
            })
            .collect();
    }
    // 常時gate（§2.5）
    destructive_gate(targets.len(), false, yes, dry_run, true)?;

    // dry-run plan（§2.6）: 状態変更・backend呼出（list系読み取りを除く）は一切なし
    if dry_run {
        let mut plan_targets: Vec<serde_json::Value> = Vec::new();
        let mut lines: Vec<String> = Vec::new();
        for (name, exited, current) in &targets {
            // dead対象はlist_tabs_forを呼ばない（EXITED名へのaction系呼び出しの
            // 副作用が未検証のため。呼んで失敗→nullでなく呼ばない）。running対象の
            // 取得失敗もnull（一覧自体は維持——list sessionsの `-` 表示と同一の緩和）
            let counts = (!exited).then(|| {
                backend.list_tabs_for(name).ok().map(|tabs| {
                    let panes: u32 = tabs
                        .iter()
                        .map(|t| t.selectable_tiled_panes_count + t.selectable_floating_panes_count)
                        .sum();
                    (tabs.len(), panes)
                })
            });
            let state = if *exited { "dead" } else { "running" };
            let mut line = format!("[plan] remove session '{name}' ({state}");
            if let Some(Some((tabs, panes))) = counts {
                line.push_str(&format!(", {tabs} tabs, {panes} panes"));
            }
            line.push(')');
            if *current {
                line.push_str(" [current session]");
            }
            lines.push(line);
            let mut obj = serde_json::json!({
                "name": name,
                "state": state,
                "tabs": counts.flatten().map(|(t, _)| t),
                "panes": counts.flatten().map(|(_, p)| p),
            });
            if *current && let Some(map) = obj.as_object_mut() {
                map.insert("current".to_string(), serde_json::json!(true));
            }
            plan_targets.push(obj);
        }
        if json {
            println!(
                "{}",
                output::json::ok(serde_json::json!({ "plan": { "targets": plan_targets } }))
            );
        } else {
            for l in &lines {
                println!("{l}");
            }
            println!(
                "{} session(s) to remove; pass --yes to execute",
                targets.len()
            );
        }
        return Ok(());
    }

    // --me実行前warning（§2.3自壊時の報告契約: kill後の出力は失われるため、
    // 実行前のstderr行が唯一確実な通知経路。実行・検証・報告は通常経路のまま）
    if let Some((name, _, true)) = targets.iter().find(|t| t.2) {
        eprintln!(
            "warning: removing the current session '{name}'; zelper may be terminated before reporting"
        );
    }

    // per-target順次実行（§2.12: 失敗しても残targetを実行・rollbackしない）。
    // detailへstate marker（dead/current）を持ち回らせ、成功行文面組立に使う
    let mut results: Vec<output::json::TargetedResult<String>> = Vec::new();
    for (name, exited, current) in &targets {
        let detail = if *current {
            Some("current".to_string())
        } else if *exited {
            Some("dead".to_string())
        } else {
            None
        };
        let (ok, errmsg) = match backend.delete_session(name) {
            Ok(()) => (true, None),
            Err(e) => (false, Some(e.message().to_string())),
        };
        results.push(output::json::TargetedResult {
            target: name.clone(),
            ok,
            detail,
            error: errmsg,
        });
    }
    // postcondition（§2.11）: 全target実行後にlist_sessionsを1回取得し全対象行の
    // 消失を確認（delete-session -fがrc=0でも無操作あり得る系のため実測の消失
    // 確認は必須）。残存はper-target失敗（VerificationFailed不使用）へfold。
    // 一覧取得失敗は元class/exitを保持しmessageへ文脈付与（add session DD-14と
    // 同一の「but failed to confirm」pattern）
    let after = backend.list_sessions().map_err(|e| {
        let list = targets
            .iter()
            .map(|(n, _, _)| format!("'{n}'"))
            .collect::<Vec<_>>()
            .join(", ");
        ZelperError::new(
            e.class().clone(),
            format!(
                "removed session(s) {list} but failed to confirm: {}",
                e.message()
            ),
        )
    })?;
    for r in results.iter_mut() {
        if r.ok && after.iter().any(|s| s.name == r.target) {
            r.ok = false;
            r.error = Some(format!("session '{}' still listed after removal", r.target));
        }
    }
    report(
        results,
        |r| {
            let suffix = match r.detail.as_deref() {
                Some("dead") => " (was dead)",
                Some("current") => " (current session)",
                _ => "",
            };
            format!("removed session '{}'{}", r.target, suffix)
        },
        json,
    )
}

/// NAME不在のNoTarget（§2.9）: candidatesは全行のname（EXITED行も含む——
/// 削除対象の存在確認はEXITED込み。DD-1.2の--session事前確認〔running名のみ〕
/// と異なる点）。行0件は候補列出力ではなく「no zellij sessions found」文面
fn session_not_found(sessions: &[SessionRef], missing: &[String]) -> ZelperError {
    if sessions.is_empty() {
        ZelperError::new(ErrorClass::NoTarget, "no zellij sessions found")
    } else {
        ZelperError::with_candidates(
            ErrorClass::NoTarget,
            format!("sessions not found: {missing:?}"),
            sessions.iter().map(|s| s.name.clone()).collect(),
        )
    }
}
