use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use adm_new_contracts::ArtifactLocale;
use adm_new_foundation::io::{now_iso, read_json, write_json, write_text};
use adm_new_foundation::{AdmError, AdmResult, sha256_hex};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::generation::{
    ParsedDesignSource, StageOutputGenerator, artifact_locale_from_inputs, localized_text,
    output_base_from_stage_dir,
};
use crate::source::SourceGroup;
use crate::stages::step00_02::StagePluginSpec;
use crate::work_units::{
    OfflineVerifiedWorkUnitExecutor, SafeUnitJournal, WorkUnitExecutor, WorkUnitKind,
    WorkUnitRequest, WorkUnitRunOutcome, WorkUnitRunStatus, WorkUnitStopToken,
    execute_work_unit_batch,
};

pub const STEP08: u32 = 8;
pub const STEP09: u32 = 9;
pub const STEP10: u32 = 10;
pub const STEP11: u32 = 11;
pub const STEP12: u32 = 12;
pub const STEP13: u32 = 13;
pub const STEP14: u32 = 14;

pub const SCENE_ASSEMBLY_REQUIRED_STAGE8_ARTIFACTS: &[&str] = &[
    "program_plan_contract.json",
    "scene_assembly_task_requirements.json",
    "ui_runtime_task_requirements.json",
    "input_runtime_task_requirements.json",
    "objective_runtime_task_requirements.json",
];

pub fn step08_plugin_spec() -> StagePluginSpec {
    plugin_spec("08", "program_plans", "devflow_Plans_*", "Plans")
}

pub fn step09_plugin_spec() -> StagePluginSpec {
    plugin_spec("09", "art_plans", "devflow_ArtPlans_*", "ArtPlans")
}

pub fn step10_plugin_spec() -> StagePluginSpec {
    plugin_spec("10", "asset_alignment", "devflow_Alignment_*", "Alignment")
}

pub fn step11_plugin_spec() -> StagePluginSpec {
    plugin_spec(
        "11",
        "dev_execution",
        "devflow_DevExecution_*",
        "DevExecution",
    )
}

pub fn step12_plugin_spec() -> StagePluginSpec {
    plugin_spec(
        "12",
        "art_production",
        "devflow_ArtProduction_*",
        "ArtProduction",
    )
}

pub fn step13_plugin_spec() -> StagePluginSpec {
    plugin_spec(
        "13",
        "scene_assembly",
        "devflow_SceneAssembly_*",
        "SceneAssembly",
    )
}

pub fn step14_plugin_spec() -> StagePluginSpec {
    plugin_spec(
        "14",
        "integration_validation",
        "devflow_Integration_*",
        "Integration",
    )
}

fn plugin_spec(
    stage_id: &'static str,
    label: &str,
    pattern: &str,
    source_id: &str,
) -> StagePluginSpec {
    StagePluginSpec {
        stage_id,
        source_groups: vec![SourceGroup {
            label: label.to_string(),
            patterns: vec![pattern.to_string()],
            mode: "latest".to_string(),
            required: false,
            source_ids: vec![source_id.to_string()],
        }],
        test_mode_status: "success",
        generation_entrypoint: "apply_development_plan_outputs",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramTask {
    pub task_id: String,
    pub requirement_id: String,
    pub title: String,
    pub phase: String,
    pub category: String,
    pub priority: String,
    pub target_path: String,
    pub output_files: Vec<String>,
    pub allowed_write_paths: Vec<String>,
    pub verification_commands: Vec<String>,
    pub source_refs: Vec<String>,
    pub acceptance: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtTask {
    pub task_id: String,
    pub asset_id: String,
    pub title: String,
    pub asset_type: String,
    pub category: String,
    pub priority: String,
    pub complexity: String,
    pub unity_target_path: String,
    pub dimensions: Value,
    pub consumer_system: String,
    pub mount_point: String,
    pub acceptance: String,
    pub source_refs: Vec<String>,
    pub generation_prompt: String,
    pub negative_prompt: String,
    pub production_tier: String,
    pub production_execution_strategy: String,
    pub semantic_policy: Value,
    pub rework_policy: Value,
}

pub fn completed_with_review_blocker(
    pipeline_state: &Value,
    continue_after_completed_with_review: bool,
) -> Option<Value> {
    let steps = pipeline_state.get("steps").and_then(Value::as_object)?;
    for step_num in [STEP11, STEP12, STEP13] {
        let status = steps
            .get(&step_num.to_string())
            .and_then(|step| step.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if !["completed_with_review", "blocked", "failed"].contains(&status) {
            continue;
        }
        if continue_after_completed_with_review {
            continue;
        }
        let message = format!("步骤 {step_num:02} 尚未满足集成验证条件。");
        return Some(json!({
            "code": "STEP14_UPSTREAM_STAGE_NOT_READY",
            "status": "blocked",
            "stage_id": "14",
            "blocked_step": step_num,
            "message": message,
            "return_target": format!("stage_{step_num:02}"),
        }));
    }
    None
}

pub fn apply_step13_standalone_metadata(result: &mut Value, metadata: &Value) {
    let mode = string_field(metadata, "standalone_mode");
    if mode.is_empty() {
        return;
    }
    let object = ensure_object(result);
    object.insert("validation_scope".to_string(), json!("standalone"));
    object.insert("standalone_mode".to_string(), json!(mode));
    object.insert(
        "artifacts_source_version".to_string(),
        json!(string_field(metadata, "artifacts_source_version")),
    );
}

pub fn apply_step14_standalone_metadata(result: &mut Value, metadata: &Value) {
    let mode = string_field(metadata, "standalone_mode");
    if mode.is_empty() {
        return;
    }
    let object = ensure_object(result);
    object.insert("validation_scope".to_string(), json!("standalone"));
    object.insert("standalone_mode".to_string(), json!(mode));
    object.insert(
        "artifacts_source_version".to_string(),
        json!(string_field(metadata, "artifacts_source_version")),
    );
    let blocked = object
        .get("status")
        .and_then(Value::as_str)
        .map(|status| status == "blocked")
        .unwrap_or(false);
    let has_blocking_issues = protocol_issue_collection_is_non_empty(
        object
            .get("blocking_issues")
            .or_else(|| object.get("blockers")),
    );
    if mode == "standalone_partial" && blocked && !has_blocking_issues {
        let warnings = object
            .get("blocking_issues")
            .cloned()
            .unwrap_or_else(|| json!([]));
        object.insert("standalone_warnings".to_string(), warnings);
        object.insert("status".to_string(), json!("completed_with_review"));
    }
}

#[derive(Debug, Clone, Default)]
pub struct Step08OutputGenerator;

impl StageOutputGenerator for Step08OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP08)?;
        stage08_outputs(parsed, out_dir, structured_inputs)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Step09OutputGenerator;

impl StageOutputGenerator for Step09OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP09)?;
        stage09_outputs(parsed, out_dir, structured_inputs)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Step10OutputGenerator;

impl StageOutputGenerator for Step10OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP10)?;
        stage10_outputs(parsed, out_dir, structured_inputs)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Step11OutputGenerator {
    executor: Option<Arc<dyn WorkUnitExecutor>>,
    journal_root: Option<PathBuf>,
    stop_token: WorkUnitStopToken,
}

impl Step11OutputGenerator {
    pub fn new(
        executor: Option<Arc<dyn WorkUnitExecutor>>,
        journal_root: Option<PathBuf>,
        stop_token: WorkUnitStopToken,
    ) -> Self {
        Self {
            executor,
            journal_root,
            stop_token,
        }
    }

    /// Explicit deterministic mode for tests and offline previews.
    pub fn offline() -> Self {
        Self::new(
            Some(Arc::new(OfflineVerifiedWorkUnitExecutor)),
            None,
            WorkUnitStopToken::default(),
        )
    }
}

impl StageOutputGenerator for Step11OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP11)?;
        stage11_outputs(
            parsed,
            out_dir,
            structured_inputs,
            self.executor.as_deref(),
            &SafeUnitJournal::new(work_journal_root(
                out_dir,
                self.journal_root.as_deref(),
                STEP11,
            )),
            &self.stop_token,
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct Step12OutputGenerator {
    executor: Option<Arc<dyn WorkUnitExecutor>>,
    journal_root: Option<PathBuf>,
    stop_token: WorkUnitStopToken,
}

impl Step12OutputGenerator {
    pub fn new(
        executor: Option<Arc<dyn WorkUnitExecutor>>,
        journal_root: Option<PathBuf>,
        stop_token: WorkUnitStopToken,
    ) -> Self {
        Self {
            executor,
            journal_root,
            stop_token,
        }
    }

    /// Explicit deterministic mode for tests and offline previews.
    pub fn offline() -> Self {
        Self::new(
            Some(Arc::new(OfflineVerifiedWorkUnitExecutor)),
            None,
            WorkUnitStopToken::default(),
        )
    }
}

impl StageOutputGenerator for Step12OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP12)?;
        stage12_outputs(
            parsed,
            out_dir,
            structured_inputs,
            self.executor.as_deref(),
            &SafeUnitJournal::new(work_journal_root(
                out_dir,
                self.journal_root.as_deref(),
                STEP12,
            )),
            &self.stop_token,
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct Step13OutputGenerator;

impl StageOutputGenerator for Step13OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP13)?;
        let mut result = stage13_outputs(parsed, out_dir, structured_inputs)?;
        if let Some(metadata) = structured_inputs.get("metadata") {
            apply_step13_standalone_metadata(&mut result, metadata);
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Step14OutputGenerator;

impl StageOutputGenerator for Step14OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP14)?;
        let mut result = stage14_outputs(parsed, out_dir, structured_inputs)?;
        if let Some(metadata) = structured_inputs.get("metadata") {
            apply_step14_standalone_metadata(&mut result, metadata);
        }
        Ok(result)
    }
}

pub fn generator_for_step(step_number: u32) -> AdmResult<Box<dyn StageOutputGenerator>> {
    match step_number {
        STEP08 => Ok(Box::new(Step08OutputGenerator)),
        STEP09 => Ok(Box::new(Step09OutputGenerator)),
        STEP10 => Ok(Box::new(Step10OutputGenerator)),
        STEP11 => Ok(Box::new(Step11OutputGenerator::default())),
        STEP12 => Ok(Box::new(Step12OutputGenerator::default())),
        STEP13 => Ok(Box::new(Step13OutputGenerator)),
        STEP14 => Ok(Box::new(Step14OutputGenerator)),
        other => Err(AdmError::new(format!(
            "Step08-14 generator cannot handle stage {other:02}"
        ))),
    }
}

fn stage08_outputs(
    parsed: &ParsedDesignSource,
    out_dir: &Path,
    structured_inputs: &Value,
) -> AdmResult<Value> {
    let locale = artifact_locale_from_inputs(structured_inputs);
    let program_contract = read_stage_json(out_dir, 3, "program_requirements_contract.json");
    let structure_spec = read_stage_json(out_dir, 3, "program_structure_spec.json");
    let capability_contract = read_stage_json(out_dir, 3, "program_capability_contract.json");
    let semantic_coverage = read_stage_json(out_dir, 3, "program_semantic_coverage_report.json");
    let program_review = read_stage_json(out_dir, 5, "program_ai_review_report.json");
    let semantic_review = read_stage_json(out_dir, 5, "program_semantic_review_report.json");
    let contracts = playable_contracts(out_dir);
    let mut blockers = Vec::new();
    for contract_id in [
        "core_playable_contract",
        "ui_flow_contract",
        "playable_acceptance_contract",
    ] {
        if is_empty_object(contracts.get(contract_id).unwrap_or(&Value::Null)) {
            blockers.push(json!({
                "code": "STEP08_PLAYABLE_CONTRACT_MISSING",
                "contract_id": contract_id,
                "message": localized_owned(
                    locale,
                    format!("步骤08需要步骤02的 `{contract_id}`。"),
                    format!("Step08 requires Step02 `{contract_id}`."),
                ),
                "return_target": "stage_02",
            }));
        }
    }
    if is_empty_object(&program_contract) {
        blockers.push(json!({
            "code": "STEP08_PROGRAM_CONTRACT_MISSING",
            "message": localized_text(locale, "步骤08需要步骤03的 program_requirements_contract.json。", "Step08 requires Step03 program_requirements_contract.json."),
            "return_target": "stage_03",
        }));
    }
    if is_empty_object(&capability_contract) {
        blockers.push(json!({
            "code": "STEP08_PROGRAM_CAPABILITY_CONTRACT_MISSING",
            "message": localized_text(locale, "步骤08需要步骤03的程序能力契约。", "Step08 requires the Step03 program capability contract."),
            "return_target": "stage_03",
            "artifact": "stage_03/program_capability_contract.json",
        }));
    }
    if is_empty_object(&semantic_coverage) {
        blockers.push(json!({
            "code": "STEP08_PROGRAM_SEMANTIC_COVERAGE_MISSING",
            "message": localized_text(locale, "步骤08需要步骤03的程序语义覆盖报告。", "Step08 requires the Step03 program semantic coverage report."),
            "return_target": "stage_03",
            "artifact": "stage_03/program_semantic_coverage_report.json",
        }));
    } else if semantic_coverage.get("status").and_then(Value::as_str) == Some("blocked")
        || !blockers_is_empty(&semantic_coverage)
    {
        blockers.push(json!({
            "code": "STEP08_PROGRAM_SEMANTIC_COVERAGE_BLOCKED",
            "message": localized_text(locale, "步骤03的程序语义覆盖仍有阻断项，步骤08不能生成执行计划。", "Step08 cannot create an execution plan while Step03 program semantic coverage is blocked."),
            "return_target": "stage_03",
            "artifact": "stage_03/program_semantic_coverage_report.json",
        }));
    }
    if review_is_blocked(&program_review) {
        blockers.push(json!({
            "code": "STEP08_PROGRAM_REVIEW_BLOCKED",
            "message": localized_text(locale, "步骤03的程序评审处于阻断状态，步骤08无法生成开发计划。", "Step08 cannot plan development from blocked program review."),
            "return_target": "stage_05",
        }));
    }
    if is_empty_object(&semantic_review) || review_is_blocked(&semantic_review) {
        blockers.push(json!({
            "code": "STEP08_PROGRAM_SEMANTIC_REVIEW_BLOCKED",
            "message": localized_text(locale, "步骤05的程序语义复核未通过，步骤08无法继续。", "Step08 cannot continue until the Step05 program semantic review passes."),
            "return_target": "stage_05",
            "artifact": "stage_05/program_semantic_review_report.json",
        }));
    }
    let tasks = program_tasks_from_contract(&program_contract, &structure_spec, parsed, locale);
    let synthesis = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "status": if blockers.is_empty() { "passed" } else { "blocked" },
        "blockers": blockers,
        "warnings": [],
        "source_refs": [
            "stage_02/playable_contracts/core_playable_contract.json",
            "stage_03/program_requirements_contract.json",
            "stage_03/program_capability_contract.json",
            "stage_03/program_semantic_coverage_report.json",
            "stage_05/program_semantic_review_report.json",
        ],
        "task_reviews": tasks.iter().map(|task| json!({
            "task_id": task.task_id,
            "status": "passed",
            "source_refs": task.source_refs,
        })).collect::<Vec<_>>(),
        "summary": { "task_count": tasks.len() },
        "task_count": tasks.len(),
        "artifact_locale": locale,
    });
    let allowed_roots = string_array(structure_spec.get("allowed_roots"));
    let breakdown = json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "tasks": tasks,
        "dependencies": [],
        "parallel_groups": [{
            "group_id": "PG-001-core",
            "task_ids": task_ids(&tasks),
            "execution": if tasks.len() > 1 { "parallel_allowed" } else { "serial" },
        }],
        "allowed_roots": if allowed_roots.is_empty() { json!(["Assets/Scripts/"]) } else { json!(allowed_roots) },
        "artifact_locale": locale,
    });
    let plan_contract = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "plan_id": "stage_08_program_plan",
        "source_contracts": [
            "stage_02/playable_contracts/core_playable_contract.json",
            "stage_02/playable_contracts/ui_flow_contract.json",
            "stage_03/program_requirements_contract.json",
            "stage_03/program_capability_contract.json",
            "stage_03/program_semantic_coverage_report.json",
            "stage_05/program_semantic_review_report.json",
        ],
        "source_project_dna": "stage_02/project_dna_contract.json",
        "project_signature": string_field(&capability_contract, "project_signature"),
        "semantic_coverage_refs": [
            "stage_03/program_semantic_coverage_report.json",
            "stage_05/program_semantic_review_report.json",
        ],
        "customization_targets": value_array(capability_contract.get("capabilities")).into_iter().map(|capability| json!({
            "capability_id": string_field(&capability, "capability_id"),
            "program_class": string_field(&capability, "program_class"),
            "source_semantic_id": string_field(&capability, "source_semantic_id"),
        })).collect::<Vec<_>>(),
        "planning_blockers": blockers,
        "tasks": tasks.iter().map(|task| json!({
            "task_id": task.task_id,
            "title": task.title,
            "phase": task.phase,
            "source_refs": task.source_refs,
            "required_outputs": task.output_files,
        })).collect::<Vec<_>>(),
        "artifact_locale": locale,
    });
    let gate = task_contract_gate(&tasks);
    write_json(&out_dir.join("program_task_breakdown.json"), &breakdown)?;
    write_json(&out_dir.join("program_plan_contract.json"), &plan_contract)?;
    write_json(&out_dir.join("task_contract_gate_report.json"), &gate)?;
    write_json(&out_dir.join("ai_task_synthesis_report.json"), &synthesis)?;
    write_json(
        &out_dir.join("playable_contract_plan_summary.json"),
        &playable_plan_summary(&contracts, &tasks, locale),
    )?;
    write_json(
        &out_dir.join("scene_assembly_task_requirements.json"),
        &scene_assembly_requirements(&contracts, &tasks, locale),
    )?;
    write_json(
        &out_dir.join("ui_runtime_task_requirements.json"),
        &ui_runtime_requirements(&contracts, &tasks, locale),
    )?;
    write_json(
        &out_dir.join("input_runtime_task_requirements.json"),
        &input_runtime_requirements(&contracts, locale),
    )?;
    write_json(
        &out_dir.join("objective_runtime_task_requirements.json"),
        &objective_runtime_requirements(&contracts, &tasks, locale),
    )?;
    write_json(
        &out_dir.join("program_semantic_coverage_matrix.json"),
        &program_semantic_matrix(
            &semantic_coverage,
            &tasks,
            blockers_is_empty(&synthesis),
            locale,
        ),
    )?;
    write_json(
        &out_dir.join("customization_score_report.json"),
        &score_report("08", &synthesis, locale),
    )?;
    write_json(
        &out_dir.join("config_schema.json"),
        &json!({"required_task_fields": ["task_id", "title", "category", "priority", "output_files", "allowed_write_paths"]}),
    )?;
    write_text(
        &out_dir.join("TEMPLATE_NOTE.md"),
        localized_text(
            locale,
            "# 模板处理说明\n\n在任务规划前，已规范化由模板产生的措辞。\n",
            "# Template Note\n\nTemplate-derived wording was normalized before task planning.\n",
        ),
    )?;
    Ok(json!({
        "status": if blockers_is_empty(&synthesis) { "success" } else { "blocked" },
        "artifact_locale": locale,
        "content_exists": true,
        "ai_review_status": if blockers_is_empty(&synthesis) { "passed" } else { "blocked" },
        "traceability_valid": tasks.iter().all(|task| !task.source_refs.is_empty()),
        "task_count": tasks.len(),
        "blocking_issues": synthesis.get("blockers").cloned().unwrap_or_else(|| json!([])),
        "program_plan_contract": "program_plan_contract.json",
    }))
}

fn stage09_outputs(
    _parsed: &ParsedDesignSource,
    out_dir: &Path,
    structured_inputs: &Value,
) -> AdmResult<Value> {
    let locale = artifact_locale_from_inputs(structured_inputs);
    let style_contract = read_stage_json(out_dir, 7, "style_application_contract.json");
    let style_confirmation = read_stage_json(out_dir, 7, "style_confirmation.json");
    let asset_registry = read_stage_json(out_dir, 4, "asset_registry.json");
    let mut blockers = Vec::new();
    if style_contract.get("status").and_then(Value::as_str) != Some("approved") {
        blockers.push(json!({
            "code": "STEP09_STYLE_APPLICATION_CONTRACT_MISSING",
            "message": localized_text(locale, "步骤09需要步骤07已批准的 style_application_contract.json。", "Step09 requires Step07 approved style_application_contract.json."),
            "return_target": "stage_07",
        }));
    }
    if style_confirmation.get("status").and_then(Value::as_str) != Some("approved") {
        blockers.push(json!({
            "code": "STEP09_STYLE_CONFIRMATION_MISSING",
            "message": localized_text(locale, "步骤09需要步骤07已批准的 style_confirmation.json。", "Step09 requires an approved Step07 style_confirmation.json."),
            "return_target": "stage_07",
        }));
    }
    let assets = asset_registry
        .as_array()
        .cloned()
        .or_else(|| {
            asset_registry
                .get("assets")
                .and_then(Value::as_array)
                .cloned()
        })
        .unwrap_or_default();
    if assets.is_empty() {
        blockers.push(json!({
            "code": "STEP09_ASSET_REGISTRY_EMPTY",
            "artifact": "stage_04/asset_registry.json",
            "message": localized_text(locale, "步骤09需要步骤04至少提供一个资源登记项。", "Step09 requires at least one Step04 asset registry entry."),
            "return_target": "stage_04",
        }));
    }
    let tasks = art_tasks_from_registry(assets.clone(), &style_contract, locale);
    if !assets.is_empty() && tasks.is_empty() {
        blockers.push(json!({
            "code": "STEP09_ART_TASK_SYNTHESIS_EMPTY",
            "artifact": "stage_04/asset_registry.json",
            "message": localized_text(locale, "步骤04的资源登记表不为空，但步骤09未能生成美术任务。", "Step09 could not synthesize art tasks from the non-empty Step04 registry."),
            "return_target": "stage_04",
        }));
    }
    let breakdown = json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "tasks": tasks,
        "artifact_locale": locale,
    });
    let contract = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "selected_style_id": string_field(&style_contract, "selected_style_id"),
        "style_application_contract": "stage_07/style_application_contract.json",
        "source_refs": [
            "stage_04/asset_registry.json",
            "stage_07/style_application_contract.json",
            "stage_07/style_confirmation.json",
        ],
        "tasks": tasks,
        "blockers": blockers,
        "artifact_locale": locale,
    });
    write_json(&out_dir.join("art_task_breakdown.json"), &breakdown)?;
    write_json(
        &out_dir.join("art_production_task_contract.json"),
        &contract,
    )?;
    write_json(
        &out_dir.join("art_semantic_coverage_matrix.json"),
        &semantic_matrix("art", &tasks, blockers.is_empty(), locale),
    )?;
    write_json(
        &out_dir.join("customization_score_report.json"),
        &score_report("09", &contract, locale),
    )?;
    write_text(
        &out_dir.join("TEMPLATE_NOTE.md"),
        localized_text(
            locale,
            "# 模板处理说明\n\n已清理美术任务标题中来自源模板的免责声明。\n",
            "# Template Note\n\nArt task titles were cleaned from source template disclaimers.\n",
        ),
    )?;
    Ok(json!({
        "status": if blockers.is_empty() { "success" } else { "blocked" },
        "artifact_locale": locale,
        "content_exists": true,
        "ai_review_status": if blockers.is_empty() { "passed" } else { "blocked" },
        "traceability_valid": tasks.iter().all(|task| !task.source_refs.is_empty()),
        "asset_count": assets.len(),
        "task_count": tasks.len(),
        "blocking_issues": blockers,
        "art_production_task_contract": "art_production_task_contract.json",
    }))
}

