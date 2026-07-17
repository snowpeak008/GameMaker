use adm_new_foundation::{AdmError, AdmResult, new_stable_id, sha256_hex};
use adm_new_game_spec::{GameSpec, parse_game_spec};
use adm_new_pipeline::stages::step07_v2::{
    AssetGateStatus, StyleAnchorCandidate, VlmReviewEvidence, VlmReviewRequest, VlmReviewService,
    VlmReviewStatus, compile_step07_art_direction, confirm_style_anchors_attended,
};
use adm_new_pipeline::stages::step08_10_v2::{FrozenAssetManifest, compile_step08_10};
use adm_new_pipeline::stages::step12_v2::{
    AssetBindingReference, AssetProductionPolicy, Step12Status, WorkspaceReferenceAssetLoader,
    discover_asset_bindings_from_workspace, run_step12_asset_production,
    run_step12_asset_production_with_vlm, validate_asset_binding_graph,
};
use image::{Rgba, RgbaImage};
use std::fs;

#[derive(Debug)]
struct FailingAssetVlmReviewer;

impl VlmReviewService for FailingAssetVlmReviewer {
    fn config_id(&self) -> &str {
        "step12-failing-test-vlm"
    }

    fn review_image(&self, request: &VlmReviewRequest) -> AdmResult<VlmReviewEvidence> {
        let bytes = std::fs::read(&request.image_path).map_err(|error| {
            AdmError::new(format!(
                "test VLM reviewer could not read {}: {error}",
                request.image_path.display()
            ))
        })?;
        let image_hash = sha256_hex(&bytes);
        let summary = format!("{}:{}:failed", request.asset_id, image_hash);
        Ok(VlmReviewEvidence {
            image_hash,
            config_id: self.config_id().to_string(),
            summary_hash: sha256_hex(summary.as_bytes()),
            status: VlmReviewStatus::Failed,
            reviewer_kind: "scripted_test_vlm".to_string(),
            message: "scripted VLM reviewer rejected the generated asset".to_string(),
            score: 20,
            differences: vec!["asset readability is below the scripted threshold".to_string()],
            cache_hit: false,
        })
    }
}

