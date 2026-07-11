use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use adm_new_foundation::process::terminate_child_process_tree;
use serde::{Deserialize, Serialize};

pub const CLI_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
pub const CLI_PROBE_OUTPUT_LIMIT: usize = 16 * 1024;

#[derive(Clone, PartialEq, Eq)]
pub struct CliProbeRequest {
    program: String,
    args: Vec<String>,
    secrets: Vec<String>,
}

impl CliProbeRequest {
    pub fn new(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            secrets: Vec::new(),
        }
    }

    pub fn with_secrets(mut self, secrets: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.secrets = secrets.into_iter().map(Into::into).collect();
        self
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }
}

impl fmt::Debug for CliProbeRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CliProbeRequest")
            .field("program_configured", &!self.program.trim().is_empty())
            .field("arg_count", &self.args.len())
            .field("secret_count", &self.secrets.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliProbeSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliProbeDiagnostic {
    pub severity: CliProbeSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliProbeReport {
    pub available: bool,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub diagnostics: Vec<CliProbeDiagnostic>,
}

impl CliProbeReport {
    fn unavailable(started: Instant, code: &str, message: String) -> Self {
        Self {
            available: false,
            success: false,
            exit_code: None,
            timed_out: false,
            duration_ms: elapsed_millis(started),
            stdout: String::new(),
            stderr: String::new(),
            stdout_truncated: false,
            stderr_truncated: false,
            diagnostics: vec![CliProbeDiagnostic {
                severity: CliProbeSeverity::Warning,
                code: code.to_string(),
                message,
            }],
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct CliLocation {
    pub program: String,
    pub available: bool,
    pub used_explicit_path: bool,
    pub diagnostics: Vec<String>,
}

impl fmt::Debug for CliLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CliLocation")
            .field("program_configured", &!self.program.trim().is_empty())
            .field("available", &self.available)
            .field("used_explicit_path", &self.used_explicit_path)
            .field("diagnostic_count", &self.diagnostics.len())
            .finish()
    }
}

pub fn locate_cli_program(explicit: Option<&str>, default_program: &str) -> CliLocation {
    let path_env = std::env::var_os("PATH")
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default();
    let path_ext = std::env::var_os("PATHEXT")
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_default();
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    locate_cli_program_in(
        explicit,
        default_program,
        &path_env,
        &path_ext,
        cfg!(windows),
        &current_dir,
    )
}

pub fn locate_cli_program_in(
    explicit: Option<&str>,
    default_program: &str,
    path_env: &str,
    path_ext: &str,
    windows: bool,
    current_dir: &Path,
) -> CliLocation {
    let explicit = explicit.map(str::trim).filter(|value| !value.is_empty());
    let mut diagnostics = Vec::new();
    if let Some(requested) = explicit {
        if let Some(program) = find_program(requested, path_env, path_ext, windows, current_dir) {
            return CliLocation {
                program: program.to_string_lossy().into_owned(),
                available: true,
                used_explicit_path: true,
                diagnostics,
            };
        }
        diagnostics.push("configured CLI program was not found; falling back to PATH".to_string());
    }

    if let Some(program) = find_program(default_program, path_env, path_ext, windows, current_dir) {
        return CliLocation {
            program: program.to_string_lossy().into_owned(),
            available: true,
            used_explicit_path: false,
            diagnostics,
        };
    }

    diagnostics.push(format!(
        "CLI program is not installed or is not available on PATH: {default_program}"
    ));
    CliLocation {
        program: explicit.unwrap_or(default_program).to_string(),
        available: false,
        used_explicit_path: explicit.is_some(),
        diagnostics,
    }
}

pub fn probe_cli(request: &CliProbeRequest) -> CliProbeReport {
    let started = Instant::now();
    if request.program.trim().is_empty() {
        return CliProbeReport::unavailable(
            started,
            "cli_program_empty",
            "CLI program cannot be empty".to_string(),
        );
    }

    let mut command = Command::new(&request.program);
    command
        .args(&request.args)
        .env_clear()
        .envs(crate::adapters::minimal_cli_environment())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_hidden_windows_process(&mut command);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return CliProbeReport::unavailable(
                started,
                "cli_unavailable",
                redact_probe_text(
                    &format!("CLI program is unavailable: {}", error),
                    &request.secrets,
                ),
            );
        }
    };
    let stdout_reader = child.stdout.take().map(spawn_capped_reader);
    let stderr_reader = child.stderr.take().map(spawn_capped_reader);
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() < CLI_PROBE_TIMEOUT => {
                thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                timed_out = true;
                break terminate_child_process_tree(&mut child).ok();
            }
            Err(error) => {
                let _ = terminate_child_process_tree(&mut child);
                let (stdout, stdout_truncated) = join_capped_reader(stdout_reader);
                let (stderr, stderr_truncated) = join_capped_reader(stderr_reader);
                return CliProbeReport {
                    available: true,
                    success: false,
                    exit_code: None,
                    timed_out: false,
                    duration_ms: elapsed_millis(started),
                    stdout: redact_probe_text(&stdout, &request.secrets),
                    stderr: redact_probe_text(&stderr, &request.secrets),
                    stdout_truncated,
                    stderr_truncated,
                    diagnostics: vec![CliProbeDiagnostic {
                        severity: CliProbeSeverity::Error,
                        code: "cli_probe_wait_failed".to_string(),
                        message: redact_probe_text(
                            &format!("failed while waiting for CLI probe: {error}"),
                            &request.secrets,
                        ),
                    }],
                };
            }
        }
    };
    let (stdout, stdout_truncated) = join_capped_reader(stdout_reader);
    let (stderr, stderr_truncated) = join_capped_reader(stderr_reader);
    let success = status.is_some_and(|status| status.success()) && !timed_out;
    let mut diagnostics = Vec::new();
    if timed_out {
        diagnostics.push(CliProbeDiagnostic {
            severity: CliProbeSeverity::Error,
            code: "cli_probe_timeout".to_string(),
            message: "CLI probe timed out after 5 seconds".to_string(),
        });
    } else if !success {
        diagnostics.push(CliProbeDiagnostic {
            severity: CliProbeSeverity::Warning,
            code: "cli_probe_failed".to_string(),
            message: "CLI probe exited without reporting success".to_string(),
        });
    }

    CliProbeReport {
        available: true,
        success,
        exit_code: status.and_then(|status| status.code()),
        timed_out,
        duration_ms: elapsed_millis(started),
        stdout: redact_probe_text(&stdout, &request.secrets),
        stderr: redact_probe_text(&stderr, &request.secrets),
        stdout_truncated,
        stderr_truncated,
        diagnostics,
    }
}

