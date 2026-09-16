use crate::domain::{LayoutRef, PaneKindId, PaneState, TabId};
use crate::error::{ErrorClass, ZelperError};
use crate::layout;
use crate::layout::TabTemplate;
use crate::layout::generator::{SlotRun, generate_instance_kdl_v2};
use crate::zellij::{LayoutSpec, NewTabSpec, OverrideSpec, ZellijBackend};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// polling定数（DD-10.7）: 250ms間隔・10s deadline（list-panes再取得で条件判定）
const POLL_INTERVAL: Duration = Duration::from_millis(250);
const POLL_DEADLINE: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// planner（DD-10.6 v2.2）
// ---------------------------------------------------------------------------

/// remap v2.2計画（DD-10.6）。対象はsession全pane（selectable・tiled・terminal）。
/// M pane + S slot（layout全体）→ k = max(1, ceil(M/S)) block × T tabを生成し、
/// tab名（anchor例外）・tab focus・pane focusを再現する。全paneプロセス保存・
/// kill/restartなし（M>N error廃止）
#[derive(Debug, Clone, PartialEq)]
pub struct V2Plan {
    /// 正規形TabTemplate列（T鋳型。plannerはname/focus/n_slots/pane_focus_slotのみ使用）
    pub tabs: Vec<TabTemplate>,
    /// layout全体のslot数 S = sum(N_t)
    pub s_slots: usize,
    /// source pane数
    pub m: usize,
    /// block数 = max(1, ceil(M/S))
    pub k: usize,
    /// 生成tab (0,0)のtab（既定 = active tab。--tab指定時はそのtab。DD-10.5/10.6）
    pub anchor: TabId,
    /// 生成tab名のbase幹（layout名 / --pathのfile stem / "remap"（--inline））
    pub base: String,
    /// k個のblock（各blockはT個のgroup = (b, t)を持つ）
    pub blocks: Vec<V2BlockPlan>,
    /// preflight warning（DD-10.13: terminal_commandに `"` / `\` を含むpane・
    /// 複数focus=true鋳型〔layout解決由来〕等。実行は可）
    pub warnings: Vec<String>,
}

/// block bの計画（groups長 = T。DD-10.6 v2.2）
#[derive(Debug, Clone, PartialEq)]
pub struct V2BlockPlan {
    pub index: usize,
    pub groups: Vec<V2TabPlan>,
}

/// group = (block b, tab t)の計画。(b, t) = (0, 0) はanchor（名前保持・
/// renameしない。D1）。それ以外は新規tab
#[derive(Debug, Clone, PartialEq)]
pub struct V2TabPlan {
    pub block: usize,
    pub tab: usize,
    /// 生成tab名。(0,0)はNone（anchor保持）
    pub name: Option<String>,
    pub assignments: Vec<V2Assignment>,
    /// 空slot数（N_t - 割当数。既定shellで埋まる）
    pub empty_slots: usize,
    /// focus-pane-id対象slot番号（鋳型pane_focus_slot.unwrap_or(0)）。
    /// E2E要因A改訂（DD-10.7 step 6）: 対象pane idは計画時でなく実行時に
    /// 適用後のlist-panes位置（visual order s番目）から決定する
    pub pane_focus_slot: usize,
    /// 最終go-to先（layoutのfocus=true鋳型の文書順最初t*のblock 0 tabのみtrue）
    pub tab_focus_target: bool,
}

/// slot割当1件。全paneがpreserved（kill/restartは存在しないため区別fieldは持たない）
#[derive(Debug, Clone, PartialEq)]
pub struct V2Assignment {
    pub slot: usize,
    pub pane: PaneKindId,
    /// 注入run。terminal_commandがSomeならSome(空白分割argv)・None（shell pane）ならNone
    pub run: Option<Vec<String>>,
}

/// remap後もtargetsに含まれないtab（source外paneのみのtab・companion plugin host
/// tab等）の報告（DD-10.9 leftover_tabs。closeしない。D6）
#[derive(Debug, Clone, PartialEq)]
pub struct LeftoverTab {
    pub id: TabId,
    pub name: String,
    pub position: u32,
    pub selectable_tiled: u32,
    pub selectable_floating: u32,
}

