use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::UNIX_EPOCH;

use adm_new_ai::adapters::{
    ClaudeCliAdapter, CodexCliAdapter, blocking_openai_adapter_from_resolved,
};
use adm_new_ai::resolution::resolve_active_ai_target;
use adm_new_ai::{AiAdapterKind, AiConfigCategory, CompletionAdapter};
use adm_new_application::runtime::RuntimeApplicationService;
use adm_new_application::{
    AiConfigApplicationService, AiInterviewApplicationService, DesignWorkbenchService,
    PackagingApplicationService, PatchApplicationService, PipelineApplicationService,
    RunLogService, SaveApplicationService, SdkKnowledgeApplicationService,
    SkillOverlayApplicationService,
};
use adm_new_contracts::ai::{ModelResultStatus, ModelTask};
use adm_new_contracts::log::{LogEntry, LogLevel};
use adm_new_contracts::pipeline::{
    PipelineCheckpoint, PipelineCheckpointStatus, PipelineRecoverySummary, PipelineResumePolicy,
    PipelineRunState, PipelineUnitStatus,
};
use adm_new_contracts::project::ProjectState;
use adm_new_design::data_loader::DesignDataLoader;
use adm_new_foundation::new_stable_id;
use adm_new_foundation::{AdmError, AdmResult, unix_timestamp};
use adm_new_pipeline::{ProductPipelineExecutor, default_development_registry};
use adm_new_storage::PipelineCheckpointRepository;
use adm_new_tauri_commands::shell::{UI_LANGUAGE_ENV, UiLanguage, normalize_ui_language};
use adm_new_tauri_commands::{CommandAdapterResult, command_error, command_failure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::design_specs::{load_design_specs, load_design_specs_from_portable_root};

pub const DESKTOP_SESSION_ID: &str = "desktop_current";
const DESKTOP_SESSION_PREFIX: &str = "desktop_";
const DESKTOP_SESSION_LOCK_DIR: &str = ".session_locks";
const LIFECYCLE_IDLE: u8 = 0;
const LIFECYCLE_PIPELINE_RUNNING: u8 = 1;
const LIFECYCLE_EXITING: u8 = 2;
const LIFECYCLE_PIPELINE_EXITING: u8 = 3;

pub struct AppRuntime {
    inner: Mutex<RuntimeState>,
    pipeline_stop: Arc<AtomicBool>,
    lifecycle: Arc<AtomicU8>,
    shutdown_started: AtomicBool,
    _session_lease: DesktopSessionLease,
}

struct DesktopSessionLease {
    session_id: String,
    #[cfg(test)]
    lock_path: PathBuf,
    lock_file: File,
    needs_initial_state: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DesktopSessionLock {
    session_id: String,
    pid: u32,
    acquired_at: String,
}

struct ProjectRestore {
    state: ProjectState,
    warnings: Vec<String>,
}

struct PipelineRestore {
    state: PipelineRunState,
    warnings: Vec<String>,
    needs_rewrite: bool,
}

pub struct PipelineRunGuard {
    lifecycle: Arc<AtomicU8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitDisposition {
    ShutdownNow,
    WaitForPipeline,
    AlreadyExiting,
}

impl Drop for PipelineRunGuard {
    fn drop(&mut self) {
        loop {
            let current = self.lifecycle.load(Ordering::SeqCst);
            let next = match current {
                LIFECYCLE_PIPELINE_RUNNING => LIFECYCLE_IDLE,
                LIFECYCLE_PIPELINE_EXITING => LIFECYCLE_EXITING,
                _ => return,
            };
            if self
                .lifecycle
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return;
            }
        }
    }
}

impl DesktopSessionLease {
    fn acquire(data_root: &Path) -> AdmResult<Self> {
        let drafts_root = data_root.join("drafts");
        fs::create_dir_all(&drafts_root)?;
        let lock_root = drafts_root.join(DESKTOP_SESSION_LOCK_DIR);
        fs::create_dir_all(&lock_root)?;
        let mut candidates = Vec::new();
        if let Ok(entries) = fs::read_dir(&drafts_root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(session_id) = entry.file_name().to_str().map(str::to_string) else {
                    continue;
                };
                if session_id != DESKTOP_SESSION_ID
                    && !session_id.starts_with(DESKTOP_SESSION_PREFIX)
                {
                    continue;
                }
                candidates.push((desktop_draft_modified_at(&path), session_id));
            }
        }
        if !candidates
            .iter()
            .any(|(_, session_id)| session_id == DESKTOP_SESSION_ID)
        {
            candidates.push((0, DESKTOP_SESSION_ID.to_string()));
        }
        candidates.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        for (_, session_id) in candidates {
            if let Some(lease) = Self::try_acquire(&drafts_root, &lock_root, &session_id)? {
                return Ok(lease);
            }
        }

        let pid = std::process::id();
        for attempt in 0..32_u32 {
            let session_id = format!("desktop_{pid}_{}_{}", unix_timestamp(), attempt);
            if let Some(lease) = Self::try_acquire(&drafts_root, &lock_root, &session_id)? {
                return Ok(lease);
            }
        }
        Err(AdmError::new(
            "failed to allocate an independent desktop draft session",
        ))
    }

    fn try_acquire(
        drafts_root: &Path,
        lock_root: &Path,
        session_id: &str,
    ) -> AdmResult<Option<Self>> {
        let session_id = adm_new_foundation::sanitize_identifier(session_id)?;
        let session_dir = drafts_root.join(&session_id);
        fs::create_dir_all(&session_dir)?;
        let lock_path = lock_root.join(format!("{session_id}.lock"));
        let needs_initial_state = !session_dir.join("draft_meta.json").is_file()
            && !session_dir.join("autosave_state.json").is_file();
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&lock_path)?;
        match file.try_lock_exclusive() {
            Ok(()) => {}
            Err(error) if is_file_lock_contention(&error) => return Ok(None),
            Err(error) => return Err(error.into()),
        }
        let payload = DesktopSessionLock {
            session_id: session_id.clone(),
            pid: std::process::id(),
            acquired_at: format!("unix:{}", unix_timestamp()),
        };
        let write_result = (|| -> AdmResult<()> {
            let text = serde_json::to_string_pretty(&payload).map_err(|error| {
                AdmError::new(format!("failed to serialize desktop session lock: {error}"))
            })?;
            file.set_len(0)?;
            file.seek(SeekFrom::Start(0))?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            Ok(())
        })();
        if let Err(error) = write_result {
            let _ = file.unlock();
            return Err(error);
        }
        Ok(Some(Self {
            session_id,
            #[cfg(test)]
            lock_path,
            lock_file: file,
            needs_initial_state,
        }))
    }
}

fn is_file_lock_contention(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock
        || matches!(error.raw_os_error(), Some(11 | 32 | 33 | 36))
}

impl Drop for DesktopSessionLease {
    fn drop(&mut self) {
        let _ = self.lock_file.unlock();
    }
}

pub struct RuntimeState {
    pub data_root: PathBuf,
    pub source_root: Option<PathBuf>,
    pub ui_language: UiLanguage,
    pub project_state: ProjectState,
    pub design: DesignWorkbenchService,
    pub save: SaveApplicationService,
    pub ai_config: AiConfigApplicationService,
    pub runtime_config: RuntimeApplicationService,
    pub ai_interview: AiInterviewApplicationService,
    pub pipeline: PipelineApplicationService,
    pub pipeline_executor: ProductPipelineExecutor,
    pub pipeline_state: PipelineRunState,
    pub packaging: PackagingApplicationService,
    pub patch: PatchApplicationService,
    pub sdk: SdkKnowledgeApplicationService,
    pub skills: SkillOverlayApplicationService,
    pub logs: RunLogService,
    pub last_package_result: Option<adm_new_tauri_commands::package::PackageRunResultView>,
}

#[derive(Debug, Clone)]
pub struct CompletionRunner {
    ai_config: AiConfigApplicationService,
    data_root: PathBuf,
}

impl AppRuntime {
    pub fn new(data_root: impl AsRef<Path>) -> AdmResult<Self> {
        let language_value = std::env::var(UI_LANGUAGE_ENV).ok();
        let ui_language = normalize_ui_language(language_value.as_deref());
        Self::new_with_ui_language_and_resource_root(data_root, ui_language, None)
    }

    #[cfg(test)]
    pub(crate) fn new_with_ui_language(
        data_root: impl AsRef<Path>,
        ui_language: UiLanguage,
    ) -> AdmResult<Self> {
        Self::new_with_ui_language_and_resource_root(data_root, ui_language, None)
    }

