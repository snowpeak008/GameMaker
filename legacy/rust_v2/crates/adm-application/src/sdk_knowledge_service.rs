use adm_foundation::{AdmError, AdmResult, UtcTimestamp, fs as foundation_fs};
use adm_sdk::SdkKnowledgeBase;
use std::fs;
use std::path::{Path, PathBuf};

pub const SDK_REVIEW_RELATIVE_PATH: &str = "sdk/review_queue.adm";
pub const SDK_APPROVED_CONTEXT_RELATIVE_PATH: &str = "sdk/approved_prompt_context.adm";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkReviewStatus {
    Pending,
    Approved,
    Rejected,
}

impl SdkReviewStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    pub fn label_zh(self) -> &'static str {
        match self {
            Self::Pending => "待审核",
            Self::Approved => "已批准",
            Self::Rejected => "已拒绝",
        }
    }

    fn parse(value: &str) -> AdmResult<Self> {
        match value.trim() {
            "pending" | "待审核" => Ok(Self::Pending),
            "approved" | "已批准" => Ok(Self::Approved),
            "rejected" | "已拒绝" => Ok(Self::Rejected),
            other => Err(AdmError::validation(format!(
                "unsupported SDK review status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkReviewRecord {
    pub id: String,
    pub sdk_name: String,
    pub url: String,
    pub status: SdkReviewStatus,
    pub category: String,
    pub target_engines: String,
    pub target_platforms: String,
    pub purpose: String,
    pub note: String,
    pub updated_at_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SdkKnowledgeSnapshot {
    pub records: Vec<SdkReviewRecord>,
    pub pending_count: usize,
    pub approved_count: usize,
    pub rejected_count: usize,
    pub approved_prompt_context: String,
}

#[derive(Debug, Clone)]
pub struct SdkKnowledgeService {
    data_root: PathBuf,
}

impl SdkKnowledgeService {
    pub fn new(data_root: impl AsRef<Path>) -> Self {
        Self {
            data_root: data_root.as_ref().to_path_buf(),
        }
    }

    pub fn review_path(&self) -> PathBuf {
        self.data_root.join(SDK_REVIEW_RELATIVE_PATH)
    }

    pub fn approved_context_path(&self) -> PathBuf {
        self.data_root.join(SDK_APPROVED_CONTEXT_RELATIVE_PATH)
    }

    pub fn snapshot(&self) -> AdmResult<SdkKnowledgeSnapshot> {
        let records = self.load_records()?;
        let pending_count = records
            .iter()
            .filter(|record| record.status == SdkReviewStatus::Pending)
            .count();
        let approved_count = records
            .iter()
            .filter(|record| record.status == SdkReviewStatus::Approved)
            .count();
        let rejected_count = records
            .iter()
            .filter(|record| record.status == SdkReviewStatus::Rejected)
            .count();
        let approved_prompt_context = self.render_approved_prompt_context_from_records(&records);
        Ok(SdkKnowledgeSnapshot {
            records,
            pending_count,
            approved_count,
            rejected_count,
            approved_prompt_context,
        })
    }

    pub fn add_pending(
        &self,
        sdk_name: impl AsRef<str>,
        url: impl AsRef<str>,
    ) -> AdmResult<SdkReviewRecord> {
        let sdk_name = required_inline("sdk_name", sdk_name.as_ref())?;
        let url = required_inline("url", url.as_ref())?;
        let mut records = self.load_records()?;
        let id = format!(
            "sdk_{}_{}",
            UtcTimestamp::now().as_millis(),
            sanitize_identifier(&sdk_name)
        );
        let record = SdkReviewRecord {
            id,
            sdk_name,
            url,
            status: SdkReviewStatus::Pending,
            category: "custom".to_string(),
            target_engines: "Unity".to_string(),
            target_platforms: "windows-desktop".to_string(),
            purpose: "用户提交的 SDK 候选项，批准后进入 AI prompt context。".to_string(),
            note: "等待人工审核。".to_string(),
            updated_at_ms: UtcTimestamp::now().as_millis(),
        };
        records.push(record.clone());
        self.save_records(&records)?;
        self.write_approved_prompt_context(&records)?;
        Ok(record)
    }

    pub fn approve(&self, id: impl AsRef<str>) -> AdmResult<SdkReviewRecord> {
        self.set_status(
            id.as_ref(),
            SdkReviewStatus::Approved,
            "已批准进入 prompt context。",
        )
    }

    pub fn reject(&self, id: impl AsRef<str>) -> AdmResult<SdkReviewRecord> {
        self.set_status(
            id.as_ref(),
            SdkReviewStatus::Rejected,
            "已拒绝，不进入 prompt context。",
        )
    }

    pub fn mark_pending(&self, id: impl AsRef<str>) -> AdmResult<SdkReviewRecord> {
        self.set_status(
            id.as_ref(),
            SdkReviewStatus::Pending,
            "已重新标记为待审核。",
        )
    }

    pub fn render_summary(&self) -> AdmResult<String> {
        let snapshot = self.snapshot()?;
        Ok(format!(
            "SDK 审批队列：待审核={}，已批准={}，已拒绝={}，自定义记录={}。\napproved_context_file={}",
            snapshot.pending_count,
            snapshot.approved_count,
            snapshot.rejected_count,
            snapshot.records.len(),
            self.approved_context_path().display()
        ))
    }

    fn set_status(
        &self,
        id: &str,
        status: SdkReviewStatus,
        note: &str,
    ) -> AdmResult<SdkReviewRecord> {
        let id = required_inline("id", id)?;
        let mut records = self.load_records()?;
        let mut updated = None;
        for record in &mut records {
            if record.id == id {
                record.status = status;
                record.note = note.to_string();
                record.updated_at_ms = UtcTimestamp::now().as_millis();
                updated = Some(record.clone());
                break;
            }
        }
        let updated = updated.ok_or_else(|| {
            AdmError::new(
                adm_foundation::AdmErrorKind::NotFound,
                format!("SDK review record not found: {id}"),
            )
        })?;
        self.save_records(&records)?;
        self.write_approved_prompt_context(&records)?;
        Ok(updated)
    }

    fn load_records(&self) -> AdmResult<Vec<SdkReviewRecord>> {
        let path = self.review_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = fs::read_to_string(path)?;
        text.lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let line = line.trim();
                if line.starts_with("- ") {
                    Some((index, line.trim_start_matches("- ").trim()))
                } else {
                    None
                }
            })
            .map(|(index, line)| parse_record(line).with_context(format!("line={}", index + 1)))
            .collect()
    }

    fn save_records(&self, records: &[SdkReviewRecord]) -> AdmResult<()> {
        let mut document = String::from("# SDK Review Queue\n");
        document.push_str(&format!("record_count={}\n", records.len()));
        document.push_str("\n## Records\n");
        for record in records {
            document.push_str(&record.render_line());
            document.push('\n');
        }
        foundation_fs::write_string(self.review_path(), &document)
    }

    fn write_approved_prompt_context(&self, records: &[SdkReviewRecord]) -> AdmResult<()> {
        foundation_fs::write_string(
            self.approved_context_path(),
            &self.render_approved_prompt_context_from_records(records),
        )
    }

    fn render_approved_prompt_context_from_records(&self, records: &[SdkReviewRecord]) -> String {
        let mut document = String::from("# Approved SDK Prompt Context\n");
        document.push_str("source=builtin_catalog_and_approved_custom_queue\n");
        document.push_str("\n## Built-in SDK Resources\n");
        document.push_str(&SdkKnowledgeBase::default_game_pipeline().render());
        document.push_str("\n## Approved Custom SDK Resources\n");
        let mut approved = 0usize;
        for record in records
            .iter()
            .filter(|record| record.status == SdkReviewStatus::Approved)
        {
            approved += 1;
            document.push_str(&format!(
                "- id={}; sdk_name={}; url={}; category={}; target_engines={}; target_platforms={}; purpose={}\n",
                sanitize_inline(&record.id),
                sanitize_inline(&record.sdk_name),
                sanitize_inline(&record.url),
                sanitize_inline(&record.category),
                sanitize_inline(&record.target_engines),
                sanitize_inline(&record.target_platforms),
                sanitize_inline(&record.purpose)
            ));
        }
        document.push_str(&format!("approved_custom_count={approved}\n"));
        document
    }
}

impl SdkReviewRecord {
    fn render_line(&self) -> String {
        format!(
            "- id={}; sdk_name={}; url={}; status={}; category={}; target_engines={}; target_platforms={}; purpose={}; note={}; updated_at_ms={}",
            sanitize_inline(&self.id),
            sanitize_inline(&self.sdk_name),
            sanitize_inline(&self.url),
            self.status.as_str(),
            sanitize_inline(&self.category),
            sanitize_inline(&self.target_engines),
            sanitize_inline(&self.target_platforms),
            sanitize_inline(&self.purpose),
            sanitize_inline(&self.note),
            self.updated_at_ms
        )
    }
}

fn parse_record(line: &str) -> AdmResult<SdkReviewRecord> {
    let fields = parse_fields(line);
    Ok(SdkReviewRecord {
        id: required_field(&fields, "id")?,
        sdk_name: required_field(&fields, "sdk_name")?,
        url: required_field(&fields, "url")?,
        status: SdkReviewStatus::parse(&required_field(&fields, "status")?)?,
        category: fields
            .get("category")
            .cloned()
            .unwrap_or_else(|| "custom".to_string()),
        target_engines: fields
            .get("target_engines")
            .cloned()
            .unwrap_or_else(|| "Unity".to_string()),
        target_platforms: fields
            .get("target_platforms")
            .cloned()
            .unwrap_or_else(|| "windows-desktop".to_string()),
        purpose: fields.get("purpose").cloned().unwrap_or_default(),
        note: fields.get("note").cloned().unwrap_or_default(),
        updated_at_ms: fields
            .get("updated_at_ms")
            .map(|value| {
                value.parse::<u128>().map_err(|error| {
                    AdmError::validation(format!("invalid SDK review updated_at_ms: {error}"))
                })
            })
            .transpose()?
            .unwrap_or_default(),
    })
}

fn parse_fields(line: &str) -> std::collections::BTreeMap<String, String> {
    let mut fields = std::collections::BTreeMap::new();
    for part in line.split(';') {
        let Some((key, value)) = part.trim().split_once('=') else {
            continue;
        };
        fields.insert(key.trim().to_string(), value.trim().to_string());
    }
    fields
}

fn required_field(
    fields: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> AdmResult<String> {
    fields
        .get(key)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| AdmError::validation(format!("SDK review record missing {key}")))
}

fn required_inline(name: &str, value: &str) -> AdmResult<String> {
    let value = sanitize_inline(value);
    if value.is_empty() {
        return Err(AdmError::invalid_input(format!("{name} cannot be empty")));
    }
    Ok(value)
}

fn sanitize_inline(value: &str) -> String {
    value.replace(['\r', '\n', ';'], " ").trim().to_string()
}

fn sanitize_identifier(value: &str) -> String {
    let value = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if value.is_empty() {
        "custom".to_string()
    } else {
        value
    }
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
    fn sdk_service_adds_reviews_and_writes_approved_context() {
        let root = std::env::temp_dir().join(format!(
            "adm_sdk_knowledge_service_{}_{}",
            std::process::id(),
            SessionId::generate()
        ));
        let service = SdkKnowledgeService::new(&root);

        let record = service
            .add_pending("DOTS Physics", "https://example.invalid/dots")
            .expect("add");
        let pending = service.snapshot().expect("snapshot");
        assert_eq!(pending.pending_count, 1);
        assert_eq!(pending.approved_count, 0);

        service.approve(&record.id).expect("approve");
        let approved = service.snapshot().expect("approved snapshot");
        assert_eq!(approved.pending_count, 0);
        assert_eq!(approved.approved_count, 1);
        assert!(approved.approved_prompt_context.contains("DOTS Physics"));
        assert!(service.approved_context_path().exists());

        service.reject(&record.id).expect("reject");
        let rejected = service.snapshot().expect("rejected snapshot");
        assert_eq!(rejected.rejected_count, 1);
        assert!(!rejected.approved_prompt_context.contains("DOTS Physics"));
        let _ = std::fs::remove_dir_all(root);
    }
}
