use adm4_foundation::{Adm4Result, ensure_dir};
use std::path::{Path, PathBuf};

/// 数据根：默认 `{cwd}/.adm4_data`，可配置；无硬编码用户路径。
#[derive(Debug, Clone)]
pub struct DataRoot {
    root: PathBuf,
}

impl DataRoot {
    pub fn new(root: impl Into<PathBuf>) -> Adm4Result<Self> {
        let root = root.into();
        ensure_dir(&root)?;
        ensure_dir(&root.join("config"))?;
        ensure_dir(&root.join("archives"))?;
        ensure_dir(&root.join("drafts"))?;
        ensure_dir(&root.join("logs"))?;
        Ok(Self { root })
    }

    pub fn default_at_cwd() -> Adm4Result<Self> {
        let cwd = std::env::current_dir()
            .map_err(|error| adm4_foundation::Adm4Error::io(format!("no cwd: {error}")))?;
        Self::new(cwd.join(".adm4_data"))
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }

    pub fn archives_dir(&self) -> PathBuf {
        self.root.join("archives")
    }

    pub fn archive_dir(&self, archive_id: &str) -> PathBuf {
        self.archives_dir().join(archive_id)
    }

    pub fn drafts_dir(&self) -> PathBuf {
        self.root.join("drafts")
    }

    pub fn draft_dir(&self, session_id: &str) -> PathBuf {
        self.drafts_dir().join(session_id)
    }

    pub fn run_log_path(&self) -> PathBuf {
        self.root.join("logs").join("run_log.jsonl")
    }
}
