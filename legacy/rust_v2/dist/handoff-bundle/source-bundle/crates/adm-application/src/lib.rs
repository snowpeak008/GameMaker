#![forbid(unsafe_code)]

mod core_pipeline;
mod pipeline_service;
mod production_readiness;
mod run_log_service;
mod sdk_knowledge_service;
mod supplement_service;
mod workbench_service;

pub use core_pipeline::{
    CorePipelineOutputs, CorePipelineServices, DEVFLOW_RUN_REPORT_PATH, DEVFLOW_RUN_STATE_PATH,
    DevFlowStepSpec, core_stage_id_for_devflow_step, devflow_step_spec, devflow_step_specs,
};
pub use pipeline_service::{
    DEVFLOW_REQUESTS_RELATIVE_PATH, DEVFLOW_STEP07_STYLE_RELATIVE_PATH,
    DEVFLOW_STOP_REQUEST_RELATIVE_PATH, DevflowRangeRunRequest, DevflowStopRequest,
    PipelineService, Step07StyleConfirmation,
};
pub use run_log_service::{RUN_LOG_RELATIVE_PATH, RunLogEntry, RunLogService};
pub use sdk_knowledge_service::{
    SDK_APPROVED_CONTEXT_RELATIVE_PATH, SDK_REVIEW_RELATIVE_PATH, SdkKnowledgeService,
    SdkKnowledgeSnapshot, SdkReviewRecord, SdkReviewStatus,
};
pub use supplement_service::{SupplementAnalysis, SupplementTask, analyze_supplement_request};
pub use workbench_service::{
    WorkbenchChecklistRow, WorkbenchDomainRow, WorkbenchInterviewRunReport, WorkbenchL4OptionRow,
    WorkbenchNodeDetail, WorkbenchNodeRow, WorkbenchService, WorkbenchSnapshot,
    WorkbenchTemplateRow,
};

use adm_ai::{
    AiProvider, AiRemoteTransport, AiSecretMaterial, AiTaskJournal, ChatCompletionsTransport,
    RemoteAiProvider, ReqwestBlockingHttpJsonClient,
};
use adm_archive::{
    ArchiveCommitReport, ArchivePackageDoctorReport, ArchiveRepository,
    ArchiveWorkspaceCleanupReport, ArchiveWorkspaceDoctorReport, FormalArchive,
    inspect_archive_package,
};
use adm_config::{
    AiProviderConfig, AiProviderDiagnostic, AppConfig, AppSecretResolver, SecretResolver,
};
use adm_design::{GameDesignBrief, WorkbenchState};
use adm_foundation::{
    AdmError, AdmErrorKind, AdmResult, ArchiveId, ProviderId, SessionId, StageId,
};
use adm_packaging::EngineBuildExecutionReport;
use adm_pipeline::{ArtifactRegistry, PipelineRunReport, PipelineRunState};
use adm_runtime::{RuntimeValidationExecutionSummary, summarize_runtime_validation_execution};
use adm_validation::ValidationReport;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub type ConfiguredChatCompletionsProvider =
    RemoteAiProvider<ChatCompletionsTransport<ReqwestBlockingHttpJsonClient>>;

pub const PROJECT_BRIEF_PATH: &str = "project/brief.adm";
pub const RUNTIME_EXECUTION_RESULTS_PATH: &str = "validation/runtime_execution_results.adm";
pub const DESIGN_WORKBENCH_STATE_PATH: &str = "design/workbench_state.json";
pub const DESIGN_WORKBENCH_EXPORT_PATH: &str = "design/project.adm";
pub const SUPPLEMENT_REQUESTS_PATH: &str = "patch/supplement_requests.adm";

#[derive(Debug, Clone)]
pub struct AdmApplication {
    config: AppConfig,
    archives: ArchiveRepository,
}

impl AdmApplication {
    pub fn new(config: AppConfig) -> AdmResult<Self> {
        config.validate()?;
        let archives = ArchiveRepository::new(config.data_root.clone());
        archives.initialize()?;
        config.ensure_profile()?;
        Ok(Self { config, archives })
    }

