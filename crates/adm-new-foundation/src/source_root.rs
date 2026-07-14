use crate::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const ROOT_MARKER: &str = ".project_root";
pub const SOURCE_PROJECT_ID: &str = "autodesignmaker-rust-v2";
const SOURCE_PROJECT_KIND: &str = "source-project-root";
const SOURCE_PROJECT_SCHEMA_VERSION: u32 = 1;
const REQUIRED_SOURCE_LOCKFILES: [&str; 2] = ["Cargo.lock", "web/package-lock.json"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProjectManifest {
    pub schema_version: u32,
    pub kind: String,
    pub project_id: String,
    pub workspace_manifest: String,
    pub lockfiles: Vec<String>,
    pub resource_manifest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProjectRoot {
    root: PathBuf,
    manifest: SourceProjectManifest,
}

impl SourceProjectRoot {
    pub fn discover(start_path: impl AsRef<Path>) -> AdmResult<Self> {
        let start_path = start_path.as_ref();
        let mut current = normalize_start_directory(start_path)?;
        loop {
            if path_entry_exists(&current.join(ROOT_MARKER))? {
                return Self::open(&current);
            }
            let Some(parent) = current.parent() else {
                return Err(project_root_not_found(start_path));
            };
            if parent == current {
                return Err(project_root_not_found(start_path));
            }
            current = parent.to_path_buf();
        }
    }

    pub fn open(root: impl AsRef<Path>) -> AdmResult<Self> {
        let root = root.as_ref().canonicalize().map_err(|error| {
            AdmError::new(format!(
                "unable to canonicalize source project root {}: {error}",
                root.as_ref().display()
            ))
        })?;
        if !root.is_dir() {
            return Err(AdmError::new(format!(
                "source project root is not a directory: {}",
                root.display()
            )));
        }

        let marker_path = root.join(ROOT_MARKER);
        let marker_metadata = fs::symlink_metadata(&marker_path).map_err(|error| {
            AdmError::new(format!(
                "source project root marker is missing at {}: {error}",
                marker_path.display()
            ))
        })?;
        if !marker_metadata.is_file() || marker_metadata.file_type().is_symlink() {
            return Err(AdmError::new(format!(
                "source project root marker must be a regular file: {}",
                marker_path.display()
            )));
        }

        let manifest: SourceProjectManifest =
            serde_json::from_slice(&fs::read(&marker_path).map_err(|error| {
                AdmError::new(format!(
                    "failed to read source project root marker {}: {error}",
                    marker_path.display()
                ))
            })?)
            .map_err(|error| {
                AdmError::new(format!(
                    "invalid source project root marker {}: {error}",
                    marker_path.display()
                ))
            })?;
        validate_source_manifest(&root, &manifest)?;

        Ok(Self { root, manifest })
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn into_path(self) -> PathBuf {
        self.root
    }

    pub fn manifest(&self) -> &SourceProjectManifest {
        &self.manifest
    }

    pub fn join(&self, relative_path: impl AsRef<Path>) -> AdmResult<PathBuf> {
        safe_project_join(&self.root, relative_path)
    }
}

pub fn safe_project_join(
    project_root: impl AsRef<Path>,
    relative_path: impl AsRef<Path>,
) -> AdmResult<PathBuf> {
    let project_root = project_root.as_ref().canonicalize().map_err(|error| {
        AdmError::new(format!(
            "unable to canonicalize project root {}: {error}",
            project_root.as_ref().display()
        ))
    })?;
    let relative_path = relative_path.as_ref();
    if relative_path.as_os_str().is_empty()
        || relative_path.is_absolute()
        || relative_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        return Err(AdmError::new(format!(
            "path must be a non-empty portable project-relative path: {}",
            relative_path.display()
        )));
    }

    let candidate = project_root.join(relative_path);
    let mut existing_ancestor = candidate.as_path();
    while !path_entry_exists(existing_ancestor)? {
        existing_ancestor = existing_ancestor.parent().ok_or_else(|| {
            AdmError::new(format!(
                "path has no existing ancestor inside project root: {}",
                candidate.display()
            ))
        })?;
    }
    let canonical_ancestor = existing_ancestor.canonicalize().map_err(|error| {
        AdmError::new(format!(
            "unable to canonicalize project path ancestor {}: {error}",
            existing_ancestor.display()
        ))
    })?;
    if !canonical_ancestor.starts_with(&project_root) {
        return Err(AdmError::new(format!(
            "project path escapes through an external link: {}",
            candidate.display()
        )));
    }

    if path_entry_exists(&candidate)? {
        let canonical_candidate = candidate.canonicalize().map_err(|error| {
            AdmError::new(format!(
                "unable to canonicalize project path {}: {error}",
                candidate.display()
            ))
        })?;
        if !canonical_candidate.starts_with(&project_root) {
            return Err(AdmError::new(format!(
                "project path resolves outside project root: {}",
                candidate.display()
            )));
        }
    }
    Ok(candidate)
}

fn path_entry_exists(path: &Path) -> AdmResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AdmError::new(format!(
            "unable to inspect source project path entry {}: {error}",
            path.display()
        ))),
    }
}

