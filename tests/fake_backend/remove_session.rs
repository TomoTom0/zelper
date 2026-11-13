// L2: remove session（fake backend。remove-session.tomlのL2層5条件。TASK-84
// 設計 tmp/tasks/tm-84/design_remove-session.md §2/§7）。
// CLI表面（selector排他のclap段・exit code実出力・env〔ZELLIJ_SESSION_NAME〕・
// stderr warning行・FAKE_LOG argv列・plan/報告の実出力）はL3層
// tests/cli/list_read_send.rs
mod fake;
use fake::FakeBackend;
use zelper::app::remove::run_session;
use zelper::domain::*;
use zelper::error::ErrorClass;

fn sess(name: &str, exited: bool) -> SessionRef {
    SessionRef {
        name: name.into(),
        created: Some("1m ago".into()),
        current: false,
        exited,
    }
}

fn names(vals: &[&str]) -> Vec<String> {
    vals.iter().map(|s| s.to_string()).collect()
}

fn delete_calls(b: &FakeBackend) -> Vec<String> {
    b.calls()
        .into_iter()
        .filter(|c| c.starts_with("delete-session "))
        .collect()
}

/// session削除の常時確認gate（§2.5）: 対象数・selector種別によらず
/// --yes / --dry-run なしの実行はPreflight（exit 7）で拒否される（単一NAME指定
/// でもgate。session削除は全pane・tab・resurrection dataを不可逆に巻き込む）。
/// --dry-runはgateを通過し状態変更を行わない
// [covers:remove-session.always-requires-yes-or-dry-run]
#[test]
fn single_name_still_requires_yes_or_dry_run() {
    // (i) 単一NAME + gate違反: Preflight 7
    let b = FakeBackend::new(vec![], vec![]);
    let err = run_session(
        &b,
        &names(&["fake"]),
        false,
        false,
        false,
        false,
        false,
        false,
    )
    .unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::Preflight,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 7, "message: {}", err.message());
    assert!(delete_calls(&b).is_empty(), "calls: {:?}", b.calls());

    // (ii) --dry-runはgate通過・状態変更なし
    let b = FakeBackend::new(vec![], vec![]);
    run_session(
        &b,
        &names(&["fake"]),
        false,
        false,
        false,
        false,
        true,
        false,
    )
    .unwrap();
    assert!(delete_calls(&b).is_empty(), "calls: {:?}", b.calls());

    // (iii) --all selectorでもgate違反は同様（selector種別によらず常時）
    let b = FakeBackend::new(vec![], vec![]);
    let err = run_session(&b, &[], true, false, false, false, false, false).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::Preflight,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 7, "message: {}", err.message());
    assert!(delete_calls(&b).is_empty(), "calls: {:?}", b.calls());
}

/// postconditionの消失確認（§2.11）: delete-session呼出がrc=0でも実行後の
/// list_sessionsに対象行が残存する系列はper-target失敗（still listed）として
/// 報告する（delete-all系のrc=0無操作実測が示すとおりexit codeのみで信用しない）。
/// 単一target全失敗のため全体はOperationFailed（exit 5）
// [covers:remove-session.postcondition-still-listed-is-failure]
#[test]
fn still_listed_session_is_reported_as_failure() {
    let b = FakeBackend::new(vec![], vec![]);
    b.mute_delete_session();
    let err = run_session(
        &b,
        &names(&["fake"]),
        false,
        false,
        false,
        true,
        false,
        false,
    )
    .unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::OperationFailed,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 5, "message: {}", err.message());
    let data = err.data().expect("per-target results in error.data");
    let results = data["results"].as_array().unwrap();
    assert_eq!(results.len(), 1, "results: {results:?}");
    assert_eq!(results[0]["target"], "fake");
    assert_eq!(results[0]["ok"], false);
    assert!(
        results[0]["error"]
            .as_str()
            .unwrap_or_default()
            .contains("still listed"),
        "results: {results:?}"
    );
    // delete-session呼出自体はrc=0成功扱いで発行済み
    assert_eq!(
        delete_calls(&b),
        vec!["delete-session fake -f".to_string()],
        "calls: {:?}",
        b.calls()
    );
}

