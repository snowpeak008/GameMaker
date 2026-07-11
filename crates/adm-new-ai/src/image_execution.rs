use std::fmt;
use std::time::Duration;

use adm_new_foundation::{AdmError, AdmResult};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde_json::Value;

use crate::image::{
    ImageApiSettings, extract_b64_from_images_json, extract_b64_from_responses_json,
    extract_b64_from_responses_stream_text, image_endpoint, images_generations_payload,
    responses_image_tool_payload,
};
use crate::resolution::mask_api_url;

mod cli;
mod http;
mod png;

pub use cli::{
    CodexCliImageExecutor, ImageProcessOutput, ImageProcessRequest, ImageProcessRunner,
    SystemImageProcessRunner,
};
pub use http::{ImageHttpResponse, ImageHttpTransport, ReqwestImageHttpTransport};

use http::{safe_http_status_error, validate_http_endpoint};
use png::{sanitize_png, validate_dimensions};

pub const DEFAULT_IMAGE_HTTP_TIMEOUT: Duration = Duration::from_secs(300);
pub const DEFAULT_IMAGE_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const MAX_IMAGE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_IMAGE_HTTP_RESPONSE_BYTES: usize = 48 * 1024 * 1024;
pub const MAX_IMAGE_PIXELS: u64 = 16 * 1024 * 1024;
pub const MAX_IMAGE_EDGE: u32 = 8_192;

const MAX_ERROR_BODY_CHARS: usize = 2_048;
const MIN_REDACTED_BASE64_RUN: usize = 64;

#[derive(Clone, PartialEq, Eq)]
pub struct ImageExecutionRequest {
    pub unit_id: String,
    pub prompt: String,
    pub requested_width: u32,
    pub requested_height: u32,
    pub output_format: String,
    pub quality: String,
}

impl ImageExecutionRequest {
    pub fn png(
        unit_id: impl Into<String>,
        prompt: impl Into<String>,
        requested_width: u32,
        requested_height: u32,
    ) -> Self {
        Self {
            unit_id: unit_id.into(),
            prompt: prompt.into(),
            requested_width,
            requested_height,
            output_format: "png".to_string(),
            quality: "high".to_string(),
        }
    }
}

impl fmt::Debug for ImageExecutionRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageExecutionRequest")
            .field("unit_id", &self.unit_id)
            .field("prompt_chars", &self.prompt.chars().count())
            .field("requested_width", &self.requested_width)
            .field("requested_height", &self.requested_height)
            .field("output_format", &self.output_format)
            .field("quality", &self.quality)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ImageExecutionResult {
    pub bytes: Vec<u8>,
    pub provider: String,
    pub model: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
}

impl fmt::Debug for ImageExecutionResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageExecutionResult")
            .field("byte_len", &self.bytes.len())
            .field("provider", &self.provider)
            .field("model", &self.model)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("format", &self.format)
            .finish()
    }
}

pub trait ImageExecutor: Send + Sync {
    fn execute(&self, request: &ImageExecutionRequest) -> AdmResult<ImageExecutionResult>;
}

#[derive(Clone)]
pub struct OpenAiImageExecutor<T> {
    settings: ImageApiSettings,
    transport: T,
    timeout: Duration,
    max_response_bytes: usize,
}

pub type BlockingOpenAiImageExecutor = OpenAiImageExecutor<ReqwestImageHttpTransport>;

impl<T> OpenAiImageExecutor<T> {
    pub fn new(settings: ImageApiSettings, transport: T) -> Self {
        Self {
            settings,
            transport,
            timeout: DEFAULT_IMAGE_HTTP_TIMEOUT,
            max_response_bytes: MAX_IMAGE_HTTP_RESPONSE_BYTES,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> AdmResult<Self> {
        if timeout.is_zero() {
            return Err(AdmError::new(
                "image execution timeout must be greater than zero",
            ));
        }
        self.timeout = timeout;
        Ok(self)
    }

    pub fn with_response_limit(mut self, max_response_bytes: usize) -> AdmResult<Self> {
        if max_response_bytes == 0 || max_response_bytes > MAX_IMAGE_HTTP_RESPONSE_BYTES {
            return Err(AdmError::new(format!(
                "image HTTP response limit must be between 1 and {MAX_IMAGE_HTTP_RESPONSE_BYTES} bytes"
            )));
        }
        self.max_response_bytes = max_response_bytes;
        Ok(self)
    }

    pub fn settings(&self) -> &ImageApiSettings {
        &self.settings
    }
}

impl OpenAiImageExecutor<ReqwestImageHttpTransport> {
    pub fn blocking(settings: ImageApiSettings) -> AdmResult<Self> {
        Ok(Self::new(settings, ReqwestImageHttpTransport::new()?))
    }
}

impl<T> fmt::Debug for OpenAiImageExecutor<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiImageExecutor")
            .field("provider_name", &self.settings.name)
            .field("provider", &self.settings.provider)
            .field("mode", &self.settings.mode)
            .field("endpoint", &mask_api_url(&image_endpoint(&self.settings)))
            .field("has_api_key", &!self.settings.api_key.is_empty())
            .field("image_model", &self.settings.image_model)
            .field("response_model", &self.settings.response_model)
            .field("enabled", &self.settings.enabled)
            .field("transport", &std::any::type_name::<T>())
            .field("timeout", &self.timeout)
            .field("max_response_bytes", &self.max_response_bytes)
            .finish()
    }
}