fn stage10_outputs(
    _parsed: &ParsedDesignSource,
    out_dir: &Path,
    structured_inputs: &Value,
) -> AdmResult<Value> {
    let locale = artifact_locale_from_inputs(structured_inputs);
    let stage8_blockers = missing_stage8_artifacts(out_dir, STEP10, locale);
    let program_tasks = stage8_program_tasks(out_dir);
    let art_contract = read_stage_json(out_dir, 9, "art_production_task_contract.json");
    let art_tasks = value_array(art_contract.get("tasks"));
    let mount_plan = read_stage_json(out_dir, 4, "unity_asset_mount_plan.json");
    let resolved_assets = read_stage_json(out_dir, 4, "asset_requirements_resolved.json");
    let asset_registry = read_stage_json(out_dir, 4, "asset_registry.json");
    let mut blockers = stage8_blockers;
    if program_tasks.is_empty() {
        blockers.push(json!({
            "code": "STEP10_PROGRAM_TASKS_MISSING",
            "artifact": "stage_08/program_task_breakdown.json",
            "message": localized_text(locale, "步骤10需要步骤08至少提供一个真实的程序任务，不能使用虚构任务代替。", "Step10 requires at least one real Step08 program task; a fabricated task cannot be substituted."),
            "return_target": "stage_08",
        }));
    }
    if art_tasks.is_empty() {
        blockers.push(json!({
            "code": "STEP10_ART_PLAN_MISSING",
            "artifact": "stage_09/art_production_task_contract.json",
            "message": localized_text(locale, "步骤10需要步骤09的美术生产任务合同。", "Step10 requires Step09 art production task contract."),
            "return_target": "stage_09",
        }));
    }
    let mut alignment_items = Vec::new();
    let mut mount_items = Vec::new();
    for (index, task) in art_tasks.iter().enumerate() {
        let asset_id = string_field(task, "asset_id");
        let art_task_id = string_field(task, "task_id");
        let unity_target_path = string_field(task, "unity_target_path");
        let consumer_system = string_field(task, "consumer_system");
        let mount_point = string_field(task, "mount_point");
        let art_source_refs = string_array(task.get("source_refs"));
        let explicit_program_task_id = string_field(task, "program_task_id");
        let program_task = if explicit_program_task_id.is_empty() {
            (!program_tasks.is_empty())
                .then(|| &program_tasks[index % program_tasks.len()])
                .filter(|task| !string_field(task, "task_id").is_empty())
        } else {
            program_tasks
                .iter()
                .find(|candidate| string_field(candidate, "task_id") == explicit_program_task_id)
        };
        let program_task_id = program_task
            .map(|task| string_field(task, "task_id"))
            .unwrap_or_default();
        let mut item_blocker_codes = Vec::new();

        if program_task_id.is_empty() {
            item_blocker_codes.push("STEP10_PROGRAM_TASK_MAPPING_MISSING");
            blockers.push(step10_task_gap(
                locale,
                "STEP10_PROGRAM_TASK_MAPPING_MISSING",
                "program_task_id",
                index,
                &asset_id,
                &art_task_id,
                "stage_08/program_task_breakdown.json",
                "stage_08",
                "美术任务缺少可验证的程序任务映射。",
                "The art task has no verifiable program-task mapping.",
            ));
        }
        for (code, field, missing, zh_cn, en_us) in [
            (
                "STEP10_ASSET_ID_MISSING",
                "asset_id",
                asset_id.is_empty(),
                "美术任务缺少资源标识 asset_id。",
                "The art task is missing asset_id.",
            ),
            (
                "STEP10_ART_TASK_ID_MISSING",
                "art_task_id",
                art_task_id.is_empty(),
                "美术任务缺少任务标识 task_id。",
                "The art task is missing task_id.",
            ),
            (
                "STEP10_UNITY_TARGET_PATH_MISSING",
                "unity_target_path",
                unity_target_path.is_empty(),
                "美术任务缺少 Unity 目标路径。",
                "The art task is missing its Unity target path.",
            ),
            (
                "STEP10_CONSUMER_SYSTEM_MISSING",
                "consumer_system",
                consumer_system.is_empty(),
                "美术任务缺少消费该资源的系统。",
                "The art task is missing its consumer system.",
            ),
            (
                "STEP10_MOUNT_POINT_MISSING",
                "mount_point",
                mount_point.is_empty(),
                "美术任务缺少资源挂载点。",
                "The art task is missing its mount point.",
            ),
            (
                "STEP10_SOURCE_REFS_MISSING",
                "source_refs",
                art_source_refs.is_empty(),
                "美术任务缺少上游来源引用。",
                "The art task is missing upstream source references.",
            ),
        ] {
            if missing {
                item_blocker_codes.push(code);
                blockers.push(step10_task_gap(
                    locale,
                    code,
                    field,
                    index,
                    &asset_id,
                    &art_task_id,
                    "stage_09/art_production_task_contract.json",
                    "stage_09",
                    zh_cn,
                    en_us,
                ));
            }
        }

        let (fallback_policy, fallback_policy_source) = step10_fallback_policy(
            task,
            &asset_id,
            &mount_plan,
            &resolved_assets,
            &asset_registry,
        );
        let merged_refs = program_task
            .map(|program_task| merged_source_refs(task, program_task))
            .unwrap_or_else(|| art_source_refs.clone());
        let item_status = if item_blocker_codes.is_empty() {
            "aligned"
        } else {
            "blocked"
        };
        alignment_items.push(json!({
            "program_task_id": program_task_id,
            "asset_id": asset_id,
            "art_task_id": art_task_id,
            "status": item_status,
            "blocker_codes": item_blocker_codes,
            "source_refs": merged_refs,
        }));
        mount_items.push(json!({
            "asset_id": asset_id,
            "task_id": art_task_id,
            "target_path": unity_target_path,
            "unity_target_path": unity_target_path,
            "consumer_system": consumer_system,
            "mount_point": mount_point,
            "fallback_policy": fallback_policy,
            "fallback_policy_source": fallback_policy_source,
            "source_refs": art_source_refs,
            "status": item_status,
        }));
    }
    let gaps = blockers
        .iter()
        .enumerate()
        .map(|(index, gap)| {
            let mut gap = gap.clone();
            let object = ensure_object(&mut gap);
            object
                .entry("gap_id".to_string())
                .or_insert_with(|| json!(format!("ALIGNMENT-GAP-{:03}", index + 1)));
            object
                .entry("severity".to_string())
                .or_insert_with(|| json!("blocking"));
            gap
        })
        .collect::<Vec<_>>();
    let alignment_ready = gaps.is_empty();
    let alignment_status = if alignment_ready {
        "success"
    } else {
        "blocked"
    };
    let report = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "status": alignment_status,
        "alignment_items": alignment_items,
        "gaps": gaps,
        "artifact_locale": locale,
    });
    let mount_summary = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "status": alignment_status,
        "ready": alignment_ready,
        "mount_items": mount_items,
        "blockers": report["gaps"],
        "artifact_locale": locale,
    });
    write_json(
        &out_dir.join("asset_alignment_matrix.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "status": alignment_status,
            "ready": alignment_ready,
            "items": alignment_items,
            "gaps": report["gaps"],
            "artifact_locale": locale,
        }),
    )?;
    write_json(&out_dir.join("asset_alignment_report.json"), &report)?;
    write_json(
        &out_dir.join("mount_readiness_summary.json"),
        &mount_summary,
    )?;
    write_json(
        &out_dir.join("semantic_alignment_report.json"),
        &json!({
            "schema_version": "1.0",
            "status": alignment_status,
            "alignment_checks": alignment_items,
            "alignment_items": alignment_items,
            "blockers": report["gaps"],
            "warnings": [],
            "coverage_summary": {
                "total": alignment_items.len(),
                "aligned": alignment_items.iter().filter(|item| item["status"] == "aligned").count(),
                "blocked": alignment_items.iter().filter(|item| item["status"] == "blocked").count(),
            },
            "artifact_locale": locale,
        }),
    )?;
    let mut semantic_coverage =
        semantic_matrix("alignment", &alignment_items, alignment_ready, locale);
    semantic_coverage["status"] = json!(if alignment_ready { "passed" } else { "blocked" });
    semantic_coverage["blockers"] = report["gaps"].clone();
    semantic_coverage["uncovered_items"] = if alignment_ready {
        json!([])
    } else {
        report["gaps"].clone()
    };
    write_json(
        &out_dir.join("semantic_coverage_matrix.json"),
        &semantic_coverage,
    )?;
    write_json(
        &out_dir.join("customization_score_report.json"),
        &score_report("10", &report, locale),
    )?;
    Ok(json!({
        "status": alignment_status,
        "artifact_locale": locale,
        "content_exists": true,
        "ai_review_status": if alignment_ready { "passed" } else { "blocked" },
        "blocking_issues": report.get("gaps").cloned().unwrap_or_else(|| json!([])),
        "traceability_valid": alignment_ready,
    }))
}

#[allow(clippy::too_many_arguments)]
fn step10_task_gap(
    locale: ArtifactLocale,
    code: &str,
    field: &str,
    task_index: usize,
    asset_id: &str,
    art_task_id: &str,
    artifact: &str,
    return_target: &str,
    zh_cn: &str,
    en_us: &str,
) -> Value {
    json!({
        "code": code,
        "field": field,
        "task_index": task_index,
        "asset_id": asset_id,
        "art_task_id": art_task_id,
        "artifact": artifact,
        "message": localized_text(locale, zh_cn, en_us),
        "return_target": return_target,
    })
}

fn step10_fallback_policy(
    art_task: &Value,
    asset_id: &str,
    mount_plan: &Value,
    resolved_assets: &Value,
    asset_registry: &Value,
) -> (String, String) {
    let candidates = [
        (
            Some(art_task.clone()),
            "stage_09/art_production_task_contract.json",
        ),
        (
            value_array(mount_plan.get("mount_items"))
                .into_iter()
                .find(|item| string_field(item, "asset_id") == asset_id),
            "stage_04/unity_asset_mount_plan.json",
        ),
        (
            value_array(resolved_assets.get("resolved_assets"))
                .into_iter()
                .find(|item| string_field(item, "asset_id") == asset_id),
            "stage_04/asset_requirements_resolved.json",
        ),
        (
            asset_registry_items(asset_registry)
                .into_iter()
                .find(|item| string_field(item, "asset_id") == asset_id),
            "stage_04/asset_registry.json",
        ),
    ];
    for (candidate, source) in candidates {
        let Some(candidate) = candidate else {
            continue;
        };
        let fallback = non_empty_or(
            string_field(&candidate, "fallback_policy"),
            &string_field(&candidate, "fallback_strategy"),
        );
        if !fallback.is_empty() {
            return (fallback, source.to_string());
        }
    }
    ("none".to_string(), "none".to_string())
}

fn asset_registry_items(asset_registry: &Value) -> Vec<Value> {
    asset_registry
        .as_array()
        .cloned()
        .or_else(|| {
            asset_registry
                .get("assets")
                .and_then(Value::as_array)
                .cloned()
        })
        .unwrap_or_default()
}

fn stage11_outputs(
    _parsed: &ParsedDesignSource,
    out_dir: &Path,
    structured_inputs: &Value,
    executor: Option<&dyn WorkUnitExecutor>,
    journal: &SafeUnitJournal,
    stop_token: &WorkUnitStopToken,
) -> AdmResult<Value> {
    let locale = artifact_locale_from_inputs(structured_inputs);
    let mut blockers = missing_stage8_artifacts(out_dir, STEP11, locale);
    for filename in [
        "asset_alignment_report.json",
        "mount_readiness_summary.json",
    ] {
        if is_empty_object(&read_stage_json(out_dir, 10, filename)) {
            blockers.push(json!({
                "code": "STEP11_REQUIRED_STAGE10_ARTIFACT_MISSING",
                "artifact": format!("stage_10/{filename}"),
                "message": localized_owned(
                    locale,
                    format!("步骤11需要步骤10的 `{filename}`。"),
                    format!("Step11 requires Stage10 `{filename}`."),
                ),
                "return_target": "stage_10",
            }));
        }
    }
    let tasks = stage8_program_tasks(out_dir);
    let requests = if blockers.is_empty() {
        work_requests(
            STEP11,
            WorkUnitKind::Development,
            &tasks,
            &mut blockers,
            locale,
        )?
    } else {
        Vec::new()
    };
    let batch = if blockers.is_empty() {
        execute_work_unit_batch(requests, executor, journal, stop_token)?
    } else {
        Default::default()
    };
    blockers.extend(work_unit_blockers(STEP11, &batch.units, locale));
    blockers.extend(work_unit_execution_object_blockers(
        STEP11,
        &batch.units,
        locale,
    ));
    if batch.stopped {
        blockers.push(json!({
            "code": "STEP11_STOP_REQUESTED",
            "message": localized_text(locale, "步骤11已在工作单元边界安全停止。", "Step11 stopped at a work unit boundary."),
            "return_target": "stage_11",
        }));
    }
    if batch.recovery_blocked {
        blockers.push(json!({
            "code": "STEP11_RECOVERY_BLOCKED",
            "message": localized_text(locale, "步骤11包含无法安全协调的副作用，恢复已阻断。", "Step11 contains a side effect that cannot be reconciled safely."),
            "return_target": "stage_11",
        }));
    }
    let records = verified_development_records(&batch.units);
    let preview_records = explicit_offline_preview_records(&batch.units);
    let status = if batch.recovery_blocked {
        "recovery_blocked"
    } else if batch.stopped {
        "stopped"
    } else if blockers.is_empty() {
        "success"
    } else {
        "blocked"
    };
    let report = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "status": status,
        "execution_records": records,
        "records": records,
        "preview_records": preview_records,
        "verified_execution_objects": records.iter().filter_map(|record| {
            if record.get("execution_object_state").and_then(Value::as_str) != Some("verified") {
                return None;
            }
            record.get("execution_object_id").and_then(Value::as_str).filter(|id| !id.is_empty()).map(str::to_string)
        }).collect::<Vec<_>>(),
        "blockers": blockers,
        "parallel_execution": {
            "write_task_parallelized": false,
            "parallel_write_task_count": 0,
            "reason": localized_text(locale, "工作单元通过安全日志串行提交。", "work units commit serially through the safe journal"),
        },
        "artifact_locale": locale,
    });
    write_json(&out_dir.join("dev_execution_report.json"), &report)?;
    write_json(&out_dir.join("devexecution.json"), &report)?;
    write_json(
        &out_dir.join("changed_files_manifest.json"),
        &json!({
            "schema_version": 1,
            "tasks": records.iter().map(|record| json!({
                "task_id": record["task_id"],
                "changed_files": record["changed_files"],
                "unexpected_changes": record["unexpected_changes"],
            })).collect::<Vec<_>>(),
            "changed_files": records.iter().flat_map(|record| string_array(record.get("changed_files"))).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(),
        }),
    )?;
    write_json(
        &out_dir.join("correction_queue.json"),
        &json!({"schema_version": 1, "items": []}),
    )?;
    Ok(json!({
        "status": status,
        "artifact_locale": locale,
        "content_exists": true,
        "ai_review_status": if blockers_is_empty(&report) { "passed" } else { "blocked" },
        "blocking_issues": report.get("blockers").cloned().unwrap_or_else(|| json!([])),
        "traceability_valid": blockers_is_empty(&report),
        "resume_supported": status != "success",
        "recovery_blocked": batch.recovery_blocked,
        "stop_requested": batch.stopped,
    }))
}

fn stage12_outputs(
    _parsed: &ParsedDesignSource,
    out_dir: &Path,
    structured_inputs: &Value,
    executor: Option<&dyn WorkUnitExecutor>,
    journal: &SafeUnitJournal,
    stop_token: &WorkUnitStopToken,
) -> AdmResult<Value> {
    let locale = artifact_locale_from_inputs(structured_inputs);
    let art_contract = read_stage_json(out_dir, 9, "art_production_task_contract.json");
    let mount = read_stage_json(out_dir, 10, "mount_readiness_summary.json");
    let art_tasks = value_array(art_contract.get("tasks"));
    let mount_items = value_array(mount.get("mount_items"));
    let mut blockers = Vec::new();
    if art_tasks.is_empty() {
        blockers.push(json!({
            "code": "STEP12_ART_TASK_CONTRACT_MISSING",
            "artifact": "stage_09/art_production_task_contract.json",
            "message": localized_text(locale, "步骤12需要步骤09的美术生产任务合同。", "Step12 requires the Step09 art production task contract."),
            "return_target": "stage_09",
        }));
    }
    if mount.get("ready").and_then(Value::as_bool) != Some(true) {
        blockers.push(json!({
            "code": "STEP12_MOUNT_READINESS_BLOCKED",
            "artifact": "stage_10/mount_readiness_summary.json",
            "message": localized_text(locale, "步骤12需要步骤10先确认挂载就绪，才能生产可消费资源。", "Step12 requires Stage10 mount readiness before producing consumable assets."),
            "return_target": "stage_10",
        }));
    }
    let requests = if blockers.is_empty() {
        work_requests(STEP12, WorkUnitKind::Art, &art_tasks, &mut blockers, locale)?
    } else {
        Vec::new()
    };
    let batch = if blockers.is_empty() {
        execute_work_unit_batch(requests, executor, journal, stop_token)?
    } else {
        Default::default()
    };
    blockers.extend(work_unit_blockers(STEP12, &batch.units, locale));
    blockers.extend(work_unit_execution_object_blockers(
        STEP12,
        &batch.units,
        locale,
    ));
    if batch.stopped {
        blockers.push(json!({
            "code": "STEP12_STOP_REQUESTED",
            "message": localized_text(locale, "步骤12已在工作单元边界安全停止。", "Step12 stopped at a work unit boundary."),
            "return_target": "stage_12",
        }));
    }
    if batch.recovery_blocked {
        blockers.push(json!({
            "code": "STEP12_RECOVERY_BLOCKED",
            "message": localized_text(locale, "步骤12包含无法安全协调的副作用，恢复已阻断。", "Step12 contains a side effect that cannot be reconciled safely."),
            "return_target": "stage_12",
        }));
    }
    let produced_assets = verified_art_records(&batch.units, &mount_items);
    let preview_assets = explicit_offline_preview_records(&batch.units);
    let missing_assets = missing_art_records(&art_tasks, &produced_assets, &mount_items, locale);
    blockers.extend(missing_assets.iter().map(|missing| {
        json!({
            "code": "STEP12_ASSET_OUTPUT_MISSING",
            "task_id": missing.get("task_id").cloned().unwrap_or(Value::Null),
            "asset_id": missing.get("asset_id").cloned().unwrap_or(Value::Null),
            "message": missing.get("message").cloned().unwrap_or_else(|| json!(localized_text(locale, "美术任务没有可验证的产出。", "An art task has no verifiable output."))),
            "return_target": "stage_12",
        })
    }));
    let status = if batch.recovery_blocked {
        "recovery_blocked"
    } else if batch.stopped {
        "stopped"
    } else if blockers.is_empty() {
        "success"
    } else {
        "blocked"
    };
    let production_report = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "status": status,
        "produced_assets": produced_assets,
        "preview_assets": preview_assets,
        "missing_assets": missing_assets,
        "blockers": blockers,
        "source_refs": ["stage_09/art_production_task_contract.json"],
        "handoff_ready": blockers.is_empty(),
        "artifact_locale": locale,
    });
    write_json(
        &out_dir.join("art_production_report.json"),
        &production_report,
    )?;
    write_stage12_support_files(
        out_dir,
        &produced_assets,
        &mount_items,
        &missing_assets,
        &blockers,
        blockers.is_empty() && missing_assets.is_empty(),
        locale,
    )?;
    Ok(json!({
        "status": status,
        "artifact_locale": locale,
        "content_exists": true,
        "blocking_issues": production_report.get("blockers").cloned().unwrap_or_else(|| json!([])),
        "ai_review_status": if blockers_is_empty(&production_report) { "passed" } else { "blocked" },
        "traceability_valid": blockers_is_empty(&production_report),
        "recovery_blocked": batch.recovery_blocked,
        "stop_requested": batch.stopped,
    }))
}

fn work_journal_root(out_dir: &Path, configured_root: Option<&Path>, step: u32) -> PathBuf {
    let root = configured_root.map(Path::to_path_buf).unwrap_or_else(|| {
        let stage_parent = out_dir.parent().unwrap_or(out_dir);
        if stage_parent.file_name().and_then(|value| value.to_str()) == Some("artifacts") {
            stage_parent
                .parent()
                .unwrap_or(stage_parent)
                .join("checkpoints")
                .join("work_units")
        } else {
            stage_parent.join("checkpoints").join("work_units")
        }
    });
    root.join(format!("stage_{step:02}"))
}

fn work_requests(
    step: u32,
    kind: WorkUnitKind,
    tasks: &[Value],
    blockers: &mut Vec<Value>,
    locale: ArtifactLocale,
) -> AdmResult<Vec<WorkUnitRequest>> {
    let mut seen = BTreeSet::new();
    let mut requests = Vec::new();
    for task in tasks {
        let task_id = string_field(task, "task_id");
        if task_id.trim().is_empty() {
            blockers.push(json!({
                "code": format!("STEP{step:02}_TASK_ID_MISSING"),
                "message": localized_text(locale, "工作任务缺少稳定的 task_id。", "A work task has no stable task_id."),
                "return_target": format!("stage_{step:02}"),
            }));
            continue;
        }
        if !seen.insert(task_id.clone()) {
            blockers.push(json!({
                "code": format!("STEP{step:02}_TASK_ID_DUPLICATED"),
                "task_id": task_id,
                "message": localized_text(locale, "同一步骤内的工作任务 task_id 必须唯一。", "Work task ids must be unique within a stage."),
                "return_target": format!("stage_{step:02}"),
            }));
            continue;
        }
        let mut payload = task.clone();
        let Some(payload_object) = payload.as_object_mut() else {
            blockers.push(json!({
                "code": format!("STEP{step:02}_TASK_PAYLOAD_INVALID"),
                "task_id": task_id,
                "message": localized_text(locale, "工作任务必须是结构化 JSON 对象。", "A work task must be a structured JSON object."),
                "return_target": format!("stage_{step:02}"),
            }));
            continue;
        };
        payload_object.insert("artifact_locale".to_string(), json!(locale));
        requests.push(WorkUnitRequest::new(
            &format!("{step:02}"),
            &task_id,
            kind,
            payload,
        )?);
    }
    Ok(requests)
}

