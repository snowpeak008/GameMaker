use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};

use adm_new_config::normalize_openai_base_url;
use adm_new_contracts::ai::{AiConfig, ApiEntry};
use adm_new_foundation::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::http_endpoint_policy::validate_http_transport_url;

#[derive(Clone, PartialEq, Eq)]
pub struct ImageApiSettings {
    pub name: String,
    pub provider: String,
    pub mode: String,
    pub api_key: String,
    pub base_url: String,
    pub image_model: String,
    pub response_model: Option<String>,
    pub endpoint: Option<String>,
    pub enabled: bool,
}

impl ImageApiSettings {
    pub fn masked_api_key(&self) -> String {
        if self.api_key.is_empty() {
            String::new()
        } else {
            "********".to_string()
        }
    }
}

impl fmt::Debug for ImageApiSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageApiSettings")
            .field("name", &self.name)
            .field("provider", &self.provider)
            .field("mode", &self.mode)
            .field("has_api_key", &!self.api_key.is_empty())
            .field("base_url", &crate::resolution::mask_api_url(&self.base_url))
            .field("image_model", &self.image_model)
            .field("response_model", &self.response_model)
            .field(
                "endpoint",
                &self
                    .endpoint
                    .as_deref()
                    .map(crate::resolution::mask_api_url),
            )
            .field("enabled", &self.enabled)
            .finish()
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageProbeRequest {
    pub endpoint: String,
    pub headers: BTreeMap<String, String>,
    pub payload: Value,
    pub mode: String,
    pub masked_api_key: String,
}

impl fmt::Debug for ImageProbeRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageProbeRequest")
            .field("endpoint", &crate::resolution::mask_api_url(&self.endpoint))
            .field("header_names", &self.headers.keys().collect::<Vec<_>>())
            .field(
                "payload_keys",
                &self
                    .payload
                    .as_object()
                    .map(|object| object.keys().collect::<Vec<_>>())
                    .unwrap_or_default(),
            )
            .field("mode", &self.mode)
            .field("has_masked_api_key", &!self.masked_api_key.is_empty())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PngMetadata {
    pub width: u32,
    pub height: u32,
    pub format: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageMetadataCheck {
    pub status: String,
    pub issues: Vec<String>,
    pub metadata: Option<PngMetadata>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodexImageCommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub stdin: String,
    pub env: BTreeMap<String, String>,
    pub generated_dir: PathBuf,
}

impl fmt::Debug for CodexImageCommandSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexImageCommandSpec")
            .field("program_configured", &!self.program.trim().is_empty())
            .field("arg_count", &self.args.len())
            .field("stdin_chars", &self.stdin.chars().count())
            .field("env_keys", &self.env.keys().collect::<Vec<_>>())
            .field("generated_dir_configured", &true)
            .finish()
    }
}

pub fn image_settings_from_config(
    config: &AiConfig,
    provider_name: Option<&str>,
) -> AdmResult<ImageApiSettings> {
    let active = provider_name
        .map(str::to_string)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| config.image.active_entry_id.clone());
    if active.trim().is_empty() {
        return Err(AdmError::new("Image API provider is not configured: relay"));
    }
    let entry = config
        .image
        .entries
        .iter()
        .find(|entry| entry.id == active)
        .ok_or_else(|| AdmError::new(format!("Image API provider is not configured: {active}")))?;
    image_settings_from_entry(&active, entry)
}

