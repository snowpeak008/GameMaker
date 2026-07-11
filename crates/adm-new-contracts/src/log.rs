#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACT_FAMILY: &str = "log";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warning => "WARNING",
            Self::Error => "ERROR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    #[serde(default = "default_log_level")]
    pub level: LogLevel,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

fn default_log_level() -> LogLevel {
    LogLevel::Info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_entry_roundtrip_matches_jsonl_payload() {
        let entry = LogEntry {
            timestamp: "2026-07-08T00:00:00".to_string(),
            level: LogLevel::Warning,
            context: "package".to_string(),
            message: "blocked".to_string(),
            source: "package_panel".to_string(),
            metadata: BTreeMap::from([(
                "issue".to_string(),
                Value::String("changed_files_empty".to_string()),
            )]),
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"level\":\"WARNING\""));
        let restored: LogEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, entry);
    }

    #[test]
    fn log_rejects_invalid_level() {
        let invalid = r#"{"timestamp":"t","level":"WARN","context":"","message":""}"#;
        assert!(serde_json::from_str::<LogEntry>(invalid).is_err());
    }
}
