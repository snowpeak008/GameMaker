#![forbid(unsafe_code)]

use adm_foundation::{AdmError, AdmResult, ContentHash, RunId, TaskId, UtcTimestamp};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEvent {
    pub task_id: TaskId,
    pub status: TaskStatus,
    pub message: String,
    pub timestamp: UtcTimestamp,
}

#[derive(Debug, Clone)]
pub struct TaskLog {
    pub run_id: RunId,
    events: Vec<RuntimeEvent>,
}

impl TaskLog {
    pub fn new(run_id: RunId) -> Self {
        Self {
            run_id,
            events: Vec::new(),
        }
    }

    pub fn record(
        &mut self,
        task_id: TaskId,
        status: TaskStatus,
        message: impl Into<String>,
    ) -> AdmResult<()> {
        self.events.push(RuntimeEvent {
            task_id,
            status,
            message: message.into(),
            timestamp: UtcTimestamp::now(),
        });
        Ok(())
    }

    pub fn events(&self) -> &[RuntimeEvent] {
        &self.events
    }
}

#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeValidationObservedStatus {
    Passed,
    Failed,
    Skipped,
}

impl RuntimeValidationObservedStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeValidationMergedStatus {
    Ready,
    Failed,
    Missing,
    Unexpected,
}

impl RuntimeValidationMergedStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Missing => "missing",
            Self::Unexpected => "unexpected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeValidationExecutionRow {
    pub result_id: String,
    pub scenario_id: String,
    pub test_id: String,
    pub acceptance_trace_id: String,
    pub telemetry_start_seen: bool,
    pub telemetry_complete_seen: bool,
    pub expected_state_seen: bool,
    pub failure_guard_triggered: bool,
    pub status: RuntimeValidationObservedStatus,
    pub notes: String,
}

