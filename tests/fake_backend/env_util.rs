// test支援資産: env操作helperの共通部（TASK-83 Phase 1。fake.rsと同じ
// `mod env_util;` include pattern。test支援資産のため条件書tag対象外）。
// TASK-71でcompanion_seed.rsに確立したEnvGuard + lock_env構成を
// fake_remap / fake_move へ拡適用した共通正本。
// binaryは別processで走るためstatic lockがbinary毎に複製されることは正しい
// （envはprocess単位・binary間排他は不要）

/// env一時変更のRAII guard: 生成時に元値をvar_osで保存し（非UTF-8値も未設定と
/// 区別して保持）、Dropで復元する。closure内でpanicしてもunwind経由で確実に
/// 復元される
struct EnvGuard {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    /// safety: XDG_LOCKで直列化した範囲内でのみ生成すること（Dropでの復元も
    /// 同範囲のlock保持中に行われる）
    unsafe fn save(keys: &[&'static str]) -> Self {
        EnvGuard {
            saved: keys.iter().map(|k| (*k, std::env::var_os(k))).collect(),
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, prev) in &self.saved {
            // safety: 保存した元値への復元のみを行う
            match prev {
                Some(p) => unsafe { std::env::set_var(key, p) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

/// env一時変更helper群の直列化lock（companion setupのcache配下書込をtest実envから
/// 分離する検証で使用）
static XDG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// XDG_LOCK取得。poisonは無視する: envの復元はEnvGuardのDropが保証するため、
/// poison（panic搬送）は隔離対象の状態破壊を意味せず、後続testを巻き込まない
fn lock_env() -> std::sync::MutexGuard<'static, ()> {
    XDG_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// XDG_CACHE_HOME隔離（companion setupのcache書込をtest実envから分離。
/// 一時dirを作成してXDG_CACHE_HOMEへ向け、fへそのpathを渡す）
pub(crate) fn with_isolated_xdg<T>(f: impl FnOnce(&std::path::Path) -> T) -> T {
    let _lock = lock_env();
    let dir = std::env::temp_dir().join(format!("zelper-env-xdg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // 宣言順の逆順でdropするため、env復元（_env）はlock解放（_lock）に先行する
    let _env = unsafe { EnvGuard::save(&["XDG_CACHE_HOME"]) };
    // safety: XDG_LOCKで直列化した範囲内でのみ一時的に上書きする
    unsafe { std::env::set_var("XDG_CACHE_HOME", &dir) };
    f(&dir)
}

/// HOME隔離 + XDG_CACHE_HOME未設定（未設定時の既定path解決の検証に使用）。
/// 実userの~/.cacheを汚染しないようHOMEを一時dirへ向けてからXDG_CACHE_HOMEをunset
/// する。fへは隔離したHOMEのpathを渡す
#[allow(dead_code)] // 一部のtest binaryから未使用になりうる（fake.rsと同一対応）
pub(crate) fn with_isolated_home_no_xdg<T>(f: impl FnOnce(&std::path::Path) -> T) -> T {
    let _lock = lock_env();
    let dir = std::env::temp_dir().join(format!("zelper-env-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // 宣言順の逆順でdropするため、env復元（_env）はlock解放（_lock）に先行する
    let _env = unsafe { EnvGuard::save(&["HOME", "XDG_CACHE_HOME"]) };
    // safety: XDG_LOCKで直列化した範囲内でのみ一時的に上書き/unsetする
    unsafe { std::env::set_var("HOME", &dir) };
    unsafe { std::env::remove_var("XDG_CACHE_HOME") };
    f(&dir)
}

/// XDG_CACHE_HOMEを任意のpathへ一時指し替えする（companion cache dir作成失敗等の
/// 失敗注入に使用。fの実行中のみ指し替え、抜ける時に元へ戻す。dirの作成は
/// 呼び出し側の責務——失敗注入では「存在しないdir」や「dirでないfile」を
/// 指すことが多いため）
#[allow(dead_code)] // 一部のtest binaryから未使用になりうる（fake.rsと同一対応）
pub(crate) fn with_xdg_redirected_to<T>(path: &std::path::Path, f: impl FnOnce() -> T) -> T {
    let _lock = lock_env();
    // 宣言順の逆順でdropするため、env復元（_env）はlock解放（_lock）に先行する
    let _env = unsafe { EnvGuard::save(&["XDG_CACHE_HOME"]) };
    // safety: XDG_LOCKで直列化した範囲内でのみ一時的に上書きする
    unsafe { std::env::set_var("XDG_CACHE_HOME", path) };
    f()
}
