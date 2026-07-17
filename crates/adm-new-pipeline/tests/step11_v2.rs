use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

use adm_new_change_kernel::{
    ChangeEvidence, ChangeFailureCategory, ChangeOutcome, EvidenceStatus, SideEffectState,
    WORKSPACE_CHANGE_SET_SCHEMA_VERSION, WorkspaceRelativePath, WorkspaceTransactionResult,
};
use adm_new_foundation::{AdmError, AdmResult, new_stable_id, sha256_hex};
use adm_new_game_spec::{GameSpec, parse_game_spec};
use adm_new_pipeline::stages::step07_v2::{
    compile_step07_art_direction, confirm_style_anchors_attended,
};
use adm_new_pipeline::stages::step08_10_v2::{
    TrustedDevelopmentTask, TrustedTaskGraph, compile_step08_10,
};
use adm_new_pipeline::stages::step11_v2::{
    Step11ExecutionBudget, Step11ExecutionEngine, Step11ExecutionState, Step11ExecutionStatus,
    Step11FailureKind, Step11StopToken, Step11TaskStatus, WorkspaceTaskAgent,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ScriptedOutcome {
    Commit,
    CompileFail,
    ScopeViolation,
    TrustedTestTamper,
    AgentError(&'static str),
}

#[derive(Debug, Clone, Default)]
struct ScriptedAgent {
    outcomes: Arc<Mutex<BTreeMap<String, VecDeque<ScriptedOutcome>>>>,
}

impl ScriptedAgent {
    fn with(task_id: &str, outcomes: Vec<ScriptedOutcome>) -> Self {
        Self {
            outcomes: Arc::new(Mutex::new(BTreeMap::from([(
                task_id.to_string(),
                outcomes.into(),
            )]))),
        }
    }
}

impl WorkspaceTaskAgent for ScriptedAgent {
    fn execute_task(
        &self,
        task: &TrustedDevelopmentTask,
        attempt: u32,
        _previous_failure: Option<&adm_new_pipeline::stages::step11_v2::Step11FailureEvidence>,
    ) -> AdmResult<WorkspaceTransactionResult> {
        let outcome = self
            .outcomes
            .lock()
            .unwrap()
            .entry(task.task_id.clone())
            .or_default()
            .pop_front()
            .unwrap_or(ScriptedOutcome::Commit);
        match outcome {
            ScriptedOutcome::Commit => Ok(committed_result(task, attempt, false, false)),
            ScriptedOutcome::CompileFail => Ok(rejected_result(
                task,
                attempt,
                ChangeFailureCategory::Compile,
                "compile failed",
            )),
            ScriptedOutcome::ScopeViolation => Ok(committed_result(task, attempt, true, false)),
            ScriptedOutcome::TrustedTestTamper => Ok(committed_result(task, attempt, false, true)),
            ScriptedOutcome::AgentError(message) => Err(AdmError::new(message)),
        }
    }
}

#[derive(Debug, Clone)]
struct StopAfterFirstCommitAgent {
    token: Step11StopToken,
    call_count: Arc<Mutex<u32>>,
}

impl WorkspaceTaskAgent for StopAfterFirstCommitAgent {
    fn execute_task(
        &self,
        task: &TrustedDevelopmentTask,
        attempt: u32,
        previous_failure: Option<&adm_new_pipeline::stages::step11_v2::Step11FailureEvidence>,
    ) -> AdmResult<WorkspaceTransactionResult> {
        let result = ScriptedAgent::default().execute_task(task, attempt, previous_failure)?;
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        if *count == 1 {
            self.token.request_stop();
        }
        Ok(result)
    }
}

#[test]
fn r1c0_task_graph_executes_all_workspace_contracts() {
    let graph = r1_graph();
    let mut state = Step11ExecutionState::new(graph.source_game_spec_hash.clone());
    let engine = Step11ExecutionEngine::new(ScriptedAgent::default(), default_budget());

    let report = engine
        .run(&graph, &mut state, &Step11StopToken::default())
        .unwrap();

    assert_eq!(report.status, Step11ExecutionStatus::Success);
    assert_eq!(report.committed_task_ids.len(), graph.tasks.len());
    assert!(report.correction_queue.is_empty());
    assert!(
        report
            .task_reports
            .iter()
            .all(|task| task.status == Step11TaskStatus::Committed)
    );
}

#[test]
fn scope_violation_trusted_test_tamper_and_stale_baseline_are_rejected() {
    let graph = r1_graph();
    let first_task = graph.tasks[0].task_id.clone();

    let mut scope_state = Step11ExecutionState::new(graph.source_game_spec_hash.clone());
    let scope_report = Step11ExecutionEngine::new(
        ScriptedAgent::with(&first_task, vec![ScriptedOutcome::ScopeViolation]),
        default_budget(),
    )
    .run(&graph, &mut scope_state, &Step11StopToken::default())
    .unwrap();
    assert_eq!(
        scope_report.correction_queue[0].failure_kind,
        Step11FailureKind::ScopeViolation
    );
    assert_eq!(scope_report.correction_queue[0].attempts, 1);

    let mut tamper_state = Step11ExecutionState::new(graph.source_game_spec_hash.clone());
    let tamper_report = Step11ExecutionEngine::new(
        ScriptedAgent::with(&first_task, vec![ScriptedOutcome::TrustedTestTamper]),
        default_budget(),
    )
    .run(&graph, &mut tamper_state, &Step11StopToken::default())
    .unwrap();
    assert_eq!(
        tamper_report.correction_queue[0].failure_kind,
        Step11FailureKind::ScopeViolation
    );
    assert_eq!(tamper_report.correction_queue[0].attempts, 1);

    let mut stale_graph = graph.clone();
    stale_graph.tasks[0].workspace_contract.base_tree_hash = "b".repeat(64);
    let mut stale_state = Step11ExecutionState::new(graph.source_game_spec_hash);
    let stale_report = Step11ExecutionEngine::new(ScriptedAgent::default(), default_budget())
        .run(&stale_graph, &mut stale_state, &Step11StopToken::default())
        .unwrap();
    assert_eq!(
        stale_report.correction_queue[0].failure_kind,
        Step11FailureKind::Conflict
    );
    assert!(stale_report.task_reports[0].attempts.is_empty());
}

#[test]
fn compile_failures_retry_but_scope_violations_do_not() {
    let graph = r1_graph();
    let first_task = graph.tasks[0].task_id.clone();
    let mut compile_state = Step11ExecutionState::new(graph.source_game_spec_hash.clone());
    let compile_report = Step11ExecutionEngine::new(
        ScriptedAgent::with(
            &first_task,
            vec![ScriptedOutcome::CompileFail, ScriptedOutcome::Commit],
        ),
        default_budget(),
    )
    .run(&graph, &mut compile_state, &Step11StopToken::default())
    .unwrap();

    assert_eq!(compile_report.status, Step11ExecutionStatus::Success);
    let retried = compile_report
        .task_reports
        .iter()
        .find(|task| task.task_id == first_task)
        .unwrap();
    assert_eq!(retried.attempts.len(), 2);
    assert_eq!(
        retried.attempts[0].failure.as_ref().unwrap().failure_kind,
        Step11FailureKind::Compile
    );

    let mut scope_state = Step11ExecutionState::new(graph.source_game_spec_hash.clone());
    let scope_report = Step11ExecutionEngine::new(
        ScriptedAgent::with(&first_task, vec![ScriptedOutcome::ScopeViolation]),
        default_budget(),
    )
    .run(&graph, &mut scope_state, &Step11StopToken::default())
    .unwrap();
    let failed = scope_report
        .task_reports
        .iter()
        .find(|task| task.task_id == first_task)
        .unwrap();
    assert_eq!(failed.attempts.len(), 1);
    assert!(!failed.attempts[0].retryable);
}

#[test]
fn unrelated_branch_continues_and_soft_stop_can_resume() {
    let mut graph = r1_graph();
    let first_task = graph.tasks[0].task_id.clone();
    let independent_task = graph.tasks[1].task_id.clone();
    graph.tasks[1].dependencies.clear();
    let mut branch_state = Step11ExecutionState::new(graph.source_game_spec_hash.clone());
    let branch_report = Step11ExecutionEngine::new(
        ScriptedAgent::with(
            &first_task,
            vec![ScriptedOutcome::AgentError("agent failed")],
        ),
        no_retry_budget(),
    )
    .run(&graph, &mut branch_state, &Step11StopToken::default())
    .unwrap();

    assert_eq!(
        branch_report.status,
        Step11ExecutionStatus::CorrectionRequired
    );
    assert!(branch_state.committed_task_ids.contains(&independent_task));
    assert!(
        branch_report
            .correction_queue
            .iter()
            .any(|item| item.task_id == first_task)
    );

    let graph = r1_graph();
    let token = Step11StopToken::default();
    let stop_agent = StopAfterFirstCommitAgent {
        token: token.clone(),
        call_count: Arc::new(Mutex::new(0)),
    };
    let mut resume_state = Step11ExecutionState::new(graph.source_game_spec_hash.clone());
    let stopped = Step11ExecutionEngine::new(stop_agent, default_budget())
        .run(&graph, &mut resume_state, &token)
        .unwrap();
    assert_eq!(stopped.status, Step11ExecutionStatus::Stopped);
    assert_eq!(resume_state.committed_task_ids.len(), 1);

    let resumed = Step11ExecutionEngine::new(ScriptedAgent::default(), default_budget())
        .run(&graph, &mut resume_state, &Step11StopToken::default())
        .unwrap();
    assert_eq!(resumed.status, Step11ExecutionStatus::Success);
    assert_eq!(resume_state.committed_task_ids.len(), graph.tasks.len());
}

fn r1_graph() -> TrustedTaskGraph {
    let root = std::env::temp_dir().join(new_stable_id("step11_v2_r1").unwrap());
    std::fs::create_dir_all(&root).unwrap();
    let step07_dir = root.join("step07");
    compile_step07_art_direction(&r1_fixture(), &step07_dir).unwrap();
    let anchors = confirm_style_anchors_attended(&step07_dir, "tester", "approved", "attended")
        .unwrap()
        .anchors;
    let graph = compile_step08_10(&r1_fixture(), &anchors, &root.join("step08_10"))
        .unwrap()
        .task_graph;
    let _ = std::fs::remove_dir_all(root);
    graph
}

fn r1_fixture() -> GameSpec {
    parse_game_spec(include_str!(
        "../../../testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"
    ))
    .unwrap()
}

fn default_budget() -> Step11ExecutionBudget {
    Step11ExecutionBudget {
        max_workers: 1,
        max_retries: 2,
    }
}

fn no_retry_budget() -> Step11ExecutionBudget {
    Step11ExecutionBudget {
        max_workers: 1,
        max_retries: 0,
    }
}

fn committed_result(
    task: &TrustedDevelopmentTask,
    attempt: u32,
    outside_scope: bool,
    tamper_trusted_test: bool,
) -> WorkspaceTransactionResult {
    let contract = &task.workspace_contract;
    let mut agent_changed_paths = contract.agent_write_paths.clone();
    if outside_scope {
        agent_changed_paths.insert(WorkspaceRelativePath::parse("Assets/Outside.cs").unwrap());
    }
    let mut trusted_hashes = trusted_test_hashes(task);
    if tamper_trusted_test {
        for value in trusted_hashes.values_mut() {
            *value = "c".repeat(64);
        }
    }
    WorkspaceTransactionResult {
        schema_version: WORKSPACE_CHANGE_SET_SCHEMA_VERSION.to_string(),
        change_set_id: contract.change_set_id.clone(),
        contract_sha256: contract.contract_hash().unwrap(),
        base_tree_hash: contract.base_tree_hash.clone(),
        outcome: ChangeOutcome::Committed,
        failure_category: None,
        side_effect_state: SideEffectState::Committed,
        stage: "step11_v2_test_agent".to_string(),
        resulting_tree_hash: Some(result_hash(task, attempt)),
        agent_changed_paths,
        trusted_tool_changed_paths: BTreeSet::new(),
        build_output_changed_paths: BTreeSet::new(),
        trusted_test_hashes: trusted_hashes,
        evidence: vec![ChangeEvidence::from_bytes(
            "step11_result",
            "step11",
            EvidenceStatus::Passed,
            format!("{}:{attempt}", task.task_id).as_bytes(),
        )],
    }
}

fn rejected_result(
    task: &TrustedDevelopmentTask,
    attempt: u32,
    category: ChangeFailureCategory,
    message: &str,
) -> WorkspaceTransactionResult {
    let contract = &task.workspace_contract;
    WorkspaceTransactionResult {
        schema_version: WORKSPACE_CHANGE_SET_SCHEMA_VERSION.to_string(),
        change_set_id: contract.change_set_id.clone(),
        contract_sha256: contract.contract_hash().unwrap(),
        base_tree_hash: contract.base_tree_hash.clone(),
        outcome: ChangeOutcome::Rejected,
        failure_category: Some(category),
        side_effect_state: SideEffectState::None,
        stage: "step11_v2_test_agent".to_string(),
        resulting_tree_hash: None,
        agent_changed_paths: BTreeSet::new(),
        trusted_tool_changed_paths: BTreeSet::new(),
        build_output_changed_paths: BTreeSet::new(),
        trusted_test_hashes: trusted_test_hashes(task),
        evidence: vec![ChangeEvidence::from_bytes(
            "step11_result",
            "step11",
            EvidenceStatus::Failed,
            format!("{}:{attempt}:{message}", task.task_id).as_bytes(),
        )],
    }
}

fn trusted_test_hashes(task: &TrustedDevelopmentTask) -> BTreeMap<String, String> {
    task.workspace_contract
        .trusted_tests
        .iter()
        .map(|test| (test.test_id.clone(), test.baseline_sha256.clone()))
        .collect()
}

fn result_hash(task: &TrustedDevelopmentTask, attempt: u32) -> String {
    sha256_hex(format!("{}:{attempt}", task.task_id).as_bytes())
}
