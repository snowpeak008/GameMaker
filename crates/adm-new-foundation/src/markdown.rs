use crate::{AdmError, AdmResult, structured_md::loads_data};
use serde_json::{Map, Value, json};

pub fn normalize_section_key(key: &str) -> String {
    key.trim()
        .to_lowercase()
        .replace([' ', '-'], "_")
        .trim_matches('_')
        .to_string()
}

pub fn normalize_section_keys(value: &Value) -> Value {
    let Value::Object(map) = value else {
        return value.clone();
    };
    let mut result = Map::new();
    for (key, child) in map {
        if key.starts_with('_') {
            continue;
        }
        if let Value::Object(child_map) = child {
            for (child_key, child_value) in child_map {
                result.insert(normalize_section_key(child_key), child_value.clone());
            }
        } else {
            result.insert(normalize_section_key(key), child.clone());
        }
    }
    Value::Object(result)
}

pub fn parse_markdown_output(text: &str) -> AdmResult<Value> {
    let has_markdown_heading = text.lines().any(|line| line.trim_start().starts_with('#'));
    if !has_markdown_heading && let Ok(value) = loads_data(text) {
        return Ok(value);
    }

    let mut root = Map::new();
    let mut section_name: Option<String> = None;
    let mut section_map = Map::new();
    let mut section_list: Vec<Value> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("# ") {
            root.insert(
                "_title".to_string(),
                Value::String(trimmed.trim_start_matches("# ").trim().to_string()),
            );
            continue;
        }
        if trimmed.starts_with("## ") && !trimmed.starts_with("### ") {
            flush_section(
                &mut root,
                &mut section_name,
                &mut section_map,
                &mut section_list,
            );
            section_name = Some(trimmed.trim_start_matches("## ").trim().to_string());
            continue;
        }
        if trimmed.starts_with("|") && trimmed.ends_with("|") {
            parse_table_line(trimmed, &mut section_map);
            continue;
        }
        if let Some((key, value)) = parse_bold_kv(trimmed) {
            section_map.insert(key, Value::String(value));
            continue;
        }
        if let Some(item) = trimmed.strip_prefix("- ") {
            section_list.push(Value::String(item.trim().to_string()));
        }
    }

    flush_section(
        &mut root,
        &mut section_name,
        &mut section_map,
        &mut section_list,
    );
    if root.is_empty() {
        Err(AdmError::new(
            "markdown output did not contain parseable sections",
        ))
    } else {
        Ok(Value::Object(root))
    }
}

pub fn parse_md_output(
    text: &str,
    required_keys: &[&str],
    require_mapping: bool,
    flatten_sections: bool,
) -> AdmResult<Value> {
    let mut parsed = parse_markdown_output(text).unwrap_or_else(|_| json!({"raw": text}));
    if flatten_sections {
        parsed = normalize_section_keys(&parsed);
    }
    if require_mapping && !parsed.is_object() {
        return Err(AdmError::new("markdown output must parse as object"));
    }
    for key in required_keys {
        if parsed.get(key).is_none() {
            return Err(AdmError::new(format!(
                "markdown output missing required field: {key}"
            )));
        }
    }
    Ok(parsed)
}

fn flush_section(
    root: &mut Map<String, Value>,
    section_name: &mut Option<String>,
    section_map: &mut Map<String, Value>,
    section_list: &mut Vec<Value>,
) {
    let Some(name) = section_name.take() else {
        return;
    };
    let value = if !section_map.is_empty() && !section_list.is_empty() {
        section_map.insert(
            "_items".to_string(),
            Value::Array(std::mem::take(section_list)),
        );
        Value::Object(std::mem::take(section_map))
    } else if !section_map.is_empty() {
        Value::Object(std::mem::take(section_map))
    } else if !section_list.is_empty() {
        Value::Array(std::mem::take(section_list))
    } else {
        Value::Object(Map::new())
    };
    root.insert(name, value);
}

fn parse_bold_kv(line: &str) -> Option<(String, String)> {
    let line = line.strip_prefix("- ")?;
    let rest = line.strip_prefix("**")?;
    let (key, value) = rest.split_once("**:")?;
    Some((key.trim().to_string(), value.trim().to_string()))
}

fn parse_table_line(line: &str, section_map: &mut Map<String, Value>) {
    let cells: Vec<Value> = line
        .trim_matches('|')
        .split('|')
        .map(|cell| Value::String(cell.trim().to_string()))
        .collect();
    if cells.iter().all(|cell| {
        cell.as_str()
            .is_some_and(|text| text.chars().all(|ch| ch == '-' || ch == ':' || ch == ' '))
    }) {
        return;
    }
    let rows = section_map
        .entry("_table_rows".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    if let Value::Array(rows) = rows {
        rows.push(Value::Array(cells));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parser_extracts_sections_key_values_and_lists() {
        let parsed =
            parse_markdown_output("# Title\n\n## Summary\n- **Name**: Demo\n- alpha\n\n## Empty\n")
                .unwrap();

        assert_eq!(parsed["_title"], json!("Title"));
        assert_eq!(parsed["Summary"]["Name"], json!("Demo"));
        assert_eq!(parsed["Summary"]["_items"], json!(["alpha"]));
        assert!(parsed["Empty"].is_object());
    }

    #[test]
    fn parse_md_output_can_flatten_sections_and_require_keys() {
        let parsed = parse_md_output(
            "## Project Info\n- **Project Name**: Demo",
            &["project_name"],
            true,
            true,
        )
        .unwrap();

        assert_eq!(parsed["project_name"], json!("Demo"));
    }
}
