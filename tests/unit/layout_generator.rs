// L1: remap v2 生成KDL matrix（test-plan §2.7 (b) R13〜R19・(a) R9のKDL側。DD-10.8 v2.2）。
// generatorは純粋関数。TASK-75（v2.2）: generate_instance_kdl_v2はtemplate引数を持たず、
// 入力baseはnormalize_tab_templatesが返す正規形subtree（children置換済み・tab属性除去済み）のみ。
// 既存条件の期待値は不変（入力の読み替えのみ）。
use zelper::layout::generator::{SlotRun, generate_instance_kdl_v2};

const THREE_SLOT: &str = "layout {\n    tab {\n        pane size=\"40%\"\n        pane size=\"30%\"\n        pane size=\"30%\"\n    }\n}\n";

/// layout文字列から正規形subtree（鋳型0）を得る。template引数は廃止済みのため、
/// default_tab_templateのchildren置換はnormalize_tab_templatesが適用済み
fn base(text: &str) -> kdl::KdlDocument {
    let (templates, _) =
        zelper::layout::normalize_tab_templates(&zelper::layout::parse(text).unwrap()).unwrap();
    templates[0].subtree.clone()
}

/// 生成KDLを再parseしてslot毎のrunを取り出す（extract_slot_commandsはcwd無し前提）
fn slots_of(kdl: &str) -> Vec<Vec<String>> {
    zelper::layout::extract_slot_commands(&zelper::layout::parse(kdl).unwrap())
        .into_iter()
        .map(|s| s.command_argv)
        .collect()
}

fn run(argv: &[&str]) -> SlotRun {
    Some(argv.iter().map(|s| s.to_string()).collect())
}

/// subtree内（node自身含む）にcompact-bar pluginが含まれるか
/// （bar leafのtop-level sibling配置検証用）
fn contains_compact_bar(n: &kdl::KdlNode) -> bool {
    if n.name().value() == "plugin"
        && n.entries()
            .iter()
            .any(|e| e.value().as_string() == Some("zellij:compact-bar"))
    {
        return true;
    }
    n.children()
        .is_some_and(|c| c.nodes().iter().any(contains_compact_bar))
}

// [covers:layout-generator.r13-occupied-slot-injects-command-and-args]
#[test]
fn r13_occupied_slot_injects_command_and_args() {
    // occupied slotはterminal_commandを空白分割し command=argv[0]・args=残りを注入
    let runs = vec![
        run(&["bash", "/work/hb.sh", "p1"]),
        run(&["codex", "exec"]),
        None,
    ];
    let kdl = generate_instance_kdl_v2(&base(THREE_SLOT), &runs).unwrap();
    assert_eq!(
        slots_of(&kdl),
        vec![
            vec![
                "bash".to_string(),
                "/work/hb.sh".to_string(),
                "p1".to_string()
            ],
            vec!["codex".to_string(), "exec".to_string()],
            Vec::new(),
        ],
        "raw: {kdl}"
    );
    // command と args は別要素（command属性 + args子node）
    assert!(kdl.contains("command=\"bash\""), "raw: {kdl}");
    assert!(kdl.contains("args \"/work/hb.sh\" \"p1\""), "raw: {kdl}");
}

// [covers:layout-generator.r14-single-word-shell-command-still-injected]
#[test]
fn r14_single_word_shell_command_is_still_injected() {
    // terminal_commandがSomeなら単語1つの起動paneでも注入する。
    // `command="bash"`起動paneはbare slot（run=None）と照合しないため
    // （旧shell_aware_argvの「shell名のみはbare扱い」はv2で廃止。DD-10.8）
    let runs = vec![run(&["bash"]), None, None];
    let kdl = generate_instance_kdl_v2(&base(THREE_SLOT), &runs).unwrap();
    assert_eq!(
        slots_of(&kdl),
        vec![vec!["bash".to_string()], Vec::new(), Vec::new()],
        "raw: {kdl}"
    );
    assert!(kdl.contains("command=\"bash\""), "raw: {kdl}");
}

// [covers:layout-generator.r15-shell-pane-stays-bare]
#[test]
fn r15_shell_pane_stays_bare() {
    // shell pane（terminal_command None）はbare pane slotのまま（run=None一致）
    let runs = vec![None, run(&["htop"]), None];
    let kdl = generate_instance_kdl_v2(&base(THREE_SLOT), &runs).unwrap();
    assert_eq!(
        slots_of(&kdl),
        vec![Vec::new(), vec!["htop".to_string()], Vec::new()],
        "raw: {kdl}"
    );
    // slot 0にcommand属性は現れない
    assert!(!kdl.contains("command=\"sh\""), "raw: {kdl}");
}

