#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use adm_new_contracts::ai::{
    AiConfig, AiResponseMode, AiResponsePayload, AiSchemaMode, ApiCategory, ApiEntry,
    CompletionJsonResult, HIGH_CONFIDENCE_THRESHOLD, ModelResult, ModelResultStatus, ModelTask,
    PartialProjectOutput,
};
use adm_new_contracts::project::{NodeState, ProjectState};
use adm_new_design::{DesignEngineService, DesignNodeSpec};
use adm_new_foundation::unix_timestamp;
use adm_new_foundation::{AdmError, AdmResult, ensure_relative_path, new_stable_id};
use adm_new_storage::ProjectRoot;
use serde_json::Value;

pub use adm_new_config::AiConfigDescriptorView;
pub use resolution::{AiCliProbeView, AiResolutionView};

pub mod adapters;
pub mod api_probe;
pub mod cli_probe;
pub mod design_contracts;
mod http_endpoint_policy;
pub mod image;
pub mod image_execution;
pub mod resolution;

pub use adm_new_config::{AiAdapterKind, AiConfigCategory, AiConfigSource};

pub const CRATE_NAME: &str = "adm-new-ai";
pub const AI_CONFIG_PATH: &str = "settings/ai_config.json";
pub const SECRET_UNCHANGED_MASK: &str = "********";
pub const DEFAULT_STRUCTURED_COMPLETION_TIMEOUT_SECONDS: u64 = 600;

