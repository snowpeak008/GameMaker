use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectEnvironmentDiagnostic {
    pub severity: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UnityProjectVersion {
    pub version: String,
    pub revision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectEnvironmentInspection {
    pub status: String,
    pub project_path: String,
    pub expected_engine: String,
    pub detected_engine: String,
    #[serde(default)]
    pub markers: Vec<String>,
    #[serde(default)]
    pub unity_version: Option<UnityProjectVersion>,
    #[serde(default)]
    pub diagnostics: Vec<ProjectEnvironmentDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnityEditorCandidate {
    pub path: String,
    pub source: String,
    pub version: String,
    pub present: bool,
    pub valid_executable: bool,
    pub configured: bool,
    pub match_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorSelectionValidation {
    pub valid: bool,
    pub engine: String,
    pub path: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub match_kind: String,
    #[serde(default)]
    pub error_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectDetection {
    pub engine: String,
    pub markers: Vec<String>,
    pub unity_version: Option<UnityProjectVersion>,
}

/// Detects one engine's project markers without owning any UI or persistence policy.
pub trait ProjectDetector {
    fn engine_id(&self) -> &'static str;
    fn detect(&self, project_path: &Path) -> Option<ProjectDetection>;
}

/// Locates and validates editor programs for one engine.
pub trait EditorLocator {
    fn engine_id(&self) -> &'static str;
    fn discover(
        &self,
        project_path: &Path,
        configured_editor_path: Option<&str>,
    ) -> Vec<UnityEditorCandidate>;
    fn validate(&self, project_path: &Path, editor_path: &Path) -> EditorSelectionValidation;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UnityProjectDetector;

#[derive(Debug, Clone, Copy, Default)]
pub struct GodotProjectDetector;

#[derive(Debug, Clone, Copy, Default)]
pub struct UnrealProjectDetector;

#[derive(Debug, Clone, Copy, Default)]
pub struct UnityEditorLocator;

impl ProjectDetector for UnityProjectDetector {
    fn engine_id(&self) -> &'static str {
        "unity"
    }

    fn detect(&self, project_path: &Path) -> Option<ProjectDetection> {
        let marker_checks = unity_marker_checks(project_path);
        let markers = marker_checks
            .iter()
            .filter(|(_, present)| *present)
            .map(|(name, _)| (*name).to_string())
            .collect::<Vec<_>>();
        (markers.len() >= 2).then(|| ProjectDetection {
            engine: self.engine_id().to_string(),
            markers,
            unity_version: read_unity_project_version(project_path),
        })
    }
}

impl ProjectDetector for GodotProjectDetector {
    fn engine_id(&self) -> &'static str {
        "godot"
    }

    fn detect(&self, project_path: &Path) -> Option<ProjectDetection> {
        project_path
            .join("project.godot")
            .is_file()
            .then(|| ProjectDetection {
                engine: self.engine_id().to_string(),
                markers: vec!["project.godot".to_string()],
                unity_version: None,
            })
    }
}

impl ProjectDetector for UnrealProjectDetector {
    fn engine_id(&self) -> &'static str {
        "unreal"
    }

    fn detect(&self, project_path: &Path) -> Option<ProjectDetection> {
        root_contains_extension(project_path, "uproject").then(|| ProjectDetection {
            engine: self.engine_id().to_string(),
            markers: vec!["*.uproject".to_string()],
            unity_version: None,
        })
    }
}

impl EditorLocator for UnityEditorLocator {
    fn engine_id(&self) -> &'static str {
        "unity"
    }

    fn discover(
        &self,
        project_path: &Path,
        configured_editor_path: Option<&str>,
    ) -> Vec<UnityEditorCandidate> {
        discover_unity_editor_candidates(project_path, configured_editor_path)
    }

    fn validate(&self, project_path: &Path, editor_path: &Path) -> EditorSelectionValidation {
        let required_version = read_unity_project_version(project_path)
            .map(|item| item.version)
            .unwrap_or_default();
        let candidate = candidate_from_path(
            editor_path.to_path_buf(),
            "selected",
            true,
            &required_version,
        );
        let valid =
            candidate.present && candidate.valid_executable && candidate.match_kind != "mismatch";
        let error_code = if !candidate.present {
            "editor_path_not_found"
        } else if !candidate.valid_executable {
            "invalid_unity_editor_executable"
        } else if candidate.match_kind == "mismatch" {
            "unity_editor_version_conflict"
        } else {
            ""
        };
        EditorSelectionValidation {
            valid,
            engine: self.engine_id().to_string(),
            path: candidate.path,
            version: candidate.version,
            match_kind: candidate.match_kind,
            error_code: error_code.to_string(),
        }
    }
}

pub fn inspect_project_directory(
    project_path: impl AsRef<Path>,
    expected_engine: &str,
) -> ProjectEnvironmentInspection {
    let project_path = project_path.as_ref();
    let expected_engine = expected_engine.trim().to_ascii_lowercase();
    let mut diagnostics = Vec::new();
    if !project_path.is_dir() {
        diagnostics.push(diagnostic(
            "blocker",
            "project_directory_missing",
            "The selected project directory does not exist.",
        ));
        return ProjectEnvironmentInspection {
            status: "invalid".to_string(),
            project_path: display_path(project_path),
            expected_engine,
            detected_engine: String::new(),
            markers: Vec::new(),
            unity_version: None,
            diagnostics,
        };
    }

    let marker_checks = unity_marker_checks(project_path);
    let unity_marker_count = marker_checks.iter().filter(|(_, present)| *present).count();
    let detectors: [&dyn ProjectDetector; 3] = [
        &UnityProjectDetector,
        &GodotProjectDetector,
        &UnrealProjectDetector,
    ];
    let detection = detectors
        .iter()
        .find_map(|detector| detector.detect(project_path));
    let detected_engine = detection
        .as_ref()
        .map(|item| item.engine.clone())
        .unwrap_or_default();
    let markers = detection
        .as_ref()
        .map(|item| item.markers.clone())
        .unwrap_or_default();
    let unity_version = detection.and_then(|item| item.unity_version);
    if detected_engine.is_empty() {
        diagnostics.push(diagnostic(
            "blocker",
            "engine_not_detected",
            "No supported project marker was detected in the selected directory.",
        ));
    } else if !expected_engine.is_empty()
        && expected_engine != "custom"
        && expected_engine != detected_engine
    {
        diagnostics.push(diagnostic(
            "warning",
            "engine_selection_conflict",
            "The detected engine differs from the configured engine.",
        ));
    }
    if detected_engine == "unity" && unity_marker_count < marker_checks.len() {
        diagnostics.push(diagnostic(
            "warning",
            "unity_project_partial",
            "The directory has Unity markers but some standard project files are missing.",
        ));
    }
    if detected_engine == "unity" && unity_version.is_none() {
        diagnostics.push(diagnostic(
            "warning",
            "unity_version_unknown",
            "ProjectSettings/ProjectVersion.txt did not contain a Unity editor version.",
        ));
    }
    let status = if detected_engine.is_empty() {
        "invalid"
    } else if diagnostics.iter().any(|item| item.severity == "warning") {
        "warning"
    } else {
        "valid"
    };
    ProjectEnvironmentInspection {
        status: status.to_string(),
        project_path: display_path(project_path),
        expected_engine,
        detected_engine,
        markers,
        unity_version,
        diagnostics,
    }
}

pub fn read_unity_project_version(project_path: impl AsRef<Path>) -> Option<UnityProjectVersion> {
    let text = fs::read_to_string(
        project_path
            .as_ref()
            .join("ProjectSettings/ProjectVersion.txt"),
    )
    .ok()?;
    parse_unity_project_version(&text)
}

pub fn parse_unity_project_version(text: &str) -> Option<UnityProjectVersion> {
    let mut version = String::new();
    let mut revision = String::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        match key.trim() {
            "m_EditorVersion" => version = value.trim().to_string(),
            "m_EditorVersionWithRevision" => revision = value.trim().to_string(),
            _ => {}
        }
    }
    (!version.is_empty()).then_some(UnityProjectVersion { version, revision })
}

pub fn discover_unity_editors(
    project_path: impl AsRef<Path>,
    configured_editor_path: Option<&str>,
) -> Vec<UnityEditorCandidate> {
    UnityEditorLocator.discover(project_path.as_ref(), configured_editor_path)
}

fn discover_unity_editor_candidates(
    project_path: &Path,
    configured_editor_path: Option<&str>,
) -> Vec<UnityEditorCandidate> {
    let required_version = read_unity_project_version(project_path)
        .map(|item| item.version)
        .unwrap_or_default();
    let mut candidates = Vec::new();
    if let Some(path) = configured_editor_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        candidates.push(candidate_from_path(
            PathBuf::from(path),
            "configured",
            true,
            &required_version,
        ));
    }
    for variable in ["ADM_UNITY_EDITOR", "UNITY_EDITOR_PATH"] {
        if let Some(path) = env::var_os(variable).filter(|value| !value.is_empty()) {
            candidates.push(candidate_from_path(
                PathBuf::from(path),
                variable,
                false,
                &required_version,
            ));
        }
    }
    for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
        let Some(program_files) = env::var_os(variable).map(PathBuf::from) else {
            continue;
        };
        let hub_root = program_files.join("Unity/Hub/Editor");
        if let Ok(entries) = fs::read_dir(&hub_root) {
            for entry in entries.flatten().take(256) {
                let version = entry.file_name().to_string_lossy().to_string();
                let path = entry.path().join("Editor/Unity.exe");
                if path.is_file() {
                    candidates.push(candidate_with_version(
                        path,
                        "unity_hub",
                        version,
                        false,
                        &required_version,
                    ));
                }
            }
        }
        let legacy = program_files.join("Unity/Editor/Unity.exe");
        if legacy.is_file() {
            candidates.push(candidate_from_path(
                legacy,
                "unity_legacy",
                false,
                &required_version,
            ));
        }
    }
    rank_unity_editor_candidates(&required_version, candidates)
}

