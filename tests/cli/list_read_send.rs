// L3: CLI契約テスト（list/read/send/watch。test-plan §2.1/2.4/2.10。watchはTASK-86）
// fake zellij shim（tests/cli/shim_common.rs正本をtest実行時に生成しPATHへ差し込む。
// 実zellijに依存しない。shim機構一覧・命名規則の正本はshim_common.rsのdoc comment）
mod shim_common;
use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use shim_common::{FAKE_ZELLIJ_LOCK, fixture_dir, setup_fake_zellij};
use std::path::PathBuf;

fn zelper(tag: &str) -> (Command, PathBuf) {
    let (dir, log) = setup_fake_zellij(tag);
    (command_on(&dir, &log), log)
}

/// 既存のshim環境（dir/log）に対する2回目以降のzeller呼出し（TASK-98）。
/// 同一shim状態file（${FAKE_LOG}.deleted等）を参照し続けるため、remove実行後の
/// list-sessions応答反映を同一環境で観測できる。zelper()の再呼び出しはsetup時の
/// 状態file初期化（.deleted truncate等）で反映状態が消えるため使えない
fn command_on(dir: &std::path::Path, log: &std::path::Path) -> Command {
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
        .env("FAKE_LOG", log)
        .env("FAKE_FIXTURES", fixture_dir());
    cmd
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
/// 前提とせず一覧表示できる（実行中のみを表示）。TASK-99: 名前2語目にmarker
/// 部分文字列（EXITED）を含むrunning行もexited汚染なしで一覧に現れる。
/// TASK-98: 空白含み名行のNAME列は完全名（x → x EXITED。切り詰め名でなく）
// [covers:list-display.sessions-live-only-with-exited-present]
#[test]
fn list_sessions_works_with_exited_sessions() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("list-sessions-exited");
    let out = cmd
        .env(
            "FAKE_SESSIONS",
            "dead-a [Created 2h ago] (EXITED - attach to resurrect)\n\
             live-a [Created 1m ago] (current)\n\
             dead-b [Created 3d ago] (EXITED - attach to resurrect)\n\
             x EXITED [Created 5m ago]",
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
    // 汚染running行（x EXITED）も一覧に現れる。nameは完全名（TASK-98。
    // TASK-99時点の期待値live-a+xから更新）
    assert_eq!(
        sessions,
        vec!["live-a", "x EXITED"],
        "空白含み名行が一覧から消える/切り詰め名で出る: {sessions:?}"
    );
    // EXITED混在環境でも実行中行のmetadata（created/current/PANES）が取れる
    assert_eq!(v["data"]["sessions"][0]["created"], "1m ago");
    assert_eq!(
        v["data"]["sessions"][0]["panes_per_tab"],
        serde_json::json!([2])
    );
    // 空白含み名行もmetadataが取れる（name=完全名・created=真の生成節。
    // list-tabs呼出は `--session x EXITED action list-tabs`）
    assert_eq!(v["data"]["sessions"][1]["created"], "5m ago");
    assert_eq!(
        v["data"]["sessions"][1]["panes_per_tab"],
        serde_json::json!([2])
    );
}

/// TASK-93 D1/D2: 0-session環境のlist sessionsは空一覧の正常系（exit 0）。
/// humanはheaderのみ + stderrへ作成案内note 1行（R4慣例。stdout契約は不変）・
/// JSONはdata.sessions:[]でstderr空・compactはstdout/stderrとも無出力・
/// --json -c併用はJSON優先（compact無視。compact-names-and-json-precedenceと
/// 同型の0-session版）
// [covers:list-display.sessions-zero-session-normal-output]
#[test]
fn list_sessions_zero_session_environment_output() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // human: headerのみ。stderrはnote 1行（作成案内）
    let (mut cmd, _) = zelper("list-sessions-zero");
    cmd.env("FAKE_SESSIONS_EMPTY", "1")
        .args(["list", "sessions"])
        .assert()
        .code(0)
        .stdout("NAME\tCREATED\tCURRENT\tPANES\n")
        .stderr(predicates::str::contains(
            "zelper: note: no running zellij sessions; create one with 'zelper add session NAME'",
        ));

    // JSON: envelopeのみ（data.sessions=[]）・stderr空
    let (mut cmd, _) = zelper("list-sessions-zero-json");
    let output = cmd
        .env("FAKE_SESSIONS_EMPTY", "1")
        .args(["list", "sessions", "--json"])
        .assert()
        .code(0)
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["sessions"], serde_json::json!([]));
    assert!(output.stderr.is_empty(), "stderr: {:?}", output.stderr);

    // compact: stdout・stderrとも空（machine向け名前列挙契約は不変）
    let (mut cmd, _) = zelper("list-sessions-zero-compact");
    let output = cmd
        .env("FAKE_SESSIONS_EMPTY", "1")
        .args(["list", "sessions", "-c"])
        .assert()
        .code(0)
        .get_output()
        .clone();
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
    assert!(output.stderr.is_empty(), "stderr: {:?}", output.stderr);

    // --json -c併用: JSON優先でcompact無視
    let (mut cmd, _) = zelper("list-sessions-zero-json-c");
    let output = cmd
        .env("FAKE_SESSIONS_EMPTY", "1")
        .args(["list", "sessions", "--json", "-c"])
        .assert()
        .code(0)
        .get_output()
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["sessions"], serde_json::json!([]));
    assert!(output.stderr.is_empty(), "stderr: {:?}", output.stderr);
}

/// list sessions のhuman出力: NAME/CREATED/CURRENT/PANES列（PANESはtab毎pane数の
/// 連結。shimのtabs fixtureはtiled=2固定）
// [covers:list-display.sessions-human-metadata-columns]
#[test]
fn list_sessions_human_shows_metadata_columns() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("list-panes-cwd");
    cmd.args(["list", "panes"]).assert().success().stdout(
        "PANE_ID\tTAB\tTITLE\tCOMMAND\tCWD\n\
             terminal_1\tTab #1\tagent-a\tcodex exec --full-auto\t/work\n\
             terminal_2\tTab #1\tagent-b\tcodex exec --plan\t/work\n\
             plugin_0\tTab #1\tZellij (tab bar)\t-\t-\n",
    );
}

/// 実fileのmtimeがUnix epoch以前の場合: modified()は成功するが
/// duration_since(UNIX_EPOCH)がErr（負経過）になり、sizeは数値・
/// modified_epochはnull（human MODIFIEDは-）へ落ちる系列の条件化
/// （旧excludedのmtime不明系列のうちpre-epoch mtimeで再現可能な側を条件化。
/// レビューminor-2: modified()自体のErr〔機能不支持fs等〕のみexcluded残置）
// [covers:list-display.layouts-pre-epoch-mtime-modified-null]
#[test]
fn list_layouts_pre_epoch_mtime_modified_is_null() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("list-layouts-preepoch");
    let dir = std::env::temp_dir().join(format!(
        "zelper-fake-layouts-preepoch-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // 実file（13 bytes）のmtimeをepoch以前（@-100 = 1969-12-31T23:58:20Z）へ設定
    let old = dir.join("old.kdl");
    std::fs::write(&old, "// pre-epoch\n").unwrap();
    let st = std::process::Command::new("touch")
        .args(["-d", "@-100"])
        .arg(&old)
        .status()
        .expect("touch -d @-100 to pin pre-epoch mtime");
    assert!(st.success(), "touch to pin old.kdl pre-epoch mtime failed");

    // JSON: sizeはSome（数値）だが、modified_epochはu64 epoch秒へ変換不能のためnull
    let out = cmd
        .env("ZELLIJ_LAYOUT_DIR", &dir)
        .args(["list", "layouts", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let entry = &v["data"]["layouts"][0];
    assert_eq!(entry["name"], "old");
    assert_eq!(entry["size"], 13);
    assert_eq!(entry["modified_epoch"], serde_json::Value::Null);

    // human: SIZEは数値・MODIFIEDは-（誤った値を出さないC16契約のmtime側）
    let (mut cmd, _) = zelper("list-layouts-preepoch-human");
    cmd.env("ZELLIJ_LAYOUT_DIR", &dir)
        .args(["list", "layouts"])
        .assert()
        .success()
        .stdout("NAME\tSIZE\tMODIFIED\nold\t13\t-\n");
}

/// 実行中sessionが複数あっても、list layoutsはsession解決を行わず動く
/// （layout一覧はfile列挙でありzellijを呼ばない）
// [covers:list-display.layouts-without-session-resolution]
#[test]
fn list_layouts_works_without_session_resolution() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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

/// symlinkなlayout fileのSIZE/MODIFIEDはlink先実体のstat（follow。TASK-82）。
/// SIZE/MODIFIED双方の値源を独立検証する構成: 実体を1024 bytes・mtimeを固定過去
/// epochへ固定し、lstat残置の誤実装ならSIZE=link先path長8（"real.kdl"）・
/// MODIFIED=link作成時刻（now近傍）でそれぞれ赤になる
// [covers:list-display.layouts-symlink-follow-target-stat]
#[test]
fn list_layouts_symlink_follows_target_stat() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("list-layouts-symlink");
    let dir = std::env::temp_dir().join(format!("zelper-fake-layouts-sym-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // (i) 実体: 内容を固定1024 bytesへ整え、mtimeを固定過去epoch 1_600_000_000
    // （2020-09-13T09:46:40Z）へ設定。手段はtouch -d @epoch（GNU coreutils。
    // symlink作成にstd::os::unixを使う時点でlinux前提と同一のため追加依存不要）
    let real = dir.join("real.kdl");
    std::fs::write(&real, vec![b'#'; 1024]).unwrap();
    let st = std::process::Command::new("touch")
        .args(["-d", "@1600000000"])
        .arg(&real)
        .status()
        .expect("touch -d @1600000000 to pin mtime");
    assert!(st.success(), "touch to pin real.kdl mtime failed");
    // (ii) real.kdlへの相対symlink（link先path "real.kdl" = 8 bytes ≠ 実体 1024 bytes）
    std::os::unix::fs::symlink("real.kdl", dir.join("linked.kdl")).unwrap();
    // (iii) 壊れたsymlink（不在target指し。lstatなら値が出るがfollowならErr→None）
    std::os::unix::fs::symlink("no-such-target", dir.join("broken.kdl")).unwrap();

    // JSON: 名前は3件sort済み。linkedのsize/modified_epochは実体由来の固定値
    // （1 assertに束ねることで、lstat残置時に双方の実測値がfail messageへ出る）
    let out = cmd
        .env("ZELLIJ_LAYOUT_DIR", &dir)
        .args(["list", "layouts", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let layouts = v["data"]["layouts"].as_array().unwrap();
    let names: Vec<&str> = layouts
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["broken", "linked", "real"]);
    let linked = &layouts[1];
    assert_eq!(
        (linked["size"].as_u64(), linked["modified_epoch"].as_u64()),
        (Some(1024), Some(1_600_000_000)),
        "linked.kdlはlink先実体real.kdlのstat（follow）由来であること"
    );
    let real_entry = &layouts[2];
    assert_eq!(
        (
            real_entry["size"].as_u64(),
            real_entry["modified_epoch"].as_u64()
        ),
        (Some(1024), Some(1_600_000_000))
    );
    // broken: 名前は一覧へ残し、両値はnull（参照先stat失敗のNone経路）
    assert_eq!(layouts[0]["size"], serde_json::Value::Null);
    assert_eq!(layouts[0]["modified_epoch"], serde_json::Value::Null);

    // human: real/linked行のSIZEはともに1024。MODIFIEDは同一epoch由来のため
    // 同一の相対表記文字列（固定過去epochなら実行時刻依存の値は一致比較で検証）。
    // broken行は名前を残して両値-
    let (mut cmd, _) = zelper("list-layouts-symlink-human");
    let out = cmd
        .env("ZELLIJ_LAYOUT_DIR", &dir)
        .args(["list", "layouts"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "header + 3行: {text}");
    assert_eq!(lines[0], "NAME\tSIZE\tMODIFIED");
    assert_eq!(lines[1], "broken\t-\t-");
    let linked_fields: Vec<&str> = lines[2].split('\t').collect();
    let real_fields: Vec<&str> = lines[3].split('\t').collect();
    assert_eq!(linked_fields[0], "linked");
    assert_eq!(real_fields[0], "real");
    assert_eq!(linked_fields[1], "1024");
    assert_eq!(real_fields[1], "1024");
    // human_ageは行毎に現在時刻を取得するため、同一epochでも行間で時間境界
    // （例: 5h59m→6h）を跨ぐと表示文字列が変わりうる（レビューminor-1）。
    // Nd Nh ago形を時間換算へparseし、差が境界1時間分以内であることを検証する
    // （値源がlstat由来=link作成時刻なら6年超の差になるため検出力は保たれる。
    // 値の厳密な同一性はJSON側modified_epoch==1600000000の一致比較が担保）
    let hours_of = |s: &str| -> u64 {
        let toks: Vec<&str> = s.split_whitespace().collect();
        assert_eq!(toks.len(), 3, "MODIFIEDは Nd Nh ago 形であること: {s}");
        assert_eq!(toks[2], "ago", "MODIFIEDはago接尾であること: {s}");
        let d: u64 = toks[0].strip_suffix('d').unwrap().parse().unwrap();
        let h: u64 = toks[1].strip_suffix('h').unwrap().parse().unwrap();
        d * 24 + h
    };
    let linked_hours = hours_of(linked_fields[2]);
    let real_hours = hours_of(real_fields[2]);
    assert!(
        linked_hours.abs_diff(real_hours) <= 1,
        "MODIFIEDは同一実体epoch由来（行間の時間境界跨ぎは+/-1hまで許容）: \
         linked={linked_hours}h real={real_hours}h"
    );
}

// [covers:read-send.read-single-and-multi-results]
#[test]
fn read_single_and_multi_with_tail() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("read-miss");
    cmd.args(["read", "999", "--json"])
        .assert()
        .failure()
        .code(3);
}

// [covers:json-contract.cli-error-envelope-on-failure]
#[test]
fn json_error_envelope_on_failure() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("send-nodash");
    cmd.args(["send", "1", "y"]).assert().failure().code(2);
}

// [covers:cli-grammar.send-text-keys-conflict-is-usage-error]
#[test]
fn send_text_and_keys_conflict_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("send-conflict");
    cmd.args(["send", "1", "--keys", "Enter", "--", "y"])
        .assert()
        .failure()
        .code(2);
}

// [covers:cli-grammar.remap-layout-sources-conflict-is-usage-error]
#[test]
fn remap_layout_sources_conflict_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("remap-conflict");
    cmd.args(["remap", "three", "--path", "./x.kdl"])
        .assert()
        .failure()
        .code(2);
}

// [covers:cli-grammar.invalid-pane-spec-is-usage-error]
#[test]
fn invalid_pane_spec_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("bad-spec");
    cmd.args(["read", "pane:12"]).assert().failure().code(2);
}

// [covers:cli-grammar.completion-generates-script]
#[test]
fn completion_generates_script() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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

/// --enter併用時のCR書き込み（write -p 13）失敗: text送信（write-chars）は成功し、
/// その後のCR書き込みのみが失敗したpaneは失敗扱い（単対象ならexit 5・
/// OperationFailed）。失敗はFAKE_FAIL_WRITE（write -p失敗注入）で再現。
/// log記録後に失敗するため、成功したwrite-chars呼び出しと失敗したwrite 13呼び出し
/// の両方がlogに観測される。CRはtext送信の後（DD-7）のため、log上の両argv位置で
/// 順序（write-chars < write 13）まで検証する
// [covers:read-send.send-enter-cr-failure-exit5]
#[test]
fn send_enter_cr_failure_is_exit5() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("send-enter-cr-fail");
    let out = cmd
        .env("FAKE_FAIL_WRITE", "terminal_1")
        .args(["send", "1", "--enter", "--json", "--", "hello"])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    // text送信（write-chars）の後にCR書き込み（write 13）が続く（順序逆転は不可）
    let raw = std::fs::read_to_string(&log).unwrap();
    let text_pos = raw
        .find("write-chars -p terminal_1 hello")
        .unwrap_or_else(|| panic!("write-chars call not found. log: {raw:?}"));
    let cr_pos = raw
        .find("write -p terminal_1 13")
        .unwrap_or_else(|| panic!("write 13 call not found. log: {raw:?}"));
    assert!(text_pos < cr_pos, "CR must follow text send. log: {raw:?}");
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["class"], "OperationFailed");
    let entry = &v["error"]["data"]["results"][0];
    assert_eq!(entry["target"], "terminal_1");
    assert_eq!(entry["ok"], false);
    assert!(
        entry["error"].as_str().unwrap().contains("boom"),
        "error: {:?}",
        entry["error"]
    );
}

