use crate::store::UcosStore;
use adm_new_foundation::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillSpec {
    pub skill_id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub skill_type: String,
    pub level: i64,
    pub version: String,
    pub status: String,
    pub domain: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub dependencies: Vec<String>,
    pub trigger_rule: Value,
    pub input_schema: Value,
    pub output_schema: Value,
    pub anti_patterns: Vec<String>,
    pub episode_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillDependencyReport {
    pub skill_id: String,
    pub dependencies: Vec<String>,
    pub has_cycle: bool,
}

#[derive(Debug, Clone)]
pub struct SkillEngine {
    store: UcosStore,
}

impl SkillEngine {
    pub fn new(store: UcosStore) -> Self {
        Self { store }
    }

    pub fn load_specs(&self) -> AdmResult<Vec<SkillSpec>> {
        let mut specs = Vec::new();
        for path in self.store.json_files("capability/skills", true)? {
            if let Some(spec) = parse_skill_file(&path)? {
                specs.push(spec);
            }
        }
        for path in self.store.json_files("plugins", true)? {
            let components = path
                .components()
                .map(|component| component.as_os_str().to_string_lossy().to_string())
                .collect::<Vec<_>>();
            if components.iter().any(|part| part == "skills")
                && let Some(spec) = parse_skill_file(&path)?
            {
                specs.push(spec);
            }
        }
        specs.sort_by(|left, right| left.skill_id.cmp(&right.skill_id));
        Ok(specs)
    }

    pub fn discover(
        &self,
        context_tags: &[String],
        inputs: &BTreeMap<String, Value>,
    ) -> AdmResult<Vec<SkillSpec>> {
        let tags = context_tags.iter().cloned().collect::<BTreeSet<_>>();
        let input_keys = inputs.keys().cloned().collect::<BTreeSet<_>>();
        let mut matches = Vec::new();
        for spec in self.load_specs()? {
            if spec.status != "active" {
                continue;
            }
            let required_tags = string_array_pointer(&spec.trigger_rule, "/required_context_tags");
            let required_inputs = string_array_pointer(&spec.trigger_rule, "/required_inputs");
            if required_tags.iter().all(|tag| tags.contains(tag))
                && required_inputs.iter().all(|key| input_keys.contains(key))
            {
                matches.push(spec);
            }
        }
        Ok(matches)
    }

    pub fn dependency_report(&self, skill_id: &str) -> SkillDependencyReport {
        let graph = self
            .store
            .read_json("capability/dependency_graph.json", json!({"edges": {}}));
        let edges = graph
            .get("edges")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let dependencies = edges
            .get(skill_id)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        SkillDependencyReport {
            skill_id: skill_id.to_string(),
            dependencies,
            has_cycle: has_cycle(&edges, skill_id, &mut BTreeSet::new(), &mut BTreeSet::new()),
        }
    }

    pub fn version_history(&self, skill_id: &str) -> AdmResult<Vec<SkillSpec>> {
        let prefix = skill_id
            .rsplit_once("_v")
            .map(|(prefix, _)| prefix)
            .unwrap_or(skill_id);
        Ok(self
            .load_specs()?
            .into_iter()
            .filter(|spec| spec.skill_id.starts_with(prefix))
            .collect())
    }
}

fn parse_skill_file(path: &std::path::Path) -> AdmResult<Option<SkillSpec>> {
    let text = std::fs::read_to_string(path)?;
    let data: Value = serde_json::from_str(&text).map_err(|error| {
        AdmError::new(format!(
            "failed to parse skill json {}: {error}",
            path.display()
        ))
    })?;
    if data.get("skill_id").is_none() {
        return Ok(None);
    }
    let spec = SkillSpec {
        skill_id: string_field(&data, "skill_id"),
        name: string_field(&data, "name"),
        skill_type: string_field(&data, "type"),
        level: data.get("level").and_then(Value::as_i64).unwrap_or(0),
        version: string_field(&data, "version"),
        status: string_field(&data, "status"),
        domain: string_field(&data, "domain"),
        description: string_field(&data, "description"),
        capabilities: string_array(&data, "capabilities"),
        dependencies: string_array(&data, "dependencies"),
        trigger_rule: data
            .get("trigger_rule")
            .cloned()
            .unwrap_or_else(|| json!({})),
        input_schema: data
            .get("input_schema")
            .cloned()
            .unwrap_or_else(|| json!({})),
        output_schema: data
            .get("output_schema")
            .cloned()
            .unwrap_or_else(|| json!({})),
        anti_patterns: string_array(&data, "anti_patterns"),
        episode_refs: string_array(&data, "episode_refs"),
    };
    Ok(Some(spec))
}

fn has_cycle(
    edges: &serde_json::Map<String, Value>,
    node: &str,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> bool {
    if visiting.contains(node) {
        return true;
    }
    if visited.contains(node) {
        return false;
    }
    visiting.insert(node.to_string());
    for dep in edges
        .get(node)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if has_cycle(edges, dep, visiting, visited) {
            return true;
        }
    }
    visiting.remove(node);
    visited.insert(node.to_string());
    false
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn string_array(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
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

fn string_array_pointer(value: &Value, pointer: &str) -> Vec<String> {
    value
        .pointer(pointer)
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
