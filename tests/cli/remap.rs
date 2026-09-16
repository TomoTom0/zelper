// L3: remap CLI契約テスト（test-plan §2.7 (g) R43〜R45・(h) R46・(i) R48〜R50出力側。
// DD-10 v2 / DD-12）。fake zellij shimをtest実行時に生成する（実zellijに依存しない）。
// fail-first: v2実装（TASK-39）前にfailする（test-plan §5）。
use assert_cmd::Command;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// fake zellij shimのwrite↔exec競合（ETXTBSY）対策: binary内のfake zellij
/// testを直列化する（CR59-5・TASK-73。src/zellij/process.rs test moduleと同一構成+契約doc comment）。
/// test fn冒頭でのみ取得すること。wrapper・helper内では再取得しないこと
/// （std::sync::Mutexは非reentrantのため同一thread再取得は永久blockし、
/// failではなくtest hangとして発覚する）。
static FAKE_ZELLIJ_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 3-slot bare layout（k=1系テスト用。inlineで渡す）
const THREE_SLOT: &str =
    "layout {\n    tab {\n        pane\n        pane\n        pane\n    }\n}\n";

/// 3-slot layoutにdefault_tab_template（children + compact-bar bar leaf）を付けた
/// layout（bar leaf出現の検証用。inlineで渡す）
const TEMPLATE_SLOT: &str = "layout {\n    default_tab_template {\n        children\n        pane size=1 borderless=true {\n            plugin location=\"zellij:compact-bar\"\n        }\n    }\n    tab {\n        pane\n        pane\n        pane\n    }\n}\n";

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
    // 実行内容とmode確定後にのみ公開する。最終の実行pathはwrite open状態にならない
    // （LinuxのETXTBSY回避）。
    let staged = dir.join("zellij.staged");
    std::fs::write(&staged, script).unwrap();
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::rename(staged, &shim).unwrap();
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
    // fail-first期のfail testがpanicでlockを毒化しても、shim公開dirはtag毎に
    // 分離されるため毒化回復しても後続testへ影響しない（直列化のみ維持。
    // tests/fake_backend/remap.rsのXDG_LOCKと同一構成）
    let _guard = FAKE_ZELLIJ_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    // fail-first期のfail testがpanicでlockを毒化しても、shim公開dirはtag毎に
    // 分離されるため毒化回復しても後続testへ影響しない（直列化のみ維持。
    // tests/fake_backend/remap.rsのXDG_LOCKと同一構成）
    let _guard = FAKE_ZELLIJ_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    // fail-first期のfail testがpanicでlockを毒化しても、shim公開dirはtag毎に
    // 分離されるため毒化回復しても後続testへ影響しない（直列化のみ維持。
    // tests/fake_backend/remap.rsのXDG_LOCKと同一構成）
    let _guard = FAKE_ZELLIJ_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    // fail-first期のfail testがpanicでlockを毒化しても、shim公開dirはtag毎に
    // 分離されるため毒化回復しても後続testへ影響しない（直列化のみ維持。
    // tests/fake_backend/remap.rsのXDG_LOCKと同一構成）
    let _guard = FAKE_ZELLIJ_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    // fail-first期のfail testがpanicでlockを毒化しても、shim公開dirはtag毎に
    // 分離されるため毒化回復しても後続testへ影響しない（直列化のみ維持。
    // tests/fake_backend/remap.rsのXDG_LOCKと同一構成）
    let _guard = FAKE_ZELLIJ_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    assert!(
        source.iter().all(|p| p.get("invoked_with").is_some()),
        "raw: {data}"
    );
    assert_eq!(
        source[0]["invoked_with"],
        serde_json::Value::Null,
        "raw: {data}"
    );
    assert_eq!(source[1]["invoked_with"], "claude --model x", "raw: {data}");
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
    // fail-first期のfail testがpanicでlockを毒化しても、shim公開dirはtag毎に
    // 分離されるため毒化回復しても後続testへ影響しない（直列化のみ維持。
    // tests/fake_backend/remap.rsのXDG_LOCKと同一構成）
    let _guard = FAKE_ZELLIJ_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
        // template無しlayoutはbar leaf（plugin leaf）を出力しない
        assert!(!kdl.contains("plugin location"), "raw: {kdl}");
    }
    assert!(
        instances[0]["kdl"]
            .as_str()
            .unwrap()
            .contains("command=\"claude\"")
    );
    assert!(
        !instances[0]["kdl"]
            .as_str()
            .unwrap()
            .contains("command=\"vim\"")
    );

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
    assert!(human.contains(" inv:"), "source表示はinv:を使う: {human}");

    // default_tab_template付きlayout: bar leaf（compact-bar）が出現し、
    // boolean属性はquoteなし真偽値（borderless=true）・数値属性 size=1 も
    // quoteなし整数で出力される。quote付き文字列形式（borderless="true"）は
    // zellij 0.44.3 layout parserに「borderless should be either true or false,
    // found "true"」として拒否される（TASK-74 E2E run3実測:
    // tmp/task74/acceptance/b_remap.json）
    let mut cmd = zelper_with("r49t", None, true);
    let out = cmd
        .args(["remap", "--inline", TEMPLATE_SLOT, "--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let instances = v["data"]["instances"].as_array().cloned().unwrap();
    assert_eq!(instances.len(), 2, "raw: {v}");
    let kdl = instances[0]["kdl"]
        .as_str()
        .unwrap_or_else(|| panic!("template付きlayoutもinstances[].kdlにpreview: {instances:?}"));
    assert!(
        kdl.contains("plugin location=\"zellij:compact-bar\""),
        "raw: {kdl}"
    );
    assert!(kdl.contains("borderless=true"), "raw: {kdl}");
    assert!(kdl.contains("size=1"), "raw: {kdl}");
    assert!(
        !kdl.contains("borderless=\"true\""),
        "quote付きboolean属性はzellij parserに拒否される: {kdl}"
    );
    assert!(
        !kdl.contains("size=\"1\""),
        "数値属性はquoteなし整数で出す: {kdl}"
    );
}

// [covers:remap-cli.r50-dry-run-lists-planned-operation-kinds]
#[test]
fn r50_dry_run_lists_planned_operation_kinds() {
    // fail-first期のfail testがpanicでlockを毒化しても、shim公開dirはtag毎に
    // 分離されるため毒化回復しても後続testへ影響しない（直列化のみ維持。
    // tests/fake_backend/remap.rsのXDG_LOCKと同一構成）
    let _guard = FAKE_ZELLIJ_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    // TASK-75（v2.2）: 種別にはfocus-pane-idと最終go-to先が加わる。最終go-to先は
    // go-to種別の表示に含まれるため "go-to" で担保する
    for kind in [
        "companion",
        "probe",
        "break",
        "rename",
        "go-to",
        "override",
        "focus-pane-id",
    ] {
        assert!(
            human.contains(kind),
            "操作列種別 {kind} が表示される: {human}"
        );
    }
}

// [covers:remap-cli.both-removed-flags-together-still-usage-error]
#[test]
fn c7_both_removed_flags_together_still_usage_error() {
    // fail-first期のfail testがpanicでlockを毒化しても、shim公開dirはtag毎に
    // 分離されるため毒化回復しても後続testへ影響しない（直列化のみ維持。
    // tests/fake_backend/remap.rsのXDG_LOCKと同一構成）
    let _guard = FAKE_ZELLIJ_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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

// ---- TASK-75（DD-10 v2.2: multi-tab layout全体再現） ----

/// T=3 multi-tab layout（tab0 "T-3x3" focus=true 9 slot・tab1 "T-single" 1 slot・
/// tab2 名無し 2 slot。N_t=9,1,2 → S=12）
const MULTI_TAB_LAYOUT: &str = "layout {\n    tab name=\"T-3x3\" focus=true {\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n    }\n    tab name=\"T-single\" {\n        pane\n    }\n    tab {\n        pane\n        pane\n    }\n}\n";

/// M=15のhetero fixture（anchor単独tab 15 pane。S=12 → k=2・block 1のtab1/tab2は
/// 割当0件の空group）でzelperを起動
fn zelper_hetero(tag: &str) -> Command {
    let mut cmd = zelper_with(tag, None, false);
    cmd.env("FAKE_PANES", "panes-hetero.json")
        .env("FAKE_TABS", "tabs-hetero.json");
    cmd
}

/// MULTI_TAB_LAYOUTをworkdirへ書き出し、そのpathを返す（--path指定用。
/// file stem "hetero" が生成tab名のbaseになる）
fn hetero_layout_file(tag: &str) -> PathBuf {
    let (dir, _log) = setup_fake_zellij(tag);
    let path = dir.join("hetero.kdl");
    std::fs::write(&path, MULTI_TAB_LAYOUT).unwrap();
    path
}

// [covers:remap-cli.dry-run-reports-s-and-per-tab-slots]
#[test]
fn dry_run_reports_s_and_per_tab_slots() {
    // fail-first期のfail testがpanicでlockを毒化しても、shim公開dirはtag毎に
    // 分離されるため毒化回復しても後続testへ影響しない（直列化のみ維持。
    // tests/fake_backend/remap.rsのXDG_LOCKと同一構成）
    let _guard = FAKE_ZELLIJ_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // dry-run出力のmulti-tab拡張: humanはM/S/kに加えTとper-tab slot数列N_tを表示し、
    // 割当表は pane → (block, tab, slot, 生成tab名)、KDL previewは生成tab (b,t)毎、
    // 操作列種別にnew-tab（空group）・focus-pane-id・最終go-to先が加わる
    let layout = hetero_layout_file("mt-h");
    let layout_str = layout.to_str().unwrap().to_string();
    let mut cmd = zelper_hetero("mt-h");
    let out = cmd
        .args(["remap", "--path", &layout_str, "--dry-run"])
        .assert()
        .success()
        .get_output()
        .clone();
    let human = String::from_utf8(out.stdout).unwrap();
    assert!(human.contains("12"), "S=12（layout全体slot数）: {human}");
    assert!(human.contains("9,1,2"), "per-tab slot数列 N_t: {human}");
    assert!(
        human.contains("T-single"),
        "block 0 tab 1の生成tab名（鋳型名幹）: {human}"
    );
    assert!(
        human.contains("new-tab"),
        "操作列にnew-tab（空group）種別: {human}"
    );
    assert!(
        human.contains("focus-pane-id"),
        "操作列にfocus-pane-id種別: {human}"
    );

    // --jsonも同構成（数値field契約は json-n-field-holds-total-slots が管轄）
    let mut cmd = zelper_hetero("mt-hj");
    let layout = hetero_layout_file("mt-hj");
    let layout_str = layout.to_str().unwrap().to_string();
    let out = cmd
        .args(["remap", "--path", &layout_str, "--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let data = &v["data"];
    assert_eq!(data["m"], 15, "raw: {data}");
    assert_eq!(data["k"], 2, "raw: {data}");
    assert_eq!(data["t"], 3, "T（鋳型数）: {data}");
    assert_eq!(data["s"], 12, "S: {data}");
    assert_eq!(data["n_slots"], serde_json::json!([9, 1, 2]), "raw: {data}");
    assert!(
        data["tabs"].as_array().is_some_and(|t| t.len() == 3),
        "鋳型のname/focus（3鋳型）: {data}"
    );
    // 割当表は生成tab (b,t)毎: 6生成tab分のplan（block 1 tab1/tab2は空group含む）
    let instances = data["instances"].as_array().cloned().unwrap_or_default();
    assert_eq!(instances.len(), 6, "(b,t)毎のplan block: {data}");
    // 生成tab名: block 0は鋳型名幹・block 1は-2接尾（名無し鋳型はbase幹）
    assert!(
        instances.iter().any(|i| i.get("name").is_some()),
        "割当表/planに生成tab名が載る: {data}"
    );
}

/// MULTI_TAB_LAYOUTのtab 2（名無し鋳型）にもfocus=trueを付けたlayout
/// （複数focus=true鋳型のwarning伝達経路検証用）
const MULTI_TAB_DUAL_FOCUS: &str = "layout {\n    tab name=\"T-3x3\" focus=true {\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n    }\n    tab name=\"T-single\" {\n        pane\n    }\n    tab focus=true {\n        pane\n        pane\n    }\n}\n";

// [covers:remap-cli.preflight-warning-paths-stderr-and-json]
#[test]
fn dry_run_multiple_focus_warning_paths() {
    // fail-first期のfail testがpanicでlockを毒化しても、shim公開dirはtag毎に
    // 分離されるため毒化回復しても後続testへ影響しない（直列化のみ維持。
    // tests/fake_backend/remap.rsのXDG_LOCKと同一構成）
    let _guard = FAKE_ZELLIJ_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // preflight warningの伝達経路（E2E vb退化の回帰固定・2026-09-16）: 複数の
    // focus=true鋳型（tab focus）を持つlayoutでは、--json dry-runでもstderrの
    // warning行へ出力され（stdoutのenvelope契約を壊さない）、機械可読経路として
    // data.warningsへ載る
    let layout = hetero_layout_file("mt-w");
    std::fs::write(&layout, MULTI_TAB_DUAL_FOCUS).unwrap();
    let layout_str = layout.to_str().unwrap().to_string();
    let mut cmd = zelper_hetero("mt-w");
    let out = cmd
        .args(["remap", "--path", &layout_str, "--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stderr = String::from_utf8(out.stderr.clone()).unwrap();
    assert!(
        stderr.contains("warning:") && stderr.to_lowercase().contains("focus"),
        "複数focus=true鋳型のwarningはstderrへ出る（--json実行時も）: {stderr}"
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true, "stderrへのwarningはenvelope契約を壊さない");
    let warnings = v["data"]["warnings"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        warnings.iter().any(|w| w
            .as_str()
            .is_some_and(|s| s.to_lowercase().contains("focus"))),
        "data.warningsへ複数focus検出warningが載る: {}",
        v["data"]
    );

    // 対照: focus=true鋳型1件のMULTI_TAB_LAYOUTではwarningは出ない・data.warningsは空
    let layout = hetero_layout_file("mt-wc");
    let layout_str = layout.to_str().unwrap().to_string();
    let mut cmd = zelper_hetero("mt-wc");
    let out = cmd
        .args(["remap", "--path", &layout_str, "--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stderr = String::from_utf8(out.stderr.clone()).unwrap();
    assert!(
        !stderr.contains("warning:"),
        "focus=true鋳型1件ならwarningなし: {stderr}"
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        v["data"]["warnings"],
        serde_json::json!([]),
        "raw: {}",
        v["data"]
    );
}

// [covers:remap-cli.json-n-field-holds-total-slots]
#[test]
fn json_n_field_holds_total_slots() {
    // fail-first期のfail testがpanicでlockを毒化しても、shim公開dirはtag毎に
    // 分離されるため毒化回復しても後続testへ影響しない（直列化のみ維持。
    // tests/fake_backend/remap.rsのXDG_LOCKと同一構成）
    let _guard = FAKE_ZELLIJ_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // JSON数値fieldのS格納互換: n fieldにはS（layout全体slot数）を格納。T=1では
    // S=Nのため従来値と同一。新規field t・s・n_slots・tabsが加わり、mapping/assignments
    // 系のkeyにblockとtab_indexが加わる（instance keyはblock値で維持し削除しない）
    let layout = hetero_layout_file("mt-n");
    let layout_str = layout.to_str().unwrap().to_string();
    let mut cmd = zelper_hetero("mt-n");
    let out = cmd
        .args(["remap", "--path", &layout_str, "--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["ok"], true);
    let data = &v["data"];
    assert_eq!(data["n"], 12, "nにはS=12を格納: {data}");
    assert_eq!(data["t"], 3, "raw: {data}");
    assert_eq!(data["s"], 12, "raw: {data}");
    assert_eq!(data["n_slots"], serde_json::json!([9, 1, 2]), "raw: {data}");
    assert!(
        data["tabs"].as_array().is_some_and(|t| !t.is_empty()),
        "鋳型のname/focus: {data}"
    );
    // mapping/assignments系のkeyへblock/tab_index追加（instanceは維持）
    let instances = data["instances"].as_array().cloned().unwrap_or_default();
    assert!(
        instances.iter().all(|i| i.get("block").is_some()),
        "instancesへblock key追加: {data}"
    );
    assert!(
        instances
            .iter()
            .flat_map(|i| i["assignments"].as_array().cloned().unwrap_or_default())
            .all(|a| a.get("tab_index").is_some()),
        "assignmentsへtab_index key追加: {data}"
    );
    assert!(
        instances.iter().all(|i| i.get("index").is_some()),
        "instance key（block値で維持）は削除しない: {data}"
    );

    // T=1（N=3）: n=3は従来値Nと同一・t=1・n_slots=[3]
    let mut cmd = zelper_with("mt-n1", None, true);
    let out = cmd
        .args(["remap", "--inline", THREE_SLOT, "--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let data = &v["data"];
    assert_eq!(data["n"], 3, "T=1ではS=N=3（従来値と同一）: {data}");
    assert_eq!(data["t"], 1, "raw: {data}");
    assert_eq!(data["n_slots"], serde_json::json!([3]), "raw: {data}");
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["ok"], true);
}
