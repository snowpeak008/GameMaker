use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use adm_new_config::AiAdapterKind;
use adm_new_config::normalize_openai_base_url;
use adm_new_contracts::ai::{ApiEntry, ModelResult, ModelResultStatus, ModelTask};
use adm_new_foundation::process::terminate_child_process_tree;
use adm_new_foundation::{AdmError, AdmResult, ensure_relative_path};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::redirect::Policy;
use serde_json::{Value, json};

use crate::http_endpoint_policy::validate_http_transport_url;
use crate::resolution::ResolvedAiTarget;
use crate::{CompletionAdapter, validate_allowed_outputs};

pub const SUPPORTED_ADAPTERS: &[&str] = &["none", "codex", "claude", "openai", "local"];

const CLI_ENV_ALLOWLIST: &[&str] = &[
    // Executable discovery and required Windows process infrastructure.
    "PATH",
    "PATHEXT",
    "SYSTEMROOT",
    "WINDIR",
    "COMSPEC",
    // User and application-data locations used by subscription login state.
    "HOME",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "CODEX_HOME",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_CODE_GIT_BASH_PATH",
    // Temporary/config/cache locations required by native and Node CLIs.
    "TEMP",
    "TMP",
    "TMPDIR",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_STATE_HOME",
    "XDG_RUNTIME_DIR",
    // Locale, terminal and certificate-file paths (values are not credentials).
    "LANG",
    "LANGUAGE",
    "LC_ALL",
    "LC_CTYPE",
    "TZ",
    "TERM",
    "COLORTERM",
    "NO_COLOR",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "NODE_EXTRA_CA_CERTS",
    "REQUESTS_CA_BUNDLE",
    "CURL_CA_BUNDLE",
    // Some runtimes require a user name even when HOME is already explicit.
    "USER",
    "USERNAME",
    "LOGNAME",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    DisabledLocal,
    CodexCli,
    ClaudeCli,
    OpenAiCompatible,
}

impl AdapterKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DisabledLocal => "local",
            Self::CodexCli => "codex",
            Self::ClaudeCli => "claude",
            Self::OpenAiCompatible => "openai",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterFactorySpec {
    pub requested_name: String,
    pub kind: AdapterKind,
    pub enabled: bool,
}

pub fn adapter_factory_spec(name: &str) -> AdmResult<AdapterFactorySpec> {
    let normalized = name.trim().to_ascii_lowercase();
    let kind = match normalized.as_str() {
        "none" | "local" => AdapterKind::DisabledLocal,
        "codex" => AdapterKind::CodexCli,
        "claude" | "claude_code" | "claude-code" => AdapterKind::ClaudeCli,
        "openai" | "openai_compatible" | "openai-compatible" => AdapterKind::OpenAiCompatible,
        other => return Err(AdmError::new(format!("unknown adapter: {other}"))),
    };
    Ok(AdapterFactorySpec {
        requested_name: normalized,
        kind,
        enabled: kind != AdapterKind::DisabledLocal,
    })
}

#[derive(Clone, PartialEq, Eq)]
pub struct CliCommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub stdin: String,
    pub timeout_seconds: u64,
    pub env: BTreeMap<String, String>,
    pub current_dir: String,
}

impl fmt::Debug for CliCommandSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CliCommandSpec")
            .field("program_configured", &!self.program.trim().is_empty())
            .field("arg_count", &self.args.len())
            .field("stdin_chars", &self.stdin.chars().count())
            .field("timeout_seconds", &self.timeout_seconds)
            .field("env_keys", &self.env.keys().collect::<Vec<_>>())
            .field(
                "current_dir_configured",
                &!self.current_dir.trim().is_empty(),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct OpenAiCompletionSettings {
    pub provider: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: Option<f64>,
    pub timeout_seconds: u64,
    pub reasoning_effort: Option<String>,
}

impl fmt::Debug for OpenAiCompletionSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiCompletionSettings")
            .field("provider", &self.provider)
            .field("base_url", &crate::resolution::mask_api_url(&self.base_url))
            .field("has_api_key", &!self.api_key.is_empty())
            .field("model", &self.model)
            .field("temperature", &self.temperature)
            .field("timeout_seconds", &self.timeout_seconds)
            .field("reasoning_effort", &self.reasoning_effort)
            .finish()
    }
}

#[derive(Clone, PartialEq)]
pub struct OpenAiRequestSpec {
    pub endpoint: String,
    pub headers: BTreeMap<String, String>,
    pub payload: Value,
    pub timeout_seconds: u64,
}

impl fmt::Debug for OpenAiRequestSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenAiRequestSpec")
            .field("endpoint", &crate::resolution::mask_api_url(&self.endpoint))
            .field("header_names", &self.headers.keys().collect::<Vec<_>>())
            .field("model", &self.payload.get("model").and_then(Value::as_str))
            .field("timeout_seconds", &self.timeout_seconds)
            .finish()
    }
}

pub trait OpenAiCompletionTransport {
    fn post_chat_completion(&self, request: &OpenAiRequestSpec) -> AdmResult<Value>;
}

#[derive(Debug, Clone)]
pub struct ReqwestOpenAiTransport {
    client: Client,
}

pub type BlockingOpenAiCompatibleAdapter = OpenAiCompatibleAdapter<ReqwestOpenAiTransport>;

impl ReqwestOpenAiTransport {
    pub fn new() -> AdmResult<Self> {
        let client = Client::builder()
            // API credentials must never be rerouted by ambient HTTP(S)_PROXY.
            // Explicit relay platforms are configured as the endpoint itself.
            .no_proxy()
            .redirect(Policy::none())
            .build()
            .map_err(|error| {
                AdmError::new(format!(
                    "failed to initialize OpenAI-compatible HTTP transport: {error}"
                ))
            })?;
        Ok(Self { client })
    }
}

