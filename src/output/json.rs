use crate::error::ZelperError;
use serde::Serialize;

pub const SCHEMA_VERSION: u32 = 1;

/// JSON成功応答（DD-4.2）
#[derive(Debug, Serialize)]
pub struct OkEnvelope<T> {
    pub schema_version: u32,
    pub ok: bool,
    pub data: T,
}

/// JSON失敗応答（DD-4.2）
#[derive(Debug, Serialize)]
pub struct ErrEnvelope {
    pub schema_version: u32,
    pub ok: bool,
    pub error: ErrorOut,
}

#[derive(Debug, Serialize)]
pub struct ErrorOut {
    pub class: String,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
    /// 部分失敗時のper-target結果等の構造化data（付与されている場合のみ）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// multi-target結果の1要素（stable fields: target, ok）
#[derive(Debug, Serialize)]
pub struct TargetedResult<T> {
    pub target: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn ok<T: Serialize>(data: T) -> String {
    let env = OkEnvelope {
        schema_version: SCHEMA_VERSION,
        ok: true,
        data,
    };
    serde_json::to_string(&env).unwrap_or_default()
}

pub fn err(e: &ZelperError) -> String {
    let env = ErrEnvelope {
        schema_version: SCHEMA_VERSION,
        ok: false,
        error: ErrorOut {
            class: e.class().as_str().to_string(),
            message: e.message().to_string(),
            candidates: e.candidates().to_vec(),
            data: e.data().cloned(),
        },
    };
    serde_json::to_string(&env).unwrap_or_default()
}
