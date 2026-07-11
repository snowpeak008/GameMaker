use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adm_new_application::{
    StageExecutor, style_image_generator_from_config, work_unit_executor_from_config,
};
use adm_new_contracts::log::LogLevel;
use adm_new_contracts::pipeline::{
    CanonicalPipelineRange, PipelineCheckpoint, PipelineCheckpointStatus, PipelineFingerprints,
    PipelineRecoverySummary, PipelineResumePolicy, PipelineRunIdentity, PipelineStageResult,
    PipelineUnitStatus, StageContextModel, StageSpec, StageStatus,
};
use adm_new_foundation::paths::resolve_configured_path;
use adm_new_foundation::{new_stable_id, sha256_hex};
use adm_new_pipeline::{
    PipelineCheckpointObserver, ProductPipelineExecutor, WorkUnitStopToken,
    initial_whole_stage_checkpoint,
};
use adm_new_storage::PipelineCheckpointRepository;
use adm_new_tauri_commands::pipeline::{
    self, ArtifactContentView, ConfirmStyleRequest, PipelineCommandView, PipelineRunReportView,
    PipelineView, ReadPipelineArtifactRequest, ResumePipelineRequest, RunPipelineRangeRequest,
};
use adm_new_tauri_commands::{
    CommandAdapterResult, command_error, command_failure, command_success,
};
use tauri::{AppHandle, Manager, State};

use crate::runtime::{AppRuntime, with_runtime};

const PIPELINE_PROTOCOL_FINGERPRINT: &str =
    "pipeline-protocol-v2-work-units-v2-step07-units-v1-artifact-locale-v1";
const LEGACY_PIPELINE_PROTOCOL_FINGERPRINT: &str =
    "pipeline-protocol-v2-work-units-v2-step07-units-v1";

#[derive(Clone)]
struct StopAwareExecutor {
    inner: ProductPipelineExecutor,
    stop: Arc<AtomicBool>,
}

enum PipelineWorkerOutcome {
    Completed(
        CommandAdapterResult<PipelineCommandView>,
        adm_new_contracts::pipeline::PipelineRunState,
    ),
    PreparationFailed,
}

impl StageExecutor for StopAwareExecutor {
    fn execute(&self, spec: &StageSpec, context: &StageContextModel) -> PipelineStageResult {
        if self.stop.load(Ordering::SeqCst) {
            return PipelineStageResult {
                status: StageStatus::Stopped,
                outputs: Default::default(),
                errors: Vec::new(),
                warnings: Vec::new(),
                message: "stop requested before stage execution".to_string(),
            };
        }
        self.inner.execute(spec, context)
    }

    fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    fn skip_manual_gate(
        &self,
        spec: &StageSpec,
        context: &StageContextModel,
        result: PipelineStageResult,
    ) -> PipelineStageResult {
        self.inner.skip_manual_gate(spec, context, result)
    }
}

#[tauri::command]
pub fn load_pipeline_view(state: State<'_, AppRuntime>) -> CommandAdapterResult<PipelineView> {
    with_runtime(&state, |runtime| {
        let mut response = pipeline::load_pipeline_view(&runtime.pipeline, &runtime.pipeline_state);
        if let Some(view) = response.data.as_mut()
            && matches!(
                view.state.status.as_str(),
                "recoverable" | "recovery_blocked"
            )
            && let Ok(Some(checkpoint)) =
                PipelineCheckpointRepository::new(&runtime.runtime_config.paths().checkpoints_dir)
                    .load_current(&view.state.run_id)
        {
            view.recovery = Some(PipelineRecoverySummary::from(&checkpoint));
        }
        response
    })
}

#[tauri::command]
pub async fn run_pipeline_range(
    app: AppHandle,
    request: RunPipelineRangeRequest,
) -> Result<CommandAdapterResult<PipelineCommandView>, String> {
    start_pipeline_range(app, request, None).await
}

