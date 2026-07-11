use crate::data_loader::{
    DEFAULT_TEMPLATE_SCHEMA_VERSION, DesignDataLoader, ProjectTemplateDeleteResult,
    ProjectTemplateLoadReport, ProjectTemplateMeta, ProjectTemplatePayload,
    ProjectTemplateWriteResult, SCALE_ORDER, TEMPLATE_INDEX_FILE, collect_json_files,
    load_json_value, scale_label, string_from_value,
};
use adm_new_foundation::paths::relative_display;
use adm_new_foundation::{AdmError, AdmResult, unix_timestamp, write_text_atomic};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const BUILTIN_PREFIX: &str = "builtin_";
pub const CUSTOM_PREFIX: &str = "custom_";

#[derive(Debug)]
struct LoadedTemplateFile {
    path: PathBuf,
    payload: ProjectTemplatePayload,
}

#[derive(Debug)]
struct TemplateScan {
    templates: Vec<LoadedTemplateFile>,
    failures: Vec<(PathBuf, String)>,
}

pub fn list_project_templates(
    loader: &DesignDataLoader,
    include_internal: bool,
) -> AdmResult<Vec<ProjectTemplatePayload>> {
    Ok(list_project_templates_report(loader, include_internal)?.templates)
}

pub fn list_project_templates_report(
    loader: &DesignDataLoader,
    include_internal: bool,
) -> AdmResult<ProjectTemplateLoadReport> {
    let scan = scan_project_templates(loader, include_internal)?;
    Ok(ProjectTemplateLoadReport {
        templates: scan
            .templates
            .into_iter()
            .map(|loaded| loaded.payload)
            .collect(),
        warnings: scan
            .failures
            .into_iter()
            .map(|(path, error)| {
                format!(
                    "skipped invalid project template {}: {error}",
                    relative_display(&path, loader.project_root())
                )
            })
            .collect(),
    })
}

pub fn find_project_template(
    loader: &DesignDataLoader,
    template_id: &str,
) -> AdmResult<ProjectTemplatePayload> {
    let template_id = validated_template_id(template_id)?;
    let scan = scan_project_templates(loader, true)?;
    if let Some(loaded) = scan
        .templates
        .into_iter()
        .find(|loaded| loaded.payload.meta.id == template_id)
    {
        return Ok(loaded.payload);
    }
    if let Some((path, error)) = scan.failures.into_iter().find(|(path, _)| {
        path.file_stem()
            .and_then(|value| value.to_str())
            .is_some_and(|stem| stem == template_id)
    }) {
        return Err(AdmError::new(format!(
            "project template {template_id} is invalid ({}): {error}",
            path.display()
        )));
    }
    Err(AdmError::new(format!(
        "project template not found: {template_id}"
    )))
}

