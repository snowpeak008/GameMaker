#![forbid(unsafe_code)]

pub mod dev_tools;
pub mod execution_objects;
pub mod iteration;
pub mod migration_tools;
pub mod project_environment;
pub mod runtime;
pub mod style_image;
pub mod validation_tools;
pub mod vlm_review;
pub mod work_unit;

pub use style_image::{AiStyleImageGenerator, style_image_generator_from_config};
pub use vlm_review::{OpenAiVisionVlmReviewer, vlm_review_service_from_config};
pub use work_unit::{
    AI_DEVELOPMENT_EXECUTOR_PROHIBITED_CALLERS, AI_DEVELOPMENT_EXECUTOR_RETAINED_CALLERS,
    AI_DEVELOPMENT_EXECUTOR_V2_REPLACEMENT, AiDevelopmentWorkUnitExecutor,
    work_unit_executor_from_config, workspace_task_agent_from_config,
};

use std::collections::{BTreeMap, BTreeSet};

pub use adm_new_ai::api_probe::AiApiProbeView;
pub use adm_new_ai::{
    AiCliProbeView, AiConfigDescriptorView, AiConfigValidationReport, AiInterviewTurnReport,
    AiResolutionView, CompletionAdapterSpec,
};
use adm_new_ai::{AiConfigService, AiInterviewService};
use adm_new_artifact::{ArtifactEvidenceSet, ArtifactService};
use adm_new_contracts::ArtifactLocale;
use adm_new_contracts::ai::{
    AiConfig, AiInterviewState, AiResponsePayload, AiSchemaMode, ApiEntry,
};
use adm_new_contracts::artifact::{
    ArtifactLayerManifest, ArtifactRegistry, ArtifactReviewReport, ArtifactValidationLayerReport,
    DependencyGraph, PreflightReport,
};
use adm_new_contracts::log::{LogEntry, LogLevel};
use adm_new_contracts::patch::{PatchRecord, PatchStatus, PatchTask};
use adm_new_contracts::pipeline::{
    PipelineRegistry, PipelineRunState, PipelineStageResult, PipelineStageRuntime, StageSpec,
    StageStatus,
};
use adm_new_contracts::project::{
    DecisionState, GameplaySystemOption, GameplaySystemWeight, ProjectState,
};
use adm_new_contracts::save::{DraftMeta, SaveIndex};
use adm_new_contracts::sdk::{SdkIndex, SdkReviewStatus, SdkSpec};
use adm_new_design::DesignEngineService;
pub use adm_new_design::data_loader::DesignDataLoader;
use adm_new_design::data_loader::ProjectTemplateMeta;
pub use adm_new_design::{
    DesignChecklistItemSpec, DesignNodeSpec, DesignOptionGroupSpec, DesignWorkbenchView,
};
use adm_new_foundation::{AdmError, AdmResult, hash_text, unix_timestamp};
pub use adm_new_packaging::PackageRunResult;
use adm_new_packaging::{PackagingService, PackagingSources};
use adm_new_patch::PatchService;
use adm_new_pipeline::PipelineService;
pub use adm_new_pipeline::{
    PipelineRunObserver, PipelineRunReport, ResolvedPipelineRange, StageExecutor,
};
use adm_new_save::SaveService;
pub use adm_new_save::{
    BlankSaveRepairReport, LoadedSave, ParallelIsolationAuditReport, ParallelRepairReport,
    SaveServiceReport,
};
use adm_new_sdk::{
    LEGACY_DESKTOP_SDK_FILE, LegacySdkMigrationReport, SdkKnowledgeBase, SdkKnowledgeService,
    SkillDocument, SkillOverlayRepository, SkillRecord,
};
use serde_json::Value;

pub const CRATE_NAME: &str = "adm-new-application";

