use adm_foundation::{AdmError, AdmResult, UtcTimestamp, fs as foundation_fs};
use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const RUN_LOG_RELATIVE_PATH: &str = "logs/run_log.jsonl";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLogEntry {
    pub timestamp_ms: u128,
    pub level: String,
    pub scope: String,
    pub message: String,
    pub context: String,
}

#[derive(Debug, Clone)]
pub struct RunLogService {
    log_path: PathBuf,
}

impl RunLogService {
    pub fn new(data_root: impl AsRef<Path>) -> Self {
        Self {
            log_path: data_root.as_ref().join(RUN_LOG_RELATIVE_PATH),
        }
    }

    pub fn path(&self) -> &Path {
        &self.log_path
    }

    pub fn append(
        &self,
        level: impl AsRef<str>,
        scope: impl AsRef<str>,
        message: impl AsRef<str>,
        context: impl AsRef<str>,
    ) -> AdmResult<RunLogEntry> {
        let entry = RunLogEntry {
            timestamp_ms: UtcTimestamp::now().as_millis(),
            level: required_inline("level", level.as_ref())?,
            scope: required_inline("scope", scope.as_ref())?,
            message: required_inline("message", message.as_ref())?,
            context: sanitize_inline(context.as_ref()),
        };
        let parent = self
            .log_path
            .parent()
            .ok_or_else(|| AdmError::invalid_input("run log path has no parent"))?;
        fs::create_dir_all(parent)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;
        file.write_all(entry.to_json_line().as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(entry)
    }

    pub fn entries(&self) -> AdmResult<Vec<RunLogEntry>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(&self.log_path)?;
        text.lines()
            .enumerate()
            .filter(|(_, line)| !line.trim().is_empty())
            .map(|(index, line)| parse_entry(line).with_context(format!("line={}", index + 1)))
            .collect()
    }

    pub fn filtered_entries(&self, filter: impl AsRef<str>) -> AdmResult<Vec<RunLogEntry>> {
        let filter = filter.as_ref().trim().to_ascii_lowercase();
        let all_entries = self.entries()?;
        if filter.is_empty() || filter == "all" || filter == "全部" {
            return Ok(all_entries);
        }
        Ok(all_entries
            .into_iter()
            .filter(|entry| entry.matches_filter(&filter))
            .collect())
    }

    pub fn render(&self, filter: impl AsRef<str>, limit: usize) -> AdmResult<String> {
        let entries = self.filtered_entries(filter.as_ref())?;
        let start = entries.len().saturating_sub(limit);
        let mut document = String::from("# Strict Run Log\n");
        document.push_str(&format!("log_file={}\n", self.log_path.display()));
        document.push_str(&format!("filter={}\n", sanitize_inline(filter.as_ref())));
        document.push_str(&format!("total_entries={}\n", entries.len()));
        document.push_str(&format!(
            "shown_entries={}\n",
            entries.len().saturating_sub(start)
        ));
        document.push_str("\n## Entries\n");
        for entry in entries.iter().skip(start) {
            document.push_str(&entry.render_line());
            document.push('\n');
        }
        Ok(document)
    }

    pub fn clear(&self) -> AdmResult<()> {
        foundation_fs::write_string(&self.log_path, "")
    }

    pub fn export_jsonl(&self, target: impl AsRef<Path>) -> AdmResult<PathBuf> {
        let target = target.as_ref();
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = if self.log_path.exists() {
            fs::read_to_string(&self.log_path)?
        } else {
            String::new()
        };
        foundation_fs::write_string(target, &text)?;
        Ok(target.to_path_buf())
    }
}

impl RunLogEntry {
    fn to_json_line(&self) -> String {
        json!({
            "timestamp_ms": self.timestamp_ms.to_string(),
            "level": self.level,
            "scope": self.scope,
            "message": self.message,
            "context": self.context,
        })
        .to_string()
    }

    fn render_line(&self) -> String {
        format!(
            "- timestamp_ms={}; level={}; scope={}; message={}; context={}",
            self.timestamp_ms,
            sanitize_inline(&self.level),
            sanitize_inline(&self.scope),
            sanitize_inline(&self.message),
            sanitize_inline(&self.context)
        )
    }

    fn matches_filter(&self, filter: &str) -> bool {
        self.level.to_ascii_lowercase().contains(filter)
            || self.scope.to_ascii_lowercase().contains(filter)
            || self.message.to_ascii_lowercase().contains(filter)
            || self.context.to_ascii_lowercase().contains(filter)
    }
}

fn parse_entry(line: &str) -> AdmResult<RunLogEntry> {
    let value = serde_json::from_str::<Value>(line)
        .map_err(|error| AdmError::validation(format!("invalid run log jsonl entry: {error}")))?;
    let timestamp_ms = value
        .get("timestamp_ms")
        .and_then(Value::as_str)
        .unwrap_or("0")
        .parse::<u128>()
        .map_err(|error| AdmError::validation(format!("invalid timestamp_ms: {error}")))?;
    Ok(RunLogEntry {
        timestamp_ms,
        level: value
            .get("level")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        scope: value
            .get("scope")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        message: value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        context: value
            .get("context")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

fn required_inline(name: &str, value: &str) -> AdmResult<String> {
    let value = sanitize_inline(value);
    if value.is_empty() {
        return Err(AdmError::invalid_input(format!(
            "run log {name} cannot be empty"
        )));
    }
    Ok(value)
}

fn sanitize_inline(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_string()
}

trait ResultContext<T> {
    fn with_context(self, context: String) -> AdmResult<T>;
}

impl<T> ResultContext<T> for AdmResult<T> {
    fn with_context(self, context: String) -> AdmResult<T> {
        self.map_err(|error| error.with_context(context))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_foundation::SessionId;

    #[test]
    fn run_log_appends_filters_and_exports_jsonl() {
        let root = std::env::temp_dir().join(format!(
            "adm_run_log_service_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let service = RunLogService::new(&root);

        service
            .append("INFO", "pipeline", "started", "archive=demo")
            .expect("append info");
        service
            .append("ERROR", "sdk", "approval failed", "id=missing")
            .expect("append error");

        let sdk_entries = service.filtered_entries("sdk").expect("filter");
        let rendered = service.render("ERROR", 10).expect("render");
        let exported = service
            .export_jsonl(root.join("exports").join("run_log.jsonl"))
            .expect("export");

        assert_eq!(sdk_entries.len(), 1);
        assert!(rendered.contains("approval failed"));
        assert!(exported.exists());

        service.clear().expect("clear");
        assert!(service.entries().expect("entries").is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}
