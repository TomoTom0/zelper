use clap::{Args, Parser, Subcommand};

/// zelper - structured automation/orchestration CLI over Zellij
#[derive(Parser, Debug)]
#[command(
    name = "zelper",
    version,
    about = "Structured automation/orchestration CLI over Zellij",
    long_about = "zelper - structured automation/orchestration CLI over Zellij\n\
\n\
Panels are addressed by pane id (terminal_3, plugin_1, or a bare number).\n\
Targets: positional pane ids plus filter options (--tab/--name/--command/--cwd/--all).\n\
\n\
Examples:\n  zelper list panes\n  zelper read --tab agents\n  zelper send --command codex -- y\n  zelper rename pane 12 worker-1\n  zelper resize equalize --tab 2\n  zelper remap three --dry-run\n  zelper move pane 3 left\n  zelper watch --command codex -- READY\n  zelper add session work",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// 対象session（省略時: ZELLIJ_SESSION_NAME > 実行中sessionが1つならそれ > error）
    #[arg(long, global = true)]
    pub session: Option<String>,

    #[command(subcommand)]
    pub command: Verb,
}

#[derive(Subcommand, Debug)]
pub enum Verb {
    /// sessions/tabs/panes/layoutsの一覧
    List {
        resource: ListResource,
        /// panes絞り込み: tab id or 一意な名前
        #[arg(long)]
        tab: Option<String>,
        /// 機械可読JSON出力
        #[arg(long)]
        json: bool,
        /// 縮小表示（human時のみ有効。panesはtab+短縮cwdのみ、他resourceは名前のみ）
        #[arg(short = 'c', long)]
        compact: bool,
    },
    /// paneの画面出力の読み取り
    Read {
        /// pane ID（terminal_3 / plugin_1 / 3）。複数指定可
        panes: Vec<String>,
        #[command(flatten)]
        filter: FilterArgs,
        /// scrollback込みで取得
        #[arg(long)]
        full: bool,
        /// 取得内容の末尾N行
        #[arg(long)]
        tail: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    /// paneへのtext/key送信
    Send {
        panes: Vec<String>,
        #[command(flatten)]
        filter: FilterArgs,
        /// text（`--`以降）
        #[arg(last = true)]
        text: Vec<String>,
        /// key列（textと排他。例: --keys Enter "Ctrl a"）
        #[arg(long, value_name = "KEY", num_args = 1.., conflicts_with = "text")]
        keys: Vec<String>,
        /// text送信後にEnterを付加
        #[arg(long, requires = "text")]
        enter: bool,
        #[arg(long)]
        json: bool,
    },
    /// pane/tabの名前変更
    Rename {
        #[command(subcommand)]
        target: RenameTarget,
    },
    /// paneサイズ変更
    Resize {
        #[command(subcommand)]
        target: ResizeTarget,
    },
    /// 既存paneをlayoutに再配置（process保持・全pane対象）
    Remap {
        /// layout名（layout_dir解決。--path/--inlineと排他）
        // '/'を含む値はlayout_dir直下のfile stemとして不成立なため構文段（exit 2）で
        // 拒否し--pathへの案内を出す（DD-1.6非例「remap ./three.kdl」の意図。TASK-65）
        #[arg(value_parser = parse_layout_name)]
        layout: Option<String>,
        #[arg(long, conflicts_with_all = ["layout", "inline"])]
        path: Option<std::path::PathBuf>,
        #[arg(long, conflicts_with_all = ["layout", "path"])]
        inline: Option<String>,
        /// sourceをこのtabに絞り込む（anchor tabもこのtab。省略時はsession全体）
        #[arg(long)]
        tab: Option<String>,
        /// floating paneをtiled化して組入れる（既定はpreflight error）
        #[arg(long)]
        embed_floating: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// pane/tab追加
    Add {
        #[command(subcommand)]
        target: AddTarget,
    },
    /// pane/tab/session削除
    Remove {
        #[command(subcommand)]
        target: RemoveTarget,
    },
    /// 単一pane/tabの移動・交換（remap=全体再配置に対するlightweight操作）
    Move {
        #[command(subcommand)]
        target: MoveTarget,
    },
    /// 対話attach（TTY必須。非TTYはexit 7）
    Attach {
        #[arg(long)]
        json: bool,
    },
    /// 対象sessionのattach clientをdetach
    Detach {
        #[arg(long)]
        json: bool,
    },
    /// pane出力のpattern監視（polling。hitまたは最大試行回数到達まで）
    Watch {
        /// pane ID（terminal_3 / plugin_1 / 3）。複数指定可（any-hit）
        panes: Vec<String>,
        #[command(flatten)]
        filter: FilterArgs,
        /// PATTERNを正規表現として照合（大文字小文字無視は`(?i)`表現）
        #[arg(long)]
        regex: bool,
        /// scrollback込みで取得（dump-screen -f）
        #[arg(long)]
        full: bool,
        /// polling間隔（秒）。1.0未満はusage error
        #[arg(long, default_value_t = 1.0)]
        interval: f64,
        /// 最大試行回数（既定30）
        #[arg(long, default_value_t = 30)]
        max: u32,
        #[arg(long)]
        json: bool,
        /// 監視pattern（`--`以降。v1はちょうど1件）
        #[arg(last = true)]
        pattern: Vec<String>,
    },
    /// shell補完scriptの生成（文書化されたverb原則の例外）
    Completion { shell: clap_complete::Shell },
    /// ドキュメント配布物の出力（文書化されたverb原則の例外）
    Docs {
        #[command(subcommand)]
        target: DocsTarget,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum ListResource {
    Sessions,
    Tabs,
    Panes,
    Layouts,
}

#[derive(Subcommand, Debug)]
pub enum DocsTarget {
    /// README.md全体（人間向け。完全usage・互換性・制限）
    Readme,
    /// LLM/agent向け配布物
    Llm {
        #[command(subcommand)]
        resource: LlmDocsResource,
    },
}

#[derive(Subcommand, Debug)]
pub enum LlmDocsResource {
    /// LLM向けusage参照（全verb文法・JSON契約・exit/error class対応・誤用対）
    Usage,
    /// Agent Skills形式skill（SKILL.md全体）
    Skill,
    /// AGENTS.md / CLAUDE.md追記用snippet
    Snippet,
}

#[derive(Subcommand, Debug)]
pub enum RenameTarget {
    /// pane名変更: zelper rename pane PANESPEC NAME
    Pane {
        pane: String,
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// tab名変更: zelper rename tab TABSPEC NAME
    Tab {
        tab: String,
        name: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ResizeTarget {
    /// 方向step変更: zelper resize pane PANESPEC (grow|shrink) (left|right|up|down) [STEPS]
    Pane {
        pane: String,
        op: GrowShrink,
        direction: Direction,
        /// resize操作の回数（既定1。1 step = 1 resize operation）
        #[arg(default_value_t = 1)]
        steps: u32,
        #[arg(long)]
        json: bool,
    },
    /// 対象tiled paneの均等化（近似）
    Equalize {
        panes: Vec<String>,
        #[arg(long)]
        tab: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum GrowShrink {
    Grow,
    Shrink,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Subcommand, Debug)]
pub enum AddTarget {
    /// detached session作成: zelper add session NAME（session 0件環境でも動作）
    Session {
        name: String,
        #[arg(long)]
        json: bool,
    },
    Pane {
        #[arg(long)]
        tab: Option<String>,
        #[arg(long, default_value_t = 1)]
        count: u32,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        cwd: Option<std::path::PathBuf>,
        /// pane内command（`--`以降）
        #[arg(last = true)]
        command: Vec<String>,
        #[arg(long)]
        json: bool,
    },
    Tab {
        #[arg(long, default_value_t = 1)]
        count: u32,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        cwd: Option<std::path::PathBuf>,
        #[arg(long, conflicts_with_all = ["path", "inline"])]
        layout: Option<String>,
        #[arg(long, conflicts_with_all = ["layout", "inline"])]
        path: Option<std::path::PathBuf>,
        #[arg(long, conflicts_with_all = ["layout", "path"])]
        inline: Option<String>,
        #[arg(last = true)]
        command: Vec<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum MoveTarget {
    /// pane移動: zelper move pane PANESPEC (left|right|up|down|rotate) [--backwards]
    ///          / zelper move pane PANESPEC --to-tab TABSPEC / --swap-with PANESPEC
    ///          （action positional / --to-tab / --swap-with は三者排他かつちょうど1つ）
    Pane {
        pane: String,
        /// 方向移動・rotate（--to-tab / --swap-with と排他）
        action: Option<MovePaneAction>,
        /// rotateを逆回しにする（action=rotateとのみ併用可）
        #[arg(long, requires = "action")]
        backwards: bool,
        /// 移動先tab（ID or 一意な名前。action positional / --swap-with と排他）
        #[arg(long = "to-tab", conflicts_with_all = ["action", "swap_with"])]
        to_tab: Option<String>,
        /// 交換相手pane（action positional / --to-tab と排他）
        #[arg(long, conflicts_with_all = ["action", "to_tab"])]
        swap_with: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// tab交換: zelper move tab TABSPEC --swap-with TABSPEC
    Tab {
        tab: String,
        #[arg(long)]
        swap_with: String,
        #[arg(long)]
        json: bool,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum MovePaneAction {
    Left,
    Right,
    Up,
    Down,
    Rotate,
}

#[derive(Subcommand, Debug)]
pub enum RemoveTarget {
    Pane {
        panes: Vec<String>,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    Tab {
        tabs: Vec<String>,
        /// 「空tab」（selectable paneが0個）を削除対象にする
        #[arg(long)]
        empty: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// session削除（TASK-84）: zelper remove session NAME... / --all / --dead / --me
    ///
    /// selectorは NAME...（1件以上）/ --all / --dead / --me の4者のうちちょうど1つ
    /// （DD-1.5排他）。単一の完全削除意味論（session行もresurrection dataも消す。
    /// backendは常に `zellij delete-session NAME -f`）。常時--yes/--dry-run必須。
    /// global --sessionは無視（対象は4 selectorのみ。--meの解決も
    /// ZELLIJ_SESSION_NAMEのみ——--sessionはsession外からも指定可能で自sessionの
    /// 証拠にならないため）
    /// --meはbest-effort: 自session削除でzelper自身が巻き込まれ、報告・exit
    /// status・検証が実行されない可能性がある（実行前にstderr warning行を出す）。
    /// 既知制限: list-sessions行なしの残data（registry掃除済み・cacheにsession_info
    /// 残留）は対象外——`zellij delete-session NAME` でのみ到達可能
    Session {
        /// 削除対象session名（複数指定可・重複は除去。--all/--dead/--meと排他）
        #[arg(conflicts_with_all = ["all", "dead", "me"])]
        names: Vec<String>,
        /// 全sessionを対象（EXITED行を含む。現在sessionは除外してwarning通知）
        #[arg(long, conflicts_with_all = ["dead", "me"])]
        all: bool,
        /// EXITED行（resurrection可能な死session）のみを対象
        #[arg(long, conflicts_with = "me")]
        dead: bool,
        /// 現在session（ZELLIJ_SESSION_NAME解決）を対象
        #[arg(long)]
        me: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
}

/// shared filter options（DD-1.4）: read/send等の共通対象絞り込み
#[derive(Args, Debug, Default)]
#[command(flatten_help = true)]
pub struct FilterArgs {
    #[arg(long)]
    pub tab: Option<String>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub command: Option<String>,
    #[arg(long)]
    pub cwd: Option<String>,
    #[arg(long)]
    pub all: bool,
}

/// remapのpositional layout名検証: layout名はlayout_dir直下の平坦なfile stemであり
/// '/'を含む値はpath誤渡しとみなす（要件2.9が排他（併用時exit 2）を要求するのに対し、
/// 単独positional pathの構文段拒否はDD-1.6非例の意図を実装側で完結させる設計裁量）
fn parse_layout_name(s: &str) -> Result<String, String> {
    if s.contains('/') {
        Err(format!(
            "invalid layout name '{s}': layout names are looked up under the layout dir; use --path for a file path"
        ))
    } else {
        Ok(s.to_string())
    }
}
