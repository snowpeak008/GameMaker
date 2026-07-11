#![forbid(unsafe_code)]

use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub mod io;
pub mod markdown;
pub mod paths;
pub mod process;
pub mod structured_md;
pub mod text_extractor;
pub mod tool;
pub mod yaml_compat;

pub type AdmResult<T> = Result<T, AdmError>;

static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmError {
    message: String,
}

impl AdmError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for AdmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for AdmError {}

impl From<std::io::Error> for AdmError {
    fn from(value: std::io::Error) -> Self {
        Self::new(value.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLevel {
    Static,
    Mock,
    Local,
    Real,
}

impl EvidenceLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Mock => "mock",
            Self::Local => "local",
            Self::Real => "real",
        }
    }

    pub fn parse(value: &str) -> AdmResult<Self> {
        match value.trim() {
            "static" => Ok(Self::Static),
            "mock" => Ok(Self::Mock),
            "local" => Ok(Self::Local),
            "real" => Ok(Self::Real),
            other => Err(AdmError::new(format!("unknown evidence level: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    Passed,
    Failed,
}

impl GateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct GateReport {
    name: String,
    status: GateStatus,
    rows: Vec<(String, String)>,
    blockers: Vec<String>,
}

impl GateReport {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: GateStatus::Passed,
            rows: Vec::new(),
            blockers: Vec::new(),
        }
    }

    pub fn add_row(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.rows.push((key.into(), value.into()));
    }

    pub fn add_blocker(&mut self, blocker: impl Into<String>) {
        self.status = GateStatus::Failed;
        self.blockers.push(blocker.into());
    }

    pub fn status(&self) -> GateStatus {
        self.status
    }

    pub fn passed(&self) -> bool {
        self.status == GateStatus::Passed
    }

    pub fn render(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("# {}\n", self.name));
        output.push_str(&format!("status={}\n", self.status.as_str()));
        output.push_str(&format!("timestamp_unix={}\n", unix_timestamp()));
        output.push_str(&format!("blocker_count={}\n", self.blockers.len()));
        for blocker in &self.blockers {
            output.push_str(&format!("blocker={blocker}\n"));
        }
        for (key, value) in &self.rows {
            output.push_str(&format!("{key}={value}\n"));
        }
        output
    }
}

pub fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn unix_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub fn new_stable_id(prefix: &str) -> AdmResult<String> {
    let clean = sanitize_identifier(prefix)?;
    let counter = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(format!("{clean}_{:x}_{counter:x}", unix_timestamp_millis()))
}

pub fn sanitize_identifier(value: &str) -> AdmResult<String> {
    let mut clean = String::new();
    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            clean.push(ch);
        } else if ch.is_whitespace() || ch == '.' {
            clean.push('_');
        }
    }
    let clean = clean.trim_matches(['_', '-']).to_string();
    if clean.is_empty() {
        Err(AdmError::new("identifier must contain portable characters"))
    } else {
        Ok(clean)
    }
}

pub fn fnv64_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv64:{hash:016x}")
}

