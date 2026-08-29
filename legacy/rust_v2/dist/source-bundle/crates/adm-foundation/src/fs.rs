use crate::{AdmError, AdmErrorKind, AdmResult};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn read_to_string(path: impl AsRef<Path>) -> AdmResult<String> {
    fs::read_to_string(path).map_err(AdmError::from)
}

pub fn write_string(path: impl AsRef<Path>, value: &str) -> AdmResult<()> {
    atomic_write(path, value.as_bytes())
}

pub fn atomic_write(path: impl AsRef<Path>, bytes: &[u8]) -> AdmResult<()> {
    let path = path.as_ref();
    let parent = path.parent().ok_or_else(|| {
        AdmError::new(
            AdmErrorKind::InvalidInput,
            "atomic write target must have a parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            AdmError::new(
                AdmErrorKind::InvalidInput,
                "atomic write target must have a valid file name",
            )
        })?;
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let temp_path = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        millis
    ));

    let write_result = (|| -> AdmResult<()> {
        let mut file = File::create(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        if path.exists() {
            fs::remove_file(path)?;
        }
        fs::rename(&temp_path, path)?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    write_result.with_context(|| format!("atomic_write target={}", path.display()))
}

trait ResultContext<T> {
    fn with_context<F>(self, context: F) -> AdmResult<T>
    where
        F: FnOnce() -> String;
}

impl<T> ResultContext<T> for AdmResult<T> {
    fn with_context<F>(self, context: F) -> AdmResult<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|error| error.with_context(context()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_creates_file() {
        let root = std::env::temp_dir().join(format!(
            "adm_atomic_write_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        let path = root.join("nested").join("file.txt");
        atomic_write(&path, b"hello").expect("write");
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
        let _ = fs::remove_dir_all(root);
    }
}
