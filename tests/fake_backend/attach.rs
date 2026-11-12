// L2: attach / detach / add session一覧取得失敗系（fake backend。attach-detach.toml
// のL2層条件。TASK-89・設計§5.2: TTY検査はrun_attach_innerのtty_ok引数注入）
// run_interactive経路（stdio inherit・timeout無効・非zero終了mapping）は
// src/zellij/process.rs内test・L3（実binary・shim）系はtests/cli/list_read_send.rs
mod fake;
use fake::FakeBackend;
use zelper::app::add::run_session;
use zelper::app::attach::{detach_report, run_attach_inner, run_detach};
use zelper::domain::*;
use zelper::error::ErrorClass;

fn pane(id: u32, title: &str) -> PaneState {
    PaneState {
        id: PaneKindId::Terminal(id),
        title: title.to_string(),
        is_selectable: true,
        is_floating: false,
        is_focused: false,
        exited: false,
        is_held: false,
        geometry: Geometry {
            x: 0,
            y: 0,
            rows: 10,
            cols: 10,
        },
        command: None,
        terminal_command: None,
        cwd: None,
        tab_id: TabId(0),
        tab_position: 0,
        tab_name: "T1".into(),
        plugin_url: None,
    }
}

// ---------------------------------------------------------------------------
// 実出力観測helper（TASK-89 REVIEW-2指摘6）。repoにL2のstdout捕捉慣例がない
// ため追加した。libtestはprintln!をtest実行threadだけでなくspawn threadでも
// captureするため、関数呼び出しによるstdout観測はsame-processでは不可能。
// そこでattach_success_output_probe（#[ignore]）を自test binaryのchild process
// （--exact --ignored --nocapture）として実行し、sentinel行間の実出力を観測する

/// attach-successの実出力観測probe（self re-exec専用。通常のcargo testでは
/// #[ignore]のためskipされる）。PROBE_JSON/PROBE_RESULT_FILE環境変数で親と通信
// [covers:attach-detach.attach-success]
#[test]
#[ignore]
fn attach_success_output_probe() {
    let json = std::env::var("PROBE_JSON")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    let b = FakeBackend::new(vec![pane(1, "a")], vec![]);
    b.arm_stdout_probe();
    println!("__PROBE_BEGIN__");
    let r = run_attach_inner(&b, "work", json, true);
    println!("__PROBE_END__");
    r.unwrap();
    if let Some(p) = std::env::var_os("PROBE_RESULT_FILE") {
        // 観測結果: 1行目=backend呼出時点のstdout内容（sentinel行は観測対象外
        // のためfd 1内容から差し引く）・2行目=attach呼出回数
        let raw_at = b.stdout_at_attach().unwrap_or_default();
        // fd 1にはlibtest banner（--nocapture時）+ sentinel行までが書かれている。
        // 観測対象はsentinel行より後（backend呼出までのアプリ出力の有無）
        let at = raw_at
            .rsplit_once("__PROBE_BEGIN__\n")
            .map(|(_, after)| after.to_string())
            .unwrap_or(raw_at);
        std::fs::write(
            p,
            format!(
                "{}\n{}",
                at,
                b.calls().iter().filter(|c| *c == "attach").count()
            ),
        )
        .unwrap();
    }
}