fn validate_source_manifest(root: &Path, manifest: &SourceProjectManifest) -> AdmResult<()> {
    if manifest.schema_version != SOURCE_PROJECT_SCHEMA_VERSION {
        return Err(AdmError::new(format!(
            "unsupported source project root schema version: {}",
            manifest.schema_version
        )));
    }
    if manifest.kind != SOURCE_PROJECT_KIND {
        return Err(AdmError::new(format!(
            "invalid source project root kind: {}",
            manifest.kind
        )));
    }
    if manifest.project_id != SOURCE_PROJECT_ID {
        return Err(AdmError::new(format!(
            "unexpected source project id: {}",
            manifest.project_id
        )));
    }

    let workspace_manifest = require_source_file(root, &manifest.workspace_manifest)?;
    let workspace_text = fs::read_to_string(&workspace_manifest).map_err(|error| {
        AdmError::new(format!(
            "failed to read workspace manifest {}: {error}",
            workspace_manifest.display()
        ))
    })?;
    if !workspace_text
        .lines()
        .any(|line| line.trim() == "[workspace]")
    {
        return Err(AdmError::new(format!(
            "workspace manifest does not declare [workspace]: {}",
            workspace_manifest.display()
        )));
    }

    if manifest.lockfiles.is_empty() {
        return Err(AdmError::new(
            "source project root manifest must declare lockfiles",
        ));
    }
    let mut lockfiles = BTreeSet::new();
    for lockfile in &manifest.lockfiles {
        if !lockfiles.insert(lockfile.as_str()) {
            return Err(AdmError::new(format!(
                "source project root manifest contains duplicate lockfile: {lockfile}"
            )));
        }
        require_source_file(root, lockfile)?;
    }
    for required in REQUIRED_SOURCE_LOCKFILES {
        if !lockfiles.contains(required) {
            return Err(AdmError::new(format!(
                "source project root manifest is missing required lockfile: {required}"
            )));
        }
    }

    let resource_manifest_path = require_source_file(root, &manifest.resource_manifest)?;
    let resource_manifest: ResourceManifestIdentity =
        serde_json::from_slice(&fs::read(&resource_manifest_path).map_err(|error| {
            AdmError::new(format!(
                "failed to read source resource manifest {}: {error}",
                resource_manifest_path.display()
            ))
        })?)
        .map_err(|error| {
            AdmError::new(format!(
                "invalid source resource manifest {}: {error}",
                resource_manifest_path.display()
            ))
        })?;
    if resource_manifest.schema_version != SOURCE_PROJECT_SCHEMA_VERSION {
        return Err(AdmError::new(format!(
            "unsupported source resource manifest schema version: {}",
            resource_manifest.schema_version
        )));
    }
    if resource_manifest.project_id != manifest.project_id {
        return Err(AdmError::new(format!(
            "source resource manifest project id mismatch: {}",
            resource_manifest.project_id
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceManifestIdentity {
    schema_version: u32,
    project_id: String,
}

fn require_source_file(root: &Path, relative_path: &str) -> AdmResult<PathBuf> {
    let path = safe_project_join(root, relative_path)?;
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        AdmError::new(format!(
            "required source project file is missing at {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AdmError::new(format!(
            "required source project path must be a regular file: {}",
            path.display()
        )));
    }
    Ok(path)
}

fn normalize_start_directory(start_path: &Path) -> AdmResult<PathBuf> {
    let mut current = start_path.canonicalize().map_err(|error| {
        AdmError::new(format!(
            "unable to canonicalize project root search start {}: {error}",
            start_path.display()
        ))
    })?;
    if current.is_file() {
        current = current
            .parent()
            .ok_or_else(|| AdmError::new("project root search start has no parent"))?
            .to_path_buf();
    }
    Ok(current)
}

fn project_root_not_found(start_path: &Path) -> AdmError {
    AdmError::new(format!(
        "unable to locate source project root: {ROOT_MARKER} was not found from {}",
        start_path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_root_rejects_a_non_rust_project_identity_before_loading_resources() {
        let root = std::env::temp_dir()
            .join(crate::new_stable_id("foreign-source-root").expect("temporary root identifier"));
        fs::create_dir_all(&root).expect("temporary root");
        fs::write(
            root.join(ROOT_MARKER),
            r#"{
                "schemaVersion": 1,
                "kind": "source-project-root",
                "projectId": "autodesignmaker-python",
                "workspaceManifest": "Cargo.toml",
                "lockfiles": ["Cargo.lock", "web/package-lock.json"],
                "resourceManifest": "knowledge/resource-manifest.json"
            }"#,
        )
        .expect("foreign root marker");

        let error = SourceProjectRoot::open(&root).expect_err("foreign root must be rejected");

        assert!(error.message().contains("unexpected source project id"));
        let _ = fs::remove_dir_all(root);
    }
}
