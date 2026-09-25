// L2: watch（polling loop・照合・pane障害。設計§5.2・watch.toml 16条件のうち
// loop系6条件。文法・出力契約・exit分類の10条件はtests/cli/list_read_send.rs）
mod fake;
use fake::FakeBackend;
use zelper::app::watch::{WatchArgs, execute};
use zelper::cli::FilterArgs;
use zelper::domain::*;
use zelper::error::ErrorClass;

fn pane(id: u32, title: &str, y: u32, x: u32) -> PaneState {
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
        command: None,
        terminal_command: None,
        cwd: None,
        tab_id: TabId(0),
        tab_position: 0,
        tab_name: "T1".into(),
        plugin_url: None,
    }
}

fn term(n: u32) -> PaneKindId {
    PaneKindId::Terminal(n)
}

/// watch::executeの呼び出しhelper（filterなし・pattern 1件）。intervalは実sleep
/// ため、実行時間抑制のため呼び出し側で--maxを最小に寄せる（設計§5.3・R6）
fn run_watch(
    b: &FakeBackend,
    panes: &[&str],
    pattern: &str,
    regex: bool,
    full: bool,
    interval: f64,
    max: u32,
) -> Result<zelper::app::watch::WatchHit, zelper::error::ZelperError> {
    let panes: Vec<String> = panes.iter().map(|s| s.to_string()).collect();
    let pattern = vec![pattern.to_string()];
    let args = WatchArgs {
        panes: &panes,
        filter: &FilterArgs::default(),
        regex,
        full,
        interval,
        max,
        pattern: &pattern,
        json: false,
    };
    execute(b, &args)
}

/// attempt 1は初期sleepなしの即時実行で、watch開始前から存在する既出テキストで
/// 即hitする（attempts_used=1・dump呼び出し1回）。照合は行単位のcontains
/// （行全体一致でなく部分一致: 行の途中にpatternを含む行がhitし、行全体を報告）
// [covers:watch.t0-immediate-first-poll]
#[test]
fn t0_immediate_first_poll_hits_on_existing_text() {
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0)], vec![]);
    b.set_dump_frames(
        term(1),
        vec!["booting...\nagent is READY now\ndone\n".into()],
    );
    let hit = run_watch(&b, &["1"], "READY", false, false, 1.0, 3).unwrap();
    assert_eq!(hit.attempts_used, 1);
    assert_eq!(hit.matched_lines, vec!["agent is READY now".to_string()]);
    // 初期sleepなし: dump_screenは1回のみ
    assert_eq!(b.dump_calls(&term(1)), 1);
}

/// 複数対象のany-hit契約: 後続attemptで2 pane目のみhitする系列は当該paneを報告
/// し、同一attemptで複数paneがhitする系列はvisual order最初のpaneを報告する
/// （決定論的tie-break。引数順でない。報告はhit pane単一のobject）
// [covers:watch.any-hit-multi-pane]
#[test]
fn any_hit_multi_pane_reports_hit_pane() {
    // 系列1: attempt 2でpane 2のみがhit → pane 2を報告
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0), pane(2, "b", 0, 100)], vec![]);
    b.set_dump_frames(term(1), vec!["idle".into(), "still idle".into()]);
    b.set_dump_frames(term(2), vec!["working".into(), "READY agent online".into()]);
    let hit = run_watch(&b, &["1", "2"], "READY", false, false, 1.0, 3).unwrap();
    assert_eq!(hit.pane.id, term(2));
    assert_eq!(hit.attempts_used, 2);
    assert_eq!(hit.matched_lines, vec!["READY agent online".to_string()]);

    // 系列2: attempt 1から両pane hit。引数順（2, 1）とvisual順（terminal_1が先:
    // y=0, x=0 < x=100）を逆にしてもvisual order最初のpaneを報告する
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0), pane(2, "b", 0, 100)], vec![]);
    b.set_dump_frames(term(1), vec!["READY here".into()]);
    b.set_dump_frames(term(2), vec!["READY there".into()]);
    let hit = run_watch(&b, &["2", "1"], "READY", false, false, 1.0, 3).unwrap();
    assert_eq!(hit.pane.id, term(1));
}

/// --regexのopt-in契約: 無指定の照合は固定substring（大文字小文字区別）で
/// hitせず回数切れ、--regex '(?i)...'指定で初めて正規表現照合になる
// [covers:watch.regex-opt-in]
#[test]
fn regex_is_opt_in_substring_by_default() {
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0)], vec![]);
    b.set_dump_frames(term(1), vec!["READY".into()]);

    // 無指定: substring照合は大文字小文字を区別 → hitせず回数切れ（exit 5相当）
    let err = run_watch(&b, &["1"], "ready", false, false, 1.0, 2).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);

    // --regex '(?i)ready': 大文字小文字無視は(?i)表現のみ → hit
    let hit = run_watch(&b, &["1"], "(?i)ready", true, false, 1.0, 2).unwrap();
    assert_eq!(hit.matched_lines, vec!["READY".to_string()]);
}