pub fn crate_ready() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct AiConfigService {
    project_root: ProjectRoot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiConfigValidationReport {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct CompletionAdapterSpec {
    pub entry_id: String,
    pub config_type: String,
    pub adapter_kind: String,
    pub api_url: String,
    pub has_api_key: bool,
}

impl fmt::Debug for CompletionAdapterSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletionAdapterSpec")
            .field("entry_id", &self.entry_id)
            .field("config_type", &self.config_type)
            .field("adapter_kind", &self.adapter_kind)
            .field("api_url_configured", &!self.api_url.trim().is_empty())
            .field("has_api_key", &self.has_api_key)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct AiInterviewService {
    engine: DesignEngineService,
    node_domains: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiInterviewTurnReport {
    pub mode: AiResponseMode,
    pub applied_project_state: bool,
    pub archive_path: String,
    pub memory_events: Vec<String>,
    pub diagnostics: Vec<String>,
}

impl AiInterviewService {
    pub fn new(specs: Vec<DesignNodeSpec>) -> Self {
        let node_domains = specs
            .iter()
            .map(|spec| (spec.node_id.clone(), spec.domain_id.clone()))
            .collect();
        Self {
            engine: DesignEngineService::new(specs),
            node_domains,
        }
    }

    pub fn handle_payload_json(
        &self,
        state: &mut ProjectState,
        schema_mode: AiSchemaMode,
        payload_json: &str,
    ) -> AdmResult<AiInterviewTurnReport> {
        let payload: AiResponsePayload = serde_json::from_str(payload_json)
            .map_err(|error| AdmError::new(format!("invalid AI payload JSON: {error}")))?;
        self.handle_payload(state, schema_mode, payload)
    }

    pub fn handle_payload(
        &self,
        state: &mut ProjectState,
        schema_mode: AiSchemaMode,
        payload: AiResponsePayload,
    ) -> AdmResult<AiInterviewTurnReport> {
        validate_payload_for_schema_mode(&schema_mode, &payload)?;
        let mut diagnostics = Vec::new();
        let mut memory_events = Vec::new();
        let mut applied_project_state = false;
        let response_mode = payload.mode.clone();
        state.ai_interview.session_turn_count += 1;
        state.ai_interview.status = "completed".to_string();
        state.ai_interview.backend_stage = "completed".to_string();
        state.ai_interview.updated_at = timestamp();
        if !payload.assistant_message.is_empty() {
            state.ai_interview.messages.push(serde_json::json!({
                "role": "assistant",
                "content": payload.assistant_message.clone(),
                "turn": state.ai_interview.session_turn_count,
            }));
        }
        if let Some(route_overview) = payload.route_overview.clone() {
            state.ai_interview.route_overview = route_overview;
        }
        state
            .ai_interview
            .inferences
            .extend(payload.inferences.clone());
        state.ai_interview.option_differences = payload.option_differences.clone();

        match response_mode.clone() {
            AiResponseMode::QuestionGroup => {
                state.ai_interview.question_group_count += 1;
                state.ai_interview.current_question_count += 1;
                state.ai_interview.awaiting_user_answer = true;
                state.ai_interview.current_question_text =
                    question_text(payload.question_group.as_ref(), &payload.assistant_message);
                memory_events.push("question_group_review_recorded".to_string());
            }
            AiResponseMode::ReadinessCheck => {
                state.ai_interview.awaiting_user_answer = false;
                state.ai_interview.last_readiness_check_group =
                    state.ai_interview.question_group_count;
                memory_events.push("readiness_check_recorded".to_string());
            }
            AiResponseMode::FullProjectOutput => {
                let full_output = payload
                    .full_project_output
                    .as_ref()
                    .ok_or_else(|| AdmError::new("full_project_output is required"))?;
                let (candidate, confidence) = parse_full_project_output(full_output)?;
                applied_project_state =
                    self.apply_high_confidence_project_state(state, candidate, &confidence)?;
                if applied_project_state {
                    memory_events.push("high_confidence_project_state_applied".to_string());
                } else {
                    diagnostics.push("full output was below confidence threshold".to_string());
                    memory_events.push("low_confidence_output_recorded".to_string());
                }
                state.ai_interview.output_history.push(full_output.clone());
            }
            AiResponseMode::PartialProjectOutput => {
                let partial = payload
                    .partial_project_output
                    .as_ref()
                    .ok_or_else(|| AdmError::new("partial_project_output is required"))?;
                applied_project_state = self.apply_partial_project_output(state, partial)?;
                if applied_project_state {
                    memory_events.push("partial_project_output_applied".to_string());
                } else {
                    memory_events.push("partial_project_output_below_threshold".to_string());
                }
            }
            AiResponseMode::Mapping => {
                memory_events.push("mapping_payload_context_recorded".to_string());
            }
            AiResponseMode::SummaryCorrection => {
                if let Some(summary) = payload.summary.clone() {
                    state.ai_interview.summary.v1.updated_at = timestamp();
                    state
                        .ai_interview
                        .summary
                        .v1
                        .node_notes
                        .insert("summary_correction".to_string(), summary);
                }
                memory_events.push("summary_correction_recorded".to_string());
            }
            AiResponseMode::Confirmation | AiResponseMode::Maintenance => {
                memory_events.push("non_output_turn_recorded".to_string());
            }
            AiResponseMode::Error => {
                state.ai_interview.status = "error".to_string();
                state.ai_interview.last_error = payload.errors.join("; ");
                memory_events.push("backend_error_recorded".to_string());
            }
        }

        let archive_path = format!(
            "ai_archives/auto/turn_{}.json",
            state.ai_interview.session_turn_count
        );
        state.ai_interview.auto_archive_path = archive_path.clone();
        state.ai_interview.last_archived_at = timestamp();
        state.ai_interview.framework_memory.updated_at = timestamp();
        for event in &memory_events {
            let key = format!(
                "turn_{}_{}",
                state.ai_interview.session_turn_count,
                state.ai_interview.framework_memory.review_chains.len()
            );
            state
                .ai_interview
                .framework_memory
                .review_chains
                .insert(key, Value::String(event.clone()));
        }

        Ok(AiInterviewTurnReport {
            mode: response_mode,
            applied_project_state,
            archive_path,
            memory_events,
            diagnostics,
        })
    }

    pub fn merge_partial_project_outputs(
        &self,
        state: &mut ProjectState,
        outputs: &[PartialProjectOutput],
    ) -> AdmResult<bool> {
        let mut applied = false;
        for output in outputs {
            applied |= self.apply_partial_project_output(state, output)?;
        }
        Ok(applied)
    }

    fn apply_high_confidence_project_state(
        &self,
        state: &mut ProjectState,
        candidate: ProjectState,
        confidence: &Value,
    ) -> AdmResult<bool> {
        let candidate = self.engine.normalize_state(candidate);
        let mut applied = false;
        if confidence_for_node(confidence, "gameplaySystems") >= HIGH_CONFIDENCE_THRESHOLD as f64 {
            state.gameplay_systems = candidate.gameplay_systems.clone();
            applied = true;
        }
        for (node_id, candidate_node) in candidate.nodes {
            let confidence = confidence_for_node(confidence, &node_id);
            if confidence >= HIGH_CONFIDENCE_THRESHOLD as f64 {
                state.nodes.insert(node_id, candidate_node);
                applied = true;
            }
        }
        if applied {
            *state = self.engine.normalize_state(state.clone());
        }
        Ok(applied)
    }

    fn apply_partial_project_output(
        &self,
        state: &mut ProjectState,
        output: &PartialProjectOutput,
    ) -> AdmResult<bool> {
        validate_partial_output_domains(output, &self.node_domains)?;
        let patch: Value =
            serde_json::from_str(&output.project_state_patch_json).map_err(|error| {
                AdmError::new(format!("invalid partial project patch JSON: {error}"))
            })?;
        let confidence: Value =
            serde_json::from_str(&output.confidence_map_json).map_err(|error| {
                AdmError::new(format!("invalid partial confidence map JSON: {error}"))
            })?;
        let Some(nodes) = patch.get("nodes").and_then(Value::as_object) else {
            return Ok(false);
        };
        let mut applied = false;
        for (node_id, node_value) in nodes {
            let confidence = confidence_for_node(&confidence, node_id);
            if confidence < HIGH_CONFIDENCE_THRESHOLD as f64 {
                continue;
            }
            let node: NodeState = serde_json::from_value(node_value.clone()).map_err(|error| {
                AdmError::new(format!("invalid partial node state for {node_id}: {error}"))
            })?;
            state.nodes.insert(node_id.clone(), node);
            applied = true;
        }
        if applied {
            *state = self.engine.normalize_state(state.clone());
        }
        Ok(applied)
    }
}

impl AiConfigService {
    pub fn new(root: impl AsRef<Path>) -> AdmResult<Self> {
        Ok(Self {
            project_root: ProjectRoot::new(root)?,
        })
    }

    pub fn load_or_default(&self) -> AdmResult<AiConfig> {
        adm_new_config::load_ai_config_contract(&self.project_root.path().join(AI_CONFIG_PATH))
    }

    pub fn load_redacted(&self) -> AdmResult<AiConfig> {
        self.load_or_default().map(redact_ai_config_for_view)
    }

    pub fn save(&self, config: &AiConfig) -> AdmResult<AiConfigValidationReport> {
        let normalized = self.normalized(config.clone());
        let report = self.validate(&normalized);
        if !report.ok {
            return Ok(report);
        }
        adm_new_config::save_ai_config_contract(
            &normalized,
            &self.project_root.path().join(AI_CONFIG_PATH),
        )?;
        Ok(report)
    }

    pub fn save_redacted(&self, config: &AiConfig) -> AdmResult<AiConfigValidationReport> {
        let existing = self.load_or_default()?;
        let merged = merge_redacted_ai_config(&existing, config);
        self.save(&merged)
    }

    pub fn validate(&self, config: &AiConfig) -> AiConfigValidationReport {
        let report = adm_new_config::validate_contract_ai_config(config);
        let mut errors = report.errors;
        validate_config_urls(config, &mut errors);
        AiConfigValidationReport {
            ok: errors.is_empty(),
            errors,
            warnings: report.warnings,
        }
    }

    pub fn normalized(&self, config: AiConfig) -> AiConfig {
        adm_new_config::normalize_contract_ai_config(config)
    }

    pub fn descriptor_views(&self) -> Vec<AiConfigDescriptorView> {
        adm_new_config::ai_config_descriptor_views()
    }

    pub fn preview_resolution(
        &self,
        config: &AiConfig,
        category_id: &str,
    ) -> AdmResult<AiResolutionView> {
        let existing = self.load_or_default()?;
        let normalized = self.normalized(merge_redacted_ai_config(&existing, config));
        resolution::resolve_active_ai_target_by_category_id(&normalized, category_id)
            .map(|target| target.view())
    }

    pub fn probe_cli(&self, config: &AiConfig, category_id: &str) -> AdmResult<AiCliProbeView> {
        let existing = self.load_or_default()?;
        let normalized = self.normalized(merge_redacted_ai_config(&existing, config));
        resolution::probe_active_ai_cli_by_category_id(&normalized, category_id)
    }

    pub fn probe_api(
        &self,
        config: &AiConfig,
        category_id: &str,
    ) -> AdmResult<api_probe::AiApiProbeView> {
        let existing = self.load_or_default()?;
        let normalized = self.normalized(merge_redacted_ai_config(&existing, config));
        api_probe::probe_active_ai_api_by_category_id(&normalized, category_id)
    }

    pub fn active_completion_entry(&self, config: &AiConfig) -> AdmResult<ApiEntry> {
        active_entry(&config.completion, "completion").cloned()
    }

    pub fn completion_adapter_spec(&self, config: &AiConfig) -> AdmResult<CompletionAdapterSpec> {
        let entry = self.active_completion_entry(config)?;
        Ok(CompletionAdapterSpec {
            entry_id: entry.id.clone(),
            config_type: entry.config_type.clone(),
            adapter_kind: adapter_kind_for_completion(&entry.config_type)?,
            api_url: if config_url_is_safe(&entry.api_url) {
                entry.api_url.clone()
            } else {
                SECRET_UNCHANGED_MASK.to_string()
            },
            has_api_key: !entry.api_key.is_empty(),
        })
    }
}

pub fn redact_ai_config_for_view(mut config: AiConfig) -> AiConfig {
    for category in [&mut config.dev, &mut config.image, &mut config.completion] {
        for entry in &mut category.entries {
            if !entry.api_key.trim().is_empty() {
                entry.api_key = SECRET_UNCHANGED_MASK.to_string();
            }
            if !config_url_is_safe(&entry.api_url) {
                entry.api_url = SECRET_UNCHANGED_MASK.to_string();
            }
            redact_sensitive_json(&mut entry.extra_json, None);
        }
    }
    for profile in &mut config.profiles {
        redact_sensitive_json(&mut profile.llm, None);
        redact_sensitive_json(&mut profile.image, None);
        for (key, value) in &mut profile.metadata {
            redact_sensitive_json(value, Some(key));
        }
    }
    config
}

pub fn merge_redacted_ai_config(existing: &AiConfig, incoming: &AiConfig) -> AiConfig {
    let mut merged = incoming.clone();
    for (merged_category, existing_category) in [
        (&mut merged.dev, &existing.dev),
        (&mut merged.image, &existing.image),
        (&mut merged.completion, &existing.completion),
    ] {
        for entry in &mut merged_category.entries {
            let Some(existing_entry) = existing_category
                .entries
                .iter()
                .find(|candidate| candidate.id == entry.id)
            else {
                if entry.api_key == SECRET_UNCHANGED_MASK {
                    entry.api_key.clear();
                }
                if entry.api_url == SECRET_UNCHANGED_MASK {
                    entry.api_url.clear();
                }
                clear_or_masked_sensitive_json(&mut entry.extra_json, None);
                continue;
            };

            let provider_identity_unchanged = normalized_config_identity(&entry.config_type)
                == normalized_config_identity(&existing_entry.config_type)
                && config_identity_compatible(&entry.extra_json, &existing_entry.extra_json);
            if entry.api_url == SECRET_UNCHANGED_MASK {
                if provider_identity_unchanged {
                    entry.api_url = existing_entry.api_url.clone();
                } else {
                    entry.api_url.clear();
                }
            }
            let secret_identity_unchanged = provider_identity_unchanged
                && config_url_origin_unchanged(&existing_entry.api_url, &entry.api_url);
            if entry.api_key == SECRET_UNCHANGED_MASK {
                if secret_identity_unchanged {
                    entry.api_key = existing_entry.api_key.clone();
                } else {
                    entry.api_key.clear();
                }
            }
            merge_sensitive_json(
                &mut entry.extra_json,
                &existing_entry.extra_json,
                None,
                secret_identity_unchanged,
            );
        }
    }
    for profile in &mut merged.profiles {
        let Some(existing_profile) = existing
            .profiles
            .iter()
            .find(|candidate| candidate.id == profile.id)
        else {
            clear_or_masked_sensitive_json(&mut profile.llm, None);
            clear_or_masked_sensitive_json(&mut profile.image, None);
            for (key, value) in &mut profile.metadata {
                clear_or_masked_sensitive_json(value, Some(key));
            }
            continue;
        };
        let profile_identity_unchanged = normalized_config_identity(&profile.adapter)
            == normalized_config_identity(&existing_profile.adapter);
        let llm_identity_unchanged = profile_identity_unchanged
            && config_identity_compatible(&profile.llm, &existing_profile.llm);
        let image_identity_unchanged = profile_identity_unchanged
            && config_identity_compatible(&profile.image, &existing_profile.image);
        let metadata_identity_unchanged = profile_identity_unchanged
            && config_identity_compatible(
                &metadata_as_json(&profile.metadata),
                &metadata_as_json(&existing_profile.metadata),
            );
        merge_sensitive_json(
            &mut profile.llm,
            &existing_profile.llm,
            None,
            llm_identity_unchanged,
        );
        merge_sensitive_json(
            &mut profile.image,
            &existing_profile.image,
            None,
            image_identity_unchanged,
        );
        merge_sensitive_metadata(
            &mut profile.metadata,
            &existing_profile.metadata,
            metadata_identity_unchanged,
        );
    }
    merged
}

fn redact_sensitive_json(value: &mut Value, field_name: Option<&str>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                redact_sensitive_json(value, Some(key));
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_sensitive_json(item, field_name);
            }
        }
        Value::String(text)
            if !text.trim().is_empty()
                && field_name.is_some_and(|field| {
                    is_sensitive_config_key(field)
                        || (is_url_config_key(field) && !config_url_value_is_safe(field, text))
                }) =>
        {
            *text = SECRET_UNCHANGED_MASK.to_string();
        }
        _ => {}
    }
}

