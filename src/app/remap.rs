use crate::domain::{LayoutRef, PaneKindId, PaneState, TabId};
use crate::error::{ErrorClass, ZelperError};
use crate::layout;
use crate::layout::generator::{SlotRun, generate_instance_kdl_v2};
use crate::zellij::{LayoutSpec, OverrideSpec, ZellijBackend};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// polling定数（DD-10.7）: 250ms間隔・10s deadline（list-panes再取得で条件判定）
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const POLL_DEADLINE: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// planner（DD-10.6）
// ---------------------------------------------------------------------------

/// remap v2計画（DD-10.6）。対象はsession全pane（selectable・tiled・terminal）。
/// M pane + N slot layout -> k = max(1, ceil(M/N)) instance反復で全paneを配置。
/// 全paneプロセス保存・kill/restartなし（M>N error廃止）。
#[derive(Debug, Clone, PartialEq)]
pub struct V2Plan {
    /// slot数（layout最初のtab subtreeの末端terminal slot数）
    pub n_slots: usize,
    /// source pane数
    pub m: usize,
    /// instance数 = max(1, ceil(M/N))
    pub k: usize,
    /// instance 0のtab（既定 = active tab。--tab指定時はそのtab。DD-10.5/10.6）
    pub anchor: TabId,
    /// instance tab名の接頭（layout名 / --pathのfile stem / "remap"（--inline））
    pub base: String,
    pub instances: Vec<V2InstancePlan>,
    /// preflight warning（DD-10.13: pane_commandに `"` / `\` を含むpane等。実行は可）
    pub warnings: Vec<String>,
}

/// instance jの割当（j=0 は anchor tab。tab名は保持・renameしない。
/// j>=1 は新規tab `<base>-<j+1>`）
#[derive(Debug, Clone, PartialEq)]
pub struct V2InstancePlan {
    pub index: usize,
    pub assignments: Vec<V2Assignment>,
    /// 空slot数（既定shellで埋まる。最終instanceにのみ発生）
    pub empty_slots: usize,
}

/// slot割当1件。全paneがpreserved（kill/restartは存在しないため区別fieldは持たない）
#[derive(Debug, Clone, PartialEq)]
pub struct V2Assignment {
    pub slot: usize,
    pub pane: PaneKindId,
    /// 注入run。pane_commandがSomeならSome(空白分割argv)・None（shell pane）ならNone
    pub run: Option<Vec<String>>,
}

/// v2 planner（DD-10.6）。純粋関数（fake backendなしで検証可能）。
/// sourceはselectable・tiledなterminal pane列（visual order未整列でもよい。
/// plannerが (tab_position, pane_y, pane_x) 昇順に割当順を固定する）。
/// 割当: source visual orderのpane index i（0開始）→ instance j = floor(i/N)・
/// slot = i % N。N=0はLayoutInvalid。
///
/// 決定論性の保証範囲（DD-10.6 (i)〜(iii)）: group（instance/tab）所属と、run
/// （command+args）に一意なpaneのslot対応は決定論的。同一runの複数pane間・複数
/// shell pane（run=None）間のslot順は保証外（zellijのrun一致照合がrun等価なpaneを
/// 互いに区別しないための、要件の決定論性からの明示例外）
pub fn plan_v2(
    source: &[PaneState],
    n_slots: usize,
    anchor: TabId,
    base: &str,
) -> Result<V2Plan, ZelperError> {
    if n_slots == 0 {
        return Err(ZelperError::new(
            ErrorClass::LayoutInvalid,
            "layout has no terminal pane slots",
        ));
    }
    let mut sorted: Vec<&PaneState> = source.iter().collect();
    sorted.sort_by_key(|p| p.visual_key());
    let m = sorted.len();
    let k = m.div_ceil(n_slots).max(1);
    let mut warnings = Vec::new();
    let mut instances = Vec::with_capacity(k);
    for j in 0..k {
        let start = j * n_slots;
        let end = m.min(start + n_slots);
        let mut assignments = Vec::with_capacity(end - start);
        for (slot, p) in sorted[start..end].iter().enumerate() {
            // pane_commandがSomeなら単語1つのshell起動paneでも注入する（DD-10.8）
            let run = p
                .command
                .as_ref()
                .map(|c| c.split_whitespace().map(str::to_string).collect());
            if let Some(cmd) = &p.command
                && needs_quoting_warning(cmd)
            {
                // 空白分割でquoteが復元できずrun一致が外れるためwarning（実行は可。
                // DD-10.13。(b)検証のpane数==Nが重複spawnとして検出する）
                warnings.push(format!(
                    "pane {} pane_command {:?} contains quotes, backslashes, newlines or \
control characters; whitespace split cannot restore quoting and slot matching may miss",
                    p.id.as_spec(),
                    cmd
                ));
            }
            assignments.push(V2Assignment {
                slot,
                pane: p.id,
                run,
            });
        }
        instances.push(V2InstancePlan {
            index: j,
            assignments,
            empty_slots: n_slots - (end - start),
        });
    }
    Ok(V2Plan {
        n_slots,
        m,
        k,
        anchor,
        base: base.to_string(),
        instances,
        warnings,
    })
}

