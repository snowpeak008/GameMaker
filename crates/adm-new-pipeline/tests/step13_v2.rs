use adm_new_foundation::new_stable_id;
use adm_new_game_spec::{GameSpec, parse_game_spec};
use adm_new_pipeline::stages::step07_v2::{
    StyleAnchorCandidate, compile_step07_art_direction, confirm_style_anchors_attended,
};
use adm_new_pipeline::stages::step08_10_v2::{
    FrozenAssetManifest, TrustedTaskGraph, compile_step08_10,
};
use adm_new_pipeline::stages::step11_v2::{
    STEP11_V2_ENGINE_VERSION, Step11ExecutionReport, Step11ExecutionStatus,
};
use adm_new_pipeline::stages::step12_v2::{
    AssetProductionPolicy, Step12AssetProductionOutput, WorkspaceReferenceAssetLoader,
    discover_asset_bindings_from_workspace, run_step12_asset_production,
};
use adm_new_pipeline::stages::step13_v2::{
    ScenarioExecutionStatus, Step13ExecutionEvidence, Step13Status, Step13ValidationPolicy,
    compute_step13_build_hash, run_step13_acceptance_validation,
};

#[test]
fn r1c0_fixture_executes_all_acceptance_scenarios() {
    let root = temp_root("step13_v2_r1c0");
    let (spec, graph, step12) = r1_inputs(&root);
    let step11 = successful_step11_report(&graph);
    let policy = nominal_policy(&spec, &step11, &step12);

    let output =
        run_step13_acceptance_validation(&spec, &step11, &step12, &policy, &root.join("step13"))
            .unwrap();

    assert_eq!(output.status, Step13Status::Passed);
    assert_eq!(
        output.scenario_results.len(),
        spec.acceptance_scenarios.len()
    );
    assert!(
        output
            .scenario_results
            .iter()
            .all(|result| result.status == ScenarioExecutionStatus::Passed)
    );
    assert!(
        output
            .scenario_results
            .iter()
            .any(|result| !result.performance_checks.is_empty())
    );
    assert!(root.join("step13/scenario_execution_results.json").exists());
    assert!(
        root.join("step13/performance_validation_report.json")
            .exists()
    );
    assert!(root.join("step13/manual_review_report.json").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn manual_review_scenarios_wait_instead_of_silently_skipping() {
    let root = temp_root("step13_v2_manual");
    let (spec, graph, step12) = r1_inputs(&root);
    let step11 = successful_step11_report(&graph);
    let mut policy = nominal_policy(&spec, &step11, &step12);
    policy
        .execution_evidence
        .as_mut()
        .unwrap()
        .completed_manual_reviews
        .clear();

    let output =
        run_step13_acceptance_validation(&spec, &step11, &step12, &policy, &root.join("step13"))
            .unwrap();

    assert_eq!(output.status, Step13Status::WaitingManualReview);
    let manual = output
        .scenario_results
        .iter()
        .find(|result| result.scenario_id == "r1c0_visual_style_anchor_review")
        .unwrap();
    assert_eq!(manual.status, ScenarioExecutionStatus::ManualReviewRequired);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn injected_bad_build_fails_the_corresponding_scenarios() {
    let root = temp_root("step13_v2_bad_build");
    let (spec, graph, step12) = r1_inputs(&root);
    let step11 = successful_step11_report(&graph);
    let mut disabled_policy = nominal_policy(&spec, &step11, &step12);
    disabled_policy
        .execution_evidence
        .as_mut()
        .unwrap()
        .disabled_action_ids
        .insert("place_basic_guardian".to_string());

    let disabled = run_step13_acceptance_validation(
        &spec,
        &step11,
        &step12,
        &disabled_policy,
        &root.join("disabled_action"),
    )
    .unwrap();
    assert_eq!(disabled.status, Step13Status::Failed);
    let resource_loop = disabled
        .scenario_results
        .iter()
        .find(|result| result.scenario_id == "r1c0_resource_positive_loop")
        .unwrap();
    assert_eq!(resource_loop.status, ScenarioExecutionStatus::Failed);

    let mut missing_asset_policy = nominal_policy(&spec, &step11, &step12);
    missing_asset_policy
        .execution_evidence
        .as_mut()
        .unwrap()
        .missing_asset_ids
        .insert("sprite_basic_guardian".to_string());
    let missing_asset = run_step13_acceptance_validation(
        &spec,
        &step11,
        &step12,
        &missing_asset_policy,
        &root.join("missing_asset"),
    )
    .unwrap();
    assert_eq!(missing_asset.status, Step13Status::Failed);
    let asset_scenario = missing_asset
        .scenario_results
        .iter()
        .find(|result| result.scenario_id == "r1c0_asset_binding_no_orphans")
        .unwrap();
    assert_eq!(asset_scenario.status, ScenarioExecutionStatus::Failed);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn missing_asset_gate_uses_explicit_scenario_field_not_name_heuristics() {
    let root = temp_root("step13_v2_asset_field_only");
    let (mut spec, graph, step12) = r1_inputs(&root);
    for scenario in spec.acceptance_scenarios.values_mut() {
        scenario.asset_validation_required = false;
    }
    let step11 = successful_step11_report(&graph);
    let mut policy = nominal_policy(&spec, &step11, &step12);
    policy
        .execution_evidence
        .as_mut()
        .unwrap()
        .missing_asset_ids
        .insert("sprite_basic_guardian".to_string());

    let output =
        run_step13_acceptance_validation(&spec, &step11, &step12, &policy, &root.join("step13"))
            .unwrap();

    assert_eq!(output.status, Step13Status::Passed);
    let asset_named = output
        .scenario_results
        .iter()
        .find(|result| result.scenario_id == "r1c0_asset_binding_no_orphans")
        .unwrap();
    assert_eq!(asset_named.status, ScenarioExecutionStatus::Passed);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn missing_execution_evidence_fails_closed_and_writes_runner_request() {
    let root = temp_root("step13_v2_missing_evidence");
    let (spec, graph, step12) = r1_inputs(&root);
    let step11 = successful_step11_report(&graph);
    let policy = Step13ValidationPolicy::strict_unattended();

    let output =
        run_step13_acceptance_validation(&spec, &step11, &step12, &policy, &root.join("step13"))
            .unwrap();

    assert_eq!(output.status, Step13Status::Failed);
    assert!(
        output
            .scenario_results
            .iter()
            .any(|result| result.failure_reason.as_deref()
                == Some("scenario execution evidence is missing"))
    );
    assert!(root.join("step13/scenario_execution_request.json").exists());
    let _ = std::fs::remove_dir_all(root);
}

fn r1_inputs(root: &std::path::Path) -> (GameSpec, TrustedTaskGraph, Step12AssetProductionOutput) {
    let spec = r1_fixture();
    let (manifest, anchors, graph) = r1_manifest_anchors_graph(root, &spec);
    let policy = AssetProductionPolicy::attended_approved(
        manifest.items.iter().map(|item| item.asset_id.clone()),
    );
    let (bindings, loader) = workspace_bindings(root, &manifest);
    let step12 = run_step12_asset_production(
        &manifest,
        &anchors,
        &root.join("step12"),
        &policy,
        &bindings,
        &loader,
    )
    .unwrap();
    (spec, graph, step12)
}

fn r1_manifest_anchors_graph(
    root: &std::path::Path,
    spec: &GameSpec,
) -> (
    FrozenAssetManifest,
    Vec<StyleAnchorCandidate>,
    TrustedTaskGraph,
) {
    let step07_dir = root.join("step07");
    compile_step07_art_direction(spec, &step07_dir).unwrap();
    let anchors = confirm_style_anchors_attended(&step07_dir, "tester", "approved", "attended")
        .unwrap()
        .anchors;
    let compiled = compile_step08_10(spec, &anchors, &root.join("step08_10")).unwrap();
    (compiled.asset_manifest, anchors, compiled.task_graph)
}

fn workspace_bindings(
    root: &std::path::Path,
    manifest: &FrozenAssetManifest,
) -> (
    Vec<adm_new_pipeline::stages::step12_v2::AssetBindingReference>,
    WorkspaceReferenceAssetLoader,
) {
    let workspace_root = root.join("workspace_target");
    let prefab_dir = workspace_root.join("Assets/AutoDesign/Prefabs");
    let scene_dir = workspace_root.join("Assets/AutoDesign/Scenes");
    std::fs::create_dir_all(&prefab_dir).unwrap();
    std::fs::create_dir_all(&scene_dir).unwrap();
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
    std::fs::write(prefab_dir.join("GeneratedBindings.prefab"), prefab).unwrap();
    std::fs::write(scene_dir.join("StyleReference.unity"), scene).unwrap();
    let bindings = discover_asset_bindings_from_workspace(manifest, &workspace_root).unwrap();
    (
        bindings,
        WorkspaceReferenceAssetLoader::new(&workspace_root),
    )
}

fn successful_step11_report(graph: &TrustedTaskGraph) -> Step11ExecutionReport {
    Step11ExecutionReport {
        schema_version: "step11_execution_report.v1".to_string(),
        engine_version: STEP11_V2_ENGINE_VERSION.to_string(),
        status: Step11ExecutionStatus::Success,
        starting_tree_hash: graph.source_game_spec_hash.clone(),
        ending_tree_hash: canonical_build_hash(graph),
        max_workers: 1,
        committed_task_ids: graph
            .tasks
            .iter()
            .map(|task| task.task_id.clone())
            .collect(),
        correction_queue: Vec::new(),
        task_reports: Vec::new(),
    }
}

fn nominal_policy(
    spec: &GameSpec,
    step11: &Step11ExecutionReport,
    step12: &Step12AssetProductionOutput,
) -> Step13ValidationPolicy {
    Step13ValidationPolicy::from_execution_evidence(
        Step13ExecutionEvidence::test_only_nominal_for_spec(
            spec,
            compute_step13_build_hash(step11, step12),
        ),
    )
}

fn canonical_build_hash(graph: &TrustedTaskGraph) -> String {
    adm_new_foundation::sha256_hex(
        format!("{}:{}", graph.source_game_spec_hash, graph.semantic_hash).as_bytes(),
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