    pub(crate) fn new_with_portable_root(
        data_root: impl AsRef<Path>,
        portable_root: impl AsRef<Path>,
    ) -> AdmResult<Self> {
        let language_value = std::env::var(UI_LANGUAGE_ENV).ok();
        let ui_language = normalize_ui_language(language_value.as_deref());
        Self::new_with_ui_language_and_resource_root(
            data_root,
            ui_language,
            Some(portable_root.as_ref().to_path_buf()),
        )
    }

    fn new_with_ui_language_and_resource_root(
        data_root: impl AsRef<Path>,
        ui_language: UiLanguage,
        portable_root: Option<PathBuf>,
    ) -> AdmResult<Self> {
        let data_root = data_root.as_ref().to_path_buf();
        fs::create_dir_all(&data_root)?;
        let session_lease = DesktopSessionLease::acquire(&data_root)?;
        let session_id = session_lease.session_id.clone();
        let loaded_specs = portable_root
            .as_deref()
            .map(load_design_specs_from_portable_root)
            .unwrap_or_else(load_design_specs)?;
        let resource_root = loaded_specs.resource_root.clone();
        let sdk = SdkKnowledgeApplicationService::open(&resource_root, &data_root)?;
        let sdk_migration = sdk.legacy_migration().clone();
        let skills = SkillOverlayApplicationService::open(&resource_root, &data_root)?;
        let skill_count = skills.list()?.len();
        let template_runtime_root = data_root.join("drafts").join(&session_id);
        let template_loader = DesignDataLoader::new(&loaded_specs.resource_root)
            .with_runtime_root(template_runtime_root);
        let design = DesignWorkbenchService::new(loaded_specs.specs.clone())
            .with_template_loader(template_loader);
        let save = SaveApplicationService::new(&data_root, &session_id)?;
        if session_lease.needs_initial_state && session_id != DESKTOP_SESSION_ID {
            save.autosave(&design.empty_project_state())?;
        }
        let restored = restore_project_state(&save, &design, &data_root, &session_id)?;
        let project_state = restored.state;
        let runtime_config = RuntimeApplicationService::new(&data_root, &session_id)?;
        let pipeline_executor = ProductPipelineExecutor::with_design_data_dir(
            &data_root,
            &session_id,
            loaded_specs.resource_root.join("knowledge/design_data"),
        )?
        .require_protocol_gate();
        let pipeline = PipelineApplicationService::new(default_development_registry())?;
        let pipeline_restore = load_pipeline_state(&runtime_config)?;
        let pipeline_state = pipeline_restore.state;
        let mut state = RuntimeState {
            data_root: data_root.clone(),
            source_root: Some(resource_root),
            ui_language,
            project_state,
            design,
            save,
            ai_config: AiConfigApplicationService::new(&data_root)?,
            runtime_config,
            ai_interview: AiInterviewApplicationService::new(loaded_specs.specs),
            pipeline,
            pipeline_executor,
            pipeline_state,
            packaging: PackagingApplicationService::new(),
            patch: PatchApplicationService::new(),
            sdk,
            skills,
            logs: RunLogService::new(),
            last_package_result: None,
        };
        state.reload_package_result();
        state.reload_patch_records();
        state.load_logs();
        if sdk_migration.migrated {
            state.write_log(
                LogLevel::Info,
                "sdk.migration",
                &format!(
                    "migrated {} legacy SDK specs into the shared overlay; archive={}",
                    sdk_migration.migrated_ids.len(),
                    sdk_migration
                        .archive_path
                        .as_deref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_default()
                ),
            );
        }
        state.write_log(
            LogLevel::Info,
            "skills.startup",
            &format!("validated {skill_count} merged Skill descriptors"),
        );
        if pipeline_restore.needs_rewrite
            && let Err(error) = state.persist_pipeline_state()
        {
            state.write_log(
                LogLevel::Error,
                "pipeline.recovery",
                &format!("failed to rewrite recovered pipeline state: {error}"),
            );
        }
        for warning in pipeline_restore.warnings {
            state.write_log(LogLevel::Warning, "pipeline.recovery", &warning);
        }
        for warning in restored.warnings {
            state.write_log(LogLevel::Warning, "save.recovery", &warning);
        }
        state.write_log(LogLevel::Info, "startup", "desktop runtime initialized");
        Ok(Self {
            inner: Mutex::new(state),
            pipeline_stop: Arc::new(AtomicBool::new(false)),
            lifecycle: Arc::new(AtomicU8::new(LIFECYCLE_IDLE)),
            shutdown_started: AtomicBool::new(false),
            _session_lease: session_lease,
        })
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, RuntimeState>, AdmError> {
        self.inner
            .lock()
            .map_err(|_| AdmError::new("desktop runtime state lock is poisoned"))
    }

    pub fn pipeline_stop_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.pipeline_stop)
    }

    pub fn request_pipeline_stop(&self) {
        self.pipeline_stop.store(true, Ordering::SeqCst);
    }

    pub fn try_begin_pipeline_run(&self) -> Option<PipelineRunGuard> {
        if self.lifecycle.load(Ordering::SeqCst) != LIFECYCLE_IDLE {
            return None;
        }
        self.lifecycle
            .compare_exchange(
                LIFECYCLE_IDLE,
                LIFECYCLE_PIPELINE_RUNNING,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .ok()
            .map(|_| {
                self.pipeline_stop.store(false, Ordering::SeqCst);
                PipelineRunGuard {
                    lifecycle: Arc::clone(&self.lifecycle),
                }
            })
    }

    pub fn pipeline_is_running(&self) -> bool {
        matches!(
            self.lifecycle.load(Ordering::SeqCst),
            LIFECYCLE_PIPELINE_RUNNING | LIFECYCLE_PIPELINE_EXITING
        )
    }

    pub fn begin_exit(&self) -> ExitDisposition {
        loop {
            let current = self.lifecycle.load(Ordering::SeqCst);
            let (next, disposition) = match current {
                LIFECYCLE_IDLE => (LIFECYCLE_EXITING, ExitDisposition::ShutdownNow),
                LIFECYCLE_PIPELINE_RUNNING => {
                    (LIFECYCLE_PIPELINE_EXITING, ExitDisposition::WaitForPipeline)
                }
                LIFECYCLE_EXITING | LIFECYCLE_PIPELINE_EXITING => {
                    return ExitDisposition::AlreadyExiting;
                }
                _ => return ExitDisposition::AlreadyExiting,
            };
            if self
                .lifecycle
                .compare_exchange(current, next, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return disposition;
            }
        }
    }

    pub fn shutdown_once(&self) -> AdmResult<()> {
        if self
            .shutdown_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }
        let result = match self.lock() {
            Ok(mut runtime) => runtime.shutdown(),
            Err(error) => Err(error),
        };
        if result.is_err() {
            self.shutdown_started.store(false, Ordering::SeqCst);
            let _ = self.lifecycle.compare_exchange(
                LIFECYCLE_EXITING,
                LIFECYCLE_IDLE,
                Ordering::SeqCst,
                Ordering::SeqCst,
            );
        }
        result
    }
}

impl RuntimeState {
    pub fn persist_project_state(&mut self, context: &str) -> AdmResult<()> {
        self.save.autosave(&self.project_state)?;
        self.write_log(LogLevel::Info, context, "project state autosaved");
        Ok(())
    }

    pub fn replace_project_state(&mut self, state: ProjectState, context: &str) -> AdmResult<()> {
        self.project_state = self.design.normalize_project_state(state);
        self.persist_project_state(context)
    }

    pub fn persist_pipeline_state(&self) -> AdmResult<()> {
        let text = serde_json::to_string_pretty(&self.pipeline_state).map_err(|error| {
            AdmError::new(format!("failed to serialize pipeline state: {error}"))
        })?;
        adm_new_foundation::write_text_atomic(&self.pipeline_state_file(), &text)?;
        adm_new_foundation::write_text_atomic(&self.durable_pipeline_state_file(), &text)
    }

    pub fn reload_pipeline_state(&mut self) {
        match load_pipeline_state(&self.runtime_config) {
            Ok(restored) => {
                self.pipeline_state = restored.state;
                if restored.needs_rewrite
                    && let Err(error) = self.persist_pipeline_state()
                {
                    self.write_log(
                        LogLevel::Error,
                        "pipeline.recovery",
                        &format!("failed to rewrite recovered pipeline state: {error}"),
                    );
                }
                for warning in restored.warnings {
                    self.write_log(LogLevel::Warning, "pipeline.recovery", &warning);
                }
            }
            Err(error) => {
                self.pipeline_state = empty_pipeline_state();
                self.write_log(
                    LogLevel::Error,
                    "pipeline.recovery",
                    &format!("failed to inspect persisted pipeline state: {error}"),
                );
            }
        }
    }

