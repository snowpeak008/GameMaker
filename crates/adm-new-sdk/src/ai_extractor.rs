use std::collections::BTreeMap;

use adm_new_ai::{CompletionAdapter, StructuredCompletionService};
use adm_new_contracts::sdk::{SdkReviewStatus, SdkSpec};
use adm_new_foundation::{AdmError, AdmResult};
use serde_json::Value;

use crate::knowledge_base::safe_sdk_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedSdkDocument {
    pub source_url: String,
    pub title: String,
    pub text: String,
}

pub fn extract_readable_text(html: &str) -> (String, String) {
    let cleaned = remove_tag_blocks(html, &["script", "style", "noscript", "svg"]);
    let title = extract_first_tag_text(&cleaned, "title").unwrap_or_default();
    let mut chunks = Vec::new();
    for tag in ["h1", "h2", "h3", "p", "li", "code"] {
        chunks.extend(extract_all_tag_text(&cleaned, tag));
    }
    (title, chunks.join("\n"))
}

pub fn build_extraction_prompt(document: &ExtractedSdkDocument) -> String {
    let mut text = document.text.clone();
    if text.len() > 12_000 {
        text.truncate(12_000);
    }
    format!(
        "Extract a concise SDK integration spec as JSON with keys: sdk_id, name, summary, integration_notes, api_requirements, risks.\n\nSource URL: {}\nTitle: {}\n\nDocument text:\n{}",
        document.source_url, document.title, text
    )
}

pub fn extract_sdk_spec_with_adapter<A>(
    document: &ExtractedSdkDocument,
    adapter: A,
) -> AdmResult<SdkSpec>
where
    A: CompletionAdapter,
{
    let service = StructuredCompletionService::with_max_retries(adapter, 1);
    let result = service.generate_json_contract("sdk_spec", &build_extraction_prompt(document));
    if !result.ok {
        return Err(AdmError::new(
            result
                .errors
                .join("; ")
                .trim()
                .to_string()
                .if_empty("SDK extraction failed"),
        ));
    }
    sdk_spec_from_completion_data(document, &result.data)
}

pub fn sdk_spec_from_completion_data(
    document: &ExtractedSdkDocument,
    data: &BTreeMap<String, Value>,
) -> AdmResult<SdkSpec> {
    let id_or_name = string_field(data, "sdk_id")
        .or_else(|| string_field(data, "name"))
        .unwrap_or_else(|| non_empty(&document.title, "sdk").to_string());
    let name = string_field(data, "name")
        .or_else(|| string_field(data, "sdk_id"))
        .unwrap_or_else(|| non_empty(&document.title, "SDK").to_string());
    Ok(SdkSpec {
        sdk_id: safe_sdk_id(&id_or_name),
        name,
        source_url: document.source_url.clone(),
        review_status: SdkReviewStatus::PendingReview,
        summary: string_field(data, "summary").unwrap_or_default(),
        integration_notes: string_list(data.get("integration_notes")),
        api_requirements: string_list(data.get("api_requirements")),
        risks: string_list(data.get("risks")),
        last_synced_at: String::new(),
        updated_at: String::new(),
    })
}

fn remove_tag_blocks(html: &str, tags: &[&str]) -> String {
    let mut output = html.to_string();
    for tag in tags {
        loop {
            let lower = output.to_ascii_lowercase();
            let Some(start) = lower.find(&format!("<{tag}")) else {
                break;
            };
            let Some(open_end) = lower[start..].find('>').map(|offset| start + offset) else {
                break;
            };
            let close_tag = format!("</{tag}>");
            let end = lower[open_end + 1..]
                .find(&close_tag)
                .map(|offset| open_end + 1 + offset + close_tag.len())
                .unwrap_or(open_end + 1);
            output.replace_range(start..end, "");
        }
    }
    output
}

fn extract_first_tag_text(html: &str, tag: &str) -> Option<String> {
    extract_all_tag_text(html, tag).into_iter().next()
}