pub fn work_unit_requests_for_stage(out_dir: &Path, step: u32) -> AdmResult<Vec<WorkUnitRequest>> {
    let (kind, tasks, locale) = match step {
        STEP11 => {
            let plan = read_stage_json(out_dir, 8, "program_task_breakdown.json");
            (
                WorkUnitKind::Development,
                value_array(plan.get("tasks")),
                artifact_locale_from_inputs(&plan),
            )
        }
        STEP12 => {
            let contract = read_stage_json(out_dir, 9, "art_production_task_contract.json");
            (
                WorkUnitKind::Art,
                value_array(contract.get("tasks")),
                artifact_locale_from_inputs(&contract),
            )
        }
        other => {
            return Err(AdmError::new(format!(
                "stage {other:02} has no resumable work-unit plan"
            )));
        }
    };
    let mut blockers = Vec::new();
    let requests = work_requests(step, kind, &tasks, &mut blockers, locale)?;
    if !blockers.is_empty() {
        return Err(AdmError::new(format!(
            "stage {step:02} work-unit plan is invalid"
        )));
    }
    Ok(requests)
}

fn is_explicit_offline_preview(outcome: &WorkUnitRunOutcome) -> bool {
    outcome
        .result
        .as_ref()
        .and_then(|result| result.data.get("mode"))
        .and_then(Value::as_str)
        == Some("explicit_offline")
}

fn explicit_offline_preview_records(outcomes: &[WorkUnitRunOutcome]) -> Vec<Value> {
    outcomes
        .iter()
        .filter(|outcome| is_explicit_offline_preview(outcome))
        .map(|outcome| {
            let result = outcome.result.as_ref();
            json!({
                "task_id": outcome.request.task_id,
                "unit_id": outcome.request.unit_id,
                "status": "preview_only",
                "preview_output_refs": result
                    .and_then(|result| result.data.get("preview_output_refs"))
                    .cloned()
                    .unwrap_or_else(|| json!([])),
                "side_effects_performed": false,
                "execution_object_id": "",
                "execution_object_state": "offline_contract_only",
            })
        })
        .collect()
}

fn work_unit_blockers(
    step: u32,
    outcomes: &[WorkUnitRunOutcome],
    locale: ArtifactLocale,
) -> Vec<Value> {
    outcomes
        .iter()
        .filter(|outcome| {
            !matches!(
                outcome.status,
                WorkUnitRunStatus::Committed | WorkUnitRunStatus::Reused
            )
        })
        .map(|outcome| {
            if is_explicit_offline_preview(outcome) {
                return json!({
                    "code": format!("STEP{step:02}_OFFLINE_PREVIEW_ONLY"),
                    "task_id": outcome.request.task_id,
                    "unit_id": outcome.request.unit_id,
                    "message": localized_text(
                        locale,
                        "显式离线执行器只生成合同预览，不会创建或验证外部副作用；本步骤必须保持阻断。",
                        "The explicit offline executor produces a contract preview only and does not create or verify external side effects; this stage must remain blocked.",
                    ),
                    "return_target": format!("stage_{step:02}"),
                });
            }
            let suffix = match outcome.status {
                WorkUnitRunStatus::Unavailable => "EXECUTOR_UNAVAILABLE",
                WorkUnitRunStatus::Stopped => "STOPPED",
                WorkUnitRunStatus::RecoveryBlocked => "RECOVERY_BLOCKED",
                WorkUnitRunStatus::Failed => "EXECUTION_FAILED",
                WorkUnitRunStatus::Committed | WorkUnitRunStatus::Reused => "",
            };
            json!({
                "code": format!("STEP{step:02}_WORK_UNIT_{suffix}"),
                "task_id": outcome.request.task_id,
                "unit_id": outcome.request.unit_id,
                "message": work_unit_status_message(step, outcome.status, locale),
                "return_target": format!("stage_{step:02}"),
            })
        })
        .collect()
}

fn work_unit_execution_object_blockers(
    step: u32,
    outcomes: &[WorkUnitRunOutcome],
    locale: ArtifactLocale,
) -> Vec<Value> {
    outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome.status,
                WorkUnitRunStatus::Committed | WorkUnitRunStatus::Reused
            )
        })
        .filter_map(|outcome| {
            let result = outcome.result.as_ref()?;
            let execution_object_id = result
                .data
                .get("execution_object_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let state = result
                .data
                .get("execution_object_state")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (execution_object_id.is_empty() || state != "verified").then(|| {
                json!({
                    "code": format!("STEP{step:02}_EXECUTION_OBJECT_NOT_VERIFIED"),
                    "task_id": outcome.request.task_id,
                    "unit_id": outcome.request.unit_id,
                    "message": localized_text(
                        locale,
                        "工作单元没有关联到已持久化并验证通过的执行对象。",
                        "The work unit is not linked to a persisted, verified execution object.",
                    ),
                    "return_target": format!("stage_{step:02}"),
                })
            })
        })
        .collect()
}

fn verified_development_records(outcomes: &[WorkUnitRunOutcome]) -> Vec<Value> {
    outcomes
        .iter()
        .filter_map(|outcome| {
            if !matches!(
                outcome.status,
                WorkUnitRunStatus::Committed | WorkUnitRunStatus::Reused
            ) {
                return None;
            }
            let result = outcome.result.as_ref()?;
            if result.data.get("mode").and_then(Value::as_str) == Some("explicit_offline") {
                return None;
            }
            let task = &outcome.request.payload;
            let task_id = &outcome.request.task_id;
            let execution_object_id = result
                .data
                .get("execution_object_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let execution_object_state = result
                .data
                .get("execution_object_state")
                .and_then(Value::as_str)
                .unwrap_or("missing");
            Some(json!({
                "task_id": task_id,
                "unit_id": outcome.request.unit_id,
                "idempotency_key": outcome.request.idempotency_key,
                "status": "success",
                "execution_object_id": execution_object_id,
                "state": execution_object_state,
                "execution_object_state": execution_object_state,
                "changed_files": result.changed_files,
                "output_refs": result.output_refs,
                "unexpected_changes": result.data.get("unexpected_changes").cloned().unwrap_or_else(|| json!([])),
                "verification_results": result.verification_results,
                "source_refs": string_array(task.get("source_refs")),
                "reused": outcome.status == WorkUnitRunStatus::Reused,
            }))
        })
        .collect()
}

fn verified_art_records(outcomes: &[WorkUnitRunOutcome], mount_items: &[Value]) -> Vec<Value> {
    outcomes
        .iter()
        .filter_map(|outcome| {
            if !matches!(
                outcome.status,
                WorkUnitRunStatus::Committed | WorkUnitRunStatus::Reused
            ) {
                return None;
            }
            let result = outcome.result.as_ref()?;
            if result.data.get("mode").and_then(Value::as_str) == Some("explicit_offline") {
                return None;
            }
            let task = &outcome.request.payload;
            let asset_id = string_field(task, "asset_id");
            let execution_object_id = result
                .data
                .get("execution_object_id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let execution_object_state = result
                .data
                .get("execution_object_state")
                .and_then(Value::as_str)
                .unwrap_or("missing");
            let mount_item = mount_items
                .iter()
                .find(|item| string_field(item, "asset_id") == asset_id)
                .cloned()
                .unwrap_or_else(|| json!({}));
            let verified_path = result
                .output_refs
                .first()
                .cloned()
                .unwrap_or_else(|| {
                    non_empty_or(
                        string_field(task, "unity_target_path"),
                        &string_field(&mount_item, "target_path"),
                    )
                });
            Some(json!({
                "asset_id": asset_id,
                "task_id": outcome.request.task_id,
                "unit_id": outcome.request.unit_id,
                "idempotency_key": outcome.request.idempotency_key,
                "path": verified_path,
                "processed_path": format!("Assets/AutoDesign/Art/Processed/{asset_id}.png"),
                "mount_point": non_empty_or(string_field(task, "mount_point"), &string_field(&mount_item, "mount_point")),
                "source_refs": string_array(task.get("source_refs")),
                "status": "verified_produced_asset",
                "execution_object_id": execution_object_id,
                "execution_object_state": execution_object_state,
                "verification_results": result.verification_results,
                "reused": outcome.status == WorkUnitRunStatus::Reused,
            }))
        })
        .collect()
}

fn missing_art_records(
    art_tasks: &[Value],
    produced_assets: &[Value],
    mount_items: &[Value],
    locale: ArtifactLocale,
) -> Vec<Value> {
    let produced_task_ids = produced_assets
        .iter()
        .filter_map(|asset| asset.get("task_id").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    art_tasks
        .iter()
        .filter_map(|task| {
            let task_id = string_field(task, "task_id");
            if task_id.is_empty() || produced_task_ids.contains(task_id.as_str()) {
                return None;
            }
            let asset_id = string_field(task, "asset_id");
            let mount_item = mount_items
                .iter()
                .find(|item| string_field(item, "asset_id") == asset_id);
            let fallback_policy = mount_item
                .map(|item| string_field(item, "fallback_policy"))
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "none".to_string());
            let reason = localized_text(
                locale,
                "美术生产任务没有经过验证的 PNG 产出，不能进入交接。",
                "The art production task has no verified PNG output and cannot be handed off.",
            );
            Some(json!({
                "code": "STEP12_ASSET_OUTPUT_MISSING",
                "task_id": task_id,
                "asset_id": asset_id,
                "reason": reason,
                "fallback_policy": fallback_policy,
                "message": reason,
                "return_target": "stage_12",
            }))
        })
        .collect()
}

fn unity_request_fingerprint(
    parsed: &ParsedDesignSource,
    out_dir: &Path,
    locale: ArtifactLocale,
    unity_project_path: &str,
    unity_editor_path: &str,
) -> AdmResult<String> {
    let contracts = playable_contracts(out_dir);
    let stage08 = SCENE_ASSEMBLY_REQUIRED_STAGE8_ARTIFACTS
        .iter()
        .map(|name| ((*name).to_string(), read_stage_json(out_dir, 8, name)))
        .collect::<BTreeMap<_, _>>();
    let material = json!({
        "protocol": "unity_scene_assembly_request_v1",
        "source": {
            "sha256": parsed.source_sha256,
            "path": parsed.source_path,
            "package": parsed.source_package,
            "input_type": parsed.source_input_type,
        },
        "artifact_locale": locale.as_str(),
        "unity_context": {
            "project_path": request_path_identity(unity_project_path),
            "editor_path": request_path_identity(unity_editor_path),
        },
        "stage_02": contracts,
        "stage_08": stage08,
        "stage_10": {
            "asset_alignment_report": read_stage_json(out_dir, 10, "asset_alignment_report.json"),
            "mount_readiness_summary": read_stage_json(out_dir, 10, "mount_readiness_summary.json"),
            "asset_mount_manifest": read_stage_json(out_dir, 10, "asset_mount_manifest.json"),
        },
        "stage_11": {
            "dev_execution_report": read_stage_json(out_dir, 11, "dev_execution_report.json"),
        },
        "stage_12": {
            "art_production_report": read_stage_json(out_dir, 12, "art_production_report.json"),
            "art_handoff_manifest": read_stage_json(out_dir, 12, "art_handoff_manifest.json"),
            "asset_mount_manifest": read_stage_json(out_dir, 12, "asset_mount_manifest.json"),
        },
    });
    let bytes = serde_json::to_vec(&material)
        .map_err(|error| AdmError::new(format!("serialize Unity request fingerprint: {error}")))?;
    Ok(sha256_hex(&bytes))
}

fn request_path_identity(value: &str) -> String {
    let normalized = normalize_path(value);
    if cfg!(windows) {
        normalized.to_ascii_lowercase()
    } else {
        normalized
    }
}

fn execution_object_store_path(out_dir: &Path) -> PathBuf {
    output_base_from_stage_dir(out_dir).join("outputs/execution_objects/execution_objects.json")
}

fn fingerprint_in_verification_record(record: &Value, expected: &str) -> bool {
    record.get("request_fingerprint").and_then(Value::as_str) == Some(expected)
        || record
            .get("evidence")
            .and_then(|evidence| evidence.get("request_fingerprint"))
            .and_then(Value::as_str)
            == Some(expected)
}

fn scene_execution_object_issues(
    out_dir: &Path,
    scene: &Value,
    expected_fingerprint: &str,
    locale: ArtifactLocale,
    stage_prefix: &str,
) -> Vec<Value> {
    let store_path = execution_object_store_path(out_dir);
    if !store_path.is_file() {
        return vec![json!({
            "code": format!("{stage_prefix}_SCENE_EXECUTION_OBJECT_STORE_MISSING"),
            "source": "outputs/execution_objects/execution_objects.json",
            "message": localized_text(
                locale,
                "找不到执行对象存储，无法核验 Unity 场景组装是否由真实执行对象完成。",
                "The execution-object store is missing, so the Unity scene assembly cannot be tied to a real execution object."
            ),
            "return_target": "stage_13",
        })];
    }
    let store = read_json(&store_path, Value::Null);
    let Some(objects) = store.get("objects").and_then(Value::as_array) else {
        return vec![json!({
            "code": format!("{stage_prefix}_SCENE_EXECUTION_OBJECT_STORE_INVALID"),
            "source": "outputs/execution_objects/execution_objects.json",
            "message": localized_text(
                locale,
                "执行对象存储格式无效，无法核验 Unity 场景组装执行对象。",
                "The execution-object store is invalid and cannot verify the Unity scene-assembly execution object."
            ),
            "return_target": "stage_13",
        })];
    };
    let execution_object_id = string_field(scene, "execution_object_id");
    let Some(object) = objects
        .iter()
        .find(|object| string_field(object, "execution_object_id") == execution_object_id)
    else {
        return vec![json!({
            "code": format!("{stage_prefix}_SCENE_EXECUTION_OBJECT_MISSING"),
            "execution_object_id": execution_object_id,
            "source": "outputs/execution_objects/execution_objects.json",
            "message": localized_text(
                locale,
                "场景报告引用的执行对象不在当前存储中，不能作为成功证据。",
                "The execution object referenced by the scene report is not present in the current store and cannot prove success."
            ),
            "return_target": "stage_13",
        })];
    };

    let mut issues = Vec::new();
    if object.get("object_type").and_then(Value::as_str) != Some("unity_scene_assembly_batch") {
        issues.push(json!({
            "code": format!("{stage_prefix}_SCENE_EXECUTION_OBJECT_TYPE_MISMATCH"),
            "execution_object_id": execution_object_id,
            "expected_object_type": "unity_scene_assembly_batch",
            "actual_object_type": object.get("object_type").cloned().unwrap_or(Value::Null),
            "message": localized_text(
                locale,
                "执行对象类型不是 Unity 场景组装批次，不能复用其成功状态。",
                "The execution object is not a Unity scene-assembly batch, so its success state cannot be reused."
            ),
            "return_target": "stage_13",
        }));
    }
    if object.get("state").and_then(Value::as_str) != Some("verified") {
        issues.push(json!({
            "code": format!("{stage_prefix}_SCENE_EXECUTION_OBJECT_STATE_NOT_VERIFIED"),
            "execution_object_id": execution_object_id,
            "actual_state": object.get("state").cloned().unwrap_or(Value::Null),
            "message": localized_text(
                locale,
                "Unity 场景组装执行对象尚未进入 verified 状态。",
                "The Unity scene-assembly execution object is not in the verified state."
            ),
            "return_target": "stage_13",
        }));
    }
    if object
        .get("metadata")
        .and_then(|metadata| metadata.get("request_fingerprint"))
        .and_then(Value::as_str)
        != Some(expected_fingerprint)
    {
        issues.push(json!({
            "code": format!("{stage_prefix}_SCENE_EXECUTION_OBJECT_FINGERPRINT_MISMATCH"),
            "execution_object_id": execution_object_id,
            "message": localized_text(
                locale,
                "执行对象元数据没有绑定本次 Unity 请求指纹。",
                "The execution-object metadata is not bound to this Unity request fingerprint."
            ),
            "return_target": "stage_13",
        }));
    }
    let verification_matches = object
        .get("verification_records")
        .and_then(Value::as_array)
        .is_some_and(|records| {
            records
                .iter()
                .any(|record| fingerprint_in_verification_record(record, expected_fingerprint))
        });
    if !verification_matches {
        issues.push(json!({
            "code": format!("{stage_prefix}_SCENE_EXECUTION_OBJECT_VERIFICATION_FINGERPRINT_MISMATCH"),
            "execution_object_id": execution_object_id,
            "message": localized_text(
                locale,
                "执行对象的验证记录没有携带本次 Unity 请求指纹。",
                "No execution-object verification record carries this Unity request fingerprint."
            ),
            "return_target": "stage_13",
        }));
    }
    issues
}

fn request_fingerprint_issue(
    document: &Value,
    expected_fingerprint: &str,
    locale: ArtifactLocale,
    stage_prefix: &str,
    subject_zh: &str,
    subject_en: &str,
) -> Option<Value> {
    let actual = string_field(document, "request_fingerprint");
    let (suffix, message) = if actual.is_empty() {
        (
            "UNITY_REQUEST_FINGERPRINT_MISSING",
            localized_owned(
                locale,
                format!("{subject_zh}缺少 request_fingerprint，旧格式证据不能证明本次执行成功。"),
                format!(
                    "{subject_en} is missing request_fingerprint; legacy evidence cannot prove this execution succeeded."
                ),
            ),
        )
    } else if actual != expected_fingerprint {
        (
            "UNITY_REQUEST_FINGERPRINT_MISMATCH",
            localized_owned(
                locale,
                format!("{subject_zh}的 request_fingerprint 与当前请求不一致。"),
                format!(
                    "{subject_en} has a request_fingerprint that does not match the current request."
                ),
            ),
        )
    } else {
        return None;
    };
    Some(json!({
        "code": format!("{stage_prefix}_{suffix}"),
        "message": message,
        "return_target": "stage_13",
    }))
}

fn scene_execution_evidence_verified(
    scene: &Value,
    smoke: &Value,
    build_settings: &Value,
    changed_manifest: &Value,
    scene_manifest: &Value,
) -> bool {
    let scene_path = string_field(scene, "scene_path");
    if scene_path.is_empty()
        || scene.get("status").and_then(Value::as_str) != Some("success")
        || scene.get("unity_attempted").and_then(Value::as_bool) != Some(true)
        || scene
            .get("unity_result")
            .and_then(|result| result.get("status"))
            .and_then(Value::as_str)
            != Some("passed")
        || scene.get("execution_object_state").and_then(Value::as_str) != Some("verified")
        || string_field(scene, "execution_object_id").is_empty()
        || scene
            .get("blocking_issues")
            .and_then(Value::as_array)
            .is_none_or(|issues| !issues.is_empty())
    {
        return false;
    }

    let materialized_files = string_array(scene.get("materialized_files"));
    let project_path = string_field(scene, "project_path");
    let project_files_exist = !project_path.is_empty()
        && Path::new(&project_path).join(&scene_path).is_file()
        && Path::new(&project_path)
            .join("ProjectSettings/EditorBuildSettings.asset")
            .is_file();
    let required_scene_exists = scene
        .get("required_files")
        .and_then(|files| files.get(&scene_path))
        .and_then(Value::as_bool)
        == Some(true);
    let changed_files = string_array(scene.get("changed_files"))
        .into_iter()
        .collect::<BTreeSet<_>>();
    let manifest_changed_files = string_array(changed_manifest.get("changed_files"))
        .into_iter()
        .collect::<BTreeSet<_>>();
    let change_evidence_consistent = changed_manifest
        .get("changed_files")
        .and_then(Value::as_array)
        .is_some()
        && changed_files == manifest_changed_files;
    let scene_manifest_verified = scene_manifest.get("scene_path").and_then(Value::as_str)
        == Some(scene_path.as_str())
        && [
            "canvas_ui_root",
            "event_system",
            "input_router",
            "objective_tracker",
        ]
        .into_iter()
        .all(|field| scene_manifest.get(field).and_then(Value::as_bool) == Some(true));
    let smoke_verified = smoke.get("status").and_then(Value::as_str) == Some("passed")
        && [
            "scene_loads",
            "camera_exists",
            "runtime_root_exists",
            "canvas_ui_root_exists",
            "event_system_exists",
            "input_router_exists",
            "objective_tracker_exists",
            "game_state_exists",
            "visible_content_verified",
        ]
        .into_iter()
        .all(|field| smoke.get(field).and_then(Value::as_bool) == Some(true));
    let scene_report_verified = [
        "demo_scene_exists",
        "build_settings_updated",
        "playmode_smoke_test_passed",
        "visible_content_verified",
    ]
    .into_iter()
    .all(|field| scene.get(field).and_then(Value::as_bool) == Some(true));

    materialized_files.iter().any(|path| path == &scene_path)
        && project_files_exist
        && required_scene_exists
        && change_evidence_consistent
        && scene_manifest_verified
        && smoke_verified
        && build_settings
            .get("build_settings_updated")
            .and_then(Value::as_bool)
            == Some(true)
        && scene_report_verified
}

fn scene_execution_context_matches(
    scene: &Value,
    unity_project_path: &str,
    unity_editor_path: &str,
) -> bool {
    protocol_paths_match(&string_field(scene, "project_path"), unity_project_path)
        && protocol_paths_match(&string_field(scene, "unity_editor_path"), unity_editor_path)
}

fn protocol_paths_match(actual: &str, expected: &str) -> bool {
    if actual.trim().is_empty() || expected.trim().is_empty() {
        return false;
    }
    if let (Ok(actual), Ok(expected)) = (
        std::fs::canonicalize(Path::new(actual)),
        std::fs::canonicalize(Path::new(expected)),
    ) {
        return actual == expected;
    }
    let actual = normalize_path(actual);
    let expected = normalize_path(expected);
    if cfg!(windows) {
        actual.eq_ignore_ascii_case(&expected)
    } else {
        actual == expected
    }
}

fn stage13_outputs(
    parsed: &ParsedDesignSource,
    out_dir: &Path,
    structured_inputs: &Value,
) -> AdmResult<Value> {
    let locale = artifact_locale_from_inputs(structured_inputs);
    let unity_project_path = string_field(structured_inputs, "unity_project_path");
    let unity_editor_path = string_field(structured_inputs, "unity_editor_path");
    let request_fingerprint = unity_request_fingerprint(
        parsed,
        out_dir,
        locale,
        &unity_project_path,
        &unity_editor_path,
    )?;
    // Step13 only prepares a Unity request. A later Unity runner may materialize the
    // scene and write these files; a rerun can then consume that corroborated evidence.
    // In particular, never infer execution success merely from ready upstream inputs.
    let previous_scene = read_json(&out_dir.join("scene_assembly_report.json"), json!({}));
    let previous_smoke = read_json(&out_dir.join("playmode_smoke_test_result.json"), json!({}));
    let previous_build_settings = read_json(
        &out_dir.join("build_settings_update_report.json"),
        json!({}),
    );
    let previous_changed_manifest =
        read_json(&out_dir.join("changed_files_manifest.json"), json!({}));
    let previous_scene_manifest =
        read_json(&out_dir.join("scene_assembly_manifest.json"), json!({}));
    let contracts = playable_contracts(out_dir);
    let mut blockers = Vec::new();
    if unity_project_path.is_empty() {
        blockers.push(json!({
            "code": "STEP13_UNITY_PROJECT_PATH_MISSING",
            "message": localized_text(
                locale,
                "步骤13没有绑定 Unity 项目路径；请先在项目配置中设置有效路径。",
                "Step13 has no bound Unity project path; configure a valid path before running it."
            ),
            "return_target": "environment_configuration",
        }));
    }
    if unity_editor_path.is_empty() {
        blockers.push(json!({
            "code": "STEP13_UNITY_EDITOR_PATH_MISSING",
            "message": localized_text(
                locale,
                "步骤13没有绑定 Unity 编辑器路径；请先在项目配置中设置有效路径。",
                "Step13 has no bound Unity editor path; configure a valid path before running it."
            ),
            "return_target": "environment_configuration",
        }));
    }
    if is_empty_object(contracts.get("ui_flow_contract").unwrap_or(&Value::Null)) {
        blockers.push(json!({
            "code": "STEP13_PLAYABLE_CONTRACT_MISSING",
            "contract_id": "ui_flow_contract",
            "message": localized_text(locale, "步骤13需要步骤02提供 `ui_flow_contract`。", "Step13 requires `ui_flow_contract` from Stage02."),
            "return_target": "stage_02",
        }));
    }
    blockers.extend(missing_stage8_artifacts(out_dir, STEP13, locale));
    let alignment = read_stage_json(out_dir, 10, "asset_alignment_report.json");
    let mount = read_stage_json(out_dir, 10, "mount_readiness_summary.json");
    let dev = read_stage_json(out_dir, 11, "dev_execution_report.json");
    let art = read_stage_json(out_dir, 12, "art_production_report.json");
    let handoff = read_stage_json(out_dir, 12, "art_handoff_manifest.json");
    if is_empty_object(&alignment) {
        blockers.push(json!({
            "code": "STEP13_ALIGNMENT_MISSING",
            "artifact": "stage_10/asset_alignment_report.json",
            "message": localized_text(locale, "步骤13需要步骤10的资源对齐报告。", "Step13 requires the Stage10 asset alignment report."),
            "return_target": "stage_10",
        }));
    }
    if mount.get("ready").and_then(Value::as_bool) != Some(true) {
        blockers.push(json!({
            "code": "STEP13_MOUNT_READINESS_BLOCKED",
            "artifact": "stage_10/mount_readiness_summary.json",
            "message": localized_text(locale, "步骤10尚未确认资源挂载就绪。", "Stage10 has not confirmed asset mount readiness."),
            "return_target": "stage_10",
        }));
    }
    if !stage_report_success(&dev) {
        blockers.push(json!({
            "code": "STEP13_DEV_EXECUTION_NOT_READY",
            "artifact": "stage_11/dev_execution_report.json",
            "message": localized_text(locale, "步骤11的开发执行结果尚未就绪。", "Stage11 development execution is not ready."),
            "return_target": "stage_11",
        }));
    }
    if !stage_report_success(&art) {
        blockers.push(json!({
            "code": "STEP13_ART_PRODUCTION_NOT_READY",
            "artifact": "stage_12/art_production_report.json",
            "message": localized_text(locale, "步骤12的美术生产结果尚未就绪。", "Stage12 art production is not ready."),
            "return_target": "stage_12",
        }));
    }
    if handoff.get("ready_for_step13").and_then(Value::as_bool) != Some(true) {
        blockers.push(json!({
            "code": "STEP13_ART_HANDOFF_NOT_READY",
            "artifact": "stage_12/art_handoff_manifest.json",
            "message": localized_text(locale, "步骤12的美术交接尚未满足步骤13的输入条件。", "The Stage12 art handoff is not ready for Step13."),
            "return_target": "stage_12",
        }));
    }
    let request_ready_for_execute = blockers.is_empty();
    let request_blockers = blockers.clone();
    let evidence_contract_verified = scene_execution_evidence_verified(
        &previous_scene,
        &previous_smoke,
        &previous_build_settings,
        &previous_changed_manifest,
        &previous_scene_manifest,
    );
    let evidence_context_matches =
        scene_execution_context_matches(&previous_scene, &unity_project_path, &unity_editor_path);
    let scene_fingerprint_issue = evidence_contract_verified
        .then(|| {
            request_fingerprint_issue(
                &previous_scene,
                &request_fingerprint,
                locale,
                "STEP13",
                "已有 Unity 场景报告",
                "The existing Unity scene report",
            )
        })
        .flatten();
    let execution_object_issues = if evidence_contract_verified
        && evidence_context_matches
        && scene_fingerprint_issue.is_none()
    {
        scene_execution_object_issues(
            out_dir,
            &previous_scene,
            &request_fingerprint,
            locale,
            "STEP13",
        )
    } else {
        Vec::new()
    };
    let execution_evidence_verified = request_ready_for_execute
        && evidence_contract_verified
        && evidence_context_matches
        && scene_fingerprint_issue.is_none()
        && execution_object_issues.is_empty();
    if request_ready_for_execute && evidence_contract_verified && !evidence_context_matches {
        blockers.push(json!({
            "code": "STEP13_UNITY_EXECUTION_EVIDENCE_CONTEXT_MISMATCH",
            "message": localized_text(
                locale,
                "已有 Unity 执行证据来自另一组项目或编辑器路径，不能复用为本次场景组装成功证据。",
                "Existing Unity execution evidence belongs to a different project or editor path and cannot verify this scene-assembly request."
            ),
            "return_target": "stage_13",
        }));
    } else if request_ready_for_execute && !evidence_contract_verified {
        blockers.push(json!({
            "code": "STEP13_UNITY_EXECUTION_EVIDENCE_MISSING",
            "message": localized_text(
                locale,
                "Unity 编辑器请求已生成，但尚未收到真实的 Unity 执行与物化证据；不能确认场景、变更文件、PlayMode 或可见内容成功。",
                "The Unity Editor request was generated, but verified Unity execution and materialization evidence is still missing; scene, changed files, PlayMode, and visible content cannot be reported as successful."
            ),
            "return_target": "stage_13",
        }));
    } else if request_ready_for_execute {
        if let Some(issue) = scene_fingerprint_issue {
            blockers.push(issue);
        } else {
            blockers.extend(execution_object_issues);
        }
    }
    let status = if execution_evidence_verified {
        "success"
    } else {
        "blocked"
    };
    let static_structure = json!({
        "entry_scene": "Assets/Scenes/DemoScene.unity",
        "entry_scene_exists": execution_evidence_verified,
        "active_camera_exists": execution_evidence_verified,
        "runtime_root_exists": execution_evidence_verified,
        "canvas_ui_root_exists": execution_evidence_verified,
        "event_system_exists": execution_evidence_verified,
        "input_router_exists": execution_evidence_verified,
        "objective_tracker_exists": execution_evidence_verified,
        "game_state_exists": execution_evidence_verified,
        "initial_ui_exists": execution_evidence_verified,
        "visible_content_verified": execution_evidence_verified,
        "audio_placeholder_declared": true,
    });
    let summary = json!({
        "entry_scene": "Assets/Scenes/DemoScene.unity",
        "ui_screen_count": if request_ready_for_execute { 1 } else { 0 },
        "input_action_count": if request_ready_for_execute { 1 } else { 0 },
        "objective_count": if request_ready_for_execute { 1 } else { 0 },
        "produced_asset_count": value_array(art.get("produced_assets")).len(),
    });
    let config = json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "stage_output_path": out_dir.to_string_lossy(),
        "project_path": unity_project_path,
        "unity_editor_path": unity_editor_path,
        "request_fingerprint": request_fingerprint,
        "scene_path": "Assets/Scenes/DemoScene.unity",
        "contract_summary": summary,
    });
    write_json(&out_dir.join("scene_assembly_config.json"), &config)?;
    write_json(
        &out_dir.join("unity_editor_request.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "project_path": unity_project_path,
            "unity_editor_path": unity_editor_path,
            "request_fingerprint": request_fingerprint,
            "scene_path": "Assets/Scenes/DemoScene.unity",
            "requires_compile_before_execute_method": true,
            "execute_method": "AutoDesignMaker.SceneAssembly.Run",
            "arguments": ["-sceneAssemblyConfig", "stage_13/scene_assembly_config.json"],
            "compile_step": {
                "required": true,
                "execute_method": "AutoDesignMaker.SceneAssembly.Run",
            },
            "execute_steps": [{
                "execute_method": "AutoDesignMaker.SceneAssembly.Run",
                "arguments": ["-sceneAssemblyConfig", "stage_13/scene_assembly_config.json"],
            }],
            "status": if request_ready_for_execute { "ready" } else { "blocked" },
            "ready_for_execute": request_ready_for_execute,
            "blocking_issues": request_blockers,
            "artifact_locale": locale,
        }),
    )?;
    let changed_files = if execution_evidence_verified {
        previous_scene
            .get("changed_files")
            .cloned()
            .unwrap_or_else(|| json!([]))
    } else {
        json!([])
    };
    write_scene13_art_reports(
        out_dir,
        execution_evidence_verified,
        &changed_files,
        &blockers,
        locale,
    )?;
    let materialized_files = if execution_evidence_verified {
        previous_scene
            .get("materialized_files")
            .cloned()
            .unwrap_or_else(|| json!([]))
    } else {
        json!([])
    };
    let unity_result = if execution_evidence_verified {
        previous_scene
            .get("unity_result")
            .cloned()
            .unwrap_or_else(|| json!({}))
    } else {
        json!({"id": "scene_assembly_unity_execute", "status": "not_executed", "errors": []})
    };
    let report = json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "status": status,
        "request_generated": true,
        "request_ready_for_execute": request_ready_for_execute,
        "unity_attempted": execution_evidence_verified,
        "unity_result": unity_result,
        "execution_evidence_verified": execution_evidence_verified,
        "scene_path": "Assets/Scenes/DemoScene.unity",
        "project_path": unity_project_path,
        "unity_editor_path": unity_editor_path,
        "request_fingerprint": request_fingerprint,
        "execution_object_id": if execution_evidence_verified { string_field(&previous_scene, "execution_object_id") } else { String::new() },
        "execution_object_state": if execution_evidence_verified { "verified" } else { "not_started" },
        "changed_files": changed_files,
        "materialized_files": materialized_files,
        "unexpected_changes": if execution_evidence_verified { previous_scene.get("unexpected_changes").cloned().unwrap_or_else(|| json!([])) } else { json!([]) },
        "required_files": if execution_evidence_verified { previous_scene.get("required_files").cloned().unwrap_or_else(|| json!({})) } else { json!({"Assets/Scenes/DemoScene.unity": false}) },
        "source_contracts": ["stage_02/playable_contracts/playable_acceptance_contract.json"],
        "contract_summary": summary,
        "static_structure": static_structure,
        "demo_scene_exists": execution_evidence_verified,
        "build_settings_updated": execution_evidence_verified,
        "playmode_smoke_test_passed": execution_evidence_verified,
        "visible_content_verified": execution_evidence_verified,
        "blocking_issues": blockers,
        "artifact_locale": locale,
    });
    write_json(&out_dir.join("scene_assembly_report.json"), &report)?;
    write_json(
        &out_dir.join("scene_assembly_manifest.json"),
        &json!({"schema_version": 1, "scene_path": "Assets/Scenes/DemoScene.unity", "canvas_ui_root": execution_evidence_verified, "event_system": execution_evidence_verified, "input_router": execution_evidence_verified, "objective_tracker": execution_evidence_verified, "artifact_locale": locale}),
    )?;
    write_json(
        &out_dir.join("changed_files_manifest.json"),
        &json!({"schema_version": 1, "changed_files": report["changed_files"], "tasks": [{"task_id": "SCENE-ASSEMBLY", "changed_files": report["changed_files"]}]}),
    )?;
    write_json(
        &out_dir.join("playmode_smoke_test_result.json"),
        &json!({"schema_version": 1, "status": if execution_evidence_verified { "passed" } else { "not_executed" }, "scene_loads": execution_evidence_verified, "camera_exists": execution_evidence_verified, "runtime_root_exists": execution_evidence_verified, "canvas_ui_root_exists": execution_evidence_verified, "event_system_exists": execution_evidence_verified, "input_router_exists": execution_evidence_verified, "objective_tracker_exists": execution_evidence_verified, "game_state_exists": execution_evidence_verified, "visible_content_verified": execution_evidence_verified, "blocking_issues": report["blocking_issues"], "artifact_locale": locale}),
    )?;
    write_json(
        &out_dir.join("build_settings_update_report.json"),
        &json!({"schema_version": 1, "status": if execution_evidence_verified { "passed" } else { "not_executed" }, "build_settings_updated": execution_evidence_verified, "blocking_issues": report["blocking_issues"], "artifact_locale": locale}),
    )?;
    write_text(
        &out_dir.join("scene_assembly.md"),
        localized_text(locale, "# 场景组装\n", "# Scene Assembly\n"),
    )?;
    Ok(json!({
        "status": status,
        "artifact_locale": locale,
        "content_exists": true,
        "blocking_issues": report.get("blocking_issues").and_then(Value::as_array).cloned().unwrap_or_default(),
        "ai_review_status": if blockers_is_empty(&report) { "passed" } else { "blocked" },
        "traceability_valid": blockers_is_empty(&report),
    }))
}

