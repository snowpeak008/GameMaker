use adm_new_contracts::log::{LogEntry, LogLevel};
use adm_new_contracts::patch::PatchRecord;
use adm_new_contracts::sdk::SdkSpec;
use adm_new_tauri_commands::logs::{self, ListLatestLogsRequest, ReadLogEntriesRequest};
use adm_new_tauri_commands::package::{
    self, PackageCurrentProjectRequest, PackageRunResultView, PackageView,
};
use adm_new_tauri_commands::patch::{
    self, AnalyzePatchRequest, ListPatchesRequest, ReadPatchRequest, UpdatePatchStatusRequest,
};
use adm_new_tauri_commands::sdk::{
    self, AddSdkRequest, ExtractSdkSpecRequest, UpdateSdkReviewStatusRequest,
};
use adm_new_tauri_commands::{
    CommandAdapterResult, command_error, command_failure, command_success,
};
use tauri::State;

use crate::runtime::{AppRuntime, RuntimeState, with_runtime};

#[tauri::command]
pub fn list_latest_logs(
    state: State<'_, AppRuntime>,
    request: ListLatestLogsRequest,
) -> CommandAdapterResult<Vec<LogEntry>> {
    with_runtime(&state, |runtime| {
        logs::list_latest_logs(&runtime.logs, request)
    })
}

#[tauri::command]
pub fn read_log_entries(
    state: State<'_, AppRuntime>,
    request: ReadLogEntriesRequest,
) -> CommandAdapterResult<Vec<LogEntry>> {
    with_runtime(&state, |runtime| {
        logs::read_log_entries(&runtime.logs, request)
    })
}

#[tauri::command]
pub fn export_log_jsonl(state: State<'_, AppRuntime>) -> CommandAdapterResult<String> {
    with_runtime(&state, |runtime| logs::export_log_jsonl(&runtime.logs))
}

#[tauri::command]
pub fn clear_logs(state: State<'_, AppRuntime>) -> CommandAdapterResult<Vec<LogEntry>> {
    with_runtime(&state, |runtime| match runtime.clear_persisted_logs() {
        Ok(()) => command_success(Vec::new()),
        Err(error) => command_failure(command_error("log_clear_failed", error.to_string())),
    })
}

#[tauri::command]
pub fn analyze_patch_request(
    state: State<'_, AppRuntime>,
    request: AnalyzePatchRequest,
) -> CommandAdapterResult<PatchRecord> {
    with_runtime(&state, |runtime| {
        let response = patch::analyze_patch_request(&mut runtime.patch, request);
        persist_patch_response(runtime, response, "patch.analyze")
    })
}

#[tauri::command]
pub fn list_patches(
    state: State<'_, AppRuntime>,
    request: ListPatchesRequest,
) -> CommandAdapterResult<Vec<PatchRecord>> {
    with_runtime(&state, |runtime| {
        patch::list_patches(&runtime.patch, request)
    })
}

#[tauri::command]
pub fn read_patch(
    state: State<'_, AppRuntime>,
    request: ReadPatchRequest,
) -> CommandAdapterResult<PatchRecord> {
    with_runtime(&state, |runtime| patch::read_patch(&runtime.patch, request))
}

#[tauri::command]
pub fn update_patch_status(
    state: State<'_, AppRuntime>,
    request: UpdatePatchStatusRequest,
) -> CommandAdapterResult<PatchRecord> {
    with_runtime(&state, |runtime| {
        let response = patch::update_patch_status(&mut runtime.patch, request);
        persist_patch_response(runtime, response, "patch.status")
    })
}

#[tauri::command]
pub fn list_sdks(state: State<'_, AppRuntime>) -> CommandAdapterResult<Vec<SdkSpec>> {
    with_runtime(&state, |runtime| sdk::list_sdks(&runtime.sdk))
}

#[tauri::command]
pub fn add_sdk(
    state: State<'_, AppRuntime>,
    request: AddSdkRequest,
) -> CommandAdapterResult<SdkSpec> {
    with_runtime(&state, |runtime| {
        let response = sdk::add_sdk(&mut runtime.sdk, request);
        persist_sdk_response(runtime, response, "sdk.add")
    })
}

#[tauri::command]
pub fn update_sdk_review_status(
    state: State<'_, AppRuntime>,
    request: UpdateSdkReviewStatusRequest,
) -> CommandAdapterResult<SdkSpec> {
    with_runtime(&state, |runtime| {
        let response = sdk::update_sdk_review_status(&mut runtime.sdk, request);
        persist_sdk_response(runtime, response, "sdk.status")
    })
}

