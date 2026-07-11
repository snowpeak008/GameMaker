use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::time::{Duration, Instant};

use adm_new_config::{AiAdapterKind, AiConfigCategory, AiConfigSource, openai_endpoint};
use adm_new_contracts::ai::AiConfig;
use adm_new_foundation::{AdmError, AdmResult};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::redirect::Policy;
use serde::{Deserialize, Serialize};

use crate::http_endpoint_policy::validate_http_transport_url;
use crate::resolution::{
    AiResolutionDiagnostic, AiResolutionSeverity, mask_api_url,
    resolve_active_ai_target_by_category_id,
};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;

#[derive(Clone)]
pub struct AiApiProbeRequest {
    endpoint: String,
    headers: BTreeMap<String, String>,
    timeout: Duration,
}

impl fmt::Debug for AiApiProbeRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AiApiProbeRequest")
            .field("endpoint", &mask_api_url(&self.endpoint))
            .field("header_names", &self.headers.keys().collect::<Vec<_>>())
            .field("timeout", &self.timeout)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiApiProbeResponse {
    pub status: u16,
}

pub trait AiApiProbeTransport: Send + Sync {
    fn execute(&self, request: &AiApiProbeRequest) -> AdmResult<AiApiProbeResponse>;
}

#[derive(Debug, Clone)]
pub struct ReqwestAiApiProbeTransport {
    client: Client,
}

impl ReqwestAiApiProbeTransport {
    pub fn new() -> AdmResult<Self> {
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| AdmError::new("failed to initialize AI API probe transport"))?;
        Ok(Self { client })
    }
}

impl AiApiProbeTransport for ReqwestAiApiProbeTransport {
    fn execute(&self, request: &AiApiProbeRequest) -> AdmResult<AiApiProbeResponse> {
        validate_api_probe_endpoint(&request.endpoint)?;
        let mut headers = HeaderMap::new();
        for (name, value) in &request.headers {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| AdmError::new("AI API probe header name is invalid"))?;
            let value = HeaderValue::from_str(value)
                .map_err(|_| AdmError::new("AI API probe header value is invalid"))?;
            headers.insert(name, value);
        }
        let mut response = self
            .client
            .get(&request.endpoint)
            .headers(headers)
            .timeout(request.timeout)
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    AdmError::new("AI API probe timed out")
                } else {
                    AdmError::new(format!(
                        "AI API probe request failed for {}",
                        mask_api_url(&request.endpoint)
                    ))
                }
            })?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES)
        {
            return Err(AdmError::new(
                "AI API probe response exceeded the size limit",
            ));
        }
        let status = response.status().as_u16();
        let mut sink = Vec::new();
        response
            .by_ref()
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut sink)
            .map_err(|_| AdmError::new("AI API probe response could not be read"))?;
        if sink.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(AdmError::new(
                "AI API probe response exceeded the size limit",
            ));
        }
        Ok(AiApiProbeResponse { status })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiApiProbeView {
    pub category: AiConfigCategory,
    pub entry_id: String,
    pub config_type: String,
    pub adapter: AiAdapterKind,
    pub endpoint: String,
    pub available: bool,
    pub status_code: Option<u16>,
    pub duration_ms: u64,
    pub diagnostics: Vec<AiResolutionDiagnostic>,
}

pub fn probe_active_ai_api_by_category_id(
    config: &AiConfig,
    category_id: &str,
) -> AdmResult<AiApiProbeView> {
    let transport = ReqwestAiApiProbeTransport::new()?;
    probe_active_ai_api_with_transport(config, category_id, &transport)
}

pub fn probe_active_ai_api_with_transport(
    config: &AiConfig,
    category_id: &str,
    transport: &dyn AiApiProbeTransport,
) -> AdmResult<AiApiProbeView> {
    let target = resolve_active_ai_target_by_category_id(config, category_id)?;
    if target.descriptor().source != AiConfigSource::Api {
        return Err(AdmError::new(
            "the active AI configuration is not an API target",
        ));
    }
    let base_url = target
        .api_url()
        .ok_or_else(|| AdmError::new("the active AI API URL is missing"))?;
    let endpoint = match target.descriptor().adapter {
        AiAdapterKind::SdWebUi => format!("{}/sd-models", base_url.trim_end_matches('/')),
        _ => openai_endpoint(base_url, "models"),
    };
    validate_api_probe_endpoint(&endpoint)?;
    let mut headers = BTreeMap::new();
    if let Some(secret) = target.api_secret() {
        headers.insert("Authorization".to_string(), format!("Bearer {secret}"));
    }
    let request = AiApiProbeRequest {
        endpoint: endpoint.clone(),
        headers,
        timeout: DEFAULT_TIMEOUT,
    };
    let started = Instant::now();
    let response = transport.execute(&request);
    let duration_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let (available, status_code, diagnostics) = match response {
        Ok(response) if (200..300).contains(&response.status) => {
            (true, Some(response.status), Vec::new())
        }
        Ok(response) => {
            let code = match response.status {
                401 | 403 => "api_authentication_failed",
                404 => "api_probe_endpoint_missing",
                429 => "api_rate_limited",
                _ => "api_http_error",
            };
            (
                false,
                Some(response.status),
                vec![AiResolutionDiagnostic {
                    severity: AiResolutionSeverity::Error,
                    code: code.to_string(),
                    message: format!("AI API probe returned HTTP {}", response.status),
                }],
            )
        }
        Err(error) => (
            false,
            None,
            vec![AiResolutionDiagnostic {
                severity: AiResolutionSeverity::Error,
                code: if error.message().contains("timed out") {
                    "api_probe_timeout".to_string()
                } else {
                    "api_probe_failed".to_string()
                },
                message: error.message().to_string(),
            }],
        ),
    };
    Ok(AiApiProbeView {
        category: target.descriptor().category,
        entry_id: target.entry_id().to_string(),
        config_type: target.descriptor().config_type.to_string(),
        adapter: target.descriptor().adapter,
        endpoint: mask_api_url(&endpoint),
        available,
        status_code,
        duration_ms,
        diagnostics,
    })
}

