use std::{env, fmt};

use adm_new_config::{
    AiAdapterKind, AiConfigCategory, AiConfigDescriptor, AiConfigSource, AiRequiredField,
    descriptor_for_config_type,
};
use adm_new_contracts::ai::{AiConfig, ApiCategory, ApiEntry};
use adm_new_foundation::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cli_probe::{CliProbeDiagnostic, CliProbeRequest, locate_cli_program, probe_cli};
use crate::http_endpoint_policy::validate_http_transport_url;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiResolutionSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiResolutionDiagnostic {
    pub severity: AiResolutionSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Clone)]
pub struct ResolvedAiTarget {
    descriptor: &'static AiConfigDescriptor,
    entry_id: String,
    api_url: Option<String>,
    api_secret: Option<String>,
    auth_mode: String,
    model: Option<String>,
    program: Option<String>,
    available: bool,
    diagnostics: Vec<AiResolutionDiagnostic>,
}

impl ResolvedAiTarget {
    pub fn descriptor(&self) -> &'static AiConfigDescriptor {
        self.descriptor
    }

    pub fn entry_id(&self) -> &str {
        &self.entry_id
    }

    pub fn api_url(&self) -> Option<&str> {
        self.api_url.as_deref()
    }

    pub fn api_secret(&self) -> Option<&str> {
        self.api_secret.as_deref()
    }

    pub fn auth_mode(&self) -> &str {
        &self.auth_mode
    }

    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn program(&self) -> Option<&str> {
        self.program.as_deref()
    }

    pub fn is_available(&self) -> bool {
        self.available
    }

    pub fn diagnostics(&self) -> &[AiResolutionDiagnostic] {
        &self.diagnostics
    }

    pub fn view(&self) -> AiResolutionView {
        AiResolutionView {
            category: self.descriptor.category,
            entry_id: self.entry_id.clone(),
            config_type: self.descriptor.config_type.to_string(),
            source: self.descriptor.source,
            adapter: self.descriptor.adapter,
            available: self.available,
            auth_mode: self.auth_mode.clone(),
            has_secret: self.api_secret.is_some(),
            masked_url: self.api_url.as_deref().map(mask_api_url),
            model: self.model.clone(),
            program: self.program.as_deref().map(safe_program_view),
            capabilities: self
                .descriptor
                .capabilities
                .iter()
                .map(|capability| (*capability).to_string())
                .collect(),
            diagnostics: self
                .diagnostics
                .iter()
                .map(|diagnostic| AiResolutionDiagnostic {
                    severity: diagnostic.severity.clone(),
                    code: diagnostic.code.clone(),
                    message: redact_resolution_message(
                        &diagnostic.message,
                        self.api_secret.as_deref(),
                    ),
                })
                .collect(),
        }
    }
}

impl fmt::Debug for ResolvedAiTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedAiTarget")
            .field("category", &self.descriptor.category)
            .field("entry_id", &self.entry_id)
            .field("config_type", &self.descriptor.config_type)
            .field("source", &self.descriptor.source)
            .field("adapter", &self.descriptor.adapter)
            .field("masked_url", &self.api_url.as_deref().map(mask_api_url))
            .field("auth_mode", &self.auth_mode)
            .field("has_secret", &self.api_secret.is_some())
            .field("model", &self.model)
            .field(
                "program",
                &self.program.as_deref().map(safe_program_debug_view),
            )
            .field("available", &self.available)
            .field("diagnostic_count", &self.diagnostics.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiResolutionView {
    pub category: AiConfigCategory,
    pub entry_id: String,
    pub config_type: String,
    pub source: AiConfigSource,
    pub adapter: AiAdapterKind,
    pub available: bool,
    pub auth_mode: String,
    pub has_secret: bool,
    pub masked_url: Option<String>,
    pub model: Option<String>,
    pub program: Option<String>,
    pub capabilities: Vec<String>,
    pub diagnostics: Vec<AiResolutionDiagnostic>,
}

/// Secret-free result of an explicit CLI availability/version probe.
///
/// Raw stdout/stderr are intentionally omitted. They are bounded and redacted
/// by `cli_probe`, then reduced to a short version summary before crossing IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiCliProbeView {
    pub category: AiConfigCategory,
    pub entry_id: String,
    pub config_type: String,
    pub source: AiConfigSource,
    pub adapter: AiAdapterKind,
    pub program: String,
    pub available: bool,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub version: Option<String>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub diagnostics: Vec<CliProbeDiagnostic>,
}

