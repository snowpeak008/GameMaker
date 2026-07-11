use crate::{AdmError, AdmResult, sanitize_identifier, unix_timestamp};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

pub const ROOT_MARKER: &str = ".project_root";
pub const SESSION_ID_ENV: &str = "AUTODESIGNMAKER_SESSION_ID";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectPaths {
    pub project_root: PathBuf,
    pub core_dir: PathBuf,
    pub pipeline_dir: PathBuf,
    pub knowledge_dir: PathBuf,
    pub schemas_dir: PathBuf,
    pub skills_dir: PathBuf,
    pub design_data_dir: PathBuf,
    pub settings_dir: PathBuf,
    pub app_config_file: PathBuf,
    pub project_settings_file: PathBuf,
    pub api_config_file: PathBuf,
    pub plugin_manifest_file: PathBuf,
    pub session_id: String,
    pub drafts_dir: PathBuf,
    pub draft_dir: PathBuf,
    pub draft_meta_file: PathBuf,
    pub source_artifacts_dir: PathBuf,
    pub outputs_dir: PathBuf,
    pub artifacts_dir: PathBuf,
    pub checkpoints_dir: PathBuf,
    pub runtime_control_dir: PathBuf,
    pub run_logs_dir: PathBuf,
    pub iteration_specs_dir: PathBuf,
    pub patches_dir: PathBuf,
    pub sdk_knowledge_dir: PathBuf,
    pub saves_dir: PathBuf,
    pub workspace_dir: PathBuf,
    pub workspace_projects_dir: PathBuf,
    pub workspace_exports_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub ucos_dir: PathBuf,
    pub memory_dir: PathBuf,
    pub docs_dir: PathBuf,
    pub scripts_dir: PathBuf,
    pub tests_dir: PathBuf,
    pub archive_dir: PathBuf,
}

impl ProjectPaths {
    pub fn new(project_root: impl Into<PathBuf>, session_id: impl Into<String>) -> Self {
        let project_root = project_root.into();
        let session_id = session_id.into();
        let core_dir = project_root.join("core");
        let pipeline_dir = project_root.join("pipeline");
        let knowledge_dir = project_root.join("knowledge");
        let settings_dir = project_root.join("settings");
        let drafts_dir = project_root.join("drafts");
        let draft_dir = drafts_dir.join(&session_id);
        let outputs_dir = draft_dir.join("outputs");
        let workspace_dir = draft_dir.join("workspace");

        Self {
            project_root: project_root.clone(),
            core_dir: core_dir.clone(),
            pipeline_dir: pipeline_dir.clone(),
            knowledge_dir: knowledge_dir.clone(),
            schemas_dir: knowledge_dir.join("schemas"),
            skills_dir: knowledge_dir.join("skills"),
            design_data_dir: knowledge_dir.join("design_data"),
            settings_dir: settings_dir.clone(),
            app_config_file: settings_dir.join("app.toml"),
            project_settings_file: settings_dir.join("project_settings.json"),
            api_config_file: settings_dir.join("api_config.toml"),
            plugin_manifest_file: pipeline_dir.join("_registry.json"),
            session_id,
            drafts_dir,
            draft_meta_file: draft_dir.join("draft_meta.json"),
            source_artifacts_dir: draft_dir.join("source_artifacts"),
            artifacts_dir: outputs_dir.join("artifacts"),
            checkpoints_dir: outputs_dir.join("checkpoints"),
            runtime_control_dir: outputs_dir.join("runtime_control"),
            run_logs_dir: outputs_dir.join("run_logs"),
            iteration_specs_dir: draft_dir.join("iteration_specs"),
            patches_dir: draft_dir.join("patches"),
            sdk_knowledge_dir: knowledge_dir.join("sdks"),
            saves_dir: project_root.join("saves"),
            workspace_projects_dir: workspace_dir.join("projects"),
            workspace_exports_dir: workspace_dir.join("exports"),
            logs_dir: project_root.join("logs"),
            ucos_dir: knowledge_dir.join("ucos"),
            memory_dir: project_root.join("memory"),
            docs_dir: project_root.join("docs"),
            scripts_dir: project_root.join("scripts"),
            tests_dir: core_dir.join("tests"),
            archive_dir: project_root.join("_archive"),
            draft_dir,
            outputs_dir,
            workspace_dir,
        }
    }

