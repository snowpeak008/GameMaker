#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::project::ProjectState;

pub const CONTRACT_FAMILY: &str = "save";
pub const SAVE_SCHEMA_VERSION: u32 = 2;

pub type AutosaveState = ProjectState;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceState {
    LinkedSave,
    #[default]
    Unsaved,
    UnsavedCopyOfDeletedSave,
}

impl WorkspaceState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LinkedSave => "linked_save",
            Self::Unsaved => "unsaved",
            Self::UnsavedCopyOfDeletedSave => "unsaved_copy_of_deleted_save",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveIndex {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub current_save_id: Option<String>,
    #[serde(default)]
    pub saves: Vec<SaveIndexEntry>,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub workspace_state: String,
    #[serde(default)]
    pub draft_updated_at: String,
    #[serde(default)]
    pub origin_deleted_save_id: Option<String>,
    #[serde(default)]
    pub has_autosave: bool,
}

impl Default for SaveIndex {
    fn default() -> Self {
        Self {
            schema_version: SAVE_SCHEMA_VERSION,
            current_save_id: None,
            saves: Vec::new(),
            updated_at: String::new(),
            workspace_state: String::new(),
            draft_updated_at: String::new(),
            origin_deleted_save_id: None,
            has_autosave: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SaveIndexEntry {
    #[serde(default)]
    pub save_id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub save_type: String,
    #[serde(default)]
    pub created_by: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub last_worked_at: String,
    #[serde(default)]
    pub progress: SaveProgress,
    #[serde(default)]
    pub last_transaction_seq: u64,
    #[serde(default)]
    pub locked_by_other: bool,
    #[serde(default)]
    pub lock_owner_pid: Option<u32>,
    #[serde(default)]
    pub lock_owner_session: String,
    #[serde(default)]
    pub integrity_status: String,
    #[serde(default)]
    pub integrity_message: String,
    #[serde(default)]
    pub workspace_file_count: u64,
    #[serde(default)]
    pub workspace_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveProgress {
    #[serde(default)]
    pub passed: u32,
    #[serde(default)]
    pub total: u32,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub design_passed: u32,
    #[serde(default)]
    pub design_total: u32,
    #[serde(default)]
    pub design_label: String,
    #[serde(default)]
    pub pipeline_passed: u32,
    #[serde(default = "default_pipeline_total")]
    pub pipeline_total: u32,
    #[serde(default)]
    pub pipeline_label: String,
}

impl Default for SaveProgress {
    fn default() -> Self {
        Self {
            passed: 0,
            total: 0,
            label: String::new(),
            design_passed: 0,
            design_total: 0,
            design_label: String::new(),
            pipeline_passed: 0,
            pipeline_total: default_pipeline_total(),
            pipeline_label: "0/15".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub save_id: String,
    pub display_name: String,
    #[serde(default)]
    pub save_type: String,
    #[serde(default)]
    pub created_by: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub last_worked_at: String,
    #[serde(default)]
    pub last_transaction_seq: u64,
    #[serde(default)]
    pub progress: SaveProgress,
    #[serde(default)]
    pub change_type: Option<String>,
    #[serde(default)]
    pub requested_version: Option<String>,
    #[serde(default)]
    pub iteration_spec_path: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftMeta {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub project_root: String,
    #[serde(default)]
    pub draft_root: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub linked_save_id: Option<String>,
    #[serde(default)]
    pub linked_archive_path: String,
    #[serde(default)]
    pub workspace_state: WorkspaceState,
    #[serde(default)]
    pub origin_deleted_save_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveLock {
    pub pid: u32,
    pub session_id: String,
    pub acquired_at: String,
    #[serde(default)]
    pub live: Option<bool>,
    #[serde(default)]
    pub lock_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileMap {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub transaction_seq: Option<u64>,
    #[serde(default)]
    pub files: Vec<FileMapEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileMapEntry {
    pub workspace_path: String,
    #[serde(default)]
    pub size_bytes: u64,
    #[serde(default)]
    pub mtime_ns: u64,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub stage: Option<u32>,
    #[serde(default)]
    pub artifact_id: Value,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub source_type: String,
    #[serde(default)]
    pub reference_manifest: String,
    #[serde(default)]
    pub latest_transaction_seq: Option<u64>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapshotManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub seq: u64,
    pub event: String,
    #[serde(default)]
    pub stage: Option<u32>,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub file_count: u32,
    #[serde(default)]
    pub added: u32,
    #[serde(default)]
    pub modified: u32,
    #[serde(default)]
    pub removed: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileMapDelta {
    #[serde(default)]
    pub added: Vec<FileMapEntry>,
    #[serde(default)]
    pub modified: Vec<FileMapChange>,
    #[serde(default)]
    pub removed: Vec<FileMapEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileMapChange {
    pub before: FileMapEntry,
    pub after: FileMapEntry,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub seq: u64,
    #[serde(default)]
    pub save_id: String,
    pub event: String,
    #[serde(default)]
    pub stage: Option<u32>,
    #[serde(default)]
    pub timestamp: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub progress: SaveProgress,
}

fn default_schema_version() -> u32 {
    SAVE_SCHEMA_VERSION
}

fn default_pipeline_total() -> u32 {
    15
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::DEFAULT_PROJECT_NAME;

    #[test]
    fn save_index_roundtrip_preserves_current_and_progress() {
        let index = SaveIndex {
            schema_version: SAVE_SCHEMA_VERSION,
            current_save_id: Some("save-1".to_string()),
            saves: vec![SaveIndexEntry {
                save_id: "save-1".to_string(),
                display_name: "Main".to_string(),
                save_type: "manual".to_string(),
                created_by: "design_workbench".to_string(),
                reason: "user_save".to_string(),
                path: "saves/save-1".to_string(),
                created_at: "2026-07-08T00:00:00".to_string(),
                last_worked_at: "2026-07-08T00:01:00".to_string(),
                progress: SaveProgress {
                    passed: 1,
                    total: 15,
                    label: "已通过 1/15".to_string(),
                    design_passed: 1,
                    design_total: 15,
                    design_label: "1/15".to_string(),
                    ..SaveProgress::default()
                },
                ..SaveIndexEntry::default()
            }],
            updated_at: "2026-07-08T00:02:00".to_string(),
            ..SaveIndex::default()
        };

        let json = serde_json::to_string(&index).unwrap();
        let restored: SaveIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, index);
    }

    #[test]
    fn save_manifest_iteration_fields_roundtrip() {
        let manifest = SaveManifest {
            schema_version: SAVE_SCHEMA_VERSION,
            save_id: "save-iter".to_string(),
            display_name: "Iteration".to_string(),
            save_type: "iteration".to_string(),
            created_by: "system".to_string(),
            reason: "iteration".to_string(),
            created_at: "2026-07-08T00:00:00".to_string(),
            last_worked_at: "2026-07-08T00:00:00".to_string(),
            last_transaction_seq: 7,
            progress: SaveProgress::default(),
            change_type: Some("patch".to_string()),
            requested_version: Some("v2".to_string()),
            iteration_spec_path: Some("iteration_specs/spec.json".to_string()),
            extra: BTreeMap::from([("customField".to_string(), Value::String("kept".to_string()))]),
        };

        let json = serde_json::to_string(&manifest).unwrap();
        assert!(json.contains("iteration_spec_path"));
        let restored: SaveManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, manifest);
    }

    #[test]
    fn save_draft_meta_rejects_invalid_workspace_state() {
        let invalid = r#"{"workspace_state":"deleted_but_open"}"#;
        assert!(serde_json::from_str::<DraftMeta>(invalid).is_err());
    }

    #[test]
    fn save_autosave_state_is_project_state_json() {
        let autosave = AutosaveState::empty();
        let json = serde_json::to_string(&autosave).unwrap();
        assert!(json.contains("projectName"));
        let restored: AutosaveState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.project_name, DEFAULT_PROJECT_NAME);
    }

    #[test]
    fn save_snapshot_and_file_map_roundtrip() {
        let file = FileMapEntry {
            workspace_path: "outputs/artifacts/stage_00/artifact_index.json".to_string(),
            size_bytes: 128,
            mtime_ns: 42,
            sha256: "abc".to_string(),
            stage: Some(0),
            artifact_id: Value::Array(vec![Value::String("stage_00:index".to_string())]),
            role: "stage_file".to_string(),
            source_type: "run_output".to_string(),
            reference_manifest: "outputs/artifacts/stage_00/reference_manifest.json".to_string(),
            latest_transaction_seq: Some(3),
            extra: BTreeMap::new(),
        };
        let snapshot = SnapshotManifest {
            schema_version: SAVE_SCHEMA_VERSION,
            seq: 3,
            event: "manual_save".to_string(),
            stage: Some(0),
            timestamp: "2026-07-08T00:00:00".to_string(),
            message: String::new(),
            file_count: 1,
            added: 1,
            modified: 0,
            removed: 0,
        };
        let map = FileMap {
            schema_version: SAVE_SCHEMA_VERSION,
            generated_at: snapshot.timestamp.clone(),
            transaction_seq: Some(snapshot.seq),
            files: vec![file],
        };

        let snapshot_json = serde_json::to_string(&snapshot).unwrap();
        let map_json = serde_json::to_string(&map).unwrap();
        assert_eq!(
            serde_json::from_str::<SnapshotManifest>(&snapshot_json).unwrap(),
            snapshot
        );
        assert_eq!(serde_json::from_str::<FileMap>(&map_json).unwrap(), map);
    }
}
