//! companion plugin setup（DD-10.3/10.4）。wasm extractとpermissions.kdl seedを提供。
//!
//! - wasm: zelper binary埋込（build.rsが`plugin/`をbuild）を
//!   `$XDG_CACHE_HOME/zelper/companion/<zelper-version>/zelper-companion.wasm`へextract
//!   （書込は同dirのtemp file + atomic rename。version入pathのため別versionとの混在・
//!   skewが構造的に発生しない）。この絶対pathがpipeの`file:<path>`とpermissions.kdlの
//!   node名に使われる（実行時解決。XDG_CACHE_HOME変更に追従）
//! - permissions: `$XDG_CACHE_HOME/zellij/permissions.kdl`にcompanion node（node名 =
//!   wasm絶対path文字列）と必要権限を事前記述すると対話dialogなしでgrantされる
//!   （E6実証。session起動前のseedで実証済み）

use crate::error::{ErrorClass, ZelperError};
use std::path::{Path, PathBuf};

/// companion pluginが必要とする権限（DD-10.3。load()のrequest_permissionと同集合。
/// permissions.kdlではnodeの子に権限名のbare nodeとして記述する）
pub const REQUIRED_PERMISSIONS: &[&str] = &[
    "ReadApplicationState",
    "ChangeApplicationState",
    "ReadCliPipes",
];

/// 実行回数の上限（seed書込競合のbounded retry。DD-10.4・設計レビューR2）
const MAX_SEED_RETRIES: usize = 3;

/// companion plugin wasm（build.rsがOUT_DIRへ配置したartifactを埋込参照。DD-10.3）
const COMPANION_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/zelper-companion.wasm"));

/// companion node block（E6実験のpermissions.kdl形式）。
/// node名 = plugin wasm絶対pathのquoted KDL node・権限は子のbare node
fn node_block(node: &str, perms: &[&str]) -> String {
    let escaped = format!("\"{}\"", node.replace('\\', "\\\\").replace('"', "\\\""));
    let mut out = format!("{escaped} {{\n");
    for p in perms {
        out.push_str(&format!("    {p}\n"));
    }
    out.push_str("}\n");
    out
}

/// 現在のpermissions.kdl内容からseed後の内容を返す（純粋関数。DD-10.4手順2）。
///
/// - `Ok(None)`: 書込不要（companion nodeが存在し必要権限すべてあり）
/// - `Ok(Some(new))`: 書込むべき新内容全体
///   - node存在・権限不足: 不足権限のみ追記して書戻し（他node・他権限は保持）
///   - node不在: 既存内容への純append（kdl round-trip整形によらない。
///     既存内容が改行終端でなければ追記部の先頭に改行を補完する）
/// - `Err`: 既存内容のKDL parse失敗はPreflight error（書き換えず手動修復を促す。
///   ユーザーの他plugin権限を壊すリスクの排除を優先）
pub fn seeded_content(
    content: Option<&str>,
    companion_node: &str,
) -> Result<Option<String>, ZelperError> {
    let Some(text) = content else {
        // file不存在: companion nodeのみの新規file（手順1）
        return Ok(Some(node_block(companion_node, REQUIRED_PERMISSIONS)));
    };
    let mut doc = kdl::KdlDocument::parse(text).map_err(|e| {
        ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "cannot parse existing permissions.kdl ({e}); fix the file manually so \
zelper will not risk other plugins' permissions: {companion_node}"
            ),
        )
    })?;

    let Some(node) = doc
        .nodes_mut()
        .iter_mut()
        .find(|n| n.name().value() == companion_node)
    else {
        // node不在: file末尾へのtext純append。追記前に既存内容が改行終端でなければ
        // 先頭に改行を補い、前行末尾への連結によるKDL破壊を構造的に防ぐ（設計R1）
        let mut out = text.to_string();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&node_block(companion_node, REQUIRED_PERMISSIONS));
        return Ok(Some(out));
    };

    let existing: Vec<String> = node
        .children()
        .map(|c| c.nodes().iter().map(|n| n.name().value().to_string()))
        .into_iter()
        .flatten()
        .collect();
    let missing: Vec<&str> = REQUIRED_PERMISSIONS
        .iter()
        .copied()
        .filter(|p| !existing.iter().any(|e| e == p))
        .collect();
    if missing.is_empty() {
        return Ok(None);
    }
    // node存在・権限不足: 不足権限のみ追記（他node・他権限は保持）
    node.ensure_children();
    if let Some(children) = node.children_mut().as_mut() {
        for p in missing {
            let mut perm = kdl::KdlNode::new(p);
            perm.set_format(kdl::KdlNodeFormat {
                leading: "\n    ".into(),
                ..Default::default()
            });
            children.nodes_mut().push(perm);
        }
    }
    Ok(Some(format!("{doc}")))
}

