#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

use adm_new_contracts::pipeline::{
    PIPELINE_CHECKPOINT_SCHEMA_VERSION, PipelineCheckpoint, PipelineCheckpointStatus,
    PipelineRecoverySummary, PipelineResumePolicy, PipelineUnitStatus,
};
use adm_new_foundation::{
    AdmError, AdmResult, sanitize_identifier, unix_timestamp_millis, write_text_atomic,
};

/// The desktop runtime owns one draft at a time, so a process-wide lock is sufficient to make
/// the repository's read-check-commit sequence atomic inside the running application. The file
/// writes themselves remain atomic replacements.
static CHECKPOINT_REPOSITORY_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone)]
pub struct PipelineCheckpointRepository {
    checkpoints_dir: PathBuf,
    #[cfg(test)]
    fail_after_current: Arc<AtomicBool>,
}

impl PipelineCheckpointRepository {
    pub fn new(checkpoints_dir: impl AsRef<Path>) -> Self {
        Self {
            checkpoints_dir: checkpoints_dir.as_ref().to_path_buf(),
            #[cfg(test)]
            fail_after_current: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn current_path(&self, run_id: &str) -> AdmResult<PathBuf> {
        let run_id = safe_component("run_id", run_id)?;
        Ok(self
            .checkpoints_dir
            .join("pipeline")
            .join(run_id)
            .join("current.json"))
    }

    pub fn attempt_path(&self, run_id: &str, attempt_id: &str) -> AdmResult<PathBuf> {
        let run_id = safe_component("run_id", run_id)?;
        let attempt_id = safe_component("attempt_id", attempt_id)?;
        Ok(self
            .checkpoints_dir
            .join("pipeline")
            .join(run_id)
            .join("attempts")
            .join(format!("{attempt_id}.json")))
    }

    pub fn load_current(&self, run_id: &str) -> AdmResult<Option<PipelineCheckpoint>> {
        let expected_run_id = safe_component("run_id", run_id)?;
        let _guard = repository_lock()?;
        self.load_authoritative_unlocked(&expected_run_id)
    }

    pub fn save_attempt_and_current(&self, checkpoint: &PipelineCheckpoint) -> AdmResult<()> {
        let _guard = repository_lock()?;
        validate_checkpoint(checkpoint)?;
        let run_id = safe_component("run_id", &checkpoint.identity.run_id)?;
        let current = self.load_authoritative_unlocked(&run_id)?;
        if current.as_ref() == Some(checkpoint) {
            return self.compensate_attempt_unlocked(checkpoint);
        }
        validate_lineage(current.as_ref(), checkpoint)?;
        self.commit_unlocked(checkpoint)
    }

    /// Atomically commits `checkpoint` when the authoritative current revision still equals
    /// `expected_revision`. Revision zero means that no checkpoint may exist yet.
    pub fn compare_and_swap(
        &self,
        expected_revision: u64,
        checkpoint: &PipelineCheckpoint,
    ) -> AdmResult<()> {
        let _guard = repository_lock()?;
        validate_checkpoint(checkpoint)?;
        let run_id = safe_component("run_id", &checkpoint.identity.run_id)?;
        let current = self.load_authoritative_unlocked(&run_id)?;
        let actual_revision = current
            .as_ref()
            .map(|current| current.revision)
            .unwrap_or(0);
        if actual_revision != expected_revision {
            return Err(AdmError::new(format!(
                "checkpoint revision conflict: expected {expected_revision}, actual {actual_revision}"
            )));
        }
        let required_revision = expected_revision
            .checked_add(1)
            .ok_or_else(|| AdmError::new("checkpoint revision overflow"))?;
        if checkpoint.revision != required_revision {
            return Err(AdmError::new(format!(
                "checkpoint CAS revision must advance by exactly one: {} != {}",
                checkpoint.revision, required_revision
            )));
        }
        validate_lineage(current.as_ref(), checkpoint)?;
        self.commit_unlocked(checkpoint)
    }

    fn load_authoritative_unlocked(&self, run_id: &str) -> AdmResult<Option<PipelineCheckpoint>> {
        let current_path = self.current_path(run_id)?;
        let mut first_error = None;
        let current = match self.load_checkpoint(&current_path, Some(run_id)) {
            Ok(current) => current,
            Err(error) => {
                first_error = Some(error);
                None
            }
        };

        let attempts_dir = self
            .checkpoints_dir
            .join("pipeline")
            .join(run_id)
            .join("attempts");
        let mut attempts = Vec::new();
        if attempts_dir.is_dir() {
            let mut paths = fs::read_dir(&attempts_dir)?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
                .collect::<Vec<_>>();
            paths.sort();
            for path in paths {
                match self.load_checkpoint(&path, Some(run_id)) {
                    Ok(Some(checkpoint)) => {
                        let expected_attempt_id = path
                            .file_stem()
                            .and_then(|value| value.to_str())
                            .unwrap_or_default();
                        if checkpoint.identity.attempt_id != expected_attempt_id {
                            let error = AdmError::new(format!(
                                "checkpoint attempt filename does not match its identity at {}",
                                path.display()
                            ));
                            quarantine_corrupt(&path)?;
                            first_error.get_or_insert(error);
                        } else {
                            attempts.push(checkpoint);
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        first_error.get_or_insert(error);
                    }
                }
            }
        }

        let selected = match current.as_ref() {
            Some(current) => advance_legal_lineage(current.clone(), &attempts)?,
            None => match select_attempt_root(&attempts)? {
                Some(root) => advance_legal_lineage(root, &attempts)?,
                None => {
                    return match first_error {
                        Some(error) => Err(error),
                        None => Ok(None),
                    };
                }
            },
        };

        if current.as_ref() != Some(&selected) {
            // `current.json` is the commit point. A newer legal attempt can only come from an
            // interrupted legacy attempt-first write, so promote it before returning it.
            write_checkpoint(&current_path, &selected)?;
        }
        // Archive repair is compensating and must not make an already valid commit unreadable.
        let _ = self.compensate_attempt_unlocked(&selected);
        Ok(Some(selected))
    }

    fn commit_unlocked(&self, checkpoint: &PipelineCheckpoint) -> AdmResult<()> {
        let current_path = self.current_path(&checkpoint.identity.run_id)?;
        write_checkpoint(&current_path, checkpoint)?;
        #[cfg(test)]
        if self.fail_after_current.swap(false, Ordering::AcqRel) {
            return Err(AdmError::new(
                "injected checkpoint failure after current commit",
            ));
        }
        self.compensate_attempt_unlocked(checkpoint)
    }

    fn compensate_attempt_unlocked(&self, checkpoint: &PipelineCheckpoint) -> AdmResult<()> {
        let attempt_path =
            self.attempt_path(&checkpoint.identity.run_id, &checkpoint.identity.attempt_id)?;
        match self.load_checkpoint(&attempt_path, Some(&checkpoint.identity.run_id)) {
            Ok(Some(existing)) if existing == *checkpoint => Ok(()),
            Ok(Some(existing)) if existing.revision > checkpoint.revision => {
                Err(AdmError::new(format!(
                    "checkpoint revision regression for attempt {}: {} < {}",
                    checkpoint.identity.attempt_id, checkpoint.revision, existing.revision
                )))
            }
            Ok(Some(_)) | Ok(None) => write_checkpoint(&attempt_path, checkpoint),
            Err(_) => write_checkpoint(&attempt_path, checkpoint),
        }
    }

    #[cfg(test)]
    fn inject_fail_after_current_once(&self) {
        self.fail_after_current.store(true, Ordering::Release);
    }

    pub fn list_recoverable(&self) -> AdmResult<Vec<PipelineRecoverySummary>> {
        let root = self.checkpoints_dir.join("pipeline");
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut summaries = Vec::new();
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(run_id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let checkpoint = match self.load_current(&run_id) {
                Ok(Some(checkpoint)) => checkpoint,
                Ok(None) | Err(_) => continue,
            };
            if checkpoint.status == PipelineCheckpointStatus::Recoverable
                && checkpoint.resume_policy == PipelineResumePolicy::ExplicitOnly
            {
                summaries.push(PipelineRecoverySummary::from(&checkpoint));
            }
        }
        summaries.sort_by(|left, right| left.run_id.cmp(&right.run_id));
        Ok(summaries)
    }

    fn load_checkpoint(
        &self,
        path: &Path,
        expected_run_id: Option<&str>,
    ) -> AdmResult<Option<PipelineCheckpoint>> {
        if !path.exists() {
            return Ok(None);
        }
        let result = fs::read_to_string(path)
            .map_err(AdmError::from)
            .and_then(|text| {
                serde_json::from_str::<PipelineCheckpoint>(&text).map_err(|error| {
                    AdmError::new(format!(
                        "invalid checkpoint JSON at {}: {error}",
                        path.display()
                    ))
                })
            })
            .and_then(|checkpoint| {
                validate_checkpoint(&checkpoint)?;
                if let Some(expected) = expected_run_id
                    && checkpoint.identity.run_id != expected
                {
                    return Err(AdmError::new(format!(
                        "checkpoint run lineage mismatch at {}",
                        path.display()
                    )));
                }
                Ok(checkpoint)
            });
        match result {
            Ok(checkpoint) => Ok(Some(checkpoint)),
            Err(error) => {
                quarantine_corrupt(path)?;
                Err(error)
            }
        }
    }
}

fn repository_lock() -> AdmResult<std::sync::MutexGuard<'static, ()>> {
    CHECKPOINT_REPOSITORY_LOCK
        .lock()
        .map_err(|_| AdmError::new("pipeline checkpoint repository lock is poisoned"))
}

fn select_attempt_root(attempts: &[PipelineCheckpoint]) -> AdmResult<Option<PipelineCheckpoint>> {
    let mut roots = attempts
        .iter()
        .filter(|checkpoint| checkpoint.identity.parent_attempt_id.is_none())
        .cloned()
        .collect::<Vec<_>>();
    roots.sort_by_key(|checkpoint| checkpoint.revision);
    roots.dedup();
    let attempt_ids = roots
        .iter()
        .map(|checkpoint| checkpoint.identity.attempt_id.as_str())
        .collect::<BTreeSet<_>>();
    if attempt_ids.len() > 1 {
        return Err(AdmError::new(
            "checkpoint attempt archive contains multiple root lineages",
        ));
    }
    Ok(roots.pop())
}

fn advance_legal_lineage(
    mut selected: PipelineCheckpoint,
    attempts: &[PipelineCheckpoint],
) -> AdmResult<PipelineCheckpoint> {
    loop {
        let same_attempt = attempts
            .iter()
            .filter(|candidate| {
                candidate.revision > selected.revision
                    && candidate.identity.attempt_id == selected.identity.attempt_id
                    && validate_lineage(Some(&selected), candidate).is_ok()
            })
            .max_by_key(|candidate| candidate.revision)
            .cloned();
        if let Some(candidate) = same_attempt {
            selected = candidate;
            continue;
        }

        let mut children = attempts
            .iter()
            .filter(|candidate| {
                candidate.revision > selected.revision
                    && candidate.identity.attempt_id != selected.identity.attempt_id
                    && candidate.identity.parent_attempt_id.as_deref()
                        == Some(selected.identity.attempt_id.as_str())
                    && validate_lineage(Some(&selected), candidate).is_ok()
            })
            .cloned()
            .collect::<Vec<_>>();
        children.sort_by_key(|candidate| candidate.revision);
        children.dedup();
        let child_attempt_ids = children
            .iter()
            .map(|candidate| candidate.identity.attempt_id.as_str())
            .collect::<BTreeSet<_>>();
        if child_attempt_ids.len() > 1 {
            return Err(AdmError::new(format!(
                "checkpoint attempt lineage branches after {}",
                selected.identity.attempt_id
            )));
        }
        let Some(candidate) = children.pop() else {
            return Ok(selected);
        };
        selected = candidate;
    }
}

fn validate_checkpoint(checkpoint: &PipelineCheckpoint) -> AdmResult<()> {
    if checkpoint.schema_version != PIPELINE_CHECKPOINT_SCHEMA_VERSION {
        return Err(AdmError::new(format!(
            "unsupported pipeline checkpoint schema version: {}",
            checkpoint.schema_version
        )));
    }
    if checkpoint.revision == 0 {
        return Err(AdmError::new(
            "checkpoint revision must be greater than zero",
        ));
    }
    safe_component("run_id", &checkpoint.identity.run_id)?;
    safe_component("attempt_id", &checkpoint.identity.attempt_id)?;
    if let Some(parent) = checkpoint.identity.parent_attempt_id.as_deref() {
        safe_component("parent_attempt_id", parent)?;
        if parent == checkpoint.identity.attempt_id {
            return Err(AdmError::new("attempt cannot be its own parent"));
        }
    }
    if checkpoint.range.stage_ids.is_empty()
        || checkpoint.range.stage_ids.first() != Some(&checkpoint.range.from_stage_id)
        || checkpoint.range.stage_ids.last() != Some(&checkpoint.range.to_stage_id)
    {
        return Err(AdmError::new(
            "canonical checkpoint range must include matching first and last stage IDs",
        ));
    }
    let mut stage_ids = BTreeSet::new();
    let mut stage_indexes = BTreeMap::new();
    for (index, stage_id) in checkpoint.range.stage_ids.iter().enumerate() {
        if stage_id.trim().is_empty() || !stage_ids.insert(stage_id.as_str()) {
            return Err(AdmError::new(
                "canonical checkpoint range stage IDs must be non-empty and unique",
            ));
        }
        safe_component("stage_id", stage_id)?;
        stage_indexes.insert(stage_id.as_str(), index);
    }
    if let Some(current_stage_id) = checkpoint.current_stage_id.as_deref()
        && !stage_indexes.contains_key(current_stage_id)
    {
        return Err(AdmError::new(format!(
            "checkpoint current stage is outside the canonical range: {current_stage_id}"
        )));
    }

    let mut unit_ids = BTreeSet::new();
    let mut unit_keys = BTreeSet::new();
    let mut previous_stage_index = None;
    for unit in &checkpoint.units {
        if unit.stage_id.trim().is_empty() || unit.unit_id.trim().is_empty() {
            return Err(AdmError::new("checkpoint unit IDs must not be empty"));
        }
        let Some(stage_index) = stage_indexes.get(unit.stage_id.as_str()).copied() else {
            return Err(AdmError::new(format!(
                "checkpoint unit stage is outside the canonical range: {}/{}",
                unit.stage_id, unit.unit_id
            )));
        };
        if previous_stage_index.is_some_and(|previous| stage_index < previous) {
            return Err(AdmError::new(
                "checkpoint units must follow canonical range order",
            ));
        }
        previous_stage_index = Some(stage_index);
        if !unit_ids.insert(unit.unit_id.as_str())
            || !unit_keys.insert((unit.stage_id.as_str(), unit.unit_id.as_str()))
        {
            return Err(AdmError::new(format!(
                "duplicate checkpoint unit: {}/{}",
                unit.stage_id, unit.unit_id
            )));
        }
        if unit.idempotency_key.trim().is_empty() {
            return Err(AdmError::new(format!(
                "checkpoint unit idempotency key must not be empty: {}/{}",
                unit.stage_id, unit.unit_id
            )));
        }
        if matches!(
            unit.status,
            PipelineUnitStatus::Running | PipelineUnitStatus::Unknown
        ) && checkpoint.status == PipelineCheckpointStatus::Recoverable
            && !unit.reconcile_required
        {
            return Err(AdmError::new(format!(
                "uncommitted unit {}/{} must require reconciliation",
                unit.stage_id, unit.unit_id
            )));
        }
    }

    let current_unit = checkpoint.current_unit_id.as_deref().map(|unit_id| {
        checkpoint
            .units
            .iter()
            .find(|unit| unit.unit_id == unit_id)
            .ok_or_else(|| {
                AdmError::new(format!(
                    "checkpoint current unit does not exist in the canonical unit list: {unit_id}"
                ))
            })
    });
    if let Some(current_unit) = current_unit.transpose()? {
        let current_stage_id = checkpoint
            .current_stage_id
            .as_deref()
            .ok_or_else(|| AdmError::new("checkpoint current unit requires a current stage"))?;
        if current_unit.stage_id != current_stage_id {
            return Err(AdmError::new(
                "checkpoint current unit does not belong to the current stage",
            ));
        }
    }

    let next_unit = checkpoint.next_unit_id.as_deref().map(|unit_id| {
        checkpoint
            .units
            .iter()
            .find(|unit| unit.unit_id == unit_id)
            .ok_or_else(|| {
                AdmError::new(format!(
                    "checkpoint next unit does not exist in the canonical unit list: {unit_id}"
                ))
            })
    });
    let next_unit = next_unit.transpose()?;
    if let (Some(current_stage_id), Some(next_unit)) =
        (checkpoint.current_stage_id.as_deref(), next_unit)
    {
        let current_index = stage_indexes[current_stage_id];
        let next_index = stage_indexes[next_unit.stage_id.as_str()];
        if next_index < current_index {
            return Err(AdmError::new(
                "checkpoint next unit cannot rewind before the current stage",
            ));
        }
    }

    if checkpoint.status == PipelineCheckpointStatus::Recoverable
        && checkpoint.resume_policy == PipelineResumePolicy::Disabled
    {
        return Err(AdmError::new(
            "recoverable checkpoint cannot disable explicit resume",
        ));
    }
    if checkpoint.status == PipelineCheckpointStatus::Recoverable {
        let next_unit = next_unit.ok_or_else(|| {
            AdmError::new("recoverable checkpoint must name its next execution unit")
        })?;
        let first_uncommitted = checkpoint
            .units
            .iter()
            .find(|unit| !unit.status.is_committed());
        if first_uncommitted.map(|unit| unit.unit_id.as_str()) != Some(next_unit.unit_id.as_str()) {
            return Err(AdmError::new(
                "recoverable checkpoint next unit must be the first uncommitted unit",
            ));
        }
    }
    if matches!(
        checkpoint.status,
        PipelineCheckpointStatus::Completed
            | PipelineCheckpointStatus::Failed
            | PipelineCheckpointStatus::WaitingConfirmation
            | PipelineCheckpointStatus::RecoveryBlocked
    ) && checkpoint.resume_policy != PipelineResumePolicy::Disabled
    {
        return Err(AdmError::new(
            "terminal or blocked checkpoint cannot enable resume",
        ));
    }
    if checkpoint.status == PipelineCheckpointStatus::Completed {
        if checkpoint.next_unit_id.is_some()
            || checkpoint.current_unit_id.is_some()
            || checkpoint
                .units
                .iter()
                .any(|unit| !unit.status.is_committed())
        {
            return Err(AdmError::new(
                "completed checkpoint must have only committed units and no current or next unit",
            ));
        }
        let completed_stages = checkpoint
            .units
            .iter()
            .map(|unit| unit.stage_id.as_str())
            .collect::<BTreeSet<_>>();
        if completed_stages != stage_ids {
            return Err(AdmError::new(
                "completed checkpoint must cover every stage in its canonical range",
            ));
        }
    }
    if checkpoint.status == PipelineCheckpointStatus::WaitingConfirmation
        && checkpoint.next_unit_id.is_some()
    {
        return Err(AdmError::new(
            "waiting-confirmation checkpoint cannot expose a resume unit",
        ));
    }
    if checkpoint.status == PipelineCheckpointStatus::RecoveryBlocked {
        if checkpoint.recovery_blocked_reason.trim().is_empty()
            || !checkpoint
                .units
                .iter()
                .any(|unit| unit.reconcile_required || unit.status == PipelineUnitStatus::Unknown)
        {
            return Err(AdmError::new(
                "recovery-blocked checkpoint must describe an unreconciled unit",
            ));
        }
    }
    Ok(())
}

fn validate_lineage(
    current: Option<&PipelineCheckpoint>,
    next: &PipelineCheckpoint,
) -> AdmResult<()> {
    let Some(current) = current else {
        if next.identity.parent_attempt_id.is_some() {
            return Err(AdmError::new(
                "initial checkpoint cannot name a parent attempt",
            ));
        }
        return Ok(());
    };
    if next.revision <= current.revision {
        return Err(AdmError::new(format!(
            "checkpoint revision must increase: {} <= {}",
            next.revision, current.revision
        )));
    }
    if next.identity.run_id != current.identity.run_id
        || next.identity.project_id != current.identity.project_id
        || next.identity.draft_id != current.identity.draft_id
        || next.identity.save_id != current.identity.save_id
        || next.range != current.range
        || next.fingerprints != current.fingerprints
        || next.skip_manual_gates != current.skip_manual_gates
        || next.created_at != current.created_at
    {
        return Err(AdmError::new(
            "checkpoint run identity, execution policy, or fingerprints changed",
        ));
    }
    let attempt_changed = next.identity.attempt_id != current.identity.attempt_id;
    if !attempt_changed {
        if next.identity.parent_attempt_id != current.identity.parent_attempt_id {
            return Err(AdmError::new("checkpoint attempt lineage changed"));
        }
    } else {
        if next.identity.parent_attempt_id.as_deref() != Some(current.identity.attempt_id.as_str())
        {
            return Err(AdmError::new(
                "new checkpoint attempt must name the current attempt as parent",
            ));
        }
        if current.status != PipelineCheckpointStatus::Recoverable
            || next.status != PipelineCheckpointStatus::Resuming
        {
            return Err(AdmError::new(
                "a new checkpoint attempt may only resume an explicitly recoverable checkpoint",
            ));
        }
    }
    validate_status_transition(current, next, attempt_changed)?;
    validate_unit_lineage(current, next)?;
    Ok(())
}

fn validate_status_transition(
    current: &PipelineCheckpoint,
    next: &PipelineCheckpoint,
    attempt_changed: bool,
) -> AdmResult<()> {
    if is_terminal_checkpoint_status(&current.status) {
        let mut expected = current.clone();
        expected.revision = next.revision;
        expected.updated_at = next.updated_at.clone();
        if expected != *next {
            return Err(AdmError::new(format!(
                "terminal checkpoint state cannot change after {:?}",
                current.status
            )));
        }
        return Ok(());
    }

    let allowed = match &current.status {
        PipelineCheckpointStatus::Running => matches!(
            &next.status,
            PipelineCheckpointStatus::Running
                | PipelineCheckpointStatus::StopRequested
                | PipelineCheckpointStatus::Stopping
                | PipelineCheckpointStatus::Stopped
                | PipelineCheckpointStatus::Recoverable
                | PipelineCheckpointStatus::WaitingConfirmation
                | PipelineCheckpointStatus::Completed
                | PipelineCheckpointStatus::Failed
                | PipelineCheckpointStatus::RecoveryBlocked
        ),
        PipelineCheckpointStatus::StopRequested => matches!(
            &next.status,
            PipelineCheckpointStatus::StopRequested
                | PipelineCheckpointStatus::Stopping
                | PipelineCheckpointStatus::Stopped
                | PipelineCheckpointStatus::Recoverable
                | PipelineCheckpointStatus::Completed
                | PipelineCheckpointStatus::Failed
                | PipelineCheckpointStatus::RecoveryBlocked
        ),
        PipelineCheckpointStatus::Stopping => matches!(
            &next.status,
            PipelineCheckpointStatus::StopRequested
                | PipelineCheckpointStatus::Stopping
                | PipelineCheckpointStatus::Stopped
                | PipelineCheckpointStatus::Recoverable
                | PipelineCheckpointStatus::Completed
                | PipelineCheckpointStatus::Failed
                | PipelineCheckpointStatus::RecoveryBlocked
        ),
        PipelineCheckpointStatus::Stopped => matches!(
            &next.status,
            PipelineCheckpointStatus::Stopped | PipelineCheckpointStatus::Recoverable
        ),
        PipelineCheckpointStatus::Recoverable => matches!(
            &next.status,
            PipelineCheckpointStatus::Recoverable
                | PipelineCheckpointStatus::Resuming
                | PipelineCheckpointStatus::RecoveryBlocked
        ),
        PipelineCheckpointStatus::Resuming => matches!(
            &next.status,
            PipelineCheckpointStatus::Resuming
                | PipelineCheckpointStatus::Running
                | PipelineCheckpointStatus::StopRequested
                | PipelineCheckpointStatus::Stopping
                | PipelineCheckpointStatus::Stopped
                | PipelineCheckpointStatus::Recoverable
                | PipelineCheckpointStatus::WaitingConfirmation
                | PipelineCheckpointStatus::Completed
                | PipelineCheckpointStatus::Failed
                | PipelineCheckpointStatus::RecoveryBlocked
        ),
        PipelineCheckpointStatus::RecoveryBlocked => matches!(
            &next.status,
            PipelineCheckpointStatus::RecoveryBlocked
                | PipelineCheckpointStatus::Recoverable
                | PipelineCheckpointStatus::Failed
        ),
        PipelineCheckpointStatus::WaitingConfirmation
        | PipelineCheckpointStatus::Completed
        | PipelineCheckpointStatus::Failed => false,
    };
    if !allowed {
        return Err(AdmError::new(format!(
            "illegal checkpoint status transition: {:?} -> {:?}",
            current.status, next.status
        )));
    }
    if next.status == PipelineCheckpointStatus::Resuming && !attempt_changed {
        return Err(AdmError::new(
            "resuming checkpoint must start a new child attempt",
        ));
    }
    Ok(())
}

fn validate_unit_lineage(current: &PipelineCheckpoint, next: &PipelineCheckpoint) -> AdmResult<()> {
    if current.units.len() != next.units.len() {
        return Err(AdmError::new(
            "checkpoint execution unit set cannot change after the run starts",
        ));
    }
    for (current_unit, next_unit) in current.units.iter().zip(&next.units) {
        if current_unit.stage_id != next_unit.stage_id
            || current_unit.unit_id != next_unit.unit_id
            || current_unit.idempotency_key != next_unit.idempotency_key
        {
            return Err(AdmError::new(
                "checkpoint execution unit identity or order changed",
            ));
        }
        if matches!(
            &current_unit.status,
            PipelineUnitStatus::Committed | PipelineUnitStatus::Skipped
        ) && current_unit.status != next_unit.status
        {
            return Err(AdmError::new(format!(
                "committed checkpoint unit cannot regress: {}/{}",
                current_unit.stage_id, current_unit.unit_id
            )));
        }
        if matches!(
            &current_unit.status,
            PipelineUnitStatus::Committed | PipelineUnitStatus::Skipped
        ) && current_unit != next_unit
        {
            return Err(AdmError::new(format!(
                "committed checkpoint unit record cannot be rewritten: {}/{}",
                current_unit.stage_id, current_unit.unit_id
            )));
        }
    }
    Ok(())
}

fn is_terminal_checkpoint_status(status: &PipelineCheckpointStatus) -> bool {
    matches!(
        status,
        PipelineCheckpointStatus::WaitingConfirmation
            | PipelineCheckpointStatus::Completed
            | PipelineCheckpointStatus::Failed
    )
}

fn write_checkpoint(path: &Path, checkpoint: &PipelineCheckpoint) -> AdmResult<()> {
    let text = serde_json::to_string_pretty(checkpoint)
        .map_err(|error| AdmError::new(format!("failed to serialize checkpoint: {error}")))?;
    write_text_atomic(path, &(text + "\n"))
}

fn quarantine_corrupt(path: &Path) -> AdmResult<PathBuf> {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| AdmError::new(format!("invalid checkpoint path: {}", path.display())))?;
    for suffix in 0..100_u8 {
        let quarantined = path.with_file_name(format!(
            "{file_name}.corrupt-{}-{suffix}",
            unix_timestamp_millis()
        ));
        if !quarantined.exists() {
            fs::rename(path, &quarantined)?;
            return Ok(quarantined);
        }
    }
    Err(AdmError::new(format!(
        "failed to allocate corrupt checkpoint path for {}",
        path.display()
    )))
}

fn safe_component(name: &str, value: &str) -> AdmResult<String> {
    let clean = sanitize_identifier(value)?;
    if clean == value {
        Ok(clean)
    } else {
        Err(AdmError::new(format!(
            "{name} contains non-portable path characters: {value}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::pipeline::{
        CanonicalPipelineRange, PipelineFingerprints, PipelineRunIdentity, PipelineUnitCheckpoint,
    };
    use adm_new_foundation::new_stable_id;

    #[test]
    fn checkpoint_roundtrip_preserves_attempt_lineage_and_lists_recovery() {
        let root = temp_dir("roundtrip");
        let repo = PipelineCheckpointRepository::new(&root);
        let first = checkpoint("run_1", "attempt_1", 1);
        repo.save_attempt_and_current(&first).unwrap();
        let mut recoverable = first.clone();
        recoverable.revision = 2;
        recoverable.status = PipelineCheckpointStatus::Recoverable;
        recoverable.resume_policy = PipelineResumePolicy::ExplicitOnly;
        repo.save_attempt_and_current(&recoverable).unwrap();

        let mut resumed = recoverable.clone();
        resumed.revision = 3;
        resumed.identity.attempt_id = "attempt_2".to_string();
        resumed.identity.parent_attempt_id = Some("attempt_1".to_string());
        resumed.status = PipelineCheckpointStatus::Resuming;
        resumed.resume_policy = PipelineResumePolicy::Disabled;
        repo.save_attempt_and_current(&resumed).unwrap();

        let mut interrupted = resumed.clone();
        interrupted.revision = 4;
        interrupted.status = PipelineCheckpointStatus::Recoverable;
        interrupted.resume_policy = PipelineResumePolicy::ExplicitOnly;
        repo.save_attempt_and_current(&interrupted).unwrap();

        assert_eq!(repo.load_current("run_1").unwrap(), Some(interrupted));
        assert!(
            repo.current_path("run_1")
                .unwrap()
                .ends_with("pipeline/run_1/current.json")
        );
        assert!(
            repo.attempt_path("run_1", "attempt_2")
                .unwrap()
                .ends_with("pipeline/run_1/attempts/attempt_2.json")
        );
        assert_eq!(repo.list_recoverable().unwrap().len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn legal_stop_and_resume_transitions_are_accepted() {
        let root = temp_dir("legal-stop-resume");
        let repo = PipelineCheckpointRepository::new(&root);
        let running = checkpoint("run_1", "attempt_1", 1);
        repo.compare_and_swap(0, &running).unwrap();

        let mut stop_requested = running.clone();
        stop_requested.revision = 2;
        stop_requested.status = PipelineCheckpointStatus::StopRequested;
        stop_requested.stop_reason = "operator_request".to_string();
        repo.compare_and_swap(1, &stop_requested).unwrap();

        let mut stopping = stop_requested.clone();
        stopping.revision = 3;
        stopping.status = PipelineCheckpointStatus::Stopping;
        repo.compare_and_swap(2, &stopping).unwrap();

        let mut recoverable = stopping.clone();
        recoverable.revision = 4;
        recoverable.status = PipelineCheckpointStatus::Recoverable;
        recoverable.resume_policy = PipelineResumePolicy::ExplicitOnly;
        repo.compare_and_swap(3, &recoverable).unwrap();

        let mut resumed = recoverable.clone();
        resumed.revision = 5;
        resumed.identity.attempt_id = "attempt_2".to_string();
        resumed.identity.parent_attempt_id = Some("attempt_1".to_string());
        resumed.status = PipelineCheckpointStatus::Resuming;
        resumed.resume_policy = PipelineResumePolicy::Disabled;
        resumed.stop_reason.clear();
        repo.compare_and_swap(4, &resumed).unwrap();

        assert_eq!(repo.load_current("run_1").unwrap(), Some(resumed));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recovery_blocked_checkpoint_can_return_to_recoverable_after_reconciliation() {
        let root = temp_dir("recovery-blocked-reconcile");
        let repo = PipelineCheckpointRepository::new(&root);
        let mut blocked = checkpoint("run_1", "attempt_1", 1);
        blocked.status = PipelineCheckpointStatus::RecoveryBlocked;
        blocked.units[0].status = PipelineUnitStatus::Unknown;
        blocked.units[0].reconcile_required = true;
        blocked.recovery_blocked_reason = "unknown side effect".to_string();
        repo.compare_and_swap(0, &blocked).unwrap();

        let mut reconciled = blocked.clone();
        reconciled.revision = 2;
        reconciled.status = PipelineCheckpointStatus::Recoverable;
        reconciled.resume_policy = PipelineResumePolicy::ExplicitOnly;
        reconciled.units[0].status = PipelineUnitStatus::Pending;
        reconciled.units[0].reconcile_required = false;
        reconciled.recovery_blocked_reason.clear();
        repo.compare_and_swap(1, &reconciled).unwrap();

        assert_eq!(repo.load_current("run_1").unwrap(), Some(reconciled));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn truncated_checkpoint_is_quarantined() {
        let root = temp_dir("corrupt");
        let repo = PipelineCheckpointRepository::new(&root);
        let path = repo.current_path("run_1").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{\"schema_version\":1").unwrap();
        assert!(repo.load_current("run_1").is_err());
        assert!(!path.exists());
        assert!(fs::read_dir(path.parent().unwrap()).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".corrupt-")
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn identical_save_is_idempotent_but_conflicting_revision_is_rejected() {
        let root = temp_dir("revision");
        let repo = PipelineCheckpointRepository::new(&root);
        let first = checkpoint("run_1", "attempt_1", 1);
        repo.save_attempt_and_current(&first).unwrap();
        repo.save_attempt_and_current(&first).unwrap();
        let mut conflicting = first.clone();
        conflicting.stop_reason = "different content at the same revision".to_string();
        assert!(repo.save_attempt_and_current(&conflicting).is_err());
        let orphan_attempt = checkpoint("run_1", "attempt_2", 2);
        assert!(repo.save_attempt_and_current(&orphan_attempt).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn current_is_the_commit_point_and_read_repairs_missing_attempt_archive() {
        let root = temp_dir("commit-point");
        let repo = PipelineCheckpointRepository::new(&root);
        let first = checkpoint("run_1", "attempt_1", 1);
        repo.inject_fail_after_current_once();

        assert!(repo.save_attempt_and_current(&first).is_err());
        let current_path = repo.current_path("run_1").unwrap();
        let attempt_path = repo.attempt_path("run_1", "attempt_1").unwrap();
        assert_eq!(read_checkpoint_direct(&current_path), first);
        assert!(!attempt_path.exists());

        assert_eq!(repo.load_current("run_1").unwrap(), Some(first.clone()));
        assert_eq!(read_checkpoint_direct(&attempt_path), first);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_promotes_the_highest_legal_attempt_revision_to_current() {
        let root = temp_dir("promote-attempt");
        let repo = PipelineCheckpointRepository::new(&root);
        let first = checkpoint("run_1", "attempt_1", 1);
        repo.save_attempt_and_current(&first).unwrap();
        let mut second = first.clone();
        second.revision = 2;
        second.units[0].status = PipelineUnitStatus::Committed;
        second.next_unit_id = Some("01:stage".to_string());
        write_checkpoint(&repo.attempt_path("run_1", "attempt_1").unwrap(), &second).unwrap();

        assert_eq!(repo.load_current("run_1").unwrap(), Some(second.clone()));
        assert_eq!(
            read_checkpoint_direct(&repo.current_path("run_1").unwrap()),
            second
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn valid_attempt_recovers_a_corrupt_current_commit() {
        let root = temp_dir("recover-current");
        let repo = PipelineCheckpointRepository::new(&root);
        let first = checkpoint("run_1", "attempt_1", 1);
        repo.save_attempt_and_current(&first).unwrap();
        let current_path = repo.current_path("run_1").unwrap();
        fs::write(&current_path, "{truncated").unwrap();

        assert_eq!(repo.load_current("run_1").unwrap(), Some(first.clone()));
        assert_eq!(read_checkpoint_direct(&current_path), first);
        assert!(
            fs::read_dir(current_path.parent().unwrap())
                .unwrap()
                .any(|entry| {
                    entry
                        .unwrap()
                        .file_name()
                        .to_string_lossy()
                        .contains("current.json.corrupt-")
                })
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn compare_and_swap_rejects_stale_or_skipped_revisions() {
        let root = temp_dir("cas");
        let repo = PipelineCheckpointRepository::new(&root);
        let first = checkpoint("run_1", "attempt_1", 1);
        repo.compare_and_swap(0, &first).unwrap();

        let mut second = first.clone();
        second.revision = 2;
        second.units[0].status = PipelineUnitStatus::Committed;
        second.next_unit_id = Some("01:stage".to_string());
        assert!(repo.compare_and_swap(0, &second).is_err());
        repo.compare_and_swap(1, &second).unwrap();

        let mut skipped = second.clone();
        skipped.revision = 4;
        assert!(repo.compare_and_swap(2, &skipped).is_err());
        assert!(repo.compare_and_swap(1, &skipped).is_err());
        assert_eq!(repo.load_current("run_1").unwrap(), Some(second));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn completed_checkpoint_cannot_return_to_running() {
        let root = temp_dir("terminal-regression");
        let repo = PipelineCheckpointRepository::new(&root);
        let mut completed = checkpoint("run_1", "attempt_1", 1);
        completed.status = PipelineCheckpointStatus::Completed;
        completed.resume_policy = PipelineResumePolicy::Disabled;
        completed.current_stage_id = None;
        completed.current_unit_id = None;
        completed.next_unit_id = None;
        for unit in &mut completed.units {
            unit.status = PipelineUnitStatus::Committed;
        }
        repo.compare_and_swap(0, &completed).unwrap();

        let mut regressed = completed.clone();
        regressed.revision = 2;
        regressed.status = PipelineCheckpointStatus::Running;
        assert!(repo.compare_and_swap(1, &regressed).is_err());
        assert_eq!(repo.load_current("run_1").unwrap(), Some(completed));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn committed_and_skipped_units_cannot_regress_or_be_rewritten() {
        for (label, terminal_unit_status) in [
            ("committed", PipelineUnitStatus::Committed),
            ("skipped", PipelineUnitStatus::Skipped),
        ] {
            let root = temp_dir(&format!("unit-{label}-regression"));
            let repo = PipelineCheckpointRepository::new(&root);
            let mut current = checkpoint("run_1", "attempt_1", 1);
            current.units[0].status = terminal_unit_status;
            current.next_unit_id = Some("01:stage".to_string());
            repo.compare_and_swap(0, &current).unwrap();

            let mut regressed = current.clone();
            regressed.revision = 2;
            regressed.units[0].status = PipelineUnitStatus::Pending;
            assert!(repo.compare_and_swap(1, &regressed).is_err());

            let mut rewritten = current.clone();
            rewritten.revision = 2;
            rewritten.units[0].result_fingerprint = "different".to_string();
            assert!(repo.compare_and_swap(1, &rewritten).is_err());
            assert_eq!(repo.load_current("run_1").unwrap(), Some(current));
            let _ = fs::remove_dir_all(root);
        }
    }

    #[test]
    fn child_attempt_requires_recoverable_parent_and_resuming_status() {
        let root = temp_dir("attempt-transition");
        let repo = PipelineCheckpointRepository::new(&root);
        let running = checkpoint("run_1", "attempt_1", 1);
        repo.compare_and_swap(0, &running).unwrap();

        let mut illegal_child = running.clone();
        illegal_child.revision = 2;
        illegal_child.identity.attempt_id = "attempt_2".to_string();
        illegal_child.identity.parent_attempt_id = Some("attempt_1".to_string());
        illegal_child.status = PipelineCheckpointStatus::Resuming;
        assert!(repo.compare_and_swap(1, &illegal_child).is_err());
        assert_eq!(repo.load_current("run_1").unwrap(), Some(running));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_compare_and_swap_allows_exactly_one_writer() {
        let root = temp_dir("cas-concurrent");
        let repo = Arc::new(PipelineCheckpointRepository::new(&root));
        let first = checkpoint("run_1", "attempt_1", 1);
        repo.compare_and_swap(0, &first).unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let mut handles = Vec::new();
        for label in ["left", "right"] {
            let repo = Arc::clone(&repo);
            let barrier = Arc::clone(&barrier);
            let mut next = first.clone();
            next.revision = 2;
            next.stop_reason = label.to_string();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                repo.compare_and_swap(1, &next)
            }));
        }
        barrier.wait();
        let results = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
        assert_eq!(repo.load_current("run_1").unwrap().unwrap().revision, 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn semantic_validation_rejects_out_of_range_or_rewinding_units() {
        let root = temp_dir("semantic-range");
        let repo = PipelineCheckpointRepository::new(&root);

        let mut outside = checkpoint("run_outside", "attempt_1", 1);
        outside.next_unit_id = Some("99:stage".to_string());
        assert!(repo.save_attempt_and_current(&outside).is_err());

        let mut rewind = checkpoint("run_rewind", "attempt_1", 1);
        rewind.current_stage_id = Some("01".to_string());
        rewind.current_unit_id = Some("01:stage".to_string());
        rewind.next_unit_id = Some("00:stage".to_string());
        assert!(repo.save_attempt_and_current(&rewind).is_err());

        let mut external_unit = checkpoint("run_external", "attempt_1", 1);
        external_unit.units.push(PipelineUnitCheckpoint {
            stage_id: "99".to_string(),
            unit_id: "99:stage".to_string(),
            idempotency_key: "unit:99".to_string(),
            ..PipelineUnitCheckpoint::default()
        });
        assert!(repo.save_attempt_and_current(&external_unit).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn recoverable_and_completed_checkpoints_enforce_next_and_terminal_invariants() {
        let root = temp_dir("semantic-terminal");
        let repo = PipelineCheckpointRepository::new(&root);

        let mut wrong_next = checkpoint("run_wrong_next", "attempt_1", 1);
        wrong_next.status = PipelineCheckpointStatus::Recoverable;
        wrong_next.resume_policy = PipelineResumePolicy::ExplicitOnly;
        wrong_next.next_unit_id = Some("01:stage".to_string());
        assert!(repo.save_attempt_and_current(&wrong_next).is_err());

        let mut incomplete = checkpoint("run_incomplete", "attempt_1", 1);
        incomplete.status = PipelineCheckpointStatus::Completed;
        incomplete.resume_policy = PipelineResumePolicy::Disabled;
        incomplete.next_unit_id = None;
        incomplete.current_unit_id = None;
        assert!(repo.save_attempt_and_current(&incomplete).is_err());

        let mut completed = checkpoint("run_completed", "attempt_1", 1);
        completed.status = PipelineCheckpointStatus::Completed;
        completed.resume_policy = PipelineResumePolicy::Disabled;
        completed.current_unit_id = None;
        completed.next_unit_id = None;
        for unit in &mut completed.units {
            unit.status = PipelineUnitStatus::Committed;
        }
        repo.save_attempt_and_current(&completed).unwrap();
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checkpoint_paths_reject_identifiers_that_need_cleaning() {
        let repo = PipelineCheckpointRepository::new(temp_dir("paths"));
        assert!(repo.current_path("../run").is_err());
        assert!(repo.attempt_path("run 1", "attempt/1").is_err());
    }

    fn checkpoint(run_id: &str, attempt_id: &str, revision: u64) -> PipelineCheckpoint {
        PipelineCheckpoint {
            revision,
            identity: PipelineRunIdentity {
                run_id: run_id.to_string(),
                attempt_id: attempt_id.to_string(),
                project_id: "project_1".to_string(),
                draft_id: "draft_1".to_string(),
                ..PipelineRunIdentity::default()
            },
            range: CanonicalPipelineRange {
                from_stage_id: "00".to_string(),
                to_stage_id: "01".to_string(),
                stage_ids: vec!["00".to_string(), "01".to_string()],
            },
            status: PipelineCheckpointStatus::Running,
            current_stage_id: Some("00".to_string()),
            next_unit_id: Some("00:stage".to_string()),
            units: vec![
                checkpoint_unit("00", PipelineUnitStatus::Pending),
                checkpoint_unit("01", PipelineUnitStatus::Pending),
            ],
            fingerprints: PipelineFingerprints {
                input: "input:v1".to_string(),
                ..PipelineFingerprints::default()
            },
            resume_policy: PipelineResumePolicy::Disabled,
            ..PipelineCheckpoint::default()
        }
    }

    fn checkpoint_unit(stage_id: &str, status: PipelineUnitStatus) -> PipelineUnitCheckpoint {
        PipelineUnitCheckpoint {
            stage_id: stage_id.to_string(),
            unit_id: format!("{stage_id}:stage"),
            status,
            idempotency_key: format!("unit:{stage_id}"),
            ..PipelineUnitCheckpoint::default()
        }
    }

    fn read_checkpoint_direct(path: &Path) -> PipelineCheckpoint {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    fn temp_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm_new_checkpoint_{label}_{}",
            new_stable_id("test").unwrap()
        ))
    }
}
