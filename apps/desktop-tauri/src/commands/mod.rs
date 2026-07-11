pub mod ai;
pub mod config;
pub mod design;
pub mod pipeline;
pub mod save;
pub mod utility;

pub use ai::*;
pub use config::*;
pub use design::*;
pub use pipeline::*;
pub use save::*;
pub use utility::*;

#[tauri::command]
pub fn get_shell_state(
    state: tauri::State<'_, crate::runtime::AppRuntime>,
) -> adm_new_tauri_commands::CommandAdapterResult<adm_new_tauri_commands::shell::ShellState> {
    crate::runtime::with_runtime(&state, |runtime| {
        adm_new_tauri_commands::command_success(shell_state(runtime))
    })
}

fn shell_state(
    runtime: &crate::runtime::RuntimeState,
) -> adm_new_tauri_commands::shell::ShellState {
    let mut shell = adm_new_tauri_commands::shell::ShellState::default();
    shell.ui_language = runtime.ui_language;
    let passed = runtime
        .pipeline_state
        .stages
        .values()
        .filter(|stage| {
            matches!(
                stage.status,
                adm_new_contracts::pipeline::StageStatus::Success
                    | adm_new_contracts::pipeline::StageStatus::Skipped
                    | adm_new_contracts::pipeline::StageStatus::CompletedWithReview
            )
        })
        .count();
    shell.progress.passed = u32::try_from(passed).unwrap_or(15).min(15);
    shell.system_status = format!("系统: 流水线 {}", runtime.pipeline_state.status);
    match runtime.ai_config.load_or_default().and_then(|config| {
        let spec = runtime.ai_config.completion_adapter_spec(&config)?;
        let entry = runtime.ai_config.active_completion_entry(&config)?;
        Ok((spec, entry))
    }) {
        Ok((spec, entry)) => {
            let has_model = entry
                .extra_json
                .get("model")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|model| !model.trim().is_empty());
            let complete = spec.adapter_kind != "openai_compatible"
                || (spec.has_api_key && !spec.api_url.trim().is_empty() && has_model);
            shell.ai_status.label = if complete {
                format!("AI: {} ({})", spec.entry_id, spec.adapter_kind)
            } else {
                format!("AI: {} 配置不完整", spec.entry_id)
            };
            shell.ai_status.ok = complete;
        }
        Err(_) => {
            shell.ai_status.label = "AI: 未配置".to_string();
            shell.ai_status.ok = false;
        }
    }
    shell
}

#[cfg(test)]
mod tests {
    use std::fs;

    use adm_new_tauri_commands::shell::UiLanguage;

    use super::*;

    #[test]
    fn shell_state_returns_the_language_captured_by_runtime() {
        let root = std::env::temp_dir().join(format!(
            "adm-newrust-shell-language-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ));
        let app =
            crate::runtime::AppRuntime::new_with_ui_language(&root, UiLanguage::EnUs).unwrap();
        let runtime = app.lock().unwrap();

        assert_eq!(shell_state(&runtime).ui_language, UiLanguage::EnUs);

        drop(runtime);
        let _ = app.shutdown_once();
        drop(app);
        let _ = fs::remove_dir_all(root);
    }
}
