use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::portable::verify_portable_resource_root;

pub const DEFAULT_DIST_EXE_NAME: &str = "AutoDesignMaker.exe";
pub const DEFAULT_MIN_EXE_BYTES: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistBuildPlan {
    pub cwd: String,
    pub command: Vec<String>,
    pub package: String,
    pub profile: String,
    pub target_exe: String,
    pub dist_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DistBundleVerification {
    pub ok: bool,
    pub portable_integrity_checked: bool,
    pub bundle_dir: String,
    pub exe_path: String,
    pub exe_size_bytes: u64,
    pub min_exe_bytes: u64,
    #[serde(default)]
    pub required_items: Vec<String>,
    #[serde(default)]
    pub errors: Vec<String>,
}

pub fn dist_build_plan(newrust_root: &Path) -> DistBuildPlan {
    DistBuildPlan {
        cwd: path_string(newrust_root),
        command: vec![
            "powershell.exe".to_string(),
            "-NoProfile".to_string(),
            "-ExecutionPolicy".to_string(),
            "Bypass".to_string(),
            "-File".to_string(),
            path_string(&newrust_root.join("tools").join("build-portable.ps1")),
        ],
        package: "desktop-tauri".to_string(),
        profile: "portable-release".to_string(),
        target_exe: path_string(
            &newrust_root
                .join("target")
                .join("x86_64-pc-windows-msvc")
                .join("release")
                .join("desktop-tauri.exe"),
        ),
        dist_dir: path_string(&newrust_root.join("dist").join("AutoDesignMaker-NEWrust")),
    }
}

pub fn verify_dist_bundle(
    bundle_dir: &Path,
    exe_name: &str,
    min_exe_bytes: u64,
    required_items: &[String],
) -> DistBundleVerification {
    let exe_path = bundle_dir.join(if exe_name.trim().is_empty() {
        DEFAULT_DIST_EXE_NAME
    } else {
        exe_name
    });
    let mut errors = Vec::new();
    let exe_size_bytes = match std::fs::metadata(&exe_path) {
        Ok(metadata) => metadata.len(),
        Err(_) => {
            errors.push(format!("Executable not found: {}", exe_path.display()));
            0
        }
    };
    if exe_size_bytes > 0 && exe_size_bytes < min_exe_bytes {
        errors.push(format!(
            "Executable is unexpectedly small: {exe_size_bytes} bytes"
        ));
    }
    for item in required_items {
        if !contains_required_item(bundle_dir, item) {
            errors.push(format!("Missing bundled item: {item}"));
        }
    }
    let portable = verify_portable_resource_root(bundle_dir);
    errors.extend(
        portable
            .blockers
            .iter()
            .map(|blocker| format!("Portable integrity: {blocker}")),
    );
    DistBundleVerification {
        ok: errors.is_empty(),
        portable_integrity_checked: true,
        bundle_dir: path_string(bundle_dir),
        exe_path: path_string(&exe_path),
        exe_size_bytes,
        min_exe_bytes,
        required_items: required_items.to_vec(),
        errors,
    }
}

fn contains_required_item(bundle_dir: &Path, relative: &str) -> bool {
    let normalized = relative.trim_matches(['/', '\\']);
    if normalized.is_empty() {
        return true;
    }
    let direct = bundle_dir.join(normalized);
    if direct.exists() {
        return true;
    }
    let Ok(files) = collect_relative_files(bundle_dir) else {
        return false;
    };
    files
        .iter()
        .any(|file| file == normalized || file.starts_with(&format!("{normalized}/")))
}

fn collect_relative_files(root: &Path) -> std::io::Result<Vec<String>> {
    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    Ok(files)
}

fn collect(root: &Path, dir: &Path, files: &mut Vec<String>) -> std::io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect(root, &path, files)?;
        } else if file_type.is_file() {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::new_stable_id;
    use std::path::PathBuf;

    #[test]
    fn dist_build_plan_matches_rust_release_orchestration() {
        let root = Path::new("NEWrust");
        let plan = dist_build_plan(root);
        assert_eq!(
            plan.command,
            vec![
                "powershell.exe",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                "NEWrust/tools/build-portable.ps1"
            ]
        );
        assert!(
            plan.target_exe
                .ends_with("target/x86_64-pc-windows-msvc/release/desktop-tauri.exe")
        );
        assert!(plan.dist_dir.ends_with("dist/AutoDesignMaker-NEWrust"));
    }

    #[test]
    fn dist_bundle_verifier_reports_missing_small_and_required_items() {
        let root = temp_root("dist_verify");
        let required = vec!["resources/app.json".to_string()];

        let missing = verify_dist_bundle(&root, DEFAULT_DIST_EXE_NAME, 10, &required);
        assert!(!missing.ok);
        assert!(
            missing
                .errors
                .iter()
                .any(|error| error.contains("Executable not found"))
        );

        std::fs::write(root.join(DEFAULT_DIST_EXE_NAME), b"12345").unwrap();
        let small = verify_dist_bundle(&root, DEFAULT_DIST_EXE_NAME, 10, &required);
        assert!(!small.ok);
        assert!(
            small
                .errors
                .iter()
                .any(|error| error.contains("unexpectedly small"))
        );

        std::fs::create_dir_all(root.join("resources")).unwrap();
        std::fs::write(root.join("resources").join("app.json"), b"{}").unwrap();
        let structurally_present_but_unverified =
            verify_dist_bundle(&root, DEFAULT_DIST_EXE_NAME, 5, &required);
        assert!(!structurally_present_but_unverified.ok);
        assert!(structurally_present_but_unverified.portable_integrity_checked);
        assert!(
            structurally_present_but_unverified
                .errors
                .iter()
                .any(|error| error.contains("Portable integrity"))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
        std::fs::create_dir_all(&root).unwrap();
        root
    }
}
