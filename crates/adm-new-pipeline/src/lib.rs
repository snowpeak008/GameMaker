#![forbid(unsafe_code)]

mod artifact_view;
mod checkpoint;
pub mod design_flow;
mod development_registry;
pub mod generation;
mod product_executor;
pub mod source;
mod stage_result;
pub mod stages;
pub mod style_image;
pub mod work_units;

pub use artifact_view::PipelineArtifactRecord;
pub use checkpoint::{
    PipelineCheckpointObserver, initial_whole_stage_checkpoint, whole_stage_unit_id,
};
pub use development_registry::default_development_registry;
pub use product_executor::ProductPipelineExecutor;
pub use style_image::{StyleImageGenerator, StyleImageRequest, StyleImageResult, StyleImageStatus};
pub use work_units::{
    OfflineVerifiedWorkUnitExecutor, SafeUnitJournal, StageWorkUnitReconcileStatus,
    WorkUnitBatchOutcome, WorkUnitExecutionResult, WorkUnitExecutionStatus, WorkUnitExecutor,
    WorkUnitJournalPhase, WorkUnitJournalRecord, WorkUnitKind, WorkUnitReconcileDecision,
    WorkUnitRequest, WorkUnitRunOutcome, WorkUnitRunStatus, WorkUnitStopToken,
    execute_work_unit_batch, reconcile_checkpoint_stage_from_journal,
};

use std::collections::{BTreeMap, BTreeSet};

use adm_new_contracts::pipeline::{
    PipelineRegistry, PipelineRunState, PipelineStageResult, PipelineStageRuntime,
    StageContextModel, StageSpec, StageStatus,
};
use adm_new_foundation::{AdmError, AdmResult, new_stable_id, unix_timestamp};

pub const CRATE_NAME: &str = "adm-new-pipeline";