fn merge_sensitive_json(
    incoming: &mut Value,
    existing: &Value,
    field_name: Option<&str>,
    allow_mask_restore: bool,
) {
    if let Value::String(text) = incoming
        && text == SECRET_UNCHANGED_MASK
        && field_name
            .is_some_and(|field| is_sensitive_config_key(field) || is_url_config_key(field))
    {
        *text = if allow_mask_restore {
            existing.as_str().unwrap_or_default().to_string()
        } else {
            String::new()
        };
        return;
    }

    match incoming {
        Value::Object(incoming) => {
            let Some(existing) = existing.as_object() else {
                for (key, value) in incoming {
                    clear_or_masked_sensitive_json(value, Some(key));
                }
                return;
            };
            for (key, value) in incoming {
                if let Some(existing_value) = existing.get(key) {
                    merge_sensitive_json(value, existing_value, Some(key), allow_mask_restore);
                } else {
                    clear_or_masked_sensitive_json(value, Some(key));
                }
            }
        }
        Value::Array(incoming) => {
            let Some(existing) = existing.as_array() else {
                for value in incoming {
                    clear_or_masked_sensitive_json(value, field_name);
                }
                return;
            };
            for (index, value) in incoming.iter_mut().enumerate() {
                if let Some(existing_value) = existing.get(index) {
                    merge_sensitive_json(value, existing_value, field_name, allow_mask_restore);
                } else {
                    clear_or_masked_sensitive_json(value, field_name);
                }
            }
        }
        _ => {}
    }
}

fn merge_sensitive_metadata(
    incoming: &mut BTreeMap<String, Value>,
    existing: &BTreeMap<String, Value>,
    allow_mask_restore: bool,
) {
    for (key, value) in incoming {
        if let Some(existing_value) = existing.get(key) {
            merge_sensitive_json(value, existing_value, Some(key), allow_mask_restore);
        } else {
            clear_or_masked_sensitive_json(value, Some(key));
        }
    }
}

fn metadata_as_json(metadata: &BTreeMap<String, Value>) -> Value {
    Value::Object(
        metadata
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    )
}

fn normalized_config_identity(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn config_url_origin_unchanged(existing: &str, incoming: &str) -> bool {
    config_url_origin_identity("api_url", existing)
        == config_url_origin_identity("api_url", incoming)
}

fn config_identity_compatible(incoming: &Value, existing: &Value) -> bool {
    let mut incoming_identity = BTreeMap::new();
    let mut existing_identity = BTreeMap::new();
    collect_config_identity(incoming, Some(existing), None, "$", &mut incoming_identity);
    collect_config_identity(existing, None, None, "$", &mut existing_identity);
    incoming_identity == existing_identity
}

fn collect_config_identity(
    value: &Value,
    existing: Option<&Value>,
    field_name: Option<&str>,
    path: &str,
    output: &mut BTreeMap<String, String>,
) {
    if !matches!(value, Value::Object(_) | Value::Array(_))
        && let Some(field_name) = field_name
        && (is_provider_identity_key(field_name) || is_url_config_key(field_name))
    {
        let effective = if value.as_str() == Some(SECRET_UNCHANGED_MASK) {
            existing.unwrap_or(value)
        } else {
            value
        };
        let identity = if is_url_config_key(field_name) {
            effective
                .as_str()
                .map(|value| config_url_origin_identity(field_name, value))
                .unwrap_or_else(|| format!("invalid:{}", effective))
        } else {
            effective
                .as_str()
                .map(normalized_config_identity)
                .unwrap_or_else(|| effective.to_string())
        };
        output.insert(path.to_string(), identity);
        return;
    }

    match value {
        Value::Object(object) => {
            let existing = existing.and_then(Value::as_object);
            for (key, value) in object {
                let child_path = format!("{path}/{}", json_pointer_component(key));
                collect_config_identity(
                    value,
                    existing.and_then(|object| object.get(key)),
                    Some(key),
                    &child_path,
                    output,
                );
            }
        }
        Value::Array(items) => {
            let existing = existing.and_then(Value::as_array);
            for (index, value) in items.iter().enumerate() {
                let child_path = format!("{path}/{index}");
                collect_config_identity(
                    value,
                    existing.and_then(|items| items.get(index)),
                    field_name,
                    &child_path,
                    output,
                );
            }
        }
        _ => {}
    }
}

fn json_pointer_component(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn is_provider_identity_key(key: &str) -> bool {
    let compact = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();
    matches!(compact.as_str(), "provider" | "configtype" | "adapter")
}

fn config_url_origin_identity(field_name: &str, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return "empty".to_string();
    }
    if let Ok(url) = reqwest::Url::parse(value)
        && matches!(url.scheme(), "http" | "https")
        && let Some(host) = url.host_str()
    {
        return format!(
            "{}://{}:{}",
            url.scheme(),
            host.to_ascii_lowercase(),
            url.port_or_known_default().unwrap_or_default()
        );
    }
    let compact = field_name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();
    if compact.ends_with("endpoint") && !value.contains("://") {
        "relative-endpoint".to_string()
    } else {
        format!("invalid:{value}")
    }
}

fn clear_or_masked_sensitive_json(value: &mut Value, field_name: Option<&str>) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                clear_or_masked_sensitive_json(value, Some(key));
            }
        }
        Value::Array(items) => {
            for item in items {
                clear_or_masked_sensitive_json(item, field_name);
            }
        }
        Value::String(text)
            if field_name.is_some_and(|field| {
                is_sensitive_config_key(field) || is_url_config_key(field)
            }) && text == SECRET_UNCHANGED_MASK =>
        {
            text.clear();
        }
        _ => {}
    }
}

fn is_sensitive_config_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace('-', "_");
    let compact = normalized
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();
    normalized == "api_key"
        || normalized.ends_with("_api_key")
        || normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("authorization")
        || normalized.ends_with("_token")
        || normalized == "token"
        || compact.ends_with("apikey")
        || compact.ends_with("token")
        || compact.ends_with("privatekey")
        || compact.ends_with("credential")
        || compact.ends_with("credentials")
}

fn is_url_config_key(key: &str) -> bool {
    let compact = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    compact.ends_with("url") || compact.ends_with("endpoint")
}

fn config_url_is_safe(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return true;
    }
    let Ok(url) = http_endpoint_policy::validate_http_transport_url(value) else {
        return false;
    };
    url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
}

fn config_url_value_is_safe(field_name: &str, value: &str) -> bool {
    let compact = field_name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect::<String>();
    if compact.ends_with("endpoint") && !value.contains("://") {
        let Ok(base) = reqwest::Url::parse("https://relative-endpoint.invalid/") else {
            return false;
        };
        let Ok(endpoint) = base.join(value.trim()) else {
            return false;
        };
        return endpoint.scheme() == "https"
            && endpoint.host_str() == base.host_str()
            && endpoint.username().is_empty()
            && endpoint.password().is_none()
            && endpoint.query().is_none()
            && endpoint.fragment().is_none();
    }
    config_url_is_safe(value)
}

