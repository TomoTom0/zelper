// L2: 決定的fake backend（test-plan §1）。全操作を記録し、状態を持つ最小実装。
// remap v2（DD-10）対応: pipe_plugin記録・probe/break pipe効果模擬・失敗注入
// （pipe効果不出現・重複spawn・focus復帰失敗）を含む（test-plan §2.7 (j)）。
use std::cell::{Cell, RefCell};
use zelper::domain::*;
use zelper::error::{ErrorClass, ZelperError};
use zelper::zellij::*;

pub struct FakeBackend {
    pub state: RefCell<FakeState>,
    /// 呼び出し記録（"rename-pane terminal_1" 形式）
    pub calls: RefCell<Vec<String>>,
    /// この操作名の次回呼び出しを失敗させる
    pub fail_op: RefCell<Vec<String>>,
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
}

pub struct FakeState {
    pub panes: Vec<PaneState>,
    pub tabs: Vec<TabState>,
    pub next_terminal_id: u32,
    pub next_tab_id: u32,
    pub next_plugin_id: u32,
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
            }),
            calls: RefCell::new(Vec::new()),
            fail_op: RefCell::new(Vec::new()),
            drop_on_override: RefCell::new(Vec::new()),
            fail_restart: RefCell::new(Vec::new()),
            pipe_mute: RefCell::new(Vec::new()),
            spawn_on_override: RefCell::new(Vec::new()),
            override_count: Cell::new(0),
            fail_go_to_after: Cell::new(None),
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
        }
    }

    pub fn record(&self, op: &str) -> Result<(), ZelperError> {
        self.calls.borrow_mut().push(op.to_string());
        let mut fail = self.fail_op.borrow_mut();
        if let Some(pos) = fail.iter().position(|f| op.starts_with(f.as_str())) {
            fail.remove(pos);
            return Err(ZelperError::new(
                ErrorClass::OperationFailed,
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

impl ZellijBackend for FakeBackend {
    fn version(&self) -> Result<String, ZelperError> {
        self.record("version")?;
        Ok("zellij 0.44.3\n".to_string())
    }

    fn list_sessions(&self) -> Result<Vec<SessionRef>, ZelperError> {
        self.record("list-sessions")?;
        Ok(vec![SessionRef {
            name: "fake".into(),
            created: Some("1m ago".into()),
            current: true,
            exited: false,
        }])
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

    fn dump_screen(&self, pane: &PaneKindId, _full: bool) -> Result<String, ZelperError> {
        self.record(&format!("dump-screen {}", pane.as_spec()))?;
        Ok(format!("SCREEN[{}]\n", pane.as_spec()))
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
        }
        if let Some(t) = s.tabs.iter_mut().find(|t| t.id == TabId(id)) {
            t.selectable_tiled_panes_count = n_panes as u32;
        }
        Ok(TabId(id))
    }

    fn close_pane(&self, pane: &PaneKindId) -> Result<(), ZelperError> {
        self.record(&format!("close-pane {}", pane.as_spec()))?;
        let mut s = self.state.borrow_mut();
        s.panes.retain(|p| &p.id != pane);
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

    fn override_layout(&self, spec: &OverrideSpec) -> Result<(), ZelperError> {
        self.record(&format!(
            "override-layout active_only={} retain_t={} retain_p={}",
            spec.apply_only_to_active_tab, spec.retain_terminal, spec.retain_plugin
        ))?;
        self.override_count.set(self.override_count.get() + 1);
        let mut s = self.state.borrow_mut();
        // layout適用で掃除されるpaneのemulate（検証失敗test用）
        let mut dropped = self.drop_on_override.borrow_mut();
        if !dropped.is_empty() {
            s.panes.retain(|p| !dropped.contains(&p.id));
            dropped.clear();
        }
        // 生成KDL（--layout-string）のslot数分だけactive tabのpaneを揃える
        // （実zellijのfind_already_running_panes模擬: run照合したpaneはslotを取り、
        //  空slotは新規既定shell paneで埋まる。E6 run4/run5の知見）
        if let Some(kdl) = spec.source.inline.as_deref()
            && let Ok(doc) = zelper::layout::parse(kdl)
            && let Some(tab) = s.tabs.iter().find(|t| t.active).map(|t| t.id)
        {
            let slots = zelper::layout::extract_slot_commands(&doc).len();
            let have = s
                .panes
                .iter()
                .filter(|p| p.tab_id == tab && p.is_remap_source())
                .count();
            for _ in have..slots {
                spawn_shell_pane(&mut s, tab, "Pane (shell)");
            }
            if let Some(t) = s.tabs.iter_mut().find(|t| t.id == tab) {
                t.selectable_tiled_panes_count = slots as u32;
            }
        }
        // command照合missによる重複spawnのemulate（R38検証用注入）
        let spawn: Vec<TabId> = self.spawn_on_override.borrow_mut().drain(..).collect();
        for tab in spawn {
            spawn_shell_pane(&mut s, tab, "Pane (duplicate)");
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
                FakeBackend::move_panes(&mut s, &ids, TabId(tab_id));
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
}
