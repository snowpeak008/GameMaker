use adm_new_foundation::{AdmError, AdmResult, structured_md};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

pub const CONTRACT_FAMILY: &str = "schema";
pub const SCHEMA_ROOT: &str = "knowledge/schemas";
pub const EXPECTED_SCHEMA_FILE_COUNT: usize = 93;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaFile {
    pub relative_path: String,
    pub schema_id: Option<String>,
    pub title: Option<String>,
    pub top_level_type: Option<String>,
    pub required_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaRegistry {
    pub schema_root: String,
    pub files: Vec<SchemaFile>,
}

impl SchemaRegistry {
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn get(&self, relative_path: &str) -> Option<&SchemaFile> {
        let normalized = normalize_path(relative_path);
        self.files
            .iter()
            .find(|file| file.relative_path == normalized)
    }

    pub fn validate_inventory(&self, expected_count: usize) -> Vec<String> {
        let mut errors = Vec::new();
        if self.files.len() != expected_count {
            errors.push(format!(
                "schema file count mismatch: expected {expected_count}, got {}",
                self.files.len()
            ));
        }
        let mut seen = BTreeSet::new();
        for file in &self.files {
            if file.relative_path.trim().is_empty() {
                errors.push("schema relative path must not be empty".to_string());
            }
            if !seen.insert(file.relative_path.clone()) {
                errors.push(format!("duplicate schema path: {}", file.relative_path));
            }
            if file.top_level_type.is_none() {
                errors.push(format!(
                    "schema missing top-level type: {}",
                    file.relative_path
                ));
            }
        }
        errors
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractValidationReport {
    pub contract: String,
    pub schema: String,
    pub valid: bool,
    pub errors: Vec<String>,
}

pub fn discover_schema_registry(project_root: &Path) -> AdmResult<SchemaRegistry> {
    let schema_root = project_root.join(SCHEMA_ROOT);
    let mut paths = Vec::new();
    collect_schema_paths(&schema_root, &mut paths)?;
    paths.sort();

    let mut files = Vec::new();
    for path in paths {
        let schema = load_structured_file(&path)?;
        let relative_path = path
            .strip_prefix(project_root)
            .map_err(|error| AdmError::new(format!("failed to relativize schema path: {error}")))?;
        files.push(schema_file_from_value(
            &normalize_path(relative_path),
            &schema,
        ));
    }

    Ok(SchemaRegistry {
        schema_root: SCHEMA_ROOT.to_string(),
        files,
    })
}

pub fn schema_file_from_value(relative_path: &str, schema: &Value) -> SchemaFile {
    SchemaFile {
        relative_path: normalize_path(relative_path),
        schema_id: schema
            .get("$id")
            .and_then(Value::as_str)
            .map(str::to_string),
        title: schema
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_string),
        top_level_type: schema
            .get("type")
            .and_then(type_name_from_schema_value)
            .map(str::to_string),
        required_fields: schema
            .get("required")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub fn load_structured_file(path: &Path) -> AdmResult<Value> {
    let text = std::fs::read_to_string(path)?;
    let text = text.trim_start_matches('\u{feff}');
    match path.extension().and_then(|value| value.to_str()) {
        Some("json") => serde_json::from_str(&text)
            .map_err(|error| AdmError::new(format!("failed to parse json: {error}"))),
        Some("yaml" | "yml" | "md") => structured_md::loads_data(&text),
        _ => structured_md::loads_data(&text).or_else(|_| {
            serde_json::from_str(&text)
                .map_err(|error| AdmError::new(format!("failed to parse structured file: {error}")))
        }),
    }
}

pub fn validate_contract(data: &Value, schema: &Value) -> Vec<String> {
    let mut active_refs = HashSet::new();
    validate_contract_at_path(data, schema, schema, "$", &mut active_refs)
}

pub fn validate_contract_file(contract_path: &Path, schema_path: &Path) -> AdmResult<Vec<String>> {
    let data = load_structured_file(contract_path)?;
    let schema = load_structured_file(schema_path)?;
    Ok(validate_contract(&data, &schema))
}

pub fn write_validation_report(
    report_path: &Path,
    contract_path: &Path,
    schema_path: &Path,
    errors: Vec<String>,
) -> AdmResult<ContractValidationReport> {
    let report = ContractValidationReport {
        contract: normalize_path(contract_path),
        schema: normalize_path(schema_path),
        valid: errors.is_empty(),
        errors,
    };
    let text = serde_json::to_string_pretty(&report)
        .map_err(|error| AdmError::new(format!("failed to serialize report: {error}")))?;
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(report_path, text)?;
    Ok(report)
}

fn validate_contract_at_path(
    data: &Value,
    schema: &Value,
    root_schema: &Value,
    path: &str,
    active_refs: &mut HashSet<(String, String)>,
) -> Vec<String> {
    let mut errors = Vec::new();

    if let Some(allowed) = schema.as_bool() {
        if !allowed {
            errors.push(format!("{path}: rejected by false schema"));
        }
        return errors;
    }

    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let active_key = (path.to_string(), reference.to_string());
        if active_refs.insert(active_key.clone()) {
            match resolve_local_reference(root_schema, reference) {
                Ok(resolved) => errors.extend(validate_contract_at_path(
                    data,
                    resolved,
                    root_schema,
                    path,
                    active_refs,
                )),
                Err(message) => errors.push(format!("{path}: {message}")),
            }
            active_refs.remove(&active_key);
        }
    }

    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        for branch in all_of
            .iter()
            .filter(|branch| branch.is_object() || branch.is_boolean())
        {
            errors.extend(validate_contract_at_path(
                data,
                branch,
                root_schema,
                path,
                active_refs,
            ));
        }
    }

    if let Some(any_of) = schema.get("anyOf").and_then(Value::as_array) {
        let branch_errors: Vec<Vec<String>> = any_of
            .iter()
            .filter(|branch| branch.is_object() || branch.is_boolean())
            .map(|branch| validate_contract_at_path(data, branch, root_schema, path, active_refs))
            .collect();
        if !branch_errors.iter().any(Vec::is_empty) {
            errors.push(format!("{path}: did not match any allowed schema"));
            for branch in branch_errors {
                errors.extend(branch);
            }
        }
    }

    if let Some(one_of) = schema.get("oneOf").and_then(Value::as_array) {
        let branch_errors: Vec<Vec<String>> = one_of
            .iter()
            .filter(|branch| branch.is_object() || branch.is_boolean())
            .map(|branch| validate_contract_at_path(data, branch, root_schema, path, active_refs))
            .collect();
        let matching_branches = branch_errors
            .iter()
            .filter(|branch| branch.is_empty())
            .count();
        match matching_branches {
            1 => {}
            0 => {
                errors.push(format!("{path}: did not match exactly one allowed schema"));
                for branch in branch_errors {
                    errors.extend(branch);
                }
            }
            count => errors.push(format!(
                "{path}: matched {count} schemas, expected exactly one"
            )),
        }
    }

    if let Some(expected_type) = schema.get("type") {
        let expected_types = expected_type_list(expected_type);
        if !expected_types.is_empty()
            && !expected_types
                .iter()
                .any(|expected| matches_json_type(data, expected))
        {
            errors.push(format!(
                "{path}: expected {}, got {}",
                render_expected_type(expected_type),
                json_type_name(data)
            ));
            return errors;
        }
    }

    if let Some(allowed_values) = schema.get("enum").and_then(Value::as_array)
        && !allowed_values.iter().any(|allowed| allowed == data)
    {
        errors.push(format!(
            "{path}: expected one of {}, got {}",
            Value::Array(allowed_values.clone()),
            data
        ));
    }

    if let Some(expected) = schema.get("const")
        && expected != data
    {
        errors.push(format!("{path}: expected constant {expected}, got {data}"));
    }

    if data.is_number() {
        if let Some(minimum) = schema.get("minimum") {
            match compare_json_numbers(data, minimum) {
                Some(Ordering::Less) => errors.push(format!(
                    "{path}: expected number greater than or equal to {minimum}, got {data}"
                )),
                None if !minimum.is_number() => {
                    errors.push(format!("{path}: schema minimum must be a number"))
                }
                _ => {}
            }
        }
        if let Some(maximum) = schema.get("maximum") {
            match compare_json_numbers(data, maximum) {
                Some(Ordering::Greater) => errors.push(format!(
                    "{path}: expected number less than or equal to {maximum}, got {data}"
                )),
                None if !maximum.is_number() => {
                    errors.push(format!("{path}: schema maximum must be a number"))
                }
                _ => {}
            }
        }
    }

    if let Some(text) = data.as_str() {
        validate_length_constraint(
            &mut errors,
            path,
            "minLength",
            schema.get("minLength"),
            text.chars().count(),
            LengthComparison::Minimum,
        );
        validate_length_constraint(
            &mut errors,
            path,
            "maxLength",
            schema.get("maxLength"),
            text.chars().count(),
            LengthComparison::Maximum,
        );
        if let Some(pattern) = schema.get("pattern").and_then(Value::as_str) {
            match Regex::new(pattern) {
                Ok(regex) if !regex.is_match(text) => {
                    errors.push(format!("{path}: string did not match pattern {pattern:?}"))
                }
                Err(error) => errors.push(format!(
                    "{path}: invalid schema pattern {pattern:?}: {error}"
                )),
                _ => {}
            }
        }
    }

    if let Some(object) = data.as_object() {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for key in required.iter().filter_map(Value::as_str) {
                if !object.contains_key(key) {
                    errors.push(format!("{path}.{key}: required field missing"));
                }
            }
        }
        if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
            for (key, child) in object {
                if let Some(child_schema) = properties.get(key) {
                    errors.extend(validate_contract_at_path(
                        child,
                        child_schema,
                        root_schema,
                        &format!("{path}.{key}"),
                        active_refs,
                    ));
                }
            }
        }
    }

    if let Some(items) = data.as_array() {
        validate_length_constraint(
            &mut errors,
            path,
            "minItems",
            schema.get("minItems"),
            items.len(),
            LengthComparison::Minimum,
        );
        validate_length_constraint(
            &mut errors,
            path,
            "maxItems",
            schema.get("maxItems"),
            items.len(),
            LengthComparison::Maximum,
        );
        if let Some(item_schema) = schema.get("items") {
            for (index, item) in items.iter().enumerate() {
                errors.extend(validate_contract_at_path(
                    item,
                    item_schema,
                    root_schema,
                    &format!("{path}[{index}]"),
                    active_refs,
                ));
            }
        }
    }

    errors
}

