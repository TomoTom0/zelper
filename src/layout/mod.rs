use crate::domain::LayoutRef;
use crate::error::{ErrorClass, ZelperError};
use kdl::{KdlDocument, KdlNode};

/// LayoutRefからKDL textを読み込む（DD-3.3/10.2）
pub fn load_kdl(r: &LayoutRef) -> Result<String, ZelperError> {
    match r {
        LayoutRef::Inline(s) => Ok(s.clone()),
        LayoutRef::Path(p) => std::fs::read_to_string(p).map_err(|e| {
            ZelperError::new(
                ErrorClass::LayoutNotFound,
                format!("cannot read layout file {}: {e}", p.display()),
            )
        }),
        LayoutRef::Name(name) => {
            let dir = layout_dir();
            let path = dir.join(format!("{name}.kdl"));
            std::fs::read_to_string(&path).map_err(|_| {
                ZelperError::new(
                    ErrorClass::LayoutNotFound,
                    format!(
                        "layout '{name}' not found in {} (searched {})",
                        dir.display(),
                        path.display()
                    ),
                )
            })
        }
    }
}

fn layout_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("ZELLIJ_LAYOUT_DIR") {
        return d.into();
    }
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(format!("{home}/.config/zellij/layouts"))
}

/// KDLをparse（LayoutInvalid検出）。
/// zellijのlayout KDLはKDL v1寄りで、bareの `true` / `false`（`borderless=true`、
/// `start_suspended true` 等）を常用するが、kdl crate（KDL v2）はこれを拒否する
/// （実機確認: レビューMR-16）。そのためquote正規化してから渡す。
pub fn parse(kdl_text: &str) -> Result<KdlDocument, ZelperError> {
    let normalized = normalize_zellij_kdl(kdl_text);
    KdlDocument::parse(&normalized).map_err(|e| {
        ZelperError::new(
            ErrorClass::LayoutInvalid,
            format!("failed to parse layout KDL: {e}"),
        )
    })
}

/// 文字列外のbareなtrue/falseトークンをquoteする（行単位の字句処理）
fn normalize_zellij_kdl(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        out.push_str(&quote_bare_bools(line));
        out.push('\n');
    }
    out
}

fn quote_bare_bools(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_string = false;
    let mut token = String::new();
    let flush = |out: &mut String, token: &mut String| {
        if token.is_empty() {
            return;
        }
        if token == "true" || token == "false" {
            out.push('"');
            out.push_str(token);
            out.push('"');
        } else {
            out.push_str(token);
        }
        token.clear();
    };
    for c in line.chars() {
        if in_string {
            out.push(c);
            if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                flush(&mut out, &mut token);
                in_string = true;
                out.push(c);
            }
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | '.' | '/' | ':' | '#' | '\\' => {
                token.push(c)
            }
            _ => {
                flush(&mut out, &mut token);
                out.push(c);
            }
        }
    }
    flush(&mut out, &mut token);
    out
}

const SKIP_NODES: &[&str] = &[
    "pane_template",
    "tab_template",
    "default_tab_template",
    "new_tab_template",
    "floating_panes",
    "swap_tiled_layout",
    "swap_floating_layout",
    // pane/tab配下に現れる非pane node（値のみ持ちslotを形成しない）
    "cwd",
    "start_suspended",
    "close_on_exit",
];

/// 末端terminal pane slot数を数える（DD-10.2）。plugin leafは除外。
pub fn count_terminal_slots(doc: &KdlDocument) -> usize {
    let mut n = 0;
    walk_slots(doc, &mut |leaf| {
        if !leaf_is_plugin(leaf) {
            n += 1;
        }
    });
    n
}

/// pane直下のcontent node（slotを形成しない子）。それ以外の子はnested pane/container
const PANE_CONTENT_NODES: &[&str] = &["plugin", "args"];

fn is_content_node(n: &KdlNode) -> bool {
    PANE_CONTENT_NODES.contains(&n.name().value())
}

/// 末端pane leafを文書順に列挙する
fn walk_slots(doc: &KdlDocument, f: &mut impl FnMut(&KdlNode)) {
    for node in doc.nodes() {
        let name = node.name().value();
        if SKIP_NODES.contains(&name) {
            continue;
        }
        if name == "plugin" {
            // plugin node配下の子node（zjstatusのformat_left等）はすべてそのpluginの
            // configurationでありslotを形成しない。実zellijは子nodeをconfig値として
            // 文字列化し（zellij-utils/src/kdl/kdl_layout_parser.rs
            // parse_plugin_user_configuration）、paneの外のbare plugin自体も無視する。
            // 子の有無にかかわらずleafとして扱い、配下へ再帰しない（MR-32）
            f(node);
            continue;
        }
        if name == "layout" || name == "tab" {
            if let Some(child) = node.children() {
                walk_slots(child, f);
            }
            continue;
        }
        match node.children() {
            None => f(node), // 子なしleaf（bare pane等）
            Some(c) => {
                let has_nested = c
                    .nodes()
                    .iter()
                    .any(|n| !is_content_node(n) && !SKIP_NODES.contains(&n.name().value()));
                if has_nested {
                    walk_slots(c, f);
                } else {
                    f(node) // contentのみ（plugin/args）を持つleaf
                }
            }
        }
    }
}

/// leafがplugin paneか（bareのplugin node、または子にplugin nodeを含むpane）
fn leaf_is_plugin(node: &KdlNode) -> bool {
    node.name().value() == "plugin"
        || node
            .children()
            .map(|c| c.nodes().iter().any(|n| n.name().value() == "plugin"))
            .unwrap_or(false)
}

/// slot i への注入内容
pub struct SlotCommand {
    pub command_argv: Vec<String>,
    pub cwd: Option<String>,
}

