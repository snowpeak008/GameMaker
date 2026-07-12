use adm_new_foundation::{
    AdmError, AdmResult,
    paths::{SOURCE_PROJECT_ID, SourceProjectRoot},
    sha256_hex,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const SOURCE_RESOURCE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceResourceManifest {
    pub schema_version: u32,
    pub project_id: String,
    #[serde(default)]
    pub generated_from: String,
    pub groups: Vec<SourceResourceGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceResourceGroup {
    pub path: String,
    pub files: u64,
    pub bytes: u64,
    #[serde(rename = "treeSha256")]
    pub tree_sha256: String,
    pub mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceTreeMeasure {
    pub files: u64,
    pub bytes: u64,
    pub tree_sha256: String,
}

#[derive(Debug)]
struct TreeFile {
    relative_path: String,
    bytes: Vec<u8>,
}

pub fn open_source_project(project_root: impl AsRef<Path>) -> AdmResult<SourceProjectRoot> {
    SourceProjectRoot::open(project_root)
}

pub fn load_source_resource_manifest(
    source_root: &SourceProjectRoot,
) -> AdmResult<SourceResourceManifest> {
    let manifest_path = source_root.join(&source_root.manifest().resource_manifest)?;
    let metadata = fs::symlink_metadata(&manifest_path).map_err(|error| {
        AdmError::new(format!(
            "source resource manifest is missing at {}: {error}",
            manifest_path.display()
        ))
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(AdmError::new(format!(
            "source resource manifest must be a regular file: {}",
            manifest_path.display()
        )));
    }
    let manifest: SourceResourceManifest = serde_json::from_slice(&fs::read(&manifest_path)?)
        .map_err(|error| {
            AdmError::new(format!(
                "failed to parse source resource manifest {}: {error}",
                manifest_path.display()
            ))
        })?;
    validate_source_resource_manifest(source_root, &manifest)?;
    Ok(manifest)
}

pub fn measure_resource_tree(path: impl AsRef<Path>) -> AdmResult<ResourceTreeMeasure> {
    let root = path.as_ref();
    let metadata = regular_metadata(root)?;
    let mut files = Vec::new();
    if metadata.is_file() {
        files.push(TreeFile {
            relative_path: ".".to_string(),
            bytes: fs::read(root)?,
        });
    } else if metadata.is_dir() {
        collect_tree_files(root, root, &mut files)?;
    } else {
        return Err(AdmError::new(format!(
            "resource path must be a regular file or directory: {}",
            root.display()
        )));
    }

    files.sort_by(|left, right| {
        left.relative_path
            .to_ascii_lowercase()
            .cmp(&right.relative_path.to_ascii_lowercase())
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    let mut bytes = 0_u64;
    let mut fingerprint_lines = Vec::with_capacity(files.len());
    for file in &files {
        let file_bytes = u64::try_from(file.bytes.len())
            .map_err(|_| AdmError::new("resource file size does not fit u64"))?;
        bytes = bytes
            .checked_add(file_bytes)
            .ok_or_else(|| AdmError::new("resource tree byte count overflowed u64"))?;
        fingerprint_lines.push(format!(
            "{}|{}|{}",
            file.relative_path,
            file_bytes,
            sha256_hex(&file.bytes)
        ));
    }
    Ok(ResourceTreeMeasure {
        files: u64::try_from(files.len())
            .map_err(|_| AdmError::new("resource file count does not fit u64"))?,
        bytes,
        tree_sha256: sha256_hex(fingerprint_lines.join("\n").as_bytes()),
    })
}

pub(crate) fn collect_regular_files(root: &Path) -> AdmResult<Vec<PathBuf>> {
    let metadata = regular_metadata(root)?;
    let mut files = Vec::new();
    if metadata.is_file() {
        files.push(root.to_path_buf());
    } else if metadata.is_dir() {
        collect_file_paths(root, &mut files)?;
    } else {
        return Err(AdmError::new(format!(
            "path must be a regular file or directory: {}",
            root.display()
        )));
    }
    files.sort();
    Ok(files)
}

fn validate_source_resource_manifest(
    source_root: &SourceProjectRoot,
    manifest: &SourceResourceManifest,
) -> AdmResult<()> {
    if manifest.schema_version != SOURCE_RESOURCE_SCHEMA_VERSION {
        return Err(AdmError::new(format!(
            "unsupported source resource manifest schema version: {}",
            manifest.schema_version
        )));
    }
    if manifest.project_id != SOURCE_PROJECT_ID
        || manifest.project_id != source_root.manifest().project_id
    {
        return Err(AdmError::new(format!(
            "source resource manifest project id mismatch: {}",
            manifest.project_id
        )));
    }
    if manifest.groups.is_empty() {
        return Err(AdmError::new(
            "source resource manifest must declare at least one resource group",
        ));
    }

    let mut paths = BTreeSet::new();
    for group in &manifest.groups {
        let normalized = group.path.replace('\\', "/");
        if normalized != group.path || !paths.insert(group.path.as_str()) {
            return Err(AdmError::new(format!(
                "source resource group path must be unique and portable: {}",
                group.path
            )));
        }
        source_root.join(&group.path)?;
        if group.files == 0
            || group.bytes == 0
            || group.tree_sha256.len() != 64
            || !group
                .tree_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || group.mode.trim().is_empty()
        {
            return Err(AdmError::new(format!(
                "source resource group declaration is incomplete: {}",
                group.path
            )));
        }
    }
    Ok(())
}

fn regular_metadata(path: &Path) -> AdmResult<fs::Metadata> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        AdmError::new(format!(
            "failed to inspect source resource path {}: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(AdmError::new(format!(
            "source resource links are not allowed: {}",
            path.display()
        )));
    }
    Ok(metadata)
}

fn collect_tree_files(root: &Path, current: &Path, files: &mut Vec<TreeFile>) -> AdmResult<()> {
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = regular_metadata(&path)?;
        if metadata.is_dir() {
            collect_tree_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative_path = path
                .strip_prefix(root)
                .map_err(|_| AdmError::new("resource file escaped its declared root"))?
                .to_string_lossy()
                .replace('\\', "/");
            files.push(TreeFile {
                relative_path,
                bytes: fs::read(path)?,
            });
        } else {
            return Err(AdmError::new(format!(
                "source resource contains an unsupported path kind: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn collect_file_paths(current: &Path, files: &mut Vec<PathBuf>) -> AdmResult<()> {
    let mut entries = fs::read_dir(current)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = regular_metadata(&path)?;
        if metadata.is_dir() {
            collect_file_paths(&path, files)?;
        } else if metadata.is_file() {
            files.push(path);
        } else {
            return Err(AdmError::new(format!(
                "source tree contains an unsupported path kind: {}",
                path.display()
            )));
        }
    }
    Ok(())
}
