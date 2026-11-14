// L2: remap v2実行sequence matrix（test-plan §2.7 R8・(d) R29〜R32・(e) R33〜R36・
// (f) R37〜R42・(i) R47/R50・(j) 失敗注入。DD-10 v2）。
// planner（R1〜R12）と生成KDL（R13〜R20）は L1（tests/unit/）、permissions seed
// （R21〜R28）は tests/fake_backend/companion_seed.rs、deprecated flag・version gate
// ・dry-run出力契約（R43〜R46・R48/R49/R50出力側）は L3（tests/cli/remap.rs）。
// fail-first: v2実装（TASK-39）前にfailする（test-plan §5）。旧仕様v1テストは
// 本matrixへの置換により削除済み（test-plan §2.7冒頭の経過措置）。
mod fake;
use fake::FakeBackend;
use std::path::Path;
use std::sync::Mutex;
use zelper::app::remap::{RemapArgs, run};
use zelper::domain::*;
use zelper::error::ErrorClass;

const THREE_SLOT: &str = "layout {\n    tab {\n        pane size=\"40%\"\n        pane size=\"30%\"\n        pane size=\"30%\"\n    }\n}\n";
/// 2-slot layout（TASK-81再E2E R1の曖昧構成〔同一cwd shell slot 2つ〕用）
const TWO_SLOT: &str =
    "layout {\n    tab {\n        pane size=\"50%\"\n        pane size=\"50%\"\n    }\n}\n";
const ANCHOR: TabId = TabId(0);

fn pane_at(
    id: u32,
    title: &str,
    tab_id: u32,
    tab_position: u32,
    y: u32,
    x: u32,
    cmd: Option<&str>,
) -> PaneState {
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
        command: cmd.map(|c| c.to_string()),
        terminal_command: None,
        cwd: Some("/w".into()),
        tab_id: TabId(tab_id),
        tab_position,
        tab_name: format!("t{tab_id}"),
        plugin_url: None,
    }
}

fn tab_state(id: u32, position: u32, name: &str, active: bool, tiled: u32) -> TabState {
    TabState {
        id: TabId(id),
        position,
        name: name.to_string(),
        active,
        selectable_tiled_panes_count: tiled,
        selectable_floating_panes_count: 0,
        are_floating_panes_visible: true,
    }
}

/// 3 pane同一tab（k=1・移動不要の最小構成。R2相当）
fn same_tab_three() -> (Vec<PaneState>, Vec<TabState>) {
    (
        vec![
            pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
            pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
            pane_at(3, "c", 0, 0, 10, 0, Some("cmd3")),
        ],
        vec![tab_state(0, 0, "t0", true, 3)],
    )
}

/// 4 pane / 2 tab（k=2・group1は1 pane。worked example #3相当）
fn four_panes_two_tabs() -> (Vec<PaneState>, Vec<TabState>) {
    (
        vec![
            pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
            pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
            pane_at(3, "c", 0, 0, 10, 0, Some("cmd3")),
            pane_at(4, "d", 1, 1, 0, 0, Some("cmd4")),
        ],
        vec![
            tab_state(0, 0, "t0", true, 3),
            tab_state(1, 1, "t1", false, 1),
        ],
    )
}

/// 4 pane同一tab（anchor単独でM>N。k=2・移動はgroup 1の新規tabのみ）
fn one_tab_four() -> (Vec<PaneState>, Vec<TabState>) {
    (
        vec![
            pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
            pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
            pane_at(3, "c", 0, 0, 10, 0, Some("cmd3")),
            pane_at(4, "d", 0, 0, 10, 50, Some("cmd4")),
        ],
        vec![tab_state(0, 0, "t0", true, 4)],
    )
}

/// 5 pane / 3 tab分散（k=2・group0はtab跨ぎ。worked example #6相当）
fn five_panes_three_tabs() -> (Vec<PaneState>, Vec<TabState>) {
    (
        vec![
            pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
            pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
            pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
            pane_at(4, "d", 1, 1, 0, 50, Some("cmd4")),
            pane_at(5, "e", 2, 2, 0, 0, Some("cmd5")),
        ],
        vec![
            tab_state(0, 0, "t0", true, 2),
            tab_state(1, 1, "t1", false, 2),
            tab_state(2, 2, "t2", false, 1),
        ],
    )
}

/// 7 pane / 3 tab（k=3・最終instanceは1 pane + 2空slot。worked example #5相当）
fn seven_panes_three_tabs() -> (Vec<PaneState>, Vec<TabState>) {
    (
        vec![
            pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
            pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
            pane_at(3, "c", 0, 0, 10, 0, Some("cmd3")),
            pane_at(4, "d", 1, 1, 0, 0, Some("cmd4")),
            pane_at(5, "e", 1, 1, 0, 50, Some("cmd5")),
            pane_at(6, "f", 1, 1, 10, 0, Some("cmd6")),
            pane_at(7, "g", 2, 2, 0, 0, Some("cmd7")),
        ],
        vec![
            tab_state(0, 0, "t0", true, 3),
            tab_state(1, 1, "t1", false, 3),
            tab_state(2, 2, "t2", false, 1),
        ],
    )
}

fn args(dry_run: bool) -> RemapArgs<'static> {
    RemapArgs {
        layout: None,
        path: None,
        inline: Some(THREE_SLOT),
        tab: None,
        embed_floating: false,
        dry_run,
        json: false,
    }
}

fn tiled_count(b: &FakeBackend, tab: TabId) -> usize {
    b.state
        .borrow()
        .panes
        .iter()
        .filter(|p| p.tab_id == tab && p.is_remap_source())
        .count()
}

/// pipe呼び出しの記録（name/payload抽出）
fn pipes(b: &FakeBackend) -> Vec<String> {
    b.calls()
        .iter()
        .filter(|c| c.starts_with("pipe "))
        .cloned()
        .collect()
}

// ---- XDG_CACHE_HOME隔離（test-plan §2.8のL2適用） ----

/// companion setupがcache配下（wasm extract・permissions.kdl）へ書き込む処理を
/// テスト実envから分離する。static Mutexで直列化し、抜ける時に元に戻す
static XDG_LOCK: Mutex<()> = Mutex::new(());

fn with_isolated_xdg<T>(f: impl FnOnce(&Path) -> T) -> T {
    // fail-first期のfail testがpanicでlockを毒化しても、XDG_CACHE_HOMEは入口で
    // 毎回上書きされるため毒化回復しても後続testへ影響しない（直列化のみ維持し、
    // 他testを巻き込まない）
    let _guard = XDG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = std::env::temp_dir().join(format!("zelper-remap-xdg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let prev = std::env::var("XDG_CACHE_HOME").ok();
    // safety: XDG_LOCKで直列化した範囲内でのみ一時的に上書きし、即座に復元する
    unsafe { std::env::set_var("XDG_CACHE_HOME", &dir) };
    let out = f(&dir);
    match prev {
        Some(p) => unsafe { std::env::set_var("XDG_CACHE_HOME", p) },
        None => unsafe { std::env::remove_var("XDG_CACHE_HOME") },
    }
    out
}

// ---- R8: floating pane ----

// [covers:remap-sequence.r8-floating-pane-in-scope-blocks-or-embeds]
#[test]
fn r8_floating_pane_in_scope_blocks_or_embeds() {
    // scopeはsession全体（v2）。対象tab（anchor）外のtabにいるfloating paneも
    // preflight errorになる
    let (mut panes, tabs) = four_panes_two_tabs();
    let mut floating = pane_at(9, "float", 1, 1, 0, 0, Some("htop"));
    floating.is_floating = true;
    panes.push(floating);

    let b = FakeBackend::new(panes.clone(), tabs.clone());
    let err = run(&b, &args(false)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
    assert!(err.candidates().contains(&"terminal_9".to_string()));
    // 状態変更前（何も起きていない）
    assert!(pipes(&b).is_empty());

    // --embed-floatingでtoggle（plan確定後の状態変更第1歩）を経てsourceに組入れ。
    // 4 tiled + 1 floating(embed予定) = 5 pane -> k=2として計画・実行される
    let b2 = FakeBackend::new(panes, tabs);
    let mut a = args(false);
    a.embed_floating = true;
    with_isolated_xdg(|_| {
        run(&b2, &a).unwrap();
    });
    let calls = b2.calls();
    assert!(
        calls.iter().any(|c| c.contains("toggle-embed terminal_9")),
        "raw: {calls:?}"
    );
    // toggle対象はpreflightで特定したselectable floating terminalのみ。
    // probe自動launchのcompanion plugin pane（non-selectable）はtoggleしない
    // （DD-10.3/10.5。C1回帰）
    assert!(
        !calls.iter().any(|c| c.contains("toggle-embed plugin_")),
        "plugin paneはtoggleしない: {calls:?}"
    );
    assert!(
        !b2.state
            .borrow()
            .panes
            .iter()
            .any(|p| p.is_floating && p.is_selectable),
        "selectable floating paneは残らない"
    );
    // companion plugin paneはfloatingのまま残る（sourceに混入しない）
    assert!(
        b2.state
            .borrow()
            .panes
            .iter()
            .any(|p| p.is_floating && !p.is_selectable),
        "plugin paneはfloatingのまま: {:?}",
        b2.state.borrow().panes
    );
    // floating paneはgroup 1（後続instance）に組入れられ、元の全5 paneが配置される。
    // 空slotは既定shellで埋まるため（R40）selectable tiled paneの総数は3*kとなり、
    // 元paneの配置をもって検証する
    let placed: Vec<PaneKindId> = b2
        .state
        .borrow()
        .panes
        .iter()
        .filter(|p| p.is_remap_source())
        .map(|p| p.id)
        .collect();
    for id in [1u32, 2, 3, 4, 9] {
        assert!(
            placed.contains(&PaneKindId::Terminal(id)),
            "raw: {placed:?}"
        );
    }
}

// ---- (d) probe pipe R29〜R32 ----

// [covers:remap-sequence.r29-probe-only-when-move-needed]
#[test]
fn r29_probe_only_when_move_needed() {
    // 移動が不要（k=1・group 0にanchor外paneなし）ならprobeもpluginも不使用
    let (panes, tabs) = same_tab_three();
    let b = FakeBackend::new(panes, tabs);
    run(&b, &args(false)).unwrap();
    assert!(pipes(&b).is_empty(), "raw: {:?}", b.calls());
    // companion setup（permissions.kdl書換）も行わない
    with_isolated_xdg(|xdg| {
        let (panes, tabs) = same_tab_three();
        let b = FakeBackend::new(panes, tabs);
        run(&b, &args(false)).unwrap();
        assert!(
            !xdg.join("zellij").join("permissions.kdl").exists(),
            "移動不要ならpermissions seedも書かない"
        );
    });

    // 移動が必要（k=2）ならprobe pipeが実行される
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).unwrap();
    });
    assert!(
        pipes(&b).iter().any(|c| c.starts_with("pipe probe ")),
        "raw: {:?}",
        b.calls()
    );
}

// [covers:remap-sequence.r30-probe-polled-then-state-change-proceeds]
#[test]
fn r30_probe_polled_then_state_change_proceeds() {
    // probe <nonce>送信後、list-panesで zelper-probe-<nonce> titleの出現を確認
    // してから状態変更（break pipe）へ進む
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).unwrap();
    });
    let calls = b.calls();
    let probe = calls.iter().position(|c| c.starts_with("pipe probe "));
    let break_at = calls.iter().position(|c| c.starts_with("pipe break-"));
    let (probe, break_at) = (probe.expect("probe pipe"), break_at.expect("break pipe"));
    assert!(probe < break_at, "probe確認がbreakより先: {calls:?}");
    // probeが実際に観測された: companion plugin paneのtitleにnonceが現れる
    let plugin_title = b
        .state
        .borrow()
        .panes
        .iter()
        .find(|p| matches!(p.id, PaneKindId::Plugin(_)))
        .map(|p| p.title.clone())
        .expect("companion plugin pane launched");
    assert!(
        plugin_title.starts_with("zelper-probe-"),
        "raw: {plugin_title}"
    );
}

// [covers:remap-sequence.r31-probe-timeout-aborts-before-any-state-change]
#[test]
fn r31_probe_timeout_aborts_before_any_state_change() {
    // probe不成立（timeout）-> 一切の状態変更前に中断。OperationFailed + 権限hint
    // （seed未反映・dialog pending・protocol非互換をここで検知。DD-10.4/10.7 step 3）
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    b.inject_failure("pipe"); // 最初のpipe（= probe）がbackend timeoutする
    let err = with_isolated_xdg(|_| run(&b, &args(false))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
    assert!(
        err.message().to_lowercase().contains("permission"),
        "権限hint（seed未反映・dialog pending・手動grant案内）: {}",
        err.message()
    );
    // probeを試みた直後に中断: break pipe・toggle・override・tab切替は不発生
    let calls = b.calls();
    assert!(
        calls.iter().any(|c| c.starts_with("pipe probe")),
        "{calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.starts_with("pipe break-")),
        "{calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.contains("toggle-embed")),
        "{calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.starts_with("override-layout")),
        "{calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.starts_with("go-to-tab")),
        "{calls:?}"
    );
}

// [covers:remap-sequence.r32-probe-nonce-unique-per-run]
#[test]
fn r32_probe_nonce_unique_per_run() {
    // nonceは実行毎に一意（前回実行のtitle残留による偽陽性を構造的に排除）
    let probe_payloads = |b: &FakeBackend| -> Vec<String> {
        pipes(b)
            .iter()
            .filter(|c| c.starts_with("pipe probe "))
            .map(|c| c.trim_start_matches("pipe probe ").to_string())
            .collect()
    };
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).unwrap();
        run(&b, &args(false)).unwrap();
    });
    let payloads = probe_payloads(&b);
    assert_eq!(payloads.len(), 2, "raw: {:?}", b.calls());
    assert_ne!(payloads[0], payloads[1], "nonceは実行毎に一意");
}

// ---- (e) move sequence R33〜R36 ----

// [covers:remap-sequence.r33-break-id-moves-inbound-before-outbound]
#[test]
fn r33_break_id_moves_inbound_before_outbound() {
    // group 0のanchor外paneを break-id <anchor_id> <ids> でanchorへ移動。
    // group 0のinboundを後続groupのoutboundより先行させる（anchor一時空による
    // 自動closeを構造的に排除する不変条件。DD-10.7 5-a）
    let (panes, tabs) = five_panes_three_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).unwrap();
    });
    let calls = b.calls();
    let break_id = calls
        .iter()
        .position(|c| c.starts_with("pipe break-id "))
        .expect("break-id pipe");
    let break_new = calls
        .iter()
        .position(|c| c.starts_with("pipe break-new "))
        .expect("break-new pipe");
    assert!(break_id < break_new, "inbound先行: {calls:?}");
    // group 0のanchor外paneは pane 3のみ（pane 1,2は当初からtab 0）
    let payload = calls[break_id].trim_start_matches("pipe break-id ");
    assert!(payload.contains('0'), "anchor tab idを含む: {payload}");
    assert!(payload.contains("terminal_3"), "raw: {payload}");
    assert!(
        !payload.contains("terminal_1"),
        "anchor内paneは移動しない: {payload}"
    );
    assert!(!payload.contains("terminal_2"), "raw: {payload}");
    // 後続group（pane 4,5）はbreak-newで新規tabへ
    let outbound = calls[break_new].trim_start_matches("pipe break-new ");
    assert!(outbound.contains("terminal_4"), "raw: {outbound}");
    assert!(outbound.contains("terminal_5"), "raw: {outbound}");
}

