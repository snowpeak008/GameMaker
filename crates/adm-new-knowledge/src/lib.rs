#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

pub mod context;
pub mod freshness;
pub mod identity;
pub mod manifest;
pub mod memory;
pub mod orchestration;
pub mod skill;
pub mod store;

pub const CRATE_NAME: &str = "adm-new-knowledge";

#[deprecated(
    since = "0.1.0",
    note = "the source resource manifest is authoritative; no fixed planned total exists"
)]
pub const PLANNED_KNOWLEDGE_NON_PYTHON_ASSETS: usize = 0;

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
            && self.total_files > 0
            && self.runtime_loaded_groups > 0
            && self.invalid_groups.is_empty()
    }

    pub fn passes_manifest_gate(&self) -> bool {
        self.passes_a00_gate()
    }
}

#[deprecated(
    since = "0.1.0",
    note = "use load_knowledge_asset_groups with an explicit source project root"
)]
pub fn planned_asset_groups() -> Vec<KnowledgeAssetGroup> {
    std::env::current_dir()
        .ok()
        .and_then(|current| adm_new_foundation::paths::SourceProjectRoot::discover(current).ok())
        .and_then(|root| load_knowledge_asset_groups(root.path()).ok())
        .unwrap_or_default()
}

pub fn load_knowledge_asset_groups(
    project_root: impl AsRef<std::path::Path>,
) -> adm_new_foundation::AdmResult<Vec<KnowledgeAssetGroup>> {
    let source_root = manifest::open_source_project(project_root)?;
    let resource_manifest = manifest::load_source_resource_manifest(&source_root)?;
    let mut groups = Vec::new();
    for declared in resource_manifest
        .groups
        .iter()
        .filter(|group| group.path == "knowledge" || group.path.starts_with("knowledge/"))
    {
        let actual = manifest::measure_resource_tree(source_root.join(&declared.path)?)?;
        if actual.files != declared.files
            || actual.bytes != declared.bytes
            || actual.tree_sha256 != declared.tree_sha256
        {
            return Err(adm_new_foundation::AdmError::new(format!(
                "source resource group does not match its manifest declaration: {}",
                declared.path
            )));
        }
        let area = declared
            .path
            .rsplit('/')
            .next()
            .unwrap_or(&declared.path)
            .to_string();
        let file_count = usize::try_from(declared.files).map_err(|_| {
            adm_new_foundation::AdmError::new(format!(
                "resource group file count does not fit usize: {}",
                declared.path
            ))
        })?;
        let runtime_default = matches!(
            declared.mode.as_str(),
            "required-read-only" | "seed-read-only"
        );
        let disposition = if runtime_default {
            KnowledgeDisposition::RuntimeLoaded
        } else if declared.mode == "test-fixture" {
            KnowledgeDisposition::DocumentationOnly
        } else {
            KnowledgeDisposition::EmbeddedReference
        };
        groups.push(KnowledgeAssetGroup::new(
            area,
            &declared.path,
            file_count,
            disposition,
            runtime_default,
        ));
    }
    groups.sort_by(|left, right| left.relative_root.cmp(&right.relative_root));
    if groups.is_empty() {
        return Err(adm_new_foundation::AdmError::new(
            "source resource manifest declares no knowledge resource groups",
        ));
    }
    Ok(groups)
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
    use crate::freshness::{build_snapshot, update_freshness, write_runtime_freshness};
    use crate::manifest::measure_resource_tree;
    use crate::memory::{MemoryEngine, MemoryTier};
    use crate::store::{RUNTIME_KNOWLEDGE_ROOT, UcosStore};
    use adm_new_foundation::{new_stable_id, sha256_hex};
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    struct TestProject {
        container: PathBuf,
        root: PathBuf,
    }

    impl TestProject {
        fn create() -> Self {
            let container =
                std::env::temp_dir().join(new_stable_id("knowledge_independence").unwrap());
            let root = container.join("独立 Rust 项目");
            std::fs::create_dir_all(root.join("crates/demo/src")).unwrap();
            std::fs::create_dir_all(root.join("knowledge/design_data")).unwrap();
            std::fs::create_dir_all(root.join("web")).unwrap();
            std::fs::create_dir_all(container.join("legacy")).unwrap();
            std::fs::write(
                container.join("legacy/old-project-only.py"),
                "raise RuntimeError('must never be inspected')\n",
            )
            .unwrap();
            std::fs::write(
                root.join("Cargo.toml"),
                "[workspace]\nmembers = [\"crates/demo\"]\nresolver = \"3\"\n",
            )
            .unwrap();
            std::fs::write(root.join("Cargo.lock"), "# fixture\n").unwrap();
            std::fs::write(root.join("web/package-lock.json"), "{}\n").unwrap();
            std::fs::write(
                root.join("crates/demo/Cargo.toml"),
                "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            )
            .unwrap();
            std::fs::write(
                root.join("crates/demo/src/lib.rs"),
                "pub fn independent() -> bool { true }\n",
            )
            .unwrap();
            std::fs::write(
                root.join("knowledge/design_data/catalog.json"),
                "{\"independent\":true}\n",
            )
            .unwrap();
            std::fs::write(
                root.join(".project_root"),
                r#"{"schemaVersion":1,"kind":"source-project-root","projectId":"autodesignmaker-rust-v2","workspaceManifest":"Cargo.toml","lockfiles":["Cargo.lock","web/package-lock.json"],"resourceManifest":"knowledge/resource-manifest.json"}"#,
            )
            .unwrap();
            let measure = measure_resource_tree(root.join("knowledge/design_data")).unwrap();
            let manifest = json!({
                "schemaVersion": 1,
                "projectId": "autodesignmaker-rust-v2",
                "generatedFrom": "test fixture",
                "groups": [{
                    "path": "knowledge/design_data",
                    "files": measure.files,
                    "bytes": measure.bytes,
                    "treeSha256": measure.tree_sha256,
                    "mode": "required-read-only"
                }]
            });
            std::fs::write(
                root.join("knowledge/resource-manifest.json"),
                format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
            )
            .unwrap();
            Self { container, root }
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.container);
        }
    }

    fn source_tree_digest(root: &Path) -> BTreeMap<String, String> {
        fn visit(root: &Path, current: &Path, files: &mut BTreeMap<String, String>) {
            let mut entries = std::fs::read_dir(current)
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                if path.is_dir() {
                    visit(root, &path, files);
                } else {
                    let relative = path
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    files.insert(relative, sha256_hex(&std::fs::read(path).unwrap()));
                }
            }
        }
        let mut files = BTreeMap::new();
        visit(root, root, &mut files);
        files
    }

    #[test]
    fn knowledge_inventory_is_derived_from_the_source_resource_manifest() {
        let project = TestProject::create();
        let groups = load_knowledge_asset_groups(&project.root).unwrap();
        let report = summarize_knowledge_assets(&groups);

        assert!(report.passes_manifest_gate());
        assert_eq!(report.total_files, 1);
        assert_eq!(report.total_groups, 1);
        assert_eq!(groups[0].relative_root, "knowledge/design_data");
        assert!(groups.iter().all(|group| {
            !group.relative_root.contains("ai_memory")
                && !group.relative_root.contains("decisions")
                && !group.relative_root.contains("governance")
                && !group.relative_root.contains("ucos")
        }));
    }

    #[test]
    fn runtime_groups_are_explicitly_filterable() {
        let project = TestProject::create();
        let groups = load_knowledge_asset_groups(&project.root).unwrap();
        let runtime_groups = runtime_loaded_groups(&groups);

        assert_eq!(runtime_groups.len(), 1);
        assert_eq!(runtime_groups[0].area, "design_data");
        assert!(
            runtime_groups
                .iter()
                .all(KnowledgeAssetGroup::is_runtime_loaded)
        );
    }

    #[test]
    fn memory_engine_writes_queries_and_decays_short_term_entries() {
        let root = std::env::temp_dir().join(new_stable_id("ucos_memory").unwrap());
        let runtime_root = root.join("runtime-data");
        let store = UcosStore::from_runtime_data_root(&root, &runtime_root);
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
        assert!(
            runtime_root
                .join(RUNTIME_KNOWLEDGE_ROOT)
                .join("knowledge/short_term/index.json")
                .exists()
        );
        assert!(!root.join("knowledge/ucos").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn freshness_uses_only_the_independent_project_manifest_and_workspace() {
        let project = TestProject::create();
        let (snapshot, missing) = build_snapshot(&project.root).unwrap();

        assert!(missing.is_empty(), "unexpected missing files: {missing:?}");
        assert!(snapshot.files.contains_key(".project_root"));
        assert!(snapshot.files.contains_key("Cargo.toml"));
        assert!(snapshot.files.contains_key("crates/demo/Cargo.toml"));
        assert!(snapshot.files.contains_key("crates/demo/src/lib.rs"));
        assert!(
            snapshot
                .files
                .contains_key("knowledge/design_data/catalog.json")
        );
        assert!(snapshot.files.keys().all(|path| !path.starts_with("core/")));
        assert!(
            snapshot
                .files
                .keys()
                .all(|path| !path.contains("old-project-only"))
        );
    }

    #[test]
    fn compatibility_update_is_read_only_and_runtime_write_is_explicit() {
        let project = TestProject::create();
        let before = source_tree_digest(&project.root);

        let (_, missing) = update_freshness(&project.root).unwrap();

        assert!(missing.is_empty());
        assert_eq!(source_tree_digest(&project.root), before);
        assert!(!project.root.join("knowledge/ai_memory").exists());
        assert!(!project.root.join("knowledge/ucos").exists());

        let refused = write_runtime_freshness(
            &project.root,
            &project.root.join("user_data/knowledge-runtime"),
        )
        .unwrap_err();
        assert!(refused.to_string().contains("must not be inside"));
        assert_eq!(source_tree_digest(&project.root), before);

        let runtime_data = project.container.join("runtime-data");
        let (_, _, output_path) = write_runtime_freshness(&project.root, &runtime_data).unwrap();
        assert!(output_path.starts_with(&runtime_data));
        assert!(output_path.is_file());
        assert_eq!(source_tree_digest(&project.root), before);
    }
}
