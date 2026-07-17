use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeFailureCategory {
    Input,
    AgentError,
    ScopeViolation,
    Compile,
    Test,
    Timeout,
    Tooling,
    Evidence,
    Conflict,
}

impl ChangeFailureCategory {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::AgentError => "agent_error",
            Self::ScopeViolation => "scope_violation",
            Self::Compile => "compile",
            Self::Test => "test",
            Self::Timeout => "timeout",
            Self::Tooling => "tooling",
            Self::Evidence => "evidence",
            Self::Conflict => "conflict",
        }
    }

    pub fn retry_disposition(self) -> RetryDisposition {
        match self {
            Self::AgentError | Self::Compile | Self::Test | Self::Timeout => {
                RetryDisposition::ExecutionBudget
            }
            Self::Tooling => RetryDisposition::AfterBindingCorrection,
            Self::Input | Self::ScopeViolation | Self::Evidence | Self::Conflict => {
                RetryDisposition::Never
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryDisposition {
    Never,
    ExecutionBudget,
    AfterBindingCorrection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeOutcome {
    Committed,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectState {
    None,
    StagedOnly,
    Committed,
    CommittedRecoveryBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceStatus {
    Passed,
    Failed,
    Observed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeEvidence {
    pub evidence_id: String,
    pub phase: String,
    pub status: EvidenceStatus,
    pub details_sha256: String,
}

impl ChangeEvidence {
    pub fn from_bytes(
        evidence_id: impl Into<String>,
        phase: impl Into<String>,
        status: EvidenceStatus,
        details: &[u8],
    ) -> Self {
        Self {
            evidence_id: evidence_id.into(),
            phase: phase.into(),
            status,
            details_sha256: sha256_bytes(details),
        }
    }

    pub fn from_serializable<T: Serialize>(
        evidence_id: impl Into<String>,
        phase: impl Into<String>,
        status: EvidenceStatus,
        details: &T,
    ) -> Result<Self, ChangeKernelError> {
        let bytes = serde_json::to_vec(details).map_err(|error| {
            ChangeKernelError::new(
                "change_kernel.evidence_serialization_failed",
                format!("failed to serialize validation evidence: {error}"),
            )
        })?;
        Ok(Self::from_bytes(evidence_id, phase, status, &bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KernelHead {
    pub revision: u64,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeAuditRecord {
    pub sequence: u64,
    pub record_id: String,
    pub change_id: String,
    pub change_digest_sha256: String,
    pub claimed_base: KernelHead,
    pub observed_base: KernelHead,
    pub outcome: ChangeOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_category: Option<ChangeFailureCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_message: Option<String>,
    pub side_effect_state: SideEffectState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committed_head: Option<KernelHead>,
    #[serde(default)]
    pub evidence: Vec<ChangeEvidence>,
}

impl ChangeAuditRecord {
    pub(crate) fn seal(mut self) -> Result<Self, ChangeKernelError> {
        self.record_id.clear();
        let bytes = serde_json::to_vec(&self).map_err(|error| {
            ChangeKernelError::new(
                "change_kernel.audit_serialization_failed",
                format!("failed to serialize audit record: {error}"),
            )
        })?;
        self.record_id = sha256_bytes(&bytes);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeReceipt {
    pub audit: ChangeAuditRecord,
}

impl ChangeReceipt {
    pub fn committed(&self) -> bool {
        self.audit.outcome == ChangeOutcome::Committed
    }
}

pub trait ChangeKernel<C> {
    fn head(&self) -> Result<KernelHead, ChangeKernelError>;
    fn submit(&self, change: C) -> Result<ChangeReceipt, ChangeKernelError>;
    fn audit_log(&self) -> Result<Vec<ChangeAuditRecord>, ChangeKernelError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeKernelError {
    pub code: String,
    pub message: String,
}

impl ChangeKernelError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ChangeKernelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ChangeKernelError {}

pub(crate) fn sha256_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn is_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
}
