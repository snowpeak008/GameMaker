use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use adm_new_contracts::sdk::{SdkIndex, SdkIndexEntry, SdkReviewStatus, SdkSpec};
use adm_new_foundation::io::now_iso;
use adm_new_foundation::{AdmError, AdmResult, new_stable_id, sha256_hex, write_text_atomic};
use serde::{Deserialize, Serialize};

pub const CRATE_NAME: &str = "adm-new-sdk";
pub const SDK_INDEX_FILE: &str = "_index.json";
pub const SDK_SPEC_TEMPLATE_FILE: &str = "_spec_template.md";
pub const SDK_TOMBSTONES_FILE: &str = "_tombstones.json";
pub const LEGACY_DESKTOP_SDK_FILE: &str = "desktop_sdk_specs.json";
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
    seed_root: Option<PathBuf>,
    quarantine_root: PathBuf,
    migration_archive_root: PathBuf,
}

impl SdkKnowledgeBase {
    pub fn new(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let data_root = infer_data_root(&root);
        Self {
            root,
            seed_root: None,
            quarantine_root: data_root.join("quarantine"),
            migration_archive_root: data_root.join("migration_archive").join("sdk"),
        }
    }

    pub fn with_seed_root(root: impl AsRef<Path>, seed_root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let data_root = infer_data_root(&root);
        Self {
            root,
            seed_root: Some(seed_root.as_ref().to_path_buf()),
            quarantine_root: data_root.join("quarantine"),
            migration_archive_root: data_root.join("migration_archive").join("sdk"),
        }
    }

    pub fn from_project_and_data_roots(
        project_root: impl AsRef<Path>,
        data_root: impl AsRef<Path>,
    ) -> Self {
        let data_root = data_root.as_ref();
        Self {
            root: data_root.join("knowledge").join("sdks"),
            seed_root: Some(project_root.as_ref().join("knowledge").join("sdks")),
            quarantine_root: data_root.join("quarantine"),
            migration_archive_root: data_root.join("migration_archive").join("sdk"),
        }
    }

