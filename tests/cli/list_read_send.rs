// L3: CLI契約テスト（list/read/send。test-plan §2.1/2.4/2.10）
// fake zellij shimをtest実行時に生成し、PATHへ差し込む（実zellijに依存しない）
use assert_cmd::Command;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// fake zellij shimのwrite↔exec競合（ETXTBSY）対策: binary内のfake zellij
/// testを直列化する（CR59-5・TASK-73。src/zellij/process.rs test moduleと同一構成+契約doc comment）。
/// test fn冒頭でのみ取得すること。wrapper・helper内では再取得しないこと
/// （std::sync::Mutexは非reentrantのため同一thread再取得は永久blockし、
/// failではなくtest hangとして発覚する）。
static FAKE_ZELLIJ_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zellij")
}

/// fake zellij実行可能fileをworkdirに生成し、(PATHに設定すべきdir, log path)を返す
fn setup_fake_zellij(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("zelper-fake-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let shim = dir.join("zellij");
    let script = r#"#!/usr/bin/env bash
echo "$*" >> "$FAKE_LOG"
if [[ "$1" == "--version" ]]; then echo "zellij 0.44.3"; exit 0; fi
if [[ "$1" == "list-sessions" ]]; then
  if [[ -n "$FAKE_SESSIONS" ]]; then printf '%s\n' "$FAKE_SESSIONS"; else echo "fake-sess [Created 1m ago] (current)"; fi
  exit 0
fi
if [[ "$3" == "action" ]]; then
  shift 3
  action="$1"; shift
  case "$action" in
    list-panes) cat "$FAKE_FIXTURES/panes.json"; exit 0;;
    list-tabs)
      if [[ -n "$FAKE_FAIL_LIST_TABS" ]]; then echo "boom" >&2; exit 1; fi
      cat "$FAKE_FIXTURES/tabs.json"; exit 0;;
    dump-screen)
      pane=""; full=""
      while [[ $# -gt 0 ]]; do
        case "$1" in
          -p) pane="$2"; shift 2;;
          -f) full="FULL"; shift;;
          *) shift;;
        esac
      done
      echo "SCREEN[$pane]$full"; exit 0;;
    *) echo "ok"; exit 0;;
  esac
fi
exit 0
"#;
    // 実行内容とmode確定後にのみ公開する。最終の実行pathはwrite open状態にならない
    // （LinuxのETXTBSY回避）。
    let staged = dir.join("zellij.staged");
    std::fs::write(&staged, script).unwrap();
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::rename(staged, &shim).unwrap();
    let log = dir.join("calls.log");
    (dir, log)
}

fn zelper(tag: &str) -> (Command, PathBuf) {
    let (dir, log) = setup_fake_zellij(tag);
    let mut cmd = Command::cargo_bin("zelper").unwrap();
    // shimを優先しつつ既存PATH（bash/cat等）を保持する。
    // ZELLIJ_SESSION_NAMEを除去し、zellij session内実行でもsession解決が
    // fake shimに向くよう隔離する（hermetic test）
    let path = format!(
        "{}:{}",
        dir.to_str().unwrap(),
        std::env::var("PATH").unwrap_or_default()
    );
    cmd.env("PATH", path)
        .env_remove("ZELLIJ_SESSION_NAME")
        .env("FAKE_LOG", &log)
        .env("FAKE_FIXTURES", fixture_dir());
    (cmd, log)
}

fn calls(log: &PathBuf) -> Vec<String> {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .collect()
}

// [covers:json-contract.list-panes-json-success-envelope]
#[test]
fn list_panes_json_contract() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("list-panes");
    let out = cmd
        .args(["list", "panes", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["ok"], true);
    let panes = v["data"]["panes"].as_array().unwrap();
    assert_eq!(panes.len(), 3);
    assert_eq!(panes[0]["pane_id"], "terminal_1");
}

// [covers:list-display.sessions-and-tabs-basic-output]
#[test]
fn list_sessions_and_tabs() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("list-sessions");
    let out = cmd
        .args(["list", "sessions", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["data"]["sessions"][0]["name"], "fake-sess");
    assert_eq!(v["data"]["sessions"][0]["created"], "1m ago");
    assert_eq!(v["data"]["sessions"][0]["current"], true);
    assert_eq!(
        v["data"]["sessions"][0]["panes_per_tab"],
        serde_json::json!([2])
    );

    let (mut cmd, _) = zelper("list-tabs");
    cmd.args(["list", "tabs"])
        .assert()
        .success()
        .stdout("TAB_ID\tPOS\tACTIVE\tNAME\tPANES\n0\t0\t*\tTab #1\t2\n");
}

