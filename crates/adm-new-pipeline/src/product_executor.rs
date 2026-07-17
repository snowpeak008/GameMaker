use std::collections::BTreeSet;
use std::collections::{BTreeMap as StdBTreeMap, BTreeSet as StdBTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use adm_new_change_kernel::{
    ChangeEvidence, ChangeFailureCategory, ChangeOutcome, CommandPurpose, EvidenceStatus,
    SideEffectState, WORKSPACE_CHANGE_SET_SCHEMA_VERSION, WorkspaceFileExpectation,
    WorkspaceFilePayload, WorkspaceOperation, WorkspaceRelativePath, WorkspaceTransactionResult,
};
use adm_new_contracts::ArtifactLocale;
use adm_new_contracts::artifact::ArtifactRegistry;
use adm_new_contracts::pipeline::{
    PipelineCheckpoint, PipelineCheckpointStatus, PipelineResumePolicy, PipelineStageResult,
    PipelineUnitStatus, StageContextModel, StageSpec, StageStatus,
};
use adm_new_contracts::project::ProjectState;
use adm_new_design::DesignEngineService;
use adm_new_design::data_loader::DesignDataLoader;
use adm_new_design::game_spec_projection::project_state_to_game_spec;
use adm_new_design::handoff::export_concept_package_from_state_with_locale;
use adm_new_foundation::io::{self, read_json, write_json};
use adm_new_foundation::paths::ProjectPaths;
use adm_new_foundation::{AdmError, AdmResult, sanitize_identifier, sha256_hex};
use adm_new_game_spec::{GameSpec, ProductEnvelope, canonicalize_game_spec, parse_game_spec};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::StageExecutor;
use crate::design_flow::design_engine_from_data;
use crate::game_spec_v2_steps::{Step00_06Compiler, StepGateStatus};
use crate::generation::{GenerationService, StageOutputGenerator};
use crate::stage_result::{failed_stage_result_with_locale, stage_result_from_generation};
use crate::stages::step11_v2::WorkspaceTaskAgent;
use crate::stages::{
    step00_02, step03_06, step07, step07_v2, step08_10_v2, step08_14, step11_v2, step12_v2,
    step13_v2, step14_v2,
};
use crate::style_image::{
    StyleImageGenerator, reconcile_unbound_style_image_record, style_image_cache_key,
};
use crate::work_units::{
    OfflineVerifiedWorkUnitExecutor, SafeUnitJournal, StageWorkUnitReconcileStatus,
    WorkUnitExecutor, WorkUnitJournalRecord, WorkUnitReconcileDecision, WorkUnitStopToken,
    reconcile_checkpoint_stage_from_journal,
};

/// Executes the real Step00-14 generators inside one draft session.
#[derive(Debug, Clone)]
pub struct ProductPipelineExecutor {
    root: PathBuf,
    session_id: String,
    artifact_root: PathBuf,
    design_data_dir: Option<PathBuf>,
    style_image_generator: Option<Arc<dyn StyleImageGenerator>>,
    vlm_review_service: Option<Arc<dyn step07_v2::VlmReviewService>>,
    work_unit_executor: Option<Arc<dyn WorkUnitExecutor>>,
    workspace_task_agent: Option<Arc<dyn WorkspaceTaskAgent>>,
    workspace_task_agent_required: bool,
    work_unit_journal_root: PathBuf,
    work_unit_stop_token: WorkUnitStopToken,
    artifact_locale: ArtifactLocale,
    protocol_root: Option<PathBuf>,
    protocol_gate_required: bool,
    allow_missing_design_data_for_tests: bool,
    unity_project_path: Option<PathBuf>,
    unity_editor_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Step12AssetConfirmationApproval {
    #[serde(default)]
    mode: String,
    #[serde(default)]
    approved_asset_ids: Vec<String>,
    #[serde(default)]
    sample_count: usize,
    #[serde(default)]
    explicit_auto_accept: bool,
}

impl ProductPipelineExecutor {
    pub fn new(root: impl AsRef<Path>, session_id: &str) -> AdmResult<Self> {
        Self::new_inner(root.as_ref(), session_id, None)
    }

    pub fn with_design_data_dir(
        root: impl AsRef<Path>,
        session_id: &str,
        design_data_dir: impl AsRef<Path>,
    ) -> AdmResult<Self> {
        let design_data_dir = design_data_dir.as_ref().to_path_buf();
        if !design_data_dir.join("domains").is_dir() {
            return Err(AdmError::new(format!(
                "pipeline design_data directory not found: {}",
                design_data_dir.display()
            )));
        }
        Self::new_inner(root.as_ref(), session_id, Some(design_data_dir))
    }

    fn new_inner(
        root: &Path,
        session_id: &str,
        design_data_dir: Option<PathBuf>,
    ) -> AdmResult<Self> {
        let root = root.to_path_buf();
        if !root.is_dir() {
            return Err(AdmError::new(format!(
                "pipeline project root not found: {}",
                root.display()
            )));
        }
        let requested_session = session_id.trim();
        let session_id = sanitize_identifier(requested_session)?;
        if session_id != requested_session {
            return Err(AdmError::new(
                "pipeline session_id must be a portable identifier",
            ));
        }
        let paths = ProjectPaths::new(&root, &session_id);
        paths.ensure_current_draft_dirs()?;
        let protocol_root = protocol_root(&root, design_data_dir.as_deref());
        Ok(Self {
            root,
            session_id,
            artifact_root: paths.artifacts_dir.clone(),
            design_data_dir,
            style_image_generator: None,
            vlm_review_service: None,
            work_unit_executor: None,
            workspace_task_agent: None,
            workspace_task_agent_required: false,
            work_unit_journal_root: paths.checkpoints_dir.join("work_units"),
            work_unit_stop_token: WorkUnitStopToken::default(),
            artifact_locale: ArtifactLocale::default(),
            protocol_root,
            protocol_gate_required: true,
            allow_missing_design_data_for_tests: false,
            unity_project_path: None,
            unity_editor_path: None,
        })
    }

    pub fn with_style_image_generator(mut self, generator: Arc<dyn StyleImageGenerator>) -> Self {
        self.style_image_generator = Some(generator);
        self
    }

    pub fn with_vlm_review_service(
        mut self,
        service: Arc<dyn step07_v2::VlmReviewService>,
    ) -> Self {
        self.vlm_review_service = Some(service);
        self
    }

    pub fn with_work_unit_executor(mut self, executor: Arc<dyn WorkUnitExecutor>) -> Self {
        self.work_unit_executor = Some(executor);
        self
    }

    pub fn with_workspace_task_agent(mut self, agent: Arc<dyn WorkspaceTaskAgent>) -> Self {
        self.workspace_task_agent = Some(agent);
        self
    }

    pub fn require_workspace_task_agent(mut self) -> Self {
        self.workspace_task_agent_required = true;
        self
    }

    /// Enables deterministic execution only when the caller explicitly requests offline mode.
    pub fn with_offline_work_unit_executor(self) -> Self {
        self.with_work_unit_executor(Arc::new(OfflineVerifiedWorkUnitExecutor))
    }

    pub fn with_work_unit_stop_token(mut self, stop_token: WorkUnitStopToken) -> Self {
        self.work_unit_stop_token = stop_token;
        self
    }

    pub fn with_artifact_locale(mut self, artifact_locale: ArtifactLocale) -> Self {
        self.artifact_locale = artifact_locale;
        self
    }

    /// Requires the packaged registry and Schema resources before any product
    /// pipeline stage may run. Product executors are fail-closed by default;
    /// this builder lets entry points state that invariant explicitly.
    pub fn require_protocol_gate(mut self) -> Self {
        self.protocol_gate_required = true;
        self
    }

    #[cfg(test)]
    fn without_protocol_gate_for_tests(mut self) -> Self {
        self.protocol_gate_required = false;
        self.allow_missing_design_data_for_tests = true;
        self
    }

    pub fn protocol_resources_ready(&self) -> bool {
        self.protocol_root.is_some()
    }

    /// Binds the Unity project used by Step13 scene-assembly requests.
    pub fn with_unity_project_path(mut self, unity_project_path: impl AsRef<Path>) -> Self {
        self.unity_project_path = non_empty_path(unity_project_path.as_ref());
        self
    }

    /// Binds the Unity editor executable used by Step13 scene-assembly requests.
    pub fn with_unity_editor_path(mut self, unity_editor_path: impl AsRef<Path>) -> Self {
        self.unity_editor_path = non_empty_path(unity_editor_path.as_ref());
        self
    }

    /// Binds one correlated Unity execution context for Step13.
    pub fn with_unity_context(
        self,
        unity_project_path: impl AsRef<Path>,
        unity_editor_path: impl AsRef<Path>,
    ) -> Self {
        self.with_unity_project_path(unity_project_path)
            .with_unity_editor_path(unity_editor_path)
    }

    pub fn artifact_locale(&self) -> ArtifactLocale {
        self.artifact_locale
    }

    pub fn work_unit_stop_token(&self) -> WorkUnitStopToken {
        self.work_unit_stop_token.clone()
    }

    pub fn reconcile_checkpoint_work_units(
        &self,
        checkpoint: &mut PipelineCheckpoint,
        stage_id: &str,
    ) -> AdmResult<StageWorkUnitReconcileStatus> {
        let step = stage_id
            .parse::<u32>()
            .map_err(|_| AdmError::new("work-unit checkpoint stage id is invalid"))?;
        if step == 7 {
            return self.reconcile_step07_checkpoint(checkpoint, stage_id);
        }
        if !matches!(step, 11 | 12) {
            return Err(AdmError::new(
                "only Step07, Step11 and Step12 have resumable internal work units",
            ));
        }
        if self.game_spec_v2_enabled() {
            return Err(AdmError::new(
                "GameSpec v2 Step11/12 checkpoint reconciliation is not compatible with legacy work-unit journals",
            ));
        }
        let executor = self.work_unit_executor.as_deref().ok_or_else(|| {
            AdmError::new("work-unit executor is unavailable for checkpoint reconciliation")
        })?;
        let stage_dir = self.artifact_root.join(format!("stage_{step:02}"));
        let requests = step08_14::work_unit_requests_for_stage(&stage_dir, step)?;
        let journal =
            SafeUnitJournal::new(self.work_unit_journal_root.join(format!("stage_{step:02}")));
        reconcile_checkpoint_stage_from_journal(
            checkpoint,
            stage_id,
            &requests,
            Some(executor),
            &journal,
        )
    }

    fn reconcile_step07_checkpoint(
        &self,
        checkpoint: &mut PipelineCheckpoint,
        stage_id: &str,
    ) -> AdmResult<StageWorkUnitReconcileStatus> {
        let stage_root = self.work_unit_journal_root.join("stage_07");
        let journal = SafeUnitJournal::new(stage_root.join("journal"));
        let records = journal.load_latest_records_unbound()?;
        let known_cache_keys = validate_step07_records(&records, stage_id)?;
        let cache_root = stage_root.join("image_cache");
        let mut recovery_blocked = false;
        for record in &records {
            let decision = reconcile_unbound_style_image_record(&cache_root, record)?;
            recovery_blocked |= decision == WorkUnitReconcileDecision::Unknown;
        }
        recovery_blocked |= step07_cache_has_orphan_entries(&cache_root, &known_cache_keys);

        let whole_unit_id = format!("{stage_id}:stage");
        let unit = checkpoint
            .units
            .iter_mut()
            .find(|unit| unit.stage_id == stage_id && unit.unit_id == whole_unit_id)
            .ok_or_else(|| AdmError::new("Step07 whole-stage checkpoint unit is missing"))?;
        unit.result_fingerprint.clear();
        if recovery_blocked {
            unit.status = PipelineUnitStatus::Unknown;
            unit.reconcile_required = true;
            unit.failure_message =
                "one or more Step07 image work units cannot be reconciled safely".to_string();
            checkpoint.status = PipelineCheckpointStatus::RecoveryBlocked;
            checkpoint.resume_policy = PipelineResumePolicy::Disabled;
            checkpoint.recovery_blocked_reason =
                "stage 07 contains an unknown image work unit side effect".to_string();
            return Ok(StageWorkUnitReconcileStatus::RecoveryBlocked);
        }

        // The image side effects are now either verified or proved safe to
        // retry. The whole-stage wrapper still reruns so Step07 can rebuild and
        // atomically publish its pure manifests and image directory.
        unit.status = PipelineUnitStatus::Pending;
        unit.reconcile_required = false;
        unit.failure_message.clear();
        checkpoint.status = PipelineCheckpointStatus::Recoverable;
        checkpoint.resume_policy = PipelineResumePolicy::ExplicitOnly;
        checkpoint.recovery_blocked_reason.clear();
        Ok(StageWorkUnitReconcileStatus::Pending)
    }

    /// Exports the current design state as the Concept, GameplayFramework and Design packages.
    pub fn prepare_project_source(&self, state: &ProjectState) -> AdmResult<PathBuf> {
        if self.protocol_gate_required && self.protocol_root.is_none() {
            return Err(AdmError::new(
                "pipeline protocol registry and Schema resources are unavailable",
            ));
        }
        let engine = self.load_design_engine()?;
        let source_root = ProjectPaths::new(&self.root, &self.session_id).source_artifacts_dir;
        let exported = export_concept_package_from_state_with_locale(
            &source_root,
            &engine,
            state,
            self.artifact_locale,
        )?;
        let concept_path = exported
            .get("package_dir")
            .and_then(Value::as_str)
            .map(PathBuf::from)
            .unwrap_or_else(|| source_root.join("devflow_Concept_v2"));
        if !concept_path.is_dir() {
            return Err(AdmError::new(format!(
                "Concept source export did not create {}",
                concept_path.display()
            )));
        }
        Ok(concept_path)
    }

    pub fn artifact_root(&self) -> &Path {
        &self.artifact_root
    }

    /// Persists a real Step07 approval. Rerunning Step07 materializes the application contract.
    pub fn confirm_style(&self, selected_style_id: &str, notes: &str) -> AdmResult<PathBuf> {
        if self.game_spec_v2_enabled() {
            let stage_dir = self.artifact_root.join("stage_07");
            if selected_style_id.trim() != "v2_anchor_set" {
                return Err(AdmError::new(format!(
                    "GameSpec v2 Step07 confirms the complete anchor set; expected v2_anchor_set, got {selected_style_id}"
                )));
            }
            let anchors =
                step07_v2::confirm_style_anchors_attended(&stage_dir, "user", notes, "attended")?;
            let compatibility = json!({
                "schema_version": "step07_v2_style_confirmation_compat.v1",
                "status": anchors.status,
                "selected_style_id": selected_style_id,
                "confirmation_mode": anchors.confirmation_mode,
                "reviewer": anchors.reviewer,
                "notes": anchors.notes,
                "anchor_count": anchors.anchors.len(),
            });
            return write_json(&stage_dir.join("style_confirmation.json"), &compatibility);
        }
        let stage_dir = self.artifact_root.join("stage_07");
        let document = read_json(&stage_dir.join("style_options.json"), json!({}));
        let options = document
            .get("options")
            .and_then(Value::as_array)
            .ok_or_else(|| AdmError::new("Step07 style_options.json is missing or empty"))?;
        let option = options
            .iter()
            .find(|option| {
                ["style_id", "option_id", "id"]
                    .iter()
                    .any(|key| option.get(*key).and_then(Value::as_str) == Some(selected_style_id))
            })
            .ok_or_else(|| {
                AdmError::new(format!(
                    "Step07 selected style not found: {selected_style_id}"
                ))
            })?;
        step07::write_style_confirmation(&stage_dir, option, notes, "approved", "manual")
    }

    fn game_spec_v2_enabled(&self) -> bool {
        let paths = ProjectPaths::new(&self.root, &self.session_id);
        [
            paths.draft_dir.join("project_config.json"),
            paths.project_settings_file,
        ]
        .into_iter()
        .map(|path| read_json(&path, json!({})))
        .any(|value| {
            bool_at_path(&value, &["game_spec_v2", "enabled"])
                || bool_at_path(&value, &["gameSpecV2", "enabled"])
                || bool_at_path(&value, &["game_spec_v2_enabled"])
                || bool_at_path(&value, &["gameSpecV2Enabled"])
                || bool_at_path(&value, &["features", "game_spec_v2"])
                || bool_at_path(&value, &["features", "gameSpecV2"])
                || value
                    .get("game_spec_v2")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
        })
    }

    fn load_or_project_game_spec_v2(&self) -> AdmResult<GameSpec> {
        let paths = ProjectPaths::new(&self.root, &self.session_id);
        for candidate in [
            self.artifact_root.join("stage_06/r1_frozen_game_spec.json"),
            self.artifact_root
                .join("stage_06/game_spec_v2_shadow/game_spec.json"),
            self.artifact_root
                .join("stage_d4/game_spec_v2_shadow/game_spec.json"),
            self.root.join(".game_spec_v2_migration/game_spec.json"),
            paths
                .draft_dir
                .join(".game_spec_v2_migration/game_spec.json"),
        ] {
            if candidate.is_file() {
                return parse_game_spec_file(&candidate);
            }
        }
        let autosave_path = paths.draft_dir.join("autosave_state.json");
        if autosave_path.is_file() {
            let state: ProjectState =
                serde_json::from_slice(&fs::read(&autosave_path)?).map_err(|error| {
                    AdmError::new(format!("autosave ProjectState is invalid: {error}"))
                })?;
            let engine = self.load_design_engine()?;
            let projection = project_state_to_game_spec(&engine, &state)?;
            let compiler = self.game_spec_v2_step00_06_compiler()?;
            let run = compiler.compile_spec(projection.spec.clone());
            let stage06_dir = self.artifact_root.join("stage_06");
            compiler.write_outputs(&stage06_dir, &run)?;
            if run.status == StepGateStatus::Passed {
                return Ok(projection.spec);
            }
            return Err(AdmError::new(
                "projected GameSpec v2 did not pass Step00-06 freeze gates",
            ));
        }
        Err(AdmError::new(
            "game_spec_v2 is enabled but no frozen GameSpec was found",
        ))
    }

    fn game_spec_v2_step00_06_compiler(&self) -> AdmResult<Step00_06Compiler> {
        let supported_envelope = ProductEnvelope {
            scene_scale: adm_new_game_spec::ProductionScale::Medium,
            system_complexity: adm_new_game_spec::ProductionScale::Medium,
            asset_scale: adm_new_game_spec::ProductionScale::Medium,
            content_volume: adm_new_game_spec::ProductionScale::Medium,
        };
        let local = self.root.join("knowledge/design_data");
        let data_dir = self
            .design_data_dir
            .clone()
            .into_iter()
            .chain(std::iter::once(local))
            .find(|path| path.join("domains").is_dir());
        let Some(data_dir) = data_dir else {
            return Err(AdmError::new(format!(
                "GameSpec v2 Step02 decision graph domains are missing: expected an explicit resource root or {}",
                self.root.join("knowledge/design_data/domains").display()
            )));
        };
        Step00_06Compiler::from_design_data_dir(supported_envelope, &self.root, data_dir)
    }

    fn execute_game_spec_v2_step(
        &self,
        step_number: u32,
        service: &GenerationService,
    ) -> PipelineStageResult {
        let stage_dir = service.stage_dir(step_number);
        let generation = self
            .run_game_spec_v2_step(step_number, &stage_dir)
            .and_then(|(raw_status, output, blockers)| {
                let report = game_spec_v2_validation_report(
                    step_number,
                    self.artifact_locale,
                    raw_status,
                    output,
                    blockers,
                );
                write_json(&stage_dir.join("game_spec_v2_stage_report.json"), &report)?;
                write_json(&stage_dir.join("validation_report.json"), &report)?;
                service.refresh_indexes(step_number)?;
                Ok(report)
            });
        stage_result_from_generation(step_number, generation, &self.artifact_root, &stage_dir)
    }

    fn run_game_spec_v2_step(
        &self,
        step_number: u32,
        stage_dir: &Path,
    ) -> AdmResult<(String, Value, Vec<String>)> {
        fs::create_dir_all(stage_dir)?;
        let spec = self.load_or_project_game_spec_v2()?;
        match step_number {
            7 => {
                let vlm = self.v2_vlm_review_service()?;
                let output =
                    step07_v2::compile_step07_art_direction_with_vlm(&spec, stage_dir, &*vlm)?;
                let blockers = output
                    .vlm_review_report
                    .as_ref()
                    .map(|report| report.blocking_issues.clone())
                    .unwrap_or_default();
                Ok((output.status.clone(), json_value(output)?, blockers))
            }
            8..=10 => {
                let anchors = self.v2_style_anchors()?;
                let output = step08_10_v2::compile_step08_10(&spec, &anchors, stage_dir)?;
                let blockers = if output.status == "success" {
                    Vec::new()
                } else {
                    output.task_graph.validation.issues.clone()
                };
                Ok((output.status.clone(), json_value(output)?, blockers))
            }
            11 => {
                let compiled = self.v2_compile_step08_10()?;
                let mut state = step11_v2::Step11ExecutionState::new(
                    compiled.task_graph.source_game_spec_hash.clone(),
                );
                let agent: Arc<dyn WorkspaceTaskAgent> = if let Some(agent) =
                    self.workspace_task_agent.clone()
                {
                    agent
                } else if self.workspace_task_agent_required {
                    return Err(AdmError::new(
                        "GameSpec v2 Step11 requires a configured WorkspaceTaskAgent; local filesystem fallback is disabled for product runs",
                    ));
                } else {
                    Arc::new(FilesystemWorkspaceTaskAgent {
                        workspace_root: ProjectPaths::new(&self.root, &self.session_id)
                            .workspace_dir
                            .join("game_spec_v2"),
                    })
                };
                let output = step11_v2::Step11ExecutionEngine::new(
                    agent,
                    step11_v2::Step11ExecutionBudget::default(),
                )
                .run(
                    &compiled.task_graph,
                    &mut state,
                    &step11_v2::Step11StopToken::from_shared(
                        self.work_unit_stop_token.shared_flag(),
                    ),
                )?;
                let blockers = output
                    .correction_queue
                    .iter()
                    .filter(|item| !item.resolved)
                    .map(|item| format!("{}:{}", item.task_id, item.reason))
                    .collect::<Vec<_>>();
                io::write_json_serializable(
                    &stage_dir.join("step11_execution_report.json"),
                    &output,
                )?;
                Ok((
                    format!("{:?}", output.status).to_ascii_lowercase(),
                    json_value(output)?,
                    blockers,
                ))
            }
            12 => {
                let anchors = self.v2_style_anchors()?;
                let compiled = self.v2_compile_step08_10()?;
                let policy =
                    self.v2_asset_production_policy(stage_dir, &compiled.asset_manifest)?;
                let vlm = self.v2_vlm_review_service()?;
                let workspace_target = self.v2_workspace_target_root();
                let bindings = step12_v2::discover_asset_bindings_from_workspace(
                    &compiled.asset_manifest,
                    &workspace_target,
                )?;
                let loader = step12_v2::WorkspaceReferenceAssetLoader::new(&workspace_target);
                let output = step12_v2::run_step12_asset_production_with_vlm(
                    &compiled.asset_manifest,
                    &anchors,
                    stage_dir,
                    &policy,
                    &bindings,
                    &loader,
                    &*vlm,
                )?;
                let blockers = output
                    .correction_queue
                    .iter()
                    .map(|item| format!("{}:{}", item.asset_id, item.reason))
                    .collect::<Vec<_>>();
                Ok((
                    format!("{:?}", output.status).to_ascii_lowercase(),
                    json_value(output)?,
                    blockers,
                ))
            }
            13 => {
                let step11 = read_json_typed::<step11_v2::Step11ExecutionReport>(
                    &self
                        .artifact_root
                        .join("stage_11/step11_execution_report.json"),
                )?;
                let step12 = read_json_typed::<step12_v2::Step12AssetProductionOutput>(
                    &self
                        .artifact_root
                        .join("stage_12/step12_asset_production_output.json"),
                )?;
                let evidence_path = stage_dir.join("scenario_execution_evidence.json");
                let policy = if evidence_path.is_file() {
                    let evidence =
                        read_json_typed::<step13_v2::Step13ExecutionEvidence>(&evidence_path)?;
                    step13_v2::Step13ValidationPolicy::from_execution_evidence(evidence)
                } else {
                    step13_v2::Step13ValidationPolicy::strict_unattended()
                };
                let output = step13_v2::run_step13_acceptance_validation(
                    &spec, &step11, &step12, &policy, stage_dir,
                )?;
                let blockers = output
                    .scenario_results
                    .iter()
                    .filter_map(|result| {
                        result
                            .failure_reason
                            .as_ref()
                            .map(|reason| format!("{}:{reason}", result.scenario_id))
                    })
                    .collect::<Vec<_>>();
                Ok((
                    format!("{:?}", output.status).to_ascii_lowercase(),
                    json_value(output)?,
                    blockers,
                ))
            }
            14 => {
                let step13 = read_json_typed::<step13_v2::Step13AcceptanceOutput>(
                    &self
                        .artifact_root
                        .join("stage_13/step13_acceptance_output.json"),
                )?;
                let derivation =
                    step14_v2::derive_r1_gate_evidence_from_sources(&self.root, stage_dir);
                let evidence_path = io::write_json_serializable(
                    &stage_dir.join("r1_gate_evidence.json"),
                    &derivation.evidence,
                )?;
                let source_report_path = io::write_json_serializable(
                    &stage_dir.join(step14_v2::R1_GATE_EVIDENCE_SOURCE_REPORT_FILE),
                    &derivation.report,
                )?;
                let mut output = step14_v2::run_step14_r1_packaging_gate(
                    &spec,
                    &step13,
                    &derivation.evidence,
                    stage_dir,
                )?;
                output.output_paths.insert(
                    "r1GateEvidence".to_string(),
                    evidence_path.to_string_lossy().replace('\\', "/"),
                );
                output.output_paths.insert(
                    "r1GateEvidenceSourceReport".to_string(),
                    source_report_path.to_string_lossy().replace('\\', "/"),
                );
                for blocker in derivation.report.blockers {
                    if !output.blockers.contains(&blocker) {
                        output.blockers.push(blocker);
                    }
                }
                if !output.blockers.is_empty() {
                    output.status = step14_v2::R1GateStatus::Blocked;
                    output.release_manifest.status = step14_v2::R1GateStatus::Blocked;
                }
                io::write_json_serializable(
                    &stage_dir.join("r1_release_manifest.json"),
                    &output.release_manifest,
                )?;
                io::write_json_serializable(
                    &stage_dir.join("step14_r1_packaging_output.json"),
                    &output,
                )?;
                Ok((
                    format!("{:?}", output.status).to_ascii_lowercase(),
                    json_value(&output)?,
                    output.blockers,
                ))
            }
            _ => Err(AdmError::new(format!(
                "game_spec_v2 has no product route for stage {step_number:02}"
            ))),
        }
    }

    fn v2_style_anchors(&self) -> AdmResult<Vec<step07_v2::StyleAnchorCandidate>> {
        let anchor_set = read_json_typed::<step07_v2::StyleAnchorSet>(
            &self.artifact_root.join("stage_07/style_anchor_set.json"),
        )?;
        if anchor_set.status != "approved" {
            return Err(AdmError::new("Step07 v2 style anchors are not approved"));
        }
        Ok(anchor_set.anchors)
    }

    fn v2_vlm_review_service(&self) -> AdmResult<Arc<dyn step07_v2::VlmReviewService>> {
        if let Some(service) = self.vlm_review_service.clone() {
            return Ok(service);
        }
        Ok(Arc::new(
            step07_v2::CachedVlmReviewService::with_cache_file(
                "vlm_unconfigured",
                self.artifact_root.join("vlm_review_cache/reviews.json"),
            )?,
        ))
    }

    fn v2_workspace_target_root(&self) -> PathBuf {
        ProjectPaths::new(&self.root, &self.session_id)
            .workspace_dir
            .join("game_spec_v2/target")
    }

    fn v2_compile_step08_10(&self) -> AdmResult<step08_10_v2::Step08_10Compilation> {
        let stage10_dir = self.artifact_root.join("stage_10");
        let existing = stage10_dir.join("step08_10_compilation.json");
        let spec = self.load_or_project_game_spec_v2()?;
        let source_hash = game_spec_content_hash(&spec)?;
        if existing.is_file() {
            let compiled = read_json_typed::<step08_10_v2::Step08_10Compilation>(&existing)?;
            validate_frozen_step08_10_hashes(&compiled, &source_hash)?;
            return Ok(compiled);
        }
        let anchors = self.v2_style_anchors()?;
        let compiled = step08_10_v2::compile_step08_10(&spec, &anchors, &stage10_dir)?;
        validate_frozen_step08_10_hashes(&compiled, &source_hash)?;
        Ok(compiled)
    }

    fn v2_asset_production_policy(
        &self,
        stage_dir: &Path,
        manifest: &step08_10_v2::FrozenAssetManifest,
    ) -> AdmResult<step12_v2::AssetProductionPolicy> {
        let approval_path = stage_dir.join("asset_confirmation_approval.json");
        if !approval_path.is_file() {
            return Ok(step12_v2::AssetProductionPolicy::attended_pending());
        }
        let approval = read_json_typed::<Step12AssetConfirmationApproval>(&approval_path)?;
        let manifest_ids = manifest
            .items
            .iter()
            .map(|item| item.asset_id.as_str())
            .collect::<StdBTreeSet<_>>();
        for asset_id in &approval.approved_asset_ids {
            if !manifest_ids.contains(asset_id.as_str()) {
                return Err(AdmError::new(format!(
                    "Step12 asset confirmation references unknown asset: {asset_id}"
                )));
            }
        }
        match approval.mode.as_str() {
            "attended" => Ok(step12_v2::AssetProductionPolicy {
                strategy: step12_v2::ConfirmationStrategy::Attended,
                approved_asset_ids: approval.approved_asset_ids.into_iter().collect(),
                sample_count: 0,
                style_distance_threshold: 115.0,
            }),
            "sample" => Ok(step12_v2::AssetProductionPolicy::sample(
                approval.sample_count.max(1),
                approval.approved_asset_ids,
            )),
            "auto_accept" if approval.explicit_auto_accept => {
                Ok(step12_v2::AssetProductionPolicy::attended_approved(
                    manifest.items.iter().map(|item| item.asset_id.clone()),
                ))
            }
            "auto_accept" => Err(AdmError::new(
                "Step12 auto_accept requires explicitAutoAccept=true",
            )),
            other => Err(AdmError::new(format!(
                "unsupported Step12 asset confirmation mode: {other}"
            ))),
        }
    }

    fn load_design_engine(&self) -> AdmResult<DesignEngineService> {
        let local = self.root.join("knowledge/design_data");
        let data_dir = self
            .design_data_dir
            .clone()
            .into_iter()
            .chain(std::iter::once(local))
            .find(|path| path.join("domains").is_dir());
        let Some(data_dir) = data_dir else {
            if self.allow_missing_design_data_for_tests {
                return Ok(DesignEngineService::new(Vec::new()));
            }
            return Err(AdmError::new(format!(
                "standalone design data is missing: expected an explicit resource root or {}",
                self.root.join("knowledge/design_data/domains").display()
            )));
        };
        let data =
            DesignDataLoader::from_design_data_dir(&self.root, data_dir).load_project_data()?;
        Ok(design_engine_from_data(&data))
    }

    fn execute_step(&self, step_number: u32) -> PipelineStageResult {
        let service = match GenerationService::new(&self.root, &self.session_id) {
            Ok(service) => service.with_artifact_locale(self.artifact_locale),
            Err(error) => {
                return failed_stage_result_with_locale(
                    step_number,
                    error.to_string(),
                    self.artifact_locale,
                );
            }
        };
        if self.protocol_gate_required && self.protocol_root.is_none() {
            return self.protocol_resources_unavailable_result(step_number, &service);
        }
        if (7..=14).contains(&step_number) && self.game_spec_v2_enabled() {
            return self.execute_game_spec_v2_step(step_number, &service);
        }
        if let Some(result) = self.upstream_locale_mismatch_result(step_number, &service) {
            return result;
        }
        let generator = match self.generator_for_step(step_number) {
            Ok(generator) => generator,
            Err(error) => {
                return failed_stage_result_with_locale(
                    step_number,
                    error.to_string(),
                    self.artifact_locale,
                );
            }
        };
        let generation = service.apply_development_plan_outputs(step_number, generator.as_ref());
        let generation = self.apply_registered_protocol_gate(step_number, &service, generation);
        let stage_dir = service.stage_dir(step_number);
        stage_result_from_generation(step_number, generation, &self.artifact_root, &stage_dir)
    }

    fn protocol_resources_unavailable_result(
        &self,
        step_number: u32,
        service: &GenerationService,
    ) -> PipelineStageResult {
        let stage_dir = service.stage_dir(step_number);
        let issue = json!({
            "code": "PIPELINE_PROTOCOL_RESOURCES_UNAVAILABLE",
            "severity": "blocker",
            "message": localized_protocol_message(
                self.artifact_locale,
                "流水线注册表或 Schema 资源不可用，已阻止无协议执行。",
                "The pipeline registry or Schema resources are unavailable; execution without protocol validation was blocked.",
            ),
            "return_target": "environment_configuration",
        });
        let business_quality = json!({
            "status": "blocked",
            "artifact_locale": self.artifact_locale,
            "content_exists": false,
            "blocking_issues": [issue.clone()],
            "review_items_count": 0,
            "ai_review_status": "blocked",
            "traceability_valid": false,
        });
        let report = json!({
            "schema_version": "1.0",
            "stage": step_number,
            "stage_id": format!("{step_number:02}"),
            "status": "blocked",
            "valid": false,
            "artifact_locale": self.artifact_locale,
            "content_exists": false,
            "blocking_issues": [issue],
            "review_items_count": 0,
            "ai_review_status": "blocked",
            "traceability_valid": false,
            "business_quality": business_quality,
        });
        let generation = fs::create_dir_all(&stage_dir)
            .map_err(AdmError::from)
            .and_then(|_| {
                write_json(&stage_dir.join("registry_protocol_report.json"), &report)?;
                write_json(&stage_dir.join("validation_report.json"), &report)?;
                service.refresh_indexes(step_number)?;
                Ok(report)
            });
        stage_result_from_generation(step_number, generation, &self.artifact_root, &stage_dir)
    }

    fn apply_registered_protocol_gate(
        &self,
        step_number: u32,
        service: &GenerationService,
        generation: AdmResult<Value>,
    ) -> AdmResult<Value> {
        let generation = generation?;
        let Some(protocol_root) = self.protocol_root.as_deref() else {
            return Ok(generation);
        };
        if step_number == 7
            && generation.get("status").and_then(Value::as_str) == Some("waiting_confirmation")
        {
            let stage_dir = service.stage_dir(step_number);
            write_json(
                &stage_dir.join("registry_protocol_report.json"),
                &json!({
                    "schema_version": "1.0",
                    "stage_id": "07",
                    "status": "deferred_for_confirmation",
                    "artifact_locale": self.artifact_locale,
                    "checks": [],
                    "blocking_issues": [],
                }),
            )?;
            service.refresh_indexes(step_number)?;
            return Ok(generation);
        }
        let report = self.validate_registered_stage_artifacts(step_number, protocol_root)?;
        let stage_dir = service.stage_dir(step_number);
        write_json(&stage_dir.join("registry_protocol_report.json"), &report)?;
        let blocking_issues = report
            .get("blocking_issues")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if blocking_issues.is_empty() {
            service.refresh_indexes(step_number)?;
            return Ok(generation);
        }
        let blocked = json!({
            "status": "blocked",
            "artifact_locale": self.artifact_locale,
            "content_exists": true,
            "blocking_issues": blocking_issues,
            "review_items_count": 0,
            "ai_review_status": "blocked",
            "traceability_valid": false,
            "message": if self.artifact_locale == ArtifactLocale::ZhCn {
                format!("步骤 {step_number:02} 的注册产物协议校验未通过。")
            } else {
                format!("Step{step_number:02} failed the registered artifact protocol gate.")
            },
            "return_target": format!("stage_{step_number:02}"),
        });
        let updated = service.update_stage_report(step_number, &blocked)?;
        service.refresh_indexes(step_number)?;
        Ok(updated)
    }

    fn validate_registered_stage_artifacts(
        &self,
        step_number: u32,
        protocol_root: &Path,
    ) -> AdmResult<Value> {
        let registry_path = protocol_root.join("pipeline/artifact_layer/registry.json");
        let registry_text = fs::read_to_string(&registry_path).map_err(|error| {
            AdmError::new(format!(
                "failed to read artifact protocol registry {}: {error}",
                registry_path.display()
            ))
        })?;
        let registry: ArtifactRegistry = serde_json::from_str(
            registry_text.trim_start_matches('\u{feff}'),
        )
        .map_err(|error| AdmError::new(format!("invalid artifact protocol registry: {error}")))?;
        let mut seen = BTreeSet::new();
        let mut checks = Vec::new();
        let mut issues = Vec::new();
        for artifact in registry
            .artifacts
            .iter()
            .filter(|artifact| artifact.stage == step_number)
        {
            for schema_ref in &artifact.schema_refs {
                let identity = (schema_ref.path.clone(), schema_ref.schema.clone());
                if !seen.insert(identity) {
                    continue;
                }
                let return_target = return_target_from_contract_path(&schema_ref.path, step_number);
                let Some(contract_path) =
                    registered_output_path(&self.artifact_root, &schema_ref.path)
                else {
                    issues.push(protocol_gate_issue(
                        "REGISTERED_ARTIFACT_PATH_INVALID",
                        &schema_ref.path,
                        &schema_ref.schema,
                        &return_target,
                        self.artifact_locale,
                        "注册表中的产物路径无效。",
                        "The registered artifact path is invalid.",
                    ));
                    continue;
                };
                let Some(schema_path) = registered_schema_path(protocol_root, &schema_ref.schema)
                else {
                    issues.push(protocol_gate_issue(
                        "REGISTERED_SCHEMA_PATH_INVALID",
                        &schema_ref.path,
                        &schema_ref.schema,
                        &return_target,
                        self.artifact_locale,
                        "注册表中的 Schema 路径无效。",
                        "The registered schema path is invalid.",
                    ));
                    continue;
                };
                if !contract_path.is_file() {
                    issues.push(protocol_gate_issue(
                        "REGISTERED_ARTIFACT_MISSING",
                        &schema_ref.path,
                        &schema_ref.schema,
                        &return_target,
                        self.artifact_locale,
                        "注册表要求的产物文件未生成。",
                        "A required registered artifact file was not generated.",
                    ));
                    continue;
                }
                if !schema_path.is_file() {
                    issues.push(protocol_gate_issue(
                        "REGISTERED_SCHEMA_MISSING",
                        &schema_ref.path,
                        &schema_ref.schema,
                        &return_target,
                        self.artifact_locale,
                        "注册表引用的 Schema 文件不存在。",
                        "The schema referenced by the registry does not exist.",
                    ));
                    continue;
                }
                let errors = match adm_new_contracts::schema::validate_contract_file(
                    &contract_path,
                    &schema_path,
                ) {
                    Ok(errors) => errors,
                    Err(_) => {
                        issues.push(protocol_gate_issue(
                            "REGISTERED_ARTIFACT_OR_SCHEMA_UNREADABLE",
                            &schema_ref.path,
                            &schema_ref.schema,
                            &return_target,
                            self.artifact_locale,
                            "注册产物或 Schema 无法安全解析。",
                            "The registered artifact or schema could not be parsed safely.",
                        ));
                        continue;
                    }
                };
                if !errors.is_empty() {
                    let mut issue = protocol_gate_issue(
                        "REGISTERED_ARTIFACT_SCHEMA_INVALID",
                        &schema_ref.path,
                        &schema_ref.schema,
                        &return_target,
                        self.artifact_locale,
                        "产物内容不符合注册表声明的 Schema。",
                        "The artifact does not match its registered schema.",
                    );
                    issue["validation_errors"] =
                        json!(errors.into_iter().take(8).collect::<Vec<_>>());
                    issues.push(issue);
                    continue;
                }
                if schema_ref.path.starts_with("outputs/artifacts/") {
                    let value = read_json(&contract_path, json!({}));
                    let actual_locale = value
                        .get("artifact_locale")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if actual_locale != self.artifact_locale.as_str() {
                        let mut issue = protocol_gate_issue(
                            "REGISTERED_ARTIFACT_LOCALE_MISMATCH",
                            &schema_ref.path,
                            &schema_ref.schema,
                            &return_target,
                            self.artifact_locale,
                            "注册产物的语言标记与本次运行不一致。",
                            "The registered artifact locale does not match this run.",
                        );
                        issue["actual_locale"] = json!(actual_locale);
                        issue["expected_locale"] = json!(self.artifact_locale);
                        issues.push(issue);
                        continue;
                    }
                }
                checks.push(json!({
                    "artifact_id": artifact.id,
                    "path": schema_ref.path,
                    "schema": schema_ref.schema,
                    "status": "passed",
                }));
            }
        }
        if seen.is_empty() {
            issues.push(protocol_gate_issue(
                "REGISTERED_STAGE_CONTRACT_MISSING",
                &format!("outputs/artifacts/stage_{step_number:02}"),
                "",
                &format!("stage_{step_number:02}"),
                self.artifact_locale,
                "注册表没有声明本步骤的 Schema 产物协议。",
                "The registry declares no schema artifact protocol for this stage.",
            ));
        }
        Ok(json!({
            "schema_version": "1.0",
            "stage_id": format!("{step_number:02}"),
            "status": if issues.is_empty() { "passed" } else { "blocked" },
            "artifact_locale": self.artifact_locale,
            "registry_version": registry.version,
            "checks": checks,
            "blocking_issues": issues,
        }))
    }

    fn upstream_locale_mismatch_result(
        &self,
        step_number: u32,
        service: &GenerationService,
    ) -> Option<PipelineStageResult> {
        let mut issues = Vec::<(u32, Value)>::new();
        let required_upstream_stages = product_required_upstream_stages(step_number);
        if let Some(protocol_root) = self.protocol_root.as_deref() {
            let registry_path = protocol_root.join("pipeline/artifact_layer/registry.json");
            let registry = fs::read(&registry_path)
                .ok()
                .and_then(|bytes| parse_json_bytes_with_bom::<ArtifactRegistry>(&bytes));
            if let Some(registry) = registry {
                let mut seen_paths = BTreeSet::new();
                for artifact in registry
                    .artifacts
                    .iter()
                    .filter(|artifact| required_upstream_stages.contains(&artifact.stage))
                {
                    for schema_ref in &artifact.schema_refs {
                        let normalized = schema_ref.path.replace('\\', "/");
                        if !normalized.starts_with("outputs/artifacts/")
                            || !seen_paths.insert(normalized.clone())
                        {
                            continue;
                        }
                        let Some(path) = registered_output_path(&self.artifact_root, &normalized)
                        else {
                            issues.push((artifact.stage, json!({
                                "code": "UPSTREAM_REGISTERED_ARTIFACT_PATH_INVALID",
                                "message": localized_protocol_message(
                                    self.artifact_locale,
                                    &format!("上游注册产物路径 `{normalized}` 无法安全解析。"),
                                    &format!("Upstream registered artifact path `{normalized}` cannot be resolved safely."),
                                ),
                                "return_target": format!("stage_{:02}", artifact.stage),
                                "upstream_stage_id": format!("{:02}", artifact.stage),
                                "artifact_path": normalized,
                                "actual_locale": Value::Null,
                                "expected_locale": self.artifact_locale,
                            })));
                            continue;
                        };
                        if !path.is_file() {
                            issues.push((artifact.stage, json!({
                                "code": "UPSTREAM_REGISTERED_ARTIFACT_MISSING",
                                "message": localized_protocol_message(
                                    self.artifact_locale,
                                    &format!("上游注册产物 `{normalized}` 不存在；不能安全执行局部重跑。"),
                                    &format!("Upstream registered artifact `{normalized}` is missing; the partial rerun is unsafe."),
                                ),
                                "return_target": format!("stage_{:02}", artifact.stage),
                                "upstream_stage_id": format!("{:02}", artifact.stage),
                                "artifact_path": normalized,
                                "actual_locale": Value::Null,
                                "expected_locale": self.artifact_locale,
                            })));
                            continue;
                        }
                        let document = fs::read(&path)
                            .ok()
                            .and_then(|bytes| parse_json_bytes_with_bom::<Value>(&bytes));
                        let Some(document) = document else {
                            issues.push((artifact.stage, json!({
                                "code": "UPSTREAM_REGISTERED_ARTIFACT_UNREADABLE",
                                "message": localized_protocol_message(
                                    self.artifact_locale,
                                    &format!("上游注册产物 `{normalized}` 无法读取为 JSON。"),
                                    &format!("Upstream registered artifact `{normalized}` cannot be read as JSON."),
                                ),
                                "return_target": format!("stage_{:02}", artifact.stage),
                                "upstream_stage_id": format!("{:02}", artifact.stage),
                                "artifact_path": normalized,
                                "actual_locale": Value::Null,
                                "expected_locale": self.artifact_locale,
                            })));
                            continue;
                        };
                        let actual_locale = document
                            .get("artifact_locale")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty());
                        if actual_locale == Some(self.artifact_locale.as_str()) {
                            continue;
                        }
                        let code = if actual_locale.is_some() {
                            "UPSTREAM_ARTIFACT_LOCALE_MISMATCH"
                        } else {
                            "UPSTREAM_ARTIFACT_LOCALE_MISSING"
                        };
                        issues.push((artifact.stage, json!({
                            "code": code,
                            "message": localized_protocol_message(
                                self.artifact_locale,
                                &format!("上游注册产物 `{normalized}` 的语言标记为 `{}`，与本次运行要求的 `{}` 不一致；请从步骤 {:02} 或更早步骤重新运行。", actual_locale.unwrap_or("<missing>"), self.artifact_locale.as_str(), artifact.stage),
                                &format!("Upstream registered artifact `{normalized}` has locale `{}`, but this run requires `{}`; rerun from Step{:02} or earlier.", actual_locale.unwrap_or("<missing>"), self.artifact_locale.as_str(), artifact.stage),
                            ),
                            "return_target": format!("stage_{:02}", artifact.stage),
                            "upstream_stage_id": format!("{:02}", artifact.stage),
                            "artifact_path": normalized,
                            "actual_locale": actual_locale,
                            "expected_locale": self.artifact_locale,
                        })));
                    }
                }
            } else {
                issues.push((
                    0,
                    json!({
                        "code": "UPSTREAM_PROTOCOL_REGISTRY_UNREADABLE",
                        "message": localized_protocol_message(
                            self.artifact_locale,
                            "流水线注册表无法读取；不能安全执行局部重跑。",
                            "The pipeline registry is unreadable; the partial rerun is unsafe.",
                        ),
                        "return_target": "environment_configuration",
                        "upstream_stage_id": "00",
                        "artifact_path": "pipeline/artifact_layer/registry.json",
                        "actual_locale": Value::Null,
                        "expected_locale": self.artifact_locale,
                    }),
                ));
            }
        } else {
            // Resource-free executors exist only for isolated library tests. Keep their
            // compatibility check strict when a legacy validation report is present:
            // a missing or unknown marker must never normalize silently to zh-CN.
            for upstream_step in required_upstream_stages {
                let report_path = self
                    .artifact_root
                    .join(format!("stage_{upstream_step:02}/validation_report.json"));
                if !report_path.is_file() {
                    continue;
                }
                let report = read_json(&report_path, json!({}));
                let actual_locale = report
                    .get("artifact_locale")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty());
                if actual_locale == Some(self.artifact_locale.as_str()) {
                    continue;
                }
                issues.push((upstream_step, json!({
                    "code": if actual_locale.is_some() { "UPSTREAM_ARTIFACT_LOCALE_MISMATCH" } else { "UPSTREAM_ARTIFACT_LOCALE_MISSING" },
                    "message": localized_protocol_message(
                        self.artifact_locale,
                        &format!("上游步骤 {upstream_step:02} 的产物语言标记为 `{}`，与本次运行要求的 `{}` 不一致；请从该步骤或更早步骤重新运行。", actual_locale.unwrap_or("<missing>"), self.artifact_locale.as_str()),
                        &format!("Upstream Step{upstream_step:02} has artifact locale `{}`, but this run requires `{}`; rerun from that step or earlier.", actual_locale.unwrap_or("<missing>"), self.artifact_locale.as_str()),
                    ),
                    "return_target": format!("stage_{upstream_step:02}"),
                    "upstream_stage_id": format!("{upstream_step:02}"),
                    "actual_locale": actual_locale,
                    "expected_locale": self.artifact_locale,
                })));
            }
        }
        if issues.is_empty() {
            return None;
        }
        issues.sort_by_key(|(stage, _)| *stage);

        let stage_dir = service.stage_dir(step_number);
        if let Err(error) = std::fs::create_dir_all(&stage_dir) {
            return Some(failed_stage_result_with_locale(
                step_number,
                error.to_string(),
                self.artifact_locale,
            ));
        }
        let earliest = issues[0].0;
        let overall_return_target = issues[0]
            .1
            .get("return_target")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("stage_{earliest:02}"));
        let issue_values = issues
            .into_iter()
            .map(|(_, issue)| issue)
            .collect::<Vec<_>>();
        let business_quality = json!({
            "status": "blocked",
            "artifact_locale": self.artifact_locale,
            "content_exists": true,
            "blocking_issues": issue_values,
            "review_items_count": 0,
            "ai_review_status": "blocked",
            "traceability_valid": false,
            "message": if self.artifact_locale == ArtifactLocale::ZhCn {
                format!("步骤 {step_number:02} 已因上游注册产物完整性或语言兼容性校验未通过而阻断。")
            } else {
                format!("Step{step_number:02} was blocked because upstream registered-artifact integrity or locale compatibility validation failed.")
            },
            "return_target": overall_return_target,
        });
        let report = json!({
            "stage": step_number,
            "status": "blocked",
            "valid": false,
            "artifact_locale": self.artifact_locale,
            "content_exists": true,
            "blocking_issues": business_quality["blocking_issues"],
            "review_items_count": 0,
            "ai_review_status": "blocked",
            "traceability_valid": false,
            "business_quality": business_quality,
        });
        if let Err(error) = write_json(&stage_dir.join("locale_compatibility_report.json"), &report)
            .and_then(|_| write_json(&stage_dir.join("validation_report.json"), &report))
            .and_then(|_| service.refresh_indexes(step_number))
        {
            return Some(failed_stage_result_with_locale(
                step_number,
                error.to_string(),
                self.artifact_locale,
            ));
        }
        Some(stage_result_from_generation(
            step_number,
            Ok(report),
            &self.artifact_root,
            &stage_dir,
        ))
    }

    fn generator_for_step(&self, step_number: u32) -> AdmResult<Box<dyn StageOutputGenerator>> {
        match step_number {
            0..=2 => step00_02::generator_for_step(step_number),
            3..=6 => step03_06::generator_for_step(step_number),
            7 => Ok(Box::new(step07::Step07OutputGenerator::with_safe_units(
                self.style_image_generator.clone(),
                self.work_unit_journal_root.join("stage_07"),
                self.work_unit_stop_token.clone(),
            ))),
            8..=10 => step08_14::generator_for_step(step_number),
            13 | 14 => Ok(Box::new(UnityContextStageOutputGenerator {
                inner: step08_14::generator_for_step(step_number)?,
                unity_project_path: self.unity_project_path.clone(),
                unity_editor_path: self.unity_editor_path.clone(),
            })),
            11 => Ok(Box::new(step08_14::Step11OutputGenerator::new(
                self.work_unit_executor.clone(),
                Some(self.work_unit_journal_root.clone()),
                self.work_unit_stop_token.clone(),
            ))),
            12 => Ok(Box::new(step08_14::Step12OutputGenerator::new(
                self.work_unit_executor.clone(),
                Some(self.work_unit_journal_root.clone()),
                self.work_unit_stop_token.clone(),
            ))),
            _ => Err(AdmError::new(format!(
                "no product generator for stage {step_number:02}"
            ))),
        }
    }
}