pub fn validate_editor_selection(
    project_engine: &str,
    project_path: impl AsRef<Path>,
    editor_path: impl AsRef<Path>,
) -> EditorSelectionValidation {
    let project_engine = project_engine.trim().to_ascii_lowercase();
    let project_path = project_path.as_ref();
    let editor_path = editor_path.as_ref();
    if project_engine == "unity" {
        return UnityEditorLocator.validate(project_path, editor_path);
    }
    let present = editor_path.is_file();
    EditorSelectionValidation {
        valid: present,
        engine: project_engine,
        path: display_path(editor_path),
        version: String::new(),
        match_kind: "unknown".to_string(),
        error_code: if present {
            String::new()
        } else {
            "editor_path_not_found".to_string()
        },
    }
}

pub fn rank_unity_editor_candidates(
    required_version: &str,
    candidates: Vec<UnityEditorCandidate>,
) -> Vec<UnityEditorCandidate> {
    let mut deduplicated = BTreeMap::<String, UnityEditorCandidate>::new();
    for mut candidate in candidates {
        candidate.match_kind = editor_match_kind(required_version, &candidate.version);
        let key = normalized_path_key(Path::new(&candidate.path));
        match deduplicated.get(&key) {
            Some(existing) if candidate_rank(existing) <= candidate_rank(&candidate) => {}
            _ => {
                deduplicated.insert(key, candidate);
            }
        }
    }
    let mut candidates = deduplicated.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        candidate_rank(left)
            .cmp(&candidate_rank(right))
            .then_with(|| left.version.cmp(&right.version).reverse())
            .then_with(|| left.path.cmp(&right.path))
    });
    candidates
}