async fn start_pipeline_range(
    app: AppHandle,
    request: RunPipelineRangeRequest,
    resume_checkpoint: Option<PipelineCheckpoint>,
) -> Result<CommandAdapterResult<PipelineCommandView>, String> {
    let state = app.state::<AppRuntime>();
    let resolved = match state.lock() {
        Ok(runtime) => match runtime
            .pipeline
            .resolve_range(&request.from_stage_id, &request.to_stage_id)
        {
            Ok(resolved) => resolved,
            Err(error) => {
                return Ok(command_failure(command_error(
                    "invalid_pipeline_range",
                    error.to_string(),
                )));
            }
        },
        Err(error) => {
            return Ok(command_failure(command_error(
                "runtime_state_unavailable",
                error.to_string(),
            )));
        }
    };
    let canonical_order = resolved.ordered_stage_ids;
    let range_stage_ids = canonical_order[resolved.from_index..=resolved.to_index].to_vec();
    let request = RunPipelineRangeRequest {
        from_stage_id: resolved.from_stage_id,
        to_stage_id: resolved.to_stage_id,
        skip_manual_gates: request.skip_manual_gates,
        artifact_locale: request.artifact_locale,
    };
    let run_guard = match state.try_begin_pipeline_run() {
        Some(guard) => guard,
        None => {
            return Ok(command_failure(command_error(
                "pipeline_already_running",
                "a pipeline range is already running",
            )));
        }
    };
    let stop = state.pipeline_stop_flag();
    let run_id = if let Some(checkpoint) = resume_checkpoint.as_ref() {
        checkpoint.identity.run_id.clone()
    } else {
        match new_stable_id("pipeline_run") {
            Ok(run_id) => run_id,
            Err(error) => {
                return Ok(command_failure(command_error(
                    "pipeline_identity_failed",
                    error.to_string(),
                )));
            }
        }
    };
    let attempt_id = match new_stable_id("pipeline_attempt") {
        Ok(attempt_id) => attempt_id,
        Err(error) => {
            return Ok(command_failure(command_error(
                "pipeline_identity_failed",
                error.to_string(),
            )));
        }
    };
    let (service, executor, mut pipeline_state, project_state, checkpoint_observer) = match state
        .lock()
    {
        Ok(mut runtime) => {
            runtime.ui_language = request.artifact_locale;
            if !runtime.pipeline_executor.protocol_resources_ready() {
                return Ok(command_failure(command_error(
                    "pipeline_protocol_resources_unavailable",
                    if request.artifact_locale.as_str() == "zh-CN" {
                        "流水线注册表或 Schema 资源不可用，已阻止无协议执行。"
                    } else {
                        "The pipeline registry or Schema resources are unavailable; execution without protocol validation was blocked."
                    },
                )));
            }
            if let Err(error) = crate::commands::save::settle_pending_execution_object_ownership(
                &runtime,
                "execution_object_ownership_recovery_before_pipeline",
            ) {
                return Ok(command_failure(command_error(
                    "execution_object_ownership_recovery_failed",
                    error.to_string(),
                )));
            }
            let project_settings = runtime.runtime_config.load_project_settings(false);
            let ai_config_snapshot = runtime.ai_config.load_or_default().unwrap_or_default();
            let input_fingerprint = serde_json::to_vec(&runtime.project_state)
                .map(|bytes| sha256_hex(&bytes))
                .unwrap_or_default();
            let ai_fingerprint_material =
                ai_configuration_fingerprint_material(&ai_config_snapshot);
            let configuration_fingerprint = serde_json::to_vec(&serde_json::json!({
                "project": &project_settings,
                "ai": &ai_fingerprint_material,
                "artifact_locale": request.artifact_locale,
            }))
            .map(|bytes| sha256_hex(&bytes))
            .unwrap_or_default();
            let legacy_configuration_fingerprint = serde_json::to_vec(&serde_json::json!({
                "project": &project_settings,
                "ai": &ai_fingerprint_material,
            }))
            .map(|bytes| sha256_hex(&bytes))
            .unwrap_or_default();
            let project_id = sha256_hex(runtime.project_state.project_name.as_bytes());
            let save_id = runtime.save.current_draft_save_id().unwrap_or_default();
            let execution_object_owner_id = work_unit_execution_object_owner_id(
                &runtime.runtime_config.paths().session_id,
                save_id.as_deref(),
            );
            let execution_plan_fingerprint = sha256_hex(canonical_order.join("\n").as_bytes());
            let current_fingerprints = PipelineFingerprints {
                input: input_fingerprint.clone(),
                configuration: configuration_fingerprint,
                execution_plan: execution_plan_fingerprint.clone(),
                application: format!(
                    "{}:{}",
                    env!("CARGO_PKG_VERSION"),
                    PIPELINE_PROTOCOL_FINGERPRINT,
                ),
            };
            let legacy_zh_fingerprints = PipelineFingerprints {
                input: input_fingerprint,
                configuration: legacy_configuration_fingerprint,
                execution_plan: execution_plan_fingerprint,
                application: format!(
                    "{}:{}",
                    env!("CARGO_PKG_VERSION"),
                    LEGACY_PIPELINE_PROTOCOL_FINGERPRINT,
                ),
            };
            let checkpoint = if let Some(previous) = resume_checkpoint.as_ref() {
                if previous.status != PipelineCheckpointStatus::Recoverable
                    || previous.resume_policy != PipelineResumePolicy::ExplicitOnly
                {
                    return Ok(command_failure(command_error(
                        "pipeline_resume_not_allowed",
                        "the checkpoint is not explicitly recoverable",
                    )));
                }
                if previous.identity.run_id != run_id
                    || previous.identity.project_id != project_id
                    || previous.identity.draft_id != runtime.runtime_config.paths().session_id
                    || previous.identity.save_id != save_id
                    || !resume_fingerprints_match(
                        previous,
                        request.artifact_locale,
                        &current_fingerprints,
                        &legacy_zh_fingerprints,
                    )
                {
                    return Ok(command_failure(command_error(
                        "pipeline_resume_fingerprint_mismatch",
                        "the project, configuration, draft, or execution plan changed",
                    )));
                }
                if previous.units.iter().any(|unit| {
                    unit.reconcile_required
                        || matches!(
                            unit.status,
                            PipelineUnitStatus::Running | PipelineUnitStatus::Unknown
                        )
                }) {
                    return Ok(command_failure(command_error(
                        "pipeline_resume_reconcile_required",
                        "the checkpoint contains an execution unit that cannot be safely reconciled",
                    )));
                }
                let mut checkpoint = previous.clone();
                checkpoint.revision = checkpoint.revision.saturating_add(1);
                checkpoint.identity.parent_attempt_id = Some(previous.identity.attempt_id.clone());
                checkpoint.identity.attempt_id = attempt_id.clone();
                checkpoint.identity.artifact_locale = request.artifact_locale;
                checkpoint.fingerprints = current_fingerprints.clone();
                checkpoint.status = PipelineCheckpointStatus::Resuming;
                checkpoint.resume_policy = PipelineResumePolicy::Disabled;
                checkpoint.current_stage_id = Some(request.from_stage_id.clone());
                checkpoint.current_unit_id = None;
                checkpoint.stop_reason.clear();
                checkpoint.stop_boundary.clear();
                checkpoint.recovery_blocked_reason.clear();
                checkpoint.updated_at = format!("unix:{}", adm_new_foundation::unix_timestamp());
                checkpoint
            } else {
                let mut checkpoint = initial_whole_stage_checkpoint(
                    PipelineRunIdentity {
                        run_id: run_id.clone(),
                        attempt_id: attempt_id.clone(),
                        project_id,
                        draft_id: runtime.runtime_config.paths().session_id.clone(),
                        save_id: save_id.clone(),
                        artifact_locale: request.artifact_locale,
                        ..PipelineRunIdentity::default()
                    },
                    CanonicalPipelineRange {
                        from_stage_id: request.from_stage_id.clone(),
                        to_stage_id: request.to_stage_id.clone(),
                        stage_ids: range_stage_ids.clone(),
                    },
                    current_fingerprints,
                );
                checkpoint.skip_manual_gates = request.skip_manual_gates;
                checkpoint
            };
            let checkpoint_observer = match PipelineCheckpointObserver::new(
                PipelineCheckpointRepository::new(&runtime.runtime_config.paths().checkpoints_dir),
                checkpoint,
            ) {
                Ok(observer) => observer,
                Err(error) => {
                    let _ = error;
                    return Ok(command_failure(command_error(
                        "pipeline_checkpoint_failed",
                        "the pipeline recovery checkpoint could not be initialized",
                    )));
                }
            };
            if let Err(error) = runtime.invalidate_package_result() {
                let _ = error;
                if finalize_checkpoint_failed_for_run(&runtime, &run_id).is_err() {
                    runtime.write_log(
                        LogLevel::Error,
                        "pipeline.recovery",
                        "failed to finalize the initialized pipeline checkpoint",
                    );
                }
                return Ok(command_failure(command_error(
                    "package_result_invalidation_failed",
                    "the previous package result could not be invalidated",
                )));
            }
            runtime.pipeline_state.status = "running".to_string();
            runtime.pipeline_state.run_id = run_id.clone();
            runtime.pipeline_state.parent_attempt_id = resume_checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.identity.attempt_id.clone());
            runtime.pipeline_state.attempt_id = attempt_id.clone();
            runtime.pipeline_state.attempt_no = if resume_checkpoint.is_some() {
                runtime.pipeline_state.attempt_no.saturating_add(1).max(2)
            } else {
                1
            };
            runtime.pipeline_state.from_stage_id = request.from_stage_id.clone();
            runtime.pipeline_state.to_stage_id = request.to_stage_id.clone();
            runtime.pipeline_state.stage_ids = range_stage_ids.clone();
            runtime.pipeline_state.stop_requested = false;
            runtime.pipeline_state.current_stage_id = Some(request.from_stage_id.clone());
            runtime.pipeline_state.current_unit_id =
                Some(format!("{}:stage", request.from_stage_id));
            runtime.pipeline_state.recovery = None;
            runtime.pipeline_state.state_version =
                runtime.pipeline_state.state_version.saturating_add(1);
            if let Err(error) = runtime.persist_pipeline_state() {
                let _ = error;
                if finalize_checkpoint_failed_for_run(&runtime, &run_id).is_err() {
                    runtime.write_log(
                        LogLevel::Error,
                        "pipeline.recovery",
                        "failed to finalize the initialized pipeline checkpoint",
                    );
                }
                runtime.pipeline_state.status = "failed".to_string();
                runtime.pipeline_state.stop_requested = false;
                runtime.write_log(
                    LogLevel::Error,
                    "pipeline",
                    "pipeline state persistence failed",
                );
                return Ok(command_failure(command_error(
                    "pipeline_state_write_failed",
                    "the pipeline state could not be persisted",
                )));
            }
            runtime.write_log(
                LogLevel::Info,
                "pipeline",
                &format!(
                    "range requested: {} -> {}",
                    request.from_stage_id, request.to_stage_id
                ),
            );
            let mut executor = runtime
                .pipeline_executor
                .clone()
                .with_artifact_locale(request.artifact_locale)
                .with_work_unit_stop_token(WorkUnitStopToken::from_shared(Arc::clone(&stop)));
            if range_stage_ids.iter().any(|stage_id| stage_id == "13") {
                match development_work_root(&runtime, &project_settings) {
                    Ok(project_path) => {
                        executor = executor.with_unity_project_path(project_path);
                    }
                    Err(error) => runtime.write_log(
                        LogLevel::Warning,
                        "pipeline.unity_context",
                        &format!("Step13 Unity project context is unavailable: {error}"),
                    ),
                }
                match development_editor_path(&runtime, &project_settings) {
                    Ok(editor_path) => {
                        executor = executor.with_unity_editor_path(editor_path);
                    }
                    Err(error) => runtime.write_log(
                        LogLevel::Warning,
                        "pipeline.unity_context",
                        &format!("Step13 Unity editor context is unavailable: {error}"),
                    ),
                }
            }
            if range_stage_ids.iter().any(|stage_id| stage_id == "07") {
                match style_image_generator_from_config(&ai_config_snapshot, &runtime.data_root) {
                    Ok(generator) => {
                        executor = executor.with_style_image_generator(generator);
                    }
                    Err(_) => {
                        runtime.write_log(
                            LogLevel::Warning,
                            "pipeline.image",
                            "active image configuration is unavailable; Step07 will use an explicit fallback",
                        );
                    }
                }
            }
            if range_stage_ids
                .iter()
                .any(|stage_id| matches!(stage_id.as_str(), "11" | "12"))
            {
                let work_executor =
                    development_work_root(&runtime, &project_settings).and_then(|work_root| {
                        let editor_path = development_editor_path(&runtime, &project_settings).ok();
                        work_unit_executor_from_config(
                            &ai_config_snapshot,
                            &work_root,
                            editor_path.as_deref(),
                            runtime.save.draft_root(),
                            &execution_object_owner_id,
                        )
                    });
                match work_executor {
                    Ok(work_executor) => {
                        executor = executor.with_work_unit_executor(work_executor);
                    }
                    Err(_) => {
                        runtime.write_log(
                            LogLevel::Warning,
                            "pipeline.work_units",
                            "a required work-unit provider is unavailable; Step11/12 will remain blocked",
                        );
                    }
                }
            }
            (
                runtime.pipeline.clone(),
                executor,
                runtime.pipeline_state.clone(),
                runtime.project_state.clone(),
                checkpoint_observer,
            )
        }
        Err(error) => {
            return Ok(command_failure(command_error(
                "runtime_state_unavailable",
                error.to_string(),
            )));
        }
    };

    let accepted_view = match pipeline::load_pipeline_view(&service, &pipeline_state) {
        response if response.ok => response.data.expect("successful pipeline view response"),
        response => {
            if let Ok(mut runtime) = state.lock() {
                mark_pipeline_failed(
                    &mut runtime,
                    "pipeline view creation failed after checkpoint initialization",
                );
            }
            return Ok(command_failure(response.error.unwrap_or_else(|| {
                command_error(
                    "pipeline_view_failed",
                    "failed to create the accepted pipeline view",
                )
            })));
        }
    };
    let accepted = command_success(PipelineCommandView {
        view: accepted_view,
        report: Some(PipelineRunReportView {
            ordered_stage_ids: canonical_order,
            executed_stage_ids: Vec::new(),
            final_status: "accepted".to_string(),
            from_stage_id: request.from_stage_id.clone(),
            to_stage_id: request.to_stage_id.clone(),
        }),
    });
    let stop_aware = StopAwareExecutor {
        inner: executor,
        stop: Arc::clone(&stop),
    };
    drop(state);
    let worker_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let task = tauri::async_runtime::spawn_blocking(move || {
            if stop.load(Ordering::SeqCst) {
                pipeline_state.stop_requested = true;
            } else if stop_aware
                .inner
                .prepare_project_source(&project_state)
                .is_err()
            {
                return PipelineWorkerOutcome::PreparationFailed;
            }
            let response = pipeline::run_pipeline_range_with_observer(
                &service,
                &mut pipeline_state,
                request,
                &stop_aware,
                &checkpoint_observer,
            );
            PipelineWorkerOutcome::Completed(response, pipeline_state)
        })
        .await;

        let state = worker_app.state::<AppRuntime>();
        match task {
            Ok(PipelineWorkerOutcome::Completed(response, mut completed_state)) => {
                if let Ok(mut runtime) = state.lock() {
                    if runtime.pipeline_state.attempt_id != completed_state.attempt_id {
                        runtime.write_log(
                            LogLevel::Warning,
                            "pipeline",
                            "ignored a stale pipeline attempt result",
                        );
                        drop(run_guard);
                        return;
                    }
                    let stop_requested = runtime.pipeline_state.stop_requested
                        || state.pipeline_stop_flag().load(Ordering::SeqCst);
                    completed_state.stop_requested |= stop_requested;
                    settle_completed_worker_state(&mut runtime, &mut completed_state, response.ok);
                    completed_state.state_version = completed_state
                        .state_version
                        .max(runtime.pipeline_state.state_version)
                        .saturating_add(1);
                    runtime.pipeline_state = completed_state;
                    if let Err(error) = runtime.persist_pipeline_state() {
                        let _ = error;
                        runtime.write_log(
                            LogLevel::Error,
                            "pipeline",
                            "completed pipeline state persistence failed",
                        );
                    }
                    record_pipeline_result(&mut runtime, &response);
                }
            }
            Ok(PipelineWorkerOutcome::PreparationFailed) => {
                if let Ok(mut runtime) = state.lock() {
                    mark_pipeline_failed(&mut runtime, "pipeline source preparation failed");
                }
            }
            Err(_) => {
                if let Ok(mut runtime) = state.lock() {
                    mark_pipeline_failed(&mut runtime, "pipeline worker failed unexpectedly");
                }
            }
        }
        drop(run_guard);
    });
    Ok(accepted)
}