impl OpenAiCompletionTransport for ReqwestOpenAiTransport {
    fn post_chat_completion(&self, request: &OpenAiRequestSpec) -> AdmResult<Value> {
        const MAX_RESPONSE_BYTES: u64 = 8 * 1024 * 1024;
        if request.timeout_seconds == 0 {
            return Err(AdmError::new(
                "OpenAI-compatible request timeout must be greater than zero",
            ));
        }
        validate_openai_http_url(&request.endpoint, "request endpoint")?;

        let secrets = sensitive_header_values(&request.headers);
        let headers = reqwest_headers(&request.headers, &secrets)?;
        let mut response = self
            .client
            .post(&request.endpoint)
            .headers(headers)
            .timeout(Duration::from_secs(request.timeout_seconds))
            .json(&request.payload)
            .send()
            .map_err(|error| request_error(error, request.timeout_seconds, &secrets))?;
        let status = response.status();
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES)
        {
            return Err(AdmError::new(format!(
                "OpenAI-compatible response exceeded {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        let mut body_bytes = Vec::new();
        response
            .by_ref()
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut body_bytes)
            .map_err(|error| {
                AdmError::new(redact_secrets(
                    &format!(
                        "failed to read OpenAI-compatible response body (HTTP {status}): {error}"
                    ),
                    &secrets,
                ))
            })?;
        if body_bytes.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(AdmError::new(format!(
                "OpenAI-compatible response exceeded {MAX_RESPONSE_BYTES} bytes"
            )));
        }
        let body = String::from_utf8(body_bytes).map_err(|error| {
            AdmError::new(redact_secrets(
                &format!("OpenAI-compatible API returned non-UTF-8 data (HTTP {status}): {error}"),
                &secrets,
            ))
        })?;

        if !status.is_success() {
            let detail = openai_error_detail(&body).unwrap_or_else(|| error_body_snippet(&body));
            let detail = redact_secrets(&detail, &secrets);
            let suffix = (!detail.is_empty())
                .then(|| format!(": {detail}"))
                .unwrap_or_default();
            return Err(AdmError::new(format!(
                "OpenAI-compatible API returned HTTP {status}{suffix}"
            )));
        }

        serde_json::from_str(&body).map_err(|error| {
            let snippet = redact_secrets(&error_body_snippet(&body), &secrets);
            let suffix = (!snippet.is_empty())
                .then(|| format!("; response: {snippet}"))
                .unwrap_or_default();
            AdmError::new(format!(
                "OpenAI-compatible API returned invalid JSON (HTTP {status}): {error}{suffix}"
            ))
        })
    }
}

#[derive(Debug, Clone)]
pub struct LocalModelAdapter {
    message: String,
}

impl Default for LocalModelAdapter {
    fn default() -> Self {
        Self {
            message: "LocalAdapter is not enabled".to_string(),
        }
    }
}

impl CompletionAdapter for LocalModelAdapter {
    fn generate(&self, task: &ModelTask) -> AdmResult<ModelResult> {
        Ok(ModelResult {
            task_id: task.task_id.clone(),
            status: ModelResultStatus::Failed,
            text: String::new(),
            errors: vec![self.message.clone()],
        })
    }
}

#[derive(Clone)]
pub struct CodexCliAdapter {
    pub cli_path: String,
}

impl fmt::Debug for CodexCliAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CodexCliAdapter")
            .field("cli_configured", &!self.cli_path.trim().is_empty())
            .finish()
    }
}

impl Default for CodexCliAdapter {
    fn default() -> Self {
        Self {
            cli_path: "codex".to_string(),
        }
    }
}

impl CompletionAdapter for CodexCliAdapter {
    fn generate(&self, task: &ModelTask) -> AdmResult<ModelResult> {
        let command = codex_exec_command(task, Some(&self.cli_path))?;
        run_cli_command(&task.task_id, &command)
    }
}

#[derive(Clone)]
pub struct ClaudeCliAdapter {
    pub cli_path: String,
}

impl fmt::Debug for ClaudeCliAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClaudeCliAdapter")
            .field("cli_configured", &!self.cli_path.trim().is_empty())
            .finish()
    }
}

impl Default for ClaudeCliAdapter {
    fn default() -> Self {
        Self {
            cli_path: "claude".to_string(),
        }
    }
}

impl CompletionAdapter for ClaudeCliAdapter {
    fn generate(&self, task: &ModelTask) -> AdmResult<ModelResult> {
        let command = claude_print_command(task, Some(&self.cli_path))?;
        run_cli_command(&task.task_id, &command)
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleAdapter<T> {
    pub settings: OpenAiCompletionSettings,
    pub transport: T,
}

impl<T> OpenAiCompatibleAdapter<T> {
    pub fn request_for_task(&self, task: &ModelTask) -> AdmResult<OpenAiRequestSpec> {
        build_openai_completion_request(&self.settings, task)
    }
}

impl<T> CompletionAdapter for OpenAiCompatibleAdapter<T>
where
    T: OpenAiCompletionTransport,
{
    fn generate(&self, task: &ModelTask) -> AdmResult<ModelResult> {
        let request = self.request_for_task(task)?;
        let response = self.transport.post_chat_completion(&request)?;
        let text = completion_text_from_openai_response(&response)?;
        Ok(ModelResult {
            task_id: task.task_id.clone(),
            status: ModelResultStatus::Succeeded,
            text,
            errors: Vec::new(),
        })
    }
}

pub fn blocking_openai_adapter_from_entry(
    entry: &ApiEntry,
) -> AdmResult<BlockingOpenAiCompatibleAdapter> {
    Ok(OpenAiCompatibleAdapter {
        settings: openai_settings_from_entry(entry)?,
        transport: ReqwestOpenAiTransport::new()?,
    })
}

pub fn blocking_openai_adapter_from_resolved(
    target: &ResolvedAiTarget,
    entry: &ApiEntry,
) -> AdmResult<BlockingOpenAiCompatibleAdapter> {
    Ok(OpenAiCompatibleAdapter {
        settings: openai_settings_from_resolved(target, entry)?,
        transport: ReqwestOpenAiTransport::new()?,
    })
}

fn reqwest_headers(headers: &BTreeMap<String, String>, secrets: &[String]) -> AdmResult<HeaderMap> {
    let mut output = HeaderMap::new();
    for (name, value) in headers {
        let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
            AdmError::new(format!(
                "invalid OpenAI-compatible HTTP header name: {}",
                redact_secrets(name, secrets)
            ))
        })?;
        let header_value = HeaderValue::from_str(value).map_err(|_| {
            AdmError::new(format!(
                "invalid OpenAI-compatible HTTP header value for {header_name}"
            ))
        })?;
        output.insert(header_name, header_value);
    }
    Ok(output)
}

fn validate_openai_http_url(value: &str, label: &str) -> AdmResult<()> {
    let url = validate_http_transport_url(value)
        .map_err(|error| AdmError::new(format!("OpenAI-compatible {label} {error}")))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(AdmError::new(format!(
            "OpenAI-compatible {label} must not contain embedded credentials"
        )));
    }
    if url.query().is_some() {
        return Err(AdmError::new(format!(
            "OpenAI-compatible {label} must not contain query parameters; configure authentication separately"
        )));
    }
    if url.fragment().is_some() {
        return Err(AdmError::new(format!(
            "OpenAI-compatible {label} must not contain a URL fragment"
        )));
    }
    Ok(())
}

fn request_error(error: reqwest::Error, timeout_seconds: u64, secrets: &[String]) -> AdmError {
    if error.is_timeout() {
        return AdmError::new(format!(
            "OpenAI-compatible request timed out after {timeout_seconds} seconds"
        ));
    }
    let error = error.without_url();
    AdmError::new(redact_secrets(
        &format!("OpenAI-compatible request failed: {error}"),
        secrets,
    ))
}

