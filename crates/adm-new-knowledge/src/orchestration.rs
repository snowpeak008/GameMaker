use crate::identity::IdentityEngine;
use crate::memory::{MemoryEngine, MemoryTier, now_string};
use crate::store::UcosStore;
use adm_new_foundation::{AdmError, AdmResult, new_stable_id, sha256_hex};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct DecisionEngine {
    identity: IdentityEngine,
}

impl DecisionEngine {
    pub fn new(identity: IdentityEngine) -> Self {
        Self { identity }
    }

    pub fn decide(&self, options: &[Value]) -> Option<Value> {
        let mut allowed = Vec::new();
        for option in options {
            let validation = self.identity.validate_action(option);
            let mut item = option.clone();
            if let Some(object) = item.as_object_mut() {
                object.insert(
                    "identity_allowed".to_string(),
                    Value::Bool(validation.allowed),
                );
                object.insert(
                    "identity_reason".to_string(),
                    Value::String(validation.reason),
                );
            }
            if validation.allowed {
                allowed.push(item);
            }
        }
        allowed.sort_by(|left, right| {
            score(right)
                .partial_cmp(&score(left))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        allowed.into_iter().next()
    }
}

#[derive(Debug, Clone)]
pub struct WorldModelEngine {
    store: UcosStore,
}

impl WorldModelEngine {
    pub fn new(store: UcosStore) -> Self {
        Self { store }
    }

    pub fn get_dependencies(&self, node_id: &str) -> Vec<String> {
        self.store
            .read_json("execution/world_model/dependency_map.json", json!({}))
            .pointer(&format!("/dependencies/{node_id}"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn isolated_nodes(&self) -> Vec<String> {
        let data = self
            .store
            .read_json("execution/world_model/causal_graph.json", json!({}));
        let nodes = data
            .get("nodes")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<std::collections::BTreeSet<_>>()
            })
            .unwrap_or_default();
        let mut connected = std::collections::BTreeSet::new();
        for edge in data
            .get("edges")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(from) = edge.get("from").and_then(Value::as_str) {
                connected.insert(from.to_string());
            }
            if let Some(to) = edge.get("to").and_then(Value::as_str) {
                connected.insert(to.to_string());
            }
        }
        nodes.difference(&connected).cloned().collect()
    }

    pub fn diagnostics(&self) -> WorldModelDiagnostics {
        WorldModelDiagnostics {
            dependency_map_present: self
                .store
                .resolve("execution/world_model/dependency_map.json")
                .exists(),
            causal_graph_present: self
                .store
                .resolve("execution/world_model/causal_graph.json")
                .exists(),
            domain_model_count: self
                .store
                .json_files("execution/world_model/domain_models", false)
                .map(|files| files.len())
                .unwrap_or(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldModelDiagnostics {
    pub dependency_map_present: bool,
    pub causal_graph_present: bool,
    pub domain_model_count: usize,
}

#[derive(Debug, Clone)]
pub struct PlanningEngine {
    world: WorldModelEngine,
}

impl PlanningEngine {
    pub fn new(world: WorldModelEngine) -> Self {
        Self { world }
    }

    pub fn plan(&self, goal: &str, context: &Value) -> Value {
        let target_stage = if goal.contains("Stage10") || goal.to_lowercase().contains("stage10") {
            "stage_10"
        } else {
            "stage_15"
        };
        let dependencies = self.world.get_dependencies(target_stage);
        let task_titles = [
            format!("Verify prerequisites for {target_stage}"),
            format!("Execute governed workflow for {target_stage}"),
            format!("Validate artifacts and record outcome for {target_stage}"),
        ];
        let tasks = task_titles
            .iter()
            .enumerate()
            .map(|(index, title)| {
                let task_id = format!("task_{:03}", index + 1);
                json!({
                    "task_id": task_id,
                    "title": title,
                    "required_skills": ["skill_read_file_v1", "skill_validate_schema_v1"],
                    "dependencies": if index == 0 { json!(dependencies) } else { json!([format!("task_{index:03}")]) },
                    "status": "pending",
                    "output": Value::Null,
                })
            })
            .collect::<Vec<_>>();
        let snapshot = serde_json::to_vec(context).unwrap_or_default();
        json!({
            "schema_version": "1.0",
            "plan_id": new_stable_id("plan").unwrap_or_else(|_| "plan_generated".to_string()),
            "goal_id": context
                .get("goal_id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| new_stable_id("goal").unwrap_or_else(|_| "goal_generated".to_string())),
            "tasks": tasks,
            "created_at": now_string(),
            "fact_snapshot_hash": sha256_hex(&snapshot),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectionResult {
    pub episode_id: String,
    pub pattern_ids: Vec<String>,
    pub failure_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ReflectionEngine {
    store: UcosStore,
    memory: MemoryEngine,
}

impl ReflectionEngine {
    pub fn new(store: UcosStore) -> Self {
        Self {
            memory: MemoryEngine::new(store.clone()),
            store,
        }
    }

    pub fn reflect(&self, episode_id: &str) -> AdmResult<ReflectionResult> {
        let path = format!("knowledge/episodic/episodes/{episode_id}.json");
        let mut episode = self.store.read_required_json(&path)?;
        let outcome_status = episode
            .pointer("/outcome/status")
            .and_then(Value::as_str)
            .unwrap_or("partial");
        let mut pattern_ids = Vec::new();
        let mut failure_ids = Vec::new();
        if outcome_status == "success" {
            let pattern_id = new_stable_id("pat").unwrap_or_else(|_| "pat_generated".to_string());
            self.memory.write(
                MemoryTier::Patterns,
                json!({
                    "schema_version": "1.0",
                    "pattern_id": pattern_id.clone(),
                    "name": string_field(&episode, "title", "successful episode pattern"),
                    "category": "process",
                    "domain": string_field(&episode, "domain", "general"),
                    "problem": string_field(&episode, "goal", ""),
                    "solution": episode.pointer("/outcome/result_summary").and_then(Value::as_str).unwrap_or(""),
                    "consequences": {"positive": episode.get("lessons").cloned().unwrap_or_else(|| json!([])), "negative": []},
                    "source_episodes": [episode_id],
                    "confidence": 0.8,
                    "usage_count": 0,
                    "human_verified": false,
                    "created_at": now_string(),
                }),
                "reflection",
                0.8,
            )?;
            pattern_ids.push(pattern_id);
        } else {
            let failure_id = new_stable_id("fail").unwrap_or_else(|_| "fail_generated".to_string());
            self.memory.write(
                MemoryTier::Failures,
                json!({
                    "schema_version": "1.0",
                    "failure_id": failure_id.clone(),
                    "title": string_field(&episode, "title", "episode failure"),
                    "status": if outcome_status == "failure" { "open" } else { "resolved" },
                    "severity": "medium",
                    "domain": string_field(&episode, "domain", "general"),
                    "failure": episode.pointer("/outcome/result_summary").and_then(Value::as_str).unwrap_or(""),
                    "reason": string_field(&episode, "reason", ""),
                    "diagnosis_steps": episode.get("lessons").cloned().unwrap_or_else(|| json!([])),
                    "solution": "",
                    "prevention": "",
                    "source_file": "",
                    "source_episode": episode_id,
                    "related_pattern_ids": [],
                    "created_at": now_string(),
                    "resolved_at": Value::Null,
                }),
                "reflection",
                0.8,
            )?;
            failure_ids.push(failure_id);
        }
        if let Some(object) = episode.as_object_mut() {
            object.insert("reflection_done".to_string(), Value::Bool(true));
            object.insert(
                "pattern_ids_extracted".to_string(),
                json!(pattern_ids.clone()),
            );
            object.insert(
                "failure_ids_extracted".to_string(),
                json!(failure_ids.clone()),
            );
        } else {
            return Err(AdmError::new("episode must be an object"));
        }
        self.store.write_json(path, &episode)?;
        Ok(ReflectionResult {
            episode_id: episode_id.to_string(),
            pattern_ids,
            failure_ids,
        })
    }
}

fn score(value: &Value) -> f64 {
    value.get("score").and_then(Value::as_f64).unwrap_or(0.0)
}

fn string_field(value: &Value, field: &str, fallback: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}
