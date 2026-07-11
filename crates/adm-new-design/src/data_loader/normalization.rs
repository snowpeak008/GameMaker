use crate::data_loader::{
    DEFAULT_DOMAIN_SCHEMA_VERSION, DEFAULT_ROLE_CLASS, EntitySchemaRegistry, ROLE_CLASS_VALUES,
    SharedTemplate, mda_layer_label, string_from_value, valid_relation_type,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

pub fn normalize_domain(
    mut domain_doc: Value,
    templates: &BTreeMap<String, SharedTemplate>,
    registry: &EntitySchemaRegistry,
) -> Value {
    if !domain_doc.is_object() {
        return domain_doc;
    }

    let mut node_ids = Vec::new();
    let domain_id = {
        let root = domain_doc.as_object_mut().expect("checked object");
        root.entry("schemaVersion".to_string())
            .or_insert_with(|| Value::String(DEFAULT_DOMAIN_SCHEMA_VERSION.to_string()));
        let domain = ensure_object_entry(root, "domain");
        domain
            .entry("priority".to_string())
            .or_insert_with(|| Value::String("P0".to_string()));
        domain
            .entry("activation".to_string())
            .or_insert_with(|| Value::String("always".to_string()));
        string_from_value(domain.get("id"))
    };

    let mut role_class_warnings = Vec::new();
    let mut entity_validation_warnings = Vec::new();
    let mut template_warnings = Vec::new();
    if let Some(nodes) = domain_doc
        .as_object_mut()
        .and_then(|root| root.get_mut("nodes"))
        .and_then(Value::as_array_mut)
    {
        for node_value in nodes.iter_mut() {
            let Some(node) = node_value.as_object_mut() else {
                continue;
            };
            for field in [
                "requires",
                "unlocks",
                "recommendedBefore",
                "requiresAny",
                "conflictsWith",
            ] {
                node.entry(field.to_string()).or_insert_with(|| json!([]));
            }
            node.entry("domain".to_string())
                .or_insert_with(|| Value::String(domain_id.clone()));
            let node_id = string_from_value(node.get("id"));
            if !node_id.is_empty() {
                node_ids.push(node_id.clone());
            }
            apply_requirement_metadata(node);

            let (role_class, warning) = normalize_role_class(node.get("roleClass"));
            node.insert("roleClass".to_string(), Value::String(role_class));
            if let Some(warning) = warning {
                role_class_warnings.push(format!("{domain_id}.{node_id}: {warning}"));
            }

            let (entities, entity_errors) = registry.normalize_design_entities(
                node.get("designEntities"),
                &format!("{domain_id}.{node_id}"),
            );
            node.insert("designEntities".to_string(), Value::Array(entities));
            node.insert(
                "entityValidationErrors".to_string(),
                serde_json::to_value(&entity_errors).unwrap_or_else(|_| json!([])),
            );
            for error in &entity_errors {
                entity_validation_warnings.push(format!("{}: {}", error.path, error.message));
            }

            normalize_checklist(node, &node_id, templates);
            if let Some(warnings) = node
                .get("_templateWarnings")
                .and_then(Value::as_array)
                .cloned()
            {
                for warning in warnings {
                    if let Some(text) = warning.as_str() {
                        template_warnings.push(format!("{domain_id}.{node_id}: {text}"));
                    }
                }
            }
        }
    }

    let root = domain_doc.as_object_mut().expect("checked object");
    if !role_class_warnings.is_empty() {
        root.insert("_roleClassWarnings".to_string(), json!(role_class_warnings));
    }
    if !entity_validation_warnings.is_empty() {
        root.insert(
            "_entityValidationWarnings".to_string(),
            json!(entity_validation_warnings),
        );
    }
    if !template_warnings.is_empty() {
        root.insert("_templateWarnings".to_string(), json!(template_warnings));
    }

    let coverage = ensure_object_entry(root, "coverageStandard");
    coverage
        .entry("domain".to_string())
        .or_insert_with(|| Value::String(domain_id));
    coverage
        .entry("unit".to_string())
        .or_insert_with(|| Value::String("nodes_and_checklist".to_string()));
    coverage
        .entry("requiredItems".to_string())
        .or_insert_with(|| json!(node_ids));
    let expected = coverage
        .get("requiredItems")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    coverage
        .entry("expected".to_string())
        .or_insert_with(|| json!(expected));
    coverage.entry("formula".to_string()).or_insert_with(|| {
        Value::String("completed_or_partial_nodes / applicable_required_items".to_string())
    });
    domain_doc
}

fn normalize_role_class(value: Option<&Value>) -> (String, Option<String>) {
    let raw_value = string_from_value(value);
    let raw_value = raw_value.trim();
    if raw_value.is_empty() {
        return (DEFAULT_ROLE_CLASS.to_string(), None);
    }
    if ROLE_CLASS_VALUES.contains(&raw_value) {
        return (raw_value.to_string(), None);
    }
    (
        DEFAULT_ROLE_CLASS.to_string(),
        Some(format!(
            "unknown roleClass {raw_value:?}; defaulted to {DEFAULT_ROLE_CLASS}"
        )),
    )
}

fn normalize_checklist(
    node: &mut Map<String, Value>,
    node_id: &str,
    templates: &BTreeMap<String, SharedTemplate>,
) {
    let source_items = node
        .get("checklist")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut checklist = Vec::new();
    let mut template_warnings = Vec::new();

    for (index, source_item) in source_items.into_iter().enumerate() {
        let legacy_id = format!("{node_id}_item_{}", index + 1);
        let mut item = Map::new();
        if let Some(source) = source_item.as_object() {
            let mut source = source.clone();
            let item_id = string_from_value(source.get("id"));
            let item_id = if item_id.trim().is_empty() {
                legacy_id.clone()
            } else {
                item_id
            };
            let label = string_from_value(source.get("label"));
            let description = string_from_value(source.get("description"));
            let output_key = string_from_value(source.get("outputKey"));
            let mut legacy_ids = normalize_legacy_ids(source.get("legacyIds"));
            if legacy_id != item_id && !legacy_ids.iter().any(|value| value == &legacy_id) {
                legacy_ids.push(legacy_id);
            }
            let template_ref = string_from_value(
                source
                    .get("templateRef")
                    .or_else(|| source.get("template_ref")),
            );
            if !template_ref.trim().is_empty() {
                source.insert(
                    "templateRef".to_string(),
                    Value::String(template_ref.clone()),
                );
                template_warnings.extend(resolve_template_ref(&mut source, templates));
            }
            item.insert("id".to_string(), Value::String(item_id.clone()));
            item.insert(
                "label".to_string(),
                Value::String(if label.trim().is_empty() {
                    item_id.clone()
                } else {
                    label
                }),
            );
            item.insert("description".to_string(), Value::String(description));
            item.insert(
                "outputKey".to_string(),
                Value::String(if output_key.trim().is_empty() {
                    camel_case(&item_id)
                } else {
                    output_key
                }),
            );
            item.insert("legacyIds".to_string(), json!(legacy_ids));
            item.insert("templateRef".to_string(), Value::String(template_ref));
            item.insert(
                "optionGroups".to_string(),
                source
                    .get("optionGroups")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            );
            item.insert(
                "optionRelations".to_string(),
                source
                    .get("optionRelations")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            );
        } else {
            let label = string_from_value(Some(&source_item));
            item.insert("id".to_string(), Value::String(legacy_id.clone()));
            item.insert("label".to_string(), Value::String(label));
            item.insert("description".to_string(), Value::String(String::new()));
            item.insert(
                "outputKey".to_string(),
                Value::String(camel_case(&legacy_id)),
            );
            item.insert("legacyIds".to_string(), json!([]));
            item.insert("templateRef".to_string(), Value::String(String::new()));
            item.insert("optionGroups".to_string(), json!([]));
            item.insert("optionRelations".to_string(), json!([]));
        }
        normalize_option_groups(&mut item);
        normalize_option_relations(&mut item);
        checklist.push(Value::Object(item));
    }

    node.insert("checklist".to_string(), Value::Array(checklist));
    if !template_warnings.is_empty() {
        node.insert("_templateWarnings".to_string(), json!(template_warnings));
    }
}

fn resolve_template_ref(
    item: &mut Map<String, Value>,
    templates: &BTreeMap<String, SharedTemplate>,
) -> Vec<String> {
    let template_ref =
        string_from_value(item.get("templateRef").or_else(|| item.get("template_ref")));
    if template_ref.trim().is_empty() {
        return Vec::new();
    }
    let Some(template) = templates.get(&template_ref) else {
        item.entry("optionGroups".to_string())
            .or_insert_with(|| json!([]));
        item.entry("optionRelations".to_string())
            .or_insert_with(|| json!([]));
        return vec![format!("templateRef {template_ref:?} does not exist")];
    };
    let should_fill_groups = item
        .get("optionGroups")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty);
    if should_fill_groups {
        item.insert(
            "optionGroups".to_string(),
            template
                .raw
                .get("optionGroups")
                .cloned()
                .unwrap_or_else(|| json!([])),
        );
    }
    let should_fill_relations = item
        .get("optionRelations")
        .and_then(Value::as_array)
        .is_none_or(Vec::is_empty);
    if should_fill_relations {
        item.insert(
            "optionRelations".to_string(),
            template
                .raw
                .get("optionRelations")
                .cloned()
                .unwrap_or_else(|| json!([])),
        );
    }
    Vec::new()
}

