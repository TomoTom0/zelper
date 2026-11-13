use clap::Parser;
use zelper::cli::{Cli, ListResource, Verb};
use zelper::error::ZelperError;
use zelper::zellij::ZellijBackend;
use zelper::zellij::process::{ZellijCliBackend, check_capability};

fn main() {
    let cli = Cli::parse();
    let json = wants_json(&cli);
    if let Err(e) = run(cli) {
        if json {
            // DD-4.2: --json指定時の失敗はstdoutへerror envelopeを出す
            println!("{}", zelper::output::json::err(&e));
        } else {
            eprintln!("zelper: {}", e.message());
            if !e.candidates().is_empty() {
                eprintln!("  candidates: {}", e.candidates().join(", "));
            }
        }
        std::process::exit(e.exit_code());
    }
}

/// 要求されたverbに--jsonが含まれるか（error時の出力形式判定）
fn wants_json(cli: &Cli) -> bool {
    use zelper::cli::*;
    match &cli.command {
        Verb::List { json, .. } => *json,
        Verb::Read { json, .. } => *json,
        Verb::Send { json, .. } => *json,
        Verb::Rename { target } => match target {
            RenameTarget::Pane { json, .. } => *json,
            RenameTarget::Tab { json, .. } => *json,
        },
        Verb::Resize { target } => match target {
            ResizeTarget::Pane { json, .. } => *json,
            ResizeTarget::Equalize { json, .. } => *json,
        },
        Verb::Remap { json, .. } => *json,
        Verb::Add { target } => match target {
            AddTarget::Pane { json, .. } => *json,
            AddTarget::Tab { json, .. } => *json,
            AddTarget::Session { json, .. } => *json,
        },
        Verb::Remove { target } => match target {
            RemoveTarget::Pane { json, .. } => *json,
            RemoveTarget::Tab { json, .. } => *json,
            RemoveTarget::Session { json, .. } => *json,
        },
        Verb::Move { target } => match target {
            MoveTarget::Pane { json, .. } => *json,
            MoveTarget::Tab { json, .. } => *json,
        },
        Verb::Attach { json, .. } => *json,
        Verb::Detach { json, .. } => *json,
        Verb::Watch { json, .. } => *json,
        Verb::Completion { .. } => false,
        Verb::Docs { .. } => false,
    }
}

