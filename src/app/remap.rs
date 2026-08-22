use crate::cli::OverflowMode;
use crate::domain::{LayoutRef, PaneKindId, PaneState, TabId};
use crate::error::{ErrorClass, ZelperError};
use crate::layout;
use crate::zellij::{LayoutSpec, NewTabSpec, OverrideSpec, ZellijBackend};
use std::collections::BTreeMap;

/// remap計画（DD-10）。plannerは純粋関数（fake backendなしで検証可能）。
#[derive(Debug, Clone, PartialEq)]
pub struct RemapPlan {
    pub tab: TabId,
    pub tab_name: String,
    pub n_slots: usize,
    pub mode: PlanMode,
    pub instances: Vec<InstancePlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanMode {
    /// M <= N: 単一instance・全pane保存
    Fill,
    /// M > N + nest: 単一instance・全pane保存・形状不保証
    Nest,
    /// M > N + tabs: layout反復・overflow paneは再作成
    Tabs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InstancePlan {
    pub index: usize,
    /// slot -> pane（tabs modeのinstance >= 1はpreserved=false）
    pub assignments: Vec<Assignment>,
    /// 空slot数（bare pane = 既定shellで埋まる）
    pub empty_slots: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub slot: usize,
    pub pane: PaneKindId,
    pub preserved: bool,
    pub command_argv: Vec<String>,
    pub cwd: Option<String>,
}

/// planner（DD-10.3）。source: visual orderのremap対象pane列。
pub fn plan(
    tab: TabId,
    tab_name: &str,
    source: &[PaneState],
    n_slots: usize,
    overflow: Option<OverflowMode>,
) -> Result<RemapPlan, ZelperError> {
    let m = source.len();
    if n_slots == 0 {
        return Err(ZelperError::new(
            ErrorClass::LayoutInvalid,
            "layout has no terminal pane slots",
        ));
    }
    if m <= n_slots {
        // fill mode: 既存M paneがslotに配置され、残りN-Mは既定shell
        let assignments = source
            .iter()
            .enumerate()
            .map(|(i, p)| Assignment {
                slot: i,
                pane: p.id,
                preserved: true,
                command_argv: Vec::new(),
                cwd: None,
            })
            .collect();
        return Ok(RemapPlan {
            tab,
            tab_name: tab_name.to_string(),
            n_slots,
            mode: PlanMode::Fill,
            instances: vec![InstancePlan {
                index: 0,
                assignments,
                empty_slots: n_slots - m,
            }],
        });
    }

    // M > N
    match overflow {
        None => Err(ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "layout has {n_slots} slots but {m} panes are selected. \
Zellij cannot preserve overflow panes across tabs (verified experimentally). \
Pass --overflow nest (preserve all panes, layout shape not guaranteed) \
or --overflow tabs (repeat the layout on new tabs; overflow panes are closed and their commands restarted)"
            ),
        )),
        Some(OverflowMode::Nest) => {
            let assignments = source
                .iter()
                .enumerate()
                .map(|(i, p)| Assignment {
                    slot: i.min(n_slots - 1),
                    pane: p.id,
                    preserved: true,
                    command_argv: Vec::new(),
                    cwd: None,
                })
                .collect();
            Ok(RemapPlan {
                tab,
                tab_name: tab_name.to_string(),
                n_slots,
                mode: PlanMode::Nest,
                instances: vec![InstancePlan {
                    index: 0,
                    assignments,
                    empty_slots: 0,
                }],
            })
        }
        Some(OverflowMode::Tabs) => {
            let instances_count = m.div_ceil(n_slots);
            let mut instances = Vec::new();
            for j in 0..instances_count {
                let start = j * n_slots;
                let end = m.min(start + n_slots);
                let mut assignments = Vec::new();
                for (k, p) in source[start..end].iter().enumerate() {
                    assignments.push(Assignment {
                        slot: k,
                        pane: p.id,
                        preserved: j == 0,
                        command_argv: if j == 0 {
                            Vec::new()
                        } else {
                            shell_aware_argv(p)
                        },
                        cwd: if j == 0 { None } else { p.cwd.clone() },
                    });
                }
                instances.push(InstancePlan {
                    index: j,
                    assignments,
                    empty_slots: n_slots - (end - start),
                });
            }
            Ok(RemapPlan {
                tab,
                tab_name: tab_name.to_string(),
                n_slots,
                mode: PlanMode::Tabs,
                instances,
            })
        }
    }
}

