use std::fmt;

use adm_new_application::{
    AiApiProbeView, AiCliProbeView, AiConfigApplicationService, AiConfigDescriptorView,
    AiConfigValidationReport, AiResolutionView, CompletionAdapterSpec,
    project_environment::{
        EditorSelectionValidation, ProjectEnvironmentInspection, UnityEditorCandidate,
        discover_unity_editors, inspect_project_directory, validate_editor_selection,
    },
    runtime::{DevelopmentPreflightReport, ProjectRuntimeSettings, RuntimeApplicationService},
};
use adm_new_contracts::ai::AiConfig;
use adm_new_foundation::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};

use crate::{CommandAdapterResult, handle_command};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveAiConfigRequest {
    pub config: AiConfig,
}

impl fmt::Debug for SaveAiConfigRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SaveAiConfigRequest")
            .field("schema_version", &self.config.schema_version)
            .field("dev_entry_count", &self.config.dev.entries.len())
            .field("image_entry_count", &self.config.image.entries.len())
            .field(
                "completion_entry_count",
                &self.config.completion.entries.len(),
            )
            .field("profile_count", &self.config.profiles.len())
            .finish()
    }
}

/// Draft configuration to resolve locally for one category.
///
/// This request may contain secret material, so it deliberately has no `Debug`
/// implementation. Command responses use only the redacted resolution/probe
/// views defined by `adm-new-ai`.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AiConfigResolutionRequest {
    pub config: AiConfig,
    #[serde(alias = "categoryId")]
    pub category_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveProjectConfigRequest {
    pub settings: ProjectRuntimeSettings,
    #[serde(default = "default_run_preflight")]
    pub run_preflight: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectPreflightRequest {
    #[serde(default)]
    pub settings: Option<ProjectRuntimeSettings>,
    #[serde(default = "default_write_report")]
    pub write_report: bool,
    #[serde(default)]
    pub prefer_run_context: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelinkProjectBindingRequest {
    pub settings: ProjectRuntimeSettings,
    pub project_path: String,
    #[serde(default)]
    pub editor_path: String,
    #[serde(default = "default_run_preflight")]
    pub run_preflight: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfigView {
    pub settings: ProjectRuntimeSettings,
    pub saved_path: String,
    #[serde(default)]
    pub preflight: Option<DevelopmentPreflightReport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativePathSelectionKind {
    Folder,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativePathFilter {
    pub name: String,
    #[serde(default)]
    pub extensions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativePathSelectionRequest {
    pub kind: NativePathSelectionKind,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub current_path: String,
    #[serde(default)]
    pub filters: Vec<NativePathFilter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativePathSelectionView {
    pub status: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectProjectEnvironmentRequest {
    pub project_path: String,
    #[serde(default)]
    pub expected_engine: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoverUnityEditorsRequest {
    pub project_path: String,
    #[serde(default)]
    pub configured_editor_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidateProjectEditorRequest {
    pub project_engine: String,
    pub project_path: String,
    pub editor_path: String,
}

impl NativePathSelectionView {
    pub fn selected(path: impl Into<String>) -> Self {
        Self {
            status: "selected".to_string(),
            path: Some(path.into()),
            message: String::new(),
        }
    }

    pub fn cancelled() -> Self {
        Self {
            status: "cancelled".to_string(),
            path: None,
            message: String::new(),
        }
    }
}

fn default_run_preflight() -> bool {
    true
}

fn default_write_report() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiConfigValidationView {
    pub ok: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl From<AiConfigValidationReport> for AiConfigValidationView {
    fn from(value: AiConfigValidationReport) -> Self {
        Self {
            ok: value.ok,
            errors: value.errors,
            warnings: value.warnings,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionAdapterSpecView {
    pub entry_id: String,
    pub config_type: String,
    pub adapter_kind: String,
    pub api_url: String,
    pub has_api_key: bool,
}

impl fmt::Debug for CompletionAdapterSpecView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompletionAdapterSpecView")
            .field("entry_id", &self.entry_id)
            .field("config_type", &self.config_type)
            .field("adapter_kind", &self.adapter_kind)
            .field("api_url_configured", &!self.api_url.trim().is_empty())
            .field("has_api_key", &self.has_api_key)
            .finish()
    }
}

impl From<CompletionAdapterSpec> for CompletionAdapterSpecView {
    fn from(value: CompletionAdapterSpec) -> Self {
        Self {
            entry_id: value.entry_id,
            config_type: value.config_type,
            adapter_kind: value.adapter_kind,
            api_url: value.api_url,
            has_api_key: value.has_api_key,
        }
    }
}

pub trait AiConfigCommandService {
    fn load_ai_config(&self) -> AdmResult<AiConfig>;
    fn save_ai_config(&self, request: &SaveAiConfigRequest) -> AdmResult<AiConfigValidationView>;
    fn validate_ai_config(&self, config: &AiConfig) -> AiConfigValidationView;
    fn completion_adapter_spec(&self, config: &AiConfig) -> AdmResult<CompletionAdapterSpecView>;
    fn list_ai_config_descriptors(&self) -> AdmResult<Vec<AiConfigDescriptorView>> {
        Err(AdmError::new(
            "AI descriptor catalog requires AiConfigApplicationService",
        ))
    }
    fn preview_ai_resolution(
        &self,
        _request: &AiConfigResolutionRequest,
    ) -> AdmResult<AiResolutionView> {
        Err(AdmError::new(
            "AI resolution preview requires AiConfigApplicationService",
        ))
    }
    fn probe_ai_cli(&self, _request: &AiConfigResolutionRequest) -> AdmResult<AiCliProbeView> {
        Err(AdmError::new(
            "AI CLI probe requires AiConfigApplicationService",
        ))
    }
    fn probe_ai_api(&self, _request: &AiConfigResolutionRequest) -> AdmResult<AiApiProbeView> {
        Err(AdmError::new(
            "AI API probe requires AiConfigApplicationService",
        ))
    }
    fn load_project_config(&self) -> AdmResult<ProjectRuntimeSettings>;
    fn save_project_config(
        &self,
        request: &SaveProjectConfigRequest,
    ) -> AdmResult<ProjectConfigView>;
    fn run_project_preflight(
        &self,
        request: &ProjectPreflightRequest,
    ) -> AdmResult<DevelopmentPreflightReport>;
    fn relink_project_binding(
        &self,
        _request: &RelinkProjectBindingRequest,
    ) -> AdmResult<ProjectConfigView> {
        Err(AdmError::new(
            "project relink requires RuntimeApplicationService",
        ))
    }
}

impl AiConfigCommandService for AiConfigApplicationService {
    fn load_ai_config(&self) -> AdmResult<AiConfig> {
        self.load_redacted()
    }

    fn save_ai_config(&self, request: &SaveAiConfigRequest) -> AdmResult<AiConfigValidationView> {
        self.save_redacted(&request.config)
            .map(AiConfigValidationView::from)
    }

    fn validate_ai_config(&self, config: &AiConfig) -> AiConfigValidationView {
        self.validate(config).into()
    }

    fn completion_adapter_spec(&self, config: &AiConfig) -> AdmResult<CompletionAdapterSpecView> {
        self.completion_adapter_spec(config)
            .map(CompletionAdapterSpecView::from)
    }

    fn list_ai_config_descriptors(&self) -> AdmResult<Vec<AiConfigDescriptorView>> {
        Ok(self.list_ai_config_descriptors())
    }

    fn preview_ai_resolution(
        &self,
        request: &AiConfigResolutionRequest,
    ) -> AdmResult<AiResolutionView> {
        self.preview_ai_resolution(&request.config, &request.category_id)
    }

    fn probe_ai_cli(&self, request: &AiConfigResolutionRequest) -> AdmResult<AiCliProbeView> {
        self.probe_ai_cli(&request.config, &request.category_id)
    }

    fn probe_ai_api(&self, request: &AiConfigResolutionRequest) -> AdmResult<AiApiProbeView> {
        self.probe_ai_api(&request.config, &request.category_id)
    }

    fn load_project_config(&self) -> AdmResult<ProjectRuntimeSettings> {
        Err(adm_new_foundation::AdmError::new(
            "project config requires RuntimeApplicationService",
        ))
    }

    fn save_project_config(&self, _: &SaveProjectConfigRequest) -> AdmResult<ProjectConfigView> {
        Err(adm_new_foundation::AdmError::new(
            "project config requires RuntimeApplicationService",
        ))
    }

    fn run_project_preflight(
        &self,
        _: &ProjectPreflightRequest,
    ) -> AdmResult<DevelopmentPreflightReport> {
        Err(adm_new_foundation::AdmError::new(
            "project preflight requires RuntimeApplicationService",
        ))
    }

    fn relink_project_binding(
        &self,
        _: &RelinkProjectBindingRequest,
    ) -> AdmResult<ProjectConfigView> {
        Err(adm_new_foundation::AdmError::new(
            "project relink requires RuntimeApplicationService",
        ))
    }
}

impl AiConfigCommandService for RuntimeApplicationService {
    fn load_ai_config(&self) -> AdmResult<AiConfig> {
        Err(adm_new_foundation::AdmError::new(
            "AI config requires AiConfigApplicationService",
        ))
    }

    fn save_ai_config(&self, _: &SaveAiConfigRequest) -> AdmResult<AiConfigValidationView> {
        Err(adm_new_foundation::AdmError::new(
            "AI config requires AiConfigApplicationService",
        ))
    }

    fn validate_ai_config(&self, _: &AiConfig) -> AiConfigValidationView {
        AiConfigValidationView {
            ok: false,
            errors: vec!["AI config requires AiConfigApplicationService".to_string()],
            warnings: Vec::new(),
        }
    }

    fn completion_adapter_spec(&self, _: &AiConfig) -> AdmResult<CompletionAdapterSpecView> {
        Err(adm_new_foundation::AdmError::new(
            "AI config requires AiConfigApplicationService",
        ))
    }

    fn load_project_config(&self) -> AdmResult<ProjectRuntimeSettings> {
        Ok(self.load_project_settings(false))
    }

    fn save_project_config(
        &self,
        request: &SaveProjectConfigRequest,
    ) -> AdmResult<ProjectConfigView> {
        self.save_project_settings(&request.settings)?;
        let preflight = if request.run_preflight {
            Some(self.run_actual_development_preflight(true, false)?)
        } else {
            None
        };
        Ok(ProjectConfigView {
            settings: self.load_project_settings(false),
            // The machine-local settings location is intentionally not part of
            // the Web/IPC view.
            saved_path: String::new(),
            preflight,
        })
    }

    fn run_project_preflight(
        &self,
        request: &ProjectPreflightRequest,
    ) -> AdmResult<DevelopmentPreflightReport> {
        if let Some(settings) = &request.settings {
            self.save_project_settings(settings)?;
        }
        self.run_actual_development_preflight(request.write_report, request.prefer_run_context)
    }

    fn relink_project_binding(
        &self,
        request: &RelinkProjectBindingRequest,
    ) -> AdmResult<ProjectConfigView> {
        let settings = RuntimeApplicationService::relink_project_binding(
            self,
            &request.settings,
            &request.project_path,
            &request.editor_path,
        )?;
        let preflight = if request.run_preflight {
            Some(self.run_actual_development_preflight(true, false)?)
        } else {
            None
        };
        Ok(ProjectConfigView {
            settings,
            saved_path: String::new(),
            preflight,
        })
    }
}

pub fn load_ai_config<S>(service: &S) -> CommandAdapterResult<AiConfig>
where
    S: AiConfigCommandService,
{
    handle_command(|| service.load_ai_config())
}

pub fn save_ai_config<S>(
    service: &S,
    request: SaveAiConfigRequest,
) -> CommandAdapterResult<AiConfigValidationView>
where
    S: AiConfigCommandService,
{
    handle_command(|| service.save_ai_config(&request))
}

pub fn validate_ai_config<S>(
    service: &S,
    config: AiConfig,
) -> CommandAdapterResult<AiConfigValidationView>
where
    S: AiConfigCommandService,
{
    handle_command(|| Ok(service.validate_ai_config(&config)))
}

pub fn completion_adapter_spec<S>(
    service: &S,
    config: AiConfig,
) -> CommandAdapterResult<CompletionAdapterSpecView>
where
    S: AiConfigCommandService,
{
    handle_command(|| service.completion_adapter_spec(&config))
}

pub fn list_ai_config_descriptors<S>(
    service: &S,
) -> CommandAdapterResult<Vec<AiConfigDescriptorView>>
where
    S: AiConfigCommandService,
{
    handle_command(|| service.list_ai_config_descriptors())
}

pub fn preview_ai_resolution<S>(
    service: &S,
    request: AiConfigResolutionRequest,
) -> CommandAdapterResult<AiResolutionView>
where
    S: AiConfigCommandService,
{
    handle_command(|| service.preview_ai_resolution(&request))
}

pub fn probe_ai_cli<S>(
    service: &S,
    request: AiConfigResolutionRequest,
) -> CommandAdapterResult<AiCliProbeView>
where
    S: AiConfigCommandService,
{
    handle_command(|| service.probe_ai_cli(&request))
}

pub fn probe_ai_api<S>(
    service: &S,
    request: AiConfigResolutionRequest,
) -> CommandAdapterResult<AiApiProbeView>
where
    S: AiConfigCommandService,
{
    handle_command(|| service.probe_ai_api(&request))
}

pub fn load_project_config<S>(service: &S) -> CommandAdapterResult<ProjectRuntimeSettings>
where
    S: AiConfigCommandService,
{
    handle_command(|| service.load_project_config())
}

pub fn save_project_config<S>(
    service: &S,
    request: SaveProjectConfigRequest,
) -> CommandAdapterResult<ProjectConfigView>
where
    S: AiConfigCommandService,
{
    handle_command(|| service.save_project_config(&request))
}

pub fn run_project_preflight<S>(
    service: &S,
    request: ProjectPreflightRequest,
) -> CommandAdapterResult<DevelopmentPreflightReport>
where
    S: AiConfigCommandService,
{
    handle_command(|| service.run_project_preflight(&request))
}

pub fn relink_project_binding<S>(
    service: &S,
    request: RelinkProjectBindingRequest,
) -> CommandAdapterResult<ProjectConfigView>
where
    S: AiConfigCommandService,
{
    handle_command(|| service.relink_project_binding(&request))
}

pub fn inspect_project_environment(
    request: InspectProjectEnvironmentRequest,
) -> CommandAdapterResult<ProjectEnvironmentInspection> {
    handle_command(|| {
        Ok(inspect_project_directory(
            request.project_path,
            &request.expected_engine,
        ))
    })
}

pub fn discover_project_unity_editors(
    request: DiscoverUnityEditorsRequest,
) -> CommandAdapterResult<Vec<UnityEditorCandidate>> {
    handle_command(|| {
        Ok(discover_unity_editors(
            request.project_path,
            Some(&request.configured_editor_path),
        ))
    })
}

pub fn validate_project_editor(
    request: ValidateProjectEditorRequest,
) -> CommandAdapterResult<EditorSelectionValidation> {
    handle_command(|| {
        Ok(validate_editor_selection(
            &request.project_engine,
            request.project_path,
            request.editor_path,
        ))
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use adm_new_contracts::ai::{ApiCategory, ApiEntry};
    use adm_new_foundation::{AdmError, AdmResult, new_stable_id};

    #[test]
    fn config_commands_load_save_and_validate_without_exposing_keys() {
        let root = temp_root("config");
        let service = AiConfigApplicationService::new(&root).unwrap();
        let config = valid_config();
        let request_debug = format!(
            "{:?}",
            SaveAiConfigRequest {
                config: config.clone(),
            }
        );
        assert!(!request_debug.contains("secret"));
        assert!(!request_debug.contains("api.example.test"));
        let saved = save_ai_config(
            &service,
            SaveAiConfigRequest {
                config: config.clone(),
            },
        );
        assert!(saved.ok);
        assert!(saved.data.unwrap().ok);

        let loaded = load_ai_config(&service);
        assert!(loaded.ok);
        assert_eq!(
            loaded.data.unwrap().completion.active_entry_id,
            "completion"
        );

        let spec = completion_adapter_spec(&service, config);
        assert!(spec.ok);
        let spec = spec.data.unwrap();
        assert!(spec.has_api_key);
        assert!(!format!("{spec:?}").contains("api.example.test"));
        let json = serde_json::to_string(&spec).unwrap();
        assert!(!json.contains("secret"));
        cleanup(root);
    }

    #[test]
    fn descriptor_and_resolution_commands_return_only_safe_local_views() {
        let root = temp_root("resolution");
        let service = AiConfigApplicationService::new(&root).unwrap();
        let descriptors = list_ai_config_descriptors(&service);
        assert!(descriptors.ok);
        let descriptors = descriptors.data.unwrap();
        assert_eq!(descriptors.len(), 12);
        assert!(
            descriptors
                .iter()
                .any(|descriptor| descriptor.config_type == "local_codex_cli")
        );

        let secret = "sk-resolution-private";
        let mut config = valid_config();
        let completion = &mut config.completion.entries[0];
        completion.api_key = secret.to_string();
        completion.api_url =
            "https://user:password@api.example.test/private/path?token=hidden".to_string();
        let request = AiConfigResolutionRequest {
            config,
            category_id: "completion".to_string(),
        };
        let preview = preview_ai_resolution(&service, request.clone());
        assert!(preview.ok);
        let preview = preview.data.unwrap();
        assert_eq!(preview.entry_id, "completion");
        assert_eq!(preview.config_type, "openai_completion_api");
        assert!(preview.available);
        assert!(preview.has_secret);
        assert_eq!(
            preview.masked_url.as_deref(),
            Some("https://api.example.test/…")
        );
        let serialized = serde_json::to_string(&preview).unwrap();
        assert!(!serialized.contains(secret));
        assert!(!serialized.contains("password"));
        assert!(!serialized.contains("private/path"));
        assert!(!serialized.contains("hidden"));

        let probe = probe_ai_cli(&service, request);
        assert!(!probe.ok);
        let error = probe.error.unwrap();
        assert!(!error.message.contains(secret));
        assert!(!error.message.contains("https://"));
        cleanup(root);
    }

    #[test]
    fn config_validation_errors_are_serialized_as_data() {
        let root = temp_root("invalid");
        let service = AiConfigApplicationService::new(&root).unwrap();
        let invalid = AiConfig {
            dev: local_codex_category(),
            completion: ApiCategory {
                category_id: "completion".to_string(),
                entries: vec![ApiEntry {
                    id: "completion".to_string(),
                    label: "Completion".to_string(),
                    config_type: "openai_completion_api".to_string(),
                    api_url: "https://api.example.test/v1".to_string(),
                    extra_json: serde_json::json!({"model": "gpt-5.5"}),
                    ..ApiEntry::default()
                }],
                active_entry_id: "completion".to_string(),
            },
            ..AiConfig::default()
        };
        let response = validate_ai_config(&service, invalid);
        assert!(response.ok);
        let report = response.data.unwrap();
        assert!(!report.ok);
        assert!(
            report
                .errors
                .iter()
                .any(|item| item.contains("missing api_key"))
        );
        cleanup(root);
    }

    #[test]
    fn config_completion_spec_errors_are_mapped() {
        let root = temp_root("missing_active");
        let service = AiConfigApplicationService::new(&root).unwrap();
        let response = completion_adapter_spec(&service, AiConfig::default());
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "CONFIGURATION_REQUIRED");
        cleanup(root);
    }

    #[test]
    fn config_command_wrapper_calls_service_trait_mock() {
        let service = MockConfigService {
            load_calls: Cell::new(0),
        };
        let response = load_ai_config(&service);
        assert!(response.ok);
        assert_eq!(service.load_calls.get(), 1);
    }

    #[test]
    fn project_config_commands_save_and_run_preflight() {
        let root = temp_root("project_config");
        let service = RuntimeApplicationService::new(&root, "session_a").unwrap();
        let response = save_project_config(
            &service,
            SaveProjectConfigRequest {
                settings: ProjectRuntimeSettings {
                    project_engine: "custom".to_string(),
                    custom_engine_name: "Internal Engine".to_string(),
                    development_path: "missing_custom_project".to_string(),
                    ..ProjectRuntimeSettings::default()
                },
                run_preflight: true,
            },
        );
        assert!(response.ok);
        let view = response.data.unwrap();
        let logical_settings = view.settings.clone();
        assert!(view.saved_path.is_empty());
        assert_eq!(view.settings.project_engine, "custom");
        assert_eq!(view.preflight.unwrap().status, "passed");

        let moved = root.join("custom-project-moved");
        fs::create_dir_all(&moved).unwrap();
        let relinked = relink_project_binding(
            &service,
            RelinkProjectBindingRequest {
                settings: logical_settings,
                project_path: moved.display().to_string(),
                editor_path: String::new(),
                run_preflight: true,
            },
        );
        assert!(relinked.ok);
        let relinked = relinked.data.unwrap();
        assert!(relinked.saved_path.is_empty());
        assert_eq!(
            relinked.settings.development_path,
            moved.display().to_string()
        );

        let loaded = load_project_config(&service);
        assert!(loaded.ok);
        assert_eq!(loaded.data.unwrap().custom_engine_name, "Internal Engine");

        let blocked = run_project_preflight(
            &service,
            ProjectPreflightRequest {
                settings: Some(ProjectRuntimeSettings {
                    project_engine: "unity".to_string(),
                    development_path: String::new(),
                    ..ProjectRuntimeSettings::default()
                }),
                write_report: false,
                prefer_run_context: false,
            },
        );
        assert!(blocked.ok);
        let blocked = blocked.data.unwrap();
        assert_eq!(blocked.status, "blocked");
        assert!(blocked.diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == "blocker"
                && diagnostic.error_code == "missing_development_path"
                && diagnostic.field == "development_path"
                && diagnostic.fix_action == "reselect_project_path"
        }));
        cleanup(root);
    }

    #[test]
    fn native_path_selection_contract_distinguishes_selected_and_cancelled() {
        let request: NativePathSelectionRequest = serde_json::from_value(serde_json::json!({
            "kind": "file",
            "title": "Choose Unity Editor",
            "current_path": "C:/Unity/Editor/Unity.exe",
            "filters": [{"name": "Executable", "extensions": ["exe"]}]
        }))
        .unwrap();
        assert_eq!(request.kind, NativePathSelectionKind::File);
        assert_eq!(request.filters[0].extensions, vec!["exe"]);

        let selected = NativePathSelectionView::selected("C:/Unity/Editor/Unity.exe");
        assert_eq!(selected.status, "selected");
        assert!(selected.path.is_some());
        let cancelled = NativePathSelectionView::cancelled();
        assert_eq!(cancelled.status, "cancelled");
        assert!(cancelled.path.is_none());
    }

    #[test]
    fn project_environment_commands_return_structured_views() {
        let root = temp_root("project_inspection");
        fs::create_dir_all(root.join("Assets")).unwrap();
        fs::create_dir_all(root.join("ProjectSettings")).unwrap();
        let response = inspect_project_environment(InspectProjectEnvironmentRequest {
            project_path: root.display().to_string(),
            expected_engine: "unity".to_string(),
        });
        assert!(response.ok);
        let inspection = response.data.unwrap();
        assert_eq!(inspection.detected_engine, "unity");
        assert!(
            inspection
                .diagnostics
                .iter()
                .any(|item| item.code == "unity_project_partial")
        );

        let candidates = discover_project_unity_editors(DiscoverUnityEditorsRequest {
            project_path: root.display().to_string(),
            configured_editor_path: String::new(),
        });
        assert!(candidates.ok);

        let invalid_editor = root.join("notepad.exe");
        fs::write(&invalid_editor, "fixture").unwrap();
        let validation = validate_project_editor(ValidateProjectEditorRequest {
            project_engine: "unity".to_string(),
            project_path: root.display().to_string(),
            editor_path: invalid_editor.display().to_string(),
        });
        assert!(validation.ok);
        let validation = validation.data.unwrap();
        assert!(!validation.valid);
        assert_eq!(validation.error_code, "invalid_unity_editor_executable");
        cleanup(root);
    }

    struct MockConfigService {
        load_calls: Cell<usize>,
    }

    impl AiConfigCommandService for MockConfigService {
        fn load_ai_config(&self) -> AdmResult<AiConfig> {
            self.load_calls.set(self.load_calls.get() + 1);
            Ok(AiConfig::default())
        }

        fn save_ai_config(&self, _: &SaveAiConfigRequest) -> AdmResult<AiConfigValidationView> {
            Err(AdmError::new("mock save not implemented"))
        }

        fn validate_ai_config(&self, _: &AiConfig) -> AiConfigValidationView {
            AiConfigValidationView {
                ok: true,
                errors: Vec::new(),
                warnings: Vec::new(),
            }
        }

        fn completion_adapter_spec(&self, _: &AiConfig) -> AdmResult<CompletionAdapterSpecView> {
            Err(AdmError::new("mock spec not implemented"))
        }

        fn load_project_config(&self) -> AdmResult<ProjectRuntimeSettings> {
            Err(AdmError::new("mock project config not implemented"))
        }

        fn save_project_config(
            &self,
            _: &SaveProjectConfigRequest,
        ) -> AdmResult<ProjectConfigView> {
            Err(AdmError::new("mock project save not implemented"))
        }

        fn run_project_preflight(
            &self,
            _: &ProjectPreflightRequest,
        ) -> AdmResult<DevelopmentPreflightReport> {
            Err(AdmError::new("mock preflight not implemented"))
        }
    }

    fn valid_config() -> AiConfig {
        AiConfig {
            dev: local_codex_category(),
            completion: ApiCategory {
                category_id: "completion".to_string(),
                entries: vec![ApiEntry {
                    id: "completion".to_string(),
                    label: "Completion".to_string(),
                    config_type: "openai_completion_api".to_string(),
                    api_url: "https://api.example.test/v1".to_string(),
                    api_key: "secret".to_string(),
                    extra_json: serde_json::json!({"model": "gpt-5.5"}),
                    ..ApiEntry::default()
                }],
                active_entry_id: "completion".to_string(),
            },
            ..AiConfig::default()
        }
    }

    fn local_codex_category() -> ApiCategory {
        ApiCategory {
            category_id: "dev".to_string(),
            entries: vec![ApiEntry {
                id: "codex_cli".to_string(),
                label: "Codex".to_string(),
                config_type: "local_codex_cli".to_string(),
                ..ApiEntry::default()
            }],
            active_entry_id: "codex_cli".to_string(),
        }
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm_new_tauri_config_{label}_{}",
            new_stable_id("root").unwrap()
        ))
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
