use std::path::PathBuf;
use std::sync::{Arc, Barrier};

use adm_new_change_kernel::{
    ChangeEvidence, ChangeFailureCategory, ChangeKernel, ChangeOutcome, EvidenceStatus,
    ExpectedSpecValue, SideEffectState, SpecPatch, SpecPatchOperation, SpecPatchSource,
    SpecPatchValidator, SpecStore, SpecValidatorDecision, SpecValueChange, committed_hash_chain,
    operation_evidence, patch_for_single_operation,
};
use adm_new_game_spec::{GameSpec, parse_game_spec};
use serde_json::Value;

fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("change kernel crate must be under the workspace crates directory")
        .to_path_buf()
}

fn fixture() -> GameSpec {
    let source = std::fs::read_to_string(project_root().join("testdata/game_spec/lane_guard.json"))
        .expect("fixture must exist");
    parse_game_spec(&source).expect("fixture must parse")
}

fn evidence() -> Vec<adm_new_change_kernel::EvidenceReference> {
    vec![operation_evidence(
        "reviewed_decision",
        b"reviewed deterministic input",
    )]
}

fn operation(
    path: &str,
    expected_old_value: ExpectedSpecValue,
    change: SpecValueChange,
    source: SpecPatchSource,
    reason: &str,
) -> SpecPatchOperation {
    SpecPatchOperation {
        path: path.to_string(),
        expected_old_value,
        change,
        source,
        reason: reason.to_string(),
        evidence: evidence(),
    }
}

fn title_patch(
    store: &SpecStore,
    patch_id: &str,
    expected: &str,
    replacement: &str,
    source: SpecPatchSource,
) -> SpecPatch {
    patch_for_single_operation(
        patch_id,
        &store.head().unwrap(),
        operation(
            "/intent/title",
            ExpectedSpecValue::Exact {
                value: Value::String(expected.to_string()),
            },
            SpecValueChange::Set {
                value: Value::String(replacement.to_string()),
            },
            source,
            "update the reviewed display title",
        ),
    )
}

#[test]
fn commit_updates_revision_parent_hash_and_immutable_audit() {
    let store = SpecStore::new(fixture()).unwrap();
    let base = store.head().unwrap();
    let receipt = store
        .submit(title_patch(
            &store,
            "title_patch_01",
            "Lane Guard Sample",
            "Reviewed Project",
            SpecPatchSource::Human,
        ))
        .unwrap();
    assert!(receipt.committed());
    assert_eq!(receipt.audit.side_effect_state, SideEffectState::Committed);
    assert_eq!(receipt.audit.claimed_base, base);

    let snapshot = store.snapshot().unwrap();
    assert_eq!(snapshot.head.revision, base.revision + 1);
    assert_ne!(snapshot.head.content_hash, base.content_hash);
    assert_eq!(snapshot.spec.identity.parent_hash, Some(base.content_hash));
    assert_eq!(snapshot.spec.intent.title, "Reviewed Project");

    let mut detached_log = store.audit_log().unwrap();
    assert_eq!(detached_log.len(), 1);
    detached_log.clear();
    assert_eq!(store.audit_log().unwrap().len(), 1);
    let domain_audit = store.spec_audit_log().unwrap();
    assert_eq!(
        domain_audit[0].patch.operations[0].source,
        SpecPatchSource::Human
    );
    assert_eq!(
        domain_audit[0].patch.operations[0].reason,
        "update the reviewed display title"
    );
    assert_eq!(
        domain_audit[0].transaction.change_digest_sha256,
        receipt.audit.change_digest_sha256
    );
}

#[test]
fn stale_patch_is_rejected_and_audited_without_side_effects() {
    let store = SpecStore::new(fixture()).unwrap();
    let stale = title_patch(
        &store,
        "stale_title",
        "Lane Guard Sample",
        "Stale",
        SpecPatchSource::Template,
    );
    let winner = title_patch(
        &store,
        "winning_title",
        "Lane Guard Sample",
        "Winner",
        SpecPatchSource::Human,
    );
    assert!(store.submit(winner).unwrap().committed());
    let rejected = store.submit(stale).unwrap();
    assert_eq!(rejected.audit.outcome, ChangeOutcome::Rejected);
    assert_eq!(
        rejected.audit.failure_category,
        Some(ChangeFailureCategory::Conflict)
    );
    assert_eq!(rejected.audit.side_effect_state, SideEffectState::None);
    assert_eq!(store.snapshot().unwrap().spec.intent.title, "Winner");
    assert_eq!(store.audit_log().unwrap().len(), 2);
}