fn validate_config_urls(config: &AiConfig, errors: &mut Vec<String>) {
    for (category_id, category) in [
        ("dev", &config.dev),
        ("image", &config.image),
        ("completion", &config.completion),
    ] {
        for entry in &category.entries {
            if !entry.api_url.trim().is_empty() && !config_url_is_safe(&entry.api_url) {
                errors.push(format!(
                    "{category_id}.{}: api_url must use HTTPS for remote hosts; HTTP is allowed only for localhost, 127.0.0.0/8, or ::1; credentials, query parameters, and fragments are not allowed",
                    entry.id
                ));
            }
            validate_json_urls(
                &entry.extra_json,
                None,
                &format!("{category_id}.{}", entry.id),
                errors,
            );
        }
    }
    for profile in &config.profiles {
        validate_json_urls(&profile.llm, None, "profile.llm", errors);
        validate_json_urls(&profile.image, None, "profile.image", errors);
        for (key, value) in &profile.metadata {
            validate_json_urls(value, Some(key), "profile.metadata", errors);
        }
    }
}

fn validate_json_urls(
    value: &Value,
    field_name: Option<&str>,
    label: &str,
    errors: &mut Vec<String>,
) {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                validate_json_urls(value, Some(key), label, errors);
            }
        }
        Value::Array(items) => {
            for value in items {
                validate_json_urls(value, field_name, label, errors);
            }
        }
        Value::String(text)
            if field_name.is_some_and(is_url_config_key)
                && !text.trim().is_empty()
                && text != SECRET_UNCHANGED_MASK
                && !config_url_value_is_safe(field_name.unwrap_or_default(), text) =>
        {
            errors.push(format!(
                "{label}: configured URL must use HTTPS for remote hosts; HTTP is allowed only for localhost, 127.0.0.0/8, or ::1; credentials, query parameters, and fragments are not allowed"
            ));
        }
        _ => {}
    }
}

pub trait CompletionAdapter {
    fn generate(&self, task: &ModelTask) -> AdmResult<ModelResult>;
}

#[derive(Debug, Clone)]
pub struct StructuredCompletionService<A> {
    adapter: A,
    max_retries: u32,
}

impl<A> StructuredCompletionService<A>
where
    A: CompletionAdapter,
{
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            max_retries: 1,
        }
    }

    pub fn with_max_retries(adapter: A, max_retries: u32) -> Self {
        Self {
            adapter,
            max_retries,
        }
    }

    pub fn generate_json_contract(&self, schema_name: &str, prompt: &str) -> CompletionJsonResult {
        let total_attempts = self.max_retries + 1;
        let mut errors = Vec::new();
        let mut raw_text = String::new();
        for attempt in 1..=total_attempts {
            let retry_hint = if attempt > 1 {
                "\nReturn only one JSON object matching schema."
            } else {
                ""
            };
            let task = ModelTask {
                task_id: new_stable_id("completion").unwrap_or_else(|_| "completion".to_string()),
                prompt: format!("{prompt}{retry_hint}"),
                input_files: Vec::new(),
                output_files: Vec::new(),
                allowed_write_paths: Vec::new(),
                timeout_seconds: DEFAULT_STRUCTURED_COMPLETION_TIMEOUT_SECONDS,
                sandbox: "read-only".to_string(),
                cwd: String::new(),
            };
            match self.adapter.generate(&task) {
                Ok(result) if result.status == ModelResultStatus::Succeeded => {
                    raw_text = result.text.clone();
                    match extract_json_object(&result.text) {
                        Ok(data) => {
                            return CompletionJsonResult {
                                ok: true,
                                data,
                                raw_text,
                                errors,
                                attempts: attempt,
                                schema_name: schema_name.to_string(),
                            };
                        }
                        Err(error) => errors.push(error.message().to_string()),
                    }
                }
                Ok(result) => {
                    raw_text = result.text;
                    errors.extend(result.errors);
                    if errors.is_empty() {
                        errors.push("completion adapter returned failed status".to_string());
                    }
                }
                Err(error) => errors.push(error.message().to_string()),
            }
        }
        CompletionJsonResult {
            ok: false,
            data: BTreeMap::new(),
            raw_text,
            errors,
            attempts: total_attempts,
            schema_name: schema_name.to_string(),
        }
    }
}

pub fn validate_allowed_outputs(task: &ModelTask) -> AdmResult<()> {
    if task.output_files.is_empty() {
        return Ok(());
    }
    if task.allowed_write_paths.is_empty() {
        return Err(AdmError::new(
            "output_files require at least one allowed_write_paths entry",
        ));
    }
    let cwd = if task.cwd.is_empty() { "." } else { &task.cwd };
    let cwd = Path::new(cwd);
    for output in &task.output_files {
        let output_path = ensure_relative_path(cwd, output)?;
        let allowed = task.allowed_write_paths.iter().any(|allowed| {
            ensure_relative_path(cwd, allowed)
                .map(|allowed_path| output_path.starts_with(allowed_path))
                .unwrap_or(false)
        });
        if !allowed {
            return Err(AdmError::new(format!(
                "output path is outside allowed_write_paths: {output}"
            )));
        }
    }
    Ok(())
}

fn active_entry<'a>(category: &'a ApiCategory, category_name: &str) -> AdmResult<&'a ApiEntry> {
    if category.active_entry_id.is_empty() {
        return Err(AdmError::new(format!(
            "{category_name}.active_entry_id is empty"
        )));
    }
    category
        .entries
        .iter()
        .find(|entry| entry.id == category.active_entry_id)
        .ok_or_else(|| {
            AdmError::new(format!(
                "{category_name}.active_entry_id does not exist: {}",
                category.active_entry_id
            ))
        })
}

fn adapter_kind_for_completion(config_type: &str) -> AdmResult<String> {
    let kind = match config_type {
        "local_codex_completion_cli" => "codex",
        "local_claude_completion_cli" => "claude",
        "openai_completion_api" | "custom_completion_api" => "openai_compatible",
        other => {
            return Err(AdmError::new(format!(
                "unsupported completion config_type: {other}"
            )));
        }
    };
    Ok(kind.to_string())
}

fn extract_json_object(text: &str) -> AdmResult<BTreeMap<String, Value>> {
    for candidate in json_candidates(text) {
        if let Ok(Value::Object(object)) = serde_json::from_str::<Value>(&candidate) {
            return Ok(object.into_iter().collect());
        }
    }
    Err(AdmError::new(
        "completion response did not contain one JSON object",
    ))
}

fn json_candidates(text: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let trimmed = text.trim();
    candidates.push(trimmed.to_string());
    if let Some(fenced) = extract_fenced_json(trimmed) {
        candidates.push(fenced);
    }
    if let Some(object) = extract_braced_object(trimmed) {
        candidates.push(object);
    }
    candidates
}

fn extract_fenced_json(text: &str) -> Option<String> {
    let start = text.find("```")?;
    let rest = &text[start + 3..];
    let rest = rest.strip_prefix("json").unwrap_or(rest).trim_start();
    let end = rest.find("```")?;
    Some(rest[..end].trim().to_string())
}

fn extract_braced_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| text[start..=end].to_string())
}

fn validate_payload_for_schema_mode(
    schema_mode: &AiSchemaMode,
    payload: &AiResponsePayload,
) -> AdmResult<()> {
    if !schema_mode_allows(schema_mode, &payload.mode) {
        return Err(AdmError::new(format!(
            "schema mode {} does not allow response mode {:?}",
            schema_mode.as_str(),
            payload.mode
        )));
    }
    match payload.mode {
        AiResponseMode::QuestionGroup if payload.question_group.is_none() => {
            Err(AdmError::new("question_group payload is required"))
        }
        AiResponseMode::ReadinessCheck if payload.readiness_check.is_none() => {
            Err(AdmError::new("readiness_check payload is required"))
        }
        AiResponseMode::FullProjectOutput if payload.full_project_output.is_none() => {
            Err(AdmError::new("full_project_output payload is required"))
        }
        AiResponseMode::PartialProjectOutput if payload.partial_project_output.is_none() => {
            Err(AdmError::new("partial_project_output payload is required"))
        }
        AiResponseMode::SummaryCorrection if payload.summary.is_none() => {
            Err(AdmError::new("summary payload is required"))
        }
        _ => Ok(()),
    }
}