fn sensitive_header_values(headers: &BTreeMap<String, String>) -> Vec<String> {
    let mut secrets = Vec::new();
    for (name, value) in headers {
        let normalized = name.to_ascii_lowercase();
        if normalized == "authorization"
            || normalized.contains("api-key")
            || normalized.contains("token")
        {
            let value = value.trim();
            if !value.is_empty() {
                secrets.push(value.to_string());
            }
            if let Some((scheme, credential)) = value.split_once(' ')
                && scheme.eq_ignore_ascii_case("bearer")
                && !credential.trim().is_empty()
            {
                secrets.push(credential.trim().to_string());
            }
        }
    }
    secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
    secrets.dedup();
    secrets
}

fn redact_secrets(message: &str, secrets: &[String]) -> String {
    secrets
        .iter()
        .fold(message.to_string(), |redacted, secret| {
            if secret.is_empty() {
                redacted
            } else {
                redacted.replace(secret, "[REDACTED]")
            }
        })
}

fn openai_error_detail(body: &str) -> Option<String> {
    let data: Value = serde_json::from_str(body).ok()?;
    let error = data.get("error").unwrap_or(&data);
    if let Some(message) = error.as_str() {
        return Some(message.to_string());
    }
    let message = error.get("message").and_then(Value::as_str)?;
    let code = error.get("code").and_then(|value| {
        value.as_str().map(ToString::to_string).or_else(|| {
            value
                .as_i64()
                .map(|number| number.to_string())
                .or_else(|| value.as_u64().map(|number| number.to_string()))
        })
    });
    Some(match code {
        Some(code) if !code.is_empty() => format!("{message} (code: {code})"),
        _ => message.to_string(),
    })
}

fn error_body_snippet(body: &str) -> String {
    const MAX_ERROR_BODY_CHARS: usize = 2_048;
    let body = body.trim();
    if body.chars().count() <= MAX_ERROR_BODY_CHARS {
        return body.to_string();
    }
    let mut snippet: String = body.chars().take(MAX_ERROR_BODY_CHARS).collect();
    snippet.push_str("...");
    snippet
}

pub fn build_file_generation_task(
    task_id: &str,
    goal: &str,
    input_files: &[String],
    output_files: &[String],
    allowed_write_paths: &[String],
) -> ModelTask {
    let mut lines = vec![goal.to_string(), String::new(), "Input files:".to_string()];
    lines.extend(input_files.iter().map(|item| format!("- {item}")));
    lines.push(String::new());
    lines.push("Output files:".to_string());
    lines.extend(output_files.iter().map(|item| format!("- {item}")));
    lines.push(String::new());
    lines.push("Write only declared output files. Preserve existing unrelated files.".to_string());
    ModelTask {
        task_id: task_id.to_string(),
        prompt: lines.join("\n"),
        input_files: input_files.to_vec(),
        output_files: output_files.to_vec(),
        allowed_write_paths: allowed_write_paths.to_vec(),
        timeout_seconds: 1800,
        sandbox: "workspace-write".to_string(),
        cwd: String::new(),
    }
}

pub fn summarize_result(text: &str, limit: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let truncated: String = text.chars().take(limit).collect();
    format!("{}\n...", truncated.trim_end())
}

pub fn prompt_with_input_files(task: &ModelTask) -> AdmResult<String> {
    if task.input_files.is_empty() {
        return Ok(task.prompt.clone());
    }
    let cwd = task_cwd(task);
    let mut prompt = task.prompt.clone();
    prompt.push_str("\n\n[Input Files]\n");
    for relative in &task.input_files {
        let path = ensure_relative_path(cwd, relative)?;
        let content = fs::read_to_string(&path).map_err(|error| {
            AdmError::new(format!(
                "failed to read input file {}: {error}",
                path.display()
            ))
        })?;
        prompt.push_str(&format!("\n### {relative}\n{content}\n"));
    }
    Ok(prompt)
}

pub fn codex_exec_command(task: &ModelTask, cli_path: Option<&str>) -> AdmResult<CliCommandSpec> {
    validate_allowed_outputs(task)?;
    let cwd = if task.cwd.trim().is_empty() {
        "."
    } else {
        task.cwd.as_str()
    };
    let args = vec![
        "exec".to_string(),
        "--strict-config".to_string(),
        "--cd".to_string(),
        cwd.to_string(),
        "--sandbox".to_string(),
        task.sandbox.clone(),
        "--skip-git-repo-check".to_string(),
        "--ephemeral".to_string(),
        "--ignore-user-config".to_string(),
        "--ignore-rules".to_string(),
        "-c".to_string(),
        "sandbox_workspace_write.exclude_tmpdir_env_var=true".to_string(),
    ];
    #[cfg(not(windows))]
    let args = {
        let mut args = args;
        args.extend([
            "-c".to_string(),
            "sandbox_workspace_write.exclude_slash_tmp=true".to_string(),
        ]);
        args
    };
    let mut env = BTreeMap::new();
    if task.sandbox == "workspace-write" && cwd != "." {
        for key in ["TEMP", "TMP", "TMPDIR"] {
            env.insert(key.to_string(), cwd.to_string());
        }
    }
    Ok(CliCommandSpec {
        program: cli_path.unwrap_or("codex").to_string(),
        args,
        stdin: task.prompt.clone(),
        timeout_seconds: task.timeout_seconds,
        env,
        current_dir: cwd.to_string(),
    })
}

pub fn claude_print_command(task: &ModelTask, cli_path: Option<&str>) -> AdmResult<CliCommandSpec> {
    validate_allowed_outputs(task)?;
    if !matches!(task.sandbox.as_str(), "read-only" | "workspace-write") {
        return Err(AdmError::new(
            "Claude CLI tasks require a read-only or workspace-write sandbox",
        ));
    }
    let cwd = if task.cwd.trim().is_empty() {
        "."
    } else {
        task.cwd.as_str()
    };
    let writable = task.sandbox == "workspace-write" && !task.output_files.is_empty();
    let mut allowed_tools = vec!["Read".to_string(), "Glob".to_string(), "Grep".to_string()];
    let mut exposed_tools = vec!["Read", "Glob", "Grep"];
    if writable {
        exposed_tools.extend(["Edit", "Write"]);
        for relative in &task.output_files {
            let relative = relative.replace('\\', "/");
            allowed_tools.push(format!("Edit(/{relative})"));
            allowed_tools.push(format!("Write(/{relative})"));
        }
    }
    let args = vec![
        "--print".to_string(),
        "--input-format".to_string(),
        "text".to_string(),
        "--permission-mode".to_string(),
        "dontAsk".to_string(),
        "--tools".to_string(),
        exposed_tools.join(","),
        "--allowedTools".to_string(),
        allowed_tools.join(","),
        "--disallowedTools".to_string(),
        "Bash,WebFetch,WebSearch,NotebookEdit,Agent,Task,Skill".to_string(),
        "--safe-mode".to_string(),
        "--disable-slash-commands".to_string(),
        "--no-session-persistence".to_string(),
        "--setting-sources".to_string(),
        "project".to_string(),
        "--strict-mcp-config".to_string(),
        "--mcp-config".to_string(),
        "{}".to_string(),
    ];
    #[cfg(not(windows))]
    let args = {
        let mut args = args;
        args.extend([
            "--settings".to_string(),
            serde_json::json!({
                "sandbox": {
                    "enabled": true,
                    "failIfUnavailable": true,
                    "autoAllowBashIfSandboxed": false,
                    "allowUnsandboxedCommands": false,
                }
            })
            .to_string(),
        ]);
        args
    };
    Ok(CliCommandSpec {
        program: cli_path.unwrap_or("claude").to_string(),
        args,
        stdin: task.prompt.clone(),
        timeout_seconds: task.timeout_seconds,
        env: BTreeMap::new(),
        current_dir: cwd.to_string(),
    })
}

