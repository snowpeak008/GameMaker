use adm_new_contracts::project::{DecisionState, ProjectState};
use adm_new_design::DesignEngineService;
use adm_new_foundation::{new_stable_id, sha256_hex};
use adm_new_game_spec::{SpecId, canonicalize_game_spec};
use adm_new_pipeline::cross_genre_evaluation::run_a09_cross_genre_evaluation;
use adm_new_pipeline::r2_release::{
    MigrationStatus, R2ReleaseEvidence, R2ReleaseSigningEvidence, R2ReleaseStatus,
    apply_game_spec_v2_sidecar_migration, preview_game_spec_v2_migration,
    rollback_game_spec_v2_sidecar_migration, run_r2_release_readiness,
};

#[test]
fn a10_sidecar_migration_is_idempotent_and_rollback_keeps_original_project_state() {
    let root = temp_root("a10_migration");
    let project_state_path = root.join("project_state.json");
    let service = service();
    let state = sample_project_state(&service);
    std::fs::write(
        &project_state_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();
    let original_hash = file_hash(&project_state_path);

    let preview = preview_game_spec_v2_migration(&service, &state).unwrap();
    assert_eq!(preview.status, MigrationStatus::Previewed);
    assert!(preview.would_write_sidecar);

    let first = apply_game_spec_v2_sidecar_migration(&service, &project_state_path).unwrap();
    let second = apply_game_spec_v2_sidecar_migration(&service, &project_state_path).unwrap();

    assert_eq!(first.status, MigrationStatus::SidecarWritten);
    assert_eq!(second.status, MigrationStatus::SidecarWritten);
    assert_eq!(
        first.project_state_hash_before,
        second.project_state_hash_before
    );
    assert_eq!(file_hash(&project_state_path), original_hash);
    assert!(root.join(".game_spec_v2_migration/game_spec.json").exists());
    assert!(
        root.join(".game_spec_v2_migration/projection_report.json")
            .exists()
    );
    assert!(
        root.join(".game_spec_v2_migration/migration_receipt.json")
            .exists()
    );

    let rollback = rollback_game_spec_v2_sidecar_migration(&project_state_path).unwrap();

    assert_eq!(rollback.status, MigrationStatus::RolledBack);
    assert_eq!(rollback.project_state_hash_after, original_hash);
    assert!(!root.join(".game_spec_v2_migration/game_spec.json").exists());
    assert!(
        root.join(format!(
            ".game_spec_v2_migration/backups/project_state.{}.json",
            original_hash
        ))
        .exists()
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a10_rejects_invalid_migration_input_without_side_effects() {
    let root = temp_root("a10_invalid_migration");
    let project_state_path = root.join("project_state.json");
    std::fs::write(&project_state_path, b"{not valid json").unwrap();
    let original_hash = file_hash(&project_state_path);

    let result = apply_game_spec_v2_sidecar_migration(&service(), &project_state_path);

    assert!(result.is_err());
    assert_eq!(file_hash(&project_state_path), original_hash);
    assert!(!root.join(".game_spec_v2_migration").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a10_migration_temp_publish_failure_leaves_no_sidecar_root() {
    let root = temp_root("a10_migration_publish_failure");
    let project_state_path = root.join("project_state.json");
    let service = service();
    let state = sample_project_state(&service);
    std::fs::write(
        &project_state_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();
    let original_hash = file_hash(&project_state_path);
    let temp_root = root.join(format!(
        ".game_spec_v2_migration.tmp.{}",
        &original_hash[..12]
    ));
    std::fs::write(&temp_root, b"not a directory").unwrap();

    let result = apply_game_spec_v2_sidecar_migration(&service, &project_state_path);

    assert!(result.is_err());
    assert_eq!(file_hash(&project_state_path), original_hash);
    assert!(!root.join(".game_spec_v2_migration").exists());
    assert!(temp_root.is_file());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a10_migration_handles_overlong_project_name_without_panicking() {
    let root = temp_root("a10_long_project_name");
    let project_state_path = root.join("project_state.json");
    let service = service();
    let mut state = sample_project_state(&service);
    state.project_name = "Very Long Project Name ".repeat(16);
    std::fs::write(
        &project_state_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let migration = apply_game_spec_v2_sidecar_migration(&service, &project_state_path).unwrap();

    assert_eq!(migration.status, MigrationStatus::SidecarWritten);
    let spec: adm_new_game_spec::GameSpec = serde_json::from_slice(
        &std::fs::read(root.join(".game_spec_v2_migration/game_spec.json")).unwrap(),
    )
    .unwrap();
    assert!(spec.identity.project_id.as_str().len() <= 96);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a10_migration_handles_overlong_multibyte_project_name_without_panicking() {
    let root = temp_root("a10_long_multibyte_project_name");
    let project_state_path = root.join("project_state.json");
    let service = service();
    let mut state = sample_project_state(&service);
    state.project_name = "超长中文项目名-生态防线-".repeat(32);
    std::fs::write(
        &project_state_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();

    let migration = apply_game_spec_v2_sidecar_migration(&service, &project_state_path).unwrap();

    assert_eq!(migration.status, MigrationStatus::SidecarWritten);
    let spec: adm_new_game_spec::GameSpec = serde_json::from_slice(
        &std::fs::read(root.join(".game_spec_v2_migration/game_spec.json")).unwrap(),
    )
    .unwrap();
    assert!(spec.identity.project_id.as_str().len() <= 96);
    assert!(SpecId::new(spec.identity.project_id.as_str()).is_ok());
    assert!(canonicalize_game_spec(&spec).is_ok());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a10_release_readiness_requires_manual_external_signature() {
    let root = temp_root("a10_release");
    let project_state_path = root.join("project_state.json");
    let service = service();
    let state = sample_project_state(&service);
    std::fs::write(
        &project_state_path,
        serde_json::to_vec_pretty(&state).unwrap(),
    )
    .unwrap();
    let migration = apply_game_spec_v2_sidecar_migration(&service, &project_state_path).unwrap();
    let a09 = run_a09_cross_genre_evaluation(&root.join("a09")).unwrap();

    let unsigned = run_r2_release_readiness(
        &a09,
        &migration,
        R2ReleaseEvidence::all_passed_for_tests(),
        R2ReleaseSigningEvidence {
            external_release_signed: false,
            signer: String::new(),
        },
        &root.join("unsigned"),
    )
    .unwrap();
    assert_eq!(unsigned.status, R2ReleaseStatus::Blocked);
    assert!(unsigned.new_project_game_spec_v2_default);
    assert!(!unsigned.external_release_allowed);
    assert!(
        unsigned
            .blockers
            .contains(&"external_release_manual_signature_missing".to_string())
    );

    let signed = run_r2_release_readiness(
        &a09,
        &migration,
        R2ReleaseEvidence::all_passed_for_tests(),
        R2ReleaseSigningEvidence {
            external_release_signed: true,
            signer: "user_acceptance".to_string(),
        },
        &root.join("signed"),
    )
    .unwrap();
    assert_eq!(signed.status, R2ReleaseStatus::Passed);
    assert!(signed.new_project_game_spec_v2_default);
    assert!(signed.old_projects_require_explicit_migration);
    assert!(signed.external_release_allowed);
    assert!(
        root.join("signed/r2_release_readiness_report.json")
            .exists()
    );
    let _ = std::fs::remove_dir_all(root);
}

fn service() -> DesignEngineService {
    DesignEngineService::new(Vec::new())
}

fn sample_project_state(service: &DesignEngineService) -> ProjectState {
    let mut state = service.empty_state();
    state.project_name = "A10 Migration Sample".to_string();
    if let Some(node) = state.nodes.values_mut().next() {
        node.decision_state = DecisionState::Selected;
        node.design_note = "A compact playable loop for migration validation.".to_string();
    }
    state
}

fn file_hash(path: &std::path::Path) -> String {
    sha256_hex(&std::fs::read(path).unwrap())
}

fn temp_root(prefix: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
    std::fs::create_dir_all(&root).unwrap();
    root
}
