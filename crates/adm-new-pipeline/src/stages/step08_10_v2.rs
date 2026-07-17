use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

use adm_new_change_kernel::{
    ChangeEvidence, CommandPermission, CommandPurpose, EvidenceStatus, TrustedTestContract,
    WORKSPACE_CHANGE_SET_SCHEMA_VERSION, WorkspaceChangeSet, WorkspaceFileExpectation,
    WorkspaceFilePayload, WorkspaceOperation, WorkspaceRelativePath, WorkspaceResourceBudget,
};
use adm_new_design::anti_overfit::{
    apply_capability_mutation, capability_mutation_suite, permute_display_labels,
};
use adm_new_foundation::{AdmError, AdmResult, io, sha256_hex};
use adm_new_game_spec::{
    ContentGeneration, GameSpec, SpaceTopology, canonicalize_game_spec, parse_game_spec,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::stages::step07_v2::StyleAnchorCandidate;

pub const STEP08_10_V2_COMPILER_VERSION: &str = "game_spec_step08_10_architecture_tasks.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSystemBoundary {
    pub system_id: String,
    pub responsibility: String,
    pub capability_refs: Vec<String>,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeArchitecture {
    pub schema_version: String,
    pub compiler_version: String,
    pub source_game_spec_hash: String,
    pub semantic_hash: String,
    pub systems: Vec<RuntimeSystemBoundary>,
    pub anti_overfit: AntiOverfitEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetManifestItem {
    pub asset_id: String,
    pub purpose: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub slice: String,
    pub budget_tier: String,
    pub dependencies: Vec<String>,
    pub acceptance: Vec<String>,
    pub source_refs: Vec<String>,
    pub style_anchor_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenAssetManifest {
    pub schema_version: String,
    pub compiler_version: String,
    pub source_game_spec_hash: String,
    pub frozen_hash: String,
    pub items: Vec<AssetManifestItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MachineAcceptanceCheck {
    pub check_id: String,
    pub compile_target: String,
    pub trusted_test_id: String,
    pub command_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedDevelopmentTask {
    pub task_id: String,
    pub title: String,
    pub ordinal_size: String,
    pub architecture_system_id: String,
    pub declared_write_paths: Vec<String>,
    pub machine_checks: Vec<MachineAcceptanceCheck>,
    pub dependencies: Vec<String>,
    pub rollback_boundary: Vec<String>,
    pub workspace_contract: WorkspaceChangeSet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedTaskGraph {
    pub schema_version: String,
    pub compiler_version: String,
    pub source_game_spec_hash: String,
    pub semantic_hash: String,
    pub tasks: Vec<TrustedDevelopmentTask>,
    pub validation: TaskGraphValidation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TaskGraphValidation {
    pub acyclic: bool,
    pub contract_count: usize,
    pub invalid_contract_count: usize,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AntiOverfitEvidence {
    pub label_permutation_systems_stable: bool,
    pub capability_mutation_systems_changed: bool,
    pub mutation_kind: String,
    #[serde(default)]
    pub capability_mutation_axis_count: usize,
    #[serde(default)]
    pub capability_mutation_changed_axes: Vec<String>,
    #[serde(default)]
    pub capability_mutation_unchanged_axes: Vec<String>,
    #[serde(default)]
    pub capability_mutation_failed_axes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step08_10Compilation {
    pub status: String,
    pub architecture: RuntimeArchitecture,
    pub asset_manifest: FrozenAssetManifest,
    pub task_graph: TrustedTaskGraph,
    pub output_paths: BTreeMap<String, String>,
}

pub fn compile_step08_10_from_json(
    game_spec_json: &str,
    style_anchors: &[StyleAnchorCandidate],
    out_dir: &Path,
) -> AdmResult<Step08_10Compilation> {
    let spec = parse_game_spec(game_spec_json).map_err(|error| AdmError::new(error.to_string()))?;
    compile_step08_10(&spec, style_anchors, out_dir)
}

pub fn compile_step08_10(
    spec: &GameSpec,
    style_anchors: &[StyleAnchorCandidate],
    out_dir: &Path,
) -> AdmResult<Step08_10Compilation> {
    std::fs::create_dir_all(out_dir)?;
    let source_hash = canonicalize_game_spec(spec)
        .map_err(|error| AdmError::new(format!("GameSpec hash failed: {error}")))?
        .content_hash;
    let architecture = compile_runtime_architecture(spec, &source_hash)?;
    let asset_manifest = compile_asset_manifest(spec, style_anchors, &source_hash)?;
    let task_graph = compile_task_graph(spec, &architecture, &asset_manifest, &source_hash)?;
    let status =
        if task_graph.validation.acyclic && task_graph.validation.invalid_contract_count == 0 {
            "success"
        } else {
            "blocked"
        }
        .to_string();

    let architecture_path =
        io::write_json_serializable(&out_dir.join("runtime_architecture.json"), &architecture)?;
    let manifest_path =
        io::write_json_serializable(&out_dir.join("frozen_asset_manifest.json"), &asset_manifest)?;
    let task_graph_path =
        io::write_json_serializable(&out_dir.join("trusted_task_graph.json"), &task_graph)?;
    let output = Step08_10Compilation {
        status,
        architecture,
        asset_manifest,
        task_graph,
        output_paths: BTreeMap::from([
            (
                "runtimeArchitecture".to_string(),
                path_string(&architecture_path),
            ),
            (
                "frozenAssetManifest".to_string(),
                path_string(&manifest_path),
            ),
            (
                "trustedTaskGraph".to_string(),
                path_string(&task_graph_path),
            ),
        ]),
    };
    io::write_json_serializable(&out_dir.join("step08_10_compilation.json"), &output)?;
    Ok(output)
}

fn compile_runtime_architecture(
    spec: &GameSpec,
    source_hash: &str,
) -> AdmResult<RuntimeArchitecture> {
    let systems = runtime_systems(spec);
    let semantic_hash = hash_json(&json!({
        "source": source_hash,
        "systems": systems,
        "compiler": STEP08_10_V2_COMPILER_VERSION,
    }))?;
    Ok(RuntimeArchitecture {
        schema_version: "step08_runtime_architecture.v1".to_string(),
        compiler_version: STEP08_10_V2_COMPILER_VERSION.to_string(),
        source_game_spec_hash: source_hash.to_string(),
        semantic_hash,
        systems,
        anti_overfit: anti_overfit_evidence(spec),
    })
}

fn compile_asset_manifest(
    spec: &GameSpec,
    style_anchors: &[StyleAnchorCandidate],
    source_hash: &str,
) -> AdmResult<FrozenAssetManifest> {
    let anchor_refs = style_anchors
        .iter()
        .map(|anchor| anchor.asset_id.clone())
        .collect::<Vec<_>>();
    let mut items = Vec::new();
    for (entity_id, entity) in &spec.entities {
        let lower = entity
            .tags
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        if lower.contains("guardian") || lower.contains("enemy") || lower.contains("objective") {
            items.push(AssetManifestItem {
                asset_id: format!("sprite_{entity_id}"),
                purpose: format!("Runtime visual for entity {entity_id}."),
                format: "png_rgba".to_string(),
                width: 256,
                height: 256,
                slice: "single_sprite".to_string(),
                budget_tier: "M".to_string(),
                dependencies: Vec::new(),
                acceptance: vec![
                    "decode_png".to_string(),
                    "alpha_present".to_string(),
                    "referenced_by_prefab_or_scene".to_string(),
                ],
                source_refs: vec![format!("entity:{entity_id}")],
                style_anchor_refs: anchor_refs.clone(),
            });
        }
    }
    items.push(AssetManifestItem {
        asset_id: "hud_status_panel".to_string(),
        purpose: "HUD resource, wave, pause, and rejection feedback.".to_string(),
        format: "png_rgba".to_string(),
        width: 512,
        height: 192,
        slice: "nine_slice_ui".to_string(),
        budget_tier: "M".to_string(),
        dependencies: vec!["sprite_protected_core".to_string()],
        acceptance: vec![
            "decode_png".to_string(),
            "contrast_pass".to_string(),
            "referenced_by_ui_scene".to_string(),
        ],
        source_refs: vec!["presentation:feedback_language".to_string()],
        style_anchor_refs: anchor_refs.clone(),
    });
    items.push(AssetManifestItem {
        asset_id: "style_reference_keyframe".to_string(),
        purpose: "Reference keyframe for consistency checks.".to_string(),
        format: "png_rgba".to_string(),
        width: 640,
        height: 384,
        slice: "full_frame".to_string(),
        budget_tier: "M".to_string(),
        dependencies: anchor_refs.clone(),
        acceptance: vec!["style_anchor_similarity_sample".to_string()],
        source_refs: spec
            .presentation
            .keys()
            .map(|id| format!("presentation:{id}"))
            .collect(),
        style_anchor_refs: anchor_refs,
    });
    items.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    let frozen_hash = hash_json(&json!({
        "source": source_hash,
        "items": items,
        "compiler": STEP08_10_V2_COMPILER_VERSION,
    }))?;
    Ok(FrozenAssetManifest {
        schema_version: "step09_frozen_asset_manifest.v1".to_string(),
        compiler_version: STEP08_10_V2_COMPILER_VERSION.to_string(),
        source_game_spec_hash: source_hash.to_string(),
        frozen_hash,
        items,
    })
}

fn compile_task_graph(
    spec: &GameSpec,
    architecture: &RuntimeArchitecture,
    manifest: &FrozenAssetManifest,
    source_hash: &str,
) -> AdmResult<TrustedTaskGraph> {
    let mut tasks = vec![
        task(
            "runtime.scaffold",
            "Create runtime bootstrap",
            "S",
            "runtime_bootstrap",
            vec![],
            vec!["Assets/AutoDesign/Scripts/Runtime/GameBootstrap.cs"],
            source_hash,
        ),
        task(
            "runtime.lane_space",
            "Implement lane space runtime",
            "M",
            "space_runtime",
            vec!["runtime.scaffold"],
            vec!["Assets/AutoDesign/Scripts/Runtime/LaneSpaceRuntime.cs"],
            source_hash,
        ),
        task(
            "runtime.resources",
            "Implement resource economy runtime",
            "M",
            "resource_runtime",
            vec!["runtime.scaffold"],
            vec!["Assets/AutoDesign/Scripts/Runtime/ResourceRuntime.cs"],
            source_hash,
        ),
        task(
            "runtime.waves",
            "Implement authored wave runtime",
            "M",
            "wave_runtime",
            vec!["runtime.lane_space", "runtime.resources"],
            vec!["Assets/AutoDesign/Scripts/Runtime/WaveRuntime.cs"],
            source_hash,
        ),
        task(
            "runtime.ui",
            "Implement HUD and feedback runtime",
            "M",
            "ui_feedback_runtime",
            vec!["runtime.resources"],
            vec!["Assets/AutoDesign/Scripts/UI/HudFeedbackRuntime.cs"],
            source_hash,
        ),
        task(
            "runtime.save_pause",
            "Implement save and pause runtime",
            "S",
            "save_pause_runtime",
            vec!["runtime.scaffold"],
            vec!["Assets/AutoDesign/Scripts/Runtime/SavePauseRuntime.cs"],
            source_hash,
        ),
        task(
            "assets.manifest_import",
            "Import frozen asset manifest",
            "S",
            "asset_manifest_runtime",
            vec!["runtime.scaffold"],
            vec!["Assets/AutoDesign/Resources/frozen_asset_manifest.json"],
            source_hash,
        ),
        task(
            "acceptance.harness",
            "Implement executable acceptance harness",
            "M",
            "acceptance_runtime",
            vec![
                "runtime.waves",
                "runtime.ui",
                "runtime.save_pause",
                "assets.manifest_import",
            ],
            vec!["Assets/AutoDesign/Tests/R1AcceptanceHarness.cs"],
            source_hash,
        ),
    ];
    let allowed_systems = architecture
        .systems
        .iter()
        .map(|system| system.system_id.as_str())
        .collect::<BTreeSet<_>>();
    for task in &mut tasks {
        if !allowed_systems.contains(task.architecture_system_id.as_str()) {
            task.architecture_system_id = "runtime_bootstrap".to_string();
        }
    }
    let validation = validate_task_graph(&tasks);
    let semantic_hash = hash_json(&json!({
        "source": source_hash,
        "architectureHash": architecture.semantic_hash,
        "assetManifestHash": manifest.frozen_hash,
        "tasks": tasks,
        "compiler": STEP08_10_V2_COMPILER_VERSION,
        "capabilitySummary": spec.capabilities,
    }))?;
    Ok(TrustedTaskGraph {
        schema_version: "step10_trusted_task_graph.v1".to_string(),
        compiler_version: STEP08_10_V2_COMPILER_VERSION.to_string(),
        source_game_spec_hash: source_hash.to_string(),
        semantic_hash,
        tasks,
        validation,
    })
}

fn runtime_systems(spec: &GameSpec) -> Vec<RuntimeSystemBoundary> {
    let mut systems = vec![
        RuntimeSystemBoundary {
            system_id: "runtime_bootstrap".to_string(),
            responsibility: "Own scene boot, dependency wiring, and deterministic startup order."
                .to_string(),
            capability_refs: vec!["connectivity.local".to_string()],
            source_refs: vec![format!("technical:{:?}", spec.technical.product_envelope)],
        },
        RuntimeSystemBoundary {
            system_id: "space_runtime".to_string(),
            responsibility: match spec.capabilities.space.topology {
                SpaceTopology::Lane => {
                    "Own lane topology, slots, and protected-edge traversal.".to_string()
                }
                _ => "Own authored spatial topology and placement surfaces.".to_string(),
            },
            capability_refs: vec![format!("space.{:?}", spec.capabilities.space.topology)],
            source_refs: spec.spaces.keys().map(|id| format!("space:{id}")).collect(),
        },
        RuntimeSystemBoundary {
            system_id: "resource_runtime".to_string(),
            responsibility:
                "Own resource state, costs, production, spending, and rejection reasons."
                    .to_string(),
            capability_refs: vec![format!(
                "control.{:?}",
                spec.capabilities.control.directness
            )],
            source_refs: spec
                .resources
                .keys()
                .map(|id| format!("resource:{id}"))
                .collect(),
        },
        RuntimeSystemBoundary {
            system_id: "wave_runtime".to_string(),
            responsibility: if spec.capabilities.content.generation == ContentGeneration::Authored {
                "Own authored wave schedules, enemy pressure, and resolution states.".to_string()
            } else {
                "Own content-driven encounter scheduling within the frozen envelope.".to_string()
            },
            capability_refs: vec![format!(
                "content.{:?}",
                spec.capabilities.content.generation
            )],
            source_refs: spec
                .content
                .keys()
                .map(|id| format!("content:{id}"))
                .collect(),
        },
        RuntimeSystemBoundary {
            system_id: "ui_feedback_runtime".to_string(),
            responsibility:
                "Own HUD state, player feedback, accessibility signals, and rejection messaging."
                    .to_string(),
            capability_refs: vec![format!(
                "information.{:?}",
                spec.capabilities.information.visibility
            )],
            source_refs: spec
                .presentation
                .keys()
                .map(|id| format!("presentation:{id}"))
                .collect(),
        },
        RuntimeSystemBoundary {
            system_id: "save_pause_runtime".to_string(),
            responsibility: "Own save/load, pause/resume, and settings persistence.".to_string(),
            capability_refs: vec![format!(
                "progression.{:?}",
                spec.capabilities.progression.persistence
            )],
            source_refs: spec
                .acceptance_scenarios
                .keys()
                .filter(|id| id.as_str().contains("save") || id.as_str().contains("pause"))
                .map(|id| format!("scenario:{id}"))
                .collect(),
        },
        RuntimeSystemBoundary {
            system_id: "asset_manifest_runtime".to_string(),
            responsibility: "Own frozen asset manifest loading and reference binding validation."
                .to_string(),
            capability_refs: vec!["presentation.asset_binding".to_string()],
            source_refs: vec!["step07:style_anchor_set".to_string()],
        },
        RuntimeSystemBoundary {
            system_id: "acceptance_runtime".to_string(),
            responsibility: "Own executable R1 acceptance harness and evidence bundle production."
                .to_string(),
            capability_refs: vec!["acceptance.executable".to_string()],
            source_refs: spec
                .acceptance_scenarios
                .keys()
                .map(|id| format!("scenario:{id}"))
                .collect(),
        },
    ];
    systems.sort_by(|left, right| left.system_id.cmp(&right.system_id));
    systems
}

fn task(
    task_id: &str,
    title: &str,
    ordinal_size: &str,
    system_id: &str,
    dependencies: Vec<&str>,
    write_paths: Vec<&str>,
    base_tree_hash: &str,
) -> TrustedDevelopmentTask {
    let write_paths = write_paths
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let command_id = format!("test_{}", stable_suffix(task_id));
    let trusted_test_path = format!("Tests/Trusted/{}.trusted.json", stable_suffix(task_id));
    let trusted_test_body = format!("{{\"task\":\"{task_id}\",\"assert\":\"machine_check\"}}\n");
    let trusted_test_hash = sha256_hex(trusted_test_body.as_bytes());
    let contract = workspace_contract(
        task_id,
        base_tree_hash,
        &write_paths,
        &trusted_test_path,
        &trusted_test_hash,
        &command_id,
    );
    TrustedDevelopmentTask {
        task_id: task_id.to_string(),
        title: title.to_string(),
        ordinal_size: ordinal_size.to_string(),
        architecture_system_id: system_id.to_string(),
        declared_write_paths: write_paths.clone(),
        machine_checks: vec![MachineAcceptanceCheck {
            check_id: format!("check_{}", stable_suffix(task_id)),
            compile_target: "unity_editmode".to_string(),
            trusted_test_id: format!("trusted_{}", stable_suffix(task_id)),
            command_id,
        }],
        dependencies: dependencies.into_iter().map(str::to_string).collect(),
        rollback_boundary: write_paths,
        workspace_contract: contract,
    }
}

fn workspace_contract(
    task_id: &str,
    base_tree_hash: &str,
    write_paths: &[String],
    trusted_test_path: &str,
    trusted_test_hash: &str,
    test_command_id: &str,
) -> WorkspaceChangeSet {
    let primary_write = rel(&write_paths[0]);
    let trusted_test = rel(trusted_test_path);
    let test_id = format!("trusted_{}", stable_suffix(task_id));
    WorkspaceChangeSet {
        schema_version: WORKSPACE_CHANGE_SET_SCHEMA_VERSION.to_string(),
        change_set_id: stable_suffix(task_id),
        base_tree_hash: base_tree_hash.to_string(),
        read_paths: BTreeSet::from([
            rel("ProjectSettings/ProjectVersion.txt"),
            trusted_test.clone(),
        ]),
        agent_write_paths: write_paths.iter().map(rel).collect(),
        trusted_tool_write_paths: BTreeSet::new(),
        build_output_paths: BTreeSet::from([rel(&format!(
            "Build/Validation/{}",
            stable_suffix(task_id)
        ))]),
        operations: vec![WorkspaceOperation::WriteFile {
            path: primary_write,
            expected: WorkspaceFileExpectation::Missing,
            payload: WorkspaceFilePayload::utf8(format!(
                "// sealed task stub for {task_id}; Step11 replaces this in isolated execution\n"
            )),
        }],
        command_permissions: vec![
            CommandPermission {
                command_id: "compile_unity_editmode".to_string(),
                tool_binding_id: "unity_editor_batchmode".to_string(),
                purpose: CommandPurpose::Compile,
                argument_template: vec!["-runEditorCompilation".to_string()],
                working_directory: None,
                timeout_ms: 120_000,
                allow_network: false,
            },
            CommandPermission {
                command_id: test_command_id.to_string(),
                tool_binding_id: "unity_editor_batchmode".to_string(),
                purpose: CommandPurpose::Test,
                argument_template: vec![
                    "-runTests".to_string(),
                    "-testPlatform".to_string(),
                    "EditMode".to_string(),
                    trusted_test_path.to_string(),
                ],
                working_directory: None,
                timeout_ms: 120_000,
                allow_network: false,
            },
        ],
        trusted_tests: vec![TrustedTestContract {
            test_id,
            path: trusted_test,
            baseline_sha256: trusted_test_hash.to_string(),
            command_id: test_command_id.to_string(),
        }],
        resource_budget: WorkspaceResourceBudget {
            max_duration_ms: 180_000,
            max_processes: 4,
            max_write_bytes: 256_000,
            max_file_count: 8,
            max_retries: 2,
        },
        evidence: vec![ChangeEvidence::from_bytes(
            "task_contract_sealed",
            "step10",
            EvidenceStatus::Observed,
            format!("{task_id}:{base_tree_hash}:{trusted_test_hash}").as_bytes(),
        )],
    }
}

fn validate_task_graph(tasks: &[TrustedDevelopmentTask]) -> TaskGraphValidation {
    let task_ids = tasks
        .iter()
        .map(|task| task.task_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut issues = Vec::new();
    let mut invalid_contract_count = 0usize;
    for task in tasks {
        for dep in &task.dependencies {
            if !task_ids.contains(dep.as_str()) {
                issues.push(format!(
                    "task {} depends on unknown task {dep}",
                    task.task_id
                ));
            }
        }
        let report = task.workspace_contract.validate();
        if !report.is_valid() {
            invalid_contract_count += 1;
            issues.extend(
                report
                    .issues
                    .into_iter()
                    .map(|issue| format!("{}:{}", task.task_id, issue.code)),
            );
        }
        if task.declared_write_paths.is_empty()
            || task.machine_checks.is_empty()
            || task.rollback_boundary.is_empty()
        {
            issues.push(format!(
                "task {} is missing contract elements",
                task.task_id
            ));
        }
    }
    let acyclic = graph_is_acyclic(tasks);
    if !acyclic {
        issues.push("task dependency graph contains a cycle".to_string());
    }
    TaskGraphValidation {
        acyclic,
        contract_count: tasks.len(),
        invalid_contract_count,
        issues,
    }
}

fn graph_is_acyclic(tasks: &[TrustedDevelopmentTask]) -> bool {
    let mut incoming = tasks
        .iter()
        .map(|task| (task.task_id.clone(), task.dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = tasks
        .iter()
        .map(|task| (task.task_id.clone(), Vec::<String>::new()))
        .collect::<BTreeMap<_, _>>();
    for task in tasks {
        for dep in &task.dependencies {
            if let Some(edges) = outgoing.get_mut(dep) {
                edges.push(task.task_id.clone());
            }
        }
    }
    let mut ready = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(id, _)| id.clone())
        .collect::<VecDeque<_>>();
    let mut visited = 0usize;
    while let Some(id) = ready.pop_front() {
        visited += 1;
        for next in outgoing.remove(&id).unwrap_or_default() {
            if let Some(count) = incoming.get_mut(&next) {
                *count -= 1;
                if *count == 0 {
                    ready.push_back(next);
                }
            }
        }
    }
    visited == tasks.len()
}

fn anti_overfit_evidence(spec: &GameSpec) -> AntiOverfitEvidence {
    let baseline = runtime_systems(spec)
        .into_iter()
        .map(|system| system.system_id)
        .collect::<Vec<_>>();
    let label_stable = permute_display_labels(spec, "permuted_architecture_label")
        .map(|permuted| {
            runtime_systems(&permuted)
                .into_iter()
                .map(|system| system.system_id)
                .collect::<Vec<_>>()
                == baseline
        })
        .unwrap_or(false);
    let (axis_count, changed_axes, unchanged_axes, failed_axes) =
        capability_mutation_system_evidence(spec);
    let changed = !changed_axes.is_empty() && failed_axes.is_empty();
    AntiOverfitEvidence {
        label_permutation_systems_stable: label_stable,
        capability_mutation_systems_changed: changed,
        mutation_kind: "multi_axis_capability_suite".to_string(),
        capability_mutation_axis_count: axis_count,
        capability_mutation_changed_axes: changed_axes,
        capability_mutation_unchanged_axes: unchanged_axes,
        capability_mutation_failed_axes: failed_axes,
    }
}

fn capability_mutation_system_evidence(
    spec: &GameSpec,
) -> (usize, Vec<String>, Vec<String>, Vec<String>) {
    let baseline = runtime_systems(spec)
        .into_iter()
        .map(|system| {
            (
                system.system_id,
                system.responsibility,
                system.capability_refs,
            )
        })
        .collect::<Vec<_>>();
    let mut changed_axes = Vec::new();
    let mut unchanged_axes = Vec::new();
    let mut failed_axes = Vec::new();
    let suite = capability_mutation_suite(spec);
    for case in &suite {
        match apply_capability_mutation(spec, case.mutation) {
            Ok(mutated) => {
                let mutated_systems = runtime_systems(&mutated)
                    .into_iter()
                    .map(|system| {
                        (
                            system.system_id,
                            system.responsibility,
                            system.capability_refs,
                        )
                    })
                    .collect::<Vec<_>>();
                if mutated_systems != baseline {
                    changed_axes.push(case.axis.to_string());
                } else {
                    unchanged_axes.push(case.axis.to_string());
                }
            }
            Err(_) => failed_axes.push(case.axis.to_string()),
        }
    }
    (suite.len(), changed_axes, unchanged_axes, failed_axes)
}

fn stable_suffix(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn rel(value: impl AsRef<str>) -> WorkspaceRelativePath {
    WorkspaceRelativePath::parse(value.as_ref()).expect("generated workspace path must be valid")
}

fn hash_json(value: &Value) -> AdmResult<String> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|error| AdmError::new(format!("failed to hash json: {error}")))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::step07_v2::{compile_step07_art_direction, confirm_style_anchors_attended};

    fn r1_fixture() -> GameSpec {
        parse_game_spec(include_str!(
            "../../../../testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"
        ))
        .unwrap()
    }

    fn temp_root(prefix: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(adm_new_foundation::new_stable_id(prefix).unwrap());
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn r1c0_fixture_compiles_architecture_manifest_and_trusted_task_graph() {
        let root = temp_root("step08_10_v2_r1c0");
        let step07_dir = root.join("step07");
        compile_step07_art_direction(&r1_fixture(), &step07_dir).unwrap();
        let anchors = confirm_style_anchors_attended(&step07_dir, "tester", "approved", "attended")
            .unwrap()
            .anchors;

        let compiled = compile_step08_10(&r1_fixture(), &anchors, &root.join("step08_10")).unwrap();

        assert_eq!(compiled.status, "success");
        assert!(
            compiled
                .architecture
                .anti_overfit
                .label_permutation_systems_stable
        );
        assert!(
            compiled
                .architecture
                .anti_overfit
                .capability_mutation_systems_changed
        );
        assert_eq!(
            compiled
                .architecture
                .anti_overfit
                .capability_mutation_axis_count,
            16
        );
        assert!(
            compiled
                .architecture
                .anti_overfit
                .capability_mutation_failed_axes
                .is_empty()
        );
        assert!(
            compiled
                .architecture
                .anti_overfit
                .capability_mutation_changed_axes
                .contains(&"space_topology".to_string())
        );
        assert!(!compiled.asset_manifest.items.is_empty());
        assert_eq!(compiled.task_graph.validation.acyclic, true);
        assert_eq!(compiled.task_graph.validation.invalid_contract_count, 0);
        for task in &compiled.task_graph.tasks {
            assert!(!task.declared_write_paths.is_empty());
            assert!(!task.machine_checks.is_empty());
            assert!(!task.dependencies.contains(&task.task_id));
            assert!(!task.rollback_boundary.is_empty());
            assert!(task.workspace_contract.validate().is_valid());
        }
        assert!(root.join("step08_10/runtime_architecture.json").exists());
        assert!(root.join("step08_10/frozen_asset_manifest.json").exists());
        assert!(root.join("step08_10/trusted_task_graph.json").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn repeated_compile_hashes_are_stable() {
        let root = temp_root("step08_10_v2_stable");
        let first = compile_step08_10(&r1_fixture(), &[], &root.join("first")).unwrap();
        let second = compile_step08_10(&r1_fixture(), &[], &root.join("second")).unwrap();

        assert_eq!(
            first.architecture.semantic_hash,
            second.architecture.semantic_hash
        );
        assert_eq!(
            first.asset_manifest.frozen_hash,
            second.asset_manifest.frozen_hash
        );
        assert_eq!(
            first.task_graph.semantic_hash,
            second.task_graph.semantic_hash
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn task_graph_validation_detects_cycles() {
        let source_hash = "a".repeat(64);
        let mut tasks = vec![
            task(
                "a.task",
                "A",
                "S",
                "runtime_bootstrap",
                vec!["b.task"],
                vec!["Assets/A.cs"],
                &source_hash,
            ),
            task(
                "b.task",
                "B",
                "S",
                "runtime_bootstrap",
                vec!["a.task"],
                vec!["Assets/B.cs"],
                &source_hash,
            ),
        ];
        tasks[0].workspace_contract.base_tree_hash = source_hash.clone();
        tasks[1].workspace_contract.base_tree_hash = source_hash;

        let validation = validate_task_graph(&tasks);

        assert!(!validation.acyclic);
        assert!(
            validation
                .issues
                .iter()
                .any(|issue| issue.contains("cycle"))
        );
    }
}
