use super::parser;
use super::{MIN_SUPPORTED, NewPaneSpec, NewTabSpec, OverrideSpec, ResizeOp, ZellijBackend};
use crate::domain::{PaneKindId, SessionRef, TabId, TabState};
use crate::error::{ErrorClass, ZelperError};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// 実zellij実行backend（DD-3.1）: 常に `zellij --session NAME action ...` 形式。
/// argv配列で実行し（shellを経由しない）、1呼び出しのtimeoutを持つ。
pub struct ZellijCliBackend {
    session: String,
    program: PathBuf,
    timeout: Duration,
}

const DEFAULT_TIMEOUT_SECS: u64 = 10;

impl ZellijCliBackend {
    pub fn new(session: impl Into<String>) -> Self {
        ZellijCliBackend {
            session: session.into(),
            program: PathBuf::from("zellij"),
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        }
    }

    pub fn with_program(mut self, program: PathBuf) -> Self {
        self.program = program;
        self
    }
    // 注: with_programはfake backend差し替え用の将来拡張ポイントとして残置

    /// 生のaction呼び出し
    fn run_action(&self, args: &[&str]) -> Result<String, ZelperError> {
        let full: Vec<String> = std::iter::once("--session".to_string())
            .chain(std::iter::once(self.session.clone()))
            .chain(std::iter::once("action".to_string()))
            .chain(args.iter().map(|s| s.to_string()))
            .collect();
        self.run(&full)
    }

    /// 生のzellij呼び出し（list-sessions等のaction外コマンド用）。
    /// stdout/stderrは専用threadで読み取り、pipe buffer満杯によるdeadlockを防ぐ。
    fn run(&self, args: &[String]) -> Result<String, ZelperError> {
        let mut cmd = Command::new(&self.program);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ZelperError::new(
                    ErrorClass::ZellijUnavailable,
                    "zellij executable not found in PATH",
                )
            } else {
                ZelperError::new(
                    ErrorClass::ZellijUnavailable,
                    format!("failed to start zellij: {e}"),
                )
            }
        })?;

        let mut stdout_pipe = child.stdout.take().expect("stdout piped");
        let mut stderr_pipe = child.stderr.take().expect("stderr piped");

        let t_out = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout_pipe.read_to_end(&mut buf);
            buf
        });
        let t_err = std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stderr_pipe.read_to_end(&mut buf);
            buf
        });

        let deadline = Instant::now() + self.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(ZelperError::new(
                            ErrorClass::OperationFailed,
                            format!("zellij call timed out after {:?}", self.timeout),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    return Err(ZelperError::new(
                        ErrorClass::OperationFailed,
                        format!("failed to wait for zellij: {e}"),
                    ));
                }
            }
        };

        let out = t_out.join().unwrap_or_default();
        let err = t_err.join().unwrap_or_default();
        let stdout = String::from_utf8_lossy(&out).into_owned();
        let stderr = String::from_utf8_lossy(&err).into_owned();
        if status.success() {
            Ok(stdout)
        } else {
            Err(ZelperError::new(
                ErrorClass::OperationFailed,
                format!("zellij exited with {}: {}", status, stderr.trim()),
            ))
        }
    }
}

impl ZellijBackend for ZellijCliBackend {
    fn version(&self) -> Result<String, ZelperError> {
        self.run(&["--version".to_string()])
    }

    fn list_sessions(&self) -> Result<Vec<SessionRef>, ZelperError> {
        let out = self.run(&["list-sessions".to_string(), "-n".to_string()])?;
        Ok(parser::parse_sessions(&out))
    }

    fn list_tabs(&self) -> Result<Vec<TabState>, ZelperError> {
        let out = self.run_action(&["list-tabs", "-a", "--json"])?;
        parser::parse_tabs(&out)
    }

    fn list_panes(&self) -> Result<Vec<crate::domain::PaneState>, ZelperError> {
        let out = self.run_action(&["list-panes", "-a", "--json"])?;
        parser::parse_panes(&out)
    }

