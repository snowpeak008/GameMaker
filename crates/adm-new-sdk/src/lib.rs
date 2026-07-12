#![forbid(unsafe_code)]

pub mod ai_extractor;
pub mod knowledge_base;
pub mod skill_overlay;

pub use ai_extractor::{
    ExtractedSdkDocument, build_extraction_prompt, extract_readable_text,
    extract_sdk_spec_with_adapter, sdk_spec_from_completion_data,
};
pub use knowledge_base::{
    CRATE_NAME, LEGACY_DESKTOP_SDK_FILE, LegacySdkMigrationReport, SDK_INDEX_FILE,
    SDK_SPEC_TEMPLATE, SDK_SPEC_TEMPLATE_FILE, SdkKnowledgeBase, SdkKnowledgeService, crate_ready,
    safe_sdk_id,
};
pub use skill_overlay::{
    SKILL_TOMBSTONES_FILE, SkillDocument, SkillFormat, SkillOrigin, SkillOverlayRepository,
    SkillRecord,
};

#[doc(hidden)]
pub const PARITY_MARKER_DOMAIN_SDK_REVIEW: &str = "sdk_service_add_placeholder_and_review_status";
