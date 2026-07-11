use crate::store::UcosStore;
use adm_new_foundation::AdmResult;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityProfile {
    pub identity_id: String,
    pub role: String,
    pub principles: Vec<String>,
    pub philosophy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionValidation {
    pub allowed: bool,
    pub reason: String,
}

#[derive(Debug, Clone)]
pub struct IdentityEngine {
    store: UcosStore,
}

impl IdentityEngine {
    pub fn new(store: UcosStore) -> Self {
        Self { store }
    }

    pub fn load_profile(&self) -> AdmResult<IdentityProfile> {
        let data = self.store.read_required_json("identity/profile.json")?;
        Ok(IdentityProfile {
            identity_id: string_field(&data, "identity_id"),
            role: string_field(&data, "role"),
            principles: string_array(&data, "principles"),
            philosophy: string_field(&data, "philosophy"),
        })
    }

    pub fn get_principles(&self) -> AdmResult<Vec<String>> {
        Ok(self.load_profile()?.principles)
    }

    pub fn get_autonomy_level(&self) -> i64 {
        self.store
            .read_json("identity/policy.json", json!({}))
            .get("autonomy_level")
            .and_then(Value::as_i64)
            .unwrap_or(0)
    }

    pub fn validate_action(&self, action: &Value) -> ActionValidation {
        let constraints = self.store.read_json(
            "identity/constraints.json",
            json!({"forbidden_actions": []}),
        );
        let target = string_field(action, "target");
        let target = if target.trim().is_empty() {
            string_field(action, "path")
        } else {
            target
        };
        let action_type = string_field(action, "type");
        let action_type = if action_type.trim().is_empty() {
            string_field(action, "action")
        } else {
            action_type
        };

        for rule in constraints
            .get("forbidden_actions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let rule_action = string_field(rule, "action");
            let targets = string_array(rule, "targets");
            let reason = string_field_or(rule, "reason", "forbidden action");
            let matches = matches_any(&target, &targets);
            if rule_action == "edit_generated_files"
                && matches
                && ["edit", "write", "delete"].contains(&action_type.as_str())
            {
                return ActionValidation {
                    allowed: false,
                    reason,
                };
            }
            if rule_action == "delete_registry" && matches && action_type == "delete" {
                return ActionValidation {
                    allowed: false,
                    reason,
                };
            }
            if rule_action == "restore_deprecated"
                && matches
                && ["create", "restore", "write"].contains(&action_type.as_str())
            {
                return ActionValidation {
                    allowed: false,
                    reason,
                };
            }
            if rule_action == "bypass_orchestrator"
                && matches
                && ["write", "edit", "create"].contains(&action_type.as_str())
            {
                return ActionValidation {
                    allowed: false,
                    reason,
                };
            }
        }
        ActionValidation {
            allowed: true,
            reason: "allowed".to_string(),
        }
    }
}

fn matches_any(target: &str, patterns: &[String]) -> bool {
    let normalized = target.replace('\\', "/");
    let name = normalized.rsplit('/').next().unwrap_or(&normalized);
    for pattern in patterns {
        let pattern = pattern.replace('\\', "/");
        if wildcard_match(&pattern, &normalized)
            || wildcard_match(&pattern, name)
            || normalized.ends_with(&pattern)
        {
            return true;
        }
    }
    false
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == value {
        return true;
    }
    if !pattern.contains('*') {
        return false;
    }
    let mut remainder = value;
    let anchored_start = !pattern.starts_with('*');
    let anchored_end = !pattern.ends_with('*');
    let parts = pattern
        .split('*')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return true;
    }
    if anchored_start && !value.starts_with(parts[0]) {
        return false;
    }
    if anchored_end && !value.ends_with(parts[parts.len() - 1]) {
        return false;
    }
    for part in parts {
        let Some(index) = remainder.find(part) else {
            return false;
        };
        remainder = &remainder[index + part.len()..];
    }
    true
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn string_field_or(value: &Value, field: &str, fallback: &str) -> String {
    let value = string_field(value, field);
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
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
