use adm_new_application::{PackageRunResult, PackagingApplicationService};
use adm_new_contracts::package::{
    PackageBuildReport, PackageManifest, PackageStatus, PackageValidationReport,
};
use adm_new_foundation::AdmResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CommandAdapterResult, handle_command};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageCurrentProjectRequest {
    pub integration: Value,
    pub actual_project_file_audit: Value,
    pub unity_validation_summary: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageRunResultView {
    pub validation_report: PackageValidationReport,
    pub build_report: PackageBuildReport,
    pub manifest: PackageManifest,
    pub package_notes: String,
}

impl From<PackageRunResult> for PackageRunResultView {
    fn from(value: PackageRunResult) -> Self {
        Self {
            validation_report: value.validation_report,
            build_report: value.build_report,
            manifest: value.manifest,
            package_notes: value.package_notes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageView {
    pub step14_status: String,
    pub can_package: bool,
    #[serde(default)]
    pub last_result: Option<PackageRunResultView>,
    pub blocking_issues: Vec<String>,
}

pub trait PackageCommandService {
    fn load_package_view(
        &self,
        last_result: Option<&PackageRunResultView>,
    ) -> AdmResult<PackageView>;
    fn package_current_project(
        &self,
        request: &PackageCurrentProjectRequest,
    ) -> AdmResult<PackageRunResultView>;
}

impl PackageCommandService for PackagingApplicationService {
    fn load_package_view(
        &self,
        last_result: Option<&PackageRunResultView>,
    ) -> AdmResult<PackageView> {
        Ok(package_view(last_result.cloned()))
    }

    fn package_current_project(
        &self,
        request: &PackageCurrentProjectRequest,
    ) -> AdmResult<PackageRunResultView> {
        Ok(self
            .package_current_project_from_values(
                request.integration.clone(),
                request.actual_project_file_audit.clone(),
                request.unity_validation_summary.clone(),
            )
            .into())
    }
}

pub fn load_package_view<S>(
    service: &S,
    last_result: Option<PackageRunResultView>,
) -> CommandAdapterResult<PackageView>
where
    S: PackageCommandService,
{
    handle_command(|| service.load_package_view(last_result.as_ref()))
}

pub fn package_current_project<S>(
    service: &S,
    request: PackageCurrentProjectRequest,
) -> CommandAdapterResult<PackageRunResultView>
where
    S: PackageCommandService,
{
    handle_command(|| service.package_current_project(&request))
}

fn package_view(last_result: Option<PackageRunResultView>) -> PackageView {
    let step14_status = last_result
        .as_ref()
        .map(|result| result.validation_report.status.as_str().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let can_package = last_result
        .as_ref()
        .map(|result| result.validation_report.status == PackageStatus::Success)
        .unwrap_or(false);
    let blocking_issues = last_result
        .as_ref()
        .map(|result| {
            result
                .validation_report
                .blocking_issues
                .iter()
                .map(|issue| format!("{}: {}", issue.id, issue.message))
                .collect()
        })
        .unwrap_or_default();
    PackageView {
        step14_status,
        can_package,
        last_result,
        blocking_issues,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use adm_new_contracts::package::REQUIRED_INTEGRATION_CHECKS;
    use adm_new_foundation::{AdmError, AdmResult};

    #[test]
    fn package_command_success_uses_service_validation() {
        let service = PackagingApplicationService::new();
        let response = package_current_project(&service, success_request());
        assert!(response.ok);
        let result = response.data.unwrap();
        assert_eq!(result.validation_report.status, PackageStatus::Success);

        let view = load_package_view(&service, Some(result));
        assert!(view.ok);
        assert!(view.data.unwrap().can_package);
    }

    #[test]
    fn package_command_blocks_without_skipping_validation() {
        let service = PackagingApplicationService::new();
        let mut request = success_request();
        request.actual_project_file_audit = serde_json::json!({
            "development_path": "UnityProject",
            "actual_changed_files": []
        });
        let response = package_current_project(&service, request);
        assert!(response.ok);
        let result = response.data.unwrap();
        assert_eq!(result.validation_report.status, PackageStatus::Blocked);
        assert!(
            result
                .validation_report
                .blocking_issues
                .iter()
                .any(|issue| { issue.id == "PACKAGE-NO-ACTUAL-PROJECT-CHANGES" })
        );
    }

    #[test]
    fn package_command_reports_failed_required_check() {
        let service = PackagingApplicationService::new();
        let mut request = success_request();
        request.integration["checks"]["playmode_smoke_passed"] = Value::Bool(false);
        let response = package_current_project(&service, request);
        assert!(response.ok);
        let result = response.data.unwrap();
        assert_eq!(result.manifest.status, PackageStatus::Blocked);
        assert!(
            result
                .validation_report
                .checks
                .iter()
                .any(|check| { check.id == "playmode_smoke_passed" && !check.passed })
        );
    }

    #[test]
    fn package_command_wrapper_calls_service_trait_mock() {
        let service = MockPackageService {
            load_calls: Cell::new(0),
        };
        let response = load_package_view(&service, None);
        assert!(response.ok);
        assert_eq!(service.load_calls.get(), 1);
    }

    struct MockPackageService {
        load_calls: Cell<usize>,
    }

    impl PackageCommandService for MockPackageService {
        fn load_package_view(
            &self,
            last_result: Option<&PackageRunResultView>,
        ) -> AdmResult<PackageView> {
            self.load_calls.set(self.load_calls.get() + 1);
            Ok(package_view(last_result.cloned()))
        }

        fn package_current_project(
            &self,
            _: &PackageCurrentProjectRequest,
        ) -> AdmResult<PackageRunResultView> {
            Err(AdmError::new("mock package not implemented"))
        }
    }

    fn success_request() -> PackageCurrentProjectRequest {
        let checks = REQUIRED_INTEGRATION_CHECKS
            .iter()
            .map(|id| ((*id).to_string(), Value::Bool(true)))
            .collect::<serde_json::Map<_, _>>();
        PackageCurrentProjectRequest {
            integration: serde_json::json!({
                "status": "success",
                "checks": Value::Object(checks)
            }),
            actual_project_file_audit: serde_json::json!({
                "development_path": "UnityProject",
                "actual_changed_files": ["Assets/DemoScene.unity"]
            }),
            unity_validation_summary: serde_json::json!({
                "valid": true,
                "unity_editor_path": "Unity.exe",
                "validation_count": 3,
                "failed_validation_count": 0
            }),
        }
    }
}
