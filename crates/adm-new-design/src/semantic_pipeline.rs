use std::collections::{BTreeMap, BTreeSet};

use adm_new_contracts::ArtifactLocale;
use adm_new_foundation::unix_timestamp;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const COMMON_REQUIRED_CONTRACTS: &[&str] = &[
    "core_playable_contract",
    "demo_flow_contract",
    "runtime_data_contract",
    "ui_flow_contract",
    "scene_bootstrap_contract",
    "asset_mount_contract",
    "playable_acceptance_contract",
];
pub const OPTIONAL_CONTRACTS: &[&str] = &["audio_requirements_contract"];
pub const REQUIRED_ART_CATEGORIES: &[&str] = &[
    "character_unit",
    "scene_space",
    "ui_hud",
    "icon_resource",
    "feedback_vfx",
    "state_variant",
    "audio_placeholder",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchetypeDetection {
    pub archetype: String,
    pub confidence: String,
    pub sources: Vec<String>,
    pub warnings: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchetypeRequirementNodes {
    pub p0: Vec<String>,
    pub p1: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchetypeCatalog {
    requirements: BTreeMap<String, ArchetypeRequirementNodes>,
    keywords: BTreeMap<String, Vec<String>>,
    data: BTreeMap<String, Value>,
}

impl Default for ArchetypeCatalog {
    fn default() -> Self {
        Self::builtin()
    }
}

impl ArchetypeCatalog {
    pub fn builtin() -> Self {
        let mut requirements = BTreeMap::new();
        requirements.insert(
            "strategy".to_string(),
            nodes(
                &[
                    "core_loop_decision",
                    "input_control_decision",
                    "objective_system_decision",
                    "data_test_design_decision",
                ],
                &["build_system_decision", "progression_system_decision"],
            ),
        );
        requirements.insert(
            "narrative".to_string(),
            nodes(
                &[
                    "core_loop_decision",
                    "objective_system_decision",
                    "ux_flow_decision",
                    "data_test_design_decision",
                ],
                &["narrative_content_decision"],
            ),
        );
        requirements.insert(
            "management".to_string(),
            nodes(
                &[
                    "core_loop_decision",
                    "objective_system_decision",
                    "hud_feedback_decision",
                    "data_test_design_decision",
                ],
                &["build_system_decision", "retention_mid_long_goal_decision"],
            ),
        );
        requirements.insert(
            "generic_playable".to_string(),
            nodes(
                &[
                    "core_loop_decision",
                    "input_control_decision",
                    "objective_system_decision",
                    "hud_feedback_decision",
                    "data_test_design_decision",
                ],
                &["art_direction_decision"],
            ),
        );
        for (id, p0, p1, _keywords) in [
            (
                "action",
                vec![
                    "core_loop_decision",
                    "input_control_decision",
                    "objective_system_decision",
                    "hud_feedback_decision",
                    "data_test_design_decision",
                ],
                vec!["art_direction_decision", "juice_control_feel_decision"],
                vec!["action", "combat", "attack", "动作", "战斗", "打击"],
            ),
            (
                "puzzle",
                vec![
                    "core_loop_decision",
                    "objective_system_decision",
                    "ux_flow_decision",
                    "hud_feedback_decision",
                    "data_test_design_decision",
                ],
                vec!["onboarding_guidance_decision"],
                vec!["puzzle", "logic", "解谜", "谜题"],
            ),
            (
                "simulation",
                vec![
                    "core_loop_decision",
                    "objective_system_decision",
                    "data_goal_metric_decision",
                    "data_test_design_decision",
                ],
                vec!["build_system_decision"],
                vec!["simulation", "sandbox", "模拟", "沙盒"],
            ),
            (
                "rpg",
                vec![
                    "core_loop_decision",
                    "objective_system_decision",
                    "ux_flow_decision",
                    "data_test_design_decision",
                ],
                vec!["progression_system_decision", "content_type_decision"],
                vec!["rpg", "role", "角色", "成长", "装备"],
            ),
        ] {
            requirements.insert(id.to_string(), nodes(&p0, &p1));
        }
        let mut keywords = BTreeMap::from([
            (
                "strategy".to_string(),
                strings(&["strategy", "build", "tactic", "策略", "建造", "战术"]),
            ),
            (
                "narrative".to_string(),
                strings(&["narrative", "story", "剧情", "叙事"]),
            ),
            (
                "management".to_string(),
                strings(&["management", "tycoon", "经营", "管理"]),
            ),
        ]);
        let mut data = BTreeMap::new();
        install_subtype(
            &mut requirements,
            &mut keywords,
            &mut data,
            tower_defense_data(),
        );
        install_subtype(
            &mut requirements,
            &mut keywords,
            &mut data,
            narrative_puzzle_data(),
        );
        Self {
            requirements,
            keywords,
            data,
        }
    }

    pub fn contains(&self, archetype: &str) -> bool {
        self.requirements.contains_key(archetype)
    }
}

pub fn requirement_metadata_for_node(node_id: &str) -> Value {
    match node_id {
        "core_loop_decision" => metadata(
            &[
                "core_playable_contract",
                "demo_flow_contract",
                "runtime_data_contract",
            ],
            &["Step02", "Step03", "Step08", "Step13", "Step14"],
        ),
        "input_control_decision" => metadata(
            &[
                "core_playable_contract",
                "ui_flow_contract",
                "scene_bootstrap_contract",
            ],
            &["Step03", "Step08", "Step13", "Step14"],
        ),
        "objective_system_decision" => metadata(
            &[
                "demo_flow_contract",
                "runtime_data_contract",
                "playable_acceptance_contract",
            ],
            &["Step03", "Step08", "Step13", "Step14"],
        ),
        "art_direction_decision" => metadata(
            &["asset_mount_contract", "scene_bootstrap_contract"],
            &["Step04", "Step10", "Step12", "Step13"],
        ),
        "data_test_design_decision" => metadata(&["playable_acceptance_contract"], &["Step14"]),
        _ => json!({}),
    }
}

pub fn detect_archetype(
    profile: &Value,
    confirmed_option_text: &[String],
    catalog: &ArchetypeCatalog,
) -> ArchetypeDetection {
    let explicit =
        first_str(profile, &["archetype", "detected_archetype", "genre_id"]).unwrap_or_default();
    if catalog.contains(&explicit) {
        return ArchetypeDetection {
            archetype: explicit,
            confidence: "high".to_string(),
            sources: vec!["profile.archetype".to_string()],
            warnings: Vec::new(),
        };
    }
    let mut text_parts = flatten_text(profile);
    text_parts.extend(confirmed_option_text.iter().cloned());
    let text = text_parts.join(" ").to_lowercase();
    let gameplay_weights = gameplay_system_weights(profile);
    let mut scored = Vec::<(f64, String, Vec<String>)>::new();
    for (archetype, keywords) in &catalog.keywords {
        let mut score = 0.0;
        let mut sources = Vec::new();
        for keyword in keywords {
            if text.contains(&keyword.to_lowercase()) {
                score += 1.0;
                sources.push(format!("keyword:{keyword}"));
            }
        }
        if let Some(data) = catalog.data.get(archetype) {
            let (rule_score, rule_sources) = rule_match_score(data, &text, &gameplay_weights);
            score += rule_score;
            sources.extend(rule_sources);
        }
        if score > 0.0 {
            scored.push((score, archetype.clone(), sources));
        }
    }
    if !scored.is_empty() {
        scored.sort_by(|left, right| right.0.total_cmp(&left.0));
        let (score, archetype, sources) = scored.remove(0);
        return ArchetypeDetection {
            archetype,
            confidence: if score >= 3.0 { "high" } else { "medium" }.to_string(),
            sources,
            warnings: Vec::new(),
        };
    }
    ArchetypeDetection {
        archetype: "generic_playable".to_string(),
        confidence: "fallback".to_string(),
        sources: vec!["fallback.generic_playable".to_string()],
        warnings: vec![json!({
            "code": "ARCHETYPE_FALLBACK_GENERIC",
            "message": "No confirmed archetype signal was found; generic_playable requirements are used.",
            "severity": "warning"
        })],
    }
}

pub fn build_archetype_requirements(
    profile: &Value,
    confirmed_option_text: &[String],
    catalog: &ArchetypeCatalog,
) -> Value {
    build_archetype_requirements_with_locale(
        profile,
        confirmed_option_text,
        catalog,
        ArtifactLocale::default(),
    )
}

pub fn build_archetype_requirements_with_locale(
    profile: &Value,
    confirmed_option_text: &[String],
    catalog: &ArchetypeCatalog,
    artifact_locale: ArtifactLocale,
) -> Value {
    let detection = detect_archetype(profile, confirmed_option_text, catalog);
    let nodes = catalog
        .requirements
        .get(&detection.archetype)
        .cloned()
        .unwrap_or_else(|| catalog.requirements["generic_playable"].clone());
    let data = catalog
        .data
        .get(&detection.archetype)
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut result = json!({
        "schema_version": "1.0",
        "detected_archetype": detection.archetype,
        "detection_confidence": detection.confidence,
        "detection_source": detection.sources,
        "required_contracts": COMMON_REQUIRED_CONTRACTS,
        "optional_contracts": OPTIONAL_CONTRACTS,
        "archetype_p0_nodes": nodes.p0,
        "archetype_p1_nodes": nodes.p1,
        "not_applicable_rules": [],
        "warnings": detection.warnings
    });
    for key in [
        "parent_archetypes",
        "detection_rules",
        "required_systems",
        "required_entities",
        "required_player_actions",
        "required_resources",
        "required_objectives",
        "minimum_playable_assets",
        "style_compatibility",
        "open_questions",
        "program_task_templates",
        "acceptance_scenarios",
    ] {
        if let Some(value) = data.get(key) {
            result[key] = value.clone();
        }
    }
    if let Some(value) = data.get("minimum_playable_assets") {
        result["required_assets"] = value.clone();
    }
    result["artifact_locale"] = json!(artifact_locale);
    result["contract_display_name"] = json!(if artifact_locale == ArtifactLocale::ZhCn {
        "原型需求契约"
    } else {
        "Archetype Requirements Contract"
    });
    if artifact_locale == ArtifactLocale::ZhCn {
        for warning in result
            .get_mut("warnings")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if warning.get("code").and_then(Value::as_str) == Some("ARCHETYPE_FALLBACK_GENERIC") {
                warning["message"] =
                    json!("未找到已确认的原型信号，当前使用 generic_playable 通用可玩需求。");
            }
        }
    }
    result
}

pub fn build_program_capability_contract(project_dna: &Value) -> Value {
    let mut capabilities = Vec::new();
    let mut blockers = Vec::new();
    let acceptance_refs = acceptance_refs(project_dna);
    let scenarios = list(project_dna, "acceptance_scenarios");

    for (index, entity) in list(project_dna, "core_entities").iter().enumerate() {
        let id = first_str(entity, &["entity_id"])
            .unwrap_or_else(|| format!("core_entity_{:02}", index + 1));
        let class_name = pascal(&id, &format!("CoreEntity{:02}", index + 1));
        let source_ref = ref_path("core_entities", index);
        capabilities.push(json!({
            "capability_id": class_name,
            "capability_type": "core_entity",
            "source_entity_refs": [source_ref],
            "source_action_refs": [],
            "source_resource_refs": [],
            "program_class": format!("{class_name}ViewModel"),
            "data_fields": [{"field":"state","type":"string"}],
            "input_actions": [],
            "state_changes": [state_change("entity_state_changes", &source_ref, "entity_state", &format!("{id} state can change during playable flow."))],
            "ui_bindings": [{"surface":"hud","binding":id}],
            "acceptance_refs": acceptance_refs,
            "acceptance_scenarios": scenarios,
            "required": true,
            "source_semantic_id": id
        }));
    }
    for (index, system) in list(project_dna, "runtime_systems").iter().enumerate() {
        let id = first_str(system, &["system_id"])
            .unwrap_or_else(|| format!("runtime_system_{:02}", index + 1));
        let class_name = pascal(&id, &format!("RuntimeSystem{:02}", index + 1));
        let source_ref = ref_path("runtime_systems", index);
        capabilities.push(json!({
            "capability_id": class_name,
            "capability_type": "gameplay_system",
            "source_entity_refs": [source_ref],
            "source_action_refs": [],
            "source_resource_refs": [],
            "program_class": class_name,
            "data_fields": [{"field":"isActive","type":"bool"}],
            "input_actions": [],
            "state_changes": [state_change("system_state_changes", &source_ref, "system_state", &format!("{id} runtime state can change during execution."))],
            "ui_bindings": [],
            "acceptance_refs": acceptance_refs,
            "acceptance_scenarios": scenarios,
            "required": true,
            "source_semantic_id": id
        }));
    }
    for (index, action) in list(project_dna, "player_actions").iter().enumerate() {
        let id = first_str(action, &["action_id"])
            .unwrap_or_else(|| format!("player_action_{:02}", index + 1));
        let state_change_id = first_str(action, &["state_change"]).unwrap_or_default();
        if state_change_id.is_empty() {
            blockers.push(json!({"code":"ACTION_HAS_NO_STATE_CHANGE","action_id":id,"message":format!("Player action `{id}` must declare a state change.")}));
        }
        let class_name = pascal(&id, &format!("PlayerAction{:02}", index + 1));
        let source_ref = ref_path("player_actions", index);
        capabilities.push(json!({
            "capability_id": class_name,
            "capability_type": "player_action",
            "source_entity_refs": [],
            "source_action_refs": [source_ref],
            "source_resource_refs": [],
            "program_class": format!("{class_name}Controller"),
            "data_fields": [{"field":"lastResult","type":"string"}],
            "input_actions": [{"action_id":id,"source_ref":source_ref}],
            "state_changes": [state_change(if state_change_id.is_empty() {"missing_state_change"} else {&state_change_id}, &source_ref, "player_action_result", &format!("{id} must produce an observable state change."))],
            "ui_bindings": [{"surface":"hud","binding":format!("{id}_feedback")}],
            "acceptance_refs": acceptance_refs,
            "acceptance_scenarios": scenarios,
            "required": true,
            "source_semantic_id": id
        }));
    }
    for (index, resource) in list(project_dna, "resources").iter().enumerate() {
        let id = first_str(resource, &["resource_id"])
            .unwrap_or_else(|| format!("resource_{:02}", index + 1));
        let class_name = pascal(&id, &format!("Resource{:02}", index + 1));
        let source_ref = ref_path("resources", index);
        capabilities.push(json!({
            "capability_id": class_name,
            "capability_type": "resource_model",
            "source_entity_refs": [],
            "source_action_refs": [],
            "source_resource_refs": [source_ref],
            "program_class": format!("{class_name}Model"),
            "data_fields": [{"field":id,"type":"int"}],
            "input_actions": [],
            "state_changes": [state_change(&format!("{id}_amount_changes"), &source_ref, "resource_state", &format!("{id} amount changes are tracked by runtime code."))],
            "ui_bindings": [{"surface":"hud","binding":id}],
            "acceptance_refs": acceptance_refs,
            "acceptance_scenarios": scenarios,
            "required": true,
            "source_semantic_id": id
        }));
    }
    for (index, objective) in list(project_dna, "objectives").iter().enumerate() {
        let id = first_str(objective, &["objective_id"])
            .unwrap_or_else(|| format!("objective_{:02}", index + 1));
        if first_str(objective, &["completion_condition"])
            .unwrap_or_default()
            .is_empty()
            && first_str(objective, &["failure_condition"])
                .unwrap_or_default()
                .is_empty()
        {
            blockers.push(json!({"code":"OBJECTIVE_HAS_NO_COMPLETION_CONDITION","objective_id":id,"message":format!("Objective `{id}` must define completion or failure conditions.")}));
        }
        let class_name = pascal(&id, &format!("Objective{:02}", index + 1));
        let source_ref = ref_path("objectives", index);
        capabilities.push(json!({
            "capability_id": class_name,
            "capability_type": "objective",
            "source_entity_refs": [source_ref],
            "source_action_refs": [],
            "source_resource_refs": [],
            "program_class": format!("{class_name}Tracker"),
            "data_fields": [{"field":"objectiveState","type":"string"}],
            "input_actions": [],
            "state_changes": [state_change("objective_state_changes", &source_ref, "objective_state", &format!("{id} completion or failure state is tracked."))],
            "ui_bindings": [{"surface":"hud","binding":id}],
            "acceptance_refs": [id],
            "acceptance_scenarios": [objective.clone()],
            "required": true,
            "source_semantic_id": id
        }));
    }

    json!({
        "schema_version":"1.0",
        "generated_at":now_iso(),
        "project_signature":string_field(project_dna,"project_signature"),
        "source_contract":"stage_02/project_dna_contract.json",
        "capabilities":capabilities,
        "unbound_semantic_items":[],
        "blocking_issues":blockers
    })
}

pub fn build_program_semantic_coverage_report(
    project_dna: &Value,
    capability_contract: &Value,
) -> Value {
    let required_total = [
        "runtime_systems",
        "core_entities",
        "player_actions",
        "resources",
        "objectives",
    ]
    .iter()
    .map(|field| list(project_dna, field).len())
    .sum::<usize>();
    let mut blockers = list(capability_contract, "blocking_issues");
    let missing = list(capability_contract, "unbound_semantic_items");
    let covered = required_total.saturating_sub(missing.len());
    let coverage = if required_total == 0 {
        1.0
    } else {
        round4(covered as f64 / required_total as f64)
    };
    if coverage < 0.85 {
        blockers.push(json!({"code":"PROGRAM_CAPABILITY_NOT_BOUND","message":"Required program semantic coverage is below 85%.","coverage":coverage}));
    }
    json!({
        "schema_version":"1.0","generated_at":now_iso(),
        "status":if blockers.is_empty() {"passed"} else {"blocked"},
        "project_signature":string_field(project_dna,"project_signature"),
        "source_contracts":["stage_02/project_dna_contract.json","stage_03/program_capability_contract.json"],
        "coverage_items":list(capability_contract,"capabilities"),
        "coverage":coverage,
        "missing_program_capabilities":missing,
        "blockers":blockers
    })
}

pub fn build_art_taxonomy_and_strategy(
    project_dna: &Value,
    archetype_requirements: &Value,
    asset_spec_contract: Option<&Value>,
) -> (Value, Value) {
    let existing_assets = asset_spec_contract
        .map(|value| list(value, "assets"))
        .unwrap_or_default();
    let mut strategies = Vec::new();
    for (index, entity) in list(project_dna, "core_entities").iter().enumerate() {
        let id =
            first_str(entity, &["entity_id"]).unwrap_or_else(|| format!("entity_{:02}", index + 1));
        strategies.push(asset_strategy(
            &format!("entity_asset_{id}"),
            &ref_path("core_entities", index),
            &format!("{id}_visual"),
            "character_unit",
            &first_str(entity, &["role"]).unwrap_or_else(|| "core_loop_runtime".to_string()),
            &format!("Assets/AutoDesign/Art/Source/{id}.png"),
            true,
            false,
            false,
        ));
    }
    for (index, surface) in list(project_dna, "ui_surfaces").iter().enumerate() {
        let id = first_str(surface, &["surface_id"])
            .unwrap_or_else(|| format!("surface_{:02}", index + 1));
        strategies.push(asset_strategy(
            &format!("ui_asset_{id}"),
            &ref_path("ui_surfaces", index),
            &format!("{id}_prefab"),
            "ui_hud",
            &id,
            &format!("Assets/AutoDesign/Prefabs/UI/{id}.prefab"),
            true,
            true,
            false,
        ));
    }
    for (index, resource) in list(project_dna, "resources").iter().enumerate() {
        let id = first_str(resource, &["resource_id"])
            .unwrap_or_else(|| format!("resource_{:02}", index + 1));
        strategies.push(asset_strategy(
            &format!("resource_icon_{id}"),
            &ref_path("resources", index),
            &format!("{id}_icon"),
            "icon_resource",
            "hud_feedback",
            &format!("Assets/AutoDesign/Art/Source/icons/{id}.png"),
            true,
            false,
            false,
        ));
    }
    let needs = {
        let own = list(project_dna, "asset_needs");
        if own.is_empty() {
            list(archetype_requirements, "required_assets")
        } else {
            own
        }
    };
    for (index, asset) in needs.iter().enumerate() {
        let role = first_str(asset, &["asset_role"])
            .unwrap_or_else(|| format!("asset_need_{:02}", index + 1));
        let category = if role.contains("feedback") {
            "feedback_vfx"
        } else {
            "scene_space"
        };
        strategies.push(asset_strategy(
            &format!("required_asset_{role}"),
            &ref_path("asset_needs", index),
            &role,
            category,
            &first_str(asset, &["consumer"]).unwrap_or_else(|| "core_loop_runtime".to_string()),
            &format!("Assets/AutoDesign/Art/Source/{role}.png"),
            true,
            role.contains("ui") || role.contains("hud"),
            false,
        ));
    }
    strategies.push(asset_strategy(
        "audio_placeholder_core_loop",
        "stage_02/project_dna_contract.json#$.acceptance_scenarios",
        "core_loop_audio_placeholder",
        "audio_placeholder",
        "AudioEventRegistry",
        "Assets/AutoDesign/Audio/Placeholders/core_loop.placeholder",
        false,
        false,
        true,
    ));
    let mut covered = categories(&strategies);
    for category in REQUIRED_ART_CATEGORIES {
        if !covered.contains(*category) {
            strategies.push(asset_strategy(
                &format!("fallback_{category}"),
                "stage_02/project_dna_contract.json#$.project_signature",
                &format!("{category}_fallback"),
                category,
                "core_loop_runtime",
                &format!("Assets/AutoDesign/Art/Fallback/{category}.png"),
                *category != "audio_placeholder",
                *category == "ui_hud",
                false,
            ));
            covered.insert((*category).to_string());
        }
    }
    let blockers = Vec::<Value>::new();
    let taxonomy = json!({
        "schema_version":"1.0","generated_at":now_iso(),
        "project_signature":string_field(project_dna,"project_signature"),
        "source_contract":"stage_02/project_dna_contract.json",
        "asset_categories":REQUIRED_ART_CATEGORIES.iter().map(|category| json!({"category_id":category,"required":true,"coverage_source":"project_dna_or_archetype"})).collect::<Vec<_>>(),
        "ui_asset_types":filter_category(&strategies, &["ui_hud"]),
        "world_asset_types":filter_category(&strategies, &["scene_space","character_unit"]),
        "character_asset_types":filter_category(&strategies, &["character_unit"]),
        "source_refs":["stage_02/project_dna_contract.json"],
        "blockers":blockers
    });
    let matrix = json!({
        "schema_version":"1.0","generated_at":now_iso(),
        "project_signature":string_field(project_dna,"project_signature"),
        "source_contracts":["stage_02/project_dna_contract.json","stage_04/asset_spec_contract.json"],
        "assets":strategies,
        "mount_requirements":strategies.iter().map(|item| json!({"asset_role":item["asset_role"],"unity_target_path":item["unity_target_path"],"consumer_system":item["consumer_system"]})).collect::<Vec<_>>(),
        "ui_slice_requirements":strategies.iter().filter(|item| item.get("ui_slice_required").and_then(Value::as_bool).unwrap_or(false)).cloned().collect::<Vec<_>>(),
        "audio_placeholders":strategies.iter().filter(|item| item.get("audio_placeholder_only").and_then(Value::as_bool).unwrap_or(false)).cloned().collect::<Vec<_>>(),
        "existing_asset_count":existing_assets.len(),
        "blockers":taxonomy["blockers"]
    });
    (taxonomy, matrix)
}

pub fn build_semantic_coverage_seed(project_dna: &Value) -> Value {
    build_semantic_coverage_seed_with_locale(project_dna, ArtifactLocale::default())
}

pub fn build_semantic_coverage_seed_with_locale(
    project_dna: &Value,
    artifact_locale: ArtifactLocale,
) -> Value {
    let mut items = Vec::new();
    for (field, item_type) in [
        ("runtime_systems", "program_system"),
        ("core_entities", "entity"),
        ("player_actions", "player_action"),
        ("objectives", "objective"),
        ("asset_needs", "asset_need"),
        ("acceptance_scenarios", "acceptance_scenario"),
    ] {
        for (index, item) in list(project_dna, field).iter().enumerate() {
            let id = first_str(
                item,
                &[
                    "system_id",
                    "entity_id",
                    "action_id",
                    "objective_id",
                    "asset_role",
                    "scenario_id",
                ],
            )
            .unwrap_or_else(|| format!("{field}_{:02}", index + 1));
            let item_type_display_name =
                semantic_item_type_display_name(item_type, artifact_locale);
            let display_name = if artifact_locale == ArtifactLocale::ZhCn {
                format!("{item_type_display_name}：{id}")
            } else {
                format!("{item_type_display_name}: {id}")
            };
            items.push(json!({
                "coverage_id":format!("{item_type}:{id}"),
                "item_type":item_type,
                "item_type_display_name": item_type_display_name,
                "display_name": display_name,
                "source_field":field,
                "source_contract":"stage_02/project_dna_contract.json",
                "program_status":"pending",
                "art_status":"pending"
            }));
        }
    }
    json!({
        "schema_version":"1.0","generated_at":now_iso(),"matrix_state":"seed",
        "artifact_locale": artifact_locale,
        "matrix_display_name": if artifact_locale == ArtifactLocale::ZhCn { "语义覆盖种子矩阵" } else { "Semantic Coverage Seed Matrix" },
        "project_signature":string_field(project_dna,"project_signature"),
        "source_contracts":["stage_02/project_dna_contract.json"],
        "coverage_items":items,
        "program_coverage":{"status":"pending","covered":0,"total":items.len()},
        "art_coverage":{"status":"pending","covered":0,"total":items.len()},
        "uncovered_items":items,
        "blockers":[]
    })
}

fn semantic_item_type_display_name(
    item_type: &str,
    artifact_locale: ArtifactLocale,
) -> &'static str {
    match (item_type, artifact_locale) {
        ("program_system", ArtifactLocale::ZhCn) => "程序系统",
        ("entity", ArtifactLocale::ZhCn) => "核心实体",
        ("player_action", ArtifactLocale::ZhCn) => "玩家动作",
        ("objective", ArtifactLocale::ZhCn) => "目标",
        ("asset_need", ArtifactLocale::ZhCn) => "资产需求",
        ("acceptance_scenario", ArtifactLocale::ZhCn) => "验收场景",
        ("program_system", ArtifactLocale::EnUs) => "Program system",
        ("entity", ArtifactLocale::EnUs) => "Core entity",
        ("player_action", ArtifactLocale::EnUs) => "Player action",
        ("objective", ArtifactLocale::EnUs) => "Objective",
        ("asset_need", ArtifactLocale::EnUs) => "Asset need",
        ("acceptance_scenario", ArtifactLocale::EnUs) => "Acceptance scenario",
        (_, ArtifactLocale::ZhCn) => "语义项",
        (_, ArtifactLocale::EnUs) => "Semantic item",
    }
}

pub fn build_semantic_alignment_outputs(
    project_dna: &Value,
    program_matrix: &Value,
    art_matrix: &Value,
) -> (Value, Value) {
    let mut all = Vec::new();
    let mut blockers = Vec::new();
    let program_items = list(program_matrix, "coverage_items");
    let art_items = list(art_matrix, "coverage_items");
    for item in &program_items {
        let status = if item.get("status").and_then(Value::as_str) == Some("covered") {
            "covered"
        } else {
            "gap"
        };
        if status != "covered" {
            blockers.push(json!({"code":"SEMANTIC_ALIGNMENT_GAP","coverage_id":item.get("coverage_id").cloned().unwrap_or(Value::Null),"message":"Program semantic item is not covered."}));
        }
        all.push(with_alignment(item, status, "program"));
    }
    for item in &art_items {
        let status = if item.get("status").and_then(Value::as_str) == Some("covered") {
            "covered"
        } else {
            "gap"
        };
        let code = if item.get("status").and_then(Value::as_str) == Some("placeholder_only") {
            "PLACEHOLDER_ONLY_ALIGNMENT"
        } else {
            "SEMANTIC_ALIGNMENT_GAP"
        };
        if status != "covered" {
            blockers.push(json!({"code":code,"coverage_id":item.get("coverage_id").cloned().unwrap_or(Value::Null),"message":"Art semantic item is not fully covered."}));
        }
        all.push(with_alignment(item, status, "art"));
    }
    let total = all.len();
    let covered = all
        .iter()
        .filter(|item| item.get("alignment_status").and_then(Value::as_str) == Some("covered"))
        .count();
    let coverage = if total == 0 {
        1.0
    } else {
        round4(covered as f64 / total as f64)
    };
    if coverage < 0.85 {
        blockers.push(json!({"code":"SEMANTIC_ALIGNMENT_GAP","message":"Combined semantic coverage is below 85%.","coverage":coverage}));
    }
    let sources = json!([
        "stage_02/project_dna_contract.json",
        "stage_08/program_semantic_coverage_matrix.json",
        "stage_09/art_semantic_coverage_matrix.json"
    ]);
    let matrix = json!({
        "schema_version":"1.0","generated_at":now_iso(),"matrix_state":"final",
        "project_signature":string_field(project_dna,"project_signature"),
        "source_contracts":sources,
        "coverage_items":all,
        "program_coverage":{"covered":program_items.iter().filter(|item| item.get("status").and_then(Value::as_str)==Some("covered")).count(),"total":program_items.len()},
        "art_coverage":{"covered":art_items.iter().filter(|item| item.get("status").and_then(Value::as_str)==Some("covered")).count(),"total":art_items.len()},
        "uncovered_items":all.iter().filter(|item| item.get("alignment_status").and_then(Value::as_str)!=Some("covered")).cloned().collect::<Vec<_>>(),
        "blockers":blockers
    });
    let report = json!({
        "schema_version":"1.0","generated_at":now_iso(),
        "status":if blockers.is_empty() {"passed"} else {"blocked"},
        "project_signature":string_field(project_dna,"project_signature"),
        "source_contracts":sources,
        "alignment_checks":matrix["coverage_items"],
        "coverage_summary":{"coverage":coverage,"covered":covered,"total":total},
        "blockers":blockers,
        "warnings":[]
    });
    (report, matrix)
}

pub fn build_style_fit_report(
    archetype_requirements: &Value,
    style_options: Option<&Value>,
    selected_style_id: &str,
    override_reason: &str,
) -> (Value, Value) {
    let empty = json!({});
    let style_options = style_options.unwrap_or(&empty);
    let archetype = string_field(archetype_requirements, "detected_archetype");
    let selected = if selected_style_id.is_empty() {
        string_field(style_options, "recommended_style_id")
    } else {
        selected_style_id.to_string()
    };
    let mut blockers = Vec::new();
    let mut risks = Vec::new();
    if archetype == "tower_defense" && selected.contains("cinematic_realism") {
        let issue = json!({"code":"STYLE_ARCHETYPE_MISMATCH","message":"Tower defense requires highly readable lanes, units, and projectiles; cinematic realism is risky for the first playable.","style_id":selected});
        if override_reason.is_empty() {
            blockers.push(issue);
        } else {
            risks.push(issue);
        }
    }
    let readability = archetype_requirements
        .get("style_compatibility")
        .and_then(|value| value.get("required_readability"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !readability.is_empty() {
        risks.push(json!({"code":"STYLE_READABILITY_RISK","message":"Style must preserve archetype readability constraints.","required_readability":readability}));
    }
    let report = json!({
        "schema_version":"1.0","generated_at":now_iso(),
        "status":if blockers.is_empty() {"passed"} else {"blocked"},
        "style_id":selected,
        "project_signature":string_field(archetype_requirements,"project_signature"),
        "source_refs":["stage_01/archetype_requirements.json","stage_07/style_options.json"],
        "fit_checks":[{"check_id":"archetype_readability","archetype":archetype,"style_id":selected,"status":if blockers.is_empty() {"passed"} else {"blocked"}}],
        "risks":risks.clone(),
        "blockers":blockers.clone()
    });
    let ack_risks = if !override_reason.is_empty() {
        risks
    } else if blockers.is_empty() {
        Vec::new()
    } else {
        blockers.clone()
    };
    let acknowledgement = json!({
        "schema_version":"1.0","generated_at":now_iso(),
        "status":if !override_reason.is_empty() {"acknowledged"} else if blockers.is_empty() {"not_required"} else {"required"},
        "style_id":selected,
        "acknowledged_risks":ack_risks,
        "human_confirmation":if override_reason.is_empty() { json!({}) } else { json!({"override_reason":override_reason}) },
        "source_refs":["stage_07/style_fit_report.json"]
    });
    (report, acknowledgement)
}

pub fn ensure_art_tasks_cover_asset_strategy(
    tasks: &mut Vec<Value>,
    asset_strategy_matrix: &Value,
    selected_style_id: &str,
) -> Value {
    let mut covered_roles = tasks
        .iter()
        .flat_map(|task| string_list(task, "source_semantics"))
        .collect::<BTreeSet<_>>();
    let mut covered_targets = tasks
        .iter()
        .filter_map(|task| first_str(task, &["unity_target_path"]))
        .collect::<BTreeSet<_>>();
    let mut added = Vec::new();
    let mut next_index = tasks.len() + 1;
    for asset in list(asset_strategy_matrix, "assets") {
        let role = first_str(&asset, &["asset_role"]).unwrap_or_default();
        let target = first_str(&asset, &["unity_target_path"]).unwrap_or_default();
        if role.is_empty()
            || covered_roles.contains(&role)
            || (!target.is_empty() && covered_targets.contains(&target))
        {
            continue;
        }
        let category = string_field(&asset, "category_id");
        let asset_type = asset_type_from_category(&category);
        let folder = target
            .rsplit_once('/')
            .map(|(left, _)| format!("{left}/"))
            .unwrap_or_else(|| target.clone());
        let task = json!({
            "task_id":format!("ART-{next_index:03}"),"asset_id":role,"title":role.replace('_'," "),
            "asset_type":asset_type,"category":if category.is_empty() {"art"} else {&category},
            "priority":if asset.get("final_asset_required").and_then(Value::as_bool).unwrap_or(true) {"P0"} else {"P2"},
            "complexity":if matches!(asset_type.as_str(),"ui"|"effect"|"audio_placeholder") {"s"} else {"m"},
            "phase":"core_playable",
            "source_refs":[first_str(&asset,&["source_ref"]).unwrap_or_else(||"stage_04/asset_strategy_matrix.json".to_string()),"stage_04/asset_strategy_matrix.json".to_string(),"stage_07/style_application_contract.json".to_string()],
            "unity_target_path":target,"dimensions":dimensions_for_asset_type(&asset_type),
            "consumer_system":first_str(&asset,&["consumer_system"]).unwrap_or_else(||"core_loop_runtime".to_string()),
            "mount_point":if target.is_empty() {"Assets/AutoDesign".to_string()} else {folder.clone()},
            "acceptance":format!("{}_available_for_{}",role,first_str(&asset,&["consumer_system"]).unwrap_or_else(||"runtime".to_string())),
            "output_files":if target.is_empty(){Vec::<String>::new()}else{vec![target.clone()]},
            "allowed_write_paths":vec![folder],
            "contract_refs":{"asset_strategy_matrix":"stage_04/asset_strategy_matrix.json","style_application_contract":"stage_07/style_application_contract.json","selected_style_id":selected_style_id,"asset_role":role},
            "source_semantics":[role],"design_requirement_refs":["stage_04/asset_strategy_matrix.json"],
            "consumer_refs":[first_str(&asset,&["consumer_system"]).unwrap_or_default()],
            "status":"planned","strategy_generated":true
        });
        covered_roles.insert(role.clone());
        if !target.is_empty() {
            covered_targets.insert(target);
        }
        tasks.push(task.clone());
        added.push(task);
        next_index += 1;
    }
    json!({"added_task_count":added.len(),"added_asset_roles":added.iter().filter_map(|task| first_str(task,&["asset_id"])).collect::<Vec<_>>()})
}

pub fn enrich_program_tasks_with_semantics(
    tasks: &mut [Value],
    capability_contract: &Value,
) -> Value {
    let capability_ids = list(capability_contract, "capabilities")
        .iter()
        .filter_map(|item| first_str(item, &["capability_id"]))
        .collect::<Vec<_>>();
    if capability_ids.is_empty() {
        for task in tasks {
            let legacy = format!(
                "legacy:{}",
                first_str(task, &["requirement_id", "task_id"])
                    .unwrap_or_else(|| "task".to_string())
            );
            push_unique(task, "project_semantic_refs", &legacy);
            set_default(task, "capability_refs", json!([legacy]));
            set_default(
                task,
                "semantic_source_contract",
                json!("stage_03/program_requirements_contract.json"),
            );
        }
        return json!({"generic_ratio":0.0,"capability_ids":[],"blockers":[],"legacy_semantic_bridge":true});
    }
    for (index, task) in tasks.iter_mut().enumerate() {
        let cap_id = &capability_ids[index % capability_ids.len()];
        push_unique(task, "project_semantic_refs", cap_id);
        set_default(task, "capability_refs", json!([cap_id]));
        set_default(
            task,
            "semantic_source_contract",
            json!("stage_03/program_capability_contract.json"),
        );
    }
    let covered = tasks
        .iter()
        .flat_map(|task| string_list(task, "project_semantic_refs"))
        .collect::<BTreeSet<_>>();
    let missing = capability_ids
        .iter()
        .filter(|id| !covered.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() && !tasks.is_empty() {
        for id in missing {
            push_unique(&mut tasks[0], "project_semantic_refs", &id);
            push_unique(&mut tasks[0], "capability_refs", &id);
        }
    }
    json!({"generic_ratio":0.0,"capability_ids":capability_ids,"blockers":[]})
}

pub fn build_program_semantic_coverage_matrix(
    tasks: &[Value],
    capability_contract: &Value,
) -> Value {
    let refs = tasks
        .iter()
        .flat_map(|task| string_list(task, "project_semantic_refs"))
        .collect::<BTreeSet<_>>();
    let mut items = Vec::new();
    let mut blockers = Vec::new();
    for capability in list(capability_contract, "capabilities") {
        let cap_id = first_str(&capability, &["capability_id"]).unwrap_or_default();
        let status = if refs.contains(&cap_id) {
            "covered"
        } else {
            "missing"
        };
        if status == "missing"
            && capability
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(true)
        {
            blockers.push(json!({"code":"CORE_MECHANIC_NOT_PLANNED","capability_id":cap_id,"message":"Required capability is not covered by a program task."}));
        }
        items.push(json!({"coverage_id":format!("program:{cap_id}"),"capability_id":cap_id,"status":status,"source_contract":"stage_03/program_capability_contract.json"}));
    }
    json!({"schema_version":"1.0","generated_at":now_iso(),"matrix_state":"program","project_signature":string_field(capability_contract,"project_signature"),"source_contracts":["stage_03/program_capability_contract.json","stage_08/program_task_breakdown.json"],"coverage_items":items,"uncovered_requirements":items.iter().filter(|item| item.get("status").and_then(Value::as_str)!=Some("covered")).cloned().collect::<Vec<_>>(),"blockers":blockers})
}

pub fn enrich_art_tasks_with_semantics(
    tasks: &mut [Value],
    asset_strategy_matrix: &Value,
) -> Value {
    let roles = list(asset_strategy_matrix, "assets")
        .iter()
        .filter_map(|item| first_str(item, &["asset_role"]))
        .collect::<Vec<_>>();
    let mut blockers = Vec::new();
    for (index, task) in tasks.iter_mut().enumerate() {
        let role = string_list(task, "source_semantics")
            .first()
            .cloned()
            .or_else(|| roles.get(index % roles.len().max(1)).cloned())
            .or_else(|| first_str(task, &["asset_id"]))
            .unwrap_or_default();
        if !role.is_empty() {
            push_unique(task, "source_semantics", &role);
        }
        set_default(
            task,
            "design_requirement_refs",
            json!(["stage_04/asset_strategy_matrix.json"]),
        );
        set_default(
            task,
            "consumer_refs",
            json!([first_str(task, &["consumer_system"]).unwrap_or_default()]),
        );
        if first_str(task, &["generation_prompt"])
            .unwrap_or_default()
            .is_empty()
        {
            task["generation_prompt"] = json!(format!(
                "Create {} for {}; usage={}; target={}; preserve transparent background when required and keep UI/gameplay readability.",
                first_str(task, &["asset_type"]).unwrap_or_else(|| "2D asset".to_string()),
                role,
                first_str(task, &["consumer_system"]).unwrap_or_default(),
                first_str(task, &["unity_target_path"]).unwrap_or_default()
            ));
        }
        if role.is_empty() {
            blockers.push(json!({"code":"ART_TASK_TOO_GENERIC","task_id":task.get("task_id").cloned().unwrap_or(Value::Null),"message":"Art task has no source semantic role."}));
        }
    }
    json!({"asset_roles":roles,"blockers":blockers})
}

pub fn build_art_semantic_coverage_matrix(tasks: &[Value], asset_strategy_matrix: &Value) -> Value {
    let task_roles = tasks
        .iter()
        .flat_map(|task| string_list(task, "source_semantics"))
        .collect::<BTreeSet<_>>();
    let mut items = Vec::new();
    let mut blockers = Vec::new();
    for asset in list(asset_strategy_matrix, "assets") {
        let role = first_str(&asset, &["asset_role"]).unwrap_or_default();
        let placeholder_only = asset
            .get("placeholder_allowed")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && !asset
                .get("final_asset_required")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        let accepted_placeholder = placeholder_only
            && asset
                .get("audio_placeholder_only")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        let status = if task_roles.contains(&role) && (!placeholder_only || accepted_placeholder) {
            "covered"
        } else if placeholder_only {
            "placeholder_only"
        } else {
            "missing"
        };
        if status == "missing" {
            blockers.push(json!({"code":"CORE_ASSET_NOT_PRODUCED","asset_role":role,"message":"Core asset strategy is not covered by an art task."}));
        }
        items.push(json!({"coverage_id":format!("art:{role}"),"asset_role":role,"status":status,"source_contract":"stage_04/asset_strategy_matrix.json"}));
    }
    json!({"schema_version":"1.0","generated_at":now_iso(),"matrix_state":"art","project_signature":string_field(asset_strategy_matrix,"project_signature"),"source_contracts":["stage_04/asset_strategy_matrix.json","stage_09/art_production_task_contract.json"],"coverage_items":items,"uncovered_assets":items.iter().filter(|item| item.get("status").and_then(Value::as_str)!=Some("covered")).cloned().collect::<Vec<_>>(),"blockers":blockers})
}

fn now_iso() -> String {
    format!("unix:{}", unix_timestamp())
}
fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}
fn nodes(p0: &[&str], p1: &[&str]) -> ArchetypeRequirementNodes {
    ArchetypeRequirementNodes {
        p0: strings(p0),
        p1: strings(p1),
    }
}
fn metadata(targets: &[&str], steps: &[&str]) -> Value {
    json!({"contract_targets":targets,"consumed_by_steps":steps,"priority":"P0","requirement_level":"required","required_for_archetypes":["all"],"optional_for_archetypes":[],"not_applicable_allowed":true,"not_applicable_requires_reason":true})
}
fn list(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}
fn string_list(value: &Value, key: &str) -> Vec<String> {
    list(value, key)
        .iter()
        .filter_map(|item| item.as_str().map(str::to_string))
        .collect()
}
fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
fn first_str(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    })
}
fn ref_path(field: &str, index: usize) -> String {
    format!("stage_02/project_dna_contract.json#$.{field}[{index}]")
}
fn state_change(id: &str, source: &str, kind: &str, description: &str) -> Value {
    json!({"change_id":id,"kind":kind,"source_ref":source,"description":description})
}
fn pascal(value: &str, fallback: &str) -> String {
    let out = value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| format!("{}{}", first.to_ascii_uppercase(), chars.as_str()))
                .unwrap_or_default()
        })
        .collect::<String>();
    if out.is_empty() {
        fallback.to_string()
    } else {
        out
    }
}
fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn flatten_text(value: &Value) -> Vec<String> {
    match value {
        Value::Null => Vec::new(),
        Value::Bool(v) => vec![v.to_string()],
        Value::Number(v) => vec![v.to_string()],
        Value::String(v) => vec![v.clone()],
        Value::Array(items) => items.iter().flat_map(flatten_text).collect(),
        Value::Object(object) => object.values().flat_map(flatten_text).collect(),
    }
}