// ---------------------------------------------------------------------------
// 実行（DD-10.7全体）
// ---------------------------------------------------------------------------

/// remap実行引数
pub struct RemapArgs<'a> {
    pub layout: Option<&'a str>,
    pub path: Option<&'a std::path::Path>,
    pub inline: Option<&'a str>,
    pub tab: Option<&'a str>,
    pub embed_floating: bool,
    pub dry_run: bool,
    pub json: bool,
}

/// pane_commandの空白分割でrunが壊れうる文字を含むか（DD-10.13 preflight warning条件。
/// `"`・`'`・`\`・改行・制御文字。実行は可）
fn needs_quoting_warning(cmd: &str) -> bool {
    cmd.contains('"')
        || cmd.contains('\'')
        || cmd.contains('\\')
        || cmd.chars().any(char::is_control)
}

/// nonce連番（実行毎に一意なprobe titleのための後付けエントロピー。DD-10.3）
static NONCE_SEQ: AtomicU64 = AtomicU64::new(0);

fn new_nonce() -> String {
    let seq = NONCE_SEQ.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}-{seq}-{}", std::process::id())
}

/// 権限hint（DD-10.4。probe不成立・移動polling timeout時に付与）
const PERMISSION_HINT: &str = "the companion plugin may lack permissions: the \
permissions.kdl seed may not be picked up by a session started before seeding, the \
permission dialog may be pending, or the plugin protocol may be incompatible \
(proven combination: zellij 0.44.3 + zellij-tile 0.44.3 only). restart the session \
after seeding, or grant permissions to the companion plugin manually, then re-run";

