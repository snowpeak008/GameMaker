use std::sync::Arc;

use adm_new_ai::adapters::{
    OpenAiCompletionTransport, OpenAiRequestSpec, blocking_openai_adapter_from_resolved,
    completion_text_from_openai_response,
};
use adm_new_ai::resolution::resolve_active_ai_target;
use adm_new_ai::{AiAdapterKind, AiConfigCategory, AiConfigSource};
use adm_new_contracts::ai::AiConfig;
use adm_new_foundation::{AdmError, AdmResult, sha256_hex};
use adm_new_pipeline::stages::step07_v2::{
    CachedVlmReviewService, VlmImageReviewer, VlmReviewEvidence, VlmReviewRequest,
    VlmReviewService, VlmReviewStatus,
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct OpenAiVisionVlmReviewer {
    settings: adm_new_ai::adapters::OpenAiCompletionSettings,
    transport: adm_new_ai::adapters::ReqwestOpenAiTransport,
}

impl OpenAiVisionVlmReviewer {
    fn new(
        settings: adm_new_ai::adapters::OpenAiCompletionSettings,
        transport: adm_new_ai::adapters::ReqwestOpenAiTransport,
    ) -> Self {
        Self {
            settings,
            transport,
        }
    }
}

impl VlmImageReviewer for OpenAiVisionVlmReviewer {
    fn review_uncached(
        &self,
        request: &VlmReviewRequest,
        image_hash: &str,
        config_id: &str,
    ) -> AdmResult<VlmReviewEvidence> {
        let image_bytes = std::fs::read(&request.image_path).map_err(|error| {
            AdmError::new(format!(
                "failed to read VLM review image {}: {error}",
                request.image_path.display()
            ))
        })?;
        let payload = vlm_review_payload(&self.settings.model, request, &image_bytes);
        let response = self.transport.post_chat_completion(&OpenAiRequestSpec {
            endpoint: format!(
                "{}/chat/completions",
                self.settings.base_url.trim_end_matches('/')
            ),
            headers: openai_headers(&self.settings.api_key),
            payload,
            timeout_seconds: self.settings.timeout_seconds,
        })?;
        let text = completion_text_from_openai_response(&response)?;
        Ok(parse_vlm_review_response(
            &text,
            image_hash,
            config_id,
            "openai_compatible_vision",
        ))
    }
}

pub fn vlm_review_service_from_config(config: &AiConfig) -> AdmResult<Arc<dyn VlmReviewService>> {
    let target = resolve_active_ai_target(config, AiConfigCategory::Completion)?;
    if !target.is_available() {
        return Err(AdmError::new(format!(
            "active completion VLM target is unavailable: {}",
            target
                .diagnostics()
                .first()
                .map(|diagnostic| diagnostic.message.as_str())
                .unwrap_or("resolution failed")
        )));
    }
    if target.descriptor().source != AiConfigSource::Api
        || target.descriptor().adapter != AiAdapterKind::OpenAiCompatible
    {
        return Err(AdmError::new(
            "VLM review requires an OpenAI-compatible completion API configuration",
        ));
    }
    let entry = config
        .completion
        .entries
        .iter()
        .find(|entry| entry.id == target.entry_id())
        .ok_or_else(|| AdmError::new("resolved completion entry disappeared"))?;
    let adapter = blocking_openai_adapter_from_resolved(&target, entry)?;
    let reviewer = OpenAiVisionVlmReviewer::new(adapter.settings, adapter.transport);
    Ok(Arc::new(CachedVlmReviewService::with_reviewer(
        format!(
            "completion:{}:{}",
            target.entry_id(),
            target.model().unwrap_or("model")
        ),
        Arc::new(reviewer),
    )))
}

fn vlm_review_payload(model: &str, request: &VlmReviewRequest, image_bytes: &[u8]) -> Value {
    let prompt = format!(
        "You are a visual quality reviewer for an automated game asset pipeline.\n\
Return only JSON with fields: status ('passed' or 'failed'), score (0-100 integer), message (short string), differences (array of strings).\n\
Do not approve images with unreadable silhouettes, obvious watermark/text artifacts, missing alpha/cutout expectations, inconsistent style, or mismatched asset purpose.\n\
Asset id: {}\nExpected content hash: {}\nSource refs: {}\nReview context: {}",
        request.asset_id,
        request.content_hash,
        request.source_refs.join(", "),
        request.review_context
    );
    let encoded = BASE64_STANDARD.encode(image_bytes);
    json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/png;base64,{encoded}")
                        }
                    }
                ]
            }
        ],
        "response_format": {"type": "json_object"}
    })
}

fn openai_headers(api_key: &str) -> std::collections::BTreeMap<String, String> {
    let mut headers = std::collections::BTreeMap::new();
    if !api_key.trim().is_empty() {
        headers.insert("Authorization".to_string(), format!("Bearer {api_key}"));
    }
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    headers
}