    pub fn for_data_root(data_root: impl Into<PathBuf>) -> AdmResult<Self> {
        Self::new(AppConfig::load_or_default(data_root)?)
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn save_config(&self) -> AdmResult<PathBuf> {
        self.config.save_profile()
    }

    pub fn upsert_ai_provider(&mut self, provider: AiProviderConfig) -> AdmResult<PathBuf> {
        self.config.ai.upsert_provider(provider)?;
        self.config.validate()?;
        self.config.save_profile()
    }

    pub fn disable_ai_provider(&mut self, provider_id: ProviderId) -> AdmResult<PathBuf> {
        self.config.ai.disable_provider(provider_id)?;
        self.config.validate()?;
        self.config.save_profile()
    }

    pub fn upsert_named_secret(
        &self,
        name: impl AsRef<str>,
        secret: impl AsRef<str>,
    ) -> AdmResult<PathBuf> {
        self.config.upsert_named_secret(name, secret)
    }

    pub fn remote_ai_provider_from_config<T: AiRemoteTransport>(
        &self,
        provider_id: &ProviderId,
        transport: T,
    ) -> AdmResult<RemoteAiProvider<T>> {
        let provider = self
            .config
            .ai
            .providers
            .iter()
            .find(|provider| &provider.provider_id == provider_id)
            .ok_or_else(|| {
                AdmError::invalid_input(format!("AI provider {} is not configured", provider_id))
            })?;
        if !provider.enabled {
            return Err(AdmError::conflict(format!(
                "AI provider {} is disabled",
                provider.provider_id
            )));
        }
        let endpoint_hint = provider.endpoint_hint.as_ref().ok_or_else(|| {
            AdmError::invalid_input(format!(
                "AI provider {} has no endpoint_hint",
                provider.provider_id
            ))
        })?;
        let secret_ref = provider.secret_ref.as_ref().ok_or_else(|| {
            AdmError::conflict(format!(
                "AI provider {} has no secret_ref",
                provider.provider_id
            ))
        })?;
        let resolver = AppSecretResolver::from_config(&self.config);
        let secret = resolver.resolve(secret_ref)?.ok_or_else(|| {
            AdmError::conflict(format!(
                "AI provider {} secret {} is not available",
                provider.provider_id,
                secret_ref.render_public()
            ))
        })?;
        RemoteAiProvider::new(
            provider.provider_id.clone(),
            endpoint_hint.clone(),
            AiSecretMaterial::new(secret)?,
            provider.capabilities.clone(),
            transport,
        )
    }

    pub fn chat_completions_provider_from_config(
        &self,
        provider_id: &ProviderId,
        model: impl Into<String>,
    ) -> AdmResult<ConfiguredChatCompletionsProvider> {
        let client = ReqwestBlockingHttpJsonClient::new()?;
        let transport = ChatCompletionsTransport::new(model, client)?;
        self.remote_ai_provider_from_config(provider_id, transport)
    }

    pub fn ai_diagnostics(&self) -> AiDiagnosticsReport {
        let resolver = AppSecretResolver::from_config(&self.config);
        let providers = self
            .config
            .ai
            .providers
            .iter()
            .map(|provider| provider.diagnose(&resolver))
            .collect();
        AiDiagnosticsReport {
            default_budget_units: self.config.ai.default_budget_units,
            retry_max_attempts: self.config.ai.retry_policy.max_attempts,
            providers,
        }
    }

    pub fn create_project(&self, display_name: &str) -> AdmResult<ProjectCreateReport> {
        let archive = self.archives.create_archive(display_name)?;
        let open = self
            .archives
            .open_archive_workspace(&archive, SessionId::generate())?;
        self.archives.write_workspace_text(
            &open.workspace,
            "project/status.adm",
            "status=created\npipeline=not_started\n",
        )?;
        let commit = self
            .archives
            .commit_workspace(&archive, &open.workspace, &open.lock)?;
        Ok(ProjectCreateReport { archive, commit })
    }

    pub fn list_projects(&self) -> AdmResult<Vec<ProjectSummary>> {
        self.archives
            .list_archives()
            .map(|archives| archives.into_iter().map(ProjectSummary::from).collect())
    }

    pub fn load_project(&self, archive_id: &str) -> AdmResult<FormalArchive> {
        self.archives
            .load_archive(&adm_foundation::ArchiveId::from_str(archive_id)?)
    }

    pub fn load_project_brief(&self, archive: &FormalArchive) -> AdmResult<GameDesignBrief> {
        let content_root = archive.root.join("content");
        let brief_path = content_root.join(PROJECT_BRIEF_PATH);
        if brief_path.exists() {
            return parse_brief_document(&fs::read_to_string(brief_path)?);
        }
        parse_brief_from_design_document(&fs::read_to_string(
            content_root.join("design/project.adm"),
        )?)
    }

    pub fn export_project(
        &self,
        archive_id: &str,
        target_file: impl AsRef<Path>,
    ) -> AdmResult<PathBuf> {
        let archive = self.load_project(archive_id)?;
        self.archives.export_archive_package(&archive, target_file)
    }

    pub fn import_project(&self, package_file: impl AsRef<Path>) -> AdmResult<ProjectSummary> {
        self.archives
            .import_archive_package(package_file)
            .map(ProjectSummary::from)
    }

    pub fn commit_design_workbench_state(
        &self,
        archive_id: Option<&str>,
        display_name: &str,
        state: &WorkbenchState,
        export_markdown: &str,
    ) -> AdmResult<DesignWorkbenchArchiveCommit> {
        let display_name = if display_name.trim().is_empty() {
            state.project_name.as_str()
        } else {
            display_name.trim()
        };
        let archive = match archive_id.map(str::trim).filter(|id| !id.is_empty()) {
            Some(archive_id) => self
                .archives
                .load_archive(&ArchiveId::from_str(archive_id)?)?,
            None => self.archives.create_archive(display_name)?,
        };
        let open = self
            .archives
            .open_archive_workspace(&archive, SessionId::generate())?;
        let state_json = serde_json::to_string_pretty(state).map_err(|error| {
            AdmError::validation(format!("failed to serialize workbench state: {error}"))
        })?;
        self.archives.write_workspace_text(
            &open.workspace,
            DESIGN_WORKBENCH_STATE_PATH,
            &state_json,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            DESIGN_WORKBENCH_EXPORT_PATH,
            export_markdown,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "project/status.adm",
            &format!(
                "status=design_workbench_saved\npipeline=not_started\nproject_name={}\n",
                state.project_name
            ),
        )?;
        let commit = self
            .archives
            .commit_workspace(&archive, &open.workspace, &open.lock)?;
        Ok(DesignWorkbenchArchiveCommit {
            archive,
            commit,
            state_file: PathBuf::from(DESIGN_WORKBENCH_STATE_PATH),
            export_file: PathBuf::from(DESIGN_WORKBENCH_EXPORT_PATH),
        })
    }

    pub fn load_design_workbench_state(&self, archive_id: &str) -> AdmResult<WorkbenchState> {
        let archive = self
            .archives
            .load_archive(&ArchiveId::from_str(archive_id.trim())?)?;
        let path = archive
            .root
            .join("content")
            .join(DESIGN_WORKBENCH_STATE_PATH);
        if !path.exists() {
            return Err(AdmError::new(
                AdmErrorKind::NotFound,
                "archive does not contain a design workbench state",
            )
            .with_context(format!("archive_id={archive_id}")));
        }
        let text = fs::read_to_string(path)?;
        serde_json::from_str::<WorkbenchState>(&text).map_err(|error| {
            AdmError::validation(format!("failed to parse design workbench state: {error}"))
        })
    }

    pub fn commit_supplement_analysis(
        &self,
        archive_id: &str,
        analysis: &SupplementAnalysis,
    ) -> AdmResult<SupplementArchiveCommit> {
        let archive = self
            .archives
            .load_archive(&ArchiveId::from_str(archive_id.trim())?)?;
        let open = self
            .archives
            .open_archive_workspace(&archive, SessionId::generate())?;
        let existing_path = open.workspace.content_root().join(SUPPLEMENT_REQUESTS_PATH);
        let mut document = if existing_path.exists() {
            fs::read_to_string(&existing_path)?
        } else {
            "# Supplemental Development Requests\n".to_string()
        };
        if !document.ends_with('\n') {
            document.push('\n');
        }
        document.push_str("\n---\n");
        document.push_str(&analysis.render());
        self.archives
            .write_workspace_text(&open.workspace, SUPPLEMENT_REQUESTS_PATH, &document)?;
        let commit = self
            .archives
            .commit_workspace(&archive, &open.workspace, &open.lock)?;
        Ok(SupplementArchiveCommit {
            commit,
            request_file: PathBuf::from(SUPPLEMENT_REQUESTS_PATH),
            task_count: analysis.tasks.len(),
        })
    }

    pub fn inspect_project_package(
        &self,
        package_file: impl AsRef<Path>,
    ) -> AdmResult<ArchivePackageDoctorReport> {
        inspect_archive_package(package_file)
    }

    pub fn inspect_workspaces(&self) -> AdmResult<ArchiveWorkspaceDoctorReport> {
        self.archives.inspect_workspaces()
    }

    pub fn cleanup_stale_workspaces(&self) -> AdmResult<ArchiveWorkspaceCleanupReport> {
        self.archives.cleanup_stale_workspaces()
    }

    pub fn commit_engine_build_execution(
        &self,
        archive: &FormalArchive,
        report: &EngineBuildExecutionReport,
    ) -> AdmResult<EngineBuildExecutionHistoryCommit> {
        let open = self
            .archives
            .open_archive_workspace(archive, SessionId::generate())?;
        let history_file = PathBuf::from("package/engine_build_history.adm");
        let history_path = open.workspace.content_root().join(&history_file);
        let mut history = if history_path.exists() {
            fs::read_to_string(&history_path)?
        } else {
            "# Engine Build Execution History\n".to_string()
        };
        if !history.ends_with('\n') {
            history.push('\n');
        }
        if !history.ends_with("\n---\n") {
            history.push_str("\n---\n");
        }
        history.push_str(&report.render());
        let record_count = history.matches("# Engine Build Execution\n").count();
        self.archives
            .write_workspace_text(&open.workspace, &history_file, &history)?;
        let commit = self
            .archives
            .commit_workspace(archive, &open.workspace, &open.lock)?;
        Ok(EngineBuildExecutionHistoryCommit {
            commit,
            history_file,
            record_count,
        })
    }

    pub fn commit_runtime_validation_execution(
        &self,
        archive: &FormalArchive,
        execution_text: &str,
    ) -> AdmResult<RuntimeValidationExecutionCommit> {
        let open = self
            .archives
            .open_archive_workspace(archive, SessionId::generate())?;
        let runtime_contract = self
            .archives
            .read_workspace_text(&open.workspace, "validation/runtime_validation_report.adm")?;
        let summary = summarize_runtime_validation_execution(&runtime_contract, execution_text)?;
        let results_document = summary.render();
        self.archives.write_workspace_text(
            &open.workspace,
            RUNTIME_EXECUTION_RESULTS_PATH,
            &results_document,
        )?;

        let readiness_path = "validation/production_readiness.adm";
        let readiness_document = self
            .archives
            .read_workspace_text(&open.workspace, readiness_path)?;
        let updated_readiness =
            update_production_readiness_with_runtime_execution(&readiness_document, &summary);
        self.archives
            .write_workspace_text(&open.workspace, readiness_path, &updated_readiness)?;

        let commit = self
            .archives
            .commit_workspace(archive, &open.workspace, &open.lock)?;
        Ok(RuntimeValidationExecutionCommit {
            commit,
            results_file: PathBuf::from(RUNTIME_EXECUTION_RESULTS_PATH),
            summary,
        })
    }

    pub fn run_core_pipeline<P: AiProvider>(
        &self,
        archive: &FormalArchive,
        brief: GameDesignBrief,
        ai_provider: &P,
    ) -> AdmResult<ProjectPipelineReport> {
        let open = self
            .archives
            .open_archive_workspace(archive, SessionId::generate())?;
        let outputs = CorePipelineServices::new().build(brief, ai_provider, &self.config.ai)?;
        self.commit_core_pipeline_outputs(archive, &open, outputs)
    }

    pub fn resume_core_pipeline<P: AiProvider>(
        &self,
        archive: &FormalArchive,
        brief: GameDesignBrief,
        ai_provider: &P,
    ) -> AdmResult<ProjectPipelineReport> {
        let open = self
            .archives
            .open_archive_workspace(archive, SessionId::generate())?;
        let state_text = self
            .archives
            .read_workspace_text(&open.workspace, "pipeline/run_state.adm")?;
        let state = PipelineRunState::from_state_text(&state_text)?;
        let preserve_ai_journal = preserved_ai_journal(&open, &state)?;
        let mut outputs = CorePipelineServices::new().build_with_state(
            brief,
            ai_provider,
            &self.config.ai,
            state,
        )?;
        if let Some(journal) = preserve_ai_journal {
            outputs.ai_journal = journal;
        }
        self.commit_core_pipeline_outputs(archive, &open, outputs)
    }

    pub fn rerun_core_pipeline_stage<P: AiProvider>(
        &self,
        archive: &FormalArchive,
        brief: GameDesignBrief,
        ai_provider: &P,
        stage_id: &str,
    ) -> AdmResult<ProjectPipelineReport> {
        let open = self
            .archives
            .open_archive_workspace(archive, SessionId::generate())?;
        let state_text = self
            .archives
            .read_workspace_text(&open.workspace, "pipeline/run_state.adm")?;
        let mut state = PipelineRunState::from_state_text(&state_text)?;
        let stage_id = StageId::new(stage_id)?;
        let services = CorePipelineServices::new();
        services.rewind_state_to_stage(&mut state, &stage_id)?;
        let preserve_ai_journal = preserved_ai_journal(&open, &state)?;
        let mut outputs = services.build_with_state(brief, ai_provider, &self.config.ai, state)?;
        if let Some(journal) = preserve_ai_journal {
            outputs.ai_journal = journal;
        }
        self.commit_core_pipeline_outputs(archive, &open, outputs)
    }

    pub fn resume_failed_core_pipeline<P: AiProvider>(
        &self,
        archive: &FormalArchive,
        brief: GameDesignBrief,
        ai_provider: &P,
    ) -> AdmResult<ProjectPipelineReport> {
        let open = self
            .archives
            .open_archive_workspace(archive, SessionId::generate())?;
        let state_text = self
            .archives
            .read_workspace_text(&open.workspace, "pipeline/run_state.adm")?;
        let report_text = self
            .archives
            .read_workspace_text(&open.workspace, "pipeline/run_report.adm")?;
        let mut state = PipelineRunState::from_state_text(&state_text)?;
        let report = PipelineRunReport::from_report_text(&report_text)?;
        let failed_stage = report
            .last_unsuccessful_stage_id()
            .ok_or_else(|| AdmError::conflict("pipeline run report has no failed stage to resume"))?
            .clone();
        let services = CorePipelineServices::new();
        services.rewind_state_to_stage(&mut state, &failed_stage)?;
        let preserve_ai_journal = preserved_ai_journal(&open, &state)?;
        let mut outputs = services.build_with_state(brief, ai_provider, &self.config.ai, state)?;
        if let Some(journal) = preserve_ai_journal {
            outputs.ai_journal = journal;
        }
        self.commit_core_pipeline_outputs(archive, &open, outputs)
    }

    fn commit_core_pipeline_outputs(
        &self,
        archive: &FormalArchive,
        open: &adm_archive::OpenArchiveSession,
        outputs: CorePipelineOutputs,
    ) -> AdmResult<ProjectPipelineReport> {
        self.archives.write_workspace_text(
            &open.workspace,
            PROJECT_BRIEF_PATH,
            &outputs.brief_document,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "design/project.adm",
            &outputs.design_document,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "development/plan.adm",
            &outputs.development_document,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "assets/plan.adm",
            &outputs.asset_document,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "sdk/index.adm",
            &outputs.sdk_document,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "package/build_targets.adm",
            &outputs.build_targets_document,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "package/manifest.adm",
            &outputs.package_document,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "validation/report.adm",
            &outputs.validation.render(),
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "validation/acceptance_matrix.adm",
            &outputs.acceptance_matrix_document,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "validation/scenario_test_plan.adm",
            &outputs.scenario_test_plan_document,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "validation/runtime_validation_report.adm",
            &outputs.runtime_validation_document,
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "validation/production_readiness.adm",
            &outputs.production_readiness_document,
        )?;
        for document in &outputs.devflow_step_documents {
            self.archives.write_workspace_text(
                &open.workspace,
                &document.relative_path,
                &document.content,
            )?;
        }
        self.archives.write_workspace_text(
            &open.workspace,
            "pipeline/run_report.adm",
            &outputs.pipeline_report.render(),
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "pipeline/run_state.adm",
            &outputs.run_state.render(),
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            DEVFLOW_RUN_REPORT_PATH,
            &outputs.devflow_pipeline_report.render(),
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            DEVFLOW_RUN_STATE_PATH,
            &outputs.devflow_run_state.render(),
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "pipeline/artifact_registry.adm",
            &outputs.artifact_registry.render(),
        )?;
        self.archives.write_workspace_text(
            &open.workspace,
            "ai/journal.adm",
            &outputs.ai_journal.render(),
        )?;

        let commit = self
            .archives
            .commit_workspace(archive, &open.workspace, &open.lock)?;

        Ok(ProjectPipelineReport {
            pipeline_report: outputs.pipeline_report,
            validation: outputs.validation,
            artifact_registry: outputs.artifact_registry,
            run_state: outputs.run_state,
            ai_journal: outputs.ai_journal,
            devflow_pipeline_report: outputs.devflow_pipeline_report,
            devflow_run_state: outputs.devflow_run_state,
            acceptance_matrix_document: outputs.acceptance_matrix_document,
            runtime_validation_document: outputs.runtime_validation_document,
            production_readiness_document: outputs.production_readiness_document,
            commit,
        })
    }
}

fn preserved_ai_journal(
    open: &adm_archive::OpenArchiveSession,
    state: &PipelineRunState,
) -> AdmResult<Option<AiTaskJournal>> {
    if !state
        .completed_stages
        .iter()
        .any(|stage| stage.as_str() == "design")
    {
        return Ok(None);
    }
    let journal_path = open.workspace.content_root().join("ai/journal.adm");
    if journal_path.exists() {
        AiTaskJournal::load_from_path(journal_path).map(Some)
    } else {
        Ok(None)
    }
}

fn update_production_readiness_with_runtime_execution(
    document: &str,
    summary: &RuntimeValidationExecutionSummary,
) -> String {
    let runtime_status = if summary.ready() {
        "ready"
    } else if summary.passed_rows > 0 {
        "warning"
    } else {
        "blocked"
    };
    let runtime_check = format!(
        "- check_id=runtime_validation_readiness; status={runtime_status}; expected={}; actual={}; artifacts=validation/runtime_validation_report.adm | {}; detail=runtime execution rows={}, passed={}, failed={}, missing={}, unexpected={}",
        summary.contract_rows,
        summary.passed_rows,
        RUNTIME_EXECUTION_RESULTS_PATH,
        summary.observed_rows,
        summary.passed_rows,
        summary.failed_rows,
        summary.missing_rows,
        summary.unexpected_rows
    );
    let mut lines = Vec::new();
    for line in document.lines() {
        if line
            .trim_start()
            .starts_with("- check_id=runtime_validation_readiness;")
        {
            lines.push(runtime_check.clone());
        } else if let Some(trace_artifacts) = line.strip_prefix("trace_artifacts=") {
            lines.push(format!(
                "trace_artifacts={}",
                append_trace_artifact(trace_artifacts, RUNTIME_EXECUTION_RESULTS_PATH)
            ));
        } else {
            lines.push(line.to_string());
        }
    }

    let mut ready_count = 0;
    let mut warning_count = 0;
    let mut blocking_count = 0;
    for line in &lines {
        if !line.trim_start().starts_with("- check_id=") {
            continue;
        }
        match readiness_check_status(line) {
            Some("ready") => ready_count += 1,
            Some("warning") => warning_count += 1,
            Some("blocked") => blocking_count += 1,
            _ => {}
        }
    }
    let overall_status = if blocking_count > 0 {
        "blocked"
    } else if warning_count > 0 {
        "warning"
    } else {
        "ready"
    };

    for line in &mut lines {
        if line.starts_with("overall_status=") {
            *line = format!("overall_status={overall_status}");
        } else if line.starts_with("ready_count=") {
            *line = format!("ready_count={ready_count}");
        } else if line.starts_with("warning_count=") {
            *line = format!("warning_count={warning_count}");
        } else if line.starts_with("blocking_count=") {
            *line = format!("blocking_count={blocking_count}");
        }
    }

    let mut updated = lines.join("\n");
    updated.push('\n');
    updated
}

fn append_trace_artifact(existing: &str, artifact: &str) -> String {
    let mut artifacts = existing
        .split('|')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if !artifacts.iter().any(|value| value == artifact) {
        artifacts.push(artifact.to_string());
    }
    artifacts.join(" | ")
}

fn readiness_check_status(line: &str) -> Option<&str> {
    line.split(';').find_map(|part| {
        part.trim()
            .strip_prefix("status=")
            .map(str::trim)
            .filter(|value| !value.is_empty())
    })
}

#[derive(Debug, Clone)]
pub struct ProjectCreateReport {
    pub archive: FormalArchive,
    pub commit: ArchiveCommitReport,
}

#[derive(Debug, Clone)]
pub struct EngineBuildExecutionHistoryCommit {
    pub commit: ArchiveCommitReport,
    pub history_file: PathBuf,
    pub record_count: usize,
}

#[derive(Debug, Clone)]
pub struct RuntimeValidationExecutionCommit {
    pub commit: ArchiveCommitReport,
    pub results_file: PathBuf,
    pub summary: RuntimeValidationExecutionSummary,
}

#[derive(Debug, Clone)]
pub struct DesignWorkbenchArchiveCommit {
    pub archive: FormalArchive,
    pub commit: ArchiveCommitReport,
    pub state_file: PathBuf,
    pub export_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct SupplementArchiveCommit {
    pub commit: ArchiveCommitReport,
    pub request_file: PathBuf,
    pub task_count: usize,
}

#[derive(Debug, Clone)]
pub struct ProjectSummary {
    pub archive_id: String,
    pub display_name: String,
    pub root: PathBuf,
}

impl From<FormalArchive> for ProjectSummary {
    fn from(value: FormalArchive) -> Self {
        Self {
            archive_id: value.manifest.archive_id.to_string(),
            display_name: value.manifest.display_name,
            root: value.root,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectPipelineReport {
    pub pipeline_report: PipelineRunReport,
    pub validation: ValidationReport,
    pub artifact_registry: ArtifactRegistry,
    pub run_state: PipelineRunState,
    pub ai_journal: AiTaskJournal,
    pub devflow_pipeline_report: PipelineRunReport,
    pub devflow_run_state: PipelineRunState,
    pub acceptance_matrix_document: String,
    pub runtime_validation_document: String,
    pub production_readiness_document: String,
    pub commit: ArchiveCommitReport,
}

#[derive(Debug, Clone)]
pub struct AiDiagnosticsReport {
    pub default_budget_units: u32,
    pub retry_max_attempts: u32,
    pub providers: Vec<AiProviderDiagnostic>,
}

impl AiDiagnosticsReport {
    pub fn ready_provider_count(&self) -> usize {
        self.providers
            .iter()
            .filter(|provider| provider.readiness == adm_config::AiProviderReadiness::Ready)
            .count()
    }

    pub fn render(&self) -> String {
        let mut document = String::new();
        document.push_str("# AI Diagnostics\n");
        document.push_str(&format!(
            "default_budget_units={}\n",
            self.default_budget_units
        ));
        document.push_str(&format!("retry_max_attempts={}\n", self.retry_max_attempts));
        document.push_str(&format!(
            "ready_provider_count={}\n",
            self.ready_provider_count()
        ));
        for provider in &self.providers {
            document.push_str(&provider.render_line());
            document.push('\n');
        }
        document
    }
}

pub fn default_demo_brief(title: &str) -> AdmResult<GameDesignBrief> {
    GameDesignBrief::new(
        title,
        "2D action adventure",
        "玩家通过快速探索、精准战斗和持续成长获得清晰反馈",
        vec![
            "探索关卡并发现风险".to_string(),
            "使用核心能力解决战斗或机关".to_string(),
            "获得反馈、资源和新目标".to_string(),
        ],
    )
}

pub fn design_brief_from_parts(
    title: &str,
    genre: &str,
    player_promise: &str,
    core_loop_steps: &str,
) -> AdmResult<GameDesignBrief> {
    let core_loop = parse_core_loop_steps_input(core_loop_steps);
    GameDesignBrief::new(title, genre, player_promise, core_loop)
}

pub fn parse_brief_document(text: &str) -> AdmResult<GameDesignBrief> {
    let title = find_document_value(text, "title")
        .ok_or_else(|| AdmError::validation("brief document missing title"))?;
    let genre = find_document_value(text, "genre")
        .ok_or_else(|| AdmError::validation("brief document missing genre"))?;
    let player_promise = find_document_value(text, "player_promise")
        .ok_or_else(|| AdmError::validation("brief document missing player_promise"))?;
    let core_loop = parse_brief_core_loop_steps(text);
    GameDesignBrief::new(title, genre, player_promise, core_loop)
}

fn parse_brief_from_design_document(text: &str) -> AdmResult<GameDesignBrief> {
    let title = find_document_value(text, "title")
        .ok_or_else(|| AdmError::validation("design document missing title"))?;
    let genre = find_document_value(text, "genre")
        .ok_or_else(|| AdmError::validation("design document missing genre"))?;
    let player_promise = find_document_value(text, "player_promise")
        .ok_or_else(|| AdmError::validation("design document missing player_promise"))?;
    let core_loop = parse_design_core_loop_steps(text);
    GameDesignBrief::new(title, genre, player_promise, core_loop)
}

fn parse_core_loop_steps_input(value: &str) -> Vec<String> {
    value
        .lines()
        .flat_map(|line| line.split('|'))
        .flat_map(|part| part.split(';'))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn parse_brief_core_loop_steps(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("- "))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn parse_design_core_loop_steps(text: &str) -> Vec<String> {
    let mut in_core_loop = false;
    let mut steps = Vec::new();
    for line in text.lines().map(str::trim) {
        if line == "## Core Loop" {
            in_core_loop = true;
            continue;
        }
        if in_core_loop && line.starts_with("## ") {
            break;
        }
        if in_core_loop && let Some((_, step)) = line.split_once(". ") {
            let step = step.trim();
            if !step.is_empty() {
                steps.push(step.to_string());
            }
        }
    }
    steps
}

fn find_document_value(text: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    text.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(&prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub fn default_data_root(base: &Path) -> PathBuf {
    base.join(".adm_rust_data")
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_ai::{
        AiCapability, AiRemoteRequest, AiRemoteResponse, AiRemoteTransport, AiTaskJournal,
        AiTaskRequest, MockAiProvider,
    };
    use adm_foundation::{ProviderId, RunId, StageId};
    use adm_packaging::{EngineBuildExecutionMode, EngineBuildExecutionStatus};
    use adm_validation::ValidationStatus;

    #[test]
    fn application_runs_core_pipeline_and_commits_outputs() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        let created = app.create_project("Demo").expect("create");
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let report = app
            .run_core_pipeline(
                &created.archive,
                default_demo_brief("Demo").unwrap(),
                &provider,
            )
            .expect("pipeline");

        assert_eq!(report.validation.status, ValidationStatus::Passed);
        assert_eq!(report.ai_journal.records().len(), 1);
        assert!(report.pipeline_report.results.iter().any(|result| {
            result
                .artifacts
                .iter()
                .any(|artifact| artifact.as_str() == "artifact_package_manifest")
        }));
        assert!(report.pipeline_report.results.iter().any(|result| {
            result
                .artifacts
                .iter()
                .any(|artifact| artifact.as_str() == "artifact_game_build_targets")
        }));
        assert!(report.pipeline_report.results.iter().any(|result| {
            result
                .artifacts
                .iter()
                .any(|artifact| artifact.as_str() == "artifact_acceptance_matrix")
        }));
        assert!(report.pipeline_report.results.iter().any(|result| {
            result
                .artifacts
                .iter()
                .any(|artifact| artifact.as_str() == "artifact_scenario_test_plan")
        }));
        assert!(report.pipeline_report.results.iter().any(|result| {
            result
                .artifacts
                .iter()
                .any(|artifact| artifact.as_str() == "artifact_runtime_validation_report")
        }));
        assert!(report.pipeline_report.results.iter().any(|result| {
            result
                .artifacts
                .iter()
                .any(|artifact| artifact.as_str() == "artifact_production_readiness")
        }));
        let loaded_journal =
            AiTaskJournal::load_from_path(created.archive.root.join("content/ai/journal.adm"))
                .expect("load persisted journal");
        assert_eq!(loaded_journal.records().len(), 1);
        let loaded_state = PipelineRunState::from_state_text(
            &std::fs::read_to_string(created.archive.root.join("content/pipeline/run_state.adm"))
                .expect("read run state"),
        )
        .expect("parse run state");
        assert_eq!(
            loaded_state.status,
            adm_pipeline::PipelineRunLifecycleStatus::Succeeded
        );
        assert_eq!(loaded_state.completed_stages.len(), 5);
        let loaded_devflow_state = PipelineRunState::from_state_text(
            &std::fs::read_to_string(
                created
                    .archive
                    .root
                    .join("content")
                    .join(DEVFLOW_RUN_STATE_PATH),
            )
            .expect("read devflow run state"),
        )
        .expect("parse devflow run state");
        let loaded_devflow_report = PipelineRunReport::from_report_text(
            &std::fs::read_to_string(
                created
                    .archive
                    .root
                    .join("content")
                    .join(DEVFLOW_RUN_REPORT_PATH),
            )
            .expect("read devflow run report"),
        )
        .expect("parse devflow run report");
        assert_eq!(loaded_devflow_state.completed_stages.len(), 15);
        assert_eq!(loaded_devflow_report.results.len(), 15);
        assert_eq!(
            loaded_devflow_report
                .results
                .last()
                .map(|result| result.stage_id.as_str()),
            Some("step14")
        );
        let brief_text = std::fs::read_to_string(
            created
                .archive
                .root
                .join("content")
                .join(PROJECT_BRIEF_PATH),
        )
        .expect("read brief");
        let loaded_brief = app
            .load_project_brief(&created.archive)
            .expect("load persisted brief");
        assert!(brief_text.contains("# Game Design Brief"));
        assert_eq!(loaded_brief.title, "Demo");
        assert_eq!(loaded_brief.core_loop.len(), 3);
        let package_manifest_text =
            std::fs::read_to_string(created.archive.root.join("content/package/manifest.adm"))
                .expect("read package manifest");
        assert!(package_manifest_text.contains("support_files="));
        assert!(package_manifest_text.contains("package/build_targets.adm"));
        assert!(package_manifest_text.contains("validation/acceptance_matrix.adm"));
        assert!(package_manifest_text.contains("validation/scenario_test_plan.adm"));
        assert!(package_manifest_text.contains("validation/runtime_validation_report.adm"));
        assert!(package_manifest_text.contains("validation/production_readiness.adm"));
        assert!(package_manifest_text.contains("ai/journal.adm"));
        let build_targets_text = std::fs::read_to_string(
            created
                .archive
                .root
                .join("content/package/build_targets.adm"),
        )
        .expect("read game build targets");
        assert!(build_targets_text.contains("target_id=windows_desktop_playable"));
        assert!(build_targets_text.contains("required_artifacts="));
        assert!(build_targets_text.contains("validation/scenario_test_plan.adm"));
        assert!(build_targets_text.contains("validation/runtime_validation_report.adm"));
        let design_text =
            std::fs::read_to_string(created.archive.root.join("content/design/project.adm"))
                .expect("read design");
        assert!(design_text.contains("## Design Pillars"));
        assert!(design_text.contains("## Gameplay Mechanics"));
        assert!(design_text.contains("## Playable Scenarios"));
        assert!(design_text.contains("## Acceptance Risks"));
        let development_text =
            std::fs::read_to_string(created.archive.root.join("content/development/plan.adm"))
                .expect("read development");
        assert!(development_text.contains("milestone=core_loop_step_1"));
        assert!(development_text.contains("scenario_id=scenario_core_loop_step_1"));
        assert!(development_text.contains("scenario_id=scenario_core_loop_step_3"));
        assert!(development_text.contains("data_contracts=core_loop_step_1.request"));
        assert!(development_text.contains("source_mechanic=Core Loop Mechanic 1"));
        assert!(development_text.contains("validation=Run unit-level state transition check"));
        assert!(development_text.contains("risk_controls=scope_drift | feedback_unclear"));
        let asset_text =
            std::fs::read_to_string(created.archive.root.join("content/assets/plan.adm"))
                .expect("read assets");
        assert!(asset_text.contains("stage=concept"));
        assert!(asset_text.contains("stage=mechanic_feedback"));
        assert!(asset_text.contains("source_mechanic=Core Loop Mechanic 1"));
        assert!(asset_text.contains("source_mechanic=Core Loop Mechanic 3"));
        assert!(asset_text.contains("risk_controls=feedback_unclear"));
        assert!(asset_text.contains("kind=audio_cues"));
        let sdk_text = std::fs::read_to_string(created.archive.root.join("content/sdk/index.adm"))
            .expect("read sdk");
        assert!(sdk_text.contains("## Unity Build Automation SDK"));
        assert!(sdk_text.contains("## Windows Desktop Packaging SDK"));
        assert!(sdk_text.contains("required_for_build=true"));
        assert!(sdk_text.contains("ai_explanation="));
        let validation_text =
            std::fs::read_to_string(created.archive.root.join("content/validation/report.adm"))
                .expect("read validation");
        assert!(!validation_text.contains("code=sdk."));
        let acceptance_matrix_text = std::fs::read_to_string(
            created
                .archive
                .root
                .join("content/validation/acceptance_matrix.adm"),
        )
        .expect("read acceptance matrix");
        assert!(acceptance_matrix_text.contains("# Acceptance Trace Matrix"));
        assert!(acceptance_matrix_text.contains("source_mechanic=Core Loop Mechanic 1"));
        assert!(acceptance_matrix_text.contains("sdk_resources=Unity Build Automation SDK"));
        assert!(acceptance_matrix_text.contains("build_targets=windows_desktop_playable"));
        assert!(acceptance_matrix_text.contains("status=ready"));
        let scenario_test_plan_text = std::fs::read_to_string(
            created
                .archive
                .root
                .join("content/validation/scenario_test_plan.adm"),
        )
        .expect("read scenario test plan");
        assert!(scenario_test_plan_text.contains("# Scenario Test Plan"));
        assert!(scenario_test_plan_text.contains("test_id=test_scenario_core_loop_step_1"));
        assert!(scenario_test_plan_text.contains("scenario_id=scenario_core_loop_step_3"));
        assert!(scenario_test_plan_text.contains("test_type=playable_smoke"));
        assert!(scenario_test_plan_text.contains("telemetry=core_loop_step_1_started"));
        assert!(scenario_test_plan_text.contains("status=ready"));
        let runtime_validation_text = std::fs::read_to_string(
            created
                .archive
                .root
                .join("content/validation/runtime_validation_report.adm"),
        )
        .expect("read runtime validation report");
        assert!(runtime_validation_text.contains("# Runtime Validation Report"));
        assert!(runtime_validation_text.contains("execution_mode=deterministic_runtime_probe"));
        assert!(runtime_validation_text.contains("result_id=runtime_scenario_core_loop_step_1"));
        assert!(runtime_validation_text.contains("acceptance_trace_id=trace_core_loop_step_1"));
        assert!(runtime_validation_text.contains("evidence=static_runtime_contract"));
        assert!(runtime_validation_text.contains("status=ready"));
        let production_readiness_text = std::fs::read_to_string(
            created
                .archive
                .root
                .join("content/validation/production_readiness.adm"),
        )
        .expect("read production readiness");
        assert!(production_readiness_text.contains("# Production Readiness Report"));
        assert!(production_readiness_text.contains("overall_status=ready"));
        assert!(production_readiness_text.contains("check_id=playable_scenario_coverage"));
        assert!(production_readiness_text.contains("check_id=scenario_test_plan_readiness"));
        assert!(production_readiness_text.contains("check_id=runtime_validation_readiness"));
        assert!(
            created
                .archive
                .root
                .join("content/pipeline/run_report.adm")
                .exists()
        );
        assert!(
            report
                .commit
                .written_files
                .iter()
                .any(|path| path.ends_with("package/manifest.adm"))
        );
        assert!(
            report
                .commit
                .written_files
                .iter()
                .any(|path| path.ends_with("package/build_targets.adm"))
        );
        assert!(
            report
                .commit
                .written_files
                .iter()
                .any(|path| path.ends_with("validation/acceptance_matrix.adm"))
        );
        assert!(
            report
                .commit
                .written_files
                .iter()
                .any(|path| path.ends_with("validation/scenario_test_plan.adm"))
        );
        assert!(
            report
                .commit
                .written_files
                .iter()
                .any(|path| path.ends_with("validation/runtime_validation_report.adm"))
        );
        assert!(
            report
                .commit
                .written_files
                .iter()
                .any(|path| path.ends_with("validation/production_readiness.adm"))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn design_brief_from_parts_parses_configurable_core_loop() {
        let brief = design_brief_from_parts(
            "Custom",
            "tactical puzzle",
            "Players solve compact tactical encounters with readable feedback",
            "Scout the room | Plan the move; Resolve the encounter",
        )
        .expect("brief");

        assert_eq!(brief.title, "Custom");
        assert_eq!(brief.genre, "tactical puzzle");
        assert_eq!(
            brief.core_loop,
            vec![
                "Scout the room".to_string(),
                "Plan the move".to_string(),
                "Resolve the encounter".to_string()
            ]
        );
    }

    #[test]
    fn application_reruns_with_persisted_custom_brief() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_persisted_brief_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        let created = app.create_project("Persisted Brief").expect("create");
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let brief = design_brief_from_parts(
            "Persisted Brief",
            "tactical puzzle adventure",
            "Players solve compact tactical routes with readable feedback",
            "Scout the room | Plan a route | Resolve the encounter with feedback",
        )
        .expect("brief");
        app.run_core_pipeline(&created.archive, brief, &provider)
            .expect("pipeline");

        let loaded_brief = app
            .load_project_brief(&created.archive)
            .expect("load persisted brief");
        let report = app
            .rerun_core_pipeline_stage(&created.archive, loaded_brief, &provider, "development")
            .expect("rerun selected stage");
        let design_text =
            std::fs::read_to_string(created.archive.root.join("content/design/project.adm"))
                .expect("read design");

        assert_eq!(report.validation.status, ValidationStatus::Passed);
        assert!(design_text.contains("genre=tactical puzzle adventure"));
        assert!(design_text.contains("1. Scout the room"));
        assert!(design_text.contains("3. Resolve the encounter with feedback"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_commits_engine_build_execution_history() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_engine_history_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        let created = app.create_project("Engine History").expect("create");
        let report = EngineBuildExecutionReport {
            engine: "Unity".to_string(),
            target_id: "windows_desktop_playable".to_string(),
            mode: EngineBuildExecutionMode::DryRun,
            status: EngineBuildExecutionStatus::Succeeded,
            launched: false,
            executable: PathBuf::from("C:/Program Files/Unity/Editor/Unity.exe"),
            working_dir: PathBuf::from("C:/workspace/game"),
            command_line: "Unity.exe -batchmode".to_string(),
            expected_output: "build/windows/AutoDesignMakerGame.zip".to_string(),
            expected_output_path: PathBuf::from(
                "C:/workspace/game/build/windows/AutoDesignMakerGame.zip",
            ),
            expected_output_present: false,
            expected_output_bytes: 0,
            expected_output_hash: None,
            exit_code: None,
            stdout: "dry-run: command not launched".to_string(),
            stderr: String::new(),
        };

        let first = app
            .commit_engine_build_execution(&created.archive, &report)
            .expect("first engine history commit");
        let second = app
            .commit_engine_build_execution(&created.archive, &report)
            .expect("second engine history commit");

        assert_eq!(first.record_count, 1);
        assert_eq!(second.record_count, 2);
        assert_eq!(
            second.history_file,
            PathBuf::from("package/engine_build_history.adm")
        );
        assert!(
            second
                .commit
                .written_files
                .iter()
                .any(|path| { path.ends_with("package/engine_build_history.adm") })
        );
        let history = std::fs::read_to_string(
            created
                .archive
                .root
                .join("content/package/engine_build_history.adm"),
        )
        .expect("read engine build history");
        assert_eq!(history.matches("# Engine Build Execution\n").count(), 2);
        assert!(history.contains("mode=dry_run"));
        assert!(history.contains("launched=false"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_commits_runtime_validation_execution_results() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_runtime_validation_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        let created = app.create_project("Runtime Validation").expect("create");
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        app.run_core_pipeline(
            &created.archive,
            default_demo_brief("Runtime Validation").unwrap(),
            &provider,
        )
        .expect("pipeline");
        let execution = "# Runtime Validation Execution\n\
            runner=unity_playmode\n\
            target_id=windows_desktop_playable\n\
            - result_id=runtime_scenario_core_loop_step_1; scenario_id=scenario_core_loop_step_1; test_id=test_scenario_core_loop_step_1; acceptance_trace_id=trace_core_loop_step_1; telemetry_start_seen=true; telemetry_complete_seen=true; expected_state_seen=true; failure_guard_triggered=false; status=passed\n\
            - result_id=runtime_scenario_core_loop_step_2; scenario_id=scenario_core_loop_step_2; test_id=test_scenario_core_loop_step_2; acceptance_trace_id=trace_core_loop_step_2; telemetry_start_seen=true; telemetry_complete_seen=true; expected_state_seen=true; failure_guard_triggered=false; status=passed\n\
            - result_id=runtime_scenario_core_loop_step_3; scenario_id=scenario_core_loop_step_3; test_id=test_scenario_core_loop_step_3; acceptance_trace_id=trace_core_loop_step_3; telemetry_start_seen=true; telemetry_complete_seen=true; expected_state_seen=true; failure_guard_triggered=false; status=passed\n";

        let commit = app
            .commit_runtime_validation_execution(&created.archive, execution)
            .expect("runtime validation commit");

        assert!(commit.summary.ready());
        assert_eq!(commit.summary.contract_rows, 3);
        assert_eq!(commit.summary.passed_rows, 3);
        assert_eq!(
            commit.results_file,
            PathBuf::from(RUNTIME_EXECUTION_RESULTS_PATH)
        );
        assert!(
            commit
                .commit
                .written_files
                .iter()
                .any(|path| path.ends_with(RUNTIME_EXECUTION_RESULTS_PATH))
        );
        let results = std::fs::read_to_string(
            created
                .archive
                .root
                .join("content")
                .join(RUNTIME_EXECUTION_RESULTS_PATH),
        )
        .expect("read runtime execution results");
        assert!(results.contains("# Runtime Validation Execution Results"));
        assert!(results.contains("ready=true"));
        assert!(results.contains("source_hash="));
        assert!(results.contains("status=ready"));
        let readiness = std::fs::read_to_string(
            created
                .archive
                .root
                .join("content/validation/production_readiness.adm"),
        )
        .expect("read production readiness");
        assert!(readiness.contains("overall_status=ready"));
        assert!(readiness.contains("validation/runtime_execution_results.adm"));
        assert!(
            readiness.contains(
                "check_id=runtime_validation_readiness; status=ready; expected=3; actual=3"
            )
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_resumes_failed_core_pipeline_from_saved_state() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_resume_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        let created = app.create_project("Resume").expect("create");
        let open = app
            .archives
            .open_archive_workspace(&created.archive, SessionId::generate())
            .expect("workspace");
        let mut failed_state = PipelineRunState::new(RunId::new("run_resume_app").unwrap());
        failed_state.complete_stage(StageId::new("design").unwrap(), "design done");
        failed_state.complete_stage(StageId::new("development").unwrap(), "development done");
        failed_state.fail("asset plan failed");
        app.archives
            .write_workspace_text(
                &open.workspace,
                "pipeline/run_state.adm",
                &failed_state.render(),
            )
            .expect("write failed state");
        app.archives
            .commit_workspace(&created.archive, &open.workspace, &open.lock)
            .expect("commit failed state");
        drop(open);

        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let report = app
            .resume_core_pipeline(
                &created.archive,
                default_demo_brief("Resume").unwrap(),
                &provider,
            )
            .expect("resume");

        assert_eq!(report.validation.status, ValidationStatus::Passed);
        assert_eq!(report.pipeline_report.results.len(), 3);
        assert_eq!(
            report.pipeline_report.results[0].stage_id,
            StageId::new("assets").unwrap()
        );
        assert_eq!(report.run_state.completed_stages.len(), 5);
        assert_eq!(report.artifact_registry.records().len(), 26);
        assert!(
            created
                .archive
                .root
                .join("content/pipeline/step14/stage.adm")
                .exists()
        );
        let loaded_state = PipelineRunState::from_state_text(
            &std::fs::read_to_string(created.archive.root.join("content/pipeline/run_state.adm"))
                .expect("read run state"),
        )
        .expect("parse run state");
        assert_eq!(loaded_state.completed_stages.len(), 5);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_reruns_selected_core_pipeline_stage_from_saved_state() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_rerun_stage_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        let created = app.create_project("Rerun Stage").expect("create");
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        app.run_core_pipeline(
            &created.archive,
            default_demo_brief("Rerun Stage").unwrap(),
            &provider,
        )
        .expect("pipeline");

        let report = app
            .rerun_core_pipeline_stage(
                &created.archive,
                default_demo_brief("Rerun Stage").unwrap(),
                &provider,
                "development",
            )
            .expect("rerun selected stage");

        assert_eq!(report.validation.status, ValidationStatus::Passed);
        assert_eq!(
            report
                .pipeline_report
                .results
                .iter()
                .map(|result| result.stage_id.as_str())
                .collect::<Vec<_>>(),
            vec!["development", "sdk", "packaging"]
        );
        assert_eq!(report.run_state.completed_stages.len(), 5);
        assert_eq!(report.artifact_registry.records().len(), 26);
        assert_eq!(report.ai_journal.records().len(), 1);
        let persisted_report =
            std::fs::read_to_string(created.archive.root.join("content/pipeline/run_report.adm"))
                .expect("read run report");
        assert!(persisted_report.contains("stage_id=development"));
        assert!(!persisted_report.contains("stage_id=assets"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_resumes_failed_core_pipeline_from_run_report_stage() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_resume_failed_report_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        let created = app.create_project("Resume Failed").expect("create");
        let open = app
            .archives
            .open_archive_workspace(&created.archive, SessionId::generate())
            .expect("workspace");
        let mut failed_state = PipelineRunState::new(RunId::new("run_resume_report").unwrap());
        for stage in ["design", "development", "assets"] {
            failed_state.complete_stage(StageId::new(stage).unwrap(), "done");
        }
        failed_state.fail("sdk failed");
        let failed_report = PipelineRunReport {
            results: vec![
                adm_pipeline::StageRunResult {
                    stage_id: StageId::new("design").unwrap(),
                    status: adm_pipeline::StageRunStatus::Succeeded,
                    artifacts: Vec::new(),
                    message: "design done".to_string(),
                },
                adm_pipeline::StageRunResult {
                    stage_id: StageId::new("development").unwrap(),
                    status: adm_pipeline::StageRunStatus::Succeeded,
                    artifacts: Vec::new(),
                    message: "development done".to_string(),
                },
                adm_pipeline::StageRunResult {
                    stage_id: StageId::new("assets").unwrap(),
                    status: adm_pipeline::StageRunStatus::Succeeded,
                    artifacts: Vec::new(),
                    message: "assets done".to_string(),
                },
                adm_pipeline::StageRunResult {
                    stage_id: StageId::new("sdk").unwrap(),
                    status: adm_pipeline::StageRunStatus::Failed,
                    artifacts: Vec::new(),
                    message: "sdk failed".to_string(),
                },
            ],
        };
        app.archives
            .write_workspace_text(
                &open.workspace,
                "pipeline/run_state.adm",
                &failed_state.render(),
            )
            .expect("write failed state");
        app.archives
            .write_workspace_text(
                &open.workspace,
                "pipeline/run_report.adm",
                &failed_report.render(),
            )
            .expect("write failed report");
        app.archives
            .commit_workspace(&created.archive, &open.workspace, &open.lock)
            .expect("commit failed state");
        drop(open);

        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let report = app
            .resume_failed_core_pipeline(
                &created.archive,
                default_demo_brief("Resume Failed").unwrap(),
                &provider,
            )
            .expect("resume failed");

        assert_eq!(report.validation.status, ValidationStatus::Passed);
        assert_eq!(
            report
                .pipeline_report
                .results
                .iter()
                .map(|result| result.stage_id.as_str())
                .collect::<Vec<_>>(),
            vec!["sdk", "packaging"]
        );
        assert_eq!(report.run_state.completed_stages.len(), 5);
        assert_eq!(report.artifact_registry.records().len(), 26);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_persists_config_and_reports_ai_diagnostics() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_config_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        let config_path = app.config().config_file_path();
        let diagnostics = app.ai_diagnostics();

        assert!(config_path.exists());
        assert_eq!(diagnostics.ready_provider_count(), 1);
        assert!(diagnostics.render().contains("mock"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_updates_ai_provider_profile() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_ai_provider_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let mut app = AdmApplication::for_data_root(&root).expect("app");
        let config_path = app
            .upsert_ai_provider(AiProviderConfig::enabled(
                ProviderId::new("openai").unwrap(),
                Some("https://api.openai.com/v1".to_string()),
                Some(adm_config::SecretRef::env_var("ADM_MISSING_OPENAI_KEY").unwrap()),
            ))
            .expect("upsert provider");
        let diagnostics = app.ai_diagnostics();

        assert!(config_path.exists());
        assert!(diagnostics.render().contains("openai"));
        assert!(diagnostics.render().contains("MissingSecret"));

        app.disable_ai_provider(ProviderId::new("openai").unwrap())
            .expect("disable provider");
        let diagnostics = app.ai_diagnostics();

        assert!(diagnostics.render().contains("Disabled"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[derive(Debug, Clone)]
    struct ApplicationRemoteTransport;

    impl AiRemoteTransport for ApplicationRemoteTransport {
        fn send(&self, request: &AiRemoteRequest) -> AdmResult<AiRemoteResponse> {
            assert_eq!(request.provider_id().as_str(), "remote_runtime");
            assert_eq!(request.endpoint_hint(), "https://example.invalid/v1");
            assert!(!request.secret().expose_secret().is_empty());
            Ok(AiRemoteResponse::new(
                "application factory remote response with enough content",
            ))
        }
    }

    #[test]
    fn application_builds_runtime_remote_provider_from_config() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_runtime_provider_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let mut app = AdmApplication::for_data_root(&root).expect("app");
        app.upsert_ai_provider(AiProviderConfig::enabled(
            ProviderId::new("remote_runtime").unwrap(),
            Some("https://example.invalid/v1".to_string()),
            Some(adm_config::SecretRef::env_var("PATH").unwrap()),
        ))
        .expect("upsert provider");

        let provider = app
            .remote_ai_provider_from_config(
                &ProviderId::new("remote_runtime").unwrap(),
                ApplicationRemoteTransport,
            )
            .expect("runtime provider");
        let request = AiTaskRequest::new(AiCapability::TextGeneration, "draft", "ctx").unwrap();
        let result = provider.run(&request).expect("run provider");
        let profile_text =
            std::fs::read_to_string(app.config().config_file_path()).expect("profile text");

        assert_eq!(
            result.raw_output,
            "application factory remote response with enough content"
        );
        assert!(profile_text.contains("env:PATH"));
        assert!(profile_text.contains("capabilities=text_generation"));
        assert!(!profile_text.contains(std::env::var("PATH").unwrap().as_str()));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_builds_runtime_remote_provider_from_named_secret() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_named_secret_provider_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let mut app = AdmApplication::for_data_root(&root).expect("app");
        let secret_path = app
            .upsert_named_secret("openai", "named-runtime-secret")
            .expect("save named secret");
        app.upsert_ai_provider(AiProviderConfig::enabled(
            ProviderId::new("remote_runtime").unwrap(),
            Some("https://example.invalid/v1".to_string()),
            Some(adm_config::SecretRef::named("openai").unwrap()),
        ))
        .expect("upsert provider");

        let diagnostics = app.ai_diagnostics();
        let provider = app
            .remote_ai_provider_from_config(
                &ProviderId::new("remote_runtime").unwrap(),
                ApplicationRemoteTransport,
            )
            .expect("runtime provider");
        let request = AiTaskRequest::new(AiCapability::TextGeneration, "draft", "ctx").unwrap();
        let result = provider.run(&request).expect("run provider");
        let profile_text =
            std::fs::read_to_string(app.config().config_file_path()).expect("profile text");

        assert!(secret_path.exists());
        assert!(diagnostics.render().contains("remote_runtime"));
        assert!(diagnostics.render().contains("Ready"));
        assert!(diagnostics.render().contains("named:openai"));
        assert_eq!(
            result.raw_output,
            "application factory remote response with enough content"
        );
        assert!(profile_text.contains("named:openai"));
        assert!(!profile_text.contains("named-runtime-secret"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_builds_chat_completions_provider_from_config_without_network() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_chat_provider_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let mut app = AdmApplication::for_data_root(&root).expect("app");
        app.upsert_ai_provider(AiProviderConfig::enabled(
            ProviderId::new("openai_compatible").unwrap(),
            Some("https://example.invalid/v1".to_string()),
            Some(adm_config::SecretRef::env_var("PATH").unwrap()),
        ))
        .expect("upsert provider");

        let provider = app
            .chat_completions_provider_from_config(
                &ProviderId::new("openai_compatible").unwrap(),
                "gpt-test",
            )
            .expect("chat provider");

        assert!(provider.supports(&AiCapability::TextGeneration));
        assert!(!provider.supports(&AiCapability::ImageGeneration));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_lists_and_exports_projects() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_list_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        let created = app.create_project("Listed").expect("create");
        let projects = app.list_projects().expect("projects");
        let target = root.join("listed.admproj");
        let exported = app
            .export_project(created.archive.manifest.archive_id.as_str(), &target)
            .expect("export");
        let inspection = app
            .inspect_project_package(&target)
            .expect("package inspect");

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].display_name, "Listed");
        assert_eq!(exported, target);
        assert!(exported.exists());
        assert!(inspection.ready());
        assert_eq!(
            inspection
                .manifest
                .as_ref()
                .map(|manifest| manifest.display_name.as_str()),
            Some("Listed")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_reports_and_cleans_stale_workspaces() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_workspace_cleanup_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        app.create_project("Workspace Cleanup").expect("create");

        let before = app.inspect_workspaces().expect("inspect");
        assert_eq!(before.active_count(), 0);
        assert!(before.stale_count() >= 1);

        let cleanup = app.cleanup_stale_workspaces().expect("cleanup");
        assert!(cleanup.removed_count() >= 1);
        assert_eq!(cleanup.skipped_active_count(), 0);

        let after = app.inspect_workspaces().expect("inspect after");
        assert_eq!(after.workspace_count(), 0);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_imports_project_packages() {
        let source_root = std::env::temp_dir().join(format!(
            "adm_application_import_source_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let target_root = std::env::temp_dir().join(format!(
            "adm_application_import_target_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let source_app = AdmApplication::for_data_root(&source_root).expect("source app");
        let target_app = AdmApplication::for_data_root(&target_root).expect("target app");
        let created = source_app.create_project("Imported").expect("create");
        let package = source_root.join("imported.admproj");
        source_app
            .export_project(created.archive.manifest.archive_id.as_str(), &package)
            .expect("export");

        let imported = target_app.import_project(&package).expect("import");
        assert_eq!(imported.display_name, "Imported");
        assert_eq!(target_app.list_projects().unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(source_root);
        let _ = std::fs::remove_dir_all(target_root);
    }

    #[test]
    fn application_commits_and_loads_design_workbench_state() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_workbench_archive_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        let mut service =
            WorkbenchService::load(&fixture_design_data_root()).expect("workbench service");
        service.set_project_name("正式工作台存档");
        let markdown = service.export_text("markdown").expect("markdown");

        let saved = app
            .commit_design_workbench_state(None, "正式工作台存档", service.state(), &markdown)
            .expect("commit workbench");
        let loaded = app
            .load_design_workbench_state(saved.archive.manifest.archive_id.as_str())
            .expect("load workbench");

        assert_eq!(loaded.project_name, "正式工作台存档");
        assert!(
            saved
                .archive
                .root
                .join("content")
                .join(&saved.state_file)
                .exists()
        );
        assert!(
            saved
                .archive
                .root
                .join("content")
                .join(&saved.export_file)
                .exists()
        );
        assert!(
            saved
                .commit
                .written_files
                .iter()
                .any(|path| path.ends_with(DESIGN_WORKBENCH_STATE_PATH))
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn application_commits_supplement_analysis_to_archive() {
        let root = std::env::temp_dir().join(format!(
            "adm_application_supplement_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let app = AdmApplication::for_data_root(&root).expect("app");
        let created = app.create_project("Supplement").expect("create");
        let analysis =
            analyze_supplement_request("增加 Unity 构建验证和 UI 动画", "pipeline ready")
                .expect("analysis");

        let commit = app
            .commit_supplement_analysis(created.archive.manifest.archive_id.as_str(), &analysis)
            .expect("commit supplement");
        let saved = std::fs::read_to_string(
            created
                .archive
                .root
                .join("content")
                .join(SUPPLEMENT_REQUESTS_PATH),
        )
        .expect("read supplement");

        assert_eq!(commit.task_count, 3);
        assert!(commit.request_file.ends_with(SUPPLEMENT_REQUESTS_PATH));
        assert!(saved.contains("area=assets"));
        assert!(saved.contains("area=sdk"));
        assert!(saved.contains("area=packaging"));

        let _ = std::fs::remove_dir_all(root);
    }

    fn fixture_design_data_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("knowledge")
            .join("design_data")
    }
}