pub fn crate_ready() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignAutosaveReport {
    pub schema_version: u32,
    pub project_name: String,
    pub state_hash: String,
    pub dirty: bool,
    pub autosave_file: String,
    pub node_count: usize,
    pub selected_gameplay_system_count: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignTemplateSelectionReport {
    pub template_id: String,
    pub project_name: String,
    pub view: DesignWorkbenchView,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignTemplateListReport {
    pub templates: Vec<DesignTemplateSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignTemplateSummary {
    pub template_id: String,
    pub source: String,
    pub source_label: String,
    pub name: String,
    pub game_name: String,
    pub target_scale: String,
    pub scale_label: String,
    pub quality_tier: String,
    pub summary: String,
    pub visibility: String,
    pub file_name: String,
    pub order: Option<i64>,
    pub analysis: Vec<Value>,
    pub verification: Value,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignTemplateSaveReport {
    pub template_id: String,
    pub template_name: String,
    pub target_scale: String,
    pub target_file_name: String,
    pub state_hash: String,
    pub overwritten: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesignTemplateDeleteReport {
    pub template_id: String,
    pub template_name: String,
    pub target_scale: String,
    pub target_file_name: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GameplaySystemUpdateRequest {
    #[serde(default)]
    pub system_id: String,
    #[serde(default)]
    pub selected: Option<bool>,
    #[serde(default)]
    pub weight: Option<Value>,
    #[serde(default)]
    pub core_loop: Option<String>,
    #[serde(default)]
    pub custom_name: Option<String>,
    #[serde(default)]
    pub delete_custom: bool,
    #[serde(default)]
    pub interview_answers: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct DesignWorkbenchService {
    engine: DesignEngineService,
    template_loader: Option<DesignDataLoader>,
}

impl DesignWorkbenchService {
    pub fn new(specs: Vec<DesignNodeSpec>) -> Self {
        Self {
            engine: DesignEngineService::new(specs),
            template_loader: None,
        }
    }

    pub fn with_template_loader(mut self, template_loader: DesignDataLoader) -> Self {
        self.template_loader = Some(template_loader);
        self
    }

    pub fn empty_project_state(&self) -> ProjectState {
        self.engine.empty_state()
    }

    pub fn normalize_project_state(&self, state: ProjectState) -> ProjectState {
        self.engine.normalize_state(state)
    }

    pub fn view_model(&self, state: &ProjectState) -> DesignWorkbenchView {
        self.engine.view_model(state)
    }

    pub fn set_checklist_item(
        &self,
        state: &mut ProjectState,
        node_id: &str,
        item_id: &str,
        checked: bool,
    ) -> AdmResult<DesignWorkbenchView> {
        self.engine
            .set_checklist_item(state, node_id, item_id, checked)?;
        Ok(self.view_model(state))
    }

    pub fn set_option_group_option(
        &self,
        state: &mut ProjectState,
        node_id: &str,
        item_id: &str,
        group_id: &str,
        option_id: &str,
        selected: bool,
    ) -> AdmResult<DesignWorkbenchView> {
        self.engine
            .set_option_group_option(state, node_id, item_id, group_id, option_id, selected)?;
        Ok(self.view_model(state))
    }

    pub fn set_option_group_primary(
        &self,
        state: &mut ProjectState,
        node_id: &str,
        item_id: &str,
        group_id: &str,
        option_id: &str,
    ) -> AdmResult<DesignWorkbenchView> {
        self.engine
            .set_option_group_primary(state, node_id, item_id, group_id, option_id)?;
        Ok(self.view_model(state))
    }

    pub fn update_node_text(
        &self,
        state: &mut ProjectState,
        node_id: &str,
        design_note: Option<&str>,
        risk_note: Option<&str>,
        not_applicable_reason: Option<&str>,
    ) -> AdmResult<DesignWorkbenchView> {
        let node = state
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| AdmError::new(format!("unknown node: {node_id}")))?;
        if let Some(value) = design_note {
            node.design_note = value.to_string();
        }
        if let Some(value) = risk_note {
            node.risk_note = value.to_string();
        }
        if let Some(value) = not_applicable_reason {
            node.not_applicable_reason = value.to_string();
        }
        self.engine.refresh_node_state(node);
        Ok(self.view_model(state))
    }

    pub fn replace_design_entities(
        &self,
        state: &mut ProjectState,
        node_id: &str,
        design_entities: Vec<Value>,
    ) -> AdmResult<DesignWorkbenchView> {
        let node = state
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| AdmError::new(format!("unknown node: {node_id}")))?;
        node.design_entities = design_entities;
        self.engine.refresh_node_state(node);
        Ok(self.view_model(state))
    }

    pub fn update_gameplay_system(
        &self,
        state: &mut ProjectState,
        request: &GameplaySystemUpdateRequest,
    ) -> AdmResult<DesignWorkbenchView> {
        let mut system_id = request.system_id.trim().to_string();
        if let Some(custom_name) = request.custom_name.as_ref().map(|value| value.trim()) {
            if !custom_name.is_empty() {
                system_id = gameplay_custom_system_id(custom_name);
                if !state
                    .gameplay_systems
                    .custom
                    .iter()
                    .any(|option| option.id == system_id)
                {
                    state.gameplay_systems.custom.push(GameplaySystemOption {
                        id: system_id.clone(),
                        name: custom_name.to_string(),
                        category: "custom".to_string(),
                        mapping_desc: "custom gameplay system from workbench".to_string(),
                        extra: BTreeMap::new(),
                    });
                }
                if request.selected.unwrap_or(true)
                    && !state
                        .gameplay_systems
                        .selected
                        .iter()
                        .any(|selected| selected == &system_id)
                {
                    state.gameplay_systems.selected.push(system_id.clone());
                }
            }
        }
        if system_id.is_empty() && request.delete_custom {
            return Err(AdmError::new(
                "system_id is required when deleting gameplay system",
            ));
        }
        if request.delete_custom {
            state
                .gameplay_systems
                .custom
                .retain(|option| option.id != system_id);
            state
                .gameplay_systems
                .selected
                .retain(|selected| selected != &system_id);
            state.gameplay_systems.weights.remove(&system_id);
            state.gameplay_systems.core_loops.remove(&system_id);
            return Ok(self.view_model(state));
        }
        if !system_id.is_empty() {
            if let Some(selected) = request.selected {
                if selected {
                    if !state
                        .gameplay_systems
                        .selected
                        .iter()
                        .any(|item| item == &system_id)
                    {
                        state.gameplay_systems.selected.push(system_id.clone());
                    }
                } else {
                    state
                        .gameplay_systems
                        .selected
                        .retain(|item| item != &system_id);
                }
            }
            if let Some(weight) = &request.weight {
                state.gameplay_systems.weights.insert(
                    system_id.clone(),
                    GameplaySystemWeight {
                        weight: weight.clone(),
                        weight_type: "percent".to_string(),
                        extra: BTreeMap::new(),
                    },
                );
            }
            if let Some(core_loop) = &request.core_loop {
                state
                    .gameplay_systems
                    .core_loops
                    .insert(system_id.clone(), core_loop.trim().to_string());
            }
        }
        if !request.interview_answers.is_empty() {
            state.gameplay_systems.interview.answers = request
                .interview_answers
                .iter()
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect();
        }
        Ok(self.view_model(state))
    }

    pub fn reset_project_state(&self, state: &mut ProjectState) -> DesignWorkbenchView {
        *state = self.empty_project_state();
        self.view_model(state)
    }

    pub fn autosave_state_summary(
        &self,
        state: &ProjectState,
        autosave_file: Option<&str>,
        dirty: bool,
    ) -> AdmResult<DesignAutosaveReport> {
        let normalized = self.normalize_project_state(state.clone());
        let serialized = serde_json::to_string(&normalized).map_err(|error| {
            AdmError::new(format!("failed to serialize project state: {error}"))
        })?;
        Ok(DesignAutosaveReport {
            schema_version: 1,
            project_name: normalized.project_name,
            state_hash: hash_text(&serialized),
            dirty,
            autosave_file: autosave_file
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("drafts/current/autosave_state.json")
                .to_string(),
            node_count: normalized.nodes.len(),
            selected_gameplay_system_count: normalized.gameplay_systems.selected.len(),
        })
    }

    pub fn list_project_templates(
        &self,
        include_internal: bool,
    ) -> AdmResult<DesignTemplateListReport> {
        let report = self
            .template_loader()?
            .load_project_templates_report(include_internal)?;
        Ok(DesignTemplateListReport {
            templates: report
                .templates
                .into_iter()
                .map(|template| DesignTemplateSummary::from(template.meta))
                .collect(),
            warnings: report.warnings,
        })
    }

    pub fn custom_project_templates_dir(&self) -> AdmResult<std::path::PathBuf> {
        Ok(self.template_loader()?.custom_project_templates_dir())
    }

    pub fn apply_project_template(
        &self,
        state: &mut ProjectState,
        template_id: &str,
        project_name_prefix: &str,
    ) -> AdmResult<DesignTemplateSelectionReport> {
        let template = self.template_loader()?.find_project_template(template_id)?;
        let mut template_state = template.project_state;
        let state_object = template_state.as_object_mut().ok_or_else(|| {
            AdmError::new(format!(
                "project template {} has invalid projectState",
                template.meta.id
            ))
        })?;
        state_object.remove("aiInterview");
        state_object.remove("ai_interview");
        let template_state: ProjectState =
            serde_json::from_value(template_state).map_err(|error| {
                AdmError::new(format!(
                    "project template {} has invalid projectState: {error}",
                    template.meta.id
                ))
            })?;
        let mut template_state = template_state;
        infer_gameplay_systems_from_nodes(&mut template_state);
        let mut normalized = self.normalize_project_state(template_state);
        let prefix = validated_display_text(project_name_prefix, "project_name_prefix", 32, true)?;
        let prefix = if prefix.is_empty() {
            "范本："
        } else {
            prefix
        };
        let preferred_name = if prefix.to_ascii_lowercase().starts_with("template")
            && !template.meta.game_name.trim().is_empty()
        {
            template.meta.game_name.trim()
        } else {
            template.meta.name.trim()
        };
        let template_name = if preferred_name.is_empty() {
            template.meta.id.as_str()
        } else {
            preferred_name
        };
        let template_name = validated_display_text(template_name, "template_name", 120, false)?;
        normalized.project_name = if prefix.ends_with(':') {
            format!("{prefix} {template_name}")
        } else {
            format!("{prefix}{template_name}")
        };
        let view = self.view_model(&normalized);
        *state = normalized;
        Ok(DesignTemplateSelectionReport {
            template_id: template.meta.id,
            project_name: state.project_name.clone(),
            view,
        })
    }

    pub fn save_project_template(
        &self,
        state: &ProjectState,
        template_name: &str,
        target_scale: &str,
        overwrite: bool,
    ) -> AdmResult<DesignTemplateSaveReport> {
        let normalized = self.normalize_project_state(state.clone());
        let mut project_state = serde_json::to_value(normalized).map_err(|error| {
            AdmError::new(format!("failed to serialize project state: {error}"))
        })?;
        if let Some(object) = project_state.as_object_mut() {
            object.remove("aiInterview");
        }
        let saved = self.template_loader()?.save_custom_project_template(
            template_name,
            target_scale,
            project_state,
            overwrite,
        )?;
        let serialized = serde_json::to_string(&saved.template.project_state).map_err(|error| {
            AdmError::new(format!("failed to serialize saved template state: {error}"))
        })?;
        Ok(DesignTemplateSaveReport {
            template_id: saved.template.meta.id,
            template_name: saved.template.meta.name,
            target_scale: saved.template.meta.target_scale,
            target_file_name: saved.template.meta.file_name,
            state_hash: hash_text(&serialized),
            overwritten: saved.overwritten,
        })
    }

    pub fn save_template_snapshot(
        &self,
        state: &ProjectState,
        template_name: &str,
        target_scale: &str,
    ) -> AdmResult<DesignTemplateSaveReport> {
        self.save_project_template(state, template_name, target_scale, false)
    }

    pub fn delete_project_template(
        &self,
        template_id: &str,
    ) -> AdmResult<DesignTemplateDeleteReport> {
        let deleted = self
            .template_loader()?
            .delete_custom_project_template(template_id)?;
        Ok(DesignTemplateDeleteReport {
            template_id: deleted.template_id,
            template_name: deleted.template_name,
            target_scale: deleted.target_scale,
            target_file_name: deleted.file_name,
        })
    }

    pub fn export_design(&self, state: &ProjectState, format: &str) -> AdmResult<String> {
        self.export_design_with_locale(state, format, ArtifactLocale::default())
    }

    pub fn export_design_with_locale(
        &self,
        state: &ProjectState,
        format: &str,
        artifact_locale: ArtifactLocale,
    ) -> AdmResult<String> {
        let normalized = self.normalize_project_state(state.clone());
        let view = self.view_model(&normalized);
        match format.trim().to_ascii_lowercase().as_str() {
            "json" => {
                let mut payload = serde_json::to_value(&normalized).map_err(|error| {
                    AdmError::new(format!("failed to serialize design JSON: {error}"))
                })?;
                payload["artifact_locale"] = serde_json::json!(artifact_locale);
                serde_json::to_string_pretty(&payload).map_err(|error| {
                    AdmError::new(format!("failed to serialize design JSON: {error}"))
                })
            }
            "markdown" | "md" => Ok(render_design_markdown(&view, artifact_locale)),
            "text" => Ok(render_design_text(&view, artifact_locale)),
            "prompt" => Ok(render_design_prompt(&view, artifact_locale)),
            other => Err(AdmError::new(format!("unsupported export format: {other}"))),
        }
    }

    fn template_loader(&self) -> AdmResult<&DesignDataLoader> {
        self.template_loader.as_ref().ok_or_else(|| {
            AdmError::new("project template storage is not configured for this runtime")
        })
    }
}

impl From<ProjectTemplateMeta> for DesignTemplateSummary {
    fn from(meta: ProjectTemplateMeta) -> Self {
        Self {
            template_id: meta.id,
            source: meta.source,
            source_label: meta.source_label,
            name: meta.name,
            game_name: meta.game_name,
            target_scale: meta.target_scale,
            scale_label: meta.scale_label,
            quality_tier: meta.quality_tier,
            summary: meta.summary,
            visibility: meta.visibility,
            file_name: meta.file_name,
            order: meta.order,
            analysis: meta.analysis,
            verification: meta.verification,
        }
    }
}

fn validated_display_text<'a>(
    value: &'a str,
    field: &str,
    max_chars: usize,
    allow_empty: bool,
) -> AdmResult<&'a str> {
    let value = value.trim();
    if !allow_empty && value.is_empty() {
        return Err(AdmError::new(format!("{field} must not be empty")));
    }
    if value.chars().any(char::is_control) {
        return Err(AdmError::new(format!(
            "{field} must not contain control characters"
        )));
    }
    if value.chars().count() > max_chars {
        return Err(AdmError::new(format!(
            "{field} must not exceed {max_chars} characters"
        )));
    }
    Ok(value)
}

const INFERRED_GAMEPLAY_SYSTEM_PRIORITY: [&str; 12] = [
    "input_control",
    "action_rule",
    "objective",
    "settlement",
    "progression",
    "buildcraft",
    "randomness",
    "meta_structure",
    "resource_economy",
    "social_competition",
    "content_delivery",
    "liveops_event",
];

fn infer_gameplay_systems_from_nodes(state: &mut ProjectState) {
    if !state.gameplay_systems.selected.is_empty() {
        return;
    }
    let mut inferred = BTreeSet::<&'static str>::new();
    for (node_id, node) in &state.nodes {
        let has_content = !node.design_entities.is_empty()
            || matches!(
                node.decision_state,
                DecisionState::Selected | DecisionState::Completed | DecisionState::Risk
            )
            || node.checklist.values().any(|checked| *checked);
        if has_content && let Some(system_id) = gameplay_system_for_node(node_id) {
            inferred.insert(system_id);
        }
    }
    let selected = INFERRED_GAMEPLAY_SYSTEM_PRIORITY
        .iter()
        .filter(|system_id| inferred.contains(*system_id))
        .map(|system_id| (*system_id).to_string())
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return;
    }

    let mut raw_weights = selected
        .iter()
        .map(|system_id| {
            let weight = state
                .gameplay_systems
                .weights
                .get(system_id)
                .map(|entry| numeric_weight(&entry.weight))
                .unwrap_or(1.0);
            (system_id.clone(), weight.max(0.0))
        })
        .collect::<Vec<_>>();
    if raw_weights.iter().all(|(_, weight)| *weight == 0.0) {
        for (_, weight) in &mut raw_weights {
            *weight = 1.0;
        }
    }
    let total = raw_weights.iter().map(|(_, weight)| *weight).sum::<f64>();
    let scaled = raw_weights
        .iter()
        .map(|(system_id, weight)| (system_id.clone(), (*weight / total) * 100.0))
        .collect::<Vec<_>>();
    let mut integer_weights = scaled
        .iter()
        .map(|(system_id, weight)| (system_id.clone(), (*weight as i64).max(1)))
        .collect::<Vec<_>>();
    let mut difference = 100
        - integer_weights
            .iter()
            .map(|(_, weight)| *weight)
            .sum::<i64>();
    let mut ranked = (0..scaled.len()).collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        let left_fraction = scaled[*left].1.fract();
        let right_fraction = scaled[*right].1.fract();
        right_fraction
            .partial_cmp(&left_fraction)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                scaled[*right]
                    .1
                    .partial_cmp(&scaled[*left].1)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });
    while difference > 0 {
        for index in &ranked {
            integer_weights[*index].1 += 1;
            difference -= 1;
            if difference == 0 {
                break;
            }
        }
    }
    while difference < 0 {
        let mut changed = false;
        let mut indexes = (0..integer_weights.len()).collect::<Vec<_>>();
        indexes.sort_by_key(|index| std::cmp::Reverse(integer_weights[*index].1));
        for index in indexes {
            if integer_weights[index].1 <= 1 {
                continue;
            }
            integer_weights[index].1 -= 1;
            difference += 1;
            changed = true;
            if difference == 0 {
                break;
            }
        }
        if !changed {
            break;
        }
    }

    state.gameplay_systems.selected = selected;
    state.gameplay_systems.weights = integer_weights
        .into_iter()
        .map(|(system_id, weight)| {
            (
                system_id,
                GameplaySystemWeight {
                    weight: Value::from(weight),
                    weight_type: "percent".to_string(),
                    extra: BTreeMap::new(),
                },
            )
        })
        .collect();
}

