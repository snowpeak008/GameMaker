use std::collections::BTreeSet;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use serde::{Deserialize, Serialize};

pub const STEP11_V2_ENGINE_VERSION: &str = "workspace_change_step11_executor.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step11ExecutionBudget {
    pub max_workers: usize,
    pub max_retries: u32,
}

impl Default for Step11ExecutionBudget {
    fn default() -> Self {
        Self {
            max_workers: 1,
            max_retries: 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step11FailureKind {
    Input,
    Compile,
    Test,
    ScopeViolation,
    Timeout,
    AgentError,
    Tooling,
    Evidence,
    Conflict,
}

impl Step11FailureKind {
    pub fn retryable(self) -> bool {
        matches!(
            self,
            Self::Compile | Self::Test | Self::Timeout | Self::AgentError
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step11TaskStatus {
    Committed,
    CorrectionQueued,
    BlockedByDependency,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step11FailureEvidence {
    pub failure_kind: Step11FailureKind,
    pub reason: String,
    pub issue_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step11AttemptReport {
    pub attempt: u32,
    pub outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<Step11FailureEvidence>,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step11TaskReport {
    pub task_id: String,
    pub status: Step11TaskStatus,
    pub attempts: Vec<Step11AttemptReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_tree_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step11CorrectionQueueItem {
    pub task_id: String,
    pub failure_kind: Step11FailureKind,
    pub reason: String,
    pub attempts: u32,
    #[serde(default)]
    pub blocked_dependents: Vec<String>,
    #[serde(default)]
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step11ExecutionState {
    pub current_tree_hash: String,
    #[serde(default)]
    pub accepted_base_hashes: BTreeSet<String>,
    #[serde(default)]
    pub committed_task_ids: BTreeSet<String>,
    #[serde(default)]
    pub correction_queue: Vec<Step11CorrectionQueueItem>,
    #[serde(default)]
    pub stopped: bool,
}

impl Step11ExecutionState {
    pub fn new(initial_tree_hash: impl Into<String>) -> Self {
        let initial_tree_hash = initial_tree_hash.into();
        Self {
            current_tree_hash: initial_tree_hash.clone(),
            accepted_base_hashes: BTreeSet::from([initial_tree_hash]),
            committed_task_ids: BTreeSet::new(),
            correction_queue: Vec::new(),
            stopped: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step11ExecutionStatus {
    Success,
    CorrectionRequired,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step11ExecutionReport {
    pub schema_version: String,
    pub engine_version: String,
    pub status: Step11ExecutionStatus,
    pub starting_tree_hash: String,
    pub ending_tree_hash: String,
    pub max_workers: usize,
    pub committed_task_ids: Vec<String>,
    pub correction_queue: Vec<Step11CorrectionQueueItem>,
    pub task_reports: Vec<Step11TaskReport>,
}

#[derive(Debug, Clone, Default)]
pub struct Step11StopToken {
    requested: Arc<AtomicBool>,
}

impl Step11StopToken {
    pub fn from_shared(flag: Arc<AtomicBool>) -> Self {
        Self { requested: flag }
    }

    pub fn request_stop(&self) {
        self.requested.store(true, Ordering::SeqCst);
    }

    pub fn is_requested(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }
}
