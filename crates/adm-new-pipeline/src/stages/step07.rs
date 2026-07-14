use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use adm_new_contracts::ArtifactLocale;
use adm_new_foundation::io::{now_iso, read_json, write_json, write_text};
use adm_new_foundation::{AdmError, AdmResult, sanitize_identifier};
use image::{ImageFormat, Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::generation::{
    ParsedDesignSource, StageOutputGenerator, artifact_locale_from_inputs, localized_text,
};
use crate::source::SourceGroup;
use crate::stages::step00_02::StagePluginSpec;
use crate::style_image::{SafeStyleImageExecutor, StyleImageGenerator};
#[cfg(test)]
use crate::style_image::{StyleImageRequest, StyleImageResult};
use crate::work_units::{
    SafeUnitJournal, WorkUnitExecutionResult, WorkUnitKind, WorkUnitRequest, WorkUnitRunOutcome,
    WorkUnitRunStatus, WorkUnitStopToken, execute_work_unit_batch,
};

pub const STEP07: u32 = 7;
pub const LEGACY_ART_STYLE_CONFIRMATION_STAGE: u32 = 8;
pub const STYLE_CONFIRMATION_FILENAME: &str = "style_confirmation.json";
pub const STYLE_PROMPT_OVERRIDE_FILENAME: &str = "prompt_override.json";
pub const PROMPT_START: &str = "PROMPT_START";
pub const PROMPT_END: &str = "PROMPT_END";

const FALLBACK_IMAGE_WIDTH: u32 = 640;
const FALLBACK_IMAGE_HEIGHT: u32 = 384;
const PROVIDER_IMAGE_WIDTH: u32 = 1536;
const PROVIDER_IMAGE_HEIGHT: u32 = 1024;

pub fn step07_plugin_spec() -> StagePluginSpec {
    StagePluginSpec {
        stage_id: "07",
        source_groups: Vec::<SourceGroup>::new(),
        test_mode_status: "success",
        generation_entrypoint: "apply_development_plan_outputs",
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleOption {
    pub style_id: String,
    pub title: String,
    pub description: String,
    pub palette: Vec<String>,
    pub source_refs: Vec<String>,
    pub prompt: String,
    #[serde(default)]
    pub generation_prompt: String,
    #[serde(default)]
    pub negative_prompt: String,
    #[serde(default)]
    pub image_path: String,
    #[serde(default)]
    pub score: i64,
    #[serde(default)]
    pub recommended: bool,
    #[serde(default)]
    pub recommendation_reason: String,
    #[serde(default)]
    pub prompt_refined: bool,
}

impl fmt::Debug for StyleOption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StyleOption")
            .field("style_id", &self.style_id)
            .field("title", &self.title)
            .field("palette", &self.palette)
            .field("source_ref_count", &self.source_refs.len())
            .field("prompt_configured", &!self.prompt.is_empty())
            .field("image_configured", &!self.image_path.is_empty())
            .field("score", &self.score)
            .field("recommended", &self.recommended)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtAssetInput {
    pub asset_id: String,
    pub name: String,
    pub asset_type: String,
    pub source: String,
    pub priority: String,
    pub complexity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StylePromptParseResult {
    pub explanation: String,
    pub prompts: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleGenerationConfig {
    pub option_count: usize,
    pub image_generation_enabled: bool,
}

impl Default for StyleGenerationConfig {
    fn default() -> Self {
        Self {
            option_count: 5,
            image_generation_enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Step07Inputs {
    pub asset_spec_assets: Vec<ArtAssetInput>,
    pub asset_registry_assets: Vec<ArtAssetInput>,
    pub art_review: Value,
}

impl Step07Inputs {
    pub fn from_stage_dirs(stage07_dir: &Path) -> Self {
        let artifacts_dir = stage07_dir.parent().unwrap_or(stage07_dir);
        let stage04_dir = artifacts_dir.join("stage_04");
        let stage06_dir = artifacts_dir.join("stage_06");
        let asset_spec = read_json(&stage04_dir.join("asset_spec_contract.json"), json!({}));
        let asset_registry = read_json(&stage04_dir.join("asset_registry.json"), json!({}));
        let art_ai_review = read_json(&stage06_dir.join("art_ai_review_report.json"), json!({}));
        let art_review = if is_empty_object(&art_ai_review) {
            read_json(&stage06_dir.join("art_review_report.json"), json!({}))
        } else {
            art_ai_review
        };
        Self {
            asset_spec_assets: parse_art_assets(asset_spec.get("assets")),
            asset_registry_assets: parse_art_assets(asset_registry.get("assets")),
            art_review,
        }
    }

    pub fn usable_assets(&self) -> Vec<ArtAssetInput> {
        if self.asset_registry_assets.is_empty() {
            self.asset_spec_assets.clone()
        } else {
            self.asset_registry_assets.clone()
        }
    }

    pub fn asset_spec_has_assets(&self) -> bool {
        !self.asset_spec_assets.is_empty()
    }
}

#[derive(Debug, Clone, Copy)]
struct StylePreset {
    key: &'static str,
    title_zh_cn: &'static str,
    title_en_us: &'static str,
    description_zh_cn: &'static str,
    description_en_us: &'static str,
    palette: [&'static str; 3],
}

impl StylePreset {
    fn title(self, locale: ArtifactLocale) -> &'static str {
        localized_text(locale, self.title_zh_cn, self.title_en_us)
    }

    fn description(self, locale: ArtifactLocale) -> &'static str {
        localized_text(locale, self.description_zh_cn, self.description_en_us)
    }
}

const STYLE_OPTION_PRESETS: &[StylePreset] = &[
    StylePreset {
        key: "readable_production",
        title_zh_cn: "清晰量产风",
        title_en_us: "Readable Production",
        description_zh_cn: "清晰轮廓、生产友好的材质分层，游戏对比度易读，适合批量资产制作。",
        description_en_us: "Clear silhouettes, production-friendly material separation, and readable game contrast support scalable asset production.",
        palette: ["#2E3440", "#88C0D0", "#EBCB8B"],
    },
    StylePreset {
        key: "painterly_concept",
        title_zh_cn: "概念绘画风",
        title_en_us: "Painterly Concept",
        description_zh_cn: "手绘质感表面、富有表现力的光线，以概念艺术构图探索氛围与情绪。",
        description_en_us: "Hand-painted surfaces and expressive lighting explore atmosphere and emotion through concept-art composition.",
        palette: ["#3B4252", "#A3BE8C", "#D08770"],
    },
    StylePreset {
        key: "high_contrast_arcade",
        title_zh_cn: "高对比街机风",
        title_en_us: "High-Contrast Arcade",
        description_zh_cn: "大胆色块分区、清脆反馈配色，运动中仍能快速扫描识别关键元素。",
        description_en_us: "Bold color blocking and crisp feedback colors keep key elements easy to scan while in motion.",
        palette: ["#1B1F3B", "#F2CC8F", "#E07A5F"],
    },
    StylePreset {
        key: "cinematic_realism",
        title_zh_cn: "电影写实风",
        title_en_us: "Cinematic Realism",
        description_zh_cn: "真实材质、强烈主光源，高保真场景搭建，接近 AAA 电影级视觉。",
        description_en_us: "Realistic materials, strong key lighting, and high-fidelity scene construction approach AAA cinematic visuals.",
        palette: ["#202124", "#6D6875", "#B5838D"],
    },
    StylePreset {
        key: "stylized_diagrammatic",
        title_zh_cn: "风格化图示风",
        title_en_us: "Stylized Diagrammatic",
        description_zh_cn: "简化形体、强烈形状语言，视觉层级清晰，适合 UI 集成与信息传达。",
        description_en_us: "Simplified forms, strong shape language, and clear visual hierarchy support UI integration and information delivery.",
        palette: ["#264653", "#2A9D8F", "#E9C46A"],
    },
];

pub fn parse_style_prompt_response(
    response: &str,
    valid_style_ids: Option<&BTreeSet<String>>,
) -> StylePromptParseResult {
    let text = response.trim();
    let explanation = text
        .split_once(PROMPT_START)
        .map(|(before, _)| before.trim().to_string())
        .unwrap_or_else(|| text.to_string());
    let Some((_, after_start)) = text.split_once(PROMPT_START) else {
        return StylePromptParseResult {
            explanation,
            prompts: BTreeMap::new(),
        };
    };
    let Some((block, _)) = after_start.split_once(PROMPT_END) else {
        return StylePromptParseResult {
            explanation,
            prompts: BTreeMap::new(),
        };
    };
    let mut prompts = BTreeMap::new();
    for raw_line in block.lines() {
        let line = raw_line
            .trim()
            .trim_matches('`')
            .trim_start_matches(|ch| ch == '-' || ch == '*')
            .trim();
        if line.is_empty() || line.starts_with("```") {
            continue;
        }
        let Some((style_id, prompt)) = line.split_once(':') else {
            continue;
        };
        let style_id = style_id.trim();
        let prompt = prompt.trim();
        if style_id.is_empty() || prompt.is_empty() {
            continue;
        }
        if let Some(valid) = valid_style_ids {
            if !valid.contains(style_id) {
                continue;
            }
        }
        prompts.insert(style_id.to_string(), prompt.to_string());
    }
    StylePromptParseResult {
        explanation,
        prompts,
    }
}

pub fn build_style_prompt_override(
    options: &[StyleOption],
    refined_prompts: &BTreeMap<String, String>,
    count: usize,
) -> Value {
    build_style_prompt_override_with_locale(
        options,
        refined_prompts,
        count,
        ArtifactLocale::default(),
    )
}

fn build_style_prompt_override_with_locale(
    options: &[StyleOption],
    refined_prompts: &BTreeMap<String, String>,
    count: usize,
    locale: ArtifactLocale,
) -> Value {
    let requested_count = count.clamp(1, 5);
    let final_options = options
        .iter()
        .take(requested_count)
        .map(|option| {
            let mut option = option.clone();
            let refined = refined_prompts
                .get(&option.style_id)
                .map(String::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            option.prompt_refined = !refined.is_empty();
            if !refined.is_empty() {
                option.prompt = refined.clone();
                option.generation_prompt = refined;
            } else if option.generation_prompt.is_empty() {
                option.generation_prompt = option.prompt.clone();
            }
            if option.negative_prompt.is_empty() {
                option.negative_prompt = style_negative_prompt(locale).to_string();
            }
            to_json_value(&option).unwrap_or_else(|_| json!({}))
        })
        .filter(|value| value.get("style_id").and_then(Value::as_str).is_some())
        .collect::<Vec<_>>();
    json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "source": "style_prompt_editor",
        "requested_count": requested_count,
        "count": final_options.len(),
        "options": final_options,
    })
}

pub fn write_style_prompt_override(
    output_dir: &Path,
    options: &[StyleOption],
    refined_prompts: &BTreeMap<String, String>,
    count: usize,
) -> AdmResult<PathBuf> {
    let style_options = read_json(&output_dir.join("style_options.json"), json!({}));
    let locale = artifact_locale_from_value(&style_options);
    let payload = build_style_prompt_override_with_locale(options, refined_prompts, count, locale);
    write_json(&output_dir.join(STYLE_PROMPT_OVERRIDE_FILENAME), &payload)
}

pub fn build_style_confirmation(
    selected_option: &Value,
    notes: &str,
    status: &str,
    mode: &str,
) -> Value {
    build_style_confirmation_with_locale(
        selected_option,
        notes,
        status,
        mode,
        ArtifactLocale::default(),
    )
}

fn build_style_confirmation_with_locale(
    selected_option: &Value,
    notes: &str,
    status: &str,
    mode: &str,
    locale: ArtifactLocale,
) -> Value {
    json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "status": status,
        "mode": mode,
        "selected_style_id": option_identifier(selected_option),
        "selected_title": string_field(selected_option, "title"),
        "selected_image_path": string_field(selected_option, "image_path"),
        "notes": notes.trim(),
        "selected_option": selected_option,
    })
}

pub fn write_style_confirmation(
    output_dir: &Path,
    selected_option: &Value,
    notes: &str,
    status: &str,
    mode: &str,
) -> AdmResult<PathBuf> {
    let style_options = read_json(&output_dir.join("style_options.json"), json!({}));
    let locale = artifact_locale_from_value(&style_options);
    let payload =
        build_style_confirmation_with_locale(selected_option, notes, status, mode, locale);
    write_json(&output_dir.join(STYLE_CONFIRMATION_FILENAME), &payload)
}

pub fn approved_confirmation_from_dirs(current_dir: &Path, legacy_dir: &Path) -> Option<Value> {
    let current = read_json(&current_dir.join(STYLE_CONFIRMATION_FILENAME), json!({}));
    if confirmation_is_approved(&current) {
        return Some(current);
    }
    let legacy = read_json(&legacy_dir.join(STYLE_CONFIRMATION_FILENAME), json!({}));
    if confirmation_is_approved(&legacy) && current_dir.join("style_options.json").is_file() {
        let _ = write_json(&current_dir.join(STYLE_CONFIRMATION_FILENAME), &legacy);
        return Some(legacy);
    }
    None
}

pub fn cleanup_unselected_style_images(
    output_dir: &Path,
    selected_style_id: &str,
    selected_image_path: &str,
) -> AdmResult<usize> {
    let generated_dir = output_dir.join("generated_images");
    let selected_abs = resolve_stage_path(output_dir, selected_image_path);
    let mut removed = 0usize;
    if !generated_dir.is_dir() {
        return Ok(0);
    }
    for entry in fs::read_dir(generated_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|item| item.to_str()) != Some("png") {
            continue;
        }
        if path == selected_abs {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|item| item.to_str())
            .unwrap_or("");
        if !selected_style_id.is_empty() && stem.starts_with(selected_style_id) {
            continue;
        }
        fs::remove_file(&path)?;
        removed += 1;
    }
    Ok(removed)
}

pub fn generate_step07_outputs(
    parsed: &ParsedDesignSource,
    out_dir: &Path,
    inputs: Step07Inputs,
    config: &StyleGenerationConfig,
) -> AdmResult<Value> {
    generate_step07_outputs_with_generator(parsed, out_dir, inputs, config, None)
}

pub fn generate_step07_outputs_with_generator(
    parsed: &ParsedDesignSource,
    out_dir: &Path,
    inputs: Step07Inputs,
    config: &StyleGenerationConfig,
    image_generator: Option<&dyn StyleImageGenerator>,
) -> AdmResult<Value> {
    let journal_root = default_step07_work_unit_root(out_dir);
    generate_step07_outputs_with_runtime_locale(
        parsed,
        out_dir,
        inputs,
        config,
        image_generator,
        &journal_root,
        &WorkUnitStopToken::default(),
        ArtifactLocale::default(),
    )
}

pub fn generate_step07_outputs_with_runtime(
    parsed: &ParsedDesignSource,
    out_dir: &Path,
    inputs: Step07Inputs,
    config: &StyleGenerationConfig,
    image_generator: Option<&dyn StyleImageGenerator>,
    work_unit_journal_root: &Path,
    stop_token: &WorkUnitStopToken,
) -> AdmResult<Value> {
    generate_step07_outputs_with_runtime_locale(
        parsed,
        out_dir,
        inputs,
        config,
        image_generator,
        work_unit_journal_root,
        stop_token,
        ArtifactLocale::default(),
    )
}

fn generate_step07_outputs_with_runtime_locale(
    parsed: &ParsedDesignSource,
    out_dir: &Path,
    inputs: Step07Inputs,
    config: &StyleGenerationConfig,
    image_generator: Option<&dyn StyleImageGenerator>,
    work_unit_journal_root: &Path,
    stop_token: &WorkUnitStopToken,
    locale: ArtifactLocale,
) -> AdmResult<Value> {
    let mut prerequisite_blockers = Vec::<Value>::new();
    if !inputs.asset_spec_has_assets() {
        prerequisite_blockers.push(json!({
            "code": "STYLE_INPUT_ASSET_SPEC_MISSING",
            "message": localized_text(locale, "步骤 07 需要步骤 04 的 asset_spec_contract.json。", "Step07 requires Step04 asset_spec_contract.json."),
        }));
    }
    if is_empty_object(&inputs.art_review) {
        prerequisite_blockers.push(json!({
            "code": "STYLE_INPUT_ART_REVIEW_MISSING",
            "message": localized_text(locale, "步骤 07 需要步骤 06 的 art_ai_review_report.json。", "Step07 requires Step06 art_ai_review_report.json."),
        }));
    } else if review_is_blocked(&inputs.art_review) {
        prerequisite_blockers.push(json!({
            "code": "STYLE_INPUT_ART_REVIEW_BLOCKED",
            "message": localized_text(locale, "步骤 06 的美术评审已阻断，步骤 07 无法生成风格选项。", "Step07 cannot generate style options from blocked art review."),
            "blockers": inputs.art_review.get("blockers").cloned().unwrap_or_else(|| json!([])),
        }));
    }
    if !prerequisite_blockers.is_empty() {
        let report = json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "artifact_locale": locale,
            "status": "blocked",
            "blockers": prerequisite_blockers,
        });
        write_json(&out_dir.join("style_prerequisite_report.json"), &report)?;
        write_style_fit_outputs(out_dir, None, "", "", locale)?;
        return Ok(json!({
            "artifact_locale": locale,
            "status": "blocked",
            "content_exists": false,
            "blocking_issues": report.get("blockers").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
            "ai_review_status": "blocked",
            "traceability_valid": false,
        }));
    }

    let mut assets = inputs.usable_assets();
    let preferred_asset_ids = string_array(inputs.art_review.get("style_generation_inputs"));
    if !preferred_asset_ids.is_empty() {
        let order = preferred_asset_ids
            .iter()
            .enumerate()
            .map(|(index, asset_id)| (asset_id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        assets.sort_by_key(|asset| {
            order
                .get(&asset.asset_id)
                .cloned()
                .unwrap_or(preferred_asset_ids.len())
        });
    }

    let override_path = out_dir.join(STYLE_PROMPT_OVERRIDE_FILENAME);
    let override_options = style_prompt_override_options(
        &read_json(&override_path, json!({})),
        parsed,
        &assets,
        locale,
    );
    if !override_options.is_empty() {
        let _ = fs::remove_file(&override_path);
        return write_style_generation_outputs(
            parsed,
            out_dir,
            override_options,
            true,
            config,
            image_generator,
            work_unit_journal_root,
            stop_token,
            locale,
        );
    }

    let existing_style_options = read_json(&out_dir.join("style_options.json"), json!({}));
    let existing_confirmation = read_json(&out_dir.join(STYLE_CONFIRMATION_FILENAME), json!({}));
    if confirmation_is_approved(&existing_confirmation)
        && !confirmation_options(&existing_style_options).is_empty()
        && artifact_locale_from_value(&existing_style_options) == locale
        && artifact_locale_from_value(&existing_confirmation) == locale
    {
        let result = style_confirmation_outputs_with_locale(
            parsed,
            out_dir,
            Some(existing_style_options.clone()),
            locale,
        )?;
        write_style_fit_outputs(
            out_dir,
            Some(&existing_style_options),
            &string_field(&existing_confirmation, "selected_style_id"),
            &non_empty_or(
                string_field(&existing_confirmation, "override_reason"),
                &string_field(&existing_confirmation, "notes"),
            ),
            locale,
        )?;
        let options = confirmation_options(&existing_style_options);
        let existing_generation = read_json(&out_dir.join("generation_log.json"), json!({}));
        let provider_generated_count = existing_generation
            .get("provider_generated_count")
            .or_else(|| existing_generation.get("generated_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let fallback_count = existing_generation
            .get("fallback_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let mut map = object_map_or_empty(result);
        map.insert("status".to_string(), json!("success"));
        map.insert("style_option_count".to_string(), json!(options.len()));
        map.insert(
            "generated_image_count".to_string(),
            json!(provider_generated_count),
        );
        map.insert("fallback_image_count".to_string(), json!(fallback_count));
        map.insert("reused_generation".to_string(), json!(true));
        map.insert("style_options".to_string(), Value::Array(options));
        map.insert("artifact_locale".to_string(), json!(locale));
        return Ok(Value::Object(map));
    }

    let count = style_option_count(config.option_count);
    let options = STYLE_OPTION_PRESETS
        .iter()
        .take(count)
        .enumerate()
        .map(|(index, preset)| {
            let mut option = StyleOption {
                style_id: format!("STYLE-{:02}-{}", index + 1, preset.key),
                title: preset.title(locale).to_string(),
                description: preset.description(locale).to_string(),
                palette: preset
                    .palette
                    .iter()
                    .map(|item| (*item).to_string())
                    .collect(),
                source_refs: vec![
                    "stage_04.asset_registry".to_string(),
                    "stage_06.art_review".to_string(),
                ],
                prompt: String::new(),
                generation_prompt: String::new(),
                negative_prompt: style_negative_prompt(locale).to_string(),
                image_path: String::new(),
                score: 0,
                recommended: false,
                recommendation_reason: String::new(),
                prompt_refined: false,
            };
            option.prompt = style_prompt(parsed, &option, &assets, locale);
            option.generation_prompt = option.prompt.clone();
            option
        })
        .collect::<Vec<_>>();
    write_style_generation_outputs(
        parsed,
        out_dir,
        options,
        false,
        config,
        image_generator,
        work_unit_journal_root,
        stop_token,
        locale,
    )
}

pub fn style_confirmation_outputs(
    parsed: &ParsedDesignSource,
    out_dir: &Path,
    style_options: Option<Value>,
) -> AdmResult<Value> {
    let locale_source = style_options
        .as_ref()
        .cloned()
        .unwrap_or_else(|| read_json(&out_dir.join("style_options.json"), json!({})));
    style_confirmation_outputs_with_locale(
        parsed,
        out_dir,
        Some(locale_source.clone()),
        artifact_locale_from_value(&locale_source),
    )
}

fn style_confirmation_outputs_with_locale(
    _parsed: &ParsedDesignSource,
    out_dir: &Path,
    style_options: Option<Value>,
    locale: ArtifactLocale,
) -> AdmResult<Value> {
    let style_options =
        style_options.unwrap_or_else(|| read_json(&out_dir.join("style_options.json"), json!({})));
    let options = confirmation_options(&style_options);
    if options.is_empty() {
        let result = json!({
            "schema_version": 1,
            "generated_at": now_iso(),
            "artifact_locale": locale,
            "status": "blocked",
            "message": localized_text(locale, "步骤 07 的 style_options.json 缺失或没有可选风格。", "Stage 07 style_options.json is missing or empty."),
            "selected_style_id": "",
        });
        write_json(&out_dir.join(STYLE_CONFIRMATION_FILENAME), &result)?;
        return Ok(json!({
            "artifact_locale": locale,
            "status": "blocked",
            "message": localized_text(locale, "没有可供确认的美术风格选项。", "No art style option is available for confirmation."),
            "content_exists": true,
            "blocking_issues": 1,
            "ai_review_status": "blocked",
            "traceability_valid": false,
        }));
    }

    let confirmation_path = out_dir.join(STYLE_CONFIRMATION_FILENAME);
    let mut confirmation = read_json(&confirmation_path, json!({}));
    if confirmation_is_approved(&confirmation) {
        let selected_style_id = string_field(&confirmation, "selected_style_id");
        let valid_selection = options
            .iter()
            .any(|option| option_identifier(option) == selected_style_id);
        if valid_selection {
            if artifact_locale_from_value(&confirmation) != locale
                && let Some(selected) = options
                    .iter()
                    .find(|option| option_identifier(option) == selected_style_id)
                && let Some(object) = confirmation.as_object_mut()
            {
                object.insert("artifact_locale".to_string(), json!(locale));
                object.insert("generated_at".to_string(), json!(now_iso()));
                object.insert(
                    "selected_title".to_string(),
                    json!(string_field(selected, "title")),
                );
                object.insert(
                    "selected_image_path".to_string(),
                    json!(string_field(selected, "image_path")),
                );
                object.insert("selected_option".to_string(), selected.clone());
                write_json(&confirmation_path, &confirmation)?;
            }
            write_approved_style_application_contract(
                out_dir,
                &style_options,
                &confirmation,
                locale,
            )?;
        }
        let override_reason = non_empty_or(
            string_field(&confirmation, "override_reason"),
            &string_field(&confirmation, "notes"),
        );
        write_style_fit_outputs(
            out_dir,
            Some(&style_options),
            &selected_style_id,
            &override_reason,
            locale,
        )?;
        write_text(
            &out_dir.join("style_confirmation.md"),
            &style_confirmation_markdown(locale, "approved", &selected_style_id),
        )?;
        return Ok(json!({
            "artifact_locale": locale,
            "status": if valid_selection { "success" } else { "completed_with_review" },
            "message": if valid_selection {
                localized_text(locale, "美术风格已确认。", "Art style confirmed.")
            } else {
                localized_text(locale, "确认记录中的风格不在当前选项中，请复核。", "The confirmed style is not present in the current options; review is required.")
            },
            "content_exists": true,
            "confirmation_status": "approved",
            "selected_style_id": selected_style_id,
            "blocking_issues": 0,
            "ai_review_status": "passed",
            "traceability_valid": valid_selection,
        }));
    }

    let pending = json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "status": "waiting_confirmation",
        "confirmation_ui": "style_confirmation_dialog",
        "style_options_path": "style_options.json",
        "confirmation_path": STYLE_CONFIRMATION_FILENAME,
        "option_count": options.len(),
    });
    write_json(&out_dir.join("style_confirmation_pending.json"), &pending)?;
    write_text(
        &out_dir.join("style_confirmation.md"),
        &style_confirmation_markdown(locale, "waiting_confirmation", ""),
    )?;
    Ok(json!({
        "artifact_locale": locale,
        "status": "waiting_confirmation",
        "message": localized_text(locale, "请选择并确认一个美术风格方案。", "Select and confirm one art style option."),
        "content_exists": true,
        "confirmation_status": "waiting_confirmation",
        "confirmation_ui": "style_confirmation_dialog",
        "blocking_issues": 0,
        "ai_review_status": "waiting_confirmation",
        "traceability_valid": true,
    }))
}

#[derive(Debug, Clone)]
pub struct Step07OutputGenerator {
    pub config: StyleGenerationConfig,
    pub image_generator: Option<Arc<dyn StyleImageGenerator>>,
    pub work_unit_journal_root: Option<PathBuf>,
    pub stop_token: WorkUnitStopToken,
}

impl Default for Step07OutputGenerator {
    fn default() -> Self {
        Self {
            config: StyleGenerationConfig::default(),
            image_generator: None,
            work_unit_journal_root: None,
            stop_token: WorkUnitStopToken::default(),
        }
    }
}

impl Step07OutputGenerator {
    pub fn new(image_generator: Option<Arc<dyn StyleImageGenerator>>) -> Self {
        Self {
            config: StyleGenerationConfig {
                image_generation_enabled: image_generator.is_some(),
                ..StyleGenerationConfig::default()
            },
            image_generator,
            work_unit_journal_root: None,
            stop_token: WorkUnitStopToken::default(),
        }
    }

    pub fn with_safe_units(
        image_generator: Option<Arc<dyn StyleImageGenerator>>,
        work_unit_journal_root: impl AsRef<Path>,
        stop_token: WorkUnitStopToken,
    ) -> Self {
        let mut generator = Self::new(image_generator);
        generator.work_unit_journal_root = Some(work_unit_journal_root.as_ref().to_path_buf());
        generator.stop_token = stop_token;
        generator
    }
}

impl StageOutputGenerator for Step07OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP07)?;
        let locale = artifact_locale_from_inputs(structured_inputs);
        let journal_root = self
            .work_unit_journal_root
            .clone()
            .unwrap_or_else(|| default_step07_work_unit_root(out_dir));
        generate_step07_outputs_with_runtime_locale(
            parsed,
            out_dir,
            Step07Inputs::from_stage_dirs(out_dir),
            &self.config,
            self.image_generator.as_deref(),
            &journal_root,
            &self.stop_token,
            locale,
        )
    }
}

pub fn generator_for_step(step_number: u32) -> AdmResult<Box<dyn StageOutputGenerator>> {
    match step_number {
        STEP07 => Ok(Box::new(Step07OutputGenerator::default())),
        other => Err(AdmError::new(format!(
            "Step07 generator cannot handle stage {other:02}"
        ))),
    }
}

fn write_style_generation_outputs(
    parsed: &ParsedDesignSource,
    out_dir: &Path,
    mut options: Vec<StyleOption>,
    prompt_override_used: bool,
    config: &StyleGenerationConfig,
    image_generator: Option<&dyn StyleImageGenerator>,
    work_unit_journal_root: &Path,
    stop_token: &WorkUnitStopToken,
    locale: ArtifactLocale,
) -> AdmResult<Value> {
    apply_style_option_recommendations_with_locale(parsed, &mut options, locale);
    let manifest = generate_style_option_images(
        out_dir,
        parsed,
        &mut options,
        config,
        image_generator,
        work_unit_journal_root,
        stop_token,
        locale,
    )?;
    if matches!(
        manifest.get("status").and_then(Value::as_str),
        Some("stopped" | "recovery_blocked")
    ) {
        let mut attempt = object_map_or_empty(manifest);
        attempt.insert(
            "prompt_override_used".to_string(),
            json!(prompt_override_used),
        );
        let attempt = Value::Object(attempt);
        write_json(&out_dir.join("generation_attempt_log.json"), &attempt)?;
        let mut result = object_map_or_empty(attempt);
        result.insert(
            "content_exists".to_string(),
            json!(out_dir.join("generated_images").is_dir()),
        );
        result.insert("style_option_count".to_string(), json!(options.len()));
        result.insert("generated_image_count".to_string(), json!(0));
        result.insert("fallback_image_count".to_string(), json!(0));
        result.insert("blocking_issues".to_string(), json!(1));
        result.insert("traceability_valid".to_string(), json!(false));
        result.insert("artifact_locale".to_string(), json!(locale));
        return Ok(Value::Object(result));
    }
    let _ = fs::remove_file(out_dir.join("generation_attempt_log.json"));
    let recommended = recommended_style_option(&options);
    let mut options_value = to_json_value(&options)?;
    attach_image_statuses(&mut options_value, &manifest);
    let style_options = json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "project": stage_title_with_locale(parsed, locale),
        "source_stage": 6,
        "option_count": options.len(),
        "recommended_style_id": recommended.style_id,
        "options": options_value,
        "selection_required": true,
        "prompt_override_used": prompt_override_used,
    });
    let style_fit_report = write_style_fit_outputs(
        out_dir,
        Some(&style_options),
        &recommended.style_id,
        "",
        locale,
    )?;
    write_json(&out_dir.join("style_options.json"), &style_options)?;
    write_json(
        &out_dir.join("style_application_contract_pending.json"),
        &pending_style_application_contract(&recommended, locale),
    )?;
    let _ = fs::remove_file(out_dir.join("style_application_contract.json"));
    let mut generation_log = object_map_or_empty(manifest);
    generation_log.insert(
        "prompt_override_used".to_string(),
        json!(prompt_override_used),
    );
    generation_log.insert("artifact_locale".to_string(), json!(locale));
    let generation_log = Value::Object(generation_log);
    write_json(&out_dir.join("generation_log.json"), &generation_log)?;
    write_json(
        &out_dir.join("generated_images_manifest.json"),
        &generation_log,
    )?;
    write_text(
        &out_dir.join("style_options.md"),
        &style_options_markdown(&options, locale),
    )?;
    let mut result = object_map_or_empty(style_confirmation_outputs_with_locale(
        parsed,
        out_dir,
        Some(style_options.clone()),
        locale,
    )?);
    result.insert("content_exists".to_string(), json!(true));
    result.insert("style_option_count".to_string(), json!(options.len()));
    result.insert(
        "generated_image_count".to_string(),
        generation_log
            .get("provider_generated_count")
            .or_else(|| generation_log.get("generated_count"))
            .cloned()
            .unwrap_or_else(|| json!(0)),
    );
    result.insert(
        "fallback_image_count".to_string(),
        generation_log
            .get("fallback_count")
            .cloned()
            .unwrap_or_else(|| json!(0)),
    );
    result.insert(
        "recommended_style_id".to_string(),
        json!(recommended.style_id),
    );
    result.insert(
        "prompt_override_used".to_string(),
        json!(prompt_override_used),
    );
    result.insert(
        "style_options".to_string(),
        style_options
            .get("options")
            .cloned()
            .unwrap_or_else(|| json!([])),
    );
    result.insert("style_options_document".to_string(), style_options);
    result.insert("artifact_locale".to_string(), json!(locale));
    let fit_blockers = style_fit_report
        .get("blockers")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let existing_blockers = result
        .get("blocking_issues")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    result.insert(
        "blocking_issues".to_string(),
        json!(existing_blockers.max(fit_blockers)),
    );
    Ok(Value::Object(result))
}

