#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub const CONTRACT_FAMILY: &str = "sdk";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SdkReviewStatus {
    Draft,
    PendingReview,
    Approved,
    Rejected,
}

impl Default for SdkReviewStatus {
    fn default() -> Self {
        Self::Draft
    }
}

impl SdkReviewStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::PendingReview => "pending_review",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkIndex {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub sdks: Vec<SdkIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkIndexEntry {
    pub sdk_id: String,
    pub name: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub review_status: SdkReviewStatus,
    #[serde(default)]
    pub last_synced_at: String,
    #[serde(default)]
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkSpec {
    pub sdk_id: String,
    pub name: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub review_status: SdkReviewStatus,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub integration_notes: Vec<String>,
    #[serde(default)]
    pub api_requirements: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub last_synced_at: String,
    #[serde(default)]
    pub updated_at: String,
}

fn default_schema_version() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_index_and_spec_roundtrip_preserve_review_status() {
        let spec = SdkSpec {
            sdk_id: "steamworks".to_string(),
            name: "Steamworks".to_string(),
            source_url: "https://example.invalid/sdk".to_string(),
            review_status: SdkReviewStatus::Approved,
            summary: "Approved SDK.".to_string(),
            integration_notes: vec!["Use wrapper".to_string()],
            api_requirements: vec!["init".to_string()],
            risks: vec!["platform coupling".to_string()],
            last_synced_at: "2026-07-08T00:00:00".to_string(),
            updated_at: "2026-07-08T00:01:00".to_string(),
        };
        let index = SdkIndex {
            schema_version: 1,
            updated_at: spec.updated_at.clone(),
            sdks: vec![SdkIndexEntry {
                sdk_id: spec.sdk_id.clone(),
                name: spec.name.clone(),
                source_url: spec.source_url.clone(),
                review_status: spec.review_status.clone(),
                last_synced_at: spec.last_synced_at.clone(),
                updated_at: spec.updated_at.clone(),
            }],
        };

        assert_eq!(
            serde_json::from_str::<SdkSpec>(&serde_json::to_string(&spec).unwrap()).unwrap(),
            spec
        );
        assert_eq!(
            serde_json::from_str::<SdkIndex>(&serde_json::to_string(&index).unwrap()).unwrap(),
            index
        );
    }

    #[test]
    fn sdk_rejects_invalid_review_status() {
        let invalid = r#"{"sdk_id":"x","name":"x","review_status":"maybe"}"#;
        assert!(serde_json::from_str::<SdkSpec>(invalid).is_err());
    }
}
