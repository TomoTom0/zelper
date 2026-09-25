// L3: CLI契約テスト（list/read/send/watch。test-plan §2.1/2.4/2.10。watchはTASK-86）
// fake zellij shimをtest実行時に生成し、PATHへ差し込む（実zellijに依存しない）
use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
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
  # TASK-89: FAKE_SESSIONS_EMPTY設定時は0-session環境（list-sessions自体がrc=1で
  # 失敗する実環境・M5 (0)実測）を再現。attach -bでの作成NAME反映
  # （${FAKE_LOG}.sessions）があれば通常応答へ戻る（FAKE_SESSIONS_CREATE=okの状態管理）
  state="${FAKE_LOG}.sessions"
  if [[ -n "$FAKE_SESSIONS_EMPTY" && ! -s "$state" ]]; then
    echo "No active zellij sessions found."; exit 1
  fi
  if [[ -n "$FAKE_SESSIONS" ]]; then printf '%s\n' "$FAKE_SESSIONS"; else echo "fake-sess [Created 1m ago] (current)"; fi
  [[ -s "$state" ]] && cat "$state"
  exit 0
fi
if [[ "$1" == "attach" ]]; then
  # TASK-89: `zellij attach ...`（positional形式のtop-level argv。--session接頭なし）
  if [[ "$2" == "-b" ]]; then
    # FAKE_SESSIONS_CREATE: attach -b応答制御（作成ok / ok-mute〔list-sessions応答へ
    # 反映しない〕/ already-exists〔M5 (e1)実測のrc=1・stderr再現〕）
    case "$FAKE_SESSIONS_CREATE" in
      already-exists) echo "Session already exists" >&2; exit 1;;
      ok-mute) exit 0;;
      *) echo "$3 [Created 1m ago] (current)" >> "${FAKE_LOG}.sessions"; exit 0;;
    esac
  fi
  exit 0
fi
if [[ "$3" == "action" ]]; then
  # TASK-89: 不在session名へのactionは実zellijと同じ rc=1・Session not found +
  # 稼働session候補表示で応答（M3 (b2)実測）。稼働session = FAKE_SESSIONS
  # （未設定時は既定のfake-sess）+ attach -b作成分（${FAKE_LOG}.sessions）
  sess="$2"
  state="${FAKE_LOG}.sessions"
  lines=""
  [[ -s "$state" ]] && lines="$(cat "$state")"
  if [[ -n "$FAKE_SESSIONS" ]]; then lines="$lines"$'\n'"$FAKE_SESSIONS"; else lines="$lines"$'\n'"fake-sess [Created 1m ago] (current)"; fi
  if ! printf '%s\n' "$lines" | awk -v s="$sess" 'NF && $1 == s { found = 1 } END { exit found ? 0 : 1 }'; then
    echo "Session '$sess' not found. The following sessions are active:" >&2
    printf '%s\n' "$lines" | awk 'NF { print $1 }' >&2
    exit 1
  fi
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
      # pane毎のdump-screen呼出回数counter（FAKE_DUMP_SCREEN_FRAMES /
      # FAKE_FAIL_DUMP_SCREEN_AFTERで使用。counter fileはshim内部管理）
      cnt="${FAKE_LOG}.cnt.$pane"
      n=0
      [[ -f "$cnt" ]] && n=$(<"$cnt")
      n=$((n + 1))
      printf '%s' "$n" > "$cnt"
      # FAKE_FAIL_DUMP_SCREEN_AFTER（pane:成功回数のspace区切りlist）に一致するpaneは
      # 成功回数を超えた呼び出しから失敗させる（polling中のpane消滅模擬。REVIEW-1）
      for spec in $FAKE_FAIL_DUMP_SCREEN_AFTER; do
        if [[ "$spec" == "$pane:"* ]]; then
          limit="${spec##*:}"
          if (( n > limit )); then echo "boom" >&2; exit 1; fi
        fi
      done
      # FAKE_FAIL_DUMP_SCREEN（失敗させるpane idのspace区切りlist）に一致するpaneは
      # 失敗させる。未設定時は現行挙動のまま（後方互換）
      for fail in $FAKE_FAIL_DUMP_SCREEN; do
        if [[ "$fail" == "$pane" ]]; then echo "boom" >&2; exit 1; fi
      done
      # FAKE_DUMP_SCREEN_FRAMES設定時は呼出回数（pane毎counter file）で内容が変わる
      # 応答を返す（@@ATTEMPT@@区切り。frame数を超えたら最終frameを継続。
      # watchの多attempt hit/exhaust再現。未設定時は従来の応答経路）
      if [[ -n "$FAKE_DUMP_SCREEN_FRAMES" ]]; then
        printf '%s' "$FAKE_DUMP_SCREEN_FRAMES" | awk -v n="$n" '
          BEGIN { RS = "@@ATTEMPT@@" }
          { f[NR] = $0 }
          END { if (n > NR) print f[NR]; else print f[n] }'
        exit 0
      fi
      # FAKE_DUMP_SCREEN設定時は応答を差し替え（末尾行へFULL接尾）。未設定時は現状通り
      if [[ -n "$FAKE_DUMP_SCREEN" ]]; then
        printf '%s%s\n' "$FAKE_DUMP_SCREEN" "$full"
      else
        echo "SCREEN[$pane]$full"
      fi
      exit 0;;
    write-chars)
      pane=""
      while [[ $# -gt 0 ]]; do
        case "$1" in
          -p) pane="$2"; shift 2;;
          *) shift;;
        esac
      done
      # FAKE_FAIL_WRITE_CHARS（失敗させるpane idのspace区切りlist）に一致するpaneは
      # 失敗させる。未設定時は "ok" exit 0（従来のcatch-all応答と同一）
      for fail in $FAKE_FAIL_WRITE_CHARS; do
        if [[ "$fail" == "$pane" ]]; then echo "boom" >&2; exit 1; fi
      done
      echo "ok"; exit 0;;
    # TASK-89: FAKE_DETACH_FAIL設定時はaction detachを非zero終了させる。
    # 未設定時は rc=0 silent no-op成功（client不在の実zellij挙動・M3 (b)と同一）
    detach)
      if [[ -n "$FAKE_DETACH_FAIL" ]]; then echo "boom" >&2; exit 1; fi
      echo "ok"; exit 0;;
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

