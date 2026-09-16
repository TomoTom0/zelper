use crate::domain::LayoutRef;
use crate::error::{ErrorClass, ZelperError};
use kdl::{KdlDocument, KdlNode, KdlValue};

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
/// （実機確認: レビューMR-16）。そのためKDL v2のboolean markerへ一時変換して
/// parseし、型を保ったまま元のbare表現へ戻す。
/// 既知の制限: 行単位の処理のため、複数行文字列（`"""..."""`）内部の
/// bare `true` / `false` は文字列外と誤認して書き換えられうる。
pub fn parse(kdl_text: &str) -> Result<KdlDocument, ZelperError> {
    let normalized = normalize_zellij_kdl(kdl_text);
    let mut doc = KdlDocument::parse(&normalized).map_err(|e| {
        ZelperError::new(
            ErrorClass::LayoutInvalid,
            format!("failed to parse layout KDL: {e}"),
        )
    })?;
    restore_bare_bool_format(&mut doc);
    Ok(doc)
}

/// 文字列外のbareなtrue/falseトークンをKDL v2のboolean markerへ変換する
/// （行単位の字句処理）。以前は`"true"`へ変換していたため、KdlValue::String
/// になり、生成KDLにもquote付きbooleanが漏れていた。
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
            out.push('#');
            out.push_str(token);
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