/// --full指定時のbackend転送: dump_screen呼び出しがfull=true（dump-screen -f）付き
/// に変わり、-f応答にのみ存在する行（scrollback由来）でhitする
// [covers:watch.full-flag]
#[test]
fn full_flag_passes_dump_screen_f_and_hits_full_only_line() {
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0)], vec![]);
    // 通常応答（未設定frame = SCREEN[terminal_1]）にはhitしない。-f応答のみ
    // scrollback由来行を含む
    b.set_dump_full(term(1), "old READY in scrollback\n".into());

    // full=false: hitせず（max 1で即回数切れ）
    let err = run_watch(&b, &["1"], "READY", false, false, 1.0, 1).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);

    // full=true: -f付き呼び出しでscrollback由来行にhit
    let hit = run_watch(&b, &["1"], "READY", false, true, 1.0, 1).unwrap();
    assert_eq!(
        hit.matched_lines,
        vec!["old READY in scrollback".to_string()]
    );
    assert!(
        b.calls().iter().any(|c| c == "dump-screen terminal_1 -f"),
        "calls: {:?}",
        b.calls()
    );
}

/// polling loopの構造: attempt間にintervalのsleepを挟み、最終attempt後には
/// sleepしない。試行回数はdump_screen呼び出し回数として計上され
/// error.data.attempts_usedへ現れる（過不足pollなし）
// [covers:watch.interval-sleep-and-attempt-count]
#[test]
fn interval_sleep_and_attempt_count() {
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0), pane(2, "b", 0, 100)], vec![]);
    // 未設定frame（SCREEN[...]固定）= 常にno hit
    let err = run_watch(&b, &["1", "2"], "READY", false, false, 1.0, 2).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
    let data = err.data().expect("exhaust error data");
    assert_eq!(data["attempts_used"], 2);
    assert_eq!(data["pattern"], "READY");
    let elapsed = data["elapsed_secs"].as_f64().expect("elapsed_secs f64");
    // attempt間sleepあり（>= interval 1.0）・最終attempt後にsleepしない
    // （interval x max = 2.0を大きくは超えない）
    assert!(elapsed >= 1.0, "elapsed: {elapsed}");
    assert!(elapsed < 3.0, "elapsed: {elapsed}");
    // 各paneのdump呼び出しは2回（= max。最終attempt後の過剰pollなし）
    assert_eq!(b.dump_calls(&term(1)), 2);
    assert_eq!(b.dump_calls(&term(2)), 2);
}

/// polling中のpane障害の扱い: dump_screen失敗paneは以後のattemptから除外され
/// （失敗後の再pollなし）、他paneのpollingは継続される。障害はpane_errorsへ
/// 記録され、生存paneでのhitが優先される（hit時もpane_errorsを併記）。
/// 対象解決が開始時1回であることはlist_panes呼び出し回数1で観察する
// [covers:watch.pane-vanish-continue]
#[test]
fn pane_vanish_excluded_and_others_continue() {
    let b = FakeBackend::new(vec![pane(1, "a", 0, 0), pane(2, "b", 0, 100)], vec![]);
    // pane 1: attempt 1は成功し、attempt 2以降はdump失敗（pane消滅模擬）
    b.set_dump_frames(term(1), vec!["no".into()]);
    b.set_dump_fail_after(term(1), 1);
    // pane 2: attempt 3でhit
    b.set_dump_frames(
        term(2),
        vec!["not yet".into(), "no".into(), "finally READY".into()],
    );
    let hit = run_watch(&b, &["1", "2"], "READY", false, false, 1.0, 3).unwrap();
    assert_eq!(hit.pane.id, term(2));
    assert_eq!(hit.attempts_used, 3);
    assert_eq!(
        hit.pane_errors
            .iter()
            .map(|e| e.target.clone())
            .collect::<Vec<_>>(),
        vec!["terminal_1".to_string()],
        "pane_errors: {:?}",
        hit.pane_errors
    );
    // pane 1の失敗（attempt 2）以降の同paneへのdump呼び出しなし
    // （成功1回 + 失敗1回の計2回で、attempt 3では除外済み）
    assert_eq!(b.dump_calls(&term(1)), 2);
    // pane 2は継続pollされhit（3回 = attempt 3まで）
    assert_eq!(b.dump_calls(&term(2)), 3);
    // 対象解決は開始時1回のみ
    let list_panes_calls = b.calls().iter().filter(|c| *c == "list-panes").count();
    assert_eq!(list_panes_calls, 1);
}