// [covers:remap-sequence.r34-break-id-completion-group0-all-on-anchor]
#[test]
fn r34_break_id_completion_group0_all_on_anchor() {
    // break-idの完了条件 = group 0の全paneのtab_id == anchor（polling）
    let (panes, tabs) = five_panes_three_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).unwrap();
    });
    let s = b.state.borrow();
    for id in [1u32, 2, 3] {
        let p = s
            .panes
            .iter()
            .find(|p| p.id == PaneKindId::Terminal(id))
            .unwrap_or_else(|| panic!("pane {id} alive"));
        assert_eq!(p.tab_id, ANCHOR, "group 0（pane {id}）はanchor所属");
    }
}

// [covers:remap-sequence.r35-break-new-membership-then-rename-tab]
#[test]
fn r35_break_new_membership_then_rename_tab() {
    // group j>=1は break-new で新規tabへ。pane id基準のmembership一致でtarget_jを
    // 特定し、直後に rename-tab-by-id <target_j> <base>-<j+1> で命名
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).unwrap();
    });
    let calls = b.calls();
    assert!(
        calls
            .iter()
            .any(|c| c.starts_with("pipe break-new ") && c.contains("terminal_4")),
        "raw: {calls:?}"
    );
    // 新tab id = 2（初期tab 0,1の次）に "remap-2"（base=remap: --inline）
    assert!(
        calls.iter().any(|c| c == "rename-tab 2 remap-2"),
        "raw: {calls:?}"
    );
    let names: Vec<String> = b
        .state
        .borrow()
        .tabs
        .iter()
        .map(|t| t.name.clone())
        .collect();
    assert!(names.contains(&"remap-2".to_string()), "raw: {names:?}");
}

// [covers:remap-sequence.r36-polling-timeout-reports-groups-and-latency-note]
#[test]
fn r36_polling_timeout_reports_groups_and_latency_note() {
    // pipe送信後に効果が現れない（polling timeout）-> OperationFailed + 権限hint +
    // 「pipeはCLI失敗後もserver側で遅延実行されうる」注意 + 移動済み/未移動groupの報告
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    b.mute_pipe("break-new"); // probeは成立するが break-new の効果が現れない
    let err = with_isolated_xdg(|_| run(&b, &args(false))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
    let msg = err.message().to_lowercase();
    assert!(msg.contains("permission"), "権限hint: {}", err.message());
    assert!(
        msg.contains("delay") || msg.contains("late") || msg.contains("遅"),
        "遅延実行の注意: {}",
        err.message()
    );
    assert!(
        msg.contains("group"),
        "移動済み/未移動groupの報告: {}",
        err.message()
    );
    // timeoutはscreen繁忙（screen応答1s timeoutによる一時的な空list応答）でも
    // 起こりうる旨のF8機構の注記を含む（CR74-4: この注記なしでは空応答由来の
    // timeoutが権限問題と誤診されうる）
    assert!(
        msg.contains("screen")
            && (msg.contains("busy") || msg.contains("1s") || msg.contains("empty")),
        "screen繁忙の1s list応答timeout機構の注記: {}",
        err.message()
    );
    // group 0は影響を受けずanchorにいる（移動済みgroupの状態は保持）
    let s = b.state.borrow();
    let p4 = s
        .panes
        .iter()
        .find(|p| p.id == PaneKindId::Terminal(4))
        .unwrap();
    assert_eq!(p4.tab_id, TabId(1), "未移動のpane 4は元のtabに留まる");
    // layout適用phaseには入らない
    assert!(!b.calls().iter().any(|c| c.starts_with("override-layout")));
}

// ---- (f) postcondition R37〜R42 ----

// [covers:remap-sequence.r37-all-source-panes-survive-on-assigned-tabs]
#[test]
fn r37_all_source_panes_survive_on_assigned_tabs() {
    // 全source pane idが生存し、割当instanceのtabに所属（pane id基準）
    let (panes, tabs) = five_panes_three_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).unwrap();
    });
    let s = b.state.borrow();
    let alive: Vec<PaneKindId> = s
        .panes
        .iter()
        .filter(|p| p.is_remap_source())
        .map(|p| p.id)
        .collect();
    for id in 1u32..=5 {
        assert!(
            alive.contains(&PaneKindId::Terminal(id)),
            "pane {id} 生存: {alive:?}"
        );
    }
    // group 0（1,2,3）はanchor・group 1（4,5）は同一新tab
    let tab_of = |id: u32| {
        s.panes
            .iter()
            .find(|p| p.id == PaneKindId::Terminal(id))
            .unwrap()
            .tab_id
    };
    assert_eq!(tab_of(1), ANCHOR);
    assert_eq!(tab_of(2), ANCHOR);
    assert_eq!(tab_of(3), ANCHOR);
    assert_eq!(tab_of(4), tab_of(5));
    assert_ne!(tab_of(4), ANCHOR);
    // 空になったsource tab（tab 1, 2）は自動close
    let tab_ids: Vec<TabId> = s.tabs.iter().map(|t| t.id).collect();
    assert!(!tab_ids.contains(&TabId(1)), "raw: {tab_ids:?}");
    assert!(!tab_ids.contains(&TabId(2)), "raw: {tab_ids:?}");
}

// [covers:remap-sequence.r38-instance-tab-pane-count-mismatch-fails-verification]
#[test]
fn r38_instance_tab_pane_count_mismatch_fails_verification() {
    // 各instance tabのselectable tiled terminal pane数 == N（command照合missによる
    // 重複spawn・未照合paneの入れ子残留を検出）。不一致はVerificationFailed
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    b.spawn_extra_on_override(ANCHOR); // group 0適用後に重複paneが1つ湧く
    let err = with_isolated_xdg(|_| run(&b, &args(false))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert!(
        err.message().contains("expected"),
        "期待/実測の差分を報告: {}",
        err.message()
    );
    // TASK-81（DD-10.13文面規則）: (b)系検証失敗のerror文面は「cwd差異（chdir・
    // 正規化・bind mount）とhold flag差（-c/-s起因）の可能性によりrun一致が外れた」
    // 旨を案内する（cwd起因の照合阻害は10.8 cwd注入で解消済みのため、残る要因の案内。
    // CLI表面にinvoked_withのcwd/holdを区別するdataが存在しないためpreflight検出不可）
    let msg = err.message().to_lowercase();
    assert!(
        msg.contains("cwd") && (msg.contains("normalization") || msg.contains("bind mount")),
        "cwd差異（chdir・正規化・bind mount）の可能性案内: {}",
        err.message()
    );
    assert!(
        msg.contains("hold") && (msg.contains("-c") || msg.contains("-s")),
        "hold flag差（-c/-s起因）の可能性案内: {}",
        err.message()
    );
}

// [covers:remap-sequence.empty-list-panes-retry-in-polls]
#[test]
fn empty_list_panes_is_retried_during_poll() {
    // poll中の空list-panes応答はretry（TASK-74副因C）: 最初のpipe（probe）以降に
    // armした空応答（run冒頭snapshotでは計数が消費されないarm法。CR74-3）が
    // probe pollに実際に届く系列。probe pollのtitle判定はlenient呼び出しの戻り値で
    // 直接行い、1 tickあたりlist-panes系backend呼び出しは1回とする（lenientを
    // gateのみに使いstrict list_panesで再取得すると、gate直後のstrict呼び出しが
    // 空応答を返した時点でfatalになる窓が残るため——CR74-2）
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    b.empty_panes_after_pipe(2);
    let result = with_isolated_xdg(|_| run(&b, &args(false)));
    assert!(result.is_ok(), "空応答を跨いでpollが成立する: {result:?}");
    let calls = b.calls();
    let probe_at = calls
        .iter()
        .position(|c| c.starts_with("pipe probe "))
        .expect("probe pipe");
    let break_at = calls
        .iter()
        .position(|c| c.starts_with("pipe break-"))
        .expect("break pipe");
    // probe poll window（probe pipeとbreak pipeの間）: 空応答2回 + 成立1回の
    // 計3回以上のlenient呼び出しで、pollが実際に空を跨いで継続したことを検証
    let window = &calls[probe_at + 1..break_at];
    let lenient = window
        .iter()
        .filter(|c| c.as_str() == "list-panes-lenient")
        .count();
    assert!(
        lenient >= 3,
        "空応答2回を跨いだpoll継続（retryの実証）: {window:?}"
    );
    assert!(
        !window.iter().any(|c| c.as_str() == "list-panes"),
        "probe pollは1 tick 1回のlenient呼び出しのみでstrict list_panesを含まない: {window:?}"
    );
}

// [covers:remap-sequence.empty-response-retry-before-verify]
#[test]
fn empty_response_is_retried_before_verify_snapshot() {
    // 検証冒頭（10.9）のlist-panes/list-tabs再取得、およびstep 6のfocus対象決定
    // （E2E要因A改訂で追加された適用後のlist-panes取得）は空応答でもdeadline内で
    // 再試行する。空はrun冒頭snapshotでも先行pollでも消費されないよう、
    // override-layout以降にarmする。v2.2の呼び出し順では先頭N回空のarm法だと
    // step 6のfocus対象決定・存在確認pollが空を全数消化して検証冒頭に届かなく
    // なるため、精密arm（指定番目の呼び出しのみ空）で特定位置へ届ける:
    // panes 1回目 = (0,0)のfocus対象決定（跨いで再試行）・
    // tabs 2回目 = 検証冒頭再取得（(1,0)存在確認が1回目で成立した後）
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    b.empty_panes_nth_after_override(1);
    b.empty_tabs_nth_after_override(2);
    let result = with_isolated_xdg(|_| run(&b, &args(false)));
    assert!(
        result.is_ok(),
        "検証冒頭の空応答を跨いで検証まで成功する: {result:?}"
    );
    // 最終override-layoutより後: panesはfocus対象決定・検証冒頭を合わせ2回以上・
    // tabsは検証冒頭の再取得が空を跨ぎ2回以上（空1回 + 成立1回）
    let calls = b.calls();
    let last_override = calls
        .iter()
        .rposition(|c| c.starts_with("override-layout"))
        .expect("override-layout");
    let after = &calls[last_override + 1..];
    let panes_calls = after
        .iter()
        .filter(|c| c.as_str() == "list-panes-lenient")
        .count();
    let tabs_calls = after
        .iter()
        .filter(|c| c.as_str() == "list-tabs-lenient")
        .count();
    assert!(
        panes_calls >= 2,
        "適用後のpanes再取得（focus対象決定・検証冒頭）が空を跨いだ再試行であることの実証: {after:?}"
    );
    assert!(
        tabs_calls >= 2,
        "検証冒頭のtabs再取得が空を跨いだ再試行であることの実証: {after:?}"
    );

    // 空応答がdeadline（10s）を超えて継続する系列は、step 6の適用後観測
    // （reconciliation gate・focus対象決定の共通取得）のpoll timeoutとして
    // OperationFailedで中断する（focus-pane-id呼出失敗のみがwarning skip
    // 〔best-effort〕で継続。観測自体の失敗は継続不能。従来のpolling timeout
    // errorへ合流。TASK-81レビューC3でコメントを実挙動へ更新）
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    b.empty_panes_after_override(1000);
    let err = with_isolated_xdg(|_| run(&b, &args(false))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
}

// [covers:remap-sequence.r39-new-tab-names-and-anchor-survives]
#[test]
fn r39_new_tab_names_and_anchor_survives() {
    // 新規tab名が <base>-<j+1> どおり・tab一覧にanchorが残存
    let (panes, tabs) = seven_panes_three_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).unwrap();
    });
    let s = b.state.borrow();
    let names: Vec<&str> = s.tabs.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"remap-2"), "raw: {names:?}");
    assert!(names.contains(&"remap-3"), "raw: {names:?}");
    assert!(
        s.tabs.iter().any(|t| t.id == ANCHOR),
        "anchor tabは残存: {:?}",
        s.tabs
    );
}

// [covers:remap-sequence.r40-empty-slots-filled-with-default-shell]
#[test]
fn r40_empty_slots_filled_with_default_shell() {
    // 空slotは既定shellで埋まる（L2ではpane数==Nとして検証。L4でshell paneの
    // 実在を確認）。7 pane / 3-slot: 第3 instanceは1 pane + 2空slot
    let (panes, tabs) = seven_panes_three_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).unwrap();
    });
    let s = b.state.borrow();
    for t in s.tabs.iter() {
        assert_eq!(
            tiled_count(&b, t.id),
            3,
            "各instance tabのpane数 == N=3（tab {}）",
            t.id.0
        );
    }
}

// [covers:remap-sequence.r41-verification-failure-reports-diff-and-progress]
#[test]
fn r41_verification_failure_reports_diff_and_progress() {
    // 検証失敗 -> VerificationFailed（exit 7）+ 期待/実測差分 + 実行済み/失敗/未実行
    // の区分。--json時は成功envelopeを出さずmapping/missingをerror.dataに載せた
    // 単一error envelopeをmainから出す（runはErrを返す。stdoutの単一性はL3契約）
    let (panes, tabs) = one_tab_four();
    let b = FakeBackend::new(panes, tabs);
    b.drop_on_next_override(PaneKindId::Terminal(3)); // group 0のpaneがlayout適用で消える
    let mut a = args(false);
    a.json = true;
    let err = with_isolated_xdg(|_| run(&b, &a)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    let data = err.data().expect("mapping/missingをerror.dataに載せる");
    assert!(
        data["missing"].as_array().is_some_and(|m| !m.is_empty()),
        "raw: {data}"
    );
    assert!(
        data["mapping"].as_array().is_some_and(|m| !m.is_empty()),
        "raw: {data}"
    );
    // TASK-75（v2.2）: error.dataは m・n（=S）・k に加え t（T）・n_slots（N_t列）を
    // 載せ、mapping要素は pane → block/tab/slot/TabId へ拡張される
    // （T=1ではn=S=N=3・t=1・n_slots=[3]）
    assert_eq!(data["t"], 1, "raw: {data}");
    assert_eq!(data["n_slots"], serde_json::json!([3]), "raw: {data}");
    assert!(
        data["mapping"]
            .as_array()
            .is_some_and(|m| m.iter().all(|e| e.get("block").is_some())),
        "mapping要素はblock keyを含む（tab keyはv2.1から維持）: {data}"
    );
    // TASK-81: error.dataにはさらにleftover_tabs（Z-5）とclosed_duplicates
    // （reconciliationのclose実績。成功・失敗双方で報告する報告義務の失敗側）の
    // 配列fieldが載る（該当なしでも空配列としてfield自体は載る。中身の収集は本体実装）
    assert!(
        data["leftover_tabs"].as_array().is_some(),
        "error.dataにleftover_tabs field: {data}"
    );
    assert!(
        data["closed_duplicates"].as_array().is_some(),
        "error.dataにclosed_duplicates field: {data}"
    );
}

// [covers:remap-sequence.r42-focus-restore-failure-is-not-operation-failure]
#[test]
fn r42_focus_restore_failure_is_not_operation_failure() {
    // focus復帰失敗は操作失敗に含めない（best-effort）。適用phaseのgo-to-tab
    // （override 2回の前）は成功し、検証後の復帰go-to-tabのみ失敗させる
    let (panes, tabs) = one_tab_four();
    let b = FakeBackend::new(panes, tabs);
    b.fail_go_to_tab_after_overrides(1); // override 2回完了後の最初のgo-to-tabを失敗
    with_isolated_xdg(|_| {
        run(&b, &args(false)).expect("focus復帰失敗は成功を壊さない");
    });
    assert!(b.calls().iter().any(|c| c.starts_with("override-layout")));
}

// ---- (i) dry-run R47/R50（出力契約R48/R49/R50種別表示はL3） ----

// [covers:remap-sequence.r47-dry-run-never-mutates-even-when-move-needed]
#[test]
fn r47_dry_run_never_mutates_even_when_move_needed() {
    // 状態変更なし: tab切替・pipe・wasm extract・permissions.kdl書換・backendの
    // mutating呼び出しすべて発生しない。M>N（k=2）でもdry-run自体は成功する
    let (panes, tabs) = one_tab_four();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|xdg| {
        run(&b, &args(true)).expect("dry-runはk=2でも計画を出して成功する");
        let calls = b.calls();
        for banned in [
            "go-to-tab",
            "pipe ",
            "toggle-embed",
            "override-layout",
            "new-tab",
            "rename-tab",
            "close-pane",
            "focus-pane-id",
        ] {
            assert!(
                !calls.iter().any(|c| c.starts_with(banned)),
                "dry-runで {banned} が呼ばれた: {calls:?}"
            );
        }
        assert!(
            !xdg.join("zellij").join("permissions.kdl").exists(),
            "dry-runはcompanion setup（permissions seed）を行わない"
        );
        assert!(
            !xdg.join("zelper").exists(),
            "dry-runはwasm extractも行わない"
        );
    });
    // snapshot（dump-layout）のような読み取りはdry-run表示に使ってよい
}

