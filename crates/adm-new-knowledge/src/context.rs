use crate::store::UcosStore;
use adm_new_foundation::{AdmResult, paths::relative_display};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const MAX_TOKENS: usize = 2000;

const PRIORITIES: [(&str, usize, bool); 6] = [
    ("working", 200, false),
    ("identity", 150, false),
    ("active_skills", 300, false),
    ("short_term", 400, true),
    ("episodic", 500, true),
    ("semantic", 450, true),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContextFormat {
    Json,
    AgentsMd,
    Summary,
}

impl ContextFormat {
    pub fn parse(value: &str) -> Self {
        match value {
            "json" => Self::Json,
            "agents-md" | "agents_md" => Self::AgentsMd,
            _ => Self::Summary,
        }
    }
}

pub fn build_context(store: &UcosStore, max_tokens: usize) -> AdmResult<Value> {
    let context = json!({
        "working": store.read_json("knowledge/working/context.json", json!({})),
        "identity": {
            "profile": store.read_json("identity/profile.json", json!({})),
            "constraints": store
                .read_json("identity/constraints.json", json!({}))
                .get("forbidden_actions")
                .cloned()
                .unwrap_or_else(|| json!([])),
            "policy": store.read_json("identity/policy.json", json!({})),
        },
        "active_skills": active_skills(store),
        "short_term": load_entries(store, "knowledge/short_term/entries", 7)?,
        "episodic": load_entries(store, "knowledge/episodic/episodes", 3)?,
        "semantic": store.read_json("knowledge/semantic/facts/domain_devflow.json", json!({})),
    });
    Ok(enforce_budget(context, max_tokens))
}

pub fn estimate_tokens(value: &Value) -> usize {
    let text = if let Some(text) = value.as_str() {
        text.to_string()
    } else {
        serde_json::to_string(value).unwrap_or_default()
    };
    std::cmp::max(1, text.chars().count() / 4)
}

pub fn enforce_budget(mut value: Value, max_tokens: usize) -> Value {
    set_token_estimate(&mut value);
    if token_estimate(&value) <= max_tokens {
        return value;
    }
    loop {
        let mut changed = false;
        for (key, minimum, trim_allowed) in PRIORITIES.iter().rev() {
            if !trim_allowed {
                continue;
            }
            let Some(current) = value.get_mut(*key) else {
                continue;
            };
            if estimate_tokens(current) <= *minimum {
                continue;
            }
            let trimmed = trim_value(current.clone());
            if trimmed != *current {
                *current = trimmed;
                changed = true;
                set_token_estimate(&mut value);
                if token_estimate(&value) <= max_tokens {
                    return value;
                }
            }
        }
        if !changed {
            break;
        }
    }
    value
}

pub fn format_context(context: &Value, format: ContextFormat) -> String {
    match format {
        ContextFormat::Json => serde_json::to_string_pretty(context).unwrap_or_default() + "\n",
        ContextFormat::AgentsMd => format_agents_md(context),
        ContextFormat::Summary => format_summary(context),
    }
}

pub fn context_inventory(store: &UcosStore) -> AdmResult<ContextInventory> {
    Ok(ContextInventory {
        active_skill_count: active_skills(store).as_array().map(Vec::len).unwrap_or(0),
        short_term_loaded: load_entries(store, "knowledge/short_term/entries", 7)?.len(),
        episodic_loaded: load_entries(store, "knowledge/episodic/episodes", 3)?.len(),
        semantic_present: store
            .resolve("knowledge/semantic/facts/domain_devflow.json")
            .exists(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextInventory {
    pub active_skill_count: usize,
    pub short_term_loaded: usize,
    pub episodic_loaded: usize,
    pub semantic_present: bool,
}

fn active_skills(store: &UcosStore) -> Value {
    let registry = store.read_json("capability/registry.json", json!({}));
    let skills = registry
        .get("skills")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .take(5)
        .collect::<Vec<_>>();
    Value::Array(skills)
}

fn load_entries(store: &UcosStore, relative: &str, limit: usize) -> AdmResult<Vec<Value>> {
    let mut paths = store.json_files(relative, false)?;
    paths.sort_by_key(|path| {
        std::cmp::Reverse(
            path.metadata()
                .and_then(|metadata| metadata.modified())
                .ok(),
        )
    });
    let mut entries = Vec::new();
    for path in paths.into_iter().take(limit) {
        let text = std::fs::read_to_string(&path)?;
        let mut value: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({}));
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "_relative_path".to_string(),
                Value::String(relative_display(&path, store.project_root())),
            );
        }
        entries.push(value);
    }
    Ok(entries)
}

fn set_token_estimate(value: &mut Value) {
    let mut without = value.clone();
    if let Some(object) = without.as_object_mut() {
        object.remove("token_estimate");
    }
    let tokens = estimate_tokens(&without);
    if let Some(object) = value.as_object_mut() {
        object.insert("token_estimate".to_string(), json!(tokens));
    }
}

fn token_estimate(value: &Value) -> usize {
    value
        .get("token_estimate")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| estimate_tokens(value) as u64) as usize
}

fn trim_value(value: Value) -> Value {
    match value {
        Value::Array(items) if items.len() > 1 => {
            let keep = std::cmp::max(1, items.len() / 2);
            Value::Array(items.into_iter().take(keep).collect())
        }
        Value::Object(map) if map.len() > 1 => {
            let keep = std::cmp::max(1, map.len() / 2);
            Value::Object(map.into_iter().take(keep).collect())
        }
        Value::Object(mut map) if map.len() == 1 => {
            let key = map.keys().next().cloned().unwrap_or_default();
            let original = map.get(&key).cloned().unwrap_or(Value::Null);
            let trimmed = trim_value(original.clone());
            if trimmed != original {
                map.insert(key, trimmed);
            }
            Value::Object(map)
        }
        Value::String(text) if text.chars().count() > 200 => Value::String(
            text.chars()
                .take(std::cmp::max(200, text.chars().count() / 2))
                .collect(),
        ),
        other => other,
    }
}

fn format_agents_md(context: &Value) -> String {
    let working = context.get("working").unwrap_or(&Value::Null);
    let profile = context.pointer("/identity/profile").unwrap_or(&Value::Null);
    let constraints = context
        .pointer("/identity/constraints")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut lines = vec![
        "# UCOS Entry".to_string(),
        String::new(),
        "## Current State".to_string(),
        format!("- Domain: {}", string_field(working, "domain")),
        format!(
            "- Active Save: {}",
            string_field(working, "active_save_name")
        ),
        format!(
            "- Progress: {}",
            working
                .get("pipeline_progress")
                .map(Value::to_string)
                .unwrap_or_else(|| "{}".to_string())
        ),
        String::new(),
        "## Identity".to_string(),
        format!("- Role: {}", string_field(profile, "role")),
        format!(
            "- Principles: {}",
            string_array(profile, "principles").join(", ")
        ),
        String::new(),
        "## Constraints".to_string(),
    ];
    for item in constraints {
        lines.push(format!(
            "- {}: {}",
            string_field(&item, "action"),
            string_array(&item, "targets").join(", ")
        ));
    }
    lines.join("\n") + "\n"
}

fn format_summary(context: &Value) -> String {
    let working = context.get("working").unwrap_or(&Value::Null);
    let profile = context.pointer("/identity/profile").unwrap_or(&Value::Null);
    [
        "# UCOS Session Summary".to_string(),
        format!("- domain: {}", string_field(working, "domain")),
        format!(
            "- active_save: {} ({})",
            string_field(working, "active_save_name"),
            string_field(working, "active_save_id")
        ),
        format!(
            "- progress: {}",
            working
                .get("pipeline_progress")
                .map(Value::to_string)
                .unwrap_or_else(|| "{}".to_string())
        ),
        format!("- role: {}", string_field(profile, "role")),
        format!(
            "- token_estimate: {}",
            context
                .get("token_estimate")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        ),
    ]
    .join("\n")
        + "\n"
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
