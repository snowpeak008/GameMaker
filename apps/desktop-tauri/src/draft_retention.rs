use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use adm_new_contracts::pipeline::PipelineRunState;
use adm_new_contracts::project::ProjectState;
use adm_new_contracts::save::{DraftMeta, SAVE_SCHEMA_VERSION, WorkspaceState};
use fs2::FileExt;

pub const PRUNE_BLANK_DRAFTS_KEEP_COUNT_ENV: &str = "ADM_NEWRUST_PRUNE_BLANK_DRAFTS_KEEP_COUNT";

const LOCK_DIR: &str = ".session_locks";
const KNOWN_DIRECTORIES: [&str; 3] = ["outputs", "outputs/run_logs", "outputs/runtime_control"];
const KNOWN_FILES: [&str; 5] = [
    "autosave_state.json",
    "draft_meta.json",
    "outputs/pipeline_state.json",
    "outputs/run_logs/desktop.jsonl",
    "outputs/runtime_control/pipeline_state.json",
];
const DRAFT_META_KEYS: [&str; 10] = [
    "schema_version",
    "session_id",
    "pid",
    "project_root",
    "draft_root",
    "updated_at",
    "linked_save_id",
    "linked_archive_path",
    "workspace_state",
    "origin_deleted_save_id",
];

#[derive(Debug, Default)]
pub struct DraftRetentionOutcome {
    pub keep_count: u32,
    pub removed_count: usize,
    pub warnings: Vec<String>,
}

struct BlankDraftCandidate {
    session_id: String,
    draft_path: PathBuf,
    lock_path: PathBuf,
    lock_file: File,
    modified_at: u64,
}

pub fn configured_keep_count() -> u32 {
    parse_keep_count(
        std::env::var(PRUNE_BLANK_DRAFTS_KEEP_COUNT_ENV)
            .ok()
            .as_deref(),
    )
}

pub fn prune_configured_blank_drafts(
    data_root: &Path,
    current_session_id: &str,
    empty_project_state: &ProjectState,
    empty_pipeline_state: &PipelineRunState,
) -> DraftRetentionOutcome {
    prune_blank_drafts(
        data_root,
        current_session_id,
        empty_project_state,
        empty_pipeline_state,
        configured_keep_count(),
    )
}

fn parse_keep_count(value: Option<&str>) -> u32 {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0)
}

fn prune_blank_drafts(
    data_root: &Path,
    current_session_id: &str,
    empty_project_state: &ProjectState,
    empty_pipeline_state: &PipelineRunState,
    keep_count: u32,
) -> DraftRetentionOutcome {
    let mut outcome = DraftRetentionOutcome {
        keep_count,
        ..DraftRetentionOutcome::default()
    };
    if keep_count == 0 {
        return outcome;
    }

    let drafts_root = data_root.join("drafts");
    let lock_root = drafts_root.join(LOCK_DIR);
    if path_is_symlink(&drafts_root) || path_is_symlink(&lock_root) {
        outcome
            .warnings
            .push("draft retention skipped because a managed root is a symbolic link".to_string());
        return outcome;
    }
    if let Err(error) = fs::create_dir_all(&lock_root) {
        outcome.warnings.push(format!(
            "draft retention could not prepare session locks: {error}"
        ));
        return outcome;
    }

    let expected_project = match serde_json::to_value(empty_project_state) {
        Ok(value) => value,
        Err(error) => {
            outcome.warnings.push(format!(
                "draft retention could not encode the blank project: {error}"
            ));
            return outcome;
        }
    };
    let expected_pipeline = match serde_json::to_value(empty_pipeline_state) {
        Ok(value) => value,
        Err(error) => {
            outcome.warnings.push(format!(
                "draft retention could not encode the idle pipeline: {error}"
            ));
            return outcome;
        }
    };

    let entries = match fs::read_dir(&drafts_root) {
        Ok(entries) => entries,
        Err(error) => {
            outcome
                .warnings
                .push(format!("draft retention could not inspect drafts: {error}"));
            return outcome;
        }
    };
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                outcome.warnings.push(format!(
                    "draft retention skipped an unreadable entry: {error}"
                ));
                continue;
            }
        };
        let Some(session_id) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if session_id == current_session_id || parse_desktop_session_id(&session_id).is_none() {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        match inspect_candidate(
            &drafts_root,
            &lock_root,
            &session_id,
            &expected_project,
            &expected_pipeline,
        ) {
            Ok(Some(candidate)) => candidates.push(candidate),
            Ok(None) => {}
            Err(error) => outcome.warnings.push(error),
        }
    }

    candidates.sort_by(|left, right| {
        right
            .modified_at
            .cmp(&left.modified_at)
            .then_with(|| left.session_id.cmp(&right.session_id))
    });
    for (index, candidate) in candidates.into_iter().enumerate() {
        if index < keep_count as usize {
            release_candidate(candidate, false);
            continue;
        }
        match fs::remove_dir_all(&candidate.draft_path) {
            Ok(()) => {
                outcome.removed_count += 1;
                release_candidate(candidate, true);
            }
            Err(error) => {
                outcome.warnings.push(format!(
                    "draft retention could not remove eligible session {}: {error}",
                    candidate.session_id
                ));
                release_candidate(candidate, false);
            }
        }
    }
    outcome
}