#[test]
fn declaration_mismatch_is_a_scope_violation() {
    let store = SpecStore::new(fixture()).unwrap();
    let base = store.head().unwrap();
    let mut patch = title_patch(
        &store,
        "scope_violation",
        "Lane Guard Sample",
        "Out of scope",
        SpecPatchSource::Ai,
    );
    patch.declared_write_paths = ["/intent/summary".to_string()].into_iter().collect();
    let receipt = store.submit(patch).unwrap();
    assert_eq!(
        receipt.audit.failure_category,
        Some(ChangeFailureCategory::ScopeViolation)
    );
    assert_eq!(store.head().unwrap(), base);
    assert_eq!(
        store.snapshot().unwrap().spec.intent.title,
        "Lane Guard Sample"
    );
}

#[test]
fn semantic_validation_failure_rolls_back_the_whole_patch() {
    let store = SpecStore::new(fixture()).unwrap();
    let base = store.snapshot().unwrap();
    let entity = serde_json::to_value(
        base.spec
            .entities
            .iter()
            .find(|(id, _)| id.as_str() == "defender")
            .expect("fixture defender")
            .1,
    )
    .unwrap();
    let patch = patch_for_single_operation(
        "remove_referenced_entity",
        &base.head,
        operation(
            "/entities/defender",
            ExpectedSpecValue::Exact { value: entity },
            SpecValueChange::Remove,
            SpecPatchSource::Migration,
            "mutation test must be rejected",
        ),
    );
    let receipt = store.submit(patch).unwrap();
    assert_eq!(
        receipt.audit.failure_category,
        Some(ChangeFailureCategory::Input)
    );
    assert_eq!(
        receipt.audit.failure_code.as_deref(),
        Some("spec_store.validation_failed")
    );
    assert_eq!(store.snapshot().unwrap(), base);
}

#[test]
fn structurally_invalid_candidate_is_rejected_with_an_audit_record() {
    let store = SpecStore::new(fixture()).unwrap();
    let base = store.snapshot().unwrap();
    let time = serde_json::to_value(&base.spec.time).unwrap();
    let patch = patch_for_single_operation(
        "remove_required_time",
        &base.head,
        operation(
            "/time",
            ExpectedSpecValue::Exact { value: time },
            SpecValueChange::Remove,
            SpecPatchSource::Migration,
            "mutation test removes a required root field",
        ),
    );
    let receipt = store.submit(patch).unwrap();
    assert_eq!(
        receipt.audit.failure_code.as_deref(),
        Some("spec_store.candidate_deserialization_failed")
    );
    assert_eq!(receipt.audit.side_effect_state, SideEffectState::None);
    assert_eq!(store.snapshot().unwrap(), base);
    assert_eq!(store.spec_audit_log().unwrap().len(), 1);
}

#[test]
fn old_value_preconditions_and_kernel_managed_fields_are_enforced() {
    let store = SpecStore::new(fixture()).unwrap();
    let base = store.snapshot().unwrap();
    let wrong_expected = title_patch(
        &store,
        "wrong_expected",
        "Different title",
        "Rejected",
        SpecPatchSource::Human,
    );
    let receipt = store.submit(wrong_expected).unwrap();
    assert_eq!(
        receipt.audit.failure_code.as_deref(),
        Some("spec_store.old_value_mismatch")
    );

    let managed = patch_for_single_operation(
        "managed_revision",
        &base.head,
        operation(
            "/identity/revision",
            ExpectedSpecValue::Exact {
                value: serde_json::json!(base.head.revision),
            },
            SpecValueChange::Set {
                value: serde_json::json!(base.head.revision + 1),
            },
            SpecPatchSource::Ai,
            "attempt to bypass the single writer",
        ),
    );
    let receipt = store.submit(managed).unwrap();
    assert_eq!(
        receipt.audit.failure_category,
        Some(ChangeFailureCategory::ScopeViolation)
    );
    assert_eq!(store.snapshot().unwrap(), base);
}

#[test]
fn missing_evidence_is_rejected_and_recorded() {
    let store = SpecStore::new(fixture()).unwrap();
    let mut patch = title_patch(
        &store,
        "missing_evidence",
        "Lane Guard Sample",
        "No evidence",
        SpecPatchSource::Ai,
    );
    patch.operations[0].evidence.clear();
    let receipt = store.submit(patch).unwrap();
    assert_eq!(
        receipt.audit.failure_category,
        Some(ChangeFailureCategory::Evidence)
    );
    assert_eq!(receipt.audit.side_effect_state, SideEffectState::None);
    assert!(!receipt.audit.evidence.is_empty());
}

