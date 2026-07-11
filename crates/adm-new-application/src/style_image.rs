use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use adm_new_ai::image::{ImageApiSettings, codex_home};
use adm_new_ai::image_execution::{
    BlockingOpenAiImageExecutor, CodexCliImageExecutor, ImageExecutionRequest, ImageExecutor,
};
use adm_new_ai::resolution::resolve_active_ai_target;
use adm_new_ai::{AiAdapterKind, AiConfigCategory, AiConfigSource};
use adm_new_contracts::ai::{AiConfig, ApiEntry};
use adm_new_foundation::{AdmError, AdmResult, sha256_hex};
use adm_new_pipeline::{
    StyleImageGenerator, StyleImageRequest, StyleImageResult, StyleImageStatus,
};
use serde_json::Value;

#[derive(Clone)]
pub struct AiStyleImageGenerator {
    executor: Arc<dyn ImageExecutor>,
    execution_scope: String,
}

impl AiStyleImageGenerator {
    pub fn new(executor: Arc<dyn ImageExecutor>) -> Self {
        Self {
            executor,
            execution_scope: "style-image-generator-v1".to_string(),
        }
    }

    pub fn with_execution_scope(mut self, execution_scope: impl Into<String>) -> Self {
        self.execution_scope = execution_scope.into();
        self
    }
}

impl fmt::Debug for AiStyleImageGenerator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AiStyleImageGenerator")
            .field("executor", &"configured")
            .finish()
    }
}

impl StyleImageGenerator for AiStyleImageGenerator {
    fn execution_scope_fingerprint(&self) -> String {
        self.execution_scope.clone()
    }

