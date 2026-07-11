use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use adm_new_contracts::project::{NodeState, ProjectState};
use adm_new_design::handoff::export_concept_package_from_state;
use adm_new_design::{
    DesignChecklistItemSpec, DesignEngineService, DesignNodeSpec, DesignOptionGroupSpec,
};
use adm_new_foundation::io::{read_json, write_json, write_json_serializable};
use adm_new_foundation::paths::relative_display;
use adm_new_foundation::structured_md::{read_structured_or_text, write_data};
use adm_new_foundation::{AdmError, AdmResult, unix_timestamp};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::execution_objects::{ExecutionObjectStoreService, save_design_project};

const DEFAULT_SKIP_PARTS: &[&str] = &[
    ".git",
    "__pycache__",
    "build",
    "dist",
    "_archive",
    "workspace",
];
const TEXT_SUFFIXES: &[&str] = &[
    "py", "json", "toml", "md", "txt", "yaml", "yml", "ps1", "spec", "csv",
];
const STORE_RELATIVE_PATH: &str = "workspace/outputs/execution_objects/execution_objects.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardcodedPathHit {
    pub path: String,
    pub line_number: usize,
    pub line: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardcodedPathScanReport {
    pub root: String,
    pub allow_docs: bool,
    pub hit_count: usize,
    pub hits: Vec<HardcodedPathHit>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationItem {
    pub source: String,
    pub target: String,
    pub size: u64,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyMigrationReport {
    pub generated_at: String,
    pub dry_run: bool,
    pub source: String,
    pub target: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub items: Vec<MigrationItem>,
    pub truncated: bool,
    pub report_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionObjectSaveIdMigrationRow {
    pub save_id: String,
    pub path: String,
    pub status: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionObjectSaveIdMigrationReport {
    pub save_root: String,
    pub apply: bool,
    pub rows: Vec<ExecutionObjectSaveIdMigrationRow>,
    pub report_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignProjectMigrationItem {
    pub file_path: String,
    pub project_name: String,
    pub execution_object_id: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignProjectMigrationReport {
    pub status: String,
    pub dry_run: bool,
    pub project_root: String,
    pub store_path: String,
    pub migrated_count: usize,
    pub error_count: usize,
    pub items: Vec<DesignProjectMigrationItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaMigrationReport {
    pub input_path: String,
    pub schema_path: String,
    pub output_path: String,
    pub old_version: String,
    pub new_version: String,
    pub rule_count: usize,
    pub applied_rule_count: usize,
    pub warning: Option<String>,
}

pub fn scan_hardcoded_paths(
    root: impl AsRef<Path>,
    allow_docs: bool,
) -> AdmResult<HardcodedPathScanReport> {
    let root = root.as_ref();
    let mut hits = Vec::new();
    collect_text_files(root, root, &mut |path| {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                hits.push(HardcodedPathHit {
                    path: relative_display(path, root),
                    line_number: 0,
                    line: format!("read failed: {error}"),
                });
                return Ok(());
            }
        };
        let relative = relative_display(path, root);
        for (index, line) in text.lines().enumerate() {
            if line_has_legacy_path(line) {
                hits.push(HardcodedPathHit {
                    path: relative.clone(),
                    line_number: index + 1,
                    line: line.trim().chars().take(240).collect(),
                });
            }
        }
        Ok(())
    })?;
    if allow_docs {
        hits.retain(|hit| !hit.path.to_ascii_lowercase().starts_with("docs/"));
    }
    Ok(HardcodedPathScanReport {
        root: root.display().to_string(),
        allow_docs,
        hit_count: hits.len(),
        status: if hits.is_empty() {
            "passed".to_string()
        } else {
            "failed".to_string()
        },
        hits,
    })
}

pub fn run_legacy_migration(
    project_root: impl AsRef<Path>,
    source: impl AsRef<Path>,
    target: impl AsRef<Path>,
    apply: bool,
) -> AdmResult<LegacyMigrationReport> {
    let project_root = project_root.as_ref();
    let source = source.as_ref();
    if !source.exists() {
        return Err(AdmError::new(format!(
            "migration source does not exist: {}",
            source.display()
        )));
    }
    let target = if target.as_ref().is_absolute() {
        target.as_ref().to_path_buf()
    } else {
        project_root.join(target)
    };
    let mut items = plan_copy(source, &target)?;
    items.sort_by(|left, right| left.source.cmp(&right.source));
    if apply {
        for item in &items {
            let source_path = Path::new(&item.source);
            let target_path = Path::new(&item.target);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source_path, target_path)?;
        }
    }
    let report_path = project_root
        .join("workspace")
        .join("outputs")
        .join("migration_report.json");
    let report = LegacyMigrationReport {
        generated_at: format!("unix:{}", unix_timestamp()),
        dry_run: !apply,
        source: source.display().to_string(),
        target: target.display().to_string(),
        file_count: items.len(),
        total_bytes: items.iter().map(|item| item.size).sum(),
        truncated: items.len() > 1000,
        items: items.into_iter().take(1000).collect(),
        report_path: report_path.display().to_string(),
    };
    write_json_serializable(&report_path, &report)?;
    Ok(report)
}

pub fn migrate_execution_object_save_ids(
    save_root: impl AsRef<Path>,
    apply: bool,
    report_path: Option<impl AsRef<Path>>,
) -> AdmResult<ExecutionObjectSaveIdMigrationReport> {
    let save_root = save_root.as_ref();
    let mut rows = Vec::new();
    if save_root.is_dir() {
        let mut save_dirs = fs::read_dir(save_root)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_dir()
                    && path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(|name| name.starts_with("save_"))
            })
            .collect::<Vec<_>>();
        save_dirs.sort();
        for save_dir in save_dirs {
            let save_id = save_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string();
            let store_path = save_dir.join(STORE_RELATIVE_PATH);
            let mut data = read_optional_object(&store_path)?;
            let status = status_for_save_id(&save_id, data.as_ref());
            let mut action = "none".to_string();
            if status == "missing_save_id" {
                if let Some(object) = data.as_mut() {
                    action = "would_update".to_string();
                    if apply {
                        let backup_path = store_path.with_file_name(format!(
                            "{}.bak",
                            store_path
                                .file_name()
                                .and_then(|value| value.to_str())
                                .unwrap_or("execution_objects.json")
                        ));
                        if !backup_path.exists() && store_path.exists() {
                            fs::copy(&store_path, backup_path)?;
                        }
                        object.insert("save_id".to_string(), Value::String(save_id.clone()));
                        write_json(&store_path, &Value::Object(object.clone()))?;
                        action = "updated".to_string();
                    }
                }
            }
            rows.push(ExecutionObjectSaveIdMigrationRow {
                save_id,
                path: store_path.display().to_string(),
                status,
                action,
            });
        }
    }
    let report_path_string = if let Some(path) = report_path {
        let path = path.as_ref();
        write_execution_object_save_id_markdown(&rows, path)?;
        Some(path.display().to_string())
    } else {
        None
    };
    Ok(ExecutionObjectSaveIdMigrationReport {
        save_root: save_root.display().to_string(),
        apply,
        rows,
        report_path: report_path_string,
    })
}

pub fn migrate_design_projects_to_execution_objects(
    project_root: impl AsRef<Path>,
    workspace_projects_dir: Option<impl AsRef<Path>>,
    store_path: impl AsRef<Path>,
    backup: bool,
    delete_originals: bool,
    dry_run: bool,
) -> AdmResult<DesignProjectMigrationReport> {
    let project_root = project_root.as_ref();
    let store_path = store_path.as_ref();
    let mut files = find_design_project_files(project_root, workspace_projects_dir)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut items = Vec::new();
    let mut store = if dry_run {
        None
    } else {
        Some(ExecutionObjectStoreService::new(store_path, None)?)
    };
    for (path, data) in files {
        let project_name = string_field(&data, "projectName", "未命名");
        if dry_run {
            items.push(DesignProjectMigrationItem {
                file_path: path.display().to_string(),
                project_name,
                execution_object_id: None,
                status: "would_migrate".to_string(),
                error: None,
            });
            continue;
        }
        let result = (|| -> AdmResult<String> {
            if backup {
                let backup_path = path.with_extension("json.bak");
                fs::write(&backup_path, fs::read_to_string(&path)?)?;
            }
            let object = save_design_project(
                store.as_mut().expect("store exists when not dry-run"),
                data.clone(),
                Some(&format!("[迁移] {project_name}")),
                "migration",
                true,
            )?;
            if delete_originals {
                fs::remove_file(&path)?;
            }
            Ok(object.execution_object_id)
        })();
        match result {
            Ok(object_id) => items.push(DesignProjectMigrationItem {
                file_path: path.display().to_string(),
                project_name,
                execution_object_id: Some(object_id),
                status: "migrated".to_string(),
                error: None,
            }),
            Err(error) => items.push(DesignProjectMigrationItem {
                file_path: path.display().to_string(),
                project_name,
                execution_object_id: None,
                status: "failed".to_string(),
                error: Some(error.to_string()),
            }),
        }
    }
    let migrated_count = items
        .iter()
        .filter(|item| item.status == "migrated")
        .count();
    let error_count = items.iter().filter(|item| item.status == "failed").count();
    Ok(DesignProjectMigrationReport {
        status: if error_count == 0 {
            "success".to_string()
        } else {
            "partial".to_string()
        },
        dry_run,
        project_root: project_root.display().to_string(),
        store_path: store_path.display().to_string(),
        migrated_count,
        error_count,
        items,
    })
}

pub fn inspect_pipeline_reports(
    artifacts_dir: impl AsRef<Path>,
    step: Option<u32>,
    max_step: Option<u32>,
) -> AdmResult<Vec<Value>> {
    let artifacts_dir = artifacts_dir.as_ref();
    let steps = if let Some(step) = step {
        vec![step]
    } else if let Some(max_step) = max_step {
        (0..=max_step).collect()
    } else {
        discover_stage_numbers(artifacts_dir)?
    };
    Ok(steps
        .into_iter()
        .map(|step| load_stage_report(artifacts_dir, step))
        .collect())
}

pub fn migrate_structured_schema(
    input_path: impl AsRef<Path>,
    schema_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> AdmResult<SchemaMigrationReport> {
    let input_path = input_path.as_ref();
    let schema_path = schema_path.as_ref();
    let output_path = output_path.as_ref();
    let old_data = read_structured_or_text(input_path)?;
    let schema = read_structured_or_text(schema_path)?;
    let rules = schema
        .get("schema_migration")
        .and_then(|value| value.get("migration_rules"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut new_data = old_data.clone();
    let old_version = old_data
        .get("alignment_version")
        .and_then(Value::as_str)
        .unwrap_or("2.0")
        .to_string();
    let mut applied_rule_count = 0usize;
    let mut warning = None;
    if rules.is_empty() {
        warning = Some("no migration rules found; copied data".to_string());
    } else {
        let applicable = rules
            .iter()
            .filter(|rule| rule.get("from").and_then(Value::as_str) == Some(old_version.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if applicable.is_empty() {
            warning = Some(format!(
                "no migration rules from version {old_version}; output may be incompatible"
            ));
        } else {
            for rule in &applicable {
                apply_schema_migration_rule(&mut new_data, rule);
            }
            applied_rule_count = applicable.len();
        }
        let new_version = schema
            .get("contract_version")
            .and_then(Value::as_str)
            .unwrap_or("2.1");
        if let Some(object) = new_data.as_object_mut() {
            object.insert(
                "alignment_version".to_string(),
                Value::String(new_version.to_string()),
            );
        }
    }
    let new_version = new_data
        .get("alignment_version")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    write_data(output_path, &new_data, "Data")?;
    Ok(SchemaMigrationReport {
        input_path: input_path.display().to_string(),
        schema_path: schema_path.display().to_string(),
        output_path: output_path.display().to_string(),
        old_version,
        new_version,
        rule_count: rules.len(),
        applied_rule_count,
        warning,
    })
}

pub fn export_design_concept_package(
    project_state_path: impl AsRef<Path>,
    target_dir: impl AsRef<Path>,
    workspace_mirror: Option<impl AsRef<Path>>,
) -> AdmResult<Value> {
    let project_state_path = project_state_path.as_ref();
    let target_dir = target_dir.as_ref();
    let value = read_json(project_state_path, Value::Null);
    if value.is_null() {
        return Err(AdmError::new(format!(
            "project state is missing or invalid JSON: {}",
            project_state_path.display()
        )));
    }
    let mut state: ProjectState = serde_json::from_value(value)
        .map_err(|error| AdmError::new(format!("failed to parse project state: {error}")))?;
    let engine = design_engine_from_project_state(&state);
    state = engine.normalize_state(state);
    let package = export_concept_package_from_state(target_dir, &engine, &state)?;
    if let Some(mirror) = workspace_mirror {
        let mirror = mirror.as_ref();
        if mirror != target_dir {
            copy_dir_recursive(target_dir, mirror)?;
        }
    }
    Ok(package)
}

fn collect_text_files<F>(root: &Path, current: &Path, visit: &mut F) -> AdmResult<()>
where
    F: FnMut(&Path) -> AdmResult<()>,
{
    if !current.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if relative.components().any(|part| {
                DEFAULT_SKIP_PARTS.contains(&part.as_os_str().to_string_lossy().as_ref())
            }) {
                continue;
            }
            collect_text_files(root, &path, visit)?;
        } else if file_type.is_file() && is_text_suffix(&path) {
            visit(&path)?;
        }
    }
    Ok(())
}

fn line_has_legacy_path(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    let slash = lower.replace('\\', "/");
    slash.contains("e:/workwork/crewai/newdemotower")
        || (slash.contains("e:/workwork/crewai/") && slash.contains("/new_tools"))
        || slash.contains("newdemotower/工程运行文件")
        || slash.contains("全流程ai设计/new_tools")
}

fn is_text_suffix(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|suffix| TEXT_SUFFIXES.contains(&suffix.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn plan_copy(source: &Path, target: &Path) -> AdmResult<Vec<MigrationItem>> {
    let mut items = Vec::new();
    if source.is_file() {
        items.push(MigrationItem {
            source: source.display().to_string(),
            target: target.display().to_string(),
            size: source.metadata()?.len(),
            action: "copy".to_string(),
        });
        return Ok(items);
    }
    collect_copy_items(source, source, target, &mut items)?;
    Ok(items)
}

fn collect_copy_items(
    root: &Path,
    current: &Path,
    target: &Path,
    items: &mut Vec<MigrationItem>,
) -> AdmResult<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path);
        if should_skip_legacy_copy(relative) {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_copy_items(root, &path, target, items)?;
        } else if file_type.is_file() {
            items.push(MigrationItem {
                source: path.display().to_string(),
                target: target.join(relative).display().to_string(),
                size: entry.metadata()?.len(),
                action: "copy".to_string(),
            });
        }
    }
    Ok(())
}

fn should_skip_legacy_copy(relative: &Path) -> bool {
    relative.components().any(|part| {
        matches!(
            part.as_os_str().to_string_lossy().as_ref(),
            "__pycache__" | ".git" | "build" | "dist"
        )
    })
}

fn read_optional_object(path: &Path) -> AdmResult<Option<Map<String, Value>>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let value: Value = serde_json::from_str(text).map_err(|error| {
        AdmError::new(format!("failed to parse json {}: {error}", path.display()))
    })?;
    value
        .as_object()
        .cloned()
        .ok_or_else(|| AdmError::new(format!("store must be a JSON object: {}", path.display())))
        .map(Some)
}

fn status_for_save_id(save_id: &str, data: Option<&Map<String, Value>>) -> String {
    let Some(data) = data else {
        return "missing".to_string();
    };
    let current = data.get("save_id").and_then(Value::as_str).unwrap_or("");
    if current == save_id {
        "consistent".to_string()
    } else if current.is_empty() {
        "missing_save_id".to_string()
    } else {
        format!("mismatch:{current}")
    }
}

fn write_execution_object_save_id_markdown(
    rows: &[ExecutionObjectSaveIdMigrationRow],
    report_path: &Path,
) -> AdmResult<()> {
    let mut lines = vec![
        "# Execution Object Save ID Migration Dry Run".to_string(),
        String::new(),
        "| save_id | status | action | path |".to_string(),
        "|---|---|---|---|".to_string(),
    ];
    for row in rows {
        lines.push(format!(
            "| `{}` | {} | {} | `{}` |",
            row.save_id, row.status, row.action, row.path
        ));
    }
    if let Some(parent) = report_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(report_path, format!("{}\n", lines.join("\n")))?;
    Ok(())
}

fn find_design_project_files(
    project_root: &Path,
    workspace_projects_dir: Option<impl AsRef<Path>>,
) -> AdmResult<Vec<(PathBuf, Value)>> {
    let mut files = Vec::new();
    collect_design_project_json(&project_root.join("projects"), &mut files)?;
    if let Some(dir) = workspace_projects_dir {
        collect_design_project_json(dir.as_ref(), &mut files)?;
    }
    Ok(files)
}

fn collect_design_project_json(dir: &Path, files: &mut Vec<(PathBuf, Value)>) -> AdmResult<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file()
            || path.extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let value = read_json(&path, Value::Null);
        if value.is_object() && value.get("projectName").is_some() {
            files.push((path, value));
        }
    }
    Ok(())
}

fn string_field(value: &Value, field: &str, fallback: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn discover_stage_numbers(artifacts_dir: &Path) -> AdmResult<Vec<u32>> {
    let mut steps = Vec::new();
    if artifacts_dir.is_dir() {
        for entry in fs::read_dir(artifacts_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(number) = name
                .strip_prefix("stage_")
                .and_then(|tail| tail.parse().ok())
            {
                steps.push(number);
            }
        }
    }
    steps.sort_unstable();
    Ok(steps)
}

fn load_stage_report(artifacts_dir: &Path, step: u32) -> Value {
    let stage_dir = artifacts_dir.join(format!("stage_{step:02}"));
    let report_path = stage_dir.join("validation_report.json");
    let layer_path = stage_dir.join("artifact_validation_layer.json");
    if !report_path.exists() {
        return json!({
            "step": step,
            "status": "missing",
            "path": report_path.display().to_string(),
        });
    }
    let mut data = read_json(
        &report_path,
        json!({
            "step": step,
            "status": "invalid_json",
            "path": report_path.display().to_string(),
        }),
    );
    if let Some(object) = data.as_object_mut() {
        object.insert("step".to_string(), json!(step));
        object.insert(
            "path".to_string(),
            json!(relative_display(&report_path, artifacts_dir)),
        );
        object.insert(
            "artifact_layer".to_string(),
            if layer_path.exists() {
                read_json(&layer_path, json!({"status": "invalid_json"}))
            } else {
                json!({"status": "missing", "path": layer_path.display().to_string()})
            },
        );
    }
    data
}

fn apply_schema_migration_rule(data: &mut Value, rule: &Value) {
    if rule.get("action").and_then(Value::as_str) != Some("wrap_to_object") {
        return;
    }
    let field = rule.get("field").and_then(Value::as_str).unwrap_or("");
    let new_field = rule.get("new_field").and_then(Value::as_str).unwrap_or("");
    let structure = rule
        .get("structure")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if field.is_empty() || new_field.is_empty() {
        return;
    }
    let Some(assets) = data.get_mut("unified_assets").and_then(Value::as_array_mut) else {
        return;
    };
    for asset in assets {
        let Some(asset_object) = asset.as_object_mut() else {
            continue;
        };
        let Some(old_value) = asset_object.remove(field) else {
            continue;
        };
        let wrapped = structure
            .iter()
            .map(|(key, value)| {
                let new_value = if value
                    .as_str()
                    .is_some_and(|text| text.strip_prefix('$') == Some("old_frames"))
                {
                    old_value.clone()
                } else {
                    value.clone()
                };
                (key.clone(), new_value)
            })
            .collect::<Map<_, _>>();
        asset_object.insert(new_field.to_string(), Value::Object(wrapped));
    }
}

fn design_engine_from_project_state(state: &ProjectState) -> DesignEngineService {
    let mut domain_ids = BTreeMap::<String, BTreeSet<String>>::new();
    let specs = state
        .nodes
        .iter()
        .map(|(node_id, node)| {
            let domain_id = node_domain_id(node_id, node);
            domain_ids
                .entry(domain_id.clone())
                .or_default()
                .insert(node_id.clone());
            DesignNodeSpec {
                node_id: node_id.clone(),
                domain_id,
                name: humanize_id(node_id),
                description: String::new(),
                role_class: "migrated".to_string(),
                checklist: checklist_specs(node),
            }
        })
        .collect::<Vec<_>>();
    DesignEngineService::new(specs)
}

fn node_domain_id(node_id: &str, _node: &NodeState) -> String {
    node_id
        .split(['.', ':', '/'])
        .next()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("migrated_design")
        .to_string()
}

fn checklist_specs(node: &NodeState) -> Vec<DesignChecklistItemSpec> {
    node.checklist
        .keys()
        .map(|item_id| DesignChecklistItemSpec {
            item_id: item_id.clone(),
            label: humanize_id(item_id),
            option_groups: node
                .checklist_options
                .get(item_id)
                .map(|groups| {
                    groups
                        .keys()
                        .map(|group_id| DesignOptionGroupSpec {
                            group_id: group_id.clone(),
                            selection_mode: "multi".to_string(),
                            allow_primary: false,
                            options: Vec::new(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect()
}

fn humanize_id(value: &str) -> String {
    let label = value
        .replace(['_', '-', '.', ':', '/'], " ")
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    if label.is_empty() {
        "Migrated Node".to_string()
    } else {
        label
    }
}

fn copy_dir_recursive(source: &Path, target: &Path) -> AdmResult<()> {
    if !source.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, target_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;

    #[test]
    fn hardcoded_path_scan_finds_legacy_absolute_paths() {
        let root = temp_root("hardcoded");
        let source = root.join("src");
        fs::create_dir_all(&source).unwrap();
        fs::write(
            source.join("main.py"),
            r#"ROOT = "E:/workwork/CrewAi/newdemotower/工程运行文件""#,
        )
        .unwrap();
        fs::write(root.join("image.png"), b"legacy path bytes ignored").unwrap();

        let report = scan_hardcoded_paths(&root, false).unwrap();

        assert_eq!(report.hit_count, 1);
        assert_eq!(report.status, "failed");
        cleanup(root);
    }

    #[test]
    fn legacy_migration_writes_report_and_skips_build_artifacts() {
        let root = temp_root("legacy");
        let source = root.join("legacy_src");
        fs::create_dir_all(source.join("Assets")).unwrap();
        fs::create_dir_all(source.join("build")).unwrap();
        fs::write(source.join("Assets/demo.txt"), "demo").unwrap();
        fs::write(source.join("build/generated.txt"), "skip").unwrap();

        let dry =
            run_legacy_migration(&root, &source, Path::new("workspace/imported"), false).unwrap();
        assert_eq!(dry.file_count, 1);
        assert!(dry.dry_run);
        assert!(
            root.join("workspace/outputs/migration_report.json")
                .exists()
        );

        let applied =
            run_legacy_migration(&root, &source, Path::new("workspace/imported"), true).unwrap();
        assert!(!applied.dry_run);
        assert!(root.join("workspace/imported/Assets/demo.txt").exists());
        assert!(!root.join("workspace/imported/build/generated.txt").exists());
        cleanup(root);
    }

    #[test]
    fn eo_save_id_migration_dry_run_and_apply_backfills_store() {
        let root = temp_root("save_ids");
        let store = root.join("save_alpha/workspace/outputs/execution_objects");
        fs::create_dir_all(&store).unwrap();
        fs::write(store.join("execution_objects.json"), r#"{"objects":[]}"#).unwrap();

        let dry =
            migrate_execution_object_save_ids(&root, false, Some(root.join("report.md"))).unwrap();
        assert_eq!(dry.rows[0].status, "missing_save_id");
        assert_eq!(dry.rows[0].action, "would_update");

        let applied =
            migrate_execution_object_save_ids(&root, true, Option::<PathBuf>::None).unwrap();
        assert_eq!(applied.rows[0].action, "updated");
        let value = read_json(&store.join("execution_objects.json"), json!({}));
        assert_eq!(value["save_id"], "save_alpha");
        assert!(store.join("execution_objects.json.bak").exists());
        cleanup(root);
    }

    #[test]
    fn design_project_migration_creates_execution_object_store() {
        let root = temp_root("design_migrate");
        let projects = root.join("projects");
        fs::create_dir_all(&projects).unwrap();
        fs::write(
            projects.join("demo.json"),
            r#"{"projectName":"Demo","nodes":{"core":{"decisionState":"completed","checklist":{"loop":true}}}}"#,
        )
        .unwrap();
        let store_path = root.join("outputs/execution_objects/execution_objects.json");

        let dry = migrate_design_projects_to_execution_objects(
            &root,
            Option::<PathBuf>::None,
            &store_path,
            true,
            false,
            true,
        )
        .unwrap();
        assert_eq!(dry.items[0].status, "would_migrate");

        let applied = migrate_design_projects_to_execution_objects(
            &root,
            Option::<PathBuf>::None,
            &store_path,
            true,
            false,
            false,
        )
        .unwrap();
        assert_eq!(applied.migrated_count, 1);
        assert!(projects.join("demo.json.bak").exists());
        let store = read_json(&store_path, json!({}));
        assert_eq!(store["objects"][0]["object_type"], "design_project");
        cleanup(root);
    }

    #[test]
    fn inspect_reports_includes_validation_and_artifact_layer() {
        let root = temp_root("reports");
        let stage = root.join("stage_00");
        fs::create_dir_all(&stage).unwrap();
        fs::write(
            stage.join("validation_report.json"),
            r#"{"status":"failed"}"#,
        )
        .unwrap();
        fs::write(
            stage.join("artifact_validation_layer.json"),
            r#"{"status":"blocked"}"#,
        )
        .unwrap();

        let reports = inspect_pipeline_reports(&root, None, None).unwrap();

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0]["artifact_layer"]["status"], "blocked");
        cleanup(root);
    }

    #[test]
    fn schema_migration_wraps_legacy_frames_field() {
        let root = temp_root("schema_migrate");
        let input = root.join("input.json");
        let schema = root.join("schema.json");
        let output = root.join("output.md");
        fs::write(
            &input,
            r#"{"alignment_version":"2.0","unified_assets":[{"id":"hero","frames":["a","b"]}]}"#,
        )
        .unwrap();
        fs::write(
            &schema,
            r#"{"contract_version":"2.1","schema_migration":{"migration_rules":[{"from":"2.0","action":"wrap_to_object","field":"frames","new_field":"animation","structure":{"frames":"$old_frames","fps":12}}]}}"#,
        )
        .unwrap();

        let report = migrate_structured_schema(&input, &schema, &output).unwrap();
        let migrated = read_structured_or_text(&output).unwrap();

        assert_eq!(report.applied_rule_count, 1);
        assert_eq!(migrated["alignment_version"], "2.1");
        assert_eq!(migrated["unified_assets"][0]["animation"]["frames"][0], "a");
        assert!(migrated["unified_assets"][0].get("frames").is_none());
        cleanup(root);
    }

    #[test]
    fn design_concept_export_writes_package_and_optional_mirror() {
        let root = temp_root("concept_export");
        let state_path = root.join("state.json");
        fs::write(
            &state_path,
            r#"{"projectName":"Demo","nodes":{"core":{"decisionState":"completed","designNote":"Loop","checklist":{"loop":true}}}}"#,
        )
        .unwrap();

        let result = export_design_concept_package(
            &state_path,
            root.join("packages"),
            Some(root.join("mirror")),
        )
        .unwrap();

        assert!(
            result["packages"]["Concept"]
                .as_str()
                .unwrap()
                .contains("devflow_Concept_v2")
        );
        assert!(
            root.join("packages/devflow_Design_v2/structured/handoff_manifest.json")
                .exists()
        );
        assert!(
            root.join("mirror/devflow_Concept_v2/attachments/concept.md")
                .exists()
        );
        cleanup(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "adm_new_migration_{label}_{}",
            new_stable_id("root").unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
