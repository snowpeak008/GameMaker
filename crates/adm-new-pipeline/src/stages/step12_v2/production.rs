use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use adm_new_foundation::{AdmError, AdmResult, io};

use crate::stages::step07_v2::{
    AssetGateStatus, ImageHardGateReport, StyleAnchorCandidate, VlmReviewItem, VlmReviewReport,
    VlmReviewRequest, VlmReviewService, VlmReviewStatus, validate_anchor_images,
};
use crate::stages::step08_10_v2::{AssetManifestItem, FrozenAssetManifest};
use crate::stages::step11_v2::Step11FailureKind;
use crate::stages::step12_v2::image_support::{
    anchor_average_colors, asset_task_from_manifest_item, min_style_distance, path_string,
    safe_asset_file_name, write_generated_asset_png,
};
use crate::stages::step12_v2::types::{
    AssetBindingReference, AssetBindingValidationReport, AssetConfirmationRecord,
    AssetProductionPolicy, ConfirmationStrategy, EngineAssetLoader, ProducedAssetRecord,
    STEP12_V2_COMPILER_VERSION, Step12AssetProductionOutput, Step12CorrectionQueueItem,
    Step12Status,
};

pub fn run_step12_asset_production(
    manifest: &FrozenAssetManifest,
    anchors: &[StyleAnchorCandidate],
    out_dir: &Path,
    policy: &AssetProductionPolicy,
    bindings: &[AssetBindingReference],
    loader: &dyn EngineAssetLoader,
) -> AdmResult<Step12AssetProductionOutput> {
    run_step12_asset_production_inner(manifest, anchors, out_dir, policy, bindings, loader, None)
}

pub fn run_step12_asset_production_with_vlm(
    manifest: &FrozenAssetManifest,
    anchors: &[StyleAnchorCandidate],
    out_dir: &Path,
    policy: &AssetProductionPolicy,
    bindings: &[AssetBindingReference],
    loader: &dyn EngineAssetLoader,
    vlm: &dyn VlmReviewService,
) -> AdmResult<Step12AssetProductionOutput> {
    run_step12_asset_production_inner(
        manifest,
        anchors,
        out_dir,
        policy,
        bindings,
        loader,
        Some(vlm),
    )
}