fn settle_completed_worker_state(
    runtime: &mut crate::runtime::RuntimeState,
    state: &mut adm_new_contracts::pipeline::PipelineRunState,
    response_ok: bool,
) {
    if !response_ok && finalize_checkpoint_failed_for_run(runtime, &state.run_id).is_err() {
        runtime.write_log(
            LogLevel::Error,
            "pipeline.recovery",
            "failed to finalize the checkpoint after a pipeline observer error",
        );
    }
    let projected = synchronize_completed_state_with_checkpoint(runtime, state);
    if !response_ok && !projected {
        state.status = "failed".to_string();
        state.stop_requested = false;
        state.recovery = None;
    }
}

fn synchronize_completed_state_with_checkpoint(
    runtime: &mut crate::runtime::RuntimeState,
    state: &mut adm_new_contracts::pipeline::PipelineRunState,
) -> bool {
    if state.run_id.trim().is_empty() {
        return false;
    }
    let repository =
        PipelineCheckpointRepository::new(&runtime.runtime_config.paths().checkpoints_dir);
    let checkpoint = match repository.load_current(&state.run_id) {
        Ok(Some(checkpoint)) => checkpoint,
        Ok(None) => {
            if matches!(
                state.status.as_str(),
                "stopped" | "stop_requested" | "stopping"
            ) {
                state.status = "recovery_blocked".to_string();
                state.recovery = None;
            }
            return false;
        }
        Err(_) => {
            runtime.write_log(
                LogLevel::Error,
                "pipeline.recovery",
                "completed pipeline checkpoint could not be loaded",
            );
            if matches!(
                state.status.as_str(),
                "stopped" | "stop_requested" | "stopping"
            ) {
                state.status = "recovery_blocked".to_string();
                state.recovery = None;
            }
            return false;
        }
    };
    state.current_stage_id = checkpoint.current_stage_id.clone();
    state.current_unit_id = checkpoint.current_unit_id.clone();
    match checkpoint.status {
        PipelineCheckpointStatus::Recoverable
        | PipelineCheckpointStatus::Stopped
        | PipelineCheckpointStatus::StopRequested
        | PipelineCheckpointStatus::Stopping => {
            state.status = "recoverable".to_string();
            state.stop_requested = false;
            state.recovery = Some(PipelineRecoverySummary::from(&checkpoint));
        }
        PipelineCheckpointStatus::RecoveryBlocked => {
            state.status = "recovery_blocked".to_string();
            state.stop_requested = false;
            state.recovery = Some(PipelineRecoverySummary::from(&checkpoint));
        }
        PipelineCheckpointStatus::WaitingConfirmation => {
            state.status = "waiting_confirmation".to_string();
            state.stop_requested = false;
            state.recovery = None;
        }
        PipelineCheckpointStatus::Completed => {
            state.status = "success".to_string();
            state.stop_requested = false;
            state.recovery = None;
        }
        PipelineCheckpointStatus::Failed => {
            state.status = "failed".to_string();
            state.stop_requested = false;
            state.recovery = None;
        }
        PipelineCheckpointStatus::Running | PipelineCheckpointStatus::Resuming => return false,
    }
    true
}

