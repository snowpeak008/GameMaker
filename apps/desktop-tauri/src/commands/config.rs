use adm_new_application::{
    AiApiProbeView, AiCliProbeView, AiConfigDescriptorView, AiResolutionView,
    project_environment::{
        EditorSelectionValidation, ProjectEnvironmentInspection, UnityEditorCandidate,
    },
    runtime::{DevelopmentPreflightReport, ProjectRuntimeSettings},
};
use adm_new_contracts::ai::AiConfig;
use adm_new_tauri_commands::config::{
    self, AiConfigResolutionRequest, AiConfigValidationView, CompletionAdapterSpecView,
    DiscoverUnityEditorsRequest, InspectProjectEnvironmentRequest, NativePathSelectionKind,
    NativePathSelectionRequest, NativePathSelectionView, ProjectConfigView,
    ProjectPreflightRequest, RelinkProjectBindingRequest, SaveAiConfigRequest,
    SaveProjectConfigRequest, ValidateProjectEditorRequest,
};
use adm_new_tauri_commands::{
    CommandAdapterResult, command_error, command_failure, command_success,
};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use crate::runtime::{AppRuntime, with_runtime};

#[tauri::command]
pub fn load_ai_config(state: State<'_, AppRuntime>) -> CommandAdapterResult<AiConfig> {
    with_runtime(&state, |runtime| config::load_ai_config(&runtime.ai_config))
}

#[tauri::command]
pub fn save_ai_config(
    state: State<'_, AppRuntime>,
    request: SaveAiConfigRequest,
) -> CommandAdapterResult<AiConfigValidationView> {
    with_runtime(&state, |runtime| {
        config::save_ai_config(&runtime.ai_config, request)
    })
}

#[tauri::command]
pub fn validate_ai_config(
    state: State<'_, AppRuntime>,
    config: AiConfig,
) -> CommandAdapterResult<AiConfigValidationView> {
    with_runtime(&state, |runtime| {
        config::validate_ai_config(&runtime.ai_config, config)
    })
}

#[tauri::command]
pub fn completion_adapter_spec(
    state: State<'_, AppRuntime>,
    config: AiConfig,
) -> CommandAdapterResult<CompletionAdapterSpecView> {
    with_runtime(&state, |runtime| {
        config::completion_adapter_spec(&runtime.ai_config, config)
    })
}

#[tauri::command]
pub fn list_ai_config_descriptors(
    state: State<'_, AppRuntime>,
) -> CommandAdapterResult<Vec<AiConfigDescriptorView>> {
    with_runtime(&state, |runtime| {
        config::list_ai_config_descriptors(&runtime.ai_config)
    })
}

#[tauri::command]
pub fn preview_ai_resolution(
    state: State<'_, AppRuntime>,
    request: AiConfigResolutionRequest,
) -> CommandAdapterResult<AiResolutionView> {
    with_runtime(&state, |runtime| {
        config::preview_ai_resolution(&runtime.ai_config, request)
    })
}

#[tauri::command]
pub async fn probe_ai_cli(
    app: AppHandle,
    request: AiConfigResolutionRequest,
) -> Result<CommandAdapterResult<AiCliProbeView>, String> {
    let state = app.state::<AppRuntime>();
    let service = match state.lock() {
        Ok(runtime) => runtime.ai_config.clone(),
        Err(error) => {
            return Ok(command_failure(command_error(
                "runtime_state_unavailable",
                error.to_string(),
            )));
        }
    };
    drop(state);
    Ok(
        match tauri::async_runtime::spawn_blocking(move || config::probe_ai_cli(&service, request))
            .await
        {
            Ok(response) => response,
            Err(error) => command_failure(command_error(
                "ai_cli_probe_failed",
                format!("AI CLI probe worker failed: {error}"),
            )),
        },
    )
}

#[tauri::command]
pub async fn probe_ai_api(
    app: AppHandle,
    request: AiConfigResolutionRequest,
) -> Result<CommandAdapterResult<AiApiProbeView>, String> {
    let state = app.state::<AppRuntime>();
    let service = match state.lock() {
        Ok(runtime) => runtime.ai_config.clone(),
        Err(error) => {
            return Ok(command_failure(command_error(
                "runtime_state_unavailable",
                error.to_string(),
            )));
        }
    };
    drop(state);
    Ok(
        match tauri::async_runtime::spawn_blocking(move || config::probe_ai_api(&service, request))
            .await
        {
            Ok(response) => response,
            Err(error) => command_failure(command_error(
                "ai_api_probe_failed",
                format!("AI API probe worker failed: {error}"),
            )),
        },
    )
}

#[tauri::command]
pub fn load_project_config(
    state: State<'_, AppRuntime>,
) -> CommandAdapterResult<ProjectRuntimeSettings> {
    with_runtime(&state, |runtime| {
        config::load_project_config(&runtime.runtime_config)
    })
}

#[tauri::command]
pub fn save_project_config(
    state: State<'_, AppRuntime>,
    request: SaveProjectConfigRequest,
) -> CommandAdapterResult<ProjectConfigView> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return command_failure(command_error(
                "pipeline_config_conflict",
                "stop the running pipeline before changing project runtime configuration",
            ));
        }
        config::save_project_config(&runtime.runtime_config, request)
    })
}

