use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use adm_new_contracts::ArtifactLocale;
use adm_new_contracts::pipeline::{PipelineStageResult, StageStatus};
use adm_new_foundation::AdmResult;
use adm_new_foundation::io::read_json;
use serde_json::{Value, json};

use crate::artifact_view::scan_stage_artifacts;

pub(crate) fn stage_result_from_generation(
    step_number: u32,
    generation: AdmResult<Value>,
    artifact_root: &Path,
    stage_dir: &Path,
) -> PipelineStageResult {
    let generation_error = generation.as_ref().err().map(ToString::to_string);
    let report = match &generation {
        Ok(report) => report.clone(),
        Err(_) => read_json(&stage_dir.join("validation_report.json"), json!({})),
    };
    let artifact_locale = report_artifact_locale(&report);
    let artifacts = scan_stage_artifacts(artifact_root, stage_dir);
    let mut outputs = outputs_from_report(&report);
    let mut errors = error_messages(&report);
    let mut warnings = warning_messages(&report);
    let artifact_scan_failed = artifacts.is_err();
    match artifacts {
        Ok(records) => {
            outputs.insert("artifact_records".to_string(), json!(records));
        }
        Err(error) => errors.push(if artifact_locale == ArtifactLocale::ZhCn {
            format!("检查阶段产物失败: {error}")
        } else {
            format!("failed to inspect stage artifacts: {error}")
        }),
    }
    if let Some(error) = generation_error {
        errors.insert(0, error);
    }
    deduplicate(&mut errors);
    deduplicate(&mut warnings);
    let status = if generation.is_err() || artifact_scan_failed {
        StageStatus::Failed
    } else {
        status_from_report(&report)
    };
    PipelineStageResult {
        message: report_message(&report, step_number, &status),
        status,
        outputs,
        errors,
        warnings,
    }
}

pub(crate) fn failed_stage_result_with_locale(
    step: u32,
    error: String,
    artifact_locale: ArtifactLocale,
) -> PipelineStageResult {
    PipelineStageResult {
        status: StageStatus::Failed,
        outputs: BTreeMap::from([
            ("artifact_records".to_string(), json!([])),
            ("artifact_locale".to_string(), json!(artifact_locale)),
        ]),
        errors: vec![error],
        warnings: Vec::new(),
        message: if artifact_locale == ArtifactLocale::ZhCn {
            format!("步骤 {step:02} 失败")
        } else {
            format!("Step{step:02} failed")
        },
    }
}

fn outputs_from_report(report: &Value) -> BTreeMap<String, Value> {
    let mut outputs = report
        .get("business_quality")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect::<BTreeMap<_, _>>();
    outputs.insert("validation_report".to_string(), report.clone());
    outputs
}

fn status_from_report(report: &Value) -> StageStatus {
    let status = report
        .get("status")
        .and_then(Value::as_str)
        .or_else(|| {
            report
                .get("business_quality")
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
        })
        .unwrap_or("failed");
    match status {
        "success" | "passed" => StageStatus::Success,
        "completed_with_review" => StageStatus::CompletedWithReview,
        "waiting_confirmation" => StageStatus::WaitingConfirmation,
        "stopped" => StageStatus::Stopped,
        "blocked" | "recovery_blocked" => StageStatus::Blocked,
        _ => StageStatus::Failed,
    }
}

fn error_messages(report: &Value) -> Vec<String> {
    messages_for_keys(
        report,
        &[
            "errors",
            "error",
            "source_error",
            "blocking_issues",
            "blockers",
            "missing_required_groups",
            "missing_upstream_artifacts",
        ],
    )
}

fn warning_messages(report: &Value) -> Vec<String> {
    let artifact_locale = report_artifact_locale(report);
    let mut warnings = messages_for_keys(
        report,
        &[
            "warnings",
            "review_items",
            "optional_missing_groups",
            "structured_source_warning",
        ],
    );
    let business = report.get("business_quality").unwrap_or(&Value::Null);
    let review_items_count = business
        .get("review_items_count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if review_items_count > 0 && warnings.is_empty() {
        warnings.push(if artifact_locale == ArtifactLocale::ZhCn {
            format!("阶段已完成，包含 {review_items_count} 个复核项，但生成器未提供详细复核信息")
        } else {
            format!(
                "stage completed with {review_items_count} review item(s); detailed review messages were not provided"
            )
        });
    }
    if let Some(status) = business
        .get("structured_input_status")
        .and_then(Value::as_str)
        .filter(|status| *status != "structured")
    {
        warnings.push(if artifact_locale == ArtifactLocale::ZhCn {
            format!("结构化输入状态: {status}")
        } else {
            format!("structured input status: {status}")
        });
    }
    warnings
}

fn messages_for_keys(report: &Value, keys: &[&str]) -> Vec<String> {
    let mut messages = Vec::new();
    for container in [
        report,
        report.get("business_quality").unwrap_or(&Value::Null),
    ] {
        for key in keys {
            if let Some(value) = container.get(*key) {
                append_messages(&mut messages, key, value);
            }
        }
    }
    messages
}