fn schema_mode_allows(schema_mode: &AiSchemaMode, response_mode: &AiResponseMode) -> bool {
    match schema_mode {
        AiSchemaMode::Turn => matches!(
            response_mode,
            AiResponseMode::QuestionGroup
                | AiResponseMode::Confirmation
                | AiResponseMode::ReadinessCheck
                | AiResponseMode::Maintenance
                | AiResponseMode::Error
        ),
        AiSchemaMode::Readiness => matches!(
            response_mode,
            AiResponseMode::ReadinessCheck | AiResponseMode::Maintenance | AiResponseMode::Error
        ),
        AiSchemaMode::FullOutput => matches!(
            response_mode,
            AiResponseMode::QuestionGroup
                | AiResponseMode::Confirmation
                | AiResponseMode::ReadinessCheck
                | AiResponseMode::FullProjectOutput
                | AiResponseMode::Maintenance
                | AiResponseMode::Error
        ),
        AiSchemaMode::PartialOutput => matches!(
            response_mode,
            AiResponseMode::PartialProjectOutput
                | AiResponseMode::Maintenance
                | AiResponseMode::Error
        ),
        AiSchemaMode::Mapping => matches!(
            response_mode,
            AiResponseMode::Mapping | AiResponseMode::Maintenance | AiResponseMode::Error
        ),
        AiSchemaMode::Summary => matches!(
            response_mode,
            AiResponseMode::SummaryCorrection | AiResponseMode::Maintenance | AiResponseMode::Error
        ),
    }
}

fn parse_full_project_output(value: &Value) -> AdmResult<(ProjectState, Value)> {
    let project_value = value
        .get("projectState")
        .or_else(|| value.get("project_state"))
        .ok_or_else(|| AdmError::new("full_project_output.projectState is required"))?;
    let confidence_value = value
        .get("confidenceMap")
        .or_else(|| value.get("confidence_map"))
        .ok_or_else(|| AdmError::new("full_project_output.confidenceMap is required"))?;
    let state = parse_project_state_value(project_value)?;
    let confidence = parse_json_value_or_string(confidence_value)?;
    if !confidence.is_object() {
        return Err(AdmError::new("full output confidenceMap must be an object"));
    }
    Ok((state, confidence))
}

fn parse_project_state_value(value: &Value) -> AdmResult<ProjectState> {
    let value = parse_json_value_or_string(value)?;
    serde_json::from_value(value)
        .map_err(|error| AdmError::new(format!("invalid full project state: {error}")))
}

fn parse_json_value_or_string(value: &Value) -> AdmResult<Value> {
    match value {
        Value::String(text) => serde_json::from_str(text)
            .map_err(|error| AdmError::new(format!("invalid embedded JSON string: {error}"))),
        other => Ok(other.clone()),
    }
}

fn confidence_for_node(confidence: &Value, node_id: &str) -> f64 {
    confidence
        .get("nodes")
        .and_then(|nodes| nodes.get(node_id))
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}

fn validate_partial_output_domains(
    output: &PartialProjectOutput,
    node_domains: &BTreeMap<String, String>,
) -> AdmResult<()> {
    if output.domain_ids.is_empty() {
        return Err(AdmError::new(
            "partialProjectOutput.domainIds cannot be empty",
        ));
    }
    let patch: Value = serde_json::from_str(&output.project_state_patch_json)
        .map_err(|error| AdmError::new(format!("invalid partial project patch JSON: {error}")))?;
    let confidence: Value = serde_json::from_str(&output.confidence_map_json)
        .map_err(|error| AdmError::new(format!("invalid partial confidence map JSON: {error}")))?;
    if !confidence
        .as_object()
        .map(|object| object.contains_key("nodes") || object.contains_key("groups"))
        .unwrap_or(false)
    {
        return Err(AdmError::new(
            "partial confidenceMapJson must contain nodes or groups",
        ));
    }
    if let Some(nodes) = patch.get("nodes").and_then(Value::as_object) {
        for node_id in nodes.keys() {
            let Some(domain_id) = node_domains.get(node_id) else {
                return Err(AdmError::new(format!(
                    "partial output references unknown node: {node_id}"
                )));
            };
            if !output.domain_ids.iter().any(|allowed| allowed == domain_id) {
                return Err(AdmError::new(format!(
                    "partial output node {node_id} is outside allowed domains"
                )));
            }
        }
    }
    Ok(())
}

fn question_text(question_group: Option<&Value>, assistant_message: &str) -> String {
    question_group
        .and_then(|value| {
            value
                .get("questionText")
                .or_else(|| value.get("question_text"))
                .and_then(Value::as_str)
        })
        .unwrap_or(assistant_message)
        .to_string()
}

