use adm_new_application::execution_objects::{
    ExecutionObjectStoreService, execution_object_store_path,
};
use adm_new_contracts::Diagnostic;
use adm_new_contracts::project::ProjectState;
use adm_new_contracts::save::SaveIndex;
use adm_new_tauri_commands::save::{
    self, CreateBlankSaveRequest, CreateSaveRequest, DeleteSaveRequest, LoadSaveRequest,
    LoadedSaveView, OpenSaveDirectoryRequest, OpenSaveDirectoryView, RenameSaveRequest,
    SaveProjectRequest, SaveReportView, SaveSwitchBehavior,
};
use adm_new_tauri_commands::{CommandAdapterResult, command_error, command_failure};
use tauri::State;

use crate::runtime::{AppRuntime, RuntimeState, with_runtime};

const EXECUTION_OBJECT_OWNERSHIP_PENDING_RELATIVE_PATH: &str =
    "runtime/execution_object_ownership_pending.json";

struct PendingExecutionObjectOwnership {
    source_owner: String,
    target_save_id: Option<String>,
}

#[tauri::command]
pub fn list_saves(state: State<'_, AppRuntime>) -> CommandAdapterResult<SaveIndex> {
    with_runtime(&state, |runtime| save::list_saves(&runtime.save))
}

#[tauri::command]
pub fn create_save(
    state: State<'_, AppRuntime>,
    mut request: CreateSaveRequest,
) -> CommandAdapterResult<SaveReportView> {
    if state.pipeline_is_running() {
        return save_conflict("create a save");
    }
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return save_conflict("create a save");
        }
        create_save_in_runtime(runtime, &mut request)
    })
}

#[tauri::command]
pub fn create_blank_save(
    state: State<'_, AppRuntime>,
    mut request: CreateBlankSaveRequest,
) -> CommandAdapterResult<SaveReportView> {
    if state.pipeline_is_running() {
        return save_conflict("create a new project save");
    }
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return save_conflict("create a new project save");
        }
        if let Err(error) = settle_pending_execution_object_ownership(
            runtime,
            "settle_execution_object_ownership_before_create_blank",
        ) {
            return command_failure(command_error(
                "execution_object_ownership_recovery_failed",
                error.to_string(),
            ));
        }
        request.state = runtime.project_state.clone();
        let mut response = save::create_blank_save(&runtime.save, request);
        append_save_report_warnings(runtime, &mut response, "save.create_blank");
        if response.ok {
            runtime.pipeline_state = crate::runtime::empty_pipeline_state();
            if let Err(error) = runtime.persist_pipeline_state() {
                runtime.write_log(
                    adm_new_contracts::log::LogLevel::Error,
                    "save.create_blank",
                    &format!("blank save committed but pipeline state refresh failed: {error}"),
                );
                response.diagnostics.push(Diagnostic {
                    level: "WARNING".to_string(),
                    message: format!(
                        "blank save committed; pipeline state file refresh failed: {error}"
                    ),
                });
            }
            runtime.reload_logs();
            runtime.reset_save_scoped_services();
        }
        persist_after_save(runtime, response, "save.create_blank")
    })
}

#[tauri::command]
pub fn save_project(
    state: State<'_, AppRuntime>,
    mut request: SaveProjectRequest,
) -> CommandAdapterResult<SaveReportView> {
    if state.pipeline_is_running() {
        return save_conflict("save the project");
    }
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return save_conflict("save the project");
        }
        save_project_in_runtime(runtime, &mut request)
    })
}

#[tauri::command]
pub fn load_save(
    state: State<'_, AppRuntime>,
    request: LoadSaveRequest,
) -> CommandAdapterResult<LoadedSaveView> {
    if state.pipeline_is_running() {
        return save_conflict("load another save");
    }
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return save_conflict("load another save");
        }
        load_save_into_runtime(runtime, request)
    })
}

#[tauri::command]
pub fn rename_save(
    state: State<'_, AppRuntime>,
    request: RenameSaveRequest,
) -> CommandAdapterResult<SaveIndex> {
    with_runtime(&state, |runtime| save::rename_save(&runtime.save, request))
}

#[tauri::command]
pub fn delete_save(
    state: State<'_, AppRuntime>,
    request: DeleteSaveRequest,
) -> CommandAdapterResult<SaveIndex> {
    if state.pipeline_is_running() {
        return save_conflict("delete a save");
    }
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return save_conflict("delete a save");
        }
        delete_save_in_runtime(runtime, request)
    })
}

#[tauri::command]
pub fn get_autosave_state(
    state: State<'_, AppRuntime>,
) -> CommandAdapterResult<Option<ProjectState>> {
    with_runtime(&state, |runtime| save::get_autosave_state(&runtime.save))
}

#[tauri::command]
pub fn open_save_directory(
    state: State<'_, AppRuntime>,
    request: OpenSaveDirectoryRequest,
) -> CommandAdapterResult<OpenSaveDirectoryView> {
    with_runtime(&state, |runtime| {
        let path = match save_directory_path(&runtime.data_root, &request.save_id) {
            Ok(path) => path,
            Err(error) => return command_failure(command_error("save_path_invalid", error)),
        };
        if !path.is_dir() {
            return command_failure(command_error(
                "save_not_found",
                format!("save directory does not exist: {}", path.display()),
            ));
        }
        let path = match canonical_save_directory(&runtime.data_root, &path) {
            Ok(path) => path,
            Err(error) => return command_failure(command_error("save_path_invalid", error)),
        };
        if let Err(error) = open_directory(&path) {
            return command_failure(command_error("open_save_directory_failed", error));
        }
        adm_new_tauri_commands::command_success(OpenSaveDirectoryView {
            path: path.display().to_string(),
        })
    })
}

