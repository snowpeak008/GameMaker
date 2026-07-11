#![forbid(unsafe_code)]

pub mod commands;
mod design_specs;
pub mod runtime;

pub const APP_NAME: &str = "AutoDesignMaker NEWrust";
pub const WEB_DIST_DIR: &str = "../../web/dist";
pub const TAURI_CONFIG: &str = "tauri.conf.json";
pub const WINDOW_TITLE: &str = "AutoDesignMaker NEWrust";

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopShellConfig {
    pub app_name: String,
    pub window: DesktopWindowConfig,
    pub startup: StartupPolicy,
    pub theme: Vec<ThemeToken>,
    pub web_dist_dir: String,
    pub tauri_config: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopWindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub min_width: u32,
    pub min_height: u32,
    pub resizable: bool,
    pub geometry_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupPolicy {
    pub pycache_prefix: String,
    pub validate_data_integrity: bool,
    pub auto_restore_current_save: bool,
    pub release_lock_at_exit: bool,
    pub prune_drafts_keep_count: u32,
    pub locked_archive_status: ShellStatus,
    pub startup_error_status: ShellStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeToken {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellStatus {
    pub label: String,
    pub color: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSmokeReport {
    pub status: String,
    pub blocker_count: usize,
    pub blockers: Vec<String>,
    pub theme_token_count: usize,
    pub web_dist_dir: String,
    pub tauri_config: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopReleaseSmokeReport {
    pub status: String,
    pub blockers: Vec<String>,
    pub shell_status: String,
    pub runtime_initialized: bool,
    pub runtime_shutdown_cleanly: bool,
    pub design_data_file_count: usize,
    pub support_files_present: bool,
    pub isolated_data_root: bool,
}

pub fn default_shell_config() -> DesktopShellConfig {
    DesktopShellConfig {
        app_name: APP_NAME.to_string(),
        window: DesktopWindowConfig {
            title: WINDOW_TITLE.to_string(),
            width: 1280,
            height: 820,
            min_width: 1180,
            min_height: 720,
            resizable: true,
            geometry_file: "settings/window_geometry.json".to_string(),
        },
        startup: StartupPolicy {
            pycache_prefix: ".cache/pycache".to_string(),
            validate_data_integrity: true,
            auto_restore_current_save: true,
            release_lock_at_exit: true,
            prune_drafts_keep_count: 0,
            locked_archive_status: ShellStatus {
                label: "系统: 上次存档已在另一个窗口打开；本窗口未自动加载。".to_string(),
                color: "#E8A23A".to_string(),
            },
            startup_error_status: ShellStatus {
                label: "系统: 启动检查异常".to_string(),
                color: "#FF6B6B".to_string(),
            },
        },
        theme: theme_tokens(),
        web_dist_dir: WEB_DIST_DIR.to_string(),
        tauri_config: TAURI_CONFIG.to_string(),
    }
}

pub fn shell_state() -> &'static str {
    "desktop-tauri shell config ready"
}

pub fn release_smoke_report_for(executable_dir: &std::path::Path) -> DesktopReleaseSmokeReport {
    let shell = desktop_smoke_report(&default_shell_config());
    let mut blockers = shell.blockers.clone();
    let design_root = executable_dir.join("knowledge").join("design_data");
    let design_data_file_count = count_files(&design_root).unwrap_or(0);
    if design_data_file_count == 0 {
        blockers.push("portable design data is missing or empty".to_string());
    }
    let registry_path = executable_dir.join("pipeline/artifact_layer/registry.json");
    match std::fs::read_to_string(&registry_path)
        .ok()
        .and_then(|text| {
            serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')).ok()
        }) {
        Some(registry)
            if registry
                .get("version")
                .and_then(serde_json::Value::as_u64)
                .is_some()
                && registry
                    .get("artifacts")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|artifacts| !artifacts.is_empty()) => {}
        Some(_) => blockers.push("portable pipeline artifact registry is invalid".to_string()),
        None if registry_path.is_file() => {
            blockers.push("portable pipeline artifact registry is invalid".to_string())
        }
        None => blockers.push("portable pipeline artifact registry is missing".to_string()),
    }
    let schema_root = executable_dir.join("knowledge/schemas");
    if count_files(&schema_root).unwrap_or(0) == 0 {
        blockers.push("portable schema directory is missing or empty".to_string());
    }
    let support_files_present = [
        "AutoDesignMaker.exe",
        "Start-AutoDesignMaker.cmd",
        "README.txt",
        "build-manifest.json",
    ]
    .iter()
    .all(|name| executable_dir.join(name).is_file());
    if !support_files_present {
        blockers.push("portable support files are incomplete".to_string());
    }

    let smoke_data_root = std::env::temp_dir().join(
        adm_new_foundation::new_stable_id("desktop-release-smoke")
            .unwrap_or_else(|_| format!("desktop-release-smoke-{}", std::process::id())),
    );
    let isolated_data_root = !smoke_data_root.starts_with(executable_dir);
    let mut runtime_initialized = false;
    let mut runtime_shutdown_cleanly = false;
    match runtime::AppRuntime::new(&smoke_data_root) {
        Ok(runtime) => {
            runtime_initialized = true;
            match runtime.shutdown_once() {
                Ok(()) => runtime_shutdown_cleanly = true,
                Err(_) => blockers.push("isolated runtime shutdown failed".to_string()),
            }
            drop(runtime);
        }
        Err(_) => blockers.push("isolated desktop runtime initialization failed".to_string()),
    }
    if !isolated_data_root {
        blockers.push("release smoke data root was not isolated".to_string());
    }
    let _ = std::fs::remove_dir_all(&smoke_data_root);

    DesktopReleaseSmokeReport {
        status: if blockers.is_empty() {
            "passed".to_string()
        } else {
            "blocked".to_string()
        },
        blockers,
        shell_status: shell.status,
        runtime_initialized,
        runtime_shutdown_cleanly,
        design_data_file_count,
        support_files_present,
        isolated_data_root,
    }
}

fn count_files(root: &std::path::Path) -> std::io::Result<usize> {
    if !root.is_dir() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            count += count_files(&entry.path())?;
        } else if entry.file_type()?.is_file() {
            count += 1;
        }
    }
    Ok(count)
}

pub fn run() {
    use tauri::{Emitter, Manager};

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_root = std::env::var_os("ADM_NEWRUST_DATA_DIR")
                .map(std::path::PathBuf::from)
                .map(Ok)
                .unwrap_or_else(|| app.path().app_data_dir())?;
            let runtime = runtime::AppRuntime::new(data_root)
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            app.manage(runtime);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_shell_state,
            commands::load_design_workbench,
            commands::set_project_name,
            commands::update_node,
            commands::export_design,
            commands::autosave_design,
            commands::list_templates,
            commands::select_template,
            commands::save_template,
            commands::delete_template,
            commands::update_gameplay_system,
            commands::reset_design,
            commands::list_saves,
            commands::create_save,
            commands::create_blank_save,
            commands::save_project,
            commands::load_save,
            commands::rename_save,
            commands::delete_save,
            commands::open_save_directory,
            commands::get_autosave_state,
            commands::load_ai_config,
            commands::save_ai_config,
            commands::validate_ai_config,
            commands::completion_adapter_spec,
            commands::list_ai_config_descriptors,
            commands::preview_ai_resolution,
            commands::probe_ai_cli,
            commands::probe_ai_api,
            commands::load_project_config,
            commands::save_project_config,
            commands::run_project_preflight,
            commands::relink_project_binding,
            commands::select_native_path,
            commands::inspect_project_environment,
            commands::discover_project_unity_editors,
            commands::validate_project_editor,
            commands::refine_style_prompts,
            commands::load_ai_interview,
            commands::submit_ai_turn,
            commands::force_ai_output,
            commands::mark_ai_inaccurate,
            commands::save_ai_archive,
            commands::load_pipeline_view,
            commands::run_pipeline_range,
            commands::resume_pipeline,
            commands::stop_pipeline,
            commands::confirm_style,
            commands::read_pipeline_artifact,
            commands::list_latest_logs,
            commands::read_log_entries,
            commands::export_log_jsonl,
            commands::clear_logs,
            commands::analyze_patch_request,
            commands::list_patches,
            commands::read_patch,
            commands::update_patch_status,
            commands::list_sdks,
            commands::add_sdk,
            commands::update_sdk_review_status,
            commands::get_approved_sdk_context,
            commands::extract_sdk_spec,
            commands::load_package_view,
            commands::package_current_project,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build AutoDesignMaker NEWrust desktop runtime");

    app.run(|app_handle, event| {
        let tauri::RunEvent::ExitRequested { api, .. } = event else {
            return;
        };
        let Some(runtime) = app_handle.try_state::<runtime::AppRuntime>() else {
            return;
        };
        match runtime.begin_exit() {
            runtime::ExitDisposition::WaitForPipeline => {
                api.prevent_exit();
                runtime.request_pipeline_stop();
                let app_handle = app_handle.clone();
                std::thread::spawn(move || {
                    loop {
                        let Some(runtime) = app_handle.try_state::<crate::runtime::AppRuntime>()
                        else {
                            return;
                        };
                        if !runtime.pipeline_is_running() {
                            match runtime.shutdown_once() {
                                Ok(()) => app_handle.exit(0),
                                Err(error) => {
                                    eprintln!("shutdown persistence failed: {error}");
                                    let _ =
                                        app_handle.emit("adm-shutdown-error", error.to_string());
                                }
                            }
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                });
            }
            runtime::ExitDisposition::ShutdownNow => {
                if let Err(error) = runtime.shutdown_once() {
                    api.prevent_exit();
                    eprintln!("shutdown persistence failed: {error}");
                    let _ = app_handle.emit("adm-shutdown-error", error.to_string());
                }
            }
            runtime::ExitDisposition::AlreadyExiting => {
                if runtime.pipeline_is_running() {
                    api.prevent_exit();
                    runtime.request_pipeline_stop();
                }
            }
        }
    });
}

pub fn theme_tokens() -> Vec<ThemeToken> {
    [
        ("bg", "#F3F6F8"),
        ("surface", "#FFFFFF"),
        ("surface_alt", "#F8FAFC"),
        ("border", "#D7E0E8"),
        ("border_strong", "#A8B7C5"),
        ("text", "#15202B"),
        ("muted", "#657486"),
        ("primary", "#2563EB"),
        ("primary_soft", "#EAF1FF"),
        ("success", "#0F8A5F"),
        ("success_soft", "#E7F7EF"),
        ("warning", "#B45309"),
        ("warning_soft", "#FFF4DE"),
        ("danger", "#B42318"),
        ("danger_soft", "#FDEBEA"),
        ("dark", "#17212B"),
        ("user_message_bg", "#EFF6FF"),
        ("user_message_border", "#2563EB"),
        ("ai_message_bg", "#ECFDF5"),
        ("ai_message_border", "#0F8A5F"),
        ("system_message_bg", "#FFF7ED"),
        ("system_message_border", "#B45309"),
    ]
    .into_iter()
    .map(|(name, value)| ThemeToken {
        name: name.to_string(),
        value: value.to_string(),
    })
    .collect()
}

pub fn center_window_position(
    screen_width: u32,
    screen_height: u32,
    width: u32,
    height: u32,
) -> WindowPosition {
    WindowPosition {
        x: ((screen_width as i32 - width as i32) / 2).max(0),
        y: ((screen_height as i32 - height as i32) / 2).max(0),
    }
}

pub fn desktop_smoke_report(config: &DesktopShellConfig) -> DesktopSmokeReport {
    let mut blockers = Vec::new();
    if config.app_name.trim().is_empty() {
        blockers.push("app_name_empty".to_string());
    }
    if config.window.width < config.window.min_width {
        blockers.push("window_width_below_min_width".to_string());
    }
    if config.window.height < config.window.min_height {
        blockers.push("window_height_below_min_height".to_string());
    }
    for required in [
        "bg", "surface", "primary", "success", "warning", "danger", "dark",
    ] {
        if !config.theme.iter().any(|token| token.name == required) {
            blockers.push(format!("theme_token_missing:{required}"));
        }
    }
    if config.web_dist_dir.trim().is_empty() {
        blockers.push("web_dist_dir_empty".to_string());
    }
    if config.tauri_config.trim().is_empty() {
        blockers.push("tauri_config_empty".to_string());
    }
    DesktopSmokeReport {
        status: if blockers.is_empty() {
            "passed".to_string()
        } else {
            "failed".to_string()
        },
        blocker_count: blockers.len(),
        blockers,
        theme_token_count: config.theme.len(),
        web_dist_dir: config.web_dist_dir.clone(),
        tauri_config: config.tauri_config.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_tauri_scaffold_exports_expected_paths() {
        assert_eq!(APP_NAME, "AutoDesignMaker NEWrust");
        assert_eq!(WEB_DIST_DIR, "../../web/dist");
        assert_eq!(TAURI_CONFIG, "tauri.conf.json");
        assert_eq!(shell_state(), "desktop-tauri shell config ready");
    }

    #[test]
    fn desktop_shell_config_matches_python_theme_and_window_policy() {
        let config = default_shell_config();
        assert_eq!(config.window.title, "AutoDesignMaker NEWrust");
        assert_eq!(config.window.min_width, 1180);
        assert_eq!(config.window.min_height, 720);
        assert!(config.startup.validate_data_integrity);
        assert!(config.startup.auto_restore_current_save);
        assert!(config.startup.release_lock_at_exit);
        assert_eq!(config.startup.prune_drafts_keep_count, 0);
        assert!(
            config
                .theme
                .iter()
                .any(|token| token.name == "primary" && token.value == "#2563EB")
        );
        assert!(
            serde_json::to_value(&config)
                .unwrap()
                .get("webDistDir")
                .is_some()
        );
    }

    #[test]
    fn desktop_smoke_report_blocks_invalid_shell_config() {
        let mut config = default_shell_config();
        let report = desktop_smoke_report(&config);
        assert_eq!(report.status, "passed");
        assert_eq!(report.theme_token_count, 22);

        config.window.width = 1000;
        config.theme.retain(|token| token.name != "primary");
        let blocked = desktop_smoke_report(&config);
        assert_eq!(blocked.status, "failed");
        assert!(
            blocked
                .blockers
                .contains(&"window_width_below_min_width".to_string())
        );
        assert!(
            blocked
                .blockers
                .contains(&"theme_token_missing:primary".to_string())
        );
    }

    #[test]
    fn center_window_position_matches_python_centering_math() {
        assert_eq!(
            center_window_position(1920, 1080, 1280, 820),
            WindowPosition { x: 320, y: 130 }
        );
        assert_eq!(
            center_window_position(1000, 700, 1280, 820),
            WindowPosition { x: 0, y: 0 }
        );
    }

    #[test]
    fn release_smoke_uses_an_isolated_runtime_and_checks_bundle_support_files() {
        let bundle = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("desktop-release-bundle").unwrap());
        std::fs::create_dir_all(bundle.join("knowledge").join("design_data")).unwrap();
        std::fs::write(bundle.join("knowledge/design_data/test.json"), b"{}").unwrap();
        std::fs::create_dir_all(bundle.join("pipeline/artifact_layer")).unwrap();
        std::fs::write(
            bundle.join("pipeline/artifact_layer/registry.json"),
            br#"{"version":1,"artifacts":[{"id":"stage_00.test","stage":0,"kind":"test"}]}"#,
        )
        .unwrap();
        std::fs::create_dir_all(bundle.join("knowledge/schemas")).unwrap();
        std::fs::write(
            bundle.join("knowledge/schemas/test.schema.json"),
            br#"{"type":"object"}"#,
        )
        .unwrap();
        for name in [
            "AutoDesignMaker.exe",
            "Start-AutoDesignMaker.cmd",
            "README.txt",
            "build-manifest.json",
        ] {
            std::fs::write(bundle.join(name), b"fixture").unwrap();
        }

        let report = release_smoke_report_for(&bundle);

        assert_eq!(report.status, "passed", "{:?}", report.blockers);
        assert!(report.runtime_initialized);
        assert!(report.runtime_shutdown_cleanly);
        assert!(report.isolated_data_root);
        assert!(report.support_files_present);
        assert_eq!(report.design_data_file_count, 1);
        let _ = std::fs::remove_dir_all(bundle);
    }

    #[test]
    fn release_smoke_blocks_missing_bundle_protocol_resources() {
        let bundle = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("desktop-release-protocol").unwrap());
        std::fs::create_dir_all(bundle.join("knowledge/design_data")).unwrap();
        std::fs::write(bundle.join("knowledge/design_data/test.json"), b"{}").unwrap();
        for name in [
            "AutoDesignMaker.exe",
            "Start-AutoDesignMaker.cmd",
            "README.txt",
            "build-manifest.json",
        ] {
            std::fs::write(bundle.join(name), b"fixture").unwrap();
        }

        let missing_all = release_smoke_report_for(&bundle);
        assert_eq!(missing_all.status, "blocked");
        assert!(
            missing_all
                .blockers
                .contains(&"portable pipeline artifact registry is missing".to_string())
        );
        assert!(
            missing_all
                .blockers
                .contains(&"portable schema directory is missing or empty".to_string())
        );

        std::fs::create_dir_all(bundle.join("pipeline/artifact_layer")).unwrap();
        std::fs::write(
            bundle.join("pipeline/artifact_layer/registry.json"),
            br#"{"version":1,"artifacts":[{"id":"stage_00.test","stage":0,"kind":"test"}]}"#,
        )
        .unwrap();
        let missing_schema = release_smoke_report_for(&bundle);
        assert_eq!(missing_schema.status, "blocked");
        assert!(
            !missing_schema
                .blockers
                .iter()
                .any(|blocker| blocker.contains("artifact registry"))
        );
        assert!(
            missing_schema
                .blockers
                .contains(&"portable schema directory is missing or empty".to_string())
        );
        let _ = std::fs::remove_dir_all(bundle);
    }
}
