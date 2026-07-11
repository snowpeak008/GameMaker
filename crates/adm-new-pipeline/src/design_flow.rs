use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use adm_new_config::{get_config, load_app_config_bundle};
use adm_new_contracts::ArtifactLocale;
use adm_new_contracts::pipeline::{
    PipelineStageResult, StageContextModel, StageKind, StageSpec, StageStatus,
};
use adm_new_contracts::project::{DecisionState, NodeState, ProjectState};
use adm_new_design::data_loader::{DesignProjectData, DomainNode};
use adm_new_design::handoff::export_concept_package_from_state_with_locale;
use adm_new_design::semantic_pipeline::{
    ArchetypeCatalog, COMMON_REQUIRED_CONTRACTS, OPTIONAL_CONTRACTS,
    build_archetype_requirements_with_locale,
};
use adm_new_design::{
    DesignChecklistItemSpec, DesignEngineService, DesignNodeSpec, DesignOptionGroupSpec,
};
use adm_new_foundation::io::{read_json, write_json, write_text};
use adm_new_foundation::paths::locate_project_root;
use adm_new_foundation::{AdmError, AdmResult};
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::StageExecutor;
use crate::source::SourceGroup;
use crate::stages::step00_02::StagePluginSpec;

pub const D1_STAGE_ID: &str = "D1";
pub const D2_STAGE_ID: &str = "D2";
pub const D3_STAGE_ID: &str = "D3";
pub const D4_STAGE_ID: &str = "D4";

const DESIGN_FLOW_ENTRYPOINT: &str = "design_flow::execute_design_stage";
const PROFILE_FIELDS: &[&str] = &[
    "businessModel",
    "operationModel",
    "socialModel",
    "platformScope",
    "primaryPlatform",
    "regionScope",
    "targetScale",
    "contentRating",
    "targetSessionBand",
];

#[derive(Debug, Clone, Default)]
pub struct DesignFlowExecutor;

impl StageExecutor for DesignFlowExecutor {
    fn execute(&self, spec: &StageSpec, context: &StageContextModel) -> PipelineStageResult {
        if matches!(
            spec.stage_id.as_str(),
            D1_STAGE_ID | D2_STAGE_ID | D3_STAGE_ID | D4_STAGE_ID
        ) {
            execute_design_stage(&spec.stage_id, context)
        } else {
            PipelineStageResult {
                status: StageStatus::Failed,
                outputs: BTreeMap::new(),
                errors: vec![format!(
                    "DesignFlowExecutor cannot execute stage {}",
                    spec.stage_id
                )],
                warnings: Vec::new(),
                message: "unsupported design flow stage".to_string(),
            }
        }
    }
}

pub fn d1_plugin_spec() -> StagePluginSpec {
    design_plugin_spec(D1_STAGE_ID)
}

pub fn d2_plugin_spec() -> StagePluginSpec {
    design_plugin_spec(D2_STAGE_ID)
}

pub fn d3_plugin_spec() -> StagePluginSpec {
    design_plugin_spec(D3_STAGE_ID)
}

pub fn d4_plugin_spec() -> StagePluginSpec {
    design_plugin_spec(D4_STAGE_ID)
}

pub fn design_plugin_specs() -> Vec<StagePluginSpec> {
    vec![
        d1_plugin_spec(),
        d2_plugin_spec(),
        d3_plugin_spec(),
        d4_plugin_spec(),
    ]
}

pub fn design_stage_specs() -> Vec<StageSpec> {
    vec![
        design_stage_spec(
            D1_STAGE_ID,
            "D1 Project Portrait",
            "pipeline.step_d1_project_portrait.plugin",
            vec![],
        ),
        design_stage_spec(
            D2_STAGE_ID,
            "D2 Design Decisions",
            "pipeline.step_d2_design_decisions.plugin",
            vec![D1_STAGE_ID],
        ),
        design_stage_spec(
            D3_STAGE_ID,
            "D3 Design Validation",
            "pipeline.step_d3_design_validation.plugin",
            vec![D2_STAGE_ID],
        ),
        design_stage_spec(
            D4_STAGE_ID,
            "D4 Devflow Handoff",
            "pipeline.step_d4_devflow_handoff.plugin",
            vec![D3_STAGE_ID],
        ),
    ]
}

fn design_plugin_spec(stage_id: &'static str) -> StagePluginSpec {
    StagePluginSpec {
        stage_id,
        source_groups: Vec::<SourceGroup>::new(),
        test_mode_status: "success",
        generation_entrypoint: DESIGN_FLOW_ENTRYPOINT,
    }
}

fn design_stage_spec(
    stage_id: &str,
    title: &str,
    plugin_ref: &str,
    requires: Vec<&str>,
) -> StageSpec {
    StageSpec {
        stage_id: stage_id.to_string(),
        kind: StageKind::Design,
        number: None,
        slug: format!("stage_{}", stage_id.to_ascii_lowercase()),
        title: title.to_string(),
        requires: requires.into_iter().map(str::to_string).collect(),
        source_groups: Vec::new(),
        plugin_ref: plugin_ref.to_string(),
        metadata: BTreeMap::new(),
    }
}

pub fn execute_design_stage(stage_id: &str, context: &StageContextModel) -> PipelineStageResult {
    match try_execute_design_stage(stage_id, context) {
        Ok(result) => result,
        Err(error) => PipelineStageResult {
            status: StageStatus::Failed,
            outputs: BTreeMap::new(),
            errors: vec![error.to_string()],
            warnings: Vec::new(),
            message: "design flow stage failed".to_string(),
        },
    }
}