pub fn openai_settings_from_entry(entry: &ApiEntry) -> AdmResult<OpenAiCompletionSettings> {
    openai_settings_from_values(
        entry,
        entry.api_url.trim(),
        entry.api_key.trim(),
        entry
            .extra_json
            .as_object()
            .and_then(|object| object.get("model"))
            .and_then(Value::as_str)
            .unwrap_or(""),
        false,
    )
}

pub fn openai_settings_from_resolved(
    target: &ResolvedAiTarget,
    entry: &ApiEntry,
) -> AdmResult<OpenAiCompletionSettings> {
    if target.descriptor().adapter != AiAdapterKind::OpenAiCompatible {
        return Err(AdmError::new(format!(
            "resolved AI target {} is not an OpenAI-compatible adapter",
            target.entry_id()
        )));
    }
    if !target.is_available() {
        return Err(AdmError::new(format!(
            "resolved AI target {} is unavailable",
            target.entry_id()
        )));
    }
    openai_settings_from_values(
        entry,
        target.api_url().unwrap_or_default(),
        target.api_secret().unwrap_or_default(),
        target.model().unwrap_or_default(),
        target.auth_mode() == "none",
    )
}

fn openai_settings_from_values(
    entry: &ApiEntry,
    api_url: &str,
    api_key: &str,
    resolved_model: &str,
    allow_no_auth: bool,
) -> AdmResult<OpenAiCompletionSettings> {
    let extra = entry.extra_json.as_object();
    let provider = extra
        .and_then(|object| object.get("provider"))
        .and_then(Value::as_str)
        .unwrap_or(if entry.config_type == "custom_completion_api" {
            "custom"
        } else {
            "openai"
        })
        .to_string();
    let model = resolved_model.trim().to_string();
    if model.is_empty() {
        return Err(AdmError::new(format!(
            "{} completion entry is missing extra_json.model",
            entry.id
        )));
    }
    if api_url.trim().is_empty() {
        return Err(AdmError::new(format!(
            "{} completion entry is missing api_url",
            entry.id
        )));
    }
    validate_openai_http_url(api_url, "base URL")?;
    if api_key.trim().is_empty() && !allow_no_auth {
        return Err(AdmError::new(format!(
            "{} completion entry is missing api_key",
            entry.id
        )));
    }
    let temperature = extra
        .and_then(|object| object.get("temperature").or_else(|| object.get("temp")))
        .and_then(Value::as_f64);
    let timeout_seconds = extra
        .and_then(|object| {
            object
                .get("timeout_seconds")
                .or_else(|| object.get("timeout"))
        })
        .and_then(Value::as_u64)
        .unwrap_or(1800);
    if timeout_seconds == 0 {
        return Err(AdmError::new(format!(
            "{} completion entry timeout must be greater than zero",
            entry.id
        )));
    }
    let reasoning_effort = extra
        .and_then(|object| object.get("reasoning_effort"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let base_url = if provider.starts_with("openai") || entry.config_type == "custom_completion_api"
    {
        normalize_openai_base_url(api_url)
    } else {
        api_url.trim_end_matches('/').to_string()
    };
    validate_openai_http_url(&base_url, "normalized base URL")?;
    Ok(OpenAiCompletionSettings {
        provider,
        base_url,
        api_key: api_key.to_string(),
        model,
        temperature,
        timeout_seconds,
        reasoning_effort,
    })
}

pub fn build_openai_completion_request(
    settings: &OpenAiCompletionSettings,
    task: &ModelTask,
) -> AdmResult<OpenAiRequestSpec> {
    validate_openai_http_url(&settings.base_url, "base URL")?;
    let prompt = prompt_with_input_files(task)?;
    let mut payload = json!({
        "model": settings.model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
    });
    if let Some(temperature) = settings.temperature {
        payload["temperature"] = json!(temperature);
    }
    if let Some(reasoning_effort) = &settings.reasoning_effort {
        payload["reasoning_effort"] = json!(reasoning_effort);
    }
    let mut headers = BTreeMap::new();
    if !settings.api_key.trim().is_empty() {
        headers.insert(
            "Authorization".to_string(),
            format!("Bearer {}", settings.api_key),
        );
    }
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    Ok(OpenAiRequestSpec {
        endpoint: format!(
            "{}/chat/completions",
            settings.base_url.trim_end_matches('/')
        ),
        headers,
        payload,
        timeout_seconds: if task.timeout_seconds > 0 {
            task.timeout_seconds
        } else {
            settings.timeout_seconds
        },
    })
}

pub fn completion_text_from_openai_response(data: &Value) -> AdmResult<String> {
    if let Some(text) = data.get("output_text").and_then(Value::as_str) {
        return Ok(text.to_string());
    }
    if let Some(text) = data
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
    {
        return Ok(text.to_string());
    }
    if let Some(text) = data
        .get("output")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                item.get("content")
                    .and_then(Value::as_array)
                    .and_then(|content| {
                        content.iter().find_map(|part| {
                            part.get("text")
                                .or_else(|| part.get("output_text"))
                                .and_then(Value::as_str)
                        })
                    })
            })
        })
    {
        return Ok(text.to_string());
    }
    Err(AdmError::new(
        "OpenAI-compatible response did not contain completion text",
    ))
}

