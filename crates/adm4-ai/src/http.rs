use crate::provider::{AiCapability, AiProvider, AiRequest, AiResponse};
use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpProviderConfig {
    pub provider_id: String,
    pub base_url: String,
    pub model: String,
    /// 密钥引用文本（env:NAME / named:NAME），解析后的密钥只存内存。
    pub api_key_ref: String,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

fn default_timeout_secs() -> u64 {
    120
}

/// 内置 preset（openai / openrouter / deepseek / local_openai）。
pub fn provider_presets() -> Vec<HttpProviderConfig> {
    vec![
        HttpProviderConfig {
            provider_id: "openai".into(),
            base_url: "https://api.openai.com/v1".into(),
            model: "gpt-4o-mini".into(),
            api_key_ref: "env:OPENAI_API_KEY".into(),
            timeout_secs: 120,
        },
        HttpProviderConfig {
            provider_id: "openrouter".into(),
            base_url: "https://openrouter.ai/api/v1".into(),
            model: "openai/gpt-4o-mini".into(),
            api_key_ref: "env:OPENROUTER_API_KEY".into(),
            timeout_secs: 120,
        },
        HttpProviderConfig {
            provider_id: "deepseek".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            model: "deepseek-chat".into(),
            api_key_ref: "env:DEEPSEEK_API_KEY".into(),
            timeout_secs: 120,
        },
        HttpProviderConfig {
            provider_id: "local_openai".into(),
            base_url: "http://127.0.0.1:1234/v1".into(),
            model: "local-model".into(),
            api_key_ref: "env:LOCAL_OPENAI_API_KEY".into(),
            timeout_secs: 300,
        },
    ]
}

/// OpenAI 兼容 chat completions Provider（阻塞式）。
pub struct OpenAiCompatibleProvider {
    config: HttpProviderConfig,
    api_key: String,
    capabilities: Vec<AiCapability>,
    client: reqwest::blocking::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: HttpProviderConfig, api_key: String) -> Adm4Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|error| Adm4Error::internal(format!("http client build failed: {error}")))?;
        Ok(Self {
            config,
            api_key,
            capabilities: vec![
                AiCapability::Text,
                AiCapability::Structured,
                AiCapability::Review,
            ],
            client,
        })
    }
}

impl AiProvider for OpenAiCompatibleProvider {
    fn id(&self) -> &str {
        &self.config.provider_id
    }

    fn capabilities(&self) -> &[AiCapability] {
        &self.capabilities
    }

    fn invoke(&self, request: &AiRequest) -> Adm4Result<AiResponse> {
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let mut body = json!({
            "model": self.config.model,
            "messages": [
                {"role": "system", "content": request.system_prompt},
                {"role": "user", "content": request.user_prompt},
            ],
        });
        if request.expect_json {
            body["response_format"] = json!({"type": "json_object"});
        }
        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .map_err(|error| Adm4Error::ai_unavailable(format!("ai request failed: {error}")))?;
        let status = response.status();
        let payload: serde_json::Value = response.json().map_err(|error| {
            Adm4Error::ai_unavailable(format!("ai response parse failed: {error}"))
        })?;
        if !status.is_success() {
            return Err(Adm4Error::ai_unavailable(format!(
                "ai provider returned status {status}: {}",
                payload
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(|message| message.as_str())
                    .unwrap_or("unknown error")
            )));
        }
        let text = payload
            .get("choices")
            .and_then(|choices| choices.get(0))
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|content| content.as_str())
            .ok_or_else(|| Adm4Error::ai_unavailable("ai response missing content"))?
            .to_string();
        Ok(AiResponse {
            text,
            provider_id: self.config.provider_id.clone(),
            model: self.config.model.clone(),
        })
    }
}
