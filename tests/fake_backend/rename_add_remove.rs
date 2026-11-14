// L2: rename / add / remove（fake backend。test-plan §2.5、remove安全gate）
mod fake;
use fake::{FakeBackend, RenamePostInjection};
use zelper::domain::*;
use zelper::error::ErrorClass;

fn pane(id: u32, title: &str, tab: u32, y: u32, x: u32) -> PaneState {
    PaneState {
        id: PaneKindId::Terminal(id),
        title: title.to_string(),
        is_selectable: true,
        is_floating: false,
        is_focused: false,
        exited: false,
        is_held: false,
        geometry: Geometry {
            x,
            y,
            rows: 10,
            cols: 10,
        },
        command: Some("codex".into()),
        terminal_command: None,
        cwd: Some("/w".into()),
        tab_id: TabId(tab),
        tab_position: 0,
        tab_name: "T1".into(),
        plugin_url: None,
    }
}

fn tab(id: u32, tiled: u32) -> TabState {
    TabState {
        id: TabId(id),
        position: id,
        name: format!("tab{id}"),
        active: id == 0,
        selectable_tiled_panes_count: tiled,
        selectable_floating_panes_count: 0,
        are_floating_panes_visible: true,
    }
}

// [covers:rename.pane-rename-call-and-postcondition]
#[test]
fn rename_pane_updates_title_and_verifies() {
    let b = FakeBackend::new(vec![pane(1, "old", 0, 0, 0)], vec![tab(0, 1)]);
    zelper::app::rename::run_pane(&b, "1", "new-name", false).unwrap();
    assert!(
        b.calls()
            .iter()
            .any(|c| c == "rename-pane terminal_1 new-name")
    );
    assert_eq!(b.state.borrow().panes[0].title, "new-name");
}

// [covers:rename.pane-backend-failure-is-operation-failed]
#[test]
fn rename_verification_failure_detected() {
    let b = FakeBackend::new(vec![pane(1, "old", 0, 0, 0)], vec![tab(0, 1)]);
    // rename-paneを失敗させる（stateが変わらない → 検証で捕捉）
    b.inject_failure("rename-pane");
    let err = zelper::app::rename::run_pane(&b, "1", "new-name", false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed); // backend呼び出し自体の失敗
}

// [covers:rename.pane-silent-noop-detected-by-postcondition]
#[test]
fn rename_pane_silent_noop_detected_by_postcondition() {
    let b = FakeBackend::new(vec![pane(1, "old", 0, 0, 0)], vec![tab(0, 1)]);
    // close-paneと違いrenameはexit 0で無視されるケース: fail_opではなくtitleを変えないfakeが必要。
    // FakeBackendは必ずtitleを変えるため、この経路はL4実zellijで検証する（test-plan §2.9）。
    // ここではstate整合を確認
    zelper::app::rename::run_pane(&b, "1", "x", false).unwrap();
    assert_eq!(b.state.borrow().panes.len(), 1);
}

// [covers:add-remove.add-pane-count-and-postcondition]
#[test]
fn add_pane_count_and_postcondition() {
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0, 0)], vec![tab(0, 1)]);
    zelper::app::add::run_pane(&b, None, 2, Some("w"), None, &[], false).unwrap();
    let s = b.state.borrow();
    let titles: Vec<_> = s.panes.iter().map(|p| p.title.clone()).collect();
    assert!(titles.contains(&"w-1".to_string()));
    assert!(titles.contains(&"w-2".to_string()));
    assert_eq!(s.panes.len(), 3);
}

// [covers:add-remove.add-tab-layout-sources-conflict]
#[test]
fn add_tab_layout_conflict_rejected() {
    let b = FakeBackend::new(vec![], vec![tab(0, 1)]);
    let err = zelper::app::add::run_tab(
        &b,
        1,
        None,
        None,
        Some(zelper::zellij::LayoutSpec {
            name: Some("x".into()),
            path: Some("./x.kdl".into()),
            inline: None,
        }),
        &[],
        false,
    )
    .unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Usage);
}