pub fn resolve_active_ai_target(
    config: &AiConfig,
    category: AiConfigCategory,
) -> AdmResult<ResolvedAiTarget> {
    let category_config = category_config(config, category);
    if category_config.active_entry_id.trim().is_empty() {
        return Err(AdmError::new(format!(
            "{}.active_entry_id is empty",
            category.as_str()
        )));
    }
    let entry = category_config
        .entries
        .iter()
        .find(|entry| entry.id == category_config.active_entry_id)
        .ok_or_else(|| {
            AdmError::new(format!(
                "{}.active_entry_id does not exist: {}",
                category.as_str(),
                category_config.active_entry_id
            ))
        })?;
    resolve_ai_entry(entry, category)
}

pub fn resolve_active_ai_target_by_category_id(
    config: &AiConfig,
    category_id: &str,
) -> AdmResult<ResolvedAiTarget> {
    let category = AiConfigCategory::from_id(category_id)
        .ok_or_else(|| AdmError::new(format!("unsupported AI category: {category_id}")))?;
    resolve_active_ai_target(config, category)
}

pub fn resolve_all_active_ai_targets(config: &AiConfig) -> AdmResult<Vec<ResolvedAiTarget>> {
    [
        AiConfigCategory::Dev,
        AiConfigCategory::Image,
        AiConfigCategory::Completion,
    ]
    .into_iter()
    .map(|category| resolve_active_ai_target(config, category))
    .collect()
}

pub fn probe_active_ai_cli(
    config: &AiConfig,
    category: AiConfigCategory,
) -> AdmResult<AiCliProbeView> {
    let target = resolve_active_ai_target(config, category)?;
    if !target.descriptor.source.is_cli() {
        return Err(AdmError::new(format!(
            "active {} AI entry is not a CLI configuration",
            category.as_str()
        )));
    }
    let program = target.program.as_deref().ok_or_else(|| {
        AdmError::new(format!(
            "active {} AI CLI entry did not resolve a program",
            category.as_str()
        ))
    })?;
    let request = CliProbeRequest::new(program, ["--version"])
        .with_secrets(target.api_secret.iter().cloned());
    let report = probe_cli(&request);
    let version = probe_version_summary(&report.stdout, &report.stderr, report.success);
    let safe_program = safe_program_view(program);
    let diagnostics = report
        .diagnostics
        .into_iter()
        .map(|diagnostic| CliProbeDiagnostic {
            severity: diagnostic.severity,
            code: diagnostic.code,
            message: redact_raw_urls(&diagnostic.message),
        })
        .collect();
    Ok(AiCliProbeView {
        category: target.descriptor.category,
        entry_id: target.entry_id,
        config_type: target.descriptor.config_type.to_string(),
        source: target.descriptor.source,
        adapter: target.descriptor.adapter,
        program: safe_program,
        available: report.available,
        success: report.success,
        exit_code: report.exit_code,
        timed_out: report.timed_out,
        duration_ms: report.duration_ms,
        version,
        stdout_truncated: report.stdout_truncated,
        stderr_truncated: report.stderr_truncated,
        diagnostics,
    })
}

pub fn probe_active_ai_cli_by_category_id(
    config: &AiConfig,
    category_id: &str,
) -> AdmResult<AiCliProbeView> {
    let category = AiConfigCategory::from_id(category_id)
        .ok_or_else(|| AdmError::new(format!("unsupported AI category: {category_id}")))?;
    probe_active_ai_cli(config, category)
}