fn normalize_option_groups(item: &mut Map<String, Value>) {
    let source_groups = item
        .get("optionGroups")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut groups = Vec::new();
    for group in source_groups {
        let Some(source) = group.as_object() else {
            continue;
        };
        let group_id = string_from_value(source.get("id").or_else(|| source.get("label")));
        let label = string_from_value(source.get("label"));
        let mda_layer = string_from_value(source.get("mdaLayer"));
        let mut normalized = Map::new();
        normalized.insert("id".to_string(), Value::String(group_id.clone()));
        normalized.insert(
            "label".to_string(),
            Value::String(if label.trim().is_empty() {
                group_id.clone()
            } else {
                label
            }),
        );
        normalized.insert(
            "description".to_string(),
            Value::String(string_from_value(source.get("description"))),
        );
        let output_key = string_from_value(source.get("outputKey"));
        normalized.insert(
            "outputKey".to_string(),
            Value::String(if output_key.trim().is_empty() {
                camel_case(&group_id)
            } else {
                output_key
            }),
        );
        let selection_mode = string_from_value(source.get("selectionMode"));
        normalized.insert(
            "selectionMode".to_string(),
            Value::String(match selection_mode.as_str() {
                "single" | "multi" => selection_mode,
                _ => "multi".to_string(),
            }),
        );
        normalized.insert(
            "required".to_string(),
            Value::Bool(
                source
                    .get("required")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            ),
        );
        normalized.insert(
            "allowPrimary".to_string(),
            Value::Bool(
                source
                    .get("allowPrimary")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            ),
        );
        normalized.insert("mdaLayer".to_string(), Value::String(mda_layer.clone()));
        let mda_label = string_from_value(source.get("mdaLayerLabel"));
        normalized.insert(
            "mdaLayerLabel".to_string(),
            Value::String(if mda_label.trim().is_empty() {
                mda_layer_label(&mda_layer)
            } else {
                mda_label
            }),
        );
        normalized.insert(
            "progressionStep".to_string(),
            json!(
                source
                    .get("progressionStep")
                    .and_then(Value::as_i64)
                    .or_else(|| {
                        string_from_value(source.get("progressionStep"))
                            .parse::<i64>()
                            .ok()
                    })
                    .unwrap_or(0)
            ),
        );
        normalized.insert(
            "relation".to_string(),
            Value::String(string_from_value(source.get("relation"))),
        );
        normalized.insert(
            "designQuestion".to_string(),
            Value::String(string_from_value(source.get("designQuestion"))),
        );
        normalized.insert(
            "options".to_string(),
            source.get("options").cloned().unwrap_or_else(|| json!([])),
        );
        normalize_group_options(&mut normalized);
        groups.push(Value::Object(normalized));
    }
    item.insert("optionGroups".to_string(), Value::Array(groups));
}

