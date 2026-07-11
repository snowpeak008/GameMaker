use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use adm_new_contracts::ArtifactLocale;
use adm_new_foundation::io::{now_iso, read_json, rel, write_json};
use adm_new_foundation::{AdmError, AdmResult, file_manifest, sha256_hex};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::source::SourceService;

pub const ALLOWED_DESIGN_SOURCE_SUFFIXES: &[&str] = &[".md", ".txt"];
pub const ART_STYLE_GENERATION_STAGE: u32 = 7;
pub const ART_STYLE_CONFIRMATION_STAGE: u32 = ART_STYLE_GENERATION_STAGE;
pub const LEGACY_ART_STYLE_CONFIRMATION_STAGE: u32 = 8;
pub const PROGRAM_PLAN_STAGE: u32 = 8;
pub const ART_PLAN_STAGE: u32 = 9;
pub const ASSET_ALIGNMENT_STAGE: u32 = 10;
pub const DEV_EXECUTION_STAGE: u32 = 11;
pub const DEV_EXECUTION_STAGE_LABEL: &str = "Step 11";
pub const DEV_EXECUTION_STAGE_NAME: &str = "Development Execution";
pub const DEV_EXECUTION_TASK_UNIT_TYPE: &str = "stage11_task";
pub const DEV_EXECUTION_RESUME_DIR_NAME: &str = "stage_11_resume_records";
pub const LEGACY_DEV_EXECUTION_RESUME_DIR_NAMES: &[&str] = &["stage_12_resume_records"];
pub const ART_PRODUCTION_STAGE: u32 = 12;
pub const SCENE_ASSEMBLY_STAGE: u32 = 13;
pub const INTEGRATION_STAGE: u32 = 14;
pub const STAGE2_REQUIRED_PLAYABLE_CONTRACTS: &[&str] = &[
    "core_playable_contract",
    "demo_flow_contract",
    "runtime_data_contract",
    "ui_flow_contract",
    "scene_bootstrap_contract",
    "asset_mount_contract",
    "audio_requirements_contract",
    "playable_acceptance_contract",
];
pub const STRUCTURED_EARLY_INPUTS: &[&str] = &["profile", "archetype_requirements", "decisions"];
pub const STRUCTURED_OPTIONAL_INPUTS: &[&str] = &["design_entities"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxonomyEntry {
    pub question_ref: &'static str,
    pub domain: &'static str,
    pub decision: &'static str,
    pub question: &'static str,
    pub item_type: &'static str,
}

pub fn taxonomy() -> Vec<TaxonomyEntry> {
    vec![
        TaxonomyEntry {
            question_ref: "project.position",
            domain: "project_vision",
            decision: "position",
            question: "What product position is this project targeting?",
            item_type: "project_position",
        },
        TaxonomyEntry {
            question_ref: "project.platform",
            domain: "project_vision",
            decision: "platform",
            question: "Which platform is targeted first?",
            item_type: "platform",
        },
        TaxonomyEntry {
            question_ref: "core.loop",
            domain: "core_experience",
            decision: "core_loop",
            question: "What loop gives the player the main experience?",
            item_type: "core_loop",
        },
        TaxonomyEntry {
            question_ref: "systems.top_level",
            domain: "system_graph",
            decision: "top_level_systems",
            question: "Which top-level systems are selected?",
            item_type: "system_layer",
        },
        TaxonomyEntry {
            question_ref: "resources.types",
            domain: "resource_graph",
            decision: "core_resource_types",
            question: "Which resources or asset types does this project need?",
            item_type: "resource",
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub index: usize,
    pub layer_number: u32,
    pub layer_title: String,
    pub layer_status: String,
    pub item_type: String,
    pub option: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub unlocks: Vec<String>,
    #[serde(default)]
    pub source_ref: String,
    #[serde(default)]
    pub source_line: u32,
}

impl Selection {
    pub fn label(&self) -> String {
        if self.item_type.is_empty() {
            self.option.clone()
        } else {
            format!("{}: {}", self.item_type, self.option)
        }
    }

    pub fn id(&self) -> String {
        format!("SEL-{:03}", self.index)
    }

    pub fn as_python_dict(&self) -> Value {
        json!({
            "id": self.id(),
            "layer_number": self.layer_number,
            "layer_title": self.layer_title,
            "layer_status": self.layer_status,
            "item_type": self.item_type,
            "option": self.option,
            "label": self.label(),
            "purpose": self.purpose,
            "dependencies": self.dependencies,
            "unlocks": self.unlocks,
            "source": self.source_ref,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignLayer {
    pub number: u32,
    pub title: String,
    #[serde(default)]
    pub status: String,
    pub source_ref: String,
    #[serde(default)]
    pub selections: Vec<Selection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedDesignSource {
    pub source: String,
    #[serde(default)]
    pub source_path: String,
    pub source_sha256: String,
    pub source_size_bytes: u64,
    pub source_line_count: usize,
    pub parsed_at: String,
    pub layers: Vec<DesignLayer>,
    pub selections: Vec<Selection>,
    pub raw_text: String,
    #[serde(default)]
    pub source_package: String,
    #[serde(default)]
    pub source_input_type: String,
    #[serde(default)]
    pub design_summary: Value,
    #[serde(default)]
    pub structured_source_warning: Option<Value>,
}

impl ParsedDesignSource {
    pub fn selection_dicts(&self) -> Vec<Value> {
        self.selections
            .iter()
            .map(Selection::as_python_dict)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesignSourceError {
    message: String,
    details: Value,
}

impl DesignSourceError {
    pub fn new(message: impl Into<String>, details: Value) -> Self {
        Self {
            message: message.into(),
            details,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn details(&self) -> &Value {
        &self.details
    }
}

impl std::fmt::Display for DesignSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DesignSourceError {}

pub fn split_values(value: &str) -> Vec<String> {
    let raw = value.trim();
    if raw.is_empty() || raw == "无" {
        return Vec::new();
    }
    raw.split(['、', ',', '，', ';', '；'])
        .map(str::trim)
        .filter(|item| !item.is_empty() && *item != "无")
        .map(str::to_string)
        .collect()
}

pub fn source_ref(source: &str, line_no: usize) -> String {
    format!("{source}:{line_no}")
}

pub fn parse_design_text(
    text: &str,
    source: &str,
    source_path: &str,
    source_sha256: Option<String>,
    source_size_bytes: Option<u64>,
) -> ParsedDesignSource {
    let mut selections = Vec::<Selection>::new();
    let mut layers = Vec::<DesignLayer>::new();
    let mut current_layer = DesignLayer {
        number: 0,
        title: "unlayered".to_string(),
        status: String::new(),
        source_ref: source_ref(source, 1),
        selections: Vec::new(),
    };
    let mut current_selection_index: Option<usize> = None;

    for (line_index, line) in text.lines().enumerate() {
        let line_no = line_index + 1;
        if let Some((number, title)) = parse_layer_heading(line) {
            current_layer = DesignLayer {
                number,
                title,
                status: String::new(),
                source_ref: source_ref(source, line_no),
                selections: Vec::new(),
            };
            layers.push(current_layer.clone());
            current_selection_index = None;
            continue;
        }

        if line.starts_with("## ") {
            current_layer = DesignLayer {
                number: 0,
                title: "unlayered".to_string(),
                status: String::new(),
                source_ref: source_ref(source, line_no),
                selections: Vec::new(),
            };
            current_selection_index = None;
            continue;
        }

        if current_layer.number != 0
            && line.contains('/')
            && line == line.trim()
            && !line.starts_with("- ")
        {
            current_layer.status = line.trim().to_string();
            if let Some(layer) = layers.last_mut() {
                layer.status = current_layer.status.clone();
            }
            continue;
        }

        let trimmed_start = line.trim_start();
        if current_layer.number != 0 && trimmed_start.starts_with("- ") {
            let body = trimmed_start.trim_start_matches("- ").trim();
            if let Some((item_type, option)) = split_item_type(body) {
                let selection = Selection {
                    index: selections.len() + 1,
                    layer_number: current_layer.number,
                    layer_title: current_layer.title.clone(),
                    layer_status: current_layer.status.clone(),
                    item_type: canonical_design_item_type(&item_type),
                    option,
                    purpose: String::new(),
                    dependencies: Vec::new(),
                    unlocks: Vec::new(),
                    source_ref: source_ref(source, line_no),
                    source_line: line_no as u32,
                };
                selections.push(selection);
                current_selection_index = Some(selections.len() - 1);
                continue;
            }
            if let Some(default_type) = default_item_type_for_layer(&current_layer.title) {
                let selection = Selection {
                    index: selections.len() + 1,
                    layer_number: current_layer.number,
                    layer_title: current_layer.title.clone(),
                    layer_status: current_layer.status.clone(),
                    item_type: default_type.to_string(),
                    option: body.to_string(),
                    purpose: String::new(),
                    dependencies: Vec::new(),
                    unlocks: Vec::new(),
                    source_ref: source_ref(source, line_no),
                    source_line: line_no as u32,
                };
                selections.push(selection);
                current_selection_index = Some(selections.len() - 1);
                continue;
            }
        }

        let Some(selection_index) = current_selection_index else {
            continue;
        };
        let stripped = line.trim();
        if let Some(value) = metadata_value(stripped, &["目的", "purpose"]) {
            selections[selection_index].purpose = value.to_string();
        } else if let Some(value) = metadata_value(stripped, &["依赖", "depends"]) {
            selections[selection_index].dependencies = split_values(value);
        } else if let Some(value) = metadata_value(stripped, &["解锁", "unlocks"]) {
            selections[selection_index].unlocks = split_values(value);
        }
    }

    let mut grouped = BTreeMap::<u32, Vec<Selection>>::new();
    for selection in &selections {
        grouped
            .entry(selection.layer_number)
            .or_default()
            .push(selection.clone());
    }
    for layer in &mut layers {
        layer.selections = grouped.remove(&layer.number).unwrap_or_default();
    }

    ParsedDesignSource {
        source: source.to_string(),
        source_path: source_path.to_string(),
        source_sha256: source_sha256.unwrap_or_else(|| sha256_hex(text.as_bytes())),
        source_size_bytes: source_size_bytes.unwrap_or_else(|| text.len() as u64),
        source_line_count: text.lines().count(),
        parsed_at: now_iso(),
        layers,
        selections,
        raw_text: text.to_string(),
        source_package: String::new(),
        source_input_type: String::new(),
        design_summary: json!({}),
        structured_source_warning: None,
    }
}

fn canonical_design_item_type(value: &str) -> String {
    let compact = value
        .chars()
        .filter(|character| !character.is_whitespace() && !matches!(character, '_' | '-' | '－'))
        .flat_map(char::to_lowercase)
        .collect::<String>();
    match compact.as_str() {
        "l5entity" | "l5实体" => "L5实体".to_string(),
        "l5node" | "l5节点" => "L5节点".to_string(),
        _ => value.trim().to_string(),
    }
}

pub(crate) fn is_l5_entity_item_type(value: &str) -> bool {
    canonical_design_item_type(value) == "L5实体"
}

pub(crate) fn is_l5_node_item_type(value: &str) -> bool {
    canonical_design_item_type(value) == "L5节点"
}

fn metadata_value<'a>(line: &'a str, aliases: &[&str]) -> Option<&'a str> {
    let (index, delimiter) = line
        .char_indices()
        .find(|(_, character)| matches!(character, ':' | '：'))?;
    let key = line[..index].trim();
    aliases
        .iter()
        .any(|alias| key.eq_ignore_ascii_case(alias))
        .then(|| line[index + delimiter.len_utf8()..].trim())
}

pub fn parse_design_doc(project_root: &Path, path: &Path) -> AdmResult<ParsedDesignSource> {
    let raw = fs::read_to_string(path)?;
    let text = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let bytes = fs::read(path)?;
    Ok(parse_design_text(
        text,
        &rel(path, project_root),
        &path.to_string_lossy(),
        Some(sha256_hex(&bytes)),
        Some(bytes.len() as u64),
    ))
}

pub fn manual_notes_as_design(notes: &str, source: &str) -> ParsedDesignSource {
    let parsed = parse_design_text(notes, source, "", None, None);
    if !parsed.selections.is_empty() {
        return parsed;
    }
    let cleaned = notes
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let title = if cleaned.is_empty() {
        "operator submitted gameplay idea".to_string()
    } else {
        cleaned.chars().take(80).collect()
    };
    let wrapped = format!(
        "# Operator Design Input\n\n## Layer 1 Initial Idea\nManual Input / Submitted\n- Gameplay Idea: {title}\n  Purpose: {}\n",
        if cleaned.is_empty() {
            "Operator submitted gameplay idea."
        } else {
            &cleaned
        }
    );
    parse_design_text(&wrapped, source, "", None, None)
}

#[derive(Debug, Clone)]
pub struct SourceContext {
    source: SourceService,
}

impl SourceContext {
    pub fn new(root: impl AsRef<Path>, session_id: &str) -> AdmResult<Self> {
        Ok(Self {
            source: SourceService::new(root, session_id)?,
        })
    }

    pub fn from_source_service(source: SourceService) -> Self {
        Self { source }
    }

    pub fn source_service(&self) -> &SourceService {
        &self.source
    }

    pub fn iter_source_packages(&self) -> Vec<PathBuf> {
        let mut packages = Vec::new();
        for source_dir in self.source.source_artifact_roots() {
            let Ok(entries) = fs::read_dir(source_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join("package_manifest.json").exists() {
                    packages.push(path);
                }
            }
        }
        packages
    }

    pub fn package_manifest(&self, package_dir: &Path) -> Value {
        let manifest = read_json(&package_dir.join("package_manifest.json"), json!({}));
        if manifest.is_object() {
            manifest
        } else {
            json!({})
        }
    }

    pub fn package_matches_type(&self, package_dir: &Path, package_type: &str) -> bool {
        let manifest = self.package_manifest(package_dir);
        let source_ids = manifest
            .get("source_ids")
            .and_then(Value::as_array)
            .into_iter()
            .flat_map(|items| items.iter())
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        let declared_type = manifest
            .get("package_type")
            .or_else(|| manifest.get("source_id"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let package_type_id = manifest
            .get("package_type_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        source_ids.contains(package_type)
            || declared_type == package_type
            || package_type_id == package_type.to_ascii_lowercase()
    }

    pub fn latest_source_package(&self, package_type: &str) -> Option<PathBuf> {
        let mut candidates = self
            .iter_source_packages()
            .into_iter()
            .filter(|path| self.package_matches_type(path, package_type))
            .collect::<Vec<_>>();
        sort_packages_newest_first(&mut candidates);
        candidates.into_iter().next()
    }

    pub fn latest_structured_handoff_package(&self) -> Option<PathBuf> {
        self.latest_source_package("Design").filter(|path| {
            path.join("structured")
                .join("handoff_manifest.json")
                .exists()
        })
    }
}

pub fn read_structured(path: &Path) -> AdmResult<Value> {
    if !path.exists() {
        return Err(AdmError::new(format!(
            "structured handoff file not found: {}",
            path.display()
        )));
    }
    let value = read_json(path, json!({}));
    Ok(if value.is_object() { value } else { json!({}) })
}

pub fn write_structured_json(path: &Path, data: &Value) -> AdmResult<PathBuf> {
    write_json(path, data)
}

pub fn find_design_handoff(
    context: &SourceContext,
    design_doc_path: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(path) = design_doc_path {
        let base = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().unwrap_or(path).to_path_buf()
        };
        let candidate = base.join("design_handoff.json");
        if candidate.exists() {
            return Some(candidate);
        }
    }
    context
        .latest_source_package("Design")
        .map(|package| package.join("design_handoff.json"))
        .filter(|path| path.exists())
}

pub fn load_design_handoff(
    context: &SourceContext,
    design_doc_path: Option<&Path>,
) -> AdmResult<(Option<PathBuf>, Value)> {
    let Some(path) = find_design_handoff(context, design_doc_path) else {
        return Ok((None, json!({})));
    };
    let value = read_structured(&path)?;
    Ok((Some(path), value))
}

pub fn load_design_sources(context: &SourceContext, design_doc_path: &Path) -> AdmResult<Value> {
    let (handoff_path, handoff) = load_design_handoff(context, Some(design_doc_path))?;
    let markdown = if design_doc_path.is_file() {
        fs::read_to_string(design_doc_path)?
    } else {
        String::new()
    };
    Ok(json!({
        "handoff_path": handoff_path.map(|path| path.to_string_lossy().replace('\\', "/")).unwrap_or_default(),
        "handoff": handoff,
        "markdown_path": design_doc_path.to_string_lossy().replace('\\', "/"),
        "markdown": markdown,
    }))
}

#[derive(Debug, Clone)]
pub struct GenerationService {
    source: SourceService,
    artifact_locale: ArtifactLocale,
}

impl GenerationService {
    pub fn new(root: impl AsRef<Path>, session_id: &str) -> AdmResult<Self> {
        Ok(Self {
            source: SourceService::new(root, session_id)?,
            artifact_locale: ArtifactLocale::default(),
        })
    }

    pub fn with_artifact_locale(mut self, artifact_locale: ArtifactLocale) -> Self {
        self.artifact_locale = artifact_locale;
        self
    }

    pub fn artifact_locale(&self) -> ArtifactLocale {
        self.artifact_locale
    }

    pub fn source(&self) -> &SourceService {
        &self.source
    }

    pub fn source_context(&self) -> SourceContext {
        SourceContext::from_source_service(self.source.clone())
    }

    pub fn stage_dir(&self, step_number: u32) -> PathBuf {
        self.source.stage_dir(step_number)
    }

    pub fn output_base_from_stage_dir(&self, out_dir: &Path) -> PathBuf {
        output_base_from_stage_dir(out_dir)
    }

    pub fn structured_handoff_input_bundle(&self, out_dir: &Path, required_by_step: &str) -> Value {
        structured_handoff_input_bundle_with_locale(
            &self.source_context(),
            out_dir,
            required_by_step,
            self.artifact_locale,
        )
    }

    pub fn latest_concept_package(&self) -> Option<PathBuf> {
        latest_concept_package(&self.source_context())
    }

    pub fn load_current_design(&self) -> Result<ParsedDesignSource, DesignSourceError> {
        load_current_design(&self.source_context())
    }

    pub fn load_design_for_stage(
        &self,
        step_number: u32,
    ) -> Result<ParsedDesignSource, DesignSourceError> {
        load_design_for_stage(&self.source_context(), step_number)
    }

    pub fn update_stage_report(&self, step_number: u32, result: &Value) -> AdmResult<Value> {
        let out_dir = self.stage_dir(step_number);
        update_stage_report_with_locale(step_number, &out_dir, result, self.artifact_locale)
    }

    pub fn refresh_indexes(&self, step_number: u32) -> AdmResult<()> {
        let out_dir = self.stage_dir(step_number);
        refresh_indexes_with_locale(&self.source, step_number, &out_dir, self.artifact_locale)
    }

    pub fn apply_development_plan_outputs<G: StageOutputGenerator + ?Sized>(
        &self,
        step_number: u32,
        generator: &G,
    ) -> AdmResult<Value> {
        let out_dir = self.stage_dir(step_number);
        fs::create_dir_all(&out_dir)?;
        let parsed = match self.load_design_for_stage(step_number) {
            Ok(parsed) => parsed,
            Err(error) => {
                let source_error =
                    localized_design_source_error(step_number, &error, self.artifact_locale);
                let public_message = source_error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| {
                        localized_text(
                            self.artifact_locale,
                            "无法读取本步骤所需的设计输入。",
                            "The design input required by this stage could not be loaded.",
                        )
                    });
                let result = with_artifact_locale(
                    json!({
                        "content_exists": false,
                        "message": public_message,
                        "source_error": source_error,
                        "blocking_issues": 1,
                        "ai_review_status": "blocked",
                        "traceability_valid": false,
                    }),
                    self.artifact_locale,
                );
                write_json(&out_dir.join("design_source_error.json"), &result)?;
                let updated = update_stage_report_with_locale(
                    step_number,
                    &out_dir,
                    &result,
                    self.artifact_locale,
                )?;
                refresh_indexes_with_locale(
                    &self.source,
                    step_number,
                    &out_dir,
                    self.artifact_locale,
                )?;
                return Err(AdmError::new(format!(
                    "{}: {updated}",
                    localized_text(
                        self.artifact_locale,
                        "流水线步骤的设计输入校验失败",
                        "Pipeline stage design input validation failed",
                    )
                )));
            }
        };
        let structured_inputs = structured_handoff_input_bundle_with_locale(
            &self.source_context(),
            &out_dir,
            &format!("Step{step_number:02}"),
            self.artifact_locale,
        );
        let result = generator.generate(step_number, &parsed, &out_dir, &structured_inputs)?;
        let result = with_artifact_locale(result, self.artifact_locale);
        let updated =
            update_stage_report_with_locale(step_number, &out_dir, &result, self.artifact_locale)?;
        refresh_indexes_with_locale(&self.source, step_number, &out_dir, self.artifact_locale)?;
        if updated.get("valid").and_then(Value::as_bool) == Some(false)
            && updated.get("status").and_then(Value::as_str) != Some("blocked")
        {
            return Err(AdmError::new(format!(
                "{}: {}",
                if self.artifact_locale == ArtifactLocale::ZhCn {
                    format!("步骤 {step_number:02} 的产物校验失败")
                } else {
                    format!("Development outputs failed validation for Step{step_number:02}")
                },
                updated
                    .get("business_quality")
                    .cloned()
                    .unwrap_or(Value::Null)
            )));
        }
        Ok(updated)
    }
}

fn localized_design_source_error(
    step_number: u32,
    error: &DesignSourceError,
    artifact_locale: ArtifactLocale,
) -> Value {
    let mut details = object_map_or_empty(error.details().clone());
    let code = details
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("DESIGN_SOURCE_UNAVAILABLE");
    let message = if artifact_locale == ArtifactLocale::ZhCn {
        match code {
            "STAGE00_CONCEPT_SOURCE_MISSING" => "步骤00未找到已提交的概念源包。".to_string(),
            "STAGE00_ATTACHMENT_INVALID" => "步骤00包含不受支持或已经缺失的附件。".to_string(),
            "STAGE00_MULTIPLE_PRIMARY_CANDIDATES" => {
                "步骤00存在多个主设计文档候选，但未明确指定主来源。".to_string()
            }
            "STAGE00_ATTACHMENT_READ_FAILED" => "步骤00无法读取设计附件。".to_string(),
            "STAGE00_ATTACHMENT_UNPARSEABLE" => {
                "步骤00的设计附件不符合可解析的分层协议。".to_string()
            }
            "STAGE00_INPUT_MISSING" => "步骤00没有操作说明或有效设计文档附件。".to_string(),
            _ => format!("步骤 {step_number:02} 无法读取所需设计输入（{code}）。"),
        }
    } else {
        error.message().to_string()
    };
    details.insert("artifact_locale".to_string(), json!(artifact_locale));
    details.insert("message".to_string(), json!(message));
    if artifact_locale == ArtifactLocale::ZhCn && details.contains_key("fix") {
        details.insert(
            "fix".to_string(),
            json!("请检查本步骤要求的源包、主设计附件与结构化交接输入。"),
        );
    }
    Value::Object(details)
}

pub fn artifact_locale_from_inputs(structured_inputs: &Value) -> ArtifactLocale {
    ArtifactLocale::normalize(
        structured_inputs
            .get("artifact_locale")
            .and_then(Value::as_str),
    )
}

pub fn localized_text<'a>(locale: ArtifactLocale, zh_cn: &'a str, en_us: &'a str) -> &'a str {
    match locale {
        ArtifactLocale::ZhCn => zh_cn,
        ArtifactLocale::EnUs => en_us,
    }
}

fn with_artifact_locale(mut result: Value, artifact_locale: ArtifactLocale) -> Value {
    if let Some(object) = result.as_object_mut() {
        object.insert("artifact_locale".to_string(), json!(artifact_locale));
    }
    result
}

pub trait StageOutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value>;
}

#[derive(Debug, Clone, Default)]
pub struct ContractStageOutputGenerator;

impl StageOutputGenerator for ContractStageOutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        let artifact_locale = artifact_locale_from_inputs(structured_inputs);
        let source_manifest = json!({
            "schema_version": 1,
            "artifact_locale": artifact_locale,
            "generated_at": now_iso(),
            "sources": [{
                "path": parsed.source,
                "sha256": parsed.source_sha256,
                "size_bytes": parsed.source_size_bytes,
                "line_count": parsed.source_line_count,
                "role": "current_project_design_doc",
                "source_package": parsed.source_package,
                "source_input_type": parsed.source_input_type,
            }],
        });
        let extraction = json!({
            "schema_version": 1,
            "artifact_locale": artifact_locale,
            "generated_at": now_iso(),
            "source": parsed.source,
            "source_package": parsed.source_package,
            "source_input_type": parsed.source_input_type,
            "layers": parsed.layers,
            "selections": parsed.selection_dicts(),
        });
        let contract = json!({
            "schema_version": "1.0",
            "artifact_locale": artifact_locale,
            "generated_at": now_iso(),
            "stage": step_number,
            "source": parsed.source,
            "selection_count": parsed.selections.len(),
            "layer_count": parsed.layers.len(),
            "structured_input_status": structured_inputs.get("status").cloned().unwrap_or(Value::Null),
            "allowed_attachment_extensions": ALLOWED_DESIGN_SOURCE_SUFFIXES,
            "taxonomy": taxonomy(),
            "rule": localized_text(
                artifact_locale,
                "A17 仅写入生成合约；具体的 Step00–Step14 业务产物由后续阶段原子负责。",
                "A17 writes generation contracts only; concrete Step00-Step14 business outputs are owned by later stage atoms."
            ),
        });
        write_json(
            &out_dir.join("design_source_manifest.json"),
            &source_manifest,
        )?;
        write_json(&out_dir.join("design_extraction.json"), &extraction)?;
        write_json(&out_dir.join("generation_contract.json"), &contract)?;
        Ok(json!({
            "content_exists": true,
            "selection_count": parsed.selections.len(),
            "layer_count": parsed.layers.len(),
            "structured_input_status": structured_inputs.get("status").and_then(Value::as_str).unwrap_or("fallback_to_markdown"),
            "status": if structured_inputs.get("status").and_then(Value::as_str) == Some("structured") {
                "success"
            } else {
                "completed_with_review"
            },
            "review_items_count": structured_inputs.get("warnings").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
            "traceability_valid": true,
            "stage_generation_contract": "generation_contract.json",
        }))
    }
}

pub fn output_base_from_stage_dir(out_dir: &Path) -> PathBuf {
    if out_dir
        .parent()
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        == Some("artifacts")
        && out_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            == Some("outputs")
    {
        out_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap_or_else(|| out_dir.parent().unwrap_or(out_dir))
            .to_path_buf()
    } else {
        out_dir.parent().unwrap_or(out_dir).to_path_buf()
    }
}

pub fn structured_handoff_input_bundle(
    context: &SourceContext,
    out_dir: &Path,
    required_by_step: &str,
) -> Value {
    structured_handoff_input_bundle_with_locale(
        context,
        out_dir,
        required_by_step,
        ArtifactLocale::default(),
    )
}

pub fn structured_handoff_input_bundle_with_locale(
    context: &SourceContext,
    _out_dir: &Path,
    required_by_step: &str,
    artifact_locale: ArtifactLocale,
) -> Value {
    let mut report = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "required_by_step": required_by_step,
        "status": "fallback_to_markdown",
        "inputs": {},
        "warnings": [],
        "missing_inputs": [],
        "artifact_locale": artifact_locale,
    });
    let Some(package) = context.latest_structured_handoff_package() else {
        push_json_array(
            &mut report,
            "warnings",
            json!({
                "code": "STRUCTURED_HANDOFF_PACKAGE_MISSING",
                "message": localized_text(
                    artifact_locale,
                    "未找到结构化设计交接包，本步骤已使用 Markdown/旧版解析输入。",
                    "Step used Markdown/legacy parsed input because no structured Design package was found."
                ),
                "required_by_step": required_by_step,
            }),
        );
        return report;
    };
    let structured_dir = package.join("structured");
    for name in STRUCTURED_EARLY_INPUTS {
        let path = structured_dir.join(format!("{name}.json"));
        if path.exists() {
            report["inputs"][*name] = read_json(&path, json!({}));
        } else {
            push_json_array(
                &mut report,
                "missing_inputs",
                json!({
                    "code": "STRUCTURED_INPUT_MISSING",
                    "contract_id": name,
                    "path": path.to_string_lossy().replace('\\', "/"),
                    "required_by_step": required_by_step,
                }),
            );
        }
    }
    for name in STRUCTURED_OPTIONAL_INPUTS {
        let path = structured_dir.join(format!("{name}.json"));
        if path.exists() {
            report["inputs"][*name] = read_json(&path, json!({}));
        }
    }
    let contract_dir = structured_dir.join("playable_contract_candidates");
    let mut playable_candidates = Map::new();
    if let Ok(entries) = fs::read_dir(&contract_dir) {
        let mut paths = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
            })
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let Some(contract_id) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(ToString::to_string)
            else {
                continue;
            };
            // `artifact_locale` is bundle metadata, not a playable contract.
            // Older D4 exports could materialize it beside the candidates, so
            // ignore it instead of turning a harmless metadata file into a
            // synthetic review item.
            if contract_id == "artifact_locale" {
                continue;
            }
            let payload = read_json(&path, Value::Null);
            if payload.is_object() {
                playable_candidates.insert(contract_id, payload);
            } else {
                push_json_array(
                    &mut report,
                    "warnings",
                    json!({
                        "code": "STRUCTURED_PLAYABLE_CONTRACT_INVALID",
                        "path": path.to_string_lossy().replace('\\', "/"),
                        "required_by_step": required_by_step,
                    }),
                );
            }
        }
    }
    if !playable_candidates.is_empty() {
        report["inputs"]["playable_contract_candidates"] = Value::Object(playable_candidates);
    }
    if report
        .get("missing_inputs")
        .and_then(Value::as_array)
        .map(Vec::is_empty)
        .unwrap_or(true)
    {
        report["status"] = json!("structured");
    } else {
        push_json_array(
            &mut report,
            "warnings",
            json!({
                "code": "STRUCTURED_INPUT_FALLBACK_USED",
                "message": localized_text(
                    artifact_locale,
                    "一个或多个结构化交接输入缺失，本步骤已使用 Markdown/旧版解析输入。",
                    "Step used Markdown/legacy parsed input because one or more structured handoff inputs were missing."
                ),
                "required_by_step": required_by_step,
            }),
        );
    }
    report
}

pub fn structured_decision_options(decisions: &Value) -> Vec<Value> {
    let mut options = Vec::new();
    for decision in decisions
        .get("decisions")
        .and_then(Value::as_array)
        .into_iter()
        .flat_map(|items| items.iter())
    {
        let decision_refs = string_array(decision.get("source_refs"));
        for option in decision
            .get("selected_options")
            .and_then(Value::as_array)
            .into_iter()
            .flat_map(|items| items.iter())
        {
            let label = option
                .get("label")
                .or_else(|| option.get("option_id"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if label.is_empty() {
                continue;
            }
            let source_refs = {
                let refs = string_array(option.get("source_refs"));
                if refs.is_empty() {
                    decision_refs.clone()
                } else {
                    refs
                }
            };
            options.push(json!({
                "node_id": decision.get("node_id").cloned().unwrap_or(Value::Null),
                "domain": decision.get("domain").cloned().unwrap_or(Value::Null),
                "priority": decision.get("priority").cloned().unwrap_or(Value::Null),
                "requirement_level": decision.get("requirement_level").cloned().unwrap_or(Value::Null),
                "contract_targets": decision.get("contract_targets").cloned().unwrap_or_else(|| json!([])),
                "label": label,
                "description": option.get("description").and_then(Value::as_str).unwrap_or_default(),
                "source_refs": source_refs,
                "optionProvenance": option.get("optionProvenance").cloned().unwrap_or_else(|| json!({})),
            }));
        }
    }
    options
}

pub fn structured_selections_from_decisions(decisions: &Value) -> Vec<Selection> {
    structured_decision_options(decisions)
        .into_iter()
        .enumerate()
        .map(|(index, option)| {
            let source_refs = string_array(option.get("source_refs"));
            Selection {
                index: index + 1,
                layer_number: 2,
                layer_title: structured_option_layer_title(&option),
                layer_status: "structured_handoff".to_string(),
                item_type: structured_option_item_type(&option),
                option: option
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("Structured decision")
                    .to_string(),
                purpose: option
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("From design workbench structured selection.")
                    .to_string(),
                dependencies: Vec::new(),
                unlocks: Vec::new(),
                source_ref: source_refs
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "structured/decisions.json".to_string()),
                source_line: 0,
            }
        })
        .collect()
}

pub fn structured_fallback_parsed(
    step_number: u32,
    source_error: &DesignSourceError,
    context: &SourceContext,
) -> Option<ParsedDesignSource> {
    if step_number > 1 {
        return None;
    }
    let out_dir = context.source_service().stage_dir(step_number);
    let bundle =
        structured_handoff_input_bundle(context, &out_dir, &format!("Step{step_number:02}"));
    let decisions = bundle
        .get("inputs")
        .and_then(|inputs| inputs.get("decisions"))?;
    let selections = structured_selections_from_decisions(decisions);
    if selections.is_empty() {
        return None;
    }
    let profile = bundle
        .get("inputs")
        .and_then(|inputs| inputs.get("profile"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let archetype = bundle
        .get("inputs")
        .and_then(|inputs| inputs.get("archetype_requirements"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let raw_text = [
        value_string(profile.get("project_id")),
        value_string(profile.get("genre").or_else(|| profile.get("genre_id"))),
        value_string(archetype.get("archetype")),
        selections
            .iter()
            .map(|selection| selection.option.clone())
            .collect::<Vec<_>>()
            .join(" "),
    ]
    .into_iter()
    .filter(|item| !item.is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    let layer = DesignLayer {
        number: 2,
        title: "structured_design".to_string(),
        status: "structured_handoff".to_string(),
        source_ref: "structured/decisions.json".to_string(),
        selections: selections.clone(),
    };
    Some(ParsedDesignSource {
        source: "structured/decisions.json".to_string(),
        source_path: String::new(),
        source_sha256: String::new(),
        source_size_bytes: 0,
        source_line_count: 0,
        parsed_at: now_iso(),
        layers: vec![layer],
        selections,
        raw_text,
        source_package: "structured_design_handoff".to_string(),
        source_input_type: "structured_handoff_fallback".to_string(),
        design_summary: json!({}),
        structured_source_warning: Some(json!({
            "code": "MARKDOWN_SOURCE_FALLBACK_TO_STRUCTURED_HANDOFF",
            "original_error": source_error.details(),
        })),
    })
}

pub fn latest_concept_package(context: &SourceContext) -> Option<PathBuf> {
    let mut candidates = context
        .iter_source_packages()
        .into_iter()
        .filter(|path| {
            context.package_matches_type(path, "Concept")
                || path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .is_some_and(|name| name.starts_with("s00_cpt_v"))
        })
        .collect::<Vec<_>>();
    sort_packages_newest_first(&mut candidates);
    candidates.into_iter().next()
}

pub fn load_current_design(
    context: &SourceContext,
) -> Result<ParsedDesignSource, DesignSourceError> {
    let package_dir = latest_concept_package(context).ok_or_else(|| {
        DesignSourceError::new(
            "Stage 00 has no submitted Concept source package.",
            json!({"code": "STAGE00_CONCEPT_SOURCE_MISSING", "fix": "Submit operator notes or one .md/.txt design attachment."}),
        )
    })?;
    let manifest = context.package_manifest(&package_dir);
    let submission = load_concept_submission(&package_dir);
    let notes = submission
        .get("notes")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let (valid_attachments, invalid_attachments) =
        concept_attachment_paths(&package_dir, &submission);
    let package_rel = rel(&package_dir, &context.source_service().paths().project_root);
    if !invalid_attachments.is_empty() {
        return Err(DesignSourceError::new(
            "Stage 00 contains unsupported or missing attachments.",
            json!({
                "code": "STAGE00_ATTACHMENT_INVALID",
                "package": package_rel,
                "invalid_attachments": invalid_attachments,
                "allowed_extensions": ALLOWED_DESIGN_SOURCE_SUFFIXES,
                "fix": "Submit only .md or .txt attachments.",
            }),
        ));
    }
    if valid_attachments.len() > 1 {
        return Err(DesignSourceError::new(
            "Stage 00 contains multiple design document attachments and no explicit primary source.",
            json!({
                "code": "STAGE00_MULTIPLE_PRIMARY_CANDIDATES",
                "package": package_rel,
                "attachments": valid_attachments.iter().map(|path| rel(path, &context.source_service().paths().project_root)).collect::<Vec<_>>(),
                "fix": "Keep only one .md/.txt primary design document attachment.",
            }),
        ));
    }
    if let Some(path) = valid_attachments.first() {
        let mut parsed = parse_design_doc(&context.source_service().paths().project_root, path)
            .map_err(|error| {
                DesignSourceError::new(
                    format!("failed to parse design attachment: {error}"),
                    json!({"code": "STAGE00_ATTACHMENT_READ_FAILED", "package": package_rel}),
                )
            })?;
        parsed.source_package = package_rel.clone();
        parsed.design_summary = manifest
            .get("design_summary")
            .cloned()
            .unwrap_or_else(|| json!({}));
        parsed.source_input_type = "concept_attachment".to_string();
        if !parsed.selections.is_empty() {
            return Ok(parsed);
        }
        return Err(DesignSourceError::new(
            "Stage 00 design attachment is not a parseable Layer document.",
            json!({
                "code": "STAGE00_ATTACHMENT_UNPARSEABLE",
                "package": parsed.source_package,
                "attachment": parsed.source,
                "fix": "Use '## Layer N ...' headings and '- Type: Option' items.",
            }),
        ));
    }
    if !notes.is_empty() {
        let mut parsed = manual_notes_as_design(
            &notes,
            &format!("{package_rel}/operator_submission.json#notes"),
        );
        parsed.source_package = package_rel.clone();
        parsed.design_summary = manifest
            .get("design_summary")
            .cloned()
            .unwrap_or_else(|| json!({}));
        parsed.source_input_type = "operator_notes".to_string();
        if !parsed.selections.is_empty() {
            return Ok(parsed);
        }
    }
    Err(DesignSourceError::new(
        "Stage 00 has no operator notes or valid design document attachment.",
        json!({"code": "STAGE00_INPUT_MISSING", "package": package_rel, "fix": "Submit notes or one .md/.txt design attachment."}),
    ))
}

pub fn load_package_by_source_id(
    context: &SourceContext,
    source_id: &str,
) -> Result<ParsedDesignSource, DesignSourceError> {
    let package_dir = context.latest_source_package(source_id).ok_or_else(|| {
        DesignSourceError::new(
            format!("Stage has no submitted {source_id} source package."),
            json!({"code": format!("STAGE_{}_SOURCE_MISSING", source_id.to_ascii_uppercase()), "fix": format!("Generate the {source_id} package.")}),
        )
    })?;
    let submission = load_concept_submission(&package_dir);
    let (valid_attachments, _) = concept_attachment_paths(&package_dir, &submission);
    let Some(path) = valid_attachments.first() else {
        return Err(DesignSourceError::new(
            format!("{source_id} package has no valid attachment."),
            json!({"code": format!("STAGE_{}_ATTACHMENT_MISSING", source_id.to_ascii_uppercase())}),
        ));
    };
    let manifest = context.package_manifest(&package_dir);
    let mut parsed = parse_design_doc(&context.source_service().paths().project_root, path)
        .map_err(|error| {
            DesignSourceError::new(
                format!("failed to parse {source_id} attachment: {error}"),
                json!({"code": format!("STAGE_{}_ATTACHMENT_READ_FAILED", source_id.to_ascii_uppercase())}),
            )
        })?;
    parsed.source_package = rel(&package_dir, &context.source_service().paths().project_root);
    parsed.design_summary = manifest
        .get("design_summary")
        .cloned()
        .unwrap_or_else(|| json!({}));
    parsed.source_input_type = format!("{}_attachment", source_id.to_ascii_lowercase());
    Ok(parsed)
}

pub fn load_design_for_stage(
    context: &SourceContext,
    step_number: u32,
) -> Result<ParsedDesignSource, DesignSourceError> {
    if step_number == 0 {
        return load_current_design(context);
    }
    if step_number == 1 {
        return load_package_by_source_id(context, "GameplayFramework")
            .or_else(|_| load_current_design(context));
    }
    load_package_by_source_id(context, "Design").or_else(|_| load_current_design(context))
}

pub fn update_stage_report(step_number: u32, out_dir: &Path, result: &Value) -> AdmResult<Value> {
    update_stage_report_with_locale(step_number, out_dir, result, ArtifactLocale::default())
}

pub fn update_stage_report_with_locale(
    step_number: u32,
    out_dir: &Path,
    result: &Value,
    artifact_locale: ArtifactLocale,
) -> AdmResult<Value> {
    let report_path = out_dir.join("validation_report.json");
    let mut report = object_map_or_empty(read_json(&report_path, json!({})));
    let status = result
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let completed_with_review = status == "completed_with_review";
    let blocking = blocking_issue_count(result.get("blocking_issues"));
    let content_exists = result
        .get("content_exists")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    report.insert("stage".to_string(), json!(step_number));
    report.insert("artifact_locale".to_string(), json!(artifact_locale));
    report.insert("content_exists".to_string(), json!(content_exists));
    report.insert(
        "ai_review_status".to_string(),
        result
            .get("ai_review_status")
            .cloned()
            .unwrap_or_else(|| json!(if blocking == 0 { "passed" } else { "blocked" })),
    );
    report.insert("blocking_issues".to_string(), json!(blocking));
    report.insert(
        "review_items_count".to_string(),
        result
            .get("review_items_count")
            .cloned()
            .unwrap_or(json!(0)),
    );
    report.insert(
        "traceability_valid".to_string(),
        json!(
            result
                .get("traceability_valid")
                .and_then(Value::as_bool)
                .unwrap_or(true)
                && blocking == 0
        ),
    );
    report.insert("scope_budget_valid".to_string(), json!(true));
    report.insert("business_quality".to_string(), result.clone());
    for source_key in [
        "imported_sources",
        "imported_upstream_artifacts",
        "missing_groups",
        "missing_required_groups",
        "optional_missing_groups",
        "missing_upstream_artifacts",
    ] {
        if let Some(value) = result.get(source_key) {
            report.insert(source_key.to_string(), value.clone());
        } else {
            report
                .entry(source_key.to_string())
                .or_insert_with(|| json!([]));
        }
    }
    if matches!(status, "stopped" | "recovery_blocked") {
        // Operational interruption states are validly reported outcomes. They
        // must reach the pipeline state machine instead of being collapsed into
        // a generator/transport failure by `apply_development_plan_outputs`.
        report.insert("status".to_string(), json!(status));
        report.insert("valid".to_string(), json!(true));
    } else if status == "blocked" || (completed_with_review && blocking > 0) {
        report.insert("status".to_string(), json!("blocked"));
        report.insert("valid".to_string(), json!(false));
    } else if completed_with_review {
        report.insert("status".to_string(), json!("completed_with_review"));
        report.insert("valid".to_string(), json!(true));
    } else if !content_exists {
        report.insert("status".to_string(), json!("content_missing"));
        report.insert("valid".to_string(), json!(false));
    } else if blocking > 0 {
        report.insert("status".to_string(), json!("failed"));
        report.insert("valid".to_string(), json!(false));
    } else if content_exists {
        report.insert(
            "status".to_string(),
            json!(if status.is_empty() { "passed" } else { status }),
        );
        report.insert("valid".to_string(), json!(true));
    }
    let value = Value::Object(report);
    write_json(&report_path, &value)?;
    Ok(value)
}

pub fn refresh_indexes(source: &SourceService, step_number: u32, out_dir: &Path) -> AdmResult<()> {
    refresh_indexes_with_locale(source, step_number, out_dir, ArtifactLocale::default())
}

pub fn refresh_indexes_with_locale(
    source: &SourceService,
    step_number: u32,
    out_dir: &Path,
    artifact_locale: ArtifactLocale,
) -> AdmResult<()> {
    let index_path = out_dir.join("artifact_index.json");
    let mut index = object_map_or_empty(read_json(&index_path, json!({})));
    index.insert("manifest".to_string(), json!(file_manifest(out_dir)?));
    index.insert("updated_at".to_string(), json!(now_iso()));
    index.insert("development_plan_outputs".to_string(), json!(true));
    index.insert("artifact_locale".to_string(), json!(artifact_locale));
    write_json(&index_path, &Value::Object(index))?;
    source.refresh_reference_manifest_file_inventory_with_locale(step_number, artifact_locale)?;
    Ok(())
}

pub fn blocking_issue_count(value: Option<&Value>) -> usize {
    match value {
        Some(Value::Array(items)) => items.len(),
        Some(Value::Object(items)) => items.len(),
        Some(Value::Number(number)) => number.as_u64().unwrap_or(0) as usize,
        Some(Value::Bool(true)) => 1,
        Some(Value::String(value)) => value
            .parse::<usize>()
            .unwrap_or(usize::from(!value.is_empty())),
        _ => 0,
    }
}

pub fn design_signal_text(parsed: &ParsedDesignSource) -> Vec<String> {
    let mut signals = vec![parsed.raw_text.clone()];
    for selection in &parsed.selections {
        signals.push(selection.item_type.clone());
        signals.push(selection.option.clone());
        signals.push(selection.label());
        signals.push(selection.purpose.clone());
    }
    signals
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect()
}

pub fn source_digest(
    parsed: &ParsedDesignSource,
    answered_questions: usize,
    total_questions: usize,
) -> String {
    let mut lines = vec![
        "# Source Digest".to_string(),
        String::new(),
        format!("- Source: {}", parsed.source),
        format!("- Parsed selections: {}", parsed.selections.len()),
        format!("- Answered design questions: {answered_questions}/{total_questions}"),
        String::new(),
        "## Selected Items".to_string(),
        String::new(),
    ];
    for selection in &parsed.selections {
        lines.push(format!(
            "- {} ({})",
            selection.label(),
            selection.source_ref
        ));
        if !selection.purpose.is_empty() {
            lines.push(format!("  - Purpose: {}", selection.purpose));
        }
        if !selection.dependencies.is_empty() {
            lines.push(format!(
                "  - Dependencies: {}",
                selection.dependencies.join(", ")
            ));
        }
        if !selection.unlocks.is_empty() {
            lines.push(format!("  - Unlocks: {}", selection.unlocks.join(", ")));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn parse_layer_heading(line: &str) -> Option<(u32, String)> {
    if let Some(rest) = line.strip_prefix("## Layer ") {
        let mut parts = rest.trim().splitn(2, char::is_whitespace);
        let number = parts.next()?.trim().parse::<u32>().ok()?;
        let title = parts.next().unwrap_or_default().trim();
        return Some((number, title.to_string()));
    }
    let rest = line.strip_prefix("## 第 ")?;
    let mut parts = rest.trim().splitn(3, char::is_whitespace);
    let number = parts.next()?.trim().parse::<u32>().ok()?;
    if parts.next()?.trim() != "层" {
        return None;
    }
    let title = parts.next().unwrap_or_default().trim();
    Some((number, title.to_string()))
}

fn split_item_type(body: &str) -> Option<(String, String)> {
    let colon = body.find('：').or_else(|| body.find(':'))?;
    let item_type = body[..colon].trim();
    let option = body[colon + body[colon..].chars().next()?.len_utf8()..].trim();
    if item_type.is_empty() || option.is_empty() {
        None
    } else {
        Some((item_type.to_string(), option.to_string()))
    }
}

fn default_item_type_for_layer(title: &str) -> Option<&'static str> {
    match title {
        "系统图" | "System Graph" => Some("系统"),
        _ => None,
    }
}

fn sort_packages_newest_first(candidates: &mut [PathBuf]) {
    candidates.sort_by(|left, right| package_sort_key(right).cmp(&package_sort_key(left)));
}

fn package_sort_key(path: &Path) -> (u32, u64) {
    let version = path
        .file_name()
        .and_then(|value| value.to_str())
        .and_then(|name| name.rsplit_once("_v").map(|(_, version)| version))
        .and_then(|version| version.parse::<u32>().ok())
        .unwrap_or(0);
    let mtime = path
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    (version, mtime)
}

fn load_concept_submission(package_dir: &Path) -> Value {
    let submission = read_json(&package_dir.join("operator_submission.json"), json!({}));
    if submission.is_object() {
        submission
    } else {
        json!({})
    }
}

fn concept_attachment_paths(package_dir: &Path, submission: &Value) -> (Vec<PathBuf>, Vec<String>) {
    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    let package_root = package_dir
        .canonicalize()
        .unwrap_or_else(|_| package_dir.to_path_buf());
    for raw in string_array(submission.get("attachments")) {
        if raw.trim().is_empty() {
            continue;
        }
        let path = package_dir.join(&raw);
        let resolved = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !resolved.starts_with(&package_root) {
            invalid.push(raw);
            continue;
        }
        let suffix = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| format!(".{}", value.to_ascii_lowercase()))
            .unwrap_or_default();
        if !ALLOWED_DESIGN_SOURCE_SUFFIXES.contains(&suffix.as_str()) || !path.is_file() {
            invalid.push(raw);
            continue;
        }
        valid.push(path);
    }
    (valid, invalid)
}

fn structured_option_item_type(option: &Value) -> String {
    let targets = string_array(option.get("contract_targets"))
        .into_iter()
        .collect::<BTreeSet<_>>();
    if !targets.is_disjoint(&BTreeSet::from([
        "core_playable_contract".to_string(),
        "demo_flow_contract".to_string(),
    ])) {
        "核心循环".to_string()
    } else if targets.contains("ui_flow_contract") {
        "HUD".to_string()
    } else if targets.contains("scene_bootstrap_contract") {
        "场景".to_string()
    } else if targets.contains("runtime_data_contract") {
        "运行时数据".to_string()
    } else if targets.contains("playable_acceptance_contract") {
        "目标".to_string()
    } else if targets.contains("asset_mount_contract") {
        "资源".to_string()
    } else {
        value_string(option.get("domain")).if_empty("结构化决策")
    }
}

fn structured_option_layer_title(option: &Value) -> String {
    let targets = string_array(option.get("contract_targets"))
        .into_iter()
        .collect::<BTreeSet<_>>();
    if targets.iter().any(|target| {
        [
            "core_playable_contract",
            "demo_flow_contract",
            "runtime_data_contract",
            "ui_flow_contract",
            "scene_bootstrap_contract",
            "asset_mount_contract",
            "playable_acceptance_contract",
        ]
        .contains(&target.as_str())
    }) {
        "系统图".to_string()
    } else {
        "结构化设计".to_string()
    }
}

fn object_map_or_empty(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(value) => Some(value.trim().to_string()),
                Value::Number(value) => Some(value.to_string()),
                Value::Bool(value) => Some(value.to_string()),
                _ => None,
            })
            .filter(|item| !item.is_empty())
            .collect(),
        Some(Value::String(value)) => value
            .split([',', ';', '\n'])
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(str::to_string)
            .collect(),
        Some(other) if !other.is_null() => vec![other.to_string()],
        _ => Vec::new(),
    }
}

fn value_string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.trim().to_string(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn push_json_array(parent: &mut Value, key: &str, value: Value) {
    if let Some(object) = parent.as_object_mut() {
        let entry = object
            .entry(key.to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(items) = entry.as_array_mut() {
            items.push(value);
        }
    }
}

trait EmptyFallback {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::io::write_text;
    use adm_new_foundation::new_stable_id;

    #[test]
    fn parse_design_text_matches_layer_selection_shape() {
        let parsed = parse_design_text(
            "## Layer 1 Core\nReady / Approved\n- 核心循环：Collect and upgrade\n  目的：Drive repeat play\n  依赖：SYS_INPUT、SYS_PLAYER\n  解锁：SYS_SCORE\n## Layer 2 系统图\n- Inventory\n",
            "design.md",
            "",
            None,
            None,
        );

        assert_eq!(parsed.layers.len(), 2);
        assert_eq!(parsed.selections.len(), 2);
        assert_eq!(parsed.selections[0].id(), "SEL-001");
        assert_eq!(
            parsed.selections[0].dependencies,
            vec!["SYS_INPUT", "SYS_PLAYER"]
        );
        assert_eq!(parsed.selections[1].item_type, "系统");
        assert!(source_digest(&parsed, 1, 5).contains("Parsed selections: 2"));
    }

    #[test]
    fn parse_design_text_accepts_exporter_english_metadata() {
        let parsed = parse_design_text(
            r#"## Layer 5 L5 Entities
Submitted / accepted
- L5 node: combat_node
  Purpose: concrete node
- L5 entity: Peashooter (pea)
  Purpose: kind=character; schema=character_card_v1
  Depends: combat_node
  Unlocks: program_requirements, art_requirements
"#,
            "design.md",
            "",
            None,
            None,
        );

        assert_eq!(parsed.selections.len(), 2);
        assert_eq!(parsed.selections[0].item_type, "L5节点");
        assert_eq!(parsed.selections[0].purpose, "concrete node");
        assert_eq!(parsed.selections[1].item_type, "L5实体");
        assert_eq!(
            parsed.selections[1].purpose,
            "kind=character; schema=character_card_v1"
        );
        assert_eq!(parsed.selections[1].dependencies, vec!["combat_node"]);
        assert_eq!(
            parsed.selections[1].unlocks,
            vec!["program_requirements", "art_requirements"]
        );
    }

    #[test]
    fn parse_design_text_accepts_chinese_layer_protocol() {
        let parsed = parse_design_text(
            r#"## 第 2 层 核心体验
已提交 / 已接受
- 核心循环: 收集资源 -> 布置单位 -> 击退敌人
  目的: 核心玩法循环
## 第 5 层 L5 实体
- L5 节点: combat_node
  目的: 具体设计节点
- L5 实体: 豌豆射手 (pea)
  目的: kind=character; schema=character_card_v1
  依赖: combat_node
  解锁: program_requirements, art_requirements
"#,
            "design.md",
            "",
            None,
            None,
        );

        assert_eq!(parsed.layers.len(), 2);
        assert_eq!(parsed.layers[0].number, 2);
        assert_eq!(parsed.layers[0].title, "核心体验");
        assert_eq!(parsed.selections[1].item_type, "L5节点");
        assert_eq!(parsed.selections[2].item_type, "L5实体");
        assert_eq!(parsed.selections[2].dependencies, vec!["combat_node"]);
    }

    #[test]
    fn source_context_finds_latest_package_and_structured_handoff() {
        let root = temp_root("generation_source_context");
        let service = GenerationService::new(&root, "session_a").unwrap();
        let source_root = service.source().paths().source_artifacts_dir.clone();
        let old = source_root.join("devflow_Design_v1");
        let latest = source_root.join("devflow_Design_v2");
        write_json(
            &old.join("package_manifest.json"),
            &json!({"package_type": "Design", "source_ids": ["Design"]}),
        )
        .unwrap();
        write_json(
            &latest.join("package_manifest.json"),
            &json!({"package_type_id": "design"}),
        )
        .unwrap();
        write_json(
            &latest.join("structured").join("handoff_manifest.json"),
            &json!({"ok": true}),
        )
        .unwrap();
        for input in STRUCTURED_EARLY_INPUTS {
            write_json(
                &latest.join("structured").join(format!("{input}.json")),
                &json!({"input": input}),
            )
            .unwrap();
        }
        write_json(
            &latest
                .join("structured/playable_contract_candidates")
                .join("core_playable_contract.json"),
            &json!({"contract": "core"}),
        )
        .unwrap();
        // Compatibility with a short-lived D4 export that accidentally wrote
        // bundle metadata beside playable contract candidates.
        write_json(
            &latest
                .join("structured/playable_contract_candidates")
                .join("artifact_locale.json"),
            &json!("zh-CN"),
        )
        .unwrap();

        let context = service.source_context();
        let bundle = structured_handoff_input_bundle(&context, &root, "Step02");

        assert_eq!(context.latest_source_package("Design").unwrap(), latest);
        assert_eq!(context.latest_structured_handoff_package().unwrap(), latest);
        assert_eq!(bundle["status"], json!("structured"));
        assert_eq!(
            bundle["inputs"]["playable_contract_candidates"]["core_playable_contract"]["contract"],
            json!("core")
        );
        assert_eq!(bundle["warnings"], json!([]));
        assert!(
            bundle["inputs"]["playable_contract_candidates"]
                .get("artifact_locale")
                .is_none()
        );
        cleanup(root);
    }

    #[test]
    fn handoff_loader_prefers_adjacent_design_handoff() {
        let root = temp_root("generation_handoff_loader");
        let service = GenerationService::new(&root, "session_a").unwrap();
        let design_doc = root.join("docs").join("design.md");
        write_text(&design_doc, "# Design").unwrap();
        write_json(
            &design_doc.parent().unwrap().join("design_handoff.json"),
            &json!({"handoff": true}),
        )
        .unwrap();

        let sources = load_design_sources(&service.source_context(), &design_doc).unwrap();

        assert_eq!(sources["handoff"]["handoff"], json!(true));
        assert_eq!(sources["markdown"], json!("# Design"));
        cleanup(root);
    }

    #[test]
    fn load_current_design_uses_notes_and_rejects_invalid_attachments() {
        let root = temp_root("generation_current_design");
        let service = GenerationService::new(&root, "session_a").unwrap();
        let package = service
            .source()
            .paths()
            .source_artifacts_dir
            .join("s00_cpt_v1");
        write_json(
            &package.join("package_manifest.json"),
            &json!({"stage": 0, "package_type": "Concept"}),
        )
        .unwrap();
        write_json(
            &package.join("operator_submission.json"),
            &json!({"notes": "Build a compact puzzle game"}),
        )
        .unwrap();

        let parsed = service.load_current_design().unwrap();

        assert_eq!(parsed.source_input_type, "operator_notes");
        assert_eq!(parsed.selections.len(), 1);

        write_json(
            &package.join("operator_submission.json"),
            &json!({"attachments": ["bad.png"]}),
        )
        .unwrap();
        write_text(&package.join("bad.png"), "png").unwrap();
        let error = service.load_current_design().unwrap_err();
        assert_eq!(error.details()["code"], json!("STAGE00_ATTACHMENT_INVALID"));
        cleanup(root);
    }

    #[test]
    fn apply_development_plan_outputs_writes_contract_report_and_indexes() {
        let root = temp_root("generation_apply");
        let service = GenerationService::new(&root, "session_a").unwrap();
        let package = service
            .source()
            .paths()
            .source_artifacts_dir
            .join("s00_cpt_v2");
        write_json(
            &package.join("package_manifest.json"),
            &json!({"stage": 0, "package_type": "Concept"}),
        )
        .unwrap();
        write_text(
            &package.join("design.md"),
            "## Layer 1 Core\nReady / Approved\n- Core Loop: Dodge and score\n",
        )
        .unwrap();
        write_json(
            &package.join("operator_submission.json"),
            &json!({"attachments": ["design.md"]}),
        )
        .unwrap();

        let report = service
            .apply_development_plan_outputs(0, &ContractStageOutputGenerator)
            .unwrap();
        let out_dir = service.stage_dir(0);

        assert_eq!(report["valid"], json!(true));
        assert!(out_dir.join("generation_contract.json").is_file());
        assert!(out_dir.join("artifact_index.json").is_file());
        assert!(out_dir.join("reference_manifest.json").is_file());
        let reference_manifest = read_json(&out_dir.join("reference_manifest.json"), json!({}));
        assert_eq!(reference_manifest["artifact_locale"], "zh-CN");
        assert_eq!(
            reference_manifest["stage"]["display_title"],
            "步骤 00 创意接收"
        );
        cleanup(root);
    }

    #[test]
    fn operational_stop_report_remains_a_valid_stopped_outcome() {
        let root = temp_root("generation_stopped_report");
        fs::create_dir_all(&root).unwrap();
        let report = update_stage_report(
            11,
            &root,
            &json!({
                "status": "stopped",
                "content_exists": true,
                "blocking_issues": 1,
                "stop_requested": true,
                "recovery_blocked": false,
            }),
        )
        .unwrap();
        assert_eq!(report["status"], "stopped");
        assert_eq!(report["valid"], true);
        assert_eq!(report["business_quality"]["stop_requested"], true);
        cleanup(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm_new_pipeline_{label}_{}",
            new_stable_id("root").unwrap()
        ))
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