pub fn unity_editor_file_is_valid(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    path.is_file()
        && path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("Unity.exe"))
}

fn candidate_from_path(
    path: PathBuf,
    source: &str,
    configured: bool,
    required_version: &str,
) -> UnityEditorCandidate {
    let version = unity_version_from_editor_path(&path).unwrap_or_default();
    candidate_with_version(path, source, version, configured, required_version)
}

fn candidate_with_version(
    path: PathBuf,
    source: &str,
    version: String,
    configured: bool,
    required_version: &str,
) -> UnityEditorCandidate {
    let present = path.is_file();
    let valid_executable = unity_editor_file_is_valid(&path);
    UnityEditorCandidate {
        path: display_path(&path),
        source: source.to_string(),
        match_kind: editor_match_kind(required_version, &version),
        version,
        present,
        valid_executable,
        configured,
    }
}

fn unity_version_from_editor_path(path: &Path) -> Option<String> {
    let editor_dir = path.parent()?;
    if !editor_dir
        .file_name()?
        .to_string_lossy()
        .eq_ignore_ascii_case("Editor")
    {
        return None;
    }
    let version_dir = editor_dir.parent()?;
    let value = version_dir.file_name()?.to_string_lossy().to_string();
    looks_like_unity_version(&value).then_some(value)
}

fn editor_match_kind(required: &str, candidate: &str) -> String {
    if required.is_empty() || candidate.is_empty() {
        "unknown".to_string()
    } else if required.eq_ignore_ascii_case(candidate) {
        "exact".to_string()
    } else if unity_compatibility_key(required) == unity_compatibility_key(candidate) {
        "compatible".to_string()
    } else {
        "mismatch".to_string()
    }
}

fn unity_compatibility_key(version: &str) -> String {
    version
        .split('.')
        .take(2)
        .collect::<Vec<_>>()
        .join(".")
        .to_ascii_lowercase()
}

fn looks_like_unity_version(value: &str) -> bool {
    let mut parts = value.split('.');
    parts
        .next()
        .is_some_and(|part| part.len() == 4 && part.chars().all(|ch| ch.is_ascii_digit()))
        && parts
            .next()
            .is_some_and(|part| part.chars().all(|ch| ch.is_ascii_digit()))
}

fn candidate_rank(candidate: &UnityEditorCandidate) -> u8 {
    if !candidate.valid_executable {
        6
    } else {
        match candidate.match_kind.as_str() {
            "exact" => 0,
            "compatible" => 1,
            _ if candidate.configured => 2,
            "unknown" => 3,
            "mismatch" => 4,
            _ => 5,
        }
    }
}