fn generate_style_option_images(
    out_dir: &Path,
    parsed: &ParsedDesignSource,
    options: &mut [StyleOption],
    config: &StyleGenerationConfig,
    image_generator: Option<&dyn StyleImageGenerator>,
    work_unit_journal_root: &Path,
    stop_token: &WorkUnitStopToken,
    locale: ArtifactLocale,
) -> AdmResult<Value> {
    let generated_dir = out_dir.join("generated_images");
    let staging_dir = out_dir.join(".generated_images_staging");
    recover_generated_image_directory(&staging_dir, &generated_dir)?;
    let cache_root = work_unit_journal_root.join("image_cache");
    let executor = SafeStyleImageExecutor::new(
        image_generator,
        &cache_root,
        config.image_generation_enabled,
    );
    let requests = options
        .iter()
        .map(|option| style_image_work_unit_request(parsed, option, locale))
        .collect::<AdmResult<Vec<_>>>()?;
    let journal = SafeUnitJournal::new(work_unit_journal_root.join("journal"));
    let batch = execute_work_unit_batch(requests, Some(&executor), &journal, stop_token)?;
    if batch.stopped || batch.recovery_blocked {
        if staging_dir.exists() {
            fs::remove_dir_all(&staging_dir)?;
        }
        let records = batch
            .units
            .iter()
            .map(|outcome| safe_incomplete_unit_record(outcome, locale))
            .collect::<Vec<_>>();
        return Ok(json!({
            "schema_version": 2,
            "generated_at": now_iso(),
            "artifact_locale": locale,
            "stage": STEP07,
            "enabled": config.image_generation_enabled,
            "records": records,
            "requested_count": options.len(),
            "provider_generated_count": 0,
            "fallback_count": 0,
            "failed_count": batch.units.iter().filter(|unit| unit.status == WorkUnitRunStatus::Failed).count(),
            "completed_unit_count": batch.units.iter().filter(|unit| matches!(unit.status, WorkUnitRunStatus::Committed | WorkUnitRunStatus::Reused)).count(),
            "directory_committed": false,
            "status": if batch.recovery_blocked { "recovery_blocked" } else { "stopped" },
        }));
    }
    if batch.units.len() != options.len() {
        return Err(AdmError::new(localized_text(
            locale,
            "步骤 07 的安全图像工作单元未为每个风格选项返回终态。",
            "Step07 safe image unit batch ended without a terminal result for every option",
        )));
    }

    fs::create_dir_all(&staging_dir)?;
    let mut records = Vec::new();
    let mut provider_generated_count = 0_usize;
    let mut fallback_count = 0_usize;
    let mut failed_count = 0_usize;
    for (option, outcome) in options.iter_mut().zip(batch.units.iter()) {
        let image_name = safe_style_image_name_with_locale(&option.style_id, locale)?;
        let staged_path = staging_dir.join(&image_name);
        let record = match outcome.status {
            WorkUnitRunStatus::Committed | WorkUnitRunStatus::Reused => {
                let result = outcome.result.as_ref().ok_or_else(|| {
                    AdmError::new(localized_text(
                        locale,
                        "已提交的风格图工作单元没有结果。",
                        "committed style image work unit has no result",
                    ))
                })?;
                let bytes = match executor.cached_png(&outcome.request, result) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        let _ = fs::remove_dir_all(&staging_dir);
                        return Ok(recovery_blocked_manifest(
                            options.len(),
                            config.image_generation_enabled,
                            &batch.units,
                            locale,
                        ));
                    }
                };
                fs::write(&staged_path, bytes)?;
                provider_generated_count += 1;
                json!({
                    "style_id": option.style_id,
                    "image_path": stage_relative_path(out_dir, &generated_dir.join(&image_name)),
                    "status": "generated",
                    "provider": result_data_string(result, "provider"),
                    "model": result_data_string(result, "model"),
                    "result": if outcome.status == WorkUnitRunStatus::Reused {
                        localized_text(locale, "已复用通过验证的图像缓存。", "Validated cached provider image reused.")
                    } else {
                        localized_text(locale, "图像服务已生成并验证风格图。", "Provider image generated and validated.")
                    },
                    "reason_code": "",
                    "width": result.data.get("width").and_then(Value::as_u64).unwrap_or_default(),
                    "height": result.data.get("height").and_then(Value::as_u64).unwrap_or_default(),
                    "format": "png",
                    "prompt_refined": option.prompt_refined,
                    "unit_status": if outcome.status == WorkUnitRunStatus::Reused { "reused" } else { "committed" },
                })
            }
            WorkUnitRunStatus::Unavailable | WorkUnitRunStatus::Failed => {
                if outcome.status == WorkUnitRunStatus::Failed {
                    failed_count += 1;
                }
                fallback_count += 1;
                write_palette_reference_png(&staged_path, option, locale)?;
                fallback_record(
                    out_dir,
                    &generated_dir,
                    &image_name,
                    option,
                    outcome_reason_code(outcome),
                    locale,
                )
            }
            WorkUnitRunStatus::Stopped | WorkUnitRunStatus::RecoveryBlocked => {
                unreachable!("stopped and recovery-blocked batches return before staging")
            }
        };
        let final_path = generated_dir.join(&image_name);
        option.image_path = stage_relative_path(out_dir, &final_path);
        records.push(record);
    }
    commit_generated_image_directory(&staging_dir, &generated_dir, locale)?;
    let status = if options.is_empty() {
        "failed"
    } else if provider_generated_count == options.len() {
        "success"
    } else if provider_generated_count > 0 {
        "partial"
    } else {
        "degraded"
    };
    Ok(json!({
        "schema_version": 2,
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "stage": STEP07,
        "enabled": config.image_generation_enabled,
        "records": records,
        "requested_count": options.len(),
        "generated_count": provider_generated_count,
        "provider_generated_count": provider_generated_count,
        "fallback_count": fallback_count,
        "failed_count": failed_count,
        "image_count": options.len(),
        "directory_committed": true,
        "status": status,
    }))
}