/// probeをchild processで実行し、(sentinel間の実stdout出力, backend呼出時点の
/// stdout内容, attach呼出回数)を返す
fn run_output_probe(json: bool) -> (String, String, usize) {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "zelper-fake-attach-probe-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let out_path = dir.join("out.log");
    let result_path = dir.join("result.txt");
    let out_file = std::fs::File::create(&out_path).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
            "attach_success_output_probe",
        ])
        .env("PROBE_JSON", if json { "1" } else { "" })
        .env("PROBE_RESULT_FILE", &result_path)
        .stdout(std::process::Stdio::from(out_file))
        .stderr(std::process::Stdio::null())
        .status()
        .expect("failed to spawn probe child");
    assert!(status.success(), "probe child failed: {status}");
    let raw = std::fs::read_to_string(&out_path).unwrap();
    let begin = raw
        .find("__PROBE_BEGIN__\n")
        .expect("begin sentinel not found");
    let end = raw.rfind("__PROBE_END__").expect("end sentinel not found");
    let segment = raw[begin + "__PROBE_BEGIN__\n".len()..end].to_string();
    let result = std::fs::read_to_string(&result_path).unwrap();
    let (at_attach, calls) = result.split_once('\n').expect("result file format");
    (segment, at_attach.to_string(), calls.parse().unwrap())
}

/// attach成功経路の報告契約: attach_session()の復帰（対話client終了）後にのみ
/// 出力を行い、対話中は一切出力しない（stdoutはzellijが占有）。humanは最小
/// サマリ1行・JSONはenvelope 1通（data.session=NAME）。検証は実出力
/// （self re-exec childのstdout捕捉）で行う: backend呼出時点の出力0
/// （arm_stdout_probe）・sentinel間の実出力が1通のみ・attach呼出回数は厳密に
/// 1回（REVIEW-2指摘6）
// [covers:attach-detach.attach-success]
#[test]
fn attach_success_reports_after_client_end() {
    // human: sentinel間の実出力は `attach session '<NAME>' ended` の1行のみ
    let (seg, at_attach, calls) = run_output_probe(false);
    assert_eq!(seg, "attach session 'work' ended\n", "captured: {seg:?}");
    assert_eq!(at_attach, "", "backend復帰前の出力が存在する");
    assert_eq!(calls, 1, "attach呼出回数");

    // JSON: sentinel間の実出力はenvelope 1通のみでdata.session=NAME
    let (seg, at_attach, calls) = run_output_probe(true);
    assert_eq!(seg.trim_end().lines().count(), 1, "captured: {seg:?}");
    let v: serde_json::Value = serde_json::from_str(seg.trim_end()).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["session"], "work");
    assert_eq!(at_attach, "", "backend復帰前の出力が存在する");
    assert_eq!(calls, 1, "attach呼出回数");
}

/// detachのno-op成功契約: zellijのaction detachはclient不在でもrc=0 silent
/// （M3 (b)実測）のため、zellerはclient数を検査せずrc=0（Ok）を無条件で成功
/// 扱いする。単一client切断（M3 (a): client exit 0・session RUNNING継続）と
/// client不在はzellerから同一に見え、human 1行サマリ/JSON envelopeで報告する
/// （human/JSONの実出力はL3 detach-argv条件が実binary stdoutで検証するため、
/// ここではbackend呼出契約・呼出回数・報告文面を検証する）
// [covers:attach-detach.detach-no-client-noop-success]
#[test]
fn detach_no_client_is_noop_success() {
    let b = FakeBackend::new(vec![pane(1, "a")], vec![]);
    // FakeBackendのdetach_sessionは常にOk（client不在とclient存在が区別されない
    // ことをそのまま模擬——zeller側にclient数依存の分岐がないことの検証）
    run_detach(&b, "work", false).unwrap();
    run_detach(&b, "work", true).unwrap();
    // 呼出回数: human/JSONの2回実行に対しdetach呼出は厳密に2回
    assert_eq!(
        b.calls().iter().filter(|c| *c == "detach").count(),
        2,
        "calls: {:?}",
        b.calls()
    );
    // 報告文面: human `detached clients from session '<NAME>'` の1行・
    // JSONはenvelope 1通でdata.session=NAME
    assert_eq!(
        detach_report("work", false),
        "detached clients from session 'work'"
    );
    let v: serde_json::Value = serde_json::from_str(&detach_report("work", true)).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["session"], "work");
}

// ---------------------------------------------------------------------------
// add session: 一覧取得失敗の伝播（TASK-93 D5。fail_op / postcheck armで失敗注入）

