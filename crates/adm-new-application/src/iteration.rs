use adm_new_contracts::project::ProjectState;
use adm_new_foundation::io::{now_iso, write_json_serializable};
use adm_new_foundation::paths::ProjectPaths;
use adm_new_foundation::{AdmError, AdmResult, sha256_hex};
use adm_new_save::SaveService;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const VALID_CHANGE_TYPES: &[&str] = &[
    "feature_addition",
    "sdk_integration",
    "bugfix",
    "refactor",
    "content_update",
];
pub const VALID_IMPACT_SCOPES: &[&str] = &["narrow", "medium", "wide"];
pub const STEP_IDS: &[&str] = &[
    "00", "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13", "14",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IterationChange {
    #[serde(rename = "type")]
    pub change_type: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub affects_systems: Vec<String>,
    #[serde(default)]
    pub feature_switch: String,
    #[serde(default)]
    pub sdk_dependency: String,
    #[serde(default)]
    pub raw: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IterationSpec {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub parent_version: String,
    #[serde(default)]
    pub change_type: String,
    #[serde(default)]
    pub impact_scope: String,
    #[serde(default)]
    pub changes: Vec<IterationChange>,
    #[serde(default)]
    pub removals: Vec<String>,
    #[serde(default)]
    pub sdk_dependencies: Vec<String>,
    #[serde(default)]
    pub explicit_exclusions: Vec<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub source_path: String,
    #[serde(default)]
    pub simplified: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

impl IterationSpec {
    pub fn valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn to_value(&self) -> AdmResult<Value> {
        let mut value = serde_json::to_value(self).map_err(|error| {
            AdmError::new(format!("failed to serialize iteration spec: {error}"))
        })?;
        if let Some(object) = value.as_object_mut() {
            object.insert("valid".to_string(), Value::Bool(self.valid()));
        }
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepDecision {
    pub action: String,
    pub reason: String,
}

impl StepDecision {
    pub fn new(action: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IterationDeltaPlan {
    pub schema_version: u32,
    pub status: String,
    #[serde(default)]
    pub iteration_label: String,
    #[serde(default)]
    pub parent_label: String,
    #[serde(default)]
    pub analysis_source: String,
    #[serde(default)]
    pub change_type: String,
    #[serde(default)]
    pub impact_scope: String,
    #[serde(default)]
    pub steps: BTreeMap<String, StepDecision>,
    #[serde(default)]
    pub estimated_steps_to_run: usize,
    #[serde(default)]
    pub estimated_steps_skipped: usize,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InheritedStage {
    pub step: String,
    pub action: String,
    pub reason: String,
    pub parent_version: String,
    pub parent_save_id: String,
    pub source_path: String,
    pub target_path: String,
    pub file_count: usize,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingInheritedStage {
    pub step: String,
    pub source_path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InheritanceReport {
    pub schema_version: u32,
    pub status: String,
    pub generated_at: String,
    pub parent_version: String,
    pub parent_save_id: String,
    pub inherited_stages: Vec<InheritedStage>,
    pub missing_stages: Vec<MissingInheritedStage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IterationPrepareRequest {
    pub project_root: PathBuf,
    pub session_id: String,
    pub spec_path: PathBuf,
    pub state: ProjectState,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IterationPrepareReport {
    pub status: String,
    pub dry_run: bool,
    pub parent_save_id: String,
    #[serde(default)]
    pub iteration_save_id: String,
    pub iteration_label: String,
    pub plan_path: String,
    #[serde(default)]
    pub copied_spec_path: String,
    pub rerun_steps: Vec<u8>,
    pub skip_steps: Vec<u8>,
    pub plan: IterationDeltaPlan,
    pub inheritance_report: Option<InheritanceReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IterationResumeSummary {
    pub status: String,
    pub plan_path: String,
    pub rerun_steps: Vec<u8>,
    pub skip_steps: Vec<u8>,
    pub from_step: Option<u8>,
    pub stop_step: Option<u8>,
}

pub fn parse_iteration_spec_text(text: &str, source_path: impl AsRef<Path>) -> IterationSpec {
    let sections = section_blocks(text);
    let mut changes = Vec::new();
    changes.extend(parse_feature_changes(&find_section(
        &sections,
        &["新增功能"],
    )));
    changes.extend(parse_modification_changes(&find_section(
        &sections,
        &["修改现有功能"],
    )));
    let sdk_dependencies = plain_list_items(&find_section(&sections, &["SDK", "外部依赖"]));
    let removals = plain_list_items(&find_section(&sections, &["移除内容"]));
    let explicit_exclusions = plain_list_items(&find_section(&sections, &["不变内容", "明确排除"]));
    let notes = plain_list_items(&find_section(&sections, &["备注"]));

    let mut spec = IterationSpec {
        title: extract_title(text),
        version: extract_field(text, &["版本", "version"]),
        parent_version: extract_field(text, &["基于", "parent", "parent_version"]),
        change_type: extract_field(text, &["类型", "change_type"]).to_lowercase(),
        impact_scope: extract_field(text, &["影响范围", "impact_scope"]).to_lowercase(),
        changes,
        removals,
        sdk_dependencies,
        explicit_exclusions,
        notes,
        source_path: source_path.as_ref().to_string_lossy().to_string(),
        simplified: false,
        warnings: Vec::new(),
        errors: Vec::new(),
    };
    spec.simplified = looks_simplified(text, &spec.changes);
    validate_spec(&mut spec);
    spec
}

pub fn parse_iteration_spec(path: impl AsRef<Path>) -> AdmResult<IterationSpec> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    Ok(parse_iteration_spec_text(text, path))
}

pub fn discover_iteration_specs(save_workspace: impl AsRef<Path>) -> AdmResult<Vec<IterationSpec>> {
    let spec_dir = save_workspace.as_ref().join("iteration_specs");
    if !spec_dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(&spec_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
        })
        .collect::<Vec<_>>();
    paths.sort();

    let mut specs = Vec::new();
    for path in paths {
        let mut spec = parse_iteration_spec(&path)?;
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if !filename_has_standard_iteration_version(filename) {
            spec.warnings
                .push("Iteration spec filename should start with v{major}.{minor}_.".to_string());
        }
        specs.push(spec);
    }
    Ok(specs)
}

pub fn build_delta_execution_plan(spec: &IterationSpec) -> IterationDeltaPlan {
    if !spec.valid() {
        return IterationDeltaPlan {
            schema_version: 1,
            status: "blocked".to_string(),
            iteration_label: spec.version.clone(),
            parent_label: spec.parent_version.clone(),
            analysis_source: spec.source_path.clone(),
            change_type: spec.change_type.clone(),
            impact_scope: spec.impact_scope.clone(),
            steps: BTreeMap::new(),
            estimated_steps_to_run: 0,
            estimated_steps_skipped: 0,
            warnings: spec.warnings.clone(),
            errors: spec.errors.clone(),
        };
    }

    let direct = direct_impacts(spec);
    let exclusions = explicit_exclusions(spec);
    let mut decisions = BTreeMap::<String, StepDecision>::new();
    for step in STEP_IDS {
        if let Some(reason) = direct.get(*step) {
            decisions.insert((*step).to_string(), StepDecision::new("rerun", reason));
        } else if let Some(reason) = exclusions.get(*step) {
            decisions.insert((*step).to_string(), StepDecision::new("skip", reason));
        } else {
            decisions.insert(
                (*step).to_string(),
                StepDecision::new("rerun", "uncertain_conservative_rerun"),
            );
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for (step, upstreams) in step_dependencies() {
            if decisions
                .get(step)
                .is_some_and(|item| item.action == "rerun")
            {
                continue;
            }
            if let Some(upstream) = upstreams.iter().find(|upstream| {
                decisions
                    .get(**upstream)
                    .is_some_and(|item| item.action == "rerun")
            }) {
                decisions.insert(
                    step.to_string(),
                    StepDecision::new("rerun", format!("upstream_rerun: step_{upstream}")),
                );
                changed = true;
            }
        }
    }

    let estimated_steps_to_run = decisions
        .values()
        .filter(|item| item.action == "rerun")
        .count();
    IterationDeltaPlan {
        schema_version: 1,
        status: "ready".to_string(),
        iteration_label: spec.version.clone(),
        parent_label: spec.parent_version.clone(),
        analysis_source: spec.source_path.clone(),
        change_type: spec.change_type.clone(),
        impact_scope: spec.impact_scope.clone(),
        steps: decisions,
        estimated_steps_to_run,
        estimated_steps_skipped: STEP_IDS.len() - estimated_steps_to_run,
        warnings: spec.warnings.clone(),
        errors: Vec::new(),
    }
}

pub fn build_delta_execution_plan_from_path(
    path: impl AsRef<Path>,
) -> AdmResult<IterationDeltaPlan> {
    Ok(build_delta_execution_plan(&parse_iteration_spec(path)?))
}

pub fn inherit_skipped_artifacts(
    parent_workspace: impl AsRef<Path>,
    target_workspace: impl AsRef<Path>,
    parent_version: &str,
    parent_save_id: &str,
    delta_plan: &IterationDeltaPlan,
) -> AdmResult<InheritanceReport> {
    let parent_workspace = parent_workspace.as_ref();
    let target_workspace = target_workspace.as_ref();
    let mut inherited_stages = Vec::new();
    let mut missing_stages = Vec::new();

    for (step, decision) in &delta_plan.steps {
        if decision.action != "skip" {
            continue;
        }
        let source = stage_dir(parent_workspace, step);
        let target = stage_dir(target_workspace, step);
        if !source.exists() {
            missing_stages.push(MissingInheritedStage {
                step: step.clone(),
                source_path: source.display().to_string(),
                reason: "parent_stage_artifact_missing".to_string(),
            });
            continue;
        }
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        copy_dir_recursive(&source, &target)?;
        let (content_hash, file_count) = hash_directory(&target)?;
        inherited_stages.push(InheritedStage {
            step: step.clone(),
            action: "skip".to_string(),
            reason: decision.reason.clone(),
            parent_version: parent_version.to_string(),
            parent_save_id: parent_save_id.to_string(),
            source_path: source.display().to_string(),
            target_path: target.display().to_string(),
            file_count,
            content_hash,
        });
    }

    let status = if missing_stages.is_empty() {
        "ready"
    } else {
        "completed_with_review"
    };
    let report = InheritanceReport {
        schema_version: 1,
        status: status.to_string(),
        generated_at: now_iso(),
        parent_version: parent_version.to_string(),
        parent_save_id: parent_save_id.to_string(),
        inherited_stages,
        missing_stages,
    };
    write_json_serializable(
        &target_workspace
            .join("outputs")
            .join("artifacts")
            .join("stage_inheritance.json"),
        &report,
    )?;
    Ok(report)
}

pub fn prepare_iteration(request: IterationPrepareRequest) -> AdmResult<IterationPrepareReport> {
    let mut spec = parse_iteration_spec(&request.spec_path)?;
    if !spec.valid() {
        return Err(AdmError::new(format!(
            "invalid iteration spec: {}",
            spec.errors.join("; ")
        )));
    }
    if spec.parent_version.trim().is_empty() {
        spec.parent_version = "current".to_string();
    }

    let service = SaveService::with_pid(&request.project_root, &request.session_id, 0)?;
    let parent_save_id = service
        .list_saves()?
        .current_save_id
        .ok_or_else(|| AdmError::new("No current save is bound."))?;
    let paths = ProjectPaths::new(&request.project_root, &request.session_id);
    paths.ensure_current_draft_dirs()?;
    let plan = build_delta_execution_plan(&spec);
    let plan_path = iteration_plan_output_path(&paths, request.dry_run);
    write_json_serializable(&plan_path, &plan)?;

    let rerun_steps = plan_steps_by_action(&plan, "rerun");
    let skip_steps = plan_steps_by_action(&plan, "skip");
    let iteration_label = iteration_label(&spec, &request.spec_path);
    if request.dry_run {
        return Ok(IterationPrepareReport {
            status: plan.status.clone(),
            dry_run: true,
            parent_save_id,
            iteration_save_id: String::new(),
            iteration_label,
            plan_path: plan_path.display().to_string(),
            copied_spec_path: String::new(),
            rerun_steps,
            skip_steps,
            plan,
            inheritance_report: None,
        });
    }

    let spec_file = request
        .spec_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AdmError::new("iteration spec path must have a file name"))?;
    let target_spec_rel = format!("iteration_specs/{spec_file}");
    let save_report = service.create_iteration_save(
        &format!("Iteration {iteration_label}"),
        &request.state,
        &spec.change_type,
        &spec.version,
        &target_spec_rel,
    )?;
    paths.ensure_current_draft_dirs()?;
    let target_spec_path = paths.iteration_specs_dir.join(spec_file);
    if !same_file_path(&request.spec_path, &target_spec_path) {
        fs::copy(&request.spec_path, &target_spec_path)?;
    }
    let final_plan_path = iteration_plan_output_path(&paths, false);
    write_json_serializable(&final_plan_path, &plan)?;
    let parent_workspace = request
        .project_root
        .join("saves")
        .join(&parent_save_id)
        .join("workspace");
    let inheritance_report = inherit_skipped_artifacts(
        parent_workspace,
        &paths.draft_dir,
        &spec.parent_version,
        &parent_save_id,
        &plan,
    )?;
    let status = if inheritance_report.status == "ready" {
        "prepared"
    } else {
        "prepared_with_review"
    };
    Ok(IterationPrepareReport {
        status: status.to_string(),
        dry_run: false,
        parent_save_id,
        iteration_save_id: save_report.manifest.save_id,
        iteration_label,
        plan_path: final_plan_path.display().to_string(),
        copied_spec_path: target_spec_path.display().to_string(),
        rerun_steps,
        skip_steps,
        plan,
        inheritance_report: Some(inheritance_report),
    })
}

pub fn summarize_iteration_resume_plan(
    plan_path: impl AsRef<Path>,
) -> AdmResult<IterationResumeSummary> {
    let plan_path = plan_path.as_ref();
    let text = fs::read_to_string(plan_path)?;
    let plan: IterationDeltaPlan = serde_json::from_str(&text)
        .map_err(|error| AdmError::new(format!("invalid iteration delta plan JSON: {error}")))?;
    let rerun_steps = plan_steps_by_action(&plan, "rerun");
    let skip_steps = plan_steps_by_action(&plan, "skip");
    let from_step = rerun_steps.iter().copied().min();
    let stop_step = rerun_steps.iter().copied().max();
    let status = if rerun_steps.is_empty() {
        "nothing_to_run"
    } else {
        "ready"
    };
    Ok(IterationResumeSummary {
        status: status.to_string(),
        plan_path: plan_path.display().to_string(),
        rerun_steps,
        skip_steps,
        from_step,
        stop_step,
    })
}

fn validate_spec(spec: &mut IterationSpec) {
    if !spec.version.is_empty() && !looks_like_version(&spec.version) {
        spec.warnings.push(format!(
            "Version history is disabled; ignoring nonstandard version label: {}",
            spec.version
        ));
    }
    if !spec.parent_version.is_empty()
        && !looks_like_version(&spec.parent_version)
        && spec.parent_version != "current"
    {
        spec.warnings.push(format!(
            "Version history is disabled; ignoring parent version label: {}",
            spec.parent_version
        ));
    }
    if !spec.change_type.is_empty() && !VALID_CHANGE_TYPES.contains(&spec.change_type.as_str()) {
        spec.errors
            .push(format!("Invalid change_type: {}", spec.change_type));
    }
    if !spec.impact_scope.is_empty() && !VALID_IMPACT_SCOPES.contains(&spec.impact_scope.as_str()) {
        spec.errors
            .push(format!("Invalid impact_scope: {}", spec.impact_scope));
    }
    if spec.changes.is_empty() && spec.removals.is_empty() && spec.sdk_dependencies.is_empty() {
        spec.warnings
            .push("No concrete change items parsed from iteration spec.".to_string());
    }
}

fn extract_title(text: &str) -> String {
    text.lines()
        .find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("# ")
                .map(|title| clean(title).to_string())
        })
        .unwrap_or_default()
}

fn extract_field(text: &str, labels: &[&str]) -> String {
    let wanted = labels
        .iter()
        .map(|label| normalize_label(label))
        .collect::<Vec<_>>();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with('-') || trimmed.starts_with('*') {
            continue;
        }
        if let Some((label, value)) = split_label_value(trimmed)
            && wanted.contains(&normalize_label(&label))
        {
            return clean(&value).trim_end().to_string();
        }
    }
    String::new()
}

fn section_blocks(text: &str) -> Vec<(String, Vec<String>)> {
    let mut sections = vec![("__root__".to_string(), Vec::new())];
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("## ") {
            sections.push((clean(title).to_string(), Vec::new()));
        } else if let Some((_, lines)) = sections.last_mut() {
            lines.push(line.trim_end().to_string());
        }
    }
    sections
}

fn find_section(sections: &[(String, Vec<String>)], keywords: &[&str]) -> Vec<String> {
    let normalized_keywords = keywords
        .iter()
        .map(|keyword| normalize_label(keyword))
        .collect::<Vec<_>>();
    sections
        .iter()
        .find(|(title, _)| {
            let normalized_title = normalize_label(title);
            normalized_keywords
                .iter()
                .any(|keyword| normalized_title.contains(keyword))
        })
        .map(|(_, lines)| lines.clone())
        .unwrap_or_default()
}

fn parse_feature_changes(lines: &[String]) -> Vec<IterationChange> {
    let mut changes = Vec::new();
    let mut current: Option<BTreeMap<String, Value>> = None;
    for line in lines {
        let text = clean(line);
        if should_skip_line(&text) || text.contains("本次迭代无") {
            continue;
        }
        if let Some(name) = bullet_bold_label_value(text, "功能名称") {
            if let Some(raw) = current.take() {
                changes.push(feature_from_raw(raw));
            }
            let mut raw = BTreeMap::new();
            raw.insert("name".to_string(), Value::String(name));
            current = Some(raw);
            continue;
        }
        if let (Some(raw), Some((key, value))) = (current.as_mut(), parse_detail_line(text)) {
            raw.insert(normalize_label(&key), Value::String(value));
        }
    }
    if let Some(raw) = current {
        changes.push(feature_from_raw(raw));
    }
    changes
}

fn feature_from_raw(raw: BTreeMap<String, Value>) -> IterationChange {
    IterationChange {
        change_type: "new_feature".to_string(),
        name: raw_string(&raw, "name"),
        description: first_raw_string(&raw, &["描述", "description"]),
        target: String::new(),
        affects_systems: split_csv(&first_raw_string(&raw, &["涉及系统", "systems"])),
        feature_switch: first_raw_string(&raw, &["开关", "feature_switch"]).replace('`', ""),
        sdk_dependency: first_raw_string(&raw, &["sdk依赖", "sdkdependency"]),
        raw,
    }
}

fn parse_modification_changes(lines: &[String]) -> Vec<IterationChange> {
    let mut changes = Vec::new();
    let mut current: Option<BTreeMap<String, Value>> = None;
    for line in lines {
        let text = clean(line);
        if should_skip_line(&text) || text.contains("本次迭代无") {
            continue;
        }
        if let Some(target) = bullet_bold_label_value(text, "目标") {
            if let Some(raw) = current.take() {
                changes.push(modification_from_raw(raw));
            }
            let mut raw = BTreeMap::new();
            raw.insert("target".to_string(), Value::String(target));
            current = Some(raw);
            continue;
        }
        if let (Some(raw), Some((key, value))) = (current.as_mut(), parse_detail_line(text)) {
            raw.insert(normalize_label(&key), Value::String(value));
        }
    }
    if let Some(raw) = current {
        changes.push(modification_from_raw(raw));
    }
    changes
}

fn modification_from_raw(raw: BTreeMap<String, Value>) -> IterationChange {
    IterationChange {
        change_type: "modification".to_string(),
        name: String::new(),
        description: first_raw_string(&raw, &["修改内容", "change", "description"]),
        target: raw_string(&raw, "target"),
        affects_systems: Vec::new(),
        feature_switch: String::new(),
        sdk_dependency: String::new(),
        raw,
    }
}

fn plain_list_items(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| {
            let text = clean(line);
            if should_skip_line(&text) {
                return None;
            }
            let item = text
                .strip_prefix("- ")
                .or_else(|| text.strip_prefix("* "))?;
            let value = clean(item).to_string();
            if is_none_marker(&value) || value.contains("本次迭代无") {
                None
            } else {
                Some(value)
            }
        })
        .collect()
}

fn parse_detail_line(text: &str) -> Option<(String, String)> {
    let value = text
        .strip_prefix("- ")
        .or_else(|| text.strip_prefix("* "))?;
    let (key, value) = split_label_value(value)?;
    Some((key.trim_matches('*').to_string(), value))
}

fn bullet_bold_label_value(text: &str, label: &str) -> Option<String> {
    let value = text
        .strip_prefix("- ")
        .or_else(|| text.strip_prefix("* "))?;
    let prefix = format!("**{label}**");
    let rest = value.strip_prefix(&prefix)?;
    let rest = rest
        .strip_prefix(':')
        .or_else(|| rest.strip_prefix('：'))
        .unwrap_or(rest);
    Some(clean(rest).to_string())
}

fn split_label_value(text: &str) -> Option<(String, String)> {
    let (index, separator) = text
        .char_indices()
        .find(|(_, ch)| matches!(ch, ':' | '：'))?;
    let value_start = index + separator.len_utf8();
    Some((
        clean(&text[..index]).to_string(),
        clean(&text[value_start..]).to_string(),
    ))
}

fn split_csv(value: &str) -> Vec<String> {
    let text = clean(value).trim_matches('`').to_string();
    if is_none_marker(&text) {
        return Vec::new();
    }
    text.split(['、', ',', '，', '/'])
        .map(|item| item.trim().trim_matches('`').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn looks_simplified(text: &str, changes: &[IterationChange]) -> bool {
    let has_standard_sections = text.contains("## 新增功能")
        && text.contains("## 修改现有功能")
        && text.contains("## 不变内容");
    !has_standard_sections || changes.is_empty()
}

fn looks_like_version(value: &str) -> bool {
    let text = clean(value);
    let text = text.strip_prefix('v').unwrap_or(&text);
    let mut parts = text.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !major.is_empty()
        && !minor.is_empty()
        && major.chars().all(|ch| ch.is_ascii_digit())
        && minor.chars().all(|ch| ch.is_ascii_digit())
}

fn filename_has_standard_iteration_version(filename: &str) -> bool {
    let Some(rest) = filename.strip_prefix('v') else {
        return false;
    };
    let Some((version, _)) = rest.split_once('_') else {
        return false;
    };
    looks_like_version(version)
}

fn direct_impacts(spec: &IterationSpec) -> BTreeMap<&'static str, String> {
    let mut direct = BTreeMap::new();
    if matches!(
        spec.change_type.as_str(),
        "feature_addition" | "bugfix" | "refactor"
    ) {
        for step in ["03", "08", "11"] {
            direct.insert(step, format!("{}: program_change", spec.change_type));
        }
    }
    if spec.change_type == "sdk_integration" || !spec.sdk_dependencies.is_empty() {
        for step in ["03", "08", "11"] {
            direct.insert(step, "sdk_integration".to_string());
        }
    }
    if spec.change_type == "content_update" {
        for step in ["04", "09", "10", "12"] {
            direct.insert(step, "content_update".to_string());
        }
    }

    for change in &spec.changes {
        let text = change_text(change);
        let label = first_non_empty(&[
            change.name.as_str(),
            change.target.as_str(),
            change.change_type.as_str(),
        ]);
        if matches!(change.change_type.as_str(), "new_feature" | "modification") {
            for step in ["03", "08", "11"] {
                direct
                    .entry(step)
                    .or_insert_with(|| format!("{}: {label}", change.change_type));
            }
        }
        if contains_any(&text, SDK_KEYWORDS) {
            for step in ["03", "08", "11"] {
                direct.insert(step, format!("sdk_related: {label}"));
            }
        }
        if contains_any(&text, ART_KEYWORDS) {
            for step in ["04", "06", "09", "10", "12"] {
                direct.insert(step, format!("asset_related: {label}"));
            }
        }
        if contains_any(&text, UI_SCENE_KEYWORDS) {
            for step in ["02", "08", "11", "13", "14"] {
                direct.insert(step, format!("ui_scene_related: {label}"));
            }
        }
        if contains_any(&text, RUNTIME_KEYWORDS) {
            for step in ["02", "03", "08", "11", "13", "14"] {
                direct.insert(step, format!("runtime_related: {label}"));
            }
        }
    }

    for removal in &spec.removals {
        if !removal.trim().is_empty() {
            direct
                .entry("08")
                .or_insert_with(|| format!("removal: {removal}"));
            direct
                .entry("11")
                .or_insert_with(|| format!("removal: {removal}"));
        }
    }
    direct
}

fn explicit_exclusions(spec: &IterationSpec) -> BTreeMap<&'static str, String> {
    let mut exclusions = BTreeMap::new();
    for text in &spec.explicit_exclusions {
        for (step, keywords) in exclusion_keywords() {
            if contains_any(text, keywords) {
                exclusions
                    .entry(step)
                    .or_insert_with(|| format!("explicit_exclusion: {text}"));
            }
        }
    }
    exclusions
}

fn step_dependencies() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("01", vec!["00"]),
        ("02", vec!["01"]),
        ("03", vec!["02"]),
        ("05", vec!["03"]),
        ("06", vec!["04"]),
        ("08", vec!["03", "05"]),
        ("09", vec!["04", "06", "07"]),
        ("10", vec!["09"]),
        ("11", vec!["08"]),
        ("12", vec!["09", "10"]),
        ("13", vec!["11", "12"]),
        ("14", vec!["13"]),
    ]
}

fn exclusion_keywords() -> Vec<(&'static str, &'static [&'static str])> {
    vec![
        ("00", &["项目基础", "项目愿景", "初始概念"]),
        ("01", &["核心玩法", "玩法循环", "系统架构"]),
        ("02", &["核心玩法", "玩法设计", "ui不变", "场景不变"]),
        ("03", &["程序需求", "代码", "逻辑", "sdk"]),
        ("04", &["美术", "美术资产", "资源", "音频"]),
        ("05", &["程序需求", "代码", "逻辑", "sdk"]),
        ("06", &["美术", "美术资产", "资源", "音频"]),
        ("07", &["美术", "美术资产", "美术风格", "风格"]),
        ("08", &["程序计划", "代码", "逻辑", "sdk"]),
        ("09", &["美术", "美术资产", "资源"]),
        ("10", &["美术", "资源", "挂载"]),
        ("11", &["代码", "逻辑", "sdk"]),
        ("12", &["美术", "美术资产", "资源", "音频"]),
        ("13", &["场景", "ui不变", "启动流程"]),
        ("14", &["验证", "集成"]),
    ]
}

const ART_KEYWORDS: &[&str] = &[
    "美术", "素材", "asset", "sprite", "icon", "tile", "audio", "音频",
];
const UI_SCENE_KEYWORDS: &[&str] = &[
    "ui", "界面", "弹窗", "hud", "scene", "场景", "启动", "挂载", "demo",
];
const SDK_KEYWORDS: &[&str] = &["sdk", "广告", "admob", "analytics", "支付"];
const RUNTIME_KEYWORDS: &[&str] = &["运行时", "目标", "胜负", "死亡", "玩家", "关卡", "数据"];

fn change_text(change: &IterationChange) -> String {
    [
        change.change_type.as_str(),
        change.name.as_str(),
        change.description.as_str(),
        change.target.as_str(),
        &change.affects_systems.join(" "),
        change.feature_switch.as_str(),
        change.sdk_dependency.as_str(),
    ]
    .join(" ")
}

fn contains_any(text: &str, keywords: &[&str]) -> bool {
    let lower = text.to_lowercase();
    keywords
        .iter()
        .any(|keyword| lower.contains(&keyword.to_lowercase()))
}

fn clean(value: &str) -> &str {
    value.trim().trim_start_matches('\u{feff}').trim()
}

fn normalize_label(label: &str) -> String {
    clean(label)
        .to_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

fn is_none_marker(value: &str) -> bool {
    matches!(
        clean(value).to_lowercase().as_str(),
        "" | "无" | "本次迭代无" | "（本次迭代无）" | "(本次迭代无)" | "none" | "n/a"
    )
}

fn should_skip_line(text: &str) -> bool {
    text.is_empty() || text.starts_with("<!--") || text.starts_with("---")
}

fn raw_string(raw: &BTreeMap<String, Value>, key: &str) -> String {
    raw.get(&normalize_label(key))
        .or_else(|| raw.get(key))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn first_raw_string(raw: &BTreeMap<String, Value>, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| {
            let value = raw_string(raw, key);
            (!value.is_empty()).then_some(value)
        })
        .unwrap_or_default()
}

fn first_non_empty(values: &[&str]) -> String {
    values
        .iter()
        .find(|value| !value.trim().is_empty())
        .copied()
        .unwrap_or("")
        .to_string()
}

fn stage_dir(root: &Path, step: &str) -> PathBuf {
    let label = step
        .trim()
        .parse::<u8>()
        .map(|number| format!("{number:02}"))
        .unwrap_or_else(|_| step.trim().to_lowercase());
    root.join("outputs")
        .join("artifacts")
        .join(format!("stage_{label}"))
}

fn hash_directory(path: &Path) -> AdmResult<(String, usize)> {
    let mut files = Vec::new();
    collect_files(path, &mut files)?;
    files.sort();
    let mut bytes = Vec::new();
    for file_path in &files {
        let relative = file_path
            .strip_prefix(path)
            .unwrap_or(file_path)
            .to_string_lossy()
            .replace('\\', "/");
        bytes.extend_from_slice(relative.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&fs::read(file_path)?);
    }
    Ok((sha256_hex(&bytes), files.len()))
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> AdmResult<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn copy_dir_recursive(source: &Path, target: &Path) -> AdmResult<()> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else if source_path.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn iteration_plan_output_path(paths: &ProjectPaths, preview: bool) -> PathBuf {
    paths.outputs_dir.join("artifacts").join(if preview {
        "delta_execution_plan_preview.json"
    } else {
        "delta_execution_plan.json"
    })
}

fn iteration_label(spec: &IterationSpec, spec_path: &Path) -> String {
    if !spec.title.trim().is_empty() {
        spec.title.clone()
    } else if !spec.version.trim().is_empty() {
        spec.version.clone()
    } else {
        spec_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("iteration")
            .to_string()
    }
}

fn plan_steps_by_action(plan: &IterationDeltaPlan, action: &str) -> Vec<u8> {
    plan.steps
        .iter()
        .filter_map(|(step, decision)| {
            (decision.action == action)
                .then(|| step.parse::<u8>().ok())
                .flatten()
        })
        .collect()
}

fn same_file_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::project::ProjectState;
    use adm_new_foundation::new_stable_id;

    const STANDARD_SPEC: &str = r#"# 迭代规格：激励广告复活

版本：v2.0
基于：v1.0
类型：feature_addition
影响范围：narrow

---

## 新增功能

- **功能名称**：激励广告复活
  - 描述：玩家死亡后弹出看广告复活提示，看完广告从当前位置复活一次，每关限一次
  - 涉及系统：死亡系统、关卡管理、UI层
  - 开关：`REWARDED_AD_REVIVE_ENABLED`，默认关闭
  - SDK依赖：（待定，用接口抽象）

---

## 修改现有功能

- **目标**：PlayerDeathHandler
  - 修改内容：死亡流程中插入广告复活判断入口

---

## 移除内容

（本次迭代无移除内容）

---

## SDK / 外部依赖

（本次迭代使用接口抽象，不绑定具体 SDK）

---

## 不变内容（明确排除）

- 核心玩法循环不变
- 美术资产不变
- 关卡设计不变
"#;

    #[test]
    fn iteration_spec_parser_matches_standard_markdown_contract() {
        let spec = parse_iteration_spec_text(STANDARD_SPEC, "iteration_specs/v2.0_rewarded_ads.md");

        assert!(spec.valid(), "{:?}", spec.errors);
        assert_eq!(spec.version, "v2.0");
        assert_eq!(spec.parent_version, "v1.0");
        assert_eq!(spec.change_type, "feature_addition");
        assert_eq!(spec.impact_scope, "narrow");
        assert_eq!(spec.changes[0].name, "激励广告复活");
        assert!(
            spec.changes[0]
                .affects_systems
                .contains(&"死亡系统".to_string())
        );
        assert_eq!(spec.changes[1].target, "PlayerDeathHandler");
        assert!(
            spec.explicit_exclusions
                .contains(&"美术资产不变".to_string())
        );
        assert!(spec.to_value().unwrap()["valid"].as_bool().unwrap());
    }

    #[test]
    fn delta_scheduler_reruns_program_validation_chain_and_skips_art() {
        let spec = parse_iteration_spec_text(STANDARD_SPEC, "");
        let plan = build_delta_execution_plan(&spec);

        assert_eq!(plan.status, "ready");
        assert_eq!(plan.steps["03"].action, "rerun");
        assert_eq!(plan.steps["08"].action, "rerun");
        assert_eq!(plan.steps["11"].action, "rerun");
        assert_eq!(plan.steps["13"].action, "rerun");
        assert_eq!(plan.steps["14"].action, "rerun");
        assert!(!plan.steps.contains_key("15"));
        assert_eq!(plan.steps["04"].action, "skip");
        assert_eq!(plan.steps["12"].action, "skip");
    }

    #[test]
    fn delta_scheduler_blocks_invalid_iteration_spec() {
        let spec = parse_iteration_spec_text("类型：unknown\n", "");
        let plan = build_delta_execution_plan(&spec);

        assert_eq!(plan.status, "blocked");
        assert!(!plan.errors.is_empty());
    }

    #[test]
    fn discover_iteration_specs_warns_on_nonstandard_filename() {
        let root = temp_root("iteration_discover");
        let spec_dir = root.join("iteration_specs");
        fs::create_dir_all(&spec_dir).unwrap();
        fs::write(spec_dir.join("rewarded_ads.md"), STANDARD_SPEC).unwrap();

        let specs = discover_iteration_specs(&root).unwrap();

        assert_eq!(specs.len(), 1);
        assert!(
            specs[0]
                .warnings
                .last()
                .unwrap()
                .starts_with("Iteration spec filename")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn artifact_inheritor_copies_skipped_stage_and_writes_sidecar() {
        let root = temp_root("iteration_inherit");
        let parent = root.join("parent");
        let target = root.join("target");
        let stage = parent.join("outputs/artifacts/stage_04");
        fs::create_dir_all(&stage).unwrap();
        fs::write(stage.join("asset_spec_contract.json"), r#"{"ok":true}"#).unwrap();
        let mut steps = BTreeMap::new();
        steps.insert(
            "04".to_string(),
            StepDecision::new("skip", "explicit_exclusion: 美术资产不变"),
        );
        steps.insert(
            "11".to_string(),
            StepDecision::new("rerun", "feature_addition"),
        );
        let plan = IterationDeltaPlan {
            schema_version: 1,
            status: "ready".to_string(),
            iteration_label: "v2.0".to_string(),
            parent_label: "v1.0".to_string(),
            analysis_source: String::new(),
            change_type: "feature_addition".to_string(),
            impact_scope: "narrow".to_string(),
            steps,
            estimated_steps_to_run: 1,
            estimated_steps_skipped: 1,
            warnings: Vec::new(),
            errors: Vec::new(),
        };

        let report = inherit_skipped_artifacts(&parent, &target, "v1.0", "save-1", &plan).unwrap();

        assert_eq!(report.status, "ready");
        assert!(
            target
                .join("outputs/artifacts/stage_04/asset_spec_contract.json")
                .exists()
        );
        assert_eq!(report.inherited_stages[0].step, "04");
        assert!(!report.inherited_stages[0].content_hash.is_empty());
        assert!(
            target
                .join("outputs/artifacts/stage_inheritance.json")
                .exists()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prepare_iteration_dry_run_keeps_current_save_and_writes_preview() {
        let root = temp_root("iteration_dry_run");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();
        let initial = service.create_blank_save("Initial").unwrap();
        let spec_path = root.join("v2.0_rewarded_ads.md");
        fs::write(&spec_path, STANDARD_SPEC).unwrap();

        let report = prepare_iteration(IterationPrepareRequest {
            project_root: root.clone(),
            session_id: "session_a".to_string(),
            spec_path,
            state: ProjectState::empty(),
            dry_run: true,
        })
        .unwrap();

        let index = service.list_saves().unwrap();
        assert_eq!(report.status, "ready");
        assert_eq!(
            index.current_save_id.as_deref(),
            Some(initial.manifest.save_id.as_str())
        );
        assert_eq!(index.saves.len(), 1);
        assert!(
            root.join("drafts/session_a/outputs/artifacts/delta_execution_plan_preview.json")
                .exists()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prepare_iteration_creates_iteration_save_and_inherits_skipped_artifacts() {
        let root = temp_root("iteration_prepare");
        let service = SaveService::with_pid(&root, "session_a", 100).unwrap();
        let initial = service.create_blank_save("Initial").unwrap();
        let parent_stage = root
            .join("saves")
            .join(&initial.manifest.save_id)
            .join("workspace/outputs/artifacts/stage_04");
        fs::create_dir_all(&parent_stage).unwrap();
        fs::write(
            parent_stage.join("asset_spec_contract.json"),
            r#"{"ok":true}"#,
        )
        .unwrap();
        let spec_path = root.join("v2.0_rewarded_ads.md");
        fs::write(&spec_path, STANDARD_SPEC).unwrap();

        let report = prepare_iteration(IterationPrepareRequest {
            project_root: root.clone(),
            session_id: "session_a".to_string(),
            spec_path,
            state: ProjectState::empty(),
            dry_run: false,
        })
        .unwrap();

        let index = service.list_saves().unwrap();
        assert_eq!(report.status, "prepared_with_review");
        assert_ne!(report.iteration_save_id, initial.manifest.save_id);
        assert_eq!(
            index.current_save_id.as_deref(),
            Some(report.iteration_save_id.as_str())
        );
        assert!(
            root.join("drafts/session_a/iteration_specs/v2.0_rewarded_ads.md")
                .exists()
        );
        assert!(
            root.join("drafts/session_a/outputs/artifacts/delta_execution_plan.json")
                .exists()
        );
        assert!(
            root.join("drafts/session_a/outputs/artifacts/stage_04/asset_spec_contract.json")
                .exists()
        );
        assert!(report.skip_steps.contains(&4));
        assert!(report.rerun_steps.contains(&11));
        let summary = summarize_iteration_resume_plan(
            root.join("drafts/session_a/outputs/artifacts/delta_execution_plan.json"),
        )
        .unwrap();
        assert_eq!(summary.status, "ready");
        assert_eq!(summary.from_step, Some(0));
        assert_eq!(summary.stop_step, Some(14));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
        fs::create_dir_all(&root).unwrap();
        root
    }
}