fn numeric_weight(value: &Value) -> f64 {
    match value {
        Value::Number(number) => number.as_f64().unwrap_or(0.0),
        Value::String(text) => text.trim().parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn gameplay_system_for_node(node_id: &str) -> Option<&'static str> {
    if node_id.starts_with("liveops_") {
        return Some("liveops_event");
    }
    match node_id {
        "input_control_decision" => Some("input_control"),
        "action_rule_decision" => Some("action_rule"),
        "objective_system_decision" => Some("objective"),
        "settlement_system_decision" => Some("settlement"),
        "progression_system_decision" => Some("progression"),
        "build_system_decision" => Some("buildcraft"),
        "randomness_system_decision" => Some("randomness"),
        "meta_structure_decision" => Some("meta_structure"),
        "item_resource_content_decision" | "balance_economy_decision" | "economy_loop_decision" => {
            Some("resource_economy")
        }
        "content_type_decision" | "level_space_decision" | "quest_event_decision" => {
            Some("content_delivery")
        }
        "social_relationship_decision"
        | "social_collaboration_decision"
        | "social_competition_decision" => Some("social_competition"),
        _ => None,
    }
}

fn gameplay_custom_system_id(value: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = false;
    for ch in value.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_separator = false;
        } else if !last_was_separator && !out.is_empty() {
            out.push('_');
            last_was_separator = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "custom_system".to_string()
    } else {
        format!("custom_{out}")
    }
}

fn render_design_markdown(view: &DesignWorkbenchView, artifact_locale: ArtifactLocale) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "<!-- artifact_locale: {} -->\n",
        artifact_locale.as_str()
    ));
    output.push_str(&format!("# {}\n\n", view.project_name));
    output.push_str(if artifact_locale == ArtifactLocale::ZhCn {
        "## 覆盖率\n"
    } else {
        "## Coverage\n"
    });
    output.push_str(&format!(
        "- {}: {}/{} ({}%)\n",
        if artifact_locale == ArtifactLocale::ZhCn {
            "节点"
        } else {
            "nodes"
        },
        view.project_coverage.done_nodes,
        view.project_coverage.total_nodes,
        view.project_coverage.node_percent
    ));
    output.push_str(&format!(
        "- {}: {}/{} ({}%)\n\n",
        if artifact_locale == ArtifactLocale::ZhCn {
            "检查项"
        } else {
            "checklist"
        },
        view.project_coverage.done_checklist,
        view.project_coverage.total_checklist,
        view.project_coverage.checklist_percent
    ));
    output.push_str(if artifact_locale == ArtifactLocale::ZhCn {
        "## 设计节点\n"
    } else {
        "## Nodes\n"
    });
    for node in &view.nodes {
        if artifact_locale == ArtifactLocale::ZhCn {
            output.push_str(&format!(
                "- {} [{}]: 状态={}, 检查项={}%, L5 实体={}\n",
                node.name,
                node.node_id,
                localized_decision_state(node.effective_state.as_str(), artifact_locale),
                node.progress.percent,
                node.l5_entity_count
            ));
        } else {
            output.push_str(&format!(
                "- {} [{}]: state={}, checklist={}%, l5_entities={}\n",
                node.name,
                node.node_id,
                node.effective_state.as_str(),
                node.progress.percent,
                node.l5_entity_count
            ));
        }
    }
    output
}

fn render_design_text(view: &DesignWorkbenchView, artifact_locale: ArtifactLocale) -> String {
    let mut output = if artifact_locale == ArtifactLocale::ZhCn {
        format!(
            "artifact_locale={}\n{}\n覆盖率: 节点 {}%, 检查项 {}%\n",
            artifact_locale.as_str(),
            view.project_name,
            view.project_coverage.node_percent,
            view.project_coverage.checklist_percent
        )
    } else {
        format!(
            "artifact_locale={}\n{}\ncoverage: {}% nodes, {}% checklist\n",
            artifact_locale.as_str(),
            view.project_name,
            view.project_coverage.node_percent,
            view.project_coverage.checklist_percent
        )
    };
    for node in &view.nodes {
        output.push_str(&format!(
            "{}: {} ({}%)\n",
            node.node_id,
            localized_decision_state(node.effective_state.as_str(), artifact_locale),
            node.progress.percent
        ));
    }
    output
}

fn render_design_prompt(view: &DesignWorkbenchView, artifact_locale: ArtifactLocale) -> String {
    format!(
        "{}\n\n{}",
        if artifact_locale == ArtifactLocale::ZhCn {
            "请将这份 AutoDesignMaker 设计状态作为项目的权威需求说明。"
        } else {
            "Use this AutoDesignMaker design state as the authoritative project brief."
        },
        render_design_markdown(view, artifact_locale)
    )
}

fn localized_decision_state(value: &str, artifact_locale: ArtifactLocale) -> &str {
    if artifact_locale == ArtifactLocale::EnUs {
        return value;
    }
    match value {
        "not_started" => "未开始",
        "selected" => "已选择",
        "completed" => "已完成",
        "risk" => "存在风险",
        "not_applicable" => "不适用",
        _ => value,
    }
}

#[derive(Debug, Clone)]
pub struct SaveApplicationService {
    service: SaveService,
    draft_root: std::path::PathBuf,
}

