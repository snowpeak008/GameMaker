#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACT_FAMILY: &str = "artifact";

pub const REVIEWER_WHITELIST: &[&str] = &[
    "structure_reviewer",
    "source_trace_reviewer",
    "task_reviewer",
    "dependency_reviewer",
];

pub const VALIDATOR_WHITELIST: &[&str] = &[
    "validator_first_contract",
    "stage_files_validator",
    "review_report_validator",
    "manifest_validator",
    "schema_contract_validator",
    "knowledge_refs_validator",
    "dependency_status_validator",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactCheckStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactReportStatus {
    Success,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactRegistry {
    pub version: u32,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub default_reviewers: Vec<String>,
    #[serde(default)]
    pub default_validators: Vec<String>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactContract>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactContract {
    pub id: String,
    pub stage: u32,
    pub kind: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
    #[serde(default)]
    pub tasks: Vec<ArtifactTask>,
    #[serde(default)]
    pub reviewers: Vec<String>,
    #[serde(default)]
    pub validators: Vec<String>,
    #[serde(default)]
    pub schema_refs: Vec<SchemaRef>,
    #[serde(default)]
    pub knowledge_refs: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ArtifactContract {
    pub fn unknown_reviewers(&self) -> Vec<String> {
        unknown_names(&self.reviewers, REVIEWER_WHITELIST)
    }

    pub fn unknown_validators(&self) -> Vec<String> {
        unknown_names(&self.validators, VALIDATOR_WHITELIST)
    }

    pub fn duplicate_task_ids(&self) -> Vec<String> {
        let mut seen = BTreeSet::new();
        let mut duplicates = BTreeSet::new();
        for task in &self.tasks {
            if !seen.insert(task.id.clone()) {
                duplicates.insert(task.id.clone());
            }
        }
        duplicates.into_iter().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTask {
    pub id: String,
    #[serde(default, rename = "type")]
    pub task_type: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaRef {
    pub path: String,
    pub schema: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactLayerManifest {
    pub step: u32,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub stage_dir: String,
    #[serde(default)]
    pub artifacts: Vec<ArtifactContract>,
    #[serde(default)]
    pub tasks: Vec<ArtifactTaskWithArtifact>,
    #[serde(default)]
    pub file_manifest: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTaskWithArtifact {
    pub id: String,
    #[serde(default, rename = "type")]
    pub task_type: String,
    #[serde(default)]
    pub description: String,
    pub artifact_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreflightReport {
    pub step: u32,
    #[serde(default)]
    pub timestamp: String,
    pub status: ArtifactReportStatus,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub artifacts: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactReviewReport {
    pub step: u32,
    #[serde(default)]
    pub timestamp: String,
    pub status: ArtifactReportStatus,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub reviews: Vec<ArtifactReview>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactReview {
    pub artifact_id: String,
    pub status: ArtifactCheckStatus,
    #[serde(default)]
    pub results: Vec<ArtifactCheckResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactValidationLayerReport {
    pub step: u32,
    #[serde(default)]
    pub timestamp: String,
    pub status: ArtifactReportStatus,
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub validations: Vec<ArtifactValidation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactValidation {
    pub artifact_id: String,
    pub status: ArtifactCheckStatus,
    #[serde(default)]
    pub results: Vec<ArtifactCheckResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactCheckResult {
    pub name: String,
    pub status: ArtifactCheckStatus,
    #[serde(default = "default_info_severity")]
    pub severity: ArtifactSeverity,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraph {
    #[serde(default)]
    pub nodes: Vec<DependencyGraphNode>,
    #[serde(default)]
    pub edges: Vec<DependencyGraphEdge>,
    #[serde(default)]
    pub topological_order: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraphNode {
    pub id: String,
    pub stage: u32,
    #[serde(default)]
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyGraphEdge {
    pub from: String,
    pub to: String,
}

fn unknown_names(values: &[String], whitelist: &[&str]) -> Vec<String> {
    let allowed: BTreeSet<&str> = whitelist.iter().copied().collect();
    values
        .iter()
        .filter(|value| !allowed.contains(value.as_str()))
        .cloned()
        .collect()
}

fn default_info_severity() -> ArtifactSeverity {
    ArtifactSeverity::Info
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_contract() -> ArtifactContract {
        ArtifactContract {
            id: "stage_14.integration_validation_bundle".to_string(),
            stage: 14,
            kind: "source_placeholder_or_import".to_string(),
            depends_on: vec!["stage_13.scene_assembly_bundle".to_string()],
            tasks: vec![
                ArtifactTask {
                    id: "stage_14.import_integration".to_string(),
                    task_type: "import".to_string(),
                    description: "Import integration validation.".to_string(),
                },
                ArtifactTask {
                    id: "stage_14.validate_package".to_string(),
                    task_type: "deterministic_validation".to_string(),
                    description: "Validate package readiness.".to_string(),
                },
            ],
            reviewers: REVIEWER_WHITELIST
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            validators: VALIDATOR_WHITELIST
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            schema_refs: vec![SchemaRef {
                path: "outputs/artifacts/stage_14/integration_validation_report.json".to_string(),
                schema: "knowledge/schemas/ai_design/integration_validation_report.schema.json"
                    .to_string(),
                description: "Integration validation report.".to_string(),
            }],
            knowledge_refs: vec!["knowledge/Core_Rules.md".to_string()],
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn artifact_registry_roundtrip_preserves_schema_refs_and_whitelists() {
        let registry = ArtifactRegistry {
            version: 1,
            description: "registry".to_string(),
            default_reviewers: REVIEWER_WHITELIST
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            default_validators: VALIDATOR_WHITELIST
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            artifacts: vec![sample_contract()],
        };

        let json = serde_json::to_string(&registry).unwrap();
        assert!(json.contains("schema_refs"));
        let restored: ArtifactRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, registry);
        assert!(restored.artifacts[0].unknown_reviewers().is_empty());
        assert!(restored.artifacts[0].unknown_validators().is_empty());
    }

    #[test]
    fn artifact_contract_detects_unknown_validator_and_duplicate_task() {
        let mut artifact = sample_contract();
        artifact.validators.push("fake_validator".to_string());
        artifact.tasks.push(artifact.tasks[0].clone());
        assert_eq!(artifact.unknown_validators(), vec!["fake_validator"]);
        assert_eq!(
            artifact.duplicate_task_ids(),
            vec!["stage_14.import_integration"]
        );
    }

    #[test]
    fn artifact_review_and_validation_reports_roundtrip() {
        let result = ArtifactCheckResult {
            name: "schema_contract_validator".to_string(),
            status: ArtifactCheckStatus::Pass,
            severity: ArtifactSeverity::Info,
            message: "contract matches schema".to_string(),
        };
        let review = ArtifactReviewReport {
            step: 14,
            timestamp: "2026-07-08T00:00:00".to_string(),
            status: ArtifactReportStatus::Success,
            phase: "review".to_string(),
            reviews: vec![ArtifactReview {
                artifact_id: "stage_14.integration_validation_bundle".to_string(),
                status: ArtifactCheckStatus::Pass,
                results: vec![result.clone()],
            }],
        };
        let validation = ArtifactValidationLayerReport {
            step: 14,
            timestamp: "2026-07-08T00:01:00".to_string(),
            status: ArtifactReportStatus::Success,
            phase: "validation".to_string(),
            validations: vec![ArtifactValidation {
                artifact_id: "stage_14.integration_validation_bundle".to_string(),
                status: ArtifactCheckStatus::Pass,
                results: vec![result],
            }],
        };

        assert_eq!(
            serde_json::from_str::<ArtifactReviewReport>(&serde_json::to_string(&review).unwrap())
                .unwrap(),
            review
        );
        assert_eq!(
            serde_json::from_str::<ArtifactValidationLayerReport>(
                &serde_json::to_string(&validation).unwrap()
            )
            .unwrap(),
            validation
        );
    }

    #[test]
    fn artifact_rejects_invalid_report_status() {
        let invalid = r#"{"step":14,"status":"ok","phase":"validation","validations":[]}"#;
        assert!(serde_json::from_str::<ArtifactValidationLayerReport>(invalid).is_err());
    }
}
