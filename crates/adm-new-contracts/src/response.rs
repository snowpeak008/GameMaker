#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub path: String,
    pub kind: String,
    pub hash: Option<String>,
}

impl EvidenceRef {
    pub fn new(path: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            kind: kind.into(),
            hash: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub level: String,
    pub message: String,
}

impl Diagnostic {
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            level: "INFO".to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
    pub evidence: Vec<EvidenceRef>,
    pub recoverable: bool,
}

impl CommandError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            evidence: Vec::new(),
            recoverable: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResponse<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub evidence: Vec<EvidenceRef>,
    pub diagnostics: Vec<Diagnostic>,
    pub error: Option<CommandError>,
}

impl<T> CommandResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            evidence: Vec::new(),
            diagnostics: Vec::new(),
            error: None,
        }
    }

    pub fn failure(error: CommandError) -> Self {
        Self {
            ok: false,
            data: None,
            evidence: Vec::new(),
            diagnostics: Vec::new(),
            error: Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_response_success_and_failure_are_explicit() {
        let ok = CommandResponse::success("ready");
        assert!(ok.ok);
        assert_eq!(ok.data, Some("ready"));
        assert!(ok.error.is_none());

        let failed: CommandResponse<&str> =
            CommandResponse::failure(CommandError::new("VALIDATION_FAILED", "invalid input"));
        assert!(!failed.ok);
        assert!(failed.data.is_none());
        assert_eq!(failed.error.unwrap().code, "VALIDATION_FAILED");
    }
}