#[test]
fn r1c0_fixture_produces_imports_binds_and_loads_all_assets() {
    let root = temp_root("step12_v2_r1c0");
    let (manifest, anchors) = r1_manifest_and_anchors(&root);
    let policy = AssetProductionPolicy::attended_approved(
        manifest.items.iter().map(|item| item.asset_id.clone()),
    );
    let (bindings, loader) = workspace_bindings(&root, &manifest);
    let output = run_step12_asset_production(
        &manifest,
        &anchors,
        &root.join("step12"),
        &policy,
        &bindings,
        &loader,
    )
    .unwrap();

    assert_eq!(output.status, Step12Status::Success);
    assert_eq!(output.produced_assets.len(), manifest.items.len());
    assert_eq!(output.binding_report.status, AssetGateStatus::Passed);
    assert!(output.correction_queue.is_empty());
    assert!(
        root.join("step12/raw_generated_asset_manifest.json")
            .exists()
    );
    assert!(root.join("step12/asset_binding_graph.json").exists());
    assert!(root.join("step12/engine_load_binding_report.json").exists());
    assert!(
        output
            .produced_assets
            .iter()
            .all(|asset| asset.confirmation_status == "approved")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn binding_validation_fails_orphan_and_missing_reference_samples() {
    let root = temp_root("step12_v2_binding_negative");
    let (manifest, anchors) = r1_manifest_and_anchors(&root);
    let policy = AssetProductionPolicy::attended_approved(
        manifest.items.iter().map(|item| item.asset_id.clone()),
    );
    let (bindings, loader) = workspace_bindings(&root, &manifest);
    let output = run_step12_asset_production(
        &manifest,
        &anchors,
        &root.join("step12"),
        &policy,
        &bindings,
        &loader,
    )
    .unwrap();

    let orphan_report = validate_asset_binding_graph(&output.produced_assets, &[], &loader);
    assert_eq!(orphan_report.status, AssetGateStatus::Failed);
    assert_eq!(
        orphan_report.orphan_assets.len(),
        output.produced_assets.len()
    );

    let mut missing_bindings = output.bindings.clone();
    missing_bindings.push(AssetBindingReference {
        asset_id: "missing_asset".to_string(),
        reference_kind: "prefab".to_string(),
        reference_path: "Assets/AutoDesign/Prefabs/Missing.prefab".to_string(),
        runtime_loader_id: "gameplay_sprite_loader".to_string(),
    });
    let missing_report =
        validate_asset_binding_graph(&output.produced_assets, &missing_bindings, &loader);
    assert_eq!(missing_report.status, AssetGateStatus::Failed);
    assert_eq!(
        missing_report.missing_referenced_assets,
        vec!["missing_asset".to_string()]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asset_production_requires_explicit_runtime_references() {
    let root = temp_root("step12_v2_requires_runtime_refs");
    let (manifest, anchors) = r1_manifest_and_anchors(&root);
    let policy = AssetProductionPolicy::attended_approved(
        manifest.items.iter().map(|item| item.asset_id.clone()),
    );
    let workspace_root = root.join("empty_workspace_target");
    fs::create_dir_all(&workspace_root).unwrap();
    let loader = WorkspaceReferenceAssetLoader::new(&workspace_root);

    let output = run_step12_asset_production(
        &manifest,
        &anchors,
        &root.join("step12"),
        &policy,
        &[],
        &loader,
    )
    .unwrap();

    assert_eq!(output.status, Step12Status::CorrectionRequired);
    assert_eq!(output.binding_report.status, AssetGateStatus::Failed);
    assert_eq!(
        output.binding_report.orphan_assets.len(),
        manifest.items.len()
    );
    assert!(
        output
            .correction_queue
            .iter()
            .any(|item| item.reason.contains("no scene or prefab reference"))
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sample_confirmation_only_blocks_unapproved_sampled_assets() {
    let root = temp_root("step12_v2_sample_confirmation");
    let (manifest, anchors) = r1_manifest_and_anchors(&root);
    let (bindings, loader) = workspace_bindings(&root, &manifest);
    let sample_count = 2.min(manifest.items.len());

    let pending_output = run_step12_asset_production(
        &manifest,
        &anchors,
        &root.join("step12_pending"),
        &AssetProductionPolicy::sample(sample_count, std::iter::empty::<String>()),
        &bindings,
        &loader,
    )
    .unwrap();

    assert_eq!(pending_output.status, Step12Status::WaitingConfirmation);
    assert_eq!(
        pending_output
            .confirmation_records
            .iter()
            .filter(|record| record.sampled)
            .count(),
        sample_count
    );
    let sampled_asset_ids = pending_output
        .confirmation_records
        .iter()
        .filter(|record| record.sampled)
        .map(|record| {
            assert_eq!(record.strategy, "sample");
            assert_eq!(record.required, format!("sample({sample_count})"));
            assert_eq!(record.status, "pending");
            assert!(record.requires_human);
            assert!(record.sample_index.is_some());
            assert_eq!(record.sample_count, sample_count);
            record.asset_id.clone()
        })
        .collect::<Vec<_>>();
    assert!(
        pending_output
            .confirmation_records
            .iter()
            .filter(|record| !record.sampled)
            .all(|record| {
                record.required == "not_required"
                    && record.status == "approved"
                    && !record.requires_human
                    && record.sample_index.is_none()
            })
    );
    let confirmation_report =
        fs::read_to_string(root.join("step12_pending/asset_confirmation_report.json")).unwrap();
    assert!(confirmation_report.contains("\"sampled\""));
    assert!(confirmation_report.contains("\"sampleIndex\""));

    let output = run_step12_asset_production(
        &manifest,
        &anchors,
        &root.join("step12_approved"),
        &AssetProductionPolicy::sample(sample_count, sampled_asset_ids),
        &bindings,
        &loader,
    )
    .unwrap();

    assert_eq!(output.status, Step12Status::Success);
    assert_eq!(
        output
            .confirmation_records
            .iter()
            .filter(|record| record.sampled && record.status == "approved")
            .count(),
        sample_count
    );
    assert!(
        output
            .produced_assets
            .iter()
            .all(|asset| asset.confirmation_status == "approved")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn style_anchor_drift_enters_correction_queue() {
    let root = temp_root("step12_v2_style_drift");
    let (manifest, _) = r1_manifest_and_anchors(&root);
    let black_anchor_path = root.join("black_anchor.png");
    RgbaImage::from_pixel(64, 64, Rgba([1, 1, 1, 255]))
        .save(&black_anchor_path)
        .unwrap();
    let anchors = vec![StyleAnchorCandidate {
        asset_id: "black_anchor".to_string(),
        image_path: black_anchor_path.to_string_lossy().replace('\\', "/"),
        content_hash: "0".repeat(64),
        gate_status: AssetGateStatus::Passed,
        source_refs: vec!["test:black_anchor".to_string()],
    }];
    let mut policy = AssetProductionPolicy::attended_approved(
        manifest.items.iter().map(|item| item.asset_id.clone()),
    );
    policy.style_distance_threshold = 10.0;
    let (bindings, loader) = workspace_bindings(&root, &manifest);

    let output = run_step12_asset_production(
        &manifest,
        &anchors,
        &root.join("step12"),
        &policy,
        &bindings,
        &loader,
    )
    .unwrap();

    assert_eq!(output.status, Step12Status::CorrectionRequired);
    assert!(
        output
            .correction_queue
            .iter()
            .any(|item| item.reason.contains("style anchors"))
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn vlm_failure_enters_correction_queue_and_writes_audit_report() {
    let root = temp_root("step12_v2_vlm_failure");
    let (manifest, anchors) = r1_manifest_and_anchors(&root);
    let policy = AssetProductionPolicy::attended_approved(
        manifest.items.iter().map(|item| item.asset_id.clone()),
    );
    let (bindings, loader) = workspace_bindings(&root, &manifest);

    let output = run_step12_asset_production_with_vlm(
        &manifest,
        &anchors,
        &root.join("step12"),
        &policy,
        &bindings,
        &loader,
        &FailingAssetVlmReviewer,
    )
    .unwrap();

    assert_eq!(output.status, Step12Status::CorrectionRequired);
    assert_eq!(
        output.vlm_review_report.as_ref().unwrap().status,
        VlmReviewStatus::Failed
    );
    assert!(
        output
            .produced_assets
            .iter()
            .all(|asset| asset.vlm_status == VlmReviewStatus::Failed)
    );
    assert!(
        output
            .correction_queue
            .iter()
            .any(|item| item.reason.contains("VLM review"))
    );
    assert!(root.join("step12/vlm_asset_review_report.json").exists());
    let _ = std::fs::remove_dir_all(root);
}

fn r1_manifest_and_anchors(
    root: &std::path::Path,
) -> (FrozenAssetManifest, Vec<StyleAnchorCandidate>) {
    let step07_dir = root.join("step07");
    compile_step07_art_direction(&r1_fixture(), &step07_dir).unwrap();
    let anchors = confirm_style_anchors_attended(&step07_dir, "tester", "approved", "attended")
        .unwrap()
        .anchors;
    let manifest = compile_step08_10(&r1_fixture(), &anchors, &root.join("step08_10"))
        .unwrap()
        .asset_manifest;
    (manifest, anchors)
}

fn workspace_bindings(
    root: &std::path::Path,
    manifest: &FrozenAssetManifest,
) -> (Vec<AssetBindingReference>, WorkspaceReferenceAssetLoader) {
    let workspace_root = root.join("workspace_target");
    let prefab_dir = workspace_root.join("Assets/AutoDesign/Prefabs");
    let scene_dir = workspace_root.join("Assets/AutoDesign/Scenes");
    fs::create_dir_all(&prefab_dir).unwrap();
    fs::create_dir_all(&scene_dir).unwrap();
    let mut prefab = String::new();
    let mut scene = String::new();
    for item in &manifest.items {
        let imported_path = format!("Assets/AutoDesign/Art/Generated/{}.png", item.asset_id);
        if item.slice == "full_frame" {
            scene.push_str(&format!(
                "asset:{}\npath:{}\n",
                item.asset_id, imported_path
            ));
        } else {
            prefab.push_str(&format!(
                "asset:{}\npath:{}\n",
                item.asset_id, imported_path
            ));
        }
    }
    fs::write(prefab_dir.join("GeneratedBindings.prefab"), prefab).unwrap();
    fs::write(scene_dir.join("StyleReference.unity"), scene).unwrap();
    let bindings = discover_asset_bindings_from_workspace(manifest, &workspace_root).unwrap();
    (
        bindings,
        WorkspaceReferenceAssetLoader::new(&workspace_root),
    )
}

fn r1_fixture() -> GameSpec {
    parse_game_spec(include_str!(
        "../../../testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"
    ))
    .unwrap()
}

fn temp_root(prefix: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
    std::fs::create_dir_all(&root).unwrap();
    root
}