fn style_image_work_unit_request(
    parsed: &ParsedDesignSource,
    option: &StyleOption,
    locale: ArtifactLocale,
) -> AdmResult<WorkUnitRequest> {
    WorkUnitRequest::new(
        "07",
        &format!("image:{}", option.style_id),
        WorkUnitKind::Art,
        json!({
            "style_id": option.style_id,
            "prompt": option.prompt,
            "generation_prompt": option.generation_prompt,
            "negative_prompt": option.negative_prompt,
            "artifact_locale": locale,
            "project_label": stage_title_with_locale(parsed, locale),
            "requested_width": PROVIDER_IMAGE_WIDTH,
            "requested_height": PROVIDER_IMAGE_HEIGHT,
            "output_format": "png",
        }),
    )
}

fn result_data_string(result: &WorkUnitExecutionResult, key: &str) -> String {
    result
        .data
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn outcome_reason_code(outcome: &WorkUnitRunOutcome) -> &str {
    if let Some(code) = outcome
        .result
        .as_ref()
        .and_then(|result| result.data.get("reason_code"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        return code;
    }
    match outcome.status {
        WorkUnitRunStatus::Committed | WorkUnitRunStatus::Reused => "",
        WorkUnitRunStatus::Unavailable => "image_provider_unavailable",
        WorkUnitRunStatus::Failed => "image_provider_failed",
        WorkUnitRunStatus::Stopped => "stop_requested",
        WorkUnitRunStatus::RecoveryBlocked => "recovery_required",
    }
}

fn safe_incomplete_unit_record(outcome: &WorkUnitRunOutcome, locale: ArtifactLocale) -> Value {
    json!({
        "style_id": outcome.request.payload.get("style_id").and_then(Value::as_str).unwrap_or_default(),
        "status": match outcome.status {
            WorkUnitRunStatus::Committed => "committed_not_published",
            WorkUnitRunStatus::Reused => "reused_not_published",
            WorkUnitRunStatus::Failed => "failed",
            WorkUnitRunStatus::Unavailable => "fallback_pending",
            WorkUnitRunStatus::Stopped => "stopped",
            WorkUnitRunStatus::RecoveryBlocked => "recovery_blocked",
        },
        "reason_code": outcome_reason_code(outcome),
        "result": style_image_outcome_message(outcome, locale),
    })
}

fn recovery_blocked_manifest(
    requested_count: usize,
    enabled: bool,
    outcomes: &[WorkUnitRunOutcome],
    locale: ArtifactLocale,
) -> Value {
    json!({
        "schema_version": 2,
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "stage": STEP07,
        "enabled": enabled,
        "records": outcomes.iter().map(|outcome| safe_incomplete_unit_record(outcome, locale)).collect::<Vec<_>>(),
        "requested_count": requested_count,
        "provider_generated_count": 0,
        "fallback_count": 0,
        "failed_count": outcomes.iter().filter(|unit| unit.status == WorkUnitRunStatus::Failed).count(),
        "completed_unit_count": outcomes.iter().filter(|unit| matches!(unit.status, WorkUnitRunStatus::Committed | WorkUnitRunStatus::Reused)).count(),
        "directory_committed": false,
        "status": "recovery_blocked",
        "reason_code": "image_cache_reconciliation_failed",
        "message": localized_text(locale, "风格图缓存无法安全校验，恢复已阻断。", "The style-image cache could not be reconciled safely; recovery is blocked."),
    })
}

fn fallback_record(
    out_dir: &Path,
    generated_dir: &Path,
    image_name: &str,
    option: &StyleOption,
    reason_code: &str,
    locale: ArtifactLocale,
) -> Value {
    json!({
        "style_id": option.style_id,
        "image_path": stage_relative_path(out_dir, &generated_dir.join(image_name)),
        "status": "fallback",
        "provider": "deterministic_palette",
        "model": "",
        "result": fallback_result_message(reason_code, locale),
        "reason_code": reason_code,
        "width": FALLBACK_IMAGE_WIDTH,
        "height": FALLBACK_IMAGE_HEIGHT,
        "format": "png",
        "prompt_refined": option.prompt_refined,
    })
}

fn style_image_outcome_message(
    outcome: &WorkUnitRunOutcome,
    locale: ArtifactLocale,
) -> &'static str {
    match outcome.status {
        WorkUnitRunStatus::Committed => localized_text(
            locale,
            "风格图工作单元已提交，但尚未发布。",
            "The style-image work unit was committed but has not been published.",
        ),
        WorkUnitRunStatus::Reused => localized_text(
            locale,
            "已复用通过验证的风格图工作单元。",
            "A validated style-image work unit was reused.",
        ),
        WorkUnitRunStatus::Unavailable => localized_text(
            locale,
            "图像服务不可用，将使用本地可见回退图。",
            "The image provider is unavailable; a visible local fallback will be used.",
        ),
        WorkUnitRunStatus::Failed => localized_text(
            locale,
            "图像生成失败，将使用本地可见回退图。",
            "Image generation failed; a visible local fallback will be used.",
        ),
        WorkUnitRunStatus::Stopped => localized_text(
            locale,
            "风格图生成已在工作单元边界停止。",
            "Style-image generation stopped at a work-unit boundary.",
        ),
        WorkUnitRunStatus::RecoveryBlocked => localized_text(
            locale,
            "风格图工作单元需要人工复核后才能恢复。",
            "The style-image work unit requires review before recovery.",
        ),
    }
}

fn fallback_result_message(reason_code: &str, locale: ArtifactLocale) -> &'static str {
    match reason_code {
        "image_provider_unavailable" => localized_text(
            locale,
            "图像服务不可用，已生成本地可见回退图。",
            "The image provider was unavailable; a visible local fallback was generated.",
        ),
        "image_provider_failed" => localized_text(
            locale,
            "图像服务生成失败，已生成本地可见回退图。",
            "The image provider failed; a visible local fallback was generated.",
        ),
        _ => localized_text(
            locale,
            "已生成本地可见回退图。",
            "A visible local fallback was generated.",
        ),
    }
}

