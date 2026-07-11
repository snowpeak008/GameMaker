#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ArtifactLocale;

pub const CONTRACT_FAMILY: &str = "pipeline";
pub const PIPELINE_CHECKPOINT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageKind {
    Design,
    Development,
    HumanGate,
    Validation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Success,
    Failed,
    Skipped,
    Blocked,
    Stopped,
    WaitingConfirmation,
    CompletedWithReview,
}

impl StageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Blocked => "blocked",
            Self::Stopped => "stopped",
            Self::WaitingConfirmation => "waiting_confirmation",
            Self::CompletedWithReview => "completed_with_review",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageContextModel {
    pub stage_id: String,
    #[serde(default)]
    pub project_root: String,
    #[serde(default)]
    pub inputs: BTreeMap<String, Value>,
    #[serde(default)]
    pub outputs: BTreeMap<String, Value>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
    #[serde(default)]
    pub knowledge: BTreeMap<String, String>,
    #[serde(default)]
    pub skills: BTreeMap<String, Value>,
    #[serde(default)]
    pub test_mode: bool,
    #[serde(default)]
    pub artifact_dir: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStageResult {
    pub status: StageStatus,
    #[serde(default)]
    pub outputs: BTreeMap<String, Value>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub message: String,
}

impl PipelineStageResult {
    pub fn ok(&self) -> bool {
        self.status == StageStatus::Success
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageMetadata {
    pub stage_id: String,
    #[serde(default)]
    pub number: Option<u32>,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub requires: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceGroupSpec {
    pub label: String,
    pub pattern: String,
    pub mode: String,
    pub source_type: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageSpec {
    pub stage_id: String,
    #[serde(default = "default_stage_kind")]
    pub kind: StageKind,
    #[serde(default)]
    pub number: Option<u32>,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub source_groups: Vec<SourceGroupSpec>,
    #[serde(default)]
    pub plugin_ref: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineRegistry {
    #[serde(default)]
    pub stages: Vec<StageSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStageRuntime {
    pub stage_id: String,
    pub status: StageStatus,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub completed_at: String,
    #[serde(default)]
    pub result: Option<PipelineStageResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineRunState {
    #[serde(default)]
    pub schema_version: u32,
    pub run_id: String,
    #[serde(default)]
    pub attempt_id: String,
    #[serde(default)]
    pub parent_attempt_id: Option<String>,
    #[serde(default)]
    pub attempt_no: u32,
    #[serde(default)]
    pub state_version: u64,
    #[serde(default = "default_pipeline_status")]
    pub status: String,
    #[serde(default)]
    pub stop_requested: bool,
    #[serde(default)]
    pub from_stage_id: String,
    #[serde(default)]
    pub to_stage_id: String,
    #[serde(default)]
    pub stage_ids: Vec<String>,
    #[serde(default)]
    pub current_stage_id: Option<String>,
    #[serde(default)]
    pub current_unit_id: Option<String>,
    #[serde(default)]
    pub recovery: Option<PipelineRecoverySummary>,
    #[serde(default)]
    pub stages: BTreeMap<String, PipelineStageRuntime>,
}

impl Default for PipelineRunState {
    fn default() -> Self {
        Self {
            schema_version: default_pipeline_state_schema_version(),
            run_id: String::new(),
            attempt_id: String::new(),
            parent_attempt_id: None,
            attempt_no: 0,
            state_version: 0,
            status: default_pipeline_status(),
            stop_requested: false,
            from_stage_id: String::new(),
            to_stage_id: String::new(),
            stage_ids: Vec::new(),
            current_stage_id: None,
            current_unit_id: None,
            recovery: None,
            stages: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineRunIdentity {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub attempt_id: String,
    #[serde(default)]
    pub parent_attempt_id: Option<String>,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub draft_id: String,
    #[serde(default)]
    pub save_id: Option<String>,
    /// Immutable user-facing artifact language captured when the run starts.
    #[serde(default)]
    pub artifact_locale: ArtifactLocale,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalPipelineRange {
    #[serde(default)]
    pub from_stage_id: String,
    #[serde(default)]
    pub to_stage_id: String,
    #[serde(default)]
    pub stage_ids: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineFingerprints {
    #[serde(default)]
    pub input: String,
    #[serde(default)]
    pub configuration: String,
    #[serde(default)]
    pub execution_plan: String,
    #[serde(default)]
    pub application: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineCheckpointStatus {
    #[default]
    Running,
    StopRequested,
    Stopping,
    Stopped,
    Recoverable,
    Resuming,
    WaitingConfirmation,
    Completed,
    Failed,
    RecoveryBlocked,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineUnitStatus {
    #[default]
    Pending,
    Running,
    ResultReady,
    Committed,
    Failed,
    Unknown,
    Skipped,
}

impl PipelineUnitStatus {
    pub fn is_committed(&self) -> bool {
        matches!(self, Self::Committed | Self::Skipped)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineResumePolicy {
    #[default]
    ExplicitOnly,
    Disabled,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineUnitCheckpoint {
    #[serde(default)]
    pub stage_id: String,
    #[serde(default)]
    pub unit_id: String,
    #[serde(default)]
    pub status: PipelineUnitStatus,
    #[serde(default)]
    pub idempotency_key: String,
    #[serde(default)]
    pub started_at: String,
    #[serde(default)]
    pub completed_at: String,
    #[serde(default)]
    pub result_fingerprint: String,
    #[serde(default)]
    pub output_refs: Vec<String>,
    #[serde(default)]
    pub reconcile_required: bool,
    #[serde(default)]
    pub failure_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineCheckpoint {
    #[serde(default = "default_checkpoint_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub identity: PipelineRunIdentity,
    #[serde(default)]
    pub range: CanonicalPipelineRange,
    #[serde(default)]
    pub status: PipelineCheckpointStatus,
    #[serde(default)]
    pub current_stage_id: Option<String>,
    #[serde(default)]
    pub current_unit_id: Option<String>,
    #[serde(default)]
    pub next_unit_id: Option<String>,
    #[serde(default)]
    pub units: Vec<PipelineUnitCheckpoint>,
    #[serde(default)]
    pub fingerprints: PipelineFingerprints,
    #[serde(default)]
    pub resume_policy: PipelineResumePolicy,
    #[serde(default)]
    pub skip_manual_gates: bool,
    #[serde(default)]
    pub stop_reason: String,
    #[serde(default)]
    pub stop_boundary: String,
    #[serde(default)]
    pub recovery_blocked_reason: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

impl Default for PipelineCheckpoint {
    fn default() -> Self {
        Self {
            schema_version: PIPELINE_CHECKPOINT_SCHEMA_VERSION,
            revision: 0,
            identity: PipelineRunIdentity::default(),
            range: CanonicalPipelineRange::default(),
            status: PipelineCheckpointStatus::default(),
            current_stage_id: None,
            current_unit_id: None,
            next_unit_id: None,
            units: Vec::new(),
            fingerprints: PipelineFingerprints::default(),
            resume_policy: PipelineResumePolicy::default(),
            skip_manual_gates: false,
            stop_reason: String::new(),
            stop_boundary: String::new(),
            recovery_blocked_reason: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineRecoverySummary {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub attempt_id: String,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub status: PipelineCheckpointStatus,
    #[serde(default)]
    pub from_stage_id: String,
    #[serde(default)]
    pub to_stage_id: String,
    #[serde(default)]
    pub current_stage_id: Option<String>,
    #[serde(default)]
    pub next_unit_id: Option<String>,
    #[serde(default)]
    pub updated_at: String,
}

impl From<&PipelineCheckpoint> for PipelineRecoverySummary {
    fn from(checkpoint: &PipelineCheckpoint) -> Self {
        Self {
            run_id: checkpoint.identity.run_id.clone(),
            attempt_id: checkpoint.identity.attempt_id.clone(),
            revision: checkpoint.revision,
            status: checkpoint.status.clone(),
            from_stage_id: checkpoint.range.from_stage_id.clone(),
            to_stage_id: checkpoint.range.to_stage_id.clone(),
            current_stage_id: checkpoint.current_stage_id.clone(),
            next_unit_id: checkpoint.next_unit_id.clone(),
            updated_at: checkpoint.updated_at.clone(),
        }
    }
}

fn default_stage_kind() -> StageKind {
    StageKind::Development
}

fn default_pipeline_status() -> String {
    "idle".to_string()
}

fn default_pipeline_state_schema_version() -> u32 {
    2
}

fn default_checkpoint_schema_version() -> u32 {
    PIPELINE_CHECKPOINT_SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_stage_result_roundtrip_preserves_completed_with_review() {
        let result = PipelineStageResult {
            status: StageStatus::CompletedWithReview,
            outputs: BTreeMap::from([(
                "blockers".to_string(),
                serde_json::json!(["standalone partial"]),
            )]),
            errors: Vec::new(),
            warnings: vec!["converted blocked result".to_string()],
            message: "partial validation preserved warnings".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("completed_with_review"));
        let restored: PipelineStageResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, result);
        assert!(!restored.ok());
    }

    #[test]
    fn pipeline_stage_context_roundtrip_preserves_knowledge_and_skills() {
        let ctx = StageContextModel {
            stage_id: "14".to_string(),
            project_root: "E:/project".to_string(),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            metadata: BTreeMap::from([(
                "validation_scope".to_string(),
                Value::String("standalone_partial".to_string()),
            )]),
            knowledge: BTreeMap::from([(
                "knowledge/Core_Rules.md".to_string(),
                "rules".to_string(),
            )]),
            skills: BTreeMap::from([("unity".to_string(), serde_json::json!({"enabled": true}))]),
            test_mode: false,
            artifact_dir: "outputs/artifacts/stage_14".to_string(),
        };

        let json = serde_json::to_string(&ctx).unwrap();
        let restored: StageContextModel = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, ctx);
    }

    #[test]
    fn pipeline_rejects_invalid_stage_status() {
        let invalid = r#"{"status":"done","outputs":{}}"#;
        assert!(serde_json::from_str::<PipelineStageResult>(invalid).is_err());
    }

    #[test]
    fn pipeline_registry_and_run_state_roundtrip() {
        let registry = PipelineRegistry {
            stages: vec![StageSpec {
                stage_id: "10".to_string(),
                kind: StageKind::Development,
                number: Some(10),
                slug: "asset_alignment".to_string(),
                title: "Asset Alignment".to_string(),
                requires: vec!["08".to_string(), "09".to_string()],
                source_groups: vec![SourceGroupSpec {
                    label: "alignment".to_string(),
                    pattern: "devflow_Alignment_*".to_string(),
                    mode: "latest".to_string(),
                    source_type: "Alignment".to_string(),
                }],
                plugin_ref: "pipeline.step10".to_string(),
                metadata: BTreeMap::new(),
            }],
        };
        let state = PipelineRunState {
            run_id: "run-1".to_string(),
            status: "running".to_string(),
            stop_requested: false,
            current_stage_id: Some("10".to_string()),
            stages: BTreeMap::from([(
                "10".to_string(),
                PipelineStageRuntime {
                    stage_id: "10".to_string(),
                    status: StageStatus::Success,
                    started_at: "unix:1".to_string(),
                    completed_at: "unix:2".to_string(),
                    result: None,
                },
            )]),
            ..PipelineRunState::default()
        };

        let registry_json = serde_json::to_string(&registry).unwrap();
        let state_json = serde_json::to_string(&state).unwrap();
        assert_eq!(
            serde_json::from_str::<PipelineRegistry>(&registry_json).unwrap(),
            registry
        );
        assert_eq!(
            serde_json::from_str::<PipelineRunState>(&state_json).unwrap(),
            state
        );
    }

    #[test]
    fn checkpoint_defaults_are_backward_compatible() {
        let checkpoint: PipelineCheckpoint = serde_json::from_str(
            r#"{"identity":{"run_id":"run_1","attempt_id":"attempt_1"},"range":{"from_stage_id":"00","to_stage_id":"01"}}"#,
        )
        .unwrap();
        assert_eq!(
            checkpoint.schema_version,
            PIPELINE_CHECKPOINT_SCHEMA_VERSION
        );
        assert_eq!(checkpoint.status, PipelineCheckpointStatus::Running);
        assert_eq!(checkpoint.resume_policy, PipelineResumePolicy::ExplicitOnly);
        assert!(checkpoint.units.is_empty());
        assert_eq!(checkpoint.identity.artifact_locale, ArtifactLocale::ZhCn);
    }

    #[test]
    fn checkpoint_identity_freezes_artifact_locale() {
        let checkpoint = PipelineCheckpoint {
            identity: PipelineRunIdentity {
                run_id: "run_locale".to_string(),
                attempt_id: "attempt_locale".to_string(),
                artifact_locale: ArtifactLocale::EnUs,
                ..PipelineRunIdentity::default()
            },
            ..PipelineCheckpoint::default()
        };

        let json = serde_json::to_string(&checkpoint).unwrap();
        let restored: PipelineCheckpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.identity.artifact_locale, ArtifactLocale::EnUs);
        assert!(json.contains("\"artifact_locale\":\"en-US\""));
    }

    #[test]
    fn recovery_summary_does_not_expose_checkpoint_paths() {
        let checkpoint = PipelineCheckpoint {
            revision: 3,
            identity: PipelineRunIdentity {
                run_id: "run_1".to_string(),
                attempt_id: "attempt_2".to_string(),
                ..PipelineRunIdentity::default()
            },
            range: CanonicalPipelineRange {
                from_stage_id: "00".to_string(),
                to_stage_id: "12".to_string(),
                ..CanonicalPipelineRange::default()
            },
            status: PipelineCheckpointStatus::Recoverable,
            next_unit_id: Some("task_4".to_string()),
            ..PipelineCheckpoint::default()
        };
        let summary = PipelineRecoverySummary::from(&checkpoint);
        assert_eq!(summary.next_unit_id.as_deref(), Some("task_4"));
        assert!(!serde_json::to_string(&summary).unwrap().contains("path"));
    }
}
