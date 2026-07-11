#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use adm_new_contracts::project::ProjectState;
use adm_new_contracts::save::{
    ArchiveLock, AutosaveState, DraftMeta, FileMap, FileMapChange, FileMapDelta, FileMapEntry,
    SAVE_SCHEMA_VERSION, SaveIndex, SaveIndexEntry, SaveManifest, SaveProgress, SnapshotManifest,
    TimelineEntry, WorkspaceState,
};
use adm_new_foundation::{
    AdmError, AdmResult, ensure_relative_path, new_stable_id, sanitize_identifier, sha256_hex,
    unix_timestamp, unix_timestamp_millis,
};
use adm_new_storage::{
    DraftWorkspaceRepository, ProjectRoot, SaveArchiveRepository, SaveIndexRepository,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use fs2::FileExt;

pub const CRATE_NAME: &str = "adm-new-save";
const MANIFEST_NAME: &str = "manifest.json";
const LEGACY_MANIFEST_NAME: &str = "save_manifest.json";
const INDEX_LOCK_NAME: &str = ".save_index_lock";
const TRANSACTION_DIR: &str = "drafts/.transactions";
const TRANSACTION_SCHEMA_VERSION: u32 = 1;
const CLEANUP_RETRY_LIMIT: usize = 3;
const TRANSACTION_LOCK_RETRY_LIMIT: usize = 20;
const TRANSACTION_LOCK_RETRY_DELAY_MS: u64 = 25;
const SNAPSHOT_KEEP_PER_SAVE: usize = 5;
const ARCHIVED_DRAFT_RELATIVE_PATHS: &[&str] = &[
    "autosave_state.json",
    "source_artifacts",
    "outputs",
    "workspace",
    "iteration_specs",
    "patches",
    "project_config.json",
    "gate_log.yaml",
];
const ARCHIVE_EXCLUDED_RELATIVE_PATHS: &[&str] = &["runtime", "outputs/runtime_control"];
const BLANK_CLEAN_RELATIVE_PATHS: &[&str] = &[
    "outputs/artifacts",
    "outputs/checkpoints",
    "outputs/run_logs",
    "outputs/runtime_control",
    "outputs/artifact_layer",
    "outputs/execution_objects/execution_objects.json",
    "workspace",
];
const BLANK_DRAFT_REPLACE_PATHS: &[&str] = &[
    "autosave_state.json",
    "draft_file_map.json",
    "runtime",
    "outputs",
    "workspace",
    "iteration_specs",
    "patches",
    "project_config.json",
    "gate_log.yaml",
    "source_artifacts",
];

const EMPTY_WORKSPACE_DIRS: &[&str] = &[
    "source_artifacts",
    "source_artifacts/operator_drafts",
    "outputs",
    "outputs/artifacts",
    "outputs/run_logs",
    "outputs/checkpoints",
    "outputs/artifact_layer",
    "outputs/runtime_control",
    "outputs/execution_objects",
    "workspace",
    "workspace/projects",
    "workspace/exports",
    "iteration_specs",
    "patches",
];

pub fn crate_ready() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlankSaveRepairReport {
    pub save_id: String,
    pub apply: bool,
    pub old_progress: SaveProgress,
    pub new_progress: SaveProgress,
    pub cleanup_paths: Vec<String>,
    pub removed_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftIsolationSummary {
    pub draft: String,
    pub linked_save_id: String,
    pub run_context_save_id: String,
    pub run_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveIsolationSummary {
    pub save_id: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParallelIsolationIssue {
    pub code: String,
    pub severity: String,
    pub path: String,
    pub message: String,
    #[serde(default)]
    pub expected: String,
    #[serde(default)]
    pub actual: String,
    #[serde(default)]
    pub linked_save_id: String,
    #[serde(default)]
    pub run_context_save_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParallelIsolationAuditReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub project_root: String,
    pub drafts: Vec<DraftIsolationSummary>,
    pub saves: Vec<SaveIsolationSummary>,
    pub issues: Vec<ParallelIsolationIssue>,
    pub status: String,
    pub issue_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParallelRepairAction {
    pub action: String,
    pub draft: String,
    #[serde(default)]
    pub old_linked_save_id: String,
    #[serde(default)]
    pub new_linked_save_id: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub reason: String,
    pub applied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParallelRepairReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub project_root: String,
    pub mode: String,
    pub actions: Vec<ParallelRepairAction>,
    pub action_count: usize,
}

#[derive(Debug, Default)]
struct HeldSaveLocks {
    archives: BTreeMap<String, HeldFileLock>,
    index: Option<HeldFileLock>,
    transaction: Option<HeldFileLock>,
    global_transaction: Option<HeldFileLock>,
}

#[derive(Debug)]
struct HeldFileLock {
    file: File,
    owner: std::thread::ThreadId,
    depth: usize,
}

#[derive(Debug, Default)]
struct SaveOperationGateState {
    owner: Option<std::thread::ThreadId>,
    depth: usize,
}

#[derive(Debug)]
struct SaveOperationGuard {
    gate: Arc<(Mutex<SaveOperationGateState>, Condvar)>,
}

#[derive(Debug)]
struct SaveTransactionGuard {
    held_locks: Arc<Mutex<HeldSaveLocks>>,
    owner: std::thread::ThreadId,
}

impl Drop for SaveTransactionGuard {
    fn drop(&mut self) {
        let Ok(mut handles) = self.held_locks.lock() else {
            return;
        };
        let Some((session_owner, session_depth)) = handles
            .transaction
            .as_ref()
            .map(|held| (held.owner, held.depth))
        else {
            return;
        };
        if session_owner != self.owner {
            return;
        }
        let Some((global_owner, global_depth)) = handles
            .global_transaction
            .as_ref()
            .map(|held| (held.owner, held.depth))
        else {
            return;
        };
        if global_owner != self.owner {
            return;
        }
        if session_depth > 1 && global_depth > 1 {
            if let Some(held) = &mut handles.transaction {
                held.depth -= 1;
            }
            if let Some(held) = &mut handles.global_transaction {
                held.depth -= 1;
            }
            return;
        }
        let session_held = handles.transaction.take();
        let global_held = handles.global_transaction.take();
        if let Some(held) = session_held {
            let _ = FileExt::unlock(&held.file);
        }
        if let Some(held) = global_held {
            let _ = FileExt::unlock(&held.file);
        }
    }
}

impl Drop for SaveOperationGuard {
    fn drop(&mut self) {
        let (state, wake) = &*self.gate;
        let Ok(mut state) = state.lock() else {
            return;
        };
        if state.owner == Some(std::thread::current().id()) {
            state.depth = state.depth.saturating_sub(1);
            if state.depth == 0 {
                state.owner = None;
                wake.notify_one();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SaveService {
    project_root: ProjectRoot,
    session_id: String,
    pid: u32,
    held_locks: Arc<Mutex<HeldSaveLocks>>,
    operation_gate: Arc<(Mutex<SaveOperationGateState>, Condvar)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SaveServiceReport {
    pub manifest: SaveManifest,
    pub index: SaveIndex,
    pub draft_meta: DraftMeta,
    pub file_map: FileMap,
    pub snapshot_manifest: SnapshotManifest,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadedSave {
    pub manifest: SaveManifest,
    pub draft_meta: DraftMeta,
    pub state: AutosaveState,
    pub warnings: Vec<String>,
}

impl SaveService {
    pub fn new(root: impl AsRef<Path>, session_id: &str) -> AdmResult<Self> {
        Self::with_pid(root, session_id, std::process::id())
    }

    pub fn with_pid(root: impl AsRef<Path>, session_id: &str, pid: u32) -> AdmResult<Self> {
        let session_id = sanitize_identifier(session_id)?;
        let project_root = ProjectRoot::new(root)?;
        recover_pending_transactions(&project_root)?;
        Ok(Self {
            project_root,
            session_id,
            pid,
            held_locks: Arc::new(Mutex::new(HeldSaveLocks::default())),
            operation_gate: Arc::new((
                Mutex::new(SaveOperationGateState::default()),
                Condvar::new(),
            )),
        })
    }

    pub fn project_root(&self) -> &ProjectRoot {
        &self.project_root
    }

    pub fn write_autosave(&self, state: &AutosaveState) -> AdmResult<DraftMeta> {
        let _operation = self.enter_operation()?;
        let draft = self.draft_repo()?;
        draft.autosave_state()?.write(state)?;
        let meta = match draft.draft_meta()?.read()? {
            Some(mut meta) => {
                meta.updated_at = timestamp();
                meta.session_id = self.session_id.clone();
                meta.pid = self.pid;
                meta
            }
            None => self.new_draft_meta(None, WorkspaceState::Unsaved, None)?,
        };
        draft.draft_meta()?.write(&meta)?;
        Ok(meta)
    }

    pub fn read_autosave(&self) -> AdmResult<Option<AutosaveState>> {
        let _operation = self.enter_operation()?;
        self.draft_repo()?.autosave_state()?.read()
    }

    pub fn recover_to_unsaved_state(&self, fallback: &AutosaveState) -> AdmResult<DraftMeta> {
        let _operation = self.enter_operation()?;
        let draft = self.draft_repo()?;
        let draft_save_id = draft
            .draft_meta()?
            .read()
            .ok()
            .flatten()
            .and_then(|meta| meta.linked_save_id);
        let index_save_id = self
            .load_index()
            .ok()
            .and_then(|index| index.current_save_id);
        draft.autosave_state()?.write(fallback)?;
        let meta = self.new_draft_meta(None, WorkspaceState::Unsaved, None)?;
        draft.draft_meta()?.write(&meta)?;
        self.update_index(|index| {
            index.current_save_id = None;
            index.updated_at = timestamp();
            Ok(())
        })?;
        let mut save_ids = [draft_save_id, index_save_id]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        save_ids.sort();
        save_ids.dedup();
        for save_id in save_ids {
            if safe_component("save_id", &save_id).is_ok() {
                self.release_archive_lock(&save_id)?;
            }
        }
        Ok(meta)
    }

    pub fn list_saves(&self) -> AdmResult<SaveIndex> {
        let _operation = self.enter_operation()?;
        let mut index = self.load_index()?;
        self.enrich_save_index(&mut index);
        Ok(index)
    }

    pub fn get_save(&self, save_id: &str) -> AdmResult<Option<SaveManifest>> {
        let _operation = self.enter_operation()?;
        let save_id = safe_component("save_id", save_id)?;
        let save_dir = self
            .project_root
            .resolve_relative(&format!("saves/{save_id}"))?;
        if !save_dir.exists() {
            return Ok(None);
        }
        self.read_archive_manifest(&save_id).map(Some)
    }

    pub fn create_save(
        &self,
        display_name: &str,
        state: &AutosaveState,
    ) -> AdmResult<SaveServiceReport> {
        let _operation = self.enter_operation()?;
        let _transaction = self.begin_transaction()?;
        let save_id = new_stable_id("save")?;
        let now = timestamp();
        let mut manifest = SaveManifest {
            schema_version: SAVE_SCHEMA_VERSION,
            save_id: save_id.clone(),
            display_name: display_name.trim().to_string(),
            save_type: "manual".to_string(),
            created_by: "design_workbench".to_string(),
            reason: "create_save".to_string(),
            created_at: now.clone(),
            last_worked_at: now,
            last_transaction_seq: 0,
            progress: progress_from_state(state, Some(&self.draft_root()?)),
            change_type: None,
            requested_version: None,
            iteration_spec_path: None,
            extra: BTreeMap::new(),
        };
        if manifest.display_name.is_empty() {
            manifest.display_name = "Untitled Save".to_string();
        }
        self.acquire_archive_lock(&save_id)?;
        self.archive_repo(&save_id)?.manifest()?.write(&manifest)?;
        match self.sync_save(&save_id, state, "create_save", "created save") {
            Ok(report) => Ok(report),
            Err(error) => {
                self.cleanup_failed_new_save(&save_id)?;
                Err(error)
            }
        }
    }

    pub fn create_blank_save(&self, display_name: &str) -> AdmResult<SaveServiceReport> {
        let _operation = self.enter_operation()?;
        self.create_blank_save_from_state(display_name, &ProjectState::empty())
    }

    pub fn create_blank_save_from_state(
        &self,
        display_name: &str,
        state: &AutosaveState,
    ) -> AdmResult<SaveServiceReport> {
        let _operation = self.enter_operation()?;
        self.create_blank_save_from_state_with_hook(display_name, state, |_| Ok(()))
    }

    fn create_blank_save_from_state_with_hook(
        &self,
        display_name: &str,
        state: &AutosaveState,
        hook: impl FnMut(&str) -> AdmResult<()>,
    ) -> AdmResult<SaveServiceReport> {
        let _transaction = self.begin_transaction()?;
        let previous_save_id = self.current_draft_save_id()?;
        let save_id = new_stable_id("save")?;
        let now = timestamp();
        let mut manifest = SaveManifest {
            schema_version: SAVE_SCHEMA_VERSION,
            save_id: save_id.clone(),
            display_name: display_name.trim().to_string(),
            save_type: "manual".to_string(),
            created_by: "design_workbench".to_string(),
            reason: "create_blank_save".to_string(),
            created_at: now.clone(),
            last_worked_at: now,
            last_transaction_seq: 0,
            progress: progress_from_state(state, None),
            change_type: None,
            requested_version: None,
            iteration_spec_path: None,
            extra: BTreeMap::new(),
        };
        if manifest.display_name.is_empty() {
            manifest.display_name = "Untitled Save".to_string();
        }
        let draft_root = self.draft_root()?;
        let blank_staging = prepare_blank_draft(&draft_root)?;
        self.acquire_index_lock()?;
        let transaction = (|| -> AdmResult<SaveServiceReport> {
            let draft = self.draft_repo()?;
            let index_path = SaveIndexRepository::new(&self.project_root)?
                .path()
                .to_path_buf();
            let draft_meta_path = draft.draft_meta()?.path().to_path_buf();
            let draft_file_map_path = draft.draft_file_map()?.path().to_path_buf();
            let timeline_path = draft_root.join("timeline.jsonl");
            let new_save_dir = self
                .project_root
                .resolve_relative(&format!("saves/{save_id}"))?;
            let new_snapshot_dir = draft_root.join("snapshots").join(format!("{save_id}_tx_1"));
            let before_image = TransactionBeforeImage {
                files: capture_transaction_files(
                    &self.project_root,
                    &[
                        &index_path,
                        &draft_meta_path,
                        &draft_file_map_path,
                        &timeline_path,
                    ],
                )?,
                directories: Vec::new(),
                remove_paths: vec![
                    transaction_root_relative(&self.project_root, &new_save_dir)?,
                    transaction_root_relative(&self.project_root, &new_snapshot_dir)?,
                ],
                archive_ids: vec![save_id.clone()],
            };
            self.acquire_archive_lock(&save_id)?;
            if let Err(error) = self.archive_repo(&save_id)?.manifest()?.write(&manifest) {
                let _ = remove_path_if_exists(&blank_staging);
                let cleanup = self.cleanup_failed_new_save(&save_id);
                return match cleanup {
                    Ok(()) => Err(error),
                    Err(cleanup_error) => Err(AdmError::new(format!(
                        "blank manifest write failed: {error}; cleanup failed: {cleanup_error}"
                    ))),
                };
            }
            let blank_swap = match swap_staged_selected_paths_journaled(
                &self.project_root,
                &self.session_id,
                "blank",
                &blank_staging,
                &draft_root,
                BLANK_DRAFT_REPLACE_PATHS,
                &[],
                before_image,
            ) {
                Ok(swap) => swap,
                Err(error) => {
                    let _ = remove_path_if_exists(&blank_staging);
                    self.cleanup_failed_new_save(&save_id)?;
                    return Err(error);
                }
            };
            let mut report = match self.sync_save_with_hook(
                &save_id,
                state,
                "create_blank_save",
                "created blank save",
                hook,
            ) {
                Ok(report) => report,
                Err(error) => {
                    let cleanup = self.cleanup_failed_new_save(&save_id);
                    let rollback = blank_swap.rollback();
                    return match (cleanup, rollback) {
                        (Ok(()), Ok(())) => Err(error),
                        (cleanup, rollback) => Err(AdmError::new(format!(
                            "blank save failed: {error}; cleanup failed: {}; draft rollback failed: {}",
                            cleanup
                                .err()
                                .map(|error| error.to_string())
                                .unwrap_or_else(|| "none".to_string()),
                            rollback
                                .err()
                                .map(|error| error.to_string())
                                .unwrap_or_else(|| "none".to_string())
                        ))),
                    };
                }
            };
            if let Err(error) = blank_swap.mark_committed() {
                let rollback = blank_swap.rollback_with_before_image(&self.project_root);
                let release_new = self.release_archive_lock(&save_id);
                let reacquire_previous = previous_save_id
                    .as_deref()
                    .filter(|previous| *previous != save_id)
                    .map(|previous| self.acquire_archive_lock(previous))
                    .transpose();
                return match (rollback, release_new, reacquire_previous) {
                    (Ok(()), Ok(()), Ok(_)) => Err(error),
                    (rollback, release, reacquire) => Err(AdmError::new(format!(
                        "blank transaction commit marker failed: {error}; rollback: {}; new lock release: {}; previous lock restore: {}",
                        cleanup_result_message(rollback),
                        cleanup_result_message(release),
                        cleanup_result_message(reacquire.map(|_| ()))
                    ))),
                };
            }
            report.warnings.extend(blank_swap.finalize());
            if let Some(previous_save_id) = &previous_save_id
                && previous_save_id != &save_id
                && let Some(warning) = self.retry_archive_lock_release_warning(previous_save_id)
            {
                report.warnings.push(warning);
            }
            Ok(report)
        })();
        let release_warning = self.retry_index_lock_release_warning();
        match transaction {
            Ok(mut report) => {
                if let Some(warning) = release_warning {
                    report.warnings.push(warning);
                }
                self.persist_report_warnings("create_blank_save", &mut report.warnings);
                Ok(report)
            }
            Err(error) => match release_warning {
                None => Err(error),
                Some(warning) => Err(AdmError::new(format!(
                    "blank save failed: {error}; {warning}"
                ))),
            },
        }
    }

    pub fn create_iteration_save(
        &self,
        display_name: &str,
        state: &AutosaveState,
        change_type: &str,
        requested_version: &str,
        iteration_spec_path: &str,
    ) -> AdmResult<SaveServiceReport> {
        let _operation = self.enter_operation()?;
        let mut report = self.create_save(display_name, state)?;
        let save_id = report.manifest.save_id.clone();
        let archive = self.archive_repo(&save_id)?;
        let mut manifest = archive.manifest()?.read_required()?;
        manifest.save_type = "iteration".to_string();
        manifest.reason = "create_iteration_save".to_string();
        manifest.change_type = Some(change_type.trim().to_string());
        manifest.requested_version = Some(requested_version.trim().to_string());
        manifest.iteration_spec_path = Some(iteration_spec_path.trim().to_string());
        manifest.last_worked_at = timestamp();
        archive.manifest()?.write(&manifest)?;
        report.manifest = manifest.clone();
        report.index = self.update_index(|index| {
            upsert_index_entry(index, index_entry_from_manifest(&manifest));
            index.updated_at = timestamp();
            Ok(())
        })?;
        Ok(report)
    }

    pub fn sync_current_save(
        &self,
        state: &AutosaveState,
        reason: &str,
    ) -> AdmResult<SaveServiceReport> {
        let _operation = self.enter_operation()?;
        let meta = self.draft_repo()?.draft_meta()?.read_required()?;
        let save_id = meta
            .linked_save_id
            .ok_or_else(|| AdmError::new("cannot sync without a linked save"))?;
        self.sync_save(&save_id, state, reason, "synced current save")
    }

    pub fn load_save(&self, save_id: &str) -> AdmResult<LoadedSave> {
        let _operation = self.enter_operation()?;
        self.load_save_with_hook(save_id, |_| Ok(()))
    }

    fn load_save_with_hook(
        &self,
        save_id: &str,
        mut hook: impl FnMut(&str) -> AdmResult<()>,
    ) -> AdmResult<LoadedSave> {
        let _transaction = self.begin_transaction()?;
        let save_id = safe_component("save_id", save_id)?;
        let previous_save_id = self.current_draft_save_id()?;
        self.acquire_archive_lock(&save_id)?;
        let load_result = (|| {
            let manifest = self.read_archive_manifest(&save_id)?;
            let state = self.read_archive_state(&save_id)?;
            let archive_workspace = self.archive_workspace_dir(&save_id)?;
            let draft_root = self.draft_root()?;
            let draft_staging = prepare_selected_paths(
                &archive_workspace,
                &draft_root,
                ARCHIVED_DRAFT_RELATIVE_PATHS,
                ARCHIVE_EXCLUDED_RELATIVE_PATHS,
            )?;
            write_json_value(
                &draft_staging.join("autosave_state.json"),
                &serde_json::to_value(&state).map_err(|error| {
                    AdmError::new(format!(
                        "failed to serialize restored project state: {error}"
                    ))
                })?,
            )?;
            ensure_empty_workspace_dirs(&draft_staging)?;
            let file_map = build_file_map(
                &archive_workspace,
                manifest.last_transaction_seq,
                timestamp(),
            )?;
            let meta = self.new_draft_meta(
                Some(save_id.clone()),
                WorkspaceState::LinkedSave,
                Some(format!("saves/{save_id}")),
            )?;
            let current_draft = self.draft_repo()?;
            let old_draft_file_map = current_draft.draft_file_map()?.read()?;
            let old_draft_meta = current_draft.draft_meta()?.read()?;
            if let Err(error) = self.acquire_index_lock() {
                let _ = remove_path_if_exists(&draft_staging);
                return Err(error);
            }
            let old_index = match self.load_index_unlocked() {
                Ok(index) => index,
                Err(error) => {
                    let _ = self.release_index_lock();
                    let _ = remove_path_if_exists(&draft_staging);
                    return Err(error);
                }
            };
            let mut index = old_index.clone();
            index.current_save_id = Some(save_id.clone());
            index.updated_at = timestamp();
            let before_image = (|| -> AdmResult<TransactionBeforeImage> {
                let index_path = SaveIndexRepository::new(&self.project_root)?
                    .path()
                    .to_path_buf();
                let draft_meta_path = current_draft.draft_meta()?.path().to_path_buf();
                let draft_file_map_path = current_draft.draft_file_map()?.path().to_path_buf();
                Ok(TransactionBeforeImage {
                    files: capture_transaction_files(
                        &self.project_root,
                        &[&index_path, &draft_meta_path, &draft_file_map_path],
                    )?,
                    archive_ids: vec![save_id.clone()],
                    ..Default::default()
                })
            })();
            let before_image = match before_image {
                Ok(before_image) => before_image,
                Err(error) => {
                    let _ = self.release_index_lock();
                    let _ = remove_path_if_exists(&draft_staging);
                    return Err(error);
                }
            };
            let mut draft_swap = None;
            let commit = (|| -> AdmResult<()> {
                draft_swap = Some(swap_staged_selected_paths_journaled(
                    &self.project_root,
                    &self.session_id,
                    "load",
                    &draft_staging,
                    &draft_root,
                    ARCHIVED_DRAFT_RELATIVE_PATHS,
                    ARCHIVE_EXCLUDED_RELATIVE_PATHS,
                    before_image,
                )?);
                hook("after_draft_swap")?;
                let draft = self.draft_repo()?;
                draft.draft_file_map()?.write(&file_map)?;
                draft.draft_meta()?.write(&meta)?;
                hook("after_draft_meta")?;
                self.write_index_unlocked(index)?;
                hook("after_index")?;
                if let Some(swap) = &draft_swap {
                    swap.mark_committed()?;
                }
                Ok(())
            })();
            if let Err(error) = commit {
                let mut rollback_errors = Vec::new();
                if let Err(rollback_error) = self.write_index_unlocked(old_index) {
                    rollback_errors.push(format!("index: {rollback_error}"));
                }
                if let Err(rollback_error) =
                    restore_optional_draft_meta(&current_draft, old_draft_meta)
                {
                    rollback_errors.push(format!("draft meta: {rollback_error}"));
                }
                if let Err(rollback_error) =
                    restore_optional_file_map(&current_draft, old_draft_file_map)
                {
                    rollback_errors.push(format!("draft file map: {rollback_error}"));
                }
                if let Some(swap) = draft_swap
                    && let Err(rollback_error) = swap.rollback()
                {
                    rollback_errors.push(format!("draft: {rollback_error}"));
                }
                let _ = remove_path_if_exists(&draft_staging);
                if let Err(release_error) = self.release_index_lock() {
                    rollback_errors.push(format!("index lock: {release_error}"));
                }
                if rollback_errors.is_empty() {
                    return Err(error);
                }
                return Err(AdmError::new(format!(
                    "load transaction failed: {error}; rollback failed: {}",
                    rollback_errors.join("; ")
                )));
            }
            let mut warnings = Vec::new();
            if let Some(warning) = self.retry_index_lock_release_warning() {
                warnings.push(warning);
            }
            if let Some(swap) = draft_swap {
                warnings.extend(swap.finalize());
            }
            Ok(LoadedSave {
                manifest,
                draft_meta: meta,
                state,
                warnings,
            })
        })();

        match load_result {
            Ok(mut loaded) => {
                if let Some(previous_save_id) = previous_save_id
                    && previous_save_id != save_id
                {
                    if let Some(warning) =
                        self.retry_archive_lock_release_warning(&previous_save_id)
                    {
                        loaded.warnings.push(warning);
                    }
                }
                self.persist_report_warnings("load_save", &mut loaded.warnings);
                Ok(loaded)
            }
            Err(error) => {
                if previous_save_id.as_deref() != Some(save_id.as_str()) {
                    let _ = self.release_archive_lock(&save_id);
                }
                Err(error)
            }
        }
    }

    pub fn delete_save(&self, save_id: &str) -> AdmResult<SaveIndex> {
        let _operation = self.enter_operation()?;
        self.delete_save_with_hook(save_id, |_| Ok(()))
    }

    fn delete_save_with_hook(
        &self,
        save_id: &str,
        mut hook: impl FnMut(&str) -> AdmResult<()>,
    ) -> AdmResult<SaveIndex> {
        let _transaction = self.begin_transaction()?;
        let save_id = safe_component("save_id", save_id)?;
        let was_current = self.current_or_index_save_id()?.as_deref() == Some(save_id.as_str());
        let save_dir = self
            .project_root
            .resolve_relative(&format!("saves/{save_id}"))?;
        let had_archive = save_dir.is_dir();
        if had_archive {
            self.acquire_archive_lock(&save_id)?;
        }
        let mut journal = None;
        let mut index_locked = false;
        let result = (|| -> AdmResult<SaveIndex> {
            self.acquire_index_lock()?;
            index_locked = true;
            let mut index = self.load_index_unlocked()?;
            let index_path = SaveIndexRepository::new(&self.project_root)?
                .path()
                .to_path_buf();
            let draft = self.draft_repo()?;
            let draft_meta_path = draft.draft_meta()?.path().to_path_buf();
            let tombstone = unique_sibling_path(&save_dir, "delete-tombstone")?;
            let tombstone_staging = unique_sibling_path(&save_dir, "delete-staging")?;
            let before_image = TransactionBeforeImage {
                files: capture_transaction_files(
                    &self.project_root,
                    &[&index_path, &draft_meta_path],
                )?,
                directories: if had_archive {
                    vec![TransactionDirectoryBeforeImage {
                        target: transaction_root_relative(&self.project_root, &save_dir)?,
                        staging: transaction_root_relative(&self.project_root, &tombstone_staging)?,
                        backup: transaction_root_relative(&self.project_root, &tombstone)?,
                        had_target: true,
                    }]
                } else {
                    Vec::new()
                },
                archive_ids: had_archive.then(|| save_id.clone()).into_iter().collect(),
                ..Default::default()
            };
            journal = Some(self.create_metadata_transaction_journal("delete", before_image)?);
            hook("after_journal")?;
            if had_archive {
                fs::rename(&save_dir, &tombstone)?;
            }
            hook("after_tombstone")?;
            index.saves.retain(|entry| entry.save_id != save_id);
            if index.current_save_id.as_deref() == Some(save_id.as_str()) {
                index.current_save_id = None;
            }
            index.updated_at = timestamp();
            self.write_index_unlocked(index.clone())?;
            hook("after_index")?;
            if was_current {
                let mut meta = self.new_draft_meta(
                    None,
                    WorkspaceState::UnsavedCopyOfDeletedSave,
                    Some(format!("saves/{save_id}")),
                )?;
                meta.origin_deleted_save_id = Some(save_id.clone());
                draft.draft_meta()?.write(&meta)?;
            }
            hook("after_draft_meta")?;
            if let Some(transaction) = &journal {
                transaction.mark_committed()?;
            }
            let mut warnings = Vec::new();
            if let Some(warning) = self.retry_index_lock_release_warning() {
                warnings.push(warning);
            }
            index_locked = false;
            if let Some(transaction) = journal.take() {
                warnings.extend(transaction.finalize());
            }
            self.persist_report_warnings("delete_save", &mut warnings);
            Ok(index)
        })();
        let result = match result {
            Ok(index) => Ok(index),
            Err(error) => {
                let mut rollback_errors = Vec::new();
                if let Some(transaction) = journal.take()
                    && let Err(rollback_error) =
                        transaction.rollback_with_before_image(&self.project_root)
                {
                    rollback_errors.push(format!("transaction: {rollback_error}"));
                }
                if index_locked && let Err(release_error) = self.release_index_lock() {
                    rollback_errors.push(format!("index lock: {release_error}"));
                }
                if rollback_errors.is_empty() {
                    Err(error)
                } else {
                    Err(AdmError::new(format!(
                        "delete failed: {error}; rollback failed: {}",
                        rollback_errors.join("; ")
                    )))
                }
            }
        };
        let should_release_archive = had_archive && (result.is_ok() || !was_current);
        let release_warning = should_release_archive
            .then(|| self.retry_archive_lock_release_warning(&save_id))
            .flatten();
        match (result, release_warning) {
            (Ok(index), None) => Ok(index),
            (Ok(index), Some(warning)) => {
                let mut warnings = vec![warning];
                self.persist_report_warnings("delete_save", &mut warnings);
                Ok(index)
            }
            (Err(error), None) => Err(error),
            (Err(error), Some(warning)) => {
                Err(AdmError::new(format!("delete failed: {error}; {warning}")))
            }
        }
    }

    pub fn rename_save(&self, save_id: &str, display_name: &str) -> AdmResult<SaveIndex> {
        let _operation = self.enter_operation()?;
        self.rename_save_with_hook(save_id, display_name, |_| Ok(()))
    }

    fn rename_save_with_hook(
        &self,
        save_id: &str,
        display_name: &str,
        mut hook: impl FnMut(&str) -> AdmResult<()>,
    ) -> AdmResult<SaveIndex> {
        let _transaction = self.begin_transaction()?;
        let save_id = safe_component("save_id", save_id)?;
        let display_name = display_name.trim();
        if display_name.is_empty() {
            return Err(AdmError::new("display_name cannot be empty"));
        }
        let is_current = self.current_or_index_save_id()?.as_deref() == Some(save_id.as_str());
        self.acquire_archive_lock(&save_id)?;
        let mut journal = None;
        let mut index_locked = false;
        let result = (|| -> AdmResult<SaveIndex> {
            self.acquire_index_lock()?;
            index_locked = true;
            let archive = self.archive_repo(&save_id)?;
            let mut manifest = self.read_archive_manifest(&save_id)?;
            manifest.display_name = display_name.to_string();
            manifest.last_worked_at = timestamp();
            let mut index = self.load_index_unlocked()?;
            let manifest_path = archive.manifest()?.path().to_path_buf();
            let index_path = SaveIndexRepository::new(&self.project_root)?
                .path()
                .to_path_buf();
            let before_image = TransactionBeforeImage {
                files: capture_transaction_files(
                    &self.project_root,
                    &[&manifest_path, &index_path],
                )?,
                archive_ids: vec![save_id.clone()],
                ..Default::default()
            };
            journal = Some(self.create_metadata_transaction_journal("rename", before_image)?);
            hook("after_journal")?;
            archive.manifest()?.write(&manifest)?;
            hook("after_manifest")?;
            upsert_index_entry(&mut index, index_entry_from_manifest(&manifest));
            index.updated_at = timestamp();
            self.write_index_unlocked(index.clone())?;
            hook("after_index")?;
            if let Some(transaction) = &journal {
                transaction.mark_committed()?;
            }
            let mut warnings = Vec::new();
            if let Some(warning) = self.retry_index_lock_release_warning() {
                warnings.push(warning);
            }
            index_locked = false;
            if let Some(transaction) = journal.take() {
                warnings.extend(transaction.finalize());
            }
            self.persist_report_warnings("rename_save", &mut warnings);
            Ok(index)
        })();
        let result = match result {
            Ok(index) => Ok(index),
            Err(error) => {
                let mut rollback_errors = Vec::new();
                if let Some(transaction) = journal.take()
                    && let Err(rollback_error) =
                        transaction.rollback_with_before_image(&self.project_root)
                {
                    rollback_errors.push(format!("transaction: {rollback_error}"));
                }
                if index_locked && let Err(release_error) = self.release_index_lock() {
                    rollback_errors.push(format!("index lock: {release_error}"));
                }
                if rollback_errors.is_empty() {
                    Err(error)
                } else {
                    Err(AdmError::new(format!(
                        "rename failed: {error}; rollback failed: {}",
                        rollback_errors.join("; ")
                    )))
                }
            }
        };
        if is_current {
            return result;
        }
        let release_warning = self.retry_archive_lock_release_warning(&save_id);
        match (result, release_warning) {
            (Ok(index), None) => Ok(index),
            (Err(error), None) => Err(error),
            (Ok(index), Some(warning)) => {
                let mut warnings = vec![warning];
                self.persist_report_warnings("rename_save", &mut warnings);
                Ok(index)
            }
            (Err(error), Some(release_warning)) => Err(AdmError::new(format!(
                "rename failed: {error}; {release_warning}"
            ))),
        }
    }

    pub fn repair_blank_save_progress(
        &self,
        save_id: &str,
        apply: bool,
    ) -> AdmResult<BlankSaveRepairReport> {
        let _operation = self.enter_operation()?;
        let save_id = safe_component("save_id", save_id)?;
        let archive = self.archive_repo(&save_id)?;
        let mut manifest = self.read_archive_manifest(&save_id)?;
        let workspace = self.archive_workspace_dir(&save_id)?;
        if !workspace.is_dir() {
            return Err(AdmError::new(format!(
                "save workspace is missing: {save_id}"
            )));
        }
        let cleanup_paths = existing_blank_cleanup_paths(&workspace)?;
        let old_progress = manifest.progress.clone();
        let mut removed_paths = Vec::new();
        if apply {
            for path in &cleanup_paths {
                remove_path_if_exists(path)?;
                removed_paths.push(relative_to_root(self.project_root.path(), path));
            }
            ensure_empty_workspace_dirs(&workspace)?;
            self.write_archive_state(&save_id, &ProjectState::empty())?;
            manifest.progress = blank_progress();
            manifest.last_worked_at = timestamp();
            archive.manifest()?.write(&manifest)?;
            self.update_index(|index| {
                upsert_index_entry(index, index_entry_from_manifest(&manifest));
                index.updated_at = timestamp();
                Ok(())
            })?;
            append_jsonl_value(
                &self
                    .project_root
                    .resolve_relative(&format!("saves/{save_id}/repair_log.jsonl"))?,
                &serde_json::json!({
                    "event": "repair_blank_save_progress",
                    "timestamp": timestamp(),
                    "save_id": save_id.clone(),
                    "old_progress": old_progress.clone(),
                    "new_progress": manifest.progress.clone(),
                    "removed_paths": removed_paths.clone(),
                }),
            )?;
        }
        Ok(BlankSaveRepairReport {
            save_id,
            apply,
            old_progress,
            new_progress: manifest.progress,
            cleanup_paths: cleanup_paths
                .iter()
                .map(|path| relative_to_root(self.project_root.path(), path))
                .collect(),
            removed_paths,
        })
    }

    pub fn audit_parallel_isolation(&self) -> AdmResult<ParallelIsolationAuditReport> {
        let _operation = self.enter_operation()?;
        let mut issues = Vec::new();
        let drafts = self.audit_drafts(&mut issues)?;
        let saves = self.audit_saves(&mut issues)?;
        Ok(ParallelIsolationAuditReport {
            schema_version: SAVE_SCHEMA_VERSION,
            generated_at: timestamp(),
            project_root: self.project_root.path().display().to_string(),
            drafts,
            saves,
            status: if issues.is_empty() {
                "passed".to_string()
            } else {
                "issues_found".to_string()
            },
            issue_count: issues.len(),
            issues,
        })
    }

    pub fn repair_parallel_save_contamination(
        &self,
        apply: bool,
    ) -> AdmResult<ParallelRepairReport> {
        let _operation = self.enter_operation()?;
        let drafts_root = self.project_root.drafts_dir();
        let mut actions = Vec::new();
        if drafts_root.is_dir() {
            for entry in fs::read_dir(&drafts_root)? {
                let draft = entry?.path();
                if !draft.is_dir() {
                    continue;
                }
                let context_path = draft.join("runtime").join("run_context.json");
                let context = read_json_value(&context_path)?.unwrap_or(Value::Null);
                let Some(save_id) = json_string(&context, "save_id") else {
                    continue;
                };
                let save_dir = self.project_root.saves_dir().join(&save_id);
                let draft_name = draft_name(&draft);
                if !save_dir.is_dir() {
                    actions.push(ParallelRepairAction {
                        action: "skip_missing_save".to_string(),
                        draft: draft_name,
                        old_linked_save_id: String::new(),
                        new_linked_save_id: save_id,
                        path: relative_to_root(self.project_root.path(), &context_path),
                        reason: "run_context save_id does not exist in saves/".to_string(),
                        applied: false,
                    });
                    continue;
                }
                let meta_path = draft.join("draft_meta.json");
                let mut meta = read_json_value(&meta_path)?
                    .unwrap_or_else(|| Value::Object(Default::default()));
                let old_save = linked_save_id_from_meta_value(&meta);
                if old_save == save_id {
                    continue;
                }
                let action = ParallelRepairAction {
                    action: "repair_draft_meta_linked_save".to_string(),
                    draft: draft_name,
                    old_linked_save_id: old_save.clone(),
                    new_linked_save_id: save_id.clone(),
                    path: relative_to_root(self.project_root.path(), &meta_path),
                    reason: "runtime/run_context.json save_id".to_string(),
                    applied: apply,
                };
                if apply {
                    repair_draft_meta_value(&mut meta, &save_id, &old_save);
                    write_json_value(&meta_path, &meta)?;
                }
                actions.push(action);
            }
        }
        Ok(ParallelRepairReport {
            schema_version: SAVE_SCHEMA_VERSION,
            generated_at: timestamp(),
            project_root: self.project_root.path().display().to_string(),
            mode: if apply { "apply" } else { "dry_run" }.to_string(),
            action_count: actions.len(),
            actions,
        })
    }

    pub fn release_current_lock(&self) -> AdmResult<()> {
        let _operation = self.enter_operation()?;
        let Some(save_id) = self.current_or_index_save_id()? else {
            return Ok(());
        };
        self.release_archive_lock(&save_id)
    }

    pub fn acquire_current_lock(&self) -> AdmResult<()> {
        let _operation = self.enter_operation()?;
        let Some(save_id) = self.current_or_index_save_id()? else {
            return Ok(());
        };
        let save_dir = self
            .project_root
            .resolve_relative(&format!("saves/{save_id}"))?;
        if !save_dir.is_dir() {
            return Err(AdmError::new(format!(
                "current save archive is missing: {save_id}"
            )));
        }
        self.acquire_archive_lock(&save_id)?;
        Ok(())
    }

    pub fn release_archive_lock(&self, save_id: &str) -> AdmResult<()> {
        let _operation = self.enter_operation()?;
        let save_id = safe_component("save_id", save_id)?;
        let current = std::thread::current().id();
        let file = {
            let mut handles = self.lock_handles()?;
            let Some(held) = handles.archives.get(&save_id) else {
                return Ok(());
            };
            if held.owner != current && !self.operation_owned_by_current_thread() {
                return Err(AdmError::new(
                    "archive lock release attempted by a concurrent save operation",
                ));
            }
            handles.archives.remove(&save_id)
        };
        let Some(held) = file else {
            return Ok(());
        };
        let lock_repo = self.archive_repo(&save_id)?.archive_lock()?;
        let mut metadata_error = None;
        if lock_repo.path().parent().is_some_and(Path::is_dir) {
            let released = ArchiveLock {
                pid: self.pid,
                session_id: self.session_id.clone(),
                acquired_at: timestamp(),
                live: Some(false),
                lock_path: Some(relative_to_root(
                    self.project_root.path(),
                    &self.archive_os_lock_path(&save_id)?,
                )),
            };
            if let Err(error) = serde_json::to_value(&released)
                .map_err(|error| {
                    AdmError::new(format!("failed to serialize archive lock: {error}"))
                })
                .and_then(|value| write_json_value(lock_repo.path(), &value))
            {
                metadata_error = Some(error);
            }
        }
        let unlock = FileExt::unlock(&held.file).map_err(AdmError::from);
        match (metadata_error, unlock) {
            (None, Ok(())) => Ok(()),
            (Some(error), Ok(())) => Err(error),
            (None, Err(error)) => Err(error),
            (Some(metadata_error), Err(unlock_error)) => Err(AdmError::new(format!(
                "archive lock metadata update failed: {metadata_error}; unlock failed: {unlock_error}"
            ))),
        }
    }

    fn sync_save(
        &self,
        save_id: &str,
        state: &AutosaveState,
        event: &str,
        message: &str,
    ) -> AdmResult<SaveServiceReport> {
        self.sync_save_with_hook(save_id, state, event, message, |_| Ok(()))
    }

    fn sync_save_with_hook(
        &self,
        save_id: &str,
        state: &AutosaveState,
        event: &str,
        message: &str,
        mut hook: impl FnMut(&str) -> AdmResult<()>,
    ) -> AdmResult<SaveServiceReport> {
        let _transaction = self.begin_transaction()?;
        let previous_save_id = self.current_draft_save_id()?;
        self.acquire_archive_lock(save_id)?;
        let archive = self.archive_repo(save_id)?;
        let old_manifest = self.read_archive_manifest(save_id)?;
        let next_seq = old_manifest.last_transaction_seq + 1;
        let now = timestamp();
        let draft = self.draft_repo()?;
        draft.autosave_state()?.write(state)?;
        let previous_map = if previous_save_id.as_deref() == Some(save_id) {
            draft
                .draft_file_map()?
                .read()?
                .unwrap_or_else(empty_file_map)
        } else {
            empty_file_map()
        };
        let archive_workspace = self.archive_workspace_dir(save_id)?;
        let workspace_staging = prepare_selected_paths(
            &self.draft_root()?,
            &archive_workspace,
            ARCHIVED_DRAFT_RELATIVE_PATHS,
            ARCHIVE_EXCLUDED_RELATIVE_PATHS,
        )?;
        let file_map = match build_file_map(&workspace_staging, next_seq, now.clone()) {
            Ok(file_map) => file_map,
            Err(error) => {
                let _ = remove_path_if_exists(&workspace_staging);
                return Err(error);
            }
        };
        let delta = diff_file_maps(&previous_map, &file_map);
        let snapshot_manifest = SnapshotManifest {
            schema_version: SAVE_SCHEMA_VERSION,
            seq: next_seq,
            event: event.to_string(),
            stage: None,
            timestamp: now.clone(),
            message: message.to_string(),
            file_count: file_map.files.len() as u32,
            added: delta.added.len() as u32,
            modified: delta.modified.len() as u32,
            removed: delta.removed.len() as u32,
        };
        let snapshot_prepared = self.prepare_snapshot(
            save_id,
            &snapshot_manifest,
            &file_map,
            &delta,
            &workspace_staging,
        );
        let (snapshot_staging, snapshot_dir) = match snapshot_prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                let _ = remove_path_if_exists(&workspace_staging);
                return Err(error);
            }
        };
        let mut manifest = old_manifest.clone();
        manifest.last_transaction_seq = next_seq;
        manifest.last_worked_at = now.clone();
        manifest.reason = event.to_string();
        manifest.progress = progress_from_state(state, Some(&self.draft_root()?));
        let draft_meta = self.new_draft_meta(
            Some(save_id.to_string()),
            WorkspaceState::LinkedSave,
            Some(format!("saves/{save_id}")),
        )?;
        let timeline_entry = TimelineEntry {
            seq: next_seq,
            save_id: save_id.to_string(),
            event: event.to_string(),
            stage: None,
            timestamp: now.clone(),
            message: message.to_string(),
            progress: manifest.progress.clone(),
        };
        let old_draft_file_map = draft.draft_file_map()?.read()?;
        let old_draft_meta = draft.draft_meta()?.read()?;
        let timeline_path = self
            .project_root
            .resolve_relative(&format!("drafts/{}/timeline.jsonl", self.session_id))?;
        let old_timeline_len = fs::metadata(&timeline_path)
            .ok()
            .map(|metadata| metadata.len());

        if let Err(error) = self.acquire_index_lock() {
            let _ = remove_path_if_exists(&workspace_staging);
            let _ = remove_path_if_exists(&snapshot_staging);
            return Err(error);
        }
        let old_index = match self.load_index_unlocked() {
            Ok(index) => index,
            Err(error) => {
                let _ = self.release_index_lock();
                let _ = remove_path_if_exists(&workspace_staging);
                let _ = remove_path_if_exists(&snapshot_staging);
                return Err(error);
            }
        };
        let mut index = old_index.clone();
        index.current_save_id = Some(save_id.to_string());
        index.updated_at = timestamp();
        upsert_index_entry(
            &mut index,
            index_entry_from_manifest_and_file_map(&manifest, &file_map),
        );
        index.saves.sort_by(|left, right| {
            right
                .last_worked_at
                .cmp(&left.last_worked_at)
                .then_with(|| left.display_name.cmp(&right.display_name))
        });
        let transaction_setup = (|| -> AdmResult<(PathBuf, TransactionBeforeImage)> {
            let snapshot_backup = unique_sibling_path(&snapshot_dir, "backup")?;
            let index_path = SaveIndexRepository::new(&self.project_root)?
                .path()
                .to_path_buf();
            let manifest_path = archive.manifest()?.path().to_path_buf();
            let draft_meta_path = draft.draft_meta()?.path().to_path_buf();
            let draft_file_map_path = draft.draft_file_map()?.path().to_path_buf();
            let before_image = TransactionBeforeImage {
                files: capture_transaction_files(
                    &self.project_root,
                    &[
                        &index_path,
                        &manifest_path,
                        &draft_meta_path,
                        &draft_file_map_path,
                        &timeline_path,
                    ],
                )?,
                directories: vec![TransactionDirectoryBeforeImage {
                    target: transaction_root_relative(&self.project_root, &snapshot_dir)?,
                    staging: transaction_root_relative(&self.project_root, &snapshot_staging)?,
                    backup: transaction_root_relative(&self.project_root, &snapshot_backup)?,
                    had_target: snapshot_dir.exists(),
                }],
                remove_paths: Vec::new(),
                archive_ids: vec![save_id.to_string()],
            };
            Ok((snapshot_backup, before_image))
        })();
        let (snapshot_backup, before_image) = match transaction_setup {
            Ok(setup) => setup,
            Err(error) => {
                let _ = self.release_index_lock();
                let _ = remove_path_if_exists(&workspace_staging);
                let _ = remove_path_if_exists(&snapshot_staging);
                return Err(error);
            }
        };

        let mut workspace_swap = None;
        let mut snapshot_swap = None;
        let commit = (|| -> AdmResult<()> {
            workspace_swap = Some(swap_staged_selected_paths_journaled(
                &self.project_root,
                &self.session_id,
                "formal",
                &workspace_staging,
                &archive_workspace,
                ARCHIVED_DRAFT_RELATIVE_PATHS,
                ARCHIVE_EXCLUDED_RELATIVE_PATHS,
                before_image,
            )?);
            hook("after_workspace_swap")?;
            snapshot_swap = Some(swap_staged_directory_with_backup(
                &snapshot_staging,
                &snapshot_dir,
                &snapshot_backup,
            )?);
            hook("after_snapshot_swap")?;
            archive.manifest()?.write(&manifest)?;
            hook("after_manifest")?;
            draft.draft_file_map()?.write(&file_map)?;
            draft.draft_meta()?.write(&draft_meta)?;
            hook("after_draft_meta")?;
            self.append_timeline(timeline_entry.clone())?;
            hook("after_timeline")?;
            self.write_index_unlocked(index.clone())?;
            hook("after_index")?;
            if let Some(swap) = &workspace_swap {
                swap.mark_committed()?;
            }
            Ok(())
        })();
        if let Err(error) = commit {
            let mut rollback_errors = Vec::new();
            if let Err(rollback_error) = self.write_index_unlocked(old_index) {
                rollback_errors.push(format!("index: {rollback_error}"));
            }
            if let Err(rollback_error) = restore_append_only_file(&timeline_path, old_timeline_len)
            {
                rollback_errors.push(format!("timeline: {rollback_error}"));
            }
            if let Err(rollback_error) = restore_optional_draft_meta(&draft, old_draft_meta) {
                rollback_errors.push(format!("draft meta: {rollback_error}"));
            }
            if let Err(rollback_error) = restore_optional_file_map(&draft, old_draft_file_map) {
                rollback_errors.push(format!("draft file map: {rollback_error}"));
            }
            if let Err(rollback_error) = archive.manifest()?.write(&old_manifest) {
                rollback_errors.push(format!("manifest: {rollback_error}"));
            }
            if let Some(swap) = snapshot_swap
                && let Err(rollback_error) = swap.rollback()
            {
                rollback_errors.push(format!("snapshot: {rollback_error}"));
            }
            if let Some(swap) = workspace_swap
                && let Err(rollback_error) = swap.rollback()
            {
                rollback_errors.push(format!("workspace: {rollback_error}"));
            }
            let _ = remove_path_if_exists(&snapshot_staging);
            let _ = remove_path_if_exists(&workspace_staging);
            if let Err(release_error) = self.release_index_lock() {
                rollback_errors.push(format!("index lock: {release_error}"));
            }
            if rollback_errors.is_empty() {
                return Err(error);
            }
            return Err(AdmError::new(format!(
                "save transaction failed: {error}; rollback failed: {}",
                rollback_errors.join("; ")
            )));
        }
        let mut warnings = Vec::new();
        if let Some(warning) = self.retry_index_lock_release_warning() {
            warnings.push(warning);
        }
        if let Some(swap) = snapshot_swap {
            warnings.extend(swap.finalize());
        }
        if let Some(swap) = workspace_swap {
            warnings.extend(swap.finalize());
        }
        if let Err(error) = self.prune_snapshots(save_id, SNAPSHOT_KEEP_PER_SAVE) {
            warnings.push(format!("snapshot pruning failed: {error}"));
        }
        if let Some(previous_save_id) = previous_save_id
            && previous_save_id != save_id
        {
            if let Some(warning) = self.retry_archive_lock_release_warning(&previous_save_id) {
                warnings.push(warning);
            }
        }
        self.persist_report_warnings("sync_save", &mut warnings);
        let report = SaveServiceReport {
            manifest,
            index,
            draft_meta,
            file_map,
            snapshot_manifest,
            warnings,
        };
        Ok(report)
    }

    fn acquire_archive_lock(&self, save_id: &str) -> AdmResult<ArchiveLock> {
        let save_id = safe_component("save_id", save_id)?;
        let current = std::thread::current().id();
        {
            let mut handles = self.lock_handles()?;
            if let Some(held) = handles.archives.get_mut(&save_id) {
                if held.owner != current && !self.operation_owned_by_current_thread() {
                    return Err(AdmError::new(
                        "save archive is already in use by another operation in this process",
                    ));
                }
                held.owner = current;
                return Ok(self.new_archive_lock_metadata(&save_id)?);
            }
        }
        let os_lock_path = self.archive_os_lock_path(&save_id)?;
        if let Some(parent) = os_lock_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&os_lock_path)?;
        if let Err(error) = FileExt::try_lock_exclusive(&file) {
            let existing = self
                .archive_repo(&save_id)?
                .archive_lock()?
                .read()
                .ok()
                .flatten();
            return match existing {
                Some(existing) => Err(AdmError::new(format!(
                    "save archive is locked by session {} pid {}",
                    existing.session_id, existing.pid
                ))),
                None => Err(AdmError::new(format!(
                    "save archive lock is held at {}: {error}",
                    os_lock_path.display()
                ))),
            };
        }
        let lock_repo = self.archive_repo(&save_id)?.archive_lock()?;
        let lock = self.new_archive_lock_metadata(&save_id)?;
        if let Err(error) = serde_json::to_value(&lock)
            .map_err(|error| AdmError::new(format!("failed to serialize archive lock: {error}")))
            .and_then(|value| write_json_value(lock_repo.path(), &value))
        {
            let _ = FileExt::unlock(&file);
            return Err(error);
        }
        self.lock_handles()?.archives.insert(
            save_id,
            HeldFileLock {
                file,
                owner: current,
                depth: 1,
            },
        );
        Ok(lock)
    }

    fn read_archive_state(&self, save_id: &str) -> AdmResult<ProjectState> {
        let path = self
            .project_root
            .resolve_relative(&format!("saves/{save_id}/workspace/autosave_state.json"))?;
        if path.exists() {
            let text = fs::read_to_string(&path)?;
            return serde_json::from_str(&text)
                .map_err(|error| AdmError::new(format!("invalid archive state JSON: {error}")));
        }
        let execution_objects = self.project_root.resolve_relative(&format!(
            "saves/{save_id}/workspace/outputs/execution_objects/execution_objects.json"
        ))?;
        read_latest_verified_design_project(&execution_objects)?.ok_or_else(|| {
            AdmError::new(format!(
                "archive workspace state is missing and no verified design_project exists: {}",
                path.display()
            ))
        })
    }

    fn write_archive_state(&self, save_id: &str, state: &ProjectState) -> AdmResult<()> {
        let workspace = self.archive_workspace_dir(save_id)?;
        fs::create_dir_all(&workspace)?;
        let text = serde_json::to_string_pretty(state).map_err(|error| {
            AdmError::new(format!("failed to serialize archive state: {error}"))
        })?;
        adm_new_foundation::write_text_atomic(
            &workspace.join("autosave_state.json"),
            &(text + "\n"),
        )
    }

    fn archive_workspace_dir(&self, save_id: &str) -> AdmResult<PathBuf> {
        self.project_root
            .resolve_relative(&format!("saves/{save_id}/workspace"))
    }

    fn draft_root(&self) -> AdmResult<PathBuf> {
        self.project_root
            .resolve_relative(&format!("drafts/{}", self.session_id))
    }

    fn archive_repo(&self, save_id: &str) -> AdmResult<SaveArchiveRepository> {
        SaveArchiveRepository::new(&self.project_root, save_id)
    }

    fn draft_repo(&self) -> AdmResult<DraftWorkspaceRepository> {
        DraftWorkspaceRepository::new(&self.project_root, &self.session_id)
    }

    fn lock_handles(&self) -> AdmResult<MutexGuard<'_, HeldSaveLocks>> {
        self.held_locks
            .lock()
            .map_err(|_| AdmError::new("save lock handle state is poisoned"))
    }

    fn enter_operation(&self) -> AdmResult<SaveOperationGuard> {
        let current = std::thread::current().id();
        let (state, wake) = &*self.operation_gate;
        let mut state = state
            .lock()
            .map_err(|_| AdmError::new("save operation gate is poisoned"))?;
        loop {
            match state.owner {
                None => {
                    state.owner = Some(current);
                    state.depth = 1;
                    break;
                }
                Some(owner) if owner == current => {
                    state.depth += 1;
                    break;
                }
                Some(_) => {
                    state = wake
                        .wait(state)
                        .map_err(|_| AdmError::new("save operation gate is poisoned"))?;
                }
            }
        }
        drop(state);
        Ok(SaveOperationGuard {
            gate: Arc::clone(&self.operation_gate),
        })
    }

    fn operation_owned_by_current_thread(&self) -> bool {
        let Ok(state) = self.operation_gate.0.lock() else {
            return false;
        };
        state.owner == Some(std::thread::current().id()) && state.depth > 0
    }

    fn archive_os_lock_path(&self, save_id: &str) -> AdmResult<PathBuf> {
        self.project_root
            .resolve_relative(&format!("saves/.locks/archive_{save_id}.lock"))
    }

    fn index_os_lock_path(&self) -> AdmResult<PathBuf> {
        self.project_root
            .resolve_relative("saves/.locks/index.lock")
    }

    fn index_lock_metadata_path(&self) -> AdmResult<PathBuf> {
        self.project_root
            .resolve_relative(&format!("saves/{INDEX_LOCK_NAME}"))
    }

    fn transaction_os_lock_path(&self) -> AdmResult<PathBuf> {
        self.project_root
            .resolve_relative(&format!("{TRANSACTION_DIR}/{}.lock", self.session_id))
    }

    fn begin_transaction(&self) -> AdmResult<SaveTransactionGuard> {
        self.begin_transaction_with_before_global_hook(|| {})
    }

    fn begin_transaction_with_before_global_hook(
        &self,
        before_global_lock: impl FnOnce(),
    ) -> AdmResult<SaveTransactionGuard> {
        let current = std::thread::current().id();
        {
            let mut handles = self.lock_handles()?;
            if let Some(session_owner) = handles.transaction.as_ref().map(|held| held.owner) {
                if session_owner != current {
                    return Err(AdmError::new(
                        "save transaction is already active on another thread",
                    ));
                }
                let Some(global_owner) = handles.global_transaction.as_ref().map(|held| held.owner)
                else {
                    return Err(AdmError::new("save global transaction guard is missing"));
                };
                if global_owner != current {
                    return Err(AdmError::new(
                        "save global transaction is active on another thread",
                    ));
                }
                if let Some(held) = &mut handles.transaction {
                    held.depth += 1;
                }
                if let Some(held) = &mut handles.global_transaction {
                    held.depth += 1;
                }
                return Ok(SaveTransactionGuard {
                    held_locks: Arc::clone(&self.held_locks),
                    owner: current,
                });
            }
        }
        before_global_lock();
        let global_path = global_transaction_lock_path(&self.project_root)?;
        if let Some(parent) = global_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let global_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&global_path)?;
        acquire_bounded_transaction_lock(&global_file, "global transaction")?;
        if let Err(error) = recover_pending_transactions_with_global_lock(&self.project_root) {
            let _ = FileExt::unlock(&global_file);
            return Err(error);
        }
        let session_path = self.transaction_os_lock_path()?;
        if let Some(parent) = session_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let session_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&session_path)?;
        if let Err(error) = acquire_bounded_transaction_lock(&session_file, "session transaction") {
            let _ = FileExt::unlock(&global_file);
            return Err(error.into());
        }
        let mut handles = self.lock_handles()?;
        handles.transaction = Some(HeldFileLock {
            file: session_file,
            owner: current,
            depth: 1,
        });
        handles.global_transaction = Some(HeldFileLock {
            file: global_file,
            owner: current,
            depth: 1,
        });
        Ok(SaveTransactionGuard {
            held_locks: Arc::clone(&self.held_locks),
            owner: current,
        })
    }

    fn new_archive_lock_metadata(&self, save_id: &str) -> AdmResult<ArchiveLock> {
        let os_lock_path = self.archive_os_lock_path(save_id)?;
        Ok(ArchiveLock {
            pid: self.pid,
            session_id: self.session_id.clone(),
            acquired_at: timestamp(),
            live: Some(true),
            lock_path: Some(relative_to_root(self.project_root.path(), &os_lock_path)),
        })
    }

    fn retry_archive_lock_release_warning(&self, save_id: &str) -> Option<String> {
        retry_cleanup_warning("archive lock release", || {
            self.release_archive_lock(save_id)
        })
    }

    fn retry_index_lock_release_warning(&self) -> Option<String> {
        retry_cleanup_warning("index lock release", || self.release_index_lock())
    }

    fn persist_report_warnings(&self, context: &str, warnings: &mut Vec<String>) {
        if warnings.is_empty() {
            return;
        }
        if let Err(error) = persist_cleanup_warnings(&self.project_root, context, warnings) {
            warnings.push(format!("cleanup warning log persistence failed: {error}"));
        }
    }

    fn observe_archive_lock(&self, save_id: &str) -> AdmResult<(Option<ArchiveLock>, bool)> {
        if self.lock_handles()?.archives.contains_key(save_id) {
            let metadata = self.archive_repo(save_id)?.archive_lock()?.read()?;
            return Ok((metadata, true));
        }
        let os_lock_path = self.archive_os_lock_path(save_id)?;
        let metadata = self.archive_repo(save_id)?.archive_lock()?.read()?;
        if !os_lock_path.is_file() {
            return Ok((metadata, false));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&os_lock_path)?;
        match FileExt::try_lock_exclusive(&file) {
            Ok(()) => {
                FileExt::unlock(&file)?;
                Ok((metadata, false))
            }
            Err(_) => Ok((metadata, true)),
        }
    }

    fn load_index(&self) -> AdmResult<SaveIndex> {
        self.with_index_lock(|| self.load_index_unlocked())
    }

    fn load_index_unlocked(&self) -> AdmResult<SaveIndex> {
        let repository = SaveIndexRepository::new(&self.project_root)?;
        match repository.read() {
            Ok(Some(index)) => Ok(index),
            Ok(None) => self.rebuild_index_from_manifests(),
            Err(_) => {
                quarantine_corrupt_index(repository.path())?;
                let index = self.rebuild_index_from_manifests()?;
                repository.write(&index)?;
                Ok(index)
            }
        }
    }

    fn write_index_unlocked(&self, index: SaveIndex) -> AdmResult<SaveIndex> {
        SaveIndexRepository::new(&self.project_root)?.write(&index)?;
        Ok(index)
    }

    fn update_index(
        &self,
        update: impl FnOnce(&mut SaveIndex) -> AdmResult<()>,
    ) -> AdmResult<SaveIndex> {
        self.with_index_lock(|| {
            let mut index = self.load_index_unlocked()?;
            update(&mut index)?;
            self.write_index_unlocked(index)
        })
    }

    fn with_index_lock<T>(&self, operation: impl FnOnce() -> AdmResult<T>) -> AdmResult<T> {
        self.acquire_index_lock()?;
        let result = operation();
        let release_warning = self.retry_index_lock_release_warning();
        match (result, release_warning) {
            (Ok(value), None) => Ok(value),
            (Err(error), None) => Err(error),
            (Ok(value), Some(warning)) => {
                let mut warnings = vec![warning];
                self.persist_report_warnings("index_operation", &mut warnings);
                Ok(value)
            }
            (Err(error), Some(release_warning)) => Err(AdmError::new(format!(
                "index operation failed: {error}; {release_warning}"
            ))),
        }
    }

    fn acquire_index_lock(&self) -> AdmResult<()> {
        let current = std::thread::current().id();
        {
            let mut handles = self.lock_handles()?;
            if let Some(held) = &mut handles.index {
                if held.owner != current {
                    return Err(AdmError::new(
                        "save index is already in use by another operation in this process",
                    ));
                }
                held.depth += 1;
                return Ok(());
            }
        }
        let os_lock_path = self.index_os_lock_path()?;
        if let Some(parent) = os_lock_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&os_lock_path)?;
        if let Err(error) = FileExt::try_lock_exclusive(&file) {
            let metadata_path = self.index_lock_metadata_path()?;
            let existing = read_archive_lock_file(&metadata_path).ok().flatten();
            return match existing {
                Some(existing) => Err(AdmError::new(format!(
                    "save index is locked by session {} pid {}",
                    existing.session_id, existing.pid
                ))),
                None => Err(AdmError::new(format!(
                    "save index lock is held at {}: {error}",
                    os_lock_path.display()
                ))),
            };
        }
        let lock = ArchiveLock {
            pid: self.pid,
            session_id: self.session_id.clone(),
            acquired_at: timestamp(),
            live: Some(true),
            lock_path: Some(relative_to_root(self.project_root.path(), &os_lock_path)),
        };
        let metadata_path = self.index_lock_metadata_path()?;
        if let Err(error) = write_json_value(
            &metadata_path,
            &serde_json::to_value(&lock).map_err(|error| {
                AdmError::new(format!("failed to serialize save index lock: {error}"))
            })?,
        ) {
            let _ = FileExt::unlock(&file);
            return Err(error);
        }
        self.lock_handles()?.index = Some(HeldFileLock {
            file,
            owner: current,
            depth: 1,
        });
        Ok(())
    }

    fn release_index_lock(&self) -> AdmResult<()> {
        let current = std::thread::current().id();
        let held = {
            let mut handles = self.lock_handles()?;
            let Some(held) = &mut handles.index else {
                return Ok(());
            };
            if held.owner != current {
                return Err(AdmError::new(
                    "save index lock release attempted by a concurrent operation",
                ));
            }
            if held.depth > 1 {
                held.depth -= 1;
                return Ok(());
            }
            handles.index.take()
        };
        let Some(held) = held else {
            return Ok(());
        };
        let released = ArchiveLock {
            pid: self.pid,
            session_id: self.session_id.clone(),
            acquired_at: timestamp(),
            live: Some(false),
            lock_path: Some(relative_to_root(
                self.project_root.path(),
                &self.index_os_lock_path()?,
            )),
        };
        let metadata = serde_json::to_value(&released).map_err(|error| {
            AdmError::new(format!("failed to serialize save index lock: {error}"))
        });
        let metadata_result = match metadata {
            Ok(metadata) => write_json_value(&self.index_lock_metadata_path()?, &metadata),
            Err(error) => Err(error),
        };
        let unlock_result = FileExt::unlock(&held.file).map_err(AdmError::from);
        match (metadata_result, unlock_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Err(metadata_error), Err(unlock_error)) => Err(AdmError::new(format!(
                "index lock metadata update failed: {metadata_error}; unlock failed: {unlock_error}"
            ))),
        }
    }

    fn rebuild_index_from_manifests(&self) -> AdmResult<SaveIndex> {
        let saves_root = self.project_root.saves_dir();
        let mut entries = Vec::new();
        if saves_root.is_dir() {
            for entry in fs::read_dir(&saves_root)? {
                let path = entry?.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(save_id) = path.file_name().and_then(|value| value.to_str()) else {
                    continue;
                };
                let Ok(save_id) = safe_component("save_id", save_id) else {
                    continue;
                };
                match self.read_archive_manifest(&save_id) {
                    Ok(manifest) if manifest.save_id == save_id => {
                        entries.push(index_entry_from_manifest(&manifest));
                    }
                    Ok(manifest) => entries.push(corrupt_index_entry(
                        &save_id,
                        &format!(
                            "manifest save_id {} does not match archive directory",
                            manifest.save_id
                        ),
                    )),
                    Err(error) => entries.push(corrupt_index_entry(&save_id, &error.to_string())),
                }
            }
        }
        entries.sort_by(|left, right| {
            right
                .last_worked_at
                .cmp(&left.last_worked_at)
                .then_with(|| left.display_name.cmp(&right.display_name))
        });
        let linked_save_id = self.current_draft_save_id()?;
        let current_save_id =
            linked_save_id.filter(|save_id| entries.iter().any(|entry| &entry.save_id == save_id));
        Ok(SaveIndex {
            current_save_id,
            saves: entries,
            updated_at: timestamp(),
            ..SaveIndex::default()
        })
    }

    fn new_draft_meta(
        &self,
        linked_save_id: Option<String>,
        workspace_state: WorkspaceState,
        linked_archive_path: Option<String>,
    ) -> AdmResult<DraftMeta> {
        let draft_root = self
            .project_root
            .resolve_relative(&format!("drafts/{}", self.session_id))?;
        Ok(DraftMeta {
            schema_version: SAVE_SCHEMA_VERSION,
            session_id: self.session_id.clone(),
            pid: self.pid,
            project_root: self.project_root.path().display().to_string(),
            draft_root: draft_root.display().to_string(),
            updated_at: timestamp(),
            linked_save_id,
            linked_archive_path: linked_archive_path.unwrap_or_default(),
            workspace_state,
            origin_deleted_save_id: None,
        })
    }

    pub fn current_draft_save_id(&self) -> AdmResult<Option<String>> {
        let _operation = self.enter_operation()?;
        Ok(self
            .current_draft_binding()?
            .flatten()
            .filter(|save_id| !save_id.trim().is_empty()))
    }

    fn current_draft_binding(&self) -> AdmResult<Option<Option<String>>> {
        Ok(self
            .draft_repo()?
            .draft_meta()?
            .read()?
            .map(|meta| meta.linked_save_id))
    }

    fn current_or_index_save_id(&self) -> AdmResult<Option<String>> {
        let save_id = match self.current_draft_binding()? {
            Some(linked_save_id) => linked_save_id,
            None => self.load_index()?.current_save_id,
        };
        Ok(save_id.filter(|save_id| !save_id.trim().is_empty()))
    }

    fn read_archive_manifest(&self, save_id: &str) -> AdmResult<SaveManifest> {
        let manifest_path = self
            .project_root
            .resolve_relative(&format!("saves/{save_id}/{MANIFEST_NAME}"))?;
        let legacy_path = self
            .project_root
            .resolve_relative(&format!("saves/{save_id}/{LEGACY_MANIFEST_NAME}"))?;
        let path = if manifest_path.exists() {
            manifest_path.clone()
        } else if legacy_path.exists() {
            legacy_path
        } else {
            return Err(AdmError::new(format!(
                "missing save manifest for {save_id}"
            )));
        };
        let text = fs::read_to_string(&path)?;
        let mut manifest = serde_json::from_str::<SaveManifest>(&text).map_err(|error| {
            AdmError::new(format!(
                "invalid save manifest at {}: {error}",
                path.display()
            ))
        })?;
        normalize_save_progress(&mut manifest.progress);
        if path.file_name().and_then(|value| value.to_str()) == Some(LEGACY_MANIFEST_NAME) {
            self.archive_repo(save_id)?.manifest()?.write(&manifest)?;
        }
        Ok(manifest)
    }

    fn cleanup_failed_new_save(&self, save_id: &str) -> AdmResult<()> {
        let cleanup = (|| {
            let save_dir = self
                .project_root
                .resolve_relative(&format!("saves/{save_id}"))?;
            remove_path_if_exists(&save_dir)?;
            self.update_index(|index| {
                index.saves.retain(|entry| entry.save_id != save_id);
                if index.current_save_id.as_deref() == Some(save_id) {
                    index.current_save_id = None;
                }
                index.updated_at = timestamp();
                Ok(())
            })?;
            Ok(())
        })();
        let release = self.release_archive_lock(save_id);
        match (cleanup, release) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(release_error)) => Err(AdmError::new(format!(
                "failed save cleanup: {error}; archive lock release failed: {release_error}"
            ))),
        }
    }

    fn create_metadata_transaction_journal(
        &self,
        role: &str,
        before_image: TransactionBeforeImage,
    ) -> AdmResult<SelectedPathsSwap> {
        let draft_root = self.draft_root()?;
        fs::create_dir_all(&draft_root)?;
        let staging = unique_sibling_path(&draft_root, "metadata-staging")?;
        fs::create_dir_all(&staging)?;
        match swap_staged_selected_paths_journaled(
            &self.project_root,
            &self.session_id,
            role,
            &staging,
            &draft_root,
            &[],
            &[],
            before_image,
        ) {
            Ok(transaction) => Ok(transaction),
            Err(error) => {
                let _ = remove_path_if_exists(&staging);
                Err(error)
            }
        }
    }

    fn prepare_snapshot(
        &self,
        save_id: &str,
        manifest: &SnapshotManifest,
        file_map: &FileMap,
        delta: &FileMapDelta,
        workspace: &Path,
    ) -> AdmResult<(PathBuf, PathBuf)> {
        let snapshots_root = self.draft_root()?.join("snapshots");
        fs::create_dir_all(&snapshots_root)?;
        let snapshot_name = format!("{save_id}_tx_{}", manifest.seq);
        let snapshot_dir = snapshots_root.join(snapshot_name);
        let staging = unique_sibling_path(&snapshot_dir, "staging")?;
        fs::create_dir_all(&staging)?;
        let result: AdmResult<(PathBuf, PathBuf)> = (|| {
            write_json_value(
                &staging.join("snapshot_manifest.json"),
                &serde_json::to_value(manifest).map_err(|error| {
                    AdmError::new(format!("failed to serialize snapshot manifest: {error}"))
                })?,
            )?;
            write_json_value(
                &staging.join("snapshot_file_map.json"),
                &serde_json::to_value(file_map).map_err(|error| {
                    AdmError::new(format!("failed to serialize snapshot file map: {error}"))
                })?,
            )?;
            let delta_dir = staging.join("delta");
            write_json_value(
                &delta_dir.join("added.json"),
                &serde_json::to_value(&delta.added).map_err(|error| {
                    AdmError::new(format!("failed to serialize added file delta: {error}"))
                })?,
            )?;
            write_json_value(
                &delta_dir.join("modified.json"),
                &serde_json::to_value(&delta.modified).map_err(|error| {
                    AdmError::new(format!("failed to serialize modified file delta: {error}"))
                })?,
            )?;
            write_json_value(
                &delta_dir.join("removed.json"),
                &serde_json::to_value(&delta.removed).map_err(|error| {
                    AdmError::new(format!("failed to serialize removed file delta: {error}"))
                })?,
            )?;
            copy_path_without_symlinks(workspace, &staging.join("full"))?;
            Ok((staging.clone(), snapshot_dir))
        })();
        if result.is_err() {
            let _ = remove_path_if_exists(&staging);
        }
        result
    }

    fn prune_snapshots(&self, save_id: &str, keep: usize) -> AdmResult<()> {
        let snapshots_root = self.draft_root()?.join("snapshots");
        if !snapshots_root.is_dir() {
            return Ok(());
        }
        let prefix = format!("{save_id}_tx_");
        let mut snapshots = Vec::new();
        for entry in fs::read_dir(&snapshots_root)? {
            let path = entry?.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            let Some(seq) = name
                .strip_prefix(&prefix)
                .and_then(|value| value.parse::<u64>().ok())
            else {
                continue;
            };
            snapshots.push((seq, path));
        }
        snapshots.sort_by(|left, right| right.0.cmp(&left.0));
        for (_, path) in snapshots.into_iter().skip(keep) {
            remove_path_if_exists(&path)?;
        }
        Ok(())
    }

    fn enrich_save_index(&self, index: &mut SaveIndex) {
        let draft = self.draft_repo().ok();
        let meta = draft
            .as_ref()
            .and_then(|repository| repository.draft_meta().ok())
            .and_then(|repository| repository.read().ok().flatten());
        if let Some(meta) = meta {
            index.current_save_id = meta
                .linked_save_id
                .filter(|save_id| !save_id.trim().is_empty());
            index.workspace_state = meta.workspace_state.as_str().to_string();
            index.draft_updated_at = meta.updated_at;
            index.origin_deleted_save_id = meta.origin_deleted_save_id;
        } else {
            index.workspace_state.clear();
            index.draft_updated_at.clear();
            index.origin_deleted_save_id = None;
        }
        index.has_autosave = draft
            .and_then(|repository| repository.autosave_state().ok())
            .is_some_and(|repository| repository.path().is_file());
        for entry in &mut index.saves {
            self.enrich_save_entry(entry);
        }
    }

    fn enrich_save_entry(&self, entry: &mut SaveIndexEntry) {
        entry.locked_by_other = false;
        entry.lock_owner_pid = None;
        entry.lock_owner_session.clear();
        let Ok(save_id) = safe_component("save_id", &entry.save_id) else {
            entry.integrity_status = "error".to_string();
            entry.integrity_message = "save id is not portable".to_string();
            return;
        };
        let mut warnings = Vec::new();
        let mut errors = Vec::new();
        let mut corrupt_manifest = false;
        match self.read_archive_manifest(&save_id) {
            Ok(manifest) => {
                entry.display_name = manifest.display_name.clone();
                entry.save_type = manifest.save_type.clone();
                entry.created_by = manifest.created_by.clone();
                entry.reason = manifest.reason.clone();
                entry.created_at = manifest.created_at.clone();
                entry.last_worked_at = manifest.last_worked_at.clone();
                entry.progress = manifest.progress.clone();
                entry.last_transaction_seq = manifest.last_transaction_seq;
            }
            Err(error) => {
                corrupt_manifest = true;
                errors.push(error.to_string());
            }
        }
        let workspace = match self.archive_workspace_dir(&save_id) {
            Ok(workspace) => workspace,
            Err(error) => {
                errors.push(error.to_string());
                entry.integrity_status = "error".to_string();
                entry.integrity_message = errors.join("; ");
                return;
            }
        };
        match workspace_stats(&workspace) {
            Ok((count, bytes)) => {
                entry.workspace_file_count = count;
                entry.workspace_bytes = bytes;
            }
            Err(error) => errors.push(error.to_string()),
        }
        let has_autosave = workspace.join("autosave_state.json").is_file();
        match self.read_archive_state(&save_id) {
            Ok(_) if !has_autosave => warnings.push(
                "autosave_state.json is missing; verified design_project fallback is available"
                    .to_string(),
            ),
            Ok(_) => {}
            Err(error) => errors.push(error.to_string()),
        }
        match self.observe_archive_lock(&save_id) {
            Ok((Some(lock), locked)) => {
                entry.lock_owner_pid = Some(lock.pid);
                entry.lock_owner_session = lock.session_id.clone();
                entry.locked_by_other =
                    locked && !(lock.pid == self.pid && lock.session_id == self.session_id);
            }
            Ok((None, _)) => {}
            Err(error) => errors.push(format!("invalid archive lock: {error}")),
        }
        if !errors.is_empty() {
            entry.integrity_status = if corrupt_manifest { "corrupt" } else { "error" }.to_string();
            entry.integrity_message = errors
                .into_iter()
                .chain(warnings)
                .collect::<Vec<_>>()
                .join("; ");
        } else if !warnings.is_empty() {
            entry.integrity_status = "warning".to_string();
            entry.integrity_message = warnings.join("; ");
        } else {
            entry.integrity_status = "ok".to_string();
            entry.integrity_message.clear();
        }
    }

    fn audit_drafts(
        &self,
        issues: &mut Vec<ParallelIsolationIssue>,
    ) -> AdmResult<Vec<DraftIsolationSummary>> {
        let mut drafts = Vec::new();
        let drafts_root = self.project_root.drafts_dir();
        if !drafts_root.is_dir() {
            return Ok(drafts);
        }
        for entry in fs::read_dir(&drafts_root)? {
            let draft = entry?.path();
            if !draft.is_dir() {
                continue;
            }
            let meta_path = draft.join("draft_meta.json");
            let context_path = draft.join("runtime").join("run_context.json");
            let meta = read_json_value(&meta_path)?.unwrap_or(Value::Null);
            let context = read_json_value(&context_path)?.unwrap_or(Value::Null);
            let linked_save = linked_save_id_from_meta_value(&meta);
            let context_save = json_string(&context, "save_id").unwrap_or_default();
            let run_id = json_string(&context, "run_id").unwrap_or_default();
            drafts.push(DraftIsolationSummary {
                draft: draft_name(&draft),
                linked_save_id: linked_save.clone(),
                run_context_save_id: context_save.clone(),
                run_id,
            });
            if !linked_save.is_empty() && !context_save.is_empty() && linked_save != context_save {
                push_issue(
                    self.project_root.path(),
                    issues,
                    "DRAFT_LINKED_SAVE_MISMATCH",
                    "P0",
                    &meta_path,
                    "draft_meta linked_save_id differs from run_context save_id.",
                    &linked_save,
                    &context_save,
                    &linked_save,
                    &context_save,
                );
            }
            if context.is_object() {
                if let Some(snapshot) = json_string(&context, "settings_snapshot") {
                    if !PathBuf::from(&snapshot).is_file() {
                        push_issue(
                            self.project_root.path(),
                            issues,
                            "RUN_SETTINGS_SNAPSHOT_MISSING",
                            "P0",
                            &context_path,
                            "run_context points to a missing project_settings snapshot.",
                            "",
                            &snapshot,
                            "",
                            &context_save,
                        );
                    }
                }
                if let Some(source_root) = json_string(&context, "source_artifacts_root") {
                    let expected = draft.join("source_artifacts");
                    let matches_expected =
                        normalize_path(&PathBuf::from(&source_root)) == normalize_path(&expected);
                    if !matches_expected {
                        push_issue(
                            self.project_root.path(),
                            issues,
                            "SOURCE_ARTIFACT_ROOT_MISMATCH",
                            "P0",
                            &context_path,
                            "run_context source_artifacts_root does not point to this draft.",
                            &expected.display().to_string(),
                            &source_root,
                            "",
                            &context_save,
                        );
                    }
                }
            }
        }
        drafts.sort_by(|left, right| left.draft.cmp(&right.draft));
        Ok(drafts)
    }

    fn audit_saves(
        &self,
        issues: &mut Vec<ParallelIsolationIssue>,
    ) -> AdmResult<Vec<SaveIsolationSummary>> {
        let mut saves = Vec::new();
        let saves_root = self.project_root.saves_dir();
        if !saves_root.is_dir() {
            return Ok(saves);
        }
        for entry in fs::read_dir(&saves_root)? {
            let save = entry?.path();
            if !save.is_dir() {
                continue;
            }
            let fallback_id = draft_name(&save);
            let manifest = read_json_value(&save.join(MANIFEST_NAME))?
                .or(read_json_value(&save.join(LEGACY_MANIFEST_NAME))?)
                .unwrap_or(Value::Null);
            let save_id = json_string(&manifest, "save_id").unwrap_or(fallback_id);
            saves.push(SaveIsolationSummary {
                save_id: save_id.clone(),
                path: relative_to_root(self.project_root.path(), &save),
            });
            let workspace = save.join("workspace");
            let eo_path = workspace
                .join("outputs")
                .join("execution_objects")
                .join("execution_objects.json");
            let eo_store = read_json_value(&eo_path)?.unwrap_or(Value::Null);
            if let Some(actual) = json_string(&eo_store, "save_id") {
                if !actual.is_empty() && actual != save_id {
                    push_issue(
                        self.project_root.path(),
                        issues,
                        "EXECUTION_OBJECT_STORE_SAVE_MISMATCH",
                        "P0",
                        &eo_path,
                        "Execution object store save_id differs from containing save.",
                        &save_id,
                        &actual,
                        "",
                        "",
                    );
                }
            }
            for relative in [
                "outputs/artifacts/stage_13/scene_assembly_report.json",
                "outputs/artifacts/stage_13/changed_files_manifest.json",
                "outputs/artifacts/stage_14/integration_validation_report.json",
            ] {
                let artifact_path = workspace.join(relative);
                let artifact = read_json_value(&artifact_path)?.unwrap_or(Value::Null);
                if !artifact.is_object() {
                    continue;
                }
                if let Some(actual) = json_string(&artifact, "save_id") {
                    if !actual.is_empty() && actual != save_id {
                        push_issue(
                            self.project_root.path(),
                            issues,
                            "ARTIFACT_SAVE_ID_MISMATCH",
                            "P0",
                            &artifact_path,
                            "Artifact save_id differs from containing save.",
                            &save_id,
                            &actual,
                            "",
                            "",
                        );
                    }
                }
                if relative.ends_with("scene_assembly_report.json") {
                    let development_path = json_string(&artifact, "development_path")
                        .or_else(|| json_string(&artifact, "project_path"))
                        .unwrap_or_default();
                    if development_path.is_empty() {
                        push_issue(
                            self.project_root.path(),
                            issues,
                            "STAGE13_DEVELOPMENT_PATH_MISSING",
                            "P1",
                            &artifact_path,
                            "Step13 report has no bound development_path.",
                            "",
                            "",
                            "",
                            "",
                        );
                    }
                }
            }
        }
        saves.sort_by(|left, right| left.save_id.cmp(&right.save_id));
        Ok(saves)
    }

    fn append_timeline(&self, entry: TimelineEntry) -> AdmResult<()> {
        let path = self
            .project_root
            .resolve_relative(&format!("drafts/{}/timeline.jsonl", self.session_id))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(&entry)
            .map_err(|error| AdmError::new(format!("failed to serialize timeline: {error}")))?;
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        file.write_all(json.as_bytes())?;
        file.write_all(b"\n")?;
        Ok(())
    }
}

fn timestamp() -> String {
    format!("unix:{}", unix_timestamp())
}

fn retry_cleanup_warning(
    label: &str,
    mut operation: impl FnMut() -> AdmResult<()>,
) -> Option<String> {
    let mut last_error = None;
    for attempt in 0..CLEANUP_RETRY_LIMIT {
        match operation() {
            Ok(()) => {
                return last_error
                    .map(|error| format!("{label} recovered after an earlier failure: {error}"));
            }
            Err(error) => last_error = Some(error),
        }
        if attempt + 1 < CLEANUP_RETRY_LIMIT {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    last_error.map(|error| format!("{label} failed after {CLEANUP_RETRY_LIMIT} attempts: {error}"))
}

fn cleanup_result_message(result: AdmResult<()>) -> String {
    result
        .err()
        .map(|error| error.to_string())
        .unwrap_or_else(|| "ok".to_string())
}

fn safe_component(name: &str, value: &str) -> AdmResult<String> {
    let clean = sanitize_identifier(value)?;
    if clean == value {
        Ok(clean)
    } else {
        Err(AdmError::new(format!(
            "{name} contains non-portable path characters: {value}"
        )))
    }
}

fn blank_progress() -> SaveProgress {
    SaveProgress {
        passed: 0,
        total: 0,
        label: "0/0".to_string(),
        design_passed: 0,
        design_total: 0,
        design_label: "0/0".to_string(),
        pipeline_passed: 0,
        pipeline_total: 15,
        pipeline_label: "0/15".to_string(),
    }
}

fn empty_file_map() -> FileMap {
    FileMap {
        schema_version: SAVE_SCHEMA_VERSION,
        generated_at: String::new(),
        transaction_seq: None,
        files: Vec::new(),
    }
}

fn read_json_value(path: &Path) -> AdmResult<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    let value = serde_json::from_str(&text)
        .map_err(|error| AdmError::new(format!("invalid JSON at {}: {error}", path.display())))?;
    Ok(Some(value))
}

fn read_latest_verified_design_project(path: &Path) -> AdmResult<Option<ProjectState>> {
    let Some(document) = read_json_value(path)? else {
        return Ok(None);
    };
    let mut candidates = document
        .get("objects")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|object| {
            object.get("object_type").and_then(Value::as_str) == Some("design_project")
                && object.get("state").and_then(Value::as_str) == Some("verified")
                && object.get("user_content").is_some_and(Value::is_object)
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        let left_updated = left
            .get("updated_at")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let right_updated = right
            .get("updated_at")
            .and_then(Value::as_str)
            .unwrap_or_default();
        right_updated.cmp(left_updated)
    });
    let Some(content) = candidates
        .first()
        .and_then(|object| object.get("user_content"))
    else {
        return Ok(None);
    };
    serde_json::from_value(content.clone())
        .map(Some)
        .map_err(|error| {
            AdmError::new(format!(
                "invalid verified design_project user_content at {}: {error}",
                path.display()
            ))
        })
}

fn read_archive_lock_file(path: &Path) -> AdmResult<Option<ArchiveLock>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    serde_json::from_str(&text).map(Some).map_err(|error| {
        AdmError::new(format!(
            "invalid archive lock at {}: {error}",
            path.display()
        ))
    })
}