// [covers:remap-sequence.r50-dry-run-includes-floating-pane-in-plan-without-toggle]
#[test]
fn r50_dry_run_includes_floating_pane_in_plan_without_toggle() {
    // floating paneはtoggle前snapshotから仮想的に計画へ含め、dry-runと実行の
    // 計画一致を保証。dry-runではtoggleしない・floatingのまま
    let (panes, tabs) = same_tab_three();
    let mut floating = pane_at(9, "float", 0, 0, 20, 0, Some("htop"));
    floating.is_floating = true;
    let panes = [panes, vec![floating]].concat();

    let b = FakeBackend::new(panes.clone(), tabs.clone());
    let mut a = args(true);
    a.embed_floating = true;
    with_isolated_xdg(|_| {
        run(&b, &a).expect("3 tiled + 1 floating(embed予定) = M=4 -> k=2として計画成功");
    });
    assert!(
        !b.calls().iter().any(|c| c.contains("toggle-embed")),
        "dry-runはtoggleしない: {:?}",
        b.calls()
    );
    assert!(
        b.state.borrow().panes.iter().any(|p| p.is_floating),
        "floating paneはそのまま"
    );

    // 実行も同一snapshotから計画する（計画一致）: 同一構成で実行すると
    // 4 paneすべてがsourceとして配置される（空slotは既定shellで埋まるためR40の
    // とおり元paneの配置をもって検証する）
    let b2 = FakeBackend::new(panes, tabs);
    let mut a2 = args(false);
    a2.embed_floating = true;
    with_isolated_xdg(|_| {
        run(&b2, &a2).expect("実行はdry-runと同じ計画（M=4, k=2）で成功する");
    });
    let placed: Vec<PaneKindId> = b2
        .state
        .borrow()
        .panes
        .iter()
        .filter(|p| p.is_remap_source())
        .map(|p| p.id)
        .collect();
    for id in [1u32, 2, 3, 9] {
        assert!(
            placed.contains(&PaneKindId::Terminal(id)),
            "floating paneも含む全4 paneが配置される: {placed:?}"
        );
    }
}

// ---- TASK-39コードレビュー対応（C3/C5・観点2補足） ----

// [covers:remap-sequence.poll-deadline-not-extended-by-slow-backend-call]
#[test]
fn c3_poll_deadline_not_extended_by_slow_backend_call() {
    // pollingのdeadlineはbackend呼出（list_panes）の所要時間を含めて保証する:
    // deadline超過後に条件が成立していても成功扱いにしない。最初のpipe（probe）
    // 後のlist_panesを10.3s遅延させ、probe titleが現れている状態でdeadlineを
    // 越える系列を注入する
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    b.slow_list_panes_after_pipe(std::time::Duration::from_millis(10_300));
    let err = with_isolated_xdg(|_| run(&b, &args(false))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
    assert!(
        err.message().contains("probe"),
        "probe不成立として報告: {}",
        err.message()
    );
    // probe側timeout文面もscreen繁忙（screen応答1s timeoutによる一時的な空list
    // 応答）で起こりうる旨のF8機構の注記を含む（CR74-4: 空応答を権限問題と
    // 誤診させない）
    let msg = err.message().to_lowercase();
    assert!(
        msg.contains("screen")
            && (msg.contains("busy") || msg.contains("1s") || msg.contains("empty")),
        "screen繁忙の1s list応答timeout機構の注記: {}",
        err.message()
    );
    // 遅延実行を成功扱いしないため、状態変更（break pipe・layout適用）へ進まない
    let calls = b.calls();
    assert!(
        !calls.iter().any(|c| c.starts_with("pipe break-")),
        "{calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.starts_with("override-layout")),
        "{calls:?}"
    );
}

// [covers:remap-sequence.new-tab-name-verified-by-target-tab-id]
#[test]
fn c5_new_tab_name_verified_by_target_tab_id() {
    // 新規tab名検証はtarget tab自身のIDと名前の対応で行う。同名tabが既に存在して
    // も、target tabのrenameが不生效なら検証失敗とする（C5: 同名存在のみの検証は
    // 誤通過する）。renameをmuteしたfakeで回帰検出する
    let (panes, tabs) = one_tab_four();
    // 既存の空tabがたまたま "remap-2" という名を持つ構成（anchor外・source外）
    let mut tabs = tabs;
    tabs.push(tab_state(1, 1, "remap-2", false, 0));
    let b = FakeBackend::new(panes, tabs);
    b.mute_rename_tab(); // break-new後のrename-tabが成功扱いだが名を変えない
    let err = with_isolated_xdg(|_| run(&b, &args(false))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    // 期待/実測の差分に「target tab自身の名前」が載る
    assert!(
        err.message().contains("expected remap-2"),
        "raw: {}",
        err.message()
    );
    assert!(
        err.message().contains("named"),
        "実測名が載る: {}",
        err.message()
    );
}

// [covers:remap-sequence.exited-held-panes-excluded-from-source-and-survive]
#[test]
fn exited_held_panes_are_excluded_from_source_and_survive() {
    // exit hold中（exited / is_held）のpaneは実行runを持たずrun一致照合の対象に
    // ならないためsource外（DD-10.5）。killも移動もされず元tabに残る
    let (mut panes, tabs) = same_tab_three();
    let mut exited = pane_at(8, "exited", 0, 0, 40, 0, Some("cmd-x"));
    exited.exited = true;
    exited.is_held = true;
    panes.push(exited);
    let mut held = pane_at(7, "held", 0, 0, 50, 0, None);
    held.is_held = true;
    panes.push(held);

    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).expect("M=3（exited/held除外）でk=1・移動不要のため成功");
    });
    let s = b.state.borrow();
    for id in 1u32..=3 {
        let p = s
            .panes
            .iter()
            .find(|p| p.id == PaneKindId::Terminal(id))
            .unwrap_or_else(|| panic!("pane {id}"));
        assert_eq!(p.tab_id, ANCHOR, "元3 paneはanchorに配置");
    }
    // exited/held paneはkillも移動もされず生存
    for id in [7u32, 8] {
        assert!(
            s.panes.iter().any(|p| p.id == PaneKindId::Terminal(id)),
            "pane {id} 生存"
        );
    }
    drop(s);
    let calls = b.calls();
    assert!(!calls.iter().any(|c| c.contains("close-pane")), "{calls:?}");
    assert!(!calls.iter().any(|c| c.starts_with("pipe ")), "{calls:?}");
    assert!(
        !calls.iter().any(|c| c.contains("toggle-embed")),
        "{calls:?}"
    );
}

// [covers:remap-sequence.embed-floating-exited-held-excluded-from-floating-set]
#[test]
fn embed_floating_exited_held_panes_excluded_from_floating_set() {
    // --embed-floating時でもexited/heldなfloating paneはfloating集合に入れない
    // （source混入とtoggleの両方を防ぐ）。run一致照合の対象にならずtoggle後の
    // membership検証にも落るため、floatingのまま元位置に残す（DD-10.5・PR#6指摘）
    let (mut panes, tabs) = same_tab_three();
    let mut exited = pane_at(8, "fx", 0, 0, 40, 0, Some("cmd-x"));
    exited.is_floating = true;
    exited.exited = true;
    exited.is_held = true;
    panes.push(exited);
    let mut held = pane_at(7, "fh", 0, 0, 50, 0, None);
    held.is_floating = true;
    held.is_held = true;
    panes.push(held);

    let b = FakeBackend::new(panes, tabs);
    let mut a = args(false);
    a.embed_floating = true;
    with_isolated_xdg(|_| {
        run(&b, &a).expect("M=3（exited/held floating除外）でk=1・移動不要のため成功");
    });
    let s = b.state.borrow();
    // 元3 paneはtiled sourceとして配置される
    for id in 1u32..=3 {
        let p = s
            .panes
            .iter()
            .find(|p| p.id == PaneKindId::Terminal(id))
            .unwrap_or_else(|| panic!("pane {id}"));
        assert!(!p.is_floating, "元3 paneはtiled");
    }
    // exited/held floating paneはtoggleされずfloatingのまま生存
    for id in [7u32, 8] {
        let p = s
            .panes
            .iter()
            .find(|p| p.id == PaneKindId::Terminal(id))
            .unwrap_or_else(|| panic!("pane {id} 生存"));
        assert!(
            p.is_floating,
            "pane {id} はfloatingのまま（toggleされない）"
        );
    }
    drop(s);
    let calls = b.calls();
    assert!(
        !calls.iter().any(|c| c.contains("toggle-embed")),
        "対象floatingがいないためtoggleは不発生: {calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.contains("close-pane")),
        "killもされない: {calls:?}"
    );
}

// ---- TASK-74段階8コードレビュー対応（CR74-1） ----

/// children block内に `children` nodeを持たないdefault_tab_template付きlayout
/// （template系LayoutInvalidの例。children marker不在）
const TEMPLATE_NO_CHILDREN: &str = "layout {\n    default_tab_template {\n        pane size=1 borderless=true {\n            plugin location=\"zellij:compact-bar\"\n        }\n    }\n    tab {\n        pane\n        pane\n        pane\n    }\n}\n";

// [covers:remap-sequence.template-invalid-detected-before-mutation]
#[test]
fn template_invalid_is_detected_before_mutation() {
    // template系LayoutInvalid（children marker不在）はrun()の状態変更前（dry-run
    // 分岐と同形の位置でのinstance_kdls計算）に検出する（CR74-1: execute step 6で
    // 計算するとprobe・break/move・rename後に発火し、DD-10.7前提（全preflightを
    // 状態変更前に完了）と設計§2.2「事前中断」に違反する）
    let (panes, tabs) = four_panes_two_tabs();
    let b = FakeBackend::new(panes, tabs);
    let mut a = args(false);
    a.inline = Some(TEMPLATE_NO_CHILDREN);
    let err = with_isolated_xdg(|xdg| {
        let err = run(&b, &a).unwrap_err();
        // companion setup（wasm extract・permissions seed）も不発生
        assert!(
            !xdg.join("zelper").exists(),
            "companion setup（wasm extract）は不発生: {}",
            xdg.display()
        );
        assert!(
            !xdg.join("zellij").join("permissions.kdl").exists(),
            "permissions seedは不発生"
        );
        err
    });
    assert_eq!(*err.class(), ErrorClass::LayoutInvalid);
    assert!(
        err.message().contains("default_tab_template"),
        "raw: {}",
        err.message()
    );
    // 一切の状態変更（probe pipe・toggle・rename・tab切替・layout適用）は不発生
    let calls = b.calls();
    for banned in [
        "pipe ",
        "toggle-embed",
        "rename-tab",
        "go-to-tab",
        "override-layout",
        "focus-pane-id",
        "new-tab",
    ] {
        assert!(
            !calls.iter().any(|c| c.starts_with(banned)),
            "状態変更前に中断するため {banned} は不発生: {calls:?}"
        );
    }
}

// ---- TASK-75（DD-10 v2.2: multi-tab layout全体再現） ----

/// T=2 layout（tab0=名無し2 slot・tab1 "single" 1 slot。N_t=2,1 → S=3）
const TWO_TAB_LAYOUT: &str = "layout {\n    tab {\n        pane\n        pane\n    }\n    tab name=\"single\" {\n        pane\n    }\n}\n";

/// T=2 layout（tab0=名無し3 slot・tab1 "sub" 2 slot。N_t=3,2 → S=5）
const TWO_TAB_3_2: &str = "layout {\n    tab {\n        pane\n        pane\n        pane\n    }\n    tab name=\"sub\" {\n        pane\n        pane\n    }\n}\n";

/// T=2 layout（tab0 slot1にpane focus=true・tab1名無し1 slot。N_t=2,1 → S=3）
const FOCUS_SLOT_LAYOUT: &str = "layout {\n    tab {\n        pane\n        pane focus=true\n    }\n    tab {\n        pane\n    }\n}\n";

/// T=3 layout（N_t=2,1,1）。系列a: focus=trueは鋳型1のみ（最終go-to先は新規tab側）
const FOCUS_SERIES_A: &str = "layout {\n    tab {\n        pane\n        pane\n    }\n    tab name=\"focus-tab\" focus=true {\n        pane\n    }\n    tab {\n        pane\n    }\n}\n";

/// T=3 layout（N_t=2,1,1）。系列b: tab0とtab2の両方にfocus=true（文書順最初のtab0）
const FOCUS_SERIES_B: &str = "layout {\n    tab focus=true {\n        pane\n        pane\n    }\n    tab name=\"focus-tab\" {\n        pane\n    }\n    tab focus=true {\n        pane\n    }\n}\n";

/// T=3 layout（N_t=2,1,1）。系列c: focus=trueなし（anchor復帰）
const FOCUS_SERIES_C: &str = "layout {\n    tab {\n        pane\n        pane\n    }\n    tab name=\"focus-tab\" {\n        pane\n    }\n    tab {\n        pane\n    }\n}\n";

/// T=3 layout（tab0=9 slot・tab1 "one" 1 slot・tab2 "two" 2 slot。N_t=9,1,2 → S=12）
const NINE_ONE_TWO: &str = "layout {\n    tab {\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n        pane\n    }\n    tab name=\"one\" {\n        pane\n    }\n    tab name=\"two\" {\n        pane\n        pane\n    }\n}\n";

