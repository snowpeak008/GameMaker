use std::collections::BTreeMap;

use adm_new_contracts::ai::{AiInterviewState, AiResponsePayload, AiSchemaMode};
use adm_new_contracts::project::ProjectState;
use adm_new_tauri_commands::CommandAdapterResult;
use adm_new_tauri_commands::ai::{
    self, AiInterviewCommandView, ForceAiOutputRequest, MarkAiInaccurateRequest,
    SaveAiArchiveRequest, SubmitAiTurnRequest,
};
use adm_new_tauri_commands::shell::UiLanguage;
use adm_new_tauri_commands::{command_error, command_failure};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager, State};

use crate::runtime::{AppRuntime, CompletionRunner, RuntimeState, with_runtime};

#[tauri::command]
pub fn load_ai_interview(state: State<'_, AppRuntime>) -> CommandAdapterResult<AiInterviewState> {
    with_runtime(&state, |runtime| {
        ai::load_ai_interview(&runtime.ai_interview, &runtime.project_state)
    })
}

#[tauri::command]
pub async fn submit_ai_turn(
    app: AppHandle,
    mut request: SubmitAiTurnRequest,
) -> Result<CommandAdapterResult<AiInterviewCommandView>, String> {
    if app.state::<AppRuntime>().pipeline_is_running() {
        return Ok(ai_pipeline_conflict());
    }
    let generated_from = if request.payload_json.is_none() {
        let state = app.state::<AppRuntime>();
        let (runner, project_state, prompt) = match state.lock() {
            Ok(runtime) => {
                let project_state = runtime.project_state.clone();
                let prompt = interview_prompt(&project_state, &request, runtime.ui_language);
                (runtime.completion_runner(), project_state, prompt)
            }
            Err(error) => return Ok(runtime_state_failure(error.to_string())),
        };
        drop(state);

        let text = match run_completion(runner, "ai_interview", prompt).await {
            Ok(text) => text,
            Err(message) => return Ok(record_generated_failure(&app, &project_state, &message)),
        };
        let Some(payload) = extract_json_payload(&text) else {
            return Ok(record_generated_failure(
                &app,
                &project_state,
                "AI completion did not return one JSON object",
            ));
        };
        request.payload_json = Some(payload);
        Some(project_state)
    } else {
        None
    };

    let state = app.state::<AppRuntime>();
    let response = match state.lock() {
        Ok(mut runtime) => {
            if state.pipeline_is_running() {
                return Ok(ai_pipeline_conflict());
            }
            if generated_from
                .as_ref()
                .is_some_and(|snapshot| snapshot != &runtime.project_state)
            {
                return Ok(stale_ai_context_failure());
            }
            if !request.user_message.trim().is_empty() {
                let turn = runtime.project_state.ai_interview.session_turn_count + 1;
                runtime
                    .project_state
                    .ai_interview
                    .messages
                    .push(serde_json::json!({
                        "role": "user",
                        "content": request.user_message.trim(),
                        "turn": turn,
                    }));
            }
            let service = runtime.ai_interview.clone();
            let response = ai::submit_ai_turn(&service, &mut runtime.project_state, request);
            persist_ai_mutation(&mut runtime, response, "ai.submit_turn")
        }
        Err(error) => runtime_state_failure(error.to_string()),
    };
    Ok(response)
}

fn interview_prompt(
    project_state: &ProjectState,
    request: &SubmitAiTurnRequest,
    language: UiLanguage,
) -> String {
    let state = serde_json::to_string_pretty(project_state).unwrap_or_else(|_| "{}".to_string());
    let (language_instruction, assistant_message, question_text) = match language {
        UiLanguage::ZhCn => (
            "Write all user-facing prose in Simplified Chinese. Keep protocol IDs and technical product names unchanged, and do not mix in English explanatory prose.",
            "请提出一个简洁的后续问题",
            "同一个后续问题",
        ),
        UiLanguage::EnUs => (
            "Write all user-facing prose in English. Keep protocol IDs and technical product names unchanged, and do not include Chinese prose.",
            "Ask one concise next question",
            "The same next question",
        ),
    };
    let response_example = json!({
        "schemaVersion": "1.0",
        "mode": "question_group",
        "assistantMessage": assistant_message,
        "questionGroup": {"questionText": question_text},
        "inferences": [],
        "optionDifferences": [],
        "errors": []
    });
    format!(
        "You are the AutoDesignMaker game-design interview assistant. {language_instruction}\n\
Return only one JSON object, without markdown fences. It must use camelCase fields and this exact minimum shape:\n\
{response_example}\n\
The requested schema mode is {}. For a normal turn, keep mode=question_group. Ask one decision-focused question grounded in unfinished nodes.\n\
User message:\n{}\n\nCurrent project state:\n{}",
        request.schema_mode.as_str(),
        request.user_message.trim(),
        state
    )
}