fn run(cli: Cli) -> Result<(), ZelperError> {
    match &cli.command {
        Verb::Completion { shell } => {
            generate_completion(*shell);
            Ok(())
        }
        Verb::Docs { target } => {
            zelper::app::docs::run(target);
            Ok(())
        }
        Verb::List {
            resource,
            tab,
            json,
            compact,
        } => {
            // Sessions/Layoutsは対象session解決を必要としない（一覧表示自体が目的で、
            // EXITED混在・複数実行中でも表示できるべき）。Tabs/Panesは対象sessionが必要。
            // Sessionsはzellijを呼ぶため起動時capability gateは他verbと同じく適用する
            // （Layoutsはzellijを呼ばないためgateなし・zellij不在でも動く）
            let backend: Box<dyn ZellijBackend> = match resource {
                ListResource::Sessions => {
                    let probe = ZellijCliBackend::new("zelper-probe-unused");
                    check_capability(&probe)?;
                    Box::new(probe)
                }
                ListResource::Layouts => Box::new(ZellijCliBackend::new("zelper-probe-unused")),
                _ => mk_backend(&cli)?,
            };
            zelper::app::list::run(backend.as_ref(), *resource, tab.as_deref(), *json, *compact)
        }
        Verb::Read {
            panes,
            filter,
            full,
            tail,
            json,
        } => {
            let backend = mk_backend(&cli)?;
            zelper::app::read::run(backend.as_ref(), panes, filter, *full, *tail, *json)
        }
        Verb::Send {
            panes,
            filter,
            text,
            keys,
            enter,
            json,
        } => {
            let backend = mk_backend(&cli)?;
            zelper::app::send::run(backend.as_ref(), panes, filter, text, keys, *enter, *json)
        }
        Verb::Rename { target } => {
            let backend = mk_backend(&cli)?;
            match target {
                zelper::cli::RenameTarget::Pane { pane, name, json } => {
                    zelper::app::rename::run_pane(backend.as_ref(), pane, name, *json)
                }
                zelper::cli::RenameTarget::Tab { tab, name, json } => {
                    zelper::app::rename::run_tab(backend.as_ref(), tab, name, *json)
                }
            }
        }
        Verb::Add { target } => {
            // AddTarget::Sessionのみ例外dispatch（設計§2.2・DD-14）: add sessionは
            // sessionが1つも存在しない環境（主要ユースケース）で動く必要があるため
            // mk_backend（session解決）を経ず、ListResource::Sessionsと同一のprobe
            // backend（capability checkのみ）を直接構築する
            if let zelper::cli::AddTarget::Session { name, json } = target {
                let probe = ZellijCliBackend::new("zelper-probe-unused");
                check_capability(&probe)?;
                return zelper::app::add::run_session(&probe, name, *json);
            }
            let backend = mk_backend(&cli)?;
            match target {
                zelper::cli::AddTarget::Pane {
                    tab,
                    count,
                    name,
                    cwd,
                    command,
                    json,
                } => zelper::app::add::run_pane(
                    backend.as_ref(),
                    tab.as_deref(),
                    *count,
                    name.as_deref(),
                    cwd.as_deref(),
                    command,
                    *json,
                ),
                zelper::cli::AddTarget::Tab {
                    count,
                    name,
                    cwd,
                    layout,
                    path,
                    inline,
                    command,
                    json,
                } => {
                    let lspec = match (&layout, &path, &inline) {
                        (None, None, None) => None,
                        (l, p, i) => Some(zelper::zellij::LayoutSpec {
                            name: (*l).clone(),
                            path: (*p).clone(),
                            inline: (*i).clone(),
                        }),
                    };
                    zelper::app::add::run_tab(
                        backend.as_ref(),
                        *count,
                        name.as_deref(),
                        cwd.as_deref(),
                        lspec,
                        command,
                        *json,
                    )
                }
                // Sessionは上記の例外dispatch（probe backend）で処理済み
                zelper::cli::AddTarget::Session { .. } => {
                    unreachable!("AddTarget::Session is dispatched via probe backend above")
                }
            }
        }
        Verb::Remove { target } => {
            // RemoveTarget::Sessionは例外dispatch（DD-14・TASK-84設計§2.7）: session
            // 集合を対象とする操作は対象session解決（mk_backend）を経ずprobe backend
            // （capability checkのみ）で直接構築する（0-session環境でも事前確認まで
            // 到達させる。global --session flagはlist sessions等と同じく無視）
            if let zelper::cli::RemoveTarget::Session {
                names,
                all,
                dead,
                me,
                yes,
                dry_run,
                json,
            } = target
            {
                let probe = ZellijCliBackend::new("zelper-probe-unused");
                check_capability(&probe)?;
                return zelper::app::remove::run_session(
                    &probe, names, *all, *dead, *me, *yes, *dry_run, *json,
                );
            }
            let backend = mk_backend(&cli)?;
            match target {
                zelper::cli::RemoveTarget::Pane {
                    panes,
                    yes,
                    dry_run,
                    json,
                } => zelper::app::remove::run_pane(backend.as_ref(), panes, *yes, *dry_run, *json),
                zelper::cli::RemoveTarget::Tab {
                    tabs,
                    empty,
                    yes,
                    dry_run,
                    json,
                } => zelper::app::remove::run_tab(
                    backend.as_ref(),
                    tabs,
                    *empty,
                    *yes,
                    *dry_run,
                    *json,
                ),
                // Sessionは上記の例外dispatch（probe backend）で処理済み
                zelper::cli::RemoveTarget::Session { .. } => {
                    unreachable!("RemoveTarget::Session is dispatched via probe backend above")
                }
            }
        }
        Verb::Move { target } => {
            let backend = mk_backend(&cli)?;
            match target {
                zelper::cli::MoveTarget::Pane {
                    pane,
                    action,
                    backwards,
                    to_tab,
                    swap_with,
                    json,
                } => {
                    // cli enum（MovePaneAction）→domain（move_::PaneMoveAction）への
                    // 写像（resize armと同一pattern。三者排他・--backwards×rotateの
                    // 最終検証はapp層）
                    let act = action.map(|a| match a {
                        zelper::cli::MovePaneAction::Left => {
                            zelper::app::move_::PaneMoveAction::Direction(
                                zelper::zellij::ResizeDirection::Left,
                            )
                        }
                        zelper::cli::MovePaneAction::Right => {
                            zelper::app::move_::PaneMoveAction::Direction(
                                zelper::zellij::ResizeDirection::Right,
                            )
                        }
                        zelper::cli::MovePaneAction::Up => {
                            zelper::app::move_::PaneMoveAction::Direction(
                                zelper::zellij::ResizeDirection::Up,
                            )
                        }
                        zelper::cli::MovePaneAction::Down => {
                            zelper::app::move_::PaneMoveAction::Direction(
                                zelper::zellij::ResizeDirection::Down,
                            )
                        }
                        zelper::cli::MovePaneAction::Rotate => {
                            zelper::app::move_::PaneMoveAction::Rotate
                        }
                    });
                    zelper::app::move_::run_pane(
                        backend.as_ref(),
                        pane,
                        act,
                        *backwards,
                        to_tab.as_deref(),
                        swap_with.as_deref(),
                        *json,
                    )
                }
                zelper::cli::MoveTarget::Tab {
                    tab,
                    swap_with,
                    json,
                } => zelper::app::move_::run_tab(backend.as_ref(), tab, swap_with, *json),
            }
        }
        Verb::Resize { target } => {
            let backend = mk_backend(&cli)?;
            match target {
                zelper::cli::ResizeTarget::Pane {
                    pane,
                    op,
                    direction,
                    steps,
                    json,
                } => {
                    let grow = matches!(op, zelper::cli::GrowShrink::Grow);
                    let dir = match direction {
                        zelper::cli::Direction::Left => zelper::zellij::ResizeDirection::Left,
                        zelper::cli::Direction::Right => zelper::zellij::ResizeDirection::Right,
                        zelper::cli::Direction::Up => zelper::zellij::ResizeDirection::Up,
                        zelper::cli::Direction::Down => zelper::zellij::ResizeDirection::Down,
                    };
                    zelper::app::resize::run_pane(backend.as_ref(), pane, grow, dir, *steps, *json)
                }
                zelper::cli::ResizeTarget::Equalize { panes, tab, json } => {
                    zelper::app::resize::run_equalize(
                        backend.as_ref(),
                        panes,
                        tab.as_deref(),
                        *json,
                    )
                }
            }
        }
        Verb::Attach { json } => {
            // 報告文面にsession名が必要なため、解決済みsession名をmk_backendから
            // 受け取る（attach/detachともsession解決自体は既存共通経路）
            let (backend, session) = mk_backend_named(&cli)?;
            zelper::app::attach::run_attach(backend.as_ref(), &session, *json)
        }
        Verb::Detach { json } => {
            let (backend, session) = mk_backend_named(&cli)?;
            zelper::app::attach::run_detach(backend.as_ref(), &session, *json)
        }
        Verb::Watch {
            panes,
            filter,
            regex,
            full,
            interval,
            max,
            json,
            pattern,
        } => {
            let backend = mk_backend(&cli)?;
            let args = zelper::app::watch::WatchArgs {
                panes,
                filter,
                regex: *regex,
                full: *full,
                interval: *interval,
                max: *max,
                pattern,
                json: *json,
            };
            zelper::app::watch::run(backend.as_ref(), &args)
        }
        Verb::Remap {
            layout,
            path,
            inline,
            tab,
            embed_floating,
            dry_run,
            json,
        } => {
            let backend = mk_backend(&cli)?;
            let args = zelper::app::remap::RemapArgs {
                layout: layout.as_deref(),
                path: path.as_deref(),
                inline: inline.as_deref(),
                tab: tab.as_deref(),
                embed_floating: *embed_floating,
                dry_run: *dry_run,
                json: *json,
            };
            // 戻り値はleftover tab報告（v2.2。human/JSON出力はrun内で完結済み）
            zelper::app::remap::run(backend.as_ref(), &args).map(|_| ())
        }
    }
}

fn mk_backend(cli: &Cli) -> Result<Box<dyn ZellijBackend>, ZelperError> {
    Ok(mk_backend_named(cli)?.0)
}

/// mk_backendに解決済みsession名を加えた変種（attach/detachの報告文面用。
/// backend構築の手順・挙動はmk_backendと同一）
fn mk_backend_named(cli: &Cli) -> Result<(Box<dyn ZellijBackend>, String), ZelperError> {
    // session解決にlist-sessionsが必要な場合があるため、先に一時backendで解決する
    let probe = ZellijCliBackend::new("zelper-probe-unused");
    check_capability(&probe)?;
    let session = zelper::app::resolve_session(cli.session.as_deref(), &probe)?;
    Ok((Box::new(ZellijCliBackend::new(&session)), session))
}

fn generate_completion(shell: clap_complete::Shell) {
    use clap_complete::generate;
    let mut cmd = <Cli as clap::CommandFactory>::command();
    let mut io = std::io::stdout();
    generate(shell, &mut cmd, "zelper", &mut io);
}