fn run_step12_asset_production_inner(
    manifest: &FrozenAssetManifest,
    anchors: &[StyleAnchorCandidate],
    out_dir: &Path,
    policy: &AssetProductionPolicy,
    bindings: &[AssetBindingReference],
    loader: &dyn EngineAssetLoader,
    vlm: Option<&dyn VlmReviewService>,
) -> AdmResult<Step12AssetProductionOutput> {
    if manifest.items.is_empty() {
        return Err(AdmError::new(
            "Step12 requires a non-empty FrozenAssetManifest",
        ));
    }
    if anchors.is_empty() {
        return Err(AdmError::new(
            "Step12 requires confirmed Step07 style anchors",
        ));
    }
    std::fs::create_dir_all(out_dir)?;
    let generated_dir = out_dir.join("generated_assets");
    std::fs::create_dir_all(&generated_dir)?;

    let tasks = manifest
        .items
        .iter()
        .map(asset_task_from_manifest_item)
        .collect::<Vec<_>>();
    let image_paths = tasks
        .iter()
        .enumerate()
        .map(|(index, task)| {
            let path = generated_dir.join(format!("{}.png", safe_asset_file_name(&task.asset_id)));
            write_generated_asset_png(task, index, &path)?;
            Ok(path)
        })
        .collect::<AdmResult<Vec<_>>>()?;
    let hard_gate_report = validate_anchor_images(&tasks, &image_paths)?;
    let mut produced_assets = produced_records(manifest, anchors, policy, &hard_gate_report)?;
    let vlm_review_report = vlm
        .map(|reviewer| review_produced_assets(reviewer, manifest, &mut produced_assets))
        .transpose()?;
    let bindings = bindings.to_vec();
    let binding_report = validate_asset_binding_graph(&produced_assets, &bindings, loader);
    let correction_queue = correction_queue(
        &produced_assets,
        &binding_report,
        policy,
        vlm_review_report.is_some(),
    );
    let confirmation_records = confirmation_records(&produced_assets, policy);
    let status = output_status(&produced_assets, &correction_queue);

    let raw_path = io::write_json_serializable(
        &out_dir.join("raw_generated_asset_manifest.json"),
        &produced_assets,
    )?;
    let processed_path = io::write_json_serializable(
        &out_dir.join("processed_asset_manifest.json"),
        &produced_assets,
    )?;
    let import_path =
        io::write_json_serializable(&out_dir.join("asset_import_report.json"), &produced_assets)?;
    let binding_path =
        io::write_json_serializable(&out_dir.join("asset_binding_graph.json"), &bindings)?;
    let load_path = io::write_json_serializable(
        &out_dir.join("engine_load_binding_report.json"),
        &binding_report,
    )?;
    let confirmation_path = io::write_json_serializable(
        &out_dir.join("asset_confirmation_report.json"),
        &confirmation_records,
    )?;
    let queue_path = io::write_json_serializable(
        &out_dir.join("asset_correction_queue.json"),
        &correction_queue,
    )?;
    let vlm_path = vlm_review_report
        .as_ref()
        .map(|report| {
            io::write_json_serializable(&out_dir.join("vlm_asset_review_report.json"), report)
        })
        .transpose()?;
    let output = Step12AssetProductionOutput {
        schema_version: "step12_asset_production.v1".to_string(),
        compiler_version: STEP12_V2_COMPILER_VERSION.to_string(),
        source_asset_manifest_hash: manifest.frozen_hash.clone(),
        status,
        produced_assets,
        confirmation_records,
        bindings,
        binding_report,
        vlm_review_report,
        correction_queue,
        output_paths: {
            let mut paths = BTreeMap::from([
                (
                    "rawGeneratedAssetManifest".to_string(),
                    path_string(&raw_path),
                ),
                (
                    "processedAssetManifest".to_string(),
                    path_string(&processed_path),
                ),
                ("assetImportReport".to_string(), path_string(&import_path)),
                ("assetBindingGraph".to_string(), path_string(&binding_path)),
                (
                    "engineLoadBindingReport".to_string(),
                    path_string(&load_path),
                ),
                (
                    "assetConfirmationReport".to_string(),
                    path_string(&confirmation_path),
                ),
                ("assetCorrectionQueue".to_string(), path_string(&queue_path)),
            ]);
            if let Some(vlm_path) = vlm_path {
                paths.insert("vlmAssetReviewReport".to_string(), path_string(&vlm_path));
            }
            paths
        },
    };
    io::write_json_serializable(
        &out_dir.join("step12_asset_production_output.json"),
        &output,
    )?;
    Ok(output)
}

