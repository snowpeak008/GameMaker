use std::path::Path;

use adm_new_contracts::package::{PackageBlockingIssue, PackageStatus};
use adm_new_foundation::AdmResult;
use adm_new_foundation::io::{read_json, write_json_serializable, write_text};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{PACKAGE_OUTPUT_RELATIVE_DIR, PackagingService, PackagingSources};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageFileRunResult {
    pub status: PackageStatus,
    pub package_dir: String,
    pub package_manifest: String,
    pub build_report: String,
    pub package_validation_report: String,
    pub package_notes: String,
    #[serde(default)]
    pub blocking_issues: Vec<PackageBlockingIssue>,
}

impl PackagingService {
    pub fn load_sources_from_artifacts(&self, artifacts_dir: &Path) -> PackagingSources {
        let stage14 = artifacts_dir.join("stage_14");
        PackagingSources {
            integration: read_json(&stage14.join("integration.json"), json!({})),
            actual_project_file_audit: read_json(
                &stage14.join("actual_project_file_audit.json"),
                json!({}),
            ),
            unity_validation_summary: read_json(
                &stage14.join("unity_validation_summary.json"),
                json!({}),
            ),
        }
    }

    pub fn run_package_to_dir(
        &self,
        artifacts_dir: &Path,
        outputs_dir: &Path,
    ) -> AdmResult<PackageFileRunResult> {
        let package_dir = outputs_dir.join(PACKAGE_OUTPUT_RELATIVE_DIR);
        std::fs::create_dir_all(&package_dir)?;
        let result = self.run_package(self.load_sources_from_artifacts(artifacts_dir));

        let build_report_path = package_dir.join("build_report.json");
        let validation_report_path = package_dir.join("package_validation_report.json");
        let notes_path = package_dir.join("PACKAGE_NOTES.md");
        let manifest_path = package_dir.join("package_manifest.json");

        write_json_serializable(&build_report_path, &result.build_report)?;
        write_json_serializable(&validation_report_path, &result.validation_report)?;
        write_text(&notes_path, &result.package_notes)?;
        write_json_serializable(&manifest_path, &result.manifest)?;

        Ok(PackageFileRunResult {
            status: result.manifest.status,
            package_dir: path_string(&package_dir),
            package_manifest: path_string(&manifest_path),
            build_report: path_string(&build_report_path),
            package_validation_report: path_string(&validation_report_path),
            package_notes: path_string(&notes_path),
            blocking_issues: result.validation_report.blocking_issues,
        })
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::package::REQUIRED_INTEGRATION_CHECKS;
    use adm_new_foundation::new_stable_id;
    use serde_json::Value;
    use std::path::PathBuf;

    #[test]
    fn file_service_writes_package_outputs_for_successful_step14() {
        let root = temp_root("package_file_success");
        let artifacts = root.join("artifacts");
        let outputs = root.join("outputs");
        write_success_stage14(&artifacts);

        let result = PackagingService::new()
            .run_package_to_dir(&artifacts, &outputs)
            .unwrap();

        assert_eq!(result.status, PackageStatus::Success);
        assert!(outputs.join("package/current/build_report.json").exists());
        assert!(
            outputs
                .join("package/current/package_validation_report.json")
                .exists()
        );
        assert!(outputs.join("package/current/PACKAGE_NOTES.md").exists());
        assert!(
            outputs
                .join("package/current/package_manifest.json")
                .exists()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn file_service_returns_blocked_for_missing_stage14_sources() {
        let root = temp_root("package_file_blocked");
        let result = PackagingService::new()
            .run_package_to_dir(&root.join("artifacts"), &root.join("outputs"))
            .unwrap();

        assert_eq!(result.status, PackageStatus::Blocked);
        assert!(!result.blocking_issues.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    fn write_success_stage14(artifacts: &Path) {
        let stage14 = artifacts.join("stage_14");
        std::fs::create_dir_all(&stage14).unwrap();
        let checks = REQUIRED_INTEGRATION_CHECKS
            .iter()
            .map(|id| ((*id).to_string(), Value::Bool(true)))
            .collect::<serde_json::Map<_, _>>();
        std::fs::write(
            stage14.join("integration.json"),
            serde_json::to_string_pretty(&json!({
                "status": "success",
                "checks": Value::Object(checks)
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            stage14.join("actual_project_file_audit.json"),
            serde_json::to_string_pretty(&json!({
                "development_path": "UnityProject",
                "actual_changed_files": ["Assets/DemoScene.unity"]
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            stage14.join("unity_validation_summary.json"),
            serde_json::to_string_pretty(&json!({
                "valid": true,
                "unity_editor_path": "Unity.exe",
                "validation_count": 3,
                "failed_validation_count": 0
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
        std::fs::create_dir_all(&root).unwrap();
        root
    }
}
