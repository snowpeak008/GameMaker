#![forbid(unsafe_code)]

use adm_ai::AiCapability;
use adm_foundation::{AdmError, AdmResult, ProviderId, read_to_string, write_string};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};

pub const APP_CONFIG_PROFILE_VERSION: u32 = 1;
pub const NAMED_SECRET_STORE_PROFILE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub profile_version: u32,
    pub app_name: String,
    pub data_root: PathBuf,
    pub archive_root: PathBuf,
    pub workspace_root: PathBuf,
    pub log_root: PathBuf,
    pub ai: AiSettings,
}

impl AppConfig {
    pub fn for_data_root(data_root: impl Into<PathBuf>) -> Self {
        let data_root = data_root.into();
        Self {
            profile_version: APP_CONFIG_PROFILE_VERSION,
            app_name: "AutoDesignMaker Rust".to_string(),
            archive_root: data_root.join("archives"),
            workspace_root: data_root.join("workspaces"),
            log_root: data_root.join("logs"),
            ai: AiSettings::default(),
            data_root,
        }
    }

    pub fn load_or_default(data_root: impl Into<PathBuf>) -> AdmResult<Self> {
        let default_config = Self::for_data_root(data_root);
        let config_file = default_config.config_file_path();
        if !config_file.exists() {
            return Ok(default_config);
        }
        Self::from_profile_text(&read_to_string(config_file)?, default_config.data_root)
    }

    pub fn validate(&self) -> AdmResult<()> {
        if self.app_name.trim().is_empty() {
            return Err(AdmError::validation("app_name cannot be empty"));
        }
        if self.profile_version == 0 || self.profile_version > APP_CONFIG_PROFILE_VERSION {
            return Err(AdmError::validation(format!(
                "unsupported app config profile_version: {}",
                self.profile_version
            )));
        }
        ensure_child(&self.data_root, &self.archive_root, "archive_root")?;
        ensure_child(&self.data_root, &self.workspace_root, "workspace_root")?;
        ensure_child(&self.data_root, &self.log_root, "log_root")?;
        self.ai.validate()?;
        Ok(())
    }

    pub fn config_file_path(&self) -> PathBuf {
        self.data_root.join("config").join("app_config.adm")
    }

    pub fn named_secrets_file_path(&self) -> PathBuf {
        self.data_root.join("config").join("named_secrets.adm")
    }

    pub fn save_profile(&self) -> AdmResult<PathBuf> {
        self.validate()?;
        let path = self.config_file_path();
        write_string(&path, &self.render_profile())?;
        Ok(path)
    }

    pub fn ensure_profile(&self) -> AdmResult<PathBuf> {
        let path = self.config_file_path();
        if path.exists() {
            return Ok(path);
        }
        self.save_profile()
    }

    pub fn load_named_secrets(&self) -> AdmResult<NamedSecretStore> {
        NamedSecretStore::load(self.named_secrets_file_path())
    }

    pub fn upsert_named_secret(
        &self,
        name: impl AsRef<str>,
        secret: impl AsRef<str>,
    ) -> AdmResult<PathBuf> {
        let path = self.named_secrets_file_path();
        let mut store = NamedSecretStore::load(&path)?;
        store.upsert(name, secret)?;
        store.save(path)
    }

    pub fn render_profile(&self) -> String {
        let mut document = String::new();
        document.push_str("# AutoDesignMaker Rust App Config\n");
        document.push_str(&format!("profile_version={}\n", self.profile_version));
        document.push_str(&format!("app_name={}\n", self.app_name));
        document.push_str(&format!(
            "data_root={}\n",
            encode_path(&self.data_root, &self.data_root)
        ));
        document.push_str(&format!(
            "archive_root={}\n",
            encode_path(&self.data_root, &self.archive_root)
        ));
        document.push_str(&format!(
            "workspace_root={}\n",
            encode_path(&self.data_root, &self.workspace_root)
        ));
        document.push_str(&format!(
            "log_root={}\n",
            encode_path(&self.data_root, &self.log_root)
        ));
        document.push_str(&self.ai.render_profile());
        document
    }