// [covers:add-remove.remove-pane-safety-gate]
#[test]
fn remove_single_pane_no_gate_multi_requires_yes() {
    let b = FakeBackend::new(
        vec![
            pane(1, "a", 0, 0, 0),
            pane(2, "b", 0, 0, 100),
            pane(3, "c", 0, 10, 0),
        ],
        vec![tab(0, 3)],
    );
    zelper::app::remove::run_pane(&b, &["1".to_string()], false, false, false).unwrap();

    let err =
        zelper::app::remove::run_pane(&b, &["2".to_string(), "3".to_string()], false, false, false)
            .unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);

    // --dry-runは実行しない
    zelper::app::remove::run_pane(&b, &["2".to_string()], true, true, false).unwrap();
    assert_eq!(b.state.borrow().panes.len(), 2); // pane 2, 3は残存

    zelper::app::remove::run_pane(&b, &["2".to_string()], true, false, false).unwrap();
    assert_eq!(b.state.borrow().panes.len(), 1); // pane 3のみ残存
}

// [covers:add-remove.remove-empty-unknown-tab-name-is-error-not-silent-widen]
#[test]
fn remove_empty_with_unknown_tab_name_is_error_not_silent_widen() {
    // レビュー回帰: 解決不能なTABSPECは黙って捨てずerror（全空tabへの暗黙拡大防止）
    let b = FakeBackend::new(vec![], vec![tab(0, 0), tab(1, 3)]);
    let err =
        zelper::app::remove::run_tab(&b, &["nosuchtab".to_string()], true, true, false, false)
            .unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);
    // 名前で一意に解決できる場合はその範囲のみ
    let b2 = FakeBackend::new(
        vec![pane(1, "a", 0, 0, 0)],
        vec![
            tab(0, 1),
            TabState {
                id: TabId(1),
                position: 1,
                name: "empty1".into(),
                active: false,
                selectable_tiled_panes_count: 0,
                selectable_floating_panes_count: 0,
                are_floating_panes_visible: true,
            },
        ],
    );
    zelper::app::remove::run_tab(&b2, &["empty1".to_string()], true, true, false, false).unwrap();
    let s = b2.state.borrow();
    assert_eq!(s.tabs.len(), 1);
    assert_eq!(s.tabs[0].id, TabId(0));
}

// [covers:add-remove.remove-empty-selects-only-empty-tabs]
#[test]
fn remove_empty_tabs_selects_only_empty() {
    // tab0: pane 2個（空でない）/ tab1: 空tab
    let b = FakeBackend::new(
        vec![pane(1, "a", 0, 0, 0), pane(2, "b", 0, 0, 100)],
        vec![tab(0, 2), tab(1, 0)],
    );
    zelper::app::remove::run_tab(&b, &[], true, true, true, false).unwrap(); // dry-run
    assert_eq!(b.state.borrow().tabs.len(), 2);
    zelper::app::remove::run_tab(&b, &[], true, true, false, false).unwrap();
    let s = b.state.borrow();
    assert_eq!(s.tabs.len(), 1);
    assert_eq!(s.tabs[0].id, TabId(0));
}

// [covers:add-remove.remove-empty-requires-gate-even-for-single]
#[test]
fn remove_empty_requires_gate_even_for_single() {
    let b = FakeBackend::new(vec![], vec![tab(0, 0), tab(1, 3)]);
    let err = zelper::app::remove::run_tab(&b, &[], true, false, false, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
}

// [covers:add-remove.remove-missing-pane-is-no-target]
#[test]
fn remove_missing_pane_is_no_target() {
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0, 0)], vec![tab(0, 1)]);
    let err =
        zelper::app::remove::run_pane(&b, &["42".to_string()], true, false, false).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::NoTarget);
}

// ---------------------------------------------------------------------------
// rename session（TASK-95・rename.tomlのL2層6条件——postcondition両判定の独立保護
// 〔rename_post_inject〕と不在/EXITED×衝突の検査優先順位test〔2条件tag〕は
// コードレビュー対応で追加。設計
// tmp/tasks/tm-95/design_rename-session.md §2/§2.9/§5実装上の注意）。
// CLI表面（exit code実出力・文面・FAKE_LOG argv列・probe dispatch）はL3層
// tests/cli/list_read_send.rs

fn sess(name: &str, exited: bool) -> SessionRef {
    SessionRef {
        name: name.into(),
        created: Some("1m ago".into()),
        current: false,
        exited,
    }
}

