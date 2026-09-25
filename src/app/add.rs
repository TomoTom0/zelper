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

/// add session NAME: detached session作成（DD-14・TASK-89）。main.rsからは
/// 0-session環境で動く前提のprobe backend（session解決なし）が渡される
/// （AddTarget::Session例外dispatch。設計§2.2）
pub fn run_session(backend: &dyn ZellijBackend, name: &str, json: bool) -> Result<(), ZelperError> {
    // 事前確認: 同名running sessionの存在はPreflightでfail-fast拒否する
    // （zellijのrc=1「Session already exists」を待たない）。EXITED行はrunningと
    // みなさない（EXITED同名はzellijへ委ねる）。0-session環境ではlist-sessions
    // 自体がrc=1で失敗する（M5 (0)実測）ため、その失敗は「0件」として扱い作成へ
    // 進む——同名衝突はzellij本体のattach -bがrc=1で拒むため安全側
    let sessions = backend.list_sessions().unwrap_or_default();
    if sessions.iter().any(|s| !s.exited && s.name == name) {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            format!("session already exists: '{name}'"),
        ));
    }
    backend.create_session_detached(name)?;
    // postcondition: 作成後のlist_sessionsにNAMEがrunning出現（DD-11 addの存在
    // 確認規則に準拠。作成成否をexit codeのみで信用しない）。0-session環境の
    // list-sessions rc=1（No active zellij sessions found.）も事前確認と同一に
    // 「0件」として扱う（REVIEW-2指摘5: 作成rc=0かつ一覧0件はpostcondition不在
    // としてVerificationFailedへ分類する。一般backend障害との区別に使える情報が
    // error文面に無いため rc=1 は一律0件扱いとする）
    let sessions = backend.list_sessions().unwrap_or_default();
    if sessions.iter().any(|s| !s.exited && s.name == name) {
        println!("{}", session_report(name, json));
        Ok(())
    } else {
        Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!("session '{name}' not found in list-sessions after creation"),
        ))
    }
}

/// add session成功の報告文。humanは `created session '<NAME>' (detached)` の1行・
/// JSONはenvelope 1通（data.session=NAME・data.created=true。DD-14）
pub fn session_report(name: &str, json: bool) -> String {
    if json {
        output::json::ok(serde_json::json!({ "session": name, "created": true }))
    } else {
        format!("created session '{name}' (detached)")
    }
}