fn find_program(
    requested: &str,
    path_env: &str,
    path_ext: &str,
    windows: bool,
    current_dir: &Path,
) -> Option<PathBuf> {
    if looks_like_path(requested) {
        let requested = PathBuf::from(requested);
        let requested = if requested.is_absolute() {
            requested
        } else {
            current_dir.join(requested)
        };
        return executable_candidates(&requested, path_ext, windows)
            .into_iter()
            .find(|candidate| is_runnable_file(candidate, windows));
    }

    for directory in split_search_path(path_env, windows) {
        let base = directory.join(requested);
        if let Some(candidate) = executable_candidates(&base, path_ext, windows)
            .into_iter()
            .find(|candidate| is_runnable_file(candidate, windows))
        {
            return Some(candidate);
        }
    }
    None
}

fn executable_candidates(base: &Path, path_ext: &str, windows: bool) -> Vec<PathBuf> {
    let mut candidates = vec![base.to_path_buf()];
    if !windows || base.extension().is_some() {
        return candidates;
    }
    let mut extensions = path_ext
        .split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_start_matches('.').to_ascii_lowercase())
        .collect::<Vec<_>>();
    if extensions.is_empty() {
        extensions = vec!["exe".to_string(), "cmd".to_string(), "bat".to_string()];
    }
    for required in ["exe", "cmd", "bat"] {
        if !extensions.iter().any(|extension| extension == required) {
            extensions.push(required.to_string());
        }
    }
    candidates.extend(
        extensions
            .into_iter()
            .map(|extension| base.with_extension(extension)),
    );
    candidates
}

fn split_search_path(path_env: &str, windows: bool) -> Vec<PathBuf> {
    if windows {
        path_env
            .split(';')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .collect()
    } else {
        std::env::split_paths(path_env).collect()
    }
}

fn looks_like_path(value: &str) -> bool {
    Path::new(value).is_absolute() || value.contains('/') || value.contains('\\')
}

fn is_runnable_file(path: &Path, windows: bool) -> bool {
    if !path.is_file() {
        return false;
    }
    if windows {
        return true;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        return std::fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
    }
    #[cfg(not(unix))]
    true
}

fn spawn_capped_reader<R>(mut reader: R) -> thread::JoinHandle<(String, bool)>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut kept = Vec::with_capacity(CLI_PROBE_OUTPUT_LIMIT.min(4096));
        let mut truncated = false;
        let mut chunk = [0_u8; 4096];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(count) => {
                    let remaining = CLI_PROBE_OUTPUT_LIMIT.saturating_sub(kept.len());
                    let copy_count = remaining.min(count);
                    kept.extend_from_slice(&chunk[..copy_count]);
                    truncated |= copy_count < count;
                }
                Err(_) => break,
            }
        }
        (String::from_utf8_lossy(&kept).into_owned(), truncated)
    })
}

fn join_capped_reader(reader: Option<thread::JoinHandle<(String, bool)>>) -> (String, bool) {
    reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default()
}

