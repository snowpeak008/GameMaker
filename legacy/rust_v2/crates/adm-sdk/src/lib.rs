#![forbid(unsafe_code)]

use adm_validation::{ValidationIssue, ValidationReport, ValidationStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkResource {
    pub sdk_name: String,
    pub category: String,
    pub target_engines: Vec<String>,
    pub target_platforms: Vec<String>,
    pub purpose: String,
    pub integration_risks: Vec<String>,
    pub validation_checklist: Vec<String>,
    pub required_for_build: bool,
    pub ai_explanation: String,
}

impl SdkResource {
    pub fn supports_engine(&self, engine: &str) -> bool {
        self.target_engines
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(engine))
    }

    pub fn supports_platform(&self, platform: &str) -> bool {
        self.target_platforms
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(platform))
    }

    pub fn can_support_ai_explanation(&self) -> bool {
        !self.purpose.trim().is_empty()
            && !self.ai_explanation.trim().is_empty()
            && !self.validation_checklist.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkKnowledgeBase {
    pub resources: Vec<SdkResource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkTargetProfile {
    pub target_engine: String,
    pub target_platform: String,
    pub requires_build_script: bool,
}

impl SdkTargetProfile {
    pub fn new(
        target_engine: impl Into<String>,
        target_platform: impl Into<String>,
        requires_build_script: bool,
    ) -> Self {
        Self {
            target_engine: target_engine.into(),
            target_platform: target_platform.into(),
            requires_build_script,
        }
    }
}

impl SdkKnowledgeBase {
    pub fn default_game_pipeline() -> Self {
        Self::for_target(&SdkTargetProfile::new("Unity", "windows-desktop", true))
    }

    pub fn for_target(profile: &SdkTargetProfile) -> Self {
        let resources = default_catalog()
            .into_iter()
            .filter(|resource| {
                resource.supports_engine(&profile.target_engine)
                    && resource.supports_platform(&profile.target_platform)
            })
            .collect();
        Self { resources }
    }

    pub fn query(&self, keyword: &str) -> Vec<&SdkResource> {
        let keyword = keyword.to_ascii_lowercase();
        self.resources
            .iter()
            .filter(|resource| {
                resource.sdk_name.to_ascii_lowercase().contains(&keyword)
                    || resource.category.to_ascii_lowercase().contains(&keyword)
                    || resource.purpose.to_ascii_lowercase().contains(&keyword)
                    || resource
                        .ai_explanation
                        .to_ascii_lowercase()
                        .contains(&keyword)
            })
            .collect()
    }

    pub fn recommended_for_target(&self, profile: &SdkTargetProfile) -> Vec<&SdkResource> {
        self.resources
            .iter()
            .filter(|resource| {
                resource.supports_engine(&profile.target_engine)
                    && resource.supports_platform(&profile.target_platform)
            })
            .collect()
    }

    pub fn render(&self) -> String {
        let mut document = String::from("# SDK Knowledge Base\n");
        for resource in &self.resources {
            document.push_str(&format!("## {}\n", resource.sdk_name));
            document.push_str(&format!("category={}\n", resource.category));
            document.push_str(&format!(
                "target_engines={}\n",
                resource.target_engines.join(",")
            ));
            document.push_str(&format!(
                "target_platforms={}\n",
                resource.target_platforms.join(",")
            ));
            document.push_str(&format!(
                "required_for_build={}\n",
                resource.required_for_build
            ));
            document.push_str(&format!("purpose={}\n", resource.purpose));
            document.push_str(&format!("ai_explanation={}\n", resource.ai_explanation));
            document.push_str("risks=\n");
            for risk in &resource.integration_risks {
                document.push_str(&format!("- {risk}\n"));
            }
            document.push_str("validation=\n");
            for item in &resource.validation_checklist {
                document.push_str(&format!("- {item}\n"));
            }
        }
        document
    }

    pub fn validate_for_target(&self, profile: &SdkTargetProfile) -> ValidationReport {
        let mut issues = Vec::new();
        if profile.target_engine.trim().is_empty() {
            issues.push(failed(
                "sdk.target_engine.empty",
                "SDK target engine cannot be empty",
            ));
        }
        if profile.target_platform.trim().is_empty() {
            issues.push(failed(
                "sdk.target_platform.empty",
                "SDK target platform cannot be empty",
            ));
        }
        if self.resources.is_empty() {
            issues.push(warning(
                "sdk.resources.empty",
                "SDK knowledge base has no resources",
            ));
        }
        if !self
            .resources
            .iter()
            .any(SdkResource::can_support_ai_explanation)
        {
            issues.push(warning(
                "sdk.ai_explanation.unsupported",
                "No SDK resource can support AI explanation",
            ));
        }
        if !profile.target_engine.trim().is_empty()
            && !self
                .resources
                .iter()
                .any(|resource| resource.supports_engine(&profile.target_engine))
        {
            issues.push(failed(
                "sdk.target_engine.unsupported",
                format!("No SDK resource supports engine {}", profile.target_engine),
            ));
        }
        if !profile.target_platform.trim().is_empty()
            && !self
                .resources
                .iter()
                .any(|resource| resource.supports_platform(&profile.target_platform))
        {
            issues.push(failed(
                "sdk.target_platform.unsupported",
                format!(
                    "No SDK resource supports platform {}",
                    profile.target_platform
                ),
            ));
        }
        if profile.requires_build_script
            && !self
                .resources
                .iter()
                .any(|resource| resource.required_for_build)
        {
            issues.push(failed(
                "sdk.build_script.not_covered",
                "SDK target requires build script coverage",
            ));
        }
        for resource in &self.resources {
            validate_resource(resource, &mut issues);
        }
        ValidationReport::from_issues(issues)
    }
}

fn validate_resource(resource: &SdkResource, issues: &mut Vec<ValidationIssue>) {
    if resource.sdk_name.trim().is_empty() {
        issues.push(failed(
            "sdk.resource.name.empty",
            "SDK resource name is empty",
        ));
    }
    if resource.category.trim().is_empty() {
        issues.push(failed(
            "sdk.resource.category.empty",
            format!("SDK resource {} has no category", resource.sdk_name),
        ));
    }
    if resource.target_engines.is_empty() {
        issues.push(failed(
            "sdk.resource.engines.empty",
            format!("SDK resource {} has no target engines", resource.sdk_name),
        ));
    }
    if resource.target_platforms.is_empty() {
        issues.push(failed(
            "sdk.resource.platforms.empty",
            format!("SDK resource {} has no target platforms", resource.sdk_name),
        ));
    }
    if resource.purpose.trim().is_empty() {
        issues.push(warning(
            "sdk.resource.purpose.empty",
            format!("SDK resource {} has no purpose", resource.sdk_name),
        ));
    }
    if resource.ai_explanation.trim().is_empty() {
        issues.push(warning(
            "sdk.resource.ai_explanation.empty",
            format!("SDK resource {} has no AI explanation", resource.sdk_name),
        ));
    }
    if resource.integration_risks.is_empty() {
        issues.push(warning(
            "sdk.resource.risks.empty",
            format!(
                "SDK resource {} has no integration risks",
                resource.sdk_name
            ),
        ));
    }
    if resource.validation_checklist.is_empty() {
        issues.push(failed(
            "sdk.resource.validation.empty",
            format!(
                "SDK resource {} has no validation checklist",
                resource.sdk_name
            ),
        ));
    }
}

fn default_catalog() -> Vec<SdkResource> {
    vec![
        sdk_resource(
            "Unity Runtime SDK",
            "runtime",
            false,
            "Runtime, scene, time-step, object lifecycle, and player-loop integration for the playable project.",
            "Use this when explaining how generated gameplay systems mount into Unity scenes and lifecycle events.",
            &[
                "Unity version mismatch can break scene serialization.",
                "Runtime lifecycle hooks can drift from generated system assumptions.",
            ],
            &[
                "Confirm Unity editor version and scripting backend.",
                "Confirm the bootstrap scene starts without editor-only APIs.",
                "Confirm one full core loop runs from a fresh player session.",
            ],
        ),
        sdk_resource(
            "Unity Input And Save SDK",
            "input_save",
            false,
            "Input actions, local save slots, and session state persistence for repeated playtesting.",
            "Use this to map development data contracts to controls, save data, and resumed test sessions.",
            &[
                "Input bindings can conflict across keyboard mouse and controller.",
                "Save schema changes can corrupt old playtest state.",
            ],
            &[
                "Confirm default keyboard and controller bindings.",
                "Confirm save/load preserves core loop progress.",
                "Confirm save migration rejects incompatible schema changes.",
            ],
        ),
        sdk_resource(
            "Unity Build Automation SDK",
            "build",
            true,
            "Command-line build, target platform selection, output path planning, and build-log capture.",
            "Use this to explain dry-run and guarded real Unity build execution.",
            &[
                "Unity executable path can be invalid on the local machine.",
                "Build target modules may be missing for Windows desktop.",
            ],
            &[
                "Confirm Unity executable exists before real build.",
                "Confirm build target is Win64 for windows-desktop.",
                "Confirm build report captures command status exit code and expected output.",
            ],
        ),
        sdk_resource(
            "Telemetry And Diagnostics SDK",
            "telemetry",
            false,
            "Gameplay event logging, validation probes, crash context, and reproducible playtest diagnostics.",
            "Use this to connect development telemetry events to validation reports and AI review context.",
            &[
                "Too much telemetry can hide actionable failures.",
                "Diagnostics can expose local paths if reports are shared without filtering.",
            ],
            &[
                "Confirm core loop started/completed events are emitted.",
                "Confirm validation probes are visible in failed run reports.",
                "Confirm diagnostics redact machine-specific secrets.",
            ],
        ),
        sdk_resource(
            "Windows Desktop Packaging SDK",
            "packaging",
            false,
            "Desktop bundle layout, manifest generation, release metadata, and delivery readiness checks.",
            "Use this to explain release doctor and delivery doctor results to the user.",
            &[
                "Release manifests can drift from staged executable content.",
                "Game build and SDK bundles can be stale after pipeline changes.",
            ],
            &[
                "Confirm release manifest hash matches the staged executable.",
                "Confirm game build bundle contains design development assets and SDK artifacts.",
                "Confirm SDK bundle contains sdk/index.adm and required delivery manifest.",
            ],
        ),
    ]
}

fn sdk_resource(
    sdk_name: &str,
    category: &str,
    required_for_build: bool,
    purpose: &str,
    ai_explanation: &str,
    integration_risks: &[&str],
    validation_checklist: &[&str],
) -> SdkResource {
    SdkResource {
        sdk_name: sdk_name.to_string(),
        category: category.to_string(),
        target_engines: vec!["Unity".to_string()],
        target_platforms: vec!["windows-desktop".to_string()],
        purpose: purpose.to_string(),
        integration_risks: integration_risks
            .iter()
            .map(|risk| (*risk).to_string())
            .collect(),
        validation_checklist: validation_checklist
            .iter()
            .map(|item| (*item).to_string())
            .collect(),
        required_for_build,
        ai_explanation: ai_explanation.to_string(),
    }
}

fn failed(code: impl Into<String>, message: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        status: ValidationStatus::Failed,
        code: code.into(),
        message: message.into(),
    }
}

fn warning(code: impl Into<String>, message: impl Into<String>) -> ValidationIssue {
    ValidationIssue {
        status: ValidationStatus::Warning,
        code: code.into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_sdk_validates_for_windows_unity_target() {
        let profile = SdkTargetProfile::new("Unity", "windows-desktop", true);
        let sdk = SdkKnowledgeBase::default_game_pipeline();
        let report = sdk.validate_for_target(&profile);

        assert_eq!(sdk.resources.len(), 5);
        assert_eq!(report.status, ValidationStatus::Passed);
        assert_eq!(sdk.recommended_for_target(&profile).len(), 5);
        assert!(sdk.render().contains("## Unity Build Automation SDK"));
        assert!(sdk.render().contains("required_for_build=true"));
    }

    #[test]
    fn sdk_query_searches_category_purpose_and_ai_summary() {
        let sdk = SdkKnowledgeBase::default_game_pipeline();

        assert_eq!(sdk.query("build").len(), 1);
        assert_eq!(sdk.query("diagnostics").len(), 1);
        assert_eq!(sdk.query("release doctor").len(), 1);
    }

    #[test]
    fn sdk_validation_reports_missing_target_and_checklist() {
        let sdk = SdkKnowledgeBase {
            resources: vec![SdkResource {
                sdk_name: "Broken SDK".to_string(),
                category: String::new(),
                target_engines: Vec::new(),
                target_platforms: Vec::new(),
                purpose: String::new(),
                integration_risks: Vec::new(),
                validation_checklist: Vec::new(),
                required_for_build: false,
                ai_explanation: String::new(),
            }],
        };
        let report = sdk.validate_for_target(&SdkTargetProfile::new("", "", true));

        assert_eq!(report.status, ValidationStatus::Failed);
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "sdk.target_engine.empty")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "sdk.resource.validation.empty")
        );
        assert!(
            report
                .issues
                .iter()
                .any(|issue| issue.code == "sdk.build_script.not_covered")
        );
    }
}