fn review_produced_assets(
    vlm: &dyn VlmReviewService,
    manifest: &FrozenAssetManifest,
    produced_assets: &mut [ProducedAssetRecord],
) -> AdmResult<VlmReviewReport> {
    let mut reviewed_images = Vec::new();
    let mut blocking_issues = Vec::new();
    for asset in produced_assets {
        let manifest_item = manifest
            .items
            .iter()
            .find(|item| item.asset_id == asset.asset_id)
            .ok_or_else(|| {
                AdmError::new(format!(
                    "Step12 VLM review could not find manifest item for asset {}",
                    asset.asset_id
                ))
            })?;
        let request = VlmReviewRequest {
            asset_id: asset.asset_id.clone(),
            image_path: PathBuf::from(&asset.raw_path),
            content_hash: asset.content_hash.clone(),
            source_refs: manifest_item.source_refs.clone(),
            review_context: format!(
                "Step12 generated asset: {}; purpose {}; acceptance {}",
                manifest_item.asset_id,
                manifest_item.purpose,
                manifest_item.acceptance.join("; ")
            ),
        };
        let evidence = vlm.review_image(&request)?;
        asset.vlm_status = evidence.status;
        asset.vlm_score = evidence.score;
        asset.vlm_summary_hash = evidence.summary_hash.clone();
        if evidence.status != VlmReviewStatus::Passed {
            blocking_issues.push(format!(
                "{}:{:?}:{}",
                asset.asset_id, evidence.status, evidence.message
            ));
        }
        reviewed_images.push(VlmReviewItem {
            asset_id: asset.asset_id.clone(),
            image_path: asset.raw_path.clone(),
            source_refs: manifest_item.source_refs.clone(),
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
        schema_version: "step12_vlm_asset_review_report.v1".to_string(),
        compiler_version: STEP12_V2_COMPILER_VERSION.to_string(),
        config_id: vlm.config_id().to_string(),
        status,
        reviewed_images,
        blocking_issues,
    })
}

pub fn validate_asset_binding_graph(
    produced_assets: &[ProducedAssetRecord],
    bindings: &[AssetBindingReference],
    loader: &dyn EngineAssetLoader,
) -> AssetBindingValidationReport {
    let produced_ids = produced_assets
        .iter()
        .map(|asset| asset.asset_id.as_str())
        .collect::<BTreeSet<_>>();
    let referenced_ids = bindings
        .iter()
        .map(|binding| binding.asset_id.as_str())
        .collect::<BTreeSet<_>>();
    let orphan_assets = produced_ids
        .difference(&referenced_ids)
        .map(|id| (*id).to_string())
        .collect::<Vec<_>>();
    let missing_referenced_assets = referenced_ids
        .difference(&produced_ids)
        .map(|id| (*id).to_string())
        .collect::<Vec<_>>();
    let load_probes = produced_assets
        .iter()
        .map(|asset| loader.load(asset, bindings))
        .collect::<Vec<_>>();
    let load_failures = load_probes
        .iter()
        .filter(|probe| !probe.loaded || !probe.instantiated)
        .map(|probe| probe.asset_id.clone())
        .collect::<Vec<_>>();
    let status = if orphan_assets.is_empty()
        && missing_referenced_assets.is_empty()
        && load_failures.is_empty()
    {
        AssetGateStatus::Passed
    } else {
        AssetGateStatus::Failed
    };
    AssetBindingValidationReport {
        status,
        orphan_assets,
        missing_referenced_assets,
        load_failures,
        load_probes,
    }
}

pub fn discover_asset_bindings_from_workspace(
    manifest: &FrozenAssetManifest,
    workspace_root: &Path,
) -> AdmResult<Vec<AssetBindingReference>> {
    if !workspace_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut reference_files = Vec::new();
    collect_reference_files(workspace_root, workspace_root, 0, &mut reference_files)?;
    let mut seen = BTreeSet::<(String, String)>::new();
    let mut bindings = Vec::new();
    for reference_file in reference_files {
        let relative_path = path_string(reference_file.strip_prefix(workspace_root).map_err(
            |error| AdmError::new(format!("workspace reference path escaped root: {error}")),
        )?);
        let text = match std::fs::read_to_string(&reference_file) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let Some((reference_kind, loader_id)) = reference_file_kind(&reference_file) else {
            continue;
        };
        for item in &manifest.items {
            let imported_path = imported_asset_path(&item.asset_id);
            if text.contains(&item.asset_id) || text.contains(&imported_path) {
                let key = (item.asset_id.clone(), relative_path.clone());
                if seen.insert(key) {
                    bindings.push(AssetBindingReference {
                        asset_id: item.asset_id.clone(),
                        reference_kind: reference_kind.to_string(),
                        reference_path: relative_path.clone(),
                        runtime_loader_id: loader_id.to_string(),
                    });
                }
            }
        }
    }
    bindings.sort_by(|left, right| {
        left.asset_id
            .cmp(&right.asset_id)
            .then_with(|| left.reference_path.cmp(&right.reference_path))
    });
    Ok(bindings)
}

fn produced_records(
    manifest: &FrozenAssetManifest,
    anchors: &[StyleAnchorCandidate],
    policy: &AssetProductionPolicy,
    gate_report: &ImageHardGateReport,
) -> AdmResult<Vec<ProducedAssetRecord>> {
    let anchor_colors = anchor_average_colors(anchors)?;
    let sampled_asset_ids = sampled_asset_ids(manifest, policy);
    manifest
        .items
        .iter()
        .zip(&gate_report.items)
        .map(|(item, gate)| {
            let distance = min_style_distance(&gate.image_path, &anchor_colors)?;
            let confirmation_required = confirmation_required(item, policy, &sampled_asset_ids);
            let confirmation_status = if confirmation_required == "not_required"
                || policy.approved_asset_ids.contains(&item.asset_id)
            {
                "approved"
            } else {
                "pending"
            };
            Ok(ProducedAssetRecord {
                asset_id: item.asset_id.clone(),
                raw_path: gate.image_path.clone(),
                imported_path: imported_asset_path(&item.asset_id),
                content_hash: gate.content_hash.clone(),
                hard_gate_status: gate.status.clone(),
                vlm_status: VlmReviewStatus::Unavailable,
                vlm_score: 0,
                vlm_summary_hash: String::new(),
                style_distance: distance,
                style_status: if distance <= policy.style_distance_threshold {
                    "passed".to_string()
                } else {
                    "failed".to_string()
                },
                confirmation_required,
                confirmation_status: confirmation_status.to_string(),
                source_refs: item.source_refs.clone(),
            })
        })
        .collect()
}

fn correction_queue(
    produced_assets: &[ProducedAssetRecord],
    binding_report: &AssetBindingValidationReport,
    policy: &AssetProductionPolicy,
    vlm_required: bool,
) -> Vec<Step12CorrectionQueueItem> {
    let mut queue = Vec::new();
    for asset in produced_assets {
        if asset.hard_gate_status != AssetGateStatus::Passed {
            queue.push(correction(
                &asset.asset_id,
                Step11FailureKind::Test,
                "asset hard gate failed",
            ));
        }
        if asset.style_distance > policy.style_distance_threshold {
            queue.push(correction(
                &asset.asset_id,
                Step11FailureKind::Test,
                "asset drifted from the confirmed style anchors",
            ));
        }
        if vlm_required && asset.vlm_status != VlmReviewStatus::Passed {
            queue.push(correction(
                &asset.asset_id,
                Step11FailureKind::Evidence,
                "asset VLM review failed or was unavailable",
            ));
        }
    }
    for asset_id in &binding_report.orphan_assets {
        queue.push(correction(
            asset_id,
            Step11FailureKind::ScopeViolation,
            "asset has no scene or prefab reference",
        ));
    }
    for asset_id in &binding_report.missing_referenced_assets {
        queue.push(correction(
            asset_id,
            Step11FailureKind::Evidence,
            "binding graph references an asset that was not produced",
        ));
    }
    for asset_id in &binding_report.load_failures {
        queue.push(correction(
            asset_id,
            Step11FailureKind::Test,
            "headless asset load probe failed",
        ));
    }
    queue
}

fn output_status(
    produced_assets: &[ProducedAssetRecord],
    correction_queue: &[Step12CorrectionQueueItem],
) -> Step12Status {
    if !correction_queue.is_empty() {
        Step12Status::CorrectionRequired
    } else if produced_assets
        .iter()
        .any(|asset| asset.confirmation_status == "pending")
    {
        Step12Status::WaitingConfirmation
    } else {
        Step12Status::Success
    }
}

fn correction(
    asset_id: &str,
    failure_kind: Step11FailureKind,
    reason: &str,
) -> Step12CorrectionQueueItem {
    Step12CorrectionQueueItem {
        asset_id: asset_id.to_string(),
        failure_kind,
        reason: reason.to_string(),
    }
}

fn collect_reference_files(
    workspace_root: &Path,
    current: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
) -> AdmResult<()> {
    if depth > 12 || out.len() >= 4096 {
        return Ok(());
    }
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(
                name.as_ref(),
                "Library" | "Temp" | "Obj" | "Build" | "Builds" | "Logs" | ".git"
            ) {
                continue;
            }
            collect_reference_files(workspace_root, &path, depth + 1, out)?;
        } else if file_type.is_file() && reference_file_kind(&path).is_some() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.len() > 2 * 1024 * 1024 {
                    continue;
                }
            }
            if path.starts_with(workspace_root) {
                out.push(path);
            }
        }
    }
    Ok(())
}

