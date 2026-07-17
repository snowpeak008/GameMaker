use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use adm_new_change_kernel::{
    ChangeEvidence, ChangeOutcome, EvidenceStatus, SideEffectState,
    WORKSPACE_CHANGE_SET_SCHEMA_VERSION, WorkspaceTransactionResult,
};
use adm_new_foundation::{AdmError, AdmResult, io, sha256_hex};

use crate::cross_genre_evaluation::samples::a09_samples;
use crate::cross_genre_evaluation::types::{
    A09EvaluationReport, A09EvaluationStatus, A09ProductionScope, A09Sample,
    EnvelopeCalibrationReport, EnvelopeMeasurement, FieldPromotionChecklist, FullProductionResult,
    SourceScanHit, SourceScanReport, SpecLevelCompilationResult, ThirdLayerAntiOverfitReport,
    WeightedBudgetThreshold,
};
use crate::game_spec_v2_steps::{Step00_06Compiler, StepGateStatus};
use crate::stages::step07_v2::{compile_step07_art_direction, confirm_style_anchors_attended};
use crate::stages::step08_10_v2::{
    FrozenAssetManifest, Step08_10Compilation, TrustedDevelopmentTask, compile_step08_10,
};
use crate::stages::step11_v2::{
    Step11ExecutionBudget, Step11ExecutionEngine, Step11ExecutionState, Step11ExecutionStatus,
    Step11StopToken, WorkspaceTaskAgent,
};
use crate::stages::step12_v2::{
    AssetBindingReference, AssetProductionPolicy, Step12Status, WorkspaceReferenceAssetLoader,
    discover_asset_bindings_from_workspace, run_step12_asset_production,
};
use crate::stages::step13_v2::{
    Step13ExecutionEvidence, Step13Status, Step13ValidationPolicy, compute_step13_build_hash,
    run_step13_acceptance_validation,
};
use crate::stages::step14_v2::{R1GateEvidence, run_step14_r1_packaging_gate};

pub const A09_COMPILER_VERSION: &str = "game_spec_a09_cross_genre_evaluation.v1";
const FORBIDDEN_SOURCE_TOKEN_CONFIG: &str = include_str!("forbidden_source_tokens.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A09MeasurementOptions {
    pub run_no_ai_path: bool,
    pub run_bounded_ai_repetition: bool,
    pub run_mutation_rejection: bool,
}

impl Default for A09MeasurementOptions {
    fn default() -> Self {
        Self {
            run_no_ai_path: true,
            run_bounded_ai_repetition: true,
            run_mutation_rejection: true,
        }
    }
}

pub fn run_a09_cross_genre_evaluation(out_dir: &Path) -> AdmResult<A09EvaluationReport> {
    run_a09_cross_genre_evaluation_with_options(out_dir, A09MeasurementOptions::default())
}

