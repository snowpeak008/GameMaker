use crate::data_loader::{collect_json_files, load_json_value, string_from_value};
use adm_new_foundation::AdmResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityValidationWarning {
    pub severity: String,
    pub path: String,
    pub message: String,
    #[serde(rename = "schemaId")]
    pub schema_id: String,
}

impl EntityValidationWarning {
    fn new(
        path: impl Into<String>,
        message: impl Into<String>,
        schema_id: impl Into<String>,
    ) -> Self {
        Self {
            severity: "WARNING".to_string(),
            path: path.into(),
            message: message.into(),
            schema_id: schema_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidationError {
    path: String,
    message: String,
    schema_id: String,
}

impl ValidationError {
    fn new(
        path: impl Into<String>,
        message: impl Into<String>,
        schema_id: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
            schema_id: schema_id.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySchemaFile {
    pub relative_path: PathBuf,
    pub schema_id: String,
    pub kind: String,
    pub schema_version: String,
    pub raw: Value,
}

#[derive(Debug, Clone, Default)]
pub struct EntitySchemaRegistry {
    schemas_by_id: BTreeMap<String, Value>,
    schemas_by_key: BTreeMap<(String, String), Value>,
    files: Vec<EntitySchemaFile>,
}

impl EntitySchemaRegistry {
    pub fn load(schema_dir: &Path) -> AdmResult<Self> {
        let mut registry = Self::default();
        if !schema_dir.exists() {
            return Ok(registry);
        }
        for path in collect_json_files(schema_dir, false)? {
            let mut schema = load_json_value(&path)?;
            let schema_id = schema
                .get("id")
                .map(|value| string_from_value(Some(value)))
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|value| value.to_str())
                        .unwrap_or_default()
                        .to_string()
                });
            let kind = schema_kind(&schema);
            let schema_version = schema_version(&schema);
            if let Some(object) = schema.as_object_mut() {
                object.insert(
                    "_schemaFile".to_string(),
                    Value::String(path.display().to_string()),
                );
                object.insert("_schemaId".to_string(), Value::String(schema_id.clone()));
            }
            registry.files.push(EntitySchemaFile {
                relative_path: path
                    .file_name()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from(schema_id.clone())),
                schema_id: schema_id.clone(),
                kind: kind.clone(),
                schema_version: schema_version.clone(),
                raw: schema.clone(),
            });
            registry.schemas_by_id.insert(schema_id, schema.clone());
            if !kind.is_empty() && !schema_version.is_empty() {
                registry
                    .schemas_by_key
                    .insert((kind, schema_version), schema);
            }
        }
        Ok(registry)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn files(&self) -> &[EntitySchemaFile] {
        &self.files
    }

    pub fn normalize_design_entities(
        &self,
        raw_entities: Option<&Value>,
        owner_path: &str,
    ) -> (Vec<Value>, Vec<EntityValidationWarning>) {
        let Some(raw_entities) = raw_entities else {
            return (Vec::new(), Vec::new());
        };
        if raw_entities.is_null() || raw_entities.as_str() == Some("") {
            return (Vec::new(), Vec::new());
        }
        let Some(items) = raw_entities.as_array() else {
            return (
                Vec::new(),
                vec![EntityValidationWarning::new(
                    format!("{owner_path}.designEntities"),
                    "designEntities must be an array",
                    "",
                )],
            );
        };

        let mut entities = Vec::new();
        let mut warnings = Vec::new();
        for (index, entity) in items.iter().enumerate() {
            let entity_path = format!("{owner_path}.designEntities[{index}]");
            if !entity.is_object() {
                warnings.push(EntityValidationWarning::new(
                    entity_path,
                    "entity must be an object",
                    "",
                ));
                continue;
            }
            entities.push(entity.clone());
            for error in self.validate(entity) {
                let suffix = error
                    .path
                    .strip_prefix('$')
                    .map(str::to_string)
                    .unwrap_or_else(|| format!(".{}", error.path));
                warnings.push(EntityValidationWarning::new(
                    format!("{entity_path}{suffix}"),
                    error.message,
                    error.schema_id,
                ));
            }
        }
        (entities, warnings)
    }

    fn validate(&self, entity: &Value) -> Vec<ValidationError> {
        let (schema, lookup_errors) = self.schema_for(entity);
        if !lookup_errors.is_empty() {
            return lookup_errors;
        }
        let Some(schema) = schema else {
            return Vec::new();
        };
        let schema_id = schema
            .get("_schemaId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut errors = Vec::new();
        validate_schema(entity, schema, "$", &mut errors, schema_id);
        errors
    }

    fn schema_for<'a>(&'a self, entity: &Value) -> (Option<&'a Value>, Vec<ValidationError>) {
        if !entity.is_object() {
            return (
                None,
                vec![ValidationError::new("$", "entity must be an object", "")],
            );
        }
        let schema_id = entity
            .get("schema")
            .map(|value| string_from_value(Some(value)))
            .unwrap_or_default();
        if !schema_id.trim().is_empty()
            && let Some(schema) = self.schemas_by_id.get(schema_id.trim())
        {
            return (Some(schema), Vec::new());
        }

        let kind = entity
            .get("kind")
            .map(|value| string_from_value(Some(value)))
            .unwrap_or_default();
        let version = normalize_schema_version(
            &entity
                .get("schemaVersion")
                .map(|value| string_from_value(Some(value)))
                .unwrap_or_default(),
        );
        if !kind.trim().is_empty()
            && !version.trim().is_empty()
            && let Some(schema) = self
                .schemas_by_key
                .get(&(kind.trim().to_string(), version.clone()))
        {
            return (Some(schema), Vec::new());
        }

        if !schema_id.trim().is_empty() {
            return (
                None,
                vec![ValidationError::new(
                    "$",
                    format!("unknown entity schema: {}", schema_id.trim()),
                    "",
                )],
            );
        }
        if kind.trim().is_empty() {
            return (
                None,
                vec![ValidationError::new("$", "missing entity kind", "")],
            );
        }
        if version.trim().is_empty() {
            return (
                None,
                vec![ValidationError::new(
                    "$",
                    "missing entity schemaVersion or schema",
                    "",
                )],
            );
        }
        (
            None,
            vec![ValidationError::new(
                "$",
                format!(
                    "unknown entity schema for kind={}, schemaVersion={version}",
                    kind.trim()
                ),
                "",
            )],
        )
    }
}

fn schema_kind(schema: &Value) -> String {
    let direct = schema
        .get("kind")
        .map(|value| string_from_value(Some(value)))
        .unwrap_or_default();
    if !direct.trim().is_empty() {
        return direct;
    }
    schema
        .pointer("/properties/kind/const")
        .map(|value| string_from_value(Some(value)))
        .unwrap_or_default()
}

fn schema_version(schema: &Value) -> String {
    normalize_schema_version(
        &schema
            .get("schemaVersion")
            .or_else(|| schema.pointer("/properties/schemaVersion/const"))
            .map(|value| string_from_value(Some(value)))
            .unwrap_or_default(),
    )
}

fn normalize_schema_version(value: &str) -> String {
    let text = value.trim();
    if text.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("v{text}")
    } else {
        text.to_string()
    }
}

fn validate_schema(
    instance: &Value,
    schema: &Value,
    path: &str,
    errors: &mut Vec<ValidationError>,
    schema_id: &str,
) {
    if let Some(schema_type) = schema.get("type")
        && !type_matches(instance, schema_type)
    {
        errors.push(ValidationError::new(
            path,
            format!(
                "expected {}, got {}",
                render_schema_type(schema_type),
                type_name(instance)
            ),
            schema_id,
        ));
        return;
    }

    if let Some(const_value) = schema.get("const")
        && instance != const_value
    {
        errors.push(ValidationError::new(
            path,
            format!("expected constant {}", const_value),
            schema_id,
        ));
    }

    if let Some(enum_values) = schema.get("enum").and_then(Value::as_array)
        && !enum_values.iter().any(|value| value == instance)
    {
        errors.push(ValidationError::new(
            path,
            format!("expected one of {}", Value::Array(enum_values.clone())),
            schema_id,
        ));
    }

    if let Some(object) = instance.as_object() {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    errors.push(ValidationError::new(
                        path,
                        format!("missing required field: {key}"),
                        schema_id,
                    ));
                }
            }
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (key, property_schema) in properties {
                if let Some(child) = object.get(key) {
                    validate_schema(
                        child,
                        property_schema,
                        &format!("{path}.{key}"),
                        errors,
                        schema_id,
                    );
                }
            }
        }
    } else if schema
        .get("required")
        .and_then(Value::as_array)
        .is_some_and(|required| !required.is_empty())
    {
        errors.push(ValidationError::new(
            path,
            "required fields can only be checked on objects",
            schema_id,
        ));
    }

    if let Some(items) = instance.as_array() {
        if let Some(min_items) = schema.get("minItems").and_then(Value::as_u64)
            && items.len() < min_items as usize
        {
            errors.push(ValidationError::new(
                path,
                format!("expected at least {min_items} item(s)"),
                schema_id,
            ));
        }
        if let Some(item_schema) = schema.get("items") {
            for (index, item) in items.iter().enumerate() {
                validate_schema(
                    item,
                    item_schema,
                    &format!("{path}[{index}]"),
                    errors,
                    schema_id,
                );
            }
        }
    }

    if let Some(text) = instance.as_str()
        && let Some(min_length) = schema.get("minLength").and_then(Value::as_u64)
        && text.chars().count() < min_length as usize
    {
        errors.push(ValidationError::new(
            path,
            format!("expected length >= {min_length}"),
            schema_id,
        ));
    }

    if let Some(branches) = schema.get("anyOf").and_then(Value::as_array)
        && !branches
            .iter()
            .any(|branch| collect_branch_errors(instance, branch, schema_id).is_empty())
    {
        errors.push(ValidationError::new(
            path,
            "must satisfy at least one anyOf branch",
            schema_id,
        ));
    }

    if let Some(branches) = schema.get("oneOf").and_then(Value::as_array) {
        let matches = branches
            .iter()
            .filter(|branch| collect_branch_errors(instance, branch, schema_id).is_empty())
            .count();
        if matches != 1 {
            errors.push(ValidationError::new(
                path,
                format!("must satisfy exactly one oneOf branch, matched {matches}"),
                schema_id,
            ));
        }
    }
}

fn collect_branch_errors(
    instance: &Value,
    schema: &Value,
    schema_id: &str,
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    validate_schema(instance, schema, "$", &mut errors, schema_id);
    errors
}

fn type_matches(value: &Value, schema_type: &Value) -> bool {
    if let Some(types) = schema_type.as_array() {
        return types.iter().any(|item| type_matches(value, item));
    }
    match schema_type.as_str().unwrap_or_default() {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => true,
    }
}

fn render_schema_type(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        text.to_string()
    } else {
        value.to_string()
    }
}

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.as_i64().is_some() || number.as_u64().is_some() => {
            "integer"
        }
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}
