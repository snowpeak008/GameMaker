use std::collections::{BTreeMap, BTreeSet};

use adm_new_change_kernel::{
    ChangeFailureCategory, ChangeKernel, ChangeOutcome, EvidenceReference, ExpectedSpecValue,
    KernelHead, SpecPatch, SpecPatchOperation, SpecPatchSource, SpecStore, SpecValueChange,
    operation_evidence, patch_for_single_operation,
};
use adm_new_foundation::{AdmError, AdmResult};
use adm_new_game_spec::GameSpec;
use serde_json::Value;

use super::{
    CandidateSpecPatch, CompletionRisk, ImpactSummary, PromptPack, ValidatedCandidate,
    object_from_map,
};

const LOW_CONFIDENCE_THRESHOLD: f32 = 0.75;

pub fn validate_candidate(
    prompt_pack: &PromptPack,
    store: &SpecStore,
    candidate: CandidateSpecPatch,
    output_hash: Option<&str>,
) -> AdmResult<ValidatedCandidate> {
    let snapshot = store
        .snapshot()
        .map_err(|error| AdmError::new(error.to_string()))?;
    validate_base(prompt_pack, &snapshot.head, &candidate)?;
    validate_candidate_scope(prompt_pack, &candidate)?;
    let spec_patch = candidate_to_spec_patch(&candidate, output_hash);
    let impact = impact_summary(&snapshot.spec, &candidate);
    let risk = classify_risk(&impact, &candidate);
    let preflight = SpecStore::new(snapshot.spec)
        .map_err(|error| AdmError::new(error.to_string()))?
        .submit(spec_patch.clone())
        .map_err(|error| AdmError::new(error.to_string()))?;
    if preflight.audit.outcome != ChangeOutcome::Committed {
        let category = preflight
            .audit
            .failure_category
            .unwrap_or(ChangeFailureCategory::Input);
        return Err(AdmError::new(format!(
            "candidate rejected during deterministic preflight ({category:?}): {}",
            preflight
                .audit
                .failure_message
                .unwrap_or_else(|| "no failure message".to_string())
        )));
    }
    Ok(ValidatedCandidate {
        candidate,
        spec_patch,
        risk,
        impact,
        preflight_audit: preflight.audit,
    })
}

pub fn candidate_to_single_operation_patch(
    patch_id: impl Into<String>,
    base: &KernelHead,
    path: impl Into<String>,
    expected_old_value: ExpectedSpecValue,
    change: SpecValueChange,
    reason: impl Into<String>,
) -> SpecPatch {
    patch_for_single_operation(
        patch_id,
        base,
        SpecPatchOperation {
            path: path.into(),
            expected_old_value,
            change,
            source: SpecPatchSource::Human,
            reason: reason.into(),
            evidence: vec![operation_evidence(
                "manual_confirmation",
                b"human provided manual bounded completion patch",
            )],
        },
    )
}

pub(crate) fn candidate_from_json_map(
    map: BTreeMap<String, Value>,
) -> AdmResult<CandidateSpecPatch> {
    serde_json::from_value(object_from_map(map))
        .map_err(|error| AdmError::new(format!("candidate spec patch schema mismatch: {error}")))
}

fn validate_base(
    prompt_pack: &PromptPack,
    head: &KernelHead,
    candidate: &CandidateSpecPatch,
) -> AdmResult<()> {
    if candidate.base_revision != prompt_pack.base_revision
        || candidate.base_hash != prompt_pack.base_hash
    {
        return Err(AdmError::new(
            "candidate base does not match the PromptPack base",
        ));
    }
    if candidate.base_revision != head.revision || candidate.base_hash != head.content_hash {
        return Err(AdmError::new(
            "candidate base is stale against the current SpecStore head",
        ));
    }
    Ok(())
}

