#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACT_FAMILY: &str = "ai";
pub const HIGH_CONFIDENCE_THRESHOLD: f32 = 0.75;
pub const AI_CONFIG_SCHEMA_VERSION: u32 = 3;
pub const AI_INTERVIEW_SCHEMA_VERSION: &str = "1.0";
pub const SUMMARY_SCHEMA_VERSION: &str = "1.0";
pub const MDA_STAGES: &[(&str, &str)] = &[
    ("aesthetics", "体验目标"),
    ("dynamics", "玩家动态"),
    ("mechanics", "机制抓手"),
    ("constraints", "边界约束"),
    ("evidence", "验收信号"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiSchemaMode {
    Turn,
    Readiness,
    FullOutput,
    PartialOutput,
    Mapping,
    Summary,
}

impl AiSchemaMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Turn => "turn",
            Self::Readiness => "readiness",
            Self::FullOutput => "full_output",
            Self::PartialOutput => "partial_output",
            Self::Mapping => "mapping",
            Self::Summary => "summary",
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    #[serde(default = "default_ai_config_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub dev: ApiCategory,
    #[serde(default)]
    pub image: ApiCategory,
    #[serde(default)]
    pub completion: ApiCategory,
    #[serde(default)]
    pub active_profile_id: String,
    #[serde(default)]
    pub profiles: Vec<AiProfile>,
}

impl fmt::Debug for AiConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AiConfig")
            .field("schema_version", &self.schema_version)
            .field("dev", &self.dev)
            .field("image", &self.image)
            .field("completion", &self.completion)
            .field("active_profile_id", &self.active_profile_id)
            .field("profile_count", &self.profiles.len())
            .finish()
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            schema_version: AI_CONFIG_SCHEMA_VERSION,
            dev: ApiCategory::new("dev"),
            image: ApiCategory::new("image"),
            completion: ApiCategory::new("completion"),
            active_profile_id: String::new(),
            profiles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCategory {
    #[serde(default)]
    pub category_id: String,
    #[serde(default)]
    pub entries: Vec<ApiEntry>,
    #[serde(default)]
    pub active_entry_id: String,
}

impl ApiCategory {
    pub fn new(category_id: &str) -> Self {
        Self {
            category_id: category_id.to_string(),
            entries: Vec::new(),
            active_entry_id: String::new(),
        }
    }
}

impl Default for ApiCategory {
    fn default() -> Self {
        Self::new("")
    }
}

#[derive(Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEntry {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub config_type: String,
    #[serde(default)]
    pub api_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub extra_json: Value,
    #[serde(default)]
    pub codex_toml_path: String,
    #[serde(default)]
    pub codex_json_path: String,
}

