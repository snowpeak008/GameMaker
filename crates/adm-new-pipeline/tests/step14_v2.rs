use adm_new_foundation::new_stable_id;
use adm_new_game_spec::{GameSpec, canonicalize_game_spec, parse_game_spec};
use adm_new_pipeline::stages::step13_v2::{
    AcceptanceScenarioResult, AutomationKind, STEP13_V2_COMPILER_VERSION, ScenarioExecutionStatus,
    Step13AcceptanceOutput, Step13Status,
};
use adm_new_pipeline::stages::step14_v2::{
    R1GateEvidence, R1GateStatus, R1PipelineGateEvidence, R1UserPlaytestSignature,
    derive_r1_gate_evidence_from_sources, run_step14_r1_packaging_gate,
};
use serde_json::{Value, json};

#[test]
fn r1_stop_gate_passes_when_all_evidence_is_present() {
    let root = temp_root("step14_v2_pass");
    let spec = r1_fixture();
    let step13 = passed_step13(&spec);
    let evidence = R1GateEvidence::all_passed_for_tests();

    let output =
        run_step14_r1_packaging_gate(&spec, &step13, &evidence, &root.join("step14")).unwrap();

    assert_eq!(output.status, R1GateStatus::Passed);
    assert!(output.blockers.is_empty());
    assert_eq!(
        output.release_manifest.release_signing,
        "manual_required_for_external_release"
    );
    assert!(root.join("step14/r1_release_manifest.json").exists());
    assert!(root.join("step14/r1_stop_gate_report.json").exists());
    assert!(root.join("step14/exe_smoke_report.json").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn r1_stop_gate_blocks_without_user_playtest_signature() {
    let root = temp_root("step14_v2_unsigned");
    let spec = r1_fixture();
    let step13 = passed_step13(&spec);
    let mut evidence = R1GateEvidence::all_passed_for_tests();
    evidence.user_playtest_signed = false;

    let output =
        run_step14_r1_packaging_gate(&spec, &step13, &evidence, &root.join("step14")).unwrap();

    assert_eq!(output.status, R1GateStatus::Blocked);
    assert!(
        output
            .blockers
            .contains(&"r1_user_playtest_signed".to_string())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn r1_stop_gate_blocks_when_acceptance_scenarios_failed() {
    let root = temp_root("step14_v2_failed_acceptance");
    let spec = r1_fixture();
    let mut step13 = passed_step13(&spec);
    step13.status = Step13Status::Failed;
    step13.scenario_results[0].status = ScenarioExecutionStatus::Failed;
    step13.scenario_results[0].failure_reason = Some("injected scenario failure".to_string());
    let evidence = R1GateEvidence::all_passed_for_tests();

    let output =
        run_step14_r1_packaging_gate(&spec, &step13, &evidence, &root.join("step14")).unwrap();

    assert_eq!(output.status, R1GateStatus::Blocked);
    assert!(
        output
            .blockers
            .contains(&"r1_acceptance_scenarios_passed".to_string())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn r1_gate_derives_tool_pipeline_and_manual_signature_evidence() {
    let root = temp_root("step14_v2_derive_pass");
    let stage_dir = root.join("stage14");
    std::fs::create_dir_all(&stage_dir).unwrap();
    let evidence_id = write_standalone_release_evidence(&root, |document| document);
    write_json_file(
        &stage_dir.join("r1_pipeline_gate_evidence.json"),
        &serde_json::to_value(R1PipelineGateEvidence::all_passed_for_tests()).unwrap(),
    );
    write_json_file(
        &stage_dir.join("r1_user_playtest_signature.json"),
        &serde_json::to_value(R1UserPlaytestSignature::manual_for_tests(&evidence_id)).unwrap(),
    );

    let derivation = derive_r1_gate_evidence_from_sources(&root, &stage_dir);

    assert!(derivation.report.blockers.is_empty());
    assert!(derivation.evidence.reproducible_build);
    assert!(derivation.evidence.integrity_passed);
    assert!(derivation.evidence.exe_smoke_passed);
    assert!(derivation.evidence.standalone_boundary_passed);
    assert!(derivation.evidence.ai_usage_evidence_present);
    assert!(derivation.evidence.ai_off_flow_supported);
    assert!(derivation.evidence.anti_overfit_gates_passed);
    assert!(derivation.evidence.content_complete);
    assert!(derivation.evidence.user_playtest_signed);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn r1_gate_derivation_blocks_without_manual_playtest_signature() {
    let root = temp_root("step14_v2_derive_unsigned");
    let stage_dir = root.join("stage14");
    std::fs::create_dir_all(&stage_dir).unwrap();
    write_standalone_release_evidence(&root, |document| document);
    write_json_file(
        &stage_dir.join("r1_pipeline_gate_evidence.json"),
        &serde_json::to_value(R1PipelineGateEvidence::all_passed_for_tests()).unwrap(),
    );

    let derivation = derive_r1_gate_evidence_from_sources(&root, &stage_dir);

    assert!(!derivation.evidence.user_playtest_signed);
    assert!(
        derivation
            .report
            .blockers
            .contains(&"r1_user_playtest_signature_missing".to_string())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn r1_gate_derivation_maps_failed_portable_smoke_to_exe_smoke_blocker() {
    let root = temp_root("step14_v2_derive_smoke_fail");
    let stage_dir = root.join("stage14");
    std::fs::create_dir_all(&stage_dir).unwrap();
    let evidence_id = write_standalone_release_evidence(&root, |mut document| {
        document["checks"]["portable_smoke"]["status"] = json!("failed");
        document["checks"]["portable_smoke"]["exitCode"] = json!(1);
        document
    });
    write_json_file(
        &stage_dir.join("r1_pipeline_gate_evidence.json"),
        &serde_json::to_value(R1PipelineGateEvidence::all_passed_for_tests()).unwrap(),
    );
    write_json_file(
        &stage_dir.join("r1_user_playtest_signature.json"),
        &serde_json::to_value(R1UserPlaytestSignature::manual_for_tests(&evidence_id)).unwrap(),
    );

    let derivation = derive_r1_gate_evidence_from_sources(&root, &stage_dir);

    assert!(!derivation.evidence.exe_smoke_passed);
    assert!(
        derivation
            .report
            .blockers
            .contains(&"standalone_release_check_missing_or_failed:portable_smoke".to_string())
    );
    let _ = std::fs::remove_dir_all(root);
}

fn passed_step13(spec: &GameSpec) -> Step13AcceptanceOutput {
    let spec_hash = canonicalize_game_spec(spec).unwrap().content_hash;
    Step13AcceptanceOutput {
        schema_version: "step13_acceptance_validation.v1".to_string(),
        compiler_version: STEP13_V2_COMPILER_VERSION.to_string(),
        status: Step13Status::Passed,
        spec_hash: spec_hash.clone(),
        build_hash: adm_new_foundation::sha256_hex(format!("{spec_hash}:build").as_bytes()),
        scenario_results: spec
            .acceptance_scenarios
            .iter()
            .map(|(scenario_id, scenario)| AcceptanceScenarioResult {
                scenario_id: scenario_id.to_string(),
                summary: scenario.summary.clone(),
                automation_kind: AutomationKind::Automated,
                status: ScenarioExecutionStatus::Passed,
                action_ids: scenario
                    .when
                    .iter()
                    .map(|action| action.action.to_string())
                    .collect(),
                spec_hash: spec_hash.clone(),
                build_hash: "r1-test-build".to_string(),
                log_hash: adm_new_foundation::sha256_hex(scenario_id.to_string().as_bytes()),
                performance_checks: Vec::new(),
                accessibility_checks: Vec::new(),
                failure_reason: None,
            })
            .collect(),
        output_paths: Default::default(),
    }
}

fn r1_fixture() -> GameSpec {
    parse_game_spec(include_str!(
        "../../../testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"
    ))
    .unwrap()
}

fn temp_root(prefix: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn write_standalone_release_evidence(
    root: &std::path::Path,
    mutate: impl FnOnce(Value) -> Value,
) -> String {
    let evidence_id = "1".repeat(32);
    let transaction_id = "2".repeat(32);
    let now = adm_new_foundation::unix_timestamp();
    let checks = required_release_checks()
        .into_iter()
        .map(|check| {
            (
                check.to_string(),
                json!({
                    "status": "passed",
                    "command": format!("fixture:{check}"),
                    "exitCode": 0,
                    "durationMs": 1,
                    "outputSha256": "a".repeat(64),
                }),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let document = json!({
        "schemaVersion": 2,
        "producer": "tools/verify-standalone.ps1/v2",
        "evidenceId": evidence_id,
        "projectId": "autodesignmaker-rust-v2",
        "status": "passed",
        "gitCommit": "b".repeat(40),
        "sourceTreeClean": true,
        "generatedAtUnix": now,
        "expiresAtUnix": now + 3600,
        "checks": checks,
        "portable": {
            "root": "dist/AutoDesignMaker-NEWrust-release",
            "executable": "AutoDesignMaker.exe",
            "executableSha256": "c".repeat(64),
            "buildManifestSha256": "d".repeat(64),
            "resourceManifestSha256": "e".repeat(64),
            "gitCommit": "b".repeat(40),
            "swapReceipt": format!("dist/.AutoDesignMaker-NEWrust-release.swap-{transaction_id}.json"),
            "swapReceiptSha256": "f".repeat(64),
            "transactionId": transaction_id,
            "transactionStatus": "finalized",
        },
        "errors": [],
    });
    write_json_file(
        &root.join("gates/standalone-release-evidence.json"),
        &mutate(document),
    );
    evidence_id
}

fn write_json_file(path: &std::path::Path, value: &Value) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn required_release_checks() -> Vec<&'static str> {
    vec![
        "cargo_fmt_check",
        "cargo_check_workspace",
        "cargo_test_workspace",
        "web_unit",
        "web_i18n",
        "web_design_content",
        "web_build",
        "web_e2e",
        "web_language_gate",
        "web_ui_gate",
        "web_ui_baseline_gate",
        "package_contract_self_test",
        "resource_manifest",
        "standalone_boundary_gate",
        "portable_build",
        "portable_smoke",
        "portable_integrity",
        "pe_architecture_crt",
        "clean_clone_relocation",
        "anti_fake_scan",
        "generated_cleanup",
    ]
}