fn validate_api_probe_endpoint(endpoint: &str) -> AdmResult<()> {
    let url = validate_http_transport_url(endpoint)
        .map_err(|error| AdmError::new(format!("AI API probe endpoint {error}")))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AdmError::new(
            "AI API probe endpoint must not contain embedded credentials",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::ai::{ApiCategory, ApiEntry};
    use serde_json::json;

    struct FakeTransport {
        status: u16,
    }

    impl AiApiProbeTransport for FakeTransport {
        fn execute(&self, request: &AiApiProbeRequest) -> AdmResult<AiApiProbeResponse> {
            let debug = format!("{request:?}");
            assert!(!debug.contains("sk-probe-secret"));
            assert!(request.endpoint.ends_with("/v1/models"));
            assert_eq!(
                request.headers.get("Authorization").map(String::as_str),
                Some("Bearer sk-probe-secret")
            );
            Ok(AiApiProbeResponse {
                status: self.status,
            })
        }
    }

    #[test]
    fn probe_uses_resolved_target_and_returns_only_masked_metadata() {
        let config = completion_config("direct", "sk-probe-secret");
        let view = probe_active_ai_api_with_transport(
            &config,
            "completion",
            &FakeTransport { status: 200 },
        )
        .unwrap();
        let serialized = serde_json::to_string(&view).unwrap();
        assert!(view.available);
        assert_eq!(view.status_code, Some(200));
        assert!(!serialized.contains("sk-probe-secret"));
        assert!(!serialized.contains("/models"));
    }

    #[test]
    fn probe_classifies_authentication_failure_without_response_body() {
        let config = completion_config("direct", "sk-probe-secret");
        let view = probe_active_ai_api_with_transport(
            &config,
            "completion",
            &FakeTransport { status: 401 },
        )
        .unwrap();
        assert!(!view.available);
        assert_eq!(view.diagnostics[0].code, "api_authentication_failed");
    }

    #[test]
    fn probe_rejects_remote_http_before_custom_and_reqwest_transports() {
        struct UnexpectedTransport;

        impl AiApiProbeTransport for UnexpectedTransport {
            fn execute(&self, _request: &AiApiProbeRequest) -> AdmResult<AiApiProbeResponse> {
                panic!("remote HTTP must be rejected before the custom transport")
            }
        }

        let mut config = completion_config("none", "");
        config.completion.entries[0].api_url = "http://api.private-example.test/v1".to_string();
        let error = probe_active_ai_api_with_transport(&config, "completion", &UnexpectedTransport)
            .unwrap_err();
        assert!(error.message().contains("must use HTTPS"));
        assert!(!error.message().contains("private-example"));

        let request = AiApiProbeRequest {
            endpoint: "http://api.private-example.test/v1/models".to_string(),
            headers: BTreeMap::new(),
            timeout: Duration::from_secs(1),
        };
        let error = ReqwestAiApiProbeTransport::new()
            .unwrap()
            .execute(&request)
            .unwrap_err();
        assert!(error.message().contains("must use HTTPS"));
        assert!(!error.message().contains("private-example"));
    }

    #[test]
    fn probe_allows_loopback_http_before_custom_transport() {
        for base_url in [
            "http://localhost:11434/v1",
            "http://127.8.9.10:11434/v1",
            "http://[::1]:11434/v1",
        ] {
            let mut config = completion_config("direct", "sk-probe-secret");
            config.completion.entries[0].api_url = base_url.to_string();
            let view = probe_active_ai_api_with_transport(
                &config,
                "completion",
                &FakeTransport { status: 200 },
            )
            .unwrap();
            assert!(view.available, "{base_url}: {:?}", view.diagnostics);
        }
    }

    fn completion_config(auth_mode: &str, key: &str) -> AiConfig {
        AiConfig {
            completion: ApiCategory {
                category_id: "completion".to_string(),
                active_entry_id: "api".to_string(),
                entries: vec![ApiEntry {
                    id: "api".to_string(),
                    config_type: "openai_completion_api".to_string(),
                    api_url: "https://api.example.test".to_string(),
                    api_key: key.to_string(),
                    extra_json: json!({"model": "gpt-test", "auth_mode": auth_mode}),
                    ..ApiEntry::default()
                }],
            },
            ..AiConfig::default()
        }
    }
}
