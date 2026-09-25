use super::parser;
use super::{MIN_SUPPORTED, NewPaneSpec, NewTabSpec, OverrideSpec, ResizeOp, ZellijBackend};
use crate::domain::{PaneKindId, SessionRef, TabId, TabState};
use crate::error::{ErrorClass, ZelperError};
use std::io::{Read, Seek, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// stdはfd差し替え（dup2）・close-on-exec付き複製（fcntl F_DUPFD_CLOEXEC）・
// signal disposition変更を公開していないため、zelperが既にlink済みのシステム
// libcの該当関数を直接宣言する（依存追加を避ける。TASK-89）
unsafe extern "C" {
    fn dup2(oldfd: std::os::fd::RawFd, newfd: std::os::fd::RawFd) -> std::os::fd::RawFd;
    fn fcntl(fd: std::os::fd::RawFd, cmd: std::os::raw::c_int, ...) -> std::os::raw::c_int;
    fn signal(signum: std::os::raw::c_int, handler: usize) -> usize;
}

/// fcntl(2)のF_DUPFD_CLOEXEC（Linux。REVIEW-2指摘2: 複製fdに原子的に
/// close-on-execを付ける。dup(2)の戻り値はCLOEXECなしで子へ漏れるため不使用）
const F_DUPFD_CLOEXEC: std::os::raw::c_int = 1030;
const SIGINT: std::os::raw::c_int = 2;
const SIG_IGN: usize = 1;

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

    /// spawn失敗のclass変換（run()/run_interactive共通。挙動は従来のrun()と同一）
    fn map_spawn_err(e: std::io::Error) -> ZelperError {
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
    }

    /// 対話実行（attach専用。DD-3.1例外・DD-14）: stdioはinherit×3でspawnし、
    /// timeout無効（`self.timeout`を参照しない。対話sessionは時間無制限が本質で
    /// run()のtimeoutで打ち切られるべきでない）で子の終了まで待つ。
    ///
    /// stderrのみ、起動前に親自身のfd 2を一時的なcapture fileへ差し替えた上で
    /// 継承する（StdioCapture）。これにより (1) 子のfd 0/1/2は親の対応fdと同一
    /// targetのまま（inherit×3の維持。ただしfd 2のtargetはcapture fileであり、
    /// 元stderrのTTY属性・即時可視性は失われる。fd 0/1は元targetを継承する）
    /// (2) 子のstderr出力をcaptureでき、終了後に本来のstderrへ一括転写するとともに
    /// 非zero終了時のerror messageへ埋め込む（backend-process.nonzero-exit-
    /// operation-failedと同一規則。zellij attachの不在session error〔Session not
    /// found + 稼働session候補〕をJSON経路にも届ける。v1で対話中の即時転送は行わない）
    ///
    /// SIGINTは実行中のみ親側でSIG_IGNへ変更する（REVIEW-2指摘3）: 同一
    /// foreground process groupではCtrl-CのSIGINTが親（zelper）にも届くため、
    /// 子zellijがSIGINTを受けて処理・終了する間に親がstderr転写・報告前に死ぬ
    /// ことを防ぐ。installはspawn成功後（SIG_IGNはexecを通じて子に継承される
    /// ため、子の起動前に親へ設定すると子までSIGINTを無視してしまう）。子の
    /// 終了検出後に旧dispositionへ復元する
    fn run_interactive(&self, args: &[String]) -> Result<(), ZelperError> {
        let mut cap = StderrCapture::new()?;
        let mut cmd = Command::new(&self.program);
        cmd.args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let e = Self::map_spawn_err(e);
                let _ = cap.restore();
                return Err(e);
            }
        };
        // SAFETY: SIGINT dispositionの一時変更（子はspawn済みのため影響しない。
        // restore_sigintで必ず戻す）
        let prev_sig = unsafe { ignore_sigint() };
        // timeout無効: child.wait()のblocking待機のみ（poll/killなし）
        let status = match child.wait() {
            Ok(s) => s,
            Err(e) => {
                let e = ZelperError::new(
                    ErrorClass::OperationFailed,
                    format!("failed to wait for zellij: {e}"),
                );
                // SAFETY: 上記ignore_sigintで退避したdispositionへ戻す
                unsafe { restore_sigint(prev_sig) };
                let _ = cap.restore();
                return Err(e);
            }
        };
        // SAFETY: 上記ignore_sigintで退避したdispositionへ戻す
        unsafe { restore_sigint(prev_sig) };
        if let Err(e) = cap.restore() {
            // 復元不能な失敗はerror経路で報告（capture内容の転写先が確定しない
            // ため読み取り結果は破棄する）
            return Err(ZelperError::new(
                ErrorClass::OperationFailed,
                format!("failed to restore stderr after zellij attach ({status}): {e}"),
            ));
        }
        let stderr = cap.read();
        if !stderr.trim().is_empty() {
            // captureしたstderrを本来のstderrへ転写（inherit相当の可視性の維持。
            // 改行終端していなければ補う）
            if stderr.ends_with('\n') {
                eprint!("{stderr}");
            } else {
                eprintln!("{stderr}");
            }
        }
        if status.success() {
            Ok(())
        } else {
            Err(ZelperError::new(
                ErrorClass::OperationFailed,
                format!("zellij exited with {}: {}", status, stderr.trim()),
            ))
        }
    }

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

        let mut child = cmd.spawn().map_err(Self::map_spawn_err)?;

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

    fn attach_session(&self) -> Result<(), ZelperError> {
        // positional形式 `zellij attach <session>`（DD-14。global --session形式は
        // M5実測で意図しない挙動になるため不使用）
        self.run_interactive(&["attach".to_string(), self.session.clone()])
    }

    fn detach_session(&self) -> Result<(), ZelperError> {
        // 現行run_actionの枠のwrap（`zellij --session NAME action detach`）
        self.run_action(&["detach"])?;
        Ok(())
    }

    fn create_session_detached(&self, name: &str) -> Result<(), ZelperError> {
        // 非対話作成（M5実測: one-shot・rc=0・silent）。session解決に依存しない
        // top-level argv（run_actionの--session接頭なし）
        self.run(&["attach".to_string(), "-b".to_string(), name.to_string()])?;
        Ok(())
    }
}