impl RuntimeValidationExecutionRow {
    pub fn ready(&self) -> bool {
        self.status == RuntimeValidationObservedStatus::Passed
            && self.telemetry_start_seen
            && self.telemetry_complete_seen
            && self.expected_state_seen
            && !self.failure_guard_triggered
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeValidationExecutionInput {
    pub runner: String,
    pub target_id: String,
    pub source_hash: ContentHash,
    pub rows: Vec<RuntimeValidationExecutionRow>,
}

impl RuntimeValidationExecutionInput {
    pub fn parse(text: &str) -> AdmResult<Self> {
        let mut runner = "unknown".to_string();
        let mut target_id = "unknown".to_string();
        let mut rows = Vec::new();
        for (line_number, raw_line) in text.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(row) = line.strip_prefix("- ") {
                rows.push(parse_execution_row(row, line_number + 1)?);
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                match key.trim() {
                    "runner" => runner = clean_value(value),
                    "target_id" => target_id = clean_value(value),
                    _ => {}
                }
            }
        }
        if rows.is_empty() {
            return Err(AdmError::validation(
                "runtime validation execution input contains no result rows",
            ));
        }
        Ok(Self {
            runner,
            target_id,
            source_hash: ContentHash::from_bytes(text.as_bytes()),
            rows,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeValidationMergedRow {
    pub result_id: String,
    pub scenario_id: String,
    pub test_id: String,
    pub acceptance_trace_id: String,
    pub observed_status: String,
    pub telemetry_start_seen: Option<bool>,
    pub telemetry_complete_seen: Option<bool>,
    pub expected_state_seen: Option<bool>,
    pub failure_guard_triggered: Option<bool>,
    pub status: RuntimeValidationMergedStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeValidationExecutionSummary {
    pub runner: String,
    pub target_id: String,
    pub source_hash: ContentHash,
    pub contract_rows: usize,
    pub observed_rows: usize,
    pub passed_rows: usize,
    pub failed_rows: usize,
    pub missing_rows: usize,
    pub unexpected_rows: usize,
    pub rows: Vec<RuntimeValidationMergedRow>,
}

impl RuntimeValidationExecutionSummary {
    pub fn ready(&self) -> bool {
        self.contract_rows > 0
            && self.passed_rows == self.contract_rows
            && self.failed_rows == 0
            && self.missing_rows == 0
            && self.unexpected_rows == 0
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Runtime Validation Execution Results\n");
        document.push_str(&format!("runner={}\n", sanitize_inline(&self.runner)));
        document.push_str(&format!("target_id={}\n", sanitize_inline(&self.target_id)));
        document.push_str(&format!("source_hash={}\n", self.source_hash));
        document.push_str("contract_file=validation/runtime_validation_report.adm\n");
        document.push_str(&format!("contract_rows={}\n", self.contract_rows));
        document.push_str(&format!("observed_rows={}\n", self.observed_rows));
        document.push_str(&format!("passed_rows={}\n", self.passed_rows));
        document.push_str(&format!("failed_rows={}\n", self.failed_rows));
        document.push_str(&format!("missing_rows={}\n", self.missing_rows));
        document.push_str(&format!("unexpected_rows={}\n", self.unexpected_rows));
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str("\n## Results\n");
        for row in &self.rows {
            document.push_str(&format!(
                "- result_id={}; scenario_id={}; test_id={}; acceptance_trace_id={}; telemetry_start_seen={}; telemetry_complete_seen={}; expected_state_seen={}; failure_guard_triggered={}; observed_status={}; status={}; detail={}\n",
                sanitize_inline(&row.result_id),
                sanitize_inline(&row.scenario_id),
                sanitize_inline(&row.test_id),
                sanitize_inline(&row.acceptance_trace_id),
                optional_bool(row.telemetry_start_seen),
                optional_bool(row.telemetry_complete_seen),
                optional_bool(row.expected_state_seen),
                optional_bool(row.failure_guard_triggered),
                sanitize_inline(&row.observed_status),
                row.status.as_str(),
                sanitize_inline(&row.detail)
            ));
        }
        document
    }
}

pub fn summarize_runtime_validation_execution(
    contract_text: &str,
    execution_text: &str,
) -> AdmResult<RuntimeValidationExecutionSummary> {
    let contracts = parse_contract_rows(contract_text)?;
    if contracts.is_empty() {
        return Err(AdmError::validation(
            "runtime validation contract contains no result rows",
        ));
    }
    let input = RuntimeValidationExecutionInput::parse(execution_text)?;
    let mut seen = HashSet::new();
    let mut observed_by_result = HashMap::new();
    for row in &input.rows {
        if !seen.insert(row.result_id.clone()) {
            return Err(AdmError::validation(format!(
                "duplicate runtime validation result_id: {}",
                row.result_id
            )));
        }
        observed_by_result.insert(row.result_id.clone(), row.clone());
    }

    let mut rows = Vec::new();
    let mut passed_rows = 0;
    let mut failed_rows = 0;
    let mut missing_rows = 0;
    let mut expected_ids = HashSet::new();
    for contract in &contracts {
        expected_ids.insert(contract.result_id.clone());
        match observed_by_result.get(&contract.result_id) {
            Some(observed) => {
                let status = if observed.ready() {
                    passed_rows += 1;
                    RuntimeValidationMergedStatus::Ready
                } else {
                    failed_rows += 1;
                    RuntimeValidationMergedStatus::Failed
                };
                rows.push(RuntimeValidationMergedRow {
                    result_id: contract.result_id.clone(),
                    scenario_id: contract.scenario_id.clone(),
                    test_id: contract.test_id.clone(),
                    acceptance_trace_id: contract.acceptance_trace_id.clone(),
                    observed_status: observed.status.as_str().to_string(),
                    telemetry_start_seen: Some(observed.telemetry_start_seen),
                    telemetry_complete_seen: Some(observed.telemetry_complete_seen),
                    expected_state_seen: Some(observed.expected_state_seen),
                    failure_guard_triggered: Some(observed.failure_guard_triggered),
                    status,
                    detail: runtime_row_detail(observed),
                });
            }
            None => {
                missing_rows += 1;
                rows.push(RuntimeValidationMergedRow {
                    result_id: contract.result_id.clone(),
                    scenario_id: contract.scenario_id.clone(),
                    test_id: contract.test_id.clone(),
                    acceptance_trace_id: contract.acceptance_trace_id.clone(),
                    observed_status: "missing".to_string(),
                    telemetry_start_seen: None,
                    telemetry_complete_seen: None,
                    expected_state_seen: None,
                    failure_guard_triggered: None,
                    status: RuntimeValidationMergedStatus::Missing,
                    detail: "expected runtime result was not reported".to_string(),
                });
            }
        }
    }

    let mut unexpected_rows = 0;
    for observed in &input.rows {
        if expected_ids.contains(&observed.result_id) {
            continue;
        }
        unexpected_rows += 1;
        rows.push(RuntimeValidationMergedRow {
            result_id: observed.result_id.clone(),
            scenario_id: observed.scenario_id.clone(),
            test_id: observed.test_id.clone(),
            acceptance_trace_id: observed.acceptance_trace_id.clone(),
            observed_status: observed.status.as_str().to_string(),
            telemetry_start_seen: Some(observed.telemetry_start_seen),
            telemetry_complete_seen: Some(observed.telemetry_complete_seen),
            expected_state_seen: Some(observed.expected_state_seen),
            failure_guard_triggered: Some(observed.failure_guard_triggered),
            status: RuntimeValidationMergedStatus::Unexpected,
            detail: "runtime result was not declared by the validation contract".to_string(),
        });
    }

    Ok(RuntimeValidationExecutionSummary {
        runner: input.runner,
        target_id: input.target_id,
        source_hash: input.source_hash,
        contract_rows: contracts.len(),
        observed_rows: input.rows.len(),
        passed_rows,
        failed_rows,
        missing_rows,
        unexpected_rows,
        rows,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeValidationContractRow {
    result_id: String,
    scenario_id: String,
    test_id: String,
    acceptance_trace_id: String,
}

fn parse_contract_rows(text: &str) -> AdmResult<Vec<RuntimeValidationContractRow>> {
    let mut rows = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        let Some(row) = line.strip_prefix("- ") else {
            continue;
        };
        let fields = parse_fields(row);
        let Some(result_id) = fields.get("result_id").filter(|value| !value.is_empty()) else {
            continue;
        };
        rows.push(RuntimeValidationContractRow {
            result_id: result_id.clone(),
            scenario_id: fields.get("scenario_id").cloned().unwrap_or_default(),
            test_id: fields.get("test_id").cloned().unwrap_or_default(),
            acceptance_trace_id: fields
                .get("acceptance_trace_id")
                .cloned()
                .unwrap_or_default(),
        });
    }
    Ok(rows)
}

fn parse_execution_row(row: &str, line_number: usize) -> AdmResult<RuntimeValidationExecutionRow> {
    let fields = parse_fields(row);
    let result_id = required_field(&fields, "result_id", line_number)?;
    let status = parse_observed_status(&required_field(&fields, "status", line_number)?)?;
    Ok(RuntimeValidationExecutionRow {
        result_id,
        scenario_id: required_field(&fields, "scenario_id", line_number)?,
        test_id: required_field(&fields, "test_id", line_number)?,
        acceptance_trace_id: required_field(&fields, "acceptance_trace_id", line_number)?,
        telemetry_start_seen: parse_bool_field(&fields, "telemetry_start_seen", line_number)?,
        telemetry_complete_seen: parse_bool_field(&fields, "telemetry_complete_seen", line_number)?,
        expected_state_seen: parse_bool_field(&fields, "expected_state_seen", line_number)?,
        failure_guard_triggered: parse_bool_field(&fields, "failure_guard_triggered", line_number)?,
        status,
        notes: fields.get("notes").cloned().unwrap_or_default(),
    })
}

fn parse_fields(row: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    for part in row.split(';') {
        let Some((key, value)) = part.trim().split_once('=') else {
            continue;
        };
        fields.insert(key.trim().to_string(), clean_value(value));
    }
    fields
}

fn required_field(
    fields: &HashMap<String, String>,
    key: &str,
    line_number: usize,
) -> AdmResult<String> {
    fields
        .get(key)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| {
            AdmError::validation(format!("runtime result line {line_number} missing {key}"))
        })
}

fn parse_bool_field(
    fields: &HashMap<String, String>,
    key: &str,
    line_number: usize,
) -> AdmResult<bool> {
    match required_field(fields, key, line_number)?.as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        value => Err(AdmError::validation(format!(
            "runtime result line {line_number} has invalid {key}: {value}"
        ))),
    }
}

fn parse_observed_status(value: &str) -> AdmResult<RuntimeValidationObservedStatus> {
    match value {
        "passed" | "ready" => Ok(RuntimeValidationObservedStatus::Passed),
        "failed" => Ok(RuntimeValidationObservedStatus::Failed),
        "skipped" => Ok(RuntimeValidationObservedStatus::Skipped),
        other => Err(AdmError::validation(format!(
            "unsupported runtime validation status: {other}"
        ))),
    }
}

fn runtime_row_detail(row: &RuntimeValidationExecutionRow) -> String {
    if row.ready() {
        "runtime execution matched the validation contract".to_string()
    } else if row.status != RuntimeValidationObservedStatus::Passed {
        format!("runtime runner reported {}", row.status.as_str())
    } else if !row.telemetry_start_seen {
        "runtime start telemetry was not observed".to_string()
    } else if !row.telemetry_complete_seen {
        "runtime completion telemetry was not observed".to_string()
    } else if !row.expected_state_seen {
        "expected runtime state was not observed".to_string()
    } else if row.failure_guard_triggered {
        "failure guard was triggered".to_string()
    } else {
        "runtime execution did not satisfy the validation contract".to_string()
    }
}

fn optional_bool(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "missing",
    }
}

fn clean_value(value: &str) -> String {
    value.trim().to_string()
}

fn sanitize_inline(value: &str) -> String {
    value.replace(['\r', '\n', ';'], " ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_log_records_events_in_order() {
        let task_id = TaskId::generate();
        let mut log = TaskLog::new(RunId::generate());
        log.record(task_id.clone(), TaskStatus::Running, "started")
            .unwrap();
        log.record(task_id, TaskStatus::Succeeded, "done").unwrap();
        assert_eq!(log.events().len(), 2);
        assert_eq!(log.events()[0].status, TaskStatus::Running);
        assert_eq!(log.events()[1].status, TaskStatus::Succeeded);
    }

    #[test]
    fn cancellation_token_is_shareable() {
        let token = CancellationToken::new();
        let cloned = token.clone();
        cloned.cancel();
        assert!(token.is_cancelled());
    }

    fn contract() -> &'static str {
        "# Runtime Validation Report\n\
        - result_id=runtime_scenario_core_loop_step_1; scenario_id=scenario_core_loop_step_1; test_id=test_scenario_core_loop_step_1; acceptance_trace_id=trace_core_loop_step_1; status=ready\n\
        - result_id=runtime_scenario_core_loop_step_2; scenario_id=scenario_core_loop_step_2; test_id=test_scenario_core_loop_step_2; acceptance_trace_id=trace_core_loop_step_2; status=ready\n"
    }

    #[test]
    fn runtime_validation_execution_summary_accepts_matching_results() {
        let execution = "# Runtime Validation Execution\n\
            runner=unity_playmode\n\
            target_id=windows_desktop_playable\n\
            - result_id=runtime_scenario_core_loop_step_1; scenario_id=scenario_core_loop_step_1; test_id=test_scenario_core_loop_step_1; acceptance_trace_id=trace_core_loop_step_1; telemetry_start_seen=true; telemetry_complete_seen=true; expected_state_seen=true; failure_guard_triggered=false; status=passed\n\
            - result_id=runtime_scenario_core_loop_step_2; scenario_id=scenario_core_loop_step_2; test_id=test_scenario_core_loop_step_2; acceptance_trace_id=trace_core_loop_step_2; telemetry_start_seen=true; telemetry_complete_seen=true; expected_state_seen=true; failure_guard_triggered=false; status=passed\n";

        let summary =
            summarize_runtime_validation_execution(contract(), execution).expect("runtime summary");
        let rendered = summary.render();

        assert!(summary.ready());
        assert_eq!(summary.contract_rows, 2);
        assert_eq!(summary.observed_rows, 2);
        assert_eq!(summary.passed_rows, 2);
        assert_eq!(summary.failed_rows, 0);
        assert_eq!(summary.missing_rows, 0);
        assert_eq!(summary.unexpected_rows, 0);
        assert!(rendered.contains("# Runtime Validation Execution Results"));
        assert!(rendered.contains("ready=true"));
        assert!(rendered.contains("status=ready"));
    }

    #[test]
    fn runtime_validation_execution_summary_reports_missing_and_failed_rows() {
        let execution = "# Runtime Validation Execution\n\
            runner=unity_playmode\n\
            target_id=windows_desktop_playable\n\
            - result_id=runtime_scenario_core_loop_step_1; scenario_id=scenario_core_loop_step_1; test_id=test_scenario_core_loop_step_1; acceptance_trace_id=trace_core_loop_step_1; telemetry_start_seen=true; telemetry_complete_seen=false; expected_state_seen=true; failure_guard_triggered=false; status=passed\n\
            - result_id=runtime_extra; scenario_id=scenario_extra; test_id=test_extra; acceptance_trace_id=trace_extra; telemetry_start_seen=true; telemetry_complete_seen=true; expected_state_seen=true; failure_guard_triggered=false; status=passed\n";

        let summary =
            summarize_runtime_validation_execution(contract(), execution).expect("runtime summary");
        let rendered = summary.render();

        assert!(!summary.ready());
        assert_eq!(summary.passed_rows, 0);
        assert_eq!(summary.failed_rows, 1);
        assert_eq!(summary.missing_rows, 1);
        assert_eq!(summary.unexpected_rows, 1);
        assert!(rendered.contains("status=failed"));
        assert!(rendered.contains("status=missing"));
        assert!(rendered.contains("status=unexpected"));
    }
}
