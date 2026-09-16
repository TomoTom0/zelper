//! remap v2 生成KDL（DD-10.8）。detailed-designのmodule構成に従い
//! layout/mod.rs から生成を分離する。
//! 改行区切り形式（dump-layout形式）・値quote必須（DD-3.3）。

use crate::error::ZelperError;
use kdl::{KdlDocument, KdlNode};

use super::{SKIP_NODES, is_content_node, leaf_is_plugin};

/// slot i への注入run（DD-10.8）。
/// - `Some(argv)`: occupied slot。terminal_command空白分割（`command`=argv[0]・`args`=残り）。
///   terminal_commandがSomeなら単語1つの起動paneでもSomeを渡す（run一致照合は
///   Run（argv）等価であり、`command="bash"`起動paneはbare slot（run=None）と
///   照合しないため。旧shell_aware_argvの「shell名のみはbare扱い」は再作成用途の
///   規則で、照合用途では誤り——v2で廃止）
/// - `None`: shell pane（terminal_command=None）または空slot。いずれもbare pane slotのまま
pub type SlotRun = Option<Vec<String>>;

/// 1 instance分の生成KDL文字列を生成する（DD-10.8 v2.2）。
/// base: 当該tab鋳型の正規形subtree（`layout::normalize_tab_templates`出力。
/// default_tab_templateのchildren置換済み・tab属性除去済み。generatorは
/// template引数を持たない——置換・slot数一致検証はlayout解決へ一元化〔D8〕）。
/// runs: slot index → run（文書順 = slot順。plugin leafはslotを消費しない）。
/// 未割当slot・空slotはlayout宣言のcommand/cwd/argsを除去してbare paneに正規化する
/// （「空slotは既定shellで埋まる」仕様。再作成paneが存在しないためcwd注入は全廃）。
/// plugin leaf・size / split_direction / name等のnode属性はlayout宣言を維持する。
/// 生成物は`layout { ... }`形式（E6実証: override-layout --layout-stringへargvで直接渡す）
pub fn generate_instance_kdl_v2(
    base: &KdlDocument,
    runs: &[SlotRun],
) -> Result<String, ZelperError> {
    let mut cloned = base.clone();
    let mut leaf_index: usize = 0;
    // base subtreeは生成KDLでlayout nodeの直下に置かれるため親はpaneではない
    inject_walk(&mut cloned, &mut leaf_index, runs, false);
    let body = format!("{cloned}");
    // 改行区切り形式（DD-3.3）で全体を組み立てる
    let mut out = String::new();
    out.push_str("layout {\n");
    for line in body.lines() {
        if line.trim().is_empty() {
            continue;
        }
        out.push_str("    ");
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.push_str("}\n");
    Ok(out)
}

/// 末端leafへrunを注入（文書順 = slot順）。
/// parent_is_pane: このdocの親がpane containerか（bare plugin nodeの除去可否に使用）
fn inject_walk(
    doc: &mut KdlDocument,
    leaf_index: &mut usize,
    runs: &[SlotRun],
    parent_is_pane: bool,
) {
    if !parent_is_pane {
        // layout直下のbare plugin nodeは実zellijでは無視される（paneを作らない）が、
        // 生成KDLはこの階層がtab配下に置かれるためInvalid tab property errorとなる。
        // 実挙動と合わせるため出力から除去する。pane run block内のplugin
        // （pane { plugin {...} }）は親がleaf扱いで再帰しないため対象外（MR-32/S11）
        doc.nodes_mut().retain(|n| n.name().value() != "plugin");
    }
    let node_count = doc.nodes().len();
    for i in 0..node_count {
        let decision = {
            let node = &doc.nodes()[i];
            let name = node.name().value();
            if SKIP_NODES.contains(&name) {
                None
            } else if name == "plugin" {
                // plugin nodeはconfig子nodeの有無にかかわらず常にleaf（walk_slotsと
                // 対称）。leaf_is_pluginでslotを消費せずskipされる（MR-32）
                Some(false)
            } else if (name == "layout" || name == "tab") && node.children().is_none() {
                // 子なしlayout/tabはslotを形成しない（walk_slotsと同じ規則。
                // leaf扱いにするとcount_terminal_slotsとinjectでindexがずれる）
                None
            } else {
                match node.children() {
                    None => Some(false), // 子なしleaf
                    Some(c) => {
                        let has_nested = c.nodes().iter().any(|n| {
                            !is_content_node(n) && !SKIP_NODES.contains(&n.name().value())
                        });
                        // has_nested / layout / tab -> container、それ以外はleaf
                        Some(has_nested || name == "layout" || name == "tab")
                    }
                }
            }
        };
        match decision {
            None => continue,
            Some(is_container) => {
                if is_container {
                    // layout/tab配下はbare plugin不可、pane/template配下はrun block可
                    let container_name = doc.nodes()[i].name().value().to_string();
                    if let Some(c) = doc.nodes_mut()[i].children_mut().as_mut() {
                        let child_parent_is_pane =
                            container_name != "layout" && container_name != "tab";
                        inject_walk(c, leaf_index, runs, child_parent_is_pane);
                    }
                } else if !leaf_is_plugin(&doc.nodes()[i]) {
                    // plugin leafはslotを消費しない（count_terminal_slotsと同じ規則）。
                    // ここでindexを進めるとcommandがplugin paneに注入され、
                    // 以降のterminal paneのslotが1つずれる
                    let idx = *leaf_index;
                    *leaf_index += 1;
                    let node = &mut doc.nodes_mut()[i];
                    normalize_leaf(node);
                    if let Some(argv) = runs.get(idx).and_then(SlotRun::as_ref)
                        && !argv.is_empty()
                    {
                        inject_run(node, argv);
                    }
                }
            }
        }
    }
}

/// leafをbare paneに正規化する: layout宣言のcommand/cwd属性とargs子nodeを除去
/// （空slotは常にbare pane・cwd注入は全廃。DD-10.2 #9 / 10.8）。
/// なおkdl crateの`KdlNode::remove`はidentifier比較にreprを含むためparsed nodeの
/// entryに一致せず、value比較のretainで除去する
fn normalize_leaf(node: &mut KdlNode) {
    node.entries_mut().retain(|e| {
        e.name()
            .map(|i| i.value())
            .is_none_or(|name| name != "command" && name != "cwd")
    });
    if let Some(children) = node.children_mut().as_mut() {
        children.nodes_mut().retain(|n| n.name().value() != "args");
        if children.nodes().is_empty() {
            // 子nodeが無くなったらchildren block自体を外す（`pane {}`を残さない）
            node.clear_children();
        }
    }
}

/// occupied slotへrun（command+args）を注入する
fn inject_run(node: &mut KdlNode, argv: &[String]) {
    set_quoted_prop(node, "command", &argv[0]);
    if argv.len() > 1 {
        let mut args = KdlNode::new("args");
        for a in &argv[1..] {
            args.push(a.as_str());
            if let Some(e) = args.entries_mut().last_mut() {
                e.set_format(kdl::KdlEntryFormat {
                    leading: " ".into(),
                    value_repr: quoted(a),
                    ..Default::default()
                });
            }
        }
        node.ensure_children();
        if let Some(child) = node.children_mut().as_mut() {
            child.nodes_mut().push(args);
        }
    }
}

/// 値を必ずquoteした表現で設定する。kdl rendererは単純な識別子状の文字列を
/// bare（command=sleep）で出力するが、zellij 0.44.3のparserはこれを拒否する
/// （実機統合テストS9で確認。実行記録はrepo管理外の検証作業dirに残置）
fn set_quoted_prop(node: &mut KdlNode, key: &str, value: &str) {
    node.insert(key, kdl::KdlValue::String(value.to_string()));
    if let Some(e) = node.entry_mut(key) {
        // insert由来のentryはformat: Noneのため、value_reprを明示設定する
        e.set_format(kdl::KdlEntryFormat {
            leading: " ".into(),
            value_repr: quoted(value),
            ..Default::default()
        });
    }
}

fn quoted(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