    fn current_tab(&self) -> Result<TabState, ZelperError> {
        let out = self.run_action(&["current-tab-info", "--json"])?;
        // TabInfo単体JSON → 配列化してparse
        let trimmed = out.trim();
        let wrapped = if trimmed.starts_with('{') {
            format!("[{trimmed}]")
        } else {
            trimmed.to_string()
        };
        let mut tabs = parser::parse_tabs(&wrapped)?;
        if tabs.len() == 1 {
            Ok(tabs.remove(0))
        } else {
            Err(ZelperError::new(
                ErrorClass::OperationFailed,
                "unexpected current-tab-info output",
            ))
        }
    }

    fn dump_screen(&self, pane: &PaneKindId, full: bool) -> Result<String, ZelperError> {
        let p = pane.as_spec();
        let mut args = vec!["dump-screen", "-p", &p];
        if full {
            args.push("-f");
        }
        self.run_action(&args)
    }

    fn write_chars(&self, pane: &PaneKindId, text: &str) -> Result<(), ZelperError> {
        let p = pane.as_spec();
        self.run_action(&["write-chars", "-p", &p, text])?;
        Ok(())
    }

    fn write_bytes(&self, pane: &PaneKindId, bytes: &[u8]) -> Result<(), ZelperError> {
        let p = pane.as_spec();
        let mut args = vec!["write".to_string(), "-p".to_string(), p];
        args.extend(bytes.iter().map(|b| b.to_string()));
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run_action(&refs)?;
        Ok(())
    }

    fn send_keys(&self, pane: &PaneKindId, keys: &[String]) -> Result<(), ZelperError> {
        let p = pane.as_spec();
        let mut args = vec!["send-keys".to_string(), "-p".to_string(), p];
        args.extend(keys.iter().cloned());
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run_action(&refs)?;
        Ok(())
    }

    fn rename_pane(&self, pane: &PaneKindId, name: &str) -> Result<(), ZelperError> {
        let p = pane.as_spec();
        self.run_action(&["rename-pane", "-p", &p, name])?;
        Ok(())
    }

    fn rename_tab(&self, tab: TabId, name: &str) -> Result<(), ZelperError> {
        self.run_action(&["rename-tab-by-id", &tab.0.to_string(), name])?;
        Ok(())
    }

