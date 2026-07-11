use adm_new_foundation::{AdmError, AdmResult};
use serde_json::Value;

pub const AI_RESPONSE_SCHEMA_VERSION: &str = "1.0";
pub const MODE_ENUM: &[&str] = &[
    "question_group",
    "confirmation",
    "readiness_check",
    "full_project_output",
    "partial_project_output",
    "maintenance",
    "error",
];
pub const TURN_MODE_ENUM: &[&str] = &[
    "question_group",
    "confirmation",
    "readiness_check",
    "maintenance",
    "error",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiResponseSchemaProfile {
    pub schema_name: &'static str,
    pub title: &'static str,
    pub allowed_modes: &'static [&'static str],
    pub required_fields: &'static [&'static str],
}

pub fn ai_response_schema_names() -> &'static [&'static str] {
    &[
        "turn",
        "readiness",
        "full_output",
        "partial_output",
        "mapping",
        "summary",
    ]
}

pub fn ai_response_schema_profile(schema_name: &str) -> AdmResult<AiResponseSchemaProfile> {
    match schema_name {
        "turn" => Ok(AiResponseSchemaProfile {
            schema_name: "turn",
            title: "Commercial game design AI interview turn response",
            allowed_modes: TURN_MODE_ENUM,
            required_fields: &[
                "schemaVersion",
                "mode",
                "assistantMessage",
                "routeOverview",
                "questionGroup",
                "readinessCheck",
                "inferences",
            ],
        }),
        "readiness" => Ok(AiResponseSchemaProfile {
            schema_name: "readiness",
            title: "Commercial game design AI interview readiness response",
            allowed_modes: &["readiness_check", "maintenance", "error"],
            required_fields: &[
                "schemaVersion",
                "mode",
                "assistantMessage",
                "routeOverview",
                "readinessCheck",
                "inferences",
            ],
        }),
        "full_output" => Ok(AiResponseSchemaProfile {
            schema_name: "full_output",
            title: "Commercial game design AI interview full output response",
            allowed_modes: MODE_ENUM,
            required_fields: &[
                "schemaVersion",
                "mode",
                "assistantMessage",
                "routeOverview",
                "fullProjectOutput",
                "optionDifferences",
                "inferences",
            ],
        }),
        "partial_output" => Ok(AiResponseSchemaProfile {
            schema_name: "partial_output",
            title: "Commercial game design AI interview partial output response",
            allowed_modes: &["partial_project_output", "maintenance", "error"],
            required_fields: &[
                "schemaVersion",
                "mode",
                "assistantMessage",
                "routeOverview",
                "partialProjectOutput",
                "inferences",
            ],
        }),
        "mapping" => Ok(AiResponseSchemaProfile {
            schema_name: "mapping",
            title: "Commercial game design AI interview background mapping response",
            allowed_modes: &["mapping", "maintenance", "error"],
            required_fields: &["schemaVersion", "mode", "assistantMessage", "inferences"],
        }),
        "summary" => Ok(AiResponseSchemaProfile {
            schema_name: "summary",
            title: "Commercial game design AI interview summary correction response",
            allowed_modes: &["summary_correction", "maintenance", "error"],
            required_fields: &["schemaVersion", "mode", "summary"],
        }),
        other => Err(AdmError::new(format!(
            "unknown AI response schema: {other}"
        ))),
    }
}

pub fn validate_ai_response_schema_shape(schema_name: &str, payload: &Value) -> AdmResult<()> {
    let profile = ai_response_schema_profile(schema_name)?;
    let object = payload
        .as_object()
        .ok_or_else(|| AdmError::new("AI response payload must be a JSON object"))?;
    for field in profile.required_fields {
        if !object.contains_key(*field) {
            return Err(AdmError::new(format!(
                "{} response is missing required field {field}",
                profile.schema_name
            )));
        }
    }
    let mode = object
        .get("mode")
        .and_then(Value::as_str)
        .ok_or_else(|| AdmError::new("AI response mode must be a string"))?;
    if !profile.allowed_modes.iter().any(|allowed| allowed == &mode) {
        return Err(AdmError::new(format!(
            "{} response mode is not allowed: {mode}",
            profile.schema_name
        )));
    }
    let schema_version = object
        .get("schemaVersion")
        .and_then(Value::as_str)
        .unwrap_or("");
    if schema_version != AI_RESPONSE_SCHEMA_VERSION {
        return Err(AdmError::new(format!(
            "unsupported AI response schemaVersion: {schema_version}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn schema_profiles_match_python_ai_schema_modes() {
        assert_eq!(
            ai_response_schema_names(),
            &[
                "turn",
                "readiness",
                "full_output",
                "partial_output",
                "mapping",
                "summary"
            ]
        );
        let turn = ai_response_schema_profile("turn").unwrap();
        assert!(turn.allowed_modes.contains(&"question_group"));
        assert!(!turn.allowed_modes.contains(&"full_project_output"));
        let full = ai_response_schema_profile("full_output").unwrap();
        assert!(full.required_fields.contains(&"optionDifferences"));
        assert!(full.allowed_modes.contains(&"partial_project_output"));
    }

    #[test]
    fn schema_shape_validator_checks_required_fields_mode_and_version() {
        let payload = json!({
            "schemaVersion": "1.0",
            "mode": "question_group",
            "assistantMessage": "Ask",
            "routeOverview": {},
            "questionGroup": null,
            "readinessCheck": null,
            "inferences": [],
        });
        assert!(validate_ai_response_schema_shape("turn", &payload).is_ok());

        let mut wrong_mode = payload.clone();
        wrong_mode["mode"] = json!("full_project_output");
        assert!(
            validate_ai_response_schema_shape("turn", &wrong_mode)
                .unwrap_err()
                .message()
                .contains("not allowed")
        );

        let missing = json!({"schemaVersion": "1.0", "mode": "mapping"});
        assert!(
            validate_ai_response_schema_shape("mapping", &missing)
                .unwrap_err()
                .message()
                .contains("assistantMessage")
        );

        let bad_version = json!({
            "schemaVersion": "0.9",
            "mode": "summary_correction",
            "summary": {},
        });
        assert!(
            validate_ai_response_schema_shape("summary", &bad_version)
                .unwrap_err()
                .message()
                .contains("unsupported")
        );
    }
}