fn extract_json_payload(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let candidates = [
        trimmed,
        trimmed
            .strip_prefix("```json")
            .and_then(|value| value.strip_suffix("```"))
            .map(str::trim)
            .unwrap_or(""),
        trimmed
            .strip_prefix("```")
            .and_then(|value| value.strip_suffix("```"))
            .map(str::trim)
            .unwrap_or(""),
    ];
    for candidate in candidates {
        if !candidate.is_empty() && serde_json::from_str::<Value>(candidate).is_ok() {
            return Some(candidate.to_string());
        }
    }
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    let candidate = &trimmed[start..=end];
    serde_json::from_str::<Value>(candidate)
        .ok()
        .map(|_| candidate.to_string())
}

fn record_ai_failure(
    runtime: &mut RuntimeState,
    message: &str,
) -> CommandAdapterResult<AiInterviewCommandView> {
    runtime.project_state.ai_interview.status = "error".to_string();
    runtime.project_state.ai_interview.backend_stage = "error".to_string();
    runtime.project_state.ai_interview.last_error = message.to_string();
    let _ = runtime.persist_project_state("ai.error");
    command_failure(command_error("ai_completion_failed", message))
}

#[tauri::command]
pub async fn force_ai_output(
    app: AppHandle,
    mut request: ForceAiOutputRequest,
) -> Result<CommandAdapterResult<AiInterviewCommandView>, String> {
    if app.state::<AppRuntime>().pipeline_is_running() {
        return Ok(ai_pipeline_conflict());
    }
    let generated_from = if request.payload.is_none() {
        if request.schema_mode != AiSchemaMode::FullOutput {
            return Ok(command_failure(command_error(
                "invalid_ai_request",
                "provider-generated force output requires schema_mode=full_output",
            )));
        }
        let state = app.state::<AppRuntime>();
        let (runner, project_state, prompt) = match state.lock() {
            Ok(runtime) => {
                let project_state = runtime.project_state.clone();
                let prompt = force_output_prompt(&project_state, runtime.ui_language);
                (runtime.completion_runner(), project_state, prompt)
            }
            Err(error) => return Ok(runtime_state_failure(error.to_string())),
        };
        drop(state);

        let text = match run_completion(runner, "ai_full_output", prompt).await {
            Ok(text) => text,
            Err(message) => return Ok(record_generated_failure(&app, &project_state, &message)),
        };
        let Some(payload_json) = extract_json_payload(&text) else {
            return Ok(record_generated_failure(
                &app,
                &project_state,
                "AI completion did not return one JSON object",
            ));
        };
        let payload = match serde_json::from_str::<AiResponsePayload>(&payload_json) {
            Ok(payload) => payload,
            Err(error) => {
                return Ok(record_generated_failure(
                    &app,
                    &project_state,
                    &format!("AI completion returned an invalid output payload: {error}"),
                ));
            }
        };
        request.payload = Some(payload);
        Some(project_state)
    } else {
        None
    };

    let state = app.state::<AppRuntime>();
    let response = match state.lock() {
        Ok(mut runtime) => {
            if state.pipeline_is_running() {
                return Ok(ai_pipeline_conflict());
            }
            if generated_from
                .as_ref()
                .is_some_and(|snapshot| snapshot != &runtime.project_state)
            {
                return Ok(stale_ai_context_failure());
            }
            let service = runtime.ai_interview.clone();
            let response = ai::force_ai_output(&service, &mut runtime.project_state, request);
            persist_ai_mutation(&mut runtime, response, "ai.force_output")
        }
        Err(error) => runtime_state_failure(error.to_string()),
    };
    Ok(response)
}

