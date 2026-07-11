#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use adm_new_ai::CompletionAdapter;
use adm_new_contracts::ai::{ModelResult, ModelResultStatus, ModelTask};
use adm_new_contracts::patch::{PatchRecord, PatchStatus, PatchTask};
use adm_new_foundation::{
    AdmError, AdmResult, StableDirectoryIdentity, acquire_project_write_lock, ensure_relative_path,
    new_stable_id, sanitize_identifier, sha256_hex, unix_timestamp,
    write_bytes_atomic as foundation_write_bytes_atomic, write_text_atomic,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const CRATE_NAME: &str = "adm-new-patch";

pub const SCRIPT_SUFFIXES: &[&str] = &["cs", "js", "ts", "py"];
pub const FEATURE_FLAG_KEYWORDS: &[&str] = &[
    "rewarded", "ad", "sdk", "purchase", "payment", "广告", "复活", "支付",
];

pub fn crate_ready() -> bool {
    true
}

pub fn new_patch_id() -> AdmResult<String> {
    new_stable_id("patch")
}

pub fn route_for_task(task: &PatchTask) -> Vec<String> {
    let text = [
        task.title.as_str(),
        task.description.as_str(),
        &task.affected_systems.join(" "),
        &task.expected_files.join(" "),
    ]
    .join(" ")
    .to_lowercase();
    let mut route = vec!["light_validator".to_string()];
    if ["scene", "prefab", "ui", "resource", "manifest", "bootstrap"]
        .iter()
        .any(|keyword| text.contains(keyword))
    {
        route.extend([
            "step13_scene_assembly".to_string(),
            "step14_integration_validation".to_string(),
        ]);
    } else if ["script", ".cs", "logic", "sdk", "广告", "复活"]
        .iter()
        .any(|keyword| text.contains(keyword))
    {
        route.push("step14_integration_validation".to_string());
    }
    route
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchStore {
    root: PathBuf,
}

struct PatchStoreLock(fs::File);

impl Drop for PatchStoreLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

impl PatchStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn from_project_root(project_root: impl AsRef<Path>, draft_session_id: &str) -> Self {
        let session = if draft_session_id.trim().is_empty() {
            "cli"
        } else {
            draft_session_id
        };
        Self::new(
            project_root
                .as_ref()
                .join("drafts")
                .join(session)
                .join("patches"),
        )
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn patch_dir(&self, patch_id: &str) -> PathBuf {
        self.root.join(safe_patch_id(patch_id))
    }

    pub fn manifest_path(&self, patch_id: &str) -> PathBuf {
        self.patch_dir(patch_id).join("patch_manifest.json")
    }

    pub fn write(&self, record: &PatchRecord) -> AdmResult<PatchRecord> {
        let mut record = record.clone();
        if record.patch_id.trim().is_empty() {
            record.patch_id = new_patch_id()?;
        }
        record.patch_id = safe_patch_id(&record.patch_id);
        let _lock = self.lock_patch(&record.patch_id)?;
        self.write_unlocked(&record)
    }

    fn write_unlocked(&self, record: &PatchRecord) -> AdmResult<PatchRecord> {
        let mut record = record.clone();
        record.patch_id = safe_patch_id(&record.patch_id);
        if record.created_at.trim().is_empty() {
            record.created_at = timestamp();
        }
        record.updated_at = timestamp();
        let text = serde_json::to_string_pretty(&record)
            .map_err(|error| AdmError::new(format!("failed to serialize patch record: {error}")))?;
        write_text_atomic(&self.manifest_path(&record.patch_id), &text)?;
        Ok(record)
    }

    fn lock_patch(&self, patch_id: &str) -> AdmResult<PatchStoreLock> {
        let patch_dir = self.patch_dir(patch_id);
        fs::create_dir_all(&patch_dir)
            .map_err(|_| AdmError::new("patch record directory is unavailable"))?;
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(patch_dir.join(".apply.lock"))
            .map_err(|_| AdmError::new("patch record lock is unavailable"))?;
        fs2::FileExt::try_lock_exclusive(&file)
            .map_err(|_| AdmError::new("patch record is busy in another process"))?;
        Ok(PatchStoreLock(file))
    }

    pub fn read(&self, patch_id: &str) -> AdmResult<Option<PatchRecord>> {
        let path = self.manifest_path(patch_id);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && !patch_metadata_is_link(&metadata) => {}
            Ok(_) => return Err(AdmError::new("patch manifest path is unsafe")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(AdmError::new("patch manifest could not be inspected")),
        }
        read_patch_manifest(&path).map(Some)
    }

    pub fn get(&self, patch_id: &str) -> AdmResult<PatchRecord> {
        self.read(patch_id)?
            .ok_or_else(|| AdmError::new(format!("unknown patch: {patch_id}")))
    }

    pub fn list(&self) -> AdmResult<Vec<PatchRecord>> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let path = entry.path().join("patch_manifest.json");
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.is_file() && !patch_metadata_is_link(&metadata) => {}
                Ok(_) => return Err(AdmError::new("patch manifest path is unsafe")),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return Err(AdmError::new("patch manifest could not be inspected")),
            }
            records.push(read_patch_manifest(&path)?);
        }
        records.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.patch_id.cmp(&right.patch_id))
        });
        Ok(records)
    }
}

#[derive(Debug, Clone)]
pub struct PatchAnalyzer {
    store: Option<PatchStore>,
}

impl PatchAnalyzer {
    pub fn new(store: Option<PatchStore>) -> Self {
        Self { store }
    }

    pub fn analyze(&self, request: &str, persist: bool) -> AdmResult<PatchRecord> {
        let mut tasks = inferred_tasks_from_request(request);
        if tasks.is_empty() {
            tasks.push(PatchTask {
                task_id: "PATCH-001".to_string(),
                title: "Patch task 1".to_string(),
                description: request.to_string(),
                affected_systems: Vec::new(),
                expected_files: Vec::new(),
                validation_route: Vec::new(),
                requires_iteration: false,
            });
        }
        self.analyze_tasks(
            request,
            tasks,
            "Deterministic patch request analysis.",
            persist,
        )
    }

