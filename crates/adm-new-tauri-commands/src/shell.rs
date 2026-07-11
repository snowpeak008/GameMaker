use adm_new_foundation::AdmResult;
use serde::{Deserialize, Serialize};

use crate::{CommandAdapterResult, handle_command};
pub use adm_new_contracts::ArtifactLocale as UiLanguage;

pub const UI_LANGUAGE_ENV: &str = "ADM_NEWRUST_LANGUAGE";

pub fn normalize_ui_language(value: Option<&str>) -> UiLanguage {
    UiLanguage::normalize(value)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellState {
    pub active_view: String,
    #[serde(default)]
    pub ui_language: UiLanguage,
    pub ai_status: ShellAiStatus,
    pub progress: ShellProgress,
    pub system_status: String,
    pub theme: Vec<ShellThemeToken>,
    pub window: ShellWindowState,
    pub startup: ShellStartupState,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            active_view: "design".to_string(),
            ui_language: UiLanguage::default(),
            ai_status: ShellAiStatus::default(),
            progress: ShellProgress::default(),
            system_status: "系统: 就绪".to_string(),
            theme: default_theme_tokens(),
            window: ShellWindowState::default(),
            startup: ShellStartupState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellAiStatus {
    pub label: String,
    pub ok: bool,
}

impl Default for ShellAiStatus {
    fn default() -> Self {
        Self {
            label: "AI: 未配置".to_string(),
            ok: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellProgress {
    pub passed: u32,
    pub total: u32,
}

impl Default for ShellProgress {
    fn default() -> Self {
        Self {
            passed: 0,
            total: 15,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellThemeToken {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellWindowState {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub min_width: u32,
    pub min_height: u32,
    pub geometry_file: String,
    pub resizable: bool,
}

impl Default for ShellWindowState {
    fn default() -> Self {
        Self {
            title: "AutoDesignMaker NEWrust".to_string(),
            width: 1280,
            height: 820,
            min_width: 1180,
            min_height: 720,
            geometry_file: "settings/window_geometry.json".to_string(),
            resizable: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellStartupState {
    pub validate_data_integrity: bool,
    pub auto_restore_current_save: bool,
    pub release_lock_at_exit: bool,
    pub prune_drafts_keep_count: u32,
    pub startup_status: String,
    pub startup_status_color: String,
}

impl Default for ShellStartupState {
    fn default() -> Self {
        Self {
            validate_data_integrity: true,
            auto_restore_current_save: true,
            release_lock_at_exit: true,
            prune_drafts_keep_count: 0,
            startup_status: "系统: 就绪".to_string(),
            startup_status_color: "#C8D3DF".to_string(),
        }
    }
}

pub fn default_theme_tokens() -> Vec<ShellThemeToken> {
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
    .map(|(name, value)| ShellThemeToken {
        name: name.to_string(),
        value: value.to_string(),
    })
    .collect()
}

pub trait ShellCommandService {
    fn get_shell_state(&self) -> AdmResult<ShellState>;
}

#[derive(Debug, Clone, Default)]
pub struct DefaultShellCommandService;

impl ShellCommandService for DefaultShellCommandService {
    fn get_shell_state(&self) -> AdmResult<ShellState> {
        Ok(ShellState::default())
    }
}

pub fn get_shell_state<S>(service: &S) -> CommandAdapterResult<ShellState>
where
    S: ShellCommandService,
{
    handle_command(|| service.get_shell_state())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use adm_new_foundation::{AdmError, AdmResult};

    #[test]
    fn shell_state_serializes_with_frontend_field_names() {
        let response = get_shell_state(&DefaultShellCommandService);
        assert!(response.ok);
        let value = serde_json::to_value(response.data.unwrap()).unwrap();
        assert_eq!(value["activeView"], "design");
        assert_eq!(value["uiLanguage"], "zh-CN");
        assert_eq!(value["aiStatus"]["label"], "AI: 未配置");
        assert_eq!(value["progress"]["total"], 15);
        assert_eq!(value["systemStatus"], "系统: 就绪");
        assert_eq!(value["window"]["minWidth"], 1180);
        assert_eq!(value["startup"]["pruneDraftsKeepCount"], 0);
        assert!(
            value["theme"]
                .as_array()
                .unwrap()
                .iter()
                .any(|token| token["name"] == "primary" && token["value"] == "#2563EB")
        );
    }

    #[test]
    fn ui_language_normalization_is_pure_and_returns_canonical_values() {
        assert_eq!(normalize_ui_language(None), UiLanguage::ZhCn);
        assert_eq!(normalize_ui_language(Some("")), UiLanguage::ZhCn);
        assert_eq!(normalize_ui_language(Some("unknown")), UiLanguage::ZhCn);
        assert_eq!(normalize_ui_language(Some(" zh-cn ")), UiLanguage::ZhCn);
        assert_eq!(normalize_ui_language(Some("en-US")), UiLanguage::EnUs);
        assert_eq!(normalize_ui_language(Some(" EN-us ")), UiLanguage::EnUs);
        assert_eq!(UiLanguage::ZhCn.as_str(), "zh-CN");
        assert_eq!(UiLanguage::EnUs.as_str(), "en-US");
    }

    #[test]
    fn shell_command_wrapper_calls_service_trait_mock() {
        let service = MockShellService {
            calls: Cell::new(0),
        };
        let response = get_shell_state(&service);
        assert!(response.ok);
        assert_eq!(service.calls.get(), 1);
    }

    struct MockShellService {
        calls: Cell<usize>,
    }

    impl ShellCommandService for MockShellService {
        fn get_shell_state(&self) -> AdmResult<ShellState> {
            self.calls.set(self.calls.get() + 1);
            Ok(ShellState::default())
        }
    }

    #[test]
    fn shell_command_errors_are_mapped() {
        struct FailingShellService;
        impl ShellCommandService for FailingShellService {
            fn get_shell_state(&self) -> AdmResult<ShellState> {
                Err(AdmError::new("shell state unavailable"))
            }
        }
        let response = get_shell_state(&FailingShellService);
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "COMMAND_FAILED");
    }
}
