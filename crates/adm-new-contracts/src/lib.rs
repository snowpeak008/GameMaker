#![forbid(unsafe_code)]

use adm_new_foundation::{AdmError, AdmResult, EvidenceLevel, hash_text};
use serde::{Deserialize, Serialize};

pub mod ai;
pub mod artifact;
pub mod execution_object;
pub mod locale;
pub mod log;
pub mod package;
pub mod patch;
pub mod pipeline;
pub mod project;
pub mod response;
pub mod save;
pub mod schema;
pub mod sdk;
pub mod view;

pub use locale::ArtifactLocale;
pub use response::{CommandError, CommandResponse, Diagnostic, EvidenceRef};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIdentity {
    pub project_id: String,
    pub project_name: String,
    pub genre: String,
    pub platform: String,
    pub player_promise: String,
    pub core_loop: Vec<String>,
}

impl ProjectIdentity {
    pub fn validate(&self) -> AdmResult<()> {
        require_text("project_id", &self.project_id)?;
        require_text("project_name", &self.project_name)?;
        require_text("genre", &self.genre)?;
        require_text("platform", &self.platform)?;
        require_text("player_promise", &self.player_promise)?;
        if self.core_loop.is_empty() {
            return Err(AdmError::new("core_loop must not be empty"));
        }
        for (index, step) in self.core_loop.iter().enumerate() {
            require_text(format!("core_loop[{index}]"), step)?;
        }
        Ok(())
    }

