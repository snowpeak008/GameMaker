use std::collections::BTreeMap;

use adm_new_change_kernel::{
    ChangeFailureCategory, ChangeOutcome, RetryDisposition, WorkspaceContractReport,
    WorkspaceTransactionResult,
};
use adm_new_foundation::{AdmError, AdmResult, sha256_hex};
use serde_json::json;

use crate::stages::step08_10_v2::{TrustedDevelopmentTask, TrustedTaskGraph};
use crate::stages::step11_v2::types::{
    STEP11_V2_ENGINE_VERSION, Step11AttemptReport, Step11CorrectionQueueItem,
    Step11ExecutionReport, Step11ExecutionState, Step11ExecutionStatus, Step11FailureEvidence,
    Step11FailureKind, Step11TaskReport, Step11TaskStatus,
};

impl From<ChangeFailureCategory> for Step11FailureKind {
    fn from(value: ChangeFailureCategory) -> Self {
        match value {
            ChangeFailureCategory::Input => Self::Input,
            ChangeFailureCategory::AgentError => Self::AgentError,
            ChangeFailureCategory::ScopeViolation => Self::ScopeViolation,
            ChangeFailureCategory::Compile => Self::Compile,
            ChangeFailureCategory::Test => Self::Test,
            ChangeFailureCategory::Timeout => Self::Timeout,
            ChangeFailureCategory::Tooling => Self::Tooling,
            ChangeFailureCategory::Evidence => Self::Evidence,
            ChangeFailureCategory::Conflict => Self::Conflict,
        }
    }
}

pub(super) fn handle_agent_result(
    task: &TrustedDevelopmentTask,
    state: &mut Step11ExecutionState,
    dependents: &BTreeMap<String, Vec<String>>,
    result: WorkspaceTransactionResult,
    attempt: u32,
    max_retries: u32,
    attempts: &mut Vec<Step11AttemptReport>,
) -> AdmResult<Option<Step11TaskReport>> {
    let report = result.validate_against(&task.workspace_contract);
    let failure = result_failure(&result, &report);
    if failure.is_none() && result.outcome == ChangeOutcome::Committed {
        let merged = merge_tree_hash(
            &state.current_tree_hash,
            &task.task_id,
            result.resulting_tree_hash.as_deref().unwrap_or_default(),
            &task
                .workspace_contract
                .contract_hash()
                .map_err(|error| AdmError::new(format!("failed to hash contract: {error}")))?,
        );
        state.current_tree_hash = merged.clone();
        state.accepted_base_hashes.insert(merged.clone());
        state.committed_task_ids.insert(task.task_id.clone());
        attempts.push(Step11AttemptReport {
            attempt,
            outcome: "committed".to_string(),
            failure: None,
            retryable: false,
        });
        return Ok(Some(Step11TaskReport {
            task_id: task.task_id.clone(),
            status: Step11TaskStatus::Committed,
            attempts: attempts.clone(),
            merged_tree_hash: Some(merged),
        }));
    }

    let failure = failure.unwrap_or_else(|| Step11FailureEvidence {
        failure_kind: Step11FailureKind::Evidence,
        reason: "agent returned a non-committed result without evidence".to_string(),
        issue_codes: Vec::new(),
    });
    let retryable = failure.failure_kind.retryable() && attempt <= max_retries;
    attempts.push(Step11AttemptReport {
        attempt,
        outcome: "rejected".to_string(),
        failure: Some(failure.clone()),
        retryable,
    });
    if retryable {
        return Ok(None);
    }
    queue_failure(task, state, dependents, &failure, attempt);
    Ok(Some(correction_report(task, attempts.clone())))
}

fn result_failure(
    result: &WorkspaceTransactionResult,
    report: &WorkspaceContractReport,
) -> Option<Step11FailureEvidence> {
    if report.is_valid() && result.outcome == ChangeOutcome::Committed {
        return None;
    }
    let issue_codes = issue_codes(report);
    let kind = if issue_codes.iter().any(|code| {
        code == "workspace_result.observed_scope_violation"
            || code == "workspace_result.trusted_test_changed"
            || code == "workspace_result.forbidden_failure_side_effect"
    }) {
        Step11FailureKind::ScopeViolation
    } else if let Some(category) = report.issues.first().map(|issue| issue.category) {
        Step11FailureKind::from(category)
    } else {
        Step11FailureKind::from(
            result
                .failure_category
                .unwrap_or(ChangeFailureCategory::Evidence),
        )
    };
    let disposition = result
        .failure_category
        .map(ChangeFailureCategory::retry_disposition)
        .unwrap_or(RetryDisposition::Never);
    let reason = if issue_codes.is_empty() {
        result
            .failure_category
            .map(|category| category.as_str().to_string())
            .unwrap_or_else(|| "invalid transaction result".to_string())
    } else {
        format!(
            "workspace transaction failed validation: {}",
            issue_codes.join(",")
        )
    };
    Some(Step11FailureEvidence {
        failure_kind: if disposition == RetryDisposition::Never && kind.retryable() {
            Step11FailureKind::Evidence
        } else {
            kind
        },
        reason,
        issue_codes,
    })
}

