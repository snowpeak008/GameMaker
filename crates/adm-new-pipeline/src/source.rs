use adm_new_contracts::ArtifactLocale;
use adm_new_foundation::io::{now_iso, read_json, rel, write_json, write_text};
use adm_new_foundation::paths::{ProjectPaths, relative_display};
use adm_new_foundation::{
    AdmError, AdmResult, file_manifest, sanitize_identifier, sha256_hex, unix_timestamp,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const SOURCE_TYPES: &[&str] = &[
    "Concept",
    "GameplayFramework",
    "SubsystemDesign",
    "AIDesignScript",
    "Design",
    "DevelopmentDesign",
    "ProgReq",
    "ArtReq",
    "ProgReview",
    "ArtReview",
    "Plans",
    "ArtPlans",
    "Alignment",
    "DevExecution",
    "ArtProduction",
    "SceneAssembly",
    "Integration",
];

pub const SOURCE_MARKERS: &[(&str, &str)] = &[
    ("selected_play_prototype.json", "Concept"),
    ("gameplay_framework.json", "GameplayFramework"),
    ("approved_subsystems.json", "SubsystemDesign"),
    ("ai_design_script.json", "AIDesignScript"),
    ("frozen_game_design.md", "Design"),
    ("development_system_design.md", "DevelopmentDesign"),
    ("program_requirements_contract.json", "ProgReq"),
    ("art_requirements_contract.json", "ArtReq"),
    ("ProgReview_report.json", "ProgReview"),
    ("ArtReview_report.json", "ArtReview"),
    ("program_plan_index.md", "Plans"),
    ("art_plan_index.md", "ArtPlans"),
    ("AlignmentProtocol.md", "Alignment"),
    ("devexecution.json", "DevExecution"),
    ("artproduction.json", "ArtProduction"),
    ("sceneassembly.json", "SceneAssembly"),
    ("integration.json", "Integration"),
];

pub const DEFAULT_PROJECT_NAME: &str = "devflow";
pub const RUN_CONTEXT_ENV: &str = "AUTODESIGNMAKER_RUN_CONTEXT_FILE";
pub const ALLOW_CROSS_DRAFT_SOURCE_FALLBACK_ENV: &str =
    "AUTODESIGNMAKER_ALLOW_CROSS_DRAFT_SOURCE_FALLBACK";

#[derive(Debug, Clone)]
pub struct SourceService {
    paths: ProjectPaths,
}

impl SourceService {
    pub fn new(root: impl AsRef<Path>, session_id: &str) -> AdmResult<Self> {
        let session_id = sanitize_identifier(session_id)?;
        let paths = ProjectPaths::new(root.as_ref(), session_id);
        paths.ensure_current_draft_dirs()?;
        Ok(Self { paths })
    }

    pub fn paths(&self) -> &ProjectPaths {
        &self.paths
    }

    pub fn source_artifact_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();
        let mut seen = BTreeSet::new();
        let allow_cross_draft_fallback = std::env::var(ALLOW_CROSS_DRAFT_SOURCE_FALLBACK_ENV)
            .ok()
            .is_some_and(|value| value.trim() == "1");

        if let Some(context_root) = self
            .load_run_context_value()
            .and_then(|value| string_field(&value, "source_artifacts_root"))
            .filter(|value| !value.is_empty())
            .and_then(|value| resolve_project_owned_path(&self.paths.project_root, &value))
        {
            push_unique_root(&mut roots, &mut seen, context_root);
            if !allow_cross_draft_fallback {
                return roots;
            }
        } else {
            push_unique_root(
                &mut roots,
                &mut seen,
                self.paths.source_artifacts_dir.clone(),
            );
        }

        if self.paths.drafts_dir.is_dir() {
            let mut draft_roots = Vec::new();
            if let Ok(entries) = fs::read_dir(&self.paths.drafts_dir) {
                for entry in entries.flatten() {
                    let root = entry.path().join("source_artifacts");
                    if root.is_dir() {
                        draft_roots.push(root);
                    }
                }
            }
            draft_roots.sort_by(|left, right| mtime_secs(right).cmp(&mtime_secs(left)));
            for root in draft_roots {
                push_unique_root(&mut roots, &mut seen, root);
            }
        }

        let legacy = self
            .paths
            .project_root
            .join("sandbox")
            .join("source_artifacts");
        if legacy.is_dir() {
            push_unique_root(&mut roots, &mut seen, legacy);
        }
        roots
    }

    pub fn source_package_metadata(&self, path: impl AsRef<Path>) -> Value {
        source_package_metadata(path)
    }

    pub fn infer_source_ids(&self, path: impl AsRef<Path>) -> Vec<String> {
        infer_source_ids(path)
    }

    pub fn find_sources(
        &self,
        patterns: &[String],
        mode: &str,
        source_ids: &[String],
    ) -> AdmResult<Vec<PathBuf>> {
        let expected_ids = if source_ids.is_empty() {
            source_ids_from_patterns(patterns)
        } else {
            source_ids.to_vec()
        };
        let mut found = BTreeMap::<PathBuf, PathBuf>::new();
        for root in self.source_artifact_roots() {
            if !root.exists() {
                continue;
            }
            let mut root_found = BTreeMap::<PathBuf, PathBuf>::new();
            if !expected_ids.is_empty() {
                for entry in fs::read_dir(&root)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() && source_matches_ids(&path, &expected_ids) {
                        root_found.insert(canonical_or_self(&path), path);
                    }
                }
            }
            if root_found.is_empty() {
                for pattern in patterns {
                    for path in matching_child_dirs(&root, pattern)? {
                        root_found.insert(canonical_or_self(&path), path);
                    }
                }
            }
            if !root_found.is_empty() {
                found = root_found;
                break;
            }
        }
        let mut ordered = found.into_values().collect::<Vec<_>>();
        ordered.sort_by(source_sort_cmp);
        match mode {
            "all" => Ok(ordered),
            "latest" => Ok(ordered.into_iter().rev().take(1).collect()),
            other => Err(AdmError::new(format!(
                "Unknown source selection mode: {other}"
            ))),
        }
    }

    pub fn find_latest(
        &self,
        prefix: &str,
        project: &str,
        include_legacy: bool,
    ) -> Option<PathBuf> {
        let mut candidates = Vec::new();
        for pattern in stage_globs(prefix, project, include_legacy) {
            if let Ok(paths) = matching_child_dirs(&self.paths.source_artifacts_dir, &pattern) {
                candidates.extend(paths);
            }
        }
        candidates.sort_by(|left, right| {
            parse_version_name(&right.file_name_text())
                .cmp(&parse_version_name(&left.file_name_text()))
        });
        candidates.into_iter().next()
    }

    pub fn find_latest_design(&self, project: &str) -> Option<PathBuf> {
        self.find_latest("Design", project, true)
    }

    pub fn find_latest_idea(&self, project: &str) -> Option<PathBuf> {
        self.find_latest("Idea", project, true)
    }

    pub fn find_latest_prog_req(&self, project: &str) -> Option<PathBuf> {
        self.find_latest("ProgReq", project, true)
    }

    pub fn make_folder(&self, prefix: &str, project: &str) -> AdmResult<PathBuf> {
        let project = sanitize_project_name(project);
        let latest = self.find_latest(prefix, &project, false);
        let next_version = latest
            .as_ref()
            .map(|path| parse_version_name(&path.file_name_text()) + 1)
            .unwrap_or(1);
        let folder = self.paths.source_artifacts_dir.join(format!(
            "{project}_{prefix}_{}_v{next_version}",
            date_stamp()
        ));
        fs::create_dir_all(&folder)?;
        Ok(folder)
    }

    pub fn make_correction_folder(&self) -> AdmResult<PathBuf> {
        let folder = self
            .paths
            .source_artifacts_dir
            .join(format!("Correction_{}", datetime_stamp()));
        fs::create_dir_all(&folder)?;
        Ok(folder)
    }

    pub fn list_temp_folders(&self) -> Vec<PathBuf> {
        let mut folders = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.paths.source_artifacts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && is_temp_folder(&path) {
                    folders.push(path);
                }
            }
        }
        folders.sort();
        folders
    }

    pub fn merge_correction_to_permanent(
        &self,
        correction_dir: impl AsRef<Path>,
        target_stage: &str,
        project: &str,
    ) -> AdmResult<PathBuf> {
        let project = sanitize_project_name(project);
        let latest = self.find_latest(target_stage, &project, true);
        let next_version = latest
            .as_ref()
            .map(|path| parse_version_name(&path.file_name_text()) + 1)
            .unwrap_or(1);
        let target = self.paths.source_artifacts_dir.join(format!(
            "{project}_{target_stage}_{}_v{next_version}",
            date_stamp()
        ));
        fs::create_dir_all(&target)?;
        if let Some(latest) = latest {
            copy_tree_contents(&latest, &target, &BTreeSet::new())?;
        }
        let correction_dir = correction_dir.as_ref();
        if correction_dir.exists() {
            copy_tree_overlay(correction_dir, &target)?;
        }
        Ok(target)
    }

    pub fn cleanup_temp_folders(&self) -> Vec<PathBuf> {
        let mut removed = Vec::new();
        for folder in self.list_temp_folders() {
            if fs::remove_dir_all(&folder).is_ok() {
                removed.push(folder);
            }
        }
        removed
    }

    pub fn resolve_design_path(&self, explicit: Option<&str>, project: &str) -> AdmResult<PathBuf> {
        if let Some(explicit) = explicit.map(str::trim).filter(|value| !value.is_empty()) {
            let path = PathBuf::from(explicit);
            if path.is_dir() {
                let design = path.join("frozen_game_design.md");
                if design.exists() {
                    return Ok(design);
                }
            } else if path.exists() {
                return Ok(path);
            }
            return Err(AdmError::new(format!(
                "specified path does not exist: {explicit}"
            )));
        }
        if let Some(latest) = self.find_latest_design(project) {
            let design = latest.join("frozen_game_design.md");
            if design.exists() {
                return Ok(design);
            }
        }
        Err(AdmError::new(
            "current project design document was not found",
        ))
    }

    pub fn registry_artifacts(&self) -> Vec<Value> {
        let registry = read_json(
            &self
                .paths
                .project_root
                .join("pipeline")
                .join("artifact_layer")
                .join("registry.json"),
            json!({}),
        );
        registry
            .get("artifacts")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.is_object())
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    pub fn upstream_artifacts_for_step(&self, step_number: u32) -> Vec<Value> {
        let artifacts = self.registry_artifacts();
        let by_id = artifacts
            .iter()
            .filter_map(|artifact| Some((string_field(artifact, "id")?, artifact.clone())))
            .collect::<BTreeMap<_, _>>();
        let mut upstream = BTreeMap::new();
        for artifact in artifacts {
            if number_field(&artifact, "stage") != Some(step_number) {
                continue;
            }
            for dep_id in artifact
                .get("depends_on")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let dep_id = value_string(dep_id);
                if let Some(dep) = by_id.get(&dep_id) {
                    upstream.insert(dep_id, dep.clone());
                }
            }
        }
        upstream.into_values().collect()
    }

    pub fn current_artifacts_for_step(&self, step_number: u32) -> Vec<Value> {
        self.registry_artifacts()
            .into_iter()
            .filter(|artifact| number_field(artifact, "stage") == Some(step_number))
            .collect()
    }

    pub fn stage_dir(&self, step_number: u32) -> PathBuf {
        self.paths
            .artifacts_dir
            .join(format!("stage_{step_number:02}"))
    }

    pub fn reset_stage(&self, step_number: u32) -> AdmResult<PathBuf> {
        fs::create_dir_all(&self.paths.artifacts_dir)?;
        let path = self.stage_dir(step_number);
        safe_reset_dir(&path, &self.paths.artifacts_dir)?;
        Ok(path)
    }

    pub fn import_upstream_artifacts(
        &self,
        step_number: u32,
        out_dir: &Path,
    ) -> AdmResult<(Vec<ImportedUpstreamArtifactRecord>, Vec<String>)> {
        let mut imported = Vec::new();
        let mut missing = Vec::new();
        for artifact in self.upstream_artifacts_for_step(step_number) {
            let artifact_id =
                string_field(&artifact, "id").unwrap_or_else(|| "unknown".to_string());
            let upstream_stage = number_field(&artifact, "stage").unwrap_or(0);
            let source_dir = self.stage_dir(upstream_stage);
            let validation_path = source_dir.join("artifact_validation_layer.json");
            let validation = read_json(&validation_path, json!({}));
            if !source_dir.exists() {
                missing.push(format!(
                    "{artifact_id}: missing {}",
                    rel(&source_dir, &self.paths.project_root)
                ));
                continue;
            }
            if validation.get("status").and_then(Value::as_str) != Some("success") {
                let status = validation
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("missing");
                missing.push(format!("{artifact_id}: artifact validation is {status}"));
                continue;
            }
            let target = out_dir
                .join("upstream")
                .join(format!("stage_{upstream_stage:02}"))
                .join(artifact_id.replace('.', "_"));
            let upstream_files = self.upstream_file_refs(&source_dir)?;
            let reference_manifest_path = source_dir.join("reference_manifest.json");
            let artifact_index_path = source_dir.join("artifact_index.json");
            let validation_report_path = source_dir.join("validation_report.json");
            write_json(
                &target.join("UPSTREAM_REFERENCE.json"),
                &json!({
                    "artifact_id": artifact_id,
                    "stage": upstream_stage,
                    "source": rel(&source_dir, &self.paths.project_root),
                    "reference_manifest": rel_if_exists(&reference_manifest_path, &self.paths.project_root),
                    "artifact_index": rel_if_exists(&artifact_index_path, &self.paths.project_root),
                    "validation_report": rel_if_exists(&validation_report_path, &self.paths.project_root),
                    "file_count": upstream_files.len(),
                    "files": upstream_files,
                    "note": "Files are referenced by manifest and are not copied into this stage.",
                }),
            )?;
            imported.push(ImportedUpstreamArtifactRecord {
                artifact_id,
                stage: upstream_stage.to_string(),
                source: rel(&source_dir, &self.paths.project_root),
                target: rel(
                    &target.join("UPSTREAM_REFERENCE.json"),
                    &self.paths.project_root,
                ),
                reference_dir: rel(&target, &self.paths.project_root),
                reference_manifest: rel_if_exists(
                    &reference_manifest_path,
                    &self.paths.project_root,
                ),
                artifact_index: rel_if_exists(&artifact_index_path, &self.paths.project_root),
                validation_report: rel_if_exists(&validation_report_path, &self.paths.project_root),
                file_count: upstream_files.len(),
            });
        }
        Ok((imported, missing))
    }

    pub fn run_import_step(
        &self,
        step_number: u32,
        groups: &[SourceGroup],
        notes: &[String],
    ) -> AdmResult<SourceImportReport> {
        let spec = step_spec(step_number)?;
        let out_dir = self.reset_stage(step_number)?;
        let (imported_upstream_artifacts, missing_upstream_artifacts) =
            self.import_upstream_artifacts(step_number, &out_dir)?;
        let has_upstream_contract = !self.upstream_artifacts_for_step(step_number).is_empty();
        let mut imported_sources = Vec::new();
        let mut missing_groups = Vec::new();
        let mut optional_missing_groups = Vec::new();
        let mut missing_required_groups = Vec::new();

        for group in groups {
            let sources = self.find_sources(&group.patterns, &group.mode, &group.source_ids)?;
            if sources.is_empty() {
                if group.required && !has_upstream_contract {
                    missing_groups.push(group.label.clone());
                    missing_required_groups.push(group.label.clone());
                } else {
                    optional_missing_groups.push(group.label.clone());
                }
                continue;
            }
            for (index, source) in sources.iter().enumerate() {
                let metadata = source_package_metadata(source);
                let source_ids = infer_source_ids(source);
                let primary_id = primary_source_id(source, &group.source_ids, &group.label);
                let version = metadata
                    .get("version")
                    .and_then(value_to_u32)
                    .unwrap_or_else(|| parse_version_name(&source.file_name_text()));
                let target_name = if version > 0 {
                    safe_component(&format!("{primary_id}_v{version}"), "source")
                } else {
                    safe_component(&format!("{}_{:03}", primary_id, index + 1), "source")
                };
                let target = out_dir
                    .join("imported")
                    .join(&group.label)
                    .join(target_name);
                copy_tree_contents(source, &target, &BTreeSet::new())?;
                imported_sources.push(ImportedSourceRecord {
                    group: group.label.clone(),
                    source_ids,
                    package_id: string_field(&metadata, "package_id").unwrap_or_default(),
                    package_type: string_field(&metadata, "package_type").unwrap_or(primary_id),
                    package_manifest: rel_if_exists(
                        &source.join("package_manifest.json"),
                        &self.paths.project_root,
                    ),
                    source: rel(source, &self.paths.project_root),
                    target: rel(&target, &self.paths.project_root),
                });
            }
        }

        if !missing_required_groups.is_empty() {
            write_text(
                &out_dir.join("MISSING_SOURCE_ARTIFACTS.md"),
                &operator_list("Missing Source Artifacts", &missing_required_groups),
            )?;
        }
        if !optional_missing_groups.is_empty() {
            write_text(
                &out_dir.join("OPTIONAL_SOURCE_ARTIFACTS_NOT_PROVIDED.md"),
                &operator_list(
                    "Optional Source Artifacts Not Provided",
                    &optional_missing_groups,
                ),
            )?;
        }
        if !missing_upstream_artifacts.is_empty() {
            write_text(
                &out_dir.join("MISSING_UPSTREAM_ARTIFACTS.md"),
                &operator_list("Missing Upstream Artifacts", &missing_upstream_artifacts),
            )?;
        }

        let imported = !imported_sources.is_empty() || !imported_upstream_artifacts.is_empty();
        let mut report_notes = notes.to_vec();
        if !missing_required_groups.is_empty() {
            report_notes
                .push("Required current-project source artifact groups are missing.".to_string());
        }
        if !missing_upstream_artifacts.is_empty() {
            report_notes.push(
                "Required upstream stage artifacts are missing or not validated.".to_string(),
            );
        }
        if !imported {
            report_notes.push(
                "No source artifact directory or upstream stage artifact matched this stage."
                    .to_string(),
            );
        }

        let report_status =
            if missing_required_groups.is_empty() && missing_upstream_artifacts.is_empty() {
                "success"
            } else {
                "failed"
            };
        let index = json!({
            "step": step_number,
            "name": spec.slug,
            "title": spec.title,
            "timestamp": now_iso(),
            "artifact_root": rel(&out_dir, &self.paths.project_root),
            "imported": imported,
            "imported_sources": to_json_value(&imported_sources)?,
            "missing_groups": missing_groups,
            "missing_required_groups": missing_required_groups,
            "optional_missing_groups": optional_missing_groups,
            "imported_upstream_artifacts": to_json_value(&imported_upstream_artifacts)?,
            "missing_upstream_artifacts": missing_upstream_artifacts,
            "manifest": to_json_value(&file_manifest(&out_dir)?)?,
        });
        write_json(&out_dir.join("artifact_index.json"), &index)?;
        write_text(
            &out_dir.join("README.md"),
            &format!(
                "# Stage {step_number:02}: {}\n\n- Imported source artifacts: {}\n- Imported upstream artifacts: {}\n- Missing required source groups: {}\n- Optional source groups not provided: {}\n- Missing upstream artifacts: {}\n",
                spec.title,
                imported_sources.len(),
                imported_upstream_artifacts.len(),
                comma_or_none(&missing_required_groups),
                comma_or_none(&optional_missing_groups),
                comma_or_none(&missing_upstream_artifacts),
            ),
        )?;

        let report = SourceImportReport {
            step: step_number,
            name: spec.slug,
            title: spec.title,
            status: report_status.to_string(),
            valid: report_status == "success",
            timestamp: now_iso(),
            artifacts_dir: rel(&out_dir, &self.paths.project_root),
            imported_sources,
            missing_groups,
            missing_required_groups,
            optional_missing_groups,
            imported_upstream_artifacts,
            missing_upstream_artifacts,
            notes: report_notes,
        };
        write_json(
            &out_dir.join("validation_report.json"),
            &to_json_value(&report)?,
        )?;

        if !report.missing_required_groups.is_empty() {
            return Err(AdmError::new(format!(
                "Missing required current-project source groups: {}",
                report.missing_required_groups.join(", ")
            )));
        }
        if !report.missing_upstream_artifacts.is_empty() {
            return Err(AdmError::new(format!(
                "Missing required upstream artifacts: {}",
                report.missing_upstream_artifacts.join(", ")
            )));
        }
        Ok(report)
    }

    pub fn build_reference_manifest(
        &self,
        step_number: u32,
        out_dir: &Path,
        imported_sources: &[ImportedSourceRecord],
        imported_upstream_artifacts: &[ImportedUpstreamArtifactRecord],
        missing_required_groups: &[String],
        optional_missing_groups: &[String],
        missing_upstream_artifacts: &[String],
    ) -> AdmResult<Value> {
        let mut source_inputs = Vec::new();
        let mut upstream_inputs = Vec::new();
        let mut relations = Vec::new();
        let current_artifacts = self.current_artifacts_for_step(step_number);
        let current_artifact_ids = current_artifacts
            .iter()
            .filter_map(|artifact| string_field(artifact, "id"))
            .collect::<Vec<_>>();

        for source in imported_sources {
            let source_root = self.paths.project_root.join(&source.source);
            let target_root = self.paths.project_root.join(&source.target);
            let mut files = Vec::new();
            for item in file_manifest(&source_root)? {
                let source_file = rel(&source_root.join(&item.path), &self.paths.project_root);
                let target_file = rel(&target_root.join(&item.path), &self.paths.project_root);
                files.push(json!({
                    "source_path": source_file,
                    "target_path": target_file,
                    "path": item.path,
                    "size_bytes": item.size_bytes,
                    "sha256": item.sha256,
                }));
                relations.push(json!({
                    "type": "source_file_copied",
                    "from": source_file,
                    "to": target_file,
                    "source_group": source.group,
                }));
            }
            source_inputs.push(json!({
                "group": source.group,
                "source": source.source,
                "target": source.target,
                "file_count": files.len(),
                "files": files,
            }));
        }

        for upstream in imported_upstream_artifacts {
            let source_dir = self.paths.project_root.join(&upstream.source);
            let files = self.upstream_file_refs(&source_dir)?;
            upstream_inputs.push(json!({
                "artifact_id": upstream.artifact_id,
                "stage": upstream.stage.parse::<u32>().unwrap_or(0),
                "stage_dir": upstream.source,
                "reference_record": upstream.target,
                "file_count": files.len(),
                "files": files,
            }));
            for file in &files {
                relations.push(json!({
                    "type": "upstream_file_referenced",
                    "from": file.get("stage_path").and_then(Value::as_str).unwrap_or_default(),
                    "to": rel(&out_dir.join("reference_manifest.json"), &self.paths.project_root),
                    "upstream_artifact_id": upstream.artifact_id,
                    "upstream_stage": upstream.stage.parse::<u32>().unwrap_or(0),
                }));
            }
            for current_id in &current_artifact_ids {
                relations.push(json!({
                    "type": "artifact_dependency",
                    "from_artifact_id": upstream.artifact_id,
                    "to_artifact_id": current_id,
                }));
            }
        }

        let spec = step_spec(step_number)?;
        let manifest = json!({
            "schema_version": 1,
            "generated_at": now_iso(),
            "stage": {
                "number": step_number,
                "name": spec.slug,
                "title": spec.title,
                "artifact_root": rel(out_dir, &self.paths.project_root),
            },
            "artifacts": current_artifacts,
            "inputs": {
                "source_artifacts": source_inputs,
                "upstream_artifacts": upstream_inputs,
                "missing_required_groups": missing_required_groups,
                "optional_missing_groups": optional_missing_groups,
                "missing_upstream_artifacts": missing_upstream_artifacts,
            },
            "files": [],
            "relations": relations,
            "summary": {
                "local_file_count": 0,
                "source_file_count": source_inputs.iter().map(|item| item.get("file_count").and_then(Value::as_u64).unwrap_or(0)).sum::<u64>(),
                "upstream_artifact_count": upstream_inputs.len(),
                "upstream_file_count": upstream_inputs.iter().map(|item| item.get("file_count").and_then(Value::as_u64).unwrap_or(0)).sum::<u64>(),
                "relation_count": relations.len(),
            },
        });
        write_json(&out_dir.join("reference_manifest.json"), &manifest)?;
        Ok(manifest)
    }

    pub fn refresh_reference_manifest_file_inventory(&self, step_number: u32) -> AdmResult<Value> {
        self.refresh_reference_manifest_file_inventory_with_locale(
            step_number,
            ArtifactLocale::default(),
        )
    }

    pub fn refresh_reference_manifest_file_inventory_with_locale(
        &self,
        step_number: u32,
        artifact_locale: ArtifactLocale,
    ) -> AdmResult<Value> {
        let out_dir = self.stage_dir(step_number);
        let path = out_dir.join("reference_manifest.json");
        let mut manifest = object_map_or_empty(read_json(&path, json!({})));
        if manifest.is_empty() {
            let spec = step_spec(step_number)?;
            manifest.insert("schema_version".to_string(), json!(1));
            manifest.insert("generated_at".to_string(), Value::String(now_iso()));
            manifest.insert(
                "stage".to_string(),
                json!({
                    "number": step_number,
                    "name": spec.slug,
                    "title": spec.title,
                    "artifact_root": rel(&out_dir, &self.paths.project_root),
                }),
            );
            manifest.insert(
                "artifacts".to_string(),
                json!(self.current_artifacts_for_step(step_number)),
            );
            manifest.insert(
                "inputs".to_string(),
                json!({
                    "source_artifacts": [],
                    "upstream_artifacts": [],
                    "missing_required_groups": [],
                    "optional_missing_groups": [],
                    "missing_upstream_artifacts": [],
                }),
            );
            manifest.insert("relations".to_string(), json!([]));
            manifest.insert("summary".to_string(), json!({}));
        }
        manifest.insert("artifact_locale".to_string(), json!(artifact_locale));
        if let Some(stage) = manifest.get_mut("stage").and_then(Value::as_object_mut) {
            stage.insert(
                "display_title".to_string(),
                json!(crate::development_registry::localized_stage_title(
                    step_number,
                    artifact_locale,
                )),
            );
        }
        manifest.insert("updated_at".to_string(), Value::String(now_iso()));
        let files = file_manifest(&out_dir)?
            .into_iter()
            .filter(|entry| entry.path != "reference_manifest.json")
            .map(|entry| {
                json!({
                    "path": entry.path,
                    "stage_path": rel(&out_dir.join(&entry.path), &self.paths.project_root),
                    "role": classify_stage_file(&entry.path),
                    "size_bytes": entry.size_bytes,
                    "sha256": entry.sha256,
                })
            })
            .collect::<Vec<_>>();
        let relation_count = manifest
            .get("relations")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        manifest.insert("files".to_string(), Value::Array(files.clone()));
        let summary = manifest
            .entry("summary".to_string())
            .or_insert_with(|| json!({}));
        if let Some(summary) = summary.as_object_mut() {
            summary.insert("local_file_count".to_string(), json!(files.len()));
            summary.insert("relation_count".to_string(), json!(relation_count));
        }
        let value = Value::Object(manifest);
        write_json(&path, &value)?;
        Ok(value)
    }

    pub fn take_snapshot(&self, step_number: u32, event: &str) -> AdmResult<PathBuf> {
        let spec = step_spec(step_number)?;
        let snapshot_name = format!(
            "{}_{}_step{step_number:02}_{}",
            datetime_stamp(),
            safe_component(event, "manual"),
            safe_component(&spec.title, "step")
        );
        let snapshot_path = self.snapshot_dir().join(snapshot_name);
        fs::create_dir_all(&snapshot_path)?;
        let mut manifest = Vec::new();
        for file in source_artifact_files(&self.paths.source_artifacts_dir)? {
            let rel_path = relative_display(&file, &self.paths.source_artifacts_dir);
            let target = snapshot_path.join(&rel_path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&file, &target)?;
            let bytes = fs::read(&file)?;
            manifest.push(json!({
                "path": rel_path,
                "size_bytes": bytes.len(),
                "sha256": sha256_hex(&bytes),
            }));
        }
        manifest.sort_by(|left, right| {
            left.get("path")
                .and_then(Value::as_str)
                .cmp(&right.get("path").and_then(Value::as_str))
        });
        write_json(
            &snapshot_path.join("manifest.json"),
            &json!({
                "step": step_number,
                "step_name": spec.title,
                "event": event,
                "timestamp": now_iso(),
                "file_count": manifest.len(),
                "files": manifest,
            }),
        )?;
        Ok(snapshot_path)
    }

    pub fn list_snapshots(&self) -> Vec<PathBuf> {
        let mut snapshots = Vec::new();
        let dir = self.snapshot_dir();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    snapshots.push(entry.path());
                }
            }
        }
        snapshots.sort();
        snapshots.reverse();
        snapshots
    }

    pub fn restore_snapshot(
        &self,
        snapshot_path: impl AsRef<Path>,
        dry_run: bool,
    ) -> AdmResult<Vec<String>> {
        let snapshot_path = snapshot_path.as_ref();
        let manifest_path = snapshot_path.join("manifest.json");
        if !manifest_path.exists() {
            return Err(AdmError::new(format!(
                "No manifest in snapshot: {}",
                snapshot_path.display()
            )));
        }
        let manifest = read_json(&manifest_path, json!({}));
        let mut actions = Vec::new();
        for entry in manifest
            .get("files")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let path = string_field(entry, "path").unwrap_or_default();
            if path.is_empty() {
                continue;
            }
            let src = snapshot_path.join(&path);
            let dst = self.paths.source_artifacts_dir.join(&path);
            if src.exists() {
                actions.push(format!("restore: {path}"));
                if !dry_run {
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(src, dst)?;
                }
            } else {
                actions.push(format!("missing in snapshot: {path}"));
            }
        }
        Ok(actions)
    }

    fn upstream_file_refs(&self, source_dir: &Path) -> AdmResult<Vec<Value>> {
        let reference_path = source_dir.join("reference_manifest.json");
        let reference = read_json(&reference_path, json!({}));
        if let Some(files) = reference.get("files").and_then(Value::as_array) {
            let result = files
                .iter()
                .filter_map(|item| {
                    let path_text = string_field(item, "stage_path")
                        .or_else(|| string_field(item, "path"))
                        .unwrap_or_default();
                    if path_text.is_empty() {
                        None
                    } else {
                        Some(json!({
                            "path": string_field(item, "path").unwrap_or_else(|| Path::new(&path_text).file_name_text()),
                            "stage_path": path_text,
                            "role": string_field(item, "role").unwrap_or_else(|| classify_stage_file(&path_text)),
                            "size_bytes": item.get("size_bytes").cloned().unwrap_or(Value::Null),
                            "sha256": item.get("sha256").cloned().unwrap_or(Value::Null),
                            "source_manifest": rel(&reference_path, &self.paths.project_root),
                        }))
                    }
                })
                .collect::<Vec<_>>();
            if !result.is_empty() {
                return Ok(result);
            }
        }
        let index = read_json(&source_dir.join("artifact_index.json"), json!({}));
        let manifest_files = index
            .get("manifest")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let files = if manifest_files.is_empty() {
            to_json_value(&file_manifest(source_dir)?)?
                .as_array()
                .cloned()
                .unwrap_or_default()
        } else {
            manifest_files
        };
        Ok(files
            .into_iter()
            .filter_map(|item| {
                let path_text = string_field(&item, "path").unwrap_or_default();
                if path_text.is_empty() {
                    None
                } else {
                    Some(json!({
                        "path": path_text,
                        "stage_path": rel(&source_dir.join(&path_text), &self.paths.project_root),
                        "role": classify_stage_file(&path_text),
                        "size_bytes": item.get("size_bytes").cloned().unwrap_or(Value::Null),
                        "sha256": item.get("sha256").cloned().unwrap_or(Value::Null),
                        "source_manifest": rel(&source_dir.join("artifact_index.json"), &self.paths.project_root),
                    }))
                }
            })
            .collect())
    }

    fn load_run_context_value(&self) -> Option<Value> {
        let env_path = std::env::var(RUN_CONTEXT_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .and_then(|value| {
                let configured = PathBuf::from(&value);
                if configured.is_absolute() {
                    configured
                        .starts_with(&self.paths.project_root)
                        .then(|| canonical_or_self(&configured))
                } else {
                    resolve_project_owned_path(&self.paths.project_root, &value)
                }
            });
        let path = env_path.unwrap_or_else(|| {
            self.paths
                .draft_dir
                .join("runtime")
                .join("run_context.json")
        });
        let value = read_json(&path, json!({}));
        value
            .as_object()
            .is_some_and(|object| !object.is_empty())
            .then_some(value)
    }

    fn snapshot_dir(&self) -> PathBuf {
        self.paths.source_artifacts_dir.join(".snapshots")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceGroup {
    pub label: String,
    pub patterns: Vec<String>,
    pub mode: String,
    pub required: bool,
    pub source_ids: Vec<String>,
}

impl SourceGroup {
    pub fn new(
        label: impl Into<String>,
        patterns: impl IntoIterator<Item = impl Into<String>>,
        mode: impl Into<String>,
        required: bool,
        source_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            label: label.into(),
            patterns: patterns.into_iter().map(Into::into).collect(),
            mode: mode.into(),
            required,
            source_ids: source_ids.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportedSourceRecord {
    pub group: String,
    pub source_ids: Vec<String>,
    pub package_id: String,
    pub package_type: String,
    pub package_manifest: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportedUpstreamArtifactRecord {
    pub artifact_id: String,
    pub stage: String,
    pub source: String,
    pub target: String,
    pub reference_dir: String,
    pub reference_manifest: String,
    pub artifact_index: String,
    pub validation_report: String,
    pub file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceImportReport {
    pub step: u32,
    pub name: String,
    pub title: String,
    pub status: String,
    pub valid: bool,
    pub timestamp: String,
    pub artifacts_dir: String,
    pub imported_sources: Vec<ImportedSourceRecord>,
    pub missing_groups: Vec<String>,
    pub missing_required_groups: Vec<String>,
    pub optional_missing_groups: Vec<String>,
    pub imported_upstream_artifacts: Vec<ImportedUpstreamArtifactRecord>,
    pub missing_upstream_artifacts: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineStepSpec {
    pub number: u32,
    pub slug: String,
    pub title: String,
    pub requires: Vec<u32>,
}

pub fn step_spec(step_number: u32) -> AdmResult<PipelineStepSpec> {
    let (slug, title, requires): (&str, &str, &[u32]) = match step_number {
        0 => ("idea_intake", "Idea Intake", &[]),
        1 => ("demo", "Gameplay Framework", &[0]),
        2 => ("design_review", "Design Review Freeze", &[1]),
        3 => ("program_requirements", "Program Requirements", &[2]),
        4 => ("art_requirements", "Art Requirements", &[3]),
        5 => ("program_review", "Program Review", &[3]),
        6 => ("art_review", "Art Review", &[4]),
        7 => (
            "art_style_generation",
            "Art Style Generation & Confirmation",
            &[6],
        ),
        8 => ("design_to_plan", "Program Plan", &[5]),
        9 => ("art_plan", "Art Plan", &[7]),
        10 => ("asset_alignment", "Asset Alignment", &[8, 9]),
        11 => ("dev_execution", "Dev Execution", &[10]),
        12 => ("art_production", "Art Production", &[10]),
        13 => ("scene_assembly", "Scene Assembly", &[11, 12]),
        14 => ("integration_validation", "Integration Validation", &[13]),
        _ => {
            return Err(AdmError::new(format!(
                "Unknown pipeline step: {step_number}"
            )));
        }
    };
    Ok(PipelineStepSpec {
        number: step_number,
        slug: slug.to_string(),
        title: title.to_string(),
        requires: requires.to_vec(),
    })
}

pub fn source_package_metadata(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    let manifest = read_json(&path.join("package_manifest.json"), json!({}));
    if manifest
        .as_object()
        .is_some_and(|object| !object.is_empty())
    {
        return manifest;
    }
    let submission = read_json(&path.join("operator_submission.json"), json!({}));
    if submission.is_object() {
        submission
    } else {
        json!({})
    }
}

pub fn infer_source_ids(path: impl AsRef<Path>) -> Vec<String> {
    let path = path.as_ref();
    let metadata = source_package_metadata(path);
    let mut ids = Vec::new();
    for key in [
        "source_id",
        "package_id",
        "package_type",
        "package_type_id",
        "prefix",
    ] {
        if let Some(value) = metadata
            .get(key)
            .map(value_string)
            .filter(|value| !value.is_empty())
        {
            ids.push(value);
        }
    }
    if let Some(raw_ids) = metadata.get("source_ids").and_then(Value::as_array) {
        ids.extend(
            raw_ids
                .iter()
                .map(value_string)
                .filter(|value| !value.is_empty()),
        );
    }
    for (marker, source_type) in SOURCE_MARKERS {
        if path.join(marker).exists() {
            ids.push((*source_type).to_string());
        }
    }
    let name = path.file_name_text();
    for source_type in SOURCE_TYPES {
        if name.contains(&format!("_{source_type}_"))
            || name.starts_with(&format!("{source_type}_"))
        {
            ids.push((*source_type).to_string());
        }
    }
    dedupe(ids)
}

pub fn source_matches_ids(path: impl AsRef<Path>, expected_ids: &[String]) -> bool {
    let expected = expected_ids
        .iter()
        .map(|item| norm_source_id(item))
        .filter(|item| !item.is_empty())
        .collect::<BTreeSet<_>>();
    if expected.is_empty() {
        return false;
    }
    let actual = infer_source_ids(path)
        .into_iter()
        .map(|item| norm_source_id(&item))
        .collect::<BTreeSet<_>>();
    !expected.is_disjoint(&actual)
}

pub fn project_glob(prefix: &str, project: &str) -> String {
    format!("{}_{prefix}_*", sanitize_project_name(project))
}

pub fn stage_globs(prefix: &str, project: &str, _include_legacy: bool) -> Vec<String> {
    vec![project_glob(prefix, project)]
}

pub fn is_temp_folder(path: &Path) -> bool {
    path.file_name_text().starts_with("Correction_")
}

pub fn classify_stage_file(path_text: &str) -> String {
    let normalized = path_text.replace('\\', "/");
    let name = Path::new(&normalized).file_name_text();
    if normalized.starts_with("guidance/") {
        "guidance"
    } else if normalized.starts_with("imported/") {
        "source_import"
    } else if normalized.starts_with("upstream/") {
        "upstream_reference"
    } else if matches!(
        name.as_str(),
        "artifact_index.json" | "reference_manifest.json"
    ) {
        "stage_index"
    } else if matches!(
        name.as_str(),
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

fn source_ids_from_patterns(patterns: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for pattern in patterns {
        for source_type in SOURCE_TYPES {
            if pattern.contains(&format!("_{source_type}_"))
                || pattern.starts_with(&format!("{source_type}_"))
            {
                result.push((*source_type).to_string());
            }
        }
    }
    dedupe(result)
}

fn matching_child_dirs(root: &Path, pattern: &str) -> AdmResult<Vec<PathBuf>> {
    let mut result = Vec::new();
    if !root.is_dir() {
        return Ok(result);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && wildcard_match(pattern, &path.file_name_text()) {
            result.push(path);
        }
    }
    Ok(result)
}

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let mut dp = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    dp[0][0] = true;
    for i in 1..=pattern.len() {
        if pattern[i - 1] == b'*' {
            dp[i][0] = dp[i - 1][0];
        }
    }
    for i in 1..=pattern.len() {
        for j in 1..=text.len() {
            dp[i][j] = match pattern[i - 1] {
                b'*' => dp[i - 1][j] || dp[i][j - 1],
                b'?' => dp[i - 1][j - 1],
                byte => byte == text[j - 1] && dp[i - 1][j - 1],
            };
        }
    }
    dp[pattern.len()][text.len()]
}

fn source_sort_cmp(left: &PathBuf, right: &PathBuf) -> std::cmp::Ordering {
    source_sort_key(left).cmp(&source_sort_key(right))
}

fn source_sort_key(path: &Path) -> (String, u32, u64, String) {
    let metadata = source_package_metadata(path);
    let created_at = string_field(&metadata, "created_at")
        .or_else(|| string_field(&metadata, "timestamp"))
        .unwrap_or_else(|| parse_date_name(&path.file_name_text()));
    let version = metadata
        .get("version")
        .and_then(value_to_u32)
        .unwrap_or_else(|| parse_version_name(&path.file_name_text()));
    (created_at, version, mtime_secs(path), path.file_name_text())
}

fn parse_version_name(name: &str) -> u32 {
    name.rsplit_once("_v")
        .and_then(|(_, version)| version.parse::<u32>().ok())
        .unwrap_or(0)
}

fn parse_date_name(name: &str) -> String {
    let bytes = name.as_bytes();
    for index in 0..bytes.len().saturating_sub(7) {
        if (index == 0 || bytes[index - 1] == b'_')
            && bytes[index..index + 8].iter().all(u8::is_ascii_digit)
            && (index + 8 == bytes.len() || bytes[index + 8] == b'_')
        {
            return name[index..index + 8].to_string();
        }
    }
    String::new()
}

fn primary_source_id(path: &Path, expected_ids: &[String], fallback: &str) -> String {
    let expected = expected_ids
        .iter()
        .map(|item| (norm_source_id(item), item.clone()))
        .collect::<BTreeMap<_, _>>();
    for source_id in infer_source_ids(path) {
        if let Some(expected_id) = expected.get(&norm_source_id(&source_id)) {
            return expected_id.clone();
        }
    }
    infer_source_ids(path)
        .into_iter()
        .next()
        .unwrap_or_else(|| fallback.to_string())
}

fn safe_component(value: &str, fallback: &str) -> String {
    let mut raw = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
            raw.push(ch);
        } else {
            raw.push('_');
        }
    }
    let raw = raw.trim_matches(['.', '_', '-']).to_string();
    if raw.is_empty() {
        fallback.to_string()
    } else {
        raw
    }
}

fn sanitize_project_name(value: &str) -> String {
    let clean = safe_component(value, DEFAULT_PROJECT_NAME)
        .trim_matches(['_', '-'])
        .to_string();
    if clean.is_empty() {
        DEFAULT_PROJECT_NAME.to_string()
    } else {
        clean
    }
}

fn copy_tree_contents(source: &Path, dest: &Path, skip_dirs: &BTreeSet<String>) -> AdmResult<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            if skip_dirs.contains(&entry.file_name().to_string_lossy().to_string()) {
                continue;
            }
            copy_tree_contents(&path, &target, skip_dirs)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn copy_tree_overlay(source: &Path, dest: &Path) -> AdmResult<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            if target.exists() {
                fs::remove_dir_all(&target)?;
            }
            copy_tree_contents(&path, &target, &BTreeSet::new())?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

fn safe_reset_dir(path: &Path, root: &Path) -> AdmResult<()> {
    fs::create_dir_all(root)?;
    let root_resolved = canonical_or_self(root);
    if path.exists() {
        let target_resolved = canonical_or_self(path);
        if target_resolved == root_resolved || !target_resolved.starts_with(&root_resolved) {
            return Err(AdmError::new(format!(
                "Refusing to reset path outside artifact root: {}",
                path.display()
            )));
        }
        fs::remove_dir_all(path)?;
    } else if !path.starts_with(root) {
        return Err(AdmError::new(format!(
            "Refusing to reset path outside artifact root: {}",
            path.display()
        )));
    }
    fs::create_dir_all(path)?;
    Ok(())
}

fn source_artifact_files(root: &Path) -> AdmResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_source_artifact_files(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_source_artifact_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
) -> AdmResult<()> {
    if !current.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path == root.join(".snapshots")
            || path
                .components()
                .any(|part| part.as_os_str() == ".snapshots")
        {
            continue;
        }
        if path.is_dir() {
            collect_source_artifact_files(root, &path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn push_unique_root(roots: &mut Vec<PathBuf>, seen: &mut BTreeSet<PathBuf>, root: PathBuf) {
    let key = canonical_or_self(&root);
    if seen.insert(key) {
        roots.push(root);
    }
}

fn canonical_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn resolve_project_owned_path(project_root: &Path, persisted: &str) -> Option<PathBuf> {
    let value = Path::new(persisted.trim());
    if value.as_os_str().is_empty()
        || value.is_absolute()
        || value.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
            )
        })
    {
        return None;
    }
    let project_root = canonical_or_self(project_root);
    let candidate = canonical_or_self(&project_root.join(value));
    candidate.starts_with(&project_root).then_some(candidate)
}

fn mtime_secs(path: &Path) -> u64 {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn value_to_u32(value: &Value) -> Option<u32> {
    value
        .as_u64()
        .and_then(|number| u32::try_from(number).ok())
        .or_else(|| value.as_str().and_then(|text| text.parse::<u32>().ok()))
}

fn number_field(value: &Value, field: &str) -> Option<u32> {
    value.get(field).and_then(value_to_u32)
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .map(value_string)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn value_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    }
}

fn object_map_or_empty(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn to_json_value<T: Serialize>(value: &T) -> AdmResult<Value> {
    serde_json::to_value(value)
        .map_err(|error| AdmError::new(format!("failed to serialize JSON value: {error}")))
}

fn rel_if_exists(path: &Path, root: &Path) -> String {
    if path.exists() {
        rel(path, root)
    } else {
        String::new()
    }
}

fn comma_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

fn operator_list(title: &str, items: &[String]) -> String {
    format!(
        "# {title}\n\n{}\n",
        items
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

fn norm_source_id(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn dedupe(items: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            result.push(item);
        }
    }
    result
}

fn date_stamp() -> String {
    let (year, month, day, _, _, _) = unix_datetime_parts(unix_timestamp());
    format!("{year:04}{month:02}{day:02}")
}

fn datetime_stamp() -> String {
    let (year, month, day, hour, minute, second) = unix_datetime_parts(unix_timestamp());
    format!("{year:04}{month:02}{day:02}_{hour:02}{minute:02}{second:02}")
}

fn unix_datetime_parts(timestamp: u64) -> (i32, u32, u32, u32, u32, u32) {
    let days = (timestamp / 86_400) as i64;
    let seconds = timestamp % 86_400;
    let (year, month, day) = civil_from_days(days);
    (
        year,
        month,
        day,
        (seconds / 3600) as u32,
        ((seconds % 3600) / 60) as u32,
        (seconds % 60) as u32,
    )
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

trait FileNameText {
    fn file_name_text(&self) -> String;
}

impl FileNameText for Path {
    fn file_name_text(&self) -> String {
        self.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string()
    }
}

impl FileNameText for PathBuf {
    fn file_name_text(&self) -> String {
        self.as_path().file_name_text()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;

    #[test]
    fn source_finder_infers_ids_and_selects_latest_by_metadata() {
        let root = temp_root("finder");
        let service = SourceService::new(&root, "session_a").unwrap();
        let first = service
            .paths
            .source_artifacts_dir
            .join("devflow_Concept_20260708_v1");
        let second = service
            .paths
            .source_artifacts_dir
            .join("devflow_Concept_20260709_v2");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        write_json(
            &first.join("package_manifest.json"),
            &json!({"package_id": "Concept", "version": 1, "created_at": "20260708"}),
        )
        .unwrap();
        write_json(
            &second.join("package_manifest.json"),
            &json!({"package_type": "Concept", "version": 2, "created_at": "20260709"}),
        )
        .unwrap();
        fs::write(second.join("selected_play_prototype.json"), "{}").unwrap();

        let ids = service.infer_source_ids(&second);
        assert!(ids.contains(&"Concept".to_string()));
        assert!(source_matches_ids(&second, &["concept".to_string()]));
        let latest = service
            .find_sources(
                &["devflow_Concept_*".to_string()],
                "latest",
                &["Concept".to_string()],
            )
            .unwrap();
        assert_eq!(latest, vec![second]);
        let all = service
            .find_sources(&["devflow_Concept_*".to_string()], "all", &[])
            .unwrap();
        assert_eq!(all.len(), 2);
        cleanup(root);
    }

    #[test]
    fn folder_manager_versions_corrections_and_design_resolution() {
        let root = temp_root("folders");
        let service = SourceService::new(&root, "session_a").unwrap();
        let first = service.make_folder("Design", "demo game").unwrap();
        fs::write(first.join("frozen_game_design.md"), "design v1").unwrap();
        let correction = service.make_correction_folder().unwrap();
        fs::write(correction.join("frozen_game_design.md"), "design v2").unwrap();
        let merged = service
            .merge_correction_to_permanent(&correction, "Design", "demo_game")
            .unwrap();
        assert!(merged.file_name_text().ends_with("_v2"));
        assert_eq!(
            fs::read_to_string(merged.join("frozen_game_design.md")).unwrap(),
            "design v2"
        );
        assert_eq!(
            service.resolve_design_path(None, "demo_game").unwrap(),
            merged.join("frozen_game_design.md")
        );
        assert_eq!(service.cleanup_temp_folders().len(), 1);
        cleanup(root);
    }

    #[test]
    fn import_step_copies_sources_writes_reports_and_reference_manifest() {
        let root = temp_root("import");
        let service = SourceService::new(&root, "session_a").unwrap();
        let source = service
            .paths
            .source_artifacts_dir
            .join("devflow_Concept_20260709_v1");
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("selected_play_prototype.json"), "{}").unwrap();
        fs::write(source.join("nested").join("note.txt"), "hello").unwrap();
        write_json(
            &source.join("package_manifest.json"),
            &json!({"package_id": "Concept", "package_type": "Concept", "version": 1}),
        )
        .unwrap();
        let group = SourceGroup::new(
            "concept",
            ["devflow_Concept_*"],
            "latest",
            true,
            ["Concept"],
        );

        let report = service.run_import_step(0, &[group], &[]).unwrap();

        assert_eq!(report.status, "success");
        assert_eq!(report.imported_sources.len(), 1);
        let copied = root
            .join(&report.imported_sources[0].target)
            .join("nested/note.txt");
        assert_eq!(fs::read_to_string(copied).unwrap(), "hello");
        let manifest = service
            .build_reference_manifest(
                0,
                &service.stage_dir(0),
                &report.imported_sources,
                &report.imported_upstream_artifacts,
                &report.missing_required_groups,
                &report.optional_missing_groups,
                &report.missing_upstream_artifacts,
            )
            .unwrap();
        assert_eq!(manifest["summary"]["source_file_count"], 3);
        let refreshed = service
            .refresh_reference_manifest_file_inventory(0)
            .unwrap();
        assert!(
            refreshed["files"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["role"] == "source_import")
        );
        cleanup(root);
    }

    #[test]
    fn import_step_reports_missing_required_source_group() {
        let root = temp_root("missing");
        let service = SourceService::new(&root, "session_a").unwrap();
        let group = SourceGroup::new(
            "concept",
            ["devflow_Concept_*"],
            "latest",
            true,
            ["Concept"],
        );

        let error = service.run_import_step(0, &[group], &[]).unwrap_err();

        assert!(error.to_string().contains("Missing required"));
        assert!(
            service
                .stage_dir(0)
                .join("MISSING_SOURCE_ARTIFACTS.md")
                .exists()
        );
        cleanup(root);
    }

    #[test]
    fn source_snapshots_copy_manifest_and_restore_files() {
        let root = temp_root("snapshot");
        let service = SourceService::new(&root, "session_a").unwrap();
        let source_file = service.paths.source_artifacts_dir.join("package/data.txt");
        fs::create_dir_all(source_file.parent().unwrap()).unwrap();
        fs::write(&source_file, "before").unwrap();

        let snapshot = service.take_snapshot(3, "unit_test").unwrap();
        fs::write(&source_file, "after").unwrap();
        let dry_run = service.restore_snapshot(&snapshot, true).unwrap();
        assert_eq!(dry_run, vec!["restore: package/data.txt"]);
        assert_eq!(fs::read_to_string(&source_file).unwrap(), "after");
        service.restore_snapshot(&snapshot, false).unwrap();
        assert_eq!(fs::read_to_string(&source_file).unwrap(), "before");
        assert_eq!(service.list_snapshots(), vec![snapshot]);
        cleanup(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm_new_source_{label}_{}",
            new_stable_id("root").unwrap()
        ))
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn relocated_source_context_never_reads_an_existing_old_data_root() {
        let current_root = temp_root("relocated_current");
        let old_root = temp_root("relocated_old");
        let current = SourceService::new(&current_root, "session_a").unwrap();
        let old_source = old_root.join("drafts/session_a/source_artifacts");
        fs::create_dir_all(&old_source).unwrap();
        fs::write(old_source.join("old-sentinel.txt"), "must not be read").unwrap();
        let runtime_dir = current.paths.draft_dir.join("runtime");
        fs::create_dir_all(&runtime_dir).unwrap();
        write_json(
            &runtime_dir.join("run_context.json"),
            &json!({"source_artifacts_root": old_source.display().to_string()}),
        )
        .unwrap();

        assert_eq!(
            current.source_artifact_roots(),
            vec![current.paths.source_artifacts_dir.clone()]
        );

        cleanup(current_root);
        cleanup(old_root);
    }

    #[test]
    fn relative_source_context_resolves_inside_the_current_data_root() {
        let root = temp_root("relative_context");
        let service = SourceService::new(&root, "session_a").unwrap();
        let runtime_dir = service.paths.draft_dir.join("runtime");
        fs::create_dir_all(&runtime_dir).unwrap();
        write_json(
            &runtime_dir.join("run_context.json"),
            &json!({"source_artifacts_root": "drafts/session_a/source_artifacts"}),
        )
        .unwrap();

        assert_eq!(
            service.source_artifact_roots(),
            vec![canonical_or_self(&service.paths.source_artifacts_dir)]
        );
        cleanup(root);
    }
}
