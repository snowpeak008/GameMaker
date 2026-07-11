use std::fmt;
use std::io::Read;
use std::time::Duration;

use adm_new_foundation::{AdmError, AdmResult};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use reqwest::redirect::Policy;
use serde_json::Value;

use crate::http_endpoint_policy::validate_http_transport_url;
use crate::resolution::mask_api_url;

use super::{
    DEFAULT_IMAGE_CONNECT_TIMEOUT, MAX_ERROR_BODY_CHARS, MAX_IMAGE_HTTP_RESPONSE_BYTES,
    MIN_REDACTED_BASE64_RUN,
};

#[derive(Clone, PartialEq, Eq)]
pub struct ImageHttpResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

impl ImageHttpResponse {
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type: None,
            body: body.into(),
        }
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }
}

impl fmt::Debug for ImageHttpResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageHttpResponse")
            .field("status", &self.status)
            .field("content_type", &self.content_type)
            .field("body_len", &self.body.len())
            .finish()
    }
}

pub trait ImageHttpTransport: Send + Sync {
    /// Sends one JSON request. `bearer_token` is secret-bearing and must never be logged.
    fn post_json(
        &self,
        endpoint: &str,
        bearer_token: &str,
        payload: &Value,
        timeout: Duration,
        max_response_bytes: usize,
    ) -> AdmResult<ImageHttpResponse>;
}

#[derive(Debug, Clone)]
pub struct ReqwestImageHttpTransport {
    client: Client,
}

impl ReqwestImageHttpTransport {
    pub fn new() -> AdmResult<Self> {
        Self::with_connect_timeout(DEFAULT_IMAGE_CONNECT_TIMEOUT)
    }

    pub fn with_connect_timeout(connect_timeout: Duration) -> AdmResult<Self> {
        if connect_timeout.is_zero() {
            return Err(AdmError::new(
                "image HTTP connect timeout must be greater than zero",
            ));
        }
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .connect_timeout(connect_timeout)
            .build()
            .map_err(|_| AdmError::new("failed to initialize image HTTP transport"))?;
        Ok(Self { client })
    }

    fn send_request(&self, request: &ActualImageHttpRequest<'_>) -> AdmResult<ImageHttpResponse> {
        validate_http_endpoint(request.endpoint)?;
        if request.timeout.is_zero() {
            return Err(AdmError::new(
                "image HTTP request timeout must be greater than zero",
            ));
        }
        if request.max_response_bytes == 0
            || request.max_response_bytes > MAX_IMAGE_HTTP_RESPONSE_BYTES
        {
            return Err(AdmError::new(format!(
                "image HTTP response limit must be between 1 and {MAX_IMAGE_HTTP_RESPONSE_BYTES} bytes"
            )));
        }
        let mut builder = self
            .client
            .post(request.endpoint)
            .header(CONTENT_TYPE, "application/json");
        if let Some(authorization) = &request.authorization {
            let authorization = HeaderValue::from_str(authorization).map_err(|_| {
                AdmError::new("image HTTP Authorization header contains invalid characters")
            })?;
            builder = builder.header(AUTHORIZATION, authorization);
        }
        let response = builder
            .timeout(request.timeout)
            .json(request.payload)
            .send()
            .map_err(|error| {
                if error.is_timeout() {
                    AdmError::new(format!(
                        "image HTTP request timed out after {} seconds",
                        request.timeout.as_secs_f64()
                    ))
                } else {
                    AdmError::new(format!(
                        "image HTTP request failed for {}",
                        mask_api_url(request.endpoint)
                    ))
                }
            })?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        if response
            .content_length()
            .is_some_and(|length| length > request.max_response_bytes as u64)
        {
            return Err(response_too_large(request.max_response_bytes));
        }
        let read_limit = request.max_response_bytes.saturating_add(1) as u64;
        let mut body = Vec::new();
        response
            .take(read_limit)
            .read_to_end(&mut body)
            .map_err(|_| AdmError::new("failed to read image HTTP response body"))?;
        if body.len() > request.max_response_bytes {
            return Err(response_too_large(request.max_response_bytes));
        }
        Ok(ImageHttpResponse {
            status,
            content_type,
            body,
        })
    }
}

