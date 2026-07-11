use std::collections::BTreeMap;

use adm_new_foundation::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectApiConfigSpec {
    pub path: String,
    pub exists: bool,
    pub active_profile: String,
    pub description: String,
    pub env: BTreeMap<String, String>,
    pub config: BTreeMap<String, Value>,
    pub codex_home: String,
    pub ignore_user_config: bool,
    pub write_codex_auth_file: bool,
    pub profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodexInterviewCommandSpec {
    pub prompt_path: String,
    pub output_path: String,
    pub schema_path: String,
    pub workdir: String,
    pub session_id: String,
    pub config_args: Vec<String>,
    pub use_schema: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InterviewBackendTaskSpec {
    pub backend_name: String,
    pub prompt: String,
    pub schema_mode: String,
    pub schema_name: String,
    pub session_id: String,
    pub timeout_seconds: u64,
    pub read_only: bool,
}

pub fn toml_cli_value(value: &Value) -> String {
    match value {
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
        }
        Value::String(value) => serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string()),
        Value::Null => "\"\"".to_string(),
    }
}

pub fn project_api_config_from_value(
    runtime_root: &str,
    payload: Option<&Value>,
) -> AdmResult<ProjectApiConfigSpec> {
    let path = format!(
        "{}/ai_api_config.json",
        runtime_root.trim_end_matches(['/', '\\'])
    );
    let Some(payload) = payload else {
        return Ok(ProjectApiConfigSpec {
            path,
            exists: false,
            active_profile: "global_codex".to_string(),
            description: "使用全局 Codex 配置".to_string(),
            env: BTreeMap::new(),
            config: BTreeMap::new(),
            codex_home: String::new(),
            ignore_user_config: false,
            write_codex_auth_file: true,
            profile: String::new(),
        });
    };
    let Some(root) = payload.as_object() else {
        return Err(AdmError::new("AI API config must be a JSON object"));
    };
    let profiles = root.get("profiles").and_then(Value::as_object);
    let mut active = root
        .get("activeProfile")
        .or_else(|| root.get("active_profile"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let profile_payload = if let Some(profiles) = profiles {
        if active.is_empty() {
            active = profiles.keys().next().cloned().unwrap_or_default();
        }
        profiles
            .get(&active)
            .ok_or_else(|| AdmError::new(format!("AI API profile is missing: {active}")))?
    } else {
        if active.is_empty() {
            active = "default".to_string();
        }
        payload
    };
    let Some(profile) = profile_payload.as_object() else {
        return Err(AdmError::new(format!(
            "AI API profile is not an object: {active}"
        )));
    };
    let env = profile
        .get("env")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| {
                    let key = key.trim();
                    (!key.is_empty()).then(|| {
                        (
                            key.to_string(),
                            value
                                .as_str()
                                .map(str::to_string)
                                .unwrap_or_else(|| value.to_string()),
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let config = profile
        .get("config")
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter(|(key, _)| !key.trim().is_empty())
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default();
    let codex_home = profile
        .get("codexHome")
        .or_else(|| profile.get("codex_home"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Ok(ProjectApiConfigSpec {
        path,
        exists: true,
        active_profile: active,
        description: profile
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        env,
        config,
        codex_home,
        ignore_user_config: bool_field(profile_payload, "ignoreUserConfig", "ignore_user_config"),
        write_codex_auth_file: bool_field(
            profile_payload,
            "writeCodexAuthFile",
            "write_codex_auth_file",
        )
        .then_some(true)
        .unwrap_or_else(|| {
            profile_payload
                .get("writeCodexAuthFile")
                .or_else(|| profile_payload.get("write_codex_auth_file"))
                .and_then(Value::as_bool)
                .unwrap_or(true)
        }),
        profile: profile
            .get("codexProfile")
            .or_else(|| profile.get("profile"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

pub fn codex_config_args(config: &ProjectApiConfigSpec) -> Vec<String> {
    let mut args = Vec::new();
    if config.ignore_user_config {
        args.push("--ignore-user-config".to_string());
    }
    if !config.profile.trim().is_empty() {
        args.extend(["--profile".to_string(), config.profile.clone()]);
    }
    for (key, value) in &config.config {
        args.extend(["-c".to_string(), format!("{key}={}", toml_cli_value(value))]);
    }
    args
}

pub fn build_codex_interview_args(spec: &CodexInterviewCommandSpec) -> Vec<String> {
    let prompt_arg = format!(
        "Read the UTF-8 file at {}. Follow the instructions in that file. Return only the final JSON object requested by the instructions. Do not edit files.",
        spec.prompt_path
    );
    let mut args = vec!["exec".to_string()];
    args.extend(spec.config_args.clone());
    if !spec.session_id.trim().is_empty() {
        args.push("resume".to_string());
        args.push("--skip-git-repo-check".to_string());
        args.push("--json".to_string());
        if spec.use_schema {
            args.extend(["--output-schema".to_string(), spec.schema_path.clone()]);
        }
        args.extend(["-o".to_string(), spec.output_path.clone()]);
        args.push(spec.session_id.clone());
        args.push(prompt_arg);
    } else {
        args.push("--skip-git-repo-check".to_string());
        args.extend(["-C".to_string(), spec.workdir.clone()]);
        args.extend(["-s".to_string(), "read-only".to_string()]);
        args.push("--json".to_string());
        if spec.use_schema {
            args.extend(["--output-schema".to_string(), spec.schema_path.clone()]);
        }
        args.extend(["-o".to_string(), spec.output_path.clone()]);
        args.push(prompt_arg);
    }
    args
}

pub fn interview_backend_task_spec(
    prompt: &str,
    schema_mode: &str,
    session_id: &str,
) -> InterviewBackendTaskSpec {
    InterviewBackendTaskSpec {
        backend_name: "codex_cli".to_string(),
        prompt: prompt.to_string(),
        schema_mode: schema_mode.to_string(),
        schema_name: format!("codex_interview_{schema_mode}_response.schema.json"),
        session_id: session_id.to_string(),
        timeout_seconds: 90,
        read_only: true,
    }
}

pub fn parse_json_lines(output: &str) -> Vec<Value> {
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .collect()
}

pub fn extract_session_id(events: &[Value]) -> Option<String> {
    for event in events {
        if let Some(found) = extract_session_id_from_value(event) {
            return Some(found);
        }
    }
    None
}

pub fn extract_json_object(text: &str) -> AdmResult<Value> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AdmError::new("empty Codex response"));
    }
    if let Ok(value @ Value::Object(_)) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }
    if let Some(fenced) = extract_fenced_json(trimmed) {
        if let Ok(value @ Value::Object(_)) = serde_json::from_str::<Value>(&fenced) {
            return Ok(value);
        }
    }
    if let Some(braced) = extract_braced_object(trimmed) {
        if let Ok(value @ Value::Object(_)) = serde_json::from_str::<Value>(&braced) {
            return Ok(value);
        }
    }
    Err(AdmError::new(
        "Codex response did not contain a JSON object",
    ))
}

fn bool_field(value: &Value, camel: &str, snake: &str) -> bool {
    value
        .get(camel)
        .or_else(|| value.get(snake))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn extract_session_id_from_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if looks_like_uuid(text) => Some(text.clone()),
        Value::Array(items) => items.iter().find_map(extract_session_id_from_value),
        Value::Object(object) => {
            for key in [
                "session_id",
                "sessionId",
                "conversation_id",
                "conversationId",
                "thread_id",
                "threadId",
            ] {
                if let Some(text) = object.get(key).and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        return Some(text.to_string());
                    }
                }
            }
            object.values().find_map(extract_session_id_from_value)
        }
        _ => None,
    }
}

fn looks_like_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|index| bytes[*index] == b'-')
        && bytes
            .iter()
            .enumerate()
            .filter(|(index, _)| ![8, 13, 18, 23].contains(index))
            .all(|(_, byte)| byte.is_ascii_hexdigit())
}

fn extract_fenced_json(text: &str) -> Option<String> {
    let start = text.find("```")?;
    let rest = &text[start + 3..];
    let rest = rest.strip_prefix("json").unwrap_or(rest).trim_start();
    let end = rest.find("```")?;
    Some(rest[..end].trim().to_string())
}

fn extract_braced_object(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then(|| text[start..=end].to_string())
}