    fn generate(&self, request: &StyleImageRequest) -> AdmResult<StyleImageResult> {
        let execution = ImageExecutionRequest::png(
            request.unit_id.clone(),
            request.prompt.clone(),
            request.requested_width,
            request.requested_height,
        );
        match self.executor.execute(&execution) {
            Ok(result) => Ok(StyleImageResult::generated(
                result.bytes,
                result.provider,
                result.model,
                result.width,
                result.height,
            )),
            Err(_) => Ok(StyleImageResult {
                status: StyleImageStatus::Failed,
                bytes: None,
                provider: String::new(),
                model: String::new(),
                width: 0,
                height: 0,
                format: "png".to_string(),
                reason_code: "image_provider_failed".to_string(),
                safe_message:
                    "image provider request failed; verify the active image configuration"
                        .to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone)]
struct UnavailableStyleImageGenerator {
    code: String,
    message: String,
    execution_scope: String,
}

impl StyleImageGenerator for UnavailableStyleImageGenerator {
    fn execution_scope_fingerprint(&self) -> String {
        self.execution_scope.clone()
    }

    fn generate(&self, _request: &StyleImageRequest) -> AdmResult<StyleImageResult> {
        Ok(StyleImageResult::unavailable(
            self.code.clone(),
            self.message.clone(),
        ))
    }
}

pub fn style_image_generator_from_config(
    config: &AiConfig,
    project_root: &Path,
) -> AdmResult<Arc<dyn StyleImageGenerator>> {
    let target = match resolve_active_ai_target(config, AiConfigCategory::Image) {
        Ok(target) => target,
        Err(error) => {
            return Ok(unavailable(
                "image_configuration_unavailable",
                error.message(),
            ));
        }
    };
    if !target.is_available() {
        return Ok(unavailable(
            "image_configuration_unavailable",
            target
                .diagnostics()
                .first()
                .map(|diagnostic| diagnostic.message.as_str())
                .unwrap_or("active image configuration is unavailable"),
        ));
    }
    let entry = config
        .image
        .entries
        .iter()
        .find(|entry| entry.id == target.entry_id())
        .ok_or_else(|| AdmError::new("resolved image entry disappeared"))?;
    if target.descriptor().source == AiConfigSource::CliBuiltin
        && target.descriptor().adapter == AiAdapterKind::Codex
    {
        let timeout_seconds = entry
            .extra_json
            .get("timeout_seconds")
            .or_else(|| entry.extra_json.get("timeout"))
            .and_then(Value::as_u64)
            .unwrap_or(300)
            .clamp(1, 1_800);
        let home_override = entry.extra_json.get("codex_home").and_then(Value::as_str);
        let executor = CodexCliImageExecutor::new(
            target
                .program()
                .ok_or_else(|| AdmError::new("resolved Codex CLI program is missing"))?,
            project_root,
            codex_home(home_override),
        )?
        .with_provider_name(entry.id.clone())?
        .with_timeout(Duration::from_secs(timeout_seconds))?;
        return Ok(Arc::new(
            AiStyleImageGenerator::new(Arc::new(executor))
                .with_execution_scope(image_execution_scope(entry, &target)),
        ));
    }
    if target.descriptor().source != AiConfigSource::Api {
        return Ok(unavailable(
            "image_adapter_not_supported",
            "the active image adapter is not supported",
        ));
    }
    if !matches!(
        target.descriptor().adapter,
        AiAdapterKind::OpenAiImage | AiAdapterKind::CustomImage
    ) {
        return Ok(unavailable(
            "image_adapter_not_supported",
            "the active image API adapter is not supported by the OpenAI image executor",
        ));
    }
    let settings = image_settings_from_resolved(entry, &target)?;
    let timeout_seconds = entry
        .extra_json
        .get("timeout_seconds")
        .or_else(|| entry.extra_json.get("timeout"))
        .and_then(Value::as_u64)
        .unwrap_or(300)
        .clamp(1, 1_800);
    let executor = BlockingOpenAiImageExecutor::blocking(settings)?
        .with_timeout(Duration::from_secs(timeout_seconds))?;
    Ok(Arc::new(
        AiStyleImageGenerator::new(Arc::new(executor))
            .with_execution_scope(image_execution_scope(entry, &target)),
    ))
}

fn image_settings_from_resolved(
    entry: &ApiEntry,
    target: &adm_new_ai::resolution::ResolvedAiTarget,
) -> AdmResult<ImageApiSettings> {
    let extra = entry.extra_json.as_object();
    let mode = extra_string(extra, "mode").unwrap_or_else(|| "responses_image_tool".to_string());
    if !matches!(mode.as_str(), "responses_image_tool" | "images_generations") {
        return Err(AdmError::new(format!(
            "unsupported resolved image API mode: {mode}"
        )));
    }
    let endpoint = extra_string(extra, "endpoint").unwrap_or_else(|| {
        if mode == "responses_image_tool" {
            "responses".to_string()
        } else {
            "images/generations".to_string()
        }
    });
    let api_url = target
        .api_url()
        .ok_or_else(|| AdmError::new("resolved image API URL is missing"))?;
    let image_model = target
        .model()
        .ok_or_else(|| AdmError::new("resolved image model is missing"))?;
    Ok(ImageApiSettings {
        name: entry.id.clone(),
        provider: extra_string(extra, "provider").unwrap_or_else(|| "openai".to_string()),
        mode,
        api_key: target.api_secret().unwrap_or_default().to_string(),
        base_url: normalized_openai_base_url(api_url),
        image_model: image_model.to_string(),
        response_model: extra_string(extra, "response_model"),
        endpoint: Some(endpoint),
        enabled: extra
            .and_then(|object| object.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
    })
}

fn extra_string(extra: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<String> {
    extra
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalized_openai_base_url(value: &str) -> String {
    let value = value.trim().trim_end_matches('/');
    if value.ends_with("/v1") {
        value.to_string()
    } else {
        format!("{value}/v1")
    }
}

fn unavailable(code: &str, message: &str) -> Arc<dyn StyleImageGenerator> {
    Arc::new(UnavailableStyleImageGenerator {
        code: code.to_string(),
        message: safe_message(message),
        execution_scope: sha256_hex(format!("unavailable-style-image-v1:{code}").as_bytes()),
    })
}

fn image_execution_scope(
    entry: &ApiEntry,
    target: &adm_new_ai::resolution::ResolvedAiTarget,
) -> String {
    sha256_hex(
        format!(
            "style-image-v2|{}|{}|{}|{}|{}|{}",
            entry.id,
            entry.config_type,
            target.descriptor().source.as_str(),
            target.descriptor().adapter.as_str(),
            target.api_url().unwrap_or_default(),
            target
                .model()
                .or_else(|| target.program())
                .unwrap_or_default(),
        )
        .as_bytes(),
    )
}

fn safe_message(message: &str) -> String {
    let flattened = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut safe = flattened.chars().take(240).collect::<String>();
    for token in safe
        .split_whitespace()
        .filter(|token| token.len() >= 12 && token.starts_with("sk-"))
        .map(str::to_string)
        .collect::<Vec<_>>()
    {
        safe = safe.replace(&token, "[REDACTED]");
    }
    safe
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::ai::ApiCategory;
    use serde_json::json;

    #[test]
    fn incomplete_image_configuration_becomes_explicit_unavailable_generator() {
        let config = AiConfig {
            image: ApiCategory {
                category_id: "image".to_string(),
                active_entry_id: "api".to_string(),
                entries: vec![ApiEntry {
                    id: "api".to_string(),
                    config_type: "openai_image_api".to_string(),
                    ..ApiEntry::default()
                }],
            },
            ..AiConfig::default()
        };
        let root = std::env::temp_dir();
        let generator = style_image_generator_from_config(&config, &root).unwrap();
        let result = generator
            .generate(&StyleImageRequest {
                unit_id: "07:image:test".to_string(),
                style_id: "test".to_string(),
                prompt: "test prompt".to_string(),
                project_label: "test".to_string(),
                requested_width: 640,
                requested_height: 384,
                output_format: "png".to_string(),
            })
            .unwrap();
        assert_eq!(result.status, StyleImageStatus::Unavailable);
        assert_eq!(result.reason_code, "image_configuration_unavailable");
    }

    #[test]
    fn openai_image_configuration_uses_the_resolved_active_entry() {
        let config = AiConfig {
            image: ApiCategory {
                category_id: "image".to_string(),
                active_entry_id: "api".to_string(),
                entries: vec![ApiEntry {
                    id: "api".to_string(),
                    config_type: "openai_image_api".to_string(),
                    api_url: "https://api.example.test/v1".to_string(),
                    api_key: "sk-secret".to_string(),
                    extra_json: json!({
                        "model": "gpt-image-test",
                        "mode": "images_generations",
                        "timeout_seconds": 7
                    }),
                    ..ApiEntry::default()
                }],
            },
            ..AiConfig::default()
        };
        let generator = style_image_generator_from_config(&config, &std::env::temp_dir()).unwrap();
        let debug = format!("{generator:?}");
        assert!(!debug.contains("sk-secret"));
    }
}