impl SaveApplicationService {
    pub fn new(root: impl AsRef<std::path::Path>, session_id: &str) -> AdmResult<Self> {
        let root = root.as_ref().to_path_buf();
        Ok(Self {
            service: SaveService::new(&root, session_id)?,
            draft_root: root.join("drafts").join(session_id),
        })
    }

    pub fn with_pid(
        root: impl AsRef<std::path::Path>,
        session_id: &str,
        pid: u32,
    ) -> AdmResult<Self> {
        let root = root.as_ref().to_path_buf();
        Ok(Self {
            service: SaveService::with_pid(&root, session_id, pid)?,
            draft_root: root.join("drafts").join(session_id),
        })
    }

    pub fn draft_root(&self) -> &std::path::Path {
        &self.draft_root
    }

    pub fn autosave(&self, state: &ProjectState) -> AdmResult<()> {
        self.service.write_autosave(state)?;
        Ok(())
    }

    pub fn autosave_state(&self) -> AdmResult<Option<ProjectState>> {
        self.service.read_autosave()
    }

    pub fn recover_to_unsaved_state(&self, fallback: &ProjectState) -> AdmResult<DraftMeta> {
        self.service.recover_to_unsaved_state(fallback)
    }

    pub fn list_saves(&self) -> AdmResult<SaveIndex> {
        self.service.list_saves()
    }

    pub fn current_draft_save_id(&self) -> AdmResult<Option<String>> {
        self.service.current_draft_save_id()
    }

    pub fn create_save(
        &self,
        display_name: &str,
        state: &ProjectState,
    ) -> AdmResult<SaveServiceReport> {
        self.service.create_save(display_name, state)
    }

    pub fn create_blank_save(&self, display_name: &str) -> AdmResult<SaveServiceReport> {
        self.service.create_blank_save(display_name)
    }

    pub fn create_blank_save_from_state(
        &self,
        display_name: &str,
        state: &ProjectState,
    ) -> AdmResult<SaveServiceReport> {
        self.service
            .create_blank_save_from_state(display_name, state)
    }

    pub fn create_iteration_save(
        &self,
        display_name: &str,
        state: &ProjectState,
        change_type: &str,
        requested_version: &str,
        iteration_spec_path: &str,
    ) -> AdmResult<SaveServiceReport> {
        self.service.create_iteration_save(
            display_name,
            state,
            change_type,
            requested_version,
            iteration_spec_path,
        )
    }

    pub fn sync_current_save(
        &self,
        state: &ProjectState,
        reason: &str,
    ) -> AdmResult<SaveServiceReport> {
        self.service.sync_current_save(state, reason)
    }

    pub fn load_save(&self, save_id: &str) -> AdmResult<LoadedSave> {
        self.service.load_save(save_id)
    }

    pub fn delete_save(&self, save_id: &str) -> AdmResult<SaveIndex> {
        self.service.delete_save(save_id)
    }

    pub fn rename_save(&self, save_id: &str, display_name: &str) -> AdmResult<SaveIndex> {
        self.service.rename_save(save_id, display_name)
    }

    pub fn release_current_lock(&self) -> AdmResult<()> {
        self.service.release_current_lock()
    }

    pub fn acquire_current_lock(&self) -> AdmResult<()> {
        self.service.acquire_current_lock()
    }

    pub fn repair_blank_save_progress(
        &self,
        save_id: &str,
        apply: bool,
    ) -> AdmResult<BlankSaveRepairReport> {
        self.service.repair_blank_save_progress(save_id, apply)
    }

    pub fn audit_parallel_isolation(&self) -> AdmResult<ParallelIsolationAuditReport> {
        self.service.audit_parallel_isolation()
    }

    pub fn repair_parallel_save_contamination(
        &self,
        apply: bool,
    ) -> AdmResult<ParallelRepairReport> {
        self.service.repair_parallel_save_contamination(apply)
    }
}

#[derive(Debug, Clone)]
pub struct AiConfigApplicationService {
    service: AiConfigService,
}

impl AiConfigApplicationService {
    pub fn new(root: impl AsRef<std::path::Path>) -> AdmResult<Self> {
        Ok(Self {
            service: AiConfigService::new(root)?,
        })
    }

    pub fn load_or_default(&self) -> AdmResult<AiConfig> {
        self.service.load_or_default()
    }

    pub fn load_redacted(&self) -> AdmResult<AiConfig> {
        self.service.load_redacted()
    }

    pub fn save(&self, config: &AiConfig) -> AdmResult<AiConfigValidationReport> {
        self.service.save(config)
    }

    pub fn save_redacted(&self, config: &AiConfig) -> AdmResult<AiConfigValidationReport> {
        self.service.save_redacted(config)
    }

    pub fn validate(&self, config: &AiConfig) -> AiConfigValidationReport {
        self.service.validate(config)
    }

    pub fn completion_adapter_spec(&self, config: &AiConfig) -> AdmResult<CompletionAdapterSpec> {
        self.service.completion_adapter_spec(config)
    }

    pub fn list_ai_config_descriptors(&self) -> Vec<AiConfigDescriptorView> {
        self.service.descriptor_views()
    }

    pub fn preview_ai_resolution(
        &self,
        config: &AiConfig,
        category_id: &str,
    ) -> AdmResult<AiResolutionView> {
        self.service.preview_resolution(config, category_id)
    }

    pub fn probe_ai_cli(&self, config: &AiConfig, category_id: &str) -> AdmResult<AiCliProbeView> {
        self.service.probe_cli(config, category_id)
    }

    pub fn probe_ai_api(&self, config: &AiConfig, category_id: &str) -> AdmResult<AiApiProbeView> {
        self.service.probe_api(config, category_id)
    }

    pub fn active_completion_entry(&self, config: &AiConfig) -> AdmResult<ApiEntry> {
        self.service.active_completion_entry(config)
    }
}

#[derive(Debug, Clone)]
pub struct AiInterviewApplicationService {
    service: AiInterviewService,
}

impl AiInterviewApplicationService {
    pub fn new(specs: Vec<DesignNodeSpec>) -> Self {
        Self {
            service: AiInterviewService::new(specs),
        }
    }

    pub fn handle_payload_json(
        &self,
        state: &mut ProjectState,
        schema_mode: AiSchemaMode,
        payload_json: &str,
    ) -> AdmResult<AiInterviewTurnReport> {
        self.service
            .handle_payload_json(state, schema_mode, payload_json)
    }

    pub fn handle_payload(
        &self,
        state: &mut ProjectState,
        schema_mode: AiSchemaMode,
        payload: AiResponsePayload,
    ) -> AdmResult<AiInterviewTurnReport> {
        self.service.handle_payload(state, schema_mode, payload)
    }

    pub fn mark_inaccurate(
        &self,
        state: &mut ProjectState,
        node_id: &str,
        reason: &str,
    ) -> AdmResult<AiInterviewState> {
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(AdmError::new("reason must not be empty"));
        }
        state.ai_interview.status = "needs_revision".to_string();
        state.ai_interview.backend_stage = "needs_revision".to_string();
        state.ai_interview.last_error =
            format!("AI output marked inaccurate for {node_id}: {reason}");
        state
            .ai_interview
            .summary
            .v1
            .last_user_corrections
            .push(serde_json::json!({
                "nodeId": node_id,
                "reason": reason,
            }));
        state.ai_interview.updated_at = format!("unix:{}", unix_timestamp());
        Ok(state.ai_interview.clone())
    }

    pub fn save_manual_archive_marker(
        &self,
        state: &mut ProjectState,
        archive_path: Option<&str>,
    ) -> AdmResult<AiInterviewState> {
        let timestamp = unix_timestamp();
        let archive_path = archive_path
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("ai_archives/manual/manual_{timestamp}.json"));
        state.ai_interview.last_manual_archive_path = archive_path;
        state.ai_interview.last_archived_at = format!("unix:{timestamp}");
        state.ai_interview.updated_at = format!("unix:{timestamp}");
        state.ai_interview.framework_memory.review_chains.insert(
            format!("manual_archive_{timestamp}"),
            "manual_archive_saved".into(),
        );
        Ok(state.ai_interview.clone())
    }
}

#[derive(Debug, Clone)]
pub struct PipelineApplicationService {
    service: PipelineService,
}

impl PipelineApplicationService {
    pub fn new(registry: PipelineRegistry) -> AdmResult<Self> {
        Ok(Self {
            service: PipelineService::new(registry)?,
        })
    }

    pub fn topological_order(&self) -> AdmResult<Vec<String>> {
        self.service.topological_order()
    }

    pub fn stage_specs(&self) -> Vec<StageSpec> {
        self.service.registry().stages.clone()
    }

    pub fn resolve_stage_input(&self, input: &str) -> AdmResult<String> {
        self.service.resolve_stage_input(input)
    }

    pub fn resolve_range(
        &self,
        from_stage_input: &str,
        to_stage_input: &str,
    ) -> AdmResult<ResolvedPipelineRange> {
        self.service.resolve_range(from_stage_input, to_stage_input)
    }

    pub fn request_stop(state: &mut PipelineRunState) {
        PipelineService::request_stop(state);
    }

    pub fn confirm_style(state: &mut PipelineRunState, stage_id: &str, message: &str) {
        Self::confirm_style_selection(state, stage_id, "", "", message);
    }

