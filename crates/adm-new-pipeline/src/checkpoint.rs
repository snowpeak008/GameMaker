use std::sync::Mutex;

use adm_new_contracts::pipeline::{
    CanonicalPipelineRange, PipelineCheckpoint, PipelineCheckpointStatus, PipelineFingerprints,
    PipelineResumePolicy, PipelineRunIdentity, PipelineRunState, PipelineUnitCheckpoint,
    PipelineUnitStatus, StageSpec, StageStatus,
};
use adm_new_foundation::{AdmError, AdmResult, sha256_hex, unix_timestamp};
use adm_new_storage::PipelineCheckpointRepository;

use crate::PipelineRunObserver;

#[derive(Debug)]
pub struct PipelineCheckpointObserver {
    repository: PipelineCheckpointRepository,
    checkpoint: Mutex<PipelineCheckpoint>,
}

impl PipelineCheckpointObserver {
    pub fn new(
        repository: PipelineCheckpointRepository,
        checkpoint: PipelineCheckpoint,
    ) -> AdmResult<Self> {
        repository.compare_and_swap(checkpoint.revision.saturating_sub(1), &checkpoint)?;
        Ok(Self {
            repository,
            checkpoint: Mutex::new(checkpoint),
        })
    }

    pub fn snapshot(&self) -> AdmResult<PipelineCheckpoint> {
        self.checkpoint
            .lock()
            .map(|checkpoint| checkpoint.clone())
            .map_err(|_| AdmError::new("pipeline checkpoint observer lock is poisoned"))
    }

    fn update(&self, mutate: impl Fn(&mut PipelineCheckpoint) -> AdmResult<()>) -> AdmResult<()> {
        let mut checkpoint = self
            .checkpoint
            .lock()
            .map_err(|_| AdmError::new("pipeline checkpoint observer lock is poisoned"))?;
        for _ in 0..4 {
            let expected_revision = checkpoint.revision;
            let mut next = checkpoint.clone();
            mutate(&mut next)?;
            next.revision = expected_revision.saturating_add(1);
            next.updated_at = timestamp();
            match self.repository.compare_and_swap(expected_revision, &next) {
                Ok(()) => {
                    *checkpoint = next;
                    return Ok(());
                }
                Err(error) => {
                    let Some(latest) = self.repository.load_current(&checkpoint.identity.run_id)?
                    else {
                        return Err(error);
                    };
                    if latest.revision == expected_revision {
                        return Err(error);
                    }
                    *checkpoint = latest;
                }
            }
        }
        Err(AdmError::new(
            "pipeline checkpoint changed repeatedly during a state transition",
        ))
    }
}

impl PipelineRunObserver for PipelineCheckpointObserver {
    fn before_stage(&self, spec: &StageSpec, _state: &PipelineRunState) -> AdmResult<()> {
        self.update(|checkpoint| {
            let unit_id = whole_stage_unit_id(&spec.stage_id);
            checkpoint.status = if matches!(
                checkpoint.status,
                PipelineCheckpointStatus::StopRequested | PipelineCheckpointStatus::Stopping
            ) {
                PipelineCheckpointStatus::Stopping
            } else {
                PipelineCheckpointStatus::Running
            };
            checkpoint.resume_policy = PipelineResumePolicy::Disabled;
            checkpoint.current_stage_id = Some(spec.stage_id.clone());
            checkpoint.current_unit_id = Some(unit_id.clone());
            checkpoint.next_unit_id = Some(unit_id.clone());
            let unit = checkpoint_unit_mut(checkpoint, &spec.stage_id, &unit_id);
            unit.status = PipelineUnitStatus::Running;
            unit.started_at = timestamp();
            unit.completed_at.clear();
            unit.reconcile_required = true;
            unit.failure_message.clear();
            Ok(())
        })
    }