impl<T> ImageExecutor for OpenAiImageExecutor<T>
where
    T: ImageHttpTransport,
{
    fn execute(&self, request: &ImageExecutionRequest) -> AdmResult<ImageExecutionResult> {
        validate_execution_request(request)?;
        validate_image_settings(&self.settings)?;
        let size = format!("{}x{}", request.requested_width, request.requested_height);
        let payload = match self.settings.mode.as_str() {
            "responses_image_tool" => responses_image_tool_payload(
                &self.settings,
                &request.prompt,
                &size,
                &request.quality,
                &request.output_format,
            ),
            "images_generations" => images_generations_payload(
                &self.settings,
                &request.prompt,
                &size,
                &request.quality,
                &request.output_format,
            ),
            other => {
                return Err(AdmError::new(format!(
                    "unsupported image API mode: {other}"
                )));
            }
        };
        let endpoint = image_endpoint(&self.settings);
        let response = self.transport.post_json(
            &endpoint,
            &self.settings.api_key,
            &payload,
            self.timeout,
            self.max_response_bytes,
        )?;
        if !(200..300).contains(&response.status) {
            return Err(safe_http_status_error(
                response.status,
                &response.body,
                &[self.settings.api_key.as_str()],
            ));
        }
        let encoded = extract_encoded_image(&self.settings.mode, &response.body)?;
        let raw = decode_image_base64(&encoded)?;
        let (bytes, width, height) = sanitize_png(&raw)?;
        Ok(ImageExecutionResult {
            bytes,
            provider: self.settings.name.clone(),
            model: self.settings.image_model.clone(),
            width,
            height,
            format: "png".to_string(),
        })
    }
}

fn validate_execution_request(request: &ImageExecutionRequest) -> AdmResult<()> {
    if request.unit_id.trim().is_empty() {
        return Err(AdmError::new("image execution unit_id cannot be empty"));
    }
    if request.prompt.trim().is_empty() {
        return Err(AdmError::new("image execution prompt cannot be empty"));
    }
    if !request.output_format.eq_ignore_ascii_case("png") {
        return Err(AdmError::new(
            "image execution currently supports PNG output only",
        ));
    }
    if !matches!(request.quality.as_str(), "auto" | "low" | "medium" | "high") {
        return Err(AdmError::new(format!(
            "unsupported image quality: {}",
            request.quality
        )));
    }
    validate_dimensions(
        request.requested_width,
        request.requested_height,
        "requested",
    )
}

fn validate_image_settings(settings: &ImageApiSettings) -> AdmResult<()> {
    if !settings.enabled {
        return Err(AdmError::new("image API provider is disabled"));
    }
    if settings.image_model.trim().is_empty() {
        return Err(AdmError::new("image API provider has no image model"));
    }
    validate_http_endpoint(&image_endpoint(settings))
}

fn extract_encoded_image(mode: &str, body: &[u8]) -> AdmResult<String> {
    let text = std::str::from_utf8(body)
        .map_err(|_| AdmError::new("image API response was not valid UTF-8"))?;
    let encoded = match mode {
        "responses_image_tool" => {
            let (stream_value, _) = extract_b64_from_responses_stream_text(text);
            stream_value.or_else(|| {
                serde_json::from_str::<Value>(text)
                    .ok()
                    .and_then(|value| extract_b64_from_responses_json(&value))
            })
        }
        "images_generations" => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|value| extract_b64_from_images_json(&value)),
        _ => None,
    };
    encoded.ok_or_else(|| AdmError::new("image API response did not contain image data"))
}

fn decode_image_base64(encoded: &str) -> AdmResult<Vec<u8>> {
    let max_encoded_len = MAX_IMAGE_BYTES
        .saturating_add(2)
        .saturating_div(3)
        .saturating_mul(4);
    if encoded.len() > max_encoded_len {
        return Err(AdmError::new(format!(
            "decoded image would exceed {MAX_IMAGE_BYTES} bytes"
        )));
    }
    let bytes = BASE64_STANDARD
        .decode(encoded.as_bytes())
        .map_err(|_| AdmError::new("image API returned invalid Base64 data"))?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(AdmError::new(format!(
            "decoded image exceeded {MAX_IMAGE_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