/// --full指定時はbackendのdump-screen呼び出しに-fが渡り、detailは-f応答
/// （FULL接尾）になる。scrollback込み取得はzellij側機能のため-f転送のみ検証
// [covers:read-send.read-full-passes-dump-screen-f]
#[test]
fn read_full_passes_dump_screen_f() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, log) = zelper("read-full");
    let out = cmd
        .args(["read", "1", "--full", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let cs = calls(&log);
    assert!(
        cs.iter()
            .any(|c| c.contains("dump-screen -p terminal_1 -f")),
        "calls: {cs:?}"
    );
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        v["data"]["results"][0]["detail"],
        "SCREEN[terminal_1]FULL\n"
    );
}

/// --tail Nは取得済みcontentの末尾N行へ切り詰める（--full併用時は-f付き
/// 取得内容に適用。N >= 行数なら全行保持）。応答はFAKE_DUMP_SCREENで
/// 識別可能な5行へ差し替える
// [covers:read-send.read-tail-applies-to-last-n-lines]
#[test]
fn read_tail_applies_to_last_n_lines() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let screen = "L1\nL2\nL3\nL4\nL5";

    let (mut cmd, _) = zelper("read-tail2");
    let out = cmd
        .env("FAKE_DUMP_SCREEN", screen)
        .args(["read", "1", "--tail", "2", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    // 末尾2行のみ（先頭側L1-L3は含まれない）
    assert_eq!(v["data"]["results"][0]["detail"], "L4\nL5");

    // --full併用: -f付き取得内容（末尾行へFULL接尾）の末尾2行
    let (mut cmd, _) = zelper("read-full-tail2");
    let out = cmd
        .env("FAKE_DUMP_SCREEN", screen)
        .args(["read", "1", "--full", "--tail", "2", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["data"]["results"][0]["detail"], "L4\nL5FULL");

    // N >= 行数: 切り詰め無し（全行保持）
    let (mut cmd, _) = zelper("read-tail99");
    let out = cmd
        .env("FAKE_DUMP_SCREEN", screen)
        .args(["read", "1", "--tail", "99", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["data"]["results"][0]["detail"], "L1\nL2\nL3\nL4\nL5");
}

/// 複数対象readの一部paneのみdump-screenが失敗（shim失敗注入）しても打ち切らず、
/// 成功paneのentry（detail）と失敗paneのentry（error）がresults[]へ混在し
/// exit 6（PartialFailure）を返す。失敗時のresults[]はerror envelopeの
/// error.dataへ同梱される（src/output/json.rs ErrEnvelope）。失敗は
/// visual order先頭pane（terminal_1）へ注入し、先頭失敗後に打ち切る実装は
/// results不足（terminal_2の成功entry欠落）でfailする
// [covers:read-send.read-partial-failure-exit6]
#[test]
fn read_partial_failure_exit6() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("read-partial-fail");
    let out = cmd
        .env("FAKE_FAIL_DUMP_SCREEN", "terminal_1")
        .args(["read", "1", "2", "--json"])
        .assert()
        .failure()
        .code(6)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["class"], "PartialFailure");
    let results = v["error"]["data"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    // 失敗entry（visual order先頭）: error（shimの失敗出力を含む）を報告しdetailは省略される
    let err_entry = &results[0];
    assert_eq!(err_entry["target"], "terminal_1");
    assert_eq!(err_entry["ok"], false);
    assert!(
        err_entry["error"].as_str().unwrap().contains("boom"),
        "error: {:?}",
        err_entry["error"]
    );
    assert!(err_entry["detail"].is_null());
    // 成功entry: detailを持ちerror fieldは省略される
    let ok_entry = &results[1];
    assert_eq!(ok_entry["target"], "terminal_2");
    assert_eq!(ok_entry["ok"], true);
    assert_eq!(ok_entry["detail"], "SCREEN[terminal_2]\n");
    assert!(ok_entry["error"].is_null());
}

/// 対象全paneのdump-screen失敗（shim失敗注入）はexit 5（OperationFailed）。
/// 部分失敗（exit 6・PartialFailure）と排他な全失敗集計の分岐
// [covers:read-send.read-all-failure-exit5]
#[test]
fn read_all_failure_exit5() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("read-all-fail");
    let out = cmd
        .env("FAKE_FAIL_DUMP_SCREEN", "terminal_1 terminal_2")
        .args(["read", "1", "2", "--json"])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["class"], "OperationFailed");
}