pub fn image_settings_from_entry(
    provider_name: &str,
    entry: &ApiEntry,
) -> AdmResult<ImageApiSettings> {
    let extra = entry.extra_json.as_object();
    if entry.config_type == "codex_cli_image" {
        return Err(AdmError::new(format!(
            "{provider_name} uses Codex CLI image generation, not image API"
        )));
    }
    let provider = extra
        .and_then(|object| object.get("provider"))
        .and_then(Value::as_str)
        .unwrap_or("openai_responses")
        .to_string();
    let mode = extra
        .and_then(|object| object.get("mode"))
        .and_then(Value::as_str)
        .unwrap_or("responses_image_tool")
        .to_string();
    let endpoint = extra
        .and_then(|object| object.get("endpoint"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            if mode == "responses_image_tool" {
                "responses".to_string()
            } else {
                "images/generations".to_string()
            }
        });
    let env_prefix = format!("IMAGE_{}", provider_name.to_ascii_uppercase());
    let api_key_env = extra
        .and_then(|object| object.get("api_key_env"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("{env_prefix}_API_KEY"));
    let api_key = non_empty(extra_string(extra, "api_key"))
        .or_else(|| non_empty(entry.api_key.clone()))
        .or_else(|| env::var(&api_key_env).ok())
        .unwrap_or_default();
    let base_url = non_empty(extra_string(extra, "base_url"))
        .or_else(|| non_empty(entry.api_url.clone()))
        .or_else(|| env::var(format!("{env_prefix}_BASE_URL")).ok())
        .unwrap_or_default();
    let image_model = non_empty(extra_string(extra, "image_model"))
        .or_else(|| non_empty(extra_string(extra, "default_model")))
        .or_else(|| non_empty(extra_string(extra, "model")))
        .or_else(|| env::var(format!("{env_prefix}_MODEL")).ok())
        .unwrap_or_default();
    let response_model = non_empty(extra_string(extra, "response_model"))
        .or_else(|| env::var(format!("{env_prefix}_RESPONSE_MODEL")).ok());
    let enabled = extra
        .and_then(|object| object.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true);

    if api_key.is_empty() {
        return Err(AdmError::new(format!(
            "Missing image API key for provider {provider_name}. Expected api_key or {api_key_env}."
        )));
    }
    if base_url.is_empty() {
        return Err(AdmError::new(format!(
            "Missing image API base_url for provider {provider_name}."
        )));
    }
    if image_model.is_empty() {
        return Err(AdmError::new(format!(
            "Missing image model for provider {provider_name}."
        )));
    }
    let base_url = if provider.starts_with("openai")
        || mode.starts_with("responses")
        || mode.starts_with("images")
    {
        normalize_openai_base_url(&base_url)
    } else {
        base_url.trim_end_matches('/').to_string()
    };
    validate_image_api_url(&base_url, "base URL")?;
    Ok(ImageApiSettings {
        name: provider_name.to_string(),
        provider,
        mode,
        api_key,
        base_url,
        image_model,
        response_model,
        endpoint: Some(endpoint),
        enabled,
    })
}

pub fn image_endpoint(settings: &ImageApiSettings) -> String {
    format!(
        "{}/{}",
        settings.base_url.trim_end_matches('/'),
        settings
            .endpoint
            .as_deref()
            .unwrap_or("")
            .trim_start_matches('/')
    )
}

pub fn build_image_probe_request(
    settings: &ImageApiSettings,
    prompt: &str,
    size: &str,
    quality: &str,
    output_format: &str,
) -> AdmResult<ImageProbeRequest> {
    let endpoint = image_endpoint(settings);
    validate_image_api_url(&endpoint, "probe endpoint")?;
    let payload = match settings.mode.as_str() {
        "responses_image_tool" => {
            responses_image_tool_payload(settings, prompt, size, quality, output_format)
        }
        "images_generations" => {
            images_generations_payload(settings, prompt, size, quality, output_format)
        }
        other => {
            return Err(AdmError::new(format!(
                "Unsupported image API mode: {other}"
            )));
        }
    };
    let mut headers = BTreeMap::new();
    headers.insert(
        "Authorization".to_string(),
        format!("Bearer {}", settings.masked_api_key()),
    );
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    Ok(ImageProbeRequest {
        endpoint,
        headers,
        payload,
        mode: settings.mode.clone(),
        masked_api_key: settings.masked_api_key(),
    })
}

fn validate_image_api_url(value: &str, label: &str) -> AdmResult<()> {
    let url = validate_http_transport_url(value)
        .map_err(|error| AdmError::new(format!("image API {label} {error}")))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AdmError::new(format!(
            "image API {label} must not contain embedded credentials"
        )));
    }
    Ok(())
}

