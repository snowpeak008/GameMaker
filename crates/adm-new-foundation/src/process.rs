use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io;
use std::process::{Child, ExitStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HiddenSubprocessOptions {
    pub stdin_null: bool,
    pub windows_create_no_window: bool,
    pub env: BTreeMap<String, String>,
}

pub fn child_process_env(
    base: &BTreeMap<String, String>,
    extra: Option<&BTreeMap<String, String>>,
) -> BTreeMap<String, String> {
    let mut env = base.clone();
    env.entry("PYTHONUNBUFFERED".to_string())
        .or_insert_with(|| "1".to_string());
    env.entry("PYTHONIOENCODING".to_string())
        .or_insert_with(|| "utf-8".to_string());
    if let Some(extra) = extra {
        for (key, value) in extra {
            env.insert(key.clone(), value.clone());
        }
    }
    env
}

pub fn current_child_process_env(
    extra: Option<&BTreeMap<String, String>>,
) -> BTreeMap<String, String> {
    let base = std::env::vars().collect();
    child_process_env(&base, extra)
}

pub fn hidden_subprocess_options(
    env: BTreeMap<String, String>,
    stdin_null: bool,
) -> HiddenSubprocessOptions {
    HiddenSubprocessOptions {
        stdin_null,
        windows_create_no_window: cfg!(windows),
        env,
    }
}

/// Terminates the launched process and, on Windows, also asks the OS to
/// terminate descendants created by a provider CLI.
pub fn terminate_child_process_tree(child: &mut Child) -> io::Result<ExitStatus> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};

        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = Command::new("taskkill.exe")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    let _ = child.kill();
    child.wait()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_process_env_adds_python_defaults_and_extra() {
        let base = BTreeMap::from([("PATH".to_string(), "bin".to_string())]);
        let extra = BTreeMap::from([("CODEX_HOME".to_string(), "home".to_string())]);
        let env = child_process_env(&base, Some(&extra));

        assert_eq!(env.get("PYTHONUNBUFFERED").unwrap(), "1");
        assert_eq!(env.get("PYTHONIOENCODING").unwrap(), "utf-8");
        assert_eq!(env.get("CODEX_HOME").unwrap(), "home");
        assert_eq!(env.get("PATH").unwrap(), "bin");
    }

    #[test]
    fn hidden_options_model_gui_safe_subprocess_policy() {
        let env = child_process_env(&BTreeMap::new(), None);
        let options = hidden_subprocess_options(env, true);

        assert!(options.stdin_null);
        assert_eq!(options.windows_create_no_window, cfg!(windows));
    }
}
