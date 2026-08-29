#![forbid(unsafe_code)]

use adm_foundation::{
    AdmError, AdmErrorKind, AdmResult, ProviderId, TaskId, UtcTimestamp, read_to_string,
    write_string,
};
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AiCapability {
    TextGeneration,
    StructuredOutput,
    ScoringReview,
    CodeGeneration,
    ImageGeneration,
    SdkExplanation,
    LongTaskAgent,
}

impl AiCapability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TextGeneration => "text_generation",
            Self::StructuredOutput => "structured_output",
            Self::ScoringReview => "scoring_review",
            Self::CodeGeneration => "code_generation",
            Self::ImageGeneration => "image_generation",
            Self::SdkExplanation => "sdk_explanation",
            Self::LongTaskAgent => "long_task_agent",
        }
    }

    pub fn parse(value: &str) -> AdmResult<Self> {
        match value {
            "text_generation" => Ok(Self::TextGeneration),
            "structured_output" => Ok(Self::StructuredOutput),
            "scoring_review" => Ok(Self::ScoringReview),
            "code_generation" => Ok(Self::CodeGeneration),
            "image_generation" => Ok(Self::ImageGeneration),
            "sdk_explanation" => Ok(Self::SdkExplanation),
            "long_task_agent" => Ok(Self::LongTaskAgent),
            _ => Err(AdmError::validation(format!(
                "unknown AI capability: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiInterventionCriteria {
    pub min_quality_score_percent: u8,
    pub on_quality_gap: bool,
    pub on_missing_content: bool,
    pub review_after_generation: bool,
}

impl AiInterventionCriteria {
    pub fn new(
        min_quality_score_percent: u8,
        on_quality_gap: bool,
        on_missing_content: bool,
        review_after_generation: bool,
    ) -> AdmResult<Self> {
        let criteria = Self {
            min_quality_score_percent,
            on_quality_gap,
            on_missing_content,
            review_after_generation,
        };
        criteria.validate()?;
        Ok(criteria)
    }

    pub fn validate(&self) -> AdmResult<()> {
        if self.min_quality_score_percent > 100 {
            return Err(AdmError::validation(
                "AI intervention quality threshold cannot exceed 100",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiInterventionReason {
    QualityScoreBelowThreshold {
        score_percent: u8,
        threshold_percent: u8,
    },
    MissingRequiredContent {
        topics: Vec<String>,
    },
    PostGenerationReview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiInterventionDecision {
    pub required: bool,
    pub capability: AiCapability,
    pub reasons: Vec<AiInterventionReason>,
}

impl AiInterventionDecision {
    pub fn reason_summary(&self) -> String {
        if self.reasons.is_empty() {
            return "no AI intervention required".to_string();
        }
        self.reasons
            .iter()
            .map(|reason| match reason {
                AiInterventionReason::QualityScoreBelowThreshold {
                    score_percent,
                    threshold_percent,
                } => format!("quality score {score_percent} below {threshold_percent}"),
                AiInterventionReason::MissingRequiredContent { topics } => {
                    format!("missing content: {}", topics.join(","))
                }
                AiInterventionReason::PostGenerationReview => {
                    "post-generation review requested".to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

pub fn decide_ai_intervention(
    score_percent: u8,
    missing_topics: &[String],
    criteria: &AiInterventionCriteria,
) -> AdmResult<AiInterventionDecision> {
    criteria.validate()?;
    if score_percent > 100 {
        return Err(AdmError::validation(
            "AI intervention score cannot exceed 100",
        ));
    }

    let mut reasons = Vec::new();
    if criteria.on_quality_gap && score_percent < criteria.min_quality_score_percent {
        reasons.push(AiInterventionReason::QualityScoreBelowThreshold {
            score_percent,
            threshold_percent: criteria.min_quality_score_percent,
        });
    }
    if criteria.on_missing_content && !missing_topics.is_empty() {
        reasons.push(AiInterventionReason::MissingRequiredContent {
            topics: missing_topics.to_vec(),
        });
    }
    if criteria.review_after_generation {
        reasons.push(AiInterventionReason::PostGenerationReview);
    }

    Ok(AiInterventionDecision {
        required: !reasons.is_empty(),
        capability: AiCapability::TextGeneration,
        reasons,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiOutputState {
    Raw,
    Parsed,
    Validated,
    Accepted,
    Rejected,
}

impl AiOutputState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Parsed => "parsed",
            Self::Validated => "validated",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> AdmResult<Self> {
        match value {
            "raw" => Ok(Self::Raw),
            "parsed" => Ok(Self::Parsed),
            "validated" => Ok(Self::Validated),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            _ => Err(AdmError::validation(format!(
                "unknown AI output state: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiTaskRequest {
    pub task_id: TaskId,
    pub capability: AiCapability,
    pub prompt: String,
    pub context_summary: String,
}

impl AiTaskRequest {
    pub fn new(
        capability: AiCapability,
        prompt: impl Into<String>,
        context_summary: impl Into<String>,
    ) -> AdmResult<Self> {
        let prompt = prompt.into();
        if prompt.trim().is_empty() {
            return Err(AdmError::invalid_input("AI task prompt cannot be empty"));
        }
        Ok(Self {
            task_id: TaskId::generate(),
            capability,
            prompt,
            context_summary: context_summary.into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiTaskResult {
    pub task_id: TaskId,
    pub provider_id: ProviderId,
    pub output_state: AiOutputState,
    pub raw_output: String,
    pub validation_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiTaskStatus {
    Requested,
    ProviderSelected,
    Completed,
    Validated,
    Accepted,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AiFailureKind {
    BudgetExceeded,
    UnsupportedCapability,
    ProviderUnavailable,
    InvalidResponse,
    ValidationRejected,
    Unknown,
}

impl AiFailureKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BudgetExceeded => "budget_exceeded",
            Self::UnsupportedCapability => "unsupported_capability",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::InvalidResponse => "invalid_response",
            Self::ValidationRejected => "validation_rejected",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(value: &str) -> AdmResult<Self> {
        match value {
            "budget_exceeded" => Ok(Self::BudgetExceeded),
            "unsupported_capability" => Ok(Self::UnsupportedCapability),
            "provider_unavailable" => Ok(Self::ProviderUnavailable),
            "invalid_response" => Ok(Self::InvalidResponse),
            "validation_rejected" => Ok(Self::ValidationRejected),
            "unknown" => Ok(Self::Unknown),
            _ => Err(AdmError::validation(format!(
                "unknown AI failure kind: {value}"
            ))),
        }
    }
}

impl AiTaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::ProviderSelected => "provider_selected",
            Self::Completed => "completed",
            Self::Validated => "validated",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> AdmResult<Self> {
        match value {
            "requested" => Ok(Self::Requested),
            "provider_selected" => Ok(Self::ProviderSelected),
            "completed" => Ok(Self::Completed),
            "validated" => Ok(Self::Validated),
            "accepted" => Ok(Self::Accepted),
            "rejected" => Ok(Self::Rejected),
            "failed" => Ok(Self::Failed),
            _ => Err(AdmError::validation(format!(
                "unknown AI task status: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiTaskRecord {
    pub request: AiTaskRequest,
    pub provider_id: Option<ProviderId>,
    pub status: AiTaskStatus,
    pub result: Option<AiTaskResult>,
    pub budget_units: u32,
    pub attempts: u32,
    pub failure_kind: Option<AiFailureKind>,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
    pub error: Option<String>,
}

impl AiTaskRecord {
    pub fn new(request: AiTaskRequest, budget_units: u32) -> Self {
        let now = UtcTimestamp::now();
        Self {
            request,
            provider_id: None,
            status: AiTaskStatus::Requested,
            result: None,
            budget_units,
            attempts: 0,
            failure_kind: None,
            created_at: now,
            updated_at: now,
            error: None,
        }
    }

    pub fn select_provider(&mut self, provider_id: ProviderId) {
        self.provider_id = Some(provider_id);
        self.status = AiTaskStatus::ProviderSelected;
        self.updated_at = UtcTimestamp::now();
    }

    pub fn complete(&mut self, result: AiTaskResult) {
        self.status = AiTaskStatus::Completed;
        self.provider_id = Some(result.provider_id.clone());
        self.result = Some(result);
        self.updated_at = UtcTimestamp::now();
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        self.fail_with_kind(error, AiFailureKind::Unknown);
    }

    pub fn fail_with_kind(&mut self, error: impl Into<String>, failure_kind: AiFailureKind) {
        self.status = AiTaskStatus::Failed;
        self.error = Some(error.into());
        self.failure_kind = Some(failure_kind);
        self.updated_at = UtcTimestamp::now();
    }

    pub fn render(&self) -> String {
        let mut document = String::new();
        write_record_fields(&mut document, self);
        document
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiBudget {
    remaining_units: u32,
}

impl AiBudget {
    pub fn new(remaining_units: u32) -> Self {
        Self { remaining_units }
    }

    pub fn remaining_units(&self) -> u32 {
        self.remaining_units
    }

    pub fn consume(&mut self, units: u32) -> AdmResult<()> {
        if units > self.remaining_units {
            return Err(AdmError::conflict("AI budget is insufficient"));
        }
        self.remaining_units -= units;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRetryPolicy {
    max_attempts: u32,
}

impl AiRetryPolicy {
    pub fn new(max_attempts: u32) -> AdmResult<Self> {
        if max_attempts == 0 {
            return Err(AdmError::validation(
                "AI retry max_attempts must be greater than zero",
            ));
        }
        Ok(Self { max_attempts })
    }

    pub fn single_attempt() -> Self {
        Self { max_attempts: 1 }
    }

    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AiTaskJournal {
    records: Vec<AiTaskRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiFailureSummary {
    pub kind: AiFailureKind,
    pub count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AiTaskJournalSummary {
    pub record_count: usize,
    pub accepted_count: usize,
    pub failed_count: usize,
    pub rejected_count: usize,
    pub failure_kinds: Vec<AiFailureSummary>,
    pub last_failure_kind: Option<AiFailureKind>,
    pub last_error: Option<String>,
}

impl AiTaskJournalSummary {
    pub fn failure_summary_line(&self) -> String {
        self.failure_kinds
            .iter()
            .map(|failure| format!("{}={}", failure.kind.as_str(), failure.count))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn has_failures(&self) -> bool {
        self.failed_count > 0 || self.rejected_count > 0 || !self.failure_kinds.is_empty()
    }
}

const AI_TASK_JOURNAL_MAGIC: &str = "ADM_AI_JOURNAL_V1";

impl AiTaskJournal {
    pub fn push(&mut self, record: AiTaskRecord) {
        self.records.push(record);
    }

    pub fn records(&self) -> &[AiTaskRecord] {
        &self.records
    }

    pub fn summary(&self) -> AiTaskJournalSummary {
        let mut summary = AiTaskJournalSummary {
            record_count: self.records.len(),
            ..AiTaskJournalSummary::default()
        };
        let mut failure_counts = BTreeMap::<AiFailureKind, usize>::new();

        for record in &self.records {
            match record.status {
                AiTaskStatus::Accepted => summary.accepted_count += 1,
                AiTaskStatus::Failed => summary.failed_count += 1,
                AiTaskStatus::Rejected => summary.rejected_count += 1,
                _ => {}
            }

            let failure_kind = record.failure_kind.clone().or_else(|| {
                matches!(record.status, AiTaskStatus::Failed).then_some(AiFailureKind::Unknown)
            });
            if let Some(kind) = failure_kind {
                *failure_counts.entry(kind.clone()).or_insert(0) += 1;
                summary.last_failure_kind = Some(kind);
            }
            if let Some(error) = &record.error {
                summary.last_error = Some(error.clone());
            }
        }

        summary.failure_kinds = failure_counts
            .into_iter()
            .map(|(kind, count)| AiFailureSummary { kind, count })
            .collect();
        summary
    }

    pub fn render(&self) -> String {
        let mut document = String::from(AI_TASK_JOURNAL_MAGIC);
        document.push('\n');
        document.push_str(&format!("record_count={}\n", self.records.len()));
        for record in &self.records {
            document.push_str("[task]\n");
            write_record_fields(&mut document, record);
        }
        document
    }

    pub fn from_text(text: &str) -> AdmResult<Self> {
        let mut lines = text.lines();
        let header = lines
            .next()
            .ok_or_else(|| AdmError::validation("AI task journal is empty"))?
            .trim();
        if header != AI_TASK_JOURNAL_MAGIC {
            return Err(AdmError::validation(format!(
                "unsupported AI task journal format: {header}"
            )));
        }

        let mut task_values = Vec::new();
        let mut current = None;
        for raw_line in lines {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == "[task]" {
                if let Some(values) = current.take() {
                    task_values.push(values);
                }
                current = Some(BTreeMap::new());
                continue;
            }
            if line.starts_with("record_count=") {
                continue;
            }
            let Some(values) = current.as_mut() else {
                return Err(AdmError::validation(format!(
                    "journal key appears before [task]: {line}"
                )));
            };
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| AdmError::validation(format!("invalid journal line: {line}")))?;
            values.insert(key.to_string(), value.to_string());
        }
        if let Some(values) = current.take() {
            task_values.push(values);
        }

        let mut journal = Self::default();
        for values in task_values {
            journal.push(record_from_values(&values)?);
        }
        Ok(journal)
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> AdmResult<()> {
        write_string(path, &self.render())
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> AdmResult<Self> {
        Self::from_text(&read_to_string(path)?)
    }
}

fn write_record_fields(document: &mut String, record: &AiTaskRecord) {
    document.push_str(&format!("task_id={}\n", record.request.task_id));
    document.push_str(&format!(
        "capability={}\n",
        record.request.capability.as_str()
    ));
    document.push_str(&format!(
        "prompt_hex={}\n",
        hex_encode(record.request.prompt.as_bytes())
    ));
    document.push_str(&format!(
        "context_summary_hex={}\n",
        hex_encode(record.request.context_summary.as_bytes())
    ));
    document.push_str(&format!(
        "provider_id={}\n",
        record
            .provider_id
            .as_ref()
            .map(ProviderId::as_str)
            .unwrap_or("")
    ));
    document.push_str(&format!("status={}\n", record.status.as_str()));
    document.push_str(&format!("budget_units={}\n", record.budget_units));
    document.push_str(&format!("attempts={}\n", record.attempts));
    document.push_str(&format!(
        "failure_kind={}\n",
        record
            .failure_kind
            .as_ref()
            .map(AiFailureKind::as_str)
            .unwrap_or("")
    ));
    document.push_str(&format!("created_at={}\n", record.created_at.as_millis()));
    document.push_str(&format!("updated_at={}\n", record.updated_at.as_millis()));
    document.push_str(&format!(
        "error_hex={}\n",
        record
            .error
            .as_deref()
            .map(|error| hex_encode(error.as_bytes()))
            .unwrap_or_default()
    ));
    match &record.result {
        Some(result) => write_result_fields(document, result),
        None => document.push_str("has_result=false\n"),
    }
}

fn write_result_fields(document: &mut String, result: &AiTaskResult) {
    document.push_str("has_result=true\n");
    document.push_str(&format!("result.task_id={}\n", result.task_id));
    document.push_str(&format!("result.provider_id={}\n", result.provider_id));
    document.push_str(&format!(
        "result.output_state={}\n",
        result.output_state.as_str()
    ));
    document.push_str(&format!(
        "result.raw_output_hex={}\n",
        hex_encode(result.raw_output.as_bytes())
    ));
    document.push_str(&format!(
        "result.validation_notes_count={}\n",
        result.validation_notes.len()
    ));
    for (index, note) in result.validation_notes.iter().enumerate() {
        document.push_str(&format!(
            "result.validation_note.{index}.hex={}\n",
            hex_encode(note.as_bytes())
        ));
    }
}

fn record_from_values(values: &BTreeMap<String, String>) -> AdmResult<AiTaskRecord> {
    let task_id = TaskId::new(required_value(values, "task_id")?)?;
    let request = AiTaskRequest {
        task_id: task_id.clone(),
        capability: AiCapability::parse(&required_value(values, "capability")?)?,
        prompt: required_hex_string(values, "prompt_hex")?,
        context_summary: required_hex_string(values, "context_summary_hex")?,
    };
    let result = if parse_bool(values, "has_result")? {
        Some(AiTaskResult {
            task_id: TaskId::new(required_value(values, "result.task_id")?)?,
            provider_id: ProviderId::new(required_value(values, "result.provider_id")?)?,
            output_state: AiOutputState::parse(&required_value(values, "result.output_state")?)?,
            raw_output: required_hex_string(values, "result.raw_output_hex")?,
            validation_notes: parse_validation_notes(values)?,
        })
    } else {
        None
    };

    Ok(AiTaskRecord {
        request,
        provider_id: optional_string(values, "provider_id")
            .map(ProviderId::new)
            .transpose()?,
        status: AiTaskStatus::parse(&required_value(values, "status")?)?,
        result,
        budget_units: parse_u32(values, "budget_units")?,
        attempts: parse_u32(values, "attempts")?,
        failure_kind: optional_string(values, "failure_kind")
            .map(|value| AiFailureKind::parse(&value))
            .transpose()?,
        created_at: UtcTimestamp::from_millis(parse_u128(values, "created_at")?),
        updated_at: UtcTimestamp::from_millis(parse_u128(values, "updated_at")?),
        error: optional_hex_string(values, "error_hex")?,
    })
}

fn parse_validation_notes(values: &BTreeMap<String, String>) -> AdmResult<Vec<String>> {
    let count = optional_string(values, "result.validation_notes_count")
        .map(|value| parse_usize_value(&value, "result.validation_notes_count"))
        .transpose()?
        .unwrap_or(0);
    let mut notes = Vec::with_capacity(count);
    for index in 0..count {
        notes.push(required_hex_string(
            values,
            &format!("result.validation_note.{index}.hex"),
        )?);
    }
    Ok(notes)
}

fn required_value(values: &BTreeMap<String, String>, key: &str) -> AdmResult<String> {
    values
        .get(key)
        .cloned()
        .ok_or_else(|| AdmError::validation(format!("missing AI journal value: {key}")))
}

fn optional_string(values: &BTreeMap<String, String>, key: &str) -> Option<String> {
    values
        .get(key)
        .filter(|value| !value.trim().is_empty())
        .cloned()
}

fn required_hex_string(values: &BTreeMap<String, String>, key: &str) -> AdmResult<String> {
    hex_decode(&required_value(values, key)?)
}

fn optional_hex_string(values: &BTreeMap<String, String>, key: &str) -> AdmResult<Option<String>> {
    optional_string(values, key)
        .map(|value| hex_decode(&value))
        .transpose()
}

fn parse_bool(values: &BTreeMap<String, String>, key: &str) -> AdmResult<bool> {
    required_value(values, key)?
        .parse::<bool>()
        .map_err(|_| AdmError::validation(format!("{key} must be true or false")))
}

fn parse_u32(values: &BTreeMap<String, String>, key: &str) -> AdmResult<u32> {
    required_value(values, key)?
        .parse::<u32>()
        .map_err(|_| AdmError::validation(format!("{key} must be an unsigned integer")))
}

fn parse_u128(values: &BTreeMap<String, String>, key: &str) -> AdmResult<u128> {
    required_value(values, key)?
        .parse::<u128>()
        .map_err(|_| AdmError::validation(format!("{key} must be an unsigned integer")))
}

fn parse_usize_value(value: &str, key: &str) -> AdmResult<usize> {
    value
        .parse::<usize>()
        .map_err(|_| AdmError::validation(format!("{key} must be an unsigned integer")))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn hex_decode(value: &str) -> AdmResult<String> {
    if value.len() % 2 != 0 {
        return Err(AdmError::validation("hex value has odd length"));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    let mut chars = value.chars();
    while let Some(high) = chars.next() {
        let low = chars
            .next()
            .ok_or_else(|| AdmError::validation("hex value has odd length"))?;
        bytes.push((hex_nibble(high)? << 4) | hex_nibble(low)?);
    }
    String::from_utf8(bytes)
        .map_err(|error| AdmError::validation(format!("hex value is not UTF-8: {error}")))
}

fn hex_nibble(value: char) -> AdmResult<u8> {
    value
        .to_digit(16)
        .map(|digit| digit as u8)
        .ok_or_else(|| AdmError::validation(format!("invalid hex digit: {value}")))
}

impl AiTaskResult {
    pub fn validate(mut self, validator: &AiOutputValidator) -> Self {
        let notes = validator.validate_raw_output(&self.raw_output);
        self.validation_notes = notes;
        self.output_state = if self.validation_notes.is_empty() {
            AiOutputState::Validated
        } else {
            AiOutputState::Rejected
        };
        self
    }

    pub fn accept(mut self) -> AdmResult<Self> {
        if self.output_state != AiOutputState::Validated {
            return Err(AdmError::validation(
                "AI output must be validated before it can be accepted",
            ));
        }
        self.output_state = AiOutputState::Accepted;
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiOutputValidator {
    min_chars: usize,
    forbidden_markers: Vec<String>,
}

impl AiOutputValidator {
    pub fn strict_default() -> Self {
        Self {
            min_chars: 16,
            forbidden_markers: vec![
                "TODO".to_string(),
                "placeholder".to_string(),
                "占位".to_string(),
            ],
        }
    }

    pub fn validate_raw_output(&self, raw_output: &str) -> Vec<String> {
        let mut notes = Vec::new();
        if raw_output.trim().chars().count() < self.min_chars {
            notes.push("AI output is too short".to_string());
        }
        for marker in &self.forbidden_markers {
            if raw_output.contains(marker) {
                notes.push(format!("AI output contains forbidden marker: {marker}"));
            }
        }
        notes
    }
}

pub trait AiProvider {
    fn provider_id(&self) -> &ProviderId;
    fn supports(&self, capability: &AiCapability) -> bool;
    fn run(&self, request: &AiTaskRequest) -> AdmResult<AiTaskResult>;
}

pub struct AiProviderRouter<'a> {
    providers: Vec<&'a dyn AiProvider>,
}

impl<'a> AiProviderRouter<'a> {
    pub fn new(providers: Vec<&'a dyn AiProvider>) -> Self {
        Self { providers }
    }

    pub fn select(&self, capability: &AiCapability) -> AdmResult<&'a dyn AiProvider> {
        self.providers
            .iter()
            .copied()
            .find(|provider| provider.supports(capability))
            .ok_or_else(|| AdmError::unsupported(format!("no provider supports {capability:?}")))
    }

    pub fn run_with_budget(
        &self,
        request: AiTaskRequest,
        budget: &mut AiBudget,
        cost_units: u32,
    ) -> AiTaskRecord {
        self.run_with_budget_and_policy(
            request,
            budget,
            cost_units,
            AiRetryPolicy::single_attempt(),
        )
    }

    pub fn run_with_budget_and_policy(
        &self,
        request: AiTaskRequest,
        budget: &mut AiBudget,
        cost_units: u32,
        retry_policy: AiRetryPolicy,
    ) -> AiTaskRecord {
        let mut record = AiTaskRecord::new(request, cost_units);
        if let Err(error) = budget.consume(cost_units) {
            record.fail_with_kind(error.to_string(), AiFailureKind::BudgetExceeded);
            return record;
        }
        let provider = match self.select(&record.request.capability) {
            Ok(provider) => provider,
            Err(error) => {
                record.fail_with_kind(error.to_string(), AiFailureKind::UnsupportedCapability);
                return record;
            }
        };
        record.select_provider(provider.provider_id().clone());

        let mut last_error = None;
        let mut last_failure_kind = None;
        for _ in 0..retry_policy.max_attempts() {
            record.attempts += 1;
            match provider.run(&record.request) {
                Ok(result) => {
                    record.complete(result);
                    return record;
                }
                Err(error) => {
                    last_failure_kind = Some(classify_provider_error(&error));
                    last_error = Some(error.to_string());
                }
            }
        }
        record.fail_with_kind(
            last_error.unwrap_or_else(|| "AI provider did not run".to_string()),
            last_failure_kind.unwrap_or(AiFailureKind::Unknown),
        );
        record
    }
}

fn classify_provider_error(error: &AdmError) -> AiFailureKind {
    match error.kind() {
        AdmErrorKind::Unsupported => AiFailureKind::UnsupportedCapability,
        AdmErrorKind::Validation | AdmErrorKind::InvalidInput => AiFailureKind::InvalidResponse,
        AdmErrorKind::Io | AdmErrorKind::Conflict => AiFailureKind::ProviderUnavailable,
        _ => AiFailureKind::Unknown,
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AiSecretMaterial {
    value: String,
}

impl AiSecretMaterial {
    pub fn new(value: impl Into<String>) -> AdmResult<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AdmError::invalid_input(
                "AI provider secret cannot be empty",
            ));
        }
        Ok(Self { value })
    }

    pub fn expose_secret(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for AiSecretMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AiSecretMaterial(REDACTED)")
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AiRemoteRequest {
    provider_id: ProviderId,
    endpoint_hint: String,
    secret: AiSecretMaterial,
    capability: AiCapability,
    prompt: String,
    context_summary: String,
}

impl AiRemoteRequest {
    pub fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    pub fn endpoint_hint(&self) -> &str {
        &self.endpoint_hint
    }

    pub fn secret(&self) -> &AiSecretMaterial {
        &self.secret
    }

    pub fn capability(&self) -> &AiCapability {
        &self.capability
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn context_summary(&self) -> &str {
        &self.context_summary
    }
}

impl fmt::Debug for AiRemoteRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AiRemoteRequest")
            .field("provider_id", &self.provider_id)
            .field("endpoint_hint", &self.endpoint_hint)
            .field("secret", &self.secret)
            .field("capability", &self.capability)
            .field("prompt", &self.prompt)
            .field("context_summary", &self.context_summary)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRemoteResponse {
    pub raw_output: String,
}

impl AiRemoteResponse {
    pub fn new(raw_output: impl Into<String>) -> Self {
        Self {
            raw_output: raw_output.into(),
        }
    }
}

pub trait AiRemoteTransport {
    fn send(&self, request: &AiRemoteRequest) -> AdmResult<AiRemoteResponse>;
}

#[derive(Debug, Clone)]
pub struct RemoteAiProvider<T: AiRemoteTransport> {
    provider_id: ProviderId,
    endpoint_hint: String,
    secret: AiSecretMaterial,
    capabilities: Vec<AiCapability>,
    transport: T,
}

impl<T: AiRemoteTransport> RemoteAiProvider<T> {
    pub fn new(
        provider_id: ProviderId,
        endpoint_hint: impl Into<String>,
        secret: AiSecretMaterial,
        capabilities: Vec<AiCapability>,
        transport: T,
    ) -> AdmResult<Self> {
        let endpoint_hint = endpoint_hint.into();
        if endpoint_hint.trim().is_empty() {
            return Err(AdmError::invalid_input(
                "AI provider endpoint_hint cannot be empty",
            ));
        }
        if capabilities.is_empty() {
            return Err(AdmError::invalid_input(
                "AI provider capabilities cannot be empty",
            ));
        }
        Ok(Self {
            provider_id,
            endpoint_hint,
            secret,
            capabilities,
            transport,
        })
    }
}

impl<T: AiRemoteTransport> AiProvider for RemoteAiProvider<T> {
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    fn supports(&self, capability: &AiCapability) -> bool {
        self.capabilities.contains(capability)
    }

    fn run(&self, request: &AiTaskRequest) -> AdmResult<AiTaskResult> {
        if !self.supports(&request.capability) {
            return Err(AdmError::unsupported(format!(
                "provider {} does not support {:?}",
                self.provider_id, request.capability
            )));
        }
        let response = self.transport.send(&AiRemoteRequest {
            provider_id: self.provider_id.clone(),
            endpoint_hint: self.endpoint_hint.clone(),
            secret: self.secret.clone(),
            capability: request.capability.clone(),
            prompt: request.prompt.clone(),
            context_summary: request.context_summary.clone(),
        })?;
        Ok(AiTaskResult {
            task_id: request.task_id.clone(),
            provider_id: self.provider_id.clone(),
            output_state: AiOutputState::Raw,
            raw_output: response.raw_output,
            validation_notes: Vec::new(),
        })
    }
}

pub trait AiHttpJsonClient {
    fn post_json(
        &self,
        endpoint: &str,
        bearer_secret: &str,
        request_body: &str,
    ) -> AdmResult<String>;
}

#[derive(Debug, Clone)]
pub struct ReqwestBlockingHttpJsonClient {
    client: reqwest::blocking::Client,
}

impl ReqwestBlockingHttpJsonClient {
    pub fn new() -> AdmResult<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(map_reqwest_error)?;
        Ok(Self { client })
    }
}

impl AiHttpJsonClient for ReqwestBlockingHttpJsonClient {
    fn post_json(
        &self,
        endpoint: &str,
        bearer_secret: &str,
        request_body: &str,
    ) -> AdmResult<String> {
        if endpoint.trim().is_empty() {
            return Err(AdmError::invalid_input("AI HTTP endpoint cannot be empty"));
        }
        if bearer_secret.trim().is_empty() {
            return Err(AdmError::invalid_input(
                "AI HTTP bearer secret cannot be empty",
            ));
        }
        if request_body.trim().is_empty() {
            return Err(AdmError::invalid_input(
                "AI HTTP request body cannot be empty",
            ));
        }
        let response = self
            .client
            .post(endpoint.trim())
            .bearer_auth(bearer_secret.trim())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::ACCEPT, "application/json")
            .body(request_body.to_string())
            .send()
            .map_err(map_reqwest_error)?;
        let status = response.status();
        let response_body = response.text().map_err(map_reqwest_error)?;
        if !status.is_success() {
            return Err(AdmError::new(
                AdmErrorKind::Io,
                format!(
                    "AI HTTP request failed with status {status}: {}",
                    truncate_http_body(&response_body)
                ),
            ));
        }
        Ok(response_body)
    }
}

fn map_reqwest_error(error: reqwest::Error) -> AdmError {
    AdmError::new(AdmErrorKind::Io, format!("AI HTTP request failed: {error}"))
}

fn truncate_http_body(value: &str) -> String {
    const MAX_ERROR_BODY_CHARS: usize = 512;
    let trimmed = value.trim();
    if trimmed.chars().count() <= MAX_ERROR_BODY_CHARS {
        return trimmed.to_string();
    }
    let mut truncated = trimmed
        .chars()
        .take(MAX_ERROR_BODY_CHARS)
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

#[derive(Debug, Clone)]
pub struct ChatCompletionsTransport<C: AiHttpJsonClient> {
    model: String,
    client: C,
}

impl<C: AiHttpJsonClient> ChatCompletionsTransport<C> {
    pub fn new(model: impl Into<String>, client: C) -> AdmResult<Self> {
        let model = model.into();
        if model.trim().is_empty() {
            return Err(AdmError::invalid_input(
                "AI chat completions model cannot be empty",
            ));
        }
        Ok(Self { model, client })
    }

    pub fn model(&self) -> &str {
        &self.model
    }
}

impl<C: AiHttpJsonClient> AiRemoteTransport for ChatCompletionsTransport<C> {
    fn send(&self, request: &AiRemoteRequest) -> AdmResult<AiRemoteResponse> {
        let endpoint = chat_completions_endpoint(request.endpoint_hint());
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {
                    "role": "system",
                    "content": format!(
                        "Capability: {}\nContext: {}",
                        request.capability().as_str(),
                        request.context_summary()
                    )
                },
                {
                    "role": "user",
                    "content": request.prompt()
                }
            ]
        });
        let response_text = self.client.post_json(
            &endpoint,
            request.secret().expose_secret(),
            &body.to_string(),
        )?;
        let response: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|error| AdmError::validation(format!("invalid AI JSON response: {error}")))?;
        let content = response
            .get("choices")
            .and_then(serde_json::Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                AdmError::validation("AI JSON response does not contain message content")
            })?;
        if content.trim().is_empty() {
            return Err(AdmError::validation("AI JSON response content is empty"));
        }
        Ok(AiRemoteResponse::new(content))
    }
}

fn chat_completions_endpoint(endpoint_hint: &str) -> String {
    let trimmed = endpoint_hint.trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

#[derive(Debug, Clone)]
pub struct MockAiProvider {
    provider_id: ProviderId,
    capabilities: Vec<AiCapability>,
}

impl MockAiProvider {
    pub fn new(provider_id: ProviderId, capabilities: Vec<AiCapability>) -> Self {
        Self {
            provider_id,
            capabilities,
        }
    }
}

impl AiProvider for MockAiProvider {
    fn provider_id(&self) -> &ProviderId {
        &self.provider_id
    }

    fn supports(&self, capability: &AiCapability) -> bool {
        self.capabilities.contains(capability)
    }

    fn run(&self, request: &AiTaskRequest) -> AdmResult<AiTaskResult> {
        if !self.supports(&request.capability) {
            return Err(AdmError::unsupported(format!(
                "provider {} does not support {:?}",
                self.provider_id, request.capability
            )));
        }
        Ok(AiTaskResult {
            task_id: request.task_id.clone(),
            provider_id: self.provider_id.clone(),
            output_state: AiOutputState::Raw,
            raw_output: format!("mock output for {}", request.prompt),
            validation_notes: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    #[test]
    fn mock_provider_returns_raw_output_for_supported_capability() {
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let request = AiTaskRequest::new(AiCapability::TextGeneration, "write", "ctx").unwrap();
        let result = provider.run(&request).unwrap();
        assert_eq!(result.task_id, request.task_id);
        assert_eq!(result.output_state, AiOutputState::Raw);
    }

    #[test]
    fn mock_provider_rejects_unsupported_capability() {
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let request = AiTaskRequest::new(AiCapability::ImageGeneration, "draw", "ctx").unwrap();
        assert!(provider.run(&request).is_err());
    }

    #[derive(Clone, Debug)]
    struct CapturingTransport {
        seen: Rc<RefCell<Option<AiRemoteRequest>>>,
    }

    impl AiRemoteTransport for CapturingTransport {
        fn send(&self, request: &AiRemoteRequest) -> AdmResult<AiRemoteResponse> {
            *self.seen.borrow_mut() = Some(request.clone());
            Ok(AiRemoteResponse::new(
                "remote provider generated a sufficiently long response",
            ))
        }
    }

    #[test]
    fn remote_provider_uses_transport_without_leaking_secret_in_debug() {
        let seen = Rc::new(RefCell::new(None));
        let provider = RemoteAiProvider::new(
            ProviderId::new("remote").unwrap(),
            "https://example.invalid/v1",
            AiSecretMaterial::new("sk-test-secret").unwrap(),
            vec![AiCapability::TextGeneration],
            CapturingTransport { seen: seen.clone() },
        )
        .unwrap();
        let request = AiTaskRequest::new(AiCapability::TextGeneration, "draft", "ctx").unwrap();

        let result = provider.run(&request).unwrap();
        let remote_request = seen.borrow().clone().unwrap();
        let debug_text = format!("{remote_request:?}");

        assert_eq!(
            result.raw_output,
            "remote provider generated a sufficiently long response"
        );
        assert_eq!(remote_request.endpoint_hint(), "https://example.invalid/v1");
        assert_eq!(remote_request.secret().expose_secret(), "sk-test-secret");
        assert!(debug_text.contains("REDACTED"));
        assert!(!debug_text.contains("sk-test-secret"));
    }

    #[test]
    fn remote_provider_rejects_empty_secret_and_endpoint() {
        assert!(AiSecretMaterial::new(" ").is_err());
        let error = RemoteAiProvider::new(
            ProviderId::new("remote").unwrap(),
            " ",
            AiSecretMaterial::new("secret").unwrap(),
            vec![AiCapability::TextGeneration],
            CapturingTransport {
                seen: Rc::new(RefCell::new(None)),
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("endpoint_hint"));
    }

    #[derive(Clone, Debug)]
    struct CapturingHttpClient {
        seen: Rc<RefCell<Option<(String, String, String)>>>,
        response: String,
    }

    impl AiHttpJsonClient for CapturingHttpClient {
        fn post_json(
            &self,
            endpoint: &str,
            bearer_secret: &str,
            request_body: &str,
        ) -> AdmResult<String> {
            *self.seen.borrow_mut() = Some((
                endpoint.to_string(),
                bearer_secret.to_string(),
                request_body.to_string(),
            ));
            Ok(self.response.clone())
        }
    }

    #[test]
    fn chat_completions_transport_builds_request_and_parses_content() {
        let seen = Rc::new(RefCell::new(None));
        let transport = ChatCompletionsTransport::new(
            "gpt-test",
            CapturingHttpClient {
                seen: seen.clone(),
                response: r#"{"choices":[{"message":{"content":"chat transport generated enough content"}}]}"#
                    .to_string(),
            },
        )
        .unwrap();
        let provider = RemoteAiProvider::new(
            ProviderId::new("chat").unwrap(),
            "https://api.example.test/v1",
            AiSecretMaterial::new("runtime-secret").unwrap(),
            vec![AiCapability::TextGeneration],
            transport,
        )
        .unwrap();
        let request =
            AiTaskRequest::new(AiCapability::TextGeneration, "draft scene", "ctx").unwrap();

        let result = provider.run(&request).unwrap();
        let (endpoint, bearer_secret, body) = seen.borrow().clone().unwrap();
        let body: serde_json::Value = serde_json::from_str(&body).unwrap();

        assert_eq!(endpoint, "https://api.example.test/v1/chat/completions");
        assert_eq!(bearer_secret, "runtime-secret");
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["messages"][1]["content"], "draft scene");
        assert_eq!(result.raw_output, "chat transport generated enough content");
    }

    #[test]
    fn chat_completions_transport_rejects_missing_content() {
        let transport = ChatCompletionsTransport::new(
            "gpt-test",
            CapturingHttpClient {
                seen: Rc::new(RefCell::new(None)),
                response: r#"{"choices":[{"message":{}}]}"#.to_string(),
            },
        )
        .unwrap();
        let provider = RemoteAiProvider::new(
            ProviderId::new("chat").unwrap(),
            "https://api.example.test/v1/chat/completions",
            AiSecretMaterial::new("runtime-secret").unwrap(),
            vec![AiCapability::TextGeneration],
            transport,
        )
        .unwrap();
        let request =
            AiTaskRequest::new(AiCapability::TextGeneration, "draft scene", "ctx").unwrap();

        let error = provider.run(&request).unwrap_err();

        assert!(error.to_string().contains("message content"));
    }

    #[test]
    fn reqwest_http_client_rejects_invalid_inputs_without_network() {
        let client = ReqwestBlockingHttpJsonClient::new().unwrap();

        assert!(
            client
                .post_json("", "secret", "{}")
                .unwrap_err()
                .to_string()
                .contains("endpoint")
        );
        assert!(
            client
                .post_json("https://example.invalid/v1", "", "{}")
                .unwrap_err()
                .to_string()
                .contains("bearer secret")
        );
        assert!(
            client
                .post_json("https://example.invalid/v1", "secret", "")
                .unwrap_err()
                .to_string()
                .contains("request body")
        );
    }

    #[test]
    fn reqwest_http_client_does_not_include_secret_in_url_parse_error() {
        let client = ReqwestBlockingHttpJsonClient::new().unwrap();
        let error = client
            .post_json("not a valid url", "SHOULD_NOT_LEAK", "{}")
            .unwrap_err()
            .to_string();

        assert!(!error.contains("SHOULD_NOT_LEAK"));
    }

    #[test]
    fn ai_output_must_validate_before_acceptance() {
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let request =
            AiTaskRequest::new(AiCapability::TextGeneration, "write a design", "ctx").unwrap();
        let raw = provider.run(&request).unwrap();
        assert!(raw.clone().accept().is_err());

        let accepted = raw
            .validate(&AiOutputValidator::strict_default())
            .accept()
            .unwrap();
        assert_eq!(accepted.output_state, AiOutputState::Accepted);
    }

    #[test]
    fn provider_router_consumes_budget_and_records_task() {
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let router = AiProviderRouter::new(vec![&provider]);
        let mut budget = AiBudget::new(5);
        let request = AiTaskRequest::new(AiCapability::TextGeneration, "write", "ctx").unwrap();
        let record = router.run_with_budget(request, &mut budget, 2);

        assert_eq!(record.status, AiTaskStatus::Completed);
        assert_eq!(record.attempts, 1);
        assert_eq!(budget.remaining_units(), 3);
        assert!(record.result.is_some());
    }

    #[test]
    fn provider_router_records_budget_failure() {
        let provider = MockAiProvider::new(
            ProviderId::new("mock").unwrap(),
            vec![AiCapability::TextGeneration],
        );
        let router = AiProviderRouter::new(vec![&provider]);
        let mut budget = AiBudget::new(1);
        let request = AiTaskRequest::new(AiCapability::TextGeneration, "write", "ctx").unwrap();
        let record = router.run_with_budget(request, &mut budget, 2);

        assert_eq!(record.status, AiTaskStatus::Failed);
        assert_eq!(record.failure_kind, Some(AiFailureKind::BudgetExceeded));
        assert!(record.error.unwrap().contains("budget"));
    }

    #[test]
    fn provider_router_records_unsupported_capability_failure() {
        let router = AiProviderRouter::new(Vec::new());
        let mut budget = AiBudget::new(3);
        let request = AiTaskRequest::new(AiCapability::ImageGeneration, "draw", "ctx").unwrap();
        let record = router.run_with_budget(request, &mut budget, 1);

        assert_eq!(record.status, AiTaskStatus::Failed);
        assert_eq!(
            record.failure_kind,
            Some(AiFailureKind::UnsupportedCapability)
        );
    }

    #[test]
    fn provider_router_classifies_provider_validation_failure() {
        struct InvalidProvider {
            provider_id: ProviderId,
        }

        impl AiProvider for InvalidProvider {
            fn provider_id(&self) -> &ProviderId {
                &self.provider_id
            }

            fn supports(&self, capability: &AiCapability) -> bool {
                capability == &AiCapability::TextGeneration
            }

            fn run(&self, _request: &AiTaskRequest) -> AdmResult<AiTaskResult> {
                Err(AdmError::validation("invalid provider response"))
            }
        }

        let provider = InvalidProvider {
            provider_id: ProviderId::new("invalid").unwrap(),
        };
        let router = AiProviderRouter::new(vec![&provider]);
        let mut budget = AiBudget::new(3);
        let request = AiTaskRequest::new(AiCapability::TextGeneration, "write", "ctx").unwrap();
        let record = router.run_with_budget(request, &mut budget, 1);

        assert_eq!(record.status, AiTaskStatus::Failed);
        assert_eq!(record.failure_kind, Some(AiFailureKind::InvalidResponse));
    }

    #[test]
    fn provider_router_retries_provider_failures() {
        struct FlakyProvider {
            provider_id: ProviderId,
            calls: Cell<u32>,
        }

        impl AiProvider for FlakyProvider {
            fn provider_id(&self) -> &ProviderId {
                &self.provider_id
            }

            fn supports(&self, capability: &AiCapability) -> bool {
                capability == &AiCapability::TextGeneration
            }

            fn run(&self, request: &AiTaskRequest) -> AdmResult<AiTaskResult> {
                let calls = self.calls.get() + 1;
                self.calls.set(calls);
                if calls == 1 {
                    return Err(AdmError::conflict("temporary provider failure"));
                }
                Ok(AiTaskResult {
                    task_id: request.task_id.clone(),
                    provider_id: self.provider_id.clone(),
                    output_state: AiOutputState::Raw,
                    raw_output: "retry succeeded with sufficient output".to_string(),
                    validation_notes: Vec::new(),
                })
            }
        }

        let provider = FlakyProvider {
            provider_id: ProviderId::new("flaky").unwrap(),
            calls: Cell::new(0),
        };
        let router = AiProviderRouter::new(vec![&provider]);
        let mut budget = AiBudget::new(5);
        let request = AiTaskRequest::new(AiCapability::TextGeneration, "write", "ctx").unwrap();
        let record = router.run_with_budget_and_policy(
            request,
            &mut budget,
            1,
            AiRetryPolicy::new(2).unwrap(),
        );

        assert_eq!(record.status, AiTaskStatus::Completed);
        assert_eq!(record.attempts, 2);
        assert_eq!(provider.calls.get(), 2);
    }

    #[test]
    fn intervention_decision_tracks_quality_and_missing_content() {
        let criteria = AiInterventionCriteria::new(75, true, true, false).unwrap();
        let missing = vec!["feedback_loop".to_string()];
        let decision = decide_ai_intervention(60, &missing, &criteria).unwrap();

        assert!(decision.required);
        assert_eq!(decision.reasons.len(), 2);
        assert!(decision.reason_summary().contains("quality score"));
        assert!(decision.reason_summary().contains("feedback_loop"));
    }

    #[test]
    fn task_journal_round_trips_full_record() {
        let request = AiTaskRequest::new(
            AiCapability::TextGeneration,
            "write line 1\nwrite line 2",
            "context=design review",
        )
        .unwrap();
        let mut record = AiTaskRecord::new(request.clone(), 2);
        record.attempts = 2;
        record.select_provider(ProviderId::new("mock").unwrap());
        record.complete(AiTaskResult {
            task_id: request.task_id.clone(),
            provider_id: ProviderId::new("mock").unwrap(),
            output_state: AiOutputState::Accepted,
            raw_output: "accepted output\nwith multiple lines".to_string(),
            validation_notes: vec!["note=ok".to_string(), "second note".to_string()],
        });
        record.status = AiTaskStatus::Accepted;

        let mut journal = AiTaskJournal::default();
        journal.push(record);
        let parsed = AiTaskJournal::from_text(&journal.render()).unwrap();

        assert_eq!(parsed, journal);
        assert_eq!(
            parsed.records()[0]
                .result
                .as_ref()
                .unwrap()
                .validation_notes[0],
            "note=ok"
        );
    }

    #[test]
    fn task_journal_summary_counts_failures() {
        let accepted_request =
            AiTaskRequest::new(AiCapability::TextGeneration, "draft", "ctx").unwrap();
        let mut accepted = AiTaskRecord::new(accepted_request.clone(), 1);
        accepted.status = AiTaskStatus::Accepted;

        let budget_request =
            AiTaskRequest::new(AiCapability::ScoringReview, "score", "ctx").unwrap();
        let mut budget_failure = AiTaskRecord::new(budget_request, 1);
        budget_failure.fail_with_kind("budget empty", AiFailureKind::BudgetExceeded);

        let provider_request =
            AiTaskRequest::new(AiCapability::CodeGeneration, "code", "ctx").unwrap();
        let mut provider_failure = AiTaskRecord::new(provider_request, 1);
        provider_failure.fail_with_kind("provider offline", AiFailureKind::ProviderUnavailable);

        let mut journal = AiTaskJournal::default();
        journal.push(accepted);
        journal.push(budget_failure);
        journal.push(provider_failure);

        let summary = journal.summary();

        assert_eq!(summary.record_count, 3);
        assert_eq!(summary.accepted_count, 1);
        assert_eq!(summary.failed_count, 2);
        assert_eq!(
            summary.failure_summary_line(),
            "budget_exceeded=1, provider_unavailable=1"
        );
        assert_eq!(
            summary.last_failure_kind,
            Some(AiFailureKind::ProviderUnavailable)
        );
        assert_eq!(summary.last_error.as_deref(), Some("provider offline"));
    }

    #[test]
    fn task_journal_saves_and_loads_from_file() {
        let request = AiTaskRequest::new(AiCapability::ScoringReview, "score", "ctx").unwrap();
        let mut record = AiTaskRecord::new(request, 1);
        record.fail("provider unavailable");
        let mut journal = AiTaskJournal::default();
        journal.push(record);
        let root = std::env::temp_dir().join(format!(
            "adm_ai_journal_{}_{}",
            std::process::id(),
            TaskId::generate()
        ));
        let path = root.join("ai").join("journal.adm");

        journal.save_to_path(&path).unwrap();
        let loaded = AiTaskJournal::load_from_path(&path).unwrap();

        assert_eq!(loaded, journal);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn task_journal_rejects_unknown_format() {
        let error = AiTaskJournal::from_text("# old journal").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("unsupported AI task journal format")
        );
    }
}
