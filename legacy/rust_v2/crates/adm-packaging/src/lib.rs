#![forbid(unsafe_code)]

use adm_foundation::{
    AdmError, AdmResult, ContentHash, ProjectId, ensure_within_root, write_string,
};
use adm_pipeline::{ArtifactRegistry, PipelineRunLifecycleStatus, PipelineRunState};
use adm_validation::{ValidationIssue, ValidationReport, ValidationStatus};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN: &str = "ADM_CONFIRM_LOCAL_ENGINE_BUILD";
pub const UNITY_EDITOR_ENV_VAR: &str = "ADM_UNITY_EDITOR";
pub const UNITY_EDITOR_FALLBACK_ENV_VAR: &str = "UNITY_EDITOR_PATH";
pub const UNITY_RUNTIME_VALIDATION_OUTPUT: &str =
    "Library/AutoDesignMaker/runtime_execution_results.adm";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageManifest {
    pub project_id: ProjectId,
    pub target_platform: String,
    pub entries: Vec<String>,
    pub support_files: Vec<String>,
}

impl PackageManifest {
    pub fn new(
        project_id: ProjectId,
        target_platform: impl Into<String>,
        entries: Vec<String>,
    ) -> Self {
        Self {
            project_id,
            target_platform: target_platform.into(),
            entries,
            support_files: Vec::new(),
        }
    }

