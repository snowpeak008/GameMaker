use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    ChangeEvidence, ChangeFailureCategory, ChangeOutcome, SideEffectState, is_sha256, is_stable_id,
    sha256_bytes,
};

pub const WORKSPACE_CHANGE_SET_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceRelativePath(String);

impl WorkspaceRelativePath {
    pub fn parse(value: impl Into<String>) -> Result<Self, WorkspacePathError> {
        let value = value.into();
        let canonical = value.to_ascii_lowercase();
        if canonical.is_empty()
            || canonical.starts_with('/')
            || canonical.ends_with('/')
            || canonical.contains('\\')
            || canonical.contains(':')
            || canonical.contains('\0')
        {
            return Err(WorkspacePathError(value));
        }
        if canonical
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
        {
            return Err(WorkspacePathError(value));
        }
        Ok(Self(canonical))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn contains(&self, candidate: &Self) -> bool {
        candidate == self
            || candidate
                .0
                .strip_prefix(&self.0)
                .is_some_and(|suffix| suffix.starts_with('/'))
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        self.contains(other) || other.contains(self)
    }
}

impl fmt::Display for WorkspaceRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for WorkspaceRelativePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WorkspaceRelativePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(value).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePathError(String);

impl fmt::Display for WorkspacePathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "workspace path must be canonical, relative, and traversal-free: {:?}",
            self.0
        )
    }
}