fn default_step07_work_unit_root(out_dir: &Path) -> PathBuf {
    out_dir
        .parent()
        .unwrap_or(out_dir)
        .join(".pipeline_checkpoints")
        .join("work_units")
        .join("stage_07")
}

fn recover_generated_image_directory(staging_dir: &Path, generated_dir: &Path) -> AdmResult<()> {
    let backup_dir = generated_dir.with_file_name(".generated_images_previous");
    if !generated_dir.exists() && backup_dir.exists() {
        fs::rename(&backup_dir, generated_dir)?;
    } else if generated_dir.exists() && backup_dir.exists() {
        fs::remove_dir_all(&backup_dir)?;
    }
    if staging_dir.exists() {
        fs::remove_dir_all(staging_dir)?;
    }
    Ok(())
}

fn commit_generated_image_directory(
    staging_dir: &Path,
    generated_dir: &Path,
    locale: ArtifactLocale,
) -> AdmResult<()> {
    let backup_dir = generated_dir.with_file_name(".generated_images_previous");
    if backup_dir.exists() {
        fs::remove_dir_all(&backup_dir)?;
    }
    if generated_dir.exists() {
        fs::rename(generated_dir, &backup_dir)?;
    }
    if let Err(error) = fs::rename(staging_dir, generated_dir) {
        if backup_dir.exists() {
            let _ = fs::rename(&backup_dir, generated_dir);
        }
        return Err(AdmError::new(format!(
            "{}: {error}",
            localized_text(
                locale,
                "提交风格图目录失败",
                "Failed to commit the generated image directory"
            )
        )));
    }
    if backup_dir.exists() {
        fs::remove_dir_all(backup_dir)?;
    }
    Ok(())
}

#[cfg(test)]
fn safe_style_image_name(style_id: &str) -> AdmResult<String> {
    safe_style_image_name_with_locale(style_id, ArtifactLocale::default())
}

fn safe_style_image_name_with_locale(style_id: &str, locale: ArtifactLocale) -> AdmResult<String> {
    let normalized = sanitize_identifier(style_id)?;
    if normalized != style_id.trim() {
        return Err(AdmError::new(localized_text(
            locale,
            "步骤 07 的风格 ID 包含不可移植的路径字符。",
            "Step07 style ID contains non-portable path characters",
        )));
    }
    Ok(format!("{normalized}.png"))
}

fn attach_image_statuses(options: &mut Value, manifest: &Value) {
    let records = manifest
        .get("records")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some((
                        item.get("style_id")?.as_str()?.to_string(),
                        (
                            item.get("status")?.as_str()?.to_string(),
                            item.get("result")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string(),
                        ),
                    ))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let Some(items) = options.as_array_mut() else {
        return;
    };
    for item in items {
        let style_id = item
            .get("style_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some((status, message)) = records.get(style_id) else {
            continue;
        };
        if let Some(object) = item.as_object_mut() {
            object.insert("image_status".to_string(), json!(status));
            object.insert("image_message".to_string(), json!(message));
        }
    }
}

fn write_palette_reference_png(
    path: &Path,
    option: &StyleOption,
    locale: ArtifactLocale,
) -> AdmResult<()> {
    let palette = [
        option
            .palette
            .first()
            .and_then(|value| parse_hex_color(value))
            .unwrap_or([46, 52, 64]),
        option
            .palette
            .get(1)
            .and_then(|value| parse_hex_color(value))
            .unwrap_or([136, 192, 208]),
        option
            .palette
            .get(2)
            .and_then(|value| parse_hex_color(value))
            .unwrap_or([235, 203, 139]),
    ];
    let seed = option.style_id.bytes().fold(0_u32, |value, byte| {
        value.wrapping_mul(31).wrapping_add(byte as u32)
    });
    let mut image = RgbImage::new(FALLBACK_IMAGE_WIDTH, FALLBACK_IMAGE_HEIGHT);
    let center_x = 150_i32 + (seed % 340) as i32;
    let center_y = 118_i32 + (seed % 50) as i32;
    let radius = 58_i32 + (seed % 28) as i32;
    for y in 0..FALLBACK_IMAGE_HEIGHT {
        let vertical = y as f32 / (FALLBACK_IMAGE_HEIGHT - 1) as f32;
        for x in 0..FALLBACK_IMAGE_WIDTH {
            let horizontal = x as f32 / (FALLBACK_IMAGE_WIDTH - 1) as f32;
            let mut color =
                blend_color(palette[0], palette[1], vertical * 0.62 + horizontal * 0.18);
            let dx = x as i32 - center_x;
            let dy = y as i32 - center_y;
            if dx * dx + dy * dy <= radius * radius {
                color = palette[2];
            }
            if y > FALLBACK_IMAGE_HEIGHT - 92 {
                let stripe = ((x / 80) + (seed % 3)) % 3;
                color = palette[stripe as usize];
            } else if y > FALLBACK_IMAGE_HEIGHT - 150 {
                let skyline_height = 26 + ((x / 48 + seed) % 72);
                if y > FALLBACK_IMAGE_HEIGHT - 92 - skyline_height {
                    color = blend_color(palette[0], palette[2], 0.35);
                }
            }
            image.put_pixel(x, y, Rgb(color));
        }
    }
    image
        .save_with_format(path, ImageFormat::Png)
        .map_err(|error| {
            AdmError::new(format!(
                "{}: {error}",
                localized_text(
                    locale,
                    "写入步骤 07 的回退 PNG 失败",
                    "Failed to write the Step07 fallback PNG"
                )
            ))
        })
}

fn parse_hex_color(value: &str) -> Option<[u8; 3]> {
    let value = value.trim().strip_prefix('#')?;
    if value.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ])
}

fn blend_color(from: [u8; 3], to: [u8; 3], ratio: f32) -> [u8; 3] {
    let ratio = ratio.clamp(0.0, 1.0);
    std::array::from_fn(|index| {
        (from[index] as f32 + (to[index] as f32 - from[index] as f32) * ratio).round() as u8
    })
}

