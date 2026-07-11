use adm_new_knowledge::memory::MemoryTier;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::HIGH_CONFIDENCE_THRESHOLD;
use super::state::now_iso;

pub const SHORT_TERM_DECAY_RATE: f64 = 0.08;
pub const SHORT_TERM_IMPORTANCE: f64 = 0.6;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UcosInterviewEvent {
    pub event_kind: String,
    pub tier: MemoryTier,
    pub relative_path: String,
    pub payload: Value,
}

pub fn record_interview_turn_events(
    turn_id: &str,
    user_text: &str,
    payload: &Value,
    project_memory_id: &str,
    evaluation_batch_id: &str,
) -> Vec<UcosInterviewEvent> {
    if turn_id.trim().is_empty() {
        return Vec::new();
    }
    let now = now_iso();
    let mut events = Vec::new();
    events.push(write_episodic_turn(
        turn_id,
        user_text,
        payload,
        project_memory_id,
        evaluation_batch_id,
        &now,
    ));
    if let Some(router_decision) = payload
        .get("routerDecision")
        .filter(|value| value.is_object())
    {
        events.push(write_short_term_router_context(
            turn_id,
            user_text,
            router_decision,
            &now,
        ));
    }
    for inference in filter_high_confidence(
        payload
            .get("inferences")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    ) {
        events.push(write_semantic_inference(turn_id, &inference, &now));
    }
    let mode = payload
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if matches!(mode, "full_project_output" | "partial_project_output")
        && payload.get("fullProjectOutput").is_some()
    {
        events.push(write_design_generation_episode(
            turn_id,
            user_text,
            payload,
            project_memory_id,
            &now,
        ));
    }
    events
}

pub fn filter_high_confidence(inferences: Vec<Value>) -> Vec<Value> {
    inferences
        .into_iter()
        .filter(|inference| {
            inference
                .get("confidence")
                .and_then(Value::as_f64)
                .unwrap_or_default()
                >= HIGH_CONFIDENCE_THRESHOLD
        })
        .collect()
}

fn write_episodic_turn(
    turn_id: &str,
    user_text: &str,
    payload: &Value,
    project_memory_id: &str,
    evaluation_batch_id: &str,
    now: &str,
) -> UcosInterviewEvent {
    let project_id = if project_memory_id.trim().is_empty() {
        "unknown_project"
    } else {
        project_memory_id
    };
    let batch_id = if evaluation_batch_id.trim().is_empty() {
        "unknown_batch"
    } else {
        evaluation_batch_id
    };
    let relative_path = format!("knowledge/episodic/turns/{project_id}/{batch_id}/{turn_id}.json");
    UcosInterviewEvent {
        event_kind: "episodic_turn".to_string(),
        tier: MemoryTier::Episodic,
        relative_path,
        payload: json!({
            "turnId": turn_id,
            "updatedAt": now,
            "createdAt": now,
            "userText": user_text,
            "mode": payload.get("mode").and_then(Value::as_str).unwrap_or_default(),
            "response": {
                "mode": payload.get("mode").and_then(Value::as_str).unwrap_or_default(),
                "questionGroup": payload.get("questionGroup").cloned().unwrap_or(Value::Null),
                "inferences": payload.get("inferences").cloned().unwrap_or_else(|| json!([])),
                "assistantMessage": payload.get("assistantMessage").and_then(Value::as_str).unwrap_or_default(),
            }
        }),
    }
}