/// EXITED（dead session）が複数残っていても、list sessionsは対象session解決を
/// 前提とせず一覧表示できる（実行中のみを表示）
// [covers:list-display.sessions-live-only-with-exited-present]
#[test]
fn list_sessions_works_with_exited_sessions() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("list-sessions-exited");
    let out = cmd
        .env(
            "FAKE_SESSIONS",
            "dead-a [Created 2h ago] (EXITED - attach to resurrect)\n\
             live-a [Created 1m ago] (current)\n\
             dead-b [Created 3d ago] (EXITED - attach to resurrect)",
        )
        .args(["list", "sessions", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let sessions: Vec<&str> = v["data"]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(sessions, vec!["live-a"]);
    // EXITED混在環境でも実行中行のmetadata（created/current/PANES）が取れる
    assert_eq!(v["data"]["sessions"][0]["created"], "1m ago");
    assert_eq!(
        v["data"]["sessions"][0]["panes_per_tab"],
        serde_json::json!([2])
    );
}

/// list sessions のhuman出力: NAME/CREATED/CURRENT/PANES列（PANESはtab毎pane数の
/// 連結。shimのtabs fixtureはtiled=2固定）
// [covers:list-display.sessions-human-metadata-columns]
#[test]
fn list_sessions_human_shows_metadata_columns() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("list-sessions-human");
    cmd.env(
        "FAKE_SESSIONS",
        "live-a [Created 1m ago] (current)\nlive-b [Created 2h ago]",
    )
    .args(["list", "sessions"])
    .assert()
    .success()
    .stdout("NAME\tCREATED\tCURRENT\tPANES\nlive-a\t1m ago\t*\t2\nlive-b\t2h ago\t\t2\n");
}

/// session解決が必要なコマンドも、EXITED込みで実行中1つなら自動解決する
/// （--sessionを要求しない）
// [covers:cli-grammar.session-auto-resolve-single-live-with-exited]
#[test]
fn read_resolves_single_live_session_with_exited_present() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("read-exited-resolve");
    cmd.env(
        "FAKE_SESSIONS",
        "dead-a [Created 2h ago] (EXITED - attach to resurrect)\n\
             live-a [Created 1m ago] (current)",
    )
    .args(["read", "1"])
    .assert()
    .success();
}

// [covers:list-display.panes-human-columns-with-cwd]
#[test]
fn list_panes_human_includes_cwd_column() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("list-panes-cwd");
    cmd.args(["list", "panes"]).assert().success().stdout(
        "PANE_ID\tTAB\tTITLE\tCOMMAND\tCWD\n\
             terminal_1\tTab #1\tagent-a\tcodex exec --full-auto\t/work\n\
             terminal_2\tTab #1\tagent-b\tcodex exec --plan\t/work\n\
             plugin_0\tTab #1\tZellij (tab bar)\t-\t-\n",
    );
}