fn elapsed_millis(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

pub fn redact_probe_text(value: &str, secrets: &[String]) -> String {
    let mut redacted = secrets.iter().fold(value.to_string(), |text, secret| {
        if secret.trim().is_empty() {
            text
        } else {
            text.replace(secret, "[REDACTED]")
        }
    });
    redacted = redact_token_after_prefix(&redacted, "Bearer ");
    redact_token_after_prefix(&redacted, "sk-")
}

fn redact_token_after_prefix(value: &str, prefix: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(offset) = remaining.find(prefix) {
        output.push_str(&remaining[..offset]);
        output.push_str(prefix);
        output.push_str("[REDACTED]");
        let token = &remaining[offset + prefix.len()..];
        let end = token
            .find(|character: char| {
                character.is_whitespace()
                    || matches!(character, '"' | '\'' | ',' | ';' | ')' | ']' | '}')
            })
            .unwrap_or(token.len());
        remaining = &token[end..];
    }
    output.push_str(remaining);
    output
}

#[cfg(windows)]
fn configure_hidden_windows_process(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_hidden_windows_process(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};

    use super::*;

    #[test]
    fn windows_locator_supports_bare_exe_cmd_and_bat_programs() {
        let root = test_dir("windows_locator");
        fs::create_dir_all(&root).unwrap();
        for extension in ["exe", "cmd", "bat"] {
            let path = root.join(format!("tool_{extension}.{extension}"));
            File::create(&path).unwrap();
            let located = locate_cli_program_in(
                None,
                &format!("tool_{extension}"),
                &root.to_string_lossy(),
                ".EXE;.CMD;.BAT",
                true,
                &root,
            );
            assert!(located.available, "failed to locate {extension}");
            assert!(located.program.ends_with(&format!(".{extension}")));
        }
        let bare = root.join("tool_bare");
        File::create(&bare).unwrap();
        let located =
            locate_cli_program_in(None, "tool_bare", &root.to_string_lossy(), "", true, &root);
        assert!(located.available);
        assert!(located.program.ends_with("tool_bare"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_cli_path_has_priority_then_falls_back_to_path() {
        let root = test_dir("explicit_priority");
        let explicit_dir = root.join("explicit");
        let path_dir = root.join("path");
        fs::create_dir_all(&explicit_dir).unwrap();
        fs::create_dir_all(&path_dir).unwrap();
        let explicit = explicit_dir.join("codex.cmd");
        let fallback = path_dir.join("codex.exe");
        File::create(&explicit).unwrap();
        File::create(&fallback).unwrap();

        let located = locate_cli_program_in(
            Some(&explicit.to_string_lossy()),
            "codex",
            &path_dir.to_string_lossy(),
            ".EXE;.CMD;.BAT",
            true,
            &root,
        );
        assert!(located.used_explicit_path);
        assert_eq!(Path::new(&located.program), explicit);

        fs::remove_file(&explicit).unwrap();
        let fallback_location = locate_cli_program_in(
            Some(&explicit.to_string_lossy()),
            "codex",
            &path_dir.to_string_lossy(),
            ".EXE;.CMD;.BAT",
            true,
            &root,
        );
        assert!(fallback_location.available);
        assert!(!fallback_location.used_explicit_path);
        assert_eq!(Path::new(&fallback_location.program), fallback);
        assert_eq!(fallback_location.diagnostics.len(), 1);
        assert!(
            !fallback_location
                .diagnostics
                .join(" ")
                .contains(explicit_dir.to_string_lossy().as_ref())
        );
        assert!(
            !format!("{fallback_location:?}").contains(explicit_dir.to_string_lossy().as_ref())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unavailable_cli_is_an_availability_warning() {
        let located = locate_cli_program_in(
            None,
            "definitely-not-installed-adm-cli",
            "",
            "",
            true,
            Path::new("."),
        );

        assert!(!located.available);
        assert!(located.diagnostics[0].contains("not installed"));
    }

    #[test]
    fn probe_uses_direct_program_and_reports_success() {
        let executable = std::env::current_exe().unwrap();
        let report = probe_cli(&CliProbeRequest::new(
            executable.to_string_lossy(),
            ["--help"],
        ));

        assert!(report.available);
        assert!(report.success);
        assert!(!report.timed_out);
    }

    #[test]
    fn probe_output_redacts_explicit_and_well_known_tokens() {
        let secret = "private-value".to_string();
        let output = redact_probe_text(
            "private-value Bearer bearer-token sk-example-token trailing",
            std::slice::from_ref(&secret),
        );

        assert!(!output.contains("private-value"));
        assert!(!output.contains("bearer-token"));
        assert!(!output.contains("example-token"));
        assert!(output.contains("[REDACTED]"));
    }

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm-new-ai-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