fn canonical_save_directory(
    root: &std::path::Path,
    target: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let data_root = std::fs::canonicalize(root)
        .map_err(|error| format!("failed to resolve application data root: {error}"))?;
    let saves_root = std::fs::canonicalize(root.join("saves"))
        .map_err(|error| format!("failed to resolve save root: {error}"))?;
    if saves_root == data_root || !saves_root.starts_with(&data_root) {
        return Err(format!(
            "save root escapes the application data root: {}",
            saves_root.display()
        ));
    }
    let target = std::fs::canonicalize(target)
        .map_err(|error| format!("failed to resolve save directory: {error}"))?;
    if target == saves_root || !target.starts_with(&saves_root) {
        return Err(format!(
            "save directory escapes the save root: {}",
            target.display()
        ));
    }
    Ok(target)
}

fn persist_after_save<T>(
    runtime: &mut crate::runtime::RuntimeState,
    response: CommandAdapterResult<T>,
    context: &str,
) -> CommandAdapterResult<T> {
    if response.ok {
        runtime.write_log(
            adm_new_contracts::log::LogLevel::Info,
            context,
            "save operation completed",
        );
    }
    response
}

fn create_save_in_runtime(
    runtime: &mut RuntimeState,
    request: &mut CreateSaveRequest,
) -> CommandAdapterResult<SaveReportView> {
    if let Err(error) = settle_pending_execution_object_ownership(
        runtime,
        "settle_execution_object_ownership_before_create_save",
    ) {
        return command_failure(command_error(
            "execution_object_ownership_recovery_failed",
            error.to_string(),
        ));
    }
    let source_owner = match current_execution_object_owner(runtime) {
        Ok(source_owner) => source_owner,
        Err(error) => {
            return command_failure(command_error(
                "execution_object_owner_source_unavailable",
                error.to_string(),
            ));
        }
    };
    if let Err(error) = validate_execution_object_owner_before_create(runtime, &source_owner) {
        return command_failure(command_error(
            "execution_object_ownership_source_mismatch",
            error.to_string(),
        ));
    }
    let has_execution_object_store =
        execution_object_store_path(runtime.save.draft_root()).is_file();
    if has_execution_object_store
        && let Err(error) = write_preparing_execution_object_ownership(runtime, &source_owner)
    {
        return command_failure(command_error(
            "execution_object_ownership_intent_write_failed",
            error.to_string(),
        ));
    }
    request.state = runtime.project_state.clone();
    let mut response = save::create_save(&runtime.save, request.clone());
    if response.ok {
        migrate_execution_object_owner_after_create(runtime, &mut response, &source_owner);
    } else if has_execution_object_store
        && let Err(error) = clear_pending_execution_object_ownership(runtime)
    {
        let warning = format!(
            "EXECUTION_OBJECT_OWNERSHIP_INTENT_CLEANUP_FAILED: 存档创建失败，且预提交所有权标记未能清理；下次操作将先安全恢复或清理。{error}"
        );
        runtime.write_log(
            adm_new_contracts::log::LogLevel::Warning,
            "save.create",
            &warning,
        );
        response.diagnostics.push(Diagnostic {
            level: "WARNING".to_string(),
            message: warning,
        });
    }
    append_save_report_warnings(runtime, &mut response, "save.create");
    persist_after_save(runtime, response, "save.create")
}

fn save_project_in_runtime(
    runtime: &mut RuntimeState,
    request: &mut SaveProjectRequest,
) -> CommandAdapterResult<SaveReportView> {
    let ownership_recovery_pending = match recover_pending_execution_object_ownership(runtime) {
        Ok(pending) => pending,
        Err(error) => {
            return command_failure(command_error(
                "execution_object_ownership_recovery_failed",
                error.to_string(),
            ));
        }
    };
    request.state = runtime.project_state.clone();
    let mut response = save::save_project(&runtime.save, request.clone());
    if response.ok
        && ownership_recovery_pending
        && let Err(error) = clear_pending_execution_object_ownership(runtime)
    {
        push_save_warning(
            &mut response,
            format!(
                "EXECUTION_OBJECT_OWNERSHIP_RECOVERY_MARKER_CLEANUP_FAILED: 执行对象归属与归档已修复，但待迁移标记清理失败；下次保存将安全重试清理。{error}"
            ),
        );
    }
    append_save_report_warnings(runtime, &mut response, "save.sync");
    persist_after_save(runtime, response, "save.sync")
}

fn delete_save_in_runtime(
    runtime: &mut RuntimeState,
    request: DeleteSaveRequest,
) -> CommandAdapterResult<SaveIndex> {
    if let Err(error) = settle_pending_execution_object_ownership(
        runtime,
        "settle_execution_object_ownership_before_delete_save",
    ) {
        return command_failure(command_error(
            "execution_object_ownership_recovery_failed",
            error.to_string(),
        ));
    }
    let current_save_id = match runtime.save.current_draft_save_id() {
        Ok(current) => current,
        Err(error) => {
            return command_failure(command_error(
                "current_save_identity_unavailable",
                error.to_string(),
            ));
        }
    };
    let deleting_current = current_save_id.as_deref() == Some(request.save_id.as_str());
    if deleting_current && execution_object_store_path(runtime.save.draft_root()).is_file() {
        return command_failure(command_error(
            "execution_object_owner_rehome_required",
            "the current save owns execution objects; create or load another save before deleting it",
        ));
    }
    save::delete_save(&runtime.save, request)
}