/// run_interactive用のstderr capture（DD-14）。親自身のfd 2を一時的なcapture
/// fileへ差し替え、子spawn中は親子で同一fd targetを保つ（stdio inherit×3の
/// 維持）。restore後にcapture内容を読み出せる。
///
/// 復元（restore）の失敗はResultとしてcaller（run_interactiveのerror経路）へ
/// 報告される。Dropはbest-effort（panic時等の最終防衛。失敗は通知できない）
struct StderrCapture {
    /// 差し替え前のfd 2の退避先（None = 未差し替え/復元済み。CLOEXEC付きで
    /// 複製されているため子へは継承されない）
    saved: Option<std::os::fd::RawFd>,
    file: Option<std::fs::File>,
    dir: PathBuf,
}

impl StderrCapture {
    fn new() -> Result<Self, ZelperError> {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // 呼出毎に一意なdir（同一process内の並行実行との分離）
        let dir =
            std::env::temp_dir().join(format!("zelper-attach-stderr-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).map_err(|e| {
            ZelperError::new(
                ErrorClass::OperationFailed,
                format!("failed to prepare stderr capture: {e}"),
            )
        })?;
        let path = dir.join("stderr.log");
        // REVIEW-2指摘1: create_new（O_CREAT|O_EXCL）で排他作成する。同名fileや
        // symlinkが既に存在すれば失敗し、symlinkを辿った既存fileのtruncateを構造
        // 排除する。mode(0o600)で作成時点から権限を限定する
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(|e| {
                ZelperError::new(
                    ErrorClass::OperationFailed,
                    format!("failed to prepare stderr capture: {e}"),
                )
            })?;
        // 作成直後にunlinkしてFDのみ保持する（path経由の追記・読取を不能にする。
        // 転写はFDのseek(0)読み取りで行う。失敗したらFDを閉じて中断）
        if let Err(e) = std::fs::remove_file(&path) {
            return Err(ZelperError::new(
                ErrorClass::OperationFailed,
                format!("failed to prepare stderr capture: unlink failed: {e}"),
            ));
        }
        let mut cap = StderrCapture {
            saved: None,
            file: Some(file),
            dir,
        };
        // SAFETY: fd 2はこの時点で有効なstderr。fcntl(F_DUPFD_CLOEXEC)は退避用の
        // 複製（close-on-exec付き。子への継承なし）を作るだけでfd 2を変更しない。
        // dup2でfd 2をcapture fileへ差し替える（失敗時はfd 2は不変のため退避分を
        // 閉じて元の状態へ戻す）
        unsafe {
            let saved = fcntl(2, F_DUPFD_CLOEXEC, 0);
            if saved < 0 {
                return Err(ZelperError::new(
                    ErrorClass::OperationFailed,
                    format!(
                        "failed to prepare stderr capture: fcntl F_DUPFD_CLOEXEC failed: {}",
                        std::io::Error::last_os_error()
                    ),
                ));
            }
            let file_fd = cap.file.as_ref().expect("file opened").as_raw_fd();
            if dup2(file_fd, 2) < 0 {
                drop(std::os::fd::OwnedFd::from_raw_fd(saved));
                return Err(ZelperError::new(
                    ErrorClass::OperationFailed,
                    format!(
                        "failed to prepare stderr capture: dup2 failed: {}",
                        std::io::Error::last_os_error()
                    ),
                ));
            }
            cap.saved = Some(saved);
        }
        Ok(cap)
    }