    pub fn reload_logs(&mut self) {
        self.logs.clear();
        self.load_logs();
    }

    pub fn reset_save_scoped_services(&mut self) {
        self.reload_package_result();
        self.reload_patch_records();
    }

    pub fn package_output_dir(&self) -> PathBuf {
        self.runtime_config
            .paths()
            .outputs_dir
            .join("package")
            .join("current")
    }

    pub fn persist_package_result(
        &mut self,
        result: &adm_new_tauri_commands::package::PackageRunResultView,
    ) -> AdmResult<()> {
        let output_dir = self.package_output_dir();
        let result_value = serde_json::to_value(result).map_err(|error| {
            AdmError::new(format!("failed to serialize package result: {error}"))
        })?;
        let validation_value =
            serde_json::to_value(&result.validation_report).map_err(|error| {
                AdmError::new(format!("failed to serialize package validation: {error}"))
            })?;
        let build_value = serde_json::to_value(&result.build_report).map_err(|error| {
            AdmError::new(format!("failed to serialize package build report: {error}"))
        })?;
        let manifest_value = serde_json::to_value(&result.manifest).map_err(|error| {
            AdmError::new(format!("failed to serialize package manifest: {error}"))
        })?;
        adm_new_foundation::io::write_json(
            &output_dir.join("package_run_result.json"),
            &result_value,
        )?;
        adm_new_foundation::io::write_json(
            &output_dir.join("package_validation_report.json"),
            &validation_value,
        )?;
        adm_new_foundation::io::write_json(&output_dir.join("build_report.json"), &build_value)?;
        adm_new_foundation::io::write_json(
            &output_dir.join("package_manifest.json"),
            &manifest_value,
        )?;
        adm_new_foundation::io::write_text(
            &output_dir.join("PACKAGE_NOTES.md"),
            &result.package_notes,
        )?;
        self.last_package_result = Some(result.clone());
        Ok(())
    }

    pub fn reload_package_result(&mut self) {
        self.last_package_result =
            fs::read_to_string(self.package_output_dir().join("package_run_result.json"))
                .ok()
                .and_then(|text| serde_json::from_str(&text).ok());
    }

    pub fn invalidate_package_result(&mut self) -> AdmResult<()> {
        self.last_package_result = None;
        let output_dir = self.package_output_dir();
        if output_dir.exists() {
            fs::remove_dir_all(output_dir)?;
        }
        Ok(())
    }

    pub fn persist_patch_records(&self) -> AdmResult<()> {
        let value = serde_json::to_value(self.patch.list()).map_err(|error| {
            AdmError::new(format!("failed to serialize patch records: {error}"))
        })?;
        adm_new_foundation::io::write_json(&self.patch_store_file(), &value)?;
        Ok(())
    }

    pub fn reload_patch_records(&mut self) {
        let records =
            adm_new_foundation::io::read_json(&self.patch_store_file(), serde_json::json!([]));
        self.patch
            .replace_records(serde_json::from_value(records).unwrap_or_else(|_| Vec::new()));
    }

    pub fn persist_sdk_specs(&mut self) -> AdmResult<()> {
        self.sdk.persist()
    }

    pub fn reload_sdk_specs(&mut self) -> AdmResult<()> {
        self.sdk.reload()
    }

    fn patch_store_file(&self) -> PathBuf {
        self.runtime_config
            .paths()
            .patches_dir
            .join("desktop_patch_records.json")
    }

    pub fn pipeline_state_file(&self) -> PathBuf {
        self.runtime_config
            .paths()
            .runtime_control_dir
            .join("pipeline_state.json")
    }

    pub fn durable_pipeline_state_file(&self) -> PathBuf {
        self.runtime_config
            .paths()
            .outputs_dir
            .join("pipeline_state.json")
    }

    pub fn shutdown(&mut self) -> AdmResult<()> {
        crate::commands::save::settle_pending_execution_object_ownership(
            self,
            "execution_object_ownership_recovery_before_shutdown",
        )?;
        match self.save.current_draft_save_id()? {
            Some(_) => (|| -> AdmResult<()> {
                self.save
                    .sync_current_save(&self.project_state, "shutdown")?;
                Ok(())
            })(),
            None => self.save.autosave(&self.project_state),
        }
        .map_err(|error| AdmError::new(format!("shutdown persistence failed: {error}")))?;

        self.save
            .release_current_lock()
            .map_err(|error| AdmError::new(format!("shutdown lock release failed: {error}")))?;
        self.write_log(
            LogLevel::Info,
            "shutdown",
            "desktop runtime stopped cleanly",
        );
        Ok(())
    }

    pub fn completion_runner(&self) -> CompletionRunner {
        CompletionRunner {
            ai_config: self.ai_config.clone(),
            data_root: self.data_root.clone(),
        }
    }

    pub fn write_log(&mut self, level: LogLevel, context: &str, message: &str) {
        let entry = LogEntry {
            timestamp: format!("unix:{}", unix_timestamp()),
            level,
            context: context.to_string(),
            message: safe_log_message(message),
            source: "desktop-tauri".to_string(),
            metadata: BTreeMap::new(),
        };
        self.logs.write(entry.clone());
        let path = self.log_file();
        let persisted = (|| -> AdmResult<()> {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let line = serde_json::to_string(&entry).map_err(|error| {
                AdmError::new(format!("failed to serialize log entry: {error}"))
            })?;
            let mut file = OpenOptions::new().create(true).append(true).open(path)?;
            writeln!(file, "{line}")?;
            Ok(())
        })();
        if persisted.is_err() {
            self.logs.write(LogEntry {
                timestamp: format!("unix:{}", unix_timestamp()),
                level: LogLevel::Error,
                context: "log.persistence".to_string(),
                message: "desktop log persistence failed".to_string(),
                source: "desktop-tauri".to_string(),
                metadata: BTreeMap::new(),
            });
        }
    }

    pub fn clear_persisted_logs(&mut self) -> AdmResult<()> {
        self.logs.clear();
        let path = self.log_file();
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn log_file(&self) -> PathBuf {
        self.runtime_config
            .paths()
            .run_logs_dir
            .join("desktop.jsonl")
    }

    fn load_logs(&mut self) {
        let Ok(text) = fs::read_to_string(self.log_file()) else {
            return;
        };
        for line in text.lines() {
            if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
                self.logs.write(entry);
            }
        }
    }
}

impl CompletionRunner {
    pub fn generate(&self, task_prefix: &str, prompt: String) -> AdmResult<String> {
        let config = self.ai_config.load_or_default()?;
        let target = resolve_active_ai_target(&config, AiConfigCategory::Completion)?;
        if !target.is_available() {
            return Err(AdmError::new(
                "active completion configuration is unavailable",
            ));
        }
        let task = ModelTask {
            task_id: new_stable_id(task_prefix).unwrap_or_else(|_| task_prefix.to_string()),
            prompt,
            input_files: Vec::new(),
            output_files: Vec::new(),
            allowed_write_paths: Vec::new(),
            timeout_seconds: 600,
            sandbox: "read-only".to_string(),
            cwd: self.data_root.display().to_string(),
        };
        let result = match target.descriptor().adapter {
            AiAdapterKind::Codex => CodexCliAdapter {
                cli_path: target
                    .program()
                    .ok_or_else(|| AdmError::new("resolved Codex CLI program is missing"))?
                    .to_string(),
            }
            .generate(&task),
            AiAdapterKind::Claude => ClaudeCliAdapter {
                cli_path: target
                    .program()
                    .ok_or_else(|| AdmError::new("resolved Claude CLI program is missing"))?
                    .to_string(),
            }
            .generate(&task),
            AiAdapterKind::OpenAiCompatible => blocking_openai_adapter_from_resolved(
                &target,
                &self.ai_config.active_completion_entry(&config)?,
            )?
            .generate(&task),
            other => {
                return Err(AdmError::new(format!(
                    "AI completion adapter is unsupported: {}",
                    other.as_str()
                )));
            }
        }
        .map_err(|_| AdmError::new("AI completion provider request failed"))?;
        if result.status == ModelResultStatus::Succeeded {
            Ok(result.text)
        } else {
            Err(AdmError::new(
                "AI completion adapter returned a failed result",
            ))
        }
    }
}