/// 成功時のbackend状態書き換え契約（§2.9）: rename_session呼出成功でsessionsの
/// from行がtoへ書き換わる（exited不変——renameは実行状態を変えない）。呼出記録は
/// `rename-session@<from> <to>` 形式（fail_opのprefix照合で失敗注入可）。
/// 呼出列は事前一覧→rename→事後一覧の3呼用・完全一致で固定（§2.5: 事前1回を
/// 全事前判定に共用するためlist-sessionsはこの3回のみ）
// [covers:rename.session-state-rewrite-on-success]
#[test]
fn rename_session_rewrites_sessions_state_on_success() {
    let b = FakeBackend::new(vec![], vec![]);
    b.set_sessions(vec![sess("old", false), sess("dead1", true)]);
    zelper::app::rename::run_session(&b, "old", "new", false).unwrap();
    assert_eq!(
        b.calls(),
        vec!["list-sessions", "rename-session@old new", "list-sessions"],
        "calls: {:?}",
        b.calls()
    );
    let s = b.state.borrow();
    let names: Vec<&str> = s.sessions.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(names, vec!["new", "dead1"], "sessions: {names:?}");
    assert!(!s.sessions[0].exited, "exited is unchanged (false)");
}

/// rc=0黙失敗のpostcondition検出（§2.5）: rename_session呼出がOkでも
/// list_sessions状態が書き換わらない系列（M4 I型・rc=0黙失敗模擬）は、事後一覧の
/// postcondition検証でVerificationFailed（exit 7）として報告する
// [covers:rename.session-silent-noop-detected-by-postcondition]
#[test]
fn rename_session_silent_noop_detected_by_postcondition() {
    let b = FakeBackend::new(vec![], vec![]);
    b.set_sessions(vec![sess("old", false)]);
    b.mute_rename_session();
    let err = zelper::app::rename::run_session(&b, "old", "new", false).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::VerificationFailed,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 7, "message: {}", err.message());
    // §2.5指定文面の完全一致（対象名の欠落を見逃さない）
    assert_eq!(
        err.message(),
        "rename of session 'old' did not take effect (unchanged in list-sessions)"
    );
    // 呼出自体はrc=0成功扱いで発行済み
    assert!(
        b.calls().iter().any(|c| c == "rename-session@old new"),
        "calls: {:?}",
        b.calls()
    );
}

/// rename_session呼出自体の失敗はOperationFailed（§2.6表）: postcondition検証に
/// 到達しない失敗経路。exit 5
// [covers:rename.session-backend-failure-operation-failed]
#[test]
fn rename_session_backend_failure_is_operation_failed() {
    let b = FakeBackend::new(vec![], vec![]);
    b.set_sessions(vec![sess("old", false)]);
    b.inject_failure("rename-session@");
    let err = zelper::app::rename::run_session(&b, "old", "new", false).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::OperationFailed,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 5, "message: {}", err.message());
}

/// 事後一覧取得失敗のclass保持伝播（§2.5）: rename呼出成功後のlist_sessions失敗は
/// 元class/exitを保持したままmessageへ文脈（but failed to confirm）を付与する。
/// OperationFailedとZellijUnavailableの両注入でclass/exit保持を検証
/// （class保持が本質——add/remove sessionと同一pattern）
// [covers:rename.session-postcondition-list-failure-preserves-class]
#[test]
fn rename_session_postcheck_list_failure_preserves_class_with_context() {
    // OperationFailed（既定注入class）: exit 5保持
    let b = FakeBackend::new(vec![], vec![]);
    b.set_sessions(vec![sess("old", false)]);
    b.fail_list_sessions_after_rename();
    let err = zelper::app::rename::run_session(&b, "old", "new", false).unwrap_err();
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
    // rename呼出は成功済み（実行結果が検証できずに報告される系列）
    assert!(
        b.calls().iter().any(|c| c == "rename-session@old new"),
        "calls: {:?}",
        b.calls()
    );

    // ZellijUnavailable注入でもclass/exit保持と文脈付与は同一
    let b = FakeBackend::new(vec![], vec![]);
    b.set_sessions(vec![sess("old", false)]);
    b.fail_list_sessions_after_rename();
    b.inject_failure_as(ErrorClass::ZellijUnavailable);
    let err = zelper::app::rename::run_session(&b, "old", "new", false).unwrap_err();
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
}