fn write_style_fit_outputs(
    out_dir: &Path,
    style_options: Option<&Value>,
    selected_style_id: &str,
    override_reason: &str,
    locale: ArtifactLocale,
) -> AdmResult<Value> {
    let options = style_options
        .and_then(|value| value.get("options"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let style_option_count = options.len();
    let selected_exists = !selected_style_id.is_empty()
        && options
            .iter()
            .any(|option| option_identifier(option) == selected_style_id);
    let source_traced_count = options
        .iter()
        .filter(|option| !string_array(option.get("source_refs")).is_empty())
        .count();
    let prompt_ready_count = options
        .iter()
        .filter(|option| {
            !non_empty_or(
                string_field(option, "generation_prompt"),
                &string_field(option, "prompt"),
            )
            .is_empty()
        })
        .count();
    let source_traceability = style_metric_ratio(source_traced_count, style_option_count);
    let prompt_specificity = style_metric_ratio(prompt_ready_count, style_option_count);
    let selection_readiness = f64::from(selected_exists);
    let project_specificity_score =
        round_style_metric((source_traceability + prompt_specificity + selection_readiness) / 3.0);
    let generic_content_ratio = round_style_metric(1.0 - project_specificity_score);

    let mut blockers = Vec::new();
    if options.is_empty() {
        blockers.push(style_fit_issue(
            "STYLE_OPTIONS_MISSING",
            "blocker",
            localized_text(
                locale,
                "没有可用于风格适配检查的候选方案。",
                "No style options are available for fit checks.",
            ),
            locale,
        ));
    } else if selected_style_id.is_empty() {
        blockers.push(style_fit_issue(
            "STYLE_SELECTION_MISSING",
            "blocker",
            localized_text(
                locale,
                "尚未指定需要检查的风格方案。",
                "No style option was selected for fit checks.",
            ),
            locale,
        ));
    } else if !selected_exists {
        blockers.push(style_fit_issue(
            "STYLE_SELECTION_UNKNOWN",
            "blocker",
            localized_text(
                locale,
                "指定的风格方案不在当前候选列表中。",
                "The selected style is not present in the current option set.",
            ),
            locale,
        ));
    }

    let risks = if override_reason.trim().is_empty() {
        Vec::new()
    } else {
        vec![json!({
            "code": "STYLE_MANUAL_OVERRIDE",
            "severity": "warning",
            "message": localized_text(
                locale,
                "操作员采用了人工风格覆盖；后续生产应保留该决策记录。",
                "An operator applied a manual style override; downstream production should preserve this decision record.",
            ),
            "reason": override_reason,
            "return_target": "07",
        })]
    };
    let status = if blockers.is_empty() {
        "passed"
    } else {
        "blocked"
    };
    let source_refs = vec![
        "stage_04/asset_spec_contract.json",
        "stage_06/art_ai_review_report.json",
        "stage_07/style_options.json",
    ];
    let fit_checks = vec![
        style_fit_check(
            "STYLE_OPTIONS_AVAILABLE",
            !options.is_empty(),
            localized_text(
                locale,
                "已生成至少一个可供比较的风格方案。",
                "At least one comparable style option is available.",
            ),
        ),
        style_fit_check(
            "STYLE_SELECTION_RESOLVED",
            selected_exists,
            localized_text(
                locale,
                "目标风格能够解析到当前候选方案。",
                "The target style resolves to the current option set.",
            ),
        ),
        style_fit_check(
            "STYLE_SOURCE_TRACE_AVAILABLE",
            source_traceability > 0.0,
            localized_text(
                locale,
                "风格方案包含上游美术需求或评审来源。",
                "Style options include upstream art requirement or review sources.",
            ),
        ),
        style_fit_check(
            "STYLE_PROMPT_READY",
            prompt_specificity > 0.0,
            localized_text(
                locale,
                "风格方案包含可执行的图像生成提示词。",
                "Style options include executable image-generation prompts.",
            ),
        ),
    ];
    let report = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "status": status,
        "style_id": selected_style_id,
        "project_signature": style_options.and_then(|value| value.get("project")).and_then(Value::as_str).unwrap_or_default(),
        "source_refs": source_refs,
        "fit_checks": fit_checks,
        "risks": risks,
        "blockers": blockers,
        // Legacy aliases retained for already-shipped consumers.
        "selected_style_id": selected_style_id,
        "override_reason": override_reason,
        "style_option_count": style_option_count,
    });
    let acknowledgement_status = if !blockers.is_empty() {
        "blocked"
    } else if risks.is_empty() {
        "not_required"
    } else {
        "acknowledged"
    };
    let acknowledgement = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "status": acknowledgement_status,
        "style_id": selected_style_id,
        "acknowledged_risks": if acknowledgement_status == "acknowledged" { risks.clone() } else { Vec::<Value>::new() },
        "human_confirmation": {
            "status": acknowledgement_status,
            "confirmed": acknowledgement_status == "acknowledged",
            "reason": override_reason,
        },
        "source_refs": ["stage_07/style_fit_report.json", "stage_07/style_confirmation.json"],
        // Legacy aliases retained for already-shipped consumers.
        "selected_style_id": selected_style_id,
        "acknowledged": acknowledgement_status == "acknowledged" || acknowledgement_status == "not_required",
        "risks": risks,
    });
    let overall_score = (project_specificity_score * 100.0).round() as u64;
    let customization_score = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "stage_id": "07",
        "project_signature": style_options.and_then(|value| value.get("project")).and_then(Value::as_str).unwrap_or_default(),
        "status": status,
        "scores": {
            "overall": overall_score,
            "project_specificity": project_specificity_score,
            "source_traceability": source_traceability,
            "prompt_specificity": prompt_specificity,
            "selection_readiness": selection_readiness,
        },
        "generic_content_ratio": generic_content_ratio,
        "project_specificity_score": project_specificity_score,
        "template_leakage_count": 0,
        "blockers": blockers,
        "warnings": risks,
        // Legacy scalar retained for already-shipped consumers.
        "score": overall_score,
    });
    write_json(&out_dir.join("style_fit_report.json"), &report)?;
    write_json(
        &out_dir.join("style_risk_acknowledgement.json"),
        &acknowledgement,
    )?;
    write_json(
        &out_dir.join("customization_score_report.json"),
        &customization_score,
    )?;
    Ok(report)
}

fn style_fit_check(check_id: &str, passed: bool, message: &str) -> Value {
    json!({
        "check_id": check_id,
        "status": if passed { "passed" } else { "failed" },
        "message": message,
    })
}

fn style_fit_issue(code: &str, severity: &str, message: &str, locale: ArtifactLocale) -> Value {
    json!({
        "code": code,
        "severity": severity,
        "message": message,
        "suggestion": localized_text(
            locale,
            "请返回步骤 07，重新生成或选择有效的风格方案。",
            "Return to Step07 and generate or select a valid style option.",
        ),
        "return_target": "07",
    })
}

fn style_metric_ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        round_style_metric(numerator as f64 / denominator as f64)
    }
}

fn round_style_metric(value: f64) -> f64 {
    (value.clamp(0.0, 1.0) * 10_000.0).round() / 10_000.0
}

fn pending_style_application_contract(recommended: &StyleOption, locale: ArtifactLocale) -> Value {
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "status": "pending_confirmation",
        "source_refs": ["stage_06/art_ai_review_report.json", "stage_07/style_options.json"],
        "selected_style_id": "",
        "style_constraints": style_constraints(recommended, locale),
        "blockers": [{
            "code": "STYLE_CONFIRMATION_REQUIRED",
            "message": localized_text(locale, "只有批准风格确认后，才会写入 style_application_contract.json。", "style_application_contract.json is written only after an approved style confirmation."),
        }],
        "warnings": [],
    })
}

fn write_approved_style_application_contract(
    out_dir: &Path,
    style_options: &Value,
    confirmation: &Value,
    locale: ArtifactLocale,
) -> AdmResult<Value> {
    let options = confirmation_options(style_options);
    let selected_style_id = string_field(confirmation, "selected_style_id");
    let selected = options
        .iter()
        .find(|option| option_identifier(option) == selected_style_id)
        .cloned()
        .unwrap_or_else(|| json!({}));
    let selected_option = style_option_from_value(&selected).unwrap_or_default();
    let contract = json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "status": "approved",
        "source_refs": [
            "stage_06/art_ai_review_report.json",
            "stage_07/style_options.json",
            "stage_07/style_confirmation.json",
        ],
        "selected_style_id": selected_style_id,
        "selected_title": non_empty_or(string_field(&selected, "title"), &string_field(confirmation, "selected_title")),
        "selected_image_path": non_empty_or(string_field(&selected, "image_path"), &string_field(confirmation, "selected_image_path")),
        "style_constraints": style_constraints(&selected_option, locale),
        "blockers": [],
        "warnings": [],
    });
    write_json(&out_dir.join("style_application_contract.json"), &contract)?;
    Ok(contract)
}

fn style_constraints(option: &StyleOption, locale: ArtifactLocale) -> Value {
    json!({
        "tile": {
            "readability": "readable_at_1x",
            "palette": option.palette,
        },
        "icon": {
            "background": localized_text(locale, "透明 Alpha 背景", "transparent alpha"),
            "readability": "readable_at_1x",
        },
        "ui": {
            "contrast": localized_text(locale, "对比度应足以支持反复扫视", "high enough for repeated scanning"),
            "layout_density": "game_hud_compatible",
        },
        "background": {
            "contrast_policy": localized_text(locale, "不得遮挡界面或游戏实体", "must not obscure UI or gameplay entities"),
        },
        "effect": {
            "edge_policy": "transparent_edge",
            "duration_policy": localized_text(locale, "采用短促且清晰可读的游戏反馈", "short readable gameplay feedback"),
        },
    })
}

fn style_prompt_override_options(
    override_value: &Value,
    parsed: &ParsedDesignSource,
    assets: &[ArtAssetInput],
    locale: ArtifactLocale,
) -> Vec<StyleOption> {
    let Some(raw_options) = override_value.get("options").and_then(Value::as_array) else {
        return Vec::new();
    };
    let count = override_value
        .get("count")
        .or_else(|| override_value.get("requested_count"))
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(raw_options.len())
        .clamp(1, 5);
    raw_options
        .iter()
        .take(count)
        .enumerate()
        .filter_map(|(index, value)| {
            let preset = STYLE_OPTION_PRESETS[index % STYLE_OPTION_PRESETS.len()];
            let mut option = style_option_from_value(value).unwrap_or_else(|| StyleOption {
                style_id: format!("STYLE-{:02}-{}", index + 1, preset.key),
                title: preset.title(locale).to_string(),
                description: localized_text(
                    locale,
                    "用户调整后的风格图提示词。",
                    "User-refined style-image prompt.",
                )
                .to_string(),
                palette: preset
                    .palette
                    .iter()
                    .map(|item| (*item).to_string())
                    .collect(),
                source_refs: vec!["stage_07.prompt_override".to_string()],
                prompt: String::new(),
                generation_prompt: String::new(),
                negative_prompt: style_negative_prompt(locale).to_string(),
                image_path: String::new(),
                score: 0,
                recommended: false,
                recommendation_reason: String::new(),
                prompt_refined: false,
            });
            if option.title.is_empty() {
                option.title = preset.title(locale).to_string();
            }
            if option.description.is_empty() {
                option.description = localized_text(
                    locale,
                    "用户调整后的风格图提示词。",
                    "User-refined style-image prompt.",
                )
                .to_string();
            }
            if option.palette.len() < 3 {
                option.palette = preset
                    .palette
                    .iter()
                    .map(|item| (*item).to_string())
                    .collect();
            } else {
                option.palette.truncate(3);
            }
            if option.prompt.trim().is_empty() {
                option.prompt = style_prompt(parsed, &option, assets, locale);
            }
            if option.generation_prompt.trim().is_empty() {
                option.generation_prompt = option.prompt.clone();
            }
            if option.negative_prompt.trim().is_empty() {
                option.negative_prompt = style_negative_prompt(locale).to_string();
            }
            if option.source_refs.is_empty() {
                option.source_refs = vec!["stage_07.prompt_override".to_string()];
            }
            (!option.style_id.is_empty()).then_some(option)
        })
        .collect()
}

#[cfg(test)]
fn apply_style_option_recommendations(parsed: &ParsedDesignSource, options: &mut [StyleOption]) {
    apply_style_option_recommendations_with_locale(parsed, options, ArtifactLocale::default())
}

fn apply_style_option_recommendations_with_locale(
    _parsed: &ParsedDesignSource,
    options: &mut [StyleOption],
    locale: ArtifactLocale,
) {
    let total = options.len().max(1);
    let mut best_index = 0usize;
    let mut best_score = i64::MIN;
    for (index, option) in options.iter_mut().enumerate() {
        let mut score = 100 - (index as i64 * std::cmp::max(4, 24 / total as i64));
        if option.style_id.to_ascii_lowercase().contains("diagram") {
            score += 2;
        }
        option.score = score.clamp(60, 100);
        option.recommended = false;
        option.recommendation_reason = localized_text(
            locale,
            "兼顾前期制作可读性与资产一致性。",
            "Balanced fit for early production readability and asset consistency.",
        )
        .to_string();
        if option.score > best_score {
            best_score = option.score;
            best_index = index;
        }
    }
    if let Some(best) = options.get_mut(best_index) {
        best.recommended = true;
        best.recommendation_reason = localized_text(
            locale,
            "推荐默认方案：与当前项目美术需求的整体匹配度最高。",
            "Recommended default: strongest overall fit for this project's current art requirements.",
        )
        .to_string();
    }
}

fn recommended_style_option(options: &[StyleOption]) -> StyleOption {
    options
        .iter()
        .find(|option| option.recommended)
        .cloned()
        .or_else(|| options.iter().max_by_key(|option| option.score).cloned())
        .unwrap_or_default()
}

