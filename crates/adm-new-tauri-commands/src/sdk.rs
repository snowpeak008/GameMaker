use adm_new_application::SdkKnowledgeApplicationService;
use adm_new_contracts::sdk::{SdkReviewStatus, SdkSpec};
use adm_new_foundation::AdmResult;
use serde::{Deserialize, Serialize};

use crate::{CommandAdapterResult, handle_command};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddSdkRequest {
    pub sdk_id: String,
    pub name: String,
    #[serde(default)]
    pub source_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateSdkReviewStatusRequest {
    pub sdk_id: String,
    pub status: SdkReviewStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractSdkSpecRequest {
    pub spec: SdkSpec,
}

pub trait SdkCommandService {
    fn list_sdks(&self) -> AdmResult<Vec<SdkSpec>>;
    fn add_sdk(&mut self, request: &AddSdkRequest) -> AdmResult<SdkSpec>;
    fn update_sdk_review_status(
        &mut self,
        request: &UpdateSdkReviewStatusRequest,
    ) -> AdmResult<SdkSpec>;
    fn get_approved_sdk_context(&self) -> AdmResult<String>;
    fn extract_sdk_spec(&mut self, request: &ExtractSdkSpecRequest) -> AdmResult<SdkSpec>;
}

impl SdkCommandService for SdkKnowledgeApplicationService {
    fn list_sdks(&self) -> AdmResult<Vec<SdkSpec>> {
        Ok(self.list_specs())
    }

    fn add_sdk(&mut self, request: &AddSdkRequest) -> AdmResult<SdkSpec> {
        self.add_placeholder_with_source_url(&request.sdk_id, &request.name, &request.source_url)
    }

    fn update_sdk_review_status(
        &mut self,
        request: &UpdateSdkReviewStatusRequest,
    ) -> AdmResult<SdkSpec> {
        self.set_review_status(&request.sdk_id, request.status.clone())
    }

    fn get_approved_sdk_context(&self) -> AdmResult<String> {
        Ok(self.approved_context())
    }

    fn extract_sdk_spec(&mut self, request: &ExtractSdkSpecRequest) -> AdmResult<SdkSpec> {
        Ok(self.ingest_ai_extracted_spec(request.spec.clone()))
    }
}

pub fn list_sdks<S>(service: &S) -> CommandAdapterResult<Vec<SdkSpec>>
where
    S: SdkCommandService,
{
    handle_command(|| service.list_sdks())
}

pub fn add_sdk<S>(service: &mut S, request: AddSdkRequest) -> CommandAdapterResult<SdkSpec>
where
    S: SdkCommandService,
{
    handle_command(|| service.add_sdk(&request))
}

pub fn update_sdk_review_status<S>(
    service: &mut S,
    request: UpdateSdkReviewStatusRequest,
) -> CommandAdapterResult<SdkSpec>
where
    S: SdkCommandService,
{
    handle_command(|| service.update_sdk_review_status(&request))
}

pub fn get_approved_sdk_context<S>(service: &S) -> CommandAdapterResult<String>
where
    S: SdkCommandService,
{
    handle_command(|| service.get_approved_sdk_context())
}

pub fn extract_sdk_spec<S>(
    service: &mut S,
    request: ExtractSdkSpecRequest,
) -> CommandAdapterResult<SdkSpec>
where
    S: SdkCommandService,
{
    handle_command(|| service.extract_sdk_spec(&request))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use adm_new_foundation::{AdmError, AdmResult};

    #[test]
    fn sdk_commands_list_add_review_and_context() {
        let mut service = SdkKnowledgeApplicationService::new();
        let added = add_sdk(
            &mut service,
            AddSdkRequest {
                sdk_id: "steamworks".to_string(),
                name: "Steamworks".to_string(),
                source_url: "https://partner.steamgames.com/doc/sdk".to_string(),
            },
        );
        assert!(added.ok);
        assert_eq!(
            added.data.as_ref().unwrap().review_status,
            SdkReviewStatus::Draft
        );
        assert_eq!(
            added.data.as_ref().unwrap().source_url,
            "https://partner.steamgames.com/doc/sdk"
        );

        let listed = list_sdks(&service);
        assert_eq!(listed.data.unwrap().len(), 1);

        let approved = update_sdk_review_status(
            &mut service,
            UpdateSdkReviewStatusRequest {
                sdk_id: "steamworks".to_string(),
                status: SdkReviewStatus::Approved,
            },
        );
        assert_eq!(
            approved.data.unwrap().review_status,
            SdkReviewStatus::Approved
        );

        let context = get_approved_sdk_context(&service);
        assert!(context.data.unwrap().contains("Steamworks"));
    }

    #[test]
    fn sdk_ai_extraction_cannot_auto_approve() {
        let mut service = SdkKnowledgeApplicationService::new();
        let response = extract_sdk_spec(
            &mut service,
            ExtractSdkSpecRequest {
                spec: SdkSpec {
                    sdk_id: "ads".to_string(),
                    name: "Ads SDK".to_string(),
                    source_url: String::new(),
                    review_status: SdkReviewStatus::Approved,
                    summary: "AI extracted".to_string(),
                    integration_notes: vec!["Initialize after consent.".to_string()],
                    api_requirements: Vec::new(),
                    risks: Vec::new(),
                    last_synced_at: String::new(),
                    updated_at: String::new(),
                },
            },
        );
        assert!(response.ok);
        assert_eq!(
            response.data.unwrap().review_status,
            SdkReviewStatus::PendingReview
        );
        assert!(get_approved_sdk_context(&service).data.unwrap().is_empty());
    }

    #[test]
    fn sdk_validation_and_not_found_errors_are_mapped() {
        let mut service = SdkKnowledgeApplicationService::new();
        let invalid = add_sdk(
            &mut service,
            AddSdkRequest {
                sdk_id: "steamworks".to_string(),
                name: String::new(),
                source_url: String::new(),
            },
        );
        assert!(!invalid.ok);
        assert_eq!(invalid.error.unwrap().code, "VALIDATION_FAILED");

        let missing = update_sdk_review_status(
            &mut service,
            UpdateSdkReviewStatusRequest {
                sdk_id: "missing".to_string(),
                status: SdkReviewStatus::Approved,
            },
        );
        assert!(!missing.ok);
        assert_eq!(missing.error.unwrap().code, "NOT_FOUND");
    }

    #[test]
    fn sdk_command_wrapper_calls_service_trait_mock() {
        let service = MockSdkService {
            list_calls: Cell::new(0),
        };
        let response = list_sdks(&service);
        assert!(response.ok);
        assert_eq!(service.list_calls.get(), 1);
    }

    struct MockSdkService {
        list_calls: Cell<usize>,
    }

    impl SdkCommandService for MockSdkService {
        fn list_sdks(&self) -> AdmResult<Vec<SdkSpec>> {
            self.list_calls.set(self.list_calls.get() + 1);
            Ok(Vec::new())
        }

        fn add_sdk(&mut self, _: &AddSdkRequest) -> AdmResult<SdkSpec> {
            Err(AdmError::new("mock add not implemented"))
        }

        fn update_sdk_review_status(
            &mut self,
            _: &UpdateSdkReviewStatusRequest,
        ) -> AdmResult<SdkSpec> {
            Err(AdmError::new("mock status not implemented"))
        }

        fn get_approved_sdk_context(&self) -> AdmResult<String> {
            Ok(String::new())
        }

        fn extract_sdk_spec(&mut self, _: &ExtractSdkSpecRequest) -> AdmResult<SdkSpec> {
            Err(AdmError::new("mock extract not implemented"))
        }
    }
}