impl ImageHttpTransport for ReqwestImageHttpTransport {
    fn post_json(
        &self,
        endpoint: &str,
        bearer_token: &str,
        payload: &Value,
        timeout: Duration,
        max_response_bytes: usize,
    ) -> AdmResult<ImageHttpResponse> {
        let request = ActualImageHttpRequest::new(
            endpoint,
            bearer_token,
            payload,
            timeout,
            max_response_bytes,
        );
        self.send_request(&request)
    }
}

pub(super) struct ActualImageHttpRequest<'a> {
    endpoint: &'a str,
    authorization: Option<String>,
    payload: &'a Value,
    timeout: Duration,
    max_response_bytes: usize,
}

impl<'a> ActualImageHttpRequest<'a> {
    pub(super) fn new(
        endpoint: &'a str,
        bearer_token: &str,
        payload: &'a Value,
        timeout: Duration,
        max_response_bytes: usize,
    ) -> Self {
        Self {
            endpoint,
            authorization: (!bearer_token.trim().is_empty())
                .then(|| format!("Bearer {bearer_token}")),
            payload,
            timeout,
            max_response_bytes,
        }
    }
}

impl fmt::Debug for ActualImageHttpRequest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ActualImageHttpRequest")
            .field("endpoint", &mask_api_url(self.endpoint))
            .field(
                "authorization",
                &self.authorization.as_ref().map(|_| "[REDACTED]"),
            )
            .field("payload_keys", &json_object_keys(self.payload))
            .field("timeout", &self.timeout)
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

pub(super) fn validate_http_endpoint(endpoint: &str) -> AdmResult<()> {
    let url = validate_http_transport_url(endpoint)
        .map_err(|error| AdmError::new(format!("image API endpoint {error}")))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AdmError::new(
            "image API endpoint must not contain embedded credentials",
        ));
    }
    Ok(())
}

pub(super) fn safe_http_status_error(status: u16, body: &[u8], secrets: &[&str]) -> AdmError {
    let raw = String::from_utf8_lossy(body);
    let detail = serde_json::from_str::<Value>(&raw)
        .ok()
        .and_then(|value| {
            let error = value.get("error").unwrap_or(&value);
            error.as_str().map(str::to_string).or_else(|| {
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| raw.trim().to_string());
    let detail = truncate_chars(
        &redact_sensitive_text(&detail, secrets),
        MAX_ERROR_BODY_CHARS,
    );
    if detail.is_empty() {
        AdmError::new(format!("image API returned HTTP {status}"))
    } else {
        AdmError::new(format!("image API returned HTTP {status}: {detail}"))
    }
}

fn response_too_large(limit: usize) -> AdmError {
    AdmError::new(format!("image HTTP response exceeded {limit} bytes"))
}

fn redact_sensitive_text(value: &str, secrets: &[&str]) -> String {
    let mut output = secrets.iter().fold(value.to_string(), |text, secret| {
        let secret = secret.trim();
        if secret.is_empty() {
            text
        } else {
            text.replace(secret, "[REDACTED]")
        }
    });
    output = redact_long_base64_runs(&output);
    output
}

fn redact_long_base64_runs(value: &str) -> String {
    let mut output = String::with_capacity(value.len().min(MAX_ERROR_BODY_CHARS));
    let mut run = String::new();
    let flush = |output: &mut String, run: &mut String| {
        if run.len() >= MIN_REDACTED_BASE64_RUN {
            output.push_str("[BASE64_REDACTED]");
        } else {
            output.push_str(run);
        }
        run.clear();
    };
    for character in value.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '+' | '/' | '=' | '_' | '-') {
            run.push(character);
        } else {
            flush(&mut output, &mut run);
            output.push(character);
        }
    }
    flush(&mut output, &mut run);
    output
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut output = value.chars().take(limit).collect::<String>();
    output.push_str("...");
    output
}

fn json_object_keys(value: &Value) -> Vec<&str> {
    value
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default()
}