/// XDG cache直下のzellij権限file path（`$XDG_CACHE_HOME/zellij/permissions.kdl`。
/// 未設定時は`~/.cache/zellij/`。DD-10.4）
pub fn permissions_path() -> Result<PathBuf, ZelperError> {
    let base = xdg_cache_home()?;
    Ok(base.join("zellij").join("permissions.kdl"))
}

/// wasm配置dir（`$XDG_CACHE_HOME/zelper/companion/<zelper-version>/`。DD-10.3）
pub fn companion_dir() -> Result<PathBuf, ZelperError> {
    Ok(xdg_cache_home()?
        .join("zelper")
        .join("companion")
        .join(env!("CARGO_PKG_VERSION")))
}

fn xdg_cache_home() -> Result<PathBuf, ZelperError> {
    if let Ok(x) = std::env::var("XDG_CACHE_HOME")
        && !x.is_empty()
    {
        return Ok(PathBuf::from(x));
    }
    let home = std::env::var("HOME").map_err(|_| {
        ZelperError::new(
            ErrorClass::Preflight,
            "cannot resolve cache dir: neither XDG_CACHE_HOME nor HOME is set",
        )
    })?;
    Ok(PathBuf::from(home).join(".cache"))
}

/// 必要ならcompanion plugin wasmをcache dirへextractし、その絶対pathを返す（DD-10.3）。
/// すでに存在すれば書かない。書込は同dirのtemp file作成 + atomic rename
pub fn ensure_companion_wasm() -> Result<PathBuf, ZelperError> {
    let dir = companion_dir()?;
    let dest = dir.join("zelper-companion.wasm");
    if dest.is_file() {
        return Ok(dest);
    }
    std::fs::create_dir_all(&dir).map_err(|e| {
        ZelperError::new(
            ErrorClass::Preflight,
            format!("cannot create companion cache dir {}: {e}", dir.display()),
        )
    })?;
    let tmp = dir.join(format!(".zelper-companion.wasm.tmp-{}", std::process::id()));
    std::fs::write(&tmp, COMPANION_WASM).map_err(|e| {
        // 部分書込のtemp fileを残さない（best-effort cleanup）
        let _ = std::fs::remove_file(&tmp);
        ZelperError::new(
            ErrorClass::Preflight,
            format!("cannot write {}: {e}", tmp.display()),
        )
    })?;
    std::fs::rename(&tmp, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "cannot move companion wasm into place ({} -> {}): {e}",
                tmp.display(),
                dest.display()
            ),
        )
    })?;
    Ok(dest)
}

/// permissions.kdlへのseed実行（DD-10.4全体）。手順:
/// 読込 -> `seeded_content`判定 -> 書込直前に再読込し内容が変化していたら
/// 変化後の内容に対してやり直す（bounded retry 3回） -> 同dir temp file作成 +
/// atomic renameによる書込。retry打ち切り後も競合が続く場合は書込まず
/// Preflight errorで中断する（last-writer-winsで他pluginの権限を黙って
/// 失わせない）。file不存在時は対象dirを作成してcompanion nodeのみの新規fileを書く。
/// 読込から書込までの区間はzelper間advisory lock（flock）で直列化する。
///
/// 既知の残リスク（設計レビューC2）: 再読込一致確認後からatomic renameまでの間に
/// zellij自身がpermissions.kdlを書き換える競合は外部から排除できず、その場合は
/// zellij側の更新がzelperの書込で上書きされうる（lost-update）。このwindowは
/// 再読込->renameの最小区間に縮小済みであり、zellijはrequest_permission呼出毎に
/// fileを再読込するため、他pluginの権限喪失時は当該pluginが次の権限要求で回復
/// できる（黙示の恒久喪失ではない）
///
/// `on_before_write`: 書込直前（再読込のタイミング）に呼ぶhook。zellij他processの
/// 書換（競合）を模擬するテスト注入用。実運用からはNoneを渡す
pub fn ensure_permissions(
    path: &std::path::Path,
    companion_node: &str,
    on_before_write: Option<&dyn Fn(&std::path::Path)>,
) -> Result<(), ZelperError> {
    // lock fileを置くため対象dirを先に作成する（file不存在時の新規作成caseを含む）
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            ZelperError::new(
                ErrorClass::Preflight,
                format!("cannot create {}: {e}", parent.display()),
            )
        })?;
    }
    // zelper自身の並行実行に対するadvisory lock: 読込->判定->書込の区間を直列化する。
    // zellij自身の書換はこのlockの外（対象外。上記残リスク）
    let _lock = SeedLock::acquire(path.parent().unwrap_or(path))?;

    let mut retries = 0usize;
    loop {
        let first = read_optional(path)?;
        let Some(new) = seeded_content(first.as_deref(), companion_node)? else {
            return Ok(()); // 書込不要（node存在・権限すべてあり）
        };
        // 書戻し直前の再読込（hookはこのタイミングで他process書換を模擬する）
        if let Some(hook) = on_before_write {
            hook(path);
        }
        let second = read_optional(path)?;
        if second == first {
            write_atomic(path, &new)?;
            return Ok(());
        }
        // zellij他processの書換を検知: 変化後の内容に対してやり直す（bounded retry）
        retries += 1;
        if retries > MAX_SEED_RETRIES {
            return Err(ZelperError::new(
                ErrorClass::Preflight,
                format!(
                    "permissions.kdl conflict: another process kept rewriting it; zelper \
did not write after {MAX_SEED_RETRIES} retries (other plugins' permissions are preserved). \
retry later or grant permissions to the companion plugin manually: {companion_node}"
                ),
            ));
        }
    }
}