    pub fn confirm_style_selection(
        state: &mut PipelineRunState,
        stage_id: &str,
        selected_style_id: &str,
        notes: &str,
        message: &str,
    ) {
        let mut outputs = state
            .stages
            .get(stage_id)
            .and_then(|runtime| runtime.result.as_ref())
            .map(|result| result.outputs.clone())
            .unwrap_or_default();
        let mut style_options = app_style_options_from_outputs(&outputs);
        let selected_style_id = app_selected_style_id(selected_style_id, message, &style_options);
        let notes = app_confirmation_notes(notes, message);
        let selected_option =
            app_mark_selected_style_option(&mut style_options, &selected_style_id);
        if !style_options.is_empty() {
            outputs.insert("style_options".to_string(), Value::Array(style_options));
        }
        let confirmation =
            app_style_confirmation_payload(&selected_style_id, &selected_option, &notes, message);
        outputs.insert(
            "confirmation_status".to_string(),
            Value::String("approved".to_string()),
        );
        outputs.insert(
            "selected_style_id".to_string(),
            Value::String(selected_style_id.clone()),
        );
        outputs.insert("style_confirmation".to_string(), confirmation);
        state.status = "style_confirmed".to_string();
        state.current_stage_id = Some(stage_id.to_string());
        state.stages.insert(
            stage_id.to_string(),
            PipelineStageRuntime {
                stage_id: stage_id.to_string(),
                status: StageStatus::Success,
                started_at: String::new(),
                completed_at: format!("unix:{}", unix_timestamp()),
                result: Some(PipelineStageResult {
                    status: StageStatus::Success,
                    outputs,
                    errors: Vec::new(),
                    warnings: Vec::new(),
                    message: message.to_string(),
                }),
            },
        );
    }

    pub fn run_range<E>(
        &self,
        state: &mut PipelineRunState,
        from_stage_id: &str,
        to_stage_id: &str,
        executor: &E,
    ) -> AdmResult<PipelineRunReport>
    where
        E: StageExecutor,
    {
        self.service
            .run_range(state, from_stage_id, to_stage_id, executor)
    }

    pub fn run_range_with_observer<E, O>(
        &self,
        state: &mut PipelineRunState,
        from_stage_id: &str,
        to_stage_id: &str,
        executor: &E,
        observer: &O,
    ) -> AdmResult<PipelineRunReport>
    where
        E: StageExecutor,
        O: PipelineRunObserver + ?Sized,
    {
        self.service
            .run_range_with_observer(state, from_stage_id, to_stage_id, executor, observer)
    }
}

fn app_style_options_from_outputs(outputs: &BTreeMap<String, Value>) -> Vec<Value> {
    outputs
        .get("style_options")
        .or_else(|| outputs.get("styleOptions"))
        .and_then(Value::as_array)
        .cloned()
        .or_else(|| {
            outputs
                .get("style_options_document")
                .and_then(|document| document.get("options"))
                .and_then(Value::as_array)
                .cloned()
        })
        .unwrap_or_default()
}

