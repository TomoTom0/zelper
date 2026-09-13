//! zelper companion plugin（DD-10.3）。
//!
//! remap v2のcross-tab移動をzellij-tile API（break_panes_to_*）で実行する。
//! CLI（zelper本体）から `zellij --session S action pipe --plugin file:<path>.wasm
//! --name <name> -- <payload>` で駆動される（E6実証）。plugin -> CLIへの応答手段
//! （cli_pipe_output）はserver内1s timeoutで不達のため設計に使わない。効果の成否は
//! zelper側のpolling + postcondition検証で判定される。
//!
//! protocol（payloadは空白区切りの位置文字列。pane idは`terminal_N`またはbare int）:
//!
//! ```text
//! probe     <nonce>                -> rename_pane_with_id(own pane, "zelper-probe-<nonce>")
//! break-id  <tab_id> <pane_id_csv> -> break_panes_to_tab_with_id(panes, tab_id, true)
//! break-new <pane_id_csv>          -> break_panes_to_new_tab(panes, None, true)
//! ```
//!
//! - `probe`は状態変更前の応答性・権限確認（設計レビューR3）: ChangeApplicationStateを
//!   要するがuserのpane/tab構成を変えない操作（plugin自身のnon-selectable paneの
//!   title書換え）を使う。zelperはnonce込みtitleの出現をpollingする
//! - `break-new`のname引数は不使用（未検証のため。DD-10.2 #8）。tab名はzelperが
//!   `rename-tab-by-id`で付与する
//! - 未知のnameは何もしない（旧binary混在時の安全側挙動）
//! - `should_change_focus=true`はE6実証値

use std::collections::BTreeMap;
use zellij_tile::prelude::*;

#[derive(Default)]
struct ZelperCompanion {}

fn parse_pane_ids(spec: &str) -> Vec<PaneId> {
    spec.split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            if let Some(rest) = s.strip_prefix("plugin_") {
                PaneId::Plugin(rest.parse::<u32>().unwrap_or(u32::MAX))
            } else if let Some(rest) = s.strip_prefix("terminal_") {
                PaneId::Terminal(rest.parse::<u32>().unwrap_or(u32::MAX))
            } else {
                PaneId::Terminal(s.parse::<u32>().unwrap_or(u32::MAX))
            }
        })
        .collect()
}

impl ZellijPlugin for ZelperCompanion {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        // remapのsource・検証に現れないようnon-selectableにする（E6実証）。
        // seed漏れ時に対話grantで救済されうる経路を残す
        set_selectable(false);
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::ReadCliPipes,
        ]);
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        let name = pipe_message.name.clone();
        let payload = pipe_message.payload.clone().unwrap_or_default();
        self.dispatch(&name, &payload);
        false
    }

    fn render(&mut self, _rows: usize, _cols: usize) {}
}

impl ZelperCompanion {
    /// 処理のみ行い応答は返さない（DD-10.3 plugin側実装規則）
    fn dispatch(&mut self, name: &str, payload: &str) {
        let toks: Vec<&str> = payload.split_whitespace().collect();
        match name {
            "probe" => {
                let Some(nonce) = toks.first() else { return };
                let ids = get_plugin_ids();
                rename_pane_with_id(
                    PaneId::Plugin(ids.plugin_id),
                    format!("zelper-probe-{nonce}"),
                );
            }
            "break-id" => {
                if toks.len() != 2 {
                    return;
                }
                let Ok(tab_id) = toks[0].parse::<u32>() else {
                    return;
                };
                let panes = parse_pane_ids(toks[1]);
                break_panes_to_tab_with_id(&panes, tab_id as usize, true);
            }
            "break-new" => {
                if toks.len() != 1 {
                    return;
                }
                let panes = parse_pane_ids(toks[0]);
                break_panes_to_new_tab(&panes, None, true);
            }
            // 未知のnameは何もしない（旧binary混在時の安全側挙動）
            _ => {}
        }
    }
}

register_plugin!(ZelperCompanion);
