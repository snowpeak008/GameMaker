use adm_new_foundation::{sha256_hex, unix_timestamp};
use serde_json::{Map, Value, json};

pub(crate) fn now_iso() -> String {
    format!("unix:{}", unix_timestamp())
}

pub(crate) fn get_str(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(crate) fn first_str(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            let text = get_str(value, key);
            (!text.trim().is_empty()).then_some(text)
        })
        .unwrap_or_default()
}

pub(crate) fn list(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn stable_hash(value: &Value) -> String {
    let data = serde_json::to_vec(value).unwrap_or_default();
    sha256_hex(&data)
}

pub(crate) fn slug(value: &str) -> String {
    let mut output = String::new();
    let mut previous_sep = false;
    for ch in value.chars() {
        let keep = ch.is_ascii_alphanumeric()
            || ch == '_'
            || ch == '-'
            || ('\u{4e00}'..='\u{9fff}').contains(&ch);
        if keep {
            output.push(ch);
            previous_sep = false;
        } else if !previous_sep {
            output.push('_');
            previous_sep = true;
        }
        if output.chars().count() >= 80 {
            break;
        }
    }
    let cleaned = output.trim_matches('_').to_string();
    if cleaned.is_empty() {
        "unnamed_project".to_string()
    } else {
        cleaned
    }
}

pub(crate) fn selection_id(selection: &Value, fallback: usize) -> String {
    first_str(selection, &["id", "selection_id"])
        .trim()
        .to_string()
        .if_empty(format!("SEL-{fallback:03}"))
}

pub(crate) fn selection_label(selection: &Value) -> String {
    let label = first_str(selection, &["label", "title", "name"]);
    if !label.is_empty() {
        return label;
    }
    let item_type = first_str(selection, &["item_type", "itemType"]);
    let option = get_str(selection, "option");
    if !item_type.is_empty() {
        format!("{item_type}: {option}")
    } else {
        option
    }
}

pub(crate) fn selection_source_refs(selection: &Value) -> Vec<Value> {
    let source = first_str(selection, &["source_ref", "sourceRef"]);
    if source.is_empty() {
        Vec::new()
    } else {
        vec![Value::String(source)]
    }
}

pub(crate) fn selection_text(selection: &Value) -> String {
    [
        selection_label(selection),
        get_str(selection, "option"),
        get_str(selection, "purpose"),
        first_str(selection, &["item_type", "itemType"]),
        first_str(selection, &["layer_title", "layerTitle"]),
    ]
    .into_iter()
    .filter(|part| !part.trim().is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

pub(crate) fn selection_items(parsed: &Value) -> Vec<Value> {
    list(parsed, "selections")
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            json!({
                "selection_id": selection_id(&item, index + 1),
                "item_type": first_str(&item, &["item_type", "itemType"]),
                "label": selection_label(&item),
                "purpose": get_str(&item, "purpose"),
                "source_ref": first_str(&item, &["source_ref", "sourceRef"]),
            })
        })
        .collect()
}

pub(crate) fn selection_fingerprint(parsed: &Value) -> Vec<Value> {
    list(parsed, "selections")
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            json!({
                "id": selection_id(&item, index + 1),
                "item_type": first_str(&item, &["item_type", "itemType"]),
                "label": selection_label(&item),
                "purpose": get_str(&item, "purpose"),
                "source_ref": first_str(&item, &["source_ref", "sourceRef"]),
            })
        })
        .collect()
}

pub(crate) fn matches_keywords(selection: &Value, keywords: &[&str]) -> bool {
    let text = selection_text(selection).to_ascii_lowercase();
    keywords
        .iter()
        .any(|keyword| text.contains(&keyword.to_ascii_lowercase()))
}

pub(crate) fn is_empty_value(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(text)) => text.trim().is_empty(),
        Some(Value::Array(items)) => items.is_empty(),
        Some(Value::Object(object)) => object.is_empty(),
        Some(_) => false,
    }
}

pub(crate) fn object_from_value(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

pub(crate) trait IfEmpty {
    fn if_empty(self, fallback: String) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: String) -> String {
        if self.trim().is_empty() {
            fallback
        } else {
            self
        }
    }
}