pub fn run(backend: &dyn ZellijBackend, args: &RemapArgs) -> Result<(), ZelperError> {
    let spec = LayoutSpec {
        name: args.layout.map(|s| s.to_string()),
        path: args.path.map(|p| p.to_path_buf()),
        inline: args.inline.map(|s| s.to_string()),
    };
    spec.validate_exclusive()?;
    let layout_ref = match (&spec.name, &spec.path, &spec.inline) {
        (Some(n), _, _) => LayoutRef::Name(n.clone()),
        (_, Some(p), _) => LayoutRef::Path(p.clone()),
        (_, _, Some(s)) => LayoutRef::Inline(s.clone()),
        _ => unreachable!(),
    };

    // 1. snapshot（DD-10.7 step 1。報告・復旧用。dry-runは表示のみのため省略）
    let panes_now = backend.list_panes()?;
    let tabs_now = backend.list_tabs()?;
    let snapshot_len = if args.dry_run {
        0
    } else {
        backend.dump_layout()?.len()
    };

    // 2. anchor id（step 2。list-tabs直前取得・直後に消費。DD-2）
    //    --tab TABSPEC時は対象tabのsource絞り込みとanchorの両方に使う（DD-10.5）
    let anchor = match args.tab {
        Some(raw) => crate::selector::resolve_tab(raw, &tabs_now)?,
        None => backend.current_tab()?.id,
    };

    // 3. layout解決・slot数N（状態変更前に失敗しうる検証をすべて済ませる）
    let kdl_text = layout::load_kdl(&layout_ref)?;
    let doc = layout::parse(&kdl_text)?;
    let base_sub = layout::base_subtree(&doc);
    let n_slots = layout::count_terminal_slots(&base_sub);
    if n_slots == 0 {
        return Err(ZelperError::new(
            ErrorClass::LayoutInvalid,
            "layout has no terminal pane slots",
        ));
    }

    // 4. source（scope = 既定でsession全体。--tab時はそのtab。DD-10.5）
    let in_scope = |p: &PaneState| args.tab.is_none() || p.tab_id == anchor;
    let floating: Vec<PaneState> = panes_now
        .iter()
        .filter(|p| {
            p.is_floating
                && p.is_selectable
                && matches!(p.id, PaneKindId::Terminal(_))
                && in_scope(p)
        })
        .map(|p| (*p).clone())
        .collect();
    if !floating.is_empty() && !args.embed_floating {
        let ids: Vec<String> = floating.iter().map(|p| p.id.as_spec()).collect();
        return Err(ZelperError::with_candidates(
            ErrorClass::Preflight,
            format!(
                "scope has {} floating pane(s) which remap cannot move (unverified for \
break panes); use --embed-floating to convert them to tiled first (processes preserved)",
                floating.len()
            ),
            ids,
        ));
    }
    // --embed-floatingのfloating paneはtiled化「予定」として計画に含める。toggle前の
    // snapshotから仮想的に含めるため、dry-runと実行が同じ計画になる（MR-1/MR-34）
    let mut source: Vec<PaneState> = panes_now
        .iter()
        .filter(|p| {
            in_scope(p)
                && (p.is_remap_source()
                    || (args.embed_floating && floating.iter().any(|f| f.id == p.id)))
        })
        .map(|p| (*p).clone())
        .collect();
    source.sort_by_key(|p| p.visual_key());

    // 5. plan（これも状態変更前）
    let base_name = match &layout_ref {
        LayoutRef::Name(n) => n.clone(),
        LayoutRef::Path(p) => p
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "remap".to_string()),
        LayoutRef::Inline(_) => "remap".to_string(),
    };
    let plan = plan_v2(&source, n_slots, anchor, &base_name)?;
    for w in &plan.warnings {
        eprintln!("warning: {w}");
    }

    // 6. dry-runはplan出力のみで一切の状態変更を行わない（DD-10.10）
    if args.dry_run {
        let kdls = instance_kdls(&plan, &base_sub)?;
        if args.json {
            println!(
                "{}",
                crate::output::json::ok(dry_run_json(&plan, &source, &kdls))
            );
        } else {
            print_dry_run_human(&plan, &source, &kdls, &tabs_now);
        }
        return Ok(());
    }

    execute(
        backend,
        args,
        &plan,
        &source,
        &base_sub,
        snapshot_len,
        &floating,
    )
}

/// instance jの移動前のsource pane所属tab（break-id対象・break-new完了条件に使用）
fn source_tab_of(source: &[PaneState], pane: PaneKindId) -> Option<TabId> {
    source.iter().find(|p| p.id == pane).map(|p| p.tab_id)
}