pub fn run_cli_command(task_id: &str, command: &CliCommandSpec) -> AdmResult<ModelResult> {
    const MAX_STREAM_BYTES: usize = 64 * 1024;
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .env_clear()
        .envs(minimal_cli_environment())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if !command.current_dir.trim().is_empty() {
        process.current_dir(&command.current_dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        process.creation_flags(CREATE_NO_WINDOW);
    }
    for (key, value) in &command.env {
        process.env(key, value);
    }
    let mut child = process.spawn().map_err(|error| {
        AdmError::new(format!("failed to start model adapter command: {error}"))
    })?;
    let stdin_writer = if command.stdin.is_empty() {
        drop(child.stdin.take());
        None
    } else {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| AdmError::new("failed to open model adapter stdin"))?;
        let input = command.stdin.clone();
        Some(thread::spawn(move || stdin.write_all(input.as_bytes())))
    };
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AdmError::new("failed to open model adapter stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AdmError::new("failed to open model adapter stderr"))?;
    let stdout_reader = thread::spawn(move || read_stream_limited(stdout, MAX_STREAM_BYTES));
    let stderr_reader = thread::spawn(move || read_stream_limited(stderr, MAX_STREAM_BYTES));
    let started = Instant::now();
    let mut timed_out = false;
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(_) => {
                let _ = terminate_child_process_tree(&mut child);
                return Err(AdmError::new("model adapter process wait failed"));
            }
        }
        if command.timeout_seconds > 0
            && started.elapsed() >= Duration::from_secs(command.timeout_seconds)
        {
            timed_out = true;
            break terminate_child_process_tree(&mut child)?;
        }
        thread::sleep(Duration::from_millis(25));
    };
    let (stdout, stdout_truncated) = stdout_reader
        .join()
        .map_err(|_| AdmError::new("model adapter stdout reader failed"))??;
    let (stderr, stderr_truncated) = stderr_reader
        .join()
        .map_err(|_| AdmError::new("model adapter stderr reader failed"))??;
    let stdin_write_result = stdin_writer.map(|writer| {
        writer
            .join()
            .map_err(|_| AdmError::new("model adapter stdin writer failed"))?
            .map_err(|_| AdmError::new("model adapter stdin write failed"))
    });
    let secrets = command
        .env
        .values()
        .filter(|value| value.len() >= 4)
        .cloned()
        .collect::<Vec<_>>();
    let stdout = redact_cli_output(&String::from_utf8_lossy(&stdout), &secrets);
    let stderr = redact_cli_output(String::from_utf8_lossy(&stderr).trim(), &secrets);
    let stdout = append_truncation_notice(stdout, stdout_truncated);
    let stderr = append_truncation_notice(stderr, stderr_truncated);
    if timed_out {
        return Ok(ModelResult {
            task_id: task_id.to_string(),
            status: ModelResultStatus::Failed,
            text: stdout,
            errors: vec![format!(
                "model adapter timed out after {} seconds{}",
                command.timeout_seconds,
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!(": {stderr}")
                }
            )],
        });
    }
    if let Some(result) = stdin_write_result {
        result?;
    }
    if exit_status.success() {
        Ok(ModelResult {
            task_id: task_id.to_string(),
            status: ModelResultStatus::Succeeded,
            text: stdout,
            errors: Vec::new(),
        })
    } else {
        Ok(ModelResult {
            task_id: task_id.to_string(),
            status: ModelResultStatus::Failed,
            text: stdout,
            errors: vec![if stderr.is_empty() {
                format!("model adapter exited with status {exit_status}")
            } else {
                stderr
            }],
        })
    }
}

pub(crate) fn minimal_cli_environment() -> BTreeMap<OsString, OsString> {
    filter_cli_environment(std::env::vars_os())
}

fn filter_cli_environment(
    environment: impl IntoIterator<Item = (OsString, OsString)>,
) -> BTreeMap<OsString, OsString> {
    environment
        .into_iter()
        .filter(|(key, _)| cli_environment_key_allowed(key))
        .collect()
}

fn cli_environment_key_allowed(key: &OsStr) -> bool {
    let Some(key) = key.to_str() else {
        return false;
    };
    #[cfg(windows)]
    {
        CLI_ENV_ALLOWLIST
            .iter()
            .any(|allowed| key.eq_ignore_ascii_case(allowed))
    }
    #[cfg(not(windows))]
    {
        CLI_ENV_ALLOWLIST.contains(&key)
    }
}

fn read_stream_limited(mut reader: impl Read, limit: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut bytes = Vec::with_capacity(limit.min(8 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        if remaining > 0 {
            bytes.extend_from_slice(&buffer[..read.min(remaining)]);
        }
        if read > remaining {
            truncated = true;
        }
    }
    Ok((bytes, truncated))
}

fn append_truncation_notice(mut value: String, truncated: bool) -> String {
    if truncated {
        value.push_str("\n[output truncated]");
    }
    value
}

fn redact_cli_output(value: &str, secrets: &[String]) -> String {
    let mut redacted = redact_secrets(value, secrets);
    let tokens = redacted
        .split_whitespace()
        .filter(|token| {
            let trimmed = token.trim_matches(|character: char| {
                matches!(
                    character,
                    '"' | '\'' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
                )
            });
            cli_token_is_sensitive(trimmed)
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    for token in tokens {
        redacted = redacted.replace(&token, "[REDACTED]");
    }
    redacted
}

fn cli_token_is_sensitive(token: &str) -> bool {
    let assignment_key = token
        .split_once('=')
        .or_else(|| token.split_once(':'))
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, _)| {
            key.trim_matches(|character: char| {
                matches!(
                    character,
                    '"' | '\'' | ',' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}'
                )
            })
            .to_ascii_lowercase()
            .replace('-', "_")
        });
    if assignment_key.is_some_and(|key| {
        key == "api_key"
            || key.ends_with("_api_key")
            || key == "token"
            || key.ends_with("_token")
            || key.contains("secret")
            || key.contains("password")
            || key.contains("authorization")
            || key.contains("credential")
    }) {
        return true;
    }
    if token.len() < 8 {
        return false;
    }
    let lower = token.to_ascii_lowercase();
    if [
        "sk-",
        "sk_",
        "bearer",
        "ghp_",
        "github_pat_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "xoxb-",
        "xoxp-",
        "aiza",
        "akia",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
    {
        return true;
    }
    if token.matches('.').count() == 2
        && token.split('.').all(|part| {
            part.len() >= 8
                && part.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '=')
                })
        })
    {
        return true;
    }
    false
}