    pub fn from_project_root(project_root: impl AsRef<Path>) -> Self {
        Self::new(project_root.as_ref().join("knowledge").join("sdks"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn seed_root(&self) -> Option<&Path> {
        self.seed_root.as_deref()
    }

    pub fn quarantine_root(&self) -> &Path {
        &self.quarantine_root
    }

    pub fn index_path(&self) -> PathBuf {
        self.root.join(SDK_INDEX_FILE)
    }

    pub fn template_path(&self) -> PathBuf {
        self.root.join(SDK_SPEC_TEMPLATE_FILE)
    }

    pub fn tombstones_path(&self) -> PathBuf {
        self.root.join(SDK_TOMBSTONES_FILE)
    }

    pub fn spec_path(&self, sdk_id: &str) -> PathBuf {
        self.root.join(safe_sdk_id(sdk_id)).join("spec.json")
    }

    pub fn initialize(&self) -> AdmResult<()> {
        fs::create_dir_all(&self.root)?;
        if !self.index_path().exists() {
            write_json_strict(&self.index_path(), &default_index())?;
        } else {
            self.read_overlay_index()?;
        }
        if !self.template_path().exists() {
            write_text_atomic(&self.template_path(), SDK_SPEC_TEMPLATE)?;
        }
        if let Some(seed_root) = self.seed_root.as_deref() {
            let seed_index = seed_root.join(SDK_INDEX_FILE);
            if seed_index.is_file() {
                self.read_seed_index(&seed_index)?;
            }
        }
        Ok(())
    }

    pub fn read_index(&self) -> AdmResult<SdkIndex> {
        self.initialize()?;
        let mut entries = BTreeMap::<String, SdkIndexEntry>::new();
        if let Some(seed_root) = self.seed_root.as_deref() {
            let seed_path = seed_root.join(SDK_INDEX_FILE);
            if seed_path.is_file() {
                let seed = self.read_seed_index(&seed_path)?;
                entries.extend(
                    seed.sdks
                        .into_iter()
                        .map(|entry| (entry.sdk_id.clone(), entry)),
                );
            }
        }
        let overlay = self.read_overlay_index()?;
        entries.extend(
            overlay
                .sdks
                .into_iter()
                .map(|entry| (entry.sdk_id.clone(), entry)),
        );
        for sdk_id in self.read_tombstones()?.sdk_ids {
            entries.remove(&sdk_id);
        }
        let mut index = SdkIndex {
            schema_version: 1,
            updated_at: overlay.updated_at,
            sdks: entries.into_values().collect(),
        };
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
        validate_index(&index, &self.index_path())?;
        write_json_strict(&self.index_path(), &index)?;
        Ok(index)
    }

    pub fn read_spec(&self, sdk_id: &str) -> AdmResult<Option<SdkSpec>> {
        self.initialize()?;
        let sdk_id = safe_sdk_id(sdk_id);
        if self
            .read_tombstones()?
            .sdk_ids
            .iter()
            .any(|id| id == &sdk_id)
        {
            return Ok(None);
        }
        let path = self.spec_path(&sdk_id);
        if !path.exists() {
            let Some(seed_root) = self.seed_root.as_deref() else {
                return Ok(None);
            };
            let seed_path = seed_root.join(&sdk_id).join("spec.json");
            if !seed_path.is_file() {
                return Ok(None);
            }
            return self.read_seed_spec(&seed_path, &sdk_id).map(Some);
        }
        self.read_overlay_spec(&path, &sdk_id).map(Some)
    }

    pub fn write_spec(&self, spec: SdkSpec) -> AdmResult<SdkSpec> {
        self.validate()?;
        let spec = normalize_spec(spec);
        self.remove_tombstone(&spec.sdk_id)?;
        validate_spec(&spec, &self.spec_path(&spec.sdk_id), Some(&spec.sdk_id))?;
        write_json_strict(&self.spec_path(&spec.sdk_id), &spec)?;
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
            let spec = self.read_spec(&entry.sdk_id)?.ok_or_else(|| {
                AdmError::new(format!(
                    "SDK index references missing spec: {}",
                    entry.sdk_id
                ))
            })?;
            specs.push(spec);
        }
        Ok(specs)
    }

    pub fn validate(&self) -> AdmResult<()> {
        self.list_specs().map(|_| ())
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

    pub fn remove_spec(&self, sdk_id: &str) -> AdmResult<()> {
        self.validate()?;
        let sdk_id = safe_sdk_id(sdk_id);
        let path = self.spec_path(&sdk_id);
        let mut index = self.read_overlay_index()?;
        index.sdks.retain(|entry| entry.sdk_id != sdk_id);
        self.write_index(&index)?;
        let mut tombstones = self.read_tombstones()?;
        if !tombstones.sdk_ids.contains(&sdk_id) {
            tombstones.sdk_ids.push(sdk_id);
            tombstones.sdk_ids.sort();
            write_json_strict(&self.tombstones_path(), &tombstones)?;
        }
        if path.is_file() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn upsert_index(&self, spec: &SdkSpec) -> AdmResult<()> {
        let mut index = self.read_overlay_index()?;
        index.sdks.retain(|entry| entry.sdk_id != spec.sdk_id);
        index.sdks.push(index_entry(spec));
        self.write_index(&index)?;
        Ok(())
    }

    fn read_tombstones(&self) -> AdmResult<SdkOverlayTombstones> {
        if !self.tombstones_path().is_file() {
            return Ok(SdkOverlayTombstones::default());
        }
        let tombstones: SdkOverlayTombstones =
            self.read_overlay_json(&self.tombstones_path(), "SDK overlay tombstones")?;
        if let Err(error) = validate_tombstones(&tombstones, &self.tombstones_path()) {
            return self.isolate_invalid_overlay(&self.tombstones_path(), error);
        }
        Ok(tombstones)
    }

    fn remove_tombstone(&self, sdk_id: &str) -> AdmResult<()> {
        let mut tombstones = self.read_tombstones()?;
        let before = tombstones.sdk_ids.len();
        tombstones.sdk_ids.retain(|id| id != sdk_id);
        if tombstones.sdk_ids.len() != before {
            write_json_strict(&self.tombstones_path(), &tombstones)?;
        }
        Ok(())
    }

    fn read_overlay_index(&self) -> AdmResult<SdkIndex> {
        let index: SdkIndex = self.read_overlay_json(&self.index_path(), "SDK overlay index")?;
        if let Err(error) = validate_index(&index, &self.index_path()) {
            return self.isolate_invalid_overlay(&self.index_path(), error);
        }
        Ok(index)
    }

    fn read_seed_index(&self, path: &Path) -> AdmResult<SdkIndex> {
        let index: SdkIndex = read_seed_json(path, "SDK seed index")?;
        validate_index(&index, path)?;
        Ok(index)
    }

    fn read_overlay_spec(&self, path: &Path, expected_id: &str) -> AdmResult<SdkSpec> {
        let spec: SdkSpec = self.read_overlay_json(path, "SDK overlay spec")?;
        if let Err(error) = validate_spec(&spec, path, Some(expected_id)) {
            return self.isolate_invalid_overlay(path, error);
        }
        Ok(spec)
    }

    fn read_seed_spec(&self, path: &Path, expected_id: &str) -> AdmResult<SdkSpec> {
        let spec: SdkSpec = read_seed_json(path, "SDK seed spec")?;
        validate_spec(&spec, path, Some(expected_id))?;
        Ok(spec)
    }

    fn read_overlay_json<T>(&self, path: &Path, label: &str) -> AdmResult<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        match read_json_strict(path, label) {
            Ok(value) => Ok(value),
            Err(error) => {
                let quarantine = self.quarantine_file(path, "sdk")?;
                Err(AdmError::new(format!(
                    "{error}; corrupt overlay isolated at {}",
                    quarantine.display()
                )))
            }
        }
    }

    fn isolate_invalid_overlay<T>(&self, path: &Path, error: AdmError) -> AdmResult<T> {
        let quarantine = self.quarantine_file(path, "sdk")?;
        Err(AdmError::new(format!(
            "{error}; corrupt overlay isolated at {}",
            quarantine.display()
        )))
    }

    fn quarantine_file(&self, path: &Path, category: &str) -> AdmResult<PathBuf> {
        let relative = path
            .strip_prefix(&self.root)
            .ok()
            .filter(|value| !value.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .or_else(|| path.file_name().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("corrupt-data"));
        let target = self
            .quarantine_root
            .join(category)
            .join(new_stable_id("corrupt")?)
            .join(relative);
        let parent = target
            .parent()
            .ok_or_else(|| AdmError::new("quarantine path has no parent"))?;
        fs::create_dir_all(parent)?;
        fs::rename(path, &target).map_err(|error| {
            AdmError::new(format!(
                "failed to isolate corrupt data {}: {error}",
                path.display()
            ))
        })?;
        Ok(target)
    }

    pub fn migrate_legacy_flat_file(
        &self,
        legacy_path: impl AsRef<Path>,
    ) -> AdmResult<LegacySdkMigrationReport> {
        let legacy_path = legacy_path.as_ref();
        if !legacy_path.is_file() {
            return Ok(LegacySdkMigrationReport::default());
        }
        self.validate()?;
        let legacy_specs: Vec<SdkSpec> =
            match read_json_strict(legacy_path, "legacy desktop SDK store") {
                Ok(specs) => specs,
                Err(error) => {
                    let quarantine = self.quarantine_file(legacy_path, "sdk-legacy")?;
                    return Err(AdmError::new(format!(
                        "{error}; corrupt legacy SDK store isolated at {}",
                        quarantine.display()
                    )));
                }
            };

        for legacy_spec in &legacy_specs {
            if let Err(error) = validate_spec(legacy_spec, legacy_path, None) {
                let quarantine = self.quarantine_file(legacy_path, "sdk-legacy")?;
                return Err(AdmError::new(format!(
                    "{error}; invalid legacy SDK store isolated at {}",
                    quarantine.display()
                )));
            }
        }

        let mut migrated_ids = Vec::new();
        for legacy_spec in legacy_specs {
            let mut target_spec = legacy_spec.clone();
            target_spec.sdk_id = safe_sdk_id(&target_spec.sdk_id);
            if let Some(existing) = self.read_overlay_spec_if_present(&target_spec.sdk_id)? {
                if sdk_specs_semantically_equal(&existing, &target_spec) {
                    migrated_ids.push(existing.sdk_id);
                    continue;
                }
                target_spec.sdk_id = self.available_legacy_collision_id(&target_spec)?;
            }
            let written = self.write_spec(target_spec)?;
            migrated_ids.push(written.sdk_id);
        }

        for sdk_id in &migrated_ids {
            if self.read_spec(sdk_id)?.is_none() {
                return Err(AdmError::new(format!(
                    "legacy SDK migration verification failed for {sdk_id}"
                )));
            }
        }

        let archive_path = self
            .migration_archive_root
            .join(new_stable_id("desktop_sdk_specs_migrated")?)
            .join(LEGACY_DESKTOP_SDK_FILE);
        let archive_parent = archive_path
            .parent()
            .ok_or_else(|| AdmError::new("SDK migration archive has no parent"))?;
        fs::create_dir_all(archive_parent)?;
        fs::rename(legacy_path, &archive_path)?;
        Ok(LegacySdkMigrationReport {
            migrated: true,
            migrated_ids,
            archive_path: Some(archive_path),
        })
    }

    fn read_overlay_spec_if_present(&self, sdk_id: &str) -> AdmResult<Option<SdkSpec>> {
        let path = self.spec_path(sdk_id);
        if !path.is_file() {
            return Ok(None);
        }
        self.read_overlay_spec(&path, sdk_id).map(Some)
    }

    fn available_legacy_collision_id(&self, spec: &SdkSpec) -> AdmResult<String> {
        let base = legacy_collision_id(spec);
        for sequence in 0..10_000_u32 {
            let candidate = if sequence == 0 {
                base.clone()
            } else {
                format!("{base}_{sequence}")
            };
            match self.read_overlay_spec_if_present(&candidate)? {
                Some(existing) if sdk_specs_semantically_equal(&existing, spec) => {
                    return Ok(candidate);
                }
                Some(_) => continue,
                None => return Ok(candidate),
            }
        }
        Err(AdmError::new(
            "failed to allocate a non-destructive legacy SDK migration id",
        ))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LegacySdkMigrationReport {
    pub migrated: bool,
    pub migrated_ids: Vec<String>,
    pub archive_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SdkOverlayTombstones {
    #[serde(default = "sdk_overlay_schema_version")]
    schema_version: u32,
    #[serde(default)]
    sdk_ids: Vec<String>,
}

impl Default for SdkOverlayTombstones {
    fn default() -> Self {
        Self {
            schema_version: sdk_overlay_schema_version(),
            sdk_ids: Vec::new(),
        }
    }
}

fn sdk_overlay_schema_version() -> u32 {
    1
}

fn infer_data_root(overlay_root: &Path) -> PathBuf {
    if overlay_root.file_name().is_some_and(|name| name == "sdks")
        && overlay_root
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "knowledge")
    {
        return overlay_root
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| overlay_root.to_path_buf());
    }
    overlay_root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| overlay_root.to_path_buf())
}

fn read_json_strict<T>(path: &Path, label: &str) -> AdmResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let bytes = fs::read(path).map_err(|error| {
        AdmError::new(format!(
            "failed to read {label} {}: {error}",
            path.display()
        ))
    })?;
    let text = std::str::from_utf8(&bytes).map_err(|error| {
        AdmError::new(format!(
            "invalid UTF-8 in {label} {}: {error}",
            path.display()
        ))
    })?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    serde_json::from_str(text)
        .map_err(|error| AdmError::new(format!("invalid {label} {}: {error}", path.display())))
}

fn read_seed_json<T>(path: &Path, label: &str) -> AdmResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    read_json_strict(path, label).map_err(|error| {
        AdmError::new(format!(
            "{error}; read-only seed rejected without modification"
        ))
    })
}

fn write_json_strict<T>(path: &Path, value: &T) -> AdmResult<()>
where
    T: Serialize,
{
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| AdmError::new(format!("failed to serialize JSON: {error}")))?;
    write_text_atomic(path, &(text + "\n"))
}

fn validate_index(index: &SdkIndex, path: &Path) -> AdmResult<()> {
    if index.schema_version != 1 {
        return Err(AdmError::new(format!(
            "unsupported SDK index schema_version {} in {}",
            index.schema_version,
            path.display()
        )));
    }
    let mut ids = BTreeMap::new();
    for entry in &index.sdks {
        if entry.sdk_id.is_empty() || safe_sdk_id(&entry.sdk_id) != entry.sdk_id {
            return Err(AdmError::new(format!(
                "invalid SDK index id {:?} in {}",
                entry.sdk_id,
                path.display()
            )));
        }
        if entry.name.trim().is_empty() {
            return Err(AdmError::new(format!(
                "SDK index entry {} has an empty name in {}",
                entry.sdk_id,
                path.display()
            )));
        }
        if ids.insert(entry.sdk_id.as_str(), ()).is_some() {
            return Err(AdmError::new(format!(
                "duplicate SDK index id {} in {}",
                entry.sdk_id,
                path.display()
            )));
        }
    }
    Ok(())
}

fn validate_spec(spec: &SdkSpec, path: &Path, expected_id: Option<&str>) -> AdmResult<()> {
    if spec.sdk_id.is_empty() || safe_sdk_id(&spec.sdk_id) != spec.sdk_id {
        return Err(AdmError::new(format!(
            "invalid SDK spec id {:?} in {}",
            spec.sdk_id,
            path.display()
        )));
    }
    if expected_id.is_some_and(|expected| expected != spec.sdk_id) {
        return Err(AdmError::new(format!(
            "SDK spec id {} does not match its path id {} in {}",
            spec.sdk_id,
            expected_id.unwrap_or_default(),
            path.display()
        )));
    }
    if spec.name.trim().is_empty() {
        return Err(AdmError::new(format!(
            "SDK spec {} has an empty name in {}",
            spec.sdk_id,
            path.display()
        )));
    }
    Ok(())
}

fn validate_tombstones(tombstones: &SdkOverlayTombstones, path: &Path) -> AdmResult<()> {
    if tombstones.schema_version != sdk_overlay_schema_version() {
        return Err(AdmError::new(format!(
            "unsupported SDK tombstone schema_version {} in {}",
            tombstones.schema_version,
            path.display()
        )));
    }
    let mut seen = BTreeMap::new();
    for sdk_id in &tombstones.sdk_ids {
        if sdk_id.is_empty() || safe_sdk_id(sdk_id) != *sdk_id {
            return Err(AdmError::new(format!(
                "invalid SDK tombstone id {sdk_id:?} in {}",
                path.display()
            )));
        }
        if seen.insert(sdk_id.as_str(), ()).is_some() {
            return Err(AdmError::new(format!(
                "duplicate SDK tombstone id {sdk_id} in {}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn sdk_specs_semantically_equal(left: &SdkSpec, right: &SdkSpec) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.sdk_id.clear();
    right.sdk_id.clear();
    left.updated_at.clear();
    right.updated_at.clear();
    left == right
}

fn legacy_collision_id(spec: &SdkSpec) -> String {
    let mut semantic = spec.clone();
    semantic.updated_at.clear();
    let bytes = serde_json::to_vec(&semantic).unwrap_or_default();
    let digest = sha256_hex(&bytes);
    safe_sdk_id(&format!(
        "{}_legacy_{}",
        spec.sdk_id,
        digest.get(..8).unwrap_or("conflict")
    ))
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
    fn sdk_overlay_reads_seed_without_writing_it_and_supports_override_and_tombstone() {
        let root = temp_root("sdk_overlay");
        let seed_root = root.join("seed");
        let overlay_root = root.join("overlay");
        let seed = SdkKnowledgeBase::new(&seed_root);
        let seed_spec = seed
            .add_placeholder("Steamworks", "https://seed.test")
            .unwrap();
        let seed_index_before = std::fs::read(seed.index_path()).unwrap();
        let kb = SdkKnowledgeBase::with_seed_root(&overlay_root, &seed_root);

        assert_eq!(
            kb.read_spec(&seed_spec.sdk_id).unwrap().unwrap().source_url,
            "https://seed.test"
        );
        let mut override_spec = seed_spec.clone();
        override_spec.name = "User Steamworks".to_string();
        kb.write_spec(override_spec).unwrap();
        assert_eq!(
            kb.read_spec(&seed_spec.sdk_id).unwrap().unwrap().name,
            "User Steamworks"
        );
        assert_eq!(std::fs::read(seed.index_path()).unwrap(), seed_index_before);

        kb.remove_spec(&seed_spec.sdk_id).unwrap();
        assert!(kb.read_spec(&seed_spec.sdk_id).unwrap().is_none());
        assert!(kb.read_index().unwrap().sdks.is_empty());

        kb.write_spec(seed_spec.clone()).unwrap();
        assert_eq!(
            kb.read_spec(&seed_spec.sdk_id).unwrap().unwrap().name,
            seed_spec.name
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_overlay_is_quarantined_and_never_replaced_with_an_empty_index() {
        let data_root = temp_root("sdk_corrupt_overlay");
        let source_root = data_root.join("source");
        let kb = SdkKnowledgeBase::from_project_and_data_roots(&source_root, &data_root);
        kb.initialize().unwrap();
        std::fs::write(kb.index_path(), b"{broken").unwrap();

        let error = kb.read_index().unwrap_err().to_string();

        assert!(error.contains("isolated"));
        assert!(!kb.index_path().exists());
        assert!(contains_file_named(kb.quarantine_root(), SDK_INDEX_FILE));
        let _ = std::fs::remove_dir_all(data_root);
    }

    #[test]
    fn corrupt_seed_fails_closed_without_modifying_or_quarantining_seed() {
        let root = temp_root("sdk_corrupt_seed");
        let seed_root = root.join("seed");
        std::fs::create_dir_all(&seed_root).unwrap();
        let seed_index = seed_root.join(SDK_INDEX_FILE);
        std::fs::write(&seed_index, b"{broken-seed").unwrap();
        let before = std::fs::read(&seed_index).unwrap();
        let kb = SdkKnowledgeBase::with_seed_root(root.join("overlay"), &seed_root);

        let error = kb.read_index().unwrap_err().to_string();

        assert!(error.contains("read-only seed rejected"));
        assert_eq!(std::fs::read(&seed_index).unwrap(), before);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_flat_store_migrates_once_and_keeps_an_archive() {
        let data_root = temp_root("sdk_legacy_migration");
        let source_root = data_root.join("source");
        let kb = SdkKnowledgeBase::from_project_and_data_roots(&source_root, &data_root);
        kb.initialize().unwrap();
        let legacy_path = kb.root().join(LEGACY_DESKTOP_SDK_FILE);
        let legacy = vec![SdkSpec {
            sdk_id: "steamworks".to_string(),
            name: "Steamworks".to_string(),
            source_url: "https://example.test/sdk".to_string(),
            review_status: SdkReviewStatus::Approved,
            summary: "Legacy data".to_string(),
            integration_notes: Vec::new(),
            api_requirements: Vec::new(),
            risks: Vec::new(),
            last_synced_at: String::new(),
            updated_at: "legacy".to_string(),
        }];
        write_json_strict(&legacy_path, &legacy).unwrap();

        let report = kb.migrate_legacy_flat_file(&legacy_path).unwrap();

        assert!(report.migrated);
        assert!(!legacy_path.exists());
        assert!(report.archive_path.unwrap().is_file());
        assert_eq!(
            kb.read_spec("steamworks").unwrap().unwrap().summary,
            "Legacy data"
        );
        assert!(!kb.migrate_legacy_flat_file(&legacy_path).unwrap().migrated);
        let _ = std::fs::remove_dir_all(data_root);
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

    fn contains_file_named(root: &Path, name: &str) -> bool {
        if !root.is_dir() {
            return false;
        }
        std::fs::read_dir(root).unwrap().flatten().any(|entry| {
            let path = entry.path();
            (path.is_file() && path.file_name().is_some_and(|value| value == name))
                || (path.is_dir() && contains_file_named(&path, name))
        })
    }
}