/// sendのbroadcast性質: visual order先頭pane（terminal_1）のみwrite-charsが失敗
/// （shim失敗注入）しても残対象（terminal_2）へ継続し、exit 6（PartialFailure）で
/// 部分失敗を報告する。shimは呼び出しlog記録後に失敗するため、失敗paneへの
/// 呼び出しもlogに観測される
// [covers:read-send.send-partial-failure-continues-remaining]
#[test]
fn send_partial_failure_continues_remaining() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, log) = zelper("send-partial-fail");
    cmd.env("FAKE_FAIL_WRITE_CHARS", "terminal_1")
        .args(["send", "1", "2", "--", "hello"])
        .assert()
        .failure()
        .code(6);
    let cs = calls(&log);
    assert!(
        cs.iter()
            .any(|c| c.contains("write-chars -p terminal_1 hello")),
        "calls: {cs:?}"
    );
    // 失敗paneより後の残対象への継続呼び出し
    assert!(
        cs.iter()
            .any(|c| c.contains("write-chars -p terminal_2 hello")),
        "calls: {cs:?}"
    );
}

/// 改行を含むtextはwrite-chars 1回の単呼びで送信され、text全体が単一argv要素として
/// 渡る（paste不使用・行単位の分割呼び出しなし）。shimのlog記録は `echo "$*` で
/// 改行入りargv要素はlogを複数行にまたがらせるため、行分割のcalls()でなく
/// log全体のraw文字列で検証する（log行内に改行が現れるのはargv要素内のみ）
// [covers:read-send.send-multiline-text-single-write-chars]
#[test]
fn send_multiline_text_single_write_chars() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, log) = zelper("send-multiline");
    cmd.args(["send", "1", "--", "first line\nsecond line"])
        .assert()
        .success();
    let raw = std::fs::read_to_string(&log).unwrap();
    // write-charsは1回のみ（行分割呼び出しは2回になるため弾ける）
    assert_eq!(raw.matches("write-chars").count(), 1, "log: {raw:?}");
    assert!(
        raw.contains("action write-chars -p terminal_1 first line\nsecond line"),
        "log: {raw:?}"
    );
    // pasteは未使用
    assert!(!raw.contains("paste"), "log: {raw:?}");
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

