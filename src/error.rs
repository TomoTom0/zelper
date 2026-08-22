/// zelperのerror class（DD-4.2）。JSON出力の `error.class` と1:1。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorClass {
    Usage,
    ZellijUnavailable,
    UnsupportedVersion,
    NoTarget,
    AmbiguousTarget,
    LayoutNotFound,
    LayoutInvalid,
    Preflight,
    OperationFailed,
    PartialFailure,
    VerificationFailed,
}

impl ErrorClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorClass::Usage => "Usage",
            ErrorClass::ZellijUnavailable => "ZellijUnavailable",
            ErrorClass::UnsupportedVersion => "UnsupportedVersion",
            ErrorClass::NoTarget => "NoTarget",
            ErrorClass::AmbiguousTarget => "AmbiguousTarget",
            ErrorClass::LayoutNotFound => "LayoutNotFound",
            ErrorClass::LayoutInvalid => "LayoutInvalid",
            ErrorClass::Preflight => "Preflight",
            ErrorClass::OperationFailed => "OperationFailed",
            ErrorClass::PartialFailure => "PartialFailure",
            ErrorClass::VerificationFailed => "VerificationFailed",
        }
    }

    /// exit status（DD-4.3対応表）
    pub fn exit_code(&self) -> i32 {
        match self {
            ErrorClass::Usage => 2,
            ErrorClass::NoTarget | ErrorClass::AmbiguousTarget => 3,
            ErrorClass::ZellijUnavailable | ErrorClass::UnsupportedVersion => 4,
            ErrorClass::OperationFailed => 5,
            ErrorClass::PartialFailure => 6,
            ErrorClass::Preflight
            | ErrorClass::LayoutNotFound
            | ErrorClass::LayoutInvalid
            | ErrorClass::VerificationFailed => 7,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ZelperError {
    #[error("{message}")]
    Classified {
        class: ErrorClass,
        message: String,
        candidates: Vec<String>,
        /// 構造化された付帯情報（部分失敗時のper-target結果等）。JSON出力時のみ使用
        data: Option<serde_json::Value>,
    },
}

impl ZelperError {
    pub fn new(class: ErrorClass, message: impl Into<String>) -> Self {
        ZelperError::Classified {
            class,
            message: message.into(),
            candidates: Vec::new(),
            data: None,
        }
    }

    pub fn with_candidates(
        class: ErrorClass,
        message: impl Into<String>,
        candidates: Vec<String>,
    ) -> Self {
        ZelperError::Classified {
            class,
            message: message.into(),
            candidates,
            data: None,
        }
    }

    /// 構造化dataを付与（部分失敗のresults等）
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        let ZelperError::Classified { data: d, .. } = &mut self;
        *d = Some(data);
        self
    }

    pub fn data(&self) -> Option<&serde_json::Value> {
        match self {
            ZelperError::Classified { data, .. } => data.as_ref(),
        }
    }

    pub fn class(&self) -> &ErrorClass {
        match self {
            ZelperError::Classified { class, .. } => class,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            ZelperError::Classified { message, .. } => message,
        }
    }

    pub fn candidates(&self) -> &[String] {
        match self {
            ZelperError::Classified { candidates, .. } => candidates,
        }
    }

    pub fn exit_code(&self) -> i32 {
        self.class().exit_code()
    }
}
