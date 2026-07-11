use serde_json::{Value, json};

use crate::DesignEngineService;

use super::{
    InterviewOutputMode, InterviewSchemaMode,
    prompt_packer::{
        PromptBuildResult, build_prompt_text, prompt_meter_entry, prompt_replay_fields,
    },
    state::recent_messages,
};

pub const SUMMARY_LIST_FIELDS: &[&str] = &[
    "confirmedIntent",
    "openQuestions",
    "rejectedAssumptions",
    "lastUserCorrections",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryPromptOptions {
    pub turn_id: String,
    pub framework_version: String,
    pub manifest_hash: String,
}

pub fn build_summary_correction_prompt(
    _engine: &DesignEngineService,
    project_state: &mut adm_new_contracts::project::ProjectState,
    options: SummaryPromptOptions,
) -> PromptBuildResult {
    let turn_id = if options.turn_id.trim().is_empty() {
        "summary_turn".to_string()
    } else {
        options.turn_id
    };
    let prompt_snapshot = json!({
        "frameworkVersion": options.framework_version,
        "manifestHash": options.manifest_hash,
    });
    let prompt_payload = json!({
        "turnId": turn_id,
        "task": "commercial_game_design_interview_summary_correction",
        "schemaMode": "summary",
        "promptFramework": {
            "snapshot": prompt_snapshot,
            "rules": [],
            "visibility": "hidden_to_user",
            "designOptionFrameworkMutation": "forbidden"
        },
        "projectName": project_state.project_name,
        "profile": project_state.profile,
        "projectMemoryId": project_state.ai_interview.framework_memory.project_memory_id,
        "evaluationBatchId": project_state.ai_interview.framework_memory.evaluation_batch_id,
        "currentSummary": serde_json::to_value(&project_state.ai_interview.summary.v1).unwrap_or_else(|_| json!({})),
        "recentMessages": recent_messages(&project_state.ai_interview, 20),
        "summaryRequirements": [
            "只修正 summary，不要新增设计框架项。",
            "保留 schemaVersion、confirmedIntent、openQuestions、rejectedAssumptions、nodeNotes、lastUserCorrections、mdaProgress、updatedAt。",
            "删除明显重复或互相矛盾的摘要项；不要把未确认内容写入 confirmedIntent。",
            "返回 mode=summary_correction 和 summary。"
        ]
    });
    let prompt_text = build_prompt_text(&prompt_snapshot, &prompt_payload);
    let schema_mode = InterviewSchemaMode::Summary;
    let output_mode = InterviewOutputMode::SummaryCorrection;
    let meter = prompt_meter_entry(
        &turn_id,
        &schema_mode,
        &output_mode,
        &prompt_payload,
        &prompt_text,
        &[],
        &project_state.ai_interview,
    );
    let replay = prompt_replay_fields(&prompt_text, 2000, false);
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

pub fn validate_summary_payload(payload: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    if !payload.is_object() {
        return vec!["summary payload is not a JSON object".to_string()];
    }
    let mode = payload
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !matches!(mode, "summary_correction" | "maintenance" | "error") {
        errors.push(format!("summary mode invalid: {mode}"));
    }
    let Some(summary) = payload.get("summary").and_then(Value::as_object) else {
        errors.push("summary must be an object".to_string());
        return errors;
    };
    for field in SUMMARY_LIST_FIELDS {
        if !summary
            .get(*field)
            .map(|value| value.is_array())
            .unwrap_or(true)
        {
            errors.push(format!("summary.{field} must be an array"));
        }
    }
    if !summary
        .get("nodeNotes")
        .map(|value| value.is_object())
        .unwrap_or(true)
    {
        errors.push("summary.nodeNotes must be an object".to_string());
    }
    if !summary
        .get("mdaProgress")
        .map(|value| value.is_object())
        .unwrap_or(true)
    {
        errors.push("summary.mdaProgress must be an object".to_string());
    }
    errors
}