fn gameplay_system_weights(profile: &Value) -> BTreeMap<String, f64> {
    let raw = profile
        .get("gameplay_systems")
        .or_else(|| profile.get("gameplaySystems"))
        .or_else(|| profile.get("systems"))
        .unwrap_or(&Value::Null);
    let mut out = BTreeMap::new();
    for item in raw
        .get("selected")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(id) = item.as_str() {
            out.insert(id.to_string(), 1.0);
        }
    }
    if let Some(weights) = raw.get("weights").and_then(Value::as_object) {
        for (key, value) in weights {
            let raw_weight = value.get("weight").unwrap_or(value).as_f64().unwrap_or(1.0);
            out.insert(
                key.clone(),
                if raw_weight > 1.0 {
                    raw_weight / 100.0
                } else {
                    raw_weight
                },
            );
        }
    }
    out
}

fn rule_match_score(
    data: &Value,
    text: &str,
    weights: &BTreeMap<String, f64>,
) -> (f64, Vec<String>) {
    let mut score = 0.0;
    let mut sources = Vec::new();
    for rule in list(data, "detection_rules") {
        match rule
            .get("signal")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "keyword" => {
                let keyword = string_field(&rule, "text").to_lowercase();
                let min_count = rule.get("min_count").and_then(Value::as_u64).unwrap_or(1) as usize;
                if !keyword.is_empty() && text.matches(&keyword).count() >= min_count {
                    score += rule.get("weight").and_then(Value::as_f64).unwrap_or(1.0);
                    sources.push(format!("rule.keyword:{}", string_field(&rule, "text")));
                }
            }
            "gameplay_system" => {
                let key = string_field(&rule, "key");
                let min_weight = rule
                    .get("min_weight")
                    .and_then(Value::as_f64)
                    .unwrap_or_default();
                if weights.get(&key).copied().unwrap_or_default() >= min_weight {
                    score += rule.get("weight").and_then(Value::as_f64).unwrap_or(1.0);
                    sources.push(format!("rule.gameplay_system:{key}"));
                }
            }
            _ => {}
        }
    }
    (score, sources)
}

