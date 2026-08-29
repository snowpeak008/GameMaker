use adm4_archive::DataRoot;
use adm4_foundation::{Adm4Result, UtcTimestamp};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunLogEntry {
    pub at: String,
    pub category: String,
    pub message: String,
}

/// 结构化运行日志（jsonl 追加）。
pub struct RunLog {
    path: std::path::PathBuf,
}

impl RunLog {
    pub fn new(data_root: &DataRoot) -> Self {
        Self {
            path: data_root.run_log_path(),
        }
    }

    pub fn append(&self, category: &str, message: &str) -> Adm4Result<()> {
        let entry = RunLogEntry {
            at: UtcTimestamp::now().to_iso8601(),
            category: category.to_string(),
            message: message.to_string(),
        };
        let line = serde_json::to_string(&entry).map_err(|error| {
            adm4_foundation::Adm4Error::internal(format!("log serialize failed: {error}"))
        })?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| adm4_foundation::Adm4Error::io(format!("open log failed: {error}")))?;
        writeln!(file, "{line}")
            .map_err(|error| adm4_foundation::Adm4Error::io(format!("write log failed: {error}")))
    }

    pub fn tail(&self, limit: usize) -> Adm4Result<Vec<RunLogEntry>> {
        if !self.path.is_file() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&self.path)
            .map_err(|error| adm4_foundation::Adm4Error::io(format!("read log failed: {error}")))?;
        let mut entries: Vec<RunLogEntry> = text
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        if entries.len() > limit {
            entries = entries.split_off(entries.len() - limit);
        }
        Ok(entries)
    }
}
