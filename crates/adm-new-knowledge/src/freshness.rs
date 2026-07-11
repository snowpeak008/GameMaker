use adm_new_foundation::{AdmError, AdmResult, io, sha256_hex, unix_timestamp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const FRESHNESS_PATH: &str = "knowledge/ai_memory/project_understanding/freshness.json";

pub const KEY_FILES: [&str; 34] = [
    "core/main.py",
    "core/engines/generation.py",
    "core/registry.py",
    "core/paths.py",
    "core/plugin_manager.py",
    "core/stage_plugin.py",
    "core/context.py",
    "core/io.py",
    "core/stage.py",
    "core/adapters/base.py",
    "core/adapters/registry.py",
    "core/adapters/openai_adapter.py",
    "core/adapters/codex_adapter.py",
    "core/artifact/graph.py",
    "core/artifact/preflight.py",
    "core/artifact/reviewer.py",
    "core/artifact/validator.py",
    "core/source/importer.py",
    "core/source/groups.py",
    "core/config/loader.py",
    "core/config/integrity.py",
    "core/runtime/control.py",
    "core/runtime/preflight.py",
    "core/runtime/pipeline_state.py",
    "core/save/manager.py",
    "core/design/engine.py",
    "core/design/exporter.py",
    "core/design/ai_backend.py",
    "core/ui/gui_app.py",
    "core/ui/app_window.py",
    "core/ui/theme.py",
    "core/ui/ai_interview_window.py",
    "pipeline/_registry.json",
    "pipeline/artifact_layer/registry.json",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileFreshness {
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshnessSnapshot {
    pub generated_at: String,
    pub files: BTreeMap<String, FileFreshness>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StalenessReport {
    pub stale: Vec<String>,
    pub fresh: Vec<String>,
    pub missing: Vec<String>,
    pub generated_at: String,
}

impl StalenessReport {
    pub fn is_clean(&self) -> bool {
        self.stale.is_empty() && self.missing.is_empty()
    }
}

pub fn compute_file_hash(path: &Path) -> AdmResult<FileFreshness> {
    let content = std::fs::read(path)?;
    let size = content.len() as u64;
    Ok(FileFreshness {
        sha256: sha256_hex(&content),
        size,
    })
}

pub fn build_snapshot(project_root: &Path) -> AdmResult<(FreshnessSnapshot, Vec<String>)> {
    let mut files = BTreeMap::new();
    let mut missing = Vec::new();
    for rel_path in KEY_FILES {
        let full_path = project_root.join(rel_path);
        if full_path.exists() {
            files.insert(rel_path.to_string(), compute_file_hash(&full_path)?);
        } else {
            missing.push(rel_path.to_string());
        }
    }
    Ok((
        FreshnessSnapshot {
            generated_at: format!("unix:{}", unix_timestamp()),
            files,
        },
        missing,
    ))
}

pub fn update_freshness(project_root: &Path) -> AdmResult<(FreshnessSnapshot, Vec<String>)> {
    let (snapshot, missing) = build_snapshot(project_root)?;
    io::write_json_serializable(&project_root.join(FRESHNESS_PATH), &snapshot)?;
    Ok((snapshot, missing))
}

pub fn check_staleness(project_root: &Path) -> AdmResult<StalenessReport> {
    let path = project_root.join(FRESHNESS_PATH);
    if !path.exists() {
        return Err(AdmError::new("freshness.json not found"));
    }
    let text = std::fs::read_to_string(&path)?;
    let snapshot: FreshnessSnapshot = serde_json::from_str(&text)
        .map_err(|error| AdmError::new(format!("failed to parse freshness snapshot: {error}")))?;
    let mut stale = Vec::new();
    let mut fresh = Vec::new();
    let mut missing = Vec::new();
    for (rel_path, cached) in &snapshot.files {
        let full_path = project_root.join(rel_path);
        if !full_path.exists() {
            missing.push(rel_path.clone());
            continue;
        }
        let current = compute_file_hash(&full_path)?;
        if current.sha256 != cached.sha256 {
            stale.push(rel_path.clone());
        } else {
            fresh.push(rel_path.clone());
        }
    }
    Ok(StalenessReport {
        stale,
        fresh,
        missing,
        generated_at: snapshot.generated_at,
    })
}

pub fn freshness_path(project_root: &Path) -> PathBuf {
    project_root.join(FRESHNESS_PATH)
}