pub fn try_execute_design_stage(
    stage_id: &str,
    context: &StageContextModel,
) -> AdmResult<PipelineStageResult> {
    match stage_id {
        D1_STAGE_ID => execute_d1(context),
        D2_STAGE_ID => execute_d2(context),
        D3_STAGE_ID => execute_d3(context),
        D4_STAGE_ID => execute_d4(context),
        other => Err(AdmError::new(format!("unknown design flow stage: {other}"))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignDataSummary {
    pub domain_count: usize,
    pub node_count: usize,
    pub checklist_count: usize,
    pub option_group_count: usize,
    pub option_count: usize,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
    pub data_source: String,
    pub domains: Vec<DesignDomainSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DesignDomainSummary {
    pub id: String,
    pub name: String,
    pub priority: String,
    pub activation: String,
    #[serde(rename = "nodeCount")]
    pub node_count: usize,
    #[serde(rename = "checklistCount")]
    pub checklist_count: usize,
    #[serde(rename = "optionGroupCount")]
    pub option_group_count: usize,
    #[serde(rename = "optionCount")]
    pub option_count: usize,
}

pub fn summarize_design_data(data: &DesignProjectData) -> DesignDataSummary {
    let mut node_count = 0usize;
    let mut checklist_count = 0usize;
    let mut option_group_count = 0usize;
    let mut option_count = 0usize;
    let mut domains = Vec::new();
    for item in &data.domains {
        let domain_node_count = item.nodes.len();
        let mut domain_checklist_count = 0usize;
        let mut domain_option_group_count = 0usize;
        let mut domain_option_count = 0usize;
        for node in &item.nodes {
            domain_checklist_count += node.checklist.len();
            for checklist in &node.checklist {
                domain_option_group_count += checklist.option_groups.len();
                for group in &checklist.option_groups {
                    domain_option_count += group.options.len();
                }
            }
        }
        node_count += domain_node_count;
        checklist_count += domain_checklist_count;
        option_group_count += domain_option_group_count;
        option_count += domain_option_count;
        domains.push(DesignDomainSummary {
            id: item.domain.id.clone(),
            name: item.domain.name.clone(),
            priority: item.domain.priority.clone(),
            activation: item.domain.activation.clone(),
            node_count: domain_node_count,
            checklist_count: domain_checklist_count,
            option_group_count: domain_option_group_count,
            option_count: domain_option_count,
        });
    }
    DesignDataSummary {
        domain_count: data.domains.len(),
        node_count,
        checklist_count,
        option_group_count,
        option_count,
        validation_error_count: data.meta.validation_errors.len(),
        validation_warning_count: data.meta.validation_warnings.len(),
        data_source: data.meta.data_source.clone(),
        domains,
    }
}

#[derive(Debug, Clone)]
struct DesignFlowInputs {
    project_root: PathBuf,
    artifact_dir: PathBuf,
    data: DesignProjectData,
    summary: DesignDataSummary,
    engine: DesignEngineService,
}

fn load_inputs(context: &StageContextModel) -> AdmResult<DesignFlowInputs> {
    let project_root = project_root_from_context(context)?;
    let artifact_dir = artifact_dir_from_context(context, &project_root)?;
    std::fs::create_dir_all(&artifact_dir)?;
    let loader = adm_new_design::data_loader::DesignDataLoader::new(&project_root);
    let data = loader.load_project_data()?;
    let summary = summarize_design_data(&data);
    let engine = design_engine_from_data(&data);
    Ok(DesignFlowInputs {
        project_root,
        artifact_dir,
        data,
        summary,
        engine,
    })
}

fn execute_d1(context: &StageContextModel) -> AdmResult<PipelineStageResult> {
    let inputs = load_inputs(context)?;
    let artifact_locale = artifact_locale_from_context(context);
    let bundle = load_app_config_bundle(&inputs.project_root.join("settings"));
    let project_name = config_string(&bundle.app_config, "project.name", "AutoDesignMaker");
    let project_version = config_string(&bundle.app_config, "project.version", "");
    let development_path = config_string(&bundle.project_settings, "development_path", "");
    let editor_path = config_string(&bundle.project_settings, "editor_path", "");
    let portrait = json!({
        "schema_version": 1,
        "artifact_locale": artifact_locale,
        "stage_id": D1_STAGE_ID,
        "project_name": project_name,
        "project_version": project_version,
        "business_model": localized_text(artifact_locale, "内部游戏生产工具", "internal game production tool"),
        "platform": ["Windows", "Rust", "Tauri"],
        "target_audience": localized_text(artifact_locale, "游戏设计师、技术设计师与 Unity 开发者", "game designers, technical designers, and Unity developers"),
        "development_path": development_path,
        "unity_editor_path": editor_path,
        "design_domain_count": inputs.summary.domain_count,
        "design_node_count": inputs.summary.node_count,
        "checklist_count": inputs.summary.checklist_count,
        "option_group_count": inputs.summary.option_group_count,
        "option_count": inputs.summary.option_count,
        "validation_error_count": inputs.summary.validation_error_count,
        "validation_warning_count": inputs.summary.validation_warning_count,
        "data_source": inputs.summary.data_source,
    });
    let portrait_path = write_stage_json(&inputs.artifact_dir, "design_portrait.json", &portrait)?;
    let mut stage_summary = design_stage_summary(
        D1_STAGE_ID,
        localized_text(artifact_locale, "项目画像", "Project Portrait"),
        &inputs.summary,
        artifact_locale,
    );
    stage_summary.insert("portrait".to_string(), json!(path_string(&portrait_path)));
    let summary_path = write_stage_json(
        &inputs.artifact_dir,
        "design_stage_summary.json",
        &Value::Object(stage_summary),
    )?;
    let mut outputs = outputs_without_domains(&inputs.summary);
    outputs.insert(
        "designPortrait".to_string(),
        json!(path_string(&portrait_path)),
    );
    outputs.insert(
        "designStageSummary".to_string(),
        json!(path_string(&summary_path)),
    );
    outputs.insert(
        "fieldCount".to_string(),
        json!(portrait.as_object().map(Map::len).unwrap_or(0)),
    );
    Ok(success(
        outputs,
        localized_text(
            artifact_locale,
            "D1 项目画像已生成",
            "D1 project portrait generated",
        ),
    ))
}

fn execute_d2(context: &StageContextModel) -> AdmResult<PipelineStageResult> {
    let inputs = load_inputs(context)?;
    let artifact_locale = artifact_locale_from_context(context);
    let project_state = load_current_project_state(context, &inputs.project_root, &inputs.engine);
    let completion = design_completion_summary(
        &inputs.data,
        &inputs.engine,
        &project_state,
        artifact_locale,
    );
    let domains = inputs
        .summary
        .domains
        .iter()
        .map(|domain| {
            json!({
                "id": domain.id,
                "name": domain.name,
                "priority": domain.priority,
                "activation": domain.activation,
                "nodeCount": domain.node_count,
                "checklistCount": domain.checklist_count,
                "optionGroupCount": domain.option_group_count,
                "optionCount": domain.option_count,
                "status": if domain.node_count == 0 || domain.checklist_count == 0 {
                    "partial"
                } else {
                    "completed"
                },
            })
        })
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": 1,
        "artifact_locale": artifact_locale,
        "stage_id": D2_STAGE_ID,
        "coverage": if inputs.summary.validation_error_count == 0 && !domains.is_empty() { 1.0 } else { 0.0 },
        "domain_count": inputs.summary.domain_count,
        "node_count": inputs.summary.node_count,
        "checklist_count": inputs.summary.checklist_count,
        "option_group_count": inputs.summary.option_group_count,
        "option_count": inputs.summary.option_count,
        "domains": domains,
    });
    let domains_path = write_stage_json(&inputs.artifact_dir, "design_domains.json", &payload)?;
    let provenance_counts = provenance_review_counts(&completion.review_items);
    let decision_report = json!({
        "schema_version": "1.0",
        "artifact_locale": artifact_locale,
        "status": "reported",
        "stage_id": D2_STAGE_ID,
        "project_id": project_state.project_name,
        "summary": {
            "p0_total": completion.priority_counts["P0"].total,
            "p0_completed": completion.priority_counts["P0"].completed,
            "p0_blocking": completion.priority_counts["P0"].blocked,
            "ai_inferred_unconfirmed": provenance_counts.get("ai_inferred").copied().unwrap_or(0),
            "migration_inferred_unconfirmed": provenance_counts.get("migration_inferred").copied().unwrap_or(0),
        },
        "contract_targets": completion.contract_targets_json(),
        "issues": completion.all_issues(),
    });
    let report_path = write_stage_json(
        &inputs.artifact_dir,
        "design_decision_report.json",
        &decision_report,
    )?;
    let report_md_path = inputs.artifact_dir.join("design_decision_report.md");
    write_text(
        &report_md_path,
        &decision_report_markdown(&decision_report, artifact_locale),
    )?;
    let mut stage_summary = design_stage_summary(
        D2_STAGE_ID,
        localized_text(artifact_locale, "设计决策", "Design Decisions"),
        &inputs.summary,
        artifact_locale,
    );
    stage_summary.insert(
        "designDomains".to_string(),
        json!(path_string(&domains_path)),
    );
    stage_summary.insert(
        "designDecisionReport".to_string(),
        json!(path_string(&report_path)),
    );
    let summary_path = write_stage_json(
        &inputs.artifact_dir,
        "design_stage_summary.json",
        &Value::Object(stage_summary),
    )?;
    let mut outputs = BTreeMap::new();
    outputs.insert(
        "designDomains".to_string(),
        json!(path_string(&domains_path)),
    );
    outputs.insert(
        "designStageSummary".to_string(),
        json!(path_string(&summary_path)),
    );
    outputs.insert(
        "designDecisionReport".to_string(),
        json!(path_string(&report_path)),
    );
    outputs.insert(
        "designDecisionReportMarkdown".to_string(),
        json!(path_string(&report_md_path)),
    );
    outputs.insert(
        "domainCount".to_string(),
        json!(inputs.summary.domain_count),
    );
    outputs.insert("coverage".to_string(), payload["coverage"].clone());
    Ok(success(
        outputs,
        localized_text(
            artifact_locale,
            "D2 设计决策报告已生成",
            "D2 design decisions reported",
        ),
    ))
}

fn execute_d3(context: &StageContextModel) -> AdmResult<PipelineStageResult> {
    let inputs = load_inputs(context)?;
    let artifact_locale = artifact_locale_from_context(context);
    let project_state = load_current_project_state(context, &inputs.project_root, &inputs.engine);
    let completion = design_completion_summary(
        &inputs.data,
        &inputs.engine,
        &project_state,
        artifact_locale,
    );
    let archetype_requirements = build_archetype_requirements_with_locale(
        &json!(project_state.profile),
        &confirmed_option_text(&inputs.data, &project_state),
        &ArchetypeCatalog::default(),
        artifact_locale,
    );
    let incomplete_domains = inputs
        .summary
        .domains
        .iter()
        .filter(|domain| {
            domain.node_count == 0 || domain.checklist_count == 0 || domain.option_group_count == 0
        })
        .cloned()
        .collect::<Vec<_>>();
    let conflicts = if incomplete_domains.is_empty() {
        Vec::new()
    } else {
        vec![json!({
            "id": "D3-INCOMPLETE-DOMAINS",
            "severity": "warning",
            "message": localized_text(artifact_locale, "一个或多个设计域缺少节点、检查项或选项组。", "One or more domains have no nodes, checklist entries, or option groups."),
            "domainIds": incomplete_domains.iter().map(|domain| domain.id.clone()).collect::<Vec<_>>(),
        })]
    };
    let mut blocking_issues = completion.blocking_issues.clone();
    blocking_issues.extend(profile_blockers(&project_state.profile, artifact_locale));
    blocking_issues.extend(archetype_warning_blockers(
        &archetype_requirements,
        artifact_locale,
    ));
    for contract in completion.contracts.values() {
        if contract.status == "missing" || contract.status == "blocked" {
            blocking_issues.push(json!({
                "code": "CONTRACT_TARGET_NOT_FORMABLE",
                "contract": contract.contract,
                "message": if artifact_locale == ArtifactLocale::ZhCn {
                    format!("无法根据已确认的设计决策构建 {}。", contract.contract)
                } else {
                    format!("{} cannot be formed from confirmed design decisions.", contract.contract)
                },
                "required_by_steps": ["D4", "Step02", "Step13", "Step14"],
                "return_to_nodes": contract.nodes,
            }));
        }
    }
    let valid = inputs.data.meta.validation_errors.is_empty()
        && conflicts.is_empty()
        && blocking_issues.is_empty();
    let coverage = json!({
        "domains": inputs.summary.domain_count,
        "nodes": inputs.summary.node_count,
        "checklist": inputs.summary.checklist_count,
        "optionGroups": inputs.summary.option_group_count,
        "options": inputs.summary.option_count,
        "domainsWithChecklist": inputs.summary.domain_count.saturating_sub(incomplete_domains.len()),
        "coverageRatio": if inputs.summary.domain_count > 0 && incomplete_domains.is_empty() { 1.0 } else { 0.0 },
    });
    let report = json!({
        "schema_version": 1,
        "artifact_locale": artifact_locale,
        "stage_id": D3_STAGE_ID,
        "status": if valid { "passed" } else { "blocked" },
        "valid": valid,
        "validation_errors": inputs.data.meta.validation_errors,
        "validation_warnings": inputs.data.meta.validation_warnings,
        "conflicts": conflicts,
        "coverage": coverage,
        "archetype_requirements": archetype_requirements,
        "contract_completion": completion.to_json(),
        "blocking_issues": blocking_issues,
        "review_items": completion.review_items,
        "data_source": inputs.summary.data_source,
    });
    let validation_path = write_stage_json(
        &inputs.artifact_dir,
        "design_validation_report.json",
        &report,
    )?;
    let gate_path = write_stage_json(&inputs.artifact_dir, "design_gate_report.json", &report)?;
    let gate_md_path = inputs.artifact_dir.join("design_gate_report.md");
    write_text(
        &gate_md_path,
        &gate_report_markdown(&report, artifact_locale),
    )?;
    let mut stage_summary = design_stage_summary(
        D3_STAGE_ID,
        localized_text(artifact_locale, "设计验证", "Design Validation"),
        &inputs.summary,
        artifact_locale,
    );
    stage_summary.insert(
        "designValidationReport".to_string(),
        json!(path_string(&validation_path)),
    );
    stage_summary.insert(
        "designGateReport".to_string(),
        json!(path_string(&gate_path)),
    );
    let summary_path = write_stage_json(
        &inputs.artifact_dir,
        "design_stage_summary.json",
        &Value::Object(stage_summary),
    )?;
    let status = if valid || context.test_mode {
        StageStatus::Success
    } else {
        StageStatus::Blocked
    };
    let errors = inputs
        .data
        .meta
        .validation_errors
        .iter()
        .map(ToString::to_string)
        .chain(
            report["blocking_issues"]
                .as_array()
                .into_iter()
                .flatten()
                .map(Value::to_string),
        )
        .collect::<Vec<_>>();
    let warnings = report["conflicts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("message").and_then(Value::as_str))
        .map(str::to_string)
        .chain(
            report["validation_warnings"]
                .as_array()
                .into_iter()
                .flatten()
                .map(Value::to_string),
        )
        .collect::<Vec<_>>();
    Ok(PipelineStageResult {
        status,
        outputs: BTreeMap::from([
            (
                "designValidationReport".to_string(),
                json!(path_string(&validation_path)),
            ),
            (
                "designGateReport".to_string(),
                json!(path_string(&gate_path)),
            ),
            (
                "designGateReportMarkdown".to_string(),
                json!(path_string(&gate_md_path)),
            ),
            (
                "designStageSummary".to_string(),
                json!(path_string(&summary_path)),
            ),
            ("valid".to_string(), report["valid"].clone()),
            ("coverage".to_string(), report["coverage"].clone()),
            (
                "conflictCount".to_string(),
                json!(report["conflicts"].as_array().map(Vec::len).unwrap_or(0)),
            ),
            (
                "blockingIssueCount".to_string(),
                json!(
                    report["blocking_issues"]
                        .as_array()
                        .map(Vec::len)
                        .unwrap_or(0)
                ),
            ),
            (
                "validationErrorCount".to_string(),
                json!(
                    report["validation_errors"]
                        .as_array()
                        .map(Vec::len)
                        .unwrap_or(0)
                ),
            ),
            (
                "validationWarningCount".to_string(),
                json!(
                    report["validation_warnings"]
                        .as_array()
                        .map(Vec::len)
                        .unwrap_or(0)
                ),
            ),
        ]),
        errors,
        warnings,
        message: localized_text(
            artifact_locale,
            "D3 设计验证已完成",
            "D3 design validation completed",
        )
        .to_string(),
    })
}

fn execute_d4(context: &StageContextModel) -> AdmResult<PipelineStageResult> {
    let inputs = load_inputs(context)?;
    let artifact_locale = artifact_locale_from_context(context);
    let mut result = execute_base_design_stage(
        context,
        &inputs,
        D4_STAGE_ID,
        localized_text(artifact_locale, "DevFlow 交接", "DevFlow Handoff"),
    )?;
    let project_state = load_current_project_state(context, &inputs.project_root, &inputs.engine);
    let target_dir = source_artifacts_root(context, &inputs.project_root)?;
    let package = export_concept_package_from_state_with_locale(
        &target_dir,
        &inputs.engine,
        &project_state,
        artifact_locale,
    )?;
    let validation = package
        .get("structured_handoff")
        .and_then(|value| value.get("validation"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let is_blocked = validation.get("status").and_then(Value::as_str) == Some("blocked");
    result
        .outputs
        .insert("conceptPackage".to_string(), package.clone());
    if is_blocked {
        result
            .outputs
            .insert("structuredHandoffStatus".to_string(), json!("blocked"));
        let blockers = validation
            .get("blocking_issues")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        result.errors.extend(blockers.iter().map(Value::to_string));
        if context.test_mode {
            result
                .warnings
                .extend(blockers.iter().map(Value::to_string));
        } else {
            result.status = StageStatus::Blocked;
        }
    } else {
        result
            .outputs
            .insert("structuredHandoffStatus".to_string(), json!("passed"));
    }
    result.message = localized_text(
        artifact_locale,
        "D4 DevFlow 交接已完成",
        "D4 DevFlow handoff completed",
    )
    .to_string();
    Ok(result)
}

fn execute_base_design_stage(
    context: &StageContextModel,
    inputs: &DesignFlowInputs,
    stage_id: &str,
    title: &str,
) -> AdmResult<PipelineStageResult> {
    let artifact_locale = artifact_locale_from_context(context);
    let summary = design_stage_summary(stage_id, title, &inputs.summary, artifact_locale);
    write_stage_json(
        &inputs.artifact_dir,
        "design_stage_summary.json",
        &Value::Object(summary.clone()),
    )?;
    Ok(success(
        summary.into_iter().collect(),
        localized_text(
            artifact_locale,
            "设计阶段摘要已生成",
            "design stage summary generated",
        ),
    ))
}

fn design_stage_summary(
    stage_id: &str,
    title: &str,
    summary: &DesignDataSummary,
    artifact_locale: ArtifactLocale,
) -> Map<String, Value> {
    let mut output = Map::new();
    output.insert("stageId".to_string(), json!(stage_id));
    output.insert("title".to_string(), json!(title));
    output.insert("artifact_locale".to_string(), json!(artifact_locale));
    for (key, value) in outputs_without_domains(summary) {
        output.insert(key, value);
    }
    output
}

fn outputs_without_domains(summary: &DesignDataSummary) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("domainCount".to_string(), json!(summary.domain_count)),
        ("nodeCount".to_string(), json!(summary.node_count)),
        ("checklistCount".to_string(), json!(summary.checklist_count)),
        (
            "optionGroupCount".to_string(),
            json!(summary.option_group_count),
        ),
        ("optionCount".to_string(), json!(summary.option_count)),
        (
            "validationErrorCount".to_string(),
            json!(summary.validation_error_count),
        ),
        (
            "validationWarningCount".to_string(),
            json!(summary.validation_warning_count),
        ),
        ("dataSource".to_string(), json!(summary.data_source)),
    ])
}

fn write_stage_json(artifact_dir: &Path, filename: &str, payload: &Value) -> AdmResult<PathBuf> {
    write_json(&artifact_dir.join(filename), payload)
}

fn success(outputs: BTreeMap<String, Value>, message: &str) -> PipelineStageResult {
    PipelineStageResult {
        status: StageStatus::Success,
        outputs,
        errors: Vec::new(),
        warnings: Vec::new(),
        message: message.to_string(),
    }
}

fn artifact_locale_from_context(context: &StageContextModel) -> ArtifactLocale {
    ArtifactLocale::normalize(
        context
            .metadata
            .get("artifact_locale")
            .or_else(|| context.inputs.get("artifact_locale"))
            .and_then(Value::as_str),
    )
}

fn localized_text<'a>(locale: ArtifactLocale, zh_cn: &'a str, en_us: &'a str) -> &'a str {
    match locale {
        ArtifactLocale::ZhCn => zh_cn,
        ArtifactLocale::EnUs => en_us,
    }
}

fn localized_status<'a>(status: &'a str, locale: ArtifactLocale) -> &'a str {
    if locale == ArtifactLocale::EnUs {
        return status;
    }
    match status {
        "passed" => "通过",
        "blocked" => "已阻断",
        "reported" => "已生成报告",
        "completed" => "已完成",
        "partial" => "部分完成",
        "missing" => "缺失",
        _ => status,
    }
}

