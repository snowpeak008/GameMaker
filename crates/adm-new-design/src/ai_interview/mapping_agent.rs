use serde_json::{Value, json};

use crate::{DesignEngineService, DesignNodeSpec};

use super::{
    CANDIDATE_NODE_LIMIT, InterviewOutputMode, InterviewSchemaMode,
    prompt_packer::{
        PromptBuildOptions, PromptBuildResult, build_prompt_text, compact_project_summary,
        framework_context, project_digest, prompt_meter_entry, prompt_replay_fields, stable_hash,
    },
    route_planner::{candidate_node_ids, text_tokens},
    state::{mda_progress_for_count, recent_messages},
};

pub fn explicit_option_signal(engine: &DesignEngineService, user_text: &str) -> bool {
    let tokens = text_tokens(user_text);
    if tokens.is_empty() {
        return false;
    }
    let lower = user_text.to_lowercase();
    engine.specs.iter().any(|spec| {
        spec.checklist.iter().any(|item| {
            item.option_groups.iter().any(|group| {
                group.options.iter().any(|option_id| {
                    let option_id = option_id.to_lowercase();
                    tokens.contains(&option_id) || lower.contains(&option_id)
                })
            })
        })
    })
}

pub fn readiness_near(question_group_count: u32, window: u32, interval: u32) -> bool {
    if question_group_count == 0 || interval == 0 {
        return false;
    }
    let remaining = interval - (question_group_count % interval);
    remaining <= window || remaining == interval
}

pub fn should_schedule_mapping(
    engine: &DesignEngineService,
    project_state: &adm_new_contracts::project::ProjectState,
    user_text: &str,
    force_output: bool,
) -> bool {
    if force_output {
        return false;
    }
    explicit_option_signal(engine, user_text)
        || readiness_near(project_state.ai_interview.question_group_count, 2, 10)
}

pub fn build_mapping_prompt(
    engine: &DesignEngineService,
    project_state: &mut adm_new_contracts::project::ProjectState,
    user_text: &str,
    mut options: PromptBuildOptions,
) -> PromptBuildResult {
    let turn_id = if options.turn_id.trim().is_empty() {
        "mapping_turn".to_string()
    } else {
        options.turn_id.clone()
    };
    options.turn_id = turn_id.clone();
    project_state.ai_interview.summary.v1.mda_progress =
        mda_progress_for_count(project_state.ai_interview.question_group_count);
    let candidate_ids = candidate_node_ids(engine, project_state, user_text, CANDIDATE_NODE_LIMIT);
    let prompt_snapshot = json!({
        "frameworkVersion": options.framework_version,
        "manifestHash": options.manifest_hash,
    });
    let prompt_payload = json!({
        "turnId": turn_id,
        "task": "commercial_game_design_background_mapping",
        "schemaMode": "mapping",
        "promptFramework": {
            "snapshot": prompt_snapshot,
            "rules": [],
            "visibility": "hidden_to_user",
            "designOptionFrameworkMutation": "forbidden"
        },
        "projectSummary": compact_project_summary(engine, project_state),
        "projectDigest": project_digest(engine, project_state, true),
        "conversationSummary": serde_json::to_value(&project_state.ai_interview.summary.v1).unwrap_or_else(|_| json!({})),
        "questionGroupCount": project_state.ai_interview.question_group_count,
        "evaluationBatchId": project_state.ai_interview.framework_memory.evaluation_batch_id,
        "projectMemoryId": project_state.ai_interview.framework_memory.project_memory_id,
        "recentMessages": recent_messages(&project_state.ai_interview, 6),
        "frameworkContext": framework_context(engine, project_state, false, &candidate_ids, CANDIDATE_NODE_LIMIT),
        "userMessage": user_text,
        "mappingRequirements": [
            "只返回 mode=mapping 和 inferences。",
            "只能映射到 frameworkContext 中出现的现有 nodeId/itemId/groupId/optionIds。",
            "证据不足时返回空 inferences，不要猜测。",
            "0.75 及以上才可作为高置信；多义时降低置信度。"
        ]
    });
    let prompt_text = build_prompt_text(&prompt_snapshot, &prompt_payload);
    let schema_mode = InterviewSchemaMode::Mapping;
    let output_mode = InterviewOutputMode::Mapping;
    let meter = prompt_meter_entry(
        &turn_id,
        &schema_mode,
        &output_mode,
        &prompt_payload,
        &prompt_text,
        &[],
        &project_state.ai_interview,
    );
    let mut replay = prompt_replay_fields(&prompt_text, 2000, options.store_full_prompt);
    if let Some(object) = replay.as_object_mut() {
        object.insert(
            "projectStateHash".to_string(),
            Value::String(project_state_hash(engine, project_state)),
        );
    }
    PromptBuildResult {
        turn_id,
        schema_mode,
        output_mode,
        prompt_text,
        prompt_payload,
        meter,
        replay,
        degradations: Vec::new(),
    }
}

pub fn project_state_hash(
    engine: &DesignEngineService,
    project_state: &adm_new_contracts::project::ProjectState,
) -> String {
    stable_hash(&project_digest(engine, project_state, true))
}

pub fn validate_mapping_payload(engine: &DesignEngineService, payload: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if !payload.is_object() {
        return vec!["mapping payload is not a JSON object".to_string()];
    }
    let mode = payload
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(mode, "mapping" | "maintenance" | "error") {
        errors.push(format!("mapping mode invalid: {mode}"));
    }
    let Some(inferences) = payload.get("inferences").and_then(Value::as_array) else {
        errors.push("mapping inferences must be an array".to_string());
        return errors;
    };
    for inference in inferences {
        let Some(object) = inference.as_object() else {
            errors.push("mapping inference must be an object".to_string());
            continue;
        };
        let node_id = object
            .get("nodeId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let item_id = object
            .get("itemId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let group_id = object
            .get("groupId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(confidence) = object.get("confidence").and_then(Value::as_f64) {
            if !(0.0..=1.0).contains(&confidence) {
                errors.push(format!("mapping confidence out of range: {confidence}"));
            }
        }
        let Some(node) = find_node(engine, node_id) else {
            errors.push(format!("mapping contains unknown node: {node_id}"));
            continue;
        };
        let Some(item) = node.checklist.iter().find(|item| item.item_id == item_id) else {
            errors.push(format!(
                "mapping contains unknown checklist: {node_id}/{item_id}"
            ));
            continue;
        };
        let Some(group) = item
            .option_groups
            .iter()
            .find(|group| group.group_id == group_id)
        else {
            errors.push(format!(
                "mapping contains unknown L4 group: {node_id}/{item_id}/{group_id}"
            ));
            continue;
        };
        let invalid = object
            .get("optionIds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|option_id| !group.options.iter().any(|allowed| allowed == option_id))
            .take(6)
            .map(str::to_string)
            .collect::<Vec<_>>();
        if !invalid.is_empty() {
            errors.push(format!(
                "mapping contains unknown option: {node_id}/{item_id}/{group_id}: {}",
                invalid.join(", ")
            ));
        }
    }
    errors
}

fn find_node<'a>(engine: &'a DesignEngineService, node_id: &str) -> Option<&'a DesignNodeSpec> {
    engine.specs.iter().find(|spec| spec.node_id == node_id)
}
