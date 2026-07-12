use adm_new_foundation::{sha256_hex, source_root::SOURCE_PROJECT_ID};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const PORTABLE_SCHEMA_VERSION: u32 = 1;
pub const PORTABLE_BUILD_ROOT_KIND: &str = "portable-build-root";
pub const PORTABLE_RESOURCE_ROOT_KIND: &str = "portable-resource-root";
pub const PORTABLE_PRODUCT: &str = "AutoDesignMaker NEWrust";
pub const PORTABLE_BUILD_MANIFEST: &str = "build-manifest.json";
pub const PORTABLE_RESOURCE_MANIFEST: &str = "portable-resource-manifest.json";
pub const PORTABLE_EXECUTABLE: &str = "AutoDesignMaker.exe";
pub const PORTABLE_LAUNCHER: &str = "Start-AutoDesignMaker.cmd";
pub const PORTABLE_ARTIFACT_REGISTRY: &str = "pipeline/artifact_layer/registry.json";
pub const PORTABLE_DATA_ROOT: &str = "user_data";
pub const PORTABLE_TARGET_TRIPLE: &str = "x86_64-pc-windows-msvc";
pub const SOURCE_RESOURCE_MANIFEST: &str = "knowledge/resource-manifest.json";
const PORTABLE_README: &str = "README.txt";
const REQUIRED_PORTABLE_SUPPORT_FILES: &[&str] =
    &[PORTABLE_LAUNCHER, PORTABLE_README, SOURCE_RESOURCE_MANIFEST];

pub const REQUIRED_PORTABLE_RESOURCE_GROUPS: &[&str] = &[
    "knowledge/design_data",
    "knowledge/schemas",
    "knowledge/market_data",
    "knowledge/sdks",
    "knowledge/skills",
    "pipeline/artifact_layer",
];

pub const REQUIRED_SOURCE_RESOURCE_GROUPS: &[&str] = &[
    "knowledge/design_data",
    "knowledge/schemas",
    "knowledge/market_data",
    "knowledge/sdks",
    "knowledge/skills",
    "pipeline/artifact_layer",
    "testdata/ui_baselines",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTreeMeasure {
    pub files: u64,
    pub bytes: u64,
    pub tree_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceGroupVerification {
    pub path: String,
    pub expected: ResourceTreeMeasure,
    pub actual: Option<ResourceTreeMeasure>,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceIntegrityBlocker {
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
}

impl fmt::Display for ResourceIntegrityBlocker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(formatter, "{}:{path}: {}", self.code, self.message),
            None => write!(formatter, "{}: {}", self.code, self.message),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceIntegrityError {
    pub code: String,
    pub path: Option<String>,
    pub message: String,
}

impl ResourceIntegrityError {
    fn new(code: impl Into<String>, path: Option<&Path>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            path: path.map(portable_path_display),
            message: message.into(),
        }
    }

    fn blocker(&self) -> ResourceIntegrityBlocker {
        ResourceIntegrityBlocker {
            code: self.code.clone(),
            path: self.path.clone(),
            message: self.message.clone(),
        }
    }
}

impl fmt::Display for ResourceIntegrityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(path) => write!(formatter, "{}:{path}: {}", self.code, self.message),
            None => write!(formatter, "{}: {}", self.code, self.message),
        }
    }
}

impl std::error::Error for ResourceIntegrityError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceResourceManifestVerificationReport {
    pub schema_version: u32,
    pub status: String,
    pub project_id: String,
    pub groups: Vec<ResourceGroupVerification>,
    pub blockers: Vec<ResourceIntegrityBlocker>,
}