    pub fn analyze_tasks(
        &self,
        request: &str,
        tasks: Vec<PatchTask>,
        analysis_summary: &str,
        persist: bool,
    ) -> AdmResult<PatchRecord> {
        if request.trim().is_empty() {
            return Err(AdmError::new("patch request cannot be empty"));
        }
        let now = timestamp();
        let tasks = tasks
            .into_iter()
            .enumerate()
            .map(|(index, mut task)| {
                if task.task_id.trim().is_empty() {
                    task.task_id = format!("PATCH-{:03}", index + 1);
                }
                if task.title.trim().is_empty() {
                    task.title = format!("Patch task {}", index + 1);
                }
                if task.validation_route.is_empty() {
                    task.validation_route = route_for_task(&task);
                }
                task
            })
            .collect::<Vec<_>>();
        let record = PatchRecord {
            patch_id: new_patch_id()?,
            request: request.trim().to_string(),
            status: PatchStatus::Analyzed,
            created_at: now.clone(),
            updated_at: now,
            tasks,
            changed_files: Vec::new(),
            validation_summary: Value::Null,
            analysis_summary: analysis_summary.to_string(),
            executor_result: Value::Null,
            promoted_iteration_spec: String::new(),
            errors: Vec::new(),
        };
        if persist {
            if let Some(store) = &self.store {
                return store.write(&record);
            }
        }
        Ok(record)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightValidator {
    project_root: PathBuf,
}

impl LightValidator {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            project_root: project_root.into(),
        }
    }

    pub fn validate_files(&self, changed_files: &[String]) -> Value {
        let mut blockers = Vec::new();
        let mut warnings = Vec::new();
        for relative in changed_files {
            let path = match ensure_relative_path(&self.project_root, relative) {
                Ok(path) => path,
                Err(error) => {
                    blockers.push(json!({
                        "code": "PATCH_PATH_INVALID",
                        "path": relative,
                        "message": error.to_string(),
                    }));
                    continue;
                }
            };
            if !path.exists() {
                blockers.push(json!({"code": "PATCH_FILE_MISSING", "path": relative}));
                continue;
            }
            if is_script_path(&path) {
                let text = fs::read(&path)
                    .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
                    .unwrap_or_default();
                for (opening, closing, code) in [
                    ('(', ')', "UNBALANCED_PARENTHESES"),
                    ('{', '}', "UNBALANCED_BRACES"),
                    ('[', ']', "UNBALANCED_BRACKETS"),
                ] {
                    if !balanced(&text, opening, closing) {
                        blockers.push(json!({"code": code, "path": relative}));
                    }
                }
                let lower = format!("{relative}\n{text}").to_lowercase();
                if FEATURE_FLAG_KEYWORDS
                    .iter()
                    .any(|keyword| lower.contains(&keyword.to_lowercase()))
                    && !text.contains("_ENABLED")
                    && !text.contains("FeatureFlag")
                {
                    warnings.push(json!({
                        "code": "FEATURE_FLAG_NOT_DETECTED",
                        "path": relative,
                        "message": "Risky SDK/ad/payment-like change lacks an obvious feature flag.",
                    }));
                }
            }
        }
        if changed_files.len() > 8 {
            warnings.push(json!({
                "code": "PATCH_TOUCHES_MANY_FILES",
                "count": changed_files.len(),
                "message": "Patch touches many files; consider promoting to a formal iteration.",
            }));
        }
        json!({
            "schema_version": 1,
            "status": if blockers.is_empty() { "passed" } else { "blocked" },
            "changed_files": changed_files,
            "blockers": blockers,
            "warnings": warnings,
        })
    }
}

