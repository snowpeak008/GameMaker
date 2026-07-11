#![forbid(unsafe_code)]

use adm_new_contracts::ai as contract;
use adm_new_foundation::{AdmError, AdmResult, unix_timestamp, write_text_atomic};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path;

mod ai_catalog;
pub use ai_catalog::*;

pub const CRATE_NAME: &str = "adm-new-config";
pub const SCHEMA_VERSION: u32 = 3;
pub const DEFAULT_REMOTE_BASE_URL: &str = "https://vip.auto-code.net/v1";
pub const DEFAULT_LOCAL_LLM_BASE_URL: &str = "http://127.0.0.1:11434/v1";
pub const DEFAULT_LOCAL_IMAGE_BASE_URL: &str = "http://127.0.0.1:7860/sdapi/v1";

pub const CATEGORY_DEV: &str = "dev";
pub const CATEGORY_IMAGE: &str = "image";
pub const CATEGORY_COMPLETION: &str = "completion";

pub const CONFIG_TYPE_LOCAL_CODEX_CLI: &str = "local_codex_cli";
pub const CONFIG_TYPE_LOCAL_CLAUDE_CLI: &str = "local_claude_cli";
pub const CONFIG_TYPE_OPENAI_DEV_API: &str = "openai_dev_api";
pub const CONFIG_TYPE_CUSTOM_DEV_API: &str = "custom_dev_api";
pub const CONFIG_TYPE_CODEX_CLI_IMAGE: &str = "codex_cli_image";
pub const CONFIG_TYPE_OPENAI_IMAGE_API: &str = "openai_image_api";
pub const CONFIG_TYPE_SD_WEBUI_API: &str = "sd_webui_api";
pub const CONFIG_TYPE_CUSTOM_IMAGE_API: &str = "custom_image_api";
pub const CONFIG_TYPE_LOCAL_CODEX_COMPLETION_CLI: &str = "local_codex_completion_cli";
pub const CONFIG_TYPE_LOCAL_CLAUDE_COMPLETION_CLI: &str = "local_claude_completion_cli";
pub const CONFIG_TYPE_OPENAI_COMPLETION_API: &str = "openai_completion_api";
pub const CONFIG_TYPE_CUSTOM_COMPLETION_API: &str = "custom_completion_api";

const SUPPORTED_ADAPTERS: &[&str] = &["openai", "codex", "claude", "local", "none"];
const LLM_SOURCES: &[&str] = &["api", "cli", "none"];
const IMAGE_SOURCES: &[&str] = &["api", "cli_builtin", "none"];