fn parse_vlm_review_response(
    text: &str,
    image_hash: &str,
    config_id: &str,
    reviewer_kind: &str,
) -> VlmReviewEvidence {
    let parsed = parse_json_object_from_text(text);
    let (status, score, message, differences) = match parsed {
        Ok(value) => {
            let status = match value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str()
            {
                "passed" | "pass" => VlmReviewStatus::Passed,
                "failed" | "fail" | "rejected" => VlmReviewStatus::Failed,
                _ => VlmReviewStatus::Failed,
            };
            let score = value
                .get("score")
                .and_then(Value::as_u64)
                .and_then(|score| u8::try_from(score.min(100)).ok())
                .unwrap_or(0);
            let message = value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("VLM review response omitted a message")
                .trim()
                .to_string();
            let differences = value
                .get("differences")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::trim)
                        .filter(|item| !item.is_empty())
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (status, score, message, differences)
        }
        Err(error) => (
            VlmReviewStatus::Failed,
            0,
            format!("VLM review response was not valid JSON: {error}"),
            vec![text.chars().take(240).collect::<String>()],
        ),
    };
    let summary = json!({
        "status": status,
        "score": score,
        "message": message,
        "differences": differences,
        "imageHash": image_hash,
        "configId": config_id,
    });
    VlmReviewEvidence {
        image_hash: image_hash.to_string(),
        config_id: config_id.to_string(),
        summary_hash: sha256_hex(summary.to_string().as_bytes()),
        status,
        reviewer_kind: reviewer_kind.to_string(),
        message,
        score,
        differences,
        cache_hit: false,
    }
}

fn parse_json_object_from_text(text: &str) -> AdmResult<Value> {
    let trimmed = text.trim();
    if let Ok(value @ Value::Object(_)) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }
    if let Some(fenced) = trimmed
        .strip_prefix("```json")
        .and_then(|rest| rest.strip_suffix("```"))
        .or_else(|| {
            trimmed
                .strip_prefix("```")
                .and_then(|rest| rest.strip_suffix("```"))
        })
        && let Ok(value @ Value::Object(_)) = serde_json::from_str::<Value>(fenced.trim())
    {
        return Ok(value);
    }
    let start = trimmed.find('{');
    let end = trimmed.rfind('}');
    if let (Some(start), Some(end)) = (start, end)
        && start <= end
        && let Ok(value @ Value::Object(_)) = serde_json::from_str::<Value>(&trimmed[start..=end])
    {
        return Ok(value);
    }
    Err(AdmError::new("no JSON object found"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::ai::{ApiCategory, ApiEntry};

    #[test]
    fn vlm_payload_embeds_png_data_url_and_strict_json_instruction() {
        let request = VlmReviewRequest {
            asset_id: "plant_unit".to_string(),
            image_path: "unit.png".into(),
            content_hash: "h".repeat(64),
            source_refs: vec!["asset:plant_unit".to_string()],
            review_context: "lane defense unit".to_string(),
        };

        let payload = vlm_review_payload("gpt-vision-test", &request, &[1, 2, 3]);

        assert_eq!(payload["model"], "gpt-vision-test");
        assert_eq!(payload["response_format"]["type"], "json_object");
        let content = payload["messages"][0]["content"].as_array().unwrap();
        assert!(
            content[0]["text"]
                .as_str()
                .unwrap()
                .contains("Return only JSON")
        );
        assert_eq!(
            content[1]["image_url"]["url"],
            format!(
                "data:image/png;base64,{}",
                BASE64_STANDARD.encode([1, 2, 3])
            )
        );
    }

    #[test]
    fn vlm_response_parser_fails_closed_on_invalid_or_rejected_json() {
        let failed = parse_vlm_review_response(
            r#"{"status":"failed","score":31,"message":"too blurry","differences":["blur"]}"#,
            "image-hash",
            "config",
            "test",
        );
        assert_eq!(failed.status, VlmReviewStatus::Failed);
        assert_eq!(failed.score, 31);
        assert_eq!(failed.differences, vec!["blur".to_string()]);

        let invalid = parse_vlm_review_response("not json", "image-hash", "config", "test");
        assert_eq!(invalid.status, VlmReviewStatus::Failed);
        assert!(invalid.message.contains("not valid JSON"));
    }

    #[test]
    fn vlm_service_factory_accepts_only_openai_compatible_completion_api() {
        let mut config = AiConfig::default();
        config.completion = ApiCategory {
            category_id: "completion".to_string(),
            active_entry_id: "vision-api".to_string(),
            entries: vec![ApiEntry {
                id: "vision-api".to_string(),
                label: "Vision API".to_string(),
                config_type: "openai_completion_api".to_string(),
                api_url: "https://api.example.test/v1".to_string(),
                api_key: "sk-test".to_string(),
                extra_json: json!({"model": "gpt-vision-test"}),
                ..ApiEntry::default()
            }],
        };

        let service = vlm_review_service_from_config(&config).unwrap();
        assert!(service.config_id().contains("vision-api"));

        config.completion.entries[0].config_type = "local_claude_completion_cli".to_string();
        config.completion.entries[0].api_url.clear();
        config.completion.entries[0].api_key.clear();
        config.completion.entries[0].extra_json = json!({"program": "claude"});

        let error = vlm_review_service_from_config(&config).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("OpenAI-compatible completion API")
                || error.to_string().contains("unavailable")
        );
    }
}