fn stage14_outputs(
    parsed: &ParsedDesignSource,
    out_dir: &Path,
    structured_inputs: &Value,
) -> AdmResult<Value> {
    let locale = artifact_locale_from_inputs(structured_inputs);
    let mut environment_issues = structured_inputs
        .get("environment_blockers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, issue)| {
            ensure_issue_contract(
                issue,
                &format!("STEP14_ENVIRONMENT_ISSUE_{:03}", index + 1),
                "environment_configuration",
                localized_text(
                    locale,
                    "运行环境未满足集成验证条件。",
                    "The runtime environment is not ready for integration validation.",
                ),
            )
        })
        .collect::<Vec<_>>();
    let preflight_blocked = structured_inputs
        .get("preflight_status")
        .and_then(Value::as_str)
        == Some("blocked");
    if preflight_blocked && environment_issues.is_empty() {
        environment_issues.push(json!({
            "code": "STEP14_ENVIRONMENT_PREFLIGHT_BLOCKED",
            "message": localized_text(locale, "环境预检未通过。", "Environment preflight did not pass."),
            "return_target": "environment_configuration",
        }));
    }
    let scene = read_stage_json(out_dir, 13, "scene_assembly_report.json");
    let scene_smoke = read_stage_json(out_dir, 13, "playmode_smoke_test_result.json");
    let scene_build_settings = read_stage_json(out_dir, 13, "build_settings_update_report.json");
    let scene_changed_manifest = read_stage_json(out_dir, 13, "changed_files_manifest.json");
    let scene_manifest = read_stage_json(out_dir, 13, "scene_assembly_manifest.json");
    let unity_request = read_stage_json(out_dir, 13, "unity_editor_request.json");
    let scene_config = read_stage_json(out_dir, 13, "scene_assembly_config.json");
    let unity_project_path = non_empty_or(
        string_field(structured_inputs, "unity_project_path"),
        &string_field(&unity_request, "project_path"),
    );
    let unity_editor_path = non_empty_or(
        string_field(structured_inputs, "unity_editor_path"),
        &string_field(&unity_request, "unity_editor_path"),
    );
    let request_fingerprint = unity_request_fingerprint(
        parsed,
        out_dir,
        locale,
        &unity_project_path,
        &unity_editor_path,
    )?;
    let evidence_contract_verified = scene_execution_evidence_verified(
        &scene,
        &scene_smoke,
        &scene_build_settings,
        &scene_changed_manifest,
        &scene_manifest,
    );
    let evidence_context_matches =
        scene_execution_context_matches(&scene, &unity_project_path, &unity_editor_path);
    let mut fingerprint_issues = Vec::new();
    if evidence_contract_verified {
        if let Some(issue) = request_fingerprint_issue(
            &unity_request,
            &request_fingerprint,
            locale,
            "STEP14_EDITOR_REQUEST",
            "步骤13的 Unity 编辑器请求",
            "The Stage13 Unity Editor request",
        ) {
            fingerprint_issues.push(issue);
        }
        if let Some(issue) = request_fingerprint_issue(
            &scene_config,
            &request_fingerprint,
            locale,
            "STEP14_SCENE_CONFIG",
            "步骤13的场景组装配置",
            "The Stage13 scene-assembly config",
        ) {
            fingerprint_issues.push(issue);
        }
        if let Some(issue) = request_fingerprint_issue(
            &scene,
            &request_fingerprint,
            locale,
            "STEP14",
            "步骤13的 Unity 场景报告",
            "The Stage13 Unity scene report",
        ) {
            fingerprint_issues.push(issue);
        }
    }
    let execution_object_issues = if evidence_contract_verified
        && evidence_context_matches
        && fingerprint_issues.is_empty()
    {
        scene_execution_object_issues(out_dir, &scene, &request_fingerprint, locale, "STEP14")
    } else {
        Vec::new()
    };
    let execution_evidence_verified = evidence_contract_verified
        && evidence_context_matches
        && fingerprint_issues.is_empty()
        && execution_object_issues.is_empty();
    let scene_static = scene
        .get("static_structure")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut blockers = Vec::new();
    let acceptance_contract_present = !is_empty_object(&read_stage_json(
        out_dir,
        2,
        "playable_contracts/playable_acceptance_contract.json",
    ));
    if !acceptance_contract_present {
        blockers.push(json!({
            "code": "STEP14_PLAYABLE_ACCEPTANCE_CONTRACT_MISSING",
            "message": localized_text(locale, "步骤14需要步骤02的 playable_acceptance_contract。", "Step14 requires Stage02 playable_acceptance_contract."),
            "return_target": "stage_02",
        }));
    }
    let scene_ready = scene.get("status").and_then(Value::as_str) == Some("success")
        && execution_evidence_verified;
    if !execution_evidence_verified {
        blockers.push(json!({
            "code": "STEP14_UNITY_EXECUTION_EVIDENCE_NOT_VERIFIED",
            "source": "stage_13/scene_assembly_report.json",
            "message": localized_text(
                locale,
                "步骤13没有提供可核验的 Unity 执行与物化证据，步骤14不能确认场景、PlayMode 或可见内容通过。",
                "Stage13 did not provide verifiable Unity execution and materialization evidence, so Step14 cannot accept the scene, PlayMode, or visible content."
            ),
            "return_target": "stage_13",
        }));
        if evidence_contract_verified && !evidence_context_matches {
            blockers.push(json!({
                "code": "STEP14_UNITY_EXECUTION_EVIDENCE_CONTEXT_MISMATCH",
                "message": localized_text(
                    locale,
                    "步骤13的 Unity 场景报告与当前编辑器请求路径不一致。",
                    "The Stage13 Unity scene report does not match the current Editor request paths."
                ),
                "return_target": "stage_13",
            }));
        }
        blockers.extend(fingerprint_issues);
        blockers.extend(execution_object_issues);
    }
    let structure_fields = [
        "canvas_ui_root_exists",
        "event_system_exists",
        "input_router_exists",
        "objective_tracker_exists",
        "visible_content_verified",
    ];
    if execution_evidence_verified {
        for field in structure_fields {
            if scene_static.get(field).and_then(Value::as_bool) == Some(true) {
                continue;
            }
            blockers.push(json!({
                "code": "STEP14_PLAYABLE_STRUCTURE_MISSING",
                "field": field,
                "source": "stage_13/scene_assembly_report.json",
                "message": localized_owned(
                    locale,
                    format!("步骤13的场景结构缺少必需项 `{field}`。"),
                    format!("Stage13 scene structure is missing required field `{field}`."),
                ),
                "return_target": "stage_13",
            }));
        }
    }
    let environment_blocked = !environment_issues.is_empty();
    let status = if environment_blocked {
        "environment_blocked"
    } else if blockers.is_empty() {
        "success"
    } else {
        "blocked"
    };
    let checks = vec![
        json!({"id": "playable_acceptance_contract_present", "passed": acceptance_contract_present}),
        json!({"id": "scene_assembly_success", "passed": scene_ready}),
        json!({"id": "unity_execution_evidence_verified", "passed": execution_evidence_verified}),
        json!({"id": "active_camera_exists", "passed": execution_evidence_verified && scene_smoke.get("camera_exists").and_then(Value::as_bool) == Some(true)}),
        json!({"id": "runtime_root_exists", "passed": execution_evidence_verified && scene_smoke.get("runtime_root_exists").and_then(Value::as_bool) == Some(true)}),
        json!({"id": "canvas_ui_root_exists", "passed": execution_evidence_verified && scene_smoke.get("canvas_ui_root_exists").and_then(Value::as_bool) == Some(true)}),
        json!({"id": "event_system_exists", "passed": execution_evidence_verified && scene_smoke.get("event_system_exists").and_then(Value::as_bool) == Some(true)}),
        json!({"id": "input_router_exists", "passed": execution_evidence_verified && scene_smoke.get("input_router_exists").and_then(Value::as_bool) == Some(true)}),
        json!({"id": "objective_tracker_exists", "passed": execution_evidence_verified && scene_smoke.get("objective_tracker_exists").and_then(Value::as_bool) == Some(true)}),
        json!({"id": "visible_content_verified", "passed": execution_evidence_verified && scene_smoke.get("visible_content_verified").and_then(Value::as_bool) == Some(true)}),
        json!({"id": "environment_ready", "passed": !environment_blocked}),
    ];
    let level_checks = |ids: &[&str]| {
        checks
            .iter()
            .filter(|check| {
                check
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|id| ids.contains(&id))
                    .unwrap_or(false)
            })
            .cloned()
            .collect::<Vec<_>>()
    };
    let validation_levels = vec![
        json!({
            "level": 1,
            "name": "artifact_presence",
            "status": if acceptance_contract_present { "passed" } else { "failed" },
            "checks": level_checks(&["playable_acceptance_contract_present"]),
        }),
        json!({
            "level": 2,
            "name": "execution_readiness",
            "status": if scene_ready { "passed" } else { "failed" },
            "checks": level_checks(&["scene_assembly_success", "unity_execution_evidence_verified"]),
        }),
        json!({
            "level": 3,
            "name": "static_scene_structure",
            "status": if blockers.is_empty() { "passed" } else { "failed" },
            "checks": level_checks(&[
                "active_camera_exists",
                "runtime_root_exists",
                "canvas_ui_root_exists",
                "event_system_exists",
                "input_router_exists",
                "objective_tracker_exists",
            ]),
        }),
        json!({
            "level": 4,
            "name": "environment_readiness",
            "status": if environment_blocked { "failed" } else { "passed" },
            "checks": level_checks(&["environment_ready"]),
        }),
        json!({
            "level": 5,
            "name": "playable_acceptance",
            "status": if blockers.is_empty() && !environment_blocked { "passed" } else { "failed" },
            "checks": level_checks(&["visible_content_verified"]),
        }),
    ];
    let playmode = json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "status": status,
        "request_fingerprint": request_fingerprint,
        "validation_levels": validation_levels,
        "environment_blockers": environment_issues,
        "environment_issues": environment_issues,
        "unity_execution_evidence_verified": execution_evidence_verified,
        "scene_loads": execution_evidence_verified && scene_smoke.get("scene_loads").and_then(Value::as_bool) == Some(true),
        "camera_exists": execution_evidence_verified && scene_smoke.get("camera_exists").and_then(Value::as_bool) == Some(true),
        "runtime_root_exists": execution_evidence_verified && scene_smoke.get("runtime_root_exists").and_then(Value::as_bool) == Some(true),
        "canvas_ui_root_exists": execution_evidence_verified && scene_smoke.get("canvas_ui_root_exists").and_then(Value::as_bool) == Some(true),
        "event_system_exists": execution_evidence_verified && scene_smoke.get("event_system_exists").and_then(Value::as_bool) == Some(true),
        "input_router_exists": execution_evidence_verified && scene_smoke.get("input_router_exists").and_then(Value::as_bool) == Some(true),
        "objective_tracker_exists": execution_evidence_verified && scene_smoke.get("objective_tracker_exists").and_then(Value::as_bool) == Some(true),
        "visible_content_verified": execution_evidence_verified && scene_smoke.get("visible_content_verified").and_then(Value::as_bool) == Some(true),
        "artifact_locale": locale,
    });
    let art_checks = vec![
        json!({"id": "scene_assembly_success", "passed": scene_ready}),
        json!({"id": "unity_execution_evidence_verified", "passed": execution_evidence_verified}),
        json!({"id": "visible_content_verified", "passed": execution_evidence_verified && scene_smoke.get("visible_content_verified").and_then(Value::as_bool) == Some(true)}),
    ];
    let art_report = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "status": if blockers.is_empty() && !environment_blocked { "success" } else { "blocked" },
        "source_refs": ["stage_12/art_handoff_manifest.json", "stage_13/unity_art_import_report.json"],
        "checks": art_checks,
        "blockers": blockers,
        "artifact_locale": locale,
    });
    let playable_report = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "status": if blockers.is_empty() && !environment_blocked { "success" } else { status },
        "source_refs": ["stage_02/playable_contracts/playable_acceptance_contract.json", "stage_13/scene_assembly_report.json"],
        "validation_levels": validation_levels,
        "blockers": blockers,
        "artifact_locale": locale,
    });
    let report = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "status": status,
        "request_fingerprint": request_fingerprint,
        "unity_execution_evidence_verified": execution_evidence_verified,
        "validation_levels": validation_levels,
        "checks": checks,
        "blocking_issues": blockers,
        "environment_issues": environment_issues,
        "scene_assembly_report": "stage_13/scene_assembly_report.json",
        "playmode_test_results": "playmode_test_results.json",
        "artifact_locale": locale,
    });
    write_json(&out_dir.join("playmode_test_results.json"), &playmode)?;
    write_json(&out_dir.join("art_acceptance_report.json"), &art_report)?;
    write_json(
        &out_dir.join("playable_acceptance_report.json"),
        &playable_report,
    )?;
    write_json(&out_dir.join("integration_validation_report.json"), &report)?;
    Ok(json!({
        "status": status,
        "artifact_locale": locale,
        "content_exists": true,
        "blocking_issues": report.get("blocking_issues").cloned().unwrap_or_else(|| json!([])),
        "environment_issues": report.get("environment_issues").cloned().unwrap_or_else(|| json!([])),
        "ai_review_status": if status == "success" { "passed" } else { status },
        "traceability_valid": status == "success",
    }))
}