/// --keys経路のsend-keys失敗: 失敗paneは失敗扱いとなり、単対象ならexit 5
/// （OperationFailed）。失敗はFAKE_FAIL_SEND_KEYS（send-keys失敗注入）で再現
// [covers:read-send.send-keys-failure-exit5]
#[test]
fn send_keys_failure_is_exit5() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("send-keys-fail");
    let out = cmd
        .env("FAKE_FAIL_SEND_KEYS", "terminal_1")
        .args(["send", "1", "--json", "--keys", "Ctrl", "a"])
        .assert()
        .failure()
        .code(5)
        .get_output()
        .stdout
        .clone();
    let cs = calls(&log);
    assert!(
        cs.iter()
            .any(|c| c.contains("send-keys -p terminal_1 Ctrl a")),
        "calls: {cs:?}"
    );
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["class"], "OperationFailed");
    let entry = &v["error"]["data"]["results"][0];
    assert_eq!(entry["target"], "terminal_1");
    assert_eq!(entry["ok"], false);
    assert!(
        entry["error"].as_str().unwrap().contains("boom"),
        "error: {:?}",
        entry["error"]
    );
}

/// send部分失敗時のper-pane報告の中身: humanは成功paneの送信行（sent ->）と
/// 失敗paneのFAILED行を混在させ、--jsonのerror.data.results[]は成功entry
/// （ok=true）と失敗entry（ok=false・error）を混在させる（部分失敗を隠さない）
// [covers:read-send.send-failure-per-pane-report-contents]
#[test]
fn send_failure_reports_per_pane_contents() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // human: 成功行とFAILED行の混在
    let (mut cmd, _) = zelper("send-report-human");
    let out = cmd
        .env("FAKE_FAIL_WRITE_CHARS", "terminal_1")
        .args(["send", "1", "2", "--", "hello"])
        .assert()
        .failure()
        .code(6)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    // 失敗pane行: terminal_1を含む行にFAILEDと失敗理由が揃う（別pane行への
    // 附着では通らない）
    let failed_line = text
        .lines()
        .find(|l| l.contains("terminal_1"))
        .unwrap_or_else(|| panic!("failed pane line not found. stdout: {text:?}"));
    assert!(
        failed_line.contains("FAILED"),
        "line: {failed_line:?} (stdout: {text:?})"
    );
    assert!(
        failed_line.contains("boom"),
        "line: {failed_line:?} (stdout: {text:?})"
    );
    // 成功pane行: terminal_2を含む行は送信成功（sent ->）を報告する
    let sent_line = text
        .lines()
        .find(|l| l.contains("terminal_2"))
        .unwrap_or_else(|| panic!("success pane line not found. stdout: {text:?}"));
    assert!(
        sent_line.contains("sent ->"),
        "line: {sent_line:?} (stdout: {text:?})"
    );

    // JSON: error.data.results[]の成功/失敗entry混在
    let (mut cmd, _) = zelper("send-report-json");
    let out = cmd
        .env("FAKE_FAIL_WRITE_CHARS", "terminal_1")
        .args(["send", "1", "2", "--json", "--", "hello"])
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
    let err_entry = &results[0];
    assert_eq!(err_entry["target"], "terminal_1");
    assert_eq!(err_entry["ok"], false);
    assert!(
        err_entry["error"].as_str().unwrap().contains("boom"),
        "error: {:?}",
        err_entry["error"]
    );
    let ok_entry = &results[1];
    assert_eq!(ok_entry["target"], "terminal_2");
    assert_eq!(ok_entry["ok"], true);
    assert!(ok_entry["error"].is_null());
}

/// read失敗時のhuman表示: 失敗pane（terminal_1）の行はpane識別子とtitleを含む
/// ヘッダ形式（実装はtab情報を省いた形式。失敗行形式は出典規定なし）で現れ、
/// FAILED表示と失敗理由（boom）が続き、成功pane（terminal_2）のヘッダと内容に
/// 混在する。検証は失敗pane行内での対応（識別子・title・FAILED・理由が同一行に
/// 揃うこと。別pane行への附着は不通過）とpane判別可能性までとする（ヘッダの
/// 括弧内形式・形式全文の固定はしない）
// [covers:read-send.read-failure-human-shows-failed-pane]
#[test]
fn read_failure_human_shows_failed_pane() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("read-fail-human");
    let out = cmd
        .env("FAKE_FAIL_DUMP_SCREEN", "terminal_1")
        .args(["read", "1", "2"])
        .assert()
        .failure()
        .code(6)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    // 失敗pane行: terminal_1識別子を含む行内にtitle・FAILED・失敗理由が揃う
    // （別pane行への附着では通らない）
    let failed_line = text
        .lines()
        .find(|l| l.contains("=== terminal_1"))
        .unwrap_or_else(|| panic!("failed pane header line not found. stdout: {text:?}"));
    assert!(
        failed_line.contains("agent-a"),
        "line: {failed_line:?} (stdout: {text:?})"
    );
    assert!(
        failed_line.contains("FAILED"),
        "line: {failed_line:?} (stdout: {text:?})"
    );
    assert!(
        failed_line.contains("boom"),
        "line: {failed_line:?} (stdout: {text:?})"
    );
    // 成功paneのヘッダと内容に混在
    assert!(text.contains("=== terminal_2"), "stdout: {text:?}");
    assert!(text.contains("SCREEN[terminal_2]"), "stdout: {text:?}");
}

/// read全失敗（exit 5）時もerror.data.results[]に全pane分の失敗entry
/// （target・ok=false・error）が同梱される（per-pane報告はexit 6/5両方に適用）
// [covers:read-send.read-all-failure-includes-results]
#[test]
fn read_all_failure_includes_results() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("read-all-fail-results");
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
    let results = v["error"]["data"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    for (entry, target) in results.iter().zip(["terminal_1", "terminal_2"]) {
        assert_eq!(entry["target"], target);
        assert_eq!(entry["ok"], false);
        assert!(
            entry["error"].as_str().unwrap().contains("boom"),
            "error: {:?}",
            entry["error"]
        );
        assert!(entry["detail"].is_null());
    }
}

/// PANES概要: 5tab超は先頭4つ+`...`で省略。floatingは連結に含めない
/// （6tab: tiled 7,7,6,2,1,3・2tab目にfloating 2）
// [covers:list-display.sessions-summary-elides-beyond-four-tabs]
#[test]
fn list_sessions_panes_summary_elides_beyond_four_tabs() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
/// `zellij --session NAME action detach` を実行する（既存run_actionの枠のwrap）。
/// human/JSON双方の実出力（1行サマリ/envelope 1通・data.session）も検証する
/// （REVIEW-2指摘6と同一の実出力観測。L2のFakeBackendはlibtestのstdout capture
/// の外で出力を観測できないため実binary側で担保）
// [covers:attach-detach.detach-argv]
#[test]
fn detach_records_session_action_detach_argv() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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

    // JSON: 実出力はenvelope 1通のみでdata.session=NAME
    let (mut cmd, log) = zelper("detach-argv-json");
    let out = cmd
        .args(["--session", "fake-sess", "detach", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["session"], "fake-sess");
    assert!(
        calls(&log)
            .iter()
            .any(|c| c == "--session fake-sess action detach"),
        "calls: {:?}",
        calls(&log)
    );
}

/// detachの失敗mapping: (a) FAKE_FAIL_DETACHによる汎用失敗はOperationFailed
/// （exit 5）へ変換されmessageへstderr本文が埋め込まれる。(b) 不在session名は
/// TASK-93 D3の事前存在確認でNoTarget（exit 3・zelper統一文面 + candidates）に
/// なり、zellij側のrc=1 Session not found応答には届かない（action detach不発）
// [covers:attach-detach.detach-fail]
#[test]
fn detach_fail_maps_generic_and_missing_name() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (a) FAKE_FAIL_DETACH: action detachが非zero終了
    let (mut cmd, _) = zelper("detach-fail-generic");
    let out = cmd
        .env("FAKE_FAIL_DETACH", "1")
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

    // (b) 不在session名: 事前存在確認でNoTarget（exit 3）+ candidates（running名）
    let (mut cmd, log) = zelper("detach-fail-notfound");
    let out = cmd
        .args(["--session", "ghost-sess", "detach", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("session 'ghost-sess' not found"),
        "message: {msg}"
    );
    let cands = v["error"]["candidates"].as_array().unwrap();
    assert!(
        cands.iter().any(|c| c == "fake-sess"),
        "candidates: {cands:?}"
    );
    // 事前存在確認で失敗するためaction detach argvは不発
    assert!(
        !calls(&log).iter().any(|c| c.contains("action detach")),
        "calls: {:?}",
        calls(&log)
    );
}

/// add session成功経路: 事前確認（同名なし）→ positional形式 `attach -b NAME`
/// （--session接頭なしのtop-level argv）→ postcondition（作成後のlist-sessionsに
/// NAMEがrunning出現）→ 報告。shimのFAKE_SESSIONS_CREATE=okは作成NAMEを
/// list-sessions応答へ反映する（statelessだとpostconditionが常に不成立になるため）
// [covers:attach-detach.add-session-argv-postcondition]
#[test]
fn add_session_creates_detached_and_reports() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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

/// add sessionのpostcondition検索の0件分類（REVIEW-2指摘5）: 作成呼出
/// （attach -b）がrc=0でも作成後のlist-sessionsが0件rc=1
/// （No active zellij sessions found.）の応答でも「0件」として扱い、NAME不在は
/// VerificationFailed（exit 7）へ分類する（OperationFailed 5の伝播でなく。
/// shimはFAKE_SESSIONS_EMPTY=1 + FAKE_SESSIONS_CREATE=ok-muteで
/// list-sessions常にrc=1・作成rc=0で応答反映なしの系列を再現）
// [covers:attach-detach.add-session-postcondition-empty-list-verification-failed]
#[test]
fn add_session_postcondition_empty_list_is_verification_failed() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("add-session-emptylist");
    let out = cmd
        .env("FAKE_SESSIONS_EMPTY", "1")
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
    // 作成呼出自体はrc=0で実行済み（一覧0件は作成後の検証段階の分類）
    assert!(
        calls(&log).iter().any(|c| c == "attach -b work"),
        "calls: {:?}",
        calls(&log)
    );
}

/// add session NAME validation（TASK-96 §2.2/§2.3）: 空文字・'/'含み・空白含み・
/// 先頭`-`（`--`経由）の4種NAMEはUsage（exit 2）で一覧取得より前に拒否する
/// （空白含みはzellij受理後もparse_sessionsがlist-sessions行の先頭wordのみを
/// 名前化するため世界modelで復元不能〔postcondition必敗・session残留〕。空文字・
/// '/'含みはzellijが明示拒否〔M96-B/C実測〕するがrename NEW_NAMEとの対称性・
/// fail-fastでwrap側でも事前検査。先頭`-`は`--`通過系列が`--`挿入なしの
/// attach -b NAME組み立てへ到達するため入力段の確定拒否が唯一のzeller側完結手段）。
/// 先頭`-`は`--`（option終端）経由でCLI解析を通過させる——`--`なしはzeller側clapが
/// exit 2で拒否するが文面がclap形式（envelope不整合）のため本契約（app層Usage文面）
/// の対象外
// [covers:attach-detach.add-session-invalid-name-usage-error]
#[test]
fn add_session_invalid_name_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (NAMEに相当するargs末尾, 期待文面)。空文字はpositional引数として到達可能な形式
    let cases: [(&[&str], &str); 4] = [
        (&[""], "NAME must not be empty"),
        (&["a/b"], "NAME must not contain '/'"),
        (&["a b"], "NAME must not contain whitespace"),
        (&["--", "-x"], "NAME must not start with '-'"),
    ];
    for (i, (name_args, expect_msg)) in cases.iter().enumerate() {
        let (mut cmd, log) = zelper(&format!("add-session-invalid-{i}"));
        let mut args: Vec<&str> = vec!["add", "session", "--json"];
        args.extend_from_slice(name_args);
        let out = cmd
            .env("FAKE_SESSIONS", "old [Created 1m ago] (current)")
            .args(&args)
            .assert()
            .failure()
            .code(2)
            .get_output()
            .stdout
            .clone();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            v["error"]["class"], "Usage",
            "case {i} (args={name_args:?})"
        );
        // 文面は完全一致で検証（§2.3）
        let msg = v["error"]["message"].as_str().unwrap();
        assert_eq!(msg, *expect_msg, "case {i} (args={name_args:?})");
        // 一覧取得より前のfail-fast（list-sessions・attach -bとも不発。
        // capability検査の--version呼び出しはdispatch契約上残るため
        // ログ全体の空は要求しない——argv単位の不在検証とする）
        let cs = calls(&log);
        assert!(
            !cs.iter().any(|c| c == "list-sessions -n"),
            "case {i} calls: {cs:?}"
        );
        assert!(
            !cs.iter().any(|c| c.starts_with("attach -b")),
            "case {i} calls: {cs:?}"
        );
    }
}