fn normalize_group_options(group: &mut Map<String, Value>) {
    let source_options = group
        .get("options")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut options = Vec::new();
    for option in source_options {
        let (option_id, label, description, output_key) = if let Some(source) = option.as_object() {
            let option_id = string_from_value(source.get("id").or_else(|| source.get("label")));
            let label = string_from_value(source.get("label"));
            let output_key = string_from_value(source.get("outputKey"));
            (
                option_id.clone(),
                if label.trim().is_empty() {
                    option_id.clone()
                } else {
                    label
                },
                string_from_value(source.get("description")),
                if output_key.trim().is_empty() {
                    camel_case(&option_id)
                } else {
                    output_key
                },
            )
        } else {
            let option_id = string_from_value(Some(&option));
            let output_key = camel_case(&option_id);
            (option_id.clone(), option_id, String::new(), output_key)
        };
        options.push(json!({
            "id": option_id,
            "label": label,
            "description": description,
            "outputKey": output_key,
        }));
    }
    group.insert("options".to_string(), Value::Array(options));
}

fn normalize_option_relations(item: &mut Map<String, Value>) {
    let source_relations = item
        .get("optionRelations")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut relations = Vec::new();
    for relation in source_relations {
        let Some(source_relation) = relation.as_object() else {
            continue;
        };
        let relation_type = string_from_value(source_relation.get("type"));
        let relation_type = if relation_type.trim().is_empty() {
            "soft_conflict".to_string()
        } else {
            relation_type
        };
        let Some(source) = normalize_option_ref(source_relation.get("source")) else {
            continue;
        };
        let targets = source_relation
            .get("targets")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| normalize_option_ref(Some(item)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !valid_relation_type(&relation_type) || targets.is_empty() {
            continue;
        }
        let relation_id = string_from_value(source_relation.get("id"));
        let relation_id = if relation_id.trim().is_empty() {
            format!(
                "{relation_type}_{}_{}",
                source["groupId"].as_str().unwrap_or_default(),
                source["optionId"].as_str().unwrap_or_default()
            )
        } else {
            relation_id
        };
        let severity = string_from_value(source_relation.get("severity"));
        let severity = if severity.trim().is_empty() {
            "warning".to_string()
        } else {
            severity
        };
        relations.push(json!({
            "id": relation_id,
            "type": relation_type,
            "source": source,
            "targets": targets,
            "reason": string_from_value(source_relation.get("reason")),
            "severity": severity,
        }));
    }
    item.insert("optionRelations".to_string(), Value::Array(relations));
}

fn normalize_option_ref(value: Option<&Value>) -> Option<Value> {
    let value = value?;
    let (group_id, option_id) = if let Some(object) = value.as_object() {
        (
            string_from_value(
                object
                    .get("groupId")
                    .or_else(|| object.get("group"))
                    .or_else(|| object.get("group_id")),
            ),
            string_from_value(
                object
                    .get("optionId")
                    .or_else(|| object.get("option"))
                    .or_else(|| object.get("option_id")),
            ),
        )
    } else if let Some(text) = value.as_str().filter(|text| text.contains('.')) {
        let mut parts = text.splitn(2, '.');
        (
            parts.next().unwrap_or_default().to_string(),
            parts.next().unwrap_or_default().to_string(),
        )
    } else {
        return None;
    };
    let group_id = group_id.trim();
    let option_id = option_id.trim();
    if group_id.is_empty() || option_id.is_empty() {
        return None;
    }
    Some(json!({
        "groupId": group_id,
        "optionId": option_id,
    }))
}

fn normalize_legacy_ids(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| string_from_value(Some(item)))
            .filter(|item| !item.trim().is_empty())
            .collect(),
        Some(value) => {
            let text = string_from_value(Some(value));
            if text.trim().is_empty() {
                Vec::new()
            } else {
                vec![text]
            }
        }
        None => Vec::new(),
    }
}

