use std::collections::BTreeSet;

use adm_new_change_kernel::{
    ChangeEvidence, ChangeFailureCategory, ChangeOutcome, CommandPermission, CommandPurpose,
    EvidenceStatus, SideEffectState, TrustedTestContract, WORKSPACE_CHANGE_SET_SCHEMA_VERSION,
    WorkspaceChangeSet, WorkspaceFileExpectation, WorkspaceFilePayload, WorkspaceOperation,
    WorkspaceRelativePath, WorkspaceResourceBudget, WorkspaceTransactionResult,
};

fn path(value: &str) -> WorkspaceRelativePath {
    WorkspaceRelativePath::parse(value).unwrap()
}

fn hash(character: char) -> String {
    std::iter::repeat_n(character, 64).collect()
}

fn evidence() -> Vec<ChangeEvidence> {
    vec![ChangeEvidence::from_bytes(
        "task_contract",
        "contract",
        EvidenceStatus::Observed,
        b"reviewed task contract",
    )]
}

fn valid_contract() -> WorkspaceChangeSet {
    WorkspaceChangeSet {
        schema_version: WORKSPACE_CHANGE_SET_SCHEMA_VERSION.to_string(),
        change_set_id: "workspace_change_01".to_string(),
        base_tree_hash: hash('a'),
        read_paths: [path("Assets/Scripts"), path("Tests/Trusted.cs")]
            .into_iter()
            .collect(),
        agent_write_paths: [
            path("Assets/Scripts/Game.cs"),
            path("Assets/Data/payload.bin"),
            path("Assets/Scripts/Old.cs"),
            path("Assets/Scripts/Move.cs"),
            path("Assets/Scripts/Moved.cs"),
        ]
        .into_iter()
        .collect(),
        trusted_tool_write_paths: [path("Assets/Scripts/Game.cs.meta")].into_iter().collect(),
        build_output_paths: [path("Build")].into_iter().collect(),
        operations: vec![
            WorkspaceOperation::WriteFile {
                path: path("Assets/Scripts/Game.cs"),
                expected: WorkspaceFileExpectation::Sha256 { value: hash('b') },
                payload: WorkspaceFilePayload::utf8("public class Game {}"),
            },
            WorkspaceOperation::WriteFile {
                path: path("Assets/Data/payload.bin"),
                expected: WorkspaceFileExpectation::Missing,
                payload: WorkspaceFilePayload::binary(vec![0, 1, 2, 255]),
            },
            WorkspaceOperation::DeleteFile {
                path: path("Assets/Scripts/Old.cs"),
                expected_sha256: hash('c'),
            },
            WorkspaceOperation::RenameFile {
                from: path("Assets/Scripts/Move.cs"),
                to: path("Assets/Scripts/Moved.cs"),
                expected_source_sha256: hash('d'),
                expected_target: WorkspaceFileExpectation::Missing,
            },
        ],
        command_permissions: vec![CommandPermission {
            command_id: "trusted_test".to_string(),
            tool_binding_id: "unity_batch".to_string(),
            purpose: CommandPurpose::Test,
            argument_template: vec!["--project={workspace}".to_string()],
            working_directory: None,
            timeout_ms: 10_000,
            allow_network: false,
        }],
        trusted_tests: vec![TrustedTestContract {
            test_id: "trusted_runtime_test".to_string(),
            path: path("Tests/Trusted.cs"),
            baseline_sha256: hash('e'),
            command_id: "trusted_test".to_string(),
        }],
        resource_budget: WorkspaceResourceBudget {
            max_duration_ms: 30_000,
            max_processes: 2,
            max_write_bytes: 1_000_000,
            max_file_count: 20,
            max_retries: 2,
        },
        evidence: evidence(),
    }
}

#[test]
fn valid_contract_covers_text_binary_delete_rename_and_derived_outputs() {
    let contract = valid_contract();
    let report = contract.validate();
    assert!(report.is_valid(), "{:?}", report.issues);
    let encoded = serde_json::to_string(&contract).unwrap();
    let decoded: WorkspaceChangeSet = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, contract);
}

