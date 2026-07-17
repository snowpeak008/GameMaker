use std::{fs, path::PathBuf};

use adm_new_game_spec::{
    GameSpec, ProductEnvelope, ProductionScale, SpecId, parse_game_spec, validate_game_spec,
    validate_game_spec_for_envelope,
};

const FIXTURES: [&str; 4] = [
    "lane_guard.json",
    "match_grid.json",
    "branching_story.json",
    "turn_tactics.json",
];

#[test]
fn all_a01_fixtures_pass_deterministic_validation() {
    for filename in FIXTURES {
        let spec = fixture(filename);
        let report = validate_game_spec(&spec);
        assert!(
            report.is_valid(),
            "fixture {filename} failed validation: {}",
            serde_json::to_string_pretty(&report).expect("serialize validation report")
        );
    }
}

#[test]
fn strict_parser_rejects_duplicate_object_ids_before_serde_can_overwrite_them() {
    let error = parse_game_spec(r#"{"entities":{"unit":{},"unit":{}}}"#)
        .expect_err("duplicate object key was accepted");
    assert_eq!(error.code, "SPEC_DUPLICATE_ID");
    assert_eq!(error.severity, adm_new_game_spec::ValidationSeverity::Error);
    assert_eq!(error.related_ids, ["unit"]);
    assert!(error.message.contains("duplicate object key `unit`"));
    assert!(error.path.starts_with('/'));
    assert!(!error.suggestion.is_empty());
}

#[test]
fn mutation_failures_are_closed_with_stable_codes_and_paths() {
    let baseline = fixture("lane_guard.json");

    let mut missing_reference = baseline.clone();
    missing_reference.resources.remove(&id("energy"));
    assert_invalid(
        &missing_reference,
        "SPEC_REFERENCE_MISSING",
        "/actions/place_defender/preconditions/0/reads/0",
    );

    let mut invalid_transition = baseline.clone();
    invalid_transition
        .state_machines
        .get_mut(&id("encounter_flow"))
        .expect("encounter flow")
        .transitions[0]
        .to = id("missing_state");
    assert_invalid(
        &invalid_transition,
        "SPEC_STATE_REFERENCE_MISSING",
        "/stateMachines/encounter_flow/transitions/0/to",
    );

    let mut empty_effects = baseline.clone();
    empty_effects
        .actions
        .get_mut(&id("place_defender"))
        .expect("placement action")
        .effects
        .clear();
    assert_invalid(
        &empty_effects,
        "SPEC_ACTION_EFFECTS_EMPTY",
        "/actions/place_defender/effects",
    );

    let mut invalid_resource = baseline.clone();
    invalid_resource
        .resources
        .get_mut(&id("energy"))
        .expect("energy resource")
        .maximum = Some(-1);
    assert_invalid(
        &invalid_resource,
        "SPEC_RESOURCE_RANGE_INVALID",
        "/resources/energy",
    );

    let mut broken_trace = baseline;
    broken_trace.trace_links.clear();
    assert_invalid(
        &broken_trace,
        "SPEC_INTENT_TRACE_MISSING",
        "/intent/experiencePromises/read_and_counter",
    );
}

#[test]
fn requested_product_envelope_must_fit_supported_capacity() {
    let spec = fixture("lane_guard.json");
    let supported = ProductEnvelope {
        scene_scale: ProductionScale::Small,
        system_complexity: ProductionScale::Small,
        asset_scale: ProductionScale::Small,
        content_volume: ProductionScale::Small,
    };
    let report = validate_game_spec_for_envelope(&spec, &supported);

    assert!(!report.is_valid());
    assert!(report.contains_code("SPEC_ENVELOPE_EXCEEDED"));
    assert_eq!(
        report
            .issues
            .iter()
            .filter(|issue| issue.code == "SPEC_ENVELOPE_EXCEEDED")
            .count(),
        4
    );
}

fn assert_invalid(spec: &GameSpec, code: &str, path: &str) {
    let report = validate_game_spec(spec);
    let issue = report
        .issues
        .iter()
        .find(|issue| issue.code == code && issue.path == path)
        .unwrap_or_else(|| {
            panic!(
                "missing {code} at {path}: {}",
                serde_json::to_string_pretty(&report).expect("serialize validation report")
            )
        });
    assert!(!issue.message.is_empty());
    assert!(!issue.suggestion.is_empty());
    assert!(!report.is_valid());
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