fn safe_log_message(message: &str) -> String {
    const MAX_LOG_CHARS: usize = 1_000;
    let mut redact_next = false;
    let mut output = Vec::new();
    for raw in message.split_whitespace() {
        let token = raw.trim_matches(|character: char| {
            matches!(
                character,
                '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';'
            )
        });
        let lower = token.to_ascii_lowercase();
        let label = matches!(
            lower.as_str(),
            "bearer" | "authorization" | "api_key" | "apikey" | "access_token"
        );
        let assignment = [
            "api_key=",
            "apikey=",
            "access_token=",
            "authorization=",
            "token=",
            "password=",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix));
        let windows_path = token.len() >= 3
            && token.as_bytes()[0].is_ascii_alphabetic()
            && token.as_bytes()[1] == b':'
            && matches!(token.as_bytes()[2], b'\\' | b'/');
        let absolute_path = windows_path
            || token.starts_with("\\\\")
            || (token.starts_with('/') && token.len() > 1);
        let url = lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("file://")
            || lower.starts_with("data:");
        let jwt = token.matches('.').count() == 2
            && token.split('.').all(|part| {
                part.len() >= 8
                    && part.chars().all(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '=')
                    })
            });
        let encoded = token.len() >= 64
            && token.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || matches!(character, '+' | '/' | '_' | '-' | '=')
            });
        let known_secret = lower.starts_with("sk-")
            || lower.starts_with("pk-")
            || lower.starts_with("ghp_")
            || lower.starts_with("xoxb-")
            || lower.starts_with("aiza");
        let replacement = if assignment || absolute_path || url || jwt || encoded || known_secret {
            redact_next = false;
            "[REDACTED]"
        } else if label {
            redact_next = true;
            "[REDACTED]"
        } else if redact_next {
            redact_next = false;
            "[REDACTED]"
        } else {
            raw
        };
        output.push(replacement);
        if output.join(" ").chars().count() >= MAX_LOG_CHARS {
            break;
        }
    }
    let mut safe = output
        .join(" ")
        .chars()
        .take(MAX_LOG_CHARS)
        .collect::<String>();
    if message.chars().count() > MAX_LOG_CHARS {
        safe.push('…');
    }
    safe
}

pub fn with_runtime<T>(
    state: &State<'_, AppRuntime>,
    handler: impl FnOnce(&mut RuntimeState) -> CommandAdapterResult<T>,
) -> CommandAdapterResult<T> {
    match state.lock() {
        Ok(mut runtime) => handler(&mut runtime),
        Err(error) => command_failure(command_error(
            "runtime_state_unavailable",
            error.to_string(),
        )),
    }
}

fn desktop_draft_modified_at(path: &Path) -> u64 {
    [
        path.to_path_buf(),
        path.join("draft_meta.json"),
        path.join("autosave_state.json"),
    ]
    .into_iter()
    .filter_map(|candidate| fs::metadata(candidate).ok()?.modified().ok())
    .filter_map(|modified| modified.duration_since(UNIX_EPOCH).ok())
    .map(|duration| duration.as_secs())
    .max()
    .unwrap_or(0)
}

fn restore_project_state(
    save: &SaveApplicationService,
    design: &DesignWorkbenchService,
    data_root: &Path,
    session_id: &str,
) -> AdmResult<ProjectRestore> {
    let mut warnings = Vec::new();
    match save.autosave_state() {
        Ok(Some(state)) => {
            if let Err(error) = save.acquire_current_lock() {
                let meta_path = data_root
                    .join("drafts")
                    .join(session_id)
                    .join("draft_meta.json");
                if let Some(path) = quarantine_invalid_json(&meta_path)? {
                    warnings.push(format!(
                        "invalid draft metadata was quarantined at {}",
                        path.display()
                    ));
                }
                save.recover_to_unsaved_state(&state)?;
                warnings.push(format!(
                    "the recovered draft was detached because its formal save could not be locked: {error}"
                ));
            }
            return Ok(ProjectRestore {
                state: design.normalize_project_state(state),
                warnings,
            });
        }
        Ok(None) => {}
        Err(error) => {
            let autosave_path = data_root
                .join("drafts")
                .join(session_id)
                .join("autosave_state.json");
            let quarantined = quarantine_corrupt_file(&autosave_path)?;
            warnings.push(match quarantined {
                Some(path) => format!(
                    "invalid draft autosave was quarantined at {}: {error}",
                    path.display()
                ),
                None => format!("draft autosave could not be read: {error}"),
            });
        }
    }

    let index = match save.list_saves() {
        Ok(index) => index,
        Err(error) => {
            let fallback = design.empty_project_state();
            save.recover_to_unsaved_state(&fallback)?;
            warnings.push(format!(
                "save index recovery failed; started with a detached draft: {error}"
            ));
            return Ok(ProjectRestore {
                state: fallback,
                warnings,
            });
        }
    };
    if let Some(save_id) = index.current_save_id {
        match save.load_save(&save_id) {
            Ok(loaded) => {
                return Ok(ProjectRestore {
                    state: design.normalize_project_state(loaded.state),
                    warnings,
                });
            }
            Err(error) => {
                let fallback = design.empty_project_state();
                save.recover_to_unsaved_state(&fallback)?;
                warnings.push(format!(
                    "current save {save_id} could not be restored and was left unchanged; started with a detached draft: {error}"
                ));
                return Ok(ProjectRestore {
                    state: fallback,
                    warnings,
                });
            }
        }
    }
    let fallback = design.empty_project_state();
    if !warnings.is_empty() {
        save.recover_to_unsaved_state(&fallback)?;
    }
    Ok(ProjectRestore {
        state: fallback,
        warnings,
    })
}

fn quarantine_invalid_json(path: &Path) -> AdmResult<Option<PathBuf>> {
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(None);
    };
    if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
        return Ok(None);
    }
    quarantine_corrupt_file(path)
}

fn quarantine_corrupt_file(path: &Path) -> AdmResult<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let parent = path
        .parent()
        .ok_or_else(|| AdmError::new(format!("corrupt file has no parent: {}", path.display())))?;
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("state");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("json");
    for attempt in 0..32_u32 {
        let candidate = parent.join(format!(
            "{stem}.corrupt.{}.{}.{extension}",
            unix_timestamp(),
            attempt
        ));
        if candidate.exists() {
            continue;
        }
        fs::rename(path, &candidate)?;
        return Ok(Some(candidate));
    }
    Err(AdmError::new(format!(
        "failed to allocate quarantine path for {}",
        path.display()
    )))
}

fn classify_interrupted_checkpoint_units(checkpoint: &mut PipelineCheckpoint) -> bool {
    let mut changed = false;
    for unit in &mut checkpoint.units {
        let interrupted = unit.status == PipelineUnitStatus::Running
            || (unit.status == PipelineUnitStatus::Unknown && unit.reconcile_required);
        if !interrupted {
            continue;
        }
        let is_whole_stage = unit.unit_id == format!("{}:stage", unit.stage_id);
        let is_explicitly_safe_local_stage = matches!(
            unit.stage_id.as_str(),
            "00" | "01" | "02" | "03" | "04" | "05" | "06" | "08" | "09" | "10" | "13" | "14"
        );
        if is_whole_stage && is_explicitly_safe_local_stage {
            unit.status = PipelineUnitStatus::Pending;
            unit.started_at.clear();
            unit.completed_at.clear();
            unit.result_fingerprint.clear();
            unit.output_refs.clear();
            unit.reconcile_required = false;
            unit.failure_message.clear();
            changed = true;
        } else if unit.status == PipelineUnitStatus::Running {
            unit.status = PipelineUnitStatus::Unknown;
            unit.reconcile_required = true;
            changed = true;
        }
    }
    if changed {
        checkpoint.next_unit_id = checkpoint
            .units
            .iter()
            .find(|unit| !unit.status.is_committed())
            .map(|unit| unit.unit_id.clone());
    }
    changed
}

