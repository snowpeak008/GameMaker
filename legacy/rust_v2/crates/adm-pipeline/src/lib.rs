#![forbid(unsafe_code)]

use adm_foundation::{AdmError, AdmResult, ArtifactId, ContentHash, RunId, StageId, UtcTimestamp};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineStage {
    pub id: StageId,
    pub name: String,
    pub dependencies: Vec<StageId>,
    pub ai_intervention_allowed: bool,
}

impl PipelineStage {
    pub fn new(id: StageId, name: impl Into<String>) -> AdmResult<Self> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(AdmError::invalid_input(
                "pipeline stage name cannot be empty",
            ));
        }
        Ok(Self {
            id,
            name,
            dependencies: Vec::new(),
            ai_intervention_allowed: false,
        })
    }

    pub fn depends_on(mut self, dependency: StageId) -> Self {
        self.dependencies.push(dependency);
        self
    }

    pub fn allow_ai_intervention(mut self) -> Self {
        self.ai_intervention_allowed = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageRunStatus {
    Succeeded,
    Failed,
    NeedsAiIntervention,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateDecision {
    Pass,
    Warn(String),
    Block(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageGate {
    pub stage_id: StageId,
    pub decision: GateDecision,
}

impl StageGate {
    pub fn pass(stage_id: StageId) -> Self {
        Self {
            stage_id,
            decision: GateDecision::Pass,
        }
    }

    pub fn block(stage_id: StageId, reason: impl Into<String>) -> Self {
        Self {
            stage_id,
            decision: GateDecision::Block(reason.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageRunResult {
    pub stage_id: StageId,
    pub status: StageRunStatus,
    pub artifacts: Vec<ArtifactId>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRecord {
    pub artifact_id: ArtifactId,
    pub stage_id: StageId,
    pub relative_path: PathBuf,
    pub content_hash: ContentHash,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtifactRegistry {
    records: Vec<ArtifactRecord>,
}

impl ArtifactRegistry {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn register(&mut self, record: ArtifactRecord) -> AdmResult<()> {
        if self
            .records
            .iter()
            .any(|existing| existing.artifact_id == record.artifact_id)
        {
            return Err(AdmError::conflict(format!(
                "artifact {} is already registered",
                record.artifact_id
            )));
        }
        self.records.push(record);
        Ok(())
    }

    pub fn by_stage(&self, stage_id: &StageId) -> Vec<&ArtifactRecord> {
        self.records
            .iter()
            .filter(|record| &record.stage_id == stage_id)
            .collect()
    }

    pub fn records(&self) -> &[ArtifactRecord] {
        &self.records
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Artifact Registry\n");
        for record in &self.records {
            document.push_str(&format!(
                "artifact_id={};stage_id={};path={};hash={}\n",
                record.artifact_id,
                record.stage_id,
                record.relative_path.display(),
                record.content_hash
            ));
        }
        document
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineRunReport {
    pub results: Vec<StageRunResult>,
}

impl PipelineRunReport {
    pub fn status(&self) -> StageRunStatus {
        self.results
            .last()
            .map(|result| result.status.clone())
            .unwrap_or(StageRunStatus::Succeeded)
    }

    pub fn from_report_text(text: &str) -> AdmResult<Self> {
        let mut results = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            let Some(entry) = trimmed.strip_prefix("- ") else {
                continue;
            };
            let Some(after_stage_prefix) = entry.strip_prefix("stage_id=") else {
                continue;
            };
            let Some((stage_id, after_status_prefix)) = after_stage_prefix.split_once("; status=")
            else {
                return Err(AdmError::validation(
                    "pipeline report stage entry missing status",
                ));
            };
            let Some((status, message)) = after_status_prefix.split_once("; message=") else {
                return Err(AdmError::validation(
                    "pipeline report stage entry missing message",
                ));
            };
            let status = match status {
                "Succeeded" => StageRunStatus::Succeeded,
                "Failed" => StageRunStatus::Failed,
                "NeedsAiIntervention" => StageRunStatus::NeedsAiIntervention,
                _ => {
                    return Err(AdmError::validation(format!(
                        "unknown pipeline report stage status: {status}"
                    )));
                }
            };
            results.push(StageRunResult {
                stage_id: StageId::new(stage_id)?,
                status,
                artifacts: Vec::new(),
                message: message.to_string(),
            });
        }
        Ok(Self { results })
    }

    pub fn last_unsuccessful_stage_id(&self) -> Option<&StageId> {
        self.results
            .iter()
            .rev()
            .find(|result| result.status != StageRunStatus::Succeeded)
            .map(|result| &result.stage_id)
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Pipeline Run Report\n");
        document.push_str(&format!("status={:?}\n", self.status()));
        for result in &self.results {
            document.push_str(&format!(
                "- stage_id={}; status={:?}; message={}\n",
                result.stage_id, result.status, result.message
            ));
        }
        document
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineRunLifecycleStatus {
    Created,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineRunState {
    pub run_id: RunId,
    pub status: PipelineRunLifecycleStatus,
    pub completed_stages: Vec<StageId>,
    pub active_stage: Option<StageId>,
    pub last_message: String,
    pub updated_at: UtcTimestamp,
}

impl PipelineRunState {
    pub fn new(run_id: RunId) -> Self {
        Self {
            run_id,
            status: PipelineRunLifecycleStatus::Created,
            completed_stages: Vec::new(),
            active_stage: None,
            last_message: String::new(),
            updated_at: UtcTimestamp::now(),
        }
    }

    pub fn start_stage(&mut self, stage_id: StageId) {
        self.status = PipelineRunLifecycleStatus::Running;
        self.active_stage = Some(stage_id);
        self.updated_at = UtcTimestamp::now();
    }

    pub fn complete_stage(&mut self, stage_id: StageId, message: impl Into<String>) {
        if !self.completed_stages.contains(&stage_id) {
            self.completed_stages.push(stage_id);
        }
        self.active_stage = None;
        self.last_message = message.into();
        self.updated_at = UtcTimestamp::now();
    }

    pub fn finish(&mut self) {
        self.status = PipelineRunLifecycleStatus::Succeeded;
        self.active_stage = None;
        self.updated_at = UtcTimestamp::now();
    }

    pub fn fail(&mut self, message: impl Into<String>) {
        self.status = PipelineRunLifecycleStatus::Failed;
        self.active_stage = None;
        self.last_message = message.into();
        self.updated_at = UtcTimestamp::now();
    }

    pub fn cancel(&mut self, message: impl Into<String>) {
        self.status = PipelineRunLifecycleStatus::Cancelled;
        self.active_stage = None;
        self.last_message = message.into();
        self.updated_at = UtcTimestamp::now();
    }

    pub fn is_stage_completed(&self, stage_id: &StageId) -> bool {
        self.completed_stages.contains(stage_id)
    }

    pub fn render(&self) -> String {
        let completed = self
            .completed_stages
            .iter()
            .map(StageId::as_str)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "run_id={}\nstatus={:?}\nactive_stage={}\ncompleted_stages={}\nlast_message={}\nupdated_at={}\n",
            self.run_id,
            self.status,
            self.active_stage
                .as_ref()
                .map(StageId::as_str)
                .unwrap_or(""),
            completed,
            self.last_message,
            self.updated_at.as_millis()
        )
    }

    pub fn from_state_text(text: &str) -> AdmResult<Self> {
        let mut run_id = None;
        let mut status = None;
        let mut active_stage = None;
        let mut completed_stages = Vec::new();
        let mut last_message = String::new();
        let mut updated_at = None;

        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "run_id" => run_id = Some(RunId::new(value)?),
                "status" => {
                    status = Some(match value {
                        "Created" => PipelineRunLifecycleStatus::Created,
                        "Running" => PipelineRunLifecycleStatus::Running,
                        "Succeeded" => PipelineRunLifecycleStatus::Succeeded,
                        "Failed" => PipelineRunLifecycleStatus::Failed,
                        "Cancelled" => PipelineRunLifecycleStatus::Cancelled,
                        _ => return Err(AdmError::validation("unknown pipeline run status")),
                    });
                }
                "active_stage" if !value.is_empty() => active_stage = Some(StageId::new(value)?),
                "completed_stages" if !value.is_empty() => {
                    completed_stages = value
                        .split(',')
                        .map(StageId::new)
                        .collect::<AdmResult<Vec<_>>>()?;
                }
                "last_message" => last_message = value.to_string(),
                "updated_at" => {
                    updated_at = Some(UtcTimestamp::from_millis(value.parse::<u128>().map_err(
                        |error| AdmError::validation(format!("invalid updated_at: {error}")),
                    )?));
                }
                _ => {}
            }
        }

        Ok(Self {
            run_id: run_id.ok_or_else(|| AdmError::validation("run state missing run_id"))?,
            status: status.ok_or_else(|| AdmError::validation("run state missing status"))?,
            completed_stages,
            active_stage,
            last_message,
            updated_at: updated_at
                .ok_or_else(|| AdmError::validation("run state missing updated_at"))?,
        })
    }

    pub fn validate_for_graph(&self, graph: &PipelineGraph) -> AdmResult<()> {
        let known_stage_ids = graph
            .stages()
            .iter()
            .map(|stage| stage.id.clone())
            .collect::<HashSet<_>>();
        for completed_stage in &self.completed_stages {
            if !known_stage_ids.contains(completed_stage) {
                return Err(AdmError::validation(format!(
                    "run state contains unknown completed stage: {completed_stage}"
                )));
            }
        }
        if let Some(active_stage) = &self.active_stage
            && !known_stage_ids.contains(active_stage)
        {
            return Err(AdmError::validation(format!(
                "run state contains unknown active stage: {active_stage}"
            )));
        }
        for stage in graph.stages() {
            if self.is_stage_completed(&stage.id)
                && !stage
                    .dependencies
                    .iter()
                    .all(|dependency| self.is_stage_completed(dependency))
            {
                return Err(AdmError::validation(format!(
                    "completed stage {} is missing completed dependencies",
                    stage.id
                )));
            }
        }
        Ok(())
    }

    pub fn rewind_to_stage(
        &mut self,
        graph: &PipelineGraph,
        stage_id: &StageId,
    ) -> AdmResult<Vec<StageId>> {
        let downstream_stages = graph.downstream_stage_ids(stage_id)?;
        let downstream_set = downstream_stages.iter().cloned().collect::<HashSet<_>>();
        self.completed_stages
            .retain(|completed| !downstream_set.contains(completed));
        self.active_stage = None;
        self.status = PipelineRunLifecycleStatus::Created;
        self.last_message = format!("rewound to stage {stage_id}");
        self.updated_at = UtcTimestamp::now();
        self.validate_for_graph(graph)?;
        Ok(downstream_stages)
    }
}

pub trait StageExecutor {
    fn execute(&mut self, stage: &PipelineStage) -> AdmResult<StageRunResult>;
}

pub trait GateEvaluator {
    fn evaluate(&mut self, stage: &PipelineStage) -> AdmResult<StageGate>;
}

#[derive(Debug, Default)]
pub struct AllowAllGateEvaluator;

impl GateEvaluator for AllowAllGateEvaluator {
    fn evaluate(&mut self, stage: &PipelineStage) -> AdmResult<StageGate> {
        Ok(StageGate::pass(stage.id.clone()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineGraph {
    stages: Vec<PipelineStage>,
}

impl PipelineGraph {
    pub fn new(stages: Vec<PipelineStage>) -> AdmResult<Self> {
        let graph = Self { stages };
        graph.validate()?;
        Ok(graph)
    }

    pub fn stages(&self) -> &[PipelineStage] {
        &self.stages
    }

    pub fn execution_order(&self) -> AdmResult<Vec<PipelineStage>> {
        let mut ordered = Vec::new();
        let mut completed: HashSet<StageId> = HashSet::new();

        while ordered.len() < self.stages.len() {
            let mut progressed = false;
            for stage in &self.stages {
                if completed.contains(&stage.id) {
                    continue;
                }
                if stage
                    .dependencies
                    .iter()
                    .all(|dependency| completed.contains(dependency))
                {
                    ordered.push(stage.clone());
                    completed.insert(stage.id.clone());
                    progressed = true;
                }
            }
            if !progressed {
                return Err(AdmError::validation(
                    "pipeline graph contains a dependency cycle",
                ));
            }
        }

        Ok(ordered)
    }

    pub fn downstream_stage_ids(&self, stage_id: &StageId) -> AdmResult<Vec<StageId>> {
        if !self.stages.iter().any(|stage| &stage.id == stage_id) {
            return Err(AdmError::validation(format!(
                "pipeline graph does not contain stage: {stage_id}"
            )));
        }
        let mut downstream = HashSet::<StageId>::new();
        downstream.insert(stage_id.clone());
        let ordered = self.execution_order()?;
        let mut changed = true;
        while changed {
            changed = false;
            for stage in &ordered {
                if downstream.contains(&stage.id) {
                    continue;
                }
                if stage
                    .dependencies
                    .iter()
                    .any(|dependency| downstream.contains(dependency))
                {
                    downstream.insert(stage.id.clone());
                    changed = true;
                }
            }
        }
        Ok(ordered
            .into_iter()
            .filter_map(|stage| downstream.contains(&stage.id).then_some(stage.id))
            .collect())
    }

    fn validate(&self) -> AdmResult<()> {
        let mut ids = HashSet::new();
        for stage in &self.stages {
            if !ids.insert(stage.id.clone()) {
                return Err(AdmError::validation(format!(
                    "duplicate stage id: {}",
                    stage.id
                )));
            }
            if stage.dependencies.contains(&stage.id) {
                return Err(AdmError::validation(format!(
                    "stage {} cannot depend on itself",
                    stage.id
                )));
            }
        }
        for stage in &self.stages {
            for dependency in &stage.dependencies {
                if !ids.contains(dependency) {
                    return Err(AdmError::validation(format!(
                        "stage {} depends on missing stage {}",
                        stage.id, dependency
                    )));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PipelineRunner {
    graph: PipelineGraph,
}

impl PipelineRunner {
    pub fn new(graph: PipelineGraph) -> Self {
        Self { graph }
    }

    pub fn run_serial<E: StageExecutor>(&self, executor: &mut E) -> AdmResult<Vec<StageRunResult>> {
        let mut results = Vec::new();
        for stage in self.graph.execution_order()? {
            let result = executor.execute(&stage)?;
            let should_stop = result.status != StageRunStatus::Succeeded;
            results.push(result);
            if should_stop {
                break;
            }
        }
        Ok(results)
    }

    pub fn run_serial_report<E: StageExecutor>(
        &self,
        executor: &mut E,
    ) -> AdmResult<PipelineRunReport> {
        Ok(PipelineRunReport {
            results: self.run_serial(executor)?,
        })
    }

    pub fn run_serial_with_state<E: StageExecutor>(
        &self,
        executor: &mut E,
        state: &mut PipelineRunState,
    ) -> AdmResult<PipelineRunReport> {
        state.validate_for_graph(&self.graph)?;
        let mut results = Vec::new();
        for stage in self.graph.execution_order()? {
            if state.is_stage_completed(&stage.id) {
                continue;
            }
            state.start_stage(stage.id.clone());
            let result = executor.execute(&stage)?;
            let should_stop = result.status != StageRunStatus::Succeeded;
            if result.status == StageRunStatus::Succeeded {
                state.complete_stage(stage.id.clone(), result.message.clone());
            } else {
                state.fail(result.message.clone());
            }
            results.push(result);
            if should_stop {
                return Ok(PipelineRunReport { results });
            }
        }
        state.finish();
        Ok(PipelineRunReport { results })
    }

    pub fn run_serial_with_gates<E: StageExecutor, G: GateEvaluator>(
        &self,
        executor: &mut E,
        gates: &mut G,
    ) -> AdmResult<PipelineRunReport> {
        let mut results = Vec::new();
        for stage in self.graph.execution_order()? {
            match gates.evaluate(&stage)?.decision {
                GateDecision::Pass | GateDecision::Warn(_) => {
                    let result = executor.execute(&stage)?;
                    let should_stop = result.status != StageRunStatus::Succeeded;
                    results.push(result);
                    if should_stop {
                        break;
                    }
                }
                GateDecision::Block(reason) => {
                    results.push(StageRunResult {
                        stage_id: stage.id,
                        status: StageRunStatus::Failed,
                        artifacts: Vec::new(),
                        message: format!("blocked by gate: {reason}"),
                    });
                    break;
                }
            }
        }
        Ok(PipelineRunReport { results })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingExecutor {
        seen: Vec<StageId>,
    }

    impl StageExecutor for RecordingExecutor {
        fn execute(&mut self, stage: &PipelineStage) -> AdmResult<StageRunResult> {
            self.seen.push(stage.id.clone());
            Ok(StageRunResult {
                stage_id: stage.id.clone(),
                status: StageRunStatus::Succeeded,
                artifacts: Vec::new(),
                message: "ok".to_string(),
            })
        }
    }

    #[test]
    fn pipeline_runs_in_dependency_order() {
        let first = StageId::new("design").unwrap();
        let second = StageId::new("review").unwrap();
        let graph = PipelineGraph::new(vec![
            PipelineStage::new(second.clone(), "Review")
                .unwrap()
                .depends_on(first.clone()),
            PipelineStage::new(first.clone(), "Design").unwrap(),
        ])
        .unwrap();
        let mut executor = RecordingExecutor::default();
        PipelineRunner::new(graph)
            .run_serial(&mut executor)
            .unwrap();
        assert_eq!(executor.seen, vec![first, second]);
    }

    #[test]
    fn pipeline_rejects_missing_dependency() {
        let graph = PipelineGraph::new(vec![
            PipelineStage::new(StageId::new("review").unwrap(), "Review")
                .unwrap()
                .depends_on(StageId::new("missing").unwrap()),
        ]);
        assert!(graph.is_err());
    }

    #[test]
    fn pipeline_gate_can_block_stage() {
        struct BlockReview;

        impl GateEvaluator for BlockReview {
            fn evaluate(&mut self, stage: &PipelineStage) -> AdmResult<StageGate> {
                if stage.id.as_str() == "review" {
                    Ok(StageGate::block(stage.id.clone(), "quality too low"))
                } else {
                    Ok(StageGate::pass(stage.id.clone()))
                }
            }
        }

        let design = StageId::new("design").unwrap();
        let review = StageId::new("review").unwrap();
        let graph = PipelineGraph::new(vec![
            PipelineStage::new(design, "Design").unwrap(),
            PipelineStage::new(review.clone(), "Review")
                .unwrap()
                .depends_on(StageId::new("design").unwrap()),
        ])
        .unwrap();
        let mut executor = RecordingExecutor::default();
        let report = PipelineRunner::new(graph)
            .run_serial_with_gates(&mut executor, &mut BlockReview)
            .unwrap();

        assert_eq!(report.status(), StageRunStatus::Failed);
        assert_eq!(report.results.last().unwrap().stage_id, review);
        assert!(report.results.last().unwrap().message.contains("blocked"));
    }

    #[test]
    fn artifact_registry_rejects_duplicate_artifacts() {
        let artifact_id = ArtifactId::new("design_doc").unwrap();
        let mut registry = ArtifactRegistry::new();
        let record = ArtifactRecord {
            artifact_id: artifact_id.clone(),
            stage_id: StageId::new("design").unwrap(),
            relative_path: PathBuf::from("design/project.adm"),
            content_hash: ContentHash::from_bytes(b"design"),
        };
        registry.register(record.clone()).unwrap();
        assert!(registry.register(record).is_err());
        assert_eq!(registry.by_stage(&StageId::new("design").unwrap()).len(), 1);
    }

    #[test]
    fn pipeline_run_state_round_trips() {
        let mut state = PipelineRunState::new(RunId::new("run_demo").unwrap());
        state.start_stage(StageId::new("design").unwrap());
        state.complete_stage(StageId::new("design").unwrap(), "done");
        state.finish();
        let parsed = PipelineRunState::from_state_text(&state.render()).unwrap();
        assert_eq!(parsed.status, PipelineRunLifecycleStatus::Succeeded);
        assert_eq!(
            parsed.completed_stages,
            vec![StageId::new("design").unwrap()]
        );
    }

    #[test]
    fn pipeline_run_report_parses_last_unsuccessful_stage() {
        let report = PipelineRunReport {
            results: vec![
                StageRunResult {
                    stage_id: StageId::new("design").unwrap(),
                    status: StageRunStatus::Succeeded,
                    artifacts: vec![ArtifactId::new("design_doc").unwrap()],
                    message: "ok".to_string(),
                },
                StageRunResult {
                    stage_id: StageId::new("review").unwrap(),
                    status: StageRunStatus::Failed,
                    artifacts: Vec::new(),
                    message: "quality gate failed".to_string(),
                },
            ],
        };

        let parsed = PipelineRunReport::from_report_text(&report.render()).unwrap();

        assert_eq!(
            parsed.last_unsuccessful_stage_id(),
            Some(&StageId::new("review").unwrap())
        );
        assert_eq!(parsed.results[1].message, "quality gate failed");
    }

    #[test]
    fn pipeline_resume_skips_completed_stages() {
        let design = StageId::new("design").unwrap();
        let review = StageId::new("review").unwrap();
        let graph = PipelineGraph::new(vec![
            PipelineStage::new(design.clone(), "Design").unwrap(),
            PipelineStage::new(review.clone(), "Review")
                .unwrap()
                .depends_on(design.clone()),
        ])
        .unwrap();
        let mut state = PipelineRunState::new(RunId::new("run_resume").unwrap());
        state.complete_stage(design.clone(), "already done");
        let mut executor = RecordingExecutor::default();

        let report = PipelineRunner::new(graph)
            .run_serial_with_state(&mut executor, &mut state)
            .unwrap();

        assert_eq!(executor.seen, vec![review.clone()]);
        assert_eq!(report.results.len(), 1);
        assert_eq!(state.status, PipelineRunLifecycleStatus::Succeeded);
        assert_eq!(state.completed_stages, vec![design, review]);
    }

    #[test]
    fn pipeline_rewind_removes_selected_stage_and_transitive_downstream_only() {
        let design = StageId::new("design").unwrap();
        let development = StageId::new("development").unwrap();
        let assets = StageId::new("assets").unwrap();
        let sdk = StageId::new("sdk").unwrap();
        let packaging = StageId::new("packaging").unwrap();
        let graph = PipelineGraph::new(vec![
            PipelineStage::new(design.clone(), "Design").unwrap(),
            PipelineStage::new(development.clone(), "Development")
                .unwrap()
                .depends_on(design.clone()),
            PipelineStage::new(assets.clone(), "Assets")
                .unwrap()
                .depends_on(design.clone()),
            PipelineStage::new(sdk.clone(), "SDK")
                .unwrap()
                .depends_on(development.clone()),
            PipelineStage::new(packaging.clone(), "Packaging")
                .unwrap()
                .depends_on(development.clone())
                .depends_on(assets.clone())
                .depends_on(sdk.clone()),
        ])
        .unwrap();
        let mut state = PipelineRunState::new(RunId::new("run_rewind").unwrap());
        for stage in [
            design.clone(),
            development.clone(),
            assets.clone(),
            sdk.clone(),
            packaging.clone(),
        ] {
            state.complete_stage(stage, "done");
        }
        state.finish();

        let rewound = state.rewind_to_stage(&graph, &development).unwrap();

        assert_eq!(rewound, vec![development, sdk, packaging]);
        assert_eq!(state.status, PipelineRunLifecycleStatus::Created);
        assert_eq!(state.completed_stages, vec![design, assets]);
        assert!(state.last_message.contains("development"));
    }

    #[test]
    fn pipeline_can_resume_after_failed_stage() {
        struct ReviewExecutor {
            seen: Vec<StageId>,
            fail_review: bool,
        }

        impl StageExecutor for ReviewExecutor {
            fn execute(&mut self, stage: &PipelineStage) -> AdmResult<StageRunResult> {
                self.seen.push(stage.id.clone());
                if self.fail_review && stage.id.as_str() == "review" {
                    return Ok(StageRunResult {
                        stage_id: stage.id.clone(),
                        status: StageRunStatus::Failed,
                        artifacts: Vec::new(),
                        message: "review failed".to_string(),
                    });
                }
                Ok(StageRunResult {
                    stage_id: stage.id.clone(),
                    status: StageRunStatus::Succeeded,
                    artifacts: Vec::new(),
                    message: "ok".to_string(),
                })
            }
        }

        let design = StageId::new("design").unwrap();
        let review = StageId::new("review").unwrap();
        let graph = PipelineGraph::new(vec![
            PipelineStage::new(design.clone(), "Design").unwrap(),
            PipelineStage::new(review.clone(), "Review")
                .unwrap()
                .depends_on(design.clone()),
        ])
        .unwrap();
        let runner = PipelineRunner::new(graph);
        let mut state = PipelineRunState::new(RunId::new("run_failed_resume").unwrap());
        let mut failing_executor = ReviewExecutor {
            seen: Vec::new(),
            fail_review: true,
        };

        let failed_report = runner
            .run_serial_with_state(&mut failing_executor, &mut state)
            .unwrap();

        assert_eq!(failed_report.status(), StageRunStatus::Failed);
        assert_eq!(state.status, PipelineRunLifecycleStatus::Failed);
        assert_eq!(state.completed_stages, vec![design.clone()]);

        let mut resumed_executor = ReviewExecutor {
            seen: Vec::new(),
            fail_review: false,
        };
        let resumed_report = runner
            .run_serial_with_state(&mut resumed_executor, &mut state)
            .unwrap();

        assert_eq!(resumed_executor.seen, vec![review.clone()]);
        assert_eq!(resumed_report.status(), StageRunStatus::Succeeded);
        assert_eq!(state.status, PipelineRunLifecycleStatus::Succeeded);
        assert_eq!(state.completed_stages, vec![design, review]);
    }

    #[test]
    fn pipeline_rejects_resume_state_with_missing_completed_dependencies() {
        let design = StageId::new("design").unwrap();
        let review = StageId::new("review").unwrap();
        let graph = PipelineGraph::new(vec![
            PipelineStage::new(design, "Design").unwrap(),
            PipelineStage::new(review.clone(), "Review")
                .unwrap()
                .depends_on(StageId::new("design").unwrap()),
        ])
        .unwrap();
        let mut state = PipelineRunState::new(RunId::new("run_bad_state").unwrap());
        state.complete_stage(review, "bad state");
        let mut executor = RecordingExecutor::default();
        let result = PipelineRunner::new(graph).run_serial_with_state(&mut executor, &mut state);

        assert!(result.is_err());
        assert!(executor.seen.is_empty());
    }
}
