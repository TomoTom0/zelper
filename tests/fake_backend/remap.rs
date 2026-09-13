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
    let _guard = XDG_LOCK.lock().unwrap();
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