fn install_subtype(
    req: &mut BTreeMap<String, ArchetypeRequirementNodes>,
    keywords: &mut BTreeMap<String, Vec<String>>,
    data: &mut BTreeMap<String, Value>,
    payload: Value,
) {
    let id = string_field(&payload, "archetype_id");
    if id.is_empty() {
        return;
    }
    let mut p0 = Vec::new();
    let mut p1 = Vec::new();
    for parent in string_list(&payload, "parent_archetypes") {
        if let Some(parent_nodes) = req.get(&parent) {
            p0.extend(parent_nodes.p0.clone());
            p1.extend(parent_nodes.p1.clone());
        }
    }
    if let Some(nodes) = payload.get("requirement_nodes") {
        p0.extend(string_list(nodes, "p0"));
        p1.extend(string_list(nodes, "p1"));
    }
    req.insert(
        id.clone(),
        ArchetypeRequirementNodes {
            p0: unique(p0),
            p1: unique(p1),
        },
    );
    let mut kw = string_list(&payload, "keywords");
    for rule in list(&payload, "detection_rules") {
        if rule.get("signal").and_then(Value::as_str) == Some("keyword") {
            if let Some(text) = first_str(&rule, &["text"]) {
                kw.push(text);
            }
        }
    }
    keywords.insert(id.clone(), unique(kw));
    data.insert(id, payload);
}

