use crate::{AdmError, AdmResult, markdown::parse_markdown_output, yaml_compat};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub fn dumps_data(data: &Value) -> AdmResult<String> {
    serde_json::to_string_pretty(data)
        .map_err(|error| AdmError::new(format!("failed to serialize structured data: {error}")))
}

pub fn loads_data(text: &str) -> AdmResult<Value> {
    let content = text.trim();
    if let Some(json_text) = extract_fenced_block(content, &["json", ""]) {
        return serde_json::from_str(json_text.trim())
            .map_err(|error| AdmError::new(format!("failed to parse fenced json: {error}")));
    }
    if let Ok(value) = serde_json::from_str(content) {
        return Ok(value);
    }
    if let Some(yaml_text) = extract_fenced_block(content, &["yaml", "yml"]) {
        return yaml_compat::safe_load(yaml_text.trim());
    }
    if looks_like_yaml_mapping(content) {
        return yaml_compat::safe_load(content);
    }
    Err(AdmError::new("no JSON or YAML structured payload found"))
}

pub fn read_data(path: &Path) -> AdmResult<Value> {
    let text = std::fs::read_to_string(path)?;
    loads_data(&text)
}

pub fn write_data(path: &Path, data: &Value, title: &str) -> AdmResult<PathBuf> {
    let text = format!("# {title}\n\n```json\n{}\n```\n", dumps_data(data)?);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)?;
    Ok(path.to_path_buf())
}

pub fn data_to_markdown(data: &Value, level: usize) -> String {
    let mut lines = Vec::new();
    emit_markdown(data, level.max(2), None, &mut lines);
    lines.join("\n").trim().to_string()
}

pub fn data_to_text(data: &Value) -> AdmResult<String> {
    match data {
        Value::String(text) => Ok(text.clone()),
        _ => dumps_data(data),
    }
}

pub fn read_structured_or_text(path: &Path) -> AdmResult<Value> {
    let text = std::fs::read_to_string(path)?;
    if let Ok(value) = loads_data(&text) {
        return Ok(value);
    }
    if matches!(
        path.extension().and_then(|value| value.to_str()),
        Some("yaml" | "yml")
    ) {
        return yaml_compat::safe_load(&text);
    }
    Ok(parse_markdown_output(&text).unwrap_or_else(|_| json!({"raw": text})))
}

fn emit_markdown(value: &Value, depth: usize, label: Option<&str>, lines: &mut Vec<String>) {
    let heading = "#".repeat(depth.clamp(2, 6));
    if let Some(label) = label {
        lines.push(format!("{heading} {label}"));
    }
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                lines.push("- none".to_string());
                return;
            }
            for (key, child) in map {
                match child {
                    Value::Object(_) | Value::Array(_) => {
                        emit_markdown(child, depth + 1, Some(key), lines);
                    }
                    _ => lines.push(format!("- **{key}**: {}", scalar_to_text(child))),
                }
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                lines.push("- none".to_string());
                return;
            }
            for (index, item) in items.iter().enumerate() {
                match item {
                    Value::Object(_) | Value::Array(_) => {
                        emit_markdown(item, depth + 1, Some(&format!("Item {}", index + 1)), lines);
                    }
                    _ => lines.push(format!("- {}", scalar_to_text(item))),
                }
            }
        }
        _ => lines.push(scalar_to_text(value)),
    }
}

fn scalar_to_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

fn extract_fenced_block<'a>(content: &'a str, languages: &[&str]) -> Option<&'a str> {
    let mut remaining = content;
    while let Some(start) = remaining.find("```") {
        let after_start = &remaining[start + 3..];
        let line_end = after_start.find('\n')?;
        let lang = after_start[..line_end]
            .split_whitespace()
            .next()
            .unwrap_or("");
        let body_start = start + 3 + line_end + 1;
        let after_body_start = &remaining[body_start..];
        let end = after_body_start.find("```")?;
        if languages.iter().any(|candidate| candidate == &lang) {
            return Some(&remaining[body_start..body_start + end]);
        }
        remaining = &after_body_start[end + 3..];
    }
    None
}

fn looks_like_yaml_mapping(content: &str) -> bool {
    content.lines().any(|line| {
        line.split_once(':')
            .is_some_and(|(key, _)| !key.trim().is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_stable_id;
    use serde_json::json;

    #[test]
    fn loads_data_accepts_json_fence_yaml_fence_and_raw_json() {
        assert_eq!(
            loads_data("```json\n{\"a\":1}\n```").unwrap(),
            json!({"a": 1})
        );
        assert_eq!(loads_data("{\"b\":2}").unwrap(), json!({"b": 2}));
        assert_eq!(
            loads_data("```yaml\nname: demo\n```").unwrap(),
            json!({"name": "demo"})
        );
    }

    #[test]
    fn write_data_round_trips_markdown_json_payload() {
        let root = std::env::temp_dir().join(new_stable_id("structured").unwrap());
        let path = root.join("data.md");

        write_data(&path, &json!({"name": "demo"}), "Data").unwrap();

        assert_eq!(read_data(&path).unwrap(), json!({"name": "demo"}));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn data_to_markdown_renders_nested_objects_and_lists() {
        let markdown = data_to_markdown(&json!({"name": "demo", "items": ["a", "b"]}), 2);

        assert!(markdown.contains("- **name**: demo"));
        assert!(markdown.contains("### items"));
        assert!(markdown.contains("- a"));
    }
}