fn current_execution_object_owner(runtime: &RuntimeState) -> adm_new_foundation::AdmResult<String> {
    if let Some(save_id) = runtime.save.current_draft_save_id()? {
        return Ok(save_id);
    }
    let session_id = runtime.runtime_config.paths().session_id.trim();
    Ok(format!(
        "draft-owner:{}",
        if session_id.is_empty() {
            "detached"
        } else {
            session_id
        }
    ))
}

fn validate_execution_object_owner_before_create(
    runtime: &RuntimeState,
    source_owner: &str,
) -> adm_new_foundation::AdmResult<()> {
    let store_path = execution_object_store_path(runtime.save.draft_root());
    if !store_path.is_file() {
        return Ok(());
    }
    let store = ExecutionObjectStoreService::new(&store_path, None)?;
    match store.document().save_id.as_deref() {
        Some(owner) if owner == source_owner => Ok(()),
        Some(owner) => Err(adm_new_foundation::AdmError::new(format!(
            "execution object owner {owner:?} does not match the allowed save-as source {source_owner:?}"
        ))),
        None => Err(adm_new_foundation::AdmError::new(
            "execution object store has no owner; run the explicit ownership migration before creating a formal save",
        )),
    }
}

fn pending_execution_object_ownership_path(runtime: &RuntimeState) -> std::path::PathBuf {
    runtime
        .save
        .draft_root()
        .join(EXECUTION_OBJECT_OWNERSHIP_PENDING_RELATIVE_PATH)
}

fn write_execution_object_ownership_marker(
    runtime: &RuntimeState,
    source_owner: &str,
    target_save_id: Option<&str>,
) -> adm_new_foundation::AdmResult<()> {
    let document = serde_json::json!({
        "schema_version": 1,
        "phase": if target_save_id.is_some() { "committed" } else { "preparing" },
        "source_owner": source_owner,
        "target_save_id": target_save_id,
        "reason": "create_save_ownership_transfer",
    });
    let text = serde_json::to_string_pretty(&document).map_err(|error| {
        adm_new_foundation::AdmError::new(format!(
            "failed to serialize execution-object ownership marker: {error}"
        ))
    })?;
    adm_new_foundation::write_text_atomic(
        &pending_execution_object_ownership_path(runtime),
        &(text + "\n"),
    )
    .map(|_| ())
}

fn write_preparing_execution_object_ownership(
    runtime: &RuntimeState,
    source_owner: &str,
) -> adm_new_foundation::AdmResult<()> {
    write_execution_object_ownership_marker(runtime, source_owner, None)
}

fn write_pending_execution_object_ownership(
    runtime: &RuntimeState,
    source_owner: &str,
    target_save_id: &str,
) -> adm_new_foundation::AdmResult<()> {
    write_execution_object_ownership_marker(runtime, source_owner, Some(target_save_id))
}

fn read_pending_execution_object_ownership(
    runtime: &RuntimeState,
) -> adm_new_foundation::AdmResult<Option<PendingExecutionObjectOwnership>> {
    let path = pending_execution_object_ownership_path(runtime);
    if !path.is_file() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path)?;
    let marker: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
        adm_new_foundation::AdmError::new(format!(
            "pending execution-object ownership marker is invalid: {error}"
        ))
    })?;
    if marker
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        return Err(adm_new_foundation::AdmError::new(
            "pending execution-object ownership marker has an unsupported schema version",
        ));
    }
    let source_owner = marker
        .get("source_owner")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            adm_new_foundation::AdmError::new(
                "pending execution-object ownership marker has no source owner",
            )
        })?;
    let target_save_id = marker
        .get("target_save_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(target_save_id) = target_save_id {
        let portable_target = adm_new_foundation::sanitize_identifier(target_save_id)?;
        if portable_target != target_save_id {
            return Err(adm_new_foundation::AdmError::new(
                "pending execution-object ownership target is not portable",
            ));
        }
    }
    Ok(Some(PendingExecutionObjectOwnership {
        source_owner: source_owner.to_string(),
        target_save_id: target_save_id.map(str::to_string),
    }))
}

pub(crate) fn recover_pending_execution_object_ownership(
    runtime: &RuntimeState,
) -> adm_new_foundation::AdmResult<bool> {
    let Some(marker) = read_pending_execution_object_ownership(runtime)? else {
        return Ok(false);
    };
    let current_save_id = runtime.save.current_draft_save_id()?;
    let target_save_id = match marker.target_save_id {
        Some(target_save_id) => {
            if current_save_id.as_deref() != Some(target_save_id.as_str()) {
                return Err(adm_new_foundation::AdmError::new(
                    "pending execution-object ownership target does not match the current formal save",
                ));
            }
            target_save_id
        }
        None => match current_save_id {
            None => {
                clear_pending_execution_object_ownership(runtime)?;
                return Ok(false);
            }
            Some(current) if current == marker.source_owner => {
                clear_pending_execution_object_ownership(runtime)?;
                return Ok(false);
            }
            Some(current) => current,
        },
    };
    if target_save_id == marker.source_owner {
        return Err(adm_new_foundation::AdmError::new(
            "pending execution-object ownership target must differ from its source owner",
        ));
    }
    let store_path = execution_object_store_path(runtime.save.draft_root());
    if !store_path.is_file() {
        return Err(adm_new_foundation::AdmError::new(
            "pending execution-object ownership recovery cannot find the draft store",
        ));
    }
    let mut store = ExecutionObjectStoreService::new(&store_path, None)?;
    match store.document().save_id.as_deref() {
        Some(owner) if owner == target_save_id => Ok(true),
        Some(owner) if owner == marker.source_owner => {
            store.transfer_ownership_to_save(
                &target_save_id,
                Some(&marker.source_owner),
                "recover_create_save_ownership_transfer",
            )?;
            Ok(true)
        }
        owner => Err(adm_new_foundation::AdmError::new(format!(
            "pending execution-object ownership recovery expected source {:?} or target {target_save_id:?}, found {owner:?}",
            marker.source_owner,
        ))),
    }
}

