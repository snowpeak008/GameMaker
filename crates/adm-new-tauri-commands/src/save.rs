use adm_new_application::{LoadedSave, SaveApplicationService, SaveServiceReport};
use adm_new_contracts::project::ProjectState;
use adm_new_contracts::save::{DraftMeta, FileMap, SaveIndex, SaveManifest, SnapshotManifest};
use adm_new_foundation::AdmResult;
use serde::{Deserialize, Serialize};

use crate::{CommandAdapterResult, handle_command};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateSaveRequest {
    pub display_name: String,
    pub state: ProjectState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateBlankSaveRequest {
    pub display_name: String,
    #[serde(default)]
    pub state: ProjectState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveProjectRequest {
    pub state: ProjectState,
    #[serde(default = "default_save_reason")]
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveSwitchBehavior {
    #[default]
    SaveCurrent,
    DiscardDraft,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LoadSaveRequest {
    pub save_id: String,
    #[serde(default)]
    pub switch_behavior: SaveSwitchBehavior,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenameSaveRequest {
    pub save_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteSaveRequest {
    pub save_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenSaveDirectoryRequest {
    pub save_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenSaveDirectoryView {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveReportView {
    pub manifest: SaveManifest,
    pub index: SaveIndex,
    pub draft_meta: DraftMeta,
    pub file_map: FileMap,
    pub snapshot_manifest: SnapshotManifest,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl From<SaveServiceReport> for SaveReportView {
    fn from(value: SaveServiceReport) -> Self {
        Self {
            manifest: value.manifest,
            index: value.index,
            draft_meta: value.draft_meta,
            file_map: value.file_map,
            snapshot_manifest: value.snapshot_manifest,
            warnings: value.warnings,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadedSaveView {
    pub manifest: SaveManifest,
    pub draft_meta: DraftMeta,
    pub state: ProjectState,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl From<LoadedSave> for LoadedSaveView {
    fn from(value: LoadedSave) -> Self {
        Self {
            manifest: value.manifest,
            draft_meta: value.draft_meta,
            state: value.state,
            warnings: value.warnings,
        }
    }
}

pub trait SaveCommandService {
    fn list_saves(&self) -> AdmResult<SaveIndex>;
    fn create_save(&self, request: &CreateSaveRequest) -> AdmResult<SaveReportView>;
    fn create_blank_save(&self, request: &CreateBlankSaveRequest) -> AdmResult<SaveReportView>;
    fn save_project(&self, request: &SaveProjectRequest) -> AdmResult<SaveReportView>;
    fn load_save(&self, request: &LoadSaveRequest) -> AdmResult<LoadedSaveView>;
    fn rename_save(&self, request: &RenameSaveRequest) -> AdmResult<SaveIndex>;
    fn delete_save(&self, request: &DeleteSaveRequest) -> AdmResult<SaveIndex>;
    fn get_autosave_state(&self) -> AdmResult<Option<ProjectState>>;
}

impl SaveCommandService for SaveApplicationService {
    fn list_saves(&self) -> AdmResult<SaveIndex> {
        self.list_saves()
    }

    fn create_save(&self, request: &CreateSaveRequest) -> AdmResult<SaveReportView> {
        self.create_save(&request.display_name, &request.state)
            .map(SaveReportView::from)
    }

    fn create_blank_save(&self, request: &CreateBlankSaveRequest) -> AdmResult<SaveReportView> {
        self.create_blank_save_from_state(&request.display_name, &request.state)
            .map(SaveReportView::from)
    }

    fn save_project(&self, request: &SaveProjectRequest) -> AdmResult<SaveReportView> {
        let reason = if request.reason.trim().is_empty() {
            default_save_reason()
        } else {
            request.reason.clone()
        };
        self.sync_current_save(&request.state, &reason)
            .map(SaveReportView::from)
    }

    fn load_save(&self, request: &LoadSaveRequest) -> AdmResult<LoadedSaveView> {
        self.load_save(&request.save_id).map(LoadedSaveView::from)
    }

    fn rename_save(&self, request: &RenameSaveRequest) -> AdmResult<SaveIndex> {
        self.rename_save(&request.save_id, &request.display_name)
    }

    fn delete_save(&self, request: &DeleteSaveRequest) -> AdmResult<SaveIndex> {
        self.delete_save(&request.save_id)
    }

    fn get_autosave_state(&self) -> AdmResult<Option<ProjectState>> {
        self.autosave_state()
    }
}

pub fn list_saves<S>(service: &S) -> CommandAdapterResult<SaveIndex>
where
    S: SaveCommandService,
{
    handle_command(|| service.list_saves())
}

pub fn create_save<S>(
    service: &S,
    request: CreateSaveRequest,
) -> CommandAdapterResult<SaveReportView>
where
    S: SaveCommandService,
{
    handle_command(|| service.create_save(&request))
}

pub fn create_blank_save<S>(
    service: &S,
    request: CreateBlankSaveRequest,
) -> CommandAdapterResult<SaveReportView>
where
    S: SaveCommandService,
{
    handle_command(|| service.create_blank_save(&request))
}

pub fn save_project<S>(
    service: &S,
    request: SaveProjectRequest,
) -> CommandAdapterResult<SaveReportView>
where
    S: SaveCommandService,
{
    handle_command(|| service.save_project(&request))
}

pub fn load_save<S>(service: &S, request: LoadSaveRequest) -> CommandAdapterResult<LoadedSaveView>
where
    S: SaveCommandService,
{
    handle_command(|| service.load_save(&request))
}

pub fn rename_save<S>(service: &S, request: RenameSaveRequest) -> CommandAdapterResult<SaveIndex>
where
    S: SaveCommandService,
{
    handle_command(|| service.rename_save(&request))
}

pub fn delete_save<S>(service: &S, request: DeleteSaveRequest) -> CommandAdapterResult<SaveIndex>
where
    S: SaveCommandService,
{
    handle_command(|| service.delete_save(&request))
}

pub fn get_autosave_state<S>(service: &S) -> CommandAdapterResult<Option<ProjectState>>
where
    S: SaveCommandService,
{
    handle_command(|| service.get_autosave_state())
}

fn default_save_reason() -> String {
    "manual_save".to_string()
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use adm_new_contracts::project::NodeState;
    use adm_new_contracts::save::SaveProgress;
    use adm_new_foundation::{AdmError, AdmResult, new_stable_id};

    #[test]
    fn save_commands_call_real_service_and_serialize_reports() {
        let root = temp_root("real");
        let service = SaveApplicationService::with_pid(&root, "session_a", 100).unwrap();
        let state = sample_state("Command Save", true);

        let created = create_save(
            &service,
            CreateSaveRequest {
                display_name: "Command Save".to_string(),
                state: state.clone(),
            },
        );
        assert!(created.ok);
        let report = created.data.unwrap();
        assert_eq!(report.manifest.display_name, "Command Save");
        assert_eq!(report.index.saves.len(), 1);

        let listed = list_saves(&service);
        assert_eq!(listed.data.unwrap().saves.len(), 1);

        let loaded = load_save(
            &service,
            LoadSaveRequest {
                save_id: report.manifest.save_id.clone(),
                switch_behavior: SaveSwitchBehavior::SaveCurrent,
            },
        );
        assert_eq!(loaded.data.unwrap().state.project_name, "Command Save");

        let saved = save_project(
            &service,
            SaveProjectRequest {
                state: sample_state("Command Save Updated", true),
                reason: "manual_save".to_string(),
            },
        );
        assert_eq!(saved.data.unwrap().manifest.last_transaction_seq, 2);

        let renamed = rename_save(
            &service,
            RenameSaveRequest {
                save_id: report.manifest.save_id.clone(),
                display_name: "Renamed".to_string(),
            },
        );
        assert_eq!(renamed.data.unwrap().saves[0].display_name, "Renamed");

        let deleted = delete_save(
            &service,
            DeleteSaveRequest {
                save_id: report.manifest.save_id,
            },
        );
        assert!(deleted.data.unwrap().saves.is_empty());
        cleanup(root);
    }

    #[test]
    fn blank_save_command_preserves_design_state_and_uses_blank_workspace_service() {
        let root = temp_root("blank");
        let service = SaveApplicationService::with_pid(&root, "session_a", 100).unwrap();
        let created = create_blank_save(
            &service,
            CreateBlankSaveRequest {
                display_name: "Fresh Pipeline".to_string(),
                state: sample_state("Preserved Design", true),
            },
        );
        assert!(created.ok, "{created:?}");
        let report = created.data.unwrap();
        let loaded = service.load_save(&report.manifest.save_id).unwrap();
        assert_eq!(loaded.state.project_name, "Preserved Design");
        cleanup(root);
    }

    #[test]
    fn blank_save_request_accepts_legacy_payload_without_state() {
        let request: CreateBlankSaveRequest =
            serde_json::from_value(serde_json::json!({"display_name": "Fresh"})).unwrap();
        assert_eq!(request.display_name, "Fresh");
        assert_eq!(request.state, ProjectState::default());
    }

    #[test]
    fn save_command_errors_are_mapped_by_adapter() {
        let root = temp_root("error");
        let service = SaveApplicationService::with_pid(&root, "session_a", 100).unwrap();
        let response = save_project(
            &service,
            SaveProjectRequest {
                state: sample_state("Unsaved", false),
                reason: String::new(),
            },
        );
        assert!(!response.ok);
        assert_eq!(response.error.unwrap().code, "VALIDATION_FAILED");
        cleanup(root);
    }

    #[test]
    fn get_autosave_state_is_command_mapped() {
        let root = temp_root("autosave");
        let service = SaveApplicationService::with_pid(&root, "session_a", 100).unwrap();
        service.autosave(&sample_state("Draft", false)).unwrap();

        let response = get_autosave_state(&service);
        assert!(response.ok);
        assert_eq!(response.data.unwrap().unwrap().project_name, "Draft");
        cleanup(root);
    }

    #[test]
    fn save_command_wrapper_calls_service_trait_mock() {
        let service = MockSaveService {
            list_calls: Cell::new(0),
        };
        let response = list_saves(&service);
        assert!(response.ok);
        assert_eq!(service.list_calls.get(), 1);
        assert_eq!(response.data.unwrap().saves.len(), 1);
    }

    struct MockSaveService {
        list_calls: Cell<usize>,
    }

    impl SaveCommandService for MockSaveService {
        fn list_saves(&self) -> AdmResult<SaveIndex> {
            self.list_calls.set(self.list_calls.get() + 1);
            Ok(SaveIndex {
                current_save_id: Some("save_mock".to_string()),
                saves: vec![adm_new_contracts::save::SaveIndexEntry {
                    save_id: "save_mock".to_string(),
                    display_name: "Mock".to_string(),
                    save_type: "manual".to_string(),
                    created_by: "test".to_string(),
                    reason: "mock".to_string(),
                    path: "saves/save_mock".to_string(),
                    created_at: "unix:1".to_string(),
                    last_worked_at: "unix:1".to_string(),
                    progress: SaveProgress {
                        passed: 1,
                        total: 1,
                        label: "1/1".to_string(),
                        ..SaveProgress::default()
                    },
                    ..adm_new_contracts::save::SaveIndexEntry::default()
                }],
                updated_at: "unix:1".to_string(),
                ..SaveIndex::default()
            })
        }

        fn create_save(&self, _: &CreateSaveRequest) -> AdmResult<SaveReportView> {
            Err(AdmError::new("mock create not implemented"))
        }

        fn create_blank_save(&self, _: &CreateBlankSaveRequest) -> AdmResult<SaveReportView> {
            Err(AdmError::new("mock blank create not implemented"))
        }

        fn save_project(&self, _: &SaveProjectRequest) -> AdmResult<SaveReportView> {
            Err(AdmError::new("mock save not implemented"))
        }

        fn load_save(&self, _: &LoadSaveRequest) -> AdmResult<LoadedSaveView> {
            Err(AdmError::new("mock load not implemented"))
        }

        fn rename_save(&self, _: &RenameSaveRequest) -> AdmResult<SaveIndex> {
            Err(AdmError::new("mock rename not implemented"))
        }

        fn delete_save(&self, _: &DeleteSaveRequest) -> AdmResult<SaveIndex> {
            Err(AdmError::new("mock delete not implemented"))
        }

        fn get_autosave_state(&self) -> AdmResult<Option<ProjectState>> {
            Err(AdmError::new("mock autosave not implemented"))
        }
    }

    fn sample_state(project_name: &str, checked: bool) -> ProjectState {
        let mut state = ProjectState::empty();
        state.project_name = project_name.to_string();
        let mut node = NodeState::default();
        node.checklist.insert("core_loop".to_string(), checked);
        state.nodes.insert("mechanics".to_string(), node);
        state
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm_new_tauri_save_{label}_{}",
            new_stable_id("root").unwrap()
        ))
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
