#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub mod context;
pub mod freshness;
pub mod identity;
pub mod memory;
pub mod orchestration;
pub mod skill;
pub mod store;

pub const CRATE_NAME: &str = "adm-new-knowledge";
pub const PLANNED_KNOWLEDGE_NON_PYTHON_ASSETS: usize = 715;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KnowledgeDisposition {
    RuntimeLoaded,
    EmbeddedReference,
    DevelopmentMemory,
    DocumentationOnly,
    PlaceholderOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeAssetGroup {
    pub area: String,
    pub relative_root: String,
    pub file_count: usize,
    pub disposition: KnowledgeDisposition,
    pub runtime_default: bool,
}

impl KnowledgeAssetGroup {
    pub fn new(
        area: impl Into<String>,
        relative_root: impl Into<String>,
        file_count: usize,
        disposition: KnowledgeDisposition,
        runtime_default: bool,
    ) -> Self {
        Self {
            area: area.into(),
            relative_root: relative_root.into(),
            file_count,
            disposition,
            runtime_default,
        }
    }

    pub fn is_runtime_loaded(&self) -> bool {
        self.disposition == KnowledgeDisposition::RuntimeLoaded || self.runtime_default
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeInventoryReport {
    pub total_groups: usize,
    pub total_files: usize,
    pub runtime_loaded_groups: usize,
    pub invalid_groups: Vec<String>,
}

impl KnowledgeInventoryReport {
    pub fn passes_a00_gate(&self) -> bool {
        self.total_groups > 0
            && self.total_files == PLANNED_KNOWLEDGE_NON_PYTHON_ASSETS
            && self.runtime_loaded_groups > 0
            && self.invalid_groups.is_empty()
    }
}

pub fn planned_asset_groups() -> Vec<KnowledgeAssetGroup> {
    use KnowledgeDisposition::{
        DevelopmentMemory, DocumentationOnly, EmbeddedReference, PlaceholderOnly, RuntimeLoaded,
    };

    vec![
        KnowledgeAssetGroup::new("top_level_docs", "knowledge", 6, DocumentationOnly, false),
        KnowledgeAssetGroup::new(
            "ai_memory",
            "knowledge/ai_memory",
            277,
            DevelopmentMemory,
            false,
        ),
        KnowledgeAssetGroup::new(
            "decisions",
            "knowledge/decisions",
            16,
            DocumentationOnly,
            false,
        ),
        KnowledgeAssetGroup::new(
            "design_data",
            "knowledge/design_data",
            156,
            RuntimeLoaded,
            true,
        ),
        KnowledgeAssetGroup::new(
            "governance",
            "knowledge/governance",
            51,
            EmbeddedReference,
            false,
        ),
        KnowledgeAssetGroup::new(
            "market_data",
            "knowledge/market_data",
            1,
            RuntimeLoaded,
            true,
        ),
        KnowledgeAssetGroup::new("schemas", "knowledge/schemas", 93, EmbeddedReference, false),
        KnowledgeAssetGroup::new("sdks", "knowledge/sdks", 2, RuntimeLoaded, true),
        KnowledgeAssetGroup::new("skills", "knowledge/skills", 16, RuntimeLoaded, true),
        KnowledgeAssetGroup::new(
            "ucos_non_python",
            "knowledge/ucos",
            97,
            PlaceholderOnly,
            false,
        ),
    ]
}

pub fn summarize_knowledge_assets(groups: &[KnowledgeAssetGroup]) -> KnowledgeInventoryReport {
    KnowledgeInventoryReport {
        total_groups: groups.len(),
        total_files: groups.iter().map(|group| group.file_count).sum(),
        runtime_loaded_groups: groups
            .iter()
            .filter(|group| group.is_runtime_loaded())
            .count(),
        invalid_groups: groups
            .iter()
            .filter(|group| {
                group.area.trim().is_empty()
                    || group.relative_root.trim().is_empty()
                    || group.file_count == 0
            })
            .map(|group| group.area.clone())
            .collect(),
    }
}

pub fn runtime_loaded_groups(groups: &[KnowledgeAssetGroup]) -> Vec<KnowledgeAssetGroup> {
    groups
        .iter()
        .filter(|group| group.is_runtime_loaded())
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{MAX_TOKENS, build_context, estimate_tokens};
    use crate::identity::IdentityEngine;
    use crate::memory::{MemoryEngine, MemoryTier};
    use crate::skill::SkillEngine;
    use crate::store::UcosStore;
    use adm_new_foundation::{new_stable_id, paths::locate_project_root};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn project_store() -> UcosStore {
        let root = locate_project_root(env!("CARGO_MANIFEST_DIR")).unwrap();
        UcosStore::new(root)
    }

    #[test]
    fn planned_groups_match_data_asset_matrix_total() {
        let report = summarize_knowledge_assets(&planned_asset_groups());

        assert!(report.passes_a00_gate());
        assert_eq!(report.total_files, PLANNED_KNOWLEDGE_NON_PYTHON_ASSETS);
        assert_eq!(report.total_groups, 10);
    }

    #[test]
    fn runtime_groups_are_explicitly_filterable() {
        let groups = planned_asset_groups();
        let runtime_groups = runtime_loaded_groups(&groups);

        assert!(
            runtime_groups
                .iter()
                .any(|group| group.area == "design_data")
        );
        assert!(
            runtime_groups
                .iter()
                .any(|group| group.area == "market_data")
        );
        assert!(
            runtime_groups
                .iter()
                .all(KnowledgeAssetGroup::is_runtime_loaded)
        );
    }

    #[test]
    fn knowledge_store_v3_inventory_loads_real_ucos_assets() {
        let inventory = project_store().inventory().unwrap();

        assert_eq!(inventory.schema_files, 10);
        assert_eq!(inventory.identity_files, 4);
        assert_eq!(inventory.registry_skill_count, 15);
        assert_eq!(inventory.capability_skill_files, 15);
        assert_eq!(inventory.plugin_skill_files, 1);
        assert_eq!(inventory.episode_files, 7);
        assert!(inventory.turn_files >= 40);
        assert!(inventory.validation_errors().is_empty());
    }

    #[test]
    fn identity_engine_applies_python_forbidden_action_rules() {
        let engine = IdentityEngine::new(project_store());

        let profile = engine.load_profile().unwrap();
        assert_eq!(profile.role, "GameArchitect");
        assert!(
            profile
                .principles
                .iter()
                .any(|item| item == "Contract First")
        );
        assert_eq!(engine.get_autonomy_level(), 1);

        let blocked = engine.validate_action(&json!({
            "type": "delete",
            "target": "pipeline/artifact_layer/registry.json"
        }));
        assert!(!blocked.allowed);
        assert!(blocked.reason.contains("权威来源"));

        let allowed = engine.validate_action(&json!({
            "type": "read",
            "target": "core/main.py"
        }));
        assert!(allowed.allowed);
    }

    #[test]
    fn skill_engine_discovers_active_skill_by_tags_and_inputs() {
        let engine = SkillEngine::new(project_store());
        let mut inputs = BTreeMap::new();
        inputs.insert("file_path".to_string(), json!("core/main.py"));

        let matches = engine
            .discover(&["devflow".to_string(), "pipeline".to_string()], &inputs)
            .unwrap();

        assert!(
            matches
                .iter()
                .any(|skill| skill.skill_id == "skill_read_file_v1")
        );
        let dep = engine.dependency_report("skill_workflow_composer_v1");
        assert_eq!(dep.dependencies, vec!["skill_skill_selector_v1"]);
        assert!(!dep.has_cycle);
    }

    #[test]
    fn context_builder_enforces_budget_and_formats_real_context() {
        let context = build_context(&project_store(), MAX_TOKENS).unwrap();

        assert!(estimate_tokens(&context) <= MAX_TOKENS + 50);
        assert_eq!(
            context
                .pointer("/identity/profile/role")
                .and_then(|value| value.as_str()),
            Some("GameArchitect")
        );
        assert!(context.get("token_estimate").is_some());
    }

    #[test]
    fn memory_engine_writes_queries_and_decays_short_term_entries() {
        let root = std::env::temp_dir().join(new_stable_id("ucos_memory").unwrap());
        let ucos_root = root.join("knowledge").join("ucos");
        let store = UcosStore::from_ucos_root(&root, &ucos_root);
        let engine = MemoryEngine::new(store.clone());

        let entry_id = engine
            .write(
                MemoryTier::ShortTerm,
                json!({
                    "type": "observation",
                    "title": "Rust parity note",
                    "content": "UCOS memory migration keeps Rust parity evidence",
                    "tags": ["rust", "parity"]
                }),
                "test",
                0.9,
            )
            .unwrap();
        let entries = engine
            .query(MemoryTier::ShortTerm, &["parity".to_string()], 5, 0.2)
            .unwrap();
        assert_eq!(entries[0].entry_id, entry_id);
        assert_eq!(engine.decay_pass(MemoryTier::ShortTerm).unwrap(), 1);
        assert!(store.resolve("knowledge/short_term/index.json").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn freshness_snapshot_covers_python_key_files_without_writing() {
        let root = locate_project_root(env!("CARGO_MANIFEST_DIR")).unwrap();
        let (snapshot, missing) = crate::freshness::build_snapshot(&root).unwrap();

        assert!(snapshot.files.len() >= 30);
        assert!(
            missing.is_empty(),
            "unexpected missing freshness files: {:?}",
            missing
        );
    }
}