/// 1 instance分のKDL文字列を生成する（DD-10.3 tabs mode / DD-10.7）。
/// base: 対象layoutの最初のtab（無い場合はlayout本体）のsubtree。
/// slot_commands: slot index → 注入command（未指定slotはbare paneのまま）。
pub fn generate_instance_kdl(
    base: &KdlDocument,
    tab_name: &str,
    slot_commands: &std::collections::BTreeMap<usize, SlotCommand>,
) -> Result<String, ZelperError> {
    let mut cloned = base.clone();
    let mut leaf_index: usize = 0;
    // base subtreeは生成KDLでtab name="..."の直下に置かれるため親はpaneではない
    inject_walk(&mut cloned, &mut leaf_index, slot_commands, false);
    let body = format!("{cloned}");
    // 改行区切り形式（DD-3.3）で全体を組み立てる
    let name_escaped = tab_name.replace('"', "\\\"");
    let mut out = String::new();
    out.push_str("layout {\n");
    out.push_str(&format!("    tab name=\"{name_escaped}\" {{\n"));
    for line in body.lines() {
        if line.trim().is_empty() {
            continue;
        }
        out.push_str("        ");
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.push_str("    }\n");
    out.push_str("}\n");
    Ok(out)
}

/// 末端leafへcommandを注入（文書順 = slot順）。
/// parent_is_pane: このdocの親がpane containerか（run block配置可否の判定に使用）
fn inject_walk(
    doc: &mut KdlDocument,
    leaf_index: &mut usize,
    slot_commands: &std::collections::BTreeMap<usize, SlotCommand>,
    parent_is_pane: bool,
) {
    if !parent_is_pane {
        // layout/tab直下のbare plugin nodeは実zellijでは無視される（paneを作らない）が、
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
                    if let Some(c) = doc.nodes_mut()[i].children_mut() {
                        let child_parent_is_pane =
                            container_name != "layout" && container_name != "tab";
                        inject_walk(c, leaf_index, slot_commands, child_parent_is_pane);
                    }
                } else if !leaf_is_plugin(&doc.nodes()[i]) {
                    // plugin leafはslotを消費しない（count_terminal_slotsと同じ規則）。
                    // ここでindexを進めるとcommandがplugin paneに注入され、
                    // 以降のterminal paneのslotが1つずれる
                    let idx = *leaf_index;
                    *leaf_index += 1;
                    if let Some(sc) = slot_commands.get(&idx) {
                        inject_into_leaf(&mut doc.nodes_mut()[i], sc);
                    }
                }
            }
        }
    }
}

fn inject_into_leaf(node: &mut KdlNode, sc: &SlotCommand) {
    if let Some(cwd) = &sc.cwd {
        set_quoted_prop(node, "cwd", cwd);
    }
    if sc.command_argv.is_empty() {
        return; // shellのみ → bare paneのまま（cwdのみ注入）
    }
    let argv0 = sc.command_argv[0].clone();
    set_quoted_prop(node, "command", &argv0);
    if sc.command_argv.len() > 1 {
        let mut args = KdlNode::new("args");
        for a in &sc.command_argv[1..] {
            args.push(a.as_str());
            if let Some(e) = args.entries_mut().last_mut() {
                e.set_format(kdl::KdlEntryFormat {
                    leading: " ".into(),
                    value_repr: quoted(a),
                    ..Default::default()
                });
            }
        }
        // 子docが無いleafには子docを作る
        if node.children_mut().is_none() {
            node.ensure_children();
        }
        if let Some(child) = node.children_mut() {
            child.nodes_mut().push(args);
        }
    }
}

/// 値を必ずquoteした表現で設定する。kdl rendererは単純な識別子状の文字列を
/// bare（command=sleep）で出力するが、zellij 0.44.3のparserはこれを拒否する
/// （実機統合テストS9で確認。tmp/phase7/integration-report.md参照）
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

/// 末端terminal slotの注入内容（command/cwd）を文書順に抽出する。
/// injectの逆変換。生成KDLの実検証（fake backendの再現・test）に使用する
pub fn extract_slot_commands(doc: &KdlDocument) -> Vec<SlotCommand> {
    let mut slots = Vec::new();
    walk_slots(doc, &mut |leaf| {
        if leaf_is_plugin(leaf) {
            return;
        }
        let mut sc = SlotCommand {
            command_argv: Vec::new(),
            cwd: None,
        };
        if let Some(v) = string_prop(leaf, "command") {
            sc.command_argv.push(v);
        }
        if let Some(v) = string_prop(leaf, "cwd") {
            sc.cwd = Some(v);
        }
        if let Some(children) = leaf.children() {
            for n in children.nodes() {
                if n.name().value() == "args" {
                    for e in n.entries() {
                        if let Some(s) = e.value().as_string() {
                            sc.command_argv.push(s.to_string());
                        }
                    }
                }
            }
        }
        slots.push(sc);
    });
    slots
}

fn string_prop(node: &KdlNode, key: &str) -> Option<String> {
    node.entries()
        .iter()
        .find(|e| e.name().is_some_and(|i| i.value() == key))
        .and_then(|e| e.value().as_string().map(|s| s.to_string()))
}

/// 対象layoutから「最初のtabのsubtree」（tabが無ければlayout本体）を取り出す
pub fn base_subtree(doc: &KdlDocument) -> KdlDocument {
    for node in doc.nodes() {
        if node.name().value() == "layout"
            && let Some(l) = node.children()
        {
            for t in l.nodes() {
                if t.name().value() == "tab"
                    && let Some(c) = t.children()
                {
                    return c.clone();
                }
            }
            return l.clone();
        }
    }
    doc.clone()
}