fn load_pipeline_state(runtime: &RuntimeApplicationService) -> AdmResult<PipelineRestore> {
    let paths = runtime.paths();
    let candidates = [
        paths.runtime_control_dir.join("pipeline_state.json"),
        paths.outputs_dir.join("pipeline_state.json"),
    ];
    let mut valid = Vec::new();
    let mut warnings = Vec::new();
    let mut needs_rewrite = false;
    for path in candidates {
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<PipelineRunState>(&text) {
                Ok(state) => valid.push((path, state)),
                Err(error) => {
                    let quarantined = quarantine_corrupt_file(&path)?;
                    warnings.push(match quarantined {
                        Some(quarantined) => format!(
                            "invalid pipeline state was quarantined at {}: {error}",
                            quarantined.display()
                        ),
                        None => format!(
                            "pipeline state disappeared while being recovered at {}: {error}",
                            path.display()
                        ),
                    });
                    needs_rewrite = true;
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                needs_rewrite = true;
            }
            Err(error) => {
                warnings.push(format!(
                    "pipeline state could not be read at {}: {error}",
                    path.display()
                ));
                needs_rewrite = true;
            }
        }
    }
    let mut state = valid
        .first()
        .map(|(_, state)| state.clone())
        .unwrap_or_else(empty_pipeline_state);
    if state.schema_version < 2 {
        state.schema_version = 2;
        state.state_version = state.state_version.saturating_add(1);
        warnings.push("legacy pipeline state was upgraded to schema version 2".to_string());
        needs_rewrite = true;
    }
    if valid.len() == 2 && valid[0].1 != valid[1].1 {
        warnings.push(format!(
            "pipeline state copies diverged; recovered the runtime-control copy from {}",
            valid[0].0.display()
        ));
        needs_rewrite = true;
    }
    if matches!(
        state.status.as_str(),
        "running" | "stop_requested" | "stopping" | "stopped" | "recoverable"
    ) && !state.run_id.is_empty()
    {
        let repository = PipelineCheckpointRepository::new(&paths.checkpoints_dir);
        match repository.load_current(&state.run_id) {
            Ok(Some(mut checkpoint)) => {
                state.attempt_id = checkpoint.identity.attempt_id.clone();
                state.parent_attempt_id = checkpoint.identity.parent_attempt_id.clone();
                state.from_stage_id = checkpoint.range.from_stage_id.clone();
                state.to_stage_id = checkpoint.range.to_stage_id.clone();
                state.stage_ids = checkpoint.range.stage_ids.clone();
                state.current_stage_id = checkpoint.current_stage_id.clone();
                state.current_unit_id = checkpoint.current_unit_id.clone();
                match checkpoint.status {
                    PipelineCheckpointStatus::Running
                    | PipelineCheckpointStatus::StopRequested
                    | PipelineCheckpointStatus::Stopping
                    | PipelineCheckpointStatus::Stopped
                    | PipelineCheckpointStatus::Resuming => {
                        checkpoint.revision = checkpoint.revision.saturating_add(1);
                        checkpoint.status = PipelineCheckpointStatus::Recoverable;
                        checkpoint.resume_policy = PipelineResumePolicy::ExplicitOnly;
                        checkpoint.updated_at = format!("unix:{}", unix_timestamp());
                        classify_interrupted_checkpoint_units(&mut checkpoint);
                        if let Err(error) = repository.save_attempt_and_current(&checkpoint) {
                            state.status = "recovery_blocked".to_string();
                            warnings.push(format!(
                                "interrupted pipeline checkpoint could not be made recoverable: {error}"
                            ));
                        } else {
                            state.status = "recoverable".to_string();
                            state.recovery = Some(PipelineRecoverySummary::from(&checkpoint));
                            warnings.push(
                                "an interrupted pipeline has a validated checkpoint and requires explicit resume"
                                    .to_string(),
                            );
                        }
                    }
                    PipelineCheckpointStatus::Recoverable => {
                        let normalized = classify_interrupted_checkpoint_units(&mut checkpoint);
                        if normalized {
                            checkpoint.revision = checkpoint.revision.saturating_add(1);
                            checkpoint.updated_at = format!("unix:{}", unix_timestamp());
                        }
                        if normalized
                            && let Err(error) = repository.save_attempt_and_current(&checkpoint)
                        {
                            state.status = "recovery_blocked".to_string();
                            warnings.push(format!(
                                "recoverable pipeline checkpoint could not be normalized: {error}"
                            ));
                        } else {
                            state.status = "recoverable".to_string();
                            state.recovery = Some(PipelineRecoverySummary::from(&checkpoint));
                        }
                    }
                    PipelineCheckpointStatus::WaitingConfirmation => {
                        state.status = "waiting_confirmation".to_string();
                        state.recovery = None;
                    }
                    PipelineCheckpointStatus::Completed => {
                        state.status = "success".to_string();
                        state.recovery = None;
                    }
                    PipelineCheckpointStatus::Failed => {
                        state.status = "failed".to_string();
                        state.recovery = None;
                    }
                    PipelineCheckpointStatus::RecoveryBlocked => {
                        state.status = "recovery_blocked".to_string();
                        state.recovery = Some(PipelineRecoverySummary::from(&checkpoint));
                    }
                }
                state.state_version = state.state_version.saturating_add(1);
                needs_rewrite = true;
            }
            Ok(None) => {
                state.status = "recovery_blocked".to_string();
                warnings.push(
                    "interrupted pipeline has no validated checkpoint; automatic continuation is disabled"
                        .to_string(),
                );
                needs_rewrite = true;
            }
            Err(_) => {
                state.status = "recovery_blocked".to_string();
                warnings.push(
                    "interrupted pipeline checkpoint is invalid; automatic continuation is disabled"
                        .to_string(),
                );
                needs_rewrite = true;
            }
        }
    }
    if matches!(state.status.as_str(), "failed" | "blocked") && !state.run_id.is_empty() {
        let repository = PipelineCheckpointRepository::new(&paths.checkpoints_dir);
        if let Ok(Some(checkpoint)) = repository.load_current(&state.run_id)
            && checkpoint.status == PipelineCheckpointStatus::Recoverable
            && checkpoint.resume_policy == PipelineResumePolicy::ExplicitOnly
        {
            state.attempt_id = checkpoint.identity.attempt_id.clone();
            state.parent_attempt_id = checkpoint.identity.parent_attempt_id.clone();
            state.from_stage_id = checkpoint.range.from_stage_id.clone();
            state.to_stage_id = checkpoint.range.to_stage_id.clone();
            state.stage_ids = checkpoint.range.stage_ids.clone();
            state.current_stage_id = checkpoint.current_stage_id.clone();
            state.current_unit_id = checkpoint.current_unit_id.clone();
            state.status = "recoverable".to_string();
            state.recovery = Some(PipelineRecoverySummary::from(&checkpoint));
            state.state_version = state.state_version.saturating_add(1);
            warnings.push(
                "pipeline runtime state was reconciled with its recoverable checkpoint".to_string(),
            );
            needs_rewrite = true;
        }
    }
    if state.stop_requested {
        state.stop_requested = false;
        needs_rewrite = true;
    }
    Ok(PipelineRestore {
        state,
        warnings,
        needs_rewrite,
    })
}