const DEV_CONFIG_TYPES: &[&str] = &[
    CONFIG_TYPE_LOCAL_CODEX_CLI,
    CONFIG_TYPE_LOCAL_CLAUDE_CLI,
    CONFIG_TYPE_OPENAI_DEV_API,
    CONFIG_TYPE_CUSTOM_DEV_API,
];
const IMAGE_CONFIG_TYPES: &[&str] = &[
    CONFIG_TYPE_CODEX_CLI_IMAGE,
    CONFIG_TYPE_OPENAI_IMAGE_API,
    CONFIG_TYPE_SD_WEBUI_API,
    CONFIG_TYPE_CUSTOM_IMAGE_API,
];
const COMPLETION_CONFIG_TYPES: &[&str] = &[
    CONFIG_TYPE_LOCAL_CODEX_COMPLETION_CLI,
    CONFIG_TYPE_LOCAL_CLAUDE_COMPLETION_CLI,
    CONFIG_TYPE_OPENAI_COMPLETION_API,
    CONFIG_TYPE_CUSTOM_COMPLETION_API,
];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigDisposition {
    ActiveRuntime,
    ExampleTemplate,
    LegacyMigration,
    GeneratedReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSource {
    pub relative_path: String,
    pub disposition: ConfigDisposition,
    pub secret_bearing: bool,
}

impl ConfigSource {
    pub fn new(
        relative_path: impl Into<String>,
        disposition: ConfigDisposition,
        secret_bearing: bool,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            disposition,
            secret_bearing,
        }
    }

    pub fn is_active_runtime(&self) -> bool {
        self.disposition == ConfigDisposition::ActiveRuntime
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigInventoryReport {
    pub total_sources: usize,
    pub active_runtime_sources: usize,
    pub secret_bearing_sources: usize,
    pub invalid_paths: Vec<String>,
}

impl ConfigInventoryReport {
    pub fn passes_a00_gate(&self) -> bool {
        self.total_sources > 0 && self.active_runtime_sources > 0 && self.invalid_paths.is_empty()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct LlmConfig {
    pub source: String,
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub cli_path: String,
    pub model: String,
    pub temperature: f64,
    pub timeout: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

impl fmt::Debug for LlmConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LlmConfig")
            .field("source", &self.source)
            .field("provider", &self.provider)
            .field("base_url_configured", &!self.base_url.trim().is_empty())
            .field("has_api_key", &!self.api_key.trim().is_empty())
            .field("cli_path_configured", &!self.cli_path.trim().is_empty())
            .field("model", &self.model)
            .field("temperature", &self.temperature)
            .field("timeout", &self.timeout)
            .field("reasoning_effort", &self.reasoning_effort)
            .finish()
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            source: "api".to_string(),
            provider: "openai".to_string(),
            base_url: String::new(),
            api_key: String::new(),
            cli_path: String::new(),
            model: "gpt-5.5".to_string(),
            temperature: 0.7,
            timeout: 300,
            reasoning_effort: None,
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageConfig {
    pub enabled: bool,
    pub source: String,
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub cli_path: String,
    pub model: String,
}

impl fmt::Debug for ImageConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageConfig")
            .field("enabled", &self.enabled)
            .field("source", &self.source)
            .field("provider", &self.provider)
            .field("base_url_configured", &!self.base_url.trim().is_empty())
            .field("has_api_key", &!self.api_key.trim().is_empty())
            .field("cli_path_configured", &!self.cli_path.trim().is_empty())
            .field("model", &self.model)
            .finish()
    }
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            source: "api".to_string(),
            provider: "openai".to_string(),
            base_url: String::new(),
            api_key: String::new(),
            cli_path: String::new(),
            model: "gpt-image-2".to_string(),
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AiProfile {
    pub id: String,
    pub name: String,
    pub adapter: String,
    pub llm: LlmConfig,
    pub image: ImageConfig,
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
            .field("llm", &self.llm)
            .field("image", &self.image)
            .field("metadata_keys", &self.metadata.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiEntry {
    pub id: String,
    pub label: String,
    pub config_type: String,
    pub api_url: String,
    pub api_key: String,
    pub extra_json: String,
    pub codex_toml_path: String,
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
            .field("extra_json_chars", &self.extra_json.chars().count())
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiCategory {
    pub category_id: String,
    pub entries: Vec<ApiEntry>,
    pub active_entry_id: String,
}

impl ApiCategory {
    pub fn new(category_id: impl Into<String>) -> Self {
        Self {
            category_id: category_id.into(),
            entries: Vec::new(),
            active_entry_id: String::new(),
        }
    }

    pub fn get_entry(&self, entry_id: &str) -> Option<&ApiEntry> {
        self.entries.iter().find(|entry| entry.id == entry_id)
    }

    pub fn active_entry(&self) -> Option<&ApiEntry> {
        self.get_entry(&self.active_entry_id)
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AiConfig {
    pub schema_version: u32,
    pub dev: ApiCategory,
    pub image: ApiCategory,
    pub completion: ApiCategory,
    #[serde(default)]
    pub active_profile_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
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

impl AiConfig {
    pub fn active_profile(&self) -> Option<&AiProfile> {
        let active = if self.active_profile_id.is_empty() {
            &self.dev.active_entry_id
        } else {
            &self.active_profile_id
        };
        self.profiles
            .iter()
            .find(|profile| profile.id == *active)
            .or_else(|| self.profiles.first())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn ok(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfigBundle {
    pub app_config: Value,
    pub project_settings: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegrityReport {
    pub ok: bool,
    pub errors: Vec<String>,
}

pub fn required_config_sources() -> Vec<ConfigSource> {
    use ConfigDisposition::{ActiveRuntime, ExampleTemplate, GeneratedReport, LegacyMigration};

    vec![
        ConfigSource::new("settings/ai_config.json", ActiveRuntime, true),
        ConfigSource::new("settings/app.toml", ActiveRuntime, false),
        ConfigSource::new("settings/project_settings.json", ActiveRuntime, false),
        ConfigSource::new("settings/api_config.toml", LegacyMigration, true),
        ConfigSource::new("settings/ai_profiles.json", LegacyMigration, true),
        ConfigSource::new("settings/ai_config.example.json", ExampleTemplate, true),
        ConfigSource::new("settings/api_config.example.toml", ExampleTemplate, true),
        ConfigSource::new(
            "logs/gates/settings-migration-v3.json",
            GeneratedReport,
            false,
        ),
    ]
}

pub fn summarize_config_sources(sources: &[ConfigSource]) -> ConfigInventoryReport {
    ConfigInventoryReport {
        total_sources: sources.len(),
        active_runtime_sources: sources
            .iter()
            .filter(|source| source.is_active_runtime())
            .count(),
        secret_bearing_sources: sources
            .iter()
            .filter(|source| source.secret_bearing)
            .count(),
        invalid_paths: sources
            .iter()
            .filter(|source| source.relative_path.trim().is_empty())
            .map(|source| source.relative_path.clone())
            .collect(),
    }
}

pub fn create_default_ai_config() -> AiConfig {
    let image_entries = default_entries(CATEGORY_IMAGE);
    let mut config = AiConfig {
        schema_version: SCHEMA_VERSION,
        dev: ApiCategory {
            category_id: CATEGORY_DEV.to_string(),
            entries: default_entries(CATEGORY_DEV),
            active_entry_id: "default".to_string(),
        },
        image: ApiCategory {
            category_id: CATEGORY_IMAGE.to_string(),
            active_entry_id: image_entries
                .first()
                .map(|entry| entry.id.clone())
                .unwrap_or_default(),
            entries: image_entries,
        },
        completion: ApiCategory {
            category_id: CATEGORY_COMPLETION.to_string(),
            entries: default_entries(CATEGORY_COMPLETION),
            active_entry_id: "completion_openai_api".to_string(),
        },
        active_profile_id: String::new(),
        profiles: Vec::new(),
    };
    ensure_category_defaults(&mut config);
    config.profiles = compat_profiles_from_entries(&config);
    config.active_profile_id = config.dev.active_entry_id.clone();
    config
}

pub fn normalize_ai_config_value(value: Option<Value>) -> AiConfig {
    let Some(Value::Object(map)) = value else {
        return create_default_ai_config();
    };
    let schema_version = map
        .get("schema_version")
        .or_else(|| map.get("schemaVersion"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if schema_version >= u64::from(SCHEMA_VERSION) {
        let mut config = AiConfig {
            schema_version: SCHEMA_VERSION,
            dev: category_from_value(map.get(CATEGORY_DEV), CATEGORY_DEV),
            image: category_from_value(map.get(CATEGORY_IMAGE), CATEGORY_IMAGE),
            completion: category_from_value(map.get(CATEGORY_COMPLETION), CATEGORY_COMPLETION),
            active_profile_id: String::new(),
            profiles: Vec::new(),
        };
        ensure_category_defaults(&mut config);
        config.active_profile_id = config.dev.active_entry_id.clone();
        config.profiles = compat_profiles_from_entries(&config);
        config
    } else {
        let (profiles, active) = legacy_profiles_from_value(&Value::Object(map));
        config_from_legacy_profiles(profiles, active)
    }
}

pub fn load_ai_config(path: &Path) -> AdmResult<AiConfig> {
    if !path.exists() {
        return Ok(create_default_ai_config());
    }
    let text = std::fs::read_to_string(path)?;
    let value = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}'))
        .map_err(|error| AdmError::new(format!("invalid AI config JSON: {error}")))?;
    Ok(normalize_ai_config_value(Some(value)))
}

pub fn save_ai_config(config: &AiConfig, path: &Path) -> AdmResult<()> {
    let value = ai_config_to_value(config);
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| AdmError::new(format!("failed to serialize AI config: {error}")))?;
    write_text_atomic(path, &(text + "\n"))
}

pub fn load_ai_config_contract(path: &Path) -> AdmResult<contract::AiConfig> {
    Ok(to_contract_ai_config(&load_ai_config(path)?))
}

pub fn save_ai_config_contract(config: &contract::AiConfig, path: &Path) -> AdmResult<()> {
    let normalized = normalize_contract_ai_config(config.clone());
    let internal = from_contract_ai_config(&normalized);
    let mut value = ai_config_to_value(&internal);
    if let Ok(text) = std::fs::read_to_string(path)
        && let Ok(existing) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}'))
    {
        preserve_unknown_ai_fields(&existing, &mut value);
    }
    let text = serde_json::to_string_pretty(&value)
        .map_err(|error| AdmError::new(format!("failed to serialize AI config: {error}")))?;
    write_text_atomic(path, &(text + "\n"))
}

pub fn normalize_contract_ai_config(config: contract::AiConfig) -> contract::AiConfig {
    to_contract_ai_config(&from_contract_ai_config(&config))
}

pub fn to_contract_ai_config(config: &AiConfig) -> contract::AiConfig {
    contract::AiConfig {
        schema_version: SCHEMA_VERSION,
        dev: to_contract_category(&config.dev),
        image: to_contract_category(&config.image),
        completion: to_contract_category(&config.completion),
        active_profile_id: config.dev.active_entry_id.clone(),
        profiles: config
            .profiles
            .iter()
            .map(to_contract_profile)
            .collect::<Vec<_>>(),
    }
}

pub fn from_contract_ai_config(config: &contract::AiConfig) -> AiConfig {
    let mut config = AiConfig {
        schema_version: SCHEMA_VERSION,
        dev: from_contract_category(&config.dev, CATEGORY_DEV),
        image: from_contract_category(&config.image, CATEGORY_IMAGE),
        completion: from_contract_category(&config.completion, CATEGORY_COMPLETION),
        active_profile_id: config.active_profile_id.clone(),
        profiles: config
            .profiles
            .iter()
            .map(from_contract_profile)
            .collect::<Vec<_>>(),
    };
    ensure_category_defaults(&mut config);
    config.active_profile_id = config.dev.active_entry_id.clone();
    if config.profiles.is_empty() {
        config.profiles = compat_profiles_from_entries(&config);
    }
    config
}

pub fn validate_ai_config(config: &AiConfig) -> ValidationResult {
    let mut errors = Vec::new();
    let warnings = Vec::new();
    if config.schema_version != SCHEMA_VERSION {
        errors.push(format!(
            "Unsupported AI config schema_version: {}",
            config.schema_version
        ));
    }
    validate_category(&config.dev, CATEGORY_DEV, DEV_CONFIG_TYPES, &mut errors);
    validate_category(
        &config.image,
        CATEGORY_IMAGE,
        IMAGE_CONFIG_TYPES,
        &mut errors,
    );
    validate_category(
        &config.completion,
        CATEGORY_COMPLETION,
        COMPLETION_CONFIG_TYPES,
        &mut errors,
    );
    ValidationResult { errors, warnings }
}

pub fn validate_contract_ai_config(config: &contract::AiConfig) -> ValidationResult {
    validate_ai_config(&from_contract_ai_config(config))
}

pub fn migrate_from_legacy(settings_dir: &Path, target_path: Option<&Path>) -> AdmResult<AiConfig> {
    let defaults = create_default_ai_config();
    let mut profiles = Vec::new();
    let legacy_profiles = read_json_file(&settings_dir.join("ai_profiles.json"));
    if let Some(items) = legacy_profiles.get("profiles").and_then(Value::as_array) {
        for (index, item) in items.iter().enumerate() {
            if let Value::Object(raw) = item {
                profiles.push(legacy_profile_to_v2(
                    raw,
                    &format!("legacy_profile_{}", index + 1),
                ));
            }
        }
    }
    if let Some(profile) =
        profile_from_api_config(&read_toml(&settings_dir.join("api_config.toml")))
    {
        profiles.push(profile);
    }
    if let Some(profile) = app_model_profile(&read_toml(&settings_dir.join("app.toml"))) {
        profiles.push(profile);
    }
    if profiles.is_empty() {
        profiles = defaults.profiles.clone();
    }
    let project_settings = read_json_file(&settings_dir.join("project_settings.json"));
    let mut active = legacy_profiles
        .get("active_profile")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if let Some(adapter) = project_settings
        .get("pipeline_adapter")
        .and_then(Value::as_str)
        .map(|value| value.trim().to_lowercase())
        .filter(|value| SUPPORTED_ADAPTERS.contains(&value.as_str()))
    {
        if let Some(default_profile) = defaults
            .profiles
            .iter()
            .find(|profile| profile.adapter == adapter)
        {
            profiles.insert(0, default_profile.clone());
            active = default_profile.id.clone();
        }
    }
    let active_ids = profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect::<BTreeSet<_>>();
    if !active_ids.contains(&active) {
        active = profiles
            .first()
            .map(|profile| profile.id.clone())
            .unwrap_or_else(|| "default".to_string());
    }
    let config = config_from_legacy_profiles(profiles, active);
    if let Some(target_path) = target_path {
        save_ai_config(&config, target_path)?;
    }
    Ok(config)
}

pub fn run_migration(settings_dir: &Path, target_path: Option<&Path>) -> AdmResult<bool> {
    let target = target_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| settings_dir.join("ai_config.json"));
    let legacy_exists = settings_dir.join("ai_profiles.json").exists()
        || settings_dir.join("api_config.toml").exists()
        || read_toml(&settings_dir.join("app.toml"))
            .get("model")
            .is_some();
    if target.exists() || !legacy_exists {
        return Ok(false);
    }
    let config = migrate_from_legacy(settings_dir, Some(&target))?;
    let log_path = settings_dir
        .parent()
        .unwrap_or(settings_dir)
        .join("logs")
        .join("config_migration.log");
    let log = format!(
        "unix:{} migrated AI config\ntarget={}\nactive_profile_id={}\nprofiles={}\n",
        unix_timestamp(),
        target.display(),
        config.active_profile_id,
        config
            .profiles
            .iter()
            .map(|profile| profile.id.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    write_text_atomic(&log_path, &log)?;
    Ok(true)
}

pub fn load_app_config_bundle(settings_dir: &Path) -> AppConfigBundle {
    let app = deep_merge(
        default_app_config(),
        read_toml(&settings_dir.join("app.toml")),
    );
    let project = deep_merge(
        default_project_settings(),
        read_json_file(&settings_dir.join("project_settings.json")),
    );
    AppConfigBundle {
        app_config: app,
        project_settings: project,
    }
}

pub fn get_config<'a>(value: &'a Value, key_path: &str) -> Option<&'a Value> {
    let mut current = value;
    for key in key_path.split('.') {
        current = current.get(key)?;
    }
    Some(current)
}

pub fn normalize_openai_base_url(base_url: &str) -> String {
    let mut base = base_url.trim().trim_end_matches('/').to_string();
    if base.is_empty() {
        return base;
    }
    let lower = base.to_ascii_lowercase();
    if let Some(index) = lower.find("/v1/") {
        base.truncate(index + 3);
        return base;
    }
    for suffix in [
        "/chat/completions",
        "/images/generations",
        "/responses",
        "/models",
    ] {
        if base.to_ascii_lowercase().ends_with(suffix) {
            base.truncate(base.len() - suffix.len());
            base = base.trim_end_matches('/').to_string();
            break;
        }
    }
    if base.to_ascii_lowercase().ends_with("/v1") {
        base
    } else {
        format!("{base}/v1")
    }
}

pub fn openai_endpoint(base_url: &str, endpoint: &str) -> String {
    format!(
        "{}/{}",
        normalize_openai_base_url(base_url).trim_end_matches('/'),
        endpoint.trim_start_matches('/')
    )
}

pub fn api_config_from_active_profile(config: &AiConfig, provider_name: &str) -> Option<Value> {
    let cfg = if provider_name == "llm" {
        let entry = config.dev.active_entry()?;
        let (_, llm) = llm_config_from_entry(Some(entry));
        json!({
            "api_key": llm.api_key,
            "base_url": if llm.provider == "openai" { normalize_openai_base_url(&llm.base_url) } else { llm.base_url.trim_end_matches('/').to_string() },
            "model": format!("{}/{}", llm.provider, llm.model),
            "default_model": llm.model,
            "provider": llm.provider,
            "reasoning_effort": llm.reasoning_effort,
            "profile_id": entry.id,
            "source": llm.source,
            "enabled": true
        })
    } else {
        let image = image_config_from_entry(config.image.active_entry());
        if image.source != "api" {
            return None;
        }
        json!({
            "api_key": image.api_key,
            "base_url": if image.provider == "openai" { normalize_openai_base_url(&image.base_url) } else { image.base_url.trim_end_matches('/').to_string() },
            "model": format!("{}/{}", image.provider, image.model),
            "default_model": image.model,
            "provider": image.provider,
            "source": image.source,
            "enabled": image.enabled
        })
    };
    if cfg
        .get("api_key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .is_empty()
        || cfg
            .get("base_url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .is_empty()
    {
        None
    } else {
        Some(cfg)
    }
}

pub fn validate_data_integrity(project_root: &Path) -> IntegrityReport {
    let mut errors = Vec::new();
    let settings_dir = project_root.join("settings");
    if let Err(error) = load_ai_config(&settings_dir.join("ai_config.json")) {
        errors.push(format!("AI config initialization failed: {error}"));
    }
    let design_domains = project_root.join("knowledge/design_data/domains");
    if design_domains.exists() {
        errors.extend(non_empty_directory(
            &design_domains,
            "Design domains directory",
            "json",
        ));
    }
    let schemas_dir = project_root.join("knowledge/schemas");
    if schemas_dir.exists() {
        errors.extend(non_empty_directory(
            &schemas_dir,
            "JSON schemas directory",
            "json",
        ));
    }
    let plugin_manifest = project_root.join("pipeline/_registry.json");
    if !plugin_manifest.exists() {
        errors.push(format!(
            "Plugin manifest not found: {}",
            plugin_manifest.display()
        ));
    } else {
        match read_json_file(&plugin_manifest)
            .get("plugins")
            .and_then(|plugins| plugins.get("stages"))
            .and_then(Value::as_object)
        {
            Some(stages) if !stages.is_empty() => {}
            _ => errors.push(format!(
                "Plugin manifest has no stages: {}",
                plugin_manifest.display()
            )),
        }
    }
    IntegrityReport {
        ok: errors.is_empty(),
        errors,
    }
}

pub fn mask_secret_value(value: &str) -> String {
    let text = value.trim();
    if text.is_empty() {
        String::new()
    } else if text.chars().count() <= 8 {
        "***".to_string()
    } else {
        let prefix: String = text.chars().take(3).collect();
        let suffix: String = text
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("{prefix}***{suffix}")
    }
}

fn ai_config_to_value(config: &AiConfig) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        CATEGORY_DEV: category_to_value(&config.dev),
        CATEGORY_IMAGE: category_to_value(&config.image),
        CATEGORY_COMPLETION: category_to_value(&config.completion),
        "active_profile_id": config.dev.active_entry_id,
        "profiles": config.profiles,
    })
}

fn preserve_unknown_ai_fields(existing: &Value, next: &mut Value) {
    const TOP_LEVEL_KNOWN: &[&str] = &[
        "schema_version",
        "schemaVersion",
        "dev",
        "image",
        "completion",
        "active_profile_id",
        "activeProfileId",
        "profiles",
    ];
    preserve_unknown_object(existing, next, TOP_LEVEL_KNOWN);
    for category_id in [CATEGORY_DEV, CATEGORY_IMAGE, CATEGORY_COMPLETION] {
        let Some(existing_category) = existing.get(category_id) else {
            continue;
        };
        let Some(next_category) = next.get_mut(category_id) else {
            continue;
        };
        preserve_unknown_object(
            existing_category,
            next_category,
            &[
                "category_id",
                "categoryId",
                "active_entry_id",
                "activeEntryId",
                "entries",
            ],
        );
        let Some(existing_entries) = existing_category.get("entries").and_then(Value::as_array)
        else {
            continue;
        };
        let Some(next_entries) = next_category
            .get_mut("entries")
            .and_then(Value::as_array_mut)
        else {
            continue;
        };
        for next_entry in next_entries {
            let Some(entry_id) = next_entry.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(existing_entry) = existing_entries
                .iter()
                .find(|entry| entry.get("id").and_then(Value::as_str) == Some(entry_id))
            else {
                continue;
            };
            preserve_unknown_object(
                existing_entry,
                next_entry,
                &[
                    "id",
                    "label",
                    "config_type",
                    "configType",
                    "api_url",
                    "apiUrl",
                    "api_key",
                    "apiKey",
                    "extra_json",
                    "extraJson",
                    "codex_toml_path",
                    "codexTomlPath",
                    "codex_json_path",
                    "codexJsonPath",
                ],
            );
        }
    }
}

fn preserve_unknown_object(existing: &Value, next: &mut Value, known: &[&str]) {
    let (Some(existing), Some(next)) = (existing.as_object(), next.as_object_mut()) else {
        return;
    };
    for (key, value) in existing {
        if !known.contains(&key.as_str()) && !next.contains_key(key) {
            next.insert(key.clone(), value.clone());
        }
    }
}

fn category_to_value(category: &ApiCategory) -> Value {
    json!({
        "category_id": category.category_id,
        "active_entry_id": category.active_entry_id,
        "entries": category.entries
    })
}

fn category_from_value(raw: Option<&Value>, category_id: &str) -> ApiCategory {
    let data = raw.and_then(Value::as_object);
    let types = types_for_category(category_id);
    let entries = data
        .and_then(|map| map.get("entries"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    entry_from_value(item, &format!("{category_id}_{}", index + 1), types[0])
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut category = ApiCategory {
        category_id: category_id.to_string(),
        entries,
        active_entry_id: data
            .and_then(|map| map.get("active_entry_id"))
            .or_else(|| data.and_then(|map| map.get("activeEntryId")))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    };
    ensure_entries_for_category(&mut category);
    if !category.active_entry_id.is_empty()
        && category.get_entry(&category.active_entry_id).is_none()
    {
        category.active_entry_id = category
            .entries
            .first()
            .map(|entry| entry.id.clone())
            .unwrap_or_default();
    }
    category
}

fn entry_from_value(raw: &Value, fallback_id: &str, fallback_type: &str) -> ApiEntry {
    let data = raw.as_object();
    let mut config_type = get_string(data, "config_type")
        .or_else(|| get_string(data, "configType"))
        .unwrap_or_else(|| fallback_type.to_string());
    if !all_config_types().contains(&config_type.as_str()) {
        config_type = fallback_type.to_string();
    }
    ApiEntry {
        id: safe_id(
            &get_string(data, "id").unwrap_or_else(|| fallback_id.to_string()),
            fallback_id,
        ),
        label: get_string(data, "label").unwrap_or_else(|| type_label(&config_type).to_string()),
        config_type,
        api_url: get_string(data, "api_url")
            .or_else(|| get_string(data, "apiUrl"))
            .or_else(|| get_string(data, "base_url"))
            .unwrap_or_default(),
        api_key: get_string(data, "api_key")
            .or_else(|| get_string(data, "apiKey"))
            .unwrap_or_default(),
        extra_json: get_extra_text(data),
        codex_toml_path: get_string(data, "codex_toml_path")
            .or_else(|| get_string(data, "codexTomlPath"))
            .unwrap_or_default(),
        codex_json_path: get_string(data, "codex_json_path")
            .or_else(|| get_string(data, "codexJsonPath"))
            .unwrap_or_default(),
    }
}

fn get_string(data: Option<&Map<String, Value>>, key: &str) -> Option<String> {
    data?.get(key).and_then(|value| match value {
        Value::String(text) => Some(text.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    })
}

fn get_extra_text(data: Option<&Map<String, Value>>) -> String {
    let Some(data) = data else {
        return String::new();
    };
    data.get("extra_json")
        .or_else(|| data.get("extraJson"))
        .or_else(|| data.get("extra"))
        .map(value_to_extra_json)
        .unwrap_or_default()
}

fn value_to_extra_json(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Object(map) if map.is_empty() => String::new(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

fn extra_json_to_value(text: &str) -> Value {
    let text = text.trim();
    if text.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
    }
}

fn default_entries(category_id: &str) -> Vec<ApiEntry> {
    match category_id {
        CATEGORY_IMAGE => vec![
            new_entry(
                "codex_cli_image",
                type_label(CONFIG_TYPE_CODEX_CLI_IMAGE),
                CONFIG_TYPE_CODEX_CLI_IMAGE,
            ),
            new_entry_with_url(
                "openai_image_api",
                type_label(CONFIG_TYPE_OPENAI_IMAGE_API),
                CONFIG_TYPE_OPENAI_IMAGE_API,
                DEFAULT_REMOTE_BASE_URL,
                "",
            ),
            new_entry_with_url(
                "sd_webui_api",
                type_label(CONFIG_TYPE_SD_WEBUI_API),
                CONFIG_TYPE_SD_WEBUI_API,
                DEFAULT_LOCAL_IMAGE_BASE_URL,
                "local",
            ),
            new_entry(
                "custom_image_api",
                type_label(CONFIG_TYPE_CUSTOM_IMAGE_API),
                CONFIG_TYPE_CUSTOM_IMAGE_API,
            ),
        ],
        CATEGORY_COMPLETION => vec![
            new_entry(
                "completion_codex_cli",
                type_label(CONFIG_TYPE_LOCAL_CODEX_COMPLETION_CLI),
                CONFIG_TYPE_LOCAL_CODEX_COMPLETION_CLI,
            ),
            new_entry(
                "completion_claude_cli",
                type_label(CONFIG_TYPE_LOCAL_CLAUDE_COMPLETION_CLI),
                CONFIG_TYPE_LOCAL_CLAUDE_COMPLETION_CLI,
            ),
            new_entry_with_url(
                "completion_openai_api",
                type_label(CONFIG_TYPE_OPENAI_COMPLETION_API),
                CONFIG_TYPE_OPENAI_COMPLETION_API,
                DEFAULT_REMOTE_BASE_URL,
                "",
            ),
            new_entry(
                "completion_custom_api",
                type_label(CONFIG_TYPE_CUSTOM_COMPLETION_API),
                CONFIG_TYPE_CUSTOM_COMPLETION_API,
            ),
        ],
        _ => vec![
            new_entry(
                "codex_cli",
                type_label(CONFIG_TYPE_LOCAL_CODEX_CLI),
                CONFIG_TYPE_LOCAL_CODEX_CLI,
            ),
            new_entry(
                "claude_cli",
                type_label(CONFIG_TYPE_LOCAL_CLAUDE_CLI),
                CONFIG_TYPE_LOCAL_CLAUDE_CLI,
            ),
            new_entry_with_url(
                "default",
                "默认 OpenAI",
                CONFIG_TYPE_OPENAI_DEV_API,
                DEFAULT_REMOTE_BASE_URL,
                "",
            ),
            ApiEntry {
                extra_json: r#"{"model": "qwen2.5:14b"}"#.to_string(),
                ..new_entry_with_url(
                    "local_ollama",
                    "本地 Ollama",
                    CONFIG_TYPE_CUSTOM_DEV_API,
                    DEFAULT_LOCAL_LLM_BASE_URL,
                    "local",
                )
            },
        ],
    }
}

fn new_entry(entry_id: &str, label: &str, config_type: &str) -> ApiEntry {
    ApiEntry {
        id: entry_id.to_string(),
        label: label.to_string(),
        config_type: config_type.to_string(),
        api_url: String::new(),
        api_key: String::new(),
        extra_json: String::new(),
        codex_toml_path: String::new(),
        codex_json_path: String::new(),
    }
}

fn new_entry_with_url(
    entry_id: &str,
    label: &str,
    config_type: &str,
    api_url: &str,
    api_key: &str,
) -> ApiEntry {
    ApiEntry {
        api_url: api_url.to_string(),
        api_key: api_key.to_string(),
        ..new_entry(entry_id, label, config_type)
    }
}

fn ensure_entries_for_category(category: &mut ApiCategory) {
    let mut existing_types = category
        .entries
        .iter()
        .map(|entry| entry.config_type.clone())
        .collect::<BTreeSet<_>>();
    let mut existing_ids = category
        .entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<BTreeSet<_>>();
    for mut default in default_entries(&category.category_id) {
        if existing_types.contains(&default.config_type) {
            continue;
        }
        let base_id = default.id.clone();
        let mut suffix = 2;
        while existing_ids.contains(&default.id) {
            default.id = format!("{base_id}_{suffix}");
            suffix += 1;
        }
        existing_types.insert(default.config_type.clone());
        existing_ids.insert(default.id.clone());
        category.entries.push(default);
    }
}

fn ensure_category_defaults(config: &mut AiConfig) {
    config.dev.category_id = CATEGORY_DEV.to_string();
    config.image.category_id = CATEGORY_IMAGE.to_string();
    config.completion.category_id = CATEGORY_COMPLETION.to_string();
    ensure_entries_for_category(&mut config.dev);
    ensure_entries_for_category(&mut config.image);
    ensure_entries_for_category(&mut config.completion);
    if config.dev.active_entry_id.is_empty()
        || config.dev.get_entry(&config.dev.active_entry_id).is_none()
    {
        config.dev.active_entry_id = if config.dev.get_entry("default").is_some() {
            "default".to_string()
        } else {
            first_entry_id(&config.dev)
        };
    }
    if config.completion.active_entry_id.is_empty()
        || config
            .completion
            .get_entry(&config.completion.active_entry_id)
            .is_none()
    {
        config.completion.active_entry_id = first_entry_id(&config.completion);
    }
    if config.image.active_entry_id.is_empty()
        || config
            .image
            .get_entry(&config.image.active_entry_id)
            .is_none()
    {
        config.image.active_entry_id = first_entry_id(&config.image);
    }
}

fn first_entry_id(category: &ApiCategory) -> String {
    category
        .entries
        .first()
        .map(|entry| entry.id.clone())
        .unwrap_or_default()
}

fn to_contract_category(category: &ApiCategory) -> contract::ApiCategory {
    contract::ApiCategory {
        category_id: category.category_id.clone(),
        entries: category.entries.iter().map(to_contract_entry).collect(),
        active_entry_id: category.active_entry_id.clone(),
    }
}

fn from_contract_category(category: &contract::ApiCategory, category_id: &str) -> ApiCategory {
    ApiCategory {
        category_id: if category.category_id.is_empty() {
            category_id.to_string()
        } else {
            category.category_id.clone()
        },
        entries: category.entries.iter().map(from_contract_entry).collect(),
        active_entry_id: category.active_entry_id.clone(),
    }
}

fn to_contract_entry(entry: &ApiEntry) -> contract::ApiEntry {
    contract::ApiEntry {
        id: entry.id.clone(),
        label: entry.label.clone(),
        config_type: entry.config_type.clone(),
        api_url: entry.api_url.clone(),
        api_key: entry.api_key.clone(),
        extra_json: extra_json_to_value(&entry.extra_json),
        codex_toml_path: entry.codex_toml_path.clone(),
        codex_json_path: entry.codex_json_path.clone(),
    }
}

fn from_contract_entry(entry: &contract::ApiEntry) -> ApiEntry {
    ApiEntry {
        id: entry.id.clone(),
        label: entry.label.clone(),
        config_type: entry.config_type.clone(),
        api_url: entry.api_url.clone(),
        api_key: entry.api_key.clone(),
        extra_json: value_to_extra_json(&entry.extra_json),
        codex_toml_path: entry.codex_toml_path.clone(),
        codex_json_path: entry.codex_json_path.clone(),
    }
}

fn to_contract_profile(profile: &AiProfile) -> contract::AiProfile {
    contract::AiProfile {
        id: profile.id.clone(),
        name: profile.name.clone(),
        adapter: profile.adapter.clone(),
        llm: serde_json::to_value(&profile.llm).unwrap_or(Value::Null),
        image: serde_json::to_value(&profile.image).unwrap_or(Value::Null),
        metadata: profile.metadata.clone(),
    }
}

fn from_contract_profile(profile: &contract::AiProfile) -> AiProfile {
    AiProfile {
        id: profile.id.clone(),
        name: profile.name.clone(),
        adapter: profile.adapter.clone(),
        llm: serde_json::from_value(profile.llm.clone()).unwrap_or_default(),
        image: serde_json::from_value(profile.image.clone()).unwrap_or_default(),
        metadata: profile.metadata.clone(),
    }
}

fn config_from_legacy_profiles(profiles: Vec<AiProfile>, active_profile_id: String) -> AiConfig {
    let dev_entries = profiles
        .iter()
        .map(dev_entry_from_profile)
        .collect::<Vec<_>>();
    let image_entries = profiles
        .iter()
        .filter_map(image_entry_from_profile)
        .collect::<Vec<_>>();
    let completion_entries = profiles
        .iter()
        .map(completion_entry_from_profile)
        .collect::<Vec<_>>();
    let active = if dev_entries
        .iter()
        .any(|entry| entry.id == active_profile_id)
    {
        active_profile_id
    } else {
        first_entry_id(&ApiCategory {
            category_id: CATEGORY_DEV.to_string(),
            entries: dev_entries.clone(),
            active_entry_id: String::new(),
        })
    };
    let image_active = format!("image_{active}");
    let completion_active = format!("completion_{active}");
    let mut config = AiConfig {
        schema_version: SCHEMA_VERSION,
        dev: ApiCategory {
            category_id: CATEGORY_DEV.to_string(),
            entries: dev_entries,
            active_entry_id: active.clone(),
        },
        image: ApiCategory {
            category_id: CATEGORY_IMAGE.to_string(),
            active_entry_id: if image_entries.iter().any(|entry| entry.id == image_active) {
                image_active
            } else {
                String::new()
            },
            entries: image_entries,
        },
        completion: ApiCategory {
            category_id: CATEGORY_COMPLETION.to_string(),
            active_entry_id: if completion_entries
                .iter()
                .any(|entry| entry.id == completion_active)
            {
                completion_active
            } else {
                String::new()
            },
            entries: completion_entries,
        },
        active_profile_id: active,
        profiles,
    };
    ensure_category_defaults(&mut config);
    config
}

fn legacy_profiles_from_value(value: &Value) -> (Vec<AiProfile>, String) {
    let defaults = create_default_ai_config();
    let Some(map) = value.as_object() else {
        return (defaults.profiles, "default".to_string());
    };
    let raw_profiles = map.get("profiles").and_then(Value::as_array);
    let profiles = raw_profiles
        .map(|items| {
            items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    item.as_object()
                        .map(|raw| profile_from_value(raw, &format!("profile_{}", index + 1)))
                })
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or(defaults.profiles);
    let mut active = map
        .get("active_profile_id")
        .or_else(|| map.get("active_profile"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if !profiles.iter().any(|profile| profile.id == active) {
        active = profiles
            .first()
            .map(|profile| profile.id.clone())
            .unwrap_or_else(|| "default".to_string());
    }
    (profiles, active)
}

fn profile_from_value(raw: &Map<String, Value>, fallback_id: &str) -> AiProfile {
    let mut adapter = raw
        .get("adapter")
        .or_else(|| raw.get("provider"))
        .and_then(Value::as_str)
        .unwrap_or("openai")
        .trim()
        .to_lowercase();
    if !SUPPORTED_ADAPTERS.contains(&adapter.as_str()) {
        adapter = "openai".to_string();
    }
    let profile_id = safe_id(
        raw.get("id")
            .or_else(|| raw.get("name"))
            .and_then(Value::as_str)
            .unwrap_or(fallback_id),
        fallback_id,
    );
    let name = raw
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(&profile_id)
        .to_string();
    let metadata = raw
        .get("metadata")
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();
    AiProfile {
        id: profile_id,
        name,
        adapter: adapter.clone(),
        llm: llm_from_value(raw.get("llm"), &adapter),
        image: image_from_value(raw.get("image"), &adapter),
        metadata,
    }
}

fn legacy_profile_to_v2(raw: &Map<String, Value>, fallback_id: &str) -> AiProfile {
    let mut profile = profile_from_value(raw, fallback_id);
    profile.metadata.insert(
        "migrated_from".to_string(),
        Value::String("ai_profiles.json".to_string()),
    );
    profile
}

fn llm_from_value(raw: Option<&Value>, adapter: &str) -> LlmConfig {
    let data = raw.and_then(Value::as_object);
    let mut source = get_string(data, "source").unwrap_or_else(|| {
        if matches!(adapter, "codex" | "claude") {
            "cli"
        } else {
            "api"
        }
        .to_string()
    });
    if source == "config" || source == "local" {
        source = "api".to_string();
    }
    if !LLM_SOURCES.contains(&source.as_str()) {
        source = "api".to_string();
    }
    LlmConfig {
        source: source.clone(),
        provider: get_string(data, "provider").unwrap_or_else(|| "openai".to_string()),
        base_url: get_string(data, "base_url").unwrap_or_default(),
        api_key: get_string(data, "api_key").unwrap_or_default(),
        cli_path: get_string(data, "cli_path").unwrap_or_else(|| {
            if source == "cli" {
                adapter.to_string()
            } else {
                String::new()
            }
        }),
        model: get_string(data, "model")
            .or_else(|| get_string(data, "default_model"))
            .unwrap_or_else(|| "gpt-5.5".to_string()),
        temperature: data
            .and_then(|map| map.get("temperature"))
            .and_then(Value::as_f64)
            .unwrap_or(0.7),
        timeout: data
            .and_then(|map| map.get("timeout"))
            .and_then(Value::as_u64)
            .unwrap_or(300),
        reasoning_effort: get_string(data, "reasoning_effort"),
    }
}

fn image_from_value(raw: Option<&Value>, adapter: &str) -> ImageConfig {
    let data = raw.and_then(Value::as_object);
    let enabled = data
        .and_then(|map| map.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut source = get_string(data, "source").unwrap_or_else(|| {
        if adapter == "codex" && enabled {
            "cli_builtin"
        } else {
            "api"
        }
        .to_string()
    });
    if source == "config" || source == "local" {
        source = "api".to_string();
    }
    if !IMAGE_SOURCES.contains(&source.as_str()) {
        source = "api".to_string();
    }
    ImageConfig {
        enabled,
        source: source.clone(),
        provider: get_string(data, "provider").unwrap_or_else(|| "openai".to_string()),
        base_url: get_string(data, "base_url").unwrap_or_default(),
        api_key: get_string(data, "api_key").unwrap_or_default(),
        cli_path: get_string(data, "cli_path").unwrap_or_else(|| {
            if source == "cli_builtin" {
                "codex".to_string()
            } else {
                String::new()
            }
        }),
        model: get_string(data, "model")
            .or_else(|| get_string(data, "default_model"))
            .unwrap_or_else(|| "gpt-image-2".to_string()),
    }
}

fn dev_entry_from_profile(profile: &AiProfile) -> ApiEntry {
    let mut extra = Map::new();
    extra.insert(
        "model".to_string(),
        Value::String(profile.llm.model.clone()),
    );
    if profile.llm.provider != "openai" {
        extra.insert(
            "provider".to_string(),
            Value::String(profile.llm.provider.clone()),
        );
    }
    match profile.adapter.as_str() {
        "codex" => new_entry(&profile.id, &profile.name, CONFIG_TYPE_LOCAL_CODEX_CLI),
        "claude" => new_entry(&profile.id, &profile.name, CONFIG_TYPE_LOCAL_CLAUDE_CLI),
        "local" => ApiEntry {
            api_url: profile.llm.base_url.clone(),
            api_key: profile.llm.api_key.clone(),
            extra_json: value_to_extra_json(&Value::Object(extra)),
            ..new_entry(&profile.id, &profile.name, CONFIG_TYPE_CUSTOM_DEV_API)
        },
        _ => ApiEntry {
            api_url: profile.llm.base_url.clone(),
            api_key: profile.llm.api_key.clone(),
            extra_json: value_to_extra_json(&Value::Object(extra)),
            ..new_entry(&profile.id, &profile.name, CONFIG_TYPE_OPENAI_DEV_API)
        },
    }
}

fn completion_entry_from_profile(profile: &AiProfile) -> ApiEntry {
    let entry_id = format!("completion_{}", profile.id);
    match profile.adapter.as_str() {
        "codex" => new_entry(
            &entry_id,
            &profile.name,
            CONFIG_TYPE_LOCAL_CODEX_COMPLETION_CLI,
        ),
        "claude" => new_entry(
            &entry_id,
            &profile.name,
            CONFIG_TYPE_LOCAL_CLAUDE_COMPLETION_CLI,
        ),
        "local" => ApiEntry {
            api_url: profile.llm.base_url.clone(),
            api_key: profile.llm.api_key.clone(),
            extra_json: value_to_extra_json(&json!({"model": profile.llm.model})),
            ..new_entry(&entry_id, &profile.name, CONFIG_TYPE_CUSTOM_COMPLETION_API)
        },
        _ => ApiEntry {
            api_url: profile.llm.base_url.clone(),
            api_key: profile.llm.api_key.clone(),
            extra_json: value_to_extra_json(&json!({"model": profile.llm.model})),
            ..new_entry(&entry_id, &profile.name, CONFIG_TYPE_OPENAI_COMPLETION_API)
        },
    }
}

fn image_entry_from_profile(profile: &AiProfile) -> Option<ApiEntry> {
    if !profile.image.enabled {
        return None;
    }
    let entry_id = format!("image_{}", profile.id);
    if profile.image.source == "cli_builtin" {
        Some(new_entry(
            &entry_id,
            &profile.name,
            CONFIG_TYPE_CODEX_CLI_IMAGE,
        ))
    } else if profile.image.source == "api" {
        let config_type = if profile.image.base_url.contains("7860") {
            CONFIG_TYPE_SD_WEBUI_API
        } else {
            CONFIG_TYPE_OPENAI_IMAGE_API
        };
        Some(ApiEntry {
            api_url: profile.image.base_url.clone(),
            api_key: profile.image.api_key.clone(),
            extra_json: value_to_extra_json(&json!({"model": profile.image.model})),
            ..new_entry(&entry_id, &profile.name, config_type)
        })
    } else {
        None
    }
}

fn compat_profiles_from_entries(config: &AiConfig) -> Vec<AiProfile> {
    let image_cfg = image_config_from_entry(config.image.active_entry());
    config
        .dev
        .entries
        .iter()
        .map(|entry| {
            let (adapter, llm) = llm_config_from_entry(Some(entry));
            AiProfile {
                id: entry.id.clone(),
                name: if entry.label.is_empty() {
                    type_label(&entry.config_type).to_string()
                } else {
                    entry.label.clone()
                },
                adapter,
                llm,
                image: image_cfg.clone(),
                metadata: BTreeMap::new(),
            }
        })
        .collect()
}

fn llm_config_from_entry(entry: Option<&ApiEntry>) -> (String, LlmConfig) {
    let Some(entry) = entry else {
        return (
            "none".to_string(),
            LlmConfig {
                source: "none".to_string(),
                ..LlmConfig::default()
            },
        );
    };
    if [
        CONFIG_TYPE_LOCAL_CODEX_CLI,
        CONFIG_TYPE_LOCAL_CODEX_COMPLETION_CLI,
    ]
    .contains(&entry.config_type.as_str())
    {
        return (
            "codex".to_string(),
            LlmConfig {
                source: "cli".to_string(),
                cli_path: "codex".to_string(),
                model: entry_model(entry, "gpt-5.5"),
                ..LlmConfig::default()
            },
        );
    }
    if [
        CONFIG_TYPE_LOCAL_CLAUDE_CLI,
        CONFIG_TYPE_LOCAL_CLAUDE_COMPLETION_CLI,
    ]
    .contains(&entry.config_type.as_str())
    {
        return (
            "claude".to_string(),
            LlmConfig {
                source: "cli".to_string(),
                cli_path: "claude".to_string(),
                model: entry_model(entry, "claude-sonnet-4-6"),
                ..LlmConfig::default()
            },
        );
    }
    (
        "openai".to_string(),
        LlmConfig {
            source: "api".to_string(),
            provider: entry_provider(entry),
            base_url: entry.api_url.clone(),
            api_key: entry.api_key.clone(),
            model: entry_model(entry, "gpt-5.5"),
            ..LlmConfig::default()
        },
    )
}

fn image_config_from_entry(entry: Option<&ApiEntry>) -> ImageConfig {
    let Some(entry) = entry else {
        return ImageConfig {
            enabled: false,
            source: "none".to_string(),
            ..ImageConfig::default()
        };
    };
    if entry.config_type == CONFIG_TYPE_CODEX_CLI_IMAGE {
        return ImageConfig {
            enabled: true,
            source: "cli_builtin".to_string(),
            cli_path: "codex".to_string(),
            ..ImageConfig::default()
        };
    }
    let fallback = if entry.config_type == CONFIG_TYPE_SD_WEBUI_API {
        "sd-webui"
    } else {
        "gpt-image-2"
    };
    ImageConfig {
        enabled: true,
        source: "api".to_string(),
        provider: entry_provider(entry),
        base_url: entry.api_url.clone(),
        api_key: entry.api_key.clone(),
        model: entry_model(entry, fallback),
        ..ImageConfig::default()
    }
}

fn entry_model(entry: &ApiEntry, fallback: &str) -> String {
    let extra = extra_dict(&entry.extra_json);
    extra
        .get("model")
        .or_else(|| extra.get("default_model"))
        .and_then(Value::as_str)
        .unwrap_or(fallback)
        .to_string()
}

fn entry_provider(entry: &ApiEntry) -> String {
    extra_dict(&entry.extra_json)
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("openai")
        .to_string()
}

fn extra_dict(raw: &str) -> Map<String, Value> {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn validate_category(
    category: &ApiCategory,
    expected_id: &str,
    supported_types: &[&str],
    errors: &mut Vec<String>,
) {
    if category.category_id != expected_id {
        errors.push(format!(
            "{expected_id}: category_id must be {expected_id}, got {}",
            category.category_id
        ));
    }
    let mut seen = BTreeMap::<String, usize>::new();
    for entry in &category.entries {
        if entry.id.trim().is_empty() {
            errors.push(format!("{expected_id}: entry id cannot be empty"));
        }
        *seen.entry(entry.id.clone()).or_default() += 1;
    }
    for (entry_id, count) in seen {
        if count > 1 {
            errors.push(format!("{expected_id}: duplicated entry id {entry_id}"));
        }
    }
    if category.active_entry_id.is_empty() {
        return;
    }
    let Some(active_entry) = category.get_entry(&category.active_entry_id) else {
        errors.push(format!("{expected_id}: active entry does not exist"));
        return;
    };
    validate_entry(active_entry, expected_id, supported_types, errors);
}

fn validate_entry(
    entry: &ApiEntry,
    category_id: &str,
    supported_types: &[&str],
    errors: &mut Vec<String>,
) {
    let label = if entry.label.is_empty() {
        type_label(&entry.config_type)
    } else {
        &entry.label
    };
    let descriptor = descriptor_for_config_type(&entry.config_type);
    if !supported_types.contains(&entry.config_type.as_str()) || descriptor.is_none() {
        errors.push(format!(
            "{category_id}.{} uses unsupported config_type {}",
            entry.id, entry.config_type
        ));
        return;
    }
    let descriptor = descriptor.expect("descriptor checked above");
    let parsed_extra = if entry.extra_json.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        match serde_json::from_str::<Value>(&entry.extra_json) {
            Ok(value @ Value::Object(_)) => value,
            Ok(_) => {
                errors.push(format!("{label}: extra_json must be a JSON object"));
                Value::Object(Map::new())
            }
            Err(error) => {
                errors.push(format!("{label}: invalid extra_json ({error})"));
                Value::Object(Map::new())
            }
        }
    };
    let extra = parsed_extra.as_object();
    if descriptor
        .required_fields
        .contains(&AiRequiredField::ApiUrl)
        && entry.api_url.trim().is_empty()
    {
        errors.push(format!("{label}: missing api_url"));
    }
    if descriptor
        .required_fields
        .contains(&AiRequiredField::ApiKey)
    {
        let auth_mode = extra
            .and_then(|object| object.get("auth_mode"))
            .and_then(Value::as_str)
            .unwrap_or("direct");
        match auth_mode {
            "direct" => {
                if entry.api_key.trim().is_empty() {
                    errors.push(format!("{label}: missing api_key"));
                }
            }
            "env" => {
                if !extra
                    .and_then(|object| object.get("api_key_env"))
                    .and_then(Value::as_str)
                    .is_some_and(|name| !name.trim().is_empty())
                {
                    errors.push(format!("{label}: auth_mode env requires api_key_env"));
                }
            }
            "none" => {}
            other => errors.push(format!("{label}: unsupported auth_mode {other}")),
        }
    }
    if descriptor.required_fields.contains(&AiRequiredField::Model)
        && !extra
            .and_then(|object| object.get("model"))
            .and_then(Value::as_str)
            .is_some_and(|model| !model.trim().is_empty())
    {
        errors.push(format!("{label}: extra_json.model is required"));
    }
}

fn profile_from_api_config(api_config: &Value) -> Option<AiProfile> {
    let llm = api_config.get("llm").and_then(Value::as_object);
    let image = api_config.get("image").and_then(Value::as_object);
    let image2 = api_config.get("image2").and_then(Value::as_object);
    if llm.is_none() && image.is_none() && image2.is_none() {
        return None;
    }
    let mut merged_image = image.cloned().unwrap_or_default();
    if let Some(image2) = image2 {
        for (key, value) in image2 {
            merged_image.insert(key.clone(), value.clone());
        }
    }
    let llm_value = llm.cloned().map(Value::Object);
    let image_value = Some(Value::Object(merged_image));
    let mut profile = AiProfile {
        id: "legacy_api".to_string(),
        name: "旧版 API 配置".to_string(),
        adapter: "openai".to_string(),
        llm: llm_from_value(llm_value.as_ref(), "openai"),
        image: image_from_value(image_value.as_ref(), "openai"),
        metadata: BTreeMap::new(),
    };
    profile.image.enabled = false;
    profile.metadata.insert(
        "migrated_from".to_string(),
        Value::String("api_config.toml".to_string()),
    );
    Some(profile)
}

fn app_model_profile(app_config: &Value) -> Option<AiProfile> {
    let model = app_config.get("model").and_then(Value::as_object)?;
    let llm_value = Value::Object(model.clone());
    let mut profile = AiProfile {
        id: "app_model".to_string(),
        name: "app.toml 模型配置".to_string(),
        adapter: "openai".to_string(),
        llm: llm_from_value(Some(&llm_value), "openai"),
        image: ImageConfig {
            enabled: false,
            source: "none".to_string(),
            ..ImageConfig::default()
        },
        metadata: BTreeMap::new(),
    };
    profile.metadata.insert(
        "migrated_from".to_string(),
        Value::String("app.toml".to_string()),
    );
    Some(profile)
}

fn read_json_file(path: &Path) -> Value {
    if !path.exists() {
        return Value::Object(Map::new());
    }
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_else(|| Value::Object(Map::new()))
}

fn read_toml(path: &Path) -> Value {
    if !path.exists() {
        return Value::Object(Map::new());
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return Value::Object(Map::new());
    };
    parse_simple_toml(&text)
}

fn parse_simple_toml(text: &str) -> Value {
    let mut root = Map::new();
    let mut section: Vec<String> = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1]
                .split('.')
                .map(|part| part.trim().to_string())
                .filter(|part| !part.is_empty())
                .collect();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        insert_toml_value(
            &mut root,
            &section,
            key.trim(),
            parse_toml_scalar(value.trim()),
        );
    }
    Value::Object(root)
}

fn insert_toml_value(root: &mut Map<String, Value>, section: &[String], key: &str, value: Value) {
    let mut current = root;
    for part in section {
        current = current
            .entry(part.clone())
            .or_insert_with(|| Value::Object(Map::new()))
            .as_object_mut()
            .expect("section value must be object");
    }
    current.insert(key.to_string(), value);
}

fn parse_toml_scalar(value: &str) -> Value {
    let value = value.trim();
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        Value::String(value[1..value.len() - 1].to_string())
    } else if value == "true" {
        Value::Bool(true)
    } else if value == "false" {
        Value::Bool(false)
    } else if let Ok(integer) = value.parse::<i64>() {
        Value::Number(integer.into())
    } else if let Ok(float) = value.parse::<f64>() {
        serde_json::Number::from_f64(float)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    } else {
        Value::String(value.to_string())
    }
}

fn default_app_config() -> Value {
    json!({
        "project": {"name": "AutoDesignMaker", "version": "1.0.0"},
        "plugins": {"manifest_path": "pipeline/_registry.json", "auto_discover": true},
        "manual_gates": {
            "enable_manual_gates": true,
            "gate_art_style": true,
            "gate_program_architecture": false
        },
        "art_style_generation": {
            "num_options": 5,
            "image_width": 1024,
            "image_height": 1024
        },
        "pipeline": {
            "unattended_execution": {
                "max_auto_repair_attempts": 2,
                "repair_timeout_seconds": 120,
                "continue_independent_tasks": true,
                "continue_after_completed_with_review": false,
                "sync_per_group": true,
                "sync_checkpoint_every_tasks": 10,
                "sync_checkpoint_seconds": 600,
                "enable_step11_auto_repair": true,
                "enable_step12_auto_repair": false
            },
            "execution": {
                "max_concurrent_dev_tasks": 1,
                "max_concurrent_art_tasks": 1,
                "write_conflict_policy": "serialize",
                "group_compile_policy": "after_group"
            }
        }
    })
}

fn default_project_settings() -> Value {
    json!({
        "unity_project_path": "",
        "editor_path": "",
        "development_path": "",
        "last_save_id": "",
        "last_active_stage": "",
        "default_export_format": "markdown",
        "recent_projects": []
    })
}

fn deep_merge(defaults: Value, override_value: Value) -> Value {
    match (defaults, override_value) {
        (Value::Object(mut defaults), Value::Object(override_map)) => {
            for (key, value) in override_map {
                let merged = if let Some(default_value) = defaults.remove(&key) {
                    deep_merge(default_value, value)
                } else {
                    value
                };
                defaults.insert(key, merged);
            }
            Value::Object(defaults)
        }
        (_, override_value) => override_value,
    }
}

fn non_empty_directory(path: &Path, label: &str, extension: &str) -> Vec<String> {
    if !path.exists() {
        return vec![format!("{label} not found: {}", path.display())];
    }
    if !path.is_dir() {
        return vec![format!("{label} is not a directory: {}", path.display())];
    }
    let has_match = std::fs::read_dir(path)
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .any(|entry| entry.path().extension().and_then(|value| value.to_str()) == Some(extension));
    if has_match {
        Vec::new()
    } else {
        vec![format!("{label} is empty: {}", path.display())]
    }
}

fn safe_id(value: &str, fallback: &str) -> String {
    let mut clean = String::new();
    let mut last_was_sep = false;
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            clean.push(ch.to_ascii_lowercase());
            last_was_sep = false;
        } else if !last_was_sep {
            clean.push('_');
            last_was_sep = true;
        }
    }
    let clean = clean.trim_matches('_').to_string();
    if clean.is_empty() {
        fallback.to_string()
    } else {
        clean
    }
}

fn types_for_category(category_id: &str) -> &'static [&'static str] {
    match category_id {
        CATEGORY_IMAGE => IMAGE_CONFIG_TYPES,
        CATEGORY_COMPLETION => COMPLETION_CONFIG_TYPES,
        _ => DEV_CONFIG_TYPES,
    }
}

fn all_config_types() -> Vec<&'static str> {
    DEV_CONFIG_TYPES
        .iter()
        .chain(IMAGE_CONFIG_TYPES.iter())
        .chain(COMPLETION_CONFIG_TYPES.iter())
        .copied()
        .collect()
}

fn type_label(config_type: &str) -> &'static str {
    match config_type {
        CONFIG_TYPE_LOCAL_CODEX_CLI => "本地 Codex CLI",
        CONFIG_TYPE_LOCAL_CLAUDE_CLI => "本地 Claude Code CLI",
        CONFIG_TYPE_OPENAI_DEV_API => "OpenAI 兼容 API",
        CONFIG_TYPE_CUSTOM_DEV_API => "自定义 API",
        CONFIG_TYPE_CODEX_CLI_IMAGE => "Codex CLI 内置生图",
        CONFIG_TYPE_OPENAI_IMAGE_API => "OpenAI 图片 API",
        CONFIG_TYPE_SD_WEBUI_API => "本地 SD WebUI",
        CONFIG_TYPE_CUSTOM_IMAGE_API => "自定义图片 API",
        CONFIG_TYPE_LOCAL_CODEX_COMPLETION_CLI => "本地 Codex CLI",
        CONFIG_TYPE_LOCAL_CLAUDE_COMPLETION_CLI => "本地 Claude Code CLI",
        CONFIG_TYPE_OPENAI_COMPLETION_API => "OpenAI 补全 API",
        CONFIG_TYPE_CUSTOM_COMPLETION_API => "自定义补全 API",
        _ => "entry",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;
    use std::path::PathBuf;

    #[test]
    fn required_sources_pass_a00_inventory_gate() {
        let report = summarize_config_sources(&required_config_sources());

        assert!(report.passes_a00_gate());
        assert_eq!(report.active_runtime_sources, 3);
        assert!(report.secret_bearing_sources >= 4);
    }

    #[test]
    fn secret_masking_matches_python_ai_config_helper() {
        assert_eq!(mask_secret_value("sk-1234567890"), "sk-***7890");
        assert_eq!(mask_secret_value("abc"), "***");
        assert_eq!(mask_secret_value("   "), "");
    }

    #[test]
    fn default_ai_config_has_three_categories_and_compat_profiles() {
        let config = create_default_ai_config();

        assert_eq!(config.schema_version, SCHEMA_VERSION);
        assert_eq!(config.dev.active_entry_id, "default");
        assert_eq!(config.image.active_entry_id, "codex_cli_image");
        assert_eq!(config.completion.active_entry_id, "completion_openai_api");
        assert!(
            config
                .profiles
                .iter()
                .any(|profile| profile.id == "default")
        );
    }

    #[test]
    fn completion_empty_active_entry_is_repaired_to_the_first_entry() {
        let mut config = create_default_ai_config();
        config.completion.active_entry_id.clear();

        ensure_category_defaults(&mut config);

        assert_eq!(
            config.completion.active_entry_id,
            config.completion.entries[0].id
        );
    }

    #[test]
    fn snake_case_v3_config_loads_and_converts_to_contract_config() {
        let config = normalize_ai_config_value(Some(json!({
            "schema_version": 3,
            "dev": {
                "category_id": "dev",
                "active_entry_id": "codex_cli",
                "entries": [{"id": "codex_cli", "label": "Codex", "config_type": "local_codex_cli"}]
            },
            "image": {"category_id": "image", "active_entry_id": "", "entries": []},
            "completion": {
                "category_id": "completion",
                "active_entry_id": "completion_openai_api",
                "entries": [{
                    "id": "completion_openai_api",
                    "label": "OpenAI",
                    "config_type": "openai_completion_api",
                    "api_url": "https://api.example.test",
                    "api_key": "secret",
                    "extra_json": "{\"model\":\"gpt-5.5\"}"
                }]
            }
        })));
        let contract = to_contract_ai_config(&config);

        assert_eq!(config.dev.active_entry_id, "codex_cli");
        assert_eq!(contract.active_profile_id, "codex_cli");
        assert_eq!(
            contract.completion.entries[0].extra_json["model"],
            Value::String("gpt-5.5".to_string())
        );
    }

    #[test]
    fn legacy_profile_migration_preserves_active_adapter() {
        let legacy = json!({
            "schema_version": 1,
            "active_profile": "local",
            "profiles": [{
                "id": "local",
                "name": "Local",
                "adapter": "local",
                "llm": {
                    "source": "local",
                    "base_url": "http://127.0.0.1:11434/v1",
                    "api_key": "local",
                    "model": "qwen"
                }
            }]
        });
        let config = normalize_ai_config_value(Some(legacy));

        assert_eq!(config.dev.active_entry_id, "local");
        assert_eq!(
            config.dev.active_entry().unwrap().config_type,
            CONFIG_TYPE_CUSTOM_DEV_API
        );
        assert_eq!(
            config.completion.active_entry().unwrap().config_type,
            CONFIG_TYPE_CUSTOM_COMPLETION_API
        );
    }

    #[test]
    fn validation_checks_only_active_credentials_and_duplicate_ids() {
        let mut config = create_default_ai_config();
        config.dev.active_entry_id = "codex_cli".to_string();
        config.completion.active_entry_id = "completion_codex_cli".to_string();
        config.dev.entries.push(config.dev.entries[0].clone());
        let report = validate_ai_config(&config);

        assert!(
            report
                .errors
                .iter()
                .any(|error| error.contains("duplicated entry id"))
        );
        assert!(
            !report
                .errors
                .iter()
                .any(|error| error.contains("codex_toml_path"))
        );
        assert!(!report.errors.iter().any(|error| error.contains("OpenAI")));
    }

    #[test]
    fn completion_api_validation_requires_a_nonempty_model() {
        let mut config = create_default_ai_config();
        config.dev.active_entry_id = "codex_cli".to_string();
        let entry = config
            .completion
            .entries
            .iter_mut()
            .find(|entry| entry.id == "completion_openai_api")
            .unwrap();
        entry.api_key = "secret".to_string();
        entry.extra_json = "{}".to_string();

        let missing = validate_ai_config(&config);
        assert!(
            missing
                .errors
                .iter()
                .any(|error| error.contains("extra_json.model is required"))
        );

        config
            .completion
            .entries
            .iter_mut()
            .find(|entry| entry.id == "completion_openai_api")
            .unwrap()
            .extra_json = r#"{"model":"gpt-test"}"#.to_string();
        let valid = validate_ai_config(&config);
        assert!(
            !valid
                .errors
                .iter()
                .any(|error| error.contains("extra_json.model"))
        );
    }

    #[test]
    fn structural_validation_supports_none_and_environment_authentication() {
        let mut config = create_default_ai_config();
        config.dev.active_entry_id = "codex_cli".to_string();
        let entry = config
            .completion
            .entries
            .iter_mut()
            .find(|entry| entry.id == "completion_openai_api")
            .unwrap();
        entry.api_key.clear();
        entry.extra_json = r#"{"model":"local-model","auth_mode":"none"}"#.to_string();
        let no_auth = validate_ai_config(&config);
        assert!(
            !no_auth
                .errors
                .iter()
                .any(|error| error.contains("missing api_key"))
        );

        let entry = config
            .completion
            .entries
            .iter_mut()
            .find(|entry| entry.id == "completion_openai_api")
            .unwrap();
        entry.extra_json =
            r#"{"model":"gpt-test","auth_mode":"env","api_key_env":"OPENAI_API_KEY"}"#.to_string();
        let env_auth = validate_ai_config(&config);
        assert!(
            !env_auth
                .errors
                .iter()
                .any(|error| error.contains("api_key"))
        );
    }

    #[test]
    fn sd_webui_active_entry_does_not_require_an_api_key() {
        let mut config = create_default_ai_config();
        config.dev.active_entry_id = "codex_cli".to_string();
        config.completion.active_entry_id = "completion_codex_cli".to_string();
        config.image.active_entry_id = "sd_webui_api".to_string();
        let report = validate_ai_config(&config);
        assert!(
            !report
                .errors
                .iter()
                .any(|error| error.contains("Stable Diffusion") && error.contains("api_key"))
        );
    }

    #[test]
    fn api_config_from_active_profile_normalizes_openai_endpoint() {
        let mut config = create_default_ai_config();
        config.dev.active_entry_id = "default".to_string();
        let entry = config.dev.get_entry("default").unwrap().clone();
        config.dev.entries = vec![ApiEntry {
            api_key: "secret".to_string(),
            extra_json: "{\"model\":\"gpt-5.5\"}".to_string(),
            ..entry
        }];

        let api = api_config_from_active_profile(&config, "llm").unwrap();

        assert_eq!(api["base_url"], "https://vip.auto-code.net/v1");
        assert_eq!(api["model"], "openai/gpt-5.5");
        assert_eq!(
            openai_endpoint("https://api.example.test", "chat/completions"),
            "https://api.example.test/v1/chat/completions"
        );
        assert_eq!(
            openai_endpoint(
                "https://api.example.test/v1/chat/completions",
                "chat/completions"
            ),
            "https://api.example.test/v1/chat/completions"
        );
        assert_eq!(
            openai_endpoint("https://relay.example.test/openai/responses", "models"),
            "https://relay.example.test/openai/v1/models"
        );
    }

    #[test]
    fn app_config_loader_deep_merges_toml_and_project_settings() {
        let root = temp_root("app_config");
        let settings = root.join("settings");
        std::fs::create_dir_all(&settings).unwrap();
        std::fs::write(
            settings.join("app.toml"),
            "[pipeline.execution]\nmax_concurrent_dev_tasks = 3\n",
        )
        .unwrap();
        std::fs::write(
            settings.join("project_settings.json"),
            r#"{"default_export_format":"json"}"#,
        )
        .unwrap();

        let bundle = load_app_config_bundle(&settings);

        assert_eq!(
            get_config(
                &bundle.app_config,
                "pipeline.execution.max_concurrent_dev_tasks"
            ),
            Some(&json!(3))
        );
        assert_eq!(
            get_config(&bundle.project_settings, "default_export_format"),
            Some(&json!("json"))
        );
        cleanup(root);
    }

    #[test]
    fn migration_from_legacy_files_writes_v3_config() {
        let root = temp_root("migration");
        let settings = root.join("settings");
        std::fs::create_dir_all(&settings).unwrap();
        std::fs::write(
            settings.join("ai_profiles.json"),
            r#"{"schema_version":1,"active_profile":"p1","profiles":[{"id":"p1","name":"P1","adapter":"openai","llm":{"base_url":"https://api.example.test/v1","api_key":"secret","model":"gpt-test"}}]}"#,
        )
        .unwrap();
        let target = settings.join("ai_config.json");

        let config = migrate_from_legacy(&settings, Some(&target)).unwrap();

        assert_eq!(config.schema_version, SCHEMA_VERSION);
        assert_eq!(config.dev.active_entry_id, "p1");
        assert!(target.exists());
        cleanup(root);
    }

    #[test]
    fn integrity_report_checks_schema_and_plugin_manifest() {
        let root = temp_root("integrity");
        std::fs::create_dir_all(root.join("settings")).unwrap();
        std::fs::create_dir_all(root.join("knowledge/schemas")).unwrap();
        std::fs::create_dir_all(root.join("pipeline")).unwrap();
        save_ai_config(
            &create_default_ai_config(),
            &root.join("settings/ai_config.json"),
        )
        .unwrap();
        std::fs::write(root.join("knowledge/schemas/sample.json"), "{}").unwrap();
        std::fs::write(
            root.join("pipeline/_registry.json"),
            r#"{"plugins":{"stages":{"00":{"module":"m","class":"Plugin"}}}}"#,
        )
        .unwrap();

        let report = validate_data_integrity(&root);

        assert!(report.ok, "{:?}", report.errors);
        cleanup(root);
    }

    #[test]
    fn contract_save_preserves_unknown_top_category_and_entry_fields() {
        let root = temp_root("ai_unknown_roundtrip");
        let path = root.join("ai_config.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&json!({
                "schema_version": 3,
                "future_top": {"enabled": true},
                "dev": {
                    "category_id": "dev",
                    "active_entry_id": "codex",
                    "future_category": 42,
                    "entries": [{
                        "id": "codex",
                        "label": "Codex",
                        "config_type": "local_codex_cli",
                        "future_entry": ["kept"]
                    }]
                },
                "image": {"category_id": "image", "active_entry_id": "", "entries": []},
                "completion": {"category_id": "completion", "active_entry_id": "", "entries": []}
            }))
            .unwrap(),
        )
        .unwrap();

        let mut config = load_ai_config_contract(&path).unwrap();
        config.dev.entries[0].label = "Renamed".to_string();
        save_ai_config_contract(&config, &path).unwrap();
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();

        assert_eq!(saved["future_top"]["enabled"], true);
        assert_eq!(saved["dev"]["future_category"], 42);
        assert_eq!(saved["dev"]["entries"][0]["future_entry"][0], "kept");
        assert_eq!(saved["dev"]["entries"][0]["label"], "Renamed");
        cleanup(root);
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(path: PathBuf) {
        let _ = std::fs::remove_dir_all(path);
    }
}
