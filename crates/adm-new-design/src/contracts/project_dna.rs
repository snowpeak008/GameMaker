use adm_new_contracts::ArtifactLocale;
use serde_json::{Value, json};

use super::common::{get_str, is_empty_value, list, now_iso, selection_items};

pub fn build_project_dna_seed(
    project_identity: &Value,
    concept_profile: &Value,
    parsed: &Value,
    archetype_requirements: &Value,
    open_questions: Option<&Value>,
) -> Value {
    build_project_dna_seed_with_locale(
        project_identity,
        concept_profile,
        parsed,
        archetype_requirements,
        open_questions,
        ArtifactLocale::default(),
    )
}

pub fn build_project_dna_seed_with_locale(
    project_identity: &Value,
    concept_profile: &Value,
    parsed: &Value,
    archetype_requirements: &Value,
    open_questions: Option<&Value>,
    artifact_locale: ArtifactLocale,
) -> Value {
    let selections = selection_items(parsed);
    let mut source_refs = list(project_identity, "source_refs");
    let source = get_str(parsed, "source");
    if !source.is_empty()
        && !source_refs
            .iter()
            .any(|item| item == &Value::String(source.clone()))
    {
        source_refs.push(Value::String(source));
    }
    let runtime_systems = list(archetype_requirements, "required_systems").if_empty(vec![json!({
        "system_id": "core_loop_runtime",
        "description": if artifact_locale == ArtifactLocale::ZhCn { "依据已确认的设计选择运行首个可玩循环。" } else { "Runs the first playable loop from explicit design selections." },
    })]);
    let core_entities = list(archetype_requirements, "required_entities").if_empty(vec![
        json!({"entity_id": "player_actor", "role": "player"}),
        json!({"entity_id": "playable_goal", "role": "objective"}),
    ]);
    let player_actions = list(archetype_requirements, "required_player_actions").if_empty(vec![json!({
        "action_id": "perform_core_action",
        "state_change": "core_loop_state_changes",
        "source_selection": selections.first().and_then(|item| item.get("selection_id")).cloned().unwrap_or_else(|| json!("")),
    })]);
    let objectives = list(archetype_requirements, "required_objectives").if_empty(vec![json!({
        "objective_id": "complete_first_playable_goal",
        "completion_condition": "player_reaches_success_state",
        "failure_condition": "player_reaches_failure_state",
    })]);
    let asset_needs = list(archetype_requirements, "minimum_playable_assets")
        .if_empty(list(archetype_requirements, "required_assets"))
        .if_empty(vec![
            json!({"asset_role": "player_or_cursor_visual", "consumer": "core_loop_runtime"}),
            json!({"asset_role": "goal_feedback_visual", "consumer": "hud_feedback"}),
        ]);
    let acceptance_scenarios = list(archetype_requirements, "acceptance_scenarios").if_empty(vec![json!({
        "scenario_id": "first_playable_flow",
        "expected": if artifact_locale == ArtifactLocale::ZhCn { "玩家执行核心动作、看到明确反馈，并到达清晰的完成或失败状态。" } else { "Player performs the core action, sees feedback, and reaches a clear completion or failure state." },
    })]);
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": artifact_locale,
        "contract_display_name": if artifact_locale == ArtifactLocale::ZhCn { "项目 DNA 种子契约" } else { "Project DNA Seed Contract" },
        "contract_state": "seed",
        "draft_session_id": get_str(project_identity, "draft_session_id"),
        "project_signature": get_str(project_identity, "project_signature"),
        "project_id": get_str(project_identity, "project_id"),
        "project_name": get_str(project_identity, "project_name"),
        "detected_archetype": get_str(archetype_requirements, "detected_archetype"),
        "source_identity_contract": "stage_00/project_identity_contract.json",
        "core_loop": concept_profile.get("core_loop").cloned().unwrap_or_else(|| json!({
            "summary": if artifact_locale == ArtifactLocale::ZhCn { "依据已确认的设计选择生成。" } else { "Derived from explicit design selections." },
            "selections": selections.iter().take(5).cloned().collect::<Vec<_>>(),
        })),
        "core_entities": core_entities,
        "resources": list(archetype_requirements, "required_resources"),
        "player_actions": player_actions,
        "objectives": objectives,
        "ui_surfaces": [{
            "surface_id": "hud",
            "purpose": if artifact_locale == ArtifactLocale::ZhCn { "显示目标、资源和操作反馈。" } else { "Displays objectives, resources, and action feedback." }
        }],
        "runtime_systems": runtime_systems,
        "asset_needs": asset_needs,
        "acceptance_scenarios": acceptance_scenarios,
        "explicit_design_fragments": selections,
        "unresolved_questions": open_questions.and_then(|value| value.get("questions")).cloned().unwrap_or_else(|| json!([])),
        "source_refs": source_refs.into_iter().filter(|item| !item.as_str().unwrap_or("").is_empty()).collect::<Vec<_>>(),
    })
}