fn app_selected_style_id(
    selected_style_id: &str,
    message: &str,
    style_options: &[Value],
) -> String {
    let explicit = selected_style_id.trim();
    if !explicit.is_empty() {
        return explicit.to_string();
    }
    if let Some(parsed) = app_message_field(message, "style") {
        return parsed;
    }
    style_options
        .iter()
        .find(|option| {
            option
                .get("selected")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .or_else(|| {
            style_options.iter().find(|option| {
                option
                    .get("recommended")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
        })
        .or_else(|| style_options.first())
        .map(app_style_option_id)
        .unwrap_or_default()
}

fn app_confirmation_notes(notes: &str, message: &str) -> String {
    let explicit = notes.trim();
    if !explicit.is_empty() {
        return explicit.to_string();
    }
    app_message_field(message, "notes").unwrap_or_default()
}

fn app_message_field(message: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    message
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix(&prefix).map(str::trim))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn app_mark_selected_style_option(style_options: &mut [Value], selected_style_id: &str) -> Value {
    let mut selected = Value::Null;
    for option in style_options {
        let is_selected =
            !selected_style_id.is_empty() && app_style_option_id(option) == selected_style_id;
        if let Some(object) = option.as_object_mut() {
            object.insert("selected".to_string(), Value::Bool(is_selected));
        }
        if is_selected {
            selected = option.clone();
        }
    }
    selected
}

fn app_style_confirmation_payload(
    selected_style_id: &str,
    selected_option: &Value,
    notes: &str,
    message: &str,
) -> Value {
    serde_json::json!({
        "schema_version": 1,
        "generated_at": format!("unix:{}", unix_timestamp()),
        "status": "approved",
        "mode": "manual",
        "selected_style_id": selected_style_id,
        "selected_title": app_string_field(selected_option, "title"),
        "selected_image_path": app_string_field(selected_option, "image_path"),
        "notes": notes,
        "message": message,
        "selected_option": selected_option,
    })
}

fn app_style_option_id(option: &Value) -> String {
    let style_id = app_string_field(option, "style_id");
    if style_id.is_empty() {
        app_string_field(option, "option_id")
    } else {
        style_id
    }
}

fn app_string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[derive(Debug, Clone)]
pub struct ArtifactApplicationService {
    service: ArtifactService,
}

impl ArtifactApplicationService {
    pub fn new(registry: ArtifactRegistry) -> AdmResult<Self> {
        Ok(Self {
            service: ArtifactService::new(registry)?,
        })
    }

    pub fn dependency_graph(&self) -> DependencyGraph {
        self.service.build_dependency_graph()
    }

    pub fn preflight_stage_contract(
        &self,
        stage: u32,
        evidence: &ArtifactEvidenceSet,
    ) -> PreflightReport {
        self.service.preflight_stage_contract(stage, evidence)
    }

    pub fn run_review_pipeline(
        &self,
        stage: u32,
        evidence: &ArtifactEvidenceSet,
    ) -> ArtifactReviewReport {
        self.service.run_review_pipeline(stage, evidence)
    }

    pub fn run_artifact_validators(
        &self,
        stage: u32,
        manifest: &ArtifactLayerManifest,
        review_report: &ArtifactReviewReport,
        evidence: &ArtifactEvidenceSet,
    ) -> ArtifactValidationLayerReport {
        self.service
            .run_artifact_validators(stage, manifest, review_report, evidence)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackagingApplicationService {
    service: PackagingService,
}

impl PackagingApplicationService {
    pub fn new() -> Self {
        Self {
            service: PackagingService::new(),
        }
    }

    pub fn package_current_project(&self, sources: PackagingSources) -> PackageRunResult {
        self.service.run_package(sources)
    }

    pub fn package_current_project_from_values(
        &self,
        integration: Value,
        actual_project_file_audit: Value,
        unity_validation_summary: Value,
    ) -> PackageRunResult {
        self.package_current_project(PackagingSources {
            integration,
            actual_project_file_audit,
            unity_validation_summary,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct PatchApplicationService {
    service: PatchService,
}

impl PatchApplicationService {
    pub fn new() -> Self {
        Self {
            service: PatchService::new(),
        }
    }

    pub fn analyze_request_shell(
        &mut self,
        request: &str,
        tasks: Vec<PatchTask>,
    ) -> AdmResult<PatchRecord> {
        self.service.analyze_request_shell(request, tasks)
    }

    pub fn list(&self) -> Vec<PatchRecord> {
        self.service.list()
    }

    pub fn read(&self, patch_id: &str) -> AdmResult<PatchRecord> {
        self.service.get(patch_id)
    }

    pub fn set_status(&mut self, patch_id: &str, status: PatchStatus) -> AdmResult<PatchRecord> {
        self.service.set_status(patch_id, status)
    }

    pub fn approved_context(&self) -> Vec<PatchRecord> {
        self.service.approved_context()
    }

    pub fn replace_records(&mut self, records: Vec<PatchRecord>) {
        self.service = PatchService::new();
        for record in records {
            self.service.write(record);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SdkKnowledgeApplicationService {
    service: SdkKnowledgeService,
    store: Option<SdkKnowledgeBase>,
    dirty_sdk_ids: BTreeSet<String>,
    legacy_migration: LegacySdkMigrationReport,
}

impl SdkKnowledgeApplicationService {
    pub fn new() -> Self {
        Self {
            service: SdkKnowledgeService::new(),
            store: None,
            dirty_sdk_ids: BTreeSet::new(),
            legacy_migration: LegacySdkMigrationReport::default(),
        }
    }

    pub fn open(
        resource_root: impl AsRef<std::path::Path>,
        data_root: impl AsRef<std::path::Path>,
    ) -> AdmResult<Self> {
        let store = SdkKnowledgeBase::from_project_and_data_roots(&resource_root, &data_root);
        store.initialize()?;
        let legacy_migration =
            store.migrate_legacy_flat_file(store.root().join(LEGACY_DESKTOP_SDK_FILE))?;
        let mut service = SdkKnowledgeService::new();
        service.replace_specs(store.list_specs()?);
        Ok(Self {
            service,
            store: Some(store),
            dirty_sdk_ids: BTreeSet::new(),
            legacy_migration,
        })
    }

    pub fn legacy_migration(&self) -> &LegacySdkMigrationReport {
        &self.legacy_migration
    }

    pub fn store_root(&self) -> Option<&std::path::Path> {
        self.store.as_ref().map(|store| store.root())
    }

    pub fn add_placeholder(&mut self, sdk_id: &str, name: &str) -> AdmResult<SdkSpec> {
        let spec = self.service.add_placeholder(sdk_id, name)?;
        self.dirty_sdk_ids.insert(spec.sdk_id.clone());
        Ok(spec)
    }

    pub fn add_placeholder_with_source_url(
        &mut self,
        sdk_id: &str,
        name: &str,
        source_url: &str,
    ) -> AdmResult<SdkSpec> {
        let spec = self
            .service
            .add_placeholder_with_source_url(sdk_id, name, source_url)?;
        self.dirty_sdk_ids.insert(spec.sdk_id.clone());
        Ok(spec)
    }

    pub fn ingest_ai_extracted_spec(&mut self, spec: SdkSpec) -> SdkSpec {
        let spec = self.service.ingest_ai_extracted_spec(spec);
        self.dirty_sdk_ids.insert(spec.sdk_id.clone());
        spec
    }

    pub fn index(&self) -> SdkIndex {
        self.service.index().clone()
    }

    pub fn list_specs(&self) -> Vec<SdkSpec> {
        self.service.list_specs()
    }

    pub fn set_review_status(
        &mut self,
        sdk_id: &str,
        status: SdkReviewStatus,
    ) -> AdmResult<SdkSpec> {
        let spec = self.service.set_review_status(sdk_id, status)?;
        self.dirty_sdk_ids.insert(spec.sdk_id.clone());
        Ok(spec)
    }

    pub fn approved_context(&self) -> String {
        self.service.approved_context()
    }

    pub fn replace_specs(&mut self, specs: Vec<SdkSpec>) {
        self.dirty_sdk_ids
            .extend(specs.iter().map(|spec| spec.sdk_id.clone()));
        self.service.replace_specs(specs);
    }

    pub fn persist(&mut self) -> AdmResult<()> {
        let Some(store) = self.store.clone() else {
            self.dirty_sdk_ids.clear();
            return Ok(());
        };
        store.validate()?;
        let dirty_ids = self.dirty_sdk_ids.iter().cloned().collect::<Vec<_>>();
        for sdk_id in dirty_ids {
            let spec = self
                .service
                .list_specs()
                .into_iter()
                .find(|spec| spec.sdk_id == sdk_id)
                .ok_or_else(|| AdmError::new(format!("missing dirty SDK spec: {sdk_id}")))?;
            store.write_spec(spec)?;
            self.dirty_sdk_ids.remove(&sdk_id);
        }
        self.reload()
    }

    pub fn reload(&mut self) -> AdmResult<()> {
        let Some(store) = self.store.as_ref() else {
            return Ok(());
        };
        self.service.replace_specs(store.list_specs()?);
        self.dirty_sdk_ids.clear();
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SkillOverlayApplicationService {
    repository: SkillOverlayRepository,
}

impl SkillOverlayApplicationService {
    pub fn open(
        resource_root: impl AsRef<std::path::Path>,
        data_root: impl AsRef<std::path::Path>,
    ) -> AdmResult<Self> {
        let repository =
            SkillOverlayRepository::from_project_and_data_roots(resource_root, data_root);
        repository.initialize()?;
        Ok(Self { repository })
    }

    pub fn overlay_root(&self) -> &std::path::Path {
        self.repository.root()
    }

    pub fn list(&self) -> AdmResult<Vec<SkillRecord>> {
        self.repository.list()
    }

    pub fn write_json(&self, skill_id: &str, value: &Value) -> AdmResult<SkillRecord> {
        self.repository.write_json(skill_id, value)
    }

    pub fn write_markdown(&self, skill_id: &str, text: &str) -> AdmResult<SkillRecord> {
        self.repository.write_markdown(skill_id, text)
    }

    pub fn remove(&self, skill_id: &str) -> AdmResult<()> {
        self.repository.remove(skill_id)
    }

    pub fn get(&self, skill_id: &str) -> AdmResult<Option<SkillRecord>> {
        self.repository.get(skill_id)
    }

    pub fn document(&self, skill_id: &str) -> AdmResult<Option<SkillDocument>> {
        Ok(self.get(skill_id)?.map(|record| record.document))
    }
}

#[derive(Debug, Clone, Default)]
pub struct RunLogService {
    entries: Vec<LogEntry>,
}

impl RunLogService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write(&mut self, entry: LogEntry) {
        self.entries.push(entry);
    }

    pub fn latest(&self, limit: usize) -> Vec<LogEntry> {
        self.entries.iter().rev().take(limit).cloned().collect()
    }

    pub fn filter_level(&self, level: LogLevel) -> Vec<LogEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.level == level)
            .cloned()
            .collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn export_jsonl(&self) -> String {
        self.entries
            .iter()
            .map(|entry| serde_json::to_string(entry).unwrap_or_else(|_| "{}".to_string()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::ai::{ApiCategory, ApiEntry};
    use adm_new_contracts::artifact::{
        ArtifactContract, ArtifactReportStatus, ArtifactTask, ArtifactTaskWithArtifact,
        REVIEWER_WHITELIST, SchemaRef, VALIDATOR_WHITELIST,
    };
    use adm_new_contracts::package::{PackageStatus, REQUIRED_INTEGRATION_CHECKS};
    use adm_new_contracts::pipeline::{
        PipelineStageResult, StageContextModel, StageKind, StageSpec, StageStatus,
    };
    use adm_new_contracts::project::NodeState;
    use adm_new_design::{DesignChecklistItemSpec, DesignOptionGroupSpec};
    use adm_new_foundation::new_stable_id;
    use serde_json::Value;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn crate_reports_ready() {
        assert!(crate_ready());
        assert_eq!(CRATE_NAME, "adm-new-application");
    }

    #[test]
    fn design_workbench_service_delegates_state_and_view_model() {
        let service = DesignWorkbenchService::new(vec![DesignNodeSpec {
            node_id: "identity".to_string(),
            domain_id: "profile".to_string(),
            name: "Identity".to_string(),
            description: String::new(),
            role_class: String::new(),
            checklist: vec![DesignChecklistItemSpec {
                item_id: "promise".to_string(),
                label: "Promise".to_string(),
                option_groups: vec![DesignOptionGroupSpec {
                    group_id: "promise_type".to_string(),
                    selection_mode: "single".to_string(),
                    allow_primary: false,
                    options: vec!["clear".to_string()],
                }],
            }],
        }]);
        let state = service.empty_project_state();
        let view = service.view_model(&state);
        assert_eq!(view.nodes.len(), 1);
        assert_eq!(view.nodes[0].node_id, "identity");
        assert_eq!(view.project_coverage.total_checklist, 1);
    }

    #[test]
    fn design_export_uses_requested_artifact_locale() {
        let service = DesignWorkbenchService::new(vec![DesignNodeSpec {
            node_id: "identity".to_string(),
            domain_id: "profile".to_string(),
            name: "Identity".to_string(),
            description: String::new(),
            role_class: String::new(),
            checklist: Vec::new(),
        }]);
        let mut state = service.empty_project_state();
        state.project_name = "Locale Demo".to_string();

        let zh_markdown = service
            .export_design_with_locale(&state, "markdown", ArtifactLocale::ZhCn)
            .unwrap();
        let en_markdown = service
            .export_design_with_locale(&state, "markdown", ArtifactLocale::EnUs)
            .unwrap();
        let zh_json: Value = serde_json::from_str(
            &service
                .export_design_with_locale(&state, "json", ArtifactLocale::ZhCn)
                .unwrap(),
        )
        .unwrap();

        assert!(zh_markdown.starts_with("<!-- artifact_locale: zh-CN -->\n# Locale Demo"));
        assert!(zh_markdown.contains("## 覆盖率"));
        assert!(en_markdown.starts_with("<!-- artifact_locale: en-US -->\n# Locale Demo"));
        assert!(en_markdown.contains("## Coverage"));
        assert_eq!(zh_json["artifact_locale"], serde_json::json!("zh-CN"));
    }

    #[test]
    fn design_workbench_service_covers_a32_state_templates_gameplay_autosave() {
        let root = temp_root("design_templates");
        let loader =
            DesignDataLoader::new(&root).with_runtime_root(root.join("drafts").join("current"));
        let service = DesignWorkbenchService::new(vec![DesignNodeSpec {
            node_id: "gameplay_core".to_string(),
            domain_id: "gameplay_system_design".to_string(),
            name: "Gameplay Core".to_string(),
            description: "Gameplay system anchor.".to_string(),
            role_class: "system_concrete".to_string(),
            checklist: vec![DesignChecklistItemSpec {
                item_id: "core_loop".to_string(),
                label: "Core Loop".to_string(),
                option_groups: vec![DesignOptionGroupSpec {
                    group_id: "loop_kind".to_string(),
                    selection_mode: "single".to_string(),
                    allow_primary: true,
                    options: vec!["tactical".to_string(), "arcade".to_string()],
                }],
            }],
        }])
        .with_template_loader(loader.clone());
        let mut state = service.empty_project_state();
        state.project_name = "Workbench Demo".to_string();
        state.ai_interview.codex_session_id = "must-not-enter-template".to_string();

        let view = service
            .update_gameplay_system(
                &mut state,
                &GameplaySystemUpdateRequest {
                    system_id: "combat".to_string(),
                    selected: Some(true),
                    weight: Some(serde_json::json!(60)),
                    core_loop: Some("choose action and resolve feedback".to_string()),
                    custom_name: None,
                    delete_custom: false,
                    interview_answers: vec!["combat should be readable".to_string()],
                },
            )
            .unwrap();
        assert_eq!(
            view.gameplay_systems["selected"].as_array().unwrap().len(),
            1
        );
        assert_eq!(
            state.gameplay_systems.weights["combat"].weight,
            serde_json::json!(60)
        );

        let autosave = service
            .autosave_state_summary(&state, Some("drafts/current/autosave_state.json"), true)
            .unwrap();
        assert_eq!(autosave.selected_gameplay_system_count, 1);
        assert!(autosave.state_hash.starts_with("fnv64:"));

        let save_report = service
            .save_project_template(&state, "Tactical Demo", "indie", false)
            .unwrap();
        assert_eq!(
            save_report.target_file_name,
            "custom_indie_Tactical_Demo.json"
        );
        assert!(!save_report.overwritten);
        assert!(
            loader
                .custom_project_templates_dir()
                .join(&save_report.target_file_name)
                .is_file()
        );
        let listed = service.list_project_templates(false).unwrap();
        assert_eq!(listed.templates.len(), 1);
        assert!(
            serde_json::to_value(&listed).unwrap()["templates"][0]
                .get("projectState")
                .is_none()
        );

        state.project_name = "Changed after template save".to_string();
        let selection = service
            .apply_project_template(&mut state, &save_report.template_id, "范本：")
            .unwrap();
        assert_eq!(selection.project_name, "范本：Tactical Demo");
        assert!(state.ai_interview.codex_session_id.is_empty());

        let before_bad_prefix = state.clone();
        assert!(
            service
                .apply_project_template(&mut state, &save_report.template_id, "bad\nprefix")
                .is_err()
        );
        assert_eq!(state, before_bad_prefix);

        fs::create_dir_all(loader.project_templates_dir()).unwrap();
        fs::write(
            loader
                .project_templates_dir()
                .join("builtin_indie_hades.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "template": {
                    "id": "builtin_indie_hades",
                    "name": "Hades（哈迪斯）",
                    "gameName": "Hades",
                    "targetScale": "indie"
                },
                "projectState": {"projectName": "Hades"}
            }))
            .unwrap(),
        )
        .unwrap();
        service
            .apply_project_template(&mut state, "builtin_indie_hades", "Template: ")
            .unwrap();
        assert_eq!(state.project_name, "Template: Hades");
        service
            .apply_project_template(&mut state, "builtin_indie_hades", "范本：")
            .unwrap();
        assert_eq!(state.project_name, "范本：Hades（哈迪斯）");

        fs::write(
            loader
                .custom_project_templates_dir()
                .join("custom_indie_bad_template.json"),
            "{broken-json",
        )
        .unwrap();
        let before_failure = state.clone();
        assert!(
            service
                .apply_project_template(&mut state, "custom_indie_bad_template", "范本：")
                .is_err()
        );
        assert_eq!(state, before_failure);

        let deleted = service
            .delete_project_template(&save_report.template_id)
            .unwrap();
        assert_eq!(deleted.template_name, "Tactical Demo");

        let mut inference_state = service.empty_project_state();
        let mut action_node = NodeState::default();
        action_node.decision_state = DecisionState::Selected;
        inference_state
            .nodes
            .insert("action_rule_decision".to_string(), action_node);
        let mut economy_node = NodeState::default();
        economy_node.checklist.insert("economy".to_string(), true);
        inference_state
            .nodes
            .insert("balance_economy_decision".to_string(), economy_node);
        inference_state.gameplay_systems.weights.insert(
            "action_rule".to_string(),
            GameplaySystemWeight {
                weight: serde_json::json!(3),
                weight_type: "percent".to_string(),
                extra: BTreeMap::new(),
            },
        );
        inference_state.gameplay_systems.weights.insert(
            "resource_economy".to_string(),
            GameplaySystemWeight {
                weight: serde_json::json!(1),
                weight_type: "percent".to_string(),
                extra: BTreeMap::new(),
            },
        );
        let inference_template = service
            .save_project_template(&inference_state, "Inference", "indie", false)
            .unwrap();
        service
            .apply_project_template(&mut state, &inference_template.template_id, "Template: ")
            .unwrap();
        assert_eq!(
            state.gameplay_systems.selected,
            vec!["action_rule".to_string(), "resource_economy".to_string()]
        );
        assert_eq!(
            state.gameplay_systems.weights["action_rule"].weight,
            serde_json::json!(75)
        );
        assert_eq!(
            state.gameplay_systems.weights["resource_economy"].weight,
            serde_json::json!(25)
        );
        service
            .delete_project_template(&inference_template.template_id)
            .unwrap();

        let reset = service.reset_project_state(&mut state);
        assert_eq!(reset.project_name, "未命名游戏设计项目");
        cleanup(root);
    }

    #[test]
    fn save_application_service_delegates_create_and_load() {
        let root = temp_root("save_app");
        let service = SaveApplicationService::with_pid(&root, "session_a", 100).unwrap();
        let mut state = ProjectState::empty();
        state.project_name = "App Save".to_string();
        let report = service.create_save("App Save", &state).unwrap();
        let lock_path = root
            .join("saves")
            .join(&report.manifest.save_id)
            .join(".archive_lock");
        assert!(lock_path.exists());
        service.release_current_lock().unwrap();
        let released: Value =
            serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
        assert_eq!(released.get("live").and_then(Value::as_bool), Some(false));
        service.acquire_current_lock().unwrap();
        let acquired: Value =
            serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
        assert_eq!(acquired.get("live").and_then(Value::as_bool), Some(true));
        let loaded = service.load_save(&report.manifest.save_id).unwrap();
        assert_eq!(loaded.state.project_name, "App Save");
        service
            .rename_save(&report.manifest.save_id, "Renamed")
            .unwrap();
        service.delete_save(&report.manifest.save_id).unwrap();
        cleanup(root);
    }

    #[test]
    fn save_application_service_delegates_a11_save_repairs() {
        let root = temp_root("save_a11");
        let service = SaveApplicationService::with_pid(&root, "session_a", 100).unwrap();

        let blank = service.create_blank_save("Blank").unwrap();
        assert_eq!(blank.manifest.reason, "create_blank_save");
        let inherited = root
            .join("saves")
            .join(&blank.manifest.save_id)
            .join("workspace/outputs/artifacts/stage_00/stale.json");
        fs::create_dir_all(inherited.parent().unwrap()).unwrap();
        fs::write(&inherited, "{}").unwrap();
        let repair = service
            .repair_blank_save_progress(&blank.manifest.save_id, true)
            .unwrap();
        assert!(repair.apply);
        assert!(!inherited.exists());

        let iteration = service
            .create_iteration_save(
                "Iteration",
                &sample_state("Iteration", true),
                "feature",
                "v2",
                "iteration_specs/feature.json",
            )
            .unwrap();
        assert_eq!(iteration.manifest.save_type, "iteration");
        assert_eq!(
            iteration.manifest.iteration_spec_path.as_deref(),
            Some("iteration_specs/feature.json")
        );

        let audit = service.audit_parallel_isolation().unwrap();
        assert_eq!(audit.status, "passed");
        let contamination = service.repair_parallel_save_contamination(false).unwrap();
        assert_eq!(contamination.mode, "dry_run");
        cleanup(root);
    }

    #[test]
    fn ai_config_application_service_delegates_validation_save_and_adapter_spec() {
        let root = temp_root("ai_config_app");
        let service = AiConfigApplicationService::new(&root).unwrap();
        let config = AiConfig {
            dev: ApiCategory {
                category_id: "dev".to_string(),
                entries: vec![ApiEntry {
                    id: "codex_cli".to_string(),
                    label: "Codex".to_string(),
                    config_type: "local_codex_cli".to_string(),
                    ..ApiEntry::default()
                }],
                active_entry_id: "codex_cli".to_string(),
            },
            completion: ApiCategory {
                category_id: "completion".to_string(),
                entries: vec![ApiEntry {
                    id: "completion".to_string(),
                    label: "Completion".to_string(),
                    config_type: "openai_completion_api".to_string(),
                    api_url: "https://api.example.test/v1".to_string(),
                    api_key: "secret".to_string(),
                    extra_json: serde_json::json!({"model": "gpt-5.5"}),
                    ..ApiEntry::default()
                }],
                active_entry_id: "completion".to_string(),
            },
            ..AiConfig::default()
        };
        let report = service.save(&config).unwrap();
        assert!(report.ok, "{:?}", report.errors);
        let loaded = service.load_or_default().unwrap();
        let spec = service.completion_adapter_spec(&loaded).unwrap();
        assert_eq!(spec.adapter_kind, "openai_compatible");
        let active = service.active_completion_entry(&loaded).unwrap();
        assert_eq!(active.id, "completion");
        assert_eq!(active.api_key, "secret");
        cleanup(root);
    }

    #[test]
    fn ai_interview_application_service_delegates_high_confidence_writeback() {
        let service = AiInterviewApplicationService::new(vec![DesignNodeSpec {
            node_id: "mechanics".to_string(),
            domain_id: "core".to_string(),
            name: "Mechanics".to_string(),
            description: String::new(),
            role_class: String::new(),
            checklist: vec![DesignChecklistItemSpec {
                item_id: "core_loop".to_string(),
                label: "Core Loop".to_string(),
                option_groups: Vec::new(),
            }],
        }]);
        let mut state = ProjectState::empty();
        let mut candidate = ProjectState::empty();
        candidate.nodes.insert(
            "mechanics".to_string(),
            NodeState {
                design_note: "From AI".to_string(),
                ..NodeState::default()
            },
        );
        let payload = serde_json::json!({
            "schemaVersion": "1.0",
            "mode": "full_project_output",
            "assistantMessage": "ready",
            "fullProjectOutput": {
                "projectState": candidate,
                "confidenceMap": {"nodes": {"mechanics": 0.91}}
            }
        });
        let report = service
            .handle_payload_json(&mut state, AiSchemaMode::FullOutput, &payload.to_string())
            .unwrap();
        assert!(report.applied_project_state);
        assert_eq!(state.nodes["mechanics"].design_note, "From AI");
    }

    #[test]
    fn pipeline_application_service_delegates_order_and_run_range() {
        let service = PipelineApplicationService::new(PipelineRegistry {
            stages: vec![
                app_stage("08", vec![]),
                app_stage("09", vec![]),
                app_stage("10", vec!["08", "09"]),
            ],
        })
        .unwrap();
        let order = service.topological_order().unwrap();
        assert_eq!(order, vec!["08", "09", "10"]);
        let mut state = PipelineRunState {
            run_id: "app-run".to_string(),
            status: "idle".to_string(),
            stop_requested: false,
            current_stage_id: None,
            stages: Default::default(),
            ..PipelineRunState::default()
        };
        let report = service
            .run_range(&mut state, "08", "10", &AppPipelineExecutor)
            .unwrap();
        assert_eq!(report.final_status, "success");
        assert_eq!(state.stages["10"].status, StageStatus::Success);
    }

    #[test]
    fn artifact_application_service_delegates_preflight_review_and_validation() {
        let artifact = ArtifactContract {
            id: "stage_00.concept_bundle".to_string(),
            stage: 0,
            kind: "design_import_bundle".to_string(),
            depends_on: Vec::new(),
            tasks: vec![ArtifactTask {
                id: "stage_00.import".to_string(),
                task_type: "import".to_string(),
                description: String::new(),
            }],
            reviewers: REVIEWER_WHITELIST
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            validators: VALIDATOR_WHITELIST
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            schema_refs: vec![SchemaRef {
                path: "outputs/artifacts/stage_00/concept_bundle.json".to_string(),
                schema: "knowledge/schemas/concept_bundle.schema.json".to_string(),
                description: String::new(),
            }],
            knowledge_refs: vec!["knowledge/Core_Rules.md".to_string()],
            extra: Default::default(),
        };
        let service = ArtifactApplicationService::new(ArtifactRegistry {
            version: 1,
            description: String::new(),
            default_reviewers: Vec::new(),
            default_validators: Vec::new(),
            artifacts: vec![artifact.clone()],
        })
        .unwrap();
        let evidence = ArtifactEvidenceSet::with_paths([
            "knowledge/Core_Rules.md",
            "knowledge/schemas/concept_bundle.schema.json",
            "outputs/artifacts/stage_00/artifact_index.json",
            "outputs/artifacts/stage_00/reference_manifest.json",
            "outputs/artifacts/stage_00/validation_report.json",
            "outputs/artifacts/stage_00/concept_bundle.json",
        ]);
        assert!(service.dependency_graph().errors.is_empty());
        let preflight = service.preflight_stage_contract(0, &evidence);
        assert_eq!(preflight.status, ArtifactReportStatus::Success);
        let review = service.run_review_pipeline(0, &evidence);
        assert_eq!(review.status, ArtifactReportStatus::Success);
        let manifest = ArtifactLayerManifest {
            step: 0,
            timestamp: String::new(),
            stage_dir: "outputs/artifacts/stage_00".to_string(),
            artifacts: vec![artifact],
            tasks: vec![ArtifactTaskWithArtifact {
                id: "stage_00.import".to_string(),
                task_type: "import".to_string(),
                description: String::new(),
                artifact_id: "stage_00.concept_bundle".to_string(),
            }],
            file_manifest: Vec::new(),
        };
        let validation = service.run_artifact_validators(0, &manifest, &review, &evidence);
        assert_eq!(validation.status, ArtifactReportStatus::Success);
    }

    #[test]
    fn packaging_application_service_delegates_package_current_project() {
        let checks = REQUIRED_INTEGRATION_CHECKS
            .iter()
            .map(|id| ((*id).to_string(), Value::Bool(true)))
            .collect::<serde_json::Map<_, _>>();
        let result = PackagingApplicationService::new().package_current_project(PackagingSources {
            integration: serde_json::json!({"status":"success","checks": Value::Object(checks)}),
            actual_project_file_audit: serde_json::json!({
                "development_path": "UnityProject",
                "actual_changed_files": ["Assets/DemoScene.unity"]
            }),
            unity_validation_summary: serde_json::json!({
                "valid": true,
                "unity_editor_path": "Unity.exe",
                "validation_count": 1,
                "failed_validation_count": 0
            }),
        });
        assert_eq!(result.validation_report.status, PackageStatus::Success);
        assert_eq!(
            result.manifest.outputs.package_dir,
            "outputs/package/current"
        );
    }

    #[test]
    fn patch_application_service_delegates_status_and_approved_context() {
        let mut service = PatchApplicationService::new();
        let record = service
            .analyze_request_shell(
                "Add patch panel refresh",
                vec![PatchTask {
                    task_id: "task-1".to_string(),
                    title: "Refresh".to_string(),
                    description: String::new(),
                    affected_systems: vec!["patch".to_string()],
                    expected_files: Vec::new(),
                    validation_route: Vec::new(),
                    requires_iteration: false,
                }],
            )
            .unwrap();
        assert_eq!(service.list().len(), 1);
        service
            .set_status(&record.patch_id, PatchStatus::Validated)
            .unwrap();
        assert_eq!(service.approved_context().len(), 1);
    }

    #[test]
    fn sdk_application_service_keeps_ai_extraction_pending_until_approved() {
        let mut service = SdkKnowledgeApplicationService::new();
        let spec = service.ingest_ai_extracted_spec(SdkSpec {
            sdk_id: "steamworks".to_string(),
            name: "Steamworks".to_string(),
            source_url: String::new(),
            review_status: SdkReviewStatus::Approved,
            summary: "Steam integration".to_string(),
            integration_notes: vec!["Use platform wrapper".to_string()],
            api_requirements: Vec::new(),
            risks: Vec::new(),
            last_synced_at: String::new(),
            updated_at: String::new(),
        });
        assert_eq!(spec.review_status, SdkReviewStatus::PendingReview);
        assert!(service.approved_context().is_empty());
        service
            .set_review_status("steamworks", SdkReviewStatus::Approved)
            .unwrap();
        assert!(service.approved_context().contains("Steamworks"));
    }

    #[test]
    fn logs_application_service_filters_latest_clears_and_exports_jsonl() {
        let mut service = RunLogService::new();
        for index in 0..6 {
            service.write(LogEntry {
                timestamp: format!("unix:{index}"),
                level: if index % 2 == 0 {
                    LogLevel::Info
                } else {
                    LogLevel::Error
                },
                context: "pipeline".to_string(),
                message: format!("entry {index}"),
                source: "test".to_string(),
                metadata: Default::default(),
            });
        }
        assert_eq!(service.latest(5).len(), 5);
        assert_eq!(service.latest(1)[0].message, "entry 5");
        assert_eq!(service.filter_level(LogLevel::Error).len(), 3);
        let jsonl = service.export_jsonl();
        assert!(jsonl.contains("\"level\":\"ERROR\""));
        service.clear();
        assert!(service.latest(1).is_empty());
    }

    struct AppPipelineExecutor;

    impl StageExecutor for AppPipelineExecutor {
        fn execute(&self, spec: &StageSpec, _: &StageContextModel) -> PipelineStageResult {
            PipelineStageResult {
                status: StageStatus::Success,
                outputs: Default::default(),
                errors: Vec::new(),
                warnings: Vec::new(),
                message: spec.stage_id.clone(),
            }
        }
    }

    fn app_stage(stage_id: &str, requires: Vec<&str>) -> StageSpec {
        StageSpec {
            stage_id: stage_id.to_string(),
            kind: StageKind::Development,
            number: stage_id.parse::<u32>().ok(),
            slug: format!("stage_{stage_id}"),
            title: format!("Stage {stage_id}"),
            requires: requires.into_iter().map(str::to_string).collect(),
            source_groups: Vec::new(),
            plugin_ref: String::new(),
            metadata: Default::default(),
        }
    }

    fn sample_state(project_name: &str, checked: bool) -> ProjectState {
        let mut state = ProjectState::empty();
        state.project_name = project_name.to_string();
        let mut node = NodeState::default();
        node.checklist.insert("core_loop".to_string(), checked);
        state.nodes.insert("mechanics".to_string(), node);
        state
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm_new_application_{label}_{}",
            new_stable_id("root").unwrap()
        ))
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