fn task_cwd(task: &ModelTask) -> &Path {
    if task.cwd.trim().is_empty() {
        Path::new(".")
    } else {
        Path::new(&task.cwd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::ai::ApiEntry;
    use serde_json::json;
    use std::io::Read;
    use std::net::TcpListener;
    use std::sync::mpsc::{self, Receiver};
    use std::thread::JoinHandle;

    #[test]
    fn adapter_registry_matches_python_names() {
        assert_eq!(
            adapter_factory_spec("none").unwrap().kind,
            AdapterKind::DisabledLocal
        );
        assert_eq!(
            adapter_factory_spec("codex").unwrap().kind,
            AdapterKind::CodexCli
        );
        assert_eq!(
            adapter_factory_spec("claude_code").unwrap().kind,
            AdapterKind::ClaudeCli
        );
        assert_eq!(
            adapter_factory_spec("openai").unwrap().kind,
            AdapterKind::OpenAiCompatible
        );
        assert!(adapter_factory_spec("gemini").is_err());
    }

    #[test]
    fn cli_environment_keeps_runtime_and_subscription_paths_but_drops_credentials() {
        let environment = [
            ("PATH", "runtime-path"),
            ("SYSTEMROOT", "windows-root"),
            ("USERPROFILE", "user-home"),
            ("APPDATA", "roaming-data"),
            ("LOCALAPPDATA", "local-data"),
            ("CODEX_HOME", "codex-home"),
            ("CLAUDE_CONFIG_DIR", "claude-home"),
            ("CLAUDE_CODE_GIT_BASH_PATH", "git-bash"),
            ("OPENAI_API_KEY", "openai-private-value"),
            ("ANTHROPIC_API_KEY", "anthropic-private-value"),
            ("AWS_SECRET_ACCESS_KEY", "aws-private-value"),
            ("GH_TOKEN", "github-private-value"),
            ("DATABASE_URL", "database-private-value"),
            ("HTTPS_PROXY", "https://user:password@proxy.example"),
            ("UNRELATED", "unrelated-value"),
        ]
        .into_iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)));

        let filtered = filter_cli_environment(environment);
        let keys = filtered
            .keys()
            .filter_map(|key| key.to_str())
            .collect::<Vec<_>>();
        for required in [
            "PATH",
            "SYSTEMROOT",
            "USERPROFILE",
            "APPDATA",
            "LOCALAPPDATA",
            "CODEX_HOME",
            "CLAUDE_CONFIG_DIR",
            "CLAUDE_CODE_GIT_BASH_PATH",
        ] {
            assert!(
                keys.contains(&required),
                "missing CLI runtime key {required}"
            );
        }
        for rejected in [
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "AWS_SECRET_ACCESS_KEY",
            "GH_TOKEN",
            "DATABASE_URL",
            "HTTPS_PROXY",
            "UNRELATED",
        ] {
            assert!(!keys.contains(&rejected), "unsafe inherited key {rejected}");
        }
        let retained_values = filtered
            .values()
            .filter_map(|value| value.to_str())
            .collect::<Vec<_>>()
            .join(" ");
        for secret in [
            "openai-private-value",
            "anthropic-private-value",
            "aws-private-value",
            "github-private-value",
            "database-private-value",
            "password",
        ] {
            assert!(!retained_values.contains(secret));
        }
    }

    #[test]
    fn local_adapter_returns_disabled_failure() {
        let task = task("local");
        let result = LocalModelAdapter::default().generate(&task).unwrap();
        assert_eq!(result.status, ModelResultStatus::Failed);
        assert!(result.errors[0].contains("LocalAdapter is not enabled"));
    }

    #[test]
    fn task_builder_and_summary_match_python_helpers() {
        let task = build_file_generation_task(
            "patch",
            "Update files",
            &["input.md".to_string()],
            &["out/result.md".to_string()],
            &["out".to_string()],
        );
        assert!(task.prompt.contains("Input files:\n- input.md"));
        assert!(task.prompt.contains("Output files:\n- out/result.md"));
        assert!(task.prompt.contains("Write only declared output files"));
        assert_eq!(summarize_result("  short  ", 100), "short");
        assert_eq!(summarize_result("abcdef", 3), "abc\n...");
    }

    #[test]
    fn codex_command_uses_task_sandbox_and_output_guard() {
        let task = ModelTask {
            output_files: vec!["out/result.json".to_string()],
            allowed_write_paths: vec!["out".to_string()],
            sandbox: "none".to_string(),
            cwd: "project".to_string(),
            ..task("codex")
        };
        let command = codex_exec_command(&task, Some("codex.cmd")).unwrap();
        assert_eq!(command.program, "codex.cmd");
        assert!(
            command
                .args
                .windows(2)
                .any(|pair| pair == ["--cd", "project"])
        );
        assert!(
            command
                .args
                .windows(2)
                .any(|pair| pair == ["--sandbox", "none"])
        );
        for flag in [
            "--skip-git-repo-check",
            "--strict-config",
            "--ephemeral",
            "--ignore-user-config",
            "--ignore-rules",
        ] {
            assert!(command.args.iter().any(|argument| argument == flag));
        }
        assert!(
            command.args.iter().any(|argument| {
                argument == "sandbox_workspace_write.exclude_tmpdir_env_var=true"
            })
        );
        assert!(command.env.is_empty());
        assert_eq!(command.current_dir, "project");
        let invalid = ModelTask {
            output_files: vec!["../bad.json".to_string()],
            ..task
        };
        assert!(codex_exec_command(&invalid, None).is_err());

        let isolated = ModelTask {
            task_id: "task".to_string(),
            prompt: "codex isolated".to_string(),
            input_files: Vec::new(),
            output_files: vec!["out/result.json".to_string()],
            allowed_write_paths: vec!["out".to_string()],
            timeout_seconds: 0,
            sandbox: "workspace-write".to_string(),
            cwd: "isolated-project".to_string(),
        };
        let isolated = codex_exec_command(&isolated, None).unwrap();
        for key in ["TEMP", "TMP", "TMPDIR"] {
            assert_eq!(
                isolated.env.get(key).map(String::as_str),
                Some("isolated-project")
            );
        }
    }

    #[test]
    fn claude_command_matches_print_adapter_shape() {
        let task = ModelTask {
            cwd: "project".to_string(),
            ..task("hello")
        };
        let command = claude_print_command(&task, Some("claude.exe")).unwrap();
        assert_eq!(command.program, "claude.exe");
        assert!(
            command
                .args
                .windows(2)
                .any(|pair| pair == ["--permission-mode", "dontAsk"])
        );
        assert!(
            command
                .args
                .windows(2)
                .any(|pair| pair == ["--tools", "Read,Glob,Grep"])
        );
        assert!(
            command
                .args
                .iter()
                .any(|argument| argument == "--safe-mode")
        );
        assert!(
            command
                .args
                .iter()
                .any(|argument| argument == "--strict-mcp-config")
        );
        assert_eq!(command.stdin, "hello");
        assert_eq!(command.current_dir, "project");
        let debug = format!("{command:?}");
        assert!(!debug.contains("hello"));
        assert!(!debug.contains("claude.exe"));
    }

    #[test]
    fn claude_workspace_write_is_limited_to_declared_outputs() {
        let task = ModelTask {
            cwd: "isolated-project".to_string(),
            output_files: vec!["Assets/Scripts/A.cs".to_string()],
            allowed_write_paths: vec!["Assets/Scripts".to_string()],
            ..task("write code")
        };
        let command = claude_print_command(&task, Some("claude.exe")).unwrap();
        let allowed = command
            .args
            .windows(2)
            .find(|pair| pair[0] == "--allowedTools")
            .map(|pair| pair[1].as_str())
            .unwrap();
        assert!(allowed.contains("Edit(/Assets/Scripts/A.cs)"));
        assert!(allowed.contains("Write(/Assets/Scripts/A.cs)"));
        assert!(!allowed.contains("Assets/Scripts/**"));
        assert!(command.args.windows(2).any(|pair| {
            pair == [
                "--disallowedTools",
                "Bash,WebFetch,WebSearch,NotebookEdit,Agent,Task,Skill",
            ]
        }));

        let invalid = ModelTask {
            sandbox: "none".to_string(),
            ..task
        };
        assert!(claude_print_command(&invalid, None).is_err());
    }

    #[test]
    fn openai_request_normalizes_endpoint_and_appends_input_files() {
        let root = std::env::temp_dir().join("adm_new_ai_adapter_input");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("context.md"), "context body").unwrap();
        let entry = ApiEntry {
            id: "completion".to_string(),
            config_type: "openai_completion_api".to_string(),
            api_url: "https://api.example.test".to_string(),
            api_key: "sk-test".to_string(),
            extra_json: json!({
                "model": "gpt-5.5",
                "temperature": 0.2,
                "reasoning_effort": "low"
            }),
            ..ApiEntry::default()
        };
        let settings = openai_settings_from_entry(&entry).unwrap();
        assert_eq!(settings.base_url, "https://api.example.test/v1");
        let task = ModelTask {
            input_files: vec!["context.md".to_string()],
            cwd: root.to_string_lossy().to_string(),
            ..task("Summarize")
        };
        let request = build_openai_completion_request(&settings, &task).unwrap();
        assert_eq!(
            request.endpoint,
            "https://api.example.test/v1/chat/completions"
        );
        assert_eq!(request.payload["temperature"], json!(0.2));
        assert_eq!(request.payload["reasoning_effort"], json!("low"));
        assert!(
            request.payload["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("context body")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn openai_adapter_uses_transport_and_parses_completion_text() {
        let entry = ApiEntry {
            id: "completion".to_string(),
            config_type: "openai_completion_api".to_string(),
            api_url: "https://api.example.test/v1".to_string(),
            api_key: "sk-test".to_string(),
            extra_json: json!({"model": "gpt-5.5"}),
            ..ApiEntry::default()
        };
        let adapter = OpenAiCompatibleAdapter {
            settings: openai_settings_from_entry(&entry).unwrap(),
            transport: FakeOpenAiTransport,
        };
        let result = adapter.generate(&task("Return JSON")).unwrap();
        assert_eq!(result.status, ModelResultStatus::Succeeded);
        assert_eq!(result.text, "{\"ok\":true}");
        assert_eq!(
            completion_text_from_openai_response(&json!({"output_text": "plain"})).unwrap(),
            "plain"
        );
        assert!(
            completion_text_from_openai_response(&json!({"choices": []}))
                .unwrap_err()
                .message()
                .contains("did not contain")
        );
    }

    #[test]
    fn reqwest_transport_posts_headers_and_json() {
        let secret = "sk-live-secret";
        let (base_url, received, server) = spawn_http_server(
            "200 OK",
            json!({
                "choices": [{"message": {"content": "{\"ok\":true}"}}]
            })
            .to_string(),
            Duration::ZERO,
        );
        let entry = ApiEntry {
            id: "completion".to_string(),
            config_type: "openai_completion_api".to_string(),
            api_url: base_url,
            api_key: secret.to_string(),
            extra_json: json!({"model": "gpt-test", "timeout_seconds": 3}),
            ..ApiEntry::default()
        };
        let adapter = blocking_openai_adapter_from_entry(&entry).unwrap();

        let result = adapter.generate(&task("Return JSON")).unwrap();

        assert_eq!(result.status, ModelResultStatus::Succeeded);
        assert_eq!(result.text, "{\"ok\":true}");
        let request = received.recv_timeout(Duration::from_secs(2)).unwrap();
        let request_lower = request.to_ascii_lowercase();
        assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
        assert!(
            request_lower.contains(&format!("authorization: bearer {secret}").to_ascii_lowercase())
        );
        assert!(request_lower.contains("content-type: application/json"));
        assert!(request.contains("\"model\":\"gpt-test\""));
        server.join().unwrap();
    }

    #[test]
    fn reqwest_transport_reports_http_error_without_leaking_key() {
        let secret = "sk-do-not-leak";
        let (base_url, _received, server) = spawn_http_server(
            "401 Unauthorized",
            json!({
                "error": {
                    "message": format!("invalid API key: {secret}"),
                    "code": "invalid_api_key"
                }
            })
            .to_string(),
            Duration::ZERO,
        );
        let error = ReqwestOpenAiTransport::new()
            .unwrap()
            .post_chat_completion(&request(&base_url, secret, 3))
            .unwrap_err();

        assert!(error.message().contains("HTTP 401 Unauthorized"));
        assert!(error.message().contains("invalid_api_key"));
        assert!(error.message().contains("[REDACTED]"));
        assert!(!error.message().contains(secret));
        server.join().unwrap();
    }

    #[test]
    fn reqwest_transport_reports_invalid_json_with_body_context() {
        let (base_url, _received, server) =
            spawn_http_server("200 OK", "not-json".to_string(), Duration::ZERO);
        let error = ReqwestOpenAiTransport::new()
            .unwrap()
            .post_chat_completion(&request(&base_url, "sk-test", 3))
            .unwrap_err();

        assert!(error.message().contains("invalid JSON"));
        assert!(error.message().contains("not-json"));
        server.join().unwrap();
    }

    #[test]
    fn reqwest_transport_enforces_request_timeout() {
        let (base_url, _received, server) =
            spawn_http_server("200 OK", "{}".to_string(), Duration::from_millis(1_250));
        let error = ReqwestOpenAiTransport::new()
            .unwrap()
            .post_chat_completion(&request(&base_url, "sk-test", 1))
            .unwrap_err();

        assert_eq!(
            error.message(),
            "OpenAI-compatible request timed out after 1 seconds"
        );
        server.join().unwrap();
    }

    #[test]
    fn openai_settings_reject_zero_timeout() {
        let entry = ApiEntry {
            id: "completion".to_string(),
            config_type: "openai_completion_api".to_string(),
            api_url: "https://api.example.test/v1".to_string(),
            api_key: "sk-test".to_string(),
            extra_json: json!({"model": "gpt-test", "timeout_seconds": 0}),
            ..ApiEntry::default()
        };

        let error = openai_settings_from_entry(&entry).unwrap_err();
        assert!(
            error
                .message()
                .contains("timeout must be greater than zero")
        );
    }

    #[test]
    fn secret_bearing_http_types_have_redacted_debug_views() {
        let secret = "sk-never-debug-this";
        let settings = OpenAiCompletionSettings {
            provider: "openai".to_string(),
            base_url: "https://example.test/v1".to_string(),
            api_key: secret.to_string(),
            model: "gpt-test".to_string(),
            temperature: None,
            timeout_seconds: 10,
            reasoning_effort: None,
        };
        let request = build_openai_completion_request(&settings, &task("hello")).unwrap();
        let settings_debug = format!("{settings:?}");
        let request_debug = format!("{request:?}");
        for debug in [settings_debug, request_debug] {
            assert!(!debug.contains(secret));
            assert!(!debug.contains("hello"));
        }
    }

    #[test]
    fn completion_urls_reject_embedded_credentials_query_and_fragment() {
        for (url, expected) in [
            (
                "https://user:password@example.test/v1",
                "embedded credentials",
            ),
            (
                "https://example.test/v1?api_key=url-private-value",
                "query parameters",
            ),
            ("https://example.test/v1#private-fragment", "fragment"),
        ] {
            let entry = ApiEntry {
                id: "unsafe-url".to_string(),
                config_type: "openai_completion_api".to_string(),
                api_url: url.to_string(),
                api_key: "sk-test".to_string(),
                extra_json: json!({"model": "gpt-test"}),
                ..ApiEntry::default()
            };
            let error = openai_settings_from_entry(&entry).unwrap_err();
            assert!(error.message().contains(expected));
            assert!(!error.message().contains("password"));
            assert!(!error.message().contains("url-private-value"));
            assert!(!error.message().contains("private-fragment"));
        }
    }

    #[test]
    fn completion_http_is_limited_to_loopback_at_every_request_boundary() {
        let loopback = ApiEntry {
            id: "local".to_string(),
            config_type: "openai_completion_api".to_string(),
            api_url: "http://localhost:11434".to_string(),
            extra_json: json!({"model": "local-model", "auth_mode": "none"}),
            ..ApiEntry::default()
        };
        let settings =
            openai_settings_from_values(&loopback, &loopback.api_url, "", "local-model", true)
                .unwrap();
        assert_eq!(settings.base_url, "http://localhost:11434/v1");

        let mut remote = loopback.clone();
        remote.id = "remote".to_string();
        remote.api_url = "http://api.private-example.test".to_string();
        let settings_error =
            openai_settings_from_values(&remote, &remote.api_url, "", "remote-model", true)
                .unwrap_err();
        assert!(settings_error.message().contains("must use HTTPS"));
        assert!(!settings_error.message().contains("private-example"));

        let unsafe_settings = OpenAiCompletionSettings {
            provider: "openai".to_string(),
            base_url: remote.api_url.clone(),
            api_key: String::new(),
            model: "remote-model".to_string(),
            temperature: None,
            timeout_seconds: 10,
            reasoning_effort: None,
        };
        let request_error =
            build_openai_completion_request(&unsafe_settings, &task("hello")).unwrap_err();
        assert!(request_error.message().contains("must use HTTPS"));

        let transport_error = ReqwestOpenAiTransport::new()
            .unwrap()
            .post_chat_completion(&request(
                "http://api.private-example.test/v1/chat/completions",
                "sk-test",
                3,
            ))
            .unwrap_err();
        assert!(transport_error.message().contains("must use HTTPS"));
        assert!(!transport_error.message().contains("private-example"));
    }

    #[test]
    fn reqwest_errors_drop_the_attached_url_before_formatting() {
        let raw = Client::new()
            .get("ftp://user:password@example.test/private?api_key=url-private-value")
            .send()
            .unwrap_err();
        let safe = request_error(raw, 10, &[]);
        assert!(!safe.message().contains("example.test"));
        assert!(!safe.message().contains("password"));
        assert!(!safe.message().contains("url-private-value"));
    }

    #[test]
    fn cli_output_redaction_covers_assignments_tokens_and_jwts() {
        let raw = concat!(
            "GH_TOKEN=github-private-value ",
            "{\"AWS_SECRET_ACCESS_KEY\":\"aws-private-value\"} ",
            "github_pat_1234567890abcdef ",
            "AIza1234567890abcdef ",
            "eyJhbGci.eyJzdWIx.c2lnbmF0dXJl"
        );
        let redacted = redact_cli_output(raw, &[]);

        for private in [
            "github-private-value",
            "aws-private-value",
            "github_pat_1234567890abcdef",
            "AIza1234567890abcdef",
            "eyJhbGci.eyJzdWIx.c2lnbmF0dXJl",
        ] {
            assert!(!redacted.contains(private));
        }
        assert!(redacted.contains("[REDACTED]"));
    }

    #[cfg(windows)]
    #[test]
    fn cli_output_is_drained_bounded_and_redacted() {
        let secret = "sk-cli-super-secret";
        let command = CliCommandSpec {
            program: "powershell.exe".to_string(),
            args: vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "[Console]::Out.Write(('x' * 70000)); [Console]::Error.Write($env:TEST_SECRET); exit 1"
                    .to_string(),
            ],
            stdin: String::new(),
            timeout_seconds: 10,
            env: BTreeMap::from([("TEST_SECRET".to_string(), secret.to_string())]),
            current_dir: String::new(),
        };

        let result = run_cli_command("bounded", &command).unwrap();

        assert_eq!(result.status, ModelResultStatus::Failed);
        assert!(result.text.len() < 66_000);
        assert!(result.text.contains("[output truncated]"));
        assert!(!result.errors.join("\n").contains(secret));
        assert!(result.errors.join("\n").contains("[REDACTED]"));
    }

    #[cfg(windows)]
    #[test]
    fn cli_timeout_still_applies_while_the_child_does_not_read_stdin() {
        let command = CliCommandSpec {
            program: "powershell.exe".to_string(),
            args: vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "Start-Sleep -Seconds 30".to_string(),
            ],
            stdin: "x".repeat(8 * 1024 * 1024),
            timeout_seconds: 1,
            env: BTreeMap::new(),
            current_dir: String::new(),
        };
        let started = Instant::now();

        let result = run_cli_command("blocked-stdin", &command).unwrap();

        assert_eq!(result.status, ModelResultStatus::Failed);
        assert!(result.errors.join("\n").contains("timed out"));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    struct FakeOpenAiTransport;

    impl OpenAiCompletionTransport for FakeOpenAiTransport {
        fn post_chat_completion(&self, request: &OpenAiRequestSpec) -> AdmResult<Value> {
            assert_eq!(
                request.endpoint,
                "https://api.example.test/v1/chat/completions"
            );
            assert_eq!(request.payload["model"], json!("gpt-5.5"));
            Ok(json!({
                "choices": [
                    {"message": {"content": "{\"ok\":true}"}}
                ]
            }))
        }
    }

    fn request(endpoint: &str, secret: &str, timeout_seconds: u64) -> OpenAiRequestSpec {
        OpenAiRequestSpec {
            endpoint: endpoint.to_string(),
            headers: BTreeMap::from([
                ("Authorization".to_string(), format!("Bearer {secret}")),
                ("Content-Type".to_string(), "application/json".to_string()),
            ]),
            payload: json!({"model": "gpt-test", "messages": []}),
            timeout_seconds,
        }
    }

    fn spawn_http_server(
        status: &str,
        body: String,
        delay: Duration,
    ) -> (String, Receiver<String>, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_string();
        let (sender, receiver) = mpsc::channel();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 4_096];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => {
                        bytes.extend_from_slice(&buffer[..count]);
                        if request_is_complete(&bytes) {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = sender.send(String::from_utf8_lossy(&bytes).to_string());
            thread::sleep(delay);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        });
        (format!("http://{address}"), receiver, handle)
    }

    fn request_is_complete(bytes: &[u8]) -> bool {
        let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
            return false;
        };
        let headers = String::from_utf8_lossy(&bytes[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or_default();
        bytes.len() >= header_end + 4 + content_length
    }

    fn task(prompt: &str) -> ModelTask {
        ModelTask {
            task_id: "task".to_string(),
            prompt: prompt.to_string(),
            input_files: Vec::new(),
            output_files: Vec::new(),
            allowed_write_paths: Vec::new(),
            timeout_seconds: 0,
            sandbox: "workspace-write".to_string(),
            cwd: String::new(),
        }
    }
}
