// L2: 決定的fake backend（test-plan §1）。全操作を記録し、状態を持つ最小実装。
// remap v2（DD-10）対応: pipe_plugin記録・probe/break pipe効果模擬・失敗注入
// （pipe効果不出現・重複spawn・focus復帰失敗）を含む（test-plan §2.7 (j)）。
// TASK-81: paneのinvoked_with（起動時Run）side-table（FakeRun）と、override-layoutの
// run等値照合（zellij find_already_running_panes模倣: 生成KDL slot列とpaneの
// invoked_withをRunCommand全field比較・None==None bare一致で照合し、不一致slotは
// spawn・一致paneはslot配置）を追加。derive互換: invoked_with未設定paneは
// PaneState.terminal_command/cwdからderive（Some→Command{cwd:None}・None→None）
// により既存条件の期待値を維持したままreconciliation系条件を支える。
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use zelper::domain::*;
use zelper::error::{ErrorClass, ZelperError};
use zelper::zellij::*;

/// paneのinvoked_with（起動時Run）の明示設定（TASK-81）。zellijのRun等価比較は
/// RunCommandでcwd・holdを含む全field一致（DD-10.1事実5 TASK-81訂正）
#[derive(Debug, Clone, PartialEq)]
pub enum FakeRun {
    /// invoked_with = None（bare起動。session開始layout・new-pane・keyboard new-tab由来）
    None,
    /// Run::Cwd（CLI new-tab既定pane・zelper自身の空group生成tabを含む）
    Cwd(PathBuf),
    /// Run::Command（zellij run起因。cwdは--cwdなし起動でもSome〔クライアントcwd〕）
    Command {
        argv: Vec<String>,
        cwd: Option<PathBuf>,
        hold_on_close: bool,
        hold_on_start: bool,
    },
}

impl FakeRun {
    /// 生成KDL slotの注入内容（SlotCommand）から期待run表現を構成する。
    /// KDL未指定のhold属性はzellijの既定（hold_on_close=true・hold_on_start=false）
    fn from_slot(sc: &zelper::layout::SlotCommand) -> FakeRun {
        if sc.command_argv.is_empty() {
            match &sc.cwd {
                // pane cwd="X"（commandなし）はRun::Cwdを表現（DD-10.1事実5）
                Some(cwd) => FakeRun::Cwd(PathBuf::from(cwd)),
                None => FakeRun::None,
            }
        } else {
            FakeRun::Command {
                argv: sc.command_argv.clone(),
                cwd: sc.cwd.as_deref().map(PathBuf::from),
                hold_on_close: true,
                hold_on_start: false,
            }
        }
    }
}

/// TASK-95レビュー対応: rename_session呼出後の事後list_sessions状態注入
/// （postcondition各判定〔旧名消失・新名running出現〕の独立保護検証用）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenamePostInjection {
    /// 旧名残存 + 新名running行追加（旧名消失検査の独立保護）
    KeepOldAddNew,
    /// 旧名・新名とも行消失（新名running出現検査の独立保護）
    DropBoth,
    /// 旧名消失 + 新名がEXITED行で出現（exited == false検査の独立保護）
    NewAsExited,
}

pub struct FakeBackend {
    pub state: RefCell<FakeState>,
    /// 呼び出し記録（"rename-pane terminal_1" 形式）
    pub calls: RefCell<Vec<String>>,
    /// この操作名の次回呼び出しを失敗させる
    pub fail_op: RefCell<Vec<String>>,
    /// 失敗注入（fail_op・fail_list_sessions_after_create）が生成するerror classの
    /// 差し替え（TASK-93: add sessionのclass保持伝播をZellijUnavailable系でも
    /// 検証するため。None時はOperationFailed）
    pub fail_class: RefCell<Option<ErrorClass>>,
    /// 次回override-layout時に除去するpane（zellijがlayout適用でdead paneを掃除する挙動のemulate）
    pub drop_on_override: RefCell<Vec<PaneKindId>>,
    /// このcommandの再作成を失敗させる（new-tabでbare paneに化ける。command再起動失敗のemulate）
    pub fail_restart: RefCell<Vec<String>>,
    /// このnameのpipeは成功するが効果を反映しない（pipe効果不出現 = polling timeout模擬）
    pub pipe_mute: RefCell<Vec<String>>,
    /// 次回override-layout時にこのtabへshell paneを1つ追加（command照合missの重複spawn模擬）
    pub spawn_on_override: RefCell<Vec<TabId>>,
    /// override-layout呼び出し回数
    pub override_count: Cell<usize>,
    /// この回数を超えたoverride後の最初のgo-to-tabを1回失敗させる（focus復帰失敗模擬）
    pub fail_go_to_after: Cell<Option<usize>>,
    /// 指定番目（1開始）のoverride-layout呼出のみ失敗させる（TASK-70: layout適用
    /// phase失敗〔DD-10.12 worked example #7のinstance 2 = 2番目のoverride失敗〕の
    /// 再現用。fail_opの先頭一致は最初の呼出に消費されるため専用arm。失敗時は
    /// 状態を変更せずErr。未設定Noneは従来挙動不変）
    pub fail_override_layout_nth: Cell<Option<usize>>,
    /// 最初のpipe呼出より後の最初のlist_panes_lenientをこの時間だけ遅延させる
    /// （polling deadline超過後の条件成立模擬。C3）
    pub slow_list_panes_after_pipe: Cell<Option<std::time::Duration>>,
    /// rename_tabを記録・成功扱いにするが名前を変えない（rename不生效模擬。C5）
    pub mute_rename_tab: Cell<bool>,
    /// 最初のpipe呼出より後のlist_panes_lenient応答の先頭N回を空（Ok(None)）にする
    /// （TASK-74。CR74-3: run冒頭のsnapshot_lenientでは計数が消費されないarm法）
    pub empty_panes_after_pipe: Cell<usize>,
    /// 最初のpipe呼出より後のlist_panes（strict）応答の先頭N回を空stdout相当の
    /// 失敗にする（busy windowはlenient/strict両呼び出しに掛かる模擬。CR74-2:
    /// poll内のstrict再取得が空でfatalになる窓を検出させる）
    pub strict_empty_panes_after_pipe: Cell<usize>,
    /// 最初のoverride-layout呼出より後のlist_panes_lenient応答の先頭N回を空にする
    /// （TASK-74。CR74-3: 検証冒頭snapshotへ空を届けるarm法。layout適用phaseと
    /// 検証冒頭の間にlenient呼び出しがないため最終override以降のarmと同義）
    pub empty_panes_after_override: Cell<usize>,
    /// 最初のoverride-layout呼出より後のlist_tabs_lenient応答の先頭N回を空にする
    pub empty_tabs_after_override: Cell<usize>,
    /// 最初のoverride-layout呼出より後のlist_panes_lenient応答のうち指定番目
    /// （1開始）の呼び出しのみ空にする（TASK-75 E2E要因A改訂: step 6のfocus対象
    /// 決定・存在確認pollが先頭N回空を全数消化するようになったため、適用後再取得
    /// の特定位置へ空を届ける精密arm法）
    pub empty_panes_nth_after_override: Cell<Option<usize>>,
    /// 同型のtabs側精密arm（指定番目の呼び出しのみ空）
    pub empty_tabs_nth_after_override: Cell<Option<usize>>,
    /// empty_*_nth_after_overrideの呼び出し計数（1開始）
    pub lenient_panes_count: Cell<usize>,
    pub lenient_tabs_count: Cell<usize>,
    /// move phase完了後（最初のbreak系pipe後・go-to-tab呼出前）のlist-tabs系応答で
    /// 指定tabのidを別idへ入れ替える（tab id再利用のemulation。TASK-75 TR75-4:
    /// step 6直前のtargets再解決〔pane所属基準〕が行われることの検証用注入）
    pub swap_tab_id_before_apply: Cell<Option<(TabId, TabId)>>,
    /// 同timingのlist-tabs系応答で指定tabをpaneごと除去する（tab消失により再解決
    /// 不能となる系列のemulation。TR75-4 fatal系）
    pub drop_tab_before_apply: Cell<Option<TabId>>,
    /// focus_paneを「Pane ... is already focused」のexit 2相当（focus状態は実際に
    /// 成立している）で応答する（E2E acceptance b系列実測のemulation。TASK-75
    /// 要因A改訂: already focusedは成功扱いであることの検証用注入）
    pub already_focused_focus: Cell<bool>,
    /// 最初のoverride-layout呼出より後のlist_panes_lenient応答のうち指定番目
    /// （1開始）の呼び出し時にだけ当該tabへpaneを1つ追加する（TASK-81
    /// reconciliation検証用: 安定観測の「未安定」系列——1回目と2回目のpane id
    /// 集合が異なる状態を作る。追加paneは恒久・末尾位置）
    pub spawn_dup_nth_after_override: Cell<Option<(TabId, usize)>>,
    /// close-paneを記録・成功扱いにするがpaneを除去しない（dup close漏れの
    /// emulation。TASK-81 (e'')検証用注入）
    pub mute_close_pane: Cell<bool>,
    /// override-layoutの未照合残留pane（過剰分）を末尾群でなく各slot直後
    /// （slot i paneと同一y・x=50）に割り込ませる配置注入（押し出し再現。
    /// TASK-81 E2E改訂F1検証用: 残留paneの介在でdup paneのvisual order順位が
    /// slot位置とずれる系列を作る。default model〔残留paneは末尾群配置〕は
    /// 維持し、既存条件の期待値を不変に保つ）
    pub interrupt_leftover_on_override: Cell<bool>,
    /// spawn_slot_paneでspawnするcommand系（FakeRun::Command）paneを即時
    /// exited/held化する注入（短命command dupのexit+hold化再現。TASK-81 E2E
    /// 改訂F2検証用: held化dupが生きpane集合から外れ、gate・対象集合拡張なしの
    /// 実装から逃避する系列を作る。bare spawn（正規spawn相当）はheld化しない）
    pub hold_spawned_command_dup: Cell<bool>,
    /// override-layoutのbare slot（commandなし・FakeRun::None）spawn paneに
    /// 付与するpane_cwd（/proc由来の現在cwd=クライアントcwdのemulation。TASK-81
    /// 再E2E R1検証用注入: t0p2 terminal_7ではbare slot spawn dupのpane_cwdが
    /// shell割当paneと同一のクライアントcwdになり属性照合の辺が複数slotに立つ。
    /// None（default）は従来どおりcwd無しで既存条件の期待値を不変に保つ）
    pub bare_spawn_cwd: RefCell<Option<String>>,
    /// override-layout後に当該tabへexited/held化したcommand系pane
    /// （terminal_command=Some・任意argv）を1つ追加する注入（TASK-81コード
    /// レビューC1検証用: F2 gate〔snapshot非存在のexited/held pane存在〕は発動
    /// するがargv一致逆引きにもbare突合にも掛からない「close対象なしの無害系列」
    /// を作る。terminal_command=Someのためbare候補数に寄与せずu=0が保たれる）
    pub spawn_held_extra_on_override: RefCell<Vec<(TabId, String)>>,
    /// watch用（TASK-86・設計§5.2）: pane毎のdump_frames。dump_screen呼出毎に
    /// 先頭から1件ずつ返し、尽きたら最終frameを継続して返す。未設定paneは
    /// 従来どおりSCREEN[pane]応答（既存条件の期待値不変）
    pub dump_frames: RefCell<HashMap<PaneKindId, VecDeque<String>>>,
    /// watch用: このpaneのdump_screen呼び出しが指定回数の成功を超えたら失敗させる
    /// （pane消滅模擬）
    pub dump_fail_after: RefCell<HashMap<PaneKindId, usize>>,
    /// watch用: full=true（dump-screen -f）検出時のみ返す応答（--full時の
    /// scrollback行の再現）。未設定paneはdump_frames/従来応答と同一
    pub dump_full_frames: RefCell<HashMap<PaneKindId, String>>,
    /// add session用（TASK-89）: create_session_detachedをOkのままlist-sessions
    /// 状態へ反映しない（rc=0・postcondition不在模擬）
    pub mute_create_session: Cell<bool>,
    /// add session用（TASK-93）: create_session_detached（attach -b）呼出を1回でも
    /// 観測した後のlist_sessions呼出をOperationFailedで失敗させる（事前確認は成功・
    /// 事後確認のみ失敗の系列。fail_opは先頭一致で事前確認側に当たるため専用arm）
    pub fail_list_sessions_after_create: Cell<bool>,
    /// attach呼出時点のstdout内容（/proc/self/fd/1の読み直し）を記録する注入
    /// （TASK-89 REVIEW-2指摘6: backend復帰前の無出力検証用。test側がfd 1を
    /// 一時fileへredirectした構成でのみ使用する——pipeだと読み取りがblockする
    /// ためarmed時のみ読む）
    pub probe_stdout_on_attach: Cell<bool>,
    /// arm_stdout_probe後のattach_session呼出時点のstdout内容
    pub stdout_at_attach: RefCell<Option<String>>,
    /// move用（TASK-88）: 指定番目（1開始）のbreak-id pipeだけ成功扱いだが効果を
    /// 反映しない（当該stepのpolling timeout模擬。swap部分失敗系列の注入）
    pub mute_break_id_nth: Cell<Option<usize>>,
    /// move用: 最初のbreak-id pipeより後のlist_panes_lenient応答の指定番目（1開始）
    /// の呼び出し時に、直近のbreak-idで移動したpaneのidを採番し直す（polling完了
    /// 後のpostcondition検証でpane id不変違反となる系列の注入。move.
    /// companion-postcondition-verification-failed）
    pub rename_moved_pane_after_break_nth: Cell<Option<usize>>,
    /// move用: 直近のbreak-id pipeで移動させたpane（rename_moved_pane_after_break_nth
    /// の適用対象追跡）
    pub last_break_moved: RefCell<Option<PaneKindId>>,
    /// move用: move_tabを記録・成功扱いにするがpositionを変えない（postcondition
    /// 検証〔VerificationFailed〕系列の注入。move.tab-swap-verification-failed）
    pub mute_move_tab: Cell<bool>,
    /// move用: 最初のbreak-id pipeより後のlist_panes_lenient呼出回数（1開始。
    /// rename_moved_pane_after_break_nthのarm用）
    pub lenient_panes_after_break: Cell<usize>,
    /// move用（REVIEW-3指摘2）: 最初のbreak-id pipeより後のlist_panes_lenient応答から
    /// 指定paneを除外する（移動自体は反映されるが観測だけ失敗する系列の注入。
    /// polling timeout後の再取得〔strict〕では移動済みが見える）
    pub hide_pane_in_lenient_after_break: Cell<Option<PaneKindId>>,
    /// move用（REVIEW-3指摘3）: 指定番目（1開始）のmove_tab呼出成功後に指定tabを
    /// paneごと除去する（tab交換の途中消滅系列の注入）
    pub vanish_tab_after_move_tab: Cell<Option<(usize, TabId)>>,
    /// remove session用（TASK-84）: delete_sessionをOkのままlist-sessions状態から
    /// 行を除去しない（rc=0・postcondition不一致模擬）
    pub mute_delete_session: Cell<bool>,
    /// remove session用（TASK-84）: delete_session（delete-session <name> -f）呼出を
    /// 1回でも観測した後のlist_sessions呼出を失敗させる（事前確認は成功・事後確認
    /// のみ失敗の系列。fail_opは先頭一致で事前確認側に当たるため専用arm）
    pub fail_list_sessions_after_delete: Cell<bool>,
    /// rename session用（TASK-95）: rename_sessionをOkのままlist-sessions状態
    /// （sessions行のname書き換え）へ反映しない（rc=0・黙失敗模擬。M4 I型）
    pub mute_rename_session: Cell<bool>,
    /// rename session用（TASK-95）: rename_session（rename-session@<from> <to>）呼出を
    /// 1回でも観測した後のlist_sessions呼出を失敗させる（事前確認は成功・事後確認
    /// のみ失敗の系列。fail_opは先頭一致で事前確認側に当たるため専用arm——
    /// fail_list_sessions_after_deleteと同型）
    pub fail_list_sessions_after_rename: Cell<bool>,
    /// rename session用（TASK-95レビュー対応）: rename_session呼出の状態作用を
    /// 差し替える事後状態注入（None = 通常の書き換え〔from行をtoへ・exited不変〕）
    pub rename_post_inject: Cell<Option<RenamePostInjection>>,
}