#[tauri::command]
pub async fn resume_pipeline(
    app: AppHandle,
    request: ResumePipelineRequest,
) -> Result<CommandAdapterResult<PipelineCommandView>, String> {
    let state = app.state::<AppRuntime>();
    let (checkpoints_dir, active_run_id, active_status) = match state.lock() {
        Ok(runtime) => (
            runtime.runtime_config.paths().checkpoints_dir.clone(),
            runtime.pipeline_state.run_id.clone(),
            runtime.pipeline_state.status.clone(),
        ),
        Err(error) => {
            return Ok(command_failure(command_error(
                "runtime_state_unavailable",
                error.to_string(),
            )));
        }
    };
    drop(state);
    if request.run_id != active_run_id
        || !matches!(active_status.as_str(), "recoverable" | "recovery_blocked")
    {
        return Ok(command_failure(command_error(
            "pipeline_resume_not_allowed",
            "the requested run is not the active recoverable pipeline",
        )));
    }
    let repository = PipelineCheckpointRepository::new(checkpoints_dir);
    let mut checkpoint = match repository.load_current(&request.run_id) {
        Ok(Some(checkpoint)) => checkpoint,
        Ok(None) => {
            return Ok(command_failure(command_error(
                "pipeline_checkpoint_missing",
                "the recoverable pipeline checkpoint is missing",
            )));
        }
        Err(_) => {
            return Ok(command_failure(command_error(
                "pipeline_checkpoint_invalid",
                "the recoverable pipeline checkpoint is invalid",
            )));
        }
    };
    if checkpoint.revision != request.expected_revision {
        return Ok(command_failure(command_error(
            "pipeline_checkpoint_revision_conflict",
            "the recoverable checkpoint changed; reload the pipeline view before resuming",
        )));
    }
    let explicitly_recoverable = checkpoint.status == PipelineCheckpointStatus::Recoverable
        && checkpoint.resume_policy == PipelineResumePolicy::ExplicitOnly;
    let requires_reconciliation = checkpoint.status == PipelineCheckpointStatus::RecoveryBlocked;
    if !explicitly_recoverable && !requires_reconciliation {
        return Ok(command_failure(command_error(
            "pipeline_resume_not_allowed",
            "the checkpoint is not explicitly recoverable",
        )));
    }
    let reconcile_stages = checkpoint
        .units
        .iter()
        .filter(|unit| {
            unit.reconcile_required
                || matches!(
                    unit.status,
                    PipelineUnitStatus::Running | PipelineUnitStatus::Unknown
                )
        })
        .map(|unit| unit.stage_id.clone())
        .filter(|stage_id| matches!(stage_id.as_str(), "07" | "11" | "12"))
        .collect::<std::collections::BTreeSet<_>>();
    if requires_reconciliation && reconcile_stages.is_empty() {
        return Ok(command_failure(command_error(
            "pipeline_resume_recovery_blocked",
            "the blocked checkpoint has no safely reconcilable execution unit",
        )));
    }
    if !reconcile_stages.is_empty() {
        let state = app.state::<AppRuntime>();
        let reconciliation = match state.lock() {
            Ok(runtime) => (|| {
                let mut executor = runtime.pipeline_executor.clone();
                if reconcile_stages
                    .iter()
                    .any(|stage_id| matches!(stage_id.as_str(), "11" | "12"))
                {
                    let settings = runtime.runtime_config.load_project_settings(false);
                    let work_root = development_work_root(&runtime, &settings)?;
                    let editor_path = development_editor_path(&runtime, &settings).ok();
                    let config = runtime.ai_config.load_or_default()?;
                    let owner_id = work_unit_execution_object_owner_id(
                        &runtime.runtime_config.paths().session_id,
                        checkpoint.identity.save_id.as_deref(),
                    );
                    executor = executor.with_work_unit_executor(work_unit_executor_from_config(
                        &config,
                        &work_root,
                        editor_path.as_deref(),
                        runtime.save.draft_root(),
                        &owner_id,
                    )?);
                }
                for stage_id in &reconcile_stages {
                    executor.reconcile_checkpoint_work_units(&mut checkpoint, stage_id)?;
                }
                Ok(())
            })(),
            Err(error) => Err(adm_new_foundation::AdmError::new(error.to_string())),
        };
        drop(state);
        if reconciliation.is_err() {
            return Ok(command_failure(command_error(
                "pipeline_resume_reconcile_failed",
                "the interrupted execution unit could not be reconciled safely",
            )));
        }
        checkpoint.revision = checkpoint.revision.saturating_add(1);
        checkpoint.updated_at = format!("unix:{}", adm_new_foundation::unix_timestamp());
        checkpoint.next_unit_id = checkpoint
            .units
            .iter()
            .find(|unit| !unit.status.is_committed())
            .map(|unit| unit.unit_id.clone());
        if checkpoint.status != PipelineCheckpointStatus::RecoveryBlocked {
            checkpoint.status = PipelineCheckpointStatus::Recoverable;
            checkpoint.resume_policy = PipelineResumePolicy::ExplicitOnly;
            checkpoint.recovery_blocked_reason.clear();
        }
        if repository
            .compare_and_swap(request.expected_revision, &checkpoint)
            .is_err()
        {
            return Ok(command_failure(command_error(
                "pipeline_checkpoint_revision_conflict",
                "the recoverable checkpoint changed during reconciliation; reload before resuming",
            )));
        }
        if checkpoint.status == PipelineCheckpointStatus::RecoveryBlocked {
            let state = app.state::<AppRuntime>();
            if let Ok(mut runtime) = state.lock() {
                runtime.pipeline_state.status = "recovery_blocked".to_string();
                runtime.pipeline_state.recovery = Some(PipelineRecoverySummary::from(&checkpoint));
                runtime.pipeline_state.state_version =
                    runtime.pipeline_state.state_version.saturating_add(1);
                let _ = runtime.persist_pipeline_state();
            }
            return Ok(command_failure(command_error(
                "pipeline_resume_recovery_blocked",
                if checkpoint.recovery_blocked_reason.is_empty() {
                    "the work-unit side effect cannot be reconciled safely".to_string()
                } else {
                    checkpoint.recovery_blocked_reason.clone()
                },
            )));
        }
    }
    if checkpoint.units.iter().any(|unit| {
        unit.reconcile_required
            || matches!(
                unit.status,
                PipelineUnitStatus::Running | PipelineUnitStatus::Unknown
            )
    }) {
        return Ok(command_failure(command_error(
            "pipeline_resume_reconcile_required",
            "the checkpoint contains an execution unit that cannot be safely reconciled",
        )));
    }
    let Some(next_stage_id) = checkpoint
        .next_unit_id
        .as_deref()
        .and_then(|unit_id| unit_id.strip_suffix(":stage"))
        .map(str::to_string)
    else {
        return Ok(command_failure(command_error(
            "pipeline_resume_unit_missing",
            "the checkpoint has no next whole-stage execution unit",
        )));
    };
    let run_request = RunPipelineRangeRequest {
        from_stage_id: next_stage_id,
        to_stage_id: checkpoint.range.to_stage_id.clone(),
        skip_manual_gates: checkpoint.skip_manual_gates,
        artifact_locale: checkpoint.identity.artifact_locale,
    };
    start_pipeline_range(app, run_request, Some(checkpoint)).await
}

