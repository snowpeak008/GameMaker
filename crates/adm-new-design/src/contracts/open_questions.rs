use adm_new_contracts::ArtifactLocale;
use serde_json::{Value, json};

use super::common::{first_str, get_str, list, now_iso};

pub fn build_open_questions_contract(
    project_identity: Option<&Value>,
    archetype_requirements: Option<&Value>,
    base_questions: &[Value],
    stage_id: &str,
) -> Value {
    build_open_questions_contract_with_locale(
        project_identity,
        archetype_requirements,
        base_questions,
        stage_id,
        ArtifactLocale::default(),
    )
}

pub fn build_open_questions_contract_with_locale(
    project_identity: Option<&Value>,
    archetype_requirements: Option<&Value>,
    base_questions: &[Value],
    stage_id: &str,
    artifact_locale: ArtifactLocale,
) -> Value {
    let identity = project_identity.unwrap_or(&Value::Null);
    let archetype = archetype_requirements.unwrap_or(&Value::Null);
    let mut questions = Vec::new();
    for (index, raw) in base_questions.iter().enumerate() {
        questions.push(normalize_question(
            raw,
            &format!("stage_{stage_id}.base"),
            index + 1,
            artifact_locale,
        ));
    }
    for (index, raw) in list(archetype, "open_questions").iter().enumerate() {
        questions.push(normalize_question(
            raw,
            &format!("stage_{stage_id}.archetype"),
            index + 1,
            artifact_locale,
        ));
    }
    let blocking_count = questions
        .iter()
        .filter(|item| {
            item.get("blocking")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && item.get("status").and_then(Value::as_str) != Some("resolved")
                && get_str(item, "answer").is_empty()
        })
        .count();
    let resolved_count = questions
        .iter()
        .filter(|item| {
            item.get("status").and_then(Value::as_str) == Some("resolved")
                || !get_str(item, "answer").is_empty()
        })
        .count();
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": artifact_locale,
        "stage_id": stage_id,
        "contract_display_name": if artifact_locale == ArtifactLocale::ZhCn { "待确认问题契约" } else { "Open Questions Contract" },
        "draft_session_id": get_str(identity, "draft_session_id"),
        "project_signature": get_str(identity, "project_signature"),
        "detected_archetype": get_str(archetype, "detected_archetype"),
        "questions": questions,
        "blocking_count": blocking_count,
        "resolved_count": resolved_count,
        "source_refs": identity.get("source_refs").cloned().unwrap_or_else(|| json!([])),
    })
}

pub fn unresolved_blocking_questions(contract: Option<&Value>) -> Vec<Value> {
    let Some(contract) = contract else {
        return Vec::new();
    };
    list(contract, "questions")
        .into_iter()
        .filter(|item| {
            item.get("blocking").and_then(Value::as_bool).unwrap_or(false)
                && item.get("status").and_then(Value::as_str) != Some("resolved")
                && get_str(item, "answer").is_empty()
        })
        .map(|item| {
            json!({
                "code": "BLOCKING_OQ_UNRESOLVED",
                "question_id": item.get("question_id").cloned().unwrap_or(Value::Null),
                "message": first_str(&item, &["prompt"]).if_empty("Blocking open question is unresolved.".to_string()),
                "required_by_step": "Step02",
            })
        })
        .collect()
}

fn normalize_question(
    raw: &Value,
    source: &str,
    index: usize,
    artifact_locale: ArtifactLocale,
) -> Value {
    if raw.is_object() {
        let blocking = raw
            .get("blocking")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let question_id =
            first_str(raw, &["question_id", "id"]).if_empty(format!("{source}_{index:02}"));
        let prompt = first_str(raw, &["prompt", "question", "message"]).if_empty(
            if artifact_locale == ArtifactLocale::ZhCn {
                format!("请确认问题 `{question_id}` 对应的项目决策。")
            } else {
                format!("Confirm the project decision for question `{question_id}`.")
            },
        );
        json!({
            "question_id": question_id,
            "prompt": prompt,
            "blocking": blocking,
            "priority": first_str(raw, &["priority"]).if_empty(if blocking { "P0" } else { "P1" }.to_string()),
            "source": source,
            "status": first_str(raw, &["status"]).if_empty("open".to_string()),
            "answer": raw.get("answer").cloned().unwrap_or_else(|| json!("")),
            "source_refs": raw.get("source_refs").cloned().unwrap_or_else(|| json!([])),
        })
    } else {
        json!({
            "question_id": format!("{source}_{index:02}"),
            "prompt": raw.as_str().map(ToString::to_string).unwrap_or_else(|| raw.to_string()),
            "blocking": false,
            "priority": "P1",
            "source": source,
            "status": "open",
            "answer": "",
            "source_refs": [],
        })
    }
}

trait IfEmpty {
    fn if_empty(self, fallback: String) -> String;
}

impl IfEmpty for String {
    fn if_empty(self, fallback: String) -> String {
        if self.trim().is_empty() {
            fallback
        } else {
            self
        }
    }
}