struct UnityContextStageOutputGenerator {
    inner: Box<dyn StageOutputGenerator>,
    unity_project_path: Option<PathBuf>,
    unity_editor_path: Option<PathBuf>,
}

impl StageOutputGenerator for UnityContextStageOutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &crate::generation::ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        let mut merged = structured_inputs.as_object().cloned().unwrap_or_default();
        if let Some(path) = self.unity_project_path.as_deref() {
            merged.insert(
                "unity_project_path".to_string(),
                json!(path.to_string_lossy()),
            );
        }
        if let Some(path) = self.unity_editor_path.as_deref() {
            merged.insert(
                "unity_editor_path".to_string(),
                json!(path.to_string_lossy()),
            );
        }
        self.inner
            .generate(step_number, parsed, out_dir, &Value::Object(merged))
    }
}

#[derive(Debug, Clone)]
struct FilesystemWorkspaceTaskAgent {
    workspace_root: PathBuf,
}

impl WorkspaceTaskAgent for FilesystemWorkspaceTaskAgent {
    fn execute_task(
        &self,
        task: &step08_10_v2::TrustedDevelopmentTask,
        attempt: u32,
        _previous_failure: Option<&step11_v2::Step11FailureEvidence>,
    ) -> AdmResult<WorkspaceTransactionResult> {
        let contract = &task.workspace_contract;
        let contract_hash = contract.contract_hash().map_err(|error| {
            AdmError::new(format!("failed to hash WorkspaceChangeSet: {error}"))
        })?;
        let report = contract.validate();
        if !report.is_valid() {
            return Ok(workspace_rejected_result(
                contract,
                &contract_hash,
                ChangeFailureCategory::Input,
                "contract validation failed",
                StdBTreeSet::new(),
                StdBTreeSet::new(),
            ));
        }
        if !contract
            .command_permissions
            .iter()
            .any(|command| command.purpose == CommandPurpose::Compile)
            || !contract
                .command_permissions
                .iter()
                .any(|command| command.purpose == CommandPurpose::Test)
        {
            return Ok(workspace_rejected_result(
                contract,
                &contract_hash,
                ChangeFailureCategory::Tooling,
                "compile and test command permissions are required before merge",
                StdBTreeSet::new(),
                StdBTreeSet::new(),
            ));
        }

        let target_root = self.workspace_root.join("target");
        let isolated_root = self
            .workspace_root
            .join("isolated")
            .join(format!("{}_attempt_{attempt}", task.task_id));
        let backup_root = self
            .workspace_root
            .join("backups")
            .join(format!("{}_attempt_{attempt}", task.task_id));
        if isolated_root.exists() {
            fs::remove_dir_all(&isolated_root)?;
        }
        fs::create_dir_all(&target_root)?;
        fs::create_dir_all(&isolated_root)?;
        copy_workspace_tree(&target_root, &isolated_root)?;

        for operation in &contract.operations {
            if let Err(error) = validate_operation_expectation(&target_root, operation) {
                return Ok(workspace_rejected_result(
                    contract,
                    &contract_hash,
                    ChangeFailureCategory::Conflict,
                    error.to_string(),
                    StdBTreeSet::new(),
                    StdBTreeSet::new(),
                ));
            }
            apply_workspace_operation(&isolated_root, operation)?;
        }

        let mut agent_changed_paths = StdBTreeSet::new();
        for operation in &contract.operations {
            collect_operation_paths(operation, &mut agent_changed_paths);
        }
        let build_output_changed_paths = write_compile_smoke_evidence(&isolated_root, contract)?;
        let trusted_test_hashes = contract
            .trusted_tests
            .iter()
            .map(|test| (test.test_id.clone(), test.baseline_sha256.clone()))
            .collect::<StdBTreeMap<_, _>>();
        let resulting_tree_hash = hash_workspace_tree(&isolated_root)?;
        replace_workspace_tree(&target_root, &isolated_root, &backup_root)?;

        Ok(WorkspaceTransactionResult {
            schema_version: WORKSPACE_CHANGE_SET_SCHEMA_VERSION.to_string(),
            change_set_id: contract.change_set_id.clone(),
            contract_sha256: contract_hash,
            base_tree_hash: contract.base_tree_hash.clone(),
            outcome: ChangeOutcome::Committed,
            failure_category: None,
            side_effect_state: SideEffectState::Committed,
            stage: "isolated_workspace_snapshot_merge_compile_smoke".to_string(),
            resulting_tree_hash: Some(resulting_tree_hash),
            agent_changed_paths,
            trusted_tool_changed_paths: StdBTreeSet::new(),
            build_output_changed_paths,
            trusted_test_hashes,
            evidence: vec![ChangeEvidence::from_bytes(
                "filesystem_workspace_agent",
                "step11",
                EvidenceStatus::Observed,
                format!("{}:{attempt}", task.task_id).as_bytes(),
            )],
        })
    }
}

