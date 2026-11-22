use crate::companion::{PERMISSION_HINT, break_id_payload, poll, setup_and_probe};
use crate::domain::{LayoutRef, PaneKindId, PaneState, TabId};
use crate::error::{ErrorClass, ZelperError};
use crate::layout;
use crate::layout::TabTemplate;
use crate::layout::generator::{RunSpec, SlotRun, generate_instance_kdl_v2};
use crate::zellij::{LayoutSpec, NewTabSpec, OverrideSpec, ZellijBackend};
use std::collections::HashSet;

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
    /// 注入run。terminal_commandがSomeならSome(RunSpec)（argv=空白分割。
    /// cwd=pane_cwd由來——TASK-81本体実装で算出）・None（shell pane）ならNone
    pub run: Option<RunSpec>,
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

/// 当該実行でspawnしcloseした複製paneの報告（DD-10.9 closed_duplicates・DD-12。
/// close実績は成功/失敗双方で報告する報告義務。収集はTASK-81本体実装
/// 〔reconciliation〕で行う）
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedDuplicate {
    pub pane: PaneKindId,
    pub slot: usize,
    pub reason: String,
}

/// remap成功（dry-run含む）時の報告（DD-10.9）。leftover_tabs: targets外の残存tab。
/// closed_duplicates: 自spawn複製paneのclose実績。warnings: preflight warning
/// （dry-run floating緩和警告等。TASK-81本体実装で収集）。
/// source_excluded_panes: 対象tab内に残留するsource外exited/held paneの報告
/// （TASK-81再E2E R2）
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RemapOutcome {
    pub leftover_tabs: Vec<LeftoverTab>,
    pub closed_duplicates: Vec<ClosedDuplicate>,
    pub warnings: Vec<String>,
    pub source_excluded_panes: Vec<SourceExcludedPane>,
}