    /// fd 2を本来のstderrへ戻す（冪等）。復元dup2が成功するまで退避FDを保持し
    /// （REVIEW-2指摘4: takeして閉じると再試行不能になるため）、EINTRは再試行
    /// する。復元成功後にのみ退避FDを閉じる
    fn restore(&mut self) -> std::io::Result<()> {
        let Some(saved) = self.saved else {
            return Ok(());
        };
        loop {
            // SAFETY: savedはfcntl(2, F_DUPFD_CLOEXEC, 0)で作った有効な退避FD。
            // 成功するまで閉じない
            let r = unsafe { dup2(saved, 2) };
            if r >= 0 {
                break;
            }
            let e = std::io::Error::last_os_error();
            if e.kind() != std::io::ErrorKind::Interrupted {
                return Err(e);
            }
        }
        self.saved = None;
        // SAFETY: 復元済み。退避FDを閉じる
        unsafe { drop(std::os::fd::OwnedFd::from_raw_fd(saved)) }
        Ok(())
    }

    /// captureしたstderr内容の読み出し（restore後。fileはunlink済みだがFD経由で
    /// 読める）
    fn read(&mut self) -> String {
        let Some(f) = self.file.as_mut() else {
            return String::new();
        };
        let _ = f.flush();
        let mut buf = String::new();
        if f.seek(std::io::SeekFrom::Start(0)).is_ok() {
            let _ = f.read_to_string(&mut buf);
        }
        buf
    }
}

impl Drop for StderrCapture {
    fn drop(&mut self) {
        // best-effort（明示経路での復元失敗はrun_interactiveが報告済み）
        let _ = self.restore();
        self.file = None;
        let _ = std::fs::remove_dir(&self.dir);
    }
}

/// SIGINTを一時的にSIG_IGNへ変更し、旧dispositionを返す（DD-14・REVIEW-2指摘3）。
/// run_interactiveの実行中、同一foreground groupのCtrl-Cで子zellijがSIGINTを
/// 処理して終了する間に親が転写・報告前に死なないための処置
///
/// # Safety
/// dispositionはprocess-wide。callerは戻り値をrestore_sigintへ渡して必ず戻す
unsafe fn ignore_sigint() -> usize {
    // SAFETY: SIGINT dispositionの変更（callerがrestore_sigintで戻す契約）
    unsafe { signal(SIGINT, SIG_IGN) }
}