fn workspace_rejected_result(
    contract: &adm_new_change_kernel::WorkspaceChangeSet,
    contract_hash: &str,
    category: ChangeFailureCategory,
    reason: impl AsRef<str>,
    agent_changed_paths: StdBTreeSet<WorkspaceRelativePath>,
    build_output_changed_paths: StdBTreeSet<WorkspaceRelativePath>,
) -> WorkspaceTransactionResult {
    WorkspaceTransactionResult {
        schema_version: WORKSPACE_CHANGE_SET_SCHEMA_VERSION.to_string(),
        change_set_id: contract.change_set_id.clone(),
        contract_sha256: contract_hash.to_string(),
        base_tree_hash: contract.base_tree_hash.clone(),
        outcome: ChangeOutcome::Rejected,
        failure_category: Some(category),
        side_effect_state: SideEffectState::None,
        stage: "isolated_workspace_rejected".to_string(),
        resulting_tree_hash: None,
        agent_changed_paths,
        trusted_tool_changed_paths: StdBTreeSet::new(),
        build_output_changed_paths,
        trusted_test_hashes: contract
            .trusted_tests
            .iter()
            .map(|test| (test.test_id.clone(), test.baseline_sha256.clone()))
            .collect(),
        evidence: vec![ChangeEvidence::from_bytes(
            "filesystem_workspace_agent_rejection",
            "step11",
            EvidenceStatus::Observed,
            reason.as_ref().as_bytes(),
        )],
    }
}

