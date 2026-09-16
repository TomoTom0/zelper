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
    assert!(
        err.message().contains("cwd"),
        "起動cwdが照合不成立の原因となる可能性を案内: {}",
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

    // 空応答がdeadline（10s）を超えて継続する系列は、focus対象決定のpoll
    // timeoutはwarning skip（best-effort）ののち検証冒頭snapshotのtimeoutとして
    // OperationFailed（従来のpolling timeout errorへ合流）
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

    // fatal系: 鋳型1（N_t=1）のtab適用後に重複paneが湧く → 期待/実測差分に当該tabの
    // N_t（expected 1）が載る。v2.2採番では (0,1) がbreak-newでtab 3を得る
    let b = FakeBackend::new(panes, tabs);
    b.spawn_extra_on_override(TabId(3));
    let err = with_isolated_xdg(|_| run(&b, &multi_args(NINE_ONE_TWO))).unwrap_err();
    assert_eq!(*err.class(), ErrorClass::VerificationFailed);
    assert!(
        err.message().contains("expected 1"),
        "鋳型1のN_t=1との差分報告: {}",
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
    // 報告: id/name/position/selectable pane数が載る（closeしない・run成功のまま）
    assert_eq!(leftovers.len(), 1, "raw: {leftovers:?}");
    let l = &leftovers[0];
    assert_eq!(l.id, TabId(1), "raw: {l:?}");
    assert_eq!(l.name, "leftover", "raw: {l:?}");
    assert_eq!(l.position, 1, "raw: {l:?}");
    assert_eq!(l.selectable_tiled, 0, "raw: {l:?}");

    // 対照: leftover不在の構成では報告は空
    let (panes, tabs) = same_tab_three();
    let b = FakeBackend::new(panes, tabs);
    let leftovers = with_isolated_xdg(|_| run(&b, &args(false)).unwrap());
    assert!(leftovers.is_empty(), "raw: {leftovers:?}");
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
