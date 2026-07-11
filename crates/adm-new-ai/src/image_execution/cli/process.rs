use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use adm_new_foundation::process::terminate_child_process_tree;
use adm_new_foundation::{AdmError, AdmResult};

use crate::adapters::minimal_cli_environment;

const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, PartialEq, Eq)]
pub struct ImageProcessRequest {
    pub program: String,
    pub args: Vec<String>,
    pub current_dir: PathBuf,
    pub env: BTreeMap<String, String>,
    pub stdin: Vec<u8>,
    pub timeout: Duration,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
}

impl fmt::Debug for ImageProcessRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageProcessRequest")
            .field("program_configured", &!self.program.trim().is_empty())
            .field("arg_count", &self.args.len())
            .field("current_dir_configured", &true)
            .field("env_keys", &self.env.keys().collect::<Vec<_>>())
            .field("stdin_bytes", &self.stdin.len())
            .field("timeout", &self.timeout)
            .field("max_stdout_bytes", &self.max_stdout_bytes)
            .field("max_stderr_bytes", &self.max_stderr_bytes)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ImageProcessOutput {
    success: bool,
    timed_out: bool,
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_truncated: bool,
    stderr_truncated: bool,
}

impl ImageProcessOutput {
    pub fn completed(
        success: bool,
        exit_code: Option<i32>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    ) -> Self {
        Self {
            success,
            timed_out: false,
            exit_code,
            stdout,
            stderr,
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    pub fn timed_out(stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self {
            success: false,
            timed_out: true,
            exit_code: None,
            stdout,
            stderr,
            stdout_truncated: false,
            stderr_truncated: false,
        }
    }

    pub(super) fn succeeded(&self) -> bool {
        self.success
    }

    pub(super) fn did_time_out(&self) -> bool {
        self.timed_out
    }

    pub(super) fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub(super) fn stdout_was_truncated(&self) -> bool {
        self.stdout_truncated
    }
}

impl fmt::Debug for ImageProcessOutput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageProcessOutput")
            .field("success", &self.success)
            .field("timed_out", &self.timed_out)
            .field("exit_code", &self.exit_code)
            .field("stdout_bytes", &self.stdout.len())
            .field("stderr_bytes", &self.stderr.len())
            .field("stdout_truncated", &self.stdout_truncated)
            .field("stderr_truncated", &self.stderr_truncated)
            .finish()
    }
}

pub trait ImageProcessRunner: Send + Sync {
    fn run(&self, request: &ImageProcessRequest) -> AdmResult<ImageProcessOutput>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemImageProcessRunner;

impl ImageProcessRunner for SystemImageProcessRunner {
    fn run(&self, request: &ImageProcessRequest) -> AdmResult<ImageProcessOutput> {
        if request.program.trim().is_empty() || request.timeout.is_zero() {
            return Err(AdmError::new("invalid image process configuration"));
        }
        let mut command = Command::new(&request.program);
        command
            .args(&request.args)
            .current_dir(&request.current_dir)
            .env_clear()
            .envs(minimal_cli_environment())
            .envs(&request.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }

        let mut child = command
            .spawn()
            .map_err(|_| AdmError::new("failed to start image process"))?;
        let stdin = child.stdin.take();
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AdmError::new("failed to capture image process output"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| AdmError::new("failed to capture image process output"))?;

        let input = request.stdin.clone();
        let stdin_thread = thread::spawn(move || {
            if let Some(mut stream) = stdin {
                let _ = stream.write_all(&input);
            }
        });
        let stdout_limit = request.max_stdout_bytes;
        let stderr_limit = request.max_stderr_bytes;
        let stdout_thread = thread::spawn(move || read_bounded(stdout, stdout_limit));
        let stderr_thread = thread::spawn(move || read_bounded(stderr, stderr_limit));

        let started = Instant::now();
        let (status, timed_out) = loop {
            match child.try_wait() {
                Ok(Some(status)) => break (status, false),
                Ok(None) if started.elapsed() < request.timeout => {
                    thread::sleep(PROCESS_POLL_INTERVAL);
                }
                Ok(None) => {
                    let status = terminate_child_process_tree(&mut child)
                        .map_err(|_| AdmError::new("failed to reap timed out image process"))?;
                    break (status, true);
                }
                Err(_) => {
                    let _ = terminate_child_process_tree(&mut child);
                    return Err(AdmError::new("failed to monitor image process"));
                }
            }
        };
        let _ = stdin_thread.join();
        let stdout = join_capture(stdout_thread)?;
        let stderr = join_capture(stderr_thread)?;
        Ok(ImageProcessOutput {
            success: !timed_out && status.success(),
            timed_out,
            exit_code: status.code(),
            stdout: stdout.bytes,
            stderr: stderr.bytes,
            stdout_truncated: stdout.truncated,
            stderr_truncated: stderr.truncated,
        })
    }
}

struct BoundedCapture {
    bytes: Vec<u8>,
    truncated: bool,
}

fn read_bounded(mut reader: impl Read, limit: usize) -> io::Result<BoundedCapture> {
    let mut bytes = Vec::with_capacity(limit.min(8 * 1024));
    let mut truncated = false;
    let mut buffer = [0_u8; 8 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = limit.saturating_sub(bytes.len());
        let retained = remaining.min(count);
        bytes.extend_from_slice(&buffer[..retained]);
        truncated |= retained < count;
    }
    Ok(BoundedCapture { bytes, truncated })
}

fn join_capture(
    handle: thread::JoinHandle<io::Result<BoundedCapture>>,
) -> AdmResult<BoundedCapture> {
    handle
        .join()
        .map_err(|_| AdmError::new("image process output reader failed"))?
        .map_err(|_| AdmError::new("image process output could not be read"))
}