#[test]
fn workspace_paths_reject_absolute_parent_and_noncanonical_forms() {
    let drive_path = ["C", ":/", "project/file.cs"].concat();
    let parent_path = ["project/", "..", "/file.cs"].concat();
    for invalid in [
        drive_path,
        "/project/file.cs".to_string(),
        parent_path,
        "project\\file.cs".to_string(),
        "project//file.cs".to_string(),
        "./file.cs".to_string(),
    ] {
        assert!(
            WorkspaceRelativePath::parse(&invalid).is_err(),
            "accepted {invalid}"
        );
    }
}

#[test]
fn workspace_paths_are_case_folded_before_scope_comparison() {
    let upper = WorkspaceRelativePath::parse("SRC/Game.cs").unwrap();
    let lower = WorkspaceRelativePath::parse("src/game.cs").unwrap();
    let parent = WorkspaceRelativePath::parse("SRC").unwrap();

    assert_eq!(upper, lower);
    assert_eq!(upper.as_str(), "src/game.cs");
    assert!(parent.contains(&lower));

    let mut contract = valid_contract();
    contract
        .agent_write_paths
        .insert(WorkspaceRelativePath::parse("SRC").unwrap());
    contract
        .trusted_tool_write_paths
        .insert(WorkspaceRelativePath::parse("src/game.cs").unwrap());
    let report = contract.validate();
    assert!(report.contains_code("workspace_change_set.attribution_overlap"));
}

#[test]
fn agent_operations_outside_declared_scope_are_rejected() {
    let mut contract = valid_contract();
    contract.operations.push(WorkspaceOperation::WriteFile {
        path: path("ProjectSettings/Settings.asset"),
        expected: WorkspaceFileExpectation::Missing,
        payload: WorkspaceFilePayload::utf8("settings"),
    });
    let report = contract.validate();
    assert!(report.contains_code("workspace_change_set.operation_outside_agent_scope"));
    assert!(report.issues.iter().any(|issue| {
        issue.category == ChangeFailureCategory::ScopeViolation && issue.path == "/operations/4"
    }));
}

#[test]
fn trusted_tests_cannot_overlap_agent_write_scope() {
    let mut contract = valid_contract();
    contract.agent_write_paths.insert(path("Tests"));
    let report = contract.validate();
    assert!(report.contains_code("workspace_change_set.trusted_test_writable"));
}

#[test]
fn agent_tool_and_build_attribution_sets_must_be_disjoint() {
    let mut contract = valid_contract();
    contract
        .trusted_tool_write_paths
        .insert(path("Assets/Scripts"));
    let report = contract.validate();
    assert!(report.contains_code("workspace_change_set.attribution_overlap"));
}

#[test]
fn tampered_text_or_binary_payload_hash_is_rejected_as_evidence_failure() {
    let mut contract = valid_contract();
    if let WorkspaceOperation::WriteFile { payload, .. } = &mut contract.operations[0] {
        *payload = WorkspaceFilePayload::Utf8 {
            content: "tampered".to_string(),
            sha256: hash('f'),
        };
    }
    let report = contract.validate();
    assert!(report.contains_code("workspace_change_set.payload_hash_mismatch"));
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.category == ChangeFailureCategory::Evidence)
    );
}

#[test]
fn command_contracts_store_binding_ids_not_machine_paths() {
    let mut contract = valid_contract();
    contract.command_permissions[0].tool_binding_id = ["C", ":/", "Tools/Unity.exe"].concat();
    contract.command_permissions[0]
        .argument_template
        .push(["..", "/outside"].concat());
    let report = contract.validate();
    assert!(report.contains_code("workspace_change_set.invalid_command_id"));
    assert!(report.contains_code("workspace_change_set.machine_path_argument"));
}

#[test]
fn command_contracts_reject_embedded_credentials() {
    let mut contract = valid_contract();
    contract.command_permissions[0]
        .argument_template
        .push("--api_key=not-allowed".to_string());
    assert!(
        contract
            .validate()
            .contains_code("workspace_change_set.sensitive_command_argument")
    );
}