/// pane_command文字列の空白分割（DD-10.3制限: 引用は失われる）。
/// shellのみ（argv長1かつshell名）はbare pane扱い（空vec）。
fn shell_aware_argv(p: &PaneState) -> Vec<String> {
    let Some(cmd) = &p.command else {
        return Vec::new();
    };
    let argv: Vec<String> = cmd.split_whitespace().map(|s| s.to_string()).collect();
    if argv.len() == 1
        && matches!(
            argv[0].as_str(),
            "/bin/sh" | "sh" | "/bin/bash" | "bash" | "zsh" | "fish"
        )
    {
        return Vec::new();
    }
    argv
}

/// remap実行（DD-10全体）
pub struct RemapArgs<'a> {
    pub layout: Option<&'a str>,
    pub path: Option<&'a std::path::Path>,
    pub inline: Option<&'a str>,
    pub tab: Option<&'a str>,
    pub session_scope: bool,
    pub overflow: Option<OverflowMode>,
    pub embed_floating: bool,
    pub dry_run: bool,
    pub json: bool,
}

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

    if args.session_scope {
        // DD-10.2: 各tabに独立適用（cross-tab統合は提供しない）
        let tabs = backend.list_tabs()?;
        for t in tabs.iter().filter(|t| t.selectable_tiled_panes_count > 0) {
            run_on_tab(backend, &layout_ref, t.id, args)?;
        }
        return Ok(());
    }
    // 対象tab解決（--tab省略時はactive tab。以降の処理はactive tabに依存しない）
    let target = match args.tab {
        Some(raw) => {
            let tabs = backend.list_tabs()?;
            crate::selector::resolve_tab(raw, &tabs)?
        }
        None => backend.current_tab()?.id,
    };
    run_on_tab(backend, &layout_ref, target, args)
}

fn run_on_tab(
    backend: &dyn ZellijBackend,
    layout_ref: &LayoutRef,
    target: TabId,
    args: &RemapArgs,
) -> Result<(), ZelperError> {
    // dry-runはbackendの状態を一切変更しない（DD-12）ためtab切替も行わない。
    // 実行時のみ対象tabをactive化し、元のactive tabを実行後に復帰する（DD-10.2）
    let restore_tab = if !args.dry_run {
        let current = backend.current_tab()?;
        if current.id != target {
            backend.go_to_tab(target)?;
            Some(current.id)
        } else {
            None
        }
    } else {
        None
    };

    let result = execute_on_tab(backend, layout_ref, target, args);

    if let Some(t) = restore_tab {
        let _ = backend.go_to_tab(t); // 復帰失敗は操作失敗に加えない（best-effort）
    }
    result
}