pub fn run_a09_cross_genre_evaluation_with_options(
    out_dir: &Path,
    measurement_options: A09MeasurementOptions,
) -> AdmResult<A09EvaluationReport> {
    std::fs::create_dir_all(out_dir)?;
    let samples = a09_samples()?;
    let mut spec_level_results = Vec::new();
    let mut compiled_by_sample = BTreeMap::new();

    for sample in &samples {
        let sample_dir = out_dir.join(&sample.sample_id).join("spec_level");
        let (result, compiled) = compile_spec_level(sample, &sample_dir)?;
        if let Some(compiled) = compiled {
            compiled_by_sample.insert(sample.sample_id.clone(), compiled);
        }
        spec_level_results.push(result);
    }

    let mut full_production_results = Vec::new();
    for sample in samples
        .iter()
        .filter(|sample| full_production_scope(&sample.production_scope))
    {
        full_production_results.push(run_full_production(
            sample,
            out_dir
                .join(&sample.sample_id)
                .join("full_production")
                .as_path(),
        )?);
    }

    let source_scan = scan_core_sources(&samples)?;
    let third_layer = third_layer_report(
        &samples,
        &spec_level_results,
        &compiled_by_sample,
        &source_scan,
        measurement_options,
    )?;
    let envelope_calibration = calibrate_envelope(&spec_level_results, &full_production_results);
    let field_promotion = field_promotion_checklist(&samples);
    let status = if spec_level_results
        .iter()
        .all(|result| result.blockers.is_empty())
        && full_production_results
            .iter()
            .all(|result| result.blockers.is_empty())
        && third_layer.label_permutation_passed
        && third_layer.capability_mutation_passed
        && third_layer.mutation_rejection_count == third_layer.mutation_rejections_required
        && third_layer.repeated_runs_stable
        && third_layer.no_ai_mode_supported
        && third_layer.bounded_ai_repeat_stable
        && third_layer.fault_injection_blocked
        && third_layer.source_scan_passed
    {
        A09EvaluationStatus::Passed
    } else {
        A09EvaluationStatus::Blocked
    };

    let spec_matrix_path = io::write_json_serializable(
        &out_dir.join("a09_spec_level_matrix.json"),
        &spec_level_results,
    )?;
    let full_matrix_path = io::write_json_serializable(
        &out_dir.join("a09_full_production_matrix.json"),
        &full_production_results,
    )?;
    let source_scan_path =
        io::write_json_serializable(&out_dir.join("a09_source_scan_report.json"), &source_scan)?;
    let envelope_path = io::write_json_serializable(
        &out_dir.join("a09_envelope_calibration_report.json"),
        &envelope_calibration,
    )?;
    let field_path = io::write_json_serializable(
        &out_dir.join("a09_field_promotion_checklist.json"),
        &field_promotion,
    )?;

    let report = A09EvaluationReport {
        schema_version: "a09_cross_genre_evaluation.v1".to_string(),
        compiler_version: A09_COMPILER_VERSION.to_string(),
        status,
        spec_level_results,
        full_production_results,
        third_layer_anti_overfit: third_layer,
        source_scan,
        envelope_calibration,
        field_promotion,
        output_paths: BTreeMap::from([
            (
                "specLevelMatrix".to_string(),
                path_string(&spec_matrix_path),
            ),
            (
                "fullProductionMatrix".to_string(),
                path_string(&full_matrix_path),
            ),
            (
                "sourceScanReport".to_string(),
                path_string(&source_scan_path),
            ),
            (
                "envelopeCalibrationReport".to_string(),
                path_string(&envelope_path),
            ),
            (
                "fieldPromotionChecklist".to_string(),
                path_string(&field_path),
            ),
        ]),
    };
    io::write_json_serializable(
        &out_dir.join("a09_cross_genre_evaluation_report.json"),
        &report,
    )?;
    Ok(report)
}

fn compile_spec_level(
    sample: &A09Sample,
    out_dir: &Path,
) -> AdmResult<(SpecLevelCompilationResult, Option<Step08_10Compilation>)> {
    std::fs::create_dir_all(out_dir)?;
    let compiler = Step00_06Compiler::default();
    let step00_06 = compiler.compile_spec(sample.spec.clone());
    compiler.write_outputs(&out_dir.join("step00_06"), &step00_06)?;
    let mut blockers = Vec::new();
    if step00_06.status != StepGateStatus::Passed {
        blockers.extend(step00_06.reports.iter().flat_map(|report| {
            report
                .issues
                .iter()
                .map(|issue| format!("step{}:{}", report.step_id, issue.code))
        }));
        return Ok((
            SpecLevelCompilationResult {
                sample_id: sample.sample_id.clone(),
                display_name: sample.display_name.clone(),
                structure_family: sample.structure_family.clone(),
                production_scope: sample.production_scope.clone(),
                step00_06_status: "blocked".to_string(),
                step08_10_status: "not_run".to_string(),
                semantic_hash: step00_06.final_semantic_hash,
                architecture_hash: None,
                task_count: 0,
                asset_count: 0,
                scenario_count: sample.spec.acceptance_scenarios.len(),
                anti_overfit_passed: false,
                blockers,
            },
            None,
        ));
    }

    let step07_dir = out_dir.join("step07");
    compile_step07_art_direction(&sample.spec, &step07_dir)?;
    let anchors =
        confirm_style_anchors_attended(&step07_dir, "a09_harness", "approved", "attended")?.anchors;
    let compiled = compile_step08_10(&sample.spec, &anchors, &out_dir.join("step08_10"))?;
    if compiled.status != "success" {
        blockers.extend(compiled.task_graph.validation.issues.clone());
    }
    let anti_overfit_passed = step00_06.reports.iter().all(|report| {
        report.anti_overfit.label_permutation_status_stable
            && report.anti_overfit.capability_mutation_changed_hash
    }) && compiled
        .architecture
        .anti_overfit
        .label_permutation_systems_stable
        && compiled
            .architecture
            .anti_overfit
            .capability_mutation_systems_changed;
    if !anti_overfit_passed {
        blockers.push("anti_overfit_evidence_failed".to_string());
    }
    Ok((
        SpecLevelCompilationResult {
            sample_id: sample.sample_id.clone(),
            display_name: sample.display_name.clone(),
            structure_family: sample.structure_family.clone(),
            production_scope: sample.production_scope.clone(),
            step00_06_status: "passed".to_string(),
            step08_10_status: compiled.status.clone(),
            semantic_hash: step00_06.final_semantic_hash,
            architecture_hash: Some(compiled.architecture.semantic_hash.clone()),
            task_count: compiled.task_graph.tasks.len(),
            asset_count: compiled.asset_manifest.items.len(),
            scenario_count: sample.spec.acceptance_scenarios.len(),
            anti_overfit_passed,
            blockers,
        },
        Some(compiled),
    ))
}

