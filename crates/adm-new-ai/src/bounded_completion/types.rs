use std::collections::{BTreeMap, BTreeSet};

use adm_new_change_kernel::{ChangeAuditRecord, ExpectedSpecValue, SpecPatch, SpecValueChange};
use adm_new_game_spec::ProductEnvelope;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const BOUNDED_COMPLETION_SCHEMA_VERSION: &str = "1.0";
pub const CANDIDATE_SPEC_PATCH_SCHEMA: &str = "candidate_spec_patch_v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromptPack {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    pub task_id: String,
    pub model_config_id: String,
    pub base_revision: u64,
    pub base_hash: String,
    pub product_envelope: ProductEnvelope,
    #[serde(default)]
    pub relevant_subgraph: Value,
    #[serde(default)]
    pub open_questions: Vec<String>,
    pub allowed_write_paths: BTreeSet<String>,
    #[serde(default = "candidate_schema_value")]
    pub output_schema: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateSpecPatch {
    pub patch_id: String,
    pub base_revision: u64,
    pub base_hash: String,
    pub declared_write_paths: BTreeSet<String>,
    pub operations: Vec<CandidateSpecOperation>,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub evidence_summary: Vec<String>,
    #[serde(default)]
    pub open_items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateSpecOperation {
    pub path: String,
    pub expected_old_value: ExpectedSpecValue,
    pub change: SpecValueChange,
    pub reason: String,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub evidence_summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionRunStatus {
    NotCalled,
    Failed,
    Rejected,
    Confirmed,
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImpactSummary {
    pub operation_count: usize,
    pub changed_root_fields: BTreeSet<String>,
    pub touches_protected_field: bool,
    pub touches_product_envelope: bool,
    pub low_confidence_operation_count: usize,
    pub product_envelope: ProductEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmationRecord {
    pub mode: String,
    pub accepted: bool,
    pub actor: String,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_size: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundedCompletionAudit {
    pub schema_version: String,
    pub model_config_id: String,
    pub input_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_hash: Option<String>,
    pub validation_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<CompletionRisk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<ConfirmationRecord>,
    #[serde(default)]
    pub errors: Vec<String>,
    pub attempts: u32,
    pub schema_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundedCompletionRun {
    pub status: CompletionRunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_patch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<CompletionRisk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub impact: Option<ImpactSummary>,
    pub audit: BoundedCompletionAudit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec_audit: Option<ChangeAuditRecord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedCandidate {
    pub candidate: CandidateSpecPatch,
    pub spec_patch: SpecPatch,
    pub risk: CompletionRisk,
    pub impact: ImpactSummary,
    pub preflight_audit: ChangeAuditRecord,
}

fn default_schema_version() -> String {
    BOUNDED_COMPLETION_SCHEMA_VERSION.to_string()
}

fn candidate_schema_value() -> Value {
    serde_json::json!({
        "schema": CANDIDATE_SPEC_PATCH_SCHEMA,
        "required": [
            "patchId",
            "baseRevision",
            "baseHash",
            "declaredWritePaths",
            "operations"
        ],
        "operation": {
            "path": "RFC 6901 JSON Pointer under one allowedWritePaths entry",
            "expectedOldValue": "SpecStore ExpectedSpecValue",
            "change": "SpecStore SpecValueChange",
            "reason": "non-empty reason",
            "confidence": "0.0-1.0"
        }
    })
}

pub(crate) fn object_from_map(map: BTreeMap<String, Value>) -> Value {
    Value::Object(map.into_iter().collect())
}
