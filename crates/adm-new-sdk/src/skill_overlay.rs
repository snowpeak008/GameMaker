use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use adm_new_foundation::{AdmError, AdmResult, new_stable_id, sha256_hex, write_text_atomic};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const SKILL_TOMBSTONES_FILE: &str = "_tombstones.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillFormat {
    Json,
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillOrigin {
    Seed,
    Overlay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "format", content = "value", rename_all = "snake_case")]
pub enum SkillDocument {
    Json(Value),
    Markdown(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRecord {
    pub skill_id: String,
    pub relative_path: String,
    pub origin: SkillOrigin,
    pub sha256: String,
    pub document: SkillDocument,
}

#[derive(Debug, Clone)]
pub struct SkillOverlayRepository {
    root: PathBuf,
    seed_root: PathBuf,
    quarantine_root: PathBuf,
}

impl SkillOverlayRepository {
    pub fn from_project_and_data_roots(
        project_root: impl AsRef<Path>,
        data_root: impl AsRef<Path>,
    ) -> Self {
        let data_root = data_root.as_ref();
        Self {
            root: data_root.join("knowledge").join("skills"),
            seed_root: project_root.as_ref().join("knowledge").join("skills"),
            quarantine_root: data_root.join("quarantine"),
        }
    }

    pub fn with_roots(
        root: impl AsRef<Path>,
        seed_root: impl AsRef<Path>,
        quarantine_root: impl AsRef<Path>,
    ) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            seed_root: seed_root.as_ref().to_path_buf(),
            quarantine_root: quarantine_root.as_ref().to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn seed_root(&self) -> &Path {
        &self.seed_root
    }

    pub fn quarantine_root(&self) -> &Path {
        &self.quarantine_root
    }

    pub fn initialize(&self) -> AdmResult<()> {
        fs::create_dir_all(&self.root)?;
        self.list().map(|_| ())
    }

    pub fn list(&self) -> AdmResult<Vec<SkillRecord>> {
        let mut merged = BTreeMap::<String, SkillRecord>::new();
        for record in self.read_tree(&self.seed_root, SkillOrigin::Seed)? {
            merged.insert(record.skill_id.clone(), record);
        }
        for record in self.read_tree(&self.root, SkillOrigin::Overlay)? {
            merged.insert(record.skill_id.clone(), record);
        }
        for skill_id in self.read_tombstones()?.skill_ids {
            merged.remove(&skill_id);
        }
        Ok(merged.into_values().collect())
    }

    pub fn get(&self, skill_id: &str) -> AdmResult<Option<SkillRecord>> {
        let skill_id = normalize_skill_id(skill_id)?;
        Ok(self
            .list()?
            .into_iter()
            .find(|record| record.skill_id == skill_id))
    }

    pub fn write_json(&self, skill_id: &str, value: &Value) -> AdmResult<SkillRecord> {
        if !value.is_object() {
            return Err(AdmError::new("skill JSON document must be an object"));
        }
        let skill_id = normalize_skill_id(skill_id)?;
        if descriptor_format(&skill_id) != Some(SkillFormat::Json) {
            return Err(AdmError::new(
                "JSON skill id must end in .json and cannot be a private control file",
            ));
        }
        self.list()?;
        let path = skill_path(&self.root, &skill_id)?;
        let text = serde_json::to_string_pretty(value)
            .map_err(|error| AdmError::new(format!("failed to serialize skill JSON: {error}")))?
            + "\n";
        write_text_atomic(&path, &text)?;
        self.remove_tombstone(&skill_id)?;
        self.read_record(&self.root, &path, SkillOrigin::Overlay)
    }

    pub fn write_markdown(&self, skill_id: &str, text: &str) -> AdmResult<SkillRecord> {
        let skill_id = normalize_skill_id(skill_id)?;
        if descriptor_format(&skill_id) != Some(SkillFormat::Markdown) {
            return Err(AdmError::new("Markdown skill id must end in /SKILL.md"));
        }
        validate_skill_markdown(text, Path::new(&skill_id))?;
        self.list()?;
        let path = skill_path(&self.root, &skill_id)?;
        write_text_atomic(&path, text)?;
        self.remove_tombstone(&skill_id)?;
        self.read_record(&self.root, &path, SkillOrigin::Overlay)
    }

    pub fn remove(&self, skill_id: &str) -> AdmResult<()> {
        let skill_id = normalize_skill_id(skill_id)?;
        if descriptor_format(&skill_id).is_none() {
            return Err(AdmError::new("skill id is not a supported descriptor"));
        }
        self.list()?;
        let path = skill_path(&self.root, &skill_id)?;
        let mut tombstones = self.read_tombstones()?;
        tombstones.skill_ids.insert(skill_id);
        write_json(&self.root.join(SKILL_TOMBSTONES_FILE), &tombstones)?;
        if path.is_file() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn read_tree(&self, root: &Path, origin: SkillOrigin) -> AdmResult<Vec<SkillRecord>> {
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut paths = Vec::new();
        collect_descriptor_paths(root, root, &mut paths)?;
        paths.sort();
        paths
            .into_iter()
            .map(|path| match self.read_record(root, &path, origin) {
                Ok(record) => Ok(record),
                Err(error) if origin == SkillOrigin::Overlay => {
                    let target = self.quarantine_file(&path)?;
                    Err(AdmError::new(format!(
                        "{error}; corrupt skill overlay isolated at {}",
                        target.display()
                    )))
                }
                Err(error) => Err(AdmError::new(format!(
                    "{error}; read-only skill seed rejected without modification"
                ))),
            })
            .collect()
    }

    fn read_record(&self, root: &Path, path: &Path, origin: SkillOrigin) -> AdmResult<SkillRecord> {
        let relative = path.strip_prefix(root).map_err(|error| {
            AdmError::new(format!("failed to derive skill relative path: {error}"))
        })?;
        let skill_id = normalize_skill_id(
            relative
                .to_str()
                .ok_or_else(|| AdmError::new("skill path is not valid UTF-8"))?,
        )?;
        let format = descriptor_format(&skill_id)
            .ok_or_else(|| AdmError::new("unsupported skill descriptor"))?;
        let bytes = fs::read(path)?;
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            AdmError::new(format!(
                "invalid UTF-8 in skill {}: {error}",
                path.display()
            ))
        })?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let document = match format {
            SkillFormat::Json => {
                let value: Value = serde_json::from_str(text).map_err(|error| {
                    AdmError::new(format!("invalid skill JSON {}: {error}", path.display()))
                })?;
                if !value.is_object() {
                    return Err(AdmError::new(format!(
                        "skill JSON must be an object: {}",
                        path.display()
                    )));
                }
                SkillDocument::Json(value)
            }
            SkillFormat::Markdown => {
                validate_skill_markdown(text, path)?;
                SkillDocument::Markdown(text.to_string())
            }
        };
        Ok(SkillRecord {
            relative_path: skill_id.clone(),
            skill_id,
            origin,
            sha256: sha256_hex(&bytes),
            document,
        })
    }

    fn read_tombstones(&self) -> AdmResult<SkillTombstones> {
        let path = self.root.join(SKILL_TOMBSTONES_FILE);
        if !path.is_file() {
            return Ok(SkillTombstones::default());
        }
        let result = (|| -> AdmResult<SkillTombstones> {
            let bytes = fs::read(&path)?;
            let value: SkillTombstones = serde_json::from_slice(&bytes).map_err(|error| {
                AdmError::new(format!(
                    "invalid skill tombstones {}: {error}",
                    path.display()
                ))
            })?;
            value.validate(&path)?;
            Ok(value)
        })();
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                let target = self.quarantine_file(&path)?;
                Err(AdmError::new(format!(
                    "{error}; corrupt skill tombstones isolated at {}",
                    target.display()
                )))
            }
        }
    }

    fn remove_tombstone(&self, skill_id: &str) -> AdmResult<()> {
        let mut tombstones = self.read_tombstones()?;
        if tombstones.skill_ids.remove(skill_id) {
            write_json(&self.root.join(SKILL_TOMBSTONES_FILE), &tombstones)?;
        }
        Ok(())
    }

    fn quarantine_file(&self, path: &Path) -> AdmResult<PathBuf> {
        let relative = path
            .strip_prefix(&self.root)
            .ok()
            .map(Path::to_path_buf)
            .or_else(|| path.file_name().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("corrupt-skill"));
        let target = self
            .quarantine_root
            .join("skills")
            .join(new_stable_id("corrupt")?)
            .join(relative);
        fs::create_dir_all(
            target
                .parent()
                .ok_or_else(|| AdmError::new("skill quarantine target has no parent"))?,
        )?;
        fs::rename(path, &target)?;
        Ok(target)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SkillTombstones {
    schema_version: u32,
    #[serde(default)]
    skill_ids: BTreeSet<String>,
}

impl Default for SkillTombstones {
    fn default() -> Self {
        Self {
            schema_version: 1,
            skill_ids: BTreeSet::new(),
        }
    }
}

impl SkillTombstones {
    fn validate(&self, path: &Path) -> AdmResult<()> {
        if self.schema_version != 1 {
            return Err(AdmError::new(format!(
                "unsupported skill tombstone schema in {}",
                path.display()
            )));
        }
        for skill_id in &self.skill_ids {
            let normalized = normalize_skill_id(skill_id)?;
            if normalized != *skill_id || descriptor_format(skill_id).is_none() {
                return Err(AdmError::new(format!(
                    "invalid skill tombstone id {skill_id:?} in {}",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}

fn collect_descriptor_paths(
    root: &Path,
    directory: &Path,
    output: &mut Vec<PathBuf>,
) -> AdmResult<()> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(AdmError::new(format!(
                "skill repository does not permit links: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            collect_descriptor_paths(root, &path, output)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| AdmError::new(format!("failed to derive skill path: {error}")))?;
            let id = relative
                .to_str()
                .ok_or_else(|| AdmError::new("skill path is not valid UTF-8"))?
                .replace('\\', "/");
            if descriptor_format(&id).is_some() {
                output.push(path);
            }
        }
    }
    Ok(())
}

fn descriptor_format(skill_id: &str) -> Option<SkillFormat> {
    let name = skill_id.rsplit('/').next()?;
    if name == "SKILL.md" {
        Some(SkillFormat::Markdown)
    } else if name.ends_with(".json") && !name.starts_with('_') {
        Some(SkillFormat::Json)
    } else {
        None
    }
}

fn normalize_skill_id(value: &str) -> AdmResult<String> {
    let normalized = value.replace('\\', "/");
    let path = Path::new(&normalized);
    let canonical = path
        .components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| AdmError::new("skill id is not valid UTF-8")),
            _ => Err(AdmError::new("skill id contains a non-portable component")),
        })
        .collect::<AdmResult<Vec<_>>>()?
        .join("/");
    if normalized.trim().is_empty() || path.is_absolute() || canonical != normalized {
        return Err(AdmError::new(format!(
            "skill id must be a portable relative path: {value:?}"
        )));
    }
    Ok(normalized)
}

fn skill_path(root: &Path, skill_id: &str) -> AdmResult<PathBuf> {
    let normalized = normalize_skill_id(skill_id)?;
    Ok(root.join(normalized.replace('/', std::path::MAIN_SEPARATOR_STR)))
}

fn validate_skill_markdown(text: &str, path: &Path) -> AdmResult<()> {
    let normalized = text.replace("\r\n", "\n");
    let mut lines = normalized.lines();
    if lines.next() != Some("---") {
        return Err(AdmError::new(format!(
            "skill Markdown requires front matter: {}",
            path.display()
        )));
    }
    let mut name = false;
    let mut description = false;
    let mut closed = false;
    for line in lines {
        if line == "---" {
            closed = true;
            break;
        }
        let (key, value) = line.split_once(':').unwrap_or(("", ""));
        if !value.trim().trim_matches(['\'', '"']).is_empty() {
            name |= key.trim() == "name";
            description |= key.trim() == "description";
        }
    }
    if !closed || !name || !description {
        return Err(AdmError::new(format!(
            "skill Markdown front matter requires name and description: {}",
            path.display()
        )));
    }
    Ok(())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AdmResult<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| AdmError::new(format!("failed to serialize JSON: {error}")))?;
    write_text_atomic(path, &(text + "\n"))
}
