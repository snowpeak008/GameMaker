#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

pub const DATA_DIR_ENV: &str = "ADM_NEWRUST_DATA_DIR";
pub const STARTUP_PROJECT_ENV: &str = "ADM_NEWRUST_STARTUP_PROJECT";
pub const LANGUAGE_ENV: &str = "ADM_NEWRUST_LANGUAGE";
pub const DEFAULT_STARTUP_PROJECT: &str = "blank";
pub const DEFAULT_LANGUAGE: &str = "zh-CN";
pub const PORTABLE_RELATIVE_ROOT: &str = "dist/AutoDesignMaker-NEWrust";
pub const ERROR_REPORT_NAME: &str = "AutoDesignMaker-launch-error.txt";

const REQUIRED_SOURCE_FILES: &[&str] = &[".project_root"];
const REQUIRED_PORTABLE_FILES: &[&str] = &[
    "AutoDesignMaker.exe",
    "build-manifest.json",
    "portable-resource-manifest.json",
    "knowledge/resource-manifest.json",
    "pipeline/artifact_layer/registry.json",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchLayout {
    pub source_root: PathBuf,
    pub portable_root: PathBuf,
    pub product_executable: PathBuf,
    pub data_root: PathBuf,
}

impl LaunchLayout {
    pub fn from_launcher_path(launcher_path: &Path) -> Result<Self, String> {
        let source_root = launcher_path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .ok_or_else(|| {
                format!(
                    "the launcher path has no parent directory: {}",
                    launcher_path.display()
                )
            })?
            .to_path_buf();
        let portable_root = source_root.join(PORTABLE_RELATIVE_ROOT);
        Ok(Self {
            product_executable: portable_root.join("AutoDesignMaker.exe"),
            data_root: portable_root.join("user_data"),
            source_root,
            portable_root,
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.source_root.is_dir() {
            return Err(format!(
                "the NEWrust source root does not exist: {}",
                self.source_root.display()
            ));
        }
        for relative in REQUIRED_SOURCE_FILES {
            require_file(&self.source_root.join(relative), "source entry file")?;
        }
        if !self.portable_root.is_dir() {
            return Err(format!(
                "the portable application directory is missing: {}\nRun tools\\build-portable.ps1 first.",
                self.portable_root.display()
            ));
        }
        for relative in REQUIRED_PORTABLE_FILES {
            require_file(
                &self.portable_root.join(relative),
                "portable application file",
            )?;
        }
        Ok(())
    }

    pub fn prepare_data_root(&self) -> Result<(), String> {
        fs::create_dir_all(&self.data_root).map_err(|error| {
            format!(
                "failed to create the Rust-owned data directory {}: {error}",
                self.data_root.display()
            )
        })
    }
}

fn require_file(path: &Path, kind: &str) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("required {kind} is missing ({}): {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(format!(
            "required {kind} is not a non-empty file: {}",
            path.display()
        ));
    }
    Ok(())
}

pub fn webview2_runtime_available() -> bool {
    if let Some(fixed_runtime) = std::env::var_os("WEBVIEW2_BROWSER_EXECUTABLE_FOLDER")
        && runtime_executable_exists(Path::new(&fixed_runtime))
    {
        return true;
    }

    ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .map(|base| base.join("Microsoft/EdgeWebView/Application"))
        .any(|root| versioned_webview2_runtime_exists(&root))
}