fn quarantine_corrupt_index(path: &Path) -> AdmResult<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let parent = path
        .parent()
        .ok_or_else(|| AdmError::new(format!("save index has no parent: {}", path.display())))?;
    let quarantine = parent.join(format!(
        "save_index.corrupt.{}.json",
        new_stable_id("index")?
    ));
    fs::rename(path, &quarantine)?;
    Ok(Some(quarantine))
}

fn write_json_value(path: &Path, value: &Value) -> AdmResult<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| AdmError::new(format!("failed to serialize JSON: {error}")))?;
    adm_new_foundation::write_text_atomic(path, &(text + "\n"))
}

fn append_jsonl_value(path: &Path, value: &Value) -> AdmResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(value)
        .map_err(|error| AdmError::new(format!("failed to serialize JSONL: {error}")))?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(json.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn restore_append_only_file(path: &Path, previous_len: Option<u64>) -> AdmResult<()> {
    match previous_len {
        Some(len) => {
            let file = OpenOptions::new().write(true).open(path)?;
            file.set_len(len)?;
            Ok(())
        }
        None => remove_file_if_exists(path),
    }
}

fn restore_optional_draft_meta(
    draft: &DraftWorkspaceRepository,
    previous: Option<DraftMeta>,
) -> AdmResult<()> {
    let repository = draft.draft_meta()?;
    match previous {
        Some(value) => {
            repository.write(&value)?;
            Ok(())
        }
        None => remove_file_if_exists(repository.path()),
    }
}

fn restore_optional_file_map(
    draft: &DraftWorkspaceRepository,
    previous: Option<FileMap>,
) -> AdmResult<()> {
    let repository = draft.draft_file_map()?;
    match previous {
        Some(value) => {
            repository.write(&value)?;
            Ok(())
        }
        None => remove_file_if_exists(repository.path()),
    }
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value
        .as_object()?
        .get(key)?
        .as_str()
        .map(|value| value.to_string())
}

fn linked_save_id_from_meta_value(meta: &Value) -> String {
    if let Some(value) = json_string(meta, "linked_save_id") {
        if !value.trim().is_empty() {
            return value;
        }
    }
    let Some(path) = json_string(meta, "linked_archive_path") else {
        return String::new();
    };
    Path::new(&path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string()
}

fn repair_draft_meta_value(meta: &mut Value, save_id: &str, old_save: &str) {
    if !meta.is_object() {
        *meta = Value::Object(Default::default());
    }
    let object = meta.as_object_mut().expect("meta object just created");
    object.insert(
        "schema_version".to_string(),
        Value::from(SAVE_SCHEMA_VERSION),
    );
    object.insert(
        "linked_save_id".to_string(),
        Value::String(save_id.to_string()),
    );
    object.insert(
        "linked_archive_path".to_string(),
        Value::String(format!("saves/{save_id}")),
    );
    object.insert(
        "workspace_state".to_string(),
        Value::String("linked_save".into()),
    );
    object.insert("updated_at".to_string(), Value::String(timestamp()));
    object.insert(
        "parallel_isolation_repair".to_string(),
        serde_json::json!({
            "repaired_at": timestamp(),
            "basis": "runtime/run_context.json save_id",
            "old_linked_save_id": old_save,
        }),
    );
}

fn existing_blank_cleanup_paths(workspace: &Path) -> AdmResult<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for relative in BLANK_CLEAN_RELATIVE_PATHS {
        let path = workspace.join(relative);
        if path.exists() {
            paths.push(path);
        }
    }
    let source_root = workspace.join("source_artifacts");
    if source_root.is_dir() {
        for entry in fs::read_dir(source_root)? {
            let path = entry?.path();
            if path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.starts_with("devflow_"))
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

fn remove_generated_design_sources(source_root: &Path) -> AdmResult<()> {
    if !source_root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(source_root)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with("devflow_"))
        {
            remove_path_if_exists(&path)?;
        }
    }
    Ok(())
}

fn prepare_blank_draft(draft_root: &Path) -> AdmResult<PathBuf> {
    let staging = unique_sibling_path(draft_root, "blank-staging")?;
    let prepare = (|| {
        if draft_root.exists() {
            copy_path_without_symlinks(draft_root, &staging)?;
        } else {
            fs::create_dir_all(&staging)?;
        }
        for relative in BLANK_DRAFT_REPLACE_PATHS {
            if *relative != "source_artifacts" {
                remove_path_if_exists(&staging.join(relative))?;
            }
        }
        remove_generated_design_sources(&staging.join("source_artifacts"))?;
        ensure_empty_workspace_dirs(&staging)?;
        Ok(staging.clone())
    })();
    if prepare.is_err() {
        let _ = remove_path_if_exists(&staging);
    }
    prepare
}

fn workspace_stats(workspace: &Path) -> AdmResult<(u64, u64)> {
    if !workspace.is_dir() {
        return Err(AdmError::new(format!(
            "save workspace is missing: {}",
            workspace.display()
        )));
    }
    let mut file_count = 0_u64;
    let mut bytes = 0_u64;
    collect_workspace_stats(workspace, &mut file_count, &mut bytes)?;
    Ok((file_count, bytes))
}

fn collect_workspace_stats(path: &Path, file_count: &mut u64, bytes: &mut u64) -> AdmResult<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() {
            return Err(AdmError::new(format!(
                "save workspace contains a symbolic link: {}",
                entry.path().display()
            )));
        }
        if metadata.is_dir() {
            collect_workspace_stats(&entry.path(), file_count, bytes)?;
        } else if metadata.is_file() {
            *file_count += 1;
            *bytes += metadata.len();
        }
    }
    Ok(())
}