// [covers:layout-generator.r16-empty-slots-normalized-to-bare-and-cwd-never-injected]
#[test]
fn r16_empty_slots_normalized_to_bare_and_cwd_never_injected() {
    // 空slotは常にbare pane。layout宣言commandは起動しない。cwd注入は全廃
    // （再作成paneが存在しないため。DD-10.2 #9 / 10.8）
    let declared = "layout {\n    tab {\n        pane command=\"htop\" cwd=\"/x\"\n        pane cwd=\"/y\"\n        pane\n    }\n}\n";
    // M=1（slot 0のみoccupied）・残り2 slotは空
    let runs = vec![run(&["codex"]), None, None];
    let kdl = generate_instance_kdl_v2(&base(declared), &runs).unwrap();
    assert_eq!(
        slots_of(&kdl),
        vec![vec!["codex".to_string()], Vec::new(), Vec::new(),],
        "raw: {kdl}"
    );
    assert!(
        !kdl.contains("htop"),
        "layout宣言commandは起動しない: {kdl}"
    );
    assert!(!kdl.contains("cwd"), "cwd注入は全廃: {kdl}");
}

// [covers:layout-generator.r9-fewer-panes-normalizes-declared-commands]
#[test]
fn r9_fewer_panes_than_slots_normalizes_declared_commands() {
    // R9（(a)表のKDL側）: M<N・layoutにcommand付きslot -> 空slotはすべてbare paneに
    // 正規化（layout宣言commandは起動しない）
    let declared = "layout {\n    tab {\n        pane command=\"htop\"\n        pane command=\"vim /etc/x\"\n        pane command=\"top\"\n    }\n}\n";
    let runs = vec![run(&["codex", "exec"]), None, None];
    let kdl = generate_instance_kdl_v2(&base(declared), &runs).unwrap();
    assert_eq!(
        slots_of(&kdl),
        vec![
            vec!["codex".to_string(), "exec".to_string()],
            Vec::new(),
            Vec::new(),
        ],
        "raw: {kdl}"
    );
    assert!(!kdl.contains("vim"), "raw: {kdl}");
    assert!(!kdl.contains("top"), "raw: {kdl}");
}

// [covers:layout-generator.r17-newline-separated-quoted-output-is-reparseable]
#[test]
fn r17_newline_separated_quoted_output_is_reparseable() {
    // 改行区切り形式（dump-layout形式）・値quote必須（DD-3.3。
    // zellij 0.44.3はbare値・単行;区切りを拒否する）
    let runs = vec![run(&["bash", "/work/hb.sh"]), None, None];
    let kdl = generate_instance_kdl_v2(&base(THREE_SLOT), &runs).unwrap();
    assert!(kdl.contains("command=\"bash\""), "値はquote必須: {kdl}");
    assert!(kdl.contains("args \"/work/hb.sh\""), "raw: {kdl}");
    // 単行;区切りではない（改行区切り）
    assert!(!kdl.contains("; "), "raw: {kdl}");
    // 生成物がそのまま再parse可能
    let reparsed = zelper::layout::parse(&kdl).unwrap();
    assert_eq!(zelper::layout::count_terminal_slots(&reparsed), 3);
}

// [covers:layout-generator.r18-plugin-leaf-preserved-without-consuming-slot]
#[test]
fn r18_plugin_leaf_preserved_without_consuming_slot() {
    // plugin leaf（bar plugin等のlayout宣言node）はslotを消費せず維持。
    // size / split_direction / name等のnode属性もlayout宣言を維持。
    // barの自動注入は行わない
    let with_bar = "layout {\n    tab {\n        pane size=1 borderless=true {\n            plugin location=\"zellij:tab-bar\"\n        }\n        pane split_direction=\"vertical\" name=\"left\"\n        pane name=\"right\"\n    }\n}\n";
    let runs = vec![run(&["cmdA"]), run(&["cmdB"])];
    let kdl = generate_instance_kdl_v2(&base(with_bar), &runs).unwrap();
    // plugin leafはslotを消費しない: cmdA/cmdBはterminal slotに injections
    // （plugin leafに注入されると以降が1つずれる。MR-32/S11回帰）
    assert_eq!(
        slots_of(&kdl),
        vec![vec!["cmdA".to_string()], vec!["cmdB".to_string()],],
        "raw: {kdl}"
    );
    // plugin leafと属性は維持
    assert!(
        kdl.contains("plugin location=\"zellij:tab-bar\""),
        "raw: {kdl}"
    );
    assert!(kdl.contains("size=1"), "raw: {kdl}");
    assert!(kdl.contains("split_direction=\"vertical\""), "raw: {kdl}");
    assert!(kdl.contains("name=\"left\""), "raw: {kdl}");
    // barの自動注入は行わない（宣言分のみ）
    assert_eq!(kdl.matches("plugin").count(), 1, "raw: {kdl}");
}

