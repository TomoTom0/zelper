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
    // 注: with_programはfake zellij program差し替えの拡張点。src内#[cfg(test)]（backend-process条件書）が使用

    /// 生のaction呼び出し
    fn run_action(&self, args: &[&str]) -> Result<String, ZelperError> {
        self.run_action_as(&self.session, args)
    }

    /// session名を明示するaction呼び出し（list_tabs_for用）
    fn run_action_as(&self, session: &str, args: &[&str]) -> Result<String, ZelperError> {
        let full: Vec<String> = std::iter::once("--session".to_string())
            .chain(std::iter::once(session.to_string()))
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

    fn list_tabs_lenient(&self) -> Result<Option<Vec<TabState>>, ZelperError> {
        let out = self.run_action(&["list-tabs", "-a", "--json"])?;
        parser::parse_tabs_opt(&out)
    }

    fn list_tabs_for(&self, session: &str) -> Result<Vec<TabState>, ZelperError> {
        let out = self.run_action_as(session, &["list-tabs", "-a", "--json"])?;
        parser::parse_tabs(&out)
    }

    fn list_panes(&self) -> Result<Vec<crate::domain::PaneState>, ZelperError> {
        let out = self.run_action(&["list-panes", "-a", "--json"])?;
        parser::parse_panes(&out)
    }

    fn list_panes_lenient(&self) -> Result<Option<Vec<crate::domain::PaneState>>, ZelperError> {
        let out = self.run_action(&["list-panes", "-a", "--json"])?;
        parser::parse_panes_opt(&out)
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

    fn focus_pane(&self, pane: &PaneKindId) -> Result<(), ZelperError> {
        // argv組み立てとResultを返す薄い実装のみ（TR75-10。warning化・retryなし。
        // PaneKindIdの系列分離〔terminal_N / plugin_N〕はas_spec()が担う）
        let p = pane.as_spec();
        self.run_action(&["focus-pane-id", &p])?;
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

    fn pipe_plugin(
        &self,
        path: &std::path::Path,
        name: &str,
        payload: &str,
    ) -> Result<(), ZelperError> {
        // action pipe --plugin file:<path> --name <name> -- <payload>（DD-10.3）。
        // 権限dialog pending等でblockし得るためrun_actionのtimeoutが必須。
        // 効果の成否は戻り値に現れない（成功時に即座exit 0）ため、callerが
        // polling + postcondition検証で判定する
        let plugin = format!("file:{}", path.display());
        self.run_action(&["pipe", "--plugin", &plugin, "--name", name, "--", payload])?;
        Ok(())
    }
}

/// 起動時のversion/可用性チェック（DD-3.5 compatibility policy）。
/// 実行時要件はzellij >=0.44.3。実証済み組合せはzellij 0.44.3 + zellij-tile 0.44.3
/// のみのため、0.44.x系列を超える未来version（0.45.0等）も受け付けない
pub fn check_capability(backend: &dyn ZellijBackend) -> Result<(), ZelperError> {
    let out = backend.version()?;
    match parser::parse_version(&out) {
        Some(v) if v.0 > MIN_SUPPORTED.0 || (v.0 == MIN_SUPPORTED.0 && v.1 > MIN_SUPPORTED.1) => {
            Err(ZelperError::new(
                ErrorClass::UnsupportedVersion,
                format!(
                    "zellij {}.{}.{} is unverified; only zellij 0.44.3 is a proven \
combination (zellij 0.44.3 + zellij-tile 0.44.3)",
                    v.0, v.1, v.2
                ),
            ))
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    static FAKE_ZELLIJ_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// fake zellij実行可能fileをtemp dirに生成し、program pathを返す
    /// （L3 shimと同様にcleanupなし。CR59-5対応: staging fileをwrite/chmod後にatomic renameで公開）
    fn setup_fake_zellij(tag: &str, script: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("zelper-fake-process-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let shim = dir.join("zellij");
        // 実行内容とmode確定後にのみ公開する。最終の実行pathはwrite open状態にならない
        // （LinuxのETXTBSY回避）。
        let staged = dir.join("zellij.staged");
        std::fs::write(&staged, script).unwrap();
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::rename(staged, &shim).unwrap();
        shim
    }

    /// test-plan §2.3・DD-5:265・DD-10.14:512: zellijの非zero exitは常に
    /// OperationFailedへ変換され、messageへexit statusとtrim済みstderr本文が埋め込まれる
    /// （stderr内容によるclass切替は設計上存在しない）
    // [covers:backend-process.nonzero-exit-operation-failed]
    #[test]
    fn nonzero_exit_becomes_operation_failed() {
        let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
        let program = setup_fake_zellij(
            "nonzero-exit",
            "#!/usr/bin/env bash\necho \"Failed to load layout: can't find layout default\" >&2\nexit 1\n",
        );
        let backend = ZellijCliBackend::new("s").with_program(program);
        let err = backend.run(&["--version".to_string()]).unwrap_err();
        assert_eq!(
            *err.class(),
            ErrorClass::OperationFailed,
            "message: {}",
            err.message()
        );
        let msg = err.message();
        assert!(msg.contains("exit status: 1"), "message: {msg}");
        assert!(msg.contains("Failed to load layout"), "message: {msg}");
        assert!(!msg.contains('\n'), "message: {msg}");
    }

    /// test-plan §2.9・DD-3.1: program不在（spawn ErrのErrorKind=NotFound）の
    /// 起動失敗はZellijUnavailableへ分類される
    // [covers:backend-process.spawn-notfound-zellij-unavailable]
    #[test]
    fn spawn_notfound_becomes_zellij_unavailable() {
        let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
        let missing = std::env::temp_dir().join(format!(
            "zelper-fake-process-notfound-{}-no-such/zellij",
            std::process::id()
        ));
        let backend = ZellijCliBackend::new("s").with_program(missing);
        let err = backend.run(&["--version".to_string()]).unwrap_err();
        assert_eq!(
            *err.class(),
            ErrorClass::ZellijUnavailable,
            "message: {}",
            err.message()
        );
        assert!(
            err.message()
                .contains("zellij executable not found in PATH"),
            "message: {}",
            err.message()
        );
    }

    /// DD-5・src/zellij/process.rs:66-70: NotFound以外のspawn失敗（実行権限なし等）も
    /// ZellijUnavailableへ分類され、messageへ起動error本文を埋め込む
    // [covers:backend-process.spawn-permission-denied-zellij-unavailable]
    #[test]
    fn spawn_permission_denied_becomes_zellij_unavailable() {
        let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
        let dir =
            std::env::temp_dir().join(format!("zelper-fake-process-noperm-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let shim = dir.join("zellij");
        std::fs::write(&shim, "#!/usr/bin/env bash\nexit 0\n").unwrap();
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o644)).unwrap();
        let backend = ZellijCliBackend::new("s").with_program(shim);
        let err = backend.run(&["--version".to_string()]).unwrap_err();
        assert_eq!(
            *err.class(),
            ErrorClass::ZellijUnavailable,
            "message: {}",
            err.message()
        );
        assert!(
            err.message().contains("failed to start zellij:"),
            "message: {}",
            err.message()
        );
    }

    /// test-plan §2.3・DD-3.5: --version出力がversion形式にparse不能な場合は
    /// UnsupportedVersionで拒否される（parse結果None経路）
    // [covers:backend-process.version-unparseable-unsupported]
    #[test]
    fn version_unparseable_becomes_unsupported() {
        let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
        let program = setup_fake_zellij(
            "version-bad",
            "#!/usr/bin/env bash\necho 'not-a-version'\nexit 0\n",
        );
        let backend = ZellijCliBackend::new("s").with_program(program);
        let err = check_capability(&backend).unwrap_err();
        assert_eq!(
            *err.class(),
            ErrorClass::UnsupportedVersion,
            "message: {}",
            err.message()
        );
        assert!(
            err.message()
                .contains("failed to parse zellij version output"),
            "message: {}",
            err.message()
        );
    }

    /// DD-3.1: 1呼び出しのtimeout超過はOperationFailedへ変換される。
    /// private field timeoutへ短時間を直接設定し、sleepするfake scriptで決定的に検証
    // [covers:backend-process.timeout-operation-failed]
    #[test]
    fn timeout_exceeded_becomes_operation_failed() {
        let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
        let program = setup_fake_zellij("timeout", "#!/usr/bin/env bash\nsleep 5\n");
        let mut backend = ZellijCliBackend::new("s").with_program(program);
        backend.timeout = Duration::from_millis(300);
        let err = backend.run(&["--version".to_string()]).unwrap_err();
        assert_eq!(
            *err.class(),
            ErrorClass::OperationFailed,
            "message: {}",
            err.message()
        );
        assert!(
            err.message().contains("zellij call timed out after"),
            "message: {}",
            err.message()
        );
    }

    /// DD-3.4・design-review §4.19 TR59-3: current-tab-info --jsonの出力が
    /// 単一tabに定まらない（parse結果が複数件）場合はOperationFailed。
    /// current_tabは`--session s action current-tab-info --json`のみ発行するため
    /// script応答は引数分岐不要の固定出力とする
    // [covers:backend-process.current-tab-unexpected-output-operation-failed]
    #[test]
    fn current_tab_unexpected_output_becomes_operation_failed() {
        let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
        let script = r#"#!/usr/bin/env bash
printf '%s\n' '[{"position":0,"name":"Tab #1","active":true,"are_floating_panes_visible":false,"selectable_tiled_panes_count":3,"selectable_floating_panes_count":0,"tab_id":0},{"position":1,"name":"Tab #2","active":false,"are_floating_panes_visible":false,"selectable_tiled_panes_count":2,"selectable_floating_panes_count":0,"tab_id":1}]'
exit 0
"#;
        let program = setup_fake_zellij("current-tab-multi", script);
        let backend = ZellijCliBackend::new("s").with_program(program);
        let err = backend.current_tab().unwrap_err();
        assert_eq!(
            *err.class(),
            ErrorClass::OperationFailed,
            "message: {}",
            err.message()
        );
        assert!(
            err.message().contains("unexpected current-tab-info output"),
            "message: {}",
            err.message()
        );
    }

    /// TASK-75設計§3.5（TR75-10）: focus_paneはargv組み立てとResultを返す薄い実装
    /// のみ（warning化・retryなし。PaneKindIdの系列分離はas_spec()が担いbackendは
    /// 分岐を持たない）。Terminal(3) -> focus-pane-id terminal_3。非zero exitは
    /// 現行run()系と同一のerror変換（OperationFailed）
    // [covers:backend-process.focus-pane-id-argv]
    #[test]
    fn focus_pane_id_argv_and_error_mapping() {
        let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "zelper-fake-process-focuspane-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("argv.log");
        let script = format!(
            "#!/usr/bin/env bash\nprintf '%s\\n' \"$*\" >> {}\nexit 0\n",
            log.display()
        );
        let program = setup_fake_zellij("focus-pane", &script);
        let backend = ZellijCliBackend::new("sess75").with_program(program);
        backend.focus_pane(&PaneKindId::Terminal(3)).unwrap();
        let recorded = std::fs::read_to_string(&log).unwrap();
        assert_eq!(
            recorded.trim(),
            "--session sess75 action focus-pane-id terminal_3",
            "argvはas_spec()由来のterminal_N形式"
        );

        // 非zero exitはOperationFailed（現行run()系と同一のerror変換）
        let program = setup_fake_zellij("focus-pane-fail", "#!/usr/bin/env bash\nexit 1\n");
        let backend = ZellijCliBackend::new("sess75").with_program(program);
        let err = backend.focus_pane(&PaneKindId::Terminal(3)).unwrap_err();
        assert_eq!(
            *err.class(),
            ErrorClass::OperationFailed,
            "message: {}",
            err.message()
        );
    }
}