/// `#true`/`#false`はparse時の型をKdlValue::Boolにするための内部表現。
/// zellij向けの出力ではKDL v1のbare booleanを維持する。
fn restore_bare_bool_format(doc: &mut KdlDocument) {
    for node in doc.nodes_mut() {
        for entry in node.entries_mut() {
            let bool_value = match entry.value() {
                KdlValue::Bool(value) => Some(*value),
                _ => None,
            };
            if let Some(value) = bool_value
                && let Some(format) = entry.format_mut()
            {
                format.value_repr = value.to_string();
            }
        }
        if let Some(children) = node.children_mut().as_mut() {
            restore_bare_bool_format(children);
        }
    }
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

/// 正規形tab鋳型（DD-10.6 v2.2）。default_tab_template反映済み・tab属性保持。
/// plannerはname/focus/n_slots/pane_focus_slotのみを使用する
#[derive(Debug, Clone, PartialEq)]
pub struct TabTemplate {
    /// tab nodeのname属性
    pub name: Option<String>,
    /// tab nodeのfocus=true
    pub focus: bool,
    /// template反映済み・tab属性除去済みのsubtree
    pub subtree: KdlDocument,
    /// subtreeの末端terminal pane slot数（plugin leaf除外・N_t）
    pub n_slots: usize,
    /// subtree内でfocus=trueを持つ文書順最初のterminal pane leafのslot index
    pub pane_focus_slot: Option<usize>,
}

/// layout docから正規形TabTemplate列を構築する（DD-10.6 v2.2。TASK-75）。
/// layoutの各tab nodeを文書順に列挙し、各tabの鋳型（subtree・name・focus・
/// n_slots・pane_focus_slot）を構築する。tab nodeが1つもないlayout（--inline・
/// tab無しfile layout）は1つの名無し鋳型（T=1）。
/// default_tab_templateのchildren置換・slot数一致検証・N_t=0検証（LayoutInvalid）
/// はこのlayout解決時点で行う（generatorは置換済みsubtreeのみを受け取る。
/// D8一元化）。戻り値は（鋳型列、warning列）。warningは複数のpane focus=true
/// 検出（文書順最初を採用。U75-1）で、planner warningsへ接続される
pub fn normalize_tab_templates(
    doc: &KdlDocument,
) -> Result<(Vec<TabTemplate>, Vec<String>), ZelperError> {
    // layout node配下を解決（無ければdoc全体）
    let layout_children: &KdlDocument = doc
        .nodes()
        .iter()
        .find(|n| n.name().value() == "layout")
        .and_then(|n| n.children())
        .unwrap_or(doc);

    // default_tab_template（root直下またはlayout配下。DD-10.8）
    let template_node = doc
        .nodes()
        .iter()
        .chain(layout_children.nodes())
        .find(|n| n.name().value() == "default_tab_template");
    let template: Option<&KdlDocument> = match template_node {
        None => None,
        // node在り・children block無し（braceなし）もLayoutInvalid。None扱いに
        // 黙って落ちると宣言のあるbar leafが欠落したまま成功する（CR74-7）
        Some(n) => Some(n.children().ok_or_else(|| {
            ZelperError::new(
                ErrorClass::LayoutInvalid,
                "default_tab_template has no children block",
            )
        })?),
    };

    let mut warnings = Vec::new();
    let mut templates = Vec::new();

    let tab_nodes: Vec<&KdlNode> = layout_children
        .nodes()
        .iter()
        .filter(|n| n.name().value() == "tab")
        .collect();

    let build = |subtree: KdlDocument,
                 name: Option<String>,
                 focus: bool,
                 warnings: &mut Vec<String>|
     -> Result<TabTemplate, ZelperError> {
        let subtree = match template {
            None => subtree,
            Some(t) => apply_template(t, &subtree)?,
        };
        let n_slots = count_terminal_slots(&subtree);
        if n_slots == 0 {
            // N_t=0鋳型（terminal slotを持たないtab・pluginのみのtab等）は
            // LayoutInvalid（DD-10.6 v2.2。remapの適用経路は全生成tabにterminal
            // pane経由の具体化を要するため事前中断）
            return Err(ZelperError::new(
                ErrorClass::LayoutInvalid,
                "layout has no terminal pane slots",
            ));
        }
        // pane_focus_slot: 文書順最初のfocus=true terminal leaf（plugin除外）。
        // 複数個は文書順最初を採用しwarning（U75-1: zellijの複数focus=true挙動は
        // 未検証のためzelper側規則として確定）
        let mut slot = 0usize;
        let mut first = None;
        let mut focus_true_count = 0usize;
        walk_slots(&subtree, &mut |leaf| {
            if leaf_is_plugin(leaf) {
                return;
            }
            let idx = slot;
            slot += 1;
            if bool_prop(leaf, "focus") == Some(true) {
                focus_true_count += 1;
                if first.is_none() {
                    first = Some(idx);
                }
            }
        });
        if focus_true_count > 1 {
            warnings.push(format!(
                "tab template declares multiple focus=true panes ({} found); using the \
first in document order",
                focus_true_count
            ));
        }
        Ok(TabTemplate {
            name,
            focus,
            subtree,
            n_slots,
            pane_focus_slot: first,
        })
    };

    if tab_nodes.is_empty() {
        // tab無しlayout: 1つの名無し鋳型（subtree = layout直下。旧base_subtree相当）。
        // template反映の置換素材からはtemplate系node（default_tab_template等）を
        // 除外する（生成KDLへtemplate nodeを残さない）
        let mut nodes = layout_children.clone();
        nodes.nodes_mut().retain(|n| {
            !matches!(
                n.name().value(),
                "default_tab_template" | "new_tab_template" | "pane_template" | "tab_template"
            )
        });
        templates.push(build(nodes, None, false, &mut warnings)?);
    } else {
        for tab in tab_nodes {
            // tab属性（name・focus）はsubtree本体から除去し鋳型fieldへ保持する
            let mut name = None;
            let mut focus = false;
            let mut cloned = tab.clone();
            cloned.entries_mut().retain(|e| match e.name() {
                Some(i) if i.value() == "name" => {
                    name = e.value().as_string().map(str::to_string);
                    false
                }
                Some(i) if i.value() == "focus" => {
                    focus = e.value().as_bool().unwrap_or(false);
                    false
                }
                _ => true,
            });
            let subtree = cloned.children().cloned().unwrap_or_default();
            templates.push(build(subtree, name, focus, &mut warnings)?);
        }
    }
    // 複数のfocus=true鋳型（tab focus）検出: 文書順最初のt*を採用しwarning
    // （DD-10.6 v2.2・§5.2 (v)系列b。zellijの複数tab focus挙動はdumpからも
    // 区別不能なためzelper側規則として文書順最初に確定し、採用を利用者へ通知する）
    let focus_tab_count = templates.iter().filter(|tpl| tpl.focus).count();
    if focus_tab_count > 1 {
        warnings.push(format!(
            "layout declares multiple focus=true tabs ({focus_tab_count} found); using the \
first in document order"
        ));
    }
    Ok((templates, warnings))
}

/// default_tab_templateのchildren置換（TASK-74 §2.2。v2.2でlayout解決へ一元化）:
/// template subtreeの文書順最初の`children` nodeをtab subtreeのnodesで置換する。
/// children node不在はLayoutInvalid。置換後terminal slot数の不一致もLayoutInvalid
/// （誤注入防止の事前検証）
fn apply_template(
    template: &KdlDocument,
    tab_subtree: &KdlDocument,
) -> Result<KdlDocument, ZelperError> {
    let mut doc = template.clone();
    if !replace_children_marker(&mut doc, tab_subtree) {
        return Err(ZelperError::new(
            ErrorClass::LayoutInvalid,
            "default_tab_template has no children node",
        ));
    }
    if count_terminal_slots(&doc) != count_terminal_slots(tab_subtree) {
        return Err(ZelperError::new(
            ErrorClass::LayoutInvalid,
            "default_tab_template children replacement changes terminal slot count",
        ));
    }
    Ok(doc)
}

/// template内の文書順最初のchildren markerをtab subtreeのnodesで置換する
fn replace_children_marker(doc: &mut KdlDocument, base: &KdlDocument) -> bool {
    for i in 0..doc.nodes().len() {
        if doc.nodes()[i].name().value() == "children" {
            doc.nodes_mut().remove(i);
            for (offset, node) in base.nodes().iter().cloned().enumerate() {
                doc.nodes_mut().insert(i + offset, node);
            }
            return true;
        }
        if let Some(children) = doc.nodes_mut()[i].children_mut().as_mut()
            && replace_children_marker(children, base)
        {
            return true;
        }
    }
    false
}

fn bool_prop(node: &KdlNode, key: &str) -> Option<bool> {
    node.entries()
        .iter()
        .find(|e| e.name().is_some_and(|i| i.value() == key))
        .and_then(|e| e.value().as_bool())
}

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
