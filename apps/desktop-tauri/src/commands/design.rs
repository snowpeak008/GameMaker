use adm_new_application::{
    DesignAutosaveReport, DesignTemplateDeleteReport, DesignTemplateListReport,
    DesignTemplateSaveReport, DesignTemplateSelectionReport, DesignWorkbenchView,
    GameplaySystemUpdateRequest,
};
use adm_new_contracts::Diagnostic;
use adm_new_contracts::project::ProjectState;
use adm_new_foundation::AdmResult;
use adm_new_tauri_commands::design::{
    self, AutosaveDesignRequest, DeleteTemplateRequest, DesignExport, DesignNodeUpdateRequest,
    ExportDesignRequest, ListTemplatesRequest, SaveTemplateRequest, TemplateSelectionRequest,
};
use adm_new_tauri_commands::{
    CommandAdapterResult, command_error, command_failure, command_success,
};
use tauri::State;

use crate::runtime::{AppRuntime, RuntimeState, with_runtime};

#[tauri::command]
pub fn load_design_workbench(
    state: State<'_, AppRuntime>,
) -> CommandAdapterResult<DesignWorkbenchView> {
    with_runtime(&state, |runtime| {
        design::load_design_workbench(&runtime.design, &runtime.project_state)
    })
}

#[tauri::command]
pub fn set_project_name(
    state: State<'_, AppRuntime>,
    name: String,
) -> CommandAdapterResult<DesignWorkbenchView> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return design_pipeline_conflict();
        }
        let name = name.trim();
        if name.is_empty() {
            return command_failure(command_error(
                "project_name_empty",
                "project name must not be empty",
            ));
        }
        runtime.project_state.project_name = name.to_string();
        if let Err(error) = runtime.persist_project_state("design.set_project_name") {
            return command_failure(command_error("autosave_failed", error.to_string()));
        }
        command_success(runtime.design.view_model(&runtime.project_state))
    })
}

#[tauri::command]
pub fn update_node(
    state: State<'_, AppRuntime>,
    request: DesignNodeUpdateRequest,
) -> CommandAdapterResult<DesignWorkbenchView> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return design_pipeline_conflict();
        }
        let response = design::update_node(&runtime.design, &mut runtime.project_state, request);
        persist_mutation(runtime, response, "design.update_node")
    })
}

#[tauri::command]
pub fn export_design(
    state: State<'_, AppRuntime>,
    request: ExportDesignRequest,
) -> CommandAdapterResult<DesignExport> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return design_pipeline_conflict();
        }
        runtime.ui_language = request.artifact_locale;
        design::export_design(&runtime.design, &runtime.project_state, request)
    })
}

#[tauri::command]
pub fn autosave_design(
    state: State<'_, AppRuntime>,
    request: AutosaveDesignRequest,
) -> CommandAdapterResult<DesignAutosaveReport> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return design_pipeline_conflict();
        }
        let response = design::autosave_design(&runtime.design, &runtime.project_state, request);
        persist_mutation(runtime, response, "design.autosave")
    })
}

#[tauri::command]
pub fn list_templates(
    state: State<'_, AppRuntime>,
    request: ListTemplatesRequest,
) -> CommandAdapterResult<DesignTemplateListReport> {
    with_runtime(&state, |runtime| {
        let mut response = design::list_templates(&runtime.design, request);
        let warnings = response
            .data
            .as_ref()
            .map(|report| report.warnings.clone())
            .unwrap_or_default();
        append_template_warnings(runtime, &mut response, warnings, "design.template.list");
        response
    })
}

#[tauri::command]
pub fn select_template(
    state: State<'_, AppRuntime>,
    request: TemplateSelectionRequest,
) -> CommandAdapterResult<DesignTemplateSelectionReport> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return design_pipeline_conflict();
        }
        let previous = runtime.project_state.clone();
        let response =
            design::select_template(&runtime.design, &mut runtime.project_state, request);
        let persist_result = if response.ok {
            runtime.persist_project_state("design.select_template")
        } else {
            Ok(())
        };
        finalize_template_selection(
            &mut runtime.project_state,
            previous,
            response,
            persist_result,
        )
    })
}

#[tauri::command]
pub fn save_template(
    state: State<'_, AppRuntime>,
    request: SaveTemplateRequest,
) -> CommandAdapterResult<DesignTemplateSaveReport> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return design_pipeline_conflict();
        }
        design::save_template(&runtime.design, &runtime.project_state, request)
    })
}

#[tauri::command]
pub fn delete_template(
    state: State<'_, AppRuntime>,
    request: DeleteTemplateRequest,
) -> CommandAdapterResult<DesignTemplateDeleteReport> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return design_pipeline_conflict();
        }
        design::delete_template(&runtime.design, request)
    })
}

#[tauri::command]
pub fn update_gameplay_system(
    state: State<'_, AppRuntime>,
    request: GameplaySystemUpdateRequest,
) -> CommandAdapterResult<DesignWorkbenchView> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return design_pipeline_conflict();
        }
        let response =
            design::update_gameplay_system(&runtime.design, &mut runtime.project_state, request);
        persist_mutation(runtime, response, "design.update_gameplay_system")
    })
}

#[tauri::command]
pub fn reset_design(state: State<'_, AppRuntime>) -> CommandAdapterResult<DesignWorkbenchView> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return design_pipeline_conflict();
        }
        let response = design::reset_design(&runtime.design, &mut runtime.project_state);
        persist_mutation(runtime, response, "design.reset")
    })
}

fn design_pipeline_conflict<T>() -> CommandAdapterResult<T> {
    command_failure(command_error(
        "pipeline_design_conflict",
        "stop the running pipeline before changing or exporting the design",
    ))
}

fn persist_mutation<T>(
    runtime: &mut RuntimeState,
    response: CommandAdapterResult<T>,
    context: &str,
) -> CommandAdapterResult<T> {
    if response.ok
        && let Err(error) = runtime.persist_project_state(context)
    {
        return command_failure(command_error("autosave_failed", error.to_string()));
    }
    response
}

fn finalize_template_selection<T>(
    state: &mut ProjectState,
    previous: ProjectState,
    response: CommandAdapterResult<T>,
    persist_result: AdmResult<()>,
) -> CommandAdapterResult<T> {
    if response.ok
        && let Err(error) = persist_result
    {
        *state = previous;
        return command_failure(command_error("autosave_failed", error.to_string()));
    }
    response
}

fn append_template_warnings<T>(
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

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::AdmError;
    use adm_new_tauri_commands::command_success;

    #[test]
    fn failed_template_autosave_rolls_back_the_in_memory_state() {
        let mut previous = ProjectState::empty();
        previous.project_name = "Before".to_string();
        let mut current = previous.clone();
        current.project_name = "Template".to_string();

        let response = finalize_template_selection(
            &mut current,
            previous,
            command_success(()),
            Err(AdmError::new("disk full")),
        );

        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "autosave_failed");
        assert_eq!(current.project_name, "Before");
    }
}