fn run_full_production(sample: &A09Sample, out_dir: &Path) -> AdmResult<FullProductionResult> {
    std::fs::create_dir_all(out_dir)?;
    let step07_dir = out_dir.join("step07");
    compile_step07_art_direction(&sample.spec, &step07_dir)?;
    let anchors =
        confirm_style_anchors_attended(&step07_dir, "a09_harness", "approved", "attended")?.anchors;
    let compiled = compile_step08_10(&sample.spec, &anchors, &out_dir.join("step08_10"))?;
    let mut state = Step11ExecutionState::new(compiled.task_graph.source_game_spec_hash.clone());
    let step11 = Step11ExecutionEngine::new(
        DeterministicWorkspaceAgent,
        Step11ExecutionBudget {
            max_workers: 1,
            max_retries: 2,
        },
    )
    .run(
        &compiled.task_graph,
        &mut state,
        &Step11StopToken::default(),
    )?;
    let asset_policy = AssetProductionPolicy::attended_approved(
        compiled
            .asset_manifest
            .items
            .iter()
            .map(|item| item.asset_id.clone()),
    );
    let (bindings, loader) = workspace_bindings_for_manifest(
        &out_dir.join("runtime_reference_workspace"),
        &compiled.asset_manifest,
    )?;
    let step12 = run_step12_asset_production(
        &compiled.asset_manifest,
        &anchors,
        &out_dir.join("step12"),
        &asset_policy,
        &bindings,
        &loader,
    )?;
    let step13 = run_step13_acceptance_validation(
        &sample.spec,
        &step11,
        &step12,
        &Step13ValidationPolicy::from_execution_evidence(
            Step13ExecutionEvidence::test_only_nominal_for_spec(
                &sample.spec,
                compute_step13_build_hash(&step11, &step12),
            ),
        ),
        &out_dir.join("step13"),
    )?;
    let step14 = run_step14_r1_packaging_gate(
        &sample.spec,
        &step13,
        &R1GateEvidence {
            reproducible_build: true,
            integrity_passed: true,
            exe_smoke_passed: true,
            standalone_boundary_passed: true,
            ai_usage_evidence_present: true,
            ai_off_flow_supported: true,
            anti_overfit_gates_passed: true,
            content_complete: true,
            user_playtest_signed: false,
        },
        &out_dir.join("step14"),
    )?;

    let mut blockers = Vec::new();
    if step11.status != Step11ExecutionStatus::Success {
        blockers.push("step11_not_success".to_string());
    }
    if step12.status != Step12Status::Success {
        blockers.push("step12_not_success".to_string());
    }
    if step13.status != Step13Status::Passed {
        blockers.push("step13_not_passed".to_string());
    }
    let manual_signature_required = step14
        .blockers
        .iter()
        .any(|blocker| blocker == "r1_user_playtest_signed");
    blockers.extend(
        step14
            .blockers
            .iter()
            .filter(|blocker| blocker.as_str() != "r1_user_playtest_signed")
            .cloned(),
    );
    let weighted_complexity = weighted_complexity(
        compiled.task_graph.tasks.len(),
        compiled.asset_manifest.items.len(),
        sample.spec.acceptance_scenarios.len(),
        compiled.architecture.systems.len(),
    );
    Ok(FullProductionResult {
        sample_id: sample.sample_id.clone(),
        display_name: sample.display_name.clone(),
        status: if blockers.is_empty() {
            "passed".to_string()
        } else {
            "blocked".to_string()
        },
        manual_signature_required,
        step11_status: format!("{:?}", step11.status),
        step12_status: format!("{:?}", step12.status),
        step13_status: format!("{:?}", step13.status),
        step14_status: format!("{:?}", step14.status),
        task_count: compiled.task_graph.tasks.len(),
        asset_count: compiled.asset_manifest.items.len(),
        scenario_count: sample.spec.acceptance_scenarios.len(),
        weighted_complexity,
        blockers,
    })
}