fn playable_contracts(out_dir: &Path) -> BTreeMap<String, Value> {
    [
        "core_playable_contract",
        "demo_flow_contract",
        "runtime_data_contract",
        "ui_flow_contract",
        "scene_bootstrap_contract",
        "asset_mount_contract",
        "audio_requirements_contract",
        "playable_acceptance_contract",
    ]
    .into_iter()
    .map(|name| {
        (
            name.to_string(),
            read_stage_json(out_dir, 2, &format!("playable_contracts/{name}.json")),
        )
    })
    .collect()
}

fn program_tasks_from_contract(
    program_contract: &Value,
    structure_spec: &Value,
    parsed: &ParsedDesignSource,
    locale: ArtifactLocale,
) -> Vec<ProgramTask> {
    let path_map = value_array(structure_spec.get("system_path_map"));
    let requirements = value_array(program_contract.get("requirements"));
    let mut tasks = Vec::new();
    for requirement in requirements {
        if is_documentation_requirement(&requirement) {
            continue;
        }
        let requirement_id = non_empty_or(
            string_field(&requirement, "id"),
            &format!("REQ-{:03}", tasks.len() + 1),
        );
        let route = path_map.iter().find(|item| {
            string_field(item, "requirement_id") == requirement_id
                || string_field(item, "selection_id") == requirement_id
        });
        let output_files = non_empty_strings(string_array(requirement.get("outputs")))
            .or_else(|| {
                route
                    .map(|item| route_outputs(item))
                    .filter(|items| !items.is_empty())
            })
            .unwrap_or_else(|| {
                vec![format!(
                    "Assets/Scripts/AutoDesignMaker/DEV{:03}.cs",
                    tasks.len() + 1
                )]
            });
        let allowed_write_paths = route
            .and_then(|item| {
                let paths = string_array(item.get("allowed_write_paths"));
                (!paths.is_empty()).then_some(paths)
            })
            .unwrap_or_else(|| output_files.clone());
        let category = requirement_category(&requirement);
        tasks.push(ProgramTask {
            task_id: format!("DEV-{:03}", tasks.len() + 1),
            requirement_id: requirement_id.clone(),
            title: clean_task_title(
                &non_empty_or(
                    string_field(&requirement, "requirement"),
                    &localized_owned(
                        locale,
                        format!("实现 {requirement_id}。"),
                        format!("Implement {requirement_id}."),
                    ),
                ),
                locale,
            ),
            phase: non_empty_or(string_field(&requirement, "phase"), "core_playable"),
            category,
            priority: "P0".to_string(),
            target_path: output_files.first().cloned().unwrap_or_default(),
            output_files,
            allowed_write_paths,
            verification_commands: vec!["contract_static_check".to_string()],
            source_refs: non_empty_strings(string_array(requirement.get("source_refs")))
                .unwrap_or_else(|| vec![parsed.source.clone()]),
            acceptance: non_empty_or(
                string_field(&requirement, "acceptance"),
                localized_text(
                    locale,
                    "任务输出存在且可追溯。",
                    "Task output exists and is traceable.",
                ),
            ),
        });
    }
    if tasks.is_empty() && !parsed.selections.is_empty() {
        for selection in parsed.selections.iter().take(3) {
            tasks.push(ProgramTask {
                task_id: format!("DEV-{:03}", tasks.len() + 1),
                requirement_id: selection.id(),
                title: clean_task_title(
                    &localized_owned(
                        locale,
                        format!("实现 {}", selection.label()),
                        format!("Implement {}", selection.label()),
                    ),
                    locale,
                ),
                phase: "core_playable".to_string(),
                category: category_from_text(&selection.label()),
                priority: "P0".to_string(),
                target_path: format!("Assets/Scripts/AutoDesignMaker/{}.cs", selection.id()),
                output_files: vec![format!(
                    "Assets/Scripts/AutoDesignMaker/{}.cs",
                    selection.id()
                )],
                allowed_write_paths: vec!["Assets/Scripts/".to_string()],
                verification_commands: vec!["contract_static_check".to_string()],
                source_refs: vec![selection.source_ref.clone()],
                acceptance: localized_text(
                    locale,
                    "所选设计项已由运行时代码实现。",
                    "Selection is represented by runtime code.",
                )
                .to_string(),
            });
        }
    }
    tasks
}

fn art_tasks_from_registry(
    assets: Vec<Value>,
    style_contract: &Value,
    locale: ArtifactLocale,
) -> Vec<ArtTask> {
    assets
        .iter()
        .enumerate()
        .map(|(index, asset)| {
            let asset_id = non_empty_or(string_field(asset, "asset_id"), &format!("ASSET-{:03}", index + 1));
            let asset_type = non_empty_or(string_field(asset, "asset_type"), "asset");
            let category = art_category(&asset_type);
            let source_refs = {
                let mut refs = string_array(asset.get("source_refs"));
                if refs.is_empty() {
                    let source = string_field(asset, "source");
                    if !source.is_empty() {
                        refs.push(source);
                    }
                }
                refs.push("stage_07/style_application_contract.json".to_string());
                sorted_unique(refs)
            };
            ArtTask {
                task_id: format!("ART-{:03}", index + 1),
                asset_id: asset_id.clone(),
                title: non_empty_or(
                    string_field(asset, "name"),
                    &localized_owned(
                        locale,
                        format!("{asset_type} 资源 {asset_id}"),
                        format!("{asset_type} asset {asset_id}"),
                    ),
                ),
                asset_type: asset_type.clone(),
                category,
                priority: non_empty_or(string_field(asset, "priority"), "P0"),
                complexity: non_empty_or(string_field(asset, "complexity"), "m"),
                unity_target_path: non_empty_or(
                    string_field(asset, "unity_target_path"),
                    &non_empty_or(
                        string_field(asset, "target_path"),
                        &format!("Assets/AutoDesign/Art/Source/{asset_id}.png"),
                    ),
                ),
                dimensions: asset_dimensions(asset),
                consumer_system: non_empty_or(string_field(asset, "consumer_system"), "gameplay"),
                mount_point: string_field(asset, "mount_point"),
                acceptance: non_empty_or(
                    string_field(asset, "acceptance_check"),
                    localized_text(
                        locale,
                        "资源已生成并正确挂载。",
                        "Asset is produced and mounted.",
                    ),
                ),
                source_refs,
                generation_prompt: localized_owned(
                    locale,
                    format!(
                        "为风格 {} 创建资源 {}。",
                        string_field(style_contract, "selected_style_id"),
                        asset_id,
                    ),
                    format!(
                        "Create {} for style {}.",
                        asset_id,
                        string_field(style_contract, "selected_style_id")
                    ),
                ),
                negative_prompt: localized_text(
                    locale,
                    "避免水印、签名、无关文字、模糊边缘和不符合既定风格的元素。",
                    "Avoid watermarks, signatures, unrelated text, blurred edges, and elements outside the approved style.",
                )
                .to_string(),
                production_tier: "placeholder_or_generated".to_string(),
                production_execution_strategy: "deterministic_contract_first".to_string(),
                semantic_policy: json!({"trace_required": true, "style_contract": "stage_07/style_application_contract.json"}),
                rework_policy: json!({"queue_artifact": "stage_12/art_rework_queue.json"}),
            }
        })
        .collect()
}

fn write_stage12_support_files(
    out_dir: &Path,
    produced_assets: &[Value],
    mount_items: &[Value],
    missing_assets: &[Value],
    production_blockers: &[Value],
    ready: bool,
    locale: ArtifactLocale,
) -> AdmResult<()> {
    let asset_count = produced_assets.len();
    let review_status = if ready { "passed" } else { "blocked" };
    let quality_checks = produced_assets
        .iter()
        .map(|asset| {
            json!({
                "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
                "path": asset.get("path").cloned().unwrap_or(Value::Null),
                "check": "verified_png_output",
                "status": "passed",
                "source_refs": asset.get("source_refs").cloned().unwrap_or_else(|| json!([])),
            })
        })
        .collect::<Vec<_>>();
    let semantic_reviews = produced_assets
        .iter()
        .map(|asset| {
            json!({
                "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
                "status": "passed",
                "message": localized_text(
                    locale,
                    "资源已按任务合同生成，并保留了可追溯来源。",
                    "The asset was produced from its task contract with traceable sources.",
                ),
                "source_refs": asset.get("source_refs").cloned().unwrap_or_else(|| json!([])),
            })
        })
        .collect::<Vec<_>>();
    let rework_items = missing_assets.to_vec();
    write_json(
        &out_dir.join("audio_placeholder_manifest_runtime.json"),
        &json!({"schema_version": 1, "generated_at": now_iso(), "placeholder_files": [], "artifact_locale": locale}),
    )?;
    write_json(
        &out_dir.join("raw_generated_asset_manifest.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "raw_assets": produced_assets,
            "assets": produced_assets,
            "artifact_locale": locale,
        }),
    )?;
    write_json(
        &out_dir.join("image_quality_report.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "status": review_status,
            "checked_asset_count": asset_count,
            "checks": quality_checks,
            "blockers": production_blockers,
            "warnings": [],
            "artifact_locale": locale,
        }),
    )?;
    write_json(
        &out_dir.join("art_semantic_review_report.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "status": review_status,
            "reviews": semantic_reviews,
            "blockers": production_blockers,
            "rework_items": rework_items,
            "artifact_locale": locale,
        }),
    )?;
    write_json(
        &out_dir.join("art_rework_queue.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "items": rework_items,
            "blocking_count": missing_assets.len(),
            "review_count": 0,
            "artifact_locale": locale,
        }),
    )?;
    write_json(
        &out_dir.join("processed_asset_manifest.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "processed_assets": produced_assets,
            "assets": produced_assets,
            "artifact_locale": locale,
        }),
    )?;
    write_json(
        &out_dir.join("sprite_slice_result_manifest.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "policy": localized_text(
                locale,
                "Rust 确定性合同路径不会通过 Python 执行像素裁切。",
                "No Python pixel slicing is performed in the Rust deterministic contract path.",
            ),
            "slices": [],
            "assets": produced_assets,
            "artifact_locale": locale,
        }),
    )?;
    write_json(
        &out_dir.join("unity_import_settings_manifest.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "import_items": [],
            "settings": [],
            "artifact_locale": locale,
        }),
    )?;
    write_json(
        &out_dir.join("sprite_atlas_plan.json"),
        &json!({"schema_version": "1.0", "generated_at": now_iso(), "atlases": [], "artifact_locale": locale}),
    )?;
    write_json(
        &out_dir.join("addressable_asset_plan.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "status": "not_required",
            "required": false,
            "reason": localized_text(locale, "当前合同未启用 Unity Addressables。", "Unity Addressables are not enabled by the current contract."),
            "groups": [],
            "entries": produced_assets,
            "artifact_locale": locale,
        }),
    )?;
    write_json(
        &out_dir.join("ugui_prefab_contract.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "prefabs": mount_items,
            "requests": mount_items,
            "artifact_locale": locale,
        }),
    )?;
    write_json(
        &out_dir.join("ui_prefab_generation_request.json"),
        &json!({"schema_version": "1.0", "generated_at": now_iso(), "requests": mount_items, "artifact_locale": locale}),
    )?;
    write_json(
        &out_dir.join("asset_mount_manifest.json"),
        &json!({"schema_version": "1.0", "generated_at": now_iso(), "mount_items": mount_items, "artifact_locale": locale}),
    )?;
    write_json(
        &out_dir.join("program_asset_binding_preflight.json"),
        &json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "ready": ready,
            "valid": ready,
            "blockers": production_blockers,
            "checked_mount_items": mount_items.len(),
            "artifact_locale": locale,
        }),
    )?;
    write_json(
        &out_dir.join("art_handoff_manifest.json"),
        &json!({"schema_version": "1.0", "generated_at": now_iso(), "ready_for_step13": ready, "blockers": production_blockers, "source_refs": ["stage_12/asset_mount_manifest.json", "stage_12/program_asset_binding_preflight.json"], "asset_count": asset_count, "mount_item_count": mount_items.len(), "artifact_locale": locale}),
    )?;
    Ok(())
}

fn write_scene13_art_reports(
    out_dir: &Path,
    success: bool,
    changed_files: &Value,
    blockers: &[Value],
    locale: ArtifactLocale,
) -> AdmResult<()> {
    let status = if success { "success" } else { "blocked" };
    let actual_changed_files = string_array(Some(changed_files));
    let imported_assets = actual_changed_files
        .iter()
        .filter(|path| {
            let path = path.to_ascii_lowercase();
            path.ends_with(".png")
                || path.ends_with(".jpg")
                || path.ends_with(".jpeg")
                || path.ends_with(".psd")
                || path.ends_with(".asset")
        })
        .count();
    let generated_prefabs = actual_changed_files
        .iter()
        .filter(|path| path.to_ascii_lowercase().ends_with(".prefab"))
        .cloned()
        .collect::<Vec<_>>();
    write_json(
        &out_dir.join("unity_art_import_report.json"),
        &json!({"schema_version": "1.0", "generated_at": now_iso(), "status": status, "imported_assets": if success { imported_assets } else { 0 }, "source_refs": ["stage_12/art_handoff_manifest.json"], "blockers": blockers, "artifact_locale": locale}),
    )?;
    write_json(
        &out_dir.join("unity_prefab_generation_report.json"),
        &json!({"schema_version": "1.0", "generated_at": now_iso(), "status": status, "generated_prefabs": if success { json!(generated_prefabs) } else { json!([]) }, "source_refs": ["stage_12/ui_prefab_generation_request.json"], "blockers": blockers, "artifact_locale": locale}),
    )?;
    write_json(
        &out_dir.join("program_asset_binding_contract.json"),
        &json!({"schema_version": "1.0", "generated_at": now_iso(), "status": status, "bindings_verified": success, "source_refs": ["stage_12/asset_mount_manifest.json"], "blockers": blockers, "artifact_locale": locale}),
    )?;
    write_json(
        &out_dir.join("unity_scene_mount_report.json"),
        &json!({"schema_version": "1.0", "generated_at": now_iso(), "status": status, "scene_path": "Assets/Scenes/DemoScene.unity", "visible_content_verified": success, "mount_item_count": if success { 1 } else { 0 }, "source_refs": ["stage_12/art_handoff_manifest.json"], "blockers": blockers, "artifact_locale": locale}),
    )?;
    Ok(())
}

fn missing_stage8_artifacts(out_dir: &Path, step: u32, locale: ArtifactLocale) -> Vec<Value> {
    SCENE_ASSEMBLY_REQUIRED_STAGE8_ARTIFACTS
        .iter()
        .filter(|filename| is_empty_object(&read_stage_json(out_dir, 8, filename)))
        .map(|filename| {
            json!({
                "code": format!("STEP{step:02}_REQUIRED_STAGE08_ARTIFACT_MISSING"),
                "artifact": format!("stage_08/{filename}"),
                "message": localized_owned(
                    locale,
                    format!("步骤{step:02}需要步骤08的 `{filename}`。"),
                    format!("Step{step:02} requires Stage08 `{filename}`."),
                ),
                "return_target": "stage_08",
            })
        })
        .collect()
}

fn stage8_program_tasks(out_dir: &Path) -> Vec<Value> {
    value_array(read_stage_json(out_dir, 8, "program_task_breakdown.json").get("tasks"))
}

fn read_stage_json(out_dir: &Path, stage: u32, filename: &str) -> Value {
    let artifacts_dir = out_dir.parent().unwrap_or(out_dir);
    read_json(
        &artifacts_dir
            .join(format!("stage_{stage:02}"))
            .join(filename),
        json!({}),
    )
}

fn scene_assembly_requirements(
    contracts: &BTreeMap<String, Value>,
    tasks: &[ProgramTask],
    locale: ArtifactLocale,
) -> Value {
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "tasks": tasks.iter().map(|task| json!({
            "task_id": task.task_id,
            "target_object": "RuntimeRoot",
            "required_components": ["Transform"],
            "source_refs": task.source_refs,
        })).collect::<Vec<_>>(),
        "source_refs": ["stage_02/playable_contracts/scene_bootstrap_contract.json"],
        "contract_present": !is_empty_object(contracts.get("scene_bootstrap_contract").unwrap_or(&Value::Null)),
        "artifact_locale": locale,
    })
}

fn ui_runtime_requirements(
    contracts: &BTreeMap<String, Value>,
    tasks: &[ProgramTask],
    locale: ArtifactLocale,
) -> Value {
    let mut screen_items = contracts
        .get("ui_flow_contract")
        .and_then(|contract| contract.get("screens"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if screen_items.is_empty() && !tasks.is_empty() {
        screen_items.push(json!({"screen_id": "game_hud", "required_widgets": []}));
    }
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "screens": screen_items,
        "tasks": tasks.iter().take(screen_items.len().max(1)).enumerate().map(|(index, task)| {
            let screen_id = screen_items
                .get(index % screen_items.len().max(1))
                .map(|screen| non_empty_or(string_field(screen, "screen_id"), "game_hud"))
                .unwrap_or_else(|| "game_hud".to_string());
            json!({
                "task_id": task.task_id,
                "ui_target": screen_id,
                "binding_refs": task.source_refs,
                "source_refs": task.source_refs,
            })
        }).collect::<Vec<_>>(),
        "source_refs": ["stage_02/playable_contracts/ui_flow_contract.json"],
        "artifact_locale": locale,
    })
}