/// 対象tab（targets内）に残留するsource外paneの報告（DD-10.9 対象tab内source外
/// pane報告・TASK-81再E2E R2）: 実行開始前snapshotに存在するがexited/heldのため
/// source選定から外れたterminal pane（DD-10.5の対象外規則により元tab/対象tabに
/// 残留。t0s sl1）。leftover_tabs（targets外tab）にも生きpane数検証（(b)の
/// is_remap_source filter）にも現れないため報告する。kill制約（DD-12）のため
/// closeはしない（報告のみ）
#[derive(Debug, Clone, PartialEq)]
pub struct SourceExcludedPane {
    pub pane: PaneKindId,
    pub tab: TabId,
    pub title: String,
    pub exited: bool,
    pub held: bool,
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
        let run = p.terminal_command.as_ref().map(|c| RunSpec {
            argv: c.split_whitespace().map(str::to_string).collect(),
            // TASK-81: cwd = pane_cwd（pane.cwd）由來（DD-10.8: `zellij run`起因paneは
            // invoked_withのcwd=Someが常に成立し無cwdslotと照合しないため注入する）。
            // pane_cwd=Noneは注入skip+warning（10.13: cwd enrichの100ms timeout欠落等）
            cwd: p.cwd.clone(),
        });
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
        if let Some(cmd) = &p.terminal_command
            && needs_shell_c_warning(cmd)
        {
            // shell -c高疑いheuristics warning（DD-10.13 TASK-81拡張）: arg境界はjoinで
            // 消失しており、quote文字を含まない`bash -c "echo hello world"`のような
            // 引数も誤argv化するため文字種検出（上記）だけでは拾えない（実行は可）
            warnings.push(format!(
                "pane {} terminal_command {:?} looks like a shell -c invocation (argv[0] is \
a shell and argv[1] is -c); argument boundaries cannot be restored by whitespace split and \
slot matching may miss",
                p.id.as_spec(),
                cmd
            ));
        }
        if let Some(cmd) = &p.terminal_command
            && p.cwd.is_none()
        {
            // pane_cwd欠落（DD-10.8/10.13）: cwd注入をskipする（生成KDLはcwd属性なし）。
            // 照合が外れうることは(b)検証でVerificationFailedとして検出される
            warnings.push(format!(
                "pane {} terminal_command {:?} has no pane_cwd (list-panes cwd enrichment \
may have timed out); skipping cwd injection for its slot and run matching may miss",
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

/// terminal_commandがshell -c起動の高疑いか（DD-10.13 TASK-81拡張のheuristics:
/// 空白分割argv[0]がshell系（*sh / bash / zsh / fish等）かつargv[1]が`-c`）。
/// quote文字を含まなくてもarg境界はjoinで消失しているため誤argv化の高疑い
fn needs_shell_c_warning(cmd: &str) -> bool {
    let mut words = cmd.split_whitespace();
    let Some(argv0) = words.next() else {
        return false;
    };
    let base = argv0.rsplit('/').next().unwrap_or(argv0);
    let is_shell = base == "fish" || base.ends_with("sh");
    is_shell && words.next() == Some("-c")
}

/// remap実行。戻り値は成功時の報告（leftover tab報告・closed_duplicates・warnings。
/// DD-10.9）
pub fn run(backend: &dyn ZellijBackend, args: &RemapArgs) -> Result<RemapOutcome, ZelperError> {
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
    // exited/heldはis_remap_source()と同様に対象外（run一致照合できず、toggle後も
    // membership検証に落るため元tabに残す。DD-10.5）
    let floating: Vec<PaneState> = panes_now
        .iter()
        .filter(|p| {
            p.is_floating
                && p.is_selectable
                && !p.exited
                && !p.is_held
                && matches!(p.id, PaneKindId::Terminal(_))
                && in_scope(p)
        })
        .map(|p| (*p).clone())
        .collect();
    // dry-runのfloating保有scope緩和（DD-10.10 TASK-81・破壊的変更: exit 7→0）:
    // dry-runは仮想embed（--embed-floating dry-runと同一logic）で計算した計画を
    // 表示し警告を出してexit 0。実行（dry-run以外）は10.5のとおりpreflight error
    let mut floating_relax_warning: Option<String> = None;
    if !floating.is_empty() && !args.embed_floating {
        if args.dry_run {
            floating_relax_warning = Some(format!(
                "scope has {} floating pane(s); this dry-run plan virtually embeds them, \
but execution requires --embed-floating (floating panes are tiled and included, processes \
preserved)",
                floating.len()
            ));
        } else {
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
    }
    // --embed-floating（および緩和されたdry-run）のfloating paneはtiled化「予定」と
    // して計画に含める。toggle前のsnapshotから仮想的に含めるため、dry-runと実行が
    // 同じ計画になる（MR-1/MR-34）
    let virtual_embed = args.embed_floating || args.dry_run;
    let mut source: Vec<PaneState> = panes_now
        .iter()
        .filter(|p| {
            in_scope(p)
                && (p.is_remap_source() || (virtual_embed && floating.iter().any(|f| f.id == p.id)))
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
    if let Some(w) = floating_relax_warning {
        plan.warnings.push(w);
    }
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
        return Ok(RemapOutcome {
            warnings: plan.warnings.clone(),
            ..Default::default()
        });
    }

    // 実行開始前snapshotのpane id集合（DD-12 TASK-81: dup close対象の限定——
    // snapshot存在paneはkill制約の対象のためcloseしない——と(e'')の新規id判定）
    let pre_ids: HashSet<PaneKindId> = panes_now.iter().map(|p| p.id).collect();

    execute(
        backend,
        args,
        &plan,
        &source,
        &kdls,
        snapshot_len,
        &floating,
        &pre_ids,
    )
}

/// instance jの移動前のsource pane所属tab（break-id対象・break-new完了条件に使用）
fn source_tab_of(source: &[PaneState], pane: PaneKindId) -> Option<TabId> {
    source.iter().find(|p| p.id == pane).map(|p| p.tab_id)
}

/// executeの入口（中断時報告の付与。TASK-105・PR #10レビュー対応）。
/// closed_duplicatesの報告義務（DD-10.9/DD-12）は成功・失敗双方に及ぶが、
/// 検証失敗のerror.dataはverifyが構成する一方、適用phaseの中断エラー
/// （partial_phase等）はverifyを経由しないため、蓄積済みのclose実績が
/// error.dataへ載らない経路があった。ここでclose実績の蓄積先を保持し、
/// data未付与のエラーのまま戻る場合にclosed_duplicatesを付与する
/// （dataを既に持つ検証失敗エラーはそのまま返す）。
#[allow(clippy::too_many_arguments)]
fn execute(
    backend: &dyn ZellijBackend,
    args: &RemapArgs,
    plan: &V2Plan,
    source: &[PaneState],
    kdls: &[Vec<String>],
    snapshot_len: usize,
    floating: &[PaneState],
    pre_ids: &HashSet<PaneKindId>,
) -> Result<RemapOutcome, ZelperError> {
    let mut closed: Vec<ClosedDuplicate> = Vec::new();
    let result = execute_body(
        backend,
        args,
        plan,
        source,
        kdls,
        snapshot_len,
        floating,
        pre_ids,
        &mut closed,
    );
    match result {
        Err(e) if e.data().is_none() && !closed.is_empty() => Err(e.with_data(
            serde_json::json!({ "closed_duplicates": closed_duplicates_json(&closed) }),
        )),
        other => other,
    }
}

/// 実行sequenceの移動phase以降（DD-10.7 step 3〜7 v2.2。preflight完了後の呼び出し）。
/// `floating`: preflightで特定した--embed-floating対象のselectable floating terminal
/// pane列（toggleはこのID集合のみ。probe自動launchのnon-selectableなcompanion plugin
/// paneはDD-10.3/10.5により対象外）。`pre_ids`: 実行開始前snapshotのpane id集合
/// （reconciliationのdup close対象限定と検証(e'')の新規id判定。DD-12/10.9 TASK-81）
#[allow(clippy::too_many_arguments)]
fn execute_body(
    backend: &dyn ZellijBackend,
    args: &RemapArgs,
    plan: &V2Plan,
    source: &[PaneState],
    kdls: &[Vec<String>],
    snapshot_len: usize,
    floating: &[PaneState],
    pre_ids: &HashSet<PaneKindId>,
    closed: &mut Vec<ClosedDuplicate>,
) -> Result<RemapOutcome, ZelperError> {
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
        // companion setup + probe pipe（状態変更前の応答性・権限確認。設計レビューR3。
        // helper切り出し: src/companion.rs。TASK-88設計§2.8）
        Some(setup_and_probe(backend)?)
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
        let payload = break_id_payload(anchor, &group00_outbound);
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

    // --- step 6: layout適用phase（v2.2: focus-pane-id追加・TASK-81: reconciliation） ---
    // 全group (b,t)をblock昇順・tab昇順で go-to-tab → override-layout（retain 2 flag
    // 常時・他tab保護。空groupはnew-tab --layout-stringで作成時点で生成KDL適用済みの
    // ためskip）→ 照合不成立のreconciliation（安定観測→位置規則→dup close→反転
    // pass 2→残余close。DD-10.7 step 6 TASK-81）→ focus-pane-id（reconciliation後の
    // 位置から対象決定。呼出失敗はwarning）。各group処理直前にlist-tabs（lenient）で
    // targets[j]の存在確認を行い、不在なら再解決する（tab id再利用への実行内防御）
    let mut recon_notes: Vec<String> = Vec::new();
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
            // 適用後観測（reconciliation gateとfocus対象決定の共通取得）: 1回の
            // list-panes（lenient・空応答は再試行）でreconciliation発動条件を判定
            // する。TASK-81 E2E改訂F2: 生きpane数 > N_t に加え、生きpane数 == N_t
            // でも当該tab内に実行開始前snapshot非存在のexited/held paneが存在する
            // なら発動する（短命command dupのheld化が生きpane集合から外れて過剰
            // 検出から逃避するのを防ぐ。正規spawn〔空slot由来〕のheld化との弁別は
            // reconcile内の対応規則が行う——空slot相当の新規paneはclose対象外）
            let n_t = plan.tabs[t].n_slots;
            let observed = poll(
                || backend.list_panes_lenient(),
                || {
                    ZelperError::new(
                        ErrorClass::OperationFailed,
                        "timed out listing panes after layout apply: screen may be busy and \
its 1s list response timeout may have produced empty output",
                    )
                },
            )?;
            let excess = !empty_group
                && (observed
                    .iter()
                    .filter(|p| p.tab_id == target && p.is_remap_source())
                    .count()
                    > n_t
                    || observed.iter().any(|p| {
                        p.tab_id == target
                            && p.is_selectable
                            && !p.is_floating
                            && matches!(p.id, PaneKindId::Terminal(_))
                            && (p.exited || p.is_held)
                            && !pre_ids.contains(&p.id)
                    }));
            let observed = if excess {
                let (final_obs, note) = reconcile(
                    backend, plan, b, t, target, observed, pre_ids, j, &targets, closed,
                )?;
                if let Some(note) = note {
                    recon_notes.push(note);
                }
                final_obs
            } else {
                observed
            };
            // focus-pane-id（E2E要因A改訂）: 対象pane idはmappingでなく適用後の実状態
            // （reconciliation後）の位置から決定する——当該tabのterminal paneをvisual
            // order（geometry y,x 昇順）に並べたs番目（s = pane_focus_slot or slot 0）。
            // bare pane群のrun一致配置は割当順と一致しないためmapping pane id基準だと
            // 実幾何とずれる。空slot（spawn pane）も位置から特定可能なため全生成tabで
            // 実行する。呼出失敗はwarning（継続）し検証(d)（同一の位置基準）で検知する。
            // 「already focused」のexit 2はfocus状態が実際に成立しているため成功扱い
            let focus_slot = g.pane_focus_slot;
            let mut in_tab: Vec<&PaneState> = observed
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
    }

    // --- step 7: 検証（10.9）→ 最終go-to（best-effort。v2.2） ---
    // layoutにfocus=true鋳型があれば文書順最初t*のblock 0 tab (0,t*)へ、
    // 無ければanchorへ復帰（T=1では前者も (0,0)=anchor と一致し現行挙動と同一）
    let result = verify(
        backend,
        args,
        plan,
        &targets,
        snapshot_len,
        pre_ids,
        &*closed,
        &recon_notes,
    );
    let final_target = plan
        .tabs
        .iter()
        .position(|tpl| tpl.focus)
        .map(|t_star| targets[t_star])
        .unwrap_or(anchor);
    let _ = backend.go_to_tab(final_target); // 復帰失敗は操作失敗に含めない（R42）
    result
}

// ---------------------------------------------------------------------------
// 照合不成立のreconciliation（DD-10.7 step 6 TASK-81）
// ---------------------------------------------------------------------------

/// 当該tabのterminal pane全体（exited / is_held含む・selectable tiled）のid集合
/// （sort済み。安定観測の比較用）。TASK-81 E2E改訂F2: reconciliationの安定観測・
/// dup特定・close・残余close・検証(e'')の対象集合を生きpane集合から拡張する。
/// `is_remap_source`（exited/held除外）はsource選定・membership照合という
/// 生きpaneのrun照合候補としての本来用途に戻す
fn tab_terminal_sorted_ids(panes: &[PaneState], tab: TabId) -> Vec<PaneKindId> {
    let mut ids: Vec<PaneKindId> = panes
        .iter()
        .filter(|p| {
            p.tab_id == tab
                && p.is_selectable
                && !p.is_floating
                && matches!(p.id, PaneKindId::Terminal(_))
        })
        .map(|p| p.id)
        .collect();
    ids.sort_by_key(|id| id.as_spec());
    ids
}

/// 当該tabのterminal pane全体（exited / is_held含む）をvisual order（geometry
/// y,x昇順）に並べて返す（TASK-81 E2E改訂F2の検査対象集合・順位付けの共通基盤）
fn tab_terminal_incl_held(panes: &[PaneState], tab: TabId) -> Vec<&PaneState> {
    let mut in_tab: Vec<&PaneState> = panes
        .iter()
        .filter(|p| {
            p.tab_id == tab
                && p.is_selectable
                && !p.is_floating
                && matches!(p.id, PaneKindId::Terminal(_))
        })
        .collect();
    in_tab.sort_by_key(|p| (p.geometry.y, p.geometry.x));
    in_tab
}

/// 安定観測（DD-10.7 step 6。E2E改訂F2: 対象集合はexited/held込み）: 当該tabの
/// terminal pane id集合が2回連続同一になった時点のpanes一覧を返す（poll 250ms/10s。
/// 空応答は未成立としてpollingを継続。spawnは追加単調なため安定=反映完了。
/// 1回きりのlist-panesでは判定しない）
fn observe_stable(
    backend: &dyn ZellijBackend,
    target: TabId,
) -> Result<Vec<PaneState>, ZelperError> {
    let mut last: Option<Vec<PaneKindId>> = None;
    poll(
        || {
            let Some(panes) = backend.list_panes_lenient()? else {
                return Ok(None);
            };
            let ids = tab_terminal_sorted_ids(&panes, target);
            match &last {
                Some(prev) if *prev == ids => Ok(Some(panes)),
                _ => {
                    last = Some(ids);
                    Ok(None)
                }
            }
        },
        || {
            ZelperError::new(
                ErrorClass::OperationFailed,
                "timed out waiting for panes to stabilize after layout apply: screen may be \
busy and its 1s list response timeout may have produced empty output",
            )
        },
    )
}

/// 自spawn複製paneのclose＋closed_duplicatesへの記録（DD-10.2 #15/DD-12）。
/// 同一pane idの重複記録は初回のslot/reasonを優先する（closeが効かず再closeした
/// 場合の報告は1件に保つ）。close失敗はpartial_phase経由でgroup番号・部分状態
/// （実行済み/失敗/未実行区分。DD-10.11）を報告する（TASK-81レビューC6: pass 2
/// override失敗と同等の中断時報告に揃える）
#[allow(clippy::too_many_arguments)]
fn close_duplicate(
    backend: &dyn ZellijBackend,
    pane: PaneKindId,
    slot: usize,
    reason: &str,
    closed: &mut Vec<ClosedDuplicate>,
    j: usize,
    targets: &[TabId],
    plan: &V2Plan,
) -> Result<(), ZelperError> {
    backend.close_pane(&pane).map_err(|e| {
        partial_phase(
            j,
            "closing spawned duplicates (reconciliation)",
            ZelperError::new(
                e.class().clone(),
                format!(
                    "closing spawned duplicate pane {} failed: {}",
                    pane.as_spec(),
                    e.message()
                ),
            ),
            targets,
            plan,
        )
    })?;
    if !closed.iter().any(|c| c.pane == pane) {
        closed.push(ClosedDuplicate {
            pane,
            slot,
            reason: reason.to_string(),
        });
    }
    Ok(())
}

/// bare dup候補↔不成立shell slotの対応づけ（DD-10.7 step 6 多段規則。
/// TASK-81 E2E改訂F1: 幾何前提の全廃。再E2E R1: visual order補助のtie-break化）。
/// 対応づいた (dup pane id, slot, 反転cwd) 列を返し、一意に決まらない場合はNone
/// （安全側fallback——反転もcloseもせず検証失敗経路）:
/// - 数整合のみで一意（U == shell割当数 かつ bare候補数 == U: 全shell slotが
///   不成立で候補全てdup）なら全対応
/// - 属性辺（dup paneのpane_cwd == 割当paneのpane_cwd）で大きさUの完全
///   マッチングがちょうど1通りなら採用
/// - 属性辺で完全マッチングが複数通り立つ曖昧構成（同一cwd shell slot複数+dup。
///   t0p2 terminal_7・t0q2 terminal_14）ではvisual order補助辺（exited/held込み
///   順位 == slot s。旧位置規則。押し出しpaneの介在で順位==slot対応が崩れうる
///   ため補助に格下げ）と両立する（全pairが補助辺を満たす）マッチングのみ残す
///   tie-breakで絞り込み、ちょうど1通りなら採用（R1: 追加辺では属性辺の曖昧さを
///   解消できないため絞り込みに用いる。実測ではdupはslot参照位置にspawnされる
///   ため補助辺の精度は十分）
/// - 属性辺で完全マッチングが立たない場合（dupのpane_cwd欠落等）は従来どおり
///   補助辺を追加辺（属性or補助）として用い、ちょうど1通りなら採用
fn match_bare_dups(
    bare: &[&PaneState],
    shell: &[(usize, PaneKindId, String)], // (slot, assigned pane, pane_cwd)
    u: usize,
    rank_of: &std::collections::HashMap<PaneKindId, usize>,
) -> Option<Vec<(PaneKindId, usize, String)>> {
    // 数整合のみで一意: 全shell slot不成立かつ候補全てdup（対応の任意性は結果
    // 〔全close・全反転〕に影響しないため順序対応で固定）
    if u == shell.len() && bare.len() == u {
        return Some(
            shell
                .iter()
                .zip(bare.iter())
                .map(|((slot, _pane, cwd), dup)| (dup.id, *slot, cwd.clone()))
                .collect(),
        );
    }
    let attr = |di: usize, si: usize| bare[di].cwd.as_deref() == Some(shell[si].2.as_str());
    let aux = |di: usize, si: usize| rank_of.get(&bare[di].id).is_some_and(|&r| r == shell[si].0);
    let collect =
        |edge: &dyn Fn(usize, usize) -> bool| -> Option<Vec<(PaneKindId, usize, String)>> {
            let (count, first) = enumerate_matchings(bare.len(), shell.len(), edge, u, 2);
            if count == 1 {
                first.map(|pairs| {
                    pairs
                        .into_iter()
                        .map(|(di, si)| (bare[di].id, shell[si].0, shell[si].2.clone()))
                        .collect()
                })
            } else {
                None
            }
        };
    // Stage 1: 属性辺のみで一意なら採用
    let attr_edge = |di: usize, si: usize| attr(di, si);
    if let Some(matched) = collect(&attr_edge) {
        return Some(matched);
    }
    // Stage 2（R1 tie-break）: 属性辺で複数通りのとき、visual order補助辺と両立する
    // （全pairが属性辺かつ補助辺を満たす）マッチングのみ残す。残りがちょうど1通り
    // なら採用（属性マッチングの部分集合であり、属性辺を満たさない対応を選ばない）
    let tie_break = |di: usize, si: usize| attr(di, si) && aux(di, si);
    if let Some(matched) = collect(&tie_break) {
        return Some(matched);
    }
    // Stage 3: 属性辺で完全マッチングが立たない場合は補助辺を追加辺として用いる
    // （既存挙動: dupのpane_cwd欠落等で属性辺が張れない系列の救済）
    let either = |di: usize, si: usize| attr(di, si) || aux(di, si);
    collect(&either)
}

/// 大きさuの完全マッチング（dup index昇順のbacktracking・決定論的）を列挙し、
/// (検出数〔limitに達したら打ち切り〕, 最初のマッチング)を返す
fn enumerate_matchings(
    n_dup: usize,
    n_shell: usize,
    edge: &dyn Fn(usize, usize) -> bool,
    u: usize,
    limit: usize,
) -> (usize, Option<Vec<(usize, usize)>>) {
    struct MatchCtx<'a> {
        n_dup: usize,
        n_shell: usize,
        edge: &'a dyn Fn(usize, usize) -> bool,
        u: usize,
        limit: usize,
        used_slots: Vec<bool>,
        chosen: Vec<(usize, usize)>,
        count: usize,
        first: Option<Vec<(usize, usize)>>,
    }
    fn rec(ctx: &mut MatchCtx, next_dup: usize) {
        if ctx.count >= ctx.limit {
            return;
        }
        if ctx.chosen.len() == ctx.u {
            ctx.count += 1;
            if ctx.first.is_none() {
                ctx.first = Some(ctx.chosen.clone());
            }
            return;
        }
        for di in next_dup..ctx.n_dup {
            for si in 0..ctx.n_shell {
                if !ctx.used_slots[si] && (ctx.edge)(di, si) {
                    ctx.used_slots[si] = true;
                    ctx.chosen.push((di, si));
                    rec(ctx, di + 1);
                    ctx.chosen.pop();
                    ctx.used_slots[si] = false;
                    if ctx.count >= ctx.limit {
                        return;
                    }
                }
            }
        }
    }
    let mut ctx = MatchCtx {
        n_dup,
        n_shell,
        edge,
        u,
        limit,
        used_slots: vec![false; n_shell],
        chosen: Vec::with_capacity(u),
        count: 0,
        first: None,
    };
    rec(&mut ctx, 0);
    (ctx.count, ctx.first)
}

/// 照合不成立のreconciliation（DD-10.7 step 6 TASK-81。2026-09-20 E2E改訂F1/F2:
/// dup/slot対応付けを位置規則から多段規則〔属性・数突合中心〕へ全面置換・
/// 対象集合をexited/held込みへ拡張）。発動済み（生きpane数 > N_t、または
/// snapshot非存在のexited/held pane存在）のgroupに対して:
/// 1. 安定観測（terminal pane id集合〔exited/held込み〕が2回連続同一）を確認
/// 2. command dup: command slot割当ごとに自spawn候補中のterminal_command ==
///    slot argv join(" ") で逆引き→close（reason=command dup・位置不問）。
///    cwd反転の対象表現を持たないためcloseのみで検証失敗経路
/// 3. bare dup: 候補中terminal_command=None群（D）と空slot数（E）の突合で超過分
///    U=D-Eをdupとみなし、不成立shell slot（割当ありshell slotのうち割当paneの
///    pane_cwdがSomeのもの）との対応を消去法（属性照合→一意性判定→visual order
///    補助は属性辺との両立絞り込みtie-break〔一意性が得られる場合のみ〕）で特定。
///    失敗時は安全側fallback
/// 4. 対応づいたbare dupをclose→対応shell slotをpane cwd反転→pass 2（高々1回・
///    複数slot同時反転可・command dup closeと同一passに載せる）
/// 5. pass 2後も過剰なら残余close（属性・数突合版。空slot正規spawn相当を除く）を
///    実施し検証失敗（10.9）へ合流する（無限反復の構造的排除）
///
/// 戻り値: (focus対象決定に使う最終観測, 検証失敗note。Noneはreconciliation成功)
#[allow(clippy::too_many_arguments)]
fn reconcile(
    backend: &dyn ZellijBackend,
    plan: &V2Plan,
    b: usize,
    t: usize,
    target: TabId,
    first_obs: Vec<PaneState>,
    pre_ids: &HashSet<PaneKindId>,
    j: usize,
    targets: &[TabId],
    closed: &mut Vec<ClosedDuplicate>,
) -> Result<(Vec<PaneState>, Option<String>), ZelperError> {
    let n_t = plan.tabs[t].n_slots;
    let assignments = &plan.blocks[b].groups[t].assignments;
    let empty_slots = n_t - assignments.len();
    // 当該reconcile呼び出し内のdup close実績（command dup/bare dupのclose数。
    // TASK-81レビューC1: 下記早期returnの判定はこのローカル計数で行う。closedは
    // executeから渡される全group累積Vecであり、k>=2で先行groupのcloseが混入して
    // いるため判定には使えない——先行groupのclose済みというだけで後続groupの
    // 無害系列（close対象なし・対応づけ不能なし）が早期returnを抜け、残余close
    // 経路の検証失敗note〔exit 7〕へ昇格する）
    let mut dup_closes = 0usize;

    // 1. 安定観測（exited/held込み集合。最初の観測集合を直前値として比較）
    let mut prev = tab_terminal_sorted_ids(&first_obs, target);
    let stable = poll(
        || {
            let Some(panes) = backend.list_panes_lenient()? else {
                return Ok(None);
            };
            let ids = tab_terminal_sorted_ids(&panes, target);
            if ids == prev {
                Ok(Some(panes))
            } else {
                prev = ids;
                Ok(None)
            }
        },
        || {
            ZelperError::new(
                ErrorClass::OperationFailed,
                "timed out waiting for panes to stabilize after layout apply: screen may be \
busy and its 1s list response timeout may have produced empty output",
            )
        },
    )?;

    // 自spawn候補: 当該tab内terminal pane全体（exited/held含む）∧snapshot非存在id
    let mut cands: Vec<&PaneState> = tab_terminal_incl_held(&stable, target)
        .into_iter()
        .filter(|p| !pre_ids.contains(&p.id))
        .collect();
    cands.sort_by_key(|p| (p.geometry.y, p.geometry.x));
    let rank_of: std::collections::HashMap<PaneKindId, usize> =
        tab_terminal_incl_held(&stable, target)
            .into_iter()
            .enumerate()
            .map(|(rank, p)| (p.id, rank))
            .collect();

    // 2. command dup（argv一致逆引き・位置不問）
    for a in assignments.iter().filter(|a| a.run.is_some()) {
        let argv = a.run.as_ref().expect("filtered").argv.join(" ");
        for c in cands
            .iter()
            .filter(|c| c.terminal_command.as_deref() == Some(argv.as_str()))
        {
            close_duplicate(
                backend,
                c.id,
                a.slot,
                "spawned duplicate matching the command slot argv (run matched on argv but \
cwd/hold flags differ); not invertible",
                closed,
                j,
                targets,
                plan,
            )?;
            dup_closes += 1;
        }
    }

    // 3. bare dupの対応づけ（数突合+消去法）
    let bare: Vec<&PaneState> = cands
        .iter()
        .filter(|c| c.terminal_command.is_none())
        .copied()
        .collect();
    let u = bare.len().saturating_sub(empty_slots);
    let shell_assigns: Vec<(usize, PaneKindId, String)> = assignments
        .iter()
        .filter(|a| a.run.is_none())
        .filter_map(|a| {
            stable
                .iter()
                .find(|p| p.id == a.pane)
                .and_then(|p| p.cwd.clone())
                .map(|cwd| (a.slot, a.pane, cwd))
        })
        .collect();
    let mut pass2_slots: Vec<(usize, String)> = Vec::new(); // (slot, cwd)
    let mut ambiguity: Vec<String> = Vec::new();
    if u > 0 {
        match match_bare_dups(&bare, &shell_assigns, u, &rank_of) {
            Some(pairs) => {
                for (dup, slot, cwd) in pairs {
                    close_duplicate(
                        backend,
                        dup,
                        slot,
                        "spawned duplicate at shell slot (bare slot did not match the assigned \
shell pane); slot inverted to pane cwd for pass 2",
                        closed,
                        j,
                        targets,
                        plan,
                    )?;
                    dup_closes += 1;
                    pass2_slots.push((slot, cwd));
                }
            }
            None => {
                // 安全側fallback（誤close・誤反転回避）: 該当bare候補はcloseも反転も
                // せず検証失敗で報告する
                let ids = bare
                    .iter()
                    .map(|p| p.id.as_spec())
                    .collect::<Vec<_>>()
                    .join(", ");
                ambiguity.push(format!(
                    "could not uniquely associate spawned duplicate pane(s) [{ids}] with a \
shell slot in tab {} (ambiguous attributes; safe-side: no close or inversion performed for \
them)",
                    target.0
                ));
            }
        }
    }

    // 4. pass 2（反転対象slotのみpane cwd="..."へ置き換えた生成KDLで再override。
    // close単独で完了扱いにしない。command dup closeとbare dup closeをこのpassに
    // 載せている——close群はすでに実行済み）
    if !pass2_slots.is_empty() {
        let mut runs: Vec<SlotRun> = (0..n_t)
            .map(|slot| {
                assignments
                    .iter()
                    .find(|a| a.slot == slot)
                    .and_then(|a| a.run.clone())
            })
            .collect();
        for (slot, cwd) in &pass2_slots {
            runs[*slot] = Some(RunSpec {
                argv: Vec::new(),
                cwd: Some(cwd.clone()),
            });
        }
        let kdl2 = generate_instance_kdl_v2(&plan.tabs[t].subtree, &runs)?;
        backend
            .override_layout(&OverrideSpec {
                source: LayoutSpec {
                    name: None,
                    path: None,
                    inline: Some(kdl2),
                },
                apply_only_to_active_tab: true,
                retain_terminal: true,
                retain_plugin: true,
            })
            .map_err(|e| {
                partial_phase(
                    j,
                    "re-applying layout (reconciliation pass 2)",
                    e,
                    targets,
                    plan,
                )
            })?;
    }

    // 安定観測・最終状態取得（pass 2の有無に応じて）
    let final_obs = if pass2_slots.is_empty() {
        if dup_closes == 0 && ambiguity.is_empty() {
            // 当該reconcile呼び出し内で何もcloseしておらず対応づけ不能もない
            // （dup_closesは本呼び出しのclose実績のみ。closed〔全group累積〕に
            // 先行groupの実績が混入していても本groupの判定には無関係）:
            // gate発動の原因（held新規pane等）は正規spawn相当（空slot由来）の
            // 可能性があり対象外。観測をそのまま返す
            return Ok((stable, None));
        }
        poll(
            || backend.list_panes_lenient(),
            || {
                ZelperError::new(
                    ErrorClass::OperationFailed,
                    "timed out listing panes after closing duplicates: screen may be busy and \
its 1s list response timeout may have produced empty output",
                )
            },
        )?
    } else {
        observe_stable(backend, target)?
    };

    // 5. pass 2後の過剰判定（生きpane数 > N_t、またはsnapshot非存在のexited/held
    // pane残存）と残余close（属性・数突合版。空slot正規spawn相当を除く）。
    // 安全側fallback（ambiguity非空）時は残余closeも行わない（誤close回避）
    let live_count = final_obs
        .iter()
        .filter(|p| p.tab_id == target && p.is_remap_source())
        .count();
    let stray_held = final_obs.iter().any(|p| {
        p.tab_id == target
            && p.is_selectable
            && !p.is_floating
            && matches!(p.id, PaneKindId::Terminal(_))
            && (p.exited || p.is_held)
            && !pre_ids.contains(&p.id)
    });
    if live_count <= n_t && !stray_held && ambiguity.is_empty() {
        return Ok((final_obs, None));
    }
    let leftovers: Vec<&PaneState> = tab_terminal_incl_held(&final_obs, target)
        .into_iter()
        .filter(|p| !pre_ids.contains(&p.id))
        .collect();
    let bare_leftover: Vec<&PaneState> = leftovers
        .iter()
        .filter(|p| p.terminal_command.is_none())
        .copied()
        .collect();
    if !ambiguity.is_empty() {
        // 安全側fallback: 対応づけられなかった新規id pane（exited/held含む）は
        // closeせず検証失敗で報告する（誤close回避）
        let closed_desc = closed
            .iter()
            .map(|c| format!("{} (slot {})", c.pane.as_spec(), c.slot))
            .collect::<Vec<_>>()
            .join(", ");
        let note = format!(
            "{}; reconciliation could not resolve duplicate panes in tab {} (closed \
duplicates: [{closed_desc}]); pane count mismatch may be caused by cwd differences (the \
process changed its cwd after start, path normalization, or bind mounts) or hold flag \
differences (zellij run -c/-s) breaking run matching",
            ambiguity.join("; "),
            target.0,
        );
        return Ok((final_obs, Some(note)));
    }
    // 空slot正規spawn相当（terminal_command=None群のうち空slot数に相当する分）は
    // close対象から除く。個体選択は決定論的に: pane_cwdがいずれかのshell割当
    // pane_cwdと一致するもの（dup可能性高）を優先し、なお超過する分はvisual order
    // 降順（末尾側=残留に近い）からcloseする
    let shell_cwds: Vec<&str> = shell_assigns.iter().map(|(_, _, c)| c.as_str()).collect();
    let mut close_budget = bare_leftover.len().saturating_sub(empty_slots);
    let mut sorted_leftover = bare_leftover.clone();
    sorted_leftover.sort_by_key(|p| {
        let matches_shell_cwd = p.cwd.as_deref().is_some_and(|c| shell_cwds.contains(&c));
        (
            !matches_shell_cwd,
            std::cmp::Reverse((p.geometry.y, p.geometry.x)),
        )
    });
    for p in sorted_leftover {
        if close_budget == 0 {
            break;
        }
        close_duplicate(
            backend,
            p.id,
            rank_of
                .get(&p.id)
                .copied()
                .unwrap_or(n_t)
                .min(n_t.saturating_sub(1)),
            "leftover duplicate pane after reconciliation (inversion budget exhausted)",
            closed,
            j,
            targets,
            plan,
        )?;
        close_budget -= 1;
    }
    // command dup相当の残り（argv一致・close漏れ）も回収
    for a in assignments.iter().filter(|a| a.run.is_some()) {
        let argv = a.run.as_ref().expect("filtered").argv.join(" ");
        for p in leftovers
            .iter()
            .filter(|p| p.terminal_command.as_deref() == Some(argv.as_str()))
        {
            close_duplicate(
                backend,
                p.id,
                a.slot,
                "leftover command duplicate pane after reconciliation",
                closed,
                j,
                targets,
                plan,
            )?;
        }
    }
    let closed_desc = closed
        .iter()
        .map(|c| format!("{} (slot {})", c.pane.as_spec(), c.slot))
        .collect::<Vec<_>>()
        .join(", ");
    let mut note = ambiguity.join("; ");
    if !note.is_empty() {
        note.push_str("; ");
    }
    // pass 2（反転）を実施した系列のみ「after one inversion」と報告する
    // （TASK-81レビューC7: command dup closeのみなど反転未実施系列で実態と
    // ずれる文面を使わない）
    let inversion_desc = if pass2_slots.is_empty() {
        "after duplicate collection"
    } else {
        "after one inversion"
    };
    note.push_str(&format!(
        "reconciliation could not resolve duplicate panes in tab {} {inversion_desc} \
(closed duplicates: [{closed_desc}]); pane count mismatch may be caused by cwd differences \
(the process changed its cwd after start, path normalization, or bind mounts) or hold flag \
differences (zellij run -c/-s) breaking run matching",
        target.0,
    ));
    // 残余close後の実状態をfocus対象決定に使う（close済みpaneをfocus対象に
    // しない）。検証は通常経路（(b)/(d)/(e')/(e'')と本note）
    let fresh = poll(
        || backend.list_panes_lenient(),
        || {
            ZelperError::new(
                ErrorClass::OperationFailed,
                "timed out listing panes after closing duplicates: screen may be busy and \
its 1s list response timeout may have produced empty output",
            )
        },
    )?;
    Ok((fresh, Some(note)))
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

/// closed_duplicates[]のJSON表現（pane id/slot/reason。成功dataと失敗error.dataの
/// 共通構造。DD-10.9/DD-12の報告義務）
fn closed_duplicates_json(closed: &[ClosedDuplicate]) -> serde_json::Value {
    serde_json::json!(
        closed
            .iter()
            .map(|c| serde_json::json!({
                "pane": c.pane.as_spec(), "slot": c.slot, "reason": c.reason,
            }))
            .collect::<Vec<_>>()
    )
}

/// postcondition検証（DD-10.9 v2.2 + TASK-81 (e')(e'')）。失敗時は単一error（--jsonは
/// mainがerror envelope）。成功時は報告（leftover tab報告〔targetsに含まれない残存
/// tab。closeしない〕・closed_duplicates・warnings）を返す。`pre_ids`: 実行開始前
/// snapshotのpane id集合（(e'')の新規id判定）。`recon_notes`: reconciliationの
/// 検証失敗note（反転1回後も過剰が残った系列。missingへ先頭に連結する）
#[allow(clippy::too_many_arguments)]
fn verify(
    backend: &dyn ZellijBackend,
    args: &RemapArgs,
    plan: &V2Plan,
    targets: &[TabId],
    snapshot_len: usize,
    pre_ids: &HashSet<PaneKindId>,
    closed: &[ClosedDuplicate],
    recon_notes: &[String],
) -> Result<RemapOutcome, ZelperError> {
    let (after, tabs_after) = snapshot_lenient(backend)?;
    let t_count = plan.tabs.len();
    let n_slots: Vec<usize> = plan.tabs.iter().map(|t| t.n_slots).collect();
    let mut mapping: Vec<serde_json::Value> = Vec::new();
    let mut missing: Vec<String> = recon_notes.to_vec();

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
{count}; pane count mismatch may be caused by cwd differences (the process changed its \
cwd after start, path normalization, or bind mounts) or hold flag differences \
(zellij run -c/-s) breaking run matching",
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
            // (e') run一意割当の位置照合（TASK-81）: 当該tab内でrun（command+args+cwd）
            // が一意な割当に限り、適用後のvisual order s番目のpane id == 割当pane idを
            // 検証する（reconciliation後に元paneがslotから押し出されたままの状態を
            // count検証（(b)）では検出できないための追加）。同一run間・shell pane
            // （run=None）間は(iii)例外のため位置検証対象外
            for a in &g.assignments {
                let Some(spec) = &a.run else {
                    continue; // shell pane割当は(iii)例外
                };
                let run_duplicated = g
                    .assignments
                    .iter()
                    .any(|other| other.slot != a.slot && other.run.as_ref() == Some(spec));
                if run_duplicated {
                    continue;
                }
                match in_tab.get(a.slot) {
                    Some(p) if p.id == a.pane => {}
                    Some(p) => missing.push(format!(
                        "tab (b={b},t={t}) slot {}: run-unique assignment expected pane {} \
at visual position {}, found {}",
                        a.slot,
                        a.pane.as_spec(),
                        a.slot,
                        p.id.as_spec()
                    )),
                    None => {} // pane数不足は(b)で検出済み
                }
            }
            // (e'') 割当ありslot位置の新規id残存検査（TASK-81。E2E改訂F2: 対象集合
            // 拡張）: 割当ありslot位置（visual order s番目。順位付けはexited/held
            // paneを含む当該tabのterminal pane全体）に当該実行でspawnした新規id
            // pane（実行開始前snapshotに存在しないid・exited/held含む）が残存して
            // いないこと（dup close漏れ・反転不成立・押し出し・短命command dupの
            // held化残留の検出）。空slot位置の新規pane（正規spawn）は対象外。
            // (b)はexited/held paneを数えないため、held化dupの残留は本検査と
            // reconciliationの安全側fallbackが検出する（(b)との補完関係）
            let mut in_tab_all: Vec<&PaneState> = after
                .iter()
                .filter(|p| {
                    p.tab_id == target
                        && p.is_selectable
                        && !p.is_floating
                        && matches!(p.id, PaneKindId::Terminal(_))
                })
                .collect();
            in_tab_all.sort_by_key(|p| (p.geometry.y, p.geometry.x));
            for a in &g.assignments {
                if let Some(p) = in_tab_all.get(a.slot)
                    && !pre_ids.contains(&p.id)
                {
                    missing.push(format!(
                        "tab (b={b},t={t}) slot {}: new pane {} (spawned by this remap) \
occupies an assigned slot position (expected {})",
                        a.slot,
                        p.id.as_spec(),
                        a.pane.as_spec()
                    ));
                }
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

    // 対象tab内source外pane報告（TASK-81再E2E R2・DD-10.9）: 実行開始前snapshotに
    // 存在するがexited/heldでsource選定から外れたterminal paneが対象tab（targets
    // 内）に残留する場合、leftover_tabs（targets外tab列挙）にも(b)検証count
    // （生きpane集合）にも現れないため報告する（t0s sl1）。kill制約のためclose
    // しない・検証失敗にもしない（幾何の巻き込みはDD-10.13の検証対象外）
    let source_excluded: Vec<SourceExcludedPane> = after
        .iter()
        .filter(|p| {
            targets.contains(&p.tab_id)
                && p.is_selectable
                && !p.is_floating
                && matches!(p.id, PaneKindId::Terminal(_))
                && (p.exited || p.is_held)
                && pre_ids.contains(&p.id)
        })
        .map(|p| SourceExcludedPane {
            pane: p.id,
            tab: p.tab_id,
            title: p.title.clone(),
            exited: p.exited,
            held: p.is_held,
        })
        .collect();

    if !missing.is_empty() {
        // --json時は成功envelopeを出さず、mapping/missingをerror.dataに載せた
        // 単一のerror envelopeをmainから出す（stdoutに2つのJSON documentが並ぶのを防ぐ）。
        // TASK-81: leftover_tabs・closed_duplicatesもerror.dataへ実値で載せる
        // （該当なしでも空配列としてfield自体は常に載せる。R2: source_excluded_panes
        // も同様）
        return Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!("remap verification failed: {}", missing.join("; ")),
        )
        .with_data(serde_json::json!({
            "m": plan.m, "n": plan.s_slots, "k": plan.k, "t": t_count,
            "n_slots": n_slots,
            "mapping": mapping, "missing": missing, "snapshot_len": snapshot_len,
            "leftover_tabs": leftover_tabs_json(&leftovers),
            "closed_duplicates": closed_duplicates_json(closed),
            "source_excluded_panes": source_excluded_panes_json(&source_excluded),
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
                // TASK-81: 自spawn複製paneのclose実績（成功時も報告する報告義務）
                "closed_duplicates": closed_duplicates_json(closed),
                // TASK-81再E2E R2: 対象tab内に残留するsource外exited/held paneの報告
                "source_excluded_panes": source_excluded_panes_json(&source_excluded),
                "mapping": mapping,
                "tabs": targets.iter().enumerate().map(|(j, t)| {
                    let (b, ti) = (j / t_count, j % t_count);
                    serde_json::json!({
                        "index": j, "block": b, "tab_index": ti, "id": t.0,
                        "name": plan.blocks[b].groups[ti].name,
                    })
                }).collect::<Vec<_>>(),
                "leftover_tabs": leftover_tabs_json(&leftovers),
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
        // 対象tab内source外paneの人間可読報告（TASK-81再E2E R2・DD-10.9:
        // closeしない・幾何の巻き込み案内）
        for sp in &source_excluded {
            eprintln!(
                "warning: pane {} {:?} in tab {} was excluded from the remap sources \
(exited/held) and remains in a remapped tab; it is preserved (never closed) and slot \
geometry around it may differ from the layout",
                sp.pane.as_spec(),
                sp.title,
                sp.tab.0
            );
        }
        // closed_duplicatesの人間可読報告（DD-12: closeしたpane id・対応slot・理由は
        // 成功・失敗双方のJSON dataとhuman出力で報告する）
        for c in closed {
            eprintln!(
                "closed duplicate pane {} (slot {}): {}",
                c.pane.as_spec(),
                c.slot,
                c.reason
            );
        }
    }
    Ok(RemapOutcome {
        leftover_tabs: leftovers,
        closed_duplicates: closed.to_vec(),
        warnings: plan.warnings.clone(),
        source_excluded_panes: source_excluded,
    })
}

/// leftover_tabs[]のJSON表現（成功dataと失敗error.dataの共通構造）
fn leftover_tabs_json(leftovers: &[LeftoverTab]) -> serde_json::Value {
    serde_json::json!(
        leftovers
            .iter()
            .map(|l| serde_json::json!({
                "id": l.id.0, "name": l.name, "position": l.position,
                "selectable_tiled": l.selectable_tiled,
                "selectable_floating": l.selectable_floating,
            }))
            .collect::<Vec<_>>()
    )
}

/// source_excluded_panes[]のJSON表現（成功dataと失敗error.dataの共通構造。
/// TASK-81再E2E R2: 対象tab内に残留するsource外exited/held paneの報告）
fn source_excluded_panes_json(panes: &[SourceExcludedPane]) -> serde_json::Value {
    serde_json::json!(
        panes
            .iter()
            .map(|p| serde_json::json!({
                "pane": p.pane.as_spec(), "tab": p.tab.0, "title": p.title,
                "exited": p.exited, "held": p.held,
            }))
            .collect::<Vec<_>>()
    )
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
                    "run": a
                        .run
                        .as_ref()
                        .map(|v| v.argv.join(" "))
                        .unwrap_or_default(),
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
                        Some(spec) => format!(" run: {}", spec.argv.join(" ")),
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