/// TASK-98 §2.3-1: add session a は空白含み既存名 a b の切り詰めprefix（先頭word
/// a）と誤衝突しない（Preflight誤拒否の解消。世界modelのnameが完全名化された後の
/// `s.name == name` 比較では a ≠ a b）。作成・postcondition（list_sessionsに
/// a行がrunning出現）を通過して成功する
// [covers:attach-detach.add-session-no-false-collision-with-truncated-prefix]
#[test]
fn add_session_no_false_collision_with_truncated_prefix() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (dir, log) = setup_fake_zellij("add-session-ws-prefix");
    let out = command_on(&dir, &log)
        .env("FAKE_SESSIONS", "a b [Created 1m ago]")
        .env("FAKE_SESSIONS_CREATE", "ok")
        .args(["add", "session", "a", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["session"], "a");
    assert_eq!(v["data"]["created"], true);
    let cs = calls(&log);
    assert!(cs.iter().any(|c| c == "attach -b a"), "calls: {cs:?}");
    // 事後一覧（同一shim状態）: 既存 a b 行と作成 a 行の共存
    let out = command_on(&dir, &log)
        .env("FAKE_SESSIONS", "a b [Created 1m ago]")
        .args(["list", "sessions", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let names: Vec<&str> = v["data"]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["a b", "a"],
        "作成後一覧（postcondition通過の実質）: {names:?}"
    );
}

/// TASK-93 D2/D3: 非TTY attachでも解決（--session不在名の事前存在確認・0件自動
/// 解決）がTTY検査より先に立つ: 不在名・完全0件（FAKE_SESSIONS_EMPTY）はNoTarget
/// （exit 3）で失敗しzellij attach呼出は不発。有効名側は従来どおりTTY検査による
/// Preflight（exit 7）——attach-non-tty-preflight条件が代表
// [covers:attach-detach.attach-non-tty-resolution-precedes-tty-check]
#[test]
fn attach_non_tty_resolution_precedes_tty_check() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (i) 不在名: NoTarget（exit 3）がTTY検査（exit 7）より先
    let (mut cmd, log) = zelper("attach-nontty-missing");
    let out = cmd
        .args(["--session", "ghost-sess", "attach", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("session 'ghost-sess' not found"),
        "message: {msg}"
    );
    // 解決で失敗するためzellij attach呼出は不発
    assert!(
        !calls(&log).iter().any(|c| c.starts_with("attach")),
        "calls: {:?}",
        calls(&log)
    );

    // (ii) 完全0-session環境（flag/envなし自動解決）: NoTarget（exit 3・作成案内）
    let (mut cmd, _) = zelper("attach-nontty-zero");
    let out = cmd
        .env("FAKE_SESSIONS_EMPTY", "1")
        .args(["attach", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(
        msg.contains("no running zellij session found"),
        "message: {msg}"
    );
    assert!(msg.contains("zelper add session"), "message: {msg}");
}

// ---------------------------------------------------------------------------
// move（TASK-88・move.tomlのCLI表面はcli-grammar管轄。設計§5.3: FAKE_LOG全argv
// 記録でmoveのargv列を担保し、FAKE_FAIL_MOVE_PANE / FAKE_FAIL_MOVE_TABで
// 非zero終了のexit分類を検証する）

/// 4 tab構成のtabs応答（FAKE_TABS_JSON。1行=1tab object。move-tabの状態管理は
/// shimがlist-tabs応答へ反映する）
const FOUR_TABS_JSON: &str = "[{\"position\":0,\"name\":\"w\",\"active\":true,\"are_floating_panes_visible\":false,\"selectable_tiled_panes_count\":2,\"selectable_floating_panes_count\":0,\"tab_id\":0},{\"position\":1,\"name\":\"x\",\"active\":false,\"are_floating_panes_visible\":false,\"selectable_tiled_panes_count\":2,\"selectable_floating_panes_count\":0,\"tab_id\":1},{\"position\":2,\"name\":\"y\",\"active\":false,\"are_floating_panes_visible\":false,\"selectable_tiled_panes_count\":2,\"selectable_floating_panes_count\":0,\"tab_id\":2},{\"position\":3,\"name\":\"z\",\"active\":false,\"are_floating_panes_visible\":false,\"selectable_tiled_panes_count\":2,\"selectable_floating_panes_count\":0,\"tab_id\":3}]";

/// move paneの三者排他（action positional / --to-tab / --swap-with はちょうど1つ）
/// と--backwords×rotate制約のCLI parse表面。いずれもusage error exit 2で、
/// backend呼出（session解決後のmove-pane argv）は不発
// [covers:cli-grammar.move-three-way-exclusive-usage-error]
#[test]
fn move_pane_three_way_exclusive_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (1) 3指定とも不在
    let (mut cmd, _log) = zelper("move-none");
    cmd.args(["move", "pane", "1"]).assert().failure().code(2);
    // (2) action positional + --to-tab同時
    let (mut cmd, log) = zelper("move-left-totab");
    cmd.args(["move", "pane", "1", "left", "--to-tab", "0"])
        .assert()
        .failure()
        .code(2);
    // (3) --swap-with + --to-tab同時
    let (mut cmd, _) = zelper("move-swap-totab");
    cmd.args(["move", "pane", "1", "--to-tab", "0", "--swap-with", "2"])
        .assert()
        .failure()
        .code(2);
    // (4) --backwards単独（action不在。clap requires）
    let (mut cmd, _) = zelper("move-backwards-alone");
    cmd.args(["move", "pane", "1", "--backwards"])
        .assert()
        .failure()
        .code(2);
    // (5) --backwards + action=left（rotate以外との併用。app層runtime検証）
    let (mut cmd, _) = zelper("move-backwards-left");
    cmd.args(["move", "pane", "1", "left", "--backwards"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicates::str::contains("--backwards"));
    // parse段・検証段ともにmove-pane argvは不発
    assert!(
        !calls(&log).iter().any(|c| c.contains("move-pane")),
        "calls: {:?}",
        calls(&log)
    );
}

/// move pane nativeのCLI表面: `move pane 1 left`は`move-pane left -p terminal_1`
/// を発行しexit 0。fake shimはpane状態を変えないため幾何比較はno-op判定になり、
/// warning行（stderr）+ JSON envelope（data.noop=true）で報告する（M1実測の
/// edge silent no-op契約。exit codeのみでは成否判別不能のための検出経路）
// [covers:cli-grammar.move-native-argv-and-noop-warning]
#[test]
fn move_pane_native_argv_json_and_noop_warning() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("move-left");
    let output = cmd
        .args(["move", "pane", "1", "left", "--json"])
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(
        calls(&log)
            .iter()
            .any(|c| c.ends_with("action move-pane left -p terminal_1")),
        "argv: {:?}",
        calls(&log)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning"),
        "no-op warning（幾何不変の検出）: {stderr}"
    );
    let v: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["target"], "terminal_1");
    assert_eq!(v["data"]["action"], "left");
    assert_eq!(v["data"]["noop"], true);

    // rotate backwardsのargv（move-pane-backwards・-p尊重）
    let (mut cmd, log) = zelper("move-rotate-back");
    cmd.args(["move", "pane", "1", "rotate", "--backwards"])
        .assert()
        .success();
    assert!(
        calls(&log)
            .iter()
            .any(|c| c.ends_with("action move-pane-backwards -p terminal_1")),
        "argv: {:?}",
        calls(&log)
    );
}

/// move系backend失敗のexit分類: move-pane・move-tabの非zero終了はいずれも
/// OperationFailed（exit 5・--json時はerror envelope）
// [covers:cli-grammar.move-failure-exit5-operation-failed]
#[test]
fn move_backend_failure_is_exit5() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("move-fail-pane");
    cmd.env("FAKE_FAIL_MOVE_PANE", "1")
        .args(["move", "pane", "1", "left", "--json"])
        .assert()
        .failure()
        .code(5)
        .stdout(predicates::str::contains("\"OperationFailed\""));

    let (mut cmd, _) = zelper("move-fail-tab");
    cmd.env("FAKE_FAIL_MOVE_TAB", "1")
        .env("FAKE_TABS_JSON", FOUR_TABS_JSON)
        .args(["move", "tab", "0", "--swap-with", "2", "--json"])
        .assert()
        .failure()
        .code(5)
        .stdout(predicates::str::contains("\"OperationFailed\""));
}

/// move tab交換のCLI表面: d=2（position 0と2）の交換は`move-tab right -t 0`×2 +
/// `move-tab left -t 2`×1の計3回（隣接transposition合成・go-to-tab不使用）で
/// position交換がpostcondition検証を通過しexit 0する
// [covers:cli-grammar.move-tab-swap-argv-sequence]
#[test]
fn move_tab_swap_argv_sequence_and_success() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // REVIEW-3指摘4: 同一dir（tag+PID再利用を模した同一setupの再実行）でも
    // setupが ${FAKE_LOG}.tabs を初期化するため、初期状態（FAKE_TABS_JSON）から
    // 再び同じ交換が成立する（前回の交換後状態が読み込まれない）
    let (dir, log) = setup_fake_zellij("move-tabswap");
    let run_once = || -> serde_json::Value {
        let mut cmd = Command::cargo_bin("zelper").unwrap();
        let path = format!(
            "{}:{}",
            dir.to_str().unwrap(),
            std::env::var("PATH").unwrap_or_default()
        );
        let stdout = cmd
            .env("PATH", path)
            .env_remove("ZELLIJ_SESSION_NAME")
            .env("FAKE_LOG", &log)
            .env("FAKE_FIXTURES", fixture_dir())
            .env("FAKE_TABS_JSON", FOUR_TABS_JSON)
            .args(["move", "tab", "0", "--swap-with", "2", "--json"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice(&stdout).unwrap()
    };
    let out = run_once();
    let argv: Vec<String> = calls(&log)
        .iter()
        .filter(|c| c.contains("action move-tab "))
        .map(|c| c.rsplit("action ").next().unwrap().to_string())
        .collect();
    assert_eq!(
        argv,
        vec![
            "move-tab right -t 0",
            "move-tab right -t 0",
            "move-tab left -t 2"
        ],
        "2d-1回の隣接transposition合成: {argv:?}"
    );
    assert!(
        !calls(&log).iter().any(|c| c.contains("go-to-tab")),
        "-tはfocus不変のためgo-to不使用: {:?}",
        calls(&log)
    );
    let v: &serde_json::Value = &out;
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["action"], "swap-tab");
    assert_eq!(
        v["data"]["tabs"][0],
        serde_json::json!({"id": 0, "position": 2})
    );
    assert_eq!(
        v["data"]["tabs"][1],
        serde_json::json!({"id": 2, "position": 0})
    );

    // 同一dirでの再実行: setup_fake_zellijを再度呼ぶ（tag+PIDが同一のため同一dirを
    // 再利用 = PID再利用の再現）。setupが ${FAKE_LOG}.tabs を初期化するため、
    // 前回の交換後状態（tab 0がposition 2にある状態）でなく初期状態から再び
    // 同一argv列・同一結果になる
    let (_dir2, _log2) = setup_fake_zellij("move-tabswap");
    let out2 = run_once();
    let argv2: Vec<String> = calls(&log)
        .iter()
        .filter(|c| c.contains("action move-tab "))
        .map(|c| c.rsplit("action ").next().unwrap().to_string())
        .skip(argv.len())
        .collect();
    assert_eq!(
        argv2,
        vec![
            "move-tab right -t 0",
            "move-tab right -t 0",
            "move-tab left -t 2"
        ],
        "再実行も同一argv列（状態fileのsetup時初期化）: {argv2:?}"
    );
    assert_eq!(
        out2["data"]["tabs"][0],
        serde_json::json!({"id": 0, "position": 2}),
        "再実行も初期状態から交換（前回結果のposition 2→0が残留しない）"
    );
}

/// verb set v2=12のうちTASK-86/87/88追加verb（watch/attach/detach/move）が
/// helpのCommands一覧に現れる（新verb構文のhelp出現・廃止option不在規約と同型）
// [covers:cli-grammar.verb-set-v2-new-verbs-in-help]
#[test]
fn help_lists_verb_set_v2_new_verbs() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("help-verbs");
    let out = cmd
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8_lossy(&out);
    for verb in ["move", "attach", "detach", "watch"] {
        assert!(s.contains(verb), "helpに{verb}が現れる: {}", first_line(&s));
    }
}

/// helper: 先頭行のみ（assert文面のコンパクト化用）
fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("")
}

// ---------------------------------------------------------------------------
// TASK-65: cli-grammar残excluded 8件の条件化（14条件。設計書§1/§3。
// FAKE_PANES_JSON系shim拡張〔pane状態管理〕とFAKE_NEW_PANE_TEMPLATEを使用）

/// FAKE_PANES_JSON用のpane object 1件分（tests/fixtures/zellij/panes.jsonと
/// 同一のfield形式。実zellij出力のfield名であり正規形domain schemaではない）
fn pane_json(id: u32, title: &str, x: u32, cols: u32, tab_id: u32) -> String {
    let tab_name = if tab_id == 0 { "Tab #1" } else { "Tab #2" };
    format!(
        r#"{{"id": {id}, "is_plugin": false, "is_focused": false, "is_fullscreen": false, "is_floating": false, "is_suppressed": false, "title": "{title}", "exited": false, "exit_status": null, "is_held": false, "pane_x": {x}, "pane_content_x": {x}, "pane_y": 1, "pane_content_y": 1, "pane_rows": 58, "pane_content_rows": 56, "pane_columns": {cols}, "pane_content_columns": {cols}, "cursor_coordinates_in_pane": null, "terminal_command": null, "plugin_url": null, "is_selectable": true, "index_in_pane_group": null, "tab_id": {tab_id}, "tab_position": {tab_id}, "tab_name": "{tab_name}", "pane_command": null, "pane_cwd": null}}"#
    )
}

/// FAKE_TABS_JSON用のtab object 1件分（既存使用例FOUR_TABS_JSONと同一field形式）
fn tab_json(pos: u32, name: &str, tiled: u32, tab_id: u32) -> String {
    format!(
        r#"{{"position": {pos}, "name": "{name}", "active": {}, "are_floating_panes_visible": false, "selectable_tiled_panes_count": {tiled}, "selectable_floating_panes_count": 0, "tab_id": {tab_id}}}"#,
        pos == 0
    )
}

/// shimのnew-pane handlerが状態fileへappendする1 pane object JSON
/// （@ID@ = 採番されたid数値placeholder。postconditionの存在確認に必要な
/// fieldのみでよいがpane_jsonと同一形式で与える）
const FAKE_NEW_PANE_TEMPLATE: &str = r#"{"id": @ID@, "is_plugin": false, "is_focused": false, "is_fullscreen": false, "is_floating": false, "is_suppressed": false, "title": "new-pane", "exited": false, "exit_status": null, "is_held": false, "pane_x": 150, "pane_content_x": 151, "pane_y": 1, "pane_content_y": 1, "pane_rows": 58, "pane_content_rows": 56, "pane_columns": 50, "pane_content_columns": 50, "cursor_coordinates_in_pane": null, "terminal_command": null, "plugin_url": null, "is_selectable": true, "index_in_pane_group": null, "tab_id": 0, "tab_position": 0, "tab_name": "Tab #1", "pane_command": null, "pane_cwd": null}"#;

/// shimのnew-tab handlerが状態fileへappendする1 tab object JSON
/// （@ID@ = 採番されたtab id数値・@POS@ = append時の行番号=position placeholder。
/// TASK-67: add tab layout source map条件のpostcondition通過に使用。
/// tab_jsonと同一field形式）
const FAKE_NEW_TAB_TEMPLATE: &str = r#"{"position": @POS@, "name": "new-tab", "active": false, "are_floating_panes_visible": false, "selectable_tiled_panes_count": 0, "selectable_floating_panes_count": 0, "tab_id": @ID@}"#;