fn inspect_candidate(
    drafts_root: &Path,
    lock_root: &Path,
    session_id: &str,
    expected_project: &serde_json::Value,
    expected_pipeline: &serde_json::Value,
) -> Result<Option<BlankDraftCandidate>, String> {
    let draft_path = drafts_root.join(session_id);
    if draft_path.parent() != Some(drafts_root) || path_is_symlink(&draft_path) {
        return Ok(None);
    }
    let lock_path = lock_root.join(format!("{session_id}.lock"));
    if path_is_symlink(&lock_path) {
        return Ok(None);
    }
    let lock_existed = lock_path.exists();
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| format!("draft retention could not open a session lock: {error}"))?;
    match lock_file.try_lock_exclusive() {
        Ok(()) => {}
        Err(error) if is_lock_contention(&error) => return Ok(None),
        Err(error) => {
            return Err(format!(
                "draft retention could not verify a session lock: {error}"
            ));
        }
    }

    let eligible = has_known_layout(&draft_path)
        && has_blank_draft_meta(&draft_path, session_id)
        && json_file_equals(&draft_path.join("autosave_state.json"), expected_project)
        && json_file_equals(
            &draft_path.join("outputs/pipeline_state.json"),
            expected_pipeline,
        )
        && json_file_equals(
            &draft_path.join("outputs/runtime_control/pipeline_state.json"),
            expected_pipeline,
        );
    if !eligible {
        let _ = lock_file.unlock();
        drop(lock_file);
        if !lock_existed {
            let _ = fs::remove_file(lock_path);
        }
        return Ok(None);
    }

    Ok(Some(BlankDraftCandidate {
        session_id: session_id.to_string(),
        modified_at: draft_modified_at(&draft_path),
        draft_path,
        lock_path,
        lock_file,
    }))
}

fn has_known_layout(draft_path: &Path) -> bool {
    let mut directories = BTreeSet::new();
    let mut files = BTreeSet::new();
    if collect_layout(draft_path, draft_path, &mut directories, &mut files).is_err() {
        return false;
    }
    directories == KNOWN_DIRECTORIES.into_iter().map(str::to_string).collect()
        && files == KNOWN_FILES.into_iter().map(str::to_string).collect()
}

fn collect_layout(
    root: &Path,
    directory: &Path,
    directories: &mut BTreeSet<String>,
    files: &mut BTreeSet<String>,
) -> Result<(), ()> {
    for entry in fs::read_dir(directory).map_err(|_| ())? {
        let entry = entry.map_err(|_| ())?;
        let file_type = entry.file_type().map_err(|_| ())?;
        if file_type.is_symlink() {
            return Err(());
        }
        let relative = normalized_relative_path(root, &entry.path()).ok_or(())?;
        if file_type.is_dir() {
            directories.insert(relative);
            collect_layout(root, &entry.path(), directories, files)?;
        } else if file_type.is_file() {
            files.insert(relative);
        } else {
            return Err(());
        }
    }
    Ok(())
}

fn normalized_relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let parts = relative
        .components()
        .map(|component| component.as_os_str().to_str().map(str::to_string))
        .collect::<Option<Vec<_>>>()?;
    Some(parts.join("/"))
}

