#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use adm_new_contracts::artifact::{
    ArtifactCheckResult, ArtifactCheckStatus, ArtifactContract, ArtifactLayerManifest,
    ArtifactRegistry, ArtifactReportStatus, ArtifactReview, ArtifactReviewReport, ArtifactSeverity,
    ArtifactValidation, ArtifactValidationLayerReport, DependencyGraph, DependencyGraphEdge,
    DependencyGraphNode,
};
use adm_new_contracts::schema;
use adm_new_foundation::io::{read_json, rel, write_json, write_json_serializable};
use adm_new_foundation::paths::ProjectPaths;
use adm_new_foundation::{
    AdmError, AdmResult, file_manifest, sanitize_identifier, sha256_hex, unix_timestamp,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub mod asset_tools;

pub const CRATE_NAME: &str = "adm-new-artifact";
pub const REGISTRY_RELATIVE_PATH: &str = "pipeline/artifact_layer/registry.json";
pub const DEPENDENCY_GRAPH_RELATIVE_PATH: &str = "pipeline/artifact_layer/dependency_graph.json";
pub const ARTIFACT_LAYER_DIR_NAME: &str = "artifact_layer";

pub fn crate_ready() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct ArtifactService {
    registry: ArtifactRegistry,
    artifacts_by_id: BTreeMap<String, ArtifactContract>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArtifactEvidenceSet {
    pub existing_paths: BTreeSet<String>,
    pub upstream_validation_status: BTreeMap<String, ArtifactReportStatus>,
}

impl ArtifactEvidenceSet {
    pub fn with_paths(paths: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            existing_paths: paths.into_iter().map(Into::into).collect(),
            upstream_validation_status: BTreeMap::new(),
        }
    }

    pub fn path_exists(&self, path: &str) -> bool {
        self.existing_paths.contains(&normalize_path(path))
    }

    pub fn upstream_success(&self, artifact_id: &str) -> bool {
        self.upstream_validation_status
            .get(artifact_id)
            .map(|status| *status == ArtifactReportStatus::Success)
            .unwrap_or(false)
    }
}

impl ArtifactService {
    pub fn new(registry: ArtifactRegistry) -> AdmResult<Self> {
        let mut artifacts_by_id = BTreeMap::new();
        for artifact in &registry.artifacts {
            if artifact.id.trim().is_empty() {
                return Err(AdmError::new("artifact id cannot be empty"));
            }
            if artifacts_by_id
                .insert(artifact.id.clone(), artifact.clone())
                .is_some()
            {
                return Err(AdmError::new(format!(
                    "duplicated artifact id: {}",
                    artifact.id
                )));
            }
        }
        Ok(Self {
            registry,
            artifacts_by_id,
        })
    }

    pub fn registry(&self) -> &ArtifactRegistry {
        &self.registry
    }

    pub fn load_registry(project_root: impl AsRef<Path>) -> AdmResult<ArtifactRegistry> {
        let path = project_root.as_ref().join(REGISTRY_RELATIVE_PATH);
        if !path.exists() {
            return Err(AdmError::new(format!(
                "Missing artifact layer registry: {}",
                path.display()
            )));
        }
        let text = fs::read_to_string(&path)?;
        let registry: ArtifactRegistry = serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|error| {
                AdmError::new(format!("failed to parse artifact registry: {error}"))
            })?;
        if registry.artifacts.is_empty() {
            return Err(AdmError::new(
                "artifact_layer/registry.json must declare a non-empty artifacts list.",
            ));
        }
        Ok(registry)
    }

    pub fn artifacts_by_id(&self) -> BTreeMap<String, ArtifactContract> {
        self.artifacts_by_id.clone()
    }

    pub fn artifacts_for_stage(&self, stage: u32) -> Vec<ArtifactContract> {
        self.registry
            .artifacts
            .iter()
            .filter(|artifact| artifact.stage == stage)
            .cloned()
            .collect()
    }

    pub fn build_dependency_graph(&self) -> DependencyGraph {
        let mut graph = DependencyGraph {
            nodes: self
                .registry
                .artifacts
                .iter()
                .map(|artifact| DependencyGraphNode {
                    id: artifact.id.clone(),
                    stage: artifact.stage,
                    kind: artifact.kind.clone(),
                })
                .collect(),
            edges: Vec::new(),
            topological_order: Vec::new(),
            errors: Vec::new(),
        };
        for artifact in &self.registry.artifacts {
            for dependency in &artifact.depends_on {
                if self.artifacts_by_id.contains_key(dependency) {
                    graph.edges.push(DependencyGraphEdge {
                        from: dependency.clone(),
                        to: artifact.id.clone(),
                    });
                } else {
                    graph.errors.push(format!(
                        "artifact {} depends on unknown artifact {dependency}",
                        artifact.id
                    ));
                }
            }
        }
        if graph.errors.is_empty() {
            match self.topological_artifact_order() {
                Ok(order) => graph.topological_order = order,
                Err(error) => graph.errors.push(error.message().to_string()),
            }
        }
        graph
    }

    pub fn topological_artifact_order(&self) -> AdmResult<Vec<String>> {
        let mut incoming = self
            .artifacts_by_id
            .keys()
            .map(|id| (id.clone(), 0usize))
            .collect::<BTreeMap<_, _>>();
        let mut outgoing = self
            .artifacts_by_id
            .keys()
            .map(|id| (id.clone(), Vec::<String>::new()))
            .collect::<BTreeMap<_, _>>();
        for artifact in self.artifacts_by_id.values() {
            for dependency in &artifact.depends_on {
                if !self.artifacts_by_id.contains_key(dependency) {
                    return Err(AdmError::new(format!(
                        "artifact {} depends on unknown artifact {dependency}",
                        artifact.id
                    )));
                }
                *incoming
                    .get_mut(&artifact.id)
                    .ok_or_else(|| AdmError::new("artifact missing incoming slot"))? += 1;
                outgoing
                    .get_mut(dependency)
                    .ok_or_else(|| AdmError::new("artifact missing outgoing slot"))?
                    .push(artifact.id.clone());
            }
        }
        let mut ready = incoming
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(id, _)| id.clone())
            .collect::<BTreeSet<_>>();
        let mut order = Vec::new();
        while let Some(id) = ready.pop_first() {
            order.push(id.clone());
            for next in outgoing.remove(&id).unwrap_or_default() {
                let count = incoming
                    .get_mut(&next)
                    .ok_or_else(|| AdmError::new("artifact missing incoming count"))?;
                *count -= 1;
                if *count == 0 {
                    ready.insert(next);
                }
            }
        }
        if order.len() != self.artifacts_by_id.len() {
            return Err(AdmError::new("artifact dependency graph contains a cycle"));
        }
        Ok(order)
    }

    pub fn topological_step_order(&self, from_step: u32, stop_step: u32) -> AdmResult<Vec<u32>> {
        let order = self.topological_artifact_order()?;
        let mut steps = Vec::new();
        let mut seen = BTreeSet::new();
        for artifact_id in order {
            let artifact = self
                .artifacts_by_id
                .get(&artifact_id)
                .ok_or_else(|| AdmError::new(format!("missing artifact: {artifact_id}")))?;
            if (from_step..=stop_step).contains(&artifact.stage) && seen.insert(artifact.stage) {
                steps.push(artifact.stage);
            }
        }
        Ok(steps)
    }

    pub fn build_dependency_graph_json(&self) -> Value {
        let graph = self.build_dependency_graph();
        json!({
            "version": self.registry.version,
            "nodes": self.registry.artifacts.iter().map(|artifact| {
                json!({
                    "id": artifact.id,
                    "stage": artifact.stage,
                    "kind": artifact.kind,
                    "tasks": artifact.tasks.iter().map(|task| task.id.clone()).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
            "edges": graph.edges.iter().map(|edge| json!({"from": edge.from, "to": edge.to})).collect::<Vec<_>>(),
            "topological_order": graph.topological_order,
            "errors": graph.errors,
        })
    }

    pub fn preflight_stage_contract(
        &self,
        stage: u32,
        evidence: &ArtifactEvidenceSet,
    ) -> adm_new_contracts::artifact::PreflightReport {
        let artifacts = self.artifacts_for_stage(stage);
        let mut errors = Vec::new();
        let warnings = Vec::new();
        if artifacts.is_empty() {
            errors.push(format!("stage {stage} has no declared artifacts"));
        }
        for artifact in &artifacts {
            errors.extend(self.contract_errors(artifact, evidence, true));
        }
        adm_new_contracts::artifact::PreflightReport {
            step: stage,
            timestamp: timestamp(),
            status: report_status(errors.is_empty(), &warnings),
            phase: "preflight".to_string(),
            artifacts: artifacts.into_iter().map(|artifact| artifact.id).collect(),
            errors,
            warnings,
        }
    }

    pub fn run_review_pipeline(
        &self,
        stage: u32,
        evidence: &ArtifactEvidenceSet,
    ) -> ArtifactReviewReport {
        let reviews = self
            .artifacts_for_stage(stage)
            .into_iter()
            .map(|artifact| {
                let results = vec![
                    check(
                        "structure_reviewer",
                        stage_files_exist(stage, evidence),
                        ArtifactSeverity::Error,
                        "stage artifact index and reference manifest must exist",
                    ),
                    check(
                        "source_trace_reviewer",
                        evidence.path_exists(&format!(
                            "outputs/artifacts/stage_{stage:02}/validation_report.json"
                        )),
                        ArtifactSeverity::Error,
                        "validation_report.json must exist",
                    ),
                    check(
                        "task_reviewer",
                        artifact.duplicate_task_ids().is_empty() && !artifact.tasks.is_empty(),
                        ArtifactSeverity::Error,
                        "tasks must exist and task ids cannot repeat",
                    ),
                    check(
                        "dependency_reviewer",
                        self.dependencies_success(&artifact, evidence),
                        ArtifactSeverity::Error,
                        "upstream artifact validation must be success",
                    ),
                ];
                ArtifactReview {
                    artifact_id: artifact.id,
                    status: aggregate_check_status(&results),
                    results,
                }
            })
            .collect::<Vec<_>>();
        ArtifactReviewReport {
            step: stage,
            timestamp: timestamp(),
            status: report_status_from_reviews(&reviews),
            phase: "review".to_string(),
            reviews,
        }
    }

    pub fn run_artifact_validators(
        &self,
        stage: u32,
        manifest: &ArtifactLayerManifest,
        review_report: &ArtifactReviewReport,
        evidence: &ArtifactEvidenceSet,
    ) -> ArtifactValidationLayerReport {
        let validations = self
            .artifacts_for_stage(stage)
            .into_iter()
            .map(|artifact| {
                let mut results = Vec::new();
                for validator in &artifact.validators {
                    results.push(self.run_validator(
                        validator,
                        &artifact,
                        manifest,
                        review_report,
                        evidence,
                    ));
                }
                ArtifactValidation {
                    artifact_id: artifact.id,
                    status: aggregate_check_status(&results),
                    results,
                }
            })
            .collect::<Vec<_>>();
        ArtifactValidationLayerReport {
            step: stage,
            timestamp: timestamp(),
            status: report_status_from_validations(&validations),
            phase: "validation".to_string(),
            validations,
        }
    }

    fn contract_errors(
        &self,
        artifact: &ArtifactContract,
        evidence: &ArtifactEvidenceSet,
        preflight: bool,
    ) -> Vec<String> {
        let mut errors = Vec::new();
        if artifact.tasks.is_empty() {
            errors.push(format!("{} must declare tasks", artifact.id));
        }
        if artifact.reviewers.is_empty() {
            errors.push(format!("{} must declare reviewers", artifact.id));
        }
        if artifact.validators.is_empty() {
            errors.push(format!("{} must declare validators", artifact.id));
        }
        for reviewer in artifact.unknown_reviewers() {
            errors.push(format!("{} uses unknown reviewer {reviewer}", artifact.id));
        }
        for validator in artifact.unknown_validators() {
            errors.push(format!(
                "{} uses unknown validator {validator}",
                artifact.id
            ));
        }
        for duplicate in artifact.duplicate_task_ids() {
            errors.push(format!("{} has duplicate task id {duplicate}", artifact.id));
        }
        for dependency in &artifact.depends_on {
            if !self.artifacts_by_id.contains_key(dependency) {
                errors.push(format!(
                    "{} depends on unknown artifact {dependency}",
                    artifact.id
                ));
            } else if preflight && !evidence.upstream_success(dependency) {
                errors.push(format!(
                    "{} dependency {dependency} validation is not success",
                    artifact.id
                ));
            }
        }
        for knowledge_ref in &artifact.knowledge_refs {
            if !evidence.path_exists(knowledge_ref) {
                errors.push(format!(
                    "{} missing knowledge_ref {knowledge_ref}",
                    artifact.id
                ));
            }
        }
        if artifact
            .validators
            .iter()
            .any(|validator| validator == "schema_contract_validator")
        {
            if artifact.schema_refs.is_empty() {
                errors.push(format!(
                    "{} uses schema_contract_validator but has no schema_refs",
                    artifact.id
                ));
            }
            for schema_ref in &artifact.schema_refs {
                if !evidence.path_exists(&schema_ref.schema) {
                    errors.push(format!(
                        "{} missing schema_ref schema {}",
                        artifact.id, schema_ref.schema
                    ));
                }
            }
        }
        errors
    }

    fn dependencies_success(
        &self,
        artifact: &ArtifactContract,
        evidence: &ArtifactEvidenceSet,
    ) -> bool {
        artifact
            .depends_on
            .iter()
            .all(|dependency| evidence.upstream_success(dependency))
    }

    fn run_validator(
        &self,
        validator: &str,
        artifact: &ArtifactContract,
        manifest: &ArtifactLayerManifest,
        review_report: &ArtifactReviewReport,
        evidence: &ArtifactEvidenceSet,
    ) -> ArtifactCheckResult {
        match validator {
            "validator_first_contract" => check(
                validator,
                self.contract_errors(artifact, evidence, false).is_empty(),
                ArtifactSeverity::Error,
                "artifact must declare valid tasks, reviewers, validators, and refs",
            ),
            "stage_files_validator" => check(
                validator,
                stage_files_exist(artifact.stage, evidence),
                ArtifactSeverity::Error,
                "stage files must exist",
            ),
            "review_report_validator" => check(
                validator,
                review_report.status == ArtifactReportStatus::Success,
                ArtifactSeverity::Error,
                "review report must be success",
            ),
            "manifest_validator" => check(
                validator,
                !manifest.artifacts.is_empty() && !manifest.tasks.is_empty(),
                ArtifactSeverity::Error,
                "artifact manifest must contain artifacts and tasks",
            ),
            "knowledge_refs_validator" => check(
                validator,
                artifact
                    .knowledge_refs
                    .iter()
                    .all(|path| evidence.path_exists(path)),
                ArtifactSeverity::Error,
                "knowledge refs must exist",
            ),
            "schema_contract_validator" => {
                let ok = !artifact.schema_refs.is_empty()
                    && artifact.schema_refs.iter().all(|schema_ref| {
                        evidence.path_exists(&schema_ref.schema)
                            && evidence.path_exists(&schema_ref.path)
                    });
                check(
                    validator,
                    ok,
                    ArtifactSeverity::Error,
                    "schema refs and target files must exist",
                )
            }
            "dependency_status_validator" => check(
                validator,
                self.dependencies_success(artifact, evidence),
                ArtifactSeverity::Error,
                "dependency validation status must be success",
            ),
            other => ArtifactCheckResult {
                name: other.to_string(),
                status: ArtifactCheckStatus::Fail,
                severity: ArtifactSeverity::Error,
                message: format!("unknown validator {other}"),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArtifactFileService {
    paths: ProjectPaths,
    service: ArtifactService,
}

impl ArtifactFileService {
    pub fn new(root: impl AsRef<Path>, session_id: &str) -> AdmResult<Self> {
        let registry = ArtifactService::load_registry(root.as_ref())?;
        Self::with_registry(root, session_id, registry)
    }

    pub fn with_registry(
        root: impl AsRef<Path>,
        session_id: &str,
        registry: ArtifactRegistry,
    ) -> AdmResult<Self> {
        let session_id = sanitize_identifier(session_id)?;
        let paths = ProjectPaths::new(root.as_ref(), session_id);
        paths.ensure_current_draft_dirs()?;
        Ok(Self {
            paths,
            service: ArtifactService::new(registry)?,
        })
    }

    pub fn paths(&self) -> &ProjectPaths {
        &self.paths
    }

    pub fn service(&self) -> &ArtifactService {
        &self.service
    }

    pub fn registry_path(&self) -> PathBuf {
        self.paths.project_root.join(REGISTRY_RELATIVE_PATH)
    }

    pub fn graph_path(&self) -> PathBuf {
        self.paths.project_root.join(DEPENDENCY_GRAPH_RELATIVE_PATH)
    }

    pub fn output_graph_path(&self) -> PathBuf {
        self.paths.outputs_dir.join("dependency_graph.json")
    }

    pub fn layer_output_dir(&self) -> PathBuf {
        self.paths.outputs_dir.join(ARTIFACT_LAYER_DIR_NAME)
    }

    pub fn stage_dir(&self, step_number: u32) -> PathBuf {
        self.paths
            .artifacts_dir
            .join(format!("stage_{step_number:02}"))
    }

    pub fn emit_dependency_graph(&self) -> AdmResult<Value> {
        let graph = self.service.build_dependency_graph_json();
        write_json(&self.graph_path(), &graph)?;
        write_json(&self.output_graph_path(), &graph)?;
        if let Some(errors) = graph.get("errors").and_then(Value::as_array)
            && !errors.is_empty()
        {
            return Err(AdmError::new(format!(
                "Artifact dependency graph is invalid: {}",
                errors
                    .iter()
                    .map(value_to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
        Ok(graph)
    }

    pub fn preflight_stage_contract(
        &self,
        step_number: u32,
    ) -> AdmResult<adm_new_contracts::artifact::PreflightReport> {
        let evidence = self.evidence_for_stage(step_number);
        let report = self
            .service
            .preflight_stage_contract(step_number, &evidence);
        let path = self
            .layer_output_dir()
            .join(format!("preflight_stage_{step_number:02}.json"));
        write_json_serializable(&path, &report)?;
        if report.status != ArtifactReportStatus::Success {
            return Err(AdmError::new(format!(
                "Artifact preflight failed for stage {step_number:02}: {:?}",
                report.errors
            )));
        }
        Ok(report)
    }

    pub fn write_stage_artifact_manifest(
        &self,
        step_number: u32,
    ) -> AdmResult<ArtifactLayerManifest> {
        let artifacts = self.service.artifacts_for_stage(step_number);
        let stage_path = self.stage_dir(step_number);
        let tasks = artifacts
            .iter()
            .flat_map(|artifact| {
                artifact.tasks.iter().map(|task| {
                    adm_new_contracts::artifact::ArtifactTaskWithArtifact {
                        id: task.id.clone(),
                        task_type: task.task_type.clone(),
                        description: task.description.clone(),
                        artifact_id: artifact.id.clone(),
                    }
                })
            })
            .collect::<Vec<_>>();
        let file_manifest = if stage_path.exists() {
            file_manifest(&stage_path)?
                .into_iter()
                .map(|entry| {
                    json!({
                        "path": entry.path,
                        "size_bytes": entry.size_bytes,
                        "sha256": entry.sha256,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        let manifest = ArtifactLayerManifest {
            step: step_number,
            timestamp: timestamp(),
            stage_dir: stage_path.display().to_string(),
            artifacts,
            tasks,
            file_manifest,
        };
        write_json_serializable(&self.stage_layer_manifest_path(step_number), &manifest)?;
        Ok(manifest)
    }

    pub fn run_review_pipeline(&self, step_number: u32) -> AdmResult<ArtifactReviewReport> {
        let _manifest = self.write_stage_artifact_manifest(step_number)?;
        let evidence = self.evidence_for_stage(step_number);
        let report = self.service.run_review_pipeline(step_number, &evidence);
        write_json_serializable(&self.stage_reviews_path(step_number), &report)?;
        if report.status != ArtifactReportStatus::Success {
            return Err(AdmError::new(format!(
                "Artifact review failed for stage {step_number:02}"
            )));
        }
        Ok(report)
    }

    pub fn run_artifact_validators(
        &self,
        step_number: u32,
    ) -> AdmResult<ArtifactValidationLayerReport> {
        let manifest = self.read_stage_artifact_manifest(step_number)?;
        let review_report = self.read_stage_reviews(step_number)?;
        let evidence = self.evidence_for_stage(step_number);
        let mut report =
            self.service
                .run_artifact_validators(step_number, &manifest, &review_report, &evidence);
        self.apply_schema_contract_results(&mut report);
        write_json_serializable(&self.stage_validation_path(step_number), &report)?;
        self.refresh_reference_manifest_file_inventory(step_number)?;
        if report.status != ArtifactReportStatus::Success {
            return Err(AdmError::new(format!(
                "Artifact validation failed for stage {step_number:02}"
            )));
        }
        Ok(report)
    }

    pub fn refresh_reference_manifest_file_inventory(&self, step_number: u32) -> AdmResult<Value> {
        let stage_path = self.stage_dir(step_number);
        let ref_path = stage_path.join("reference_manifest.json");
        let mut data = object_map_or_empty(read_json(&ref_path, json!({})));
        let files = if stage_path.exists() {
            file_manifest(&stage_path)?
                .into_iter()
                .filter(|entry| entry.path != "reference_manifest.json")
                .map(|entry| {
                    json!({
                        "path": entry.path,
                        "stage_path": rel(&stage_path.join(&entry.path), &self.paths.project_root),
                        "role": classify_stage_file(&entry.path),
                        "size_bytes": entry.size_bytes,
                        "sha256": entry.sha256,
                    })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        data.insert("files".to_string(), Value::Array(files));
        let value = Value::Object(data);
        write_json(&ref_path, &value)?;
        Ok(value)
    }

    pub fn evidence_for_stage(&self, step_number: u32) -> ArtifactEvidenceSet {
        let mut evidence = ArtifactEvidenceSet::default();
        for relative in [
            format!("outputs/artifacts/stage_{step_number:02}/artifact_index.json"),
            format!("outputs/artifacts/stage_{step_number:02}/reference_manifest.json"),
            format!("outputs/artifacts/stage_{step_number:02}/validation_report.json"),
        ] {
            if self.contract_ref_exists(&relative) {
                evidence.existing_paths.insert(normalize_path(&relative));
            }
        }
        for artifact in &self.service.registry.artifacts {
            for knowledge_ref in &artifact.knowledge_refs {
                if self.paths.project_root.join(knowledge_ref).exists() {
                    evidence
                        .existing_paths
                        .insert(normalize_path(knowledge_ref));
                }
            }
            for schema_ref in &artifact.schema_refs {
                if self.paths.project_root.join(&schema_ref.schema).exists() {
                    evidence
                        .existing_paths
                        .insert(normalize_path(&schema_ref.schema));
                }
                if self.resolve_contract_ref(&schema_ref.path).0.is_some() {
                    evidence
                        .existing_paths
                        .insert(normalize_path(&schema_ref.path));
                }
            }
            for dependency in &artifact.depends_on {
                if let Some(dep) = self.service.artifacts_by_id.get(dependency) {
                    let status = self
                        .dependency_validation_status(dep.stage)
                        .unwrap_or(ArtifactReportStatus::Failed);
                    evidence
                        .upstream_validation_status
                        .insert(dependency.clone(), status);
                }
            }
        }
        evidence
    }

    fn apply_schema_contract_results(&self, report: &mut ArtifactValidationLayerReport) {
        let artifacts = self.service.artifacts_by_id();
        for validation in &mut report.validations {
            let Some(artifact) = artifacts.get(&validation.artifact_id) else {
                continue;
            };
            if !artifact
                .validators
                .iter()
                .any(|validator| validator == "schema_contract_validator")
            {
                continue;
            }
            validation
                .results
                .retain(|result| result.name != "schema_contract_validator");
            let mut schema_results = self.run_schema_contract_refs(artifact);
            if schema_results.is_empty() {
                schema_results.push(check(
                    "schema_contract_validator",
                    false,
                    ArtifactSeverity::Error,
                    "schema_contract_validator declared but no schema_refs provided.",
                ));
            }
            validation.results.extend(schema_results);
            validation.status = aggregate_check_status(&validation.results);
        }
        report.status = report_status_from_validations(&report.validations);
    }

    fn run_schema_contract_refs(&self, artifact: &ArtifactContract) -> Vec<ArtifactCheckResult> {
        let mut results = Vec::new();
        for schema_ref in &artifact.schema_refs {
            let (contract_path, checked_paths) = self.resolve_contract_ref(&schema_ref.path);
            let schema_path = self.paths.project_root.join(&schema_ref.schema);
            let Some(contract_path) = contract_path else {
                results.push(check(
                    "schema_contract_validator",
                    false,
                    ArtifactSeverity::Error,
                    &format!(
                        "Contract file missing: {}; checked: {}",
                        schema_ref.path,
                        checked_paths
                            .iter()
                            .map(|path| display_path(path, &self.paths.project_root))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
                continue;
            };
            if !schema_path.exists() {
                results.push(check(
                    "schema_contract_validator",
                    false,
                    ArtifactSeverity::Error,
                    &format!("Schema file missing: {}", schema_ref.schema),
                ));
                continue;
            }
            match schema::validate_contract_file(&contract_path, &schema_path) {
                Ok(errors) if errors.is_empty() => results.push(check(
                    "schema_contract_validator",
                    true,
                    ArtifactSeverity::Error,
                    &format!(
                        "{} matches {} via {}.",
                        schema_ref.path,
                        schema_ref.schema,
                        display_path(&contract_path, &self.paths.project_root)
                    ),
                )),
                Ok(errors) => results.push(check(
                    "schema_contract_validator",
                    false,
                    ArtifactSeverity::Error,
                    &format!(
                        "{} failed {}: {:?}",
                        schema_ref.path,
                        schema_ref.schema,
                        errors.into_iter().take(5).collect::<Vec<_>>()
                    ),
                )),
                Err(error) => results.push(check(
                    "schema_contract_validator",
                    false,
                    ArtifactSeverity::Error,
                    &format!("{} failed {}: {error}", schema_ref.path, schema_ref.schema),
                )),
            }
        }
        results
    }

    fn read_stage_artifact_manifest(&self, step_number: u32) -> AdmResult<ArtifactLayerManifest> {
        let text = fs::read_to_string(self.stage_layer_manifest_path(step_number))?;
        serde_json::from_str(&text)
            .map_err(|error| AdmError::new(format!("failed to parse artifact manifest: {error}")))
    }

    fn read_stage_reviews(&self, step_number: u32) -> AdmResult<ArtifactReviewReport> {
        let text = fs::read_to_string(self.stage_reviews_path(step_number))?;
        serde_json::from_str(&text)
            .map_err(|error| AdmError::new(format!("failed to parse artifact reviews: {error}")))
    }

    fn stage_layer_manifest_path(&self, step_number: u32) -> PathBuf {
        self.stage_dir(step_number)
            .join("artifact_layer_manifest.json")
    }

    fn stage_reviews_path(&self, step_number: u32) -> PathBuf {
        self.stage_dir(step_number).join("artifact_reviews.json")
    }

    fn stage_validation_path(&self, step_number: u32) -> PathBuf {
        self.stage_dir(step_number)
            .join("artifact_validation_layer.json")
    }

    fn dependency_validation_status(&self, step_number: u32) -> Option<ArtifactReportStatus> {
        let value = read_json(&self.stage_validation_path(step_number), json!({}));
        match value.get("status").and_then(Value::as_str) {
            Some("success") => Some(ArtifactReportStatus::Success),
            Some("failed") => Some(ArtifactReportStatus::Failed),
            _ => None,
        }
    }

    fn contract_ref_exists(&self, path_text: &str) -> bool {
        self.resolve_contract_ref(path_text).0.is_some()
    }

    fn resolve_contract_ref(&self, path_text: &str) -> (Option<PathBuf>, Vec<PathBuf>) {
        let candidates = self.contract_ref_candidates(path_text);
        for candidate in &candidates {
            if candidate.exists() {
                return (Some(candidate.clone()), candidates);
            }
        }
        (None, candidates)
    }

    fn contract_ref_candidates(&self, path_text: &str) -> Vec<PathBuf> {
        let path = PathBuf::from(path_text);
        if path.is_absolute() {
            return vec![path];
        }
        let normalized = normalize_path(path_text);
        let mut candidates = Vec::new();
        if normalized.starts_with("outputs/") {
            candidates.push(self.paths.draft_dir.join(&normalized));
            if let Some(save_id) = self.current_save_id() {
                candidates.push(
                    self.paths
                        .saves_dir
                        .join(save_id)
                        .join("workspace")
                        .join(&normalized),
                );
            }
        }
        candidates.push(self.paths.project_root.join(&normalized));
        dedupe_paths(candidates)
    }

    fn current_save_id(&self) -> Option<String> {
        read_json(&self.paths.saves_dir.join("save_index.json"), json!({}))
            .get("current_save_id")
            .and_then(Value::as_str)
            .map(str::to_string)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionedDirMeta {
    pub project: String,
    pub stage: String,
    pub date: String,
    pub version: u32,
    pub name: String,
    pub path: String,
}

pub fn parse_versioned_dir(path: impl AsRef<Path>) -> VersionedDirMeta {
    let path = path.as_ref();
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string();
    let parsed = name.rsplit_once("_v").and_then(|(left, version)| {
        let version = version.parse::<u32>().ok()?;
        let (left, date) = left.rsplit_once('_')?;
        if date.len() != 8 || !date.chars().all(|ch| ch.is_ascii_digit()) {
            return None;
        }
        let (project, stage) = left.rsplit_once('_')?;
        Some((
            project.to_string(),
            stage.to_string(),
            date.to_string(),
            version,
        ))
    });
    match parsed {
        Some((project, stage, date, version)) => VersionedDirMeta {
            project,
            stage,
            date,
            version,
            name,
            path: path.display().to_string(),
        },
        None => VersionedDirMeta {
            project: String::new(),
            stage: String::new(),
            date: String::new(),
            version: 0,
            name,
            path: path.display().to_string(),
        },
    }
}

pub fn sha256_file(path: impl AsRef<Path>) -> AdmResult<String> {
    let bytes = fs::read(path)?;
    Ok(sha256_hex(&bytes))
}

pub fn file_entry(
    base_dir: impl AsRef<Path>,
    relative_path: &str,
    role: &str,
    status: &str,
) -> Value {
    let base_dir = base_dir.as_ref();
    let normalized = normalize_path(relative_path);
    let path = base_dir.join(&normalized);
    let mut entry = Map::new();
    entry.insert("path".to_string(), Value::String(normalized));
    entry.insert("role".to_string(), Value::String(role.to_string()));
    entry.insert("status".to_string(), Value::String(status.to_string()));
    entry.insert("exists".to_string(), Value::Bool(path.exists()));
    if path.is_file()
        && let Ok(bytes) = fs::read(&path)
    {
        entry.insert("bytes".to_string(), json!(bytes.len()));
        entry.insert("sha256".to_string(), Value::String(sha256_hex(&bytes)));
    }
    Value::Object(entry)
}

pub fn path_ref(path: Option<&Path>, root: Option<&Path>) -> String {
    let Some(path) = path else {
        return String::new();
    };
    if let Some(root) = root
        && let Ok(relative) = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf())
            .strip_prefix(root.canonicalize().unwrap_or_else(|_| root.to_path_buf()))
    {
        return relative.to_string_lossy().replace('\\', "/");
    }
    path.to_string_lossy().replace('\\', "/")
}

pub fn write_artifact_manifest(
    output_dir: impl AsRef<Path>,
    stage: &str,
    mode: &str,
    files: Vec<Value>,
    upstream: Option<Value>,
    patch: Option<Value>,
    extra: Option<Value>,
) -> AdmResult<PathBuf> {
    let output_dir = output_dir.as_ref();
    let meta = parse_versioned_dir(output_dir);
    let mut manifest = Map::new();
    manifest.insert("project".to_string(), Value::String(meta.project));
    manifest.insert("stage".to_string(), Value::String(stage.to_string()));
    manifest.insert("version".to_string(), json!(meta.version));
    manifest.insert("artifact_dir".to_string(), Value::String(meta.name));
    manifest.insert("mode".to_string(), Value::String(mode.to_string()));
    manifest.insert("generated_at".to_string(), Value::String(timestamp()));
    manifest.insert("files".to_string(), Value::Array(files));
    if let Some(upstream) = upstream {
        manifest.insert("upstream".to_string(), upstream);
    }
    if let Some(patch) = patch {
        manifest.insert("patch".to_string(), patch);
    }
    if let Some(Value::Object(extra)) = extra {
        for (key, value) in extra {
            manifest.insert(key, value);
        }
    }
    let path = output_dir.join("artifact_manifest.json");
    write_json(&path, &Value::Object(manifest))?;
    Ok(path)
}

fn stage_files_exist(stage: u32, evidence: &ArtifactEvidenceSet) -> bool {
    evidence.path_exists(&format!(
        "outputs/artifacts/stage_{stage:02}/artifact_index.json"
    )) && evidence.path_exists(&format!(
        "outputs/artifacts/stage_{stage:02}/reference_manifest.json"
    ))
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn classify_stage_file(path_text: &str) -> String {
    let normalized = path_text.replace('\\', "/");
    let name = Path::new(&normalized)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if normalized.starts_with("guidance/") {
        "guidance"
    } else if normalized.starts_with("imported/") {
        "source_import"
    } else if normalized.starts_with("upstream/") {
        "upstream_reference"
    } else if matches!(name, "artifact_index.json" | "reference_manifest.json") {
        "stage_index"
    } else if matches!(
        name,
        "validation_report.json" | "artifact_reviews.json" | "artifact_validation_layer.json"
    ) {
        "validation"
    } else if name == "artifact_layer_manifest.json" {
        "artifact_layer"
    } else if name == "README.md" || name.starts_with("MISSING_") || name.starts_with("OPTIONAL_") {
        "operator_report"
    } else {
        "stage_file"
    }
    .to_string()
}

fn object_map_or_empty(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => value.to_string(),
    }
}

fn display_path(path: &Path, root: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .strip_prefix(root.canonicalize().unwrap_or_else(|_| root.to_path_buf()))
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"))
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for path in paths {
        let key = path.to_string_lossy().replace('\\', "/");
        if seen.insert(key) {
            result.push(path);
        }
    }
    result
}

fn timestamp() -> String {
    format!("unix:{}", unix_timestamp())
}

fn check(
    name: &str,
    condition: bool,
    severity_when_failed: ArtifactSeverity,
    message: &str,
) -> ArtifactCheckResult {
    ArtifactCheckResult {
        name: name.to_string(),
        status: if condition {
            ArtifactCheckStatus::Pass
        } else {
            ArtifactCheckStatus::Fail
        },
        severity: if condition {
            ArtifactSeverity::Info
        } else {
            severity_when_failed
        },
        message: message.to_string(),
    }
}

fn aggregate_check_status(results: &[ArtifactCheckResult]) -> ArtifactCheckStatus {
    if results.iter().all(|result| {
        result.status == ArtifactCheckStatus::Pass && result.severity == ArtifactSeverity::Info
    }) {
        ArtifactCheckStatus::Pass
    } else {
        ArtifactCheckStatus::Fail
    }
}

fn report_status(ok: bool, warnings: &[String]) -> ArtifactReportStatus {
    if ok && warnings.is_empty() {
        ArtifactReportStatus::Success
    } else {
        ArtifactReportStatus::Failed
    }
}

fn report_status_from_reviews(reviews: &[ArtifactReview]) -> ArtifactReportStatus {
    if reviews
        .iter()
        .all(|review| review.status == ArtifactCheckStatus::Pass)
    {
        ArtifactReportStatus::Success
    } else {
        ArtifactReportStatus::Failed
    }
}

fn report_status_from_validations(validations: &[ArtifactValidation]) -> ArtifactReportStatus {
    if validations
        .iter()
        .all(|validation| validation.status == ArtifactCheckStatus::Pass)
    {
        ArtifactReportStatus::Success
    } else {
        ArtifactReportStatus::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::artifact::{
        ArtifactTask, ArtifactTaskWithArtifact, REVIEWER_WHITELIST, SchemaRef, VALIDATOR_WHITELIST,
    };
    use adm_new_foundation::new_stable_id;

    #[test]
    fn crate_reports_ready() {
        assert!(crate_ready());
        assert_eq!(CRATE_NAME, "adm-new-artifact");
    }

    #[test]
    fn artifact_archive_fallback_reads_canonical_save_index() {
        let root = temp_root("save_index");
        let service =
            ArtifactFileService::with_registry(&root, "session_a", sample_registry()).unwrap();
        fs::create_dir_all(root.join("saves/save_canonical/workspace/outputs/artifacts")).unwrap();
        fs::write(
            root.join("saves/save_canonical/workspace/outputs/artifacts/result.json"),
            "{}",
        )
        .unwrap();
        fs::write(
            root.join("saves/save_index.json"),
            r#"{"current_save_id":"save_canonical"}"#,
        )
        .unwrap();
        fs::write(
            root.join("saves/index.json"),
            r#"{"current_save_id":"save_legacy_wrong"}"#,
        )
        .unwrap();

        let (resolved, _) = service.resolve_contract_ref("outputs/artifacts/result.json");
        assert_eq!(
            resolved.unwrap(),
            root.join("saves/save_canonical/workspace/outputs/artifacts/result.json")
        );
        cleanup(root);
    }

    #[test]
    fn artifact_dependency_graph_uses_registry_and_detects_errors() {
        let service = ArtifactService::new(sample_registry()).unwrap();
        let graph = service.build_dependency_graph();
        assert!(graph.errors.is_empty(), "{:?}", graph.errors);
        assert_eq!(
            graph.topological_order,
            vec![
                "stage_00.concept_bundle".to_string(),
                "stage_01.framework_bundle".to_string()
            ]
        );
        assert_eq!(graph.edges.len(), 1);

        let unknown = ArtifactService::new(ArtifactRegistry {
            artifacts: vec![artifact("stage_01.framework_bundle", 1, vec!["missing"])],
            ..registry_base()
        })
        .unwrap();
        let graph = unknown.build_dependency_graph();
        assert!(!graph.errors.is_empty());
    }

    #[test]
    fn artifact_preflight_blocks_missing_schema_refs_unknown_reviewer_and_upstream_failure() {
        let mut missing_schema = artifact(
            "stage_01.framework_bundle",
            1,
            vec!["stage_00.concept_bundle"],
        );
        missing_schema.schema_refs.clear();
        let service = ArtifactService::new(ArtifactRegistry {
            artifacts: vec![
                artifact("stage_00.concept_bundle", 0, vec![]),
                missing_schema,
            ],
            ..registry_base()
        })
        .unwrap();
        let failed = service.preflight_stage_contract(1, &full_evidence(false));
        assert_eq!(failed.status, ArtifactReportStatus::Failed);
        assert!(
            failed
                .errors
                .iter()
                .any(|error| error.contains("has no schema_refs"))
        );
        assert!(
            failed
                .errors
                .iter()
                .any(|error| error.contains("validation is not success"))
        );

        let mut unknown_reviewer = artifact("stage_01.framework_bundle", 1, vec![]);
        unknown_reviewer.reviewers.push("fake_reviewer".to_string());
        let service = ArtifactService::new(ArtifactRegistry {
            artifacts: vec![unknown_reviewer],
            ..registry_base()
        })
        .unwrap();
        let failed = service.preflight_stage_contract(1, &full_evidence(true));
        assert!(
            failed
                .errors
                .iter()
                .any(|error| error.contains("unknown reviewer"))
        );
    }

    #[test]
    fn artifact_review_and_validation_succeed_with_required_evidence() {
        let service = ArtifactService::new(sample_registry()).unwrap();
        let evidence = full_evidence(true);
        let preflight = service.preflight_stage_contract(1, &evidence);
        assert_eq!(preflight.status, ArtifactReportStatus::Success);
        let review = service.run_review_pipeline(1, &evidence);
        assert_eq!(review.status, ArtifactReportStatus::Success);
        let manifest = manifest_for_stage(1, service.artifacts_for_stage(1));
        let validation = service.run_artifact_validators(1, &manifest, &review, &evidence);
        assert_eq!(validation.status, ArtifactReportStatus::Success);
    }

    #[test]
    fn artifact_validation_fails_when_review_failed_or_schema_target_missing() {
        let service = ArtifactService::new(sample_registry()).unwrap();
        let mut evidence = full_evidence(true);
        evidence
            .existing_paths
            .remove("outputs/artifacts/stage_01/framework_bundle.json");
        let review = ArtifactReviewReport {
            step: 1,
            timestamp: timestamp(),
            status: ArtifactReportStatus::Failed,
            phase: "review".to_string(),
            reviews: Vec::new(),
        };
        let manifest = manifest_for_stage(1, service.artifacts_for_stage(1));
        let validation = service.run_artifact_validators(1, &manifest, &review, &evidence);
        assert_eq!(validation.status, ArtifactReportStatus::Failed);
        assert!(validation.validations[0].results.iter().any(|result| {
            result.name == "review_report_validator" && result.status == ArtifactCheckStatus::Fail
        }));
        assert!(validation.validations[0].results.iter().any(|result| {
            result.name == "schema_contract_validator" && result.status == ArtifactCheckStatus::Fail
        }));
    }

    #[test]
    fn artifact_warning_result_is_not_success() {
        let results = vec![ArtifactCheckResult {
            name: "warning_validator".to_string(),
            status: ArtifactCheckStatus::Pass,
            severity: ArtifactSeverity::Warning,
            message: "warning must not count as success".to_string(),
        }];
        assert_eq!(aggregate_check_status(&results), ArtifactCheckStatus::Fail);
    }

    #[test]
    fn artifact_registry_loader_graph_and_manifest_helpers_use_python_paths() {
        let root = temp_root("registry");
        let registry_path = root.join(REGISTRY_RELATIVE_PATH);
        write_json_serializable(&registry_path, &sample_registry()).unwrap();

        let loaded = ArtifactService::load_registry(&root).unwrap();
        let service = ArtifactService::new(loaded).unwrap();
        assert_eq!(service.topological_step_order(0, 1).unwrap(), vec![0, 1]);

        let file_service =
            ArtifactFileService::with_registry(&root, "session_a", sample_registry()).unwrap();
        let graph = file_service.emit_dependency_graph().unwrap();
        assert_eq!(graph["version"], 1);
        assert!(file_service.graph_path().exists());
        assert!(file_service.output_graph_path().exists());

        let output_dir = root.join("devflow_Design_20260709_v3");
        fs::create_dir_all(&output_dir).unwrap();
        fs::write(output_dir.join("payload.json"), "{\"name\":\"demo\"}").unwrap();
        let meta = parse_versioned_dir(&output_dir);
        assert_eq!(meta.project, "devflow");
        assert_eq!(meta.stage, "Design");
        assert_eq!(meta.version, 3);
        let entry = file_entry(&output_dir, "payload.json", "contract", "generated");
        assert_eq!(entry["exists"], true);
        assert_eq!(
            sha256_file(output_dir.join("payload.json")).unwrap(),
            entry["sha256"].as_str().unwrap()
        );
        assert_eq!(
            path_ref(Some(&output_dir.join("payload.json")), Some(&root)),
            "devflow_Design_20260709_v3/payload.json"
        );
        let manifest_path = write_artifact_manifest(
            &output_dir,
            "Design",
            "generated",
            vec![entry],
            Some(json!({"source": "stage_02"})),
            None,
            Some(json!({"extra_field": true})),
        )
        .unwrap();
        let manifest = read_json(&manifest_path, json!({}));
        assert_eq!(manifest["artifact_dir"], "devflow_Design_20260709_v3");
        assert_eq!(manifest["extra_field"], true);
        cleanup(root);
    }

    #[test]
    fn artifact_file_service_writes_preflight_review_validation_layer_files() {
        let root = temp_root("file_service");
        let file_service =
            ArtifactFileService::with_registry(&root, "session_a", sample_registry()).unwrap();
        fs::create_dir_all(root.join("knowledge/schemas")).unwrap();
        fs::write(root.join("knowledge/Core_Rules.md"), "rules").unwrap();
        fs::write(
            root.join("knowledge/schemas/framework_bundle.schema.json"),
            r#"{"type":"object","required":["name"],"properties":{"name":{"type":"string"}}}"#,
        )
        .unwrap();
        fs::write(
            root.join("knowledge/schemas/concept_bundle.schema.json"),
            r#"{"type":"object"}"#,
        )
        .unwrap();

        let stage0 = file_service.stage_dir(0);
        fs::create_dir_all(&stage0).unwrap();
        write_json(
            &stage0.join("artifact_validation_layer.json"),
            &json!({"status": "success"}),
        )
        .unwrap();

        let stage1 = file_service.stage_dir(1);
        fs::create_dir_all(&stage1).unwrap();
        write_json(
            &stage1.join("artifact_index.json"),
            &json!({"manifest": []}),
        )
        .unwrap();
        write_json(
            &stage1.join("reference_manifest.json"),
            &json!({"files": []}),
        )
        .unwrap();
        write_json(
            &stage1.join("validation_report.json"),
            &json!({"status": "success", "valid": true, "imported_sources": [{"group": "framework"}]}),
        )
        .unwrap();
        write_json(
            &stage1.join("framework_bundle.json"),
            &json!({"name": "demo"}),
        )
        .unwrap();

        let preflight = file_service.preflight_stage_contract(1).unwrap();
        assert_eq!(preflight.status, ArtifactReportStatus::Success);
        assert!(
            file_service
                .layer_output_dir()
                .join("preflight_stage_01.json")
                .exists()
        );
        let review = file_service.run_review_pipeline(1).unwrap();
        assert_eq!(review.status, ArtifactReportStatus::Success);
        assert!(stage1.join("artifact_layer_manifest.json").exists());
        assert!(stage1.join("artifact_reviews.json").exists());
        let validation = file_service.run_artifact_validators(1).unwrap();
        assert_eq!(validation.status, ArtifactReportStatus::Success);
        assert!(stage1.join("artifact_validation_layer.json").exists());
        let reference = read_json(&stage1.join("reference_manifest.json"), json!({}));
        assert!(reference["files"].as_array().unwrap().iter().any(|entry| {
            entry["path"] == "artifact_validation_layer.json" && entry["role"] == "validation"
        }));
        cleanup(root);
    }

    fn registry_base() -> ArtifactRegistry {
        ArtifactRegistry {
            version: 1,
            description: "test registry".to_string(),
            default_reviewers: REVIEWER_WHITELIST
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            default_validators: VALIDATOR_WHITELIST
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            artifacts: Vec::new(),
        }
    }

    fn sample_registry() -> ArtifactRegistry {
        ArtifactRegistry {
            artifacts: vec![
                artifact("stage_00.concept_bundle", 0, vec![]),
                artifact(
                    "stage_01.framework_bundle",
                    1,
                    vec!["stage_00.concept_bundle"],
                ),
            ],
            ..registry_base()
        }
    }

    fn artifact(id: &str, stage: u32, depends_on: Vec<&str>) -> ArtifactContract {
        ArtifactContract {
            id: id.to_string(),
            stage,
            kind: "source_placeholder_or_import".to_string(),
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            tasks: vec![ArtifactTask {
                id: format!("{id}.import"),
                task_type: "import".to_string(),
                description: "Import source".to_string(),
            }],
            reviewers: REVIEWER_WHITELIST
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            validators: VALIDATOR_WHITELIST
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            schema_refs: vec![SchemaRef {
                path: format!("outputs/artifacts/stage_{stage:02}/{}.json", short_id(id)),
                schema: format!("knowledge/schemas/{}.schema.json", short_id(id)),
                description: String::new(),
            }],
            knowledge_refs: vec!["knowledge/Core_Rules.md".to_string()],
            extra: BTreeMap::new(),
        }
    }

    fn full_evidence(upstream_success: bool) -> ArtifactEvidenceSet {
        let mut evidence = ArtifactEvidenceSet::with_paths([
            "knowledge/Core_Rules.md",
            "knowledge/schemas/concept_bundle.schema.json",
            "knowledge/schemas/framework_bundle.schema.json",
            "outputs/artifacts/stage_00/artifact_index.json",
            "outputs/artifacts/stage_00/reference_manifest.json",
            "outputs/artifacts/stage_00/validation_report.json",
            "outputs/artifacts/stage_00/concept_bundle.json",
            "outputs/artifacts/stage_01/artifact_index.json",
            "outputs/artifacts/stage_01/reference_manifest.json",
            "outputs/artifacts/stage_01/validation_report.json",
            "outputs/artifacts/stage_01/framework_bundle.json",
        ]);
        if upstream_success {
            evidence.upstream_validation_status.insert(
                "stage_00.concept_bundle".to_string(),
                ArtifactReportStatus::Success,
            );
        } else {
            evidence.upstream_validation_status.insert(
                "stage_00.concept_bundle".to_string(),
                ArtifactReportStatus::Failed,
            );
        }
        evidence
    }

    fn manifest_for_stage(stage: u32, artifacts: Vec<ArtifactContract>) -> ArtifactLayerManifest {
        let tasks = artifacts
            .iter()
            .flat_map(|artifact| {
                artifact.tasks.iter().map(|task| ArtifactTaskWithArtifact {
                    id: task.id.clone(),
                    task_type: task.task_type.clone(),
                    description: task.description.clone(),
                    artifact_id: artifact.id.clone(),
                })
            })
            .collect();
        ArtifactLayerManifest {
            step: stage,
            timestamp: timestamp(),
            stage_dir: format!("outputs/artifacts/stage_{stage:02}"),
            artifacts,
            tasks,
            file_manifest: Vec::new(),
        }
    }

    fn short_id(id: &str) -> &str {
        id.rsplit('.').next().unwrap_or(id)
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm_new_artifact_{label}_{}",
            new_stable_id("root").unwrap()
        ))
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