pub fn responses_image_tool_payload(
    settings: &ImageApiSettings,
    prompt: &str,
    size: &str,
    quality: &str,
    output_format: &str,
) -> Value {
    json!({
        "model": settings.response_model.clone().unwrap_or_else(|| "gpt-5.5".to_string()),
        "stream": true,
        "tool_choice": "auto",
        "input": [
            {
                "role": "user",
                "content": [{"type": "input_text", "text": prompt}],
            }
        ],
        "tools": [
            {
                "type": "image_generation",
                "model": settings.image_model,
                "size": size,
                "quality": quality,
                "output_format": output_format,
                "background": "opaque",
            }
        ],
    })
}

pub fn images_generations_payload(
    settings: &ImageApiSettings,
    prompt: &str,
    size: &str,
    quality: &str,
    output_format: &str,
) -> Value {
    let mut payload = json!({
        "model": settings.image_model,
        "prompt": prompt,
        "size": size,
        "quality": quality,
        "response_format": "b64_json",
    });
    if !output_format.trim().is_empty() {
        payload["output_format"] = json!(output_format);
    }
    payload
}

pub fn extract_b64_from_responses_stream_text(text: &str) -> (Option<String>, Vec<String>) {
    let mut final_b64 = None;
    let mut event_types = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        let Ok(event) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !event_type.is_empty() {
            event_types.push(event_type.clone());
        }
        if event_type == "response.output_item.done" {
            if let Some(result) = event
                .get("item")
                .filter(|item| {
                    item.get("type").and_then(Value::as_str) == Some("image_generation_call")
                })
                .and_then(|item| item.get("result"))
                .and_then(Value::as_str)
            {
                final_b64 = Some(result.to_string());
            }
        } else if event_type == "response.completed" {
            final_b64 =
                extract_b64_from_responses_json(event.get("response").unwrap_or(&Value::Null))
                    .or(final_b64);
        }
    }
    (final_b64, event_types)
}

pub fn extract_b64_from_responses_json(data: &Value) -> Option<String> {
    data.get("output")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                (item.get("type").and_then(Value::as_str) == Some("image_generation_call"))
                    .then(|| item.get("result").and_then(Value::as_str))
                    .flatten()
                    .map(ToString::to_string)
            })
        })
}