fn normalized_path_key(path: &Path) -> String {
    let key = display_path(path).replace('/', "\\");
    if cfg!(windows) {
        key.to_ascii_lowercase()
    } else {
        key
    }
}

fn unity_marker_checks(project_path: &Path) -> [(&'static str, bool); 4] {
    [
        ("Assets", project_path.join("Assets").is_dir()),
        (
            "ProjectSettings",
            project_path.join("ProjectSettings").is_dir(),
        ),
        (
            "Packages/manifest.json",
            project_path.join("Packages/manifest.json").is_file(),
        ),
        (
            "ProjectSettings/ProjectVersion.txt",
            project_path
                .join("ProjectSettings/ProjectVersion.txt")
                .is_file(),
        ),
    ]
}

fn root_contains_extension(root: &Path, extension: &str) -> bool {
    fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .take(256)
        .any(|entry| {
            entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(extension))
        })
}

fn diagnostic(severity: &str, code: &str, message: &str) -> ProjectEnvironmentDiagnostic {
    ProjectEnvironmentDiagnostic {
        severity: severity.to_string(),
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    if let Some(unc) = value.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(local) = value.strip_prefix(r"\\?\") {
        local.to_string()
    } else {
        value.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;

    #[test]
    fn unity_project_inspection_reads_markers_and_full_version() {
        let root = temp_root("unity-project");
        fs::create_dir_all(root.join("Assets")).unwrap();
        fs::create_dir_all(root.join("ProjectSettings")).unwrap();
        fs::create_dir_all(root.join("Packages")).unwrap();
        fs::write(root.join("Packages/manifest.json"), "{}").unwrap();
        fs::write(
            root.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.21f1\nm_EditorVersionWithRevision: 2022.3.21f1 (abc123)\n",
        )
        .unwrap();

        let inspection = inspect_project_directory(&root, "unity");
        assert_eq!(inspection.status, "valid");
        assert_eq!(inspection.detected_engine, "unity");
        assert_eq!(inspection.unity_version.unwrap().version, "2022.3.21f1");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_inspection_reports_partial_and_engine_conflict() {
        let root = temp_root("unity-partial");
        fs::create_dir_all(root.join("Assets")).unwrap();
        fs::create_dir_all(root.join("ProjectSettings")).unwrap();

        let inspection = inspect_project_directory(&root, "godot");
        assert_eq!(inspection.status, "warning");
        assert_eq!(inspection.detected_engine, "unity");
        assert!(
            inspection
                .diagnostics
                .iter()
                .any(|item| item.code == "engine_selection_conflict")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unity_editor_validation_is_strict_and_ranking_is_deterministic() {
        let root = temp_root("unity-editors");
        let exact = root.join("2022.3.21f1/Editor/Unity.exe");
        let compatible = root.join("2022.3.9f1/Editor/Unity.exe");
        let wrong = root.join("2021.3.1f1/Editor/Unity Hub.exe");
        for path in [&exact, &compatible, &wrong] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "fixture").unwrap();
        }
        assert!(unity_editor_file_is_valid(&exact));
        assert!(!unity_editor_file_is_valid(&wrong));

        let ranked = rank_unity_editor_candidates(
            "2022.3.21f1",
            vec![
                candidate_from_path(compatible.clone(), "hub", false, "2022.3.21f1"),
                candidate_from_path(exact.clone(), "configured", true, "2022.3.21f1"),
                candidate_from_path(wrong, "configured", true, "2022.3.21f1"),
                candidate_from_path(exact, "duplicate", false, "2022.3.21f1"),
            ],
        );
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].match_kind, "exact");
        assert_eq!(ranked[1].match_kind, "compatible");
        assert!(!ranked[2].valid_executable);

        let validation = UnityEditorLocator.validate(&root, &compatible);
        assert!(validation.valid);
        assert_eq!(validation.engine, "unity");
        let invalid = validate_editor_selection("unity", &root, root.join("notepad.exe"));
        assert!(!invalid.valid);
        assert_eq!(invalid.error_code, "editor_path_not_found");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn project_detector_traits_keep_engine_marker_rules_independent() {
        let root = temp_root("detector-traits");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("project.godot"), "[application]").unwrap();

        assert!(UnityProjectDetector.detect(&root).is_none());
        let godot = GodotProjectDetector.detect(&root).unwrap();
        assert_eq!(godot.engine, "godot");
        assert_eq!(godot.markers, vec!["project.godot"]);

        fs::remove_file(root.join("project.godot")).unwrap();
        fs::write(root.join("Demo.uproject"), "{}").unwrap();
        let unreal = UnrealProjectDetector.detect(&root).unwrap();
        assert_eq!(unreal.engine, "unreal");
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "adm-new-project-environment-{label}-{}",
            new_stable_id("test").unwrap()
        ))
    }
}