pub trait PatchRunner {
    fn run(&self, record: &PatchRecord) -> AdmResult<PatchRunResult>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchRunResult {
    pub status: String,
    pub changed_files: Vec<String>,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub errors: Vec<String>,
}

impl PatchRunResult {
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            status: "failed".to_string(),
            changed_files: Vec::new(),
            stdout: String::new(),
            errors: vec![message.into()],
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "status": self.status,
            "changed_files": self.changed_files,
            "stdout": self.stdout,
            "errors": self.errors,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CodexPatchRunner<A> {
    project_root: PathBuf,
    adapter: A,
    timeout_seconds: u64,
}

impl<A> CodexPatchRunner<A>
where
    A: CompletionAdapter,
{
    pub fn new(project_root: impl Into<PathBuf>, adapter: A) -> Self {
        Self {
            project_root: project_root.into(),
            adapter,
            timeout_seconds: 1800,
        }
    }

    pub fn with_timeout_seconds(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    pub fn build_task(&self, record: &PatchRecord) -> AdmResult<ModelTask> {
        let expected_files = declared_expected_files(record);
        if expected_files.is_empty() {
            return Err(AdmError::new(
                "Patch has no declared expected_files; refusing unrestricted execution.",
            ));
        }
        Ok(ModelTask {
            task_id: record.patch_id.clone(),
            prompt: build_codex_patch_prompt(record, &expected_files),
            input_files: Vec::new(),
            output_files: expected_files.clone(),
            allowed_write_paths: expected_files,
            timeout_seconds: self.timeout_seconds,
            sandbox: "workspace-write".to_string(),
            cwd: self.project_root.to_string_lossy().to_string(),
        })
    }
}

impl<A> PatchRunner for CodexPatchRunner<A>
where
    A: CompletionAdapter,
{
    fn run(&self, record: &PatchRecord) -> AdmResult<PatchRunResult> {
        let expected_files = declared_expected_files(record);
        if expected_files.is_empty() {
            return Ok(PatchRunResult::failed(
                "Patch has no declared expected_files; refusing unrestricted execution.",
            ));
        }
        let project_root = fs::canonicalize(&self.project_root)
            .map_err(|_| AdmError::new("patch project root is unavailable"))?;
        if !project_root.is_dir() {
            return Ok(PatchRunResult::failed("Patch project root is unavailable."));
        }
        let before = snapshot_patch_files(&project_root, &expected_files)?;
        let isolated = IsolatedPatchRoot::new()?;
        for relative in &expected_files {
            if let Some(source) = safe_patch_existing_file(&project_root, relative)? {
                let target = safe_patch_target(isolated.path(), relative)?;
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(source, target)?;
            }
        }
        let isolated_before = patch_directory_manifest(isolated.verified_path()?)?;
        let mut task = self.build_task(record)?;
        task.cwd = isolated.path().to_string_lossy().to_string();
        let result: ModelResult = match self.adapter.generate(&task) {
            Ok(result) => result,
            Err(_) => {
                return Ok(PatchRunResult::failed(
                    "Patch adapter invocation failed; private provider output was not persisted.",
                ));
            }
        };
        if result.status != ModelResultStatus::Succeeded {
            return Ok(PatchRunResult::failed(
                "Patch adapter reported a failure; private provider output was not persisted.",
            ));
        }
        let isolated_root = isolated.verified_path()?;
        let isolated_after = patch_directory_manifest(isolated_root)?;
        let declared = expected_files.iter().cloned().collect::<BTreeSet<_>>();
        let unexpected = changed_patch_manifest_paths(&isolated_before, &isolated_after)
            .into_iter()
            .filter(|path| !declared.contains(path))
            .collect::<Vec<_>>();
        if !unexpected.is_empty() {
            return Ok(PatchRunResult::failed(format!(
                "Patch adapter changed {} undeclared file(s); no project files were committed.",
                unexpected.len()
            )));
        }
        if expected_files
            .iter()
            .any(|path| !isolated_after.contains_key(path))
        {
            return Ok(PatchRunResult::failed(
                "Patch adapter did not produce every declared output; no project files were committed.",
            ));
        }
        let _project_write_lock = acquire_project_write_lock(&project_root)?;
        if snapshot_patch_files(&project_root, &expected_files)? != before {
            return Ok(PatchRunResult::failed(
                "Patch targets changed while the adapter was running; no generated files were committed.",
            ));
        }
        let changed_files = expected_files
            .iter()
            .filter(|path| {
                before.get(*path).and_then(|value| value.as_ref()) != isolated_after.get(*path)
            })
            .cloned()
            .collect::<Vec<_>>();
        commit_patch_outputs(
            &project_root,
            &isolated,
            &changed_files,
            &before,
            &isolated_after,
        )?;
        Ok(PatchRunResult {
            status: "success".to_string(),
            changed_files,
            stdout: "patch adapter completed in an isolated work root".to_string(),
            errors: Vec::new(),
        })
    }
}

struct IsolatedPatchRoot {
    path: PathBuf,
    identity: StableDirectoryIdentity,
}

fn read_patch_file_bounded(path: &Path, max_bytes: u64, message: &str) -> AdmResult<Vec<u8>> {
    use std::io::Read;

    let file = fs::File::open(path).map_err(|_| AdmError::new(message))?;
    let metadata = file.metadata().map_err(|_| AdmError::new(message))?;
    if !metadata.is_file() || metadata.len() > max_bytes {
        return Err(AdmError::new(message));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| AdmError::new(message))?;
    if bytes.len() as u64 > max_bytes {
        return Err(AdmError::new(message));
    }
    Ok(bytes)
}

fn read_patch_manifest(path: &Path) -> AdmResult<PatchRecord> {
    let bytes = read_patch_file_bounded(path, 4 * 1024 * 1024, "patch manifest is unavailable")?;
    serde_json::from_slice(&bytes)
        .map_err(|error| AdmError::new(format!("invalid patch manifest: {error}")))
}

fn write_new_patch_file(path: &Path, bytes: &[u8]) -> AdmResult<()> {
    use std::io::Write;

    let result = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(path);
        return Err(AdmError::new("patch rollback target could not be restored"));
    }
    Ok(())
}

impl IsolatedPatchRoot {
    fn new() -> AdmResult<Self> {
        let path = std::env::temp_dir().join(new_stable_id("adm-patch-work")?);
        fs::create_dir(&path)
            .map_err(|_| AdmError::new("isolated patch work root could not be created"))?;
        let path = fs::canonicalize(path)
            .map_err(|_| AdmError::new("isolated patch work root could not be resolved"))?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| AdmError::new("isolated patch work root could not be resolved"))?;
        if patch_metadata_is_link(&metadata) || !metadata.is_dir() {
            return Err(AdmError::new("isolated patch work root is unsafe"));
        }
        let identity = StableDirectoryIdentity::capture(&path)?;
        Ok(Self { path, identity })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn verified_path(&self) -> AdmResult<&Path> {
        let metadata = fs::symlink_metadata(&self.path)
            .map_err(|_| AdmError::new("isolated patch work root changed during execution"))?;
        if patch_metadata_is_link(&metadata)
            || !metadata.is_dir()
            || !self.identity.matches_path(&self.path)?
            || fs::canonicalize(&self.path)
                .map_err(|_| AdmError::new("isolated patch work root changed during execution"))?
                != self.path
        {
            return Err(AdmError::new(
                "isolated patch work root changed during execution",
            ));
        }
        Ok(&self.path)
    }
}

impl Drop for IsolatedPatchRoot {
    fn drop(&mut self) {
        let safe = self.verified_path().is_ok();
        self.identity.release();
        if safe {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn snapshot_patch_files(
    root: &Path,
    files: &[String],
) -> AdmResult<BTreeMap<String, Option<String>>> {
    const MAX_PATCH_FILE_BYTES: u64 = 32 * 1024 * 1024;
    let mut snapshot = BTreeMap::new();
    for relative in files {
        let hash = match safe_patch_existing_file(root, relative)? {
            Some(path) => {
                let metadata = fs::metadata(&path)?;
                if metadata.len() > MAX_PATCH_FILE_BYTES {
                    return Err(AdmError::new(
                        "declared patch file exceeds the verification size limit",
                    ));
                }
                Some(sha256_hex(&read_patch_file_bounded(
                    &path,
                    MAX_PATCH_FILE_BYTES,
                    "declared patch file exceeds the verification size limit",
                )?))
            }
            None => None,
        };
        snapshot.insert(relative.clone(), hash);
    }
    Ok(snapshot)
}

fn safe_patch_existing_file(root: &Path, relative: &str) -> AdmResult<Option<PathBuf>> {
    let target = safe_patch_target(root, relative)?;
    if !target.exists() {
        return Ok(None);
    }
    let canonical_root =
        fs::canonicalize(root).map_err(|_| AdmError::new("patch project root is unavailable"))?;
    let canonical = fs::canonicalize(&target)
        .map_err(|_| AdmError::new("declared patch file could not be resolved"))?;
    if !canonical.starts_with(&canonical_root) || !canonical.is_file() {
        return Err(AdmError::new(
            "declared patch file escapes its configured project root",
        ));
    }
    Ok(Some(canonical))
}

fn safe_patch_target(root: &Path, relative: &str) -> AdmResult<PathBuf> {
    let canonical_root =
        fs::canonicalize(root).map_err(|_| AdmError::new("patch project root is unavailable"))?;
    if !canonical_root.is_dir() {
        return Err(AdmError::new("patch project root is unavailable"));
    }
    let target = ensure_relative_path(&canonical_root, relative)?;
    if target == canonical_root {
        return Err(AdmError::new("declared patch target must be a child file"));
    }
    reject_patch_link_components(&canonical_root, &target)?;
    let mut ancestor = target.as_path();
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| AdmError::new("declared patch target has no safe parent"))?;
    }
    let canonical_ancestor = fs::canonicalize(ancestor)
        .map_err(|_| AdmError::new("declared patch target parent could not be resolved"))?;
    if !canonical_ancestor.starts_with(&canonical_root) {
        return Err(AdmError::new(
            "declared patch target escapes its configured project root",
        ));
    }
    Ok(target)
}

fn reject_patch_link_components(root: &Path, target: &Path) -> AdmResult<()> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| AdmError::new("declared patch target escapes its project root"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if patch_metadata_is_link(&metadata) => {
                return Err(AdmError::new(
                    "declared patch target must not traverse a symbolic link or junction",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => {
                return Err(AdmError::new(
                    "declared patch target components could not be verified",
                ));
            }
        }
    }
    Ok(())
}

fn patch_metadata_is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

fn patch_directory_manifest(root: &Path) -> AdmResult<BTreeMap<String, String>> {
    const MAX_PATCH_FILE_BYTES: u64 = 32 * 1024 * 1024;
    const MAX_PATCH_TOTAL_BYTES: u64 = 128 * 1024 * 1024;
    const MAX_PATCH_ENTRIES: usize = 4_096;
    const MAX_PATCH_DEPTH: usize = 32;

    fn visit(
        root: &Path,
        directory: &Path,
        depth: usize,
        entries: &mut usize,
        total: &mut u64,
        manifest: &mut BTreeMap<String, String>,
    ) -> AdmResult<()> {
        if depth > MAX_PATCH_DEPTH {
            return Err(AdmError::new(
                "patch adapter output tree exceeds the depth limit",
            ));
        }
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            *entries = entries.saturating_add(1);
            if *entries > MAX_PATCH_ENTRIES {
                return Err(AdmError::new(
                    "patch adapter output tree exceeds the entry limit",
                ));
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if patch_metadata_is_link(&metadata) {
                return Err(AdmError::new(
                    "patch adapter created a symbolic link or junction",
                ));
            }
            if metadata.is_dir() {
                visit(root, &path, depth + 1, entries, total, manifest)?;
            } else if metadata.is_file() {
                if metadata.len() > MAX_PATCH_FILE_BYTES {
                    return Err(AdmError::new(
                        "patch adapter output exceeds the per-file verification limit",
                    ));
                }
                let bytes = read_patch_file_bounded(
                    &path,
                    MAX_PATCH_FILE_BYTES,
                    "patch adapter output exceeds the per-file verification limit",
                )?;
                *total = total.saturating_add(bytes.len() as u64);
                if *total > MAX_PATCH_TOTAL_BYTES {
                    return Err(AdmError::new(
                        "patch adapter outputs exceed the total verification limit",
                    ));
                }
                let relative = path
                    .strip_prefix(root)
                    .map_err(|_| AdmError::new("patch adapter output escaped its work root"))?
                    .to_string_lossy()
                    .replace('\\', "/");
                manifest.insert(relative, sha256_hex(&bytes));
            }
        }
        Ok(())
    }

    let canonical_root = fs::canonicalize(root)
        .map_err(|_| AdmError::new("isolated patch work root could not be resolved"))?;
    let mut manifest = BTreeMap::new();
    let mut entries = 0;
    let mut total = 0;
    visit(
        &canonical_root,
        &canonical_root,
        0,
        &mut entries,
        &mut total,
        &mut manifest,
    )?;
    Ok(manifest)
}

fn changed_patch_manifest_paths(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<String> {
    before
        .keys()
        .chain(after.keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| before.get(path) != after.get(path))
        .collect()
}

fn commit_patch_outputs(
    project_root: &Path,
    isolated: &IsolatedPatchRoot,
    changed_files: &[String],
    expected: &BTreeMap<String, Option<String>>,
    staged_hashes: &BTreeMap<String, String>,
) -> AdmResult<()> {
    let mut prepared = Vec::new();
    for relative in changed_files {
        let isolated_root = isolated.verified_path()?;
        let source = safe_patch_existing_file(isolated_root, relative)?
            .ok_or_else(|| AdmError::new("declared patch output is missing"))?;
        let captured = source.parent().unwrap_or(isolated_root).join(format!(
            ".{}.captured",
            new_stable_id("patch-staged-output")?
        ));
        isolated.verified_path()?;
        fs::rename(&source, &captured)
            .map_err(|_| AdmError::new("declared patch output could not be sealed"))?;
        let metadata = fs::symlink_metadata(&captured)
            .map_err(|_| AdmError::new("sealed patch output is unavailable"))?;
        if patch_metadata_is_link(&metadata) {
            return Err(AdmError::new("sealed patch output is an unsafe link"));
        }
        let generated = read_patch_file_bounded(
            &captured,
            32 * 1024 * 1024,
            "sealed patch output exceeds the verification limit",
        )?;
        if staged_hashes.get(relative) != Some(&sha256_hex(&generated)) {
            return Err(AdmError::new(
                "declared patch output changed after isolated verification",
            ));
        }
        let previous = safe_patch_existing_file(project_root, relative)?
            .map(|path| {
                read_patch_file_bounded(
                    &path,
                    32 * 1024 * 1024,
                    "declared patch target exceeds the verification limit",
                )
            })
            .transpose()?;
        let current_hash = previous.as_ref().map(|bytes| sha256_hex(bytes));
        if expected.get(relative) != Some(&current_hash) {
            return Err(AdmError::new("declared patch target changed before commit"));
        }
        prepared.push((relative.clone(), generated, previous));
    }

    let mut committed = 0;
    for (relative, generated, previous) in &prepared {
        let current = safe_patch_existing_file(project_root, relative)?
            .map(|path| {
                read_patch_file_bounded(
                    &path,
                    32 * 1024 * 1024,
                    "declared patch target exceeds the verification limit",
                )
            })
            .transpose()?;
        if current.as_ref() != previous.as_ref() {
            if rollback_patch_outputs(project_root, &prepared[..committed]).is_err() {
                return Err(AdmError::new(
                    "declared patch target changed and rollback was incomplete",
                ));
            }
            return Err(AdmError::new("declared patch target changed during commit"));
        }
        if let Err(error) = write_patch_file_atomic(project_root, relative, generated) {
            if rollback_patch_outputs(project_root, &prepared[..committed]).is_err() {
                return Err(AdmError::new(
                    "declared patch commit failed and rollback was incomplete",
                ));
            }
            return Err(error);
        }
        committed += 1;
    }
    Ok(())
}

fn rollback_patch_outputs(
    root: &Path,
    committed: &[(String, Vec<u8>, Option<Vec<u8>>)],
) -> AdmResult<()> {
    let mut rollback_failed = false;
    for (relative, generated, previous) in committed.iter().rev() {
        let Ok(target) = safe_patch_target(root, relative) else {
            rollback_failed = true;
            continue;
        };
        if !target.exists() {
            continue;
        }
        let parent = target.parent().unwrap_or(root);
        let quarantine = parent.join(format!(
            ".{}.rollback",
            match new_stable_id("patch-rollback") {
                Ok(id) => id,
                Err(_) => {
                    rollback_failed = true;
                    continue;
                }
            }
        ));
        if fs::rename(&target, &quarantine).is_err() {
            rollback_failed = true;
            continue;
        }
        let captured = match read_patch_file_bounded(
            &quarantine,
            32 * 1024 * 1024,
            "patch rollback source could not be verified",
        ) {
            Ok(bytes) => bytes,
            Err(_) => {
                rollback_failed = true;
                continue;
            }
        };
        let restore = if &captured == generated {
            previous.as_deref()
        } else {
            Some(captured.as_slice())
        };
        if let Some(restore) = restore
            && write_new_patch_file(&target, restore).is_err()
        {
            rollback_failed = true;
            continue;
        }
        if fs::remove_file(&quarantine).is_err() {
            rollback_failed = true;
        }
    }
    if rollback_failed {
        Err(AdmError::new("patch rollback was incomplete"))
    } else {
        Ok(())
    }
}

fn write_patch_file_atomic(root: &Path, relative: &str, bytes: &[u8]) -> AdmResult<()> {
    let target = safe_patch_target(root, relative)?;
    let parent = target
        .parent()
        .ok_or_else(|| AdmError::new("declared patch output has no parent"))?;
    create_patch_directories(root, parent)?;
    let target = safe_patch_target(root, relative)?;
    let parent = fs::canonicalize(
        target
            .parent()
            .ok_or_else(|| AdmError::new("declared patch output has no parent"))?,
    )?;
    let file_name = target
        .file_name()
        .ok_or_else(|| AdmError::new("declared patch output has no file name"))?;
    let target = parent.join(file_name);
    foundation_write_bytes_atomic(&target, bytes)
}

fn create_patch_directories(root: &Path, directory: &Path) -> AdmResult<()> {
    let canonical_root =
        fs::canonicalize(root).map_err(|_| AdmError::new("patch project root is unavailable"))?;
    let relative = directory
        .strip_prefix(&canonical_root)
        .map_err(|_| AdmError::new("declared patch parent escapes the project root"))?;
    let mut current = canonical_root;
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !patch_metadata_is_link(&metadata) => {}
            Ok(_) => return Err(AdmError::new("declared patch parent is unsafe")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match fs::create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(_) => {
                        return Err(AdmError::new("declared patch parent could not be created"));
                    }
                }
                let metadata = fs::symlink_metadata(&current)?;
                if !metadata.is_dir() || patch_metadata_is_link(&metadata) {
                    return Err(AdmError::new("declared patch parent is unsafe"));
                }
            }
            Err(_) => return Err(AdmError::new("declared patch parent could not be verified")),
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct PatchExecutor {
    project_root: PathBuf,
    store: PatchStore,
}

impl PatchExecutor {
    pub fn new(project_root: impl Into<PathBuf>, store: PatchStore) -> Self {
        Self {
            project_root: project_root.into(),
            store,
        }
    }

    pub fn apply<R: PatchRunner>(&self, patch_id: &str, runner: &R) -> AdmResult<PatchRecord> {
        let _lock = self.store.lock_patch(patch_id)?;
        let mut record = self.store.get(patch_id)?;
        let expected_files = declared_expected_files(&record);
        if record.executor_result.get("status").and_then(Value::as_str) == Some("applying") {
            let before = record
                .executor_result
                .get("before_hashes")
                .cloned()
                .ok_or_else(|| AdmError::new("patch applying intent has no before snapshot"))?;
            let before = serde_json::from_value::<BTreeMap<String, Option<String>>>(before)
                .map_err(|_| AdmError::new("patch applying intent has an invalid snapshot"))?;
            if snapshot_patch_files(&self.project_root, &expected_files)? != before {
                return Err(AdmError::new(
                    "previous patch attempt changed project files and requires manual reconciliation",
                ));
            }
        }
        let before = snapshot_patch_files(&self.project_root, &expected_files)?;
        record.executor_result = json!({
            "status": "applying",
            "attempt_id": new_stable_id("patch-apply-attempt")?,
            "expected_files": expected_files,
            "before_hashes": before,
        });
        record = self.store.write_unlocked(&record)?;
        match runner.run(&record) {
            Ok(result) => {
                record.changed_files = result.changed_files.clone();
                record.status = if result.status == "success" || result.status == "succeeded" {
                    PatchStatus::Applied
                } else {
                    PatchStatus::Failed
                };
                record.executor_result = result.to_json();
                if !result.errors.is_empty() {
                    record.errors.extend(result.errors);
                }
            }
            Err(error) => {
                record.status = PatchStatus::Failed;
                record.errors.push(error.to_string());
            }
        }
        self.store.write_unlocked(&record)
    }

    pub fn validate(&self, patch_id: &str) -> AdmResult<PatchRecord> {
        let _lock = self.store.lock_patch(patch_id)?;
        let mut record = self.store.get(patch_id)?;
        let summary = LightValidator::new(&self.project_root).validate_files(&record.changed_files);
        record.validation_summary = summary.clone();
        record.status = if summary.get("status").and_then(Value::as_str) == Some("passed") {
            PatchStatus::Validated
        } else {
            PatchStatus::Failed
        };
        self.store.write_unlocked(&record)
    }

    pub fn promote(&self, patch_id: &str, iteration_spec: &str) -> AdmResult<PatchRecord> {
        let _lock = self.store.lock_patch(patch_id)?;
        let mut record = self.store.get(patch_id)?;
        record.status = PatchStatus::Promoted;
        record.promoted_iteration_spec = iteration_spec.to_string();
        self.store.write_unlocked(&record)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PatchService {
    records: BTreeMap<String, PatchRecord>,
}

impl PatchService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn analyze_request_shell(
        &mut self,
        request: &str,
        tasks: Vec<PatchTask>,
    ) -> AdmResult<PatchRecord> {
        let record = PatchAnalyzer::new(None).analyze_tasks(
            request,
            tasks,
            "Shell provided patch tasks.",
            false,
        )?;
        self.write(record.clone());
        Ok(record)
    }

    pub fn write(&mut self, mut record: PatchRecord) {
        if record.created_at.is_empty() {
            record.created_at = timestamp();
        }
        if record.updated_at.is_empty() {
            record.updated_at = record.created_at.clone();
        }
        self.records.insert(record.patch_id.clone(), record);
    }

    pub fn list(&self) -> Vec<PatchRecord> {
        let mut records = self.records.values().cloned().collect::<Vec<_>>();
        records.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.patch_id.cmp(&right.patch_id))
        });
        records
    }

    pub fn get(&self, patch_id: &str) -> AdmResult<PatchRecord> {
        self.records
            .get(patch_id)
            .cloned()
            .ok_or_else(|| AdmError::new(format!("unknown patch_id: {patch_id}")))
    }

    pub fn set_status(&mut self, patch_id: &str, status: PatchStatus) -> AdmResult<PatchRecord> {
        let record = self
            .records
            .get_mut(patch_id)
            .ok_or_else(|| AdmError::new(format!("unknown patch_id: {patch_id}")))?;
        record.status = status;
        record.updated_at = timestamp();
        Ok(record.clone())
    }

    pub fn filter_by_status(&self, status: PatchStatus) -> Vec<PatchRecord> {
        self.list()
            .into_iter()
            .filter(|record| record.status == status)
            .collect()
    }

    pub fn approved_context(&self) -> Vec<PatchRecord> {
        self.list()
            .into_iter()
            .filter(|record| {
                matches!(
                    record.status,
                    PatchStatus::Validated | PatchStatus::Promoted
                )
            })
            .collect()
    }
}

fn inferred_tasks_from_request(request: &str) -> Vec<PatchTask> {
    let expected_files = infer_expected_files(request);
    let title = request
        .split(['\n', '.', '。'])
        .map(str::trim)
        .find(|text| !text.is_empty())
        .unwrap_or("Patch task 1")
        .chars()
        .take(80)
        .collect::<String>();
    vec![PatchTask {
        task_id: "PATCH-001".to_string(),
        title,
        description: request.to_string(),
        affected_systems: infer_affected_systems(request),
        expected_files,
        validation_route: Vec::new(),
        requires_iteration: false,
    }]
}

fn infer_expected_files(request: &str) -> Vec<String> {
    let mut files = BTreeSet::new();
    for token in request.split_whitespace() {
        let clean = token
            .trim_matches(|ch: char| {
                matches!(
                    ch,
                    ',' | ';' | ':' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}'
                )
            })
            .replace('\\', "/");
        if clean.contains('/')
            && [".cs", ".js", ".ts", ".py", ".rs", ".json", ".md"]
                .iter()
                .any(|suffix| clean.to_lowercase().ends_with(suffix))
            && !clean.contains("..")
        {
            files.insert(clean.trim_start_matches('/').to_string());
        }
    }
    files.into_iter().collect()
}

fn infer_affected_systems(request: &str) -> Vec<String> {
    let lower = request.to_lowercase();
    let mut systems = Vec::new();
    for (keyword, system) in [
        ("ui", "UI"),
        ("scene", "Scene"),
        ("prefab", "Prefab"),
        ("sdk", "SDK"),
        ("save", "Save"),
        ("pipeline", "Pipeline"),
        ("广告", "Ads"),
        ("复活", "Revive"),
    ] {
        if lower.contains(keyword) {
            systems.push(system.to_string());
        }
    }
    systems
}

fn safe_patch_id(value: &str) -> String {
    sanitize_identifier(value).unwrap_or_else(|_| "patch".to_string())
}

fn declared_expected_files(record: &PatchRecord) -> Vec<String> {
    let mut files = BTreeSet::new();
    for task in &record.tasks {
        for path in &task.expected_files {
            let path = path.replace('\\', "/");
            let clean = path.trim().trim_matches('/').to_string();
            if !clean.is_empty() && !clean.contains("..") {
                files.insert(clean);
            }
        }
    }
    files.into_iter().collect()
}

fn build_codex_patch_prompt(record: &PatchRecord, expected_files: &[String]) -> String {
    let tasks_json =
        serde_json::to_string_pretty(&record.tasks).unwrap_or_else(|_| "[]".to_string());
    let allowed = expected_files
        .iter()
        .map(|path| format!("- {path}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Apply this AutoDesignMaker quick patch.\n\
You may edit only the declared expected files.\n\
Do not run broad refactors or change unrelated files.\n\n\
Patch request:\n{}\n\n\
Patch tasks JSON:\n{}\n\n\
Allowed files:\n{}",
        record.request, tasks_json, allowed
    )
}

fn is_script_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| {
            SCRIPT_SUFFIXES
                .iter()
                .any(|suffix| suffix.eq_ignore_ascii_case(extension))
        })
        .unwrap_or(false)
}

fn balanced(text: &str, opening: char, closing: char) -> bool {
    let mut depth = 0i32;
    for ch in text.chars() {
        if ch == opening {
            depth += 1;
        } else if ch == closing {
            depth -= 1;
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

fn timestamp() -> String {
    format!("unix:{}", unix_timestamp())
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::ai::{ModelResult, ModelResultStatus};

    #[test]
    fn crate_reports_ready() {
        assert!(crate_ready());
        assert_eq!(CRATE_NAME, "adm-new-patch");
    }

    #[test]
    fn patch_analyzer_writes_structured_record_and_routes_validation() {
        let root = temp_root("patch_analyzer");
        let store = PatchStore::new(&root);
        let record = PatchAnalyzer::new(Some(store.clone()))
            .analyze_tasks(
                "新增激励广告复活",
                vec![PatchTask {
                    task_id: String::new(),
                    title: "Add revive UI".to_string(),
                    description: "Show rewarded ad revive popup.".to_string(),
                    affected_systems: vec!["UI".to_string(), "DeathSystem".to_string()],
                    expected_files: vec!["Assets/Scripts/UI/RevivePopup.cs".to_string()],
                    validation_route: Vec::new(),
                    requires_iteration: false,
                }],
                "Add rewarded revive patch.",
                true,
            )
            .unwrap();

        let saved = store.get(&record.patch_id).unwrap();

        assert_eq!(saved.status, PatchStatus::Analyzed);
        assert_eq!(saved.tasks[0].task_id, "PATCH-001");
        assert!(
            saved.tasks[0]
                .validation_route
                .contains(&"step13_scene_assembly".to_string())
        );
        assert_eq!(saved.analysis_summary, "Add rewarded revive patch.");
        cleanup(root);
    }

    #[test]
    fn light_validator_blocks_unbalanced_script_and_warns_missing_feature_flag() {
        let root = temp_root("patch_validator");
        let script = root.join("Assets/Scripts/RewardedAdRevive.cs");
        fs::create_dir_all(script.parent().unwrap()).unwrap();
        fs::write(
            &script,
            "public class RewardedAdRevive { void Run() { if (true) { }",
        )
        .unwrap();

        let report = LightValidator::new(&root)
            .validate_files(&["Assets/Scripts/RewardedAdRevive.cs".to_string()]);

        assert_eq!(report["status"], json!("blocked"));
        assert!(
            report["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == json!("UNBALANCED_BRACES"))
        );
        assert!(
            report["warnings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == json!("FEATURE_FLAG_NOT_DETECTED"))
        );
        cleanup(root);
    }

    #[test]
    fn patch_executor_apply_validate_and_promote_updates_store() {
        let root = temp_root("patch_executor");
        let store = PatchStore::new(root.join("patches"));
        let script = root.join("Assets/Scripts/Patch.cs");
        fs::create_dir_all(script.parent().unwrap()).unwrap();
        fs::write(&script, "public class Patch {}").unwrap();
        let record = store
            .write(&PatchRecord {
                patch_id: "patch-1".to_string(),
                request: "change".to_string(),
                status: PatchStatus::Analyzed,
                created_at: String::new(),
                updated_at: String::new(),
                tasks: vec![PatchTask {
                    task_id: "PATCH-001".to_string(),
                    title: "Patch".to_string(),
                    description: "Patch".to_string(),
                    affected_systems: Vec::new(),
                    expected_files: vec!["Assets/Scripts/Patch.cs".to_string()],
                    validation_route: Vec::new(),
                    requires_iteration: false,
                }],
                changed_files: Vec::new(),
                validation_summary: Value::Null,
                analysis_summary: String::new(),
                executor_result: Value::Null,
                promoted_iteration_spec: String::new(),
                errors: Vec::new(),
            })
            .unwrap();
        let executor = PatchExecutor::new(&root, store.clone());

        let applied = executor
            .apply(&record.patch_id, &StaticPatchRunner::success())
            .unwrap();
        let validated = executor.validate(&record.patch_id).unwrap();
        let promoted = executor
            .promote(&record.patch_id, "iteration_specs/v2.0_change.md")
            .unwrap();

        assert_eq!(applied.status, PatchStatus::Applied);
        assert_eq!(validated.status, PatchStatus::Validated);
        assert_eq!(validated.validation_summary["status"], json!("passed"));
        assert_eq!(promoted.status, PatchStatus::Promoted);
        assert_eq!(
            promoted.promoted_iteration_spec,
            "iteration_specs/v2.0_change.md"
        );
        cleanup(root);
    }

    #[test]
    fn codex_patch_runner_restricts_to_declared_expected_files() {
        let root = temp_root("patch_codex");
        let adapter = FakeAdapter;
        let runner = CodexPatchRunner::new(&root, adapter);
        let record = PatchRecord {
            patch_id: "patch-1".to_string(),
            request: "change".to_string(),
            status: PatchStatus::Analyzed,
            created_at: String::new(),
            updated_at: String::new(),
            tasks: vec![PatchTask {
                task_id: "PATCH-001".to_string(),
                title: "Patch".to_string(),
                description: String::new(),
                affected_systems: Vec::new(),
                expected_files: vec!["Assets/Scripts/Patch.cs".to_string()],
                validation_route: Vec::new(),
                requires_iteration: false,
            }],
            changed_files: Vec::new(),
            validation_summary: Value::Null,
            analysis_summary: String::new(),
            executor_result: Value::Null,
            promoted_iteration_spec: String::new(),
            errors: Vec::new(),
        };

        let task = runner.build_task(&record).unwrap();
        let result = runner.run(&record).unwrap();

        assert_eq!(task.allowed_write_paths, vec!["Assets/Scripts/Patch.cs"]);
        assert_eq!(result.status, "success");
        assert_eq!(result.changed_files, vec!["Assets/Scripts/Patch.cs"]);
        assert!(!result.stdout.contains("secret"));
        assert!(result.errors.is_empty());
        cleanup(root);
    }

    #[test]
    fn codex_patch_runner_refuses_unbounded_patch() {
        let root = temp_root("patch_unbounded");
        let runner = CodexPatchRunner::new(&root, FakeAdapter);
        let record = PatchRecord {
            patch_id: "patch-1".to_string(),
            request: "change".to_string(),
            status: PatchStatus::Analyzed,
            created_at: String::new(),
            updated_at: String::new(),
            tasks: Vec::new(),
            changed_files: Vec::new(),
            validation_summary: Value::Null,
            analysis_summary: String::new(),
            executor_result: Value::Null,
            promoted_iteration_spec: String::new(),
            errors: Vec::new(),
        };

        let result = runner.run(&record).unwrap();

        assert_eq!(result.status, "failed");
        assert!(result.errors[0].contains("refusing unrestricted execution"));
        cleanup(root);
    }

    #[test]
    fn codex_patch_runner_rejects_undeclared_staging_changes() {
        let root = temp_root("patch_rogue");
        let runner = CodexPatchRunner::new(&root, RogueAdapter);
        let record = record_with_expected_file("Assets/Scripts/Patch.cs");

        let result = runner.run(&record).unwrap();

        assert_eq!(result.status, "failed");
        assert!(result.errors[0].contains("undeclared file"));
        assert!(!root.join("Assets/Scripts/Patch.cs").exists());
        assert!(!root.join("rogue.txt").exists());
        cleanup(root);
    }

    #[test]
    fn codex_patch_runner_does_not_overwrite_a_concurrent_edit() {
        let root = temp_root("patch_cas");
        let target = root.join("Assets/Scripts/Patch.cs");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "original").unwrap();
        let runner = CodexPatchRunner::new(
            &root,
            ConcurrentEditAdapter {
                project_target: target.clone(),
            },
        );
        let record = record_with_expected_file("Assets/Scripts/Patch.cs");

        let result = runner.run(&record).unwrap();

        assert_eq!(result.status, "failed");
        assert!(result.errors[0].contains("changed while"));
        assert_eq!(fs::read_to_string(&target).unwrap(), "player-edit");
        cleanup(root);
    }

    #[test]
    fn patch_commit_rejects_staging_changes_after_verification() {
        let root = temp_root("patch_staged_cas");
        let target = root.join("Assets/Scripts/Patch.cs");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "project-original").unwrap();
        let isolated = IsolatedPatchRoot::new().unwrap();
        let staged_target = isolated.path().join("Assets/Scripts/Patch.cs");
        fs::create_dir_all(staged_target.parent().unwrap()).unwrap();
        fs::write(&staged_target, "verified").unwrap();
        let files = vec!["Assets/Scripts/Patch.cs".to_string()];
        let before = snapshot_patch_files(&root, &files).unwrap();
        let staged = patch_directory_manifest(isolated.path()).unwrap();
        fs::write(&staged_target, "tampered").unwrap();

        let error = commit_patch_outputs(&root, &isolated, &files, &before, &staged).unwrap_err();

        assert!(error.message().contains("after isolated verification"));
        assert_eq!(fs::read_to_string(&target).unwrap(), "project-original");
        cleanup(root);
    }

    #[test]
    fn patch_manifest_rejects_excessive_directory_depth() {
        let isolated = IsolatedPatchRoot::new().unwrap();
        let mut directory = isolated.path().to_path_buf();
        for index in 0..34 {
            directory.push(format!("d{index}"));
            fs::create_dir(&directory).unwrap();
        }
        fs::write(directory.join("out.txt"), b"deep").unwrap();

        let error = patch_directory_manifest(isolated.path()).unwrap_err();

        assert!(error.message().contains("depth limit"));
    }

    #[test]
    fn patch_store_reports_corrupt_manifest_instead_of_treating_it_as_missing() {
        let root = temp_root("patch_corrupt_manifest");
        let store = PatchStore::new(root.join("patches"));
        let path = store.manifest_path("patch-bad");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"{not-json").unwrap();

        assert!(store.read("patch-bad").is_err());
        assert!(store.list().is_err());
        cleanup(root);
    }

    #[test]
    fn patch_apply_intent_blocks_reexecution_after_project_drift() {
        let root = temp_root("patch_apply_recovery");
        let project = root.join("project");
        let target = project.join("Assets/Scripts/Patch.cs");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "before").unwrap();
        let store = PatchStore::new(root.join("patches"));
        let mut record = record_with_expected_file("Assets/Scripts/Patch.cs");
        let files = declared_expected_files(&record);
        let before = snapshot_patch_files(&project, &files).unwrap();
        record.executor_result = json!({
            "status": "applying",
            "attempt_id": "interrupted-attempt",
            "expected_files": files,
            "before_hashes": before,
        });
        store.write(&record).unwrap();
        fs::write(&target, "already-changed").unwrap();
        let executor = PatchExecutor::new(&project, store.clone());

        let error = executor
            .apply(&record.patch_id, &PanicPatchRunner)
            .unwrap_err();

        assert!(error.message().contains("manual reconciliation"));
        assert_eq!(
            store.get(&record.patch_id).unwrap().executor_result["status"],
            "applying"
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "already-changed");
        cleanup(root);
    }

    #[test]
    fn patch_store_lock_rejects_a_second_apply_owner() {
        let root = temp_root("patch_apply_lock");
        let store = PatchStore::new(root.join("patches"));
        let first = store.lock_patch("patch-1").unwrap();
        assert!(store.lock_patch("patch-1").is_err());
        drop(first);
        assert!(store.lock_patch("patch-1").is_ok());
        cleanup(root);
    }

    #[test]
    fn patch_service_rejects_empty_request_and_lists_by_updated_at_desc() {
        let mut service = PatchService::new();
        assert!(service.analyze_request_shell("", Vec::new()).is_err());
        let older = PatchRecord {
            patch_id: "patch-old".to_string(),
            request: "old".to_string(),
            status: PatchStatus::Analyzed,
            created_at: "unix:1".to_string(),
            updated_at: "unix:1".to_string(),
            tasks: Vec::new(),
            changed_files: Vec::new(),
            validation_summary: Value::Null,
            analysis_summary: String::new(),
            executor_result: Value::Null,
            promoted_iteration_spec: String::new(),
            errors: Vec::new(),
        };
        let newer = PatchRecord {
            patch_id: "patch-new".to_string(),
            request: "new".to_string(),
            updated_at: "unix:2".to_string(),
            ..older.clone()
        };
        service.write(older);
        service.write(newer);
        assert_eq!(service.list()[0].patch_id, "patch-new");
    }

    #[test]
    fn patch_service_status_filter_and_approved_context() {
        let mut service = PatchService::new();
        let record = service
            .analyze_request_shell(
                "Add save dialog",
                vec![PatchTask {
                    task_id: "task-1".to_string(),
                    title: "Wire command".to_string(),
                    description: String::new(),
                    affected_systems: vec!["save".to_string()],
                    expected_files: Vec::new(),
                    validation_route: Vec::new(),
                    requires_iteration: false,
                }],
            )
            .unwrap();
        assert_eq!(service.filter_by_status(PatchStatus::Analyzed).len(), 1);
        service
            .set_status(&record.patch_id, PatchStatus::Validated)
            .unwrap();
        assert!(service.filter_by_status(PatchStatus::Analyzed).is_empty());
        assert_eq!(service.approved_context().len(), 1);
    }

    #[derive(Debug, Clone)]
    struct StaticPatchRunner {
        result: PatchRunResult,
    }

    struct PanicPatchRunner;

    impl PatchRunner for PanicPatchRunner {
        fn run(&self, _: &PatchRecord) -> AdmResult<PatchRunResult> {
            panic!("runner must not execute while an applying intent has unresolved drift")
        }
    }

    impl StaticPatchRunner {
        fn success() -> Self {
            Self {
                result: PatchRunResult {
                    status: "success".to_string(),
                    changed_files: vec!["Assets/Scripts/Patch.cs".to_string()],
                    stdout: "ok".to_string(),
                    errors: Vec::new(),
                },
            }
        }
    }

    impl PatchRunner for StaticPatchRunner {
        fn run(&self, _: &PatchRecord) -> AdmResult<PatchRunResult> {
            Ok(self.result.clone())
        }
    }

    #[derive(Debug, Clone)]
    struct FakeAdapter;

    impl CompletionAdapter for FakeAdapter {
        fn generate(&self, task: &ModelTask) -> AdmResult<ModelResult> {
            let target = PathBuf::from(&task.cwd).join(&task.output_files[0]);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, "patched").unwrap();
            Ok(ModelResult {
                task_id: task.task_id.clone(),
                status: ModelResultStatus::Succeeded,
                text: "private C:\\secret sk-provider-secret".to_string(),
                errors: vec!["private provider diagnostic".to_string()],
            })
        }
    }

    #[derive(Debug, Clone)]
    struct RogueAdapter;

    impl CompletionAdapter for RogueAdapter {
        fn generate(&self, task: &ModelTask) -> AdmResult<ModelResult> {
            let root = PathBuf::from(&task.cwd);
            let target = root.join(&task.output_files[0]);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, "patched").unwrap();
            fs::write(root.join("rogue.txt"), "rogue").unwrap();
            Ok(ModelResult {
                task_id: task.task_id.clone(),
                status: ModelResultStatus::Succeeded,
                text: String::new(),
                errors: Vec::new(),
            })
        }
    }

