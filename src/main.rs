use clap::Parser;
use zelper::cli::{Cli, Verb};
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
        },
        Verb::Remove { target } => match target {
            RemoveTarget::Pane { json, .. } => *json,
            RemoveTarget::Tab { json, .. } => *json,
        },
        Verb::Completion { .. } => false,
    }
}

fn run(cli: Cli) -> Result<(), ZelperError> {
    match &cli.command {
        Verb::Completion { shell } => {
            generate_completion(*shell);
            Ok(())
        }
        Verb::List {
            resource,
            tab,
            json,
        } => {
            let backend = mk_backend(&cli)?;
            zelper::app::list::run(backend.as_ref(), *resource, tab.as_deref(), *json)
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
            }
        }
        Verb::Remove { target } => {
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
        Verb::Remap {
            layout,
            path,
            inline,
            tab,
            session_scope,
            overflow,
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
                session_scope: *session_scope,
                overflow: *overflow,
                embed_floating: *embed_floating,
                dry_run: *dry_run,
                json: *json,
            };
            zelper::app::remap::run(backend.as_ref(), &args)
        }
    }
}

fn mk_backend(cli: &Cli) -> Result<Box<dyn ZellijBackend>, ZelperError> {
    // session解決にlist-sessionsが必要な場合があるため、先に一時backendで解決する
    let probe = ZellijCliBackend::new("zelper-probe-unused");
    check_capability(&probe)?;
    let session = zelper::app::resolve_session(cli.session.as_deref(), &probe)?;
    Ok(Box::new(ZellijCliBackend::new(session)))
}

fn generate_completion(shell: clap_complete::Shell) {
    use clap_complete::generate;
    let mut cmd = <Cli as clap::CommandFactory>::command();
    let mut io = std::io::stdout();
    generate(shell, &mut cmd, "zelper", &mut io);
}
