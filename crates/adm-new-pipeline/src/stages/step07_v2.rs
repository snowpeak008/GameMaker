use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use adm_new_foundation::{AdmError, AdmResult, io, sha256_hex};
use adm_new_game_spec::{GameSpec, canonicalize_game_spec, parse_game_spec};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
#[cfg(test)]
use image::{Rgb, RgbImage};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::collections::BTreeSet;

pub const STEP07_V2_COMPILER_VERSION: &str = "game_spec_step07_art_direction.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetGateStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtDirectionSpec {
    pub schema_version: String,
    pub compiler_version: String,
    pub source_game_spec_hash: String,
    pub style_summary: String,
    pub palette: Vec<String>,
    pub composition_constraints: Vec<String>,
    pub usage_categories: Vec<String>,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RepresentativeAssetTask {
    pub asset_id: String,
    pub title: String,
    pub asset_type: String,
    pub expected_width: u32,
    pub expected_height: u32,
    pub require_alpha: bool,
    pub transparent_margin_px: u32,
    pub prompt: String,
    pub negative_prompt: String,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetGateIssue {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageHardGateItem {
    pub asset_id: String,
    pub image_path: String,
    pub content_hash: String,
    pub status: AssetGateStatus,
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub issues: Vec<AssetGateIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageHardGateReport {
    pub schema_version: String,
    pub status: AssetGateStatus,
    pub items: Vec<ImageHardGateItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StyleAnchorCandidate {
    pub asset_id: String,
    pub image_path: String,
    pub content_hash: String,
    pub gate_status: AssetGateStatus,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StyleAnchorSet {
    pub schema_version: String,
    pub status: String,
    pub confirmation_mode: String,
    pub reviewer: String,
    pub notes: String,
    pub anchors: Vec<StyleAnchorCandidate>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step07ArtDirectionOutput {
    pub status: String,
    pub art_direction_spec: ArtDirectionSpec,
    pub representative_tasks: Vec<RepresentativeAssetTask>,
    pub hard_gate_report: ImageHardGateReport,
    pub anchor_candidates: Vec<StyleAnchorCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlm_review_report: Option<VlmReviewReport>,
    pub output_paths: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VlmReviewStatus {
    Passed,
    Failed,
    Unavailable,
}

impl Default for VlmReviewStatus {
    fn default() -> Self {
        Self::Unavailable
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlmReviewRequest {
    pub asset_id: String,
    pub image_path: PathBuf,
    pub content_hash: String,
    pub source_refs: Vec<String>,
    pub review_context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VlmReviewEvidence {
    pub image_hash: String,
    pub config_id: String,
    pub summary_hash: String,
    #[serde(default)]
    pub status: VlmReviewStatus,
    #[serde(default)]
    pub reviewer_kind: String,
    #[serde(default)]
    pub message: String,
    pub score: u8,
    pub differences: Vec<String>,
    pub cache_hit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VlmReviewItem {
    pub asset_id: String,
    pub image_path: String,
    pub source_refs: Vec<String>,
    pub evidence: VlmReviewEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VlmReviewReport {
    pub schema_version: String,
    pub compiler_version: String,
    pub config_id: String,
    pub status: VlmReviewStatus,
    pub reviewed_images: Vec<VlmReviewItem>,
    pub blocking_issues: Vec<String>,
}

pub trait VlmReviewService: Send + Sync + std::fmt::Debug {
    fn config_id(&self) -> &str;
    fn review_image(&self, request: &VlmReviewRequest) -> AdmResult<VlmReviewEvidence>;
}

pub trait VlmImageReviewer: Send + Sync + std::fmt::Debug {
    fn review_uncached(
        &self,
        request: &VlmReviewRequest,
        image_hash: &str,
        config_id: &str,
    ) -> AdmResult<VlmReviewEvidence>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableVlmImageReviewer;

impl VlmImageReviewer for UnavailableVlmImageReviewer {
    fn review_uncached(
        &self,
        request: &VlmReviewRequest,
        image_hash: &str,
        config_id: &str,
    ) -> AdmResult<VlmReviewEvidence> {
        let summary = format!(
            "{config_id}:{image_hash}:{}:vlm_review_unavailable",
            request.asset_id
        );
        Ok(VlmReviewEvidence {
            image_hash: image_hash.to_string(),
            config_id: config_id.to_string(),
            summary_hash: sha256_hex(summary.as_bytes()),
            status: VlmReviewStatus::Unavailable,
            reviewer_kind: "unconfigured".to_string(),
            message: "VLM review service is not configured for this product run".to_string(),
            score: 0,
            differences: vec!["VLM review was unavailable; acceptance is fail-closed.".to_string()],
            cache_hit: false,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CachedVlmReviewService {
    config_id: String,
    cache: Arc<Mutex<BTreeMap<String, VlmReviewEvidence>>>,
    cache_path: Option<PathBuf>,
    reviewer: Arc<dyn VlmImageReviewer>,
}

impl CachedVlmReviewService {
    pub fn new(config_id: impl Into<String>) -> Self {
        Self::with_reviewer(config_id, Arc::new(UnavailableVlmImageReviewer))
    }

    pub fn with_reviewer(
        config_id: impl Into<String>,
        reviewer: Arc<dyn VlmImageReviewer>,
    ) -> Self {
        Self {
            config_id: config_id.into(),
            cache: Arc::new(Mutex::new(BTreeMap::new())),
            cache_path: None,
            reviewer,
        }
    }

    pub fn with_cache_file(
        config_id: impl Into<String>,
        cache_path: impl Into<PathBuf>,
    ) -> AdmResult<Self> {
        Self::with_cache_file_and_reviewer(
            config_id,
            cache_path,
            Arc::new(UnavailableVlmImageReviewer),
        )
    }

    pub fn with_cache_file_and_reviewer(
        config_id: impl Into<String>,
        cache_path: impl Into<PathBuf>,
        reviewer: Arc<dyn VlmImageReviewer>,
    ) -> AdmResult<Self> {
        let cache_path = cache_path.into();
        Ok(Self {
            config_id: config_id.into(),
            cache: Arc::new(Mutex::new(load_vlm_review_cache(&cache_path)?)),
            cache_path: Some(cache_path),
            reviewer,
        })
    }

    pub fn review_image_path(&self, image_path: &Path) -> AdmResult<VlmReviewEvidence> {
        let request = VlmReviewRequest {
            asset_id: image_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("image")
                .to_string(),
            image_path: image_path.to_path_buf(),
            content_hash: String::new(),
            source_refs: Vec::new(),
            review_context: "direct image review".to_string(),
        };
        <Self as VlmReviewService>::review_image(self, &request)
    }

    fn cache_key(&self, image_hash: &str) -> String {
        format!("{}:{image_hash}", self.config_id)
    }

    fn cache_guard(&self) -> AdmResult<MutexGuard<'_, BTreeMap<String, VlmReviewEvidence>>> {
        self.cache
            .lock()
            .map_err(|_| AdmError::new("VLM review cache lock was poisoned"))
    }

    fn persist_cache(&self, cache: &BTreeMap<String, VlmReviewEvidence>) -> AdmResult<()> {
        let Some(cache_path) = self.cache_path.as_deref() else {
            return Ok(());
        };
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(cache)
            .map_err(|error| AdmError::new(format!("failed to serialize VLM cache: {error}")))?;
        let temp_path = cache_path.with_extension("json.tmp");
        std::fs::write(&temp_path, text + "\n")?;
        if cache_path.exists() {
            std::fs::remove_file(cache_path)?;
        }
        std::fs::rename(&temp_path, cache_path)?;
        Ok(())
    }
}

impl VlmReviewService for CachedVlmReviewService {
    fn config_id(&self) -> &str {
        &self.config_id
    }

    fn review_image(&self, request: &VlmReviewRequest) -> AdmResult<VlmReviewEvidence> {
        let image_hash = file_hash(&request.image_path)?;
        let cache_key = self.cache_key(&image_hash);
        if let Some(cached) = self.cache_guard()?.get(&cache_key) {
            let mut evidence = cached.clone();
            evidence.cache_hit = true;
            return Ok(evidence);
        }
        let mut evidence = self
            .reviewer
            .review_uncached(request, &image_hash, &self.config_id)?;
        evidence.image_hash = image_hash;
        evidence.config_id = self.config_id.clone();
        evidence.cache_hit = false;
        let mut cache = self.cache_guard()?;
        cache.insert(cache_key, evidence.clone());
        self.persist_cache(&cache)?;
        Ok(evidence)
    }
}

fn load_vlm_review_cache(cache_path: &Path) -> AdmResult<BTreeMap<String, VlmReviewEvidence>> {
    if !cache_path.is_file() {
        return Ok(BTreeMap::new());
    }
    let text = std::fs::read_to_string(cache_path).map_err(|error| {
        AdmError::new(format!(
            "failed to read VLM review cache {}: {error}",
            cache_path.display()
        ))
    })?;
    serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|error| {
        AdmError::new(format!(
            "invalid VLM review cache {}: {error}",
            cache_path.display()
        ))
    })
}

pub fn compile_step07_art_direction_from_json(
    game_spec_json: &str,
    out_dir: &Path,
) -> AdmResult<Step07ArtDirectionOutput> {
    let spec = parse_game_spec(game_spec_json).map_err(|error| AdmError::new(error.to_string()))?;
    compile_step07_art_direction(&spec, out_dir)
}

pub fn compile_step07_art_direction(
    spec: &GameSpec,
    out_dir: &Path,
) -> AdmResult<Step07ArtDirectionOutput> {
    compile_step07_art_direction_inner(spec, out_dir, None)
}

pub fn compile_step07_art_direction_with_vlm(
    spec: &GameSpec,
    out_dir: &Path,
    vlm: &dyn VlmReviewService,
) -> AdmResult<Step07ArtDirectionOutput> {
    compile_step07_art_direction_inner(spec, out_dir, Some(vlm))
}

fn compile_step07_art_direction_inner(
    spec: &GameSpec,
    out_dir: &Path,
    vlm: Option<&dyn VlmReviewService>,
) -> AdmResult<Step07ArtDirectionOutput> {
    std::fs::create_dir_all(out_dir)?;
    let art_spec = build_art_direction_spec(spec)?;
    let tasks = representative_asset_tasks(spec, &art_spec);
    let image_dir = out_dir.join("representative_assets");
    std::fs::create_dir_all(&image_dir)?;
    let mut image_paths = Vec::new();
    for (index, task) in tasks.iter().enumerate() {
        let image_path = image_dir.join(format!("{}.png", task.asset_id));
        write_representative_png(task, index, &image_path)?;
        image_paths.push(image_path);
    }
    let hard_gate_report = validate_anchor_images(&tasks, &image_paths)?;
    let anchor_candidates = hard_gate_report
        .items
        .iter()
        .map(|item| {
            let task = tasks
                .iter()
                .find(|task| task.asset_id == item.asset_id)
                .expect("hard gate item should match task");
            StyleAnchorCandidate {
                asset_id: item.asset_id.clone(),
                image_path: item.image_path.clone(),
                content_hash: item.content_hash.clone(),
                gate_status: item.status.clone(),
                source_refs: task.source_refs.clone(),
            }
        })
        .collect::<Vec<_>>();
    let vlm_review_report = vlm
        .map(|reviewer| review_anchor_candidates(reviewer, &anchor_candidates, &tasks))
        .transpose()?;
    let spec_path =
        io::write_json_serializable(&out_dir.join("art_direction_spec.json"), &art_spec)?;
    let task_path =
        io::write_json_serializable(&out_dir.join("representative_asset_tasks.json"), &tasks)?;
    let gate_path = io::write_json_serializable(
        &out_dir.join("asset_hard_gate_report.json"),
        &hard_gate_report,
    )?;
    let candidates_path = io::write_json_serializable(
        &out_dir.join("style_anchor_candidates.json"),
        &anchor_candidates,
    )?;
    let vlm_path = vlm_review_report
        .as_ref()
        .map(|report| {
            io::write_json_serializable(&out_dir.join("vlm_style_review_report.json"), report)
        })
        .transpose()?;
    let vlm_passed = vlm_review_report
        .as_ref()
        .map(|report| report.status == VlmReviewStatus::Passed)
        .unwrap_or(true);
    let output = Step07ArtDirectionOutput {
        status: if hard_gate_report.status == AssetGateStatus::Passed && vlm_passed {
            "waiting_attended_confirmation".to_string()
        } else {
            "blocked".to_string()
        },
        art_direction_spec: art_spec,
        representative_tasks: tasks,
        hard_gate_report,
        anchor_candidates,
        vlm_review_report,
        output_paths: {
            let mut paths = BTreeMap::from([
                ("artDirectionSpec".to_string(), path_string(&spec_path)),
                (
                    "representativeAssetTasks".to_string(),
                    path_string(&task_path),
                ),
                ("assetHardGateReport".to_string(), path_string(&gate_path)),
                (
                    "styleAnchorCandidates".to_string(),
                    path_string(&candidates_path),
                ),
            ]);
            if let Some(vlm_path) = vlm_path {
                paths.insert("vlmStyleReviewReport".to_string(), path_string(&vlm_path));
            }
            paths
        },
    };
    io::write_json_serializable(&out_dir.join("step07_art_direction_output.json"), &output)?;
    Ok(output)
}

pub fn confirm_style_anchors_attended(
    out_dir: &Path,
    reviewer: &str,
    notes: &str,
    mode: &str,
) -> AdmResult<StyleAnchorSet> {
    if mode != "attended" {
        return Err(AdmError::new(
            "Step07 style anchors require attended confirmation; auto_accept is forbidden",
        ));
    }
    let candidates = read_anchor_candidates(out_dir)?;
    if candidates.is_empty() {
        return Err(AdmError::new("style anchor candidates are missing"));
    }
    if candidates
        .iter()
        .any(|candidate| candidate.gate_status != AssetGateStatus::Passed)
    {
        return Err(AdmError::new(
            "style anchors cannot be confirmed while a hard gate item failed",
        ));
    }
    let vlm_report_path = out_dir.join("vlm_style_review_report.json");
    if vlm_report_path.is_file() {
        let text = std::fs::read_to_string(&vlm_report_path).map_err(|error| {
            AdmError::new(format!("failed to read Step07 VLM review report: {error}"))
        })?;
        let report: VlmReviewReport = serde_json::from_str(&text).map_err(|error| {
            AdmError::new(format!("failed to parse Step07 VLM review report: {error}"))
        })?;
        if report.status != VlmReviewStatus::Passed {
            return Err(AdmError::new(
                "style anchors cannot be confirmed before Step07 VLM review passes",
            ));
        }
    }
    let anchor_set = StyleAnchorSet {
        schema_version: "step07_style_anchor_set.v1".to_string(),
        status: "approved".to_string(),
        confirmation_mode: mode.to_string(),
        reviewer: reviewer.trim().to_string(),
        notes: notes.trim().to_string(),
        anchors: candidates,
    };
    io::write_json_serializable(&out_dir.join("style_anchor_set.json"), &anchor_set)?;
    Ok(anchor_set)
}

pub fn validate_anchor_images(
    tasks: &[RepresentativeAssetTask],
    image_paths: &[PathBuf],
) -> AdmResult<ImageHardGateReport> {
    if tasks.len() != image_paths.len() {
        return Err(AdmError::new("task/image count mismatch"));
    }
    let mut items = Vec::new();
    let mut seen_hashes = BTreeMap::<String, String>::new();
    for (task, image_path) in tasks.iter().zip(image_paths) {
        let mut issues = Vec::new();
        let file_size = validate_image_file_integrity(image_path, &mut issues);
        let image_hash = file_hash(image_path).unwrap_or_else(|error| {
            issues.push(gate_issue(
                "FILE_UNREADABLE",
                image_path,
                format!("image file could not be read: {error}"),
            ));
            String::new()
        });
        if !image_hash.is_empty() {
            if let Some(first_asset_id) =
                seen_hashes.insert(image_hash.clone(), task.asset_id.clone())
            {
                issues.push(gate_issue(
                    "DUPLICATE_IMAGE",
                    image_path,
                    format!("image duplicates asset {first_asset_id}"),
                ));
            }
        }
        let decoded = image::open(image_path);
        let (width, height, has_alpha) = match decoded {
            Ok(image) => {
                let dimensions = image.dimensions();
                let has_alpha = image.color().has_alpha();
                validate_decoded_file_integrity(image_path, &image, file_size, &mut issues);
                validate_decoded_image(task, image_path, &image, &mut issues);
                (dimensions.0, dimensions.1, has_alpha)
            }
            Err(error) => {
                issues.push(gate_issue(
                    "DECODE_FAILED",
                    image_path,
                    format!("image decode failed: {error}"),
                ));
                (0, 0, false)
            }
        };
        items.push(ImageHardGateItem {
            asset_id: task.asset_id.clone(),
            image_path: path_string(image_path),
            content_hash: image_hash,
            status: if issues.is_empty() {
                AssetGateStatus::Passed
            } else {
                AssetGateStatus::Failed
            },
            width,
            height,
            has_alpha,
            issues,
        });
    }
    let failed = items
        .iter()
        .any(|item| item.status == AssetGateStatus::Failed);
    Ok(ImageHardGateReport {
        schema_version: "step07_asset_hard_gate.v1".to_string(),
        status: if failed {
            AssetGateStatus::Failed
        } else {
            AssetGateStatus::Passed
        },
        items,
    })
}

pub fn review_anchor_candidates(
    vlm: &dyn VlmReviewService,
    anchor_candidates: &[StyleAnchorCandidate],
    tasks: &[RepresentativeAssetTask],
) -> AdmResult<VlmReviewReport> {
    let mut reviewed_images = Vec::new();
    let mut blocking_issues = Vec::new();
    for candidate in anchor_candidates {
        let task = tasks
            .iter()
            .find(|task| task.asset_id == candidate.asset_id)
            .ok_or_else(|| {
                AdmError::new(format!(
                    "Step07 VLM review could not find task for asset {}",
                    candidate.asset_id
                ))
            })?;
        let request = VlmReviewRequest {
            asset_id: candidate.asset_id.clone(),
            image_path: PathBuf::from(&candidate.image_path),
            content_hash: candidate.content_hash.clone(),
            source_refs: candidate.source_refs.clone(),
            review_context: format!(
                "Step07 style anchor candidate: {}; expected {}x{} {}; {}",
                task.title, task.expected_width, task.expected_height, task.asset_type, task.prompt
            ),
        };
        let evidence = vlm.review_image(&request)?;
        if evidence.status != VlmReviewStatus::Passed {
            blocking_issues.push(format!(
                "{}:{:?}:{}",
                candidate.asset_id, evidence.status, evidence.message
            ));
        }
        reviewed_images.push(VlmReviewItem {
            asset_id: candidate.asset_id.clone(),
            image_path: candidate.image_path.clone(),
            source_refs: candidate.source_refs.clone(),
            evidence,
        });
    }
    let status = if reviewed_images.is_empty() {
        VlmReviewStatus::Unavailable
    } else if reviewed_images
        .iter()
        .all(|item| item.evidence.status == VlmReviewStatus::Passed)
    {
        VlmReviewStatus::Passed
    } else if reviewed_images
        .iter()
        .any(|item| item.evidence.status == VlmReviewStatus::Failed)
    {
        VlmReviewStatus::Failed
    } else {
        VlmReviewStatus::Unavailable
    };
    Ok(VlmReviewReport {
        schema_version: "step07_vlm_review_report.v1".to_string(),
        compiler_version: STEP07_V2_COMPILER_VERSION.to_string(),
        config_id: vlm.config_id().to_string(),
        status,
        reviewed_images,
        blocking_issues,
    })
}

fn build_art_direction_spec(spec: &GameSpec) -> AdmResult<ArtDirectionSpec> {
    let source_hash = canonicalize_game_spec(spec)
        .map_err(|error| AdmError::new(format!("GameSpec hash failed: {error}")))?
        .content_hash;
    let presentation = spec
        .presentation
        .values()
        .next()
        .map(|item| item.summary.clone())
        .unwrap_or_else(|| spec.intent.summary.clone());
    let palette = palette_for_capabilities(spec, &source_hash);
    Ok(ArtDirectionSpec {
        schema_version: "step07_art_direction_spec.v1".to_string(),
        compiler_version: STEP07_V2_COMPILER_VERSION.to_string(),
        source_game_spec_hash: source_hash,
        style_summary: presentation,
        palette,
        composition_constraints: vec![
            "Readable silhouettes at gameplay zoom.".to_string(),
            "No watermark, no embedded text labels in representative assets.".to_string(),
            "Transparent margins preserved for sprites and UI cutouts.".to_string(),
        ],
        usage_categories: vec![
            "core_character_or_unit".to_string(),
            "enemy_or_pressure".to_string(),
            "hud_or_status_ui".to_string(),
            "style_keyframe".to_string(),
        ],
        source_refs: spec
            .presentation
            .keys()
            .map(|id| format!("presentation:{id}"))
            .chain(
                spec.intent
                    .experience_promises
                    .keys()
                    .map(|id| format!("intent:{id}")),
            )
            .collect(),
    })
}

fn palette_for_capabilities(spec: &GameSpec, source_hash: &str) -> Vec<String> {
    let accent_index = source_hash.as_bytes().first().copied().unwrap_or_default() as usize % 3;
    let accent = ["#3BA99C", "#7AA95C", "#6D8FD6"][accent_index];
    let warning = if spec.capabilities.information.visibility
        == adm_new_game_spec::InformationVisibility::Partial
    {
        "#D97941"
    } else {
        "#E45757"
    };
    vec![
        "#243447".to_string(),
        accent.to_string(),
        "#F5C451".to_string(),
        warning.to_string(),
    ]
}

fn representative_asset_tasks(
    spec: &GameSpec,
    art_spec: &ArtDirectionSpec,
) -> Vec<RepresentativeAssetTask> {
    let mut tasks = Vec::new();
    let entity_by_tag = |tag: &str| {
        spec.entities
            .iter()
            .find(|(_, entity)| entity.tags.contains(tag))
            .map(|(id, entity)| (id.to_string(), entity.summary.clone()))
    };
    let guardian = entity_by_tag("guardian")
        .or_else(|| entity_by_tag("player_controlled"))
        .unwrap_or_else(|| ("core_unit".to_string(), "Core playable unit".to_string()));
    let enemy = entity_by_tag("enemy")
        .or_else(|| entity_by_tag("hostile"))
        .unwrap_or_else(|| {
            (
                "pressure_unit".to_string(),
                "Readable pressure unit".to_string(),
            )
        });
    tasks.push(asset_task(
        "anchor_core_guardian",
        "Core guardian sprite",
        "sprite",
        &guardian.1,
        256,
        256,
        vec![format!("entity:{}", guardian.0)],
        art_spec,
    ));
    tasks.push(asset_task(
        "anchor_enemy_pressure",
        "Enemy pressure sprite",
        "sprite",
        &enemy.1,
        256,
        256,
        vec![format!("entity:{}", enemy.0)],
        art_spec,
    ));
    tasks.push(asset_task(
        "anchor_hud_status",
        "HUD status strip",
        "ui",
        "Resource, wave, pause, and rejection feedback readable during play.",
        512,
        192,
        vec!["presentation:feedback_language".to_string()],
        art_spec,
    ));
    tasks.push(asset_task(
        "anchor_style_keyframe",
        "Style keyframe",
        "keyframe",
        &spec.intent.summary,
        640,
        384,
        art_spec.source_refs.clone(),
        art_spec,
    ));
    tasks
}

fn asset_task(
    asset_id: &str,
    title: &str,
    asset_type: &str,
    description: &str,
    width: u32,
    height: u32,
    source_refs: Vec<String>,
    art_spec: &ArtDirectionSpec,
) -> RepresentativeAssetTask {
    RepresentativeAssetTask {
        asset_id: asset_id.to_string(),
        title: title.to_string(),
        asset_type: asset_type.to_string(),
        expected_width: width,
        expected_height: height,
        require_alpha: true,
        transparent_margin_px: 8,
        prompt: format!(
            "{}. {}. Palette: {}. No watermark, no logo, no text.",
            title,
            description,
            art_spec.palette.join(", ")
        ),
        negative_prompt: "watermark, logo, text, unreadable silhouette, cropped transparent edge"
            .to_string(),
        source_refs,
    }
}

fn write_representative_png(
    task: &RepresentativeAssetTask,
    index: usize,
    path: &Path,
) -> AdmResult<()> {
    let mut image = RgbaImage::from_pixel(
        task.expected_width,
        task.expected_height,
        Rgba([0, 0, 0, 0]),
    );
    let colors = [
        Rgba([42u8, 132u8, 89u8, 255u8]),
        Rgba([202u8, 80u8, 70u8, 255u8]),
        Rgba([242u8, 190u8, 83u8, 255u8]),
        Rgba([45u8, 90u8, 145u8, 255u8]),
    ];
    let fill = colors[index % colors.len()];
    let margin = task.transparent_margin_px.max(8);
    for y in margin..task.expected_height.saturating_sub(margin) {
        for x in margin..task.expected_width.saturating_sub(margin) {
            let stripe = ((x / 16) + (y / 16) + index as u32).is_multiple_of(3);
            let pixel = if stripe {
                Rgba([238u8, 244u8, 218u8, 255u8])
            } else {
                fill
            };
            image.put_pixel(x, y, pixel);
        }
    }
    image
        .save(path)
        .map_err(|error| AdmError::new(format!("failed to write representative PNG: {error}")))
}

fn validate_decoded_image(
    task: &RepresentativeAssetTask,
    image_path: &Path,
    image: &DynamicImage,
    issues: &mut Vec<AssetGateIssue>,
) {
    let (width, height) = image.dimensions();
    if width != task.expected_width || height != task.expected_height {
        issues.push(gate_issue(
            "DIMENSION_MISMATCH",
            image_path,
            format!(
                "expected {}x{}, got {}x{}",
                task.expected_width, task.expected_height, width, height
            ),
        ));
    }
    if task.require_alpha && !image.color().has_alpha() {
        issues.push(gate_issue(
            "ALPHA_CHANNEL_MISSING",
            image_path,
            "image must preserve an alpha channel",
        ));
    }
    let rgba = image.to_rgba8();
    if task.require_alpha && !transparent_margin_ok(&rgba, task.transparent_margin_px) {
        issues.push(gate_issue(
            "TRANSPARENT_MARGIN_MISSING",
            image_path,
            "required transparent edge margin is missing",
        ));
    }
    if contrast_span(&rgba) < 42.0 {
        issues.push(gate_issue(
            "CONTRAST_TOO_LOW",
            image_path,
            "non-transparent pixels do not have enough luminance contrast",
        ));
    }
    validate_slice_geometry(task, image_path, width, height, issues);
    if watermark_like_mark(&rgba) {
        issues.push(gate_issue(
            "WATERMARK_LIKE_MARK",
            image_path,
            "edge high-contrast mark resembles a watermark or text label",
        ));
    }
}

fn validate_image_file_integrity(
    image_path: &Path,
    issues: &mut Vec<AssetGateIssue>,
) -> Option<u64> {
    let metadata = match std::fs::metadata(image_path) {
        Ok(metadata) => metadata,
        Err(error) => {
            issues.push(gate_issue(
                "FILE_METADATA_UNREADABLE",
                image_path,
                format!("image metadata could not be read: {error}"),
            ));
            return None;
        }
    };
    if !metadata.is_file() {
        issues.push(gate_issue(
            "FILE_NOT_REGULAR",
            image_path,
            "image path must point to a regular file",
        ));
        return Some(metadata.len());
    }
    let file_size = metadata.len();
    if file_size == 0 {
        issues.push(gate_issue("FILE_EMPTY", image_path, "image file is empty"));
    }
    if image_path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
    {
        match std::fs::read(image_path) {
            Ok(bytes) if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") => {
                issues.push(gate_issue(
                    "PNG_SIGNATURE_INVALID",
                    image_path,
                    "png file does not start with a valid PNG signature",
                ));
            }
            Ok(_) | Err(_) => {}
        }
    }
    Some(file_size)
}

fn validate_decoded_file_integrity(
    image_path: &Path,
    image: &DynamicImage,
    file_size: Option<u64>,
    issues: &mut Vec<AssetGateIssue>,
) {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        issues.push(gate_issue(
            "IMAGE_DIMENSION_ZERO",
            image_path,
            "decoded image dimensions must be non-zero",
        ));
    }
    let Some(file_size) = file_size else {
        return;
    };
    if file_size < 64 {
        issues.push(gate_issue(
            "FILE_TOO_SMALL",
            image_path,
            format!("decoded image file is implausibly small: {file_size} bytes"),
        ));
    }
    let pixel_count = u64::from(width).saturating_mul(u64::from(height));
    let max_reasonable_size = pixel_count.saturating_mul(32).max(256 * 1024);
    if file_size > max_reasonable_size {
        issues.push(gate_issue(
            "FILE_SIZE_IMPLAUSIBLE",
            image_path,
            format!(
                "decoded image file is too large for {}x{}: {} bytes",
                width, height, file_size
            ),
        ));
    }
}

fn validate_slice_geometry(
    task: &RepresentativeAssetTask,
    image_path: &Path,
    width: u32,
    height: u32,
    issues: &mut Vec<AssetGateIssue>,
) {
    let kind = task.asset_type.trim().to_ascii_lowercase();
    let bad = match kind.as_str() {
        "single_sprite" | "sprite" => (width != height || width < 64)
            .then_some("single sprite assets must be square and at least 64px on each side"),
        "nine_slice_ui" => {
            let margin = task.transparent_margin_px.max(8);
            (width <= margin.saturating_mul(4) || height <= margin.saturating_mul(4))
                .then_some("nine-slice UI assets must leave a non-empty center after edge margins")
        }
        "full_frame" | "keyframe" => (width < height || width < 320 || height < 180).then_some(
            "full-frame reference assets must be landscape-oriented and at least 320x180",
        ),
        _ => None,
    };
    if let Some(message) = bad {
        issues.push(gate_issue("SLICE_GEOMETRY_MISMATCH", image_path, message));
    }
}

fn transparent_margin_ok(image: &RgbaImage, margin: u32) -> bool {
    if margin == 0 {
        return true;
    }
    if margin.saturating_mul(2) >= image.width() || margin.saturating_mul(2) >= image.height() {
        return false;
    }
    for y in 0..image.height() {
        for x in 0..image.width() {
            let in_margin = x < margin
                || y < margin
                || x >= image.width().saturating_sub(margin)
                || y >= image.height().saturating_sub(margin);
            if in_margin && image.get_pixel(x, y).0[3] != 0 {
                return false;
            }
        }
    }
    true
}

fn contrast_span(image: &RgbaImage) -> f32 {
    let mut min_luma = f32::MAX;
    let mut max_luma = f32::MIN;
    for pixel in image.pixels().filter(|pixel| pixel.0[3] > 0) {
        let luma =
            0.2126 * pixel.0[0] as f32 + 0.7152 * pixel.0[1] as f32 + 0.0722 * pixel.0[2] as f32;
        min_luma = min_luma.min(luma);
        max_luma = max_luma.max(luma);
    }
    if min_luma == f32::MAX {
        0.0
    } else {
        max_luma - min_luma
    }
}

fn watermark_like_mark(image: &RgbaImage) -> bool {
    bottom_right_dark_mark(image) || edge_text_like_mark(image)
}

fn bottom_right_dark_mark(image: &RgbaImage) -> bool {
    let start_x = image.width() * 3 / 4;
    let start_y = image.height() * 3 / 4;
    let mut opaque = 0usize;
    let mut very_dark = 0usize;
    for y in start_y..image.height() {
        for x in start_x..image.width() {
            let pixel = image.get_pixel(x, y).0;
            if pixel[3] == 0 {
                continue;
            }
            opaque += 1;
            let luma =
                0.2126 * pixel[0] as f32 + 0.7152 * pixel[1] as f32 + 0.0722 * pixel[2] as f32;
            if luma < 32.0 {
                very_dark += 1;
            }
        }
    }
    opaque > 32 && very_dark * 3 > opaque
}

fn edge_text_like_mark(image: &RgbaImage) -> bool {
    if image.width() < 32 || image.height() < 32 {
        return false;
    }
    let width = image.width();
    let height = image.height();
    let zones = [
        (0, 0, width, height / 5),
        (0, height.saturating_mul(4) / 5, width, height),
        (0, 0, width / 5, height),
        (width.saturating_mul(4) / 5, 0, width, height),
    ];
    zones.into_iter().any(|zone| text_like_zone(image, zone))
}

fn text_like_zone(image: &RgbaImage, zone: (u32, u32, u32, u32)) -> bool {
    let (start_x, start_y, end_x, end_y) = zone;
    if end_x <= start_x || end_y <= start_y {
        return false;
    }
    let zone_width = (end_x - start_x) as usize;
    let zone_height = (end_y - start_y) as usize;
    let zone_area = zone_width.saturating_mul(zone_height);
    let mut ink_pixels = 0usize;
    let mut rows_with_runs = 0usize;
    for y in start_y..end_y {
        let mut row_ink = 0usize;
        let mut row_runs = 0usize;
        let mut in_run = false;
        for x in start_x..end_x {
            let is_ink = watermark_text_ink(image.get_pixel(x, y).0);
            if is_ink {
                ink_pixels += 1;
                row_ink += 1;
                if !in_run {
                    row_runs += 1;
                    in_run = true;
                }
            } else {
                in_run = false;
            }
        }
        if row_ink >= 4 && row_runs >= 2 && row_ink * 2 <= zone_width {
            rows_with_runs += 1;
        }
    }
    let mut columns_with_runs = 0usize;
    for x in start_x..end_x {
        let mut column_ink = 0usize;
        let mut column_runs = 0usize;
        let mut in_run = false;
        for y in start_y..end_y {
            let is_ink = watermark_text_ink(image.get_pixel(x, y).0);
            if is_ink {
                column_ink += 1;
                if !in_run {
                    column_runs += 1;
                    in_run = true;
                }
            } else {
                in_run = false;
            }
        }
        if column_ink >= 4 && column_runs >= 1 && column_ink * 3 <= zone_height * 2 {
            columns_with_runs += 1;
        }
    }
    ink_pixels >= 18 && ink_pixels * 4 <= zone_area && rows_with_runs >= 4 && columns_with_runs >= 4
}

fn watermark_text_ink(pixel: [u8; 4]) -> bool {
    if pixel[3] < 200 {
        return false;
    }
    let luma = 0.2126 * pixel[0] as f32 + 0.7152 * pixel[1] as f32 + 0.0722 * pixel[2] as f32;
    luma < 45.0 || luma > 245.0
}

fn read_anchor_candidates(out_dir: &Path) -> AdmResult<Vec<StyleAnchorCandidate>> {
    let path = out_dir.join("style_anchor_candidates.json");
    let text = std::fs::read_to_string(&path).map_err(|error| {
        AdmError::new(format!("failed to read style anchor candidates: {error}"))
    })?;
    serde_json::from_str(&text)
        .map_err(|error| AdmError::new(format!("failed to parse style anchor candidates: {error}")))
}

fn file_hash(path: &Path) -> AdmResult<String> {
    let bytes = std::fs::read(path)
        .map_err(|error| AdmError::new(format!("failed to read image bytes: {error}")))?;
    Ok(sha256_hex(&bytes))
}

fn gate_issue(code: &str, path: &Path, message: impl Into<String>) -> AssetGateIssue {
    AssetGateIssue {
        code: code.to_string(),
        path: path_string(path),
        message: message.into(),
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct PassingTestVlmReviewer;

    impl VlmImageReviewer for PassingTestVlmReviewer {
        fn review_uncached(
            &self,
            request: &VlmReviewRequest,
            image_hash: &str,
            config_id: &str,
        ) -> AdmResult<VlmReviewEvidence> {
            let summary = format!("{config_id}:{image_hash}:{}:passed", request.asset_id);
            Ok(VlmReviewEvidence {
                image_hash: image_hash.to_string(),
                config_id: config_id.to_string(),
                summary_hash: sha256_hex(summary.as_bytes()),
                status: VlmReviewStatus::Passed,
                reviewer_kind: "scripted_test_vlm".to_string(),
                message: "scripted VLM reviewer accepted the image".to_string(),
                score: 95,
                differences: Vec::new(),
                cache_hit: false,
            })
        }
    }

    fn r1_fixture() -> GameSpec {
        parse_game_spec(include_str!(
            "../../../../testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"
        ))
        .unwrap()
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(adm_new_foundation::new_stable_id(prefix).unwrap());
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn r1c0_fixture_compiles_art_direction_and_attended_anchor_set() {
        let root = temp_root("step07_v2_r1c0");
        let output = compile_step07_art_direction(&r1_fixture(), &root).unwrap();

        assert_eq!(output.status, "waiting_attended_confirmation");
        assert_eq!(output.representative_tasks.len(), 4);
        assert_eq!(output.hard_gate_report.status, AssetGateStatus::Passed);
        assert!(root.join("art_direction_spec.json").exists());
        assert!(root.join("style_anchor_candidates.json").exists());

        let rejected = confirm_style_anchors_attended(&root, "tester", "approved", "auto_accept");
        assert!(rejected.is_err());

        let anchors =
            confirm_style_anchors_attended(&root, "tester", "approved", "attended").unwrap();
        assert_eq!(anchors.status, "approved");
        assert_eq!(anchors.confirmation_mode, "attended");
        assert_eq!(anchors.anchors.len(), 4);
        assert!(root.join("style_anchor_set.json").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn hard_gate_rejects_bad_samples() {
        let root = temp_root("step07_v2_bad_samples");
        let task = RepresentativeAssetTask {
            asset_id: "bad".to_string(),
            title: "Bad".to_string(),
            asset_type: "sprite".to_string(),
            expected_width: 128,
            expected_height: 128,
            require_alpha: true,
            transparent_margin_px: 8,
            prompt: String::new(),
            negative_prompt: String::new(),
            source_refs: Vec::new(),
        };
        let wrong_size = root.join("wrong_size.png");
        RgbaImage::from_pixel(64, 128, Rgba([80, 80, 80, 255]))
            .save(&wrong_size)
            .unwrap();
        let no_alpha = root.join("no_alpha.png");
        RgbImage::from_pixel(128, 128, Rgb([80, 80, 80]))
            .save(&no_alpha)
            .unwrap();
        let duplicate_a = root.join("duplicate_a.png");
        let duplicate_b = root.join("duplicate_b.png");
        write_representative_png(&task, 0, &duplicate_a).unwrap();
        std::fs::copy(&duplicate_a, &duplicate_b).unwrap();
        let watermark = root.join("watermark.png");
        let mut watermarked = RgbaImage::from_pixel(128, 128, Rgba([0, 0, 0, 0]));
        for y in 12..116 {
            for x in 12..116 {
                watermarked.put_pixel(x, y, Rgba([140, 220, 120, 255]));
            }
        }
        for y in 104..120 {
            for x in 88..120 {
                watermarked.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        watermarked.save(&watermark).unwrap();

        let report = validate_anchor_images(
            &[task.clone(), task.clone(), task.clone(), task.clone(), task],
            &[wrong_size, no_alpha, duplicate_a, duplicate_b, watermark],
        )
        .unwrap();
        let codes = report
            .items
            .iter()
            .flat_map(|item| item.issues.iter().map(|issue| issue.code.as_str()))
            .collect::<BTreeSet<_>>();

        assert_eq!(report.status, AssetGateStatus::Failed);
        assert!(codes.contains("DIMENSION_MISMATCH"));
        assert!(codes.contains("ALPHA_CHANNEL_MISSING"));
        assert!(codes.contains("DUPLICATE_IMAGE"));
        assert!(codes.contains("WATERMARK_LIKE_MARK"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn hard_gate_rejects_file_integrity_failures() {
        let root = temp_root("step07_v2_file_integrity");
        let task = RepresentativeAssetTask {
            asset_id: "bad_file".to_string(),
            title: "Bad file".to_string(),
            asset_type: "sprite".to_string(),
            expected_width: 128,
            expected_height: 128,
            require_alpha: true,
            transparent_margin_px: 8,
            prompt: String::new(),
            negative_prompt: String::new(),
            source_refs: Vec::new(),
        };
        let empty = root.join("empty.png");
        std::fs::write(&empty, []).unwrap();
        let invalid_png = root.join("invalid.png");
        std::fs::write(&invalid_png, b"not a png image").unwrap();

        let report = validate_anchor_images(&[task.clone(), task], &[empty, invalid_png]).unwrap();
        let codes = report
            .items
            .iter()
            .flat_map(|item| item.issues.iter().map(|issue| issue.code.as_str()))
            .collect::<BTreeSet<_>>();

        assert_eq!(report.status, AssetGateStatus::Failed);
        assert!(codes.contains("FILE_EMPTY"));
        assert!(codes.contains("PNG_SIGNATURE_INVALID"));
        assert!(codes.contains("DECODE_FAILED"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn hard_gate_rejects_text_like_watermark_outside_bottom_right() {
        let root = temp_root("step07_v2_text_watermark");
        let task = RepresentativeAssetTask {
            asset_id: "text_mark".to_string(),
            title: "Text mark".to_string(),
            asset_type: "sprite".to_string(),
            expected_width: 128,
            expected_height: 128,
            require_alpha: true,
            transparent_margin_px: 8,
            prompt: String::new(),
            negative_prompt: String::new(),
            source_refs: Vec::new(),
        };
        let image_path = root.join("text_mark.png");
        let mut image = RgbaImage::from_pixel(128, 128, Rgba([0, 0, 0, 0]));
        for y in 8..120 {
            for x in 8..120 {
                image.put_pixel(x, y, Rgba([128, 218, 130, 255]));
            }
        }
        for y in 104..116 {
            for x in 16..18 {
                image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
            for x in 24..26 {
                image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
            for x in 32..34 {
                image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        image.save(&image_path).unwrap();

        let report = validate_anchor_images(&[task], &[image_path]).unwrap();

        assert_eq!(report.status, AssetGateStatus::Failed);
        assert!(
            report.items[0]
                .issues
                .iter()
                .any(|issue| issue.code == "WATERMARK_LIKE_MARK")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn hard_gate_rejects_invalid_slice_geometry() {
        let root = temp_root("step07_v2_slice_geometry");
        let task = RepresentativeAssetTask {
            asset_id: "bad_slice".to_string(),
            title: "Bad slice".to_string(),
            asset_type: "single_sprite".to_string(),
            expected_width: 128,
            expected_height: 96,
            require_alpha: true,
            transparent_margin_px: 8,
            prompt: String::new(),
            negative_prompt: String::new(),
            source_refs: Vec::new(),
        };
        let image_path = root.join("bad_slice.png");
        let mut image = RgbaImage::from_pixel(128, 96, Rgba([0, 0, 0, 0]));
        for y in 8..88 {
            for x in 8..120 {
                image.put_pixel(x, y, Rgba([80, 160, 220, 255]));
            }
        }
        image.save(&image_path).unwrap();

        let report = validate_anchor_images(&[task], &[image_path]).unwrap();

        assert_eq!(report.status, AssetGateStatus::Failed);
        assert!(
            report.items[0]
                .issues
                .iter()
                .any(|issue| issue.code == "SLICE_GEOMETRY_MISMATCH")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn hard_gate_rejects_declared_margin_that_cannot_fit_image_geometry() {
        let root = temp_root("step07_v2_margin_geometry");
        let task = RepresentativeAssetTask {
            asset_id: "bad_margin".to_string(),
            title: "Bad margin".to_string(),
            asset_type: "sprite".to_string(),
            expected_width: 16,
            expected_height: 16,
            require_alpha: true,
            transparent_margin_px: 8,
            prompt: String::new(),
            negative_prompt: String::new(),
            source_refs: Vec::new(),
        };
        let image_path = root.join("bad_margin.png");
        RgbaImage::from_pixel(16, 16, Rgba([0, 0, 0, 0]))
            .save(&image_path)
            .unwrap();

        let report = validate_anchor_images(&[task], &[image_path]).unwrap();

        assert_eq!(report.status, AssetGateStatus::Failed);
        assert!(
            report.items[0]
                .issues
                .iter()
                .any(|issue| issue.code == "TRANSPARENT_MARGIN_MISSING")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn vlm_review_reuses_cache_by_image_hash() {
        let root = temp_root("step07_v2_vlm_cache");
        let output = compile_step07_art_direction(&r1_fixture(), &root).unwrap();
        let image_path = PathBuf::from(&output.anchor_candidates[0].image_path);
        let service = CachedVlmReviewService::with_reviewer(
            "vlm-reviewer-test",
            Arc::new(PassingTestVlmReviewer),
        );

        let first = service.review_image_path(&image_path).unwrap();
        let second = service.review_image_path(&image_path).unwrap();

        assert!(!first.cache_hit);
        assert!(second.cache_hit);
        assert_eq!(first.status, VlmReviewStatus::Passed);
        assert_eq!(first.image_hash, second.image_hash);
        assert_eq!(first.config_id, "vlm-reviewer-test");
        assert_eq!(first.summary_hash, second.summary_hash);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn vlm_review_persists_cache_across_service_instances() {
        let root = temp_root("step07_v2_vlm_persistent_cache");
        let output = compile_step07_art_direction(&r1_fixture(), &root).unwrap();
        let image_path = PathBuf::from(&output.anchor_candidates[0].image_path);
        let cache_path = root.join("vlm_review_cache/reviews.json");
        let first_service = CachedVlmReviewService::with_cache_file_and_reviewer(
            "vlm-reviewer-test",
            &cache_path,
            Arc::new(PassingTestVlmReviewer),
        )
        .unwrap();

        let first = first_service.review_image_path(&image_path).unwrap();
        let second_service = CachedVlmReviewService::with_cache_file_and_reviewer(
            "vlm-reviewer-test",
            &cache_path,
            Arc::new(PassingTestVlmReviewer),
        )
        .unwrap();
        let second = second_service.review_image_path(&image_path).unwrap();

        assert!(!first.cache_hit);
        assert!(second.cache_hit);
        assert_eq!(first.summary_hash, second.summary_hash);
        assert!(cache_path.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn compile_with_unconfigured_vlm_blocks_and_writes_review_report() {
        let root = temp_root("step07_v2_unconfigured_vlm");
        let service = CachedVlmReviewService::new("unconfigured-test-vlm");

        let output = compile_step07_art_direction_with_vlm(&r1_fixture(), &root, &service).unwrap();

        assert_eq!(output.status, "blocked");
        assert_eq!(
            output.vlm_review_report.as_ref().unwrap().status,
            VlmReviewStatus::Unavailable
        );
        assert!(root.join("vlm_style_review_report.json").exists());
        assert!(confirm_style_anchors_attended(&root, "tester", "must fail", "attended").is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
