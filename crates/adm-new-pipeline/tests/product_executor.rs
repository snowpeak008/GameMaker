use std::fs;
use std::path::PathBuf;

use adm_new_contracts::project::ProjectState;
use adm_new_foundation::{new_stable_id, paths::SourceProjectRoot};
use adm_new_pipeline::ProductPipelineExecutor;

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

fn temp_root(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
    fs::create_dir_all(&root).unwrap();
    root
}