/// ignore_sigintで退避したdispositionへSIGINTを復元し、復元直前のdispositionを
/// 返す
///
/// # Safety
/// prevはignore_sigintの戻り値（またはそれと同型の正当なdisposition値）であること
unsafe fn restore_sigint(prev: usize) -> usize {
    // SAFETY: prevは退避済みの正当なdisposition値
    unsafe { signal(SIGINT, prev) }
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

    /// TASK-89（DD-14・設計§2.3/§5.2）: attachのbackend経路（run_interactive）は
    /// stdioをinherit×3でspawnし（fd 0/1/2とも子は親の対応fdと同一targetを継承。
    /// stderrは一時的captureへ差し替えた上での継承）、run()のtimeout機構を適用
    /// しない（対話sessionは時間無制限が本質）。(a)は子がfd 0/1/2の参照先を親
    /// （getppid）と比較し報告fileへ書くfake programでinheritを検出（あわせて
    /// REVIEW-2指摘2: 退避FD〔元stderrの複製〕がclose-on-exec付きで子へ漏れて
    /// いないことをfd 3以上の一覧で検証）、(b)はbackendのtimeout値を超えて
    /// sleepするfake programがtimeout errorにならずOkで完結することでtimeout無効
    /// を検証、(c)はSIGINT dispositionのinstall→restore往復を検証する
    /// （実signal送信はtest全体を終了させるため対象外——関数の往復のみ）
    // [covers:attach-detach.attach-run-interactive-stdio-inherit-no-timeout]
    #[test]
    fn attach_run_interactive_inherits_stdio_and_skips_timeout() {
        let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
        // (a) stdio inherit検出: 子のfd 0/1/2の参照先を親と比較し報告fileへ書く。
        // あわせてfd 3以上の一覧（target付き）も報告させる
        let dir = std::env::temp_dir().join(format!(
            "zelper-fake-process-attachfd-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let report = dir.join("fd_report.txt");
        let script = format!(
            r#"#!/usr/bin/env bash
p=$PPID
: > "{report}"
for n in 0 1 2; do
  # 注: /proc/self だと readlinkプロセス自身（$()のpipe）が見えるため
  # $$（このscriptのPID）で比較する
  a=$(readlink /proc/$$/fd/$n)
  b=$(readlink /proc/$p/fd/$n)
  if [[ "$a" == "$b" ]]; then
    echo "fd$n=same" >> "{report}"
  else
    echo "fd$n=DIFF" >> "{report}"
  fi
done
# fd 3以上の一覧（退避FD漏れ検出用。loop setup時にglob展開されるため
# report追記用に一時開くfdは一覧に現れない）
for f in /proc/self/fd/*; do
  n="${{f##*/}}"
  if (( n >= 3 )); then
    echo "extra$n=$(readlink "$f")" >> "{report}"
  fi
done
exit 0
"#,
            report = report.display()
        );
        let program = setup_fake_zellij("attach-fd", &script);
        let backend = ZellijCliBackend::new("sess89").with_program(program);
        // 差し替え前の元stderrのtarget（退避FDが子へ漏れていた場合にこのtargetと
        // 同一のfdが子の一覧へ現れる）
        let orig_stderr = std::fs::read_link("/proc/self/fd/2").expect("readlink stderr");
        backend.attach_session().unwrap();
        let recorded = std::fs::read_to_string(&report).unwrap();
        for want in ["fd0=same", "fd1=same", "fd2=same"] {
            assert!(recorded.lines().any(|l| l == want), "fd report: {recorded}");
        }
        // REVIEW-2指摘2: 退避FD（元stderrの複製）はF_DUPFD_CLOEXEC付きで作られる
        // ため子へ継承されない——fd 3以上に元stderrと同一targetのfdが現れない。
        // （bash自身のscript fd〔255等〕は現れてよいが、それは元stderrと別target）
        for line in recorded.lines().filter(|l| l.starts_with("extra")) {
            let (_, target) = line.split_once('=').expect("extra<N>=<target>");
            assert_ne!(
                target,
                orig_stderr.to_str().unwrap_or("?"),
                "退避FDが子へ漏れている: {line}"
            );
        }

        // (b) timeout無効: backend.timeout（数百ms）を超えてsleepしてもtimeout
        // error（zellij call timed out）にならずOkで完結する
        let program = setup_fake_zellij("attach-slow", "#!/usr/bin/env bash\nsleep 1\nexit 0\n");
        let mut backend = ZellijCliBackend::new("sess89").with_program(program);
        backend.timeout = Duration::from_millis(300);
        backend.attach_session().unwrap();

        // (c) SIGINT dispositionのinstall→restore往復（REVIEW-2指摘3）:
        // ignore→復元すると復元直前のdispositionはSIG_IGN、復元後に再度ignoreすると
        // 戻り値（直前のdisposition）は最初に退避した値と一致する。最後に戻す
        // SAFETY: dispositionの往復（最後に元へ戻す）
        unsafe {
            let prev = ignore_sigint();
            let cur = restore_sigint(prev);
            assert_eq!(cur, SIG_IGN, "復元直前のdispositionはSIG_IGN");
            let prev2 = ignore_sigint();
            assert_eq!(prev2, prev, "復値後のignoreの戻り値は旧dispositionと一致");
            let _ = restore_sigint(prev2);
        }
    }

    /// TASK-89（DD-14）: attachのzellij非zero終了はOperationFailed（exit 5相当）へ
    /// 変換され、messageへexit statusとtrim済みstderr本文（不在sessionのSession
    /// not found + 稼働session候補表示）が埋め込まれる。Session not foundを
    /// NoTarget（exit 3）へ切り替えない（stderr内容によるclass切替は設計上存在
    /// しない——backend-process.nonzero-exit-operation-failedと同一規則。
    /// --session明示時はresolve_sessionが存在確認せずpass-throughのため不在名の
    /// 検出はzellij側になる）
    // [covers:attach-detach.attach-zellij-fail]
    #[test]
    fn attach_nonzero_exit_becomes_operation_failed_with_stderr() {
        let _guard = FAKE_ZELLIJ_LOCK.lock().unwrap();
        let script = "#!/usr/bin/env bash\n\
            echo \"Session 'ghost' not found. The following sessions are active:\" >&2\n\
            echo \"sess-a\" >&2\n\
            echo \"sess-b\" >&2\n\
            exit 1\n";
        let program = setup_fake_zellij("attach-fail", script);
        let backend = ZellijCliBackend::new("ghost").with_program(program);
        let err = backend.attach_session().unwrap_err();
        assert_eq!(
            *err.class(),
            ErrorClass::OperationFailed,
            "message: {}",
            err.message()
        );
        let msg = err.message();
        assert!(msg.contains("exit status: 1"), "message: {msg}");
        assert!(msg.contains("Session 'ghost' not found"), "message: {msg}");
        assert!(msg.contains("sess-a"), "message: {msg}");
        assert!(msg.contains("sess-b"), "message: {msg}");
    }
}