fn validate_operation_expectation(root: &Path, operation: &WorkspaceOperation) -> AdmResult<()> {
    match operation {
        WorkspaceOperation::WriteFile { path, expected, .. } => {
            validate_file_expectation(&workspace_path(root, path), expected)
        }
        WorkspaceOperation::DeleteFile {
            path,
            expected_sha256,
        } => validate_file_hash(&workspace_path(root, path), expected_sha256),
        WorkspaceOperation::RenameFile {
            from,
            to,
            expected_source_sha256,
            expected_target,
        } => {
            validate_file_hash(&workspace_path(root, from), expected_source_sha256)?;
            validate_file_expectation(&workspace_path(root, to), expected_target)
        }
    }
}

fn validate_file_expectation(path: &Path, expected: &WorkspaceFileExpectation) -> AdmResult<()> {
    match expected {
        WorkspaceFileExpectation::Missing if path.exists() => Err(AdmError::new(format!(
            "expected missing file already exists: {}",
            path.display()
        ))),
        WorkspaceFileExpectation::Missing => Ok(()),
        WorkspaceFileExpectation::Sha256 { value } => validate_file_hash(path, value),
    }
}

fn validate_file_hash(path: &Path, expected_sha256: &str) -> AdmResult<()> {
    let actual = sha256_hex(&fs::read(path)?);
    if actual == expected_sha256 {
        Ok(())
    } else {
        Err(AdmError::new(format!(
            "file hash mismatch for {}",
            path.display()
        )))
    }
}

