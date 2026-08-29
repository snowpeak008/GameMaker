#![forbid(unsafe_code)]

use adm_foundation::{
    AdmError, AdmErrorKind, AdmResult, ArchiveId, ContentHash, ProjectId, SessionId, UtcTimestamp,
    atomic_write, ensure_within_root,
};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const ARCHIVE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveManifest {
    pub archive_id: ArchiveId,
    pub project_id: ProjectId,
    pub display_name: String,
    pub format_version: u32,
    pub created_at: UtcTimestamp,
    pub updated_at: UtcTimestamp,
}

impl ArchiveManifest {
    pub fn new(display_name: impl Into<String>) -> AdmResult<Self> {
        let display_name = display_name.into();
        if display_name.trim().is_empty() {
            return Err(AdmError::invalid_input(
                "archive display_name cannot be empty",
            ));
        }
        let now = UtcTimestamp::now();
        Ok(Self {
            archive_id: ArchiveId::generate(),
            project_id: ProjectId::generate(),
            display_name,
            format_version: ARCHIVE_FORMAT_VERSION,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn to_manifest_text(&self) -> String {
        format!(
            "format_version={}\narchive_id={}\nproject_id={}\ndisplay_name={}\ncreated_at={}\nupdated_at={}\n",
            self.format_version,
            self.archive_id,
            self.project_id,
            self.display_name,
            self.created_at.as_millis(),
            self.updated_at.as_millis()
        )
    }

    pub fn from_manifest_text(text: &str) -> AdmResult<Self> {
        let mut format_version = None;
        let mut archive_id = None;
        let mut project_id = None;
        let mut display_name = None;
        let mut created_at = None;
        let mut updated_at = None;

        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key {
                "format_version" => {
                    format_version = Some(value.parse::<u32>().map_err(|error| {
                        AdmError::validation(format!("invalid format_version: {error}"))
                    })?);
                }
                "archive_id" => archive_id = Some(ArchiveId::new(value)?),
                "project_id" => project_id = Some(ProjectId::new(value)?),
                "display_name" => display_name = Some(value.to_string()),
                "created_at" => {
                    created_at = Some(UtcTimestamp::from_millis(value.parse::<u128>().map_err(
                        |error| AdmError::validation(format!("invalid created_at: {error}")),
                    )?));
                }
                "updated_at" => {
                    updated_at = Some(UtcTimestamp::from_millis(value.parse::<u128>().map_err(
                        |error| AdmError::validation(format!("invalid updated_at: {error}")),
                    )?));
                }
                _ => {}
            }
        }

        let manifest = Self {
            archive_id: archive_id
                .ok_or_else(|| AdmError::validation("manifest missing archive_id"))?,
            project_id: project_id
                .ok_or_else(|| AdmError::validation("manifest missing project_id"))?,
            display_name: display_name
                .ok_or_else(|| AdmError::validation("manifest missing display_name"))?,
            format_version: format_version
                .ok_or_else(|| AdmError::validation("manifest missing format_version"))?,
            created_at: created_at
                .ok_or_else(|| AdmError::validation("manifest missing created_at"))?,
            updated_at: updated_at
                .ok_or_else(|| AdmError::validation("manifest missing updated_at"))?,
        };

        if manifest.format_version != ARCHIVE_FORMAT_VERSION {
            return Err(AdmError::unsupported(format!(
                "unsupported archive format version {}",
                manifest.format_version
            )));
        }
        if manifest.display_name.trim().is_empty() {
            return Err(AdmError::validation(
                "manifest display_name cannot be empty",
            ));
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormalArchive {
    pub root: PathBuf,
    pub manifest: ArchiveManifest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivePackageFileInspection {
    pub relative_path: PathBuf,
    pub expected_hash: String,
    pub actual_hash: ContentHash,
    pub bytes: usize,
    pub hash_matches: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivePackageDoctorReport {
    pub package_file: PathBuf,
    pub package_present: bool,
    pub package_bytes: u64,
    pub format_label: Option<String>,
    pub format_version: Option<u32>,
    pub expected_payload_hash: Option<String>,
    pub actual_payload_hash: Option<ContentHash>,
    pub declared_file_count: Option<usize>,
    pub manifest: Option<ArchiveManifest>,
    pub files: Vec<ArchivePackageFileInspection>,
    pub issues: Vec<String>,
}

impl ArchivePackageDoctorReport {
    pub fn ready(&self) -> bool {
        self.package_present
            && self.manifest.is_some()
            && !self.files.is_empty()
            && self.issues.is_empty()
    }

    pub fn content_bytes(&self) -> usize {
        self.files.iter().map(|file| file.bytes).sum()
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Project Package Doctor\n");
        document.push_str(&format!("package_file={}\n", self.package_file.display()));
        document.push_str(&format!("package_present={}\n", self.package_present));
        document.push_str(&format!("ready={}\n", self.ready()));
        document.push_str(&format!("package_bytes={}\n", self.package_bytes));
        document.push_str(&format!("content_bytes={}\n", self.content_bytes()));
        document.push_str(&format!(
            "format={}\n",
            self.format_label.as_deref().unwrap_or("unknown")
        ));
        if let Some(format_version) = self.format_version {
            document.push_str(&format!("format_version={format_version}\n"));
        }
        if let Some(expected_payload_hash) = &self.expected_payload_hash {
            document.push_str(&format!("payload_hash_expected={expected_payload_hash}\n"));
        }
        if let Some(actual_payload_hash) = &self.actual_payload_hash {
            document.push_str(&format!("payload_hash_actual={actual_payload_hash}\n"));
        }
        if let (Some(expected), Some(actual)) =
            (&self.expected_payload_hash, &self.actual_payload_hash)
        {
            document.push_str(&format!(
                "payload_hash_match={}\n",
                expected == actual.as_str()
            ));
        }
        if let Some(declared_file_count) = self.declared_file_count {
            document.push_str(&format!("file_count_declared={declared_file_count}\n"));
        }
        document.push_str(&format!("file_count_actual={}\n", self.files.len()));

        if let Some(manifest) = &self.manifest {
            document.push_str("## Manifest\n");
            document.push_str(&format!("archive_id={}\n", manifest.archive_id));
            document.push_str(&format!("project_id={}\n", manifest.project_id));
            document.push_str(&format!("display_name={}\n", manifest.display_name));
            document.push_str(&format!("created_at={}\n", manifest.created_at.as_millis()));
            document.push_str(&format!("updated_at={}\n", manifest.updated_at.as_millis()));
        }

        document.push_str("## Files\n");
        if self.files.is_empty() {
            document.push_str("- none\n");
        } else {
            for file in &self.files {
                document.push_str(&format!("path={}\n", file.relative_path.display()));
                document.push_str(&format!("bytes={}\n", file.bytes));
                document.push_str(&format!("hash_expected={}\n", file.expected_hash));
                document.push_str(&format!("hash_actual={}\n", file.actual_hash));
                document.push_str(&format!("hash_match={}\n", file.hash_matches));
            }
        }

        document.push_str("## Issues\n");
        if self.issues.is_empty() {
            document.push_str("- none\n");
        } else {
            for issue in &self.issues {
                document.push_str(&format!("- {issue}\n"));
            }
        }
        document
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSession {
    pub session_id: SessionId,
    pub root: PathBuf,
    pub linked_archive_id: Option<ArchiveId>,
}

impl WorkspaceSession {
    pub fn content_root(&self) -> PathBuf {
        self.root.join("content")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInspection {
    pub session_id: String,
    pub root: PathBuf,
    pub active_lock: bool,
    pub file_count: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveWorkspaceDoctorReport {
    pub workspaces: Vec<WorkspaceInspection>,
}

impl ArchiveWorkspaceDoctorReport {
    pub fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    pub fn active_count(&self) -> usize {
        self.workspaces
            .iter()
            .filter(|workspace| workspace.active_lock)
            .count()
    }

    pub fn stale_count(&self) -> usize {
        self.workspaces
            .iter()
            .filter(|workspace| !workspace.active_lock)
            .count()
    }

    pub fn total_bytes(&self) -> u64 {
        self.workspaces
            .iter()
            .map(|workspace| workspace.bytes)
            .sum()
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Workspace Doctor\n");
        document.push_str(&format!("workspace_count={}\n", self.workspace_count()));
        document.push_str(&format!("active_count={}\n", self.active_count()));
        document.push_str(&format!("stale_count={}\n", self.stale_count()));
        document.push_str(&format!("total_bytes={}\n", self.total_bytes()));
        document.push_str("## Workspaces\n");
        if self.workspaces.is_empty() {
            document.push_str("- none\n");
        } else {
            for workspace in &self.workspaces {
                document.push_str(&format!("session_id={}\n", workspace.session_id));
                document.push_str(&format!("root={}\n", workspace.root.display()));
                document.push_str(&format!("active_lock={}\n", workspace.active_lock));
                document.push_str(&format!("file_count={}\n", workspace.file_count));
                document.push_str(&format!("bytes={}\n", workspace.bytes));
            }
        }
        document
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveWorkspaceCleanupReport {
    pub before: ArchiveWorkspaceDoctorReport,
    pub removed: Vec<WorkspaceInspection>,
    pub skipped_active: Vec<WorkspaceInspection>,
}

impl ArchiveWorkspaceCleanupReport {
    pub fn removed_count(&self) -> usize {
        self.removed.len()
    }

    pub fn skipped_active_count(&self) -> usize {
        self.skipped_active.len()
    }

    pub fn removed_bytes(&self) -> u64 {
        self.removed.iter().map(|workspace| workspace.bytes).sum()
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# Workspace Cleanup\n");
        document.push_str(&format!(
            "workspace_count_before={}\n",
            self.before.workspace_count()
        ));
        document.push_str(&format!(
            "stale_count_before={}\n",
            self.before.stale_count()
        ));
        document.push_str(&format!("removed_count={}\n", self.removed_count()));
        document.push_str(&format!(
            "skipped_active_count={}\n",
            self.skipped_active_count()
        ));
        document.push_str(&format!("removed_bytes={}\n", self.removed_bytes()));

        document.push_str("## Removed\n");
        if self.removed.is_empty() {
            document.push_str("- none\n");
        } else {
            for workspace in &self.removed {
                document.push_str(&format!("session_id={}\n", workspace.session_id));
                document.push_str(&format!("root={}\n", workspace.root.display()));
                document.push_str(&format!("file_count={}\n", workspace.file_count));
                document.push_str(&format!("bytes={}\n", workspace.bytes));
            }
        }

        document.push_str("## Skipped Active\n");
        if self.skipped_active.is_empty() {
            document.push_str("- none\n");
        } else {
            for workspace in &self.skipped_active {
                document.push_str(&format!("session_id={}\n", workspace.session_id));
                document.push_str(&format!("root={}\n", workspace.root.display()));
            }
        }
        document
    }
}

pub struct ArchiveLock {
    path: PathBuf,
    session_id: SessionId,
}

impl ArchiveLock {
    pub fn acquire(archive_root: impl AsRef<Path>, session_id: SessionId) -> AdmResult<Self> {
        let path = archive_root.as_ref().join(".archive_lock");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    let owner = fs::read_to_string(&path)
                        .unwrap_or_else(|_| "existing lock owner is unreadable".to_string());
                    AdmError::new(
                        AdmErrorKind::AlreadyLocked,
                        "formal archive is already locked",
                    )
                    .with_context(owner)
                } else {
                    AdmError::from(error)
                }
            })?;

        let content = format!(
            "session_id={}\npid={}\ncreated_at={}\n",
            session_id,
            std::process::id(),
            UtcTimestamp::now().as_millis()
        );
        file.write_all(content.as_bytes())?;
        file.sync_all()?;

        Ok(Self { path, session_id })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }
}

impl Drop for ArchiveLock {
    fn drop(&mut self) {
        let Ok(content) = fs::read_to_string(&self.path) else {
            return;
        };
        if content.contains(self.session_id.as_str()) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArchiveRepository {
    root: PathBuf,
}

impl ArchiveRepository {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn initialize(&self) -> AdmResult<()> {
        fs::create_dir_all(self.archives_root())?;
        fs::create_dir_all(self.workspaces_root())?;
        Ok(())
    }

    pub fn create_archive(&self, display_name: impl Into<String>) -> AdmResult<FormalArchive> {
        self.initialize()?;
        let manifest = ArchiveManifest::new(display_name)?;
        let root = self.archives_root().join(manifest.archive_id.as_str());
        fs::create_dir_all(root.join("content"))?;
        atomic_write(
            root.join("manifest.adm"),
            manifest.to_manifest_text().as_bytes(),
        )?;
        Ok(FormalArchive { root, manifest })
    }

    pub fn load_archive(&self, archive_id: &ArchiveId) -> AdmResult<FormalArchive> {
        let root = self.archives_root().join(archive_id.as_str());
        if !root.exists() {
            return Err(
                AdmError::new(AdmErrorKind::NotFound, "formal archive does not exist")
                    .with_context(format!("archive_id={archive_id}")),
            );
        }
        let manifest_text = fs::read_to_string(root.join("manifest.adm"))?;
        let manifest = ArchiveManifest::from_manifest_text(&manifest_text)?;
        if &manifest.archive_id != archive_id {
            return Err(AdmError::validation(
                "archive manifest id does not match directory name",
            ));
        }
        Ok(FormalArchive { root, manifest })
    }

    pub fn list_archives(&self) -> AdmResult<Vec<FormalArchive>> {
        self.initialize()?;
        let mut archives = Vec::new();
        for entry in fs::read_dir(self.archives_root())? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let manifest_path = entry.path().join("manifest.adm");
            if !manifest_path.exists() {
                continue;
            }
            let manifest_text = fs::read_to_string(&manifest_path)?;
            let manifest = ArchiveManifest::from_manifest_text(&manifest_text)?;
            archives.push(FormalArchive {
                root: entry.path(),
                manifest,
            });
        }
        archives.sort_by(|left, right| left.manifest.archive_id.cmp(&right.manifest.archive_id));
        Ok(archives)
    }

    pub fn inspect_workspaces(&self) -> AdmResult<ArchiveWorkspaceDoctorReport> {
        self.initialize()?;
        let active_session_ids = self.active_lock_session_ids()?;
        let mut workspaces = Vec::new();
        for entry in fs::read_dir(self.workspaces_root())? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let root = entry.path();
            let session_id = entry.file_name().to_string_lossy().to_string();
            let files = collect_files(&root)?;
            let mut bytes = 0;
            for file in &files {
                bytes += fs::metadata(file)?.len();
            }
            workspaces.push(WorkspaceInspection {
                active_lock: active_session_ids.contains(&session_id),
                session_id,
                root,
                file_count: files.len(),
                bytes,
            });
        }
        workspaces.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        Ok(ArchiveWorkspaceDoctorReport { workspaces })
    }

    pub fn cleanup_stale_workspaces(&self) -> AdmResult<ArchiveWorkspaceCleanupReport> {
        let before = self.inspect_workspaces()?;
        let mut removed = Vec::new();
        let mut skipped_active = Vec::new();
        for workspace in &before.workspaces {
            if workspace.active_lock {
                skipped_active.push(workspace.clone());
                continue;
            }
            let safe_root = ensure_within_root(self.workspaces_root(), &workspace.root)?;
            fs::remove_dir_all(&safe_root)?;
            removed.push(workspace.clone());
        }
        Ok(ArchiveWorkspaceCleanupReport {
            before,
            removed,
            skipped_active,
        })
    }

    pub fn create_blank_workspace(&self, session_id: SessionId) -> AdmResult<WorkspaceSession> {
        self.initialize()?;
        let root = self.workspaces_root().join(session_id.as_str());
        fs::create_dir_all(root.join("content"))?;
        Ok(WorkspaceSession {
            session_id,
            root,
            linked_archive_id: None,
        })
    }

    pub fn open_archive_workspace(
        &self,
        archive: &FormalArchive,
        session_id: SessionId,
    ) -> AdmResult<OpenArchiveSession> {
        let lock = ArchiveLock::acquire(&archive.root, session_id.clone())?;
        let root = self.workspaces_root().join(session_id.as_str());
        fs::create_dir_all(root.join("content"))?;
        let workspace = WorkspaceSession {
            session_id,
            root,
            linked_archive_id: Some(archive.manifest.archive_id.clone()),
        };
        copy_dir_contents(&archive.root.join("content"), &workspace.content_root())?;
        Ok(OpenArchiveSession { workspace, lock })
    }

    pub fn write_workspace_text(
        &self,
        workspace: &WorkspaceSession,
        relative_path: impl AsRef<Path>,
        content: &str,
    ) -> AdmResult<PathBuf> {
        self.write_workspace_bytes(workspace, relative_path, content.as_bytes())
    }

    pub fn write_workspace_bytes(
        &self,
        workspace: &WorkspaceSession,
        relative_path: impl AsRef<Path>,
        content: &[u8],
    ) -> AdmResult<PathBuf> {
        let content_root = workspace.content_root();
        let target = ensure_within_root(&content_root, relative_path)?;
        atomic_write(&target, content)?;
        Ok(target)
    }

    pub fn read_workspace_text(
        &self,
        workspace: &WorkspaceSession,
        relative_path: impl AsRef<Path>,
    ) -> AdmResult<String> {
        let content_root = workspace.content_root();
        let target = ensure_within_root(&content_root, relative_path)?;
        fs::read_to_string(target).map_err(AdmError::from)
    }

    pub fn commit_workspace(
        &self,
        archive: &FormalArchive,
        workspace: &WorkspaceSession,
        lock: &ArchiveLock,
    ) -> AdmResult<ArchiveCommitReport> {
        if lock.path.parent() != Some(archive.root.as_path()) {
            return Err(AdmError::new(
                AdmErrorKind::AlreadyLocked,
                "archive lock does not belong to this formal archive",
            ));
        }

        if workspace.linked_archive_id.as_ref() != Some(&archive.manifest.archive_id) {
            return Err(AdmError::conflict(
                "workspace is not linked to this formal archive",
            ));
        }

        let workspace_content = workspace.content_root();
        let archive_content = archive.root.join("content");
        replace_dir(&workspace_content, &archive_content)?;

        let mut written_files = Vec::new();
        for source in collect_files(&workspace_content)? {
            let relative = source.strip_prefix(&workspace_content).map_err(|error| {
                AdmError::new(AdmErrorKind::PathEscape, error.to_string())
                    .with_context(format!("source={}", source.display()))
            })?;
            let target = ensure_within_root(&archive_content, relative)?;
            let bytes = fs::read(&source)?;
            atomic_write(&target, &bytes)?;
            written_files.push(relative.to_path_buf());
        }

        Ok(ArchiveCommitReport {
            archive_id: archive.manifest.archive_id.clone(),
            session_id: workspace.session_id.clone(),
            written_files,
        })
    }

    pub fn export_archive_package(
        &self,
        archive: &FormalArchive,
        target_file: impl AsRef<Path>,
    ) -> AdmResult<PathBuf> {
        let (payload, file_count) = render_package_payload(archive)?;
        let payload_hash = ContentHash::from_bytes(payload.as_bytes());
        let mut package = String::from("ADM_PACKAGE_V3\n");
        package.push_str("format_version=3\n");
        package.push_str(&format!("payload_hash={}\n", payload_hash.as_str()));
        package.push_str(&format!("file_count={file_count}\n"));
        package.push_str("[payload]\n");
        package.push_str(&payload);
        atomic_write(&target_file, package.as_bytes())?;
        Ok(target_file.as_ref().to_path_buf())
    }

    pub fn import_archive_package(
        &self,
        package_file: impl AsRef<Path>,
    ) -> AdmResult<FormalArchive> {
        self.initialize()?;
        let package = fs::read_to_string(package_file)?;
        let imported = ImportedPackage::parse(&package)?;
        imported.validate_file_hashes()?;
        let root = self
            .archives_root()
            .join(imported.manifest.archive_id.as_str());
        if root.exists() {
            return Err(AdmError::conflict(format!(
                "archive {} already exists",
                imported.manifest.archive_id
            )));
        }
        fs::create_dir_all(root.join("content"))?;
        atomic_write(
            root.join("manifest.adm"),
            imported.manifest.to_manifest_text().as_bytes(),
        )?;
        for file in imported.files {
            let target = ensure_within_root(root.join("content"), &file.relative_path)?;
            let actual_hash = ContentHash::from_bytes(&file.bytes);
            if actual_hash.as_str() != file.hash {
                return Err(AdmError::validation(format!(
                    "package hash mismatch for {}",
                    file.relative_path.display()
                )));
            }
            atomic_write(target, &file.bytes)?;
        }
        self.load_archive(&imported.manifest.archive_id)
    }

    fn archives_root(&self) -> PathBuf {
        self.root.join("archives")
    }

    fn workspaces_root(&self) -> PathBuf {
        self.root.join("workspaces")
    }

    fn active_lock_session_ids(&self) -> AdmResult<HashSet<String>> {
        let mut active = HashSet::new();
        if !self.archives_root().exists() {
            return Ok(active);
        }
        for entry in fs::read_dir(self.archives_root())? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let lock_path = entry.path().join(".archive_lock");
            if !lock_path.exists() {
                continue;
            }
            let owner = fs::read_to_string(lock_path)?;
            for line in owner.lines() {
                if let Some(session_id) = line.strip_prefix("session_id=") {
                    if !session_id.trim().is_empty() {
                        active.insert(session_id.to_string());
                    }
                }
            }
        }
        Ok(active)
    }
}

pub fn inspect_archive_package(
    package_file: impl AsRef<Path>,
) -> AdmResult<ArchivePackageDoctorReport> {
    let package_file = package_file.as_ref().to_path_buf();
    let mut report = ArchivePackageDoctorReport {
        package_file: package_file.clone(),
        package_present: package_file.is_file(),
        package_bytes: 0,
        format_label: None,
        format_version: None,
        expected_payload_hash: None,
        actual_payload_hash: None,
        declared_file_count: None,
        manifest: None,
        files: Vec::new(),
        issues: Vec::new(),
    };

    if !report.package_present {
        report.issues.push("package file is missing".to_string());
        return Ok(report);
    }

    report.package_bytes = fs::metadata(&package_file)?.len();
    let package = fs::read_to_string(&package_file)?;
    match ImportedPackage::parse(&package) {
        Ok(imported) => {
            report.format_label = Some(imported.format_label.clone());
            report.format_version = Some(imported.format_version);
            report.expected_payload_hash = imported.payload_hash.clone();
            report.actual_payload_hash = imported.actual_payload_hash.clone();
            report.declared_file_count = imported.declared_file_count;
            report.manifest = Some(imported.manifest.clone());
            report.files = imported.file_inspections();
            for file in &report.files {
                if !file.hash_matches {
                    report.issues.push(format!(
                        "file hash mismatch for {}",
                        file.relative_path.display()
                    ));
                }
            }
            if report.files.is_empty() {
                report
                    .issues
                    .push("package contains no content files".to_string());
            }
        }
        Err(error) => report.issues.push(error.to_string()),
    }

    Ok(report)
}

fn copy_dir_contents(source_root: &Path, target_root: &Path) -> AdmResult<()> {
    if !source_root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(source_root)? {
        let entry = entry?;
        let source = entry.path();
        let relative = source.strip_prefix(source_root).map_err(|error| {
            AdmError::new(AdmErrorKind::PathEscape, error.to_string())
                .with_context(format!("source={}", source.display()))
        })?;
        let target = ensure_within_root(target_root, relative)?;
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir_contents(&source, &target)?;
        } else {
            let bytes = fs::read(&source)?;
            atomic_write(&target, &bytes)?;
        }
    }
    Ok(())
}

fn replace_dir(source_root: &Path, target_root: &Path) -> AdmResult<()> {
    if target_root.exists() {
        fs::remove_dir_all(target_root)?;
    }
    fs::create_dir_all(target_root)?;
    copy_dir_contents(source_root, target_root)
}

fn collect_files(root: &Path) -> AdmResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    if !root.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            files.extend(collect_files(&path)?);
        } else {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn render_package_payload(archive: &FormalArchive) -> AdmResult<(String, usize)> {
    let mut payload = String::from("[manifest]\n");
    payload.push_str(&archive.manifest.to_manifest_text());
    payload.push_str("[files]\n");

    let mut file_count = 0;
    let content_root = archive.root.join("content");
    for source in collect_files(&content_root)? {
        let relative = source.strip_prefix(&content_root).map_err(|error| {
            AdmError::new(AdmErrorKind::PathEscape, error.to_string())
                .with_context(format!("source={}", source.display()))
        })?;
        let bytes = fs::read(&source)?;
        let hash = ContentHash::from_bytes(&bytes);
        payload.push_str("[file]\n");
        payload.push_str(&format!("path={}\n", relative.to_string_lossy()));
        payload.push_str(&format!("hash={}\n", hash.as_str()));
        payload.push_str(&format!("bytes={}\n", encode_hex(&bytes)));
        file_count += 1;
    }

    Ok((payload, file_count))
}

#[derive(Debug, Clone)]
struct ImportedPackage {
    format_label: String,
    format_version: u32,
    payload_hash: Option<String>,
    actual_payload_hash: Option<ContentHash>,
    declared_file_count: Option<usize>,
    manifest: ArchiveManifest,
    files: Vec<ImportedFile>,
}

#[derive(Debug, Clone)]
struct ImportedFile {
    relative_path: PathBuf,
    hash: String,
    bytes: Vec<u8>,
}

impl ImportedPackage {
    fn parse(package: &str) -> AdmResult<Self> {
        let Some(header) = package.lines().next() else {
            return Err(AdmError::validation("unsupported package header"));
        };
        match header {
            "ADM_PACKAGE_V2" => Self::parse_v2(package),
            "ADM_PACKAGE_V3" => Self::parse_v3(package),
            _ => Err(AdmError::validation("unsupported package header")),
        }
    }

    fn parse_v2(package: &str) -> AdmResult<Self> {
        let payload = package
            .strip_prefix("ADM_PACKAGE_V2\n")
            .ok_or_else(|| AdmError::validation("package missing V2 payload"))?;
        let (manifest, files) = Self::parse_payload(payload)?;
        Ok(Self {
            format_label: "ADM_PACKAGE_V2".to_string(),
            format_version: 2,
            payload_hash: None,
            actual_payload_hash: None,
            declared_file_count: None,
            manifest,
            files,
        })
    }

    fn parse_v3(package: &str) -> AdmResult<Self> {
        let (header, payload) = package
            .split_once("\n[payload]\n")
            .ok_or_else(|| AdmError::validation("package missing payload section"))?;
        let mut format_version = None;
        let mut payload_hash = None;
        let mut file_count = None;

        for line in header.lines().skip(1) {
            let Some((key, value)) = line.split_once('=') else {
                return Err(AdmError::validation(format!(
                    "invalid package header line: {line}"
                )));
            };
            match key {
                "format_version" => {
                    format_version = Some(value.parse::<u32>().map_err(|error| {
                        AdmError::validation(format!("invalid package format_version: {error}"))
                    })?);
                }
                "payload_hash" => payload_hash = Some(value.to_string()),
                "file_count" => {
                    file_count = Some(value.parse::<usize>().map_err(|error| {
                        AdmError::validation(format!("invalid package file_count: {error}"))
                    })?);
                }
                _ => {}
            }
        }

        if format_version != Some(3) {
            return Err(AdmError::unsupported("unsupported package format version"));
        }
        let expected_hash =
            payload_hash.ok_or_else(|| AdmError::validation("package missing payload_hash"))?;
        let actual_hash = ContentHash::from_bytes(payload.as_bytes());
        if actual_hash.as_str() != expected_hash {
            return Err(AdmError::validation("package payload hash mismatch"));
        }

        let (manifest, files) = Self::parse_payload(payload)?;
        let expected_file_count =
            file_count.ok_or_else(|| AdmError::validation("package missing file_count"))?;
        if files.len() != expected_file_count {
            return Err(AdmError::validation(format!(
                "package file count mismatch: expected {}, actual {}",
                expected_file_count,
                files.len()
            )));
        }

        Ok(Self {
            format_label: "ADM_PACKAGE_V3".to_string(),
            format_version: 3,
            payload_hash: Some(expected_hash),
            actual_payload_hash: Some(actual_hash),
            declared_file_count: Some(expected_file_count),
            manifest,
            files,
        })
    }

    fn parse_payload(payload: &str) -> AdmResult<(ArchiveManifest, Vec<ImportedFile>)> {
        let mut lines = payload.lines().peekable();
        match lines.next() {
            Some("[manifest]") => {}
            _ => return Err(AdmError::validation("package missing manifest section")),
        }

        let mut manifest_text = String::new();
        while let Some(line) = lines.peek().copied() {
            if line == "[files]" {
                lines.next();
                break;
            }
            manifest_text.push_str(line);
            manifest_text.push('\n');
            lines.next();
        }
        let manifest = ArchiveManifest::from_manifest_text(&manifest_text)?;

        let mut files = Vec::new();
        while let Some(line) = lines.next() {
            if line.trim().is_empty() {
                continue;
            }
            if line != "[file]" {
                return Err(AdmError::validation(format!(
                    "unexpected package line: {line}"
                )));
            }
            let path_line = lines
                .next()
                .ok_or_else(|| AdmError::validation("package file missing path"))?;
            let hash_line = lines
                .next()
                .ok_or_else(|| AdmError::validation("package file missing hash"))?;
            let bytes_line = lines
                .next()
                .ok_or_else(|| AdmError::validation("package file missing bytes"))?;
            let path = path_line
                .strip_prefix("path=")
                .ok_or_else(|| AdmError::validation("invalid package path line"))?;
            let hash = hash_line
                .strip_prefix("hash=")
                .ok_or_else(|| AdmError::validation("invalid package hash line"))?
                .to_string();
            let bytes_hex = bytes_line
                .strip_prefix("bytes=")
                .ok_or_else(|| AdmError::validation("invalid package bytes line"))?;
            files.push(ImportedFile {
                relative_path: PathBuf::from(path),
                hash,
                bytes: decode_hex(bytes_hex)?,
            });
        }

        Ok((manifest, files))
    }

    fn file_inspections(&self) -> Vec<ArchivePackageFileInspection> {
        self.files
            .iter()
            .map(|file| {
                let actual_hash = ContentHash::from_bytes(&file.bytes);
                let hash_matches = actual_hash.as_str() == file.hash;
                ArchivePackageFileInspection {
                    relative_path: file.relative_path.clone(),
                    expected_hash: file.hash.clone(),
                    actual_hash,
                    bytes: file.bytes.len(),
                    hash_matches,
                }
            })
            .collect()
    }

    fn validate_file_hashes(&self) -> AdmResult<()> {
        for file in self.file_inspections() {
            if !file.hash_matches {
                return Err(AdmError::validation(format!(
                    "package hash mismatch for {}",
                    file.relative_path.display()
                )));
            }
        }
        Ok(())
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[(byte >> 4) as usize]));
        encoded.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    encoded
}

fn decode_hex(encoded: &str) -> AdmResult<Vec<u8>> {
    if !encoded.len().is_multiple_of(2) {
        return Err(AdmError::validation("hex payload must have even length"));
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    let chars = encoded.as_bytes();
    let mut index = 0;
    while index < chars.len() {
        let high = hex_value(chars[index])?;
        let low = hex_value(chars[index + 1])?;
        bytes.push((high << 4) | low);
        index += 2;
    }
    Ok(bytes)
}

fn hex_value(byte: u8) -> AdmResult<u8> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(AdmError::validation("invalid hex character")),
    }
}

pub struct OpenArchiveSession {
    pub workspace: WorkspaceSession,
    pub lock: ArchiveLock,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveCommitReport {
    pub archive_id: ArchiveId,
    pub session_id: SessionId,
    pub written_files: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm_archive_{name}_{}_{}",
            std::process::id(),
            UtcTimestamp::now().as_millis()
        ))
    }

    #[test]
    fn archive_lock_is_exclusive_and_released_on_drop() {
        let root = temp_root("lock");
        fs::create_dir_all(&root).unwrap();
        let first = ArchiveLock::acquire(&root, SessionId::generate()).expect("first lock");
        let second = ArchiveLock::acquire(&root, SessionId::generate());
        assert!(second.is_err());
        drop(first);
        ArchiveLock::acquire(&root, SessionId::generate()).expect("lock after drop");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn repository_creates_archive_and_workspace() {
        let root = temp_root("repo");
        let repo = ArchiveRepository::new(&root);
        let archive = repo.create_archive("Test Project").expect("archive");
        let session = repo
            .open_archive_workspace(&archive, SessionId::generate())
            .expect("workspace");
        assert!(archive.root.join("manifest.adm").exists());
        assert!(session.workspace.root.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn repository_cleans_stale_workspaces_but_keeps_active_locked_workspace() {
        let root = temp_root("workspace_cleanup");
        let repo = ArchiveRepository::new(&root);
        let archive = repo.create_archive("Workspace Cleanup").expect("archive");
        let active = repo
            .open_archive_workspace(&archive, SessionId::generate())
            .expect("active workspace");
        repo.write_workspace_text(&active.workspace, "active.txt", "active")
            .expect("active write");
        let stale = repo
            .create_blank_workspace(SessionId::generate())
            .expect("stale workspace");
        repo.write_workspace_text(&stale, "stale.txt", "stale")
            .expect("stale write");

        let before = repo.inspect_workspaces().expect("inspect");
        assert_eq!(before.workspace_count(), 2);
        assert_eq!(before.active_count(), 1);
        assert_eq!(before.stale_count(), 1);

        let cleanup = repo.cleanup_stale_workspaces().expect("cleanup");
        assert_eq!(cleanup.removed_count(), 1);
        assert_eq!(cleanup.skipped_active_count(), 1);
        assert_eq!(cleanup.removed[0].session_id, stale.session_id.as_str());
        assert!(active.workspace.root.exists());
        assert!(!stale.root.exists());
        assert!(cleanup.render().contains("removed_count=1"));
        assert!(cleanup.render().contains("skipped_active_count=1"));

        drop(active);
        let second = repo.cleanup_stale_workspaces().expect("second cleanup");
        assert_eq!(second.removed_count(), 1);
        assert!(!second.removed[0].active_lock);
        assert!(!archive.root.join(".archive_lock").exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_manifest_round_trips_and_archive_can_be_loaded() {
        let root = temp_root("load");
        let repo = ArchiveRepository::new(&root);
        let archive = repo.create_archive("Loadable Project").expect("archive");
        let loaded = repo
            .load_archive(&archive.manifest.archive_id)
            .expect("loaded");
        let archives = repo.list_archives().expect("archives");

        assert_eq!(loaded.manifest, archive.manifest);
        assert_eq!(archives.len(), 1);
        assert_eq!(archives[0].manifest.display_name, "Loadable Project");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn commit_workspace_writes_content_to_formal_archive() {
        let root = temp_root("commit");
        let repo = ArchiveRepository::new(&root);
        let archive = repo.create_archive("Test Project").expect("archive");
        let open = repo
            .open_archive_workspace(&archive, SessionId::generate())
            .expect("workspace");

        repo.write_workspace_text(&open.workspace, "design/state.txt", "accepted design")
            .expect("workspace write");
        let report = repo
            .commit_workspace(&archive, &open.workspace, &open.lock)
            .expect("commit");

        assert_eq!(report.archive_id, archive.manifest.archive_id);
        assert_eq!(
            report.written_files,
            vec![PathBuf::from("design/state.txt")]
        );
        assert_eq!(
            fs::read_to_string(archive.root.join("content/design/state.txt")).unwrap(),
            "accepted design"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn commit_workspace_requires_linked_archive() {
        let root = temp_root("commit_unlinked");
        let repo = ArchiveRepository::new(&root);
        let archive = repo.create_archive("Test Project").expect("archive");
        let workspace = repo
            .create_blank_workspace(SessionId::generate())
            .expect("workspace");
        let lock = ArchiveLock::acquire(&archive.root, SessionId::generate()).expect("lock");

        let result = repo.commit_workspace(&archive, &workspace, &lock);
        assert!(result.is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn commit_workspace_removes_files_deleted_from_workspace() {
        let root = temp_root("delete_sync");
        let repo = ArchiveRepository::new(&root);
        let archive = repo.create_archive("Test Project").expect("archive");
        let open = repo
            .open_archive_workspace(&archive, SessionId::generate())
            .expect("workspace");

        repo.write_workspace_text(&open.workspace, "stale.txt", "stale")
            .expect("workspace write");
        repo.commit_workspace(&archive, &open.workspace, &open.lock)
            .expect("commit");
        fs::remove_file(open.workspace.content_root().join("stale.txt")).unwrap();
        repo.commit_workspace(&archive, &open.workspace, &open.lock)
            .expect("second commit");

        assert!(!archive.root.join("content/stale.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn export_archive_package_writes_single_text_package() {
        let root = temp_root("export");
        let repo = ArchiveRepository::new(&root);
        let archive = repo.create_archive("Test Project").expect("archive");
        let open = repo
            .open_archive_workspace(&archive, SessionId::generate())
            .expect("workspace");
        repo.write_workspace_text(&open.workspace, "design/state.txt", "ready")
            .expect("workspace write");
        repo.commit_workspace(&archive, &open.workspace, &open.lock)
            .expect("commit");

        let package_path = root.join("export.admproj");
        repo.export_archive_package(&archive, &package_path)
            .expect("export");
        let package = fs::read_to_string(package_path).unwrap();
        assert!(package.contains("ADM_PACKAGE_V3"));
        assert!(package.contains("payload_hash=fnv64:"));
        assert!(package.contains("file_count=1"));
        assert!(
            package.contains("path=design\\state.txt") || package.contains("path=design/state.txt")
        );
        assert!(package.contains("hash=fnv64:"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn inspect_archive_package_reports_ready_package() {
        let root = temp_root("inspect_ready");
        let repo = ArchiveRepository::new(&root);
        let archive = repo.create_archive("Inspectable Project").expect("archive");
        let open = repo
            .open_archive_workspace(&archive, SessionId::generate())
            .expect("workspace");
        repo.write_workspace_text(&open.workspace, "design/state.txt", "ready")
            .expect("workspace write");
        repo.commit_workspace(&archive, &open.workspace, &open.lock)
            .expect("commit");
        let package_path = root.join("inspectable.admproj");
        repo.export_archive_package(&archive, &package_path)
            .expect("export");

        let report = inspect_archive_package(&package_path).expect("inspect");
        assert!(report.ready());
        assert_eq!(report.format_label.as_deref(), Some("ADM_PACKAGE_V3"));
        assert_eq!(report.declared_file_count, Some(1));
        assert_eq!(
            report
                .manifest
                .as_ref()
                .map(|manifest| &manifest.display_name),
            Some(&"Inspectable Project".to_string())
        );
        assert_eq!(report.files.len(), 1);
        assert!(report.files[0].hash_matches);
        let rendered = report.render();
        assert!(rendered.contains("ready=true"));
        assert!(rendered.contains("payload_hash_match=true"));
        assert!(
            rendered.contains("path=design\\state.txt")
                || rendered.contains("path=design/state.txt")
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_archive_package_restores_binary_content() {
        let source_root = temp_root("import_source");
        let source_repo = ArchiveRepository::new(&source_root);
        let archive = source_repo
            .create_archive("Binary Project")
            .expect("archive");
        let open = source_repo
            .open_archive_workspace(&archive, SessionId::generate())
            .expect("workspace");
        source_repo
            .write_workspace_bytes(&open.workspace, "bin/data.bytes", &[0, 1, 2, 255])
            .expect("binary write");
        source_repo
            .commit_workspace(&archive, &open.workspace, &open.lock)
            .expect("commit");
        let package_path = source_root.join("binary.admproj");
        source_repo
            .export_archive_package(&archive, &package_path)
            .expect("export");

        let target_root = temp_root("import_target");
        let target_repo = ArchiveRepository::new(&target_root);
        let imported = target_repo
            .import_archive_package(&package_path)
            .expect("import");
        assert_eq!(imported.manifest.display_name, "Binary Project");
        assert_eq!(
            fs::read(imported.root.join("content/bin/data.bytes")).unwrap(),
            vec![0, 1, 2, 255]
        );

        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(target_root);
    }

    #[test]
    fn import_archive_package_rejects_payload_hash_mismatch() {
        let source_root = temp_root("payload_hash_source");
        let source_repo = ArchiveRepository::new(&source_root);
        let archive = source_repo
            .create_archive("Payload Hash Project")
            .expect("archive");
        let open = source_repo
            .open_archive_workspace(&archive, SessionId::generate())
            .expect("workspace");
        source_repo
            .write_workspace_text(&open.workspace, "design/state.txt", "ready")
            .expect("workspace write");
        source_repo
            .commit_workspace(&archive, &open.workspace, &open.lock)
            .expect("commit");

        let package_path = source_root.join("package.admproj");
        source_repo
            .export_archive_package(&archive, &package_path)
            .expect("export");
        let package = fs::read_to_string(&package_path)
            .unwrap()
            .replace("bytes=7265616479\n", "bytes=7265616478\n");
        let damaged_path = source_root.join("damaged.admproj");
        fs::write(&damaged_path, package).unwrap();

        let target_root = temp_root("payload_hash_target");
        let target_repo = ArchiveRepository::new(&target_root);
        let error = target_repo
            .import_archive_package(&damaged_path)
            .expect_err("damaged payload must be rejected");
        assert_eq!(error.kind(), &AdmErrorKind::Validation);
        assert!(error.message().contains("payload hash"));

        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(target_root);
    }

    #[test]
    fn import_archive_package_validates_file_hashes_before_creating_archive() {
        let source_root = temp_root("file_hash_source");
        fs::create_dir_all(&source_root).unwrap();
        let manifest = ArchiveManifest::new("Wrong Hash Project").expect("manifest");
        let bytes = b"ready".to_vec();
        let wrong_hash = ContentHash::from_bytes(b"not ready");
        let package = format!(
            "ADM_PACKAGE_V2\n[manifest]\n{}[files]\n[file]\npath=design/state.txt\nhash={}\nbytes={}\n",
            manifest.to_manifest_text(),
            wrong_hash.as_str(),
            encode_hex(&bytes)
        );
        let package_path = source_root.join("wrong-hash.admproj");
        fs::write(&package_path, package).unwrap();

        let report = inspect_archive_package(&package_path).expect("inspect");
        assert!(!report.ready());
        assert_eq!(report.files.len(), 1);
        assert!(!report.files[0].hash_matches);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.contains("file hash mismatch"))
        );

        let target_root = temp_root("file_hash_target");
        let target_repo = ArchiveRepository::new(&target_root);
        let error = target_repo
            .import_archive_package(&package_path)
            .expect_err("wrong file hash must be rejected");
        assert_eq!(error.kind(), &AdmErrorKind::Validation);
        assert!(error.message().contains("package hash mismatch"));
        assert!(
            !target_root
                .join("archives")
                .join(manifest.archive_id.as_str())
                .exists()
        );

        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(target_root);
    }

    #[test]
    fn import_archive_package_accepts_legacy_v2_package() {
        let source_root = temp_root("legacy_v2_source");
        fs::create_dir_all(&source_root).unwrap();
        let manifest = ArchiveManifest::new("Legacy Project").expect("manifest");
        let bytes = vec![0, 1, 2, 255];
        let hash = ContentHash::from_bytes(&bytes);
        let package = format!(
            "ADM_PACKAGE_V2\n[manifest]\n{}[files]\n[file]\npath=legacy/data.bin\nhash={}\nbytes={}\n",
            manifest.to_manifest_text(),
            hash.as_str(),
            encode_hex(&bytes)
        );
        let package_path = source_root.join("legacy.admproj");
        fs::write(&package_path, package).unwrap();

        let target_root = temp_root("legacy_v2_target");
        let target_repo = ArchiveRepository::new(&target_root);
        let imported = target_repo
            .import_archive_package(&package_path)
            .expect("legacy import");

        assert_eq!(imported.manifest.display_name, "Legacy Project");
        assert_eq!(
            fs::read(imported.root.join("content/legacy/data.bin")).unwrap(),
            bytes
        );

        let _ = fs::remove_dir_all(source_root);
        let _ = fs::remove_dir_all(target_root);
    }
}
