use std::{fs, path::PathBuf};

use adm_new_game_spec::{
    GameSpec, ProductionScale, SpecId, canonicalize_game_spec, parse_game_spec,
};

#[test]
fn unordered_collections_and_json_object_order_have_one_canonical_hash() {
    let original_text = read_fixture("branching_story.json");
    let original = parse_game_spec(&original_text).expect("parse fixture");
    let mut reordered = original.clone();

    reordered.intent.audiences.reverse();
    reordered.intent.target_platforms.reverse();
    reordered.technical.platforms.reverse();
    for resource in reordered.resources.values_mut() {
        resource.source_actions.reverse();
        resource.sink_actions.reverse();
    }

    let first = canonicalize_game_spec(&original).expect("canonical fixture");
    let second = canonicalize_game_spec(&reordered).expect("canonical reordered fixture");
    assert_eq!(first.json, second.json);
    assert_eq!(first.content_hash, second.content_hash);
    assert_eq!(first.content_hash.len(), 64);
    assert!(
        first
            .content_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    );

    let value: serde_json::Value = serde_json::from_str(&original_text).expect("fixture JSON");
    let value_round_trip = serde_json::to_string_pretty(&value).expect("reordered object text");
    let reparsed = parse_game_spec(&value_round_trip).expect("parse object-reordered fixture");
    assert_eq!(
        first.content_hash,
        canonicalize_game_spec(&reparsed)
            .expect("canonical reparsed fixture")
            .content_hash
    );
}

#[test]
fn ordered_effect_sequence_changes_the_content_hash() {
    let original = fixture("lane_guard.json");
    let mut reordered = original.clone();
    reordered
        .actions
        .get_mut(&id("place_defender"))
        .expect("placement action")
        .effects
        .reverse();

    assert_ne!(
        canonicalize_game_spec(&original)
            .expect("canonical original")
            .content_hash,
        canonicalize_game_spec(&reordered)
            .expect("canonical reordered")
            .content_hash
    );
}

#[test]
fn product_envelope_is_hashed_but_execution_budget_is_not_in_the_spec() {
    let original = fixture("lane_guard.json");
    let mut larger = original.clone();
    larger.technical.product_envelope.asset_scale = ProductionScale::Large;

    let first = canonicalize_game_spec(&original).expect("canonical original");
    let second = canonicalize_game_spec(&larger).expect("canonical larger envelope");
    assert_ne!(first.content_hash, second.content_hash);

    for forbidden in [
        "maxAiCostUnits",
        "taskTimeoutSeconds",
        "maxRetryAttempts",
        "maxParallelWorkers",
    ] {
        assert!(
            !first.json.contains(forbidden),
            "local execution policy leaked into GameSpec: {forbidden}"
        );
    }
}

fn fixture(filename: &str) -> GameSpec {
    parse_game_spec(&read_fixture(filename))
        .unwrap_or_else(|error| panic!("fixture {filename} did not parse: {error}"))
}

fn id(value: &str) -> SpecId {
    SpecId::new(value).expect("test ID")
}

fn read_fixture(filename: &str) -> String {
    fs::read_to_string(workspace_root().join("testdata/game_spec").join(filename))
        .unwrap_or_else(|error| panic!("failed to read fixture {filename}: {error}"))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate must be inside the workspace")
        .to_path_buf()
}