fn timestamp() -> String {
    format!("unix:{}", unix_timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::ai::AI_CONFIG_SCHEMA_VERSION;
    use adm_new_design::{DesignChecklistItemSpec, DesignOptionGroupSpec};
    use std::cell::RefCell;

    #[test]
    fn crate_reports_ready() {
        assert!(crate_ready());
        assert_eq!(CRATE_NAME, "adm-new-ai");
    }

    #[test]
    fn ai_config_service_saves_normalized_v3_config_and_active_profile() {
        let root = temp_root("config_save");
        let service = AiConfigService::new(&root).unwrap();
        let config = valid_config();

        let report = service.save(&config).unwrap();
        assert!(report.ok, "{:?}", report.errors);
        let loaded = service.load_or_default().unwrap();
        assert_eq!(loaded.schema_version, AI_CONFIG_SCHEMA_VERSION);
        assert_eq!(loaded.dev.category_id, "dev");
        assert_eq!(loaded.active_profile_id, "dev_codex");
        assert!(root.join(AI_CONFIG_PATH).exists());
        cleanup(root);
    }

    #[test]
    fn redacted_ai_config_roundtrip_covers_dev_image_completion_and_profiles() {
        let root = temp_root("config_redacted");
        let service = AiConfigService::new(&root).unwrap();
        let mut config = valid_config();
        config.dev = ApiCategory {
            category_id: "dev".to_string(),
            active_entry_id: "dev-api".to_string(),
            entries: vec![ApiEntry {
                id: "dev-api".to_string(),
                label: "Development API".to_string(),
                config_type: "openai_dev_api".to_string(),
                api_url: "https://dev.example.test/v1".to_string(),
                api_key: "dev-private-value".to_string(),
                extra_json: serde_json::json!({
                    "model": "gpt-dev-test",
                    "apiKey": "dev-extra-private-value"
                }),
                ..ApiEntry::default()
            }],
        };
        config.image = ApiCategory {
            category_id: "image".to_string(),
            active_entry_id: "image-api".to_string(),
            entries: vec![ApiEntry {
                id: "image-api".to_string(),
                label: "Image API".to_string(),
                config_type: "openai_image_api".to_string(),
                api_url: "https://image.example.test/v1".to_string(),
                api_key: "image-private-value".to_string(),
                extra_json: serde_json::json!({
                    "model": "gpt-image-test",
                    "accessToken": "image-extra-private-value"
                }),
                ..ApiEntry::default()
            }],
        };
        config.completion.entries[0].api_key = "sk-private-value".to_string();
        config.completion.entries[0].extra_json = serde_json::json!({
            "model": "gpt-test",
            "nested": {"access_token": "token-private-value"}
        });
        assert!(service.save(&config).unwrap().ok);

        let mut redacted = service.load_redacted().unwrap();
        let serialized = serde_json::to_string(&redacted).unwrap();
        for secret in [
            "dev-private-value",
            "dev-extra-private-value",
            "image-private-value",
            "image-extra-private-value",
            "sk-private-value",
            "token-private-value",
        ] {
            assert!(
                !serialized.contains(secret),
                "secret escaped redaction: {secret}"
            );
        }
        assert!(serialized.contains(SECRET_UNCHANGED_MASK));
        assert_eq!(redacted.profiles[0].llm["api_key"], SECRET_UNCHANGED_MASK);
        assert_eq!(redacted.profiles[0].image["api_key"], SECRET_UNCHANGED_MASK);

        redacted.dev.entries[0].label = "renamed dev".to_string();
        redacted.image.entries[0].label = "renamed image".to_string();
        redacted.completion.entries[0].label = "renamed".to_string();
        assert!(service.save_redacted(&redacted).unwrap().ok);
        let restored = service.load_or_default().unwrap();
        assert_eq!(restored.dev.entries[0].api_key, "dev-private-value");
        assert_eq!(
            restored.dev.entries[0].extra_json["apiKey"],
            "dev-extra-private-value"
        );
        assert_eq!(restored.image.entries[0].api_key, "image-private-value");
        assert_eq!(
            restored.image.entries[0].extra_json["accessToken"],
            "image-extra-private-value"
        );
        assert_eq!(restored.completion.entries[0].api_key, "sk-private-value");
        assert_eq!(
            restored.completion.entries[0].extra_json["nested"]["access_token"],
            "token-private-value"
        );
        assert_eq!(restored.completion.entries[0].label, "renamed");
        assert_eq!(restored.profiles[0].llm["api_key"], "dev-private-value");
        assert_eq!(restored.profiles[0].image["api_key"], "image-private-value");
        cleanup(root);
    }

    #[test]
    fn redaction_and_mask_merge_cover_camel_case_profile_metadata() {
        let existing = AiConfig {
            profiles: vec![adm_new_contracts::ai::AiProfile {
                id: "profile".to_string(),
                name: "Profile".to_string(),
                adapter: "openai".to_string(),
                llm: serde_json::json!({"apiKey": "profile-llm-private-value"}),
                image: serde_json::json!({"accessToken": "profile-image-private-value"}),
                metadata: BTreeMap::from([
                    (
                        "refreshToken".to_string(),
                        Value::String("profile-metadata-private-value".to_string()),
                    ),
                    (
                        "nested".to_string(),
                        serde_json::json!({"privateKey": "profile-private-key"}),
                    ),
                ]),
            }],
            ..AiConfig::default()
        };

        let redacted = redact_ai_config_for_view(existing.clone());
        let serialized = serde_json::to_string(&redacted).unwrap();
        for secret in [
            "profile-llm-private-value",
            "profile-image-private-value",
            "profile-metadata-private-value",
            "profile-private-key",
        ] {
            assert!(!serialized.contains(secret));
        }

        let merged = merge_redacted_ai_config(&existing, &redacted);
        assert_eq!(merged, existing);
    }

    #[test]
    fn masked_entry_secrets_are_bound_to_config_provider_and_url_origin() {
        let existing = AiConfig {
            completion: ApiCategory {
                category_id: "completion".to_string(),
                active_entry_id: "completion-api".to_string(),
                entries: vec![ApiEntry {
                    id: "completion-api".to_string(),
                    config_type: "openai_completion_api".to_string(),
                    api_url: "https://api.example.test/v1".to_string(),
                    api_key: "entry-private-value".to_string(),
                    extra_json: serde_json::json!({
                        "provider": "openai",
                        "base_url": "https://api.example.test/v1",
                        "api_key": "nested-private-value"
                    }),
                    ..ApiEntry::default()
                }],
            },
            ..AiConfig::default()
        };

        let mut changed_origin = redact_ai_config_for_view(existing.clone());
        changed_origin.completion.entries[0].api_url =
            "https://collector.example.test/v1".to_string();
        let changed_origin = merge_redacted_ai_config(&existing, &changed_origin);
        assert!(changed_origin.completion.entries[0].api_key.is_empty());
        assert!(
            changed_origin.completion.entries[0].extra_json["api_key"]
                .as_str()
                .unwrap()
                .is_empty()
        );

        let mut changed_provider = redact_ai_config_for_view(existing.clone());
        changed_provider.completion.entries[0].extra_json["provider"] =
            Value::String("relay".to_string());
        let changed_provider = merge_redacted_ai_config(&existing, &changed_provider);
        assert!(changed_provider.completion.entries[0].api_key.is_empty());
        assert!(
            changed_provider.completion.entries[0].extra_json["api_key"]
                .as_str()
                .unwrap()
                .is_empty()
        );

        let mut changed_config_type = redact_ai_config_for_view(existing.clone());
        changed_config_type.completion.entries[0].config_type = "custom_completion_api".to_string();
        let changed_config_type = merge_redacted_ai_config(&existing, &changed_config_type);
        assert!(changed_config_type.completion.entries[0].api_key.is_empty());
        assert!(
            changed_config_type.completion.entries[0].extra_json["api_key"]
                .as_str()
                .unwrap()
                .is_empty()
        );

        let mut changed_path = redact_ai_config_for_view(existing.clone());
        changed_path.completion.entries[0].api_url =
            "https://api.example.test/v2/responses".to_string();
        changed_path.completion.entries[0].extra_json["base_url"] =
            Value::String("https://api.example.test/v2".to_string());
        let changed_path = merge_redacted_ai_config(&existing, &changed_path);
        assert_eq!(
            changed_path.completion.entries[0].api_key,
            "entry-private-value"
        );
        assert_eq!(
            changed_path.completion.entries[0].extra_json["api_key"],
            "nested-private-value"
        );
    }

    #[test]
    fn nested_base_url_change_cannot_rebind_entry_or_extra_json_secrets() {
        let existing = AiConfig {
            image: ApiCategory {
                category_id: "image".to_string(),
                active_entry_id: "image-api".to_string(),
                entries: vec![ApiEntry {
                    id: "image-api".to_string(),
                    config_type: "openai_image_api".to_string(),
                    api_url: "https://api.example.test/v1".to_string(),
                    api_key: "entry-image-private-value".to_string(),
                    extra_json: serde_json::json!({
                        "provider": "openai",
                        "base_url": "https://api.example.test/v1",
                        "auth": {
                            "apiKey": "nested-image-private-value",
                            "Authorization": "Bearer nested-private-value"
                        }
                    }),
                    ..ApiEntry::default()
                }],
            },
            ..AiConfig::default()
        };
        let mut incoming = redact_ai_config_for_view(existing.clone());
        incoming.image.entries[0].extra_json["base_url"] =
            Value::String("https://collector.example.test/v1".to_string());

        let merged = merge_redacted_ai_config(&existing, &incoming);
        assert!(merged.image.entries[0].api_key.is_empty());
        assert!(
            merged.image.entries[0].extra_json["auth"]["apiKey"]
                .as_str()
                .unwrap()
                .is_empty()
        );
        assert!(
            merged.image.entries[0].extra_json["auth"]["Authorization"]
                .as_str()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn masked_profile_secrets_are_bound_to_section_origin_and_profile_adapter() {
        let existing = AiConfig {
            profiles: vec![adm_new_contracts::ai::AiProfile {
                id: "profile".to_string(),
                name: "Profile".to_string(),
                adapter: "openai".to_string(),
                llm: serde_json::json!({
                    "provider": "openai",
                    "base_url": "https://llm.example.test/v1",
                    "api_key": "llm-private-value"
                }),
                image: serde_json::json!({
                    "provider": "openai",
                    "base_url": "https://image.example.test/v1",
                    "api_key": "image-private-value"
                }),
                metadata: BTreeMap::new(),
            }],
            ..AiConfig::default()
        };

        let mut changed_llm_origin = redact_ai_config_for_view(existing.clone());
        changed_llm_origin.profiles[0].llm["base_url"] =
            Value::String("https://collector.example.test/v1".to_string());
        let changed_llm_origin = merge_redacted_ai_config(&existing, &changed_llm_origin);
        assert!(
            changed_llm_origin.profiles[0].llm["api_key"]
                .as_str()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            changed_llm_origin.profiles[0].image["api_key"],
            "image-private-value"
        );

        let mut changed_adapter = redact_ai_config_for_view(existing.clone());
        changed_adapter.profiles[0].adapter = "relay".to_string();
        let changed_adapter = merge_redacted_ai_config(&existing, &changed_adapter);
        assert!(
            changed_adapter.profiles[0].llm["api_key"]
                .as_str()
                .unwrap()
                .is_empty()
        );
        assert!(
            changed_adapter.profiles[0].image["api_key"]
                .as_str()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn credential_bearing_config_urls_are_hidden_from_views_and_rejected_on_save() {
        let root = temp_root("unsafe-config-url");
        let service = AiConfigService::new(&root).unwrap();
        let mut config = valid_config();
        config.profiles = vec![adm_new_contracts::ai::AiProfile {
            id: "unsafe-profile".to_string(),
            name: "Unsafe profile".to_string(),
            adapter: "openai".to_string(),
            llm: serde_json::json!({"base_url": "https://api.example.test/v1"}),
            image: serde_json::json!({}),
            metadata: BTreeMap::new(),
        }];
        config.completion.entries[0].api_url =
            "https://user:password@example.test/v1?api_key=url-secret".to_string();
        config.profiles[0].llm["base_url"] =
            Value::String("https://user:password@example.test/v1#private".to_string());

        let redacted = redact_ai_config_for_view(config.clone());
        let serialized = serde_json::to_string(&redacted).unwrap();
        assert!(!serialized.contains("password"));
        assert!(!serialized.contains("url-secret"));
        assert!(!serialized.contains("private"));
        assert_eq!(
            redacted.completion.entries[0].api_url,
            SECRET_UNCHANGED_MASK
        );
        assert_eq!(redacted.profiles[0].llm["base_url"], SECRET_UNCHANGED_MASK);
        assert_eq!(merge_redacted_ai_config(&config, &redacted), config);

        let report = service.validate(&config);
        assert!(!report.ok);
        assert!(report.errors.iter().any(|error| error.contains("api_url")));
        cleanup(root);
    }

    #[test]
    fn relative_endpoints_reject_query_fragment_and_authority_rebinding() {
        assert!(config_url_value_is_safe("endpoint", "responses"));
        assert!(config_url_value_is_safe(
            "image_endpoint",
            "/v1/images/generations"
        ));
        assert!(!config_url_value_is_safe(
            "endpoint",
            "responses?api_key=secret"
        ));
        assert!(!config_url_value_is_safe("endpoint", "responses#private"));
        assert!(!config_url_value_is_safe("endpoint", "//evil.example/v1"));

        let root = temp_root("unsafe-relative-endpoint");
        let service = AiConfigService::new(&root).unwrap();
        let mut config = valid_config();
        config.completion.entries[0].extra_json = serde_json::json!({
            "endpoint": "responses?api_key=relative-secret"
        });
        let redacted = redact_ai_config_for_view(config.clone());
        assert_eq!(
            redacted.completion.entries[0].extra_json["endpoint"],
            SECRET_UNCHANGED_MASK
        );
        let report = service.validate(&config);
        assert!(!report.ok);
        assert!(report.errors.iter().any(|error| error.contains("query")));
        cleanup(root);
    }

    #[test]
    fn config_urls_require_https_except_for_explicit_loopback_http() {
        for url in [
            "https://api.example.test/v1",
            "http://localhost:11434/v1",
            "http://127.42.0.9:11434/v1",
            "http://[::1]:11434/v1",
        ] {
            assert!(config_url_is_safe(url), "{url}");
        }
        for url in [
            "http://api.example.test/v1",
            "http://10.0.0.8/v1",
            "http://192.168.1.20/v1",
            "http://0.0.0.0/v1",
            "http://[::]/v1",
        ] {
            assert!(!config_url_is_safe(url), "{url}");
        }

        let root = temp_root("remote-http-config");
        let service = AiConfigService::new(&root).unwrap();
        let mut config = valid_config();
        config.completion.entries[0].api_url = "http://api.example.test/v1".to_string();
        config.completion.entries[0].extra_json = serde_json::json!({
            "model": "gpt-test",
            "base_url": "http://relay.example.test/v1",
            "endpoint": "http://images.example.test/v1/responses"
        });
        config.profiles = vec![adm_new_contracts::ai::AiProfile {
            id: "remote-http".to_string(),
            name: "Remote HTTP".to_string(),
            adapter: "openai".to_string(),
            llm: serde_json::json!({"base_url": "http://llm.example.test/v1"}),
            image: serde_json::json!({"endpoint": "http://image.example.test/v1"}),
            metadata: BTreeMap::from([(
                "status_url".to_string(),
                serde_json::json!("http://status.example.test/health"),
            )]),
        }];

        let report = service.validate(&config);
        assert!(!report.ok);
        for label in [
            "completion.completion_openai",
            "profile.llm",
            "profile.image",
            "profile.metadata",
        ] {
            assert!(
                report.errors.iter().any(|error| error.contains(label)),
                "missing validation error for {label}: {:?}",
                report.errors
            );
        }
        assert!(
            report
                .errors
                .iter()
                .all(|error| !error.contains("api.example.test"))
        );
        let save_report = service.save(&config).unwrap();
        assert!(!save_report.ok);
        assert!(!root.join(AI_CONFIG_PATH).exists());
        cleanup(root);
    }

    #[test]
    fn ai_config_validation_rejects_unsupported_duplicate_and_missing_api_secret() {
        let root = temp_root("config_invalid");
        let service = AiConfigService::new(&root).unwrap();
        let mut config = AiConfig::default();
        config.completion = ApiCategory {
            category_id: "completion".to_string(),
            entries: vec![
                ApiEntry {
                    id: "same".to_string(),
                    config_type: "openai_completion_api".to_string(),
                    api_url: String::new(),
                    api_key: String::new(),
                    ..ApiEntry::default()
                },
                ApiEntry {
                    id: "same".to_string(),
                    config_type: "not_supported".to_string(),
                    ..ApiEntry::default()
                },
                ApiEntry {
                    id: "bad_active".to_string(),
                    config_type: "not_supported".to_string(),
                    ..ApiEntry::default()
                },
            ],
            active_entry_id: "bad_active".to_string(),
        };
        let report = service.validate(&config);
        assert!(!report.ok);
        assert!(report.errors.iter().any(|item| item.contains("duplicated")));
        assert!(
            report
                .errors
                .iter()
                .any(|item| item.contains("unsupported"))
        );
        config.completion.active_entry_id = "same".to_string();
        let missing_report = service.validate(&config);
        assert!(
            missing_report
                .errors
                .iter()
                .any(|item| item.contains("missing api_url"))
        );
        assert!(
            missing_report
                .errors
                .iter()
                .any(|item| item.contains("missing api_key"))
        );
        cleanup(root);
    }

    #[test]
    fn ai_config_completion_adapter_spec_redacts_api_key_presence() {
        let root = temp_root("adapter_spec");
        let service = AiConfigService::new(&root).unwrap();
        let spec = service.completion_adapter_spec(&valid_config()).unwrap();
        assert_eq!(spec.adapter_kind, "openai_compatible");
        assert_eq!(spec.entry_id, "completion_openai");
        assert!(spec.has_api_key);
    }

    #[test]
    fn completion_service_accepts_pure_fenced_and_embedded_json_with_retry_hint() {
        let adapter = FakeCompletionAdapter::new(vec![
            ModelResult {
                task_id: "first".to_string(),
                status: ModelResultStatus::Succeeded,
                text: "not json".to_string(),
                errors: Vec::new(),
            },
            ModelResult {
                task_id: "second".to_string(),
                status: ModelResultStatus::Succeeded,
                text: "```json\n{\"task\":\"patch\"}\n```".to_string(),
                errors: Vec::new(),
            },
        ]);
        let service = StructuredCompletionService::new(adapter);
        let result = service.generate_json_contract("patch", "Analyze");
        assert!(result.ok, "{:?}", result.errors);
        assert_eq!(result.attempts, 2);
        assert_eq!(result.data["task"], Value::String("patch".to_string()));
    }

    #[test]
    fn completion_service_reports_failure_after_retries() {
        let adapter = FakeCompletionAdapter::new(vec![
            ModelResult {
                task_id: "first".to_string(),
                status: ModelResultStatus::Failed,
                text: String::new(),
                errors: vec!["adapter down".to_string()],
            },
            ModelResult {
                task_id: "second".to_string(),
                status: ModelResultStatus::Succeeded,
                text: "[]".to_string(),
                errors: Vec::new(),
            },
        ]);
        let service = StructuredCompletionService::new(adapter);
        let result = service.generate_json_contract("sdk", "Extract");
        assert!(!result.ok);
        assert_eq!(result.attempts, 2);
        assert!(
            result
                .errors
                .iter()
                .any(|item| item.contains("adapter down"))
        );
    }

    #[test]
    fn completion_service_assigns_a_finite_default_timeout() {
        #[derive(Debug)]
        struct TimeoutAssertingAdapter;

        impl CompletionAdapter for TimeoutAssertingAdapter {
            fn generate(&self, task: &ModelTask) -> AdmResult<ModelResult> {
                assert_eq!(
                    task.timeout_seconds,
                    DEFAULT_STRUCTURED_COMPLETION_TIMEOUT_SECONDS
                );
                assert!(task.timeout_seconds > 0);
                Ok(ModelResult {
                    task_id: task.task_id.clone(),
                    status: ModelResultStatus::Succeeded,
                    text: "{\"ok\":true}".to_string(),
                    errors: Vec::new(),
                })
            }
        }

        let result = StructuredCompletionService::new(TimeoutAssertingAdapter)
            .generate_json_contract("timeout", "Check timeout");
        assert!(result.ok);
        assert_eq!(result.attempts, 1);
    }

    #[test]
    fn completion_output_guard_rejects_paths_outside_allowed_roots() {
        let valid = ModelTask {
            task_id: "task".to_string(),
            prompt: String::new(),
            input_files: Vec::new(),
            output_files: vec!["outputs/result.json".to_string()],
            allowed_write_paths: vec!["outputs".to_string()],
            timeout_seconds: 0,
            sandbox: "workspace-write".to_string(),
            cwd: ".".to_string(),
        };
        assert!(validate_allowed_outputs(&valid).is_ok());
        let invalid = ModelTask {
            output_files: vec!["../outside/result.json".to_string()],
            ..valid
        };
        assert!(validate_allowed_outputs(&invalid).is_err());
    }

    #[test]
    fn interview_rejects_invalid_json_and_schema_mode_mismatch() {
        let service = interview_service();
        let mut state = ProjectState::empty();
        let invalid = service
            .handle_payload_json(&mut state, AiSchemaMode::Turn, "{not-json")
            .unwrap_err();
        assert!(invalid.message().contains("invalid AI payload JSON"));

        let payload = serde_json::json!({
            "schemaVersion": "1.0",
            "mode": "full_project_output",
            "assistantMessage": "ready",
            "fullProjectOutput": {
                "projectState": candidate_state("New", true),
                "confidenceMap": {"nodes": {"mechanics": 0.9}}
            }
        });
        let mismatch = service
            .handle_payload_json(&mut state, AiSchemaMode::Turn, &payload.to_string())
            .unwrap_err();
        assert!(mismatch.message().contains("does not allow response mode"));
    }

    #[test]
    fn interview_low_confidence_full_output_does_not_write_project_state() {
        let service = interview_service();
        let mut state = candidate_state("Old", false);
        let payload = serde_json::json!({
            "schemaVersion": "1.0",
            "mode": "full_project_output",
            "assistantMessage": "candidate ready",
            "fullProjectOutput": {
                "projectState": candidate_state("New", true),
                "confidenceMap": {"nodes": {"mechanics": 0.4}}
            }
        });

        let report = service
            .handle_payload_json(&mut state, AiSchemaMode::FullOutput, &payload.to_string())
            .unwrap();
        assert!(!report.applied_project_state);
        assert_eq!(state.nodes["mechanics"].design_note, "Old");
        assert!(!state.nodes["mechanics"].checklist["core_loop"]);
        assert!(
            report
                .memory_events
                .iter()
                .any(|event| event == "low_confidence_output_recorded")
        );
    }

    #[test]
    fn interview_high_confidence_full_output_writes_and_archives_state() {
        let service = interview_service();
        let mut state = candidate_state("Old", false);
        let payload = serde_json::json!({
            "schemaVersion": "1.0",
            "mode": "full_project_output",
            "assistantMessage": "candidate ready",
            "fullProjectOutput": {
                "projectState": candidate_state("New", true),
                "confidenceMap": {"nodes": {"mechanics": 0.9}}
            }
        });

        let report = service
            .handle_payload_json(&mut state, AiSchemaMode::FullOutput, &payload.to_string())
            .unwrap();
        assert!(report.applied_project_state);
        assert_eq!(state.nodes["mechanics"].design_note, "New");
        assert!(state.nodes["mechanics"].checklist["core_loop"]);
        assert_eq!(state.ai_interview.output_history.len(), 1);
        assert_eq!(state.ai_interview.auto_archive_path, report.archive_path);
        assert!(
            state
                .ai_interview
                .framework_memory
                .review_chains
                .values()
                .any(|value| value == "high_confidence_project_state_applied")
        );
    }

    #[test]
    fn interview_partial_output_merges_only_allowed_high_confidence_domain() {
        let service = interview_service();
        let mut state = candidate_state("Old", false);
        let patch = serde_json::json!({
            "nodes": {
                "mechanics": {
                    "designNote": "Partial New",
                    "checklist": {"core_loop": true}
                }
            }
        });
        let partial = PartialProjectOutput {
            domain_ids: vec!["core".to_string()],
            project_state_patch_json: patch.to_string(),
            confidence_map_json: serde_json::json!({"nodes": {"mechanics": 0.85}}).to_string(),
        };

        assert!(
            service
                .merge_partial_project_outputs(&mut state, &[partial])
                .unwrap()
        );
        assert_eq!(state.nodes["mechanics"].design_note, "Partial New");
        assert!(state.nodes["mechanics"].checklist["core_loop"]);
    }

    #[test]
    fn interview_partial_output_rejects_cross_domain_patch() {
        let service = interview_service();
        let mut state = candidate_state("Old", false);
        let partial = PartialProjectOutput {
            domain_ids: vec!["narrative".to_string()],
            project_state_patch_json: serde_json::json!({
                "nodes": {
                    "mechanics": {"designNote": "Wrong Domain"}
                }
            })
            .to_string(),
            confidence_map_json: serde_json::json!({"nodes": {"mechanics": 0.9}}).to_string(),
        };
        let error = service
            .merge_partial_project_outputs(&mut state, &[partial])
            .unwrap_err();
        assert!(error.message().contains("outside allowed domains"));
    }

    #[derive(Debug)]
    struct FakeCompletionAdapter {
        results: RefCell<Vec<ModelResult>>,
    }

    impl FakeCompletionAdapter {
        fn new(mut results: Vec<ModelResult>) -> Self {
            results.reverse();
            Self {
                results: RefCell::new(results),
            }
        }
    }

    impl CompletionAdapter for FakeCompletionAdapter {
        fn generate(&self, task: &ModelTask) -> AdmResult<ModelResult> {
            if task.prompt.contains("Return only one JSON object") {
                assert!(task.sandbox == "read-only");
            }
            self.results
                .borrow_mut()
                .pop()
                .ok_or_else(|| AdmError::new("no fake result left"))
        }
    }

    fn valid_config() -> AiConfig {
        AiConfig {
            dev: ApiCategory {
                category_id: "dev".to_string(),
                entries: vec![ApiEntry {
                    id: "dev_codex".to_string(),
                    label: "Codex".to_string(),
                    config_type: "local_codex_cli".to_string(),
                    codex_toml_path: "codex.toml".to_string(),
                    ..ApiEntry::default()
                }],
                active_entry_id: "dev_codex".to_string(),
            },
            image: ApiCategory {
                category_id: "image".to_string(),
                entries: Vec::new(),
                active_entry_id: String::new(),
            },
            completion: ApiCategory {
                category_id: "completion".to_string(),
                entries: vec![ApiEntry {
                    id: "completion_openai".to_string(),
                    label: "OpenAI".to_string(),
                    config_type: "openai_completion_api".to_string(),
                    api_url: "https://api.example.test/v1".to_string(),
                    api_key: "secret".to_string(),
                    extra_json: serde_json::json!({"model": "gpt-test"}),
                    ..ApiEntry::default()
                }],
                active_entry_id: "completion_openai".to_string(),
            },
            ..AiConfig::default()
        }
    }

    fn interview_service() -> AiInterviewService {
        AiInterviewService::new(vec![
            DesignNodeSpec {
                node_id: "mechanics".to_string(),
                domain_id: "core".to_string(),
                name: "Mechanics".to_string(),
                description: String::new(),
                role_class: String::new(),
                checklist: vec![DesignChecklistItemSpec {
                    item_id: "core_loop".to_string(),
                    label: "Core Loop".to_string(),
                    option_groups: vec![DesignOptionGroupSpec {
                        group_id: "pace".to_string(),
                        selection_mode: "single".to_string(),
                        allow_primary: false,
                        options: vec!["fast".to_string(), "slow".to_string()],
                    }],
                }],
            },
            DesignNodeSpec {
                node_id: "story".to_string(),
                domain_id: "narrative".to_string(),
                name: "Story".to_string(),
                description: String::new(),
                role_class: String::new(),
                checklist: Vec::new(),
            },
        ])
    }

    fn candidate_state(note: &str, checked: bool) -> ProjectState {
        let mut state = ProjectState::empty();
        let mut node = NodeState {
            design_note: note.to_string(),
            ..NodeState::default()
        };
        node.checklist.insert("core_loop".to_string(), checked);
        state.nodes.insert("mechanics".to_string(), node);
        state
    }

    fn temp_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "adm_new_ai_{label}_{}",
            new_stable_id("root").unwrap()
        ))
    }

    fn cleanup(root: std::path::PathBuf) {
        let _ = std::fs::remove_dir_all(root);
    }
}
