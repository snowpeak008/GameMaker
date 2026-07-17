use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use adm_new_contracts::pipeline::{
    PipelineCheckpoint, PipelineCheckpointStatus, PipelineResumePolicy, PipelineUnitCheckpoint,
    PipelineUnitStatus, StageContextModel, StageStatus,
};
use adm_new_contracts::project::ProjectState;
use adm_new_foundation::{
    AdmError, AdmResult, new_stable_id, paths::SourceProjectRoot, sha256_hex,
};
use adm_new_game_spec::parse_game_spec;
use adm_new_pipeline::stages::step07_v2::{
    VlmReviewEvidence, VlmReviewRequest, VlmReviewService, VlmReviewStatus,
};
use adm_new_pipeline::stages::step08_10_v2::{FrozenAssetManifest, Step08_10Compilation};
use adm_new_pipeline::stages::step11_v2::{
    STEP11_V2_ENGINE_VERSION, Step11ExecutionReport, Step11ExecutionStatus,
};
use adm_new_pipeline::stages::step12_v2::Step12AssetProductionOutput;
use adm_new_pipeline::stages::step13_v2::{Step13ExecutionEvidence, compute_step13_build_hash};
use adm_new_pipeline::{ProductPipelineExecutor, StageExecutor, default_development_registry};
use serde_json::json;

#[derive(Debug)]
struct PassingProductVlmReviewer;

impl VlmReviewService for PassingProductVlmReviewer {
    fn config_id(&self) -> &str {
        "product-test-vlm"
    }

    fn review_image(&self, request: &VlmReviewRequest) -> AdmResult<VlmReviewEvidence> {
        let bytes = fs::read(&request.image_path).map_err(|error| {
            AdmError::new(format!(
                "test VLM reviewer could not read {}: {error}",
                request.image_path.display()
            ))
        })?;
        let image_hash = sha256_hex(&bytes);
        let summary = format!(
            "product-test-vlm:{}:{}:{}",
            request.asset_id, image_hash, request.review_context
        );
        Ok(VlmReviewEvidence {
            image_hash,
            config_id: self.config_id().to_string(),
            summary_hash: sha256_hex(summary.as_bytes()),
            status: VlmReviewStatus::Passed,
            reviewer_kind: "scripted_test_vlm".to_string(),
            message: "scripted product test VLM accepted the image".to_string(),
            score: 96,
            differences: Vec::new(),
            cache_hit: false,
        })
    }
}

fn v2_executor(root: &Path, design_data_dir: &Path) -> ProductPipelineExecutor {
    ProductPipelineExecutor::with_design_data_dir(root, "session_a", design_data_dir)
        .unwrap()
        .with_vlm_review_service(Arc::new(PassingProductVlmReviewer))
}

fn write_workspace_asset_references(root: &Path, session_id: &str, manifest: &FrozenAssetManifest) {
    let workspace_root = root
        .join("drafts")
        .join(session_id)
        .join("workspace/game_spec_v2/target");
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
}