fn project_root_from_context(context: &StageContextModel) -> AdmResult<PathBuf> {
    if !context.project_root.trim().is_empty() {
        Ok(PathBuf::from(&context.project_root))
    } else {
        locate_project_root(env!("CARGO_MANIFEST_DIR"))
    }
}

fn artifact_dir_from_context(
    context: &StageContextModel,
    project_root: &Path,
) -> AdmResult<PathBuf> {
    if !context.artifact_dir.trim().is_empty() {
        return Ok(PathBuf::from(&context.artifact_dir));
    }
    Ok(project_root.join(format!(
        "drafts/design_flow/outputs/artifacts/stage_{}",
        context.stage_id.to_ascii_lowercase()
    )))
}

fn draft_root_from_artifact_dir(artifact_dir: &Path) -> Option<PathBuf> {
    for ancestor in artifact_dir.ancestors() {
        if ancestor.file_name().and_then(|value| value.to_str()) == Some("outputs") {
            return ancestor.parent().map(Path::to_path_buf);
        }
    }
    None
}

fn source_artifacts_root(context: &StageContextModel, project_root: &Path) -> AdmResult<PathBuf> {
    let artifact_dir = artifact_dir_from_context(context, project_root)?;
    let root = draft_root_from_artifact_dir(&artifact_dir)
        .unwrap_or_else(|| project_root.join("drafts/design_flow"));
    Ok(root.join("source_artifacts"))
}

