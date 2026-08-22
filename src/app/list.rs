use crate::cli::ListResource;
use crate::error::ZelperError;
use crate::output;
use crate::selector;
use crate::zellij::ZellijBackend;

pub fn run(
    backend: &dyn ZellijBackend,
    resource: ListResource,
    tab: Option<&str>,
    json: bool,
) -> Result<(), ZelperError> {
    match resource {
        ListResource::Sessions => {
            let sessions = backend.list_sessions()?;
            let names: Vec<String> = sessions.iter().map(|s| s.name.clone()).collect();
            if json {
                println!(
                    "{}",
                    output::json::ok(serde_json::json!({ "sessions": names }))
                );
            } else {
                for n in names {
                    println!("{n}");
                }
            }
        }
        ListResource::Tabs => {
            let tabs = backend.list_tabs()?;
            if json {
                let rows: Vec<serde_json::Value> = tabs
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "tab_id": t.id.0,
                            "position": t.position,
                            "name": t.name,
                            "active": t.active,
                            "selectable_tiled_panes_count": t.selectable_tiled_panes_count,
                            "selectable_floating_panes_count": t.selectable_floating_panes_count,
                        })
                    })
                    .collect();
                println!("{}", output::json::ok(serde_json::json!({ "tabs": rows })));
            } else {
                println!("TAB_ID\tPOS\tACTIVE\tNAME");
                for t in tabs {
                    println!(
                        "{}\t{}\t{}\t{}",
                        t.id.0,
                        t.position,
                        if t.active { "*" } else { "" },
                        t.name
                    );
                }
            }
        }
        ListResource::Panes => {
            let panes = backend.list_panes()?;
            let filtered: Vec<_> = if let Some(raw) = tab {
                let tabs = backend.list_tabs()?;
                let tid = selector::resolve_tab(raw, &tabs)?;
                panes.into_iter().filter(|p| p.tab_id == tid).collect()
            } else {
                panes
            };
            if json {
                let rows: Vec<serde_json::Value> = filtered
                    .iter()
                    .map(|p| {
                        serde_json::json!({
                            "pane_id": p.id.as_spec(),
                            "title": p.title,
                            "tab_id": p.tab_id.0,
                            "tab_name": p.tab_name,
                            "command": p.command,
                            "cwd": p.cwd,
                            "floating": p.is_floating,
                            "selectable": p.is_selectable,
                            "geometry": { "x": p.geometry.x, "y": p.geometry.y, "rows": p.geometry.rows, "cols": p.geometry.cols },
                        })
                    })
                    .collect();
                println!("{}", output::json::ok(serde_json::json!({ "panes": rows })));
            } else {
                println!("PANE_ID\tTAB\tTITLE\tCOMMAND");
                for p in &filtered {
                    println!(
                        "{}\t{}\t{}\t{}",
                        p.id.as_spec(),
                        p.tab_name,
                        p.title,
                        p.command.as_deref().unwrap_or("-")
                    );
                }
            }
        }
        ListResource::Layouts => {
            let dir = layout_dir();
            let mut names: Vec<String> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for e in entries.flatten() {
                    let path = e.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("kdl")
                        && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                    {
                        names.push(stem.to_string());
                    }
                }
            }
            names.sort();
            if json {
                println!(
                    "{}",
                    output::json::ok(
                        serde_json::json!({ "layouts": names, "dir": dir.to_string_lossy() })
                    )
                );
            } else {
                for n in names {
                    println!("{n}");
                }
            }
        }
    }
    Ok(())
}

fn layout_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("ZELLIJ_LAYOUT_DIR") {
        return d.into();
    }
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(format!("{home}/.config/zellij/layouts"))
}
