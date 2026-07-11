#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACT_FAMILY: &str = "execution_object";
pub const EXECUTION_OBJECT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionObjectStatus {
    Draft,
    StaleDraft,
    Submitted,
    Analyzing,
    AwaitingConfirmation,
    Approved,
    ConflictBlocked,
    StaleBeforeExecution,
    Executing,
    CancellationRequested,
    ExecutionFailed,
    Verified,
    Rejected,
    Cancelled,
    Superseded,
}

impl ExecutionObjectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::StaleDraft => "stale_draft",
            Self::Submitted => "submitted",
            Self::Analyzing => "analyzing",
            Self::AwaitingConfirmation => "awaiting_confirmation",
            Self::Approved => "approved",
            Self::ConflictBlocked => "conflict_blocked",
            Self::StaleBeforeExecution => "stale_before_execution",
            Self::Executing => "executing",
            Self::CancellationRequested => "cancellation_requested",
            Self::ExecutionFailed => "execution_failed",
            Self::Verified => "verified",
            Self::Rejected => "rejected",
            Self::Cancelled => "cancelled",
            Self::Superseded => "superseded",
        }
    }
}

pub const FORMAL_ACTIVE_STATES: &[ExecutionObjectStatus] = &[
    ExecutionObjectStatus::Submitted,
    ExecutionObjectStatus::Analyzing,
    ExecutionObjectStatus::AwaitingConfirmation,
    ExecutionObjectStatus::Approved,
    ExecutionObjectStatus::ConflictBlocked,
    ExecutionObjectStatus::StaleBeforeExecution,
    ExecutionObjectStatus::Executing,
    ExecutionObjectStatus::CancellationRequested,
    ExecutionObjectStatus::ExecutionFailed,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationLevel {
    NormalConfirm,
    ElevatedConfirm,
    T3ArtConfirm,
    DestructiveConfirm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionObjectStoreDocument {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub save_id: Option<String>,
    #[serde(default)]
    pub objects: Vec<ExecutionObject>,
    #[serde(default)]
    pub audit_cleanup_evidence: Vec<Value>,
    #[serde(default)]
    pub ownership_migrations: Vec<OwnershipMigration>,
}

impl Default for ExecutionObjectStoreDocument {
    fn default() -> Self {
        Self {
            schema_version: EXECUTION_OBJECT_SCHEMA_VERSION,
            generated_at: String::new(),
            updated_at: String::new(),
            save_id: None,
            objects: Vec::new(),
            audit_cleanup_evidence: Vec::new(),
            ownership_migrations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionObject {
    pub execution_object_id: String,
    pub object_type: String,
    pub title: String,
    pub state: ExecutionObjectStatus,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub source_diagnostic_id: String,
    #[serde(default)]
    pub source_execution_object_id: String,
    #[serde(default)]
    pub prefilled_content: Value,
    #[serde(default)]
    pub user_content: Value,
    #[serde(default)]
    pub related_facts: Value,
    #[serde(default)]
    pub write_scope: Vec<String>,
    #[serde(default)]
    pub submission_snapshot: Option<SubmissionSnapshot>,
    #[serde(default)]
    pub final_submitted_content: Option<Value>,
    #[serde(default)]
    pub confirmation_level: Option<ConfirmationLevel>,
    #[serde(default)]
    pub impact_analysis: Option<Value>,
    #[serde(default)]
    pub confirmation_records: Vec<Value>,
    #[serde(default)]
    pub cancellation_records: Vec<Value>,
    #[serde(default)]
    pub drift_checks: Vec<Value>,
    #[serde(default)]
    pub conflict_checks: Vec<Value>,
    #[serde(default)]
    pub execution_records: Vec<Value>,
    #[serde(default)]
    pub failure_records: Vec<Value>,
    #[serde(default)]
    pub verification_records: Vec<Value>,
    #[serde(default)]
    pub audit_cleanup_evidence: Vec<Value>,
    #[serde(default)]
    pub state_history: Vec<StateHistoryRecord>,
    #[serde(default)]
    pub metadata: Value,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmissionSnapshot {
    pub snapshot_id: String,
    pub execution_object_id: String,
    pub draft_id: String,
    #[serde(default)]
    pub source_diagnostic_id: String,
    #[serde(default)]
    pub submitted_at: String,
    #[serde(default)]
    pub submitter_marker: String,
    #[serde(default)]
    pub submission_confirmation_marker: String,
    pub confirmation_level: ConfirmationLevel,
    #[serde(default)]
    pub related_facts: Value,
    #[serde(default)]
    pub write_scope: Vec<String>,
    #[serde(default)]
    pub prefilled_content_hash: String,
    #[serde(default)]
    pub final_content: Value,
    #[serde(default)]
    pub final_content_hash: String,
    #[serde(default)]
    pub prefill_to_final_diff_hash: String,
    #[serde(default)]
    pub stale_draft_refresh_source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateHistoryRecord {
    #[serde(default)]
    pub at: String,
    #[serde(default)]
    pub from: Option<ExecutionObjectStatus>,
    pub to: ExecutionObjectStatus,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub evidence: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnershipMigration {
    #[serde(default)]
    pub from_save_id: Option<String>,
    pub to_save_id: String,
    pub reason: String,
    #[serde(default)]
    pub at: String,
}

fn default_schema_version() -> u32 {
    EXECUTION_OBJECT_SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_execution_object_status_covers_python_workflow_states() {
        let states = [
            ExecutionObjectStatus::Draft,
            ExecutionObjectStatus::StaleDraft,
            ExecutionObjectStatus::Submitted,
            ExecutionObjectStatus::Analyzing,
            ExecutionObjectStatus::AwaitingConfirmation,
            ExecutionObjectStatus::Approved,
            ExecutionObjectStatus::ConflictBlocked,
            ExecutionObjectStatus::StaleBeforeExecution,
            ExecutionObjectStatus::Executing,
            ExecutionObjectStatus::CancellationRequested,
            ExecutionObjectStatus::ExecutionFailed,
            ExecutionObjectStatus::Verified,
            ExecutionObjectStatus::Rejected,
            ExecutionObjectStatus::Cancelled,
            ExecutionObjectStatus::Superseded,
        ];
        assert_eq!(states.len(), 15);
        assert_eq!(
            ExecutionObjectStatus::ExecutionFailed.as_str(),
            "execution_failed"
        );
        assert!(FORMAL_ACTIVE_STATES.contains(&ExecutionObjectStatus::Executing));
        assert!(!FORMAL_ACTIVE_STATES.contains(&ExecutionObjectStatus::Verified));
    }

    #[test]
    fn save_execution_object_store_roundtrip_preserves_verified_design_project() {
        let object = ExecutionObject {
            execution_object_id: "EO-000001".to_string(),
            object_type: "design_project".to_string(),
            title: "设计项目: Demo".to_string(),
            state: ExecutionObjectStatus::Verified,
            created_at: "2026-07-08T00:00:00".to_string(),
            updated_at: "2026-07-08T00:01:00".to_string(),
            source_diagnostic_id: "workbench:design_project:Demo".to_string(),
            source_execution_object_id: String::new(),
            prefilled_content: serde_json::json!({}),
            user_content: serde_json::json!({"projectName": "Demo"}),
            related_facts: serde_json::json!({"node_count": 1}),
            write_scope: vec![
                "design:project_state".to_string(),
                "design:nodes".to_string(),
            ],
            submission_snapshot: Some(SubmissionSnapshot {
                snapshot_id: "SS-EO-000001".to_string(),
                execution_object_id: "EO-000001".to_string(),
                draft_id: "EO-000001".to_string(),
                source_diagnostic_id: "workbench:design_project:Demo".to_string(),
                submitted_at: "2026-07-08T00:00:10".to_string(),
                submitter_marker: "workbench_user".to_string(),
                submission_confirmation_marker: "Demo:submitted".to_string(),
                confirmation_level: ConfirmationLevel::NormalConfirm,
                related_facts: serde_json::json!({"node_count": 1}),
                write_scope: vec!["design:project_state".to_string()],
                prefilled_content_hash: "h1".to_string(),
                final_content: serde_json::json!({"projectName": "Demo"}),
                final_content_hash: "h2".to_string(),
                prefill_to_final_diff_hash: "h3".to_string(),
                stale_draft_refresh_source: String::new(),
            }),
            final_submitted_content: Some(serde_json::json!({"projectName": "Demo"})),
            confirmation_level: Some(ConfirmationLevel::NormalConfirm),
            impact_analysis: Some(serde_json::json!({"analysis_id": "IA-EO-000001"})),
            confirmation_records: vec![serde_json::json!({"confirmed": true})],
            cancellation_records: Vec::new(),
            drift_checks: vec![serde_json::json!({"status": "passed"})],
            conflict_checks: vec![serde_json::json!({"status": "passed"})],
            execution_records: vec![serde_json::json!({"written_files": []})],
            failure_records: Vec::new(),
            verification_records: vec![serde_json::json!({"project_state_hash": "hash"})],
            audit_cleanup_evidence: Vec::new(),
            state_history: vec![StateHistoryRecord {
                at: "2026-07-08T00:00:00".to_string(),
                from: None,
                to: ExecutionObjectStatus::Draft,
                reason: "created".to_string(),
                evidence: serde_json::json!({}),
            }],
            metadata: serde_json::json!({"save_type": "manual"}),
            extra: BTreeMap::new(),
        };
        let store = ExecutionObjectStoreDocument {
            schema_version: EXECUTION_OBJECT_SCHEMA_VERSION,
            generated_at: "2026-07-08T00:00:00".to_string(),
            updated_at: "2026-07-08T00:01:00".to_string(),
            save_id: Some("save-1".to_string()),
            objects: vec![object],
            audit_cleanup_evidence: Vec::new(),
            ownership_migrations: Vec::new(),
        };

        let json = serde_json::to_string(&store).unwrap();
        assert!(json.contains("\"state\":\"verified\""));
        let restored: ExecutionObjectStoreDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, store);
    }

    #[test]
    fn save_execution_object_rejects_invalid_state() {
        let invalid =
            r#"{"execution_object_id":"EO-1","object_type":"x","title":"x","state":"done"}"#;
        assert!(serde_json::from_str::<ExecutionObject>(invalid).is_err());
    }
}