#[tauri::command]
pub fn run_project_preflight(
    state: State<'_, AppRuntime>,
    request: ProjectPreflightRequest,
) -> CommandAdapterResult<DevelopmentPreflightReport> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return command_failure(command_error(
                "pipeline_config_conflict",
                "stop the running pipeline before running a project preflight that updates settings",
            ));
        }
        config::run_project_preflight(&runtime.runtime_config, request)
    })
}

#[tauri::command]
pub fn relink_project_binding(
    state: State<'_, AppRuntime>,
    request: RelinkProjectBindingRequest,
) -> CommandAdapterResult<ProjectConfigView> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return command_failure(command_error(
                "pipeline_config_conflict",
                "stop the running pipeline before relinking project paths",
            ));
        }
        config::relink_project_binding(&runtime.runtime_config, request)
    })
}

#[tauri::command]
pub async fn select_native_path(
    app: AppHandle,
    request: NativePathSelectionRequest,
) -> CommandAdapterResult<NativePathSelectionView> {
    let mut picker = app.dialog().file();
    if !request.title.trim().is_empty() {
        picker = picker.set_title(request.title.trim());
    }
    if let Some(directory) = initial_picker_directory(&request) {
        picker = picker.set_directory(directory);
    }
    if request.kind == NativePathSelectionKind::File {
        for filter in &request.filters {
            let extensions = filter
                .extensions
                .iter()
                .map(|extension| extension.trim().trim_start_matches('.'))
                .filter(|extension| !extension.is_empty())
                .collect::<Vec<_>>();
            if !extensions.is_empty() {
                picker = picker.add_filter(filter.name.trim(), &extensions);
            }
        }
    }
    let selected = match request.kind {
        NativePathSelectionKind::Folder => picker.blocking_pick_folder(),
        NativePathSelectionKind::File => picker.blocking_pick_file(),
    };
    let Some(selected) = selected else {
        return command_success(NativePathSelectionView::cancelled());
    };
    let path = match selected.simplified().into_path() {
        Ok(path) => path,
        Err(error) => {
            return command_failure(command_error(
                "native_path_conversion_failed",
                format!("selected path cannot be converted to a local filesystem path: {error}"),
            ));
        }
    };
    let valid_kind = match request.kind {
        NativePathSelectionKind::Folder => path.is_dir(),
        NativePathSelectionKind::File => path.is_file(),
    };
    if !valid_kind {
        return command_failure(command_error(
            "native_path_kind_mismatch",
            "selected path does not match the requested file or folder kind",
        ));
    }
    command_success(NativePathSelectionView::selected(display_native_path(
        &path,
    )))
}

#[tauri::command]
pub fn inspect_project_environment(
    request: InspectProjectEnvironmentRequest,
) -> CommandAdapterResult<ProjectEnvironmentInspection> {
    config::inspect_project_environment(request)
}

#[tauri::command]
pub fn discover_project_unity_editors(
    request: DiscoverUnityEditorsRequest,
) -> CommandAdapterResult<Vec<UnityEditorCandidate>> {
    config::discover_project_unity_editors(request)
}

#[tauri::command]
pub fn validate_project_editor(
    request: ValidateProjectEditorRequest,
) -> CommandAdapterResult<EditorSelectionValidation> {
    config::validate_project_editor(request)
}

fn initial_picker_directory(request: &NativePathSelectionRequest) -> Option<std::path::PathBuf> {
    let current = std::path::PathBuf::from(request.current_path.trim());
    if current.as_os_str().is_empty() {
        return None;
    }
    let candidate = if current.is_dir() {
        current
    } else {
        current.parent()?.to_path_buf()
    };
    candidate.is_dir().then_some(candidate)
}

fn display_native_path(path: &std::path::Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(local) = value.strip_prefix(r"\\?\") {
        local.to_string()
    } else {
        value.into_owned()
    }
}

#[tauri::command]
pub async fn refine_style_prompts(
    app: AppHandle,
    request: Value,
) -> Result<CommandAdapterResult<Value>, String> {
    let state = app.state::<AppRuntime>();
    let runner = match state.lock() {
        Ok(runtime) => runtime.completion_runner(),
        Err(error) => {
            return Ok(command_failure(command_error(
                "runtime_state_unavailable",
                error.to_string(),
            )));
        }
    };
    drop(state);
    let prompt = style_prompt_text(&request);
    let completion =
        tauri::async_runtime::spawn_blocking(move || runner.generate("style_prompt", prompt)).await;
    Ok(match completion {
        Ok(Ok(text)) => command_success(json!({"text": text})),
        Ok(Err(error)) => command_failure(command_error("ai_completion_failed", error.to_string())),
        Err(error) => command_failure(command_error(
            "ai_completion_failed",
            format!("AI completion worker failed: {error}"),
        )),
    })
}

fn style_prompt_text(request: &Value) -> String {
    let messages = request
        .get("messages")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let content = item.get("content")?.as_str()?.trim();
                    if content.is_empty() {
                        None
                    } else {
                        let role = item.get("role").and_then(Value::as_str).unwrap_or("user");
                        Some(format!("[{role}]\n{content}"))
                    }
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        })
        .unwrap_or_default();
    if messages.is_empty() {
        request.to_string()
    } else {
        messages
    }
}
