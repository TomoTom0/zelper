// L3: remap CLI契約テスト（test-plan §2.7 (g) R43〜R45・(h) R46・(i) R48〜R50出力側。
// DD-10 v2 / DD-12）。fake zellij shimをtest実行時に生成する（実zellijに依存しない）。
// fail-first: v2実装（TASK-39）前にfailする（test-plan §5）。
use assert_cmd::Command;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// 3-slot bare layout（k=1系テスト用。inlineで渡す）
const THREE_SLOT: &str =
    "layout {\n    tab {\n        pane\n        pane\n        pane\n    }\n}\n";

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zellij")
}

/// fake zellij実行可能fileをworkdirに生成し、(PATHに設定すべきdir, log path)を返す。
/// FAKE_VERSION envで --version 応答を、FAKE_PANES/FAKE_TABS envでfixtureを差し替え
fn setup_fake_zellij(tag: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("zelper-fake-remap-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let shim = dir.join("zellij");
    let script = r#"#!/usr/bin/env bash
echo "$*" >> "$FAKE_LOG"
if [[ "$1" == "--version" ]]; then echo "zellij ${FAKE_VERSION:-0.44.3}"; exit 0; fi
if [[ "$1" == "list-sessions" ]]; then echo "fake-sess [Created 1m ago]"; exit 0; fi
if [[ "$3" == "action" ]]; then
  shift 3
  action="$1"; shift
  case "$action" in
    list-panes) cat "$FAKE_FIXTURES/${FAKE_PANES:-panes.json}"; exit 0;;
    list-tabs) cat "$FAKE_FIXTURES/${FAKE_TABS:-tabs.json}"; exit 0;;
    current-tab-info)
      # anchor = tab 0（fixture最初のtab。単体object形式）
      printf '{"position":0,"name":"Tab #1","active":true,"panes_to_hide":0,"is_fullscreen_active":false,"is_sync_panes_active":false,"are_floating_panes_visible":true,"other_focused_clients":null,"active_swap_layout_name":null,"is_swap_layout_dirty":false,"viewport_rows":60,"viewport_columns":200,"display_area_rows":60,"display_area_columns":200,"selectable_tiled_panes_count":3,"selectable_floating_panes_count":0,"tab_id":0,"has_bell_notification":null,"is_flashing_bell":false}\n'
      exit 0;;
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

/// 3-pane同一tab fixture（k=1・移動不要）。廃止flag拒否後の正常系検証に使う。
/// 既定panes.jsonはselectable terminal paneが2つのため、M=N=3（k=1）になる
/// 3-pane fixtureを向ける
fn zelper(tag: &str) -> Command {
    let mut cmd = zelper_with(tag, None, false);
    cmd.env("FAKE_PANES", "panes-three.json");
    cmd
}

/// versionとfixtureを差し替える（R46/R48〜R50）
fn zelper_with(tag: &str, version: Option<&str>, remap_fixture: bool) -> Command {
    let (dir, _log) = setup_fake_zellij(tag);
    let mut cmd = Command::cargo_bin("zelper").unwrap();
    let path = format!(
        "{}:{}",
        dir.to_str().unwrap(),
        std::env::var("PATH").unwrap_or_default()
    );
    cmd.env("PATH", path)
        .env_remove("ZELLIJ_SESSION_NAME")
        .env("FAKE_LOG", dir.join("calls.log"))
        .env("FAKE_FIXTURES", fixture_dir());
    if let Some(v) = version {
        cmd.env("FAKE_VERSION", v);
    }
    if remap_fixture {
        // 4 pane / 2 tab（M=4, N=3 -> k=2。dry-run出力の検証用）
        cmd.env("FAKE_PANES", "panes-remap.json")
            .env("FAKE_TABS", "tabs-remap.json");
    }
    cmd
}

/// stdoutを行arrayにする
fn stderr_lines(out: &std::process::Output) -> Vec<String> {
    String::from_utf8(out.stderr.clone())
        .unwrap()
        .lines()
        .map(|l| l.to_string())
        .collect()
}

// ---- (g) 廃止flag R43〜R45（DD-12。旧no-op受理仕様からv0.1.xで削除へ反転済み） ----

