use adm_new_foundation::paths::{ProjectPaths, relative_display, resolve_configured_path};
use adm_new_foundation::{
    AdmError, AdmResult, new_stable_id, sanitize_identifier, sha256_hex, structured_md,
    unix_timestamp, write_text_atomic,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const STOP_REQUEST_NAME: &str = "stop_request.json";
pub const RUN_STATE_NAME: &str = "run_state.json";
pub const RUN_CONTEXT_ENV: &str = "AUTODESIGNMAKER_RUN_CONTEXT_FILE";
pub const RUN_CONTEXT_RELATIVE_PATH: &str = "runtime/run_context.json";
pub const SETTINGS_SNAPSHOT_RELATIVE_PATH: &str = "runtime/project_settings.snapshot.json";
pub const EXECUTION_OBJECT_STORE_RELATIVE_PATH: &str =
    "outputs/execution_objects/execution_objects.json";
pub const PIPELINE_STATE_NAME: &str = "pipeline_state.md";

const VALID_PIPELINE_STATUSES: &[&str] = &[
    "pending",
    "in_progress",
    "success",
    "failed",
    "skipped",
    "blocked",
    "stopped",
    "waiting_confirmation",
    "completed_with_review",
];

#[derive(Debug, Clone)]
pub struct RuntimeApplicationService {
    paths: ProjectPaths,
}

impl RuntimeApplicationService {
    pub fn new(root: impl AsRef<Path>, session_id: &str) -> AdmResult<Self> {
        let session_id = sanitize_identifier(session_id)?;
        let paths = ProjectPaths::new(root.as_ref(), session_id);
        paths.ensure_current_draft_dirs()?;
        Ok(Self { paths })
    }

    pub fn paths(&self) -> &ProjectPaths {
        &self.paths
    }

    pub fn control_dir(&self) -> PathBuf {
        self.paths.runtime_control_dir.clone()
    }

    pub fn stop_request_path(&self) -> PathBuf {
        self.control_dir().join(STOP_REQUEST_NAME)
    }

    pub fn run_state_path(&self) -> PathBuf {
        self.control_dir().join(RUN_STATE_NAME)
    }

    pub fn current_run_id(&self) -> String {
        read_json_or(&self.run_state_path(), json!({}))
            .get("run_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }

    pub fn request_stop(&self, options: StopRequestOptions) -> AdmResult<StopRequest> {
        let run_id = options.run_id.unwrap_or_else(|| self.current_run_id());
        let request = StopRequest {
            schema_version: 1,
            status: "requested".to_string(),
            mode: options.mode,
            boundary: options.boundary,
            reason: options.reason,
            scope: options.scope,
            run_id,
            requested_at: now_runtime_timestamp(),
        };
        write_json_value(&self.stop_request_path(), &to_json_value(&request)?)?;
        Ok(request)
    }

    pub fn read_stop_request(&self) -> AdmResult<Option<StopRequest>> {
        let value = read_json_or(&self.stop_request_path(), json!({}));
        if !value.as_object().is_some_and(|object| !object.is_empty()) {
            return Ok(None);
        }
        Ok(serde_json::from_value(value).ok())
    }

    pub fn read_stop_request_value(&self) -> Value {
        object_or_empty(read_json_or(&self.stop_request_path(), json!({})))
    }

    pub fn stop_requested(&self) -> bool {
        self.read_stop_request_value()
            .get("status")
            .and_then(Value::as_str)
            == Some("requested")
    }

    pub fn clear_stop_request(&self) -> AdmResult<()> {
        let path = self.stop_request_path();
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn clear_stale_stop_request(&self, run_id: &str) -> AdmResult<()> {
        let request = self.read_stop_request_value();
        let requested_same_run = request.get("status").and_then(Value::as_str) == Some("requested")
            && request.get("run_id").and_then(Value::as_str) == Some(run_id);
        if !requested_same_run {
            self.clear_stop_request()?;
        }
        Ok(())
    }

    pub fn read_run_state(&self) -> Value {
        object_or_empty(read_json_or(&self.run_state_path(), json!({})))
    }

    pub fn write_run_state(
        &self,
        updates: Value,
        replace: bool,
        clear_fields: &[&str],
    ) -> AdmResult<Value> {
        let mut current = if replace {
            Map::new()
        } else {
            object_map_or_empty(read_json_or(&self.run_state_path(), json!({})))
        };
        for field in clear_fields {
            current.remove(*field);
        }
        let updates = updates
            .as_object()
            .ok_or_else(|| AdmError::new("run state updates must be a JSON object"))?;
        current.insert("schema_version".to_string(), json!(1));
        current.insert(
            "updated_at".to_string(),
            Value::String(now_runtime_timestamp()),
        );
        for (key, value) in updates {
            current.insert(key.clone(), value.clone());
        }
        let value = Value::Object(current);
        write_json_value(&self.run_state_path(), &value)?;
        Ok(value)
    }

    pub fn mark_stopped(&self, updates: Value) -> AdmResult<Value> {
        let request = self.read_stop_request_value();
        let mut stopped = object_map_or_empty(updates);
        stopped.insert("status".to_string(), Value::String("stopped".to_string()));
        stopped.insert(
            "stopped_at".to_string(),
            Value::String(now_runtime_timestamp()),
        );
        stopped.insert("stop_request".to_string(), request.clone());
        let stopped_value = Value::Object(stopped);
        self.write_run_state(stopped_value.clone(), false, &[])?;
        if request.as_object().is_some_and(|object| !object.is_empty()) {
            let mut handled = object_map_or_empty(request);
            handled.insert("status".to_string(), Value::String("handled".to_string()));
            handled.insert(
                "handled_at".to_string(),
                Value::String(now_runtime_timestamp()),
            );
            write_json_value(&self.stop_request_path(), &Value::Object(handled))?;
        }
        Ok(stopped_value)
    }

    pub fn project_settings_path(&self) -> PathBuf {
        self.paths.settings_dir.join("project_settings.json")
    }

    pub fn project_bindings_path(&self) -> PathBuf {
        self.paths.settings_dir.join("project_bindings.json")
    }

    pub fn active_project_settings_path(&self) -> PathBuf {
        self.paths.draft_dir.join("project_config.json")
    }

    pub fn preflight_report_path(&self) -> PathBuf {
        self.paths
            .outputs_dir
            .join("preflight")
            .join("actual_development_preflight.json")
    }

    pub fn blank_project_settings(&self) -> ProjectRuntimeSettings {
        let mut settings =
            normalize_project_settings(&read_json_or(&self.project_settings_path(), json!({})));
        settings.schema_version = 2;
        settings.binding_id = new_stable_id("project_binding")
            .unwrap_or_else(|_| format!("project_binding_{}", unix_timestamp()));
        settings.development_path.clear();
        settings.editor_path.clear();
        settings
    }

    pub fn save_project_settings(&self, settings: &ProjectRuntimeSettings) -> AdmResult<PathBuf> {
        let mut existing = object_map_or_empty(read_json_or(
            &self.active_project_settings_path(),
            json!({}),
        ));
        if existing.is_empty() {
            existing = object_map_or_empty(read_json_or(&self.project_settings_path(), json!({})));
        }
        let existing_settings = normalize_project_settings(&Value::Object(existing.clone()));
        let binding_id = if !settings.binding_id.trim().is_empty() {
            sanitize_identifier(&settings.binding_id)?
        } else if !existing_settings.binding_id.trim().is_empty() {
            sanitize_identifier(&existing_settings.binding_id)?
        } else {
            new_stable_id("project_binding")?
        };
        let required_editor_version = if !settings.required_editor_version.trim().is_empty() {
            settings.required_editor_version.trim().to_string()
        } else if settings.project_engine == "unity" && !settings.development_path.trim().is_empty()
        {
            crate::project_environment::read_unity_project_version(resolve_configured_path(
                &settings.development_path,
                &self.paths.project_root,
            ))
            .map(|version| version.version)
            .unwrap_or_default()
        } else {
            String::new()
        };

        let mut bindings = self.load_project_binding_store();
        bindings.bindings.insert(
            binding_id.clone(),
            ProjectMachineBinding {
                project_path: settings.development_path.trim().to_string(),
                editor_path: settings.editor_path.trim().to_string(),
                verified_at: now_runtime_timestamp(),
            },
        );
        write_json_value(&self.project_bindings_path(), &to_json_value(&bindings)?)?;

        existing.remove("development_path");
        existing.remove("editor_path");
        existing.insert("schema_version".to_string(), json!(2));
        existing.insert("binding_id".to_string(), json!(binding_id));
        existing.insert("project_engine".to_string(), json!(settings.project_engine));
        existing.insert(
            "pipeline_adapter".to_string(),
            json!(settings.pipeline_adapter),
        );
        existing.insert(
            "game_spec_v2_enabled".to_string(),
            json!(settings.game_spec_v2_enabled),
        );
        existing.insert(
            "game_spec_v2".to_string(),
            json!({"enabled": settings.game_spec_v2_enabled}),
        );
        existing.insert(
            "custom_engine_name".to_string(),
            json!(settings.custom_engine_name),
        );
        existing.insert(
            "required_editor_version".to_string(),
            json!(required_editor_version),
        );
        let path = self.active_project_settings_path();
        write_json_value(&path, &Value::Object(existing))?;
        Ok(path)
    }

    /// Rebinds an existing logical project to paths on this machine.
    ///
    /// This operation deliberately writes only `project_bindings.json`; the
    /// archived logical project document is treated as immutable metadata.
    pub fn relink_project_binding(
        &self,
        logical_settings: &ProjectRuntimeSettings,
        project_path_text: &str,
        editor_path_text: &str,
    ) -> AdmResult<ProjectRuntimeSettings> {
        let active = read_json_or(&self.active_project_settings_path(), json!({}));
        let logical_value = if active.as_object().is_some_and(|object| !object.is_empty()) {
            active
        } else {
            read_json_or(&self.project_settings_path(), json!({}))
        };
        let persisted = normalize_project_settings(&logical_value);
        if persisted.binding_id.trim().is_empty() {
            return Err(AdmError::new(
                "project binding is missing; save the logical project configuration before relinking",
            ));
        }
        let binding_id = sanitize_identifier(&persisted.binding_id)?;
        if !logical_settings.binding_id.trim().is_empty()
            && sanitize_identifier(&logical_settings.binding_id)? != binding_id
        {
            return Err(AdmError::new(
                "project binding conflict: requested binding_id does not match the active project",
            ));
        }
        if !logical_settings.project_engine.trim().is_empty()
            && logical_settings.project_engine != persisted.project_engine
        {
            return Err(AdmError::new(
                "project engine conflict: relink cannot change logical project metadata",
            ));
        }

        let project_path_text = project_path_text.trim();
        if project_path_text.is_empty() {
            return Err(AdmError::new("relink project_path must not be empty"));
        }
        let project_path = resolve_configured_path(project_path_text, &self.paths.project_root);
        if !project_path.is_dir() {
            return Err(AdmError::new(format!(
                "relink project_path not found: {}",
                project_path.display()
            )));
        }
        if persisted.project_engine != "custom" {
            let inspection = crate::project_environment::inspect_project_directory(
                &project_path,
                &persisted.project_engine,
            );
            if inspection.status == "invalid"
                || inspection.detected_engine != persisted.project_engine
            {
                return Err(AdmError::new(format!(
                    "project engine conflict: expected {}, detected {}",
                    persisted.project_engine,
                    if inspection.detected_engine.is_empty() {
                        "unknown"
                    } else {
                        &inspection.detected_engine
                    }
                )));
            }
            if persisted.project_engine == "unity" {
                let detected_version = inspection
                    .unity_version
                    .as_ref()
                    .map(|item| item.version.as_str())
                    .unwrap_or_default();
                if !persisted.required_editor_version.trim().is_empty()
                    && !detected_version.is_empty()
                    && persisted.required_editor_version != detected_version
                {
                    return Err(AdmError::new(format!(
                        "Unity project version conflict: expected {}, detected {}",
                        persisted.required_editor_version, detected_version
                    )));
                }
            }
        }

        let editor_path_text = editor_path_text.trim();
        if persisted.project_engine != "custom" && editor_path_text.is_empty() {
            return Err(AdmError::new("relink editor_path must not be empty"));
        }
        if !editor_path_text.is_empty() {
            let editor_path = resolve_configured_path(editor_path_text, &self.paths.project_root);
            let validation = crate::project_environment::validate_editor_selection(
                &persisted.project_engine,
                &project_path,
                &editor_path,
            );
            if !validation.valid {
                return Err(AdmError::new(format!(
                    "invalid editor selection: {}",
                    validation.error_code
                )));
            }
        }

        let mut bindings = self.load_project_binding_store();
        bindings.bindings.insert(
            binding_id,
            ProjectMachineBinding {
                project_path: project_path_text.to_string(),
                editor_path: editor_path_text.to_string(),
                verified_at: now_runtime_timestamp(),
            },
        );
        write_json_value(&self.project_bindings_path(), &to_json_value(&bindings)?)?;

        let mut effective = persisted;
        effective.schema_version = 2;
        effective.development_path = project_path_text.to_string();
        effective.editor_path = editor_path_text.to_string();
        Ok(effective)
    }

    pub fn load_project_settings(&self, prefer_run_context: bool) -> ProjectRuntimeSettings {
        if prefer_run_context {
            if let Ok(Some(snapshot)) = self.load_settings_snapshot() {
                return self.resolve_project_binding(normalize_project_settings(&snapshot));
            }
        }
        let active = read_json_or(&self.active_project_settings_path(), json!({}));
        if active.as_object().is_some_and(|object| !object.is_empty()) {
            return self.resolve_project_binding(normalize_project_settings(&active));
        }
        self.resolve_project_binding(normalize_project_settings(&read_json_or(
            &self.project_settings_path(),
            json!({}),
        )))
    }

    fn load_project_binding_store(&self) -> ProjectBindingStore {
        serde_json::from_value(read_json_or(&self.project_bindings_path(), json!({})))
            .unwrap_or_default()
    }

    fn resolve_project_binding(
        &self,
        mut settings: ProjectRuntimeSettings,
    ) -> ProjectRuntimeSettings {
        if settings.binding_id.trim().is_empty() {
            return settings;
        }
        settings.schema_version = 2;
        let bindings = self.load_project_binding_store();
        if let Some(binding) = bindings.bindings.get(&settings.binding_id) {
            settings.development_path = binding.project_path.clone();
            settings.editor_path = binding.editor_path.clone();
        } else {
            settings.development_path.clear();
            settings.editor_path.clear();
        }
        settings
    }

    pub fn run_actual_development_preflight(
        &self,
        write_report: bool,
        prefer_run_context: bool,
    ) -> AdmResult<DevelopmentPreflightReport> {
        let settings = self.load_project_settings(prefer_run_context);
        let mut blockers = Vec::new();
        let mut warnings = Vec::new();
        let mut diagnostics = Vec::new();
        let development_path_text = settings.development_path.trim();
        let editor_path_text = settings.editor_path.trim();

        if settings.project_engine == "custom" {
            if !development_path_text.is_empty() {
                let dev_path =
                    resolve_configured_path(development_path_text, &self.paths.project_root);
                if !dev_path.exists() {
                    let message =
                        format!("development_path does not exist: {}", dev_path.display());
                    warnings.push(message.clone());
                    diagnostics.push(DevelopmentPreflightDiagnostic::warning(
                        "development_path_not_found",
                        "development_path",
                        message,
                        "reselect_project_path",
                    ));
                }
            }
        } else if development_path_text.is_empty() {
            let blocker = PreflightBlocker::new(
                "missing_development_path",
                "development_path",
                "development_path is not set.",
                "Set development_path in the current save project config.",
            );
            diagnostics.push(DevelopmentPreflightDiagnostic::blocker(
                &blocker.code,
                &blocker.field,
                &blocker.message,
                "reselect_project_path",
            ));
            blockers.push(blocker);
        } else {
            let dev_path = resolve_configured_path(development_path_text, &self.paths.project_root);
            if !dev_path.exists() {
                let blocker = PreflightBlocker::new(
                    "development_path_not_found",
                    "development_path",
                    format!("development_path does not exist: {}", dev_path.display()),
                    "Update development_path in the current save project config.",
                );
                diagnostics.push(DevelopmentPreflightDiagnostic::blocker(
                    &blocker.code,
                    &blocker.field,
                    &blocker.message,
                    "reselect_project_path",
                ));
                blockers.push(blocker);
            } else {
                let inspection = crate::project_environment::inspect_project_directory(
                    &dev_path,
                    &settings.project_engine,
                );
                if inspection.status == "invalid"
                    || inspection.detected_engine != settings.project_engine
                {
                    let blocker = PreflightBlocker::new(
                        "project_engine_conflict",
                        "development_path",
                        format!(
                            "configured engine {} does not match detected engine {}",
                            settings.project_engine,
                            if inspection.detected_engine.is_empty() {
                                "unknown"
                            } else {
                                &inspection.detected_engine
                            }
                        ),
                        "Select the correct project directory.",
                    );
                    diagnostics.push(DevelopmentPreflightDiagnostic::blocker(
                        &blocker.code,
                        &blocker.field,
                        &blocker.message,
                        "reselect_project_path",
                    ));
                    blockers.push(blocker);
                }
                if settings.project_engine == "unity" {
                    let markers = unity_project_markers(&dev_path);
                    if !markers.assets_dir {
                        let message = "Unity project missing Assets/ directory.".to_string();
                        warnings.push(message.clone());
                        diagnostics.push(DevelopmentPreflightDiagnostic::warning(
                            "unity_assets_directory_missing",
                            "development_path",
                            message,
                            "reselect_project_path",
                        ));
                    }
                }
            }
        }

        if settings.project_engine == "unity" {
            if editor_path_text.is_empty() {
                let blocker = PreflightBlocker::new(
                    "missing_editor_path",
                    "editor_path",
                    "editor_path is not set.",
                    "Discover or select a Unity editor.",
                );
                diagnostics.push(DevelopmentPreflightDiagnostic::blocker(
                    &blocker.code,
                    &blocker.field,
                    &blocker.message,
                    "rescan_editor",
                ));
                blockers.push(blocker);
            } else {
                let editor_path =
                    resolve_configured_path(editor_path_text, &self.paths.project_root);
                let project_path =
                    resolve_configured_path(development_path_text, &self.paths.project_root);
                let validation = crate::project_environment::validate_editor_selection(
                    "unity",
                    &project_path,
                    &editor_path,
                );
                if !validation.valid {
                    let blocker = PreflightBlocker::new(
                        if validation.error_code.is_empty() {
                            "invalid_editor_path"
                        } else {
                            &validation.error_code
                        },
                        "editor_path",
                        format!("editor_path is not a compatible Unity editor: {editor_path_text}"),
                        "Select a valid Unity.exe or scan installed Unity editors.",
                    );
                    diagnostics.push(DevelopmentPreflightDiagnostic::blocker(
                        &blocker.code,
                        &blocker.field,
                        &blocker.message,
                        if validation.error_code == "unity_editor_version_conflict" {
                            "rescan_editor"
                        } else {
                            "reselect_editor_path"
                        },
                    ));
                    blockers.push(blocker);
                }
            }
        } else if matches!(settings.project_engine.as_str(), "unreal" | "godot")
            && editor_path_text.is_empty()
        {
            let message = format!(
                "editor_path is not set for {}.",
                engine_label(&settings.project_engine)
            );
            warnings.push(message.clone());
            diagnostics.push(DevelopmentPreflightDiagnostic::warning(
                "missing_editor_path",
                "editor_path",
                message,
                "reselect_editor_path",
            ));
        }

        let report = DevelopmentPreflightReport {
            schema_version: 2,
            timestamp: now_runtime_timestamp(),
            status: if blockers.is_empty() {
                "passed".to_string()
            } else {
                "blocked".to_string()
            },
            blockers,
            warnings,
            diagnostics,
            settings,
        };
        if write_report {
            let persisted = preflight_report_for_storage(&report, &self.paths.project_root);
            write_json_value(&self.preflight_report_path(), &to_json_value(&persisted)?)?;
        }
        Ok(report)
    }

    pub fn assert_actual_development_preflight(
        &self,
        write_report: bool,
        prefer_run_context: bool,
    ) -> AdmResult<()> {
        let report = self.run_actual_development_preflight(write_report, prefer_run_context)?;
        if report.status == "passed" {
            Ok(())
        } else {
            Err(AdmError::new(format!(
                "Development preflight blocked: {:?}",
                report.blockers
            )))
        }
    }

    pub fn run_context_path(&self) -> PathBuf {
        if let Ok(path) = std::env::var(RUN_CONTEXT_ENV) {
            let path = path.trim();
            if !path.is_empty() {
                let configured = PathBuf::from(path);
                let candidate = if configured.is_absolute() {
                    if !configured.starts_with(&self.paths.project_root) {
                        return self.paths.draft_dir.join(RUN_CONTEXT_RELATIVE_PATH);
                    }
                    configured
                } else {
                    if configured.components().any(|component| {
                        matches!(
                            component,
                            std::path::Component::ParentDir
                                | std::path::Component::Prefix(_)
                                | std::path::Component::RootDir
                        )
                    }) {
                        return self.paths.draft_dir.join(RUN_CONTEXT_RELATIVE_PATH);
                    }
                    self.paths.project_root.join(configured)
                };
                if path_is_inside_root(&candidate, &self.paths.project_root) {
                    return candidate;
                }
            }
        }
        self.paths.draft_dir.join(RUN_CONTEXT_RELATIVE_PATH)
    }

    pub fn settings_snapshot_path(&self) -> PathBuf {
        self.paths.draft_dir.join(SETTINGS_SNAPSHOT_RELATIVE_PATH)
    }

    pub fn load_run_context(&self, required: bool) -> AdmResult<Option<RunContext>> {
        let path = self.run_context_path();
        if !path.exists() {
            return if required {
                Err(AdmError::new(format!(
                    "run_context not found: {}",
                    path.display()
                )))
            } else {
                Ok(None)
            };
        }
        let value = read_json_or(&path, json!({}));
        if !value.as_object().is_some_and(|object| !object.is_empty()) {
            return if required {
                Err(AdmError::new(format!(
                    "run_context not found: {}",
                    path.display()
                )))
            } else {
                Ok(None)
            };
        }
        match RunContext::from_value(&value)
            .and_then(|context| self.normalize_and_resolve_run_context(&path, context))
        {
            Ok(context) => Ok(Some(context)),
            Err(error) if required => Err(error),
            Err(_) => Ok(None),
        }
    }

    pub fn load_settings_snapshot(&self) -> AdmResult<Option<Value>> {
        let path = if let Some(context) = self.load_run_context(false)? {
            PathBuf::from(context.settings_snapshot)
        } else {
            self.settings_snapshot_path()
        };
        let value = read_json_or(&path, json!({}));
        if value.as_object().is_some_and(|object| !object.is_empty()) {
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    pub fn create_run_context(
        &self,
        run_id: &str,
        save_id: &str,
        settings: Option<ProjectRuntimeSettings>,
        overwrite: bool,
        activate: bool,
    ) -> AdmResult<RunContext> {
        let path = self.run_context_path();
        if path.exists() && !overwrite {
            if let Some(existing) = self.load_run_context(false)? {
                return Ok(existing);
            }
        }

        let normalized_settings = settings.unwrap_or_else(|| self.load_project_settings(false));
        let snapshot = self.settings_snapshot_path();
        let mut snapshot_payload = object_map_or_empty(to_json_value(&normalized_settings)?);
        snapshot_payload.insert("schema_version".to_string(), json!(1));
        snapshot_payload.insert(
            "snapshot_generated_at".to_string(),
            Value::String(now_runtime_timestamp()),
        );
        snapshot_payload.insert(
            "source_settings_path".to_string(),
            Value::String(project_relative_path(
                &self.paths.project_root,
                &self.active_project_settings_path(),
            )?),
        );
        write_json_value(&snapshot, &Value::Object(snapshot_payload))?;

        let execution_store = self
            .paths
            .project_root
            .join("saves")
            .join(save_id)
            .join("workspace")
            .join(EXECUTION_OBJECT_STORE_RELATIVE_PATH);
        let persisted_context = RunContext {
            schema_version: "2.0".to_string(),
            run_id: run_id.to_string(),
            draft_session_id: self.paths.session_id.clone(),
            save_id: save_id.to_string(),
            project_root: ".".to_string(),
            draft_root: project_relative_path(&self.paths.project_root, &self.paths.draft_dir)?,
            source_artifacts_root: project_relative_path(
                &self.paths.project_root,
                &self.paths.source_artifacts_dir,
            )?,
            settings_snapshot: project_relative_path(&self.paths.project_root, &snapshot)?,
            development_path: normalized_settings.development_path,
            editor_path: normalized_settings.editor_path,
            project_engine: normalized_settings.project_engine,
            pipeline_adapter: normalized_settings.pipeline_adapter,
            execution_object_store: project_relative_path(
                &self.paths.project_root,
                &execution_store,
            )?,
            artifact_root: project_relative_path(
                &self.paths.project_root,
                &self.paths.artifacts_dir,
            )?,
            log_root: project_relative_path(&self.paths.project_root, &self.paths.run_logs_dir)?,
            owner_pid: std::process::id(),
            created_at: now_runtime_timestamp(),
            isolation_policy: isolation_policy(),
        };
        write_json_value(&path, &to_json_value(&persisted_context)?)?;
        if activate {
            self.write_run_state(
                json!({"run_context_path": project_relative_path(&self.paths.project_root, &path)?}),
                false,
                &[],
            )?;
        }
        resolve_run_context_paths(persisted_context, &self.paths.project_root)
    }

    pub fn ensure_run_context(&self, run_id: &str, save_id: &str) -> AdmResult<RunContext> {
        if let Some(context) = self.load_run_context(false)? {
            self.write_run_state(
                json!({"run_context_path": project_relative_path(
                    &self.paths.project_root,
                    &self.run_context_path(),
                )?}),
                false,
                &[],
            )?;
            return Ok(context);
        }
        self.create_run_context(run_id, save_id, None, false, true)
    }

    fn normalize_and_resolve_run_context(
        &self,
        path: &Path,
        mut context: RunContext,
    ) -> AdmResult<RunContext> {
        let save_id = sanitize_identifier(&context.save_id)?;
        context.schema_version = "2.0".to_string();
        context.draft_session_id = self.paths.session_id.clone();
        context.project_root = ".".to_string();
        context.draft_root =
            project_relative_path(&self.paths.project_root, &self.paths.draft_dir)?;
        context.source_artifacts_root =
            project_relative_path(&self.paths.project_root, &self.paths.source_artifacts_dir)?;
        context.settings_snapshot =
            project_relative_path(&self.paths.project_root, &self.settings_snapshot_path())?;
        context.execution_object_store = project_relative_path(
            &self.paths.project_root,
            &self
                .paths
                .project_root
                .join("saves")
                .join(save_id)
                .join("workspace")
                .join(EXECUTION_OBJECT_STORE_RELATIVE_PATH),
        )?;
        context.artifact_root =
            project_relative_path(&self.paths.project_root, &self.paths.artifacts_dir)?;
        context.log_root =
            project_relative_path(&self.paths.project_root, &self.paths.run_logs_dir)?;
        let normalized = to_json_value(&context)?;
        if normalized != read_json_or(path, json!({})) {
            write_json_value(path, &normalized)?;
        }
        resolve_run_context_paths(context, &self.paths.project_root)
    }

    pub fn draft_meta_bound_save_id(&self) -> Option<String> {
        let value = read_json_or(&self.paths.draft_meta_file, json!({}));
        if let Some(save_id) = value
            .get("linked_save_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(save_id.to_string());
        }
        let archive_path = value
            .get("linked_archive_path")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if archive_path.is_empty() {
            None
        } else {
            Path::new(archive_path)
                .file_name()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        }
    }

    pub fn bound_save_id(&self, allow_current: bool) -> Option<String> {
        if let Ok(Some(context)) = self.load_run_context(false) {
            return Some(context.save_id);
        }
        if let Some(save_id) = self.draft_meta_bound_save_id() {
            return Some(save_id);
        }
        if allow_current {
            read_json_or(&self.paths.saves_dir.join("save_index.json"), json!({}))
                .get("current_save_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        } else {
            None
        }
    }

    pub fn compare_identity(
        &self,
        artifact: &Value,
        context: Option<&RunContext>,
        source: &str,
        fields: &[&str],
    ) -> Vec<ContextMismatchIssue> {
        let Some(artifact) = artifact.as_object() else {
            return Vec::new();
        };
        let owned_context;
        let context = match context {
            Some(context) => context,
            None => match self.load_run_context(false) {
                Ok(Some(context)) => {
                    owned_context = context;
                    &owned_context
                }
                _ => return Vec::new(),
            },
        };
        let expected = identity_payload(context);
        fields
            .iter()
            .filter_map(|field| {
                let expected_value = expected.get(*field).cloned().unwrap_or_default();
                let actual_value = artifact
                    .get(*field)
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if !expected_value.is_empty()
                    && !actual_value.is_empty()
                    && expected_value != actual_value
                {
                    Some(context_mismatch_issue(
                        field,
                        expected_value,
                        actual_value,
                        source,
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn pipeline_state_path(&self) -> PathBuf {
        self.paths.source_artifacts_dir.join(PIPELINE_STATE_NAME)
    }

    pub fn load_pipeline_state(&self) -> AdmResult<Value> {
        let path = self.pipeline_state_path();
        if !path.exists() {
            return Ok(json!({"steps": {}}));
        }
        let mut value = structured_md::read_structured_or_text(&path)?;
        let object = object_map_or_empty(value.take());
        let mut value = Value::Object(object);
        ensure_steps_object(&mut value);
        Ok(value)
    }

    pub fn save_pipeline_state(&self, state: Value) -> AdmResult<PathBuf> {
        let mut state = object_map_or_empty(state);
        state.insert(
            "updated_at".to_string(),
            Value::String(now_runtime_timestamp()),
        );
        let value = Value::Object(state);
        structured_md::write_data(&self.pipeline_state_path(), &value, "Pipeline State")
    }

    pub fn update_step_state(
        &self,
        step_number: u32,
        status: &str,
        snapshot_id: Option<&str>,
        output_path: Option<&str>,
        message: Option<&str>,
    ) -> AdmResult<PathBuf> {
        if !VALID_PIPELINE_STATUSES.contains(&status) {
            return Err(AdmError::new(format!("Invalid pipeline status: {status}")));
        }
        let mut state = self.load_pipeline_state()?;
        ensure_steps_object(&mut state);
        let steps = state
            .get_mut("steps")
            .and_then(Value::as_object_mut)
            .ok_or_else(|| AdmError::new("pipeline state steps must be an object"))?;
        let key = step_number.to_string();
        let mut step_state = steps
            .get(&key)
            .cloned()
            .map(object_map_or_empty)
            .unwrap_or_default();
        step_state.insert("step".to_string(), json!(step_number));
        step_state.insert(
            "step_name".to_string(),
            Value::String(step_name(step_number)),
        );
        step_state.insert("status".to_string(), Value::String(status.to_string()));
        step_state.insert("timestamp".to_string(), Value::String(compact_timestamp()));
        if let Some(value) = snapshot_id {
            step_state.insert("snapshot_id".to_string(), Value::String(value.to_string()));
        }
        if let Some(value) = output_path {
            step_state.insert("output_path".to_string(), Value::String(value.to_string()));
        }
        if let Some(value) = message {
            step_state.insert("message".to_string(), Value::String(value.to_string()));
        }
        steps.insert(key, Value::Object(step_state));
        self.save_pipeline_state(state)
    }

    pub fn get_step_state(&self, step_number: u32) -> AdmResult<Option<Value>> {
        Ok(self
            .load_pipeline_state()?
            .get("steps")
            .and_then(Value::as_object)
            .and_then(|steps| steps.get(&step_number.to_string()).cloned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopRequestOptions {
    pub mode: String,
    pub boundary: String,
    pub reason: String,
    pub scope: String,
    pub run_id: Option<String>,
}

impl Default for StopRequestOptions {
    fn default() -> Self {
        Self {
            mode: "graceful".to_string(),
            boundary: "after_current_unit".to_string(),
            reason: "operator_stop".to_string(),
            scope: "current_run".to_string(),
            run_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StopRequest {
    pub schema_version: u32,
    pub status: String,
    pub mode: String,
    pub boundary: String,
    pub reason: String,
    pub scope: String,
    pub run_id: String,
    pub requested_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineExecutionConfig {
    pub max_concurrent_dev_tasks: u32,
    pub max_concurrent_art_tasks: u32,
    pub write_conflict_policy: String,
    pub group_compile_policy: String,
}

impl Default for PipelineExecutionConfig {
    fn default() -> Self {
        Self {
            max_concurrent_dev_tasks: 1,
            max_concurrent_art_tasks: 1,
            write_conflict_policy: "serialize".to_string(),
            group_compile_policy: "after_group".to_string(),
        }
    }
}

impl PipelineExecutionConfig {
    pub fn from_value(value: &Value) -> Self {
        let execution = value
            .get("pipeline")
            .and_then(|pipeline| pipeline.get("execution"))
            .unwrap_or(value);
        Self {
            max_concurrent_dev_tasks: bounded_u32(
                execution.get("max_concurrent_dev_tasks"),
                1,
                1,
                8,
            ),
            max_concurrent_art_tasks: bounded_u32(
                execution.get("max_concurrent_art_tasks"),
                1,
                1,
                8,
            ),
            write_conflict_policy: execution
                .get("write_conflict_policy")
                .and_then(Value::as_str)
                .unwrap_or("serialize")
                .to_string(),
            group_compile_policy: execution
                .get("group_compile_policy")
                .and_then(Value::as_str)
                .unwrap_or("after_group")
                .to_string(),
        }
    }
}

pub fn new_run_id() -> AdmResult<String> {
    Ok(format!(
        "{}-{}",
        unix_timestamp(),
        new_stable_id("run")?.trim_start_matches("run_")
    ))
}

pub fn normalize_write_path(path: &str) -> String {
    path.replace('\\', "/").trim().trim_matches('/').to_string()
}

pub fn task_write_set(task: &Value) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for key in ["output_files", "allowed_write_paths"] {
        if let Some(values) = task.get(key).and_then(Value::as_array) {
            for value in values {
                let path = normalize_write_path(value.as_str().unwrap_or(&value.to_string()));
                if !path.is_empty() {
                    paths.insert(path);
                }
            }
        }
    }
    if task
        .get("package_changes")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
    {
        paths.insert("Packages/manifest.json".to_string());
    }
    paths
}

pub fn build_conflict_batches(
    task_ids: &[String],
    tasks_by_id: &BTreeMap<String, Value>,
) -> Vec<Vec<String>> {
    let mut batches: Vec<Vec<String>> = Vec::new();
    let mut batch_write_sets: Vec<BTreeSet<String>> = Vec::new();
    for task_id in task_ids {
        let writes = tasks_by_id
            .get(task_id)
            .map(task_write_set)
            .unwrap_or_default();
        let mut placed = false;
        for (index, existing_writes) in batch_write_sets.iter_mut().enumerate() {
            if writes.is_disjoint(existing_writes) {
                batches[index].push(task_id.clone());
                existing_writes.extend(writes.clone());
                placed = true;
                break;
            }
        }
        if !placed {
            batches.push(vec![task_id.clone()]);
            batch_write_sets.push(writes);
        }
    }
    batches
}

pub fn build_write_conflict_report(
    parallel_groups: &[ParallelGroupInput],
    tasks_by_id: &BTreeMap<String, Value>,
) -> WriteConflictReport {
    let mut groups = Vec::new();
    let mut conflict_group_count = 0usize;
    for group in parallel_groups {
        let task_ids = group
            .task_ids
            .iter()
            .map(|task_id| task_id.trim())
            .filter(|task_id| !task_id.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let conflict_batches = build_conflict_batches(&task_ids, tasks_by_id);
        let has_write_conflict = conflict_batches.len() > 1;
        if has_write_conflict {
            conflict_group_count += 1;
        }
        groups.push(WriteConflictGroupReport {
            group_id: group.group_id.clone(),
            task_ids,
            conflict_batches,
            has_write_conflict,
            execution: if has_write_conflict {
                "serial_batches".to_string()
            } else {
                "parallel_allowed".to_string()
            },
        });
    }
    WriteConflictReport {
        schema_version: 1,
        conflict_group_count,
        groups,
    }
}

pub fn build_parallel_readiness_report(
    parallel_groups: &[ParallelGroupInput],
    tasks_by_id: &BTreeMap<String, Value>,
    max_workers: u32,
) -> ParallelReadinessReport {
    let conflict_report = build_write_conflict_report(parallel_groups, tasks_by_id);
    let mut groups = Vec::new();
    let mut runnable_parallel_batch_count = 0usize;
    let mut serial_batch_count = 0usize;
    for group in conflict_report.groups {
        let mut batches = Vec::new();
        for batch in group.conflict_batches {
            let can_parallel = max_workers > 1 && batch.len() > 1;
            if can_parallel {
                runnable_parallel_batch_count += 1;
            } else {
                serial_batch_count += 1;
            }
            let worker_count = if can_parallel {
                max_workers.min(batch.len() as u32)
            } else {
                1
            };
            batches.push(ParallelBatchReport {
                task_ids: batch,
                execution: if can_parallel {
                    "parallel".to_string()
                } else {
                    "serial".to_string()
                },
                max_workers: worker_count,
                reason: if can_parallel {
                    "write_sets_disjoint".to_string()
                } else {
                    "single_task_or_parallel_disabled".to_string()
                },
            });
        }
        groups.push(ParallelReadinessGroupReport {
            group_id: group.group_id,
            has_write_conflict: group.has_write_conflict,
            batches,
        });
    }
    ParallelReadinessReport {
        schema_version: 1,
        max_workers,
        parallel_enabled: max_workers > 1,
        runnable_parallel_batch_count,
        serial_batch_count,
        ready_for_parallel_execution: max_workers > 1 && runnable_parallel_batch_count > 0,
        groups,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParallelGroupInput {
    pub group_id: String,
    pub task_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteConflictReport {
    pub schema_version: u32,
    pub conflict_group_count: usize,
    pub groups: Vec<WriteConflictGroupReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteConflictGroupReport {
    pub group_id: String,
    pub task_ids: Vec<String>,
    pub conflict_batches: Vec<Vec<String>>,
    pub has_write_conflict: bool,
    pub execution: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParallelReadinessReport {
    pub schema_version: u32,
    pub max_workers: u32,
    pub parallel_enabled: bool,
    pub runnable_parallel_batch_count: usize,
    pub serial_batch_count: usize,
    pub groups: Vec<ParallelReadinessGroupReport>,
    pub ready_for_parallel_execution: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParallelReadinessGroupReport {
    pub group_id: String,
    pub has_write_conflict: bool,
    pub batches: Vec<ParallelBatchReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParallelBatchReport {
    pub task_ids: Vec<String>,
    pub execution: String,
    pub max_workers: u32,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageExecutionStateSnapshot {
    pub execution_records: Vec<Value>,
    pub package_reports: Vec<Value>,
    pub changed_files_manifest: Vec<Value>,
    pub skipped_records: Vec<Value>,
}

#[derive(Debug, Clone, Default)]
pub struct ThreadSafeStageExecutionState {
    inner: Arc<Mutex<StageExecutionStateSnapshot>>,
}

impl Default for StageExecutionStateSnapshot {
    fn default() -> Self {
        Self {
            execution_records: Vec::new(),
            package_reports: Vec::new(),
            changed_files_manifest: Vec::new(),
            skipped_records: Vec::new(),
        }
    }
}

impl ThreadSafeStageExecutionState {
    pub fn append_execution_record(&self, record: Value) -> usize {
        let mut state = self
            .inner
            .lock()
            .expect("stage execution state lock poisoned");
        state.execution_records.push(record);
        state.execution_records.len() - 1
    }

    pub fn append_package_report(&self, report: Value) -> usize {
        let mut state = self
            .inner
            .lock()
            .expect("stage execution state lock poisoned");
        state.package_reports.push(report);
        state.package_reports.len() - 1
    }

    pub fn append_changed_files_manifest(&self, item: Value) -> usize {
        let mut state = self
            .inner
            .lock()
            .expect("stage execution state lock poisoned");
        state.changed_files_manifest.push(item);
        state.changed_files_manifest.len() - 1
    }

    pub fn append_skipped_record(&self, record: Value) -> usize {
        let mut state = self
            .inner
            .lock()
            .expect("stage execution state lock poisoned");
        state.skipped_records.push(record);
        state.skipped_records.len() - 1
    }

    pub fn snapshot(&self) -> StageExecutionStateSnapshot {
        self.inner
            .lock()
            .expect("stage execution state lock poisoned")
            .clone()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeLock {
    pub path: PathBuf,
    pub owner: Value,
    acquired: bool,
}

impl RuntimeLock {
    pub fn new(path: PathBuf, owner: Value) -> Self {
        Self {
            path,
            owner: object_or_empty(owner),
            acquired: false,
        }
    }

    pub fn acquire(mut self) -> AdmResult<Self> {
        let parent = self.path.parent().ok_or_else(|| {
            AdmError::new(format!("lock path has no parent: {}", self.path.display()))
        })?;
        fs::create_dir_all(parent)?;
        let mut payload = object_map_or_empty(self.owner.clone());
        payload.insert("schema_version".to_string(), json!(1));
        payload.insert(
            "created_at".to_string(),
            Value::String(now_runtime_timestamp()),
        );
        payload.insert("pid".to_string(), json!(std::process::id()));
        let payload_value = Value::Object(payload);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
        {
            Ok(mut file) => {
                let text = to_pretty_json(&payload_value)?;
                file.write_all(text.as_bytes())?;
                file.sync_all()?;
                self.owner = payload_value;
                self.acquired = true;
                Ok(self)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = read_json_or(&self.path, json!({}));
                Err(AdmError::new(format!(
                    "Runtime resource is locked: {} owner={existing}",
                    self.path.display()
                )))
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn release(&mut self) -> AdmResult<()> {
        if !self.acquired {
            return Ok(());
        }
        let existing = read_json_or(&self.path, json!({}));
        let same_pid = existing.get("pid") == self.owner.get("pid");
        let same_run = existing.get("run_id") == self.owner.get("run_id");
        if same_pid && same_run && self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        self.acquired = false;
        Ok(())
    }
}

impl Drop for RuntimeLock {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

pub fn lock_root(project_root: impl AsRef<Path>) -> PathBuf {
    project_root.as_ref().join("locks")
}

pub fn save_lock_path(project_root: impl AsRef<Path>, save_id: &str) -> PathBuf {
    lock_root(project_root)
        .join("saves")
        .join(format!("{save_id}.lock.json"))
}

pub fn draft_lock_path(project_root: impl AsRef<Path>, draft_session_id: &str) -> PathBuf {
    lock_root(project_root)
        .join("drafts")
        .join(format!("{draft_session_id}.lock.json"))
}

pub fn unity_lock_path(
    project_root: impl AsRef<Path>,
    development_path: impl AsRef<Path>,
) -> PathBuf {
    let hash = sha256_hex(development_path.as_ref().to_string_lossy().as_bytes());
    lock_root(project_root)
        .join("unity")
        .join(format!("{}.lock.json", &hash[..24]))
}

pub fn acquire_save_lock(
    project_root: impl AsRef<Path>,
    save_id: &str,
    run_id: &str,
    draft_session_id: &str,
) -> AdmResult<RuntimeLock> {
    RuntimeLock::new(
        save_lock_path(project_root, save_id),
        json!({
            "lock_type": "save",
            "save_id": save_id,
            "run_id": run_id,
            "draft_session_id": draft_session_id,
        }),
    )
    .acquire()
}

pub fn acquire_draft_lock(
    project_root: impl AsRef<Path>,
    draft_session_id: &str,
    run_id: &str,
    save_id: &str,
) -> AdmResult<RuntimeLock> {
    RuntimeLock::new(
        draft_lock_path(project_root, draft_session_id),
        json!({
            "lock_type": "draft",
            "draft_session_id": draft_session_id,
            "run_id": run_id,
            "save_id": save_id,
        }),
    )
    .acquire()
}

pub fn acquire_unity_project_lock(
    project_root: impl AsRef<Path>,
    development_path: impl AsRef<Path>,
    run_id: &str,
    save_id: &str,
    draft_session_id: &str,
) -> AdmResult<RuntimeLock> {
    let development_path_text = development_path.as_ref().to_string_lossy().to_string();
    RuntimeLock::new(
        unity_lock_path(project_root, development_path_text.as_str()),
        json!({
            "lock_type": "unity_project",
            "development_path": development_path_text,
            "run_id": run_id,
            "save_id": save_id,
            "draft_session_id": draft_session_id,
        }),
    )
    .acquire()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectRuntimeSettings {
    pub schema_version: u32,
    pub binding_id: String,
    pub project_engine: String,
    pub pipeline_adapter: String,
    pub game_spec_v2_enabled: bool,
    pub custom_engine_name: String,
    pub required_editor_version: String,
    pub development_path: String,
    pub editor_path: String,
}

impl Default for ProjectRuntimeSettings {
    fn default() -> Self {
        Self {
            schema_version: 2,
            binding_id: String::new(),
            project_engine: "unity".to_string(),
            pipeline_adapter: "none".to_string(),
            game_spec_v2_enabled: false,
            custom_engine_name: String::new(),
            required_editor_version: String::new(),
            development_path: String::new(),
            editor_path: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProjectBindingStore {
    #[serde(default = "project_binding_schema_version")]
    schema_version: u32,
    #[serde(default)]
    bindings: BTreeMap<String, ProjectMachineBinding>,
}

impl Default for ProjectBindingStore {
    fn default() -> Self {
        Self {
            schema_version: project_binding_schema_version(),
            bindings: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct ProjectMachineBinding {
    #[serde(default)]
    project_path: String,
    #[serde(default)]
    editor_path: String,
    #[serde(default)]
    verified_at: String,
}

fn project_binding_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnityProjectMarkers {
    pub assets_dir: bool,
    pub project_settings_dir: bool,
    pub packages_manifest: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevelopmentPreflightReport {
    pub schema_version: u32,
    pub timestamp: String,
    pub status: String,
    pub blockers: Vec<PreflightBlocker>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub diagnostics: Vec<DevelopmentPreflightDiagnostic>,
    pub settings: ProjectRuntimeSettings,
}

fn preflight_report_for_storage(
    report: &DevelopmentPreflightReport,
    configuration_root: &Path,
) -> DevelopmentPreflightReport {
    let mut persisted = report.clone();
    let configured_paths = [
        report.settings.development_path.trim(),
        report.settings.editor_path.trim(),
    ];
    let mut private_tokens = configured_paths
        .iter()
        .filter(|value| !value.is_empty())
        .flat_map(|value| {
            let resolved = resolve_configured_path(value, configuration_root)
                .to_string_lossy()
                .to_string();
            [
                (*value).to_string(),
                value.replace('\\', "/"),
                resolved.clone(),
                resolved.replace('\\', "/"),
            ]
        })
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    private_tokens.sort_by_key(|value| std::cmp::Reverse(value.len()));
    private_tokens.dedup();
    let redact = |text: &mut String| {
        for token in &private_tokens {
            *text = text.replace(token, "[MACHINE_PATH]");
        }
    };
    for blocker in &mut persisted.blockers {
        redact(&mut blocker.message);
        redact(&mut blocker.fix);
    }
    for warning in &mut persisted.warnings {
        redact(warning);
    }
    for diagnostic in &mut persisted.diagnostics {
        redact(&mut diagnostic.message);
    }
    persisted.settings.development_path.clear();
    persisted.settings.editor_path.clear();
    persisted
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevelopmentPreflightDiagnostic {
    pub severity: String,
    #[serde(alias = "code")]
    pub error_code: String,
    pub field: String,
    pub message: String,
    #[serde(alias = "fix")]
    pub fix_action: String,
}

impl DevelopmentPreflightDiagnostic {
    fn blocker(
        error_code: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
        fix_action: impl Into<String>,
    ) -> Self {
        Self::new("blocker", error_code, field, message, fix_action)
    }

    fn warning(
        error_code: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
        fix_action: impl Into<String>,
    ) -> Self {
        Self::new("warning", error_code, field, message, fix_action)
    }

    fn new(
        severity: impl Into<String>,
        error_code: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
        fix_action: impl Into<String>,
    ) -> Self {
        Self {
            severity: severity.into(),
            error_code: error_code.into(),
            field: field.into(),
            message: message.into(),
            fix_action: fix_action.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreflightBlocker {
    pub code: String,
    pub field: String,
    pub message: String,
    pub fix: String,
}

impl PreflightBlocker {
    fn new(
        code: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
        fix: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            field: field.into(),
            message: message.into(),
            fix: fix.into(),
        }
    }
}

pub fn normalize_project_settings(raw: &Value) -> ProjectRuntimeSettings {
    let mut settings = ProjectRuntimeSettings::default();
    settings.schema_version = raw
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(1);
    settings.binding_id = string_field(raw, "binding_id");
    let engine = raw
        .get("project_engine")
        .and_then(Value::as_str)
        .unwrap_or("unity")
        .trim()
        .to_ascii_lowercase();
    settings.project_engine = if matches!(engine.as_str(), "unity" | "unreal" | "godot" | "custom")
    {
        engine
    } else {
        "unity".to_string()
    };
    settings.pipeline_adapter = raw
        .get("pipeline_adapter")
        .and_then(Value::as_str)
        .unwrap_or("none")
        .trim()
        .to_ascii_lowercase();
    settings.game_spec_v2_enabled = bool_setting(raw, "game_spec_v2_enabled")
        || bool_setting(raw, "gameSpecV2Enabled")
        || bool_setting(raw, "game_spec_v2")
        || bool_at_path(raw, &["game_spec_v2", "enabled"])
        || bool_at_path(raw, &["gameSpecV2", "enabled"])
        || bool_at_path(raw, &["features", "game_spec_v2"])
        || bool_at_path(raw, &["features", "gameSpecV2"]);
    settings.custom_engine_name = string_field(raw, "custom_engine_name");
    settings.required_editor_version = string_field(raw, "required_editor_version");
    settings.development_path = string_field(raw, "development_path");
    settings.editor_path = string_field(raw, "editor_path");
    settings
}

pub fn is_unity_editor_path(path_text: &str) -> bool {
    let path_text = path_text.trim();
    if path_text.is_empty() {
        return false;
    }
    let lower_text = path_text.replace('\\', "/").to_ascii_lowercase();
    if lower_text.contains("unity hub") || lower_text.contains("unityhub") {
        return false;
    }
    let path = Path::new(path_text);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name.ends_with(".app") {
        return name.contains("unity");
    }
    name == "unity.exe" || name == "unity.app"
}

pub fn unity_project_markers(development_path: &Path) -> UnityProjectMarkers {
    UnityProjectMarkers {
        assets_dir: development_path.join("Assets").is_dir(),
        project_settings_dir: development_path.join("ProjectSettings").is_dir(),
        packages_manifest: development_path
            .join("Packages")
            .join("manifest.json")
            .is_file(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunContext {
    pub schema_version: String,
    pub run_id: String,
    pub draft_session_id: String,
    pub save_id: String,
    pub project_root: String,
    pub draft_root: String,
    pub source_artifacts_root: String,
    pub settings_snapshot: String,
    pub development_path: String,
    pub editor_path: String,
    pub project_engine: String,
    pub pipeline_adapter: String,
    pub execution_object_store: String,
    pub artifact_root: String,
    pub log_root: String,
    pub owner_pid: u32,
    pub created_at: String,
    pub isolation_policy: Value,
}

impl RunContext {
    pub fn from_value(value: &Value) -> AdmResult<Self> {
        let required = [
            "schema_version",
            "run_id",
            "draft_session_id",
            "save_id",
            "project_root",
            "draft_root",
            "source_artifacts_root",
            "settings_snapshot",
            "execution_object_store",
            "artifact_root",
            "log_root",
        ];
        let missing = required
            .iter()
            .filter(|field| string_field(value, field).is_empty())
            .map(|field| (*field).to_string())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(AdmError::new(format!(
                "run_context missing required fields: {missing:?}"
            )));
        }
        Ok(Self {
            schema_version: string_field(value, "schema_version"),
            run_id: string_field(value, "run_id"),
            draft_session_id: string_field(value, "draft_session_id"),
            save_id: string_field(value, "save_id"),
            project_root: string_field(value, "project_root"),
            draft_root: string_field(value, "draft_root"),
            source_artifacts_root: string_field(value, "source_artifacts_root"),
            settings_snapshot: string_field(value, "settings_snapshot"),
            development_path: string_field(value, "development_path"),
            editor_path: string_field(value, "editor_path"),
            project_engine: string_field(value, "project_engine")
                .trim()
                .to_ascii_lowercase()
                .if_empty("unity"),
            pipeline_adapter: string_field(value, "pipeline_adapter")
                .trim()
                .to_ascii_lowercase()
                .if_empty("none"),
            execution_object_store: string_field(value, "execution_object_store"),
            artifact_root: string_field(value, "artifact_root"),
            log_root: string_field(value, "log_root"),
            owner_pid: value.get("owner_pid").and_then(Value::as_u64).unwrap_or(0) as u32,
            created_at: string_field(value, "created_at"),
            isolation_policy: value
                .get("isolation_policy")
                .cloned()
                .unwrap_or_else(isolation_policy),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextMismatchIssue {
    pub id: String,
    pub field: String,
    pub expected: String,
    pub actual: String,
    pub source: String,
    pub message: String,
}

pub fn identity_payload(context: &RunContext) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("save_id".to_string(), context.save_id.clone()),
        ("run_id".to_string(), context.run_id.clone()),
        (
            "draft_session_id".to_string(),
            context.draft_session_id.clone(),
        ),
        (
            "development_path".to_string(),
            context.development_path.clone(),
        ),
        (
            "source_artifacts_root".to_string(),
            context.source_artifacts_root.clone(),
        ),
        (
            "execution_object_store".to_string(),
            context.execution_object_store.clone(),
        ),
    ])
}

pub fn context_mismatch_issue(
    field: &str,
    expected: String,
    actual: String,
    source: &str,
) -> ContextMismatchIssue {
    ContextMismatchIssue {
        id: "ISOLATION-CONTEXT-MISMATCH".to_string(),
        field: field.to_string(),
        expected,
        actual,
        source: source.to_string(),
        message: format!("Runtime isolation context mismatch for {field}."),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForbiddenRuntimeMatch {
    pub path: String,
    pub line: usize,
    pub text: String,
}

pub fn forbidden_runtime_matches(
    base_dir: impl AsRef<Path>,
) -> AdmResult<Vec<ForbiddenRuntimeMatch>> {
    let base_dir = base_dir.as_ref();
    let mut matches = Vec::new();
    collect_forbidden_runtime_matches(base_dir, base_dir, &mut matches)?;
    Ok(matches)
}

fn collect_forbidden_runtime_matches(
    base_dir: &Path,
    current: &Path,
    matches: &mut Vec<ForbiddenRuntimeMatch>,
) -> AdmResult<()> {
    if !current.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if !is_skipped_scan_dir(&path) {
                collect_forbidden_runtime_matches(base_dir, &path, matches)?;
            }
        } else if file_type.is_file() && is_runtime_guard_file(&path) {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            for (index, line) in text.lines().enumerate() {
                if line_matches_forbidden_runtime(line) {
                    matches.push(ForbiddenRuntimeMatch {
                        path: relative_display(&path, base_dir),
                        line: index + 1,
                        text: line.trim().to_string(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn line_matches_forbidden_runtime(line: &str) -> bool {
    let line = line.to_ascii_lowercase();
    line.contains("from crewai")
        || line.contains("import crewai")
        || line.contains("crewai_tools")
        || line.contains("crew(")
        || line.contains("agent(")
        || line.contains("task(")
        || line.contains("process.")
        || line.contains("llm(")
}

fn is_runtime_guard_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("py"))
        || name.starts_with("requirements")
}

fn is_skipped_scan_dir(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    matches!(
        name,
        ".git"
            | "__pycache__"
            | "venv"
            | ".venv"
            | "outputs"
            | "_cleanup_backup"
            | "GeneratedAssets"
            | "ArtAssets"
    )
}

fn now_runtime_timestamp() -> String {
    format!("unix:{}", unix_timestamp())
}

fn compact_timestamp() -> String {
    unix_timestamp().to_string()
}

fn step_name(step_number: u32) -> String {
    format!("Step {step_number:02}")
}

fn engine_label(engine: &str) -> &str {
    match engine {
        "unity" => "Unity",
        "unreal" => "Unreal Engine",
        "godot" => "Godot",
        "custom" => "custom",
        _ => engine,
    }
}

fn isolation_policy() -> Value {
    json!({
        "allow_cross_draft_source_fallback": false,
        "allow_global_current_save_runtime_binding": false,
        "allow_global_settings_runtime_read": false,
    })
}

fn read_json_or(path: &Path, default: Value) -> Value {
    if !path.exists() {
        return default;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return default;
    };
    let text = text.trim_start_matches('\u{feff}');
    serde_json::from_str(text).unwrap_or(default)
}

fn write_json_value(path: &Path, value: &Value) -> AdmResult<()> {
    let text = to_pretty_json(value)?;
    write_text_atomic(path, &text)
}

fn to_json_value<T: Serialize>(value: &T) -> AdmResult<Value> {
    serde_json::to_value(value)
        .map_err(|error| AdmError::new(format!("failed to serialize JSON value: {error}")))
}

fn to_pretty_json(value: &Value) -> AdmResult<String> {
    serde_json::to_string_pretty(value)
        .map_err(|error| AdmError::new(format!("failed to serialize JSON: {error}")))
}

fn object_or_empty(value: Value) -> Value {
    Value::Object(object_map_or_empty(value))
}

fn object_map_or_empty(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn bool_setting(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn bool_at_path(value: &Value, path: &[&str]) -> bool {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(*key) else {
            return false;
        };
        current = next;
    }
    current.as_bool().unwrap_or(false)
}

fn bounded_u32(value: Option<&Value>, default: u32, minimum: u32, maximum: u32) -> u32 {
    let number = value
        .and_then(Value::as_i64)
        .and_then(|number| u32::try_from(number).ok())
        .unwrap_or(default);
    number.clamp(minimum, maximum)
}

fn project_relative_path(project_root: &Path, path: &Path) -> AdmResult<String> {
    let relative = if let Ok(relative) = path.strip_prefix(project_root) {
        relative.to_path_buf()
    } else {
        let canonical_root = project_root.canonicalize().map_err(|error| {
            AdmError::new(format!(
                "runtime data root cannot be resolved at {}: {error}",
                project_root.display()
            ))
        })?;
        path.strip_prefix(&canonical_root)
            .map(Path::to_path_buf)
            .map_err(|_| {
                AdmError::new(format!(
                    "project-owned path is outside the runtime data root: {}",
                    path.display()
                ))
            })?
    };
    if relative.as_os_str().is_empty() {
        return Ok(".".to_string());
    }
    if relative.components().any(|component| {
        matches!(
            component,
            std::path::Component::ParentDir
                | std::path::Component::Prefix(_)
                | std::path::Component::RootDir
        )
    }) {
        return Err(AdmError::new(format!(
            "project-owned path is not portable: {}",
            path.display()
        )));
    }
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn resolve_run_context_paths(
    mut context: RunContext,
    project_root: &Path,
) -> AdmResult<RunContext> {
    context.project_root = resolve_project_owned_path(project_root, &context.project_root)?;
    context.draft_root = resolve_project_owned_path(project_root, &context.draft_root)?;
    context.source_artifacts_root =
        resolve_project_owned_path(project_root, &context.source_artifacts_root)?;
    context.settings_snapshot =
        resolve_project_owned_path(project_root, &context.settings_snapshot)?;
    context.execution_object_store =
        resolve_project_owned_path(project_root, &context.execution_object_store)?;
    context.artifact_root = resolve_project_owned_path(project_root, &context.artifact_root)?;
    context.log_root = resolve_project_owned_path(project_root, &context.log_root)?;
    Ok(context)
}

fn resolve_project_owned_path(project_root: &Path, relative: &str) -> AdmResult<String> {
    let relative = Path::new(relative.trim());
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::Prefix(_)
                    | std::path::Component::RootDir
            )
        })
    {
        return Err(AdmError::new(format!(
            "persisted project-owned path must be relative: {}",
            relative.display()
        )));
    }
    let candidate = if relative == Path::new(".") {
        project_root.to_path_buf()
    } else {
        project_root.join(relative)
    };
    if !path_is_inside_root(&candidate, project_root) {
        return Err(AdmError::new(format!(
            "persisted project-owned path escapes the runtime data root: {}",
            relative.display()
        )));
    }
    Ok(candidate
        .canonicalize()
        .unwrap_or(candidate)
        .display()
        .to_string())
}

fn path_is_inside_root(candidate: &Path, project_root: &Path) -> bool {
    if candidate != project_root && !candidate.starts_with(project_root) {
        return false;
    }
    let root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let mut existing = candidate;
    while !existing.exists() {
        let Some(parent) = existing.parent() else {
            return false;
        };
        existing = parent;
    }
    let resolved = existing
        .canonicalize()
        .unwrap_or_else(|_| existing.to_path_buf());
    resolved == root || resolved.starts_with(root)
}

fn ensure_steps_object(value: &mut Value) {
    let object = match value {
        Value::Object(object) => object,
        _ => {
            *value = json!({});
            value.as_object_mut().expect("object inserted")
        }
    };
    if !object.get("steps").is_some_and(Value::is_object) {
        object.insert("steps".to_string(), json!({}));
    }
}

trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.trim().is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;

    #[test]
    fn runtime_control_writes_stop_request_and_run_state() {
        let root = temp_root("control");
        let service = RuntimeApplicationService::new(&root, "session_a").unwrap();
        let run_id = new_run_id().unwrap();
        service
            .write_run_state(json!({"run_id": run_id, "status": "running"}), true, &[])
            .unwrap();
        assert_eq!(service.current_run_id(), run_id);

        let request = service.request_stop(StopRequestOptions::default()).unwrap();
        assert_eq!(request.status, "requested");
        assert!(service.stop_requested());
        service.mark_stopped(json!({"stage": "08"})).unwrap();
        assert_eq!(
            service
                .read_stop_request_value()
                .get("status")
                .and_then(Value::as_str),
            Some("handled")
        );
        service.clear_stale_stop_request("other-run").unwrap();
        assert!(!service.stop_request_path().exists());
        cleanup(root);
    }

    #[test]
    fn runtime_current_save_fallback_reads_canonical_save_index() {
        let root = temp_root("save_index");
        let service = RuntimeApplicationService::new(&root, "session_a").unwrap();
        fs::create_dir_all(root.join("saves")).unwrap();
        write_json_value(
            &root.join("saves/save_index.json"),
            &json!({"current_save_id": "save_canonical"}),
        )
        .unwrap();
        write_json_value(
            &root.join("saves/index.json"),
            &json!({"current_save_id": "save_legacy_wrong"}),
        )
        .unwrap();

        assert_eq!(
            service.bound_save_id(true).as_deref(),
            Some("save_canonical")
        );
        cleanup(root);
    }

    #[test]
    fn execution_config_planner_and_state_match_python_shapes() {
        let config = PipelineExecutionConfig::from_value(&json!({
            "pipeline": {
                "execution": {
                    "max_concurrent_dev_tasks": 99,
                    "max_concurrent_art_tasks": 0,
                    "write_conflict_policy": "serialize",
                    "group_compile_policy": "after_all"
                }
            }
        }));
        assert_eq!(config.max_concurrent_dev_tasks, 8);
        assert_eq!(config.max_concurrent_art_tasks, 1);
        assert_eq!(config.group_compile_policy, "after_all");

        let mut tasks = BTreeMap::new();
        tasks.insert("a".to_string(), json!({"output_files": ["Assets/A.cs"]}));
        tasks.insert("b".to_string(), json!({"output_files": ["Assets/B.cs"]}));
        tasks.insert(
            "c".to_string(),
            json!({"allowed_write_paths": ["Assets/A.cs"]}),
        );
        let groups = vec![ParallelGroupInput {
            group_id: "dev".to_string(),
            task_ids: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        }];
        let conflict = build_write_conflict_report(&groups, &tasks);
        assert_eq!(conflict.conflict_group_count, 1);
        assert_eq!(conflict.groups[0].conflict_batches.len(), 2);
        let readiness = build_parallel_readiness_report(&groups, &tasks, 2);
        assert!(readiness.ready_for_parallel_execution);

        let state = ThreadSafeStageExecutionState::default();
        assert_eq!(state.append_execution_record(json!({"task_id": "a"})), 0);
        assert_eq!(state.append_package_report(json!({"status": "ok"})), 0);
        let snapshot = state.snapshot();
        assert_eq!(snapshot.execution_records[0]["task_id"], "a");
        assert_eq!(snapshot.package_reports[0]["status"], "ok");
    }

    #[test]
    fn runtime_locks_are_exclusive_and_release_by_owner() {
        let root = temp_root("locks");
        let mut lock = acquire_save_lock(&root, "save_a", "run_a", "session_a").unwrap();
        let second = acquire_save_lock(&root, "save_a", "run_b", "session_b");
        assert!(second.is_err());
        lock.release().unwrap();
        let _third = acquire_save_lock(&root, "save_a", "run_c", "session_c").unwrap();
        cleanup(root);
    }

    #[test]
    fn preflight_run_context_identity_and_pipeline_state_round_trip() {
        let root = temp_root("context");
        let service = RuntimeApplicationService::new(&root, "session_a").unwrap();
        let custom_settings = ProjectRuntimeSettings {
            project_engine: "custom".to_string(),
            development_path: "missing".to_string(),
            ..ProjectRuntimeSettings::default()
        };
        service.save_project_settings(&custom_settings).unwrap();
        let report = service
            .run_actual_development_preflight(true, false)
            .unwrap();
        assert_eq!(report.status, "passed");
        assert_eq!(report.warnings.len(), 1);
        let persisted_preflight = fs::read_to_string(service.preflight_report_path()).unwrap();
        assert!(!persisted_preflight.contains(&root.to_string_lossy().to_string()));
        let persisted_preflight: Value = serde_json::from_str(&persisted_preflight).unwrap();
        assert_eq!(persisted_preflight["settings"]["development_path"], "");
        assert_eq!(persisted_preflight["settings"]["editor_path"], "");

        let context = service
            .create_run_context("run_a", "save_a", Some(custom_settings), true, false)
            .unwrap();
        assert_eq!(context.save_id, "save_a");
        let persisted_context = fs::read_to_string(service.run_context_path()).unwrap();
        assert!(!persisted_context.contains(&root.to_string_lossy().to_string()));
        let persisted_context: Value = serde_json::from_str(&persisted_context).unwrap();
        assert_eq!(persisted_context["schema_version"], "2.0");
        assert_eq!(persisted_context["project_root"], ".");
        assert_eq!(
            persisted_context["source_artifacts_root"],
            "drafts/session_a/source_artifacts"
        );
        assert_eq!(
            identity_payload(&context).get("run_id").map(String::as_str),
            Some("run_a")
        );
        let issues = service.compare_identity(
            &json!({"save_id": "save_b", "run_id": "run_a"}),
            Some(&context),
            "artifact.json",
            &["save_id", "run_id"],
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].field, "save_id");

        service
            .update_step_state(
                8,
                "in_progress",
                Some("snap"),
                Some("out.json"),
                Some("running"),
            )
            .unwrap();
        let step = service.get_step_state(8).unwrap().unwrap();
        assert_eq!(step["status"], "in_progress");
        assert!(
            service
                .update_step_state(8, "bad", None, None, None)
                .is_err()
        );
        cleanup(root);
    }

    #[test]
    fn copied_run_context_is_rebased_without_reading_the_existing_old_root() {
        let old_root = temp_root("context_old_root");
        let new_root = temp_root("context_new_root_中文 空格");
        let old_service = RuntimeApplicationService::new(&old_root, "session_a").unwrap();
        old_service
            .create_run_context("run_a", "save_a", None, true, false)
            .unwrap();
        write_json_value(
            &old_service.settings_snapshot_path(),
            &json!({"sentinel": "old-root"}),
        )
        .unwrap();

        let new_service = RuntimeApplicationService::new(&new_root, "session_a").unwrap();
        fs::create_dir_all(new_service.run_context_path().parent().unwrap()).unwrap();
        fs::copy(
            old_service.run_context_path(),
            new_service.run_context_path(),
        )
        .unwrap();
        write_json_value(
            &new_service.settings_snapshot_path(),
            &json!({"sentinel": "new-root"}),
        )
        .unwrap();

        let context = new_service.load_run_context(true).unwrap().unwrap();
        assert_eq!(
            PathBuf::from(&context.project_root),
            new_root.canonicalize().unwrap()
        );
        assert!(
            !context
                .project_root
                .contains(&old_root.to_string_lossy().to_string())
        );
        assert_eq!(
            new_service.load_settings_snapshot().unwrap().unwrap()["sentinel"],
            "new-root"
        );
        let migrated = fs::read_to_string(new_service.run_context_path()).unwrap();
        assert!(!migrated.contains(&old_root.to_string_lossy().to_string()));
        assert!(!migrated.contains(&new_root.to_string_lossy().to_string()));

        cleanup(old_root);
        cleanup(new_root);
    }

    #[test]
    fn unity_preflight_helpers_and_runtime_guard_match_python_policy() {
        let root = temp_root("preflight");
        let unity_project = root.join("UnityProject");
        fs::create_dir_all(unity_project.join("ProjectSettings")).unwrap();
        fs::create_dir_all(unity_project.join("Packages")).unwrap();
        fs::write(unity_project.join("Packages").join("manifest.json"), "{}").unwrap();
        let service = RuntimeApplicationService::new(&root, "session_a").unwrap();
        service
            .save_project_settings(&ProjectRuntimeSettings {
                development_path: "UnityProject".to_string(),
                editor_path: "C:/Program Files/Unity Hub/Unity Hub.exe".to_string(),
                ..ProjectRuntimeSettings::default()
            })
            .unwrap();
        let report = service
            .run_actual_development_preflight(false, false)
            .unwrap();
        assert_eq!(report.status, "blocked");
        assert!(report.warnings.iter().any(|item| item.contains("Assets")));
        assert!(
            report
                .diagnostics
                .iter()
                .any(|item| item.error_code == "invalid_unity_editor_executable"
                    && item.fix_action == "reselect_editor_path")
        );
        assert!(is_unity_editor_path(
            "C:/Program Files/Unity/Editor/Unity.exe"
        ));
        assert!(!is_unity_editor_path(
            "C:/Program Files/Unity Hub/Unity Hub.exe"
        ));
        assert!(!is_unity_editor_path("C:/Unity/notepad.exe"));

        fs::write(root.join("bad.py"), "from crewai import Agent\n").unwrap();
        let matches = forbidden_runtime_matches(&root).unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "bad.py");
        cleanup(root);
    }

    #[test]
    fn moved_project_relink_updates_only_machine_binding_and_rejects_conflicts() {
        let root = temp_root("project_relink");
        let old_project = root.join("Projects/Old");
        let moved_project = root.join("Moved/Project");
        let editor = root.join("Unity/2022.3.21f1/Editor/Unity.exe");
        let invalid_editor = root.join("Unity/2022.3.21f1/Editor/notepad.exe");
        for project in [&old_project, &moved_project] {
            fs::create_dir_all(project.join("Assets")).unwrap();
            fs::create_dir_all(project.join("ProjectSettings")).unwrap();
            fs::create_dir_all(project.join("Packages")).unwrap();
            fs::write(project.join("Packages/manifest.json"), "{}").unwrap();
            fs::write(
                project.join("ProjectSettings/ProjectVersion.txt"),
                "m_EditorVersion: 2022.3.21f1\n",
            )
            .unwrap();
        }
        fs::create_dir_all(editor.parent().unwrap()).unwrap();
        fs::write(&editor, "fixture").unwrap();
        fs::write(&invalid_editor, "fixture").unwrap();

        let service = RuntimeApplicationService::new(&root, "session_a").unwrap();
        fs::write(
            service.active_project_settings_path(),
            r#"{"schema_version":1,"project_label":"keep me","future_field":{"keep":true}}"#,
        )
        .unwrap();
        service
            .save_project_settings(&ProjectRuntimeSettings {
                project_engine: "unity".to_string(),
                development_path: old_project.display().to_string(),
                editor_path: editor.display().to_string(),
                ..ProjectRuntimeSettings::default()
            })
            .unwrap();
        let logical_before = fs::read_to_string(service.active_project_settings_path()).unwrap();
        let settings = service.load_project_settings(false);
        fs::remove_dir_all(&old_project).unwrap();

        let relinked = service
            .relink_project_binding(
                &settings,
                &moved_project.display().to_string(),
                &editor.display().to_string(),
            )
            .unwrap();
        assert_eq!(
            relinked.development_path,
            moved_project.display().to_string()
        );
        assert_eq!(
            fs::read_to_string(service.active_project_settings_path()).unwrap(),
            logical_before
        );
        let effective = service.load_project_settings(false);
        assert_eq!(
            effective.development_path,
            moved_project.display().to_string()
        );
        assert_eq!(
            serde_json::from_str::<Value>(&logical_before).unwrap()["project_label"],
            "keep me"
        );

        let invalid = service.relink_project_binding(
            &settings,
            &moved_project.display().to_string(),
            &invalid_editor.display().to_string(),
        );
        assert!(invalid.is_err());
        assert_eq!(
            service.load_project_settings(false).editor_path,
            editor.display().to_string()
        );

        let mut wrong_binding = settings.clone();
        wrong_binding.binding_id = "another_binding".to_string();
        assert!(
            service
                .relink_project_binding(
                    &wrong_binding,
                    &moved_project.display().to_string(),
                    &editor.display().to_string(),
                )
                .is_err()
        );

        let godot = root.join("GodotProject");
        fs::create_dir_all(&godot).unwrap();
        fs::write(godot.join("project.godot"), "[application]").unwrap();
        assert!(
            service
                .relink_project_binding(
                    &settings,
                    &godot.display().to_string(),
                    &editor.display().to_string(),
                )
                .is_err()
        );
        cleanup(root);
    }

    #[test]
    fn project_binding_v2_keeps_machine_paths_out_of_the_archived_project_document() {
        let root = temp_root("project_binding_v2");
        let project = root.join("Projects/演示 Unity 项目");
        let editor = root.join("Unity/2022.3.21f1/Editor/Unity.exe");
        fs::create_dir_all(project.join("Assets")).unwrap();
        fs::create_dir_all(project.join("ProjectSettings")).unwrap();
        fs::create_dir_all(project.join("Packages")).unwrap();
        fs::create_dir_all(editor.parent().unwrap()).unwrap();
        fs::write(project.join("Packages/manifest.json"), "{}").unwrap();
        fs::write(
            project.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.21f1\n",
        )
        .unwrap();
        fs::write(&editor, "fixture").unwrap();
        let service = RuntimeApplicationService::new(&root, "session_a").unwrap();
        fs::write(
            service.active_project_settings_path(),
            r#"{"schema_version":1,"future_field":{"keep":true}}"#,
        )
        .unwrap();

        service
            .save_project_settings(&ProjectRuntimeSettings {
                project_engine: "unity".to_string(),
                development_path: project.display().to_string(),
                editor_path: editor.display().to_string(),
                ..ProjectRuntimeSettings::default()
            })
            .unwrap();

        let project_document = fs::read_to_string(service.active_project_settings_path()).unwrap();
        assert!(!project_document.contains(&project.display().to_string()));
        assert!(!project_document.contains(&editor.display().to_string()));
        let project_document: Value = serde_json::from_str(&project_document).unwrap();
        assert_eq!(project_document["schema_version"], 2);
        assert_eq!(project_document["future_field"]["keep"], true);
        assert!(project_document.get("development_path").is_none());
        assert!(project_document.get("editor_path").is_none());

        let effective = service.load_project_settings(false);
        assert_eq!(effective.development_path, project.display().to_string());
        assert_eq!(effective.editor_path, editor.display().to_string());
        assert_eq!(effective.required_editor_version, "2022.3.21f1");
        assert!(!effective.binding_id.is_empty());
        assert!(service.project_bindings_path().is_file());

        service
            .create_run_context("run_a", "save_a", Some(effective.clone()), true, false)
            .unwrap();
        let snapshot_path = service.settings_snapshot_path();
        let mut snapshot = read_json_or(&snapshot_path, json!({}));
        snapshot["development_path"] = Value::String(r"C:\stale-machine\project".to_string());
        snapshot["editor_path"] = Value::String(r"C:\stale-machine\Unity.exe".to_string());
        write_json_value(&snapshot_path, &snapshot).unwrap();
        let rebound_snapshot = service.load_project_settings(true);
        assert_eq!(
            rebound_snapshot.development_path,
            project.display().to_string()
        );
        assert_eq!(rebound_snapshot.editor_path, editor.display().to_string());
        assert_eq!(
            service
                .run_actual_development_preflight(false, true)
                .unwrap()
                .status,
            "passed"
        );

        fs::remove_file(service.project_bindings_path()).unwrap();
        let unbound = service.load_project_settings(false);
        assert!(unbound.development_path.is_empty());
        assert!(unbound.editor_path.is_empty());
        let unbound_snapshot = service.load_project_settings(true);
        assert!(unbound_snapshot.development_path.is_empty());
        assert!(unbound_snapshot.editor_path.is_empty());
        cleanup(root);
    }

    #[test]
    fn project_settings_persist_game_spec_v2_switch_without_machine_paths() {
        let root = temp_root("project_config_game_spec_v2");
        let service = RuntimeApplicationService::new(&root, "session_a").unwrap();
        fs::write(
            service.active_project_settings_path(),
            r#"{"schema_version":1,"future_field":{"keep":true}}"#,
        )
        .unwrap();

        service
            .save_project_settings(&ProjectRuntimeSettings {
                project_engine: "custom".to_string(),
                game_spec_v2_enabled: true,
                development_path: "local-machine-project".to_string(),
                ..ProjectRuntimeSettings::default()
            })
            .unwrap();

        let project_document: Value = serde_json::from_str(
            &fs::read_to_string(service.active_project_settings_path()).unwrap(),
        )
        .unwrap();
        assert_eq!(project_document["future_field"]["keep"], true);
        assert_eq!(project_document["game_spec_v2_enabled"], true);
        assert_eq!(project_document["game_spec_v2"]["enabled"], true);
        assert!(project_document.get("development_path").is_none());
        assert!(service.load_project_settings(false).game_spec_v2_enabled);
        assert!(
            normalize_project_settings(&json!({"game_spec_v2": {"enabled": true}}))
                .game_spec_v2_enabled
        );
        assert!(
            normalize_project_settings(&json!({"gameSpecV2Enabled": true})).game_spec_v2_enabled
        );

        cleanup(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm_new_runtime_{label}_{}",
            new_stable_id("root").unwrap()
        ))
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
