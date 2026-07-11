#![forbid(unsafe_code)]

use adm_new_contracts::package::{
    PackageBlockingIssue, PackageBuildReport, PackageManifest, PackageManifestOutputs,
    PackageRequiredCheck, PackageStatus, PackageUnityValidation, PackageValidationReport,
    REQUIRED_INTEGRATION_CHECKS,
};
use adm_new_foundation::unix_timestamp;
use serde_json::Value;

pub mod dist;
pub mod file_service;

pub use dist::{
    DEFAULT_DIST_EXE_NAME, DEFAULT_MIN_EXE_BYTES, DistBuildPlan, DistBundleVerification,
    dist_build_plan, verify_dist_bundle,
};
pub use file_service::PackageFileRunResult;

pub const CRATE_NAME: &str = "adm-new-packaging";
pub const PACKAGE_DIR: &str = "outputs/package/current";
pub const PACKAGE_OUTPUT_RELATIVE_DIR: &str = "package/current";

pub fn crate_ready() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackagingSources {
    pub integration: Value,
    pub actual_project_file_audit: Value,
    pub unity_validation_summary: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackageRunResult {
    pub validation_report: PackageValidationReport,
    pub build_report: PackageBuildReport,
    pub manifest: PackageManifest,
    pub package_notes: String,
}

#[derive(Debug, Clone, Default)]
pub struct PackagingService;

impl PackagingService {
    pub fn new() -> Self {
        Self
    }

    pub fn run_package(&self, sources: PackagingSources) -> PackageRunResult {
        let generated_at = timestamp();
        let checks = required_checks_from_integration(&sources.integration);
        let mut blocking_issues = Vec::new();
        if string_at(&sources.integration, "status").as_deref() != Some("success") {
            blocking_issues.push(issue(
                "PACKAGE-STEP14-NOT-SUCCESS",
                "Step14 integration validation status is not success.",
            ));
        }
        for check in &checks {
            if !check.passed {
                blocking_issues.push(issue(
                    &format!("PACKAGE-CHECK-{}", check.id.to_ascii_uppercase()),
                    &format!("Required integration check failed: {}", check.id),
                ));
            }
        }
        let development_path =
            string_at(&sources.actual_project_file_audit, "development_path").unwrap_or_default();
        let changed_files = array_at(&sources.actual_project_file_audit, "actual_changed_files");
        if changed_files.is_empty() {
            blocking_issues.push(issue(
                "PACKAGE-NO-ACTUAL-PROJECT-CHANGES",
                "No actual Unity project changes are available to package.",
            ));
        }
        let unity_validation = unity_validation(&sources.unity_validation_summary);
        if bool_at(&sources.unity_validation_summary, "valid") != Some(true) {
            blocking_issues.push(issue(
                "PACKAGE-UNITY-VALIDATION-MISSING",
                "Unity validation summary is missing or not valid.",
            ));
        }
        let status = if blocking_issues.is_empty() {
            PackageStatus::Success
        } else {
            PackageStatus::Blocked
        };
        let validation_report = PackageValidationReport {
            schema_version: 1,
            generated_at: generated_at.clone(),
            status: status.clone(),
            source_stage: 14,
            source_stage_name: "integration_validation".to_string(),
            blocking_issues: blocking_issues.clone(),
            checks,
            development_path: development_path.clone(),
            changed_files: changed_files.clone(),
            unity_validation: Some(unity_validation.clone()),
        };
        let build_report = PackageBuildReport {
            schema_version: 1,
            generated_at: generated_at.clone(),
            status: status.clone(),
            package_type: "current_project_build_package".to_string(),
            source_stage: 14,
            source_stage_name: "integration_validation".to_string(),
            development_path: development_path.clone(),
            changed_files: changed_files.clone(),
            unity_validation: Some(unity_validation),
            blocking_issues: blocking_issues.clone(),
        };
        let manifest = PackageManifest {
            schema_version: 1,
            generated_at,
            package_type: "current_project_build_package".to_string(),
            status,
            development_path,
            changed_files,
            source_stage: 14,
            source_stage_name: "integration_validation".to_string(),
            outputs: PackageManifestOutputs {
                package_dir: PACKAGE_DIR.to_string(),
                build_report: format!("{PACKAGE_DIR}/build_report.json"),
                package_validation_report: format!("{PACKAGE_DIR}/package_validation_report.json"),
                package_notes: format!("{PACKAGE_DIR}/PACKAGE_NOTES.md"),
            },
        };
        let package_notes = package_notes(&validation_report);
        PackageRunResult {
            validation_report,
            build_report,
            manifest,
            package_notes,
        }
    }
}

fn required_checks_from_integration(integration: &Value) -> Vec<PackageRequiredCheck> {
    REQUIRED_INTEGRATION_CHECKS
        .iter()
        .map(|id| PackageRequiredCheck {
            id: (*id).to_string(),
            passed: required_check_passed(integration, id),
        })
        .collect()
}

fn required_check_passed(integration: &Value, id: &str) -> bool {
    if let Some(value) = integration.get("checks").and_then(|checks| checks.get(id)) {
        return value.as_bool().unwrap_or(false);
    }
    integration
        .get("checks")
        .and_then(Value::as_array)
        .and_then(|checks| {
            checks.iter().find(|check| {
                check
                    .get("id")
                    .and_then(Value::as_str)
                    .map(|value| value == id)
                    .unwrap_or(false)
            })
        })
        .and_then(|check| check.get("passed").and_then(Value::as_bool))
        .unwrap_or(false)
}

fn unity_validation(summary: &Value) -> PackageUnityValidation {
    PackageUnityValidation {
        unity_editor_path: string_at(summary, "unity_editor_path").unwrap_or_default(),
        validation_count: u32_at(summary, "validation_count"),
        failed_validation_count: u32_at(summary, "failed_validation_count"),
    }
}

fn package_notes(report: &PackageValidationReport) -> String {
    let mut notes = String::new();
    notes.push_str("# Package Notes\n\n");
    notes.push_str(&format!("status: {}\n", report.status.as_str()));
    notes.push_str("source_stage: 14 integration_validation\n");
    if report.blocking_issues.is_empty() {
        notes.push_str("\nThe current project passed packaging readiness checks.\n");
    } else {
        notes.push_str("\n## Blocking Issues\n");
        for issue in &report.blocking_issues {
            notes.push_str(&format!("- {}: {}\n", issue.id, issue.message));
        }
    }
    notes
}

fn issue(id: &str, message: &str) -> PackageBlockingIssue {
    PackageBlockingIssue {
        id: id.to_string(),
        message: message.to_string(),
    }
}

fn string_at(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn bool_at(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn u32_at(value: &Value, key: &str) -> u32 {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
}

fn array_at(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn timestamp() -> String {
    format!("unix:{}", unix_timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn crate_reports_ready() {
        assert!(crate_ready());
        assert_eq!(CRATE_NAME, "adm-new-packaging");
    }

    #[test]
    fn package_step14_success_with_changed_files_succeeds() {
        let result = PackagingService::new().run_package(success_sources());
        assert_eq!(result.validation_report.status, PackageStatus::Success);
        assert_eq!(result.build_report.status, PackageStatus::Success);
        assert_eq!(result.manifest.status, PackageStatus::Success);
        assert_eq!(
            result.validation_report.checks.len(),
            REQUIRED_INTEGRATION_CHECKS.len()
        );
        assert!(result.package_notes.contains("passed packaging readiness"));
    }

    #[test]
    fn package_missing_changed_files_blocks_even_when_step14_succeeded() {
        let mut sources = success_sources();
        sources.actual_project_file_audit = json!({
            "development_path": "UnityProject",
            "actual_changed_files": []
        });
        let result = PackagingService::new().run_package(sources);
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
    fn package_missing_unity_validation_blocks() {
        let mut sources = success_sources();
        sources.unity_validation_summary = json!({
            "valid": false,
            "unity_editor_path": "",
            "validation_count": 0,
            "failed_validation_count": 1
        });
        let result = PackagingService::new().run_package(sources);
        assert_eq!(result.validation_report.status, PackageStatus::Blocked);
        assert!(
            result
                .validation_report
                .blocking_issues
                .iter()
                .any(|issue| { issue.id == "PACKAGE-UNITY-VALIDATION-MISSING" })
        );
    }

    #[test]
    fn package_failed_required_check_blocks_and_is_reported() {
        let mut sources = success_sources();
        sources.integration["checks"]["playmode_smoke_passed"] = Value::Bool(false);
        let result = PackagingService::new().run_package(sources);
        assert_eq!(result.validation_report.status, PackageStatus::Blocked);
        assert!(
            result
                .validation_report
                .checks
                .iter()
                .any(|check| { check.id == "playmode_smoke_passed" && !check.passed })
        );
    }

    fn success_sources() -> PackagingSources {
        let checks = REQUIRED_INTEGRATION_CHECKS
            .iter()
            .map(|id| ((*id).to_string(), Value::Bool(true)))
            .collect::<serde_json::Map<_, _>>();
        PackagingSources {
            integration: json!({
                "status": "success",
                "checks": Value::Object(checks)
            }),
            actual_project_file_audit: json!({
                "development_path": "UnityProject",
                "actual_changed_files": ["Assets/DemoScene.unity"]
            }),
            unity_validation_summary: json!({
                "valid": true,
                "unity_editor_path": "Unity.exe",
                "validation_count": 3,
                "failed_validation_count": 0
            }),
        }
    }
}
