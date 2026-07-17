use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionScale {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProductEnvelope {
    pub scene_scale: ProductionScale,
    pub system_complexity: ProductionScale,
    pub asset_scale: ProductionScale,
    pub content_volume: ProductionScale,
}

impl ProductEnvelope {
    pub fn fits_within(&self, supported: &Self) -> bool {
        self.violations_against(supported).is_empty()
    }

    pub fn violations_against(&self, supported: &Self) -> Vec<EnvelopeViolation> {
        let dimensions = [
            (
                EnvelopeDimension::SceneScale,
                self.scene_scale,
                supported.scene_scale,
            ),
            (
                EnvelopeDimension::SystemComplexity,
                self.system_complexity,
                supported.system_complexity,
            ),
            (
                EnvelopeDimension::AssetScale,
                self.asset_scale,
                supported.asset_scale,
            ),
            (
                EnvelopeDimension::ContentVolume,
                self.content_volume,
                supported.content_volume,
            ),
        ];

        dimensions
            .into_iter()
            .filter(|(_, required, available)| required > available)
            .map(|(dimension, required, supported)| EnvelopeViolation {
                dimension,
                required,
                supported,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeDimension {
    SceneScale,
    SystemComplexity,
    AssetScale,
    ContentVolume,
}

impl EnvelopeDimension {
    pub fn json_field(self) -> &'static str {
        match self {
            Self::SceneScale => "sceneScale",
            Self::SystemComplexity => "systemComplexity",
            Self::AssetScale => "assetScale",
            Self::ContentVolume => "contentVolume",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EnvelopeViolation {
    pub dimension: EnvelopeDimension,
    pub required: ProductionScale,
    pub supported: ProductionScale,
}