fn input_runtime_requirements(
    contracts: &BTreeMap<String, Value>,
    locale: ArtifactLocale,
) -> Value {
    let actions = contracts
        .get("core_playable_contract")
        .and_then(|contract| contract.get("action_verbs"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| vec![json!("primary_action")]);
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "actions": actions.iter().enumerate().map(|(index, action)| {
            let fallback_id = format!("ACTION-{:03}", index + 1);
            let action_id = if action.is_object() {
                non_empty_or(string_field(action, "action_id"), &fallback_id)
            } else {
                fallback_id
            };
            let scalar = scalar_to_string(action);
            let label = if action.is_object() {
                non_empty_or(
                    string_field(action, "display_name"),
                    &non_empty_or(string_field(action, "label"), &action_id),
                )
            } else {
                non_empty_or(scalar.clone(), &action_id)
            };
            let binding = if action.is_object() {
                non_empty_or(
                    string_field(action, "input_binding"),
                    &non_empty_or(string_field(action, "binding"), "PrimaryAction"),
                )
            } else {
                non_empty_or(scalar, "PrimaryAction")
            };
            json!({
                "action_id": action_id,
                "label": label,
                "binding": binding,
                "source_refs": string_array(action.get("source_refs")).into_iter().chain(["stage_02/playable_contracts/core_playable_contract.json".to_string()]).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "tasks": actions.iter().enumerate().map(|(index, action)| {
            let fallback_id = format!("ACTION-{:03}", index + 1);
            let action_id = if action.is_object() {
                non_empty_or(string_field(action, "action_id"), &fallback_id)
            } else {
                fallback_id
            };
            json!({
            "task_id": format!("INPUT-{:03}", index + 1),
            "action_id": action_id,
            "runtime_target": "InputRouter",
            "source_refs": ["stage_02/playable_contracts/core_playable_contract.json"],
        })}).collect::<Vec<_>>(),
        "source_refs": ["stage_02/playable_contracts/core_playable_contract.json"],
        "artifact_locale": locale,
    })
}

fn objective_runtime_requirements(
    _contracts: &BTreeMap<String, Value>,
    tasks: &[ProgramTask],
    locale: ArtifactLocale,
) -> Value {
    let objectives = tasks
        .iter()
        .take(1)
        .enumerate()
        .map(|(index, task)| {
            json!({
                "objective_id": format!("OBJECTIVE-{:03}", index + 1),
                "success_condition": task.acceptance,
                "source_refs": task.source_refs,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "objectives": objectives,
        "tasks": tasks.iter().take(1).enumerate().map(|(index, task)| json!({
            "task_id": task.task_id,
            "objective_id": format!("OBJECTIVE-{:03}", index + 1),
            "runtime_target": "ObjectiveTracker",
            "objective": task.acceptance,
            "source_refs": task.source_refs,
        })).collect::<Vec<_>>(),
        "source_refs": ["stage_02/playable_contracts/playable_acceptance_contract.json"],
        "artifact_locale": locale,
    })
}

fn playable_plan_summary(
    contracts: &BTreeMap<String, Value>,
    tasks: &[ProgramTask],
    locale: ArtifactLocale,
) -> Value {
    json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "contract_source": "stage_02/playable_contracts",
        "contract_count": contracts.values().filter(|value| !is_empty_object(value)).count(),
        "task_count": tasks.len(),
        "tasks": tasks.iter().map(|task| json!({
            "task_id": task.task_id,
            "task_type": task.category,
            "requirement_id": task.requirement_id,
            "output_files": task.output_files,
        })).collect::<Vec<_>>(),
        "artifact_locale": locale,
    })
}

fn task_contract_gate(tasks: &[ProgramTask]) -> Value {
    let mut bad = 0usize;
    let mut checked = 0usize;
    for task in tasks {
        for output in &task.output_files {
            checked += 1;
            if !task
                .allowed_write_paths
                .iter()
                .any(|allowed| output_is_allowed(output, allowed))
            {
                bad += 1;
            }
        }
    }
    json!({
        "schema_version": 1,
        "valid": bad == 0,
        "bad_output_allowed_pairs": bad,
        "generic_fallback_count": 0,
        "checked_output_count": checked,
    })
}

fn output_is_allowed(output: &str, allowed: &str) -> bool {
    let output = normalize_path(output);
    let allowed = normalize_path(allowed);
    output == allowed || output.starts_with(&(allowed + "/"))
}

fn route_outputs(route: &Value) -> Vec<String> {
    let mut outputs = Vec::new();
    for key in ["target_path", "test_path"] {
        let path = string_field(route, key);
        if !path.is_empty() {
            if path.ends_with('/') || path.ends_with('\\') {
                outputs.push(format!("{}GeneratedTask.cs", normalize_path(&path)));
            } else {
                outputs.push(path);
            }
        }
    }
    outputs
}

fn is_documentation_requirement(requirement: &Value) -> bool {
    let text = format!(
        "{} {}",
        string_field(requirement, "requirement"),
        string_array(requirement.get("source_refs")).join(" ")
    )
    .to_lowercase();
    text.contains("documentation") || text.contains("文档")
}

fn clean_task_title(text: &str, locale: ArtifactLocale) -> String {
    let mut title = text
        .replace("范本反推", "")
        .replace("基于公开信息", "")
        .replace("非官方配置", "")
        .replace("：", ":")
        .trim()
        .to_string();
    while title.contains("  ") {
        title = title.replace("  ", " ");
    }
    if title.is_empty() {
        localized_text(
            locale,
            "实现可追溯的玩法任务。",
            "Implement traced gameplay task.",
        )
        .to_string()
    } else {
        title
    }
}

fn requirement_category(requirement: &Value) -> String {
    category_from_text(&format!(
        "{} {} {}",
        string_field(requirement, "requirement"),
        string_field(requirement, "phase"),
        string_array(requirement.get("dependencies")).join(" ")
    ))
}

fn category_from_text(text: &str) -> String {
    let lower = text.to_lowercase();
    if [
        "combat", "attack", "weapon", "skill", "damage", "战斗", "攻击",
    ]
    .iter()
    .any(|token| lower.contains(token))
    {
        "combat".to_string()
    } else if ["ui", "hud", "界面", "用户界面"]
        .iter()
        .any(|token| lower.contains(token))
    {
        "ui".to_string()
    } else if lower.contains("scene") || lower.contains("场景") {
        "scene".to_string()
    } else {
        "runtime".to_string()
    }
}

fn art_category(asset_type: &str) -> String {
    match asset_type {
        "effect" | "vfx" => "vfx",
        "ui" | "icon" => "ui",
        "environment" | "background" | "room" => "environment",
        "character" | "enemy" => "character",
        "audio" | "music" | "sfx" => "audio",
        _ => "asset",
    }
    .to_string()
}

fn semantic_matrix<T: Serialize>(
    scope: &str,
    items: &T,
    valid: bool,
    locale: ArtifactLocale,
) -> Value {
    let value = to_json_value(items).unwrap_or_else(|_| json!([]));
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "scope": scope,
        "matrix_state": if scope == "alignment" { "final" } else { scope },
        "valid": valid,
        "items": value,
        "coverage_items": value,
        "blockers": [],
        "artifact_locale": locale,
    })
}

fn program_semantic_matrix(
    semantic_coverage: &Value,
    tasks: &[ProgramTask],
    valid: bool,
    locale: ArtifactLocale,
) -> Value {
    let coverage_items = value_array(semantic_coverage.get("coverage_items"));
    let blockers = value_array(semantic_coverage.get("blockers"));
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "scope": "program",
        "matrix_state": "planned",
        "valid": valid && blockers.is_empty(),
        "project_signature": string_field(semantic_coverage, "project_signature"),
        "source_contracts": non_empty_strings(string_array(semantic_coverage.get("source_contracts"))).unwrap_or_else(|| vec![
            "stage_03/program_capability_contract.json".to_string(),
            "stage_03/program_semantic_coverage_report.json".to_string(),
        ]),
        "coverage_items": coverage_items,
        "items": tasks,
        "planned_task_ids": task_ids(tasks),
        "uncovered_requirements": value_array(semantic_coverage.get("missing_program_capabilities")),
        "blockers": blockers,
        "artifact_locale": locale,
    })
}

fn score_report(stage_id: &str, source: &Value, locale: ArtifactLocale) -> Value {
    let score = if blockers_is_empty(source) { 100 } else { 60 };
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "stage_id": stage_id,
        "status": if blockers_is_empty(source) { "passed" } else { "blocked" },
        "scores": { "overall": score },
        "score": score,
        "generic_content_ratio": 0.0,
        "project_specificity_score": f64::from(score) / 100.0,
        "template_leakage_count": 0,
        "blockers": source.get("blockers").or_else(|| source.get("gaps")).cloned().unwrap_or_else(|| json!([])),
        "warnings": [],
        "artifact_locale": locale,
    })
}

fn task_ids(tasks: &[ProgramTask]) -> Vec<String> {
    tasks.iter().map(|task| task.task_id.clone()).collect()
}

fn merged_source_refs(left: &Value, right: &Value) -> Vec<String> {
    sorted_unique(
        string_array(left.get("source_refs"))
            .into_iter()
            .chain(string_array(right.get("source_refs")))
            .collect(),
    )
}

fn stage_report_success(value: &Value) -> bool {
    matches!(
        value.get("status").and_then(Value::as_str),
        Some("success") | Some("passed")
    ) || value
        .get("blockers")
        .and_then(Value::as_array)
        .map(Vec::is_empty)
        .unwrap_or(false)
}

fn blockers_is_empty(value: &Value) -> bool {
    value
        .get("blockers")
        .or_else(|| value.get("gaps"))
        .or_else(|| value.get("blocking_issues"))
        .and_then(Value::as_array)
        .map(Vec::is_empty)
        .unwrap_or(true)
}

fn protocol_issue_collection_is_non_empty(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(items)) => !items.is_empty(),
        Some(Value::Number(number)) => number.as_u64().unwrap_or(0) > 0,
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => {
            let value = value.trim();
            !value.is_empty() && value != "0"
        }
        _ => false,
    }
}