    fn after_stage(&self, spec: &StageSpec, state: &PipelineRunState) -> AdmResult<()> {
        let runtime = state.stages.get(&spec.stage_id).ok_or_else(|| {
            AdmError::new(format!(
                "pipeline state is missing completed stage {}",
                spec.stage_id
            ))
        })?;
        let result = runtime.result.as_ref().ok_or_else(|| {
            AdmError::new(format!(
                "pipeline state is missing stage result {}",
                spec.stage_id
            ))
        })?;
        let result_fingerprint = sha256_hex(&serde_json::to_vec(result).map_err(|error| {
            AdmError::new(format!("failed to fingerprint stage result: {error}"))
        })?);
        let internal_recovery_blocked = result
            .outputs
            .get("recovery_blocked")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let internal_stop_requested = result
            .outputs
            .get("stop_requested")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        self.update(|checkpoint| {
            let unit_id = whole_stage_unit_id(&spec.stage_id);
            let next_unit_id = next_whole_stage_unit(checkpoint, &spec.stage_id);
            let unit = checkpoint_unit_mut(checkpoint, &spec.stage_id, &unit_id);
            unit.completed_at = timestamp();
            unit.result_fingerprint = result_fingerprint.clone();
            unit.output_refs = result.outputs.keys().cloned().collect();
            if internal_recovery_blocked {
                unit.status = PipelineUnitStatus::Unknown;
                unit.reconcile_required = true;
                unit.failure_message =
                    "internal work unit side effect requires reconciliation".to_string();
            } else if internal_stop_requested {
                unit.status = PipelineUnitStatus::Pending;
                unit.reconcile_required = false;
                unit.result_fingerprint.clear();
                unit.output_refs.clear();
                unit.failure_message.clear();
            } else {
                match &result.status {
                    StageStatus::Success
                    | StageStatus::Skipped
                    | StageStatus::CompletedWithReview
                    | StageStatus::WaitingConfirmation => {
                        unit.status = PipelineUnitStatus::Committed;
                        unit.reconcile_required = false;
                    }
                    StageStatus::Stopped => {
                        if result.message == "stop requested before stage execution" {
                            unit.status = PipelineUnitStatus::Pending;
                            unit.reconcile_required = false;
                            unit.result_fingerprint.clear();
                            unit.output_refs.clear();
                        } else {
                            unit.status = PipelineUnitStatus::Unknown;
                            unit.reconcile_required = true;
                        }
                    }
                    StageStatus::Blocked | StageStatus::Failed => {
                        unit.status = PipelineUnitStatus::Failed;
                        unit.reconcile_required = false;
                        unit.failure_message = result.message.clone();
                    }
                }
            }
            checkpoint.next_unit_id = next_unit_id.clone();
            if internal_recovery_blocked {
                checkpoint.status = PipelineCheckpointStatus::RecoveryBlocked;
                checkpoint.resume_policy = PipelineResumePolicy::Disabled;
                checkpoint.next_unit_id = Some(unit_id);
                checkpoint.recovery_blocked_reason = format!(
                    "stage {} contains an unknown internal work unit side effect",
                    spec.stage_id
                );
            } else if internal_stop_requested {
                checkpoint.status = PipelineCheckpointStatus::Recoverable;
                checkpoint.resume_policy = PipelineResumePolicy::ExplicitOnly;
                checkpoint.next_unit_id = Some(unit_id);
            } else {
                match &result.status {
                    StageStatus::WaitingConfirmation => {
                        checkpoint.status = PipelineCheckpointStatus::WaitingConfirmation;
                        checkpoint.resume_policy = PipelineResumePolicy::Disabled;
                        checkpoint.next_unit_id = None;
                    }
                    StageStatus::Stopped => {
                        checkpoint.status = PipelineCheckpointStatus::Recoverable;
                        checkpoint.resume_policy = PipelineResumePolicy::ExplicitOnly;
                        checkpoint.next_unit_id = Some(unit_id);
                    }
                    StageStatus::Blocked | StageStatus::Failed => {
                        checkpoint.status = PipelineCheckpointStatus::Failed;
                        checkpoint.resume_policy = PipelineResumePolicy::Disabled;
                    }
                    _ => {}
                }
            }
            Ok(())
        })
    }