/// zellij不在実path: PATHにzellij（shim含む）が無ければ操作系verbは
/// ZellijUnavailable（exit 4）。assert_cmdはcargo_binの絶対pathでzelper自身を
/// 起動するため、PATHを空dirへ差し替えてもzelperの起動には影響しない
// [covers:cli-grammar.zellij-unavailable-real-path-exit4]
#[test]
fn zellij_unavailable_real_path_exit4() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let empty = std::env::temp_dir().join(format!("zelper-empty-path-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let mut cmd = Command::cargo_bin("zelper").unwrap();
    cmd.env("PATH", &empty)
        .env_remove("ZELLIJ_SESSION_NAME")
        .args(["read", "1"])
        .assert()
        .failure()
        .code(4)
        .stderr(predicates::str::contains(
            "zelper: zellij executable not found in PATH",
        ));
}

/// PANESPEC plugin_N受理の正例: 既定fixtureのplugin_0（tab bar）を対象に
/// readが成立し、dump-screen -p plugin_0として発行される
// [covers:cli-grammar.plugin-pane-spec-read-positive]
#[test]
fn read_plugin_pane_spec_positive() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("read-plugin0");
    let out = cmd
        .args(["read", "plugin_0", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["data"]["results"][0]["target"], "plugin_0");
    assert!(
        calls(&log)
            .iter()
            .any(|c| c == "--session fake-sess action dump-screen -p plugin_0"),
        "calls: {:?}",
        calls(&log)
    );
}

/// DD-1.6非例の重複option: 同一flagの重複はclapの重複拒否でusage error
// [covers:cli-grammar.duplicate-flag-is-usage-error]
#[test]
fn duplicate_flag_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("dup-json");
    cmd.args(["read", "--json", "--json"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicates::str::contains("cannot be used multiple times"));
}

/// DD-1.6非例「remapへのpathのpositional渡し」の構文段拒否: layout名に'/'を
/// 含む値はusage error（exit 2）で--pathへの案内を出す（純parse test・backend呼出不発）
// [covers:cli-grammar.remap-positional-path-is-usage-error]
#[test]
fn remap_positional_path_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("remap-pospath");
    cmd.args(["remap", "./three.kdl"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicates::str::contains("--path"));
    // 純parse test: backend呼用は不発（shim非依存）
    assert!(
        !calls(&log)
            .iter()
            .any(|c| c.contains("list-panes") || c.contains("dump-layout")),
        "calls: {:?}",
        calls(&log)
    );
}

/// equalize×grow|shrink混在の拒否: equalize構文へのgrow語混入はPANESPEC検査で・
/// pane構文への余剰positional（equalize語）はclap unexpected argumentでexit 2
// [covers:cli-grammar.resize-equalize-grow-mix-is-usage-error]
#[test]
fn resize_equalize_grow_mix_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("resize-eq-grow");
    cmd.args(["resize", "equalize", "grow", "left"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicates::str::contains("invalid pane spec 'grow'"));

    let (mut cmd, _) = zelper("resize-pane-extra");
    cmd.args(["resize", "pane", "1", "grow", "left", "3", "equalize"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicates::str::contains("unexpected argument"));
}

/// session解決の--session > env段: 両方指定時は--session優先・--sessionなしは
/// ZELLIJ_SESSION_NAMEで解決する（解決名はbackend argvの--session接頭で観測）
// [covers:cli-grammar.session-flag-priority-and-env-resolution]
#[test]
fn session_flag_priority_and_env_resolution() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sessions = "other-sess [Created 1m ago] (current)\nenv-sess [Created 2m ago]";
    // (i) 両方指定: --sessionがenvに優先
    let (mut cmd, log) = zelper("sess-flag-prio");
    cmd.env("FAKE_SESSIONS", sessions)
        .env("ZELLIJ_SESSION_NAME", "env-sess")
        .args(["--session", "other-sess", "read", "1"])
        .assert()
        .success();
    assert!(
        calls(&log)
            .iter()
            .any(|c| c.starts_with("--session other-sess action ")),
        "calls: {:?}",
        calls(&log)
    );

    // (ii) --sessionなし: env解決（shimのsession guardのためenv-sessをFAKE_SESSIONSへ含む）
    let (mut cmd, log) = zelper("sess-env-resolve");
    cmd.env("FAKE_SESSIONS", sessions)
        .env("ZELLIJ_SESSION_NAME", "env-sess")
        .args(["read", "1"])
        .assert()
        .success();
    assert!(
        calls(&log)
            .iter()
            .any(|c| c.starts_with("--session env-sess action ")),
        "calls: {:?}",
        calls(&log)
    );
    // TASK-93 D3適用範囲限定の回帰検出: env解決経路では一覧取得（list-sessions）を
    // 呼ばない（FAKE_LOGにlist-sessions argvなし）
    assert!(
        !calls(&log).iter().any(|c| c.contains("list-sessions")),
        "calls: {:?}",
        calls(&log)
    );
}

/// session解決のerror段: 実行中複数はAmbiguousTarget（候補列出）・EXITEDのみ
/// （実行中0件）はNoTarget。いずれもexit 3
// [covers:cli-grammar.session-multiple-and-none-running-error]
#[test]
fn session_multiple_and_none_running_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (i) 実行中2件
    let (mut cmd, _) = zelper("sess-two-live");
    cmd.env(
        "FAKE_SESSIONS",
        "live-a [Created 1m ago] (current)\nlive-b [Created 2m ago]",
    )
    .args(["read", "1"])
    .assert()
    .failure()
    .code(3)
    .stderr(
        predicates::str::contains("multiple zellij sessions running")
            .and(predicates::str::contains("candidates: live-a, live-b")),
    );

    // (ii) EXITEDのみ（list-sessions成功・実行中0件）: 作成案内つき新文面
    // （TASK-93 D5: 0個の内訳〔完全0件/EXITEDのみ〕で文面を分けない）
    let (mut cmd, _) = zelper("sess-none-live");
    cmd.env(
        "FAKE_SESSIONS",
        "dead-a [Created 2h ago] (EXITED - attach to resurrect)",
    )
    .args(["read", "1"])
    .assert()
    .failure()
    .code(3)
    .stderr(
        predicates::str::contains("no running zellij session found")
            .and(predicates::str::contains("zelper add session")),
    );

    // (iii) 完全0-session環境（FAKE_SESSIONS_EMPTY: list-sessions自体がrc=1 +
    // stderr 0件文面で応答）もbackend decode（TASK-93 D1）で0件扱いとなり(ii)と
    // 同じNoTarget（exit 3）になる（従来はOperationFailed exit 5）
    let (mut cmd, _) = zelper("sess-zero-empty");
    cmd.env("FAKE_SESSIONS_EMPTY", "1")
        .args(["read", "1"])
        .assert()
        .failure()
        .code(3)
        .stderr(
            predicates::str::contains("no running zellij session found")
                .and(predicates::str::contains("zelper add session")),
        );
}

/// TASK-98: --session "a b"（空白含み完全名）はresolve_sessionの事前存在確認を
/// 完全名で通過し、backend argvの--session接頭へ完全名（a bは単一argv要素——
/// shimのaction入口照合が完全名一致で受理）が渡る（切り詰め名parseだと世界model
/// に載らずNoTargetになる系列の解消）
// [covers:cli-grammar.session-flag-whitespace-name-pass-through]
#[test]
fn session_flag_whitespace_name_passes_resolution() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("sess-flag-ws");
    cmd.env("FAKE_SESSIONS", "a b [Created 1m ago]")
        .args(["--session", "a b", "read", "1"])
        .assert()
        .success();
    assert!(
        calls(&log)
            .iter()
            .any(|c| c == "--session a b action dump-screen -p terminal_1"),
        "calls: {:?}",
        calls(&log)
    );
}

/// TASK-93 D3: --session不在名の事前存在確認。稼働ありならNoTarget（exit 3）+
/// candidates（running名一覧。EXITED行は候補から除外）でaction argv不発。完全0-session環境なら作成案内
/// 文面（candidatesなし）。EXITED名明示は「存在」扱いで解決を通過する
/// （attachによるresurrectionの正規経路を壊さない。後続action argvが発行される）
// [covers:cli-grammar.session-flag-missing-name-notarget]
#[test]
fn session_flag_missing_name_is_notarget_and_exited_passes() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (i) 稼働あり + 不在名: exit 3 NoTarget + candidates（running名のみ。EXITED行
    // dead-xは候補から除外される——TASK-93レビュー指摘2）
    let (mut cmd, log) = zelper("sess-flag-missing");
    let out = cmd
        .env(
            "FAKE_SESSIONS",
            "live-a [Created 1m ago] (current)\nlive-b [Created 2h ago]\n\
             dead-x [Created 3h ago] (EXITED - attach to resurrect)",
        )
        .args(["--session", "ghost", "read", "1", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(msg.contains("session 'ghost' not found"), "message: {msg}");
    let cands: Vec<&str> = v["error"]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    assert_eq!(cands, vec!["live-a", "live-b"]);
    // 事前存在確認で失敗するためaction argvは不発
    assert!(
        !calls(&log)
            .iter()
            .any(|c| c.starts_with("--session ghost action")),
        "calls: {:?}",
        calls(&log)
    );

    // (ii) 完全0-session環境 + 不在名: 作成案内文面（candidatesなし）
    let (mut cmd, _) = zelper("sess-flag-missing-zero");
    cmd.env("FAKE_SESSIONS_EMPTY", "1")
        .args(["--session", "ghost", "read", "1"])
        .assert()
        .failure()
        .code(3)
        .stderr(
            predicates::str::contains("session 'ghost' not found")
                .and(predicates::str::contains("no running zellij sessions"))
                .and(predicates::str::contains(
                    "create one with 'zelper add session ghost'",
                )),
        );

    // (iii) EXITED名明示は存在確認で拒否しない: 解決を通過し後続action argvが
    // FAKE_LOGに記録される（read成功）
    let (mut cmd, log) = zelper("sess-flag-exited");
    cmd.env(
        "FAKE_SESSIONS",
        "dead-a [Created 2h ago] (EXITED - attach to resurrect)",
    )
    .args(["--session", "dead-a", "read", "1"])
    .assert()
    .success();
    assert!(
        calls(&log)
            .iter()
            .any(|c| c == "--session dead-a action dump-screen -p terminal_1"),
        "calls: {:?}",
        calls(&log)
    );
}

/// rename verbのCLI parse表面: DD-1.6の例形式がrename-pane -p / rename-tab-by-id
/// argvとして発行され、shim状態のtitle/name書換でpostconditionを通過してexit 0
// [covers:cli-grammar.rename-parse-emits-typed-argv]
#[test]
fn rename_parse_emits_typed_argv() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (i) pane: FAKE_PANES_JSONのtitle書換状態管理でpostcondition通過
    let (mut cmd, log) = zelper("rename-pane-argv");
    cmd.env(
        "FAKE_PANES_JSON",
        format!("[{}]", pane_json(1, "agent-a", 0, 100, 0)),
    )
    .args(["rename", "pane", "terminal_1", "worker-1"])
    .assert()
    .success()
    .stdout("renamed pane terminal_1 -> worker-1\n");
    assert!(
        calls(&log)
            .iter()
            .any(|c| c == "--session fake-sess action rename-pane -p terminal_1 worker-1"),
        "calls: {:?}",
        calls(&log)
    );

    // (ii) tab: FAKE_TABS_JSONのname書換状態管理でpostcondition通過
    let (mut cmd, log) = zelper("rename-tab-argv");
    cmd.env(
        "FAKE_TABS_JSON",
        format!("[{}]", tab_json(0, "Tab #1", 2, 0)),
    )
    .args(["rename", "tab", "0", "agents"])
    .assert()
    .success()
    .stdout("renamed tab 0 -> agents\n");
    assert!(
        calls(&log)
            .iter()
            .any(|c| c == "--session fake-sess action rename-tab-by-id 0 agents"),
        "calls: {:?}",
        calls(&log)
    );
}

/// resize verbのCLI parse表面: pane構文は幾何不変shimでno-op 2連続打切り
/// （argv 2回・warning・0 step適用）。equalize（PANESPEC形式）は静的幾何の
/// no-progress打切りでincrease/decrease right各2回の計4回
// [covers:cli-grammar.resize-parse-emits-typed-argv]
#[test]
fn resize_parse_emits_typed_argv() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (i) pane構文（既定fixture・幾何不変）
    let (mut cmd, log) = zelper("resize-pane-argv");
    let output = cmd
        .args(["resize", "pane", "1", "grow", "left", "3"])
        .assert()
        .success()
        .get_output()
        .clone();
    let resize_calls: Vec<String> = calls(&log)
        .into_iter()
        .filter(|c| c.contains(" action resize "))
        .collect();
    assert_eq!(
        resize_calls,
        vec![
            "--session fake-sess action resize -p terminal_1 increase left",
            "--session fake-sess action resize -p terminal_1 increase left",
        ],
        "no-op 2連続打切り: {resize_calls:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("warning"), "stderr: {stderr}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0 step(s) applied"), "stdout: {stdout}");

    // (ii) equalize: 同一row・cols=10/30の2 pane（目標20との差で双方振動打切り）
    let (mut cmd, log) = zelper("resize-equalize-argv");
    let output = cmd
        .env(
            "FAKE_PANES_JSON",
            format!(
                "[{},{}]",
                pane_json(1, "agent-a", 0, 10, 0),
                pane_json(2, "agent-b", 10, 30, 0)
            ),
        )
        .args(["resize", "equalize", "1", "2"])
        .assert()
        .success()
        .get_output()
        .clone();
    let argv: Vec<String> = calls(&log)
        .iter()
        .filter(|c| c.contains(" action resize "))
        .map(|c| c.rsplit("action ").next().unwrap().to_string())
        .collect();
    assert_eq!(
        argv,
        vec![
            "resize -p terminal_1 increase right",
            "resize -p terminal_1 increase right",
            "resize -p terminal_2 decrease right",
            "resize -p terminal_2 decrease right",
        ],
        "静的幾何のno-progress打切り: {argv:?}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("approximate"), "stdout: {stdout}");
}

/// resize equalizeの対象指定排他（DD-1.2 grammar・DD-1.5）: PANESPEC群と--tabの
/// 併用はusage error（exit 2・clap標準文面）。対照正例（--tab単独）はtab 1の
/// terminal_3/4へequalizeを適用（action resize呼出）してexit 0
// [covers:cli-grammar.resize-equalize-tab-panes-conflict-is-usage-error]
#[test]
fn resize_equalize_tab_panes_conflict_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // tab 1所属・同一rowのterminal_3/4（cols=10/30）。--tab 1のID解決には
    // list-tabs応答にtab_id=1の行が必要なためFAKE_TABS_JSONを併用
    // （既定fixtureのtabs.jsonはtab 0のみ）
    let panes = format!(
        "[{},{}]",
        pane_json(3, "agent-c", 0, 10, 1),
        pane_json(4, "agent-d", 10, 30, 1)
    );
    let tabs = format!(
        "[{},{}]",
        tab_json(0, "Tab #1", 2, 0),
        tab_json(1, "Tab #2", 2, 1)
    );
    // (i) positional先・(ii) option先: いずれも同一parse結果（panes=[3]・tab=1）
    // となるため両方検出。usage errorはparse段成立だが、現行実装の黙誤動作
    // （--tab黙無視のPANESPEC経路・terminal_3単独no-opのexit 0）と切り分ける
    // ため同一fixtureを負例にも適用する（fail-first: 現行はexit 0で赤）
    let cases: [&[&str]; 2] = [
        &["resize", "equalize", "3", "--tab", "1"],
        &["resize", "equalize", "--tab", "1", "3"],
    ];
    for (i, args) in cases.iter().enumerate() {
        let (mut cmd, log) = zelper(&format!("resize-eq-tab-conflict-{i}"));
        cmd.env("FAKE_PANES_JSON", &panes)
            .env("FAKE_TABS_JSON", &tabs)
            .args(args.to_vec())
            .assert()
            .failure()
            .code(2)
            .stderr(predicates::str::contains("cannot be used with"));
        assert!(
            !calls(&log).iter().any(|c| c.contains(" action resize ")),
            "case {i} calls: {:?}",
            calls(&log)
        );
    }
    // 対照正例: --tab単独はexit 0でtab 1のterminal_3/4へequalize適用
    // （cols=10/30→目標20との振動・静的幾何のno-progress打切り）
    let (mut cmd, log) = zelper("resize-eq-tab-conflict-ok");
    cmd.env("FAKE_PANES_JSON", &panes)
        .env("FAKE_TABS_JSON", &tabs)
        .args(["resize", "equalize", "--tab", "1"])
        .assert()
        .success();
    let resize_calls: Vec<String> = calls(&log)
        .into_iter()
        .filter(|c| c.contains(" action resize "))
        .collect();
    assert!(
        resize_calls.iter().any(|c| c.contains("-p terminal_3")),
        "calls: {resize_calls:?}"
    );
    assert!(
        resize_calls.iter().any(|c| c.contains("-p terminal_4")),
        "calls: {resize_calls:?}"
    );
}

/// add pane verbのCLI parse表面: --count 2はnew-pane --tab-id 0を2回発行し、
/// shimの採番応答（terminal_3/4）とpostcondition存在確認を通過してexit 0
// [covers:cli-grammar.add-parse-emits-typed-argv]
#[test]
fn add_parse_emits_typed_argv() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("add-pane-argv");
    cmd.env(
        "FAKE_TABS_JSON",
        format!("[{}]", tab_json(0, "Tab #1", 2, 0)),
    )
    .env(
        "FAKE_PANES_JSON",
        format!(
            "[{},{}]",
            pane_json(1, "agent-a", 0, 50, 0),
            pane_json(2, "agent-b", 50, 50, 0)
        ),
    )
    .env("FAKE_NEW_PANE_TEMPLATE", FAKE_NEW_PANE_TEMPLATE)
    .args(["add", "pane", "--count", "2", "--tab", "0"])
    .assert()
    .success()
    .stdout("created pane terminal_3\ncreated pane terminal_4\n");
    let new_panes: Vec<String> = calls(&log)
        .into_iter()
        .filter(|c| c.ends_with("action new-pane --tab-id 0"))
        .collect();
    assert_eq!(new_panes.len(), 2, "calls: {:?}", calls(&log));
}

// ---------------------------------------------------------------------------
// TASK-67: addのoption map系（add-remove.tomlのL3層2条件。DD-11 addのbackend
// 引数map。excluded 1件目の条件化）。cli-grammar.add-parse-emits-typed-argv
// （TASK-65）はcount複数回発行と--tab-id map（--name無し）を検証済みのため、
// 本groupは未検証のoption map（pane: --cwd/--name/-- CMD...・tab: layout source
// 3形式のmap先）に絞る（分担は各条件descriptionへ記載）