pub fn extract_b64_from_images_json(data: &Value) -> Option<String> {
    data.get("data")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.first().and_then(|first| {
                first
                    .get("b64_json")
                    .or_else(|| first.get("b64"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
        })
        .or_else(|| extract_b64_from_responses_json(data))
}

pub fn png_metadata_from_bytes(bytes: &[u8]) -> AdmResult<PngMetadata> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != PNG_SIGNATURE {
        return Err(AdmError::new("image bytes are not PNG"));
    }
    if &bytes[12..16] != b"IHDR" {
        return Err(AdmError::new("PNG missing IHDR chunk"));
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Ok(PngMetadata {
        width,
        height,
        format: "PNG".to_string(),
    })
}

pub fn check_png_metadata_bytes(
    bytes: &[u8],
    expected_width: u32,
    expected_height: u32,
    expected_format: &str,
) -> ImageMetadataCheck {
    match png_metadata_from_bytes(bytes) {
        Ok(metadata) => {
            let mut issues = Vec::new();
            if expected_width > 0 && metadata.width != expected_width {
                issues.push(format!(
                    "width mismatch: expected {expected_width}, actual {}",
                    metadata.width
                ));
            }
            if expected_height > 0 && metadata.height != expected_height {
                issues.push(format!(
                    "height mismatch: expected {expected_height}, actual {}",
                    metadata.height
                ));
            }
            if !expected_format.trim().is_empty()
                && metadata.format.to_ascii_uppercase() != expected_format.to_ascii_uppercase()
            {
                issues.push(format!(
                    "format mismatch: expected {expected_format}, actual {}",
                    metadata.format
                ));
            }
            ImageMetadataCheck {
                status: if issues.is_empty() { "PASS" } else { "FAIL" }.to_string(),
                issues,
                metadata: Some(metadata),
            }
        }
        Err(error) => ImageMetadataCheck {
            status: "FAIL".to_string(),
            issues: vec![format!("cannot open image: {}", error.message())],
            metadata: None,
        },
    }
}

pub fn codex_home(override_value: Option<&str>) -> PathBuf {
    override_value
        .map(PathBuf::from)
        .or_else(|| env::var("CODEX_HOME").ok().map(PathBuf::from))
        .or_else(|| env::var("CODEX_WORKSPACE").ok().map(PathBuf::from))
        .unwrap_or_else(|| {
            env::var("USERPROFILE")
                .map(|value| Path::new(&value).join(".codex"))
                .or_else(|_| env::var("HOME").map(|value| Path::new(&value).join(".codex")))
                .unwrap_or_else(|_| PathBuf::from(".codex"))
        })
}

pub fn codex_image_command(
    project_root: &Path,
    prompt: &str,
    codex_home_override: Option<&str>,
    cli_path: Option<&str>,
) -> CodexImageCommandSpec {
    let home = codex_home(codex_home_override);
    let generated_dir = home.join("generated_images");
    let task_prompt = [
        "Use the image_gen tool to generate exactly one game art style reference image.",
        "Do not edit repository files.",
        "Save the result as a PNG image.",
        "",
        "Art direction:",
        prompt,
    ]
    .join("\n");
    let mut env = BTreeMap::new();
    env.insert("CODEX_HOME".to_string(), home.to_string_lossy().to_string());
    CodexImageCommandSpec {
        program: cli_path.unwrap_or("codex").to_string(),
        args: vec![
            "exec".to_string(),
            "--strict-config".to_string(),
            "--cd".to_string(),
            project_root.to_string_lossy().to_string(),
            "--sandbox".to_string(),
            "workspace-write".to_string(),
            "--skip-git-repo-check".to_string(),
            "--ephemeral".to_string(),
            "--ignore-user-config".to_string(),
            "--ignore-rules".to_string(),
        ],
        stdin: task_prompt,
        env,
        generated_dir,
    }
}

pub fn saved_png_paths_from_output(text: &str) -> Vec<String> {
    let saved_lines: Vec<&str> = text
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("saved")
                || lower.contains("generated")
                || lower.contains("output")
                || line.contains("已保存")
                || line.contains("保存")
        })
        .collect();
    let joined = saved_lines.join("\n");
    let first = png_paths_from_text(&joined);
    if first.is_empty() {
        png_paths_from_text(text)
    } else {
        first
    }
}

pub fn session_png_dirs_from_output(text: &str, generated_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for token in text.split_whitespace() {
        let clean = token.trim_matches(|ch: char| !ch.is_ascii_hexdigit() && ch != '-');
        if is_uuid_like(clean) {
            dirs.push(generated_dir.join(clean));
        }
    }
    dirs
}

fn png_paths_from_text(text: &str) -> Vec<String> {
    let mut paths = Vec::new();
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        let mut search_from = 0;
        while let Some(relative_end) = lower[search_from..].find(".png") {
            let end = search_from + relative_end + 4;
            if let Some(start) = find_path_start(line, end) {
                let candidate = line[start..end]
                    .trim_matches(|ch: char| {
                        matches!(
                            ch,
                            '`' | '"'
                                | '\''
                                | '<'
                                | '>'
                                | '('
                                | ')'
                                | '['
                                | ']'
                                | ','
                                | ';'
                                | '，'
                                | '。'
                        )
                    })
                    .to_string();
                if candidate.to_ascii_lowercase().ends_with(".png") && !paths.contains(&candidate) {
                    paths.push(candidate);
                }
            }
            search_from = end;
        }
    }
    paths
}

fn find_path_start(line: &str, end: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut start = None;
    for index in 0..end.saturating_sub(2) {
        if bytes[index].is_ascii_alphabetic()
            && bytes.get(index + 1) == Some(&b':')
            && matches!(bytes.get(index + 2), Some(b'\\' | b'/'))
        {
            start = Some(index);
        }
    }
    if start.is_some() {
        return start;
    }
    line[..end].rfind('/').map(|slash| {
        line[..slash]
            .rfind(|ch: char| ch.is_whitespace() || matches!(ch, '`' | '"' | '\'' | '<' | '>'))
            .map(|boundary| boundary + 1)
            .unwrap_or(0)
    })
}

