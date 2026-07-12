use crate::manifest::{
    collect_regular_files, load_source_resource_manifest, measure_resource_tree,
    open_source_project,
};
use adm_new_foundation::{AdmError, AdmResult, io, sha256_hex, unix_timestamp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const RUNTIME_FRESHNESS_PATH: &str = "knowledge/freshness.json";

#[deprecated(
    since = "0.1.0",
    note = "pass an explicit runtime-data root to freshness_path; source-tree snapshots are no longer used"
)]
pub const FRESHNESS_PATH: &str = RUNTIME_FRESHNESS_PATH;

#[deprecated(
    since = "0.1.0",
    note = "freshness inputs are discovered from .project_root, Cargo.toml and resource-manifest.json"
)]
pub const KEY_FILES: [&str; 0] = [];

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

#[derive(Debug, Deserialize)]
struct CargoWorkspaceManifest {
    workspace: CargoWorkspace,
}

#[derive(Debug, Deserialize)]
struct CargoWorkspace {
    members: Vec<String>,
}

pub fn compute_file_hash(path: &Path) -> AdmResult<FileFreshness> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        AdmError::new(format!(
            "failed to inspect freshness input {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AdmError::new(format!(
            "freshness input must be a regular file: {}",
            path.display()
        )));
    }
    let content = fs::read(path)?;
    let size = u64::try_from(content.len())
        .map_err(|_| AdmError::new("freshness input size does not fit u64"))?;
    Ok(FileFreshness {
        sha256: sha256_hex(&content),
        size,
    })
}

/// Builds a read-only snapshot from the independent source-root contract.
///
/// The source marker selects the workspace manifest, lockfiles and resource
/// manifest. Workspace members and resource groups are then discovered from
/// those manifests; no path from a parent or legacy project is consulted.
pub fn build_snapshot(project_root: &Path) -> AdmResult<(FreshnessSnapshot, Vec<String>)> {
    let source_root = open_source_project(project_root)?;
    let resource_manifest = load_source_resource_manifest(&source_root)?;
    let mut relative_files = BTreeSet::new();
    let mut missing = BTreeSet::new();

    relative_files.insert(".project_root".to_string());
    relative_files.insert(source_root.manifest().workspace_manifest.clone());
    relative_files.extend(source_root.manifest().lockfiles.iter().cloned());
    relative_files.insert(source_root.manifest().resource_manifest.clone());

    for optional_control_file in ["rust-toolchain.toml", ".cargo/config.toml"] {
        if source_root.join(optional_control_file)?.is_file() {
            relative_files.insert(optional_control_file.to_string());
        }
    }

    discover_workspace_files(&source_root, &mut relative_files, &mut missing)?;
    for group in &resource_manifest.groups {
        let group_path = source_root.join(&group.path)?;
        if !path_exists(&group_path)? {
            missing.insert(group.path.clone());
            continue;
        }
        for file in collect_regular_files(&group_path)? {
            relative_files.insert(relative_to_source(source_root.path(), &file)?);
        }
    }

    let mut files = BTreeMap::new();
    for relative_path in relative_files {
        let path = source_root.join(&relative_path)?;
        if !path_exists(&path)? {
            missing.insert(relative_path);
            continue;
        }
        files.insert(relative_path, compute_file_hash(&path)?);
    }
    Ok((
        FreshnessSnapshot {
            generated_at: now_string(),
            files,
        },
        missing.into_iter().collect(),
    ))
}

/// Compatibility entry point. It is intentionally read-only.
///
/// Older versions wrote `knowledge/ai_memory/.../freshness.json` in the source
/// checkout. That behavior was removed; callers that need persistence must use
/// [`write_runtime_freshness`] and provide an explicit runtime-data root.
pub fn update_freshness(project_root: &Path) -> AdmResult<(FreshnessSnapshot, Vec<String>)> {
    build_snapshot(project_root)
}

