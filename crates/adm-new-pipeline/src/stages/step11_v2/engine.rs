use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use adm_new_change_kernel::WorkspaceTransactionResult;
use adm_new_foundation::{AdmError, AdmResult};

use crate::stages::step08_10_v2::{TrustedDevelopmentTask, TrustedTaskGraph};
use crate::stages::step11_v2::support::{
    agent_error_failure, append_blocked_dependency_reports, correction_report, dependents_by_task,
    execution_report, handle_agent_result, queue_failure, queue_without_agent,
};
use crate::stages::step11_v2::types::{
    Step11ExecutionBudget, Step11ExecutionReport, Step11ExecutionState, Step11FailureEvidence,
    Step11FailureKind, Step11StopToken, Step11TaskReport, Step11TaskStatus,
};

pub trait WorkspaceTaskAgent: Send + Sync + std::fmt::Debug {
    fn execute_task(
        &self,
        task: &TrustedDevelopmentTask,
        attempt: u32,
        previous_failure: Option<&Step11FailureEvidence>,
    ) -> AdmResult<WorkspaceTransactionResult>;
}

impl<T> WorkspaceTaskAgent for Arc<T>
where
    T: WorkspaceTaskAgent + ?Sized,
{
    fn execute_task(
        &self,
        task: &TrustedDevelopmentTask,
        attempt: u32,
        previous_failure: Option<&Step11FailureEvidence>,
    ) -> AdmResult<WorkspaceTransactionResult> {
        self.as_ref().execute_task(task, attempt, previous_failure)
    }
}

pub struct Step11ExecutionEngine<A> {
    agent: A,
    budget: Step11ExecutionBudget,
}

impl<A> Step11ExecutionEngine<A>
where
    A: WorkspaceTaskAgent,
{
    pub fn new(agent: A, budget: Step11ExecutionBudget) -> Self {
        Self { agent, budget }
    }

    pub fn run(
        &self,
        graph: &TrustedTaskGraph,
        state: &mut Step11ExecutionState,
        stop_token: &Step11StopToken,
    ) -> AdmResult<Step11ExecutionReport> {
        if !graph.validation.acyclic || graph.validation.invalid_contract_count > 0 {
            return Err(AdmError::new("Step11 cannot execute an invalid task graph"));
        }
        let starting_tree_hash = state.current_tree_hash.clone();
        state.stopped = false;
        let tasks = graph
            .tasks
            .iter()
            .map(|task| (task.task_id.clone(), task))
            .collect::<BTreeMap<_, _>>();
        let dependents = dependents_by_task(graph);
        let mut reports = Vec::new();

        loop {
            if stop_token.is_requested() {
                state.stopped = true;
                break;
            }
            let queued = state
                .correction_queue
                .iter()
                .filter(|item| !item.resolved)
                .map(|item| item.task_id.as_str())
                .collect::<BTreeSet<_>>();
            let ready = graph
                .tasks
                .iter()
                .filter(|task| {
                    !state.committed_task_ids.contains(&task.task_id)
                        && !queued.contains(task.task_id.as_str())
                        && task
                            .dependencies
                            .iter()
                            .all(|dep| state.committed_task_ids.contains(dep))
                })
                .take(self.budget.max_workers.max(1))
                .collect::<Vec<_>>();
            if ready.is_empty() {
                break;
            }
            for task in ready {
                if stop_token.is_requested() {
                    reports.push(Step11TaskReport {
                        task_id: task.task_id.clone(),
                        status: Step11TaskStatus::Stopped,
                        attempts: Vec::new(),
                        merged_tree_hash: None,
                    });
                    state.stopped = true;
                    break;
                }
                reports.push(self.execute_one(task, state, &dependents)?);
            }
            if state.stopped {
                break;
            }
        }

        append_blocked_dependency_reports(graph, state, &tasks, &mut reports);
        Ok(execution_report(
            graph,
            state,
            starting_tree_hash,
            self.budget.max_workers.max(1),
            reports,
        ))
    }

    fn execute_one(
        &self,
        task: &TrustedDevelopmentTask,
        state: &mut Step11ExecutionState,
        dependents: &BTreeMap<String, Vec<String>>,
    ) -> AdmResult<Step11TaskReport> {
        let mut attempts = Vec::new();
        let contract_report = task.workspace_contract.validate();
        if !contract_report.is_valid() {
            return Ok(queue_without_agent(
                task,
                state,
                dependents,
                Step11FailureKind::Input,
                "invalid WorkspaceChangeSet contract",
                contract_report
                    .issues
                    .iter()
                    .map(|issue| issue.code.clone())
                    .collect(),
            ));
        }
        if !state
            .accepted_base_hashes
            .contains(&task.workspace_contract.base_tree_hash)
        {
            return Ok(queue_without_agent(
                task,
                state,
                dependents,
                Step11FailureKind::Conflict,
                "contract base tree hash is not an accepted Step11 baseline",
                Vec::new(),
            ));
        }

        let max_retries = self
            .budget
            .max_retries
            .min(task.workspace_contract.resource_budget.max_retries);
        let mut previous_failure = None;
        for attempt in 1..=max_retries + 1 {
            match self
                .agent
                .execute_task(task, attempt, previous_failure.as_ref())
            {
                Ok(result) => {
                    if let Some(report) = handle_agent_result(
                        task,
                        state,
                        dependents,
                        result,
                        attempt,
                        max_retries,
                        &mut attempts,
                    )? {
                        return Ok(report);
                    }
                    previous_failure = attempts.last().and_then(|attempt| attempt.failure.clone());
                }
                Err(error) => {
                    let failure = agent_error_failure(error);
                    let retryable = failure.failure_kind.retryable() && attempt <= max_retries;
                    attempts.push(crate::stages::step11_v2::types::Step11AttemptReport {
                        attempt,
                        outcome: "agent_error".to_string(),
                        failure: Some(failure.clone()),
                        retryable,
                    });
                    if retryable {
                        previous_failure = Some(failure);
                        continue;
                    }
                    queue_failure(task, state, dependents, &failure, attempt);
                    return Ok(correction_report(task, attempts));
                }
            }
        }
        unreachable!("bounded Step11 attempt loop always returns")
    }
}
