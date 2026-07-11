use crate::{AdmResult, structured_md};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn extract_from_ui_graph_value(graph: &Value) -> BTreeMap<String, String> {
    let mut texts = BTreeMap::new();
    let Some(panels) = graph
        .get("registry")
        .and_then(|registry| registry.get("panels"))
        .and_then(Value::as_array)
    else {
        return texts;
    };
    for panel in panels {
        let Some(panel_id) = panel.get("id").and_then(Value::as_str) else {
            continue;
        };
        let title = panel_id.rsplit('.').next().unwrap_or(panel_id);
        texts.insert(format!("{panel_id}.title"), title.to_string());
    }
    texts
}

pub fn extract_from_config_schema_value(schema: &Value) -> BTreeMap<String, String> {
    let mut texts = BTreeMap::new();
    let Some(tables) = schema.get("tables").and_then(Value::as_array) else {
        return texts;
    };
    for table in tables {
        let Some(table_name) = table.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(columns) = table.get("columns").and_then(Value::as_array) else {
            continue;
        };
        for column in columns {
            let is_string = column.get("type").and_then(Value::as_str) == Some("string");
            let Some(column_name) = column.get("name").and_then(Value::as_str) else {
                continue;
            };
            if is_string {
                texts.insert(format!("{table_name}.{column_name}"), String::new());
            }
        }
    }
    texts
}

pub fn extract_from_ui_graph(path: &Path) -> AdmResult<BTreeMap<String, String>> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let graph = structured_md::read_structured_or_text(path)?;
    Ok(extract_from_ui_graph_value(&graph))
}

pub fn extract_from_config_schema(path: &Path) -> AdmResult<BTreeMap<String, String>> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let schema = structured_md::read_structured_or_text(path)?;
    Ok(extract_from_config_schema_value(&schema))
}

pub fn generate_strings_file(
    texts: &BTreeMap<String, String>,
    output_path: &Path,
    language: &str,
) -> AdmResult<PathBuf> {
    let value = serde_json::to_value(texts)
        .map_err(|error| crate::AdmError::new(format!("failed to serialize strings: {error}")))?;
    structured_md::write_data(
        output_path,
        &value,
        &format!("Translation Strings ({language})"),
    )
}

pub fn run_text_extraction(plans_dir: &Path, output_dir: &Path) -> AdmResult<Option<PathBuf>> {
    let mut ui_graph = plans_dir.join("ui_graph.json");
    if !ui_graph.exists() {
        ui_graph = plans_dir.join("ui_graph.md");
    }
    let mut config_schema = plans_dir.join("config_schema.json");
    if !config_schema.exists() {
        config_schema = plans_dir.join("config_schema.md");
    }

    let mut texts = extract_from_ui_graph(&ui_graph)?;
    texts.extend(extract_from_config_schema(&config_schema)?);
    if texts.is_empty() {
        return Ok(None);
    }
    std::fs::create_dir_all(output_dir)?;
    Ok(Some(generate_strings_file(
        &texts,
        &output_dir.join("zh-CN.md"),
        "zh-CN",
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_stable_id;
    use serde_json::json;

    #[test]
    fn extracts_titles_and_schema_strings() {
        let ui_texts = extract_from_ui_graph_value(&json!({
            "registry": {"panels": [{"id": "main.inventory"}, {"id": "settings"}]}
        }));
        let schema_texts = extract_from_config_schema_value(&json!({
            "tables": [{"name": "item", "columns": [
                {"name": "title", "type": "string"},
                {"name": "count", "type": "int"}
            ]}]
        }));

        assert_eq!(ui_texts.get("main.inventory.title").unwrap(), "inventory");
        assert_eq!(schema_texts.get("item.title").unwrap(), "");
        assert!(!schema_texts.contains_key("item.count"));
    }

    #[test]
    fn run_text_extraction_writes_language_file_when_text_exists() {
        let root = std::env::temp_dir().join(new_stable_id("text_extraction").unwrap());
        let plans = root.join("plans");
        let output = root.join("out");
        std::fs::create_dir_all(&plans).unwrap();
        std::fs::write(
            plans.join("ui_graph.json"),
            r#"{"registry":{"panels":[{"id":"main.menu"}]}}"#,
        )
        .unwrap();

        let lang_file = run_text_extraction(&plans, &output).unwrap().unwrap();

        assert!(lang_file.ends_with("zh-CN.md"));
        assert!(
            std::fs::read_to_string(lang_file)
                .unwrap()
                .contains("main.menu.title")
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