/// add sessionの事前一覧取得失敗: list_sessionsの真の失敗（rc=1 marker不一致等）は
/// 0件へ丸めず元class/exitのまま伝播し、作成呼出（attach -b）は不発になる
/// （unwrap_or_default妥協〔F7〕の解消）。OperationFailed（exit 5）と
/// ZellijUnavailable（exit 4）の両注入でclass/exit保持を検証する
/// （OperationFailed特化でなくclass保持が本質——TASK-93レビュー指摘1）
// [covers:attach-detach.add-session-precheck-list-failure-propagates]
#[test]
fn add_session_precheck_list_failure_propagates() {
    let b = FakeBackend::new(vec![pane(1, "a")], vec![]);
    b.inject_failure("list-sessions");
    let err = run_session(&b, "work", true).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::OperationFailed,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 5, "message: {}", err.message());
    assert!(
        err.message()
            .contains("injected failure at 'list-sessions'"),
        "message: {}",
        err.message()
    );
    // 作成呼出（attach -b）は不発
    assert!(
        !b.calls().iter().any(|c| c.starts_with("attach -b")),
        "calls: {:?}",
        b.calls()
    );

    // ZellijUnavailable（exit 4）注入でもclass/exitを丸めず伝播する
    let b = FakeBackend::new(vec![pane(1, "a")], vec![]);
    b.inject_failure("list-sessions");
    b.inject_failure_as(ErrorClass::ZellijUnavailable);
    let err = run_session(&b, "work", true).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::ZellijUnavailable,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 4, "message: {}", err.message());
    assert!(
        err.message()
            .contains("injected failure at 'list-sessions'"),
        "message: {}",
        err.message()
    );
    assert!(
        !b.calls().iter().any(|c| c.starts_with("attach -b")),
        "calls: {:?}",
        b.calls()
    );
}

/// add sessionの事後一覧取得失敗: 作成呼出（attach -b）は成功済みのため、確認失敗は
/// 元class/exitを保持しつつmessageへ文脈を付与する（D5: created session 'NAME'
/// but failed to confirm: <元message>）。OperationFailed（exit 5）と
/// ZellijUnavailable（exit 4）の両注入でclass/exit保持と文脈付与を検証する
/// （TASK-93レビュー指摘1）
// [covers:attach-detach.add-session-postcheck-list-failure-keeps-class]
#[test]
fn add_session_postcheck_list_failure_keeps_class_with_context() {
    let b = FakeBackend::new(vec![pane(1, "a")], vec![]);
    b.fail_list_sessions_after_create();
    let err = run_session(&b, "work", true).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::OperationFailed,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 5, "message: {}", err.message());
    let msg = err.message();
    assert!(
        msg.contains("created session 'work' but failed to confirm"),
        "message: {msg}"
    );
    assert!(msg.contains("injected failure"), "message: {msg}");
    // 作成呼出自体は成功済み（rc=0・作成NAMEは状態へ反映されている）
    assert!(
        b.calls().iter().any(|c| c == "attach -b work"),
        "calls: {:?}",
        b.calls()
    );

    // ZellijUnavailable（exit 4）注入でもclass/exit保持と文脈付与は同一
    let b = FakeBackend::new(vec![pane(1, "a")], vec![]);
    b.fail_list_sessions_after_create();
    b.inject_failure_as(ErrorClass::ZellijUnavailable);
    let err = run_session(&b, "work", true).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::ZellijUnavailable,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 4, "message: {}", err.message());
    let msg = err.message();
    assert!(
        msg.contains("created session 'work' but failed to confirm"),
        "message: {msg}"
    );
    assert!(msg.contains("injected failure"), "message: {msg}");
    assert!(
        b.calls().iter().any(|c| c == "attach -b work"),
        "calls: {:?}",
        b.calls()
    );
}
