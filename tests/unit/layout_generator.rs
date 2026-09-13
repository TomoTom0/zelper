// L1: remap v2 生成KDL matrix（test-plan §2.7 (b) R13〜R19・(a) R9のKDL側。DD-10.8）。
// generatorは純粋関数。fail-first: 実装（TASK-39）前にfailする（test-plan §5）。
use zelper::layout::generator::{SlotRun, generate_instance_kdl_v2};

const THREE_SLOT: &str = "layout {\n    tab {\n        pane size=\"40%\"\n        pane size=\"30%\"\n        pane size=\"30%\"\n    }\n}\n";

fn base(text: &str) -> kdl::KdlDocument {
    zelper::layout::base_subtree(&zelper::layout::parse(text).unwrap())
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

// [covers:layout-generator.r13-occupied-slot-injects-command-and-args]
#[test]
fn r13_occupied_slot_injects_command_and_args() {
    // occupied slotはpane_commandを空白分割し command=argv[0]・args=残りを注入
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
    // pane_commandがSomeなら単語1つのshell起動paneでも注入する。
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
    // shell pane（pane_command None）はbare pane slotのまま（run=None一致）
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
