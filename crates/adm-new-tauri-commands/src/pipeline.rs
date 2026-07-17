use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use adm_new_application::{
    PipelineApplicationService, PipelineRunObserver, PipelineRunReport, StageExecutor,
};
use adm_new_contracts::ArtifactLocale;
use adm_new_contracts::pipeline::{
    PipelineRecoverySummary, PipelineRunState, PipelineStageResult, PipelineStageRuntime,
    StageContextModel, StageKind, StageSpec, StageStatus,
};
use adm_new_foundation::{
    AdmError, AdmResult, ensure_child_path, ensure_relative_path, sanitize_identifier,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CommandAdapterResult, handle_command};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunPipelineRangeRequest {
    pub from_stage_id: String,
    pub to_stage_id: String,
    #[serde(default)]
    pub skip_manual_gates: bool,
    #[serde(default)]
    pub artifact_locale: ArtifactLocale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumePipelineRequest {
    pub run_id: String,
    pub expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmStyleRequest {
    pub stage_id: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub selected_style_id: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineRunReportView {
    pub ordered_stage_ids: Vec<String>,
    pub executed_stage_ids: Vec<String>,
    pub final_status: String,
    #[serde(default)]
    pub from_stage_id: String,
    #[serde(default)]
    pub to_stage_id: String,
}

impl From<PipelineRunReport> for PipelineRunReportView {
    fn from(value: PipelineRunReport) -> Self {
        Self {
            ordered_stage_ids: value.ordered_stage_ids,
            executed_stage_ids: value.executed_stage_ids,
            final_status: value.final_status,
            from_stage_id: String::new(),
            to_stage_id: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineView {
    pub ordered_stage_ids: Vec<String>,
    pub stages: Vec<PipelineStageView>,
    #[serde(skip)]
    pub state: PipelineRunState,
    #[serde(rename = "state")]
    pub state_view: PipelineStateView,
    pub current_stage_id: Option<String>,
    pub running: bool,
    pub waiting_confirmation: bool,
    pub style_options: Vec<StyleOptionView>,
    #[serde(default)]
    pub recovery: Option<PipelineRecoverySummary>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineStateView {
    pub schema_version: u32,
    pub run_id: String,
    pub attempt_id: String,
    pub parent_attempt_id: Option<String>,
    pub attempt_no: u32,
    pub state_version: u64,
    pub status: String,
    pub stop_requested: bool,
    pub from_stage_id: String,
    pub to_stage_id: String,
    pub stage_ids: Vec<String>,
    pub current_stage_id: Option<String>,
    pub current_unit_id: Option<String>,
}

impl From<&PipelineRunState> for PipelineStateView {
    fn from(state: &PipelineRunState) -> Self {
        Self {
            schema_version: state.schema_version,
            run_id: state.run_id.clone(),
            attempt_id: state.attempt_id.clone(),
            parent_attempt_id: state.parent_attempt_id.clone(),
            attempt_no: state.attempt_no,
            state_version: state.state_version,
            status: state.status.clone(),
            stop_requested: state.stop_requested,
            from_stage_id: state.from_stage_id.clone(),
            to_stage_id: state.to_stage_id.clone(),
            stage_ids: state.stage_ids.clone(),
            current_stage_id: state.current_stage_id.clone(),
            current_unit_id: state.current_unit_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineCommandView {
    pub view: PipelineView,
    #[serde(default)]
    pub report: Option<PipelineRunReportView>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineStageView {
    pub stage_id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub message: String,
    pub is_step07: bool,
    #[serde(default, skip_serializing)]
    pub outputs: BTreeMap<String, Value>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing)]
    pub artifacts: Vec<PipelineArtifactRecordView>,
    pub semantic_quality: PipelineSemanticQualityView,
    pub bounded_completion: PipelineBoundedCompletionView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineBoundedCompletionView {
    pub status: String,
    #[serde(default)]
    pub task_id: String,
    #[serde(default)]
    pub model_config_id: String,
    #[serde(default)]
    pub candidate_patch_id: Option<String>,
    #[serde(default)]
    pub risk: Option<String>,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub confirmation_mode: Option<String>,
    #[serde(default)]
    pub confirmation_actor: Option<String>,
    #[serde(default)]
    pub confirmation_accepted: Option<bool>,
    #[serde(default)]
    pub error_count: usize,
    #[serde(default)]
    pub errors: Vec<String>,
}

impl Default for PipelineBoundedCompletionView {
    fn default() -> Self {
        Self {
            status: "not_called".to_string(),
            task_id: String::new(),
            model_config_id: String::new(),
            candidate_patch_id: None,
            risk: None,
            attempts: 0,
            confirmation_mode: None,
            confirmation_actor: None,
            confirmation_accepted: None,
            error_count: 0,
            errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineArtifactRecordView {
    pub relative_path: String,
    pub name: String,
    pub size_bytes: u64,
    pub content_type: String,
    #[serde(default)]
    pub content_preview: String,
    #[serde(default)]
    pub is_binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadPipelineArtifactRequest {
    pub stage_id: String,
    pub relative_path: String,
    #[serde(default)]
    pub max_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactContentView {
    pub relative_path: String,
    pub name: String,
    pub size_bytes: u64,
    pub content_type: String,
    pub encoding: String,
    pub content: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleOptionView {
    pub option_id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub image_path: String,
    #[serde(default)]
    pub image_status: String,
    #[serde(default)]
    pub image_message: String,
    #[serde(default)]
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineSemanticQualityView {
    pub status: String,
    #[serde(default)]
    pub project_specificity_score: Option<f64>,
    #[serde(default)]
    pub required_semantic_coverage: Option<f64>,
    #[serde(default)]
    pub generic_template_ratio: Option<f64>,
    #[serde(default)]
    pub placeholder_ratio: Option<f64>,
    #[serde(default)]
    pub return_targets: Vec<PipelineIssueReturnView>,
    #[serde(default)]
    pub report_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineIssueReturnView {
    pub severity: String,
    pub code: String,
    pub message: String,
    pub return_target: String,
    pub source_file: String,
}

pub trait PipelineCommandService {
    fn load_pipeline_view(&self, state: &PipelineRunState) -> AdmResult<PipelineView>;

    fn run_pipeline_range(
        &self,
        state: &mut PipelineRunState,
        request: &RunPipelineRangeRequest,
        executor: &dyn StageExecutor,
    ) -> AdmResult<PipelineCommandView>;

    fn run_pipeline_range_with_observer(
        &self,
        state: &mut PipelineRunState,
        request: &RunPipelineRangeRequest,
        executor: &dyn StageExecutor,
        _observer: &dyn PipelineRunObserver,
    ) -> AdmResult<PipelineCommandView> {
        self.run_pipeline_range(state, request, executor)
    }

    fn stop_pipeline(&self, state: &mut PipelineRunState) -> AdmResult<PipelineView>;

    fn confirm_style(
        &self,
        state: &mut PipelineRunState,
        request: &ConfirmStyleRequest,
    ) -> AdmResult<PipelineView>;
}

impl PipelineCommandService for PipelineApplicationService {
    fn load_pipeline_view(&self, state: &PipelineRunState) -> AdmResult<PipelineView> {
        Ok(view_from_state(
            self.topological_order()?,
            self.stage_specs(),
            state.clone(),
        ))
    }

    fn run_pipeline_range(
        &self,
        state: &mut PipelineRunState,
        request: &RunPipelineRangeRequest,
        executor: &dyn StageExecutor,
    ) -> AdmResult<PipelineCommandView> {
        self.run_pipeline_range_with_observer(
            state,
            request,
            executor,
            &NoopCommandPipelineObserver,
        )
    }

    fn run_pipeline_range_with_observer(
        &self,
        state: &mut PipelineRunState,
        request: &RunPipelineRangeRequest,
        executor: &dyn StageExecutor,
        observer: &dyn PipelineRunObserver,
    ) -> AdmResult<PipelineCommandView> {
        let resolved = self.resolve_range(&request.from_stage_id, &request.to_stage_id)?;
        state.stop_requested = false;
        let adapter = DynStageExecutor {
            executor,
            skip_manual_gates: request.skip_manual_gates,
        };
        let report = self.run_range_with_observer(
            state,
            &resolved.from_stage_id,
            &resolved.to_stage_id,
            &adapter,
            observer,
        )?;
        let mut report: PipelineRunReportView = report.into();
        report.from_stage_id = resolved.from_stage_id;
        report.to_stage_id = resolved.to_stage_id;
        Ok(PipelineCommandView {
            view: self.load_pipeline_view(state)?,
            report: Some(report),
        })
    }

    fn stop_pipeline(&self, state: &mut PipelineRunState) -> AdmResult<PipelineView> {
        PipelineApplicationService::request_stop(state);
        self.load_pipeline_view(state)
    }

    fn confirm_style(
        &self,
        state: &mut PipelineRunState,
        request: &ConfirmStyleRequest,
    ) -> AdmResult<PipelineView> {
        PipelineApplicationService::confirm_style_selection(
            state,
            &request.stage_id,
            &request.selected_style_id(),
            &request.notes(),
            &request.message,
        );
        self.load_pipeline_view(state)
    }
}

impl ConfirmStyleRequest {
    fn selected_style_id(&self) -> String {
        if !self.selected_style_id.trim().is_empty() {
            return self.selected_style_id.trim().to_string();
        }
        message_field(&self.message, "style").unwrap_or_default()
    }

    fn notes(&self) -> String {
        if !self.notes.trim().is_empty() {
            return self.notes.trim().to_string();
        }
        message_field(&self.message, "notes").unwrap_or_default()
    }
}

struct DynStageExecutor<'a> {
    executor: &'a dyn StageExecutor,
    skip_manual_gates: bool,
}

struct NoopCommandPipelineObserver;

impl PipelineRunObserver for NoopCommandPipelineObserver {}

impl StageExecutor for DynStageExecutor<'_> {
    fn execute(&self, spec: &StageSpec, context: &StageContextModel) -> PipelineStageResult {
        let result = self.executor.execute(spec, context);
        if self.skip_manual_gates
            && spec.kind == StageKind::HumanGate
            && result.status == StageStatus::WaitingConfirmation
        {
            return self.executor.skip_manual_gate(spec, context, result);
        }
        result
    }

    fn stop_requested(&self) -> bool {
        self.executor.stop_requested()
    }
}

pub fn load_pipeline_view<S>(
    service: &S,
    state: &PipelineRunState,
) -> CommandAdapterResult<PipelineView>
where
    S: PipelineCommandService,
{
    handle_command(|| service.load_pipeline_view(state))
}

pub fn run_pipeline_range<S>(
    service: &S,
    state: &mut PipelineRunState,
    request: RunPipelineRangeRequest,
    executor: &dyn StageExecutor,
) -> CommandAdapterResult<PipelineCommandView>
where
    S: PipelineCommandService,
{
    handle_command(|| service.run_pipeline_range(state, &request, executor))
}

pub fn run_pipeline_range_with_observer<S>(
    service: &S,
    state: &mut PipelineRunState,
    request: RunPipelineRangeRequest,
    executor: &dyn StageExecutor,
    observer: &dyn PipelineRunObserver,
) -> CommandAdapterResult<PipelineCommandView>
where
    S: PipelineCommandService,
{
    handle_command(|| service.run_pipeline_range_with_observer(state, &request, executor, observer))
}

pub fn stop_pipeline<S>(
    service: &S,
    state: &mut PipelineRunState,
) -> CommandAdapterResult<PipelineView>
where
    S: PipelineCommandService,
{
    handle_command(|| service.stop_pipeline(state))
}

pub fn confirm_style<S>(
    service: &S,
    state: &mut PipelineRunState,
    request: ConfirmStyleRequest,
) -> CommandAdapterResult<PipelineView>
where
    S: PipelineCommandService,
{
    handle_command(|| service.confirm_style(state, &request))
}

pub fn read_pipeline_artifact(
    artifact_root: impl AsRef<Path>,
    request: ReadPipelineArtifactRequest,
) -> CommandAdapterResult<ArtifactContentView> {
    handle_command(|| read_pipeline_artifact_inner(artifact_root.as_ref(), &request))
}

fn view_from_state(
    ordered_stage_ids: Vec<String>,
    stage_specs: Vec<StageSpec>,
    state: PipelineRunState,
) -> PipelineView {
    let current_stage_id = state.current_stage_id.clone();
    let running = matches!(
        state.status.as_str(),
        "running" | "stop_requested" | "stopping" | "resuming"
    );
    let waiting_confirmation = state.status == "waiting_confirmation";
    let recovery = state.recovery.clone();
    let stages = ordered_stage_ids
        .iter()
        .map(|stage_id| {
            let spec = stage_specs.iter().find(|spec| &spec.stage_id == stage_id);
            let runtime = state.stages.get(stage_id);
            let status = runtime
                .map(|runtime| runtime.status.as_str().to_string())
                .unwrap_or_else(|| "pending".to_string());
            let message = runtime
                .and_then(|runtime| runtime.result.as_ref())
                .map(|result| safe_user_text(&result.message))
                .unwrap_or_default();
            let semantic_quality = semantic_quality_from_runtime(runtime);
            let result = runtime.and_then(|runtime| runtime.result.as_ref());
            let outputs = result
                .map(|result| result.outputs.clone())
                .unwrap_or_default();
            let errors = result
                .map(|result| {
                    result
                        .errors
                        .iter()
                        .map(|item| safe_user_text(item))
                        .collect()
                })
                .unwrap_or_default();
            let warnings = result
                .map(|result| {
                    result
                        .warnings
                        .iter()
                        .map(|item| safe_user_text(item))
                        .collect()
                })
                .unwrap_or_default();
            let artifacts = artifact_records_from_outputs(&outputs);
            let bounded_completion = bounded_completion_from_outputs(&outputs);
            PipelineStageView {
                stage_id: stage_id.clone(),
                title: spec
                    .map(|spec| spec.title.clone())
                    .filter(|title| !title.is_empty())
                    .unwrap_or_else(|| format!("Step {stage_id}")),
                kind: spec
                    .map(|spec| format!("{:?}", spec.kind).to_ascii_lowercase())
                    .unwrap_or_else(|| "development".to_string()),
                status,
                message,
                is_step07: stage_id == "07",
                outputs,
                errors,
                warnings,
                artifacts,
                semantic_quality,
                bounded_completion,
            }
        })
        .collect();
    let style_options = style_options_from_state(&state);
    let state_view = PipelineStateView::from(&state);
    PipelineView {
        ordered_stage_ids,
        stages,
        state,
        state_view,
        current_stage_id,
        running,
        waiting_confirmation,
        style_options,
        recovery,
    }
}

fn artifact_records_from_outputs(
    outputs: &BTreeMap<String, Value>,
) -> Vec<PipelineArtifactRecordView> {
    outputs
        .get("artifact_records")
        .or_else(|| outputs.get("artifacts"))
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

fn bounded_completion_from_outputs(
    outputs: &BTreeMap<String, Value>,
) -> PipelineBoundedCompletionView {
    let Some(value) = bounded_completion_output(outputs) else {
        return PipelineBoundedCompletionView::default();
    };
    let mut errors = bounded_completion_errors(value);
    let Some(object) = value.as_object() else {
        let mut view = PipelineBoundedCompletionView::default();
        view.status = value
            .as_str()
            .and_then(|status| normalize_completion_status(status, &mut errors))
            .unwrap_or_else(|| {
                errors.push("bounded completion record is not an object".to_string());
                "failed".to_string()
            });
        view.error_count = errors.len();
        view.errors = errors.into_iter().take(4).collect();
        return view;
    };

    let null = Value::Null;
    let audit = object.get("audit").unwrap_or(&null);
    let confirmation = audit
        .get("confirmation")
        .or_else(|| value.get("confirmation"))
        .unwrap_or(&null);
    let status = string_any(value, &["status"])
        .or_else(|| string_any(audit, &["status"]))
        .and_then(|status| normalize_completion_status(&status, &mut errors))
        .unwrap_or_else(|| {
            errors.push("bounded completion record is missing status".to_string());
            "failed".to_string()
        });
    let error_count = usize_any(value, &["error_count", "errorCount"])
        .or_else(|| usize_any(audit, &["error_count", "errorCount"]))
        .unwrap_or(errors.len());
    PipelineBoundedCompletionView {
        status,
        task_id: string_any(value, &["task_id", "taskId", "id"])
            .or_else(|| string_any(audit, &["task_id", "taskId"]))
            .map(|text| safe_user_text(&text))
            .unwrap_or_default(),
        model_config_id: string_any(audit, &["model_config_id", "modelConfigId"])
            .or_else(|| string_any(value, &["model_config_id", "modelConfigId"]))
            .map(|text| safe_user_text(&text))
            .unwrap_or_default(),
        candidate_patch_id: string_any(value, &["candidate_patch_id", "candidatePatchId"])
            .or_else(|| string_any(audit, &["candidate_patch_id", "candidatePatchId"]))
            .map(|text| safe_user_text(&text)),
        risk: risk_any(value)
            .or_else(|| risk_any(audit))
            .map(|text| safe_user_text(&text)),
        attempts: u32_any(value, &["attempts"])
            .or_else(|| u32_any(audit, &["attempts"]))
            .unwrap_or(0),
        confirmation_mode: string_any(confirmation, &["mode", "policy"])
            .map(|text| safe_user_text(&text)),
        confirmation_actor: string_any(
            confirmation,
            &["actor", "reviewer", "confirmed_by", "confirmedBy"],
        )
        .map(|text| safe_user_text(&text)),
        confirmation_accepted: bool_any(confirmation, &["accepted", "approved", "confirmed"]),
        error_count,
        errors: errors.into_iter().take(4).collect(),
    }
}

fn bounded_completion_output(outputs: &BTreeMap<String, Value>) -> Option<&Value> {
    [
        "bounded_completion",
        "boundedCompletion",
        "bounded_completion_run",
        "boundedCompletionRun",
        "completion_run",
        "completionRun",
        "ai_completion",
        "aiCompletion",
    ]
    .iter()
    .find_map(|key| outputs.get(*key))
}

fn normalize_completion_status(status: &str, errors: &mut Vec<String>) -> Option<String> {
    let normalized = status.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "not_called" | "failed" | "rejected" | "confirmed" | "committed" => Some(normalized),
        "" => None,
        _ => {
            errors.push(format!(
                "unknown bounded completion status: {}",
                safe_user_text(status)
            ));
            Some("failed".to_string())
        }
    }
}

fn bounded_completion_errors(value: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    collect_completion_errors(value.get("errors"), &mut errors);
    collect_completion_errors(
        value.get("audit").and_then(|audit| audit.get("errors")),
        &mut errors,
    );
    let mut seen = std::collections::BTreeSet::new();
    errors.retain(|error| seen.insert(error.clone()));
    errors
}

fn collect_completion_errors(value: Option<&Value>, errors: &mut Vec<String>) {
    let Some(value) = value else {
        return;
    };
    match value {
        Value::Array(items) => {
            for item in items {
                collect_completion_error_item(item, errors);
            }
        }
        item => collect_completion_error_item(item, errors),
    }
}

fn collect_completion_error_item(value: &Value, errors: &mut Vec<String>) {
    let message = value
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            string_any(
                value,
                &["message", "error", "reason", "summary", "detail", "details"],
            )
        })
        .unwrap_or_else(|| value.to_string());
    let safe = safe_user_text(&message);
    if !safe.is_empty() {
        errors.push(safe);
    }
}

fn string_any(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
}

fn risk_any(value: &Value) -> Option<String> {
    value
        .get("risk")
        .and_then(|risk| {
            risk.as_str().map(str::to_string).or_else(|| {
                string_any(
                    risk,
                    &["level", "classification", "category", "name", "status"],
                )
            })
        })
        .or_else(|| string_any(value, &["risk_level", "riskLevel"]))
}

fn u32_any(value: &Value, keys: &[&str]) -> Option<u32> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(|item| {
                item.as_u64()
                    .or_else(|| item.as_str().and_then(|text| text.parse::<u64>().ok()))
            })
            .and_then(|number| u32::try_from(number).ok())
    })
}

fn usize_any(value: &Value, keys: &[&str]) -> Option<usize> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(|item| {
                item.as_u64()
                    .or_else(|| item.as_str().and_then(|text| text.parse::<u64>().ok()))
            })
            .and_then(|number| usize::try_from(number).ok())
    })
}

fn bool_any(value: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|item| {
            item.as_bool().or_else(|| {
                item.as_str()
                    .and_then(|text| match text.trim().to_ascii_lowercase().as_str() {
                        "true" | "yes" | "1" => Some(true),
                        "false" | "no" | "0" => Some(false),
                        _ => None,
                    })
            })
        })
    })
}

fn read_pipeline_artifact_inner(
    artifact_root: &Path,
    request: &ReadPipelineArtifactRequest,
) -> AdmResult<ArtifactContentView> {
    let requested_stage = request.stage_id.trim();
    let stage_id = sanitize_identifier(requested_stage)?;
    if stage_id != requested_stage
        || stage_id.len() != 2
        || !stage_id.chars().all(|character| character.is_ascii_digit())
    {
        return Err(AdmError::new(
            "pipeline artifact stage_id must be a two-digit identifier",
        ));
    }
    if !artifact_root.is_dir() {
        return Err(AdmError::new("pipeline artifact root is unavailable"));
    }
    let stage_name = format!("stage_{stage_id}");
    let stage_root = artifact_root.join(&stage_name);
    if !stage_root.is_dir() {
        return Err(AdmError::new("pipeline stage preview is unavailable"));
    }
    let normalized_relative = request.relative_path.trim().replace('\\', "/");
    let first_component = Path::new(&normalized_relative)
        .components()
        .next()
        .and_then(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        });
    if first_component.is_some_and(|value| value.starts_with("stage_") && value != stage_name) {
        return Err(AdmError::new(format!(
            "pipeline preview request escapes root for stage {stage_id}"
        )));
    }
    let includes_stage = first_component == Some(stage_name.as_str());
    let candidate = if includes_stage {
        ensure_relative_path(artifact_root, &normalized_relative)?
    } else {
        ensure_relative_path(&stage_root, &normalized_relative)?
    };
    if !candidate.is_file() {
        return Err(AdmError::new("pipeline preview image is unavailable"));
    }
    let candidate = ensure_child_path(&stage_root, &candidate)?;
    let bytes = fs::read(&candidate)?;
    let limit = request
        .max_bytes
        .unwrap_or(1024 * 1024)
        .clamp(1, 4 * 1024 * 1024);
    let truncated = bytes.len() > limit;
    let selected = &bytes[..bytes.len().min(limit)];
    let content_type = artifact_content_type(&candidate).to_string();
    let (encoding, content) = if is_text_artifact(&content_type) {
        match String::from_utf8(selected.to_vec()) {
            Ok(text) => ("utf-8".to_string(), text),
            Err(_) => ("base64".to_string(), base64_encode(selected)),
        }
    } else {
        ("base64".to_string(), base64_encode(selected))
    };
    Ok(ArtifactContentView {
        relative_path: if includes_stage {
            normalized_relative
        } else {
            format!("{stage_name}/{normalized_relative}")
        },
        name: candidate
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string(),
        size_bytes: bytes.len() as u64,
        content_type,
        encoding,
        content,
        truncated,
    })
}

fn artifact_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => "application/json",
        "md" | "markdown" => "text/markdown",
        "txt" | "log" => "text/plain",
        "html" => "text/html",
        "css" => "text/css",
        "js" => "text/javascript",
        "csv" => "text/csv",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn is_text_artifact(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || matches!(content_type, "application/json" | "image/svg+xml")
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(TABLE[(first >> 2) as usize] as char);
        output.push(TABLE[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn semantic_quality_from_runtime(
    runtime: Option<&PipelineStageRuntime>,
) -> PipelineSemanticQualityView {
    let Some(result) = runtime.and_then(|runtime| runtime.result.as_ref()) else {
        return empty_semantic_quality();
    };
    let explicit = result
        .outputs
        .get("semantic_quality")
        .or_else(|| result.outputs.get("semanticQuality"))
        .cloned()
        .unwrap_or(Value::Null);
    let return_targets = if let Some(items) = explicit
        .get("return_targets")
        .or_else(|| explicit.get("returnTargets"))
        .and_then(Value::as_array)
    {
        items
            .iter()
            .map(|item| {
                let code = string_field(item, "code").unwrap_or_else(|| "REVIEW_ISSUE".to_string());
                let message = string_field(item, "message")
                    .or_else(|| string_field(item, "message_zh"))
                    .map(|message| safe_user_text(&message))
                    .unwrap_or_default();
                PipelineIssueReturnView {
                    severity: string_field(item, "severity")
                        .unwrap_or_else(|| "warning".to_string()),
                    return_target: string_field(item, "return_target")
                        .or_else(|| string_field(item, "returnTarget"))
                        .unwrap_or_else(|| pipeline_return_target_for_code(&code).to_string()),
                    source_file: String::new(),
                    code,
                    message,
                }
            })
            .collect()
    } else if let Some(items) = structured_return_targets(result) {
        items
    } else {
        derived_return_targets(result)
    };
    let status = string_field(&explicit, "status").unwrap_or_else(|| {
        semantic_status_from_result(&result.status, &return_targets, !result.warnings.is_empty())
    });
    PipelineSemanticQualityView {
        status,
        project_specificity_score: semantic_metric(
            &explicit,
            &result.outputs,
            &["project_specificity_score", "projectSpecificityScore"],
        ),
        required_semantic_coverage: semantic_metric(
            &explicit,
            &result.outputs,
            &[
                "required_semantic_coverage",
                "requiredSemanticCoverage",
                "semantic_coverage",
            ],
        ),
        generic_template_ratio: semantic_metric(
            &explicit,
            &result.outputs,
            &[
                "generic_template_ratio",
                "genericTemplateRatio",
                "generic_content_ratio",
            ],
        ),
        placeholder_ratio: semantic_metric(
            &explicit,
            &result.outputs,
            &["placeholder_ratio", "placeholderRatio"],
        ),
        report_files: Vec::new(),
        return_targets,
    }
}

fn structured_return_targets(result: &PipelineStageResult) -> Option<Vec<PipelineIssueReturnView>> {
    let mut items = Vec::new();
    for (container, key, default_severity) in [
        (None, "blocking_issues", "blocked"),
        (None, "blockers", "blocked"),
        (None, "review_items", "warning"),
        (Some("validation_report"), "blocking_issues", "blocked"),
        (Some("validation_report"), "blockers", "blocked"),
        (Some("validation_report"), "review_items", "warning"),
    ] {
        let direct = if let Some(name) = container {
            result.outputs.get(name).and_then(|root| {
                root.get(key).or_else(|| {
                    root.get("business_quality")
                        .and_then(|business| business.get(key))
                })
            })
        } else {
            result.outputs.get(key)
        };
        let Some(values) = direct.and_then(Value::as_array) else {
            continue;
        };
        for item in values {
            let Some(code) = string_field(item, "code") else {
                continue;
            };
            let message = string_field(item, "message")
                .or_else(|| string_field(item, "message_zh"))
                .map(|message| safe_user_text(&message))
                .unwrap_or_default();
            let return_target = string_field(item, "return_target")
                .or_else(|| string_field(item, "returnTarget"))
                .or_else(|| {
                    item.get("return_target")
                        .or_else(|| item.get("returnTarget"))
                        .and_then(|target| {
                            target
                                .get("stage_id")
                                .or_else(|| target.get("stageId"))
                                .and_then(Value::as_str)
                                .map(|stage_id| stage_id.to_string())
                        })
                })
                .unwrap_or_else(|| pipeline_return_target_for_code(&code).to_string());
            items.push(PipelineIssueReturnView {
                severity: string_field(item, "severity")
                    .unwrap_or_else(|| default_severity.to_string()),
                code,
                message,
                return_target,
                source_file: String::new(),
            });
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    items.retain(|item| seen.insert((item.code.clone(), item.message.clone())));
    (!items.is_empty()).then_some(items)
}

fn empty_semantic_quality() -> PipelineSemanticQualityView {
    PipelineSemanticQualityView {
        status: "missing".to_string(),
        project_specificity_score: None,
        required_semantic_coverage: None,
        generic_template_ratio: None,
        placeholder_ratio: None,
        return_targets: Vec::new(),
        report_files: Vec::new(),
    }
}

fn derived_return_targets(result: &PipelineStageResult) -> Vec<PipelineIssueReturnView> {
    result
        .errors
        .iter()
        .map(|message| derived_issue_return("blocked", message))
        .chain(
            result
                .warnings
                .iter()
                .map(|message| derived_issue_return("warning", message)),
        )
        .collect()
}

fn derived_issue_return(severity: &str, message: &str) -> PipelineIssueReturnView {
    let code = infer_pipeline_issue_code(message);
    PipelineIssueReturnView {
        severity: severity.to_string(),
        code: code.clone(),
        message: safe_user_text(message),
        return_target: pipeline_return_target_for_code(&code).to_string(),
        source_file: String::new(),
    }
}

fn semantic_status_from_result(
    status: &StageStatus,
    return_targets: &[PipelineIssueReturnView],
    has_warnings: bool,
) -> String {
    match status {
        StageStatus::Failed | StageStatus::Blocked | StageStatus::Stopped => "blocked".to_string(),
        StageStatus::CompletedWithReview => "warning".to_string(),
        _ if return_targets
            .iter()
            .any(|target| target.severity == "blocked") =>
        {
            "blocked".to_string()
        }
        _ if has_warnings || !return_targets.is_empty() => "warning".to_string(),
        StageStatus::Success => "success".to_string(),
        _ => "missing".to_string(),
    }
}

fn semantic_metric(
    explicit: &Value,
    outputs: &std::collections::BTreeMap<String, Value>,
    keys: &[&str],
) -> Option<f64> {
    for key in keys {
        if let Some(value) = explicit.get(*key).and_then(Value::as_f64) {
            return Some(normalize_ratio(value));
        }
        if let Some(value) = explicit
            .get("metrics")
            .and_then(|metrics| metrics.get(*key))
            .and_then(Value::as_f64)
        {
            return Some(normalize_ratio(value));
        }
        if let Some(value) = outputs.get(*key).and_then(Value::as_f64) {
            return Some(normalize_ratio(value));
        }
    }
    None
}

fn normalize_ratio(value: f64) -> f64 {
    if value > 1.0 { value / 100.0 } else { value }
}

fn infer_pipeline_issue_code(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    if lower.contains("placeholder") || message.contains("占位") {
        "PLACEHOLDER_TOKEN_REMAINS".to_string()
    } else if lower.contains("source trace") || message.contains("来源追踪") {
        "SOURCE_TRACE_MISSING".to_string()
    } else if lower.contains("semantic")
        || lower.contains("alignment")
        || message.contains("语义")
        || message.contains("对齐")
    {
        "SEMANTIC_ALIGNMENT_GAP".to_string()
    } else if lower.contains("dependency") || message.contains("依赖") {
        "REVIEW_ISSUE".to_string()
    } else {
        "UNCLASSIFIED_ISSUE".to_string()
    }
}

fn pipeline_return_target_for_code(code: &str) -> &'static str {
    match code {
        "PROGRAM_CAPABILITY_NOT_BOUND" => "Step03 程序能力合约",
        "CORE_ENTITY_WITHOUT_ASSET_STRATEGY" => "Step04 美术资产策略",
        "STYLE_ARCHETYPE_MISMATCH" | "STYLE_OVERRIDE_REASON_MISSING" => "Step07 风格确认",
        "GENERIC_PLAN_DOMINANCE" | "PROGRAM_TASK_WITHOUT_PROJECT_REF" => {
            "Step08，必要时回 Step02/03"
        }
        "CORE_ASSET_NOT_PRODUCED" | "PLACEHOLDER_ONLY_CORE_ASSET" => "Step09 或 Step04",
        "SEMANTIC_ALIGNMENT_GAP" | "PLACEHOLDER_ONLY_ALIGNMENT" => "Step10 根据缺口返回来源阶段",
        "PLACEHOLDER_TOKEN_REMAINS" => "对应来源 stage 报告，通常返回 Step03/04/09",
        "SOURCE_TRACE_MISSING" => "对应来源 stage 补齐 source_refs",
        _ => "查看对应 stage 报告并返回最近的上游设计步骤",
    }
}

fn style_options_from_state(state: &PipelineRunState) -> Vec<StyleOptionView> {
    state
        .stages
        .get("07")
        .and_then(|runtime| runtime.result.as_ref())
        .and_then(|result| {
            result
                .outputs
                .get("style_options")
                .or_else(|| result.outputs.get("styleOptions"))
        })
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .map(|(index, item)| StyleOptionView {
                    option_id: string_field(item, "optionId")
                        .or_else(|| string_field(item, "option_id"))
                        .or_else(|| string_field(item, "styleId"))
                        .or_else(|| string_field(item, "style_id"))
                        .or_else(|| string_field(item, "id"))
                        .unwrap_or_else(|| format!("style_{}", index + 1)),
                    title: string_field(item, "title")
                        .unwrap_or_else(|| format!("Style {}", index + 1)),
                    description: string_field(item, "description").unwrap_or_default(),
                    image_path: string_field(item, "imagePath")
                        .or_else(|| string_field(item, "image_path"))
                        .unwrap_or_default(),
                    image_status: string_field(item, "imageStatus")
                        .or_else(|| string_field(item, "image_status"))
                        .unwrap_or_default(),
                    image_message: string_field(item, "imageMessage")
                        .or_else(|| string_field(item, "image_message"))
                        .map(|message| safe_user_text(&message))
                        .unwrap_or_default(),
                    selected: item
                        .get("selected")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn message_field(message: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    message
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix(&prefix).map(str::trim))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn safe_user_text(input: &str) -> String {
    const MAX_VISIBLE_CHARS: usize = 480;
    let mut output = Vec::new();
    let mut redact_next = false;
    for raw in input.split_whitespace() {
        let token = raw.trim_matches(|character: char| {
            matches!(
                character,
                '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
            )
        });
        let lower = token.to_ascii_lowercase();
        let secret_label = matches!(
            lower.as_str(),
            "bearer" | "authorization" | "api_key" | "apikey" | "access_token" | "accesstoken"
        );
        let secret_assignment = [
            "api_key=",
            "apikey=",
            "access_token=",
            "accesstoken=",
            "authorization=",
            "token=",
            "password=",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix));
        let url = lower.starts_with("http://") || lower.starts_with("https://");
        let path = token.contains('\\')
            || token.contains('/')
            || (token.len() >= 3
                && token.as_bytes()[1] == b':'
                && token.as_bytes()[0].is_ascii_alphabetic());
        let jwt = token.matches('.').count() == 2
            && token
                .split('.')
                .all(|part| part.len() >= 8 && part.chars().all(is_base64_character));
        let long_base64 = token.len() >= 64 && token.chars().all(is_base64_character);
        let known_secret = lower.starts_with("sk-")
            || lower.starts_with("pk-")
            || lower.starts_with("ghp_")
            || lower.starts_with("xoxb-");
        let replacement = if redact_next || secret_assignment || jwt || long_base64 || known_secret
        {
            redact_next = false;
            "[REDACTED]"
        } else if secret_label {
            redact_next = true;
            "[REDACTED]"
        } else if url {
            "[REMOTE_URL]"
        } else if path {
            "[PATH]"
        } else {
            raw
        };
        output.push(replacement);
        if output.join(" ").chars().count() >= MAX_VISIBLE_CHARS {
            break;
        }
    }
    let mut safe = output
        .join(" ")
        .chars()
        .take(MAX_VISIBLE_CHARS)
        .collect::<String>();
    if input.chars().count() > MAX_VISIBLE_CHARS {
        safe.push('…');
    }
    safe
}

fn is_base64_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '+' | '/' | '_' | '-' | '=')
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use adm_new_contracts::pipeline::{
        PipelineRegistry, PipelineStageRuntime, StageKind, StageSpec, StageStatus,
    };
    use adm_new_foundation::{AdmError, AdmResult, new_stable_id};

    #[test]
    fn run_pipeline_request_defaults_artifact_locale_and_accepts_explicit_locale() {
        let legacy: RunPipelineRangeRequest = serde_json::from_value(serde_json::json!({
            "from_stage_id": "00",
            "to_stage_id": "02"
        }))
        .unwrap();
        assert_eq!(legacy.artifact_locale, ArtifactLocale::ZhCn);

        let english: RunPipelineRangeRequest = serde_json::from_value(serde_json::json!({
            "from_stage_id": "00",
            "to_stage_id": "02",
            "artifact_locale": "en-US"
        }))
        .unwrap();
        assert_eq!(english.artifact_locale, ArtifactLocale::EnUs);
    }

    #[test]
    fn pipeline_command_run_range_reports_waiting_confirmation() {
        let service = PipelineApplicationService::new(sample_registry()).unwrap();
        let mut state = empty_state();
        let response = run_pipeline_range(
            &service,
            &mut state,
            RunPipelineRangeRequest {
                from_stage_id: "07".to_string(),
                to_stage_id: "10".to_string(),
                skip_manual_gates: false,
                artifact_locale: ArtifactLocale::default(),
            },
            &StaticExecutor::with_status("07", StageStatus::WaitingConfirmation),
        );
        assert!(response.ok);
        let data = response.data.unwrap();
        assert_eq!(data.report.unwrap().final_status, "waiting_confirmation");
        assert!(data.view.waiting_confirmation);
        assert_eq!(data.view.current_stage_id.as_deref(), Some("07"));
    }

    #[test]
    fn pipeline_command_forwards_stop_observed_after_the_final_stage() {
        struct StopAtBoundaryExecutor(Cell<bool>);

        impl StageExecutor for StopAtBoundaryExecutor {
            fn execute(
                &self,
                _spec: &StageSpec,
                _context: &StageContextModel,
            ) -> PipelineStageResult {
                self.0.set(true);
                PipelineStageResult {
                    status: StageStatus::Success,
                    outputs: BTreeMap::new(),
                    errors: Vec::new(),
                    warnings: Vec::new(),
                    message: "stage completed".to_string(),
                }
            }

            fn stop_requested(&self) -> bool {
                self.0.get()
            }
        }

        let service = PipelineApplicationService::new(sample_registry()).unwrap();
        let mut state = empty_state();
        let response = run_pipeline_range(
            &service,
            &mut state,
            RunPipelineRangeRequest {
                from_stage_id: "10".to_string(),
                to_stage_id: "10".to_string(),
                skip_manual_gates: false,
                artifact_locale: ArtifactLocale::default(),
            },
            &StopAtBoundaryExecutor(Cell::new(false)),
        );

        assert!(response.ok);
        let data = response.data.unwrap();
        assert_eq!(data.report.unwrap().final_status, "stopped");
        assert!(data.view.state.stop_requested);
        assert_eq!(data.view.state.stages["10"].status, StageStatus::Success);
    }

    #[test]
    fn pipeline_command_skip_manual_gate_continues_and_clears_stale_stop() {
        let service = PipelineApplicationService::new(sample_registry()).unwrap();
        let mut state = empty_state();
        state.stop_requested = true;
        let response = run_pipeline_range(
            &service,
            &mut state,
            RunPipelineRangeRequest {
                from_stage_id: "07".to_string(),
                to_stage_id: "10".to_string(),
                skip_manual_gates: true,
                artifact_locale: ArtifactLocale::default(),
            },
            &StaticExecutor::with_status("07", StageStatus::WaitingConfirmation),
        );

        assert!(response.ok);
        let data = response.data.unwrap();
        assert_eq!(data.report.unwrap().final_status, "success");
        assert_eq!(data.view.state.stages["07"].status, StageStatus::Skipped);
        assert_eq!(data.view.state.stages["10"].status, StageStatus::Success);
        assert!(!data.view.state.stop_requested);
        let gate_result = data.view.state.stages["07"].result.as_ref().unwrap();
        assert_eq!(gate_result.outputs["manual_gate_skipped"], true);
        assert!(!gate_result.warnings.is_empty());
    }

    #[test]
    fn pipeline_command_stop_and_confirm_style_update_state() {
        let service = PipelineApplicationService::new(sample_registry()).unwrap();
        let mut state = empty_state();
        let stopped = stop_pipeline(&service, &mut state);
        assert!(stopped.ok);
        assert!(stopped.data.unwrap().state.stop_requested);

        let confirmed = confirm_style(
            &service,
            &mut state,
            ConfirmStyleRequest {
                stage_id: "07".to_string(),
                message: "style confirmed".to_string(),
                selected_style_id: String::new(),
                notes: String::new(),
            },
        );
        assert!(confirmed.ok);
        let view = confirmed.data.unwrap();
        assert_eq!(view.state.status, "style_confirmed");
        assert_eq!(view.state.stages["07"].status, StageStatus::Success);
    }

    #[test]
    fn pipeline_view_includes_stage_summaries_and_style_options() {
        let service = PipelineApplicationService::new(sample_registry()).unwrap();
        let state = PipelineRunState {
            stages: BTreeMap::from([(
                "07".to_string(),
                PipelineStageRuntime {
                    stage_id: "07".to_string(),
                    status: StageStatus::WaitingConfirmation,
                    started_at: "unix:1".to_string(),
                    completed_at: String::new(),
                    result: Some(PipelineStageResult {
                        status: StageStatus::WaitingConfirmation,
                        outputs: BTreeMap::from([(
                            "style_options".to_string(),
                            serde_json::json!([
                                {
                                    "style_id": "STYLE-01-stylized",
                                    "title": "Stylized",
                                    "description": "Readable silhouettes",
                                    "image_path": "style/stylized.png",
                                    "image_status": "fallback",
                                    "selected": true
                                }
                            ]),
                        )]),
                        errors: Vec::new(),
                        warnings: Vec::new(),
                        message: "choose a style".to_string(),
                    }),
                },
            )]),
            current_stage_id: Some("07".to_string()),
            status: "waiting_confirmation".to_string(),
            ..empty_state()
        };
        let response = load_pipeline_view(&service, &state);
        assert!(response.ok);
        let view = response.data.unwrap();
        assert_eq!(view.stages[0].title, "Stage 07");
        assert!(view.stages.iter().any(|stage| stage.is_step07));
        assert_eq!(view.style_options[0].option_id, "STYLE-01-stylized");
        assert_eq!(view.style_options[0].image_status, "fallback");
        assert!(view.style_options[0].selected);
    }

    #[test]
    fn pipeline_view_includes_semantic_quality_return_paths() {
        let service = PipelineApplicationService::new(sample_registry()).unwrap();
        let state = PipelineRunState {
            stages: BTreeMap::from([(
                "10".to_string(),
                PipelineStageRuntime {
                    stage_id: "10".to_string(),
                    status: StageStatus::CompletedWithReview,
                    started_at: "unix:1".to_string(),
                    completed_at: "unix:2".to_string(),
                    result: Some(PipelineStageResult {
                        status: StageStatus::CompletedWithReview,
                        outputs: BTreeMap::from([(
                            "semantic_quality".to_string(),
                            serde_json::json!({
                                "status": "blocked",
                                "metrics": {
                                    "project_specificity_score": 0.82,
                                    "required_semantic_coverage": 0.64,
                                    "generic_template_ratio": 0.18,
                                    "placeholder_ratio": 0.07
                                },
                                "return_targets": [
                                    {
                                        "severity": "blocked",
                                        "code": "SEMANTIC_ALIGNMENT_GAP",
                                        "message": "program and art plans diverge",
                                        "source_file": "semantic_alignment_report.json"
                                    }
                                ],
                                "report_files": ["semantic_alignment_report.json"]
                            }),
                        )]),
                        errors: Vec::new(),
                        warnings: vec!["semantic alignment gap".to_string()],
                        message: "review required".to_string(),
                    }),
                },
            )]),
            current_stage_id: Some("10".to_string()),
            status: "completed_with_review".to_string(),
            ..empty_state()
        };
        let response = load_pipeline_view(&service, &state);
        assert!(response.ok);
        let view = response.data.unwrap();
        let stage = view
            .stages
            .iter()
            .find(|stage| stage.stage_id == "10")
            .unwrap();
        assert_eq!(stage.semantic_quality.status, "blocked");
        assert_eq!(
            stage.semantic_quality.required_semantic_coverage,
            Some(0.64)
        );
        assert_eq!(
            stage.semantic_quality.return_targets[0].return_target,
            "Step10 根据缺口返回来源阶段"
        );
        assert!(
            stage.semantic_quality.return_targets[0]
                .source_file
                .is_empty()
        );
        assert!(stage.semantic_quality.report_files.is_empty());
        let json = serde_json::to_value(stage).unwrap();
        assert_eq!(
            json["semantic_quality"]["return_targets"][0]["code"],
            "SEMANTIC_ALIGNMENT_GAP"
        );
    }

    #[test]
    fn pipeline_view_preserves_structured_issue_code_with_chinese_message() {
        let service = PipelineApplicationService::new(sample_registry()).unwrap();
        let state = PipelineRunState {
            stages: BTreeMap::from([(
                "10".to_string(),
                PipelineStageRuntime {
                    stage_id: "10".to_string(),
                    status: StageStatus::CompletedWithReview,
                    started_at: "unix:1".to_string(),
                    completed_at: "unix:2".to_string(),
                    result: Some(PipelineStageResult {
                        status: StageStatus::CompletedWithReview,
                        outputs: BTreeMap::from([(
                            "review_items".to_string(),
                            serde_json::json!([{
                                "severity": "warning",
                                "code": "SEMANTIC_ALIGNMENT_GAP",
                                "message": "程序任务与美术任务的语义未对齐。",
                                "return_target": {"stage_id": "10", "anchor": "alignment"}
                            }]),
                        )]),
                        errors: Vec::new(),
                        warnings: vec!["程序任务与美术任务的语义未对齐。".to_string()],
                        message: "需要复核".to_string(),
                    }),
                },
            )]),
            ..empty_state()
        };

        let view = load_pipeline_view(&service, &state).data.unwrap();
        let stage = view
            .stages
            .iter()
            .find(|stage| stage.stage_id == "10")
            .expect("sample registry should expose Step10");
        let target = &stage.semantic_quality.return_targets[0];
        assert_eq!(target.code, "SEMANTIC_ALIGNMENT_GAP");
        assert_eq!(target.message, "程序任务与美术任务的语义未对齐。");
        assert_eq!(target.return_target, "10");
    }

    #[test]
    fn pipeline_view_redacts_paths_urls_tokens_and_long_base64_from_visible_messages() {
        let service = PipelineApplicationService::new(sample_registry()).unwrap();
        let encoded = "A".repeat(96);
        let state = PipelineRunState {
            stages: BTreeMap::from([(
                "10".to_string(),
                PipelineStageRuntime {
                    stage_id: "10".to_string(),
                    status: StageStatus::Failed,
                    started_at: "unix:1".to_string(),
                    completed_at: "unix:2".to_string(),
                    result: Some(PipelineStageResult {
                        status: StageStatus::Failed,
                        outputs: BTreeMap::new(),
                        errors: vec![format!(
                            "failed C:\\Users\\Alice\\secret.json https://user:pass@example.test/run?api_key=secret sk-private {encoded}"
                        )],
                        warnings: vec!["Bearer jwt.private.value".to_string()],
                        message: "inspect stage_10/private/report.json".to_string(),
                    }),
                },
            )]),
            ..empty_state()
        };
        let view = load_pipeline_view(&service, &state).data.unwrap();
        let serialized = serde_json::to_string(&view).unwrap();
        assert!(!serialized.contains("Alice"));
        assert!(!serialized.contains("example.test"));
        assert!(!serialized.contains("sk-private"));
        assert!(!serialized.contains(&encoded));
        assert!(!serialized.contains("private/report.json"));
        assert!(serialized.contains("[REDACTED]"));
        assert!(serialized.contains("[PATH]"));
    }

    #[test]
    fn pipeline_view_exposes_outputs_errors_warnings_and_artifacts() {
        let service = PipelineApplicationService::new(sample_registry()).unwrap();
        let state = PipelineRunState {
            stages: BTreeMap::from([(
                "10".to_string(),
                PipelineStageRuntime {
                    stage_id: "10".to_string(),
                    status: StageStatus::Failed,
                    started_at: "unix:1".to_string(),
                    completed_at: "unix:2".to_string(),
                    result: Some(PipelineStageResult {
                        status: StageStatus::Failed,
                        outputs: BTreeMap::from([
                            (
                                "validation_report".to_string(),
                                serde_json::json!({"valid": false}),
                            ),
                            (
                                "artifact_records".to_string(),
                                serde_json::json!([{
                                    "relative_path": "stage_10/validation_report.json",
                                    "name": "validation_report.json",
                                    "size_bytes": 24,
                                    "content_type": "application/json",
                                    "content_preview": "{\"valid\":false}",
                                    "is_binary": false
                                }]),
                            ),
                        ]),
                        errors: vec!["alignment failed".to_string()],
                        warnings: vec!["review source references".to_string()],
                        message: "failed".to_string(),
                    }),
                },
            )]),
            current_stage_id: Some("10".to_string()),
            status: "failed".to_string(),
            ..empty_state()
        };

        let response = load_pipeline_view(&service, &state);
        assert!(response.ok);
        let view = response.data.unwrap();
        let stage = view
            .stages
            .iter()
            .find(|stage| stage.stage_id == "10")
            .unwrap();
        assert_eq!(stage.outputs["validation_report"]["valid"], false);
        assert_eq!(stage.errors, vec!["alignment failed"]);
        assert_eq!(stage.warnings, vec!["review source references"]);
        assert_eq!(stage.artifacts[0].name, "validation_report.json");
        assert_eq!(stage.artifacts[0].content_type, "application/json");
        let serialized = serde_json::to_string(&view).unwrap();
        assert!(!serialized.contains("artifact_records"));
        assert!(!serialized.contains("validation_report.json"));
        assert!(!serialized.contains("content_preview"));
        let serialized: Value = serde_json::from_str(&serialized).unwrap();
        assert!(serialized["state"].get("stages").is_none());
        assert!(serialized["stages"][10].get("outputs").is_none());
        assert!(serialized["stages"][10].get("artifacts").is_none());
    }

    #[test]
    fn pipeline_view_exposes_bounded_completion_five_states_without_raw_outputs() {
        let service = PipelineApplicationService::new(PipelineRegistry {
            stages: vec![
                stage("00", vec![]),
                stage("01", vec![]),
                stage("02", vec![]),
                stage("03", vec![]),
                stage("04", vec![]),
            ],
        })
        .unwrap();
        let state = PipelineRunState {
            stages: BTreeMap::from([
                (
                    "01".to_string(),
                    completion_runtime(
                        "01",
                        "failed",
                        serde_json::json!({
                            "status": "failed",
                            "task_id": "task-01",
                            "candidate_patch_id": "patch-01",
                            "risk": {"level": "high"},
                            "audit": {
                                "model_config_id": "local-codex",
                                "attempts": 2,
                                "confirmation": {
                                    "mode": "attended",
                                    "actor": "reviewer",
                                    "accepted": false
                                },
                                "errors": [
                                    "failed at https://example.test/run with token=secret"
                                ]
                            }
                        }),
                    ),
                ),
                (
                    "02".to_string(),
                    completion_runtime(
                        "02",
                        "rejected",
                        serde_json::json!({"status": "rejected", "audit": {"attempts": 1}}),
                    ),
                ),
                (
                    "03".to_string(),
                    completion_runtime(
                        "03",
                        "confirmed",
                        serde_json::json!({"status": "confirmed", "audit": {"attempts": 1}}),
                    ),
                ),
                (
                    "04".to_string(),
                    completion_runtime(
                        "04",
                        "committed",
                        serde_json::json!({"status": "committed", "audit": {"attempts": 1}}),
                    ),
                ),
            ]),
            ..empty_state()
        };

        let view = load_pipeline_view(&service, &state).data.unwrap();
        let by_id = |id: &str| {
            view.stages
                .iter()
                .find(|stage| stage.stage_id == id)
                .unwrap()
        };
        assert_eq!(by_id("00").bounded_completion.status, "not_called");
        assert_eq!(by_id("01").bounded_completion.status, "failed");
        assert_eq!(
            by_id("01").bounded_completion.model_config_id,
            "local-codex"
        );
        assert_eq!(by_id("01").bounded_completion.risk.as_deref(), Some("high"));
        assert_eq!(by_id("01").bounded_completion.attempts, 2);
        assert_eq!(
            by_id("01").bounded_completion.confirmation_mode.as_deref(),
            Some("attended")
        );
        assert_eq!(
            by_id("01").bounded_completion.confirmation_accepted,
            Some(false)
        );
        assert_eq!(by_id("02").bounded_completion.status, "rejected");
        assert_eq!(by_id("03").bounded_completion.status, "confirmed");
        assert_eq!(by_id("04").bounded_completion.status, "committed");
        let serialized = serde_json::to_string(&view).unwrap();
        assert!(serialized.contains("bounded_completion"));
        assert!(serialized.contains("not_called"));
        assert!(serialized.contains("committed"));
        assert!(!serialized.contains("example.test"));
        assert!(!serialized.contains("token=secret"));
        let json: Value = serde_json::from_str(&serialized).unwrap();
        assert!(json["stages"][1].get("outputs").is_none());
        assert!(json["stages"][1].get("artifacts").is_none());
    }

    #[test]
    fn pipeline_artifact_read_returns_content_and_rejects_path_escape() {
        let root = temp_root("pipeline_artifact_read");
        let artifact_root = root.join("artifacts");
        let report_path = artifact_root.join("stage_00/report.json");
        fs::create_dir_all(report_path.parent().unwrap()).unwrap();
        fs::write(&report_path, br#"{"status":"success"}"#).unwrap();
        let other_stage_path = artifact_root.join("stage_01/private.json");
        fs::create_dir_all(other_stage_path.parent().unwrap()).unwrap();
        fs::write(&other_stage_path, b"{}").unwrap();

        let response = read_pipeline_artifact(
            &artifact_root,
            ReadPipelineArtifactRequest {
                stage_id: "00".to_string(),
                relative_path: "stage_00/report.json".to_string(),
                max_bytes: None,
            },
        );
        assert!(response.ok);
        let content = response.data.unwrap();
        assert_eq!(content.encoding, "utf-8");
        assert_eq!(content.content_type, "application/json");
        assert!(content.content.contains("success"));
        assert!(!content.truncated);

        let escaped = read_pipeline_artifact(
            &artifact_root,
            ReadPipelineArtifactRequest {
                stage_id: "00".to_string(),
                relative_path: "../autosave_state.json".to_string(),
                max_bytes: None,
            },
        );
        assert!(!escaped.ok);
        assert_eq!(escaped.error.unwrap().code, "PATH_GUARD_FAILED");

        let cross_stage = read_pipeline_artifact(
            &artifact_root,
            ReadPipelineArtifactRequest {
                stage_id: "00".to_string(),
                relative_path: "stage_01/private.json".to_string(),
                max_bytes: None,
            },
        );
        assert!(!cross_stage.ok);
        assert_eq!(cross_stage.error.unwrap().code, "PATH_GUARD_FAILED");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn pipeline_confirm_style_records_selected_option() {
        let service = PipelineApplicationService::new(sample_registry()).unwrap();
        let mut state = PipelineRunState {
            stages: BTreeMap::from([(
                "07".to_string(),
                PipelineStageRuntime {
                    stage_id: "07".to_string(),
                    status: StageStatus::WaitingConfirmation,
                    started_at: "unix:1".to_string(),
                    completed_at: String::new(),
                    result: Some(PipelineStageResult {
                        status: StageStatus::WaitingConfirmation,
                        outputs: BTreeMap::from([(
                            "style_options".to_string(),
                            serde_json::json!([
                                {
                                    "option_id": "readable",
                                    "title": "Readable",
                                    "description": "Production",
                                    "image_path": "generated_images/readable.png"
                                },
                                {
                                    "option_id": "painterly",
                                    "title": "Painterly",
                                    "description": "Concept",
                                    "image_path": "generated_images/painterly.png"
                                }
                            ]),
                        )]),
                        errors: Vec::new(),
                        warnings: Vec::new(),
                        message: "choose".to_string(),
                    }),
                },
            )]),
            current_stage_id: Some("07".to_string()),
            status: "waiting_confirmation".to_string(),
            ..empty_state()
        };

        let response = confirm_style(
            &service,
            &mut state,
            ConfirmStyleRequest {
                stage_id: "07".to_string(),
                selected_style_id: "painterly".to_string(),
                notes: "Use warmer lighting.".to_string(),
                message: "style=painterly; notes=Use warmer lighting.".to_string(),
            },
        );

        assert!(response.ok);
        let view = response.data.unwrap();
        assert_eq!(view.state.status, "style_confirmed");
        assert_eq!(view.style_options[1].option_id, "painterly");
        assert!(view.style_options[1].selected);
        let outputs = &view.state.stages["07"].result.as_ref().unwrap().outputs;
        assert_eq!(
            outputs["style_confirmation"]["selected_style_id"],
            "painterly"
        );
        assert_eq!(
            outputs["style_confirmation"]["notes"],
            "Use warmer lighting."
        );
    }

    #[test]
    fn pipeline_command_errors_are_mapped() {
        let service = PipelineApplicationService::new(sample_registry()).unwrap();
        let mut state = empty_state();
        let response = run_pipeline_range(
            &service,
            &mut state,
            RunPipelineRangeRequest {
                from_stage_id: "missing".to_string(),
                to_stage_id: "10".to_string(),
                skip_manual_gates: false,
                artifact_locale: ArtifactLocale::default(),
            },
            &StaticExecutor::success(),
        );
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "VALIDATION_FAILED");

        let response = run_pipeline_range(
            &service,
            &mut state,
            RunPipelineRangeRequest {
                from_stage_id: "99".to_string(),
                to_stage_id: "10".to_string(),
                skip_manual_gates: false,
                artifact_locale: ArtifactLocale::default(),
            },
            &StaticExecutor::success(),
        );
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "NOT_FOUND");
    }

    #[test]
    fn pipeline_command_wrapper_calls_service_trait_mock() {
        let service = MockPipelineService {
            load_calls: Cell::new(0),
        };
        let state = empty_state();
        let response = load_pipeline_view(&service, &state);
        assert!(response.ok);
        assert_eq!(service.load_calls.get(), 1);
    }

    struct MockPipelineService {
        load_calls: Cell<usize>,
    }

    impl PipelineCommandService for MockPipelineService {
        fn load_pipeline_view(&self, state: &PipelineRunState) -> AdmResult<PipelineView> {
            self.load_calls.set(self.load_calls.get() + 1);
            Ok(view_from_state(Vec::new(), Vec::new(), state.clone()))
        }

        fn run_pipeline_range(
            &self,
            _: &mut PipelineRunState,
            _: &RunPipelineRangeRequest,
            _: &dyn StageExecutor,
        ) -> AdmResult<PipelineCommandView> {
            Err(AdmError::new("mock run not implemented"))
        }

        fn stop_pipeline(&self, _: &mut PipelineRunState) -> AdmResult<PipelineView> {
            Err(AdmError::new("mock stop not implemented"))
        }

        fn confirm_style(
            &self,
            _: &mut PipelineRunState,
            _: &ConfirmStyleRequest,
        ) -> AdmResult<PipelineView> {
            Err(AdmError::new("mock confirm not implemented"))
        }
    }

    #[derive(Debug, Clone)]
    struct StaticExecutor {
        statuses: BTreeMap<String, StageStatus>,
    }

    impl StaticExecutor {
        fn success() -> Self {
            Self {
                statuses: BTreeMap::new(),
            }
        }

        fn with_status(stage_id: &str, status: StageStatus) -> Self {
            Self {
                statuses: BTreeMap::from([(stage_id.to_string(), status)]),
            }
        }
    }

    impl StageExecutor for StaticExecutor {
        fn execute(&self, spec: &StageSpec, _: &StageContextModel) -> PipelineStageResult {
            PipelineStageResult {
                status: self
                    .statuses
                    .get(&spec.stage_id)
                    .cloned()
                    .unwrap_or(StageStatus::Success),
                outputs: BTreeMap::new(),
                errors: Vec::new(),
                warnings: Vec::new(),
                message: spec.stage_id.clone(),
            }
        }
    }

    fn sample_registry() -> PipelineRegistry {
        PipelineRegistry {
            stages: vec![
                stage("07", vec![]),
                stage("08", vec![]),
                stage("09", vec!["07"]),
                stage("10", vec!["08", "09"]),
            ],
        }
    }

    fn stage(stage_id: &str, requires: Vec<&str>) -> StageSpec {
        StageSpec {
            stage_id: stage_id.to_string(),
            kind: if stage_id == "07" {
                StageKind::HumanGate
            } else {
                StageKind::Development
            },
            number: stage_id.parse::<u32>().ok(),
            slug: format!("stage_{stage_id}"),
            title: format!("Stage {stage_id}"),
            requires: requires.into_iter().map(str::to_string).collect(),
            source_groups: Vec::new(),
            plugin_ref: String::new(),
            metadata: BTreeMap::new(),
        }
    }

    fn empty_state() -> PipelineRunState {
        PipelineRunState {
            run_id: "run-command".to_string(),
            status: "idle".to_string(),
            stop_requested: false,
            current_stage_id: None,
            stages: BTreeMap::new(),
            ..PipelineRunState::default()
        }
    }

    fn completion_runtime(
        stage_id: &str,
        message: &str,
        completion: Value,
    ) -> PipelineStageRuntime {
        PipelineStageRuntime {
            stage_id: stage_id.to_string(),
            status: StageStatus::Success,
            started_at: "unix:1".to_string(),
            completed_at: "unix:2".to_string(),
            result: Some(PipelineStageResult {
                status: StageStatus::Success,
                outputs: BTreeMap::from([("bounded_completion".to_string(), completion)]),
                errors: Vec::new(),
                warnings: Vec::new(),
                message: message.to_string(),
            }),
        }
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
        fs::create_dir_all(&root).unwrap();
        root
    }
}