#[tauri::command]
pub fn get_approved_sdk_context(state: State<'_, AppRuntime>) -> CommandAdapterResult<String> {
    with_runtime(&state, |runtime| {
        sdk::get_approved_sdk_context(&runtime.sdk)
    })
}

#[tauri::command]
pub fn extract_sdk_spec(
    state: State<'_, AppRuntime>,
    request: ExtractSdkSpecRequest,
) -> CommandAdapterResult<SdkSpec> {
    with_runtime(&state, |runtime| {
        let response = sdk::extract_sdk_spec(&mut runtime.sdk, request);
        persist_sdk_response(runtime, response, "sdk.extract")
    })
}

#[tauri::command]
pub fn load_package_view(state: State<'_, AppRuntime>) -> CommandAdapterResult<PackageView> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return command_success(PackageView {
                step14_status: "running".to_string(),
                can_package: false,
                last_result: None,
                blocking_issues: vec![
                    "PIPELINE-RUNNING: wait for the pipeline to finish before package validation"
                        .to_string(),
                ],
            });
        }
        let result = package::package_current_project(
            &runtime.packaging,
            package_request_from_artifacts(runtime),
        )
        .data;
        package::load_package_view(&runtime.packaging, result)
    })
}

#[tauri::command]
pub fn package_current_project(
    state: State<'_, AppRuntime>,
    request: Option<PackageCurrentProjectRequest>,
) -> CommandAdapterResult<PackageRunResultView> {
    if state.pipeline_is_running() {
        return command_failure(command_error(
            "pipeline_package_conflict",
            "stop the running pipeline before validating a package",
        ));
    }
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return command_failure(command_error(
                "pipeline_package_conflict",
                "stop the running pipeline before validating a package",
            ));
        }
        let request = request.unwrap_or_else(|| package_request_from_artifacts(runtime));
        let response = package::package_current_project(&runtime.packaging, request);
        if response.ok
            && let Some(result) = response.data.as_ref()
        {
            if let Err(error) = runtime.persist_package_result(result) {
                return command_failure(command_error(
                    "package_result_write_failed",
                    error.to_string(),
                ));
            }
            runtime.write_log(
                if result.validation_report.status.as_str() == "success" {
                    LogLevel::Info
                } else {
                    LogLevel::Warning
                },
                "package",
                &format!(
                    "package validation finished: {} ({} blockers)",
                    result.validation_report.status.as_str(),
                    result.validation_report.blocking_issues.len()
                ),
            );
        }
        response
    })
}

fn package_request_from_artifacts(runtime: &RuntimeState) -> PackageCurrentProjectRequest {
    let root = runtime.pipeline_executor.artifact_root();
    let mut integration = read_artifact(root, 14, "integration_validation_report.json");
    if !integration.is_object() {
        integration = serde_json::json!({"status": "pending"});
    }
    let development = read_artifact(root, 11, "dev_execution_report.json");
    let art_handoff = read_artifact(root, 12, "art_handoff_manifest.json");
    let scene = read_artifact(root, 13, "scene_assembly_report.json");
    let build_settings = read_artifact(root, 13, "build_settings_update_report.json");
    let playmode = read_artifact(root, 13, "playmode_smoke_test_result.json");
    let actual_project_file_audit = read_artifact(root, 14, "actual_project_file_audit.json");
    let unity_validation_summary = read_artifact(root, 14, "unity_validation_summary.json");
    let unity_valid = unity_validation_summary
        .get("valid")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
        && unity_validation_summary
            .get("failed_validation_count")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1)
            == 0;
    integration["checks"] = serde_json::json!({
        "actual_development_succeeded": development.get("status").and_then(serde_json::Value::as_str) == Some("success"),
        "scene_assembly_succeeded": scene.get("status").and_then(serde_json::Value::as_str) == Some("success"),
        "demo_scene_exists": scene.get("demo_scene_exists").and_then(serde_json::Value::as_bool).unwrap_or(false),
        "visible_content_verified": scene.get("visible_content_verified").and_then(serde_json::Value::as_bool).unwrap_or(false),
        "build_settings_contains_demo_scene": build_settings.get("build_settings_updated").and_then(serde_json::Value::as_bool).unwrap_or(false),
        "playmode_smoke_passed": playmode.get("status").and_then(serde_json::Value::as_str) == Some("passed"),
        "unity_batchmode_validation_passed": unity_valid,
        "assets_traced": art_handoff.get("ready_for_step13").and_then(serde_json::Value::as_bool).unwrap_or(false),
        "execution_objects_verified": development.get("verified_execution_objects").and_then(serde_json::Value::as_array).is_some_and(|items| !items.is_empty()),
    });
    PackageCurrentProjectRequest {
        integration,
        actual_project_file_audit,
        unity_validation_summary,
    }
}