/// v2.2 planner（DD-10.6）。純粋関数（fake backendなしで検証可能）。
/// sourceはselectable・tiledなterminal pane列（visual order未整列でもよい。
/// plannerが (tab_position, pane_y, pane_x) 昇順に割当順を固定する）。
/// 割当: source visual orderのpane index i（0開始）→ block b = floor(i/S)・
/// block内offset o = i%S → 累積slot数 C_t = N_0+...+N_{t-1}（C_0=0）について
/// o in [C_t, C_t+N_t) を満たすtab t・slot s = o - C_t（visual order逐次充填）。
/// k = max(1, ceil(M/S))。N_t=0鋳型・空鋳型列はLayoutInvalid。
///
/// 決定論性の保証範囲（DD-10.6 (i)〜(iii)）: group（block×tab）所属と、run
/// （command+args）に一意なpaneのslot対応は決定論的。同一runの複数pane間・複数
/// shell pane（run=None）間のslot順は保証外（zellijのrun一致照合がrun等価なpaneを
/// 互いに区別しないための、要件の決定論性からの明示例外）
///
/// T=1後方互換（DD-10.6 v2.2・TR75-3）: T=1ではS=N_0=Nであり割当式・k式・命名・
/// tab focus復帰先がv2.1と式レベルで一致する（保証範囲は計画レベルの一致で、
/// focus-pane-id呼出はT=1でも新たに実行される）
pub fn plan_v2(
    source: &[PaneState],
    templates: &[TabTemplate],
    anchor: TabId,
    base: &str,
) -> Result<V2Plan, ZelperError> {
    if templates.is_empty() {
        return Err(ZelperError::new(
            ErrorClass::LayoutInvalid,
            "layout has no terminal pane slots",
        ));
    }
    if templates.iter().any(|t| t.n_slots == 0) {
        // N_t=0鋳型はLayoutInvalid（normalizeでも弾くがplan単体呼出の防御）
        return Err(ZelperError::new(
            ErrorClass::LayoutInvalid,
            "layout has no terminal pane slots",
        ));
    }
    let t_count = templates.len();
    let s_slots: usize = templates.iter().map(|t| t.n_slots).sum();
    let mut sorted: Vec<&PaneState> = source.iter().collect();
    sorted.sort_by_key(|p| p.visual_key());
    let m = sorted.len();
    let k = m.div_ceil(s_slots).max(1);

    // 累積slot列 C_t（C_0 = 0）
    let mut cumulative = Vec::with_capacity(t_count + 1);
    let mut acc = 0usize;
    for tpl in templates {
        cumulative.push(acc);
        acc += tpl.n_slots;
    }

    let mut warnings = Vec::new();
    let mut blocks: Vec<V2BlockPlan> = (0..k)
        .map(|b| V2BlockPlan {
            index: b,
            groups: (0..t_count)
                .map(|t| V2TabPlan {
                    block: b,
                    tab: t,
                    name: generated_tab_name(base, templates, b, t),
                    assignments: Vec::new(),
                    empty_slots: templates[t].n_slots,
                    pane_focus_slot: templates[t].pane_focus_slot.unwrap_or(0),
                    tab_focus_target: false,
                })
                .collect(),
        })
        .collect();

    for (i, p) in sorted.iter().enumerate() {
        let b = i / s_slots;
        let o = i % s_slots;
        let t = cumulative
            .iter()
            .zip(templates)
            .position(|(&c, tpl)| o < c + tpl.n_slots)
            .expect("cumulative covers S");
        let s = o - cumulative[t];
        let run = p
            .terminal_command
            .as_ref()
            .map(|c| c.split_whitespace().map(str::to_string).collect());
        if let Some(cmd) = &p.terminal_command
            && needs_quoting_warning(cmd)
        {
            // 空白分割でquoteが復元できずrun一致が外れるためwarning（実行は可。
            // DD-10.13。(b)検証のpane数==N_tが重複spawnとして検出する）
            warnings.push(format!(
                "pane {} terminal_command {:?} contains quotes, backslashes, newlines or \
control characters; whitespace split cannot restore quoting and slot matching may miss",
                p.id.as_spec(),
                cmd
            ));
        }
        blocks[b].groups[t].assignments.push(V2Assignment {
            slot: s,
            pane: p.id,
            run,
        });
    }

    // empty_slots・tab_focus_target（focus=true鋳型の文書順最初t*のblock 0 tabのみ）。
    // pane_focus_slotは鋳型から直接（対象pane idは実行時に適用後の位置から決定）
    let focus_target_tab = templates.iter().position(|tpl| tpl.focus);
    for (b, block) in blocks.iter_mut().enumerate() {
        for (t, g) in block.groups.iter_mut().enumerate() {
            g.empty_slots = templates[t].n_slots - g.assignments.len();
            g.tab_focus_target = b == 0 && Some(t) == focus_target_tab;
        }
    }

    Ok(V2Plan {
        tabs: templates.to_vec(),
        s_slots,
        m,
        k,
        anchor,
        base: base.to_string(),
        blocks,
        warnings,
    })
}

/// 生成tab (b, t)の名前（DD-10.6 v2.2命名規則）: (b,t)=(0,0)はNone（anchor保持・
/// renameしない・D1）。幹 = T>=2 かつ鋳型tがname属性を持つなら鋳型名、
/// そうでなければbase。b>=1なら幹に-<b+1>を接尾
fn generated_tab_name(base: &str, templates: &[TabTemplate], b: usize, t: usize) -> Option<String> {
    if b == 0 && t == 0 {
        return None;
    }
    let stem = if templates.len() >= 2 && templates[t].name.is_some() {
        templates[t].name.clone().unwrap_or_default()
    } else {
        base.to_string()
    };
    if b >= 1 {
        Some(format!("{stem}-{}", b + 1))
    } else {
        Some(stem)
    }
}

// ---------------------------------------------------------------------------
// 実行（DD-10.7全体・v2.2一般化）
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

/// terminal_commandの空白分割でrunが壊れうる文字を含むか（DD-10.13 preflight warning条件。
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