fn apply_workspace_operation(root: &Path, operation: &WorkspaceOperation) -> AdmResult<()> {
    match operation {
        WorkspaceOperation::WriteFile { path, payload, .. } => {
            let path = workspace_path(root, path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, payload_bytes(payload))?;
        }
        WorkspaceOperation::DeleteFile { path, .. } => {
            let path = workspace_path(root, path);
            if path.exists() {
                fs::remove_file(path)?;
            }
        }
        WorkspaceOperation::RenameFile { from, to, .. } => {
            let from = workspace_path(root, from);
            let to = workspace_path(root, to);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(from, to)?;
        }
    }
    Ok(())
}

fn payload_bytes(payload: &WorkspaceFilePayload) -> Vec<u8> {
    match payload {
        WorkspaceFilePayload::Utf8 { content, .. } => content.as_bytes().to_vec(),
        WorkspaceFilePayload::Binary { bytes, .. } => bytes.clone(),
    }
}

fn collect_operation_paths(
    operation: &WorkspaceOperation,
    paths: &mut StdBTreeSet<WorkspaceRelativePath>,
) {
    match operation {
        WorkspaceOperation::WriteFile { path, .. }
        | WorkspaceOperation::DeleteFile { path, .. } => {
            paths.insert(path.clone());
        }
        WorkspaceOperation::RenameFile { from, to, .. } => {
            paths.insert(from.clone());
            paths.insert(to.clone());
        }
    }
}

fn write_compile_smoke_evidence(
    target_root: &Path,
    contract: &adm_new_change_kernel::WorkspaceChangeSet,
) -> AdmResult<StdBTreeSet<WorkspaceRelativePath>> {
    let mut changed = StdBTreeSet::new();
    let Some(scope) = contract.build_output_paths.iter().next() else {
        return Ok(changed);
    };
    let evidence_path =
        WorkspaceRelativePath::parse(format!("{}/compile_smoke.json", scope.as_str()))
            .map_err(|error| AdmError::new(error.to_string()))?;
    let absolute = workspace_path(target_root, &evidence_path);
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        absolute,
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": "step11_compile_smoke_evidence.v1",
            "status": "not_executed",
            "mode": "local_filesystem_contract_fallback",
            "verificationRequired": true,
            "message": "local fallback materialized the declared WorkspaceChangeSet but did not spawn build or smoke commands",
            "commandPermissionCount": contract.command_permissions.len(),
        }))
        .map_err(|error| {
            AdmError::new(format!("failed to encode compile smoke evidence: {error}"))
        })?,
    )?;
    changed.insert(evidence_path);
    Ok(changed)
}

fn workspace_path(root: &Path, path: &WorkspaceRelativePath) -> PathBuf {
    root.join(path.as_str())
}

fn copy_workspace_tree(source: &Path, destination: &Path) -> AdmResult<()> {
    if !source.exists() {
        return Ok(());
    }
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_workspace_tree(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &destination_path)?;
        } else {
            return Err(AdmError::new(format!(
                "unsupported workspace entry type while copying isolated tree: {}",
                source_path.display()
            )));
        }
    }
    Ok(())
}

fn replace_workspace_tree(target: &Path, replacement: &Path, backup: &Path) -> AdmResult<()> {
    if backup.exists() {
        fs::remove_dir_all(backup)?;
    }
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }

    let target_existed = target.exists();
    if target_existed {
        fs::rename(target, backup)?;
    }
    match fs::rename(replacement, target) {
        Ok(()) => {
            if backup.exists() {
                fs::remove_dir_all(backup)?;
            }
            Ok(())
        }
        Err(error) => {
            if target_existed {
                if target.exists() {
                    let _ = fs::remove_dir_all(target);
                }
                if let Err(restore_error) = fs::rename(backup, target) {
                    return Err(AdmError::new(format!(
                        "workspace commit failed ({error}) and rollback failed ({restore_error})"
                    )));
                }
            }
            Err(AdmError::new(format!(
                "workspace commit failed before target replacement completed: {error}"
            )))
        }
    }
}

fn hash_workspace_tree(root: &Path) -> AdmResult<String> {
    let mut files = StdBTreeMap::new();
    collect_workspace_hashes(root, root, &mut files)?;
    let payload = serde_json::to_vec(&files)
        .map_err(|error| AdmError::new(format!("failed to encode workspace tree hash: {error}")))?;
    Ok(sha256_hex(&payload))
}

fn collect_workspace_hashes(
    root: &Path,
    current: &Path,
    files: &mut StdBTreeMap<String, String>,
) -> AdmResult<()> {
    if !current.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_workspace_hashes(root, &path, files)?;
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(relative, sha256_hex(&fs::read(path)?));
        }
    }
    Ok(())
}

fn bool_at_path(value: &Value, path: &[&str]) -> bool {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(*key) else {
            return false;
        };
        current = next;
    }
    current.as_bool().unwrap_or(false)
}

fn parse_game_spec_file(path: &Path) -> AdmResult<GameSpec> {
    let input = fs::read_to_string(path)?;
    parse_game_spec(&input).map_err(|error| AdmError::new(error.to_string()))
}

fn game_spec_content_hash(spec: &GameSpec) -> AdmResult<String> {
    canonicalize_game_spec(spec)
        .map(|canonical| canonical.content_hash)
        .map_err(|error| AdmError::new(format!("GameSpec hash failed: {error}")))
}

fn validate_frozen_step08_10_hashes(
    compiled: &step08_10_v2::Step08_10Compilation,
    expected_source_hash: &str,
) -> AdmResult<()> {
    let observed = [
        (
            "runtime_architecture.source_game_spec_hash",
            compiled.architecture.source_game_spec_hash.as_str(),
        ),
        (
            "frozen_asset_manifest.source_game_spec_hash",
            compiled.asset_manifest.source_game_spec_hash.as_str(),
        ),
        (
            "trusted_task_graph.source_game_spec_hash",
            compiled.task_graph.source_game_spec_hash.as_str(),
        ),
    ];
    for (field, value) in observed {
        if value != expected_source_hash {
            return Err(AdmError::new(format!(
                "frozen Step10 source hash mismatch at {field}; rerun Step10 before executing Step11/12"
            )));
        }
    }
    Ok(())
}

fn read_json_typed<T: DeserializeOwned>(path: &Path) -> AdmResult<T> {
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| AdmError::new(format!("failed to parse {}: {error}", path.display())))
}

fn json_value<T: Serialize>(value: T) -> AdmResult<Value> {
    serde_json::to_value(value)
        .map_err(|error| AdmError::new(format!("failed to encode stage output JSON: {error}")))
}

fn game_spec_v2_validation_report(
    step_number: u32,
    locale: ArtifactLocale,
    raw_status: String,
    output: Value,
    blockers: Vec<String>,
) -> Value {
    let normalized = raw_status.to_ascii_lowercase();
    let status = if !blockers.is_empty() || normalized.contains("blocked") {
        "blocked"
    } else if normalized.contains("waiting") {
        "waiting_confirmation"
    } else if normalized.contains("stopped") {
        "stopped"
    } else if normalized.contains("success") || normalized.contains("passed") {
        "success"
    } else if normalized.contains("correction")
        || normalized.contains("failed")
        || normalized.contains("manual")
    {
        "blocked"
    } else {
        "failed"
    };
    let blocking_issues = blockers
        .iter()
        .map(|message| {
            json!({
                "code": "GAME_SPEC_V2_STAGE_BLOCKER",
                "severity": "blocker",
                "message": message,
                "return_target": format!("stage_{step_number:02}"),
            })
        })
        .collect::<Vec<_>>();
    let blocking_issue_count = blocking_issues.len();
    json!({
        "schema_version": "game_spec_v2_product_stage_report.v1",
        "stage": step_number,
        "stage_id": format!("{step_number:02}"),
        "status": status,
        "raw_status": raw_status,
        "valid": status == "success" || status == "waiting_confirmation",
        "artifact_locale": locale,
        "content_exists": true,
        "game_spec_v2": true,
        "blocking_issues": blocking_issues,
        "review_items_count": 0,
        "ai_review_status": "not_called",
        "traceability_valid": status == "success" || status == "waiting_confirmation",
        "business_quality": {
            "status": status,
            "artifact_locale": locale,
            "content_exists": true,
            "blocking_issues": blocking_issue_count,
            "review_items_count": 0,
            "ai_review_status": "not_called",
            "traceability_valid": status == "success" || status == "waiting_confirmation",
            "game_spec_v2": true,
        },
        "outputs": output,
    })
}

fn non_empty_path(path: &Path) -> Option<PathBuf> {
    (!path.as_os_str().is_empty()).then(|| path.to_path_buf())
}

fn parse_json_bytes_with_bom<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    let text = std::str::from_utf8(bytes).ok()?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

fn product_required_upstream_stages(step_number: u32) -> BTreeSet<u32> {
    let registry = crate::default_development_registry();
    let by_id = registry
        .stages
        .iter()
        .map(|stage| (stage.stage_id.as_str(), stage))
        .collect::<std::collections::BTreeMap<_, _>>();
    let current_id = format!("{step_number:02}");
    let mut pending = by_id
        .get(current_id.as_str())
        .map(|stage| stage.requires.clone())
        .unwrap_or_default();
    let mut required = BTreeSet::new();
    while let Some(stage_id) = pending.pop() {
        let Some(stage) = by_id.get(stage_id.as_str()) else {
            continue;
        };
        let Some(number) = stage.number.or_else(|| stage.stage_id.parse().ok()) else {
            continue;
        };
        if required.insert(number) {
            pending.extend(stage.requires.iter().cloned());
        }
    }
    required
}

fn protocol_root(root: &Path, design_data_dir: Option<&Path>) -> Option<PathBuf> {
    let candidate = match design_data_dir {
        Some(design_data_dir) => design_data_dir
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)?,
        None => root.to_path_buf(),
    };
    (candidate
        .join("pipeline/artifact_layer/registry.json")
        .is_file()
        && candidate.join("knowledge/schemas").is_dir())
    .then_some(candidate)
}

fn registered_output_path(artifact_root: &Path, registered_path: &str) -> Option<PathBuf> {
    let normalized = registered_path.replace('\\', "/");
    let (base, relative) = if let Some(relative) = normalized.strip_prefix("outputs/artifacts/") {
        (artifact_root, relative)
    } else if let Some(relative) = normalized.strip_prefix("outputs/") {
        (artifact_root.parent()?, relative)
    } else {
        return None;
    };
    safe_join(base, relative)
}

fn registered_schema_path(protocol_root: &Path, registered_path: &str) -> Option<PathBuf> {
    let normalized = registered_path.replace('\\', "/");
    if !normalized.starts_with("knowledge/schemas/") {
        return None;
    }
    safe_join(protocol_root, &normalized)
}

fn safe_join(base: &Path, relative: &str) -> Option<PathBuf> {
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(base.join(relative))
}

fn return_target_from_contract_path(path: &str, fallback_step: u32) -> String {
    path.replace('\\', "/")
        .split('/')
        .find_map(|part| {
            let step = part.strip_prefix("stage_")?;
            (step.len() == 2 && step.chars().all(|character| character.is_ascii_digit()))
                .then(|| format!("stage_{step}"))
        })
        .unwrap_or_else(|| format!("stage_{fallback_step:02}"))
}

#[allow(clippy::too_many_arguments)]
fn protocol_gate_issue(
    code: &str,
    artifact: &str,
    schema: &str,
    return_target: &str,
    locale: ArtifactLocale,
    zh_cn: &str,
    en_us: &str,
) -> Value {
    json!({
        "code": code,
        "severity": "blocker",
        "message": if locale == ArtifactLocale::ZhCn { zh_cn } else { en_us },
        "artifact": artifact,
        "schema": schema,
        "return_target": return_target,
    })
}

fn localized_protocol_message(locale: ArtifactLocale, zh_cn: &str, en_us: &str) -> String {
    if locale == ArtifactLocale::ZhCn {
        zh_cn.to_string()
    } else {
        en_us.to_string()
    }
}

fn validate_step07_records(
    records: &[WorkUnitJournalRecord],
    stage_id: &str,
) -> AdmResult<BTreeSet<String>> {
    let mut known_cache_keys = BTreeSet::new();
    for record in records {
        let expected_unit_id = format!("{stage_id}:art:{}", record.task_id);
        if record.stage_id != stage_id
            || !record.task_id.starts_with("image:")
            || record.unit_id != expected_unit_id
        {
            return Err(AdmError::new(
                "Step07 work unit journal contains an invalid image-unit identity",
            ));
        }
        record
            .updated_at
            .strip_prefix("unix:")
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| AdmError::new("Step07 work unit journal timestamp is invalid"))?;
        known_cache_keys.insert(style_image_cache_key(&record.idempotency_key));
    }
    Ok(known_cache_keys)
}