// [covers:layout-generator.r19-same-run-panes-share-instance-slot-order-unspecified]
#[test]
fn r19_same_run_panes_share_instance_slot_order_unspecified() {
    // 同一runの複数pane間・複数shell pane（run=None）間のslot順は保証しない
    // （要件決定論性からの明示例外。DD-10.6 (iii)/10.13。zellijのrun一致照合が
    // run等価なpaneを互いに区別しないため）。保証されるのは所属group・
    // プロセス保存・pane数・所属tab。generatorはslot -> runの写像のみを担い、
    // 同一runが複数slotに現れることを妨げない
    let runs = vec![run(&["codex", "exec"]), run(&["codex", "exec"]), None];
    let kdl = generate_instance_kdl_v2(&base(THREE_SLOT), &runs).unwrap();
    let slots = slots_of(&kdl);
    assert_eq!(slots.len(), 3, "raw: {kdl}");
    assert_eq!(
        slots[0], slots[1],
        "同一runが2 slotに現れる（slot順の優劣はgeneratorは決めない）: {kdl}"
    );
    assert!(slots[2].is_empty());

    // 複数shell pane間も同様: 複数のNone runは区別されないbare slotとして並ぶ
    let runs = vec![None, None, run(&["htop"])];
    let kdl = generate_instance_kdl_v2(&base(THREE_SLOT), &runs).unwrap();
    assert_eq!(
        slots_of(&kdl),
        vec![Vec::new(), Vec::new(), vec!["htop".to_string()]],
        "raw: {kdl}"
    );
}

// [covers:layout-generator.template-children-substitution]
#[test]
fn template_children_substitution_and_validation() {
    // TASK-75（v2.2）: children置換・slot数一致検証はnormalize_tab_templates
    // （layout解決）へ一元化。generatorは置換済みsubtreeのみを受ける
    let runs = vec![run(&["cmdA"]), run(&["cmdB"])];
    // children + compact-barからなるdefault_tab_template付きlayout（base 2-slot）
    let with_template = "layout {\n    default_tab_template {\n        children\n        pane size=1 borderless=true {\n            plugin location=\"zellij:compact-bar\"\n        }\n    }\n    pane\n    pane\n}\n";
    let normal = generate_instance_kdl_v2(&base(with_template), &runs).unwrap();
    assert!(
        normal.contains("plugin location=\"zellij:compact-bar\""),
        "raw: {normal}"
    );
    assert!(normal.contains("command=\"cmdA\""), "raw: {normal}");
    assert!(normal.contains("command=\"cmdB\""), "raw: {normal}");
    // bar leaf属性の字句形式: boolean属性はquoteなし真偽値・数値属性はquoteなし整数。
    // quote付き文字列形式（borderless="true"）はzellij 0.44.3 layout parserに
    // 「borderless should be either true or false, found "true"」として拒否される
    // （TASK-74 E2E run3実測: tmp/task74/acceptance/b_remap.json）
    assert!(normal.contains("borderless=true"), "raw: {normal}");
    assert!(normal.contains("size=1"), "raw: {normal}");
    assert!(
        !normal.contains("borderless=\"true\""),
        "quote付きboolean属性はzellij parserに拒否される: {normal}"
    );
    assert!(
        !normal.contains("size=\"1\""),
        "数値属性はquoteなし整数で出す: {normal}"
    );

    // children node不在のtemplateはLayoutInvalid（normalize時点）
    let no_children = "layout {\n    default_tab_template {\n        pane size=1 {\n            plugin location=\"zellij:compact-bar\"\n        }\n    }\n    pane\n    pane\n}\n";
    let err = normalize_err(no_children).expect("children node不在はnormalizeでLayoutInvalid");
    assert_eq!(*err.class(), zelper::error::ErrorClass::LayoutInvalid);

    // children以外にterminal paneを持つtemplate（置換後leaf数不一致）もLayoutInvalid
    let extra_leaf = "layout {\n    default_tab_template {\n        children\n        pane\n    }\n    pane\n    pane\n}\n";
    let err = normalize_err(extra_leaf).expect("leaf数不一致はnormalizeでLayoutInvalid");
    assert_eq!(*err.class(), zelper::error::ErrorClass::LayoutInvalid);

    // children block自体を持たない（braceなし）default_tab_template nodeも
    // LayoutInvalid（CR74-7: children()=NoneからNoneを返しtemplate無し扱いに
    // 黙って落ちない）
    let braceless =
        "layout {\n    default_tab_template\n    tab {\n        pane\n        pane\n    }\n}\n";
    let err = normalize_err(braceless).expect("children block不在（braceなし）もLayoutInvalid");
    assert_eq!(*err.class(), zelper::error::ErrorClass::LayoutInvalid);

    // template無しlayoutはslot木のみ（bar leafは出現しない）
    let plain = generate_instance_kdl_v2(&base("layout { pane; pane }"), &runs).unwrap();
    assert!(!plain.contains("compact-bar"), "raw: {plain}");
}