/// 空白含みNEW_NAMEの事前拒否（§2.3）: zellij自体は空白含み名を受理する
/// （実測M95-C3: "a b" rename成功）が、parse_sessionsがlist-sessions行の先頭
/// wordのみを名前化するため空白含み名はzellerの世界modelで復元不能
/// （postcondition必敗）。zellij受理可能性と世界modelでの表現可能性は別問題で
/// あり、世界modelで復元不能な名前の作成はUsage（exit 2）で事前拒否する
// [covers:rename.session-whitespace-name-rejected]
#[test]
fn rename_session_rejects_whitespace_in_new_name() {
    let b = FakeBackend::new(vec![], vec![]);
    b.set_sessions(vec![sess("s3", false)]);
    let err = zelper::app::rename::run_session(&b, "s3", "a b", false).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::Usage,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 2, "message: {}", err.message());
    assert_eq!(err.message(), "NEW_NAME must not contain whitespace");
    // 一覧取得より前のfail-fast: いずれのbackend呼出も不発
    assert!(b.calls().is_empty(), "calls: {:?}", b.calls());
}

/// postcondition各判定の独立保護（§2.5）: 旧名消失・新名running出現の両判定が
/// それぞれ単独で不成立となる事後状態では、いずれもVerificationFailed（exit 7）
/// になる。実装を片方の判定のみへ弱めても検出されることの固定
/// （(a) 旧名残存+新名running行追加 (b) 旧名・新名とも行消失
/// (c) 旧名消失+新名がEXITED行で出現——exited == false検査の独立保護）
// [covers:rename.session-postcondition-both-branches-required]
#[test]
fn rename_session_postcondition_requires_both_branches() {
    for (i, inj) in [
        RenamePostInjection::KeepOldAddNew,
        RenamePostInjection::DropBoth,
        RenamePostInjection::NewAsExited,
    ]
    .iter()
    .enumerate()
    {
        let b = FakeBackend::new(vec![], vec![]);
        b.set_sessions(vec![sess("old", false)]);
        b.inject_rename_post(*inj);
        let err = zelper::app::rename::run_session(&b, "old", "new", false).unwrap_err();
        assert_eq!(
            *err.class(),
            ErrorClass::VerificationFailed,
            "case {i} message: {}",
            err.message()
        );
        assert_eq!(err.exit_code(), 7, "case {i} message: {}", err.message());
        // 呼出自体はrc=0成功扱いで発行済み（全系列で一覧→rename→一覧の3呼用）
        assert_eq!(
            b.calls(),
            vec!["list-sessions", "rename-session@old new", "list-sessions"],
            "case {i} calls: {:?}",
            b.calls()
        );
    }
}

/// 事前検査の優先順位固定（§2.8）: 複合違反入力で (i) 不在NAME×NEW_NAME衝突は
/// 不在（NoTarget exit 3）が、(ii) EXITED行NAME×NEW_NAME衝突はEXITED拒否
/// （Preflight exit 7・resurrection案内文面）が、それぞれNEW_NAME衝突検査に
/// 優先する。文面・candidatesは完全一致で検証
// [covers:rename.session-missing-name-notarget-with-candidates]
// [covers:rename.session-exited-name-preflight-refused]
#[test]
fn rename_session_check_order_missing_and_exited_before_collision() {
    // (i) 不在NAME（ghost）× NEW_NAME=既存running名（taken）: NoTarget優先
    let b = FakeBackend::new(vec![], vec![]);
    b.set_sessions(vec![sess("old", false), sess("taken", false)]);
    let err = zelper::app::rename::run_session(&b, "ghost", "taken", false).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::NoTarget,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 3, "message: {}", err.message());
    assert_eq!(err.message(), "session 'ghost' not found");
    // candidates=全行のname（EXITED込み・行順）の完全一致
    assert_eq!(err.candidates(), &["old".to_string(), "taken".to_string()]);

    // (ii) EXITED行NAME（deadx）× NEW_NAME=既存running名（old）: EXITED優先
    let b = FakeBackend::new(vec![], vec![]);
    b.set_sessions(vec![sess("old", false), sess("deadx", true)]);
    let err = zelper::app::rename::run_session(&b, "deadx", "old", false).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::Preflight,
        "message: {}",
        err.message()
    );
    assert_eq!(err.exit_code(), 7, "message: {}", err.message());
    assert_eq!(
        err.message(),
        "cannot rename an EXITED (dead) session 'deadx' (attach to resurrect it first)"
    );
}

