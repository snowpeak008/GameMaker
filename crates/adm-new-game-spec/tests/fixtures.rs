use std::{collections::BTreeSet, fs, path::PathBuf};

use adm_new_game_spec::{GAME_SPEC_SCHEMA_VERSION, GameSpec};

const FIXTURES: [(&str, &str); 4] = [
    ("lane_guard", "lane_guard.json"),
    ("match_grid", "match_grid.json"),
    ("branching_story", "branching_story.json"),
    ("turn_tactics", "turn_tactics.json"),
];

#[test]
fn structurally_distinct_fixtures_round_trip_through_one_schema() {
    let mut topologies = BTreeSet::new();
    let mut time_models = BTreeSet::new();
    let mut control_models = BTreeSet::new();

    for (name, filename) in FIXTURES {
        let fixture = read_fixture(filename);
        let parsed: GameSpec = serde_json::from_str(&fixture)
            .unwrap_or_else(|error| panic!("fixture {name} did not parse: {error}"));
        assert_eq!(parsed.identity.schema_version, GAME_SPEC_SCHEMA_VERSION);
        assert_eq!(parsed.identity.revision, 1);
        assert!(!parsed.intent.experience_promises.is_empty());
        assert!(!parsed.actions.is_empty());
        assert!(!parsed.acceptance_scenarios.is_empty());

        topologies.insert(format!("{:?}", parsed.capabilities.space.topology));
        time_models.insert(format!("{:?}", parsed.capabilities.time.progression));
        control_models.insert(format!("{:?}", parsed.capabilities.control.cardinality));

        let canonical_shape = serde_json::to_string_pretty(&parsed).expect("serialize fixture");
        let reparsed: GameSpec =
            serde_json::from_str(&canonical_shape).expect("reparse serialized fixture");
        assert_eq!(reparsed, parsed, "fixture {name} changed semantically");
        assert_eq!(
            serde_json::to_string(&reparsed).expect("serialize reparsed fixture"),
            serde_json::to_string(&parsed).expect("serialize parsed fixture"),
            "fixture {name} serialization is not deterministic"
        );
    }

    assert!(topologies.len() >= 3, "fixtures need spatial diversity");
    assert!(time_models.len() >= 3, "fixtures need temporal diversity");
    assert!(control_models.len() >= 3, "fixtures need control diversity");
}

#[test]
fn capability_profile_rejects_unmodeled_type_labels() {
    let fixture = read_fixture(FIXTURES[0].1);
    let mut payload: serde_json::Value = serde_json::from_str(&fixture).expect("fixture JSON");
    payload["capabilities"]["genre"] = serde_json::Value::String("example".to_string());

    let error = serde_json::from_value::<GameSpec>(payload).expect_err("unknown field accepted");
    assert!(error.to_string().contains("unknown field `genre`"));
}

fn read_fixture(filename: &str) -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .ancestors()
        .nth(2)
        .expect("crate must be inside the workspace");
    fs::read_to_string(
        workspace_root
            .join("testdata")
            .join("game_spec")
            .join(filename),
    )
    .unwrap_or_else(|error| panic!("failed to read fixture {filename}: {error}"))
}
