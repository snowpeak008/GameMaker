use std::fs;
use std::path::{Path, PathBuf};

use adm_new_foundation::new_stable_id;
use adm_new_sdk::{SkillDocument, SkillOrigin, SkillOverlayRepository};
use serde_json::json;

#[test]
fn seed_override_tombstone_and_readd_are_deterministic_and_seed_is_read_only() {
    let root = temp_root("skill_overlay");
    let seed = root.join("seed");
    let overlay = root.join("overlay");
    let quarantine = root.join("quarantine");
    write(&seed.join("dev/run.json"), r#"{"name":"seed"}"#);
    write(
        &seed.join("art/example/SKILL.md"),
        "---\nname: example\ndescription: seed skill\n---\n\n# Seed\n",
    );
    let seed_before = tree_bytes(&seed);
    let repository = SkillOverlayRepository::with_roots(&overlay, &seed, &quarantine);

    assert_eq!(repository.list().unwrap().len(), 2);
    let record = repository
        .write_json("dev/run.json", &json!({"name": "overlay"}))
        .unwrap();
    assert_eq!(record.origin, SkillOrigin::Overlay);
    repository.remove("art/example/SKILL.md").unwrap();
    let records = repository.list().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].skill_id, "dev/run.json");
    assert_eq!(tree_bytes(&seed), seed_before);

    repository
        .write_markdown(
            "art/example/SKILL.md",
            "---\nname: example\ndescription: user override\n---\n\n# User\n",
        )
        .unwrap();
    assert_eq!(repository.list().unwrap().len(), 2);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn corrupt_overlay_is_quarantined_but_corrupt_seed_is_never_modified() {
    let root = temp_root("skill_corruption");
    let seed = root.join("seed");
    let overlay = root.join("overlay");
    let quarantine = root.join("quarantine");
    write(&seed.join("dev/valid.json"), r#"{"name":"valid"}"#);
    write(&overlay.join("dev/broken.json"), "{broken");
    let repository = SkillOverlayRepository::with_roots(&overlay, &seed, &quarantine);
    assert!(
        repository
            .list()
            .unwrap_err()
            .to_string()
            .contains("isolated")
    );
    assert!(!overlay.join("dev/broken.json").exists());
    assert!(contains_file(&quarantine, "broken.json"));

    write(&seed.join("dev/broken-seed.json"), "{broken-seed");
    let before = fs::read(seed.join("dev/broken-seed.json")).unwrap();
    let error = repository.list().unwrap_err().to_string();
    assert!(error.contains("read-only skill seed rejected"));
    assert_eq!(fs::read(seed.join("dev/broken-seed.json")).unwrap(), before);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn recursive_ids_and_documents_are_stable() {
    let root = temp_root("skill_ids");
    let repository = SkillOverlayRepository::with_roots(
        root.join("overlay"),
        root.join("seed"),
        root.join("quarantine"),
    );
    repository
        .write_json("pipeline/nested/generate.json", &json!({"name":"generate"}))
        .unwrap();
    let record = repository
        .get("pipeline\\nested\\generate.json")
        .unwrap()
        .unwrap();
    assert_eq!(record.skill_id, "pipeline/nested/generate.json");
    assert!(matches!(record.document, SkillDocument::Json(_)));
    let _ = fs::remove_dir_all(root);
}

fn temp_root(prefix: &str) -> PathBuf {
    let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
    fs::create_dir_all(&root).unwrap();
    root
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn tree_bytes(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut output = Vec::new();
    collect(root, root, &mut output);
    output.sort_by(|left, right| left.0.cmp(&right.0));
    output
}

fn collect(root: &Path, path: &Path, output: &mut Vec<(String, Vec<u8>)>) {
    if !path.is_dir() {
        return;
    }
    for entry in fs::read_dir(path).unwrap().flatten() {
        let child = entry.path();
        if child.is_dir() {
            collect(root, &child, output);
        } else {
            output.push((
                child
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                fs::read(child).unwrap(),
            ));
        }
    }
}

fn contains_file(root: &Path, name: &str) -> bool {
    root.is_dir()
        && fs::read_dir(root).unwrap().flatten().any(|entry| {
            let path = entry.path();
            (path.is_file() && path.file_name().is_some_and(|value| value == name))
                || (path.is_dir() && contains_file(&path, name))
        })
}