pub(crate) fn settle_pending_execution_object_ownership(
    runtime: &RuntimeState,
    reason: &str,
) -> adm_new_foundation::AdmResult<bool> {
    let pending = recover_pending_execution_object_ownership(runtime)?;
    if !pending {
        return Ok(false);
    }
    runtime
        .save
        .sync_current_save(&runtime.project_state, reason)?;
    clear_pending_execution_object_ownership(runtime)?;
    Ok(true)
}

pub(crate) fn clear_pending_execution_object_ownership(
    runtime: &RuntimeState,
) -> adm_new_foundation::AdmResult<()> {
    let path = pending_execution_object_ownership_path(runtime);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// A create/save-as operation is the only place where execution-object ownership may
/// move to a new formal save. The formal save is already committed when this helper
/// runs, so failures remain a committed success with an explicit recovery warning;
/// returning a generic failure here would invite a retry that creates a duplicate save.
fn migrate_execution_object_owner_after_create(
    runtime: &mut RuntimeState,
    response: &mut CommandAdapterResult<SaveReportView>,
    source_owner: &str,
) {
    let Some(new_save_id) = response
        .data
        .as_ref()
        .map(|report| report.manifest.save_id.clone())
    else {
        return;
    };
    let store_path = execution_object_store_path(runtime.save.draft_root());
    if !store_path.is_file() {
        return;
    }
    if let Err(error) =
        write_pending_execution_object_ownership(runtime, source_owner, &new_save_id)
    {
        push_save_warning(
            response,
            format!(
                "EXECUTION_OBJECT_OWNERSHIP_RECOVERY_MARKER_UPDATE_FAILED: 存档已创建，但所有权补偿标记未能写入目标 ID；预提交意图仍会让下次保存或流水线启动从当前存档安全推断目标。{error}"
            ),
        );
    }
    let mut store = match ExecutionObjectStoreService::new(&store_path, None) {
        Ok(store) => store,
        Err(error) => {
            push_save_warning(
                response,
                format!(
                    "EXECUTION_OBJECT_STORE_UNREADABLE: 存档已创建，但执行对象存储无法读取；后续执行前请修复或重新保存。{error}"
                ),
            );
            return;
        }
    };
    if store.document().save_id.as_deref() != Some(new_save_id.as_str())
        && let Err(error) = store.transfer_ownership_to_save(
            &new_save_id,
            Some(source_owner),
            "create_save_ownership_transfer",
        )
    {
        push_save_warning(
            response,
            format!(
                "EXECUTION_OBJECT_OWNERSHIP_TRANSFER_FAILED: 存档已创建，但执行对象归属迁移失败；不会静默改写原归属。{error}"
            ),
        );
        return;
    }

    match runtime.save.sync_current_save(
        &runtime.project_state,
        "execution_object_ownership_transfer",
    ) {
        Ok(report) => {
            let previous_warnings = response
                .data
                .as_ref()
                .map(|report| report.warnings.clone())
                .unwrap_or_default();
            let mut synchronized = SaveReportView::from(report);
            for warning in previous_warnings {
                if !synchronized.warnings.contains(&warning) {
                    synchronized.warnings.push(warning);
                }
            }
            response.data = Some(synchronized);
            if let Err(error) = clear_pending_execution_object_ownership(runtime) {
                push_save_warning(
                    response,
                    format!(
                        "EXECUTION_OBJECT_OWNERSHIP_RECOVERY_MARKER_CLEANUP_FAILED: 执行对象归属已同步，但待迁移标记清理失败；下次保存将安全重试清理。{error}"
                    ),
                );
            }
        }
        Err(error) => push_save_warning(
            response,
            format!(
                "EXECUTION_OBJECT_OWNERSHIP_ARCHIVE_SYNC_FAILED: 执行对象已迁移到新存档，但归档二次同步失败；请立即再次保存。{error}"
            ),
        ),
    }
}

fn push_save_warning(response: &mut CommandAdapterResult<SaveReportView>, warning: String) {
    if let Some(report) = response.data.as_mut()
        && !report.warnings.contains(&warning)
    {
        report.warnings.push(warning);
    }
}

fn save_conflict<T>(operation: &str) -> CommandAdapterResult<T> {
    command_failure(command_error(
        "pipeline_save_conflict",
        format!("stop the running pipeline before attempting to {operation}"),
    ))
}

fn load_save_into_runtime(
    runtime: &mut RuntimeState,
    request: LoadSaveRequest,
) -> CommandAdapterResult<LoadedSaveView> {
    if let Err(error) = settle_pending_execution_object_ownership(
        runtime,
        "settle_execution_object_ownership_before_load_save",
    ) {
        return command_failure(command_error(
            "execution_object_ownership_recovery_failed",
            error.to_string(),
        ));
    }
    let index = match runtime.save.list_saves() {
        Ok(index) => index,
        Err(error) => {
            return command_failure(command_error("save_index_read_failed", error.to_string()));
        }
    };
    if request.switch_behavior == SaveSwitchBehavior::SaveCurrent {
        if index.current_save_id.is_none() {
            return command_failure(command_error(
                "save_as_required",
                "the current draft has no formal save; save it as a copy before switching",
            ));
        }
        if let Err(error) = runtime
            .save
            .sync_current_save(&runtime.project_state, "before_load_save")
        {
            return command_failure(command_error(
                "save_sync_before_load_failed",
                error.to_string(),
            ));
        }
    }
    let mut response = save::load_save(&runtime.save, request);
    append_loaded_save_warnings(runtime, &mut response, "save.load");
    if response.ok
        && let Some(loaded) = response.data.as_ref()
    {
        runtime.reload_pipeline_state();
        runtime.reload_logs();
        runtime.reset_save_scoped_services();
        runtime.project_state = runtime.design.normalize_project_state(loaded.state.clone());
        runtime.write_log(
            adm_new_contracts::log::LogLevel::Info,
            "save.load",
            "save archive restored into the active workspace",
        );
    }
    response
}

fn append_save_report_warnings(
    runtime: &mut RuntimeState,
    response: &mut CommandAdapterResult<SaveReportView>,
    context: &str,
) {
    let warnings = response
        .data
        .as_ref()
        .map(|report| report.warnings.clone())
        .unwrap_or_default();
    append_warning_diagnostics(runtime, response, warnings, context);
}

fn append_loaded_save_warnings(
    runtime: &mut RuntimeState,
    response: &mut CommandAdapterResult<LoadedSaveView>,
    context: &str,
) {
    let warnings = response
        .data
        .as_ref()
        .map(|loaded| loaded.warnings.clone())
        .unwrap_or_default();
    append_warning_diagnostics(runtime, response, warnings, context);
}

fn append_warning_diagnostics<T>(
    runtime: &mut RuntimeState,
    response: &mut CommandAdapterResult<T>,
    warnings: Vec<String>,
    context: &str,
) {
    for warning in warnings {
        runtime.write_log(adm_new_contracts::log::LogLevel::Warning, context, &warning);
        if response
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == warning)
        {
            continue;
        }
        response.diagnostics.push(Diagnostic {
            level: "WARNING".to_string(),
            message: warning,
        });
    }
}