fn autosave_candidates(context: &StageContextModel, project_root: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = context
        .metadata
        .get("autosave_state_path")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(artifact_dir) = artifact_dir_from_context(context, project_root) {
        if let Some(draft_root) = draft_root_from_artifact_dir(&artifact_dir) {
            candidates.push(draft_root.join("autosave_state.json"));
        }
    }
    if let Some(session_id) = context
        .metadata
        .get("draft_session_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        candidates.push(
            project_root
                .join("drafts")
                .join(session_id)
                .join("autosave_state.json"),
        );
    }
    if let Ok(session_id) = std::env::var("AUTODESIGNMAKER_SESSION_ID") {
        if !session_id.trim().is_empty() {
            candidates.push(
                project_root
                    .join("drafts")
                    .join(session_id)
                    .join("autosave_state.json"),
            );
        }
    }
    candidates
}

fn load_current_project_state(
    context: &StageContextModel,
    project_root: &Path,
    engine: &DesignEngineService,
) -> ProjectState {
    if let Some(value) = context.metadata.get("project_state") {
        if let Ok(mut state) = serde_json::from_value::<ProjectState>(value.clone()) {
            ensure_profile_defaults(&mut state);
            return engine.normalize_state(state);
        }
    }
    for path in autosave_candidates(context, project_root) {
        let value = read_json(&path, Value::Null);
        if !value.is_null() {
            if let Ok(mut state) = serde_json::from_value::<ProjectState>(value) {
                ensure_profile_defaults(&mut state);
                return engine.normalize_state(state);
            }
        }
    }
    let mut state = engine.empty_state();
    ensure_profile_defaults(&mut state);
    engine.normalize_state(state)
}

