use std::collections::{BTreeMap, BTreeSet};

use adm_new_game_spec::GameSpec;
use serde::{Deserialize, Serialize};

pub const STEP13_V2_COMPILER_VERSION: &str = "game_spec_step13_acceptance.v1";
pub const STEP13_EXECUTION_EVIDENCE_SCHEMA_VERSION: &str = "step13_execution_evidence.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step13Status {
    Passed,
    Failed,
    WaitingManualReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioExecutionStatus {
    Passed,
    Failed,
    ManualReviewRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationKind {
    Automated,
    ManualReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PerformanceCheckResult {
    pub budget_id: String,
    pub limit: u64,
    pub observed: Option<u64>,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessibilityCheckResult {
    pub requirement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed: Option<bool>,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptanceScenarioResult {
    pub scenario_id: String,
    pub summary: String,
    pub automation_kind: AutomationKind,
    pub status: ScenarioExecutionStatus,
    pub action_ids: Vec<String>,
    pub spec_hash: String,
    pub build_hash: String,
    pub log_hash: String,
    #[serde(default)]
    pub performance_checks: Vec<PerformanceCheckResult>,
    #[serde(default)]
    pub accessibility_checks: Vec<AccessibilityCheckResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step13ValidationPolicy {
    #[serde(default)]
    pub execution_evidence: Option<Step13ExecutionEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScenarioExecutionObservation {
    pub executed_action_ids: Vec<String>,
    pub status: ScenarioExecutionStatus,
    pub log_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step13ExecutionEvidence {
    pub schema_version: String,
    pub executor_id: String,
    pub build_hash: String,
    #[serde(default)]
    pub scenario_observations: BTreeMap<String, ScenarioExecutionObservation>,
    #[serde(default)]
    pub completed_manual_reviews: BTreeSet<String>,
    #[serde(default)]
    pub performance_observations: BTreeMap<String, u64>,
    #[serde(default)]
    pub accessibility_observations: BTreeMap<String, bool>,
    #[serde(default)]
    pub disabled_action_ids: BTreeSet<String>,
    #[serde(default)]
    pub missing_asset_ids: BTreeSet<String>,
}

impl Step13ValidationPolicy {
    pub fn from_execution_evidence(evidence: Step13ExecutionEvidence) -> Self {
        Self {
            execution_evidence: Some(evidence),
        }
    }

    pub fn strict_unattended() -> Self {
        Self {
            execution_evidence: None,
        }
    }
}

impl Step13ExecutionEvidence {
    pub fn empty_real_executor(
        build_hash: impl Into<String>,
        executor_id: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: STEP13_EXECUTION_EVIDENCE_SCHEMA_VERSION.to_string(),
            executor_id: executor_id.into(),
            build_hash: build_hash.into(),
            scenario_observations: BTreeMap::new(),
            completed_manual_reviews: BTreeSet::new(),
            performance_observations: BTreeMap::new(),
            accessibility_observations: BTreeMap::new(),
            disabled_action_ids: BTreeSet::new(),
            missing_asset_ids: BTreeSet::new(),
        }
    }

    pub fn test_only_nominal_for_spec(spec: &GameSpec, build_hash: impl Into<String>) -> Self {
        let build_hash = build_hash.into();
        Self {
            schema_version: STEP13_EXECUTION_EVIDENCE_SCHEMA_VERSION.to_string(),
            executor_id: "test_only_nominal_step13_fixture".to_string(),
            build_hash: build_hash.clone(),
            scenario_observations: spec
                .acceptance_scenarios
                .iter()
                .filter(|(_, scenario)| !scenario.manual_review_required)
                .map(|(id, scenario)| {
                    (
                        id.to_string(),
                        ScenarioExecutionObservation {
                            executed_action_ids: scenario
                                .when
                                .iter()
                                .map(|action| action.action.to_string())
                                .collect(),
                            status: ScenarioExecutionStatus::Passed,
                            log_hash: adm_new_foundation::sha256_hex(
                                format!("test-only-step13:{build_hash}:{id}").as_bytes(),
                            ),
                            failure_reason: None,
                        },
                    )
                })
                .collect(),
            completed_manual_reviews: spec
                .acceptance_scenarios
                .iter()
                .filter(|(_, scenario)| scenario.manual_review_required)
                .map(|(id, _)| id.to_string())
                .collect(),
            performance_observations: spec
                .technical
                .performance_budgets
                .iter()
                .map(|(id, limit)| (id.to_string(), *limit))
                .collect(),
            accessibility_observations: spec
                .technical
                .accessibility_requirements
                .iter()
                .map(|requirement| (requirement.clone(), true))
                .collect(),
            disabled_action_ids: BTreeSet::new(),
            missing_asset_ids: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step13AcceptanceOutput {
    pub schema_version: String,
    pub compiler_version: String,
    pub status: Step13Status,
    pub spec_hash: String,
    pub build_hash: String,
    pub scenario_results: Vec<AcceptanceScenarioResult>,
    pub output_paths: BTreeMap<String, String>,
}