fn resolve_ai_entry(entry: &ApiEntry, category: AiConfigCategory) -> AdmResult<ResolvedAiTarget> {
    let descriptor = descriptor_for_config_type(&entry.config_type).ok_or_else(|| {
        AdmError::new(format!("unsupported AI config_type: {}", entry.config_type))
    })?;
    if descriptor.category != category {
        return Err(AdmError::new(format!(
            "AI config_type {} belongs to {}, not {}",
            descriptor.config_type,
            descriptor.category.as_str(),
            category.as_str()
        )));
    }

    let extra = entry.extra_json.as_object();
    let model = extra
        .and_then(|object| object.get("model"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let api_url = nonempty(&entry.api_url);
    let default_auth_mode = if descriptor
        .required_fields
        .contains(&AiRequiredField::ApiKey)
    {
        "direct"
    } else {
        "none"
    };
    let auth_mode = extra
        .and_then(|object| object.get("auth_mode"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_auth_mode)
        .to_ascii_lowercase();
    let mut auth_diagnostics = Vec::new();
    let api_secret = match auth_mode.as_str() {
        "direct" => nonempty(&entry.api_key),
        "env" => {
            let env_name = extra
                .and_then(|object| object.get("api_key_env"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            match env_name {
                Some(env_name) => match env::var(env_name) {
                    Ok(value) => nonempty(&value).or_else(|| {
                        auth_diagnostics.push(AiResolutionDiagnostic {
                            severity: AiResolutionSeverity::Error,
                            code: "api_auth_env_empty".to_string(),
                            message: format!(
                                "AI authentication environment variable is empty: {env_name}"
                            ),
                        });
                        None
                    }),
                    Err(_) => {
                        auth_diagnostics.push(AiResolutionDiagnostic {
                            severity: AiResolutionSeverity::Error,
                            code: "api_auth_env_missing".to_string(),
                            message: format!(
                                "AI authentication environment variable is not set: {env_name}"
                            ),
                        });
                        None
                    }
                },
                None => {
                    auth_diagnostics.push(AiResolutionDiagnostic {
                        severity: AiResolutionSeverity::Error,
                        code: "api_auth_env_name_missing".to_string(),
                        message: "AI auth_mode env requires extra_json.api_key_env".to_string(),
                    });
                    None
                }
            }
        }
        "none" => None,
        other => {
            auth_diagnostics.push(AiResolutionDiagnostic {
                severity: AiResolutionSeverity::Error,
                code: "api_auth_mode_invalid".to_string(),
                message: format!("unsupported AI authentication mode: {other}"),
            });
            None
        }
    };
    let mut diagnostics = required_field_diagnostics(
        descriptor,
        api_url.as_deref(),
        api_secret.as_deref(),
        model.as_deref(),
        auth_mode.as_str(),
    );
    if descriptor.source == AiConfigSource::Api
        && let Some(api_url) = api_url.as_deref()
        && let Err(error) = validate_http_transport_url(api_url)
    {
        diagnostics.push(AiResolutionDiagnostic {
            severity: AiResolutionSeverity::Error,
            code: "api_url_transport_not_allowed".to_string(),
            message: format!("AI API URL {error}"),
        });
    }
    diagnostics.extend(auth_diagnostics);
    let mut program = None;
    let mut cli_available = true;
    if descriptor.source.is_cli() {
        let default_program = descriptor.default_program.ok_or_else(|| {
            AdmError::new(format!(
                "AI descriptor {} has no default CLI program",
                descriptor.config_type
            ))
        })?;
        let explicit_program = extra
            .and_then(|object| object.get("cli_path"))
            .and_then(Value::as_str);
        let location = locate_cli_program(explicit_program, default_program);
        program = Some(location.program);
        cli_available = location.available;
        diagnostics.extend(location.diagnostics.into_iter().map(|message| {
            AiResolutionDiagnostic {
                severity: AiResolutionSeverity::Warning,
                code: "cli_unavailable".to_string(),
                message,
            }
        }));
    }
    let has_errors = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == AiResolutionSeverity::Error);

    Ok(ResolvedAiTarget {
        descriptor,
        entry_id: entry.id.clone(),
        api_url,
        api_secret,
        auth_mode,
        model,
        program,
        available: cli_available && !has_errors,
        diagnostics,
    })
}

fn category_config(config: &AiConfig, category: AiConfigCategory) -> &ApiCategory {
    match category {
        AiConfigCategory::Dev => &config.dev,
        AiConfigCategory::Image => &config.image,
        AiConfigCategory::Completion => &config.completion,
    }
}

fn required_field_diagnostics(
    descriptor: &AiConfigDescriptor,
    api_url: Option<&str>,
    api_secret: Option<&str>,
    model: Option<&str>,
    auth_mode: &str,
) -> Vec<AiResolutionDiagnostic> {
    descriptor
        .required_fields
        .iter()
        .filter_map(|required| {
            let present = match required {
                AiRequiredField::ApiUrl => api_url.is_some(),
                AiRequiredField::ApiKey => auth_mode == "none" || api_secret.is_some(),
                AiRequiredField::Model => model.is_some(),
            };
            (!present).then(|| AiResolutionDiagnostic {
                severity: AiResolutionSeverity::Error,
                code: "required_field_missing".to_string(),
                message: format!(
                    "required AI configuration field is missing: {}",
                    required.as_str()
                ),
            })
        })
        .collect()
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn safe_program_view(value: &str) -> String {
    if value.contains("://") {
        "***".to_string()
    } else {
        value.to_string()
    }
}

fn safe_program_debug_view(value: &str) -> String {
    if value
        .chars()
        .any(|character| matches!(character, '/' | '\\' | ':'))
    {
        "<configured-path>".to_string()
    } else {
        safe_program_view(value)
    }
}

fn probe_version_summary(stdout: &str, stderr: &str, success: bool) -> Option<String> {
    if !success {
        return None;
    }
    stdout
        .lines()
        .chain(stderr.lines())
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(512).collect::<String>())
        .map(|line| redact_raw_urls(&line))
}

fn redact_raw_urls(value: &str) -> String {
    value
        .split_whitespace()
        .map(|token| {
            if token.contains("://") {
                "[URL REDACTED]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_resolution_message(value: &str, secret: Option<&str>) -> String {
    let value = match secret.filter(|secret| !secret.is_empty()) {
        Some(secret) => value.replace(secret, "[REDACTED]"),
        None => value.to_string(),
    };
    redact_raw_urls(&value)
}

pub fn mask_api_url(value: &str) -> String {
    let value = value.trim();
    let Some((scheme, rest)) = value.split_once("://") else {
        return if value.is_empty() {
            String::new()
        } else {
            "***".to_string()
        };
    };
    let authority_end = rest
        .find(|character| matches!(character, '/' | '?' | '#'))
        .unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let safe_authority = authority.rsplit('@').next().unwrap_or_default();
    if scheme.is_empty() || safe_authority.is_empty() {
        return "***".to_string();
    }
    let has_path = rest[authority_end..].starts_with('/');
    format!(
        "{scheme}://{safe_authority}{}",
        if has_path { "/…" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use adm_new_config::{
        AiConfigCategory, CONFIG_TYPE_CODEX_CLI_IMAGE, CONFIG_TYPE_LOCAL_CODEX_CLI,
        CONFIG_TYPE_OPENAI_COMPLETION_API,
    };
    use adm_new_contracts::ai::{AiConfig, ApiCategory, ApiEntry};
    use serde_json::json;

    use super::*;

    #[test]
    fn each_category_resolves_its_own_active_entry() {
        let config = AiConfig {
            dev: category(
                "dev",
                "dev-active",
                entry("dev-active", CONFIG_TYPE_LOCAL_CODEX_CLI),
            ),
            image: category(
                "image",
                "image-active",
                entry("image-active", CONFIG_TYPE_CODEX_CLI_IMAGE),
            ),
            completion: category(
                "completion",
                "completion-active",
                ApiEntry {
                    id: "completion-active".to_string(),
                    config_type: CONFIG_TYPE_OPENAI_COMPLETION_API.to_string(),
                    api_url: "https://api.example.test/v1".to_string(),
                    api_key: "sk-secret-completion".to_string(),
                    extra_json: json!({"model": "gpt-test"}),
                    ..ApiEntry::default()
                },
            ),
            ..AiConfig::default()
        };

        let targets = resolve_all_active_ai_targets(&config).unwrap();
        assert_eq!(targets[0].entry_id(), "dev-active");
        assert_eq!(targets[1].entry_id(), "image-active");
        assert_eq!(targets[2].entry_id(), "completion-active");
        assert_eq!(
            targets[2].descriptor().category,
            AiConfigCategory::Completion
        );
        assert_eq!(targets[2].api_secret(), Some("sk-secret-completion"));
        let view = targets[2].view();
        assert_eq!(view.category, AiConfigCategory::Completion);
        assert_eq!(view.entry_id, "completion-active");
        assert_eq!(view.config_type, CONFIG_TYPE_OPENAI_COMPLETION_API);
        assert_eq!(view.source, AiConfigSource::Api);
        assert_eq!(view.adapter, AiAdapterKind::OpenAiCompatible);
        assert!(view.available);
    }

    #[test]
    fn resolution_rejects_a_config_type_from_another_category() {
        let config = AiConfig {
            dev: category("dev", "wrong", entry("wrong", CONFIG_TYPE_CODEX_CLI_IMAGE)),
            ..AiConfig::default()
        };

        let error = resolve_active_ai_target(&config, AiConfigCategory::Dev).unwrap_err();
        assert!(error.message().contains("belongs to image"));
    }

    #[test]
    fn debug_and_serializable_view_never_expose_the_secret_or_raw_url() {
        let secret = "sk-super-private";
        let config = AiConfig {
            completion: category(
                "completion",
                "api",
                ApiEntry {
                    id: "api".to_string(),
                    config_type: CONFIG_TYPE_OPENAI_COMPLETION_API.to_string(),
                    api_url: "https://user:password@api.example.test/private/token?key=hidden"
                        .to_string(),
                    api_key: secret.to_string(),
                    extra_json: json!({"model": "gpt-safe"}),
                    ..ApiEntry::default()
                },
            ),
            ..AiConfig::default()
        };

        let target = resolve_active_ai_target(&config, AiConfigCategory::Completion).unwrap();
        let debug = format!("{target:?}");
        let serialized = serde_json::to_string(&target.view()).unwrap();
        assert!(!debug.contains(secret));
        assert!(!debug.contains("password"));
        assert!(!debug.contains("/private/token"));
        assert!(!serialized.contains(secret));
        assert!(!serialized.contains("password"));
        assert!(!serialized.contains("hidden"));
        assert!(serialized.contains("https://api.example.test/…"));

        let value = serde_json::to_value(target.view()).unwrap();
        let keys = value
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec![
                "adapter",
                "auth_mode",
                "available",
                "capabilities",
                "category",
                "config_type",
                "diagnostics",
                "entry_id",
                "has_secret",
                "masked_url",
                "model",
                "program",
                "source",
            ]
        );
    }

    #[test]
    fn cli_probe_rejects_api_entries_without_executing_them() {
        let config = AiConfig {
            completion: category(
                "completion",
                "api",
                ApiEntry {
                    id: "api".to_string(),
                    config_type: CONFIG_TYPE_OPENAI_COMPLETION_API.to_string(),
                    api_url: "https://api.example.test/v1".to_string(),
                    api_key: "sk-private".to_string(),
                    extra_json: json!({"model": "gpt-test"}),
                    ..ApiEntry::default()
                },
            ),
            ..AiConfig::default()
        };

        let error = probe_active_ai_cli(&config, AiConfigCategory::Completion).unwrap_err();
        assert_eq!(
            error.message(),
            "active completion AI entry is not a CLI configuration"
        );
        assert!(!error.message().contains("sk-private"));
        assert!(!error.message().contains("https://"));
    }

    #[test]
    fn public_probe_text_never_contains_raw_urls() {
        assert_eq!(
            safe_program_view("https://user:secret@example.test/cli"),
            "***"
        );
        assert_eq!(
            probe_version_summary(
                "codex 1.2.3 from https://user:secret@example.test/config",
                "",
                true,
            )
            .as_deref(),
            Some("codex 1.2.3 from [URL REDACTED]")
        );

        let config = AiConfig {
            dev: category(
                "dev",
                "cli",
                ApiEntry {
                    id: "cli".to_string(),
                    config_type: CONFIG_TYPE_LOCAL_CODEX_CLI.to_string(),
                    api_key: "configured-secret".to_string(),
                    extra_json: json!({
                        "cli_path": "https://user:password@example.test/configured-secret"
                    }),
                    ..ApiEntry::default()
                },
            ),
            ..AiConfig::default()
        };
        let view = resolve_active_ai_target(&config, AiConfigCategory::Dev)
            .unwrap()
            .view();
        let serialized = serde_json::to_string(&view).unwrap();
        assert!(!serialized.contains("configured-secret"));
        assert!(!serialized.contains("password"));
        assert!(!serialized.contains("https://"));
    }

    #[test]
    fn missing_required_api_values_are_resolution_diagnostics() {
        let config = AiConfig {
            completion: category(
                "completion",
                "api",
                entry("api", CONFIG_TYPE_OPENAI_COMPLETION_API),
            ),
            ..AiConfig::default()
        };

        let target = resolve_active_ai_target(&config, AiConfigCategory::Completion).unwrap();
        assert!(!target.is_available());
        assert_eq!(target.diagnostics().len(), 3);
        assert!(
            target
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.severity == AiResolutionSeverity::Error)
        );
    }

    #[test]
    fn explicit_no_auth_api_target_is_available_without_a_secret() {
        let config = AiConfig {
            completion: category(
                "completion",
                "local",
                ApiEntry {
                    id: "local".to_string(),
                    config_type: CONFIG_TYPE_OPENAI_COMPLETION_API.to_string(),
                    api_url: "http://127.0.0.1:11434/v1".to_string(),
                    extra_json: json!({"model": "local-model", "auth_mode": "none"}),
                    ..ApiEntry::default()
                },
            ),
            ..AiConfig::default()
        };

        let target = resolve_active_ai_target(&config, AiConfigCategory::Completion).unwrap();
        assert!(target.is_available(), "{:?}", target.diagnostics());
        assert_eq!(target.auth_mode(), "none");
        assert_eq!(target.api_secret(), None);
        assert!(!target.view().has_secret);
    }

    #[test]
    fn loopback_http_api_targets_are_available() {
        for api_url in [
            "http://localhost:11434/v1",
            "http://127.99.0.7:11434/v1",
            "http://[::1]:11434/v1",
        ] {
            let config = AiConfig {
                completion: category(
                    "completion",
                    "local",
                    ApiEntry {
                        id: "local".to_string(),
                        config_type: CONFIG_TYPE_OPENAI_COMPLETION_API.to_string(),
                        api_url: api_url.to_string(),
                        extra_json: json!({"model": "local-model", "auth_mode": "none"}),
                        ..ApiEntry::default()
                    },
                ),
                ..AiConfig::default()
            };

            let target = resolve_active_ai_target(&config, AiConfigCategory::Completion).unwrap();
            assert!(
                target.is_available(),
                "{api_url}: {:?}",
                target.diagnostics()
            );
        }
    }

    #[test]
    fn remote_http_api_target_is_unavailable_with_stable_diagnostic() {
        let config = AiConfig {
            completion: category(
                "completion",
                "remote",
                ApiEntry {
                    id: "remote".to_string(),
                    config_type: CONFIG_TYPE_OPENAI_COMPLETION_API.to_string(),
                    api_url: "http://api.private-example.test/v1".to_string(),
                    extra_json: json!({"model": "remote-model", "auth_mode": "none"}),
                    ..ApiEntry::default()
                },
            ),
            ..AiConfig::default()
        };

        let target = resolve_active_ai_target(&config, AiConfigCategory::Completion).unwrap();
        assert!(!target.is_available());
        let diagnostic = target
            .diagnostics()
            .iter()
            .find(|diagnostic| diagnostic.code == "api_url_transport_not_allowed")
            .expect("transport policy diagnostic");
        assert!(diagnostic.message.contains("must use HTTPS"));
        assert!(!diagnostic.message.contains("private-example"));
    }

    #[test]
    fn missing_environment_authentication_is_a_run_blocker_without_leaking_values() {
        let config = AiConfig {
            completion: category(
                "completion",
                "env",
                ApiEntry {
                    id: "env".to_string(),
                    config_type: CONFIG_TYPE_OPENAI_COMPLETION_API.to_string(),
                    api_url: "https://api.example.test/v1".to_string(),
                    extra_json: json!({
                        "model": "gpt-test",
                        "auth_mode": "env",
                        "api_key_env": "ADM_TEST_ENV_THAT_MUST_NOT_EXIST_7D3C"
                    }),
                    ..ApiEntry::default()
                },
            ),
            ..AiConfig::default()
        };

        let target = resolve_active_ai_target(&config, AiConfigCategory::Completion).unwrap();
        assert!(!target.is_available());
        assert!(
            target
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "api_auth_env_missing")
        );
    }

    #[test]
    fn url_masking_preserves_only_safe_origin_information() {
        assert_eq!(
            mask_api_url("https://user:secret@example.test:8443/v1/models?key=secret"),
            "https://example.test:8443/…"
        );
        assert_eq!(mask_api_url("token-only"), "***");
        assert_eq!(mask_api_url(""), "");
    }

    fn category(category_id: &str, active_entry_id: &str, entry: ApiEntry) -> ApiCategory {
        ApiCategory {
            category_id: category_id.to_string(),
            entries: vec![entry],
            active_entry_id: active_entry_id.to_string(),
        }
    }

    fn entry(id: &str, config_type: &str) -> ApiEntry {
        ApiEntry {
            id: id.to_string(),
            label: id.to_string(),
            config_type: config_type.to_string(),
            ..ApiEntry::default()
        }
    }
}