// ---------------------------------------------------------------------------
// watch（TASK-86・watch.toml 16条件のうち文法・出力契約・exit分類の10条件。
// loop系6条件はtests/fake_backend/watch.rs。設計§5.3/R6: 実行時間抑制のため
// intervalは明示1・maxは1〜2に寄せる）

/// watchのpattern数検証: PATTERN 0件（`--`の省略時はPATTERNがPANESPECとして
/// 解釈されるため結果的にpattern 0件）と2件はUsage exit 2で拒否する
// [covers:watch.pattern-count-usage]
#[test]
fn watch_pattern_count_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    // PATTERN 2件（READY GO相当の2語指定）
    let (mut cmd, _) = zelper("watch-two-patterns");
    cmd.args(["watch", "1", "--", "READY", "GO"])
        .assert()
        .failure()
        .code(2);

    // PATTERN 0件: `--`以降に何も取らない指定
    let (mut cmd, _) = zelper("watch-no-pattern");
    cmd.args(["watch", "1", "--"]).assert().failure().code(2);

    // PATTERN 0件: `--`全体の省略（READYがPANESPECとして解釈されるため結果的に
    // pattern 0件。panes検証と同様にUsage exit 2）
    let (mut cmd, _) = zelper("watch-no-dash");
    cmd.args(["watch", "1", "READY"]).assert().failure().code(2);
}

/// 空文字列PATTERNの拒否: substring・--regex両形式とも空patternは全行matchの
/// 即時偽hitになるため、polling開始前にUsage exit 2で拒否する
// [covers:watch.empty-pattern-usage]
#[test]
fn watch_empty_pattern_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("watch-empty");
    cmd.args(["watch", "1", "--", ""])
        .assert()
        .failure()
        .code(2);

    let (mut cmd, _) = zelper("watch-empty-regex");
    cmd.args(["watch", "1", "--regex", "--", ""])
        .assert()
        .failure()
        .code(2);
}

/// intervalの下限と有限性: 1.0未満・NaN・無限大・Durationへ変換不能な巨大有限値
/// （1e30）のいずれもUsage exit 2。無限大はinterval >= 1.0の比較を通過するため
/// 有限性検査が、巨大有限値は下限・有限性の両検査を通過するためDuration変換検査が
/// それぞれ独立した分岐（REVIEW-1）
// [covers:watch.interval-lower-bound-usage]
#[test]
fn watch_interval_lower_bound_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    for (tag, val) in [
        ("watch-interval-05", "0.5"),
        ("watch-interval-nan", "nan"),
        ("watch-interval-inf", "inf"),
        ("watch-interval-1e30", "1e30"),
    ] {
        let (mut cmd, _) = zelper(tag);
        cmd.args(["watch", "1", "--interval", val, "--", "READY"])
            .assert()
            .failure()
            .code(2);
    }
}

/// maxの下限: 最大試行回数0はUsage exit 2（1以上の正整数のみ受理）
// [covers:watch.max-lower-bound-usage]
#[test]
fn watch_max_lower_bound_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("watch-max0");
    cmd.args(["watch", "1", "--max", "0", "--", "READY"])
        .assert()
        .failure()
        .code(2);
}

/// --regex指定時のcompile検証: 正規表現として不正なpatternはwatch開始（対象解決・
/// polling）前にUsage exit 2で失敗し、error文にregex compile errorの内容を含む
// [covers:watch.regex-compile-usage]
#[test]
fn watch_regex_compile_error_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("watch-bad-regex");
    cmd.args(["watch", "1", "--regex", "--", "[invalid"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicates::str::contains("regex parse error"));
}

/// watchの対象解決の負荷側: 存在しないpane ID指定（対象0件）はpolling開始前に
/// NoTarget（exit 3）で失敗する
// [covers:watch.no-target-exit3]
#[test]
fn watch_nonexistent_pane_is_no_target_exit3() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("watch-miss");
    cmd.args(["watch", "999", "--", "READY"])
        .assert()
        .failure()
        .code(3);
}

