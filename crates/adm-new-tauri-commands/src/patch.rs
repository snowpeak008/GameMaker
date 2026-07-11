use adm_new_application::PatchApplicationService;
use adm_new_contracts::patch::{PatchRecord, PatchStatus, PatchTask};
use adm_new_foundation::AdmResult;
use serde::{Deserialize, Serialize};

use crate::{CommandAdapterResult, handle_command};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzePatchRequest {
    pub request: String,
    #[serde(default)]
    pub tasks: Vec<PatchTask>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListPatchesRequest {
    #[serde(default)]
    pub status: Option<PatchStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadPatchRequest {
    pub patch_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdatePatchStatusRequest {
    pub patch_id: String,
    pub status: PatchStatus,
}

pub trait PatchCommandService {
    fn analyze_patch_request(&mut self, request: &AnalyzePatchRequest) -> AdmResult<PatchRecord>;
    fn list_patches(&self, request: &ListPatchesRequest) -> AdmResult<Vec<PatchRecord>>;
    fn read_patch(&self, request: &ReadPatchRequest) -> AdmResult<PatchRecord>;
    fn update_patch_status(&mut self, request: &UpdatePatchStatusRequest)
    -> AdmResult<PatchRecord>;
}

impl PatchCommandService for PatchApplicationService {
    fn analyze_patch_request(&mut self, request: &AnalyzePatchRequest) -> AdmResult<PatchRecord> {
        self.analyze_request_shell(&request.request, request.tasks.clone())
    }

    fn list_patches(&self, request: &ListPatchesRequest) -> AdmResult<Vec<PatchRecord>> {
        let records = match &request.status {
            Some(status) => self
                .list()
                .into_iter()
                .filter(|record| &record.status == status)
                .collect(),
            None => self.list(),
        };
        Ok(records)
    }

    fn read_patch(&self, request: &ReadPatchRequest) -> AdmResult<PatchRecord> {
        self.read(&request.patch_id)
    }

    fn update_patch_status(
        &mut self,
        request: &UpdatePatchStatusRequest,
    ) -> AdmResult<PatchRecord> {
        self.set_status(&request.patch_id, request.status.clone())
    }
}

pub fn analyze_patch_request<S>(
    service: &mut S,
    request: AnalyzePatchRequest,
) -> CommandAdapterResult<PatchRecord>
where
    S: PatchCommandService,
{
    handle_command(|| service.analyze_patch_request(&request))
}

pub fn list_patches<S>(
    service: &S,
    request: ListPatchesRequest,
) -> CommandAdapterResult<Vec<PatchRecord>>
where
    S: PatchCommandService,
{
    handle_command(|| service.list_patches(&request))
}

pub fn read_patch<S>(service: &S, request: ReadPatchRequest) -> CommandAdapterResult<PatchRecord>
where
    S: PatchCommandService,
{
    handle_command(|| service.read_patch(&request))
}

pub fn update_patch_status<S>(
    service: &mut S,
    request: UpdatePatchStatusRequest,
) -> CommandAdapterResult<PatchRecord>
where
    S: PatchCommandService,
{
    handle_command(|| service.update_patch_status(&request))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use adm_new_foundation::{AdmError, AdmResult};

    #[test]
    fn patch_commands_analyze_list_read_and_update_status() {
        let mut service = PatchApplicationService::new();
        let created = analyze_patch_request(
            &mut service,
            AnalyzePatchRequest {
                request: "Add pipeline refresh".to_string(),
                tasks: vec![PatchTask {
                    task_id: "task-1".to_string(),
                    title: "Refresh".to_string(),
                    description: String::new(),
                    affected_systems: vec!["pipeline".to_string()],
                    expected_files: Vec::new(),
                    validation_route: Vec::new(),
                    requires_iteration: false,
                }],
            },
        );
        assert!(created.ok);
        let patch_id = created.data.as_ref().unwrap().patch_id.clone();

        let listed = list_patches(&service, ListPatchesRequest { status: None });
        assert_eq!(listed.data.unwrap().len(), 1);

        let read = read_patch(
            &service,
            ReadPatchRequest {
                patch_id: patch_id.clone(),
            },
        );
        assert_eq!(read.data.unwrap().request, "Add pipeline refresh");

        let updated = update_patch_status(
            &mut service,
            UpdatePatchStatusRequest {
                patch_id,
                status: PatchStatus::Validated,
            },
        );
        assert_eq!(updated.data.unwrap().status, PatchStatus::Validated);
    }

    #[test]
    fn patch_validation_and_not_found_errors_are_mapped() {
        let mut service = PatchApplicationService::new();
        let empty = analyze_patch_request(
            &mut service,
            AnalyzePatchRequest {
                request: String::new(),
                tasks: Vec::new(),
            },
        );
        assert!(!empty.ok);
        assert_eq!(empty.error.unwrap().code, "VALIDATION_FAILED");

        let missing = read_patch(
            &service,
            ReadPatchRequest {
                patch_id: "missing".to_string(),
            },
        );
        assert!(!missing.ok);
        assert_eq!(missing.error.unwrap().code, "NOT_FOUND");
    }

    #[test]
    fn patch_command_wrapper_calls_service_trait_mock() {
        let service = MockPatchService {
            list_calls: Cell::new(0),
        };
        let response = list_patches(&service, ListPatchesRequest { status: None });
        assert!(response.ok);
        assert_eq!(service.list_calls.get(), 1);
    }

    struct MockPatchService {
        list_calls: Cell<usize>,
    }

    impl PatchCommandService for MockPatchService {
        fn analyze_patch_request(&mut self, _: &AnalyzePatchRequest) -> AdmResult<PatchRecord> {
            Err(AdmError::new("mock analyze not implemented"))
        }

        fn list_patches(&self, _: &ListPatchesRequest) -> AdmResult<Vec<PatchRecord>> {
            self.list_calls.set(self.list_calls.get() + 1);
            Ok(Vec::new())
        }

        fn read_patch(&self, _: &ReadPatchRequest) -> AdmResult<PatchRecord> {
            Err(AdmError::new("mock read not implemented"))
        }

        fn update_patch_status(&mut self, _: &UpdatePatchStatusRequest) -> AdmResult<PatchRecord> {
            Err(AdmError::new("mock update not implemented"))
        }
    }
}