/// remap実行。戻り値はleftover tab報告（targetsに含まれない残存tab。DD-10.9）
pub fn run(backend: &dyn ZellijBackend, args: &RemapArgs) -> Result<Vec<LeftoverTab>, ZelperError> {
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
    let (panes_now, tabs_now) = snapshot_lenient(backend)?;
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

    // 3. layout解決・正規形TabTemplate列（状態変更前に失敗しうる検証をすべて済ませる。
    //    N_t=0・children置換のLayoutInvalid検証もこの時点〔DD-10.6/10.7前提〕）
    let kdl_text = layout::load_kdl(&layout_ref)?;
    let doc = layout::parse(&kdl_text)?;
    let (mut templates, template_warnings) = layout::normalize_tab_templates(&doc)?;
    // --tab指定時はlayout最初のtab鋳型（鋳型0）による単tab適用（DD-10.5 v2.2・D10）
    if args.tab.is_some() {
        templates.truncate(1);
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
    let mut plan = plan_v2(&source, &templates, anchor, &base_name)?;
    // 複数focus=true鋳型検出warningをplanner warningsへ接続（DD-10.6 v2.2）
    plan.warnings.extend(template_warnings);
    for w in &plan.warnings {
        eprintln!("warning: {w}");
    }

    // 6. 生成KDLも状態変更前に計算する（DD-10.7: 全preflightを完了してから実行）
    let kdls = instance_kdls(&plan)?;

    // dry-runはplan出力のみで一切の状態変更を行わない（DD-10.10）
    if args.dry_run {
        if args.json {
            println!(
                "{}",
                crate::output::json::ok(dry_run_json(&plan, &source, &kdls))
            );
        } else {
            print_dry_run_human(&plan, &source, &kdls, &tabs_now);
        }
        return Ok(Vec::new());
    }

    execute(
        backend,
        args,
        &plan,
        &source,
        &kdls,
        snapshot_len,
        &floating,
    )
}

/// instance jの移動前のsource pane所属tab（break-id対象・break-new完了条件に使用）
fn source_tab_of(source: &[PaneState], pane: PaneKindId) -> Option<TabId> {
    source.iter().find(|p| p.id == pane).map(|p| p.tab_id)
}

/// 実行sequenceの移動phase以降（DD-10.7 step 3〜7 v2.2。preflight完了後の呼び出し）。
/// `floating`: preflightで特定した--embed-floating対象のselectable floating terminal
/// pane列（toggleはこのID集合のみ。probe自動launchのnon-selectableなcompanion plugin
/// paneはDD-10.3/10.5により対象外）
#[allow(clippy::too_many_arguments)]
fn execute(
    backend: &dyn ZellijBackend,
    args: &RemapArgs,
    plan: &V2Plan,
    source: &[PaneState],
    kdls: &[Vec<String>],
    snapshot_len: usize,
    floating: &[PaneState],
) -> Result<Vec<LeftoverTab>, ZelperError> {
    let anchor = plan.anchor;
    let t_count = plan.tabs.len();

    // --- step 3: companion setup（break系移動が必要な場合のみ。DD-10.3/10.4） ---
    // move_needed判定とprobe要否の分離（v2.2）: probe要否は「break系pipeを
    // 使用するか」（割当ありgroup (b,t)!=(0,0)の存在 = break-new、または
    // group (0,0)のanchor外pane = break-id）に分離する。空groupのnew-tabのみで
    // 新規tabを作る構成（新規tabあり・break系なし）はprobeもpluginも不使用
    let group00_outbound: Vec<PaneKindId> = plan.blocks[0].groups[0]
        .assignments
        .iter()
        .filter(|a| source_tab_of(source, a.pane) != Some(anchor))
        .map(|a| a.pane)
        .collect();
    let uses_break_new = plan.blocks.iter().enumerate().any(|(b, block)| {
        block
            .groups
            .iter()
            .enumerate()
            .any(|(t, g)| (b, t) != (0, 0) && !g.assignments.is_empty())
    });
    let uses_break_pipes = plan.k > 1 || uses_break_new || !group00_outbound.is_empty();
    let wasm = if uses_break_pipes {
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
                let Some(panes) = backend.list_panes_lenient()? else {
                    return Ok(None);
                };
                // lenient取得の戻り値で直接判定する。空stdoutは未成立として扱い、
                // 1 tick 1回のbackend呼出でstrict再取得によるfatal窓を作らない。
                Ok(panes.iter().any(|p| p.title == want_title).then_some(()))
            },
            || {
                ZelperError::new(
                    ErrorClass::OperationFailed,
                    format!(
                        "probe not observed within 10s (companion plugin did not respond); \
screen may be busy and its 1s list response timeout may have produced empty output. no \
session state was changed. {PERMISSION_HINT}"
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
    // targets配列（長さk*T・targets[b*T+t]。block昇順tab昇順。DD-10.7 v2.2/TR75-4:
    // 単一execute実行scope内の逐次参照は許容。step 6の各group処理直前に存在確認）
    let mut targets: Vec<TabId> = vec![TabId(0); plan.k * t_count];
    targets[0] = anchor;

    // 5-a. group (0,0)（anchor group）のanchor外paneをbreak-idでanchorへ。
    //      group (0,0)のinboundを後続groupのoutboundより先に行う（anchorが一時的に
    //      空になり自動closeする事態を構造的に排除する不変条件の一般化。
    //      DD-10.7 5-a。M>=1なら逐次充填によりgroup (0,0)は必ず1 pane以上を持つ）
    if !group00_outbound.is_empty() {
        let ids = group00_outbound
            .iter()
            .map(|p| p.as_spec())
            .collect::<Vec<_>>()
            .join(",");
        let payload = format!("{} {ids}", anchor.0);
        let wasm = wasm.as_deref().expect("move needed implies companion wasm");
        backend
            .pipe_plugin(wasm, "break-id", &payload)
            .map_err(|e| move_pipe_error("break-id", 0, e, plan))?;
        let group00: Vec<PaneKindId> = plan.blocks[0].groups[0]
            .assignments
            .iter()
            .map(|a| a.pane)
            .collect();
        poll(
            || {
                let Some(panes) = backend.list_panes_lenient()? else {
                    return Ok(None);
                };
                let done = panes
                    .iter()
                    .filter(|p| group00.contains(&p.id))
                    .all(|p| p.tab_id == anchor);
                Ok(done.then_some(()))
            },
            || move_timeout_error("break-id", 0, plan),
        )?;
    }

    // 5-b. block b昇順・tab t昇順で (0,0) 以外の各groupへ:
    //     割当pane 1件以上: break-new → membership poll → rename-tab-by-id
    //     （割当0件の空group）: new-tab --layout-string <生成KDL>（tab作成時点で
    //     生成KDL適用。E2E要因B改訂）→ stdout id parse → rename-tab-by-id。
    //     new-tab由来idはこのrenameで「取得直後に消費」する（DD-2運用）
    for (b, block) in plan.blocks.iter().enumerate() {
        for (t, g) in block.groups.iter().enumerate() {
            if b == 0 && t == 0 {
                continue;
            }
            let j = b * t_count + t;
            let name = g.name.clone().expect("non-anchor group has a name");
            if g.assignments.is_empty() {
                // 空group: new-tab --layout-string <生成KDL>でtab作成時点で生成KDLを
                // 適用する（E2E要因B改訂・U75-10解決: layout引数なし→override-layout
                // の旧経路は既定pane〔bare shell〕がbare slotに照合されず残存し
                // N_t+1 paneで検証(b)がfailするため不採用。bare slotはtab作成経路
                // でのみ具体化される）
                let id = backend
                    .new_tab(&NewTabSpec {
                        name: None,
                        cwd: None,
                        layout: Some(LayoutSpec {
                            name: None,
                            path: None,
                            inline: Some(kdls[b][t].clone()),
                        }),
                        command: Vec::new(),
                    })
                    .map_err(|e| {
                        partial_phase(
                            j,
                            &format!("creating tab for empty group (b={b},t={t})"),
                            e,
                            &targets,
                            plan,
                        )
                    })?;
                backend
                    .rename_tab(id, &name)
                    .map_err(|e| partial_phase(j, "renaming group tab", e, &targets, plan))?;
                targets[j] = id;
            } else {
                let ids: Vec<PaneKindId> = g.assignments.iter().map(|a| a.pane).collect();
                // 移動前の所属tab（source snapshot時点）: 完了条件で「移動がまだ」を
                // 弁別するために使う（移動前からmembership一致しているだけのtabを
                // 誤認しない）
                let pre_tabs: Vec<Option<TabId>> =
                    ids.iter().map(|p| source_tab_of(source, *p)).collect();
                let payload = ids
                    .iter()
                    .map(|p| p.as_spec())
                    .collect::<Vec<_>>()
                    .join(",");
                let wasm = wasm.as_deref().expect("move needed implies companion wasm");
                backend
                    .pipe_plugin(wasm, "break-new", &payload)
                    .map_err(|e| move_pipe_error("break-new", j, e, plan))?;
                let target = poll(
                    || {
                        let Some(panes) = backend.list_panes_lenient()? else {
                            return Ok(None);
                        };
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
                backend
                    .rename_tab(target, &name)
                    .map_err(|e| partial_phase(j, "renaming group tab", e, &targets, plan))?;
                targets[j] = target;
            }
        }
    }

    // --- step 6: layout適用phase（v2.2: focus-pane-id追加） ---
    // 全group (b,t)をblock昇順・tab昇順で go-to-tab → override-layout（retain 2 flag
    // 常時・他tab保護。空groupはnew-tab --layout-stringで作成時点で生成KDL適用済みの
    // ためskip）→ focus-pane-id（適用後の位置から対象決定。呼出失敗はwarning）。
    // 各group処理直前にlist-tabs（lenient）でtargets[j]の存在確認を行い、不在なら
    // 再解決する（tab id再利用への実行内防御。DD-10.7 v2.2）
    for (b, block) in plan.blocks.iter().enumerate() {
        for (t, g) in block.groups.iter().enumerate() {
            let j = b * t_count + t;
            let tabs = poll(
                || backend.list_tabs_lenient(),
                || {
                    ZelperError::new(
                        ErrorClass::OperationFailed,
                        "timed out listing tabs before layout apply: screen may be busy and \
its 1s list response timeout may have produced empty output",
                    )
                },
            )?;
            if !tabs.iter().any(|x| x.id == targets[j]) {
                targets[j] = resolve_target(backend, plan, b, t, &tabs, &targets)?;
            }
            let target = targets[j];
            backend
                .go_to_tab(target)
                .map_err(|e| partial_phase(j, "focusing group tab", e, &targets, plan))?;
            let empty_group = (b, t) != (0, 0) && g.assignments.is_empty();
            if !empty_group {
                backend
                    .override_layout(&OverrideSpec {
                        source: LayoutSpec {
                            name: None,
                            path: None,
                            inline: Some(kdls[b][t].clone()),
                        },
                        apply_only_to_active_tab: true,
                        retain_terminal: true,
                        retain_plugin: true,
                    })
                    .map_err(|e| partial_phase(j, "applying layout", e, &targets, plan))?;
            }
            // focus-pane-id（E2E要因A改訂）: 対象pane idはmappingでなく適用後の実状態
            // の位置から決定する——当該tabのterminal paneをvisual order（geometry
            // y,x 昇順）に並べたs番目（s = pane_focus_slot or slot 0）。bare pane群の
            // run一致配置は割当順と一致しないためmapping pane id基準だと実幾何とずれる。
            // 空slot（spawn pane）も位置から特定可能なため全生成tabで実行する。
            // 呼出失敗はwarning（継続）し検証(d)（同一の位置基準）で検知する。
            // 「already focused」のexit 2はfocus状態が実際に成立しているため成功扱い
            let focus_slot = g.pane_focus_slot;
            match poll(
                || backend.list_panes_lenient(),
                || {
                    ZelperError::new(
                        ErrorClass::OperationFailed,
                        "timed out listing panes for focus target: screen may be busy and its \
1s list response timeout may have produced empty output",
                    )
                },
            ) {
                Ok(panes) => {
                    let mut in_tab: Vec<&PaneState> = panes
                        .iter()
                        .filter(|p| p.tab_id == target && p.is_remap_source())
                        .collect();
                    in_tab.sort_by_key(|p| (p.geometry.y, p.geometry.x));
                    if let Some(pane) = in_tab.get(focus_slot) {
                        if let Err(e) = backend.focus_pane(&pane.id) {
                            if e.message().contains("already focused") {
                                // focus状態が実際に成立しているため成功扱い
                            } else {
                                eprintln!(
                                    "warning: focus-pane-id {} failed for group \
(b={b},t={t}): {} (continuing; verification will detect an unfocused pane)",
                                    pane.id.as_spec(),
                                    e.message()
                                );
                            }
                        }
                    } else {
                        eprintln!(
                            "warning: focus target slot {focus_slot} not found in tab {} \
for group (b={b},t={t}) (found {} panes); skipping focus, verification will detect a \
mismatch",
                            target.0,
                            in_tab.len()
                        );
                    }
                }
                Err(e) => {
                    // focusはbest-effort: 対象特定のためのlist-panesが取れない場合は
                    // skipし、検証(d)で検知する
                    eprintln!(
                        "warning: could not determine focus target for group (b={b},t={t}): \
{} (continuing)",
                        e.message()
                    );
                }
            }
        }
    }

    // --- step 7: 検証（10.9）→ 最終go-to（best-effort。v2.2） ---
    // layoutにfocus=true鋳型があれば文書順最初t*のblock 0 tab (0,t*)へ、
    // 無ければanchorへ復帰（T=1では前者も (0,0)=anchor と一致し現行挙動と同一）
    let result = verify(backend, args, plan, &targets, snapshot_len);
    let final_target = plan
        .tabs
        .iter()
        .position(|tpl| tpl.focus)
        .map(|t_star| targets[t_star])
        .unwrap_or(anchor);
    let _ = backend.go_to_tab(final_target); // 復帰失敗は操作失敗に含めない（R42）
    result
}

/// targets[j]不在時の再解決（DD-10.7 v2.2/TR75-4）。割当ありgroupはpane所属基準
/// （5-b membership特定と同一）・空groupはrename済み生成tab名基準（list-tabsの
/// name一致）。再解決不能はOperationFailed（中断時報告）
fn resolve_target(
    backend: &dyn ZellijBackend,
    plan: &V2Plan,
    b: usize,
    t: usize,
    tabs: &[crate::domain::TabState],
    targets: &[TabId],
) -> Result<TabId, ZelperError> {
    let g = &plan.blocks[b].groups[t];
    let context = || {
        format!(
            "partial state: generated tabs {:?}, failed at group (b={b},t={t}), no rollback \
performed; panes survive",
            targets.iter().map(|x| x.0).collect::<Vec<_>>()
        )
    };
    if !g.assignments.is_empty() {
        let ids: Vec<PaneKindId> = g.assignments.iter().map(|a| a.pane).collect();
        // 取得はlenient poll（CR75-1）: 再解決時の一時的な空stdout（screen繁忙の1s
        // list応答timeout）で即時失敗しない。DD-10.7/10.9の空応答設計（一時状態は
        // poll継続・deadline規則は他lenient取得と同一）に統一
        let panes = poll(
            || backend.list_panes_lenient(),
            || {
                ZelperError::new(
                    ErrorClass::OperationFailed,
                    "timed out listing panes while re-resolving target tab: screen may be \
busy and its 1s list response timeout may have produced empty output",
                )
            },
        )?;
        let mut tabs_of: Vec<TabId> = Vec::with_capacity(ids.len());
        for p in &ids {
            let Some((tab, _)) = q_lookup(&panes, *p) else {
                return Err(ZelperError::new(
                    ErrorClass::OperationFailed,
                    format!(
                        "cannot re-resolve target tab for group (b={b},t={t}): pane {} not \
found. {}",
                        p.as_spec(),
                        context()
                    ),
                ));
            };
            tabs_of.push(tab);
        }
        if tabs_of.iter().all(|x| *x == tabs_of[0]) && membership_matches(&panes, tabs_of[0], &ids)
        {
            Ok(tabs_of[0])
        } else {
            Err(ZelperError::new(
                ErrorClass::OperationFailed,
                format!(
                    "cannot re-resolve target tab for group (b={b},t={t}): panes are not \
gathered on a single matching tab. {}",
                    context()
                ),
            ))
        }
    } else if let Some(name) = &g.name {
        // 空group: rename済み生成tab名基準（go-to-tab-nameは同名tab曖昧性のため不使用）
        tabs.iter()
            .find(|x| &x.name == name)
            .map(|x| x.id)
            .ok_or_else(|| {
                ZelperError::new(
                    ErrorClass::OperationFailed,
                    format!(
                        "cannot re-resolve target tab for empty group (b={b},t={t}): no tab \
named {name:?}. {}",
                        context()
                    ),
                )
            })
    } else {
        Err(ZelperError::new(
            ErrorClass::OperationFailed,
            format!(
                "anchor tab {} for group (b=0,t=0) is missing before layout apply. {}",
                plan.anchor.0,
                context()
            ),
        ))
    }
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
            plan.k * plan.tabs.len().saturating_sub(j)
        ),
    )
}

fn move_timeout_error(op: &str, j: usize, plan: &V2Plan) -> ZelperError {
    ZelperError::new(
        ErrorClass::OperationFailed,
        format!(
            "timed out waiting for '{op}' effect on group {j}: moved groups 0..{}, not \
moved: groups {j}..{} (panes survive). note: screen may be busy and its 1s list response \
timeout may have produced empty output. a piped command may be delayed and still execute \
server-side after the CLI fails, so state may change later. {PERMISSION_HINT}",
            j.saturating_sub(1),
            plan.k * plan.tabs.len().saturating_sub(1)
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
            "remap failed at {step} of group {j}: {}. partial state: groups 0..={} \
applied (tabs {:?}), failed at group {j}, not executed: groups {}..{} (no rollback \
performed; panes survive on their assigned tabs, only the layout shape is unapplied)",
            e.message(),
            j.saturating_sub(1),
            targets.iter().map(|t| t.0).collect::<Vec<_>>(),
            j + 1,
            plan.k * plan.tabs.len().saturating_sub(1)
        ),
    )
}

/// 各生成tab (b,t)の生成KDL（DD-10.8 v2.2。blocks × tabs。slot -> runの写像を
/// 文書順で組み立てる。baseは当該鋳型の正規形subtree）
fn instance_kdls(plan: &V2Plan) -> Result<Vec<Vec<String>>, ZelperError> {
    (0..plan.k)
        .map(|b| {
            (0..plan.tabs.len())
                .map(|t| {
                    let g = &plan.blocks[b].groups[t];
                    let runs: Vec<SlotRun> = (0..plan.tabs[t].n_slots)
                        .map(|slot| {
                            g.assignments
                                .iter()
                                .find(|a| a.slot == slot)
                                .and_then(|a| a.run.clone())
                        })
                        .collect();
                    generate_instance_kdl_v2(&plan.tabs[t].subtree, &runs)
                })
                .collect()
        })
        .collect()
}

/// postcondition検証（DD-10.9 v2.2）。失敗時は単一error（--jsonはmainがerror envelope）。
/// 成功時はleftover tab報告（targetsに含まれない残存tab。closeしない）を返す
fn verify(
    backend: &dyn ZellijBackend,
    args: &RemapArgs,
    plan: &V2Plan,
    targets: &[TabId],
    snapshot_len: usize,
) -> Result<Vec<LeftoverTab>, ZelperError> {
    let (after, tabs_after) = snapshot_lenient(backend)?;
    let t_count = plan.tabs.len();
    let n_slots: Vec<usize> = plan.tabs.iter().map(|t| t.n_slots).collect();
    let mut mapping: Vec<serde_json::Value> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for b in 0..plan.k {
        for t in 0..t_count {
            let j = b * t_count + t;
            let g = &plan.blocks[b].groups[t];
            let target = targets[j];
            // (a) 全source pane idが生存し、割当group（block×tab）のtabに所属
            //     （pane id基準。tab id基準の照合はid再利用回避のため補助のみ）
            for a in &g.assignments {
                let found = after.iter().find(|p| p.id == a.pane);
                let ok = found.map(|p| p.tab_id == target).unwrap_or(false);
                mapping.push(serde_json::json!({
                    "pane": a.pane.as_spec(), "instance": b, "block": b, "tab_index": t,
                    "slot": a.slot, "tab": target.0, "preserved": true, "alive": ok,
                }));
                if !ok {
                    missing.push(format!(
                        "pane {} expected on group (b={b},t={t}) tab {}, found {}",
                        a.pane.as_spec(),
                        target.0,
                        found
                            .map(|p| format!("tab {}", p.tab_id.0))
                            .unwrap_or_else(|| "gone".to_string())
                    ));
                }
            }
            // (b) 生成tab (b,t)のselectable tiled terminal pane数 == N_t
            //     （anchor (0,0)を含む全生成tabが対象。command照合missによる重複
            //     spawn・未照合paneの入れ子残留を検出）
            let count = after
                .iter()
                .filter(|p| p.tab_id == target && p.is_remap_source())
                .count();
            if count != plan.tabs[t].n_slots {
                missing.push(format!(
                    "tab (b={b},t={t}) id {}: expected {} selectable tiled panes, found \
{count}; pane count mismatch may be caused by the pane's startup cwd preventing run \
matching",
                    target.0, plan.tabs[t].n_slots
                ));
            }
            // (c) 生成tab名が命名規則どおり。（(0,0)=anchorは名前検証から除外——
            //     anchor tab名保持の唯一の例外・D1）。tab id基準で特定する（C5）
            if let Some(want) = &g.name {
                match tabs_after.iter().find(|x| x.id == target) {
                    Some(x) if &x.name == want => {}
                    Some(x) => missing.push(format!(
                        "tab (b={b},t={t}) id {} named {:?}, expected {want}",
                        target.0, x.name
                    )),
                    None => missing.push(format!(
                        "tab (b={b},t={t}) id {} missing after remap",
                        target.0
                    )),
                }
            }
            // (d) pane focus（v2.2・E2E要因A改訂: 位置ベース）: 当該tabのterminal pane
            //     をvisual order（geometry y,x 昇順）に並べたs番目（s =
            //     pane_focus_slot or slot 0）のpaneがis_focused == true。
            //     step 6のfocus対象決定と同一の位置基準。全生成tabが対象
            //     （空slot tab除外は撤廃）
            let focus_slot = g.pane_focus_slot;
            let mut in_tab: Vec<&PaneState> = after
                .iter()
                .filter(|p| p.tab_id == target && p.is_remap_source())
                .collect();
            in_tab.sort_by_key(|p| (p.geometry.y, p.geometry.x));
            match in_tab.get(focus_slot) {
                Some(p) if p.is_focused => {}
                Some(p) => missing.push(format!(
                    "tab (b={b},t={t}) focus slot {focus_slot} pane {} is not focused",
                    p.id.as_spec()
                )),
                None => missing.push(format!(
                    "tab (b={b},t={t}) focus slot {focus_slot} pane missing (found {} \
selectable tiled panes)",
                    in_tab.len()
                )),
            }
        }
    }
    // (c) 続き: tab一覧にanchorが残存
    if !tabs_after.iter().any(|x| x.id == plan.anchor) {
        missing.push(format!("anchor tab {} missing after remap", plan.anchor.0));
    }

    // leftover_tabs報告（v2.2・D6）: targetsに含まれないtabを報告する（closeしない）
    let leftovers: Vec<LeftoverTab> = tabs_after
        .iter()
        .filter(|x| !targets.contains(&x.id))
        .map(|x| LeftoverTab {
            id: x.id,
            name: x.name.clone(),
            position: x.position,
            selectable_tiled: x.selectable_tiled_panes_count,
            selectable_floating: x.selectable_floating_panes_count,
        })
        .collect();

    if !missing.is_empty() {
        // --json時は成功envelopeを出さず、mapping/missingをerror.dataに載せた
        // 単一のerror envelopeをmainから出す（stdoutに2つのJSON documentが並ぶのを防ぐ）
        return Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!("remap verification failed: {}", missing.join("; ")),
        )
        .with_data(serde_json::json!({
            "m": plan.m, "n": plan.s_slots, "k": plan.k, "t": t_count,
            "n_slots": n_slots,
            "mapping": mapping, "missing": missing, "snapshot_len": snapshot_len,
        })));
    }
    if args.json {
        println!(
            "{}",
            crate::output::json::ok(serde_json::json!({
                "m": plan.m, "n": plan.s_slots, "k": plan.k, "t": t_count,
                "s": plan.s_slots, "n_slots": n_slots,
                // preflight warning（複数focus=true鋳型/pane検出・quoting等の
                // 機械可読経路。humanはstderrのwarning行〔--json実行時もstderr〕）
                "warnings": plan.warnings,
                "mapping": mapping,
                "tabs": targets.iter().enumerate().map(|(j, t)| {
                    let (b, ti) = (j / t_count, j % t_count);
                    serde_json::json!({
                        "index": j, "block": b, "tab_index": ti, "id": t.0,
                        "name": plan.blocks[b].groups[ti].name,
                    })
                }).collect::<Vec<_>>(),
                "leftover_tabs": leftovers.iter().map(|l| serde_json::json!({
                    "id": l.id.0, "name": l.name, "position": l.position,
                    "selectable_tiled": l.selectable_tiled,
                    "selectable_floating": l.selectable_floating,
                })).collect::<Vec<_>>(),
                "snapshot_len": snapshot_len,
            }))
        );
    } else {
        println!(
            "remap: {} pane(s) preserved into {} block(s) x {} tab(s) of {} slot(s)",
            plan.m, plan.k, t_count, plan.s_slots
        );
        for m in &mapping {
            println!("{m}");
        }
        for l in &leftovers {
            eprintln!(
                "warning: leftover tab {} {:?} (position {}, {} selectable panes) is not \
part of the remap and was left open; use `zelper remove tab` to close it explicitly",
                l.id.0, l.name, l.position, l.selectable_tiled
            );
        }
    }
    Ok(leftovers)
}

// ---------------------------------------------------------------------------
// dry-run出力（DD-10.10 v2.2拡張）
// ---------------------------------------------------------------------------

/// dry-runの--json data（R48/R49 + v2.2: source一覧・M/S/k/T/N_t・割当表・
/// (b,t)毎の生成KDL preview・操作列）
fn dry_run_json(plan: &V2Plan, source: &[PaneState], kdls: &[Vec<String>]) -> serde_json::Value {
    let t_count = plan.tabs.len();
    serde_json::json!({
        "dry_run": true,
        "m": plan.m, "n": plan.s_slots, "k": plan.k, "t": t_count,
        "s": plan.s_slots,
        "n_slots": plan.tabs.iter().map(|t| t.n_slots).collect::<Vec<_>>(),
        // preflight warning（複数focus=true鋳型/pane検出・quoting等。機械可読経路。
        // humanはstderrのwarning行〔--json実行時もstderr。envelope契約を壊さない〕）
        "warnings": plan.warnings,
        "tabs": plan.tabs.iter().enumerate().map(|(i, t)| serde_json::json!({
            "index": i, "name": t.name, "focus": t.focus,
        })).collect::<Vec<_>>(),
        "anchor": plan.anchor.0, "base": plan.base,
        "source": source.iter().map(|p| serde_json::json!({
            "pane": p.id.as_spec(), "title": p.title,
            "tab_id": p.tab_id.0, "tab_name": p.tab_name, "tab_position": p.tab_position,
            "command": p.command,
            "invoked_with": p.terminal_command,
        })).collect::<Vec<_>>(),
        "instances": (0..plan.k).flat_map(|b| {
            (0..t_count).map(move |t| serde_json::json!({
                "index": b, "block": b, "tab_index": t,
                "name": plan.blocks[b].groups[t].name,
                "empty_slots": plan.blocks[b].groups[t].empty_slots,
                "assignments": plan.blocks[b].groups[t].assignments.iter().map(|a| serde_json::json!({
                    "slot": a.slot, "tab_index": t, "pane": a.pane.as_spec(),
                    "preserved": true,
                    "run": a.run.as_ref().map(|v| v.join(" ")).unwrap_or_default(),
                })).collect::<Vec<_>>(),
                "kdl": kdls[b][t],
            }))
        }).collect::<Vec<_>>(),
        "operations": plan_operations(plan, source),
    })
}

fn print_dry_run_human(
    plan: &V2Plan,
    source: &[PaneState],
    kdls: &[Vec<String>],
    tabs: &[crate::domain::TabState],
) {
    let t_count = plan.tabs.len();
    let anchor_name = tabs
        .iter()
        .find(|t| t.id == plan.anchor)
        .map(|t| t.name.as_str())
        .unwrap_or("?");
    let n_slots_csv = plan
        .tabs
        .iter()
        .map(|t| t.n_slots.to_string())
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "[plan] remap layout base=\"{}\" slots={} panes={} blocks={} tabs={} \
per-tab-slots={} anchor=tab {} ({})",
        plan.base, plan.s_slots, plan.m, plan.k, t_count, n_slots_csv, plan.anchor.0, anchor_name
    );
    println!("[plan] source (visual order):");
    for p in source {
        println!(
            "[plan]   {} (tab {} \"{}\"){}",
            p.id.as_spec(),
            p.tab_id.0,
            p.tab_name,
            match &p.terminal_command {
                Some(c) => format!(" inv: {c}"),
                None => String::new(),
            }
        );
    }
    for (b, block) in plan.blocks.iter().enumerate() {
        for (t, g) in block.groups.iter().enumerate() {
            let tab_desc = match &g.name {
                None => format!("anchor {}", anchor_name),
                Some(name) => name.clone(),
            };
            let focus_desc = if g.tab_focus_target {
                " (tab focus target)"
            } else {
                ""
            };
            println!("[plan] block {b} tab {t} (tab: {tab_desc}){focus_desc}");
            for a in &g.assignments {
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
            if g.empty_slots > 0 {
                println!("[plan]   {} empty slot(s) -> default shell", g.empty_slots);
            }
            println!("[plan] generated KDL preview (block {b}, tab {t}):");
            for line in kdls[b][t].lines() {
                println!("[plan]   {line}");
            }
        }
    }
    println!("[plan] planned backend operations:");
    for op in plan_operations(plan, source) {
        println!("[plan]   {op}");
    }
}

/// 空stdoutをpoll継続扱いにするsnapshot取得（DD-10.7）。非空parse失敗と
/// backend errorはそのままfatal、空がdeadlineまで続く場合はphase付きerror。
fn snapshot_lenient(
    backend: &dyn ZellijBackend,
) -> Result<(Vec<PaneState>, Vec<crate::domain::TabState>), ZelperError> {
    poll(
        || {
            let panes = backend.list_panes_lenient()?;
            let tabs = backend.list_tabs_lenient()?;
            match (panes, tabs) {
                (Some(panes), Some(tabs)) => Ok(Some((panes, tabs))),
                _ => Ok(None),
            }
        },
        || {
            ZelperError::new(
                ErrorClass::OperationFailed,
                "timed out obtaining remap snapshot: screen may be busy and its 1s list response timeout may have produced empty output",
            )
        },
    )
}

/// 実行予定backend操作列（種別表示。DD-10.10 (e) v2.2。R50）:
/// companion setup・probe pipe・break pipe（break-id/break-new）・new-tab（空group）・
/// rename・go-to・override・focus-pane-id・最終go-to先を種別表示
fn plan_operations(plan: &V2Plan, source: &[PaneState]) -> Vec<String> {
    let t_count = plan.tabs.len();
    let group00_outbound: Vec<String> = plan.blocks[0].groups[0]
        .assignments
        .iter()
        .filter(|a| source_tab_of(source, a.pane) != Some(plan.anchor))
        .map(|a| a.pane.as_spec())
        .collect();
    // probe要否は「break系を使用するか」に分離（v2.2: 空groupのnew-tabのみの
    // 構成はcompanion pluginなしで完結する）
    let uses_break_new = plan.blocks.iter().enumerate().any(|(b, block)| {
        block
            .groups
            .iter()
            .enumerate()
            .any(|(t, g)| (b, t) != (0, 0) && !g.assignments.is_empty())
    });
    let uses_break_pipes = plan.k > 1 || uses_break_new || !group00_outbound.is_empty();
    let mut ops = Vec::new();
    if uses_break_pipes {
        ops.push("companion setup (wasm extract + permissions.kdl seed)".to_string());
        ops.push("probe pipe (companion plugin responsiveness check)".to_string());
    }
    if !group00_outbound.is_empty() {
        ops.push(format!(
            "break pipe: break-id {} {}",
            plan.anchor.0,
            group00_outbound.join(",")
        ));
    }
    for b in 0..plan.k {
        for t in 0..t_count {
            if b == 0 && t == 0 {
                continue;
            }
            let g = &plan.blocks[b].groups[t];
            let name = g.name.clone().expect("non-anchor group has a name");
            if g.assignments.is_empty() {
                ops.push(format!(
                    "new-tab --layout-string (empty group b={b},t={t}) + rename tab {name}"
                ));
            } else {
                let ids: Vec<String> = g.assignments.iter().map(|a| a.pane.as_spec()).collect();
                ops.push(format!("break pipe: break-new {}", ids.join(",")));
                ops.push(format!("rename tab {name}"));
            }
        }
    }
    for b in 0..plan.k {
        for t in 0..t_count {
            let g = &plan.blocks[b].groups[t];
            let empty_group = (b, t) != (0, 0) && g.assignments.is_empty();
            if empty_group {
                // 空groupはnew-tab --layout-stringで作成時点で生成KDL適用済み
                ops.push(format!(
                    "go-to tab (block {b}, tab {t}; layout applied via new-tab)"
                ));
            } else {
                ops.push(format!("go-to tab + override-layout (block {b}, tab {t})"));
            }
            // focus-pane-id対象pane idは実行時に適用後の位置から決定するため
            // 種別表示はslot番号で示す（E2E要因A改訂）
            ops.push(format!(
                "focus-pane-id (block {b}, tab {t}, visual slot {})",
                g.pane_focus_slot
            ));
        }
    }
    // 最終go-to先（v2.2）: focus=true鋳型の文書順最初t*のblock 0 tab・無ければanchor
    match plan.tabs.iter().position(|tpl| tpl.focus) {
        Some(t_star) => ops.push(format!(
            "final go-to tab (layout focus -> block 0 tab {t_star})"
        )),
        None => ops.push(format!(
            "final go-to tab (anchor restore -> tab {})",
            plan.anchor.0
        )),
    }
    ops
}