#[derive(Debug, Clone, Copy)]
enum LengthComparison {
    Minimum,
    Maximum,
}

fn validate_length_constraint(
    errors: &mut Vec<String>,
    path: &str,
    keyword: &str,
    constraint: Option<&Value>,
    actual: usize,
    comparison: LengthComparison,
) {
    let Some(constraint) = constraint else {
        return;
    };
    let Some(limit) = constraint.as_u64() else {
        errors.push(format!(
            "{path}: schema {keyword} must be a non-negative integer"
        ));
        return;
    };
    let violated = match comparison {
        LengthComparison::Minimum => (actual as u128) < u128::from(limit),
        LengthComparison::Maximum => (actual as u128) > u128::from(limit),
    };
    if violated {
        let comparison = match comparison {
            LengthComparison::Minimum => "at least",
            LengthComparison::Maximum => "at most",
        };
        errors.push(format!(
            "{path}: {keyword} expected {comparison} {limit} items/characters, got {actual}"
        ));
    }
}

fn compare_json_numbers(left: &Value, right: &Value) -> Option<Ordering> {
    let left = left.as_number()?;
    let right = right.as_number()?;

    if let (Some(left), Some(right)) = (left.as_i64(), right.as_i64()) {
        return Some(left.cmp(&right));
    }
    if let (Some(left), Some(right)) = (left.as_u64(), right.as_u64()) {
        return Some(left.cmp(&right));
    }
    if let (Some(left), Some(right)) = (left.as_i64(), right.as_u64()) {
        return Some(if left.is_negative() {
            Ordering::Less
        } else {
            (left as u64).cmp(&right)
        });
    }
    if let (Some(left), Some(right)) = (left.as_u64(), right.as_i64()) {
        return Some(if right.is_negative() {
            Ordering::Greater
        } else {
            left.cmp(&(right as u64))
        });
    }

    left.as_f64()?.partial_cmp(&right.as_f64()?)
}