/// permissions.kdlと同dirのadvisory lock file（flock）によるzelper間直列化guard。
/// Dropでunlock + lock file削除（best-effort）
struct SeedLock {
    file: Option<std::fs::File>,
    path: std::path::PathBuf,
}

impl SeedLock {
    fn acquire(dir: &std::path::Path) -> Result<Self, ZelperError> {
        let path = dir.join(".zelper-seed.lock");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false) // lock対象として開くのみで内容は使わない（残っていても再利用）
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| {
                ZelperError::new(
                    ErrorClass::Preflight,
                    format!("cannot open seed lock {}: {e}", path.display()),
                )
            })?;
        flock_exclusive(&file)?;
        Ok(SeedLock {
            file: Some(file),
            path,
        })
    }
}

impl Drop for SeedLock {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            let _ = unlock(&file);
        }
        // 残ったlock fileは次回実行で再利用可能だが、dirを清潔に保つため削除する
        let _ = std::fs::remove_file(&self.path);
    }
}

/// 排他flock（blocking）。zelper間のadvisory lockであり、保持側は常に短時間で
/// 解放する（読込->seed->書込の区間のみ）
#[cfg(unix)]
fn flock_exclusive(file: &std::fs::File) -> Result<(), ZelperError> {
    use std::os::unix::io::AsRawFd;
    const LOCK_EX: std::ffi::c_int = 2;
    // SAFETY: fdは開いているfile descriptorであり、flockはadvisory lock操作を行う
    unsafe extern "C" {
        fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
    }
    let rc = unsafe { flock(file.as_raw_fd(), LOCK_EX) };
    if rc != 0 {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            "cannot acquire the permissions.kdl seed lock".to_string(),
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn flock_exclusive(_file: &std::fs::File) -> Result<(), ZelperError> {
    Ok(())
}

#[cfg(unix)]
fn unlock(file: &std::fs::File) -> Result<(), ZelperError> {
    use std::os::unix::io::AsRawFd;
    const LOCK_UN: std::ffi::c_int = 8;
    // SAFETY: fdは開いているfile descriptorであり、flockはadvisory lock操作を行う
    unsafe extern "C" {
        fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int;
    }
    let rc = unsafe { flock(file.as_raw_fd(), LOCK_UN) };
    if rc != 0 {
        return Err(ZelperError::new(
            ErrorClass::Preflight,
            "cannot release the permissions.kdl seed lock".to_string(),
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn unlock(_file: &std::fs::File) -> Result<(), ZelperError> {
    Ok(())
}

/// 読込（不在はNone。それ以外の失敗はPreflight error）
fn read_optional(path: &Path) -> Result<Option<String>, ZelperError> {
    match std::fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ZelperError::new(
            ErrorClass::Preflight,
            format!("cannot read {}: {e}", path.display()),
        )),
    }
}

/// 同dir temp file作成 + atomic renameによる書込（readerが途中状態を観測しない。
/// 設計レビューR2）。対象dirが無ければ作成する。write失敗時も部分書込tempを
/// best-effortで削除する（残留防止）
fn write_atomic(path: &Path, content: &str) -> Result<(), ZelperError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            ZelperError::new(
                ErrorClass::Preflight,
                format!("cannot create {}: {e}", parent.display()),
            )
        })?;
    }
    let tmp = path.with_file_name(format!(
        ".{}.zelper-tmp-{}",
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        std::process::id()
    ));
    std::fs::write(&tmp, content).map_err(|e| {
        // 部分書込のtemp fileを残さない（best-effort cleanup）
        let _ = std::fs::remove_file(&tmp);
        ZelperError::new(
            ErrorClass::Preflight,
            format!("cannot write {}: {e}", tmp.display()),
        )
    })?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        ZelperError::new(
            ErrorClass::Preflight,
            format!(
                "cannot rename {} into place ({}): {e}",
                tmp.display(),
                path.display()
            ),
        )
    })?;
    Ok(())
}
