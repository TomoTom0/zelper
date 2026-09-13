use crate::domain::LayoutRef;
use crate::error::{ErrorClass, ZelperError};
use kdl::{KdlDocument, KdlNode};

pub mod generator;

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

/// slot i への注入内容（抽出用。生成はgenerator.rsのSlotRunによる統一規則。DD-10.8）
pub struct SlotCommand {
    pub command_argv: Vec<String>,
    pub cwd: Option<String>,
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