// [covers:remap-cli.r43-removed-flags-are-usage-error-exit2]
#[test]
fn r43_removed_flags_are_usage_error_exit2() {
    // --overflow nest / --overflow tabs / --session-scope は削除済み。
    // 指定はusage error（exit 2）でremap本体は実行されない
    for extra in [
        vec!["--overflow", "nest"],
        vec!["--overflow", "tabs"],
        vec!["--session-scope"],
    ] {
        let mut cmd = zelper("r43");
        let mut args = vec!["remap", "--inline", THREE_SLOT];
        args.extend(extra.iter().copied());
        let out = cmd
            .args(&args)
            .assert()
            .failure()
            .code(2)
            .get_output()
            .clone();
        assert!(
            out.stdout.is_empty(),
            "stdoutには何も出ない: {:?}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert!(!stderr_lines(&out).is_empty(), "usage errorはstderrへ出る");
    }
}

// [covers:remap-cli.r44-removed-flag-json-keeps-stdout-empty]
#[test]
fn r44_removed_flag_json_keeps_stdout_empty() {
    // --json併用時もusage errorはclap経由でstderrのみ。stdoutには何も出ない
    // （単一envelope契約を破るstdout出力は存在しない）
    let mut cmd = zelper("r44");
    let out = cmd
        .args(["remap", "--inline", THREE_SLOT, "--session-scope", "--json"])
        .assert()
        .failure()
        .code(2)
        .get_output()
        .clone();
    assert!(
        out.stdout.is_empty(),
        "stdout空（--json時もerror envelopeではなくclap usage error）: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(!stderr_lines(&out).is_empty(), "usage errorはstderrへ出る");
}

// [covers:remap-cli.r45-removed-flags-absent-from-help]
#[test]
fn r45_removed_flags_absent_from_help() {
    // 削除済みoptionはclap helpに出現しない
    let mut cmd = zelper("r45");
    let out = cmd
        .args(["remap", "--help"])
        .assert()
        .success()
        .get_output()
        .clone();
    let help = String::from_utf8(out.stdout).unwrap();
    assert!(!help.contains("--overflow"), "raw: {help}");
    assert!(!help.contains("--session-scope"), "raw: {help}");
    // 対照: 廃止対象外のoptionは見える
    assert!(help.contains("--embed-floating"), "raw: {help}");
}

// ---- (h) version gate R46（DD-3.1/3.5。最小0.44.3） ----

// [covers:remap-cli.r46-below-minimum-and-future-series-exit4]
#[test]
fn r46_below_minimum_and_future_series_exit4() {
    // zellij <0.44.3（0.44.2 / 0.43.1）-> UnsupportedVersion exit 4。
    // 実証済み0.44.x系列超過（0.45.0等）も同様
    for v in ["0.44.2", "0.43.1", "0.45.0"] {
        let mut cmd = zelper_with("r46", Some(v), false);
        cmd.args(["remap", "--inline", THREE_SLOT])
            .assert()
            .failure()
            .code(4);
    }
}

// ---- (i) dry-run出力契約 R48〜R50（状態変更なしR47/R50計画一致はL2） ----

// [covers:remap-cli.r48-dry-run-json-reports-source-mnk-and-partition]
#[test]
fn r48_dry_run_json_reports_source_mnk_and_partition() {
    // source一覧（visual order・現在tab付き）・M/N/k・pane割当表（全件preserved・
    // empty slot数）を出力。4 pane / 3-slot -> M=4, N=3, k=2
    let mut cmd = zelper_with("r48", None, true);
    let out = cmd
        .args(["remap", "--inline", THREE_SLOT, "--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    let data = &v["data"];
    assert_eq!(data["m"], 4, "raw: {data}");
    assert_eq!(data["n"], 3, "raw: {data}");
    assert_eq!(data["k"], 2, "raw: {data}");
    // source一覧: 4件・visual order・現在tab付き
    let source = data["source"].as_array().cloned().unwrap_or_default();
    assert_eq!(source.len(), 4, "raw: {data}");
    // 割当表: 全4件preserved（instance 0 = 3件・instance 1 = 1件 + empty 2）
    let instances = data["instances"].as_array().cloned().unwrap_or_default();
    assert_eq!(instances.len(), 2, "raw: {data}");
    let assigned: usize = instances
        .iter()
        .filter_map(|i| i["assignments"].as_array().map(|a| a.len()))
        .sum();
    assert_eq!(assigned, 4, "全paneが割当表に載る（全件preserved）: {data}");
    assert_eq!(instances[1]["empty_slots"], 2, "raw: {data}");
}

// [covers:remap-cli.r49-dry-run-shows-kdl-preview-per-instance]
#[test]
fn r49_dry_run_shows_kdl_preview_per_instance() {
    // 生成KDL preview（instance毎。humanは [plan] block・--jsonは instances[].kdl）
    let mut cmd = zelper_with("r49", None, true);
    let out = cmd
        .args(["remap", "--inline", THREE_SLOT, "--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let instances = v["data"]["instances"].as_array().cloned().unwrap();
    assert_eq!(instances.len(), 2);
    for inst in &instances {
        let kdl = inst["kdl"]
            .as_str()
            .unwrap_or_else(|| panic!("instances[].kdl に生成KDL preview: {inst}"));
        // 3-slotの生成KDL（pane node 3つ）
        assert!(kdl.matches("pane").count() >= 3, "raw: {kdl}");
    }

    // human形式は [plan] block
    let mut cmd = zelper_with("r49h", None, true);
    let out = cmd
        .args(["remap", "--inline", THREE_SLOT, "--dry-run"])
        .assert()
        .success()
        .get_output()
        .clone();
    let human = String::from_utf8(out.stdout).unwrap();
    assert!(human.contains("[plan]"), "raw: {human}");
}

// [covers:remap-cli.r50-dry-run-lists-planned-operation-kinds]
#[test]
fn r50_dry_run_lists_planned_operation_kinds() {
    // 実行予定backend操作列の種別表示（companion setup・probe pipe・break pipe・
    // rename・go-to・override）。M=4/N=3/k=2では全種別が計画に現れる
    let mut cmd = zelper_with("r50", None, true);
    let out = cmd
        .args(["remap", "--inline", THREE_SLOT, "--dry-run"])
        .assert()
        .success()
        .get_output()
        .clone();
    let human = String::from_utf8(out.stdout).unwrap();
    for kind in ["companion", "probe", "break", "rename", "go-to", "override"] {
        assert!(
            human.contains(kind),
            "操作列種別 {kind} が表示される: {human}"
        );
    }
}

// [covers:remap-cli.both-removed-flags-together-still-usage-error]
#[test]
fn c7_both_removed_flags_together_still_usage_error() {
    // --session-scopeと--overflowの同時指定も単一のusage errorで拒否される
    // （C7回帰: 旧warning出力の統合問題はoption削除で消滅）
    let mut cmd = zelper("c7");
    let out = cmd
        .args([
            "remap",
            "--inline",
            THREE_SLOT,
            "--session-scope",
            "--overflow",
            "tabs",
            "--json",
        ])
        .assert()
        .failure()
        .code(2)
        .get_output()
        .clone();
    assert!(
        out.stdout.is_empty(),
        "stdoutには何も出ない: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
}