#[test]
fn concurrent_writers_allow_exactly_one_commit() {
    let store = Arc::new(SpecStore::new(fixture()).unwrap());
    let base = store.head().unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for (id, title) in [("writer_a", "Writer A"), ("writer_b", "Writer B")] {
        let store = Arc::clone(&store);
        let barrier = Arc::clone(&barrier);
        let patch = patch_for_single_operation(
            id,
            &base,
            operation(
                "/intent/title",
                ExpectedSpecValue::Exact {
                    value: Value::String("Lane Guard Sample".to_string()),
                },
                SpecValueChange::Set {
                    value: Value::String(title.to_string()),
                },
                SpecPatchSource::Human,
                "concurrent compare-and-swap test",
            ),
        );
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            store.submit(patch).unwrap()
        }));
    }
    barrier.wait();
    let receipts = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        receipts
            .iter()
            .filter(|receipt| receipt.committed())
            .count(),
        1
    );
    assert_eq!(
        receipts
            .iter()
            .filter(|receipt| {
                receipt.audit.failure_category == Some(ChangeFailureCategory::Conflict)
            })
            .count(),
        1
    );
    assert_eq!(store.head().unwrap().revision, base.revision + 1);
    assert_eq!(store.audit_log().unwrap().len(), 2);
}

fn replay_sequence() -> (Vec<(u64, String)>, Vec<String>) {
    let store = SpecStore::new(fixture()).unwrap();
    store
        .submit(title_patch(
            &store,
            "replay_title",
            "Lane Guard Sample",
            "Replay Title",
            SpecPatchSource::Template,
        ))
        .unwrap();
    let summary = store.snapshot().unwrap().spec.intent.summary;
    store
        .submit(patch_for_single_operation(
            "replay_summary",
            &store.head().unwrap(),
            operation(
                "/intent/summary",
                ExpectedSpecValue::Exact {
                    value: Value::String(summary),
                },
                SpecValueChange::Set {
                    value: Value::String("Deterministic replay summary".to_string()),
                },
                SpecPatchSource::Human,
                "deterministic replay second patch",
            ),
        ))
        .unwrap();
    let audit = store.audit_log().unwrap();
    (
        committed_hash_chain(&audit).into_iter().collect(),
        audit.into_iter().map(|record| record.record_id).collect(),
    )
}

#[test]
fn replay_produces_the_same_revision_hash_and_audit_chains() {
    assert_eq!(replay_sequence(), replay_sequence());
}

struct RejectingValidator;

impl SpecPatchValidator for RejectingValidator {
    fn validate(
        &self,
        _base: &GameSpec,
        _candidate: &GameSpec,
        _patch: &SpecPatch,
    ) -> Result<SpecValidatorDecision, adm_new_change_kernel::ChangeKernelError> {
        Ok(SpecValidatorDecision::rejected(
            ChangeFailureCategory::Input,
            "test.policy_rejected",
            "deterministic test validator rejected the candidate",
            ChangeEvidence::from_bytes(
                "test_policy",
                "pre_commit",
                EvidenceStatus::Failed,
                b"rejected",
            ),
        ))
    }
}

#[test]
fn deterministic_validation_hooks_run_before_commit() {
    let store = SpecStore::with_validators(fixture(), vec![Arc::new(RejectingValidator)]).unwrap();
    let base = store.snapshot().unwrap();
    let receipt = store
        .submit(title_patch(
            &store,
            "hook_rejection",
            "Lane Guard Sample",
            "Rejected",
            SpecPatchSource::Ai,
        ))
        .unwrap();
    assert_eq!(
        receipt.audit.failure_code.as_deref(),
        Some("test.policy_rejected")
    );
    assert_eq!(store.snapshot().unwrap(), base);
}

#[test]
fn every_source_uses_the_same_submission_path() {
    for source in [
        SpecPatchSource::Human,
        SpecPatchSource::Template,
        SpecPatchSource::Migration,
        SpecPatchSource::Ai,
    ] {
        let store = SpecStore::new(fixture()).unwrap();
        assert!(
            store
                .submit(title_patch(
                    &store,
                    "unified_source",
                    "Lane Guard Sample",
                    "Unified",
                    source,
                ))
                .unwrap()
                .committed()
        );
    }
}
