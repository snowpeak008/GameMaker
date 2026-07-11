use crate::store::UcosStore;
use adm_new_foundation::{AdmError, AdmResult, new_stable_id, unix_timestamp};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTier {
    Working,
    ShortTerm,
    Episodic,
    Semantic,
    Patterns,
    Failures,
}

impl MemoryTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::ShortTerm => "short_term",
            Self::Episodic => "episodic",
            Self::Semantic => "semantic",
            Self::Patterns => "patterns",
            Self::Failures => "failures",
        }
    }

    pub fn parse(value: &str) -> AdmResult<Self> {
        match value.trim() {
            "working" => Ok(Self::Working),
            "short_term" => Ok(Self::ShortTerm),
            "episodic" => Ok(Self::Episodic),
            "semantic" => Ok(Self::Semantic),
            "patterns" | "pattern" => Ok(Self::Patterns),
            "failures" | "failure" => Ok(Self::Failures),
            other => Err(AdmError::new(format!("unknown memory tier: {other}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub entry_id: String,
    pub tier: MemoryTier,
    pub content: Value,
    pub relevance: f64,
}

#[derive(Debug, Clone)]
pub struct MemoryEngine {
    store: UcosStore,
}

impl MemoryEngine {
    pub fn new(store: UcosStore) -> Self {
        Self { store }
    }

    pub fn store(&self) -> &UcosStore {
        &self.store
    }

    pub fn write(
        &self,
        tier: MemoryTier,
        content: Value,
        source: &str,
        importance: f64,
    ) -> AdmResult<String> {
        let now = now_string();
        match tier {
            MemoryTier::Working => {
                let mut data = self
                    .store
                    .read_json("knowledge/working/context.json", json!({}));
                merge_objects(&mut data, &content);
                set_object_field(&mut data, "updated_at", Value::String(now));
                self.store
                    .write_json("knowledge/working/context.json", &data)?;
                Ok(data
                    .get("session_id")
                    .and_then(Value::as_str)
                    .unwrap_or("working_context")
                    .to_string())
            }
            MemoryTier::ShortTerm => self.write_short_term(content, source, importance, &now),
            MemoryTier::Semantic => self.write_semantic(content, source, importance, &now),
            MemoryTier::Episodic | MemoryTier::Patterns | MemoryTier::Failures => {
                self.write_entry(tier, content, &now)
            }
        }
    }

    pub fn query(
        &self,
        tier: MemoryTier,
        keywords: &[String],
        top_k: usize,
        min_decay: f64,
    ) -> AdmResult<Vec<MemoryEntry>> {
        let mut scored = Vec::new();
        for path in self.iter_entry_paths(tier)? {
            let content = read_json_path(&path)?;
            let text = serde_json::to_string(&content)
                .unwrap_or_default()
                .to_lowercase();
            let score = keywords
                .iter()
                .filter(|keyword| text.contains(&keyword.to_lowercase()))
                .count();
            let relevance = content
                .get("current_relevance")
                .or_else(|| content.get("importance"))
                .and_then(Value::as_f64)
                .unwrap_or(1.0);
            if score > 0 && relevance >= min_decay {
                let entry_id = entry_id_from_value(&content).unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|value| value.to_str())
                        .unwrap_or_default()
                        .to_string()
                });
                scored.push((
                    score,
                    relevance,
                    MemoryEntry {
                        entry_id,
                        tier,
                        content,
                        relevance,
                    },
                ));
            }
        }
        scored.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| right.1.total_cmp(&left.1))
        });
        Ok(scored
            .into_iter()
            .take(top_k)
            .map(|(_, _, entry)| entry)
            .collect())
    }

    pub fn decay_pass(&self, tier: MemoryTier) -> AdmResult<usize> {
        if tier != MemoryTier::ShortTerm {
            return Ok(0);
        }
        let mut changed = 0;
        for path in self.iter_entry_paths(tier)? {
            let mut data = read_json_path(&path)?;
            let importance = data
                .get("importance")
                .and_then(Value::as_f64)
                .unwrap_or(0.5);
            let decay_rate = data
                .get("decay_rate")
                .and_then(Value::as_f64)
                .unwrap_or(0.05);
            let current = data
                .get("current_relevance")
                .and_then(Value::as_f64)
                .unwrap_or(importance);
            let relevance = (current * (1.0 - decay_rate)).max(0.0);
            set_object_field(
                &mut data,
                "current_relevance",
                json!((relevance * 10000.0).round() / 10000.0),
            );
            set_object_field(
                &mut data,
                "consolidate_to_episodic",
                Value::Bool(relevance < 0.3 && importance >= 0.7),
            );
            write_json_path(&path, &data)?;
            changed += 1;
        }
        Ok(changed)
    }

    fn write_short_term(
        &self,
        content: Value,
        source: &str,
        importance: f64,
        now: &str,
    ) -> AdmResult<String> {
        let entry_id = content
            .get("stm_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| new_stable_id("stm").unwrap_or_else(|_| "stm_entry".to_string()));
        let entry = json!({
            "schema_version": "1.0",
            "stm_id": entry_id,
            "type": string_or(&content, "type", "observation"),
            "title": string_or(&content, "title", ""),
            "content": string_or(&content, "content", ""),
            "source": {
                "type": source,
                "session_id": string_or(&content, "session_id", ""),
                "file_ref": string_or(&content, "file_ref", ""),
            },
            "tags": content.get("tags").cloned().unwrap_or_else(|| json!([])),
            "importance": importance,
            "created_at": content.get("created_at").and_then(Value::as_str).unwrap_or(now),
            "last_accessed": now,
            "decay_rate": content.get("decay_rate").and_then(Value::as_f64).unwrap_or(0.05),
            "current_relevance": importance,
            "consolidate_to_episodic": false,
        });
        let path = format!("knowledge/short_term/entries/{entry_id}.json");
        self.store.write_json(&path, &entry)?;
        self.upsert_index(MemoryTier::ShortTerm, &entry_id, &path)?;
        Ok(entry_id)
    }

    fn write_semantic(
        &self,
        content: Value,
        source: &str,
        importance: f64,
        now: &str,
    ) -> AdmResult<String> {
        let entry_id = content
            .get("fact_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                let domain = string_or(&content, "domain", "general");
                new_stable_id(&format!("sf_{domain}")).unwrap_or_else(|_| "sf_general".to_string())
            });
        let mut staged = object_or_empty(content);
        staged
            .entry("schema_version".to_string())
            .or_insert_with(|| Value::String("1.0".to_string()));
        staged
            .entry("fact_id".to_string())
            .or_insert_with(|| Value::String(entry_id.clone()));
        staged
            .entry("type".to_string())
            .or_insert_with(|| Value::String("fact".to_string()));
        staged.entry("source".to_string()).or_insert_with(|| {
            json!({
                "type": source,
                "ref": "",
                "episode_id": "",
            })
        });
        staged
            .entry("confidence".to_string())
            .or_insert_with(|| json!(importance));
        staged.insert("review_required".to_string(), Value::Bool(true));
        staged
            .entry("version".to_string())
            .or_insert_with(|| json!(1));
        staged
            .entry("last_verified".to_string())
            .or_insert_with(|| Value::String(now.to_string()));
        staged
            .entry("tags".to_string())
            .or_insert_with(|| json!([]));
        staged
            .entry("created_at".to_string())
            .or_insert_with(|| Value::String(now.to_string()));
        self.store.write_json(
            format!("knowledge/semantic/staging/staged_{entry_id}.json"),
            &Value::Object(staged),
        )?;
        Ok(entry_id)
    }

    fn write_entry(&self, tier: MemoryTier, content: Value, now: &str) -> AdmResult<String> {
        let mut data = object_or_empty(content);
        let entry_id = data
            .get("episode_id")
            .or_else(|| data.get("pattern_id"))
            .or_else(|| data.get("failure_id"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                new_stable_id(tier.as_str()).unwrap_or_else(|_| format!("{}_entry", tier.as_str()))
            });
        data.entry("created_at".to_string())
            .or_insert_with(|| Value::String(now.to_string()));
        let relative_path = entry_relative_path(tier, &entry_id)?;
        self.store
            .write_json(&relative_path, &Value::Object(data))?;
        self.upsert_index(tier, &entry_id, &relative_path)?;
        Ok(entry_id)
    }

    fn upsert_index(&self, tier: MemoryTier, entry_id: &str, entry_path: &str) -> AdmResult<()> {
        let Some(index_path) = index_relative_path(tier) else {
            return Ok(());
        };
        let mut index = self.store.read_json(
            index_path,
            json!({
                "schema_version": "1.0",
                "entries": []
            }),
        );
        let mut entries = index
            .get("entries")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|item| item.get("id").and_then(Value::as_str) != Some(entry_id))
            .collect::<Vec<_>>();
        entries.push(json!({
            "id": entry_id,
            "path": format!("ucos/{entry_path}"),
            "updated_at": now_string(),
        }));
        set_object_field(&mut index, "entries", Value::Array(entries));
        self.store.write_json(index_path, &index)?;
        Ok(())
    }

    fn iter_entry_paths(&self, tier: MemoryTier) -> AdmResult<Vec<PathBuf>> {
        let relative = match tier {
            MemoryTier::ShortTerm => "knowledge/short_term/entries",
            MemoryTier::Episodic => "knowledge/episodic/episodes",
            MemoryTier::Patterns => "knowledge/patterns/entries",
            MemoryTier::Failures => "knowledge/failures/entries",
            MemoryTier::Semantic => "knowledge/semantic/staging",
            MemoryTier::Working => return Ok(Vec::new()),
        };
        self.store.json_files(relative, false)
    }
}