fn multi_args(layout: &'static str) -> RemapArgs<'static> {
    let mut a = args(false);
    a.inline = Some(layout);
    a
}

// [covers:remap-sequence.move-phase-generalized-block-tab-order]
#[test]
fn move_phase_generalized_block_tab_order() {
    // 移動系列のblock×tab一般化: 5-a（group (0,0)のanchor外paneをbreak-idで先行）
    // → 5-b（block昇順・tab昇順で (0,0)以外を break-new → membership poll → rename）。
    // T=2（N_t=3,2 → S=5）にM=7: (0,0)=p1,2,3（うちp3がanchor外）・(0,1)=p4,5・
    // (1,0)=p6,7・(1,1)=空group（new-tab経路で具体化）
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
        pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
        pane_at(4, "d", 1, 1, 0, 50, Some("cmd4")),
        pane_at(5, "e", 2, 2, 0, 0, Some("cmd5")),
        pane_at(6, "f", 2, 2, 0, 50, Some("cmd6")),
        pane_at(7, "g", 3, 3, 0, 0, Some("cmd7")),
    ];
    let tabs = vec![
        tab_state(0, 0, "t0", true, 2),
        tab_state(1, 1, "t1", false, 2),
        tab_state(2, 2, "t2", false, 2),
        tab_state(3, 3, "t3", false, 1),
    ];
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &multi_args(TWO_TAB_3_2)).expect("multi-tab移動系列で成功する");
    });
    let calls = b.calls();
    // 5-a: group (0,0)のanchor外pane（p3）のbreak-idが最初のbreak-newより先
    let break_id_at = calls
        .iter()
        .position(|c| c.starts_with("pipe break-id "))
        .expect("break-id pipe");
    let first_break_new = calls
        .iter()
        .position(|c| c.starts_with("pipe break-new "))
        .expect("break-new pipe");
    assert!(break_id_at < first_break_new, "inbound先行: {calls:?}");
    let payload = calls[break_id_at].trim_start_matches("pipe break-id ");
    assert!(payload.contains("terminal_3"), "raw: {payload}");
    // 5-b: (0,1) break-new（p4,5）→ rename（鋳型名幹 "sub"）→ (1,0) break-new
    // （p6,7）→ rename（base幹 + -2接尾）の順
    let bn_sub = calls
        .iter()
        .position(|c| {
            c.starts_with("pipe break-new ") && c.contains("terminal_4") && c.contains("terminal_5")
        })
        .expect("group (0,1) break-new");
    let rn_sub = calls
        .iter()
        .position(|c| c == "rename-tab 4 sub")
        .expect("group (0,1) rename（鋳型名幹・b=0は接尾なし）");
    let bn_b2 = calls
        .iter()
        .position(|c| {
            c.starts_with("pipe break-new ") && c.contains("terminal_6") && c.contains("terminal_7")
        })
        .expect("group (1,0) break-new");
    let rn_b2 = calls
        .iter()
        .position(|c| c == "rename-tab 5 remap-2")
        .expect("group (1,0) rename（base幹 + -<b+1>接尾）");
    assert!(bn_sub < rn_sub, "raw: {calls:?}");
    assert!(rn_sub < bn_b2, "block昇順: {calls:?}");
    assert!(bn_b2 < rn_b2, "raw: {calls:?}");
    // (1,1) 空group: break-new（空list）ではなく new-tab --layout-string（生成KDL
    // 適用済みtab作成・E2E要因B改訂）で具体化し rename（鋳型名幹 + -2接尾）
    let new_tab_at = calls
        .iter()
        .position(|c| c.starts_with("new-tab name=None layout=Some("))
        .expect("空groupはnew-tab --layout-stringでtab作成");
    let rn_empty = calls
        .iter()
        .position(|c| c == "rename-tab 6 sub-2")
        .expect("空groupのrename（鋳型名幹 + -2接尾）");
    assert!(new_tab_at < rn_empty, "raw: {calls:?}");
    assert!(
        calls
            .iter()
            .filter(|c| c.starts_with("pipe break-new "))
            .all(|c| c.contains("terminal_")),
        "空listのbreak-new呼出は不発生: {calls:?}"
    );
}

// [covers:remap-sequence.empty-group-tab-created-via-new-tab-then-rename]
#[test]
fn empty_group_tab_created_via_new_tab_then_rename() {
    // 割当0件のgroupはbreak-new（空list）ではなく new-tab --layout-string <生成KDL>
    // でtabを作成時点で生成KDLを適用し、stdout由来idをparse直後にrenameで命名
    // （取得直後に消費）。E2E要因B改訂: 旧経路（layout引数なし→override-layoutで
    // 正規化）は既定paneがbare slotに照合されず残存するため廃止。step 6では
    // 当該tabへのoverride-layoutはskip（作成時点で適用済み）
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
    ];
    let tabs = vec![tab_state(0, 0, "t0", true, 2)];
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &multi_args(TWO_TAB_LAYOUT)).expect("M=2・S=3・k=1（(0,1)は空group）で成功");
    });
    let calls = b.calls();
    assert!(
        calls
            .iter()
            .any(|c| c.starts_with("new-tab name=None layout=Some(")),
        "空groupはnew-tab --layout-string <生成KDL>（tab作成時点で生成KDL適用）: {calls:?}"
    );
    assert!(
        calls.iter().any(|c| c == "rename-tab 1 single"),
        "new-tab由来idはparse直後のrenameで消費: {calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.starts_with("pipe break-")),
        "割当ありgroup (0,0)は全pane anchor内のためbreak系不発生: {calls:?}"
    );
    // step 6: 生成tab (0,1)へはgo-toするが override-layout は不発生（new-tab
    // --layout-stringで作成時点で生成KDL適用済みのためskip）
    let go_to_new = calls
        .iter()
        .position(|c| c == "go-to-tab 1")
        .expect("空group生成tabへgo-to");
    let scope = &calls[go_to_new..];
    let next_tab_boundary = scope
        .iter()
        .position(|c| c.starts_with("go-to-tab") && *c != "go-to-tab 1")
        .unwrap_or(scope.len());
    assert!(
        !scope[..next_tab_boundary]
            .iter()
            .any(|c| c.starts_with("override-layout")),
        "空groupのstep 6はoverride-layoutをskip: {calls:?}"
    );
    let s = b.state.borrow();
    let single = s.tabs.iter().find(|t| t.name == "single").expect("生成tab");
    assert_eq!(
        tiled_count(&b, single.id),
        1,
        "tab作成時点の生成KDL適用でpane数 == N_t=1"
    );

    // new-tabの失敗（非zero exit / stdout parse不能に相当するL2注入）は
    // OperationFailedで中断（部分状態報告）
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
    ];
    let tabs = vec![tab_state(0, 0, "t0", true, 2)];
    let b = FakeBackend::new(panes, tabs);
    b.inject_failure("new-tab");
    let err = with_isolated_xdg(|_| run(&b, &multi_args(TWO_TAB_LAYOUT))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::OperationFailed);
}

// [covers:remap-sequence.focus-pane-id-after-override-per-tab]
#[test]
fn focus_pane_id_after_override_per_tab() {
    // 各生成tabの処理は go-to-tab → override-layout → （list-panesで対象特定）→
    // focus-pane-id terminal_<id> の順。対象pane idはmappingでなく適用後の実状態の
    // 位置——当該tabのterminal paneをvisual order（geometry y,x昇順）に並べたs番目
    // （s = 鋳型pane_focus_slot、無ければslot 0）——から決定する（E2E要因A改訂。
    // bare pane群のrun一致配置は割当順と一致しないためmapping基準だと実幾何とずれる）。
    // 空slot（spawn pane）も位置から特定可能なため全生成tabで実行。呼出失敗は
    // warningで継続し、focus未反映は検証(d)が検知する
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
        pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
    ];
    let tabs = vec![
        tab_state(0, 0, "t0", true, 2),
        tab_state(1, 1, "t1", false, 1),
    ];
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &multi_args(FOCUS_SLOT_LAYOUT)).expect("focus対象は全tab occupiedのため成功");
    });
    let calls = b.calls();
    let ops: Vec<&String> = calls
        .iter()
        .filter(|c| {
            c.starts_with("go-to-tab")
                || c.starts_with("override-layout")
                || c.starts_with("focus-pane-id")
        })
        .collect();
    assert!(
        ops.len() >= 6,
        "各tab go-to→override→focus-pane-idの3操作×2tab: {ops:?}"
    );
    assert!(ops[0].starts_with("go-to-tab"), "raw: {ops:?}");
    assert!(ops[1].starts_with("override-layout"), "raw: {ops:?}");
    assert_eq!(
        ops[2], "focus-pane-id terminal_2",
        "tab0のfocus対象は適用後のvisual order slot 1（pane focus=true。= terminal_2）: {ops:?}"
    );
    assert!(ops[3].starts_with("go-to-tab"), "raw: {ops:?}");
    assert!(ops[4].starts_with("override-layout"), "raw: {ops:?}");
    assert_eq!(
        ops[5], "focus-pane-id terminal_3",
        "tab1はfocus指定なしなのでvisual slot 0（= terminal_3）: {ops:?}"
    );

    // focus-pane-id呼出をErrにする系列: 呼出失敗はwarning（継続）のためrun自体は
    // OperationFailedにならず、focus未反映が検証(d)のVerificationFailedとして検知される
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
        pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
    ];
    let tabs = vec![
        tab_state(0, 0, "t0", true, 2),
        tab_state(1, 1, "t1", false, 1),
    ];
    let b = FakeBackend::new(panes, tabs);
    b.inject_failure("focus-pane-id");
    let err = with_isolated_xdg(|_| run(&b, &multi_args(FOCUS_SLOT_LAYOUT))).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::VerificationFailed,
        "呼出失敗はwarning化され検証(d)で検知: {}",
        err.message()
    );
    assert!(
        b.calls().iter().any(|c| c.starts_with("focus-pane-id ")),
        "focus-pane-id呼出自体は試みられている: {:?}",
        b.calls()
    );

    // focus対象slotが最終blockで空slotになる構成: 位置ベース決定のため空slot
    // （spawn pane）も位置から特定でき、全生成tabでfocus-pane-idが実行される
    // （E2E要因A改訂: occupied限定撤廃）。
    // (0,0)=t1,t2 → visual slot 1 = terminal_2・(0,1)=t3 → slot 0 = terminal_3・
    // (1,0)=t4+spawn(id 6・y=末尾) → slot 1 = terminal_6（spawn pane = 空slot相当）・
    // (1,1)=new-tab --layout-string生成pane(id 5) → slot 0 = terminal_5
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
        pane_at(3, "c", 0, 0, 10, 0, Some("cmd3")),
        pane_at(4, "d", 0, 0, 10, 50, Some("cmd4")),
    ];
    let tabs = vec![tab_state(0, 0, "t0", true, 4)];
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &multi_args(FOCUS_SLOT_LAYOUT))
            .expect("空slot対象も位置から特定されfocus検証（位置基準）が成功する");
    });
    let focuses: Vec<String> = b
        .calls()
        .iter()
        .filter(|c| c.starts_with("focus-pane-id "))
        .cloned()
        .collect();
    assert_eq!(
        focuses,
        vec![
            "focus-pane-id terminal_2".to_string(),
            "focus-pane-id terminal_3".to_string(),
            "focus-pane-id terminal_6".to_string(),
            "focus-pane-id terminal_5".to_string(),
        ],
        "全生成tabでfocus-pane-id: (1,0)のfocus対象は空slot（spawn pane = terminal_6）\
・(1,1)はnew-tab --layout-string生成pane = terminal_5: {:?}",
        b.calls()
    );

    // zellijが「already focused」でexit 2を返す系列: focus状態が実際に成立している
    // ため成功扱い（warningでなく継続）。runは成功し検証(d)も位置基準で通る
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
        pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
    ];
    let tabs = vec![
        tab_state(0, 0, "t0", true, 2),
        tab_state(1, 1, "t1", false, 1),
    ];
    let b = FakeBackend::new(panes, tabs);
    b.already_focused_focus();
    with_isolated_xdg(|_| {
        run(&b, &multi_args(FOCUS_SLOT_LAYOUT))
            .expect("already focusedは成功扱い（focus成立と同じ状態）のためrun成功");
    });
    let s = b.state.borrow();
    let focused2 = s
        .panes
        .iter()
        .find(|p| p.id == PaneKindId::Terminal(2))
        .unwrap()
        .is_focused;
    drop(s);
    assert!(
        focused2,
        "already focused応答でもfocus状態は成立している: {:?}",
        b.calls()
    );
}

// [covers:remap-sequence.final-go-to-honors-template-focus]
#[test]
fn final_go_to_honors_template_focus() {
    // tab focus決定則: focus=true鋳型の文書順最初t*についてblock 0のtab (0,t*)へ
    // 最終go-toする。無ければanchor復帰。最終go-toは検証後・best-effort
    let last_go_to = |b: &FakeBackend| -> String {
        let calls = b.calls();
        let at = calls
            .iter()
            .rposition(|c| c.starts_with("go-to-tab"))
            .expect("最終go-to呼出");
        calls[at].clone()
    };
    let four_panes_three_tabs = || -> (Vec<PaneState>, Vec<TabState>) {
        (
            vec![
                pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
                pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
                pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
                pane_at(4, "d", 2, 2, 0, 0, Some("cmd4")),
            ],
            vec![
                tab_state(0, 0, "t0", true, 2),
                tab_state(1, 1, "t1", false, 1),
                tab_state(2, 2, "t2", false, 1),
            ],
        )
    };

    // 系列a: focus=trueは鋳型1のみ → 最終go-to先はblock 0の (0,1)（anchorでない）
    let (panes, tabs) = four_panes_three_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &multi_args(FOCUS_SERIES_A)).expect("系列aは成功");
    });
    assert_eq!(
        last_go_to(&b),
        "go-to-tab 3",
        "系列a: 最終go-to先は (0,1) の生成tab（anchorでない）: {:?}",
        b.calls()
    );

    // 系列b: 複数focus=true（tab0とtab2）→ 文書順最初のtab0（= anchor）へ
    let (panes, tabs) = four_panes_three_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &multi_args(FOCUS_SERIES_B)).expect("系列bは成功");
    });
    assert_eq!(
        last_go_to(&b),
        "go-to-tab 0",
        "系列b: 文書順最初のfocus=true鋳型 (0,0)（= anchor）: {:?}",
        b.calls()
    );

    // 系列c: focus=trueなし → anchor復帰（現行挙動と同一）
    let (panes, tabs) = four_panes_three_tabs();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &multi_args(FOCUS_SERIES_C)).expect("系列cは成功");
    });
    assert_eq!(
        last_go_to(&b),
        "go-to-tab 0",
        "系列c: focus指定なしはanchor復帰: {:?}",
        b.calls()
    );
}