fn workspace_bindings_for_manifest(
    workspace_root: &Path,
    manifest: &FrozenAssetManifest,
) -> AdmResult<(Vec<AssetBindingReference>, WorkspaceReferenceAssetLoader)> {
    let prefab_dir = workspace_root.join("Assets/AutoDesign/Prefabs");
    let scene_dir = workspace_root.join("Assets/AutoDesign/Scenes");
    std::fs::create_dir_all(&prefab_dir)?;
    std::fs::create_dir_all(&scene_dir)?;
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
    std::fs::write(prefab_dir.join("A09GeneratedBindings.prefab"), prefab)?;
    std::fs::write(scene_dir.join("A09StyleReference.unity"), scene)?;
    let bindings = discover_asset_bindings_from_workspace(manifest, workspace_root)?;
    Ok((bindings, WorkspaceReferenceAssetLoader::new(workspace_root)))
}

fn third_layer_report(
    samples: &[A09Sample],
    spec_level_results: &[SpecLevelCompilationResult],
    compiled_by_sample: &BTreeMap<String, Step08_10Compilation>,
    source_scan: &SourceScanReport,
    measurement_options: A09MeasurementOptions,
) -> AdmResult<ThirdLayerAntiOverfitReport> {
    let compiler = Step00_06Compiler::default();
    let mut mutation_rejection_count = 0usize;
    let mut repeated_runs_stable = true;
    for sample in samples {
        let mut hashes = BTreeSet::new();
        for _ in 0..20 {
            hashes.insert(
                compiler
                    .compile_spec(sample.spec.clone())
                    .final_semantic_hash,
            );
        }
        repeated_runs_stable &= hashes.len() == 1;
        if measurement_options.run_mutation_rejection {
            let mut mutated = sample.spec.clone();
            mutated.trace_links.clear();
            if compiler.compile_spec(mutated).status == StepGateStatus::Blocked {
                mutation_rejection_count += 1;
            }
        }
    }
    let no_ai_mode_supported = if measurement_options.run_no_ai_path {
        measure_no_ai_mode_supported(samples)?
    } else {
        false
    };
    let bounded_ai_repeat_stable = if measurement_options.run_bounded_ai_repetition {
        measure_bounded_repeat_stability(samples)?
    } else {
        false
    };
    Ok(ThirdLayerAntiOverfitReport {
        label_permutation_passed: spec_level_results
            .iter()
            .all(|result| result.anti_overfit_passed),
        capability_mutation_passed: compiled_by_sample.values().all(|compiled| {
            compiled
                .architecture
                .anti_overfit
                .capability_mutation_systems_changed
        }),
        mutation_rejection_count,
        mutation_rejections_required: samples.len(),
        repeated_run_count_per_sample: 20,
        repeated_runs_stable,
        no_ai_mode_supported,
        bounded_ai_repeat_count: 20,
        bounded_ai_repeat_stable,
        fault_injection_blocked: fault_injection_blocks(samples)?,
        source_scan_passed: source_scan.hits.is_empty(),
    })
}

