use adm_new_foundation::{AdmError, AdmResult, io, paths::relative_display};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const UCOS_ROOT: &str = "knowledge/ucos";

#[derive(Debug, Clone)]
pub struct UcosStore {
    project_root: PathBuf,
    ucos_root: PathBuf,
}

impl UcosStore {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        let project_root = project_root.into();
        let knowledge_root = project_root.join(UCOS_ROOT);
        let ucos_root = if knowledge_root.exists() {
            knowledge_root
        } else {
            project_root.join("ucos")
        };
        Self {
            project_root,
            ucos_root,
        }
    }

    pub fn from_ucos_root(project_root: impl Into<PathBuf>, ucos_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
            ucos_root: ucos_root.into(),
        }
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn ucos_root(&self) -> &Path {
        &self.ucos_root
    }

    pub fn resolve(&self, relative_path: impl AsRef<Path>) -> PathBuf {
        self.ucos_root.join(relative_path)
    }

    pub fn relative_to_project(&self, path: &Path) -> String {
        relative_display(path, &self.project_root)
    }

    pub fn read_json(&self, relative_path: impl AsRef<Path>, default: Value) -> Value {
        io::read_json(&self.resolve(relative_path), default)
    }

    pub fn write_json<T: Serialize>(
        &self,
        relative_path: impl AsRef<Path>,
        value: &T,
    ) -> AdmResult<PathBuf> {
        io::write_json_serializable(&self.resolve(relative_path), value)
    }

    pub fn read_required_json(&self, relative_path: impl AsRef<Path>) -> AdmResult<Value> {
        let path = self.resolve(relative_path);
        let text = std::fs::read_to_string(&path)?;
        serde_json::from_str(&text).map_err(|error| {
            AdmError::new(format!("failed to parse json {}: {error}", path.display()))
        })
    }

    pub fn json_files(
        &self,
        relative_path: impl AsRef<Path>,
        recursive: bool,
    ) -> AdmResult<Vec<PathBuf>> {
        let mut paths = Vec::new();
        collect_json_files(&self.resolve(relative_path), recursive, &mut paths)?;
        paths.sort();
        Ok(paths)
    }

    pub fn inventory(&self) -> AdmResult<UcosInventory> {
        let schema_files = self.json_files("schemas", false)?.len();
        let identity_files = self.json_files("identity", false)?.len();
        let capability_skill_files = self.json_files("capability/skills", true)?.len();
        let plugin_skill_files = self
            .json_files("plugins", true)?
            .into_iter()
            .filter(|path| {
                path.components()
                    .any(|component| component.as_os_str().to_string_lossy() == "skills")
            })
            .count();
        let episode_files = self.json_files("knowledge/episodic/episodes", false)?.len();
        let turn_files = self.json_files("knowledge/episodic/turns", true)?.len();
        let semantic_fact_files = self.json_files("knowledge/semantic/facts", false)?.len();
        let short_term_files = self
            .json_files("knowledge/short_term/entries", false)?
            .len();
        let pattern_files = self.json_files("knowledge/patterns/entries", false)?.len();
        let failure_files = self.json_files("knowledge/failures/entries", false)?.len();
        let registry = self.read_json("capability/registry.json", json!({}));
        let registry_skill_count = registry
            .get("skills")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        Ok(UcosInventory {
            ucos_root: self.relative_to_project(&self.ucos_root),
            schema_files,
            identity_files,
            capability_skill_files,
            plugin_skill_files,
            registry_skill_count,
            episode_files,
            turn_files,
            semantic_fact_files,
            short_term_files,
            pattern_files,
            failure_files,
            has_identity_profile: self.resolve("identity/profile.json").exists(),
            has_identity_constraints: self.resolve("identity/constraints.json").exists(),
            has_identity_policy: self.resolve("identity/policy.json").exists(),
            has_capability_registry: self.resolve("capability/registry.json").exists(),
            has_dependency_graph: self.resolve("capability/dependency_graph.json").exists(),
            has_episodic_index: self.resolve("knowledge/episodic/index.json").exists(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UcosInventory {
    pub ucos_root: String,
    pub schema_files: usize,
    pub identity_files: usize,
    pub capability_skill_files: usize,
    pub plugin_skill_files: usize,
    pub registry_skill_count: usize,
    pub episode_files: usize,
    pub turn_files: usize,
    pub semantic_fact_files: usize,
    pub short_term_files: usize,
    pub pattern_files: usize,
    pub failure_files: usize,
    pub has_identity_profile: bool,
    pub has_identity_constraints: bool,
    pub has_identity_policy: bool,
    pub has_capability_registry: bool,
    pub has_dependency_graph: bool,
    pub has_episodic_index: bool,
}

impl UcosInventory {
    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.schema_files < 10 {
            errors.push(format!(
                "expected at least 10 UCOS schemas, got {}",
                self.schema_files
            ));
        }
        if !self.has_identity_profile {
            errors.push("missing identity/profile.json".to_string());
        }
        if !self.has_identity_constraints {
            errors.push("missing identity/constraints.json".to_string());
        }
        if !self.has_identity_policy {
            errors.push("missing identity/policy.json".to_string());
        }
        if !self.has_capability_registry {
            errors.push("missing capability/registry.json".to_string());
        }
        if self.registry_skill_count == 0 {
            errors.push("capability registry has no skills".to_string());
        }
        if self.capability_skill_files + self.plugin_skill_files < self.registry_skill_count {
            errors.push(format!(
                "skill file count lower than registry: files={}, registry={}",
                self.capability_skill_files + self.plugin_skill_files,
                self.registry_skill_count
            ));
        }
        errors
    }
}

fn collect_json_files(dir: &Path, recursive: bool, paths: &mut Vec<PathBuf>) -> AdmResult<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() && recursive {
            collect_json_files(&path, recursive, paths)?;
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("json")
        {
            paths.push(path);
        }
    }
    Ok(())
}