fn is_uuid_like(value: &str) -> bool {
    let parts: Vec<&str> = value.split('-').collect();
    let lengths = [8, 4, 4, 4, 12];
    parts.len() == 5
        && parts
            .iter()
            .zip(lengths)
            .all(|(part, len)| part.len() == len && part.chars().all(|ch| ch.is_ascii_hexdigit()))
}

fn extra_string(extra: Option<&serde_json::Map<String, Value>>, key: &str) -> String {
    extra
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::ai::{AiConfig, ApiCategory, ApiEntry};
    use serde_json::json;

    #[test]
    fn image_settings_normalize_endpoint_and_mask_secret() {
        let config = AiConfig {
            image: ApiCategory {
                category_id: "image".to_string(),
                active_entry_id: "openai_image".to_string(),
                entries: vec![ApiEntry {
                    id: "openai_image".to_string(),
                    config_type: "openai_image_api".to_string(),
                    api_url: "https://api.example.test".to_string(),
                    api_key: "sk-1234567890".to_string(),
                    extra_json: json!({
                        "model": "gpt-image-2",
                        "response_model": "gpt-5.5",
                        "mode": "responses_image_tool"
                    }),
                    ..ApiEntry::default()
                }],
            },
            ..AiConfig::default()
        };
        let settings = image_settings_from_config(&config, None).unwrap();
        assert_eq!(settings.base_url, "https://api.example.test/v1");
        assert_eq!(settings.endpoint.as_deref(), Some("responses"));
        assert_eq!(settings.masked_api_key(), "********");
        let debug = format!("{settings:?}");
        assert!(!debug.contains("sk-1234567890"));
    }

    #[test]
    fn image_probe_payloads_match_python_tools() {
        let response_settings = settings("responses_image_tool");
        let request =
            build_image_probe_request(&response_settings, "tiny icon", "1024x1024", "high", "png")
                .unwrap();
        assert_eq!(request.endpoint, "https://api.example.test/v1/responses");
        assert_eq!(request.mode, "responses_image_tool");
        assert_eq!(
            request.headers.get("Authorization").map(String::as_str),
            Some("Bearer ********")
        );
        let serialized = serde_json::to_string(&request).unwrap();
        assert!(!serialized.contains("sk-1234567890"));
        assert_eq!(request.payload["stream"], json!(true));
        assert_eq!(
            request.payload["tools"][0]["type"],
            json!("image_generation")
        );
        assert_eq!(request.payload["tools"][0]["model"], json!("gpt-image-2"));

        let mut generation = settings("images_generations");
        generation.endpoint = Some("images/generations".to_string());
        let request =
            build_image_probe_request(&generation, "tiny icon", "512x512", "low", "png").unwrap();
        assert_eq!(
            request.endpoint,
            "https://api.example.test/v1/images/generations"
        );
        assert_eq!(request.payload["response_format"], json!("b64_json"));
        assert_eq!(request.payload["prompt"], json!("tiny icon"));
    }

    #[test]
    fn image_settings_and_probe_reject_remote_http_but_allow_loopback() {
        let local_entry = ApiEntry {
            id: "local-image".to_string(),
            config_type: "openai_image_api".to_string(),
            api_url: "http://localhost:11434".to_string(),
            api_key: "local-secret".to_string(),
            extra_json: json!({
                "model": "local-image-model",
                "mode": "images_generations"
            }),
            ..ApiEntry::default()
        };
        let local = image_settings_from_entry("local-image", &local_entry).unwrap();
        assert_eq!(local.base_url, "http://localhost:11434/v1");
        assert!(build_image_probe_request(&local, "icon", "8x8", "low", "png").is_ok());

        let mut remote_entry = local_entry.clone();
        remote_entry.id = "remote-image".to_string();
        remote_entry.api_url = "http://images.private-example.test".to_string();
        let error = image_settings_from_entry("remote-image", &remote_entry).unwrap_err();
        assert!(error.message().contains("must use HTTPS"));
        assert!(!error.message().contains("private-example"));

        let mut remote_probe = settings("images_generations");
        remote_probe.base_url = "http://images.private-example.test/v1".to_string();
        let error =
            build_image_probe_request(&remote_probe, "icon", "8x8", "low", "png").unwrap_err();
        assert!(error.message().contains("must use HTTPS"));
        assert!(!error.message().contains("private-example"));
    }

    #[test]
    fn image_b64_extractors_support_stream_and_json() {
        let stream = concat!(
            "data: {\"type\":\"response.output_item.done\",\"item\":{\"type\":\"image_generation_call\",\"result\":\"AAA\"}}\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"output\":[{\"type\":\"image_generation_call\",\"result\":\"BBB\"}]}}\n",
            "data: [DONE]\n"
        );
        let (b64, events) = extract_b64_from_responses_stream_text(stream);
        assert_eq!(b64.as_deref(), Some("BBB"));
        assert_eq!(
            events,
            vec!["response.output_item.done", "response.completed"]
        );
        assert_eq!(
            extract_b64_from_images_json(&json!({"data": [{"b64_json": "CCC"}]})).as_deref(),
            Some("CCC")
        );
    }

    #[test]
    fn png_metadata_checker_reports_pass_and_mismatch() {
        let png = minimal_png_header(640, 480);
        let metadata = png_metadata_from_bytes(&png).unwrap();
        assert_eq!(metadata.width, 640);
        assert_eq!(metadata.height, 480);
        let pass = check_png_metadata_bytes(&png, 640, 480, "PNG");
        assert_eq!(pass.status, "PASS");
        let fail = check_png_metadata_bytes(&png, 320, 480, "PNG");
        assert_eq!(fail.status, "FAIL");
        assert!(fail.issues[0].contains("width mismatch"));
    }

    #[test]
    fn codex_image_command_and_output_parsers_follow_python_shape() {
        let command = codex_image_command(
            Path::new("E:/repo"),
            "cel shaded ship",
            Some("E:/codex-home"),
            Some("codex.cmd"),
        );
        assert_eq!(command.program, "codex.cmd");
        assert_eq!(
            command.args,
            vec![
                "exec",
                "--strict-config",
                "--cd",
                "E:/repo",
                "--sandbox",
                "workspace-write",
                "--skip-git-repo-check",
                "--ephemeral",
                "--ignore-user-config",
                "--ignore-rules"
            ]
        );
        assert!(command.stdin.contains("Use the image_gen tool"));
        assert!(command.stdin.contains("cel shaded ship"));
        assert_eq!(
            command.generated_dir,
            PathBuf::from("E:/codex-home").join("generated_images")
        );
        let paths = saved_png_paths_from_output(
            "saved: E:\\work\\generated_images\\one.png\nother /tmp/generated/two.png",
        );
        assert_eq!(paths[0], "E:\\work\\generated_images\\one.png");
        assert_eq!(
            session_png_dirs_from_output(
                "session id: 12345678-1234-abcd-9876-123456789abc",
                Path::new("E:/codex-home/generated_images"),
            )[0],
            PathBuf::from("E:/codex-home/generated_images")
                .join("12345678-1234-abcd-9876-123456789abc")
        );
    }

    fn settings(mode: &str) -> ImageApiSettings {
        ImageApiSettings {
            name: "relay".to_string(),
            provider: "openai_responses".to_string(),
            mode: mode.to_string(),
            api_key: "sk-1234567890".to_string(),
            base_url: "https://api.example.test/v1".to_string(),
            image_model: "gpt-image-2".to_string(),
            response_model: Some("gpt-5.5".to_string()),
            endpoint: Some(if mode == "responses_image_tool" {
                "responses".to_string()
            } else {
                "images/generations".to_string()
            }),
            enabled: true,
        }
    }

    fn minimal_png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes.extend_from_slice(&[8, 6, 0, 0, 0]);
        bytes.extend_from_slice(&0_u32.to_be_bytes());
        bytes
    }
}