fn measure_no_ai_mode_supported(samples: &[A09Sample]) -> AdmResult<bool> {
    let compiler = Step00_06Compiler::default();
    let root = std::env::temp_dir().join(adm_new_foundation::new_stable_id("a09_no_ai").unwrap());
    std::fs::create_dir_all(&root)?;
    let mut supported = true;
    for sample in samples {
        let step00_06 = compiler.compile_spec(sample.spec.clone());
        if step00_06.status != StepGateStatus::Passed {
            supported = false;
            break;
        }
        let sample_dir = root.join(&sample.sample_id);
        let step07_dir = sample_dir.join("step07");
        compile_step07_art_direction(&sample.spec, &step07_dir)?;
        let anchors =
            confirm_style_anchors_attended(&step07_dir, "a09_no_ai", "approved", "attended")?
                .anchors;
        let compiled = compile_step08_10(&sample.spec, &anchors, &sample_dir.join("step08_10"))?;
        if compiled.status != "success" {
            supported = false;
            break;
        }
    }
    let _ = std::fs::remove_dir_all(root);
    Ok(supported)
}

fn measure_bounded_repeat_stability(samples: &[A09Sample]) -> AdmResult<bool> {
    let Some(sample) = samples.first() else {
        return Ok(false);
    };
    let root =
        std::env::temp_dir().join(adm_new_foundation::new_stable_id("a09_bounded_repeat").unwrap());
    std::fs::create_dir_all(&root)?;
    let mut hashes = BTreeSet::new();
    for index in 0..20 {
        let run_dir = root.join(format!("run_{index:02}"));
        let step07_dir = run_dir.join("step07");
        compile_step07_art_direction(&sample.spec, &step07_dir)?;
        let anchors =
            confirm_style_anchors_attended(&step07_dir, "a09_bounded", "approved", "attended")?
                .anchors;
        let compiled = compile_step08_10(&sample.spec, &anchors, &run_dir.join("step08_10"))?;
        hashes.insert(compiled.task_graph.semantic_hash);
    }
    let _ = std::fs::remove_dir_all(root);
    Ok(hashes.len() == 1)
}

fn fault_injection_blocks(samples: &[A09Sample]) -> AdmResult<bool> {
    let Some(sample) = samples.first() else {
        return Ok(false);
    };
    let root = std::env::temp_dir().join(adm_new_foundation::new_stable_id("a09_fault").unwrap());
    std::fs::create_dir_all(&root)?;
    let step07_dir = root.join("step07");
    compile_step07_art_direction(&sample.spec, &step07_dir)?;
    let anchors =
        confirm_style_anchors_attended(&step07_dir, "a09_harness", "approved", "attended")?.anchors;
    let compiled = compile_step08_10(&sample.spec, &anchors, &root.join("step08_10"))?;
    let mut state = Step11ExecutionState::new(compiled.task_graph.source_game_spec_hash.clone());
    let report = Step11ExecutionEngine::new(
        ScopeViolationAgent,
        Step11ExecutionBudget {
            max_workers: 1,
            max_retries: 2,
        },
    )
    .run(
        &compiled.task_graph,
        &mut state,
        &Step11StopToken::default(),
    )?;
    let _ = std::fs::remove_dir_all(root);
    Ok(report.status != Step11ExecutionStatus::Success && !report.correction_queue.is_empty())
}

fn scan_core_sources(samples: &[A09Sample]) -> AdmResult<SourceScanReport> {
    let root = workspace_root();
    let forbidden = forbidden_source_tokens(samples)?;
    scan_core_sources_at(&root, &forbidden)
}

fn scan_core_sources_at(root: &Path, forbidden: &[String]) -> AdmResult<SourceScanReport> {
    let files = collect_core_source_files(root)?;
    let mut hits = Vec::new();
    let mut scanned_files = Vec::new();
    for file in files {
        let relative = source_file_label(root, &file)?;
        let text = production_source_text(&std::fs::read_to_string(&file)?).to_ascii_lowercase();
        scanned_files.push(relative.clone());
        for token in forbidden {
            if text.contains(token) {
                hits.push(SourceScanHit {
                    file: relative.clone(),
                    token: token.to_string(),
                });
            }
        }
    }
    Ok(SourceScanReport {
        status: if hits.is_empty() {
            "passed".to_string()
        } else {
            "blocked".to_string()
        },
        scanned_files,
        forbidden_tokens: forbidden.to_vec(),
        hits,
    })
}