/// layout文字列をnormalizeした際のErr（LayoutInvalid検証用helper）
fn normalize_err(text: &str) -> Option<zelper::error::ZelperError> {
    zelper::layout::normalize_tab_templates(&zelper::layout::parse(text).unwrap()).err()
}

// [covers:layout-generator.template-children-substitution-nested-base]
#[test]
fn template_children_substitution_nested_base() {
    // 9-pane.kdl相当の実形状layout（layout wrapper + default_tab_template +
    // 3x3入れ子grid base）。v2.2ではnormalize_tab_templates経由で正規形鋳型subtree
    // （template反映済み・bar leaf込み）を取得する——直接組み立てたfixtureだと
    // 実layoutからの抽出経路が検証されない（TASK-74 E2E acceptance (ii) の主因）
    let real = "layout {\n    default_tab_template {\n        children\n        pane size=1 borderless=true {\n            plugin location=\"zellij:compact-bar\"\n        }\n    }\n    tab name=\"T\" {\n        pane split_direction=\"horizontal\" {\n            pane split_direction=\"vertical\" {\n                pane\n                pane\n                pane\n            }\n            pane split_direction=\"vertical\" {\n                pane\n                pane\n                pane\n            }\n            pane split_direction=\"vertical\" {\n                pane\n                pane\n                pane\n            }\n        }\n    }\n}\n";
    let (templates, _) =
        zelper::layout::normalize_tab_templates(&zelper::layout::parse(real).unwrap()).unwrap();
    assert_eq!(templates.len(), 1);
    let tpl = &templates[0];
    // 実layoutからtemplate反映済みsubtreeが取り出せる（bar leafを含む）こと
    // （現行の失敗経路: templateがroot直下しか探索されずbar leafが黙って欠落）
    let subtree_text = format!("{}", tpl.subtree);
    assert!(
        subtree_text.contains("compact-bar"),
        "正規形subtreeはdefault_tab_templateのbar leafを含む: {subtree_text}"
    );
    assert_eq!(tpl.n_slots, 9, "base subtreeは9 slot: {real}");

    let runs = vec![
        run(&["cmdA"]),
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    ];
    let kdl = generate_instance_kdl_v2(&tpl.subtree, &runs).unwrap();

    // 生成KDLのlayout node直下は2 node（horizontal container + bar leaf）
    let generated = zelper::layout::parse(&kdl).unwrap();
    let layout_children = generated
        .nodes()
        .iter()
        .find(|n| n.name().value() == "layout")
        .and_then(|n| n.children())
        .expect("生成KDLにlayout nodeがある");
    assert_eq!(
        layout_children.nodes().len(),
        2,
        "layout直下はhorizontal container + bar leafの2 node: {kdl}"
    );

    // bar leafはtop-level sibling: compact-barを含む直下nodeは1つで、それは
    // childrenがpluginのみのleaf。column・terminal paneの内側に入ると
    // compact-barを含む直下nodeはhorizontal container側になる
    let holders: Vec<_> = layout_children
        .nodes()
        .iter()
        .filter(|n| contains_compact_bar(n))
        .collect();
    assert_eq!(holders.len(), 1, "raw: {kdl}");
    let bar_leaf = holders[0];
    assert!(
        bar_leaf.name().value() == "pane"
            && bar_leaf
                .children()
                .is_some_and(|c| c.nodes().iter().all(|m| m.name().value() == "plugin")),
        "bar leafがtop-level siblingのplugin leafでない: {kdl}"
    );
    // bar leafの属性はlayout宣言を維持し、run注入（command/args）は行われない。
    // boolean属性はquoteなし真偽値・数値属性はquoteなし整数で出力される——
    // quote付き文字列形式（borderless="true"）はzellij 0.44.3 layout parserに
    // 「borderless should be either true or false, found "true"」として拒否される
    // （TASK-74 E2E run3実測: tmp/task74/acceptance/b_remap.json。文字列属性
    // 〔location等〕はquote付きのまま）
    let prop = |key: &str| {
        bar_leaf
            .entries()
            .iter()
            .find(|e| e.name().is_some_and(|i| i.value() == key))
            .map(|e| e.value())
    };
    assert_eq!(
        prop("size").and_then(|v| v.as_integer()),
        Some(1),
        "raw: {kdl}"
    );
    assert_eq!(
        prop("borderless").and_then(|v| v.as_bool()),
        Some(true),
        "raw: {kdl}"
    );
    assert!(prop("command").is_none(), "bar leafへrun注入: {kdl}");
    // 出力文字列の字句形式: quote付きboolean/数値属性は含まない
    assert!(kdl.contains("borderless=true"), "raw: {kdl}");
    assert!(kdl.contains("size=1"), "raw: {kdl}");
    assert!(
        !kdl.contains("borderless=\"true\""),
        "quote付きboolean属性はzellij parserに拒否される: {kdl}"
    );
    assert!(
        !kdl.contains("size=\"1\""),
        "数値属性はquoteなし整数で出す: {kdl}"
    );

    // slot 0のみcmdA注入（bar leafはslotを消費しない）
    assert_eq!(
        slots_of(&kdl),
        vec![
            vec!["cmdA".to_string()],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ],
        "raw: {kdl}"
    );
    assert_eq!(kdl.matches("command=\"cmdA\"").count(), 1, "raw: {kdl}");

    // 再parseしてterminal slot数=9（bar leafはplugin leafのため数えない）
    assert_eq!(zelper::layout::count_terminal_slots(&generated), 9);
}