/// hit時の出力契約: JSONはdataに7 field（target/pattern/regex/attempts_used/
/// elapsed_secs/matched_lines/pane_errors）を持ち、humanはread同型のpane識別
/// ヘッダー行+match行すべてを出力する（matched_linesはhit pane内のmatch行すべて）
// [covers:watch.hit-exit0-output]
#[test]
fn watch_hit_exit0_output_json_and_human() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let screen = "booting\nREADY one\nREADY two\nidle";
    // JSON: data 7 field・matched_linesはmatch行両方
    let (mut cmd, _) = zelper("watch-hit-json");
    let out = cmd
        .env("FAKE_DUMP_SCREEN", screen)
        .args(["watch", "1", "--json", "--", "READY"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["target"], "terminal_1");
    assert_eq!(v["data"]["pattern"], "READY");
    assert_eq!(v["data"]["regex"], false);
    assert_eq!(v["data"]["attempts_used"], 1);
    assert_eq!(
        v["data"]["matched_lines"],
        serde_json::json!(["READY one", "READY two"])
    );
    assert_eq!(v["data"]["pane_errors"], serde_json::json!([]));
    assert!(v["data"]["elapsed_secs"].is_f64());

    // human: read同型のpane header + match行両方
    let (mut cmd, _) = zelper("watch-hit-human");
    cmd.env("FAKE_DUMP_SCREEN", screen)
        .args(["watch", "1", "--", "READY"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("=== terminal_1 (agent-a, tab:0 Tab #1) ===")
                .and(predicates::str::contains("READY one"))
                .and(predicates::str::contains("READY two")),
        );
}

/// 回数切れ（pane障害なし）: max到達no hitはOperationFailed（exit 5）で、
/// error.dataにattempts_used・elapsed_secs・patternを報告する。多attemptの内容
/// 変化はshimのFAKE_DUMP_SCREEN_FRAMESで再現（実行時間は--max 2で抑制）
// [covers:watch.exhaust-operationfailed]
#[test]
fn watch_exhaust_is_operation_failed_with_attempts() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("watch-exhaust");
    let out = cmd
        .env(
            "FAKE_DUMP_SCREEN_FRAMES",
            "attempt1 no match\nattempt1 line2@@ATTEMPT@@attempt2 still no match",
        )
        .args([
            "watch",
            "1",
            "--interval",
            "1",
            "--max",
            "2",
            "--json",
            "--",
            "READY",
        ])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["class"], "OperationFailed");
    assert_eq!(v["error"]["data"]["attempts_used"], 2);
    assert_eq!(v["error"]["data"]["pattern"], "READY");
    assert!(v["error"]["data"]["elapsed_secs"].is_f64());
}

/// 回数切れ+一部pane障害の排他契約: 一部paneのdump失敗を伴う回数切れはexit 5で
/// なくPartialFailure（exit 6）に分類され、失敗paneのみのresults[]をerror.dataへ
/// 報告する。系列は条件書givenどおり「pane 1が途中（2回目の呼び出し）から失敗」
/// （FAKE_FAIL_DUMP_SCREEN_AFTER=terminal_1:1・初回はframe応答で成功。REVIEW-1）
// [covers:watch.exhaust-partial-failure]
#[test]
fn watch_exhaust_with_pane_failure_is_partial_failure() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("watch-exhaust-partial");
    let out = cmd
        .env("FAKE_FAIL_DUMP_SCREEN_AFTER", "terminal_1:1")
        .env(
            "FAKE_DUMP_SCREEN_FRAMES",
            "attempt1 ok@@ATTEMPT@@attempt2 ok",
        )
        .args([
            "watch",
            "1",
            "2",
            "--interval",
            "1",
            "--max",
            "2",
            "--json",
            "--",
            "READY",
        ])
        .assert()
        .failure()
        .code(6)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["class"], "PartialFailure");
    let results = v["error"]["data"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 1, "results: {results:?}");
    assert_eq!(results[0]["target"], "terminal_1");
    assert_eq!(results[0]["ok"], false);
    assert!(results[0]["error"].as_str().unwrap().contains("boom"));
}

/// 全pane障害の分類: 対象全paneのdump_screenが失敗する系列はPartialFailure
/// （exit 6）でなくOperationFailed（exit 5）。一部=6・全て=5の失敗範囲集計判定
// [covers:watch.exhaust-all-pane-failure-exit5]
#[test]
fn watch_exhaust_all_pane_failure_is_exit5() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("watch-exhaust-all");
    let out = cmd
        .env("FAKE_FAIL_DUMP_SCREEN", "terminal_1 terminal_2")
        .args([
            "watch",
            "1",
            "2",
            "--interval",
            "1",
            "--max",
            "2",
            "--json",
            "--",
            "READY",
        ])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["class"], "OperationFailed");
}