fn validate_candidate_scope(
    prompt_pack: &PromptPack,
    candidate: &CandidateSpecPatch,
) -> AdmResult<()> {
    if candidate.operations.is_empty() || candidate.declared_write_paths.is_empty() {
        return Err(AdmError::new(
            "candidate must declare at least one write path and operation",
        ));
    }
    let operation_paths = candidate
        .operations
        .iter()
        .map(|operation| operation.path.clone())
        .collect::<BTreeSet<_>>();
    if operation_paths != candidate.declared_write_paths {
        return Err(AdmError::new(
            "candidate declaredWritePaths must exactly match operation paths",
        ));
    }
    for path in &candidate.declared_write_paths {
        if !path.starts_with('/') || path == "/" || path.contains("..") {
            return Err(AdmError::new(format!(
                "candidate write path is not a valid non-root JSON Pointer: {path}"
            )));
        }
        if is_identity_path(path) {
            return Err(AdmError::new(format!(
                "candidate attempts to modify a kernel-protected field: {path}"
            )));
        }
        if !prompt_pack
            .allowed_write_paths
            .iter()
            .any(|allowed| pointer_contains(allowed, path))
        {
            return Err(AdmError::new(format!(
                "candidate write path is outside PromptPack allowedWritePaths: {path}"
            )));
        }
    }
    for operation in &candidate.operations {
        if operation.reason.trim().is_empty() {
            return Err(AdmError::new("candidate operation reason cannot be empty"));
        }
    }
    Ok(())
}

fn candidate_to_spec_patch(candidate: &CandidateSpecPatch, output_hash: Option<&str>) -> SpecPatch {
    let evidence = evidence_refs(candidate, output_hash);
    let operations = candidate
        .operations
        .iter()
        .map(|operation| SpecPatchOperation {
            path: operation.path.clone(),
            expected_old_value: operation.expected_old_value.clone(),
            change: operation.change.clone(),
            source: SpecPatchSource::Ai,
            reason: operation.reason.clone(),
            evidence: evidence.clone(),
        })
        .collect();
    SpecPatch {
        patch_id: candidate.patch_id.clone(),
        base_revision: candidate.base_revision,
        base_hash: candidate.base_hash.clone(),
        declared_write_paths: candidate.declared_write_paths.clone(),
        operations,
    }
}

fn evidence_refs(
    candidate: &CandidateSpecPatch,
    output_hash: Option<&str>,
) -> Vec<EvidenceReference> {
    let details = serde_json::json!({
        "patchId": candidate.patch_id,
        "modelOutputHash": output_hash.unwrap_or("missing_output_hash"),
        "evidenceSummary": candidate.evidence_summary,
    });
    let bytes = serde_json::to_vec(&details).unwrap_or_default();
    vec![operation_evidence("ai_candidate_output", &bytes)]
}

fn impact_summary(spec: &GameSpec, candidate: &CandidateSpecPatch) -> ImpactSummary {
    let changed_root_fields = candidate
        .declared_write_paths
        .iter()
        .filter_map(|path| path.trim_start_matches('/').split('/').next())
        .filter(|root| !root.is_empty())
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    let touches_protected_field = candidate
        .declared_write_paths
        .iter()
        .any(|path| is_identity_path(path) || pointer_contains("/technical/productEnvelope", path));
    let touches_product_envelope = candidate
        .declared_write_paths
        .iter()
        .any(|path| pointer_contains("/technical/productEnvelope", path));
    let low_confidence_operation_count = candidate
        .operations
        .iter()
        .filter(|operation| operation.confidence.unwrap_or(1.0) < LOW_CONFIDENCE_THRESHOLD)
        .count();
    ImpactSummary {
        operation_count: candidate.operations.len(),
        changed_root_fields,
        touches_protected_field,
        touches_product_envelope,
        low_confidence_operation_count,
        product_envelope: spec.technical.product_envelope.clone(),
    }
}

fn classify_risk(impact: &ImpactSummary, candidate: &CandidateSpecPatch) -> CompletionRisk {
    if impact.touches_protected_field
        || impact.touches_product_envelope
        || candidate
            .operations
            .iter()
            .any(|operation| matches!(operation.change, SpecValueChange::Remove))
    {
        return CompletionRisk::High;
    }
    if impact.operation_count > 3
        || impact.changed_root_fields.len() > 1
        || impact.low_confidence_operation_count > 0
    {
        return CompletionRisk::Medium;
    }
    CompletionRisk::Low
}

fn is_identity_path(path: &str) -> bool {
    pointer_contains("/identity", path)
}

fn pointer_contains(parent: &str, candidate: &str) -> bool {
    candidate == parent
        || candidate
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with('/'))
}