fn read_artifact(root: &std::path::Path, stage: u32, name: &str) -> serde_json::Value {
    adm_new_foundation::io::read_json(
        &root.join(format!("stage_{stage:02}")).join(name),
        serde_json::json!({}),
    )
}

fn persist_patch_response(
    runtime: &mut RuntimeState,
    response: CommandAdapterResult<PatchRecord>,
    context: &str,
) -> CommandAdapterResult<PatchRecord> {
    if response.ok {
        if let Err(error) = runtime.persist_patch_records() {
            return command_failure(command_error("patch_store_write_failed", error.to_string()));
        }
        runtime.write_log(LogLevel::Info, context, "patch records persisted");
    }
    response
}

fn persist_sdk_response(
    runtime: &mut RuntimeState,
    response: CommandAdapterResult<SdkSpec>,
    context: &str,
) -> CommandAdapterResult<SdkSpec> {
    if response.ok {
        if let Err(error) = runtime.persist_sdk_specs() {
            return command_failure(command_error("sdk_store_write_failed", error.to_string()));
        }
        runtime.write_log(LogLevel::Info, context, "SDK knowledge persisted");
    }
    response
}

#[cfg(test)]
mod tests {
    use std::fs;

    use adm_new_contracts::package::PackageStatus;

    use super::*;

    #[test]
    fn package_request_uses_pipeline_artifacts_and_preserves_missing_unity_blockers() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-package-view-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app = AppRuntime::new(&root).unwrap();
        {
            let mut runtime = app.lock().unwrap();
            let artifacts = runtime.pipeline_executor.artifact_root();
            write_stage_json(
                artifacts,
                11,
                "dev_execution_report.json",
                serde_json::json!({
                    "status": "success",
                    "verified_execution_objects": ["EO-001"]
                }),
            );
            write_stage_json(
                artifacts,
                12,
                "art_handoff_manifest.json",
                serde_json::json!({"ready_for_step13": true}),
            );
            write_stage_json(
                artifacts,
                13,
                "scene_assembly_report.json",
                serde_json::json!({
                    "status": "success",
                    "demo_scene_exists": true,
                    "visible_content_verified": true
                }),
            );
            write_stage_json(
                artifacts,
                13,
                "build_settings_update_report.json",
                serde_json::json!({"build_settings_updated": true}),
            );
            write_stage_json(
                artifacts,
                13,
                "playmode_smoke_test_result.json",
                serde_json::json!({"status": "passed"}),
            );
            write_stage_json(
                artifacts,
                14,
                "integration_validation_report.json",
                serde_json::json!({"status": "success"}),
            );

            let request = package_request_from_artifacts(&runtime);
            assert_eq!(
                request.integration["checks"]["actual_development_succeeded"],
                serde_json::json!(true)
            );
            assert_eq!(
                request.integration["checks"]["unity_batchmode_validation_passed"],
                serde_json::json!(false)
            );
            let response = package::package_current_project(&runtime.packaging, request);
            assert!(response.ok, "{response:?}");
            let result = response.data.unwrap();
            assert_eq!(result.validation_report.status, PackageStatus::Blocked);
            assert!(
                result
                    .validation_report
                    .blocking_issues
                    .iter()
                    .any(|issue| { issue.id == "PACKAGE-UNITY-VALIDATION-MISSING" })
            );
            assert!(
                result
                    .validation_report
                    .blocking_issues
                    .iter()
                    .any(|issue| { issue.id == "PACKAGE-NO-ACTUAL-PROJECT-CHANGES" })
            );
            runtime.persist_package_result(&result).unwrap();
            assert!(
                runtime
                    .package_output_dir()
                    .join("package_validation_report.json")
                    .is_file()
            );
            runtime.reload_package_result();
            assert_eq!(
                runtime
                    .last_package_result
                    .as_ref()
                    .unwrap()
                    .validation_report
                    .status,
                PackageStatus::Blocked
            );
            runtime.invalidate_package_result().unwrap();
            assert!(runtime.last_package_result.is_none());
            assert!(!runtime.package_output_dir().exists());
        }
        drop(app);
        let _ = fs::remove_dir_all(root);
    }

    fn write_stage_json(root: &std::path::Path, stage: u32, name: &str, value: serde_json::Value) {
        adm_new_foundation::io::write_json(
            &root.join(format!("stage_{stage:02}")).join(name),
            &value,
        )
        .unwrap();
    }
}