impl fmt::Debug for ApiEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiEntry")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("config_type", &self.config_type)
            .field("api_url_configured", &!self.api_url.trim().is_empty())
            .field("has_api_key", &!self.api_key.trim().is_empty())
            .field("extra_json_kind", &json_value_kind(&self.extra_json))
            .field(
                "codex_toml_path_configured",
                &!self.codex_toml_path.trim().is_empty(),
            )
            .field(
                "codex_json_path_configured",
                &!self.codex_json_path.trim().is_empty(),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProfile {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub adapter: String,
    #[serde(default)]
    pub llm: Value,
    #[serde(default)]
    pub image: Value,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl fmt::Debug for AiProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AiProfile")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("adapter", &self.adapter)
            .field("llm_kind", &json_value_kind(&self.llm))
            .field("image_kind", &json_value_kind(&self.image))
            .field("metadata_keys", &self.metadata.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTask {
    pub task_id: String,
    pub prompt: String,
    #[serde(default)]
    pub input_files: Vec<String>,
    #[serde(default)]
    pub output_files: Vec<String>,
    #[serde(default)]
    pub allowed_write_paths: Vec<String>,
    #[serde(default)]
    pub timeout_seconds: u64,
    #[serde(default = "default_read_only_sandbox")]
    pub sandbox: String,
    #[serde(default)]
    pub cwd: String,
}

impl fmt::Debug for ModelTask {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModelTask")
            .field("task_id", &self.task_id)
            .field("prompt_chars", &self.prompt.chars().count())
            .field("input_file_count", &self.input_files.len())
            .field("output_file_count", &self.output_files.len())
            .field("allowed_write_path_count", &self.allowed_write_paths.len())
            .field("timeout_seconds", &self.timeout_seconds)
            .field("sandbox", &self.sandbox)
            .field("cwd_configured", &!self.cwd.trim().is_empty())
            .finish()
    }
}

fn json_value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelResultStatus {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelResult {
    pub task_id: String,
    pub status: ModelResultStatus,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiResponseMode {
    QuestionGroup,
    Confirmation,
    ReadinessCheck,
    FullProjectOutput,
    PartialProjectOutput,
    Mapping,
    SummaryCorrection,
    Maintenance,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResponsePayload {
    #[serde(default = "default_ai_response_schema_version")]
    pub schema_version: String,
    pub mode: AiResponseMode,
    #[serde(default)]
    pub assistant_message: String,
    #[serde(default)]
    pub route_overview: Option<AiRouteOverview>,
    #[serde(default)]
    pub question_group: Option<Value>,
    #[serde(default)]
    pub readiness_check: Option<Value>,
    #[serde(default)]
    pub full_project_output: Option<Value>,
    #[serde(default)]
    pub partial_project_output: Option<PartialProjectOutput>,
    #[serde(default)]
    pub inferences: Vec<Value>,
    #[serde(default)]
    pub option_differences: Vec<Value>,
    #[serde(default)]
    pub summary: Option<Value>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialProjectOutput {
    #[serde(default)]
    pub domain_ids: Vec<String>,
    #[serde(default)]
    pub project_state_patch_json: String,
    #[serde(default)]
    pub confidence_map_json: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CodexRunResult {
    #[serde(default)]
    pub payload: Value,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub raw_output: String,
    #[serde(default)]
    pub raw_events: Vec<Value>,
    #[serde(default)]
    pub duration_seconds: f64,
    #[serde(default)]
    pub first_event_seconds: Option<f64>,
    #[serde(default)]
    pub response_chars: u64,
    #[serde(default)]
    pub api_profile: String,
    #[serde(default)]
    pub api_model: String,
    #[serde(default)]
    pub api_base_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionJsonResult {
    pub ok: bool,
    #[serde(default)]
    pub data: BTreeMap<String, Value>,
    #[serde(default)]
    pub raw_text: String,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub attempts: u32,
    #[serde(default)]
    pub schema_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiInterviewState {
    #[serde(default = "default_ai_interview_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub codex_session_id: String,
    #[serde(default)]
    pub session_turn_count: u32,
    #[serde(default = "default_idle_status")]
    pub status: String,
    #[serde(default)]
    pub active_turn_id: String,
    #[serde(default)]
    pub run_started_at: String,
    #[serde(default = "default_idle_status")]
    pub backend_stage: String,
    #[serde(default)]
    pub backend_started_at: String,
    #[serde(default)]
    pub last_backend_duration_seconds: f64,
    #[serde(default)]
    pub last_first_event_seconds: Option<f64>,
    #[serde(default)]
    pub question_group_count: u32,
    #[serde(default)]
    pub last_readiness_check_group: u32,
    #[serde(default)]
    pub current_question_text: String,
    #[serde(default)]
    pub current_question_turn_id: String,
    #[serde(default)]
    pub current_question_count: u32,
    #[serde(default)]
    pub awaiting_user_answer: bool,
    #[serde(default)]
    pub interview_archive_id: String,
    #[serde(default)]
    pub auto_archive_path: String,
    #[serde(default)]
    pub last_manual_archive_path: String,
    #[serde(default)]
    pub last_archived_at: String,
    #[serde(default)]
    pub route_overview: AiRouteOverview,
    #[serde(default)]
    pub messages: Vec<Value>,
    #[serde(default)]
    pub summary: AiInterviewSummary,
    #[serde(default)]
    pub inferences: Vec<Value>,
    #[serde(default)]
    pub recent_question_targets: Vec<Value>,
    #[serde(default)]
    pub applicability_scores: BTreeMap<String, Value>,
    #[serde(default)]
    pub framework_memory: FrameworkMemoryState,
    #[serde(default)]
    pub output_history: Vec<Value>,
    #[serde(default)]
    pub option_differences: Vec<Value>,
    #[serde(default)]
    pub last_error: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for AiInterviewState {
    fn default() -> Self {
        Self {
            schema_version: default_ai_interview_schema_version(),
            codex_session_id: String::new(),
            session_turn_count: 0,
            status: default_idle_status(),
            active_turn_id: String::new(),
            run_started_at: String::new(),
            backend_stage: default_idle_status(),
            backend_started_at: String::new(),
            last_backend_duration_seconds: 0.0,
            last_first_event_seconds: None,
            question_group_count: 0,
            last_readiness_check_group: 0,
            current_question_text: String::new(),
            current_question_turn_id: String::new(),
            current_question_count: 0,
            awaiting_user_answer: false,
            interview_archive_id: String::new(),
            auto_archive_path: String::new(),
            last_manual_archive_path: String::new(),
            last_archived_at: String::new(),
            route_overview: AiRouteOverview::default(),
            messages: Vec::new(),
            summary: AiInterviewSummary::default(),
            inferences: Vec::new(),
            recent_question_targets: Vec::new(),
            applicability_scores: BTreeMap::new(),
            framework_memory: FrameworkMemoryState::default(),
            output_history: Vec::new(),
            option_differences: Vec::new(),
            last_error: String::new(),
            updated_at: String::new(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRouteOverview {
    #[serde(default = "default_current_mda_stage")]
    pub current_mda_stage: String,
    #[serde(default)]
    pub expected_domains: Vec<String>,
    #[serde(default)]
    pub completed_nodes: Vec<String>,
    #[serde(default)]
    pub clarification_targets: Vec<Value>,
    #[serde(default)]
    pub low_applicability_candidates: Vec<Value>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for AiRouteOverview {
    fn default() -> Self {
        Self {
            current_mda_stage: default_current_mda_stage(),
            expected_domains: Vec::new(),
            completed_nodes: Vec::new(),
            clarification_targets: Vec::new(),
            low_applicability_candidates: Vec::new(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiInterviewSummary {
    #[serde(default)]
    pub v1: ConversationSummaryV1,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for AiInterviewSummary {
    fn default() -> Self {
        Self {
            v1: ConversationSummaryV1::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummaryV1 {
    #[serde(default = "default_summary_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub confirmed_intent: Vec<Value>,
    #[serde(default)]
    pub open_questions: Vec<Value>,
    #[serde(default)]
    pub rejected_assumptions: Vec<Value>,
    #[serde(default)]
    pub node_notes: BTreeMap<String, Value>,
    #[serde(default)]
    pub last_user_corrections: Vec<Value>,
    #[serde(default = "default_mda_progress")]
    pub mda_progress: BTreeMap<String, String>,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for ConversationSummaryV1 {
    fn default() -> Self {
        Self {
            schema_version: default_summary_schema_version(),
            confirmed_intent: Vec::new(),
            open_questions: Vec::new(),
            rejected_assumptions: Vec::new(),
            node_notes: BTreeMap::new(),
            last_user_corrections: Vec::new(),
            mda_progress: default_mda_progress(),
            updated_at: String::new(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameworkMemoryState {
    #[serde(default)]
    pub project_memory_id: String,
    #[serde(default)]
    pub evaluation_batch_id: String,
    #[serde(default = "default_idle_status")]
    pub batch_status: String,
    #[serde(default)]
    pub prompt_version_snapshot: BTreeMap<String, Value>,
    #[serde(default)]
    pub last_completed_batch_id: String,
    #[serde(default)]
    pub review_chains: BTreeMap<String, Value>,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for FrameworkMemoryState {
    fn default() -> Self {
        Self {
            project_memory_id: String::new(),
            evaluation_batch_id: String::new(),
            batch_status: default_idle_status(),
            prompt_version_snapshot: BTreeMap::new(),
            last_completed_batch_id: String::new(),
            review_chains: BTreeMap::new(),
            updated_at: String::new(),
            extra: BTreeMap::new(),
        }
    }
}

fn default_ai_interview_schema_version() -> String {
    AI_INTERVIEW_SCHEMA_VERSION.to_string()
}

fn default_ai_config_schema_version() -> u32 {
    AI_CONFIG_SCHEMA_VERSION
}

fn default_summary_schema_version() -> String {
    SUMMARY_SCHEMA_VERSION.to_string()
}

fn default_ai_response_schema_version() -> String {
    "1.0".to_string()
}

fn default_idle_status() -> String {
    "idle".to_string()
}

fn default_read_only_sandbox() -> String {
    "read-only".to_string()
}

fn default_current_mda_stage() -> String {
    MDA_STAGES
        .first()
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_default()
}

fn default_mda_progress() -> BTreeMap<String, String> {
    MDA_STAGES
        .iter()
        .map(|(stage_id, _)| ((*stage_id).to_string(), "pending".to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_response_payload_roundtrip_preserves_partial_output_contract() {
        let payload = AiResponsePayload {
            schema_version: "1.0".to_string(),
            mode: AiResponseMode::PartialProjectOutput,
            assistant_message: "partial output ready".to_string(),
            route_overview: None,
            question_group: None,
            readiness_check: None,
            full_project_output: None,
            partial_project_output: Some(PartialProjectOutput {
                domain_ids: vec!["combat".to_string()],
                project_state_patch_json: "{\"nodes\":{}}".to_string(),
                confidence_map_json: "{\"nodes\":{\"combat\":0.9}}".to_string(),
            }),
            inferences: Vec::new(),
            option_differences: Vec::new(),
            summary: None,
            errors: Vec::new(),
            extra: BTreeMap::new(),
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("partial_project_output"));
        let restored: AiResponsePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, payload);
    }

    #[test]
    fn ai_completion_json_result_is_separate_from_interview_payload() {
        let result = CompletionJsonResult {
            ok: true,
            data: BTreeMap::from([("task".to_string(), Value::String("patch".to_string()))]),
            raw_text: "{\"task\":\"patch\"}".to_string(),
            errors: Vec::new(),
            attempts: 1,
            schema_name: "patch_analyzer".to_string(),
        };

        let restored: CompletionJsonResult =
            serde_json::from_str(&serde_json::to_string(&result).unwrap()).unwrap();
        assert_eq!(restored, result);
    }

    #[test]
    fn ai_config_v3_roundtrip_preserves_categories_and_active_profile() {
        let config = AiConfig {
            dev: ApiCategory {
                category_id: "dev".to_string(),
                entries: vec![ApiEntry {
                    id: "codex".to_string(),
                    label: "Codex".to_string(),
                    config_type: "local_codex_cli".to_string(),
                    api_url: String::new(),
                    api_key: String::new(),
                    extra_json: Value::Null,
                    codex_toml_path: "codex.toml".to_string(),
                    codex_json_path: String::new(),
                }],
                active_entry_id: "codex".to_string(),
            },
            active_profile_id: "codex".to_string(),
            ..AiConfig::default()
        };

        let restored: AiConfig =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(restored.schema_version, AI_CONFIG_SCHEMA_VERSION);
        assert_eq!(restored.dev.active_entry_id, "codex");
        assert_eq!(restored.active_profile_id, "codex");
    }

    #[test]
    fn secret_bearing_ai_dtos_do_not_expose_secrets_or_prompts_in_debug() {
        let secret = "sk-contract-private-value";
        let prompt = "private prompt body";
        let config = AiConfig {
            dev: ApiCategory {
                category_id: "dev".to_string(),
                active_entry_id: "api".to_string(),
                entries: vec![ApiEntry {
                    id: "api".to_string(),
                    label: "API".to_string(),
                    config_type: "openai_dev_api".to_string(),
                    api_url: "https://user:password@example.test/v1?api_key=url-secret".to_string(),
                    api_key: secret.to_string(),
                    extra_json: serde_json::json!({"apiKey": "extra-private-value"}),
                    codex_toml_path: "C:/Users/private/codex.toml".to_string(),
                    codex_json_path: "C:/Users/private/auth.json".to_string(),
                }],
            },
            profiles: vec![AiProfile {
                id: "profile".to_string(),
                name: "Profile".to_string(),
                adapter: "openai".to_string(),
                llm: serde_json::json!({"apiKey": "profile-private-value"}),
                image: Value::Null,
                metadata: BTreeMap::from([(
                    "accessToken".to_string(),
                    Value::String("metadata-private-value".to_string()),
                )]),
            }],
            ..AiConfig::default()
        };
        let task = ModelTask {
            task_id: "debug".to_string(),
            prompt: prompt.to_string(),
            input_files: Vec::new(),
            output_files: Vec::new(),
            allowed_write_paths: Vec::new(),
            timeout_seconds: 10,
            sandbox: "read-only".to_string(),
            cwd: "C:/Users/private/project".to_string(),
        };

        let debug = format!("{config:?} {task:?}");
        for private in [
            secret,
            "password",
            "url-secret",
            "extra-private-value",
            "profile-private-value",
            "metadata-private-value",
            prompt,
            "C:/Users/private",
        ] {
            assert!(!debug.contains(private), "Debug leaked {private}");
        }
    }

    #[test]
    fn ai_rejects_invalid_schema_mode_and_response_mode() {
        assert!(serde_json::from_str::<AiSchemaMode>("\"almost_full\"").is_err());
        assert!(
            serde_json::from_str::<AiResponsePayload>(r#"{"schemaVersion":"1.0","mode":"maybe"}"#)
                .is_err()
        );
    }

    #[test]
    fn ai_interview_roundtrip_preserves_unknown_nested_fields() {
        let raw = serde_json::json!({
            "schemaVersion": "1.0",
            "futureInterview": {"enabled": true},
            "routeOverview": {
                "futureRoute": ["mechanics"]
            },
            "summary": {
                "futureSummary": "kept",
                "v1": {
                    "futureConversation": 42
                }
            },
            "frameworkMemory": {
                "futureMemory": {"version": 2}
            }
        });

        let state: AiInterviewState = serde_json::from_value(raw).unwrap();
        let restored = serde_json::to_value(state).unwrap();

        assert_eq!(restored["futureInterview"]["enabled"], true);
        assert_eq!(restored["routeOverview"]["futureRoute"][0], "mechanics");
        assert_eq!(restored["summary"]["futureSummary"], "kept");
        assert_eq!(restored["summary"]["v1"]["futureConversation"], 42);
        assert_eq!(restored["frameworkMemory"]["futureMemory"]["version"], 2);
    }
}