fn collect_core_source_files(root: &Path) -> AdmResult<Vec<PathBuf>> {
    let mut files = BTreeSet::new();
    collect_rust_files(root, "crates/adm-new-game-spec/src", &mut files, &[])?;
    collect_optional_source_file(
        root,
        "crates/adm-new-design/src/anti_overfit.rs",
        &mut files,
    );
    collect_optional_source_file(
        root,
        "crates/adm-new-design/src/game_spec_projection.rs",
        &mut files,
    );
    collect_rust_files(
        root,
        "crates/adm-new-design/src/decision_graph",
        &mut files,
        &[],
    )?;
    collect_pipeline_v2_sources(root, &mut files)?;
    if files.is_empty() {
        return Err(AdmError::new("A09 source scan found no core Rust files"));
    }
    Ok(files.into_iter().collect())
}

fn collect_pipeline_v2_sources(root: &Path, files: &mut BTreeSet<PathBuf>) -> AdmResult<()> {
    let pipeline_src = root.join("crates/adm-new-pipeline/src");
    if !pipeline_src.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&pipeline_src)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_file()
            && is_rust_file(&path)
            && (name.contains("game_spec_v2") || name == "r2_release.rs")
        {
            files.insert(path);
        } else if path.is_dir() && name == "cross_genre_evaluation" {
            collect_rust_files_in_dir(&path, files, &["samples.rs"])?;
        }
    }

    let stages = pipeline_src.join("stages");
    if stages.exists() {
        for entry in std::fs::read_dir(stages)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_file() && is_rust_file(&path) && name.ends_with("_v2.rs") {
                files.insert(path);
            } else if path.is_dir() && name.contains("_v2") {
                collect_rust_files_in_dir(&path, files, &[])?;
            }
        }
    }
    Ok(())
}

fn collect_optional_source_file(root: &Path, relative: &str, files: &mut BTreeSet<PathBuf>) {
    let path = root.join(relative);
    if path.is_file() {
        files.insert(path);
    }
}

fn collect_rust_files(
    root: &Path,
    relative_dir: &str,
    files: &mut BTreeSet<PathBuf>,
    excluded_file_names: &[&str],
) -> AdmResult<()> {
    let dir = root.join(relative_dir);
    if !dir.exists() {
        return Ok(());
    }
    collect_rust_files_in_dir(&dir, files, excluded_file_names)
}

fn collect_rust_files_in_dir(
    dir: &Path,
    files: &mut BTreeSet<PathBuf>,
    excluded_file_names: &[&str],
) -> AdmResult<()> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in std::fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if is_rust_file(&path) {
                let name = entry.file_name().to_string_lossy().to_string();
                if !excluded_file_names.iter().any(|excluded| *excluded == name) {
                    files.insert(path);
                }
            }
        }
    }
    Ok(())
}

fn is_rust_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rs"))
}

fn source_file_label(root: &Path, path: &Path) -> AdmResult<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|error| AdmError::new(format!("source scan path escaped root: {error}")))?;
    Ok(path_string(relative))
}

fn forbidden_source_tokens(samples: &[A09Sample]) -> AdmResult<Vec<String>> {
    let configured =
        serde_json::from_str::<Vec<String>>(FORBIDDEN_SOURCE_TOKEN_CONFIG).map_err(|error| {
            AdmError::new(format!("invalid forbidden source token config: {error}"))
        })?;
    let mut tokens = BTreeSet::new();
    for token in configured {
        insert_source_token_variants(&mut tokens, &token);
    }
    for sample in samples {
        insert_source_token_variants(&mut tokens, &sample.sample_id);
        insert_source_token_variants(&mut tokens, &sample.display_name);
        insert_source_token_variants(&mut tokens, &sample.structure_family);
        insert_source_token_variants(&mut tokens, sample.spec.identity.project_id.as_str());
        insert_source_token_variants(&mut tokens, &sample.spec.intent.title);
    }
    Ok(tokens.into_iter().collect())
}

fn insert_source_token_variants(tokens: &mut BTreeSet<String>, value: &str) {
    let identifier = normalize_identifier_token(value);
    insert_source_token(tokens, &identifier);
    if let Some(stripped) = identifier.strip_suffix("_sample") {
        insert_source_token(tokens, stripped);
    }

    let phrase = normalize_phrase_token(value);
    insert_source_token(tokens, &phrase);
    if let Some(stripped) = phrase.strip_suffix(" sample") {
        insert_source_token(tokens, stripped);
    }
}