pub struct FakeState {
    pub panes: Vec<PaneState>,
    pub tabs: Vec<TabState>,
    pub next_terminal_id: u32,
    pub next_tab_id: u32,
    pub next_plugin_id: u32,
    /// paneのinvoked_with（起動時Run）side-table（TASK-81）。未登録paneは
    /// PaneState.terminal_command/cwdからderive（derive互換）
    pub invoked_with: HashMap<PaneKindId, FakeRun>,
    /// session一覧（TASK-89・add session用。従来のlist_sessions応答と同一内容を
    /// 既定とし、create_session_detachedでNAMEを追加する状態管理）
    pub sessions: Vec<SessionRef>,
}

impl FakeBackend {
    pub fn new(panes: Vec<PaneState>, tabs: Vec<TabState>) -> Self {
        let max_term = panes
            .iter()
            .filter_map(|p| match p.id {
                PaneKindId::Terminal(n) => Some(n),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let max_plugin = panes
            .iter()
            .filter_map(|p| match p.id {
                PaneKindId::Plugin(n) => Some(n),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let max_tab = tabs.iter().map(|t| t.id.0).max().unwrap_or(0);
        FakeBackend {
            state: RefCell::new(FakeState {
                panes,
                tabs,
                next_terminal_id: max_term + 1,
                next_tab_id: max_tab + 1,
                next_plugin_id: max_plugin + 1,
                invoked_with: HashMap::new(),
                sessions: vec![SessionRef {
                    name: "fake".into(),
                    created: Some("1m ago".into()),
                    current: true,
                    exited: false,
                }],
            }),
            calls: RefCell::new(Vec::new()),
            fail_op: RefCell::new(Vec::new()),
            fail_class: RefCell::new(None),
            drop_on_override: RefCell::new(Vec::new()),
            fail_restart: RefCell::new(Vec::new()),
            pipe_mute: RefCell::new(Vec::new()),
            spawn_on_override: RefCell::new(Vec::new()),
            override_count: Cell::new(0),
            fail_go_to_after: Cell::new(None),
            fail_override_layout_nth: Cell::new(None),
            slow_list_panes_after_pipe: Cell::new(None),
            mute_rename_tab: Cell::new(false),
            empty_panes_after_pipe: Cell::new(0),
            strict_empty_panes_after_pipe: Cell::new(0),
            empty_panes_after_override: Cell::new(0),
            empty_tabs_after_override: Cell::new(0),
            empty_panes_nth_after_override: Cell::new(None),
            empty_tabs_nth_after_override: Cell::new(None),
            lenient_panes_count: Cell::new(0),
            lenient_tabs_count: Cell::new(0),
            swap_tab_id_before_apply: Cell::new(None),
            drop_tab_before_apply: Cell::new(None),
            already_focused_focus: Cell::new(false),
            spawn_dup_nth_after_override: Cell::new(None),
            mute_close_pane: Cell::new(false),
            interrupt_leftover_on_override: Cell::new(false),
            hold_spawned_command_dup: Cell::new(false),
            bare_spawn_cwd: RefCell::new(None),
            spawn_held_extra_on_override: RefCell::new(Vec::new()),
            dump_frames: RefCell::new(HashMap::new()),
            dump_fail_after: RefCell::new(HashMap::new()),
            dump_full_frames: RefCell::new(HashMap::new()),
            mute_create_session: Cell::new(false),
            fail_list_sessions_after_create: Cell::new(false),
            probe_stdout_on_attach: Cell::new(false),
            stdout_at_attach: RefCell::new(None),
            mute_break_id_nth: Cell::new(None),
            rename_moved_pane_after_break_nth: Cell::new(None),
            last_break_moved: RefCell::new(None),
            mute_move_tab: Cell::new(false),
            lenient_panes_after_break: Cell::new(0),
            hide_pane_in_lenient_after_break: Cell::new(None),
            vanish_tab_after_move_tab: Cell::new(None),
            mute_delete_session: Cell::new(false),
            fail_list_sessions_after_delete: Cell::new(false),
            mute_rename_session: Cell::new(false),
            fail_list_sessions_after_rename: Cell::new(false),
            rename_post_inject: Cell::new(None),
        }
    }

    pub fn record(&self, op: &str) -> Result<(), ZelperError> {
        self.calls.borrow_mut().push(op.to_string());
        let mut fail = self.fail_op.borrow_mut();
        if let Some(pos) = fail.iter().position(|f| op.starts_with(f.as_str())) {
            fail.remove(pos);
            return Err(ZelperError::new(
                self.fail_class
                    .borrow()
                    .clone()
                    .unwrap_or(ErrorClass::OperationFailed),
                // 権限dialog pendingでpipe CLIがblockする状況のbackend timeout模擬
                // （DD-10.3 E6事実3）を含む注入
                format!("injected failure at '{op}'"),
            ));
        }
        Ok(())
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }

    /// pipe_pluginが既に呼ばれたか（busy window系注入〔CR74-2/3〕のarm条件。
    /// 最初のpipe = probe）
    fn pipe_seen(&self) -> bool {
        self.calls.borrow().iter().any(|c| c.starts_with("pipe "))
    }

    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn inject_failure(&self, op: &str) {
        self.fail_op.borrow_mut().push(op.to_string());
    }

    /// 失敗注入（inject_failure・fail_list_sessions_after_create）のerror classを
    /// 差し替える（TASK-93: add sessionのclass保持伝播をZellijUnavailableでも
    /// 検証するため。未設定はOperationFailed）
    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn inject_failure_as(&self, class: ErrorClass) {
        *self.fail_class.borrow_mut() = Some(class);
    }

    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn drop_on_next_override(&self, pane: PaneKindId) {
        self.drop_on_override.borrow_mut().push(pane);
    }

    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn starve_command_restart(&self, command: &str) {
        self.fail_restart.borrow_mut().push(command.to_string());
    }

    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn mute_pipe(&self, name: &str) {
        self.pipe_mute.borrow_mut().push(name.to_string());
    }

    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn spawn_extra_on_override(&self, tab: TabId) {
        self.spawn_on_override.borrow_mut().push(tab);
    }

    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn fail_go_to_tab_after_overrides(&self, after: usize) {
        self.fail_go_to_after.set(Some(after));
    }

    /// 指定番目（1開始）のoverride-layout呼出を失敗させる（TASK-70。移動phase完了
    /// 後の特定instance適用失敗 = worked example #7の系列注入。one-shot）
    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn fail_override_layout_nth(&self, nth: usize) {
        self.fail_override_layout_nth.set(Some(nth));
    }

    /// companion setup失敗注入（TASK-70）: remap::run経由のcompanion setupをwasm
    /// extract失敗で中断させる。companion setup（wasm extract・permissions seed）は
    /// backend呼出でなくcompanion.rsのfilesystem処理のため、隔離XDG_CACHE_HOME配下
    /// の`zelper`を通常fileとして塞ぎcache dir作成を失敗させる（事前配置形式。
    /// 未設定時は従来挙動不変）
    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn fail_companion_wasm_extract(&self, xdg_root: &Path) {
        std::fs::write(xdg_root.join("zelper"), b"")
            .expect("arm: $XDG_CACHE_HOME/zelperを通常fileで塞ぐ");
    }

    /// companion setup失敗注入（TASK-70）: remap::run経由のcompanion setupを
    /// permissions seed失敗で中断させる。zellij/permissions.kdlに不正KDLを事前配置
    /// してparse失敗（Preflight）させる（事前配置形式。未設定時は従来挙動不変）
    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn fail_companion_permissions_seed(&self, xdg_root: &Path) {
        let dir = xdg_root.join("zellij");
        std::fs::create_dir_all(&dir).expect("arm: zellij cache dir作成");
        std::fs::write(dir.join("permissions.kdl"), "broken {\n")
            .expect("arm: 不正KDLのpermissions.kdl事前配置");
    }

    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn slow_list_panes_after_pipe(&self, delay: std::time::Duration) {
        self.slow_list_panes_after_pipe.set(Some(delay));
    }

    #[allow(dead_code)] // 一部のtest binaryから未使用になりうる
    pub fn mute_rename_tab(&self) {
        self.mute_rename_tab.set(true);
    }

    /// 最初のpipe呼出以降にbusy windowを発生させる: lenient list-panes応答の
    /// 先頭N回を空（Ok(None)）にし、strict list_panes応答の先頭N回を空stdout
    /// 相当の失敗にする（TASK-74。CR74-3のarm法。実backendでは空stdoutは
    /// lenientでOk(None)・strictでparse失敗となるため両方に掛ける）
    #[allow(dead_code)]
    pub fn empty_panes_after_pipe(&self, count: usize) {
        self.empty_panes_after_pipe.set(count);
        self.strict_empty_panes_after_pipe.set(count);
    }

    #[allow(dead_code)]
    pub fn empty_panes_after_override(&self, count: usize) {
        self.empty_panes_after_override.set(count);
    }

    #[allow(dead_code)]
    pub fn empty_tabs_after_override(&self, count: usize) {
        self.empty_tabs_after_override.set(count);
    }

    /// 最初のoverride-layout呼出より後のlist_panes_lenient応答のうち指定番目
    /// （1開始）の呼び出しのみ空にする（TASK-75 E2E要因A改訂の精密arm法）
    #[allow(dead_code)]
    pub fn empty_panes_nth_after_override(&self, nth: usize) {
        self.empty_panes_nth_after_override.set(Some(nth));
    }

    /// 同型のtabs側精密arm
    #[allow(dead_code)]
    pub fn empty_tabs_nth_after_override(&self, nth: usize) {
        self.empty_tabs_nth_after_override.set(Some(nth));
    }

    /// move phase完了後（break系pipe後・最初のgo-to-tab前）のlist-tabs系応答で
    /// tab idを入れ替える（TR75-4。v2.2実装のstep 6直前存在確認経路で消費される。
    /// 現行実装はこのwindowでlist-tabsを呼ばないため適用されず残る）
    #[allow(dead_code)]
    pub fn swap_tab_id_before_apply(&self, old: TabId, new: TabId) {
        self.swap_tab_id_before_apply.set(Some((old, new)));
    }

    /// 同timingのlist-tabs系応答でtabをpaneごと除去する（TR75-4 fatal系）
    #[allow(dead_code)]
    pub fn drop_tab_before_apply(&self, tab: TabId) {
        self.drop_tab_before_apply.set(Some(tab));
    }

    /// focus_pane応答を「already focused」のexit 2相当へ切替（TASK-75要因A改訂の
    /// 検証用注入。focus状態は設定したうえでzellij実機と同じerrorを返す）
    #[allow(dead_code)]
    pub fn already_focused_focus(&self) {
        self.already_focused_focus.set(true);
    }

    /// paneのinvoked_with（起動時Run）を明示設定（TASK-81。Run::Cwd系shell・cwd/hold
    /// 付きCommand等、deriveでは表現できない起動系列のfixture用）
    #[allow(dead_code)]
    pub fn set_invoked_with(&self, pane: PaneKindId, run: FakeRun) {
        self.state.borrow_mut().invoked_with.insert(pane, run);
    }

    /// 最初のoverride-layout呼出より後のlist_panes_lenient応答の指定番目（1開始）の
    /// 呼び出し時にだけ当該tabへpaneを1つ追加する（安定観測の「未安定」系列注入。
    /// 1回目と2回目のpane id集合が異なる状態を作る）
    #[allow(dead_code)]
    pub fn spawn_dup_nth_after_override(&self, tab: TabId, nth: usize) {
        self.spawn_dup_nth_after_override.set(Some((tab, nth)));
    }

    /// close-paneを成功扱いにするがpaneを除去しない（dup close漏れのemulation）
    #[allow(dead_code)]
    pub fn mute_close_pane(&self) {
        self.mute_close_pane.set(true);
    }

    /// override-layoutの未照合残留paneを各slot直後に割り込ませる配置に切替
    /// （押し出し再現。TASK-81 E2E改訂F1: 残留paneの介在でdup paneのvisual
    /// order順位がslot位置とずれる系列の注入）
    #[allow(dead_code)]
    pub fn interrupt_leftover_on_override(&self) {
        self.interrupt_leftover_on_override.set(true);
    }

    /// spawnされるcommand系dupを即時exited/held化（短命command dupの
    /// exit+hold化再現。TASK-81 E2E改訂F2: t0q qp8系列の注入）
    #[allow(dead_code)]
    pub fn hold_spawned_command_dup(&self) {
        self.hold_spawned_command_dup.set(true);
    }

    /// bare slot（commandなし）spawn paneのpane_cwdを設定（/proc由来の現在cwd
    /// =クライアントcwdのemulation。TASK-81再E2E R1: t0p2 terminal_7系列——
    /// 同一cwd shell slot複数+dupで属性照合が曖昧になる系列の注入）
    #[allow(dead_code)]
    pub fn set_bare_spawn_cwd(&self, cwd: &str) {
        *self.bare_spawn_cwd.borrow_mut() = Some(cwd.to_string());
    }

    /// 次回以降のoverride-layout後に当該tabへ、exited/held化したcommand系pane
    /// （terminal_command=command・cwd=None・末尾位置）を1つ追加する（TASK-81
    /// コードレビューC1: gateは発動するがreconciliationのclose対象にならない
    /// 無害系列の注入）
    #[allow(dead_code)]
    pub fn spawn_held_extra_on_override(&self, tab: TabId, command: &str) {
        self.spawn_held_extra_on_override
            .borrow_mut()
            .push((tab, command.to_string()));
    }

    /// watch用: paneのdump_screen応答frame列を設定（呼出毎に1件ずつ消費し
    /// 尽きたら最終frameを継続。TASK-86）
    #[allow(dead_code)]
    pub fn set_dump_frames(&self, pane: PaneKindId, frames: Vec<String>) {
        self.dump_frames
            .borrow_mut()
            .insert(pane, VecDeque::from(frames));
    }

    /// watch用: dump_screenの成功回数を制限し、指定回数を超えた呼び出しを失敗
    /// させる（pane消滅模擬。TASK-86）
    #[allow(dead_code)]
    pub fn set_dump_fail_after(&self, pane: PaneKindId, ok_calls: usize) {
        self.dump_fail_after.borrow_mut().insert(pane, ok_calls);
    }

    /// watch用: full=true検出時のみのdump_screen応答を設定（--full時のscrollback
    /// 行の再現。TASK-86）
    #[allow(dead_code)]
    pub fn set_dump_full(&self, pane: PaneKindId, content: String) {
        self.dump_full_frames.borrow_mut().insert(pane, content);
    }

    /// add session用: create_session_detachedをOkのままlist-sessions状態へ反映
    /// しない（rc=0・postcondition不在模擬。TASK-89）
    #[allow(dead_code)]
    pub fn mute_create_session(&self) {
        self.mute_create_session.set(true);
    }

    /// add session用（TASK-93）: 事後確認（postcheck）のlist-sessions呼出のみを
    /// 失敗させる（事前確認は成功・作成呼出は成功済みの系列）
    #[allow(dead_code)]
    pub fn fail_list_sessions_after_create(&self) {
        self.fail_list_sessions_after_create.set(true);
    }

    /// attach呼出時点のstdout内容の記録を有効化（TASK-89 REVIEW-2指摘6。
    /// test側がfd 1を一時fileへredirectしている構成でのみ使用）
    #[allow(dead_code)]
    pub fn arm_stdout_probe(&self) {
        self.probe_stdout_on_attach.set(true);
    }

    /// arm_stdout_probe後の直近attach_session呼出時点のstdout内容
    /// （読み取り失敗時はNone）
    #[allow(dead_code)]
    pub fn stdout_at_attach(&self) -> Option<String> {
        self.stdout_at_attach.borrow().clone()
    }

    /// move用: 指定番目（1開始）のbreak-id pipeを成功扱いだが効果不出現にする
    /// （TASK-88。swap 2step目失敗の部分状態報告系列）
    #[allow(dead_code)]
    pub fn mute_break_id_nth(&self, nth: usize) {
        self.mute_break_id_nth.set(Some(nth));
    }

    /// move用: 最初のbreak-id pipeより後の指定番目（1開始）のlist_panes_lenient
    /// 呼出時に、直近のbreak-id移動paneのidを採番し直す（polling完了後のpane id
    /// 不変違反系列）
    #[allow(dead_code)]
    pub fn rename_moved_pane_after_break_nth(&self, nth: usize) {
        self.rename_moved_pane_after_break_nth.set(Some(nth));
    }

    /// move用: move_tabを記録・成功扱いにするがpositionを変えない
    #[allow(dead_code)]
    pub fn mute_move_tab(&self) {
        self.mute_move_tab.set(true);
    }

    /// move用（REVIEW-3指摘2）: 最初のbreak-id pipeより後のlist_panes_lenient応答から
    /// 指定paneを除外する（移動は反映されるが観測だけ失敗する系列）
    #[allow(dead_code)]
    pub fn hide_pane_in_lenient_after_break(&self, pane: PaneKindId) {
        self.hide_pane_in_lenient_after_break.set(Some(pane));
    }

    /// move用（REVIEW-3指摘3）: 指定番目（1開始）のmove_tab呼出成功後に指定tabを
    /// paneごと除去する（tab交換の途中消滅系列）
    #[allow(dead_code)]
    pub fn vanish_tab_after_move_tab(&self, nth: usize, tab: TabId) {
        self.vanish_tab_after_move_tab.set(Some((nth, tab)));
    }

    /// remove session用（TASK-84）: delete_sessionをOkのままlist-sessions状態から
    /// 行を除去しない（rc=0・postcondition不一致模擬）
    #[allow(dead_code)]
    pub fn mute_delete_session(&self) {
        self.mute_delete_session.set(true);
    }

    /// remove session用（TASK-84）: 事後確認（postcheck）のlist-sessions呼出のみを
    /// 失敗させる（事前確認は成功・delete-session呼出は成功済みの系列）
    #[allow(dead_code)]
    pub fn fail_list_sessions_after_delete(&self) {
        self.fail_list_sessions_after_delete.set(true);
    }

    /// rename session用（TASK-95）: rename_sessionをOkのままlist-sessions状態
    /// （sessions行のname書き換え）へ反映しない（rc=0・黙失敗模擬）
    #[allow(dead_code)]
    pub fn mute_rename_session(&self) {
        self.mute_rename_session.set(true);
    }

    /// rename session用（TASK-95）: 事後確認（postcheck）のlist-sessions呼出のみを
    /// 失敗させる（事前確認は成功・rename-session呼出は成功済みの系列）
    #[allow(dead_code)]
    pub fn fail_list_sessions_after_rename(&self) {
        self.fail_list_sessions_after_rename.set(true);
    }

    /// rename session用（TASK-95レビュー対応）: rename_session呼出の状態作用を
    /// 差し替える（postcondition各判定の独立保護検証用の事後状態注入）
    #[allow(dead_code)]
    pub fn inject_rename_post(&self, inj: RenamePostInjection) {
        self.rename_post_inject.set(Some(inj));
    }

    /// remove session用（TASK-84）: session一覧の初期状態を差し替える
    /// （EXITED行混在・複数稼働等のfixture。既定はfake 1件）
    #[allow(dead_code)]
    pub fn set_sessions(&self, sessions: Vec<SessionRef>) {
        self.state.borrow_mut().sessions = sessions;
    }

    /// watch用: paneへのdump_screen呼び出し回数（記録から計上。-f付き呼び出しを
    /// 含む。TASK-86）
    pub fn dump_calls(&self, pane: &PaneKindId) -> usize {
        let spec = format!("dump-screen {}", pane.as_spec());
        let with_f = format!("{spec} ");
        self.calls
            .borrow()
            .iter()
            .filter(|c| c.as_str() == spec || c.starts_with(&with_f))
            .count()
    }

    /// armed state（break系pipe呼出済み・go-to-tab未呼び出し）のlist-tabs系呼び出し
    /// でtab id操作注入を適用する（1回で消費）
    fn apply_tab_id_injection(&self) {
        let armed = {
            let calls = self.calls.borrow();
            calls.iter().any(|c| c.starts_with("pipe break-"))
                && !calls.iter().any(|c| c.starts_with("go-to-tab"))
        };
        if !armed {
            return;
        }
        if let Some((old, new)) = self.swap_tab_id_before_apply.get() {
            self.swap_tab_id_before_apply.set(None);
            let mut s = self.state.borrow_mut();
            if let Some(t) = s.tabs.iter_mut().find(|t| t.id == old) {
                t.id = new;
            }
            for p in s.panes.iter_mut() {
                if p.tab_id == old {
                    p.tab_id = new;
                }
            }
        }
        if let Some(drop) = self.drop_tab_before_apply.get() {
            self.drop_tab_before_apply.set(None);
            let mut s = self.state.borrow_mut();
            s.tabs.retain(|t| t.id != drop);
            s.panes.retain(|p| p.tab_id != drop);
        }
    }

    /// move用（TASK-88）: 最初のbreak-id pipeより後のlist_panes系呼出を計数し、
    /// 指定番目の呼び出し時に直近のbreak-id移動paneのidを採番し直す（polling完了
    /// 条件はtab所属のみのため、完了後の再取得でpane id不変違反として検出される
    /// 系列の注入。move.companion-postcondition-verification-failed）
    fn apply_break_rename_injection(&self) {
        let armed = self
            .calls
            .borrow()
            .iter()
            .any(|c| c.starts_with("pipe break-id"));
        if !armed {
            return;
        }
        let n = self.lenient_panes_after_break.get() + 1;
        self.lenient_panes_after_break.set(n);
        if let Some(nth) = self.rename_moved_pane_after_break_nth.get()
            && n == nth
        {
            self.rename_moved_pane_after_break_nth.set(None);
            if let Some(moved) = *self.last_break_moved.borrow() {
                let mut s = self.state.borrow_mut();
                let new_id = s.next_terminal_id;
                s.next_terminal_id += 1;
                let run = s.invoked_with.remove(&moved);
                let renamed = s.panes.iter_mut().find(|p| p.id == moved).map(|p| {
                    p.id = PaneKindId::Terminal(new_id);
                    p.id
                });
                if let (Some(new_key), Some(run)) = (renamed, run) {
                    s.invoked_with.insert(new_key, run);
                }
            }
        }
    }

    /// pane群をtabへ移動（E6実証: プロセス保存・pane id不変）。
    /// 移動元tabがselectable tiled terminal paneを失い空になったら自動closeする
    /// （E6事実1の模擬）
    fn move_panes(s: &mut FakeState, ids: &[PaneKindId], tab: TabId) {
        let Some(t) = s.tabs.iter().find(|t| t.id == tab) else {
            return;
        };
        let (name, pos) = (t.name.clone(), t.position);
        for p in s.panes.iter_mut() {
            if ids.contains(&p.id) {
                p.tab_id = tab;
                p.tab_name = name.clone();
                p.tab_position = pos;
            }
        }
        let occupied: Vec<TabId> = s
            .panes
            .iter()
            .filter(|p| p.is_remap_source())
            .map(|p| p.tab_id)
            .collect();
        s.tabs.retain(|t| occupied.contains(&t.id));
        // 全tabがcloseされるとlist-tabsが空になる。実zellijでは最終tabの pane喪失で
        // sessionが終了するが、fakeではactive tabだけは保持する（テスト安定化）
        if s.tabs.is_empty() {
            s.tabs.push(TabState {
                id: tab,
                position: pos,
                name,
                active: true,
                selectable_tiled_panes_count: 0,
                selectable_floating_panes_count: 0,
                are_floating_panes_visible: true,
            });
        }
    }
}

/// pipe payloadのpane id csv（`terminal_4,terminal_5` 等）をparse。
/// 区切り文字（`,` / 空白）は実装の選択に依存させないため両方受理する
fn parse_pane_ids(csv: &str) -> Vec<PaneKindId> {
    csv.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .filter_map(PaneKindId::parse_spec)
        .collect()
}

/// paneのinvoked_with（起動時Run）。side-table（invoked_with map）優先・未設定paneは
/// PaneState.terminal_commandからderiveする（derive互換: Some→Command{argv: 空白分割,
/// cwd: None}・None→None。cwdを含む照合を行うTASK-81系条件はset_invoked_withで明示設定）
fn invoked_of(s: &FakeState, p: &PaneState) -> FakeRun {
    if let Some(r) = s.invoked_with.get(&p.id) {
        return r.clone();
    }
    match &p.terminal_command {
        Some(c) => FakeRun::Command {
            argv: c.split_whitespace().map(str::to_string).collect(),
            cwd: None,
            hold_on_close: true,
            hold_on_start: false,
        },
        None => FakeRun::None,
    }
}

/// 生成KDL slotのrun表現で新規paneをspawnする（run等値照合不成立slotのemulation。
/// TASK-81）。invoked_with = slot表現・geometryはslot位置（y=slot番号）へ置き、
/// 「当該tabのvisual order s番目 = slot s」の対応をfake内で決定的にする
fn spawn_slot_pane(s: &mut FakeState, tab: TabId, slot: usize, run: FakeRun) -> PaneKindId {
    let pid = s.next_terminal_id;
    s.next_terminal_id += 1;
    let (tab_name, tab_pos) = s
        .tabs
        .iter()
        .find(|t| t.id == tab)
        .map(|t| (t.name.clone(), t.position))
        .unwrap_or_default();
    let (terminal_command, cwd) = match &run {
        FakeRun::Command { argv, cwd, .. } => (
            Some(argv.join(" ")),
            cwd.as_ref().map(|p| p.to_string_lossy().into_owned()),
        ),
        FakeRun::Cwd(cwd) => (None, Some(cwd.to_string_lossy().into_owned())),
        FakeRun::None => (None, None),
    };
    let id = PaneKindId::Terminal(pid);
    s.panes.push(PaneState {
        id,
        title: format!("Pane #{pid}"),
        is_selectable: true,
        is_floating: false,
        is_focused: false,
        exited: false,
        is_held: false,
        geometry: Geometry {
            x: 0,
            y: slot as u32,
            rows: 10,
            cols: 10,
        },
        command: None,
        terminal_command,
        cwd,
        tab_id: tab,
        tab_position: tab_pos,
        tab_name,
        plugin_url: None,
    });
    s.invoked_with.insert(id, run);
    id
}

fn spawn_shell_pane(s: &mut FakeState, tab: TabId, title: &str) -> PaneKindId {
    let pid = s.next_terminal_id;
    s.next_terminal_id += 1;
    let (tab_name, tab_pos) = s
        .tabs
        .iter()
        .find(|t| t.id == tab)
        .map(|t| (t.name.clone(), t.position))
        .unwrap_or_default();
    // TASK-75（E2E要因A改訂）: focus対象・検証(d)は適用後のvisual order（y,x昇順）
    // で決まるため、spawn pane（空slot相当）は当該tabの既存terminal paneより常に
    // 後ろに来る位置（max y + 1行）へ置く。割当paneのvisual order = mapping slot順・
    // spawn paneは空slot、の対応をfake内で決定的にする
    let y = s
        .panes
        .iter()
        .filter(|p| p.tab_id == tab && p.is_remap_source())
        .map(|p| p.geometry.y)
        .max()
        .unwrap_or(0)
        + 1;
    s.panes.push(PaneState {
        id: PaneKindId::Terminal(pid),
        title: title.to_string(),
        is_selectable: true,
        is_floating: false,
        is_focused: false,
        exited: false,
        is_held: false,
        geometry: Geometry {
            x: 0,
            y,
            rows: 10,
            cols: 10,
        },
        command: None,
        terminal_command: None,
        cwd: None,
        tab_id: tab,
        tab_position: tab_pos,
        tab_name,
        plugin_url: None,
    });
    PaneKindId::Terminal(pid)
}

/// 1次元範囲の重複判定（境界接続は隣接候補の条件に含めない——範囲の重複のみ）
fn overlaps_1d(a0: u32, a1: u32, b0: u32, b1: u32) -> bool {
    a0 < b1 && b0 < a1
}

/// M1実測基準の幾何隣接則（REVIEW-3指摘1）: 対象pane（group[idx]）の方向dにおける
/// 交換相手のgroup内indexを返す。候補なし（端）はNone = no-op。
/// left/right と up/down は直交軸の重複を要求するため同一視しない
/// （M1実測: C left→A〔左の大pane〕だが C up→B〔同一列の上〕・A right→C〔右側の
/// 下段〕——visual orderの前後とは異なる交換相手）
fn neighbor_in_direction(
    group: &[(PaneKindId, Geometry, bool)],
    idx: usize,
    d: ResizeDirection,
) -> Option<usize> {
    let (_, t, _) = group[idx];
    let (tx0, tx1) = (t.x, t.x.saturating_add(t.cols));
    let (ty0, ty1) = (t.y, t.y.saturating_add(t.rows));
    let mut best: Option<(usize, (u32, u32))> = None; // (index, 優先key)
    for (i, (_, g, _)) in group.iter().enumerate() {
        if i == idx {
            continue;
        }
        let (gx0, gx1) = (g.x, g.x.saturating_add(g.cols));
        let (gy0, gy1) = (g.y, g.y.saturating_add(g.rows));
        let key: Option<(u32, u32)> = match d {
            // 左に位置（右端が対象の左端以下）しy範囲が重複: 右端が最大のもの
            ResizeDirection::Left if gx1 <= tx0 && overlaps_1d(gy0, gy1, ty0, ty1) => {
                Some((gx1, gy0))
            }
            // 右に位置（左端が対象の右端以上）しy範囲が重複: 左端が最小・
            // 同値は上端が最大のもの（M1実測: A right は B でなく C）
            ResizeDirection::Right if gx0 >= tx1 && overlaps_1d(gy0, gy1, ty0, ty1) => {
                Some((u32::MAX - gx0, gy0))
            }
            // 上に位置しx範囲が重複: 下端が最大のもの
            ResizeDirection::Up if gy1 <= ty0 && overlaps_1d(gx0, gx1, tx0, tx1) => {
                Some((gy1, gx0))
            }
            // 下に位置しx範囲が重複: 上端が最小のもの
            ResizeDirection::Down if gy0 >= ty1 && overlaps_1d(gx0, gx1, tx0, tx1) => {
                Some((u32::MAX - gy0, gx0))
            }
            _ => None,
        };
        if let Some(key) = key
            && best.is_none_or(|(_, bk)| key > bk)
        {
            best = Some((i, key));
        }
    }
    best.map(|(i, _)| i)
}

impl ZellijBackend for FakeBackend {
    fn version(&self) -> Result<String, ZelperError> {
        self.record("version")?;
        Ok("zellij 0.44.3\n".to_string())
    }

    fn list_sessions(&self) -> Result<Vec<SessionRef>, ZelperError> {
        self.record("list-sessions")?;
        // TASK-93: 事後確認のみ失敗させる注入（arm後・作成呼出観測後のlist-sessions）
        if self.fail_list_sessions_after_create.get()
            && self
                .calls
                .borrow()
                .iter()
                .any(|c| c.starts_with("attach -b "))
        {
            return Err(ZelperError::new(
                self.fail_class
                    .borrow()
                    .clone()
                    .unwrap_or(ErrorClass::OperationFailed),
                "injected failure at 'list-sessions' (postcheck)".to_string(),
            ));
        }
        // TASK-84: remove sessionの事後確認のみ失敗させる注入（arm後・
        // delete-session呼出観測後のlist-sessions。事前確認側には当てない）
        if self.fail_list_sessions_after_delete.get()
            && self
                .calls
                .borrow()
                .iter()
                .any(|c| c.starts_with("delete-session "))
        {
            return Err(ZelperError::new(
                self.fail_class
                    .borrow()
                    .clone()
                    .unwrap_or(ErrorClass::OperationFailed),
                "injected failure at 'list-sessions' (postcheck)".to_string(),
            ));
        }
        // TASK-95: rename sessionの事後確認のみ失敗させる注入（arm後・
        // rename-session@呼出観測後のlist-sessions。事前確認側には当てない）
        if self.fail_list_sessions_after_rename.get()
            && self
                .calls
                .borrow()
                .iter()
                .any(|c| c.starts_with("rename-session@"))
        {
            return Err(ZelperError::new(
                self.fail_class
                    .borrow()
                    .clone()
                    .unwrap_or(ErrorClass::OperationFailed),
                "injected failure at 'list-sessions' (postcheck)".to_string(),
            ));
        }
        Ok(self.state.borrow().sessions.clone())
    }

    fn list_tabs(&self) -> Result<Vec<TabState>, ZelperError> {
        self.record("list-tabs")?;
        self.apply_tab_id_injection();
        Ok(self.state.borrow().tabs.clone())
    }

    fn list_tabs_lenient(&self) -> Result<Option<Vec<TabState>>, ZelperError> {
        self.record("list-tabs-lenient")?;
        // 検証冒頭（override-layout以降）へ空を届けるbusy window注入（CR74-3）
        if self.override_count.get() > 0 {
            let remaining = self.empty_tabs_after_override.get();
            if remaining > 0 {
                self.empty_tabs_after_override.set(remaining - 1);
                return Ok(None);
            }
            // 精密arm（TASK-75 E2E要因A改訂）: 指定番目の呼び出しのみ空
            let n = self.lenient_tabs_count.get() + 1;
            self.lenient_tabs_count.set(n);
            if self.empty_tabs_nth_after_override.get() == Some(n) {
                return Ok(None);
            }
        }
        self.apply_tab_id_injection();
        Ok(Some(self.state.borrow().tabs.clone()))
    }

    fn list_tabs_for(&self, session: &str) -> Result<Vec<TabState>, ZelperError> {
        self.record(&format!("list-tabs@{session}"))?;
        Ok(self.state.borrow().tabs.clone())
    }

    fn list_panes(&self) -> Result<Vec<PaneState>, ZelperError> {
        self.record("list-panes")?;
        // busy window中のstrict呼び出しは空stdout相当（実backendのlist_panesは
        // 空出力でparse失敗。CR74-2の窓検出用注入）
        if self.pipe_seen() {
            let remaining = self.strict_empty_panes_after_pipe.get();
            if remaining > 0 {
                self.strict_empty_panes_after_pipe.set(remaining - 1);
                return Err(ZelperError::new(
                    ErrorClass::OperationFailed,
                    "failed to parse list-panes output: empty response (injected)",
                ));
            }
        }
        self.apply_break_rename_injection();
        Ok(self.state.borrow().panes.clone())
    }

    fn list_panes_lenient(&self) -> Result<Option<Vec<PaneState>>, ZelperError> {
        self.record("list-panes-lenient")?;
        // 最初のpipe呼出より後の最初のlenient list-panesを遅延させる（C3注入:
        // backend呼出がdeadlineを消費してから条件成立を返す系列の模擬）。
        if let Some(delay) = self.slow_list_panes_after_pipe.get()
            && self.calls.borrow().iter().any(|c| c.starts_with("pipe "))
        {
            self.slow_list_panes_after_pipe.set(None);
            std::thread::sleep(delay);
        }
        // busy window注入（CR74-3）: 最初のpipe以降のlenient応答の先頭N回は空。
        // run冒頭snapshot（pipeより前）では消費されずpollに実際に空が届く
        if self.pipe_seen() {
            let remaining = self.empty_panes_after_pipe.get();
            if remaining > 0 {
                self.empty_panes_after_pipe.set(remaining - 1);
                return Ok(None);
            }
        }
        // 検証冒頭（override-layout以降）へ空を届けるbusy window注入（CR74-3）
        if self.override_count.get() > 0 {
            let remaining = self.empty_panes_after_override.get();
            if remaining > 0 {
                self.empty_panes_after_override.set(remaining - 1);
                return Ok(None);
            }
            // 精密arm（TASK-75 E2E要因A改訂）: 指定番目の呼び出しのみ空
            let n = self.lenient_panes_count.get() + 1;
            self.lenient_panes_count.set(n);
            if self.empty_panes_nth_after_override.get() == Some(n) {
                return Ok(None);
            }
            // TASK-81: 安定観測の「未安定」系列注入——指定番目の呼び出し時にだけ
            // 当該tabへpaneを1つ追加する（1回目と2回目のpane id集合が異なる状態）
            if let Some((tab, nth)) = self.spawn_dup_nth_after_override.get()
                && nth == n
            {
                self.spawn_dup_nth_after_override.set(None);
                let mut s = self.state.borrow_mut();
                spawn_shell_pane(&mut s, tab, "Pane (unstable)");
            }
        }
        self.apply_break_rename_injection();
        // REVIEW-3指摘2: 移動自体は反映されているがlenient観測だけ失敗する系列の
        // 注入（break-id pipe後のlenient応答から指定paneを除外。strict list_panes
        // では見えるため、polling timeout後の再取得が「移動済み」を観測する）
        if self
            .calls
            .borrow()
            .iter()
            .any(|c| c.starts_with("pipe break-id"))
            && let Some(hidden) = self.hide_pane_in_lenient_after_break.get()
        {
            return Ok(Some(
                self.state
                    .borrow()
                    .panes
                    .iter()
                    .filter(|p| p.id != hidden)
                    .cloned()
                    .collect(),
            ));
        }
        Ok(Some(self.state.borrow().panes.clone()))
    }

    fn current_tab(&self) -> Result<TabState, ZelperError> {
        self.record("current-tab")?;
        self.state
            .borrow()
            .tabs
            .iter()
            .find(|t| t.active)
            .cloned()
            .ok_or_else(|| ZelperError::new(ErrorClass::OperationFailed, "no active tab"))
    }

    /// TASK-86（watch）: dump_frames/dump_fail_after/dump_full_frames注入に対応。
    /// 未設定pane・未設定項目は従来挙動のまま（既存条件の期待値不変）。full=trueの
    /// 呼び出しは記録に"-f"を付ける（既存L2条件はfull=falseのみのため影響なし）
    fn dump_screen(&self, pane: &PaneKindId, full: bool) -> Result<String, ZelperError> {
        let spec = pane.as_spec();
        self.record(&format!(
            "dump-screen {spec}{}",
            if full { " -f" } else { "" }
        ))?;
        // 消滅模擬: 成功回数の上限を超えた呼び出しは失敗する
        let n = self.dump_calls(pane);
        if let Some(ok_calls) = self.dump_fail_after.borrow().get(pane)
            && n > *ok_calls
        {
            return Err(ZelperError::new(
                ErrorClass::OperationFailed,
                format!("pane {spec} vanished (injected)"),
            ));
        }
        if full && let Some(c) = self.dump_full_frames.borrow().get(pane) {
            return Ok(c.clone());
        }
        let mut frames = self.dump_frames.borrow_mut();
        if let Some(dq) = frames.get_mut(pane) {
            // 呼出毎に先頭frameを1件消費。尽きたら最終frameを継続して返す
            if dq.len() > 1 {
                dq.pop_front()
            } else {
                dq.front().cloned()
            }
            .map(Ok)
            .unwrap_or_else(|| Ok(format!("SCREEN[{spec}]\n")))
        } else {
            Ok(format!("SCREEN[{spec}]\n"))
        }
    }

    fn write_chars(&self, pane: &PaneKindId, text: &str) -> Result<(), ZelperError> {
        self.record(&format!("write-chars {} {}", pane.as_spec(), text))?;
        Ok(())
    }

    fn write_bytes(&self, pane: &PaneKindId, bytes: &[u8]) -> Result<(), ZelperError> {
        self.record(&format!(
            "write {} {}",
            pane.as_spec(),
            bytes
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        ))?;
        Ok(())
    }

    fn send_keys(&self, pane: &PaneKindId, keys: &[String]) -> Result<(), ZelperError> {
        self.record(&format!("send-keys {} {}", pane.as_spec(), keys.join(" ")))?;
        Ok(())
    }

    fn rename_pane(&self, pane: &PaneKindId, name: &str) -> Result<(), ZelperError> {
        self.record(&format!("rename-pane {} {}", pane.as_spec(), name))?;
        let mut s = self.state.borrow_mut();
        if let Some(p) = s.panes.iter_mut().find(|p| &p.id == pane) {
            p.title = name.to_string();
            Ok(())
        } else {
            Err(ZelperError::new(
                ErrorClass::OperationFailed,
                "pane not found",
            ))
        }
    }

    fn rename_tab(&self, tab: TabId, name: &str) -> Result<(), ZelperError> {
        self.record(&format!("rename-tab {} {}", tab.0, name))?;
        if self.mute_rename_tab.get() {
            // 呼び出し自体は成功扱いだが名前を変えない（C5注入:
            // 同名tab存在のみの検証では誤通過することの回帰検出用）
            return Ok(());
        }
        let mut s = self.state.borrow_mut();
        if let Some(t) = s.tabs.iter_mut().find(|t| t.id == tab) {
            t.name = name.to_string();
            for p in s.panes.iter_mut().filter(|p| p.tab_id == tab) {
                p.tab_name = name.to_string();
            }
            Ok(())
        } else {
            Err(ZelperError::new(
                ErrorClass::OperationFailed,
                "tab not found",
            ))
        }
    }

    fn new_pane(&self, spec: &NewPaneSpec) -> Result<PaneKindId, ZelperError> {
        self.record(&format!(
            "new-pane tab={:?} name={:?} cmd={:?}",
            spec.tab.map(|t| t.0),
            spec.name,
            spec.command
        ))?;
        let mut s = self.state.borrow_mut();
        let id = s.next_terminal_id;
        s.next_terminal_id += 1;
        let pane = PaneState {
            id: PaneKindId::Terminal(id),
            title: spec.name.clone().unwrap_or_else(|| format!("Pane #{id}")),
            is_selectable: true,
            is_floating: false,
            is_focused: true,
            exited: false,
            is_held: false,
            geometry: Geometry {
                x: 0,
                y: 0,
                rows: 10,
                cols: 10,
            },
            command: if spec.command.is_empty() {
                None
            } else {
                Some(spec.command.join(" "))
            },
            terminal_command: None,
            cwd: spec.cwd.as_ref().map(|p| p.to_string_lossy().into_owned()),
            tab_id: spec.tab.unwrap_or(TabId(0)),
            tab_position: 0,
            tab_name: "Tab #1".into(),
            plugin_url: None,
        };
        s.panes.push(pane);
        Ok(PaneKindId::Terminal(id))
    }

    fn new_tab(&self, spec: &NewTabSpec) -> Result<TabId, ZelperError> {
        self.record(&format!(
            "new-tab name={:?} layout={:?}",
            spec.name,
            spec.layout.as_ref().map(|l| match &l.inline {
                Some(k) => format!("inline:{}", k.lines().next().unwrap_or("")),
                _ => "other".to_string(),
            })
        ))?;
        let mut s = self.state.borrow_mut();
        let id = s.next_tab_id;
        s.next_tab_id += 1;
        let tab = TabState {
            id: TabId(id),
            position: s.tabs.len() as u32,
            name: spec
                .name
                .clone()
                .unwrap_or_else(|| format!("Tab #{}", id + 1)),
            active: true,
            selectable_tiled_panes_count: 0,
            selectable_floating_panes_count: 0,
            are_floating_panes_visible: true,
        };
        s.tabs.push(tab);
        // zellij挙動のエミュレート: layout指定時はそのslot数のpaneを作り、
        // 生成KDLに注入されたcommand/cwdを再現する（remap検証が実挙動と同じ条件で走るように）
        let slot_specs = spec
            .layout
            .as_ref()
            .and_then(|l| l.inline.as_deref())
            .and_then(|kdl| zelper::layout::parse(kdl).ok())
            .map(|doc| zelper::layout::extract_slot_commands(&doc));
        let n_panes = slot_specs.as_ref().map_or(1, Vec::len);
        for i in 0..n_panes {
            let pid = s.next_terminal_id;
            s.next_terminal_id += 1;
            let sc = slot_specs.as_ref().and_then(|s| s.get(i));
            let command = sc
                .map(|s| s.command_argv.join(" "))
                .filter(|c| !c.is_empty() && !self.fail_restart.borrow().contains(c));
            // TASK-75（E2E要因A改訂）: layout生成paneのvisual orderがslot宣言順に
            // 対応するよう幾何は行位置 i に置く（focus対象・検証(d)の位置基準）
            s.panes.push(PaneState {
                id: PaneKindId::Terminal(pid),
                title: format!("Pane #{pid}"),
                is_selectable: true,
                is_floating: false,
                is_focused: false,
                exited: false,
                is_held: false,
                geometry: Geometry {
                    x: 0,
                    y: i as u32,
                    rows: 10,
                    cols: 10,
                },
                command,
                terminal_command: None,
                cwd: sc.and_then(|s| s.cwd.clone()),
                tab_id: TabId(id),
                tab_position: 0,
                tab_name: format!("Tab #{}", id + 1),
                plugin_url: None,
            });
            // TASK-81: new-tab --layout-stringの具体化paneのinvoked_with（DD-10.1
            // 事実5: bare slotはadd_cwd_to_layoutでRun::Cwd化〔クライアントcwd〕・
            // command slotはRun::Command。zelper自身の空group生成tab = Run::Cwd系
            // shellでありreconciliation反転の照合先になる。クライアントcwdは
            // 決定性のため固定値）
            let pane_id = PaneKindId::Terminal(pid);
            let run = match sc {
                Some(sc) if !sc.command_argv.is_empty() => FakeRun::from_slot(sc),
                Some(sc) => match &sc.cwd {
                    Some(cwd) => FakeRun::Cwd(PathBuf::from(cwd)),
                    None => FakeRun::Cwd(PathBuf::from("/fake-client-cwd")),
                },
                None => FakeRun::Cwd(PathBuf::from("/fake-client-cwd")),
            };
            s.invoked_with.insert(pane_id, run);
        }
        if let Some(t) = s.tabs.iter_mut().find(|t| t.id == TabId(id)) {
            t.selectable_tiled_panes_count = n_panes as u32;
        }
        Ok(TabId(id))
    }

    fn close_pane(&self, pane: &PaneKindId) -> Result<(), ZelperError> {
        self.record(&format!("close-pane {}", pane.as_spec()))?;
        if self.mute_close_pane.get() {
            // 呼び出し自体は成功扱いだがpaneを除去しない（dup close漏れのemulation）
            return Ok(());
        }
        let mut s = self.state.borrow_mut();
        s.panes.retain(|p| &p.id != pane);
        s.invoked_with.remove(pane);
        Ok(())
    }

    fn close_tab(&self, tab: TabId) -> Result<(), ZelperError> {
        self.record(&format!("close-tab {}", tab.0))?;
        let mut s = self.state.borrow_mut();
        s.tabs.retain(|t| t.id != tab);
        s.panes.retain(|p| p.tab_id != tab);
        Ok(())
    }

    fn resize(&self, pane: Option<&PaneKindId>, op: ResizeOp) -> Result<(), ZelperError> {
        let (verb, dir) = match op {
            ResizeOp::Grow(d) => ("increase", d.as_str()),
            ResizeOp::Shrink(d) => ("decrease", d.as_str()),
        };
        let target = pane
            .map(|p| p.as_spec())
            .unwrap_or_else(|| "focused".into());
        self.record(&format!("resize {target} {verb} {dir}"))?;
        // fakeはgeometryを変化させる（equalize収束テスト用にcols+10/increase right）
        // titleに "frozen" を含むpaneはresizeがno-opになる（no-op打ち切りテスト用）
        let mut s = self.state.borrow_mut();
        if let Some(p) = pane.and_then(|id| s.panes.iter_mut().find(|p| &p.id == id)) {
            if p.title.contains("frozen") {
                return Ok(());
            }
            match op {
                ResizeOp::Grow(ResizeDirection::Right) => p.geometry.cols += 10,
                ResizeOp::Shrink(ResizeDirection::Right) => {
                    p.geometry.cols = p.geometry.cols.saturating_sub(10)
                }
                ResizeOp::Grow(ResizeDirection::Down) => p.geometry.rows += 5,
                ResizeOp::Shrink(ResizeDirection::Down) => {
                    p.geometry.rows = p.geometry.rows.saturating_sub(5)
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// TASK-88（DD-13）: native move-pane系のemulation。M1/M8実測の基準値どおり
    /// 「slot幾何完全不変・pane id/title不変の交換」を再現する（REVIEW-3指摘1:
    /// 実測と同一の交換相手を選ぶ規則）:
    /// - 方向移動: 対象paneの矩形と「その方向に位置し直交軸の範囲が重複する」
    ///   paneのうち最も近いものと幾何をswap（left=左でy重複のうち右端が最大 /
    ///   right=右でy重複のうち左端が最小・同値は上端が最大〔M1実測: A right→C〕 /
    ///   up=上でx重複のうち下端が最大 / down=下でx重複のうち上端が最小）。
    ///   候補なしはno-op（M1 edge実測 EL/EU/ER/ED: rc=0・出力空・状態完全不変）。
    ///   M1実測との対応: C left→A（左の大pane）・A right→C（右側の下段）・
    ///   C up→B・B down→C（同一列の上下）・Left/UpとRight/Downを同一扱いしない
    /// - rotate: 対象paneと視覚順序（y,x昇順）上の「次/前」の1つとのみ交換
    ///   （forwards=次・末尾は先頭とwrap / backwards=前・先頭は末尾とwrap。
    ///   m8 move3基準: 対象0〔順序1番目〕→0↔1交換で2不変。全pane循環でない）
    /// - focus（M1実測: Lでは相手がfocusなら移動paneへ遷移・U/Dではfocus pane
    ///   不参加なら不変・移動pane自身がfocusなら維持）: 交換する2 paneのうち
    ///   focus==trueがあればfocusは移動対象paneへ移る。いなければ不変
    fn move_pane(&self, pane: &PaneKindId, op: MovePaneOp) -> Result<(), ZelperError> {
        let rec = match op {
            MovePaneOp::Direction(d) => format!("move-pane {} -p {}", d.as_str(), pane.as_spec()),
            MovePaneOp::RotateForwards => format!("move-pane -p {}", pane.as_spec()),
            MovePaneOp::RotateBackwards => format!("move-pane-backwards -p {}", pane.as_spec()),
        };
        self.record(&rec)?;
        let mut s = self.state.borrow_mut();
        let Some(target) = s.panes.iter().find(|p| &p.id == pane).cloned() else {
            return Ok(());
        };
        // 同一tabのtiled selectable terminal pane群（visual order）
        let mut group: Vec<(PaneKindId, Geometry, bool)> = s
            .panes
            .iter()
            .filter(|p| {
                p.tab_id == target.tab_id
                    && p.is_selectable
                    && !p.is_floating
                    && matches!(p.id, PaneKindId::Terminal(_))
            })
            .map(|p| (p.id, p.geometry, p.is_focused))
            .collect();
        group.sort_by_key(|(_, g, _)| (g.y, g.x));
        let n = group.len();
        let Some(idx) = group.iter().position(|(id, _, _)| *id == target.id) else {
            return Ok(());
        };
        let partner = match op {
            // 方向移動: 幾何隣接則で交換相手を決める（候補なし=no-op）
            MovePaneOp::Direction(d) => neighbor_in_direction(&group, idx, d),
            // rotate: 視覚順序の前後の1つ（wrap）
            MovePaneOp::RotateForwards => (n > 1).then(|| (idx + 1) % n),
            MovePaneOp::RotateBackwards => (n > 1).then(|| (idx + n - 1) % n),
        };
        if let Some(j) = partner {
            let (id_a, geom_a, _) = group[idx];
            let (id_b, geom_b, _) = group[j];
            let focused = group[idx].2 || group[j].2;
            if let Some(p) = s.panes.iter_mut().find(|p| p.id == id_a) {
                p.geometry = geom_b;
                if focused {
                    p.is_focused = true;
                }
            }
            if let Some(p) = s.panes.iter_mut().find(|p| p.id == id_b) {
                p.geometry = geom_a;
                if focused {
                    p.is_focused = false;
                }
            }
        }
        Ok(())
    }

    /// TASK-88（DD-13）: native move-tabのemulation。M2実測の基準値どおり
    /// 「1回の呼び出しで1 position移動・隣接tabとは1回で純swap・edgeはwrap
    /// （全体rotation）・tab id/name不変」。mute_move_tab注入時は記録のみで
    /// positionを変えない（postcondition検証系列）
    fn move_tab(&self, tab: TabId, direction: TabMoveDirection) -> Result<(), ZelperError> {
        self.record(&format!("move-tab {} -t {}", direction.as_str(), tab.0))?;
        if self.mute_move_tab.get() {
            return Ok(());
        }
        let mut s = self.state.borrow_mut();
        let Some(pos) = s.tabs.iter().position(|t| t.id == tab) else {
            return Ok(());
        };
        let len = s.tabs.len();
        let moved = s.tabs.remove(pos);
        // edgeはwrap（M2実測: 最後尾rightは全体rotation）
        let new_pos = match direction {
            TabMoveDirection::Right => {
                if pos + 1 >= len {
                    0
                } else {
                    pos + 1
                }
            }
            TabMoveDirection::Left => {
                if pos == 0 {
                    len - 1
                } else {
                    pos - 1
                }
            }
        };
        s.tabs.insert(new_pos, moved);
        for (i, t) in s.tabs.iter_mut().enumerate() {
            t.position = i as u32;
        }
        // REVIEW-3指摘3: 指定番目（1開始）のmove_tab成功後に指定tabをpaneごと除去
        // （tab交換の途中消滅系列。呼出回数は記録の"move-tab"接頭で計上）
        if let Some((nth, tab)) = self.vanish_tab_after_move_tab.get() {
            let called = self
                .calls
                .borrow()
                .iter()
                .filter(|c| c.starts_with("move-tab "))
                .count();
            if called == nth {
                self.vanish_tab_after_move_tab.set(None);
                s.tabs.retain(|t| t.id != tab);
                s.panes.retain(|p| p.tab_id != tab);
                for (i, t) in s.tabs.iter_mut().enumerate() {
                    t.position = i as u32;
                }
            }
        }
        Ok(())
    }

    fn override_layout(&self, spec: &OverrideSpec) -> Result<(), ZelperError> {
        self.record(&format!(
            "override-layout active_only={} retain_t={} retain_p={}",
            spec.apply_only_to_active_tab, spec.retain_terminal, spec.retain_plugin
        ))?;
        self.override_count.set(self.override_count.get() + 1);
        // TASK-70: 指定番目のoverride-layout呼出のみ失敗させる注入（状態変更前に
        // Errを返すためpane構成・形状は不変 = 形状のみ未適用の系列）
        if self
            .fail_override_layout_nth
            .get()
            .is_some_and(|nth| nth == self.override_count.get())
        {
            self.fail_override_layout_nth.set(None);
            return Err(ZelperError::new(
                ErrorClass::OperationFailed,
                "injected override-layout failure",
            ));
        }
        let mut s = self.state.borrow_mut();
        // layout適用で掃除されるpaneのemulate（検証失敗test用）
        let mut dropped = self.drop_on_override.borrow_mut();
        if !dropped.is_empty() {
            s.panes.retain(|p| !dropped.contains(&p.id));
            dropped.clear();
        }
        // 生成KDL（--layout-string）のslot列とactive tabのpaneをrun等値照合する
        // （実zellijのfind_already_running_panes模倣。TASK-81: 照合keyは
        //  invoked_with（RunCommand全field比較・None==None bare一致・Run::Cwdは
        //  pane cwd="X" slotと一致）。一致paneはslot位置（visual order s番目 =
        //  slot s）へ配置し、不一致slotは新規paneをspawn（新id採番）する。
        //  未照合の過剰paneは入れ子残留として残る（(b)検証で検出される系列）
        if let Some(kdl) = spec.source.inline.as_deref()
            && let Ok(doc) = zelper::layout::parse(kdl)
            && let Some(tab) = s.tabs.iter().find(|t| t.active).map(|t| t.id)
        {
            let slots = zelper::layout::extract_slot_commands(&doc);
            let mut candidates: Vec<(PaneKindId, (u32, u32), FakeRun)> = s
                .panes
                .iter()
                .filter(|p| p.tab_id == tab && p.is_remap_source())
                .map(|p| (p.id, (p.geometry.y, p.geometry.x), invoked_of(&s, p)))
                .collect();
            candidates.sort_by_key(|(_, pos, _)| *pos);
            let mut used: Vec<PaneKindId> = Vec::new();
            for (i, sc) in slots.iter().enumerate() {
                let want = FakeRun::from_slot(sc);
                match candidates
                    .iter()
                    .find(|(id, _, run)| !used.contains(id) && *run == want)
                {
                    Some((id, _, _)) => {
                        used.push(*id);
                        if let Some(p) = s.panes.iter_mut().find(|p| p.id == *id) {
                            p.geometry.y = i as u32;
                            p.geometry.x = 0;
                        }
                    }
                    None => {
                        let is_command_spawn = matches!(want, FakeRun::Command { .. });
                        let is_bare_spawn = matches!(want, FakeRun::None);
                        let spawned = spawn_slot_pane(&mut s, tab, i, want);
                        // TASK-81 E2E改訂F2: 短命command dupの即時exit+hold化注入
                        // （t0q qp8再現。command系spawnのみ。bare spawn〔正規spawn
                        // 相当〕はheld化しない）
                        if self.hold_spawned_command_dup.get()
                            && is_command_spawn
                            && let Some(p) = s.panes.iter_mut().find(|p| p.id == spawned)
                        {
                            p.exited = true;
                            p.is_held = true;
                        }
                        // TASK-81再E2E R1: bare slot spawn paneのpane_cwd注入
                        // （/proc由来の現在cwd=クライアントcwdのemulation。t0p2
                        // terminal_7ではshell割当paneと同一cwdになり属性照合の辺が
                        // 複数slotに立つ。cwd付きslot spawn〔Run::Cwd〕はslot由来の
                        // cwdを持つため対象外。default〔None〕はcwd無し=従来挙動）
                        if is_bare_spawn
                            && let Some(cwd) = self.bare_spawn_cwd.borrow().clone()
                            && let Some(p) = s.panes.iter_mut().find(|p| p.id == spawned)
                        {
                            p.cwd = Some(cwd);
                        }
                    }
                }
            }
            // 未照合の残留pane（過剰分）の配置。default（注入なし）は末尾群: 実zellij
            // では入れ子残留で幾何が崩れるが、fakeは「visual order s番目 = slot s」の
            // 対応と残留paneの相対順の決定性を保証する（既存条件の期待値の維持）。
            // interrupt_leftover_on_override注入時は残留paneを各slot直後（j番目の
            // 残留を y=j・x=50）に割り込ませる（押し出し再現: 残留paneの介在でdup
            // paneのvisual order順位がslot位置とずれる系列。TASK-81 E2E改訂F1）
            let interrupt = self.interrupt_leftover_on_override.get();
            let mut leftover_y = slots.len() as u32;
            let mut leftover_idx = 0u32;
            for (id, _, _) in &candidates {
                if !used.contains(id)
                    && let Some(p) = s.panes.iter_mut().find(|p| p.id == *id)
                {
                    if interrupt {
                        p.geometry.y = leftover_idx;
                        p.geometry.x = 50;
                    } else {
                        p.geometry.y = leftover_y;
                        p.geometry.x = 0;
                        leftover_y += 1;
                    }
                    leftover_idx += 1;
                }
            }
            if let Some(t) = s.tabs.iter_mut().find(|t| t.id == tab) {
                t.selectable_tiled_panes_count = slots.len() as u32;
            }
        }
        // command照合missによる重複spawnのemulate（R38検証用注入）
        let spawn: Vec<TabId> = self.spawn_on_override.borrow_mut().drain(..).collect();
        for tab in spawn {
            spawn_shell_pane(&mut s, tab, "Pane (duplicate)");
        }
        // 当該tabへのexited/held化command pane追加（TASK-81コードレビューC1検証用
        // 注入: gate発動・close対象なしの無害系列。terminal_command=Someのため
        // bare突合に入らず、argvは割当と不一致にする想定）
        let held_extra: Vec<(TabId, String)> = self
            .spawn_held_extra_on_override
            .borrow_mut()
            .drain(..)
            .collect();
        for (tab, command) in held_extra {
            let pid = s.next_terminal_id;
            s.next_terminal_id += 1;
            let (tab_name, tab_pos) = s
                .tabs
                .iter()
                .find(|t| t.id == tab)
                .map(|t| (t.name.clone(), t.position))
                .unwrap_or_default();
            let y = s
                .panes
                .iter()
                .filter(|p| p.tab_id == tab && p.is_remap_source())
                .map(|p| p.geometry.y)
                .max()
                .unwrap_or(0)
                + 1;
            s.panes.push(PaneState {
                id: PaneKindId::Terminal(pid),
                title: format!("Pane #{pid} (held)"),
                is_selectable: true,
                is_floating: false,
                is_focused: false,
                exited: true,
                is_held: true,
                geometry: Geometry {
                    x: 0,
                    y,
                    rows: 10,
                    cols: 10,
                },
                command: None,
                terminal_command: Some(command),
                cwd: None,
                tab_id: tab,
                tab_position: tab_pos,
                tab_name,
                plugin_url: None,
            });
        }
        Ok(())
    }

    fn go_to_tab(&self, tab: TabId) -> Result<(), ZelperError> {
        self.record(&format!("go-to-tab {}", tab.0))?;
        if self
            .fail_go_to_after
            .get()
            .is_some_and(|n| self.override_count.get() > n)
        {
            self.fail_go_to_after.set(None);
            return Err(ZelperError::new(
                ErrorClass::OperationFailed,
                "injected focus-restore failure",
            ));
        }
        let mut s = self.state.borrow_mut();
        for t in s.tabs.iter_mut() {
            t.active = t.id == tab;
        }
        Ok(())
    }

    /// TASK-75（v2.2・TR75-10）: 薄い実装+呼出記録。対象paneのtab内でfocusを設定
    /// する（R75-3: 非active tabもis_focusedを持つ。focus-pane-idが他tabのfocusを
    /// 解除しない前提〔U75-3〕と同形に、tab内の他paneのみfocus解除する）。
    /// already_focused_focus注入時はfocus状態を設定したうえでzellij実機と同じ
    /// 「already focused」error（exit 2相当）を返す
    fn focus_pane(&self, pane: &PaneKindId) -> Result<(), ZelperError> {
        self.record(&format!("focus-pane-id {}", pane.as_spec()))?;
        let mut s = self.state.borrow_mut();
        let Some(tab) = s.panes.iter().find(|p| &p.id == pane).map(|p| p.tab_id) else {
            return Err(ZelperError::new(
                ErrorClass::OperationFailed,
                "pane not found",
            ));
        };
        for p in s.panes.iter_mut() {
            if p.tab_id == tab {
                p.is_focused = p.id == *pane;
            }
        }
        if self.already_focused_focus.get() {
            return Err(ZelperError::new(
                ErrorClass::OperationFailed,
                format!("Pane {:?} is already focused", pane),
            ));
        }
        Ok(())
    }

    fn dump_layout(&self) -> Result<String, ZelperError> {
        self.record("dump-layout")?;
        Ok("layout {\n}\n".to_string())
    }

    fn toggle_embed_floating(&self, pane: &PaneKindId) -> Result<(), ZelperError> {
        self.record(&format!("toggle-embed {}", pane.as_spec()))?;
        let mut s = self.state.borrow_mut();
        if let Some(p) = s.panes.iter_mut().find(|p| &p.id == pane) {
            p.is_floating = !p.is_floating;
        }
        Ok(())
    }

    fn pipe_plugin(
        &self,
        _path: &std::path::Path,
        name: &str,
        payload: &str,
    ) -> Result<(), ZelperError> {
        self.record(&format!("pipe {name} {payload}"))?;
        if self.pipe_mute.borrow().iter().any(|m| m == name) {
            // pipe自体は成功するが効果が現れない（polling timeout模擬）
            return Ok(());
        }
        let mut s = self.state.borrow_mut();
        match name {
            // probe <nonce>: pluginは自動launchされhost tab（active tab）に現れ、
            // 自身のnon-selectable paneのtitleを書き換える（DD-10.3 protocol）
            "probe" => {
                let host = s
                    .tabs
                    .iter()
                    .find(|t| t.active)
                    .map(|t| t.id)
                    .unwrap_or(TabId(0));
                let title = format!("zelper-probe-{payload}");
                if let Some(p) = s
                    .panes
                    .iter_mut()
                    .find(|p| matches!(p.id, PaneKindId::Plugin(_)))
                {
                    p.title = title;
                } else {
                    let pid = s.next_plugin_id;
                    s.next_plugin_id += 1;
                    s.panes.push(PaneState {
                        id: PaneKindId::Plugin(pid),
                        title,
                        is_selectable: false,
                        is_floating: true,
                        is_focused: false,
                        exited: false,
                        is_held: false,
                        geometry: Geometry {
                            x: 0,
                            y: 0,
                            rows: 10,
                            cols: 10,
                        },
                        command: None,
                        terminal_command: None,
                        cwd: None,
                        tab_id: host,
                        tab_position: 0,
                        tab_name: String::new(),
                        plugin_url: Some("file:zelper-companion.wasm".into()),
                    });
                }
            }
            // break-id <tab_id> <pane_id_csv>: 既存tabへの移動（プロセス保存）
            "break-id" => {
                let mut parts = payload.split_whitespace();
                let tab_id = parts.next().unwrap_or_default().parse::<u32>().unwrap_or(0);
                let ids = parse_pane_ids(&parts.collect::<Vec<_>>().join(" "));
                // TASK-88: 指定番目（1開始）のbreak-id pipeのみ成功扱いだが効果を
                // 反映しない（swap 2step目失敗のpolling timeout系列注入）
                let seen_break_ids = self
                    .calls
                    .borrow()
                    .iter()
                    .filter(|c| c.starts_with("pipe break-id"))
                    .count();
                if let Some(nth) = self.mute_break_id_nth.get()
                    && seen_break_ids == nth
                {
                    self.mute_break_id_nth.set(None);
                    return Ok(());
                }
                FakeBackend::move_panes(&mut s, &ids, TabId(tab_id));
                *self.last_break_moved.borrow_mut() = ids.first().copied();
            }
            // break-new <pane_id_csv>: 新規tab作成 + 移動
            "break-new" => {
                let ids = parse_pane_ids(payload);
                let id = s.next_tab_id;
                s.next_tab_id += 1;
                let position = s.tabs.len() as u32;
                s.tabs.push(TabState {
                    id: TabId(id),
                    position,
                    name: format!("Tab #{}", id + 1),
                    active: false,
                    selectable_tiled_panes_count: 0,
                    selectable_floating_panes_count: 0,
                    are_floating_panes_visible: true,
                });
                FakeBackend::move_panes(&mut s, &ids, TabId(id));
            }
            // 未知のnameは何もしない（旧binary混在時の安全側挙動。DD-10.3）
            _ => {}
        }
        Ok(())
    }

    /// TASK-89（DD-14）: attach/detach/create_session_detachedは呼出記録のみの
    /// 薄い実装。attachは対話clientの実挙動（stdio inherit等）ではなくapp層の
    /// 報告契約の検証用（run_interactive経路の検証はsrc/zellij/process.rs内test）。
    /// arm_stdout_probe時は呼出時点のstdout内容（/proc/self/fd/1の読み直し）を
    /// 記録する（backend復帰前の無出力検証。REVIEW-2指摘6）
    fn attach_session(&self) -> Result<(), ZelperError> {
        self.record("attach")?;
        if self.probe_stdout_on_attach.get() {
            // fd 1がtest側で一時fileへredirectされている前提（pipeだとblockする
            // ためarmed時のみ）。読み取りは新しいfile descriptionで先頭から
            *self.stdout_at_attach.borrow_mut() = std::fs::read_to_string("/proc/self/fd/1").ok();
        }
        Ok(())
    }

    fn detach_session(&self) -> Result<(), ZelperError> {
        self.record("detach")?;
        Ok(())
    }

    fn create_session_detached(&self, name: &str) -> Result<(), ZelperError> {
        self.record(&format!("attach -b {name}"))?;
        if self.mute_create_session.get() {
            // 呼び出し自体は成功扱いだがlist-sessions状態へ反映しない
            // （rc=0・postcondition不在模擬）
            return Ok(());
        }
        let mut s = self.state.borrow_mut();
        s.sessions.push(SessionRef {
            name: name.into(),
            created: Some("1m ago".into()),
            current: false,
            exited: false,
        });
        Ok(())
    }

    /// TASK-84（設計§2.7）: delete_session。呼出記録（`delete-session <name> -f`）
    /// と、sessions状態からの対象行除去（実zellijの完全削除〔行+resurrection
    /// data〕のうちlist_sessions可視分の再現）を行う。mute_delete_session注入時は
    /// 記録のみで行を除去しない（rc=0・postcondition不一致模擬）
    fn delete_session(&self, name: &str) -> Result<(), ZelperError> {
        self.record(&format!("delete-session {name} -f"))?;
        if self.mute_delete_session.get() {
            return Ok(());
        }
        let mut s = self.state.borrow_mut();
        s.sessions.retain(|x| x.name != name);
        Ok(())
    }

    /// TASK-95（設計§2.9）: rename_session。呼出記録
    /// （`rename-session@{from} {to}`——list_tabs_forの`list-tabs@{session}`流儀。
    /// fail_opのprefix照合〔record内starts_with〕により`rename-session@`等で
    /// 失敗注入可）と、sessions内のname == from行をname = toへ書き換え
    /// （exitedは不変——renameは実行状態を変えない）を行う。
    /// mute_rename_session注入時は記録のみで書き換えない（rc=0・黙失敗模擬）。
    /// rename_post_inject注入時（レビュー対応）は状態作用を差し替える
    /// （postcondition各判定の独立保護検証用）
    fn rename_session(&self, from: &str, to: &str) -> Result<(), ZelperError> {
        self.record(&format!("rename-session@{from} {to}"))?;
        if self.mute_rename_session.get() {
            return Ok(());
        }
        let mut s = self.state.borrow_mut();
        match self.rename_post_inject.get() {
            // 旧名残存 + 新名running行追加（旧名消失検査の独立保護）
            Some(RenamePostInjection::KeepOldAddNew) => s.sessions.push(SessionRef {
                name: to.to_string(),
                created: Some("1m ago".into()),
                current: false,
                exited: false,
            }),
            // 旧名・新名とも行消失（新名running出現検査の独立保護）
            Some(RenamePostInjection::DropBoth) => {
                s.sessions.retain(|x| x.name != from);
            }
            // 旧名消失 + 新名がEXITED行で出現（exited == false検査の独立保護）
            Some(RenamePostInjection::NewAsExited) => {
                if let Some(x) = s.sessions.iter_mut().find(|x| x.name == from) {
                    x.name = to.to_string();
                    x.exited = true;
                }
            }
            None => {
                if let Some(x) = s.sessions.iter_mut().find(|x| x.name == from) {
                    x.name = to.to_string();
                }
            }
        }
        Ok(())
    }
}