    #[derive(Debug, Clone)]
    struct ConcurrentEditAdapter {
        project_target: PathBuf,
    }

    impl CompletionAdapter for ConcurrentEditAdapter {
        fn generate(&self, task: &ModelTask) -> AdmResult<ModelResult> {
            let target = PathBuf::from(&task.cwd).join(&task.output_files[0]);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, "generated").unwrap();
            fs::write(&self.project_target, "player-edit").unwrap();
            Ok(ModelResult {
                task_id: task.task_id.clone(),
                status: ModelResultStatus::Succeeded,
                text: String::new(),
                errors: Vec::new(),
            })
        }
    }

    fn record_with_expected_file(path: &str) -> PatchRecord {
        PatchRecord {
            patch_id: "patch-1".to_string(),
            request: "change".to_string(),
            status: PatchStatus::Analyzed,
            created_at: String::new(),
            updated_at: String::new(),
            tasks: vec![PatchTask {
                task_id: "PATCH-001".to_string(),
                title: "Patch".to_string(),
                description: String::new(),
                affected_systems: Vec::new(),
                expected_files: vec![path.to_string()],
                validation_route: Vec::new(),
                requires_iteration: false,
            }],
            changed_files: Vec::new(),
            validation_summary: Value::Null,
            analysis_summary: String::new(),
            executor_result: Value::Null,
            promoted_iteration_spec: String::new(),
            errors: Vec::new(),
        }
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(new_patch_id().unwrap().replace("patch", prefix));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