#[tauri::command]
pub fn stop_pipeline(state: State<'_, AppRuntime>) -> CommandAdapterResult<PipelineView> {
    state.request_pipeline_stop();
    with_runtime(&state, |runtime| {
        let response = pipeline::stop_pipeline(&runtime.pipeline, &mut runtime.pipeline_state);
        if response.ok {
            if let Err(error) = mark_checkpoint_stop_requested(runtime) {
                let _ = error;
                runtime.write_log(
                    LogLevel::Error,
                    "pipeline.stop",
                    "the stop checkpoint could not be updated",
                );
                return command_failure(command_error(
                    "pipeline_checkpoint_stop_failed",
                    "the stop signal was accepted but its recovery checkpoint could not be updated",
                ));
            }
            if let Err(error) = runtime.persist_pipeline_state() {
                let _ = error;
                return command_failure(command_error(
                    "pipeline_state_write_failed",
                    "the stop state could not be persisted",
                ));
            }
            runtime.write_log(LogLevel::Warning, "pipeline", "stop requested by operator");
        }
        response
    })
}

fn mark_checkpoint_stop_requested(
    runtime: &mut crate::runtime::RuntimeState,
) -> adm_new_foundation::AdmResult<()> {
    let run_id = runtime.pipeline_state.run_id.trim();
    if run_id.is_empty() {
        return Ok(());
    }
    let repository =
        PipelineCheckpointRepository::new(&runtime.runtime_config.paths().checkpoints_dir);
    for _ in 0..4 {
        let Some(current) = repository.load_current(run_id)? else {
            return Ok(());
        };
        if matches!(
            current.status,
            PipelineCheckpointStatus::Completed
                | PipelineCheckpointStatus::Failed
                | PipelineCheckpointStatus::RecoveryBlocked
                | PipelineCheckpointStatus::WaitingConfirmation
                | PipelineCheckpointStatus::Recoverable
        ) {
            return Ok(());
        }
        let mut next = current.clone();
        next.revision = current.revision.saturating_add(1);
        next.status = PipelineCheckpointStatus::StopRequested;
        next.resume_policy = PipelineResumePolicy::Disabled;
        next.stop_reason = "operator_request".to_string();
        next.stop_boundary = current
            .current_unit_id
            .clone()
            .or_else(|| {
                current
                    .current_stage_id
                    .as_ref()
                    .map(|stage| format!("{stage}:stage"))
            })
            .unwrap_or_else(|| "next_safe_boundary".to_string());
        next.updated_at = format!("unix:{}", adm_new_foundation::unix_timestamp());
        match repository.compare_and_swap(current.revision, &next) {
            Ok(()) => return Ok(()),
            Err(error) => {
                let latest = repository.load_current(run_id)?;
                if latest.as_ref().map(|item| item.revision) == Some(current.revision) {
                    return Err(error);
                }
            }
        }
    }
    Err(adm_new_foundation::AdmError::new(
        "pipeline checkpoint changed repeatedly while requesting stop",
    ))
}

#[tauri::command]
pub fn confirm_style(
    state: State<'_, AppRuntime>,
    request: ConfirmStyleRequest,
) -> CommandAdapterResult<PipelineView> {
    if state.pipeline_is_running() {
        return command_failure(command_error(
            "pipeline_already_running",
            "wait for the current pipeline range to stop before confirming style",
        ));
    }
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return command_failure(command_error(
                "pipeline_already_running",
                "wait for the current pipeline range to stop before confirming style",
            ));
        }
        if let Err(error) = runtime.invalidate_package_result() {
            let _ = error;
            return command_failure(command_error(
                "package_result_invalidation_failed",
                "the previous package result could not be invalidated",
            ));
        }
        let style_id = selected_style_id(&request);
        if style_id.is_empty() {
            return command_failure(command_error(
                "style_selection_missing",
                "select one Step07 style before confirming",
            ));
        }
        let repository =
            PipelineCheckpointRepository::new(&runtime.runtime_config.paths().checkpoints_dir);
        let checkpoint = match repository.load_current(&runtime.pipeline_state.run_id) {
            Ok(Some(checkpoint))
                if checkpoint.status == PipelineCheckpointStatus::WaitingConfirmation
                    && checkpoint.current_stage_id.as_deref() == Some("07") =>
            {
                checkpoint
            }
            _ => {
                return command_failure(command_error(
                    "style_confirmation_checkpoint_missing",
                    "the active Step07 waiting-confirmation checkpoint is unavailable",
                ));
            }
        };
        let artifact_locale = checkpoint.identity.artifact_locale;
        let executor = runtime
            .pipeline_executor
            .clone()
            .with_artifact_locale(artifact_locale);
        if let Err(error) = executor.confirm_style(&style_id, &request.notes) {
            let _ = error;
            return command_failure(command_error(
                "style_confirmation_failed",
                "the Step07 style confirmation could not be persisted",
            ));
        }
        let rerun = pipeline::run_pipeline_range(
            &runtime.pipeline,
            &mut runtime.pipeline_state,
            RunPipelineRangeRequest {
                from_stage_id: "07".to_string(),
                to_stage_id: "07".to_string(),
                skip_manual_gates: false,
                artifact_locale,
            },
            &executor,
        );
        if rerun.ok {
            let Some(result) = runtime
                .pipeline_state
                .stages
                .get("07")
                .and_then(|stage| stage.result.as_ref())
            else {
                return command_failure(command_error(
                    "style_confirmation_result_missing",
                    "the Step07 confirmation rerun produced no stage result",
                ));
            };
            let next_checkpoint = match advance_style_confirmation_checkpoint(&checkpoint, result) {
                Ok(checkpoint) => checkpoint,
                Err(error) => {
                    return command_failure(command_error(
                        "style_confirmation_checkpoint_failed",
                        error.to_string(),
                    ));
                }
            };
            if repository
                .compare_and_swap(checkpoint.revision, &next_checkpoint)
                .is_err()
            {
                return command_failure(command_error(
                    "style_confirmation_checkpoint_conflict",
                    "the Step07 checkpoint changed while confirmation was being committed",
                ));
            }
            runtime.pipeline_state.current_stage_id = next_checkpoint.current_stage_id.clone();
            runtime.pipeline_state.current_unit_id = next_checkpoint.current_unit_id.clone();
            runtime.pipeline_state.stop_requested = false;
            if next_checkpoint.status == PipelineCheckpointStatus::Completed {
                runtime.pipeline_state.status = "success".to_string();
                runtime.pipeline_state.recovery = None;
            } else {
                runtime.pipeline_state.status = "recoverable".to_string();
                runtime.pipeline_state.recovery =
                    Some(PipelineRecoverySummary::from(&next_checkpoint));
            }
            runtime.pipeline_state.state_version =
                runtime.pipeline_state.state_version.saturating_add(1);
            if let Err(error) = runtime.persist_pipeline_state() {
                let _ = error;
                return command_failure(command_error(
                    "pipeline_state_write_failed",
                    "the Step07 confirmation state could not be persisted",
                ));
            }
            runtime.write_log(
                LogLevel::Info,
                "pipeline",
                &format!("Step07 style confirmed: {style_id}"),
            );
            return pipeline::load_pipeline_view(&runtime.pipeline, &runtime.pipeline_state);
        }
        command_failure(rerun.error.unwrap_or_else(|| {
            command_error(
                "style_confirmation_failed",
                "Step07 confirmation rerun failed",
            )
        }))
    })
}