#[test]
fn explicit_design_data_dir_works_with_a_separate_persistent_root() {
    let root = temp_root("product_pipeline_explicit_data");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let design_data_dir = source_root.join("knowledge/design_data").unwrap();
    assert!(design_data_dir.join("domains").is_dir());

    let executor =
        ProductPipelineExecutor::with_design_data_dir(&root, "session_a", &design_data_dir)
            .unwrap();
    let mut state = ProjectState::empty();
    state.project_name = "Explicit Data Project".to_string();
    let concept_dir = executor.prepare_project_source(&state).unwrap();

    assert!(concept_dir.join("package_manifest.json").is_file());
    assert!(
        root.join("drafts/session_a/source_artifacts/devflow_Design_v2/structured/decisions.json")
            .is_file()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn game_spec_v2_switch_routes_product_step07_to_v2_outputs() {
    let root = temp_root("product_pipeline_v2_step07");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let repository_root = source_root.into_path();
    let design_data_dir = repository_root.join("knowledge/design_data");
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(
        root.join("settings/project_settings.json"),
        serde_json::to_vec_pretty(&json!({"game_spec_v2": {"enabled": true}})).unwrap(),
    )
    .unwrap();
    let executor = v2_executor(&root, &design_data_dir);
    let stage06 = executor.artifact_root().join("stage_06");
    fs::create_dir_all(&stage06).unwrap();
    fs::copy(
        repository_root.join("testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"),
        stage06.join("r1_frozen_game_spec.json"),
    )
    .unwrap();
    let registry = default_development_registry();
    let step07 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "07")
        .unwrap();

    let result = executor.execute(step07, &empty_context("07"));

    assert_eq!(result.status, StageStatus::WaitingConfirmation);
    assert!(
        executor
            .artifact_root()
            .join("stage_07/art_direction_spec.json")
            .exists()
    );
    assert!(
        executor
            .artifact_root()
            .join("stage_07/style_anchor_candidates.json")
            .exists()
    );
    assert!(
        !executor
            .artifact_root()
            .join("stage_07/style_options.json")
            .exists()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn game_spec_v2_product_step07_blocks_without_vlm_review_service() {
    let root = temp_root("product_pipeline_v2_step07_no_vlm");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let repository_root = source_root.into_path();
    let design_data_dir = repository_root.join("knowledge/design_data");
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(
        root.join("settings/project_settings.json"),
        serde_json::to_vec_pretty(&json!({"game_spec_v2": {"enabled": true}})).unwrap(),
    )
    .unwrap();
    let executor =
        ProductPipelineExecutor::with_design_data_dir(&root, "session_a", &design_data_dir)
            .unwrap();
    let stage06 = executor.artifact_root().join("stage_06");
    fs::create_dir_all(&stage06).unwrap();
    fs::copy(
        repository_root.join("testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"),
        stage06.join("r1_frozen_game_spec.json"),
    )
    .unwrap();
    let registry = default_development_registry();
    let step07 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "07")
        .unwrap();

    let result = executor.execute(step07, &empty_context("07"));

    assert_eq!(result.status, StageStatus::Blocked);
    assert!(
        executor
            .artifact_root()
            .join("stage_07/vlm_style_review_report.json")
            .exists()
    );
    assert!(
        executor
            .confirm_style("v2_anchor_set", "must fail")
            .is_err()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn game_spec_v2_product_step08_10_require_v2_style_anchors_and_compile_v2_outputs() {
    let root = temp_root("product_pipeline_v2_step08");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let repository_root = source_root.into_path();
    let design_data_dir = repository_root.join("knowledge/design_data");
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(
        root.join("settings/project_settings.json"),
        serde_json::to_vec_pretty(&json!({"game_spec_v2": {"enabled": true}})).unwrap(),
    )
    .unwrap();
    let executor = v2_executor(&root, &design_data_dir);
    let stage06 = executor.artifact_root().join("stage_06");
    fs::create_dir_all(&stage06).unwrap();
    fs::copy(
        repository_root.join("testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"),
        stage06.join("r1_frozen_game_spec.json"),
    )
    .unwrap();
    let registry = default_development_registry();
    let step07 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "07")
        .unwrap();
    let step08 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "08")
        .unwrap();

    let step07_result = executor.execute(step07, &empty_context("07"));
    assert_eq!(step07_result.status, StageStatus::WaitingConfirmation);
    executor
        .confirm_style("v2_anchor_set", "approved by product test")
        .unwrap();
    let step08_result = executor.execute(step08, &empty_context("08"));

    assert_eq!(step08_result.status, StageStatus::Success);
    assert!(
        executor
            .artifact_root()
            .join("stage_08/trusted_task_graph.json")
            .exists()
    );
    assert!(
        executor
            .artifact_root()
            .join("stage_08/game_spec_v2_stage_report.json")
            .exists()
    );
    assert!(
        !executor
            .artifact_root()
            .join("stage_08/production_plan.json")
            .exists()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn game_spec_v2_product_step11_rejects_stage10_source_hash_drift_without_overwrite() {
    let root = temp_root("product_pipeline_v2_step10_hash_drift");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let repository_root = source_root.into_path();
    let design_data_dir = repository_root.join("knowledge/design_data");
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(
        root.join("settings/project_settings.json"),
        serde_json::to_vec_pretty(&json!({"game_spec_v2": {"enabled": true}})).unwrap(),
    )
    .unwrap();
    let executor = v2_executor(&root, &design_data_dir);
    let stage06 = executor.artifact_root().join("stage_06");
    fs::create_dir_all(&stage06).unwrap();
    let frozen_spec_path = stage06.join("r1_frozen_game_spec.json");
    fs::copy(
        repository_root.join("testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"),
        &frozen_spec_path,
    )
    .unwrap();
    let registry = default_development_registry();
    let step07 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "07")
        .unwrap();
    let step10 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "10")
        .unwrap();
    let step11 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "11")
        .unwrap();

    assert_eq!(
        executor.execute(step07, &empty_context("07")).status,
        StageStatus::WaitingConfirmation
    );
    executor
        .confirm_style("v2_anchor_set", "approved by product test")
        .unwrap();
    assert_eq!(
        executor.execute(step10, &empty_context("10")).status,
        StageStatus::Success
    );
    let graph_path = executor
        .artifact_root()
        .join("stage_10/trusted_task_graph.json");
    let original_graph = fs::read_to_string(&graph_path).unwrap();
    let mut changed_spec: serde_json::Value =
        serde_json::from_slice(&fs::read(&frozen_spec_path).unwrap()).unwrap();
    changed_spec["intent"]["title"] = json!("Changed After Step10 Freeze");
    fs::write(
        &frozen_spec_path,
        serde_json::to_vec_pretty(&changed_spec).unwrap(),
    )
    .unwrap();

    let step11_result = executor.execute(step11, &empty_context("11"));

    assert_eq!(step11_result.status, StageStatus::Failed);
    assert_eq!(fs::read_to_string(&graph_path).unwrap(), original_graph);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn game_spec_v2_product_step11_runs_workspace_agent_after_v2_task_graph() {
    let root = temp_root("product_pipeline_v2_step11");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let repository_root = source_root.into_path();
    let design_data_dir = repository_root.join("knowledge/design_data");
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(
        root.join("settings/project_settings.json"),
        serde_json::to_vec_pretty(&json!({"game_spec_v2": {"enabled": true}})).unwrap(),
    )
    .unwrap();
    let executor = v2_executor(&root, &design_data_dir);
    let stage06 = executor.artifact_root().join("stage_06");
    fs::create_dir_all(&stage06).unwrap();
    fs::copy(
        repository_root.join("testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"),
        stage06.join("r1_frozen_game_spec.json"),
    )
    .unwrap();
    let registry = default_development_registry();
    let step07 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "07")
        .unwrap();
    let step08 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "08")
        .unwrap();
    let step11 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "11")
        .unwrap();

    assert_eq!(
        executor.execute(step07, &empty_context("07")).status,
        StageStatus::WaitingConfirmation
    );
    executor
        .confirm_style("v2_anchor_set", "approved by product test")
        .unwrap();
    assert_eq!(
        executor.execute(step08, &empty_context("08")).status,
        StageStatus::Success
    );
    let step11_result = executor.execute(step11, &empty_context("11"));

    assert_eq!(step11_result.status, StageStatus::Success);
    assert!(
        executor
            .artifact_root()
            .join("stage_11/step11_execution_report.json")
            .exists()
    );
    assert!(
        root.join("drafts/session_a/workspace/game_spec_v2/target")
            .is_dir()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn game_spec_v2_product_step11_requires_real_agent_when_product_mode_demands_it() {
    let root = temp_root("product_pipeline_v2_step11_requires_agent");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let repository_root = source_root.into_path();
    let design_data_dir = repository_root.join("knowledge/design_data");
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(
        root.join("settings/project_settings.json"),
        serde_json::to_vec_pretty(&json!({"game_spec_v2": {"enabled": true}})).unwrap(),
    )
    .unwrap();
    let executor = v2_executor(&root, &design_data_dir).require_workspace_task_agent();
    let stage06 = executor.artifact_root().join("stage_06");
    fs::create_dir_all(&stage06).unwrap();
    fs::copy(
        repository_root.join("testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"),
        stage06.join("r1_frozen_game_spec.json"),
    )
    .unwrap();
    let registry = default_development_registry();
    let step07 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "07")
        .unwrap();
    let step08 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "08")
        .unwrap();
    let step11 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "11")
        .unwrap();

    assert_eq!(
        executor.execute(step07, &empty_context("07")).status,
        StageStatus::WaitingConfirmation
    );
    executor
        .confirm_style("v2_anchor_set", "approved by product test")
        .unwrap();
    assert_eq!(
        executor.execute(step08, &empty_context("08")).status,
        StageStatus::Success
    );
    let step11_result = executor.execute(step11, &empty_context("11"));

    assert_eq!(step11_result.status, StageStatus::Failed);
    assert!(
        step11_result
            .errors
            .iter()
            .any(|error| error.contains("requires a configured WorkspaceTaskAgent"))
    );
    assert!(
        !root
            .join("drafts/session_a/workspace/game_spec_v2/target")
            .exists()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn game_spec_v2_product_step11_honors_shared_stop_token() {
    let root = temp_root("product_pipeline_v2_step11_stop");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let repository_root = source_root.into_path();
    let design_data_dir = repository_root.join("knowledge/design_data");
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(
        root.join("settings/project_settings.json"),
        serde_json::to_vec_pretty(&json!({"game_spec_v2": {"enabled": true}})).unwrap(),
    )
    .unwrap();
    let executor = v2_executor(&root, &design_data_dir);
    let stage06 = executor.artifact_root().join("stage_06");
    fs::create_dir_all(&stage06).unwrap();
    fs::copy(
        repository_root.join("testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"),
        stage06.join("r1_frozen_game_spec.json"),
    )
    .unwrap();
    let registry = default_development_registry();
    let step07 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "07")
        .unwrap();
    let step08 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "08")
        .unwrap();
    let step11 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "11")
        .unwrap();

    assert_eq!(
        executor.execute(step07, &empty_context("07")).status,
        StageStatus::WaitingConfirmation
    );
    executor
        .confirm_style("v2_anchor_set", "approved by product test")
        .unwrap();
    assert_eq!(
        executor.execute(step08, &empty_context("08")).status,
        StageStatus::Success
    );
    executor.work_unit_stop_token().request_stop();
    let stopped = executor.execute(step11, &empty_context("11"));

    assert_eq!(stopped.status, StageStatus::Stopped);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn game_spec_v2_product_step12_waits_for_explicit_asset_confirmation() {
    let root = temp_root("product_pipeline_v2_step12_confirmation");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let repository_root = source_root.into_path();
    let design_data_dir = repository_root.join("knowledge/design_data");
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(
        root.join("settings/project_settings.json"),
        serde_json::to_vec_pretty(&json!({"game_spec_v2": {"enabled": true}})).unwrap(),
    )
    .unwrap();
    let executor = v2_executor(&root, &design_data_dir);
    let stage06 = executor.artifact_root().join("stage_06");
    fs::create_dir_all(&stage06).unwrap();
    fs::copy(
        repository_root.join("testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"),
        stage06.join("r1_frozen_game_spec.json"),
    )
    .unwrap();
    let registry = default_development_registry();
    let step07 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "07")
        .unwrap();
    let step08 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "08")
        .unwrap();
    let step12 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "12")
        .unwrap();

    assert_eq!(
        executor.execute(step07, &empty_context("07")).status,
        StageStatus::WaitingConfirmation
    );
    executor
        .confirm_style("v2_anchor_set", "approved by product test")
        .unwrap();
    assert_eq!(
        executor.execute(step08, &empty_context("08")).status,
        StageStatus::Success
    );
    let compiled: Step08_10Compilation = serde_json::from_slice(
        &fs::read(
            executor
                .artifact_root()
                .join("stage_08/step08_10_compilation.json"),
        )
        .unwrap(),
    )
    .unwrap();
    write_workspace_asset_references(&root, "session_a", &compiled.asset_manifest);
    let waiting = executor.execute(step12, &empty_context("12"));
    assert_eq!(waiting.status, StageStatus::WaitingConfirmation);
    assert!(
        executor
            .artifact_root()
            .join("stage_12/vlm_asset_review_report.json")
            .exists()
    );
    fs::write(
        executor
            .artifact_root()
            .join("stage_12/asset_confirmation_approval.json"),
        serde_json::to_vec_pretty(&json!({
            "mode": "sample",
            "sampleCount": 2,
            "approvedAssetIds": []
        }))
        .unwrap(),
    )
    .unwrap();
    let sample_waiting = executor.execute(step12, &empty_context("12"));
    assert_eq!(sample_waiting.status, StageStatus::WaitingConfirmation);
    let step12_output: Step12AssetProductionOutput = serde_json::from_slice(
        &fs::read(
            executor
                .artifact_root()
                .join("stage_12/step12_asset_production_output.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let sampled_asset_ids = step12_output
        .confirmation_records
        .iter()
        .filter(|record| record.sampled)
        .map(|record| record.asset_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(sampled_asset_ids.len(), 2);
    assert!(
        step12_output
            .confirmation_records
            .iter()
            .filter(|record| record.sampled)
            .all(|record| record.requires_human && record.status == "pending")
    );
    fs::write(
        executor
            .artifact_root()
            .join("stage_12/asset_confirmation_approval.json"),
        serde_json::to_vec_pretty(&json!({
            "mode": "sample",
            "sampleCount": 2,
            "approvedAssetIds": sampled_asset_ids
        }))
        .unwrap(),
    )
    .unwrap();
    let sample_approved = executor.execute(step12, &empty_context("12"));
    assert_eq!(sample_approved.status, StageStatus::Success);

    fs::write(
        executor
            .artifact_root()
            .join("stage_12/asset_confirmation_approval.json"),
        serde_json::to_vec_pretty(&json!({
            "mode": "auto_accept",
            "explicitAutoAccept": true
        }))
        .unwrap(),
    )
    .unwrap();
    let approved = executor.execute(step12, &empty_context("12"));

    assert_eq!(approved.status, StageStatus::Success);
    assert!(
        executor
            .artifact_root()
            .join("stage_12/asset_confirmation_report.json")
            .exists()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn game_spec_v2_product_step13_requires_real_execution_evidence() {
    let root = temp_root("product_pipeline_v2_step13_evidence");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let repository_root = source_root.into_path();
    let design_data_dir = repository_root.join("knowledge/design_data");
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(
        root.join("settings/project_settings.json"),
        serde_json::to_vec_pretty(&json!({"game_spec_v2": {"enabled": true}})).unwrap(),
    )
    .unwrap();
    let executor = v2_executor(&root, &design_data_dir);
    let stage06 = executor.artifact_root().join("stage_06");
    fs::create_dir_all(&stage06).unwrap();
    let spec_path =
        repository_root.join("testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json");
    fs::copy(&spec_path, stage06.join("r1_frozen_game_spec.json")).unwrap();
    let spec = parse_game_spec(&fs::read_to_string(&spec_path).unwrap()).unwrap();
    let registry = default_development_registry();
    let step07 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "07")
        .unwrap();
    let step08 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "08")
        .unwrap();
    let step12 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "12")
        .unwrap();
    let step13 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "13")
        .unwrap();

    assert_eq!(
        executor.execute(step07, &empty_context("07")).status,
        StageStatus::WaitingConfirmation
    );
    executor
        .confirm_style("v2_anchor_set", "approved by product test")
        .unwrap();
    assert_eq!(
        executor.execute(step08, &empty_context("08")).status,
        StageStatus::Success
    );
    let compiled: Step08_10Compilation = serde_json::from_slice(
        &fs::read(
            executor
                .artifact_root()
                .join("stage_08/step08_10_compilation.json"),
        )
        .unwrap(),
    )
    .unwrap();
    write_workspace_asset_references(&root, "session_a", &compiled.asset_manifest);
    fs::create_dir_all(executor.artifact_root().join("stage_12")).unwrap();
    fs::write(
        executor
            .artifact_root()
            .join("stage_12/asset_confirmation_approval.json"),
        serde_json::to_vec_pretty(&json!({
            "mode": "auto_accept",
            "explicitAutoAccept": true
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        executor.execute(step12, &empty_context("12")).status,
        StageStatus::Success
    );
    let step11 = successful_product_step11_report(&compiled);
    fs::create_dir_all(executor.artifact_root().join("stage_11")).unwrap();
    fs::write(
        executor
            .artifact_root()
            .join("stage_11/step11_execution_report.json"),
        serde_json::to_vec_pretty(&step11).unwrap(),
    )
    .unwrap();

    let missing_evidence = executor.execute(step13, &empty_context("13"));
    assert_eq!(missing_evidence.status, StageStatus::Blocked);
    assert!(
        executor
            .artifact_root()
            .join("stage_13/scenario_execution_request.json")
            .exists()
    );
    let missing_report: serde_json::Value = serde_json::from_slice(
        &fs::read(
            executor
                .artifact_root()
                .join("stage_13/validation_report.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        missing_report["blocking_issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["message"]
                .as_str()
                .is_some_and(|message| message.contains("scenario execution evidence is missing")))
    );

    let step12_output: Step12AssetProductionOutput = serde_json::from_slice(
        &fs::read(
            executor
                .artifact_root()
                .join("stage_12/step12_asset_production_output.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let evidence = Step13ExecutionEvidence::test_only_nominal_for_spec(
        &spec,
        compute_step13_build_hash(&step11, &step12_output),
    );
    fs::write(
        executor
            .artifact_root()
            .join("stage_13/scenario_execution_evidence.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    let with_evidence = executor.execute(step13, &empty_context("13"));

    assert_eq!(with_evidence.status, StageStatus::Success);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn game_spec_v2_confirm_style_rejects_unknown_anchor_set_id() {
    let root = temp_root("product_pipeline_v2_confirm_style");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let repository_root = source_root.into_path();
    let design_data_dir = repository_root.join("knowledge/design_data");
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(
        root.join("settings/project_settings.json"),
        serde_json::to_vec_pretty(&json!({"game_spec_v2": {"enabled": true}})).unwrap(),
    )
    .unwrap();
    let executor = v2_executor(&root, &design_data_dir);
    let stage06 = executor.artifact_root().join("stage_06");
    fs::create_dir_all(&stage06).unwrap();
    fs::copy(
        repository_root.join("testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"),
        stage06.join("r1_frozen_game_spec.json"),
    )
    .unwrap();
    let registry = default_development_registry();
    let step07 = registry
        .stages
        .iter()
        .find(|stage| stage.stage_id == "07")
        .unwrap();

    assert_eq!(
        executor.execute(step07, &empty_context("07")).status,
        StageStatus::WaitingConfirmation
    );
    let result = executor.confirm_style("not_the_anchor_set", "should fail");

    assert!(result.is_err());
    assert!(
        !executor
            .artifact_root()
            .join("stage_07/style_anchor_set.json")
            .exists()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn game_spec_v2_checkpoint_recovery_rejects_legacy_work_unit_journals() {
    let root = temp_root("product_pipeline_v2_checkpoint_fail_closed");
    let source_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
    let repository_root = source_root.into_path();
    let design_data_dir = repository_root.join("knowledge/design_data");
    fs::create_dir_all(root.join("settings")).unwrap();
    fs::write(
        root.join("settings/project_settings.json"),
        serde_json::to_vec_pretty(&json!({"game_spec_v2_enabled": true})).unwrap(),
    )
    .unwrap();
    let executor =
        ProductPipelineExecutor::with_design_data_dir(&root, "session_a", &design_data_dir)
            .unwrap();
    let mut checkpoint = PipelineCheckpoint {
        status: PipelineCheckpointStatus::RecoveryBlocked,
        resume_policy: PipelineResumePolicy::Disabled,
        recovery_blocked_reason: "interrupted v2 Step11".to_string(),
        units: vec![PipelineUnitCheckpoint {
            stage_id: "11".to_string(),
            unit_id: "11:task".to_string(),
            status: PipelineUnitStatus::Unknown,
            reconcile_required: true,
            failure_message: "interrupted".to_string(),
            ..PipelineUnitCheckpoint::default()
        }],
        ..PipelineCheckpoint::default()
    };

    let result = executor.reconcile_checkpoint_work_units(&mut checkpoint, "11");

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("not compatible with legacy work-unit journals")
    );
    let _ = fs::remove_dir_all(root);
}

fn successful_product_step11_report(compiled: &Step08_10Compilation) -> Step11ExecutionReport {
    Step11ExecutionReport {
        schema_version: "step11_execution_report.v1".to_string(),
        engine_version: STEP11_V2_ENGINE_VERSION.to_string(),
        status: Step11ExecutionStatus::Success,
        starting_tree_hash: compiled.task_graph.source_game_spec_hash.clone(),
        ending_tree_hash: sha256_hex(
            format!(
                "{}:{}",
                compiled.task_graph.source_game_spec_hash, compiled.task_graph.semantic_hash
            )
            .as_bytes(),
        ),
        max_workers: 1,
        committed_task_ids: compiled
            .task_graph
            .tasks
            .iter()
            .map(|task| task.task_id.clone())
            .collect(),
        correction_queue: Vec::new(),
        task_reports: Vec::new(),
    }
}

fn empty_context(stage_id: &str) -> StageContextModel {
    StageContextModel {
        stage_id: stage_id.to_string(),
        project_root: String::new(),
        inputs: Default::default(),
        outputs: Default::default(),
        metadata: Default::default(),
        knowledge: Default::default(),
        skills: Default::default(),
        test_mode: false,
        artifact_dir: String::new(),
    }
}

fn temp_root(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
    fs::create_dir_all(&root).unwrap();
    root
}
