// L2: 決定的fake backend（test-plan §1）。全操作を記録し、状態を持つ最小実装。
use std::cell::RefCell;
use zelper::domain::*;
use zelper::error::{ErrorClass, ZelperError};
use zelper::zellij::*;

pub struct FakeBackend {
    pub state: RefCell<FakeState>,
    /// 呼び出し記録（"rename-pane terminal_1" 形式）
    pub calls: RefCell<Vec<String>>,
    /// この操作名の次回呼び出しを失敗させる
    pub fail_op: RefCell<Vec<String>>,
}

pub struct FakeState {
    pub panes: Vec<PaneState>,
    pub tabs: Vec<TabState>,
    pub next_terminal_id: u32,
    pub next_tab_id: u32,
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
        let max_tab = tabs.iter().map(|t| t.id.0).max().unwrap_or(0);
        FakeBackend {
            state: RefCell::new(FakeState {
                panes,
                tabs,
                next_terminal_id: max_term + 1,
                next_tab_id: max_tab + 1,
            }),
            calls: RefCell::new(Vec::new()),
            fail_op: RefCell::new(Vec::new()),
        }
    }

    pub fn record(&self, op: &str) -> Result<(), ZelperError> {
        self.calls.borrow_mut().push(op.to_string());
        let mut fail = self.fail_op.borrow_mut();
        if let Some(pos) = fail.iter().position(|f| op.starts_with(f.as_str())) {
            fail.remove(pos);
            return Err(ZelperError::new(
                ErrorClass::OperationFailed,
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
        }])
    }

    fn list_tabs(&self) -> Result<Vec<TabState>, ZelperError> {
        self.record("list-tabs")?;
        Ok(self.state.borrow().tabs.clone())
    }

    fn list_panes(&self) -> Result<Vec<PaneState>, ZelperError> {
        self.record("list-panes")?;
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
        let mut s = self.state.borrow_mut();
        if let Some(t) = s.tabs.iter_mut().find(|t| t.id == tab) {
            t.name = name.to_string();
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
        // zellij挙動のエミュレート: layout指定時はそのslot数のpaneを作る
        // （bare pane = 既定shell相当。command注入までは再現しない）
        let layout_slots = spec
            .layout
            .as_ref()
            .and_then(|l| l.inline.as_deref())
            .and_then(|kdl| {
                zelper::layout::parse(kdl)
                    .ok()
                    .map(|doc| zelper::layout::count_terminal_slots(&doc))
            });
        let n_panes = layout_slots.unwrap_or(1);
        for _ in 0..n_panes {
            let pid = s.next_terminal_id;
            s.next_terminal_id += 1;
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
                command: None,
                cwd: None,
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
        Ok(())
    }

    fn go_to_tab(&self, tab: TabId) -> Result<(), ZelperError> {
        self.record(&format!("go-to-tab {}", tab.0))?;
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
}
