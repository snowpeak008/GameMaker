use crate::error::{Adm4Error, Adm4Result};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub fn ensure_dir(path: &Path) -> Adm4Result<()> {
    fs::create_dir_all(path)
        .map_err(|error| Adm4Error::io(format!("create dir {} failed: {error}", path.display())))
}

/// 原子写：写临时文件后 rename 覆盖。
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Adm4Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Adm4Error::invalid_input(format!("no parent for {}", path.display())))?;
    ensure_dir(parent)?;
    let temp = parent.join(format!(
        ".{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file")
    ));
    fs::write(&temp, bytes)
        .map_err(|error| Adm4Error::io(format!("write {} failed: {error}", temp.display())))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| Adm4Error::io(format!("remove {} failed: {error}", path.display())))?;
    }
    fs::rename(&temp, path)
        .map_err(|error| Adm4Error::io(format!("rename to {} failed: {error}", path.display())))
}

pub fn write_json_file<T: Serialize>(path: &Path, value: &T) -> Adm4Result<()> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| Adm4Error::internal(format!("serialize failed: {error}")))?;
    atomic_write(path, text.as_bytes())
}

pub fn read_json_file<T: DeserializeOwned>(path: &Path) -> Adm4Result<T> {
    let text = fs::read_to_string(path)
        .map_err(|error| Adm4Error::io(format!("read {} failed: {error}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|error| Adm4Error::validation(format!("parse {} failed: {error}", path.display())))
}

/// 校验相对路径不越界（无 `..`、无根、无盘符）。
pub fn ensure_within_root(relative: &Path) -> Adm4Result<PathBuf> {
    let mut safe = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => safe.push(part),
            Component::CurDir => {}
            _ => {
                return Err(Adm4Error::path_escape(format!(
                    "unsafe path component in {}",
                    relative.display()
                )));
            }
        }
    }
    Ok(safe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_within_root_rejects_parent_components() {
        assert!(ensure_within_root(Path::new("a/../b")).is_err());
        assert!(ensure_within_root(Path::new("a/b.json")).is_ok());
    }
}
