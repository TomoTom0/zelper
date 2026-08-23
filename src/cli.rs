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
Examples:\n  zelper list panes\n  zelper read --tab agents\n  zelper send --command codex -- y\n  zelper rename pane 12 worker-1\n  zelper resize equalize --tab 2\n  zelper remap three --dry-run",
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
    /// 既存paneをlayoutに再配置（process保持）
    Remap {
        /// layout名（layout_dir解決。--path/--inlineと排他）
        layout: Option<String>,
        #[arg(long, conflicts_with_all = ["layout", "inline"])]
        path: Option<std::path::PathBuf>,
        #[arg(long, conflicts_with_all = ["layout", "path"])]
        inline: Option<String>,
        /// 対象tab（--session-scopeと排他）
        #[arg(long, conflicts_with = "session_scope")]
        tab: Option<String>,
        /// 全tabに独立適用（--tabと排他）
        #[arg(long, conflicts_with = "tab")]
        session_scope: bool,
        /// M>N時のoverflow戦略（既定はerror）
        #[arg(long, value_enum)]
        overflow: Option<OverflowMode>,
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
    /// pane/tab削除
    Remove {
        #[command(subcommand)]
        target: RemoveTarget,
    },
    /// shell補完scriptの生成（文書化されたverb原則の例外）
    Completion { shell: clap_complete::Shell },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowMode {
    /// 全pane保存・第1tab内入れ子・layout形状は保証しない
    Nest,
    /// layoutを追加tabで反復。overflow paneはcommand再起動（破壊的）
    Tabs,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum ListResource {
    Sessions,
    Tabs,
    Panes,
    Layouts,
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