pub fn versioned_webview2_runtime_exists(application_root: &Path) -> bool {
    let Ok(entries) = fs::read_dir(application_root) else {
        return false;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .any(|path| path.is_dir() && runtime_executable_exists(&path))
}

fn runtime_executable_exists(root: &Path) -> bool {
    root.join("msedgewebview2.exe").is_file()
}

pub fn product_command(
    layout: &LaunchLayout,
    arguments: &[OsString],
    startup_project: Option<&OsStr>,
    language: Option<&OsStr>,
) -> Command {
    let mut command = Command::new(&layout.product_executable);
    command
        .args(arguments)
        .current_dir(&layout.portable_root)
        .env(DATA_DIR_ENV, &layout.data_root)
        .env(
            STARTUP_PROJECT_ENV,
            non_empty_or_default(startup_project, DEFAULT_STARTUP_PROJECT),
        )
        .env(
            LANGUAGE_ENV,
            non_empty_or_default(language, DEFAULT_LANGUAGE),
        );
    command
}

fn non_empty_or_default<'a>(value: Option<&'a OsStr>, default: &'a str) -> &'a OsStr {
    value
        .filter(|candidate| !candidate.is_empty())
        .unwrap_or_else(|| OsStr::new(default))
}

pub fn launch_product(layout: &LaunchLayout, arguments: &[OsString]) -> Result<Child, String> {
    let startup_project = std::env::var_os(STARTUP_PROJECT_ENV);
    let language = std::env::var_os(LANGUAGE_ENV);
    product_command(
        layout,
        arguments,
        startup_project.as_deref(),
        language.as_deref(),
    )
    .spawn()
    .map_err(|error| {
        format!(
            "failed to start the Rust desktop executable {}: {error}",
            layout.product_executable.display()
        )
    })
}

pub fn write_error_report(source_root: &Path, message: &str) -> io::Result<PathBuf> {
    let report_path = source_root.join(ERROR_REPORT_NAME);
    let report = format!(
        "AutoDesignMaker NEWrust could not be started.\r\n\r\n{message}\r\n\r\nWebView2 installer:\r\nhttps://go.microsoft.com/fwlink/p/?LinkId=2124703\r\n"
    );
    fs::write(&report_path, report)?;
    Ok(report_path)
}

pub fn clear_error_report(source_root: &Path) {
    match fs::remove_file(source_root.join(ERROR_REPORT_NAME)) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temp_root(name: &str) -> PathBuf {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "adm_new_root_launcher_{name}_{}_{}",
            std::process::id(),
            sequence
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn complete_layout(name: &str) -> LaunchLayout {
        let root = temp_root(name);
        fs::write(root.join(".project_root"), b"{}").unwrap();
        let layout = LaunchLayout::from_launcher_path(&root.join("AutoDesignMaker.exe")).unwrap();
        for relative in REQUIRED_PORTABLE_FILES {
            let path = layout.portable_root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"required").unwrap();
        }
        layout
    }

    #[test]
    fn layout_is_anchored_to_the_root_executable() {
        let root = PathBuf::from(r"C:\work\NEWrust");
        let layout = LaunchLayout::from_launcher_path(&root.join("AutoDesignMaker.exe")).unwrap();
        assert_eq!(layout.source_root, root);
        assert_eq!(
            layout.product_executable,
            root.join("dist/AutoDesignMaker-NEWrust/AutoDesignMaker.exe")
        );
        assert_eq!(
            layout.data_root,
            root.join("dist/AutoDesignMaker-NEWrust/user_data")
        );
    }

    #[test]
    fn complete_root_contract_validates_and_creates_data_root() {
        let layout = complete_layout("complete");
        assert!(layout.validate().is_ok());
        layout.prepare_data_root().unwrap();
        assert!(layout.data_root.is_dir());
        let _ = fs::remove_dir_all(layout.source_root);
    }

    #[test]
    fn missing_portable_manifest_fails_closed() {
        let layout = complete_layout("missing_manifest");
        fs::remove_file(layout.portable_root.join("build-manifest.json")).unwrap();
        let error = layout.validate().unwrap_err();
        assert!(error.contains("build-manifest.json"));
        let _ = fs::remove_dir_all(layout.source_root);
    }

    #[test]
    fn webview2_detection_enumerates_version_directories() {
        let root = temp_root("webview2");
        let version = root.join("137.0.3296.83");
        fs::create_dir_all(&version).unwrap();
        fs::write(version.join("msedgewebview2.exe"), b"runtime").unwrap();
        assert!(versioned_webview2_runtime_exists(&root));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn product_command_pins_rust_data_and_blank_defaults() {
        let layout = complete_layout("command_defaults");
        let command = product_command(&layout, &[], None, None);
        let environment = command
            .get_envs()
            .map(|(key, value)| (key.to_os_string(), value.map(OsStr::to_os_string)))
            .collect::<Vec<_>>();
        assert_eq!(command.get_program(), layout.product_executable);
        assert_eq!(
            command.get_current_dir(),
            Some(layout.portable_root.as_path())
        );
        assert!(environment.contains(&(
            OsString::from(DATA_DIR_ENV),
            Some(layout.data_root.as_os_str().to_os_string())
        )));
        assert!(environment.contains(&(
            OsString::from(STARTUP_PROJECT_ENV),
            Some(OsString::from(DEFAULT_STARTUP_PROJECT))
        )));
        assert!(environment.contains(&(
            OsString::from(LANGUAGE_ENV),
            Some(OsString::from(DEFAULT_LANGUAGE))
        )));
        let _ = fs::remove_dir_all(layout.source_root);
    }

    #[test]
    fn product_command_preserves_explicit_recovery_and_language() {
        let layout = complete_layout("command_explicit");
        let arguments = vec![OsString::from("--smoke")];
        let command = product_command(
            &layout,
            &arguments,
            Some(OsStr::new("restore")),
            Some(OsStr::new("en-US")),
        );
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![OsStr::new("--smoke")]
        );
        let environment = command
            .get_envs()
            .map(|(key, value)| (key.to_os_string(), value.map(OsStr::to_os_string)))
            .collect::<Vec<_>>();
        assert!(environment.contains(&(
            OsString::from(STARTUP_PROJECT_ENV),
            Some(OsString::from("restore"))
        )));
        assert!(
            environment.contains(&(OsString::from(LANGUAGE_ENV), Some(OsString::from("en-US"))))
        );
        let _ = fs::remove_dir_all(layout.source_root);
    }
}