/// 実行中sessionが複数あっても、list layoutsはsession解決を行わず動く
/// （layout一覧はfile列挙でありzellijを呼ばない）
// [covers:list-display.layouts-without-session-resolution]
#[test]
fn list_layouts_works_without_session_resolution() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("list-layouts-nosession");
    let dir = std::env::temp_dir().join(format!("zelper-fake-layouts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("three.kdl"), "// test layout\n").unwrap();
    std::fs::write(dir.join("two.kdl"), "// test layout\n").unwrap();
    let out = cmd
        .env(
            "FAKE_SESSIONS",
            "live-a [Created 1m ago] (current)\nlive-b [Created 2m ago]",
        )
        .env("ZELLIJ_LAYOUT_DIR", &dir)
        .args(["list", "layouts", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let layouts = v["data"]["layouts"].as_array().unwrap();
    assert_eq!(layouts.len(), 2);
    assert_eq!(layouts[0]["name"], "three");
    assert_eq!(layouts[0]["size"], 15);
    assert_eq!(layouts[1]["name"], "two");
    assert!(layouts[0]["modified_epoch"].is_u64());

    // human出力: ヘッダ + name/size列（MODIFIEDは実行時刻依存のため部分一致）
    let (mut cmd, _) = zelper("list-layouts-human");
    let out = cmd
        .env("ZELLIJ_LAYOUT_DIR", &dir)
        .args(["list", "layouts"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.starts_with("NAME\tSIZE\tMODIFIED\n"));
    assert!(text.contains("three\t15\t"));
    assert!(text.contains("two\t15\t"));
}

// [covers:read-send.read-single-and-multi-results]
#[test]
fn read_single_and_multi_with_tail() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("read-1");
    let out = cmd
        .args(["read", "1", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["data"]["results"][0]["target"], "terminal_1");
    assert_eq!(v["data"]["results"][0]["detail"], "SCREEN[terminal_1]\n");

    let (mut cmd, _) = zelper("read-2");
    cmd.args(["read", "1", "2", "--json"]).assert().success();
}

// [covers:read-send.read-nonexistent-pane-is-no-target-exit3]
#[test]
fn read_nonexistent_pane_is_no_target_exit3() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("read-miss");
    cmd.args(["read", "999", "--json"])
        .assert()
        .failure()
        .code(3);
}

// [covers:json-contract.cli-error-envelope-on-failure]
#[test]
fn json_error_envelope_on_failure() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    // レビュー回帰: --json指定時の失敗はstdoutにerror envelope（DD-4.2）
    let (mut cmd, _) = zelper("json-err");
    let out = cmd
        .args(["read", "999", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["class"], "NoTarget");
}

// [covers:read-send.send-broadcast-visual-order-calls]
#[test]
fn send_broadcast_records_calls_in_visual_order() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, log) = zelper("send-broadcast");
    cmd.args(["send", "2", "1", "--", "y"]).assert().success();
    let cs = calls(&log);
    let writes: Vec<_> = cs
        .iter()
        .filter(|c| c.starts_with("--session fake-sess action write-chars"))
        .collect();
    // visual order: agent-a(y=1,x=0)がterminal_1、agent-b(x=100)がterminal_2
    assert_eq!(writes.len(), 2);
    assert!(writes[0].contains("-p terminal_1 y"));
    assert!(writes[1].contains("-p terminal_2 y"));
}

// [covers:read-send.send-enter-appends-cr-and-keys-use-send-keys]
#[test]
fn send_enter_appends_cr_and_keys_use_send_keys() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, log) = zelper("send-enter");
    cmd.args(["send", "1", "--enter", "--", "hello"])
        .assert()
        .success();
    let cs = calls(&log);
    assert!(
        cs.iter()
            .any(|c| c.contains("write-chars -p terminal_1 hello"))
    );
    assert!(cs.iter().any(|c| c.contains("write -p terminal_1 13")));

    let (mut cmd, log) = zelper("send-keys");
    cmd.args(["send", "1", "--keys", "Ctrl", "a"])
        .assert()
        .success();
    let cs = calls(&log);
    assert!(
        cs.iter()
            .any(|c| c.contains("send-keys -p terminal_1 Ctrl a"))
    );
}

// [covers:cli-grammar.send-requires-double-dash-text-is-usage-error]
#[test]
fn send_without_double_dash_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("send-nodash");
    cmd.args(["send", "1", "y"]).assert().failure().code(2);
}

// [covers:cli-grammar.send-text-keys-conflict-is-usage-error]
#[test]
fn send_text_and_keys_conflict_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("send-conflict");
    cmd.args(["send", "1", "--keys", "Enter", "--", "y"])
        .assert()
        .failure()
        .code(2);
}

// [covers:cli-grammar.remap-layout-sources-conflict-is-usage-error]
#[test]
fn remap_layout_sources_conflict_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("remap-conflict");
    cmd.args(["remap", "three", "--path", "./x.kdl"])
        .assert()
        .failure()
        .code(2);
}

// [covers:cli-grammar.invalid-pane-spec-is-usage-error]
#[test]
fn invalid_pane_spec_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("bad-spec");
    cmd.args(["read", "pane:12"]).assert().failure().code(2);
}

// [covers:cli-grammar.completion-generates-script]
#[test]
fn completion_generates_script() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("completion");
    let out = cmd
        .args(["completion", "bash"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(out).unwrap();
    assert!(s.contains("zelper"));
    assert!(s.len() > 100);
}

// [covers:read-send.human-output-pane-headers]
#[test]
fn human_read_output_has_pane_headers() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("read-human");
    cmd.args(["read", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "=== terminal_1 (agent-a, tab:0 Tab #1) ===",
        ));
}