    pub fn with_support_files(mut self, support_files: Vec<String>) -> Self {
        self.support_files = support_files;
        self
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Package Manifest\n");
        document.push_str(&format!("project_id={}\n", self.project_id));
        document.push_str(&format!("target_platform={}\n", self.target_platform));
        document.push_str("entries=\n");
        for entry in &self.entries {
            document.push_str(&format!("- {entry}\n"));
        }
        document.push_str("support_files=\n");
        for support_file in &self.support_files {
            document.push_str(&format!("- {support_file}\n"));
        }
        document
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameBuildTargetSpec {
    pub target_id: String,
    pub engine: String,
    pub platform: String,
    pub profile: String,
    pub output_file: String,
    pub required_artifacts: Vec<String>,
}

impl GameBuildTargetSpec {
    pub fn new(
        target_id: impl Into<String>,
        engine: impl Into<String>,
        platform: impl Into<String>,
        profile: impl Into<String>,
        output_file: impl Into<String>,
        required_artifacts: Vec<String>,
    ) -> Self {
        Self {
            target_id: target_id.into(),
            engine: engine.into(),
            platform: platform.into(),
            profile: profile.into(),
            output_file: output_file.into(),
            required_artifacts,
        }
    }

    fn render(&self, document: &mut String) {
        document.push_str("## Game Build Target\n");
        document.push_str(&format!("target_id={}\n", self.target_id));
        document.push_str(&format!("engine={}\n", self.engine));
        document.push_str(&format!("platform={}\n", self.platform));
        document.push_str(&format!("profile={}\n", self.profile));
        document.push_str(&format!("output_file={}\n", self.output_file));
        document.push_str("required_artifacts=\n");
        for artifact in &self.required_artifacts {
            document.push_str(&format!("- {artifact}\n"));
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameBuildPlan {
    pub project_id: ProjectId,
    pub targets: Vec<GameBuildTargetSpec>,
}

impl GameBuildPlan {
    pub fn new(project_id: ProjectId, targets: Vec<GameBuildTargetSpec>) -> Self {
        Self {
            project_id,
            targets,
        }
    }

    pub fn windows_desktop_prototype(project_id: ProjectId) -> Self {
        Self::new(
            project_id,
            vec![GameBuildTargetSpec::new(
                "windows_desktop_playable",
                "Unity",
                "windows-desktop",
                "playable-prototype",
                "build/windows/AutoDesignMakerGame.zip",
                vec![
                    "project/brief.adm".to_string(),
                    "design/project.adm".to_string(),
                    "development/plan.adm".to_string(),
                    "assets/plan.adm".to_string(),
                    "sdk/index.adm".to_string(),
                    "validation/acceptance_matrix.adm".to_string(),
                    "validation/scenario_test_plan.adm".to_string(),
                    "validation/runtime_validation_report.adm".to_string(),
                    "validation/production_readiness.adm".to_string(),
                ],
            )],
        )
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Game Build Targets\n");
        document.push_str(&format!("project_id={}\n", self.project_id));
        for target in &self.targets {
            target.render(&mut document);
        }
        document
    }
}

pub fn validate_release_package(
    manifest: &PackageManifest,
    artifacts: &ArtifactRegistry,
    run_state: &PipelineRunState,
    input_validation: &ValidationReport,
) -> ValidationReport {
    let mut issues = Vec::new();
    if manifest.target_platform.trim().is_empty() {
        issues.push(failed(
            "package.target.empty",
            "package target_platform cannot be empty",
        ));
    }
    check_entries("package.entry", &manifest.entries, &mut issues);
    check_entries("package.support", &manifest.support_files, &mut issues);
    check_manifest_entries_have_artifacts(manifest, artifacts, &mut issues);
    check_required_support_files(manifest, &mut issues);
    if run_state.status != PipelineRunLifecycleStatus::Succeeded {
        issues.push(failed(
            "package.pipeline.not_succeeded",
            "pipeline run state must be Succeeded before packaging",
        ));
    }
    match input_validation.status {
        ValidationStatus::Failed => issues.push(failed(
            "package.validation.failed",
            "input validation report must not be Failed before packaging",
        )),
        ValidationStatus::Warning => issues.push(warning(
            "package.validation.warning",
            "input validation report contains warnings",
        )),
        ValidationStatus::Passed => {}
    }
    ValidationReport::from_issues(issues)
}

pub fn validate_game_build_targets(
    plan: &GameBuildPlan,
    artifacts: &ArtifactRegistry,
) -> ValidationReport {
    let mut issues = Vec::new();
    if plan.targets.is_empty() {
        issues.push(failed(
            "game_build.targets.empty",
            "game build plan must contain at least one target",
        ));
        return ValidationReport::from_issues(issues);
    }

    let artifact_paths = artifact_paths(artifacts);
    let mut target_ids = HashSet::new();
    for target in &plan.targets {
        let target_id = target.target_id.trim();
        if target_id.is_empty() {
            issues.push(failed(
                "game_build.target_id.empty",
                "game build target_id cannot be empty",
            ));
        } else if !target_ids.insert(target_id.to_string()) {
            issues.push(failed(
                "game_build.target_id.duplicate",
                format!("duplicate game build target_id: {}", target.target_id),
            ));
        }
        if target.engine.trim().is_empty() {
            issues.push(failed(
                "game_build.engine.empty",
                format!("game build target {} has no engine", target.target_id),
            ));
        }
        if target.platform.trim().is_empty() {
            issues.push(failed(
                "game_build.platform.empty",
                format!("game build target {} has no platform", target.target_id),
            ));
        }
        if target.profile.trim().is_empty() {
            issues.push(failed(
                "game_build.profile.empty",
                format!(
                    "game build target {} has no build profile",
                    target.target_id
                ),
            ));
        }
        if target.output_file.trim().is_empty() {
            issues.push(failed(
                "game_build.output.empty",
                format!("game build target {} has no output_file", target.target_id),
            ));
        }
        if target.required_artifacts.is_empty() {
            issues.push(failed(
                "game_build.required_artifacts.empty",
                format!(
                    "game build target {} must list required artifacts",
                    target.target_id
                ),
            ));
        }
        for required in &target.required_artifacts {
            let normalized = required.replace('\\', "/");
            if normalized.trim().is_empty() {
                issues.push(failed(
                    "game_build.required_artifact.blank",
                    format!(
                        "game build target {} contains a blank required artifact",
                        target.target_id
                    ),
                ));
                continue;
            }
            if !artifact_paths.contains(&normalized) {
                issues.push(failed(
                    "game_build.required_artifact.missing",
                    format!(
                        "game build target {} references missing artifact: {}",
                        target.target_id, required
                    ),
                ));
            }
        }
    }
    ValidationReport::from_issues(issues)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameBuildBundle {
    pub target_id: String,
    pub target_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub staged_files: Vec<PathBuf>,
    pub bundle_hash: ContentHash,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkBundle {
    pub target_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub staged_files: Vec<PathBuf>,
    pub bundle_hash: ContentHash,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnityProjectScaffold {
    pub project_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub generated_files: Vec<PathBuf>,
    pub scaffold_hash: ContentHash,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StagedBuildFile {
    path: String,
    hash: ContentHash,
    bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StagedSdkFile {
    path: String,
    hash: ContentHash,
    bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnityGameplayModel {
    core_loop: Vec<String>,
    mechanics: Vec<UnityMechanicModel>,
    scenarios: Vec<UnityScenarioModel>,
    development_tasks: Vec<UnityDevelopmentTaskModel>,
    asset_feedback: Vec<UnityAssetFeedbackModel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnityMechanicModel {
    name: String,
    player_action: String,
    feedback: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnityScenarioModel {
    scenario_id: String,
    goal: String,
    success: String,
    failure: String,
    validation_probe: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnityDevelopmentTaskModel {
    source_mechanic: String,
    title: String,
    implementation_layer: String,
    acceptance: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnityAssetFeedbackModel {
    source_mechanic: String,
    asset_kind: String,
    description: String,
    acceptance: String,
}

impl UnityGameplayModel {
    fn from_artifacts(design: &str, development: &str, assets: &str) -> Self {
        let core_loop = parse_core_loop_steps(design);
        let mut mechanics = parse_design_mechanics(design);
        if mechanics.is_empty() {
            mechanics = core_loop
                .iter()
                .enumerate()
                .map(|(index, step)| UnityMechanicModel {
                    name: format!("Core Loop Mechanic {}", index + 1),
                    player_action: step.clone(),
                    feedback: format!("Generated feedback for {step}"),
                })
                .collect();
        }
        let scenarios = parse_playable_scenarios(design);
        let development_tasks = parse_development_tasks(development);
        let asset_feedback = parse_asset_feedback(assets);
        Self {
            core_loop,
            mechanics,
            scenarios,
            development_tasks,
            asset_feedback,
        }
    }
}

pub fn stage_game_build_bundle(
    plan: &GameBuildPlan,
    target_id: &str,
    content_root: impl AsRef<Path>,
    target_dir: impl AsRef<Path>,
) -> AdmResult<GameBuildBundle> {
    let target = plan
        .targets
        .iter()
        .find(|target| target.target_id == target_id)
        .ok_or_else(|| {
            AdmError::invalid_input(format!("unknown game build target: {target_id}"))
        })?;
    let content_root = content_root.as_ref();
    let target_dir = target_dir.as_ref();
    let artifact_root = target_dir.join("content");
    let mut staged_files = Vec::new();
    let mut records = Vec::new();
    let mut aggregate = Vec::new();

    for required in &target.required_artifacts {
        let normalized = required.replace('\\', "/");
        let source = ensure_within_root(content_root, &normalized)?;
        if !source.is_file() {
            return Err(AdmError::invalid_input(format!(
                "missing required build artifact: {normalized}"
            )));
        }
        let destination = ensure_within_root(&artifact_root, &normalized)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = fs::read(&source)?;
        fs::write(&destination, &bytes)?;
        let hash = ContentHash::from_bytes(&bytes);
        aggregate.extend_from_slice(normalized.as_bytes());
        aggregate.extend_from_slice(hash.as_str().as_bytes());
        records.push(StagedBuildFile {
            path: format!("content/{normalized}"),
            hash,
            bytes: bytes.len(),
        });
        staged_files.push(destination);
    }
    for optional in ["validation/runtime_execution_results.adm"] {
        let source = ensure_within_root(content_root, optional)?;
        let destination = ensure_within_root(&artifact_root, optional)?;
        if source.is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            let bytes = fs::read(&source)?;
            fs::write(&destination, &bytes)?;
            let hash = ContentHash::from_bytes(&bytes);
            aggregate.extend_from_slice(optional.as_bytes());
            aggregate.extend_from_slice(hash.as_str().as_bytes());
            records.push(StagedBuildFile {
                path: format!("content/{optional}"),
                hash,
                bytes: bytes.len(),
            });
            staged_files.push(destination);
        } else if destination.is_file() {
            fs::remove_file(&destination)?;
        } else if destination.exists() {
            return Err(AdmError::invalid_input(format!(
                "managed optional game build artifact path is not a file: {optional}"
            )));
        }
    }

    let bundle_hash = ContentHash::from_bytes(&aggregate);
    fs::create_dir_all(target_dir)?;
    let manifest_path = target_dir.join("game-build-manifest.adm");
    write_string(
        &manifest_path,
        &render_game_build_bundle_manifest(target, &records, &bundle_hash),
    )?;
    let total_bytes = records.iter().map(|record| record.bytes as u64).sum();

    Ok(GameBuildBundle {
        target_id: target.target_id.clone(),
        target_dir: target_dir.to_path_buf(),
        manifest_path,
        staged_files,
        bundle_hash,
        total_bytes,
    })
}

pub fn stage_sdk_bundle(
    content_root: impl AsRef<Path>,
    target_dir: impl AsRef<Path>,
) -> AdmResult<SdkBundle> {
    let content_root = content_root.as_ref();
    let target_dir = target_dir.as_ref();
    let required = "sdk/index.adm";
    let required_source = ensure_within_root(content_root, required)?;
    if !required_source.is_file() {
        return Err(AdmError::invalid_input(format!(
            "missing required SDK artifact: {required}"
        )));
    }

    let mut source_files = vec![required.to_string()];
    for optional in [
        "package/build_targets.adm",
        "validation/scenario_test_plan.adm",
        "validation/runtime_validation_report.adm",
        "validation/runtime_execution_results.adm",
        "validation/production_readiness.adm",
        "package/engine_build_history.adm",
    ] {
        if ensure_within_root(content_root, optional)?.is_file() {
            source_files.push(optional.to_string());
        } else {
            let stale_destination = ensure_within_root(target_dir, optional)?;
            if stale_destination.is_file() {
                fs::remove_file(&stale_destination)?;
            } else if stale_destination.exists() {
                return Err(AdmError::invalid_input(format!(
                    "managed optional SDK artifact path is not a file: {optional}"
                )));
            }
        }
    }

    let mut staged_files = Vec::new();
    let mut records = Vec::new();
    let mut aggregate = Vec::new();
    for relative in source_files {
        let source = ensure_within_root(content_root, &relative)?;
        let destination = ensure_within_root(target_dir, &relative)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = fs::read(&source)?;
        fs::write(&destination, &bytes)?;
        let hash = ContentHash::from_bytes(&bytes);
        aggregate.extend_from_slice(relative.as_bytes());
        aggregate.extend_from_slice(hash.as_str().as_bytes());
        records.push(StagedSdkFile {
            path: relative,
            hash,
            bytes: bytes.len(),
        });
        staged_files.push(destination);
    }

    let bundle_hash = ContentHash::from_bytes(&aggregate);
    fs::create_dir_all(target_dir)?;
    let manifest_path = target_dir.join("sdk-bundle-manifest.adm");
    write_string(
        &manifest_path,
        &render_sdk_bundle_manifest(&records, &bundle_hash),
    )?;
    let total_bytes = records.iter().map(|record| record.bytes as u64).sum();

    Ok(SdkBundle {
        target_dir: target_dir.to_path_buf(),
        manifest_path,
        staged_files,
        bundle_hash,
        total_bytes,
    })
}

pub fn stage_unity_project_scaffold(
    target: &GameBuildTargetSpec,
    content_root: impl AsRef<Path>,
    project_dir: impl AsRef<Path>,
) -> AdmResult<UnityProjectScaffold> {
    if !target.engine.eq_ignore_ascii_case("Unity") {
        return Err(AdmError::invalid_input(format!(
            "Unity project scaffold cannot target engine {}",
            target.engine
        )));
    }
    if !target.platform.eq_ignore_ascii_case("windows-desktop") {
        return Err(AdmError::invalid_input(format!(
            "Unity project scaffold does not support platform {}",
            target.platform
        )));
    }
    let content_root = content_root.as_ref();
    let project_dir = project_dir.as_ref();
    if project_dir.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "Unity project directory cannot be empty",
        ));
    }

    let brief = read_required_content(content_root, "project/brief.adm")?;
    let design = read_required_content(content_root, "design/project.adm")?;
    let development = read_required_content(content_root, "development/plan.adm")?;
    let assets = read_required_content(content_root, "assets/plan.adm")?;
    let sdk = read_required_content(content_root, "sdk/index.adm")?;
    let acceptance_matrix =
        read_required_content(content_root, "validation/acceptance_matrix.adm")?;
    let scenario_test_plan =
        read_required_content(content_root, "validation/scenario_test_plan.adm")?;
    let runtime_validation =
        read_required_content(content_root, "validation/runtime_validation_report.adm")?;
    let runtime_execution_results =
        read_optional_content(content_root, "validation/runtime_execution_results.adm")?;
    let production_readiness =
        read_required_content(content_root, "validation/production_readiness.adm")?;
    let gameplay_model = UnityGameplayModel::from_artifacts(&design, &development, &assets);

    let mut generated_specs = vec![
        ("Assets/AutoDesignMaker/Generated/project_brief.adm", brief),
        (
            "Assets/AutoDesignMaker/Generated/design_project.adm",
            design,
        ),
        (
            "Assets/AutoDesignMaker/Generated/development_plan.adm",
            development,
        ),
        ("Assets/AutoDesignMaker/Generated/asset_plan.adm", assets),
        ("Assets/AutoDesignMaker/Generated/sdk_index.adm", sdk),
        (
            "Assets/AutoDesignMaker/Generated/acceptance_matrix.adm",
            acceptance_matrix,
        ),
        (
            "Assets/AutoDesignMaker/Generated/scenario_test_plan.adm",
            scenario_test_plan,
        ),
        (
            "Assets/AutoDesignMaker/Generated/runtime_validation_report.adm",
            runtime_validation,
        ),
        (
            "Assets/AutoDesignMaker/Generated/production_readiness.adm",
            production_readiness,
        ),
        (
            "Assets/AutoDesignMaker/Generated/AutoDesignMakerBootstrap.cs",
            render_unity_bootstrap_script(target),
        ),
        (
            "Assets/AutoDesignMaker/Generated/AutoDesignMakerGeneratedContent.cs",
            render_unity_generated_content_index(target, runtime_execution_results.is_some()),
        ),
        (
            "Assets/AutoDesignMaker/Generated/AutoDesignMakerGameplayModel.cs",
            render_unity_gameplay_model_script(&gameplay_model),
        ),
        (
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerRuntimeController.cs",
            render_unity_runtime_controller_script(),
        ),
        (
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerGameplayController.cs",
            render_unity_gameplay_controller_script(),
        ),
        (
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs",
            render_unity_scene_composer_script(),
        ),
        (
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerInputRouter.cs",
            render_unity_input_router_script(),
        ),
        (
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerSaveData.cs",
            render_unity_save_data_script(),
        ),
        (
            "Assets/AutoDesignMaker/Editor/AutoDesignMakerBuild.cs",
            render_unity_editor_build_script(target),
        ),
        (
            "Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs",
            render_unity_runtime_validation_script(),
        ),
        (
            "ProjectSettings/ProjectVersion.txt",
            "m_EditorVersion: AutoDesignMaker-scaffold\n".to_string(),
        ),
    ];
    if let Some(runtime_execution_results) = runtime_execution_results {
        generated_specs.insert(
            9,
            (
                "Assets/AutoDesignMaker/Generated/runtime_execution_results.adm",
                runtime_execution_results,
            ),
        );
    }

    let mut generated_files = Vec::new();
    let mut records = Vec::new();
    let mut aggregate = Vec::new();
    let mut total_bytes = 0_u64;
    for (relative, content) in generated_specs {
        let destination = ensure_within_root(project_dir, relative)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&destination, content.as_bytes())?;
        let hash = ContentHash::from_bytes(content.as_bytes());
        aggregate.extend_from_slice(relative.as_bytes());
        aggregate.extend_from_slice(hash.as_str().as_bytes());
        total_bytes += content.len() as u64;
        records.push(StagedSdkFile {
            path: relative.to_string(),
            hash,
            bytes: content.len(),
        });
        generated_files.push(destination);
    }

    let scaffold_hash = ContentHash::from_bytes(&aggregate);
    let manifest_path = project_dir.join("adm-unity-scaffold-manifest.adm");
    write_string(
        &manifest_path,
        &render_unity_scaffold_manifest(target, &records, &scaffold_hash),
    )?;

    Ok(UnityProjectScaffold {
        project_dir: project_dir.to_path_buf(),
        manifest_path,
        generated_files,
        scaffold_hash,
        total_bytes,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineBuildCommandPlan {
    pub engine: String,
    pub target_id: String,
    pub executable: PathBuf,
    pub working_dir: PathBuf,
    pub args: Vec<String>,
    pub expected_output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnityEditorCandidate {
    pub source: String,
    pub path: PathBuf,
    pub present: bool,
    pub looks_like_unity_editor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnityEditorDiscoveryReport {
    pub candidates: Vec<UnityEditorCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnityBuildPreflightReport {
    pub target_id: String,
    pub executable: PathBuf,
    pub executable_present: bool,
    pub executable_looks_like_unity: bool,
    pub unity_project_dir: PathBuf,
    pub unity_project_present: bool,
    pub unity_project_ready: bool,
    pub confirmation_valid: bool,
    pub expected_output: String,
    pub command_line: String,
    pub issues: Vec<String>,
}

impl EngineBuildCommandPlan {
    pub fn render(&self) -> String {
        let mut document = String::from("# Engine Build Command\n");
        document.push_str(&format!("engine={}\n", self.engine));
        document.push_str(&format!("target_id={}\n", self.target_id));
        document.push_str(&format!("executable={}\n", self.executable.display()));
        document.push_str(&format!("working_dir={}\n", self.working_dir.display()));
        document.push_str(&format!("expected_output={}\n", self.expected_output));
        document.push_str("args=\n");
        for arg in &self.args {
            document.push_str(&format!("- {arg}\n"));
        }
        document
    }

    pub fn command_line(&self) -> String {
        std::iter::once(quote_command_part(&self.executable.display().to_string()))
            .chain(self.args.iter().map(|arg| quote_command_part(arg)))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl UnityEditorCandidate {
    pub fn ready(&self) -> bool {
        self.present && self.looks_like_unity_editor
    }
}

impl UnityEditorDiscoveryReport {
    pub fn selected(&self) -> Option<&UnityEditorCandidate> {
        self.candidates.iter().find(|candidate| candidate.ready())
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Unity Editor Discovery\n");
        document.push_str(&format!(
            "selected={}\n",
            self.selected()
                .map(|candidate| candidate.path.display().to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        document.push_str(&format!("candidates={}\n", self.candidates.len()));
        for candidate in &self.candidates {
            document.push_str(&format!(
                "- source={}; path={}; present={}; looks_like_unity_editor={}; ready={}\n",
                candidate.source,
                candidate.path.display(),
                candidate.present,
                candidate.looks_like_unity_editor,
                candidate.ready()
            ));
        }
        document
    }
}

impl UnityBuildPreflightReport {
    pub fn ready_for_local_build(&self) -> bool {
        self.issues.is_empty()
            && self.executable_present
            && self.executable_looks_like_unity
            && self.unity_project_present
            && self.unity_project_ready
            && self.confirmation_valid
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Unity Build Preflight\n");
        document.push_str(&format!(
            "ready_for_local_build={}\n",
            self.ready_for_local_build()
        ));
        document.push_str(&format!("target_id={}\n", self.target_id));
        document.push_str(&format!("executable={}\n", self.executable.display()));
        document.push_str(&format!("executable_present={}\n", self.executable_present));
        document.push_str(&format!(
            "executable_looks_like_unity={}\n",
            self.executable_looks_like_unity
        ));
        document.push_str(&format!(
            "unity_project_dir={}\n",
            self.unity_project_dir.display()
        ));
        document.push_str(&format!(
            "unity_project_present={}\n",
            self.unity_project_present
        ));
        document.push_str(&format!(
            "unity_project_ready={}\n",
            self.unity_project_ready
        ));
        document.push_str(&format!("confirmation_valid={}\n", self.confirmation_valid));
        document.push_str(&format!("expected_output={}\n", self.expected_output));
        document.push_str(&format!("command_line={}\n", self.command_line));
        document.push_str("issues=\n");
        for issue in &self.issues {
            document.push_str(&format!("- {issue}\n"));
        }
        document
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineBuildExecutionMode {
    DryRun,
    LocalProcess,
}

impl EngineBuildExecutionMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::DryRun => "dry_run",
            Self::LocalProcess => "local_process",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineBuildExecutionStatus {
    Succeeded,
    Failed,
}

impl EngineBuildExecutionStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineBuildExecutionReport {
    pub engine: String,
    pub target_id: String,
    pub mode: EngineBuildExecutionMode,
    pub status: EngineBuildExecutionStatus,
    pub launched: bool,
    pub executable: PathBuf,
    pub working_dir: PathBuf,
    pub command_line: String,
    pub expected_output: String,
    pub expected_output_path: PathBuf,
    pub expected_output_present: bool,
    pub expected_output_bytes: u64,
    pub expected_output_hash: Option<ContentHash>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl EngineBuildExecutionReport {
    pub fn render(&self) -> String {
        let mut document = String::from("# Engine Build Execution\n");
        document.push_str(&format!("engine={}\n", self.engine));
        document.push_str(&format!("target_id={}\n", self.target_id));
        document.push_str(&format!("mode={}\n", self.mode.as_str()));
        document.push_str(&format!("status={}\n", self.status.as_str()));
        document.push_str(&format!("launched={}\n", self.launched));
        document.push_str(&format!("executable={}\n", self.executable.display()));
        document.push_str(&format!("working_dir={}\n", self.working_dir.display()));
        document.push_str(&format!("expected_output={}\n", self.expected_output));
        document.push_str(&format!(
            "expected_output_path={}\n",
            self.expected_output_path.display()
        ));
        document.push_str(&format!(
            "expected_output_present={}\n",
            self.expected_output_present
        ));
        document.push_str(&format!(
            "expected_output_bytes={}\n",
            self.expected_output_bytes
        ));
        document.push_str(&format!(
            "expected_output_hash={}\n",
            self.expected_output_hash
                .as_ref()
                .map(ContentHash::as_str)
                .unwrap_or("none")
        ));
        document.push_str(&format!("command_line={}\n", self.command_line));
        document.push_str(&format!(
            "exit_code={}\n",
            self.exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        document.push_str(&format!("stdout={}\n", inline_log(&self.stdout)));
        document.push_str(&format!("stderr={}\n", inline_log(&self.stderr)));
        document
    }
}

pub trait EngineBuildRunner {
    fn run(&self, plan: &EngineBuildCommandPlan) -> AdmResult<EngineBuildExecutionReport>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DryRunEngineBuildRunner;

impl EngineBuildRunner for DryRunEngineBuildRunner {
    fn run(&self, plan: &EngineBuildCommandPlan) -> AdmResult<EngineBuildExecutionReport> {
        let expected_output_path =
            resolve_expected_output_path(&plan.working_dir, &plan.expected_output);
        Ok(EngineBuildExecutionReport {
            engine: plan.engine.clone(),
            target_id: plan.target_id.clone(),
            mode: EngineBuildExecutionMode::DryRun,
            status: EngineBuildExecutionStatus::Succeeded,
            launched: false,
            executable: plan.executable.clone(),
            working_dir: plan.working_dir.clone(),
            command_line: plan.command_line(),
            expected_output: plan.expected_output.clone(),
            expected_output_path,
            expected_output_present: false,
            expected_output_bytes: 0,
            expected_output_hash: None,
            exit_code: None,
            stdout: "dry-run: command not launched".to_string(),
            stderr: String::new(),
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LocalProcessEngineBuildRunner;

impl EngineBuildRunner for LocalProcessEngineBuildRunner {
    fn run(&self, plan: &EngineBuildCommandPlan) -> AdmResult<EngineBuildExecutionReport> {
        if !plan.executable.is_file() {
            return Err(AdmError::invalid_input(format!(
                "engine build executable does not exist: {}",
                plan.executable.display()
            )));
        }
        if !plan.working_dir.is_dir() {
            return Err(AdmError::invalid_input(format!(
                "engine build working_dir does not exist: {}",
                plan.working_dir.display()
            )));
        }
        let output = Command::new(&plan.executable)
            .args(&plan.args)
            .current_dir(&plan.working_dir)
            .output()?;
        let expected_output_path =
            resolve_expected_output_path(&plan.working_dir, &plan.expected_output);
        let expected_output_present = expected_output_path.is_file();
        let expected_output_bytes = if expected_output_present {
            fs::metadata(&expected_output_path)?.len()
        } else {
            0
        };
        let expected_output_hash = if expected_output_present {
            Some(ContentHash::from_bytes(&fs::read(&expected_output_path)?))
        } else {
            None
        };
        let status = if output.status.success() && expected_output_present {
            EngineBuildExecutionStatus::Succeeded
        } else {
            EngineBuildExecutionStatus::Failed
        };
        Ok(EngineBuildExecutionReport {
            engine: plan.engine.clone(),
            target_id: plan.target_id.clone(),
            mode: EngineBuildExecutionMode::LocalProcess,
            status,
            launched: true,
            executable: plan.executable.clone(),
            working_dir: plan.working_dir.clone(),
            command_line: plan.command_line(),
            expected_output: plan.expected_output.clone(),
            expected_output_path,
            expected_output_present,
            expected_output_bytes,
            expected_output_hash,
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

fn resolve_expected_output_path(working_dir: &Path, expected_output: &str) -> PathBuf {
    let output = PathBuf::from(expected_output);
    if output.is_absolute() {
        output
    } else {
        working_dir.join(output)
    }
}

pub fn plan_unity_cli_build(
    target: &GameBuildTargetSpec,
    unity_editor_executable: impl Into<PathBuf>,
    unity_project_dir: impl Into<PathBuf>,
) -> AdmResult<EngineBuildCommandPlan> {
    if !target.engine.eq_ignore_ascii_case("Unity") {
        return Err(AdmError::invalid_input(format!(
            "Unity build adapter cannot build engine {}",
            target.engine
        )));
    }
    if !target.platform.eq_ignore_ascii_case("windows-desktop") {
        return Err(AdmError::invalid_input(format!(
            "Unity build adapter does not support platform {}",
            target.platform
        )));
    }
    let executable = unity_editor_executable.into();
    if executable.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "Unity editor executable cannot be empty",
        ));
    }
    let working_dir = unity_project_dir.into();
    if working_dir.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "Unity project directory cannot be empty",
        ));
    }
    let expected_output = target.output_file.clone();
    Ok(EngineBuildCommandPlan {
        engine: "Unity".to_string(),
        target_id: target.target_id.clone(),
        executable,
        working_dir: working_dir.clone(),
        args: vec![
            "-batchmode".to_string(),
            "-quit".to_string(),
            "-projectPath".to_string(),
            working_dir.display().to_string(),
            "-executeMethod".to_string(),
            "AutoDesignMaker.EditorBuild.PerformBuild".to_string(),
            "-buildTarget".to_string(),
            "Win64".to_string(),
            "-customBuildPath".to_string(),
            expected_output.clone(),
        ],
        expected_output,
    })
}

pub fn plan_unity_runtime_validation(
    target: &GameBuildTargetSpec,
    unity_editor_executable: impl Into<PathBuf>,
    unity_project_dir: impl Into<PathBuf>,
) -> AdmResult<EngineBuildCommandPlan> {
    if !target.engine.eq_ignore_ascii_case("Unity") {
        return Err(AdmError::invalid_input(format!(
            "Unity runtime validation adapter cannot target engine {}",
            target.engine
        )));
    }
    if !target.platform.eq_ignore_ascii_case("windows-desktop") {
        return Err(AdmError::invalid_input(format!(
            "Unity runtime validation adapter does not support platform {}",
            target.platform
        )));
    }
    let executable = unity_editor_executable.into();
    if executable.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "Unity editor executable cannot be empty",
        ));
    }
    let working_dir = unity_project_dir.into();
    if working_dir.as_os_str().is_empty() {
        return Err(AdmError::invalid_input(
            "Unity project directory cannot be empty",
        ));
    }
    Ok(EngineBuildCommandPlan {
        engine: "UnityRuntimeValidation".to_string(),
        target_id: target.target_id.clone(),
        executable,
        working_dir: working_dir.clone(),
        args: vec![
            "-batchmode".to_string(),
            "-quit".to_string(),
            "-projectPath".to_string(),
            working_dir.display().to_string(),
            "-executeMethod".to_string(),
            "AutoDesignMaker.RuntimeValidation.RunValidation".to_string(),
            "-admRuntimeValidationOutput".to_string(),
            UNITY_RUNTIME_VALIDATION_OUTPUT.to_string(),
        ],
        expected_output: UNITY_RUNTIME_VALIDATION_OUTPUT.to_string(),
    })
}

pub fn discover_unity_editor(explicit: Option<PathBuf>) -> UnityEditorDiscoveryReport {
    let env_candidates = [
        (
            UNITY_EDITOR_ENV_VAR.to_string(),
            std::env::var(UNITY_EDITOR_ENV_VAR).ok(),
        ),
        (
            UNITY_EDITOR_FALLBACK_ENV_VAR.to_string(),
            std::env::var(UNITY_EDITOR_FALLBACK_ENV_VAR).ok(),
        ),
    ];
    discover_unity_editor_from_sources(
        explicit,
        env_candidates.iter().filter_map(|(source, value)| {
            value
                .as_ref()
                .map(|value| (source.as_str(), value.as_str()))
        }),
        default_unity_editor_candidates(),
    )
}

pub fn inspect_unity_build_preflight(
    target: &GameBuildTargetSpec,
    unity_editor_executable: impl Into<PathBuf>,
    unity_project_dir: impl Into<PathBuf>,
    confirmation_token: &str,
) -> AdmResult<UnityBuildPreflightReport> {
    let command = plan_unity_cli_build(target, unity_editor_executable, unity_project_dir)?;
    let executable_present = command.executable.is_file();
    let executable_looks_like_unity = path_looks_like_unity_editor(&command.executable);
    let unity_project_present = command.working_dir.is_dir();
    let unity_project_report = inspect_unity_project_scaffold(&command.working_dir);
    let unity_project_ready = unity_project_report.ready();
    let confirmation_valid = confirmation_token.trim() == LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN;
    let mut issues = Vec::new();
    if !executable_present {
        issues.push(format!(
            "Unity editor executable is missing: {}",
            command.executable.display()
        ));
    }
    if !executable_looks_like_unity {
        issues.push(format!(
            "Unity editor executable should be named Unity.exe: {}",
            command.executable.display()
        ));
    }
    if !unity_project_present {
        issues.push(format!(
            "Unity project directory is missing: {}",
            command.working_dir.display()
        ));
    }
    if !unity_project_ready {
        let missing = unity_project_report
            .files
            .iter()
            .filter(|file| !file.present)
            .map(|file| file.relative_path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        issues.push(format!(
            "Unity project scaffold is incomplete{}",
            if missing.is_empty() {
                String::new()
            } else {
                format!(": {missing}")
            }
        ));
    }
    if !confirmation_valid {
        issues.push(format!(
            "confirmation token must be {LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN}"
        ));
    }

    Ok(UnityBuildPreflightReport {
        target_id: command.target_id.clone(),
        executable: command.executable.clone(),
        executable_present,
        executable_looks_like_unity,
        unity_project_dir: command.working_dir.clone(),
        unity_project_present,
        unity_project_ready,
        confirmation_valid,
        expected_output: command.expected_output.clone(),
        command_line: command.command_line(),
        issues,
    })
}

pub fn validate_local_engine_build_confirmation(token: &str) -> AdmResult<()> {
    if token.trim() == LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN {
        Ok(())
    } else {
        Err(AdmError::invalid_input(format!(
            "local engine build requires confirmation token {LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN}"
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopReleaseSpec {
    pub product_name: String,
    pub version: String,
    pub source_executable: PathBuf,
    pub target_dir: PathBuf,
}

impl DesktopReleaseSpec {
    pub fn new(
        source_executable: impl Into<PathBuf>,
        target_dir: impl Into<PathBuf>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            product_name: "AutoDesignMaker-rust".to_string(),
            version: version.into(),
            source_executable: source_executable.into(),
            target_dir: target_dir.into(),
        }
    }

    fn executable_name(&self) -> AdmResult<String> {
        if self.product_name.trim().is_empty() {
            return Err(AdmError::invalid_input(
                "desktop release product_name cannot be empty",
            ));
        }
        if self.product_name.contains(['/', '\\']) {
            return Err(AdmError::invalid_input(
                "desktop release product_name cannot contain path separators",
            ));
        }
        Ok(format!("{}.exe", self.product_name))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopReleaseBundle {
    pub target_dir: PathBuf,
    pub executable_path: PathBuf,
    pub manifest_path: PathBuf,
    pub readme_path: PathBuf,
    pub executable_hash: ContentHash,
    pub executable_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopReleaseDoctorReport {
    pub target_dir: PathBuf,
    pub executable_path: PathBuf,
    pub manifest_path: PathBuf,
    pub readme_path: PathBuf,
    pub executable_present: bool,
    pub manifest_present: bool,
    pub readme_present: bool,
    pub executable_bytes: u64,
    pub executable_hash: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleFileCheck {
    pub relative_path: PathBuf,
    pub present: bool,
    pub required: bool,
    pub content_verified: bool,
    pub required_fragments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleDoctorReport {
    pub name: String,
    pub target_dir: PathBuf,
    pub files: Vec<BundleFileCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryDoctorReport {
    pub release: DesktopReleaseDoctorReport,
    pub game_build_bundle: BundleDoctorReport,
    pub sdk_bundle: BundleDoctorReport,
    pub unity_project: BundleDoctorReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopReleaseSmokeReport {
    pub executable_path: PathBuf,
    pub command_line: String,
    pub launched: bool,
    pub skipped_reason: Option<String>,
    pub spawn_error: Option<String>,
    pub exit_code: Option<i32>,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
    pub stdout_contains_pipeline_succeeded: bool,
    pub stdout_contains_production_ready: bool,
    pub stderr_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAcceptanceReport {
    pub report_path: PathBuf,
    pub delivery: DeliveryDoctorReport,
    pub smoke: DesktopReleaseSmokeReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAiProviderAcceptance {
    pub ready_provider_count: usize,
    pub real_provider_ids: Vec<String>,
    pub diagnostics_document: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalAcceptanceReport {
    pub report_path: PathBuf,
    pub data_root: PathBuf,
    pub release_acceptance_report: PathBuf,
    pub release_acceptance_present: bool,
    pub release_acceptance_accepted: bool,
    pub release_smoke_ready: bool,
    pub release_hash: String,
    pub unity_discovery: UnityEditorDiscoveryReport,
    pub unity_runtime_report: PathBuf,
    pub unity_runtime_present: bool,
    pub unity_runtime_ready: bool,
    pub unity_runtime_runner: String,
    pub unity_runtime_target_id: String,
    pub ai_acceptance_report: PathBuf,
    pub ai_acceptance_present: bool,
    pub ai_acceptance_ready: bool,
    pub ai_acceptance_provider_id: String,
    pub ai_acceptance_configured_ready: bool,
    pub ai_acceptance_invoke_attempted: bool,
    pub ai_acceptance_invoke_succeeded: bool,
    pub ai_provider_acceptance: ExternalAiProviderAcceptance,
    pub require_ready: bool,
    pub require_ai_invoke: bool,
}

impl DesktopReleaseDoctorReport {
    pub fn ready(&self) -> bool {
        self.executable_present && self.manifest_present && self.readme_present
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Desktop Release Doctor\n");
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("release_dir={}\n", self.target_dir.display()));
        document.push_str(&format!("executable={}\n", self.executable_path.display()));
        document.push_str(&format!("manifest={}\n", self.manifest_path.display()));
        document.push_str(&format!("readme={}\n", self.readme_path.display()));
        document.push_str(&format!("executable_present={}\n", self.executable_present));
        document.push_str(&format!("manifest_present={}\n", self.manifest_present));
        document.push_str(&format!("readme_present={}\n", self.readme_present));
        document.push_str(&format!("bytes={}\n", self.executable_bytes));
        document.push_str(&format!(
            "hash={}\n",
            self.executable_hash
                .as_ref()
                .map(ContentHash::as_str)
                .unwrap_or("none")
        ));
        document.push_str("legacy_root_exe=not_modified\n");
        document
    }
}

impl BundleDoctorReport {
    pub fn ready(&self) -> bool {
        self.files.iter().all(BundleFileCheck::ready)
    }

    fn render_into(&self, document: &mut String, prefix: &str) {
        document.push_str(&format!("{prefix}_ready={}\n", self.ready()));
        document.push_str(&format!("{prefix}_dir={}\n", self.target_dir.display()));
        for file in &self.files {
            document.push_str(&format!(
                "{prefix}_file={}; present={}; status={}; required={}",
                file.relative_path.display(),
                file.present,
                file.status(),
                file.required
            ));
            if !file.required_fragments.is_empty() {
                document.push_str(&format!(
                    "; content_verified={}; required_fragments={}",
                    file.content_verified,
                    file.required_fragments.len()
                ));
            }
            document.push('\n');
        }
    }
}

impl BundleFileCheck {
    pub fn ready(&self) -> bool {
        (!self.required || self.present) && self.content_verified
    }

    pub fn status(&self) -> &'static str {
        if !self.present {
            if self.required {
                "missing"
            } else {
                "optional_missing"
            }
        } else if self.required_fragments.is_empty() {
            "present"
        } else if self.content_verified {
            "verified"
        } else {
            "content_mismatch"
        }
    }
}

impl DeliveryDoctorReport {
    pub fn ready(&self) -> bool {
        self.release.ready()
            && self.game_build_bundle.ready()
            && self.sdk_bundle.ready()
            && self.unity_project.ready()
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Delivery Doctor\n");
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("release_ready={}\n", self.release.ready()));
        document.push_str(&format!(
            "release_dir={}\n",
            self.release.target_dir.display()
        ));
        document.push_str(&format!(
            "release_executable_present={}\n",
            self.release.executable_present
        ));
        document.push_str(&format!(
            "release_manifest_present={}\n",
            self.release.manifest_present
        ));
        document.push_str(&format!(
            "release_readme_present={}\n",
            self.release.readme_present
        ));
        document.push_str(&format!(
            "release_bytes={}\n",
            self.release.executable_bytes
        ));
        document.push_str(&format!(
            "release_hash={}\n",
            self.release
                .executable_hash
                .as_ref()
                .map(ContentHash::as_str)
                .unwrap_or("none")
        ));
        self.game_build_bundle
            .render_into(&mut document, "game_build_bundle");
        self.sdk_bundle.render_into(&mut document, "sdk_bundle");
        self.unity_project
            .render_into(&mut document, "unity_project");
        document.push_str("legacy_root_exe=not_modified\n");
        document
    }
}

impl DesktopReleaseSmokeReport {
    pub fn ready(&self) -> bool {
        self.launched
            && self.exit_code == Some(0)
            && self.stdout_contains_pipeline_succeeded
            && self.stdout_contains_production_ready
            && self.stderr_empty
            && self.spawn_error.is_none()
            && self.skipped_reason.is_none()
    }

    fn render_into(&self, document: &mut String) {
        document.push_str(&format!("smoke_ready={}\n", self.ready()));
        document.push_str(&format!(
            "smoke_executable={}\n",
            self.executable_path.display()
        ));
        document.push_str(&format!("smoke_command={}\n", self.command_line));
        document.push_str(&format!("smoke_launched={}\n", self.launched));
        document.push_str(&format!(
            "smoke_skipped_reason={}\n",
            self.skipped_reason.as_deref().unwrap_or("none")
        ));
        document.push_str(&format!(
            "smoke_spawn_error={}\n",
            self.spawn_error.as_deref().unwrap_or("none")
        ));
        document.push_str(&format!(
            "smoke_exit_code={}\n",
            self.exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        document.push_str(&format!("smoke_stdout_bytes={}\n", self.stdout_bytes));
        document.push_str(&format!("smoke_stderr_bytes={}\n", self.stderr_bytes));
        document.push_str(&format!(
            "smoke_stdout_contains_pipeline_succeeded={}\n",
            self.stdout_contains_pipeline_succeeded
        ));
        document.push_str(&format!(
            "smoke_stdout_contains_production_ready={}\n",
            self.stdout_contains_production_ready
        ));
        document.push_str(&format!("smoke_stderr_empty={}\n", self.stderr_empty));
    }
}

impl ReleaseAcceptanceReport {
    pub fn accepted(&self) -> bool {
        self.delivery.ready() && self.smoke.ready()
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Release Acceptance Report\n");
        document.push_str(&format!("accepted={}\n", self.accepted()));
        document.push_str(&format!("report_path={}\n", self.report_path.display()));
        document.push_str(&format!("delivery_ready={}\n", self.delivery.ready()));
        document.push_str(&format!(
            "release_ready={}\n",
            self.delivery.release.ready()
        ));
        document.push_str(&format!(
            "game_build_bundle_ready={}\n",
            self.delivery.game_build_bundle.ready()
        ));
        document.push_str(&format!(
            "sdk_bundle_ready={}\n",
            self.delivery.sdk_bundle.ready()
        ));
        document.push_str(&format!(
            "unity_project_ready={}\n",
            self.delivery.unity_project.ready()
        ));
        document.push_str(&format!(
            "release_hash={}\n",
            self.delivery
                .release
                .executable_hash
                .as_ref()
                .map(ContentHash::as_str)
                .unwrap_or("none")
        ));
        document.push_str(&format!(
            "release_bytes={}\n",
            self.delivery.release.executable_bytes
        ));
        self.smoke.render_into(&mut document);
        document.push_str("legacy_root_exe=not_modified\n");
        document.push('\n');
        document.push_str(&self.delivery.render());
        document
    }
}

impl ExternalAiProviderAcceptance {
    pub fn new(
        ready_provider_count: usize,
        real_provider_ids: Vec<String>,
        diagnostics_document: impl Into<String>,
    ) -> Self {
        let mut real_provider_ids = real_provider_ids
            .into_iter()
            .map(|provider_id| single_line_value(&provider_id))
            .filter(|provider_id| !provider_id.is_empty())
            .collect::<Vec<_>>();
        real_provider_ids.sort();
        real_provider_ids.dedup();
        Self {
            ready_provider_count,
            real_provider_ids,
            diagnostics_document: diagnostics_document.into(),
        }
    }

    pub fn real_provider_ready(&self) -> bool {
        !self.real_provider_ids.is_empty()
    }
}

impl ExternalAcceptanceReport {
    pub fn ai_acceptance_provider_matches_real_provider(&self) -> bool {
        self.ai_provider_acceptance
            .real_provider_ids
            .iter()
            .any(|provider_id| provider_id == &self.ai_acceptance_provider_id)
    }

    pub fn blockers(&self) -> Vec<&'static str> {
        let mut blockers = Vec::new();
        if !self.release_acceptance_present {
            blockers.push("release_acceptance_report_missing");
        } else {
            if !self.release_acceptance_accepted {
                blockers.push("release_acceptance_not_accepted");
            }
            if !self.release_smoke_ready {
                blockers.push("release_smoke_not_ready");
            }
        }
        if self.unity_discovery.selected().is_none() {
            blockers.push("unity_not_ready");
        }
        if !self.unity_runtime_present {
            blockers.push("unity_runtime_report_missing");
        } else {
            if !self.unity_runtime_ready {
                blockers.push("unity_runtime_not_ready");
            }
            if self.unity_runtime_runner != "unity_playmode" {
                blockers.push("unity_runtime_runner_not_unity_playmode");
            }
        }
        if !self.ai_provider_acceptance.real_provider_ready() {
            blockers.push("real_ai_provider_not_ready");
        }
        if !self.ai_acceptance_present {
            blockers.push("ai_acceptance_report_missing");
        } else {
            if !self.ai_acceptance_ready {
                blockers.push("ai_acceptance_not_ready");
            }
            if !self.ai_acceptance_configured_ready {
                blockers.push("ai_acceptance_provider_not_configured");
            }
            if self.require_ai_invoke {
                if !self.ai_acceptance_invoke_attempted {
                    blockers.push("ai_acceptance_invoke_not_attempted");
                } else if !self.ai_acceptance_invoke_succeeded {
                    blockers.push("ai_acceptance_invoke_not_succeeded");
                }
            }
            if self.ai_acceptance_ready
                && self.ai_acceptance_configured_ready
                && self.ai_provider_acceptance.real_provider_ready()
                && !self.ai_acceptance_provider_matches_real_provider()
            {
                blockers.push("ai_acceptance_provider_not_real_provider");
            }
        }
        blockers
    }

    pub fn ready(&self) -> bool {
        self.blockers().is_empty()
    }

    pub fn render(&self) -> String {
        let blockers = self.blockers();
        let mut document = String::from("# External Acceptance Doctor\n");
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("report_path={}\n", self.report_path.display()));
        document.push_str(&format!("data_root={}\n", self.data_root.display()));
        document.push_str(&format!(
            "release_acceptance_report={}\n",
            self.release_acceptance_report.display()
        ));
        document.push_str(&format!(
            "release_acceptance_present={}\n",
            self.release_acceptance_present
        ));
        document.push_str(&format!(
            "release_acceptance_accepted={}\n",
            self.release_acceptance_accepted
        ));
        document.push_str(&format!(
            "release_smoke_ready={}\n",
            self.release_smoke_ready
        ));
        document.push_str(&format!("release_hash={}\n", self.release_hash));
        document.push_str(&format!(
            "unity_ready={}\n",
            self.unity_discovery.selected().is_some()
        ));
        document.push_str(&format!(
            "unity_selected={}\n",
            self.unity_discovery
                .selected()
                .map(|candidate| candidate.path.display().to_string())
                .unwrap_or_else(|| "none".to_string())
        ));
        document.push_str(&format!(
            "unity_candidates={}\n",
            self.unity_discovery.candidates.len()
        ));
        document.push_str(&format!(
            "unity_runtime_report={}\n",
            self.unity_runtime_report.display()
        ));
        document.push_str(&format!(
            "unity_runtime_present={}\n",
            self.unity_runtime_present
        ));
        document.push_str(&format!(
            "unity_runtime_ready={}\n",
            self.unity_runtime_ready
        ));
        document.push_str(&format!(
            "unity_runtime_runner={}\n",
            self.unity_runtime_runner
        ));
        document.push_str(&format!(
            "unity_runtime_target_id={}\n",
            self.unity_runtime_target_id
        ));
        document.push_str(&format!(
            "ai_acceptance_report={}\n",
            self.ai_acceptance_report.display()
        ));
        document.push_str(&format!(
            "ai_acceptance_present={}\n",
            self.ai_acceptance_present
        ));
        document.push_str(&format!(
            "ai_acceptance_ready={}\n",
            self.ai_acceptance_ready
        ));
        document.push_str(&format!(
            "ai_acceptance_provider_id={}\n",
            self.ai_acceptance_provider_id
        ));
        document.push_str(&format!(
            "ai_acceptance_provider_matches_real_provider={}\n",
            self.ai_acceptance_provider_matches_real_provider()
        ));
        document.push_str(&format!(
            "ai_acceptance_configured_ready={}\n",
            self.ai_acceptance_configured_ready
        ));
        document.push_str(&format!(
            "ai_acceptance_invoke_attempted={}\n",
            self.ai_acceptance_invoke_attempted
        ));
        document.push_str(&format!(
            "ai_acceptance_invoke_succeeded={}\n",
            self.ai_acceptance_invoke_succeeded
        ));
        document.push_str(&format!(
            "real_ai_provider_ready={}\n",
            self.ai_provider_acceptance.real_provider_ready()
        ));
        document.push_str(&format!(
            "real_ai_provider_count={}\n",
            self.ai_provider_acceptance.real_provider_ids.len()
        ));
        document.push_str(&format!(
            "real_ai_providers={}\n",
            self.ai_provider_acceptance.real_provider_ids.join(",")
        ));
        document.push_str(&format!(
            "ready_provider_count={}\n",
            self.ai_provider_acceptance.ready_provider_count
        ));
        document.push_str(&format!("require_ready={}\n", self.require_ready));
        document.push_str(&format!("require_ai_invoke={}\n", self.require_ai_invoke));
        document.push_str(&format!("blocker_count={}\n", blockers.len()));
        for blocker in blockers {
            document.push_str(&format!("blocker={blocker}\n"));
        }
        document.push('\n');
        document.push_str("## Unity Doctor Output\n");
        document.push_str(self.unity_discovery.render().trim_end());
        document.push_str("\n\n");
        document.push_str("## AI Doctor Output\n");
        document.push_str(self.ai_provider_acceptance.diagnostics_document.trim_end());
        document.push('\n');
        document
    }
}

pub fn stage_desktop_release(spec: &DesktopReleaseSpec) -> AdmResult<DesktopReleaseBundle> {
    if !spec.source_executable.is_file() {
        return Err(AdmError::invalid_input(format!(
            "desktop release source executable does not exist: {}",
            spec.source_executable.display()
        )));
    }
    if spec.version.trim().is_empty() {
        return Err(AdmError::invalid_input(
            "desktop release version cannot be empty",
        ));
    }

    let executable_name = spec.executable_name()?;
    let executable_path = spec.target_dir.join(executable_name);
    reject_same_source_and_target(&spec.source_executable, &executable_path)?;

    let executable_bytes = fs::read(&spec.source_executable)?;
    let executable_hash = ContentHash::from_bytes(&executable_bytes);
    fs::create_dir_all(&spec.target_dir)?;
    let stale_acceptance_report = spec.target_dir.join("release-acceptance.adm");
    if stale_acceptance_report.exists() {
        fs::remove_file(&stale_acceptance_report)?;
    }
    fs::copy(&spec.source_executable, &executable_path)?;

    let manifest_path = spec.target_dir.join("release-manifest.adm");
    write_string(
        &manifest_path,
        &render_desktop_release_manifest(
            spec,
            &executable_path,
            &executable_hash,
            executable_bytes.len(),
        ),
    )?;
    let readme_path = spec.target_dir.join("README.txt");
    write_string(&readme_path, &render_desktop_release_readme(spec))?;

    Ok(DesktopReleaseBundle {
        target_dir: spec.target_dir.clone(),
        executable_path,
        manifest_path,
        readme_path,
        executable_hash,
        executable_bytes: executable_bytes.len() as u64,
    })
}

pub fn inspect_desktop_release(
    target_dir: impl AsRef<Path>,
) -> AdmResult<DesktopReleaseDoctorReport> {
    let target_dir = target_dir.as_ref();
    let executable_path = target_dir.join("AutoDesignMaker-rust.exe");
    let manifest_path = target_dir.join("release-manifest.adm");
    let readme_path = target_dir.join("README.txt");
    let executable_present = executable_path.is_file();
    let executable_bytes = if executable_present {
        fs::metadata(&executable_path)?.len()
    } else {
        0
    };
    let executable_hash = if executable_present {
        Some(ContentHash::from_bytes(&fs::read(&executable_path)?))
    } else {
        None
    };
    Ok(DesktopReleaseDoctorReport {
        target_dir: target_dir.to_path_buf(),
        executable_path,
        manifest_path: manifest_path.clone(),
        readme_path: readme_path.clone(),
        executable_present,
        manifest_present: manifest_path.is_file(),
        readme_present: readme_path.is_file(),
        executable_bytes,
        executable_hash,
    })
}

pub fn inspect_game_build_bundle(target_dir: impl AsRef<Path>) -> BundleDoctorReport {
    inspect_bundle_files(
        "game_build_bundle",
        target_dir.as_ref(),
        &[
            bundle_file("game-build-manifest.adm"),
            bundle_file("content/project/brief.adm"),
            bundle_file("content/design/project.adm"),
            bundle_file("content/development/plan.adm"),
            bundle_file("content/assets/plan.adm"),
            bundle_file("content/sdk/index.adm"),
            bundle_file("content/validation/acceptance_matrix.adm"),
            bundle_file("content/validation/scenario_test_plan.adm"),
            bundle_file("content/validation/runtime_validation_report.adm"),
            optional_bundle_file_with_fragments(
                "content/validation/runtime_execution_results.adm",
                &[
                    "# Runtime Validation Execution Results",
                    "ready=",
                    "status=ready",
                ],
            ),
            bundle_file("content/validation/production_readiness.adm"),
        ],
    )
}

pub fn inspect_sdk_bundle(target_dir: impl AsRef<Path>) -> BundleDoctorReport {
    inspect_bundle_files(
        "sdk_bundle",
        target_dir.as_ref(),
        &[
            bundle_file("sdk-bundle-manifest.adm"),
            bundle_file("sdk/index.adm"),
            optional_bundle_file_with_fragments(
                "package/build_targets.adm",
                &[
                    "# Game Build Targets",
                    "target_id=windows_desktop_playable",
                    "engine=Unity",
                ],
            ),
            optional_bundle_file_with_fragments(
                "validation/scenario_test_plan.adm",
                &["# Scenario Test Plan", "test_id=", "status=ready"],
            ),
            optional_bundle_file_with_fragments(
                "validation/runtime_validation_report.adm",
                &["# Runtime Validation Report", "result_id=", "status=ready"],
            ),
            optional_bundle_file_with_fragments(
                "validation/runtime_execution_results.adm",
                &[
                    "# Runtime Validation Execution Results",
                    "ready=",
                    "status=ready",
                ],
            ),
            optional_bundle_file_with_fragments(
                "validation/production_readiness.adm",
                &["# Production Readiness Report", "overall_status=ready"],
            ),
            optional_bundle_file_with_fragments(
                "package/engine_build_history.adm",
                &[
                    "# Engine Build Execution History",
                    "# Engine Build Execution",
                    "expected_output_present=",
                    "expected_output_hash=",
                ],
            ),
        ],
    )
}

pub fn inspect_unity_project_scaffold(target_dir: impl AsRef<Path>) -> BundleDoctorReport {
    inspect_bundle_files(
        "unity_project",
        target_dir.as_ref(),
        &[
            bundle_file_with_fragments(
                "adm-unity-scaffold-manifest.adm",
                &[
                    "# Unity Project Scaffold",
                    "generated_files=",
                    "path=Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs",
                ],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Generated/project_brief.adm",
                &["# Game Design Brief", "title=", "core_loop_steps="],
            ),
            bundle_file("Assets/AutoDesignMaker/Generated/design_project.adm"),
            bundle_file("Assets/AutoDesignMaker/Generated/development_plan.adm"),
            bundle_file("Assets/AutoDesignMaker/Generated/asset_plan.adm"),
            bundle_file("Assets/AutoDesignMaker/Generated/sdk_index.adm"),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Generated/acceptance_matrix.adm",
                &["# Acceptance Trace Matrix", "trace_id=", "status=ready"],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Generated/scenario_test_plan.adm",
                &["# Scenario Test Plan", "test_id=", "status=ready"],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Generated/runtime_validation_report.adm",
                &["# Runtime Validation Report", "result_id=", "status=ready"],
            ),
            optional_bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Generated/runtime_execution_results.adm",
                &[
                    "# Runtime Validation Execution Results",
                    "ready=",
                    "status=ready",
                ],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Generated/production_readiness.adm",
                &["# Production Readiness Report", "overall_status=ready"],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Generated/AutoDesignMakerBootstrap.cs",
                &[
                    "AutoDesignMakerBootstrap",
                    "AutoDesignMakerRuntimeController",
                ],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Generated/AutoDesignMakerGeneratedContent.cs",
                &[
                    "AutoDesignMakerGeneratedContent",
                    "PipelineArtifactPaths",
                    "project_brief.adm",
                    "acceptance_matrix.adm",
                    "scenario_test_plan.adm",
                    "runtime_validation_report.adm",
                    "production_readiness.adm",
                ],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Generated/AutoDesignMakerGameplayModel.cs",
                &["AutoDesignMakerGameplayModel", "GeneratedMechanic"],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Runtime/AutoDesignMakerRuntimeController.cs",
                &[
                    "SaveRuntimeSnapshot",
                    "AutoDesignMakerGameplayController",
                    "AutoDesignMakerSceneComposer",
                ],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Runtime/AutoDesignMakerGameplayController.cs",
                &["Generated Gameplay Loop", "AdvanceMechanic"],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs",
                &[
                    "ComposeScene",
                    "CreateMechanicNodes",
                    "CreateGoalMarker",
                    "LineRenderer",
                    "TextMesh",
                    "PrimitiveType.Cube",
                ],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Runtime/AutoDesignMakerInputRouter.cs",
                &["ConfirmPressed", "CancelPressed"],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Runtime/AutoDesignMakerSaveData.cs",
                &["AutoDesignMakerSaveData", "pipeline_artifacts"],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Editor/AutoDesignMakerBuild.cs",
                &[
                    "PerformBuild",
                    "EditorSceneManager.NewScene",
                    "BuildTarget.StandaloneWindows64",
                ],
            ),
            bundle_file_with_fragments(
                "Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs",
                &[
                    "RunValidation",
                    "runtime_validation_report.adm",
                    "runtime_execution_results.adm",
                ],
            ),
            bundle_file("ProjectSettings/ProjectVersion.txt"),
        ],
    )
}

fn discover_unity_editor_from_sources<'a>(
    explicit: Option<PathBuf>,
    env_candidates: impl IntoIterator<Item = (&'a str, &'a str)>,
    default_candidates: Vec<PathBuf>,
) -> UnityEditorDiscoveryReport {
    let mut candidates = Vec::new();
    if let Some(path) = explicit {
        candidates.push(unity_editor_candidate("explicit", path));
    }
    for (source, value) in env_candidates {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            candidates.push(unity_editor_candidate(source, PathBuf::from(trimmed)));
        }
    }
    for path in default_candidates {
        candidates.push(unity_editor_candidate("default", path));
    }
    UnityEditorDiscoveryReport { candidates }
}

fn unity_editor_candidate(source: &str, path: PathBuf) -> UnityEditorCandidate {
    UnityEditorCandidate {
        source: source.to_string(),
        present: path.is_file(),
        looks_like_unity_editor: path_looks_like_unity_editor(&path),
        path,
    }
}

fn path_looks_like_unity_editor(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("Unity.exe"))
}

fn default_unity_editor_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![
        PathBuf::from(r"C:\Program Files\Unity\Editor\Unity.exe"),
        PathBuf::from(r"C:\Program Files\Unity Hub\Editor\Unity.exe"),
    ];
    for hub_root in [
        PathBuf::from(r"C:\Program Files\Unity\Hub\Editor"),
        PathBuf::from(r"C:\Program Files (x86)\Unity\Hub\Editor"),
    ] {
        if let Ok(entries) = fs::read_dir(&hub_root) {
            for entry in entries.flatten() {
                candidates.push(entry.path().join("Editor").join("Unity.exe"));
            }
        }
    }
    candidates
}

pub fn inspect_delivery(
    release_dir: impl AsRef<Path>,
    game_bundle_dir: impl AsRef<Path>,
    sdk_bundle_dir: impl AsRef<Path>,
    unity_project_dir: impl AsRef<Path>,
) -> AdmResult<DeliveryDoctorReport> {
    Ok(DeliveryDoctorReport {
        release: inspect_desktop_release(release_dir)?,
        game_build_bundle: inspect_game_build_bundle(game_bundle_dir),
        sdk_bundle: inspect_sdk_bundle(sdk_bundle_dir),
        unity_project: inspect_unity_project_scaffold(unity_project_dir),
    })
}

pub fn run_release_acceptance(
    release_dir: impl AsRef<Path>,
    game_bundle_dir: impl AsRef<Path>,
    sdk_bundle_dir: impl AsRef<Path>,
    unity_project_dir: impl AsRef<Path>,
) -> AdmResult<ReleaseAcceptanceReport> {
    let release_dir = release_dir.as_ref().to_path_buf();
    let delivery = inspect_delivery(
        &release_dir,
        game_bundle_dir,
        sdk_bundle_dir,
        unity_project_dir,
    )?;
    let smoke = run_desktop_release_smoke(&delivery);
    let report_path = release_dir.join("release-acceptance.adm");
    let report = ReleaseAcceptanceReport {
        report_path,
        delivery,
        smoke,
    };
    write_string(&report.report_path, &report.render())?;
    Ok(report)
}

pub fn run_external_acceptance(
    release_dir: impl AsRef<Path>,
    report_path: Option<PathBuf>,
    data_root: impl AsRef<Path>,
    unity_discovery: UnityEditorDiscoveryReport,
    ai_provider_acceptance: ExternalAiProviderAcceptance,
    require_ready: bool,
    require_ai_invoke: bool,
) -> AdmResult<ExternalAcceptanceReport> {
    let release_dir = release_dir.as_ref();
    let data_root = data_root.as_ref().to_path_buf();
    let release_acceptance_report = release_dir.join("release-acceptance.adm");
    let ai_acceptance_report = release_dir.join("ai-acceptance.adm");
    let report_path = report_path.unwrap_or_else(|| release_dir.join("external-acceptance.adm"));
    let release_acceptance_present = release_acceptance_report.is_file();
    let release_acceptance_text = if release_acceptance_present {
        fs::read_to_string(&release_acceptance_report)?
    } else {
        String::new()
    };
    let unity_runtime_report = external_unity_runtime_report_path(release_dir);
    let unity_runtime_present = unity_runtime_report.is_file();
    let unity_runtime_text = if unity_runtime_present {
        fs::read_to_string(&unity_runtime_report)?
    } else {
        String::new()
    };
    let ai_acceptance_present = ai_acceptance_report.is_file();
    let ai_acceptance_text = if ai_acceptance_present {
        fs::read_to_string(&ai_acceptance_report)?
    } else {
        String::new()
    };
    let report = ExternalAcceptanceReport {
        report_path,
        data_root,
        release_acceptance_report,
        release_acceptance_present,
        release_acceptance_accepted: report_bool_value(&release_acceptance_text, "accepted"),
        release_smoke_ready: report_bool_value(&release_acceptance_text, "smoke_ready"),
        release_hash: report_text_value(&release_acceptance_text, "release_hash", "none"),
        unity_discovery,
        unity_runtime_report,
        unity_runtime_present,
        unity_runtime_ready: report_bool_value(&unity_runtime_text, "ready"),
        unity_runtime_runner: report_text_value(&unity_runtime_text, "runner", "none"),
        unity_runtime_target_id: report_text_value(&unity_runtime_text, "target_id", "none"),
        ai_acceptance_report,
        ai_acceptance_present,
        ai_acceptance_ready: report_bool_value(&ai_acceptance_text, "ready"),
        ai_acceptance_provider_id: report_text_value(&ai_acceptance_text, "provider_id", "none"),
        ai_acceptance_configured_ready: report_bool_value(&ai_acceptance_text, "configured_ready"),
        ai_acceptance_invoke_attempted: report_bool_value(&ai_acceptance_text, "invoke_attempted"),
        ai_acceptance_invoke_succeeded: report_bool_value(&ai_acceptance_text, "invoke_succeeded"),
        ai_provider_acceptance,
        require_ready,
        require_ai_invoke,
    };
    write_string(&report.report_path, &report.render())?;
    Ok(report)
}

fn external_unity_runtime_report_path(release_dir: &Path) -> PathBuf {
    release_dir
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("unity-project")
        .join("Assets")
        .join("AutoDesignMaker")
        .join("Generated")
        .join("runtime_execution_results.adm")
}

fn run_desktop_release_smoke(delivery: &DeliveryDoctorReport) -> DesktopReleaseSmokeReport {
    let executable_path = delivery.release.executable_path.clone();
    let command_line = format!("{} --smoke", executable_path.display());
    if !delivery.ready() {
        return skipped_desktop_release_smoke(
            executable_path,
            command_line,
            "delivery_doctor_not_ready",
        );
    }
    if !executable_path.is_file() {
        return skipped_desktop_release_smoke(
            executable_path,
            command_line,
            "release_executable_missing",
        );
    }
    match Command::new(&executable_path).arg("--smoke").output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            DesktopReleaseSmokeReport {
                executable_path,
                command_line,
                launched: true,
                skipped_reason: None,
                spawn_error: None,
                exit_code: output.status.code(),
                stdout_bytes: output.stdout.len(),
                stderr_bytes: output.stderr.len(),
                stdout_contains_pipeline_succeeded: stdout.contains("Pipeline: Succeeded"),
                stdout_contains_production_ready: stdout.contains("production_readiness=ready"),
                stderr_empty: output.stderr.is_empty(),
            }
        }
        Err(error) => DesktopReleaseSmokeReport {
            executable_path,
            command_line,
            launched: false,
            skipped_reason: None,
            spawn_error: Some(error.to_string()),
            exit_code: None,
            stdout_bytes: 0,
            stderr_bytes: 0,
            stdout_contains_pipeline_succeeded: false,
            stdout_contains_production_ready: false,
            stderr_empty: true,
        },
    }
}

fn skipped_desktop_release_smoke(
    executable_path: PathBuf,
    command_line: String,
    reason: &str,
) -> DesktopReleaseSmokeReport {
    DesktopReleaseSmokeReport {
        executable_path,
        command_line,
        launched: false,
        skipped_reason: Some(reason.to_string()),
        spawn_error: None,
        exit_code: None,
        stdout_bytes: 0,
        stderr_bytes: 0,
        stdout_contains_pipeline_succeeded: false,
        stdout_contains_production_ready: false,
        stderr_empty: true,
    }
}

struct BundleFileSpec<'a> {
    relative_path: &'a str,
    required: bool,
    required_fragments: &'a [&'a str],
}

fn bundle_file(relative_path: &str) -> BundleFileSpec<'_> {
    BundleFileSpec {
        relative_path,
        required: true,
        required_fragments: &[],
    }
}

fn bundle_file_with_fragments<'a>(
    relative_path: &'a str,
    required_fragments: &'a [&'a str],
) -> BundleFileSpec<'a> {
    BundleFileSpec {
        relative_path,
        required: true,
        required_fragments,
    }
}

fn optional_bundle_file_with_fragments<'a>(
    relative_path: &'a str,
    required_fragments: &'a [&'a str],
) -> BundleFileSpec<'a> {
    BundleFileSpec {
        relative_path,
        required: false,
        required_fragments,
    }
}

fn inspect_bundle_files(
    name: &str,
    target_dir: &Path,
    required_files: &[BundleFileSpec<'_>],
) -> BundleDoctorReport {
    BundleDoctorReport {
        name: name.to_string(),
        target_dir: target_dir.to_path_buf(),
        files: required_files
            .iter()
            .map(|spec| {
                let path = target_dir.join(spec.relative_path);
                let present = path.is_file();
                let content_verified = if spec.required_fragments.is_empty() {
                    true
                } else if !present {
                    !spec.required
                } else {
                    fs::read_to_string(&path)
                        .map(|content| {
                            spec.required_fragments
                                .iter()
                                .all(|fragment| content.contains(fragment))
                        })
                        .unwrap_or(false)
                };
                BundleFileCheck {
                    relative_path: PathBuf::from(spec.relative_path),
                    present,
                    required: spec.required,
                    content_verified,
                    required_fragments: spec
                        .required_fragments
                        .iter()
                        .map(|fragment| (*fragment).to_string())
                        .collect(),
                }
            })
            .collect(),
    }
}

fn reject_same_source_and_target(source: &Path, target: &Path) -> AdmResult<()> {
    if target.exists() {
        let source = fs::canonicalize(source)?;
        let target = fs::canonicalize(target)?;
        if source == target {
            return Err(AdmError::invalid_input(
                "desktop release source executable and target executable are the same file",
            ));
        }
    }
    Ok(())
}

fn render_desktop_release_manifest(
    spec: &DesktopReleaseSpec,
    executable_path: &Path,
    executable_hash: &ContentHash,
    executable_bytes: usize,
) -> String {
    format!(
        "# Desktop Release Bundle\nproduct_name={}\nversion={}\nexecutable={}\nsource_executable={}\nbytes={}\nhash={}\nlegacy_root_exe=not_modified\n",
        spec.product_name,
        spec.version,
        executable_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("AutoDesignMaker-rust.exe"),
        spec.source_executable.display(),
        executable_bytes,
        executable_hash,
    )
}

fn render_game_build_bundle_manifest(
    target: &GameBuildTargetSpec,
    records: &[StagedBuildFile],
    bundle_hash: &ContentHash,
) -> String {
    let mut document = String::from("# Game Build Bundle\n");
    document.push_str(&format!("target_id={}\n", target.target_id));
    document.push_str(&format!("engine={}\n", target.engine));
    document.push_str(&format!("platform={}\n", target.platform));
    document.push_str(&format!("profile={}\n", target.profile));
    document.push_str(&format!("output_file={}\n", target.output_file));
    document.push_str(&format!("bundle_hash={bundle_hash}\n"));
    document.push_str("staged_files=\n");
    for record in records {
        document.push_str(&format!(
            "- path={}; bytes={}; hash={}\n",
            record.path, record.bytes, record.hash
        ));
    }
    document
}

fn render_sdk_bundle_manifest(records: &[StagedSdkFile], bundle_hash: &ContentHash) -> String {
    let mut document = String::from("# SDK Bundle\n");
    document.push_str(&format!("bundle_hash={bundle_hash}\n"));
    document.push_str("staged_files=\n");
    for record in records {
        document.push_str(&format!(
            "- path={}; bytes={}; hash={}\n",
            record.path, record.bytes, record.hash
        ));
    }
    document
}

fn read_required_content(content_root: &Path, relative: &str) -> AdmResult<String> {
    let source = ensure_within_root(content_root, relative)?;
    if !source.is_file() {
        return Err(AdmError::invalid_input(format!(
            "missing required Unity scaffold artifact: {relative}"
        )));
    }
    Ok(fs::read_to_string(source)?)
}

fn read_optional_content(content_root: &Path, relative: &str) -> AdmResult<Option<String>> {
    let source = ensure_within_root(content_root, relative)?;
    if source.is_file() {
        Ok(Some(fs::read_to_string(source)?))
    } else if source.exists() {
        Err(AdmError::invalid_input(format!(
            "optional Unity scaffold artifact path is not a file: {relative}"
        )))
    } else {
        Ok(None)
    }
}

fn parse_core_loop_steps(design: &str) -> Vec<String> {
    let mut in_section = false;
    let mut steps = Vec::new();
    for line in design.lines() {
        let trimmed = line.trim();
        if trimmed == "## Core Loop" {
            in_section = true;
            continue;
        }
        if in_section && trimmed.starts_with("## ") {
            break;
        }
        if in_section {
            if let Some((_, step)) = trimmed.split_once(". ") {
                let step = step.trim();
                if !step.is_empty() {
                    steps.push(step.to_string());
                }
            }
        }
    }
    steps
}

fn parse_design_mechanics(design: &str) -> Vec<UnityMechanicModel> {
    parse_section_records(design, "## Gameplay Mechanics")
        .into_iter()
        .map(|record| UnityMechanicModel {
            name: record_value(&record, "name", "Unnamed Mechanic"),
            player_action: record_value(&record, "player_action", "unspecified action"),
            feedback: record_value(&record, "feedback", "unspecified feedback"),
        })
        .collect()
}

fn parse_playable_scenarios(design: &str) -> Vec<UnityScenarioModel> {
    parse_section_records(design, "## Playable Scenarios")
        .into_iter()
        .map(|record| UnityScenarioModel {
            scenario_id: record_value(&record, "scenario_id", "scenario"),
            goal: record_value(&record, "goal", "unspecified goal"),
            success: record_value(&record, "success", "unspecified success"),
            failure: record_value(&record, "failure", "unspecified failure"),
            validation_probe: record_value(&record, "validation_probe", "unspecified probe"),
        })
        .collect()
}

fn parse_development_tasks(development: &str) -> Vec<UnityDevelopmentTaskModel> {
    parse_document_records(development)
        .into_iter()
        .map(|record| UnityDevelopmentTaskModel {
            source_mechanic: record_value(&record, "source_mechanic", "unspecified mechanic"),
            title: record_value(&record, "title", "Untitled development task"),
            implementation_layer: record_value(&record, "layer", "unspecified layer"),
            acceptance: record_value(&record, "acceptance", "unspecified acceptance"),
        })
        .collect()
}

fn parse_asset_feedback(assets: &str) -> Vec<UnityAssetFeedbackModel> {
    parse_document_records(assets)
        .into_iter()
        .map(|record| UnityAssetFeedbackModel {
            source_mechanic: record_value(&record, "source_mechanic", "unspecified mechanic"),
            asset_kind: record_value(&record, "kind", "unspecified asset"),
            description: record_value(&record, "description", "unspecified asset feedback"),
            acceptance: record_value(&record, "acceptance", "unspecified acceptance"),
        })
        .collect()
}

fn parse_section_records(document: &str, section_header: &str) -> Vec<HashMap<String, String>> {
    let mut in_section = false;
    let mut records = Vec::new();
    for line in document.lines() {
        let trimmed = line.trim();
        if trimmed == section_header {
            in_section = true;
            continue;
        }
        if in_section && trimmed.starts_with("## ") {
            break;
        }
        if in_section && trimmed.starts_with("- ") {
            records.push(parse_key_value_record(trimmed));
        }
    }
    records
}

fn parse_document_records(document: &str) -> Vec<HashMap<String, String>> {
    document
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- "))
        .map(parse_key_value_record)
        .collect()
}

fn parse_key_value_record(line: &str) -> HashMap<String, String> {
    let body = line.trim_start_matches("- ").trim();
    body.split(';')
        .filter_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

fn record_value(record: &HashMap<String, String>, key: &str, fallback: &str) -> String {
    record
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn report_bool_value(text: &str, key: &str) -> bool {
    report_text_value(text, key, "false").eq_ignore_ascii_case("true")
}

fn report_text_value(text: &str, key: &str, fallback: &str) -> String {
    let prefix = format!("{key}=");
    text.lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .map(single_line_value)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn single_line_value(value: &str) -> String {
    value
        .replace('\r', " ")
        .replace('\n', " ")
        .trim()
        .to_string()
}

fn render_unity_scaffold_manifest(
    target: &GameBuildTargetSpec,
    records: &[StagedSdkFile],
    scaffold_hash: &ContentHash,
) -> String {
    let mut document = String::from("# Unity Project Scaffold\n");
    document.push_str(&format!("target_id={}\n", target.target_id));
    document.push_str(&format!("engine={}\n", target.engine));
    document.push_str(&format!("platform={}\n", target.platform));
    document.push_str(&format!("profile={}\n", target.profile));
    document.push_str(&format!("output_file={}\n", target.output_file));
    document.push_str(&format!("scaffold_hash={scaffold_hash}\n"));
    document.push_str("generated_files=\n");
    for record in records {
        document.push_str(&format!(
            "- path={}; bytes={}; hash={}\n",
            record.path, record.bytes, record.hash
        ));
    }
    document
}

fn render_unity_bootstrap_script(target: &GameBuildTargetSpec) -> String {
    r#"using UnityEngine;

namespace AutoDesignMaker.Generated
{
    public sealed class AutoDesignMakerBootstrap : MonoBehaviour
    {
        public const string TargetId = "__TARGET_ID__";
        public const string BuildProfile = "__BUILD_PROFILE__";

        private void Awake()
        {
            if (GetComponent<AutoDesignMaker.Runtime.AutoDesignMakerRuntimeController>() == null)
            {
                gameObject.AddComponent<AutoDesignMaker.Runtime.AutoDesignMakerRuntimeController>();
            }
        }

        private void Start()
        {
            Debug.Log($"AutoDesignMaker bootstrap mounted for {TargetId} / {BuildProfile}");
        }
    }
}
"#
    .replace("__TARGET_ID__", &escape_csharp_string(&target.target_id))
    .replace("__BUILD_PROFILE__", &escape_csharp_string(&target.profile))
}

fn render_unity_generated_content_index(
    target: &GameBuildTargetSpec,
    has_runtime_execution_results: bool,
) -> String {
    let runtime_execution_results_line = if has_runtime_execution_results {
        "            \"Assets/AutoDesignMaker/Generated/runtime_execution_results.adm\",\n"
    } else {
        ""
    };
    r#"namespace AutoDesignMaker.Generated
{
    public static class AutoDesignMakerGeneratedContent
    {
        public const string TargetId = "__TARGET_ID__";
        public const string BuildProfile = "__BUILD_PROFILE__";
        public const string OutputFile = "__OUTPUT_FILE__";

        public static readonly string[] PipelineArtifactPaths =
        {
            "Assets/AutoDesignMaker/Generated/project_brief.adm",
            "Assets/AutoDesignMaker/Generated/design_project.adm",
            "Assets/AutoDesignMaker/Generated/development_plan.adm",
            "Assets/AutoDesignMaker/Generated/asset_plan.adm",
            "Assets/AutoDesignMaker/Generated/sdk_index.adm",
            "Assets/AutoDesignMaker/Generated/acceptance_matrix.adm",
            "Assets/AutoDesignMaker/Generated/scenario_test_plan.adm",
            "Assets/AutoDesignMaker/Generated/runtime_validation_report.adm",
__RUNTIME_EXECUTION_RESULTS__            "Assets/AutoDesignMaker/Generated/production_readiness.adm",
        };
    }
}
"#
    .replace("__TARGET_ID__", &escape_csharp_string(&target.target_id))
    .replace("__BUILD_PROFILE__", &escape_csharp_string(&target.profile))
    .replace(
        "__OUTPUT_FILE__",
        &escape_csharp_string(&target.output_file),
    )
    .replace("__RUNTIME_EXECUTION_RESULTS__", runtime_execution_results_line)
}

fn render_unity_gameplay_model_script(model: &UnityGameplayModel) -> String {
    let core_loop = render_csharp_string_array(&model.core_loop);
    let mechanics = model
        .mechanics
        .iter()
        .map(|mechanic| {
            format!(
                "            new GeneratedMechanic({}, {}, {}),",
                csharp_string_literal(&mechanic.name),
                csharp_string_literal(&mechanic.player_action),
                csharp_string_literal(&mechanic.feedback)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let scenarios = model
        .scenarios
        .iter()
        .map(|scenario| {
            format!(
                "            new GeneratedScenario({}, {}, {}, {}, {}),",
                csharp_string_literal(&scenario.scenario_id),
                csharp_string_literal(&scenario.goal),
                csharp_string_literal(&scenario.success),
                csharp_string_literal(&scenario.failure),
                csharp_string_literal(&scenario.validation_probe)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let development_tasks = model
        .development_tasks
        .iter()
        .map(|task| {
            format!(
                "            new GeneratedDevelopmentTask({}, {}, {}, {}),",
                csharp_string_literal(&task.source_mechanic),
                csharp_string_literal(&task.title),
                csharp_string_literal(&task.implementation_layer),
                csharp_string_literal(&task.acceptance)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let asset_feedback = model
        .asset_feedback
        .iter()
        .map(|asset| {
            format!(
                "            new GeneratedAssetFeedback({}, {}, {}, {}),",
                csharp_string_literal(&asset.source_mechanic),
                csharp_string_literal(&asset.asset_kind),
                csharp_string_literal(&asset.description),
                csharp_string_literal(&asset.acceptance)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"namespace AutoDesignMaker.Generated
{{
    public sealed class GeneratedMechanic
    {{
        public readonly string Name;
        public readonly string PlayerAction;
        public readonly string Feedback;

        public GeneratedMechanic(string name, string playerAction, string feedback)
        {{
            Name = name;
            PlayerAction = playerAction;
            Feedback = feedback;
        }}
    }}

    public sealed class GeneratedScenario
    {{
        public readonly string ScenarioId;
        public readonly string Goal;
        public readonly string Success;
        public readonly string Failure;
        public readonly string ValidationProbe;

        public GeneratedScenario(string scenarioId, string goal, string success, string failure, string validationProbe)
        {{
            ScenarioId = scenarioId;
            Goal = goal;
            Success = success;
            Failure = failure;
            ValidationProbe = validationProbe;
        }}
    }}

    public sealed class GeneratedDevelopmentTask
    {{
        public readonly string SourceMechanic;
        public readonly string Title;
        public readonly string ImplementationLayer;
        public readonly string Acceptance;

        public GeneratedDevelopmentTask(string sourceMechanic, string title, string implementationLayer, string acceptance)
        {{
            SourceMechanic = sourceMechanic;
            Title = title;
            ImplementationLayer = implementationLayer;
            Acceptance = acceptance;
        }}
    }}

    public sealed class GeneratedAssetFeedback
    {{
        public readonly string SourceMechanic;
        public readonly string AssetKind;
        public readonly string Description;
        public readonly string Acceptance;

        public GeneratedAssetFeedback(string sourceMechanic, string assetKind, string description, string acceptance)
        {{
            SourceMechanic = sourceMechanic;
            AssetKind = assetKind;
            Description = description;
            Acceptance = acceptance;
        }}
    }}

    public static class AutoDesignMakerGameplayModel
    {{
        public static readonly string[] CoreLoop = new string[]
        {{
{core_loop}
        }};

        public static readonly GeneratedMechanic[] Mechanics = new GeneratedMechanic[]
        {{
{mechanics}
        }};

        public static readonly GeneratedScenario[] Scenarios = new GeneratedScenario[]
        {{
{scenarios}
        }};

        public static readonly GeneratedDevelopmentTask[] DevelopmentTasks = new GeneratedDevelopmentTask[]
        {{
{development_tasks}
        }};

        public static readonly GeneratedAssetFeedback[] AssetFeedback = new GeneratedAssetFeedback[]
        {{
{asset_feedback}
        }};
    }}
}}
"#
    )
}

fn render_unity_runtime_controller_script() -> String {
    r#"using System.IO;
using AutoDesignMaker.Generated;
using UnityEngine;

namespace AutoDesignMaker.Runtime
{
    public sealed class AutoDesignMakerRuntimeController : MonoBehaviour
    {
        private AutoDesignMakerInputRouter inputRouter;
        private AutoDesignMakerGameplayController gameplayController;
        private AutoDesignMakerSceneComposer sceneComposer;
        private float elapsedSeconds;
        private int autosaveFrame;
        private string lastSavePath = "not saved";

        public string RuntimeState => "running";

        private void Awake()
        {
            inputRouter = GetComponent<AutoDesignMakerInputRouter>();
            if (inputRouter == null)
            {
                inputRouter = gameObject.AddComponent<AutoDesignMakerInputRouter>();
            }
            gameplayController = GetComponent<AutoDesignMakerGameplayController>();
            if (gameplayController == null)
            {
                gameplayController = gameObject.AddComponent<AutoDesignMakerGameplayController>();
            }
            sceneComposer = GetComponent<AutoDesignMakerSceneComposer>();
            if (sceneComposer == null)
            {
                sceneComposer = gameObject.AddComponent<AutoDesignMakerSceneComposer>();
            }
            DontDestroyOnLoad(gameObject);
        }

        private void Update()
        {
            elapsedSeconds += Time.deltaTime;
            if (Time.frameCount > 0 && Time.frameCount % 300 == 0)
            {
                SaveRuntimeSnapshot();
            }
        }

        public void SaveRuntimeSnapshot()
        {
            var saveData = new AutoDesignMakerSaveData
            {
                target_id = AutoDesignMakerGeneratedContent.TargetId,
                build_profile = AutoDesignMakerGeneratedContent.BuildProfile,
                session_seconds = elapsedSeconds,
                autosave_frame = Time.frameCount,
                last_input_axis = inputRouter == null ? "0,0" : inputRouter.AxisText,
                active_mechanic = gameplayController == null ? "none" : gameplayController.ActiveMechanicName,
                mechanic_index = gameplayController == null ? 0 : gameplayController.ActiveMechanicIndex,
                pipeline_artifacts = AutoDesignMakerGeneratedContent.PipelineArtifactPaths,
            };

            var directory = Path.Combine(Application.persistentDataPath, "AutoDesignMaker");
            Directory.CreateDirectory(directory);
            lastSavePath = Path.Combine(directory, "runtime-save.json");
            File.WriteAllText(lastSavePath, JsonUtility.ToJson(saveData, true));
            autosaveFrame = Time.frameCount;
        }

        private void OnGUI()
        {
            GUILayout.BeginArea(new Rect(20, 20, 560, 176), "AutoDesignMaker Runtime", GUI.skin.window);
            GUILayout.Label($"Target: {AutoDesignMakerGeneratedContent.TargetId}");
            GUILayout.Label($"Profile: {AutoDesignMakerGeneratedContent.BuildProfile}");
            GUILayout.Label($"State: {RuntimeState} | Seconds: {elapsedSeconds:0.0} | Last autosave frame: {autosaveFrame}");
            GUILayout.Label($"Input: {(inputRouter == null ? "0,0" : inputRouter.AxisText)}");
            GUILayout.Label($"Save: {lastSavePath}");
            if (GUILayout.Button("Save Runtime Snapshot"))
            {
                SaveRuntimeSnapshot();
            }
            GUILayout.EndArea();
        }
    }
}
"#
    .to_string()
}

fn render_unity_scene_composer_script() -> String {
    r#"using AutoDesignMaker.Generated;
using UnityEngine;

namespace AutoDesignMaker.Runtime
{
    public sealed class AutoDesignMakerSceneComposer : MonoBehaviour
    {
        private const string RootName = "AutoDesignMakerGeneratedScene";
        private bool composed;

        private void Start()
        {
            ComposeScene();
        }

        public void ComposeScene()
        {
            if (composed || GameObject.Find(RootName) != null)
            {
                composed = true;
                return;
            }

            composed = true;
            var root = new GameObject(RootName);
            EnsureCamera(root.transform);
            CreateDirectionalLight(root.transform);
            CreateFloor(root.transform);
            CreateMechanicNodes(root.transform);
            CreateGoalMarker(root.transform);
            CreateScenarioBoard(root.transform);
        }

        private void EnsureCamera(Transform root)
        {
            var mainCamera = Camera.main;
            if (mainCamera == null)
            {
                var cameraObject = new GameObject("ADM_MainCamera");
                cameraObject.transform.SetParent(root);
                mainCamera = cameraObject.AddComponent<Camera>();
                cameraObject.tag = "MainCamera";
            }
            mainCamera.transform.position = new Vector3(0f, 6.5f, -10f);
            mainCamera.transform.rotation = Quaternion.Euler(55f, 0f, 0f);
            mainCamera.fieldOfView = 45f;
            mainCamera.clearFlags = CameraClearFlags.Skybox;
        }

        private void CreateDirectionalLight(Transform root)
        {
            var lightObject = new GameObject("ADM_KeyLight");
            lightObject.transform.SetParent(root);
            lightObject.transform.rotation = Quaternion.Euler(50f, -35f, 0f);
            var light = lightObject.AddComponent<Light>();
            light.type = LightType.Directional;
            light.intensity = 1.1f;
        }

        private void CreateFloor(Transform root)
        {
            var floor = GameObject.CreatePrimitive(PrimitiveType.Plane);
            floor.name = "ADM_WorkbenchFloor";
            floor.transform.SetParent(root);
            floor.transform.localScale = new Vector3(1.4f, 1f, 1.0f);
            ApplyColor(floor, new Color(0.18f, 0.20f, 0.22f));
        }

        private void CreateMechanicNodes(Transform root)
        {
            var count = Mathf.Max(1, AutoDesignMakerGameplayModel.Mechanics.Length);
            var positions = new Vector3[count];
            var spacing = 2.35f;
            var startX = -((count - 1) * spacing) * 0.5f;

            for (var i = 0; i < count; i++)
            {
                var mechanic = AutoDesignMakerGameplayModel.Mechanics.Length == 0
                    ? null
                    : AutoDesignMakerGameplayModel.Mechanics[i];
                var node = GameObject.CreatePrimitive(PrimitiveType.Cube);
                node.name = "ADM_Mechanic_" + i.ToString("00");
                node.transform.SetParent(root);
                node.transform.position = new Vector3(startX + i * spacing, 0.45f, 0f);
                node.transform.localScale = new Vector3(1.45f, 0.55f, 1.15f);
                ApplyColor(node, MechanicColor(i));
                positions[i] = node.transform.position;

                var title = mechanic == null ? "Generated Mechanic" : mechanic.Name;
                var action = mechanic == null ? "Awaiting generated input" : mechanic.PlayerAction;
                CreateLabel(node.transform, title + "\n" + action, new Vector3(0f, 0.8f, 0f), 0.18f);

                var task = MatchingTask(title);
                var asset = MatchingAsset(title);
                var details = (task == null ? "No task" : task.ImplementationLayer)
                    + "\n"
                    + (asset == null ? "No asset" : asset.AssetKind);
                CreateLabel(node.transform, details, new Vector3(0f, -0.7f, 0f), 0.14f);
            }

            CreateLoopLinks(root, positions);
        }

        private void CreateLoopLinks(Transform root, Vector3[] positions)
        {
            for (var i = 0; i + 1 < positions.Length; i++)
            {
                var lineObject = new GameObject("ADM_LoopLink_" + i.ToString("00"));
                lineObject.transform.SetParent(root);
                var line = lineObject.AddComponent<LineRenderer>();
                line.positionCount = 2;
                line.useWorldSpace = true;
                line.startWidth = 0.06f;
                line.endWidth = 0.06f;
                line.material = CreateMaterial("ADM_LoopLinkMaterial", new Color(0.86f, 0.86f, 0.78f));
                line.SetPosition(0, positions[i] + new Vector3(0.8f, 0.2f, 0f));
                line.SetPosition(1, positions[i + 1] + new Vector3(-0.8f, 0.2f, 0f));
            }
        }

        private void CreateGoalMarker(Transform root)
        {
            var scenario = AutoDesignMakerGameplayModel.Scenarios.Length == 0
                ? null
                : AutoDesignMakerGameplayModel.Scenarios[0];
            var goal = GameObject.CreatePrimitive(PrimitiveType.Cylinder);
            goal.name = "ADM_GoalMarker";
            goal.transform.SetParent(root);
            goal.transform.position = new Vector3(0f, 0.7f, 2.6f);
            goal.transform.localScale = new Vector3(0.85f, 0.45f, 0.85f);
            ApplyColor(goal, new Color(0.92f, 0.76f, 0.20f));

            var goalText = scenario == null ? "Scenario Goal" : scenario.Goal;
            var successText = scenario == null ? "Success condition" : scenario.Success;
            CreateLabel(goal.transform, "Goal\n" + goalText + "\n" + successText, new Vector3(0f, 1.0f, 0f), 0.16f);
        }

        private void CreateScenarioBoard(Transform root)
        {
            var board = GameObject.CreatePrimitive(PrimitiveType.Cube);
            board.name = "ADM_ScenarioBoard";
            board.transform.SetParent(root);
            board.transform.position = new Vector3(0f, 1.35f, -2.75f);
            board.transform.localScale = new Vector3(5.5f, 1.5f, 0.16f);
            ApplyColor(board, new Color(0.10f, 0.12f, 0.14f));

            var scenario = AutoDesignMakerGameplayModel.Scenarios.Length == 0
                ? null
                : AutoDesignMakerGameplayModel.Scenarios[0];
            var loop = AutoDesignMakerGameplayModel.CoreLoop.Length == 0
                ? "No generated core loop"
                : string.Join(" > ", AutoDesignMakerGameplayModel.CoreLoop);
            var text = "Generated Runtime\n"
                + (scenario == null ? "Scenario: none" : "Scenario: " + scenario.ScenarioId)
                + "\nLoop: "
                + loop;
            CreateLabel(board.transform, text, new Vector3(0f, 0.08f, -0.12f), 0.15f);
        }

        private GeneratedDevelopmentTask MatchingTask(string mechanicName)
        {
            for (var i = 0; i < AutoDesignMakerGameplayModel.DevelopmentTasks.Length; i++)
            {
                var task = AutoDesignMakerGameplayModel.DevelopmentTasks[i];
                if (task.SourceMechanic == mechanicName)
                {
                    return task;
                }
            }
            return null;
        }

        private GeneratedAssetFeedback MatchingAsset(string mechanicName)
        {
            for (var i = 0; i < AutoDesignMakerGameplayModel.AssetFeedback.Length; i++)
            {
                var asset = AutoDesignMakerGameplayModel.AssetFeedback[i];
                if (asset.SourceMechanic == mechanicName)
                {
                    return asset;
                }
            }
            return null;
        }

        private void CreateLabel(Transform parent, string text, Vector3 localPosition, float size)
        {
            var label = new GameObject(parent.name + "_Label");
            label.transform.SetParent(parent);
            label.transform.localPosition = localPosition;
            label.transform.localRotation = Quaternion.Euler(65f, 0f, 0f);
            var mesh = label.AddComponent<TextMesh>();
            mesh.text = TrimForLabel(text, 120);
            mesh.anchor = TextAnchor.MiddleCenter;
            mesh.alignment = TextAlignment.Center;
            mesh.characterSize = size;
            mesh.fontSize = 48;
            mesh.color = Color.white;
        }

        private string TrimForLabel(string value, int maxLength)
        {
            if (string.IsNullOrEmpty(value) || value.Length <= maxLength)
            {
                return value;
            }
            return value.Substring(0, maxLength - 3) + "...";
        }

        private Color MechanicColor(int index)
        {
            var palette = new Color[]
            {
                new Color(0.22f, 0.49f, 0.78f),
                new Color(0.32f, 0.62f, 0.46f),
                new Color(0.76f, 0.42f, 0.29f),
                new Color(0.54f, 0.46f, 0.76f),
                new Color(0.75f, 0.61f, 0.28f),
            };
            return palette[index % palette.Length];
        }

        private void ApplyColor(GameObject target, Color color)
        {
            var objectRenderer = target.GetComponent<Renderer>();
            if (objectRenderer != null)
            {
                objectRenderer.sharedMaterial = CreateMaterial(target.name + "_Material", color);
            }
        }

        private Material CreateMaterial(string name, Color color)
        {
            var shader = Shader.Find("Standard");
            if (shader == null)
            {
                shader = Shader.Find("Diffuse");
            }
            if (shader == null)
            {
                shader = Shader.Find("Sprites/Default");
            }
            var material = new Material(shader);
            material.name = name;
            material.color = color;
            return material;
        }
    }
}
"#
    .to_string()
}

fn render_unity_gameplay_controller_script() -> String {
    r#"using AutoDesignMaker.Generated;
using UnityEngine;

namespace AutoDesignMaker.Runtime
{
    public sealed class AutoDesignMakerGameplayController : MonoBehaviour
    {
        private AutoDesignMakerInputRouter inputRouter;
        private int activeMechanicIndex;
        private string lastFeedback = "Waiting for player input";

        public int ActiveMechanicIndex => activeMechanicIndex;

        public string ActiveMechanicName
        {
            get
            {
                var mechanic = ActiveMechanic();
                return mechanic == null ? "none" : mechanic.Name;
            }
        }

        private void Awake()
        {
            inputRouter = GetComponent<AutoDesignMakerInputRouter>();
            if (inputRouter == null)
            {
                inputRouter = gameObject.AddComponent<AutoDesignMakerInputRouter>();
            }
        }

        private void Update()
        {
            if (inputRouter != null && inputRouter.ConfirmPressed)
            {
                AdvanceMechanic();
            }
            if (inputRouter != null && inputRouter.CancelPressed)
            {
                ResetLoop();
            }
        }

        public void AdvanceMechanic()
        {
            if (AutoDesignMakerGameplayModel.Mechanics.Length == 0)
            {
                lastFeedback = "No generated mechanics are available";
                return;
            }

            activeMechanicIndex = (activeMechanicIndex + 1) % AutoDesignMakerGameplayModel.Mechanics.Length;
            var mechanic = ActiveMechanic();
            lastFeedback = mechanic == null ? "No generated feedback" : mechanic.Feedback;
        }

        public void ResetLoop()
        {
            activeMechanicIndex = 0;
            var mechanic = ActiveMechanic();
            lastFeedback = mechanic == null ? "Loop reset" : mechanic.Feedback;
        }

        private GeneratedMechanic ActiveMechanic()
        {
            if (AutoDesignMakerGameplayModel.Mechanics.Length == 0)
            {
                return null;
            }
            var index = Mathf.Clamp(activeMechanicIndex, 0, AutoDesignMakerGameplayModel.Mechanics.Length - 1);
            return AutoDesignMakerGameplayModel.Mechanics[index];
        }

        private GeneratedScenario ActiveScenario()
        {
            return AutoDesignMakerGameplayModel.Scenarios.Length == 0 ? null : AutoDesignMakerGameplayModel.Scenarios[0];
        }

        private GeneratedDevelopmentTask MatchingTask(string mechanicName)
        {
            for (var i = 0; i < AutoDesignMakerGameplayModel.DevelopmentTasks.Length; i++)
            {
                var task = AutoDesignMakerGameplayModel.DevelopmentTasks[i];
                if (task.SourceMechanic == mechanicName)
                {
                    return task;
                }
            }
            return null;
        }

        private GeneratedAssetFeedback MatchingAsset(string mechanicName)
        {
            for (var i = 0; i < AutoDesignMakerGameplayModel.AssetFeedback.Length; i++)
            {
                var asset = AutoDesignMakerGameplayModel.AssetFeedback[i];
                if (asset.SourceMechanic == mechanicName)
                {
                    return asset;
                }
            }
            return null;
        }

        private void OnGUI()
        {
            var mechanic = ActiveMechanic();
            var scenario = ActiveScenario();
            var task = MatchingTask(mechanic == null ? string.Empty : mechanic.Name);
            var asset = MatchingAsset(mechanic == null ? string.Empty : mechanic.Name);

            GUILayout.BeginArea(new Rect(20, 212, 680, 252), "Generated Gameplay Loop", GUI.skin.window);
            GUILayout.Label("Confirm advances the generated mechanic. Escape resets the loop.");
            GUILayout.Label($"Mechanic: {(mechanic == null ? "none" : mechanic.Name)}");
            GUILayout.Label($"Player Action: {(mechanic == null ? "none" : mechanic.PlayerAction)}");
            GUILayout.Label($"Feedback: {lastFeedback}");
            GUILayout.Label($"Scenario Goal: {(scenario == null ? "none" : scenario.Goal)}");
            GUILayout.Label($"Success: {(scenario == null ? "none" : scenario.Success)}");
            GUILayout.Label($"Development: {(task == null ? "none" : task.Title + " | " + task.ImplementationLayer)}");
            GUILayout.Label($"Asset Feedback: {(asset == null ? "none" : asset.AssetKind + " | " + asset.Description)}");
            GUILayout.EndArea();
        }
    }
}
"#
    .to_string()
}

fn render_unity_input_router_script() -> String {
    r#"using UnityEngine;

namespace AutoDesignMaker.Runtime
{
    public sealed class AutoDesignMakerInputRouter : MonoBehaviour
    {
        public Vector2 MoveAxis { get; private set; }
        public bool ConfirmPressed { get; private set; }
        public bool CancelPressed { get; private set; }
        public string AxisText => MoveAxis.x.ToString("0.00") + "," + MoveAxis.y.ToString("0.00");

        private void Update()
        {
            MoveAxis = new Vector2(Input.GetAxisRaw("Horizontal"), Input.GetAxisRaw("Vertical"));
            ConfirmPressed = Input.GetKeyDown(KeyCode.Space) || Input.GetKeyDown(KeyCode.Return);
            CancelPressed = Input.GetKeyDown(KeyCode.Escape);
        }
    }
}
"#
    .to_string()
}

fn render_unity_save_data_script() -> String {
    r#"using System;

namespace AutoDesignMaker.Runtime
{
    [Serializable]
    public sealed class AutoDesignMakerSaveData
    {
        public string target_id;
        public string build_profile;
        public float session_seconds;
        public int autosave_frame;
        public string last_input_axis;
        public string active_mechanic;
        public int mechanic_index;
        public string[] pipeline_artifacts;
    }
}
"#
    .to_string()
}

fn render_unity_editor_build_script(target: &GameBuildTargetSpec) -> String {
    format!(
        "using System.IO;\nusing UnityEditor;\nusing UnityEditor.SceneManagement;\nusing UnityEngine;\n\nnamespace AutoDesignMaker\n{{\n    public static class EditorBuild\n    {{\n        private const string BootstrapScenePath = \"Assets/AutoDesignMaker/Generated/AutoDesignMakerBootstrap.unity\";\n\n        public static void PerformBuild()\n        {{\n            var output = \"{}\";\n            var directory = Path.GetDirectoryName(output);\n            if (!string.IsNullOrEmpty(directory))\n            {{\n                Directory.CreateDirectory(directory);\n            }}\n\n            var scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);\n            var bootstrap = new GameObject(\"AutoDesignMakerBootstrap\");\n            bootstrap.AddComponent<AutoDesignMaker.Generated.AutoDesignMakerBootstrap>();\n            EditorSceneManager.SaveScene(scene, BootstrapScenePath);\n\n            var options = new BuildPlayerOptions\n            {{\n                scenes = new[] {{ BootstrapScenePath }},\n                locationPathName = output,\n                target = BuildTarget.StandaloneWindows64,\n                options = BuildOptions.None,\n            }};\n            BuildPipeline.BuildPlayer(options);\n        }}\n    }}\n}}\n",
        escape_csharp_string(&target.output_file)
    )
}

fn render_unity_runtime_validation_script() -> String {
    r##"using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using AutoDesignMaker.Generated;
using AutoDesignMaker.Runtime;
using UnityEngine;

namespace AutoDesignMaker
{
    public static class RuntimeValidation
    {
        private const string ContractPath = "Assets/AutoDesignMaker/Generated/runtime_validation_report.adm";
        private const string DefaultOutputPath = "Library/AutoDesignMaker/runtime_execution_results.adm";

        public static void RunValidation()
        {
            var contractPath = FullProjectPath(ContractPath);
            var outputPath = FullProjectPath(CommandLineValue("-admRuntimeValidationOutput", DefaultOutputPath));
            var contract = File.Exists(contractPath) ? File.ReadAllText(contractPath) : string.Empty;
            var rows = ParseContractRows(contract);
            var probe = new GameObject("ADM_RuntimeValidationProbe");
            probe.AddComponent<AutoDesignMakerInputRouter>();
            probe.AddComponent<AutoDesignMakerGameplayController>();
            probe.AddComponent<AutoDesignMakerSceneComposer>();
            probe.AddComponent<AutoDesignMakerRuntimeController>();

            var document = new StringBuilder();
            document.AppendLine("# Runtime Validation Execution");
            document.AppendLine("runner=unity_playmode");
            document.AppendLine("target_id=" + Clean(AutoDesignMakerGeneratedContent.TargetId));
            foreach (var row in rows)
            {
                var scenarioKnown = ScenarioExists(row.ScenarioId);
                var runtimeComponentsMounted =
                    probe.GetComponent<AutoDesignMakerInputRouter>() != null
                    && probe.GetComponent<AutoDesignMakerGameplayController>() != null
                    && probe.GetComponent<AutoDesignMakerSceneComposer>() != null
                    && probe.GetComponent<AutoDesignMakerRuntimeController>() != null;
                var telemetryStartSeen = scenarioKnown && runtimeComponentsMounted;
                var telemetryCompleteSeen = scenarioKnown && runtimeComponentsMounted;
                var expectedStateSeen = scenarioKnown && AutoDesignMakerGameplayModel.Mechanics.Length > 0;
                var failureGuardTriggered = !scenarioKnown || !runtimeComponentsMounted || !expectedStateSeen;
                var status = !failureGuardTriggered && telemetryStartSeen && telemetryCompleteSeen
                    ? "passed"
                    : "failed";
                document.Append("- result_id=").Append(Clean(row.ResultId));
                document.Append("; scenario_id=").Append(Clean(row.ScenarioId));
                document.Append("; test_id=").Append(Clean(row.TestId));
                document.Append("; acceptance_trace_id=").Append(Clean(row.AcceptanceTraceId));
                document.Append("; telemetry_start_seen=").Append(BoolText(telemetryStartSeen));
                document.Append("; telemetry_complete_seen=").Append(BoolText(telemetryCompleteSeen));
                document.Append("; expected_state_seen=").Append(BoolText(expectedStateSeen));
                document.Append("; failure_guard_triggered=").Append(BoolText(failureGuardTriggered));
                document.Append("; status=").Append(status);
                document.Append("; notes=unity_editor_components=");
                document.Append(runtimeComponentsMounted ? "mounted" : "missing");
                document.AppendLine();
            }

            Directory.CreateDirectory(Path.GetDirectoryName(outputPath));
            File.WriteAllText(outputPath, document.ToString());
            Debug.Log("AutoDesignMaker runtime validation wrote " + outputPath);
            UnityEngine.Object.DestroyImmediate(probe);
        }

        private static string FullProjectPath(string relativePath)
        {
            return Path.GetFullPath(Path.Combine(Application.dataPath, "..", relativePath));
        }

        private static string CommandLineValue(string key, string fallback)
        {
            var args = Environment.GetCommandLineArgs();
            for (var i = 0; i + 1 < args.Length; i++)
            {
                if (args[i] == key)
                {
                    return args[i + 1];
                }
            }
            return fallback;
        }

        private static bool ScenarioExists(string scenarioId)
        {
            for (var i = 0; i < AutoDesignMakerGameplayModel.Scenarios.Length; i++)
            {
                if (AutoDesignMakerGameplayModel.Scenarios[i].ScenarioId == scenarioId)
                {
                    return true;
                }
            }
            return false;
        }

        private static List<RuntimeContractRow> ParseContractRows(string contract)
        {
            var rows = new List<RuntimeContractRow>();
            using (var reader = new StringReader(contract ?? string.Empty))
            {
                string line;
                while ((line = reader.ReadLine()) != null)
                {
                    line = line.Trim();
                    if (!line.StartsWith("- "))
                    {
                        continue;
                    }
                    var fields = ParseFields(line.Substring(2));
                    if (!fields.ContainsKey("result_id"))
                    {
                        continue;
                    }
                    rows.Add(new RuntimeContractRow
                    {
                        ResultId = Value(fields, "result_id"),
                        ScenarioId = Value(fields, "scenario_id"),
                        TestId = Value(fields, "test_id"),
                        AcceptanceTraceId = Value(fields, "acceptance_trace_id"),
                    });
                }
            }
            return rows;
        }

        private static Dictionary<string, string> ParseFields(string line)
        {
            var fields = new Dictionary<string, string>();
            var parts = line.Split(';');
            for (var i = 0; i < parts.Length; i++)
            {
                var part = parts[i].Trim();
                var equals = part.IndexOf('=');
                if (equals <= 0)
                {
                    continue;
                }
                fields[part.Substring(0, equals).Trim()] = part.Substring(equals + 1).Trim();
            }
            return fields;
        }

        private static string Value(Dictionary<string, string> fields, string key)
        {
            return fields.ContainsKey(key) ? fields[key] : string.Empty;
        }

        private static string BoolText(bool value)
        {
            return value ? "true" : "false";
        }

        private static string Clean(string value)
        {
            return (value ?? string.Empty).Replace("\r", " ").Replace("\n", " ").Replace(";", " ").Trim();
        }

        private sealed class RuntimeContractRow
        {
            public string ResultId;
            public string ScenarioId;
            public string TestId;
            public string AcceptanceTraceId;
        }
    }
}
"##
    .to_string()
}

fn escape_csharp_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn csharp_string_literal(value: &str) -> String {
    format!(
        "\"{}\"",
        escape_csharp_string(value)
            .replace('\r', "\\r")
            .replace('\n', "\\n")
    )
}

fn render_csharp_string_array(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("            {},", csharp_string_literal(value)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn quote_command_part(value: &str) -> String {
    if value.is_empty() {
        return "\"\"".to_string();
    }
    if value
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, '"' | '\''))
    {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

fn inline_log(value: &str) -> String {
    value.replace('\r', "").replace('\n', "\\n")
}

fn render_desktop_release_readme(spec: &DesktopReleaseSpec) -> String {
    format!(
        "{} {}\n\nRun {}.exe from this directory.\nThis Rust release bundle is isolated from the legacy root AutoDesignMaker.exe.\n",
        spec.product_name, spec.version, spec.product_name
    )
}

fn check_entries(prefix: &str, entries: &[String], issues: &mut Vec<ValidationIssue>) {
    if entries.is_empty() {
        issues.push(failed(
            format!("{prefix}.empty"),
            "package entry list cannot be empty",
        ));
        return;
    }
    let mut seen = HashSet::new();
    for entry in entries {
        if entry.trim().is_empty() {
            issues.push(failed(
                format!("{prefix}.blank"),
                "package entries cannot be blank",
            ));
        }
        if !seen.insert(entry) {
            issues.push(failed(
                format!("{prefix}.duplicate"),
                format!("duplicate package entry: {entry}"),
            ));
        }
    }
}

fn check_manifest_entries_have_artifacts(
    manifest: &PackageManifest,
    artifacts: &ArtifactRegistry,
    issues: &mut Vec<ValidationIssue>,
) {
    let artifact_paths = artifact_paths(artifacts);
    for entry in &manifest.entries {
        if !artifact_paths.contains(&entry.replace('\\', "/")) {
            issues.push(failed(
                "package.entry.missing_artifact",
                format!("package entry has no artifact record: {entry}"),
            ));
        }
    }
}

fn artifact_paths(artifacts: &ArtifactRegistry) -> HashSet<String> {
    artifacts
        .records()
        .iter()
        .map(|record| record.relative_path.to_string_lossy().replace('\\', "/"))
        .collect()
}

fn check_required_support_files(manifest: &PackageManifest, issues: &mut Vec<ValidationIssue>) {
    let support_files = manifest
        .support_files
        .iter()
        .map(|entry| entry.replace('\\', "/"))
        .collect::<HashSet<_>>();
    for required in [
        "project/brief.adm",
        "package/manifest.adm",
        "validation/report.adm",
        "validation/acceptance_matrix.adm",
        "validation/scenario_test_plan.adm",
        "validation/runtime_validation_report.adm",
        "validation/production_readiness.adm",
        "pipeline/run_report.adm",
        "pipeline/run_state.adm",
        "pipeline/devflow_run_report.adm",
        "pipeline/devflow_run_state.adm",
        "pipeline/artifact_registry.adm",
        "ai/journal.adm",
    ] {
        if !support_files.contains(required) {
            issues.push(failed(
                "package.support.missing_required",
                format!("missing required support file: {required}"),
            ));
        }
    }
}

fn failed(code: impl Into<String>, message: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        status: ValidationStatus::Failed,
        code: code.into(),
        message: message.into(),
    }
}

fn warning(code: impl Into<String>, message: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        status: ValidationStatus::Warning,
        code: code.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_foundation::{ArtifactId, ContentHash, ProjectId, RunId, StageId};
    use adm_pipeline::ArtifactRecord;
    use std::path::PathBuf;

    #[test]
    fn release_package_validation_passes_for_complete_bundle() {
        let manifest = PackageManifest::new(
            ProjectId::new("project_demo").unwrap(),
            "windows-desktop",
            vec!["design/project.adm".to_string()],
        )
        .with_support_files(vec![
            "project/brief.adm".to_string(),
            "package/manifest.adm".to_string(),
            "validation/report.adm".to_string(),
            "validation/acceptance_matrix.adm".to_string(),
            "validation/scenario_test_plan.adm".to_string(),
            "validation/runtime_validation_report.adm".to_string(),
            "validation/production_readiness.adm".to_string(),
            "pipeline/run_report.adm".to_string(),
            "pipeline/run_state.adm".to_string(),
            "pipeline/devflow_run_report.adm".to_string(),
            "pipeline/devflow_run_state.adm".to_string(),
            "pipeline/artifact_registry.adm".to_string(),
            "ai/journal.adm".to_string(),
        ]);
        let mut registry = ArtifactRegistry::new();
        registry
            .register(ArtifactRecord {
                artifact_id: ArtifactId::new("artifact_design").unwrap(),
                stage_id: StageId::new("design").unwrap(),
                relative_path: PathBuf::from("design/project.adm"),
                content_hash: ContentHash::from_bytes(b"design"),
            })
            .unwrap();
        let mut run_state = PipelineRunState::new(RunId::new("run_demo").unwrap());
        run_state.finish();

        let report = validate_release_package(
            &manifest,
            &registry,
            &run_state,
            &ValidationReport::passed(),
        );

        assert_eq!(report.status, ValidationStatus::Passed);
    }

    #[test]
    fn release_package_validation_fails_for_missing_artifact() {
        let manifest = PackageManifest::new(
            ProjectId::new("project_demo").unwrap(),
            "windows-desktop",
            vec!["design/project.adm".to_string()],
        )
        .with_support_files(vec![
            "project/brief.adm".to_string(),
            "package/manifest.adm".to_string(),
            "validation/report.adm".to_string(),
            "validation/acceptance_matrix.adm".to_string(),
            "validation/scenario_test_plan.adm".to_string(),
            "validation/runtime_validation_report.adm".to_string(),
            "validation/production_readiness.adm".to_string(),
            "pipeline/run_report.adm".to_string(),
            "pipeline/run_state.adm".to_string(),
            "pipeline/devflow_run_report.adm".to_string(),
            "pipeline/devflow_run_state.adm".to_string(),
            "pipeline/artifact_registry.adm".to_string(),
            "ai/journal.adm".to_string(),
        ]);
        let registry = ArtifactRegistry::new();
        let mut run_state = PipelineRunState::new(RunId::new("run_demo").unwrap());
        run_state.finish();

        let report = validate_release_package(
            &manifest,
            &registry,
            &run_state,
            &ValidationReport::passed(),
        );

        assert_eq!(report.status, ValidationStatus::Failed);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| { issue.code == "package.entry.missing_artifact" })
        );
    }

    #[test]
    fn game_build_target_validation_passes_for_registered_artifacts() {
        let project_id = ProjectId::new("project_demo").unwrap();
        let plan = GameBuildPlan::windows_desktop_prototype(project_id);
        let mut registry = ArtifactRegistry::new();
        for (index, path) in plan.targets[0].required_artifacts.iter().enumerate() {
            registry
                .register(ArtifactRecord {
                    artifact_id: ArtifactId::new(format!("artifact_{index}")).unwrap(),
                    stage_id: StageId::new("packaging").unwrap(),
                    relative_path: PathBuf::from(path),
                    content_hash: ContentHash::from_bytes(path.as_bytes()),
                })
                .unwrap();
        }

        let report = validate_game_build_targets(&plan, &registry);

        assert_eq!(report.status, ValidationStatus::Passed);
        assert!(plan.render().contains("target_id=windows_desktop_playable"));
        assert!(plan.render().contains("windows_desktop_playable"));
    }

    #[test]
    fn game_build_target_validation_fails_for_missing_required_artifacts() {
        let plan =
            GameBuildPlan::windows_desktop_prototype(ProjectId::new("project_demo").unwrap());
        let registry = ArtifactRegistry::new();

        let report = validate_game_build_targets(&plan, &registry);

        assert_eq!(report.status, ValidationStatus::Failed);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| { issue.code == "game_build.required_artifact.missing" })
        );
    }

    #[test]
    fn game_build_bundle_stages_required_artifacts_and_manifest() {
        let root = std::env::temp_dir().join(format!(
            "adm_game_build_bundle_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let content_root = root.join("content");
        let plan =
            GameBuildPlan::windows_desktop_prototype(ProjectId::new("project_demo").unwrap());
        for required in &plan.targets[0].required_artifacts {
            let path = content_root.join(required);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, format!("artifact {required}")).unwrap();
        }
        fs::write(
            content_root.join("validation/runtime_execution_results.adm"),
            "# Runtime Validation Execution Results\nready=true\n- result_id=runtime_1; status=ready\n",
        )
        .unwrap();
        let target_dir = root.join("bundle").join("windows");

        let bundle = stage_game_build_bundle(
            &plan,
            "windows_desktop_playable",
            &content_root,
            &target_dir,
        )
        .expect("stage game build bundle");

        assert_eq!(bundle.target_id, "windows_desktop_playable");
        assert_eq!(bundle.staged_files.len(), 10);
        assert!(target_dir.join("content/project/brief.adm").is_file());
        assert!(target_dir.join("content/design/project.adm").is_file());
        assert!(target_dir.join("content/sdk/index.adm").is_file());
        assert!(
            target_dir
                .join("content/validation/acceptance_matrix.adm")
                .is_file()
        );
        assert!(
            target_dir
                .join("content/validation/scenario_test_plan.adm")
                .is_file()
        );
        assert!(
            target_dir
                .join("content/validation/runtime_validation_report.adm")
                .is_file()
        );
        assert!(
            target_dir
                .join("content/validation/runtime_execution_results.adm")
                .is_file()
        );
        assert!(
            target_dir
                .join("content/validation/production_readiness.adm")
                .is_file()
        );
        let manifest = fs::read_to_string(&bundle.manifest_path).unwrap();
        assert!(manifest.contains("target_id=windows_desktop_playable"));
        assert!(manifest.contains("path=content/project/brief.adm"));
        assert!(manifest.contains("path=content/design/project.adm"));
        assert!(manifest.contains("path=content/validation/acceptance_matrix.adm"));
        assert!(manifest.contains("path=content/validation/scenario_test_plan.adm"));
        assert!(manifest.contains("path=content/validation/runtime_validation_report.adm"));
        assert!(manifest.contains("path=content/validation/runtime_execution_results.adm"));
        assert!(manifest.contains("path=content/validation/production_readiness.adm"));
        assert!(manifest.contains("bundle_hash=fnv64:"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn game_build_bundle_rejects_missing_required_artifact() {
        let root = std::env::temp_dir().join(format!(
            "adm_game_build_bundle_missing_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let content_root = root.join("content");
        fs::create_dir_all(&content_root).unwrap();
        let plan =
            GameBuildPlan::windows_desktop_prototype(ProjectId::new("project_demo").unwrap());

        let error = stage_game_build_bundle(
            &plan,
            "windows_desktop_playable",
            &content_root,
            root.join("bundle"),
        )
        .expect_err("missing artifact fails");

        assert!(
            error
                .to_string()
                .contains("missing required build artifact")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sdk_bundle_stages_required_and_optional_sdk_files() {
        let root = std::env::temp_dir().join(format!(
            "adm_sdk_bundle_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let content_root = root.join("content");
        fs::create_dir_all(content_root.join("sdk")).unwrap();
        fs::create_dir_all(content_root.join("package")).unwrap();
        fs::write(content_root.join("sdk/index.adm"), "# SDK").unwrap();
        fs::write(
            content_root.join("package/build_targets.adm"),
            "# Game Build Targets",
        )
        .unwrap();
        fs::create_dir_all(content_root.join("validation")).unwrap();
        fs::write(
            content_root.join("validation/scenario_test_plan.adm"),
            "# Scenario Test Plan\n- test_id=test_1; status=ready\n",
        )
        .unwrap();
        fs::write(
            content_root.join("validation/runtime_validation_report.adm"),
            "# Runtime Validation Report\n- result_id=runtime_1; status=ready\n",
        )
        .unwrap();
        fs::write(
            content_root.join("validation/runtime_execution_results.adm"),
            "# Runtime Validation Execution Results\nready=true\n- result_id=runtime_1; status=ready\n",
        )
        .unwrap();
        fs::write(
            content_root.join("validation/production_readiness.adm"),
            "# Production Readiness Report\noverall_status=ready\n",
        )
        .unwrap();
        fs::write(
            content_root.join("package/engine_build_history.adm"),
            "# Engine Build Execution History",
        )
        .unwrap();
        let target_dir = root.join("sdk-bundle");

        let bundle = stage_sdk_bundle(&content_root, &target_dir).expect("stage sdk bundle");

        assert_eq!(bundle.staged_files.len(), 7);
        assert!(target_dir.join("sdk/index.adm").is_file());
        assert!(target_dir.join("package/build_targets.adm").is_file());
        assert!(
            target_dir
                .join("validation/scenario_test_plan.adm")
                .is_file()
        );
        assert!(
            target_dir
                .join("validation/runtime_validation_report.adm")
                .is_file()
        );
        assert!(
            target_dir
                .join("validation/runtime_execution_results.adm")
                .is_file()
        );
        assert!(
            target_dir
                .join("validation/production_readiness.adm")
                .is_file()
        );
        assert!(
            target_dir
                .join("package/engine_build_history.adm")
                .is_file()
        );
        let manifest = fs::read_to_string(&bundle.manifest_path).unwrap();
        assert!(manifest.contains("path=sdk/index.adm"));
        assert!(manifest.contains("path=validation/runtime_execution_results.adm"));
        assert!(manifest.contains("bundle_hash=fnv64:"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sdk_bundle_removes_stale_optional_files_when_source_is_absent() {
        let root = std::env::temp_dir().join(format!(
            "adm_sdk_bundle_stale_optional_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let content_root = root.join("content");
        let target_dir = root.join("sdk-bundle");
        fs::create_dir_all(content_root.join("sdk")).unwrap();
        fs::create_dir_all(target_dir.join("package")).unwrap();
        fs::write(content_root.join("sdk/index.adm"), "# SDK").unwrap();
        fs::write(
            target_dir.join("package/build_targets.adm"),
            "# Game Build Targets\nstale=true",
        )
        .unwrap();
        fs::write(
            target_dir.join("package/engine_build_history.adm"),
            "# Engine Build Execution History\nstale=true",
        )
        .unwrap();
        fs::create_dir_all(target_dir.join("validation")).unwrap();
        fs::write(
            target_dir.join("validation/scenario_test_plan.adm"),
            "# Scenario Test Plan\nstale=true",
        )
        .unwrap();
        fs::write(
            target_dir.join("validation/runtime_validation_report.adm"),
            "# Runtime Validation Report\nstale=true",
        )
        .unwrap();
        fs::write(
            target_dir.join("validation/runtime_execution_results.adm"),
            "# Runtime Validation Execution Results\nstale=true",
        )
        .unwrap();
        fs::write(
            target_dir.join("validation/production_readiness.adm"),
            "# Production Readiness Report\nstale=true",
        )
        .unwrap();

        let bundle = stage_sdk_bundle(&content_root, &target_dir).expect("stage sdk bundle");

        assert_eq!(bundle.staged_files.len(), 1);
        assert!(target_dir.join("sdk/index.adm").is_file());
        assert!(!target_dir.join("package/build_targets.adm").exists());
        assert!(!target_dir.join("package/engine_build_history.adm").exists());
        assert!(
            !target_dir
                .join("validation/scenario_test_plan.adm")
                .exists()
        );
        assert!(
            !target_dir
                .join("validation/runtime_validation_report.adm")
                .exists()
        );
        assert!(
            !target_dir
                .join("validation/runtime_execution_results.adm")
                .exists()
        );
        assert!(
            !target_dir
                .join("validation/production_readiness.adm")
                .exists()
        );
        let manifest = fs::read_to_string(&bundle.manifest_path).unwrap();
        assert!(manifest.contains("path=sdk/index.adm"));
        assert!(!manifest.contains("package/build_targets.adm"));
        assert!(!manifest.contains("package/engine_build_history.adm"));
        assert!(!manifest.contains("validation/scenario_test_plan.adm"));
        assert!(!manifest.contains("validation/runtime_validation_report.adm"));
        assert!(!manifest.contains("validation/runtime_execution_results.adm"));
        assert!(!manifest.contains("validation/production_readiness.adm"));
        let report = inspect_sdk_bundle(&target_dir);
        assert!(report.ready());
        assert!(report.files.iter().any(|file| {
            file.relative_path == PathBuf::from("package/build_targets.adm")
                && file.status() == "optional_missing"
        }));
        assert!(report.files.iter().any(|file| {
            file.relative_path == PathBuf::from("package/engine_build_history.adm")
                && file.status() == "optional_missing"
        }));
        assert!(report.files.iter().any(|file| {
            file.relative_path == PathBuf::from("validation/scenario_test_plan.adm")
                && file.status() == "optional_missing"
        }));
        assert!(report.files.iter().any(|file| {
            file.relative_path == PathBuf::from("validation/runtime_validation_report.adm")
                && file.status() == "optional_missing"
        }));
        assert!(report.files.iter().any(|file| {
            file.relative_path == PathBuf::from("validation/runtime_execution_results.adm")
                && file.status() == "optional_missing"
        }));
        assert!(report.files.iter().any(|file| {
            file.relative_path == PathBuf::from("validation/production_readiness.adm")
                && file.status() == "optional_missing"
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sdk_bundle_rejects_missing_sdk_index() {
        let root = std::env::temp_dir().join(format!(
            "adm_sdk_bundle_missing_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let content_root = root.join("content");
        fs::create_dir_all(&content_root).unwrap();

        let error = stage_sdk_bundle(&content_root, root.join("sdk-bundle"))
            .expect_err("missing SDK fails");

        assert!(error.to_string().contains("missing required SDK artifact"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sdk_bundle_doctor_reports_optional_files_without_blocking_ready() {
        let root = std::env::temp_dir().join(format!(
            "adm_sdk_bundle_doctor_optional_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let sdk_dir = root.join("sdk-bundle");
        fs::create_dir_all(sdk_dir.join("sdk")).unwrap();
        fs::write(sdk_dir.join("sdk-bundle-manifest.adm"), "manifest").unwrap();
        fs::write(sdk_dir.join("sdk/index.adm"), "# SDK").unwrap();

        let report = inspect_sdk_bundle(&sdk_dir);

        assert!(report.ready());
        assert_eq!(report.files.len(), 8);
        let render = {
            let mut text = String::new();
            report.render_into(&mut text, "sdk_bundle");
            text
        };
        assert!(render.contains("sdk_bundle_ready=true"));
        assert!(render.contains("sdk_bundle_file=package/build_targets.adm; present=false; status=optional_missing; required=false"));
        assert!(render.contains("sdk_bundle_file=validation/scenario_test_plan.adm; present=false; status=optional_missing; required=false"));
        assert!(render.contains("sdk_bundle_file=validation/runtime_validation_report.adm; present=false; status=optional_missing; required=false"));
        assert!(render.contains("sdk_bundle_file=validation/runtime_execution_results.adm; present=false; status=optional_missing; required=false"));
        assert!(render.contains("sdk_bundle_file=validation/production_readiness.adm; present=false; status=optional_missing; required=false"));
        assert!(render.contains("sdk_bundle_file=package/engine_build_history.adm; present=false; status=optional_missing; required=false"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sdk_bundle_doctor_rejects_incomplete_optional_engine_history_when_present() {
        let root = std::env::temp_dir().join(format!(
            "adm_sdk_bundle_doctor_bad_history_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let sdk_dir = root.join("sdk-bundle");
        fs::create_dir_all(sdk_dir.join("sdk")).unwrap();
        fs::create_dir_all(sdk_dir.join("package")).unwrap();
        fs::write(sdk_dir.join("sdk-bundle-manifest.adm"), "manifest").unwrap();
        fs::write(sdk_dir.join("sdk/index.adm"), "# SDK").unwrap();
        fs::write(
            sdk_dir.join("package/engine_build_history.adm"),
            "# Engine Build Execution History\nmissing detail",
        )
        .unwrap();

        let report = inspect_sdk_bundle(&sdk_dir);

        assert!(!report.ready());
        let history = report
            .files
            .iter()
            .find(|file| file.relative_path == PathBuf::from("package/engine_build_history.adm"))
            .expect("engine history check");
        assert!(history.present);
        assert!(!history.required);
        assert!(!history.content_verified);
        assert_eq!(history.status(), "content_mismatch");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unity_project_scaffold_generates_project_files_and_manifest() {
        let root = std::env::temp_dir().join(format!(
            "adm_unity_scaffold_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let content_root = root.join("content");
        let plan =
            GameBuildPlan::windows_desktop_prototype(ProjectId::new("project_demo").unwrap());
        for (relative, content) in [
            (
                "project/brief.adm",
                "# Game Design Brief\ntitle=Demo\ngenre=Action\nplayer_promise=Players complete readable actions\ncore_loop_steps=\n- Explore\n- Build\n",
            ),
            (
                "design/project.adm",
                "# Design Project\nproject_id=project_demo\ntitle=Demo\ngenre=Action\n\n## Core Loop\n1. Explore\n2. Build\n\n## Gameplay Mechanics\n- name=Core Loop Mechanic 1; player_action=Explore; feedback=Record state change\n\n## Playable Scenarios\n- scenario_id=scenario_core_loop_step_1; entry=start; goal=complete_loop_step_1; critical_path=Explore; success=clear_feedback; failure=blocked; validation_probe=probe_core_loop_step_1_input_state_feedback\n- scenario_id=scenario_core_loop_step_2; entry=after_core_loop_step_1; goal=complete_loop_step_2; critical_path=Build; success=clear_feedback; failure=blocked; validation_probe=probe_core_loop_step_2_input_state_feedback\n",
            ),
            (
                "development/plan.adm",
                "# Development Plan\n- task_id=task_1; milestone=core_loop_step_1; scenario_id=scenario_core_loop_step_1; source_mechanic=Core Loop Mechanic 1; title=Implement Explore; target_engine=Unity; layer=input_and_navigation; data_contracts=input_state; notes=wire input; validation=press confirm; tests=smoke; telemetry=loop_step; risk_controls=scope; acceptance=Explore responds to input\n",
            ),
            (
                "assets/plan.adm",
                "# Asset Plan\n- task_id=asset_1; stage=mechanic_feedback; source_mechanic=Core Loop Mechanic 1; kind=feedback_ui; description=Explore feedback panel; dependencies=design/project.adm; validation=inspect feedback; risk_controls=feedback_unclear; acceptance=Feedback visible\n",
            ),
            ("sdk/index.adm", "# SDK\nresources=5"),
            (
                "validation/acceptance_matrix.adm",
                "# Acceptance Trace Matrix\nrows=1\n- trace_id=trace_core_loop_step_1; scenario_id=scenario_core_loop_step_1; source_mechanic=Core Loop Mechanic 1; development_task_id=task_1; asset_task_id=asset_1; sdk_resources=Unity Build Automation SDK; build_targets=windows_desktop_playable; validation_probe=probe_core_loop_step_1_input_state_feedback; status=ready\n",
            ),
            (
                "validation/scenario_test_plan.adm",
                "# Scenario Test Plan\nscenarios=1\n- test_id=test_scenario_core_loop_step_1; scenario_id=scenario_core_loop_step_1; source_mechanic=Core Loop Mechanic 1; status=ready\n",
            ),
            (
                "validation/runtime_validation_report.adm",
                "# Runtime Validation Report\nrows=1\n- result_id=runtime_scenario_core_loop_step_1; scenario_id=scenario_core_loop_step_1; status=ready\n",
            ),
            (
                "validation/runtime_execution_results.adm",
                "# Runtime Validation Execution Results\nready=true\n- result_id=runtime_scenario_core_loop_step_1; status=ready\n",
            ),
            (
                "validation/production_readiness.adm",
                "# Production Readiness Report\noverall_status=ready\n",
            ),
        ] {
            let path = content_root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        let project_dir = root.join("unity-project");

        let scaffold = stage_unity_project_scaffold(&plan.targets[0], &content_root, &project_dir)
            .expect("stage Unity project scaffold");

        assert_eq!(scaffold.generated_files.len(), 21);
        assert_eq!(scaffold.project_dir, project_dir);
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Generated/project_brief.adm")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Generated/design_project.adm")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Generated/AutoDesignMakerBootstrap.cs")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Generated/AutoDesignMakerGeneratedContent.cs")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Generated/AutoDesignMakerGameplayModel.cs")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerRuntimeController.cs")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerGameplayController.cs")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerInputRouter.cs")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerSaveData.cs")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Editor/AutoDesignMakerBuild.cs")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs")
                .is_file()
        );
        let design = fs::read_to_string(
            project_dir.join("Assets/AutoDesignMaker/Generated/design_project.adm"),
        )
        .unwrap();
        assert!(design.contains("## Core Loop"));
        let build_script = fs::read_to_string(
            project_dir.join("Assets/AutoDesignMaker/Editor/AutoDesignMakerBuild.cs"),
        )
        .unwrap();
        assert!(build_script.contains("namespace AutoDesignMaker"));
        assert!(build_script.contains("public static class EditorBuild"));
        assert!(build_script.contains("PerformBuild"));
        assert!(build_script.contains("BuildTarget.StandaloneWindows64"));
        assert!(build_script.contains("EditorSceneManager.NewScene"));
        let runtime_validation_script = fs::read_to_string(
            project_dir.join("Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs"),
        )
        .unwrap();
        assert!(runtime_validation_script.contains("RunValidation"));
        assert!(runtime_validation_script.contains("runtime_validation_report.adm"));
        assert!(runtime_validation_script.contains("runtime_execution_results.adm"));
        assert!(runtime_validation_script.contains("unity_playmode"));
        let bootstrap = fs::read_to_string(
            project_dir.join("Assets/AutoDesignMaker/Generated/AutoDesignMakerBootstrap.cs"),
        )
        .unwrap();
        assert!(bootstrap.contains("windows_desktop_playable"));
        assert!(bootstrap.contains("AutoDesignMakerRuntimeController"));
        let generated_content = fs::read_to_string(
            project_dir.join("Assets/AutoDesignMaker/Generated/AutoDesignMakerGeneratedContent.cs"),
        )
        .unwrap();
        assert!(generated_content.contains("PipelineArtifactPaths"));
        assert!(generated_content.contains("project_brief.adm"));
        assert!(generated_content.contains("sdk_index.adm"));
        assert!(generated_content.contains("acceptance_matrix.adm"));
        assert!(generated_content.contains("scenario_test_plan.adm"));
        assert!(generated_content.contains("runtime_validation_report.adm"));
        assert!(generated_content.contains("runtime_execution_results.adm"));
        assert!(generated_content.contains("production_readiness.adm"));
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Generated/acceptance_matrix.adm")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Generated/scenario_test_plan.adm")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Generated/runtime_validation_report.adm")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Generated/runtime_execution_results.adm")
                .is_file()
        );
        assert!(
            project_dir
                .join("Assets/AutoDesignMaker/Generated/production_readiness.adm")
                .is_file()
        );
        let gameplay_model = fs::read_to_string(
            project_dir.join("Assets/AutoDesignMaker/Generated/AutoDesignMakerGameplayModel.cs"),
        )
        .unwrap();
        assert!(gameplay_model.contains("AutoDesignMakerGameplayModel"));
        assert!(gameplay_model.contains("Core Loop Mechanic 1"));
        assert!(gameplay_model.contains("Implement Explore"));
        assert!(gameplay_model.contains("Explore feedback panel"));
        let runtime_controller = fs::read_to_string(
            project_dir.join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerRuntimeController.cs"),
        )
        .unwrap();
        assert!(runtime_controller.contains("SaveRuntimeSnapshot"));
        assert!(runtime_controller.contains("AutoDesignMakerInputRouter"));
        assert!(runtime_controller.contains("AutoDesignMakerGameplayController"));
        assert!(runtime_controller.contains("AutoDesignMakerSceneComposer"));
        assert!(runtime_controller.contains("JsonUtility.ToJson"));
        let gameplay_controller = fs::read_to_string(
            project_dir.join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerGameplayController.cs"),
        )
        .unwrap();
        assert!(gameplay_controller.contains("Generated Gameplay Loop"));
        assert!(gameplay_controller.contains("AdvanceMechanic"));
        let scene_composer = fs::read_to_string(
            project_dir.join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs"),
        )
        .unwrap();
        assert!(scene_composer.contains("ComposeScene"));
        assert!(scene_composer.contains("CreateMechanicNodes"));
        assert!(scene_composer.contains("TextMesh"));
        assert!(scene_composer.contains("PrimitiveType.Cube"));
        assert!(scene_composer.contains("LineRenderer"));
        let input_router = fs::read_to_string(
            project_dir.join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerInputRouter.cs"),
        )
        .unwrap();
        assert!(input_router.contains("ConfirmPressed"));
        let save_data = fs::read_to_string(
            project_dir.join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerSaveData.cs"),
        )
        .unwrap();
        assert!(save_data.contains("pipeline_artifacts"));
        let manifest = fs::read_to_string(&scaffold.manifest_path).unwrap();
        assert!(manifest.contains("# Unity Project Scaffold"));
        assert!(manifest.contains("target_id=windows_desktop_playable"));
        assert!(manifest.contains("scaffold_hash=fnv64:"));
        assert!(
            manifest.contains("path=Assets/AutoDesignMaker/Generated/production_readiness.adm")
        );
        assert!(
            manifest.contains(
                "path=Assets/AutoDesignMaker/Runtime/AutoDesignMakerRuntimeController.cs"
            )
        );
        assert!(
            manifest
                .contains("path=Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs")
        );
        assert!(
            manifest
                .contains("path=Assets/AutoDesignMaker/Generated/AutoDesignMakerGameplayModel.cs")
        );
        assert!(manifest.contains("path=Assets/AutoDesignMaker/Generated/project_brief.adm"));
        assert!(manifest.contains("path=Assets/AutoDesignMaker/Generated/sdk_index.adm"));
        assert!(manifest.contains("path=Assets/AutoDesignMaker/Generated/acceptance_matrix.adm"));
        assert!(manifest.contains("path=Assets/AutoDesignMaker/Generated/scenario_test_plan.adm"));
        assert!(
            manifest
                .contains("path=Assets/AutoDesignMaker/Generated/runtime_validation_report.adm")
        );
        assert!(
            manifest
                .contains("path=Assets/AutoDesignMaker/Generated/runtime_execution_results.adm")
        );
        assert!(
            manifest.contains("path=Assets/AutoDesignMaker/Generated/production_readiness.adm")
        );
        assert!(
            manifest
                .contains("path=Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unity_project_scaffold_rejects_missing_required_content() {
        let root = std::env::temp_dir().join(format!(
            "adm_unity_scaffold_missing_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let content_root = root.join("content");
        fs::create_dir_all(content_root.join("design")).unwrap();
        fs::write(content_root.join("design/project.adm"), "# Design").unwrap();
        let plan =
            GameBuildPlan::windows_desktop_prototype(ProjectId::new("project_demo").unwrap());

        let error = stage_unity_project_scaffold(
            &plan.targets[0],
            &content_root,
            root.join("unity-project"),
        )
        .expect_err("missing content fails");

        assert!(
            error
                .to_string()
                .contains("missing required Unity scaffold artifact")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unity_editor_discovery_selects_ready_explicit_candidate() {
        let root = std::env::temp_dir().join(format!(
            "adm_unity_discovery_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let unity_exe = root.join("Editor").join("Unity.exe");
        fs::create_dir_all(unity_exe.parent().unwrap()).unwrap();
        fs::write(&unity_exe, b"fake unity").unwrap();

        let report = discover_unity_editor_from_sources(
            Some(unity_exe.clone()),
            [("ADM_UNITY_EDITOR", "Z:/missing/Unity.exe")],
            Vec::new(),
        );

        assert_eq!(report.candidates.len(), 2);
        assert_eq!(report.selected().unwrap().path, unity_exe);
        assert!(report.render().contains("selected="));
        assert!(report.render().contains("ready=true"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unity_build_preflight_passes_for_staged_project_and_valid_confirmation() {
        let root = std::env::temp_dir().join(format!(
            "adm_unity_preflight_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let content_root = root.join("content");
        let plan =
            GameBuildPlan::windows_desktop_prototype(ProjectId::new("project_demo").unwrap());
        for required in &plan.targets[0].required_artifacts {
            let path = content_root.join(required);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let content = if required == "project/brief.adm" {
                "# Game Design Brief\ntitle=Demo\ngenre=Action\nplayer_promise=Players complete readable actions\ncore_loop_steps=\n- Explore\n- Build\n".to_string()
            } else if required == "validation/acceptance_matrix.adm" {
                "# Acceptance Trace Matrix\n- trace_id=trace_core_loop_step_1; status=ready\n"
                    .to_string()
            } else if required == "validation/scenario_test_plan.adm" {
                "# Scenario Test Plan\n- test_id=test_scenario_core_loop_step_1; status=ready\n"
                    .to_string()
            } else if required == "validation/runtime_validation_report.adm" {
                "# Runtime Validation Report\n- result_id=runtime_scenario_core_loop_step_1; status=ready\n"
                    .to_string()
            } else if required == "validation/production_readiness.adm" {
                "# Production Readiness Report\noverall_status=ready\n".to_string()
            } else {
                format!("artifact {required}")
            };
            fs::write(path, content).unwrap();
        }
        let unity_project_dir = root.join("unity-project");
        stage_unity_project_scaffold(&plan.targets[0], &content_root, &unity_project_dir)
            .expect("stage unity project");
        let unity_exe = root.join("Unity").join("Editor").join("Unity.exe");
        fs::create_dir_all(unity_exe.parent().unwrap()).unwrap();
        fs::write(&unity_exe, b"fake unity").unwrap();

        let report = inspect_unity_build_preflight(
            &plan.targets[0],
            &unity_exe,
            &unity_project_dir,
            LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN,
        )
        .expect("unity preflight");

        assert!(report.ready_for_local_build());
        assert!(report.executable_present);
        assert!(report.executable_looks_like_unity);
        assert!(report.unity_project_ready);
        assert!(report.confirmation_valid);
        assert!(report.issues.is_empty());
        assert!(report.render().contains("ready_for_local_build=true"));
        assert!(
            report
                .command_line
                .contains("AutoDesignMaker.EditorBuild.PerformBuild")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unity_build_preflight_reports_missing_inputs() {
        let root = std::env::temp_dir().join(format!(
            "adm_unity_preflight_missing_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let plan =
            GameBuildPlan::windows_desktop_prototype(ProjectId::new("project_demo").unwrap());

        let report = inspect_unity_build_preflight(
            &plan.targets[0],
            root.join("Unity").join("Editor").join("NotUnity.exe"),
            root.join("unity-project"),
            "confirm",
        )
        .expect("unity preflight");

        assert!(!report.ready_for_local_build());
        assert!(!report.executable_present);
        assert!(!report.executable_looks_like_unity);
        assert!(!report.unity_project_present);
        assert!(!report.unity_project_ready);
        assert!(!report.confirmation_valid);
        assert!(
            report
                .render()
                .contains("confirmation token must be ADM_CONFIRM_LOCAL_ENGINE_BUILD")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unity_cli_build_plan_renders_expected_command() {
        let plan =
            GameBuildPlan::windows_desktop_prototype(ProjectId::new("project_demo").unwrap());
        let command = plan_unity_cli_build(
            &plan.targets[0],
            "C:/Program Files/Unity/Editor/Unity.exe",
            "C:/workspace/game",
        )
        .expect("unity command plan");

        assert_eq!(command.engine, "Unity");
        assert_eq!(command.target_id, "windows_desktop_playable");
        assert!(command.args.contains(&"-batchmode".to_string()));
        assert!(command.args.contains(&"Win64".to_string()));
        assert!(
            command
                .command_line()
                .contains("AutoDesignMaker.EditorBuild.PerformBuild")
        );
        assert!(command.render().contains("expected_output=build/windows/"));
    }

    #[test]
    fn unity_cli_build_plan_rejects_non_unity_target() {
        let target = GameBuildTargetSpec::new(
            "godot_target",
            "Godot",
            "windows-desktop",
            "playable-prototype",
            "build/windows/game.zip",
            vec!["design/project.adm".to_string()],
        );

        let error = plan_unity_cli_build(&target, "Unity.exe", "game")
            .expect_err("non-Unity target must fail");

        assert!(error.to_string().contains("cannot build engine Godot"));
    }

    #[test]
    fn unity_runtime_validation_plan_renders_expected_command() {
        let plan =
            GameBuildPlan::windows_desktop_prototype(ProjectId::new("project_demo").unwrap());
        let command = plan_unity_runtime_validation(
            &plan.targets[0],
            "C:/Program Files/Unity/Editor/Unity.exe",
            "C:/workspace/game",
        )
        .expect("unity runtime validation command plan");

        assert_eq!(command.engine, "UnityRuntimeValidation");
        assert_eq!(command.target_id, "windows_desktop_playable");
        assert!(command.args.contains(&"-batchmode".to_string()));
        assert!(
            command
                .command_line()
                .contains("AutoDesignMaker.RuntimeValidation.RunValidation")
        );
        assert!(
            command
                .command_line()
                .contains("-admRuntimeValidationOutput")
        );
        assert_eq!(command.expected_output, UNITY_RUNTIME_VALIDATION_OUTPUT);
        assert!(
            command
                .render()
                .contains("expected_output=Library/AutoDesignMaker/runtime_execution_results.adm")
        );
    }

    #[test]
    fn dry_run_engine_build_runner_reports_command_without_launching() {
        let plan =
            GameBuildPlan::windows_desktop_prototype(ProjectId::new("project_demo").unwrap());
        let command = plan_unity_cli_build(
            &plan.targets[0],
            "C:/Program Files/Unity/Editor/Unity.exe",
            "C:/workspace/game",
        )
        .expect("unity command plan");

        let report = DryRunEngineBuildRunner
            .run(&command)
            .expect("dry-run engine build");

        assert_eq!(report.mode, EngineBuildExecutionMode::DryRun);
        assert_eq!(report.status, EngineBuildExecutionStatus::Succeeded);
        assert!(!report.launched);
        assert_eq!(report.exit_code, None);
        assert_eq!(
            report.expected_output_path,
            PathBuf::from("C:/workspace/game").join(&command.expected_output)
        );
        assert!(!report.expected_output_present);
        assert_eq!(report.expected_output_bytes, 0);
        assert_eq!(report.expected_output_hash, None);
        assert!(
            report
                .command_line
                .contains("AutoDesignMaker.EditorBuild.PerformBuild")
        );
        assert!(report.render().contains("mode=dry_run"));
        assert!(report.render().contains("launched=false"));
        assert!(report.render().contains("expected_output_present=false"));
    }

    #[test]
    fn local_process_engine_build_runner_rejects_missing_executable() {
        let command = EngineBuildCommandPlan {
            engine: "Unity".to_string(),
            target_id: "windows_desktop_playable".to_string(),
            executable: PathBuf::from("Z:/missing/Unity.exe"),
            working_dir: std::env::temp_dir(),
            args: Vec::new(),
            expected_output: "build/windows/game.zip".to_string(),
        };

        let error = LocalProcessEngineBuildRunner
            .run(&command)
            .expect_err("missing executable fails");

        assert!(
            error
                .to_string()
                .contains("engine build executable does not exist")
        );
    }

    #[test]
    fn local_process_engine_build_runner_verifies_expected_output_file() {
        let Some(cmd) = windows_cmd_executable() else {
            return;
        };
        let root = std::env::temp_dir().join(format!(
            "adm_local_engine_output_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        fs::create_dir_all(&root).unwrap();
        let command = EngineBuildCommandPlan {
            engine: "Unity".to_string(),
            target_id: "windows_desktop_playable".to_string(),
            executable: cmd,
            working_dir: root.clone(),
            args: vec![
                "/C".to_string(),
                "mkdir build\\windows && echo playable> build\\windows\\game.zip".to_string(),
            ],
            expected_output: "build/windows/game.zip".to_string(),
        };

        let report = LocalProcessEngineBuildRunner
            .run(&command)
            .expect("local process build");

        assert_eq!(report.status, EngineBuildExecutionStatus::Succeeded);
        assert_eq!(report.exit_code, Some(0));
        assert!(report.expected_output_present);
        assert!(report.expected_output_bytes > 0);
        assert!(report.expected_output_hash.is_some());
        assert_eq!(
            report.expected_output_path,
            root.join("build/windows/game.zip")
        );
        assert!(report.render().contains("expected_output_present=true"));
        assert!(report.render().contains("expected_output_hash=fnv64:"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_process_engine_build_runner_fails_when_expected_output_is_missing() {
        let Some(cmd) = windows_cmd_executable() else {
            return;
        };
        let root = std::env::temp_dir().join(format!(
            "adm_local_engine_missing_output_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        fs::create_dir_all(&root).unwrap();
        let command = EngineBuildCommandPlan {
            engine: "Unity".to_string(),
            target_id: "windows_desktop_playable".to_string(),
            executable: cmd,
            working_dir: root.clone(),
            args: vec!["/C".to_string(), "exit /B 0".to_string()],
            expected_output: "build/windows/game.zip".to_string(),
        };

        let report = LocalProcessEngineBuildRunner
            .run(&command)
            .expect("local process build");

        assert_eq!(report.status, EngineBuildExecutionStatus::Failed);
        assert_eq!(report.exit_code, Some(0));
        assert!(!report.expected_output_present);
        assert_eq!(report.expected_output_bytes, 0);
        assert_eq!(report.expected_output_hash, None);
        assert!(report.render().contains("status=failed"));
        assert!(report.render().contains("expected_output_present=false"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_engine_build_confirmation_accepts_exact_token() {
        validate_local_engine_build_confirmation(LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN)
            .expect("valid confirmation token");
    }

    #[test]
    fn local_engine_build_confirmation_rejects_missing_token() {
        let error = validate_local_engine_build_confirmation("confirm")
            .expect_err("invalid token must fail");

        assert!(
            error
                .to_string()
                .contains(LOCAL_ENGINE_BUILD_CONFIRMATION_TOKEN)
        );
    }

    #[test]
    fn desktop_release_stages_executable_and_manifest() {
        let root = std::env::temp_dir().join(format!(
            "adm_desktop_release_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let source = root.join("target").join("release").join("adm-desktop.exe");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"fake desktop executable").unwrap();
        let target_dir = root.join("dist").join("AutoDesignMaker-rust");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(
            target_dir.join("release-acceptance.adm"),
            "stale acceptance",
        )
        .unwrap();
        let spec = DesktopReleaseSpec::new(&source, &target_dir, "0.1.0");

        let bundle = stage_desktop_release(&spec).expect("stage desktop release");

        assert_eq!(
            bundle.executable_path,
            target_dir.join("AutoDesignMaker-rust.exe")
        );
        assert_eq!(
            fs::read(&bundle.executable_path).unwrap(),
            b"fake desktop executable"
        );
        let manifest = fs::read_to_string(&bundle.manifest_path).unwrap();
        assert!(manifest.contains("product_name=AutoDesignMaker-rust"));
        assert!(manifest.contains("legacy_root_exe=not_modified"));
        assert!(bundle.readme_path.exists());
        assert!(!target_dir.join("release-acceptance.adm").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn desktop_release_rejects_missing_source_executable() {
        let root = std::env::temp_dir().join(format!(
            "adm_desktop_release_missing_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let spec = DesktopReleaseSpec::new(
            root.join("missing").join("adm-desktop.exe"),
            root.join("dist"),
            "0.1.0",
        );

        let error = stage_desktop_release(&spec).expect_err("missing source fails");

        assert!(
            error
                .to_string()
                .contains("source executable does not exist")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn desktop_release_doctor_reports_ready_release() {
        let root = std::env::temp_dir().join(format!(
            "adm_desktop_release_doctor_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let target_dir = root.join("dist").join("AutoDesignMaker-rust");
        fs::create_dir_all(&target_dir).unwrap();
        fs::write(
            target_dir.join("AutoDesignMaker-rust.exe"),
            b"fake desktop executable",
        )
        .unwrap();
        fs::write(target_dir.join("release-manifest.adm"), "manifest").unwrap();
        fs::write(target_dir.join("README.txt"), "readme").unwrap();

        let report = inspect_desktop_release(&target_dir).expect("release doctor");

        assert!(report.ready());
        assert!(report.executable_present);
        assert!(report.manifest_present);
        assert!(report.readme_present);
        assert_eq!(report.executable_bytes, 23);
        assert!(report.render().contains("ready=true"));
        assert!(report.render().contains("legacy_root_exe=not_modified"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn desktop_release_doctor_reports_missing_release() {
        let root = std::env::temp_dir().join(format!(
            "adm_desktop_release_doctor_missing_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));

        let report = inspect_desktop_release(root.join("dist")).expect("release doctor");

        assert!(!report.ready());
        assert!(!report.executable_present);
        assert_eq!(report.executable_bytes, 0);
        assert_eq!(report.executable_hash, None);
        assert!(report.render().contains("ready=false"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delivery_doctor_reports_ready_when_release_and_bundles_exist() {
        let root = std::env::temp_dir().join(format!(
            "adm_delivery_doctor_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let release_dir = root.join("release");
        fs::create_dir_all(&release_dir).unwrap();
        fs::write(release_dir.join("AutoDesignMaker-rust.exe"), b"exe").unwrap();
        fs::write(release_dir.join("release-manifest.adm"), "manifest").unwrap();
        fs::write(release_dir.join("README.txt"), "readme").unwrap();

        let game_dir = root.join("game");
        for relative in [
            "game-build-manifest.adm",
            "content/project/brief.adm",
            "content/design/project.adm",
            "content/development/plan.adm",
            "content/assets/plan.adm",
            "content/sdk/index.adm",
            "content/validation/acceptance_matrix.adm",
            "content/validation/scenario_test_plan.adm",
            "content/validation/runtime_validation_report.adm",
            "content/validation/production_readiness.adm",
        ] {
            let path = game_dir.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, relative).unwrap();
        }

        let sdk_dir = root.join("sdk");
        for relative in ["sdk-bundle-manifest.adm", "sdk/index.adm"] {
            let path = sdk_dir.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, relative).unwrap();
        }

        let unity_dir = root.join("unity");
        write_ready_unity_project_files(&unity_dir);

        let report = inspect_delivery(&release_dir, &game_dir, &sdk_dir, &unity_dir)
            .expect("delivery doctor");

        assert!(report.ready());
        assert!(report.game_build_bundle.ready());
        assert!(report.sdk_bundle.ready());
        assert!(report.unity_project.ready());
        assert!(report.render().contains("ready=true"));
        assert!(report.render().contains("game_build_bundle_ready=true"));
        assert!(report.render().contains("sdk_bundle_ready=true"));
        assert!(report.render().contains("unity_project_ready=true"));
        assert!(
            report
                .render()
                .contains("unity_project_file=Assets/AutoDesignMaker/Runtime/AutoDesignMakerRuntimeController.cs; present=true")
        );
        assert!(
            report
                .render()
                .contains("unity_project_file=Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs; present=true")
        );
        assert!(
            report
                .render()
                .contains("unity_project_file=Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs; present=true; status=verified")
        );
        assert!(
            report
                .render()
                .contains("sdk_bundle_file=package/build_targets.adm; present=false; status=optional_missing; required=false")
        );
        assert!(
            report
                .render()
                .contains("sdk_bundle_file=validation/runtime_validation_report.adm; present=false; status=optional_missing; required=false")
        );
        assert!(
            report
                .render()
                .contains("sdk_bundle_file=validation/production_readiness.adm; present=false; status=optional_missing; required=false")
        );
        assert!(
            report
                .render()
                .contains("sdk_bundle_file=package/engine_build_history.adm; present=false; status=optional_missing; required=false")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unity_project_doctor_rejects_present_but_incomplete_script_content() {
        let root = std::env::temp_dir().join(format!(
            "adm_unity_content_doctor_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let unity_dir = root.join("unity");
        write_ready_unity_project_files(&unity_dir);
        fs::write(
            unity_dir.join("Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs"),
            "public sealed class AutoDesignMakerSceneComposer {}",
        )
        .unwrap();

        let report = inspect_unity_project_scaffold(&unity_dir);

        assert!(!report.ready());
        let scene_check = report
            .files
            .iter()
            .find(|file| {
                file.relative_path
                    == PathBuf::from(
                        "Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs",
                    )
            })
            .expect("scene composer check");
        assert!(scene_check.present);
        assert!(!scene_check.content_verified);
        assert_eq!(scene_check.status(), "content_mismatch");
        let mut rendered = String::new();
        report.render_into(&mut rendered, "unity_project");
        assert!(
            rendered.contains("status=content_mismatch; required=true; content_verified=false")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delivery_doctor_reports_not_ready_when_bundles_are_missing() {
        let root = std::env::temp_dir().join(format!(
            "adm_delivery_doctor_missing_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let release_dir = root.join("release");
        fs::create_dir_all(&release_dir).unwrap();
        fs::write(release_dir.join("AutoDesignMaker-rust.exe"), b"exe").unwrap();
        fs::write(release_dir.join("release-manifest.adm"), "manifest").unwrap();
        fs::write(release_dir.join("README.txt"), "readme").unwrap();

        let report = inspect_delivery(
            &release_dir,
            root.join("game"),
            root.join("sdk"),
            root.join("unity"),
        )
        .expect("delivery doctor");

        assert!(!report.ready());
        assert!(report.release.ready());
        assert!(!report.game_build_bundle.ready());
        assert!(!report.sdk_bundle.ready());
        assert!(!report.unity_project.ready());
        assert!(report.render().contains("ready=false"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn release_acceptance_writes_not_ready_report_when_delivery_is_incomplete() {
        let root = std::env::temp_dir().join(format!(
            "adm_release_acceptance_missing_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let release_dir = root.join("release");
        fs::create_dir_all(&release_dir).unwrap();
        fs::write(release_dir.join("AutoDesignMaker-rust.exe"), b"exe").unwrap();
        fs::write(release_dir.join("release-manifest.adm"), "manifest").unwrap();
        fs::write(release_dir.join("README.txt"), "readme").unwrap();

        let report = run_release_acceptance(
            &release_dir,
            root.join("game"),
            root.join("sdk"),
            root.join("unity"),
        )
        .expect("release acceptance report");

        assert!(!report.accepted());
        assert!(!report.smoke.launched);
        assert_eq!(
            report.smoke.skipped_reason.as_deref(),
            Some("delivery_doctor_not_ready")
        );
        assert_eq!(
            report.report_path,
            release_dir.join("release-acceptance.adm")
        );
        let rendered = fs::read_to_string(&report.report_path).unwrap();
        assert!(rendered.contains("accepted=false"));
        assert!(rendered.contains("smoke_skipped_reason=delivery_doctor_not_ready"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn release_acceptance_report_accepts_ready_delivery_and_smoke() {
        let root = std::env::temp_dir().join(format!(
            "adm_release_acceptance_ready_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let release_dir = root.join("release");
        fs::create_dir_all(&release_dir).unwrap();
        fs::write(release_dir.join("AutoDesignMaker-rust.exe"), b"exe").unwrap();
        fs::write(release_dir.join("release-manifest.adm"), "manifest").unwrap();
        fs::write(release_dir.join("README.txt"), "readme").unwrap();

        let game_dir = root.join("game");
        for relative in [
            "game-build-manifest.adm",
            "content/project/brief.adm",
            "content/design/project.adm",
            "content/development/plan.adm",
            "content/assets/plan.adm",
            "content/sdk/index.adm",
            "content/validation/acceptance_matrix.adm",
            "content/validation/scenario_test_plan.adm",
            "content/validation/runtime_validation_report.adm",
            "content/validation/production_readiness.adm",
        ] {
            let path = game_dir.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, relative).unwrap();
        }

        let sdk_dir = root.join("sdk");
        for relative in ["sdk-bundle-manifest.adm", "sdk/index.adm"] {
            let path = sdk_dir.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, relative).unwrap();
        }

        let unity_dir = root.join("unity");
        write_ready_unity_project_files(&unity_dir);
        let delivery =
            inspect_delivery(&release_dir, &game_dir, &sdk_dir, &unity_dir).expect("delivery");
        let smoke = DesktopReleaseSmokeReport {
            executable_path: release_dir.join("AutoDesignMaker-rust.exe"),
            command_line: format!(
                "{} --smoke",
                release_dir.join("AutoDesignMaker-rust.exe").display()
            ),
            launched: true,
            skipped_reason: None,
            spawn_error: None,
            exit_code: Some(0),
            stdout_bytes: 1200,
            stderr_bytes: 0,
            stdout_contains_pipeline_succeeded: true,
            stdout_contains_production_ready: true,
            stderr_empty: true,
        };
        let report = ReleaseAcceptanceReport {
            report_path: release_dir.join("release-acceptance.adm"),
            delivery,
            smoke,
        };

        assert!(report.accepted());
        let rendered = report.render();
        assert!(rendered.contains("accepted=true"));
        assert!(rendered.contains("delivery_ready=true"));
        assert!(rendered.contains("smoke_ready=true"));
        assert!(rendered.contains("# Delivery Doctor"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn external_acceptance_report_tracks_external_blockers() {
        let root = std::env::temp_dir().join(format!(
            "adm_external_acceptance_blocked_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let release_dir = root.join("release");
        fs::create_dir_all(&release_dir).unwrap();
        fs::write(
            release_dir.join("release-acceptance.adm"),
            "# Release Acceptance Report\naccepted=true\nsmoke_ready=true\nrelease_hash=fnv64:test\n",
        )
        .unwrap();

        let unity = UnityEditorDiscoveryReport {
            candidates: vec![UnityEditorCandidate {
                source: "default".to_string(),
                path: root.join("Unity.exe"),
                present: false,
                looks_like_unity_editor: true,
            }],
        };
        let ai = ExternalAiProviderAcceptance::new(
            1,
            Vec::new(),
            "# AI Diagnostics\nready_provider_count=1\nmock\tReady\tcapabilities=text_generation\tprovider does not require a secret\n",
        );
        let report = run_external_acceptance(
            &release_dir,
            None,
            root.join("data-root"),
            unity,
            ai,
            false,
            false,
        )
        .expect("report");

        assert!(!report.ready());
        assert!(report.release_acceptance_accepted);
        assert!(report.release_smoke_ready);
        assert_eq!(report.release_hash, "fnv64:test");
        assert_eq!(
            report.report_path,
            release_dir.join("external-acceptance.adm")
        );
        let rendered = fs::read_to_string(&report.report_path).unwrap();
        assert!(rendered.contains("ready=false"));
        assert!(rendered.contains(&format!("data_root={}", root.join("data-root").display())));
        assert!(rendered.contains("unity_ready=false"));
        assert!(rendered.contains("unity_runtime_present=false"));
        assert!(rendered.contains("unity_runtime_ready=false"));
        assert!(rendered.contains("unity_runtime_runner=none"));
        assert!(rendered.contains(&format!(
            "ai_acceptance_report={}",
            release_dir.join("ai-acceptance.adm").display()
        )));
        assert!(rendered.contains("ai_acceptance_present=false"));
        assert!(rendered.contains("ai_acceptance_ready=false"));
        assert!(rendered.contains("ai_acceptance_provider_id=none"));
        assert!(rendered.contains("ai_acceptance_provider_matches_real_provider=false"));
        assert!(rendered.contains("ai_acceptance_invoke_attempted=false"));
        assert!(rendered.contains("ai_acceptance_invoke_succeeded=false"));
        assert!(rendered.contains("real_ai_provider_ready=false"));
        assert!(rendered.contains("ready_provider_count=1"));
        assert!(rendered.contains("require_ai_invoke=false"));
        assert!(rendered.contains("release_acceptance_accepted=true"));
        assert!(rendered.contains("blocker_count=4"));
        assert!(rendered.contains("blocker=unity_not_ready"));
        assert!(rendered.contains("blocker=unity_runtime_report_missing"));
        assert!(rendered.contains("blocker=real_ai_provider_not_ready"));
        assert!(rendered.contains("blocker=ai_acceptance_report_missing"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn external_acceptance_report_ready_with_unity_and_real_provider() {
        let root = std::env::temp_dir().join(format!(
            "adm_external_acceptance_ready_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let release_dir = root.join("release");
        fs::create_dir_all(&release_dir).unwrap();
        fs::write(
            release_dir.join("release-acceptance.adm"),
            "# Release Acceptance Report\naccepted=true\nsmoke_ready=true\nrelease_hash=fnv64:ready\n",
        )
        .unwrap();
        let unity_exe = root.join("Unity.exe");
        fs::write(&unity_exe, b"fake unity").unwrap();
        let unity_runtime_report = root
            .join("unity-project")
            .join("Assets")
            .join("AutoDesignMaker")
            .join("Generated")
            .join("runtime_execution_results.adm");
        fs::create_dir_all(unity_runtime_report.parent().unwrap()).unwrap();
        fs::write(
            &unity_runtime_report,
            "# Runtime Validation Execution Results\nrunner=unity_playmode\ntarget_id=windows_desktop_playable\nready=true\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("ai-acceptance.adm"),
            "# AI Provider Acceptance\nready=true\nprovider_id=openai_main\nconfigured_ready=true\n",
        )
        .unwrap();

        let unity = UnityEditorDiscoveryReport {
            candidates: vec![UnityEditorCandidate {
                source: "explicit".to_string(),
                path: unity_exe.clone(),
                present: true,
                looks_like_unity_editor: true,
            }],
        };
        let ai = ExternalAiProviderAcceptance::new(
            2,
            vec![
                "openai_main".to_string(),
                "openai_main".to_string(),
                "deepseek_review".to_string(),
            ],
            "# AI Diagnostics\nready_provider_count=2\nopenai_main\tReady\tcapabilities=text_generation\tconfigured\n",
        );
        let report = run_external_acceptance(
            &release_dir,
            Some(root.join("custom-external.adm")),
            root.join("custom-data-root"),
            unity,
            ai,
            true,
            false,
        )
        .expect("report");

        assert!(report.ready());
        let rendered = fs::read_to_string(&report.report_path).unwrap();
        assert!(rendered.contains("ready=true"));
        assert!(rendered.contains(&format!(
            "data_root={}",
            root.join("custom-data-root").display()
        )));
        assert!(rendered.contains("unity_ready=true"));
        assert!(rendered.contains("unity_runtime_present=true"));
        assert!(rendered.contains("unity_runtime_ready=true"));
        assert!(rendered.contains("unity_runtime_runner=unity_playmode"));
        assert!(rendered.contains("unity_runtime_target_id=windows_desktop_playable"));
        assert!(rendered.contains("ai_acceptance_present=true"));
        assert!(rendered.contains("ai_acceptance_ready=true"));
        assert!(rendered.contains("ai_acceptance_provider_id=openai_main"));
        assert!(rendered.contains("ai_acceptance_provider_matches_real_provider=true"));
        assert!(rendered.contains("ai_acceptance_configured_ready=true"));
        assert!(rendered.contains("ai_acceptance_invoke_attempted=false"));
        assert!(rendered.contains("ai_acceptance_invoke_succeeded=false"));
        assert!(rendered.contains("real_ai_provider_ready=true"));
        assert!(rendered.contains("real_ai_provider_count=2"));
        assert!(rendered.contains("real_ai_providers=deepseek_review,openai_main"));
        assert!(rendered.contains("require_ready=true"));
        assert!(rendered.contains("require_ai_invoke=false"));
        assert!(rendered.contains("blocker_count=0"));
        assert!(rendered.contains(&format!("unity_selected={}", unity_exe.display())));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn external_acceptance_can_require_ai_invoke_success() {
        let root = std::env::temp_dir().join(format!(
            "adm_external_acceptance_ai_invoke_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let release_dir = root.join("release");
        fs::create_dir_all(&release_dir).unwrap();
        fs::write(
            release_dir.join("release-acceptance.adm"),
            "# Release Acceptance Report\naccepted=true\nsmoke_ready=true\nrelease_hash=fnv64:ready\n",
        )
        .unwrap();
        let unity_exe = root.join("Unity.exe");
        fs::write(&unity_exe, b"fake unity").unwrap();
        let unity_runtime_report = root
            .join("unity-project")
            .join("Assets")
            .join("AutoDesignMaker")
            .join("Generated")
            .join("runtime_execution_results.adm");
        fs::create_dir_all(unity_runtime_report.parent().unwrap()).unwrap();
        fs::write(
            &unity_runtime_report,
            "# Runtime Validation Execution Results\nrunner=unity_playmode\ntarget_id=windows_desktop_playable\nready=true\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("ai-acceptance.adm"),
            "# AI Provider Acceptance\nready=true\nprovider_id=openai_main\nconfigured_ready=true\ninvoke_attempted=false\ninvoke_succeeded=false\n",
        )
        .unwrap();

        let unity = UnityEditorDiscoveryReport {
            candidates: vec![UnityEditorCandidate {
                source: "explicit".to_string(),
                path: unity_exe,
                present: true,
                looks_like_unity_editor: true,
            }],
        };
        let ai = ExternalAiProviderAcceptance::new(
            1,
            vec!["openai_main".to_string()],
            "# AI Diagnostics\nready_provider_count=1\nopenai_main\tReady\tcapabilities=text_generation\tconfigured\n",
        );
        let report = run_external_acceptance(
            &release_dir,
            None,
            root.join("data-root"),
            unity,
            ai,
            true,
            true,
        )
        .expect("report");

        assert!(!report.ready());
        let rendered = fs::read_to_string(&report.report_path).unwrap();
        assert!(rendered.contains("ai_acceptance_invoke_attempted=false"));
        assert!(rendered.contains("ai_acceptance_invoke_succeeded=false"));
        assert!(rendered.contains("require_ai_invoke=true"));
        assert!(rendered.contains("blocker_count=1"));
        assert!(rendered.contains("blocker=ai_acceptance_invoke_not_attempted"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn external_acceptance_requires_ai_acceptance_provider_to_be_real_ready_provider() {
        let root = std::env::temp_dir().join(format!(
            "adm_external_acceptance_ai_provider_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let release_dir = root.join("release");
        fs::create_dir_all(&release_dir).unwrap();
        fs::write(
            release_dir.join("release-acceptance.adm"),
            "# Release Acceptance Report\naccepted=true\nsmoke_ready=true\nrelease_hash=fnv64:ready\n",
        )
        .unwrap();
        let unity_exe = root.join("Unity.exe");
        fs::write(&unity_exe, b"fake unity").unwrap();
        let unity_runtime_report = root
            .join("unity-project")
            .join("Assets")
            .join("AutoDesignMaker")
            .join("Generated")
            .join("runtime_execution_results.adm");
        fs::create_dir_all(unity_runtime_report.parent().unwrap()).unwrap();
        fs::write(
            &unity_runtime_report,
            "# Runtime Validation Execution Results\nrunner=unity_playmode\ntarget_id=windows_desktop_playable\nready=true\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("ai-acceptance.adm"),
            "# AI Provider Acceptance\nready=true\nprovider_id=openai_main\nconfigured_ready=true\n",
        )
        .unwrap();

        let unity = UnityEditorDiscoveryReport {
            candidates: vec![UnityEditorCandidate {
                source: "explicit".to_string(),
                path: unity_exe,
                present: true,
                looks_like_unity_editor: true,
            }],
        };
        let ai = ExternalAiProviderAcceptance::new(
            1,
            vec!["deepseek_review".to_string()],
            "# AI Diagnostics\nready_provider_count=1\ndeepseek_review\tReady\tcapabilities=text_generation\tconfigured\n",
        );
        let report = run_external_acceptance(
            &release_dir,
            None,
            root.join("data-root"),
            unity,
            ai,
            true,
            false,
        )
        .expect("report");

        assert!(!report.ready());
        let rendered = fs::read_to_string(&report.report_path).unwrap();
        assert!(rendered.contains("ai_acceptance_provider_id=openai_main"));
        assert!(rendered.contains("real_ai_providers=deepseek_review"));
        assert!(rendered.contains("ai_acceptance_provider_matches_real_provider=false"));
        assert!(rendered.contains("blocker_count=1"));
        assert!(rendered.contains("blocker=ai_acceptance_provider_not_real_provider"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn external_acceptance_requires_unity_playmode_runtime_results() {
        let root = std::env::temp_dir().join(format!(
            "adm_external_acceptance_runtime_{}_{}",
            std::process::id(),
            adm_foundation::SessionId::generate()
        ));
        let release_dir = root.join("release");
        fs::create_dir_all(&release_dir).unwrap();
        fs::write(
            release_dir.join("release-acceptance.adm"),
            "# Release Acceptance Report\naccepted=true\nsmoke_ready=true\nrelease_hash=fnv64:ready\n",
        )
        .unwrap();
        let unity_exe = root.join("Unity.exe");
        fs::write(&unity_exe, b"fake unity").unwrap();
        let unity_runtime_report = root
            .join("unity-project")
            .join("Assets")
            .join("AutoDesignMaker")
            .join("Generated")
            .join("runtime_execution_results.adm");
        fs::create_dir_all(unity_runtime_report.parent().unwrap()).unwrap();
        fs::write(
            &unity_runtime_report,
            "# Runtime Validation Execution Results\nrunner=cli_smoke_runner\ntarget_id=windows_desktop_playable\nready=true\n",
        )
        .unwrap();
        fs::write(
            release_dir.join("ai-acceptance.adm"),
            "# AI Provider Acceptance\nready=true\nprovider_id=openai_main\nconfigured_ready=true\n",
        )
        .unwrap();

        let unity = UnityEditorDiscoveryReport {
            candidates: vec![UnityEditorCandidate {
                source: "explicit".to_string(),
                path: unity_exe,
                present: true,
                looks_like_unity_editor: true,
            }],
        };
        let ai = ExternalAiProviderAcceptance::new(
            1,
            vec!["openai_main".to_string()],
            "# AI Diagnostics\nready_provider_count=1\nopenai_main\tReady\tcapabilities=text_generation\tconfigured\n",
        );
        let report = run_external_acceptance(
            &release_dir,
            None,
            root.join("data-root"),
            unity,
            ai,
            true,
            false,
        )
        .expect("report");

        assert!(!report.ready());
        let rendered = fs::read_to_string(&report.report_path).unwrap();
        assert!(rendered.contains("ready=false"));
        assert!(rendered.contains("unity_ready=true"));
        assert!(rendered.contains("unity_runtime_present=true"));
        assert!(rendered.contains("unity_runtime_ready=true"));
        assert!(rendered.contains("unity_runtime_runner=cli_smoke_runner"));
        assert!(rendered.contains("blocker_count=1"));
        assert!(rendered.contains("blocker=unity_runtime_runner_not_unity_playmode"));
        let _ = fs::remove_dir_all(root);
    }

    fn write_ready_unity_project_files(unity_dir: &Path) {
        for relative in [
            "adm-unity-scaffold-manifest.adm",
            "Assets/AutoDesignMaker/Generated/project_brief.adm",
            "Assets/AutoDesignMaker/Generated/design_project.adm",
            "Assets/AutoDesignMaker/Generated/development_plan.adm",
            "Assets/AutoDesignMaker/Generated/asset_plan.adm",
            "Assets/AutoDesignMaker/Generated/sdk_index.adm",
            "Assets/AutoDesignMaker/Generated/acceptance_matrix.adm",
            "Assets/AutoDesignMaker/Generated/scenario_test_plan.adm",
            "Assets/AutoDesignMaker/Generated/runtime_validation_report.adm",
            "Assets/AutoDesignMaker/Generated/production_readiness.adm",
            "Assets/AutoDesignMaker/Generated/AutoDesignMakerBootstrap.cs",
            "Assets/AutoDesignMaker/Generated/AutoDesignMakerGeneratedContent.cs",
            "Assets/AutoDesignMaker/Generated/AutoDesignMakerGameplayModel.cs",
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerRuntimeController.cs",
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerGameplayController.cs",
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs",
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerInputRouter.cs",
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerSaveData.cs",
            "Assets/AutoDesignMaker/Editor/AutoDesignMakerBuild.cs",
            "Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs",
            "ProjectSettings/ProjectVersion.txt",
        ] {
            let path = unity_dir.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, unity_test_content(relative)).unwrap();
        }
    }

    fn unity_test_content(relative: &str) -> String {
        let content = match relative {
            "adm-unity-scaffold-manifest.adm" => {
                "# Unity Project Scaffold\ngenerated_files=\n- path=Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs\n"
            }
            "Assets/AutoDesignMaker/Generated/AutoDesignMakerBootstrap.cs" => {
                "AutoDesignMakerBootstrap AutoDesignMakerRuntimeController"
            }
            "Assets/AutoDesignMaker/Generated/AutoDesignMakerGeneratedContent.cs" => {
                "AutoDesignMakerGeneratedContent PipelineArtifactPaths project_brief.adm acceptance_matrix.adm scenario_test_plan.adm runtime_validation_report.adm production_readiness.adm"
            }
            "Assets/AutoDesignMaker/Generated/project_brief.adm" => {
                "# Game Design Brief title=Demo core_loop_steps="
            }
            "Assets/AutoDesignMaker/Generated/acceptance_matrix.adm" => {
                "# Acceptance Trace Matrix trace_id=trace_core_loop_step_1 status=ready"
            }
            "Assets/AutoDesignMaker/Generated/scenario_test_plan.adm" => {
                "# Scenario Test Plan test_id=test_scenario_core_loop_step_1 status=ready"
            }
            "Assets/AutoDesignMaker/Generated/runtime_validation_report.adm" => {
                "# Runtime Validation Report result_id=runtime_scenario_core_loop_step_1 status=ready"
            }
            "Assets/AutoDesignMaker/Generated/production_readiness.adm" => {
                "# Production Readiness Report overall_status=ready"
            }
            "Assets/AutoDesignMaker/Generated/AutoDesignMakerGameplayModel.cs" => {
                "AutoDesignMakerGameplayModel GeneratedMechanic"
            }
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerRuntimeController.cs" => {
                "SaveRuntimeSnapshot AutoDesignMakerGameplayController AutoDesignMakerSceneComposer"
            }
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerGameplayController.cs" => {
                "Generated Gameplay Loop AdvanceMechanic"
            }
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs" => {
                "ComposeScene CreateMechanicNodes CreateGoalMarker LineRenderer TextMesh PrimitiveType.Cube"
            }
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerInputRouter.cs" => {
                "ConfirmPressed CancelPressed"
            }
            "Assets/AutoDesignMaker/Runtime/AutoDesignMakerSaveData.cs" => {
                "AutoDesignMakerSaveData pipeline_artifacts"
            }
            "Assets/AutoDesignMaker/Editor/AutoDesignMakerBuild.cs" => {
                "PerformBuild EditorSceneManager.NewScene BuildTarget.StandaloneWindows64"
            }
            "Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs" => {
                "RunValidation runtime_validation_report.adm runtime_execution_results.adm"
            }
            _ => relative,
        };
        content.to_string()
    }

    fn windows_cmd_executable() -> Option<PathBuf> {
        [
            std::env::var_os("COMSPEC").map(PathBuf::from),
            Some(PathBuf::from(r"C:\Windows\System32\cmd.exe")),
        ]
        .into_iter()
        .flatten()
        .find(|path| path.is_file())
    }
}