fn index_relative_path(tier: MemoryTier) -> Option<&'static str> {
    match tier {
        MemoryTier::ShortTerm => Some("knowledge/short_term/index.json"),
        MemoryTier::Episodic => Some("knowledge/episodic/index.json"),
        MemoryTier::Patterns => Some("knowledge/patterns/index.json"),
        MemoryTier::Failures => Some("knowledge/failures/index.json"),
        _ => None,
    }
}

fn entry_relative_path(tier: MemoryTier, entry_id: &str) -> AdmResult<String> {
    match tier {
        MemoryTier::Episodic => Ok(format!("knowledge/episodic/episodes/{entry_id}.json")),
        MemoryTier::Patterns => Ok(format!("knowledge/patterns/entries/{entry_id}.json")),
        MemoryTier::Failures => Ok(format!("knowledge/failures/entries/{entry_id}.json")),
        other => Err(AdmError::new(format!(
            "tier {} does not use generic entry paths",
            other.as_str()
        ))),
    }
}

fn object_or_empty(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn merge_objects(target: &mut Value, source: &Value) {
    let target_object = if let Some(object) = target.as_object_mut() {
        object
    } else {
        *target = json!({});
        target.as_object_mut().expect("object")
    };
    if let Some(source_object) = source.as_object() {
        for (key, value) in source_object {
            target_object.insert(key.clone(), value.clone());
        }
    }
}

fn set_object_field(target: &mut Value, key: &str, value: Value) {
    if !target.is_object() {
        *target = json!({});
    }
    target
        .as_object_mut()
        .expect("object")
        .insert(key.to_string(), value);
}

fn string_or(value: &Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

fn entry_id_from_value(value: &Value) -> Option<String> {
    for key in [
        "stm_id",
        "episode_id",
        "pattern_id",
        "failure_id",
        "fact_id",
    ] {
        if let Some(id) = value.get(key).and_then(Value::as_str) {
            return Some(id.to_string());
        }
    }
    None
}

fn read_json_path(path: &std::path::Path) -> AdmResult<Value> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text)
        .map_err(|error| AdmError::new(format!("failed to parse json {}: {error}", path.display())))
}

fn write_json_path(path: &std::path::Path, value: &Value) -> AdmResult<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| AdmError::new(format!("failed to serialize json: {error}")))?;
    std::fs::write(path, format!("{text}\n"))?;
    Ok(())
}

pub fn now_string() -> String {
    format!("unix:{}", unix_timestamp())
}
