use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adm_new_contracts::pipeline::{
    PipelineCheckpoint, PipelineCheckpointStatus, PipelineResumePolicy, PipelineUnitStatus,
};
use adm_new_foundation::io::now_iso;
use adm_new_foundation::{AdmError, AdmResult, sha256_hex};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkUnitKind {
    Development,
    Art,
}

impl WorkUnitKind {
    fn label(self) -> &'static str {
        match self {
            Self::Development => "program",
            Self::Art => "art",
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkUnitRequest {
    pub stage_id: String,
    pub task_id: String,
    pub unit_id: String,
    pub idempotency_key: String,
    pub kind: WorkUnitKind,
    /// Opaque hash binding this request to the executor, target root and
    /// machine-side configuration that will perform its side effects.
    #[serde(default)]
    pub execution_scope: String,
    pub payload: Value,
}

impl fmt::Debug for WorkUnitRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkUnitRequest")
            .field("stage_id", &self.stage_id)
            .field("task_id", &self.task_id)
            .field("unit_id", &self.unit_id)
            .field("kind", &self.kind)
            .field(
                "execution_scope_configured",
                &!self.execution_scope.is_empty(),
            )
            .field(
                "payload_keys",
                &self
                    .payload
                    .as_object()
                    .map(|object| object.keys().collect::<Vec<_>>())
                    .unwrap_or_default(),
            )
            .finish()
    }
}

impl WorkUnitRequest {
    pub fn new(
        stage_id: &str,
        task_id: &str,
        kind: WorkUnitKind,
        payload: Value,
    ) -> AdmResult<Self> {
        let stage_id = stage_id.trim();
        let task_id = task_id.trim();
        if stage_id.is_empty() || task_id.is_empty() {
            return Err(AdmError::new(
                "work unit stage_id and task_id cannot be empty",
            ));
        }
        let unit_id = format!("{stage_id}:{}:{task_id}", kind.label());
        let request_fingerprint = request_fingerprint(stage_id, task_id, kind, "", &payload)?;
        Ok(Self {
            stage_id: stage_id.to_string(),
            task_id: task_id.to_string(),
            unit_id,
            idempotency_key: sha256_hex(format!("work-unit-v1:{request_fingerprint}").as_bytes()),
            kind,
            execution_scope: String::new(),
            payload,
        })
    }

    fn fingerprint(&self) -> AdmResult<String> {
        request_fingerprint(
            &self.stage_id,
            &self.task_id,
            self.kind,
            &self.execution_scope,
            &self.payload,
        )
    }

