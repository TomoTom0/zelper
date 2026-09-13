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
    /// 最初のpipe呼出より後の最初のlist_panesをこの時間だけ遅延させる
    /// （polling deadline超過後の条件成立模擬。C3）
    pub slow_list_panes_after_pipe: Cell<Option<std::time::Duration>>,
    /// rename_tabを記録・成功扱いにするが名前を変えない（rename不生效模擬。C5）
    pub mute_rename_tab: Cell<bool>,
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
            y: 0,
            rows: 10,
            cols: 10,
        },
        command: None,
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
        Ok(self.state.borrow().tabs.clone())
    }

    fn list_tabs_for(&self, session: &str) -> Result<Vec<TabState>, ZelperError> {
        self.record(&format!("list-tabs@{session}"))?;
        Ok(self.state.borrow().tabs.clone())
    }

    fn list_panes(&self) -> Result<Vec<PaneState>, ZelperError> {
        self.record("list-panes")?;
        // 最初のpipe呼出より後の最初のlist_panesを遅延させる（C3注入:
        // backend呼出がdeadlineを消費してから条件成立を返す系列の模擬）
        if let Some(delay) = self.slow_list_panes_after_pipe.get()
            && self.calls.borrow().iter().any(|c| c.starts_with("pipe "))
        {
            self.slow_list_panes_after_pipe.set(None);
            std::thread::sleep(delay);
        }
        Ok(self.state.borrow().panes.clone())
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
                    y: 0,
                    rows: 10,
                    cols: 10,
                },
                command,
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