fn resolve_local_reference<'a>(
    root_schema: &'a Value,
    reference: &str,
) -> Result<&'a Value, String> {
    if reference == "#" {
        return Ok(root_schema);
    }
    let Some(pointer) = reference.strip_prefix('#') else {
        return Err(format!(
            "external schema reference is not supported: {reference}"
        ));
    };
    if !pointer.starts_with('/') {
        return Err(format!(
            "schema anchor reference is not supported: {reference}"
        ));
    }
    root_schema
        .pointer(pointer)
        .ok_or_else(|| format!("unresolved local schema reference: {reference}"))
}

fn collect_schema_paths(dir: &Path, paths: &mut Vec<PathBuf>) -> AdmResult<()> {
    if !dir.exists() {
        return Err(AdmError::new(format!(
            "schema directory does not exist: {}",
            dir.display()
        )));
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_schema_paths(&path, paths)?;
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("json")
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn expected_type_list(value: &Value) -> Vec<&str> {
    if let Some(item) = value.as_str() {
        vec![item]
    } else {
        value
            .as_array()
            .map(|items| items.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default()
    }
}

fn matches_json_type(data: &Value, expected_type: &str) -> bool {
    match expected_type {
        "object" => data.is_object(),
        "array" => data.is_array(),
        "string" => data.is_string(),
        "integer" => data.as_i64().is_some() || data.as_u64().is_some(),
        "number" => data.is_number(),
        "boolean" => data.is_boolean(),
        "null" => data.is_null(),
        _ => false,
    }
}

fn json_type_name(data: &Value) -> &'static str {
    match data {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) if number.is_i64() || number.is_u64() => "integer",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn type_name_from_schema_value(value: &Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.as_array()?.first()?.as_str())
}

fn render_expected_type(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        text.to_string()
    } else {
        value.to_string()
    }
}

fn normalize_path(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::{new_stable_id, paths::SourceProjectRoot};
    use serde_json::json;

    #[test]
    fn lightweight_validator_matches_python_subset() {
        let schema = json!({
            "type": "object",
            "required": ["name", "mode", "items"],
            "properties": {
                "name": {"type": "string"},
                "mode": {"type": "string", "enum": ["auto", "manual"]},
                "items": {"type": "array", "items": {"type": "integer"}},
                "optional": {"type": ["string", "null"]}
            }
        });

        assert_eq!(
            validate_contract(
                &json!({"name": "demo", "mode": "auto", "items": [1, 2], "optional": null}),
                &schema
            ),
            Vec::<String>::new()
        );

        let errors =
            validate_contract(&json!({"name": 1, "mode": "bad", "items": [true]}), &schema);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("$.name: expected string"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("$.mode: expected one of"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("$.items[0]: expected integer"))
        );

        let missing_errors = validate_contract(&json!({"name": "demo", "mode": "auto"}), &schema);
        assert!(
            missing_errors
                .iter()
                .any(|error| error.contains("$.items: required field missing"))
        );
    }

    #[test]
    fn any_of_accepts_the_first_matching_branch() {
        let schema = json!({"anyOf": [{"type": "string"}, {"type": "integer"}]});

        assert!(validate_contract(&json!("x"), &schema).is_empty());
        assert!(validate_contract(&json!(1), &schema).is_empty());
        assert!(!validate_contract(&json!(true), &schema).is_empty());
    }

    #[test]
    fn validates_collection_string_number_pattern_and_const_constraints() {
        let schema = json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "minItems": 2,
                    "maxItems": 3
                },
                "label": {
                    "type": "string",
                    "minLength": 2,
                    "maxLength": 4,
                    "pattern": "^[\\p{Han}A-Z]+$"
                },
                "score": {
                    "type": "number",
                    "minimum": 0.25,
                    "maximum": 0.75
                },
                "kind": {"const": "demo"}
            }
        });

        assert!(
            validate_contract(
                &json!({
                    "items": [1, 2],
                    "label": "中文A",
                    "score": 0.5,
                    "kind": "demo"
                }),
                &schema
            )
            .is_empty()
        );

        let errors = validate_contract(
            &json!({
                "items": [1, 2, 3, 4],
                "label": "a",
                "score": 1.0,
                "kind": "other"
            }),
            &schema,
        );
        assert!(errors.iter().any(|error| error.starts_with("$.items:")));
        assert!(
            errors
                .iter()
                .any(|error| error.starts_with("$.label:") && error.contains("at least 2"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.starts_with("$.label:") && error.contains("pattern"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.starts_with("$.score:") && error.contains("0.75"))
        );
        assert!(
            errors
                .iter()
                .any(|error| error.starts_with("$.kind:") && error.contains("constant"))
        );

        let too_short = validate_contract(&json!({"items": []}), &schema);
        assert!(
            too_short
                .iter()
                .any(|error| error.starts_with("$.items:") && error.contains("at least 2"))
        );
    }

    #[test]
    fn string_lengths_count_unicode_characters_instead_of_bytes() {
        let schema = json!({"type": "string", "minLength": 2, "maxLength": 2});

        assert!(validate_contract(&json!("中文"), &schema).is_empty());
        assert!(!validate_contract(&json!("中"), &schema).is_empty());
        assert!(!validate_contract(&json!("中文稿"), &schema).is_empty());
    }

    #[test]
    fn numeric_bounds_compare_large_integers_without_float_rounding() {
        let schema = json!({"type": "integer", "minimum": u64::MAX});

        assert!(validate_contract(&json!(u64::MAX), &schema).is_empty());
        assert!(
            validate_contract(&json!(u64::MAX - 1), &schema)
                .iter()
                .any(|error| error.contains("greater than or equal"))
        );
    }

    #[test]
    fn one_of_and_all_of_enforce_branch_cardinality_and_siblings() {
        let schema = json!({
            "type": "integer",
            "minimum": 0,
            "allOf": [{"maximum": 10}],
            "oneOf": [
                {"enum": [2, 4, 6, 8, 10]},
                {"enum": [1, 3, 5, 7, 9]}
            ]
        });

        assert!(validate_contract(&json!(4), &schema).is_empty());
        assert!(
            validate_contract(&json!(12), &schema)
                .iter()
                .any(|error| error.contains("less than or equal to 10"))
        );

        let ambiguous = json!({"oneOf": [{"type": "number"}, {"minimum": 0}]});
        assert!(
            validate_contract(&json!(1), &ambiguous)
                .iter()
                .any(|error| error.contains("matched 2 schemas"))
        );

        let any_of_with_sibling = json!({
            "anyOf": [{"type": "integer"}, {"type": "string"}],
            "const": 3
        });
        assert!(!validate_contract(&json!(2), &any_of_with_sibling).is_empty());
    }

    #[test]
    fn resolves_local_refs_and_defs_including_recursive_contracts() {
        let schema = json!({
            "$defs": {
                "node": {
                    "type": "object",
                    "required": ["value"],
                    "properties": {
                        "value": {"type": "integer"},
                        "next": {
                            "anyOf": [
                                {"type": "null"},
                                {"$ref": "#/$defs/node"}
                            ]
                        }
                    }
                }
            },
            "$ref": "#/$defs/node"
        });

        assert!(
            validate_contract(
                &json!({"value": 1, "next": {"value": 2, "next": null}}),
                &schema
            )
            .is_empty()
        );
        let errors = validate_contract(
            &json!({"value": 1, "next": {"value": "bad", "next": null}}),
            &schema,
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("$.next.value: expected integer"))
        );
    }

    #[test]
    fn unsupported_or_unresolved_refs_are_explicit_validation_errors() {
        let external =
            validate_contract(&json!({}), &json!({"$ref": "other.schema.json#/contract"}));
        assert!(
            external
                .iter()
                .any(|error| error.contains("external schema reference is not supported"))
        );

        let unresolved = validate_contract(&json!({}), &json!({"$ref": "#/$defs/missing"}));
        assert!(
            unresolved
                .iter()
                .any(|error| error.contains("unresolved local schema reference"))
        );
    }

    #[test]
    fn structured_json_loader_accepts_utf8_bom() {
        let root = std::env::temp_dir().join(new_stable_id("schema_bom").unwrap());
        let path = root.join("contract.json");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&path, b"\xEF\xBB\xBF{\"artifact_locale\":\"zh-CN\"}").unwrap();

        let document = load_structured_file(&path).unwrap();

        assert_eq!(document["artifact_locale"], "zh-CN");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn schema_registry_discovers_all_project_schema_files() {
        let project_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
            .into_path();
        let registry = discover_schema_registry(&project_root).unwrap();

        assert_eq!(registry.len(), EXPECTED_SCHEMA_FILE_COUNT);
        assert!(
            registry
                .get("knowledge/schemas/ai_design/project_identity_contract.schema.json")
                .is_some()
        );
        assert_eq!(
            registry.validate_inventory(EXPECTED_SCHEMA_FILE_COUNT),
            Vec::<String>::new()
        );
    }

    #[test]
    fn seed_contracts_validate_against_shared_schemas() {
        let project_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
            .into_path();
        let project_dna_schema = load_structured_file(
            &project_root.join("knowledge/schemas/ai_design/project_dna_contract.schema.json"),
        )
        .unwrap();
        let semantic_matrix_schema = load_structured_file(
            &project_root.join("knowledge/schemas/ai_design/semantic_coverage_matrix.schema.json"),
        )
        .unwrap();

        assert!(
            validate_contract(
                &json!({
                    "schema_version": "1.0",
                    "contract_state": "seed",
                    "project_signature": "demo"
                }),
                &project_dna_schema
            )
            .is_empty()
        );
        assert!(
            validate_contract(
                &json!({
                    "schema_version": "1.0",
                    "matrix_state": "seed",
                    "coverage_items": []
                }),
                &semantic_matrix_schema
            )
            .is_empty()
        );
    }

    #[test]
    fn validation_report_writer_matches_python_report_shape() {
        let root = std::env::temp_dir().join(new_stable_id("schema_report").unwrap());
        let report_path = root.join("report.json");
        let report = write_validation_report(
            &report_path,
            Path::new("contract.json"),
            Path::new("schema.json"),
            vec!["$.name: required field missing".to_string()],
        )
        .unwrap();

        assert!(!report.valid);
        let json: Value =
            serde_json::from_str(&std::fs::read_to_string(&report_path).unwrap()).unwrap();
        assert_eq!(json["contract"], "contract.json");
        assert_eq!(json["schema"], "schema.json");
        assert_eq!(json["valid"], false);
        let _ = std::fs::remove_dir_all(root);
    }
}