impl SourceResourceManifestVerificationReport {
    pub fn passed(&self) -> bool {
        self.blockers.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortableResourceRootVerificationReport {
    pub schema_version: u32,
    pub status: String,
    pub build_root_kind: String,
    pub resource_root_kind: String,
    pub product: String,
    pub groups: Vec<ResourceGroupVerification>,
    pub blockers: Vec<ResourceIntegrityBlocker>,
}

impl PortableResourceRootVerificationReport {
    pub fn passed(&self) -> bool {
        self.blockers.is_empty()
    }
}

#[derive(Debug, Deserialize)]
struct SourceResourceManifest {
    #[serde(rename = "schemaVersion", default)]
    schema_version: u32,
    #[serde(rename = "projectId", default)]
    project_id: String,
    #[serde(default)]
    groups: Vec<SourceResourceGroup>,
}

#[derive(Debug, Deserialize)]
struct SourceResourceGroup {
    #[serde(default)]
    path: String,
    #[serde(default)]
    files: u64,
    #[serde(default)]
    bytes: u64,
    #[serde(rename = "treeSha256", default)]
    tree_sha256: String,
    #[serde(default)]
    mode: String,
}

#[derive(Debug, Deserialize)]
struct PortableResourceManifest {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    root_kind: String,
    #[serde(default)]
    groups: Vec<PortableResourceGroup>,
}

#[derive(Debug, Deserialize)]
struct PortableResourceGroup {
    #[serde(default)]
    path: String,
    #[serde(default)]
    files: u64,
    #[serde(default)]
    bytes: u64,
    #[serde(default)]
    tree_sha256: String,
    #[serde(default)]
    mode: String,
}

#[derive(Debug, Deserialize)]
struct PortableBuildManifest {
    #[serde(default)]
    schema_version: u32,
    #[serde(default)]
    root_kind: String,
    #[serde(default)]
    product: String,
    #[serde(default)]
    target_triple: String,
    #[serde(default)]
    executable: String,
    #[serde(default)]
    executable_sha256: String,
    #[serde(default)]
    executable_bytes: u64,
    #[serde(default)]
    resource_manifest: String,
    #[serde(default)]
    resource_manifest_sha256: String,
    #[serde(default)]
    source_resource_manifest: String,
    #[serde(default)]
    source_resource_manifest_sha256: String,
    #[serde(default)]
    artifact_registry: String,
    #[serde(default)]
    artifact_registry_sha256: String,
    #[serde(default)]
    launcher: String,
    #[serde(default)]
    launcher_sha256: String,
    #[serde(default)]
    launcher_bytes: Option<u64>,
    #[serde(default)]
    portable_data_root: String,
    #[serde(default)]
    support_files: Vec<PortableSupportFileEvidence>,
}

#[derive(Debug, Deserialize)]
struct PortableSupportFileEvidence {
    #[serde(default)]
    path: String,
    #[serde(default)]
    bytes: u64,
    #[serde(default)]
    sha256: String,
}

#[derive(Debug)]
struct TreeFile {
    relative_path: String,
    bytes: Vec<u8>,
}

pub fn measure_resource_tree(
    path: impl AsRef<Path>,
) -> Result<ResourceTreeMeasure, ResourceIntegrityError> {
    let root = path.as_ref();
    let metadata = safe_metadata(root)?;
    let mut files = Vec::new();
    if metadata.is_file() {
        files.push(TreeFile {
            relative_path: ".".to_string(),
            bytes: read_regular_file(root)?,
        });
    } else if metadata.is_dir() {
        collect_tree_files(root, root, &mut files)?;
    } else {
        return Err(ResourceIntegrityError::new(
            "resource_path_kind_invalid",
            Some(root),
            "resource path must be a regular file or directory",
        ));
    }

    files.sort_by(|left, right| {
        left.relative_path
            .to_ascii_lowercase()
            .cmp(&right.relative_path.to_ascii_lowercase())
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    let mut bytes = 0_u64;
    let mut fingerprint_lines = Vec::with_capacity(files.len());
    for file in &files {
        let file_bytes = u64::try_from(file.bytes.len()).map_err(|_| {
            ResourceIntegrityError::new(
                "resource_file_size_overflow",
                Some(root),
                "resource file size does not fit u64",
            )
        })?;
        bytes = bytes.checked_add(file_bytes).ok_or_else(|| {
            ResourceIntegrityError::new(
                "resource_tree_size_overflow",
                Some(root),
                "resource tree byte count overflowed u64",
            )
        })?;
        fingerprint_lines.push(format!(
            "{}|{}|{}",
            file.relative_path,
            file_bytes,
            sha256_hex(&file.bytes)
        ));
    }
    let fingerprint = fingerprint_lines.join("\n");
    Ok(ResourceTreeMeasure {
        files: u64::try_from(files.len()).map_err(|_| {
            ResourceIntegrityError::new(
                "resource_file_count_overflow",
                Some(root),
                "resource file count does not fit u64",
            )
        })?,
        bytes,
        tree_sha256: sha256_hex(fingerprint.as_bytes()),
    })
}

pub fn verify_source_resource_manifest(
    source_root: impl AsRef<Path>,
) -> SourceResourceManifestVerificationReport {
    let source_root = source_root.as_ref();
    let mut blockers = Vec::new();
    let mut groups = Vec::new();
    if let Err(error) = validate_root_directory(source_root) {
        blockers.push(error.blocker());
        return source_report(String::new(), groups, blockers);
    }

    let manifest =
        match read_json_at::<SourceResourceManifest>(source_root, SOURCE_RESOURCE_MANIFEST) {
            Ok(manifest) => manifest,
            Err(error) => {
                blockers.push(error.blocker());
                return source_report(String::new(), groups, blockers);
            }
        };
    if manifest.schema_version != PORTABLE_SCHEMA_VERSION {
        push_blocker(
            &mut blockers,
            "source_resource_schema_version_invalid",
            Some(SOURCE_RESOURCE_MANIFEST),
            format!(
                "expected schema version {PORTABLE_SCHEMA_VERSION}, found {}",
                manifest.schema_version
            ),
        );
    }
    if manifest.project_id != SOURCE_PROJECT_ID {
        push_blocker(
            &mut blockers,
            "source_resource_project_id_invalid",
            Some(SOURCE_RESOURCE_MANIFEST),
            format!(
                "expected project id {SOURCE_PROJECT_ID}, found {}",
                manifest.project_id
            ),
        );
    }
    let declared = manifest
        .groups
        .iter()
        .map(|group| group.path.as_str())
        .collect::<Vec<_>>();
    validate_required_groups(
        &declared,
        REQUIRED_SOURCE_RESOURCE_GROUPS,
        "source_resource",
        &mut blockers,
    );
    for group in &manifest.groups {
        if !matches!(
            group.mode.as_str(),
            "required-read-only" | "seed-read-only" | "test-fixture"
        ) {
            push_blocker(
                &mut blockers,
                "source_resource_group_mode_invalid",
                Some(&group.path),
                format!("unsupported source resource group mode: {}", group.mode),
            );
        }
        if let Some(expected_mode) = expected_source_group_mode(&group.path)
            && group.mode != expected_mode
        {
            push_blocker(
                &mut blockers,
                "source_resource_group_mode_mismatch",
                Some(&group.path),
                format!("expected mode {expected_mode}, found {}", group.mode),
            );
        }
        if REQUIRED_SOURCE_RESOURCE_GROUPS.contains(&group.path.as_str()) && group.files == 0 {
            push_blocker(
                &mut blockers,
                "source_resource_group_empty",
                Some(&group.path),
                "required source resource group must contain at least one file",
            );
        }
    }
    verify_groups(
        source_root,
        manifest.groups.iter().map(|group| {
            (
                group.path.as_str(),
                ResourceTreeMeasure {
                    files: group.files,
                    bytes: group.bytes,
                    tree_sha256: group.tree_sha256.clone(),
                },
            )
        }),
        "source_resource",
        &mut groups,
        &mut blockers,
    );
    source_report(manifest.project_id, groups, blockers)
}

pub fn verify_portable_resource_root(
    portable_root: impl AsRef<Path>,
) -> PortableResourceRootVerificationReport {
    let portable_root = portable_root.as_ref();
    let mut blockers = Vec::new();
    let mut groups = Vec::new();
    if let Err(error) = validate_root_directory(portable_root) {
        blockers.push(error.blocker());
        return portable_report(
            String::new(),
            String::new(),
            String::new(),
            groups,
            blockers,
        );
    }

    let (build_manifest, build_bytes) = match read_json_with_bytes_at::<PortableBuildManifest>(
        portable_root,
        PORTABLE_BUILD_MANIFEST,
    ) {
        Ok(value) => value,
        Err(error) => {
            blockers.push(error.blocker());
            return portable_report(
                String::new(),
                String::new(),
                String::new(),
                groups,
                blockers,
            );
        }
    };
    let _ = build_bytes;
    verify_build_identity(&build_manifest, &mut blockers);

    let (resource_manifest, resource_manifest_bytes) =
        match read_json_with_bytes_at::<PortableResourceManifest>(
            portable_root,
            PORTABLE_RESOURCE_MANIFEST,
        ) {
            Ok(value) => value,
            Err(error) => {
                blockers.push(error.blocker());
                verify_build_files(portable_root, &build_manifest, &mut blockers);
                return portable_report(
                    build_manifest.root_kind,
                    String::new(),
                    build_manifest.product,
                    groups,
                    blockers,
                );
            }
        };
    if resource_manifest.schema_version != PORTABLE_SCHEMA_VERSION {
        push_blocker(
            &mut blockers,
            "portable_resource_schema_version_invalid",
            Some(PORTABLE_RESOURCE_MANIFEST),
            format!(
                "expected schema version {PORTABLE_SCHEMA_VERSION}, found {}",
                resource_manifest.schema_version
            ),
        );
    }
    if resource_manifest.root_kind != PORTABLE_RESOURCE_ROOT_KIND {
        push_blocker(
            &mut blockers,
            "portable_resource_root_kind_invalid",
            Some(PORTABLE_RESOURCE_MANIFEST),
            format!(
                "expected root kind {PORTABLE_RESOURCE_ROOT_KIND}, found {}",
                resource_manifest.root_kind
            ),
        );
    }
    verify_declared_identity(
        &build_manifest.resource_manifest,
        PORTABLE_RESOURCE_MANIFEST,
        "portable_resource_manifest_path_invalid",
        PORTABLE_BUILD_MANIFEST,
        &mut blockers,
    );
    verify_hash(
        &resource_manifest_bytes,
        &build_manifest.resource_manifest_sha256,
        "portable_resource_manifest_hash_mismatch",
        PORTABLE_RESOURCE_MANIFEST,
        &mut blockers,
    );

    let declared = resource_manifest
        .groups
        .iter()
        .map(|group| group.path.as_str())
        .collect::<Vec<_>>();
    validate_required_groups(
        &declared,
        REQUIRED_PORTABLE_RESOURCE_GROUPS,
        "portable_resource",
        &mut blockers,
    );
    let expected_paths = REQUIRED_PORTABLE_RESOURCE_GROUPS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for path in &declared {
        if !expected_paths.contains(path) {
            push_blocker(
                &mut blockers,
                "portable_resource_group_unexpected",
                Some(path),
                "portable resource manifest contains an unexpected group",
            );
        }
    }
    for group in &resource_manifest.groups {
        let expected_mode = expected_source_group_mode(&group.path).unwrap_or("");
        if group.mode != expected_mode {
            push_blocker(
                &mut blockers,
                "portable_resource_group_mode_mismatch",
                Some(&group.path),
                format!("expected mode {expected_mode}, found {}", group.mode),
            );
        }
        if group.files == 0 {
            push_blocker(
                &mut blockers,
                "portable_resource_group_empty",
                Some(&group.path),
                "portable resource group must contain at least one file",
            );
        }
    }
    verify_groups(
        portable_root,
        resource_manifest.groups.iter().map(|group| {
            (
                group.path.as_str(),
                ResourceTreeMeasure {
                    files: group.files,
                    bytes: group.bytes,
                    tree_sha256: group.tree_sha256.clone(),
                },
            )
        }),
        "portable_resource",
        &mut groups,
        &mut blockers,
    );
    verify_build_files(portable_root, &build_manifest, &mut blockers);
    portable_report(
        build_manifest.root_kind,
        resource_manifest.root_kind,
        build_manifest.product,
        groups,
        blockers,
    )
}

fn verify_build_identity(
    manifest: &PortableBuildManifest,
    blockers: &mut Vec<ResourceIntegrityBlocker>,
) {
    if manifest.schema_version != PORTABLE_SCHEMA_VERSION {
        push_blocker(
            blockers,
            "portable_build_schema_version_invalid",
            Some(PORTABLE_BUILD_MANIFEST),
            format!(
                "expected schema version {PORTABLE_SCHEMA_VERSION}, found {}",
                manifest.schema_version
            ),
        );
    }
    for (actual, expected, code, label) in [
        (
            manifest.root_kind.as_str(),
            PORTABLE_BUILD_ROOT_KIND,
            "portable_build_root_kind_invalid",
            "root kind",
        ),
        (
            manifest.product.as_str(),
            PORTABLE_PRODUCT,
            "portable_build_product_invalid",
            "product",
        ),
        (
            manifest.target_triple.as_str(),
            PORTABLE_TARGET_TRIPLE,
            "portable_build_target_invalid",
            "target triple",
        ),
        (
            manifest.portable_data_root.as_str(),
            PORTABLE_DATA_ROOT,
            "portable_data_root_invalid",
            "portable data root",
        ),
    ] {
        if actual != expected {
            push_blocker(
                blockers,
                code,
                Some(PORTABLE_BUILD_MANIFEST),
                format!("expected {label} {expected}, found {actual}"),
            );
        }
    }
}

fn verify_build_files(
    root: &Path,
    manifest: &PortableBuildManifest,
    blockers: &mut Vec<ResourceIntegrityBlocker>,
) {
    verify_declared_file(
        root,
        &manifest.executable,
        PORTABLE_EXECUTABLE,
        &manifest.executable_sha256,
        Some(manifest.executable_bytes),
        "portable_executable",
        blockers,
    );
    verify_declared_file(
        root,
        &manifest.artifact_registry,
        PORTABLE_ARTIFACT_REGISTRY,
        &manifest.artifact_registry_sha256,
        None,
        "portable_artifact_registry",
        blockers,
    );
    verify_declared_file(
        root,
        &manifest.launcher,
        PORTABLE_LAUNCHER,
        &manifest.launcher_sha256,
        manifest.launcher_bytes,
        "portable_launcher",
        blockers,
    );
    verify_declared_file(
        root,
        &manifest.source_resource_manifest,
        SOURCE_RESOURCE_MANIFEST,
        &manifest.source_resource_manifest_sha256,
        None,
        "portable_source_resource_manifest",
        blockers,
    );
    verify_support_files(root, &manifest.support_files, blockers);

    if let Ok(bytes) = read_file_at(root, PORTABLE_ARTIFACT_REGISTRY) {
        match parse_bom_json::<serde_json::Value>(&bytes, PORTABLE_ARTIFACT_REGISTRY) {
            Ok(registry)
                if registry
                    .get("version")
                    .and_then(serde_json::Value::as_u64)
                    .is_some()
                    && registry
                        .get("artifacts")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|artifacts| !artifacts.is_empty()) => {}
            Ok(_) => push_blocker(
                blockers,
                "portable_artifact_registry_identity_invalid",
                Some(PORTABLE_ARTIFACT_REGISTRY),
                "artifact registry must declare a version and non-empty artifacts",
            ),
            Err(error) => blockers.push(error.blocker()),
        }
    }
}

fn verify_support_files(
    root: &Path,
    support_files: &[PortableSupportFileEvidence],
    blockers: &mut Vec<ResourceIntegrityBlocker>,
) {
    let declared = support_files
        .iter()
        .map(|evidence| evidence.path.as_str())
        .collect::<Vec<_>>();
    validate_required_groups(
        &declared,
        REQUIRED_PORTABLE_SUPPORT_FILES,
        "portable_support_file",
        blockers,
    );
    let expected = REQUIRED_PORTABLE_SUPPORT_FILES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for evidence in support_files {
        if !expected.contains(evidence.path.as_str()) {
            push_blocker(
                blockers,
                "portable_support_file_unexpected",
                Some(&evidence.path),
                "build manifest contains an unexpected portable support file",
            );
            continue;
        }
        let bytes = match read_file_at(root, &evidence.path) {
            Ok(bytes) => bytes,
            Err(error) => {
                blockers.push(error.blocker());
                continue;
            }
        };
        verify_hash(
            &bytes,
            &evidence.sha256,
            "portable_support_file_hash_mismatch",
            &evidence.path,
            blockers,
        );
        let actual_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual_bytes != evidence.bytes {
            push_blocker(
                blockers,
                "portable_support_file_size_mismatch",
                Some(&evidence.path),
                format!("expected {} bytes, found {actual_bytes}", evidence.bytes),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn verify_declared_file(
    root: &Path,
    declared_path: &str,
    expected_path: &str,
    expected_sha256: &str,
    expected_bytes: Option<u64>,
    code_prefix: &str,
    blockers: &mut Vec<ResourceIntegrityBlocker>,
) {
    verify_declared_identity(
        declared_path,
        expected_path,
        &format!("{code_prefix}_path_invalid"),
        PORTABLE_BUILD_MANIFEST,
        blockers,
    );
    let bytes = match read_file_at(root, expected_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            blockers.push(error.blocker());
            return;
        }
    };
    verify_hash(
        &bytes,
        expected_sha256,
        &format!("{code_prefix}_hash_mismatch"),
        expected_path,
        blockers,
    );
    if let Some(expected_bytes) = expected_bytes {
        let actual_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual_bytes != expected_bytes {
            push_blocker(
                blockers,
                format!("{code_prefix}_size_mismatch"),
                Some(expected_path),
                format!("expected {expected_bytes} bytes, found {actual_bytes}"),
            );
        }
    }
}

fn verify_declared_identity(
    actual: &str,
    expected: &str,
    code: &str,
    manifest_path: &str,
    blockers: &mut Vec<ResourceIntegrityBlocker>,
) {
    if actual != expected {
        push_blocker(
            blockers,
            code,
            Some(manifest_path),
            format!("expected path {expected}, found {actual}"),
        );
    }
}

fn verify_hash(
    bytes: &[u8],
    expected_sha256: &str,
    code: &str,
    path: &str,
    blockers: &mut Vec<ResourceIntegrityBlocker>,
) {
    let actual = sha256_hex(bytes);
    if expected_sha256.len() != 64
        || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !actual.eq_ignore_ascii_case(expected_sha256)
    {
        push_blocker(
            blockers,
            code,
            Some(path),
            format!("expected sha256 {expected_sha256}, found {actual}"),
        );
    }
}

fn validate_required_groups(
    declared: &[&str],
    required: &[&str],
    code_prefix: &str,
    blockers: &mut Vec<ResourceIntegrityBlocker>,
) {
    let mut unique = BTreeSet::new();
    for path in declared {
        let identity = path.to_ascii_lowercase();
        if !unique.insert(identity) {
            push_blocker(
                blockers,
                format!("{code_prefix}_group_duplicate"),
                Some(path),
                "resource group path is duplicated",
            );
        }
    }
    for required_path in required {
        if !declared.iter().any(|path| path == required_path) {
            push_blocker(
                blockers,
                format!("{code_prefix}_group_missing"),
                Some(required_path),
                "required resource group is missing",
            );
        }
    }
}

fn expected_source_group_mode(path: &str) -> Option<&'static str> {
    match path {
        "knowledge/design_data" | "knowledge/schemas" | "pipeline/artifact_layer" => {
            Some("required-read-only")
        }
        "knowledge/market_data" | "knowledge/sdks" | "knowledge/skills" => Some("seed-read-only"),
        "testdata/ui_baselines" => Some("test-fixture"),
        _ => None,
    }
}

fn verify_groups<'a>(
    root: &Path,
    declared: impl Iterator<Item = (&'a str, ResourceTreeMeasure)>,
    code_prefix: &str,
    groups: &mut Vec<ResourceGroupVerification>,
    blockers: &mut Vec<ResourceIntegrityBlocker>,
) {
    for (path, expected) in declared {
        let actual = match resolve_safe_existing(root, path) {
            Ok(group_path) => measure_resource_tree(&group_path),
            Err(error) => Err(error),
        };
        match actual {
            Ok(actual) => {
                let passed = actual == expected;
                if !passed {
                    push_blocker(
                        blockers,
                        format!("{code_prefix}_group_measure_mismatch"),
                        Some(path),
                        format!("expected {expected:?}, found {actual:?}"),
                    );
                }
                groups.push(ResourceGroupVerification {
                    path: path.to_string(),
                    expected,
                    actual: Some(actual),
                    passed,
                });
            }
            Err(error) => {
                blockers.push(error.blocker());
                groups.push(ResourceGroupVerification {
                    path: path.to_string(),
                    expected,
                    actual: None,
                    passed: false,
                });
            }
        }
    }
}

fn source_report(
    project_id: String,
    groups: Vec<ResourceGroupVerification>,
    blockers: Vec<ResourceIntegrityBlocker>,
) -> SourceResourceManifestVerificationReport {
    SourceResourceManifestVerificationReport {
        schema_version: PORTABLE_SCHEMA_VERSION,
        status: status(&blockers),
        project_id,
        groups,
        blockers,
    }
}

fn portable_report(
    build_root_kind: String,
    resource_root_kind: String,
    product: String,
    groups: Vec<ResourceGroupVerification>,
    blockers: Vec<ResourceIntegrityBlocker>,
) -> PortableResourceRootVerificationReport {
    PortableResourceRootVerificationReport {
        schema_version: PORTABLE_SCHEMA_VERSION,
        status: status(&blockers),
        build_root_kind,
        resource_root_kind,
        product,
        groups,
        blockers,
    }
}

fn status(blockers: &[ResourceIntegrityBlocker]) -> String {
    if blockers.is_empty() {
        "passed"
    } else {
        "blocked"
    }
    .to_string()
}

fn push_blocker(
    blockers: &mut Vec<ResourceIntegrityBlocker>,
    code: impl Into<String>,
    path: Option<&str>,
    message: impl Into<String>,
) {
    blockers.push(ResourceIntegrityBlocker {
        code: code.into(),
        path: path.map(str::to_string),
        message: message.into(),
    });
}

fn validate_root_directory(root: &Path) -> Result<(), ResourceIntegrityError> {
    let metadata = safe_metadata(root)?;
    if !metadata.is_dir() {
        return Err(ResourceIntegrityError::new(
            "resource_root_not_directory",
            Some(root),
            "resource root must be a directory",
        ));
    }
    Ok(())
}

fn read_json_at<T: DeserializeOwned>(
    root: &Path,
    relative_path: &str,
) -> Result<T, ResourceIntegrityError> {
    read_json_with_bytes_at(root, relative_path).map(|(value, _)| value)
}

fn read_json_with_bytes_at<T: DeserializeOwned>(
    root: &Path,
    relative_path: &str,
) -> Result<(T, Vec<u8>), ResourceIntegrityError> {
    let bytes = read_file_at(root, relative_path)?;
    let value = parse_bom_json(&bytes, relative_path)?;
    Ok((value, bytes))
}

fn parse_bom_json<T: DeserializeOwned>(
    bytes: &[u8],
    label: &str,
) -> Result<T, ResourceIntegrityError> {
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    serde_json::from_slice(bytes).map_err(|error| {
        ResourceIntegrityError::new(
            "resource_json_invalid",
            Some(Path::new(label)),
            format!("JSON parsing failed: {error}"),
        )
    })
}

fn read_file_at(root: &Path, relative_path: &str) -> Result<Vec<u8>, ResourceIntegrityError> {
    let path = resolve_safe_existing(root, relative_path)?;
    read_regular_file(&path)
}

fn resolve_safe_existing(
    root: &Path,
    relative_path: &str,
) -> Result<PathBuf, ResourceIntegrityError> {
    let relative = validate_relative_path(relative_path)?;
    validate_root_directory(root)?;
    let mut candidate = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(ResourceIntegrityError::new(
                "resource_relative_path_invalid",
                Some(Path::new(relative_path)),
                "resource path contains a non-portable component",
            ));
        };
        candidate.push(component);
        safe_metadata(&candidate)?;
    }
    let canonical_root = root.canonicalize().map_err(|error| {
        ResourceIntegrityError::new(
            "resource_root_canonicalize_failed",
            Some(root),
            error.to_string(),
        )
    })?;
    let canonical_candidate = candidate.canonicalize().map_err(|error| {
        ResourceIntegrityError::new(
            "resource_path_canonicalize_failed",
            Some(&candidate),
            error.to_string(),
        )
    })?;
    if !canonical_candidate.starts_with(&canonical_root) {
        return Err(ResourceIntegrityError::new(
            "resource_path_escaped_root",
            Some(&candidate),
            "resource path resolved outside the portable root",
        ));
    }
    Ok(candidate)
}

fn validate_relative_path(relative_path: &str) -> Result<PathBuf, ResourceIntegrityError> {
    if relative_path.is_empty()
        || relative_path.trim() != relative_path
        || relative_path.contains('\\')
        || relative_path.contains(':')
        || relative_path.contains('\0')
        || relative_path.contains('|')
        || relative_path.contains('\r')
        || relative_path.contains('\n')
        || relative_path.starts_with('/')
        || relative_path.ends_with('/')
        || relative_path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(ResourceIntegrityError::new(
            "resource_relative_path_invalid",
            Some(Path::new(relative_path)),
            "resource path must be a clean, forward-slash, root-relative portable path",
        ));
    }
    let relative = PathBuf::from(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ResourceIntegrityError::new(
            "resource_relative_path_invalid",
            Some(Path::new(relative_path)),
            "resource path must contain only normal relative components",
        ));
    }
    Ok(relative)
}

fn safe_metadata(path: &Path) -> Result<fs::Metadata, ResourceIntegrityError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        ResourceIntegrityError::new("resource_path_missing", Some(path), error.to_string())
    })?;
    if is_reparse_or_symlink(&metadata) {
        return Err(ResourceIntegrityError::new(
            "resource_reparse_point_refused",
            Some(path),
            "resource paths must not contain symlinks, junctions, or other reparse points",
        ));
    }
    Ok(metadata)
}