// ---------------------------------------------------------------------------
// attach / detach / add session（TASK-89・attach-detach.tomlのL3層8条件。
// run_interactive経路系2条件はsrc/zellij/process.rs内test・L2層2条件は
// tests/fake_backend/attach.rs。層帰属は設計§5.2/§5.3・低-9修正）

/// attachのTTY検査fail-fast: 実binaryはtest環境ではstdin・stdoutとも非TTYの
/// ためPreflight（exit 7）で拒否される。M4実測（非TTY素attachはzellij panic
/// exit 101・stdout汚染 / stdout片側pipeではUI出力先不在でblock）に到達させる
/// 前に止めることが目的。誘導文で非対話作成（add session）への迂回を案内する
// [covers:attach-detach.attach-non-tty-preflight]
#[test]
fn attach_non_tty_is_preflight_exit7_fail_fast() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, log) = zelper("attach-nontty");
    let out = cmd
        .args(["--session", "fake-sess", "attach", "--json"])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["class"], "Preflight");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(msg.contains("attach requires a TTY"), "message: {msg}");
    assert!(msg.contains("add session"), "message: {msg}");
    // fail-fast: zellij attach呼出は発生しない（FAKE_LOGにattach行なし）
    let cs = calls(&log);
    assert!(!cs.iter().any(|c| c.starts_with("attach")), "calls: {cs:?}");
}

/// detachのbackend argv契約: session解決済みのsession名で
/// `zellij --session NAME action detach` を実行する（既存run_actionの枠のwrap）
// [covers:attach-detach.detach-argv]
#[test]
fn detach_records_session_action_detach_argv() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, log) = zelper("detach-argv");
    cmd.args(["--session", "fake-sess", "detach"])
        .assert()
        .success()
        .stdout("detached clients from session 'fake-sess'\n");
    let cs = calls(&log);
    assert!(
        cs.iter().any(|c| c == "--session fake-sess action detach"),
        "calls: {cs:?}"
    );
}

/// detachのzellij非zero終了mapping: (a) FAKE_DETACH_FAILによる汎用失敗と
/// (b) 不在session名（shimはrc=1・Session not found + 稼働session候補表示）の
/// 双方がOperationFailed（exit 5）へ変換され、messageへstderr本文が埋め込まれる
/// （候補表示がmessage経由で届く）。Session not foundをNoTargetへ切り替えない
// [covers:attach-detach.detach-fail]
#[test]
fn detach_fail_is_operation_failed_with_stderr() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    // (a) FAKE_DETACH_FAIL: action detachが非zero終了
    let (mut cmd, _) = zelper("detach-fail-generic");
    let out = cmd
        .env("FAKE_DETACH_FAIL", "1")
        .args(["--session", "fake-sess", "detach", "--json"])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "OperationFailed");
    assert!(
        v["error"]["message"].as_str().unwrap().contains("boom"),
        "message: {}",
        v["error"]["message"]
    );

    // (b) 不在session名: rc=1・Session not found + 稼働session候補表示
    let (mut cmd, _) = zelper("detach-fail-notfound");
    let out = cmd
        .args(["--session", "ghost-sess", "detach", "--json"])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "OperationFailed");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("Session 'ghost-sess' not found"),
        "message: {msg}"
    );
    assert!(msg.contains("fake-sess"), "message: {msg}");
}