fn extract_all_tag_text(html: &str, tag: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let open_prefix = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut cursor = 0usize;
    let mut chunks = Vec::new();
    while let Some(relative_start) = lower[cursor..].find(&open_prefix) {
        let start = cursor + relative_start;
        let Some(open_end) = lower[start..].find('>').map(|offset| start + offset) else {
            break;
        };
        let content_start = open_end + 1;
        let Some(relative_close) = lower[content_start..].find(&close) else {
            break;
        };
        let content_end = content_start + relative_close;
        let text = collapse_whitespace(&decode_entities(&strip_tags(
            &html[content_start..content_end],
        )));
        if !text.is_empty() {
            chunks.push(text);
        }
        cursor = content_end + close.len();
    }
    chunks
}

fn strip_tags(value: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    output
}

fn decode_entities(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn string_field(data: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    data.get(key).and_then(|value| match value {
        Value::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    })
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(text) => {
                    let text = text.trim();
                    (!text.is_empty()).then(|| text.to_string())
                }
                Value::Number(number) => Some(number.to_string()),
                _ => None,
            })
            .collect(),
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.is_empty() {
                Vec::new()
            } else {
                vec![text.to_string()]
            }
        }
        _ => Vec::new(),
    }
}

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value.trim()
    }
}

trait EmptyFallback {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.trim().is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_contracts::ai::{ModelResult, ModelResultStatus, ModelTask};

    #[test]
    fn readable_text_removes_scripts_and_keeps_content() {
        let (title, text) = extract_readable_text(
            "<html><head><title>SDK</title><script>bad()</script></head>\
             <body><h1>Ad SDK</h1><p>Initialize first.</p><li>Load rewarded ad.</li></body></html>",
        );

        assert_eq!(title, "SDK");
        assert!(text.contains("Ad SDK"));
        assert!(text.contains("Initialize first."));
        assert!(!text.contains("bad()"));
    }

    #[test]
    fn completion_data_maps_to_pending_review_spec() {
        let document = ExtractedSdkDocument {
            source_url: "https://example.test/sdk".to_string(),
            title: "Ad SDK".to_string(),
            text: "Initialize first.".to_string(),
        };
        let data = BTreeMap::from([
            ("sdk_id".to_string(), Value::String("Ad SDK".to_string())),
            ("name".to_string(), Value::String("Ad SDK".to_string())),
            (
                "summary".to_string(),
                Value::String("Rewarded ads.".to_string()),
            ),
            (
                "integration_notes".to_string(),
                Value::Array(vec![Value::String("Use adapter.".to_string())]),
            ),
            (
                "api_requirements".to_string(),
                Value::Array(vec![Value::String("Initialize.".to_string())]),
            ),
        ]);

        let spec = sdk_spec_from_completion_data(&document, &data).unwrap();

        assert_eq!(spec.sdk_id, "ad_sdk");
        assert_eq!(spec.review_status, SdkReviewStatus::PendingReview);
        assert_eq!(spec.source_url, "https://example.test/sdk");
        assert_eq!(spec.integration_notes, vec!["Use adapter."]);
    }

    #[test]
    fn adapter_extraction_uses_read_only_json_contract_prompt() {
        let document = ExtractedSdkDocument {
            source_url: "local.html".to_string(),
            title: "Ads".to_string(),
            text: "Initialize rewarded ads.".to_string(),
        };
        let spec = extract_sdk_spec_with_adapter(&document, MockAdapter).unwrap();

        assert_eq!(spec.sdk_id, "ads");
        assert_eq!(spec.review_status, SdkReviewStatus::PendingReview);
        assert_eq!(spec.api_requirements, vec!["Init before load"]);
    }

    struct MockAdapter;

    impl adm_new_ai::CompletionAdapter for MockAdapter {
        fn generate(&self, task: &ModelTask) -> AdmResult<ModelResult> {
            assert_eq!(task.sandbox, "read-only");
            assert!(task.prompt.contains("SDK integration spec"));
            assert!(task.prompt.contains("JSON"));
            Ok(ModelResult {
                task_id: task.task_id.clone(),
                status: ModelResultStatus::Succeeded,
                text: r#"{"sdk_id":"ads","name":"Ads","summary":"Ads SDK","api_requirements":["Init before load"]}"#.to_string(),
                errors: Vec::new(),
            })
        }
    }
}
