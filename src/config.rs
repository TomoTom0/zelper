//! zelperのユーザー設定（TASK-1）。tomlファイル1つのみ・全項目任意。
//!
//! 探索順: `ZELPER_CONFIG`（path直接指定。存在しなければerror）>
//! `$XDG_CONFIG_HOME/zelper/config.toml` > `$HOME/.config/zelper/config.toml`。
//! 既定位置にfileが無いことはerrorにしない（設定なし = 全項目未指定）。
//!
//! ```toml
//! [session]
//! default_layout = "work"   # add session の layout 既定（名前またはpath）
//! ```
use crate::error::{ErrorClass, ZelperError};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub session: SessionConfig,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionConfig {
    pub default_layout: Option<String>,
}

pub fn parse(text: &str, origin: &Path) -> Result<Config, ZelperError> {
    toml::from_str(text).map_err(|e| {
        ZelperError::new(
            ErrorClass::Usage,
            format!("invalid config {}: {}", origin.display(), e.message()),
        )
    })
}

/// 設定fileのpath解決。(path, 明示指定か)
fn config_path() -> Option<(PathBuf, bool)> {
    if let Some(p) = std::env::var_os("ZELPER_CONFIG").filter(|v| !v.is_empty()) {
        return Some((PathBuf::from(p), true));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|h| PathBuf::from(h).join(".config"))
        })?;
    Some((base.join("zelper").join("config.toml"), false))
}

pub fn load() -> Result<Config, ZelperError> {
    let Some((path, explicit)) = config_path() else {
        return Ok(Config::default());
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => parse(&text, &path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && !explicit => Ok(Config::default()),
        Err(e) => Err(ZelperError::new(
            ErrorClass::Usage,
            format!("cannot read config {}: {e}", path.display()),
        )),
    }
}
