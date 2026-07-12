#![forbid(unsafe_code)]

use adm_new_contracts::{CommandError, CommandResponse, Diagnostic, EvidenceRef};
use adm_new_foundation::{AdmError, AdmResult};

pub mod ai;
pub mod config;
pub mod design;
pub mod logs;
pub mod package;
pub mod patch;
pub mod pipeline;
pub mod save;
pub mod sdk;
pub mod shell;

pub const CRATE_NAME: &str = "adm-new-tauri-commands";

pub type CommandAdapterResult<T> = CommandResponse<T>;

pub fn crate_ready() -> bool {
    true
}

pub fn command_success<T>(data: T) -> CommandResponse<T> {
    CommandResponse::success(data)
}

pub fn command_success_with<T>(
    data: T,
    evidence: Vec<EvidenceRef>,
    diagnostics: Vec<Diagnostic>,
) -> CommandResponse<T> {
    CommandResponse {
        ok: true,
        data: Some(data),
        evidence,
        diagnostics,
        error: None,
    }
}

pub fn command_failure<T>(error: CommandError) -> CommandResponse<T> {
    CommandResponse::failure(error)
}

pub fn command_failure_with_evidence<T>(
    mut error: CommandError,
    evidence: Vec<EvidenceRef>,
) -> CommandResponse<T> {
    error.evidence = evidence.clone();
    CommandResponse {
        ok: false,
        data: None,
        evidence,
        diagnostics: Vec::new(),
        error: Some(error),
    }
}

pub fn command_error(code: impl Into<String>, message: impl Into<String>) -> CommandError {
    CommandError::new(code, message)
}

pub fn command_error_from_adm(error: AdmError) -> CommandError {
    let message = error.message().to_string();
    let code = infer_error_code(&message);
    CommandError {
        code: code.to_string(),
        message,
        evidence: Vec::new(),
        recoverable: true,
    }
}

pub fn map_command_result<T>(result: AdmResult<T>) -> CommandResponse<T> {
    match result {
        Ok(data) => command_success(data),
        Err(error) => command_failure(command_error_from_adm(error)),
    }
}

pub fn handle_command<T, F>(handler: F) -> CommandResponse<T>
where
    F: FnOnce() -> AdmResult<T>,
{
    map_command_result(handler())
}

fn infer_error_code(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("outside root")
        || lower.contains("escapes root")
        || lower.contains("path must be relative")
        || lower.contains("not portable")
    {
        "PATH_GUARD_FAILED"
    } else if lower.contains("save archive is locked")
        || lower.contains("save index is locked")
        || lower.contains("save index lock acquisition")
        || lower.contains("already open in another window")
    {
        "SAVE_LOCKED"
    } else if lower.contains("invalid save manifest")
        || lower.contains("invalid archive state json")
        || lower.contains("corrupt save")
    {
        "SAVE_CORRUPT"
    } else if lower.contains("backend unavailable") || lower.contains("cli unavailable") {
        "BACKEND_UNAVAILABLE"
    } else if lower.contains("not configured")
        || lower.contains("active_entry_id is empty")
        || lower.contains("requires api_url")
        || lower.contains("requires codex")
    {
        "CONFIGURATION_REQUIRED"
    } else if lower.contains("unknown ")
        || lower.contains("not found")
        || lower.contains("missing ")
    {
        "NOT_FOUND"
    } else if lower.contains("invalid")
        || lower.contains("cannot")
        || lower.contains("must not")
        || lower.contains("must be")
        || lower.contains("required")
        || lower.contains("empty")
        || lower.contains("does not allow")
    {
        "VALIDATION_FAILED"
    } else {
        "COMMAND_FAILED"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::AdmError;

    #[test]
    fn crate_reports_ready() {
        assert!(crate_ready());
        assert_eq!(CRATE_NAME, "adm-new-tauri-commands");
    }

    #[test]
    fn success_response_serializes_stable_shape() {
        let response = command_success_with(
            "ready".to_string(),
            vec![EvidenceRef::new(
                "docs/independence/README.md",
                "ui_contract",
            )],
            vec![Diagnostic::info("command adapter mapped service result")],
        );

        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["ok"], true);
        assert_eq!(value["data"], "ready");
        assert_eq!(value["evidence"][0]["path"], "docs/independence/README.md");
        assert_eq!(value["evidence"][0]["kind"], "ui_contract");
        assert_eq!(value["diagnostics"][0]["level"], "INFO");
        assert!(value["error"].is_null());
    }

    #[test]
    fn error_response_serializes_stable_shape() {
        let response: CommandResponse<String> = command_failure_with_evidence(
            command_error("PACKAGE_BLOCKED", "Step14 validation did not pass"),
            vec![EvidenceRef::new("sandbox/outputs/stage_14", "artifact")],
        );

        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["ok"], false);
        assert!(value["data"].is_null());
        assert_eq!(value["error"]["code"], "PACKAGE_BLOCKED");
        assert_eq!(value["error"]["recoverable"], true);
        assert_eq!(value["error"]["evidence"][0]["kind"], "artifact");
        assert_eq!(value["evidence"][0]["path"], "sandbox/outputs/stage_14");
    }

    #[test]
    fn adm_error_maps_to_command_error_without_service_logic() {
        let error = command_error_from_adm(AdmError::new("project_id must not be empty"));
        assert_eq!(error.code, "VALIDATION_FAILED");
        assert_eq!(error.message, "project_id must not be empty");
        assert!(error.recoverable);
        assert!(error.evidence.is_empty());

        let path_error =
            command_error_from_adm(AdmError::new("path escapes root or is not portable"));
        assert_eq!(path_error.code, "PATH_GUARD_FAILED");

        let lock_error = command_error_from_adm(AdmError::new(
            "save index is locked by session desktop_a pid 42",
        ));
        assert_eq!(lock_error.code, "SAVE_LOCKED");
    }

    #[test]
    fn handler_helper_maps_success_and_error_results() {
        let ok = handle_command(|| Ok::<_, AdmError>("mapped".to_string()));
        assert!(ok.ok);
        assert_eq!(ok.data, Some("mapped".to_string()));
        assert!(ok.error.is_none());

        let failed: CommandResponse<String> =
            handle_command(|| Err(AdmError::new("unknown save_id: save_001")));
        assert!(!failed.ok);
        assert!(failed.data.is_none());
        assert_eq!(failed.error.unwrap().code, "NOT_FOUND");
    }
}