// [covers:remap-sequence.verify-per-tab-pane-count]
#[test]
fn verify_per_tab_pane_count() {
    // 検証(b)のper-tab化: 各生成tab (b,t)のpane数 == N_t。anchor (0,0)を含む全生成
    // tabが対象（TR75-6: anchor例外は名前・位置のみであり形状・slot数の例外ではない）
    let mut panes = Vec::new();
    for i in 1..=9u32 {
        panes.push(pane_at(
            i,
            "a",
            0,
            0,
            (i - 1) / 3,
            ((i - 1) % 3) * 50,
            Some("cmd"),
        ));
    }
    panes.push(pane_at(10, "j", 1, 1, 0, 0, Some("cmd")));
    panes.push(pane_at(11, "k", 2, 2, 0, 0, Some("cmd")));
    panes.push(pane_at(12, "l", 2, 2, 0, 50, Some("cmd")));
    let tabs = vec![
        tab_state(0, 0, "t0", true, 9),
        tab_state(1, 1, "t1", false, 1),
        tab_state(2, 2, "t2", false, 2),
    ];

    // 成立系: M=12 = S → k=1。各tabのpane数 == N_t（anchor tabも9）
    let b = FakeBackend::new(panes.clone(), tabs.clone());
    with_isolated_xdg(|_| {
        run(&b, &multi_args(NINE_ONE_TWO)).expect("M=12・S=12・k=1で各tab pane数 == N_tとなり成功");
    });
    {
        let s = b.state.borrow();
        let by_name = |n: &str| {
            s.tabs
                .iter()
                .find(|t| t.name == n)
                .unwrap_or_else(|| panic!("tab {n}: {:?}", s.tabs))
                .id
        };
        let counts = [
            (tiled_count(&b, TabId(0)), 9),
            (tiled_count(&b, by_name("one")), 1),
            (tiled_count(&b, by_name("two")), 2),
        ];
        for (got, want) in counts {
            assert_eq!(got, want, "各生成tabのpane数 == N_t");
        }
    }

    // fatal系: 鋳型1（N_t=1）のtab適用後に重複paneが湧く → 検証失敗。v2.2採番では
    // (0,1) がbreak-newでtab 3を得る。TASK-81 E2E改訂後はreconciliation（多段規則）
    // が重複paneの回収を試みる（数整合で反転1回）が、割当paneのinvoked_withが
    // derive None系のためpass 2のpane cwd slotと照合せず再spawnし、残余close後も
    // 検証失敗として報告される（(b)のN_t差分またはreconciliationの解消不能note）
    let b = FakeBackend::new(panes, tabs);
    b.spawn_extra_on_override(TabId(3));
    let err = with_isolated_xdg(|_| run(&b, &multi_args(NINE_ONE_TWO))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert!(
        err.message().contains("expected 1")
            || err.message().contains("could not resolve duplicate panes"),
        "鋳型1のN_t=1との差分またはreconciliation解消不能の報告: {}",
        err.message()
    );
}

// [covers:remap-sequence.pane-focus-verified-via-is-focused]
#[test]
fn pane_focus_verified_via_is_focused() {
    // 検証(d) pane focus: 各生成tabの期待focus対象pane（pane_focus_slot or slot 0の
    // occupied pane）が is_focused == true。非active tabも検証対象（R75-3）。
    // focus未反映はVerificationFailedとして検知される
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
        pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
    ];
    let tabs = vec![
        tab_state(0, 0, "t0", true, 2),
        tab_state(1, 1, "t1", false, 1),
    ];
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &multi_args(FOCUS_SLOT_LAYOUT)).expect("成立系は成功");
    });
    let s = b.state.borrow();
    let focused = |id: u32| {
        s.panes
            .iter()
            .find(|p| p.id == PaneKindId::Terminal(id))
            .unwrap_or_else(|| panic!("pane {id}"))
            .is_focused
    };
    assert!(focused(2), "tab0の期待focus対象（slot 1 = terminal_2）");
    assert!(
        focused(3),
        "tab1の期待focus対象（slot 0 = terminal_3。非active tabも検証対象）"
    );
    drop(s);

    // fatal系: focus-pane-id呼出が失敗しfocusが未反映のまま → 検証(d)の
    // VerificationFailedとして検知される（呼出失敗のwarning化と区別される）
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
        pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
    ];
    let tabs = vec![
        tab_state(0, 0, "t0", true, 2),
        tab_state(1, 1, "t1", false, 1),
    ];
    let b = FakeBackend::new(panes, tabs);
    b.inject_failure("focus-pane-id");
    let err = with_isolated_xdg(|_| run(&b, &multi_args(FOCUS_SLOT_LAYOUT))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert!(
        err.message().contains("focus"),
        "期待focus対象paneの差分が載る: {}",
        err.message()
    );
}

// [covers:remap-sequence.leftover-tabs-reported-not-closed]
#[test]
fn leftover_tabs_reported_not_closed() {
    // 過剰tab（targetsに含まれないtab）はcloseせず放置し、leftover_tabs
    // （id/name/position/selectable pane数）として報告する（D6。失敗条件にしない）。
    // v2.2実装でrun()の戻り値がVec<LeftoverTab>（報告経路）へ拡張されたため、
    // 報告欄のassertを実装後の経路で検証する（human warning行・--jsonの
    // data.leftover_tabsはこの戻り値から出力される）
    let (mut panes, mut tabs) = same_tab_three();
    let mut plugin = pane_at(8, "companion-host", 1, 1, 0, 0, None);
    plugin.id = PaneKindId::Plugin(1);
    plugin.is_selectable = false;
    plugin.plugin_url = Some("file:zelper-companion.wasm".into());
    panes.push(plugin);
    tabs.push(tab_state(1, 1, "leftover", false, 0));
    let b = FakeBackend::new(panes, tabs);
    let leftovers =
        with_isolated_xdg(|_| run(&b, &args(false)).expect("leftoverの存在は失敗条件にしない"));
    let calls = b.calls();
    assert!(
        !calls.iter().any(|c| c.starts_with("close-pane")),
        "明示的なcloseは行わない: {calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.starts_with("close-tab")),
        "明示的なcloseは行わない: {calls:?}"
    );
    assert!(
        b.state.borrow().tabs.iter().any(|t| t.id == TabId(1)),
        "leftover tabは生存（放置）: {:?}",
        b.state.borrow().tabs
    );
    // 報告: id/name/position/selectable pane数が載る（closeしない・run成功のまま）。
    // TASK-81: run()の戻り値はRemapOutcomeへ拡張され、leftover_tabs報告はそのfield
    assert_eq!(leftovers.leftover_tabs.len(), 1, "raw: {leftovers:?}");
    let l = &leftovers.leftover_tabs[0];
    assert_eq!(l.id, TabId(1), "raw: {l:?}");
    assert_eq!(l.name, "leftover", "raw: {l:?}");
    assert_eq!(l.position, 1, "raw: {l:?}");
    assert_eq!(l.selectable_tiled, 0, "raw: {l:?}");

    // 対照: leftover不在の構成では報告は空
    let (panes, tabs) = same_tab_three();
    let b = FakeBackend::new(panes, tabs);
    let leftovers = with_isolated_xdg(|_| run(&b, &args(false)).unwrap());
    assert!(leftovers.leftover_tabs.is_empty(), "raw: {leftovers:?}");
}

// [covers:remap-sequence.new-tab-only-needs-no-companion]
#[test]
fn new_tab_only_needs_no_companion() {
    // M=0（source空）のmulti-tab remap: k=1・生成tab 3（(0,0)=anchor）。break系pipe
    // 0件のためcompanion plugin・probe・permissions.kdl書換を一切行わない（move_needed
    // 判定とprobe要否の分離）。preflightは状態変更前に同一depthで完了している
    // （LayoutInvalid系はtemplate-invalid-detected-before-mutation）
    let b = FakeBackend::new(vec![], vec![tab_state(0, 0, "t0", true, 0)]);
    with_isolated_xdg(|xdg| {
        run(&b, &multi_args(NINE_ONE_TWO)).expect("M=0・k=1で成功");
        let calls = b.calls();
        assert!(
            pipes(&b).is_empty(),
            "break系pipe・probeともに不使用: {calls:?}"
        );
        assert!(
            !xdg.join("zelper").exists(),
            "companion setup（wasm extract）不使用: {}",
            xdg.display()
        );
        assert!(
            !xdg.join("zellij").join("permissions.kdl").exists(),
            "permissions seed不使用"
        );
        assert_eq!(
            calls
                .iter()
                .filter(|c| c.starts_with("new-tab name=None layout=Some("))
                .count(),
            2,
            "空group (0,1)・(0,2) はnew-tab --layout-string（作成時点で生成KDL適用）で具体化: {calls:?}"
        );
        assert!(
            calls.iter().any(|c| c == "rename-tab 1 one")
                && calls.iter().any(|c| c == "rename-tab 2 two"),
            "生成tab名は鋳型名どおり: {calls:?}"
        );
        assert_eq!(
            calls.iter().filter(|c| c.starts_with("go-to-tab")).count(),
            4,
            "step 6の3 tab分 + 最終go-to（anchor復帰）: {calls:?}"
        );
        assert_eq!(
            calls
                .iter()
                .filter(|c| c.starts_with("override-layout"))
                .count(),
            1,
            "override-layoutはanchor (0,0)のみ（空groupはnew-tab --layout-stringで作成時点適用のためskip・E2E要因B改訂）: {calls:?}"
        );
        assert_eq!(
            calls
                .iter()
                .filter(|c| c.starts_with("focus-pane-id "))
                .count(),
            3,
            "focus-pane-idは全生成tabで実行（位置ベース決定のため空slot/spawn paneも特定できる・E2E要因A改訂）: {calls:?}"
        );
    });
}

// [covers:remap-sequence.tab-id-resolution-lenient-before-apply]
#[test]
fn tab_id_resolution_lenient_before_apply() {
    // targets[b*T+t]配列をexecute scope内で保持し、step 6の各group処理直前に
    // list-tabs（lenient）で存在確認する。idが不在なら再解決（割当ありgroupはpane
    // 所属基準・空groupはrename済み生成tab名基準）。再解決不能はOperationFailed
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
        pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
    ];
    let tabs = vec![
        tab_state(0, 0, "t0", true, 2),
        tab_state(1, 1, "t1", false, 1),
    ];

    // 成立系: 生成tab (0,1)のid（break-newで採番されたtab 2）がlist-tabs上で
    // 別id（5）に入れ替わる → 再解決（pane 3の所属基準）で go-to-tab 5 が向く
    let b = FakeBackend::new(panes.clone(), tabs.clone());
    b.swap_tab_id_before_apply(TabId(2), TabId(5));
    with_isolated_xdg(|_| {
        run(&b, &multi_args(TWO_TAB_LAYOUT)).expect("再解決成立系は成功");
    });
    assert!(
        b.calls().iter().any(|c| c == "go-to-tab 5"),
        "go-to/overrideは再解決後のtab idへ向けられる: {:?}",
        b.calls()
    );

    // CR75-1: 再解決時のlist_panes取得はlenient poll（一時的な空stdout〔screen繁忙の
    // 1s list応答timeout〕で即時失敗しない。DD-10.7/10.9の空応答設計と同一規則）。
    // 再解決位置（(0,1)の存在確認でid不在→resolve_target内のlist_panes_lenient。
    // override後2回目のlist-panes-lenient呼び出し）に空応答を精密armし、retryで
    // 再解決が成立することを検証（strict 1回呼び出しなら再解決は空応答を跨げず
    // go-to 5 直前の連続lenient呼び出しが1回だけになるためfailする）
    let b = FakeBackend::new(panes.clone(), tabs.clone());
    b.swap_tab_id_before_apply(TabId(2), TabId(5));
    b.empty_panes_nth_after_override(2);
    with_isolated_xdg(|_| {
        run(&b, &multi_args(TWO_TAB_LAYOUT)).expect("再解決時の空応答を跨いでretry成功");
    });
    let calls = b.calls();
    let gt5 = calls
        .iter()
        .position(|c| c == "go-to-tab 5")
        .expect("再解決後のidへgo-to");
    let consecutive: Vec<&String> = calls[..gt5]
        .iter()
        .rev()
        .take_while(|c| c.as_str() == "list-panes-lenient")
        .collect();
    assert!(
        consecutive.len() >= 2,
        "再解決のlist-panes取得は空→retryの連続2回（strict 1回呼び出しでない）: {calls:?}"
    );

    // fatal系: tab自体が消失し再解決不能 → OperationFailed（中断時報告）
    let b = FakeBackend::new(panes, tabs);
    b.drop_tab_before_apply(TabId(2));
    let err = with_isolated_xdg(|_| run(&b, &multi_args(TWO_TAB_LAYOUT))).unwrap_err();
    assert_eq!(
        *err.class(),
        ErrorClass::OperationFailed,
        "再解決不能はOperationFailed（検証失敗ではない）: {}",
        err.message()
    );
}

// ---- TASK-81（DD-10.7 step 6 reconciliation・DD-10.9 (e')(e'')・DD-10.10緩和） ----

/// reconciliation系testの共通構成: pane 2(y=0)・pane 3(y=10)はderive None系shell
/// （slot 0/1割当）、pane 1(y=50)はinvoked_withを明示設定するRun::Cwd系shell
/// （slot 2割当）。THREE_SLOT（3 bare slot）でk=1・移動なし
fn recon_backend(invoked_1: fake::FakeRun, cwd_1: Option<&str>) -> FakeBackend {
    let panes = vec![
        pane_at(2, "sh-a", 0, 0, 0, 0, None),
        pane_at(3, "sh-b", 0, 0, 10, 0, None),
        {
            let mut p = pane_at(1, "cwd-shell", 0, 0, 50, 0, None);
            p.cwd = cwd_1.map(|c| c.to_string());
            p
        },
    ];
    let tabs = vec![tab_state(0, 0, "t0", true, 3)];
    let b = FakeBackend::new(panes, tabs);
    b.set_invoked_with(PaneKindId::Terminal(1), invoked_1);
    b
}

/// override-layout呼出回数
fn override_calls(b: &FakeBackend) -> usize {
    b.calls()
        .iter()
        .filter(|c| c.starts_with("override-layout"))
        .count()
}

/// close-pane呼出の記録
fn close_pane_calls(b: &FakeBackend) -> Vec<String> {
    b.calls()
        .iter()
        .filter(|c| c.starts_with("close-pane "))
        .cloned()
        .collect()
}

/// 成功系の基本構成: pane 1はinvoked_with=Cwd("/w")・pane_cwd=Some("/w")。
/// pass 1のbare slot 2とは照合せずspawn（dup）し、reconciliationの反転
/// （pass 2でslot 2をpane cwd="/w"へ置換）で照合成立する系列
fn recon_ok_backend() -> FakeBackend {
    recon_backend(
        fake::FakeRun::Cwd(std::path::PathBuf::from("/w")),
        Some("/w"),
    )
}

// [covers:remap-sequence.stable-observation-gates-reconciliation]
#[test]
fn stable_observation_gates_reconciliation() {
    // reconciliationの安定観測gate（DD-10.7 step 6 TASK-81）: override-layout後の
    // 当該tabのpane id集合がpollingで2回連続同一・かつ要素数 >= N_tの両方を満たした
    // 場合のみreconciliationへ進む。1回きりの観測（未安定）では反転・closeを
    // 実行しない。安定していてもpane数 == N_t（過剰なし）ならreconciliation不発動
    // （focus-pane-idへ直行）
    let b = recon_ok_backend();
    // 未安定系列: override後のlist-panes応答2回目にだけpaneが1つ現れる
    // （1回目と2回目でpane id集合が異なる→安定確定は3回目の観測以降）
    b.spawn_dup_nth_after_override(ANCHOR, 2);
    let result = with_isolated_xdg(|_| run(&b, &args(false)));
    let calls = b.calls();
    // id集合変化中（1回目/2回目の観測）にはclose・反転を実行しない: close-paneが
    // 存在するなら、その呼出位置より前にlist-panes-lenientが3回（未安定2回 +
    // 安定確定1回）以上現れている
    if let Some(close_at) = calls.iter().position(|c| c.starts_with("close-pane ")) {
        let lenient_before = calls[..close_at]
            .iter()
            .filter(|c| c.as_str() == "list-panes-lenient")
            .count();
        assert!(
            lenient_before >= 3,
            "未安定の間はclose-pane不発動（2回連続同一の安定確定後）: {calls:?}"
        );
    }
    // 改訂後の多段規則では本系列のbare dup対応づけは一意にならない（追加paneに
    // より超過数が不整合）ため安全側fallback: pass 2のoverride-layoutも不発
    assert_eq!(
        override_calls(&b),
        1,
        "未安定→安定系列の安全側fallback: 反転（pass 2）不発: {calls:?}"
    );
    // 安定確定後にreconciliationが走った証跡: 検証失敗note（safe-side fallback文面）
    let err = result.expect_err("fallback系列は超過解消不能のため検証失敗");
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert!(
        err.message().contains("safe-side"),
        "対応づけ不能の安全側fallback報告: {}",
        err.message()
    );

    // 過剰なし系列: 安定していてもpane数 == N_tならreconciliation不発動
    // （overrideは当該group 1回のみ・反転〔2回目override〕不発生・close不発生）
    let (panes, tabs) = same_tab_three();
    let b = FakeBackend::new(panes, tabs);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).expect("過剰なし系列はreconciliation不発動で成功");
    });
    assert_eq!(
        override_calls(&b),
        1,
        "pass 2のoverride-layoutは不発生: {:?}",
        b.calls()
    );
    assert!(
        close_pane_calls(&b).is_empty(),
        "close-pane不発生: {:?}",
        b.calls()
    );
}

