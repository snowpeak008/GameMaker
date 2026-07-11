use std::path::{Path, PathBuf};

use adm_new_contracts::ArtifactLocale;
use serde_json::{Value, json};

use super::common::{first_str, get_str, now_iso, selection_fingerprint, slug, stable_hash};

pub fn build_project_identity(
    parsed: &Value,
    out_dir: &Path,
    concept_profile: Option<&Value>,
    linked_save_id: Option<&str>,
) -> Value {
    build_project_identity_with_locale(
        parsed,
        out_dir,
        concept_profile,
        linked_save_id,
        ArtifactLocale::default(),
    )
}

pub fn build_project_identity_with_locale(
    parsed: &Value,
    out_dir: &Path,
    concept_profile: Option<&Value>,
    linked_save_id: Option<&str>,
    artifact_locale: ArtifactLocale,
) -> Value {
    let concept_profile = concept_profile.unwrap_or(&Value::Null);
    let output_base = output_base_from_stage_dir(out_dir);
    let draft_session_id = output_base
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_default();
    let project_name = project_name(parsed, concept_profile, artifact_locale);
    let decisions_hash = stable_hash(&json!({
        "source_sha256": get_str(parsed, "source_sha256"),
        "selections": selection_fingerprint(parsed),
    }));
    let template_hash = stable_hash(&json!({
        "source_package": get_str(parsed, "source_package"),
        "source_input_type": get_str(parsed, "source_input_type"),
        "concept_profile": concept_profile,
    }));
    let source_artifacts_path = first_str(parsed, &["source_package", "source"]);
    let signature_input = json!({
        "draft_session_id": draft_session_id,
        "output_base_dir": output_base.to_string_lossy().to_string(),
        "source_artifacts_path": source_artifacts_path,
        "decisions_hash": decisions_hash,
        "template_hash": template_hash,
        "project_name": project_name,
    });
    let project_signature = stable_hash(&signature_input);
    let project_id =
        slug(&first_str(concept_profile, &["project_id"]).if_empty(project_name.clone()));
    let mut blockers = Vec::new();
    for field in [
        "draft_session_id",
        "output_base_dir",
        "project_id",
        "project_name",
        "project_signature",
    ] {
        let present = match field {
            "draft_session_id" => !draft_session_id.is_empty(),
            "output_base_dir" => !output_base.as_os_str().is_empty(),
            "project_id" => !project_id.is_empty(),
            "project_name" => !project_name.is_empty(),
            "project_signature" => !project_signature.is_empty(),
            _ => true,
        };
        if !present {
            blockers.push(json!({
                "code": "PROJECT_IDENTITY_INCOMPLETE",
                "field": field,
                "message": if artifact_locale == ArtifactLocale::ZhCn {
                    format!("项目身份字段 `{field}` 不能为空。")
                } else {
                    format!("Project identity field `{field}` is required.")
                },
            }));
        }
    }
    let identity_terms = [
        project_name.clone(),
        first_str(concept_profile, &["genre", "genre_key"]),
        first_str(concept_profile, &["referenceGame", "reference_game"]),
    ]
    .into_iter()
    .filter(|item| !item.trim().is_empty())
    .map(Value::String)
    .collect::<Vec<_>>();
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": artifact_locale,
        "contract_display_name": if artifact_locale == ArtifactLocale::ZhCn { "项目身份契约" } else { "Project Identity Contract" },
        "draft_session_id": draft_session_id,
        "output_base_dir": output_base.to_string_lossy().to_string(),
        "linked_save_id": linked_save_id.unwrap_or(""),
        "project_id": project_id,
        "project_name": project_name,
        "project_signature": project_signature,
        "signature_input": signature_input,
        "decisions_hash": decisions_hash,
        "template_hash": template_hash,
        "source_artifacts_path": get_str(parsed, "source_package"),
        "source_refs": [get_str(parsed, "source")],
        "identity_terms": identity_terms,
        "excluded_template_terms": [],
        "blockers": blockers,
        "warnings": [],
    })
}

