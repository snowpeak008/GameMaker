use crate::{AdmError, AdmResult};
use serde_json::{Map, Number, Value};

pub fn safe_load(text: &str) -> AdmResult<Value> {
    let content = text.trim();
    if content.is_empty() {
        return Ok(Value::Null);
    }
    if let Ok(value) = serde_json::from_str(content) {
        return Ok(value);
    }
    parse_simple_yaml_mapping(content)
}

pub fn safe_dump(value: &Value) -> AdmResult<String> {
    serde_json::to_string_pretty(value)
        .map_err(|error| AdmError::new(format!("failed to dump yaml-compatible json: {error}")))
}

pub fn dump(value: &Value) -> AdmResult<String> {
    safe_dump(value)
}

fn parse_simple_yaml_mapping(content: &str) -> AdmResult<Value> {
    let mut map = Map::new();
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(AdmError::new(
                "PyYAML-compatible parsing is not available for this YAML shape",
            ));
        };
        let key = key.trim().trim_matches('"').trim_matches('\'');
        if key.is_empty() {
            return Err(AdmError::new("yaml mapping key must not be empty"));
        }
        map.insert(key.to_string(), parse_scalar(value.trim()));
    }
    Ok(Value::Object(map))
}

fn parse_scalar(value: &str) -> Value {
    let value = value.trim();
    if value.is_empty() || value == "null" || value == "~" {
        Value::Null
    } else if value == "true" {
        Value::Bool(true)
    } else if value == "false" {
        Value::Bool(false)
    } else if let Ok(integer) = value.parse::<i64>() {
        Value::Number(Number::from(integer))
    } else if let Ok(float) = value.parse::<f64>() {
        Number::from_f64(float)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    } else {
        Value::String(value.trim_matches('"').trim_matches('\'').to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn safe_load_accepts_json_yaml_subset_and_flat_yaml() {
        assert_eq!(safe_load(r#"{"a":1}"#).unwrap(), json!({"a": 1}));
        assert_eq!(
            safe_load("name: demo\nenabled: true\ncount: 2").unwrap(),
            json!({"name": "demo", "enabled": true, "count": 2})
        );
    }
}
