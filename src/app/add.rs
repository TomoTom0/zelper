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
    run_session_with(backend, name, None, None, None, json)
}

/// layout/cwd/設定既定つきのadd session（TASK-1）。layoutの優先順位は
/// `--layout` > 設定file `[session] default_layout` > 未指定（zellij自身の
/// default_layout設定に委ねる）。いずれもNAME検査の後・一覧取得より前にfail-fast
pub fn run_session_with(
    backend: &dyn ZellijBackend,
    name: &str,
    layout: Option<&str>,
    cwd: Option<&std::path::Path>,
    config_layout: Option<&str>,
    json: bool,
) -> Result<(), ZelperError> {
    // app層Usage検査（TASK-96 §2.2-§2.4: 一覧取得より前にfail-fast）。空白含み名は
    // zellijが受理する（M96-A実測）がparse_sessionsがlist-sessions行の先頭wordのみを
    // 名前化するため世界modelで復元不能（postcondition必敗・session残留）。
    // 空文字・'/'含みはzellijが明示拒否する（M96-B/C実測）がrename NEW_NAMEとの
    // 対称性・fail-fastでwrap側でも事前検査。先頭'-'は`--`通過系列が`--`挿入なしの
    // attach -b NAME組み立てへ到達するため（有効option一致値の下流挙動は未測定・
    // zellij側option集合はzeller管理外）入力段の確定拒否が唯一のzeller側完結手段。
    // 検査順は empty → '/' → whitespace → 先頭'-'（複合違反時の優先順位固定）。
    // それ以外の文字種は推測で事前拒否せず、すり抜けはpostconditionが最終防衛
    if name.is_empty() {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "NAME must not be empty",
        ));
    }
    if name.contains('/') {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "NAME must not contain '/'",
        ));
    }
    if name.contains(char::is_whitespace) {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "NAME must not contain whitespace",
        ));
    }
    if name.starts_with('-') {
        return Err(ZelperError::new(
            ErrorClass::Usage,
            "NAME must not start with '-'",
        ));
    }
    let opts = resolve_create_opts(layout, cwd, config_layout)?;
    // 事前確認: 同名running sessionの存在はPreflightでfail-fast拒否する
    // （zellijのrc=1「Session already exists」を待たない）。EXITED行はrunningと
    // みなさない（EXITED同名はzellijへ委ねる）。一覧取得の失敗は0件へ丸めず
    // 元class/exitのまま伝播する（TASK-93 D5: 従来のrc=1一律0件扱い〔F7〕を
    // backendの0件decode〔D1〕で解消し、真のbackend障害を表面化させる）
    let sessions = backend.list_sessions()?;
    if sessions.iter().any(|s| !s.exited && s.name == name) {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            format!("session already exists: '{name}'"),
        ));
    }
    backend.create_session_detached(name, &opts)?;
    // postcondition: 作成後のlist_sessionsにNAMEがrunning出現（DD-11 addの存在
    // 確認規則に準拠。作成成否をexit codeのみで信用しない）。0-session環境の
    // 作成直後はbackendの0件decode（D1）によりOk(空一覧)が返るため、作成rc=0
    // かつ一覧0件はpostcondition不在としてVerificationFailedへ分類される
    // （REVIEW-2指摘5）。事後確認の失敗は作成呼出（attach -b）が成功済みの
    // 可能性があるため、元class/exitを保持したままmessageへ文脈を付与する
    // （D5: created session 'NAME' but failed to confirm: <元message>）
    let sessions = backend.list_sessions().map_err(|e| {
        ZelperError::new(
            e.class().clone(),
            format!(
                "created session '{name}' but failed to confirm: {}",
                e.message()
            ),
        )
    })?;
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

/// layout/cwd指定の解決・検査（TASK-1）。
/// - layout: 空文字・先頭`-`はUsage。'/'含みまたは`.kdl`終端はpath扱いで絶対化し
///   存在必須（cwd変更後の相対path誤解決を防ぐ）。それ以外はlayout名としてzellijへ
///   渡す（探索・存在判定はzellij側。不在はzellijがrc=1でsession未作成のまま失敗）
/// - cwd: 既存directory必須（zellijは不在cwdを黙って受理するため事前検査）。絶対化する
pub fn resolve_create_opts(
    layout: Option<&str>,
    cwd: Option<&std::path::Path>,
    config_layout: Option<&str>,
) -> Result<crate::zellij::SessionCreateOpts, ZelperError> {
    let layout = layout.or(config_layout);
    let layout = match layout {
        None => None,
        Some(l) => {
            if l.is_empty() {
                return Err(ZelperError::new(
                    ErrorClass::Usage,
                    "layout must not be empty",
                ));
            }
            if l.starts_with('-') {
                return Err(ZelperError::new(
                    ErrorClass::Usage,
                    "layout must not start with '-'",
                ));
            }
            if l.contains('/') || l.ends_with(".kdl") {
                let p = std::fs::canonicalize(l).map_err(|e| {
                    ZelperError::new(
                        ErrorClass::Usage,
                        format!("layout file '{l}' is not readable: {e}"),
                    )
                })?;
                Some(p.to_string_lossy().into_owned())
            } else {
                Some(l.to_string())
            }
        }
    };
    let cwd = match cwd {
        None => None,
        Some(c) => {
            let p = std::fs::canonicalize(c).map_err(|e| {
                ZelperError::new(
                    ErrorClass::Usage,
                    format!("cwd '{}' is not accessible: {e}", c.display()),
                )
            })?;
            if !p.is_dir() {
                return Err(ZelperError::new(
                    ErrorClass::Usage,
                    format!("cwd '{}' is not a directory", c.display()),
                ));
            }
            Some(p)
        }
    };
    Ok(crate::zellij::SessionCreateOpts { layout, cwd })
}
