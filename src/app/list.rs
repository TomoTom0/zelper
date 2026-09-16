use crate::cli::ListResource;
use crate::domain::TabState;
use crate::error::ZelperError;
use crate::output;
use crate::selector;
use crate::zellij::ZellijBackend;

pub fn run(
    backend: &dyn ZellijBackend,
    resource: ListResource,
    tab: Option<&str>,
    json: bool,
    compact: bool,
) -> Result<(), ZelperError> {
    // compactはhuman表示の縮小（--json併用時はJSONを優先し無視）
    let compact = compact && !json;
    match resource {
        ListResource::Sessions => {
            // 実行中（EXITED以外）のsessionを表示
            let sessions: Vec<_> = backend
                .list_sessions()?
                .into_iter()
                .filter(|s| !s.exited)
                .collect();
            if json {
                let rows: Vec<serde_json::Value> = sessions
                    .iter()
                    .map(|s| {
                        let panes = backend
                            .list_tabs_for(&s.name)
                            .ok()
                            .map(|tabs| per_tab_pane_counts(&tabs));
                        serde_json::json!({
                            "name": s.name,
                            "created": s.created,
                            "current": s.current,
                            "panes_per_tab": panes,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    output::json::ok(serde_json::json!({ "sessions": rows }))
                );
            } else if compact {
                for s in &sessions {
                    println!("{}", s.name);
                }
            } else {
                println!("NAME\tCREATED\tCURRENT\tPANES");
                for s in &sessions {
                    // 他sessionのlist-tabs失敗（実行中に消えた等）は一覧を落とさない
                    let panes = backend
                        .list_tabs_for(&s.name)
                        .ok()
                        .map(|tabs| summarize_panes(&tabs))
                        .unwrap_or_else(|| "-".to_string());
                    println!(
                        "{}\t{}\t{}\t{}",
                        s.name,
                        s.created.as_deref().unwrap_or("-"),
                        if s.current { "*" } else { "" },
                        panes
                    );
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
            } else if compact {
                for t in &tabs {
                    println!("{}", t.name);
                }
            } else {
                println!("TAB_ID\tPOS\tACTIVE\tNAME\tPANES");
                for t in tabs {
                    println!(
                        "{}\t{}\t{}\t{}\t{}",
                        t.id.0,
                        t.position,
                        if t.active { "*" } else { "" },
                        t.name,
                        pane_count_label(&t)
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
            } else if compact {
                // tab title と短縮cwdのみ。識別子を持たない行を避けるため
                // 非selectable plugin pane（tab bar等）は除外する
                for p in filtered.iter().filter(|p| p.is_selectable) {
                    println!(
                        "{}\t{}",
                        p.tab_name,
                        p.cwd
                            .as_deref()
                            .map(short_path)
                            .unwrap_or_else(|| "-".into())
                    );
                }
            } else {
                println!("PANE_ID\tTAB\tTITLE\tCOMMAND\tCWD");
                for p in &filtered {
                    println!(
                        "{}\t{}\t{}\t{}\t{}",
                        p.id.as_spec(),
                        p.tab_name,
                        p.title,
                        p.command.as_deref().unwrap_or("-"),
                        p.cwd.as_deref().unwrap_or("-")
                    );
                }
            }
        }
        ListResource::Layouts => {
            let dir = layout_dir();
            // (name, size_bytes, modified_epoch_secs)。stat失敗・mtime不明時は
            // 誤った値（size 0・半世紀前等）を表示しないようNoneを保持する
            let mut entries: Vec<(String, Option<u64>, Option<u64>)> = Vec::new();
            if let Ok(iter) = std::fs::read_dir(&dir) {
                for e in iter.flatten() {
                    let path = e.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("kdl")
                        && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                    {
                        let (size, modified) = e
                            .metadata()
                            .map(|m| {
                                let modified = m
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs());
                                (Some(m.len()), modified)
                            })
                            .unwrap_or((None, None));
                        entries.push((stem.to_string(), size, modified));
                    }
                }
            }
            entries.sort();
            if json {
                let rows: Vec<serde_json::Value> = entries
                    .iter()
                    .map(|(n, size, modified)| {
                        serde_json::json!({
                            "name": n,
                            "size": size,
                            "modified_epoch": modified,
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    output::json::ok(
                        serde_json::json!({ "layouts": rows, "dir": dir.to_string_lossy() })
                    )
                );
            } else if compact {
                for (n, _, _) in &entries {
                    println!("{n}");
                }
            } else {
                println!("NAME\tSIZE\tMODIFIED");
                for (n, size, modified) in entries {
                    println!(
                        "{n}\t{}\t{}",
                        size.map(|s| s.to_string()).unwrap_or_else(|| "-".into()),
                        modified.map(human_age).unwrap_or_else(|| "-".into())
                    );
                }
            }
        }
    }
    Ok(())
}

/// cwdの短縮表記: HOME prefixを `~` に置換する
fn short_path(p: &str) -> String {
    short_path_with(p, &std::env::var("HOME").unwrap_or_default())
}

/// HOME末尾の `/` は正規化して比較する（環境変数の揺れで短縮が
/// 無音で効かなくなるのを避ける）
fn short_path_with(p: &str, raw_home: &str) -> String {
    let home = raw_home.trim_end_matches('/');
    if !home.is_empty() && (p == home || p.starts_with(&format!("{home}/"))) {
        format!("~{}", &p[home.len()..])
    } else {
        p.to_string()
    }
}

/// tabのpane数label: tiled count（floating paneがあれば `2+1f` 形式で併記）。
/// list tabsのPANES列専用（単tabなので `+` との曖昧性なし）
fn pane_count_label(t: &TabState) -> String {
    match t.selectable_floating_panes_count {
        0 => format!("{}", t.selectable_tiled_panes_count),
        f => format!("{}+{f}f", t.selectable_tiled_panes_count),
    }
}

/// session行のPANES概要: tab毎のtiled pane数を `+` 連結（5tab目以降は `...` で省略）。
/// floatingは `2+1f` 併記をtab連結に入れるとtab境界が曖昧になるため含めない
/// （floatingはlist tabsのPANES列で確認する。JSONのpanes_per_tabとも対称）
fn summarize_panes(tabs: &[TabState]) -> String {
    let counts: Vec<String> = tabs
        .iter()
        .map(|t| t.selectable_tiled_panes_count.to_string())
        .collect();
    if counts.len() > 4 {
        format!("{}+...", counts[..4].join("+"))
    } else {
        counts.join("+")
    }
}

/// JSON用: tab毎のtiled pane数（floatingはlist tabsで確認する役割分担）
fn per_tab_pane_counts(tabs: &[TabState]) -> Vec<u32> {
    tabs.iter()
        .map(|t| t.selectable_tiled_panes_count)
        .collect()
}

/// epoch秒からの経過を `3d 5h ago` 形式で（zellijのCreated表記と同系の相対表記）
fn human_age(epoch_secs: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs = now.saturating_sub(epoch_secs);
    let (d, h, m, s) = (
        secs / 86400,
        (secs % 86400) / 3600,
        (secs % 3600) / 60,
        secs % 60,
    );
    if d > 0 {
        format!("{d}d {h}h ago")
    } else if h > 0 {
        format!("{h}h {m}m ago")
    } else if m > 0 {
        format!("{m}m {s}s ago")
    } else {
        format!("{s}s ago")
    }
}

fn layout_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("ZELLIJ_LAYOUT_DIR") {
        return d.into();
    }
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(format!("{home}/.config/zellij/layouts"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P1-2(d): human_ageの粒度境界（private fnのためlib内testで検証）
    // [covers:list-display.human-age-granularity-boundaries]
    #[test]
    fn human_age_granularity_boundaries() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(human_age(now), "0s ago");
        assert_eq!(human_age(now - 1), "1s ago");
        assert_eq!(human_age(now - 60), "1m 0s ago");
        assert_eq!(human_age(now - 3600), "1h 0m ago");
        assert_eq!(human_age(now - 86400), "1d 0h ago");
        // 未来時刻はsaturating_subで0s扱い（負経過を表示しない）
        assert_eq!(human_age(now + 3600), "0s ago");
    }

    /// P1-2(a)(b): PANES概要の5tab超省略とfloating除外
    // [covers:list-display.panes-summary-elision-and-floating-exclusion]
    #[test]
    fn summarize_panes_elides_beyond_four_tabs_and_excludes_floating() {
        let tab = |tiled: u32, floating: u32| TabState {
            id: crate::domain::TabId(0),
            position: 0,
            name: String::new(),
            active: false,
            selectable_tiled_panes_count: tiled,
            selectable_floating_panes_count: floating,
            are_floating_panes_visible: false,
        };
        assert_eq!(summarize_panes(&[tab(7, 0), tab(7, 0), tab(6, 0)]), "7+7+6");
        // floatingが混ざっても連結はtiledのみ
        assert_eq!(summarize_panes(&[tab(2, 1), tab(2, 0)]), "2+2");
        // 5tab目以降は省略
        assert_eq!(
            summarize_panes(&[tab(1, 0), tab(2, 0), tab(3, 0), tab(4, 0), tab(5, 0)]),
            "1+2+3+4+..."
        );
    }
}

#[cfg(test)]
mod short_path_tests {
    use super::short_path_with;

    /// レビューC21: HOME置換の境界case（prefix類似dir・HOME自身・未設定・末尾/）
    // [covers:list-display.home-substitution-boundaries]
    #[test]
    fn short_path_boundaries() {
        assert_eq!(short_path_with("/home/tomo/work", "/home/tomo"), "~/work");
        assert_eq!(short_path_with("/home/tomo", "/home/tomo"), "~");
        // prefix類似dirは置換しない（/home/tomox ≠ /home/tomo 配下）
        assert_eq!(short_path_with("/home/tomox", "/home/tomo"), "/home/tomox");
        assert_eq!(
            short_path_with("/home/tomo2/x", "/home/tomo"),
            "/home/tomo2/x"
        );
        // HOME外・HOME未設定相当（空）
        assert_eq!(short_path_with("/usr/bin", "/home/tomo"), "/usr/bin");
        assert_eq!(short_path_with("/home/tomo/work", ""), "/home/tomo/work");
        // HOME末尾スラッシュは正規化して一致
        assert_eq!(short_path_with("/home/tomo/work", "/home/tomo/"), "~/work");
    }
}