fn advance_style_confirmation_checkpoint(
    current: &PipelineCheckpoint,
    result: &PipelineStageResult,
) -> adm_new_foundation::AdmResult<PipelineCheckpoint> {
    if current.status != PipelineCheckpointStatus::WaitingConfirmation
        || current.current_stage_id.as_deref() != Some("07")
        || result.status != StageStatus::Success
    {
        return Err(adm_new_foundation::AdmError::new(
            "Step07 checkpoint or confirmation result is not committable",
        ));
    }
    let mut next = current.clone();
    next.revision = current.revision.saturating_add(1);
    next.updated_at = format!("unix:{}", adm_new_foundation::unix_timestamp());
    next.stop_reason.clear();
    next.stop_boundary.clear();
    next.recovery_blocked_reason.clear();
    let result_fingerprint = sha256_hex(&serde_json::to_vec(result).map_err(|error| {
        adm_new_foundation::AdmError::new(format!(
            "failed to fingerprint the Step07 confirmation result: {error}"
        ))
    })?);
    let unit = next
        .units
        .iter_mut()
        .find(|unit| unit.stage_id == "07")
        .ok_or_else(|| adm_new_foundation::AdmError::new("Step07 checkpoint unit is missing"))?;
    unit.status = PipelineUnitStatus::Committed;
    unit.completed_at = next.updated_at.clone();
    unit.result_fingerprint = result_fingerprint;
    unit.output_refs = result.outputs.keys().cloned().collect();
    unit.failure_message.clear();
    unit.reconcile_required = false;

    if let Some(pending) = next
        .units
        .iter()
        .find(|unit| unit.status != PipelineUnitStatus::Committed)
    {
        next.status = PipelineCheckpointStatus::Recoverable;
        next.resume_policy = PipelineResumePolicy::ExplicitOnly;
        next.current_stage_id = Some(pending.stage_id.clone());
        next.current_unit_id = None;
        next.next_unit_id = Some(pending.unit_id.clone());
    } else {
        next.status = PipelineCheckpointStatus::Completed;
        next.resume_policy = PipelineResumePolicy::Disabled;
        next.current_stage_id = None;
        next.current_unit_id = None;
        next.next_unit_id = None;
    }
    Ok(next)
}

#[tauri::command]
pub fn read_pipeline_artifact(
    state: State<'_, AppRuntime>,
    request: ReadPipelineArtifactRequest,
) -> CommandAdapterResult<ArtifactContentView> {
    with_runtime(&state, |runtime| {
        pipeline::read_pipeline_artifact(runtime.pipeline_executor.artifact_root(), request)
    })
}

fn selected_style_id(request: &ConfirmStyleRequest) -> String {
    let explicit = request.selected_style_id.trim();
    if !explicit.is_empty() {
        return explicit.to_string();
    }
    request
        .message
        .split(';')
        .map(str::trim)
        .find_map(|part| part.strip_prefix("style=").map(str::trim))
        .unwrap_or_default()
        .to_string()
}

fn work_unit_execution_object_owner_id(draft_id: &str, save_id: Option<&str>) -> String {
    let save_id = save_id.unwrap_or_default().trim();
    if save_id.is_empty() {
        let draft_id = draft_id.trim();
        format!(
            "draft-owner:{}",
            if draft_id.is_empty() {
                "detached"
            } else {
                draft_id
            }
        )
    } else {
        save_id.to_string()
    }
}

fn development_work_root(
    runtime: &crate::runtime::RuntimeState,
    settings: &adm_new_application::runtime::ProjectRuntimeSettings,
) -> adm_new_foundation::AdmResult<std::path::PathBuf> {
    let configured = settings.development_path.trim();
    if configured.is_empty() {
        return Err(adm_new_foundation::AdmError::new(
            "the current save has no bound development project",
        ));
    }
    if !settings.project_engine.eq_ignore_ascii_case("unity") {
        return Err(adm_new_foundation::AdmError::new(
            "Step11/12 currently require a Unity project binding",
        ));
    }
    let resolved =
        resolve_configured_path(configured, &runtime.runtime_config.paths().project_root);
    let canonical = std::fs::canonicalize(resolved).map_err(|_| {
        adm_new_foundation::AdmError::new("the bound development project is unavailable")
    })?;
    if !canonical.is_dir() {
        return Err(adm_new_foundation::AdmError::new(
            "the bound development project is unavailable",
        ));
    }
    let markers = adm_new_application::runtime::unity_project_markers(&canonical);
    if !markers.assets_dir || !markers.project_settings_dir || !markers.packages_manifest {
        return Err(adm_new_foundation::AdmError::new(
            "the bound development path is not a complete Unity project",
        ));
    }
    Ok(canonical)
}

fn development_editor_path(
    runtime: &crate::runtime::RuntimeState,
    settings: &adm_new_application::runtime::ProjectRuntimeSettings,
) -> adm_new_foundation::AdmResult<std::path::PathBuf> {
    let configured = settings.editor_path.trim();
    if configured.is_empty() {
        return Err(adm_new_foundation::AdmError::new(
            "the current save has no bound Unity editor",
        ));
    }
    let resolved =
        resolve_configured_path(configured, &runtime.runtime_config.paths().project_root);
    let canonical = std::fs::canonicalize(resolved)
        .map_err(|_| adm_new_foundation::AdmError::new("the bound Unity editor is unavailable"))?;
    if !adm_new_application::project_environment::unity_editor_file_is_valid(&canonical) {
        return Err(adm_new_foundation::AdmError::new(
            "the bound Unity editor is not a compatible Unity executable",
        ));
    }
    Ok(canonical)
}

fn ai_configuration_fingerprint_material(
    config: &adm_new_contracts::ai::AiConfig,
) -> serde_json::Value {
    fn category(value: &adm_new_contracts::ai::ApiCategory) -> serde_json::Value {
        let entry = value
            .entries
            .iter()
            .find(|entry| entry.id == value.active_entry_id);
        let Some(entry) = entry else {
            return serde_json::json!({
                "active_entry_id": value.active_entry_id,
                "missing": true,
            });
        };
        let extra = entry.extra_json.as_object();
        let selected_extra = [
            "model",
            "image_model",
            "response_model",
            "provider",
            "mode",
            "endpoint",
            "auth_mode",
            "api_key_env",
            "cli_path",
            "codex_home",
            "timeout",
            "timeout_seconds",
            "temperature",
            "reasoning_effort",
        ]
        .into_iter()
        .filter_map(|key| {
            extra
                .and_then(|object| object.get(key))
                .map(|value| (key.to_string(), value.clone()))
        })
        .collect::<serde_json::Map<_, _>>();
        serde_json::json!({
            "active_entry_id": entry.id,
            "config_type": entry.config_type,
            "api_origin": if entry.api_url.trim().is_empty() {
                String::new()
            } else {
                adm_new_ai::resolution::mask_api_url(&entry.api_url)
            },
            "has_api_key": !entry.api_key.trim().is_empty(),
            "codex_toml_path": entry.codex_toml_path,
            "codex_json_path": entry.codex_json_path,
            "extra": selected_extra,
        })
    }

    serde_json::json!({
        "schema_version": config.schema_version,
        "dev": category(&config.dev),
        "image": category(&config.image),
        "completion": category(&config.completion),
    })
}

fn resume_fingerprints_match(
    previous: &PipelineCheckpoint,
    requested_locale: adm_new_contracts::ArtifactLocale,
    current: &PipelineFingerprints,
    legacy_zh: &PipelineFingerprints,
) -> bool {
    previous.fingerprints == *current
        || (requested_locale == adm_new_contracts::ArtifactLocale::ZhCn
            && previous.identity.artifact_locale == adm_new_contracts::ArtifactLocale::ZhCn
            && previous.fingerprints == *legacy_zh)
}