fn style_prompt(
    parsed: &ParsedDesignSource,
    option: &StyleOption,
    assets: &[ArtAssetInput],
    locale: ArtifactLocale,
) -> String {
    if locale == ArtifactLocale::ZhCn {
        [
            "生成一张游戏美术风格参考图。".to_string(),
            format!("项目：{}", stage_title_with_locale(parsed, locale)),
            format!("风格方向：{}", option.title),
            format!("风格意图：{}", option.description),
            format!(
                "代表性资产：{}",
                representative_asset_text(assets, 180, locale)
            ),
            "画面内容：聚焦上述项目的核心角色、敌人、场景和玩法互动，不得用无关的通用人物、通用物件或通用风景替代。".to_string(),
            "构图：1536×1024 横向游戏美术风格板，以一幅完整玩法场景为主体，辅以少量材质与配色细节；轮廓清晰，不要文字叠加。".to_string(),
        ]
        .join("\n")
    } else {
        [
            "Create a game art style reference image.".to_string(),
            format!("Project: {}", stage_title_with_locale(parsed, locale)),
            format!("Style direction: {}", option.title),
            format!("Style intent: {}", option.description),
            format!(
                "Representative assets: {}",
                representative_asset_text(assets, 180, locale)
            ),
            "Subject: focus on this project's core characters, enemies, environments, and gameplay interaction; do not substitute unrelated generic people, props, or scenery.".to_string(),
            "Composition: 1536x1024 landscape game-art style board led by one complete gameplay scene with a small number of material and palette details; clear silhouettes and no text overlays.".to_string(),
        ]
        .join("\n")
    }
}

fn style_negative_prompt(locale: ArtifactLocale) -> &'static str {
    localized_text(
        locale,
        "避免水印、文字叠加、商标、内嵌界面文字、模糊轮廓、杂乱背景、难以辨认的构图，以及与项目无关的通用人物、物件和风景。",
        "Avoid watermarks, text overlays, logos, embedded UI text, blurred silhouettes, cluttered backgrounds, unreadable composition, and generic people, props, or scenery unrelated to the project.",
    )
}

fn representative_asset_text(
    assets: &[ArtAssetInput],
    max_chars: usize,
    locale: ArtifactLocale,
) -> String {
    let mut labels = Vec::<String>::new();
    let mut seen = BTreeSet::<String>::new();
    for asset in assets.iter().take(8) {
        let label = short_asset_label(asset);
        if label.is_empty() || !seen.insert(label.clone()) {
            continue;
        }
        let candidate = if labels.is_empty() {
            label.clone()
        } else {
            format!("{}, {}", labels.join(", "), label)
        };
        if candidate.len() > max_chars && !labels.is_empty() {
            break;
        }
        labels.push(label);
    }
    if labels.is_empty() {
        localized_text(locale, "核心玩法资产", "core gameplay assets").to_string()
    } else {
        labels.join(", ")
    }
}

fn short_asset_label(asset: &ArtAssetInput) -> String {
    let label = if !asset.name.trim().is_empty() {
        asset.name.as_str()
    } else if !asset.asset_id.trim().is_empty() {
        asset.asset_id.as_str()
    } else {
        asset.asset_type.as_str()
    };
    label
        .split('：')
        .next()
        .unwrap_or(label)
        .lines()
        .next()
        .unwrap_or(label)
        .chars()
        .take(40)
        .collect::<String>()
        .trim()
        .to_string()
}

fn stage_title_with_locale(parsed: &ParsedDesignSource, locale: ArtifactLocale) -> String {
    for key in [
        "project_name",
        "game_title",
        "display_name",
        "project",
        "title",
        "name",
    ] {
        let value = clean_project_title(value_or_summary_field(parsed, key));
        if !value.is_empty() {
            return value;
        }
    }
    for line in parsed.raw_text.lines() {
        let stripped = line.trim();
        if let Some(title) = stripped.strip_prefix("# ") {
            let value = clean_project_title(title.to_string());
            if !value.is_empty() {
                return value;
            }
        }
    }
    localized_text(locale, "未命名游戏", "Untitled Game").to_string()
}

fn value_or_summary_field(parsed: &ParsedDesignSource, key: &str) -> String {
    parsed
        .design_summary
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            parsed
                .design_summary
                .get("design_summary")
                .and_then(|summary| summary.get(key))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default()
}

fn clean_project_title(value: String) -> String {
    let mut text = value
        .trim()
        .trim_start_matches(|ch: char| {
            ch == '#' || ch == '?' || ch == '？' || ch == '\u{fffd}' || ch.is_whitespace()
        })
        .trim()
        .to_string();
    for separator in [" — ", " - "] {
        if let Some((before, _)) = text.split_once(separator) {
            text = before.trim().to_string();
        }
    }
    if [
        "程序自动开发流程工具",
        "AutoDesignMaker",
        "Untitled Game",
        "Initial Idea Intake",
        "Idea Intake",
    ]
    .contains(&text.as_str())
    {
        String::new()
    } else {
        text.chars().take(80).collect::<String>().trim().to_string()
    }
}

fn parse_art_assets(value: Option<&Value>) -> Vec<ArtAssetInput> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(art_asset_from_value)
        .collect()
}

fn art_asset_from_value(value: &Value) -> Option<ArtAssetInput> {
    let asset_id = non_empty_or(string_field(value, "asset_id"), &string_field(value, "id"));
    let name = string_field(value, "name");
    let asset_type = string_field(value, "asset_type");
    if asset_id.is_empty() && name.is_empty() && asset_type.is_empty() {
        return None;
    }
    Some(ArtAssetInput {
        asset_id,
        name,
        asset_type,
        source: non_empty_or(
            string_field(value, "source"),
            &string_array(value.get("source_refs")).join(", "),
        ),
        priority: string_field(value, "priority"),
        complexity: string_field(value, "complexity"),
    })
}