fn has_blank_draft_meta(draft_path: &Path, session_id: &str) -> bool {
    let value = match read_json_value(&draft_path.join("draft_meta.json")) {
        Some(value) => value,
        None => return false,
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    let keys = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if keys != DRAFT_META_KEYS.into_iter().collect() {
        return false;
    }
    let meta = match serde_json::from_value::<DraftMeta>(value) {
        Ok(meta) => meta,
        Err(_) => return false,
    };
    let Some((pid, _, _)) = parse_desktop_session_id(session_id) else {
        return false;
    };
    meta.schema_version == SAVE_SCHEMA_VERSION
        && meta.session_id == session_id
        && meta.pid == pid
        && meta.linked_save_id.is_none()
        && meta.linked_archive_path.is_empty()
        && meta.workspace_state == WorkspaceState::Unsaved
        && meta.origin_deleted_save_id.is_none()
}

fn parse_desktop_session_id(session_id: &str) -> Option<(u32, u64, u32)> {
    let mut parts = session_id.split('_');
    if parts.next()? != "desktop" {
        return None;
    }
    let pid = parts.next()?.parse().ok()?;
    let timestamp = parts.next()?.parse().ok()?;
    let attempt = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((pid, timestamp, attempt))
}

fn json_file_equals(path: &Path, expected: &serde_json::Value) -> bool {
    read_json_value(path).is_some_and(|value| value == *expected)
}

fn read_json_value(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn draft_modified_at(path: &Path) -> u64 {
    KNOWN_FILES
        .into_iter()
        .map(|relative| path.join(relative))
        .chain(std::iter::once(path.to_path_buf()))
        .filter_map(|candidate| fs::metadata(candidate).ok()?.modified().ok())
        .filter_map(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .max()
        .unwrap_or(0)
}

fn path_is_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn is_lock_contention(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::WouldBlock
        || matches!(error.raw_os_error(), Some(11 | 32 | 33 | 36))
}

fn release_candidate(candidate: BlankDraftCandidate, remove_lock: bool) {
    let _ = candidate.lock_file.unlock();
    drop(candidate.lock_file);
    if remove_lock {
        let _ = fs::remove_file(candidate.lock_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "adm-newrust-draft-retention-{label}-{}",
            adm_new_foundation::new_stable_id("test").unwrap()
        ))
    }

    fn write_blank_draft(
        root: &Path,
        session_id: &str,
        project: &ProjectState,
        pipeline: &PipelineRunState,
    ) {
        let draft = root.join("drafts").join(session_id);
        fs::create_dir_all(draft.join("outputs/runtime_control")).unwrap();
        fs::create_dir_all(draft.join("outputs/run_logs")).unwrap();
        let (pid, _, _) = parse_desktop_session_id(session_id).unwrap();
        let meta = DraftMeta {
            schema_version: SAVE_SCHEMA_VERSION,
            session_id: session_id.to_string(),
            pid,
            project_root: ".".to_string(),
            draft_root: format!("drafts/{session_id}"),
            updated_at: "unix:1".to_string(),
            linked_save_id: None,
            linked_archive_path: String::new(),
            workspace_state: WorkspaceState::Unsaved,
            origin_deleted_save_id: None,
        };
        fs::write(
            draft.join("draft_meta.json"),
            serde_json::to_vec_pretty(&meta).unwrap(),
        )
        .unwrap();
        fs::write(
            draft.join("autosave_state.json"),
            serde_json::to_vec_pretty(project).unwrap(),
        )
        .unwrap();
        let pipeline_json = serde_json::to_vec_pretty(pipeline).unwrap();
        fs::write(draft.join("outputs/pipeline_state.json"), &pipeline_json).unwrap();
        fs::write(
            draft.join("outputs/runtime_control/pipeline_state.json"),
            &pipeline_json,
        )
        .unwrap();
        File::create(draft.join("outputs/run_logs/desktop.jsonl"))
            .unwrap()
            .write_all(b"{}\n")
            .unwrap();
    }

    #[test]
    fn keep_count_is_disabled_for_missing_invalid_or_zero_values() {
        assert_eq!(parse_keep_count(None), 0);
        assert_eq!(parse_keep_count(Some("")), 0);
        assert_eq!(parse_keep_count(Some("invalid")), 0);
        assert_eq!(parse_keep_count(Some("0")), 0);
        assert_eq!(parse_keep_count(Some(" 3 ")), 3);
    }

    #[test]
    fn disabled_retention_does_not_touch_blank_drafts() {
        let root = test_root("disabled");
        let project = ProjectState::empty();
        let pipeline = PipelineRunState::default();
        write_blank_draft(&root, "desktop_100_1000_0", &project, &pipeline);

        let outcome = prune_blank_drafts(&root, "desktop_200_2000_0", &project, &pipeline, 0);

        assert_eq!(outcome.removed_count, 0);
        assert!(root.join("drafts/desktop_100_1000_0").is_dir());
        assert!(!root.join("drafts/.session_locks").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn retention_only_removes_excess_proven_blank_unlocked_drafts() {
        let root = test_root("eligible");
        let project = ProjectState::empty();
        let pipeline = PipelineRunState::default();
        for session in [
            "desktop_101_1001_0",
            "desktop_102_1002_0",
            "desktop_103_1003_0",
        ] {
            write_blank_draft(&root, session, &project, &pipeline);
        }

        let outcome = prune_blank_drafts(&root, "desktop_999_9999_0", &project, &pipeline, 1);
        let remaining = fs::read_dir(root.join("drafts"))
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("desktop_"))
            .count();

        assert_eq!(outcome.removed_count, 2);
        assert_eq!(remaining, 1);
        assert!(outcome.warnings.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn nonblank_unknown_linked_and_locked_drafts_are_preserved() {
        let root = test_root("protected");
        let project = ProjectState::empty();
        let pipeline = PipelineRunState::default();
        write_blank_draft(&root, "desktop_201_2001_0", &project, &pipeline);
        write_blank_draft(&root, "desktop_202_2002_0", &project, &pipeline);

        let mut changed = project.clone();
        changed.project_name = "Changed".to_string();
        write_blank_draft(&root, "desktop_203_2003_0", &changed, &pipeline);

        write_blank_draft(&root, "desktop_204_2004_0", &project, &pipeline);
        fs::write(root.join("drafts/desktop_204_2004_0/unknown.json"), b"{}").unwrap();

        write_blank_draft(&root, "desktop_205_2005_0", &project, &pipeline);
        let linked_path = root.join("drafts/desktop_205_2005_0/draft_meta.json");
        let mut linked: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&linked_path).unwrap()).unwrap();
        linked["linked_save_id"] = serde_json::json!("save_1");
        linked["workspace_state"] = serde_json::json!("linked_save");
        fs::write(linked_path, serde_json::to_vec_pretty(&linked).unwrap()).unwrap();

        write_blank_draft(&root, "desktop_206_2006_0", &project, &pipeline);
        let lock_root = root.join("drafts/.session_locks");
        fs::create_dir_all(&lock_root).unwrap();
        let lock_path = lock_root.join("desktop_206_2006_0.lock");
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .unwrap();
        lock_file.lock_exclusive().unwrap();

        let mut running_pipeline = pipeline.clone();
        running_pipeline.run_id = "run_1".to_string();
        running_pipeline.status = "running".to_string();
        write_blank_draft(&root, "desktop_207_2007_0", &project, &running_pipeline);

        write_blank_draft(&root, "desktop_208_2008_0", &project, &pipeline);
        fs::write(
            root.join("drafts/desktop_208_2008_0/autosave_state.json"),
            b"{invalid-json",
        )
        .unwrap();

        let outcome = prune_blank_drafts(&root, "desktop_999_9999_0", &project, &pipeline, 1);

        assert_eq!(outcome.removed_count, 1);
        for protected in [
            "desktop_203_2003_0",
            "desktop_204_2004_0",
            "desktop_205_2005_0",
            "desktop_206_2006_0",
            "desktop_207_2007_0",
            "desktop_208_2008_0",
        ] {
            assert!(root.join("drafts").join(protected).is_dir(), "{protected}");
        }
        lock_file.unlock().unwrap();
        drop(lock_file);
        let _ = fs::remove_dir_all(root);
    }
}