    fn bind_execution_scope(&self, scope: &str) -> AdmResult<Self> {
        let mut request = self.clone();
        request.execution_scope = scope.to_string();
        request.idempotency_key =
            sha256_hex(format!("work-unit-v1:{}", request.fingerprint()?).as_bytes());
        Ok(request)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkUnitExecutionStatus {
    Verified,
    Failed,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkUnitExecutionResult {
    pub status: WorkUnitExecutionStatus,
    #[serde(default)]
    pub output_refs: Vec<String>,
    #[serde(default)]
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub verification_results: Vec<Value>,
    #[serde(default)]
    pub data: Value,
    #[serde(default)]
    pub message: String,
}

impl WorkUnitExecutionResult {
    pub fn verified(
        output_refs: Vec<String>,
        changed_files: Vec<String>,
        verification_results: Vec<Value>,
        data: Value,
    ) -> Self {
        Self {
            status: WorkUnitExecutionStatus::Verified,
            output_refs,
            changed_files,
            verification_results,
            data,
            message: "work unit execution was verified".to_string(),
        }
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            status: WorkUnitExecutionStatus::Unavailable,
            output_refs: Vec::new(),
            changed_files: Vec::new(),
            verification_results: Vec::new(),
            data: Value::Null,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkUnitJournalPhase {
    Started,
    ResultReady,
    Committed,
    Failed,
    RecoveryBlocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkUnitJournalRecord {
    pub schema_version: u32,
    pub revision: u64,
    pub stage_id: String,
    pub task_id: String,
    pub unit_id: String,
    pub idempotency_key: String,
    pub request_fingerprint: String,
    pub phase: WorkUnitJournalPhase,
    #[serde(default)]
    pub result: Option<WorkUnitExecutionResult>,
    #[serde(default)]
    pub result_fingerprint: String,
    #[serde(default)]
    pub failure_message: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkUnitReconcileDecision {
    /// External state proves that the recorded result is complete and valid.
    Verified,
    /// External state proves that no side effect exists and execution can safely retry.
    SafeToRetry,
    /// External state proves that the unit failed without an ambiguous side effect.
    Failed,
    /// The executor cannot prove whether the side effect happened.
    Unknown,
}

pub const GAME_SPEC_V2_PRODUCT_STEP11_USAGE: &str = "gamespec_v2_product_step11";
pub const WORK_UNIT_EXECUTOR_RETAINED_CALLERS: &[&str] = &[
    "legacy_step08_14",
    "step07_image_work_units",
    "legacy_checkpoint_recovery",
    "r0_harness",
];
pub const WORK_UNIT_EXECUTOR_PROHIBITED_CALLERS: &[&str] = &[
    GAME_SPEC_V2_PRODUCT_STEP11_USAGE,
    "gamespec_v2_authoritative_execution_evidence",
];
pub const WORK_UNIT_EXECUTOR_V2_REPLACEMENT: &str =
    "adm_new_pipeline::stages::step11_v2::WorkspaceTaskAgent + WorkspaceChangeSet";

/// Legacy Step08-14 compatibility execution surface.
///
/// The GameSpec v2 production path uses `stages::step11_v2` and
/// `WorkspaceChangeSet` contracts as the authoritative Step11 execution model.
/// This trait remains for the existing desktop pipeline, Step07 image work
/// units, and R0 regression harness until those callers are migrated behind the
/// v2 contract executor.
pub trait WorkUnitExecutor: Send + Sync + fmt::Debug {
    /// Returns an opaque, non-secret fingerprint of the adapter and target
    /// side-effect scope. Changing project roots or adapters must change it.
    fn execution_scope_fingerprint(&self) -> String {
        "work-unit-executor-v1".to_string()
    }

    fn execute(&self, request: &WorkUnitRequest) -> AdmResult<WorkUnitExecutionResult>;

    fn reconcile(
        &self,
        request: &WorkUnitRequest,
        record: &WorkUnitJournalRecord,
    ) -> AdmResult<WorkUnitReconcileDecision>;
}

/// Explicit deterministic executor for tests and intentionally selected offline previews.
///
/// The historical type name is retained for API compatibility, but this
/// executor never verifies or commits an external side effect. Its result is
/// deliberately `Unavailable`, with the declared paths kept only as preview
/// metadata, so product-stage truth cannot mistake a contract preview for
/// materialized output.
/// It must never be installed implicitly by the product executor.
#[derive(Debug, Clone, Default)]
pub struct OfflineVerifiedWorkUnitExecutor;

impl WorkUnitExecutor for OfflineVerifiedWorkUnitExecutor {
    fn execution_scope_fingerprint(&self) -> String {
        "explicit-offline-preview-v2".to_string()
    }

    fn execute(&self, request: &WorkUnitRequest) -> AdmResult<WorkUnitExecutionResult> {
        let preview_output_refs = match request.kind {
            WorkUnitKind::Development => string_array(request.payload.get("output_files")),
            WorkUnitKind::Art => request
                .payload
                .get("unity_target_path")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .into_iter()
                .collect::<Vec<_>>(),
        };
        Ok(WorkUnitExecutionResult {
            status: WorkUnitExecutionStatus::Unavailable,
            output_refs: Vec::new(),
            changed_files: Vec::new(),
            verification_results: vec![json!({
                "id": "offline_contract_verification",
                "status": "not_executed",
                "mode": "explicit_offline",
                "evidence_complete": false,
            })],
            data: json!({
                "mode": "explicit_offline",
                "preview_only": true,
                "side_effects_performed": false,
                "execution_object_state": "offline_contract_only",
                "preview_output_refs": preview_output_refs,
            }),
            message: "offline preview did not execute or verify external side effects".to_string(),
        })
    }

    fn reconcile(
        &self,
        _request: &WorkUnitRequest,
        _record: &WorkUnitJournalRecord,
    ) -> AdmResult<WorkUnitReconcileDecision> {
        // Offline preview has no external side effect to prove. Returning
        // `Verified` here would upgrade a preview (including a legacy journal)
        // into a committed production result during resume.
        Ok(WorkUnitReconcileDecision::SafeToRetry)
    }
}

#[derive(Debug, Clone, Default)]
pub struct WorkUnitStopToken(Arc<AtomicBool>);

impl WorkUnitStopToken {
    pub fn from_shared(flag: Arc<AtomicBool>) -> Self {
        Self(flag)
    }

    pub fn shared_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.0)
    }

    pub fn request_stop(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_stop_requested(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn clear(&self) {
        self.0.store(false, Ordering::Release);
    }
}

#[derive(Debug, Clone)]
pub struct SafeUnitJournal {
    root: PathBuf,
    #[cfg(test)]
    fail_before_commit: Arc<AtomicBool>,
    #[cfg(test)]
    fail_recovery_blocked_transition: Arc<AtomicBool>,
}

impl SafeUnitJournal {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            #[cfg(test)]
            fail_before_commit: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            fail_recovery_blocked_transition: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn load(&self, request: &WorkUnitRequest) -> AdmResult<Option<WorkUnitJournalRecord>> {
        let dir = self.unit_dir(request);
        if !dir.try_exists()? {
            return Ok(None);
        }
        if !fs::metadata(&dir)?.is_dir() {
            return Err(AdmError::new(format!(
                "work unit journal lineage is not a directory: {}",
                dir.display()
            )));
        }
        let mut snapshots = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) == Some("json") {
                snapshots.push(path);
            }
        }
        snapshots.sort();
        let Some(path) = snapshots.pop() else {
            return Ok(None);
        };
        let bytes = fs::read(&path)?;
        let record = serde_json::from_slice::<WorkUnitJournalRecord>(&bytes).map_err(|error| {
            AdmError::new(format!("work unit journal snapshot is invalid: {error}"))
        })?;
        self.validate_record(request, &record)?;
        Ok(Some(record))
    }

    /// Loads the newest snapshot from every journal lineage without requiring the
    /// original in-memory request. This is used only by Step07 recovery preflight,
    /// where the executor scope is intentionally not persisted in plaintext.
    pub(crate) fn load_latest_records_unbound(&self) -> AdmResult<Vec<WorkUnitJournalRecord>> {
        if !self.root.try_exists()? {
            return Ok(Vec::new());
        }
        if !fs::metadata(&self.root)?.is_dir() {
            return Err(AdmError::new("work unit journal root is not a directory"));
        }
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                return Err(AdmError::new(
                    "work unit journal root contains an invalid non-lineage entry",
                ));
            }
            let mut snapshots = Vec::new();
            for snapshot in fs::read_dir(entry.path())? {
                let path = snapshot?.path();
                if path.extension().and_then(|value| value.to_str()) == Some("json") {
                    snapshots.push(path);
                }
            }
            snapshots.sort();
            let Some(path) = snapshots.pop() else {
                continue;
            };
            let record = serde_json::from_slice::<WorkUnitJournalRecord>(&fs::read(&path)?)
                .map_err(|error| {
                    AdmError::new(format!("work unit journal snapshot is invalid: {error}"))
                })?;
            validate_unbound_record(&entry.path(), &path, &record)?;
            records.push(record);
        }
        Ok(records)
    }

    fn append(&self, record: &WorkUnitJournalRecord) -> AdmResult<()> {
        let dir = self.unit_dir_for_lineage(&record.unit_id, &record.idempotency_key);
        fs::create_dir_all(&dir)?;
        let final_path = dir.join(format!("{:020}.json", record.revision));
        if final_path.exists() {
            return Err(AdmError::new(format!(
                "work unit journal revision already exists: {}",
                record.revision
            )));
        }
        let temp_path = dir.join(format!(
            ".{:020}.{}.tmp",
            record.revision,
            std::process::id()
        ));
        let bytes = serde_json::to_vec_pretty(record)
            .map_err(|error| AdmError::new(format!("failed to serialize work journal: {error}")))?;
        {
            let mut file = fs::File::create(&temp_path)?;
            use std::io::Write;
            file.write_all(&bytes)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
        }
        fs::rename(&temp_path, &final_path)?;
        Ok(())
    }

    fn transition(
        &self,
        request: &WorkUnitRequest,
        previous: Option<&WorkUnitJournalRecord>,
        phase: WorkUnitJournalPhase,
        result: Option<WorkUnitExecutionResult>,
        failure_message: String,
    ) -> AdmResult<WorkUnitJournalRecord> {
        let result_fingerprint = result
            .as_ref()
            .map(result_fingerprint)
            .transpose()?
            .unwrap_or_default();
        let record = WorkUnitJournalRecord {
            schema_version: 1,
            revision: previous.map(|record| record.revision + 1).unwrap_or(1),
            stage_id: request.stage_id.clone(),
            task_id: request.task_id.clone(),
            unit_id: request.unit_id.clone(),
            idempotency_key: request.idempotency_key.clone(),
            request_fingerprint: request.fingerprint()?,
            phase,
            result,
            result_fingerprint,
            failure_message,
            updated_at: now_iso(),
        };
        #[cfg(test)]
        if phase == WorkUnitJournalPhase::RecoveryBlocked
            && self
                .fail_recovery_blocked_transition
                .swap(false, Ordering::AcqRel)
        {
            return Err(AdmError::new(
                "injected failure while persisting recovery-blocked transition",
            ));
        }
        self.append(&record)?;
        Ok(record)
    }

    fn validate_record(
        &self,
        request: &WorkUnitRequest,
        record: &WorkUnitJournalRecord,
    ) -> AdmResult<()> {
        if record.schema_version != 1
            || record.unit_id != request.unit_id
            || record.idempotency_key != request.idempotency_key
            || record.request_fingerprint != request.fingerprint()?
        {
            return Err(AdmError::new(format!(
                "work unit journal lineage mismatch for {}",
                request.unit_id
            )));
        }
        if let Some(result) = &record.result
            && record.result_fingerprint != result_fingerprint(result)?
        {
            return Err(AdmError::new(format!(
                "work unit journal result fingerprint mismatch for {}",
                request.unit_id
            )));
        }
        if matches!(
            record.phase,
            WorkUnitJournalPhase::ResultReady | WorkUnitJournalPhase::Committed
        ) && record.result.is_none()
        {
            return Err(AdmError::new(format!(
                "work unit journal {} phase requires a result",
                request.unit_id
            )));
        }
        Ok(())
    }

    fn unit_dir(&self, request: &WorkUnitRequest) -> PathBuf {
        self.unit_dir_for_lineage(&request.unit_id, &request.idempotency_key)
    }

    fn unit_dir_for_lineage(&self, unit_id: &str, idempotency_key: &str) -> PathBuf {
        self.root.join(sha256_hex(
            format!("{unit_id}:{idempotency_key}").as_bytes(),
        ))
    }

    #[cfg(test)]
    fn inject_fail_before_commit(&self) {
        self.fail_before_commit.store(true, Ordering::Release);
    }

    #[cfg(test)]
    fn inject_fail_recovery_blocked_transition(&self) {
        self.fail_recovery_blocked_transition
            .store(true, Ordering::Release);
    }
}

fn validate_unbound_record(
    lineage_dir: &Path,
    snapshot_path: &Path,
    record: &WorkUnitJournalRecord,
) -> AdmResult<()> {
    let expected_lineage =
        sha256_hex(format!("{}:{}", record.unit_id, record.idempotency_key).as_bytes());
    let actual_lineage = lineage_dir
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let snapshot_revision = snapshot_path
        .file_stem()
        .and_then(|value| value.to_str())
        .and_then(|value| value.parse::<u64>().ok());
    if record.schema_version != 1
        || record.revision == 0
        || record.stage_id.trim().is_empty()
        || record.task_id.trim().is_empty()
        || record.unit_id.trim().is_empty()
        || !is_sha256_hex(&record.idempotency_key)
        || !is_sha256_hex(&record.request_fingerprint)
        || record
            .updated_at
            .strip_prefix("unix:")
            .and_then(|value| value.parse::<u64>().ok())
            .is_none()
        || actual_lineage != expected_lineage
        || snapshot_revision != Some(record.revision)
    {
        return Err(AdmError::new(
            "work unit journal snapshot has an invalid unbound lineage",
        ));
    }
    if let Some(result) = &record.result
        && record.result_fingerprint != result_fingerprint(result)?
    {
        return Err(AdmError::new(
            "work unit journal snapshot result fingerprint is invalid",
        ));
    }
    if record.result.is_none() && !record.result_fingerprint.is_empty() {
        return Err(AdmError::new(
            "work unit journal snapshot has a fingerprint without a result",
        ));
    }
    if matches!(
        record.phase,
        WorkUnitJournalPhase::ResultReady | WorkUnitJournalPhase::Committed
    ) && record.result.is_none()
    {
        return Err(AdmError::new(
            "work unit journal snapshot phase requires a result",
        ));
    }
    if record.phase == WorkUnitJournalPhase::Started
        && (record.result.is_some() || !record.result_fingerprint.is_empty())
    {
        return Err(AdmError::new(
            "work unit journal started phase cannot contain a result",
        ));
    }
    Ok(())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkUnitRunStatus {
    Committed,
    Reused,
    Failed,
    Unavailable,
    Stopped,
    RecoveryBlocked,
}

#[derive(Debug, Clone)]
pub struct WorkUnitRunOutcome {
    pub request: WorkUnitRequest,
    pub status: WorkUnitRunStatus,
    pub result: Option<WorkUnitExecutionResult>,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct WorkUnitBatchOutcome {
    pub units: Vec<WorkUnitRunOutcome>,
    pub stopped: bool,
    pub recovery_blocked: bool,
}

pub fn execute_work_unit_batch(
    requests: Vec<WorkUnitRequest>,
    executor: Option<&dyn WorkUnitExecutor>,
    journal: &SafeUnitJournal,
    stop_token: &WorkUnitStopToken,
) -> AdmResult<WorkUnitBatchOutcome> {
    let mut batch = WorkUnitBatchOutcome::default();
    let Some(executor) = executor else {
        batch.units = requests
            .into_iter()
            .map(|request| WorkUnitRunOutcome {
                request,
                status: WorkUnitRunStatus::Unavailable,
                result: None,
                message: "no work unit executor is configured".to_string(),
            })
            .collect();
        return Ok(batch);
    };

    let execution_scope = executor.execution_scope_fingerprint();
    for request in requests {
        let request = request.bind_execution_scope(&execution_scope)?;
        if stop_token.is_stop_requested() {
            batch.stopped = true;
            batch.units.push(WorkUnitRunOutcome {
                request,
                status: WorkUnitRunStatus::Stopped,
                result: None,
                message: "stop requested before work unit execution".to_string(),
            });
            break;
        }
        let outcome = execute_one(&request, executor, journal)?;
        if outcome.status == WorkUnitRunStatus::RecoveryBlocked {
            batch.recovery_blocked = true;
            batch.units.push(outcome);
            break;
        }
        batch.units.push(outcome);
    }
    if stop_token.is_stop_requested() {
        batch.stopped = true;
    }
    Ok(batch)
}

fn execute_one(
    request: &WorkUnitRequest,
    executor: &dyn WorkUnitExecutor,
    journal: &SafeUnitJournal,
) -> AdmResult<WorkUnitRunOutcome> {
    let mut previous = match journal.load(request) {
        Ok(record) => record,
        Err(error) => {
            return Ok(recovery_blocked_outcome(
                request,
                &format!("work unit journal cannot be verified: {error}"),
            ));
        }
    };
    if let Some(record) = &previous {
        match record.phase {
            WorkUnitJournalPhase::Committed => match executor.reconcile(request, record) {
                Err(_) => {
                    let message = "committed work unit could not be reconciled safely";
                    let persisted_message = journal
                        .transition(
                            request,
                            Some(record),
                            WorkUnitJournalPhase::RecoveryBlocked,
                            record.result.clone(),
                            message.to_string(),
                        )
                        .ok()
                        .map(|blocked| blocked.failure_message);
                    return Ok(recovery_blocked_outcome(
                        request,
                        persisted_message.as_deref().unwrap_or(message),
                    ));
                }
                Ok(WorkUnitReconcileDecision::Verified) => {
                    return Ok(WorkUnitRunOutcome {
                        request: request.clone(),
                        status: WorkUnitRunStatus::Reused,
                        result: record.result.clone(),
                        message: "revalidated and reused committed work unit result".to_string(),
                    });
                }
                Ok(WorkUnitReconcileDecision::SafeToRetry) => {
                    previous = Some(
                        journal.transition(
                            request,
                            Some(record),
                            WorkUnitJournalPhase::Failed,
                            record.result.clone(),
                            "committed side effect is no longer present; safe retry required"
                                .to_string(),
                        )?,
                    );
                }
                Ok(WorkUnitReconcileDecision::Failed) => {
                    return Ok(failed_outcome(
                        request,
                        "committed work unit no longer passes reconciliation",
                    ));
                }
                Ok(WorkUnitReconcileDecision::Unknown) => {
                    let blocked = journal.transition(
                        request,
                        Some(record),
                        WorkUnitJournalPhase::RecoveryBlocked,
                        record.result.clone(),
                        "committed work unit side effect cannot be reconciled safely".to_string(),
                    )?;
                    return Ok(recovery_blocked_outcome(request, &blocked.failure_message));
                }
            },
            WorkUnitJournalPhase::Started | WorkUnitJournalPhase::ResultReady => {
                let decision = match executor.reconcile(request, record) {
                    Ok(decision) => decision,
                    Err(_) => {
                        return Ok(recovery_blocked_outcome(
                            request,
                            "work unit reconciliation failed without a provably safe result",
                        ));
                    }
                };
                match decision {
                    WorkUnitReconcileDecision::Verified => {
                        let Some(result) = record.result.clone() else {
                            let blocked = journal.transition(
                                request,
                                Some(record),
                                WorkUnitJournalPhase::RecoveryBlocked,
                                None,
                                "executor reported verified without a recorded result".to_string(),
                            )?;
                            return Ok(recovery_blocked_outcome(request, &blocked.failure_message));
                        };
                        let committed = journal.transition(
                            request,
                            Some(record),
                            WorkUnitJournalPhase::Committed,
                            Some(result.clone()),
                            String::new(),
                        )?;
                        return Ok(WorkUnitRunOutcome {
                            request: request.clone(),
                            status: WorkUnitRunStatus::Reused,
                            result: committed.result,
                            message: "reconciled and committed existing work unit result"
                                .to_string(),
                        });
                    }
                    WorkUnitReconcileDecision::SafeToRetry => {}
                    WorkUnitReconcileDecision::Failed => {
                        let failed = journal.transition(
                            request,
                            Some(record),
                            WorkUnitJournalPhase::Failed,
                            record.result.clone(),
                            "executor reconciliation proved failure".to_string(),
                        )?;
                        return Ok(failed_outcome(request, &failed.failure_message));
                    }
                    WorkUnitReconcileDecision::Unknown => {
                        let blocked = journal.transition(
                            request,
                            Some(record),
                            WorkUnitJournalPhase::RecoveryBlocked,
                            record.result.clone(),
                            "work unit side effect cannot be reconciled safely".to_string(),
                        )?;
                        return Ok(recovery_blocked_outcome(request, &blocked.failure_message));
                    }
                }
            }
            WorkUnitJournalPhase::RecoveryBlocked => match executor.reconcile(request, record) {
                Err(_) => {
                    return Ok(recovery_blocked_outcome(request, &record.failure_message));
                }
                Ok(WorkUnitReconcileDecision::Verified) => {
                    let Some(result) = record.result.clone() else {
                        return Ok(recovery_blocked_outcome(
                            request,
                            "reconciliation reported verified without a recorded result",
                        ));
                    };
                    let committed = journal.transition(
                        request,
                        Some(record),
                        WorkUnitJournalPhase::Committed,
                        Some(result.clone()),
                        String::new(),
                    )?;
                    return Ok(WorkUnitRunOutcome {
                        request: request.clone(),
                        status: WorkUnitRunStatus::Reused,
                        result: committed.result,
                        message: "reconciled a previously blocked work unit".to_string(),
                    });
                }
                Ok(WorkUnitReconcileDecision::SafeToRetry) => {
                    previous = Some(journal.transition(
                        request,
                        Some(record),
                        WorkUnitJournalPhase::Failed,
                        record.result.clone(),
                        "previously blocked side effect is now safe to retry".to_string(),
                    )?);
                }
                Ok(WorkUnitReconcileDecision::Failed) => {
                    let failed = journal.transition(
                        request,
                        Some(record),
                        WorkUnitJournalPhase::Failed,
                        record.result.clone(),
                        "previously blocked side effect now proves failure".to_string(),
                    )?;
                    return Ok(failed_outcome(request, &failed.failure_message));
                }
                Ok(WorkUnitReconcileDecision::Unknown) => {
                    return Ok(recovery_blocked_outcome(request, &record.failure_message));
                }
            },
            WorkUnitJournalPhase::Failed => {
                // A proved failure is safe to retry explicitly on the next stage invocation.
            }
        }
    }

    let started = journal.transition(
        request,
        previous.as_ref(),
        WorkUnitJournalPhase::Started,
        None,
        String::new(),
    )?;
    previous = Some(started);
    let result = match executor.execute(request) {
        Ok(result) => result,
        Err(_) => {
            return Ok(WorkUnitRunOutcome {
                request: request.clone(),
                status: WorkUnitRunStatus::RecoveryBlocked,
                result: None,
                message: "work unit execution was interrupted; reconciliation is required"
                    .to_string(),
            });
        }
    };
    match result.status {
        WorkUnitExecutionStatus::Verified => {
            let ready = journal.transition(
                request,
                previous.as_ref(),
                WorkUnitJournalPhase::ResultReady,
                Some(result.clone()),
                String::new(),
            )?;
            #[cfg(test)]
            if journal.fail_before_commit.swap(false, Ordering::AcqRel) {
                return Err(AdmError::new("injected failure before work unit commit"));
            }
            let committed = journal.transition(
                request,
                Some(&ready),
                WorkUnitJournalPhase::Committed,
                Some(result.clone()),
                String::new(),
            )?;
            Ok(WorkUnitRunOutcome {
                request: request.clone(),
                status: WorkUnitRunStatus::Committed,
                result: committed.result,
                message: "work unit execution committed".to_string(),
            })
        }
        WorkUnitExecutionStatus::Unavailable => {
            let failed = journal.transition(
                request,
                previous.as_ref(),
                WorkUnitJournalPhase::Failed,
                Some(result.clone()),
                result.message.clone(),
            )?;
            Ok(WorkUnitRunOutcome {
                request: request.clone(),
                status: WorkUnitRunStatus::Unavailable,
                result: failed.result,
                message: failed.failure_message,
            })
        }
        WorkUnitExecutionStatus::Failed => {
            // A process can write files before returning a failure. Persist the
            // observed result first, then require the executor to prove whether
            // retrying is safe instead of treating every non-zero exit as clean.
            let ready = journal.transition(
                request,
                previous.as_ref(),
                WorkUnitJournalPhase::ResultReady,
                Some(result.clone()),
                result.message.clone(),
            )?;
            match executor.reconcile(request, &ready) {
                Err(_) => {
                    let message = "failed execution could not be reconciled safely";
                    let persisted_message = journal
                        .transition(
                            request,
                            Some(&ready),
                            WorkUnitJournalPhase::RecoveryBlocked,
                            Some(result),
                            message.to_string(),
                        )
                        .ok()
                        .map(|blocked| blocked.failure_message);
                    Ok(recovery_blocked_outcome(
                        request,
                        persisted_message.as_deref().unwrap_or(message),
                    ))
                }
                Ok(WorkUnitReconcileDecision::SafeToRetry)
                | Ok(WorkUnitReconcileDecision::Failed) => {
                    let failed = journal.transition(
                        request,
                        Some(&ready),
                        WorkUnitJournalPhase::Failed,
                        Some(result),
                        ready.failure_message.clone(),
                    )?;
                    Ok(WorkUnitRunOutcome {
                        request: request.clone(),
                        status: WorkUnitRunStatus::Failed,
                        result: failed.result,
                        message: failed.failure_message,
                    })
                }
                Ok(WorkUnitReconcileDecision::Verified)
                | Ok(WorkUnitReconcileDecision::Unknown) => {
                    let blocked = journal.transition(
                        request,
                        Some(&ready),
                        WorkUnitJournalPhase::RecoveryBlocked,
                        Some(result),
                        "failed execution may have left an unverified side effect".to_string(),
                    )?;
                    Ok(recovery_blocked_outcome(request, &blocked.failure_message))
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageWorkUnitReconcileStatus {
    Committed,
    Pending,
    RecoveryBlocked,
}

/// Reconciles a whole-stage checkpoint against the finer Step11/12 journal.
/// A stage marked unknown is committed only when every internal unit is provably committed.
pub fn reconcile_checkpoint_stage_from_journal(
    checkpoint: &mut PipelineCheckpoint,
    stage_id: &str,
    requests: &[WorkUnitRequest],
    executor: Option<&dyn WorkUnitExecutor>,
    journal: &SafeUnitJournal,
) -> AdmResult<StageWorkUnitReconcileStatus> {
    let scoped_requests = bind_requests_to_executor_scope(requests, executor)?;
    let status = reconcile_requests(&scoped_requests, executor, journal)?;
    let whole_unit_id = format!("{stage_id}:stage");
    let unit = checkpoint
        .units
        .iter_mut()
        .find(|unit| unit.stage_id == stage_id && unit.unit_id == whole_unit_id);
    if let Some(unit) = unit {
        match status {
            StageWorkUnitReconcileStatus::Committed => {
                // Internal side effects are committed, but the pure stage
                // summary/manifest may not have been written before a crash.
                // Rerun the stage synthesis; its internal units will be reused.
                unit.status = PipelineUnitStatus::Pending;
                unit.reconcile_required = false;
                unit.failure_message.clear();
                unit.result_fingerprint = committed_batch_fingerprint(&scoped_requests, journal)?;
            }
            StageWorkUnitReconcileStatus::Pending => {
                unit.status = PipelineUnitStatus::Pending;
                unit.reconcile_required = false;
                unit.failure_message.clear();
                unit.result_fingerprint.clear();
            }
            StageWorkUnitReconcileStatus::RecoveryBlocked => {
                unit.status = PipelineUnitStatus::Unknown;
                unit.reconcile_required = true;
                unit.failure_message =
                    "one or more internal work units cannot be reconciled safely".to_string();
            }
        }
    }
    if status == StageWorkUnitReconcileStatus::RecoveryBlocked {
        checkpoint.status = PipelineCheckpointStatus::RecoveryBlocked;
        checkpoint.resume_policy = PipelineResumePolicy::Disabled;
        checkpoint.recovery_blocked_reason =
            format!("stage {stage_id} contains an unknown work unit side effect");
    } else {
        checkpoint.status = PipelineCheckpointStatus::Recoverable;
        checkpoint.resume_policy = PipelineResumePolicy::ExplicitOnly;
        checkpoint.recovery_blocked_reason.clear();
    }
    Ok(status)
}

fn reconcile_requests(
    requests: &[WorkUnitRequest],
    executor: Option<&dyn WorkUnitExecutor>,
    journal: &SafeUnitJournal,
) -> AdmResult<StageWorkUnitReconcileStatus> {
    let mut all_committed = !requests.is_empty();
    for request in requests {
        let record = match journal.load(request) {
            Ok(record) => record,
            Err(_) => return Ok(StageWorkUnitReconcileStatus::RecoveryBlocked),
        };
        let Some(record) = record else {
            all_committed = false;
            continue;
        };
        match record.phase {
            WorkUnitJournalPhase::Committed => {
                let Some(executor) = executor else {
                    return Ok(StageWorkUnitReconcileStatus::RecoveryBlocked);
                };
                match executor.reconcile(request, &record) {
                    Ok(WorkUnitReconcileDecision::Verified) => {}
                    Ok(WorkUnitReconcileDecision::SafeToRetry)
                    | Ok(WorkUnitReconcileDecision::Failed) => {
                        journal.transition(
                            request,
                            Some(&record),
                            WorkUnitJournalPhase::Failed,
                            record.result.clone(),
                            "committed side effect no longer validates".to_string(),
                        )?;
                        all_committed = false;
                    }
                    Ok(WorkUnitReconcileDecision::Unknown) | Err(_) => {
                        return Ok(StageWorkUnitReconcileStatus::RecoveryBlocked);
                    }
                }
            }
            WorkUnitJournalPhase::Failed => all_committed = false,
            WorkUnitJournalPhase::RecoveryBlocked => {
                let Some(executor) = executor else {
                    return Ok(StageWorkUnitReconcileStatus::RecoveryBlocked);
                };
                match executor.reconcile(request, &record) {
                    Ok(WorkUnitReconcileDecision::Verified) if record.result.is_some() => {
                        journal.transition(
                            request,
                            Some(&record),
                            WorkUnitJournalPhase::Committed,
                            record.result.clone(),
                            String::new(),
                        )?;
                    }
                    Ok(WorkUnitReconcileDecision::SafeToRetry)
                    | Ok(WorkUnitReconcileDecision::Failed) => {
                        journal.transition(
                            request,
                            Some(&record),
                            WorkUnitJournalPhase::Failed,
                            record.result.clone(),
                            "previously blocked work unit is now safe to retry".to_string(),
                        )?;
                        all_committed = false;
                    }
                    Ok(WorkUnitReconcileDecision::Verified)
                    | Ok(WorkUnitReconcileDecision::Unknown)
                    | Err(_) => return Ok(StageWorkUnitReconcileStatus::RecoveryBlocked),
                }
            }
            WorkUnitJournalPhase::Started | WorkUnitJournalPhase::ResultReady => {
                let Some(executor) = executor else {
                    return Ok(StageWorkUnitReconcileStatus::RecoveryBlocked);
                };
                let decision = match executor.reconcile(request, &record) {
                    Ok(decision) => decision,
                    Err(_) => return Ok(StageWorkUnitReconcileStatus::RecoveryBlocked),
                };
                match decision {
                    WorkUnitReconcileDecision::Verified if record.result.is_some() => {
                        journal.transition(
                            request,
                            Some(&record),
                            WorkUnitJournalPhase::Committed,
                            record.result.clone(),
                            String::new(),
                        )?;
                    }
                    WorkUnitReconcileDecision::SafeToRetry | WorkUnitReconcileDecision::Failed => {
                        all_committed = false
                    }
                    WorkUnitReconcileDecision::Verified | WorkUnitReconcileDecision::Unknown => {
                        return Ok(StageWorkUnitReconcileStatus::RecoveryBlocked);
                    }
                }
            }
        }
    }
    Ok(if all_committed {
        StageWorkUnitReconcileStatus::Committed
    } else {
        StageWorkUnitReconcileStatus::Pending
    })
}

fn committed_batch_fingerprint(
    requests: &[WorkUnitRequest],
    journal: &SafeUnitJournal,
) -> AdmResult<String> {
    let mut fingerprints = Vec::new();
    for request in requests {
        let record = journal.load(request)?.ok_or_else(|| {
            AdmError::new(format!("missing committed journal for {}", request.unit_id))
        })?;
        if record.phase != WorkUnitJournalPhase::Committed {
            return Err(AdmError::new(format!(
                "work unit is not committed: {}",
                request.unit_id
            )));
        }
        fingerprints.push(record.result_fingerprint);
    }
    Ok(sha256_hex(fingerprints.join(":").as_bytes()))
}

fn request_fingerprint(
    stage_id: &str,
    task_id: &str,
    kind: WorkUnitKind,
    execution_scope: &str,
    payload: &Value,
) -> AdmResult<String> {
    let bytes = serde_json::to_vec(&(stage_id, task_id, kind, execution_scope, payload)).map_err(
        |error| AdmError::new(format!("failed to fingerprint work unit request: {error}")),
    )?;
    Ok(sha256_hex(&bytes))
}

fn bind_requests_to_executor_scope(
    requests: &[WorkUnitRequest],
    executor: Option<&dyn WorkUnitExecutor>,
) -> AdmResult<Vec<WorkUnitRequest>> {
    let scope = executor
        .map(WorkUnitExecutor::execution_scope_fingerprint)
        .unwrap_or_else(|| "work-unit-executor-unavailable-v1".to_string());
    requests
        .iter()
        .map(|request| request.bind_execution_scope(&scope))
        .collect()
}

fn result_fingerprint(result: &WorkUnitExecutionResult) -> AdmResult<String> {
    let bytes = serde_json::to_vec(result).map_err(|error| {
        AdmError::new(format!("failed to fingerprint work unit result: {error}"))
    })?;
    Ok(sha256_hex(&bytes))
}

fn recovery_blocked_outcome(request: &WorkUnitRequest, message: &str) -> WorkUnitRunOutcome {
    WorkUnitRunOutcome {
        request: request.clone(),
        status: WorkUnitRunStatus::RecoveryBlocked,
        result: None,
        message: message.to_string(),
    }
}

fn failed_outcome(request: &WorkUnitRequest, message: &str) -> WorkUnitRunOutcome {
    WorkUnitRunOutcome {
        request: request.clone(),
        status: WorkUnitRunStatus::Failed,
        result: None,
        message: message.to_string(),
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::pipeline::{PipelineUnitCheckpoint, PipelineUnitStatus};
    use adm_new_foundation::new_stable_id;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FakeFault {
        None,
        BeforeSideEffect,
        AfterSideEffect,
    }

    #[derive(Debug)]
    struct FakeExecutor {
        fault: Mutex<FakeFault>,
        reconcile: WorkUnitReconcileDecision,
        executions: Mutex<BTreeMap<String, usize>>,
        stop_after_first: Option<WorkUnitStopToken>,
    }

    impl FakeExecutor {
        fn new(fault: FakeFault, reconcile: WorkUnitReconcileDecision) -> Self {
            Self {
                fault: Mutex::new(fault),
                reconcile,
                executions: Mutex::new(BTreeMap::new()),
                stop_after_first: None,
            }
        }

        fn count(&self, unit_id: &str) -> usize {
            *self.executions.lock().unwrap().get(unit_id).unwrap_or(&0)
        }
    }

    impl WorkUnitExecutor for FakeExecutor {
        fn execute(&self, request: &WorkUnitRequest) -> AdmResult<WorkUnitExecutionResult> {
            let mut fault = self.fault.lock().unwrap();
            if *fault == FakeFault::BeforeSideEffect {
                *fault = FakeFault::None;
                return Err(AdmError::new("injected before side effect"));
            }
            *self
                .executions
                .lock()
                .unwrap()
                .entry(request.unit_id.clone())
                .or_default() += 1;
            if let Some(token) = &self.stop_after_first {
                token.request_stop();
            }
            if *fault == FakeFault::AfterSideEffect {
                *fault = FakeFault::None;
                return Err(AdmError::new("injected after side effect"));
            }
            Ok(WorkUnitExecutionResult::verified(
                vec![format!("out/{}", request.task_id)],
                Vec::new(),
                vec![json!({"status": "passed"})],
                json!({"task_id": request.task_id}),
            ))
        }

        fn reconcile(
            &self,
            _request: &WorkUnitRequest,
            _record: &WorkUnitJournalRecord,
        ) -> AdmResult<WorkUnitReconcileDecision> {
            Ok(self.reconcile)
        }
    }

    #[derive(Debug)]
    struct ReconcileErrorExecutor {
        execution_result: WorkUnitExecutionResult,
    }

    impl ReconcileErrorExecutor {
        fn verified() -> Self {
            Self {
                execution_result: WorkUnitExecutionResult::verified(
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Value::Null,
                ),
            }
        }

        fn failed() -> Self {
            Self {
                execution_result: WorkUnitExecutionResult {
                    status: WorkUnitExecutionStatus::Failed,
                    output_refs: Vec::new(),
                    changed_files: Vec::new(),
                    verification_results: Vec::new(),
                    data: Value::Null,
                    message: "injected execution failure".to_string(),
                },
            }
        }
    }

    impl WorkUnitExecutor for ReconcileErrorExecutor {
        fn execute(&self, _request: &WorkUnitRequest) -> AdmResult<WorkUnitExecutionResult> {
            Ok(self.execution_result.clone())
        }

        fn reconcile(
            &self,
            _request: &WorkUnitRequest,
            _record: &WorkUnitJournalRecord,
        ) -> AdmResult<WorkUnitReconcileDecision> {
            Err(AdmError::new("injected reconcile I/O failure"))
        }
    }

    #[test]
    fn journal_load_rejects_a_non_directory_lineage_instead_of_treating_it_as_missing() {
        let (root, journal) = journal();
        let bound_request = request("DEV-JOURNAL-IO")
            .bind_execution_scope("work-unit-executor-v1")
            .unwrap();
        let lineage = journal.unit_dir(&bound_request);
        fs::create_dir_all(lineage.parent().unwrap()).unwrap();
        fs::write(&lineage, b"not a journal directory").unwrap();

        assert!(journal.load(&bound_request).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn committed_result_is_reused_without_repeating_side_effect() {
        let (root, journal) = journal();
        let request = request("DEV-001");
        let executor = FakeExecutor::new(FakeFault::None, WorkUnitReconcileDecision::Verified);
        let stop = WorkUnitStopToken::default();
        let first =
            execute_work_unit_batch(vec![request.clone()], Some(&executor), &journal, &stop)
                .unwrap();
        let second =
            execute_work_unit_batch(vec![request.clone()], Some(&executor), &journal, &stop)
                .unwrap();
        assert_eq!(first.units[0].status, WorkUnitRunStatus::Committed);
        assert_eq!(second.units[0].status, WorkUnitRunStatus::Reused);
        assert_eq!(executor.count(&request.unit_id), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reconcile_io_failure_for_committed_and_blocked_records_stays_recovery_blocked() {
        let (root, journal) = journal();
        let request = request("DEV-RECONCILE-IO-COMMITTED");
        let stop = WorkUnitStopToken::default();
        let initial = FakeExecutor::new(FakeFault::None, WorkUnitReconcileDecision::Verified);
        execute_work_unit_batch(vec![request.clone()], Some(&initial), &journal, &stop).unwrap();

        let failing = ReconcileErrorExecutor::verified();
        let committed_failure =
            execute_work_unit_batch(vec![request.clone()], Some(&failing), &journal, &stop)
                .unwrap();
        assert!(committed_failure.recovery_blocked);
        assert_eq!(
            committed_failure.units[0].status,
            WorkUnitRunStatus::RecoveryBlocked
        );
        let bound_request = &committed_failure.units[0].request;
        let blocked_record = journal.load(bound_request).unwrap().unwrap();
        assert_eq!(blocked_record.phase, WorkUnitJournalPhase::RecoveryBlocked);

        let blocked_failure =
            execute_work_unit_batch(vec![request], Some(&failing), &journal, &stop).unwrap();
        assert!(blocked_failure.recovery_blocked);
        assert_eq!(
            blocked_failure.units[0].status,
            WorkUnitRunStatus::RecoveryBlocked
        );
        assert_eq!(
            journal
                .load(&blocked_failure.units[0].request)
                .unwrap()
                .unwrap()
                .phase,
            WorkUnitJournalPhase::RecoveryBlocked
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reconcile_io_failure_after_failed_execution_stays_recovery_blocked() {
        let (root, journal) = journal();
        let request = request("DEV-RECONCILE-IO-FAILED");
        let executor = ReconcileErrorExecutor::failed();
        let batch = execute_work_unit_batch(
            vec![request],
            Some(&executor),
            &journal,
            &WorkUnitStopToken::default(),
        )
        .unwrap();
        assert!(batch.recovery_blocked);
        assert_eq!(batch.units[0].status, WorkUnitRunStatus::RecoveryBlocked);
        assert_eq!(
            journal
                .load(&batch.units[0].request)
                .unwrap()
                .unwrap()
                .phase,
            WorkUnitJournalPhase::RecoveryBlocked
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn committed_reconcile_io_failure_stays_blocked_when_blocked_journal_write_fails() {
        let (root, journal) = journal();
        let request = request("DEV-RECONCILE-IO-COMMITTED-WRITE");
        let stop = WorkUnitStopToken::default();
        let initial = FakeExecutor::new(FakeFault::None, WorkUnitReconcileDecision::Verified);
        execute_work_unit_batch(vec![request.clone()], Some(&initial), &journal, &stop).unwrap();
        journal.inject_fail_recovery_blocked_transition();

        let batch = execute_work_unit_batch(
            vec![request],
            Some(&ReconcileErrorExecutor::verified()),
            &journal,
            &stop,
        )
        .unwrap();
        assert!(batch.recovery_blocked);
        assert_eq!(batch.units[0].status, WorkUnitRunStatus::RecoveryBlocked);
        assert_eq!(
            journal
                .load(&batch.units[0].request)
                .unwrap()
                .unwrap()
                .phase,
            WorkUnitJournalPhase::Committed
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failed_reconcile_io_failure_stays_blocked_when_blocked_journal_write_fails() {
        let (root, journal) = journal();
        journal.inject_fail_recovery_blocked_transition();
        let batch = execute_work_unit_batch(
            vec![request("DEV-RECONCILE-IO-FAILED-WRITE")],
            Some(&ReconcileErrorExecutor::failed()),
            &journal,
            &WorkUnitStopToken::default(),
        )
        .unwrap();
        assert!(batch.recovery_blocked);
        assert_eq!(batch.units[0].status, WorkUnitRunStatus::RecoveryBlocked);
        assert_eq!(
            journal
                .load(&batch.units[0].request)
                .unwrap()
                .unwrap()
                .phase,
            WorkUnitJournalPhase::ResultReady
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failure_before_side_effect_reconciles_then_retries() {
        let (root, journal) = journal();
        let request = request("DEV-002");
        let executor = FakeExecutor::new(
            FakeFault::BeforeSideEffect,
            WorkUnitReconcileDecision::SafeToRetry,
        );
        let stop = WorkUnitStopToken::default();
        let first =
            execute_work_unit_batch(vec![request.clone()], Some(&executor), &journal, &stop)
                .unwrap();
        let second =
            execute_work_unit_batch(vec![request.clone()], Some(&executor), &journal, &stop)
                .unwrap();
        assert_eq!(first.units[0].status, WorkUnitRunStatus::RecoveryBlocked);
        assert_eq!(second.units[0].status, WorkUnitRunStatus::Committed);
        assert_eq!(executor.count(&request.unit_id), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn committed_result_is_reexecuted_when_reconcile_proves_outputs_absent() {
        let (root, journal) = journal();
        let request = request("DEV-RETRY");
        let executor = FakeExecutor::new(FakeFault::None, WorkUnitReconcileDecision::SafeToRetry);
        let stop = WorkUnitStopToken::default();
        let first =
            execute_work_unit_batch(vec![request.clone()], Some(&executor), &journal, &stop)
                .unwrap();
        let second =
            execute_work_unit_batch(vec![request.clone()], Some(&executor), &journal, &stop)
                .unwrap();
        assert_eq!(first.units[0].status, WorkUnitRunStatus::Committed);
        assert_eq!(second.units[0].status, WorkUnitRunStatus::Committed);
        assert_eq!(executor.count(&request.unit_id), 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn execution_scope_creates_a_new_idempotency_lineage() {
        #[derive(Debug)]
        struct ScopedExecutor {
            scope: &'static str,
            executions: Arc<AtomicBool>,
        }

        impl WorkUnitExecutor for ScopedExecutor {
            fn execution_scope_fingerprint(&self) -> String {
                self.scope.to_string()
            }

            fn execute(&self, request: &WorkUnitRequest) -> AdmResult<WorkUnitExecutionResult> {
                self.executions.store(true, Ordering::Release);
                Ok(WorkUnitExecutionResult::verified(
                    vec![format!("out/{}", request.task_id)],
                    Vec::new(),
                    vec![json!({"status": "passed"})],
                    Value::Null,
                ))
            }

            fn reconcile(
                &self,
                _request: &WorkUnitRequest,
                _record: &WorkUnitJournalRecord,
            ) -> AdmResult<WorkUnitReconcileDecision> {
                Ok(WorkUnitReconcileDecision::Verified)
            }
        }

        let (root, journal) = journal();
        let request = request("DEV-SCOPE");
        let stop = WorkUnitStopToken::default();
        let first_ran = Arc::new(AtomicBool::new(false));
        let second_ran = Arc::new(AtomicBool::new(false));
        let first = ScopedExecutor {
            scope: "project-a",
            executions: Arc::clone(&first_ran),
        };
        let second = ScopedExecutor {
            scope: "project-b",
            executions: Arc::clone(&second_ran),
        };
        execute_work_unit_batch(vec![request.clone()], Some(&first), &journal, &stop).unwrap();
        execute_work_unit_batch(vec![request], Some(&second), &journal, &stop).unwrap();
        assert!(first_ran.load(Ordering::Acquire));
        assert!(second_ran.load(Ordering::Acquire));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn failure_after_side_effect_blocks_when_reconciliation_is_unknown() {
        let (root, journal) = journal();
        let request = request("DEV-003");
        let executor = FakeExecutor::new(
            FakeFault::AfterSideEffect,
            WorkUnitReconcileDecision::Unknown,
        );
        let stop = WorkUnitStopToken::default();
        execute_work_unit_batch(vec![request.clone()], Some(&executor), &journal, &stop).unwrap();
        let second =
            execute_work_unit_batch(vec![request.clone()], Some(&executor), &journal, &stop)
                .unwrap();
        assert_eq!(second.units[0].status, WorkUnitRunStatus::RecoveryBlocked);
        assert_eq!(executor.count(&request.unit_id), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovery_blocked_journal_can_be_reconciled_after_external_state_is_repaired() {
        let (root, journal) = journal();
        let request = request("DEV-RECONCILE-BLOCKED");
        let stop = WorkUnitStopToken::default();
        let unknown = FakeExecutor::new(
            FakeFault::AfterSideEffect,
            WorkUnitReconcileDecision::Unknown,
        );
        let first = execute_work_unit_batch(vec![request.clone()], Some(&unknown), &journal, &stop)
            .unwrap();
        assert!(first.recovery_blocked);
        let second =
            execute_work_unit_batch(vec![request.clone()], Some(&unknown), &journal, &stop)
                .unwrap();
        assert!(second.recovery_blocked);
        let bound_request = request
            .bind_execution_scope(&unknown.execution_scope_fingerprint())
            .unwrap();
        assert_eq!(
            journal.load(&bound_request).unwrap().unwrap().phase,
            WorkUnitJournalPhase::RecoveryBlocked
        );

        let repaired = FakeExecutor::new(FakeFault::None, WorkUnitReconcileDecision::SafeToRetry);
        let resumed =
            execute_work_unit_batch(vec![request], Some(&repaired), &journal, &stop).unwrap();
        assert!(!resumed.recovery_blocked);
        assert_eq!(resumed.units[0].status, WorkUnitRunStatus::Committed);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn result_ready_before_commit_is_reconciled_and_reused() {
        let (root, journal) = journal();
        let request = request("DEV-004");
        let executor = FakeExecutor::new(FakeFault::None, WorkUnitReconcileDecision::Verified);
        let stop = WorkUnitStopToken::default();
        journal.inject_fail_before_commit();
        assert!(
            execute_work_unit_batch(vec![request.clone()], Some(&executor), &journal, &stop,)
                .is_err()
        );
        let second =
            execute_work_unit_batch(vec![request.clone()], Some(&executor), &journal, &stop)
                .unwrap();
        assert_eq!(second.units[0].status, WorkUnitRunStatus::Reused);
        assert_eq!(executor.count(&request.unit_id), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stop_token_stops_at_the_boundary_between_units() {
        let (root, journal) = journal();
        let stop = WorkUnitStopToken::default();
        let executor = FakeExecutor {
            fault: Mutex::new(FakeFault::None),
            reconcile: WorkUnitReconcileDecision::Unknown,
            executions: Mutex::new(BTreeMap::new()),
            stop_after_first: Some(stop.clone()),
        };
        let batch = execute_work_unit_batch(
            vec![request("DEV-005"), request("DEV-006")],
            Some(&executor),
            &journal,
            &stop,
        )
        .unwrap();
        assert!(batch.stopped);
        assert_eq!(batch.units[0].status, WorkUnitRunStatus::Committed);
        assert_eq!(batch.units[1].status, WorkUnitRunStatus::Stopped);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unknown_internal_unit_blocks_whole_stage_checkpoint_recovery() {
        let (root, journal) = journal();
        let request = request("DEV-007");
        let executor = FakeExecutor::new(
            FakeFault::AfterSideEffect,
            WorkUnitReconcileDecision::Unknown,
        );
        execute_work_unit_batch(
            vec![request.clone()],
            Some(&executor),
            &journal,
            &WorkUnitStopToken::default(),
        )
        .unwrap();
        let mut checkpoint = PipelineCheckpoint {
            units: vec![PipelineUnitCheckpoint {
                stage_id: "11".to_string(),
                unit_id: "11:stage".to_string(),
                status: PipelineUnitStatus::Unknown,
                reconcile_required: true,
                ..PipelineUnitCheckpoint::default()
            }],
            ..PipelineCheckpoint::default()
        };
        let status = reconcile_checkpoint_stage_from_journal(
            &mut checkpoint,
            "11",
            &[request],
            Some(&executor),
            &journal,
        )
        .unwrap();
        assert_eq!(status, StageWorkUnitReconcileStatus::RecoveryBlocked);
        assert_eq!(checkpoint.status, PipelineCheckpointStatus::RecoveryBlocked);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn explicit_offline_executor_is_preview_only_and_never_reconciles_as_verified() {
        let (root, journal) = journal();
        let request = request("DEV-OFFLINE-PREVIEW");
        let executor = OfflineVerifiedWorkUnitExecutor;
        let batch = execute_work_unit_batch(
            vec![request.clone()],
            Some(&executor),
            &journal,
            &WorkUnitStopToken::default(),
        )
        .unwrap();

        assert_eq!(batch.units[0].status, WorkUnitRunStatus::Unavailable);
        let result = batch.units[0].result.as_ref().unwrap();
        assert_eq!(result.status, WorkUnitExecutionStatus::Unavailable);
        assert!(result.output_refs.is_empty());
        assert!(result.changed_files.is_empty());
        assert_eq!(result.data["mode"], "explicit_offline");
        assert_eq!(result.data["preview_only"], true);
        assert_eq!(result.data["side_effects_performed"], false);
        assert_eq!(
            result.data["execution_object_state"],
            "offline_contract_only"
        );
        assert_eq!(
            result.data["preview_output_refs"],
            json!(["Assets/DEV-OFFLINE-PREVIEW.cs"])
        );
        assert_eq!(result.verification_results[0]["status"], "not_executed");

        let scoped = request
            .bind_execution_scope(&executor.execution_scope_fingerprint())
            .unwrap();
        let record = journal.load(&scoped).unwrap().unwrap();
        assert_eq!(record.phase, WorkUnitJournalPhase::Failed);
        assert_eq!(
            executor.reconcile(&scoped, &record).unwrap(),
            WorkUnitReconcileDecision::SafeToRetry
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn work_unit_executor_retention_boundary_excludes_v2_product_step11() {
        assert!(
            WORK_UNIT_EXECUTOR_RETAINED_CALLERS.contains(&"legacy_step08_14"),
            "legacy projects still need the compatibility executor while game_spec_v2 is opt-in"
        );
        assert!(
            WORK_UNIT_EXECUTOR_PROHIBITED_CALLERS.contains(&GAME_SPEC_V2_PRODUCT_STEP11_USAGE),
            "v2 product Step11 must use the WorkspaceTaskAgent contract executor"
        );
        assert!(WORK_UNIT_EXECUTOR_V2_REPLACEMENT.contains("WorkspaceTaskAgent"));
    }

    fn request(task_id: &str) -> WorkUnitRequest {
        WorkUnitRequest::new(
            "11",
            task_id,
            WorkUnitKind::Development,
            json!({"output_files": [format!("Assets/{task_id}.cs")]}),
        )
        .unwrap()
    }

    fn journal() -> (PathBuf, SafeUnitJournal) {
        let root = std::env::temp_dir().join(new_stable_id("work-unit-journal").unwrap());
        (root.clone(), SafeUnitJournal::new(root.join("journal")))
    }
}