fn execute_on_tab(
    backend: &dyn ZellijBackend,
    layout_ref: &LayoutRef,
    target: TabId,
    args: &RemapArgs,
) -> Result<(), ZelperError> {
    // 1. preflight: 現状取得（対象tabは引数で確定済み。active tabに依存しない）
    let mut panes_now = backend.list_panes()?;
    let tabs = backend.list_tabs()?;
    let tab = tabs.iter().find(|t| t.id == target).ok_or_else(|| {
        ZelperError::new(ErrorClass::NoTarget, format!("tab {0} not found", target.0))
    })?;

    let in_tab: Vec<&PaneState> = panes_now.iter().filter(|p| p.tab_id == target).collect();
    let floating: Vec<_> = in_tab
        .iter()
        .filter(|p| p.is_floating && p.is_selectable)
        .collect();
    if !floating.is_empty() {
        if !args.embed_floating {
            let ids: Vec<_> = floating.iter().map(|p| p.id.as_spec()).collect();
            return Err(ZelperError::with_candidates(
                ErrorClass::Preflight,
                format!(
                    "tab has {} floating pane(s); remap would destroy them. \
Use --embed-floating to convert them to tiled (processes preserved) first",
                    floating.len()
                ),
                ids,
            ));
        }
        // dry-runは状態を一切変更しない（DD-12）。実行時のみtiled化し、
        // 化したpaneをsourceに反映するため状態を再取得する
        if !args.dry_run {
            for p in &floating {
                backend.toggle_embed_floating(&p.id)?;
            }
            panes_now = backend.list_panes()?;
        }
    }
    let mut source: Vec<PaneState> = panes_now
        .iter()
        .filter(|p| {
            p.tab_id == target
                && (p.is_remap_source()
                    // dry-run + --embed-floating: embedされる予定のpaneを計画に含める
                    || (args.dry_run && args.embed_floating && p.is_floating && p.is_selectable && matches!(p.id, PaneKindId::Terminal(_))))
        })
        .map(|p| (*p).clone())
        .collect();
    source.sort_by_key(|p| p.visual_key());

    // 2. layout解決・slot数。Nは「適用対象 = 先頭tab」のslot数とする
    //    （override-layout --apply-only-to-active-tab はlayoutの先頭tabのみ適用する。
    //     実機help確認済み。全tab合計で数えるとoverflow判定が狂う）
    let kdl_text = layout::load_kdl(layout_ref)?;
    let doc = layout::parse(&kdl_text)?;
    let base = layout::base_subtree(&doc);
    let n_slots = layout::count_terminal_slots(&base);

    // 3. plan
    let p = plan(target, &tab.name, &source, n_slots, args.overflow)?;
    let plan_json = plan_to_json(&p);

    if args.dry_run {
        if args.json {
            println!(
                "{}",
                crate::output::json::ok(serde_json::json!({ "dry_run": true, "plan": plan_json }))
            );
        } else {
            print_plan_human(&p);
        }
        return Ok(());
    }

    // 4. 実行
    let layout_spec = LayoutSpec {
        name: matches!(layout_ref, LayoutRef::Name(_)).then(|| match layout_ref {
            LayoutRef::Name(n) => n.clone(),
            _ => unreachable!(),
        }),
        path: matches!(layout_ref, LayoutRef::Path(_)).then(|| match layout_ref {
            LayoutRef::Path(p) => p.clone(),
            _ => unreachable!(),
        }),
        inline: matches!(layout_ref, LayoutRef::Inline(_)).then(|| match layout_ref {
            LayoutRef::Inline(s) => s.clone(),
            _ => unreachable!(),
        }),
    };
    let snapshot = backend.dump_layout()?;

    // tabs modeの進捗（部分適用報告に使用）
    let mut created_tabs: Vec<(usize, TabId)> = Vec::new();

    let exec_result: Result<(), ZelperError> = (|| {
        match p.mode {
            PlanMode::Fill | PlanMode::Nest => {
                backend.override_layout(&OverrideSpec {
                    source: layout_spec,
                    apply_only_to_active_tab: true,
                    retain_terminal: true,
                    retain_plugin: true,
                })?;
            }
            PlanMode::Tabs => {
                // 4a. overflow paneをclose（kill。明示flagによる同意）
                let overflow_ids: Vec<String> = p
                    .instances
                    .iter()
                    .skip(1)
                    .flat_map(|i| i.assignments.iter().map(|a| a.pane.as_spec()))
                    .collect();
                for inst in p.instances.iter().skip(1) {
                    for a in &inst.assignments {
                        backend.close_pane(&a.pane).map_err(|e| {
                            partial(
                                &format!("closing overflow pane {}", a.pane.as_spec()),
                                e,
                                &created_tabs,
                            )
                        })?;
                    }
                }
                let _ = overflow_ids;
                // 4b. instance 0: 対象tabへ適用
                backend
                    .override_layout(&OverrideSpec {
                        source: layout_spec,
                        apply_only_to_active_tab: true,
                        retain_terminal: true,
                        retain_plugin: true,
                    })
                    .map_err(|e| partial("applying layout to the target tab", e, &created_tabs))?;
                // 4c. instance >= 1: 新規tab生成 + rename
                let base_name = match layout_ref {
                    LayoutRef::Name(n) => n.clone(),
                    _ => "remap".to_string(),
                };
                for inst in p.instances.iter().skip(1) {
                    let mut slot_cmds: BTreeMap<usize, layout::SlotCommand> = BTreeMap::new();
                    for a in &inst.assignments {
                        if !a.command_argv.is_empty() || a.cwd.is_some() {
                            slot_cmds.insert(
                                a.slot,
                                layout::SlotCommand {
                                    command_argv: a.command_argv.clone(),
                                    cwd: a.cwd.clone(),
                                },
                            );
                        }
                    }
                    let tab_name = format!("{base_name}-{}", inst.index + 1);
                    let kdl = layout::generate_instance_kdl(&base, &tab_name, &slot_cmds).map_err(
                        |e| {
                            partial(
                                &format!("generating KDL for instance {}", inst.index),
                                e,
                                &created_tabs,
                            )
                        },
                    )?;
                    let new_tab = backend
                        .new_tab(&NewTabSpec {
                            name: None, // KDL側のtab nameを使用
                            cwd: None,
                            layout: Some(LayoutSpec {
                                name: None,
                                path: None,
                                inline: Some(kdl),
                            }),
                            command: vec![],
                        })
                        .map_err(|e| {
                            partial(
                                &format!("creating tab for instance {}", inst.index),
                                e,
                                &created_tabs,
                            )
                        })?;
                    backend.rename_tab(new_tab, &tab_name).map_err(|e| {
                        partial(
                            &format!("renaming instance tab {}", inst.index),
                            e,
                            &created_tabs,
                        )
                    })?;
                    created_tabs.push((inst.index, new_tab));
                }
            }
        }
        Ok(())
    })();

    exec_result?;

    // 5. 検証（DD-10.4）。tabs modeはinstance毎に「そのtabの中」で検証する
    //    （session全体のcommand一致検索は同command paneとの交差matchを生むため）
    let after = backend.list_panes()?;
    let mut mapping: Vec<serde_json::Value> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    match p.mode {
        PlanMode::Fill | PlanMode::Nest => {
            for inst in &p.instances {
                for a in &inst.assignments {
                    let alive = after.iter().any(|q| q.id == a.pane);
                    mapping.push(serde_json::json!({
                        "pane": a.pane.as_spec(), "preserved": true, "alive": alive,
                    }));
                    if !alive {
                        missing.push(a.pane.as_spec());
                    }
                }
            }
        }
        PlanMode::Tabs => {
            // instance 0: 対象tab内で保存検証
            for a in &p.instances[0].assignments {
                let alive = after.iter().any(|q| q.id == a.pane && q.tab_id == target);
                mapping.push(serde_json::json!({
                    "pane": a.pane.as_spec(), "preserved": true, "alive": alive,
                }));
                if !alive {
                    missing.push(a.pane.as_spec());
                }
            }
            // instance >= 1: 作成tab内で pane数 + command一致を検証
            for (index, tab_id) in &created_tabs {
                let panes_in_tab: Vec<&PaneState> =
                    after.iter().filter(|q| q.tab_id == *tab_id).collect();
                let tiled_count = panes_in_tab.iter().filter(|q| q.is_remap_source()).count();
                let expected_slots = p.n_slots;
                let count_ok = tiled_count == expected_slots;
                if !count_ok {
                    missing.push(format!("instance {index} tab: expected {expected_slots} tiled panes, found {tiled_count}"));
                }
                let inst = p
                    .instances
                    .iter()
                    .find(|i| i.index == *index)
                    .expect("instance");
                for a in &inst.assignments {
                    let found = if a.command_argv.is_empty() {
                        // shell paneはbare paneで再現される: command None or 既定shell
                        panes_in_tab.iter().any(|q| {
                            q.command.is_none() || is_shell(q.command.as_deref().unwrap_or(""))
                        })
                    } else {
                        let argv = a.command_argv.join(" ");
                        panes_in_tab.iter().any(|q| {
                            q.command
                                .as_deref()
                                .map(|c| c.contains(&argv))
                                .unwrap_or(false)
                        })
                    };
                    mapping.push(serde_json::json!({
                        "old_pane": a.pane.as_spec(), "preserved": false, "restarted_match": found,
                    }));
                }
            }
        }
    }

    let ok = missing.is_empty();
    if args.json {
        let env = serde_json::json!({
            "schema_version": crate::output::json::SCHEMA_VERSION,
            "ok": ok,
            "data": { "mode": format!("{:?}", p.mode), "mapping": mapping,
                      "snapshot_len": snapshot.len() },
        });
        println!("{env}");
    } else {
        for m in &mapping {
            println!("{m}");
        }
    }
    if ok {
        Ok(())
    } else {
        Err(ZelperError::new(
            ErrorClass::VerificationFailed,
            format!("remap verification failed: {missing:?}"),
        ))
    }
}