fn ensure_empty_workspace_dirs(root: &Path) -> AdmResult<()> {
    for relative in EMPTY_WORKSPACE_DIRS {
        fs::create_dir_all(root.join(relative))?;
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> AdmResult<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn relative_to_root(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn draft_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string()
}

fn normalize_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn push_issue(
    root: &Path,
    issues: &mut Vec<ParallelIsolationIssue>,
    code: &str,
    severity: &str,
    path: &Path,
    message: &str,
    expected: &str,
    actual: &str,
    linked_save_id: &str,
    run_context_save_id: &str,
) {
    issues.push(ParallelIsolationIssue {
        code: code.to_string(),
        severity: severity.to_string(),
        path: relative_to_root(root, path),
        message: message.to_string(),
        expected: expected.to_string(),
        actual: actual.to_string(),
        linked_save_id: linked_save_id.to_string(),
        run_context_save_id: run_context_save_id.to_string(),
    });
}

fn index_entry_from_manifest(manifest: &SaveManifest) -> SaveIndexEntry {
    SaveIndexEntry {
        save_id: manifest.save_id.clone(),
        display_name: manifest.display_name.clone(),
        save_type: manifest.save_type.clone(),
        created_by: manifest.created_by.clone(),
        reason: manifest.reason.clone(),
        path: format!("saves/{}", manifest.save_id),
        created_at: manifest.created_at.clone(),
        last_worked_at: manifest.last_worked_at.clone(),
        progress: manifest.progress.clone(),
        last_transaction_seq: manifest.last_transaction_seq,
        locked_by_other: false,
        lock_owner_pid: None,
        lock_owner_session: String::new(),
        integrity_status: String::new(),
        integrity_message: String::new(),
        workspace_file_count: 0,
        workspace_bytes: 0,
    }
}

fn corrupt_index_entry(save_id: &str, message: &str) -> SaveIndexEntry {
    SaveIndexEntry {
        save_id: save_id.to_string(),
        display_name: save_id.to_string(),
        path: format!("saves/{save_id}"),
        integrity_status: "corrupt".to_string(),
        integrity_message: message.to_string(),
        ..SaveIndexEntry::default()
    }
}

fn index_entry_from_manifest_and_file_map(
    manifest: &SaveManifest,
    file_map: &FileMap,
) -> SaveIndexEntry {
    let mut entry = index_entry_from_manifest(manifest);
    entry.integrity_status = "ok".to_string();
    entry.workspace_file_count = file_map.files.len() as u64;
    entry.workspace_bytes = file_map.files.iter().map(|file| file.size_bytes).sum();
    entry
}

fn upsert_index_entry(index: &mut SaveIndex, entry: SaveIndexEntry) {
    if let Some(existing) = index
        .saves
        .iter_mut()
        .find(|existing| existing.save_id == entry.save_id)
    {
        let previous_file_count = existing.workspace_file_count;
        let previous_bytes = existing.workspace_bytes;
        let previous_integrity_status = existing.integrity_status.clone();
        let previous_integrity_message = existing.integrity_message.clone();
        *existing = entry;
        if existing.workspace_file_count == 0 {
            existing.workspace_file_count = previous_file_count;
            existing.workspace_bytes = previous_bytes;
        }
        if existing.integrity_status.is_empty() {
            existing.integrity_status = previous_integrity_status;
            existing.integrity_message = previous_integrity_message;
        }
    } else {
        index.saves.push(entry);
    }
}

fn progress_from_state(state: &ProjectState, workspace: Option<&Path>) -> SaveProgress {
    let total = state
        .nodes
        .values()
        .map(|node| node.checklist.len() as u32)
        .sum();
    let passed = state
        .nodes
        .values()
        .flat_map(|node| node.checklist.values())
        .filter(|value| **value)
        .count() as u32;
    let pipeline_passed = workspace.map(pipeline_passed_from_reports).unwrap_or(0);
    SaveProgress {
        passed,
        total,
        label: progress_label(passed, total),
        design_passed: passed,
        design_total: total,
        design_label: progress_label(passed, total),
        pipeline_passed,
        pipeline_total: 15,
        pipeline_label: format!("{pipeline_passed}/15"),
    }
}

fn progress_label(passed: u32, total: u32) -> String {
    if total == 0 {
        "0/0".to_string()
    } else {
        format!("{passed}/{total}")
    }
}

fn normalize_save_progress(progress: &mut SaveProgress) {
    if progress.design_total == 0 && progress.total > 0 {
        progress.design_passed = progress.passed;
        progress.design_total = progress.total;
        progress.design_label = if progress.label.trim().is_empty() {
            progress_label(progress.passed, progress.total)
        } else {
            progress.label.clone()
        };
    }
    if progress.design_label.trim().is_empty() {
        progress.design_label = progress_label(progress.design_passed, progress.design_total);
    }
    if progress.pipeline_total == 0 {
        progress.pipeline_total = 15;
    }
    if progress.pipeline_label.trim().is_empty() {
        progress.pipeline_label =
            format!("{}/{}", progress.pipeline_passed, progress.pipeline_total);
    }
}

fn pipeline_passed_from_reports(workspace: &Path) -> u32 {
    (0..15)
        .filter(|stage| {
            let path = workspace
                .join("outputs")
                .join("artifacts")
                .join(format!("stage_{stage:02}"))
                .join("validation_report.json");
            let Ok(Some(report)) = read_json_value(&path) else {
                return false;
            };
            validation_report_passed(&report)
        })
        .count() as u32
}

fn validation_report_passed(report: &Value) -> bool {
    let status = report
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| {
            report
                .get("business_quality")
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
        })
        .unwrap_or_default();
    matches!(
        status,
        "success" | "passed" | "completed_with_review" | "skipped"
    ) || (status.is_empty() && report.get("valid").and_then(Value::as_bool) == Some(true))
}

fn remove_file_if_exists(path: &Path) -> AdmResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
fn transactional_replace_selected_paths_with(
    source_root: &Path,
    target_root: &Path,
    relative_paths: &[&str],
    excluded_paths: &[&str],
    mut copy_selected: impl FnMut(&Path, &Path) -> AdmResult<()>,
) -> AdmResult<()> {
    let staging = prepare_selected_paths_with(
        source_root,
        target_root,
        relative_paths,
        excluded_paths,
        &mut copy_selected,
    )?;
    let swap = swap_staged_selected_paths(&staging, target_root, relative_paths, excluded_paths)?;
    let _ = swap.finalize();
    Ok(())
}

fn prepare_selected_paths(
    source_root: &Path,
    target_root: &Path,
    relative_paths: &[&str],
    excluded_paths: &[&str],
) -> AdmResult<PathBuf> {
    prepare_selected_paths_with(
        source_root,
        target_root,
        relative_paths,
        excluded_paths,
        &mut |source, target| copy_path_filtered(source_root, source, target, excluded_paths),
    )
}

fn prepare_selected_paths_with(
    source_root: &Path,
    target_root: &Path,
    relative_paths: &[&str],
    excluded_paths: &[&str],
    copy_selected: &mut impl FnMut(&Path, &Path) -> AdmResult<()>,
) -> AdmResult<PathBuf> {
    let staging = unique_sibling_path(target_root, "staging")?;
    fs::create_dir_all(&staging)?;
    let prepare = (|| {
        if target_root.is_dir() {
            let mut skipped = relative_paths
                .iter()
                .map(|path| normalize_relative(path))
                .collect::<Vec<_>>();
            skipped.extend(excluded_paths.iter().map(|path| normalize_relative(path)));
            copy_directory_contents_filtered(target_root, &staging, target_root, &skipped)?;
        }
        for relative in relative_paths {
            let source = source_root.join(relative);
            if source.exists() {
                copy_selected(&source, &staging.join(relative))?;
            }
        }
        for excluded in excluded_paths {
            remove_path_if_exists(&staging.join(excluded))?;
        }
        Ok(())
    })();
    if let Err(error) = prepare {
        let _ = remove_path_if_exists(&staging);
        return Err(error);
    }
    Ok(staging)
}

fn unique_sibling_path(target: &Path, role: &str) -> AdmResult<PathBuf> {
    let parent = target
        .parent()
        .ok_or_else(|| AdmError::new(format!("path has no parent: {}", target.display())))?;
    fs::create_dir_all(parent)?;
    let name = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("workspace");
    Ok(parent.join(format!(".{name}.{role}.{}", new_stable_id("txn")?)))
}

fn create_selected_swap_journal(
    project_root: &ProjectRoot,
    session_id: &str,
    role: &str,
    target_root: &Path,
    staging_root: &Path,
    backup_root: &Path,
    entries: Vec<SelectedSwapJournalEntry>,
    before_image: TransactionBeforeImage,
) -> AdmResult<SelectedSwapJournalHandle> {
    let role = sanitize_identifier(role)?;
    let transaction_id = new_stable_id("txn")?;
    let journal_dir = project_root.resolve_relative(TRANSACTION_DIR)?;
    fs::create_dir_all(&journal_dir)?;
    let journal_path = journal_dir.join(format!("{session_id}_{role}_{transaction_id}.json"));
    let commit_marker_path = journal_path.with_extension("commit");
    let journal = SelectedSwapJournal {
        schema_version: TRANSACTION_SCHEMA_VERSION,
        transaction_id: transaction_id.clone(),
        session_id: session_id.to_string(),
        role,
        created_at_unix_ms: unix_timestamp_millis(),
        target_root: transaction_root_relative(project_root, target_root)?,
        staging_root: transaction_root_relative(project_root, staging_root)?,
        backup_root: transaction_root_relative(project_root, backup_root)?,
        entries,
        before_image,
    };
    let text = serde_json::to_string_pretty(&journal)
        .map_err(|error| AdmError::new(format!("failed to serialize save transaction: {error}")))?;
    adm_new_foundation::write_text_atomic(&journal_path, &(text + "\n"))?;
    Ok(SelectedSwapJournalHandle {
        transaction_id,
        journal_path,
        commit_marker_path,
        before_image: journal.before_image,
        project_root: project_root.path().to_path_buf(),
    })
}

fn transaction_root_relative(project_root: &ProjectRoot, path: &Path) -> AdmResult<String> {
    let relative = path.strip_prefix(project_root.path()).map_err(|_| {
        AdmError::new(format!(
            "transaction path escapes project root: {}",
            path.display()
        ))
    })?;
    let relative = relative.to_string_lossy().replace('\\', "/");
    project_root.resolve_relative(&relative)?;
    Ok(relative)
}

fn capture_transaction_files(
    project_root: &ProjectRoot,
    paths: &[&Path],
) -> AdmResult<Vec<TransactionFileBeforeImage>> {
    paths
        .iter()
        .map(|path| {
            let existed = path.is_file();
            let content = if existed {
                fs::read_to_string(path)?
            } else {
                String::new()
            };
            Ok(TransactionFileBeforeImage {
                path: transaction_root_relative(project_root, path)?,
                existed,
                content,
            })
        })
        .collect()
}

fn restore_transaction_files(
    project_root: &ProjectRoot,
    files: &[TransactionFileBeforeImage],
) -> AdmResult<()> {
    let mut errors = Vec::new();
    for before in files {
        let result: AdmResult<()> = (|| {
            let path = project_root.resolve_relative(&before.path)?;
            if before.existed {
                adm_new_foundation::write_text_atomic(&path, &before.content)
            } else {
                remove_file_if_exists(&path)
            }
        })();
        if let Err(error) = result {
            errors.push(format!("restore {}: {error}", before.path));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AdmError::new(errors.join("; ")))
    }
}

fn rollback_transaction_directories(
    project_root: &ProjectRoot,
    directories: &[TransactionDirectoryBeforeImage],
) -> AdmResult<()> {
    let mut errors = Vec::new();
    for before in directories.iter().rev() {
        let result: AdmResult<()> = (|| {
            let target = project_root.resolve_relative(&before.target)?;
            let staging = project_root.resolve_relative(&before.staging)?;
            let backup = project_root.resolve_relative(&before.backup)?;
            if backup.exists() {
                if target.exists() {
                    if !staging.exists() {
                        if let Some(parent) = staging.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::rename(&target, &staging)?;
                    } else {
                        remove_path_if_exists(&target)?;
                    }
                }
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(&backup, &target)?;
            } else if !before.had_target && target.exists() && !staging.exists() {
                if let Some(parent) = staging.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(&target, &staging)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            errors.push(format!("restore directory {}: {error}", before.target));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AdmError::new(errors.join("; ")))
    }
}

fn remove_transaction_rollback_paths(
    project_root: &ProjectRoot,
    paths: &[String],
) -> AdmResult<()> {
    let mut errors = Vec::new();
    for relative in paths.iter().rev() {
        match project_root
            .resolve_relative(relative)
            .and_then(|path| remove_path_if_exists(&path))
        {
            Ok(()) => {}
            Err(error) => errors.push(format!("remove rollback path {relative}: {error}")),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AdmError::new(errors.join("; ")))
    }
}

fn recover_pending_transactions(project_root: &ProjectRoot) -> AdmResult<()> {
    let global_path = global_transaction_lock_path(project_root)?;
    let Some(global_guard) = try_acquire_recovery_lock(&global_path)? else {
        return Ok(());
    };
    let recovery = recover_pending_transactions_with_global_lock(project_root);
    let unlock = FileExt::unlock(&global_guard).map_err(AdmError::from);
    match (recovery, unlock) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(unlock_error)) => Err(AdmError::new(format!(
            "transaction recovery failed: {error}; global lock release failed: {unlock_error}"
        ))),
    }
}

fn global_transaction_lock_path(project_root: &ProjectRoot) -> AdmResult<PathBuf> {
    project_root.resolve_relative(&format!("{TRANSACTION_DIR}/global.lock"))
}

fn recover_pending_transactions_with_global_lock(project_root: &ProjectRoot) -> AdmResult<()> {
    let journal_dir = project_root.resolve_relative(TRANSACTION_DIR)?;
    if !journal_dir.is_dir() {
        return Ok(());
    }
    let mut pending = BTreeMap::<String, Vec<(PathBuf, SelectedSwapJournal)>>::new();
    for entry in fs::read_dir(&journal_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let journal: SelectedSwapJournal = serde_json::from_str(&text).map_err(|error| {
            AdmError::new(format!(
                "invalid save transaction journal at {}: {error}",
                path.display()
            ))
        })?;
        if journal.schema_version != TRANSACTION_SCHEMA_VERSION {
            return Err(AdmError::new(format!(
                "unsupported save transaction schema {} at {}",
                journal.schema_version,
                path.display()
            )));
        }
        if sanitize_identifier(&journal.session_id)? != journal.session_id {
            return Err(AdmError::new(format!(
                "save transaction has a non-portable session id at {}",
                path.display()
            )));
        }
        let expected_prefix = format!("{}_", journal.session_id);
        if !path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with(&expected_prefix))
        {
            return Err(AdmError::new(format!(
                "save transaction owner mismatch at {}",
                path.display()
            )));
        }
        pending
            .entry(journal.session_id.clone())
            .or_default()
            .push((path, journal));
    }
    for (session_id, mut journals) in pending {
        let session_lock_path =
            project_root.resolve_relative(&format!("{TRANSACTION_DIR}/{session_id}.lock"))?;
        let Some(_session_guard) = try_acquire_recovery_lock(&session_lock_path)? else {
            continue;
        };
        let index_lock_path = project_root.resolve_relative("saves/.locks/index.lock")?;
        let Some(_index_guard) = try_acquire_recovery_lock(&index_lock_path)? else {
            continue;
        };
        let mut archive_ids = BTreeSet::new();
        for (_, journal) in &journals {
            collect_transaction_archive_ids(journal, &mut archive_ids);
        }
        let mut archive_guards = Vec::new();
        let mut all_archives_available = true;
        for save_id in archive_ids {
            let path =
                project_root.resolve_relative(&format!("saves/.locks/archive_{save_id}.lock"))?;
            match try_acquire_recovery_lock(&path)? {
                Some(guard) => archive_guards.push(guard),
                None => {
                    all_archives_available = false;
                    break;
                }
            }
        }
        if !all_archives_available {
            continue;
        }
        journals.sort_by(|left, right| {
            right
                .1
                .created_at_unix_ms
                .cmp(&left.1.created_at_unix_ms)
                .then_with(|| right.1.transaction_id.cmp(&left.1.transaction_id))
        });
        for (journal_path, journal) in journals {
            recover_selected_swap_journal(project_root, &journal_path, &journal)?;
        }
        drop(archive_guards);
    }
    Ok(())
}

