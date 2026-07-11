#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACT_FAMILY: &str = "package";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageStatus {
    Success,
    Blocked,
}

impl PackageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Blocked => "blocked",
        }
    }
}

pub const REQUIRED_INTEGRATION_CHECKS: &[&str] = &[
    "actual_development_succeeded",
    "scene_assembly_succeeded",
    "demo_scene_exists",
    "visible_content_verified",
    "build_settings_contains_demo_scene",
    "playmode_smoke_passed",
    "unity_batchmode_validation_passed",
    "assets_traced",
    "execution_objects_verified",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageBlockingIssue {
    pub id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageRequiredCheck {
    pub id: String,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageUnityValidation {
    #[serde(default)]
    pub unity_editor_path: String,
    #[serde(default)]
    pub validation_count: u32,
    #[serde(default)]
    pub failed_validation_count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageValidationReport {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub generated_at: String,
    pub status: PackageStatus,
    #[serde(default = "default_source_stage")]
    pub source_stage: u32,
    #[serde(default = "default_source_stage_name")]
    pub source_stage_name: String,
    #[serde(default)]
    pub blocking_issues: Vec<PackageBlockingIssue>,
    #[serde(default)]
    pub checks: Vec<PackageRequiredCheck>,
    #[serde(default)]
    pub development_path: String,
    #[serde(default)]
    pub changed_files: Vec<Value>,
    #[serde(default)]
    pub unity_validation: Option<PackageUnityValidation>,
}

impl PackageValidationReport {
    pub fn changed_files_present(&self) -> bool {
        !self.changed_files.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageBuildReport {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub generated_at: String,
    pub status: PackageStatus,
    #[serde(default = "default_package_type")]
    pub package_type: String,
    #[serde(default = "default_source_stage")]
    pub source_stage: u32,
    #[serde(default = "default_source_stage_name")]
    pub source_stage_name: String,
    #[serde(default)]
    pub development_path: String,
    #[serde(default)]
    pub changed_files: Vec<Value>,
    #[serde(default)]
    pub unity_validation: Option<PackageUnityValidation>,
    #[serde(default)]
    pub blocking_issues: Vec<PackageBlockingIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default = "default_package_type")]
    pub package_type: String,
    pub status: PackageStatus,
    #[serde(default)]
    pub development_path: String,
    #[serde(default)]
    pub changed_files: Vec<Value>,
    #[serde(default = "default_source_stage")]
    pub source_stage: u32,
    #[serde(default = "default_source_stage_name")]
    pub source_stage_name: String,
    pub outputs: PackageManifestOutputs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageManifestOutputs {
    pub package_dir: String,
    pub build_report: String,
    pub package_validation_report: String,
    pub package_notes: String,
}

fn default_schema_version() -> u32 {
    1
}

fn default_source_stage() -> u32 {
    14
}

fn default_source_stage_name() -> String {
    "integration_validation".to_string()
}

fn default_package_type() -> String {
    "current_project_build_package".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn required_checks() -> Vec<PackageRequiredCheck> {
        REQUIRED_INTEGRATION_CHECKS
            .iter()
            .map(|id| PackageRequiredCheck {
                id: (*id).to_string(),
                passed: true,
            })
            .collect()
    }

    #[test]
    fn package_validation_report_roundtrip_preserves_required_checks() {
        let report = PackageValidationReport {
            schema_version: 1,
            generated_at: "2026-07-08T00:00:00".to_string(),
            status: PackageStatus::Success,
            source_stage: 14,
            source_stage_name: "integration_validation".to_string(),
            blocking_issues: Vec::new(),
            checks: required_checks(),
            development_path: "workspace/projects/demo".to_string(),
            changed_files: vec![Value::String("Assets/DemoScene.unity".to_string())],
            unity_validation: Some(PackageUnityValidation {
                unity_editor_path: "Unity.exe".to_string(),
                validation_count: 3,
                failed_validation_count: 0,
            }),
        };

        assert_eq!(report.checks.len(), REQUIRED_INTEGRATION_CHECKS.len());
        let restored: PackageValidationReport =
            serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        assert_eq!(restored, report);
        assert!(restored.changed_files_present());
    }

    #[test]
    fn package_manifest_roundtrip_preserves_current_outputs() {
        let manifest = PackageManifest {
            schema_version: 1,
            generated_at: "2026-07-08T00:00:00".to_string(),
            package_type: "current_project_build_package".to_string(),
            status: PackageStatus::Blocked,
            development_path: String::new(),
            changed_files: Vec::new(),
            source_stage: 14,
            source_stage_name: "integration_validation".to_string(),
            outputs: PackageManifestOutputs {
                package_dir: "outputs/package/current".to_string(),
                build_report: "outputs/package/current/build_report.json".to_string(),
                package_validation_report: "outputs/package/current/package_validation_report.json"
                    .to_string(),
                package_notes: "outputs/package/current/PACKAGE_NOTES.md".to_string(),
            },
        };

        let restored: PackageManifest =
            serde_json::from_str(&serde_json::to_string(&manifest).unwrap()).unwrap();
        assert_eq!(restored, manifest);
    }

    #[test]
    fn package_empty_changed_files_contract_is_blocked() {
        let report = PackageValidationReport {
            schema_version: 1,
            generated_at: String::new(),
            status: PackageStatus::Blocked,
            source_stage: 14,
            source_stage_name: "integration_validation".to_string(),
            blocking_issues: vec![PackageBlockingIssue {
                id: "PACKAGE-NO-ACTUAL-PROJECT-CHANGES".to_string(),
                message: "No actual Unity project changes are available to package.".to_string(),
            }],
            checks: required_checks(),
            development_path: String::new(),
            changed_files: Vec::new(),
            unity_validation: None,
        };
        assert!(!report.changed_files_present());
        assert_eq!(report.status, PackageStatus::Blocked);
    }

    #[test]
    fn package_rejects_invalid_status() {
        let invalid = r#"{"status":"ready","outputs":{"package_dir":"","build_report":"","package_validation_report":"","package_notes":""}}"#;
        assert!(serde_json::from_str::<PackageManifest>(invalid).is_err());
    }
}