pub fn crate_ready() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct PipelineService {
    registry: PipelineRegistry,
    stages_by_id: BTreeMap<String, StageSpec>,
    stage_ids_by_number: BTreeMap<u32, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineRunReport {
    pub ordered_stage_ids: Vec<String>,
    pub executed_stage_ids: Vec<String>,
    pub final_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPipelineRange {
    pub ordered_stage_ids: Vec<String>,
    pub from_stage_id: String,
    pub to_stage_id: String,
    pub from_index: usize,
    pub to_index: usize,
}

pub trait StageExecutor {
    fn execute(&self, spec: &StageSpec, context: &StageContextModel) -> PipelineStageResult;

    fn stop_requested(&self) -> bool {
        false
    }

    fn skip_manual_gate(
        &self,
        spec: &StageSpec,
        _context: &StageContextModel,
        mut result: PipelineStageResult,
    ) -> PipelineStageResult {
        result.status = StageStatus::Skipped;
        result.outputs.insert(
            "manual_gate_skipped".to_string(),
            serde_json::Value::Bool(true),
        );
        result
            .warnings
            .push(format!("manual gate {} skipped by request", spec.stage_id));
        result.message = format!("manual gate {} skipped", spec.stage_id);
        result
    }
}

pub trait PipelineRunObserver {
    fn before_stage(&self, _spec: &StageSpec, _state: &PipelineRunState) -> AdmResult<()> {
        Ok(())
    }

    fn after_stage(&self, _spec: &StageSpec, _state: &PipelineRunState) -> AdmResult<()> {
        Ok(())
    }

    fn finish(&self, _state: &PipelineRunState) -> AdmResult<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopPipelineRunObserver;

impl PipelineRunObserver for NoopPipelineRunObserver {}

impl PipelineService {
    pub fn new(registry: PipelineRegistry) -> AdmResult<Self> {
        let mut stages_by_id = BTreeMap::new();
        let mut stage_ids_by_number = BTreeMap::new();
        for stage in &registry.stages {
            if stage.stage_id.trim().is_empty() {
                return Err(AdmError::new("stage_id cannot be empty"));
            }
            if stages_by_id
                .insert(stage.stage_id.clone(), stage.clone())
                .is_some()
            {
                return Err(AdmError::new(format!(
                    "duplicated stage_id: {}",
                    stage.stage_id
                )));
            }
            if let Some(number) = stage.number
                && let Some(existing_stage_id) =
                    stage_ids_by_number.insert(number, stage.stage_id.clone())
            {
                return Err(AdmError::new(format!(
                    "duplicated stage number {number}: {existing_stage_id} and {}",
                    stage.stage_id
                )));
            }
        }
        for stage in &registry.stages {
            for required in &stage.requires {
                if !stages_by_id.contains_key(required) {
                    return Err(AdmError::new(format!(
                        "stage {} requires unknown dependency {required}",
                        stage.stage_id
                    )));
                }
            }
        }
        Ok(Self {
            registry,
            stages_by_id,
            stage_ids_by_number,
        })
    }

    pub fn registry(&self) -> &PipelineRegistry {
        &self.registry
    }

    pub fn topological_order(&self) -> AdmResult<Vec<String>> {
        let mut incoming = self
            .stages_by_id
            .keys()
            .map(|stage_id| (stage_id.clone(), 0usize))
            .collect::<BTreeMap<_, _>>();
        let mut outgoing = self
            .stages_by_id
            .keys()
            .map(|stage_id| (stage_id.clone(), Vec::<String>::new()))
            .collect::<BTreeMap<_, _>>();

        for stage in self.stages_by_id.values() {
            for required in &stage.requires {
                *incoming
                    .get_mut(&stage.stage_id)
                    .ok_or_else(|| AdmError::new("stage missing incoming slot"))? += 1;
                outgoing
                    .get_mut(required)
                    .ok_or_else(|| AdmError::new("stage missing outgoing slot"))?
                    .push(stage.stage_id.clone());
            }
        }

        let mut ready = incoming
            .iter()
            .filter(|(_, count)| **count == 0)
            .map(|(stage_id, _)| stage_id.clone())
            .collect::<BTreeSet<_>>();
        let mut order = Vec::new();
        while let Some(stage_id) = ready.pop_first() {
            order.push(stage_id.clone());
            for next in outgoing.remove(&stage_id).unwrap_or_default() {
                let count = incoming
                    .get_mut(&next)
                    .ok_or_else(|| AdmError::new("stage missing incoming count"))?;
                *count -= 1;
                if *count == 0 {
                    ready.insert(next);
                }
            }
        }
        if order.len() != self.stages_by_id.len() {
            return Err(AdmError::new(
                "pipeline registry contains a dependency cycle",
            ));
        }
        Ok(order)
    }

    pub fn resolve_stage_input(&self, input: &str) -> AdmResult<String> {
        let normalized = input.trim();
        if normalized.is_empty() {
            return Err(AdmError::new("stage input cannot be empty"));
        }
        if !normalized.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(AdmError::new(format!(
                "stage input must be an ASCII decimal integer: {input}"
            )));
        }
        let number = normalized.parse::<u32>().map_err(|_| {
            AdmError::new(format!(
                "stage input is outside the supported range: {input}"
            ))
        })?;
        self.stage_ids_by_number
            .get(&number)
            .cloned()
            .ok_or_else(|| AdmError::new(format!("unknown stage number: {number}")))
    }

    pub fn resolve_range(
        &self,
        from_stage_input: &str,
        to_stage_input: &str,
    ) -> AdmResult<ResolvedPipelineRange> {
        let from_stage_id = self.resolve_stage_input(from_stage_input)?;
        let to_stage_id = self.resolve_stage_input(to_stage_input)?;
        let ordered_stage_ids = self.topological_order()?;
        let from_index = ordered_stage_ids
            .iter()
            .position(|stage_id| stage_id == &from_stage_id)
            .ok_or_else(|| AdmError::new(format!("missing stage in topology: {from_stage_id}")))?;
        let to_index = ordered_stage_ids
            .iter()
            .position(|stage_id| stage_id == &to_stage_id)
            .ok_or_else(|| AdmError::new(format!("missing stage in topology: {to_stage_id}")))?;
        if from_index > to_index {
            return Err(AdmError::new(format!(
                "from stage {from_stage_id} appears after to stage {to_stage_id}"
            )));
        }
        Ok(ResolvedPipelineRange {
            ordered_stage_ids,
            from_stage_id,
            to_stage_id,
            from_index,
            to_index,
        })
    }

    pub fn request_stop(state: &mut PipelineRunState) {
        if !state.stop_requested {
            state.stop_requested = true;
            if state.status == "running" {
                state.status = "stop_requested".to_string();
            }
            state.state_version = state.state_version.saturating_add(1);
        }
    }

    pub fn run_range<E>(
        &self,
        state: &mut PipelineRunState,
        from_stage_id: &str,
        to_stage_id: &str,
        executor: &E,
    ) -> AdmResult<PipelineRunReport>
    where
        E: StageExecutor,
    {
        self.run_range_with_observer(
            state,
            from_stage_id,
            to_stage_id,
            executor,
            &NoopPipelineRunObserver,
        )
    }

    pub fn run_range_with_observer<E, O>(
        &self,
        state: &mut PipelineRunState,
        from_stage_id: &str,
        to_stage_id: &str,
        executor: &E,
        observer: &O,
    ) -> AdmResult<PipelineRunReport>
    where
        E: StageExecutor,
        O: PipelineRunObserver + ?Sized,
    {
        let resolved_range = self.resolve_range(from_stage_id, to_stage_id)?;
        if state.run_id.is_empty() {
            state.run_id = new_stable_id("pipeline_run")?;
        }
        let ResolvedPipelineRange {
            from_stage_id: canonical_from_stage_id,
            to_stage_id: canonical_to_stage_id,
            ordered_stage_ids,
            from_index,
            to_index,
        } = resolved_range;

        state.schema_version = 2;
        state.from_stage_id = canonical_from_stage_id;
        state.to_stage_id = canonical_to_stage_id;
        state.stage_ids = ordered_stage_ids[from_index..=to_index].to_vec();
        state.recovery = None;
        state.status = "running".to_string();
        state.state_version = state.state_version.saturating_add(1);
        let mut executed_stage_ids = Vec::new();
        for stage_id in &ordered_stage_ids[from_index..=to_index] {
            if state.stop_requested {
                state.status = "stopped".to_string();
                state.current_stage_id = Some(stage_id.clone());
                state.current_unit_id = Some(format!("{stage_id}:stage"));
                state.state_version = state.state_version.saturating_add(1);
                state.stages.insert(
                    stage_id.clone(),
                    PipelineStageRuntime {
                        stage_id: stage_id.clone(),
                        status: StageStatus::Stopped,
                        started_at: timestamp(),
                        completed_at: timestamp(),
                        result: Some(PipelineStageResult {
                            status: StageStatus::Stopped,
                            outputs: BTreeMap::new(),
                            errors: Vec::new(),
                            warnings: Vec::new(),
                            message: "stop requested before stage execution".to_string(),
                        }),
                    },
                );
                break;
            }
            let spec = self
                .stages_by_id
                .get(stage_id)
                .ok_or_else(|| AdmError::new(format!("missing stage spec: {stage_id}")))?;
            if let Some(blocked_status) = self.first_blocking_dependency(spec, state) {
                let result = PipelineStageResult {
                    status: StageStatus::Blocked,
                    outputs: BTreeMap::new(),
                    errors: vec![format!(
                        "dependency for {} is not runnable: {blocked_status}",
                        spec.stage_id
                    )],
                    warnings: Vec::new(),
                    message: "dependency blocked pipeline stage".to_string(),
                };
                state.status = "blocked".to_string();
                state.current_stage_id = Some(stage_id.clone());
                state.current_unit_id = Some(format!("{stage_id}:stage"));
                state.state_version = state.state_version.saturating_add(1);
                state.stages.insert(
                    stage_id.clone(),
                    PipelineStageRuntime {
                        stage_id: stage_id.clone(),
                        status: StageStatus::Blocked,
                        started_at: timestamp(),
                        completed_at: timestamp(),
                        result: Some(result),
                    },
                );
                break;
            }
            let context = self.stage_context(spec);
            state.current_stage_id = Some(stage_id.clone());
            state.current_unit_id = Some(format!("{stage_id}:stage"));
            state.state_version = state.state_version.saturating_add(1);
            observer.before_stage(spec, state)?;
            let started_at = timestamp();
            let result = executor.execute(spec, &context);
            let completed_at = timestamp();
            let status = result.status.clone();
            state.stages.insert(
                stage_id.clone(),
                PipelineStageRuntime {
                    stage_id: stage_id.clone(),
                    status: status.clone(),
                    started_at,
                    completed_at,
                    result: Some(result),
                },
            );
            executed_stage_ids.push(stage_id.clone());
            state.state_version = state.state_version.saturating_add(1);
            observer.after_stage(spec, state)?;
            if executor.stop_requested()
                && matches!(
                    &status,
                    StageStatus::Success | StageStatus::Skipped | StageStatus::CompletedWithReview
                )
            {
                state.stop_requested = true;
                state.status = "stopped".to_string();
                break;
            }
            match status {
                StageStatus::Success | StageStatus::Skipped | StageStatus::CompletedWithReview => {
                    state.status = "running".to_string();
                }
                StageStatus::WaitingConfirmation => {
                    state.status = "waiting_confirmation".to_string();
                    break;
                }
                StageStatus::Stopped => {
                    state.status = "stopped".to_string();
                    state.stop_requested = true;
                    break;
                }
                StageStatus::Blocked => {
                    state.status = "blocked".to_string();
                    break;
                }
                StageStatus::Failed => {
                    state.status = "failed".to_string();
                    break;
                }
            }
        }
        if state.status == "running" {
            state.status = "success".to_string();
        }
        state.state_version = state.state_version.saturating_add(1);
        observer.finish(state)?;
        Ok(PipelineRunReport {
            ordered_stage_ids,
            executed_stage_ids,
            final_status: state.status.clone(),
        })
    }

    fn stage_context(&self, spec: &StageSpec) -> StageContextModel {
        StageContextModel {
            stage_id: spec.stage_id.clone(),
            project_root: String::new(),
            inputs: BTreeMap::new(),
            outputs: BTreeMap::new(),
            metadata: spec.metadata.clone(),
            knowledge: BTreeMap::new(),
            skills: BTreeMap::new(),
            test_mode: true,
            artifact_dir: format!("outputs/artifacts/stage_{}", spec.stage_id),
        }
    }

    fn first_blocking_dependency(
        &self,
        spec: &StageSpec,
        state: &PipelineRunState,
    ) -> Option<&'static str> {
        for required in &spec.requires {
            let Some(runtime) = state.stages.get(required) else {
                continue;
            };
            match runtime.status {
                StageStatus::Success | StageStatus::Skipped | StageStatus::CompletedWithReview => {}
                StageStatus::Failed => return Some("failed"),
                StageStatus::Blocked => return Some("blocked"),
                StageStatus::Stopped => return Some("stopped"),
                StageStatus::WaitingConfirmation => return Some("waiting_confirmation"),
            }
        }
        None
    }
}

fn timestamp() -> String {
    format!("unix:{}", unix_timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::pipeline::{PipelineRegistry, StageKind, StageSpec};
    use serde_json::json;
    use std::cell::Cell;

    #[test]
    fn crate_reports_ready() {
        assert!(crate_ready());
        assert_eq!(CRATE_NAME, "adm-new-pipeline");
    }

    #[test]
    fn pipeline_topological_order_uses_registry_dependencies() {
        let service = PipelineService::new(sample_registry()).unwrap();
        let order = service.topological_order().unwrap();
        assert_before(&order, "08", "10");
        assert_before(&order, "09", "10");
        assert_before(&order, "10", "11");
        assert_before(&order, "10", "12");
        assert_before(&order, "13", "14");
    }

    #[test]
    fn stage_input_resolution_uses_declared_numbers_and_returns_canonical_ids() {
        let service = PipelineService::new(PipelineRegistry {
            stages: vec![
                numbered_stage("opening", 0, vec![]),
                numbered_stage("concept", 1, vec!["opening"]),
                numbered_stage("delivery", 12, vec!["concept"]),
            ],
        })
        .unwrap();

        assert_eq!(service.resolve_stage_input("0").unwrap(), "opening");
        assert_eq!(service.resolve_stage_input("00").unwrap(), "opening");
        assert_eq!(service.resolve_stage_input("1").unwrap(), "concept");
        assert_eq!(service.resolve_stage_input("01").unwrap(), "concept");
        assert_eq!(service.resolve_stage_input(" 012 ").unwrap(), "delivery");
        assert!(service.resolve_stage_input("opening").is_err());
    }

    #[test]
    fn stage_input_resolution_rejects_invalid_overflow_and_unknown_numbers() {
        let service = PipelineService::new(sample_registry()).unwrap();

        for invalid in ["", "   ", "+1", "-1", "1.0", "1a", "１", "4294967296", "99"] {
            assert!(
                service.resolve_stage_input(invalid).is_err(),
                "{invalid:?} should be rejected"
            );
        }
    }

    #[test]
    fn pipeline_rejects_duplicate_declared_stage_numbers() {
        let duplicated = PipelineRegistry {
            stages: vec![
                numbered_stage("first", 1, vec![]),
                numbered_stage("second", 1, vec![]),
            ],
        };

        let error = PipelineService::new(duplicated).unwrap_err();
        assert!(error.to_string().contains("duplicated stage number 1"));
    }

    #[test]
    fn range_resolution_validates_order_by_topology_instead_of_stage_number() {
        let service = PipelineService::new(PipelineRegistry {
            stages: vec![
                numbered_stage("topology-first", 9, vec![]),
                numbered_stage("topology-last", 1, vec!["topology-first"]),
            ],
        })
        .unwrap();

        let range = service.resolve_range("09", "1").unwrap();
        assert_eq!(range.from_stage_id, "topology-first");
        assert_eq!(range.to_stage_id, "topology-last");
        assert!(service.resolve_range("1", "9").is_err());
    }

    #[test]
    fn pipeline_run_range_executes_registry_order_and_records_state() {
        let service = PipelineService::new(sample_registry()).unwrap();
        let executor = StaticStageExecutor::success();
        let mut state = PipelineRunState {
            run_id: "run-1".to_string(),
            status: "idle".to_string(),
            stop_requested: false,
            current_stage_id: None,
            stages: BTreeMap::new(),
            ..PipelineRunState::default()
        };

        let report = service
            .run_range(&mut state, "8", "010", &executor)
            .unwrap();
        assert_eq!(report.final_status, "success");
        assert_eq!(report.executed_stage_ids, vec!["08", "09", "10"]);
        assert_eq!(state.stages["10"].status, StageStatus::Success);
    }

    #[test]
    fn invalid_range_does_not_mutate_state_or_call_the_executor() {
        let service = PipelineService::new(sample_registry()).unwrap();
        let executor = CountingExecutor::default();
        let mut state = PipelineRunState {
            run_id: String::new(),
            status: "idle".to_string(),
            stop_requested: false,
            current_stage_id: None,
            stages: BTreeMap::new(),
            ..PipelineRunState::default()
        };
        let original_state = state.clone();

        assert!(
            service
                .run_range(&mut state, "10", "08", &executor)
                .is_err()
        );
        assert_eq!(state, original_state);
        assert_eq!(executor.calls.get(), 0);

        assert!(
            service
                .run_range(&mut state, "invalid", "10", &executor)
                .is_err()
        );
        assert_eq!(state, original_state);
        assert_eq!(executor.calls.get(), 0);
    }

    #[test]
    fn pipeline_stop_signal_prevents_next_stage_execution() {
        let service = PipelineService::new(sample_registry()).unwrap();
        let executor = StaticStageExecutor::success();
        let mut state = PipelineRunState {
            run_id: "run-stop".to_string(),
            status: "idle".to_string(),
            stop_requested: true,
            current_stage_id: None,
            stages: BTreeMap::new(),
            ..PipelineRunState::default()
        };

        let report = service
            .run_range(&mut state, "08", "10", &executor)
            .unwrap();
        assert_eq!(report.final_status, "stopped");
        assert!(report.executed_stage_ids.is_empty());
        assert_eq!(state.stages["08"].status, StageStatus::Stopped);
    }

    #[test]
    fn stop_observed_after_the_final_stage_is_not_reported_as_uninterrupted_success() {
        let service = PipelineService::new(sample_registry()).unwrap();
        let executor = StopAtBoundaryExecutor(Cell::new(false));
        let mut state = PipelineRunState {
            run_id: "run-final-boundary-stop".to_string(),
            status: "idle".to_string(),
            ..PipelineRunState::default()
        };

        let report = service
            .run_range(&mut state, "08", "08", &executor)
            .unwrap();

        assert_eq!(report.final_status, "stopped");
        assert!(state.stop_requested);
        assert_eq!(state.stages["08"].status, StageStatus::Success);
    }

    #[test]
    fn pipeline_waiting_confirmation_halts_on_human_gate() {
        let service = PipelineService::new(sample_registry()).unwrap();
        let executor = StaticStageExecutor::with_statuses(BTreeMap::from([(
            "07".to_string(),
            StageStatus::WaitingConfirmation,
        )]));
        let mut state = PipelineRunState {
            run_id: "run-wait".to_string(),
            status: "idle".to_string(),
            stop_requested: false,
            current_stage_id: None,
            stages: BTreeMap::new(),
            ..PipelineRunState::default()
        };

        let report = service
            .run_range(&mut state, "07", "10", &executor)
            .unwrap();
        assert_eq!(report.final_status, "waiting_confirmation");
        assert_eq!(report.executed_stage_ids, vec!["07"]);
        assert_eq!(state.current_stage_id.as_deref(), Some("07"));
    }

    #[test]
    fn pipeline_blocks_when_prior_dependency_failed() {
        let service = PipelineService::new(sample_registry()).unwrap();
        let executor = StaticStageExecutor::success();
        let mut state = PipelineRunState {
            run_id: "run-block".to_string(),
            status: "idle".to_string(),
            stop_requested: false,
            current_stage_id: None,
            stages: BTreeMap::from([(
                "08".to_string(),
                PipelineStageRuntime {
                    stage_id: "08".to_string(),
                    status: StageStatus::Failed,
                    started_at: String::new(),
                    completed_at: String::new(),
                    result: None,
                },
            )]),
            ..PipelineRunState::default()
        };

        let report = service
            .run_range(&mut state, "10", "10", &executor)
            .unwrap();
        assert_eq!(report.final_status, "blocked");
        assert!(report.executed_stage_ids.is_empty());
        assert_eq!(state.stages["10"].status, StageStatus::Blocked);
    }

    #[test]
    fn pipeline_rejects_unknown_dependencies_and_cycles() {
        let unknown = PipelineRegistry {
            stages: vec![StageSpec {
                stage_id: "01".to_string(),
                requires: vec!["missing".to_string()],
                ..stage("01", vec![])
            }],
        };
        assert!(PipelineService::new(unknown).is_err());

        let cyclic = PipelineRegistry {
            stages: vec![stage("a", vec!["b"]), stage("b", vec!["a"])],
        };
        let service = PipelineService::new(cyclic).unwrap();
        assert!(service.topological_order().is_err());
    }

    #[derive(Debug, Clone)]
    struct StaticStageExecutor {
        statuses: BTreeMap<String, StageStatus>,
    }

    struct StopAtBoundaryExecutor(Cell<bool>);

    impl StageExecutor for StopAtBoundaryExecutor {
        fn execute(&self, _spec: &StageSpec, _context: &StageContextModel) -> PipelineStageResult {
            self.0.set(true);
            PipelineStageResult {
                status: StageStatus::Success,
                outputs: BTreeMap::new(),
                errors: Vec::new(),
                warnings: Vec::new(),
                message: "completed at stop boundary".to_string(),
            }
        }

        fn stop_requested(&self) -> bool {
            self.0.get()
        }
    }

    impl StaticStageExecutor {
        fn success() -> Self {
            Self {
                statuses: BTreeMap::new(),
            }
        }

        fn with_statuses(statuses: BTreeMap<String, StageStatus>) -> Self {
            Self { statuses }
        }
    }

    impl StageExecutor for StaticStageExecutor {
        fn execute(&self, spec: &StageSpec, context: &StageContextModel) -> PipelineStageResult {
            assert_eq!(context.stage_id, spec.stage_id);
            PipelineStageResult {
                status: self
                    .statuses
                    .get(&spec.stage_id)
                    .cloned()
                    .unwrap_or(StageStatus::Success),
                outputs: BTreeMap::from([(
                    "stage".to_string(),
                    json!({"id": spec.stage_id, "artifactDir": context.artifact_dir}),
                )]),
                errors: Vec::new(),
                warnings: Vec::new(),
                message: "static executor".to_string(),
            }
        }
    }

    #[derive(Debug, Default)]
    struct CountingExecutor {
        calls: Cell<usize>,
    }

    impl StageExecutor for CountingExecutor {
        fn execute(&self, _spec: &StageSpec, _context: &StageContextModel) -> PipelineStageResult {
            self.calls.set(self.calls.get() + 1);
            PipelineStageResult {
                status: StageStatus::Success,
                outputs: BTreeMap::new(),
                errors: Vec::new(),
                warnings: Vec::new(),
                message: "counting executor".to_string(),
            }
        }
    }

    fn sample_registry() -> PipelineRegistry {
        PipelineRegistry {
            stages: vec![
                stage("03", vec![]),
                stage("04", vec!["03"]),
                stage("05", vec!["03"]),
                stage("06", vec!["04"]),
                stage("07", vec!["06"]),
                stage("08", vec!["05"]),
                stage("09", vec!["07"]),
                stage("10", vec!["08", "09"]),
                stage("11", vec!["10"]),
                stage("12", vec!["10"]),
                stage("13", vec!["11", "12"]),
                stage("14", vec!["13"]),
            ],
        }
    }

    fn stage(stage_id: &str, requires: Vec<&str>) -> StageSpec {
        StageSpec {
            stage_id: stage_id.to_string(),
            kind: if stage_id == "07" {
                StageKind::HumanGate
            } else {
                StageKind::Development
            },
            number: stage_id.parse::<u32>().ok(),
            slug: format!("stage_{stage_id}"),
            title: format!("Stage {stage_id}"),
            requires: requires.into_iter().map(str::to_string).collect(),
            source_groups: Vec::new(),
            plugin_ref: format!("pipeline.step{stage_id}"),
            metadata: BTreeMap::new(),
        }
    }

    fn numbered_stage(stage_id: &str, number: u32, requires: Vec<&str>) -> StageSpec {
        StageSpec {
            number: Some(number),
            ..stage(stage_id, requires)
        }
    }

    fn assert_before(order: &[String], left: &str, right: &str) {
        let left_index = order.iter().position(|stage_id| stage_id == left).unwrap();
        let right_index = order.iter().position(|stage_id| stage_id == right).unwrap();
        assert!(left_index < right_index, "{left} should be before {right}");
    }
}
