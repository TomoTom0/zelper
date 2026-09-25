// L2: attach / detach（fake backend。attach-detach.tomlのL2層2条件。
// TASK-89・設計§5.2: TTY検査はrun_attach_innerのtty_ok引数注入）
// run_interactive経路（stdio inherit・timeout無効・非zero終了mapping）は
// src/zellij/process.rs内test・L3（実binary・shim）系はtests/cli/list_read_send.rs
mod fake;
use fake::FakeBackend;
use zelper::app::attach::{attach_report, detach_report, run_attach_inner, run_detach};
use zelper::domain::*;

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

/// attach成功経路の報告契約: attach_session()の復帰（対話client終了）後にのみ
/// 出力を行い、対話中は一切出力しない（構造: attach_session復帰後にのみ報告。
/// stdoutはzellijが占有）。humanは最小サマリ1行・JSONはenvelope 1通
/// （data.session=NAME）。tty_ok=true注入
// [covers:attach-detach.attach-success]
#[test]
fn attach_success_reports_after_client_end() {
    let b = FakeBackend::new(vec![pane(1, "a")], vec![]);
    // human: `attach session '<NAME>' ended` の1行のみ
    run_attach_inner(&b, "work", false, true).unwrap();
    assert_eq!(attach_report("work", false), "attach session 'work' ended");
    // JSON: envelope 1通でdata.session=NAME
    let env = attach_report("work", true);
    let v: serde_json::Value = serde_json::from_str(&env).unwrap();
    assert_eq!(v["schema_version"], 1);
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["session"], "work");
    // backend呼出はattach 1回（対話clientの起動）
    assert!(b.calls().iter().any(|c| c == "attach"), "calls: {:?}", b.calls());
}

/// detachのno-op成功契約: zellijのaction detachはclient不在でもrc=0 silent
/// （M3 (b)実測）のため、zellerはclient数を検査せずrc=0（Ok）を無条件で成功
/// 扱いする。単一client切断（M3 (a): client exit 0・session RUNNING継続）と
/// client不在はzellerから同一に見え、human 1行サマリ/JSON envelopeで報告する
// [covers:attach-detach.detach-no-client-noop-success]
#[test]
fn detach_no_client_is_noop_success() {
    let b = FakeBackend::new(vec![pane(1, "a")], vec![]);
    // FakeBackendのdetach_sessionは常にOk（client不在とclient存在が区別されない
    // ことをそのまま模擬——zeller側にclient数依存の分岐がないことの検証）
    run_detach(&b, "work", false).unwrap();
    run_detach(&b, "work", true).unwrap();
    assert!(
        b.calls().iter().any(|c| c == "detach"),
        "calls: {:?}",
        b.calls()
    );
    // human: `detached clients from session '<NAME>'` の1行
    assert_eq!(
        detach_report("work", false),
        "detached clients from session 'work'"
    );
    // JSON: envelope 1通でdata.session=NAME
    let env = detach_report("work", true);
    let v: serde_json::Value = serde_json::from_str(&env).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["session"], "work");
}