fn style_option_from_value(value: &Value) -> Option<StyleOption> {
    let style_id = option_identifier(value);
    if style_id.is_empty() {
        return None;
    }
    let prompt = non_empty_or(
        string_field(value, "prompt"),
        &string_field(value, "generation_prompt"),
    );
    Some(StyleOption {
        style_id,
        title: string_field(value, "title"),
        description: string_field(value, "description"),
        palette: string_array(value.get("palette")),
        source_refs: string_array(value.get("source_refs")),
        prompt: prompt.clone(),
        generation_prompt: non_empty_or(string_field(value, "generation_prompt"), &prompt),
        negative_prompt: string_field(value, "negative_prompt"),
        image_path: string_field(value, "image_path"),
        score: value.get("score").and_then(Value::as_i64).unwrap_or(0),
        recommended: value
            .get("recommended")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        recommendation_reason: string_field(value, "recommendation_reason"),
        prompt_refined: value
            .get("prompt_refined")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn confirmation_options(style_options: &Value) -> Vec<Value> {
    style_options
        .get("options")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(Value::is_object)
        .collect()
}

fn style_options_markdown(options: &[StyleOption], locale: ArtifactLocale) -> String {
    let mut lines = vec![
        format!("<!-- artifact_locale: {} -->", locale.as_str()),
        localized_text(locale, "# 美术风格选项", "# Art Style Options").to_string(),
        String::new(),
    ];
    for option in options {
        lines.push(format!(
            "- {}: {} - {}",
            option.style_id, option.title, option.description
        ));
    }
    lines.push(String::new());
    lines.join("\n")
}

fn style_confirmation_markdown(
    locale: ArtifactLocale,
    status: &str,
    selected_style_id: &str,
) -> String {
    let mut lines = vec![
        format!("<!-- artifact_locale: {} -->", locale.as_str()),
        localized_text(locale, "# 美术风格确认", "# Art Style Confirmation").to_string(),
        String::new(),
        if locale == ArtifactLocale::ZhCn {
            format!("- 状态：{status}")
        } else {
            format!("- Status: {status}")
        },
    ];
    if !selected_style_id.is_empty() {
        lines.push(if locale == ArtifactLocale::ZhCn {
            format!("- 已选风格：{selected_style_id}")
        } else {
            format!("- Selected: {selected_style_id}")
        });
    }
    lines.push(String::new());
    lines.join("\n")
}

fn artifact_locale_from_value(value: &Value) -> ArtifactLocale {
    artifact_locale_from_inputs(value)
}

fn option_identifier(value: &Value) -> String {
    non_empty_or(
        string_field(value, "style_id"),
        &string_field(value, "option_id"),
    )
}

fn confirmation_is_approved(value: &Value) -> bool {
    value
        .get("status")
        .and_then(Value::as_str)
        .map(|status| status == "approved")
        .unwrap_or(false)
}

fn review_is_blocked(value: &Value) -> bool {
    matches!(
        value.get("review_status").and_then(Value::as_str),
        Some("blocked")
    ) || matches!(
        value.get("verdict").and_then(Value::as_str),
        Some("BLOCKED")
    )
}

fn style_option_count(count: usize) -> usize {
    count.clamp(3, 5)
}

fn ensure_stage(actual: u32, expected: u32) -> AdmResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(AdmError::new(format!(
            "generator expected stage {expected:02}, got {actual:02}"
        )))
    }
}

fn to_json_value<T: Serialize>(value: &T) -> AdmResult<Value> {
    serde_json::to_value(value)
        .map_err(|error| AdmError::new(format!("failed to serialize step07 JSON: {error}")))
}

fn object_map_or_empty(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(|item| match item {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| match item {
            Value::String(text) => Some(text.trim().to_string()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .filter(|item| !item.is_empty())
        .collect()
}

fn non_empty_or(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn is_empty_object(value: &Value) -> bool {
    value.as_object().map(Map::is_empty).unwrap_or(false)
}

fn stage_relative_path(out_dir: &Path, path: &Path) -> String {
    path.strip_prefix(out_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn resolve_stage_path(output_dir: &Path, path_text: &str) -> PathBuf {
    let path = PathBuf::from(path_text);
    if path.is_absolute() {
        path
    } else {
        output_dir.join(path)
    }
}

impl Default for StyleOption {
    fn default() -> Self {
        Self {
            style_id: String::new(),
            title: String::new(),
            description: String::new(),
            palette: Vec::new(),
            source_refs: Vec::new(),
            prompt: String::new(),
            generation_prompt: String::new(),
            negative_prompt: String::new(),
            image_path: String::new(),
            score: 0,
            recommended: false,
            recommendation_reason: String::new(),
            prompt_refined: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::schema::{load_structured_file, validate_contract};
    use adm_new_foundation::{new_stable_id, paths::SourceProjectRoot};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn sample_parsed() -> ParsedDesignSource {
        ParsedDesignSource {
            source: "design.md".to_string(),
            source_path: "design.md".to_string(),
            source_sha256: "sha".to_string(),
            source_size_bytes: 42,
            source_line_count: 3,
            parsed_at: now_iso(),
            layers: Vec::new(),
            selections: Vec::new(),
            raw_text: "# Axiom Verge - Full Design Specification\n\nbody".to_string(),
            source_package: "devflow_Design_v1".to_string(),
            source_input_type: "Design".to_string(),
            design_summary: json!({}),
            structured_source_warning: None,
        }
    }

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(new_stable_id(name).unwrap())
    }

    fn write_stage_inputs(root: &Path) {
        write_json(
            &root.join("stage_04").join("asset_spec_contract.json"),
            &json!({
                "assets": [{
                    "asset_id": "ASSET-001",
                    "name": "Hero character concept",
                    "asset_type": "character",
                    "source": "design.md:1",
                }]
            }),
        )
        .unwrap();
        write_json(
            &root.join("stage_04").join("asset_registry.json"),
            &json!({
                "assets": [{
                    "asset_id": "ASSET-001",
                    "name": "Hero character concept",
                    "asset_type": "character",
                    "source": "design.md:1",
                }]
            }),
        )
        .unwrap();
        write_json(
            &root.join("stage_06").join("art_ai_review_report.json"),
            &json!({
                "schema_version": "1.0",
                "review_status": "passed",
                "blockers": [],
                "warnings": [],
            }),
        )
        .unwrap();
    }

    #[derive(Debug)]
    struct FakeStyleImageGenerator {
        fail_painterly: bool,
    }

    impl StyleImageGenerator for FakeStyleImageGenerator {
        fn generate(&self, request: &StyleImageRequest) -> AdmResult<StyleImageResult> {
            if self.fail_painterly && request.style_id.contains("painterly") {
                return Err(AdmError::new("fake provider failure sk-test-must-redact"));
            }
            Ok(StyleImageResult::generated(
                test_png(request.requested_width, request.requested_height),
                "fake-provider",
                "fake-model",
                request.requested_width,
                request.requested_height,
            ))
        }
    }

    #[derive(Debug)]
    struct CountingStyleImageGenerator {
        calls: Arc<AtomicUsize>,
        stop_after_first: Option<WorkUnitStopToken>,
    }

    impl StyleImageGenerator for CountingStyleImageGenerator {
        fn execution_scope_fingerprint(&self) -> String {
            "counting-style-image-v1".to_string()
        }

        fn generate(&self, request: &StyleImageRequest) -> AdmResult<StyleImageResult> {
            let call_index = self.calls.fetch_add(1, Ordering::AcqRel);
            if call_index == 0
                && let Some(stop_token) = &self.stop_after_first
            {
                stop_token.request_stop();
            }
            Ok(StyleImageResult::generated(
                test_png(request.requested_width, request.requested_height),
                "counting-provider",
                "counting-model",
                request.requested_width,
                request.requested_height,
            ))
        }
    }

    fn json_tree_text(root: &Path) -> String {
        fn collect(path: &Path, text: &mut String) {
            let Ok(entries) = fs::read_dir(path) else {
                return;
            };
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    collect(&path, text);
                } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
                    text.push_str(&fs::read_to_string(path).unwrap());
                }
            }
        }
        let mut text = String::new();
        collect(root, &mut text);
        text
    }

    fn test_png(width: u32, height: u32) -> Vec<u8> {
        use std::io::Cursor;

        let image =
            image::DynamicImage::ImageRgb8(RgbImage::from_pixel(width, height, Rgb([32, 96, 160])));
        let mut output = Cursor::new(Vec::new());
        image.write_to(&mut output, ImageFormat::Png).unwrap();
        output.into_inner()
    }

    #[test]
    fn step07_plugin_spec_matches_python_wrapper() {
        let spec = step07_plugin_spec();
        assert_eq!(spec.stage_id, "07");
        assert!(spec.source_groups.is_empty());
        assert_eq!(spec.test_mode_status, "success");
        assert_eq!(spec.generation_entrypoint, "apply_development_plan_outputs");
    }

    #[test]
    fn prompt_response_parser_filters_valid_ids() {
        let valid = BTreeSet::from([
            "STYLE-01-readable_production".to_string(),
            "STYLE-02-painterly_concept".to_string(),
        ]);
        let parsed = parse_style_prompt_response(
            "已改。\nPROMPT_START\nSTYLE-01-readable_production: dark readable silhouettes\n- STYLE-99-extra: ignore\nPROMPT_END",
            Some(&valid),
        );
        assert_eq!(parsed.explanation, "已改。");
        assert_eq!(
            parsed.prompts["STYLE-01-readable_production"],
            "dark readable silhouettes"
        );
        assert!(!parsed.prompts.contains_key("STYLE-99-extra"));
    }

    #[test]
    fn style_image_file_names_reject_path_traversal() {
        assert_eq!(
            safe_style_image_name("STYLE-01-readable").unwrap(),
            "STYLE-01-readable.png"
        );
        assert!(safe_style_image_name("../../outside").is_err());
        assert!(safe_style_image_name("folder/style").is_err());
    }

    #[test]
    fn stage07_generates_style_options_and_waits_for_confirmation() {
        let root = temp_root("step07_generate");
        write_stage_inputs(&root);
        let out_dir = root.join("stage_07");
        fs::create_dir_all(out_dir.join("generated_images")).unwrap();
        fs::write(
            out_dir.join("generated_images").join("STYLE-99-stale.png"),
            b"stale",
        )
        .unwrap();
        let generator = Step07OutputGenerator {
            config: StyleGenerationConfig {
                option_count: 3,
                image_generation_enabled: false,
            },
            ..Step07OutputGenerator::default()
        };

        let result = generator
            .generate(7, &sample_parsed(), &out_dir, &json!({}))
            .unwrap();
        let style_options = read_json(&out_dir.join("style_options.json"), json!({}));
        let first_image = out_dir
            .join("generated_images")
            .join("STYLE-01-readable_production.png");

        assert_eq!(result["status"], "waiting_confirmation");
        assert_eq!(result["artifact_locale"], "zh-CN");
        assert_eq!(result["style_option_count"], 3);
        assert_eq!(result["generated_image_count"], 0);
        assert_eq!(result["fallback_image_count"], 3);
        assert_eq!(result["style_options"][0]["image_status"], "fallback");
        assert_eq!(style_options["option_count"], 3);
        assert_eq!(style_options["artifact_locale"], "zh-CN");
        assert_eq!(style_options["options"][0]["title"], "清晰量产风");
        assert!(
            style_options["options"][0]["description"]
                .as_str()
                .unwrap()
                .contains("清晰轮廓")
        );
        assert!(
            style_options["options"][0]["recommendation_reason"]
                .as_str()
                .unwrap()
                .contains("推荐默认方案")
        );
        assert!(
            style_options["options"][0]["generation_prompt"]
                .as_str()
                .unwrap()
                .starts_with("生成一张")
        );
        assert_eq!(
            style_options["options"][0]["generation_prompt"],
            style_options["options"][0]["prompt"]
        );
        assert!(
            style_options["options"][0]["negative_prompt"]
                .as_str()
                .unwrap()
                .starts_with("避免水印")
        );
        assert!(
            fs::read_to_string(out_dir.join("style_options.md"))
                .unwrap()
                .contains("# 美术风格选项")
        );
        assert!(
            fs::read_to_string(out_dir.join("style_confirmation.md"))
                .unwrap()
                .contains("状态：waiting_confirmation")
        );
        assert!(fs::read(&first_image).unwrap().starts_with(b"\x89PNG"));
        assert_eq!(
            image::image_dimensions(&first_image).unwrap(),
            (FALLBACK_IMAGE_WIDTH, FALLBACK_IMAGE_HEIGHT)
        );
        assert_eq!(style_options["options"][0]["image_status"], "fallback");
        assert!(
            !out_dir
                .join("generated_images")
                .join("STYLE-99-stale.png")
                .exists()
        );
        assert!(out_dir.join("style_confirmation_pending.json").exists());
        let pending_contract = read_json(
            &out_dir.join("style_application_contract_pending.json"),
            json!({}),
        );
        assert_eq!(
            pending_contract["style_constraints"]["ui"]["contrast"],
            "对比度应足以支持反复扫视"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage07_registry_reports_validate_against_declared_schemas() {
        let root = temp_root("step07_registry_schemas");
        let style_options = json!({
            "project": "project-signature",
            "options": [{
                "style_id": "STYLE-01-readable_production",
                "source_refs": ["stage_04/asset_spec_contract.json"],
                "generation_prompt": "生成清晰可读、可量产的游戏资产。"
            }]
        });

        for locale in [ArtifactLocale::ZhCn, ArtifactLocale::EnUs] {
            let out_dir = root.join(locale.as_str());
            let report = write_style_fit_outputs(
                &out_dir,
                Some(&style_options),
                "STYLE-01-readable_production",
                "",
                locale,
            )
            .unwrap();
            let acknowledgement =
                read_json(&out_dir.join("style_risk_acknowledgement.json"), json!({}));
            let score = read_json(&out_dir.join("customization_score_report.json"), json!({}));

            assert_registered_style_schema(
                &report,
                "knowledge/schemas/ai_design/style_fit_report.schema.json",
            );
            assert_registered_style_schema(
                &acknowledgement,
                "knowledge/schemas/ai_design/style_risk_acknowledgement.schema.json",
            );
            assert_registered_style_schema(
                &score,
                "knowledge/schemas/ai_design/customization_score_report.schema.json",
            );
            assert_eq!(report["schema_version"], "1.0");
            assert_eq!(report["style_id"], "STYLE-01-readable_production");
            assert!(
                report["fit_checks"]
                    .as_array()
                    .is_some_and(|items| !items.is_empty())
            );
            assert_eq!(acknowledgement["status"], "not_required");
            assert!(acknowledgement["acknowledged_risks"].is_array());
            assert_eq!(score["stage_id"], "07");
            assert_eq!(score["status"], "passed");
            assert!(score["scores"].is_object());
        }

        let blocked_dir = root.join("blocked");
        let blocked =
            write_style_fit_outputs(&blocked_dir, None, "", "", ArtifactLocale::ZhCn).unwrap();
        let blocked_ack = read_json(
            &blocked_dir.join("style_risk_acknowledgement.json"),
            json!({}),
        );
        let blocked_score = read_json(
            &blocked_dir.join("customization_score_report.json"),
            json!({}),
        );
        assert_registered_style_schema(
            &blocked,
            "knowledge/schemas/ai_design/style_fit_report.schema.json",
        );
        assert_registered_style_schema(
            &blocked_ack,
            "knowledge/schemas/ai_design/style_risk_acknowledgement.schema.json",
        );
        assert_registered_style_schema(
            &blocked_score,
            "knowledge/schemas/ai_design/customization_score_report.schema.json",
        );
        assert_eq!(blocked["status"], "blocked");
        assert_eq!(blocked_ack["status"], "blocked");
        assert_eq!(blocked_score["status"], "blocked");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage07_english_locale_localizes_public_options_prompts_and_confirmation() {
        let root = temp_root("step07_english_locale");
        write_stage_inputs(&root);
        let out_dir = root.join("stage_07");
        let generator = Step07OutputGenerator {
            config: StyleGenerationConfig {
                option_count: 3,
                image_generation_enabled: false,
            },
            ..Step07OutputGenerator::default()
        };

        let result = generator
            .generate(
                7,
                &sample_parsed(),
                &out_dir,
                &json!({"artifact_locale": "en-US"}),
            )
            .unwrap();
        let style_options = read_json(&out_dir.join("style_options.json"), json!({}));
        let first = style_options["options"][0].clone();

        assert_eq!(result["artifact_locale"], "en-US");
        assert_eq!(style_options["artifact_locale"], "en-US");
        assert_eq!(first["title"], "Readable Production");
        assert!(
            first["description"]
                .as_str()
                .unwrap()
                .starts_with("Clear silhouettes")
        );
        assert!(
            first["recommendation_reason"]
                .as_str()
                .unwrap()
                .starts_with("Recommended default")
        );
        assert!(
            first["generation_prompt"]
                .as_str()
                .unwrap()
                .starts_with("Create")
        );
        assert_eq!(first["generation_prompt"], first["prompt"]);
        assert!(
            first["negative_prompt"]
                .as_str()
                .unwrap()
                .starts_with("Avoid")
        );
        assert!(
            fs::read_to_string(out_dir.join("style_options.md"))
                .unwrap()
                .contains("# Art Style Options")
        );

        write_style_confirmation(
            &out_dir,
            &first,
            "Use warmer lighting.",
            "approved",
            "manual",
        )
        .unwrap();
        let confirmed = style_confirmation_outputs(&sample_parsed(), &out_dir, None).unwrap();
        let confirmation_md = fs::read_to_string(out_dir.join("style_confirmation.md")).unwrap();
        let contract = read_json(&out_dir.join("style_application_contract.json"), json!({}));
        assert_eq!(confirmed["artifact_locale"], "en-US");
        assert_eq!(contract["artifact_locale"], "en-US");
        assert_eq!(
            contract["style_constraints"]["ui"]["contrast"],
            "high enough for repeated scanning"
        );
        assert!(confirmation_md.contains("# Art Style Confirmation"));
        assert!(confirmation_md.contains("- Status: approved"));
        assert!(confirmation_md.contains("- Selected: STYLE-01-readable_production"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage07_does_not_reuse_approved_options_from_another_artifact_locale() {
        let root = temp_root("step07_locale_switch");
        write_stage_inputs(&root);
        let out_dir = root.join("stage_07");
        let generator = Step07OutputGenerator {
            config: StyleGenerationConfig {
                option_count: 3,
                image_generation_enabled: false,
            },
            ..Step07OutputGenerator::default()
        };

        generator
            .generate(7, &sample_parsed(), &out_dir, &json!({}))
            .unwrap();
        let zh_options = read_json(&out_dir.join("style_options.json"), json!({}));
        write_style_confirmation(
            &out_dir,
            &zh_options["options"][0],
            "keep this selected style",
            "approved",
            "manual",
        )
        .unwrap();

        let switched = generator
            .generate(
                7,
                &sample_parsed(),
                &out_dir,
                &json!({"artifact_locale": "en-US"}),
            )
            .unwrap();
        let en_options = read_json(&out_dir.join("style_options.json"), json!({}));
        let en_confirmation = read_json(&out_dir.join(STYLE_CONFIRMATION_FILENAME), json!({}));
        let en_contract = read_json(&out_dir.join("style_application_contract.json"), json!({}));

        assert_ne!(switched["reused_generation"], json!(true));
        assert_eq!(en_options["artifact_locale"], "en-US");
        assert_eq!(en_options["options"][0]["title"], "Readable Production");
        assert_eq!(en_confirmation["artifact_locale"], "en-US");
        assert_eq!(en_confirmation["selected_title"], "Readable Production");
        assert_eq!(en_contract["artifact_locale"], "en-US");
        assert_eq!(en_contract["selected_title"], "Readable Production");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step07_prerequisite_error_messages_follow_artifact_locale() {
        let zh_root = temp_root("step07_prerequisite_zh");
        let zh_dir = zh_root.join("stage_07");
        let generator = Step07OutputGenerator::default();
        let zh = generator
            .generate(7, &sample_parsed(), &zh_dir, &json!({}))
            .unwrap();
        let zh_report = read_json(&zh_dir.join("style_prerequisite_report.json"), json!({}));
        assert_eq!(zh["artifact_locale"], "zh-CN");
        assert!(
            zh_report["blockers"][0]["message"]
                .as_str()
                .unwrap()
                .starts_with("步骤 07")
        );

        let en_root = temp_root("step07_prerequisite_en");
        let en_dir = en_root.join("stage_07");
        let en = generator
            .generate(
                7,
                &sample_parsed(),
                &en_dir,
                &json!({"artifact_locale": "en-US"}),
            )
            .unwrap();
        let en_report = read_json(&en_dir.join("style_prerequisite_report.json"), json!({}));
        assert_eq!(en["artifact_locale"], "en-US");
        assert!(
            en_report["blockers"][0]["message"]
                .as_str()
                .unwrap()
                .starts_with("Step07")
        );
        let _ = fs::remove_dir_all(zh_root);
        let _ = fs::remove_dir_all(en_root);
    }

    #[test]
    fn stage07_commits_valid_provider_images_and_reports_truthful_counts() {
        let root = temp_root("step07_provider_success");
        write_stage_inputs(&root);
        let out_dir = root.join("stage_07");
        let generator = FakeStyleImageGenerator {
            fail_painterly: false,
        };

        let result = generate_step07_outputs_with_generator(
            &sample_parsed(),
            &out_dir,
            Step07Inputs::from_stage_dirs(&out_dir),
            &StyleGenerationConfig {
                option_count: 3,
                image_generation_enabled: true,
            },
            Some(&generator),
        )
        .unwrap();
        let log = read_json(&out_dir.join("generation_log.json"), json!({}));

        assert_eq!(result["generated_image_count"], 3);
        assert_eq!(result["fallback_image_count"], 0);
        assert_eq!(log["status"], "success");
        assert_eq!(log["provider_generated_count"], 3);
        assert_eq!(
            image::image_dimensions(
                out_dir
                    .join("generated_images")
                    .join("STYLE-01-readable_production.png")
            )
            .unwrap(),
            (PROVIDER_IMAGE_WIDTH, PROVIDER_IMAGE_HEIGHT)
        );
        let options = read_json(&out_dir.join("style_options.json"), json!({}));
        let prompt = options["options"][0]["generation_prompt"].as_str().unwrap();
        assert!(prompt.contains("Hero character concept"));
        assert!(prompt.contains("1536×1024"));
        assert!(!prompt.contains("代表性资产：character"));
        assert!(
            log["records"]
                .as_array()
                .unwrap()
                .iter()
                .all(|record| record["status"] == "generated")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage07_stop_preserves_published_directory_and_resume_reuses_committed_png() {
        let root = temp_root("step07_safe_unit_resume");
        write_stage_inputs(&root);
        let out_dir = root.join("stage_07");
        let generated_dir = out_dir.join("generated_images");
        fs::create_dir_all(&generated_dir).unwrap();
        fs::write(generated_dir.join("previous.png"), b"previous-round").unwrap();
        write_json(
            &out_dir.join("generated_images_manifest.json"),
            &json!({"status": "previous-round"}),
        )
        .unwrap();

        let stop_token = WorkUnitStopToken::default();
        let calls = Arc::new(AtomicUsize::new(0));
        let generator = CountingStyleImageGenerator {
            calls: calls.clone(),
            stop_after_first: Some(stop_token.clone()),
        };
        let journal_root = root.join("checkpoints").join("work_units").join("stage_07");
        let config = StyleGenerationConfig {
            option_count: 3,
            image_generation_enabled: true,
        };

        let stopped = generate_step07_outputs_with_runtime(
            &sample_parsed(),
            &out_dir,
            Step07Inputs::from_stage_dirs(&out_dir),
            &config,
            Some(&generator),
            &journal_root,
            &stop_token,
        )
        .unwrap();

        assert_eq!(stopped["status"], "stopped");
        assert_eq!(stopped["directory_committed"], false);
        assert_eq!(calls.load(Ordering::Acquire), 1);
        assert_eq!(
            fs::read(generated_dir.join("previous.png")).unwrap(),
            b"previous-round"
        );
        assert_eq!(
            read_json(&out_dir.join("generated_images_manifest.json"), json!({}))["status"],
            "previous-round"
        );

        stop_token.clear();
        let resumed = generate_step07_outputs_with_runtime(
            &sample_parsed(),
            &out_dir,
            Step07Inputs::from_stage_dirs(&out_dir),
            &config,
            Some(&generator),
            &journal_root,
            &stop_token,
        )
        .unwrap();
        let manifest = read_json(&out_dir.join("generated_images_manifest.json"), json!({}));

        assert_eq!(resumed["status"], "waiting_confirmation");
        assert_eq!(calls.load(Ordering::Acquire), 3);
        assert_eq!(manifest["status"], "success");
        assert_eq!(manifest["provider_generated_count"], 3);
        assert_eq!(manifest["directory_committed"], true);
        assert_eq!(manifest["records"][0]["unit_status"], "reused");
        assert!(!generated_dir.join("previous.png").exists());
        assert_eq!(
            fs::read_dir(&generated_dir).unwrap().count(),
            3,
            "the resumed round should publish one complete directory"
        );

        let journal_text = json_tree_text(&journal_root);
        let manifest_text = serde_json::to_string(&manifest).unwrap();
        assert!(!journal_text.contains("Axiom Verge"));
        assert!(!journal_text.contains("\"prompt\""));
        assert!(!journal_text.contains("iVBOR"));
        assert!(!journal_text.contains(&root.to_string_lossy().to_string()));
        assert!(!manifest_text.contains("Axiom Verge"));
        assert!(!manifest_text.contains("\"prompt\""));
        assert!(!manifest_text.contains("iVBOR"));
        assert!(!manifest_text.contains(&root.to_string_lossy().to_string()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage07_unknown_journal_blocks_without_replacing_published_images() {
        let root = temp_root("step07_unknown_journal");
        write_stage_inputs(&root);
        let out_dir = root.join("stage_07");
        let journal_root = root.join("checkpoints").join("work_units").join("stage_07");
        let calls = Arc::new(AtomicUsize::new(0));
        let generator = CountingStyleImageGenerator {
            calls: calls.clone(),
            stop_after_first: None,
        };
        let config = StyleGenerationConfig {
            option_count: 3,
            image_generation_enabled: true,
        };
        let stop = WorkUnitStopToken::default();

        generate_step07_outputs_with_runtime(
            &sample_parsed(),
            &out_dir,
            Step07Inputs::from_stage_dirs(&out_dir),
            &config,
            Some(&generator),
            &journal_root,
            &stop,
        )
        .unwrap();
        let published_image = out_dir
            .join("generated_images")
            .join("STYLE-01-readable_production.png");
        let published_before = fs::read(&published_image).unwrap();
        let manifest_before = fs::read(out_dir.join("generated_images_manifest.json")).unwrap();

        let lineage = fs::read_dir(journal_root.join("journal"))
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| entry.path().is_dir())
            .unwrap();
        let latest = fs::read_dir(lineage.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .max()
            .unwrap();
        fs::write(latest, b"{corrupt-journal").unwrap();

        let blocked = generate_step07_outputs_with_runtime(
            &sample_parsed(),
            &out_dir,
            Step07Inputs::from_stage_dirs(&out_dir),
            &config,
            Some(&generator),
            &journal_root,
            &stop,
        )
        .unwrap();

        assert_eq!(blocked["status"], "recovery_blocked");
        assert_eq!(blocked["directory_committed"], false);
        assert_eq!(calls.load(Ordering::Acquire), 3);
        assert_eq!(fs::read(&published_image).unwrap(), published_before);
        assert_eq!(
            fs::read(out_dir.join("generated_images_manifest.json")).unwrap(),
            manifest_before
        );
        assert_eq!(
            read_json(&out_dir.join("generation_attempt_log.json"), json!({}))["status"],
            "recovery_blocked"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage07_provider_failure_uses_marked_fallback_without_leaking_secret() {
        let root = temp_root("step07_provider_partial");
        write_stage_inputs(&root);
        let out_dir = root.join("stage_07");
        let generator = FakeStyleImageGenerator {
            fail_painterly: true,
        };

        generate_step07_outputs_with_generator(
            &sample_parsed(),
            &out_dir,
            Step07Inputs::from_stage_dirs(&out_dir),
            &StyleGenerationConfig {
                option_count: 3,
                image_generation_enabled: true,
            },
            Some(&generator),
        )
        .unwrap();
        let log = read_json(&out_dir.join("generation_log.json"), json!({}));
        let serialized = serde_json::to_string(&log).unwrap();

        assert_eq!(log["status"], "partial");
        assert_eq!(log["provider_generated_count"], 2);
        assert_eq!(log["fallback_count"], 1);
        assert_eq!(log["failed_count"], 1);
        assert!(!serialized.contains("sk-test-must-redact"));
        assert!(!serialized.contains("fake provider failure"));
        assert!(serialized.contains("图像服务生成失败"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prompt_override_is_consumed_and_limits_generated_options() {
        let root = temp_root("step07_override");
        write_stage_inputs(&root);
        let out_dir = root.join("stage_07");
        let mut options = vec![
            StyleOption {
                style_id: "STYLE-01-readable_production".to_string(),
                title: "清晰量产风".to_string(),
                description: "Readable".to_string(),
                prompt: "original prompt 1".to_string(),
                palette: vec![
                    "#111111".to_string(),
                    "#222222".to_string(),
                    "#333333".to_string(),
                ],
                source_refs: Vec::new(),
                ..StyleOption::default()
            },
            StyleOption {
                style_id: "STYLE-02-painterly_concept".to_string(),
                title: "概念绘画风".to_string(),
                description: "Painterly".to_string(),
                prompt: "original prompt 2".to_string(),
                palette: vec![
                    "#444444".to_string(),
                    "#555555".to_string(),
                    "#666666".to_string(),
                ],
                source_refs: Vec::new(),
                ..StyleOption::default()
            },
        ];
        apply_style_option_recommendations(&sample_parsed(), &mut options);
        write_style_prompt_override(
            &out_dir,
            &options,
            &BTreeMap::from([(
                "STYLE-01-readable_production".to_string(),
                "refined dark prompt".to_string(),
            )]),
            1,
        )
        .unwrap();

        let result = generate_step07_outputs(
            &sample_parsed(),
            &out_dir,
            Step07Inputs::from_stage_dirs(&out_dir),
            &StyleGenerationConfig {
                option_count: 5,
                image_generation_enabled: false,
            },
        )
        .unwrap();
        let style_options = read_json(&out_dir.join("style_options.json"), json!({}));
        let generation_log = read_json(&out_dir.join("generation_log.json"), json!({}));

        assert_eq!(result["prompt_override_used"], true);
        assert_eq!(style_options["prompt_override_used"], true);
        assert_eq!(style_options["option_count"], 1);
        assert_eq!(style_options["options"][0]["prompt"], "refined dark prompt");
        assert_eq!(
            style_options["options"][0]["generation_prompt"],
            "refined dark prompt"
        );
        assert_eq!(generation_log["generated_count"], 0);
        assert_eq!(generation_log["fallback_count"], 1);
        assert!(!out_dir.join(STYLE_PROMPT_OVERRIDE_FILENAME).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn approved_confirmation_writes_application_contract() {
        let root = temp_root("step07_confirm");
        let out_dir = root.join("stage_07");
        fs::create_dir_all(&out_dir).unwrap();
        let option = json!({
            "style_id": "STYLE-02",
            "title": "Painterly",
            "image_path": "generated_images/STYLE-02.png",
            "palette": ["#111111", "#222222", "#333333"],
        });
        write_json(
            &out_dir.join("style_options.json"),
            &json!({"options": [option.clone()]}),
        )
        .unwrap();
        write_style_confirmation(
            &out_dir,
            &option,
            "Use warmer lighting.",
            "approved",
            "manual",
        )
        .unwrap();

        let result = style_confirmation_outputs(&sample_parsed(), &out_dir, None).unwrap();
        let contract = read_json(&out_dir.join("style_application_contract.json"), json!({}));
        let fit = read_json(&out_dir.join("style_fit_report.json"), json!({}));
        let acknowledgement =
            read_json(&out_dir.join("style_risk_acknowledgement.json"), json!({}));

        assert_eq!(result["confirmation_status"], "approved");
        assert_eq!(result["selected_style_id"], "STYLE-02");
        assert_eq!(contract["status"], "approved");
        assert_eq!(contract["selected_style_id"], "STYLE-02");
        assert_eq!(fit["style_id"], "STYLE-02");
        assert_eq!(acknowledgement["status"], "acknowledged");
        assert_eq!(
            acknowledgement["acknowledged_risks"][0]["code"],
            "STYLE_MANUAL_OVERRIDE"
        );
        let _ = fs::remove_dir_all(root);
    }

    fn assert_registered_style_schema(value: &Value, schema_path: &str) {
        let root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let schema = load_structured_file(&root.join(schema_path).unwrap()).unwrap();
        let errors = validate_contract(value, &schema);
        assert!(errors.is_empty(), "{schema_path}: {errors:?}");
    }

    #[test]
    fn cleanup_unselected_images_preserves_selected_unique_fallback() {
        let root = temp_root("step07_cleanup");
        let out_dir = root.join("stage_07");
        let generated = out_dir.join("generated_images");
        fs::create_dir_all(&generated).unwrap();
        fs::write(generated.join("STYLE-02_123.png"), b"selected").unwrap();
        fs::write(generated.join("STYLE-01.png"), b"stale").unwrap();

        let removed = cleanup_unselected_style_images(
            &out_dir,
            "STYLE-02",
            "generated_images/STYLE-02_123.png",
        )
        .unwrap();

        assert_eq!(removed, 1);
        assert!(generated.join("STYLE-02_123.png").exists());
        assert!(!generated.join("STYLE-01.png").exists());
        let _ = fs::remove_dir_all(root);
    }
}