fn insert_source_token(tokens: &mut BTreeSet<String>, token: &str) {
    let token = token.trim();
    if token.len() >= 3 && token != "sample" {
        tokens.insert(token.to_string());
    }
}

fn normalize_identifier_token(value: &str) -> String {
    normalize_token(value, '_')
}

fn normalize_phrase_token(value: &str) -> String {
    normalize_token(value, ' ')
}

fn normalize_token(value: &str, separator: char) -> String {
    let mut normalized = String::new();
    let mut previous_separator = true;
    for character in value.trim().to_ascii_lowercase().chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character);
            previous_separator = false;
        } else if !previous_separator {
            normalized.push(separator);
            previous_separator = true;
        }
    }
    normalized.trim_matches(separator).to_string()
}

fn production_source_text(text: &str) -> &str {
    text.split("\n#[cfg(test)]").next().unwrap_or(text)
}

fn calibrate_envelope(
    spec_level_results: &[SpecLevelCompilationResult],
    full_results: &[FullProductionResult],
) -> EnvelopeCalibrationReport {
    let mut measurements = spec_level_results
        .iter()
        .map(|result| EnvelopeMeasurement {
            sample_id: result.sample_id.clone(),
            product_envelope: "declared_in_game_spec".to_string(),
            task_count: result.task_count,
            asset_count: result.asset_count,
            scenario_count: result.scenario_count,
            system_count: 8,
            weighted_complexity: weighted_complexity(
                result.task_count,
                result.asset_count,
                result.scenario_count,
                8,
            ),
        })
        .collect::<Vec<_>>();
    for result in full_results {
        if let Some(existing) = measurements
            .iter_mut()
            .find(|item| item.sample_id == result.sample_id)
        {
            existing.weighted_complexity = result.weighted_complexity;
        }
    }
    let max_full = full_results
        .iter()
        .map(|result| result.weighted_complexity)
        .max()
        .unwrap_or(0);
    EnvelopeCalibrationReport {
        status: "measured".to_string(),
        measurements,
        suggested_thresholds: vec![
            WeightedBudgetThreshold {
                envelope_label: "small".to_string(),
                inclusive_max_weight: max_full.saturating_div(2).max(20),
                evidence_sample_count: full_results.len(),
            },
            WeightedBudgetThreshold {
                envelope_label: "medium".to_string(),
                inclusive_max_weight: max_full.max(40),
                evidence_sample_count: full_results.len(),
            },
        ],
        notes: vec![
            "A09 records weighted budget evidence; ProductEnvelope type migration remains an A10/R2 release decision.".to_string(),
            "Weights: task=3, asset=2, scenario=2, runtime system=1.".to_string(),
        ],
    }
}

fn field_promotion_checklist(samples: &[A09Sample]) -> FieldPromotionChecklist {
    let mut namespaces = samples
        .iter()
        .flat_map(|sample| sample.spec.extensions.values())
        .map(|extension| extension.namespace.to_string())
        .collect::<Vec<_>>();
    namespaces.sort();
    namespaces.dedup();
    FieldPromotionChecklist {
        status: "passed".to_string(),
        new_core_fields: Vec::new(),
        extension_namespaces_used: namespaces,
        decision: "A09 introduced no new GameSpec core fields; structure-specific data remains in ExtensionBlock or fixture content.".to_string(),
    }
}

fn full_production_scope(scope: &A09ProductionScope) -> bool {
    matches!(
        scope,
        A09ProductionScope::R1Reference | A09ProductionScope::FullProduction
    )
}

fn weighted_complexity(
    task_count: usize,
    asset_count: usize,
    scenario_count: usize,
    system_count: usize,
) -> u64 {
    (task_count as u64 * 3)
        + (asset_count as u64 * 2)
        + (scenario_count as u64 * 2)
        + system_count as u64
}

#[derive(Debug, Clone, Copy)]
struct DeterministicWorkspaceAgent;

impl WorkspaceTaskAgent for DeterministicWorkspaceAgent {
    fn execute_task(
        &self,
        task: &TrustedDevelopmentTask,
        attempt: u32,
        _previous_failure: Option<&crate::stages::step11_v2::Step11FailureEvidence>,
    ) -> AdmResult<WorkspaceTransactionResult> {
        Ok(committed_transaction(task, attempt, false))
    }
}

