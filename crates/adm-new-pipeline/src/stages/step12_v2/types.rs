use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::stages::step07_v2::{AssetGateStatus, VlmReviewReport, VlmReviewStatus};
use crate::stages::step11_v2::Step11FailureKind;

pub const STEP12_V2_COMPILER_VERSION: &str = "game_spec_step12_asset_binding.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step12Status {
    Success,
    WaitingConfirmation,
    CorrectionRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationStrategy {
    Attended,
    Sample,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetProductionPolicy {
    pub strategy: ConfirmationStrategy,
    pub approved_asset_ids: BTreeSet<String>,
    pub sample_count: usize,
    pub style_distance_threshold: f32,
}

impl AssetProductionPolicy {
    pub fn attended_pending() -> Self {
        Self {
            strategy: ConfirmationStrategy::Attended,
            approved_asset_ids: BTreeSet::new(),
            sample_count: 0,
            style_distance_threshold: 115.0,
        }
    }

    pub fn attended_approved(asset_ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            strategy: ConfirmationStrategy::Attended,
            approved_asset_ids: asset_ids.into_iter().collect(),
            sample_count: 0,
            style_distance_threshold: 115.0,
        }
    }

    pub fn sample(
        sample_count: usize,
        approved_asset_ids: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            strategy: ConfirmationStrategy::Sample,
            approved_asset_ids: approved_asset_ids.into_iter().collect(),
            sample_count,
            style_distance_threshold: 115.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProducedAssetRecord {
    pub asset_id: String,
    pub raw_path: String,
    pub imported_path: String,
    pub content_hash: String,
    pub hard_gate_status: AssetGateStatus,
    pub vlm_status: VlmReviewStatus,
    pub vlm_score: u8,
    pub vlm_summary_hash: String,
    pub style_distance: f32,
    pub style_status: String,
    pub confirmation_required: String,
    pub confirmation_status: String,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetConfirmationRecord {
    pub asset_id: String,
    pub strategy: String,
    pub required: String,
    pub status: String,
    pub sampled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_index: Option<usize>,
    pub sample_count: usize,
    pub requires_human: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetBindingReference {
    pub asset_id: String,
    pub reference_kind: String,
    pub reference_path: String,
    pub runtime_loader_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EngineLoadProbe {
    pub asset_id: String,
    pub imported_path: String,
    pub loaded: bool,
    pub instantiated: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetBindingValidationReport {
    pub status: AssetGateStatus,
    pub orphan_assets: Vec<String>,
    pub missing_referenced_assets: Vec<String>,
    pub load_failures: Vec<String>,
    pub load_probes: Vec<EngineLoadProbe>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step12CorrectionQueueItem {
    pub asset_id: String,
    pub failure_kind: Step11FailureKind,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step12AssetProductionOutput {
    pub schema_version: String,
    pub compiler_version: String,
    pub source_asset_manifest_hash: String,
    pub status: Step12Status,
    pub produced_assets: Vec<ProducedAssetRecord>,
    #[serde(default)]
    pub confirmation_records: Vec<AssetConfirmationRecord>,
    pub bindings: Vec<AssetBindingReference>,
    pub binding_report: AssetBindingValidationReport,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlm_review_report: Option<VlmReviewReport>,
    pub correction_queue: Vec<Step12CorrectionQueueItem>,
    pub output_paths: BTreeMap<String, String>,
}

pub trait EngineAssetLoader {
    fn load(
        &self,
        asset: &ProducedAssetRecord,
        bindings: &[AssetBindingReference],
    ) -> EngineLoadProbe;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicHeadlessAssetLoader;

impl EngineAssetLoader for DeterministicHeadlessAssetLoader {
    fn load(
        &self,
        asset: &ProducedAssetRecord,
        bindings: &[AssetBindingReference],
    ) -> EngineLoadProbe {
        let referenced = bindings
            .iter()
            .any(|binding| binding.asset_id == asset.asset_id);
        EngineLoadProbe {
            asset_id: asset.asset_id.clone(),
            imported_path: asset.imported_path.clone(),
            loaded: false,
            instantiated: false,
            message: if referenced {
                "deterministic loader cannot prove a real engine/headless asset load; use WorkspaceReferenceAssetLoader or a Unity-backed loader".to_string()
            } else {
                "asset was not referenced by a runtime scene or prefab".to_string()
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceReferenceAssetLoader {
    workspace_root: PathBuf,
}

impl WorkspaceReferenceAssetLoader {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }
}

impl EngineAssetLoader for WorkspaceReferenceAssetLoader {
    fn load(
        &self,
        asset: &ProducedAssetRecord,
        bindings: &[AssetBindingReference],
    ) -> EngineLoadProbe {
        if asset.hard_gate_status != AssetGateStatus::Passed {
            return EngineLoadProbe {
                asset_id: asset.asset_id.clone(),
                imported_path: asset.imported_path.clone(),
                loaded: false,
                instantiated: false,
                message: "asset hard gate failed before engine load probing".to_string(),
            };
        }
        if !Path::new(&asset.raw_path).is_file() {
            return EngineLoadProbe {
                asset_id: asset.asset_id.clone(),
                imported_path: asset.imported_path.clone(),
                loaded: false,
                instantiated: false,
                message: "generated asset file is missing before engine load probing".to_string(),
            };
        }
        let asset_bindings = bindings
            .iter()
            .filter(|binding| binding.asset_id == asset.asset_id)
            .collect::<Vec<_>>();
        if asset_bindings.is_empty() {
            return EngineLoadProbe {
                asset_id: asset.asset_id.clone(),
                imported_path: asset.imported_path.clone(),
                loaded: false,
                instantiated: false,
                message: "asset was not referenced by a runtime scene or prefab".to_string(),
            };
        }
        let mut readable_reference = false;
        for binding in asset_bindings {
            let Some(reference_path) =
                workspace_reference_path(&self.workspace_root, &binding.reference_path)
            else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(&reference_path) else {
                continue;
            };
            readable_reference = true;
            if text.contains(&asset.imported_path) || text.contains(&asset.asset_id) {
                return EngineLoadProbe {
                    asset_id: asset.asset_id.clone(),
                    imported_path: asset.imported_path.clone(),
                    loaded: true,
                    instantiated: true,
                    message: format!(
                        "workspace reference {} contains the generated asset binding",
                        binding.reference_path
                    ),
                };
            }
        }
        EngineLoadProbe {
            asset_id: asset.asset_id.clone(),
            imported_path: asset.imported_path.clone(),
            loaded: readable_reference,
            instantiated: false,
            message: "runtime reference files were readable but did not instantiate this asset"
                .to_string(),
        }
    }
}

fn workspace_reference_path(workspace_root: &Path, reference_path: &str) -> Option<PathBuf> {
    let reference = Path::new(reference_path);
    if reference.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::new();
    for component in reference.components() {
        match component {
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    Some(workspace_root.join(normalized))
}