/// add session成功経路: 事前確認（同名なし）→ positional形式 `attach -b NAME`
/// （--session接頭なしのtop-level argv）→ postcondition（作成後のlist-sessionsに
/// NAMEがrunning出現）→ 報告。shimのFAKE_SESSIONS_CREATE=okは作成NAMEを
/// list-sessions応答へ反映する（statelessだとpostconditionが常に不成立になるため）
// [covers:attach-detach.add-session-argv-postcondition]
#[test]
fn add_session_creates_detached_and_reports() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    // human: `created session 'NAME' (detached)` の1行
    let (mut cmd, log) = zelper("add-session-human");
    cmd.env("FAKE_SESSIONS_CREATE", "ok")
        .args(["add", "session", "work"])
        .assert()
        .success()
        .stdout("created session 'work' (detached)\n");
    let cs = calls(&log);
    assert!(cs.iter().any(|c| c == "attach -b work"), "calls: {cs:?}");
    // 例外dispatch: 対象session解決（mk_backend）を経ないため--session接頭のargv無し
    assert!(
        !cs.iter().any(|c| c.starts_with("--session")),
        "calls: {cs:?}"
    );

    // JSON: envelope 1通でdata.session=NAME・data.created=true
    let (mut cmd, log) = zelper("add-session-json");
    let out = cmd
        .env("FAKE_SESSIONS_CREATE", "ok")
        .args(["add", "session", "work2", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["session"], "work2");
    assert_eq!(v["data"]["created"], true);
    assert!(
        calls(&log).iter().any(|c| c == "attach -b work2"),
        "calls: {:?}",
        calls(&log)
    );
}

/// add sessionの同名事前確認: list_sessionsに同名running sessionが存在する場合は
/// 作成を試みずPreflight（exit 7）で拒否する（zellij側rc=1を待たないfail-fast）
// [covers:attach-detach.add-session-already-exists]
#[test]
fn add_session_existing_running_name_is_preflight() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, log) = zelper("add-session-exists");
    let out = cmd
        .env("FAKE_SESSIONS", "work [Created 2m ago] (current)")
        .args(["add", "session", "work", "--json"])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "Preflight");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("session already exists"),
        "message: {}",
        v["error"]["message"]
    );
    // fail-fast: attach -b argvは発生しない
    let cs = calls(&log);
    assert!(
        !cs.iter().any(|c| c.starts_with("attach -b")),
        "calls: {cs:?}"
    );
}

/// add sessionの0-session環境動作: 対象session解決を経ない例外dispatchにより、
/// list-sessions応答が0件（shimは0-session実環境どおりrc=1で失敗）の環境でも
/// 作成が成立する。共通経路（mk_backend）のまま実装するとresolve_sessionが
/// NoTargetで失敗する回帰の検出。成功にはpostconditionの通過を含む
// [covers:attach-detach.add-session-no-session-env]
#[test]
fn add_session_works_in_zero_session_environment() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, log) = zelper("add-session-zeroenv");
    let out = cmd
        .env("FAKE_SESSIONS_EMPTY", "1")
        .env("FAKE_SESSIONS_CREATE", "ok")
        .args(["add", "session", "work", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["session"], "work");
    assert_eq!(v["data"]["created"], true);
    // positional形式 `attach -b NAME`（--session接頭なし）
    assert!(
        calls(&log).iter().any(|c| c == "attach -b work"),
        "calls: {:?}",
        calls(&log)
    );
}

/// add sessionの作成失敗mapping: 事前確認を通過した後のattach -b非zero終了
/// （EXITED同名衝突・race等）はOperationFailed（exit 5）へ変換され、messageへ
/// stderr本文（Session already exists）が埋め込まれる（M5 (e1)実測の再現）
// [covers:attach-detach.add-session-create-fail-operation-failed]
#[test]
fn add_session_create_fail_is_operation_failed() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, _) = zelper("add-session-createfail");
    let out = cmd
        .env("FAKE_SESSIONS_CREATE", "already-exists")
        .args(["add", "session", "work", "--json"])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "OperationFailed");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Session already exists"),
        "message: {}",
        v["error"]["message"]
    );
}

/// add sessionのpostcondition検証: attach -bがrc=0でも作成後のlist_sessionsに
/// NAMEがrunning出現しない場合はVerificationFailed（exit 7）で失敗報告する
/// （作成成否をexit codeのみで信用しない。shimはFAKE_SESSIONS_CREATE=ok-muteで
/// rc=0・list-sessions応答不変の系列を再現）
// [covers:attach-detach.add-session-postcondition-missing-verification-failed]
#[test]
fn add_session_postcondition_missing_is_verification_failed() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
    let (mut cmd, log) = zelper("add-session-postmiss");
    let out = cmd
        .env("FAKE_SESSIONS_CREATE", "ok-mute")
        .args(["add", "session", "work", "--json"])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "VerificationFailed");
    // attach -b自体は発行済み（rc=0・list-sessions不変の系列）
    assert!(
        calls(&log).iter().any(|c| c == "attach -b work"),
        "calls: {:?}",
        calls(&log)
    );
}