/// 実行sequenceの移動phase以降（DD-10.7 step 3〜7。preflight完了後の呼び出し）。
/// `floating`: preflightで特定した--embed-floating対象のselectable floating terminal
/// pane列（toggleはこのID集合のみ。probe自動launchのnon-selectableなcompanion plugin
/// paneはDD-10.3/10.5により対象外）
fn execute(
    backend: &dyn ZellijBackend,
    args: &RemapArgs,
    plan: &V2Plan,
    source: &[PaneState],
    base_sub: &kdl::KdlDocument,
    snapshot_len: usize,
    floating: &[PaneState],
) -> Result<(), ZelperError> {
    let anchor = plan.anchor;

    // --- step 3: companion setup（移動が必要な場合のみ。DD-10.3/10.4） ---
    // 移動が不要ならprobeもpluginも不使用（remapはcompanion pluginなしで完結する）
    let group0_outbound: Vec<PaneKindId> = plan.instances[0]
        .assignments
        .iter()
        .filter(|a| source_tab_of(source, a.pane) != Some(anchor))
        .map(|a| a.pane)
        .collect();
    let move_needed = plan.k > 1 || !group0_outbound.is_empty();
    let wasm = if move_needed {
        let path = crate::companion::ensure_companion_wasm()?;
        crate::companion::ensure_permissions(
            &crate::companion::permissions_path()?,
            &path.to_string_lossy(),
            None,
        )?;
        // probe pipe: 状態変更前の応答性・権限確認（設計レビューR3）。
        // probe不成立（timeout）は一切の状態変更前に中断する
        let nonce = new_nonce();
        let want_title = format!("zelper-probe-{nonce}");
        backend.pipe_plugin(&path, "probe", &nonce).map_err(|e| {
            ZelperError::new(
                ErrorClass::OperationFailed,
                format!(
                    "probe pipe did not complete; no session state was changed. {e}. \
{PERMISSION_HINT}"
                ),
            )
        })?;
        poll(
            || {
                backend
                    .list_panes()
                    .map(|ps| ps.iter().any(|p| p.title == want_title))
                    .map(|hit| hit.then_some(()))
            },
            || {
                ZelperError::new(
                    ErrorClass::OperationFailed,
                    format!(
                        "probe not observed within 10s (companion plugin did not respond); \
no session state was changed. {PERMISSION_HINT}"
                    ),
                )
            },
        )?;
        Some(path)
    } else {
        None
    };

    // --- step 4: --embed-floating時のtiled化（最初の状態変更。DD-10.5） ---
    // 対象はpreflightで特定したselectable floating terminal pane（ID基準。DD-2）。
    // probeの自動launchで現れるcompanion plugin pane（non-selectable）はtoggleしない
    for fp in floating {
        backend.toggle_embed_floating(&fp.id)?;
    }

    // --- step 5: 移動phase（plugin pipeはfire-and-forget。効果はpollingで確認） ---
    let mut targets: Vec<TabId> = Vec::with_capacity(plan.k);
    targets.push(anchor);

    // 5-a. group 0のanchor外paneをbreak-idでanchorへ。
    //      group 0のinboundを後続groupのoutboundより先に行う（anchorが一時的に空に
    //      なり自動closeする事態を構造的に排除する不変条件。DD-10.7 5-a）
    if !group0_outbound.is_empty() {
        let ids = group0_outbound
            .iter()
            .map(|p| p.as_spec())
            .collect::<Vec<_>>()
            .join(",");
        let payload = format!("{} {ids}", anchor.0);
        let wasm = wasm.as_deref().expect("move needed implies companion wasm");
        backend
            .pipe_plugin(wasm, "break-id", &payload)
            .map_err(|e| move_pipe_error("break-id", 0, e, plan))?;
        let group0: Vec<PaneKindId> = plan.instances[0]
            .assignments
            .iter()
            .map(|a| a.pane)
            .collect();
        poll(
            || {
                let done = backend
                    .list_panes()?
                    .iter()
                    .filter(|p| group0.contains(&p.id))
                    .all(|p| p.tab_id == anchor);
                Ok(done.then_some(()))
            },
            || move_timeout_error("break-id", 0, plan),
        )?;
    }

    // 5-b. j>=1はbreak-newで新規tabへ。完了条件 = pane id基準のmembership一致
    //     （tab id再利用に耐える特定方法。DD-10.7 5-b）
    for inst in plan.instances.iter().skip(1) {
        let j = inst.index;
        let ids: Vec<PaneKindId> = inst.assignments.iter().map(|a| a.pane).collect();
        // 移動前の所属tab（source snapshot時点）: 完了条件で「移動がまだ」を弁別する
        // ために使う（移動前からmembership一致しているだけのtabを誤認しない）
        let pre_tabs: Vec<Option<TabId>> = ids.iter().map(|p| source_tab_of(source, *p)).collect();
        let payload = ids
            .iter()
            .map(|p| p.as_spec())
            .collect::<Vec<_>>()
            .join(",");
        let wasm = wasm.as_deref().expect("k>1 implies companion wasm");
        backend
            .pipe_plugin(wasm, "break-new", &payload)
            .map_err(|e| move_pipe_error("break-new", j, e, plan))?;
        let target = poll(
            || {
                let panes = backend.list_panes()?;
                let mut tabs: Vec<TabId> = Vec::with_capacity(ids.len());
                for (i, p) in ids.iter().enumerate() {
                    let Some((tab, _)) = q_lookup(&panes, *p) else {
                        return Ok(None); // pane消失はpolling継続（timeoutで報告）
                    };
                    if Some(tab) == pre_tabs[i] {
                        return Ok(None); // まだ移動していない
                    }
                    tabs.push(tab);
                }
                if tabs.iter().any(|t| *t != tabs[0]) {
                    return Ok(None); // 同一tabに集まっていない
                }
                let candidate = tabs[0];
                Ok(membership_matches(&panes, candidate, &ids).then_some(candidate))
            },
            || move_timeout_error("break-new", j, plan),
        )?;
        // 直後にrename-tab-by-idで命名（break-newのname引数は不使用。DD-10.2 #8）
        let name = format!("{}-{}", plan.base, j + 1);
        backend
            .rename_tab(target, &name)
            .map_err(|e| partial_phase(j, "renaming instance tab", e, &targets, plan))?;
        targets.push(target);
    }

    // --- step 6: layout適用phase（生成KDLを各instance tabへ。DD-10.7 6） ---
    let kdls = instance_kdls(plan, base_sub)?;
    for (j, kdl) in kdls.iter().enumerate() {
        backend
            .go_to_tab(targets[j])
            .map_err(|e| partial_phase(j, "focusing instance tab", e, &targets, plan))?;
        backend
            .override_layout(&OverrideSpec {
                source: LayoutSpec {
                    name: None,
                    path: None,
                    inline: Some(kdl.clone()),
                },
                apply_only_to_active_tab: true,
                retain_terminal: true,
                retain_plugin: true,
            })
            .map_err(|e| partial_phase(j, "applying layout", e, &targets, plan))?;
    }

    // --- step 7: 検証 → anchor tabへfocus復帰（best-effort。DD-10.9/10.7 7） ---
    let result = verify(backend, args, plan, &targets, snapshot_len);
    let _ = backend.go_to_tab(anchor); // 復帰失敗は操作失敗に含めない（R42）
    result
}