/// 事後一覧取得失敗のclass保持伝播（§2.11）: 全target実行後のlist_sessions失敗は
/// 元class/exitを保持したままmessageへ文脈（but failed to confirm）を付与する。
/// OperationFailed（exit 5）とZellijUnavailable（exit 4）の両注入でclass/exit保持
/// を検証する（class保持が本質——add session DD-14のpatternに倣う）
// [covers:remove-session.postcondition-list-failure-preserves-class]
#[test]
fn postcheck_list_failure_preserves_class_with_context() {
    // OperationFailed（既定注入class）: exit 5保持
    let b = FakeBackend::new(vec![], vec![]);
    b.fail_list_sessions_after_delete();
    let err = run_session(
        &b,
        &names(&["fake"]),
        false,
        false,
        false,
        true,
        false,
        false,
    )
    .unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::OperationFailed,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 5, "message: {}", err.message());
    let msg = err.message();
    assert!(msg.contains("but failed to confirm"), "message: {msg}");
    assert!(msg.contains("injected failure"), "message: {msg}");
    assert_eq!(
        delete_calls(&b),
        vec!["delete-session fake -f".to_string()],
        "calls: {:?}",
        b.calls()
    );

    // ZellijUnavailable注入でもclass/exit保持と文脈付与は同一
    let b = FakeBackend::new(vec![], vec![]);
    b.fail_list_sessions_after_delete();
    b.inject_failure_as(ErrorClass::ZellijUnavailable);
    let err = run_session(
        &b,
        &names(&["fake"]),
        false,
        false,
        false,
        true,
        false,
        false,
    )
    .unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::ZellijUnavailable,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 4, "message: {}", err.message());
    let msg = err.message();
    assert!(msg.contains("but failed to confirm"), "message: {msg}");
    assert!(msg.contains("injected failure"), "message: {msg}");
    assert_eq!(
        delete_calls(&b),
        vec!["delete-session fake -f".to_string()],
        "calls: {:?}",
        b.calls()
    );
}

/// NAME重複の除去（§2.0）: NAME...に同一名称が複数回現れる場合は1回だけ実行し
/// （重複除去）・報告も1回。実行順序は指定順（最初に出現した位置）を保持する
// [covers:remove-session.duplicate-names-deduplicated]
#[test]
fn duplicate_names_are_deduplicated_in_given_order() {
    let b = FakeBackend::new(vec![], vec![]);
    b.set_sessions(vec![sess("a", false), sess("b", false), sess("c", false)]);
    run_session(
        &b,
        &names(&["a", "c", "a", "b", "c"]),
        false,
        false,
        false,
        true,
        false,
        false,
    )
    .unwrap();
    // 指定順（a, c, b）で各1回のみ。重複分の重複呼出なし
    assert_eq!(
        delete_calls(&b),
        vec![
            "delete-session a -f".to_string(),
            "delete-session c -f".to_string(),
            "delete-session b -f".to_string(),
        ],
        "calls: {:?}",
        b.calls()
    );
    // 全対象がlist_sessions状態から消えている（postcondition通過・報告1回分の対象集合）
    let remaining: Vec<String> = b
        .state
        .borrow()
        .sessions
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert!(remaining.is_empty(), "remaining: {remaining:?}");
}

/// EXITED行の受理（§2.4）: 明示NAME指定でlist_sessionsのEXITED行に一致する名称は
/// 削除対象として受理され（NoTargetにならない）・実行はRUNNINGと同一の
/// delete-session <name> -f 1呼び出し（状態分岐なし）。dry-runでも受理されるが
/// 実行は不発
// [covers:remove-session.exited-name-accepted-as-target]
#[test]
fn exited_name_is_accepted_and_removed_via_dead_path() {
    // (i) 実行: EXITED行を受理しdelete-session 1回・postcondition通過
    let b = FakeBackend::new(vec![], vec![]);
    b.set_sessions(vec![sess("live", false), sess("dead1", true)]);
    run_session(
        &b,
        &names(&["dead1"]),
        false,
        false,
        false,
        true,
        false,
        false,
    )
    .unwrap();
    assert_eq!(
        delete_calls(&b),
        vec!["delete-session dead1 -f".to_string()],
        "calls: {:?}",
        b.calls()
    );
    let remaining: Vec<String> = b
        .state
        .borrow()
        .sessions
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(
        remaining,
        vec!["live".to_string()],
        "remaining: {remaining:?}"
    );

    // (ii) dry-run: 受理されるが実行不発・状態不変
    let b = FakeBackend::new(vec![], vec![]);
    b.set_sessions(vec![sess("live", false), sess("dead1", true)]);
    run_session(
        &b,
        &names(&["dead1"]),
        false,
        false,
        false,
        false,
        true,
        false,
    )
    .unwrap();
    assert!(delete_calls(&b).is_empty(), "calls: {:?}", b.calls());
    assert_eq!(b.state.borrow().sessions.len(), 2);
}
