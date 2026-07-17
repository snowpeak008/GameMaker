use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};

use adm_new_game_spec::{GameSpec, canonicalize_game_spec, validate_game_spec};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::{
    ChangeAuditRecord, ChangeEvidence, ChangeFailureCategory, ChangeKernel, ChangeKernelError,
    ChangeOutcome, ChangeReceipt, EvidenceStatus, KernelHead, SideEffectState, is_sha256,
    is_stable_id,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecPatchSource {
    Human,
    Template,
    Migration,
    Ai,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceReference {
    pub evidence_id: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ExpectedSpecValue {
    Absent,
    Exact { value: Value },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SpecValueChange {
    Set { value: Value },
    Remove,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecPatchOperation {
    pub path: String,
    pub expected_old_value: ExpectedSpecValue,
    pub change: SpecValueChange,
    pub source: SpecPatchSource,
    pub reason: String,
    pub evidence: Vec<EvidenceReference>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecPatch {
    pub patch_id: String,
    pub base_revision: u64,
    pub base_hash: String,
    pub declared_write_paths: BTreeSet<String>,
    pub operations: Vec<SpecPatchOperation>,
}

impl SpecPatch {
    pub fn claimed_base(&self) -> KernelHead {
        KernelHead {
            revision: self.base_revision,
            content_hash: self.base_hash.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpecSnapshot {
    pub head: KernelHead,
    pub spec: GameSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecAuditRecord {
    pub transaction: ChangeAuditRecord,
    pub patch: SpecPatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecValidatorDecision {
    pub accepted: bool,
    pub failure_category: Option<ChangeFailureCategory>,
    pub failure_code: Option<String>,
    pub failure_message: Option<String>,
    pub evidence: ChangeEvidence,
}

impl SpecValidatorDecision {
    pub fn passed(evidence: ChangeEvidence) -> Self {
        Self {
            accepted: true,
            failure_category: None,
            failure_code: None,
            failure_message: None,
            evidence,
        }
    }

    pub fn rejected(
        category: ChangeFailureCategory,
        code: impl Into<String>,
        message: impl Into<String>,
        evidence: ChangeEvidence,
    ) -> Self {
        Self {
            accepted: false,
            failure_category: Some(category),
            failure_code: Some(code.into()),
            failure_message: Some(message.into()),
            evidence,
        }
    }
}

pub trait SpecPatchValidator: Send + Sync {
    fn validate(
        &self,
        base: &GameSpec,
        candidate: &GameSpec,
        patch: &SpecPatch,
    ) -> Result<SpecValidatorDecision, ChangeKernelError>;
}

struct SpecStoreState {
    snapshot: SpecSnapshot,
    audit: Vec<SpecAuditRecord>,
}

pub struct SpecStore {
    state: Mutex<SpecStoreState>,
    validators: Vec<Arc<dyn SpecPatchValidator>>,
}

impl SpecStore {
    pub fn new(initial: GameSpec) -> Result<Self, ChangeKernelError> {
        Self::with_validators(initial, Vec::new())
    }

    pub fn with_validators(
        initial: GameSpec,
        validators: Vec<Arc<dyn SpecPatchValidator>>,
    ) -> Result<Self, ChangeKernelError> {
        let report = validate_game_spec(&initial);
        if !report.is_valid() {
            return Err(ChangeKernelError::new(
                "spec_store.invalid_initial_spec",
                format!(
                    "initial GameSpec failed deterministic validation with {} error(s)",
                    report.error_count()
                ),
            ));
        }
        let canonical = canonicalize_game_spec(&initial).map_err(|error| {
            ChangeKernelError::new(
                "spec_store.initial_hash_failed",
                format!("initial GameSpec could not be canonicalized: {error}"),
            )
        })?;
        Ok(Self {
            state: Mutex::new(SpecStoreState {
                snapshot: SpecSnapshot {
                    head: KernelHead {
                        revision: initial.identity.revision,
                        content_hash: canonical.content_hash,
                    },
                    spec: initial,
                },
                audit: Vec::new(),
            }),
            validators,
        })
    }

    pub fn snapshot(&self) -> Result<SpecSnapshot, ChangeKernelError> {
        Ok(self.lock_state()?.snapshot.clone())
    }

    pub fn spec_audit_log(&self) -> Result<Vec<SpecAuditRecord>, ChangeKernelError> {
        Ok(self.lock_state()?.audit.clone())
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, SpecStoreState>, ChangeKernelError> {
        self.state.lock().map_err(|_| {
            ChangeKernelError::new(
                "change_kernel.writer_poisoned",
                "the single-writer transaction lock is poisoned",
            )
        })
    }

    fn reject(
        state: &mut SpecStoreState,
        patch: &SpecPatch,
        rejection: PatchRejection,
        mut evidence: Vec<ChangeEvidence>,
    ) -> Result<ChangeReceipt, ChangeKernelError> {
        evidence.push(ChangeEvidence::from_serializable(
            "kernel_rejection",
            "pre_commit",
            EvidenceStatus::Failed,
            &json!({
                "category": rejection.category,
                "code": rejection.code,
                "message": rejection.message,
            }),
        )?);
        let audit = ChangeAuditRecord {
            sequence: state.audit.len() as u64 + 1,
            record_id: String::new(),
            change_id: stable_change_id(&patch.patch_id),
            change_digest_sha256: spec_patch_digest(patch)?,
            claimed_base: patch.claimed_base(),
            observed_base: state.snapshot.head.clone(),
            outcome: ChangeOutcome::Rejected,
            failure_category: Some(rejection.category),
            failure_code: Some(rejection.code),
            failure_message: Some(rejection.message),
            side_effect_state: SideEffectState::None,
            committed_head: None,
            evidence,
        }
        .seal()?;
        state.audit.push(SpecAuditRecord {
            transaction: audit.clone(),
            patch: patch.clone(),
        });
        Ok(ChangeReceipt { audit })
    }
}

impl ChangeKernel<SpecPatch> for SpecStore {
    fn head(&self) -> Result<KernelHead, ChangeKernelError> {
        Ok(self.lock_state()?.snapshot.head.clone())
    }

    fn submit(&self, patch: SpecPatch) -> Result<ChangeReceipt, ChangeKernelError> {
        let mut state = self.lock_state()?;
        if let Err(rejection) = validate_patch_shape(&patch) {
            return Self::reject(&mut state, &patch, rejection, Vec::new());
        }
        if patch.base_revision != state.snapshot.head.revision
            || patch.base_hash != state.snapshot.head.content_hash
        {
            return Self::reject(
                &mut state,
                &patch,
                PatchRejection::new(
                    ChangeFailureCategory::Conflict,
                    "spec_store.stale_patch",
                    "patch base revision or hash does not match the current head",
                ),
                Vec::new(),
            );
        }

        let base_value = serde_json::to_value(&state.snapshot.spec).map_err(|error| {
            ChangeKernelError::new(
                "spec_store.snapshot_serialization_failed",
                format!("failed to serialize current GameSpec: {error}"),
            )
        })?;
        let mut candidate_value = base_value.clone();
        for operation in &patch.operations {
            if let Err(rejection) = apply_operation(&mut candidate_value, operation) {
                return Self::reject(&mut state, &patch, rejection, Vec::new());
            }
        }

        let mut changed_paths = BTreeSet::new();
        collect_changed_paths(&base_value, &candidate_value, "", &mut changed_paths);
        if changed_paths.is_empty() {
            return Self::reject(
                &mut state,
                &patch,
                PatchRejection::new(
                    ChangeFailureCategory::Input,
                    "spec_store.no_effect_patch",
                    "patch operations do not change the GameSpec",
                ),
                Vec::new(),
            );
        }
        if let Some(path) = changed_paths.iter().find(|changed| {
            !patch
                .declared_write_paths
                .iter()
                .any(|declared| pointer_contains(declared, changed))
        }) {
            return Self::reject(
                &mut state,
                &patch,
                PatchRejection::new(
                    ChangeFailureCategory::ScopeViolation,
                    "spec_store.changed_path_outside_scope",
                    format!("candidate changed undeclared path '{path}'"),
                ),
                Vec::new(),
            );
        }

        let mut candidate: GameSpec = match serde_json::from_value(candidate_value) {
            Ok(candidate) => candidate,
            Err(error) => {
                let evidence = vec![ChangeEvidence::from_bytes(
                    "game_spec_deserialization",
                    "pre_commit",
                    EvidenceStatus::Failed,
                    error.to_string().as_bytes(),
                )];
                return Self::reject(
                    &mut state,
                    &patch,
                    PatchRejection::new(
                        ChangeFailureCategory::Input,
                        "spec_store.candidate_deserialization_failed",
                        format!("candidate GameSpec could not be deserialized: {error}"),
                    ),
                    evidence,
                );
            }
        };
        candidate.identity.revision =
            state.snapshot.head.revision.checked_add(1).ok_or_else(|| {
                ChangeKernelError::new(
                    "spec_store.revision_overflow",
                    "GameSpec revision cannot be incremented",
                )
            })?;
        candidate.identity.parent_hash = Some(state.snapshot.head.content_hash.clone());

        let report = validate_game_spec(&candidate);
        let mut evidence = vec![ChangeEvidence::from_serializable(
            "game_spec_validation",
            "pre_commit",
            if report.is_valid() {
                EvidenceStatus::Passed
            } else {
                EvidenceStatus::Failed
            },
            &report,
        )?];
        if !report.is_valid() {
            return Self::reject(
                &mut state,
                &patch,
                PatchRejection::new(
                    ChangeFailureCategory::Input,
                    "spec_store.validation_failed",
                    format!(
                        "candidate GameSpec failed deterministic validation with {} error(s)",
                        report.error_count()
                    ),
                ),
                evidence,
            );
        }

        for validator in &self.validators {
            let decision = match validator.validate(&state.snapshot.spec, &candidate, &patch) {
                Ok(decision) => decision,
                Err(error) => {
                    evidence.push(ChangeEvidence::from_bytes(
                        "validation_hook_error",
                        "pre_commit",
                        EvidenceStatus::Failed,
                        error.to_string().as_bytes(),
                    ));
                    return Self::reject(
                        &mut state,
                        &patch,
                        PatchRejection::new(
                            ChangeFailureCategory::Evidence,
                            "spec_store.validation_hook_failed",
                            "a validation hook failed without an accepted decision",
                        ),
                        evidence,
                    );
                }
            };
            evidence.push(decision.evidence);
            if !decision.accepted {
                return Self::reject(
                    &mut state,
                    &patch,
                    PatchRejection::new(
                        decision
                            .failure_category
                            .unwrap_or(ChangeFailureCategory::Evidence),
                        decision
                            .failure_code
                            .unwrap_or_else(|| "spec_store.validator_rejected".to_string()),
                        decision.failure_message.unwrap_or_else(|| {
                            "a deterministic validation hook rejected the candidate".to_string()
                        }),
                    ),
                    evidence,
                );
            }
        }

        let canonical = canonicalize_game_spec(&candidate).map_err(|error| {
            ChangeKernelError::new(
                "spec_store.candidate_hash_failed",
                format!("candidate GameSpec could not be canonicalized: {error}"),
            )
        })?;
        evidence.push(ChangeEvidence::from_bytes(
            "canonical_game_spec",
            "pre_commit",
            EvidenceStatus::Passed,
            canonical.json.as_bytes(),
        ));
        let committed_head = KernelHead {
            revision: candidate.identity.revision,
            content_hash: canonical.content_hash,
        };
        let audit = ChangeAuditRecord {
            sequence: state.audit.len() as u64 + 1,
            record_id: String::new(),
            change_id: patch.patch_id.clone(),
            change_digest_sha256: spec_patch_digest(&patch)?,
            claimed_base: patch.claimed_base(),
            observed_base: state.snapshot.head.clone(),
            outcome: ChangeOutcome::Committed,
            failure_category: None,
            failure_code: None,
            failure_message: None,
            side_effect_state: SideEffectState::Committed,
            committed_head: Some(committed_head.clone()),
            evidence,
        }
        .seal()?;

        state.snapshot = SpecSnapshot {
            head: committed_head,
            spec: candidate,
        };
        state.audit.push(SpecAuditRecord {
            transaction: audit.clone(),
            patch: patch.clone(),
        });
        Ok(ChangeReceipt { audit })
    }

    fn audit_log(&self) -> Result<Vec<ChangeAuditRecord>, ChangeKernelError> {
        Ok(self
            .lock_state()?
            .audit
            .iter()
            .map(|record| record.transaction.clone())
            .collect())
    }
}

#[derive(Debug)]
struct PatchRejection {
    category: ChangeFailureCategory,
    code: String,
    message: String,
}

impl PatchRejection {
    fn new(
        category: ChangeFailureCategory,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            code: code.into(),
            message: message.into(),
        }
    }
}

fn validate_patch_shape(patch: &SpecPatch) -> Result<(), PatchRejection> {
    if !is_stable_id(&patch.patch_id) {
        return Err(PatchRejection::new(
            ChangeFailureCategory::Input,
            "spec_store.invalid_patch_id",
            "patch id must be a stable lowercase identifier",
        ));
    }
    if patch.base_revision == 0 || !is_sha256(&patch.base_hash) {
        return Err(PatchRejection::new(
            ChangeFailureCategory::Input,
            "spec_store.invalid_base",
            "base revision and base hash must identify a valid transaction base",
        ));
    }
    if patch.operations.is_empty() || patch.declared_write_paths.is_empty() {
        return Err(PatchRejection::new(
            ChangeFailureCategory::Input,
            "spec_store.empty_patch",
            "patch must declare and contain at least one operation",
        ));
    }

    let mut operation_paths = BTreeSet::new();
    for (index, operation) in patch.operations.iter().enumerate() {
        validate_pointer(&operation.path).map_err(|message| {
            PatchRejection::new(
                ChangeFailureCategory::Input,
                "spec_store.invalid_operation_path",
                format!("operation {index} has invalid path: {message}"),
            )
        })?;
        if is_kernel_managed_path(&operation.path) {
            return Err(PatchRejection::new(
                ChangeFailureCategory::ScopeViolation,
                "spec_store.kernel_managed_path",
                format!(
                    "operation {index} attempts to modify kernel-managed path '{}'",
                    operation.path
                ),
            ));
        }
        if !operation_paths.insert(operation.path.clone()) {
            return Err(PatchRejection::new(
                ChangeFailureCategory::Input,
                "spec_store.duplicate_operation_path",
                format!("multiple operations target '{}'", operation.path),
            ));
        }
        if operation.reason.trim().is_empty() {
            return Err(PatchRejection::new(
                ChangeFailureCategory::Evidence,
                "spec_store.missing_operation_reason",
                format!("operation {index} has no reason"),
            ));
        }
        if operation.evidence.is_empty() {
            return Err(PatchRejection::new(
                ChangeFailureCategory::Evidence,
                "spec_store.missing_operation_evidence",
                format!("operation {index} has no evidence reference"),
            ));
        }
        for reference in &operation.evidence {
            if !is_stable_id(&reference.evidence_id) || !is_sha256(&reference.sha256) {
                return Err(PatchRejection::new(
                    ChangeFailureCategory::Evidence,
                    "spec_store.invalid_evidence_reference",
                    format!("operation {index} has an invalid evidence reference"),
                ));
            }
        }
        if matches!(operation.change, SpecValueChange::Remove)
            && matches!(operation.expected_old_value, ExpectedSpecValue::Absent)
        {
            return Err(PatchRejection::new(
                ChangeFailureCategory::Input,
                "spec_store.remove_expects_absent",
                format!("operation {index} cannot remove a value expected to be absent"),
            ));
        }
    }

    for path in &patch.declared_write_paths {
        validate_pointer(path).map_err(|message| {
            PatchRejection::new(
                ChangeFailureCategory::Input,
                "spec_store.invalid_declared_path",
                format!("declared write path is invalid: {message}"),
            )
        })?;
        if is_kernel_managed_path(path) {
            return Err(PatchRejection::new(
                ChangeFailureCategory::ScopeViolation,
                "spec_store.kernel_managed_scope",
                format!("declared scope includes kernel-managed path '{path}'"),
            ));
        }
    }
    if operation_paths != patch.declared_write_paths {
        return Err(PatchRejection::new(
            ChangeFailureCategory::ScopeViolation,
            "spec_store.scope_declaration_mismatch",
            "declared write paths must exactly match operation target paths",
        ));
    }
    let paths = operation_paths.iter().collect::<Vec<_>>();
    for (index, left) in paths.iter().enumerate() {
        for right in paths.iter().skip(index + 1) {
            if pointer_overlap(left, right) {
                return Err(PatchRejection::new(
                    ChangeFailureCategory::Input,
                    "spec_store.overlapping_operations",
                    format!("operation paths '{left}' and '{right}' overlap"),
                ));
            }
        }
    }
    Ok(())
}

fn apply_operation(root: &mut Value, operation: &SpecPatchOperation) -> Result<(), PatchRejection> {
    let tokens = parse_pointer(&operation.path).map_err(|message| {
        PatchRejection::new(
            ChangeFailureCategory::Input,
            "spec_store.invalid_operation_path",
            message,
        )
    })?;
    let (last, parent_tokens) = tokens.split_last().expect("root paths are rejected");
    let parent = value_at_mut(root, parent_tokens).ok_or_else(|| {
        PatchRejection::new(
            ChangeFailureCategory::Conflict,
            "spec_store.parent_path_missing",
            format!("parent of '{}' does not exist", operation.path),
        )
    })?;

    match parent {
        Value::Object(map) => apply_object_operation(map, last, operation),
        Value::Array(array) => apply_array_operation(array, last, operation),
        _ => Err(PatchRejection::new(
            ChangeFailureCategory::Conflict,
            "spec_store.parent_not_container",
            format!("parent of '{}' is not an object or array", operation.path),
        )),
    }
}

fn apply_object_operation(
    map: &mut Map<String, Value>,
    key: &str,
    operation: &SpecPatchOperation,
) -> Result<(), PatchRejection> {
    assert_expected(map.get(key), &operation.expected_old_value, &operation.path)?;
    match &operation.change {
        SpecValueChange::Set { value } => {
            map.insert(key.to_string(), value.clone());
        }
        SpecValueChange::Remove => {
            map.remove(key);
        }
    }
    Ok(())
}

fn apply_array_operation(
    array: &mut [Value],
    token: &str,
    operation: &SpecPatchOperation,
) -> Result<(), PatchRejection> {
    let index = token.parse::<usize>().map_err(|_| {
        PatchRejection::new(
            ChangeFailureCategory::Input,
            "spec_store.invalid_array_index",
            format!("'{}' is not an existing array index", operation.path),
        )
    })?;
    let current = array.get(index);
    assert_expected(current, &operation.expected_old_value, &operation.path)?;
    match &operation.change {
        SpecValueChange::Set { value } if index < array.len() => {
            array[index] = value.clone();
            Ok(())
        }
        SpecValueChange::Set { .. } => Err(PatchRejection::new(
            ChangeFailureCategory::Input,
            "spec_store.array_insert_requires_parent_replace",
            "array insertion must replace the containing ordered sequence explicitly",
        )),
        SpecValueChange::Remove => Err(PatchRejection::new(
            ChangeFailureCategory::Input,
            "spec_store.array_remove_requires_parent_replace",
            "array removal must replace the containing ordered sequence explicitly",
        )),
    }
}

fn assert_expected(
    current: Option<&Value>,
    expected: &ExpectedSpecValue,
    path: &str,
) -> Result<(), PatchRejection> {
    let matches = match expected {
        ExpectedSpecValue::Absent => current.is_none(),
        ExpectedSpecValue::Exact { value } => current == Some(value),
    };
    if matches {
        Ok(())
    } else {
        Err(PatchRejection::new(
            ChangeFailureCategory::Conflict,
            "spec_store.old_value_mismatch",
            format!("old value precondition failed at '{path}'"),
        ))
    }
}

fn validate_pointer(path: &str) -> Result<(), String> {
    parse_pointer(path).map(|_| ())
}

fn parse_pointer(path: &str) -> Result<Vec<String>, String> {
    if path.is_empty() || path == "/" || !path.starts_with('/') {
        return Err("path must be a non-root RFC 6901 JSON Pointer".to_string());
    }
    path[1..].split('/').map(decode_pointer_token).collect()
}

fn decode_pointer_token(token: &str) -> Result<String, String> {
    if token.is_empty() {
        return Err("empty JSON Pointer tokens are not allowed".to_string());
    }
    let mut decoded = String::new();
    let mut chars = token.chars();
    while let Some(character) = chars.next() {
        if character != '~' {
            decoded.push(character);
            continue;
        }
        match chars.next() {
            Some('0') => decoded.push('~'),
            Some('1') => decoded.push('/'),
            _ => return Err("JSON Pointer contains an invalid escape".to_string()),
        }
    }
    Ok(decoded)
}

fn value_at_mut<'a>(mut value: &'a mut Value, tokens: &[String]) -> Option<&'a mut Value> {
    for token in tokens {
        value = match value {
            Value::Object(map) => map.get_mut(token)?,
            Value::Array(array) => array.get_mut(token.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(value)
}

fn collect_changed_paths(
    before: &Value,
    after: &Value,
    path: &str,
    changed: &mut BTreeSet<String>,
) {
    if before == after {
        return;
    }
    match (before, after) {
        (Value::Object(left), Value::Object(right)) => {
            let keys = left.keys().chain(right.keys()).collect::<BTreeSet<_>>();
            for key in keys {
                let child = format!("{path}/{}", encode_pointer_token(key));
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        collect_changed_paths(left, right, &child, changed)
                    }
                    _ => {
                        changed.insert(child);
                    }
                }
            }
        }
        (Value::Array(left), Value::Array(right)) if left.len() == right.len() => {
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                collect_changed_paths(left, right, &format!("{path}/{index}"), changed);
            }
        }
        _ => {
            changed.insert(if path.is_empty() {
                "/".to_string()
            } else {
                path.to_string()
            });
        }
    }
}

fn encode_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

fn pointer_contains(parent: &str, candidate: &str) -> bool {
    candidate == parent
        || candidate
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn pointer_overlap(left: &str, right: &str) -> bool {
    pointer_contains(left, right) || pointer_contains(right, left)
}

fn is_kernel_managed_path(path: &str) -> bool {
    pointer_overlap(path, "/identity/revision") || pointer_overlap(path, "/identity/parentHash")
}

fn stable_change_id(value: &str) -> String {
    if is_stable_id(value) {
        value.to_string()
    } else {
        "invalid_change".to_string()
    }
}

fn spec_patch_digest(patch: &SpecPatch) -> Result<String, ChangeKernelError> {
    let bytes = serde_json::to_vec(patch).map_err(|error| {
        ChangeKernelError::new(
            "spec_store.patch_serialization_failed",
            format!("failed to seal SpecPatch: {error}"),
        )
    })?;
    Ok(crate::sha256_bytes(&bytes))
}

pub fn operation_evidence(evidence_id: impl Into<String>, details: &[u8]) -> EvidenceReference {
    EvidenceReference {
        evidence_id: evidence_id.into(),
        sha256: crate::sha256_bytes(details),
    }
}

pub fn patch_for_single_operation(
    patch_id: impl Into<String>,
    base: &KernelHead,
    operation: SpecPatchOperation,
) -> SpecPatch {
    let path = operation.path.clone();
    SpecPatch {
        patch_id: patch_id.into(),
        base_revision: base.revision,
        base_hash: base.content_hash.clone(),
        declared_write_paths: BTreeSet::from([path.clone()]),
        operations: vec![operation],
    }
}

pub fn committed_hash_chain(audit: &[ChangeAuditRecord]) -> BTreeMap<u64, String> {
    audit
        .iter()
        .filter_map(|record| {
            record
                .committed_head
                .as_ref()
                .map(|head| (head.revision, head.content_hash.clone()))
        })
        .collect()
}