fn force_output_prompt(project_state: &ProjectState, language: UiLanguage) -> String {
    let confidence_nodes = project_state
        .nodes
        .keys()
        .map(|node_id| (node_id.clone(), json!(0.0)))
        .chain(std::iter::once(("gameplaySystems".to_string(), json!(0.0))))
        .collect::<BTreeMap<_, _>>();
    let (language_instruction, assistant_message) = match language {
        UiLanguage::ZhCn => (
            "Write all user-facing design prose in Simplified Chinese. Keep protocol IDs and technical product names unchanged, and do not mix in English explanatory prose.",
            "已生成完整项目设计。",
        ),
        UiLanguage::EnUs => (
            "Write all user-facing design prose in English. Keep protocol IDs and technical product names unchanged, and do not include Chinese prose.",
            "The complete project design has been generated.",
        ),
    };
    let contract_template = json!({
        "schemaVersion": "1.0",
        "mode": "full_project_output",
        "assistantMessage": assistant_message,
        "fullProjectOutput": {
            "projectState": project_state,
            "confidenceMap": {"nodes": confidence_nodes}
        },
        "inferences": [],
        "optionDifferences": [],
        "errors": []
    });
    let template = serde_json::to_string_pretty(&contract_template)
        .unwrap_or_else(|_| contract_template.to_string());
    format!(
        "You are the AutoDesignMaker game-design output generator. {language_instruction}\n\
Return only one valid JSON object, without markdown fences or commentary. Preserve every field in fullProjectOutput.projectState and every existing node id. Fill unfinished design decisions, notes, checklists, entities, gameplay systems, and project profile without deleting existing user decisions.\n\
The mode must be full_project_output. For confidenceMap.nodes, keep every listed key and replace each numeric value with a 0.0-1.0 confidence. Use 0.75 or higher only where the generated content is sufficiently grounded.\n\
Use this complete contract template as the input and output shape:\n{template}"
    )
}

async fn run_completion(
    runner: CompletionRunner,
    task_prefix: &'static str,
    prompt: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || runner.generate(task_prefix, prompt))
        .await
        .map_err(|error| format!("AI completion worker failed: {error}"))?
        .map_err(|error| error.to_string())
}

fn record_generated_failure(
    app: &AppHandle,
    project_snapshot: &ProjectState,
    message: &str,
) -> CommandAdapterResult<AiInterviewCommandView> {
    let state = app.state::<AppRuntime>();
    match state.lock() {
        Ok(mut runtime) => {
            if state.pipeline_is_running() {
                return ai_pipeline_conflict();
            }
            if &runtime.project_state != project_snapshot {
                stale_ai_context_failure()
            } else {
                record_ai_failure(&mut runtime, message)
            }
        }
        Err(error) => runtime_state_failure(error.to_string()),
    }
}

fn stale_ai_context_failure<T>() -> CommandAdapterResult<T> {
    command_failure(command_error(
        "ai_context_changed",
        "project state changed while AI was generating; the stale response was not applied",
    ))
}

fn runtime_state_failure<T>(message: String) -> CommandAdapterResult<T> {
    command_failure(command_error("runtime_state_unavailable", message))
}

#[tauri::command]
pub fn mark_ai_inaccurate(
    state: State<'_, AppRuntime>,
    request: MarkAiInaccurateRequest,
) -> CommandAdapterResult<AiInterviewState> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return ai_pipeline_conflict();
        }
        let response =
            ai::mark_ai_inaccurate(&runtime.ai_interview, &mut runtime.project_state, request);
        persist_ai_mutation(runtime, response, "ai.mark_inaccurate")
    })
}

#[tauri::command]
pub fn save_ai_archive(
    state: State<'_, AppRuntime>,
    request: SaveAiArchiveRequest,
) -> CommandAdapterResult<AiInterviewState> {
    with_runtime(&state, |runtime| {
        if state.pipeline_is_running() {
            return ai_pipeline_conflict();
        }
        let response =
            ai::save_ai_archive(&runtime.ai_interview, &mut runtime.project_state, request);
        persist_ai_mutation(runtime, response, "ai.save_archive")
    })
}

fn ai_pipeline_conflict<T>() -> CommandAdapterResult<T> {
    command_failure(command_error(
        "pipeline_ai_conflict",
        "stop the running pipeline before changing the AI design state",
    ))
}

fn persist_ai_mutation<T>(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_prompts_follow_the_captured_ui_language() {
        let project = ProjectState::default();
        let request = SubmitAiTurnRequest {
            user_message: "test".to_string(),
            schema_mode: AiSchemaMode::Turn,
            payload_json: None,
        };

        let chinese = interview_prompt(&project, &request, UiLanguage::ZhCn);
        assert!(chinese.contains("Write all user-facing prose in Simplified Chinese"));
        assert!(chinese.contains("请提出一个简洁的后续问题"));

        let english = interview_prompt(&project, &request, UiLanguage::EnUs);
        assert!(english.contains("Write all user-facing prose in English"));
        assert!(english.contains("Ask one concise next question"));

        let chinese_output = force_output_prompt(&project, UiLanguage::ZhCn);
        assert!(chinese_output.contains("已生成完整项目设计。"));
        let english_output = force_output_prompt(&project, UiLanguage::EnUs);
        assert!(english_output.contains("The complete project design has been generated."));
    }
}
