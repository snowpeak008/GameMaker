#![forbid(unsafe_code)]

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use adm_new_contracts::save::{
    ArchiveLock, AutosaveState, DraftMeta, FileMap, SaveIndex, SaveManifest, SnapshotManifest,
};
use adm_new_foundation::{
    AdmError, AdmResult, FileManifestEntry, ensure_relative_path, file_manifest_entry,
    sanitize_identifier, write_text_atomic,
};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub mod pipeline_checkpoint;
pub use pipeline_checkpoint::PipelineCheckpointRepository;

pub const CRATE_NAME: &str = "adm-new-storage";

pub fn crate_ready() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoot {
    root: PathBuf,
}

impl ProjectRoot {
    pub fn new(root: impl AsRef<Path>) -> AdmResult<Self> {
        fs::create_dir_all(root.as_ref())?;
        let root = root.as_ref().canonicalize().map_err(|error| {
            AdmError::new(format!("failed to canonicalize project root: {error}"))
        })?;
        Ok(Self { root })
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn resolve_relative(&self, relative: &str) -> AdmResult<PathBuf> {
        ensure_relative_path(&self.root, relative)
    }

    pub fn saves_dir(&self) -> PathBuf {
        self.root.join("saves")
    }

    pub fn drafts_dir(&self) -> PathBuf {
        self.root.join("drafts")
    }
}

#[derive(Debug, Clone)]
pub struct JsonRepository<T> {
    path: PathBuf,
    project_root: PathBuf,
    _marker: PhantomData<T>,
}

impl<T> JsonRepository<T>
where
    T: Serialize + DeserializeOwned,
{
    pub fn new(project_root: &ProjectRoot, relative_path: &str) -> AdmResult<Self> {
        let path = project_root.resolve_relative(relative_path)?;
        Ok(Self {
            path,
            project_root: project_root.path().to_path_buf(),
            _marker: PhantomData,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read(&self) -> AdmResult<Option<T>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let text = fs::read_to_string(&self.path)?;
        let value = serde_json::from_str(&text).map_err(|error| {
            AdmError::new(format!("invalid JSON at {}: {error}", self.path.display()))
        })?;
        Ok(Some(value))
    }

    pub fn read_required(&self) -> AdmResult<T> {
        self.read()?.ok_or_else(|| {
            AdmError::new(format!(
                "required JSON file is missing: {}",
                self.path.display()
            ))
        })
    }

    pub fn write(&self, value: &T) -> AdmResult<FileManifestEntry> {
        let text = serde_json::to_string_pretty(value)
            .map_err(|error| AdmError::new(format!("failed to serialize JSON: {error}")))?;
        write_text_atomic(&self.path, &(text + "\n"))?;
        self.manifest_entry()
    }

    pub fn manifest_entry(&self) -> AdmResult<FileManifestEntry> {
        let relative = self
            .path
            .strip_prefix(&self.project_root)
            .map_err(|error| AdmError::new(format!("path is outside project root: {error}")))?
            .to_string_lossy()
            .replace('\\', "/");
        file_manifest_entry(&self.project_root, &relative)
    }
}

#[derive(Debug, Clone)]
pub struct SaveIndexRepository {
    repo: JsonRepository<SaveIndex>,
}

impl SaveIndexRepository {
    pub fn new(project_root: &ProjectRoot) -> AdmResult<Self> {
        Ok(Self {
            repo: JsonRepository::new(project_root, "saves/save_index.json")?,
        })
    }

    pub fn read(&self) -> AdmResult<Option<SaveIndex>> {
        self.repo.read()
    }

    pub fn write(&self, value: &SaveIndex) -> AdmResult<FileManifestEntry> {
        self.repo.write(value)
    }

    pub fn path(&self) -> &Path {
        self.repo.path()
    }
}

#[derive(Debug, Clone)]
pub struct SaveArchiveRepository {
    project_root: ProjectRoot,
    save_id: String,
}

impl SaveArchiveRepository {
    pub fn new(project_root: &ProjectRoot, save_id: &str) -> AdmResult<Self> {
        Ok(Self {
            project_root: project_root.clone(),
            save_id: safe_component("save_id", save_id)?,
        })
    }

    pub fn manifest(&self) -> AdmResult<JsonRepository<SaveManifest>> {
        JsonRepository::new(
            &self.project_root,
            &format!("saves/{}/manifest.json", self.save_id),
        )
    }

    pub fn archive_lock(&self) -> AdmResult<ArchiveLockRepository> {
        ArchiveLockRepository::new(&self.project_root, &self.save_id)
    }
}

#[derive(Debug, Clone)]
pub struct DraftWorkspaceRepository {
    project_root: ProjectRoot,
    session_id: String,
}

impl DraftWorkspaceRepository {
    pub fn new(project_root: &ProjectRoot, session_id: &str) -> AdmResult<Self> {
        Ok(Self {
            project_root: project_root.clone(),
            session_id: safe_component("session_id", session_id)?,
        })
    }

    pub fn draft_meta(&self) -> AdmResult<JsonRepository<DraftMeta>> {
        JsonRepository::new(
            &self.project_root,
            &format!("drafts/{}/draft_meta.json", self.session_id),
        )
    }

    pub fn autosave_state(&self) -> AdmResult<JsonRepository<AutosaveState>> {
        JsonRepository::new(
            &self.project_root,
            &format!("drafts/{}/autosave_state.json", self.session_id),
        )
    }

    pub fn draft_file_map(&self) -> AdmResult<JsonRepository<FileMap>> {
        JsonRepository::new(
            &self.project_root,
            &format!("drafts/{}/draft_file_map.json", self.session_id),
        )
    }

    pub fn snapshot(&self, snapshot_name: &str) -> AdmResult<SnapshotRepository> {
        SnapshotRepository::new(&self.project_root, &self.session_id, snapshot_name)
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotRepository {
    project_root: ProjectRoot,
    session_id: String,
    snapshot_name: String,
}

impl SnapshotRepository {
    pub fn new(
        project_root: &ProjectRoot,
        session_id: &str,
        snapshot_name: &str,
    ) -> AdmResult<Self> {
        Ok(Self {
            project_root: project_root.clone(),
            session_id: safe_component("session_id", session_id)?,
            snapshot_name: safe_component("snapshot_name", snapshot_name)?,
        })
    }

    pub fn manifest(&self) -> AdmResult<JsonRepository<SnapshotManifest>> {
        JsonRepository::new(
            &self.project_root,
            &format!(
                "drafts/{}/snapshots/{}/snapshot_manifest.json",
                self.session_id, self.snapshot_name
            ),
        )
    }

    pub fn file_map(&self) -> AdmResult<JsonRepository<FileMap>> {
        JsonRepository::new(
            &self.project_root,
            &format!(
                "drafts/{}/snapshots/{}/snapshot_file_map.json",
                self.session_id, self.snapshot_name
            ),
        )
    }
}

#[derive(Debug, Clone)]
pub struct ArchiveLockRepository {
    repo: JsonRepository<ArchiveLock>,
}

impl ArchiveLockRepository {
    pub fn new(project_root: &ProjectRoot, save_id: &str) -> AdmResult<Self> {
        let save_id = safe_component("save_id", save_id)?;
        Ok(Self {
            repo: JsonRepository::new(project_root, &format!("saves/{save_id}/.archive_lock"))?,
        })
    }

    pub fn read(&self) -> AdmResult<Option<ArchiveLock>> {
        self.repo.read()
    }

    pub fn try_create(&self, lock: &ArchiveLock) -> AdmResult<bool> {
        if let Some(parent) = self.repo.path().parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(lock)
            .map_err(|error| AdmError::new(format!("failed to serialize archive lock: {error}")))?;
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.repo.path())
        {
            Ok(mut file) => {
                file.write_all(text.as_bytes())?;
                file.write_all(b"\n")?;
                file.sync_all()?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub fn path(&self) -> &Path {
        self.repo.path()
    }
}

fn safe_component(name: &str, value: &str) -> AdmResult<String> {
    let clean = sanitize_identifier(value)?;
    if clean != value {
        Err(AdmError::new(format!(
            "{name} contains non-portable path characters: {value}"
        )))
    } else {
        Ok(clean)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::project::ProjectState;
    use adm_new_contracts::save::{SaveIndexEntry, SaveProgress, WorkspaceState};
    use adm_new_foundation::new_stable_id;

    #[test]
    fn crate_reports_ready() {
        assert!(crate_ready());
        assert_eq!(CRATE_NAME, "adm-new-storage");
    }

    #[test]
    fn storage_project_root_rejects_path_traversal() {
        let root = temp_project_root("path").unwrap();
        assert!(root.resolve_relative("saves/save_index.json").is_ok());
        assert!(root.resolve_relative("../outside.json").is_err());
        cleanup(root);
    }

    #[test]
    fn storage_typed_json_repository_reads_missing_and_roundtrips() {
        let root = temp_project_root("roundtrip").unwrap();
        let repo = SaveIndexRepository::new(&root).unwrap();
        assert!(repo.read().unwrap().is_none());

        let index = SaveIndex {
            schema_version: 1,
            current_save_id: Some("save_1".to_string()),
            saves: vec![SaveIndexEntry {
                save_id: "save_1".to_string(),
                display_name: "Main".to_string(),
                save_type: "manual".to_string(),
                created_by: "test".to_string(),
                reason: "unit".to_string(),
                path: "saves/save_1".to_string(),
                created_at: "2026-07-08T00:00:00".to_string(),
                last_worked_at: "2026-07-08T00:01:00".to_string(),
                progress: SaveProgress {
                    passed: 1,
                    total: 15,
                    label: "1/15".to_string(),
                    ..SaveProgress::default()
                },
                last_transaction_seq: 1,
                locked_by_other: false,
                lock_owner_pid: None,
                lock_owner_session: String::new(),
                integrity_status: "ok".to_string(),
                integrity_message: String::new(),
                workspace_file_count: 1,
                workspace_bytes: 1,
            }],
            updated_at: "2026-07-08T00:02:00".to_string(),
            ..SaveIndex::default()
        };
        let manifest = repo.write(&index).unwrap();
        assert_eq!(manifest.relative_path, "saves/save_index.json");
        assert_eq!(repo.read().unwrap(), Some(index));
        cleanup(root);
    }

    #[test]
    fn storage_typed_json_repository_rejects_invalid_json() {
        let root = temp_project_root("invalid").unwrap();
        let path = root.resolve_relative("saves/save_index.json").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{not json").unwrap();
        let repo = SaveIndexRepository::new(&root).unwrap();
        assert!(repo.read().is_err());
        cleanup(root);
    }

    #[test]
    fn storage_atomic_write_updates_manifest_hash() {
        let root = temp_project_root("atomic").unwrap();
        let repo = SaveIndexRepository::new(&root).unwrap();
        let first = SaveIndex::default();
        let first_manifest = repo.write(&first).unwrap();
        let second = SaveIndex {
            updated_at: "changed".to_string(),
            ..SaveIndex::default()
        };
        let second_manifest = repo.write(&second).unwrap();
        assert_ne!(first_manifest.content_hash, second_manifest.content_hash);
        assert_eq!(repo.read().unwrap(), Some(second));
        cleanup(root);
    }

    #[test]
    fn storage_save_and_draft_repositories_use_expected_paths() {
        let root = temp_project_root("paths").unwrap();
        let save_repo = SaveArchiveRepository::new(&root, "save_1").unwrap();
        let manifest_repo = save_repo.manifest().unwrap();
        assert!(
            manifest_repo
                .path()
                .ends_with(Path::new("saves/save_1/manifest.json"))
        );

        let draft = DraftWorkspaceRepository::new(&root, "session_1").unwrap();
        let meta = DraftMeta {
            schema_version: 1,
            session_id: "session_1".to_string(),
            pid: 123,
            project_root: root.path().display().to_string(),
            draft_root: root
                .resolve_relative("drafts/session_1")
                .unwrap()
                .display()
                .to_string(),
            updated_at: "2026-07-08T00:00:00".to_string(),
            linked_save_id: Some("save_1".to_string()),
            linked_archive_path: "saves/save_1".to_string(),
            workspace_state: WorkspaceState::LinkedSave,
            origin_deleted_save_id: None,
        };
        draft.draft_meta().unwrap().write(&meta).unwrap();
        draft
            .autosave_state()
            .unwrap()
            .write(&ProjectState::empty())
            .unwrap();
        let file_map = FileMap {
            schema_version: 1,
            generated_at: "2026-07-08T00:00:00".to_string(),
            transaction_seq: Some(1),
            files: Vec::new(),
        };
        draft.draft_file_map().unwrap().write(&file_map).unwrap();
        let snapshot = draft.snapshot("snap_1").unwrap();
        snapshot
            .manifest()
            .unwrap()
            .write(&SnapshotManifest {
                schema_version: 1,
                seq: 1,
                event: "manual_save".to_string(),
                stage: None,
                timestamp: "2026-07-08T00:00:00".to_string(),
                message: String::new(),
                file_count: 0,
                added: 0,
                modified: 0,
                removed: 0,
            })
            .unwrap();
        snapshot.file_map().unwrap().write(&file_map).unwrap();

        assert!(draft.draft_meta().unwrap().read_required().is_ok());
        cleanup(root);
    }

    #[test]
    fn storage_archive_lock_create_new_does_not_overwrite() {
        let root = temp_project_root("lock").unwrap();
        let lock_repo = ArchiveLockRepository::new(&root, "save_1").unwrap();
        let lock = ArchiveLock {
            pid: 42,
            session_id: "session_1".to_string(),
            acquired_at: "2026-07-08T00:00:00".to_string(),
            live: None,
            lock_path: None,
        };
        assert!(lock_repo.try_create(&lock).unwrap());
        let second = ArchiveLock {
            pid: 43,
            ..lock.clone()
        };
        assert!(!lock_repo.try_create(&second).unwrap());
        assert_eq!(lock_repo.read().unwrap().unwrap().pid, 42);
        cleanup(root);
    }

    #[test]
    fn storage_rejects_non_portable_save_id() {
        let root = temp_project_root("bad-id").unwrap();
        assert!(SaveArchiveRepository::new(&root, "../save").is_err());
        cleanup(root);
    }

    fn temp_project_root(label: &str) -> AdmResult<ProjectRoot> {
        let dir = std::env::temp_dir().join(format!(
            "adm_new_storage_{label}_{}",
            new_stable_id("root")?
        ));
        ProjectRoot::new(dir)
    }

    fn cleanup(root: ProjectRoot) {
        let _ = fs::remove_dir_all(root.path());
    }
}