fn write_short_term_router_context(
    turn_id: &str,
    user_text: &str,
    router_decision: &Value,
    now: &str,
) -> UcosInterviewEvent {
    let candidate_nodes = router_decision
        .get("candidateNodes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let candidate_names = candidate_nodes
        .iter()
        .filter_map(|node| {
            node.get("name")
                .or_else(|| node.get("id"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    let prefix = turn_id.chars().take(16).collect::<String>();
    let stm_id = format!("stm_router_{prefix}");
    UcosInterviewEvent {
        event_kind: "short_term_router_context".to_string(),
        tier: MemoryTier::ShortTerm,
        relative_path: format!("knowledge/short_term/entries/{stm_id}.json"),
        payload: json!({
            "schema_version": "1.0",
            "stm_id": stm_id,
            "type": "ai_routing_context",
            "title": format!("AI 路由决策：{}", candidate_names.iter().take(3).cloned().collect::<Vec<_>>().join(", ")),
            "content": format!("用户输入：{}；候选节点：{}", user_text.chars().take(200).collect::<String>(), candidate_names.join(", ")),
            "source": {
                "type": "ai_interview",
                "session_id": "",
                "file_ref": format!("episodic/turns/.../{turn_id}.json")
            },
            "tags": std::iter::once("ai_interview".to_string()).chain(std::iter::once("router_decision".to_string())).chain(candidate_names.iter().take(5).cloned()).collect::<Vec<_>>(),
            "importance": SHORT_TERM_IMPORTANCE,
            "created_at": now,
            "last_accessed": now,
            "decay_rate": SHORT_TERM_DECAY_RATE,
            "current_relevance": SHORT_TERM_IMPORTANCE,
            "consolidate_to_episodic": false
        }),
    }
}

fn write_semantic_inference(turn_id: &str, inference: &Value, now: &str) -> UcosInterviewEvent {
    let node_id = inference
        .get("nodeId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let fact_id = format!(
        "sf_interview_{}_{}",
        turn_id.chars().take(16).collect::<String>(),
        node_id.chars().take(16).collect::<String>()
    );
    UcosInterviewEvent {
        event_kind: "semantic_inference".to_string(),
        tier: MemoryTier::Semantic,
        relative_path: format!("knowledge/semantic/staging/staged_{fact_id}.json"),
        payload: json!({
            "schema_version": "1.0",
            "fact_id": fact_id,
            "type": "design_decision_inference",
            "domain": "game_design",
            "subject": node_id,
            "content": {
                "nodeId": node_id,
                "checklist": inference.get("checklist").cloned().unwrap_or_else(|| json!([])),
                "options": inference.get("options").cloned().unwrap_or_else(|| json!([])),
                "confidence": inference.get("confidence").cloned().unwrap_or_else(|| json!(0)),
                "reason": inference.get("reason").and_then(Value::as_str).unwrap_or_default()
            },
            "source": {
                "type": "ai_interview_inference",
                "ref": turn_id,
                "episode_id": ""
            },
            "confidence": inference.get("confidence").and_then(Value::as_f64).unwrap_or_default(),
            "review_required": true,
            "version": 1,
            "last_verified": now,
            "tags": ["ai_interview", "design_inference", node_id],
            "created_at": now
        }),
    }
}

fn write_design_generation_episode(
    turn_id: &str,
    user_text: &str,
    payload: &Value,
    project_memory_id: &str,
    now: &str,
) -> UcosInterviewEvent {
    let episode_id = format!("ep_design_{}", turn_id.chars().take(20).collect::<String>());
    let project_name = payload
        .get("fullProjectOutput")
        .and_then(extract_project_name)
        .unwrap_or_default();
    let key_decisions = payload
        .get("inferences")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .take(10)
        .filter_map(|inference| {
            inference.as_object().map(|_| {
                json!({
                    "summary": inference
                        .get("reason")
                        .or_else(|| inference.get("nodeId"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .chars()
                        .take(200)
                        .collect::<String>()
                })
            })
        })
        .collect::<Vec<_>>();
    UcosInterviewEvent {
        event_kind: "design_generation_episode".to_string(),
        tier: MemoryTier::Episodic,
        relative_path: format!("knowledge/episodic/episodes/{episode_id}.json"),
        payload: json!({
            "schema_version": "1.0",
            "episode_id": episode_id,
            "title": format!("设计内容生成：{}", if project_name.is_empty() { "游戏设计项目" } else { &project_name }),
            "domain": "game_design",
            "goal": format!("基于用户输入生成完整游戏设计方案。用户输入：{}", user_text.chars().take(200).collect::<String>()),
            "reason": "full_project_output",
            "key_decisions": key_decisions,
            "outcome": {
                "status": "success",
                "result_summary": format!("生成完整设计方案，项目：{project_name}")
            },
            "lessons": [],
            "related_episodes": [],
            "skill_ids": [],
            "pattern_ids_extracted": [],
            "failure_ids_extracted": [],
            "reflection_done": false,
            "created_at": now,
            "source_files": [],
            "project_memory_id": project_memory_id,
            "turn_id": turn_id
        }),
    }
}

fn extract_project_name(full_output: &Value) -> Option<String> {
    let project_state_raw = full_output
        .get("projectStateJson")
        .and_then(Value::as_str)?;
    let state: Value = serde_json::from_str(project_state_raw).ok()?;
    state
        .get("projectName")
        .and_then(Value::as_str)
        .map(str::to_string)
}
