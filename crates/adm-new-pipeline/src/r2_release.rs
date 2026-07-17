use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use adm_new_contracts::project::ProjectState;
use adm_new_design::DesignEngineService;
use adm_new_design::game_spec_projection::project_state_to_game_spec;
use adm_new_foundation::{AdmError, AdmResult, io, sha256_hex};
use serde::{Deserialize, Serialize};

use crate::cross_genre_evaluation::{A09EvaluationReport, A09EvaluationStatus};

pub const A10_RELEASE_COMPILER_VERSION: &str = "game_spec_a10_migration_release.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStatus {
    Previewed,
    SidecarWritten,
    RolledBack,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameSpecV2MigrationPreview {
    pub schema_version: String,
    pub status: MigrationStatus,
    pub project_state_hash: String,
    pub projected_game_spec_hash: String,
    pub validation_error_count: usize,
    pub would_write_sidecar: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameSpecV2MigrationReport {
    pub schema_version: String,
    pub status: MigrationStatus,
    pub project_state_path: String,
    pub project_state_hash_before: String,
    pub project_state_hash_after: String,
    pub projected_game_spec_hash: String,
    pub backup_path: String,
    pub game_spec_path: String,
    pub projection_report_path: String,
    pub receipt_path: String,
    pub original_project_state_unchanged: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameSpecV2RollbackReport {
    pub schema_version: String,
    pub status: MigrationStatus,
    pub project_state_path: String,
    pub removed_paths: Vec<String>,
    pub project_state_hash_after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct R2ReleaseEvidence {
    pub rust_workspace_gate_passed: bool,
    pub web_gate_passed: bool,
    pub a09_regression_passed: bool,
    pub standalone_boundary_passed: bool,
    pub portable_smoke_passed: bool,
    pub cross_computer_relocation_passed: bool,
}

impl R2ReleaseEvidence {
    pub fn all_passed_for_tests() -> Self {
        Self {
            rust_workspace_gate_passed: true,
            web_gate_passed: true,
            a09_regression_passed: true,
            standalone_boundary_passed: true,
            portable_smoke_passed: true,
            cross_computer_relocation_passed: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct R2ReleaseSigningEvidence {
    pub external_release_signed: bool,
    pub signer: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum R2ReleaseStatus {
    Passed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct R2ReleaseReadinessReport {
    pub schema_version: String,
    pub compiler_version: String,
    pub status: R2ReleaseStatus,
    pub new_project_game_spec_v2_default: bool,
    pub old_projects_require_explicit_migration: bool,
    pub external_release_allowed: bool,
    pub migration_status: MigrationStatus,
    pub a09_status: A09EvaluationStatus,
    pub evidence: R2ReleaseEvidence,
    pub signing: R2ReleaseSigningEvidence,
    pub blockers: Vec<String>,
    pub output_paths: BTreeMap<String, String>,
}

pub fn preview_game_spec_v2_migration(
    engine: &DesignEngineService,
    project_state: &ProjectState,
) -> AdmResult<GameSpecV2MigrationPreview> {
    let project_state_hash = hash_json(project_state)?;
    let projection = project_state_to_game_spec(engine, project_state)?;
    let blockers = if projection.report.validation_error_count == 0 {
        Vec::new()
    } else {
        vec!["projected_game_spec_validation_errors".to_string()]
    };
    Ok(GameSpecV2MigrationPreview {
        schema_version: "game_spec_v2_migration_preview.v1".to_string(),
        status: if blockers.is_empty() {
            MigrationStatus::Previewed
        } else {
            MigrationStatus::Blocked
        },
        project_state_hash,
        projected_game_spec_hash: projection.report.game_spec_hash,
        validation_error_count: projection.report.validation_error_count,
        would_write_sidecar: blockers.is_empty(),
        blockers,
    })
}

pub fn apply_game_spec_v2_sidecar_migration(
    engine: &DesignEngineService,
    project_state_path: &Path,
) -> AdmResult<GameSpecV2MigrationReport> {
    let original_text = std::fs::read_to_string(project_state_path)?;
    let project_state: ProjectState = serde_json::from_str(&original_text).map_err(|error| {
        AdmError::new(format!(
            "project state migration input is not valid JSON: {error}"
        ))
    })?;
    let before_hash = sha256_hex(original_text.as_bytes());
    let projection = project_state_to_game_spec(engine, &project_state)?;
    let mut blockers = Vec::new();
    if projection.report.validation_error_count != 0 {
        blockers.push("projected_game_spec_validation_errors".to_string());
    }
    if !blockers.is_empty() {
        return Ok(blocked_migration_report(
            project_state_path,
            &before_hash,
            projection.report.game_spec_hash,
            blockers,
        ));
    }

    let root = migration_root(project_state_path)?;
    let temp_root = migration_temp_root(&root, &before_hash);
    if temp_root.exists() {
        std::fs::remove_dir_all(&temp_root)?;
    }
    let write_result = write_sidecar_migration_files(
        project_state_path,
        &temp_root,
        &root,
        &original_text,
        &before_hash,
        projection,
    );
    let report = match write_result {
        Ok(report) => report,
        Err(error) => {
            if temp_root.exists() {
                let _ = std::fs::remove_dir_all(&temp_root);
            }
            return Err(error);
        }
    };
    atomic_publish_migration_root(&root, &temp_root, &before_hash)?;
    Ok(report)
}

fn write_sidecar_migration_files(
    project_state_path: &Path,
    write_root: &Path,
    report_root: &Path,
    original_text: &str,
    before_hash: &str,
    projection: adm_new_design::game_spec_projection::GameSpecProjection,
) -> AdmResult<GameSpecV2MigrationReport> {
    let backup_dir = write_root.join("backups");
    std::fs::create_dir_all(&backup_dir)?;
    let backup_path = backup_dir.join(format!("project_state.{before_hash}.json"));
    if !backup_path.exists() {
        std::fs::write(&backup_path, original_text.as_bytes())?;
    }
    io::write_json_serializable(&write_root.join("game_spec.json"), &projection.spec)?;
    io::write_json_serializable(
        &write_root.join("projection_report.json"),
        &projection.report,
    )?;
    let after_text = std::fs::read_to_string(project_state_path)?;
    let after_hash = sha256_hex(after_text.as_bytes());
    let original_project_state_unchanged = before_hash == after_hash;
    let receipt = serde_json::json!({
        "schemaVersion": "game_spec_v2_migration_receipt.v1",
        "mode": "sidecar_only",
        "projectStateHash": before_hash,
        "gameSpecHash": projection.report.game_spec_hash,
        "originalProjectStateUnchanged": before_hash == after_hash,
    });
    io::write_json(&write_root.join("migration_receipt.json"), &receipt)?;
    let reported_backup_path = report_root
        .join("backups")
        .join(format!("project_state.{before_hash}.json"));
    Ok(GameSpecV2MigrationReport {
        schema_version: "game_spec_v2_migration_report.v1".to_string(),
        status: MigrationStatus::SidecarWritten,
        project_state_path: path_string(project_state_path),
        project_state_hash_before: before_hash.to_string(),
        project_state_hash_after: after_hash,
        projected_game_spec_hash: projection.report.game_spec_hash,
        backup_path: path_string(&reported_backup_path),
        game_spec_path: path_string(&report_root.join("game_spec.json")),
        projection_report_path: path_string(&report_root.join("projection_report.json")),
        receipt_path: path_string(&report_root.join("migration_receipt.json")),
        original_project_state_unchanged,
        blockers: Vec::new(),
    })
}

pub fn rollback_game_spec_v2_sidecar_migration(
    project_state_path: &Path,
) -> AdmResult<GameSpecV2RollbackReport> {
    let root = migration_root(project_state_path)?;
    let mut removed_paths = Vec::new();
    for filename in [
        "game_spec.json",
        "projection_report.json",
        "migration_receipt.json",
    ] {
        let path = root.join(filename);
        if path.exists() {
            std::fs::remove_file(&path)?;
            removed_paths.push(path_string(&path));
        }
    }
    let project_state_hash_after = sha256_hex(std::fs::read(project_state_path)?.as_slice());
    Ok(GameSpecV2RollbackReport {
        schema_version: "game_spec_v2_rollback_report.v1".to_string(),
        status: MigrationStatus::RolledBack,
        project_state_path: path_string(project_state_path),
        removed_paths,
        project_state_hash_after,
    })
}

pub fn run_r2_release_readiness(
    a09: &A09EvaluationReport,
    migration: &GameSpecV2MigrationReport,
    evidence: R2ReleaseEvidence,
    signing: R2ReleaseSigningEvidence,
    out_dir: &Path,
) -> AdmResult<R2ReleaseReadinessReport> {
    std::fs::create_dir_all(out_dir)?;
    let mut blockers = Vec::new();
    if a09.status != A09EvaluationStatus::Passed {
        blockers.push("a09_regression_not_passed".to_string());
    }
    if migration.status != MigrationStatus::SidecarWritten
        || !migration.original_project_state_unchanged
    {
        blockers.push("migration_not_safe".to_string());
    }
    for (id, passed) in [
        ("rust_workspace_gate", evidence.rust_workspace_gate_passed),
        ("web_gate", evidence.web_gate_passed),
        ("a09_regression", evidence.a09_regression_passed),
        ("standalone_boundary", evidence.standalone_boundary_passed),
        ("portable_smoke", evidence.portable_smoke_passed),
        (
            "cross_computer_relocation",
            evidence.cross_computer_relocation_passed,
        ),
    ] {
        if !passed {
            blockers.push(format!("{id}_not_passed"));
        }
    }
    if !signing.external_release_signed || signing.signer.trim().is_empty() {
        blockers.push("external_release_manual_signature_missing".to_string());
    }
    let new_project_default = a09.status == A09EvaluationStatus::Passed
        && migration.status == MigrationStatus::SidecarWritten
        && migration.original_project_state_unchanged;
    let status = if blockers.is_empty() {
        R2ReleaseStatus::Passed
    } else {
        R2ReleaseStatus::Blocked
    };
    let mut report = R2ReleaseReadinessReport {
        schema_version: "r2_release_readiness.v1".to_string(),
        compiler_version: A10_RELEASE_COMPILER_VERSION.to_string(),
        status,
        new_project_game_spec_v2_default: new_project_default,
        old_projects_require_explicit_migration: true,
        external_release_allowed: blockers.is_empty(),
        migration_status: migration.status.clone(),
        a09_status: a09.status.clone(),
        evidence,
        signing,
        blockers,
        output_paths: BTreeMap::new(),
    };
    let path = out_dir.join("r2_release_readiness_report.json");
    report
        .output_paths
        .insert("r2ReleaseReadinessReport".to_string(), path_string(&path));
    io::write_json_serializable(&path, &report)?;
    Ok(report)
}

fn blocked_migration_report(
    project_state_path: &Path,
    before_hash: &str,
    game_spec_hash: String,
    blockers: Vec<String>,
) -> GameSpecV2MigrationReport {
    GameSpecV2MigrationReport {
        schema_version: "game_spec_v2_migration_report.v1".to_string(),
        status: MigrationStatus::Blocked,
        project_state_path: path_string(project_state_path),
        project_state_hash_before: before_hash.to_string(),
        project_state_hash_after: before_hash.to_string(),
        projected_game_spec_hash: game_spec_hash,
        backup_path: String::new(),
        game_spec_path: String::new(),
        projection_report_path: String::new(),
        receipt_path: String::new(),
        original_project_state_unchanged: true,
        blockers,
    }
}

fn migration_root(project_state_path: &Path) -> AdmResult<PathBuf> {
    let parent = project_state_path
        .parent()
        .ok_or_else(|| AdmError::new("project state path has no parent"))?;
    Ok(parent.join(".game_spec_v2_migration"))
}

fn migration_temp_root(root: &Path, before_hash: &str) -> PathBuf {
    let suffix = before_hash.get(..12).unwrap_or(before_hash);
    root.with_file_name(format!(".game_spec_v2_migration.tmp.{suffix}"))
}

fn migration_old_root(root: &Path, before_hash: &str) -> PathBuf {
    let suffix = before_hash.get(..12).unwrap_or(before_hash);
    root.with_file_name(format!(".game_spec_v2_migration.old.{suffix}"))
}

fn atomic_publish_migration_root(
    root: &Path,
    temp_root: &Path,
    before_hash: &str,
) -> AdmResult<()> {
    let old_root = migration_old_root(root, before_hash);
    if old_root.exists() {
        std::fs::remove_dir_all(&old_root)?;
    }
    if root.exists() {
        std::fs::rename(root, &old_root)?;
    }
    if let Err(error) = std::fs::rename(temp_root, root) {
        if old_root.exists() && !root.exists() {
            let _ = std::fs::rename(&old_root, root);
        }
        return Err(AdmError::new(format!(
            "failed to publish sidecar migration atomically: {error}"
        )));
    }
    if old_root.exists() {
        std::fs::remove_dir_all(old_root)?;
    }
    Ok(())
}

fn hash_json<T: Serialize>(value: &T) -> AdmResult<String> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|error| AdmError::new(format!("failed to hash JSON: {error}")))
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