fn try_acquire_recovery_lock(path: &Path) -> AdmResult<Option<File>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)?;
    match FileExt::try_lock_exclusive(&file) {
        Ok(()) => Ok(Some(file)),
        Err(error) if lock_error_is_contended(&error) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn lock_error_is_contended(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock
        || matches!(error.raw_os_error(), Some(11 | 32 | 33 | 36))
}

fn acquire_bounded_transaction_lock(file: &File, role: &str) -> AdmResult<()> {
    for attempt in 0..TRANSACTION_LOCK_RETRY_LIMIT {
        match FileExt::try_lock_exclusive(file) {
            Ok(()) => return Ok(()),
            Err(error) if lock_error_is_contended(&error) => {
                if attempt + 1 < TRANSACTION_LOCK_RETRY_LIMIT {
                    std::thread::sleep(std::time::Duration::from_millis(
                        TRANSACTION_LOCK_RETRY_DELAY_MS,
                    ));
                    continue;
                }
                return Err(AdmError::new(format!(
                    "save index is locked by another {role}"
                )));
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(AdmError::new(format!(
        "save index is locked by another {role}"
    )))
}

fn collect_transaction_archive_ids(
    journal: &SelectedSwapJournal,
    archive_ids: &mut BTreeSet<String>,
) {
    collect_archive_id_from_relative(&journal.target_root, archive_ids);
    for file in &journal.before_image.files {
        collect_archive_id_from_relative(&file.path, archive_ids);
    }
    for path in &journal.before_image.remove_paths {
        collect_archive_id_from_relative(path, archive_ids);
    }
    for save_id in &journal.before_image.archive_ids {
        if safe_component("save_id", save_id).is_ok() {
            archive_ids.insert(save_id.clone());
        }
    }
}

fn collect_archive_id_from_relative(relative: &str, archive_ids: &mut BTreeSet<String>) {
    let normalized = normalize_relative(relative);
    let mut components = normalized.split('/').map(str::to_string);
    if components.next().as_deref() != Some("saves") {
        return;
    }
    let Some(save_id) = components.next() else {
        return;
    };
    if save_id.starts_with('.') || save_id.contains('.') {
        return;
    }
    if safe_component("save_id", &save_id).is_ok() {
        archive_ids.insert(save_id);
    }
}

fn recover_selected_swap_journal(
    project_root: &ProjectRoot,
    journal_path: &Path,
    journal: &SelectedSwapJournal,
) -> AdmResult<()> {
    let commit_marker_path = journal_path.with_extension("commit");
    let target_root = project_root.resolve_relative(&journal.target_root)?;
    let staging_root = project_root.resolve_relative(&journal.staging_root)?;
    let backup_root = project_root.resolve_relative(&journal.backup_root)?;
    let entries = journal
        .entries
        .iter()
        .map(|entry| {
            let target = ensure_relative_path(&target_root, &entry.relative_path)?;
            let staged = ensure_relative_path(&staging_root, &entry.relative_path)?;
            let backup = ensure_relative_path(&backup_root, &entry.relative_path)?;
            Ok(SelectedPathSwapEntry {
                target,
                staged,
                backup,
                will_install: entry.will_install,
            })
        })
        .collect::<AdmResult<Vec<_>>>()?;
    if commit_marker_path.exists() {
        let marker = fs::read_to_string(&commit_marker_path)?;
        if marker.trim() != journal.transaction_id {
            return Err(AdmError::new(format!(
                "save transaction commit marker mismatch at {}",
                commit_marker_path.display()
            )));
        }
        let mut warnings = Vec::new();
        collect_cleanup_warning(&mut warnings, "recovered staging", &staging_root);
        collect_cleanup_warning(&mut warnings, "recovered backup", &backup_root);
        for directory in &journal.before_image.directories {
            collect_cleanup_warning(
                &mut warnings,
                "recovered transaction directory staging",
                &project_root.resolve_relative(&directory.staging)?,
            );
            collect_cleanup_warning(
                &mut warnings,
                "recovered transaction directory backup",
                &project_root.resolve_relative(&directory.backup)?,
            );
        }
        if warnings.is_empty() {
            if let Err(error) = remove_file_if_exists(journal_path) {
                warnings.push(format!(
                    "recovered journal cleanup failed at {}: {error}",
                    journal_path.display()
                ));
            } else if let Err(error) = remove_file_if_exists(&commit_marker_path) {
                warnings.push(format!(
                    "recovered commit marker cleanup failed at {}: {error}",
                    commit_marker_path.display()
                ));
            }
        }
        if !warnings.is_empty() {
            persist_cleanup_warnings(project_root, "committed_transaction_recovery", &warnings)?;
        }
        return Ok(());
    }

    restore_transaction_files(project_root, &journal.before_image.files)?;
    rollback_transaction_directories(project_root, &journal.before_image.directories)?;
    rollback_selected_swap_entries(&entries)?;
    remove_transaction_rollback_paths(project_root, &journal.before_image.remove_paths)?;
    remove_file_if_exists(journal_path)?;
    let mut warnings = Vec::new();
    collect_cleanup_warning(&mut warnings, "rolled back staging", &staging_root);
    collect_cleanup_warning(&mut warnings, "rolled back backup", &backup_root);
    for directory in &journal.before_image.directories {
        collect_cleanup_warning(
            &mut warnings,
            "rolled back transaction directory staging",
            &project_root.resolve_relative(&directory.staging)?,
        );
        collect_cleanup_warning(
            &mut warnings,
            "rolled back transaction directory backup",
            &project_root.resolve_relative(&directory.backup)?,
        );
    }
    if !warnings.is_empty() {
        persist_cleanup_warnings(project_root, "rolled_back_transaction_recovery", &warnings)?;
    }
    Ok(())
}

fn persist_cleanup_warnings(
    project_root: &ProjectRoot,
    context: &str,
    warnings: &[String],
) -> AdmResult<()> {
    if warnings.is_empty() {
        return Ok(());
    }
    let path =
        project_root.resolve_relative(&format!("{TRANSACTION_DIR}/cleanup_warnings.jsonl"))?;
    append_jsonl_value(
        &path,
        &serde_json::json!({
            "timestamp": timestamp(),
            "context": context,
            "warnings": warnings,
        }),
    )
}

#[derive(Debug)]
struct DirectorySwap {
    target: PathBuf,
    backup: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SelectedSwapJournalEntry {
    relative_path: String,
    will_install: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct TransactionBeforeImage {
    #[serde(default)]
    files: Vec<TransactionFileBeforeImage>,
    #[serde(default)]
    directories: Vec<TransactionDirectoryBeforeImage>,
    #[serde(default)]
    remove_paths: Vec<String>,
    #[serde(default)]
    archive_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransactionFileBeforeImage {
    path: String,
    existed: bool,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransactionDirectoryBeforeImage {
    target: String,
    staging: String,
    backup: String,
    had_target: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SelectedSwapJournal {
    schema_version: u32,
    transaction_id: String,
    session_id: String,
    role: String,
    created_at_unix_ms: u128,
    target_root: String,
    staging_root: String,
    backup_root: String,
    entries: Vec<SelectedSwapJournalEntry>,
    #[serde(default)]
    before_image: TransactionBeforeImage,
}

#[derive(Debug)]
struct SelectedSwapJournalHandle {
    transaction_id: String,
    journal_path: PathBuf,
    commit_marker_path: PathBuf,
    before_image: TransactionBeforeImage,
    project_root: PathBuf,
}

#[derive(Debug)]
struct SelectedPathSwapEntry {
    target: PathBuf,
    staged: PathBuf,
    backup: PathBuf,
    will_install: bool,
}

#[derive(Debug)]
struct SelectedPathsSwap {
    staging_root: PathBuf,
    backup_root: PathBuf,
    entries: Vec<SelectedPathSwapEntry>,
    journal: Option<SelectedSwapJournalHandle>,
}

impl SelectedPathsSwap {
    fn rollback(self) -> AdmResult<()> {
        rollback_selected_swap_entries(&self.entries)?;
        if let Some(journal) = &self.journal {
            remove_file_if_exists(&journal.journal_path)?;
            let _ = remove_file_if_exists(&journal.commit_marker_path);
        }
        let mut errors = Vec::new();
        collect_cleanup_error(&mut errors, "staging", &self.staging_root);
        collect_cleanup_error(&mut errors, "backup", &self.backup_root);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(AdmError::new(errors.join("; ")))
        }
    }

    fn mark_committed(&self) -> AdmResult<()> {
        let Some(journal) = &self.journal else {
            return Ok(());
        };
        adm_new_foundation::write_text_atomic(
            &journal.commit_marker_path,
            &format!("{}\n", journal.transaction_id),
        )
    }

    fn rollback_with_before_image(self, project_root: &ProjectRoot) -> AdmResult<()> {
        let Some(journal) = &self.journal else {
            return self.rollback();
        };
        restore_transaction_files(project_root, &journal.before_image.files)?;
        rollback_transaction_directories(project_root, &journal.before_image.directories)?;
        rollback_selected_swap_entries(&self.entries)?;
        remove_transaction_rollback_paths(project_root, &journal.before_image.remove_paths)?;
        remove_file_if_exists(&journal.journal_path)?;
        let _ = remove_file_if_exists(&journal.commit_marker_path);
        let mut errors = Vec::new();
        collect_cleanup_error(&mut errors, "staging", &self.staging_root);
        collect_cleanup_error(&mut errors, "backup", &self.backup_root);
        for directory in &journal.before_image.directories {
            collect_cleanup_error(
                &mut errors,
                "transaction directory staging",
                &project_root.resolve_relative(&directory.staging)?,
            );
            collect_cleanup_error(
                &mut errors,
                "transaction directory backup",
                &project_root.resolve_relative(&directory.backup)?,
            );
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(AdmError::new(errors.join("; ")))
        }
    }

    fn finalize(self) -> Vec<String> {
        let mut warnings = Vec::new();
        collect_cleanup_warning(&mut warnings, "staging", &self.staging_root);
        collect_cleanup_warning(&mut warnings, "backup", &self.backup_root);
        if let Some(journal) = &self.journal {
            for directory in &journal.before_image.directories {
                collect_cleanup_warning(
                    &mut warnings,
                    "transaction directory staging",
                    &journal.project_root.join(&directory.staging),
                );
                collect_cleanup_warning(
                    &mut warnings,
                    "transaction directory backup",
                    &journal.project_root.join(&directory.backup),
                );
            }
        }
        if warnings.is_empty()
            && let Some(journal) = &self.journal
        {
            if let Err(error) = remove_file_if_exists(&journal.journal_path) {
                warnings.push(format!(
                    "transaction journal cleanup failed at {}: {error}",
                    journal.journal_path.display()
                ));
            } else if let Err(error) = remove_file_if_exists(&journal.commit_marker_path) {
                warnings.push(format!(
                    "transaction commit marker cleanup failed at {}: {error}",
                    journal.commit_marker_path.display()
                ));
            }
        }
        warnings
    }
}

#[cfg(test)]
fn swap_staged_selected_paths(
    staging_root: &Path,
    target_root: &Path,
    relative_paths: &[&str],
    excluded_paths: &[&str],
) -> AdmResult<SelectedPathsSwap> {
    swap_staged_selected_paths_internal(
        staging_root,
        target_root,
        relative_paths,
        excluded_paths,
        None,
    )
}

fn swap_staged_selected_paths_journaled(
    project_root: &ProjectRoot,
    session_id: &str,
    role: &str,
    staging_root: &Path,
    target_root: &Path,
    relative_paths: &[&str],
    excluded_paths: &[&str],
    before_image: TransactionBeforeImage,
) -> AdmResult<SelectedPathsSwap> {
    swap_staged_selected_paths_internal(
        staging_root,
        target_root,
        relative_paths,
        excluded_paths,
        Some((project_root, session_id, role, before_image)),
    )
}

fn swap_staged_selected_paths_internal(
    staging_root: &Path,
    target_root: &Path,
    relative_paths: &[&str],
    excluded_paths: &[&str],
    journal_context: Option<(&ProjectRoot, &str, &str, TransactionBeforeImage)>,
) -> AdmResult<SelectedPathsSwap> {
    fs::create_dir_all(target_root)?;
    let backup_root = unique_sibling_path(target_root, "selected-backup")?;
    let mut paths = relative_paths
        .iter()
        .chain(excluded_paths.iter())
        .map(|path| normalize_relative(path))
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| path.matches('/').count());
    paths.dedup();
    let mut selected = Vec::<String>::new();
    for path in paths {
        if selected
            .iter()
            .any(|parent| path.starts_with(&format!("{parent}/")))
        {
            continue;
        }
        selected.push(path);
    }
    let journal_entries = selected
        .iter()
        .map(|relative| SelectedSwapJournalEntry {
            relative_path: relative.clone(),
            will_install: staging_root.join(relative).exists(),
        })
        .collect::<Vec<_>>();
    let journal = match journal_context {
        Some((project_root, session_id, role, before_image)) => Some(create_selected_swap_journal(
            project_root,
            session_id,
            role,
            target_root,
            staging_root,
            &backup_root,
            journal_entries,
            before_image,
        )?),
        None => None,
    };
    let mut swap = SelectedPathsSwap {
        staging_root: staging_root.to_path_buf(),
        backup_root: backup_root.clone(),
        entries: Vec::new(),
        journal,
    };
    if let Err(error) = fs::create_dir_all(&backup_root) {
        let rollback = swap.rollback();
        return selected_swap_error(error.into(), rollback);
    }
    for relative in selected {
        let target = target_root.join(&relative);
        let staged = staging_root.join(&relative);
        let backup = backup_root.join(&relative);
        let will_install = staged.exists();
        if target.exists() {
            if let Some(parent) = backup.parent() {
                if let Err(error) = fs::create_dir_all(parent) {
                    let rollback = swap.rollback();
                    return selected_swap_error(error.into(), rollback);
                }
            }
            if let Err(error) = fs::rename(&target, &backup) {
                let rollback = swap.rollback();
                return selected_swap_error(error.into(), rollback);
            }
        }
        swap.entries.push(SelectedPathSwapEntry {
            target: target.clone(),
            staged: staged.clone(),
            backup,
            will_install,
        });
        if staged.exists() {
            if let Some(parent) = target.parent() {
                if let Err(error) = fs::create_dir_all(parent) {
                    let rollback = swap.rollback();
                    return selected_swap_error(error.into(), rollback);
                }
            }
            if let Err(error) = fs::rename(&staged, &target) {
                let rollback = swap.rollback();
                return selected_swap_error(error.into(), rollback);
            }
        }
    }
    Ok(swap)
}

fn rollback_selected_swap_entries(entries: &[SelectedPathSwapEntry]) -> AdmResult<()> {
    let mut errors = Vec::new();
    for entry in entries.iter().rev() {
        if let Err(error) = rollback_selected_swap_entry(entry) {
            errors.push(error.to_string());
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AdmError::new(errors.join("; ")))
    }
}

fn rollback_selected_swap_entry(entry: &SelectedPathSwapEntry) -> AdmResult<()> {
    if entry.backup.exists() {
        if entry.target.exists() {
            if entry.will_install && !entry.staged.exists() {
                if let Some(parent) = entry.staged.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::rename(&entry.target, &entry.staged)?;
            } else {
                remove_path_if_exists(&entry.target)?;
            }
        }
        if let Some(parent) = entry.target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&entry.backup, &entry.target)?;
    } else if entry.will_install && !entry.staged.exists() && entry.target.exists() {
        if let Some(parent) = entry.staged.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&entry.target, &entry.staged)?;
    }
    Ok(())
}

fn collect_cleanup_error(errors: &mut Vec<String>, role: &str, path: &Path) {
    if let Err(error) = remove_path_if_exists(path) {
        errors.push(format!("{role} cleanup {}: {error}", path.display()));
    }
}

fn collect_cleanup_warning(warnings: &mut Vec<String>, role: &str, path: &Path) {
    if let Err(error) = remove_path_if_exists(path) {
        warnings.push(format!(
            "{role} cleanup failed at {}: {error}",
            path.display()
        ));
    }
}

fn selected_swap_error(error: AdmError, rollback: AdmResult<()>) -> AdmResult<SelectedPathsSwap> {
    match rollback {
        Ok(()) => Err(error),
        Err(rollback_error) => Err(AdmError::new(format!(
            "selected path swap failed: {error}; rollback failed: {rollback_error}"
        ))),
    }
}

impl DirectorySwap {
    fn rollback(self) -> AdmResult<()> {
        remove_path_if_exists(&self.target)?;
        if let Some(backup) = self.backup {
            fs::rename(backup, self.target)?;
        }
        Ok(())
    }

    fn finalize(self) -> Vec<String> {
        let mut warnings = Vec::new();
        if let Some(backup) = self.backup {
            collect_cleanup_warning(&mut warnings, "directory backup", &backup);
        }
        warnings
    }
}

fn swap_staged_directory_with_backup(
    staging: &Path,
    target: &Path,
    backup: &Path,
) -> AdmResult<DirectorySwap> {
    let had_target = target.exists();
    if had_target {
        fs::rename(target, backup)?;
    }
    match fs::rename(staging, target) {
        Ok(()) => Ok(DirectorySwap {
            target: target.to_path_buf(),
            backup: had_target.then(|| backup.to_path_buf()),
        }),
        Err(commit_error) => {
            let restore = if had_target {
                fs::rename(backup, target).map_err(AdmError::from)
            } else {
                Ok(())
            };
            let _ = remove_path_if_exists(staging);
            match restore {
                Ok(()) => Err(commit_error.into()),
                Err(restore_error) => Err(AdmError::new(format!(
                    "staged directory commit failed: {commit_error}; backup restore failed: {restore_error}"
                ))),
            }
        }
    }
}

fn copy_directory_contents_filtered(
    source: &Path,
    target: &Path,
    relative_root: &Path,
    skipped_paths: &[String],
) -> AdmResult<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        copy_path_filtered_with_normalized_skips(
            relative_root,
            &entry.path(),
            &target.join(entry.file_name()),
            skipped_paths,
        )?;
    }
    Ok(())
}

fn copy_path_filtered(
    relative_root: &Path,
    source: &Path,
    target: &Path,
    skipped_paths: &[&str],
) -> AdmResult<()> {
    let skipped = skipped_paths
        .iter()
        .map(|path| normalize_relative(path))
        .collect::<Vec<_>>();
    copy_path_filtered_with_normalized_skips(relative_root, source, target, &skipped)
}

fn copy_path_filtered_with_normalized_skips(
    relative_root: &Path,
    source: &Path,
    target: &Path,
    skipped_paths: &[String],
) -> AdmResult<()> {
    let relative = source
        .strip_prefix(relative_root)
        .map_err(|error| AdmError::new(format!("copy source escaped root: {error}")))?
        .to_string_lossy()
        .replace('\\', "/");
    if skipped_paths
        .iter()
        .any(|skipped| relative == *skipped || relative.starts_with(&format!("{skipped}/")))
    {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(AdmError::new(format!(
            "save workspace cannot contain symbolic links: {}",
            source.display()
        )));
    }
    if metadata.is_dir() {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_path_filtered_with_normalized_skips(
                relative_root,
                &entry.path(),
                &target.join(entry.file_name()),
                skipped_paths,
            )?;
        }
    } else if metadata.is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
    }
    Ok(())
}

fn normalize_relative(path: &str) -> String {
    path.trim_matches(['/', '\\']).replace('\\', "/")
}

fn copy_path_without_symlinks(source: &Path, target: &Path) -> AdmResult<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.file_type().is_symlink() {
        return Err(AdmError::new(format!(
            "save workspace cannot contain symbolic links: {}",
            source.display()
        )));
    }
    if metadata.is_dir() {
        fs::create_dir_all(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_path_without_symlinks(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else if metadata.is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, target)?;
    }
    Ok(())
}

fn build_file_map(
    workspace: &Path,
    transaction_seq: u64,
    generated_at: String,
) -> AdmResult<FileMap> {
    let mut files = Vec::new();
    if workspace.exists() {
        collect_file_map_entries(workspace, workspace, transaction_seq, &mut files)?;
    }
    files.sort_by(|left, right| left.workspace_path.cmp(&right.workspace_path));
    Ok(FileMap {
        schema_version: SAVE_SCHEMA_VERSION,
        generated_at,
        transaction_seq: Some(transaction_seq),
        files,
    })
}

fn collect_file_map_entries(
    root: &Path,
    current: &Path,
    transaction_seq: u64,
    files: &mut Vec<FileMapEntry>,
) -> AdmResult<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_file_map_entries(root, &path, transaction_seq, files)?;
            continue;
        }
        let metadata = entry.metadata()?;
        let relative = path
            .strip_prefix(root)
            .map_err(|error| AdmError::new(format!("workspace path escaped root: {error}")))?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&path)?;
        let mtime_ns = metadata
            .modified()
            .ok()
            .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
            .unwrap_or(0);
        files.push(FileMapEntry {
            workspace_path: relative,
            size_bytes: metadata.len(),
            mtime_ns,
            sha256: sha256_hex(&bytes),
            stage: None,
            artifact_id: Value::Null,
            role: "archive_workspace_file".to_string(),
            source_type: "autosave_sync".to_string(),
            reference_manifest: String::new(),
            latest_transaction_seq: Some(transaction_seq),
            extra: BTreeMap::new(),
        });
    }
    Ok(())
}