/// 先頭`-`NEW_NAMEのUsage事前拒否（TASK-97 §2.1/§2.2）: 先頭`-`NEW_NAMEは
/// Usage（exit 2）で一覧取得より前に拒否する（`--`option終端経由の通過系列が
/// `--`挿入なしのrename-session <to>組み立てへ到達するため入力段の確定拒否。
/// add session NAME検査〔TASK-96〕と対称の4種目）。直呼びのためclapを経由
/// しない系列（先頭`-`直値）で検証。あわせて (i) 先頭`-`×同一名の複合違反は
/// 先頭`-`優先（同一名differの前——§2.2検査順の保護） (ii) 先頭`-`×'/'・
/// 先頭`-`×空白の複合違反は既存文字種優先（先頭`-`検査を'/'やwhitespaceより
/// 前に誤挿入する実装の検出——empty → '/' → whitespace → 先頭`-` → differの
/// 順序固定。add側L2条件〔TASK-96〕はdifferを持たずrename側の挿入位置を保護
/// しないためrename側で保護） (iii) 旧NAME="-x"（一覧に行存在・running）は
/// Usage拒否されずbackend呼出へ到達（§2.3: 旧NAMEは文字種検査対象外——世界model
/// にある名前はそのまま対象化。対称性から旧NAMEへもstarts_with('-')を誤追加
/// する実装の検出）を担う。違反は全case実行後に一括報告（実装前fail-firstの
/// 切り分け観測のため最初の違反で即panicしない）
// [covers:rename.session-leading-dash-new-name-rejected]
#[test]
fn rename_session_rejects_leading_dash_in_new_name() {
    // (NAME, NEW_NAME, 期待文面)。Usage検査はbackend呼出より前のためsessions内容は本質でない
    let invalid_cases = [
        ("s0", "-x", "NEW_NAME must not start with '-'"),
        ("-x", "-x", "NEW_NAME must not start with '-'"),
        ("s0", "-a/b", "NEW_NAME must not contain '/'"),
        ("s0", "-a b", "NEW_NAME must not contain whitespace"),
    ];
    let mut failures: Vec<String> = vec![];
    for (i, (name, new_name, expect_msg)) in invalid_cases.iter().enumerate() {
        let b = FakeBackend::new(vec![], vec![]);
        b.set_sessions(vec![sess("s0", false)]);
        match zelper::app::rename::run_session(&b, name, new_name, false) {
            Ok(()) => failures.push(format!(
                "case {i} ({name:?} -> {new_name:?}): 検査不在により誤通過 (Okが返った)"
            )),
            Err(err) => {
                if *err.class() != ErrorClass::Usage {
                    failures.push(format!(
                        "case {i} ({name:?} -> {new_name:?}): class={:?} (Usage期待)",
                        err.class()
                    ));
                }
                if err.exit_code() != 2 {
                    failures.push(format!(
                        "case {i} ({name:?} -> {new_name:?}): exit={} (2期待)",
                        err.exit_code()
                    ));
                }
                if err.message() != *expect_msg {
                    failures.push(format!(
                        "case {i} ({name:?} -> {new_name:?}): message={:?} ({expect_msg}期待)",
                        err.message()
                    ));
                }
                if !b.calls().is_empty() {
                    failures.push(format!(
                        "case {i} ({name:?} -> {new_name:?}): calls={:?} (空期待)",
                        b.calls()
                    ));
                }
            }
        }
    }

    // (iv) 旧NAME="-x"（一覧存在・running）: Usage拒否されずbackend到達（§2.3契約保護）
    {
        let b = FakeBackend::new(vec![], vec![]);
        b.set_sessions(vec![sess("-x", false)]);
        match zelper::app::rename::run_session(&b, "-x", "new", false) {
            Ok(()) => assert_eq!(
                b.calls(),
                vec!["list-sessions", "rename-session@-x new", "list-sessions"],
                "case 4 (-x -> new) calls: {:?}",
                b.calls()
            ),
            Err(err) => failures.push(format!(
                "case 4 (-x -> new): 旧NAME先頭`-`が誤拒否された (class={:?} exit={} message={:?})",
                err.class(),
                err.exit_code(),
                err.message()
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "leading-dash new-name cases failed:\n{}",
        failures.join("\n")
    );
}