pub fn validate_project_dna_contract(contract: &Value) -> Vec<Value> {
    let mut blockers = Vec::new();
    if get_str(contract, "contract_state") != "frozen" {
        blockers.push(json!({
            "code": "CORE_ENTITY_UNFROZEN",
            "field": "contract_state",
            "message": "Project DNA must be frozen before Step03+ consumption.",
        }));
    }
    for field in [
        "project_signature",
        "core_entities",
        "runtime_systems",
        "player_actions",
        "objectives",
        "asset_needs",
        "acceptance_scenarios",
    ] {
        if is_empty_value(contract.get(field)) {
            blockers.push(json!({
                "code": "NULL_CONTRACT_FIELD",
                "field": field,
                "message": format!("Frozen Project DNA field `{field}` must not be empty."),
            }));
        }
    }
    blockers
}

pub fn freeze_project_dna(
    seed: &Value,
    archetype_requirements: &Value,
    playable_bundle: &Value,
    open_question_blockers: &[Value],
) -> (Value, Vec<Value>) {
    freeze_project_dna_with_locale(
        seed,
        archetype_requirements,
        playable_bundle,
        open_question_blockers,
        ArtifactLocale::default(),
    )
}

pub fn freeze_project_dna_with_locale(
    seed: &Value,
    archetype_requirements: &Value,
    playable_bundle: &Value,
    open_question_blockers: &[Value],
    artifact_locale: ArtifactLocale,
) -> (Value, Vec<Value>) {
    let mut contract = seed.clone();
    contract["schema_version"] = json!("1.0");
    contract["generated_at"] = json!(now_iso());
    contract["artifact_locale"] = json!(artifact_locale);
    contract["contract_display_name"] = json!(if artifact_locale == ArtifactLocale::ZhCn {
        "已冻结项目 DNA 契约"
    } else {
        "Frozen Project DNA Contract"
    });
    contract["contract_state"] = json!("frozen");
    contract["source_project_dna_seed"] = json!("stage_00/project_dna_seed.json");
    contract["source_contracts"] = json!([
        "stage_00/project_dna_seed.json",
        "stage_01/archetype_requirements.json",
        "stage_02/playable_contracts/core_playable_contract.json",
        "stage_02/playable_contracts/demo_flow_contract.json"
    ]);
    fill_if_empty(
        &mut contract,
        "core_entities",
        list(archetype_requirements, "required_entities"),
    );
    fill_if_empty(
        &mut contract,
        "runtime_systems",
        list(archetype_requirements, "required_systems"),
    );
    fill_if_empty(
        &mut contract,
        "player_actions",
        list(archetype_requirements, "required_player_actions"),
    );
    fill_if_empty(
        &mut contract,
        "objectives",
        list(archetype_requirements, "required_objectives"),
    );
    fill_if_empty(
        &mut contract,
        "asset_needs",
        list(archetype_requirements, "required_assets"),
    );
    fill_if_empty(
        &mut contract,
        "acceptance_scenarios",
        list(archetype_requirements, "acceptance_scenarios"),
    );

    let mut refs = serde_json::Map::new();
    for key in [
        "core_playable_contract",
        "demo_flow_contract",
        "runtime_data_contract",
        "ui_flow_contract",
        "scene_bootstrap_contract",
        "asset_mount_contract",
        "playable_acceptance_contract",
    ] {
        if playable_bundle.get(key).is_some() && !is_empty_value(playable_bundle.get(key)) {
            refs.insert(
                key.to_string(),
                Value::String(format!("stage_02/playable_contracts/{key}.json")),
            );
        }
    }
    contract["playable_contract_refs"] = Value::Object(refs);
    let mut blockers = open_question_blockers.to_vec();
    if is_empty_value(playable_bundle.get("demo_flow_contract")) {
        blockers.push(json!({
            "code": "PLAYABLE_SCENARIO_MISSING",
            "message": if artifact_locale == ArtifactLocale::ZhCn {
                "冻结项目 DNA 前，步骤 02 必须提供演示流程可玩契约。"
            } else {
                "Step02 requires a demo flow playable contract before freezing Project DNA."
            },
        }));
    }
    blockers.extend(validate_project_dna_contract(&contract));
    localize_validation_blockers(&mut blockers, artifact_locale);
    contract["blockers"] = Value::Array(blockers.clone());
    contract["status"] = json!(if blockers.is_empty() {
        "frozen"
    } else {
        "blocked"
    });
    (contract, blockers)
}

