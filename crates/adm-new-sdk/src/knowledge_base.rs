use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use adm_new_contracts::sdk::{SdkIndex, SdkIndexEntry, SdkReviewStatus, SdkSpec};
use adm_new_foundation::io::{now_iso, read_json, write_json_serializable, write_text};
use adm_new_foundation::{AdmError, AdmResult};
use serde_json::json;

pub const CRATE_NAME: &str = "adm-new-sdk";
pub const SDK_INDEX_FILE: &str = "_index.json";
pub const SDK_SPEC_TEMPLATE_FILE: &str = "_spec_template.md";
pub const SDK_SPEC_TEMPLATE: &str = "# SDK Spec Template\n\n## Summary\n\n## Integration Notes\n\n## API Requirements\n\n## Risks\n";

pub fn crate_ready() -> bool {
    true
}

pub fn safe_sdk_id(value: &str) -> String {
    let mut clean = String::new();
    let mut last_was_underscore = false;
    for ch in value.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            clean.push(ch);
            last_was_underscore = false;
        } else if !last_was_underscore {
            clean.push('_');
            last_was_underscore = true;
        }
    }
    let clean = clean.trim_matches(['_', '-']).to_string();
    if clean.is_empty() {
        "sdk".to_string()
    } else {
        clean
    }
}

#[derive(Debug, Clone)]
pub struct SdkKnowledgeBase {
    root: PathBuf,
}