    fn finish(&self, state: &PipelineRunState) -> AdmResult<()> {
        self.update(|checkpoint| {
            if checkpoint.status == PipelineCheckpointStatus::RecoveryBlocked
                || (checkpoint.status == PipelineCheckpointStatus::Recoverable
                    && state.status == "failed")
            {
                return Ok(());
            }
            match state.status.as_str() {
                "success" => {
                    if checkpoint
                        .units
                        .iter()
                        .any(|unit| unit.status != PipelineUnitStatus::Committed)
                    {
                        return Err(AdmError::new(
                            "pipeline cannot complete while checkpoint units remain uncommitted",
                        ));
                    }
                    checkpoint.status = PipelineCheckpointStatus::Completed;
                    checkpoint.resume_policy = PipelineResumePolicy::Disabled;
                    checkpoint.current_unit_id = None;
                    checkpoint.next_unit_id = None;
                }
                "waiting_confirmation" | "style_confirmed" => {
                    checkpoint.status = PipelineCheckpointStatus::WaitingConfirmation;
                    checkpoint.resume_policy = PipelineResumePolicy::Disabled;
                    checkpoint.next_unit_id = None;
                }
                "stopped" => {
                    if checkpoint
                        .units
                        .iter()
                        .all(|unit| unit.status == PipelineUnitStatus::Committed)
                    {
                        checkpoint.status = PipelineCheckpointStatus::Completed;
                        checkpoint.resume_policy = PipelineResumePolicy::Disabled;
                        checkpoint.current_unit_id = None;
                        checkpoint.next_unit_id = None;
                        if checkpoint.stop_reason.is_empty() {
                            checkpoint.stop_reason = "stop_observed_at_final_boundary".to_string();
                        }
                        if checkpoint.stop_boundary.is_empty() {
                            checkpoint.stop_boundary = "pipeline_complete".to_string();
                        }
                    } else {
                        checkpoint.status = PipelineCheckpointStatus::Recoverable;
                        checkpoint.resume_policy = PipelineResumePolicy::ExplicitOnly;
                        if checkpoint.next_unit_id.is_none() {
                            checkpoint.next_unit_id = checkpoint.current_unit_id.clone();
                        }
                    }
                }
                "failed" | "blocked" => {
                    checkpoint.status = PipelineCheckpointStatus::Failed;
                    checkpoint.resume_policy = PipelineResumePolicy::Disabled;
                }
                _ => {
                    checkpoint.status = PipelineCheckpointStatus::Running;
                    checkpoint.resume_policy = PipelineResumePolicy::Disabled;
                }
            }
            Ok(())
        })
    }
}

pub fn initial_whole_stage_checkpoint(
    identity: PipelineRunIdentity,
    range: CanonicalPipelineRange,
    fingerprints: PipelineFingerprints,
) -> PipelineCheckpoint {
    let now = timestamp();
    let units = range
        .stage_ids
        .iter()
        .map(|stage_id| PipelineUnitCheckpoint {
            stage_id: stage_id.clone(),
            unit_id: whole_stage_unit_id(stage_id),
            status: PipelineUnitStatus::Pending,
            idempotency_key: sha256_hex(
                format!(
                    "{}:{}:{}",
                    identity.run_id, stage_id, fingerprints.execution_plan
                )
                .as_bytes(),
            ),
            ..PipelineUnitCheckpoint::default()
        })
        .collect::<Vec<_>>();
    PipelineCheckpoint {
        revision: 1,
        current_stage_id: range.stage_ids.first().cloned(),
        next_unit_id: range
            .stage_ids
            .first()
            .map(|stage_id| whole_stage_unit_id(stage_id)),
        identity,
        range,
        units,
        fingerprints,
        status: PipelineCheckpointStatus::Running,
        resume_policy: PipelineResumePolicy::Disabled,
        created_at: now.clone(),
        updated_at: now,
        ..PipelineCheckpoint::default()
    }
}

pub fn whole_stage_unit_id(stage_id: &str) -> String {
    format!("{stage_id}:stage")
}

fn checkpoint_unit_mut<'a>(
    checkpoint: &'a mut PipelineCheckpoint,
    stage_id: &str,
    unit_id: &str,
) -> &'a mut PipelineUnitCheckpoint {
    if let Some(index) = checkpoint
        .units
        .iter()
        .position(|unit| unit.stage_id == stage_id && unit.unit_id == unit_id)
    {
        return &mut checkpoint.units[index];
    }
    checkpoint.units.push(PipelineUnitCheckpoint {
        stage_id: stage_id.to_string(),
        unit_id: unit_id.to_string(),
        ..PipelineUnitCheckpoint::default()
    });
    checkpoint
        .units
        .last_mut()
        .expect("checkpoint unit inserted")
}