fn q_lookup(panes: &[PaneState], pane: PaneKindId) -> Option<(TabId, bool)> {
    panes
        .iter()
        .find(|q| q.id == pane)
        .map(|q| (q.tab_id, q.is_remap_source()))
}

/// tabのselectable tiled terminal pane集合 == group（pane id基準のmembership一致）
fn membership_matches(panes: &[PaneState], tab: TabId, ids: &[PaneKindId]) -> bool {
    let in_tab: Vec<PaneKindId> = panes
        .iter()
        .filter(|p| p.tab_id == tab && p.is_remap_source())
        .map(|p| p.id)
        .collect();
    in_tab.len() == ids.len() && ids.iter().all(|p| in_tab.contains(p))
}

/// polling（DD-10.7: 250ms間隔・10s deadline）。条件は最初に評価するため、
/// 即座に成立する場合はsleepしない。成立時に条件値（例: 新規tab id）を返す。
/// deadlineは条件評価（list_panes等のbackend呼出）の所要時間を含めて保証する:
/// 呼出し後にdeadlineを再検査し、超過後の条件成立は成立扱いにしない（遅延実行を
/// 成功と誤認しない）。sleepは残時間を超えない
fn poll<T>(
    mut cond: impl FnMut() -> Result<Option<T>, ZelperError>,
    timeout: impl FnOnce() -> ZelperError,
) -> Result<T, ZelperError> {
    let deadline = Instant::now() + POLL_DEADLINE;
    loop {
        let hit = cond()?;
        if Instant::now() >= deadline {
            return Err(timeout());
        }
        if let Some(v) = hit {
            return Ok(v);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(remaining.min(POLL_INTERVAL));
    }
}

fn move_pipe_error(op: &str, j: usize, e: ZelperError, plan: &V2Plan) -> ZelperError {
    ZelperError::new(
        e.class().clone(),
        format!(
            "move phase failed at '{op}' for group {j}: {}. {}. partial state: moved \
groups 0..{}, not moved: groups {}..{} (no rollback performed; panes survive)",
            e.message(),
            PERMISSION_HINT,
            j.saturating_sub(1),
            j,
            plan.k.saturating_sub(1)
        ),
    )
}

fn move_timeout_error(op: &str, j: usize, plan: &V2Plan) -> ZelperError {
    ZelperError::new(
        ErrorClass::OperationFailed,
        format!(
            "timed out waiting for '{op}' effect on group {j}: moved groups 0..{}, not \
moved: groups {j}..{} (panes survive). note: a piped command may be delayed and still \
execute server-side after the CLI fails, so state may change later. {PERMISSION_HINT}",
            j.saturating_sub(1),
            plan.k.saturating_sub(1)
        ),
    )
}

/// 実行途中失敗時に、実行済み/失敗/未実行の区分をerror messageに載せる（DD-10.11）
fn partial_phase(
    j: usize,
    step: &str,
    e: ZelperError,
    targets: &[TabId],
    plan: &V2Plan,
) -> ZelperError {
    ZelperError::new(
        e.class().clone(),
        format!(
            "remap failed at {step} of instance {j}: {}. partial state: instances 0..={} \
applied (tabs {:?}), failed at instance {j}, not executed: instances {}..{} (no rollback \
performed; panes survive on their assigned tabs, only the layout shape is unapplied)",
            e.message(),
            j.saturating_sub(1),
            targets.iter().map(|t| t.0).collect::<Vec<_>>(),
            j + 1,
            plan.k.saturating_sub(1)
        ),
    )
}

/// 各instanceの生成KDL（DD-10.8。slot -> runの写像を文書順で組み立てる）
fn instance_kdls(plan: &V2Plan, base_sub: &kdl::KdlDocument) -> Result<Vec<String>, ZelperError> {
    plan.instances
        .iter()
        .map(|inst| {
            let runs: Vec<SlotRun> = (0..plan.n_slots)
                .map(|slot| {
                    inst.assignments
                        .iter()
                        .find(|a| a.slot == slot)
                        .and_then(|a| a.run.clone())
                })
                .collect();
            generate_instance_kdl_v2(base_sub, &runs)
        })
        .collect()
}

/// postcondition検証（DD-10.9）。失敗時は単一error（--jsonはmainがerror envelope）
fn verify(
    backend: &dyn ZellijBackend,
    args: &RemapArgs,
    plan: &V2Plan,
    targets: &[TabId],
    snapshot_len: usize,
) -> Result<(), ZelperError> {
    let after = backend.list_panes()?;
    let tabs_after = backend.list_tabs()?;
    let mut mapping: Vec<serde_json::Value> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    // (a) 全source pane idが生存し、割当instanceのtabに所属（pane id基準）
    for (j, inst) in plan.instances.iter().enumerate() {
        let target = targets[j];
        for a in &inst.assignments {
            let found = after.iter().find(|p| p.id == a.pane);
            let ok = found.map(|p| p.tab_id == target).unwrap_or(false);
            mapping.push(serde_json::json!({
                "pane": a.pane.as_spec(), "instance": j, "slot": a.slot,
                "tab": target.0, "preserved": true, "alive": ok,
            }));
            if !ok {
                missing.push(format!(
                    "pane {} expected on instance {j} tab {}, found {}",
                    a.pane.as_spec(),
                    target.0,
                    found
                        .map(|p| format!("tab {}", p.tab_id.0))
                        .unwrap_or_else(|| "gone".to_string())
                ));
            }
        }
        // (b) 各instance tabのselectable tiled terminal pane数 == N
        //     （command照合missによる重複spawn・未照合paneの入れ子残留を検出）
        let count = after
            .iter()
            .filter(|p| p.tab_id == target && p.is_remap_source())
            .count();
        if count != plan.n_slots {
            missing.push(format!(
                "instance {j} tab {}: expected {} selectable tiled panes, found {count}",
                target.0, plan.n_slots
            ));
        }
        // (c) 新規tab名が <base>-<j+1> どおり。target tab自身のIDと名前を対応付けて
        //     検証する（同名tabの存在のみでは競合・後続変更を見逃す。tab id照合は
        //     id再利用回避のため補助のみで、pane id基準の(a)が主検証）
        if j >= 1 {
            let want = format!("{}-{}", plan.base, j + 1);
            match tabs_after.iter().find(|t| t.id == target) {
                Some(t) if t.name == want => {}
                Some(t) => missing.push(format!(
                    "instance {j} tab {} named {:?}, expected {want}",
                    target.0, t.name
                )),
                None => missing.push(format!("instance {j} tab {} missing after remap", target.0)),
            }
        }
    }
    // (c) 続き: tab一覧にanchorが残存
    if !tabs_after.iter().any(|t| t.id == plan.anchor) {
        missing.push(format!("anchor tab {} missing after remap", plan.anchor.0));
    }

    if !missing.is_empty() {
        // --json時は成功envelopeを出さず、mapping/missingをerror.dataに載せた
        // 単一のerror envelopeをmainから出す（stdoutに2つのJSON documentが並ぶのを防ぐ）
        return Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!("remap verification failed: {}", missing.join("; ")),
        )
        .with_data(serde_json::json!({
            "m": plan.m, "n": plan.n_slots, "k": plan.k,
            "mapping": mapping, "missing": missing, "snapshot_len": snapshot_len,
        })));
    }
    if args.json {
        println!(
            "{}",
            crate::output::json::ok(serde_json::json!({
                "m": plan.m, "n": plan.n_slots, "k": plan.k,
                "mapping": mapping,
                "tabs": targets.iter().enumerate().map(|(j, t)| serde_json::json!({
                    "index": j, "id": t.0, "name": if j == 0 {
                        serde_json::Value::Null
                    } else {
                        serde_json::json!(format!("{}-{}", plan.base, j + 1))
                    },
                })).collect::<Vec<_>>(),
                "snapshot_len": snapshot_len,
            }))
        );
    } else {
        println!(
            "remap: {} pane(s) preserved into {} instance(s) of {} slot(s)",
            plan.m, plan.k, plan.n_slots
        );
        for m in &mapping {
            println!("{m}");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// dry-run出力（DD-10.10）
// ---------------------------------------------------------------------------

/// dry-runの--json data（R48/R49: source一覧・M/N/k・割当表・生成KDL preview・操作列）
fn dry_run_json(plan: &V2Plan, source: &[PaneState], kdls: &[String]) -> serde_json::Value {
    serde_json::json!({
        "dry_run": true,
        "m": plan.m, "n": plan.n_slots, "k": plan.k,
        "anchor": plan.anchor.0, "base": plan.base,
        "source": source.iter().map(|p| serde_json::json!({
            "pane": p.id.as_spec(), "title": p.title,
            "tab_id": p.tab_id.0, "tab_name": p.tab_name, "tab_position": p.tab_position,
            "command": p.command,
        })).collect::<Vec<_>>(),
        "instances": plan.instances.iter().zip(kdls).map(|(inst, kdl)| serde_json::json!({
            "index": inst.index,
            "empty_slots": inst.empty_slots,
            "assignments": inst.assignments.iter().map(|a| serde_json::json!({
                "slot": a.slot, "pane": a.pane.as_spec(), "preserved": true,
                "run": a.run.as_ref().map(|v| v.join(" ")).unwrap_or_default(),
            })).collect::<Vec<_>>(),
            "kdl": kdl,
        })).collect::<Vec<_>>(),
        "operations": plan_operations(plan, source),
    })
}

fn print_dry_run_human(
    plan: &V2Plan,
    source: &[PaneState],
    kdls: &[String],
    tabs: &[crate::domain::TabState],
) {
    let anchor_name = tabs
        .iter()
        .find(|t| t.id == plan.anchor)
        .map(|t| t.name.as_str())
        .unwrap_or("?");
    println!(
        "[plan] remap layout base=\"{}\" slots={} panes={} instances={} anchor=tab {} ({})",
        plan.base, plan.n_slots, plan.m, plan.k, plan.anchor.0, anchor_name
    );
    println!("[plan] source (visual order):");
    for p in source {
        println!(
            "[plan]   {} (tab {} \"{}\"){}",
            p.id.as_spec(),
            p.tab_id.0,
            p.tab_name,
            match &p.command {
                Some(c) => format!(" cmd: {c}"),
                None => String::new(),
            }
        );
    }
    for (inst, kdl) in plan.instances.iter().zip(kdls) {
        let tab_desc = if inst.index == 0 {
            format!("anchor {}", anchor_name)
        } else {
            format!("{}-{}", plan.base, inst.index + 1)
        };
        println!("[plan] instance {} (tab: {})", inst.index, tab_desc);
        for a in &inst.assignments {
            println!(
                "[plan]   slot {} <- {} (preserve){}",
                a.slot,
                a.pane.as_spec(),
                match &a.run {
                    Some(argv) => format!(" run: {}", argv.join(" ")),
                    None => String::new(),
                }
            );
        }
        if inst.empty_slots > 0 {
            println!(
                "[plan]   {} empty slot(s) -> default shell",
                inst.empty_slots
            );
        }
        println!("[plan] generated KDL preview (instance {}):", inst.index);
        for line in kdl.lines() {
            println!("[plan]   {line}");
        }
    }
    println!("[plan] planned backend operations:");
    for op in plan_operations(plan, source) {
        println!("[plan]   {op}");
    }
}

/// 実行予定backend操作列（種別表示。DD-10.10 (e)。R50）
fn plan_operations(plan: &V2Plan, source: &[PaneState]) -> Vec<String> {
    let group0_outbound: Vec<String> = plan.instances[0]
        .assignments
        .iter()
        .filter(|a| source_tab_of(source, a.pane) != Some(plan.anchor))
        .map(|a| a.pane.as_spec())
        .collect();
    let mut ops = Vec::new();
    if plan.k > 1 || !group0_outbound.is_empty() {
        ops.push("companion setup (wasm extract + permissions.kdl seed)".to_string());
        ops.push("probe pipe (companion plugin responsiveness check)".to_string());
    }
    if !group0_outbound.is_empty() {
        ops.push(format!(
            "break pipe: break-id {} {}",
            plan.anchor.0,
            group0_outbound.join(",")
        ));
    }
    for inst in plan.instances.iter().skip(1) {
        let ids: Vec<String> = inst.assignments.iter().map(|a| a.pane.as_spec()).collect();
        ops.push(format!("break pipe: break-new {}", ids.join(",")));
        ops.push(format!("rename tab {}-{}", plan.base, inst.index + 1));
    }
    for inst in &plan.instances {
        ops.push(format!(
            "go-to tab + override-layout (instance {})",
            inst.index
        ));
    }
    ops
}