fn append_messages(messages: &mut Vec<String>, key: &str, value: &Value) {
    match value {
        Value::Null | Value::Bool(false) => {}
        Value::Number(number) if number.as_u64() == Some(0) => {}
        Value::String(text) if !text.trim().is_empty() => messages.push(text.clone()),
        Value::Array(items) => {
            for item in items {
                append_messages(messages, key, item);
            }
        }
        Value::Object(object) => {
            if let Some(message) = object.get("message").and_then(Value::as_str) {
                messages.push(message.to_string());
            } else {
                messages.push(Value::Object(object.clone()).to_string());
            }
        }
        other => messages.push(format!("{key}: {other}")),
    }
}

fn report_message(report: &Value, step: u32, status: &StageStatus) -> String {
    let artifact_locale = report_artifact_locale(report);
    report
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| {
            report
                .get("business_quality")
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
        })
        .map(str::to_string)
        .unwrap_or_else(|| {
            if artifact_locale == ArtifactLocale::ZhCn {
                format!("步骤 {step:02} {}", localized_stage_status(status))
            } else {
                format!("Step{step:02} {}", status.as_str())
            }
        })
}

fn report_artifact_locale(report: &Value) -> ArtifactLocale {
    ArtifactLocale::normalize(
        report
            .get("artifact_locale")
            .or_else(|| {
                report
                    .get("business_quality")
                    .and_then(|value| value.get("artifact_locale"))
            })
            .and_then(Value::as_str),
    )
}

fn localized_stage_status(status: &StageStatus) -> &'static str {
    match status {
        StageStatus::Success => "成功",
        StageStatus::Failed => "失败",
        StageStatus::Skipped => "已跳过",
        StageStatus::Blocked => "已阻断",
        StageStatus::Stopped => "已停止",
        StageStatus::WaitingConfirmation => "等待确认",
        StageStatus::CompletedWithReview => "已完成，需复核",
    }
}

fn deduplicate(messages: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    messages.retain(|message| seen.insert(message.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::{AdmError, new_stable_id};
    use std::fs;

    #[test]
    fn stage_result_preserves_generator_statuses_and_review_errors() {
        let root = temp_root("stage_result_status");
        let stage_dir = root.join("stage_08");
        fs::create_dir_all(&stage_dir).unwrap();

        for (raw_status, expected) in [
            ("success", StageStatus::Success),
            ("completed_with_review", StageStatus::CompletedWithReview),
            ("waiting_confirmation", StageStatus::WaitingConfirmation),
            ("stopped", StageStatus::Stopped),
            ("blocked", StageStatus::Blocked),
            ("recovery_blocked", StageStatus::Blocked),
        ] {
            let report = json!({
                "status": raw_status,
                "business_quality": {
                    "status": raw_status,
                    "blocking_issues": if raw_status == "completed_with_review" { 1 } else { 0 },
                    "content_exists": true
                }
            });
            let result = stage_result_from_generation(8, Ok(report), &root, &stage_dir);
            assert_eq!(result.status, expected);
            if raw_status == "completed_with_review" {
                assert!(!result.errors.is_empty());
            }
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage_result_preserves_operational_failure_message() {
        let root = temp_root("stage_result_failure");
        let stage_dir = root.join("stage_02");
        fs::create_dir_all(&stage_dir).unwrap();
        let result = stage_result_from_generation(
            2,
            Err(AdmError::new("source package failed validation")),
            &root,
            &stage_dir,
        );
        assert_eq!(result.status, StageStatus::Failed);
        assert!(result.errors[0].contains("source package failed validation"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage_result_fallback_warning_includes_review_count() {
        let root = temp_root("stage_result_review_count");
        let stage_dir = root.join("stage_02");
        fs::create_dir_all(&stage_dir).unwrap();
        let result = stage_result_from_generation(
            2,
            Ok(json!({
                "status": "completed_with_review",
                "business_quality": {
                    "status": "completed_with_review",
                    "review_items_count": 51,
                    "artifact_locale": "en-US"
                }
            })),
            &root,
            &stage_dir,
        );

        assert_eq!(result.status, StageStatus::CompletedWithReview);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("51 review item(s)"));
        assert!(result.warnings[0].contains("detailed review messages"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage_result_fallback_warning_uses_chinese_artifact_locale() {
        let root = temp_root("stage_result_review_count_zh");
        let stage_dir = root.join("stage_02");
        fs::create_dir_all(&stage_dir).unwrap();
        let result = stage_result_from_generation(
            2,
            Ok(json!({
                "status": "completed_with_review",
                "artifact_locale": "zh-CN",
                "business_quality": {
                    "status": "completed_with_review",
                    "review_items_count": 2,
                    "artifact_locale": "zh-CN"
                }
            })),
            &root,
            &stage_dir,
        );

        assert_eq!(result.status, StageStatus::CompletedWithReview);
        assert!(result.warnings[0].contains("2 个复核项"));
        assert!(result.warnings[0].contains("未提供详细复核信息"));
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(prefix: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(new_stable_id(prefix).unwrap());
        fs::create_dir_all(&root).unwrap();
        root
    }
}
