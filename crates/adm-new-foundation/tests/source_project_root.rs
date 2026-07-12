use std::path::Path;

use adm_new_foundation::new_stable_id;
use adm_new_foundation::source_root::{
    ROOT_MARKER, SOURCE_PROJECT_ID, SourceProjectRoot, safe_project_join,
};
use serde_json::json;

#[test]
fn source_project_root_validates_renamed_unicode_checkout() {
    let root = std::env::temp_dir()
        .join(new_stable_id("source_root").unwrap())
        .join("独立 项目-renamed");
    write_source_project_fixture(&root);
    let nested = root.join("crates/demo/src");
    std::fs::create_dir_all(&nested).unwrap();

    let source_root = SourceProjectRoot::discover(&nested).unwrap();

    assert_eq!(source_root.path(), root.canonicalize().unwrap());
    assert_eq!(source_root.manifest().project_id, SOURCE_PROJECT_ID);
    assert_eq!(
        source_root.join("knowledge/design_data").unwrap(),
        root.canonicalize().unwrap().join("knowledge/design_data")
    );
    let cleanup_root = root.parent().unwrap().to_path_buf();
    let _ = std::fs::remove_dir_all(cleanup_root);
}

#[test]
fn source_project_root_rejects_invalid_nearest_marker_instead_of_using_parent() {
    let outer = std::env::temp_dir().join(new_stable_id("source_boundary").unwrap());
    write_source_project_fixture(&outer);
    let nested_root = outer.join("unrelated_child");
    let start = nested_root.join("crates/demo");
    std::fs::create_dir_all(&start).unwrap();
    std::fs::write(nested_root.join(ROOT_MARKER), "{}").unwrap();

    let error = SourceProjectRoot::discover(&start).unwrap_err();

    assert!(
        error
            .message()
            .contains("invalid source project root marker")
    );
    let _ = std::fs::remove_dir_all(outer);
}

#[test]
fn source_project_root_rejects_dangling_nearest_marker_instead_of_using_parent() {
    let outer = std::env::temp_dir().join(new_stable_id("source_dangling_boundary").unwrap());
    write_source_project_fixture(&outer);
    let nested_root = outer.join("unrelated_child");
    let start = nested_root.join("crates/demo");
    std::fs::create_dir_all(&start).unwrap();
    if !create_file_symlink(
        &nested_root.join("missing-marker-target"),
        &nested_root.join(ROOT_MARKER),
    ) {
        let _ = std::fs::remove_dir_all(outer);
        return;
    }

    let error = SourceProjectRoot::discover(&start).unwrap_err();

    assert!(
        error
            .message()
            .contains("source project root marker must be a regular file")
    );
    let _ = std::fs::remove_dir_all(outer);
}

#[test]
fn source_project_root_rejects_missing_declared_lockfile() {
    let root = std::env::temp_dir().join(new_stable_id("source_lockfile").unwrap());
    write_source_project_fixture(&root);
    std::fs::remove_file(root.join("web/package-lock.json")).unwrap();

    let error = SourceProjectRoot::open(&root).unwrap_err();

    assert!(
        error
            .message()
            .contains("required source project file is missing")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn safe_project_join_rejects_escape_and_keeps_new_paths_inside_root() {
    let root = std::env::temp_dir().join(new_stable_id("safe_project_join").unwrap());
    std::fs::create_dir_all(&root).unwrap();

    assert!(safe_project_join(&root, "../outside.json").is_err());
    assert!(safe_project_join(&root, "").is_err());
    assert_eq!(
        safe_project_join(&root, "testdata/new/report.json").unwrap(),
        root.canonicalize()
            .unwrap()
            .join("testdata/new/report.json")
    );
    let dangling = root.join("dangling.json");
    if create_file_symlink(&root.join("missing.json"), &dangling) {
        assert!(safe_project_join(&root, "dangling.json").is_err());
    }
    let _ = std::fs::remove_dir_all(root);
}

fn write_source_project_fixture(root: &Path) {
    std::fs::create_dir_all(root.join("knowledge/design_data")).unwrap();
    std::fs::create_dir_all(root.join("web")).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
    std::fs::write(root.join("Cargo.lock"), "# fixture\n").unwrap();
    std::fs::write(root.join("web/package-lock.json"), "{}\n").unwrap();
    std::fs::write(
        root.join("knowledge/resource-manifest.json"),
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "projectId": SOURCE_PROJECT_ID,
            "groups": []
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        root.join(ROOT_MARKER),
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "kind": "source-project-root",
            "projectId": SOURCE_PROJECT_ID,
            "workspaceManifest": "Cargo.toml",
            "lockfiles": ["Cargo.lock", "web/package-lock.json"],
            "resourceManifest": "knowledge/resource-manifest.json"
        }))
        .unwrap(),
    )
    .unwrap();
}

#[cfg(unix)]
fn create_file_symlink(source: &Path, target: &Path) -> bool {
    std::os::unix::fs::symlink(source, target).is_ok()
}

#[cfg(windows)]
fn create_file_symlink(source: &Path, target: &Path) -> bool {
    std::os::windows::fs::symlink_file(source, target).is_ok()
}