fn value_array(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| match item {
            Value::String(text) => Some(text.trim().to_string()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .filter(|item| !item.is_empty())
        .collect()
}

fn non_empty_strings(values: Vec<String>) -> Option<Vec<String>> {
    let values = values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>();
    (!values.is_empty()).then_some(values)
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn non_empty_or(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn localized_owned(locale: ArtifactLocale, zh_cn: String, en_us: String) -> String {
    localized_text(locale, &zh_cn, &en_us).to_string()
}

fn work_unit_status_message(
    step: u32,
    status: WorkUnitRunStatus,
    locale: ArtifactLocale,
) -> String {
    let (zh_cn, en_us) = match status {
        WorkUnitRunStatus::Unavailable => (
            format!("步骤{step:02}的工作单元执行器不可用。"),
            format!("The work-unit executor for Step{step:02} is unavailable."),
        ),
        WorkUnitRunStatus::Stopped => (
            format!("步骤{step:02}的工作单元已安全停止。"),
            format!("The Step{step:02} work unit stopped safely."),
        ),
        WorkUnitRunStatus::RecoveryBlocked => (
            format!("步骤{step:02}的工作单元无法安全恢复。"),
            format!("The Step{step:02} work unit cannot be recovered safely."),
        ),
        WorkUnitRunStatus::Failed => (
            format!("步骤{step:02}的工作单元执行失败。"),
            format!("The Step{step:02} work unit failed."),
        ),
        WorkUnitRunStatus::Committed | WorkUnitRunStatus::Reused => (
            format!("步骤{step:02}的工作单元已完成。"),
            format!("The Step{step:02} work unit completed."),
        ),
    };
    localized_owned(locale, zh_cn, en_us)
}

fn ensure_issue_contract(
    mut issue: Value,
    default_code: &str,
    default_return_target: &str,
    default_message: &str,
) -> Value {
    let object = ensure_object(&mut issue);
    if object
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        object.insert("code".to_string(), json!(default_code));
    }
    if object
        .get("return_target")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        object.insert("return_target".to_string(), json!(default_return_target));
    }
    if object
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
    {
        object.insert("message".to_string(), json!(default_message));
    }
    issue
}

fn asset_dimensions(asset: &Value) -> Value {
    if let Some(dimensions) = asset.get("dimensions").filter(|value| value.is_object()) {
        return dimensions.clone();
    }
    let text = string_field(asset, "dimensions");
    let parts = text
        .split(['x', 'X', '×'])
        .map(str::trim)
        .filter_map(|part| part.parse::<u64>().ok())
        .collect::<Vec<_>>();
    let (width, height) = if parts.len() == 2 {
        (parts[0], parts[1])
    } else {
        (512, 512)
    };
    json!({"width": width, "height": height})
}

fn normalize_path(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim()
        .trim_end_matches('/')
        .to_string()
}

fn scalar_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(|item| match item {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn is_empty_object(value: &Value) -> bool {
    value.as_object().map(Map::is_empty).unwrap_or(false)
}

fn review_is_blocked(value: &Value) -> bool {
    matches!(
        value.get("review_status").and_then(Value::as_str),
        Some("blocked")
    ) || matches!(
        value.get("verdict").and_then(Value::as_str),
        Some("BLOCKED")
    )
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value.as_object_mut().unwrap()
}

fn ensure_stage(actual: u32, expected: u32) -> AdmResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(AdmError::new(format!(
            "generator expected stage {expected:02}, got {actual:02}"
        )))
    }
}

fn to_json_value<T: Serialize>(value: &T) -> AdmResult<Value> {
    serde_json::to_value(value)
        .map_err(|error| AdmError::new(format!("failed to serialize step08-14 JSON: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::parse_design_text;
    use crate::stages::step03_06::Step04OutputGenerator;
    use crate::work_units::{
        WorkUnitExecutionResult, WorkUnitJournalRecord, WorkUnitReconcileDecision,
    };
    use adm_new_foundation::new_stable_id;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct StopAfterFirstExecutor {
        stop: WorkUnitStopToken,
        count: AtomicUsize,
    }

    impl WorkUnitExecutor for StopAfterFirstExecutor {
        fn execute(&self, request: &WorkUnitRequest) -> AdmResult<WorkUnitExecutionResult> {
            self.count.fetch_add(1, Ordering::AcqRel);
            self.stop.request_stop();
            Ok(WorkUnitExecutionResult::verified(
                string_array(request.payload.get("output_files")),
                Vec::new(),
                vec![json!({"status": "passed"})],
                json!({"mode": "test"}),
            ))
        }

        fn reconcile(
            &self,
            _request: &WorkUnitRequest,
            _record: &WorkUnitJournalRecord,
        ) -> AdmResult<WorkUnitReconcileDecision> {
            Ok(WorkUnitReconcileDecision::Verified)
        }
    }

    #[derive(Debug, Clone)]
    struct PersistedExecutionObjectExecutor;

    impl WorkUnitExecutor for PersistedExecutionObjectExecutor {
        fn execute(&self, request: &WorkUnitRequest) -> AdmResult<WorkUnitExecutionResult> {
            let output_refs = match request.kind {
                WorkUnitKind::Development => string_array(request.payload.get("output_files")),
                WorkUnitKind::Art => vec![string_field(&request.payload, "unity_target_path")],
            };
            Ok(WorkUnitExecutionResult::verified(
                output_refs.clone(),
                output_refs,
                vec![json!({"id": "real_execution_evidence", "status": "passed"})],
                json!({
                    "execution_object_id": format!("EO-REAL-{}", request.task_id),
                    "execution_object_state": "verified",
                    "side_effects_committed": true,
                }),
            ))
        }

        fn reconcile(
            &self,
            _request: &WorkUnitRequest,
            _record: &WorkUnitJournalRecord,
        ) -> AdmResult<WorkUnitReconcileDecision> {
            Ok(WorkUnitReconcileDecision::Verified)
        }
    }

    fn sample_parsed() -> ParsedDesignSource {
        ParsedDesignSource {
            source: "design.md".to_string(),
            source_path: "design.md".to_string(),
            source_sha256: "sha".to_string(),
            source_size_bytes: 10,
            source_line_count: 3,
            parsed_at: now_iso(),
            layers: Vec::new(),
            selections: Vec::new(),
            raw_text: "# Playable\n".to_string(),
            source_package: "devflow_Design".to_string(),
            source_input_type: "Design".to_string(),
            design_summary: json!({}),
            structured_source_warning: None,
        }
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(new_stable_id(name).unwrap())
    }

    fn offline_generator_for_step(step: u32) -> Box<dyn StageOutputGenerator> {
        match step {
            STEP11 => Box::new(Step11OutputGenerator::offline()),
            STEP12 => Box::new(Step12OutputGenerator::offline()),
            _ => generator_for_step(step).unwrap(),
        }
    }

    fn verified_generator_for_step(step: u32) -> Box<dyn StageOutputGenerator> {
        match step {
            STEP11 => Box::new(Step11OutputGenerator::new(
                Some(Arc::new(PersistedExecutionObjectExecutor)),
                None,
                WorkUnitStopToken::default(),
            )),
            STEP12 => Box::new(Step12OutputGenerator::new(
                Some(Arc::new(PersistedExecutionObjectExecutor)),
                None,
                WorkUnitStopToken::default(),
            )),
            _ => generator_for_step(step).unwrap(),
        }
    }

    fn write_playable_bundle(root: &Path, include_ui: bool) {
        let contract_dir = root.join("stage_02").join("playable_contracts");
        fs::create_dir_all(&contract_dir).unwrap();
        write_json(
            &contract_dir.join("core_playable_contract.json"),
            &json!({"action_verbs": ["primary_action"]}),
        )
        .unwrap();
        if include_ui {
            write_json(
                &contract_dir.join("ui_flow_contract.json"),
                &json!({"screens": [{"screen_id": "game_hud"}]}),
            )
            .unwrap();
        }
        write_json(
            &contract_dir.join("playable_acceptance_contract.json"),
            &json!({"checks": [{"id": "scene_loads"}]}),
        )
        .unwrap();
        write_json(
            &contract_dir.join("scene_bootstrap_contract.json"),
            &json!({"entry_scene": "Assets/Scenes/DemoScene.unity"}),
        )
        .unwrap();
    }

    #[test]
    fn step08_preserves_structured_action_ids_labels_and_bindings() {
        let contracts = BTreeMap::from([(
            "core_playable_contract".to_string(),
            json!({
                "action_verbs": [{
                    "action_id": "action_collect",
                    "display_name": "收集资源",
                    "input_binding": "PrimaryAction",
                    "source_refs": ["design_entities/collect_action"]
                }]
            }),
        )]);

        let requirements = input_runtime_requirements(&contracts, ArtifactLocale::ZhCn);

        assert_eq!(requirements["actions"][0]["action_id"], "action_collect");
        assert_eq!(requirements["actions"][0]["label"], "收集资源");
        assert_eq!(requirements["actions"][0]["binding"], "PrimaryAction");
        assert_eq!(requirements["tasks"][0]["action_id"], "action_collect");
        assert!(
            requirements["actions"][0]["source_refs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|source| source == "design_entities/collect_action")
        );
    }

    #[test]
    fn step08_blocks_when_program_capability_handoff_is_missing() {
        let root = temp_root("step08_missing_program_capability");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        fs::remove_file(root.join("stage_03/program_capability_contract.json")).unwrap();

        let result = Step08OutputGenerator
            .generate(8, &sample_parsed(), &root.join("stage_08"), &json!({}))
            .unwrap();

        assert_eq!(result["status"], "blocked");
        assert!(
            result["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| {
                    issue["code"] == "STEP08_PROGRAM_CAPABILITY_CONTRACT_MISSING"
                        && issue["return_target"] == "stage_03"
                })
        );
        let _ = fs::remove_dir_all(root);
    }

    fn write_stage3_5(root: &Path) {
        write_json(
            &root
                .join("stage_03")
                .join("program_requirements_contract.json"),
            &json!({
                "requirements": [{
                    "id": "REQ-001",
                    "requirement": "Implement RuntimeBootstrap.",
                    "phase": "core_playable",
                    "source_refs": ["stage_02/playable_contracts/core_playable_contract.json"],
                    "acceptance": "Runtime starts.",
                    "outputs": ["Assets/Scripts/Runtime/RuntimeBootstrap.cs"]
                }]
            }),
        )
        .unwrap();
        write_json(
            &root.join("stage_03").join("program_structure_spec.json"),
            &json!({"allowed_roots": ["Assets/Scripts/"], "system_path_map": []}),
        )
        .unwrap();
        write_json(
            &root
                .join("stage_03")
                .join("program_capability_contract.json"),
            &json!({
                "project_signature": "test-project-signature",
                "capabilities": [{
                    "capability_id": "RuntimeBootstrap",
                    "program_class": "RuntimeBootstrap",
                    "source_semantic_id": "runtime_bootstrap"
                }],
                "blocking_issues": [],
                "artifact_locale": "zh-CN"
            }),
        )
        .unwrap();
        write_json(
            &root
                .join("stage_03")
                .join("program_semantic_coverage_report.json"),
            &json!({
                "status": "passed",
                "project_signature": "test-project-signature",
                "source_contracts": [
                    "stage_02/project_dna_contract.json",
                    "stage_03/program_capability_contract.json"
                ],
                "coverage_items": [{"capability_id": "RuntimeBootstrap"}],
                "missing_program_capabilities": [],
                "blockers": [],
                "artifact_locale": "zh-CN"
            }),
        )
        .unwrap();
        write_json(
            &root.join("stage_05").join("program_ai_review_report.json"),
            &json!({"review_status": "passed", "blockers": []}),
        )
        .unwrap();
        write_json(
            &root
                .join("stage_05")
                .join("program_semantic_review_report.json"),
            &json!({"status": "passed", "review_status": "passed", "blockers": [], "artifact_locale": "zh-CN"}),
        )
        .unwrap();
    }

    fn write_style_and_assets(root: &Path) {
        write_json(
            &root.join("stage_07").join("style_application_contract.json"),
            &json!({"schema_version": "1.0", "status": "approved", "selected_style_id": "style_test"}),
        )
        .unwrap();
        write_json(
            &root.join("stage_07").join("style_confirmation.json"),
            &json!({"schema_version": 1, "status": "approved", "selected_style_id": "style_test"}),
        )
        .unwrap();
        write_json(
            &root.join("stage_04").join("asset_registry.json"),
            &json!([{
                "asset_id": "ASSET-HUD-001",
                "name": "HUD panel sprite",
                "asset_type": "ui",
                "priority": "P0",
                "complexity": "m",
                "unity_target_path": "Assets/AutoDesign/Art/Source/ASSET_HUD_001.png",
                "consumer_system": "UIController",
                "mount_point": "Canvas/HUD",
                "source_refs": ["stage_02/playable_contracts/ui_flow_contract.json"]
            }]),
        )
        .unwrap();
    }

    fn unity_context_inputs(root: &Path, locale: ArtifactLocale) -> Value {
        json!({
            "artifact_locale": locale,
            "unity_project_path": root.join("UnityProject").to_string_lossy(),
            "unity_editor_path": root.join("UnityEditor/Unity.exe").to_string_lossy(),
        })
    }

    fn write_verified_unity_evidence(root: &Path) -> Value {
        let stage13 = root.join("stage_13");
        let project = root.join("UnityProject");
        let editor = root.join("UnityEditor/Unity.exe");
        let scene_path = "Assets/Scenes/DemoScene.unity";
        let unity_inputs = unity_context_inputs(root, ArtifactLocale::ZhCn);
        let request_fingerprint = unity_request_fingerprint(
            &sample_parsed(),
            &stage13,
            ArtifactLocale::ZhCn,
            &project.to_string_lossy(),
            &editor.to_string_lossy(),
        )
        .unwrap();
        fs::create_dir_all(project.join("Assets/Scenes")).unwrap();
        fs::create_dir_all(project.join("ProjectSettings")).unwrap();
        fs::create_dir_all(editor.parent().unwrap()).unwrap();
        fs::write(&editor, b"test editor fixture").unwrap();
        fs::write(project.join(scene_path), "%YAML 1.1\n").unwrap();
        fs::write(
            project.join("ProjectSettings/EditorBuildSettings.asset"),
            "build settings",
        )
        .unwrap();
        write_json(
            &stage13.join("scene_assembly_report.json"),
            &json!({
                "schema_version": 1,
                "generated_at": now_iso(),
                "status": "success",
                "scene_path": scene_path,
                "project_path": project.to_string_lossy(),
                "unity_editor_path": editor.to_string_lossy(),
                "request_fingerprint": request_fingerprint,
                "unity_attempted": true,
                "unity_result": {"id": "scene_assembly_unity_execute", "status": "passed", "errors": []},
                "execution_object_id": "EO-SCENE-REAL-001",
                "execution_object_state": "verified",
                "changed_files": [scene_path],
                "materialized_files": [scene_path],
                "unexpected_changes": [],
                "required_files": {"Assets/Scenes/DemoScene.unity": true},
                "demo_scene_exists": true,
                "build_settings_updated": true,
                "playmode_smoke_test_passed": true,
                "visible_content_verified": true,
                "blocking_issues": [],
            }),
        )
        .unwrap();
        write_json(
            &stage13.join("playmode_smoke_test_result.json"),
            &json!({
                "schema_version": 1,
                "status": "passed",
                "scene_loads": true,
                "camera_exists": true,
                "runtime_root_exists": true,
                "canvas_ui_root_exists": true,
                "event_system_exists": true,
                "input_router_exists": true,
                "objective_tracker_exists": true,
                "game_state_exists": true,
                "visible_content_verified": true,
            }),
        )
        .unwrap();
        write_json(
            &stage13.join("build_settings_update_report.json"),
            &json!({"schema_version": 1, "status": "passed", "build_settings_updated": true}),
        )
        .unwrap();
        write_json(
            &stage13.join("changed_files_manifest.json"),
            &json!({"schema_version": 1, "changed_files": [scene_path]}),
        )
        .unwrap();
        write_json(
            &stage13.join("scene_assembly_manifest.json"),
            &json!({
                "schema_version": 1,
                "scene_path": scene_path,
                "canvas_ui_root": true,
                "event_system": true,
                "input_router": true,
                "objective_tracker": true,
            }),
        )
        .unwrap();
        write_json(
            &root.join("outputs/execution_objects/execution_objects.json"),
            &json!({
                "schema_version": 1,
                "generated_at": now_iso(),
                "updated_at": now_iso(),
                "save_id": "save-test",
                "objects": [{
                    "execution_object_id": "EO-SCENE-REAL-001",
                    "object_type": "unity_scene_assembly_batch",
                    "title": "Unity scene assembly fixture",
                    "state": "verified",
                    "created_at": now_iso(),
                    "updated_at": now_iso(),
                    "source_diagnostic_id": "",
                    "source_execution_object_id": "",
                    "prefilled_content": {},
                    "user_content": {},
                    "related_facts": {},
                    "write_scope": [scene_path, "ProjectSettings/EditorBuildSettings.asset"],
                    "submission_snapshot": null,
                    "final_submitted_content": null,
                    "confirmation_level": "destructive_confirm",
                    "impact_analysis": null,
                    "confirmation_records": [],
                    "cancellation_records": [],
                    "drift_checks": [],
                    "conflict_checks": [],
                    "execution_records": [],
                    "failure_records": [],
                    "verification_records": [{
                        "verification_record_id": "VR-SCENE-REAL-001",
                        "at": now_iso(),
                        "evidence": {
                            "request_fingerprint": request_fingerprint,
                            "scene_path": scene_path,
                            "playmode_smoke_test_passed": true,
                            "visible_content_verified": true
                        }
                    }],
                    "audit_cleanup_evidence": [],
                    "state_history": [],
                    "metadata": {
                        "request_fingerprint": request_fingerprint,
                        "project_path": project.to_string_lossy(),
                        "unity_editor_path": editor.to_string_lossy()
                    }
                }],
                "audit_cleanup_evidence": [],
                "ownership_migrations": []
            }),
        )
        .unwrap();
        unity_inputs
    }

    fn write_ready_stage13_inputs(root: &Path) {
        write_playable_bundle(root, true);
        write_stage3_5(root);
        write_style_and_assets(root);
        for stage in 8..=12 {
            verified_generator_for_step(stage)
                .generate(
                    stage,
                    &sample_parsed(),
                    &root.join(format!("stage_{stage:02}")),
                    &json!({}),
                )
                .unwrap();
        }
    }

    fn real_stage04_design() -> ParsedDesignSource {
        parse_design_text(
            r#"# Action Prototype

## Layer 2 Systems
Selected / Stable
- 玩法系统: Combat Weapon System
  目的: weapon attack damage and hit response
  依赖: combat_node

## Layer 5 Entities
Frozen / Traceable
- L5实体: Shadow Sword
  目的: schema=weapon.v1；kind=weapon；status=precise
  依赖: combat_node
- L5实体: Combat HUD
  目的: schema=ui.v1；kind=ui；status=precise
  依赖: ui_node
"#,
            "sample.md",
            "",
            None,
            None,
        )
    }

    #[test]
    fn plugin_specs_match_python_wrappers() {
        assert_eq!(step08_plugin_spec().source_groups[0].label, "program_plans");
        assert_eq!(
            step09_plugin_spec().source_groups[0].patterns[0],
            "devflow_ArtPlans_*"
        );
        assert_eq!(
            step10_plugin_spec().source_groups[0].source_ids[0],
            "Alignment"
        );
        assert_eq!(step11_plugin_spec().source_groups[0].label, "dev_execution");
        assert_eq!(
            step12_plugin_spec().source_groups[0].source_ids[0],
            "ArtProduction"
        );
        assert_eq!(
            step13_plugin_spec().source_groups[0].label,
            "scene_assembly"
        );
        assert_eq!(
            step14_plugin_spec().source_groups[0].patterns[0],
            "devflow_Integration_*"
        );
    }

    #[test]
    fn step08_writes_program_plan_and_blocks_missing_ui_contract() {
        let root = temp_root("step08_14_step08");
        write_playable_bundle(&root, false);
        write_stage3_5(&root);
        let out_dir = root.join("stage_08");

        let result = Step08OutputGenerator
            .generate(8, &sample_parsed(), &out_dir, &json!({}))
            .unwrap();
        let synthesis = read_json(&out_dir.join("ai_task_synthesis_report.json"), json!({}));
        let breakdown = read_json(&out_dir.join("program_task_breakdown.json"), json!({}));

        assert_eq!(result["status"], "blocked");
        assert!(
            synthesis["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["contract_id"] == "ui_flow_contract")
        );
        assert_eq!(
            breakdown["tasks"][0]["output_files"][0],
            "Assets/Scripts/Runtime/RuntimeBootstrap.cs"
        );
        assert!(
            out_dir
                .join("scene_assembly_task_requirements.json")
                .exists()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_offline_step11_and_step12_remain_blocked_preview_only() {
        let root = temp_root("step08_14_art_chain");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        Step08OutputGenerator
            .generate(8, &sample_parsed(), &root.join("stage_08"), &json!({}))
            .unwrap();
        Step09OutputGenerator
            .generate(9, &sample_parsed(), &root.join("stage_09"), &json!({}))
            .unwrap();
        Step10OutputGenerator
            .generate(10, &sample_parsed(), &root.join("stage_10"), &json!({}))
            .unwrap();
        Step11OutputGenerator::offline()
            .generate(11, &sample_parsed(), &root.join("stage_11"), &json!({}))
            .unwrap();
        let result12 = Step12OutputGenerator::offline()
            .generate(12, &sample_parsed(), &root.join("stage_12"), &json!({}))
            .unwrap();

        let art_contract = read_json(
            &root
                .join("stage_09")
                .join("art_production_task_contract.json"),
            json!({}),
        );
        let alignment = read_json(
            &root.join("stage_10").join("asset_alignment_report.json"),
            json!({}),
        );
        let dev = read_json(
            &root.join("stage_11").join("dev_execution_report.json"),
            json!({}),
        );
        let handoff = read_json(
            &root.join("stage_12").join("art_handoff_manifest.json"),
            json!({}),
        );

        assert_eq!(art_contract["tasks"][0]["asset_type"], "ui");
        assert_eq!(alignment["alignment_items"][0]["asset_id"], "ASSET-HUD-001");
        assert_eq!(dev["status"], "blocked");
        assert!(dev["execution_records"].as_array().unwrap().is_empty());
        assert!(
            dev["verified_execution_objects"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(!dev["preview_records"].as_array().unwrap().is_empty());
        assert_eq!(dev["preview_records"][0]["status"], "preview_only");
        assert_eq!(
            dev["preview_records"][0]["execution_object_state"],
            "offline_contract_only"
        );
        assert!(
            dev["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue["code"] == "STEP11_OFFLINE_PREVIEW_ONLY")
        );
        assert_eq!(result12["status"], "blocked");
        let production = read_json(&root.join("stage_12/art_production_report.json"), json!({}));
        assert!(production["produced_assets"].as_array().unwrap().is_empty());
        assert!(!production["preview_assets"].as_array().unwrap().is_empty());
        assert!(
            production["missing_assets"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| {
                    item["reason"]
                        .as_str()
                        .is_some_and(|value| !value.is_empty())
                        && item["fallback_policy"]
                            .as_str()
                            .is_some_and(|value| !value.is_empty())
                })
        );
        assert_eq!(production["handoff_ready"], false);
        assert!(
            production["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue["code"] == "STEP12_OFFLINE_PREVIEW_ONLY")
        );
        assert_eq!(handoff["ready_for_step13"], false);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step11_and_step12_publish_executor_owned_execution_object_ids() {
        let root = temp_root("step11_12_real_execution_object_ids");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        for stage in 8..=10 {
            generator_for_step(stage)
                .unwrap()
                .generate(
                    stage,
                    &sample_parsed(),
                    &root.join(format!("stage_{stage:02}")),
                    &json!({}),
                )
                .unwrap();
        }
        let executor: Arc<dyn WorkUnitExecutor> = Arc::new(PersistedExecutionObjectExecutor);
        let step11 =
            Step11OutputGenerator::new(Some(executor.clone()), None, WorkUnitStopToken::default())
                .generate(11, &sample_parsed(), &root.join("stage_11"), &json!({}))
                .unwrap();
        let step12 = Step12OutputGenerator::new(Some(executor), None, WorkUnitStopToken::default())
            .generate(12, &sample_parsed(), &root.join("stage_12"), &json!({}))
            .unwrap();
        let development = read_json(&root.join("stage_11/dev_execution_report.json"), json!({}));
        let art = read_json(&root.join("stage_12/art_production_report.json"), json!({}));

        assert_eq!(step11["status"], "success");
        assert_eq!(step12["status"], "success");
        assert_eq!(
            development["execution_records"][0]["execution_object_id"],
            "EO-REAL-DEV-001"
        );
        assert_eq!(
            development["verified_execution_objects"][0],
            "EO-REAL-DEV-001"
        );
        assert_eq!(
            art["produced_assets"][0]["execution_object_id"],
            "EO-REAL-ART-001"
        );
        assert_eq!(
            art["produced_assets"][0]["execution_object_state"],
            "verified"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step11_without_an_executor_reports_unavailable_instead_of_fake_success() {
        let root = temp_root("step08_14_step11_unavailable");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        for stage in 8..=10 {
            generator_for_step(stage)
                .unwrap()
                .generate(
                    stage,
                    &sample_parsed(),
                    &root.join(format!("stage_{stage:02}")),
                    &json!({}),
                )
                .unwrap();
        }

        let result = Step11OutputGenerator::default()
            .generate(11, &sample_parsed(), &root.join("stage_11"), &json!({}))
            .unwrap();
        let report = read_json(
            &root.join("stage_11").join("dev_execution_report.json"),
            json!({}),
        );

        assert_eq!(result["status"], "blocked");
        assert!(report["execution_records"].as_array().unwrap().is_empty());
        assert!(report["blockers"].as_array().unwrap().iter().any(|item| {
            item["code"] == "STEP11_WORK_UNIT_EXECUTOR_UNAVAILABLE"
                && item["unit_id"]
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("11:program:")
        }));
        assert!(!root.join("stage_11").join(".work_units").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step11_stop_at_a_work_unit_boundary_reports_stopped_not_failed() {
        let root = temp_root("step08_14_step11_stopped");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        for stage in 8..=10 {
            generator_for_step(stage)
                .unwrap()
                .generate(
                    stage,
                    &sample_parsed(),
                    &root.join(format!("stage_{stage:02}")),
                    &json!({}),
                )
                .unwrap();
        }
        let stop = WorkUnitStopToken::default();
        let executor = Arc::new(StopAfterFirstExecutor {
            stop: stop.clone(),
            count: AtomicUsize::new(0),
        });
        let result = Step11OutputGenerator::new(Some(executor.clone()), None, stop)
            .generate(11, &sample_parsed(), &root.join("stage_11"), &json!({}))
            .unwrap();
        assert_eq!(result["status"], "stopped");
        assert_eq!(result["stop_requested"], true);
        assert_eq!(result["recovery_blocked"], false);
        assert_eq!(executor.count.load(Ordering::Acquire), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step12_without_an_executor_does_not_claim_assets_were_produced() {
        let root = temp_root("step08_14_step12_unavailable");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        for stage in 8..=10 {
            generator_for_step(stage)
                .unwrap()
                .generate(
                    stage,
                    &sample_parsed(),
                    &root.join(format!("stage_{stage:02}")),
                    &json!({}),
                )
                .unwrap();
        }

        let result = Step12OutputGenerator::default()
            .generate(12, &sample_parsed(), &root.join("stage_12"), &json!({}))
            .unwrap();
        let report = read_json(
            &root.join("stage_12").join("art_production_report.json"),
            json!({}),
        );
        let quality = read_json(
            &root.join("stage_12").join("image_quality_report.json"),
            json!({}),
        );
        let semantic = read_json(
            &root
                .join("stage_12")
                .join("art_semantic_review_report.json"),
            json!({}),
        );
        let rework = read_json(
            &root.join("stage_12").join("art_rework_queue.json"),
            json!({}),
        );
        let preflight = read_json(
            &root
                .join("stage_12")
                .join("program_asset_binding_preflight.json"),
            json!({}),
        );
        let handoff = read_json(
            &root.join("stage_12").join("art_handoff_manifest.json"),
            json!({}),
        );

        assert_eq!(result["status"], "blocked");
        assert!(report["produced_assets"].as_array().unwrap().is_empty());
        assert!(!report["missing_assets"].as_array().unwrap().is_empty());
        assert!(report["blockers"].as_array().unwrap().iter().any(|item| {
            item["code"] == "STEP12_WORK_UNIT_EXECUTOR_UNAVAILABLE"
                && item["unit_id"]
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("12:art:")
        }));
        assert_eq!(quality["status"], "blocked");
        assert!(!quality["blockers"].as_array().unwrap().is_empty());
        assert_eq!(semantic["status"], "blocked");
        assert!(!semantic["rework_items"].as_array().unwrap().is_empty());
        assert!(rework["blocking_count"].as_u64().unwrap() > 0);
        assert_eq!(preflight["ready"], false);
        assert!(!preflight["blockers"].as_array().unwrap().is_empty());
        assert_eq!(handoff["ready_for_step13"], false);
        assert!(!handoff["blockers"].as_array().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn real_step04_registry_flows_through_step09_and_step10() {
        let root = temp_root("step08_14_real_step04_chain");
        let parsed = real_stage04_design();
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        Step04OutputGenerator
            .generate(4, &parsed, &root.join("stage_04"), &json!({}))
            .unwrap();
        Step08OutputGenerator
            .generate(8, &parsed, &root.join("stage_08"), &json!({}))
            .unwrap();
        let stage09 = Step09OutputGenerator
            .generate(9, &parsed, &root.join("stage_09"), &json!({}))
            .unwrap();
        let stage10 = Step10OutputGenerator
            .generate(10, &parsed, &root.join("stage_10"), &json!({}))
            .unwrap();

        assert_eq!(stage09["status"], json!("success"));
        assert!(stage09["asset_count"].as_u64().unwrap() > 0);
        assert_eq!(stage09["asset_count"], stage09["task_count"]);
        assert_eq!(stage10["status"], json!("success"));
        let alignment = read_json(
            &root.join("stage_10").join("asset_alignment_report.json"),
            json!({}),
        );
        assert!(!alignment["alignment_items"].as_array().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step13_generates_request_but_does_not_claim_success_without_unity_evidence() {
        let root = temp_root("step08_14_step13");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        for stage in 8..=12 {
            verified_generator_for_step(stage)
                .generate(
                    stage,
                    &sample_parsed(),
                    &root.join(format!("stage_{stage:02}")),
                    &json!({}),
                )
                .unwrap();
        }
        let unity_inputs = unity_context_inputs(&root, ArtifactLocale::ZhCn);
        let result = Step13OutputGenerator
            .generate(13, &sample_parsed(), &root.join("stage_13"), &unity_inputs)
            .unwrap();
        let report = read_json(
            &root.join("stage_13").join("scene_assembly_report.json"),
            json!({}),
        );

        let request = read_json(
            &root.join("stage_13").join("unity_editor_request.json"),
            json!({}),
        );
        let smoke = read_json(
            &root
                .join("stage_13")
                .join("playmode_smoke_test_result.json"),
            json!({}),
        );

        assert_eq!(result["status"], "blocked");
        assert_eq!(request["status"], "ready");
        assert_eq!(request["ready_for_execute"], true);
        assert_eq!(request["project_path"], unity_inputs["unity_project_path"]);
        assert_eq!(
            request["unity_editor_path"],
            unity_inputs["unity_editor_path"]
        );
        assert_eq!(report["project_path"], unity_inputs["unity_project_path"]);
        assert_eq!(
            report["unity_editor_path"],
            unity_inputs["unity_editor_path"]
        );
        assert_eq!(report["execution_evidence_verified"], false);
        assert_eq!(report["unity_attempted"], false);
        assert_eq!(report["static_structure"]["canvas_ui_root_exists"], false);
        assert_eq!(report["changed_files"], json!([]));
        assert_eq!(report["materialized_files"], json!([]));
        assert_eq!(report["playmode_smoke_test_passed"], false);
        assert_eq!(report["visible_content_verified"], false);
        assert_eq!(smoke["status"], "not_executed");
        let evidence_issue = report["blocking_issues"]
            .as_array()
            .unwrap()
            .iter()
            .find(|issue| issue["code"] == "STEP13_UNITY_EXECUTION_EVIDENCE_MISSING")
            .unwrap();
        assert_eq!(evidence_issue["return_target"], "stage_13");
        assert!(
            evidence_issue["message"]
                .as_str()
                .unwrap()
                .contains("真实的 Unity")
        );

        let blocked_root = temp_root("step08_14_step13_blocked");
        write_playable_bundle(&blocked_root, false);
        let blocked = Step13OutputGenerator
            .generate(
                13,
                &sample_parsed(),
                &blocked_root.join("stage_13"),
                &json!({}),
            )
            .unwrap();
        assert_eq!(blocked["status"], "blocked");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(blocked_root);
    }

    #[test]
    fn step13_missing_unity_context_blocks_without_placeholder_paths_in_both_locales() {
        for (locale, expected_project_fragment, expected_editor_fragment) in [
            (
                ArtifactLocale::ZhCn,
                "没有绑定 Unity 项目路径",
                "没有绑定 Unity 编辑器路径",
            ),
            (
                ArtifactLocale::EnUs,
                "no bound Unity project path",
                "no bound Unity editor path",
            ),
        ] {
            let root = temp_root(&format!("step13_missing_unity_context_{}", locale.as_str()));
            let inputs = json!({"artifact_locale": locale});

            let result = Step13OutputGenerator
                .generate(13, &sample_parsed(), &root.join("stage_13"), &inputs)
                .unwrap();
            let request = read_json(&root.join("stage_13/unity_editor_request.json"), json!({}));
            let report = read_json(&root.join("stage_13/scene_assembly_report.json"), json!({}));

            assert_eq!(result["status"], "blocked");
            assert_eq!(request["status"], "blocked");
            assert_eq!(request["ready_for_execute"], false);
            assert_eq!(request["project_path"], "");
            assert_eq!(request["unity_editor_path"], "");
            assert_eq!(report["project_path"], "");
            assert_eq!(report["unity_editor_path"], "");
            let issues = result["blocking_issues"].as_array().unwrap();
            let project_issue = issues
                .iter()
                .find(|issue| issue["code"] == "STEP13_UNITY_PROJECT_PATH_MISSING")
                .unwrap();
            assert!(
                project_issue["message"]
                    .as_str()
                    .unwrap()
                    .contains(expected_project_fragment)
            );
            let editor_issue = issues
                .iter()
                .find(|issue| issue["code"] == "STEP13_UNITY_EDITOR_PATH_MISSING")
                .unwrap();
            assert!(
                editor_issue["message"]
                    .as_str()
                    .unwrap()
                    .contains(expected_editor_fragment)
            );
            let serialized = serde_json::to_string(&request).unwrap();
            assert!(!serialized.contains("UnityProject"));
            assert!(!serialized.contains("\"Unity.exe\""));
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn step13_rejects_verified_evidence_from_another_unity_context() {
        let root = temp_root("step13_unity_context_mismatch");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        for stage in 8..=12 {
            verified_generator_for_step(stage)
                .generate(
                    stage,
                    &sample_parsed(),
                    &root.join(format!("stage_{stage:02}")),
                    &json!({}),
                )
                .unwrap();
        }
        let mut unity_inputs = write_verified_unity_evidence(&root);
        let other_editor = root.join("AnotherUnityEditor/Unity.exe");
        fs::create_dir_all(other_editor.parent().unwrap()).unwrap();
        fs::write(&other_editor, b"different editor fixture").unwrap();
        unity_inputs["unity_editor_path"] = json!(other_editor.to_string_lossy());

        let result = Step13OutputGenerator
            .generate(13, &sample_parsed(), &root.join("stage_13"), &unity_inputs)
            .unwrap();
        let request = read_json(&root.join("stage_13/unity_editor_request.json"), json!({}));

        assert_eq!(result["status"], "blocked");
        assert_eq!(request["ready_for_execute"], true);
        assert!(
            result["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| {
                    issue["code"] == "STEP13_UNITY_EXECUTION_EVIDENCE_CONTEXT_MISMATCH"
                        && issue["return_target"] == "stage_13"
                })
        );
        assert!(
            !result["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue["code"] == "STEP13_UNITY_EXECUTION_EVIDENCE_MISSING")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step13_and_step14_accept_correlated_unity_execution_evidence() {
        let root = temp_root("step08_14_verified_unity_evidence");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        for stage in 8..=12 {
            verified_generator_for_step(stage)
                .generate(
                    stage,
                    &sample_parsed(),
                    &root.join(format!("stage_{stage:02}")),
                    &json!({}),
                )
                .unwrap();
        }
        let unity_inputs = write_verified_unity_evidence(&root);

        let stage13 = Step13OutputGenerator
            .generate(13, &sample_parsed(), &root.join("stage_13"), &unity_inputs)
            .unwrap();
        let scene = read_json(&root.join("stage_13/scene_assembly_report.json"), json!({}));
        let config = read_json(&root.join("stage_13/scene_assembly_config.json"), json!({}));
        let request = read_json(&root.join("stage_13/unity_editor_request.json"), json!({}));
        let stage14 = Step14OutputGenerator
            .generate(14, &sample_parsed(), &root.join("stage_14"), &json!({}))
            .unwrap();

        assert_eq!(stage13["status"], "success");
        assert_eq!(scene["execution_evidence_verified"], true);
        assert_eq!(scene["unity_attempted"], true);
        assert_eq!(
            scene["changed_files"],
            json!(["Assets/Scenes/DemoScene.unity"])
        );
        assert!(!string_field(&scene, "request_fingerprint").is_empty());
        assert_eq!(scene["request_fingerprint"], request["request_fingerprint"]);
        assert_eq!(scene["request_fingerprint"], config["request_fingerprint"]);
        assert_eq!(stage14["status"], "success");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step13_rejects_legacy_scene_evidence_without_request_fingerprint() {
        let root = temp_root("step13_legacy_scene_evidence");
        write_ready_stage13_inputs(&root);
        let unity_inputs = write_verified_unity_evidence(&root);
        let scene_path = root.join("stage_13/scene_assembly_report.json");
        let mut scene = read_json(&scene_path, json!({}));
        scene.as_object_mut().unwrap().remove("request_fingerprint");
        write_json(&scene_path, &scene).unwrap();

        let result = Step13OutputGenerator
            .generate(13, &sample_parsed(), &root.join("stage_13"), &unity_inputs)
            .unwrap();
        let request = read_json(&root.join("stage_13/unity_editor_request.json"), json!({}));
        let report = read_json(&scene_path, json!({}));

        assert_eq!(result["status"], "blocked");
        assert!(
            result["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue["code"] == "STEP13_UNITY_REQUEST_FINGERPRINT_MISSING")
        );
        assert!(!string_field(&request, "request_fingerprint").is_empty());
        assert_eq!(
            request["request_fingerprint"],
            report["request_fingerprint"]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step13_rejects_scene_report_referencing_the_wrong_execution_object() {
        let root = temp_root("step13_wrong_scene_execution_object");
        write_ready_stage13_inputs(&root);
        let unity_inputs = write_verified_unity_evidence(&root);
        let scene_path = root.join("stage_13/scene_assembly_report.json");
        let mut scene = read_json(&scene_path, json!({}));
        scene["execution_object_id"] = json!("EO-SCENE-NOT-IN-STORE");
        write_json(&scene_path, &scene).unwrap();

        let result = Step13OutputGenerator
            .generate(13, &sample_parsed(), &root.join("stage_13"), &unity_inputs)
            .unwrap();

        assert_eq!(result["status"], "blocked");
        assert!(
            result["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| issue["code"] == "STEP13_SCENE_EXECUTION_OBJECT_MISSING")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step13_rejects_execution_object_with_the_wrong_request_fingerprint() {
        let root = temp_root("step13_wrong_execution_object_fingerprint");
        write_ready_stage13_inputs(&root);
        let unity_inputs = write_verified_unity_evidence(&root);
        let store_path = root.join("outputs/execution_objects/execution_objects.json");
        let mut store = read_json(&store_path, json!({}));
        store["objects"][0]["metadata"]["request_fingerprint"] = json!("wrong-fingerprint");
        write_json(&store_path, &store).unwrap();

        let result = Step13OutputGenerator
            .generate(13, &sample_parsed(), &root.join("stage_13"), &unity_inputs)
            .unwrap();

        assert_eq!(result["status"], "blocked");
        assert!(
            result["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| {
                    issue["code"] == "STEP13_SCENE_EXECUTION_OBJECT_FINGERPRINT_MISMATCH"
                })
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step13_rejects_execution_object_verification_without_matching_fingerprint() {
        let root = temp_root("step13_wrong_verification_fingerprint");
        write_ready_stage13_inputs(&root);
        let unity_inputs = write_verified_unity_evidence(&root);
        let store_path = root.join("outputs/execution_objects/execution_objects.json");
        let mut store = read_json(&store_path, json!({}));
        store["objects"][0]["verification_records"][0]["evidence"]["request_fingerprint"] =
            json!("wrong-verification-fingerprint");
        write_json(&store_path, &store).unwrap();

        let result = Step13OutputGenerator
            .generate(13, &sample_parsed(), &root.join("stage_13"), &unity_inputs)
            .unwrap();

        assert_eq!(result["status"], "blocked");
        assert!(
            result["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| {
                    issue["code"]
                        == "STEP13_SCENE_EXECUTION_OBJECT_VERIFICATION_FINGERPRINT_MISMATCH"
                })
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step14_rechecks_execution_object_state_after_step13_success() {
        let root = temp_root("step14_rechecks_execution_object");
        write_ready_stage13_inputs(&root);
        let unity_inputs = write_verified_unity_evidence(&root);
        let stage13 = Step13OutputGenerator
            .generate(13, &sample_parsed(), &root.join("stage_13"), &unity_inputs)
            .unwrap();
        assert_eq!(stage13["status"], "success");

        let store_path = root.join("outputs/execution_objects/execution_objects.json");
        let mut store = read_json(&store_path, json!({}));
        store["objects"][0]["state"] = json!("execution_failed");
        write_json(&store_path, &store).unwrap();

        let stage14 = Step14OutputGenerator
            .generate(14, &sample_parsed(), &root.join("stage_14"), &json!({}))
            .unwrap();

        assert_eq!(stage14["status"], "blocked");
        assert!(
            stage14["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| {
                    issue["code"] == "STEP14_SCENE_EXECUTION_OBJECT_STATE_NOT_VERIFIED"
                })
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step14_rejects_stage13_without_verified_unity_evidence() {
        let root = temp_root("step08_14_step14");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        for stage in 8..=13 {
            verified_generator_for_step(stage)
                .generate(
                    stage,
                    &sample_parsed(),
                    &root.join(format!("stage_{stage:02}")),
                    &json!({}),
                )
                .unwrap();
        }
        let result = Step14OutputGenerator
            .generate(14, &sample_parsed(), &root.join("stage_14"), &json!({}))
            .unwrap();
        let integration = read_json(
            &root
                .join("stage_14")
                .join("integration_validation_report.json"),
            json!({}),
        );

        assert_eq!(result["status"], "blocked");
        assert_eq!(integration["unity_execution_evidence_verified"], false);
        assert!(
            integration["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(
                    |issue| issue["code"] == "STEP14_UNITY_EXECUTION_EVIDENCE_NOT_VERIFIED"
                        && issue["return_target"] == "stage_13"
                )
        );
        assert!(
            integration["validation_levels"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["level"] == 5)
        );

        let blocker = completed_with_review_blocker(
            &json!({"steps": {"13": {"status": "completed_with_review"}}}),
            false,
        )
        .unwrap();
        assert_eq!(blocker["blocked_step"], 13);
        let mut partial = json!({"status": "blocked", "blocking_issues": [{"code": "MISSING"}]});
        apply_step14_standalone_metadata(
            &mut partial,
            &json!({"standalone_mode": "standalone_partial", "artifacts_source_version": "v1"}),
        );
        assert_eq!(partial["status"], "blocked");
        assert!(partial.get("standalone_warnings").is_none());
        assert_eq!(partial["validation_scope"], "standalone");

        let mut review_only = json!({"status": "blocked", "blocking_issues": []});
        apply_step14_standalone_metadata(
            &mut review_only,
            &json!({"standalone_mode": "standalone_partial", "artifacts_source_version": "v1"}),
        );
        assert_eq!(review_only["status"], "completed_with_review");
        assert_eq!(review_only["standalone_warnings"], json!([]));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step08_to_step14_default_artifact_locale_is_chinese() {
        let root = temp_root("step08_14_zh_cn_artifacts");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);

        for stage in 8..=13 {
            offline_generator_for_step(stage)
                .generate(
                    stage,
                    &sample_parsed(),
                    &root.join(format!("stage_{stage:02}")),
                    &json!({}),
                )
                .unwrap();
        }
        let stage14 = Step14OutputGenerator
            .generate(14, &sample_parsed(), &root.join("stage_14"), &json!({}))
            .unwrap();

        let step08_note = fs::read_to_string(root.join("stage_08/TEMPLATE_NOTE.md")).unwrap();
        let art_contract = read_json(
            &root.join("stage_09/art_production_task_contract.json"),
            json!({}),
        );
        let semantic_alignment = read_json(
            &root.join("stage_10/semantic_alignment_report.json"),
            json!({}),
        );
        let dev_report = read_json(&root.join("stage_11/dev_execution_report.json"), json!({}));
        let slice_report = read_json(
            &root.join("stage_12/sprite_slice_result_manifest.json"),
            json!({}),
        );
        let scene_markdown = fs::read_to_string(root.join("stage_13/scene_assembly.md")).unwrap();
        let integration = read_json(
            &root.join("stage_14/integration_validation_report.json"),
            json!({}),
        );

        assert!(step08_note.contains("模板处理说明"));
        assert!(
            art_contract["tasks"][0]["generation_prompt"]
                .as_str()
                .unwrap()
                .contains("创建资源")
        );
        assert!(
            art_contract["tasks"][0]["negative_prompt"]
                .as_str()
                .unwrap()
                .contains("避免水印")
        );
        assert!(semantic_alignment["alignment_checks"].is_array());
        assert_eq!(dev_report["status"], "blocked");
        assert!(
            dev_report["execution_records"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            dev_report["preview_records"][0]["execution_object_state"],
            "offline_contract_only"
        );
        assert!(
            dev_report["verified_execution_objects"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            dev_report["parallel_execution"]["reason"]
                .as_str()
                .unwrap()
                .contains("串行提交")
        );
        assert!(
            slice_report["policy"]
                .as_str()
                .unwrap()
                .contains("不会通过 Python")
        );
        assert!(scene_markdown.contains("场景组装"));
        assert!(integration["environment_issues"].is_array());
        assert!(integration["checks"].is_array());
        assert!(integration["validation_levels"].as_array().unwrap().len() >= 4);
        assert!(stage14["environment_issues"].is_array());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step08_to_step14_en_us_preserves_english_extension_and_issue_contracts() {
        let root = temp_root("step08_14_en_us_artifacts");
        let locale_inputs = json!({"artifact_locale": "en-US"});
        let result = Step08OutputGenerator
            .generate(8, &sample_parsed(), &root.join("stage_08"), &locale_inputs)
            .unwrap();
        let synthesis = read_json(
            &root.join("stage_08/ai_task_synthesis_report.json"),
            json!({}),
        );
        let blocker = &synthesis["blockers"][0];
        let note = fs::read_to_string(root.join("stage_08/TEMPLATE_NOTE.md")).unwrap();
        let tasks = art_tasks_from_registry(
            vec![json!({
                "asset_id": "ASSET-001",
                "asset_type": "ui",
                "source_refs": ["stage_04/asset_registry.json"],
            })],
            &json!({"selected_style_id": "style_test"}),
            ArtifactLocale::EnUs,
        );
        let zh_tasks = art_tasks_from_registry(
            vec![json!({
                "asset_id": "ASSET-001",
                "asset_type": "ui",
                "source_refs": ["stage_04/asset_registry.json"],
            })],
            &json!({"selected_style_id": "style_test"}),
            ArtifactLocale::ZhCn,
        );
        let blocked13 = Step13OutputGenerator
            .generate(13, &sample_parsed(), &root.join("stage_13"), &locale_inputs)
            .unwrap();

        assert_eq!(result["status"], "blocked");
        assert!(
            blocker["message"]
                .as_str()
                .unwrap()
                .starts_with("Step08 requires")
        );
        assert!(blocker["code"].as_str().unwrap().starts_with("STEP08_"));
        assert_eq!(blocker["return_target"], "stage_02");
        assert!(note.contains("# Template Note"));
        assert!(tasks[0].title.contains("asset"));
        assert!(tasks[0].acceptance.contains("produced and mounted"));
        assert!(tasks[0].generation_prompt.starts_with("Create"));
        assert!(tasks[0].negative_prompt.starts_with("Avoid"));
        assert_eq!(tasks[0].task_id, zh_tasks[0].task_id);
        assert_eq!(tasks[0].asset_id, zh_tasks[0].asset_id);
        assert_eq!(tasks[0].asset_type, zh_tasks[0].asset_type);
        assert_eq!(tasks[0].unity_target_path, zh_tasks[0].unity_target_path);
        assert_ne!(tasks[0].generation_prompt, zh_tasks[0].generation_prompt);
        assert!(
            blocked13["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .all(|issue| issue["code"].is_string() && issue["return_target"].is_string())
        );

        let evidence_root = temp_root("step08_14_en_us_missing_unity_evidence");
        write_playable_bundle(&evidence_root, true);
        write_stage3_5(&evidence_root);
        write_style_and_assets(&evidence_root);
        for stage in 8..=12 {
            verified_generator_for_step(stage)
                .generate(
                    stage,
                    &sample_parsed(),
                    &evidence_root.join(format!("stage_{stage:02}")),
                    &locale_inputs,
                )
                .unwrap();
        }
        for step in [STEP11, STEP12] {
            let requests =
                work_unit_requests_for_stage(&evidence_root.join(format!("stage_{step:02}")), step)
                    .unwrap();
            assert!(!requests.is_empty());
            assert!(
                requests
                    .iter()
                    .all(|request| { request.payload["artifact_locale"] == json!("en-US") })
            );
        }
        let en_unity_inputs = unity_context_inputs(&evidence_root, ArtifactLocale::EnUs);
        let en_stage13 = Step13OutputGenerator
            .generate(
                13,
                &sample_parsed(),
                &evidence_root.join("stage_13"),
                &en_unity_inputs,
            )
            .unwrap();
        let evidence_issue = en_stage13["blocking_issues"]
            .as_array()
            .unwrap()
            .iter()
            .find(|issue| issue["code"] == "STEP13_UNITY_EXECUTION_EVIDENCE_MISSING")
            .unwrap();
        assert!(
            evidence_issue["message"]
                .as_str()
                .unwrap()
                .starts_with("The Unity Editor request was generated")
        );
        assert_eq!(evidence_issue["return_target"], "stage_13");
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(evidence_root);
    }

    #[test]
    fn step08_to_step14_registry_artifacts_match_declared_schemas() {
        let root = temp_root("step08_14_registry_schemas");
        write_playable_bundle(&root, true);
        write_json(
            &root.join("stage_02/playable_contracts/playable_acceptance_contract.json"),
            &json!({
                "schema_version": "1.0",
                "editmode_checks": [],
                "playmode_checks": [{
                    "check_id": "scene_loads",
                    "start_scene": "Assets/Scenes/DemoScene.unity",
                    "steps": ["load_scene"],
                    "expected_visible_text_or_object": "RuntimeRoot",
                    "expected_state_change": "scene_loaded",
                    "timeout_seconds": 30,
                    "blocking": true
                }],
                "visual_checks": [],
                "interaction_checks": [],
                "data_progression_checks": [],
                "build_checks": [],
                "artifact_locale": "zh-CN"
            }),
        )
        .unwrap();
        write_stage3_5(&root);
        write_style_and_assets(&root);
        for stage in 8..=12 {
            offline_generator_for_step(stage)
                .generate(
                    stage,
                    &sample_parsed(),
                    &root.join(format!("stage_{stage:02}")),
                    &json!({}),
                )
                .unwrap();
        }
        let unity_inputs = write_verified_unity_evidence(&root);
        Step13OutputGenerator
            .generate(13, &sample_parsed(), &root.join("stage_13"), &unity_inputs)
            .unwrap();
        Step14OutputGenerator
            .generate(14, &sample_parsed(), &root.join("stage_14"), &json!({}))
            .unwrap();

        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let registry = read_json(
            &repository_root.join("pipeline/artifact_layer/registry.json"),
            json!({}),
        );
        let mut checked = 0usize;
        let mut locale_mismatches = Vec::new();
        for artifact in registry["artifacts"].as_array().unwrap() {
            let Some(stage) = artifact.get("stage").and_then(Value::as_u64) else {
                continue;
            };
            if !(8..=14).contains(&stage) {
                continue;
            }
            for schema_ref in artifact["schema_refs"].as_array().into_iter().flatten() {
                let contract_ref = schema_ref["path"].as_str().unwrap();
                let Some(stage_relative) = contract_ref.strip_prefix("outputs/artifacts/") else {
                    // Execution-object persistence is owned by the application layer and
                    // has its own integration tests; this test covers stage generators.
                    continue;
                };
                let contract_path = root.join(stage_relative);
                let schema_path = repository_root.join(schema_ref["schema"].as_str().unwrap());
                assert!(
                    contract_path.is_file(),
                    "registered Step{stage:02} artifact is missing: {contract_ref}"
                );
                let errors =
                    adm_new_contracts::schema::validate_contract_file(&contract_path, &schema_path)
                        .unwrap();
                assert!(
                    errors.is_empty(),
                    "{contract_ref} does not match {}: {errors:?}",
                    schema_ref["schema"].as_str().unwrap()
                );
                let contract = read_json(&contract_path, json!({}));
                if contract.get("artifact_locale").and_then(Value::as_str) != Some("zh-CN") {
                    locale_mismatches.push(contract_ref.to_string());
                }
                checked += 1;
            }
        }
        assert!(checked >= 59, "expected the complete Step08-14 schema set");
        assert!(
            locale_mismatches.is_empty(),
            "registered artifacts have no matching locale marker: {locale_mismatches:?}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step10_blocks_empty_program_tasks_without_fabricating_dev_001() {
        let root = temp_root("step10_empty_program_tasks");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        Step08OutputGenerator
            .generate(8, &sample_parsed(), &root.join("stage_08"), &json!({}))
            .unwrap();
        Step09OutputGenerator
            .generate(9, &sample_parsed(), &root.join("stage_09"), &json!({}))
            .unwrap();
        let breakdown_path = root.join("stage_08/program_task_breakdown.json");
        let mut breakdown = read_json(&breakdown_path, json!({}));
        breakdown["tasks"] = json!([]);
        write_json(&breakdown_path, &breakdown).unwrap();

        let result = Step10OutputGenerator
            .generate(10, &sample_parsed(), &root.join("stage_10"), &json!({}))
            .unwrap();
        let alignment = read_json(
            &root.join("stage_10/asset_alignment_report.json"),
            json!({}),
        );
        let mount = read_json(
            &root.join("stage_10/mount_readiness_summary.json"),
            json!({}),
        );
        let semantic = read_json(
            &root.join("stage_10/semantic_alignment_report.json"),
            json!({}),
        );
        let coverage = read_json(
            &root.join("stage_10/semantic_coverage_matrix.json"),
            json!({}),
        );

        assert_eq!(result["status"], "blocked");
        assert!(alignment["gaps"].as_array().unwrap().iter().any(|gap| {
            gap["code"] == "STEP10_PROGRAM_TASKS_MISSING" && gap["return_target"] == "stage_08"
        }));
        assert!(
            alignment["gaps"]
                .as_array()
                .unwrap()
                .iter()
                .any(|gap| { gap["code"] == "STEP10_PROGRAM_TASK_MAPPING_MISSING" })
        );
        assert_eq!(alignment["alignment_items"][0]["program_task_id"], "");
        assert!(
            !serde_json::to_string(&alignment)
                .unwrap()
                .contains("DEV-001")
        );
        assert_eq!(mount["ready"], false);
        assert_eq!(mount["status"], "blocked");
        assert_eq!(semantic["status"], "blocked");
        assert!(!semantic["blockers"].as_array().unwrap().is_empty());
        assert_eq!(coverage["status"], "blocked");
        assert_eq!(coverage["valid"], false);
        assert!(!coverage["blockers"].as_array().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step10_reports_missing_alignment_fields_in_both_locales() {
        for locale in [ArtifactLocale::ZhCn, ArtifactLocale::EnUs] {
            let root = temp_root(&format!("step10_missing_fields_{}", locale.as_str()));
            write_playable_bundle(&root, true);
            write_stage3_5(&root);
            write_style_and_assets(&root);
            Step08OutputGenerator
                .generate(
                    8,
                    &sample_parsed(),
                    &root.join("stage_08"),
                    &json!({"artifact_locale": locale}),
                )
                .unwrap();
            write_json(
                &root.join("stage_09/art_production_task_contract.json"),
                &json!({
                    "schema_version": "1.0",
                    "source_refs": ["stage_04/asset_registry.json"],
                    "tasks": [{"program_task_id": "DEV-NOT-FOUND"}],
                    "blockers": [],
                    "artifact_locale": locale,
                }),
            )
            .unwrap();

            let result = Step10OutputGenerator
                .generate(
                    10,
                    &sample_parsed(),
                    &root.join("stage_10"),
                    &json!({"artifact_locale": locale}),
                )
                .unwrap();
            let alignment = read_json(
                &root.join("stage_10/asset_alignment_report.json"),
                json!({}),
            );
            let mount = read_json(
                &root.join("stage_10/mount_readiness_summary.json"),
                json!({}),
            );
            let semantic = read_json(
                &root.join("stage_10/semantic_alignment_report.json"),
                json!({}),
            );
            let matrix = read_json(
                &root.join("stage_10/asset_alignment_matrix.json"),
                json!({}),
            );
            let codes = alignment["gaps"]
                .as_array()
                .unwrap()
                .iter()
                .map(|gap| string_field(gap, "code"))
                .collect::<BTreeSet<_>>();

            for code in [
                "STEP10_PROGRAM_TASK_MAPPING_MISSING",
                "STEP10_ASSET_ID_MISSING",
                "STEP10_ART_TASK_ID_MISSING",
                "STEP10_UNITY_TARGET_PATH_MISSING",
                "STEP10_CONSUMER_SYSTEM_MISSING",
                "STEP10_MOUNT_POINT_MISSING",
                "STEP10_SOURCE_REFS_MISSING",
            ] {
                assert!(codes.contains(code), "missing blocker code {code}");
            }
            assert_eq!(result["status"], "blocked");
            assert_eq!(result["traceability_valid"], false);
            assert_eq!(alignment["status"], "blocked");
            assert_eq!(alignment["alignment_items"][0]["status"], "blocked");
            assert_eq!(mount["ready"], false);
            assert_eq!(mount["mount_items"][0]["target_path"], "");
            assert_eq!(mount["mount_items"][0]["fallback_policy"], "none");
            assert_eq!(mount["mount_items"][0]["fallback_policy_source"], "none");
            assert!(
                !serde_json::to_string(&mount)
                    .unwrap()
                    .contains("placeholder.png")
            );
            assert_eq!(semantic["status"], "blocked");
            assert!(!semantic["blockers"].as_array().unwrap().is_empty());
            assert_eq!(matrix["ready"], false);
            assert_eq!(matrix["artifact_locale"], locale.as_str());
            let mapping_message = alignment["gaps"]
                .as_array()
                .unwrap()
                .iter()
                .find(|gap| gap["code"] == "STEP10_PROGRAM_TASK_MAPPING_MISSING")
                .and_then(|gap| gap["message"].as_str())
                .unwrap();
            if locale == ArtifactLocale::ZhCn {
                assert!(mapping_message.contains("程序任务映射"));
            } else {
                assert!(mapping_message.contains("program-task mapping"));
            }
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn step10_derives_fallback_policy_from_upstream_mount_contract() {
        let root = temp_root("step10_upstream_fallback_policy");
        write_playable_bundle(&root, true);
        write_stage3_5(&root);
        write_style_and_assets(&root);
        Step08OutputGenerator
            .generate(8, &sample_parsed(), &root.join("stage_08"), &json!({}))
            .unwrap();
        Step09OutputGenerator
            .generate(9, &sample_parsed(), &root.join("stage_09"), &json!({}))
            .unwrap();
        write_json(
            &root.join("stage_04/unity_asset_mount_plan.json"),
            &json!({
                "schema_version": "1.0",
                "mount_items": [{
                    "asset_id": "ASSET-HUD-001",
                    "fallback_policy": "block_on_missing",
                }],
                "blockers": [],
            }),
        )
        .unwrap();

        let result = Step10OutputGenerator
            .generate(10, &sample_parsed(), &root.join("stage_10"), &json!({}))
            .unwrap();
        let mount = read_json(
            &root.join("stage_10/mount_readiness_summary.json"),
            json!({}),
        );

        assert_eq!(result["status"], "success");
        assert_eq!(mount["ready"], true);
        assert_eq!(
            mount["mount_items"][0]["fallback_policy"],
            "block_on_missing"
        );
        assert_eq!(
            mount["mount_items"][0]["fallback_policy_source"],
            "stage_04/unity_asset_mount_plan.json"
        );
        assert_ne!(
            mount["mount_items"][0]["fallback_policy"],
            "placeholder_allowed"
        );
        let _ = fs::remove_dir_all(root);
    }
}