pub fn hash_text(text: &str) -> String {
    fnv64_hex(text.as_bytes())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (bytes.len() as u64).wrapping_mul(8);
    let mut message = bytes.to_vec();
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in message.chunks(64) {
        let mut w = [0u32; 64];
        for (index, word) in w.iter_mut().enumerate().take(16) {
            let offset = index * 4;
            *word = u32::from_be_bytes([
                chunk[offset],
                chunk[offset + 1],
                chunk[offset + 2],
                chunk[offset + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    h.iter().map(|word| format!("{word:08x}")).collect()
}

pub fn ensure_relative_path(root: &Path, relative: &str) -> AdmResult<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute() {
        return Err(AdmError::new(format!(
            "path must be relative to root: {relative}"
        )));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::Prefix(_) | Component::RootDir
        )
    }) {
        return Err(AdmError::new(format!(
            "path escapes root or is not portable: {relative}"
        )));
    }
    Ok(root.join(path))
}

pub fn ensure_child_path(root: &Path, candidate: &Path) -> AdmResult<PathBuf> {
    let root = root
        .canonicalize()
        .map_err(|error| AdmError::new(format!("failed to canonicalize root: {error}")))?;
    let candidate = candidate.canonicalize().map_err(|error| {
        AdmError::new(format!(
            "failed to canonicalize candidate path {}: {error}",
            candidate.display()
        ))
    })?;
    if candidate.starts_with(&root) {
        Ok(candidate)
    } else {
        Err(AdmError::new(format!(
            "path is outside root: {}",
            candidate.display()
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileManifestEntry {
    pub relative_path: String,
    pub content_hash: String,
    pub byte_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileManifestRecord {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

pub fn file_manifest(root: &Path) -> AdmResult<Vec<FileManifestRecord>> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<FileManifestRecord>) -> AdmResult<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_files(root, &path, files)?;
        } else if file_type.is_file() {
            let bytes = fs::read(&path)?;
            let relative = path
                .strip_prefix(root)
                .map_err(|error| AdmError::new(format!("failed to relativize file: {error}")))?;
            files.push(FileManifestRecord {
                path: relative.to_string_lossy().replace('\\', "/"),
                size_bytes: bytes.len() as u64,
                sha256: sha256_hex(&bytes),
            });
        }
    }
    Ok(())
}

pub fn file_manifest_entry(root: &Path, relative: &str) -> AdmResult<FileManifestEntry> {
    let path = ensure_relative_path(root, relative)?;
    let bytes = fs::read(&path)?;
    Ok(FileManifestEntry {
        relative_path: relative.replace('\\', "/"),
        content_hash: fnv64_hex(&bytes),
        byte_len: bytes.len() as u64,
    })
}

pub fn write_text_atomic(path: &Path, text: &str) -> AdmResult<()> {
    write_bytes_atomic(path, text.as_bytes())
}

pub fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> AdmResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AdmError::new(format!("path has no parent: {}", path.display())))?;
    fs::create_dir_all(parent)?;
    let mut file = atomic_write_file::AtomicWriteFile::options()
        .open(path)
        .map_err(|error| {
            AdmError::new(format!(
                "failed to open atomic writer for {}: {error}",
                path.display()
            ))
        })?;
    file.write_all(bytes)?;
    file.commit().map_err(|error| {
        AdmError::new(format!(
            "failed to commit atomic write for {}: {error}",
            path.display()
        ))
    })?;
    Ok(())
}

pub struct StableDirectoryIdentity {
    handle: Option<same_file::Handle>,
}

impl fmt::Debug for StableDirectoryIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StableDirectoryIdentity")
            .field("captured", &self.handle.is_some())
            .finish()
    }
}

impl StableDirectoryIdentity {
    pub fn capture(path: &Path) -> AdmResult<Self> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| AdmError::new("directory identity could not be captured"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(AdmError::new("directory identity target is unsafe"));
        }
        let handle = same_file::Handle::from_path(path)
            .map_err(|_| AdmError::new("directory identity could not be captured"))?;
        Ok(Self {
            handle: Some(handle),
        })
    }

    pub fn matches_path(&self, path: &Path) -> AdmResult<bool> {
        let Some(expected) = self.handle.as_ref() else {
            return Ok(false);
        };
        let current = same_file::Handle::from_path(path)
            .map_err(|_| AdmError::new("directory identity could not be verified"))?;
        Ok(current.eq(expected))
    }

    pub fn release(&mut self) {
        self.handle.take();
    }
}

pub struct ProjectWriteLock {
    file: fs::File,
}

impl fmt::Debug for ProjectWriteLock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProjectWriteLock")
            .field("held", &true)
            .finish()
    }
}

impl Drop for ProjectWriteLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