    pub fn from_profile_text(text: &str, data_root: PathBuf) -> AdmResult<Self> {
        let values = parse_key_values(text);
        let fallback = Self::for_data_root(&data_root);
        let app_name = values
            .get("app_name")
            .cloned()
            .unwrap_or_else(|| fallback.app_name.clone());
        let profile_version =
            parse_optional_u32(&values, "profile_version")?.unwrap_or(APP_CONFIG_PROFILE_VERSION);
        if profile_version > APP_CONFIG_PROFILE_VERSION {
            return Err(AdmError::validation(format!(
                "unsupported app config profile_version: {profile_version}"
            )));
        }
        let config = Self {
            profile_version,
            app_name,
            data_root: decode_path(
                &data_root,
                values.get("data_root").map(String::as_str).unwrap_or("."),
            ),
            archive_root: decode_path(
                &data_root,
                values
                    .get("archive_root")
                    .map(String::as_str)
                    .unwrap_or("archives"),
            ),
            workspace_root: decode_path(
                &data_root,
                values
                    .get("workspace_root")
                    .map(String::as_str)
                    .unwrap_or("workspaces"),
            ),
            log_root: decode_path(
                &data_root,
                values.get("log_root").map(String::as_str).unwrap_or("logs"),
            ),
            ai: AiSettings::from_profile_values(&values)?,
        };
        config.validate()?;
        Ok(config)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiSettings {
    pub default_budget_units: u32,
    pub retry_policy: AiRetryPolicyConfig,
    pub intervention_policy: AiInterventionPolicyConfig,
    pub providers: Vec<AiProviderConfig>,
}

impl Default for AiSettings {
    fn default() -> Self {
        Self {
            default_budget_units: 8,
            retry_policy: AiRetryPolicyConfig { max_attempts: 2 },
            intervention_policy: AiInterventionPolicyConfig::default(),
            providers: vec![AiProviderConfig::local(
                ProviderId::new("mock").expect("static provider id"),
                Some("Built-in mock provider".to_string()),
            )],
        }
    }
}

impl AiSettings {
    pub fn validate(&self) -> AdmResult<()> {
        if self.default_budget_units == 0 {
            return Err(AdmError::validation(
                "ai.default_budget_units must be greater than zero",
            ));
        }
        self.retry_policy.validate()?;
        self.intervention_policy.validate()?;

        let mut seen = HashSet::new();
        for provider in &self.providers {
            provider.validate()?;
            if !seen.insert(provider.provider_id.as_str().to_string()) {
                return Err(AdmError::validation(format!(
                    "duplicate AI provider id: {}",
                    provider.provider_id
                )));
            }
        }
        Ok(())
    }

    pub fn enabled_providers(&self) -> impl Iterator<Item = &AiProviderConfig> {
        self.providers.iter().filter(|provider| provider.enabled)
    }

    pub fn upsert_provider(&mut self, provider: AiProviderConfig) -> AdmResult<()> {
        provider.validate()?;
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|existing| existing.provider_id == provider.provider_id)
        {
            *existing = provider;
        } else {
            self.providers.push(provider);
        }
        self.validate()
    }

    pub fn disable_provider(&mut self, provider_id: ProviderId) -> AdmResult<()> {
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|existing| existing.provider_id == provider_id)
        {
            existing.enabled = false;
        } else {
            self.providers.push(AiProviderConfig::disabled(provider_id));
        }
        self.validate()
    }

    pub fn render_profile(&self) -> String {
        let mut document = String::new();
        document.push_str(&format!(
            "ai.default_budget_units={}\n",
            self.default_budget_units
        ));
        document.push_str(&format!(
            "ai.retry.max_attempts={}\n",
            self.retry_policy.max_attempts
        ));
        document.push_str(&format!(
            "ai.intervention.min_quality_score_percent={}\n",
            self.intervention_policy.min_quality_score_percent
        ));
        document.push_str(&format!(
            "ai.intervention.on_quality_gap={}\n",
            self.intervention_policy.on_quality_gap
        ));
        document.push_str(&format!(
            "ai.intervention.on_missing_content={}\n",
            self.intervention_policy.on_missing_content
        ));
        document.push_str(&format!(
            "ai.intervention.review_after_generation={}\n",
            self.intervention_policy.review_after_generation
        ));
        document.push_str(&format!("ai.provider.count={}\n", self.providers.len()));
        for (index, provider) in self.providers.iter().enumerate() {
            document.push_str(&provider.render_profile(index));
        }
        document
    }