fn read_regular_file(path: &Path) -> Result<Vec<u8>, ResourceIntegrityError> {
    let metadata = safe_metadata(path)?;
    if !metadata.is_file() {
        return Err(ResourceIntegrityError::new(
            "resource_file_kind_invalid",
            Some(path),
            "expected a regular file",
        ));
    }
    fs::read(path).map_err(|error| {
        ResourceIntegrityError::new("resource_file_read_failed", Some(path), error.to_string())
    })
}

fn collect_tree_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<TreeFile>,
) -> Result<(), ResourceIntegrityError> {
    let metadata = safe_metadata(directory)?;
    if !metadata.is_dir() {
        return Err(ResourceIntegrityError::new(
            "resource_directory_kind_invalid",
            Some(directory),
            "tree traversal encountered a non-directory",
        ));
    }
    let entries = fs::read_dir(directory).map_err(|error| {
        ResourceIntegrityError::new(
            "resource_directory_read_failed",
            Some(directory),
            error.to_string(),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            ResourceIntegrityError::new(
                "resource_directory_entry_failed",
                Some(directory),
                error.to_string(),
            )
        })?;
        let path = entry.path();
        let metadata = safe_metadata(&path)?;
        if metadata.is_dir() {
            collect_tree_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative_path = tree_relative_path(root, &path)?;
            files.push(TreeFile {
                relative_path,
                bytes: read_regular_file(&path)?,
            });
        } else {
            return Err(ResourceIntegrityError::new(
                "resource_path_kind_invalid",
                Some(&path),
                "resource tree contains a non-file, non-directory entry",
            ));
        }
    }
    Ok(())
}