// [covers:layout-generator.per-tab-kdl-from-normalized-templates]
#[test]
fn per_tab_kdl_from_normalized_templates() {
    // 正規形subtreeからのper-tab KDL生成: multi-tab layoutでは生成tab (b,t)毎に
    // 当該鋳型の正規形subtreeをbaseとした生成KDLを個別に返す。template由来bar leaf
    // は全tabの生成KDLへ出現し（R75-1）、tab属性は生成KDLに載せない（D7）
    let two_tab_tpl = "layout {\n    default_tab_template {\n        children\n        pane size=1 borderless=true {\n            plugin location=\"zellij:compact-bar\"\n        }\n    }\n    tab {\n        pane\n        pane\n    }\n    tab name=\"single\" {\n        pane\n    }\n}\n";
    let (templates, _) =
        zelper::layout::normalize_tab_templates(&zelper::layout::parse(two_tab_tpl).unwrap())
            .unwrap();
    assert_eq!(templates.len(), 2);
    assert_eq!(templates[0].n_slots, 2);
    assert_eq!(templates[1].n_slots, 1);

    let kdl0 =
        generate_instance_kdl_v2(&templates[0].subtree, &[run(&["cmdA"]), run(&["cmdB"])]).unwrap();
    let kdl1 = generate_instance_kdl_v2(&templates[1].subtree, &[run(&["cmdC"])]).unwrap();

    // 鋳型0側: 2 terminal slot（cmdA/cmdB注入）+ compact-bar leaf
    assert_eq!(
        slots_of(&kdl0),
        vec![vec!["cmdA".to_string()], vec!["cmdB".to_string()]],
        "raw: {kdl0}"
    );
    assert!(
        kdl0.contains("plugin location=\"zellij:compact-bar\""),
        "template由来bar leafは全tabの生成KDLへ出現: {kdl0}"
    );
    // 鋳型1側: 1 terminal slot（cmdC注入）+ 同一bar leaf
    assert_eq!(
        slots_of(&kdl1),
        vec![vec!["cmdC".to_string()]],
        "raw: {kdl1}"
    );
    assert!(
        kdl1.contains("plugin location=\"zellij:compact-bar\""),
        "raw: {kdl1}"
    );
    // 両KDLともtab nodeなしのlayout { ... }形式で、他鋳型のsubtreeを含まない
    for kdl in [&kdl0, &kdl1] {
        let doc = zelper::layout::parse(kdl).unwrap();
        let layout = doc
            .nodes()
            .iter()
            .find(|n| n.name().value() == "layout")
            .expect("layout node");
        assert!(
            layout
                .children()
                .is_none_or(|c| { c.nodes().iter().all(|n| n.name().value() != "tab") }),
            "生成KDLはtab nodeを含まない: {kdl}"
        );
    }
    assert!(!kdl0.contains("cmdC"), "raw: {kdl0}");
    assert!(!kdl1.contains("cmdA"), "raw: {kdl1}");
    assert!(!kdl1.contains("cmdB"), "raw: {kdl1}");
}