/// add paneのbackend引数map（DD-11: --cwd/--name/--tab-id/-- commandをmap）:
/// --tab/--name/--cwd/-- CMD...がnew-pane argvの--tab-id/--name/--cwd/--へ
/// mapされpostcondition通過でexit 0する。argv順はprocess.rs new_tabの組立順
/// （--tab-id → --name → --cwd → --）
// [covers:add-remove.add-pane-option-argv-map]
#[test]
fn add_pane_option_argv_map() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("add-pane-options");
    cmd.env(
        "FAKE_TABS_JSON",
        format!("[{}]", tab_json(0, "Tab #1", 2, 0)),
    )
    .env(
        "FAKE_PANES_JSON",
        format!("[{}]", pane_json(1, "agent-a", 0, 50, 0)),
    )
    .env("FAKE_NEW_PANE_TEMPLATE", FAKE_NEW_PANE_TEMPLATE)
    .args([
        "add", "pane", "--tab", "0", "--name", "w", "--cwd", "/work", "--", "codex", "exec",
    ])
    .assert()
    .success()
    .stdout("created pane terminal_2\n");
    let argv: Vec<String> = calls(&log)
        .into_iter()
        .filter(|c| c.contains(" action new-pane "))
        .map(|c| c.rsplit("action ").next().unwrap().to_string())
        .collect();
    assert_eq!(
        argv,
        vec!["new-pane --tab-id 0 --name w --cwd /work -- codex exec"],
        "calls: {:?}",
        calls(&log)
    );
}

/// add tabのlayout source map（DD-11: --layoutは-l NAMEへ、--pathは-l PATHへ、
/// --inlineは--layout-stringへmap）: 3形式それぞれのmap先argvがFAKE_LOGで確認
/// でき、shimのnew-tab handler（採番id応答+tabs状態file反映）により
/// postcondition通過でexit 0する。(iv)は--layoutへのpath形式入力（'/'入り）が
/// 拒否・書換えされず-lへ透過するzellerの契約検証（TASK-91: 適用しない決定。
/// zellij -lがpathを正規受理するため。DD-11 tab --layout受理仕様）。
/// TASK-102: (v)〜(viii)でZELLIJ_LAYOUT_DIR設定時の--layout-dir付与契約を検証
/// （名前探索dirを一覧基準へ統一。設計tmp/tasks/tm-102/design-layout-dir-unification.md §5/§7）。
/// (v)(viii)は付与正例（flag前置: new-tab --layout-dir <dir> ... -l <name>。
/// shimの-l単一値検査と整合する順序）、(vi)は空文字envのargv不変、(vii)は
/// --path/--inline/sourceなしのargv不変（全source誤付与の検出）。env制御は
/// 各Commandの.env()/.env_remove()で子process環境のみ隔離（設計§7 minor-1。
/// 親process envは並列test汚染のため不使用）
/// --name連番suffixと排他負例（3source併用）はadd-removeのL2既存条件が担う
// [covers:add-remove.add-tab-layout-source-argv-map]
#[test]
fn add_tab_layout_source_argv_map() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (ZELLIJ_LAYOUT_DIR設定: None=未設定保証〔env_remove〕/ Some(v)=設定〔空文字含む〕,
    //  layout source引数列, 期待new-tab argv断片)。layout名はlayout_dir解決のため
    //  shimでは存在確認されない（mapが本条件の対象）
    for (i, (ldir, source, expect)) in [
        // (i)〜(iv): ZELLIJ_LAYOUT_DIR未設定環境（付与されない。env_removeで保証）
        (None, vec!["--layout", "mylayout"], "new-tab -l mylayout"),
        (None, vec!["--path", "./x.kdl"], "new-tab -l ./x.kdl"),
        (
            None,
            vec!["--inline", "layout {}"],
            "new-tab --layout-string layout {}",
        ),
        (None, vec!["--layout", "./x.kdl"], "new-tab -l ./x.kdl"),
        // (v) 非空env + 名前 → --layout-dir付与の正例（現行実装では--layout-dir不在で赤。
        //  実装工程で緑化）
        (
            Some("/ly"),
            vec!["--layout", "mylayout"],
            "new-tab --layout-dir /ly -l mylayout",
        ),
        // (vi) 空文字env + 名前 → argv不変（空文字誤付与の検出）
        (
            Some(""),
            vec!["--layout", "mylayout"],
            "new-tab -l mylayout",
        ),
        // (vii) 非空env + --path / --inline / sourceなし → argv不変（全source誤付与の検出）
        (Some("/ly"), vec!["--path", "./x.kdl"], "new-tab -l ./x.kdl"),
        (
            Some("/ly"),
            vec!["--inline", "layout {}"],
            "new-tab --layout-string layout {}",
        ),
        (Some("/ly"), vec![], "new-tab"),
        // (viii) 非空env + --layout path形式値 → flag付与・-l値不変
        (
            Some("/ly"),
            vec!["--layout", "./x.kdl"],
            "new-tab --layout-dir /ly -l ./x.kdl",
        ),
    ]
    .iter()
    .enumerate()
    {
        let (mut cmd, log) = zelper(&format!("add-tab-layout-{i}"));
        let mut args: Vec<&str> = vec!["add", "tab"];
        args.extend(source.iter().copied());
        match ldir {
            Some(v) => {
                cmd.env("ZELLIJ_LAYOUT_DIR", v);
            }
            None => {
                cmd.env_remove("ZELLIJ_LAYOUT_DIR");
            }
        }
        cmd.env("FAKE_TABS_JSON", format!("[{}]", tab_json(0, "work", 2, 0)))
            .env("FAKE_NEW_TAB_TEMPLATE", FAKE_NEW_TAB_TEMPLATE)
            .args(&args)
            .assert()
            .success()
            .stdout("created tab 1\n");
        // 末尾spaceなしprefix: sourceなしcase（argv空・行末がnew-tabで終わる）も拾う
        let argv: Vec<String> = calls(&log)
            .into_iter()
            .filter(|c| c.contains(" action new-tab"))
            .map(|c| c.rsplit("action ").next().unwrap().to_string())
            .collect();
        assert_eq!(
            argv,
            vec![expect.to_string()],
            "case {i} calls: {:?}",
            calls(&log)
        );
    }
}

/// remove verbのCLI parse表面: pane削除はclose-pane -p argvを発行し行除去で
/// postcondition通過。tab --empty --dry-runは空tab（tab1）のみplan出力し
/// close-tab argvは不発
// [covers:cli-grammar.remove-parse-emits-typed-argv]
#[test]
fn remove_parse_emits_typed_argv() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (i) pane: FAKE_PANES_JSONの行除去状態管理でpostcondition通過
    let (mut cmd, log) = zelper("remove-pane-argv");
    cmd.env(
        "FAKE_PANES_JSON",
        format!("[{}]", pane_json(1, "agent-a", 0, 100, 0)),
    )
    .args(["remove", "pane", "1", "--yes"])
    .assert()
    .success()
    .stdout("removed pane terminal_1\n");
    assert!(
        calls(&log)
            .iter()
            .any(|c| c == "--session fake-sess action close-pane -p terminal_1"),
        "calls: {:?}",
        calls(&log)
    );

    // (ii) tab --empty --dry-run: tab0=非空（tiled 2）・tab1=空（tiled 0）
    let (mut cmd, log) = zelper("remove-tab-empty-plan");
    cmd.env(
        "FAKE_TABS_JSON",
        format!(
            "[{},{}]",
            tab_json(0, "work", 2, 0),
            tab_json(1, "empty", 0, 1)
        ),
    )
    .args(["remove", "tab", "--empty", "--dry-run"])
    .assert()
    .success()
    .stdout("[plan] close tab 1\n");
    assert!(
        !calls(&log).iter().any(|c| c.contains("close-tab")),
        "calls: {:?}",
        calls(&log)
    );
}

/// --tab×--all併合の正例（--tab内のall）: tab 1のterminal_3/4のみが対象となり、
/// --all単独なら4件となる構成で併合絞り込みを区別する
// [covers:cli-grammar.tab-and-all-filter-merge-positive]
#[test]
fn tab_and_all_filter_merge_positive() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("read-tab1-all");
    let out = cmd
        .env(
            "FAKE_TABS_JSON",
            format!(
                "[{},{}]",
                tab_json(0, "work", 2, 0),
                tab_json(1, "agents", 2, 1)
            ),
        )
        .env(
            "FAKE_PANES_JSON",
            format!(
                "[{},{},{},{}]",
                pane_json(1, "agent-a", 0, 50, 0),
                pane_json(2, "agent-b", 50, 50, 0),
                pane_json(3, "agent-c", 0, 50, 1),
                pane_json(4, "agent-d", 50, 50, 1)
            ),
        )
        .args(["read", "--tab", "1", "--all", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let targets: Vec<&str> = v["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["target"].as_str().unwrap())
        .collect();
    assert_eq!(targets, vec!["terminal_3", "terminal_4"]);
    let dumps: Vec<String> = calls(&log)
        .into_iter()
        .filter(|c| c.contains(" action dump-screen "))
        .collect();
    assert_eq!(dumps.len(), 2, "calls: {dumps:?}");
    assert!(
        dumps
            .iter()
            .all(|c| c.ends_with("-p terminal_3") || c.ends_with("-p terminal_4")),
        "calls: {dumps:?}"
    );
}

/// --dry-run×--yes併用の正例: 併用は可で--yesは無視され、注意行（stderr）を
/// 出力する。dry-run優先でclose-paneは不発・plan出力で終わる
// [covers:cli-grammar.dry-run-with-yes-note-output]
#[test]
fn dry_run_with_yes_note_output() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("remove-dryrun-yes");
    let output = cmd
        .args(["remove", "pane", "1", "--yes", "--dry-run"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("zelper: note: --yes is ignored when --dry-run is given"),
        "stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[plan] close pane terminal_1"),
        "stdout: {stdout}"
    );
    assert!(
        !calls(&log).iter().any(|c| c.contains("close-pane")),
        "calls: {:?}",
        calls(&log)
    );
}

/// completionのshell種別: zsh/fishがbashと同様に空でないscriptを出力し内容は
/// 種別毎に相異する。非対応shell名（tcsh）はclap ValueEnum拒否でexit 2
// [covers:cli-grammar.completion-shell-variants]
#[test]
fn completion_shell_variants() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let get = |tag: &str, shell: &str| -> String {
        let (mut cmd, _) = zelper(tag);
        let out = cmd
            .args(["completion", shell])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        String::from_utf8(out).unwrap()
    };
    let zsh = get("completion-zsh", "zsh");
    let fish = get("completion-fish", "fish");
    let bash = get("completion-bash2", "bash");
    for (name, s) in [("zsh", &zsh), ("fish", &fish)] {
        assert!(s.contains("zelper"), "{name}: {}", first_line(s));
        assert!(s.len() > 100, "{name}: {} bytes", s.len());
    }
    assert_ne!(zsh, fish);
    assert_ne!(zsh, bash);
    assert_ne!(fish, bash);

    let (mut cmd, _) = zelper("completion-tcsh");
    cmd.args(["completion", "tcsh"]).assert().failure().code(2);
}

// ---------------------------------------------------------------------------
// remove session（TASK-84・remove-session.tomlのL3層16条件。設計§2/§7。
// shimのFAKE_SESSIONS〔EXITED行混在可〕・FAKE_SESSIONS_EMPTY〔0-session環境〕・
// FAKE_FAIL_DELETE_SESSION / FAKE_MUTE_DELETE_SESSION・${FAKE_LOG}.deleted
// 〔delete-session作用のlist-sessions応答反映〕。L2層5条件は
// tests/fake_backend/remove_session.rs）

/// remove sessionの4 selector排他（§2.0）: NAME...（1件以上）/ --all / --dead /
/// --me は相互排他かつちょうど1つ。不在・併用はusage error（exit 2）で状態変更
/// 前に拒否。対照正例（NAME単独）はexit 2にならず実行経路へ進む
// [covers:remove-session.selector-exactly-one-usage-error]
#[test]
fn remove_session_selector_exactly_one_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let cases: [&[&str]; 5] = [
        // (i) 全selector不在
        &["remove", "session", "--yes"],
        // (ii) --all × --dead 併用
        &["remove", "session", "--all", "--dead", "--yes"],
        // (iii) NAME × --all 併用
        &["remove", "session", "fake-sess", "--all", "--yes"],
        // (iv) NAME × --me 併用
        &["remove", "session", "fake-sess", "--me", "--yes"],
        // (v) --dead × --me 併用
        &["remove", "session", "--dead", "--me", "--yes"],
    ];
    for (i, args) in cases.iter().enumerate() {
        let (mut cmd, log) = zelper(&format!("remove-sel-usage-{i}"));
        cmd.args(args.to_vec()).assert().failure().code(2);
        assert!(
            !calls(&log).iter().any(|c| c.starts_with("delete-session")),
            "case {i} calls: {:?}",
            calls(&log)
        );
    }
    // 対照正例: NAME単独はexit 2にならず実行経路へ進む（実行成功）
    let (mut cmd, log) = zelper("remove-sel-usage-ok");
    cmd.args(["remove", "session", "fake-sess", "--yes"])
        .assert()
        .success();
    assert!(
        calls(&log)
            .iter()
            .any(|c| c == "delete-session fake-sess -f"),
        "calls: {:?}",
        calls(&log)
    );
}