fn tree_relative_path(root: &Path, path: &Path) -> Result<String, ResourceIntegrityError> {
    let relative = path.strip_prefix(root).map_err(|_| {
        ResourceIntegrityError::new(
            "resource_path_escaped_root",
            Some(path),
            "resource file is outside the measured tree",
        )
    })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(ResourceIntegrityError::new(
                "resource_tree_path_invalid",
                Some(path),
                "resource file path contains a non-portable component",
            ));
        };
        let part = component.to_str().ok_or_else(|| {
            ResourceIntegrityError::new(
                "resource_tree_path_not_utf8",
                Some(path),
                "resource file path must be valid UTF-8",
            )
        })?;
        if part.is_empty()
            || part.contains('|')
            || part.contains('\r')
            || part.contains('\n')
            || part.contains(':')
        {
            return Err(ResourceIntegrityError::new(
                "resource_tree_path_invalid",
                Some(path),
                "resource file name is not portable in a tree fingerprint",
            ));
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(ResourceIntegrityError::new(
            "resource_tree_path_invalid",
            Some(path),
            "resource file path is empty",
        ));
    }
    Ok(parts.join("/"))
}

#[cfg(windows)]
fn is_reparse_or_symlink(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_or_symlink(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn portable_path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;
    use serde_json::{Value, json};

    #[test]
    fn portable_root_verifier_accepts_bom_manifests_and_complete_integrity() {
        let fixture = PortableFixture::new(true);

        let report = verify_portable_resource_root(&fixture.root);

        assert!(report.passed(), "{:#?}", report.blockers);
        assert_eq!(report.status, "passed");
        assert_eq!(report.groups.len(), REQUIRED_PORTABLE_RESOURCE_GROUPS.len());
        fixture.cleanup();
    }

    #[test]
    fn portable_root_verifier_blocks_resource_and_launcher_tampering() {
        let fixture = PortableFixture::new(false);
        fs::write(
            fixture.root.join("knowledge/design_data/data.json"),
            br#"{"tampered":true}"#,
        )
        .unwrap();
        fs::write(
            fixture.root.join(PORTABLE_LAUNCHER),
            b"@echo off\r\necho tampered\r\n",
        )
        .unwrap();

        let report = verify_portable_resource_root(&fixture.root);

        assert!(!report.passed());
        assert!(has_code(
            &report.blockers,
            "portable_resource_group_measure_mismatch"
        ));
        assert!(has_code(
            &report.blockers,
            "portable_launcher_hash_mismatch"
        ));
        fixture.cleanup();
    }

    #[test]
    fn portable_root_verifier_blocks_manifest_path_traversal() {
        let fixture = PortableFixture::new(false);
        let manifest_path = fixture.root.join(PORTABLE_RESOURCE_MANIFEST);
        let mut manifest: Value =
            serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        manifest["groups"][0]["path"] = json!("knowledge/design_data/../../outside");
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
        fs::write(&manifest_path, &manifest_bytes).unwrap();
        fixture.rewrite_build_resource_hash(&manifest_bytes);

        let report = verify_portable_resource_root(&fixture.root);

        assert!(!report.passed());
        assert!(has_code(&report.blockers, "resource_relative_path_invalid"));
        fixture.cleanup();
    }

    #[test]
    fn source_resource_manifest_verifier_detects_group_tampering() {
        let root = temp_root("source-resource");
        write_resource_groups(&root, REQUIRED_SOURCE_RESOURCE_GROUPS);
        write_source_manifest(&root, true);
        let initial = verify_source_resource_manifest(&root);
        assert!(initial.passed(), "{:#?}", initial.blockers);

        fs::write(
            root.join("testdata/ui_baselines/data.json"),
            br#"{"changed":true}"#,
        )
        .unwrap();
        let tampered = verify_source_resource_manifest(&root);

        assert!(!tampered.passed());
        assert!(has_code(
            &tampered.blockers,
            "source_resource_group_measure_mismatch"
        ));
        cleanup(&root);
    }

    #[test]
    fn tracked_source_resource_manifest_matches_the_independent_checkout() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

        let report = verify_source_resource_manifest(root);

        assert!(report.passed(), "{:#?}", report.blockers);
    }

    #[test]
    fn resource_tree_measure_refuses_reparse_descendants_when_supported() {
        let root = temp_root("resource-reparse");
        let group = root.join("knowledge/design_data");
        fs::create_dir_all(&group).unwrap();
        let outside = root.join("outside.json");
        fs::write(&outside, b"outside").unwrap();
        let link = group.join("linked.json");
        if !create_file_symlink(&outside, &link) {
            cleanup(&root);
            return;
        }

        let error = measure_resource_tree(&group).unwrap_err();

        assert_eq!(error.code, "resource_reparse_point_refused");
        cleanup(&root);
    }

    fn has_code(blockers: &[ResourceIntegrityBlocker], code: &str) -> bool {
        blockers.iter().any(|blocker| blocker.code == code)
    }

    struct PortableFixture {
        root: PathBuf,
    }

    impl PortableFixture {
        fn new(with_bom: bool) -> Self {
            let root = temp_root("portable-root");
            write_resource_groups(&root, REQUIRED_PORTABLE_RESOURCE_GROUPS);
            fs::write(root.join(PORTABLE_EXECUTABLE), b"MZ portable fixture").unwrap();
            fs::write(
                root.join(PORTABLE_LAUNCHER),
                b"@echo off\r\nstart \"\" \"%~dp0AutoDesignMaker.exe\"\r\n",
            )
            .unwrap();
            fs::write(root.join(PORTABLE_README), b"portable fixture").unwrap();
            fs::write(
                root.join(SOURCE_RESOURCE_MANIFEST),
                br#"{"schemaVersion":1,"projectId":"autodesignmaker-rust-v2"}"#,
            )
            .unwrap();

            let groups = REQUIRED_PORTABLE_RESOURCE_GROUPS
                .iter()
                .map(|path| {
                    let measure = measure_resource_tree(root.join(path)).unwrap();
                    json!({
                        "path": path,
                        "mode": expected_source_group_mode(path).unwrap(),
                        "files": measure.files,
                        "bytes": measure.bytes,
                        "tree_sha256": measure.tree_sha256,
                    })
                })
                .collect::<Vec<_>>();
            let resource_manifest = json!({
                "schema_version": PORTABLE_SCHEMA_VERSION,
                "root_kind": PORTABLE_RESOURCE_ROOT_KIND,
                "groups": groups,
            });
            let resource_bytes = json_bytes(&resource_manifest, with_bom);
            fs::write(root.join(PORTABLE_RESOURCE_MANIFEST), &resource_bytes).unwrap();

            let executable = fs::read(root.join(PORTABLE_EXECUTABLE)).unwrap();
            let registry = fs::read(root.join(PORTABLE_ARTIFACT_REGISTRY)).unwrap();
            let launcher = fs::read(root.join(PORTABLE_LAUNCHER)).unwrap();
            let source_manifest = fs::read(root.join(SOURCE_RESOURCE_MANIFEST)).unwrap();
            let readme = fs::read(root.join(PORTABLE_README)).unwrap();
            let build_manifest = json!({
                "schema_version": PORTABLE_SCHEMA_VERSION,
                "root_kind": PORTABLE_BUILD_ROOT_KIND,
                "product": PORTABLE_PRODUCT,
                "target_triple": PORTABLE_TARGET_TRIPLE,
                "portable_data_root": PORTABLE_DATA_ROOT,
                "resource_manifest": PORTABLE_RESOURCE_MANIFEST,
                "resource_manifest_sha256": sha256_hex(&resource_bytes),
                "source_resource_manifest": SOURCE_RESOURCE_MANIFEST,
                "source_resource_manifest_sha256": sha256_hex(&source_manifest),
                "executable": PORTABLE_EXECUTABLE,
                "executable_sha256": sha256_hex(&executable),
                "executable_bytes": executable.len(),
                "artifact_registry": PORTABLE_ARTIFACT_REGISTRY,
                "artifact_registry_sha256": sha256_hex(&registry),
                "launcher": PORTABLE_LAUNCHER,
                "launcher_sha256": sha256_hex(&launcher),
                "launcher_bytes": launcher.len(),
                "support_files": [
                    {"path": PORTABLE_LAUNCHER, "bytes": launcher.len(), "sha256": sha256_hex(&launcher)},
                    {"path": PORTABLE_README, "bytes": readme.len(), "sha256": sha256_hex(&readme)},
                    {"path": SOURCE_RESOURCE_MANIFEST, "bytes": source_manifest.len(), "sha256": sha256_hex(&source_manifest)},
                ],
            });
            fs::write(
                root.join(PORTABLE_BUILD_MANIFEST),
                json_bytes(&build_manifest, with_bom),
            )
            .unwrap();
            Self { root }
        }

        fn rewrite_build_resource_hash(&self, resource_manifest_bytes: &[u8]) {
            let path = self.root.join(PORTABLE_BUILD_MANIFEST);
            let bytes = fs::read(&path).unwrap();
            let json_bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
            let mut manifest: Value = serde_json::from_slice(json_bytes).unwrap();
            manifest["resource_manifest_sha256"] = json!(sha256_hex(resource_manifest_bytes));
            fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
        }

        fn cleanup(self) {
            cleanup(&self.root);
        }
    }

    fn write_resource_groups(root: &Path, groups: &[&str]) {
        for path in groups {
            let directory = root.join(path);
            fs::create_dir_all(&directory).unwrap();
            if *path == "pipeline/artifact_layer" {
                fs::write(
                    directory.join("registry.json"),
                    br#"{"version":1,"artifacts":[{"id":"fixture"}]}"#,
                )
                .unwrap();
            } else {
                fs::write(
                    directory.join("data.json"),
                    format!("{{\"group\":{}}}", serde_json::to_string(path).unwrap()),
                )
                .unwrap();
            }
        }
    }

    fn write_source_manifest(root: &Path, with_bom: bool) {
        let groups = REQUIRED_SOURCE_RESOURCE_GROUPS
            .iter()
            .map(|path| {
                let measure = measure_resource_tree(root.join(path)).unwrap();
                json!({
                    "path": path,
                    "files": measure.files,
                    "bytes": measure.bytes,
                    "treeSha256": measure.tree_sha256,
                    "mode": expected_source_group_mode(path).unwrap(),
                })
            })
            .collect::<Vec<_>>();
        let manifest = json!({
            "schemaVersion": PORTABLE_SCHEMA_VERSION,
            "projectId": SOURCE_PROJECT_ID,
            "groups": groups,
        });
        let path = root.join(SOURCE_RESOURCE_MANIFEST);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, json_bytes(&manifest, with_bom)).unwrap();
    }

    fn json_bytes(value: &Value, with_bom: bool) -> Vec<u8> {
        let json = serde_json::to_vec_pretty(value).unwrap();
        if !with_bom {
            return json;
        }
        let mut bytes = vec![0xef, 0xbb, 0xbf];
        bytes.extend(json);
        bytes
    }

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(new_stable_id(label).unwrap());
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[cfg(unix)]
    fn create_file_symlink(source: &Path, target: &Path) -> bool {
        std::os::unix::fs::symlink(source, target).is_ok()
    }

    #[cfg(windows)]
    fn create_file_symlink(source: &Path, target: &Path) -> bool {
        std::os::windows::fs::symlink_file(source, target).is_ok()
    }
}
