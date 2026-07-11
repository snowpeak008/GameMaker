use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use adm_new_foundation::{AdmError, AdmResult};

const MAX_SNAPSHOT_FILES: usize = 8_192;
const MAX_SNAPSHOT_DEPTH: usize = 32;
static WINDOW_MARKER_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) struct InvocationMarker {
    path: PathBuf,
    pub(super) modified: SystemTime,
}

impl InvocationMarker {
    pub(super) fn create(root: &Path, label: &str) -> AdmResult<Self> {
        let sequence = WINDOW_MARKER_COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = root.join(format!(
            ".adm-image-window-{}-{sequence}-{timestamp}-{label}.stamp",
            std::process::id()
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| AdmError::new("Codex image invocation window could not be created"))?;
        let modified = file
            .write_all(b"window")
            .and_then(|_| file.sync_all())
            .and_then(|_| file.metadata())
            .and_then(|metadata| metadata.modified());
        let modified = match modified {
            Ok(modified) => modified,
            Err(_) => {
                drop(file);
                let _ = fs::remove_file(&path);
                return Err(AdmError::new(
                    "Codex image invocation window could not be recorded",
                ));
            }
        };
        Ok(Self { path, modified })
    }
}

impl Drop for InvocationMarker {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct PngStamp {
    modified: SystemTime,
}

pub(super) fn snapshot_pngs(root: &Path) -> AdmResult<BTreeMap<PathBuf, PngStamp>> {
    let mut result = BTreeMap::new();
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_SNAPSHOT_DEPTH {
            return Err(AdmError::new(
                "Codex image output tree exceeded safe limits",
            ));
        }
        let entries = fs::read_dir(&directory)
            .map_err(|_| AdmError::new("Codex image output directory could not be inspected"))?;
        for entry in entries {
            let entry = entry.map_err(|_| {
                AdmError::new("Codex image output directory could not be inspected")
            })?;
            let file_type = entry.file_type().map_err(|_| {
                AdmError::new("Codex image output directory could not be inspected")
            })?;
            let path = entry.path();
            if file_type.is_dir() && !file_type.is_symlink() {
                pending.push((path, depth + 1));
                continue;
            }
            if !has_png_extension(&path) {
                continue;
            }
            let canonical = match fs::canonicalize(&path) {
                Ok(path) if path.starts_with(root) => path,
                _ => continue,
            };
            let metadata = fs::metadata(&canonical).map_err(|_| {
                AdmError::new("Codex image output directory could not be inspected")
            })?;
            if !metadata.is_file() {
                continue;
            }
            let modified = metadata
                .modified()
                .map_err(|_| AdmError::new("Codex image output timestamp could not be verified"))?;
            result.insert(canonical, PngStamp { modified });
            if result.len() > MAX_SNAPSHOT_FILES {
                return Err(AdmError::new(
                    "Codex image output tree exceeded safe limits",
                ));
            }
        }
    }
    Ok(result)
}

pub(super) fn changed_in_window(
    before: &BTreeMap<PathBuf, PngStamp>,
    after: &BTreeMap<PathBuf, PngStamp>,
    started: SystemTime,
    finished: SystemTime,
) -> Vec<PathBuf> {
    let mut candidates = BTreeSet::new();
    for (path, stamp) in after {
        let changed = before
            .get(path)
            .is_none_or(|previous| previous.modified != stamp.modified);
        if changed && stamp.modified >= started && stamp.modified <= finished {
            candidates.insert(path.clone());
        }
    }
    candidates.into_iter().collect()
}

fn has_png_extension(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
}