pub fn acquire_project_write_lock(project_root: &Path) -> AdmResult<ProjectWriteLock> {
    let canonical = fs::canonicalize(project_root)
        .map_err(|_| AdmError::new("project write root is unavailable"))?;
    if !canonical.is_dir() {
        return Err(AdmError::new("project write root is unavailable"));
    }
    let mut identity = canonical.to_string_lossy().replace('\\', "/");
    #[cfg(windows)]
    {
        identity = identity.to_ascii_lowercase();
    }
    let lock_root = std::env::temp_dir()
        .join("AutoDesignMaker")
        .join("project_write_locks");
    fs::create_dir_all(&lock_root)
        .map_err(|_| AdmError::new("project write lock directory is unavailable"))?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_root.join(format!("{}.lock", sha256_hex(identity.as_bytes()))))
        .map_err(|_| AdmError::new("project write lock is unavailable"))?;
    for _ in 0..250 {
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => return Ok(ProjectWriteLock { file }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return Err(AdmError::new("project write lock could not be acquired")),
        }
    }
    Err(AdmError::new(
        "project write lock timed out; another commit is active",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_level_round_trips() {
        assert_eq!(
            EvidenceLevel::parse(EvidenceLevel::Local.as_str()).unwrap(),
            EvidenceLevel::Local
        );
        assert!(EvidenceLevel::parse("unknown").is_err());
    }

    #[test]
    fn report_records_blockers() {
        let mut report = GateReport::new("Test Gate");
        report.add_row("sample", "true");
        report.add_blocker("missing_contract");
        let rendered = report.render();
        assert!(!report.passed());
        assert!(rendered.contains("status=failed"));
        assert!(rendered.contains("blocker=missing_contract"));
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(hash_text("NEWrust"), hash_text("NEWrust"));
        assert_ne!(hash_text("NEWrust"), hash_text("old-rust"));
    }

    #[test]
    fn atomic_text_write_replaces_existing_file_without_a_delete_gap() {
        let root = std::env::temp_dir().join(format!(
            "adm-new-foundation-atomic-{}",
            new_stable_id("test").unwrap()
        ));
        let path = root.join("state.json");
        write_text_atomic(&path, "old").unwrap();
        write_text_atomic(&path, "new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sha256_matches_standard_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn relative_paths_cannot_escape_root() {
        let root = Path::new("root");
        assert!(ensure_relative_path(root, "a/b.txt").is_ok());
        assert!(ensure_relative_path(root, "../b.txt").is_err());
    }

    #[test]
    fn identifiers_are_portable_and_unique() {
        let left = new_stable_id("plan gate").unwrap();
        let right = new_stable_id("plan gate").unwrap();
        assert!(left.starts_with("plan_gate_"));
        assert_ne!(left, right);
        assert!(sanitize_identifier("...").is_err());
    }

    #[test]
    fn atomic_write_replaces_text_and_manifest_hash_changes() {
        let root = std::env::temp_dir().join(format!(
            "adm_new_foundation_test_{}",
            new_stable_id("atomic").unwrap()
        ));
        let path = root.join("nested").join("file.txt");
        write_text_atomic(&path, "first").unwrap();
        let first = file_manifest_entry(&root, "nested/file.txt").unwrap();
        write_text_atomic(&path, "second").unwrap();
        let second = file_manifest_entry(&root, "nested/file.txt").unwrap();
        assert_eq!(second.byte_len, 6);
        assert_ne!(first.content_hash, second.content_hash);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn file_manifest_returns_python_compatible_shape_with_sha256() {
        let root = std::env::temp_dir().join(format!(
            "adm_new_foundation_manifest_test_{}",
            new_stable_id("manifest").unwrap()
        ));
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join("b.txt"), b"b").unwrap();
        std::fs::write(nested.join("a.txt"), b"a").unwrap();

        let manifest = file_manifest(&root).unwrap();

        assert_eq!(
            manifest
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            vec!["b.txt", "nested/a.txt"]
        );
        assert_eq!(manifest[0].size_bytes, 1);
        assert_eq!(
            manifest[1].sha256,
            "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn child_path_must_stay_under_root() {
        let root = std::env::temp_dir().join(format!(
            "adm_new_foundation_path_test_{}",
            new_stable_id("root").unwrap()
        ));
        let child_dir = root.join("child");
        std::fs::create_dir_all(&child_dir).unwrap();
        let inside = ensure_child_path(&root, &child_dir).unwrap();
        assert!(inside.ends_with("child"));
        let outside = std::env::temp_dir();
        assert!(ensure_child_path(&root, &outside).is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
