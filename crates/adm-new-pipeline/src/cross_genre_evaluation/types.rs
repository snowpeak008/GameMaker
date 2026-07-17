use std::collections::BTreeMap;

use adm_new_game_spec::GameSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum A09EvaluationStatus {
    Passed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum A09ProductionScope {
    R1Reference,
    FullProduction,
    SpecLevelOnly,
    ArchitectureOnly,
}

#[derive(Debug, Clone)]
pub struct A09Sample {
    pub sample_id: String,
    pub display_name: String,
    pub structure_family: String,
    pub production_scope: A09ProductionScope,
    pub spec: GameSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecLevelCompilationResult {
    pub sample_id: String,
    pub display_name: String,
    pub structure_family: String,
    pub production_scope: A09ProductionScope,
    pub step00_06_status: String,
    pub step08_10_status: String,
    pub semantic_hash: Option<String>,
    pub architecture_hash: Option<String>,
    pub task_count: usize,
    pub asset_count: usize,
    pub scenario_count: usize,
    pub anti_overfit_passed: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FullProductionResult {
    pub sample_id: String,
    pub display_name: String,
    pub status: String,
    pub manual_signature_required: bool,
    pub step11_status: String,
    pub step12_status: String,
    pub step13_status: String,
    pub step14_status: String,
    pub task_count: usize,
    pub asset_count: usize,
    pub scenario_count: usize,
    pub weighted_complexity: u64,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThirdLayerAntiOverfitReport {
    pub label_permutation_passed: bool,
    pub capability_mutation_passed: bool,
    pub mutation_rejection_count: usize,
    pub mutation_rejections_required: usize,
    pub repeated_run_count_per_sample: usize,
    pub repeated_runs_stable: bool,
    pub no_ai_mode_supported: bool,
    pub bounded_ai_repeat_count: usize,
    pub bounded_ai_repeat_stable: bool,
    pub fault_injection_blocked: bool,
    pub source_scan_passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceScanHit {
    pub file: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceScanReport {
    pub status: String,
    pub scanned_files: Vec<String>,
    pub forbidden_tokens: Vec<String>,
    pub hits: Vec<SourceScanHit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvelopeMeasurement {
    pub sample_id: String,
    pub product_envelope: String,
    pub task_count: usize,
    pub asset_count: usize,
    pub scenario_count: usize,
    pub system_count: usize,
    pub weighted_complexity: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WeightedBudgetThreshold {
    pub envelope_label: String,
    pub inclusive_max_weight: u64,
    pub evidence_sample_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvelopeCalibrationReport {
    pub status: String,
    pub measurements: Vec<EnvelopeMeasurement>,
    pub suggested_thresholds: Vec<WeightedBudgetThreshold>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FieldPromotionChecklist {
    pub status: String,
    pub new_core_fields: Vec<String>,
    pub extension_namespaces_used: Vec<String>,
    pub decision: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct A09EvaluationReport {
    pub schema_version: String,
    pub compiler_version: String,
    pub status: A09EvaluationStatus,
    pub spec_level_results: Vec<SpecLevelCompilationResult>,
    pub full_production_results: Vec<FullProductionResult>,
    pub third_layer_anti_overfit: ThirdLayerAntiOverfitReport,
    pub source_scan: SourceScanReport,
    pub envelope_calibration: EnvelopeCalibrationReport,
    pub field_promotion: FieldPromotionChecklist,
    pub output_paths: BTreeMap<String, String>,
}