    pub fn from_root(project_root: impl Into<PathBuf>) -> Self {
        Self::new(project_root, session_id_from_env(None))
    }

    pub fn ensure_current_draft_dirs(&self) -> AdmResult<()> {
        for path in [
            &self.source_artifacts_dir,
            &self.artifacts_dir,
            &self.checkpoints_dir,
            &self.runtime_control_dir,
            &self.run_logs_dir,
            &self.iteration_specs_dir,
            &self.patches_dir,
            &self.workspace_projects_dir,
            &self.workspace_exports_dir,
        ] {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }

    pub fn stage_artifact_dir(&self, stage_id: &str) -> AdmResult<PathBuf> {
        let safe_stage = sanitize_identifier(&stage_id.to_lowercase().replace(' ', "_"))?;
        let path = self.artifacts_dir.join(format!("stage_{safe_stage}"));
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }
}

pub fn locate_project_root(start_path: impl AsRef<Path>) -> AdmResult<PathBuf> {
    let start_path = start_path.as_ref();
    let mut current = start_path
        .canonicalize()
        .unwrap_or_else(|_| start_path.to_path_buf());
    if current.is_file() {
        current = current
            .parent()
            .ok_or_else(|| AdmError::new("start path has no parent"))?
            .to_path_buf();
    }
    loop {
        if current.join(ROOT_MARKER).exists() {
            return Ok(current);
        }
        let Some(parent) = current.parent() else {
            return Err(AdmError::new(format!(
                "unable to locate project root: {ROOT_MARKER} was not found from {}",
                start_path.display()
            )));
        };
        if parent == current {
            return Err(AdmError::new(format!(
                "unable to locate project root: {ROOT_MARKER} was not found from {}",
                start_path.display()
            )));
        }
        current = parent.to_path_buf();
    }
}

pub fn pycache_prefix(project_root: &Path, env_value: Option<&str>) -> Option<PathBuf> {
    if !project_root.join(ROOT_MARKER).exists() {
        return None;
    }
    let value = env_value
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| project_root.join(".cache").join("pycache"));
    Some(value)
}

pub fn session_id_from_env(value: Option<&str>) -> String {
    let env_value = value
        .map(str::to_string)
        .or_else(|| env::var(SESSION_ID_ENV).ok())
        .unwrap_or_default();
    let env_value = env_value.trim();
    if env_value.is_empty() {
        format!("{}_{}", unix_timestamp(), std::process::id())
    } else {
        env_value.to_string()
    }
}

pub fn project_path(root: &Path, relative_path: impl AsRef<Path>) -> PathBuf {
    root.join(relative_path)
}

pub fn get_draft_dir(root: &Path, session_id: &str) -> PathBuf {
    root.join("drafts").join(session_id)
}

pub fn resolve_configured_path(value: impl AsRef<Path>, base: &Path) -> PathBuf {
    let path = value.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

pub fn relative_display(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_stable_id;

    #[test]
    fn project_root_walks_up_to_marker() {
        let root = std::env::temp_dir().join(new_stable_id("project_root").unwrap());
        let nested = root.join("core").join("utils");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join(ROOT_MARKER), "").unwrap();

        assert_eq!(
            locate_project_root(&nested).unwrap(),
            root.canonicalize().unwrap()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn pycache_prefix_uses_project_cache_by_default() {
        let root = std::env::temp_dir().join(new_stable_id("pycache").unwrap());
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(ROOT_MARKER), "").unwrap();

        assert_eq!(
            pycache_prefix(&root, None).unwrap(),
            root.join(".cache").join("pycache")
        );
        assert_eq!(
            pycache_prefix(&root, Some("custom")).unwrap(),
            PathBuf::from("custom")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn project_paths_match_python_draft_policy() {
        let root = std::env::temp_dir().join(new_stable_id("project_paths").unwrap());
        let paths = ProjectPaths::new(&root, "session-1");

        assert_eq!(paths.draft_dir, root.join("drafts/session-1"));
        assert_eq!(
            paths.source_artifacts_dir,
            root.join("drafts/session-1/source_artifacts")
        );
        assert_eq!(
            paths.stage_artifact_dir("Step 00 Idea").unwrap(),
            root.join("drafts/session-1/outputs/artifacts/stage_step_00_idea")
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
