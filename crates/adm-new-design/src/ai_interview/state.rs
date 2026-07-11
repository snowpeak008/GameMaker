use std::collections::BTreeMap;

use adm_new_contracts::ai::{
    AI_INTERVIEW_SCHEMA_VERSION, AiInterviewState, AiResponseMode, AiResponsePayload,
    ConversationSummaryV1, FrameworkMemoryState, MDA_STAGES, SUMMARY_SCHEMA_VERSION,
};
use adm_new_contracts::project::ProjectState;
use adm_new_foundation::{new_stable_id, unix_timestamp};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{HIGH_CONFIDENCE_THRESHOLD, project_state_ai_mut};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewTurnStart {
    pub turn_id: String,
    pub force_output: bool,
    pub user_message_index: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewApplyReport {
    pub mode: AiResponseMode,
    pub message_count: usize,
    pub question_group_count: u32,
    pub awaiting_user_answer: bool,
    pub summary_changed: bool,
    pub applicability_changed: bool,
    pub memory_event_hints: Vec<String>,
}

pub fn now_iso() -> String {
    format!("unix:{}", unix_timestamp())
}

pub fn normalize_interview_state(state: &mut AiInterviewState) {
    state.schema_version = AI_INTERVIEW_SCHEMA_VERSION.to_string();
    if state.status.trim().is_empty() {
        state.status = "idle".to_string();
    }
    if state.backend_stage.trim().is_empty() {
        state.backend_stage = "idle".to_string();
    }
    state.summary.v1.schema_version = SUMMARY_SCHEMA_VERSION.to_string();
    if state.summary.v1.mda_progress.is_empty() {
        state.summary.v1.mda_progress = mda_progress_for_count(state.question_group_count);
    }
    if state.framework_memory.project_memory_id.trim().is_empty() {
        state.framework_memory.project_memory_id =
            new_stable_id("project").unwrap_or_else(|_| "project_memory".to_string());
    }
    normalize_framework_memory(&mut state.framework_memory);
    state.summary.v1.mda_progress = mda_progress_for_count(state.question_group_count);
    if state.updated_at.trim().is_empty() {
        state.updated_at = now_iso();
    }
}

fn normalize_framework_memory(memory: &mut FrameworkMemoryState) {
    if memory.batch_status.trim().is_empty() {
        memory.batch_status = "idle".to_string();
    }
    if memory.updated_at.trim().is_empty() {
        memory.updated_at = now_iso();
    }
}

pub fn mda_progress_for_count(group_count: u32) -> BTreeMap<String, String> {
    let stage_count = MDA_STAGES.len().max(1);
    let current_index = group_count as usize % stage_count;
    let completed_cycle = group_count as usize >= stage_count;
    MDA_STAGES
        .iter()
        .enumerate()
        .map(|(index, (stage_id, _))| {
            let value = if index == current_index {
                "in_progress"
            } else if completed_cycle || index < current_index {
                "explored"
            } else {
                "pending"
            };
            ((*stage_id).to_string(), value.to_string())
        })
        .collect()
}

pub fn conversation_summary(state: &mut AiInterviewState) -> &mut ConversationSummaryV1 {
    state.summary.v1.schema_version = SUMMARY_SCHEMA_VERSION.to_string();
    if state.summary.v1.mda_progress.is_empty() {
        state.summary.v1.mda_progress = mda_progress_for_count(state.question_group_count);
    }
    &mut state.summary.v1
}

pub fn start_user_turn(
    project_state: &mut ProjectState,
    user_text: &str,
    turn_id: &str,
    force_output: bool,
) -> InterviewTurnStart {
    let ai_state = project_state_ai_mut(project_state);
    normalize_interview_state(ai_state);
    let turn_id = if turn_id.trim().is_empty() {
        new_stable_id("turn").unwrap_or_else(|_| "turn".to_string())
    } else {
        turn_id.to_string()
    };
    ai_state.status = "running".to_string();
    ai_state.active_turn_id = turn_id.clone();
    ai_state.run_started_at = now_iso();
    ai_state.backend_stage = "queued".to_string();
    ai_state.awaiting_user_answer = false;
    let index = ai_state.messages.len();
    add_message(
        ai_state,
        "user",
        user_text,
        Some(json!({
            "turnId": turn_id,
            "forceOutput": force_output,
        })),
    );
    InterviewTurnStart {
        turn_id,
        force_output,
        user_message_index: index,
    }
}

pub fn mark_backend_started(ai_state: &mut AiInterviewState, backend_name: &str) {
    normalize_interview_state(ai_state);
    ai_state.status = "running".to_string();
    ai_state.backend_stage = if backend_name.trim().is_empty() {
        "running".to_string()
    } else {
        format!("running:{backend_name}")
    };
    ai_state.backend_started_at = now_iso();
    ai_state.updated_at = now_iso();
}

pub fn mark_backend_failed(ai_state: &mut AiInterviewState, message: &str) {
    normalize_interview_state(ai_state);
    ai_state.status = "error".to_string();
    ai_state.backend_stage = "error".to_string();
    ai_state.last_error = message.to_string();
    ai_state.updated_at = now_iso();
}

pub fn add_message(
    ai_state: &mut AiInterviewState,
    role: &str,
    content: &str,
    meta: Option<Value>,
) -> Value {
    let mut message = serde_json::Map::new();
    message.insert("role".to_string(), Value::String(role.to_string()));
    message.insert("content".to_string(), Value::String(content.to_string()));
    message.insert("createdAt".to_string(), Value::String(now_iso()));
    if let Some(meta) = meta {
        message.insert("meta".to_string(), meta);
    }
    let value = Value::Object(message);
    ai_state.messages.push(value.clone());
    ai_state.updated_at = now_iso();
    value
}

pub fn recent_messages(ai_state: &AiInterviewState, limit: usize) -> Vec<Value> {
    ai_state
        .messages
        .iter()
        .rev()
        .take(limit)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

pub fn apply_payload_to_interview_state(
    project_state: &mut ProjectState,
    payload: &AiResponsePayload,
    turn_id: Option<&str>,
) -> InterviewApplyReport {
    normalize_interview_state(&mut project_state.ai_interview);
    let mut memory_event_hints = Vec::new();
    let mut summary_changed = false;
    let mut applicability_changed = false;

    {
        let ai_state = &mut project_state.ai_interview;
        ai_state.session_turn_count += 1;
        ai_state.status = "completed".to_string();
        ai_state.backend_stage = "completed".to_string();
        ai_state.updated_at = now_iso();
        if let Some(turn_id) = turn_id {
            ai_state.active_turn_id = turn_id.to_string();
        }
        if !payload.assistant_message.trim().is_empty() {
            add_message(
                ai_state,
                "assistant",
                &payload.assistant_message,
                Some(json!({"mode": payload_mode_string(&payload.mode)})),
            );
        }
        if let Some(route) = payload.route_overview.clone() {
            ai_state.route_overview = route;
        }
        ai_state.inferences.extend(payload.inferences.clone());
        ai_state.option_differences = payload.option_differences.clone();
    }

    match payload.mode {
        AiResponseMode::QuestionGroup => {
            let ai_state = &mut project_state.ai_interview;
            ai_state.question_group_count += 1;
            ai_state.current_question_count += 1;
            ai_state.awaiting_user_answer = true;
            ai_state.current_question_turn_id = turn_id.unwrap_or_default().to_string();
            ai_state.current_question_text =
                question_text(payload.question_group.as_ref(), &payload.assistant_message);
            remember_recent_targets(ai_state, payload.question_group.as_ref());
            memory_event_hints.push("question_group_review_recorded".to_string());
        }
        AiResponseMode::ReadinessCheck => {
            let ai_state = &mut project_state.ai_interview;
            ai_state.awaiting_user_answer = false;
            ai_state.last_readiness_check_group = ai_state.question_group_count;
            memory_event_hints.push("readiness_check_recorded".to_string());
        }
        AiResponseMode::FullProjectOutput => {
            let ai_state = &mut project_state.ai_interview;
            ai_state.awaiting_user_answer = false;
            if let Some(output) = payload.full_project_output.clone() {
                ai_state.output_history.push(output);
            }
            memory_event_hints.push("full_project_output_recorded".to_string());
        }
        AiResponseMode::PartialProjectOutput => {
            memory_event_hints.push("partial_project_output_recorded".to_string());
        }
        AiResponseMode::Mapping => {
            memory_event_hints.push("mapping_payload_context_recorded".to_string());
        }
        AiResponseMode::SummaryCorrection => {
            if let Some(summary) = payload.summary.as_ref() {
                summary_changed |= apply_summary_value(&mut project_state.ai_interview, summary);
            }
            memory_event_hints.push("summary_correction_recorded".to_string());
        }
        AiResponseMode::Confirmation | AiResponseMode::Maintenance => {
            memory_event_hints.push("non_output_turn_recorded".to_string());
        }
        AiResponseMode::Error => {
            let ai_state = &mut project_state.ai_interview;
            ai_state.status = "error".to_string();
            ai_state.backend_stage = "error".to_string();
            ai_state.last_error = payload.errors.join("; ");
            memory_event_hints.push("backend_error_recorded".to_string());
        }
    }
    summary_changed |= update_conversation_summary(project_state, Some(payload), "");
    applicability_changed |= update_applicability_scores(project_state, &payload.inferences);
    project_state.ai_interview.summary.v1.mda_progress =
        mda_progress_for_count(project_state.ai_interview.question_group_count);
    project_state.ai_interview.updated_at = now_iso();

    InterviewApplyReport {
        mode: payload.mode.clone(),
        message_count: project_state.ai_interview.messages.len(),
        question_group_count: project_state.ai_interview.question_group_count,
        awaiting_user_answer: project_state.ai_interview.awaiting_user_answer,
        summary_changed,
        applicability_changed,
        memory_event_hints,
    }
}

pub fn update_conversation_summary(
    project_state: &mut ProjectState,
    payload: Option<&AiResponsePayload>,
    correction: &str,
) -> bool {
    normalize_interview_state(&mut project_state.ai_interview);
    let mut changed = false;
    let now = now_iso();
    {
        let ai_state = &mut project_state.ai_interview;
        let question_group_count = ai_state.question_group_count;
        let summary = conversation_summary(ai_state);
        summary.mda_progress = mda_progress_for_count(question_group_count);
        if !correction.trim().is_empty() {
            let item = json!({
                "createdAt": now,
                "text": short_text(correction, 180),
            });
            append_limited(&mut summary.last_user_corrections, item.clone(), 12);
            append_limited(&mut summary.rejected_assumptions, item, 12);
            changed = true;
        }
    }
    if let Some(payload) = payload {
        if let Some(question_group) = payload.question_group.as_ref() {
            let open_item = open_question_item(question_group);
            append_limited(
                &mut project_state.ai_interview.summary.v1.open_questions,
                open_item,
                12,
            );
            changed = true;
        }
        for inference in &payload.inferences {
            let Some(note) = inference_summary_note(inference) else {
                continue;
            };
            let confidence = note
                .get("confidence")
                .and_then(Value::as_f64)
                .unwrap_or_default();
            if confidence >= HIGH_CONFIDENCE_THRESHOLD {
                append_limited(
                    &mut project_state.ai_interview.summary.v1.confirmed_intent,
                    note.clone(),
                    24,
                );
            }
            if let Some(node_id) = note.get("nodeId").and_then(Value::as_str) {
                let entry = project_state
                    .ai_interview
                    .summary
                    .v1
                    .node_notes
                    .entry(node_id.to_string())
                    .or_insert_with(|| Value::Array(Vec::new()));
                if let Some(items) = entry.as_array_mut() {
                    items.push(note);
                    if items.len() > 8 {
                        items.drain(0..items.len() - 8);
                    }
                }
            }
            changed = true;
        }
    }
    if changed {
        project_state.ai_interview.summary.v1.updated_at = now_iso();
        project_state.ai_interview.updated_at = now_iso();
    }
    changed
}

pub fn update_applicability_scores(project_state: &mut ProjectState, inferences: &[Value]) -> bool {
    let mut changed = false;
    for inference in inferences {
        let Some(node_id) = inference.get("nodeId").and_then(Value::as_str) else {
            continue;
        };
        if node_id.trim().is_empty() {
            continue;
        }
        let mut score = inference
            .get("applicabilityScore")
            .and_then(Value::as_f64)
            .unwrap_or(f64::NAN);
        if score.is_nan() {
            let confidence = inference
                .get("confidence")
                .and_then(Value::as_f64)
                .unwrap_or_default();
            if inference
                .get("notApplicable")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                score = (1.0 - confidence).max(0.0);
            } else if inference
                .get("optionIds")
                .and_then(Value::as_array)
                .map(|items| !items.is_empty())
                .unwrap_or(false)
            {
                score = confidence;
            } else {
                continue;
            }
        }
        let score = score.clamp(0.0, 1.0);
        let entry = project_state
            .ai_interview
            .applicability_scores
            .entry(node_id.to_string())
            .or_insert_with(|| {
                json!({
                    "score": 0.5,
                    "evidenceCount": 0,
                    "reason": "",
                    "updatedAt": now_iso(),
                })
            });
        let current = entry.get("score").and_then(Value::as_f64).unwrap_or(0.5);
        let count = entry
            .get("evidenceCount")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let next_count = count + 1;
        let next_score =
            (((current * count as f64) + score) / next_count as f64 * 1000.0).round() / 1000.0;
        *entry = json!({
            "score": next_score,
            "evidenceCount": next_count,
            "reason": inference.get("applicabilityReason").or_else(|| inference.get("reason")).cloned().unwrap_or_else(|| Value::String(String::new())),
            "updatedAt": now_iso(),
        });
        changed = true;
    }
    if changed {
        project_state.ai_interview.updated_at = now_iso();
    }
    changed
}

fn apply_summary_value(ai_state: &mut AiInterviewState, summary: &Value) -> bool {
    let Some(object) = summary.as_object() else {
        return false;
    };
    let target = &mut ai_state.summary.v1;
    if let Some(items) = object.get("confirmedIntent").and_then(Value::as_array) {
        target.confirmed_intent = items.clone();
    }
    if let Some(items) = object.get("openQuestions").and_then(Value::as_array) {
        target.open_questions = items.clone();
    }
    if let Some(items) = object.get("rejectedAssumptions").and_then(Value::as_array) {
        target.rejected_assumptions = items.clone();
    }
    if let Some(items) = object.get("lastUserCorrections").and_then(Value::as_array) {
        target.last_user_corrections = items.clone();
    }
    if let Some(items) = object.get("nodeNotes").and_then(Value::as_object) {
        target.node_notes = items
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
    }
    if let Some(items) = object.get("mdaProgress").and_then(Value::as_object) {
        target.mda_progress = items
            .iter()
            .map(|(key, value)| (key.clone(), value.as_str().unwrap_or_default().to_string()))
            .collect();
    }
    target.updated_at = now_iso();
    true
}

fn question_text(question_group: Option<&Value>, fallback: &str) -> String {
    question_group
        .and_then(|group| group.get("questions"))
        .and_then(Value::as_array)
        .and_then(|questions| questions.first())
        .and_then(|question| question.get("text"))
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn remember_recent_targets(ai_state: &mut AiInterviewState, question_group: Option<&Value>) {
    let Some(question_group) = question_group else {
        return;
    };
    let mut target_ids = Vec::new();
    if let Some(questions) = question_group.get("questions").and_then(Value::as_array) {
        for question in questions {
            if let Some(ids) = question.get("targetNodeIds").and_then(Value::as_array) {
                for id in ids {
                    if let Some(id) = id.as_str() {
                        target_ids.push(Value::String(id.to_string()));
                    }
                }
            }
        }
    }
    if !target_ids.is_empty() {
        ai_state.recent_question_targets.push(json!({
            "createdAt": now_iso(),
            "nodeIds": target_ids,
        }));
        if ai_state.recent_question_targets.len() > 12 {
            ai_state
                .recent_question_targets
                .drain(0..ai_state.recent_question_targets.len() - 12);
        }
    }
}

fn open_question_item(question_group: &Value) -> Value {
    let mut target_ids = Vec::<String>::new();
    let mut questions_out = Vec::<Value>::new();
    if let Some(questions) = question_group.get("questions").and_then(Value::as_array) {
        for question in questions.iter().take(4) {
            if let Some(text) = question.get("text").and_then(Value::as_str) {
                questions_out.push(Value::String(short_text(text, 120)));
            }
            if let Some(ids) = question.get("targetNodeIds").and_then(Value::as_array) {
                for id in ids {
                    if let Some(id) = id.as_str() {
                        target_ids.push(id.to_string());
                    }
                }
            }
        }
    }
    target_ids.sort();
    target_ids.dedup();
    json!({
        "createdAt": now_iso(),
        "purpose": short_text(question_group.get("purpose").and_then(Value::as_str).unwrap_or_default(), 120),
        "targetNodeIds": target_ids.into_iter().take(8).collect::<Vec<_>>(),
        "questions": questions_out,
    })
}

fn inference_summary_note(inference: &Value) -> Option<Value> {
    let node_id = inference
        .get("nodeId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if node_id.trim().is_empty() {
        return None;
    }
    let confidence = inference
        .get("confidence")
        .and_then(Value::as_f64)
        .unwrap_or_default();
    Some(json!({
        "createdAt": now_iso(),
        "nodeId": node_id,
        "itemId": inference.get("itemId").and_then(Value::as_str).unwrap_or_default(),
        "groupId": inference.get("groupId").and_then(Value::as_str).unwrap_or_default(),
        "optionIds": inference.get("optionIds").and_then(Value::as_array).cloned().unwrap_or_default().into_iter().take(8).collect::<Vec<_>>(),
        "confidence": (confidence * 1000.0).round() / 1000.0,
        "reason": short_text(inference.get("reason").and_then(Value::as_str).unwrap_or_default(), 180),
        "notApplicable": inference.get("notApplicable").and_then(Value::as_bool).unwrap_or(false),
    }))
}

fn append_limited(items: &mut Vec<Value>, item: Value, limit: usize) {
    if item.is_null() {
        return;
    }
    items.push(item);
    if items.len() > limit {
        items.drain(0..items.len() - limit);
    }
}

pub fn short_text(value: &str, limit: usize) -> String {
    value
        .replace('\n', " ")
        .trim()
        .chars()
        .take(limit)
        .collect()
}

pub fn payload_mode_string(mode: &AiResponseMode) -> &'static str {
    match mode {
        AiResponseMode::QuestionGroup => "question_group",
        AiResponseMode::Confirmation => "confirmation",
        AiResponseMode::ReadinessCheck => "readiness_check",
        AiResponseMode::FullProjectOutput => "full_project_output",
        AiResponseMode::PartialProjectOutput => "partial_project_output",
        AiResponseMode::Mapping => "mapping",
        AiResponseMode::SummaryCorrection => "summary_correction",
        AiResponseMode::Maintenance => "maintenance",
        AiResponseMode::Error => "error",
    }
}