fn save_directory_path(
    root: &std::path::Path,
    save_id: &str,
) -> Result<std::path::PathBuf, String> {
    let normalized =
        adm_new_foundation::sanitize_identifier(save_id).map_err(|error| error.to_string())?;
    if normalized != save_id {
        return Err(format!("save id is not portable: {save_id}"));
    }
    Ok(root.join("saves").join(normalized))
}

fn open_directory(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = std::process::Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut command = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = std::process::Command::new("xdg-open");

    command
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to open {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use adm_new_contracts::log::LogLevel;

    use super::*;

    #[test]
    fn save_directory_path_rejects_path_escape() {
        let root = std::path::Path::new("C:/safe-root");
        assert!(save_directory_path(root, "save_valid").is_ok());
        assert!(save_directory_path(root, "../outside").is_err());
        assert!(save_directory_path(root, "nested/save").is_err());
    }

    #[test]
    fn canonical_save_directory_rejects_targets_outside_save_root() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-save-directory-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let valid = root.join("saves").join("save_valid");
        let outside = root.join("outside");
        fs::create_dir_all(&valid).unwrap();
        fs::create_dir_all(&outside).unwrap();

        assert_eq!(
            canonical_save_directory(&root, &valid).unwrap(),
            std::fs::canonicalize(&valid).unwrap()
        );
        assert!(canonical_save_directory(&root, &outside).is_err());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn switching_saves_syncs_current_draft_and_reloads_save_scoped_runtime() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-save-switch-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();

            runtime.clear_persisted_logs().unwrap();
            runtime.project_state.project_name = "Save A".to_string();
            runtime.pipeline_state.status = "save_a_original".to_string();
            runtime.persist_project_state("test.a").unwrap();
            runtime.persist_pipeline_state().unwrap();
            let save_a = runtime
                .save
                .create_save("Save A", &runtime.project_state)
                .unwrap()
                .manifest
                .save_id;

            runtime.clear_persisted_logs().unwrap();
            runtime.project_state.project_name = "Save B".to_string();
            runtime.pipeline_state.status = "save_b".to_string();
            runtime.write_log(LogLevel::Info, "test", "only-save-b");
            runtime.persist_project_state("test.b").unwrap();
            runtime.persist_pipeline_state().unwrap();
            let save_b = runtime
                .save
                .create_save("Save B", &runtime.project_state)
                .unwrap()
                .manifest
                .save_id;

            let loaded_a = load_save_into_runtime(
                &mut runtime,
                LoadSaveRequest {
                    save_id: save_a.clone(),
                    switch_behavior: SaveSwitchBehavior::SaveCurrent,
                },
            );
            assert!(loaded_a.ok, "{loaded_a:?}");
            runtime.clear_persisted_logs().unwrap();
            runtime.project_state.project_name = "Save A unsaved draft".to_string();
            runtime.pipeline_state.status = "save_a_unsaved".to_string();
            runtime.write_log(LogLevel::Warning, "test", "only-save-a-unsaved");
            runtime.persist_project_state("test.a.unsaved").unwrap();
            runtime.persist_pipeline_state().unwrap();

            let loaded_b = load_save_into_runtime(
                &mut runtime,
                LoadSaveRequest {
                    save_id: save_b,
                    switch_behavior: SaveSwitchBehavior::SaveCurrent,
                },
            );
            assert!(loaded_b.ok, "{loaded_b:?}");
            assert_eq!(runtime.project_state.project_name, "Save B");
            assert_eq!(runtime.pipeline_state.status, "save_b");
            assert!(
                runtime
                    .logs
                    .latest(100)
                    .iter()
                    .all(|entry| entry.message != "only-save-a-unsaved")
            );

            let restored_a = load_save_into_runtime(
                &mut runtime,
                LoadSaveRequest {
                    save_id: save_a,
                    switch_behavior: SaveSwitchBehavior::SaveCurrent,
                },
            );
            assert!(restored_a.ok, "{restored_a:?}");
            assert_eq!(runtime.project_state.project_name, "Save A unsaved draft");
            assert_eq!(runtime.pipeline_state.status, "save_a_unsaved");
            assert!(
                runtime
                    .logs
                    .latest(100)
                    .iter()
                    .any(|entry| entry.message == "only-save-a-unsaved")
            );
            runtime.shutdown().unwrap();
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_and_switch_requires_a_formal_save_for_detached_drafts() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-save-switch-detached-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            runtime.project_state.project_name = "Load Target".to_string();
            runtime.persist_project_state("test.target").unwrap();
            let target = runtime
                .save
                .create_save("Load Target", &runtime.project_state)
                .unwrap()
                .manifest
                .save_id;

            runtime.project_state.project_name = "Detached Draft".to_string();
            runtime.persist_project_state("test.detached").unwrap();
            runtime
                .save
                .recover_to_unsaved_state(&runtime.project_state)
                .unwrap();

            let rejected = load_save_into_runtime(
                &mut runtime,
                LoadSaveRequest {
                    save_id: target.clone(),
                    switch_behavior: SaveSwitchBehavior::SaveCurrent,
                },
            );
            assert!(!rejected.ok);
            assert_eq!(rejected.error.unwrap().code, "save_as_required");
            assert_eq!(runtime.project_state.project_name, "Detached Draft");

            let discarded = load_save_into_runtime(
                &mut runtime,
                LoadSaveRequest {
                    save_id: target,
                    switch_behavior: SaveSwitchBehavior::DiscardDraft,
                },
            );
            assert!(discarded.ok, "{discarded:?}");
            assert_eq!(runtime.project_state.project_name, "Load Target");
            runtime.shutdown().unwrap();
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn create_save_transfers_detached_execution_objects_and_archives_new_owner() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-save-eo-owner-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let source_owner = current_execution_object_owner(&runtime).unwrap();
            assert!(source_owner.starts_with("draft-owner:"));
            let store_path = execution_object_store_path(runtime.save.draft_root());
            let mut store =
                ExecutionObjectStoreService::new(&store_path, Some(source_owner.clone())).unwrap();
            store.save().unwrap();

            runtime.project_state.project_name = "Execution Object Save".to_string();
            let mut request = CreateSaveRequest {
                display_name: "Execution Object Save".to_string(),
                state: ProjectState::empty(),
            };
            let response = create_save_in_runtime(&mut runtime, &mut request);
            assert!(response.ok, "{response:?}");

            let report = response.data.as_ref().unwrap();
            let new_save_id = report.manifest.save_id.clone();
            assert_eq!(report.manifest.last_transaction_seq, 2);
            assert!(report.warnings.is_empty(), "{:?}", report.warnings);

            let draft_store = ExecutionObjectStoreService::new(&store_path, None).unwrap();
            assert_eq!(
                draft_store.document().save_id.as_deref(),
                Some(new_save_id.as_str())
            );
            assert_eq!(draft_store.document().ownership_migrations.len(), 1);
            assert_eq!(
                draft_store.document().ownership_migrations[0]
                    .from_save_id
                    .as_deref(),
                Some(source_owner.as_str())
            );

            let archive_store_path = root
                .join("saves")
                .join(&new_save_id)
                .join("workspace/outputs/execution_objects/execution_objects.json");
            let archive_store =
                ExecutionObjectStoreService::new(&archive_store_path, None).unwrap();
            assert_eq!(
                archive_store.document().save_id.as_deref(),
                Some(new_save_id.as_str())
            );
            assert_eq!(archive_store.document().ownership_migrations.len(), 1);
            runtime.shutdown().unwrap();
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn create_save_without_execution_objects_keeps_single_transaction() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-save-without-eo-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let store_path = execution_object_store_path(runtime.save.draft_root());
            assert!(!store_path.exists());
            let mut request = CreateSaveRequest {
                display_name: "No Execution Objects".to_string(),
                state: ProjectState::empty(),
            };

            let response = create_save_in_runtime(&mut runtime, &mut request);

            assert!(response.ok, "{response:?}");
            assert_eq!(
                response
                    .data
                    .as_ref()
                    .unwrap()
                    .manifest
                    .last_transaction_seq,
                1
            );
            assert!(!store_path.exists());
            runtime.shutdown().unwrap();
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn normal_save_recovers_a_create_committed_after_preparing_marker() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-save-eo-recovery-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let source_owner = current_execution_object_owner(&runtime).unwrap();
            let store_path = execution_object_store_path(runtime.save.draft_root());
            let mut store =
                ExecutionObjectStoreService::new(&store_path, Some(source_owner.clone())).unwrap();
            store.save().unwrap();

            // Simulate a crash after formal commit but before the preparing
            // marker could be updated with the generated target save_id.
            write_preparing_execution_object_ownership(&runtime, &source_owner).unwrap();
            let committed = runtime
                .save
                .create_save("Pending Ownership", &runtime.project_state)
                .unwrap();
            let target_save_id = committed.manifest.save_id;
            assert_eq!(
                ExecutionObjectStoreService::new(&store_path, None)
                    .unwrap()
                    .document()
                    .save_id
                    .as_deref(),
                Some(source_owner.as_str())
            );
            let mut request = SaveProjectRequest {
                state: ProjectState::empty(),
                reason: "manual_recovery_save".to_string(),
            };

            let response = save_project_in_runtime(&mut runtime, &mut request);

            assert!(response.ok, "{response:?}");
            assert!(!pending_execution_object_ownership_path(&runtime).exists());
            assert_eq!(
                ExecutionObjectStoreService::new(&store_path, None)
                    .unwrap()
                    .document()
                    .save_id
                    .as_deref(),
                Some(target_save_id.as_str())
            );
            let archive_store = ExecutionObjectStoreService::new(
                root.join("saves")
                    .join(&target_save_id)
                    .join("workspace/outputs/execution_objects/execution_objects.json"),
                None,
            )
            .unwrap();
            assert_eq!(
                archive_store.document().save_id.as_deref(),
                Some(target_save_id.as_str())
            );
            runtime.shutdown().unwrap();
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stale_preparing_marker_without_create_commit_is_cleared_without_transfer() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-save-eo-stale-prepare-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let source_owner = current_execution_object_owner(&runtime).unwrap();
            let store_path = execution_object_store_path(runtime.save.draft_root());
            let mut store =
                ExecutionObjectStoreService::new(&store_path, Some(source_owner.clone())).unwrap();
            store.save().unwrap();
            write_preparing_execution_object_ownership(&runtime, &source_owner).unwrap();

            let settled =
                settle_pending_execution_object_ownership(&runtime, "test_stale_preparing_marker")
                    .unwrap();

            assert!(!settled);
            assert!(!pending_execution_object_ownership_path(&runtime).exists());
            assert_eq!(
                ExecutionObjectStoreService::new(&store_path, None)
                    .unwrap()
                    .document()
                    .save_id
                    .as_deref(),
                Some(source_owner.as_str())
            );
            runtime.shutdown().unwrap();
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn create_save_rejects_an_unrelated_execution_object_owner_before_commit() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-save-eo-owner-mismatch-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let store_path = execution_object_store_path(runtime.save.draft_root());
            let mut store =
                ExecutionObjectStoreService::new(&store_path, Some("save_unrelated".to_string()))
                    .unwrap();
            store.save().unwrap();
            let before = runtime.save.list_saves().unwrap();
            let mut request = CreateSaveRequest {
                display_name: "Must Not Commit".to_string(),
                state: ProjectState::empty(),
            };

            let response = create_save_in_runtime(&mut runtime, &mut request);

            assert!(!response.ok);
            assert_eq!(
                response.error.as_ref().unwrap().code,
                "execution_object_ownership_source_mismatch"
            );
            assert_eq!(runtime.save.list_saves().unwrap(), before);
            assert!(runtime.save.current_draft_save_id().unwrap().is_none());
            runtime.shutdown().unwrap();
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn discard_load_settles_pending_owner_before_switching_current_save() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-save-eo-discard-recovery-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let original = runtime
                .save
                .create_save("Load Target", &runtime.project_state)
                .unwrap();
            let original_save_id = original.manifest.save_id;
            let store_path = execution_object_store_path(runtime.save.draft_root());
            let mut store =
                ExecutionObjectStoreService::new(&store_path, Some(original_save_id.clone()))
                    .unwrap();
            store.save().unwrap();
            runtime
                .save
                .sync_current_save(&runtime.project_state, "seed_execution_objects")
                .unwrap();
            write_preparing_execution_object_ownership(&runtime, &original_save_id).unwrap();
            let pending = runtime
                .save
                .create_save("Pending Current", &runtime.project_state)
                .unwrap();
            let pending_save_id = pending.manifest.save_id;

            let loaded = load_save_into_runtime(
                &mut runtime,
                LoadSaveRequest {
                    save_id: original_save_id.clone(),
                    switch_behavior: SaveSwitchBehavior::DiscardDraft,
                },
            );

            assert!(loaded.ok, "{loaded:?}");
            assert_eq!(
                runtime.save.current_draft_save_id().unwrap().as_deref(),
                Some(original_save_id.as_str())
            );
            assert!(!pending_execution_object_ownership_path(&runtime).exists());
            let pending_archive = ExecutionObjectStoreService::new(
                root.join("saves")
                    .join(&pending_save_id)
                    .join("workspace/outputs/execution_objects/execution_objects.json"),
                None,
            )
            .unwrap();
            assert_eq!(
                pending_archive.document().save_id.as_deref(),
                Some(pending_save_id.as_str())
            );
            runtime.shutdown().unwrap();
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn second_save_as_settles_previous_pending_owner_before_new_transfer() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-save-eo-second-save-as-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let original = runtime
                .save
                .create_save("Original", &runtime.project_state)
                .unwrap();
            let original_save_id = original.manifest.save_id;
            let store_path = execution_object_store_path(runtime.save.draft_root());
            let mut store =
                ExecutionObjectStoreService::new(&store_path, Some(original_save_id.clone()))
                    .unwrap();
            store.save().unwrap();
            runtime
                .save
                .sync_current_save(&runtime.project_state, "seed_execution_objects")
                .unwrap();
            write_preparing_execution_object_ownership(&runtime, &original_save_id).unwrap();
            let first_copy = runtime
                .save
                .create_save("First Copy", &runtime.project_state)
                .unwrap();
            let first_copy_id = first_copy.manifest.save_id;
            let mut request = CreateSaveRequest {
                display_name: "Second Copy".to_string(),
                state: ProjectState::empty(),
            };

            let response = create_save_in_runtime(&mut runtime, &mut request);

            assert!(response.ok, "{response:?}");
            let second_copy_id = response.data.as_ref().unwrap().manifest.save_id.clone();
            let first_archive = ExecutionObjectStoreService::new(
                root.join("saves")
                    .join(&first_copy_id)
                    .join("workspace/outputs/execution_objects/execution_objects.json"),
                None,
            )
            .unwrap();
            assert_eq!(
                first_archive.document().save_id.as_deref(),
                Some(first_copy_id.as_str())
            );
            let second_archive = ExecutionObjectStoreService::new(
                root.join("saves")
                    .join(&second_copy_id)
                    .join("workspace/outputs/execution_objects/execution_objects.json"),
                None,
            )
            .unwrap();
            assert_eq!(
                second_archive.document().save_id.as_deref(),
                Some(second_copy_id.as_str())
            );
            assert_eq!(second_archive.document().ownership_migrations.len(), 2);
            assert!(!pending_execution_object_ownership_path(&runtime).exists());
            runtime.shutdown().unwrap();
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn deleting_current_save_with_execution_objects_requires_rehome() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-delete-current-eo-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let created = runtime
                .save
                .create_save("Owned Save", &runtime.project_state)
                .unwrap();
            let save_id = created.manifest.save_id;
            let store_path = execution_object_store_path(runtime.save.draft_root());
            let mut store =
                ExecutionObjectStoreService::new(&store_path, Some(save_id.clone())).unwrap();
            store.save().unwrap();

            let response = delete_save_in_runtime(
                &mut runtime,
                DeleteSaveRequest {
                    save_id: save_id.clone(),
                },
            );

            assert!(!response.ok);
            assert_eq!(
                response.error.as_ref().unwrap().code,
                "execution_object_owner_rehome_required"
            );
            assert!(
                runtime
                    .save
                    .list_saves()
                    .unwrap()
                    .saves
                    .iter()
                    .any(|entry| entry.save_id == save_id)
            );
            assert_eq!(
                runtime.save.current_draft_save_id().unwrap().as_deref(),
                Some(save_id.as_str())
            );
            runtime.shutdown().unwrap();
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn linked_save_as_transfers_execution_objects_from_old_save_to_new_save() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-save-eo-linked-owner-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            runtime.project_state.project_name = "Original Save".to_string();
            let original = runtime
                .save
                .create_save("Original Save", &runtime.project_state)
                .unwrap();
            let old_save_id = original.manifest.save_id;
            let store_path = execution_object_store_path(runtime.save.draft_root());
            let mut store =
                ExecutionObjectStoreService::new(&store_path, Some(old_save_id.clone())).unwrap();
            store.save().unwrap();
            runtime
                .save
                .sync_current_save(&runtime.project_state, "seed_execution_objects")
                .unwrap();

            let mut request = CreateSaveRequest {
                display_name: "Save As Copy".to_string(),
                state: ProjectState::empty(),
            };
            let response = create_save_in_runtime(&mut runtime, &mut request);

            assert!(response.ok, "{response:?}");
            let new_save_id = response.data.as_ref().unwrap().manifest.save_id.clone();
            assert_ne!(new_save_id, old_save_id);
            let draft_store = ExecutionObjectStoreService::new(&store_path, None).unwrap();
            assert_eq!(
                draft_store.document().save_id.as_deref(),
                Some(new_save_id.as_str())
            );
            assert_eq!(
                draft_store.document().ownership_migrations[0]
                    .from_save_id
                    .as_deref(),
                Some(old_save_id.as_str())
            );
            let old_archive = ExecutionObjectStoreService::new(
                root.join("saves")
                    .join(&old_save_id)
                    .join("workspace/outputs/execution_objects/execution_objects.json"),
                None,
            )
            .unwrap();
            assert_eq!(
                old_archive.document().save_id.as_deref(),
                Some(old_save_id.as_str())
            );
            let new_archive = ExecutionObjectStoreService::new(
                root.join("saves")
                    .join(&new_save_id)
                    .join("workspace/outputs/execution_objects/execution_objects.json"),
                None,
            )
            .unwrap();
            assert_eq!(
                new_archive.document().save_id.as_deref(),
                Some(new_save_id.as_str())
            );
            runtime.shutdown().unwrap();
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }
}