fn step07_cache_has_orphan_entries(cache_root: &Path, known_cache_keys: &BTreeSet<String>) -> bool {
    match cache_root.try_exists() {
        Ok(false) => return false,
        Err(_) => return true,
        Ok(true) => {}
    }
    let entries = match std::fs::read_dir(cache_root) {
        Ok(entries) => entries,
        Err(_) => return true,
    };
    for entry in entries {
        let Ok(entry) = entry else {
            return true;
        };
        let Ok(file_type) = entry.file_type() else {
            return true;
        };
        if !file_type.is_file() {
            return true;
        }
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            return true;
        };
        let Some(cache_key) = file_name.strip_suffix(".png") else {
            return true;
        };
        if !known_cache_keys.contains(cache_key) {
            return true;
        }
    }
    false
}

impl StageExecutor for ProductPipelineExecutor {
    fn execute(&self, spec: &StageSpec, _context: &StageContextModel) -> PipelineStageResult {
        let step_number = spec.number.or_else(|| spec.stage_id.parse().ok());
        match step_number {
            Some(step @ 0..=14) => self.execute_step(step),
            _ => failed_stage_result_with_locale(
                0,
                format!("unsupported product pipeline stage: {}", spec.stage_id),
                self.artifact_locale,
            ),
        }
    }