fn diff_file_maps(previous: &FileMap, next: &FileMap) -> FileMapDelta {
    let previous_by_path = previous
        .files
        .iter()
        .map(|entry| (entry.workspace_path.clone(), entry.clone()))
        .collect::<BTreeMap<_, _>>();
    let next_by_path = next
        .files
        .iter()
        .map(|entry| (entry.workspace_path.clone(), entry.clone()))
        .collect::<BTreeMap<_, _>>();
    let added = next_by_path
        .iter()
        .filter(|(path, _)| !previous_by_path.contains_key(*path))
        .map(|(_, entry)| entry.clone())
        .collect();
    let modified = next_by_path
        .iter()
        .filter_map(|(path, after)| {
            let before = previous_by_path.get(path)?;
            (before.sha256 != after.sha256 || before.size_bytes != after.size_bytes).then(|| {
                FileMapChange {
                    before: before.clone(),
                    after: after.clone(),
                }
            })
        })
        .collect();
    let removed = previous_by_path
        .iter()
        .filter(|(path, _)| !next_by_path.contains_key(*path))
        .map(|(_, entry)| entry.clone())
        .collect();
    FileMapDelta {
        added,
        modified,
        removed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::project::NodeState;

    #[test]
    fn crate_reports_ready() {
        assert!(crate_ready());
        assert_eq!(CRATE_NAME, "adm-new-save");
    }

    #[test]
    fn save_service_autosaves_and_creates_formal_archive_snapshot() {
        let root = temp_root("create");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();
        let state = sample_state("Project A", true);

        let autosave_meta = service.write_autosave(&state).unwrap();
        assert_eq!(autosave_meta.workspace_state, WorkspaceState::Unsaved);

        let report = service.create_save("Main Save", &state).unwrap();
        assert_eq!(report.manifest.last_transaction_seq, 1);
        assert_eq!(
            report.index.current_save_id.as_deref(),
            Some(report.manifest.save_id.as_str())
        );
        assert_eq!(report.file_map.files.len(), 1);
        assert_eq!(report.snapshot_manifest.added, 1);
        assert!(
            root.join("saves")
                .join(&report.manifest.save_id)
                .join("workspace")
                .join("autosave_state.json")
                .exists()
        );
        assert!(
            root.join("drafts")
                .join("session_a")
                .join("timeline.jsonl")
                .exists()
        );
        cleanup(root);
    }

    #[test]
    fn save_service_sync_increments_transaction_and_tracks_modified_file() {
        let root = temp_root("sync");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();
        let created = service
            .create_save("Main Save", &sample_state("Project A", false))
            .unwrap();
        let synced = service
            .sync_current_save(&sample_state("Project B", true), "manual_save")
            .unwrap();

        assert_eq!(created.manifest.last_transaction_seq, 1);
        assert_eq!(synced.manifest.last_transaction_seq, 2);
        assert_eq!(synced.snapshot_manifest.modified, 1);
        assert_eq!(synced.snapshot_manifest.added, 0);
        cleanup(root);
    }

    #[test]
    fn save_service_rejects_live_os_lock_even_when_metadata_is_stale() {
        let root = temp_root("lock");
        let current_pid = std::process::id();
        let service_a = SaveService::with_pid(&root, "session_a", current_pid).unwrap();
        let created = service_a
            .create_save("Locked Save", &sample_state("Project A", true))
            .unwrap();
        let service_b = SaveService::with_pid(&root, "session_b", current_pid).unwrap();
        let conflict = service_b.load_save(&created.manifest.save_id).unwrap_err();
        assert!(conflict.message().contains("locked by session session_a"));

        let lock_path = root
            .join("saves")
            .join(&created.manifest.save_id)
            .join(".archive_lock");
        let stale_lock = ArchiveLock {
            pid: 0,
            session_id: "session_a".to_string(),
            acquired_at: timestamp(),
            live: Some(true),
            lock_path: Some(format!("saves/{}/.archive_lock", created.manifest.save_id)),
        };
        fs::write(
            &lock_path,
            serde_json::to_string_pretty(&stale_lock).unwrap(),
        )
        .unwrap();
        let still_locked = service_b.load_save(&created.manifest.save_id).unwrap_err();
        assert!(
            still_locked
                .message()
                .contains("locked by session session_a")
        );
        service_a.release_current_lock().unwrap();
        let loaded = service_b.load_save(&created.manifest.save_id).unwrap();
        assert_eq!(loaded.state.project_name, "Project A");
        let replaced: ArchiveLock =
            serde_json::from_str(&fs::read_to_string(lock_path).unwrap()).unwrap();
        assert_eq!(replaced.session_id, "session_b");
        cleanup(root);
    }

    #[test]
    fn save_service_switch_and_release_unlocks_stable_archive_guards() {
        let root = temp_root("release_lock");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let first = service
            .create_save("First", &sample_state("Project A", true))
            .unwrap();
        let first_lock = root
            .join("saves")
            .join(&first.manifest.save_id)
            .join(".archive_lock");
        assert!(first_lock.exists());

        let second = service
            .create_save("Second", &sample_state("Project B", false))
            .unwrap();
        let second_lock = root
            .join("saves")
            .join(&second.manifest.save_id)
            .join(".archive_lock");
        assert_eq!(
            read_archive_lock_file(&first_lock).unwrap().unwrap().live,
            Some(false)
        );
        assert_archive_lock_available(&service, &first.manifest.save_id);
        assert!(second_lock.exists());

        service.release_current_lock().unwrap();
        assert_eq!(
            read_archive_lock_file(&second_lock).unwrap().unwrap().live,
            Some(false)
        );
        assert_archive_lock_available(&service, &second.manifest.save_id);
        cleanup(root);
    }

    #[test]
    fn save_service_acquire_current_lock_preserves_newer_draft_state_and_outputs() {
        let root = temp_root("acquire_current_lock");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let draft = root.join("drafts/session_a");
        let artifact = draft.join("outputs/artifacts/stage_00/result.json");
        fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        fs::write(&artifact, r#"{"version":"archived"}"#).unwrap();
        let created = service
            .create_save("Current", &sample_state("Archived State", true))
            .unwrap();
        service.release_current_lock().unwrap();

        service
            .write_autosave(&sample_state("Newer Autosave", false))
            .unwrap();
        fs::write(&artifact, r#"{"version":"newer-draft"}"#).unwrap();
        let autosave_path = draft.join("autosave_state.json");
        let meta_path = draft.join("draft_meta.json");
        let autosave_before = fs::read(&autosave_path).unwrap();
        let artifact_before = fs::read(&artifact).unwrap();
        let meta_before = fs::read(&meta_path).unwrap();

        service.acquire_current_lock().unwrap();

        assert_eq!(fs::read(&autosave_path).unwrap(), autosave_before);
        assert_eq!(fs::read(&artifact).unwrap(), artifact_before);
        assert_eq!(fs::read(&meta_path).unwrap(), meta_before);
        assert_eq!(
            fs::read_to_string(
                root.join("saves")
                    .join(&created.manifest.save_id)
                    .join("workspace/outputs/artifacts/stage_00/result.json")
            )
            .unwrap(),
            r#"{"version":"archived"}"#
        );
        assert!(
            root.join("saves")
                .join(&created.manifest.save_id)
                .join(".archive_lock")
                .is_file()
        );
        cleanup(root);
    }

    #[test]
    fn save_service_acquire_current_lock_falls_back_to_save_index() {
        let root = temp_root("acquire_current_lock_index");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let created = service
            .create_save("Current", &sample_state("Current", true))
            .unwrap();
        service.release_current_lock().unwrap();
        let meta_path = root.join("drafts/session_a/draft_meta.json");
        fs::remove_file(&meta_path).unwrap();

        service.acquire_current_lock().unwrap();

        assert!(!meta_path.exists());
        assert!(
            root.join("saves")
                .join(&created.manifest.save_id)
                .join(".archive_lock")
                .is_file()
        );
        cleanup(root);
    }

    #[test]
    fn save_service_restart_restores_pipeline_outputs_state_and_logs_without_contamination() {
        let root = temp_root("restore_pipeline");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let draft = root.join("drafts/session_a");
        write_draft_runtime_fixture(&draft, "save-a", "failed", "error-a");
        let first = service
            .create_save("First", &sample_state("Project A", true))
            .unwrap();

        write_draft_runtime_fixture(&draft, "save-b", "success", "log-b");
        let contaminating_artifact = draft.join("outputs/artifacts/stage_01/only-b.json");
        fs::create_dir_all(contaminating_artifact.parent().unwrap()).unwrap();
        fs::write(&contaminating_artifact, r#"{"save":"b"}"#).unwrap();
        service
            .create_save("Second", &sample_state("Project B", false))
            .unwrap();
        service.release_current_lock().unwrap();

        fs::write(
            draft.join("outputs/artifacts/stage_00/result.json"),
            r#"{"save":"unsaved"}"#,
        )
        .unwrap();
        let restarted = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let loaded = restarted.load_save(&first.manifest.save_id).unwrap();

        assert_eq!(loaded.state.project_name, "Project A");
        assert_eq!(
            fs::read_to_string(draft.join("outputs/artifacts/stage_00/result.json")).unwrap(),
            r#"{"save":"save-a"}"#
        );
        assert!(
            !draft
                .join("outputs/runtime_control/run_state.json")
                .exists()
        );
        assert_eq!(
            fs::read_to_string(draft.join("outputs/run_logs/pipeline.jsonl")).unwrap(),
            "error-a\n"
        );
        assert!(!draft.join("runtime/run_context.json").exists());
        assert_eq!(
            fs::read_to_string(draft.join("source_artifacts/pipeline_state.md")).unwrap(),
            "pipeline-save-a\n"
        );
        assert!(!contaminating_artifact.exists());
        let first_workspace = root
            .join("saves")
            .join(&first.manifest.save_id)
            .join("workspace");
        assert!(!first_workspace.join("runtime").exists());
        assert!(!first_workspace.join("outputs/runtime_control").exists());
        assert!(root.join("saves/save_index.json").is_file());
        assert!(!root.join("saves/index.json").exists());
        cleanup(root);
    }

    #[test]
    fn save_service_delete_current_save_marks_draft_deleted_copy() {
        let root = temp_root("delete");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();
        let created = service
            .create_save("Delete Me", &sample_state("Project A", true))
            .unwrap();

        let index = service.delete_save(&created.manifest.save_id).unwrap();
        assert!(index.current_save_id.is_none());
        assert!(index.saves.is_empty());
        let meta: DraftMeta = serde_json::from_str(
            &fs::read_to_string(root.join("drafts/session_a/draft_meta.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            meta.workspace_state,
            WorkspaceState::UnsavedCopyOfDeletedSave
        );
        assert_eq!(
            meta.origin_deleted_save_id.as_deref(),
            Some(created.manifest.save_id.as_str())
        );
        cleanup(root);
    }

    #[test]
    fn save_service_rename_updates_manifest_and_index() {
        let root = temp_root("rename");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();
        let created = service
            .create_save("Old Name", &sample_state("Project A", true))
            .unwrap();

        let index = service
            .rename_save(&created.manifest.save_id, "New Name")
            .unwrap();
        assert_eq!(index.saves[0].display_name, "New Name");
        let manifest: SaveManifest = serde_json::from_str(
            &fs::read_to_string(
                root.join("saves")
                    .join(&created.manifest.save_id)
                    .join("manifest.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.display_name, "New Name");
        cleanup(root);
    }

    #[test]
    fn rename_failure_restores_manifest_and_index() {
        let root = temp_root("rename_rollback");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let created = service
            .create_save("Original", &sample_state("Original", false))
            .unwrap();
        let save_id = created.manifest.save_id;
        let manifest_path = root.join("saves").join(&save_id).join("manifest.json");
        let index_path = root.join("saves/save_index.json");
        let manifest_before = fs::read(&manifest_path).unwrap();
        let index_before = fs::read(&index_path).unwrap();

        let result = service.rename_save_with_hook(&save_id, "Changed", |phase| {
            if phase == "after_manifest" {
                Err(AdmError::new("injected rename failure"))
            } else {
                Ok(())
            }
        });

        assert!(result.is_err());
        assert_eq!(fs::read(manifest_path).unwrap(), manifest_before);
        assert_eq!(fs::read(index_path).unwrap(), index_before);
        assert!(!transaction_journals(&root).iter().any(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("_rename_")
        }));
        cleanup(root);
    }

    #[test]
    fn delete_failure_restores_archive_index_and_draft_binding() {
        let root = temp_root("delete_rollback");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let created = service
            .create_save("Current", &sample_state("Current", true))
            .unwrap();
        let save_id = created.manifest.save_id;
        let save_dir = root.join("saves").join(&save_id);
        let index_path = root.join("saves/save_index.json");
        let meta_path = root.join("drafts/session_a/draft_meta.json");
        let archive_before = comparable_file_map(&save_dir);
        let index_before = fs::read(&index_path).unwrap();
        let meta_before = fs::read(&meta_path).unwrap();

        let result = service.delete_save_with_hook(&save_id, |phase| {
            if phase == "after_draft_meta" {
                Err(AdmError::new("injected delete failure"))
            } else {
                Ok(())
            }
        });

        assert!(result.is_err());
        assert!(save_dir.is_dir());
        assert_eq!(comparable_file_map(&save_dir), archive_before);
        assert_eq!(fs::read(index_path).unwrap(), index_before);
        assert_eq!(fs::read(meta_path).unwrap(), meta_before);
        assert!(!transaction_journals(&root).iter().any(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("_delete_")
        }));
        cleanup(root);
    }

    #[test]
    fn save_service_create_blank_save_resets_draft_and_progress() {
        let root = temp_root("blank");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();
        let draft_artifacts = root.join("drafts/session_a/outputs/artifacts/stage_00");
        fs::create_dir_all(&draft_artifacts).unwrap();
        fs::write(draft_artifacts.join("stale.json"), "{}").unwrap();

        let report = service.create_blank_save("Blank Save").unwrap();

        assert_eq!(report.manifest.reason, "create_blank_save");
        assert_eq!(report.manifest.progress, blank_progress());
        assert!(!draft_artifacts.exists());
        assert!(
            root.join("drafts/session_a/source_artifacts/operator_drafts")
                .is_dir()
        );
        let archived: ProjectState = serde_json::from_str(
            &fs::read_to_string(
                root.join("saves")
                    .join(&report.manifest.save_id)
                    .join("workspace/autosave_state.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(archived, ProjectState::empty());
        cleanup(root);
    }

    #[test]
    fn save_service_create_iteration_save_records_iteration_metadata() {
        let root = temp_root("iteration");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();

        let report = service
            .create_iteration_save(
                "Iteration Save",
                &sample_state("Project A", true),
                "feature",
                "v2",
                "iteration_specs/feature.json",
            )
            .unwrap();

        assert_eq!(report.manifest.save_type, "iteration");
        assert_eq!(report.manifest.change_type.as_deref(), Some("feature"));
        assert_eq!(report.manifest.requested_version.as_deref(), Some("v2"));
        assert_eq!(
            report.manifest.iteration_spec_path.as_deref(),
            Some("iteration_specs/feature.json")
        );
        assert_eq!(report.index.saves[0].save_type, "iteration");
        cleanup(root);
    }

    #[test]
    fn save_service_loads_legacy_manifest_and_migrates_manifest_json() {
        let root = temp_root("legacy");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();
        let save_id = "save_legacy";
        let save_dir = root.join("saves").join(save_id);
        fs::create_dir_all(save_dir.join("workspace")).unwrap();
        let manifest = SaveManifest {
            schema_version: SAVE_SCHEMA_VERSION,
            save_id: save_id.to_string(),
            display_name: "Legacy".to_string(),
            save_type: "manual".to_string(),
            created_by: "python".to_string(),
            reason: "legacy".to_string(),
            created_at: "unix:1".to_string(),
            last_worked_at: "unix:1".to_string(),
            last_transaction_seq: 0,
            progress: SaveProgress::default(),
            change_type: None,
            requested_version: None,
            iteration_spec_path: None,
            extra: BTreeMap::new(),
        };
        fs::write(
            save_dir.join(LEGACY_MANIFEST_NAME),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(
            save_dir.join("workspace/autosave_state.json"),
            serde_json::to_string_pretty(&sample_state("Legacy Project", true)).unwrap(),
        )
        .unwrap();

        let loaded = service.load_save(save_id).unwrap();

        assert_eq!(loaded.manifest.display_name, "Legacy");
        assert_eq!(loaded.state.project_name, "Legacy Project");
        assert!(save_dir.join(MANIFEST_NAME).exists());
        cleanup(root);
    }

    #[test]
    fn blank_save_repair_dry_run_and_apply_removes_inherited_outputs() {
        let root = temp_root("blank_repair");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();
        let created = service
            .create_save("Inherited", &sample_state("Project A", true))
            .unwrap();
        let inherited = root
            .join("saves")
            .join(&created.manifest.save_id)
            .join("workspace/outputs/artifacts/stage_00/report.json");
        fs::create_dir_all(inherited.parent().unwrap()).unwrap();
        fs::write(&inherited, "{}").unwrap();

        let dry = service
            .repair_blank_save_progress(&created.manifest.save_id, false)
            .unwrap();
        assert!(!dry.apply);
        assert!(
            dry.cleanup_paths
                .iter()
                .any(|path| path.contains("outputs/artifacts"))
        );
        assert!(inherited.exists());

        let applied = service
            .repair_blank_save_progress(&created.manifest.save_id, true)
            .unwrap();
        assert!(applied.apply);
        assert_eq!(applied.new_progress, blank_progress());
        assert!(!inherited.exists());
        assert!(
            root.join("saves")
                .join(&created.manifest.save_id)
                .join("workspace/source_artifacts/operator_drafts")
                .is_dir()
        );
        assert!(
            root.join("saves")
                .join(&created.manifest.save_id)
                .join("repair_log.jsonl")
                .exists()
        );
        cleanup(root);
    }

    #[test]
    fn parallel_isolation_audit_and_repair_draft_meta_mismatch() {
        let root = temp_root("parallel_repair");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();
        let created = service
            .create_save("Parallel", &sample_state("Project A", true))
            .unwrap();
        let draft = root.join("drafts/session_a");
        let runtime = draft.join("runtime");
        let source_root = draft.join("source_artifacts");
        fs::create_dir_all(&runtime).unwrap();
        fs::create_dir_all(&source_root).unwrap();
        let settings_snapshot = runtime.join("project_settings.snapshot.json");
        fs::write(&settings_snapshot, "{}").unwrap();
        write_json_value(
            &runtime.join("run_context.json"),
            &serde_json::json!({
                "save_id": created.manifest.save_id,
                "run_id": "run_1",
                "settings_snapshot": settings_snapshot.display().to_string(),
                "source_artifacts_root": source_root.display().to_string(),
            }),
        )
        .unwrap();
        write_json_value(
            &draft.join("draft_meta.json"),
            &serde_json::json!({
                "linked_save_id": "save_other",
                "linked_archive_path": "saves/save_other"
            }),
        )
        .unwrap();

        let audit = service.audit_parallel_isolation().unwrap();
        assert_eq!(audit.status, "issues_found");
        assert!(
            audit
                .issues
                .iter()
                .any(|issue| issue.code == "DRAFT_LINKED_SAVE_MISMATCH")
        );

        let dry = service.repair_parallel_save_contamination(false).unwrap();
        assert_eq!(dry.action_count, 1);
        assert!(!dry.actions[0].applied);
        let applied = service.repair_parallel_save_contamination(true).unwrap();
        assert_eq!(applied.mode, "apply");
        let repaired = read_json_value(&draft.join("draft_meta.json"))
            .unwrap()
            .unwrap();
        assert_eq!(
            json_string(&repaired, "linked_save_id").as_deref(),
            Some(created.manifest.save_id.as_str())
        );
        cleanup(root);
    }

    #[test]
    fn parallel_isolation_audit_detects_save_artifact_mismatch() {
        let root = temp_root("parallel_artifact");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();
        let created = service
            .create_save("Artifact", &sample_state("Project A", true))
            .unwrap();
        let report_path = root
            .join("saves")
            .join(&created.manifest.save_id)
            .join("workspace/outputs/artifacts/stage_13/scene_assembly_report.json");
        fs::create_dir_all(report_path.parent().unwrap()).unwrap();
        write_json_value(
            &report_path,
            &serde_json::json!({
                "save_id": "save_other",
                "status": "success"
            }),
        )
        .unwrap();

        let audit = service.audit_parallel_isolation().unwrap();

        assert!(
            audit
                .issues
                .iter()
                .any(|issue| issue.code == "ARTIFACT_SAVE_ID_MISMATCH")
        );
        assert!(
            audit
                .issues
                .iter()
                .any(|issue| issue.code == "STAGE13_DEVELOPMENT_PATH_MISSING")
        );
        cleanup(root);
    }

    #[test]
    fn selected_path_copy_failure_leaves_existing_target_unchanged() {
        let root = temp_root("copy_failure");
        let source = root.join("source");
        let target = root.join("target");
        fs::create_dir_all(source.join("outputs")).unwrap();
        fs::create_dir_all(target.join("outputs")).unwrap();
        fs::write(source.join("outputs/new.json"), "new").unwrap();
        fs::write(target.join("outputs/old.json"), "old").unwrap();

        let result = transactional_replace_selected_paths_with(
            &source,
            &target,
            &["outputs"],
            &[],
            |_, _| Err(AdmError::new("injected copy failure")),
        );

        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(target.join("outputs/old.json")).unwrap(),
            "old"
        );
        assert!(!target.join("outputs/new.json").exists());
        cleanup(root);
    }

    #[test]
    fn formal_sync_rolls_back_workspace_and_metadata_after_injected_failure() {
        let root = temp_root("transaction_rollback");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let created = service
            .create_save("Transactional", &sample_state("Before", false))
            .unwrap();
        let save_id = created.manifest.save_id.clone();
        let manifest_path = root.join("saves").join(&save_id).join("manifest.json");
        let workspace_state = root
            .join("saves")
            .join(&save_id)
            .join("workspace/autosave_state.json");
        let timeline = root.join("drafts/session_a/timeline.jsonl");
        let manifest_before = fs::read(&manifest_path).unwrap();
        let state_before = fs::read(&workspace_state).unwrap();
        let timeline_before = fs::read(&timeline).unwrap();

        let result = service.sync_save_with_hook(
            &save_id,
            &sample_state("After", true),
            "manual_save",
            "injected failure",
            |phase| {
                if phase == "after_manifest" {
                    Err(AdmError::new("injected metadata failure"))
                } else {
                    Ok(())
                }
            },
        );

        assert!(result.is_err());
        assert_eq!(fs::read(&manifest_path).unwrap(), manifest_before);
        assert_eq!(fs::read(&workspace_state).unwrap(), state_before);
        assert_eq!(fs::read(&timeline).unwrap(), timeline_before);
        assert!(
            !root
                .join(format!("drafts/session_a/snapshots/{save_id}_tx_2"))
                .exists()
        );
        let index = service.list_saves().unwrap();
        assert_eq!(index.saves[0].last_transaction_seq, 1);
        cleanup(root);
    }

    #[test]
    fn blank_save_from_state_preserves_design_and_user_sources_only() {
        let root = temp_root("blank_from_state");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let draft = root.join("drafts/session_a");
        fs::create_dir_all(draft.join("source_artifacts/user_notes")).unwrap();
        fs::create_dir_all(draft.join("source_artifacts/devflow_generated")).unwrap();
        fs::create_dir_all(draft.join("outputs/artifacts/stage_00")).unwrap();
        fs::write(draft.join("source_artifacts/user_notes/idea.md"), "keep").unwrap();
        fs::write(
            draft.join("source_artifacts/devflow_generated/design.md"),
            "remove",
        )
        .unwrap();
        fs::write(draft.join("outputs/artifacts/stage_00/stale.json"), "{}").unwrap();

        let state = sample_state("Retained Design", true);
        let report = service
            .create_blank_save_from_state("Blank Pipeline", &state)
            .unwrap();
        let workspace = root
            .join("saves")
            .join(&report.manifest.save_id)
            .join("workspace");
        let archived: ProjectState = serde_json::from_str(
            &fs::read_to_string(workspace.join("autosave_state.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(archived.project_name, "Retained Design");
        assert!(
            workspace
                .join("source_artifacts/user_notes/idea.md")
                .is_file()
        );
        assert!(
            !workspace
                .join("source_artifacts/devflow_generated")
                .exists()
        );
        assert!(
            !workspace
                .join("outputs/artifacts/stage_00/stale.json")
                .exists()
        );
        assert!(!draft.join("source_artifacts/devflow_generated").exists());
        assert!(draft.join("source_artifacts/user_notes/idea.md").is_file());
        cleanup(root);
    }

    #[test]
    fn locked_delete_is_rejected_and_non_current_rename_releases_lock() {
        let root = temp_root("lock_mutations");
        let owner = SaveService::with_pid(&root, "owner", std::process::id()).unwrap();
        let first = owner
            .create_save("First", &sample_state("First", false))
            .unwrap();
        let second = owner
            .create_save("Second", &sample_state("Second", false))
            .unwrap();
        let first_lock = root
            .join("saves")
            .join(&first.manifest.save_id)
            .join(".archive_lock");

        owner
            .rename_save(&first.manifest.save_id, "Renamed")
            .unwrap();
        assert_archive_lock_available(&owner, &first.manifest.save_id);
        assert!(owner.rename_save(&first.manifest.save_id, "").is_err());
        assert!(first_lock.exists());
        assert_archive_lock_available(&owner, &first.manifest.save_id);

        let contender = SaveService::with_pid(&root, "contender", std::process::id()).unwrap();
        assert!(contender.delete_save(&second.manifest.save_id).is_err());
        assert!(root.join("saves").join(&second.manifest.save_id).is_dir());
        cleanup(root);
    }

    #[test]
    fn snapshots_are_save_scoped_recoverable_and_pruned_to_five() {
        let root = temp_root("snapshot_history");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let created = service
            .create_save("History", &sample_state("Revision 1", false))
            .unwrap();
        let save_id = created.manifest.save_id;
        for seq in 2..=7 {
            service
                .sync_current_save(
                    &sample_state(&format!("Revision {seq}"), seq % 2 == 0),
                    "manual_save",
                )
                .unwrap();
        }
        let snapshots_root = root.join("drafts/session_a/snapshots");
        let prefix = format!("{save_id}_tx_");
        let snapshots = fs::read_dir(&snapshots_root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
            .collect::<Vec<_>>();
        assert_eq!(snapshots.len(), 5);
        assert!(!snapshots_root.join(format!("{save_id}_tx_1")).exists());
        let latest = snapshots_root.join(format!("{save_id}_tx_7"));
        assert!(latest.join("full/autosave_state.json").is_file());
        assert!(latest.join("delta/added.json").is_file());
        assert!(latest.join("delta/modified.json").is_file());
        assert!(latest.join("delta/removed.json").is_file());
        let timeline = fs::read_to_string(root.join("drafts/session_a/timeline.jsonl")).unwrap();
        assert!(timeline.contains(&format!(r#""save_id":"{save_id}""#)));
        cleanup(root);
    }

    #[test]
    fn save_progress_splits_design_and_pipeline_validation_reports() {
        let root = temp_root("split_progress");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let artifacts = root.join("drafts/session_a/outputs/artifacts");
        for (stage, status) in [(0, "success"), (1, "completed_with_review"), (2, "failed")] {
            let path = artifacts.join(format!("stage_{stage:02}/validation_report.json"));
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, format!(r#"{{"status":"{status}"}}"#)).unwrap();
        }
        let report = service
            .create_save("Progress", &sample_state("Progress", true))
            .unwrap();
        let progress = report.manifest.progress;
        assert_eq!((progress.passed, progress.total), (1, 1));
        assert_eq!((progress.design_passed, progress.design_total), (1, 1));
        assert_eq!((progress.pipeline_passed, progress.pipeline_total), (2, 15));
        assert_eq!(progress.pipeline_label, "2/15");
        cleanup(root);
    }

    #[test]
    fn archive_state_falls_back_to_latest_verified_design_project_read_only() {
        let root = temp_root("execution_object_fallback");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let created = service
            .create_save("Legacy Python", &sample_state("Initial", false))
            .unwrap();
        let save_id = created.manifest.save_id;
        let workspace = root.join("saves").join(&save_id).join("workspace");
        fs::remove_file(workspace.join("autosave_state.json")).unwrap();
        let eo_path = workspace.join("outputs/execution_objects/execution_objects.json");
        fs::create_dir_all(eo_path.parent().unwrap()).unwrap();
        fs::write(
            &eo_path,
            serde_json::to_string_pretty(&serde_json::json!({
                "objects": [
                    {
                        "object_type": "design_project",
                        "state": "verified",
                        "updated_at": "2026-01-01T00:00:00Z",
                        "user_content": sample_state("Older", false)
                    },
                    {
                        "object_type": "design_project",
                        "state": "verified",
                        "updated_at": "2026-02-01T00:00:00Z",
                        "user_content": sample_state("Recovered", true)
                    }
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        service.release_current_lock().unwrap();

        let loader = SaveService::with_pid(&root, "session_b", std::process::id()).unwrap();
        let loaded = loader.load_save(&save_id).unwrap();
        assert_eq!(loaded.state.project_name, "Recovered");
        assert!(!workspace.join("autosave_state.json").exists());
        assert_eq!(
            loader.read_autosave().unwrap().unwrap().project_name,
            "Recovered"
        );
        cleanup(root);
    }

    #[test]
    fn corrupt_index_is_quarantined_and_all_archive_directories_remain_visible() {
        let root = temp_root("index_rebuild");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let created = service
            .create_save("Valid", &sample_state("Valid", true))
            .unwrap();
        let corrupt_id = "save_corrupt";
        fs::create_dir_all(root.join("saves").join(corrupt_id)).unwrap();
        fs::write(
            root.join("saves").join(corrupt_id).join("manifest.json"),
            "{not json",
        )
        .unwrap();
        fs::write(root.join("saves/save_index.json"), "{also invalid").unwrap();

        let index = service.list_saves().unwrap();
        assert_eq!(
            index.current_save_id.as_deref(),
            Some(created.manifest.save_id.as_str())
        );
        assert!(
            index
                .saves
                .iter()
                .any(|entry| entry.save_id == created.manifest.save_id)
        );
        let corrupt = index
            .saves
            .iter()
            .find(|entry| entry.save_id == corrupt_id)
            .unwrap();
        assert_eq!(corrupt.integrity_status, "corrupt");
        assert!(
            fs::read_dir(root.join("saves"))
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("save_index.corrupt."))
        );
        cleanup(root);
    }

    #[test]
    fn detached_draft_does_not_inherit_global_current_and_recovery_keeps_archive() {
        let root = temp_root("detached_recovery");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let created = service
            .create_save("Archive", &sample_state("Archived", false))
            .unwrap();
        let archive = root.join("saves").join(&created.manifest.save_id);
        let fallback = sample_state("Recovered Draft", true);
        let meta = service.recover_to_unsaved_state(&fallback).unwrap();

        assert_eq!(meta.workspace_state, WorkspaceState::Unsaved);
        assert!(meta.linked_save_id.is_none());
        assert!(archive.is_dir());
        assert_eq!(
            read_archive_lock_file(&archive.join(".archive_lock"))
                .unwrap()
                .unwrap()
                .live,
            Some(false)
        );
        assert_archive_lock_available(&service, &created.manifest.save_id);
        let index = service.list_saves().unwrap();
        assert!(index.current_save_id.is_none());
        assert_eq!(index.workspace_state, "unsaved");
        assert_eq!(
            service.read_autosave().unwrap().unwrap().project_name,
            "Recovered Draft"
        );
        assert!(service.release_current_lock().is_ok());
        cleanup(root);
    }

    #[test]
    fn failed_load_restores_complete_draft_index_and_session_lease() {
        let root = temp_root("load_rollback");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let draft = root.join("drafts/session_a");
        fs::create_dir_all(draft.join("outputs/artifacts/stage_00")).unwrap();
        fs::write(draft.join("outputs/artifacts/stage_00/save.txt"), "save-a").unwrap();
        let save_a = service
            .create_save("Save A", &sample_state("Save A", false))
            .unwrap();
        fs::write(draft.join("outputs/artifacts/stage_00/save.txt"), "save-b").unwrap();
        let save_b = service
            .create_save("Save B", &sample_state("Save B", true))
            .unwrap();
        let lease_path = draft.join(".desktop_session_lock");
        fs::write(&lease_path, "lease-must-not-move").unwrap();
        let _lease_handle = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lease_path)
            .unwrap();
        let draft_before = comparable_file_map(&draft);
        let index_before = fs::read(root.join("saves/save_index.json")).unwrap();

        let result = service.load_save_with_hook(&save_a.manifest.save_id, |phase| {
            if phase == "after_draft_meta" {
                Err(AdmError::new("injected load failure"))
            } else {
                Ok(())
            }
        });

        assert!(result.is_err());
        assert_eq!(comparable_file_map(&draft), draft_before);
        assert_eq!(
            fs::read(root.join("saves/save_index.json")).unwrap(),
            index_before
        );
        assert_eq!(
            fs::read_to_string(&lease_path).unwrap(),
            "lease-must-not-move"
        );
        assert_archive_lock_available(&service, &save_a.manifest.save_id);
        assert!(
            root.join("saves")
                .join(&save_b.manifest.save_id)
                .join(".archive_lock")
                .is_file()
        );
        drop(_lease_handle);
        cleanup(root);
    }

    #[test]
    fn failed_blank_save_restores_complete_old_draft_and_archive_link() {
        let root = temp_root("blank_rollback");
        let service = SaveService::with_pid(&root, "session_a", std::process::id()).unwrap();
        let draft = root.join("drafts/session_a");
        fs::create_dir_all(draft.join("source_artifacts/user_notes")).unwrap();
        fs::create_dir_all(draft.join("source_artifacts/devflow_generated")).unwrap();
        fs::create_dir_all(draft.join("outputs/artifacts/stage_00")).unwrap();
        fs::write(draft.join("source_artifacts/user_notes/idea.md"), "user").unwrap();
        fs::write(
            draft.join("source_artifacts/devflow_generated/design.md"),
            "generated",
        )
        .unwrap();
        fs::write(
            draft.join("outputs/artifacts/stage_00/result.json"),
            "old-output",
        )
        .unwrap();
        let current = service
            .create_save("Current", &sample_state("Current", true))
            .unwrap();
        let lease_path = draft.join(".desktop_session_lock");
        fs::write(&lease_path, "stable-lease").unwrap();
        let _lease_handle = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lease_path)
            .unwrap();
        let draft_before = comparable_file_map(&draft);

        let result = service.create_blank_save_from_state_with_hook(
            "Will Fail",
            &sample_state("Retained", false),
            |phase| {
                if phase == "after_manifest" {
                    Err(AdmError::new("injected blank failure"))
                } else {
                    Ok(())
                }
            },
        );

        assert!(result.is_err());
        assert_eq!(comparable_file_map(&draft), draft_before);
        assert_eq!(fs::read_to_string(&lease_path).unwrap(), "stable-lease");
        let index = service.list_saves().unwrap();
        assert_eq!(index.saves.len(), 1);
        assert_eq!(
            index.current_save_id.as_deref(),
            Some(current.manifest.save_id.as_str())
        );
        assert!(
            root.join("saves")
                .join(&current.manifest.save_id)
                .join(".archive_lock")
                .is_file()
        );
        drop(_lease_handle);
        cleanup(root);
    }

    #[test]
    fn live_transaction_is_not_recovered_then_runtime_recovery_restores_full_before_image() {
        let root = temp_root("runtime_recovery");
        let crashed = SaveService::with_pid(&root, "crashed", std::process::id()).unwrap();
        let observer = SaveService::with_pid(&root, "observer", std::process::id()).unwrap();
        let project_path = crashed.project_root.path().to_path_buf();
        let target = project_path.join("drafts/crashed");
        let source = project_path.join("transaction_source");
        fs::create_dir_all(target.join("workspace")).unwrap();
        fs::create_dir_all(source.join("workspace")).unwrap();
        fs::write(target.join("workspace/value.txt"), "old").unwrap();
        fs::write(source.join("workspace/value.txt"), "new").unwrap();
        let index_path = project_path.join("saves/save_index.json");
        write_json_value(
            &index_path,
            &serde_json::to_value(SaveIndex::default()).unwrap(),
        )
        .unwrap();
        let staging = prepare_selected_paths(&source, &target, &["workspace"], &[]).unwrap();
        let before_image = TransactionBeforeImage {
            files: capture_transaction_files(&crashed.project_root, &[&index_path]).unwrap(),
            ..Default::default()
        };
        let live_guard = crashed.begin_transaction().unwrap();
        let swap = swap_staged_selected_paths_journaled(
            &crashed.project_root,
            "crashed",
            "test_runtime",
            &staging,
            &target,
            &["workspace"],
            &[],
            before_image,
        )
        .unwrap();
        let mut half_committed = SaveIndex::default();
        half_committed.current_save_id = Some("half_committed".to_string());
        write_json_value(&index_path, &serde_json::to_value(&half_committed).unwrap()).unwrap();
        std::mem::forget(swap);

        let while_live = SaveService::with_pid(&root, "third", std::process::id()).unwrap();
        assert_eq!(
            fs::read_to_string(target.join("workspace/value.txt")).unwrap(),
            "new"
        );
        assert!(transaction_journals(&project_path).iter().any(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("crashed_")
        }));
        drop(while_live);
        let ready = Arc::new(std::sync::Barrier::new(2));
        let worker_ready = Arc::clone(&ready);
        let observer_worker = std::thread::spawn(move || {
            let transaction = observer
                .begin_transaction_with_before_global_hook(|| {
                    worker_ready.wait();
                })
                .unwrap();
            let created = observer
                .create_save("Observer", &sample_state("Observer", false))
                .unwrap();
            drop(transaction);
            created
        });
        ready.wait();
        assert_eq!(
            fs::read_to_string(target.join("workspace/value.txt")).unwrap(),
            "new"
        );
        drop(live_guard);
        drop(crashed);

        let created = observer_worker.join().unwrap();
        assert_eq!(
            fs::read_to_string(target.join("workspace/value.txt")).unwrap(),
            "old"
        );
        assert!(
            !created
                .index
                .saves
                .iter()
                .any(|entry| entry.save_id == "half_committed")
        );
        assert!(!transaction_journals(&project_path).iter().any(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("crashed_")
        }));
        cleanup(root);
    }

    #[test]
    fn committed_transaction_residual_is_finalized_without_rollback() {
        let root = temp_root("committed_recovery");
        let crashed = SaveService::with_pid(&root, "committed", std::process::id()).unwrap();
        let project_path = crashed.project_root.path().to_path_buf();
        let target = project_path.join("drafts/committed");
        let source = project_path.join("committed_source");
        fs::create_dir_all(target.join("workspace")).unwrap();
        fs::create_dir_all(source.join("workspace")).unwrap();
        fs::write(target.join("workspace/value.txt"), "old").unwrap();
        fs::write(source.join("workspace/value.txt"), "new").unwrap();
        let metadata_path = project_path.join("saves/committed_test.json");
        fs::create_dir_all(metadata_path.parent().unwrap()).unwrap();
        fs::write(&metadata_path, "old-metadata").unwrap();
        let staging = prepare_selected_paths(&source, &target, &["workspace"], &[]).unwrap();
        let before_image = TransactionBeforeImage {
            files: capture_transaction_files(&crashed.project_root, &[&metadata_path]).unwrap(),
            ..Default::default()
        };
        let live_guard = crashed.begin_transaction().unwrap();
        let swap = swap_staged_selected_paths_journaled(
            &crashed.project_root,
            "committed",
            "test_committed",
            &staging,
            &target,
            &["workspace"],
            &[],
            before_image,
        )
        .unwrap();
        fs::write(&metadata_path, "new-metadata").unwrap();
        swap.mark_committed().unwrap();
        std::mem::forget(swap);
        drop(live_guard);
        drop(crashed);

        let _restarted = SaveService::with_pid(&root, "observer", std::process::id()).unwrap();
        assert_eq!(
            fs::read_to_string(target.join("workspace/value.txt")).unwrap(),
            "new"
        );
        assert_eq!(fs::read_to_string(metadata_path).unwrap(), "new-metadata");
        assert!(!transaction_journals(&project_path).iter().any(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("committed_")
        }));
        cleanup(root);
    }

    #[test]
    fn uncommitted_rename_residual_restores_manifest_and_index_on_startup() {
        let root = temp_root("rename_residual");
        let crashed = SaveService::with_pid(&root, "crashed", std::process::id()).unwrap();
        let created = crashed
            .create_save("Original", &sample_state("Original", false))
            .unwrap();
        let save_id = created.manifest.save_id;
        let manifest_path = crashed
            .project_root
            .resolve_relative(&format!("saves/{save_id}/manifest.json"))
            .unwrap();
        let index_path = crashed
            .project_root
            .resolve_relative("saves/save_index.json")
            .unwrap();
        let manifest_before = fs::read(&manifest_path).unwrap();
        let index_before = fs::read(&index_path).unwrap();
        let transaction_guard = crashed.begin_transaction().unwrap();
        crashed.acquire_index_lock().unwrap();
        let before_image = TransactionBeforeImage {
            files: capture_transaction_files(&crashed.project_root, &[&manifest_path, &index_path])
                .unwrap(),
            archive_ids: vec![save_id.clone()],
            ..Default::default()
        };
        let journal = crashed
            .create_metadata_transaction_journal("rename", before_image)
            .unwrap();
        let mut manifest = crashed.read_archive_manifest(&save_id).unwrap();
        manifest.display_name = "Half Renamed".to_string();
        crashed
            .archive_repo(&save_id)
            .unwrap()
            .manifest()
            .unwrap()
            .write(&manifest)
            .unwrap();
        let mut index = crashed.load_index_unlocked().unwrap();
        upsert_index_entry(&mut index, index_entry_from_manifest(&manifest));
        crashed.write_index_unlocked(index).unwrap();
        std::mem::forget(journal);
        drop(transaction_guard);
        drop(crashed);

        let restarted = SaveService::with_pid(&root, "observer", std::process::id()).unwrap();
        assert_eq!(fs::read(manifest_path).unwrap(), manifest_before);
        assert_eq!(fs::read(index_path).unwrap(), index_before);
        assert_eq!(
            restarted.get_save(&save_id).unwrap().unwrap().display_name,
            "Original"
        );
        cleanup(root);
    }

    #[test]
    fn committed_delete_residual_finishes_tombstone_cleanup_on_startup() {
        let root = temp_root("delete_residual");
        let crashed = SaveService::with_pid(&root, "crashed", std::process::id()).unwrap();
        let created = crashed
            .create_save("Delete Me", &sample_state("Delete Me", true))
            .unwrap();
        let save_id = created.manifest.save_id;
        let save_dir = crashed
            .project_root
            .resolve_relative(&format!("saves/{save_id}"))
            .unwrap();
        let index_path = crashed
            .project_root
            .resolve_relative("saves/save_index.json")
            .unwrap();
        let draft = crashed.draft_repo().unwrap();
        let meta_path = draft.draft_meta().unwrap().path().to_path_buf();
        let tombstone = unique_sibling_path(&save_dir, "delete-tombstone").unwrap();
        let tombstone_staging = unique_sibling_path(&save_dir, "delete-staging").unwrap();
        let transaction_guard = crashed.begin_transaction().unwrap();
        crashed.acquire_index_lock().unwrap();
        let before_image = TransactionBeforeImage {
            files: capture_transaction_files(&crashed.project_root, &[&index_path, &meta_path])
                .unwrap(),
            directories: vec![TransactionDirectoryBeforeImage {
                target: transaction_root_relative(&crashed.project_root, &save_dir).unwrap(),
                staging: transaction_root_relative(&crashed.project_root, &tombstone_staging)
                    .unwrap(),
                backup: transaction_root_relative(&crashed.project_root, &tombstone).unwrap(),
                had_target: true,
            }],
            archive_ids: vec![save_id.clone()],
            ..Default::default()
        };
        let journal = crashed
            .create_metadata_transaction_journal("delete", before_image)
            .unwrap();
        fs::rename(&save_dir, &tombstone).unwrap();
        let mut index = crashed.load_index_unlocked().unwrap();
        index.saves.retain(|entry| entry.save_id != save_id);
        index.current_save_id = None;
        crashed.write_index_unlocked(index).unwrap();
        let mut detached = crashed
            .new_draft_meta(
                None,
                WorkspaceState::UnsavedCopyOfDeletedSave,
                Some(format!("saves/{save_id}")),
            )
            .unwrap();
        detached.origin_deleted_save_id = Some(save_id.clone());
        draft.draft_meta().unwrap().write(&detached).unwrap();
        journal.mark_committed().unwrap();
        std::mem::forget(journal);
        drop(transaction_guard);
        drop(crashed);

        let restarted = SaveService::with_pid(&root, "observer", std::process::id()).unwrap();
        assert!(!save_dir.exists());
        assert!(!tombstone.exists());
        assert!(restarted.get_save(&save_id).unwrap().is_none());
        let index = restarted.list_saves().unwrap();
        assert!(!index.saves.iter().any(|entry| entry.save_id == save_id));
        let restored_meta: DraftMeta =
            serde_json::from_str(&fs::read_to_string(meta_path).unwrap()).unwrap();
        assert_eq!(
            restored_meta.origin_deleted_save_id.as_deref(),
            Some(save_id.as_str())
        );
        cleanup(root);
    }

    #[test]
    fn concurrent_archive_and_index_lock_attempts_have_single_winner() {
        let root = temp_root("lock_barrier");
        let owner = SaveService::with_pid(&root, "owner", std::process::id()).unwrap();
        let created = owner
            .create_save("Lock Target", &sample_state("Lock Target", false))
            .unwrap();
        owner.release_current_lock().unwrap();
        let archive_results = concurrent_lock_attempts(
            SaveService::with_pid(&root, "archive_a", std::process::id()).unwrap(),
            SaveService::with_pid(&root, "archive_b", std::process::id()).unwrap(),
            Some(created.manifest.save_id.clone()),
        );
        assert_eq!(archive_results.into_iter().filter(|won| *won).count(), 1);

        let index_results = concurrent_lock_attempts(
            SaveService::with_pid(&root, "index_a", std::process::id()).unwrap(),
            SaveService::with_pid(&root, "index_b", std::process::id()).unwrap(),
            None,
        );
        assert_eq!(index_results.into_iter().filter(|won| *won).count(), 1);
        cleanup(root);
    }

    #[test]
    fn global_transaction_contention_returns_save_locked_without_unbounded_wait() {
        let root = temp_root("global_lock_timeout");
        let owner = SaveService::with_pid(&root, "owner", std::process::id()).unwrap();
        let contender = SaveService::with_pid(&root, "contender", std::process::id()).unwrap();
        let owner_guard = owner.begin_transaction().unwrap();

        let started = std::time::Instant::now();
        let error = contender.begin_transaction().unwrap_err();
        let elapsed = started.elapsed();

        assert!(error.message().contains("save index is locked"));
        assert!(elapsed < std::time::Duration::from_secs(2));
        drop(owner_guard);
        cleanup(root);
    }

    fn concurrent_lock_attempts(
        first: SaveService,
        second: SaveService,
        archive_id: Option<String>,
    ) -> [bool; 2] {
        let start = Arc::new(std::sync::Barrier::new(3));
        let finish = Arc::new(std::sync::Barrier::new(3));
        let spawn_attempt = |service: SaveService| {
            let start = Arc::clone(&start);
            let finish = Arc::clone(&finish);
            let archive_id = archive_id.clone();
            std::thread::spawn(move || {
                start.wait();
                let won = match &archive_id {
                    Some(save_id) => service.acquire_archive_lock(save_id).is_ok(),
                    None => service.acquire_index_lock().is_ok(),
                };
                finish.wait();
                if won {
                    match &archive_id {
                        Some(save_id) => service.release_archive_lock(save_id).unwrap(),
                        None => service.release_index_lock().unwrap(),
                    }
                }
                won
            })
        };
        let first = spawn_attempt(first);
        let second = spawn_attempt(second);
        start.wait();
        finish.wait();
        [first.join().unwrap(), second.join().unwrap()]
    }

    fn transaction_journals(root: &Path) -> Vec<PathBuf> {
        fs::read_dir(root.join(TRANSACTION_DIR))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect()
    }

    fn comparable_file_map(root: &Path) -> Vec<(String, u64, String)> {
        build_file_map(root, 0, String::new())
            .unwrap()
            .files
            .into_iter()
            .map(|entry| (entry.workspace_path, entry.size_bytes, entry.sha256))
            .collect()
    }

    fn assert_archive_lock_available(service: &SaveService, save_id: &str) {
        let path = service.archive_os_lock_path(save_id).unwrap();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        FileExt::try_lock_exclusive(&file).unwrap();
        FileExt::unlock(&file).unwrap();
    }

    fn sample_state(project_name: &str, checked: bool) -> ProjectState {
        let mut state = ProjectState::empty();
        state.project_name = project_name.to_string();
        let mut node = NodeState::default();
        node.checklist.insert("core_loop".to_string(), checked);
        state.nodes.insert("mechanics".to_string(), node);
        state
    }

    fn write_draft_runtime_fixture(draft: &Path, label: &str, status: &str, log: &str) {
        let artifact = draft.join("outputs/artifacts/stage_00/result.json");
        let run_state = draft.join("outputs/runtime_control/run_state.json");
        let run_log = draft.join("outputs/run_logs/pipeline.jsonl");
        let run_context = draft.join("runtime/run_context.json");
        let pipeline_state = draft.join("source_artifacts/pipeline_state.md");
        for path in [
            &artifact,
            &run_state,
            &run_log,
            &run_context,
            &pipeline_state,
        ] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
        }
        fs::write(&artifact, format!(r#"{{"save":"{label}"}}"#)).unwrap();
        fs::write(&run_state, format!(r#"{{"status":"{status}"}}"#)).unwrap();
        fs::write(&run_log, format!("{log}\n")).unwrap();
        fs::write(&run_context, format!(r#"{{"run":"{label}"}}"#)).unwrap();
        fs::write(&pipeline_state, format!("pipeline-{label}\n")).unwrap();
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm_new_save_{label}_{}",
            new_stable_id("root").unwrap()
        ))
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