/// 明示NAMEの不在分類（§2.9）: 指定NAMEがlist_sessionsの行（RUNNING・EXITEDとも）
/// に存在しない場合はNoTarget（exit 3）でcandidates=全行名（EXITED込み）。一覧が
/// 完全0件の場合は「no zellij sessions found」の文面でNoTarget
// [covers:remove-session.missing-name-notarget-with-candidates]
#[test]
fn remove_session_missing_name_is_notarget_with_candidates() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (i) 不在NAME: candidatesに稼働名とEXITED名の両方（EXITED込み）
    let (mut cmd, log) = zelper("remove-missing");
    let out = cmd
        .env(
            "FAKE_SESSIONS",
            "fake-sess [Created 1m ago] (current)\ndeadx [Created 2m ago] (EXITED - attach to resurrect)",
        )
        .args(["remove", "session", "ghost", "--yes", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let cands = v["error"]["candidates"].as_array().unwrap();
    assert!(
        cands.iter().any(|c| c == "fake-sess"),
        "candidates: {cands:?}"
    );
    assert!(cands.iter().any(|c| c == "deadx"), "candidates: {cands:?}");
    assert!(
        !calls(&log).iter().any(|c| c.starts_with("delete-session")),
        "calls: {:?}",
        calls(&log)
    );

    // (ii) 行0件（0-session環境）: no zellij sessions found文面
    let (mut cmd, _) = zelper("remove-missing-zero");
    let out = cmd
        .env("FAKE_SESSIONS_EMPTY", "1")
        .args(["remove", "session", "foo", "--yes", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(msg.contains("no zellij sessions found"), "message: {msg}");
}

/// dry-runのplan表示契約（§2.6）: 対象毎のstate（running/dead）とlist_tabs_for
/// 由来のtabs/panes数。dead対象はlist_tabs_forを呼ばずtabs/panes=null固定
/// （EXITED名へのaction系呼び出しの副作用が未検証のため呼ばない）。状態変更は
/// 一切行わない
// [covers:remove-session.dry-run-plan-states-and-counts]
#[test]
fn remove_session_dry_run_plan_states_and_counts() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sessions =
        "foo [Created 1m ago] (current)\ndeadx [Created 2m ago] (EXITED - attach to resurrect)";
    // human: [plan]行×2 + 合計行（tabs/panesはfixtures/tabs.json基準: 1 tab・2 pane）
    let (mut cmd, log) = zelper("remove-dryrun-plan");
    let out = cmd
        .env("FAKE_SESSIONS", sessions)
        .args(["remove", "session", "foo", "deadx", "--dry-run"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();
    assert_eq!(
        stdout,
        "[plan] remove session 'foo' (running, 1 tabs, 2 panes)\n\
         [plan] remove session 'deadx' (dead)\n\
         2 session(s) to remove; pass --yes to execute\n",
        "stdout: {stdout:?}"
    );
    // 状態変更なし・dead対象はlist_tabs_for不呼出（running名のみlist-tabs呼出）
    let cs = calls(&log);
    assert!(
        !cs.iter().any(|c| c.starts_with("delete-session")),
        "calls: {cs:?}"
    );
    assert!(
        cs.iter()
            .any(|c| c == "--session foo action list-tabs -a --json"),
        "calls: {cs:?}"
    );
    assert!(
        !cs.iter().any(|c| c.contains("deadx")),
        "dead対象へのaction系呼出: {cs:?}"
    );

    // JSON: data.plan.targets[]（dead対象はtabs/panes=null）
    let (mut cmd, log) = zelper("remove-dryrun-plan-json");
    let out = cmd
        .env("FAKE_SESSIONS", sessions)
        .args(["remove", "session", "foo", "deadx", "--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    let targets = v["data"]["plan"]["targets"].as_array().unwrap();
    assert_eq!(targets.len(), 2, "targets: {targets:?}");
    assert_eq!(targets[0]["name"], "foo");
    assert_eq!(targets[0]["state"], "running");
    assert_eq!(targets[0]["tabs"], 1);
    assert_eq!(targets[0]["panes"], 2);
    assert_eq!(targets[1]["name"], "deadx");
    assert_eq!(targets[1]["state"], "dead");
    assert_eq!(targets[1]["tabs"], serde_json::Value::Null);
    assert_eq!(targets[1]["panes"], serde_json::Value::Null);
    let cs = calls(&log);
    assert!(
        !cs.iter().any(|c| c.starts_with("delete-session")),
        "calls: {cs:?}"
    );
    assert!(!cs.iter().any(|c| c.contains("deadx")), "calls: {cs:?}");
}

/// 削除のbackend argv契約（§2.1/§2.7）: 各対象は `zellij delete-session <NAME> -f`
/// のtop-level positional形式（--session接頭なし）で1回。kill-session /
/// kill-all-sessions / delete-all-sessionsは発行しない
// [covers:remove-session.delete-session-f-argv]
#[test]
fn remove_session_emits_delete_session_f_argv() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("remove-delargv");
    cmd.env("FAKE_SESSIONS", "foo [Created 1m ago] (current)")
        .args(["remove", "session", "foo", "--yes"])
        .assert()
        .success();
    let cs = calls(&log);
    let dels: Vec<&String> = cs
        .iter()
        .filter(|c| c.starts_with("delete-session"))
        .collect();
    assert_eq!(
        dels,
        vec![&"delete-session foo -f".to_string()],
        "calls: {cs:?}"
    );
    // top-level positional形式（--session接頭argvなし）
    assert!(
        !cs.iter().any(|c| c.starts_with("--session")),
        "calls: {cs:?}"
    );
    // kill/delete-all系zellij commandは発行しない（§2.2不採用）
    assert!(
        !cs.iter().any(|c| c.contains("kill-session")
            || c.contains("kill-all")
            || c.contains("delete-all")),
        "calls: {cs:?}"
    );
}

/// 部分失敗の継続実行と報告（§2.12）: 一部targetのdelete-session呼出が非zero終了
/// しても残targetを実行し、全体はPartialFailure（exit 6）でper-target結果
/// （data.results[]）付きで報告する。rollbackしない。失敗は先頭target（foo）に
/// 注入——最後のtargetの失敗では早期打ち切り実装でも通るため、後続target（bar）
/// の呼出・成功結果まで検証できる構成とする
// [covers:remove-session.per-target-partial-failure]
#[test]
fn remove_session_partial_failure_is_exit6_with_results() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("remove-partial");
    let out = cmd
        .env(
            "FAKE_SESSIONS",
            "foo [Created 1m ago] (current)\nbar [Created 2m ago]",
        )
        .env("FAKE_FAIL_DELETE_SESSION", "foo")
        .args(["remove", "session", "foo", "bar", "--yes", "--json"])
        .assert()
        .failure()
        .code(6)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "PartialFailure");
    let results = v["error"]["data"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 2, "results: {results:?}");
    let foo = results.iter().find(|r| r["target"] == "foo").unwrap();
    let bar = results.iter().find(|r| r["target"] == "bar").unwrap();
    assert_eq!(foo["ok"], false, "results: {results:?}");
    assert_eq!(bar["ok"], true, "results: {results:?}");
    assert!(
        foo["error"].as_str().unwrap_or_default().contains("boom"),
        "results: {results:?}"
    );
    // 残target実行（先頭失敗後も指定順に後続targetを呼ぶ——早期打ち切りでは
    // barの呼出・成功結果が欠ける）
    let cs = calls(&log);
    let dels: Vec<&String> = cs
        .iter()
        .filter(|c| c.starts_with("delete-session"))
        .collect();
    assert_eq!(
        dels,
        vec![
            &"delete-session foo -f".to_string(),
            &"delete-session bar -f".to_string(),
        ],
        "calls: {cs:?}"
    );
}

/// NAME経路の自session保護（§2.3）: ZELLIJ_SESSION_NAMEが解決済みtarget集合に
/// 含まれる場合、--meなしの実行は状態変更前にPreflight（exit 7）で拒否
/// （messageは remove session --me へ案内）。他target（other）も実行されない
// [covers:remove-session.self-name-without-me-refused]
#[test]
fn remove_session_self_name_without_me_is_refused() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("remove-self-refused");
    let out = cmd
        .env(
            "FAKE_SESSIONS",
            "fake-sess [Created 1m ago] (current)\nother [Created 2m ago]",
        )
        .env("ZELLIJ_SESSION_NAME", "fake-sess")
        .args(["remove", "session", "fake-sess", "other", "--yes", "--json"])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "Preflight");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(msg.contains("--me"), "message: {msg}");
    assert!(
        !calls(&log).iter().any(|c| c.starts_with("delete-session")),
        "fail-fast（他targetも不発）: {:?}",
        calls(&log)
    );
}

/// --meのenv解決必須（§2.3）: ZELLIJ_SESSION_NAME不在での--me実行はdry-runでも
/// Preflight（exit 7）で拒否される（plan表示に解決名が必要なため）
// [covers:remove-session.me-requires-env]
#[test]
fn remove_session_me_requires_env() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    for (i, args) in [
        vec!["remove", "session", "--me", "--yes", "--json"],
        vec!["remove", "session", "--me", "--dry-run", "--json"],
    ]
    .iter()
    .enumerate()
    {
        let (mut cmd, log) = zelper(&format!("remove-me-env-{i}"));
        let out = cmd
            .args(args.clone())
            .assert()
            .failure()
            .code(7)
            .get_output()
            .stdout
            .clone();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["error"]["class"], "Preflight");
        let msg = v["error"]["message"].as_str().unwrap();
        assert!(
            msg.contains("requires running inside a zellij session"),
            "message: {msg}"
        );
        assert!(msg.contains("ZELLIJ_SESSION_NAME"), "message: {msg}");
        assert!(
            !calls(&log).iter().any(|c| c.starts_with("delete-session")),
            "case {i} calls: {:?}",
            calls(&log)
        );
    }
}

/// --meの解決名実行契約（§2.3）: env解決名を対象として通常経路
/// （事前存在確認→delete-session <name> -f→報告）で実行する。env解決名が
/// list_sessionsの行に存在しない場合はNoTarget（exit 3）
// [covers:remove-session.me-selector-argv-uses-resolved-name]
#[test]
fn remove_session_me_uses_resolved_name() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (i) env解決名が行に存在: 解決名でdelete-session実行
    let (mut cmd, log) = zelper("remove-me-resolved");
    cmd.env("ZELLIJ_SESSION_NAME", "fake-sess")
        .args(["remove", "session", "--me", "--yes"])
        .assert()
        .success();
    assert_eq!(
        calls(&log)
            .iter()
            .filter(|c| c.starts_with("delete-session"))
            .count(),
        1,
        "calls: {:?}",
        calls(&log)
    );
    assert!(
        calls(&log)
            .iter()
            .any(|c| c == "delete-session fake-sess -f"),
        "env解決名での実行: {:?}",
        calls(&log)
    );

    // (ii) env解決名が行に不存在: NoTarget（env値を無検証でtargetにしない）
    let (mut cmd, log) = zelper("remove-me-ghost");
    let out = cmd
        .env("ZELLIJ_SESSION_NAME", "ghost")
        .args(["remove", "session", "--me", "--yes", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    assert!(
        !calls(&log).iter().any(|c| c.starts_with("delete-session")),
        "calls: {:?}",
        calls(&log)
    );
}

/// --me実行前のstderr警告行（§2.3）: 自session削除はzelper自身のprocess treeを
/// 巻き込みうるため、delete-session呼出の前にstderrへ警告行を出す。shimの
/// delete-session handler開始marker（親zellerのstderrへ直接出力）より前に警告行が
/// 現れることで順序を検証する（終了後の存在確認だけでは削除後に移しても通る）。
/// 実行は通常経路（fake shim環境では自壊しないため成功報告まで完結）
// [covers:remove-session.me-preflight-warning-before-execution]
#[test]
fn remove_session_me_warns_before_execution() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("remove-me-warning");
    let output = cmd
        .env("ZELLIJ_SESSION_NAME", "fake-sess")
        .args(["remove", "session", "--me", "--yes"])
        .assert()
        .success()
        .get_output()
        .clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let warn_pos = stderr
        .find("removing the current session")
        .unwrap_or_else(|| panic!("warning行なし: {stderr}"));
    let marker_pos = stderr
        .find("shim: delete-session handler start")
        .unwrap_or_else(|| panic!("delete handler開始markerなし: {stderr}"));
    assert!(
        warn_pos < marker_pos,
        "warning行がdelete-session開始より前であること: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("removed session 'fake-sess' (current session)"),
        "stdout: {stdout}"
    );
    assert!(
        calls(&log)
            .iter()
            .any(|c| c == "delete-session fake-sess -f"),
        "calls: {:?}",
        calls(&log)
    );
}