    fn from_profile_values(values: &BTreeMap<String, String>) -> AdmResult<Self> {
        let fallback = Self::default();
        let providers = match parse_optional_u32(values, "ai.provider.count")? {
            Some(provider_count) => {
                let mut providers = Vec::new();
                for index in 0..provider_count as usize {
                    providers.push(AiProviderConfig::from_profile_values(values, index)?);
                }
                providers
            }
            None => fallback.providers.clone(),
        };

        let settings = Self {
            default_budget_units: parse_optional_u32(values, "ai.default_budget_units")?
                .unwrap_or(fallback.default_budget_units),
            retry_policy: AiRetryPolicyConfig {
                max_attempts: parse_optional_u32(values, "ai.retry.max_attempts")?
                    .unwrap_or(fallback.retry_policy.max_attempts),
            },
            intervention_policy: AiInterventionPolicyConfig {
                min_quality_score_percent: parse_optional_u8(
                    values,
                    "ai.intervention.min_quality_score_percent",
                )?
                .unwrap_or(fallback.intervention_policy.min_quality_score_percent),
                on_quality_gap: parse_optional_bool(values, "ai.intervention.on_quality_gap")?
                    .unwrap_or(fallback.intervention_policy.on_quality_gap),
                on_missing_content: parse_optional_bool(
                    values,
                    "ai.intervention.on_missing_content",
                )?
                .unwrap_or(fallback.intervention_policy.on_missing_content),
                review_after_generation: parse_optional_bool(
                    values,
                    "ai.intervention.review_after_generation",
                )?
                .unwrap_or(fallback.intervention_policy.review_after_generation),
            },
            providers,
        };
        settings.validate()?;
        Ok(settings)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRetryPolicyConfig {
    pub max_attempts: u32,
}

impl AiRetryPolicyConfig {
    pub fn validate(&self) -> AdmResult<()> {
        if self.max_attempts == 0 {
            return Err(AdmError::validation(
                "ai.retry.max_attempts must be greater than zero",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiInterventionPolicyConfig {
    pub min_quality_score_percent: u8,
    pub on_quality_gap: bool,
    pub on_missing_content: bool,
    pub review_after_generation: bool,
}

impl Default for AiInterventionPolicyConfig {
    fn default() -> Self {
        Self {
            min_quality_score_percent: 75,
            on_quality_gap: true,
            on_missing_content: true,
            review_after_generation: true,
        }
    }
}

impl AiInterventionPolicyConfig {
    pub fn validate(&self) -> AdmResult<()> {
        if self.min_quality_score_percent > 100 {
            return Err(AdmError::validation(
                "ai.intervention.min_quality_score_percent cannot exceed 100",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderConfig {
    pub provider_id: ProviderId,
    pub enabled: bool,
    pub display_name: Option<String>,
    pub endpoint_hint: Option<String>,
    pub secret_ref: Option<SecretRef>,
    pub requires_secret: bool,
    pub capabilities: Vec<AiCapability>,
}

impl AiProviderConfig {
    pub fn disabled(provider_id: ProviderId) -> Self {
        Self {
            provider_id,
            enabled: false,
            display_name: None,
            endpoint_hint: None,
            secret_ref: None,
            requires_secret: false,
            capabilities: Vec::new(),
        }
    }

    pub fn enabled(
        provider_id: ProviderId,
        endpoint_hint: Option<String>,
        secret_ref: Option<SecretRef>,
    ) -> Self {
        Self {
            provider_id,
            enabled: true,
            display_name: None,
            endpoint_hint,
            secret_ref,
            requires_secret: true,
            capabilities: default_enabled_provider_capabilities(),
        }
    }

    pub fn local(provider_id: ProviderId, display_name: Option<String>) -> Self {
        Self {
            provider_id,
            enabled: true,
            display_name,
            endpoint_hint: Some("local".to_string()),
            secret_ref: None,
            requires_secret: false,
            capabilities: default_enabled_provider_capabilities(),
        }
    }

    pub fn validate(&self) -> AdmResult<()> {
        if let Some(display_name) = &self.display_name
            && display_name.trim().is_empty()
        {
            return Err(AdmError::validation(format!(
                "AI provider {} display name cannot be empty",
                self.provider_id
            )));
        }
        if let Some(endpoint_hint) = &self.endpoint_hint
            && endpoint_hint.trim().is_empty()
        {
            return Err(AdmError::validation(format!(
                "AI provider {} endpoint_hint cannot be empty",
                self.provider_id
            )));
        }
        if self.enabled && self.capabilities.is_empty() {
            return Err(AdmError::validation(format!(
                "AI provider {} capabilities cannot be empty",
                self.provider_id
            )));
        }
        let mut seen = HashSet::new();
        for capability in &self.capabilities {
            if !seen.insert(capability.as_str()) {
                return Err(AdmError::validation(format!(
                    "AI provider {} has duplicate capability: {}",
                    self.provider_id,
                    capability.as_str()
                )));
            }
        }
        Ok(())
    }

    pub fn diagnose<R: SecretResolver>(&self, resolver: &R) -> AiProviderDiagnostic {
        if !self.enabled {
            return AiProviderDiagnostic {
                provider_id: self.provider_id.to_string(),
                readiness: AiProviderReadiness::Disabled,
                capabilities: self.capability_names(),
                notes: vec!["provider is disabled".to_string()],
            };
        }
        if !self.requires_secret {
            return AiProviderDiagnostic {
                provider_id: self.provider_id.to_string(),
                readiness: AiProviderReadiness::Ready,
                capabilities: self.capability_names(),
                notes: vec!["provider does not require a secret".to_string()],
            };
        }
        let Some(secret_ref) = &self.secret_ref else {
            return AiProviderDiagnostic {
                provider_id: self.provider_id.to_string(),
                readiness: AiProviderReadiness::MissingSecret,
                capabilities: self.capability_names(),
                notes: vec!["provider requires a secret_ref".to_string()],
            };
        };
        match resolver.resolve(secret_ref) {
            Ok(Some(_)) => AiProviderDiagnostic {
                provider_id: self.provider_id.to_string(),
                readiness: AiProviderReadiness::Ready,
                capabilities: self.capability_names(),
                notes: vec![format!("secret {} resolved", secret_ref.render_public())],
            },
            Ok(None) => AiProviderDiagnostic {
                provider_id: self.provider_id.to_string(),
                readiness: AiProviderReadiness::MissingSecret,
                capabilities: self.capability_names(),
                notes: vec![format!(
                    "secret {} is not available",
                    secret_ref.render_public()
                )],
            },
            Err(error) => AiProviderDiagnostic {
                provider_id: self.provider_id.to_string(),
                readiness: AiProviderReadiness::UnsupportedSecretReference,
                capabilities: self.capability_names(),
                notes: vec![error.to_string()],
            },
        }
    }

    pub fn capability_names(&self) -> Vec<String> {
        self.capabilities
            .iter()
            .map(|capability| capability.as_str().to_string())
            .collect()
    }

    fn render_profile(&self, index: usize) -> String {
        let prefix = format!("ai.provider.{index}");
        let mut document = String::new();
        document.push_str(&format!("{prefix}.id={}\n", self.provider_id));
        document.push_str(&format!("{prefix}.enabled={}\n", self.enabled));
        document.push_str(&format!(
            "{prefix}.display_name={}\n",
            self.display_name.as_deref().unwrap_or("")
        ));
        document.push_str(&format!(
            "{prefix}.endpoint_hint={}\n",
            self.endpoint_hint.as_deref().unwrap_or("")
        ));
        document.push_str(&format!(
            "{prefix}.secret_ref={}\n",
            self.secret_ref
                .as_ref()
                .map(SecretRef::render)
                .unwrap_or_default()
        ));
        document.push_str(&format!(
            "{prefix}.requires_secret={}\n",
            self.requires_secret
        ));
        document.push_str(&format!(
            "{prefix}.capabilities={}\n",
            render_capabilities(&self.capabilities)
        ));
        document
    }

    fn from_profile_values(values: &BTreeMap<String, String>, index: usize) -> AdmResult<Self> {
        let prefix = format!("ai.provider.{index}");
        let provider_id = required_value(values, &format!("{prefix}.id"))?;
        let enabled = parse_optional_bool(values, &format!("{prefix}.enabled"))?.unwrap_or(true);
        let secret_ref = optional_string(values, &format!("{prefix}.secret_ref"))
            .map(SecretRef::new)
            .transpose()?;
        let capabilities = optional_string(values, &format!("{prefix}.capabilities"))
            .map(|value| parse_capabilities(&value))
            .transpose()?
            .unwrap_or_else(|| default_provider_capabilities_for_enabled(enabled));
        let provider = Self {
            provider_id: ProviderId::new(provider_id)?,
            enabled,
            display_name: optional_string(values, &format!("{prefix}.display_name")),
            endpoint_hint: optional_string(values, &format!("{prefix}.endpoint_hint")),
            secret_ref,
            requires_secret: parse_optional_bool(values, &format!("{prefix}.requires_secret"))?
                .unwrap_or(false),
            capabilities,
        };
        provider.validate()?;
        Ok(provider)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderPreset {
    pub preset_id: String,
    pub display_name: String,
    pub endpoint_hint: String,
    pub default_secret_ref: Option<String>,
    pub requires_secret: bool,
    pub capabilities: Vec<AiCapability>,
    allow_insecure_local_http: bool,
}

impl AiProviderPreset {
    pub fn to_provider_config(
        &self,
        provider_id: ProviderId,
        secret_ref: Option<SecretRef>,
    ) -> AdmResult<AiProviderConfig> {
        validate_openai_compatible_endpoint(&self.endpoint_hint, self.allow_insecure_local_http)?;
        if self.requires_secret && secret_ref.is_none() {
            return Err(AdmError::validation(format!(
                "AI provider preset {} requires a secret_ref",
                self.preset_id
            )));
        }
        let provider = AiProviderConfig {
            provider_id,
            enabled: true,
            display_name: Some(self.display_name.clone()),
            endpoint_hint: Some(self.endpoint_hint.clone()),
            secret_ref,
            requires_secret: self.requires_secret,
            capabilities: self.capabilities.clone(),
        };
        provider.validate()?;
        Ok(provider)
    }

    pub fn render_line(&self) -> String {
        format!(
            "{}\tendpoint={}\tdefault_secret_ref={}\trequires_secret={}\tcapabilities={}",
            self.preset_id,
            self.endpoint_hint,
            self.default_secret_ref.as_deref().unwrap_or("none"),
            self.requires_secret,
            render_capabilities(&self.capabilities)
        )
    }
}

pub fn ai_provider_presets() -> Vec<AiProviderPreset> {
    vec![
        remote_openai_compatible_preset(
            "openai",
            "OpenAI Chat Completions",
            "https://api.openai.com/v1",
            "env:OPENAI_API_KEY",
        ),
        remote_openai_compatible_preset(
            "openrouter",
            "OpenRouter OpenAI-compatible API",
            "https://openrouter.ai/api/v1",
            "env:OPENROUTER_API_KEY",
        ),
        remote_openai_compatible_preset(
            "deepseek",
            "DeepSeek OpenAI-compatible API",
            "https://api.deepseek.com/v1",
            "env:DEEPSEEK_API_KEY",
        ),
        AiProviderPreset {
            preset_id: "local_openai".to_string(),
            display_name: "Local OpenAI-compatible API".to_string(),
            endpoint_hint: "http://localhost:11434/v1".to_string(),
            default_secret_ref: None,
            requires_secret: false,
            capabilities: vec![AiCapability::TextGeneration],
            allow_insecure_local_http: true,
        },
    ]
}

pub fn ai_provider_preset(preset_id: &str) -> AdmResult<AiProviderPreset> {
    let normalized = preset_id.trim().to_ascii_lowercase();
    ai_provider_presets()
        .into_iter()
        .find(|preset| preset.preset_id == normalized)
        .ok_or_else(|| AdmError::invalid_input(format!("unknown AI provider preset: {preset_id}")))
}

pub fn default_secret_ref_for_preset(preset: &AiProviderPreset) -> AdmResult<Option<SecretRef>> {
    preset
        .default_secret_ref
        .as_deref()
        .map(SecretRef::new)
        .transpose()
}

pub fn validate_openai_compatible_endpoint(
    endpoint_hint: &str,
    allow_insecure_local_http: bool,
) -> AdmResult<()> {
    let endpoint = endpoint_hint.trim().trim_end_matches('/');
    if endpoint.is_empty() {
        return Err(AdmError::validation(
            "OpenAI-compatible endpoint cannot be empty",
        ));
    }
    let lower = endpoint.to_ascii_lowercase();
    let is_https = lower.starts_with("https://");
    let is_http = lower.starts_with("http://");
    if !is_https && !is_http {
        return Err(AdmError::validation(format!(
            "OpenAI-compatible endpoint must start with http:// or https://: {endpoint_hint}"
        )));
    }
    if is_http && !(allow_insecure_local_http && is_local_http_endpoint(&lower)) {
        return Err(AdmError::validation(format!(
            "OpenAI-compatible remote endpoint must use https: {endpoint_hint}"
        )));
    }
    if !(lower.ends_with("/v1")
        || lower.ends_with("/v1/chat/completions")
        || lower.contains("/v1/"))
    {
        return Err(AdmError::validation(format!(
            "OpenAI-compatible endpoint should include /v1: {endpoint_hint}"
        )));
    }
    Ok(())
}

fn remote_openai_compatible_preset(
    preset_id: &str,
    display_name: &str,
    endpoint_hint: &str,
    default_secret_ref: &str,
) -> AiProviderPreset {
    AiProviderPreset {
        preset_id: preset_id.to_string(),
        display_name: display_name.to_string(),
        endpoint_hint: endpoint_hint.to_string(),
        default_secret_ref: Some(default_secret_ref.to_string()),
        requires_secret: true,
        capabilities: vec![
            AiCapability::TextGeneration,
            AiCapability::StructuredOutput,
            AiCapability::ScoringReview,
            AiCapability::SdkExplanation,
        ],
        allow_insecure_local_http: false,
    }
}

fn is_local_http_endpoint(lower_endpoint: &str) -> bool {
    lower_endpoint.starts_with("http://localhost:")
        || lower_endpoint.starts_with("http://127.0.0.1:")
        || lower_endpoint.starts_with("http://[::1]:")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiProviderDiagnostic {
    pub provider_id: String,
    pub readiness: AiProviderReadiness,
    pub capabilities: Vec<String>,
    pub notes: Vec<String>,
}

impl AiProviderDiagnostic {
    pub fn render_line(&self) -> String {
        format!(
            "{}\t{:?}\tcapabilities={}\t{}",
            self.provider_id,
            self.readiness,
            self.capabilities.join(","),
            self.notes.join("; ")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiProviderReadiness {
    Disabled,
    Ready,
    MissingSecret,
    UnsupportedSecretReference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef {
    kind: SecretRefKind,
    key: String,
}

impl SecretRef {
    pub fn new(value: impl Into<String>) -> AdmResult<Self> {
        Self::parse(value)
    }

    pub fn env_var(name: impl Into<String>) -> AdmResult<Self> {
        let name = name.into();
        validate_secret_key(&name)?;
        Ok(Self {
            kind: SecretRefKind::EnvVar,
            key: name,
        })
    }

    pub fn named(name: impl Into<String>) -> AdmResult<Self> {
        let name = name.into();
        validate_secret_key(&name)?;
        Ok(Self {
            kind: SecretRefKind::Named,
            key: name,
        })
    }

    pub fn parse(value: impl Into<String>) -> AdmResult<Self> {
        let value = value.into();
        let trimmed = value.trim();
        if let Some(name) = trimmed.strip_prefix("env:") {
            return Self::env_var(name);
        }
        if let Some(name) = trimmed.strip_prefix("named:") {
            return Self::named(name);
        }
        Self::named(trimmed)
    }

    pub fn kind(&self) -> &SecretRefKind {
        &self.kind
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn as_str(&self) -> &str {
        &self.key
    }

    pub fn render(&self) -> String {
        match self.kind {
            SecretRefKind::EnvVar => format!("env:{}", self.key),
            SecretRefKind::Named => format!("named:{}", self.key),
        }
    }

    pub fn render_public(&self) -> String {
        self.render()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretRefKind {
    EnvVar,
    Named,
}

pub trait SecretResolver {
    fn resolve(&self, secret_ref: &SecretRef) -> AdmResult<Option<String>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedSecretStore {
    secrets: BTreeMap<String, String>,
}

impl Default for NamedSecretStore {
    fn default() -> Self {
        Self {
            secrets: BTreeMap::new(),
        }
    }
}

impl NamedSecretStore {
    pub fn load(path: impl AsRef<Path>) -> AdmResult<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        Self::from_profile_text(&read_to_string(path)?)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> AdmResult<PathBuf> {
        let path = path.as_ref().to_path_buf();
        write_string(&path, &self.render_profile())?;
        Ok(path)
    }

    pub fn from_profile_text(text: &str) -> AdmResult<Self> {
        let values = parse_key_values(text);
        let profile_version = parse_optional_u32(&values, "profile_version")?
            .unwrap_or(NAMED_SECRET_STORE_PROFILE_VERSION);
        if profile_version > NAMED_SECRET_STORE_PROFILE_VERSION {
            return Err(AdmError::validation(format!(
                "unsupported named secret store profile_version: {profile_version}"
            )));
        }

        let mut store = Self::default();
        for (key, value) in values {
            if let Some(name) = key.strip_prefix("secret.") {
                store.upsert(name, value)?;
            }
        }
        Ok(store)
    }

    pub fn render_profile(&self) -> String {
        let mut document = String::new();
        document.push_str("# AutoDesignMaker Rust Named Secrets\n");
        document.push_str(&format!(
            "profile_version={NAMED_SECRET_STORE_PROFILE_VERSION}\n"
        ));
        for (name, secret) in &self.secrets {
            document.push_str(&format!("secret.{name}={secret}\n"));
        }
        document
    }

    pub fn upsert(&mut self, name: impl AsRef<str>, secret: impl AsRef<str>) -> AdmResult<()> {
        let name = name.as_ref().trim();
        validate_secret_key(name)?;
        let secret = secret.as_ref().trim();
        if secret.is_empty() {
            return Err(AdmError::invalid_input(
                "named secret value cannot be empty",
            ));
        }
        if secret.contains('\n') || secret.contains('\r') {
            return Err(AdmError::invalid_input(
                "named secret value cannot contain line breaks",
            ));
        }
        self.secrets.insert(name.to_string(), secret.to_string());
        Ok(())
    }

    pub fn resolve(&self, name: &str) -> Option<String> {
        self.secrets
            .get(name)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    pub fn len(&self) -> usize {
        self.secrets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.secrets.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct EnvSecretResolver;

impl SecretResolver for EnvSecretResolver {
    fn resolve(&self, secret_ref: &SecretRef) -> AdmResult<Option<String>> {
        match secret_ref.kind() {
            SecretRefKind::EnvVar => Ok(env::var(secret_ref.key())
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())),
            SecretRefKind::Named => Err(AdmError::unsupported(format!(
                "named secret references are not implemented yet: {}",
                secret_ref.render_public()
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppSecretResolver {
    env: EnvSecretResolver,
    named: NamedSecretStore,
    named_load_error: Option<String>,
}

impl AppSecretResolver {
    pub fn from_config(config: &AppConfig) -> Self {
        Self::from_named_secret_path(config.named_secrets_file_path())
    }

    pub fn from_named_secret_path(path: impl AsRef<Path>) -> Self {
        match NamedSecretStore::load(path) {
            Ok(named) => Self {
                env: EnvSecretResolver,
                named,
                named_load_error: None,
            },
            Err(error) => Self {
                env: EnvSecretResolver,
                named: NamedSecretStore::default(),
                named_load_error: Some(error.to_string()),
            },
        }
    }
}

impl SecretResolver for AppSecretResolver {
    fn resolve(&self, secret_ref: &SecretRef) -> AdmResult<Option<String>> {
        match secret_ref.kind() {
            SecretRefKind::EnvVar => self.env.resolve(secret_ref),
            SecretRefKind::Named => {
                if let Some(error) = &self.named_load_error {
                    return Err(AdmError::validation(format!(
                        "named secret store could not be loaded: {error}"
                    )));
                }
                Ok(self.named.resolve(secret_ref.key()))
            }
        }
    }
}

fn ensure_child(root: &Path, child: &Path, name: &str) -> AdmResult<()> {
    if !child.starts_with(root) {
        return Err(AdmError::validation(format!(
            "{name} must stay inside data_root"
        )));
    }
    Ok(())
}

fn validate_secret_key(value: &str) -> AdmResult<()> {
    if value.trim().is_empty() {
        return Err(AdmError::invalid_input("secret reference cannot be empty"));
    }
    if value.contains('\n') || value.contains('\r') {
        return Err(AdmError::invalid_input(
            "secret reference cannot contain line breaks",
        ));
    }
    Ok(())
}

fn parse_key_values(text: &str) -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    values
}

fn required_value(values: &BTreeMap<String, String>, key: &str) -> AdmResult<String> {
    values
        .get(key)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| AdmError::validation(format!("missing config value: {key}")))
}

fn optional_string(values: &BTreeMap<String, String>, key: &str) -> Option<String> {
    values.get(key).and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_optional_bool(values: &BTreeMap<String, String>, key: &str) -> AdmResult<Option<bool>> {
    values
        .get(key)
        .map(|value| {
            value
                .parse::<bool>()
                .map_err(|_| AdmError::validation(format!("{key} must be true or false")))
        })
        .transpose()
}

fn parse_optional_u32(values: &BTreeMap<String, String>, key: &str) -> AdmResult<Option<u32>> {
    values
        .get(key)
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| AdmError::validation(format!("{key} must be an unsigned integer")))
        })
        .transpose()
}

fn parse_optional_u8(values: &BTreeMap<String, String>, key: &str) -> AdmResult<Option<u8>> {
    values
        .get(key)
        .map(|value| {
            value
                .parse::<u8>()
                .map_err(|_| AdmError::validation(format!("{key} must be 0-255")))
        })
        .transpose()
}

fn default_enabled_provider_capabilities() -> Vec<AiCapability> {
    vec![AiCapability::TextGeneration]
}

fn default_provider_capabilities_for_enabled(enabled: bool) -> Vec<AiCapability> {
    if enabled {
        default_enabled_provider_capabilities()
    } else {
        Vec::new()
    }
}

fn render_capabilities(capabilities: &[AiCapability]) -> String {
    capabilities
        .iter()
        .map(AiCapability::as_str)
        .collect::<Vec<_>>()
        .join(",")
}

fn parse_capabilities(value: &str) -> AdmResult<Vec<AiCapability>> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(AiCapability::parse)
        .collect()
}

fn encode_path(root: &Path, path: &Path) -> String {
    if path == root {
        return ".".to_string();
    }
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

fn decode_path(root: &Path, value: &str) -> PathBuf {
    if value == "." {
        return root.to_path_buf();
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = AppConfig::for_data_root(std::env::temp_dir().join("adm_config_test"));
        config.validate().expect("valid config");
    }

    #[test]
    fn app_config_round_trips_profile_text() {
        let root = std::env::temp_dir().join("adm_config_round_trip");
        let config = AppConfig::for_data_root(&root);
        let parsed = AppConfig::from_profile_text(&config.render_profile(), root).unwrap();

        assert_eq!(parsed.profile_version, APP_CONFIG_PROFILE_VERSION);
        assert_eq!(parsed.app_name, config.app_name);
        assert_eq!(parsed.archive_root, config.archive_root);
        assert_eq!(parsed.ai.default_budget_units, 8);
        assert_eq!(parsed.ai.providers[0].provider_id.as_str(), "mock");
        assert_eq!(
            parsed.ai.providers[0].capabilities,
            vec![AiCapability::TextGeneration]
        );
        assert!(
            config
                .render_profile()
                .contains(&format!("profile_version={APP_CONFIG_PROFILE_VERSION}"))
        );
        assert!(
            config
                .render_profile()
                .contains("ai.provider.0.capabilities=text_generation")
        );
    }

    #[test]
    fn app_config_accepts_legacy_profile_without_version() {
        let root = std::env::temp_dir().join("adm_config_legacy_profile");
        let config = AppConfig::for_data_root(&root);
        let legacy_profile = config
            .render_profile()
            .lines()
            .filter(|line| !line.starts_with("profile_version="))
            .collect::<Vec<_>>()
            .join("\n");

        let parsed = AppConfig::from_profile_text(&legacy_profile, root).unwrap();

        assert_eq!(parsed.profile_version, APP_CONFIG_PROFILE_VERSION);
    }

    #[test]
    fn app_config_rejects_future_profile_version() {
        let root = std::env::temp_dir().join("adm_config_future_profile");
        let profile = AppConfig::for_data_root(&root)
            .render_profile()
            .replace("profile_version=1", "profile_version=999");

        let error = AppConfig::from_profile_text(&profile, root).unwrap_err();

        assert!(error.to_string().contains("profile_version"));
    }

    #[test]
    fn env_secret_resolver_reports_available_secret() {
        let secret = SecretRef::env_var("PATH").unwrap();
        let resolved = EnvSecretResolver.resolve(&secret).unwrap();

        assert!(resolved.is_some());
    }

    #[test]
    fn named_secret_store_round_trips_and_resolves_without_profile_leak() {
        let root = std::env::temp_dir().join("adm_config_named_secret_store");
        let config = AppConfig::for_data_root(&root);
        let secret_path = config
            .upsert_named_secret("openai", "sk-test-named-secret")
            .expect("save named secret");
        let store = config.load_named_secrets().expect("load named secrets");
        let resolver = AppSecretResolver::from_config(&config);
        let secret = SecretRef::named("openai").unwrap();
        let profile_text = config.render_profile();

        assert_eq!(secret_path, config.named_secrets_file_path());
        assert_eq!(store.len(), 1);
        assert_eq!(
            resolver.resolve(&secret).unwrap().as_deref(),
            Some("sk-test-named-secret")
        );
        assert!(!profile_text.contains("sk-test-named-secret"));
        assert!(
            read_to_string(secret_path)
                .unwrap()
                .contains("secret.openai=")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn app_secret_resolver_reports_missing_named_secret() {
        let root = std::env::temp_dir().join("adm_config_missing_named_secret");
        let config = AppConfig::for_data_root(&root);
        let resolver = AppSecretResolver::from_config(&config);
        let secret = SecretRef::named("openai").unwrap();

        assert_eq!(resolver.resolve(&secret).unwrap(), None);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn provider_diagnostic_detects_missing_secret() {
        let provider = AiProviderConfig::enabled(
            ProviderId::new("openai").unwrap(),
            Some("https://api.openai.com".to_string()),
            Some(SecretRef::env_var("ADM_MISSING_SECRET").unwrap()),
        );
        let diagnostic = provider.diagnose(&EnvSecretResolver);

        assert_eq!(diagnostic.readiness, AiProviderReadiness::MissingSecret);
        assert_eq!(diagnostic.capabilities, vec!["text_generation"]);
    }

    #[test]
    fn openai_compatible_provider_presets_build_valid_profiles_without_network() {
        let preset = ai_provider_preset("openai").unwrap();
        let secret_ref = default_secret_ref_for_preset(&preset).unwrap();
        let provider = preset
            .to_provider_config(ProviderId::new("openai_main").unwrap(), secret_ref)
            .unwrap();

        assert_eq!(provider.provider_id.as_str(), "openai_main");
        assert_eq!(
            provider.endpoint_hint.as_deref(),
            Some("https://api.openai.com/v1")
        );
        assert!(provider.requires_secret);
        assert!(
            provider
                .capabilities
                .contains(&AiCapability::StructuredOutput)
        );
        assert!(
            preset
                .render_line()
                .contains("endpoint=https://api.openai.com/v1")
        );
    }

    #[test]
    fn openai_compatible_provider_presets_allow_local_http_without_secret() {
        let preset = ai_provider_preset("local_openai").unwrap();
        let provider = preset
            .to_provider_config(ProviderId::new("local_llm").unwrap(), None)
            .unwrap();

        assert_eq!(
            provider.endpoint_hint.as_deref(),
            Some("http://localhost:11434/v1")
        );
        assert!(!provider.requires_secret);
        assert!(provider.secret_ref.is_none());
    }

    #[test]
    fn openai_compatible_provider_presets_reject_missing_secret_for_remote() {
        let preset = ai_provider_preset("deepseek").unwrap();
        let error = preset
            .to_provider_config(ProviderId::new("deepseek_main").unwrap(), None)
            .unwrap_err();

        assert!(error.to_string().contains("requires a secret_ref"));
    }

    #[test]
    fn openai_compatible_endpoint_validation_rejects_unsafe_or_non_v1_inputs() {
        assert!(validate_openai_compatible_endpoint("https://api.example.test/v1", false).is_ok());
        assert!(
            validate_openai_compatible_endpoint(
                "https://api.example.test/v1/chat/completions",
                false
            )
            .is_ok()
        );
        assert!(validate_openai_compatible_endpoint("http://localhost:11434/v1", true).is_ok());
        assert!(validate_openai_compatible_endpoint("http://api.example.test/v1", false).is_err());
        assert!(validate_openai_compatible_endpoint("https://api.example.test", false).is_err());
    }

    #[test]
    fn provider_profile_round_trips_capabilities() {
        let root = std::env::temp_dir().join("adm_config_capabilities_profile");
        let profile = format!(
            "{}{}{}{}{}{}",
            "# AutoDesignMaker Rust App Config\n",
            "profile_version=1\n",
            "app_name=AutoDesignMaker Rust\n",
            "ai.provider.count=1\n",
            "ai.provider.0.id=openai\n",
            "ai.provider.0.enabled=true\nai.provider.0.endpoint_hint=https://api.openai.com/v1\nai.provider.0.secret_ref=env:OPENAI_API_KEY\nai.provider.0.requires_secret=true\nai.provider.0.capabilities=text_generation,structured_output,scoring_review\n"
        );

        let config = AppConfig::from_profile_text(&profile, root).unwrap();

        assert_eq!(
            config.ai.providers[0].capabilities,
            vec![
                AiCapability::TextGeneration,
                AiCapability::StructuredOutput,
                AiCapability::ScoringReview
            ]
        );
    }

    #[test]
    fn ai_settings_upserts_and_disables_provider() {
        let mut settings = AiSettings::default();
        settings
            .upsert_provider(AiProviderConfig::enabled(
                ProviderId::new("openai").unwrap(),
                Some("https://api.openai.com/v1".to_string()),
                Some(SecretRef::env_var("OPENAI_API_KEY").unwrap()),
            ))
            .unwrap();
        settings
            .upsert_provider(AiProviderConfig::enabled(
                ProviderId::new("openai").unwrap(),
                Some("https://example.invalid/v1".to_string()),
                Some(SecretRef::env_var("ADM_TEST_KEY").unwrap()),
            ))
            .unwrap();

        let provider = settings
            .providers
            .iter()
            .find(|provider| provider.provider_id.as_str() == "openai")
            .unwrap();
        assert_eq!(settings.providers.len(), 2);
        assert_eq!(
            provider.endpoint_hint.as_deref(),
            Some("https://example.invalid/v1")
        );
        assert_eq!(
            provider
                .secret_ref
                .as_ref()
                .map(SecretRef::render)
                .as_deref(),
            Some("env:ADM_TEST_KEY")
        );
        assert_eq!(provider.capabilities, vec![AiCapability::TextGeneration]);

        settings
            .disable_provider(ProviderId::new("openai").unwrap())
            .unwrap();

        let provider = settings
            .providers
            .iter()
            .find(|provider| provider.provider_id.as_str() == "openai")
            .unwrap();
        assert!(!provider.enabled);
    }
}