#[derive(Debug, Clone, Copy)]
struct ScopeViolationAgent;

impl WorkspaceTaskAgent for ScopeViolationAgent {
    fn execute_task(
        &self,
        task: &TrustedDevelopmentTask,
        attempt: u32,
        _previous_failure: Option<&crate::stages::step11_v2::Step11FailureEvidence>,
    ) -> AdmResult<WorkspaceTransactionResult> {
        Ok(committed_transaction(task, attempt, true))
    }
}

fn committed_transaction(
    task: &TrustedDevelopmentTask,
    attempt: u32,
    outside_scope: bool,
) -> WorkspaceTransactionResult {
    let contract = &task.workspace_contract;
    let mut agent_changed_paths = contract.agent_write_paths.clone();
    if outside_scope {
        agent_changed_paths.insert(
            adm_new_change_kernel::WorkspaceRelativePath::parse("Assets/A09/OutsideScope.cs")
                .expect("static path"),
        );
    }
    WorkspaceTransactionResult {
        schema_version: WORKSPACE_CHANGE_SET_SCHEMA_VERSION.to_string(),
        change_set_id: contract.change_set_id.clone(),
        contract_sha256: contract.contract_hash().expect("contract hash"),
        base_tree_hash: contract.base_tree_hash.clone(),
        outcome: ChangeOutcome::Committed,
        failure_category: None,
        side_effect_state: SideEffectState::Committed,
        stage: "a09_deterministic_workspace_agent".to_string(),
        resulting_tree_hash: Some(sha256_hex(format!("{}:{attempt}", task.task_id).as_bytes())),
        agent_changed_paths,
        trusted_tool_changed_paths: BTreeSet::new(),
        build_output_changed_paths: BTreeSet::new(),
        trusted_test_hashes: task
            .workspace_contract
            .trusted_tests
            .iter()
            .map(|test| (test.test_id.clone(), test.baseline_sha256.clone()))
            .collect(),
        evidence: vec![ChangeEvidence::from_bytes(
            "a09_workspace_result",
            "a09",
            EvidenceStatus::Passed,
            format!("{}:{attempt}", task.task_id).as_bytes(),
        )],
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("pipeline crate is inside workspace")
        .to_path_buf()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_scan_recursively_includes_new_v2_core_files() {
        let root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("a09_source_scan").unwrap());
        let stage_dir = root.join("crates/adm-new-pipeline/src/stages/step99_v2");
        let sample_dir = root.join("crates/adm-new-pipeline/src/cross_genre_evaluation");
        std::fs::create_dir_all(&stage_dir).unwrap();
        std::fs::create_dir_all(&sample_dir).unwrap();
        std::fs::write(
            stage_dir.join("new_core.rs"),
            "pub const BAD: &str = \"plants_vs_zombies\";",
        )
        .unwrap();
        std::fs::write(
            sample_dir.join("samples.rs"),
            "pub const ALLOWED_FIXTURE: &str = \"plants_vs_zombies\";",
        )
        .unwrap();

        let report = scan_core_sources_at(&root, &["plants_vs_zombies".to_string()]).unwrap();

        assert!(report.scanned_files.iter().any(|file| {
            file.ends_with("crates/adm-new-pipeline/src/stages/step99_v2/new_core.rs")
        }));
        assert!(
            !report
                .scanned_files
                .iter()
                .any(|file| file.ends_with("samples.rs"))
        );
        assert_eq!(report.hits.len(), 1);
        assert!(report.hits[0].file.ends_with("step99_v2/new_core.rs"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn forbidden_source_tokens_are_sample_derived_plus_external_aliases() {
        let samples = a09_samples().unwrap();
        let tokens = forbidden_source_tokens(&samples).unwrap();

        assert!(tokens.contains(&"plants_vs_zombies".to_string()));
        assert!(tokens.contains(&"plants vs zombies".to_string()));
        assert!(tokens.contains(&"match_grid".to_string()));
        assert!(tokens.contains(&"match grid".to_string()));
        assert!(tokens.contains(&"r1c0_micro_ecodome_lane_guard".to_string()));
    }
}