pub(super) fn queue_without_agent(
    task: &TrustedDevelopmentTask,
    state: &mut Step11ExecutionState,
    dependents: &BTreeMap<String, Vec<String>>,
    failure_kind: Step11FailureKind,
    reason: &str,
    issue_codes: Vec<String>,
) -> Step11TaskReport {
    let failure = Step11FailureEvidence {
        failure_kind,
        reason: reason.to_string(),
        issue_codes,
    };
    queue_failure(task, state, dependents, &failure, 0);
    correction_report(task, Vec::new())
}

pub(super) fn queue_failure(
    task: &TrustedDevelopmentTask,
    state: &mut Step11ExecutionState,
    dependents: &BTreeMap<String, Vec<String>>,
    failure: &Step11FailureEvidence,
    attempts: u32,
) {
    if state
        .correction_queue
        .iter()
        .any(|item| item.task_id == task.task_id && !item.resolved)
    {
        return;
    }
    state.correction_queue.push(Step11CorrectionQueueItem {
        task_id: task.task_id.clone(),
        failure_kind: failure.failure_kind,
        reason: failure.reason.clone(),
        attempts,
        blocked_dependents: dependents.get(&task.task_id).cloned().unwrap_or_default(),
        resolved: false,
    });
}

pub(super) fn correction_report(
    task: &TrustedDevelopmentTask,
    attempts: Vec<Step11AttemptReport>,
) -> Step11TaskReport {
    Step11TaskReport {
        task_id: task.task_id.clone(),
        status: Step11TaskStatus::CorrectionQueued,
        attempts,
        merged_tree_hash: None,
    }
}

pub(super) fn append_blocked_dependency_reports(
    graph: &TrustedTaskGraph,
    state: &Step11ExecutionState,
    tasks: &BTreeMap<String, &TrustedDevelopmentTask>,
    reports: &mut Vec<Step11TaskReport>,
) {
    for task in graph.tasks.iter().filter(|task| {
        !state.committed_task_ids.contains(&task.task_id)
            && !state
                .correction_queue
                .iter()
                .any(|item| item.task_id == task.task_id)
            && task
                .dependencies
                .iter()
                .any(|dep| !state.committed_task_ids.contains(dep))
    }) {
        if tasks.contains_key(&task.task_id) {
            reports.push(Step11TaskReport {
                task_id: task.task_id.clone(),
                status: Step11TaskStatus::BlockedByDependency,
                attempts: Vec::new(),
                merged_tree_hash: None,
            });
        }
    }
}

pub(super) fn execution_report(
    graph: &TrustedTaskGraph,
    state: &Step11ExecutionState,
    starting_tree_hash: String,
    max_workers: usize,
    task_reports: Vec<Step11TaskReport>,
) -> Step11ExecutionReport {
    let unresolved = state.correction_queue.iter().any(|item| !item.resolved);
    let status = if state.stopped {
        Step11ExecutionStatus::Stopped
    } else if unresolved || state.committed_task_ids.len() != graph.tasks.len() {
        Step11ExecutionStatus::CorrectionRequired
    } else {
        Step11ExecutionStatus::Success
    };
    Step11ExecutionReport {
        schema_version: "step11_execution_report.v1".to_string(),
        engine_version: STEP11_V2_ENGINE_VERSION.to_string(),
        status,
        starting_tree_hash,
        ending_tree_hash: state.current_tree_hash.clone(),
        max_workers,
        committed_task_ids: state.committed_task_ids.iter().cloned().collect(),
        correction_queue: state.correction_queue.clone(),
        task_reports,
    }
}

pub(super) fn agent_error_failure(error: AdmError) -> Step11FailureEvidence {
    let kind = if error.to_string().to_ascii_lowercase().contains("timeout") {
        Step11FailureKind::Timeout
    } else {
        Step11FailureKind::AgentError
    };
    Step11FailureEvidence {
        failure_kind: kind,
        reason: error.to_string(),
        issue_codes: Vec::new(),
    }
}

pub(super) fn dependents_by_task(graph: &TrustedTaskGraph) -> BTreeMap<String, Vec<String>> {
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    for task in &graph.tasks {
        for dep in &task.dependencies {
            dependents
                .entry(dep.clone())
                .or_default()
                .push(task.task_id.clone());
        }
    }
    dependents
}

fn issue_codes(report: &WorkspaceContractReport) -> Vec<String> {
    report
        .issues
        .iter()
        .map(|issue| issue.code.clone())
        .collect()
}

fn merge_tree_hash(
    current_tree_hash: &str,
    task_id: &str,
    result_tree_hash: &str,
    contract_hash: &str,
) -> String {
    let payload = json!({
        "currentTreeHash": current_tree_hash,
        "taskId": task_id,
        "resultTreeHash": result_tree_hash,
        "contractHash": contract_hash,
        "engineVersion": STEP11_V2_ENGINE_VERSION,
    });
    sha256_hex(payload.to_string().as_bytes())
}