fn reference_file_kind(path: &Path) -> Option<(&'static str, &'static str)> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("unity") => Some(("scene", "unity_scene_reference_loader")),
        Some("prefab") => Some(("prefab", "unity_prefab_reference_loader")),
        Some("asset") => Some(("asset", "unity_asset_reference_loader")),
        _ => None,
    }
}

fn imported_asset_path(asset_id: &str) -> String {
    format!("Assets/AutoDesign/Art/Generated/{asset_id}.png")
}

fn sampled_asset_ids(
    manifest: &FrozenAssetManifest,
    policy: &AssetProductionPolicy,
) -> BTreeSet<String> {
    if policy.strategy != ConfirmationStrategy::Sample {
        return BTreeSet::new();
    }
    let target_count = policy.sample_count.max(1).min(manifest.items.len());
    let mut selected = BTreeSet::new();
    let mut seen_slices = BTreeSet::new();
    for item in &manifest.items {
        if selected.len() >= target_count {
            break;
        }
        if seen_slices.insert(item.slice.clone()) {
            selected.insert(item.asset_id.clone());
        }
    }
    for item in &manifest.items {
        if selected.len() >= target_count {
            break;
        }
        selected.insert(item.asset_id.clone());
    }
    selected
}

fn confirmation_required(
    item: &AssetManifestItem,
    policy: &AssetProductionPolicy,
    sampled_asset_ids: &BTreeSet<String>,
) -> String {
    match policy.strategy {
        ConfirmationStrategy::Attended => "attended".to_string(),
        ConfirmationStrategy::Sample if sampled_asset_ids.contains(&item.asset_id) => {
            format!("sample({})", policy.sample_count.max(1))
        }
        ConfirmationStrategy::Sample => "not_required".to_string(),
    }
}

fn confirmation_records(
    assets: &[ProducedAssetRecord],
    policy: &AssetProductionPolicy,
) -> Vec<AssetConfirmationRecord> {
    let sample_count = if policy.strategy == ConfirmationStrategy::Sample {
        policy.sample_count.max(1).min(assets.len())
    } else {
        0
    };
    let mut sample_index = 0usize;
    assets
        .iter()
        .map(|asset| {
            let sampled = asset.confirmation_required.starts_with("sample(");
            let index = if sampled {
                sample_index += 1;
                Some(sample_index)
            } else {
                None
            };
            AssetConfirmationRecord {
                asset_id: asset.asset_id.clone(),
                strategy: strategy_name(&policy.strategy).to_string(),
                required: asset.confirmation_required.clone(),
                status: asset.confirmation_status.clone(),
                sampled,
                sample_index: index,
                sample_count,
                requires_human: asset.confirmation_status == "pending",
            }
        })
        .collect()
}

fn strategy_name(strategy: &ConfirmationStrategy) -> &'static str {
    match strategy {
        ConfirmationStrategy::Attended => "attended",
        ConfirmationStrategy::Sample => "sample",
    }
}