fn next_whole_stage_unit(checkpoint: &PipelineCheckpoint, stage_id: &str) -> Option<String> {
    checkpoint
        .range
        .stage_ids
        .iter()
        .position(|candidate| candidate == stage_id)
        .and_then(|index| checkpoint.range.stage_ids.get(index + 1))
        .map(|next| whole_stage_unit_id(next))
}

fn timestamp() -> String {
    format!("unix:{}", unix_timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::pipeline::{PipelineStageResult, PipelineStageRuntime, StageKind};
    use adm_new_foundation::new_stable_id;
    use std::collections::BTreeMap;

    #[test]
    fn whole_stage_observer_commits_each_stage_and_finishes_checkpoint() {
        let root = std::env::temp_dir().join(format!(
            "adm-new-pipeline-observer-{}",
            new_stable_id("test").unwrap()
        ));
        let repository = PipelineCheckpointRepository::new(&root);
        let checkpoint = initial_whole_stage_checkpoint(
            PipelineRunIdentity {
                run_id: "run_1".to_string(),
                attempt_id: "attempt_1".to_string(),
                project_id: "project_1".to_string(),
                draft_id: "draft_1".to_string(),
                ..PipelineRunIdentity::default()
            },
            CanonicalPipelineRange {
                from_stage_id: "00".to_string(),
                to_stage_id: "00".to_string(),
                stage_ids: vec!["00".to_string()],
            },
            PipelineFingerprints {
                execution_plan: "plan-v1".to_string(),
                ..PipelineFingerprints::default()
            },
        );
        let observer = PipelineCheckpointObserver::new(repository, checkpoint).unwrap();
        let spec = StageSpec {
            stage_id: "00".to_string(),
            kind: StageKind::Development,
            number: Some(0),
            slug: String::new(),
            title: String::new(),
            requires: Vec::new(),
            source_groups: Vec::new(),
            plugin_ref: String::new(),
            metadata: BTreeMap::new(),
        };
        let mut state = PipelineRunState {
            run_id: "run_1".to_string(),
            status: "running".to_string(),
            stop_requested: false,
            current_stage_id: Some("00".to_string()),
            stages: BTreeMap::new(),
            ..PipelineRunState::default()
        };
        observer.before_stage(&spec, &state).unwrap();
        state.stages.insert(
            "00".to_string(),
            PipelineStageRuntime {
                stage_id: "00".to_string(),
                status: StageStatus::Success,
                started_at: timestamp(),
                completed_at: timestamp(),
                result: Some(PipelineStageResult {
                    status: StageStatus::Success,
                    outputs: BTreeMap::from([("summary".to_string(), serde_json::json!(true))]),
                    errors: Vec::new(),
                    warnings: Vec::new(),
                    message: "done".to_string(),
                }),
            },
        );
        observer.after_stage(&spec, &state).unwrap();
        state.status = "success".to_string();
        observer.finish(&state).unwrap();

        let snapshot = observer.snapshot().unwrap();
        assert_eq!(snapshot.status, PipelineCheckpointStatus::Completed);
        assert_eq!(snapshot.units[0].status, PipelineUnitStatus::Committed);
        assert_eq!(snapshot.next_unit_id, None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn internal_work_unit_uncertainty_marks_checkpoint_recovery_blocked() {
        let (root, observer, spec, mut state) = observer_fixture("recovery_blocked");
        observer.before_stage(&spec, &state).unwrap();
        insert_failed_result(&mut state, "recovery_blocked");
        observer.after_stage(&spec, &state).unwrap();
        state.status = "failed".to_string();
        observer.finish(&state).unwrap();

        let snapshot = observer.snapshot().unwrap();
        assert_eq!(snapshot.status, PipelineCheckpointStatus::RecoveryBlocked);
        assert_eq!(snapshot.resume_policy, PipelineResumePolicy::Disabled);
        assert_eq!(snapshot.units[0].status, PipelineUnitStatus::Unknown);
        assert!(snapshot.units[0].reconcile_required);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn internal_stop_at_unit_boundary_keeps_checkpoint_explicitly_recoverable() {
        let (root, observer, spec, mut state) = observer_fixture("unit_stop");
        observer.before_stage(&spec, &state).unwrap();
        insert_failed_result(&mut state, "stop_requested");
        observer.after_stage(&spec, &state).unwrap();
        state.status = "failed".to_string();
        observer.finish(&state).unwrap();

        let snapshot = observer.snapshot().unwrap();
        assert_eq!(snapshot.status, PipelineCheckpointStatus::Recoverable);
        assert_eq!(snapshot.resume_policy, PipelineResumePolicy::ExplicitOnly);
        assert_eq!(snapshot.units[0].status, PipelineUnitStatus::Pending);
        assert!(!snapshot.units[0].reconcile_required);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn stop_observed_after_final_commit_records_completed_boundary() {
        let (root, observer, spec, mut state) = observer_fixture("final_boundary_stop");
        observer.before_stage(&spec, &state).unwrap();
        state.stages.insert(
            "11".to_string(),
            PipelineStageRuntime {
                stage_id: "11".to_string(),
                status: StageStatus::Success,
                started_at: timestamp(),
                completed_at: timestamp(),
                result: Some(PipelineStageResult {
                    status: StageStatus::Success,
                    outputs: BTreeMap::new(),
                    errors: Vec::new(),
                    warnings: Vec::new(),
                    message: "committed".to_string(),
                }),
            },
        );
        observer.after_stage(&spec, &state).unwrap();
        state.status = "stopped".to_string();
        state.stop_requested = true;
        observer.finish(&state).unwrap();

        let snapshot = observer.snapshot().unwrap();
        assert_eq!(snapshot.status, PipelineCheckpointStatus::Completed);
        assert_eq!(snapshot.stop_reason, "stop_observed_at_final_boundary");
        assert_eq!(snapshot.stop_boundary, "pipeline_complete");
        assert_eq!(snapshot.next_unit_id, None);
        let _ = std::fs::remove_dir_all(root);
    }

    fn observer_fixture(
        suffix: &str,
    ) -> (
        std::path::PathBuf,
        PipelineCheckpointObserver,
        StageSpec,
        PipelineRunState,
    ) {
        let root = std::env::temp_dir().join(format!(
            "adm-new-pipeline-observer-{suffix}-{}",
            new_stable_id("test").unwrap()
        ));
        let repository = PipelineCheckpointRepository::new(&root);
        let checkpoint = initial_whole_stage_checkpoint(
            PipelineRunIdentity {
                run_id: format!("run_{suffix}"),
                attempt_id: "attempt_1".to_string(),
                project_id: "project_1".to_string(),
                draft_id: "draft_1".to_string(),
                ..PipelineRunIdentity::default()
            },
            CanonicalPipelineRange {
                from_stage_id: "11".to_string(),
                to_stage_id: "11".to_string(),
                stage_ids: vec!["11".to_string()],
            },
            PipelineFingerprints::default(),
        );
        let observer = PipelineCheckpointObserver::new(repository, checkpoint).unwrap();
        let spec = StageSpec {
            stage_id: "11".to_string(),
            kind: StageKind::Development,
            number: Some(11),
            slug: String::new(),
            title: String::new(),
            requires: Vec::new(),
            source_groups: Vec::new(),
            plugin_ref: String::new(),
            metadata: BTreeMap::new(),
        };
        let state = PipelineRunState {
            run_id: format!("run_{suffix}"),
            status: "running".to_string(),
            current_stage_id: Some("11".to_string()),
            ..PipelineRunState::default()
        };
        (root, observer, spec, state)
    }

    fn insert_failed_result(state: &mut PipelineRunState, flag: &str) {
        state.stages.insert(
            "11".to_string(),
            PipelineStageRuntime {
                stage_id: "11".to_string(),
                status: StageStatus::Failed,
                started_at: timestamp(),
                completed_at: timestamp(),
                result: Some(PipelineStageResult {
                    status: StageStatus::Failed,
                    outputs: BTreeMap::from([(flag.to_string(), serde_json::json!(true))]),
                    errors: Vec::new(),
                    warnings: Vec::new(),
                    message: "internal work unit did not finish".to_string(),
                }),
            },
        );
    }
}