pub fn build_playable_scenario_contract(project_dna: &Value, playable_bundle: &Value) -> Value {
    build_playable_scenario_contract_with_locale(
        project_dna,
        playable_bundle,
        ArtifactLocale::default(),
    )
}

pub fn build_playable_scenario_contract_with_locale(
    project_dna: &Value,
    playable_bundle: &Value,
    artifact_locale: ArtifactLocale,
) -> Value {
    let demo_flow = playable_bundle
        .get("demo_flow_contract")
        .cloned()
        .unwrap_or_else(|| json!({}));
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": artifact_locale,
        "contract_display_name": if artifact_locale == ArtifactLocale::ZhCn { "首个可玩场景契约" } else { "First Playable Scenario Contract" },
        "project_signature": get_str(project_dna, "project_signature"),
        "source_project_dna": "stage_02/project_dna_contract.json",
        "scenarios": project_dna.get("acceptance_scenarios").cloned()
            .filter(|value| !is_empty_value(Some(value)))
            .or_else(|| demo_flow.get("acceptance_scenarios").cloned())
            .unwrap_or_else(|| json!([{
                "scenario_id": "first_playable_flow",
                "expected": if artifact_locale == ArtifactLocale::ZhCn { "玩家可以进入演示场景、执行核心动作、获得反馈，并到达清晰的成功或失败状态。" } else { "Player can enter the demo scene, perform the core action, receive feedback, and reach a clear success/failure state." },
            }])),
        "acceptance_checks": demo_flow.get("acceptance_checks").cloned().unwrap_or_else(|| project_dna.get("acceptance_scenarios").cloned().unwrap_or_else(|| json!([]))),
        "source_refs": [
            "stage_02/project_dna_contract.json",
            "stage_02/playable_contracts/demo_flow_contract.json",
        ],
        "blockers": [],
    })
}

fn localize_validation_blockers(blockers: &mut [Value], artifact_locale: ArtifactLocale) {
    if artifact_locale != ArtifactLocale::ZhCn {
        return;
    }
    for blocker in blockers {
        let code = blocker
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match code {
            "CORE_ENTITY_UNFROZEN" => {
                blocker["message"] =
                    json!("步骤 03 及后续步骤使用项目 DNA 前，项目 DNA 必须处于冻结状态。");
            }
            "NULL_CONTRACT_FIELD" => {
                let field = blocker
                    .get("field")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                blocker["message"] = json!(format!("已冻结项目 DNA 字段 `{field}` 不能为空。"));
            }
            _ => {}
        }
    }
}

fn fill_if_empty(contract: &mut Value, field: &str, fallback: Vec<Value>) {
    if is_empty_value(contract.get(field)) && !fallback.is_empty() {
        contract[field] = Value::Array(fallback);
    }
}

trait VecIfEmpty {
    fn if_empty(self, fallback: Vec<Value>) -> Vec<Value>;
}

impl VecIfEmpty for Vec<Value> {
    fn if_empty(self, fallback: Vec<Value>) -> Vec<Value> {
        if self.is_empty() { fallback } else { self }
    }
}