// [covers:remap-sequence.reconciliation-inverts-shell-slot-with-pane-cwd]
#[test]
fn reconciliation_inverts_shell_slot_with_pane_cwd() {
    // shell slot反転のreconciliation系列（TASK-81改訂の核心）: (1) 位置規則——
    // visual order s番目のpaneが実行開始前snapshot非存在id（dup spawn）∧ slot sが
    // 割当あり（shell pane割当）ならslot sを反転対象。(2) 反転対象slot位置の新規
    // spawn paneをclose-pane。(3) 反転対象slotのみpane cwd="<pane_cwd>"（Run::Cwd
    // 表現）へ置き換えた生成KDLでoverride-layoutを再実行（close単独で完了扱いに
    // しない）。pass 2でRun::Cwd系shell（invoked_with）と照合成立する
    let b = recon_ok_backend();
    with_isolated_xdg(|_| {
        run(&b, &args(false)).expect("pass 2で照合成立し成功する");
    });
    let calls = b.calls();
    let closes = close_pane_calls(&b);
    assert!(
        !closes.is_empty(),
        "dup spawn paneへのclose-pane: {calls:?}"
    );
    assert_eq!(
        override_calls(&b),
        2,
        "close単独で完了扱いにしない（pass 1 + pass 2のoverride-layout）: {calls:?}"
    );
    // close-pane -> 2回目override（pass 2）の順
    let close_at = calls
        .iter()
        .position(|c| c.starts_with("close-pane "))
        .expect("close-pane");
    let second_override = calls
        .iter()
        .rposition(|c| c.starts_with("override-layout"))
        .expect("pass 2 override");
    assert!(close_at < second_override, "raw: {calls:?}");
    // 成立済みslot（pane 2/3）のpaneはcloseも反転もされず、pass 2後の当該位置は
    // 割当shell pane id（pane 1 = slot 2位置）
    let s = b.state.borrow();
    let mut in_tab: Vec<&PaneState> = s
        .panes
        .iter()
        .filter(|p| p.tab_id == ANCHOR && p.is_remap_source())
        .collect();
    in_tab.sort_by_key(|p| (p.geometry.y, p.geometry.x));
    assert_eq!(
        in_tab[2].id,
        PaneKindId::Terminal(1),
        "pass 2後のvisual order 2番目（slot 2）は割当shell pane: {:?}",
        in_tab.iter().map(|p| p.id).collect::<Vec<_>>()
    );
    assert!(
        in_tab.iter().any(|p| p.id == PaneKindId::Terminal(2))
            && in_tab.iter().any(|p| p.id == PaneKindId::Terminal(3)),
        "成立済みslotのpaneはcloseされない: {:?}",
        in_tab.iter().map(|p| p.id).collect::<Vec<_>>()
    );
    // close対象はsnapshot非存在idのみ（source pane id 1..3以外）
    for c in &closes {
        assert!(
            !c.contains("terminal_1") && !c.contains("terminal_2") && !c.contains("terminal_3"),
            "close対象はdup spawn pane: {c}"
        );
    }
}