    pub fn stable_hash(&self) -> AdmResult<String> {
        self.validate()?;
        Ok(hash_text(&format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            self.project_id,
            self.project_name,
            self.genre,
            self.platform,
            self.player_promise,
            self.core_loop.join("|")
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageContract {
    pub stage_id: String,
    pub title: String,
    pub input_evidence: Vec<String>,
    pub structured_content: String,
    pub acceptance_criteria: Vec<String>,
    pub downstream_contract: Vec<String>,
    pub evidence_level: EvidenceLevel,
}

impl StageContract {
    pub fn validate(&self) -> AdmResult<()> {
        require_text("stage_id", &self.stage_id)?;
        require_text("title", &self.title)?;
        require_text("structured_content", &self.structured_content)?;
        require_non_empty_list("input_evidence", &self.input_evidence)?;
        require_non_empty_list("acceptance_criteria", &self.acceptance_criteria)?;
        require_non_empty_list("downstream_contract", &self.downstream_contract)?;
        reject_placeholder_text("structured_content", &self.structured_content)?;
        Ok(())
    }

    pub fn render(&self) -> AdmResult<String> {
        self.validate()?;
        Ok(format!(
            "# {}\nstage_id={}\nevidence_level={}\n\n## Input Evidence\n{}\n\n## Structured Content\n{}\n\n## Acceptance Criteria\n{}\n\n## Downstream Contract\n{}\n",
            self.title,
            self.stage_id,
            self.evidence_level.as_str(),
            render_list(&self.input_evidence),
            self.structured_content,
            render_list(&self.acceptance_criteria),
            render_list(&self.downstream_contract)
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub artifact_id: String,
    pub kind: String,
    pub producer: String,
    pub path: String,
    pub content_hash: String,
    pub schema_version: u32,
    pub evidence_level: EvidenceLevel,
}

impl ArtifactRecord {
    pub fn validate(&self) -> AdmResult<()> {
        require_text("artifact_id", &self.artifact_id)?;
        require_text("kind", &self.kind)?;
        require_text("producer", &self.producer)?;
        require_text("path", &self.path)?;
        require_text("content_hash", &self.content_hash)?;
        if self.schema_version == 0 {
            return Err(AdmError::new("schema_version must be greater than zero"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceEvidence {
    pub evidence_id: String,
    pub evidence_level: EvidenceLevel,
    pub command: String,
    pub build_hash: Option<String>,
    pub status: String,
}

impl AcceptanceEvidence {
    pub fn validate(&self) -> AdmResult<()> {
        require_text("evidence_id", &self.evidence_id)?;
        require_text("command", &self.command)?;
        require_text("status", &self.status)?;
        if self.evidence_level == EvidenceLevel::Real && self.build_hash.is_none() {
            return Err(AdmError::new("real evidence must include build_hash"));
        }
        Ok(())
    }
}

fn require_text(name: impl AsRef<str>, value: &str) -> AdmResult<()> {
    if value.trim().is_empty() {
        Err(AdmError::new(format!(
            "{} must not be empty",
            name.as_ref()
        )))
    } else {
        Ok(())
    }
}

fn require_non_empty_list(name: &str, values: &[String]) -> AdmResult<()> {
    if values.is_empty() {
        return Err(AdmError::new(format!("{name} must not be empty")));
    }
    for (index, value) in values.iter().enumerate() {
        require_text(format!("{name}[{index}]"), value)?;
    }
    Ok(())
}

fn reject_placeholder_text(name: &str, value: &str) -> AdmResult<()> {
    let lower = value.to_ascii_lowercase();
    for marker in ["todo", "placeholder", "未命名", "待补充"] {
        if lower.contains(marker) || value.contains(marker) {
            return Err(AdmError::new(format!(
                "{name} contains placeholder marker: {marker}"
            )));
        }
    }
    Ok(())
}

fn render_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("- {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_identity_requires_core_loop() {
        let identity = ProjectIdentity {
            project_id: "p1".to_string(),
            project_name: "Demo".to_string(),
            genre: "tactical puzzle".to_string(),
            platform: "windows".to_string(),
            player_promise: "Clear tactical decisions.".to_string(),
            core_loop: vec!["read".to_string(), "plan".to_string()],
        };
        assert!(identity.validate().is_ok());
        assert!(identity.stable_hash().unwrap().starts_with("fnv64:"));
    }

    #[test]
    fn stage_contract_rejects_placeholder_content() {
        let contract = StageContract {
            stage_id: "step00".to_string(),
            title: "Step00".to_string(),
            input_evidence: vec!["brief".to_string()],
            structured_content: "TODO".to_string(),
            acceptance_criteria: vec!["has identity".to_string()],
            downstream_contract: vec!["step01".to_string()],
            evidence_level: EvidenceLevel::Static,
        };
        assert!(contract.validate().is_err());
    }

    #[test]
    fn real_acceptance_requires_build_hash() {
        let evidence = AcceptanceEvidence {
            evidence_id: "unity".to_string(),
            evidence_level: EvidenceLevel::Real,
            command: "run unity".to_string(),
            build_hash: None,
            status: "passed".to_string(),
        };
        assert!(evidence.validate().is_err());
    }

    #[test]
    fn contract_family_modules_are_exported() {
        let families = [
            project::CONTRACT_FAMILY,
            save::CONTRACT_FAMILY,
            execution_object::CONTRACT_FAMILY,
            pipeline::CONTRACT_FAMILY,
            artifact::CONTRACT_FAMILY,
            ai::CONTRACT_FAMILY,
            package::CONTRACT_FAMILY,
            patch::CONTRACT_FAMILY,
            sdk::CONTRACT_FAMILY,
            schema::CONTRACT_FAMILY,
            log::CONTRACT_FAMILY,
            view::CONTRACT_FAMILY,
        ];
        assert_eq!(families.len(), 12);
        assert!(families.contains(&"project"));
        assert!(families.contains(&"package"));
        assert_eq!(ai::HIGH_CONFIDENCE_THRESHOLD, 0.75);
        assert_eq!(package::REQUIRED_INTEGRATION_CHECKS.len(), 9);
    }

    #[test]
    fn command_response_json_round_trips() {
        let mut response = CommandResponse::success(ProjectIdentity {
            project_id: "p1".to_string(),
            project_name: "Demo".to_string(),
            genre: "strategy".to_string(),
            platform: "windows".to_string(),
            player_promise: "Readable planning.".to_string(),
            core_loop: vec!["choose".to_string(), "resolve".to_string()],
        });
        response.evidence.push(EvidenceRef::new(
            "docs/independence/README.md",
            "ui_contract",
        ));
        response
            .diagnostics
            .push(Diagnostic::info("contract skeleton ready"));

        let json = serde_json::to_string(&response).unwrap();
        let restored: CommandResponse<ProjectIdentity> = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, response);
    }

    #[test]
    fn stage_contract_json_uses_stable_evidence_level_values() {
        let contract = StageContract {
            stage_id: "step00".to_string(),
            title: "Step00".to_string(),
            input_evidence: vec!["brief".to_string()],
            structured_content: "Identity and constraints are captured.".to_string(),
            acceptance_criteria: vec!["identity exists".to_string()],
            downstream_contract: vec!["step01".to_string()],
            evidence_level: EvidenceLevel::Local,
        };

        let json = serde_json::to_string(&contract).unwrap();
        assert!(json.contains("\"evidence_level\":\"local\""));
        let restored: StageContract = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, contract);
    }
}