pub fn save_custom_project_template(
    loader: &DesignDataLoader,
    template_name: &str,
    target_scale: &str,
    project_state: Value,
    overwrite: bool,
) -> AdmResult<ProjectTemplateWriteResult> {
    let template_name = validated_template_name(template_name)?;
    let target_scale = validated_target_scale(target_scale)?;
    let scan = scan_project_templates(loader, true)?;

    if scan.templates.iter().any(|loaded| {
        loaded.payload.meta.source == "builtin"
            && same_template_key(&loaded.payload.meta, &template_name, &target_scale)
    }) {
        return Err(AdmError::new(format!(
            "cannot replace builtin project template with the same name and scale: {template_name} ({target_scale})"
        )));
    }

    let existing_custom = scan.templates.iter().find(|loaded| {
        loaded.payload.meta.source == "custom"
            && same_template_key(&loaded.payload.meta, &template_name, &target_scale)
    });
    if existing_custom.is_some() && !overwrite {
        return Err(AdmError::new(format!(
            "custom project template already exists; overwrite is required: {template_name} ({target_scale})"
        )));
    }

    let custom_dir = loader.custom_project_templates_dir();
    let generated_file_name = template_filename("custom", &target_scale, &template_name);
    let target_path = existing_custom
        .map(|loaded| loaded.path.clone())
        .unwrap_or_else(|| custom_dir.join(&generated_file_name));
    let target_file_name = target_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AdmError::new("custom project template path has no portable file name"))?
        .to_string();
    if !target_file_name.starts_with(CUSTOM_PREFIX)
        || target_path.parent() != Some(custom_dir.as_path())
    {
        return Err(AdmError::new(format!(
            "custom project template path escapes writable template directory: {}",
            target_path.display()
        )));
    }
    let target_exists = target_path.exists();
    if target_exists && existing_custom.is_none() {
        return Err(AdmError::new(format!(
            "custom project template file already exists for a different template name: {}",
            target_path.display()
        )));
    }

    let existing_created_at = existing_custom.and_then(|loaded| {
        loaded
            .payload
            .raw
            .pointer("/template/createdAt")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let payload = build_custom_template_payload(
        &template_name,
        &target_scale,
        &target_file_name,
        project_state,
        existing_created_at,
    )?;
    let text = serde_json::to_string_pretty(&payload).map_err(|error| {
        AdmError::new(format!(
            "failed to serialize custom project template {template_name}: {error}"
        ))
    })?;
    write_text_atomic(&target_path, &(text + "\n"))?;

    let mut template = load_template_file(&target_path)?;
    template.meta.path = relative_display(&target_path, loader.project_root());
    Ok(ProjectTemplateWriteResult {
        template,
        overwritten: target_exists,
    })
}

pub fn delete_custom_project_template(
    loader: &DesignDataLoader,
    template_id: &str,
) -> AdmResult<ProjectTemplateDeleteResult> {
    let template_id = validated_template_id(template_id)?;
    let scan = scan_project_templates(loader, true)?;
    let loaded = scan
        .templates
        .into_iter()
        .find(|loaded| loaded.payload.meta.id == template_id)
        .ok_or_else(|| AdmError::new(format!("project template not found: {template_id}")))?;
    let custom_dir = loader.custom_project_templates_dir();
    let file_name = loaded
        .path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    if loaded.payload.meta.source != "custom"
        || !file_name.starts_with(CUSTOM_PREFIX)
        || loaded.path.parent() != Some(custom_dir.as_path())
    {
        return Err(AdmError::new(format!(
            "cannot delete builtin project template: {template_id}"
        )));
    }
    fs::remove_file(&loaded.path).map_err(|error| {
        AdmError::new(format!(
            "failed to delete custom project template {}: {error}",
            loaded.path.display()
        ))
    })?;
    Ok(ProjectTemplateDeleteResult {
        template_id,
        template_name: loaded.payload.meta.name,
        target_scale: loaded.payload.meta.target_scale,
        file_name,
    })
}

pub fn load_template_file(path: &Path) -> AdmResult<ProjectTemplatePayload> {
    let payload = load_json_value(path)?;
    normalize_template_payload(payload, Some(path))
}

fn scan_project_templates(
    loader: &DesignDataLoader,
    include_internal: bool,
) -> AdmResult<TemplateScan> {
    let mut templates = BTreeMap::<String, LoadedTemplateFile>::new();
    let mut failures = Vec::new();
    for directory in template_dirs(loader) {
        for path in collect_json_files(&directory, false)? {
            let Some(file_name) = path
                .file_name()
                .and_then(|value| value.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            if file_name == TEMPLATE_INDEX_FILE {
                continue;
            }
            let mut payload = match load_template_file(&path) {
                Ok(payload) => payload,
                Err(error) => {
                    failures.push((path, error.to_string()));
                    continue;
                }
            };
            payload.meta.path = relative_display(&path, loader.project_root());
            if !include_internal && payload.meta.visibility == "internal" {
                continue;
            }
            let is_custom = payload.meta.source == "custom";
            if !templates.contains_key(&file_name) || is_custom {
                templates.insert(file_name, LoadedTemplateFile { path, payload });
            }
        }
    }
    let mut templates = templates.into_values().collect::<Vec<_>>();
    templates.sort_by(|left, right| sort_key(&left.payload).cmp(&sort_key(&right.payload)));
    Ok(TemplateScan {
        templates,
        failures,
    })
}

fn template_dirs(loader: &DesignDataLoader) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let bundled = loader.project_templates_dir();
    let writable = loader.custom_project_templates_dir();
    if bundled.exists() {
        dirs.push(bundled.clone());
    }
    if writable != bundled && writable.exists() {
        dirs.push(writable);
    }
    dirs
}

fn normalize_template_payload(
    mut payload: Value,
    path: Option<&Path>,
) -> AdmResult<ProjectTemplatePayload> {
    if !payload.is_object() {
        return Err(AdmError::new("project template root must be an object"));
    }
    if payload
        .get("template")
        .is_some_and(|value| !value.is_object())
    {
        return Err(AdmError::new("project template meta must be an object"));
    }
    if payload
        .get("projectState")
        .is_some_and(|value| !value.is_object())
    {
        return Err(AdmError::new(
            "project template projectState must be an object",
        ));
    }
    {
        let root = payload.as_object_mut().expect("validated object");
        root.entry("schemaVersion".to_string())
            .or_insert_with(|| Value::String(DEFAULT_TEMPLATE_SCHEMA_VERSION.to_string()));
        root.entry("projectState".to_string())
            .or_insert_with(|| json!({}));
    }

    let target_scale_from_state = payload
        .pointer("/projectState/profile/targetScale")
        .map(|value| string_from_value(Some(value)))
        .unwrap_or_default();
    let path_file_name = path
        .and_then(|path| path.file_name().and_then(|value| value.to_str()))
        .unwrap_or_default()
        .to_string();
    let path_stem = path
        .and_then(|path| path.file_stem().and_then(|value| value.to_str()))
        .unwrap_or_default()
        .to_string();
    let source_from_path = path.map(source_from_filename);

    {
        let root = payload.as_object_mut().expect("validated object");
        let template = ensure_object_entry(root, "template");
        if !path_file_name.is_empty() {
            template.insert(
                "fileName".to_string(),
                Value::String(path_file_name.clone()),
            );
        }
        if let Some(source) = source_from_path {
            template.insert("source".to_string(), Value::String(source));
        }
        template
            .entry("source".to_string())
            .or_insert_with(|| Value::String("custom".to_string()));
        let id_fallback = if path_stem.is_empty() {
            safe_template_slug(&string_from_value(template.get("name")), "project_template")
        } else {
            path_stem.clone()
        };
        if string_from_value(template.get("id")).trim().is_empty() {
            template.insert("id".to_string(), Value::String(id_fallback));
        }
        let name_fallback = {
            let game_name = string_from_value(template.get("gameName"));
            if game_name.trim().is_empty() {
                let id = string_from_value(template.get("id"));
                if id.trim().is_empty() {
                    "未命名模板".to_string()
                } else {
                    id
                }
            } else {
                game_name
            }
        };
        if string_from_value(template.get("name")).trim().is_empty() {
            template.insert("name".to_string(), Value::String(name_fallback));
        }
        let game_name_fallback = string_from_value(template.get("name"));
        if string_from_value(template.get("gameName"))
            .trim()
            .is_empty()
        {
            template.insert("gameName".to_string(), Value::String(game_name_fallback));
        }
        let target_scale = if target_scale_from_state.trim().is_empty() {
            "unknown".to_string()
        } else {
            target_scale_from_state.clone()
        };
        if string_from_value(template.get("targetScale"))
            .trim()
            .is_empty()
        {
            template.insert("targetScale".to_string(), Value::String(target_scale));
        }
        let source = string_from_value(template.get("source"));
        template
            .entry("qualityTier".to_string())
            .or_insert_with(|| {
                Value::String(if source == "custom" {
                    "custom".to_string()
                } else {
                    "B".to_string()
                })
            });
        template
            .entry("sourceLabel".to_string())
            .or_insert_with(|| {
                Value::String(
                    match source.as_str() {
                        "builtin" => "内置",
                        "custom" => "自定义",
                        other => other,
                    }
                    .to_string(),
                )
            });
        let scale_value = string_from_value(template.get("targetScale"));
        template
            .entry("scaleLabel".to_string())
            .or_insert_with(|| Value::String(scale_label(&scale_value)));
        template
            .entry("summary".to_string())
            .or_insert_with(|| Value::String(String::new()));
        template
            .entry("analysis".to_string())
            .or_insert_with(|| json!([]));
        template
            .entry("verification".to_string())
            .or_insert_with(|| json!({}));
    }

    let schema_version = payload
        .get("schemaVersion")
        .map(|value| string_from_value(Some(value)))
        .unwrap_or_else(|| DEFAULT_TEMPLATE_SCHEMA_VERSION.to_string());
    let meta = ProjectTemplateMeta::from_value(payload.get("template").unwrap_or(&Value::Null))?;
    let project_state = payload
        .get("projectState")
        .cloned()
        .unwrap_or_else(|| json!({}));
    Ok(ProjectTemplatePayload {
        schema_version,
        meta,
        project_state,
        raw: payload,
    })
}

fn build_custom_template_payload(
    template_name: &str,
    target_scale: &str,
    file_name: &str,
    mut project_state: Value,
    existing_created_at: Option<String>,
) -> AdmResult<Value> {
    let state = project_state
        .as_object_mut()
        .ok_or_else(|| AdmError::new("project state must be an object"))?;
    state.remove("aiInterview");
    let profile = ensure_object_entry(state, "profile");
    profile.insert(
        "targetScale".to_string(),
        Value::String(target_scale.to_string()),
    );
    let now = format!("unix:{}", unix_timestamp());
    let template_id = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("custom_project_template")
        .to_string();
    Ok(json!({
        "schemaVersion": DEFAULT_TEMPLATE_SCHEMA_VERSION,
        "template": {
            "id": template_id,
            "source": "custom",
            "sourceLabel": "",
            "name": template_name,
            "gameName": template_name,
            "targetScale": target_scale,
            "scaleLabel": scale_label(target_scale),
            "qualityTier": "custom",
            "summary": "",
            "analysis": [],
            "verification": {
                "mode": "user_saved",
                "createdAt": now,
                "runtimeNetwork": "none"
            },
            "fileName": file_name,
            "createdAt": existing_created_at.unwrap_or_else(|| now.clone()),
            "updatedAt": now
        },
        "projectState": project_state
    }))
}

fn sort_key(payload: &ProjectTemplatePayload) -> (usize, usize, i64, String) {
    let scale_rank = SCALE_ORDER
        .iter()
        .position(|scale| *scale == payload.meta.target_scale)
        .unwrap_or(99);
    let source_rank = if payload.meta.source == "builtin" {
        0
    } else {
        1
    };
    (
        scale_rank,
        source_rank,
        payload.meta.order.unwrap_or(999),
        payload.meta.name.clone(),
    )
}

fn source_from_filename(path: &Path) -> String {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if name.starts_with(BUILTIN_PREFIX) {
        "builtin".to_string()
    } else {
        "custom".to_string()
    }
}

fn template_filename(source: &str, target_scale: &str, name: &str) -> String {
    let prefix = if source == "custom" {
        CUSTOM_PREFIX
    } else {
        BUILTIN_PREFIX
    };
    format!(
        "{prefix}{}_{}.json",
        safe_template_slug(target_scale, "unknown"),
        safe_template_slug(name, "project_template")
    )
}

fn safe_template_slug(value: &str, fallback: &str) -> String {
    let mut output = String::new();
    let mut last_was_separator = false;
    for ch in value.trim().chars() {
        let forbidden =
            ch.is_control() || matches!(ch, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|');
        if forbidden || ch.is_whitespace() {
            if !last_was_separator && !output.is_empty() {
                output.push('_');
                last_was_separator = true;
            }
        } else {
            output.push(ch);
            last_was_separator = false;
        }
    }
    let output = output.trim_matches(['.', '_', ' ', '-']).to_string();
    if output.is_empty() {
        fallback.to_string()
    } else {
        output
    }
}

fn same_template_key(meta: &ProjectTemplateMeta, name: &str, target_scale: &str) -> bool {
    meta.name.trim().to_lowercase() == name.trim().to_lowercase()
        && meta.target_scale.trim() == target_scale.trim()
}

fn required_value(value: &str, field: &str) -> AdmResult<String> {
    let value = value.trim();
    if value.is_empty() {
        Err(AdmError::new(format!("{field} must not be empty")))
    } else {
        Ok(value.to_string())
    }
}

fn validated_template_id(value: &str) -> AdmResult<String> {
    let value = required_value(value, "template_id")?;
    if value.chars().any(char::is_control) {
        return Err(AdmError::new(
            "template_id must not contain control characters",
        ));
    }
    if value.chars().count() > 200 {
        return Err(AdmError::new("template_id must not exceed 200 characters"));
    }
    Ok(value)
}

fn validated_template_name(value: &str) -> AdmResult<String> {
    let value = required_value(value, "template_name")?;
    if value.chars().any(char::is_control) {
        return Err(AdmError::new(
            "template_name must not contain control characters",
        ));
    }
    if value.chars().count() > 120 {
        return Err(AdmError::new(
            "template_name must not exceed 120 characters",
        ));
    }
    Ok(value)
}

fn validated_target_scale(value: &str) -> AdmResult<String> {
    let value = required_value(value, "target_scale")?;
    if !SCALE_ORDER.contains(&value.as_str()) {
        return Err(AdmError::new(format!(
            "target_scale must be one of: {}",
            SCALE_ORDER.join(", ")
        )));
    }
    Ok(value)
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

impl ProjectTemplateMeta {
    fn from_value(value: &Value) -> AdmResult<Self> {
        if !value.is_object() {
            return Err(AdmError::new("project template meta must be an object"));
        }
        Ok(Self {
            id: string_field(value, "id"),
            source: string_field(value, "source"),
            source_label: string_field(value, "sourceLabel"),
            name: string_field(value, "name"),
            game_name: string_field(value, "gameName"),
            target_scale: string_field(value, "targetScale"),
            scale_label: string_field(value, "scaleLabel"),
            quality_tier: string_field(value, "qualityTier"),
            summary: string_field(value, "summary"),
            visibility: string_field_or(value, "visibility", "public"),
            file_name: string_field(value, "fileName"),
            path: string_field(value, "path"),
            order: value.get("order").and_then(Value::as_i64),
            analysis: value
                .get("analysis")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            verification: value
                .get("verification")
                .cloned()
                .unwrap_or_else(|| json!({})),
        })
    }
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .map(|value| string_from_value(Some(value)))
        .unwrap_or_default()
}

fn string_field_or(value: &Value, field: &str, fallback: &str) -> String {
    let value = string_field(value, field);
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;

    #[test]
    fn list_skips_a_corrupt_template_and_reports_a_warning() {
        let root = temp_root("list_warning");
        let loader =
            DesignDataLoader::new(&root).with_runtime_root(root.join("drafts").join("current"));
        fs::create_dir_all(loader.project_templates_dir()).unwrap();
        write_template(
            &loader.project_templates_dir().join("builtin_indie_ok.json"),
            "builtin_indie_ok",
            "Good",
            "indie",
        );
        fs::write(
            loader
                .project_templates_dir()
                .join("builtin_indie_bad.json"),
            "{not-json",
        )
        .unwrap();

        let report = list_project_templates_report(&loader, true).unwrap();
        assert_eq!(report.templates.len(), 1);
        assert_eq!(report.templates[0].meta.id, "builtin_indie_ok");
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("builtin_indie_bad.json"));
        cleanup(root);
    }

    #[test]
    fn custom_template_lifecycle_enforces_collisions_and_strips_ai_interview() {
        let root = temp_root("custom_lifecycle");
        let loader =
            DesignDataLoader::new(&root).with_runtime_root(root.join("drafts").join("current"));
        fs::create_dir_all(loader.project_templates_dir()).unwrap();
        write_template(
            &loader
                .project_templates_dir()
                .join("builtin_indie_builtin.json"),
            "builtin_indie_builtin",
            "Builtin",
            "indie",
        );
        let state = json!({
            "projectName": "Current",
            "profile": {},
            "aiInterview": {"answers": ["private"]}
        });

        assert!(find_project_template(&loader, "").is_err());
        assert!(delete_custom_project_template(&loader, "").is_err());
        assert!(
            save_custom_project_template(&loader, "Bad Scale", "planet", state.clone(), false)
                .unwrap_err()
                .to_string()
                .contains("target_scale must be one of")
        );
        assert!(
            save_custom_project_template(&loader, "Bad\nName", "indie", state.clone(), false,)
                .unwrap_err()
                .to_string()
                .contains("control characters")
        );

        let builtin_error =
            save_custom_project_template(&loader, "Builtin", "indie", state.clone(), false)
                .unwrap_err();
        assert!(builtin_error.to_string().contains("builtin"));

        let first =
            save_custom_project_template(&loader, "中文 模板", "indie", state.clone(), false)
                .unwrap();
        assert!(!first.overwritten);
        assert!(first.template.meta.file_name.contains("中文_模板"));
        assert!(first.template.meta.source_label.is_empty());
        assert!(first.template.meta.summary.is_empty());
        assert!(first.template.meta.analysis.is_empty());
        assert!(first.template.project_state.get("aiInterview").is_none());
        assert_eq!(
            first.template.project_state.pointer("/profile/targetScale"),
            Some(&json!("indie"))
        );

        let duplicate =
            save_custom_project_template(&loader, "中文 模板", "indie", state.clone(), false)
                .unwrap_err();
        assert!(duplicate.to_string().contains("overwrite is required"));
        let overwritten =
            save_custom_project_template(&loader, "中文 模板", "indie", state, true).unwrap();
        assert!(overwritten.overwritten);

        let collision_state = json!({"projectName": "Collision", "profile": {}});
        let collision_owner =
            save_custom_project_template(&loader, "A B", "midcore", collision_state.clone(), false)
                .unwrap();
        let collision =
            save_custom_project_template(&loader, "A:B", "midcore", collision_state, true)
                .unwrap_err();
        assert!(collision.to_string().contains("different template name"));
        delete_custom_project_template(&loader, &collision_owner.template.meta.id).unwrap();

        let builtin_delete =
            delete_custom_project_template(&loader, "builtin_indie_builtin").unwrap_err();
        assert!(builtin_delete.to_string().contains("cannot delete builtin"));
        let deleted =
            delete_custom_project_template(&loader, &overwritten.template.meta.id).unwrap();
        assert_eq!(deleted.template_name, "中文 模板");
        assert!(
            list_project_templates(&loader, true)
                .unwrap()
                .iter()
                .all(|template| template.meta.id != overwritten.template.meta.id)
        );
        cleanup(root);
    }

    fn write_template(path: &Path, id: &str, name: &str, target_scale: &str) {
        fs::write(
            path,
            serde_json::to_string_pretty(&json!({
                "schemaVersion": "0.1.0",
                "template": {
                    "id": id,
                    "name": name,
                    "gameName": name,
                    "targetScale": target_scale
                },
                "projectState": {
                    "projectName": name,
                    "profile": {"targetScale": target_scale}
                }
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm-newrust-project-template-{label}-{}",
            new_stable_id("test").unwrap()
        ))
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