pub(crate) fn empty_pipeline_state() -> PipelineRunState {
    PipelineRunState {
        run_id: String::new(),
        status: "idle".to_string(),
        stop_requested: false,
        current_stage_id: None,
        stages: BTreeMap::new(),
        ..PipelineRunState::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::patch::PatchTask;
    use adm_new_contracts::project::DecisionState;
    use adm_new_contracts::sdk::SdkReviewStatus;
    use adm_new_tauri_commands::pipeline::{self, RunPipelineRangeRequest};
    use serde_json::json;

    #[test]
    fn persisted_log_messages_hide_machine_paths_urls_and_credentials() {
        let safe = safe_log_message(
            "failed at C:\\Users\\private\\project https://user:pass@example.test/v1 Authorization Bearer secret-value api_key=another-secret",
        );
        assert!(!safe.contains("C:\\Users"));
        assert!(!safe.contains("example.test"));
        assert!(!safe.contains("secret-value"));
        assert!(!safe.contains("another-secret"));
        assert!(safe.contains("[REDACTED]"));
    }

    #[test]
    fn runtime_autosave_survives_reconstruction() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        {
            let mut state = runtime.lock().unwrap();
            state.project_state.project_name = "Restarted Project".to_string();
            state.persist_project_state("test").unwrap();
        }
        drop(runtime);
        let restored = AppRuntime::new(&root).unwrap();
        assert_eq!(
            restored.lock().unwrap().project_state.project_name,
            "Restarted Project"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn custom_templates_follow_the_session_draft_through_archive_restore_and_restart() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-template-archive-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        let template_id;
        {
            let mut state = runtime.lock().unwrap();
            let expected_template_dir = state
                .save
                .draft_root()
                .join("workspace")
                .join("projects")
                .join("templates");
            assert_eq!(
                state.design.custom_project_templates_dir().unwrap(),
                expected_template_dir
            );
            state.project_state.project_name = "Archived Template Project".to_string();
            let saved = state
                .design
                .save_project_template(&state.project_state, "Archived Template", "indie", false)
                .unwrap();
            template_id = saved.template_id.clone();
            assert!(expected_template_dir.join(saved.target_file_name).is_file());

            let formal = state
                .save
                .create_save("Template Archive", &state.project_state)
                .unwrap();
            state.design.delete_project_template(&template_id).unwrap();
            assert!(
                state
                    .design
                    .list_project_templates(true)
                    .unwrap()
                    .templates
                    .iter()
                    .all(|template| template.template_id != template_id)
            );

            let loaded = state.save.load_save(&formal.manifest.save_id).unwrap();
            state.project_state = state.design.normalize_project_state(loaded.state);
            assert!(
                state
                    .design
                    .list_project_templates(true)
                    .unwrap()
                    .templates
                    .iter()
                    .any(|template| template.template_id == template_id)
            );
            let design = state.design.clone();
            design
                .apply_project_template(&mut state.project_state, &template_id, "范本：")
                .unwrap();
            assert_eq!(state.project_state.project_name, "范本：Archived Template");
            state
                .persist_project_state("test.template_restore")
                .unwrap();
            state.shutdown().unwrap();
        }
        drop(runtime);

        let restored = AppRuntime::new(&root).unwrap();
        {
            let mut state = restored.lock().unwrap();
            let listed = state.design.list_project_templates(true).unwrap();
            assert!(
                listed
                    .templates
                    .iter()
                    .any(|template| template.template_id == template_id)
            );
            let design = state.design.clone();
            design
                .apply_project_template(&mut state.project_state, &template_id, "Template: ")
                .unwrap();
            assert_eq!(
                state.project_state.project_name,
                "Template: Archived Template"
            );
            state.shutdown().unwrap();
        }
        drop(restored);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn canonical_session_restores_formal_save_when_drafts_are_missing() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-formal-only-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        {
            let mut state = runtime.lock().unwrap();
            state.project_state.project_name = "Formal Only Project".to_string();
            state.persist_project_state("test").unwrap();
            state
                .save
                .create_save("Formal Only Project", &state.project_state)
                .unwrap();
        }
        runtime.shutdown_once().unwrap();
        drop(runtime);
        fs::remove_dir_all(root.join("drafts")).unwrap();

        let restored = AppRuntime::new(&root).unwrap();
        assert_eq!(
            restored.lock().unwrap().project_state.project_name,
            "Formal Only Project"
        );
        restored.shutdown_once().unwrap();
        drop(restored);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_draft_autosave_is_quarantined_and_formal_save_is_restored() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-corrupt-autosave-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        let session_id = runtime._session_lease.session_id.clone();
        {
            let mut state = runtime.lock().unwrap();
            state.project_state.project_name = "Formal Project".to_string();
            state.persist_project_state("test").unwrap();
            state
                .save
                .create_save("Formal Project", &state.project_state)
                .unwrap();
        }
        let autosave_path = root
            .join("drafts")
            .join(&session_id)
            .join("autosave_state.json");
        fs::write(&autosave_path, "{invalid").unwrap();
        drop(runtime);

        let restored = AppRuntime::new(&root).unwrap();
        let state = restored.lock().unwrap();
        assert_eq!(state.project_state.project_name, "Formal Project");
        assert!(state.logs.latest(100).iter().any(|entry| {
            entry.context == "save.recovery" && entry.message.contains("quarantined")
        }));
        drop(state);
        assert!(
            fs::read_dir(autosave_path.parent().unwrap())
                .unwrap()
                .flatten()
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("autosave_state.corrupt."))
        );

        restored.shutdown_once().unwrap();
        drop(restored);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_draft_and_formal_save_start_as_detached_without_overwriting_archive() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-corrupt-formal-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        let session_id = runtime._session_lease.session_id.clone();
        let save_id = {
            let mut state = runtime.lock().unwrap();
            state.project_state.project_name = "Corrupt Formal".to_string();
            state.persist_project_state("test").unwrap();
            state
                .save
                .create_save("Corrupt Formal", &state.project_state)
                .unwrap()
                .manifest
                .save_id
        };
        fs::write(
            root.join("drafts")
                .join(session_id)
                .join("autosave_state.json"),
            "{invalid-draft",
        )
        .unwrap();
        let manifest_path = root.join("saves").join(&save_id).join("manifest.json");
        fs::write(&manifest_path, "{invalid-formal").unwrap();
        drop(runtime);

        let restored = AppRuntime::new(&root).unwrap();
        let state = restored.lock().unwrap();
        assert_ne!(state.project_state.project_name, "Corrupt Formal");
        let index = state.save.list_saves().unwrap();
        assert!(index.current_save_id.is_none());
        assert!(
            index
                .saves
                .iter()
                .any(|entry| { entry.save_id == save_id && entry.integrity_status == "corrupt" })
        );
        assert!(state.logs.latest(100).iter().any(|entry| {
            entry.context == "save.recovery" && entry.message.contains("left unchanged")
        }));
        drop(state);
        assert_eq!(
            fs::read_to_string(&manifest_path).unwrap(),
            "{invalid-formal"
        );

        restored.shutdown_once().unwrap();
        drop(restored);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn locked_formal_save_detaches_but_preserves_valid_draft() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-locked-recovery-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        let save_id;
        {
            let mut state = runtime.lock().unwrap();
            state.project_state.project_name = "Unsaved Draft Content".to_string();
            state.persist_project_state("test").unwrap();
            save_id = state
                .save
                .create_save("Locked Project", &state.project_state)
                .unwrap()
                .manifest
                .save_id;
            state.project_state.project_name = "Unsaved Draft Content".to_string();
            state.persist_project_state("test.unsaved").unwrap();
        }
        drop(runtime);
        let blocker = SaveApplicationService::new(&root, "another_live_window").unwrap();
        blocker.load_save(&save_id).unwrap();
        let lock_path = root.join("saves").join(&save_id).join(".archive_lock");

        let restored = AppRuntime::new(&root).unwrap();
        let state = restored.lock().unwrap();
        assert_eq!(state.project_state.project_name, "Unsaved Draft Content");
        let index = state.save.list_saves().unwrap();
        assert!(index.current_save_id.is_none());
        assert!(state.logs.latest(100).iter().any(|entry| {
            entry.context == "save.recovery" && entry.message.contains("detached")
        }));
        drop(state);
        assert!(
            lock_path.is_file(),
            "another session's lock must be retained"
        );

        restored.shutdown_once().unwrap();
        drop(restored);
        blocker.release_current_lock().unwrap();
        drop(blocker);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_desktop_runtimes_use_isolated_drafts() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-session-isolation-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let first = AppRuntime::new(&root).unwrap();
        {
            let mut state = first.lock().unwrap();
            state.project_state.project_name = "First Window".to_string();
            state.persist_project_state("test.first").unwrap();
            state
                .save
                .create_save("First Window", &state.project_state)
                .unwrap();
        }

        let second = AppRuntime::new(&root).unwrap();
        assert_ne!(
            first._session_lease.session_id,
            second._session_lease.session_id
        );
        assert_ne!(
            second.lock().unwrap().project_state.project_name,
            "First Window",
            "a new concurrent window must not inherit another window's draft or current save"
        );
        assert!(first._session_lease.lock_path.is_file());
        assert!(second._session_lease.lock_path.is_file());

        drop(second);
        drop(first);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unheld_desktop_session_lock_file_can_be_reacquired() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-stale-session-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let draft = root.join("drafts").join(DESKTOP_SESSION_ID);
        let lock_root = root.join("drafts").join(DESKTOP_SESSION_LOCK_DIR);
        let lock_path = lock_root.join(format!("{DESKTOP_SESSION_ID}.lock"));
        fs::create_dir_all(&draft).unwrap();
        fs::create_dir_all(&lock_root).unwrap();
        fs::write(
            &lock_path,
            serde_json::to_string(&DesktopSessionLock {
                session_id: DESKTOP_SESSION_ID.to_string(),
                pid: 0,
                acquired_at: "unix:1".to_string(),
            })
            .unwrap(),
        )
        .unwrap();

        let runtime = AppRuntime::new(&root).unwrap();
        assert_eq!(runtime._session_lease.session_id, DESKTOP_SESSION_ID);
        drop(runtime);
        assert!(lock_path.is_file());
        let reacquired = AppRuntime::new(&root).unwrap();
        assert_eq!(reacquired._session_lease.session_id, DESKTOP_SESSION_ID);
        drop(reacquired);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn malformed_unheld_lock_metadata_is_overwritten_without_hiding_autosave() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-malformed-session-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        {
            let mut state = runtime.lock().unwrap();
            state.project_state.project_name = "Recover Hidden Draft".to_string();
            state.persist_project_state("test").unwrap();
        }
        drop(runtime);
        let lock_root = root.join("drafts").join(DESKTOP_SESSION_LOCK_DIR);
        let lock_path = lock_root.join(format!("{DESKTOP_SESSION_ID}.lock"));
        fs::write(&lock_path, "{partial").unwrap();

        let restored = AppRuntime::new(&root).unwrap();
        assert_eq!(restored._session_lease.session_id, DESKTOP_SESSION_ID);
        assert_eq!(
            restored.lock().unwrap().project_state.project_name,
            "Recover Hidden Draft"
        );
        restored.shutdown_once().unwrap();
        drop(restored);
        let lock: DesktopSessionLock =
            serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
        assert_eq!(lock.session_id, DESKTOP_SESSION_ID);
        assert_eq!(lock.pid, std::process::id());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_pipeline_state_is_reconciled_and_rewritten() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-pipeline-recovery-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime_config = RuntimeApplicationService::new(&root, DESKTOP_SESSION_ID).unwrap();
        let paths = runtime_config.paths();
        let mut volatile = empty_pipeline_state();
        volatile.run_id = "interrupted-run".to_string();
        volatile.status = "running".to_string();
        volatile.stop_requested = true;
        let durable = empty_pipeline_state();
        let volatile_path = paths.runtime_control_dir.join("pipeline_state.json");
        let durable_path = paths.outputs_dir.join("pipeline_state.json");
        adm_new_foundation::write_text_atomic(
            &volatile_path,
            &serde_json::to_string_pretty(&volatile).unwrap(),
        )
        .unwrap();
        adm_new_foundation::write_text_atomic(
            &durable_path,
            &serde_json::to_string_pretty(&durable).unwrap(),
        )
        .unwrap();

        let runtime = AppRuntime::new(&root).unwrap();
        let state = runtime.lock().unwrap();
        assert_eq!(state.pipeline_state.run_id, "interrupted-run");
        assert_eq!(state.pipeline_state.status, "recovery_blocked");
        assert!(!state.pipeline_state.stop_requested);
        assert!(state.logs.latest(100).iter().any(|entry| {
            entry.context == "pipeline.recovery" && entry.message.contains("diverged")
        }));
        assert!(state.logs.latest(100).iter().any(|entry| {
            entry.context == "pipeline.recovery"
                && entry.message.contains("no validated checkpoint")
        }));
        let expected = state.pipeline_state.clone();
        drop(state);
        for path in [&volatile_path, &durable_path] {
            let restored: PipelineRunState =
                serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
            assert_eq!(restored, expected);
        }

        runtime.shutdown_once().unwrap();
        drop(runtime);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_pure_stage_becomes_an_explicitly_recoverable_safe_retry() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-pipeline-checkpoint-recovery-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime_config = RuntimeApplicationService::new(&root, DESKTOP_SESSION_ID).unwrap();
        let paths = runtime_config.paths();
        let mut state = empty_pipeline_state();
        state.run_id = "recoverable_run".to_string();
        state.status = "running".to_string();
        let text = serde_json::to_string_pretty(&state).unwrap();
        adm_new_foundation::write_text_atomic(
            &paths.runtime_control_dir.join("pipeline_state.json"),
            &text,
        )
        .unwrap();
        adm_new_foundation::write_text_atomic(
            &paths.outputs_dir.join("pipeline_state.json"),
            &text,
        )
        .unwrap();
        let repository = PipelineCheckpointRepository::new(&paths.checkpoints_dir);
        let mut checkpoint = adm_new_pipeline::initial_whole_stage_checkpoint(
            adm_new_contracts::pipeline::PipelineRunIdentity {
                run_id: "recoverable_run".to_string(),
                attempt_id: "attempt_1".to_string(),
                project_id: "project_1".to_string(),
                draft_id: DESKTOP_SESSION_ID.to_string(),
                ..Default::default()
            },
            adm_new_contracts::pipeline::CanonicalPipelineRange {
                from_stage_id: "00".to_string(),
                to_stage_id: "01".to_string(),
                stage_ids: vec!["00".to_string(), "01".to_string()],
            },
            Default::default(),
        );
        checkpoint.units[0].status = PipelineUnitStatus::Running;
        checkpoint.units[0].reconcile_required = true;
        repository.save_attempt_and_current(&checkpoint).unwrap();

        let runtime = AppRuntime::new(&root).unwrap();
        assert_eq!(runtime.lock().unwrap().pipeline_state.status, "recoverable");
        let recovered = repository.load_current("recoverable_run").unwrap().unwrap();
        assert_eq!(recovered.status, PipelineCheckpointStatus::Recoverable);
        assert_eq!(recovered.units[0].status, PipelineUnitStatus::Pending);
        assert!(!recovered.units[0].reconcile_required);
        assert_eq!(recovered.next_unit_id.as_deref(), Some("00:stage"));

        runtime.shutdown_once().unwrap();
        drop(runtime);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_external_unit_stage_still_requires_reconciliation() {
        let mut checkpoint = adm_new_pipeline::initial_whole_stage_checkpoint(
            adm_new_contracts::pipeline::PipelineRunIdentity {
                run_id: "classify_interrupted".to_string(),
                ..Default::default()
            },
            adm_new_contracts::pipeline::CanonicalPipelineRange {
                from_stage_id: "00".to_string(),
                to_stage_id: "07".to_string(),
                stage_ids: vec!["00".to_string(), "07".to_string()],
            },
            Default::default(),
        );
        checkpoint.units[0].status = PipelineUnitStatus::Running;
        checkpoint.units[0].reconcile_required = true;
        checkpoint.units[1].status = PipelineUnitStatus::Running;
        checkpoint.units[1].reconcile_required = true;

        assert!(classify_interrupted_checkpoint_units(&mut checkpoint));
        assert_eq!(checkpoint.units[0].status, PipelineUnitStatus::Pending);
        assert!(!checkpoint.units[0].reconcile_required);
        assert_eq!(checkpoint.units[1].status, PipelineUnitStatus::Unknown);
        assert!(checkpoint.units[1].reconcile_required);
        assert_eq!(checkpoint.next_unit_id.as_deref(), Some("00:stage"));
    }

    #[test]
    fn corrupt_pipeline_copy_is_quarantined_and_rebuilt_from_durable_state() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-pipeline-corrupt-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime_config = RuntimeApplicationService::new(&root, DESKTOP_SESSION_ID).unwrap();
        let paths = runtime_config.paths();
        let volatile_path = paths.runtime_control_dir.join("pipeline_state.json");
        let durable_path = paths.outputs_dir.join("pipeline_state.json");
        let mut durable = empty_pipeline_state();
        durable.run_id = "durable-run".to_string();
        durable.status = "failed".to_string();
        adm_new_foundation::write_text_atomic(&volatile_path, "{invalid").unwrap();
        adm_new_foundation::write_text_atomic(
            &durable_path,
            &serde_json::to_string_pretty(&durable).unwrap(),
        )
        .unwrap();

        let runtime = AppRuntime::new(&root).unwrap();
        let state = runtime.lock().unwrap();
        assert_eq!(state.pipeline_state.run_id, "durable-run");
        assert_eq!(state.pipeline_state.status, "failed");
        assert!(state.logs.latest(100).iter().any(|entry| {
            entry.context == "pipeline.recovery" && entry.message.contains("quarantined")
        }));
        drop(state);
        assert!(
            fs::read_dir(volatile_path.parent().unwrap())
                .unwrap()
                .flatten()
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("pipeline_state.corrupt."))
        );
        let rebuilt: PipelineRunState =
            serde_json::from_str(&fs::read_to_string(&volatile_path).unwrap()).unwrap();
        assert_eq!(rebuilt.run_id, "durable-run");

        runtime.shutdown_once().unwrap();
        drop(runtime);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_captures_injected_ui_language_without_process_environment_changes() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-language-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new_with_ui_language(&root, UiLanguage::EnUs).unwrap();
        assert_eq!(runtime.lock().unwrap().ui_language, UiLanguage::EnUs);
        drop(runtime);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_exit_coordination_waits_for_pipeline_and_shutdown_is_idempotent() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-exit-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        let run_guard = runtime.try_begin_pipeline_run().unwrap();

        assert!(runtime.pipeline_is_running());
        assert_eq!(runtime.begin_exit(), ExitDisposition::WaitForPipeline);
        assert_eq!(runtime.begin_exit(), ExitDisposition::AlreadyExiting);
        runtime.request_pipeline_stop();
        assert!(runtime.pipeline_stop_flag().load(Ordering::SeqCst));

        drop(run_guard);
        assert!(!runtime.pipeline_is_running());
        runtime.shutdown_once().unwrap();
        runtime.shutdown_once().unwrap();
        drop(runtime);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_exit_claim_prevents_a_new_pipeline_from_starting() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-exit-race-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();

        assert_eq!(runtime.begin_exit(), ExitDisposition::ShutdownNow);
        assert!(runtime.try_begin_pipeline_run().is_none());
        runtime.shutdown_once().unwrap();
        drop(runtime);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn losing_pipeline_start_does_not_clear_the_active_run_stop_request() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-pipeline-start-race-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        let run_guard = runtime.try_begin_pipeline_run().unwrap();
        runtime.request_pipeline_stop();

        assert!(runtime.try_begin_pipeline_run().is_none());
        assert!(runtime.pipeline_stop_flag().load(Ordering::SeqCst));

        drop(run_guard);
        drop(runtime);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_shutdown_keeps_archive_lock_for_retry() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-shutdown-failure-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        let save_id = {
            let mut state = runtime.lock().unwrap();
            state.project_state.project_name = "Protected Save".to_string();
            state.persist_project_state("test").unwrap();
            state
                .save
                .create_save("Protected Save", &state.project_state)
                .unwrap()
                .manifest
                .save_id
        };
        let lock_path = root.join("saves").join(&save_id).join(".archive_lock");
        let manifest_path = root.join("saves").join(&save_id).join("manifest.json");
        assert!(lock_path.is_file());
        let original_manifest = fs::read_to_string(&manifest_path).unwrap();
        fs::write(&manifest_path, "{invalid").unwrap();

        let error = runtime.shutdown_once().unwrap_err();
        assert!(error.to_string().contains("shutdown persistence failed"));
        assert!(
            lock_path.is_file(),
            "failed persistence must retain the lock"
        );

        fs::write(&manifest_path, original_manifest).unwrap();
        runtime.shutdown_once().unwrap();
        let lock: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
        assert_eq!(
            lock.get("live").and_then(serde_json::Value::as_bool),
            Some(false)
        );
        let os_lock_path = root
            .join("saves")
            .join(".locks")
            .join(format!("archive_{save_id}.lock"));
        let probe = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(os_lock_path)
            .unwrap();
        FileExt::try_lock_exclusive(&probe).unwrap();
        FileExt::unlock(&probe).unwrap();

        drop(runtime);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn shutdown_does_not_downgrade_to_autosave_when_index_is_temporarily_locked() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-shutdown-index-lock-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        let save_id = {
            let mut state = runtime.lock().unwrap();
            state.project_state.project_name = "Index Locked Save".to_string();
            state.persist_project_state("test").unwrap();
            let save_id = state
                .save
                .create_save("Index Locked Save", &state.project_state)
                .unwrap()
                .manifest
                .save_id;
            state.project_state.project_name = "Must Reach Formal Save".to_string();
            state.persist_project_state("test.unsaved").unwrap();
            save_id
        };
        let archive_lock = root.join("saves").join(&save_id).join(".archive_lock");
        let index_lock = root.join("saves").join(".locks").join("index.lock");
        fs::create_dir_all(index_lock.parent().unwrap()).unwrap();
        let index_guard = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&index_lock)
            .unwrap();
        FileExt::try_lock_exclusive(&index_guard).unwrap();

        let error = runtime.shutdown_once().unwrap_err();
        assert!(error.to_string().contains("save index"));
        assert!(archive_lock.is_file());

        FileExt::unlock(&index_guard).unwrap();
        drop(index_guard);
        runtime.shutdown_once().unwrap();
        let lock: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&archive_lock).unwrap()).unwrap();
        assert_eq!(
            lock.get("live").and_then(serde_json::Value::as_bool),
            Some(false)
        );
        drop(runtime);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_patch_and_sdk_records_survive_reconstruction() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-utility-state-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        {
            let mut state = runtime.lock().unwrap();
            state
                .patch
                .analyze_request_shell(
                    "Add an integration hook",
                    vec![PatchTask {
                        task_id: "PATCH-001".to_string(),
                        title: "Integration hook".to_string(),
                        description: "Add an integration hook".to_string(),
                        affected_systems: vec!["integration".to_string()],
                        expected_files: vec!["Assets/Scripts/Integration.cs".to_string()],
                        validation_route: vec!["step14_integration_validation".to_string()],
                        requires_iteration: false,
                    }],
                )
                .unwrap();
            state.persist_patch_records().unwrap();
            let sdk = state
                .sdk
                .add_placeholder_with_source_url(
                    "steamworks",
                    "Steamworks",
                    "https://partner.steamgames.com/doc/sdk",
                )
                .unwrap();
            state
                .sdk
                .set_review_status(&sdk.sdk_id, SdkReviewStatus::Approved)
                .unwrap();
            state.persist_sdk_specs().unwrap();
        }
        drop(runtime);

        let restored = AppRuntime::new(&root).unwrap();
        {
            let state = restored.lock().unwrap();
            assert_eq!(state.patch.list().len(), 1);
            assert_eq!(state.sdk.list_specs().len(), 1);
            assert!(state.sdk.approved_context().contains("Steamworks"));
        }
        drop(restored);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_pipeline_and_artifacts_survive_formal_save_restart() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-runtime-pipeline-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let runtime = AppRuntime::new(&root).unwrap();
        {
            let mut state = runtime.lock().unwrap();
            state.project_state.project_name = "Restartable Pipeline Project".to_string();
            state.project_state.profile.extend([
                ("targetScale".to_string(), json!("indie")),
                ("businessModel".to_string(), json!("premium")),
                ("platformScope".to_string(), json!("pc")),
                ("genre".to_string(), json!("action_rpg")),
            ]);
            state.project_state.gameplay_systems.selected = vec![
                "combat".to_string(),
                "progression".to_string(),
                "exploration".to_string(),
                "inventory".to_string(),
                "quests".to_string(),
            ];
            for (index, node) in state.project_state.nodes.values_mut().enumerate() {
                node.decision_state = DecisionState::Completed;
                node.design_note = format!("Project-specific decision {index}");
                node.checklist
                    .values_mut()
                    .for_each(|checked| *checked = true);
                node.design_entities.push(json!({
                    "id": format!("entity_{index}"),
                    "kind": "system",
                    "name": format!("System {index}"),
                    "description": "Executable project-specific design entity"
                }));
            }
            let selected_option = load_design_specs().unwrap().specs.iter().find_map(|node| {
                node.checklist.iter().find_map(|item| {
                    item.option_groups.iter().find_map(|group| {
                        group.options.first().map(|option| {
                            (
                                node.node_id.clone(),
                                item.item_id.clone(),
                                group.group_id.clone(),
                                option.clone(),
                            )
                        })
                    })
                })
            });
            let (node_id, item_id, group_id, option_id) =
                selected_option.expect("design fixtures expose at least one selectable option");
            let design = state.design.clone();
            design
                .set_option_group_option(
                    &mut state.project_state,
                    &node_id,
                    &item_id,
                    &group_id,
                    &option_id,
                    true,
                )
                .unwrap();
            state.persist_project_state("test.seed").unwrap();
            state
                .save
                .create_save("Restartable Pipeline Project", &state.project_state)
                .unwrap();
            state
                .pipeline_executor
                .prepare_project_source(&state.project_state)
                .unwrap();
            let service = state.pipeline.clone();
            // This test verifies save/restart persistence, not availability of an
            // external development provider or a real Unity materialization run.
            // Keep those production boundaries fail-closed and stop before the
            // Step11 execution-object/Unity gate.
            let executor = state
                .pipeline_executor
                .clone()
                .with_offline_work_unit_executor();
            let mut pipeline_state = state.pipeline_state.clone();
            let response = pipeline::run_pipeline_range(
                &service,
                &mut pipeline_state,
                RunPipelineRangeRequest {
                    from_stage_id: "00".to_string(),
                    to_stage_id: "10".to_string(),
                    skip_manual_gates: true,
                    artifact_locale: UiLanguage::default(),
                },
                &executor,
            );
            assert!(response.ok, "pipeline adapter failed: {response:?}");
            let view = &response.data.as_ref().unwrap().view;
            assert_eq!(
                view.stages
                    .iter()
                    .filter(|stage| stage.status != "pending")
                    .count(),
                11,
                "pipeline stopped early: {:?}",
                view.stages
                    .iter()
                    .map(|stage| (&stage.stage_id, &stage.status, &stage.errors))
                    .collect::<Vec<_>>()
            );
            state.pipeline_state = pipeline_state;
            state.persist_pipeline_state().unwrap();
            state
                .save
                .sync_current_save(&state.project_state, "pipeline_test")
                .unwrap();
            assert!(
                state
                    .pipeline_executor
                    .artifact_root()
                    .join("stage_10")
                    .is_dir()
            );
            state.shutdown().unwrap();
        }
        drop(runtime);

        let restored = AppRuntime::new(&root).unwrap();
        {
            let state = restored.lock().unwrap();
            assert_eq!(
                state.project_state.project_name,
                "Restartable Pipeline Project"
            );
            assert_eq!(state.pipeline_state.stages.len(), 11);
            assert!(
                state
                    .pipeline_executor
                    .artifact_root()
                    .join("stage_00/concept_profile.json")
                    .is_file()
            );
            assert!(
                state
                    .pipeline_executor
                    .artifact_root()
                    .join("stage_10")
                    .is_dir()
            );
        }
        let _ = restored.lock().unwrap().shutdown();
        drop(restored);
        let _ = fs::remove_dir_all(root);
    }
}