/// Verifies the current manifest-backed resource trees without writing a
/// snapshot. Rust workspace paths missing from the source contract are also
/// reported.
pub fn check_staleness(project_root: &Path) -> AdmResult<StalenessReport> {
    let source_root = open_source_project(project_root)?;
    let resource_manifest = load_source_resource_manifest(&source_root)?;
    let (_, workspace_missing) = build_snapshot(source_root.path())?;
    let mut stale = Vec::new();
    let mut fresh = Vec::new();
    let mut missing = workspace_missing.into_iter().collect::<BTreeSet<_>>();

    for group in &resource_manifest.groups {
        let path = source_root.join(&group.path)?;
        if !path_exists(&path)? {
            missing.insert(group.path.clone());
            continue;
        }
        let actual = measure_resource_tree(path)?;
        if actual.files == group.files
            && actual.bytes == group.bytes
            && actual.tree_sha256 == group.tree_sha256
        {
            fresh.push(group.path.clone());
        } else {
            stale.push(group.path.clone());
        }
    }
    stale.sort();
    fresh.sort();
    Ok(StalenessReport {
        stale,
        fresh,
        missing: missing.into_iter().collect(),
        generated_at: now_string(),
    })
}

pub fn check_snapshot_staleness(
    project_root: &Path,
    baseline: &FreshnessSnapshot,
) -> AdmResult<StalenessReport> {
    let (current, discovered_missing) = build_snapshot(project_root)?;
    let all_paths = baseline
        .files
        .keys()
        .chain(current.files.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut stale = Vec::new();
    let mut fresh = Vec::new();
    let mut missing = discovered_missing.into_iter().collect::<BTreeSet<_>>();
    for path in all_paths {
        match (baseline.files.get(&path), current.files.get(&path)) {
            (Some(expected), Some(actual)) if expected == actual => fresh.push(path),
            (Some(_), Some(_)) | (None, Some(_)) => stale.push(path),
            (Some(_), None) => {
                missing.insert(path);
            }
            (None, None) => {}
        }
    }
    Ok(StalenessReport {
        stale,
        fresh,
        missing: missing.into_iter().collect(),
        generated_at: baseline.generated_at.clone(),
    })
}

pub fn write_runtime_freshness(
    project_root: &Path,
    runtime_data_root: &Path,
) -> AdmResult<(FreshnessSnapshot, Vec<String>, PathBuf)> {
    let source_root = open_source_project(project_root)?;
    let runtime_data_root =
        validate_runtime_data_root(source_root.path(), runtime_data_root, true)?;
    let (snapshot, missing) = build_snapshot(source_root.path())?;
    let path = freshness_path(&runtime_data_root);
    io::write_json_serializable(&path, &snapshot)?;
    Ok((snapshot, missing, path))
}

pub fn check_runtime_freshness(
    project_root: &Path,
    runtime_data_root: &Path,
) -> AdmResult<StalenessReport> {
    let source_root = open_source_project(project_root)?;
    let runtime_data_root =
        validate_runtime_data_root(source_root.path(), runtime_data_root, false)?;
    let path = freshness_path(&runtime_data_root);
    let snapshot: FreshnessSnapshot =
        serde_json::from_slice(&fs::read(&path).map_err(|error| {
            AdmError::new(format!(
                "runtime freshness snapshot is missing at {}: {error}",
                path.display()
            ))
        })?)
        .map_err(|error| {
            AdmError::new(format!(
                "failed to parse runtime freshness snapshot {}: {error}",
                path.display()
            ))
        })?;
    check_snapshot_staleness(source_root.path(), &snapshot)
}

/// Returns the snapshot path for an explicit runtime-data root.
pub fn freshness_path(runtime_data_root: &Path) -> PathBuf {
    runtime_data_root.join(RUNTIME_FRESHNESS_PATH)
}

fn discover_workspace_files(
    source_root: &adm_new_foundation::paths::SourceProjectRoot,
    files: &mut BTreeSet<String>,
    missing: &mut BTreeSet<String>,
) -> AdmResult<()> {
    let workspace_path = source_root.join(&source_root.manifest().workspace_manifest)?;
    let workspace_text = fs::read_to_string(&workspace_path)?;
    let workspace: CargoWorkspaceManifest = toml::from_str(&workspace_text).map_err(|error| {
        AdmError::new(format!(
            "failed to parse Cargo workspace manifest {}: {error}",
            workspace_path.display()
        ))
    })?;
    if workspace.workspace.members.is_empty() {
        return Err(AdmError::new(
            "Cargo workspace must declare at least one member",
        ));
    }
    let mut members = BTreeSet::new();
    for member in workspace.workspace.members {
        let member = member.replace('\\', "/");
        if member.contains(['*', '?', '[', ']']) {
            return Err(AdmError::new(format!(
                "Cargo workspace member globs are not supported by the source contract: {member}"
            )));
        }
        if !members.insert(member.clone()) {
            return Err(AdmError::new(format!(
                "Cargo workspace contains duplicate member: {member}"
            )));
        }
        let member_root = source_root.join(&member)?;
        if !path_exists(&member_root)? {
            missing.insert(member);
            continue;
        }
        let member_metadata = fs::symlink_metadata(&member_root)?;
        if !member_metadata.is_dir() || member_metadata.file_type().is_symlink() {
            return Err(AdmError::new(format!(
                "Cargo workspace member must be a regular directory: {}",
                member_root.display()
            )));
        }
        let member_manifest = format!("{member}/Cargo.toml");
        let member_manifest_path = source_root.join(&member_manifest)?;
        if !path_exists(&member_manifest_path)? {
            missing.insert(member_manifest);
            continue;
        }
        files.insert(member_manifest);
        collect_rust_sources(source_root.path(), &member_root, files)?;
    }
    Ok(())
}

fn collect_rust_sources(
    source_root: &Path,
    current: &Path,
    files: &mut BTreeSet<String>,
) -> AdmResult<()> {
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(AdmError::new(format!(
                "Cargo workspace source links are not allowed: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            let name = entry.file_name();
            if matches!(name.to_str(), Some("target" | ".git" | "node_modules")) {
                continue;
            }
            collect_rust_sources(source_root, &path, files)?;
        } else if metadata.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("rs")
        {
            files.insert(relative_to_source(source_root, &path)?);
        }
    }
    Ok(())
}

fn validate_runtime_data_root(
    source_root: &Path,
    runtime_data_root: &Path,
    create: bool,
) -> AdmResult<PathBuf> {
    if !runtime_data_root.is_absolute() {
        return Err(AdmError::new(format!(
            "runtime-data root must be absolute: {}",
            runtime_data_root.display()
        )));
    }
    let canonical_source = source_root.canonicalize()?;
    let canonical_runtime = if path_exists(runtime_data_root)? {
        runtime_data_root.canonicalize()?
    } else {
        let mut ancestor = runtime_data_root;
        while !path_exists(ancestor)? {
            ancestor = ancestor.parent().ok_or_else(|| {
                AdmError::new(format!(
                    "runtime-data root has no existing ancestor: {}",
                    runtime_data_root.display()
                ))
            })?;
        }
        let canonical_ancestor = ancestor.canonicalize()?;
        if canonical_ancestor.starts_with(&canonical_source) {
            return Err(AdmError::new(format!(
                "runtime-data root must not be inside the source project: {}",
                runtime_data_root.display()
            )));
        }
        runtime_data_root.to_path_buf()
    };
    if canonical_runtime.starts_with(&canonical_source) {
        return Err(AdmError::new(format!(
            "runtime-data root must not be inside the source project: {}",
            runtime_data_root.display()
        )));
    }
    if create {
        fs::create_dir_all(&canonical_runtime)?;
    } else if !canonical_runtime.is_dir() {
        return Err(AdmError::new(format!(
            "runtime-data root is missing or not a directory: {}",
            canonical_runtime.display()
        )));
    }
    Ok(canonical_runtime)
}

fn relative_to_source(source_root: &Path, path: &Path) -> AdmResult<String> {
    path.strip_prefix(source_root)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| {
            AdmError::new(format!(
                "freshness path escaped the source project: {}",
                path.display()
            ))
        })
}

fn path_exists(path: &Path) -> AdmResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(AdmError::new(format!(
            "failed to inspect freshness path {}: {error}",
            path.display()
        ))),
    }
}

fn now_string() -> String {
    format!("unix:{}", unix_timestamp())
}