fn is_shell(cmd: &str) -> bool {
    matches!(
        cmd,
        "/bin/sh" | "sh" | "/bin/bash" | "bash" | "zsh" | "fish"
    )
}

/// 実行途中失敗時に、実行済みの状態をerror messageに載せる（DD-10.5・DD-12）
fn partial(step: &str, e: ZelperError, created_tabs: &[(usize, TabId)]) -> ZelperError {
    let applied = if created_tabs.is_empty() {
        "instances: 0 applied".to_string()
    } else {
        format!(
            "instances: 0..={} applied (tabs {:?})",
            created_tabs.last().map(|(i, _)| *i).unwrap_or(0),
            created_tabs.iter().map(|(_, t)| t.0).collect::<Vec<_>>()
        )
    };
    ZelperError::new(
        e.class().clone(),
        format!(
            "remap failed at step '{step}': {}. partial state: {applied}; remaining instances were not created (no rollback performed)",
            e.message()
        ),
    )
}

fn plan_to_json(p: &RemapPlan) -> serde_json::Value {
    serde_json::json!({
        "mode": format!("{:?}", p.mode),
        "n_slots": p.n_slots,
        "operations": plan_operations(p),
        "instances": p.instances.iter().map(|i| serde_json::json!({
            "index": i.index,
            "empty_slots": i.empty_slots,
            "assignments": i.assignments.iter().map(|a| serde_json::json!({
                "slot": a.slot, "pane": a.pane.as_spec(), "preserved": a.preserved,
                "command": if a.command_argv.is_empty() { serde_json::Value::Null } else { serde_json::json!(a.command_argv.join(" ")) },
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn print_plan_human(p: &RemapPlan) {
    println!(
        "[plan] mode={:?} slots={} instances={}",
        p.mode,
        p.n_slots,
        p.instances.len()
    );
    for i in &p.instances {
        println!(
            "[plan] instance {} (tab: {})",
            i.index,
            if i.index == 0 {
                p.tab_name.clone()
            } else {
                format!("{}-{}", p.tab_name, i.index + 1)
            }
        );
        for a in &i.assignments {
            let kind = if a.preserved { "preserve" } else { "recreate" };
            println!(
                "[plan]   slot {} <- {} ({}){}",
                a.slot,
                a.pane.as_spec(),
                kind,
                if a.command_argv.is_empty() {
                    String::new()
                } else {
                    format!(" cmd={}", a.command_argv.join(" "))
                }
            );
        }
        if i.empty_slots > 0 {
            println!("[plan]   {} empty slot(s) -> default shell", i.empty_slots);
        }
    }
    println!("[plan] planned backend operations:");
    for op in plan_operations(p) {
        println!("[plan]   {op}");
    }
}

/// dry-runに表示する実行予定backend操作列（DD-10.4 (d)）
fn plan_operations(p: &RemapPlan) -> Vec<String> {
    let mut ops = Vec::new();
    if p.mode == PlanMode::Tabs {
        let overflow: Vec<String> = p
            .instances
            .iter()
            .skip(1)
            .flat_map(|i| i.assignments.iter().map(|a| a.pane.as_spec()))
            .collect();
        ops.push(format!(
            "close-pane {} (overflow, destructive)",
            overflow.join(" ")
        ));
    }
    ops.push(
        "override-layout --apply-only-to-active-tab --retain-existing-terminal-panes --retain-existing-plugin-panes"
            .to_string(),
    );
    if p.mode == PlanMode::Tabs {
        for i in p.instances.iter().skip(1) {
            ops.push(format!(
                "new-tab (layout instance {}) + rename-tab \"{}-{}\"",
                i.index,
                p.tab_name,
                i.index + 1
            ));
        }
    }
    ops
}
