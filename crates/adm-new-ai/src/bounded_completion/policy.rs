use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::CompletionRisk;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode", deny_unknown_fields)]
pub enum ConfirmationMode {
    Attended,
    Unattended,
    Sample { sample_size: usize },
    AutoAccept,
}

impl ConfirmationMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Attended => "attended",
            Self::Unattended => "unattended",
            Self::Sample { .. } => "sample",
            Self::AutoAccept => "auto_accept",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmationPolicyConfig {
    #[serde(default = "default_ai_enabled")]
    pub ai_enabled: bool,
    pub low_risk: ConfirmationMode,
    pub medium_risk: ConfirmationMode,
    pub high_risk: ConfirmationMode,
    #[serde(default)]
    pub explicit_auto_accept_paths: BTreeSet<String>,
}

impl ConfirmationPolicyConfig {
    pub fn quality_first_r1() -> Self {
        Self {
            ai_enabled: true,
            low_risk: ConfirmationMode::Attended,
            medium_risk: ConfirmationMode::Attended,
            high_risk: ConfirmationMode::Attended,
            explicit_auto_accept_paths: BTreeSet::new(),
        }
    }

    pub fn decision_for(
        &self,
        risk: CompletionRisk,
        write_paths: &BTreeSet<String>,
    ) -> ConfirmationDecision {
        let mode = match risk {
            CompletionRisk::Low => &self.low_risk,
            CompletionRisk::Medium => &self.medium_risk,
            CompletionRisk::High => &self.high_risk,
        };
        match mode {
            ConfirmationMode::AutoAccept
                if self.write_paths_are_explicitly_auto_accepted(write_paths) =>
            {
                ConfirmationDecision {
                    mode: mode.clone(),
                    requires_human: false,
                    auto_commit: true,
                    sample_size: None,
                    reason: "all declared write paths are explicitly configured for auto_accept"
                        .to_string(),
                }
            }
            ConfirmationMode::AutoAccept => ConfirmationDecision {
                mode: mode.clone(),
                requires_human: true,
                auto_commit: false,
                sample_size: None,
                reason:
                    "auto_accept was requested but at least one write path lacks explicit approval"
                        .to_string(),
            },
            ConfirmationMode::Unattended
                if self.write_paths_are_explicitly_auto_accepted(write_paths) =>
            {
                ConfirmationDecision {
                    mode: mode.clone(),
                    requires_human: false,
                    auto_commit: true,
                    sample_size: None,
                    reason: "unattended spec patch is limited to explicitly approved write paths"
                        .to_string(),
                }
            }
            ConfirmationMode::Sample { sample_size } if *sample_size == 0 => {
                ConfirmationDecision {
                    mode: mode.clone(),
                    requires_human: true,
                    auto_commit: false,
                    sample_size: Some(0),
                    reason:
                        "sample confirmation requires sample_size > 0; fail closed until policy is corrected"
                            .to_string(),
                }
            }
            ConfirmationMode::Sample { sample_size } => ConfirmationDecision {
                mode: mode.clone(),
                requires_human: true,
                auto_commit: false,
                sample_size: Some(*sample_size),
                reason: format!(
                    "sample confirmation requires explicit review of {sample_size} sampled candidate(s) before canonical write"
                ),
            },
            _ => ConfirmationDecision {
                mode: mode.clone(),
                requires_human: true,
                auto_commit: false,
                sample_size: None,
                reason: "candidate requires explicit confirmation before canonical write"
                    .to_string(),
            },
        }
    }

    fn write_paths_are_explicitly_auto_accepted(&self, write_paths: &BTreeSet<String>) -> bool {
        !write_paths.is_empty()
            && write_paths.iter().all(|path| {
                self.explicit_auto_accept_paths
                    .iter()
                    .any(|approved| pointer_contains(approved, path))
            })
    }
}

impl Default for ConfirmationPolicyConfig {
    fn default() -> Self {
        Self::quality_first_r1()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmationDecision {
    pub mode: ConfirmationMode,
    pub requires_human: bool,
    pub auto_commit: bool,
    pub sample_size: Option<usize>,
    pub reason: String,
}

fn pointer_contains(parent: &str, candidate: &str) -> bool {
    candidate == parent
        || candidate
            .strip_prefix(parent)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn default_ai_enabled() -> bool {
    true
}