fn result_for(
    contract: &WorkspaceChangeSet,
    category: ChangeFailureCategory,
) -> WorkspaceTransactionResult {
    WorkspaceTransactionResult {
        schema_version: WORKSPACE_CHANGE_SET_SCHEMA_VERSION.to_string(),
        change_set_id: contract.change_set_id.clone(),
        contract_sha256: contract.contract_hash().unwrap(),
        base_tree_hash: contract.base_tree_hash.clone(),
        outcome: ChangeOutcome::Rejected,
        failure_category: Some(category),
        side_effect_state: SideEffectState::None,
        stage: "isolated_validation".to_string(),
        resulting_tree_hash: None,
        agent_changed_paths: BTreeSet::new(),
        trusted_tool_changed_paths: BTreeSet::new(),
        build_output_changed_paths: BTreeSet::new(),
        trusted_test_hashes: contract
            .trusted_tests
            .iter()
            .map(|test| (test.test_id.clone(), test.baseline_sha256.clone()))
            .collect(),
        evidence: evidence(),
    }
}

#[test]
fn all_r0_failure_categories_round_trip_with_retry_semantics() {
    let contract = valid_contract();
    for category in [
        ChangeFailureCategory::Input,
        ChangeFailureCategory::AgentError,
        ChangeFailureCategory::ScopeViolation,
        ChangeFailureCategory::Compile,
        ChangeFailureCategory::Test,
        ChangeFailureCategory::Timeout,
        ChangeFailureCategory::Tooling,
        ChangeFailureCategory::Evidence,
    ] {
        let result = result_for(&contract, category);
        assert!(result.validate_against(&contract).is_valid());
        let encoded = serde_json::to_string(&result).unwrap();
        assert!(encoded.contains(category.as_str()));
        let decoded: WorkspaceTransactionResult = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, result);
    }
    assert_eq!(
        ChangeFailureCategory::Compile.retry_disposition(),
        adm_new_change_kernel::RetryDisposition::ExecutionBudget
    );
    assert_eq!(
        ChangeFailureCategory::ScopeViolation.retry_disposition(),
        adm_new_change_kernel::RetryDisposition::Never
    );
}

#[test]
fn post_commit_compile_failure_preserves_side_effect_and_tree_evidence() {
    let contract = valid_contract();
    let mut result = result_for(&contract, ChangeFailureCategory::Compile);
    result.stage = "compile".to_string();
    result.side_effect_state = SideEffectState::CommittedRecoveryBlocked;
    result.resulting_tree_hash = Some(hash('f'));
    result
        .agent_changed_paths
        .insert(path("Assets/Scripts/Game.cs"));
    result
        .trusted_tool_changed_paths
        .insert(path("Assets/Scripts/Game.cs.meta"));
    assert!(result.validate_against(&contract).is_valid());
}

#[test]
fn transaction_result_must_rehash_every_trusted_test() {
    let contract = valid_contract();
    let mut result = result_for(&contract, ChangeFailureCategory::Test);
    result
        .trusted_test_hashes
        .insert("trusted_runtime_test".to_string(), hash('f'));
    assert!(
        result
            .validate_against(&contract)
            .contains_code("workspace_result.trusted_test_changed")
    );
    result.trusted_test_hashes.clear();
    assert!(
        result
            .validate_against(&contract)
            .contains_code("workspace_result.trusted_test_hash_missing")
    );
}

#[test]
fn scope_or_evidence_failure_can_never_claim_committed_side_effects() {
    let contract = valid_contract();
    for category in [
        ChangeFailureCategory::ScopeViolation,
        ChangeFailureCategory::Evidence,
    ] {
        let mut result = result_for(&contract, category);
        result.side_effect_state = SideEffectState::Committed;
        result.resulting_tree_hash = Some(hash('f'));
        assert!(
            result
                .validate_against(&contract)
                .contains_code("workspace_result.forbidden_failure_side_effect")
        );
    }
}

#[test]
fn unknown_contract_fields_fail_closed() {
    let mut value = serde_json::to_value(valid_contract()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .insert("unreviewedField".to_string(), serde_json::json!(true));
    assert!(serde_json::from_value::<WorkspaceChangeSet>(value).is_err());
}