fn record_pipeline_result(
    runtime: &mut crate::runtime::RuntimeState,
    response: &CommandAdapterResult<PipelineCommandView>,
) {
    if let Some(data) = response.data.as_ref() {
        runtime.write_log(
            if data.view.state.status == "success" {
                LogLevel::Info
            } else {
                LogLevel::Warning
            },
            "pipeline",
            &format!("pipeline range finished: {}", data.view.state.status),
        );
        for stage in data
            .view
            .stages
            .iter()
            .filter(|stage| !stage.errors.is_empty())
        {
            runtime.write_log(
                LogLevel::Error,
                "pipeline",
                &format!("Step{}: {}", stage.stage_id, stage.errors.join("; ")),
            );
        }
    } else if let Some(error) = response.error.as_ref() {
        runtime.write_log(LogLevel::Error, "pipeline", &error.message);
    }
}

fn mark_pipeline_failed(runtime: &mut crate::runtime::RuntimeState, message: &str) {
    if finalize_active_checkpoint_failed(runtime).is_err() {
        runtime.write_log(
            LogLevel::Error,
            "pipeline.recovery",
            "failed to finalize the active pipeline checkpoint",
        );
    }
    runtime.pipeline_state.status = "failed".to_string();
    runtime.pipeline_state.stop_requested = false;
    runtime.write_log(LogLevel::Error, "pipeline", message);
    if let Err(error) = runtime.persist_pipeline_state() {
        let _ = error;
        runtime.write_log(
            LogLevel::Error,
            "pipeline",
            "failed to persist pipeline failure state",
        );
    }
}

fn finalize_active_checkpoint_failed(
    runtime: &crate::runtime::RuntimeState,
) -> adm_new_foundation::AdmResult<()> {
    finalize_checkpoint_failed_for_run(runtime, &runtime.pipeline_state.run_id)
}

