#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACT_FAMILY: &str = "patch";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatchStatus {
    Analyzed,
    Applied,
    Validated,
    Promoted,
    Failed,
}

impl PatchStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Analyzed => "analyzed",
            Self::Applied => "applied",
            Self::Validated => "validated",
            Self::Promoted => "promoted",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchTask {
    pub task_id: String,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub affected_systems: Vec<String>,
    #[serde(default)]
    pub expected_files: Vec<String>,
    #[serde(default)]
    pub validation_route: Vec<String>,
    #[serde(default)]
    pub requires_iteration: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatchRecord {
    pub patch_id: String,
    pub request: String,
    #[serde(default = "default_patch_status")]
    pub status: PatchStatus,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub tasks: Vec<PatchTask>,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub validation_summary: Value,
    #[serde(default)]
    pub analysis_summary: String,
    #[serde(default)]
    pub executor_result: Value,
    #[serde(default)]
    pub promoted_iteration_spec: String,
    #[serde(default)]
    pub errors: Vec<String>,
}

fn default_patch_status() -> PatchStatus {
    PatchStatus::Analyzed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_manifest_roundtrip_preserves_tasks_and_validation_summary() {
        let record = PatchRecord {
            patch_id: "patch_1".to_string(),
            request: "Add save button".to_string(),
            status: PatchStatus::Validated,
            created_at: "2026-07-08T00:00:00".to_string(),
            updated_at: "2026-07-08T00:01:00".to_string(),
            tasks: vec![PatchTask {
                task_id: "task_1".to_string(),
                title: "Wire command".to_string(),
                description: "Add backend command binding.".to_string(),
                affected_systems: vec!["save".to_string()],
                expected_files: vec!["src/save.rs".to_string()],
                validation_route: vec!["cargo test".to_string()],
                requires_iteration: false,
            }],
            changed_files: vec!["src/save.rs".to_string()],
            validation_summary: serde_json::json!({"status": "passed"}),
            analysis_summary: "Patch save button wiring.".to_string(),
            executor_result: serde_json::json!({"status": "success"}),
            promoted_iteration_spec: String::new(),
            errors: Vec::new(),
        };

        let restored: PatchRecord =
            serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
        assert_eq!(restored, record);
    }

    #[test]
    fn patch_rejects_invalid_status() {
        let invalid = r#"{"patch_id":"p","request":"r","status":"complete"}"#;
        assert!(serde_json::from_str::<PatchRecord>(invalid).is_err());
    }
}
