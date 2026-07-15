use std::{fs, path::PathBuf};

const CORE_SOURCE_FILES: [&str; 4] = ["lib.rs", "id.rs", "capability.rs", "spec.rs"];

#[test]
fn core_vocabulary_has_no_fixture_or_engine_specific_defaults() {
    let forbidden = [
        "plants_vs_zombies",
        "tower_defense",
        "match_three",
        "visual_novel",
        "turn_tactics",
        "deckbuilder",
        "unity_engine",
    ];

    for path in CORE_SOURCE_FILES {
        let source = read_crate_file(PathBuf::from("src").join(path)).to_ascii_lowercase();
        for token in forbidden {
            assert!(
                !source.contains(token),
                "core source {path} contains fixture-specific token {token}"
            );
        }
    }
}

#[test]
fn domain_crate_has_no_application_layer_dependencies() {
    let manifest = read_crate_file("Cargo.toml");
    for dependency in [
        "adm-new-ai",
        "adm-new-contracts",
        "adm-new-design",
        "adm-new-pipeline",
        "adm-new-storage",
        "tauri",
    ] {
        assert!(
            !manifest.contains(dependency),
            "domain crate depends on application layer package {dependency}"
        );
    }
}

fn read_crate_file(relative: impl AsRef<std::path::Path>) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative))
        .expect("read domain crate source")
}