// [covers:remap-sequence.command-slot-dup-closed-without-inversion]
#[test]
fn command_slot_dup_closed_without_inversion() {
    // command slot由来dupの取り扱い: 反転対象slotの割当がcommand pane
    // （terminal_command=Some）で、pass 1後に当該runとargv一致の新規spawn pane
    // （snapshot非存在id・cwd差により照合不成立）が現れた系列では、cwd反転の対象
    // 表現が存在しないため（Run::Commandはargv+cwdで既に注入済み）、当該dup paneを
    // closeのみ実施して検証失敗経路へ合流する（pass 2のoverrideを行わない）。
    // close実績はclosed_duplicatesへ報告
    let panes = vec![
        {
            let mut p = pane_at(1, "cmd", 0, 0, 0, 0, None);
            p.terminal_command = Some("htop".into());
            p.cwd = None; // pane_cwd欠落系: 生成KDL slotはcwd無しで出る
            p
        },
        pane_at(2, "sh-a", 0, 0, 10, 0, None),
        pane_at(3, "sh-b", 0, 0, 20, 0, None),
    ];
    let b = FakeBackend::new(panes, vec![tab_state(0, 0, "t0", true, 3)]);
    // 実機のzellij run起因pane: invoked_withのcwd=Some（クライアントcwd）だが
    // 生成KDL slot（pane_cwd欠落でcwd無し）とは照合しない
    b.set_invoked_with(
        PaneKindId::Terminal(1),
        fake::FakeRun::Command {
            argv: vec!["htop".into()],
            cwd: Some(std::path::PathBuf::from("/w")),
            hold_on_close: true,
            hold_on_start: false,
        },
    );
    let mut a = args(false);
    a.json = true;
    let err = with_isolated_xdg(|_| run(&b, &a)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert!(
        !close_pane_calls(&b).is_empty(),
        "dup paneへのclose-pane呼出は実行済み: {:?}",
        b.calls()
    );
    assert_eq!(
        override_calls(&b),
        1,
        "反転に相当するpass 2のoverride-layoutは当該slotについて行わない: {:?}",
        b.calls()
    );
    let data = err.data().expect("error.data");
    assert!(
        data["closed_duplicates"]
            .as_array()
            .is_some_and(|arr| !arr.is_empty()),
        "close実績がerror.data.closed_duplicatesに載る: {data}"
    );
}

// [covers:remap-sequence.reconciliation-at-most-once-then-close-leftovers]
#[test]
fn reconciliation_at_most_once_then_close_leftovers() {
    // 反転の回数上限と残余のclose: 反転（pass 2）は高々1回。pass 2後も当該tabに
    // 過剰pane（新規spawn id）が残る系列（pass 2の照合も不成立）では、以降の反転を
    // 行わず、残った新規spawn paneをclose-paneして検証失敗（10.9）へ合流する
    // （無限反復の構造的排除）。closeしたpaneはclosed_duplicatesへ報告
    let b = recon_backend(
        // pass 2のslot cwd（pane_cwd="/w"）とも不一致なinvoked_with: 反転しても
        // 照合が成立しない系列
        fake::FakeRun::Cwd(std::path::PathBuf::from("/other")),
        Some("/w"),
    );
    let mut a = args(false);
    a.json = true;
    let err = with_isolated_xdg(|_| run(&b, &a)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert_eq!(
        override_calls(&b),
        2,
        "反転は高々1回（pass 1 + pass 2のみ・3回目の反転再実行なし）: {:?}",
        b.calls()
    );
    assert!(
        !close_pane_calls(&b).is_empty(),
        "残余の新規spawn paneへのclose-pane: {:?}",
        b.calls()
    );
    let data = err.data().expect("error.data");
    assert!(
        data["closed_duplicates"]
            .as_array()
            .is_some_and(|arr| !arr.is_empty()),
        "残余close実績がclosed_duplicatesに載る: {data}"
    );
}

// [covers:remap-sequence.source-panes-never-closed-by-reconciliation]
#[test]
fn source_panes_never_closed_by_reconciliation() {
    // close対象の限定（DD-12 kill制約との整合）: reconciliationによるclose-paneの
    // 呼出対象は、すべて当該実行開始前snapshotに存在しないid（自spawn複製pane）に
    // 限られる。source pane（snapshot存在id = terminal_1..3）へのclose-paneは
    // reconciliation経路では不発生（成功系・失敗系のいずれでも）
    let b = recon_ok_backend();
    with_isolated_xdg(|_| {
        run(&b, &args(false)).expect("反転で解消し成功する系");
    });
    let closes = close_pane_calls(&b);
    assert!(
        !closes.is_empty(),
        "dup closeが発生して成功する系列（close対象の実在確認）: {:?}",
        b.calls()
    );
    for c in &closes {
        assert!(
            !c.contains("terminal_1") && !c.contains("terminal_2") && !c.contains("terminal_3"),
            "close対象はすべて実行開始前snapshot非存在id: {c}"
        );
    }

    // 失敗系: 検証失敗終了でもsource paneのcloseは不発生
    let panes = vec![
        {
            let mut p = pane_at(1, "cmd", 0, 0, 0, 0, None);
            p.terminal_command = Some("htop".into());
            p.cwd = None;
            p
        },
        pane_at(2, "sh-a", 0, 0, 10, 0, None),
        pane_at(3, "sh-b", 0, 0, 20, 0, None),
    ];
    let b = FakeBackend::new(panes, vec![tab_state(0, 0, "t0", true, 3)]);
    b.set_invoked_with(
        PaneKindId::Terminal(1),
        fake::FakeRun::Command {
            argv: vec!["htop".into()],
            cwd: Some(std::path::PathBuf::from("/w")),
            hold_on_close: true,
            hold_on_start: false,
        },
    );
    let _ = with_isolated_xdg(|_| run(&b, &args(false)));
    for c in close_pane_calls(&b) {
        assert!(
            !c.contains("terminal_1") && !c.contains("terminal_2") && !c.contains("terminal_3"),
            "失敗系でもsource pane idへのclose-paneは不発生: {c}"
        );
    }
}

// [covers:remap-sequence.closed-duplicates-reported-success-and-failure]
#[test]
fn closed_duplicates_reported_success_and_failure() {
    // closed_duplicatesの報告義務（成功・失敗双方・DD-10.9/DD-12）: 当該実行で
    // spawnしcloseした複製paneは、run成功時の報告と検証失敗時のerror.dataの双方で
    // closed_duplicates[]（pane id・対応slot・reason）として報告される（closeして
    // 黙らせない。fake backend側はdata構造の検証を担う）
    let b = recon_ok_backend();
    let mut a = args(false);
    a.json = true;
    let outcome = with_isolated_xdg(|_| run(&b, &a)).expect("反転で解消し成功");
    assert!(
        !outcome.closed_duplicates.is_empty(),
        "成功系のclosed_duplicates報告: {:?}",
        outcome.closed_duplicates
    );
    let cd = &outcome.closed_duplicates[0];
    assert!(
        matches!(cd.pane, PaneKindId::Terminal(n) if n > 3),
        "closeした複製paneのid（snapshot非存在id）: {:?}",
        cd
    );
    assert!(cd.slot < 3, "対応slot: {cd:?}");
    assert!(!cd.reason.is_empty(), "理由: {cd:?}");

    // 失敗系: dup closeが発生したまま検証失敗する系列でも同一構造で報告
    let panes = vec![
        {
            let mut p = pane_at(1, "cmd", 0, 0, 0, 0, None);
            p.terminal_command = Some("htop".into());
            p.cwd = None;
            p
        },
        pane_at(2, "sh-a", 0, 0, 10, 0, None),
        pane_at(3, "sh-b", 0, 0, 20, 0, None),
    ];
    let b = FakeBackend::new(panes, vec![tab_state(0, 0, "t0", true, 3)]);
    b.set_invoked_with(
        PaneKindId::Terminal(1),
        fake::FakeRun::Command {
            argv: vec!["htop".into()],
            cwd: Some(std::path::PathBuf::from("/w")),
            hold_on_close: true,
            hold_on_start: false,
        },
    );
    let err = with_isolated_xdg(|_| run(&b, &a)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    let data = err.data().expect("error.data");
    let arr = data["closed_duplicates"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        !arr.is_empty(),
        "失敗系のerror.data.closed_duplicates報告: {data}"
    );
    assert!(
        arr.iter().all(|e| {
            e.get("pane").is_some() && e.get("slot").is_some() && e.get("reason").is_some()
        }),
        "要素はpane id/slot/reasonを含む: {arr:?}"
    );
}

// [covers:remap-sequence.verify-run-unique-slot-position]
#[test]
fn verify_run_unique_slot_position() {
    // postcondition (e')（TASK-81）: 当該tab内でrunが一意な割当について、適用後の
    // 当該tab visual order s番目のpane id == 割当pane idを検証する（reconciliation
    // 後の「元paneがslotから押し出されたまま」をcount検証では検出できないための
    // 追加検証）。同一run複数pane間は(iii)例外のため位置検証対象外
    // 正常系: run一意なcommand pane（pane 2・derive: terminal_command=Some・
    // pane_cwd=None→slot 1はcwd無しで照合一致）
    let panes = vec![
        pane_at(1, "sh-a", 0, 0, 0, 0, None),
        {
            let mut p = pane_at(2, "cmd", 0, 0, 10, 0, None);
            p.terminal_command = Some("htop".into());
            p.cwd = None;
            p
        },
        pane_at(3, "sh-b", 0, 0, 20, 0, None),
    ];
    let b = FakeBackend::new(panes, vec![tab_state(0, 0, "t0", true, 3)]);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).expect("正常系: 割当paneがslot位置にいる");
    });

    // fatal系: pane 2のinvoked_withのcwdがslotと不一致（押し出され）: 適用後の
    // visual order 1番目が割当pane idと異なるため(e')がVerificationFailedとして
    // 検出する（期待/実測差分に当該slotと期待pane id・実測pane idが載る）
    let panes = vec![
        pane_at(1, "sh-a", 0, 0, 0, 0, None),
        {
            let mut p = pane_at(2, "cmd", 0, 0, 10, 0, None);
            p.terminal_command = Some("htop".into());
            p.cwd = None;
            p
        },
        pane_at(3, "sh-b", 0, 0, 20, 0, None),
    ];
    let b = FakeBackend::new(panes, vec![tab_state(0, 0, "t0", true, 3)]);
    b.set_invoked_with(
        PaneKindId::Terminal(2),
        fake::FakeRun::Command {
            argv: vec!["htop".into()],
            cwd: Some(std::path::PathBuf::from("/w")),
            hold_on_close: true,
            hold_on_start: false,
        },
    );
    let err = with_isolated_xdg(|_| run(&b, &args(false))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert!(
        err.message().contains("terminal_2") && err.message().contains("slot 1"),
        "(e')の期待/実測差分に当該slotと期待pane id・実測pane idが載る: {}",
        err.message()
    );

    // (iii)例外対照: 同一run 2 pane（pane 1/2ともterminal_command=Some("codex exec")）
    // のslot 0/1割当。2 pane間の位置が割当順と入れ替わっていても(e')では検証されずOk
    let panes = vec![
        {
            let mut p = pane_at(1, "same-a", 0, 0, 0, 0, None);
            p.terminal_command = Some("codex exec".into());
            p.cwd = None;
            p
        },
        {
            let mut p = pane_at(2, "same-b", 0, 0, 10, 0, None);
            p.terminal_command = Some("codex exec".into());
            p.cwd = None;
            p
        },
        pane_at(3, "sh", 0, 0, 20, 0, None),
    ];
    let b = FakeBackend::new(panes, vec![tab_state(0, 0, "t0", true, 3)]);
    with_isolated_xdg(|_| {
        run(&b, &args(false)).expect("同一run 2 pane間は(iii)例外で位置検証対象外");
    });
}

// [covers:remap-sequence.verify-assigned-slots-hold-original-ids]
#[test]
fn verify_assigned_slots_hold_original_ids() {
    // postcondition (e'')（TASK-81）: 割当ありslot位置（visual order s番目）に
    // 当該実行でspawnした新規id pane（snapshot非存在id）が残存していないことを検証
    // する（dup close漏れ・反転不成立・押し出しの検出）。空slot位置の新規pane
    // （正規spawn）は対象外
    // 正常系: pane 2(slot 0)・pane 1(slot 1・Run::Cwd系shell・pane_cwd="/w")の2 pane
    // + 3-slot layout（slot 2は空slot）。pass 1のslot 1は照合外れでspawnするが
    // 反転で解消し、空slot位置のspawn paneは(e'')対象外で成功する
    let b = FakeBackend::new(
        vec![pane_at(2, "sh-a", 0, 0, 0, 0, None), {
            let mut p = pane_at(1, "cwd-shell", 0, 0, 50, 0, None);
            p.cwd = Some("/w".into());
            p
        }],
        vec![tab_state(0, 0, "t0", true, 2)],
    );
    b.set_invoked_with(
        PaneKindId::Terminal(1),
        fake::FakeRun::Cwd(std::path::PathBuf::from("/w")),
    );
    with_isolated_xdg(|_| {
        run(&b, &args(false)).expect("空slot位置のspawn paneは検証対象外でOk");
    });

    // fatal系: dup closeが漏れる系列（close-pane呼出は成功扱いだがpaneが残る注入）
    // ——割当ありslot位置に新規id paneが残存するため(e'')がVerificationFailedと
    // して検出する（期待/実測差分に当該slot位置と残存新規idが載る）
    let b = recon_ok_backend();
    b.mute_close_pane();
    let err = with_isolated_xdg(|_| run(&b, &args(false))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert!(
        err.message().contains("terminal_4") && err.message().contains("slot 2"),
        "(e'')の期待/実測差分に当該slot位置と残存新規idが載る: {}",
        err.message()
    );
}

// [covers:remap-sequence.dry-run-floating-scope-relaxed-exit0]
#[test]
fn dry_run_floating_scope_relaxed_exit0() {
    // dry-run floating緩和のrun挙動（TASK-81・破壊的変更: exit 7→0）: selectable
    // floating terminal paneを含むscopeでの--embed-floatingなし--dry-runは
    // Preflight errorとせず、仮想embedで計算した計画（floating込みのM・k）を返し、
    // 警告（embed-floating要求）を出す。toggle-embed等のmutating呼出は不発生。
    // 実行（dry-runなし）はpreflight errorのまま（r8）
    let (mut panes, tabs) = same_tab_three();
    let mut floating = pane_at(9, "float", 0, 0, 20, 0, None);
    floating.is_floating = true;
    panes.push(floating);

    let b = FakeBackend::new(panes, tabs);
    let outcome = with_isolated_xdg(|_| {
        run(&b, &args(true)).expect("dry-runは緩和されPreflight errorにならない")
    });
    let calls = b.calls();
    for banned in [
        "toggle-embed",
        "pipe ",
        "override-layout",
        "new-tab",
        "rename-tab",
        "close-pane",
        "focus-pane-id",
        "go-to-tab",
    ] {
        assert!(
            !calls.iter().any(|c| c.starts_with(banned)),
            "dry-runのmutating呼出不発生（{banned}）: {calls:?}"
        );
    }
    assert!(
        outcome
            .warnings
            .iter()
            .any(|w| w.contains("embed-floating") && w.contains("floating")),
        "警告（embed-floating要求・floating語）がrun結果のwarningsへ出る: {:?}",
        outcome.warnings
    );

    // 対照: 実行（dry-runなし）はpreflight errorのまま（r8）
    let (mut panes, tabs) = same_tab_three();
    let mut floating = pane_at(9, "float", 0, 0, 20, 0, None);
    floating.is_floating = true;
    panes.push(floating);
    let b = FakeBackend::new(panes, tabs);
    let err = run(&b, &args(false)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::Preflight);
}

// [covers:remap-sequence.reconciliation-inverts-despite-order-displacement]
#[test]
fn reconciliation_inverts_despite_order_displacement() {
    // 押し出し割り込み系列（TASK-81 E2E改訂F1・t0p trio再現）: command slot 2つ +
    // shell slot 1つ（Run::Cwd系shell割当）の3-slot構成で、pass 1の未照合残留
    // （割当shell pane）がslot間に割り込む配置注入（fake拡張(i)）により、bare dup
    // spawnのvisual order順位が割当slot位置と異なって現れても、多段規則（bare dup
    // の数突合+属性/一意性判定）により対応づけが成立し反転（close→pane cwd反転→
    // pass 2）が機能する。旧位置規則（visual order s番目==slot s）はこの系列で
    // dup特定不能となり反転不発だった（第1回E2E不具合F1の根拠）
    let panes = vec![
        {
            let mut p = pane_at(1, "cmd-a", 0, 0, 0, 0, None);
            p.terminal_command = Some("htop".into());
            p.cwd = None;
            p
        },
        {
            let mut p = pane_at(2, "cwd-shell", 0, 0, 10, 0, None);
            p.cwd = Some("/w".into());
            p
        },
        {
            let mut p = pane_at(3, "cmd-b", 0, 0, 20, 0, None);
            p.terminal_command = Some("top".into());
            p.cwd = None;
            p
        },
    ];
    let b = FakeBackend::new(panes, vec![tab_state(0, 0, "t0", true, 3)]);
    b.set_invoked_with(
        PaneKindId::Terminal(2),
        fake::FakeRun::Cwd(std::path::PathBuf::from("/w")),
    );
    // 押し出し再現: pass 1でslotに入れなかった割当shell pane 2がvisual order中間
    // （slot 0直後）に割り込み、dup spawn（slot 1位置）の順位がslot 1とずれる
    b.interrupt_leftover_on_override();
    with_isolated_xdg(|_| {
        run(&b, &args(false)).expect("押し出し割り込み下でも反転が機能し成功する");
    });
    let calls = b.calls();
    let closes = close_pane_calls(&b);
    assert_eq!(
        closes.len(),
        1,
        "bare dup 1個のclose（順位ずれでも属性・数突合で特定）: {calls:?}"
    );
    assert!(
        closes[0].contains("terminal_4"),
        "close対象はspawn dup（snapshot非存在id）: {closes:?}"
    );
    assert_eq!(
        override_calls(&b),
        2,
        "close単独で完了扱いにしない（pass 1 + pass 2）: {calls:?}"
    );
    let close_at = calls
        .iter()
        .position(|c| c.starts_with("close-pane "))
        .expect("close-pane");
    let second_override = calls
        .iter()
        .rposition(|c| c.starts_with("override-layout"))
        .expect("pass 2 override");
    assert!(
        close_at < second_override,
        "close -> pass 2 の順: {calls:?}"
    );
    // pass 2後: 割当shell slot 1位置（visual order 1番目）は割当shell pane 2
    let s = b.state.borrow();
    let mut in_tab: Vec<&PaneState> = s
        .panes
        .iter()
        .filter(|p| p.tab_id == ANCHOR && p.is_remap_source())
        .collect();
    in_tab.sort_by_key(|p| (p.geometry.y, p.geometry.x));
    assert_eq!(
        in_tab[1].id,
        PaneKindId::Terminal(2),
        "pass 2後のslot 1位置は割当shell pane: {:?}",
        in_tab.iter().map(|p| p.id).collect::<Vec<_>>()
    );
    assert!(
        in_tab.iter().any(|p| p.id == PaneKindId::Terminal(1))
            && in_tab.iter().any(|p| p.id == PaneKindId::Terminal(3)),
        "source pane（command 2つ）はcloseされない: {:?}",
        in_tab.iter().map(|p| p.id).collect::<Vec<_>>()
    );
}

// [covers:remap-sequence.reconciliation-recovers-held-command-dup]
#[test]
fn reconciliation_recovers_held_command_dup() {
    // 短命command dupのheld化系列（TASK-81 E2E改訂F2・t0q qp8再現）: 照合不成立の
    // command slotから再spawnされたdupが即座にexitしexited/held化する注入
    // （fake拡張(ii)）でも、gate拡張（生きpane数==N_tでもsnapshot非存在の
    // exited/held pane存在で発動）と対象集合拡張（terminal pane全体・exited/held
    // 込み）により、argv一致逆引き（位置不問）で特定してcloseする。command dupは
    // 反転不能のためcloseのみで検証失敗経路へ合流し、closed_duplicatesへ報告
    let panes = vec![
        {
            let mut p = pane_at(1, "cmd", 0, 0, 0, 0, None);
            p.terminal_command = Some("short".into());
            p.cwd = Some("/w".into());
            p
        },
        pane_at(2, "sh-a", 0, 0, 10, 0, None),
        pane_at(3, "sh-b", 0, 0, 20, 0, None),
    ];
    let b = FakeBackend::new(panes, vec![tab_state(0, 0, "t0", true, 3)]);
    // invoked_withのcwd差（/other vs 生成KDL注入cwd=/w）で照合不成立とする
    b.set_invoked_with(
        PaneKindId::Terminal(1),
        fake::FakeRun::Command {
            argv: vec!["short".into()],
            cwd: Some(std::path::PathBuf::from("/other")),
            hold_on_close: true,
            hold_on_start: false,
        },
    );
    b.hold_spawned_command_dup();
    let mut a = args(false);
    a.json = true;
    let err = with_isolated_xdg(|_| run(&b, &a)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    let closes = close_pane_calls(&b);
    assert_eq!(
        closes.len(),
        1,
        "held化dupへのclose-pane（生きpane数==N_tでもreconciliation発動の証明）: {:?}",
        b.calls()
    );
    assert!(
        closes[0].contains("terminal_4"),
        "close対象はheld化したspawn dup: {closes:?}"
    );
    assert_eq!(
        override_calls(&b),
        1,
        "command dupは反転不能のためpass 2のoverride-layoutを行わない: {:?}",
        b.calls()
    );
    let data = err.data().expect("error.data");
    let arr = data["closed_duplicates"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        arr.len(),
        1,
        "close実績がerror.data.closed_duplicatesに載る: {data}"
    );
    assert!(
        arr[0]["pane"]
            .as_str()
            .is_some_and(|p| p.contains("terminal_4")),
        "報告されるpane id: {arr:?}"
    );
    assert!(
        arr[0]["reason"].as_str().is_some_and(|r| !r.is_empty()),
        "理由（command dup経路）が載る: {arr:?}"
    );
}

// [covers:remap-sequence.reconciliation-visual-order-tie-break-resolves-same-cwd-dups]
#[test]
fn reconciliation_visual_order_tie_break_resolves_same_cwd_dups() {
    // 同一cwd shell slot複数+dupの曖昧構成（TASK-81再E2E R1・t0p2 terminal_7再現）:
    // 2-slot構成で割当shell pane 2つともpane_cwd=Some(同一値)のうち、slot 0
    // （None系shell）はpass 1のbare slotと照合成立し、slot 1（Run::Cwd系shell）は
    // 照合不成立でbare slot spawn dup（pane_cwd=クライアントcwd=同一値）がslot 1
    // 参照位置に現れる。属性辺（pane_cwd一致）はdupから両shell slotに立って
    // 完全マッチング2通り（曖昧・u < shell割当数のため数整合shortcutも不適用）の
    // ため、visual order補助辺（exited/held込み順位 == slot s）と両立するマッチング
    // のみ残すtie-breakで対応づけが一意に定まり、反転（close→slot 1のpane cwd反転
    // →pass 2）が機能する。旧実装（補助辺を属性辺への追加辺として用いる）では
    // 曖昧さが解消できず安全側fallback→dup残留で検証失敗になる系列
    let panes = vec![
        pane_at(2, "sh-none", 0, 0, 0, 0, None), // None系shell（visual先頭 → slot 0）
        pane_at(1, "sh-cwd", 0, 0, 50, 0, None), // Run::Cwd系shell（visual 2番目 → slot 1）
    ];
    let b = FakeBackend::new(panes, vec![tab_state(0, 0, "t0", true, 2)]);
    b.set_invoked_with(
        PaneKindId::Terminal(1),
        fake::FakeRun::Cwd(std::path::PathBuf::from("/w")),
    );
    // bare slot spawn dupのpane_cwd（/proc由来の現在cwd=クライアントcwd）注入:
    // 属性照合の辺がdupから両shell slotに立つ（pane_at既定cwd="/w"と同一値）
    b.set_bare_spawn_cwd("/w");
    let mut a = args(false);
    a.inline = Some(TWO_SLOT);
    with_isolated_xdg(|_| {
        run(&b, &a).expect("tie-breakで対応づけ成立し反転で成功する");
    });
    let calls = b.calls();
    let closes = close_pane_calls(&b);
    assert_eq!(
        closes.len(),
        1,
        "bare dup 1個のclose（tie-breakで対応づけ特定）: {calls:?}"
    );
    assert!(
        closes[0].contains("terminal_3"),
        "close対象はspawn dup（snapshot非存在id）: {closes:?}"
    );
    assert_eq!(
        override_calls(&b),
        2,
        "close単独で完了扱いにしない（pass 1 + pass 2）: {calls:?}"
    );
    // close-pane -> pass 2 override の順
    let close_at = calls
        .iter()
        .position(|c| c.starts_with("close-pane "))
        .expect("close-pane");
    let second_override = calls
        .iter()
        .rposition(|c| c.starts_with("override-layout"))
        .expect("pass 2 override");
    assert!(close_at < second_override, "raw: {calls:?}");
    // pass 2後: slot 1位置（visual order 1番目）は割当shell pane（Run::Cwd系）。
    // 成立済みslot 0のpane（None系shell）はcloseされない
    let s = b.state.borrow();
    let mut in_tab: Vec<&PaneState> = s
        .panes
        .iter()
        .filter(|p| p.tab_id == ANCHOR && p.is_remap_source())
        .collect();
    in_tab.sort_by_key(|p| (p.geometry.y, p.geometry.x));
    assert_eq!(
        in_tab[1].id,
        PaneKindId::Terminal(1),
        "pass 2後のslot 1位置は割当shell pane: {:?}",
        in_tab.iter().map(|p| p.id).collect::<Vec<_>>()
    );
    assert!(
        in_tab.iter().any(|p| p.id == PaneKindId::Terminal(2)),
        "成立済みslot 0のpaneはcloseされない: {:?}",
        in_tab.iter().map(|p| p.id).collect::<Vec<_>>()
    );
}

// [covers:remap-sequence.source-excluded-held-panes-in-target-tab-reported]
#[test]
fn source_excluded_held_panes_in_target_tab_reported() {
    // 対象tab内source外held paneの報告（TASK-81再E2E R2・t0s sl1再現）: remap前に
    // exit+heldした短命pane（snapshot存在のためkill制約でclose不可）が対象tab
    // （anchor再利用）内に残留する系列では、leftover_tabs（targets外tab列挙）にも
    // (b)検証count（is_remap_source filter=生きpane集合）にも現れないため、
    // source_excluded_panes（pane id・tab id・exited/held）として報告される。
    // closeも検証失敗化もしない（幾何の巻き込みはDD-10.13の検証対象外）
    //
    // 成功系: live shell 2（3-slot・slot 2は空slot）+ 対象tab内held pane 1
    let panes = vec![
        pane_at(2, "sh-a", 0, 0, 0, 0, None),
        pane_at(1, "sh-b", 0, 0, 10, 0, None),
        {
            let mut p = pane_at(4, "sl1", 0, 0, 20, 0, None); // 短命pane（remap前にexit+held）
            p.exited = true;
            p.is_held = true;
            p
        },
    ];
    let b = FakeBackend::new(panes, vec![tab_state(0, 0, "t0", true, 3)]);
    let outcome =
        with_isolated_xdg(|_| run(&b, &args(false)).expect("生きpane集合の検証はpassし成功"));
    assert_eq!(
        outcome.source_excluded_panes.len(),
        1,
        "対象tab内source外held paneの報告: {:?}",
        outcome.source_excluded_panes
    );
    let sp = &outcome.source_excluded_panes[0];
    assert_eq!(sp.pane, PaneKindId::Terminal(4), "pane id: {sp:?}");
    assert_eq!(sp.tab, ANCHOR, "対象tab（anchor）: {sp:?}");
    assert!(sp.exited && sp.held, "exited/held属性: {sp:?}");
    for c in close_pane_calls(&b) {
        assert!(
            !c.contains("terminal_4"),
            "snapshot存在pane（kill制約対象）はcloseしない: {c}"
        );
    }

    // 失敗系: 検証失敗終了でもerror.data.source_excluded_panesに報告される
    // （command dup close後も(e')差分が残る系列）
    let panes = vec![
        {
            let mut p = pane_at(1, "cmd", 0, 0, 0, 0, None);
            p.terminal_command = Some("htop".into());
            p.cwd = None;
            p
        },
        pane_at(2, "sh-a", 0, 0, 10, 0, None),
        pane_at(3, "sh-b", 0, 0, 20, 0, None),
        {
            let mut p = pane_at(4, "sl1", 0, 0, 30, 0, None);
            p.exited = true;
            p.is_held = true;
            p
        },
    ];
    let b = FakeBackend::new(panes, vec![tab_state(0, 0, "t0", true, 4)]);
    b.set_invoked_with(
        PaneKindId::Terminal(1),
        fake::FakeRun::Command {
            argv: vec!["htop".into()],
            cwd: Some(std::path::PathBuf::from("/w")),
            hold_on_close: true,
            hold_on_start: false,
        },
    );
    let mut a = args(false);
    a.json = true;
    let err = with_isolated_xdg(|_| run(&b, &a)).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    let data = err.data().expect("error.data");
    let arr = data["source_excluded_panes"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        arr.len(),
        1,
        "失敗系でもerror.data.source_excluded_panesへ報告: {data}"
    );
    assert!(
        arr[0]["pane"]
            .as_str()
            .is_some_and(|p| p.contains("terminal_4")),
        "報告されるpane id: {arr:?}"
    );
}

// [covers:remap-sequence.reconciliation-early-return-ignores-earlier-groups]
#[test]
fn reconciliation_early_return_ignores_earlier_group_closes() {
    // reconciliation早期returnの判定scope（TASK-81コードレビューC1）: pass 2なし系列
    // の早期return（当該呼び出しで何もcloseしておらず対応づけ不能もないならgate発動
    // 原因は正規spawn相当として観測をそのまま返す）の判定は、当該reconcile呼び出し内
    // のclose実績で行う。先行groupのclose実績（全group累積のclosed_duplicates）は
    // 本groupの判定に無関係であることの検証: k=2構成で先行group (0,0)がbare dupを
    // close済み（closed累積1件）のとき、後続group (1,0)に「gate発動するがclose対象
    // なし」（terminal_command=Someのexited/held新規pane・argv一致逆引きにもbare突合
    // にも掛からない）の無害系列があっても、reconciliationは問題なしと判定して検証
    // 成功になる（旧実装は累積closed非空で早期returnを抜け、残余close経路の
    // close_budget=0系列が「reconciliation could not resolve ...」noteを生み検証
    // 失敗〔exit 7〕に昇格した）
    let (panes, tabs) = one_tab_four();
    let b = FakeBackend::new(panes, tabs);
    // 先行group (0,0): pane 3（slot 2割当）をRun::Cwd系shellにしてslot 2のbare
    // slotと照合不成立→spawn dup→bare dup close+反転（pass 2）で解消する系列
    b.set_invoked_with(
        PaneKindId::Terminal(3),
        fake::FakeRun::Cwd(std::path::PathBuf::from("/w")),
    );
    // 後続group (1,0)（新規tab id 1・pane 4 slot 0 + 空slot 2つ）: 適用後の当該tabに
    // どの割当argvとも一致しないcommand系のexited/held paneを1つ追加（生きpane数は
    // N_t==3のままgate〔F2条件〕のみ発動する無害系列）
    b.spawn_held_extra_on_override(TabId(1), "ghost");
    let outcome = with_isolated_xdg(|_| {
        run(&b, &args(false)).expect("後続groupの無害系列は早期returnで成功する")
    });
    // closeは先行group (0,0)のbare dup 1件のみ（後続group (1,0)ではcloseしない）
    let closes = close_pane_calls(&b);
    assert_eq!(
        closes.len(),
        1,
        "close-paneは先行groupのdup closeのみ: {:?}",
        b.calls()
    );
    assert!(
        closes[0].contains("terminal_5"),
        "close対象は先行groupのspawn dup（snapshot非存在id・採番5）: {closes:?}"
    );
    assert_eq!(
        outcome.closed_duplicates.len(),
        1,
        "成功dataのclosed_duplicatesも先行groupの1件のみ: {:?}",
        outcome.closed_duplicates
    );
    assert_eq!(
        override_calls(&b),
        3,
        "override-layoutは pass1(0,0) + pass2(0,0) + pass1(1,0) の3回: {:?}",
        b.calls()
    );
}

// ---- TASK-69: --tab TABSPEC絞り込み（DD-10.5/10.6/10.14・D10） ----

/// --tab TABSPEC検証用fixture: tab 0（active・pane 2件+scope外確認用floating 1件）
/// + tab 1 build（非active・pane 3件）
fn two_tabs_target_three() -> (Vec<PaneState>, Vec<TabState>) {
    let mut panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(2, "b", 0, 0, 0, 50, Some("cmd2")),
        pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
        pane_at(4, "d", 1, 1, 0, 50, Some("cmd4")),
        pane_at(5, "e", 1, 1, 10, 0, Some("cmd5")),
    ];
    // scope外（tab 0）のfloating pane（r8のC1回帰と同じ選択的terminal構成）
    let mut floating = pane_at(9, "float", 0, 0, 0, 0, Some("htop"));
    floating.is_floating = true;
    panes.push(floating);
    (
        panes,
        vec![
            tab_state(0, 0, "t0", true, 2),
            tab_state(1, 1, "build", false, 3),
        ],
    )
}

/// floating pane（terminal_9）がtab 0でfloatingのまま生存しているか
fn floating_stays_in_tab0(b: &FakeBackend) -> bool {
    b.state
        .borrow()
        .panes
        .iter()
        .any(|p| p.id == PaneKindId::Terminal(9) && p.is_floating && p.tab_id == TabId(0))
}

// [covers:remap-sequence.tab-spec-scopes-source-and-anchor-to-target-tab]
#[test]
fn tab_spec_scopes_source_and_anchor_to_target_tab() {
    // --tab TABSPEC（ID・一意名とも）: sourceはそのtabのpaneのみでanchorもそのtab
    // （DD-10.5/10.6）。active tab（tab 0）側のpaneはsource外で移動もlayout適用も
    // されず、当該tabはleftover_tabs報告。全sourceがanchor内のためbreak系pipe不使用
    for spec in ["build", "1"] {
        let (panes, tabs) = two_tabs_target_three();
        let b = FakeBackend::new(panes, tabs);
        let mut a = args(false);
        a.tab = Some(spec);
        let outcome = with_isolated_xdg(|_| run(&b, &a).expect("M=3=N（k=1）で成功"));
        let calls = b.calls();
        assert!(
            pipes(&b).is_empty(),
            "全sourceがtab 1（=anchor）内のためprobe・break系とも不使用: {calls:?}"
        );
        assert!(
            !calls.iter().any(|c| c.starts_with("new-tab")),
            "k=1のため新規tab不使用: {calls:?}"
        );
        assert!(
            !calls.iter().any(|c| c.starts_with("rename-tab")),
            "rename不使用: {calls:?}"
        );
        let go_to: Vec<&String> = calls
            .iter()
            .filter(|c| c.starts_with("go-to-tab"))
            .collect();
        assert_eq!(go_to.len(), 2, "step 6のanchor訪問+最終go-to: {calls:?}");
        assert!(
            go_to.iter().all(|c| c.as_str() == "go-to-tab 1"),
            "go-to先はすべて--tab対象tab（anchor切替。active tab 0でない）: {go_to:?}"
        );
        assert_eq!(
            calls
                .iter()
                .filter(|c| c.starts_with("override-layout"))
                .count(),
            1,
            "layout適用は対象tabへの1回のみ: {calls:?}"
        );
        assert_eq!(tiled_count(&b, TabId(1)), 3, "対象tabのpane数は3（N）");
        // 他tab（active tab 0）のpaneはsource外: 移動されず元tabに生存
        let state = b.state.borrow();
        for id in [1u32, 2] {
            let p = state
                .panes
                .iter()
                .find(|p| p.id == PaneKindId::Terminal(id))
                .unwrap_or_else(|| panic!("pane {id} は生存"));
            assert_eq!(p.tab_id, TabId(0), "tab 0に留まる（source外）");
        }
        drop(state);
        // source外tabはleftover_tabsとして報告（closeしない・失敗条件にしない）
        assert_eq!(outcome.leftover_tabs.len(), 1, "raw: {outcome:?}");
        let l = &outcome.leftover_tabs[0];
        assert_eq!(l.id, TabId(0), "raw: {l:?}");
        assert_eq!(l.name, "t0", "raw: {l:?}");
        assert_eq!(l.position, 0, "raw: {l:?}");
        assert_eq!(l.selectable_tiled, 2, "raw: {l:?}");
        // scope外（tab 0）のfloating paneは通常実行のPreflight error対象外
        // （r8のsession全体scopeとは違い--tab絞り込みでscope外になるため）
        assert!(
            !calls.iter().any(|c| c.starts_with("toggle-embed")),
            "scope外floatingはtoggleしない: {calls:?}"
        );
        assert!(
            floating_stays_in_tab0(&b),
            "scope外floatingはfloatingのままtab 0に生存: {:?}",
            b.state.borrow().panes
        );
    }

    // (b) --embed-floating指定時もscope外（tab 0）floatingはtoggle対象外
    // （floating抽出のin_scope判定。r8のC1回帰と対になる--tab側の限定）
    let (panes, tabs) = two_tabs_target_three();
    let b = FakeBackend::new(panes, tabs);
    let mut a = args(false);
    a.tab = Some("build");
    a.embed_floating = true;
    let outcome = with_isolated_xdg(|_| run(&b, &a).expect("scope内floating不在で成功"));
    let calls = b.calls();
    assert!(
        !calls.iter().any(|c| c.starts_with("toggle-embed")),
        "scope外floatingはembed指定でもtoggleしない: {calls:?}"
    );
    assert!(
        floating_stays_in_tab0(&b),
        "scope外floatingはfloatingのまま: {:?}",
        b.state.borrow().panes
    );
    assert_eq!(
        tiled_count(&b, TabId(1)),
        3,
        "sourceはtab 1の3 paneのまま（floating混入なし）"
    );
    assert_eq!(
        outcome.leftover_tabs.len(),
        1,
        "tab 0はleftover: {outcome:?}"
    );
}

// [covers:remap-sequence.tab-spec-unresolvable-is-no-target]
#[test]
fn tab_spec_unresolvable_is_no_target() {
    // --tab解決不能（不在名・不在ID）はNoTargetで状態変更前に中断（DD-10.14・
    // DD-12 preflightの対象解決）。TABSPEC解決の汎用挙動（ID/名前/曖昧の矩阵）は
    // selector.toml既存条件の担当のためここでは重複検証しない（TASK-66/68先例）
    for spec in ["ghost", "99"] {
        let (panes, tabs) = same_tab_three();
        let b = FakeBackend::new(panes, tabs);
        let mut a = args(false);
        a.tab = Some(spec);
        let err = with_isolated_xdg(|_| run(&b, &a).unwrap_err());
        assert_eq!(*err.class(), ErrorClass::NoTarget, "spec={spec}");
        assert!(
            err.message().contains(spec),
            "文面に指定TABSPECが載る: {}",
            err.message()
        );
        let calls = b.calls();
        for op in [
            "pipe",
            "toggle-embed",
            "override-layout",
            "go-to-tab",
            "new-tab",
            "rename-tab",
            "close-pane",
            "focus-pane-id",
        ] {
            assert!(
                !calls.iter().any(|c| c.starts_with(op)),
                "状態変更は一切不発生（{op}）: {calls:?}"
            );
        }
    }
}

// [covers:remap-sequence.tab-spec-applies-first-template-single-tab]
#[test]
fn tab_spec_applies_first_template_single_tab() {
    // --tab時のlayout適用はlayout最初のtab鋳型（鋳型0）による単tab適用（DD-10.5
    // v2.2・D10）: T=2 layout（N_0=2・鋳型1 name=single）を指定してもS=N_0の
    // T=1経路となり、2番目以降のtab鋳型は具体化されない
    let mut a = multi_args(TWO_TAB_LAYOUT);
    a.tab = Some("1");

    // (i) M=2=N_0 → k=1: 鋳型1（--tabなしなら空groupのnew-tab対象）は具体化しない
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
        pane_at(4, "d", 1, 1, 0, 50, Some("cmd4")),
    ];
    let tabs = vec![
        tab_state(0, 0, "t0", true, 1),
        tab_state(1, 1, "build", false, 2),
    ];
    let b = FakeBackend::new(panes, tabs);
    let outcome = with_isolated_xdg(|_| run(&b, &a).expect("M=2=N_0でk=1"));
    let calls = b.calls();
    assert!(pipes(&b).is_empty(), "probe・break系とも不使用: {calls:?}");
    assert!(
        !calls.iter().any(|c| c.starts_with("new-tab")),
        "鋳型1は具体化しない（T=2経路なら空group (0,1) のnew-tab --layout-stringが走る）: {calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.starts_with("rename-tab")),
        "鋳型1名（single）での命名不発生: {calls:?}"
    );
    assert_eq!(
        calls
            .iter()
            .filter(|c| c.starts_with("override-layout"))
            .count(),
        1,
        "layout適用は対象tabへの1回のみ: {calls:?}"
    );
    assert_eq!(tiled_count(&b, TabId(1)), 2, "対象tabのpane数はN_0=2");
    assert_eq!(
        outcome.leftover_tabs.len(),
        1,
        "tab 0はleftover: {outcome:?}"
    );

    // (ii) M=3>N_0 → k=2（S=N_0刻み）: 追加tabはbase-2命名（鋳型1名singleでない）
    let panes = vec![
        pane_at(1, "a", 0, 0, 0, 0, Some("cmd1")),
        pane_at(3, "c", 1, 1, 0, 0, Some("cmd3")),
        pane_at(4, "d", 1, 1, 0, 50, Some("cmd4")),
        pane_at(5, "e", 1, 1, 10, 0, Some("cmd5")),
    ];
    let tabs = vec![
        tab_state(0, 0, "t0", true, 1),
        tab_state(1, 1, "build", false, 3),
    ];
    let b = FakeBackend::new(panes, tabs);
    let outcome = with_isolated_xdg(|_| run(&b, &a).expect("M=3>N_0でk=2"));
    let calls = b.calls();
    assert!(
        calls
            .iter()
            .any(|c| c.starts_with("pipe break-new ") && c.contains("terminal_5")),
        "3番目のpaneがbreak-newで追加tabへ: {calls:?}"
    );
    assert!(
        calls.iter().any(|c| c == "rename-tab 2 remap-2"),
        "追加tab命名はT=1経路のbase-2（--tabなしなら鋳型1名single）: {calls:?}"
    );
    assert!(
        !calls.iter().any(|c| c.contains("single")),
        "鋳型1名での命名・適用は不発生: {calls:?}"
    );
    assert_eq!(tiled_count(&b, TabId(1)), 2, "anchor tab 1はN_0=2 pane");
    assert_eq!(
        tiled_count(&b, TabId(2)),
        2,
        "追加tabもN_0=2 pane（1割当+1空slot充填）"
    );
    assert_eq!(
        outcome.leftover_tabs.len(),
        1,
        "tab 0はleftover: {outcome:?}"
    );
    let state = b.state.borrow();
    let p1 = state
        .panes
        .iter()
        .find(|p| p.id == PaneKindId::Terminal(1))
        .expect("tab 0のpaneは生存");
    assert_eq!(p1.tab_id, TabId(0), "tab 0に留まる（source外）");
}