fn ensure_profile_defaults(state: &mut ProjectState) {
    for field in PROFILE_FIELDS {
        state
            .profile
            .entry((*field).to_string())
            .or_insert_with(|| json!("unknown"));
    }
}

pub fn design_engine_from_data(data: &DesignProjectData) -> DesignEngineService {
    let specs = data
        .domains
        .iter()
        .flat_map(|domain| domain.nodes.iter())
        .map(|node| DesignNodeSpec {
            node_id: node.id.clone(),
            domain_id: node.domain.clone(),
            name: node.name.clone(),
            description: node.description.clone(),
            role_class: node.role_class.clone(),
            checklist: node
                .checklist
                .iter()
                .map(|item| DesignChecklistItemSpec {
                    item_id: item.id.clone(),
                    label: item.label.clone(),
                    option_groups: item
                        .option_groups
                        .iter()
                        .map(|group| DesignOptionGroupSpec {
                            group_id: group.id.clone(),
                            selection_mode: group.selection_mode.clone(),
                            allow_primary: group.allow_primary,
                            options: group
                                .options
                                .iter()
                                .map(|option| option.id.clone())
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        })
        .collect();
    DesignEngineService::new(specs)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct PriorityCount {
    total: usize,
    completed: usize,
    blocked: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct ContractCompletion {
    contract: String,
    status: String,
    nodes: Vec<String>,
    blocking_issues: Vec<Value>,
    review_items: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
struct CompletionSummary {
    priority_counts: BTreeMap<String, PriorityCount>,
    contracts: BTreeMap<String, ContractCompletion>,
    blocking_issues: Vec<Value>,
    review_items: Vec<Value>,
}

impl CompletionSummary {
    fn to_json(&self) -> Value {
        json!({
            "schema_version": "1.0",
            "status": if self.blocking_issues.is_empty() { "ready" } else { "blocked" },
            "priority_counts": self.priority_counts,
            "contracts": self.contracts,
            "blocking_issues": self.blocking_issues,
            "review_items": self.review_items,
        })
    }

    fn contract_targets_json(&self) -> Value {
        Value::Array(
            self.contracts
                .values()
                .map(|contract| {
                    json!({
                        "contract_id": contract.contract,
                        "required": contract.status != "optional",
                        "source_nodes": contract.nodes,
                        "status": contract.status,
                        "blocking_issues": contract.blocking_issues,
                        "review_items": contract.review_items,
                    })
                })
                .collect(),
        )
    }

    fn all_issues(&self) -> Value {
        Value::Array(
            self.blocking_issues
                .iter()
                .chain(self.review_items.iter())
                .cloned()
                .collect(),
        )
    }
}

fn design_completion_summary(
    data: &DesignProjectData,
    engine: &DesignEngineService,
    project_state: &ProjectState,
    artifact_locale: ArtifactLocale,
) -> CompletionSummary {
    let mut contracts = COMMON_REQUIRED_CONTRACTS
        .iter()
        .chain(OPTIONAL_CONTRACTS.iter())
        .map(|contract| {
            (
                (*contract).to_string(),
                ContractCompletion {
                    contract: (*contract).to_string(),
                    status: "missing".to_string(),
                    nodes: Vec::new(),
                    blocking_issues: Vec::new(),
                    review_items: Vec::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut priority_counts = ["P0", "P1", "P2"]
        .into_iter()
        .map(|priority| {
            (
                priority.to_string(),
                PriorityCount {
                    total: 0,
                    completed: 0,
                    blocked: 0,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut blocking_issues = Vec::new();
    let mut review_items = Vec::new();

    for node in data.domains.iter().flat_map(|domain| domain.nodes.iter()) {
        let targets = node.contract_targets.clone();
        if targets.is_empty() && !priority_counts.contains_key(&node.priority) {
            continue;
        }
        let empty_node = NodeState::default();
        let node_state = project_state.nodes.get(&node.id).unwrap_or(&empty_node);
        let effective = effective_node_state(engine, node_state);
        let mut confirmed = node_has_confirmed_structured_selection(node_state);
        let mut node_blockers = Vec::new();
        let mut node_reviews = Vec::new();

        if let Some(counts) = priority_counts.get_mut(&node.priority) {
            counts.total += 1;
        }

        if effective == DecisionState::NotApplicable {
            if node.not_applicable_requires_reason
                && node_state.not_applicable_reason.trim().is_empty()
            {
                node_blockers.push(json!({
                    "code": "NOT_APPLICABLE_REASON_MISSING",
                    "node_id": node.id,
                    "message": localized_text(
                        artifact_locale,
                        "标记为不适用的 P0/P1 节点必须填写原因。",
                        "Not applicable P0/P1 node must include a reason."
                    ),
                }));
            } else {
                confirmed = true;
            }
        } else if !matches!(
            effective,
            DecisionState::Selected | DecisionState::Completed | DecisionState::Risk
        ) {
            node_blockers.push(json!({
                "code": "REQUIRED_DESIGN_NODE_NOT_SELECTED",
                "node_id": node.id,
                "message": localized_text(
                    artifact_locale,
                    "必需设计节点尚未选择或完成。",
                    "Required design node is not selected or completed."
                ),
            }));
        }

        if !targets.is_empty() && effective != DecisionState::NotApplicable && !confirmed {
            node_blockers.push(json!({
                "code": "STRUCTURED_SELECTION_NOT_CONFIRMED",
                "node_id": node.id,
                "message": localized_text(
                    artifact_locale,
                    "必需契约节点需要 user_selected 或 user_confirmed_ai 选项来源并完成确认。",
                    "Required contract node needs user_selected or user_confirmed_ai option provenance."
                ),
            }));
        }

        for record in selected_option_provenance_records(node, node_state) {
            if record.source == "ai_inferred" || record.source == "migration_inferred" {
                node_reviews.push(json!({
                    "code": "OPTION_REQUIRES_HUMAN_CONFIRMATION",
                    "node_id": node.id,
                    "item_id": record.item_id,
                    "group_id": record.group_id,
                    "option_id": record.option_id,
                    "source": record.source,
                    "message": localized_text(
                        artifact_locale,
                        "AI 或迁移推断的选项必须经人工确认后才能计为完成。",
                        "AI or migration inferred option must be confirmed before it counts as complete."
                    ),
                }));
            }
        }

        if let Some(counts) = priority_counts.get_mut(&node.priority) {
            if !node_blockers.is_empty() {
                counts.blocked += 1;
            } else if confirmed || effective == DecisionState::NotApplicable {
                counts.completed += 1;
            }
        }

        blocking_issues.extend(node_blockers.clone());
        review_items.extend(node_reviews.clone());
        for target in targets {
            let contract = contracts
                .entry(target.clone())
                .or_insert_with(|| ContractCompletion {
                    contract: target.clone(),
                    status: "missing".to_string(),
                    nodes: Vec::new(),
                    blocking_issues: Vec::new(),
                    review_items: Vec::new(),
                });
            contract.nodes.push(node.id.clone());
            contract.blocking_issues.extend(node_blockers.clone());
            contract.review_items.extend(node_reviews.clone());
        }
    }

    let optional = OPTIONAL_CONTRACTS
        .iter()
        .map(|value| (*value).to_string())
        .collect::<BTreeSet<_>>();
    for (contract_id, state) in &mut contracts {
        if !state.blocking_issues.is_empty() {
            state.status = "blocked".to_string();
        } else if !state.nodes.is_empty() {
            state.status = "covered".to_string();
        } else if optional.contains(contract_id) {
            state.status = "optional".to_string();
        } else {
            state.status = "missing".to_string();
            let issue = json!({
                "code": "CONTRACT_TARGET_NODE_MISSING",
                "contract": contract_id,
                "message": if artifact_locale == ArtifactLocale::ZhCn {
                    format!("当前没有设计节点指向契约 {contract_id}。")
                } else {
                    format!("No design node currently targets {contract_id}.")
                },
            });
            state.blocking_issues.push(issue.clone());
            blocking_issues.push(issue);
        }
    }

    CompletionSummary {
        priority_counts,
        contracts,
        blocking_issues,
        review_items,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProvenanceRecord {
    item_id: String,
    group_id: String,
    option_id: String,
    source: String,
    confirmed: bool,
}

fn selected_option_provenance_records(
    node: &DomainNode,
    node_state: &NodeState,
) -> Vec<ProvenanceRecord> {
    let mut records = Vec::new();
    for item in &node.checklist {
        for group in &item.option_groups {
            let selected = node_state
                .checklist_options
                .get(&item.id)
                .and_then(|items| items.get(&group.id))
                .map(|group_state| group_state.selected.clone())
                .unwrap_or_default();
            for option_id in selected {
                let provenance = node_state
                    .option_provenance
                    .get(&item.id)
                    .and_then(|items| items.get(&group.id))
                    .and_then(|options| options.get(&option_id));
                records.push(ProvenanceRecord {
                    item_id: item.id.clone(),
                    group_id: group.id.clone(),
                    option_id,
                    source: provenance
                        .map(|entry| entry.source.clone())
                        .unwrap_or_default(),
                    confirmed: provenance
                        .and_then(|entry| entry.confirmed)
                        .unwrap_or(false),
                });
            }
        }
    }
    records
}

fn node_has_confirmed_structured_selection(node_state: &NodeState) -> bool {
    node_state
        .option_provenance
        .values()
        .flat_map(BTreeMap::values)
        .flat_map(BTreeMap::values)
        .any(|entry| {
            (entry.source == "user_selected" || entry.source == "user_confirmed_ai")
                && entry.confirmed.unwrap_or(false)
        })
}

fn effective_node_state(engine: &DesignEngineService, node_state: &NodeState) -> DecisionState {
    engine.effective_node_state(node_state)
}

fn provenance_review_counts(review_items: &[Value]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::from([
        ("ai_inferred".to_string(), 0usize),
        ("migration_inferred".to_string(), 0usize),
    ]);
    for item in review_items {
        if let Some(source) = item.get("source").and_then(Value::as_str) {
            if let Some(count) = counts.get_mut(source) {
                *count += 1;
            }
        }
    }
    counts
}

fn profile_blockers(
    profile: &BTreeMap<String, Value>,
    artifact_locale: ArtifactLocale,
) -> Vec<Value> {
    let unknown = profile
        .iter()
        .filter_map(|(key, value)| {
            let is_unknown = value.is_null()
                || value.as_str().map(str::trim).unwrap_or_default().is_empty()
                || value.as_str() == Some("unknown");
            is_unknown.then(|| key.clone())
        })
        .collect::<Vec<_>>();
    if unknown.is_empty() {
        Vec::new()
    } else {
        vec![json!({
            "code": "PROFILE_INCOMPLETE",
            "message": localized_text(
                artifact_locale,
                "D4 交接前必须明确填写项目画像。",
                "Project profile must be explicit before D4 handoff."
            ),
            "path": "profile",
            "missing_or_unknown_fields": unknown,
            "required_by_steps": ["D4", "Step00", "Step13"],
        })]
    }
}

fn archetype_warning_blockers(
    archetype_requirements: &Value,
    artifact_locale: ArtifactLocale,
) -> Vec<Value> {
    archetype_requirements
        .get("warnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            json!({
                "code": "ARCHETYPE_FALLBACK_GENERIC",
                "message": item.get("message").and_then(Value::as_str).unwrap_or_else(|| localized_text(
                    artifact_locale,
                    "通用可玩原型回退需要复核。",
                    "Generic playable archetype fallback requires review."
                )),
                "severity": "warning",
                "required_by_steps": ["D4", "Step00", "Step02"],
            })
        })
        .collect()
}

fn confirmed_option_text(data: &DesignProjectData, state: &ProjectState) -> Vec<String> {
    let mut text = Vec::new();
    for node in data.domains.iter().flat_map(|domain| domain.nodes.iter()) {
        let Some(node_state) = state.nodes.get(&node.id) else {
            continue;
        };
        for item in &node.checklist {
            for group in &item.option_groups {
                let selected = node_state
                    .checklist_options
                    .get(&item.id)
                    .and_then(|items| items.get(&group.id))
                    .map(|group_state| &group_state.selected);
                let Some(selected) = selected else {
                    continue;
                };
                for option_id in selected {
                    let confirmed = node_state
                        .option_provenance
                        .get(&item.id)
                        .and_then(|items| items.get(&group.id))
                        .and_then(|options| options.get(option_id))
                        .map(|entry| {
                            (entry.source == "user_selected" || entry.source == "user_confirmed_ai")
                                && entry.confirmed.unwrap_or(false)
                        })
                        .unwrap_or(false);
                    if !confirmed {
                        continue;
                    }
                    if let Some(option) =
                        group.options.iter().find(|option| option.id == *option_id)
                    {
                        text.push(option.label.clone());
                        text.push(option.description.clone());
                    }
                }
            }
        }
    }
    text
}

fn decision_report_markdown(report: &Value, artifact_locale: ArtifactLocale) -> String {
    let mut lines = vec![
        format!("<!-- artifact_locale: {} -->", artifact_locale.as_str()),
        localized_text(
            artifact_locale,
            "# 设计决策报告",
            "# Design Decision Report",
        )
        .to_string(),
        String::new(),
        format!(
            "- {}: {}",
            localized_text(artifact_locale, "状态", "Status"),
            localized_status(str_field(report, "status"), artifact_locale)
        ),
        format!(
            "- {}: {}",
            localized_text(artifact_locale, "项目", "Project"),
            str_field(report, "project_id")
        ),
        format!(
            "- P0: {}/{}",
            report["summary"]["p0_completed"].as_u64().unwrap_or(0),
            report["summary"]["p0_total"].as_u64().unwrap_or(0)
        ),
        format!(
            "- {}: {}",
            localized_text(artifact_locale, "P0 阻断项", "P0 blocking"),
            report["summary"]["p0_blocking"].as_u64().unwrap_or(0)
        ),
        format!(
            "- {}: {}",
            localized_text(
                artifact_locale,
                "AI 推断但未确认",
                "AI inferred unconfirmed"
            ),
            report["summary"]["ai_inferred_unconfirmed"]
                .as_u64()
                .unwrap_or(0)
        ),
        format!(
            "- {}: {}",
            localized_text(
                artifact_locale,
                "迁移推断但未确认",
                "Migration inferred unconfirmed"
            ),
            report["summary"]["migration_inferred_unconfirmed"]
                .as_u64()
                .unwrap_or(0)
        ),
        String::new(),
        localized_text(artifact_locale, "## 合约目标", "## Contract Targets").to_string(),
        String::new(),
    ];
    for contract in report["contract_targets"].as_array().into_iter().flatten() {
        lines.push(format!(
            "- {}: {} ({}: {}, {}: {})",
            str_field(contract, "contract_id"),
            localized_status(str_field(contract, "status"), artifact_locale),
            localized_text(artifact_locale, "节点", "nodes"),
            contract["source_nodes"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0),
            localized_text(artifact_locale, "阻断项", "blocking"),
            contract["blocking_issues"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0)
        ));
    }
    lines.extend([
        String::new(),
        localized_text(artifact_locale, "## 问题", "## Issues").to_string(),
        String::new(),
    ]);
    if let Some(issues) = report["issues"]
        .as_array()
        .filter(|items| !items.is_empty())
    {
        for issue in issues {
            lines.push(format!(
                "- {}: {} {}",
                str_field(issue, "code"),
                issue
                    .get("node_id")
                    .or_else(|| issue.get("contract"))
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                str_field(issue, "message")
            ));
        }
    } else {
        lines.push(localized_text(artifact_locale, "- 无", "- none").to_string());
    }
    lines.join("\n") + "\n"
}

fn gate_report_markdown(report: &Value, artifact_locale: ArtifactLocale) -> String {
    let mut lines = vec![
        format!("<!-- artifact_locale: {} -->", artifact_locale.as_str()),
        localized_text(artifact_locale, "# 设计门禁报告", "# Design Gate Report").to_string(),
        String::new(),
        format!(
            "- {}: {}",
            localized_text(artifact_locale, "状态", "Status"),
            localized_status(str_field(report, "status"), artifact_locale)
        ),
        format!(
            "- {}: {}",
            localized_text(artifact_locale, "有效", "Valid"),
            report["valid"].as_bool().unwrap_or(false)
        ),
        format!(
            "- {}: {}",
            localized_text(artifact_locale, "阻断项", "Blocking issues"),
            report["blocking_issues"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0)
        ),
        String::new(),
        localized_text(artifact_locale, "## 阻断项", "## Blocking Issues").to_string(),
        String::new(),
    ];
    if let Some(issues) = report["blocking_issues"]
        .as_array()
        .filter(|items| !items.is_empty())
    {
        for issue in issues {
            lines.push(format!(
                "- {}: {} {}",
                str_field(issue, "code"),
                issue
                    .get("node_id")
                    .or_else(|| issue.get("contract"))
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                str_field(issue, "message")
            ));
        }
    } else {
        lines.push(localized_text(artifact_locale, "- 无", "- none").to_string());
    }
    lines.extend([
        String::new(),
        localized_text(artifact_locale, "## 复核项", "## Review Items").to_string(),
        String::new(),
    ]);
    if let Some(items) = report["review_items"]
        .as_array()
        .filter(|items| !items.is_empty())
    {
        for item in items {
            lines.push(format!(
                "- {}: {} {}",
                str_field(item, "code"),
                str_field(item, "node_id"),
                str_field(item, "message")
            ));
        }
    } else {
        lines.push(localized_text(artifact_locale, "- 无", "- none").to_string());
    }
    lines.join("\n") + "\n"
}

fn config_string(root: &Value, key_path: &str, fallback: &str) -> String {
    get_config(root, key_path)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn str_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("")
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;

    fn project_root() -> PathBuf {
        locate_project_root(env!("CARGO_MANIFEST_DIR")).unwrap()
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn context(stage_id: &str, root: &Path, test_mode: bool) -> (StageContextModel, PathBuf) {
        let temp = temp_root(&format!("design_flow_{stage_id}"));
        let artifact_dir = temp
            .join("drafts/session_a/outputs/artifacts")
            .join(format!("stage_{}", stage_id.to_ascii_lowercase()));
        std::fs::create_dir_all(&artifact_dir).unwrap();
        (
            StageContextModel {
                stage_id: stage_id.to_string(),
                project_root: path_string(root),
                inputs: BTreeMap::new(),
                outputs: BTreeMap::new(),
                metadata: BTreeMap::new(),
                knowledge: BTreeMap::new(),
                skills: BTreeMap::new(),
                test_mode,
                artifact_dir: path_string(&artifact_dir),
            },
            temp,
        )
    }

    #[test]
    fn plugin_specs_and_stage_specs_match_python_registry() {
        let plugin_specs = design_plugin_specs();
        assert_eq!(
            plugin_specs
                .iter()
                .map(|spec| spec.stage_id)
                .collect::<Vec<_>>(),
            vec!["D1", "D2", "D3", "D4"]
        );
        assert!(
            plugin_specs
                .iter()
                .all(|spec| spec.source_groups.is_empty())
        );
        assert!(
            plugin_specs
                .iter()
                .all(|spec| spec.generation_entrypoint == DESIGN_FLOW_ENTRYPOINT)
        );
        let stages = design_stage_specs();
        assert_eq!(stages[0].title, "D1 Project Portrait");
        assert_eq!(
            stages[3].plugin_ref,
            "pipeline.step_d4_devflow_handoff.plugin"
        );
        assert_eq!(stages[3].requires, vec!["D3"]);
        assert!(matches!(stages[0].kind, StageKind::Design));
    }

    #[test]
    fn d1_writes_project_portrait_from_real_design_data() {
        let root = project_root();
        let (ctx, temp) = context(D1_STAGE_ID, &root, true);

        let result = execute_design_stage(D1_STAGE_ID, &ctx);

        assert_eq!(result.status, StageStatus::Success);
        assert!(result.outputs["domainCount"].as_u64().unwrap() >= 1);
        let portrait = PathBuf::from(result.outputs["designPortrait"].as_str().unwrap());
        assert!(portrait.exists());
        let payload = read_json(&portrait, json!({}));
        assert_eq!(payload["project_name"], json!("AutoDesignMaker"));
        assert!(payload["design_node_count"].as_u64().unwrap() > 0);
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn design_flow_artifacts_default_to_chinese_and_keep_english_extension() {
        let root = project_root();
        let (zh_context, zh_temp) = context(D1_STAGE_ID, &root, true);

        let zh_result = execute_design_stage(D1_STAGE_ID, &zh_context);

        assert_eq!(zh_result.status, StageStatus::Success);
        assert_eq!(zh_result.message, "D1 项目画像已生成");
        let zh_portrait = PathBuf::from(
            zh_result.outputs["designPortrait"]
                .as_str()
                .expect("D1 should expose its portrait path"),
        );
        let zh_payload = read_json(&zh_portrait, json!({}));
        assert_eq!(zh_payload["artifact_locale"], json!("zh-CN"));
        assert_eq!(
            zh_payload["target_audience"],
            json!("游戏设计师、技术设计师与 Unity 开发者")
        );

        let (mut en_context, en_temp) = context(D1_STAGE_ID, &root, true);
        en_context
            .metadata
            .insert("artifact_locale".to_string(), json!("en-US"));

        let en_result = execute_design_stage(D1_STAGE_ID, &en_context);

        assert_eq!(en_result.status, StageStatus::Success);
        assert_eq!(en_result.message, "D1 project portrait generated");
        let en_portrait = PathBuf::from(
            en_result.outputs["designPortrait"]
                .as_str()
                .expect("D1 should expose its portrait path"),
        );
        let en_payload = read_json(&en_portrait, json!({}));
        assert_eq!(en_payload["artifact_locale"], json!("en-US"));
        assert_eq!(
            en_payload["target_audience"],
            json!("game designers, technical designers, and Unity developers")
        );

        let _ = std::fs::remove_dir_all(zh_temp);
        let _ = std::fs::remove_dir_all(en_temp);
    }

    #[test]
    fn d2_reports_empty_project_as_incomplete() {
        let root = project_root();
        let (ctx, temp) = context(D2_STAGE_ID, &root, true);

        let result = execute_design_stage(D2_STAGE_ID, &ctx);

        assert_eq!(result.status, StageStatus::Success);
        let report_path = PathBuf::from(result.outputs["designDecisionReport"].as_str().unwrap());
        let report = read_json(&report_path, json!({}));
        assert_eq!(report["status"], json!("reported"));
        assert!(report["summary"]["p0_total"].as_u64().unwrap() > 0);
        assert_eq!(report["summary"]["p0_completed"], json!(0));
        assert!(report["summary"]["p0_blocking"].as_u64().unwrap() > 0);
        let issue = report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .find(|issue| issue["code"] == "REQUIRED_DESIGN_NODE_NOT_SELECTED")
            .unwrap();
        assert_eq!(issue["message"], "必需设计节点尚未选择或完成。");
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn d3_blocks_empty_project_but_test_mode_keeps_plugin_success() {
        let root = project_root();
        let (mut ctx, temp) = context(D3_STAGE_ID, &root, false);

        let blocked = execute_design_stage(D3_STAGE_ID, &ctx);

        assert_eq!(blocked.status, StageStatus::Blocked);
        let report_path = PathBuf::from(blocked.outputs["designGateReport"].as_str().unwrap());
        let report = read_json(&report_path, json!({}));
        assert_eq!(report["status"], json!("blocked"));
        assert_eq!(report["valid"], json!(false));
        let blockers = report["blocking_issues"].as_array().unwrap();
        assert!(
            blockers
                .iter()
                .any(|issue| issue["code"] == json!("PROFILE_INCOMPLETE"))
        );
        assert!(blockers.iter().any(|issue| {
            issue["code"] == json!("PROFILE_INCOMPLETE")
                && issue["message"] == json!("D4 交接前必须明确填写项目画像。")
        }));
        assert!(blockers.iter().any(|issue| {
            issue["code"] == json!("CONTRACT_TARGET_NOT_FORMABLE")
                && issue["contract"] == json!("ui_flow_contract")
        }));

        ctx.test_mode = true;
        let test_mode = execute_design_stage(D3_STAGE_ID, &ctx);
        assert_eq!(test_mode.status, StageStatus::Success);
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn d4_exports_concept_package_and_propagates_handoff_blockers() {
        let root = project_root();
        let (ctx, temp) = context(D4_STAGE_ID, &root, false);

        let result = execute_design_stage(D4_STAGE_ID, &ctx);

        assert_eq!(result.status, StageStatus::Blocked);
        assert_eq!(result.outputs["structuredHandoffStatus"], json!("blocked"));
        assert!(result.outputs["conceptPackage"]["packages"]["Design"].is_string());
        let package_dir = temp.join("drafts/session_a/source_artifacts/devflow_Design_v2");
        assert!(
            package_dir
                .join("structured/handoff_manifest.json")
                .exists()
        );

        let (ctx, temp_test) = context(D4_STAGE_ID, &root, true);
        let test_mode = execute_design_stage(D4_STAGE_ID, &ctx);
        assert_eq!(test_mode.status, StageStatus::Success);
        assert!(!test_mode.warnings.is_empty());
        let _ = std::fs::remove_dir_all(temp);
        let _ = std::fs::remove_dir_all(temp_test);
    }
}