/// --me dry-runのcurrent注記（§2.6）: env解決名のplan行に現在sessionである旨の
/// 注記を付ける（human: 行末尾 [current session]・JSON: current: true）
// [covers:remove-session.me-dry-run-current-annotation]
#[test]
fn remove_session_me_dry_run_annotates_current() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // human: plan行の [current session] 注記（tabs/panesはfixtures/tabs.json基準）
    let (mut cmd, log) = zelper("remove-me-dryrun");
    let out = cmd
        .env("ZELLIJ_SESSION_NAME", "fake-sess")
        .args(["remove", "session", "--me", "--dry-run"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();
    assert!(
        stdout.lines().any(|l| l
            == "[plan] remove session 'fake-sess' (running, 1 tabs, 2 panes) [current session]"),
        "stdout: {stdout:?}"
    );
    assert!(
        !calls(&log).iter().any(|c| c.starts_with("delete-session")),
        "calls: {:?}",
        calls(&log)
    );

    // JSON: 当該targetへ current: true field
    let (mut cmd, log) = zelper("remove-me-dryrun-json");
    let out = cmd
        .env("ZELLIJ_SESSION_NAME", "fake-sess")
        .args(["remove", "session", "--me", "--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let targets = v["data"]["plan"]["targets"].as_array().unwrap();
    assert_eq!(targets.len(), 1, "targets: {targets:?}");
    assert_eq!(targets[0]["name"], "fake-sess");
    assert_eq!(targets[0]["current"], true, "targets: {targets:?}");
    assert!(
        !calls(&log).iter().any(|c| c.starts_with("delete-session")),
        "calls: {:?}",
        calls(&log)
    );
}

/// --allの自session除外（§2.3）: 対象集合からZELLIJ_SESSION_NAME該当行を除外し、
/// 除外した旨をstderrのwarning行で通知する（stdout契約は不変）。除外対象への
/// delete-sessionは発行されない
// [covers:remove-session.all-excludes-self-with-warning]
#[test]
fn remove_session_all_excludes_self_with_warning() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("remove-all-self");
    let output = cmd
        .env(
            "FAKE_SESSIONS",
            "fake-sess [Created 1m ago] (current)\nother [Created 2m ago]",
        )
        .env("ZELLIJ_SESSION_NAME", "fake-sess")
        .args(["remove", "session", "--all", "--yes"])
        .assert()
        .success()
        .get_output()
        .clone();
    let cs = calls(&log);
    let dels: Vec<&String> = cs
        .iter()
        .filter(|c| c.starts_with("delete-session"))
        .collect();
    assert_eq!(
        dels,
        vec![&"delete-session other -f".to_string()],
        "calls: {cs:?}"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("warning") && stderr.contains("current session"),
        "stderr: {stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("removed session 'other'") && !stdout.contains("fake-sess"),
        "stdout: {stdout}"
    );
}

/// --allの除外後0件分類（§2.3）: 自sessionのみが存在する環境での--all実行は
/// self除外により対象0件になるためNoTarget（exit 3）。messageは
/// remove session --me への案内
// [covers:remove-session.all-self-only-notarget]
#[test]
fn remove_session_all_self_only_is_notarget() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("remove-all-selfonly");
    let out = cmd
        .env("FAKE_SESSIONS", "fake-sess [Created 1m ago] (current)")
        .env("ZELLIJ_SESSION_NAME", "fake-sess")
        .args(["remove", "session", "--all", "--yes", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(msg.contains("--me"), "message: {msg}");
    assert!(
        !calls(&log).iter().any(|c| c.starts_with("delete-session")),
        "calls: {:?}",
        calls(&log)
    );
}

/// --dead selectorの対象選択（§2.2）: 対象はlist_sessionsのEXITED行のみ
/// （RUNNING行は対象外）。EXITED行が1件もない環境ではNoTarget（exit 3・
/// no sessions to remove）
// [covers:remove-session.dead-selector-exited-rows-only]
#[test]
fn remove_session_dead_selects_exited_rows_only() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (i) EXITED行あり: EXITED行のみ削除（RUNNING行は対象外）
    let (mut cmd, log) = zelper("remove-dead-hit");
    cmd.env(
        "FAKE_SESSIONS",
        "fake-sess [Created 1m ago] (current)\ndeadx [Created 2m ago] (EXITED - attach to resurrect)",
    )
    .args(["remove", "session", "--dead", "--yes"])
    .assert()
    .success();
    let cs = calls(&log);
    let dels: Vec<&String> = cs
        .iter()
        .filter(|c| c.starts_with("delete-session"))
        .collect();
    assert_eq!(
        dels,
        vec![&"delete-session deadx -f".to_string()],
        "calls: {cs:?}"
    );

    // (ii) EXITED行なし（RUNNINGのみ）: NoTarget
    let (mut cmd, log) = zelper("remove-dead-none");
    let out = cmd
        .env("FAKE_SESSIONS", "fake-sess [Created 1m ago] (current)")
        .args(["remove", "session", "--dead", "--yes", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let msg = v["error"]["message"].as_str().unwrap();
    assert!(msg.contains("no sessions to remove"), "message: {msg}");
    assert!(
        !calls(&log).iter().any(|c| c.starts_with("delete-session")),
        "calls: {:?}",
        calls(&log)
    );
}

/// TASK-99 §2.2-1: --dead対象選択はmarker部分文字列のfield汚染を受けない。
/// 名前2語目にEXITEDを含むrunning行（zellij外部由来の空白含み名想定）が
/// exited=trueに誤parseされると、--deadが当該行を汚染対象に選びtruncated名
/// （先頭word x）でdelete-sessionを発行する破壊系系列になる。anchor化後は
/// 本物EXITED suffix行のみが対象
// [covers:remove-session.dead-selector-immune-to-marker-substring-pollution]
#[test]
fn remove_session_dead_immune_to_marker_substring_pollution() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sessions = "x EXITED [Created 1m ago]\n\
                   dead1 [Created 2m ago] (EXITED - attach to resurrect)";
    // 実行1: --dry-run --json → planは本物deadのみ・delete-session不発
    let (mut cmd, log) = zelper("remove-dead-polluted-dryrun");
    let out = cmd
        .env("FAKE_SESSIONS", sessions)
        .args(["remove", "session", "--dead", "--dry-run", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    let targets = v["data"]["plan"]["targets"].as_array().unwrap();
    assert_eq!(
        targets.len(),
        1,
        "汚染running行（truncated名x）がplanへ混入: {targets:?}"
    );
    assert_eq!(targets[0]["name"], "dead1");
    assert_eq!(targets[0]["state"], "dead");
    assert!(
        !calls(&log).iter().any(|c| c.starts_with("delete-session")),
        "dry-runでdelete-sessionが発行された: {:?}",
        calls(&log)
    );

    // 実行2: --yes --json → delete-sessionは本物deadのみ（truncated名xへの呼出なし）
    let (mut cmd, log) = zelper("remove-dead-polluted-yes");
    let out = cmd
        .env("FAKE_SESSIONS", sessions)
        .args(["remove", "session", "--dead", "--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    let results = v["data"]["results"].as_array().unwrap();
    let names: Vec<&str> = results
        .iter()
        .map(|r| r["target"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["dead1"],
        "報告へ汚染行（truncated名x）が混入: {results:?}"
    );
    assert!(
        results.iter().all(|r| r["ok"] == true),
        "results: {results:?}"
    );
    let cs = calls(&log);
    let dels: Vec<&String> = cs
        .iter()
        .filter(|c| c.starts_with("delete-session"))
        .collect();
    assert_eq!(
        dels,
        vec![&"delete-session dead1 -f".to_string()],
        "truncated名xへのdelete-session呼出あり: {cs:?}"
    );
}

/// TASK-98 §2.1-1/§2.3-2: 空白含みsession名は完全名でtargetingされる。
/// (i) `remove session "a b"` → `delete-session a b -f`（完全名・単一argv要素は
/// shimのargv要素数検査が保証）。(ii) 切り詰め名（先頭word a）入力は世界modelに
/// 存在しないためNoTarget・candidates=完全名一覧（誤射系列の消滅検出）。
/// (iii) 先頭word衝突共存（a と a b）で a b を削除しても非対象 a 行は残存
/// （shim削除反映の完全名一致——先頭word一致の反映が a 行まで消す巻き込み検出）
// [covers:remove-session.whitespace-name-full-name-targeting]
#[test]
fn remove_session_whitespace_name_full_name_targeting() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sessions = "a b [Created 1m ago]\nc [Created 3m ago]";
    // (i) 完全名targeting: delete-session a b -f のみ発行・cへの呼出なし
    let (mut cmd, log) = zelper("remove-ws-full");
    let out = cmd
        .env("FAKE_SESSIONS", sessions)
        .args(["remove", "session", "a b", "--yes", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    let cs = calls(&log);
    let dels: Vec<&String> = cs
        .iter()
        .filter(|c| c.starts_with("delete-session"))
        .collect();
    assert_eq!(
        dels,
        vec![&"delete-session a b -f".to_string()],
        "calls: {cs:?}"
    );

    // (ii) 切り詰め名入力: NoTarget・candidates=完全名一覧・delete-session不発
    let (mut cmd, log) = zelper("remove-ws-truncated");
    let out = cmd
        .env("FAKE_SESSIONS", sessions)
        .args(["remove", "session", "a", "--yes", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    assert_eq!(
        v["error"]["candidates"],
        serde_json::json!(["a b", "c"]),
        "candidates: {}",
        v["error"]["candidates"]
    );
    assert!(
        !calls(&log).iter().any(|c| c.starts_with("delete-session")),
        "calls: {:?}",
        calls(&log)
    );

    // (iii) 先頭word衝突共存: a b削除の巻き込みで非対象 a 行が消えないこと。
    // 事後一覧は同一shim状態file（${FAKE_LOG}.deleted反映済み）を参照する2回目の
    // list sessionsで観測する（zelper()再呼び出しは状態を初期化するため不可）
    let (dir, log) = setup_fake_zellij("remove-ws-coexist");
    let coexist = "a [Created 1m ago]\na b [Created 2m ago]";
    command_on(&dir, &log)
        .env("FAKE_SESSIONS", coexist)
        .args(["remove", "session", "a b", "--yes"])
        .assert()
        .success();
    let cs = calls(&log);
    let dels: Vec<&String> = cs
        .iter()
        .filter(|c| c.starts_with("delete-session"))
        .collect();
    assert_eq!(
        dels,
        vec![&"delete-session a b -f".to_string()],
        "calls: {cs:?}"
    );
    let out = command_on(&dir, &log)
        .env("FAKE_SESSIONS", coexist)
        .args(["list", "sessions", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let names: Vec<&str> = v["data"]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["a"],
        "a b削除の反映が非対象 a 行まで消した: {names:?}"
    );

    // (iv) marker含有名共存（実装レビューmajor）: a と a [Created 5m ago]
    // （true name "a [Created 5m ago]"）が並存する環境で a を削除しても
    // marker含有行は巻き込まれない（shim削除反映の完全名一致——接頭一致だと
    // `a [Created `接頭が両行に一致し後者も消える）
    let (dir, log) = setup_fake_zellij("remove-ws-marker-coexist");
    let coexist_marker = "a [Created 1m ago]\na [Created 5m ago] [Created 2m ago]";
    command_on(&dir, &log)
        .env("FAKE_SESSIONS", coexist_marker)
        .args(["remove", "session", "a", "--yes"])
        .assert()
        .success();
    let cs = calls(&log);
    let dels: Vec<&String> = cs
        .iter()
        .filter(|c| c.starts_with("delete-session"))
        .collect();
    assert_eq!(
        dels,
        vec![&"delete-session a -f".to_string()],
        "calls: {cs:?}"
    );
    let out = command_on(&dir, &log)
        .env("FAKE_SESSIONS", coexist_marker)
        .args(["list", "sessions", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let names: Vec<&str> = v["data"]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["a [Created 5m ago]"],
        "a削除がmarker含有名行まで消した: {names:?}"
    );
}

/// TASK-98 §2.2: --allのself除外は完全名一致。ZELLIJ_SESSION_NAME="a b"（空白含み
/// 自session）が世界modelの行（name=完全名）と一致して除外され、otherのみ削除
/// （切り詰め名比較だと自session行が除外されず delete-session a が発行され、
/// postconditionもstill listedで失敗する）
// [covers:remove-session.all-self-exclusion-with-whitespace-name]
#[test]
fn remove_session_all_excludes_self_with_whitespace_name() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("remove-all-ws-self");
    let output = cmd
        .env(
            "FAKE_SESSIONS",
            "a b [Created 1m ago]\nother [Created 1m ago]",
        )
        .env("ZELLIJ_SESSION_NAME", "a b")
        .args(["remove", "session", "--all", "--yes"])
        .assert()
        .success()
        .get_output()
        .clone();
    let cs = calls(&log);
    let dels: Vec<&String> = cs
        .iter()
        .filter(|c| c.starts_with("delete-session"))
        .collect();
    assert_eq!(
        dels,
        vec![&"delete-session other -f".to_string()],
        "calls: {cs:?}"
    );
    // 除外warning行は完全名 a bで通知される。実装レビューminor: warning行を
    // 抽出して件数（ちょうど1行）と内容（行内一致）で検証する——stderr全体の
    // contains組合せではwarning重複・別行分割で通るため
    let stderr = String::from_utf8_lossy(&output.stderr);
    let warnings: Vec<&str> = stderr.lines().filter(|l| l.contains("warning")).collect();
    assert_eq!(warnings.len(), 1, "warning行はちょうど1行: {stderr}");
    assert!(
        warnings[0].contains("current session") && warnings[0].contains("'a b'"),
        "warning行: {}",
        warnings[0]
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("removed session 'other'") && !stdout.contains("removed session 'a'"),
        "stdout: {stdout}"
    );
}

/// 0-session環境の事前確認（§2.9）: NAME指定（no zellij sessions found文面）・
/// --all / --dead（no sessions to remove文面）・--me（env解決名が行になし）の
/// いずれもNoTarget（exit 3）で中断する
// [covers:remove-session.zero-session-notarget]
#[test]
fn remove_session_zero_session_environment_is_notarget() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    struct Case {
        tag: &'static str,
        args: Vec<&'static str>,
        env: Option<(&'static str, &'static str)>,
        want_msg: &'static str,
    }
    let cases = [
        Case {
            tag: "zero-name",
            args: vec!["remove", "session", "foo", "--yes", "--json"],
            env: None,
            want_msg: "no zellij sessions found",
        },
        Case {
            tag: "zero-all",
            args: vec!["remove", "session", "--all", "--yes", "--json"],
            env: None,
            want_msg: "no sessions to remove",
        },
        Case {
            tag: "zero-dead",
            args: vec!["remove", "session", "--dead", "--yes", "--json"],
            env: None,
            want_msg: "no sessions to remove",
        },
        Case {
            tag: "zero-me",
            args: vec!["remove", "session", "--me", "--yes", "--json"],
            env: Some(("ZELLIJ_SESSION_NAME", "ghost")),
            want_msg: "",
        },
    ];
    for c in cases {
        let (mut cmd, log) = zelper(&format!("remove-{}", c.tag));
        let mut cmd = cmd.env("FAKE_SESSIONS_EMPTY", "1");
        if let Some((k, v)) = c.env {
            cmd = cmd.env(k, v);
        }
        let out = cmd
            .args(c.args.clone())
            .assert()
            .failure()
            .code(3)
            .get_output()
            .stdout
            .clone();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["error"]["class"], "NoTarget", "case {}", c.tag);
        if !c.want_msg.is_empty() {
            let msg = v["error"]["message"].as_str().unwrap();
            assert!(msg.contains(c.want_msg), "case {}: {msg}", c.tag);
        }
        assert!(
            !calls(&log).iter().any(|x| x.starts_with("delete-session")),
            "case {} calls: {:?}",
            c.tag,
            calls(&log)
        );
    }
}

/// remove sessionの例外dispatch（§2.7）: 対象session解決（mk_backend）を経ない
/// probe backend直接構築により、0-session環境でもremove sessionが事前確認まで
/// 到達する（共通経路のまま実装するとresolve_sessionのNoTarget文面
/// 〔no running zellij session found + add session案内〕で失敗する回帰の検出）
// [covers:remove-session.dispatch-via-probe-backend]
#[test]
fn remove_session_dispatches_via_probe_backend() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("remove-probe-dispatch");
    let out = cmd
        .env("FAKE_SESSIONS_EMPTY", "1")
        .args(["remove", "session", "foo", "--dry-run", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let msg = v["error"]["message"].as_str().unwrap();
    // run_session層の文面（事前存在確認まで到達）。共通解決経路の文面ではない
    assert!(msg.contains("no zellij sessions found"), "message: {msg}");
    assert!(
        !msg.contains("no running zellij session found"),
        "message: {msg}"
    );
    let cs = calls(&log);
    // probe backend経由でlist-sessions呼出は発生している（0-session環境で動作）。
    // 記録はshimの `echo "$*"`（= `list-sessions -n`。backend list_sessionsの
    // 確定argv・TASK-93 D1）
    assert!(cs.iter().any(|c| c == "list-sessions -n"), "calls: {cs:?}");
    assert!(
        !cs.iter().any(|c| c.starts_with("delete-session")),
        "calls: {cs:?}"
    );
}

/// 成功報告の契約（§2.9報告形式）: humanは対象のstateを反映した成功行
/// （running対象は removed session 'X'・dead対象は (was dead) 付き）で報告し、
/// JSONはper-targetのresults[]（target=name・ok=true）
// [covers:remove-session.success-report-results-and-human-lines]
#[test]
fn remove_session_success_report_results_and_human_lines() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sessions = "foo [Created 1m ago] (current)\nbar [Created 2m ago]\ndeadx [Created 3m ago] (EXITED - attach to resurrect)";
    // human: 指定順の成功行（dead対象は (was dead) 付き）
    let (mut cmd, log) = zelper("remove-report-human");
    let out = cmd
        .env("FAKE_SESSIONS", sessions)
        .args(["remove", "session", "foo", "bar", "deadx", "--yes"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();
    assert_eq!(
        stdout,
        "removed session 'foo'\nremoved session 'bar'\nremoved session 'deadx' (was dead)\n",
        "stdout: {stdout:?}"
    );
    let dels: Vec<String> = calls(&log)
        .into_iter()
        .filter(|c| c.starts_with("delete-session"))
        .collect();
    assert_eq!(dels.len(), 3, "calls: {dels:?}");

    // JSON: data.results[]（target/ok）
    let (mut cmd, _) = zelper("remove-report-json");
    let out = cmd
        .env("FAKE_SESSIONS", sessions)
        .args([
            "remove", "session", "foo", "bar", "deadx", "--yes", "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    let results = v["data"]["results"].as_array().unwrap();
    let names: Vec<&str> = results
        .iter()
        .map(|r| r["target"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["foo", "bar", "deadx"], "results: {results:?}");
    assert!(
        results.iter().all(|r| r["ok"] == true),
        "results: {results:?}"
    );
}

// ---------------------------------------------------------------------------
// rename session（TASK-95・rename.tomlのL3層7条件。設計
// tmp/tasks/tm-95/design_rename-session.md §2/§2.10。shimのFAKE_SESSIONS
// 〔EXITED行混在可〕・FAKE_SESSIONS_EMPTY〔0-session環境〕・${FAKE_LOG}.renamed
// 〔rename-session作用のlist-sessions応答反映〕。FAKE_FAIL_RENAME_SESSION /
// FAKE_MUTE_RENAME_SESSIONは機構のみ維持——呼出失敗・黙失敗系の検証はL2の
// fail_op/mute注入条件が担う〔設計§2.10行使分担〕。L2層6条件は
// tests/fake_backend/rename_add_remove.rs）

/// rename session成功経路のargvと報告（§2.0/2.6/2.7）: 実行は
/// `--session <from> action rename-session <to>` のaction形式1回（top-level
/// rename-sessionはzellij 0.44.3に存在しない・M1/M3）。postcondition
/// （list-sessions再取得で旧名消失+新名running出現）通過後にhuman 1行 /
/// JSON envelope（renamed_session=旧名・name=新名）で報告
// [covers:rename.session-success-argv-and-report]
#[test]
fn rename_session_success_argv_and_report() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sessions = "old [Created 1m ago] (current)";
    // human: 報告1行（quote付き・user指定文面）
    let (mut cmd, log) = zelper("rename-sess-report-human");
    let out = cmd
        .env("FAKE_SESSIONS", sessions)
        .args(["rename", "session", "old", "new"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();
    assert_eq!(
        stdout, "renamed session 'old' -> 'new'\n",
        "stdout: {stdout:?}"
    );
    let cs = calls(&log);
    // argv契約: action形式ちょうど1回
    assert_eq!(
        cs.iter()
            .filter(|c| **c == "--session old action rename-session new")
            .count(),
        1,
        "calls: {cs:?}"
    );
    // top-level rename-session argvは使わない
    assert!(
        !cs.iter().any(|c| c.starts_with("rename-session")),
        "calls: {cs:?}"
    );

    // JSON: data.renamed_session=旧名・data.name=新名
    let (mut cmd, _) = zelper("rename-sess-report-json");
    let out = cmd
        .env("FAKE_SESSIONS", sessions)
        .args(["rename", "session", "old", "new", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["renamed_session"], "old");
    assert_eq!(v["data"]["name"], "new");
}

/// TASK-98 §2.1-3/§2.3-2: 旧NAMEに空白含み完全名を指定したrenameは完全名で対象
/// 解決される（`--session a b action rename-session new`。a bは単一argv要素。
/// 切り詰め名parseだとNAME="a b"が世界modelに載らずNoTargetになる）
// [covers:rename.session-whitespace-name-as-source]
#[test]
fn rename_session_whitespace_name_as_source() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("rename-sess-ws-source");
    let out = cmd
        .env("FAKE_SESSIONS", "a b [Created 1m ago]")
        .args(["rename", "session", "a b", "new", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["renamed_session"], "a b");
    assert_eq!(v["data"]["name"], "new");
    let cs = calls(&log);
    // 完全名での対象解決: action形式ちょうど1回（top-level形式は不使用）
    assert_eq!(
        cs.iter()
            .filter(|c| **c == "--session a b action rename-session new")
            .count(),
        1,
        "calls: {cs:?}"
    );
    assert!(
        !cs.iter().any(|c| c.starts_with("rename-session")),
        "calls: {cs:?}"
    );

    // marker含有名共存（実装レビューmajor）: x と x [Created 5m ago]
    // （true name "x [Created 5m ago]"）が並存する環境で x を改名しても
    // marker含有行は書き換わらない（shim改名反映の完全名一致——接頭一致だと
    // 先頭field比較が両行に一致し後者も書き換わる）
    let (dir, log) = setup_fake_zellij("rename-sess-marker-coexist");
    let sessions = "x [Created 1m ago]\nx [Created 5m ago] [Created 2m ago]";
    command_on(&dir, &log)
        .env("FAKE_SESSIONS", sessions)
        .args(["rename", "session", "x", "new", "--json"])
        .assert()
        .success();
    let cs = calls(&log);
    assert_eq!(
        cs.iter()
            .filter(|c| **c == "--session x action rename-session new")
            .count(),
        1,
        "calls: {cs:?}"
    );
    let out = command_on(&dir, &log)
        .env("FAKE_SESSIONS", sessions)
        .args(["list", "sessions", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let names: Vec<&str> = v["data"]["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["new", "x [Created 5m ago]"],
        "x改名がmarker含有名行まで書き換えた: {names:?}"
    );
}

/// 旧NAME不在の分類（§2.1）: NAMEがlist_sessionsの行（RUNNING・EXITEDとも）に
/// 存在しない場合はNoTarget（exit 3）でcandidates=全行名（EXITED込み）。一覧が
/// 完全0件（0-session環境）の場合は「no zellij sessions found」文面。いずれも
/// rename呼出不発（fail-fast）
// [covers:rename.session-missing-name-notarget-with-candidates]
#[test]
fn rename_session_missing_name_is_notarget_with_candidates() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (i) 不在NAME: candidatesに稼働名とEXITED名の両方（EXITED込み）
    let (mut cmd, log) = zelper("rename-sess-missing");
    let out = cmd
        .env(
            "FAKE_SESSIONS",
            "old [Created 1m ago] (current)\n\
             deadx [Created 2m ago] (EXITED - attach to resurrect)",
        )
        .args(["rename", "session", "ghost", "new", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let msg = v["error"]["message"].as_str().unwrap();
    // 単一対象のため単数形のsession不在文面（removeの複数名前提文面と異なる点）。
    // 対象名の欠落を見逃さないよう完全一致で検証
    assert_eq!(msg, "session 'ghost' not found");
    // 共通解離経路の文面ではない
    assert!(
        !msg.contains("no running zellij session found"),
        "message: {msg}"
    );
    // candidates=全行のname（EXITED込み・行順）の完全一致
    assert_eq!(
        v["error"]["candidates"],
        serde_json::json!(["old", "deadx"])
    );
    assert!(
        !calls(&log).iter().any(|c| c.contains("rename-session")),
        "calls: {:?}",
        calls(&log)
    );

    // (ii) 行0件（0-session環境）: no zellij sessions found文面
    let (mut cmd, _) = zelper("rename-sess-missing-zero");
    let out = cmd
        .env("FAKE_SESSIONS_EMPTY", "1")
        .args(["rename", "session", "foo", "bar", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let msg = v["error"]["message"].as_str().unwrap();
    assert_eq!(msg, "no zellij sessions found");
}

/// NEW_NAME衝突の事前fail-fast（§2.2）: NEW_NAMEが既存行（running・EXITEDの
/// 両方）と同名の場合はPreflight（exit 7）で状態変更前に拒否。文面はadd session
/// と同一形式（zellijは既存名へのrenameをrc=0・両stream空の黙って失敗として
/// 受け入れる〔M4 I実測〕ため、wrap側の事前検査が唯一の防衛）
// [covers:rename.session-new-name-collision-preflight]
#[test]
fn rename_session_new_name_collision_is_preflight() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let sessions = "old [Created 1m ago] (current)\n\
                    taken [Created 2m ago]\n\
                    deadname [Created 3m ago] (EXITED - attach to resurrect)";
    for (i, new_name) in ["taken", "deadname"].iter().enumerate() {
        let (mut cmd, log) = zelper(&format!("rename-sess-collision-{i}"));
        let out = cmd
            .env("FAKE_SESSIONS", sessions)
            .args(["rename", "session", "old", new_name, "--json"])
            .assert()
            .failure()
            .code(7)
            .get_output()
            .stdout
            .clone();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(v["error"]["class"], "Preflight", "case {i}");
        let msg = v["error"]["message"].as_str().unwrap();
        // 対象名の欠落を見逃さないよう完全一致で検証
        assert_eq!(msg, format!("session already exists: '{new_name}'"));
        assert!(
            !calls(&log).iter().any(|c| c.contains("rename-session")),
            "case {i} calls: {:?}",
            calls(&log)
        );
    }
}

/// 同一名renameのUsage事前拒否（§2.2）: NAME == NEW_NAMEは一覧取得より前に
/// Usage（exit 2）で拒否する（無駄なbackend呼び出しをしない。放置すると
/// zellijのno-op rc=0成功〔M95-B実測〕がpostconditionでVerificationFailedに
/// 誤分類される唯一の系）
// [covers:rename.session-same-name-usage-error]
#[test]
fn rename_session_same_name_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("rename-sess-same-name");
    let out = cmd
        .env("FAKE_SESSIONS", "old [Created 1m ago] (current)")
        .args(["rename", "session", "old", "old", "--json"])
        .assert()
        .failure()
        .code(2)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "Usage");
    let msg = v["error"]["message"].as_str().unwrap();
    assert_eq!(msg, "NEW_NAME must differ from NAME");
    // 一覧取得より前のfail-fast（list-sessions・rename-sessionとも不発。
    // check_capabilityの--version呼び出しはdispatch契約上発生する）
    let cs = calls(&log);
    assert!(!cs.iter().any(|c| c == "list-sessions -n"), "calls: {cs:?}");
    assert!(
        !cs.iter().any(|c| c.contains("rename-session")),
        "calls: {cs:?}"
    );
}

/// NEW_NAME validationの最小set（§2.3 + TASK-97 §2.1/§2.2）: 空文字・'/'含み・
/// 空白含み・先頭`-`（`--`経由）の4種NEW_NAMEはUsage（exit 2）で一覧取得より
/// 前に拒否する（空文字は黙失敗〔M95-C1〕・'/'含みはcacheのsession_info
/// directoryがnested pathへ部分移動する状態破壊〔M95-C2〕・空白含みはzellij受理後も
/// list_sessions行から復元不能〔parse_sessionsが先頭wordのみ名前化〕のため。
/// 先頭`-`はTASK-97追加——`--`option終端経由の通過系列が`--`挿入なしの
/// rename-session <to>組み立てへ到達するため入力段の確定拒否〔add session NAME
/// 検査〔TASK-96〕と対称〕。それ以外の文字種は事前拒否しない）。先頭`-`は`--`
/// （option終端）経由でCLI解析を通過させる——`--`なしはzeller側clapがexit 2で
/// 拒否するが文面がclap形式（envelope不整合）のため本契約の対象外。4案目のJSON
/// 検証は`--json`を`--`より前に置く（`--`以降の`--json`はpositional化するため）
// [covers:rename.session-invalid-new-name-usage-error]
#[test]
fn rename_session_invalid_new_name_is_usage_error() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // (base=`rename session old` + この引数列, 期待文面)。既存3案はNEW_NAME値に
    // `-`がなく`--`不要のため元構成（NEW_NAMEの後ろに--json）のまま
    let cases: [(&[&str], &str); 4] = [
        (&["", "--json"], "NEW_NAME must not be empty"),
        (&["a/b", "--json"], "NEW_NAME must not contain '/'"),
        (&["a b", "--json"], "NEW_NAME must not contain whitespace"),
        (&["--json", "--", "-x"], "NEW_NAME must not start with '-'"),
    ];
    for (i, (case_args, expect_msg)) in cases.iter().enumerate() {
        let (mut cmd, log) = zelper(&format!("rename-sess-invalid-{i}"));
        let mut args: Vec<&str> = vec!["rename", "session", "old"];
        args.extend_from_slice(case_args);
        let out = cmd
            .env("FAKE_SESSIONS", "old [Created 1m ago] (current)")
            .args(&args)
            .assert()
            .failure()
            .code(2)
            .get_output()
            .stdout
            .clone();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(
            v["error"]["class"], "Usage",
            "case {i} (args={case_args:?})"
        );
        // 文面は完全一致で検証
        let msg = v["error"]["message"].as_str().unwrap();
        assert_eq!(msg, *expect_msg, "case {i} (args={case_args:?})");
        let cs = calls(&log);
        assert!(
            !cs.iter().any(|c| c == "list-sessions -n"),
            "case {i} calls: {cs:?}"
        );
        assert!(
            !cs.iter().any(|c| c.contains("rename-session")),
            "case {i} calls: {cs:?}"
        );
    }
}

/// EXITED sessionのrename不可（§2.4）: NAMEが事前一覧のEXITED行に一致する場合は
/// Preflight（exit 7）で状態変更前に拒否しresurrection（attach）への案内文面を
/// 出す（server不在名へのaction系呼び出しは失敗のみ〔M95-A/D実測〕のため。
/// 存在確認〔行にあるか=NoTarget境界〕と実行可否判定〔runningか=Preflight境界〕
/// は別個の検査であることの固定）
// [covers:rename.session-exited-name-preflight-refused]
#[test]
fn rename_session_exited_name_is_refused() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("rename-sess-exited");
    let out = cmd
        .env(
            "FAKE_SESSIONS",
            "old [Created 1m ago] (current)\n\
             deadx [Created 2m ago] (EXITED - attach to resurrect)",
        )
        .args(["rename", "session", "deadx", "new", "--json"])
        .assert()
        .failure()
        .code(7)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "Preflight");
    let msg = v["error"]["message"].as_str().unwrap();
    // §2.4指定文面の完全一致（resurrection案内を含む）
    assert_eq!(
        msg,
        "cannot rename an EXITED (dead) session 'deadx' (attach to resurrect it first)"
    );
    assert!(
        !calls(&log).iter().any(|c| c.contains("rename-session")),
        "calls: {:?}",
        calls(&log)
    );
}

/// rename sessionの例外dispatch（§2.7）: 対象session解決（mk_backend）を経ない
/// probe backend直接構築により、zellij sessionが1つも存在しない環境でも
/// rename sessionが事前確認まで到達する（共通経路のまま実装すると
/// resolve_sessionのNoTarget〔共通解離文面〕が先に立ち本機能自身の事前確認
/// 文面に到達しない回帰の検出）
// [covers:rename.session-dispatch-via-probe-backend]
#[test]
fn rename_session_dispatches_via_probe_backend() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, log) = zelper("rename-sess-probe-dispatch");
    let out = cmd
        .env("FAKE_SESSIONS_EMPTY", "1")
        .args(["rename", "session", "foo", "bar", "--json"])
        .assert()
        .failure()
        .code(3)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["error"]["class"], "NoTarget");
    let msg = v["error"]["message"].as_str().unwrap();
    // run_session層の文面（事前存在確認まで到達）。共通解決経路の文面ではない
    assert!(msg.contains("no zellij sessions found"), "message: {msg}");
    assert!(
        !msg.contains("no running zellij session found"),
        "message: {msg}"
    );
    let cs = calls(&log);
    // probe backend経由でlist-sessions呼出は発生している（0-session環境で動作）。
    // 記録はshimの `echo "$*"`（= `list-sessions -n`。backend list_sessionsの
    // 確定argv・TASK-93 D1）
    assert!(cs.iter().any(|c| c == "list-sessions -n"), "calls: {cs:?}");
    assert!(
        !cs.iter().any(|c| c.contains("rename-session")),
        "calls: {cs:?}"
    );
}

/// rename tab成功経路のTABSPEC解決とargv・報告（DD-8/DD-1.3。TASK-66: excluded
/// 転換で条件化）: TABSPEC一意名はresolve_tabでTabIdへ解決され（全verb共通解決の
/// verb経由確認）、実行は `--session fake-sess action rename-tab-by-id <id> <name>`
/// 1回。postcondition（list-tabs再取得のname反映）通過後にhuman 1行 / JSON
/// envelope（renamed_tab=id・name=新名）。cli-grammar.rename-parse-emits-typed-argv
/// （ID指定・human報告・argv形式）と分担: 本条件は一意名解決込み系列とJSON
/// envelopeを担う
// [covers:rename.tab-success-argv-and-report]
#[test]
fn rename_tab_success_argv_and_report() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tabs = format!("[{}]", tab_json(0, "mytab", 2, 0));
    // human: TABSPEC一意名 mytab → tab_id 0へ解決。報告1行
    let (mut cmd, log) = zelper("rename-tab-report-human");
    let out = cmd
        .env("FAKE_TABS_JSON", &tabs)
        .args(["rename", "tab", "mytab", "newtab"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();
    assert_eq!(stdout, "renamed tab 0 -> newtab\n", "stdout: {stdout:?}");
    let cs = calls(&log);
    assert_eq!(
        cs.iter()
            .filter(|c| **c == "--session fake-sess action rename-tab-by-id 0 newtab")
            .count(),
        1,
        "calls: {cs:?}"
    );

    // JSON: data.renamed_tab=id（数値）・data.name=新名
    let (mut cmd, _) = zelper("rename-tab-report-json");
    let out = cmd
        .env("FAKE_TABS_JSON", &tabs)
        .args(["rename", "tab", "mytab", "newtab", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["renamed_tab"], 0);
    assert_eq!(v["data"]["name"], "newtab");
}

// TASK-69 minor-3: remap --tab TABSPECのparse受理・mainからのRemapArgs.tab引き渡し
// （remap-sequence.tomlのtab-spec系3条件はRemapArgs.tab直接代入のためこの段が
// 未検証）。--tab build（一意名）指定のdry-run JSON planにtab絞り込みが反映される:
// anchor=対象tab（active tab 0でない）・source=対象tabのpaneのみ
// [covers:cli-grammar.remap-tab-spec-parse-scopes-plan]
#[test]
fn remap_tab_spec_parse_scopes_plan() {
    let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (mut cmd, _) = zelper("remap-tabspec");
    cmd.env(
        "FAKE_PANES_JSON",
        format!(
            "[{},{},{},{},{}]",
            pane_json(1, "a", 0, 50, 0),
            pane_json(2, "b", 50, 50, 0),
            pane_json(3, "c", 0, 30, 1),
            pane_json(4, "d", 30, 30, 1),
            pane_json(5, "e", 60, 30, 1)
        ),
    )
    .env(
        "FAKE_TABS_JSON",
        format!(
            "[{},{}]",
            tab_json(0, "Tab #1", 2, 0),
            tab_json(1, "build", 3, 1)
        ),
    )
    .args([
        "remap",
        "--inline",
        "layout {\n    tab {\n        pane\n        pane\n        pane\n    }\n}\n",
        "--tab",
        "build",
        "--dry-run",
        "--json",
    ]);
    let out = cmd.assert().success().get_output().stdout.clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], true);
    let data = &v["data"];
    assert_eq!(
        data["anchor"], 1,
        "anchorは--tab対象tab（active tab 0でない）: {data}"
    );
    assert_eq!(data["m"], 3, "raw: {data}");
    assert_eq!(data["k"], 1, "raw: {data}");
    let source = data["source"].as_array().cloned().unwrap_or_default();
    let ids: Vec<&str> = source
        .iter()
        .map(|p| p["pane"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        ids,
        vec!["terminal_3", "terminal_4", "terminal_5"],
        "sourceは対象tabのpaneのみ（tab 0のpane不載）: {data}"
    );
}