    fn new_pane(&self, spec: &NewPaneSpec) -> Result<PaneKindId, ZelperError> {
        let mut args = vec!["new-pane".to_string()];
        if let Some(t) = spec.tab {
            args.push("--tab-id".into());
            args.push(t.0.to_string());
        }
        if let Some(n) = &spec.name {
            args.push("--name".into());
            args.push(n.clone());
        }
        if let Some(cwd) = &spec.cwd {
            args.push("--cwd".into());
            args.push(cwd.to_string_lossy().into_owned());
        }
        if !spec.command.is_empty() {
            args.push("--".into());
            args.extend(spec.command.iter().cloned());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let out = self.run_action(&refs)?;
        parser::parse_created_pane(&out)
    }

    fn new_tab(&self, spec: &NewTabSpec) -> Result<TabId, ZelperError> {
        let mut args = vec!["new-tab".to_string()];
        if let Some(n) = &spec.name {
            args.push("--name".into());
            args.push(n.clone());
        }
        if let Some(cwd) = &spec.cwd {
            args.push("--cwd".into());
            args.push(cwd.to_string_lossy().into_owned());
        }
        if let Some(layout) = &spec.layout {
            layout.validate_exclusive()?;
            if let Some(name) = &layout.name {
                args.push("-l".into());
                args.push(name.clone());
            } else if let Some(path) = &layout.path {
                args.push("-l".into());
                args.push(path.to_string_lossy().into_owned());
            } else if let Some(inline) = &layout.inline {
                args.push("--layout-string".into());
                args.push(inline.clone());
            }
        }
        if !spec.command.is_empty() {
            args.push("--".into());
            args.extend(spec.command.iter().cloned());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let out = self.run_action(&refs)?;
        parser::parse_created_tab(&out)
    }

    fn close_pane(&self, pane: &PaneKindId) -> Result<(), ZelperError> {
        let p = pane.as_spec();
        self.run_action(&["close-pane", "-p", &p])?;
        Ok(())
    }

    fn close_tab(&self, tab: TabId) -> Result<(), ZelperError> {
        self.run_action(&["close-tab-by-id", &tab.0.to_string()])?;
        Ok(())
    }

    fn resize(&self, pane: Option<&PaneKindId>, op: ResizeOp) -> Result<(), ZelperError> {
        let (verb, dir) = match op {
            ResizeOp::Grow(d) => ("increase", d),
            ResizeOp::Shrink(d) => ("decrease", d),
        };
        let mut args = vec!["resize".to_string()];
        if let Some(p) = pane {
            args.push("-p".to_string());
            args.push(p.as_spec());
        }
        args.push(verb.to_string());
        args.push(dir.as_str().to_string());
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run_action(&refs)?;
        Ok(())
    }

    fn override_layout(&self, spec: &OverrideSpec) -> Result<(), ZelperError> {
        spec.source.validate_exclusive()?;
        let mut args = vec!["override-layout".to_string()];
        // nameはbare name（拡張子なし）でlayout_dir解決。pathはpositional。inlineは--layout-string。
        if let Some(name) = &spec.source.name {
            args.push(name.clone());
        } else if let Some(path) = &spec.source.path {
            args.push(path.to_string_lossy().into_owned());
        } else if let Some(inline) = &spec.source.inline {
            args.push("--layout-string".into());
            args.push(inline.clone());
        }
        if spec.apply_only_to_active_tab {
            args.push("--apply-only-to-active-tab".into());
        }
        if spec.retain_terminal {
            args.push("--retain-existing-terminal-panes".into());
        }
        if spec.retain_plugin {
            args.push("--retain-existing-plugin-panes".into());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        self.run_action(&refs)?;
        Ok(())
    }

    fn go_to_tab(&self, tab: TabId) -> Result<(), ZelperError> {
        self.run_action(&["go-to-tab-by-id", &tab.0.to_string()])?;
        Ok(())
    }

    fn dump_layout(&self) -> Result<String, ZelperError> {
        self.run_action(&["dump-layout"])
    }

    fn toggle_embed_floating(&self, pane: &PaneKindId) -> Result<(), ZelperError> {
        let p = pane.as_spec();
        self.run_action(&["toggle-pane-embed-or-floating", "-p", &p])?;
        Ok(())
    }
}

/// 起動時のversion/可用性チェック（DD-3.5）
pub fn check_capability(backend: &dyn ZellijBackend) -> Result<(), ZelperError> {
    let out = backend.version()?;
    match parser::parse_version(&out) {
        Some(v) if v.0 > MIN_SUPPORTED.0 => Err(ZelperError::new(
            ErrorClass::UnsupportedVersion,
            format!(
                "zellij {}.{}.{} is from a newer major version; this zelper supports the {}.x series",
                v.0, v.1, v.2, MIN_SUPPORTED.0
            ),
        )),
        Some(v) if v >= MIN_SUPPORTED => Ok(()),
        Some(v) => Err(ZelperError::new(
            ErrorClass::UnsupportedVersion,
            format!(
                "zelper requires zellij >= {}.{}.{}, found {}.{}.{}",
                MIN_SUPPORTED.0, MIN_SUPPORTED.1, MIN_SUPPORTED.2, v.0, v.1, v.2
            ),
        )),
        None => Err(ZelperError::new(
            ErrorClass::UnsupportedVersion,
            format!("failed to parse zellij version output: {out:?}"),
        )),
    }
}
