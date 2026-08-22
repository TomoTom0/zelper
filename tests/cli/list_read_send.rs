// L3: CLI契約テスト（list/read/send。test-plan §2.1/2.4/2.10）
// fake zellij shimをtest実行時に生成し、PATHへ差し込む（実zellijに依存しない）
use assert_cmd::Command;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

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
if [[ "$1" == "list-sessions" ]]; then echo "fake-sess [Created 1m ago]"; exit 0; fi
if [[ "$3" == "action" ]]; then
  shift 3
  action="$1"; shift
  case "$action" in
    list-panes) cat "$FAKE_FIXTURES/panes.json"; exit 0;;
    list-tabs) cat "$FAKE_FIXTURES/tabs.json"; exit 0;;
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
    std::fs::write(&shim, script).unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
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

#[test]
fn list_panes_json_contract() {
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

#[test]
fn list_sessions_and_tabs() {
    let (mut cmd, _) = zelper("list-sessions");
    let out = cmd
        .args(["list", "sessions", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["data"]["sessions"][0], "fake-sess");

    let (mut cmd, _) = zelper("list-tabs");
    cmd.args(["list", "tabs"])
        .assert()
        .success()
        .stdout("TAB_ID\tPOS\tACTIVE\tNAME\n0\t0\t*\tTab #1\n");
}

#[test]
fn read_single_and_multi_with_tail() {
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

#[test]
fn read_nonexistent_pane_is_no_target_exit3() {
    let (mut cmd, _) = zelper("read-miss");
    cmd.args(["read", "999", "--json"])
        .assert()
        .failure()
        .code(3);
}

#[test]
fn json_error_envelope_on_failure() {
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

#[test]
fn send_broadcast_records_calls_in_visual_order() {
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

#[test]
fn send_enter_appends_cr_and_keys_use_send_keys() {
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

#[test]
fn send_without_double_dash_is_usage_error() {
    let (mut cmd, _) = zelper("send-nodash");
    cmd.args(["send", "1", "y"]).assert().failure().code(2);
}

#[test]
fn send_text_and_keys_conflict_is_usage_error() {
    let (mut cmd, _) = zelper("send-conflict");
    cmd.args(["send", "1", "--keys", "Enter", "--", "y"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn remap_layout_sources_conflict_is_usage_error() {
    let (mut cmd, _) = zelper("remap-conflict");
    cmd.args(["remap", "three", "--path", "./x.kdl"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn invalid_pane_spec_is_usage_error() {
    let (mut cmd, _) = zelper("bad-spec");
    cmd.args(["read", "pane:12"]).assert().failure().code(2);
}

#[test]
fn completion_generates_script() {
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

#[test]
fn human_read_output_has_pane_headers() {
    let (mut cmd, _) = zelper("read-human");
    cmd.args(["read", "1"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "=== terminal_1 (agent-a, tab:0 Tab #1) ===",
        ));
}