/// PANES概要: 5tab超は先頭4つ+`...`で省略。floatingは連結に含めない
/// （6tab: tiled 7,7,6,2,1,3・2tab目にfloating 2）
// [covers:list-display.sessions-summary-elides-beyond-four-tabs]
#[test]
fn list_sessions_panes_summary_elides_beyond_four_tabs() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("list-sessions-6tabs");
    let dir = std::env::temp_dir().join(format!("zelper-fake-tabs6-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let tab = |pos: u32, tiled: u32, floating: u32, name: &str| {
        format!(
            r#"{{"position": {pos}, "name": "{name}", "active": {}, "are_floating_panes_visible": {}, "selectable_tiled_panes_count": {tiled}, "selectable_floating_panes_count": {floating}, "tab_id": {pos}}}"#,
            pos == 0,
            floating > 0
        )
    };
    let tabs = format!(
        "[{},{},{},{},{},{}]",
        tab(0, 7, 0, "t0"),
        tab(1, 7, 2, "t1"),
        tab(2, 6, 0, "t2"),
        tab(3, 2, 0, "t3"),
        tab(4, 1, 0, "t4"),
        tab(5, 3, 0, "t5")
    );
    std::fs::write(dir.join("tabs.json"), tabs).unwrap();
    cmd.env("FAKE_SESSIONS", "live-a [Created 1m ago] (current)")
        .env("FAKE_FIXTURES", &dir)
        .args(["list", "sessions"])
        .assert()
        .success()
        .stdout("NAME\tCREATED\tCURRENT\tPANES\nlive-a\t1m ago\t*\t7+7+6+2+...\n");

    // list tabsのPANES列はfloatingを `2+1f` 形式で併記（単tabなので曖昧性なし）
    let (mut cmd, _) = zelper("list-tabs-floating");
    cmd.env("FAKE_FIXTURES", &dir)
        .args(["list", "tabs"])
        .assert()
        .success()
        .stdout(
            "TAB_ID\tPOS\tACTIVE\tNAME\tPANES\n\
             0\t0\t*\tt0\t7\n\
             1\t1\t\tt1\t7+2f\n\
             2\t2\t\tt2\t6\n\
             3\t3\t\tt3\t2\n\
             4\t4\t\tt4\t1\n\
             5\t5\t\tt5\t3\n",
        );
}

/// 他sessionのlist-tabs取得失敗は該当行を `-`（JSONはnull）に落とし、
/// 一覧自体は失敗させない。currentなしsessionのJSON current:falseも検証
// [covers:list-display.sessions-tabs-fetch-failure-fallback]
#[test]
fn list_sessions_fallback_when_tabs_fetch_fails() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("list-sessions-tabsfail");
    cmd.env("FAKE_SESSIONS", "live-a [Created 1m ago] (current)")
        .env("FAKE_FAIL_LIST_TABS", "1")
        .args(["list", "sessions"])
        .assert()
        .success()
        .stdout("NAME\tCREATED\tCURRENT\tPANES\nlive-a\t1m ago\t*\t-\n");

    let (mut cmd, _) = zelper("list-sessions-tabsfail-json");
    let out = cmd
        .env("FAKE_SESSIONS", "live-a [Created 1m ago]")
        .env("FAKE_FAIL_LIST_TABS", "1")
        .args(["list", "sessions", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["data"]["sessions"][0]["current"], false);
    assert!(v["data"]["sessions"][0]["panes_per_tab"].is_null());
}

/// compact表示: panesはtab名と短縮cwdのみ（HOME prefixは `~` 置換）
// [covers:list-display.panes-compact-shows-tab-and-short-cwd]
#[test]
fn list_panes_compact_shows_tab_and_short_cwd() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("list-panes-compact");
    let dir = std::env::temp_dir().join(format!("zelper-fake-panes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let base = std::fs::read_to_string(fixture_dir().join("panes.json")).unwrap();
    std::fs::write(
        dir.join("panes.json"),
        base.replace("\"/work\"", "\"/home/tester/work/app\""),
    )
    .unwrap();
    cmd.env("HOME", "/home/tester")
        .env("FAKE_FIXTURES", &dir)
        .args(["list", "panes", "--compact"])
        .assert()
        .success()
        .stdout("Tab #1\t~/work/app\nTab #1\t~/work/app\n");
}

/// compact表示: sessions/tabsは名前のみ。--json併用時はJSON優先（compact無視）
// [covers:list-display.compact-names-and-json-precedence]
#[test]
fn list_compact_names_and_json_precedence() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("list-tabs-compact");
    cmd.args(["list", "tabs", "--compact"])
        .assert()
        .success()
        .stdout("Tab #1\n");

    let (mut cmd, _) = zelper("list-sessions-compact");
    cmd.env("FAKE_SESSIONS", "live-a [Created 1m ago] (current)")
        .args(["list", "sessions", "-c"])
        .assert()
        .success()
        .stdout("live-a\n");

    let (mut cmd, _) = zelper("list-compact-json");
    let out = cmd
        .env("FAKE_SESSIONS", "live-a [Created 1m ago] (current)")
        .args(["list", "sessions", "--compact", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["data"]["sessions"][0]["created"], "1m ago");
}

/// layouts のcompact表示: 名前のみ（レビューC24: 未カバー分岐の固定）
// [covers:list-display.layouts-compact-names-only]
#[test]
fn list_layouts_compact_shows_names_only() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("list-layouts-compact");
    let dir = std::env::temp_dir().join(format!("zelper-fake-layouts2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("solo.kdl"), "// test layout\n").unwrap();
    cmd.env("ZELLIJ_LAYOUT_DIR", &dir)
        .args(["list", "layouts", "--compact"])
        .assert()
        .success()
        .stdout("solo\n");
}