    fn skip_manual_gate(
        &self,
        spec: &StageSpec,
        _context: &StageContextModel,
        mut result: PipelineStageResult,
    ) -> PipelineStageResult {
        if spec.stage_id != "07" || result.status != StageStatus::WaitingConfirmation {
            result.status = StageStatus::Skipped;
            result
                .outputs
                .insert("manual_gate_skipped".to_string(), Value::Bool(true));
            result
                .warnings
                .push(if self.artifact_locale == ArtifactLocale::ZhCn {
                    format!("已根据运行请求跳过人工门禁 {}", spec.stage_id)
                } else {
                    format!("manual gate {} skipped by request", spec.stage_id)
                });
            result.message = if self.artifact_locale == ArtifactLocale::ZhCn {
                format!("人工门禁 {} 已跳过", spec.stage_id)
            } else {
                format!("manual gate {} skipped", spec.stage_id)
            };
            return result;
        }
        let selected_style_id = result
            .outputs
            .get("recommended_style_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| recommended_style_id(result.outputs.get("style_options")))
            .unwrap_or_default();
        if selected_style_id.is_empty() {
            result.status = StageStatus::Failed;
            result.errors.push(
                if self.artifact_locale == ArtifactLocale::ZhCn {
                    "步骤 07 没有可自动确认的风格选项"
                } else {
                    "Step07 has no style option to auto-confirm"
                }
                .to_string(),
            );
            return result;
        }
        if let Err(error) = self.confirm_style(
            &selected_style_id,
            if self.artifact_locale == ArtifactLocale::ZhCn {
                "运行请求已允许跳过人工门禁，因此自动批准。"
            } else {
                "Automatically approved because skip_manual_gates was requested."
            },
        ) {
            result.status = StageStatus::Failed;
            result.errors.push(error.to_string());
            return result;
        }
        let mut confirmed = self.execute_step(7);
        confirmed
            .outputs
            .insert("manual_gate_skipped".to_string(), Value::Bool(true));
        confirmed.outputs.insert(
            "auto_confirmed_style_id".to_string(),
            Value::String(selected_style_id.clone()),
        );
        confirmed
            .warnings
            .push(if self.artifact_locale == ArtifactLocale::ZhCn {
                format!("人工门禁 07 已自动确认推荐风格 {selected_style_id}")
            } else {
                format!("manual gate 07 auto-confirmed recommended style {selected_style_id}")
            });
        confirmed
    }
}

fn recommended_style_id(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_array).and_then(|options| {
        options
            .iter()
            .find(|option| {
                option
                    .get("recommended")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .or_else(|| options.first())
            .and_then(|option| {
                ["style_id", "option_id", "id"]
                    .iter()
                    .find_map(|key| option.get(*key).and_then(Value::as_str))
            })
            .map(str::to_string)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::parse_design_text;
    use crate::{PipelineService, default_development_registry};
    use adm_new_change_kernel::{
        CommandPermission, TrustedTestContract, WorkspaceChangeSet, WorkspaceResourceBudget,
    };
    use adm_new_contracts::pipeline::{
        PipelineCheckpoint, PipelineRunState, PipelineUnitCheckpoint, PipelineUnitStatus,
    };
    use adm_new_foundation::{new_stable_id, paths::SourceProjectRoot, sha256_hex};
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    use crate::work_units::{WorkUnitJournalPhase, WorkUnitKind, WorkUnitRequest};

    #[test]
    fn product_executor_never_reads_design_data_from_its_parent_directory() {
        let parent = temp_root("parent_design_data_must_be_ignored");
        let root = parent.join("renamed child");
        fs::create_dir_all(parent.join("knowledge/design_data/domains")).unwrap();
        fs::create_dir_all(&root).unwrap();
        let executor = ProductPipelineExecutor::new(&root, "session_a").unwrap();

        let error = executor.load_design_engine().unwrap_err();
        assert!(
            error
                .to_string()
                .contains("standalone design data is missing")
        );
        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn product_executor_writes_real_step00_through_step02_artifacts() {
        let root = temp_root("product_pipeline_real");
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests();
        let mut project = ProjectState::empty();
        project.project_name = "Pipeline Test Game".to_string();
        project.profile = BTreeMap::from([
            ("targetScale".to_string(), json!("indie")),
            ("businessModel".to_string(), json!("premium")),
            ("platformScope".to_string(), json!("pc")),
        ]);
        project.gameplay_systems.selected = vec![
            "combat".to_string(),
            "progression".to_string(),
            "exploration".to_string(),
            "inventory".to_string(),
            "quests".to_string(),
        ];
        executor.prepare_project_source(&project).unwrap();

        let service = PipelineService::new(default_development_registry()).unwrap();
        let mut state = empty_state();
        let report = service
            .run_range(&mut state, "00", "02", &executor)
            .unwrap();

        assert_eq!(report.executed_stage_ids, vec!["00", "01", "02"]);
        assert_ne!(state.stages["00"].status, StageStatus::Failed);
        assert_ne!(state.stages["01"].status, StageStatus::Failed);
        assert_ne!(state.stages["02"].status, StageStatus::Failed);
        assert!(
            executor
                .artifact_root()
                .join("stage_00/concept_profile.json")
                .is_file()
        );
        assert!(
            executor
                .artifact_root()
                .join("stage_01/gameplay_framework.json")
                .is_file()
        );
        assert!(
            executor
                .artifact_root()
                .join("stage_02/design_freeze_report.json")
                .is_file()
        );
        let records = state.stages["00"]
            .result
            .as_ref()
            .unwrap()
            .outputs
            .get("artifact_records")
            .and_then(Value::as_array)
            .unwrap();
        assert!(records.iter().any(|record| {
            record["relative_path"] == "stage_00/concept_profile.json"
                && record["content_type"] == "application/json"
                && !record["content_preview"]
                    .as_str()
                    .unwrap_or_default()
                    .is_empty()
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn product_executor_preserves_en_us_through_step00_02_reports_and_indexes() {
        let root = temp_root("product_pipeline_en_us");
        let repository_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
            .into_path();
        let executor = ProductPipelineExecutor::with_design_data_dir(
            &root,
            "session_a",
            repository_root.join("knowledge/design_data"),
        )
        .unwrap()
        .with_artifact_locale(ArtifactLocale::EnUs);
        let mut project = ProjectState::empty();
        project.project_name = "Pipeline English Test".to_string();
        project.profile = BTreeMap::from([
            ("targetScale".to_string(), json!("indie")),
            ("businessModel".to_string(), json!("premium")),
            ("platformScope".to_string(), json!("pc")),
        ]);
        project.gameplay_systems.selected = vec![
            "combat".to_string(),
            "progression".to_string(),
            "exploration".to_string(),
            "inventory".to_string(),
            "quests".to_string(),
        ];
        executor.prepare_project_source(&project).unwrap();

        let service = PipelineService::new(default_development_registry()).unwrap();
        let mut state = empty_state();
        let report = service
            .run_range(&mut state, "00", "02", &executor)
            .unwrap();

        assert_eq!(report.executed_stage_ids, vec!["00", "01", "02"]);
        for (stage, file) in [
            (0, "intent_interpretation_contract.json"),
            (1, "gameplay_concretization_contract.json"),
            (2, "project_dna_contract.json"),
            (2, "design_ai_review_report.json"),
        ] {
            let artifact = read_json(
                &executor
                    .artifact_root()
                    .join(format!("stage_{stage:02}/{file}")),
                json!({}),
            );
            assert_eq!(artifact["artifact_locale"], "en-US", "{file}");
        }
        for stage in 0..=2 {
            for file in ["validation_report.json", "artifact_index.json"] {
                let artifact = read_json(
                    &executor
                        .artifact_root()
                        .join(format!("stage_{stage:02}/{file}")),
                    json!({}),
                );
                assert_eq!(artifact["artifact_locale"], "en-US", "{file}");
            }
        }
        let dna = read_json(
            &executor
                .artifact_root()
                .join("stage_02/project_dna_contract.json"),
            json!({}),
        );
        assert_eq!(dna["contract_display_name"], "Frozen Project DNA Contract");
        assert!(
            !dna["contract_display_name"]
                .as_str()
                .unwrap()
                .chars()
                .any(|character| ('\u{3400}'..='\u{9fff}').contains(&character))
        );
        for stage in 0..=2 {
            let protocol = read_json(
                &executor
                    .artifact_root()
                    .join(format!("stage_{stage:02}/registry_protocol_report.json")),
                json!({}),
            );
            assert_eq!(protocol["status"], "passed");
            assert_eq!(protocol["artifact_locale"], "en-US");
            assert!(protocol["blocking_issues"].as_array().unwrap().is_empty());
        }
        fs::remove_file(
            executor
                .artifact_root()
                .join("stage_00/intent_interpretation_contract.json"),
        )
        .unwrap();
        let failed_gate = executor
            .validate_registered_stage_artifacts(0, executor.protocol_root.as_deref().unwrap())
            .unwrap();
        assert_eq!(failed_gate["status"], "blocked");
        assert!(
            failed_gate["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| {
                    issue["code"] == "REGISTERED_ARTIFACT_MISSING"
                        && issue["return_target"] == "stage_00"
                })
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn product_executor_preserves_source_failure_and_error_artifacts() {
        let root = temp_root("product_pipeline_failure");
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests();
        let registry = default_development_registry();
        let result = executor.execute(&registry.stages[0], &empty_context("00"));

        assert_eq!(result.status, StageStatus::Failed);
        assert!(!result.errors.is_empty());
        assert!(result.errors.iter().any(|error| error.contains("概念源包")));
        let error_path = executor
            .artifact_root()
            .join("stage_00/design_source_error.json");
        assert!(error_path.is_file());
        let error_artifact = read_json(&error_path, json!({}));
        assert_eq!(error_artifact["artifact_locale"], "zh-CN");
        assert_eq!(error_artifact["message"], "步骤00未找到已提交的概念源包。");
        assert!(
            result.outputs["artifact_records"]
                .as_array()
                .unwrap()
                .iter()
                .any(|record| record["name"] == "design_source_error.json")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn required_protocol_gate_blocks_instead_of_silently_disabling_validation() {
        let root = temp_root("product_pipeline_protocol_required");
        let executor = ProductPipelineExecutor::new(&root, "session_a").unwrap();
        assert!(!executor.protocol_resources_ready());
        assert!(
            executor
                .prepare_project_source(&ProjectState::empty())
                .is_err()
        );
        let registry = default_development_registry();

        let result = executor.execute(&registry.stages[0], &empty_context("00"));

        assert_eq!(result.status, StageStatus::Blocked);
        assert_eq!(
            result.outputs["blocking_issues"][0]["code"],
            "PIPELINE_PROTOCOL_RESOURCES_UNAVAILABLE"
        );
        assert!(
            executor
                .artifact_root()
                .join("stage_00/registry_protocol_report.json")
                .is_file()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn design_data_and_protocol_readiness_are_decoupled_for_product_ui_startup() {
        let root = temp_root("product_pipeline_protocol_decoupled");
        let design_data = root.join("packaged/knowledge/design_data");
        fs::create_dir_all(design_data.join("domains")).unwrap();

        let executor =
            ProductPipelineExecutor::with_design_data_dir(&root, "session_a", &design_data)
                .unwrap()
                .require_protocol_gate();

        assert!(!executor.protocol_resources_ready());
        assert!(
            executor
                .prepare_project_source(&ProjectState::empty())
                .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_design_package_never_falls_back_to_stale_persistent_protocol_files() {
        let root = temp_root("product_pipeline_protocol_package_scope");
        fs::create_dir_all(root.join("pipeline/artifact_layer")).unwrap();
        fs::create_dir_all(root.join("knowledge/schemas")).unwrap();
        fs::write(
            root.join("pipeline/artifact_layer/registry.json"),
            "{\"version\":1,\"artifacts\":[]}",
        )
        .unwrap();
        let design_data = root.join("separate-package/knowledge/design_data");
        fs::create_dir_all(design_data.join("domains")).unwrap();

        let executor =
            ProductPipelineExecutor::with_design_data_dir(&root, "session_a", &design_data)
                .unwrap();

        assert!(!executor.protocol_resources_ready());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn root_based_executor_discovers_protocol_resources_without_an_explicit_design_package() {
        let root = temp_root("product_pipeline_protocol_root_scope");
        fs::create_dir_all(root.join("pipeline/artifact_layer")).unwrap();
        fs::create_dir_all(root.join("knowledge/schemas")).unwrap();
        fs::write(
            root.join("pipeline/artifact_layer/registry.json"),
            "{\"version\":1,\"artifacts\":[]}",
        )
        .unwrap();

        let executor = ProductPipelineExecutor::new(&root, "session_a").unwrap();

        assert!(executor.protocol_resources_ready());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn upstream_protocol_json_parser_accepts_utf8_bom() {
        let document =
            parse_json_bytes_with_bom::<Value>(b"\xEF\xBB\xBF{\"artifact_locale\":\"zh-CN\"}")
                .unwrap();
        assert_eq!(document["artifact_locale"], "zh-CN");
    }

    #[test]
    fn unreadable_registry_keeps_environment_return_target() {
        let root = temp_root("product_pipeline_bad_registry_target");
        let design_data = root.join("knowledge/design_data");
        fs::create_dir_all(design_data.join("domains")).unwrap();
        fs::create_dir_all(root.join("knowledge/schemas")).unwrap();
        fs::create_dir_all(root.join("pipeline/artifact_layer")).unwrap();
        fs::write(
            root.join("pipeline/artifact_layer/registry.json"),
            "not-json",
        )
        .unwrap();
        let executor =
            ProductPipelineExecutor::with_design_data_dir(&root, "session_a", &design_data)
                .unwrap();
        let service = GenerationService::new(&root, "session_a").unwrap();

        let result = executor
            .upstream_locale_mismatch_result(1, &service)
            .unwrap();

        assert_eq!(result.status, StageStatus::Blocked);
        assert_eq!(result.outputs["return_target"], "environment_configuration");
        assert_eq!(
            result.outputs["blocking_issues"][0]["code"],
            "UPSTREAM_PROTOCOL_REGISTRY_UNREADABLE"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn product_executor_blocks_partial_run_with_mixed_upstream_artifact_locale() {
        let root = temp_root("product_pipeline_locale_mismatch");
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests()
            .with_artifact_locale(ArtifactLocale::EnUs);
        write_json(
            &executor
                .artifact_root()
                .join("stage_03/validation_report.json"),
            &json!({
                "status": "success",
                "valid": true,
                "artifact_locale": "zh-CN"
            }),
        )
        .unwrap();
        let registry = default_development_registry();
        let step08 = registry
            .stages
            .iter()
            .find(|stage| stage.stage_id == "08")
            .unwrap();

        let result = executor.execute(step08, &empty_context("08"));

        assert_eq!(result.status, StageStatus::Blocked);
        assert_eq!(
            result.outputs["blocking_issues"][0]["code"],
            "UPSTREAM_ARTIFACT_LOCALE_MISMATCH"
        );
        assert_eq!(
            result.outputs["blocking_issues"][0]["return_target"],
            "stage_03"
        );
        assert!(result.message.starts_with("Step08 was blocked"));
        let locale_report = read_json(
            &executor
                .artifact_root()
                .join("stage_08/locale_compatibility_report.json"),
            json!({}),
        );
        assert_eq!(locale_report["artifact_locale"], "en-US");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn game_spec_v2_product_step14_derives_gate_evidence_from_release_sources() {
        let root = temp_root("product_pipeline_v2_step14_sources");
        fs::create_dir_all(root.join("settings")).unwrap();
        fs::write(
            root.join("settings/project_settings.json"),
            serde_json::to_vec_pretty(&json!({"game_spec_v2": {"enabled": true}})).unwrap(),
        )
        .unwrap();
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests();
        let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
            .into_path();
        let spec_path =
            source_root.join("testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json");
        let spec = parse_game_spec_file(&spec_path).unwrap();
        let stage06 = executor.artifact_root().join("stage_06");
        fs::create_dir_all(&stage06).unwrap();
        fs::copy(&spec_path, stage06.join("r1_frozen_game_spec.json")).unwrap();
        let stage13 = executor.artifact_root().join("stage_13");
        fs::create_dir_all(&stage13).unwrap();
        io::write_json_serializable(
            &stage13.join("step13_acceptance_output.json"),
            &passed_step13_for_product_step14_test(&spec),
        )
        .unwrap();
        let stage14 = executor.artifact_root().join("stage_14");
        fs::create_dir_all(&stage14).unwrap();
        let evidence_id = write_product_step14_standalone_release_evidence(&root);
        io::write_json_serializable(
            &stage14.join(step14_v2::R1_PIPELINE_GATE_EVIDENCE_FILE),
            &step14_v2::R1PipelineGateEvidence::all_passed_for_tests(),
        )
        .unwrap();
        io::write_json_serializable(
            &stage14.join(step14_v2::R1_USER_PLAYTEST_SIGNATURE_FILE),
            &step14_v2::R1UserPlaytestSignature::manual_for_tests(&evidence_id),
        )
        .unwrap();
        let registry = default_development_registry();
        let step14 = registry
            .stages
            .iter()
            .find(|stage| stage.stage_id == "14")
            .unwrap();

        let result = executor.execute(step14, &empty_context("14"));

        assert_eq!(result.status, StageStatus::Success);
        assert!(stage14.join("r1_gate_evidence.json").is_file());
        assert!(
            stage14
                .join(step14_v2::R1_GATE_EVIDENCE_SOURCE_REPORT_FILE)
                .is_file()
        );
        let derived = read_json(
            &stage14.join(step14_v2::R1_GATE_EVIDENCE_SOURCE_REPORT_FILE),
            json!({}),
        );
        assert!(derived["blockers"].as_array().unwrap().is_empty());
        assert_eq!(
            derived["standaloneReleaseEvidencePath"],
            root.join("gates/standalone-release-evidence.json")
                .to_string_lossy()
                .replace('\\', "/")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn product_executor_does_not_normalize_a_missing_upstream_locale_marker() {
        let root = temp_root("product_pipeline_locale_marker_missing");
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests();
        write_json(
            &executor
                .artifact_root()
                .join("stage_03/validation_report.json"),
            &json!({"status": "success", "valid": true}),
        )
        .unwrap();
        let registry = default_development_registry();
        let step08 = registry
            .stages
            .iter()
            .find(|stage| stage.stage_id == "08")
            .unwrap();

        let result = executor.execute(step08, &empty_context("08"));

        assert_eq!(result.status, StageStatus::Blocked);
        assert_eq!(
            result.outputs["blocking_issues"][0]["code"],
            "UPSTREAM_ARTIFACT_LOCALE_MISSING"
        );
        assert!(result.outputs["blocking_issues"][0]["actual_locale"].is_null());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn product_upstream_locale_scope_follows_the_dependency_closure() {
        assert_eq!(
            product_required_upstream_stages(4),
            BTreeSet::from([0, 1, 2])
        );
        assert_eq!(
            product_required_upstream_stages(9),
            BTreeSet::from([0, 1, 2, 4, 6, 7])
        );
        let root = temp_root("product_pipeline_dependency_locale_scope");
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests();
        write_json(
            &executor
                .artifact_root()
                .join("stage_03/validation_report.json"),
            &json!({"artifact_locale": "en-US"}),
        )
        .unwrap();
        write_json(
            &executor
                .artifact_root()
                .join("stage_08/validation_report.json"),
            &json!({"artifact_locale": "en-US"}),
        )
        .unwrap();
        let service = GenerationService::new(&root, "session_a").unwrap();

        assert!(
            executor
                .upstream_locale_mismatch_result(4, &service)
                .is_none()
        );
        assert!(
            executor
                .upstream_locale_mismatch_result(9, &service)
                .is_none()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn product_executor_checks_registered_upstream_artifacts_not_only_validation_report() {
        let root = temp_root("product_pipeline_registered_upstream_locale");
        let repository_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
            .into_path();
        let executor = ProductPipelineExecutor::with_design_data_dir(
            &root,
            "session_a",
            repository_root.join("knowledge/design_data"),
        )
        .unwrap();
        let mut project = ProjectState::empty();
        project.project_name = "Registered Locale Test".to_string();
        project.profile = BTreeMap::from([
            ("targetScale".to_string(), json!("indie")),
            ("businessModel".to_string(), json!("premium")),
            ("platformScope".to_string(), json!("pc")),
        ]);
        project.gameplay_systems.selected = vec![
            "combat".to_string(),
            "progression".to_string(),
            "exploration".to_string(),
            "inventory".to_string(),
            "quests".to_string(),
        ];
        executor.prepare_project_source(&project).unwrap();
        let registry = default_development_registry();
        let step00 = registry
            .stages
            .iter()
            .find(|stage| stage.stage_id == "00")
            .unwrap();
        assert_ne!(
            executor.execute(step00, &empty_context("00")).status,
            StageStatus::Failed
        );
        let artifact_path = executor
            .artifact_root()
            .join("stage_00/intent_interpretation_contract.json");
        let mut artifact = read_json(&artifact_path, json!({}));
        artifact.as_object_mut().unwrap().remove("artifact_locale");
        write_json(&artifact_path, &artifact).unwrap();
        // The old stage-level report remains zh-CN. A report-only check would
        // incorrectly allow the partial Step01 run.
        assert_eq!(
            read_json(
                &executor
                    .artifact_root()
                    .join("stage_00/validation_report.json"),
                json!({}),
            )["artifact_locale"],
            "zh-CN"
        );
        let step01 = registry
            .stages
            .iter()
            .find(|stage| stage.stage_id == "01")
            .unwrap();

        let result = executor.execute(step01, &empty_context("01"));

        assert_eq!(result.status, StageStatus::Blocked);
        assert!(
            result.outputs["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| {
                    issue["code"] == "UPSTREAM_ARTIFACT_LOCALE_MISSING"
                        && issue["artifact_path"]
                            == "outputs/artifacts/stage_00/intent_interpretation_contract.json"
                })
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn product_executor_clone_reinjects_unity_context_for_step13_resume() {
        let root = temp_root("product_pipeline_unity_context_resume");
        let unity_project_path = root.join("ConfiguredUnityProject");
        let unity_editor_path = root.join("ConfiguredUnityEditor/Unity.exe");
        fs::create_dir_all(&unity_project_path).unwrap();
        fs::create_dir_all(unity_editor_path.parent().unwrap()).unwrap();
        fs::write(&unity_editor_path, b"editor fixture").unwrap();
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests()
            .with_unity_context(&unity_project_path, &unity_editor_path);

        // Checkpoint resume may clone the executor; Step13 must receive the same
        // runtime-only context instead of writing fallback path literals.
        let resumed_executor = executor.clone();
        let generator = resumed_executor.generator_for_step(13).unwrap();
        let parsed = parse_design_text("# Unity context test\n", "context-test.md", "", None, None);
        let out_dir = resumed_executor.artifact_root().join("stage_13");
        let result = generator
            .generate(
                13,
                &parsed,
                &out_dir,
                &json!({"artifact_locale": "en-US", "status": "structured"}),
            )
            .unwrap();
        let request = read_json(&out_dir.join("unity_editor_request.json"), json!({}));

        assert_eq!(request["artifact_locale"], "en-US");
        assert_eq!(request["project_path"], "");
        assert_eq!(request["unity_editor_path"], "");
        assert_eq!(request["machine_binding"]["status"], "bound");
        assert!(
            !request["machine_binding"]["binding_id"]
                .as_str()
                .unwrap_or_default()
                .is_empty()
        );
        let persisted = serde_json::to_string(&request).unwrap();
        assert!(!persisted.contains(unity_project_path.to_string_lossy().as_ref()));
        assert!(!persisted.contains(unity_editor_path.to_string_lossy().as_ref()));
        assert!(
            !result["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|issue| matches!(
                    issue["code"].as_str(),
                    Some("STEP13_UNITY_PROJECT_PATH_MISSING" | "STEP13_UNITY_EDITOR_PATH_MISSING")
                ))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step07_unknown_whole_stage_is_prepared_for_inner_journal_reconciliation() {
        let root = temp_root("product_pipeline_step07_resume");
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests();
        let mut checkpoint = blocked_step07_checkpoint();
        let status = executor
            .reconcile_checkpoint_work_units(&mut checkpoint, "07")
            .unwrap();
        assert_eq!(status, StageWorkUnitReconcileStatus::Pending);
        assert_eq!(checkpoint.units[0].status, PipelineUnitStatus::Pending);
        assert!(!checkpoint.units[0].reconcile_required);
        assert_eq!(checkpoint.status, PipelineCheckpointStatus::Recoverable);
        assert_eq!(checkpoint.resume_policy, PipelineResumePolicy::ExplicitOnly);
        assert!(checkpoint.recovery_blocked_reason.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step07_unknown_image_side_effect_is_reconciled_before_resume_is_accepted() {
        let root = temp_root("product_pipeline_step07_immediate_reconcile");
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests();
        let (_, cache_path) = write_step07_started_record(&executor, "STYLE-01", "first", 1);
        fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        fs::write(&cache_path, b"possible provider side effect").unwrap();

        let mut checkpoint = blocked_step07_checkpoint();
        let blocked = executor
            .reconcile_checkpoint_work_units(&mut checkpoint, "07")
            .unwrap();
        assert_eq!(blocked, StageWorkUnitReconcileStatus::RecoveryBlocked);
        assert_eq!(checkpoint.units[0].status, PipelineUnitStatus::Unknown);
        assert!(checkpoint.units[0].reconcile_required);
        assert_eq!(checkpoint.status, PipelineCheckpointStatus::RecoveryBlocked);
        assert_eq!(checkpoint.resume_policy, PipelineResumePolicy::Disabled);

        fs::remove_file(cache_path).unwrap();
        let pending = executor
            .reconcile_checkpoint_work_units(&mut checkpoint, "07")
            .unwrap();
        assert_eq!(pending, StageWorkUnitReconcileStatus::Pending);
        assert_eq!(checkpoint.units[0].status, PipelineUnitStatus::Pending);
        assert!(!checkpoint.units[0].reconcile_required);
        assert_eq!(checkpoint.status, PipelineCheckpointStatus::Recoverable);
        assert_eq!(checkpoint.resume_policy, PipelineResumePolicy::ExplicitOnly);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step07_older_unknown_lineage_cannot_be_hidden_by_a_newer_safe_lineage() {
        let root = temp_root("product_pipeline_step07_all_lineages");
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests();
        let (_, old_cache_path) = write_step07_started_record(&executor, "STYLE-01", "old", 1);
        write_step07_started_record(&executor, "STYLE-01", "new", 2);
        fs::create_dir_all(old_cache_path.parent().unwrap()).unwrap();
        fs::write(old_cache_path, b"older unresolved provider side effect").unwrap();

        let mut checkpoint = blocked_step07_checkpoint();
        let status = executor
            .reconcile_checkpoint_work_units(&mut checkpoint, "07")
            .unwrap();
        assert_eq!(status, StageWorkUnitReconcileStatus::RecoveryBlocked);
        assert_eq!(checkpoint.status, PipelineCheckpointStatus::RecoveryBlocked);
        assert_eq!(checkpoint.resume_policy, PipelineResumePolicy::Disabled);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step07_orphan_cache_without_journal_remains_recovery_blocked() {
        let root = temp_root("product_pipeline_step07_orphan_cache");
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests();
        let cache_path = executor
            .work_unit_journal_root
            .join("stage_07/image_cache/orphan.png");
        fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        fs::write(cache_path, b"unbound provider side effect").unwrap();

        let mut checkpoint = blocked_step07_checkpoint();
        let status = executor
            .reconcile_checkpoint_work_units(&mut checkpoint, "07")
            .unwrap();
        assert_eq!(status, StageWorkUnitReconcileStatus::RecoveryBlocked);
        assert_eq!(checkpoint.units[0].status, PipelineUnitStatus::Unknown);
        assert!(checkpoint.units[0].reconcile_required);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step07_invalid_internal_journal_is_rejected_before_resume_acceptance() {
        let root = temp_root("product_pipeline_step07_invalid_journal");
        let executor = ProductPipelineExecutor::new(&root, "session_a")
            .unwrap()
            .without_protocol_gate_for_tests();
        let journal_dir = executor
            .work_unit_journal_root
            .join("stage_07/journal/invalid-lineage");
        fs::create_dir_all(&journal_dir).unwrap();
        fs::write(
            journal_dir.join("00000000000000000001.json"),
            b"{invalid journal",
        )
        .unwrap();

        let mut checkpoint = blocked_step07_checkpoint();
        assert!(
            executor
                .reconcile_checkpoint_work_units(&mut checkpoint, "07")
                .is_err()
        );
        assert_eq!(checkpoint.status, PipelineCheckpointStatus::RecoveryBlocked);
        assert_eq!(checkpoint.resume_policy, PipelineResumePolicy::Disabled);
        assert_eq!(checkpoint.units[0].status, PipelineUnitStatus::Unknown);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn filesystem_workspace_agent_renames_from_target_snapshot() {
        let root = temp_root("filesystem_workspace_rename_snapshot");
        let workspace_root = root.join("workspace");
        let target = workspace_root.join("target");
        fs::create_dir_all(target.join("assets")).unwrap();
        fs::write(target.join("assets/old.txt"), "sealed source").unwrap();
        let source_hash = sha256_hex(b"sealed source");
        let contract = workspace_contract(vec![WorkspaceOperation::RenameFile {
            from: workspace_relative("assets/old.txt"),
            to: workspace_relative("assets/new.txt"),
            expected_source_sha256: source_hash,
            expected_target: WorkspaceFileExpectation::Missing,
        }]);
        let task = workspace_task("rename_snapshot", contract.clone());
        let agent = FilesystemWorkspaceTaskAgent {
            workspace_root: workspace_root.clone(),
        };

        let result = agent.execute_task(&task, 1, None).unwrap();

        assert_eq!(result.outcome, ChangeOutcome::Committed);
        assert_eq!(result.side_effect_state, SideEffectState::Committed);
        assert!(!target.join("assets/old.txt").exists());
        assert_eq!(
            fs::read_to_string(target.join("assets/new.txt")).unwrap(),
            "sealed source"
        );
        let compile_smoke: Value =
            serde_json::from_slice(&fs::read(target.join("build/compile_smoke.json")).unwrap())
                .unwrap();
        assert_eq!(compile_smoke["status"], "not_executed");
        assert_eq!(compile_smoke["verificationRequired"], true);
        assert!(
            !workspace_root
                .join("isolated/rename_snapshot_attempt_1")
                .exists()
        );
        assert!(result.validate_against(&contract).is_valid());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn filesystem_workspace_agent_rejects_conflict_without_mutating_target() {
        let root = temp_root("filesystem_workspace_conflict_no_mutation");
        let workspace_root = root.join("workspace");
        let target = workspace_root.join("target");
        fs::create_dir_all(target.join("assets")).unwrap();
        fs::write(target.join("assets/existing.txt"), "original").unwrap();
        let contract = workspace_contract(vec![WorkspaceOperation::WriteFile {
            path: workspace_relative("assets/existing.txt"),
            expected: WorkspaceFileExpectation::Missing,
            payload: WorkspaceFilePayload::utf8("replacement"),
        }]);
        let task = workspace_task("conflict_no_mutation", contract);
        let agent = FilesystemWorkspaceTaskAgent {
            workspace_root: workspace_root.clone(),
        };

        let result = agent.execute_task(&task, 1, None).unwrap();

        assert_eq!(result.outcome, ChangeOutcome::Rejected);
        assert_eq!(result.side_effect_state, SideEffectState::None);
        assert_eq!(
            fs::read_to_string(target.join("assets/existing.txt")).unwrap(),
            "original"
        );
        assert!(!target.join("build/compile_smoke.json").exists());
        let _ = fs::remove_dir_all(root);
    }

    fn write_step07_started_record(
        executor: &ProductPipelineExecutor,
        style_id: &str,
        lineage_variant: &str,
        updated_at: u64,
    ) -> (WorkUnitJournalRecord, PathBuf) {
        let request = WorkUnitRequest::new(
            "07",
            &format!("image:{style_id}"),
            WorkUnitKind::Art,
            json!({"style_id": style_id, "lineage_variant": lineage_variant}),
        )
        .unwrap();
        let record = WorkUnitJournalRecord {
            schema_version: 1,
            revision: 1,
            stage_id: request.stage_id.clone(),
            task_id: request.task_id.clone(),
            unit_id: request.unit_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            request_fingerprint: "0".repeat(64),
            phase: WorkUnitJournalPhase::Started,
            result: None,
            result_fingerprint: String::new(),
            failure_message: String::new(),
            updated_at: format!("unix:{updated_at}"),
        };
        let lineage =
            sha256_hex(format!("{}:{}", record.unit_id, record.idempotency_key).as_bytes());
        let journal_dir = executor
            .work_unit_journal_root
            .join("stage_07/journal")
            .join(lineage);
        fs::create_dir_all(&journal_dir).unwrap();
        fs::write(
            journal_dir.join("00000000000000000001.json"),
            serde_json::to_vec_pretty(&record).unwrap(),
        )
        .unwrap();
        let cache_path = executor
            .work_unit_journal_root
            .join("stage_07/image_cache")
            .join(format!(
                "{}.png",
                style_image_cache_key(&record.idempotency_key)
            ));
        (record, cache_path)
    }

    fn blocked_step07_checkpoint() -> PipelineCheckpoint {
        PipelineCheckpoint {
            status: PipelineCheckpointStatus::RecoveryBlocked,
            resume_policy: PipelineResumePolicy::Disabled,
            recovery_blocked_reason: "interrupted Step07".to_string(),
            units: vec![PipelineUnitCheckpoint {
                stage_id: "07".to_string(),
                unit_id: "07:stage".to_string(),
                status: PipelineUnitStatus::Unknown,
                reconcile_required: true,
                failure_message: "interrupted".to_string(),
                ..PipelineUnitCheckpoint::default()
            }],
            ..PipelineCheckpoint::default()
        }
    }

    fn empty_state() -> PipelineRunState {
        PipelineRunState {
            run_id: "product-test".to_string(),
            status: "idle".to_string(),
            stop_requested: false,
            current_stage_id: None,
            stages: BTreeMap::new(),
            ..PipelineRunState::default()
        }
    }

    fn empty_context(stage_id: &str) -> StageContextModel {
        StageContextModel {
            stage_id: stage_id.to_string(),
            project_root: String::new(),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            metadata: BTreeMap::new(),
            knowledge: BTreeMap::new(),
            skills: BTreeMap::new(),
            test_mode: false,
            artifact_dir: String::new(),
        }
    }

    fn passed_step13_for_product_step14_test(spec: &GameSpec) -> step13_v2::Step13AcceptanceOutput {
        let spec_hash = canonicalize_game_spec(spec).unwrap().content_hash;
        step13_v2::Step13AcceptanceOutput {
            schema_version: "step13_acceptance_validation.v1".to_string(),
            compiler_version: step13_v2::STEP13_V2_COMPILER_VERSION.to_string(),
            status: step13_v2::Step13Status::Passed,
            spec_hash: spec_hash.clone(),
            build_hash: sha256_hex(format!("{spec_hash}:build").as_bytes()),
            scenario_results: spec
                .acceptance_scenarios
                .iter()
                .map(
                    |(scenario_id, scenario)| step13_v2::AcceptanceScenarioResult {
                        scenario_id: scenario_id.to_string(),
                        summary: scenario.summary.clone(),
                        automation_kind: step13_v2::AutomationKind::Automated,
                        status: step13_v2::ScenarioExecutionStatus::Passed,
                        action_ids: scenario
                            .when
                            .iter()
                            .map(|action| action.action.to_string())
                            .collect(),
                        spec_hash: spec_hash.clone(),
                        build_hash: "product-step14-test-build".to_string(),
                        log_hash: sha256_hex(scenario_id.to_string().as_bytes()),
                        performance_checks: Vec::new(),
                        accessibility_checks: Vec::new(),
                        failure_reason: None,
                    },
                )
                .collect(),
            output_paths: Default::default(),
        }
    }

    fn write_product_step14_standalone_release_evidence(root: &Path) -> String {
        let evidence_id = "3".repeat(32);
        let transaction_id = "4".repeat(32);
        let now = adm_new_foundation::unix_timestamp();
        let checks = product_step14_required_release_checks()
            .into_iter()
            .map(|check| {
                (
                    check.to_string(),
                    json!({
                        "status": "passed",
                        "command": format!("fixture:{check}"),
                        "exitCode": 0,
                        "durationMs": 1,
                        "outputSha256": "a".repeat(64),
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        write_json(
            &root.join("gates/standalone-release-evidence.json"),
            &json!({
                "schemaVersion": 2,
                "producer": "tools/verify-standalone.ps1/v2",
                "evidenceId": evidence_id,
                "projectId": "autodesignmaker-rust-v2",
                "status": "passed",
                "gitCommit": "b".repeat(40),
                "sourceTreeClean": true,
                "generatedAtUnix": now,
                "expiresAtUnix": now + 3600,
                "checks": checks,
                "portable": {
                    "root": "dist/AutoDesignMaker-NEWrust-release",
                    "executable": "AutoDesignMaker.exe",
                    "executableSha256": "c".repeat(64),
                    "buildManifestSha256": "d".repeat(64),
                    "resourceManifestSha256": "e".repeat(64),
                    "gitCommit": "b".repeat(40),
                    "swapReceipt": format!("dist/.AutoDesignMaker-NEWrust-release.swap-{transaction_id}.json"),
                    "swapReceiptSha256": "f".repeat(64),
                    "transactionId": transaction_id,
                    "transactionStatus": "finalized",
                },
                "errors": [],
            }),
        )
        .unwrap();
        evidence_id
    }

    fn product_step14_required_release_checks() -> Vec<&'static str> {
        vec![
            "cargo_fmt_check",
            "cargo_check_workspace",
            "cargo_test_workspace",
            "web_unit",
            "web_i18n",
            "web_design_content",
            "web_build",
            "web_e2e",
            "web_language_gate",
            "web_ui_gate",
            "web_ui_baseline_gate",
            "package_contract_self_test",
            "resource_manifest",
            "standalone_boundary_gate",
            "portable_build",
            "portable_smoke",
            "portable_integrity",
            "pe_architecture_crt",
            "clean_clone_relocation",
            "anti_fake_scan",
            "generated_cleanup",
        ]
    }

    fn workspace_relative(path: &str) -> WorkspaceRelativePath {
        WorkspaceRelativePath::parse(path).unwrap()
    }

    fn workspace_contract(operations: Vec<WorkspaceOperation>) -> WorkspaceChangeSet {
        let mut agent_write_paths = StdBTreeSet::new();
        let mut read_paths = StdBTreeSet::from([workspace_relative("Tests/TrustedSmoke.cs")]);
        for operation in &operations {
            collect_operation_paths(operation, &mut agent_write_paths);
            match operation {
                WorkspaceOperation::WriteFile { path, expected, .. } => {
                    if !matches!(expected, WorkspaceFileExpectation::Missing) {
                        read_paths.insert(path.clone());
                    }
                }
                WorkspaceOperation::DeleteFile { path, .. } => {
                    read_paths.insert(path.clone());
                }
                WorkspaceOperation::RenameFile {
                    from,
                    to,
                    expected_target,
                    ..
                } => {
                    read_paths.insert(from.clone());
                    if !matches!(expected_target, WorkspaceFileExpectation::Missing) {
                        read_paths.insert(to.clone());
                    }
                }
            }
        }
        WorkspaceChangeSet {
            schema_version: WORKSPACE_CHANGE_SET_SCHEMA_VERSION.to_string(),
            change_set_id: "filesystem_workspace_agent_contract".to_string(),
            base_tree_hash: sha256_hex(b"base-tree"),
            read_paths,
            agent_write_paths,
            trusted_tool_write_paths: StdBTreeSet::new(),
            build_output_paths: StdBTreeSet::from([workspace_relative("build")]),
            operations,
            command_permissions: vec![
                command_permission("compile_workspace", CommandPurpose::Compile),
                command_permission("trusted_test", CommandPurpose::Test),
            ],
            trusted_tests: vec![TrustedTestContract {
                test_id: "trusted_runtime_smoke".to_string(),
                path: workspace_relative("Tests/TrustedSmoke.cs"),
                baseline_sha256: sha256_hex(b"trusted-test-baseline"),
                command_id: "trusted_test".to_string(),
            }],
            resource_budget: WorkspaceResourceBudget {
                max_duration_ms: 30_000,
                max_processes: 2,
                max_write_bytes: 1_000_000,
                max_file_count: 20,
                max_retries: 2,
            },
            evidence: vec![ChangeEvidence::from_bytes(
                "filesystem_workspace_agent_contract",
                "test",
                EvidenceStatus::Observed,
                b"sealed contract fixture",
            )],
        }
    }

    fn command_permission(command_id: &str, purpose: CommandPurpose) -> CommandPermission {
        CommandPermission {
            command_id: command_id.to_string(),
            tool_binding_id: "local_test_tool".to_string(),
            purpose,
            argument_template: vec!["--workspace".to_string()],
            working_directory: None,
            timeout_ms: 10_000,
            allow_network: false,
        }
    }

    fn workspace_task(
        task_id: &str,
        workspace_contract: WorkspaceChangeSet,
    ) -> step08_10_v2::TrustedDevelopmentTask {
        step08_10_v2::TrustedDevelopmentTask {
            task_id: task_id.to_string(),
            title: task_id.to_string(),
            ordinal_size: "S".to_string(),
            architecture_system_id: "test_runtime".to_string(),
            declared_write_paths: workspace_contract
                .agent_write_paths
                .iter()
                .map(|path| path.as_str().to_string())
                .collect(),
            machine_checks: vec![step08_10_v2::MachineAcceptanceCheck {
                check_id: "check_trusted_runtime_smoke".to_string(),
                compile_target: "test".to_string(),
                trusted_test_id: "trusted_runtime_smoke".to_string(),
                command_id: "trusted_test".to_string(),
            }],
            dependencies: Vec::new(),
            rollback_boundary: Vec::new(),
            workspace_contract,
        }
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
        fs::create_dir_all(&root).unwrap();
        root
    }
}
