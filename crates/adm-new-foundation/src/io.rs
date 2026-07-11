use crate::{AdmError, AdmResult, file_manifest, paths::relative_display, unix_timestamp};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub fn now_iso() -> String {
    format!("unix:{}", unix_timestamp())
}

pub fn timestamp() -> String {
    unix_timestamp().to_string()
}

pub fn rel(path: &Path, root: &Path) -> String {
    let resolved_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let resolved_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    relative_display(&resolved_path, &resolved_root)
}

pub fn write_json(path: &Path, data: &Value) -> AdmResult<PathBuf> {
    let text = serde_json::to_string_pretty(data)
        .map_err(|error| AdmError::new(format!("failed to serialize json: {error}")))?;
    write_text(path, &(text + "\n"))
}

pub fn write_json_serializable<T: Serialize>(path: &Path, data: &T) -> AdmResult<PathBuf> {
    let text = serde_json::to_string_pretty(data)
        .map_err(|error| AdmError::new(format!("failed to serialize json: {error}")))?;
    write_text(path, &(text + "\n"))
}

pub fn read_json(path: &Path, default: Value) -> Value {
    if !path.exists() {
        return default;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return default;
    };
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    serde_json::from_str(text).unwrap_or(default)
}

pub fn write_text(path: &Path, text: &str) -> AdmResult<PathBuf> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)?;
    Ok(path.to_path_buf())
}

pub fn manifest_json(root: &Path) -> AdmResult<Value> {
    serde_json::to_value(file_manifest(root)?)
        .map_err(|error| AdmError::new(format!("failed to serialize manifest: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::new_stable_id;
    use serde_json::json;

    #[test]
    fn json_read_returns_default_for_missing_or_invalid_files() {
        let root = std::env::temp_dir().join(new_stable_id("json_io").unwrap());
        let path = root.join("data.json");

        assert_eq!(
            read_json(&path, json!({"default": true})),
            json!({"default": true})
        );
        write_text(&path, "{bad json").unwrap();
        assert_eq!(
            read_json(&path, json!({"default": true})),
            json!({"default": true})
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn json_write_creates_parent_dirs_and_reads_back() {
        let root = std::env::temp_dir().join(new_stable_id("json_write").unwrap());
        let path = root.join("nested").join("data.json");

        write_json(&path, &json!({"a": 1})).unwrap();

        assert_eq!(read_json(&path, json!(null)), json!({"a": 1}));
        let _ = std::fs::remove_dir_all(root);
    }
}