impl std::error::Error for WorkspacePathError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceFileExpectation {
    Missing,
    Sha256 { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceFilePayload {
    Utf8 { content: String, sha256: String },
    Binary { bytes: Vec<u8>, sha256: String },
}

impl WorkspaceFilePayload {
    pub fn utf8(content: impl Into<String>) -> Self {
        let content = content.into();
        let sha256 = sha256_bytes(content.as_bytes());
        Self::Utf8 { content, sha256 }
    }

    pub fn binary(bytes: Vec<u8>) -> Self {
        let sha256 = sha256_bytes(&bytes);
        Self::Binary { bytes, sha256 }
    }

    fn byte_len(&self) -> u64 {
        match self {
            Self::Utf8 { content, .. } => content.len() as u64,
            Self::Binary { bytes, .. } => bytes.len() as u64,
        }
    }

    fn declared_hash(&self) -> &str {
        match self {
            Self::Utf8 { sha256, .. } | Self::Binary { sha256, .. } => sha256,
        }
    }

    fn actual_hash(&self) -> String {
        match self {
            Self::Utf8 { content, .. } => sha256_bytes(content.as_bytes()),
            Self::Binary { bytes, .. } => sha256_bytes(bytes),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkspaceOperation {
    WriteFile {
        path: WorkspaceRelativePath,
        expected: WorkspaceFileExpectation,
        payload: WorkspaceFilePayload,
    },
    DeleteFile {
        path: WorkspaceRelativePath,
        expected_sha256: String,
    },
    RenameFile {
        from: WorkspaceRelativePath,
        to: WorkspaceRelativePath,
        expected_source_sha256: String,
        expected_target: WorkspaceFileExpectation,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandPurpose {
    Compile,
    Test,
    Smoke,
    Tooling,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandPermission {
    pub command_id: String,
    pub tool_binding_id: String,
    pub purpose: CommandPurpose,
    #[serde(default)]
    pub argument_template: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<WorkspaceRelativePath>,
    pub timeout_ms: u64,
    #[serde(default)]
    pub allow_network: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedTestContract {
    pub test_id: String,
    pub path: WorkspaceRelativePath,
    pub baseline_sha256: String,
    pub command_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceResourceBudget {
    pub max_duration_ms: u64,
    pub max_processes: u32,
    pub max_write_bytes: u64,
    pub max_file_count: u32,
    pub max_retries: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceChangeSet {
    pub schema_version: String,
    pub change_set_id: String,
    pub base_tree_hash: String,
    #[serde(default)]
    pub read_paths: BTreeSet<WorkspaceRelativePath>,
    #[serde(default)]
    pub agent_write_paths: BTreeSet<WorkspaceRelativePath>,
    #[serde(default)]
    pub trusted_tool_write_paths: BTreeSet<WorkspaceRelativePath>,
    #[serde(default)]
    pub build_output_paths: BTreeSet<WorkspaceRelativePath>,
    #[serde(default)]
    pub operations: Vec<WorkspaceOperation>,
    #[serde(default)]
    pub command_permissions: Vec<CommandPermission>,
    #[serde(default)]
    pub trusted_tests: Vec<TrustedTestContract>,
    pub resource_budget: WorkspaceResourceBudget,
    #[serde(default)]
    pub evidence: Vec<ChangeEvidence>,
}

impl WorkspaceChangeSet {
    pub fn contract_hash(&self) -> Result<String, serde_json::Error> {
        serde_json::to_vec(self).map(|bytes| sha256_bytes(&bytes))
    }

    pub fn validate(&self) -> WorkspaceContractReport {
        let mut report = WorkspaceContractReport::default();
        if self.schema_version != WORKSPACE_CHANGE_SET_SCHEMA_VERSION {
            report.push(
                "workspace_change_set.schema_version",
                "/schemaVersion",
                ChangeFailureCategory::Input,
                "unsupported WorkspaceChangeSet schema version",
                "migrate the contract to the active schema version",
            );
        }
        if !is_stable_id(&self.change_set_id) {
            report.push(
                "workspace_change_set.change_id",
                "/changeSetId",
                ChangeFailureCategory::Input,
                "changeSetId must be a stable lowercase identifier",
                "use lowercase letters, digits, dots, underscores, or hyphens",
            );
        }
        if !is_sha256(&self.base_tree_hash) {
            report.push(
                "workspace_change_set.base_tree_hash",
                "/baseTreeHash",
                ChangeFailureCategory::Input,
                "baseTreeHash must be a lowercase SHA-256 digest",
                "compute the hash from the sealed workspace base tree",
            );
        }
        validate_budget(&self.resource_budget, &mut report);
        validate_scope_separation(self, &mut report);
        let commands = validate_commands(self, &mut report);
        validate_trusted_tests(self, &commands, &mut report);
        validate_operations(self, &mut report);
        validate_evidence(&self.evidence, "/evidence", &mut report);
        report
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceTransactionResult {
    pub schema_version: String,
    pub change_set_id: String,
    pub contract_sha256: String,
    pub base_tree_hash: String,
    pub outcome: ChangeOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_category: Option<ChangeFailureCategory>,
    pub side_effect_state: SideEffectState,
    pub stage: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resulting_tree_hash: Option<String>,
    #[serde(default)]
    pub agent_changed_paths: BTreeSet<WorkspaceRelativePath>,
    #[serde(default)]
    pub trusted_tool_changed_paths: BTreeSet<WorkspaceRelativePath>,
    #[serde(default)]
    pub build_output_changed_paths: BTreeSet<WorkspaceRelativePath>,
    #[serde(default)]
    pub trusted_test_hashes: BTreeMap<String, String>,
    #[serde(default)]
    pub evidence: Vec<ChangeEvidence>,
}

impl WorkspaceTransactionResult {
    pub fn validate_against(&self, contract: &WorkspaceChangeSet) -> WorkspaceContractReport {
        let mut report = WorkspaceContractReport::default();
        let expected_contract_hash = contract.contract_hash().ok();
        if self.schema_version != WORKSPACE_CHANGE_SET_SCHEMA_VERSION
            || self.change_set_id != contract.change_set_id
            || Some(&self.contract_sha256) != expected_contract_hash.as_ref()
            || self.base_tree_hash != contract.base_tree_hash
        {
            report.push(
                "workspace_result.contract_identity",
                "/",
                ChangeFailureCategory::Evidence,
                "transaction result does not identify its exact input contract",
                "bind the result to schemaVersion, changeSetId, and baseTreeHash",
            );
        }
        if self.stage.trim().is_empty() || self.evidence.is_empty() {
            report.push(
                "workspace_result.missing_evidence",
                "/evidence",
                ChangeFailureCategory::Evidence,
                "transaction result must preserve its stage and validation evidence",
                "record deterministic evidence before reporting the result",
            );
        }
        if self.outcome == ChangeOutcome::Rejected && self.failure_category.is_none() {
            report.push(
                "workspace_result.missing_failure_category",
                "/failureCategory",
                ChangeFailureCategory::Evidence,
                "a rejected result must carry a stable failure category",
                "classify the failure using the shared R0 taxonomy",
            );
        }
        if self.outcome == ChangeOutcome::Committed && self.failure_category.is_some() {
            report.push(
                "workspace_result.committed_failure",
                "/failureCategory",
                ChangeFailureCategory::Evidence,
                "a successful committed result cannot also claim failure",
                "use a rejected recovery-blocked result when post-commit verification fails",
            );
        }
        let has_committed_side_effect = matches!(
            self.side_effect_state,
            SideEffectState::Committed | SideEffectState::CommittedRecoveryBlocked
        );
        if self.outcome == ChangeOutcome::Committed
            && self.side_effect_state != SideEffectState::Committed
        {
            report.push(
                "workspace_result.invalid_commit_state",
                "/sideEffectState",
                ChangeFailureCategory::Evidence,
                "a successful outcome must identify a committed side effect",
                "report staged work as rejected until serial merge completes",
            );
        }
        if self
            .resulting_tree_hash
            .as_deref()
            .is_some_and(|hash| !is_sha256(hash))
        {
            report.push(
                "workspace_result.invalid_resulting_tree",
                "/resultingTreeHash",
                ChangeFailureCategory::Evidence,
                "resultingTreeHash is not a lowercase SHA-256 digest",
                "seal the observed post-execution workspace tree",
            );
        }
        if has_committed_side_effect
            && self
                .resulting_tree_hash
                .as_deref()
                .is_none_or(|hash| !is_sha256(hash))
        {
            report.push(
                "workspace_result.missing_resulting_tree",
                "/resultingTreeHash",
                ChangeFailureCategory::Evidence,
                "committed side effects require the resulting tree hash",
                "seal and record the post-commit workspace tree",
            );
        }
        if matches!(
            self.failure_category,
            Some(ChangeFailureCategory::ScopeViolation | ChangeFailureCategory::Evidence)
        ) && self.side_effect_state != SideEffectState::None
        {
            report.push(
                "workspace_result.forbidden_failure_side_effect",
                "/sideEffectState",
                ChangeFailureCategory::Evidence,
                "scope or evidence failures must not commit project side effects",
                "reject the isolated candidate before serial merge",
            );
        }
        validate_observed_scope(
            &self.agent_changed_paths,
            &contract.agent_write_paths,
            "/agentChangedPaths",
            &mut report,
        );
        validate_observed_scope(
            &self.trusted_tool_changed_paths,
            &contract.trusted_tool_write_paths,
            "/trustedToolChangedPaths",
            &mut report,
        );
        validate_observed_scope(
            &self.build_output_changed_paths,
            &contract.build_output_paths,
            "/buildOutputChangedPaths",
            &mut report,
        );
        validate_trusted_test_results(self, contract, &mut report);
        validate_evidence(&self.evidence, "/evidence", &mut report);
        report
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceContractIssue {
    pub code: String,
    pub path: String,
    pub category: ChangeFailureCategory,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceContractReport {
    #[serde(default)]
    pub issues: Vec<WorkspaceContractIssue>,
}

impl WorkspaceContractReport {
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn contains_code(&self, code: &str) -> bool {
        self.issues.iter().any(|issue| issue.code == code)
    }

    fn push(
        &mut self,
        code: impl Into<String>,
        path: impl Into<String>,
        category: ChangeFailureCategory,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) {
        self.issues.push(WorkspaceContractIssue {
            code: code.into(),
            path: path.into(),
            category,
            message: message.into(),
            suggestion: suggestion.into(),
        });
    }
}

fn validate_budget(budget: &WorkspaceResourceBudget, report: &mut WorkspaceContractReport) {
    if budget.max_duration_ms == 0
        || budget.max_processes == 0
        || budget.max_write_bytes == 0
        || budget.max_file_count == 0
    {
        report.push(
            "workspace_change_set.invalid_budget",
            "/resourceBudget",
            ChangeFailureCategory::Input,
            "duration, process, byte, and file budgets must be greater than zero",
            "supply explicit finite execution limits",
        );
    }
}

fn validate_scope_separation(contract: &WorkspaceChangeSet, report: &mut WorkspaceContractReport) {
    if contract.agent_write_paths.is_empty() {
        report.push(
            "workspace_change_set.empty_agent_scope",
            "/agentWritePaths",
            ChangeFailureCategory::Input,
            "agent write scope must not be empty",
            "declare every path the isolated coding agent may modify",
        );
    }
    for (left_name, left, right_name, right) in [
        (
            "agentWritePaths",
            &contract.agent_write_paths,
            "trustedToolWritePaths",
            &contract.trusted_tool_write_paths,
        ),
        (
            "agentWritePaths",
            &contract.agent_write_paths,
            "buildOutputPaths",
            &contract.build_output_paths,
        ),
        (
            "trustedToolWritePaths",
            &contract.trusted_tool_write_paths,
            "buildOutputPaths",
            &contract.build_output_paths,
        ),
    ] {
        for left_path in left {
            for right_path in right {
                if left_path.overlaps(right_path) {
                    report.push(
                        "workspace_change_set.attribution_overlap",
                        format!("/{left_name}"),
                        ChangeFailureCategory::ScopeViolation,
                        format!("'{left_path}' overlaps '{right_path}' in {right_name}"),
                        "separate agent, trusted-tool, and build-output ownership",
                    );
                }
            }
        }
    }
}

fn validate_commands<'a>(
    contract: &'a WorkspaceChangeSet,
    report: &mut WorkspaceContractReport,
) -> BTreeMap<String, &'a CommandPermission> {
    let mut commands = BTreeMap::new();
    if contract.command_permissions.is_empty() {
        report.push(
            "workspace_change_set.no_commands",
            "/commandPermissions",
            ChangeFailureCategory::Input,
            "at least one deterministic validation command must be declared",
            "declare compile, test, smoke, or tooling permissions by local binding id",
        );
    }
    for (index, command) in contract.command_permissions.iter().enumerate() {
        if !is_stable_id(&command.command_id) || !is_stable_id(&command.tool_binding_id) {
            report.push(
                "workspace_change_set.invalid_command_id",
                format!("/commandPermissions/{index}"),
                ChangeFailureCategory::Tooling,
                "command and tool binding ids must be stable identifiers, not machine paths",
                "resolve executable paths only in the local binding layer",
            );
        }
        if commands
            .insert(command.command_id.clone(), command)
            .is_some()
        {
            report.push(
                "workspace_change_set.duplicate_command",
                format!("/commandPermissions/{index}/commandId"),
                ChangeFailureCategory::Input,
                "command ids must be unique",
                "assign one stable id per allowed command",
            );
        }
        if command.timeout_ms == 0 || command.timeout_ms > contract.resource_budget.max_duration_ms
        {
            report.push(
                "workspace_change_set.command_timeout",
                format!("/commandPermissions/{index}/timeoutMs"),
                ChangeFailureCategory::Timeout,
                "command timeout must be finite and within the resource budget",
                "lower the command timeout or revise the local execution budget",
            );
        }
        for argument in &command.argument_template {
            if contains_machine_path(argument) {
                report.push(
                    "workspace_change_set.machine_path_argument",
                    format!("/commandPermissions/{index}/argumentTemplate"),
                    ChangeFailureCategory::Tooling,
                    "command templates cannot persist absolute or parent-relative machine paths",
                    "use workspace-relative paths or local binding placeholders",
                );
            }
            let lower = argument.to_ascii_lowercase();
            if ["authorization", "bearer ", "api_key", "api-key", "token="]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                report.push(
                    "workspace_change_set.sensitive_command_argument",
                    format!("/commandPermissions/{index}/argumentTemplate"),
                    ChangeFailureCategory::Evidence,
                    "command template appears to contain authentication material",
                    "resolve credentials in the local binding layer and keep them out of contracts",
                );
            }
        }
    }
    commands
}

fn validate_trusted_tests(
    contract: &WorkspaceChangeSet,
    commands: &BTreeMap<String, &CommandPermission>,
    report: &mut WorkspaceContractReport,
) {
    if contract.trusted_tests.is_empty() {
        report.push(
            "workspace_change_set.no_trusted_tests",
            "/trustedTests",
            ChangeFailureCategory::Test,
            "at least one immutable trusted test must be declared",
            "bind task acceptance to a baseline-hashed test outside agent write scope",
        );
    }
    let mut ids = BTreeSet::new();
    for (index, test) in contract.trusted_tests.iter().enumerate() {
        if !is_stable_id(&test.test_id) || !ids.insert(test.test_id.clone()) {
            report.push(
                "workspace_change_set.invalid_trusted_test_id",
                format!("/trustedTests/{index}/testId"),
                ChangeFailureCategory::Test,
                "trusted test ids must be unique stable identifiers",
                "assign a unique lowercase id to each trusted test",
            );
        }
        if !is_sha256(&test.baseline_sha256) {
            report.push(
                "workspace_change_set.invalid_trusted_test_hash",
                format!("/trustedTests/{index}/baselineSha256"),
                ChangeFailureCategory::Evidence,
                "trusted test baseline hash is invalid",
                "seal the trusted test before issuing the task contract",
            );
        }
        if !scope_contains(&contract.read_paths, &test.path) {
            report.push(
                "workspace_change_set.trusted_test_not_readable",
                format!("/trustedTests/{index}/path"),
                ChangeFailureCategory::Test,
                "trusted test path is outside the declared read set",
                "add the trusted test to the read set without adding write permission",
            );
        }
        if scope_contains(&contract.agent_write_paths, &test.path) {
            report.push(
                "workspace_change_set.trusted_test_writable",
                format!("/trustedTests/{index}/path"),
                ChangeFailureCategory::ScopeViolation,
                "coding agent write scope overlaps a trusted test",
                "move the test outside agent scope or narrow the write set",
            );
        }
        for (scope_name, scopes) in [
            ("trustedToolWritePaths", &contract.trusted_tool_write_paths),
            ("buildOutputPaths", &contract.build_output_paths),
        ] {
            if scope_contains(scopes, &test.path) {
                report.push(
                    "workspace_change_set.trusted_test_writable",
                    format!("/trustedTests/{index}/path"),
                    ChangeFailureCategory::ScopeViolation,
                    format!("{scope_name} overlaps a trusted test"),
                    "keep trusted tests outside every mutable output scope",
                );
            }
        }
        if let Some(command) = commands.get(&test.command_id) {
            if command.purpose != CommandPurpose::Test {
                report.push(
                    "workspace_change_set.trusted_test_command_purpose",
                    format!("/trustedTests/{index}/commandId"),
                    ChangeFailureCategory::Test,
                    "trusted test command must have purpose=test",
                    "bind the test to an explicitly reviewed test command",
                );
            }
        } else {
            report.push(
                "workspace_change_set.trusted_test_command_missing",
                format!("/trustedTests/{index}/commandId"),
                ChangeFailureCategory::Tooling,
                "trusted test references an undeclared command",
                "declare the exact command permission before execution",
            );
        }
    }
}

fn validate_operations(contract: &WorkspaceChangeSet, report: &mut WorkspaceContractReport) {
    if contract.operations.is_empty() {
        report.push(
            "workspace_change_set.no_operations",
            "/operations",
            ChangeFailureCategory::Input,
            "change set must contain at least one file operation",
            "declare the exact write, delete, or rename operations",
        );
        return;
    }
    let declared_files = contract.operations.len()
        + contract.trusted_tool_write_paths.len()
        + contract.build_output_paths.len();
    if declared_files > contract.resource_budget.max_file_count as usize {
        report.push(
            "workspace_change_set.file_budget_exceeded",
            "/operations",
            ChangeFailureCategory::Input,
            "declared file effects exceed maxFileCount",
            "split the task or increase its explicit local resource budget",
        );
    }
    let mut write_bytes = 0u64;
    for (index, operation) in contract.operations.iter().enumerate() {
        match operation {
            WorkspaceOperation::WriteFile {
                path,
                expected,
                payload,
            } => {
                validate_agent_path(path, index, contract, report);
                validate_expectation(expected, index, report);
                if !matches!(expected, WorkspaceFileExpectation::Missing)
                    && !scope_contains(&contract.read_paths, path)
                {
                    report.push(
                        "workspace_change_set.write_source_not_readable",
                        format!("/operations/{index}/path"),
                        ChangeFailureCategory::ScopeViolation,
                        "replacing an existing file requires declared read access",
                        "add the file to readPaths or declare it missing",
                    );
                }
                write_bytes = write_bytes.saturating_add(payload.byte_len());
                if !is_sha256(payload.declared_hash())
                    || payload.declared_hash() != payload.actual_hash()
                {
                    report.push(
                        "workspace_change_set.payload_hash_mismatch",
                        format!("/operations/{index}/payload"),
                        ChangeFailureCategory::Evidence,
                        "file payload hash is invalid or does not match its bytes",
                        "seal the exact UTF-8 or binary payload before submission",
                    );
                }
            }
            WorkspaceOperation::DeleteFile {
                path,
                expected_sha256,
            } => {
                validate_agent_path(path, index, contract, report);
                validate_hash(
                    expected_sha256,
                    format!("/operations/{index}/expectedSha256"),
                    report,
                );
                require_read_path(path, index, contract, report);
            }
            WorkspaceOperation::RenameFile {
                from,
                to,
                expected_source_sha256,
                expected_target,
            } => {
                validate_agent_path(from, index, contract, report);
                validate_agent_path(to, index, contract, report);
                validate_hash(
                    expected_source_sha256,
                    format!("/operations/{index}/expectedSourceSha256"),
                    report,
                );
                validate_expectation(expected_target, index, report);
                require_read_path(from, index, contract, report);
                if !matches!(expected_target, WorkspaceFileExpectation::Missing) {
                    require_read_path(to, index, contract, report);
                }
            }
        }
    }
    if write_bytes > contract.resource_budget.max_write_bytes {
        report.push(
            "workspace_change_set.byte_budget_exceeded",
            "/operations",
            ChangeFailureCategory::Input,
            "sealed payload bytes exceed maxWriteBytes",
            "split the task or increase its explicit local resource budget",
        );
    }
}

fn validate_agent_path(
    path: &WorkspaceRelativePath,
    index: usize,
    contract: &WorkspaceChangeSet,
    report: &mut WorkspaceContractReport,
) {
    if !scope_contains(&contract.agent_write_paths, path) {
        report.push(
            "workspace_change_set.operation_outside_agent_scope",
            format!("/operations/{index}"),
            ChangeFailureCategory::ScopeViolation,
            format!("operation path '{path}' is outside agentWritePaths"),
            "reject the candidate or issue a newly reviewed task contract",
        );
    }
}

fn require_read_path(
    path: &WorkspaceRelativePath,
    index: usize,
    contract: &WorkspaceChangeSet,
    report: &mut WorkspaceContractReport,
) {
    if !scope_contains(&contract.read_paths, path) {
        report.push(
            "workspace_change_set.operation_source_not_readable",
            format!("/operations/{index}"),
            ChangeFailureCategory::ScopeViolation,
            format!("operation source '{path}' is outside readPaths"),
            "declare source read access before issuing the change set",
        );
    }
}

fn validate_expectation(
    expected: &WorkspaceFileExpectation,
    index: usize,
    report: &mut WorkspaceContractReport,
) {
    if let WorkspaceFileExpectation::Sha256 { value } = expected {
        validate_hash(value, format!("/operations/{index}/expected"), report);
    }
}

fn validate_hash(hash: &str, path: impl Into<String>, report: &mut WorkspaceContractReport) {
    if !is_sha256(hash) {
        report.push(
            "workspace_change_set.invalid_hash",
            path,
            ChangeFailureCategory::Evidence,
            "expected file hash must be a lowercase SHA-256 digest",
            "seal the source file before issuing the change set",
        );
    }
}

fn validate_evidence(
    evidence: &[ChangeEvidence],
    path: &str,
    report: &mut WorkspaceContractReport,
) {
    if evidence.is_empty() {
        report.push(
            "workspace_change_set.missing_evidence",
            path,
            ChangeFailureCategory::Evidence,
            "contract or result has no validation evidence",
            "attach hash-only non-sensitive evidence before submission",
        );
    }
    let mut ids = BTreeSet::new();
    for (index, item) in evidence.iter().enumerate() {
        if !is_stable_id(&item.evidence_id)
            || !ids.insert(item.evidence_id.clone())
            || item.phase.trim().is_empty()
            || !is_sha256(&item.details_sha256)
        {
            report.push(
                "workspace_change_set.invalid_evidence",
                format!("{path}/{index}"),
                ChangeFailureCategory::Evidence,
                "evidence id, phase, and digest must be complete and unique",
                "record stable evidence metadata without secrets or raw prompts",
            );
        }
    }
}

fn validate_observed_scope(
    observed: &BTreeSet<WorkspaceRelativePath>,
    declared: &BTreeSet<WorkspaceRelativePath>,
    path: &str,
    report: &mut WorkspaceContractReport,
) {
    for item in observed {
        if !scope_contains(declared, item) {
            report.push(
                "workspace_result.observed_scope_violation",
                path,
                ChangeFailureCategory::ScopeViolation,
                format!("observed path '{item}' is outside its attributed write set"),
                "reject the isolated result without serial merge",
            );
        }
    }
}

fn validate_trusted_test_results(
    result: &WorkspaceTransactionResult,
    contract: &WorkspaceChangeSet,
    report: &mut WorkspaceContractReport,
) {
    let expected = contract
        .trusted_tests
        .iter()
        .map(|test| (test.test_id.as_str(), test.baseline_sha256.as_str()))
        .collect::<BTreeMap<_, _>>();
    for (test_id, baseline_hash) in &expected {
        match result.trusted_test_hashes.get(*test_id) {
            Some(observed) if observed == baseline_hash => {}
            Some(_) => report.push(
                "workspace_result.trusted_test_changed",
                format!("/trustedTestHashes/{test_id}"),
                ChangeFailureCategory::ScopeViolation,
                "trusted test hash changed from its sealed baseline",
                "reject the isolated candidate without serial merge",
            ),
            None => report.push(
                "workspace_result.trusted_test_hash_missing",
                format!("/trustedTestHashes/{test_id}"),
                ChangeFailureCategory::Evidence,
                "transaction result omitted a trusted test hash",
                "rehash every trusted test after isolated execution",
            ),
        }
    }
    for test_id in result.trusted_test_hashes.keys() {
        if !expected.contains_key(test_id.as_str()) {
            report.push(
                "workspace_result.unknown_trusted_test",
                format!("/trustedTestHashes/{test_id}"),
                ChangeFailureCategory::Evidence,
                "transaction result reports an undeclared trusted test",
                "bind evidence only to tests in the reviewed change-set contract",
            );
        }
    }
}

fn scope_contains(
    scopes: &BTreeSet<WorkspaceRelativePath>,
    candidate: &WorkspaceRelativePath,
) -> bool {
    scopes.iter().any(|scope| scope.contains(candidate))
}

fn contains_machine_path(value: &str) -> bool {
    let value = value.trim();
    let bytes = value.as_bytes();
    let drive_path = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\');
    let parent_traversal = bytes
        .windows(3)
        .any(|window| window[0] == b'.' && window[1] == b'.' && matches!(window[2], b'/' | b'\\'));
    drive_path
        || value.starts_with("\\\\")
        || (value.starts_with('/') && !value.starts_with("--"))
        || parent_traversal
}