fn camel_case(value: &str) -> String {
    let mut parts = Vec::new();
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            parts.push(current.clone());
            current.clear();
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    let Some((head, tail)) = parts.split_first() else {
        return String::new();
    };
    let mut result = head.to_lowercase();
    for part in tail {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            result.push(first.to_ascii_uppercase());
            result.push_str(chars.as_str());
        }
    }
    result
}

fn ensure_object_entry<'a>(
    object: &'a mut Map<String, Value>,
    key: &str,
) -> &'a mut Map<String, Value> {
    let value = object.entry(key.to_string()).or_insert_with(|| json!({}));
    if !value.is_object() {
        *value = json!({});
    }
    value.as_object_mut().expect("ensured object")
}

fn apply_requirement_metadata(node: &mut Map<String, Value>) {
    let node_id = string_from_value(node.get("id"));
    let Some(metadata) = requirement_metadata_for_node(&node_id) else {
        return;
    };
    for (key, value) in metadata {
        node.entry(key.to_string()).or_insert(value);
    }
}

fn requirement_metadata_for_node(node_id: &str) -> Option<Vec<(&'static str, Value)>> {
    let (contract_targets, consumed_by_steps, contract_fields): (&[&str], &[&str], &[&str]) =
        match node_id {
            "core_loop_decision" => (
                &[
                    "core_playable_contract",
                    "demo_flow_contract",
                    "runtime_data_contract",
                ],
                &["Step02", "Step03", "Step08", "Step13", "Step14"],
                &[],
            ),
            "input_control_decision" => (
                &[
                    "core_playable_contract",
                    "ui_flow_contract",
                    "scene_bootstrap_contract",
                ],
                &["Step03", "Step08", "Step13", "Step14"],
                &[],
            ),
            "objective_system_decision" => (
                &[
                    "demo_flow_contract",
                    "runtime_data_contract",
                    "playable_acceptance_contract",
                ],
                &["Step03", "Step08", "Step13", "Step14"],
                &[],
            ),
            "ux_information_architecture_decision" => (
                &["ui_flow_contract"],
                &["Step03", "Step08", "Step13", "Step14"],
                &[],
            ),
            "ux_flow_decision" => (
                &["ui_flow_contract", "demo_flow_contract"],
                &["Step03", "Step08", "Step13", "Step14"],
                &[],
            ),
            "hud_feedback_decision" => (
                &["ui_flow_contract", "playable_acceptance_contract"],
                &["Step03", "Step08", "Step13", "Step14"],
                &[],
            ),
            "art_direction_decision" => (
                &["asset_mount_contract", "scene_bootstrap_contract"],
                &["Step04", "Step10", "Step12", "Step13"],
                &[],
            ),
            "platform_play_context_decision" => (
                &["scene_bootstrap_contract"],
                &["Step02", "Step03", "Step13"],
                &["scene_bootstrap_contract.camera"],
            ),
            "data_test_design_decision" => (&["playable_acceptance_contract"], &["Step14"], &[]),
            _ => return None,
        };
    Some(vec![
        ("contract_targets", json!(contract_targets)),
        ("consumed_by_steps", json!(consumed_by_steps)),
        ("contract_fields", json!(contract_fields)),
        ("priority", json!("P0")),
        ("requirement_level", json!("required")),
        ("required_for_archetypes", json!(["all"])),
        ("optional_for_archetypes", json!([])),
        ("not_applicable_allowed", json!(true)),
        ("not_applicable_requires_reason", json!(true)),
    ])
}