pub fn build_customization_score_report(
    stage_id: &str,
    project_identity: Option<&Value>,
    status: &str,
    blockers: &[Value],
    warnings: &[Value],
    scores: Option<Value>,
) -> Value {
    build_customization_score_report_with_locale(
        stage_id,
        project_identity,
        status,
        blockers,
        warnings,
        scores,
        ArtifactLocale::default(),
    )
}

pub fn build_customization_score_report_with_locale(
    stage_id: &str,
    project_identity: Option<&Value>,
    status: &str,
    blockers: &[Value],
    warnings: &[Value],
    scores: Option<Value>,
    artifact_locale: ArtifactLocale,
) -> Value {
    let identity = project_identity.unwrap_or(&Value::Null);
    let status = if blockers.is_empty() {
        status.to_string()
    } else {
        "blocked".to_string()
    };
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": artifact_locale,
        "stage_id": stage_id,
        "stage_display_name": stage_display_name(stage_id, artifact_locale),
        "report_display_name": if artifact_locale == ArtifactLocale::ZhCn { "项目定制化评分报告" } else { "Customization Score Report" },
        "project_signature": get_str(identity, "project_signature"),
        "status": status,
        "scores": scores.unwrap_or_else(|| json!({
            "project_signature_present": if get_str(identity, "project_signature").is_empty() { 0.0 } else { 1.0 },
            "project_name_present": if get_str(identity, "project_name").is_empty() { 0.0 } else { 1.0 },
            "template_leakage_count": 0,
        })),
        "generic_content_ratio": 0.0,
        "project_specificity_score": if get_str(identity, "project_signature").is_empty() { 0.0 } else { 1.0 },
        "template_leakage_count": 0,
        "blockers": blockers,
        "warnings": warnings,
    })
}

fn output_base_from_stage_dir(out_dir: &Path) -> PathBuf {
    let parent = out_dir.parent();
    let grandparent = parent.and_then(Path::parent);
    if parent.and_then(Path::file_name).and_then(|v| v.to_str()) == Some("artifacts")
        && grandparent
            .and_then(Path::file_name)
            .and_then(|v| v.to_str())
            == Some("outputs")
    {
        grandparent
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| out_dir.parent().unwrap_or(out_dir).to_path_buf())
    } else {
        out_dir.parent().unwrap_or(out_dir).to_path_buf()
    }
}

fn project_name(
    parsed: &Value,
    concept_profile: &Value,
    artifact_locale: ArtifactLocale,
) -> String {
    for source in [concept_profile, parsed] {
        let name = first_str(
            source,
            &[
                "project_name",
                "game_title",
                "display_name",
                "project",
                "title",
                "name",
            ],
        );
        if !name.is_empty() {
            return name;
        }
    }
    let fallback = first_str(
        concept_profile,
        &["project_id", "referenceGame", "reference_game"],
    );
    if !fallback.is_empty() {
        return fallback;
    }
    let source = get_str(parsed, "source");
    if !source.is_empty() {
        return Path::new(&source)
            .file_stem()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or(source);
    }
    if artifact_locale == ArtifactLocale::ZhCn {
        "未命名游戏项目".to_string()
    } else {
        "Untitled Game Project".to_string()
    }
}

fn stage_display_name(stage_id: &str, artifact_locale: ArtifactLocale) -> &'static str {
    match (stage_id, artifact_locale) {
        ("00", ArtifactLocale::ZhCn) => "步骤 00：创意收集",
        ("01", ArtifactLocale::ZhCn) => "步骤 01：玩法框架",
        ("02", ArtifactLocale::ZhCn) => "步骤 02：设计冻结",
        ("00", ArtifactLocale::EnUs) => "Step 00: Idea Intake",
        ("01", ArtifactLocale::EnUs) => "Step 01: Gameplay Framework",
        ("02", ArtifactLocale::EnUs) => "Step 02: Design Freeze",
        (_, ArtifactLocale::ZhCn) => "流水线步骤",
        (_, ArtifactLocale::EnUs) => "Pipeline Stage",
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