fn unique(items: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect()
}
fn acceptance_refs(dna: &Value) -> Vec<String> {
    list(dna, "acceptance_scenarios")
        .iter()
        .enumerate()
        .map(|(i, s)| {
            first_str(s, &["scenario_id"]).unwrap_or_else(|| ref_path("acceptance_scenarios", i))
        })
        .collect()
}
fn categories(items: &[Value]) -> BTreeSet<String> {
    items
        .iter()
        .filter_map(|item| {
            item.get("category_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}
fn filter_category(items: &[Value], categories: &[&str]) -> Vec<Value> {
    items
        .iter()
        .filter(|item| {
            item.get("category_id")
                .and_then(Value::as_str)
                .is_some_and(|value| categories.contains(&value))
        })
        .cloned()
        .collect()
}
fn asset_strategy(
    id: &str,
    source_ref: &str,
    role: &str,
    category: &str,
    consumer: &str,
    path: &str,
    final_required: bool,
    ui_slice: bool,
    audio_only: bool,
) -> Value {
    json!({"strategy_id":id,"source_ref":source_ref,"asset_role":role,"category_id":category,"consumer_system":consumer,"unity_target_path":path,"placeholder_allowed":true,"final_asset_required":final_required,"ui_slice_required":ui_slice,"audio_placeholder_only":audio_only})
}
fn with_alignment(item: &Value, status: &str, layer: &str) -> Value {
    let mut out = item.clone();
    out["alignment_status"] = json!(status);
    out["layer"] = json!(layer);
    out
}
fn set_default(value: &mut Value, key: &str, default: Value) {
    if value.get(key).is_none() || value.get(key).is_some_and(Value::is_null) {
        value[key] = default;
    }
}
fn push_unique(value: &mut Value, key: &str, item: &str) {
    if !value.get(key).is_some_and(Value::is_array) {
        value[key] = json!([]);
    }
    if let Some(items) = value.get_mut(key).and_then(Value::as_array_mut) {
        if !items.iter().any(|existing| existing.as_str() == Some(item)) {
            items.push(json!(item));
        }
    }
}
fn asset_type_from_category(category: &str) -> String {
    match category {
        "scene_space" => "environment",
        "ui_hud" | "icon_resource" => "ui",
        "feedback_vfx" | "state_variant" => "effect",
        "audio_placeholder" => "audio_placeholder",
        _ => "art_asset",
    }
    .to_string()
}
fn dimensions_for_asset_type(asset_type: &str) -> Value {
    match asset_type {
        "ui" | "icon" => json!({"width":256,"height":256}),
        "effect" | "vfx" => json!({"width":512,"height":512}),
        "audio_placeholder" => json!({"width":0,"height":0}),
        _ => json!({"width":1024,"height":1024}),
    }
}

fn tower_defense_data() -> Value {
    json!({"archetype_id":"tower_defense","parent_archetypes":["strategy"],"keywords":["tower defense","lane defense","plants vs. zombies","plants vs zombies","grid defense","enemy waves","sun economy","plant roles","防线","塔防","格子","路线","波次","阳光","植物","僵尸"],"detection_rules":[{"signal":"keyword","text":"lane defense","min_count":1,"weight":2},{"signal":"keyword","text":"格子","min_count":1,"weight":1},{"signal":"keyword","text":"波次","min_count":1,"weight":1},{"signal":"keyword","text":"阳光","min_count":1,"weight":1},{"signal":"gameplay_system","key":"resource_economy","min_weight":0.1,"weight":0.5},{"signal":"gameplay_system","key":"objective","min_weight":0.1,"weight":0.5}],"requirement_nodes":{"p0":["core_loop_decision","input_control_decision","objective_system_decision","hud_feedback_decision","data_test_design_decision"],"p1":["build_system_decision","progression_system_decision","art_direction_decision"]},"required_systems":[{"system_id":"lane_grid_system"}],"required_entities":[{"entity_id":"grid_tile"}],"required_player_actions":[{"action_id":"place_defender","state_change":"grid_occupancy_changes"}],"required_resources":[{"resource_id":"combat_resource"}],"required_objectives":[{"objective_id":"survive_waves","completion_condition":"all_waves_cleared"}],"minimum_playable_assets":[{"asset_role":"defender_sprite","consumer":"placement_system"}],"style_compatibility":{"required_readability":["lane_boundaries"]},"open_questions":[{"question_id":"lane_count"}],"program_task_templates":[{"task_id":"GridLaneManager"}],"acceptance_scenarios":[{"scenario_id":"place_defender_and_survive_wave"}]})
}
fn narrative_puzzle_data() -> Value {
    json!({"archetype_id":"narrative_puzzle","parent_archetypes":["narrative","management"],"keywords":["papers please","papers, please","document inspection","passport","checkpoint","rule escalation","moral pressure","family economy","daily audit","证件","文书","审核","检查站","规则递增","道德压力","家庭经济"],"detection_rules":[{"signal":"keyword","text":"document inspection","min_count":1,"weight":2},{"signal":"keyword","text":"passport","min_count":1,"weight":1},{"signal":"keyword","text":"证件","min_count":1,"weight":1},{"signal":"keyword","text":"规则递增","min_count":1,"weight":1},{"signal":"gameplay_system","key":"input_control","min_weight":0.1,"weight":0.5},{"signal":"gameplay_system","key":"resource_economy","min_weight":0.1,"weight":0.5}],"requirement_nodes":{"p0":["core_loop_decision","input_control_decision","objective_system_decision","ux_flow_decision","hud_feedback_decision","data_test_design_decision"],"p1":["narrative_content_decision","retention_mid_long_goal_decision","art_direction_decision"]},"required_systems":[{"system_id":"document_inspection_system"}],"required_entities":[{"entity_id":"document"}],"required_player_actions":[{"action_id":"inspect_document","state_change":"focused_evidence_changes"}],"required_resources":[{"resource_id":"time_remaining"}],"required_objectives":[{"objective_id":"process_cases","completion_condition":"workday_ends"}],"minimum_playable_assets":[{"asset_role":"document_panel","consumer":"document_inspection_system"}],"style_compatibility":{"required_readability":["document_text_legibility"]},"open_questions":[{"question_id":"first_day_rule_count"}],"program_task_templates":[{"task_id":"DocumentInspectionController"}],"acceptance_scenarios":[{"scenario_id":"inspect_and_resolve_case"}]})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archetype_detection_and_requirements_match_python_tests() {
        let catalog = ArchetypeCatalog::default();
        let tower = detect_archetype(
            &json!({"referenceGame":"Plants vs. Zombies","referenceArchetype":"2D lane defense casual strategy"}),
            &[String::from("格子防线、阳光经济、植物职能、敌人波次")],
            &catalog,
        );
        assert_eq!(tower.archetype, "tower_defense");
        assert_eq!(tower.confidence, "high");
        let narrative = detect_archetype(
            &json!({"referenceGame":"Papers, Please","referenceArchetype":"2D document inspection narrative simulation"}),
            &[String::from("证件审核、规则递增、道德压力、家庭经济")],
            &catalog,
        );
        assert_eq!(narrative.archetype, "narrative_puzzle");
        let weighted = detect_archetype(
            &json!({"gameplaySystems":{"selected":["resource_economy","objective"],"weights":{"resource_economy":{"weight":20},"objective":{"weight":20}}}}),
            &[String::from("格子路线和敌人波次")],
            &catalog,
        );
        assert!(
            weighted
                .sources
                .iter()
                .any(|source| source.starts_with("rule.gameplay_system:"))
        );
        let req =
            build_archetype_requirements(&json!({"archetype":"tower_defense"}), &[], &catalog);
        assert_eq!(req["parent_archetypes"], json!(["strategy"]));
        assert!(
            req["archetype_p1_nodes"]
                .as_array()
                .unwrap()
                .contains(&json!("build_system_decision"))
        );
        assert!(
            req["required_contracts"]
                .as_array()
                .unwrap()
                .contains(&json!("ui_flow_contract"))
        );
        let fallback = build_archetype_requirements(
            &json!({"dimension":"2D"}),
            &[String::from("短流程")],
            &catalog,
        );
        assert_eq!(
            fallback["warnings"][0]["code"],
            json!("ARCHETYPE_FALLBACK_GENERIC")
        );
    }

    #[test]
    fn program_and_art_semantic_contracts_cover_gate_codes() {
        let dna = sample_dna();
        let capability = build_program_capability_contract(&dna);
        assert_eq!(capability["blocking_issues"], json!([]));
        assert_eq!(
            build_program_semantic_coverage_report(&dna, &capability)["status"],
            json!("passed")
        );
        let blocked = build_program_capability_contract(
            &json!({"player_actions":[{"action_id":"click_button"}]}),
        );
        assert!(
            blocked["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == json!("ACTION_HAS_NO_STATE_CHANGE"))
        );
        let (taxonomy, matrix) =
            build_art_taxonomy_and_strategy(&dna, &json!({}), Some(&json!({"assets":[]})));
        assert_eq!(taxonomy["blockers"], json!([]));
        assert_eq!(matrix["blockers"], json!([]));
        let categories = categories(matrix["assets"].as_array().unwrap());
        for category in REQUIRED_ART_CATEGORIES {
            assert!(categories.contains(*category));
        }
    }

    #[test]
    fn alignment_style_and_task_semanticizers_match_python_tests() {
        let seed = build_semantic_coverage_seed(&sample_dna());
        assert_eq!(seed["matrix_state"], json!("seed"));
        let (report, matrix) = build_semantic_alignment_outputs(
            &json!({"project_signature":"sig"}),
            &json!({"coverage_items":[{"coverage_id":"program:Grid","status":"covered"}]}),
            &json!({"coverage_items":[{"coverage_id":"art:plant","status":"placeholder_only"}]}),
        );
        assert_eq!(matrix["matrix_state"], json!("final"));
        assert_eq!(report["status"], json!("blocked"));
        assert!(
            report["blockers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == json!("PLACEHOLDER_ONLY_ALIGNMENT"))
        );
        let (blocked, ack) = build_style_fit_report(
            &json!({"detected_archetype":"tower_defense","style_compatibility":{"required_readability":["lane_boundaries"]}}),
            Some(&json!({"recommended_style_id":"cinematic_realism"})),
            "",
            "",
        );
        assert_eq!(blocked["status"], json!("blocked"));
        assert_eq!(ack["status"], json!("required"));
        let (passed, ack) = build_style_fit_report(
            &json!({"detected_archetype":"tower_defense"}),
            Some(&json!({"recommended_style_id":"cinematic_realism"})),
            "",
            "用户确认需要电影写实测试。",
        );
        assert_eq!(passed["status"], json!("passed"));
        assert_eq!(ack["status"], json!("acknowledged"));

        let mut program_tasks =
            vec![json!({"task_id":"DEV-001","title":"RuntimeBootstrap","source_refs":[]})];
        let capabilities = json!({"project_signature":"sig","capabilities":[{"capability_id":"GridManager","required":true},{"capability_id":"WaveSpawner","required":true}]});
        let summary = enrich_program_tasks_with_semantics(&mut program_tasks, &capabilities);
        assert_eq!(summary["generic_ratio"], json!(0.0));
        assert_eq!(
            build_program_semantic_coverage_matrix(&program_tasks, &capabilities)["blockers"],
            json!([])
        );
        assert_eq!(
            program_tasks[0]["project_semantic_refs"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|item| item.as_str().map(str::to_string))
                .collect::<BTreeSet<_>>(),
            ["GridManager".to_string(), "WaveSpawner".to_string()]
                .into_iter()
                .collect()
        );

        let mut art_tasks = vec![
            json!({"task_id":"ART-001","asset_id":"plant","asset_type":"ui","consumer_system":"hud","unity_target_path":"Assets/AutoDesign/Art/Source/plant.png"}),
        ];
        let strategy = json!({"project_signature":"sig","assets":[{"asset_role":"plant_sprite","final_asset_required":true}]});
        let art_summary = enrich_art_tasks_with_semantics(&mut art_tasks, &strategy);
        assert_eq!(art_summary["blockers"], json!([]));
        assert_eq!(art_tasks[0]["source_semantics"], json!(["plant_sprite"]));
        assert_eq!(
            build_art_semantic_coverage_matrix(&art_tasks, &strategy)["blockers"],
            json!([])
        );
    }

    #[test]
    fn art_strategy_task_backfill_adds_missing_tasks() {
        let mut tasks = Vec::new();
        let summary = ensure_art_tasks_cover_asset_strategy(
            &mut tasks,
            &json!({"assets":[{"asset_role":"plant_sprite","category_id":"character_unit","consumer_system":"hud","unity_target_path":"Assets/AutoDesign/Art/Source/plant.png","final_asset_required":true}]}),
            "flat_readable",
        );
        assert_eq!(summary["added_task_count"], json!(1));
        assert_eq!(tasks[0]["asset_type"], json!("art_asset"));
        assert_eq!(tasks[0]["dimensions"], json!({"width":1024,"height":1024}));
    }

    fn sample_dna() -> Value {
        json!({"project_signature":"sig","core_entities":[{"entity_id":"grid_tile","role":"placement_slot"}],"runtime_systems":[{"system_id":"lane_grid_system"}],"player_actions":[{"action_id":"place_defender","state_change":"grid_occupancy_changes"}],"resources":[{"resource_id":"sun"}],"objectives":[{"objective_id":"survive_waves","completion_condition":"all_waves_cleared"}],"acceptance_scenarios":[{"scenario_id":"first_flow"}],"ui_surfaces":[{"surface_id":"hud"}],"asset_needs":[{"asset_role":"enemy_sprite","consumer":"wave_spawner"}]})
    }
}