fn finalize_checkpoint_failed_for_run(
    runtime: &crate::runtime::RuntimeState,
    run_id: &str,
) -> adm_new_foundation::AdmResult<()> {
    let run_id = run_id.trim();
    if run_id.is_empty() {
        return Ok(());
    }
    let repository =
        PipelineCheckpointRepository::new(&runtime.runtime_config.paths().checkpoints_dir);
    for _ in 0..4 {
        let Some(current) = repository.load_current(run_id)? else {
            return Ok(());
        };
        if matches!(
            current.status,
            PipelineCheckpointStatus::Completed
                | PipelineCheckpointStatus::Failed
                | PipelineCheckpointStatus::WaitingConfirmation
                | PipelineCheckpointStatus::RecoveryBlocked
        ) {
            return Ok(());
        }
        let mut next = current.clone();
        next.revision = current.revision.saturating_add(1);
        next.status = PipelineCheckpointStatus::Failed;
        next.resume_policy = PipelineResumePolicy::Disabled;
        next.updated_at = format!("unix:{}", adm_new_foundation::unix_timestamp());
        match repository.compare_and_swap(current.revision, &next) {
            Ok(()) => return Ok(()),
            Err(error) => {
                let latest = repository.load_current(run_id)?;
                if latest.as_ref().map(|item| item.revision) == Some(current.revision) {
                    return Err(error);
                }
            }
        }
    }
    Err(adm_new_foundation::AdmError::new(
        "pipeline checkpoint changed repeatedly while recording failure",
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn work_unit_execution_object_owner_is_stable_for_detached_and_saved_drafts() {
        assert_eq!(
            work_unit_execution_object_owner_id("desktop-session", None),
            "draft-owner:desktop-session"
        );
        assert_eq!(
            work_unit_execution_object_owner_id("desktop-session", Some("save_123")),
            "save_123"
        );
        assert_eq!(
            work_unit_execution_object_owner_id("desktop-session", Some(" save_123 ")),
            "save_123"
        );
    }

    #[test]
    fn pipeline_failure_cleanup_persists_a_terminal_state() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-pipeline-failure-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let run_id = "run_preparation_failure";
            let checkpoint = initial_whole_stage_checkpoint(
                PipelineRunIdentity {
                    run_id: run_id.to_string(),
                    attempt_id: "attempt_preparation_failure".to_string(),
                    project_id: "project".to_string(),
                    draft_id: runtime.runtime_config.paths().session_id.clone(),
                    ..PipelineRunIdentity::default()
                },
                CanonicalPipelineRange {
                    from_stage_id: "00".to_string(),
                    to_stage_id: "00".to_string(),
                    stage_ids: vec!["00".to_string()],
                },
                PipelineFingerprints::default(),
            );
            let repository =
                PipelineCheckpointRepository::new(&runtime.runtime_config.paths().checkpoints_dir);
            repository.compare_and_swap(0, &checkpoint).unwrap();
            runtime.pipeline_state.run_id = run_id.to_string();
            runtime.pipeline_state.status = "running".to_string();
            runtime.pipeline_state.stop_requested = true;
            mark_pipeline_failed(&mut runtime, "source preparation failed");
            assert_eq!(runtime.pipeline_state.status, "failed");
            assert!(!runtime.pipeline_state.stop_requested);
            let persisted = fs::read_to_string(runtime.pipeline_state_file()).unwrap();
            assert_eq!(
                serde_json::from_str::<adm_new_contracts::pipeline::PipelineRunState>(&persisted)
                    .unwrap()
                    .status,
                "failed"
            );
            let finalized = repository.load_current(run_id).unwrap().unwrap();
            assert_eq!(finalized.status, PipelineCheckpointStatus::Failed);
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn initialized_checkpoint_cleanup_does_not_depend_on_runtime_projection() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-pipeline-early-failure-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let runtime = app.lock().unwrap();
            let run_id = "run_before_runtime_projection";
            let checkpoint = initial_whole_stage_checkpoint(
                PipelineRunIdentity {
                    run_id: run_id.to_string(),
                    attempt_id: "attempt_before_runtime_projection".to_string(),
                    project_id: "project".to_string(),
                    draft_id: runtime.runtime_config.paths().session_id.clone(),
                    ..PipelineRunIdentity::default()
                },
                CanonicalPipelineRange {
                    from_stage_id: "00".to_string(),
                    to_stage_id: "00".to_string(),
                    stage_ids: vec!["00".to_string()],
                },
                PipelineFingerprints::default(),
            );
            let repository =
                PipelineCheckpointRepository::new(&runtime.runtime_config.paths().checkpoints_dir);
            repository.compare_and_swap(0, &checkpoint).unwrap();

            assert!(runtime.pipeline_state.run_id.is_empty());
            finalize_checkpoint_failed_for_run(&runtime, run_id).unwrap();
            assert_eq!(
                repository.load_current(run_id).unwrap().unwrap().status,
                PipelineCheckpointStatus::Failed
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stop_request_is_persisted_with_reason_boundary_and_revision() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-pipeline-stop-checkpoint-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let run_id = "run_stop_test";
            let checkpoint = initial_whole_stage_checkpoint(
                PipelineRunIdentity {
                    run_id: run_id.to_string(),
                    attempt_id: "attempt_stop_1".to_string(),
                    project_id: "project".to_string(),
                    draft_id: runtime.runtime_config.paths().session_id.clone(),
                    ..PipelineRunIdentity::default()
                },
                CanonicalPipelineRange {
                    from_stage_id: "11".to_string(),
                    to_stage_id: "11".to_string(),
                    stage_ids: vec!["11".to_string()],
                },
                PipelineFingerprints {
                    input: "input".to_string(),
                    configuration: "configuration".to_string(),
                    execution_plan: "plan".to_string(),
                    application: "application".to_string(),
                },
            );
            let repository =
                PipelineCheckpointRepository::new(&runtime.runtime_config.paths().checkpoints_dir);
            repository.compare_and_swap(0, &checkpoint).unwrap();
            runtime.pipeline_state.run_id = run_id.to_string();
            runtime.pipeline_state.status = "running".to_string();
            runtime.pipeline_state.current_stage_id = Some("11".to_string());
            runtime.pipeline_state.current_unit_id = Some("11:stage".to_string());

            mark_checkpoint_stop_requested(&mut runtime).unwrap();
            let stopped = repository.load_current(run_id).unwrap().unwrap();
            assert_eq!(stopped.revision, 2);
            assert_eq!(stopped.status, PipelineCheckpointStatus::StopRequested);
            assert_eq!(stopped.stop_reason, "operator_request");
            assert_eq!(stopped.stop_boundary, "11:stage");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn style_confirmation_advances_waiting_checkpoint_and_preserves_frozen_locale() {
        let mut checkpoint = initial_whole_stage_checkpoint(
            PipelineRunIdentity {
                run_id: "run_style_confirmation".to_string(),
                attempt_id: "attempt_style_confirmation".to_string(),
                artifact_locale: adm_new_contracts::ArtifactLocale::EnUs,
                ..PipelineRunIdentity::default()
            },
            CanonicalPipelineRange {
                from_stage_id: "07".to_string(),
                to_stage_id: "08".to_string(),
                stage_ids: vec!["07".to_string(), "08".to_string()],
            },
            PipelineFingerprints::default(),
        );
        checkpoint.status = PipelineCheckpointStatus::WaitingConfirmation;
        checkpoint.resume_policy = PipelineResumePolicy::Disabled;
        checkpoint.current_stage_id = Some("07".to_string());
        checkpoint.next_unit_id = None;
        checkpoint.units[0].status = PipelineUnitStatus::Committed;
        let result = PipelineStageResult {
            status: StageStatus::Success,
            outputs: std::collections::BTreeMap::from([(
                "artifact_locale".to_string(),
                serde_json::json!("en-US"),
            )]),
            errors: Vec::new(),
            warnings: Vec::new(),
            message: "Step07 success".to_string(),
        };

        let advanced = advance_style_confirmation_checkpoint(&checkpoint, &result).unwrap();

        assert_eq!(advanced.identity.artifact_locale.as_str(), "en-US");
        assert_eq!(advanced.status, PipelineCheckpointStatus::Recoverable);
        assert_eq!(advanced.resume_policy, PipelineResumePolicy::ExplicitOnly);
        assert_eq!(advanced.current_stage_id.as_deref(), Some("08"));
        assert_eq!(advanced.next_unit_id.as_deref(), Some("08:stage"));
        assert_eq!(advanced.units[0].status, PipelineUnitStatus::Committed);
        assert!(!advanced.units[0].result_fingerprint.is_empty());
    }

    #[test]
    fn legacy_checkpoint_fingerprint_is_resumable_only_as_its_safe_chinese_default() {
        let current = PipelineFingerprints {
            input: "input".to_string(),
            configuration: "current-config".to_string(),
            execution_plan: "plan".to_string(),
            application: "current-app".to_string(),
        };
        let legacy = PipelineFingerprints {
            input: "input".to_string(),
            configuration: "legacy-config".to_string(),
            execution_plan: "plan".to_string(),
            application: "legacy-app".to_string(),
        };
        let mut checkpoint = initial_whole_stage_checkpoint(
            PipelineRunIdentity::default(),
            CanonicalPipelineRange {
                from_stage_id: "00".to_string(),
                to_stage_id: "00".to_string(),
                stage_ids: vec!["00".to_string()],
            },
            legacy.clone(),
        );

        assert!(resume_fingerprints_match(
            &checkpoint,
            adm_new_contracts::ArtifactLocale::ZhCn,
            &current,
            &legacy,
        ));
        assert!(!resume_fingerprints_match(
            &checkpoint,
            adm_new_contracts::ArtifactLocale::EnUs,
            &current,
            &legacy,
        ));

        checkpoint.fingerprints = current.clone();
        checkpoint.identity.artifact_locale = adm_new_contracts::ArtifactLocale::EnUs;
        assert!(resume_fingerprints_match(
            &checkpoint,
            adm_new_contracts::ArtifactLocale::EnUs,
            &current,
            &legacy,
        ));
    }

    #[test]
    fn completed_worker_projects_recoverable_checkpoint_into_runtime_state() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-pipeline-recoverable-projection-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let run_id = "run_recoverable_test";
            let mut checkpoint = initial_whole_stage_checkpoint(
                PipelineRunIdentity {
                    run_id: run_id.to_string(),
                    attempt_id: "attempt_recoverable_1".to_string(),
                    project_id: "project".to_string(),
                    draft_id: runtime.runtime_config.paths().session_id.clone(),
                    ..PipelineRunIdentity::default()
                },
                CanonicalPipelineRange {
                    from_stage_id: "11".to_string(),
                    to_stage_id: "11".to_string(),
                    stage_ids: vec!["11".to_string()],
                },
                PipelineFingerprints {
                    input: "input".to_string(),
                    configuration: "configuration".to_string(),
                    execution_plan: "plan".to_string(),
                    application: "application".to_string(),
                },
            );
            checkpoint.status = PipelineCheckpointStatus::Recoverable;
            checkpoint.resume_policy = PipelineResumePolicy::ExplicitOnly;
            let repository =
                PipelineCheckpointRepository::new(&runtime.runtime_config.paths().checkpoints_dir);
            repository.compare_and_swap(0, &checkpoint).unwrap();
            let mut completed = runtime.pipeline_state.clone();
            completed.run_id = run_id.to_string();
            completed.status = "stopped".to_string();
            completed.stop_requested = true;
            assert!(synchronize_completed_state_with_checkpoint(
                &mut runtime,
                &mut completed
            ));
            assert_eq!(completed.status, "recoverable");
            assert!(!completed.stop_requested);
            assert_eq!(completed.recovery.as_ref().unwrap().run_id, run_id);
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_worker_response_terminalizes_a_running_checkpoint() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-pipeline-observer-failure-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let run_id = "run_observer_failure";
            let mut checkpoint = initial_whole_stage_checkpoint(
                PipelineRunIdentity {
                    run_id: run_id.to_string(),
                    attempt_id: "attempt_observer_failure".to_string(),
                    project_id: "project".to_string(),
                    draft_id: runtime.runtime_config.paths().session_id.clone(),
                    ..PipelineRunIdentity::default()
                },
                CanonicalPipelineRange {
                    from_stage_id: "00".to_string(),
                    to_stage_id: "00".to_string(),
                    stage_ids: vec!["00".to_string()],
                },
                PipelineFingerprints::default(),
            );
            checkpoint.current_unit_id = Some("00:stage".to_string());
            checkpoint.units[0].status = PipelineUnitStatus::Running;
            checkpoint.units[0].reconcile_required = true;
            let repository =
                PipelineCheckpointRepository::new(&runtime.runtime_config.paths().checkpoints_dir);
            repository.compare_and_swap(0, &checkpoint).unwrap();
            let mut completed = runtime.pipeline_state.clone();
            completed.run_id = run_id.to_string();
            completed.status = "running".to_string();

            settle_completed_worker_state(&mut runtime, &mut completed, false);

            assert_eq!(completed.status, "failed");
            assert!(!completed.stop_requested);
            let terminal = repository.load_current(run_id).unwrap().unwrap();
            assert_eq!(terminal.status, PipelineCheckpointStatus::Failed);
            assert_eq!(terminal.resume_policy, PipelineResumePolicy::Disabled);
        }
        let _ = fs::remove_dir_all(root);
    }
}