impl SdkKnowledgeBase {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn from_project_root(project_root: impl AsRef<Path>) -> Self {
        Self::new(project_root.as_ref().join("knowledge").join("sdks"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index_path(&self) -> PathBuf {
        self.root.join(SDK_INDEX_FILE)
    }

    pub fn template_path(&self) -> PathBuf {
        self.root.join(SDK_SPEC_TEMPLATE_FILE)
    }

    pub fn spec_path(&self, sdk_id: &str) -> PathBuf {
        self.root.join(safe_sdk_id(sdk_id)).join("spec.json")
    }

    pub fn initialize(&self) -> AdmResult<()> {
        std::fs::create_dir_all(&self.root)?;
        if !self.index_path().exists() {
            write_json_serializable(&self.index_path(), &default_index())?;
        }
        if !self.template_path().exists() {
            write_text(&self.template_path(), SDK_SPEC_TEMPLATE)?;
        }
        Ok(())
    }

    pub fn read_index(&self) -> AdmResult<SdkIndex> {
        self.initialize()?;
        let value = read_json(&self.index_path(), json!({}));
        let mut index: SdkIndex = serde_json::from_value(value).unwrap_or_else(|_| default_index());
        index.schema_version = 1;
        index
            .sdks
            .sort_by(|left, right| left.sdk_id.cmp(&right.sdk_id));
        Ok(index)
    }

    pub fn write_index(&self, index: &SdkIndex) -> AdmResult<SdkIndex> {
        self.initialize()?;
        let mut index = index.clone();
        index.schema_version = 1;
        index.updated_at = timestamp();
        index
            .sdks
            .sort_by(|left, right| left.sdk_id.cmp(&right.sdk_id));
        write_json_serializable(&self.index_path(), &index)?;
        Ok(index)
    }

    pub fn read_spec(&self, sdk_id: &str) -> AdmResult<Option<SdkSpec>> {
        self.initialize()?;
        let path = self.spec_path(sdk_id);
        if !path.exists() {
            return Ok(None);
        }
        let value = read_json(&path, json!(null));
        if value.is_null() {
            return Ok(None);
        }
        serde_json::from_value(value)
            .map(Some)
            .map_err(|error| AdmError::new(format!("invalid SDK spec {}: {error}", path.display())))
    }

    pub fn write_spec(&self, spec: SdkSpec) -> AdmResult<SdkSpec> {
        self.initialize()?;
        let spec = normalize_spec(spec);
        write_json_serializable(&self.spec_path(&spec.sdk_id), &spec)?;
        self.upsert_index(&spec)?;
        Ok(spec)
    }

    pub fn add_placeholder(&self, name: &str, source_url: &str) -> AdmResult<SdkSpec> {
        if name.trim().is_empty() {
            return Err(AdmError::new("sdk name cannot be empty"));
        }
        let sdk_id = safe_sdk_id(name);
        if let Some(existing) = self.read_spec(&sdk_id)? {
            return Ok(existing);
        }
        self.write_spec(SdkSpec {
            sdk_id,
            name: name.trim().to_string(),
            source_url: source_url.trim().to_string(),
            review_status: SdkReviewStatus::Draft,
            summary: String::new(),
            integration_notes: Vec::new(),
            api_requirements: Vec::new(),
            risks: Vec::new(),
            last_synced_at: String::new(),
            updated_at: timestamp(),
        })
    }

    pub fn update_review_status(
        &self,
        sdk_id: &str,
        review_status: SdkReviewStatus,
    ) -> AdmResult<SdkSpec> {
        let mut spec = self
            .read_spec(sdk_id)?
            .ok_or_else(|| AdmError::new(format!("unknown sdk_id: {sdk_id}")))?;
        spec.review_status = review_status;
        self.write_spec(spec)
    }

    pub fn list_specs(&self) -> AdmResult<Vec<SdkSpec>> {
        let mut specs = Vec::new();
        for entry in self.read_index()?.sdks {
            if let Some(spec) = self.read_spec(&entry.sdk_id)? {
                specs.push(spec);
            }
        }
        Ok(specs)
    }

    pub fn approved_specs(&self) -> AdmResult<Vec<SdkSpec>> {
        Ok(self
            .list_specs()?
            .into_iter()
            .filter(|spec| spec.review_status == SdkReviewStatus::Approved)
            .collect())
    }

    pub fn approved_prompt_context(&self) -> AdmResult<String> {
        Ok(render_approved_prompt_context(self.approved_specs()?))
    }

    fn upsert_index(&self, spec: &SdkSpec) -> AdmResult<()> {
        let mut index = self.read_index()?;
        index.sdks.retain(|entry| entry.sdk_id != spec.sdk_id);
        index.sdks.push(index_entry(spec));
        self.write_index(&index)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SdkKnowledgeService {
    index: SdkIndex,
    specs: BTreeMap<String, SdkSpec>,
}

impl SdkKnowledgeService {
    pub fn new() -> Self {
        Self {
            index: default_index(),
            specs: BTreeMap::new(),
        }
    }

    pub fn add_placeholder(&mut self, sdk_id: &str, name: &str) -> AdmResult<SdkSpec> {
        self.add_placeholder_with_source_url(sdk_id, name, "")
    }

    pub fn add_placeholder_with_source_url(
        &mut self,
        sdk_id: &str,
        name: &str,
        source_url: &str,
    ) -> AdmResult<SdkSpec> {
        if name.trim().is_empty() {
            return Err(AdmError::new("sdk name cannot be empty"));
        }
        let sdk_id = safe_sdk_id(if sdk_id.trim().is_empty() {
            name
        } else {
            sdk_id
        });
        if let Some(existing) = self.specs.get(&sdk_id) {
            return Ok(existing.clone());
        }
        let spec = SdkSpec {
            sdk_id,
            name: name.trim().to_string(),
            source_url: source_url.trim().to_string(),
            review_status: SdkReviewStatus::Draft,
            summary: String::new(),
            integration_notes: Vec::new(),
            api_requirements: Vec::new(),
            risks: Vec::new(),
            last_synced_at: String::new(),
            updated_at: timestamp(),
        };
        self.upsert_spec(spec.clone());
        Ok(spec)
    }

    pub fn ingest_ai_extracted_spec(&mut self, spec: SdkSpec) -> SdkSpec {
        let mut spec = normalize_spec(spec);
        spec.review_status = SdkReviewStatus::PendingReview;
        self.upsert_spec(spec.clone());
        spec
    }

    pub fn replace_specs(&mut self, specs: Vec<SdkSpec>) {
        self.index = default_index();
        self.specs.clear();
        for spec in specs {
            self.upsert_spec(normalize_spec(spec));
        }
    }

    pub fn set_review_status(
        &mut self,
        sdk_id: &str,
        review_status: SdkReviewStatus,
    ) -> AdmResult<SdkSpec> {
        let sdk_id = safe_sdk_id(sdk_id);
        let spec = self
            .specs
            .get_mut(&sdk_id)
            .ok_or_else(|| AdmError::new(format!("unknown sdk_id: {sdk_id}")))?;
        spec.review_status = review_status;
        spec.updated_at = timestamp();
        let cloned = spec.clone();
        self.refresh_index_entry(&cloned);
        Ok(cloned)
    }

    pub fn index(&self) -> &SdkIndex {
        &self.index
    }

    pub fn list_specs(&self) -> Vec<SdkSpec> {
        self.index
            .sdks
            .iter()
            .filter_map(|entry| self.specs.get(&entry.sdk_id).cloned())
            .collect()
    }

    pub fn approved_context(&self) -> String {
        render_approved_prompt_context(
            self.list_specs()
                .into_iter()
                .filter(|spec| spec.review_status == SdkReviewStatus::Approved),
        )
    }

    fn upsert_spec(&mut self, spec: SdkSpec) {
        self.specs.insert(spec.sdk_id.clone(), spec.clone());
        self.refresh_index_entry(&spec);
    }

    fn refresh_index_entry(&mut self, spec: &SdkSpec) {
        self.index.updated_at = timestamp();
        let entry = index_entry(spec);
        if let Some(existing) = self
            .index
            .sdks
            .iter_mut()
            .find(|existing| existing.sdk_id == spec.sdk_id)
        {
            *existing = entry;
        } else {
            self.index.sdks.push(entry);
        }
        self.index
            .sdks
            .sort_by(|left, right| left.sdk_id.cmp(&right.sdk_id));
    }
}

impl Default for SdkKnowledgeService {
    fn default() -> Self {
        Self::new()
    }
}

fn default_index() -> SdkIndex {
    SdkIndex {
        schema_version: 1,
        updated_at: String::new(),
        sdks: Vec::new(),
    }
}

fn normalize_spec(mut spec: SdkSpec) -> SdkSpec {
    let id_source = if spec.sdk_id.trim().is_empty() {
        spec.name.as_str()
    } else {
        spec.sdk_id.as_str()
    };
    spec.sdk_id = safe_sdk_id(id_source);
    spec.name = if spec.name.trim().is_empty() {
        spec.sdk_id.clone()
    } else {
        spec.name.trim().to_string()
    };
    spec.source_url = spec.source_url.trim().to_string();
    spec.integration_notes = clean_list(spec.integration_notes);
    spec.api_requirements = clean_list(spec.api_requirements);
    spec.risks = clean_list(spec.risks);
    spec.updated_at = timestamp();
    spec
}

fn clean_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn index_entry(spec: &SdkSpec) -> SdkIndexEntry {
    SdkIndexEntry {
        sdk_id: spec.sdk_id.clone(),
        name: spec.name.clone(),
        source_url: spec.source_url.clone(),
        review_status: spec.review_status.clone(),
        last_synced_at: spec.last_synced_at.clone(),
        updated_at: spec.updated_at.clone(),
    }
}

pub fn render_approved_prompt_context(specs: impl IntoIterator<Item = SdkSpec>) -> String {
    let specs = specs.into_iter().collect::<Vec<_>>();
    if specs.is_empty() {
        return String::new();
    }
    let mut lines = vec!["# Approved SDK Context".to_string(), String::new()];
    for spec in specs {
        lines.push(format!("## {}", non_empty(&spec.name, &spec.sdk_id)));
        if !spec.summary.is_empty() {
            lines.push(spec.summary);
        }
        push_list(&mut lines, "Integration Notes", &spec.integration_notes);
        push_list(&mut lines, "API Requirements", &spec.api_requirements);
        push_list(&mut lines, "Risks", &spec.risks);
        lines.push(String::new());
    }
    format!("{}\n", lines.join("\n").trim())
}

fn push_list(lines: &mut Vec<String>, title: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    lines.push(format!("### {title}"));
    lines.extend(values.iter().map(|item| format!("- {item}")));
}

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn timestamp() -> String {
    now_iso()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;

    #[test]
    fn crate_reports_ready() {
        assert!(crate_ready());
        assert_eq!(CRATE_NAME, "adm-new-sdk");
    }

    #[test]
    fn safe_id_matches_python_lowercase_shape() {
        assert_eq!(safe_sdk_id("AdMob SDK"), "admob_sdk");
        assert_eq!(safe_sdk_id("..."), "sdk");
    }

    #[test]
    fn sdk_file_store_initializes_index_and_template() {
        let root = temp_root("sdk_file_init");
        let kb = SdkKnowledgeBase::new(&root);

        kb.initialize().unwrap();

        assert!(root.join(SDK_INDEX_FILE).exists());
        assert!(root.join(SDK_SPEC_TEMPLATE_FILE).exists());
        assert!(kb.read_index().unwrap().sdks.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sdk_file_store_add_review_and_context_matches_python_contract() {
        let root = temp_root("sdk_file_context");
        let kb = SdkKnowledgeBase::new(&root);
        let spec = kb.add_placeholder("AdMob", "https://example.test").unwrap();
        assert_eq!(spec.sdk_id, "admob");
        let mut spec = kb
            .update_review_status("admob", SdkReviewStatus::Approved)
            .unwrap();
        spec.summary = "Rewarded ads SDK.".to_string();
        spec.integration_notes = vec!["Use an adapter interface.".to_string()];
        spec.api_requirements = vec!["Initialize before loading rewarded ads.".to_string()];
        kb.write_spec(spec).unwrap();

        let context = kb.approved_prompt_context().unwrap();

        assert!(context.contains("# Approved SDK Context"));
        assert!(context.contains("## AdMob"));
        assert!(context.contains("Rewarded ads SDK."));
        assert!(context.contains("### API Requirements"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sdk_service_add_placeholder_and_review_status() {
        let mut service = SdkKnowledgeService::new();
        let spec = service.add_placeholder("steamworks", "Steamworks").unwrap();
        assert_eq!(spec.review_status, SdkReviewStatus::Draft);
        service
            .set_review_status("steamworks", SdkReviewStatus::Approved)
            .unwrap();
        assert_eq!(
            service.index().sdks[0].review_status,
            SdkReviewStatus::Approved
        );
    }

    #[test]
    fn sdk_service_add_placeholder_preserves_source_url() {
        let mut service = SdkKnowledgeService::new();
        let spec = service
            .add_placeholder_with_source_url(
                "steamworks",
                "Steamworks",
                " https://partner.steamgames.com/doc/sdk ",
            )
            .unwrap();
        assert_eq!(spec.source_url, "https://partner.steamgames.com/doc/sdk");
        assert_eq!(
            service.index().sdks[0].source_url,
            "https://partner.steamgames.com/doc/sdk"
        );
    }

    #[test]
    fn sdk_service_ai_extraction_cannot_auto_approve_and_approved_context_filters() {
        let mut service = SdkKnowledgeService::new();
        let extracted = service.ingest_ai_extracted_spec(SdkSpec {
            sdk_id: "ads".to_string(),
            name: "Ads SDK".to_string(),
            source_url: String::new(),
            review_status: SdkReviewStatus::Approved,
            summary: "AI extracted".to_string(),
            integration_notes: vec!["Initialize before menu".to_string()],
            api_requirements: Vec::new(),
            risks: Vec::new(),
            last_synced_at: String::new(),
            updated_at: String::new(),
        });
        assert_eq!(extracted.review_status, SdkReviewStatus::PendingReview);
        assert!(service.approved_context().is_empty());
        service
            .set_review_status("ads", SdkReviewStatus::Approved)
            .unwrap();
        let context = service.approved_context();
        assert!(context.contains("Ads SDK"));
        assert!(context.contains("Initialize before menu"));
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
        std::fs::create_dir_all(&root).unwrap();
        root
    }
}
