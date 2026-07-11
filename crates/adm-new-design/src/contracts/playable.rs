use adm_new_contracts::ArtifactLocale;
use serde_json::{Value, json};

use super::common::{
    first_str, get_str, is_empty_value, list, matches_keywords, now_iso, object_from_value,
    selection_label, selection_source_refs,
};

pub const PLAYABLE_CONTRACT_VERSION: &str = "2.0";
pub const PLAYABLE_CONTRACT_DIR: &str = "playable_contracts";
pub const AUDIO_PLACEHOLDER_PATH: &str = "Assets/Audio/.audio_placeholder";
pub const DEFAULT_START_SCENE: &str = "Assets/Scenes/DemoScene.unity";

pub fn build_playable_contract_bundle_from_decisions(
    decisions: &Value,
    profile: &Value,
    archetype_requirements: &Value,
) -> Value {
    build_playable_contract_bundle_from_decisions_with_locale(
        decisions,
        profile,
        archetype_requirements,
        ArtifactLocale::default(),
    )
}

pub fn build_playable_contract_bundle_from_decisions_with_locale(
    decisions: &Value,
    profile: &Value,
    archetype_requirements: &Value,
    artifact_locale: ArtifactLocale,
) -> Value {
    let selections = structured_selections_from_decisions(decisions);
    let parsed = json!({
        "source": first_str(decisions, &["source"]).if_empty("decisions.json".to_string()),
        "selections": selections,
    });
    let mut bundle = build_playable_contract_bundle_with_locale(&parsed, artifact_locale);
    annotate_structured_bundle(
        &mut bundle,
        decisions,
        profile,
        archetype_requirements,
        &parsed["selections"],
    );
    // The generic builder validates its initial inference mode. Structured D4
    // decisions replace that provenance, so recompute completeness after the
    // annotation instead of retaining a synthetic abstract-inference review.
    bundle["design_completeness_report"] = validate_playable_contract_bundle(&bundle);
    if parsed["selections"]
        .as_array()
        .map(Vec::is_empty)
        .unwrap_or(true)
    {
        let blocker = json!({
            "code": "STRUCTURED_DECISIONS_EMPTY",
            "message": if artifact_locale == ArtifactLocale::ZhCn {
                "D4 结构化决策至少必须包含一个已确认的选中选项。"
            } else {
                "D4 structured decisions must contain at least one confirmed selected option."
            },
            "source_refs": [get_str(&parsed, "source")],
            "required_by_steps": ["D4", "Step02", "Step03", "Step13"],
        });
        let issues =
            bundle["design_completeness_report"]["playable_completeness"]["blocking_issues"]
                .as_array_mut()
                .expect("blocking issues array");
        issues.push(blocker);
        bundle["design_completeness_report"]["playable_completeness"]["valid"] = json!(false);
        bundle["design_completeness_report"]["status"] = json!("blocked");
    }
    bundle
}

pub fn build_playable_contract_bundle(parsed: &Value) -> Value {
    build_playable_contract_bundle_with_locale(parsed, ArtifactLocale::default())
}

pub fn build_playable_contract_bundle_with_locale(
    parsed: &Value,
    artifact_locale: ArtifactLocale,
) -> Value {
    let generated_at = now_iso();
    let selections = list(parsed, "selections");
    let source = contract_source_ref(parsed);
    let project_id = project_id(parsed);
    let actions = build_action_verbs(&selections);
    let systems = runtime_systems(&selections, &actions);
    let primary_goal = primary_goal(&selections);
    let first_action = actions.first().cloned().unwrap_or_else(default_action);
    let core_playable = json!({
        "schema_version": PLAYABLE_CONTRACT_VERSION,
        "generated_at": generated_at,
        "project_id": project_id,
        "project_type": "unity",
        "source": source,
        "generation_mode": "inferred_from_design_selections",
        "genre_profile": {
            "genre_id": "generic_playable",
            "selected_option_count": selections.len(),
        },
        "player_model": {
            "controlled_entity": "primary_player_actor",
            "input_devices": ["keyboard_mouse", "touch_or_controller_optional"],
            "first_minute_goal": primary_goal,
            "main_objective": primary_goal,
        },
        "core_loop": {
            "loop_id": "first_playable_loop",
            "start_state": "demo_scene_loaded",
            "player_action": first_action["action_id"],
            "state_change": "demo_state.progress increments or visible state changes",
            "feedback": "game_hud.status_message and visible scene update",
            "reward_or_progress": "demo objective progress advances",
            "repeat_condition": "objective is not complete",
            "completion_condition": "demo_state.progress reaches demo_target",
        },
        "action_verbs": actions,
        "state_model": [{
            "state_id": "demo_state",
            "fields": [
                {"field": "current_objective", "type": "string"},
                {"field": "progress", "type": "integer", "default": 0},
                {"field": "demo_target", "type": "integer", "default": 1},
                {"field": "completed", "type": "boolean", "default": false},
            ],
        }],
        "runtime_systems": systems,
        "demo_flow_id": "first_playable_demo_flow",
        "required_contracts": [
            "demo_flow_contract.json",
            "runtime_data_contract.json",
            "ui_flow_contract.json",
            "scene_bootstrap_contract.json",
            "asset_mount_contract.json",
            "audio_requirements_contract.json",
            "playable_acceptance_contract.json",
        ],
    });
    let demo_flow = json!({
        "schema_version": PLAYABLE_CONTRACT_VERSION,
        "generated_at": generated_at,
        "demo_flow_id": "first_playable_demo_flow",
        "start_scene": DEFAULT_START_SCENE,
        "entry_condition": "Unity Play starts in the configured first scene.",
        "steps": [
            demo_step("load_scene", "Start the demo scene.", "Play", "demo_state is initialized", "game_hud is visible"),
            demo_step("read_objective", "Read the current objective.", "none", "objective is active", "objective_panel displays the first objective"),
            demo_step("perform_primary_action", first_action["display_name"].as_str().unwrap_or("Primary action"), first_action["input_binding"].as_str().unwrap_or("PrimaryAction"), "demo_state.progress changes", "status_panel reports progress"),
            demo_step("observe_feedback", "Observe visible feedback.", "none", "visible feedback state is updated", "status message changes"),
            demo_step("complete_objective", "Reach the demo target.", first_action["input_binding"].as_str().unwrap_or("PrimaryAction"), "demo_state.completed becomes true", "completion panel or status is visible"),
        ],
        "completion_condition": "demo_state.completed == true",
        "failure_or_blocked_states": ["missing_runtime_root", "missing_ui_root", "missing_input_binding"],
        "expected_visible_results": ["MainCamera renders visible content", "game_hud is visible"],
        "playmode_acceptance": [
            "playmode_demo_scene_starts",
            "playmode_primary_action_changes_state",
            "playmode_demo_completion_reachable",
        ],
    });
    let runtime_data = json!({
        "schema_version": PLAYABLE_CONTRACT_VERSION,
        "generated_at": generated_at,
        "tables": [{
            "table_id": "default_gameplay_data",
            "path": "Assets/Config/default_gameplay_data.json",
            "purpose": if artifact_locale == ArtifactLocale::ZhCn {
                "保存首个可玩演示的目标、进度与完成状态。"
            } else {
                "Store objective, progress, and completion state for the first playable demo."
            },
            "consumer_systems": ["RuntimeRoot", "ObjectiveTracker", "game_hud"],
            "records": [{
                "record_id": "default",
                "current_objective": "complete_demo_objective",
                "progress": 0,
                "demo_target": 1,
                "completed": false,
            }],
            "required_fields": ["current_objective", "progress", "demo_target", "completed"],
        }],
        "entities": [{
            "entity_id": "demo_state",
            "entity_type": "runtime_state",
            "source_table": "default_gameplay_data",
            "fields": [
                {"field": "current_objective", "type": "string", "required": true},
                {"field": "progress", "type": "integer", "required": true, "default": 0},
                {"field": "demo_target", "type": "integer", "required": true, "default": 1},
                {"field": "completed", "type": "boolean", "required": true, "default": false},
            ],
        }],
        "relations": [],
        "state_models": [{
            "model_id": "demo_state",
            "owner_system": "RuntimeRoot",
            "fields": ["current_objective", "progress", "demo_target", "completed"],
            "initial_values": {
                "current_objective": "complete_demo_objective",
                "progress": 0,
                "demo_target": 1,
                "completed": false,
            },
        }],
        "consumer_systems": ["RuntimeRoot", "ObjectiveTracker", "game_hud"],
    });
    let ui_flow = json!({
        "schema_version": PLAYABLE_CONTRACT_VERSION,
        "generated_at": generated_at,
        "ui_framework": "Unity UGUI",
        "screens": [{
            "screen_id": "game_hud",
            "purpose": if artifact_locale == ArtifactLocale::ZhCn {
                "显示目标、进度、状态和主动作反馈。"
            } else {
                "Display objective, progress, status, and primary-action feedback."
            },
            "scene_context": DEFAULT_START_SCENE,
            "required": true,
            "panels": ["objective_panel", "status_panel"],
            "controls": [{
                "control_id": "primary_action",
                "action_binding": first_action["input_binding"],
                "action_id": first_action["action_id"],
            }],
            "visible_on_start": true,
            "open_close_rules": if artifact_locale == ArtifactLocale::ZhCn {
                "演示场景处于活动状态时保持可见。"
            } else {
                "Visible while the demo scene is active."
            },
            "acceptance_tests": ["playmode_demo_scene_starts", "playmode_primary_action_changes_state"],
            "required_widgets": ["objective_panel", "status_panel", "primary_action"],
        }],
        "data_bindings": [
            {"source": "demo_state.current_objective", "target": "objective_panel.text"},
            {"source": "demo_state.progress", "target": "status_panel.progress"},
        ],
        "playmode_checks": ["playmode_demo_scene_starts", "playmode_primary_action_changes_state"],
        "screen_graph": [{"from": "game_hud", "to": "game_hud", "trigger": "demo_state_changed"}],
        "hud": {"screen_id": "game_hud", "widgets": ["objective_panel", "status_panel", "primary_action"]},
        "input_entry_points": [{
            "control_id": "primary_action",
            "action_binding": first_action["input_binding"],
            "screen_id": "game_hud",
        }],
        "empty_states": [],
    });
    let scene_bootstrap = json!({
        "schema_version": PLAYABLE_CONTRACT_VERSION,
        "generated_at": generated_at,
        "engine": "unity",
        "start_scene": DEFAULT_START_SCENE,
        "build_settings_policy": {"make_start_scene_first": true},
        "camera": {
            "role": "main_gameplay_camera",
            "projection": "orthographic",
            "position": [0, 0, -10],
            "rotation": [0, 0, 0],
            "orthographic_size": 5,
            "clear_flags": "solid_color",
            "background_color": "#20242A",
            "required": true,
        },
        "runtime_roots": [{
            "name": "RuntimeRoot",
            "required_components": ["Transform"],
            "required": true,
        }],
        "initial_objects": [
            {"name": "ObjectiveTracker", "parent": "RuntimeRoot", "required": true},
            {"name": "FallbackVisualRoot", "parent": "RuntimeRoot", "required": true},
        ],
        "ui_roots": [{
            "name": "UIRoot",
            "screen": "game_hud",
            "canvas_mode": "screen_space_overlay",
            "required": true,
        }],
        "data_loaders": [{"name": "PlayableRuntimeDataLoader", "table_id": "default_gameplay_data"}],
        "input_roots": [{
            "name": "InputRouter",
            "actions": [first_action["input_binding"]],
            "required": true,
        }],
        "event_system": {"name": "EventSystem", "required": true},
        "objective_tracker": {"name": "ObjectiveTracker", "state_model": "demo_state", "required": true},
        "fallback_visuals": [{
            "name": "FallbackVisualRoot",
            "strategy": "generated_primitive_mesh",
            "required": true,
        }],
        "scene_acceptance": [
            "playmode_demo_scene_starts",
            "visible_content_verified",
            "build_settings_contains_start_scene",
        ],
    });
    let asset_mount = json!({
        "schema_version": PLAYABLE_CONTRACT_VERSION,
        "generated_at": generated_at,
        "assets": [
            {
                "asset_id": "asset_fallback_visual",
                "asset_type": "fallback_visual",
                "target_path": "Assets/Art/Fallback/fallback_visual.prefab",
                "required": true,
                "consumer_system": "FallbackVisualRoot",
                "mount_point": "Assets/Art/Fallback",
                "fallback_strategy": "generated primitive mesh",
                "acceptance_check": "visible_content_verified",
            },
            {
                "asset_id": "asset_audio_placeholder",
                "asset_type": "audio_placeholder",
                "target_path": AUDIO_PLACEHOLDER_PATH,
                "required": true,
                "consumer_system": "AudioEventRegistry",
                "mount_point": "Assets/Audio",
                "fallback_strategy": "silent playback",
                "acceptance_check": "audio_placeholder_file_exists",
            },
        ],
    });
    let audio_requirements = json!({
        "schema_version": PLAYABLE_CONTRACT_VERSION,
        "generated_at": generated_at,
        "audio_generation_status": "placeholder_only",
        "events": [
            {
                "audio_event_id": "ui_primary_action",
                "event_type": "ui",
                "trigger": first_action["action_id"],
                "priority": "recommended",
                "runtime_blocking": false,
                "placeholder_file": AUDIO_PLACEHOLDER_PATH,
                "future_ai_prompt": "Generate a short UI confirmation sound matching the final art direction.",
            },
            {
                "audio_event_id": "objective_complete",
                "event_type": "stinger",
                "trigger": "demo_state.completed",
                "priority": "recommended",
                "runtime_blocking": false,
                "placeholder_file": AUDIO_PLACEHOLDER_PATH,
                "future_ai_prompt": "Generate a concise objective completion stinger.",
            },
        ],
    });
    let audio_event_map = json!({
        "schema_version": PLAYABLE_CONTRACT_VERSION,
        "generated_at": generated_at,
        "bindings": [
            {
                "system_id": "game_hud",
                "state_or_action": first_action["action_id"],
                "audio_event_id": "ui_primary_action",
                "fallback_behavior": "silent",
            },
            {
                "system_id": "runtime_system_01",
                "state_or_action": "demo_state.completed",
                "audio_event_id": "objective_complete",
                "fallback_behavior": "silent",
            },
        ],
    });
    let audio_placeholder_manifest = json!({
        "schema_version": PLAYABLE_CONTRACT_VERSION,
        "generated_at": generated_at,
        "placeholder_files": [{
            "path": AUDIO_PLACEHOLDER_PATH,
            "file_type": "empty_marker",
            "required_now": true,
            "replace_later": true,
        }],
    });
    let playable_acceptance = json!({
        "schema_version": PLAYABLE_CONTRACT_VERSION,
        "generated_at": generated_at,
        "editmode_checks": ["contract_schema_valid"],
        "playmode_checks": [
            playmode_check("playmode_demo_scene_starts", &["load_scene", "wait_one_frame"], "game_hud or FallbackVisualRoot", "demo_state initialized", 10),
            playmode_check("playmode_primary_action_changes_state", &["load_scene", first_action["action_id"].as_str().unwrap_or("action_01")], "status_panel", "demo_state.progress > 0", 10),
            playmode_check("playmode_demo_completion_reachable", &["load_scene", first_action["action_id"].as_str().unwrap_or("action_01"), first_action["action_id"].as_str().unwrap_or("action_01")], "completion status", "demo_state.completed == true", 15),
        ],
        "visual_checks": ["visible_content_verified"],
        "interaction_checks": ["primary_action_has_input_or_ui_entry"],
        "data_progression_checks": ["demo_progress_updates"],
        "build_checks": ["unity_batchmode_compile"],
    });
    let mut bundle = json!({
        "artifact_locale": artifact_locale,
        "core_playable_contract": core_playable,
        "demo_flow_contract": demo_flow,
        "runtime_data_contract": runtime_data,
        "ui_flow_contract": ui_flow,
        "scene_bootstrap_contract": scene_bootstrap,
        "asset_mount_contract": asset_mount,
        "audio_requirements_contract": audio_requirements,
        "audio_event_map": audio_event_map,
        "audio_placeholder_manifest": audio_placeholder_manifest,
        "playable_acceptance_contract": playable_acceptance,
    });
    let report = validate_playable_contract_bundle(&bundle);
    bundle["design_completeness_report"] = report;
    apply_artifact_locale(&mut bundle, artifact_locale);
    if artifact_locale == ArtifactLocale::ZhCn {
        localize_playable_value(&mut bundle);
    }
    bundle
}

fn apply_artifact_locale(bundle: &mut Value, artifact_locale: ArtifactLocale) {
    let Some(root) = bundle.as_object_mut() else {
        return;
    };
    root.insert("artifact_locale".to_string(), json!(artifact_locale));
    for value in root.values_mut() {
        if let Some(object) = value.as_object_mut() {
            object.insert("artifact_locale".to_string(), json!(artifact_locale));
        }
    }
}

fn localize_playable_value(value: &mut Value) {
    match value {
        Value::String(text) => {
            if let Some(localized) = playable_zh_cn_text(text) {
                *text = localized.to_string();
            }
        }
        Value::Array(items) => {
            for item in items {
                localize_playable_value(item);
            }
        }
        Value::Object(object) => {
            for value in object.values_mut() {
                localize_playable_value(value);
            }
        }
        _ => {}
    }
}

fn playable_zh_cn_text(value: &str) -> Option<&'static str> {
    Some(match value {
        "Primary action" => "主要操作",
        "Interact" => "互动",
        "Unity Play starts in the configured first scene." => "Unity Play 从已配置的首个场景启动。",
        "Start the demo scene." => "启动演示场景。",
        "Read the current objective." => "查看当前目标。",
        "Observe visible feedback." => "观察可见反馈。",
        "Reach the demo target." => "达成演示目标。",
        "objective is active" => "目标已激活",
        "objective_panel displays the first objective" => "目标面板显示第一个目标",
        "visible feedback state is updated" => "可见反馈状态已更新",
        "status message changes" => "状态消息已变更",
        "completion panel or status is visible" => "完成面板或完成状态可见",
        "MainCamera renders visible content" => "MainCamera 正常渲染可见内容",
        "game_hud is visible" => "game_hud 可见",
        "demo objective progress advances" => "演示目标进度向前推进",
        "objective is not complete" => "目标尚未完成",
        "generated primitive mesh" => "生成的基础几何网格",
        "silent playback" => "静音播放",
        "Generate a short UI confirmation sound matching the final art direction." => {
            "生成一段符合最终美术方向的简短 UI 确认音效。"
        }
        "Generate a concise objective completion stinger." => "生成一段简洁的目标完成提示音。",
        "Core playable contract must define a core loop." => "核心可玩合约必须定义核心循环。",
        "At least one player action is required." => "至少需要一个玩家操作。",
        "Demo flow must contain at least five steps." => "演示流程至少必须包含五个步骤。",
        "Demo flow must define a completion condition." => "演示流程必须定义完成条件。",
        "Runtime data contract must define at least one table." => {
            "运行时数据合约至少必须定义一张表。"
        }
        "UI flow must define at least one screen." => "UI 流程至少必须定义一个界面。",
        "Scene bootstrap must define a start scene." => "场景引导合约必须定义启动场景。",
        "Scene bootstrap must define runtime roots." => "场景引导合约必须定义运行时根节点。",
        "Scene bootstrap must define UI roots or an explicit no-UI policy." => {
            "场景引导合约必须定义 UI 根节点，或明确声明无 UI 策略。"
        }
        "Asset mount contract must define required assets." => "资产挂载合约必须定义必需资产。",
        "Audio events must be specified even when audio is placeholder-only." => {
            "即使音频仅使用占位内容，也必须定义音频事件。"
        }
        "Playable acceptance must include PlayMode checks." => "可玩验收必须包含 PlayMode 检查。",
        "Playable contracts cannot pass without at least one current-project design selection." => {
            "当前项目至少需要一个设计选项，可玩合约才能通过。"
        }
        "Required assets must define target_path." => "必需资产必须定义 target_path。",
        "Audio placeholder should use the standard empty marker path." => {
            "音频占位内容应使用标准空标记路径。"
        }
        "Playable contracts were inferred from design selections; human confirmation is recommended." => {
            "可玩合约由设计选项推导而来，建议进行人工确认。"
        }
        _ => return None,
    })
}

pub fn validate_playable_contract_bundle(bundle: &Value) -> Value {
    let artifact_locale =
        ArtifactLocale::normalize(bundle.get("artifact_locale").and_then(Value::as_str));
    let mut blockers = Vec::new();
    let mut review_items = Vec::new();
    let core = bundle.get("core_playable_contract").unwrap_or(&Value::Null);
    let demo = bundle.get("demo_flow_contract").unwrap_or(&Value::Null);
    let data = bundle.get("runtime_data_contract").unwrap_or(&Value::Null);
    let ui = bundle.get("ui_flow_contract").unwrap_or(&Value::Null);
    let scene = bundle
        .get("scene_bootstrap_contract")
        .unwrap_or(&Value::Null);
    let assets = bundle.get("asset_mount_contract").unwrap_or(&Value::Null);
    let audio = bundle
        .get("audio_requirements_contract")
        .unwrap_or(&Value::Null);
    let acceptance = bundle
        .get("playable_acceptance_contract")
        .unwrap_or(&Value::Null);
    require(
        !is_empty_value(core.get("core_loop")),
        "CORE_LOOP_MISSING",
        "Core playable contract must define a core loop.",
        &mut blockers,
    );
    require(
        !is_empty_value(core.get("action_verbs")),
        "ACTIONS_MISSING",
        "At least one player action is required.",
        &mut blockers,
    );
    require(
        list(demo, "steps").len() >= 5,
        "DEMO_FLOW_TOO_SHORT",
        "Demo flow must contain at least five steps.",
        &mut blockers,
    );
    require(
        !is_empty_value(demo.get("completion_condition")),
        "DEMO_COMPLETION_MISSING",
        "Demo flow must define a completion condition.",
        &mut blockers,
    );
    require(
        !is_empty_value(data.get("tables")),
        "RUNTIME_DATA_MISSING",
        "Runtime data contract must define at least one table.",
        &mut blockers,
    );
    require(
        !is_empty_value(ui.get("screens")),
        "UI_FLOW_MISSING",
        "UI flow must define at least one screen.",
        &mut blockers,
    );
    require(
        !is_empty_value(scene.get("start_scene")),
        "START_SCENE_MISSING",
        "Scene bootstrap must define a start scene.",
        &mut blockers,
    );
    require(
        !is_empty_value(scene.get("runtime_roots")),
        "RUNTIME_ROOT_MISSING",
        "Scene bootstrap must define runtime roots.",
        &mut blockers,
    );
    require(
        !is_empty_value(scene.get("ui_roots")),
        "UI_ROOT_MISSING",
        "Scene bootstrap must define UI roots or an explicit no-UI policy.",
        &mut blockers,
    );
    require(
        !is_empty_value(assets.get("assets")),
        "ASSET_MOUNTS_MISSING",
        "Asset mount contract must define required assets.",
        &mut blockers,
    );
    require(
        !is_empty_value(audio.get("events")),
        "AUDIO_EVENTS_MISSING",
        "Audio events must be specified even when audio is placeholder-only.",
        &mut blockers,
    );
    require(
        !is_empty_value(acceptance.get("playmode_checks")),
        "PLAYMODE_CHECKS_MISSING",
        "Playable acceptance must include PlayMode checks.",
        &mut blockers,
    );
    let selected_option_count = core
        .get("genre_profile")
        .and_then(|profile| profile.get("selected_option_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if selected_option_count == 0 {
        blockers.push(json!({
            "code": "NO_DESIGN_SELECTIONS",
            "message": "Playable contracts cannot pass without at least one current-project design selection.",
        }));
    }
    for asset in list(assets, "assets") {
        if asset
            .get("required")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && is_empty_value(asset.get("target_path"))
        {
            blockers.push(json!({
                "code": "REQUIRED_ASSET_TARGET_MISSING",
                "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
                "message": "Required assets must define target_path.",
            }));
        }
        if asset.get("asset_type").and_then(Value::as_str) == Some("audio_placeholder")
            && asset.get("target_path").and_then(Value::as_str) != Some(AUDIO_PLACEHOLDER_PATH)
        {
            review_items.push(json!({
                "code": "AUDIO_PLACEHOLDER_PATH_NONSTANDARD",
                "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
                "message": "Audio placeholder should use the standard empty marker path.",
            }));
        }
    }
    if core.get("generation_mode").and_then(Value::as_str)
        == Some("inferred_from_design_selections")
    {
        review_items.push(json!({
            "code": "CONTRACT_INFERRED_FROM_ABSTRACT_DESIGN",
            "message": "Playable contracts were inferred from design selections; human confirmation is recommended.",
        }));
    }
    let coverage = json!({
        "core_playable": !core.is_null(),
        "demo_flow": !demo.is_null(),
        "runtime_data": !data.is_null(),
        "ui_flow": !ui.is_null(),
        "scene_bootstrap": !scene.is_null(),
        "asset_mount": !assets.is_null(),
        "audio_placeholder": !audio.is_null(),
        "acceptance": !acceptance.is_null(),
    });
    let coverage_count = coverage
        .as_object()
        .unwrap()
        .values()
        .filter(|value| value.as_bool() == Some(true))
        .count();
    let valid = blockers.is_empty();
    let mut report = json!({
        "schema_version": PLAYABLE_CONTRACT_VERSION,
        "generated_at": now_iso(),
        "artifact_locale": artifact_locale,
        "status": if valid { "passed" } else { "blocked" },
        "playable_completeness": {
            "valid": valid,
            "score": ((coverage_count as f64 / 8.0) * 10000.0).round() / 10000.0,
            "blocking_issues": blockers,
            "review_items": review_items,
            "contract_coverage": coverage,
        },
    });
    if artifact_locale == ArtifactLocale::ZhCn {
        localize_playable_value(&mut report);
    }
    report
}

pub fn build_playable_development_tasks(bundle: &Value, start_index: usize) -> Vec<Value> {
    if is_empty_value(bundle.get("core_playable_contract")) {
        return Vec::new();
    }
    let checks = list(
        bundle
            .get("playable_acceptance_contract")
            .unwrap_or(&Value::Null),
        "playmode_checks",
    )
    .into_iter()
    .filter_map(|item| {
        item.get("check_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    })
    .collect::<Vec<_>>();
    let specs = vec![
        (
            "PLAYABLE-DATA-001",
            "Create runtime data contract files and data loader for the first playable demo.",
            "runtime_data",
            "Assets/Config/",
            vec![
                "Assets/Config/default_gameplay_data.json",
                "Assets/Scripts/Core/PlayableRuntimeData.cs",
                "Assets/Tests/EditMode/Core/PlayableRuntimeDataTests.cs",
            ],
            vec![
                "Assets/Config/",
                "Assets/Scripts/Core/",
                "Assets/Tests/EditMode/Core/",
            ],
            "Runtime data exists, can be loaded, and includes the primary demo objective.",
        ),
        (
            "PLAYABLE-UI-001",
            "Implement the required game HUD, UI data bindings, and visible feedback states.",
            "ui_screen",
            "Assets/Scripts/UI/",
            vec![
                "Assets/Scripts/UI/GeneratedGameHud.cs",
                "Assets/Tests/EditMode/UI/GeneratedGameHudTests.cs",
            ],
            vec!["Assets/Scripts/UI/", "Assets/Tests/EditMode/UI/"],
            "HUD shows objective, progress, status, and primary action feedback on scene start.",
        ),
        (
            "PLAYABLE-SCENE-001",
            "Implement scene bootstrap builder from the scene bootstrap contract.",
            "scene_bootstrap",
            "Assets/Editor/AutoDesignMaker/",
            vec![
                "Assets/Editor/AutoDesignMaker/PlayableSceneBootstrapBuilder.cs",
                "Assets/Scripts/AutoDesignMaker/PlayableRuntimeRoot.cs",
            ],
            vec![
                "Assets/Editor/AutoDesignMaker/",
                "Assets/Scripts/AutoDesignMaker/",
                "Assets/Scenes/",
            ],
            "DemoScene can be generated with camera, runtime root, UI root, input root, and visible fallback content.",
        ),
        (
            "PLAYABLE-AUDIO-001",
            "Create audio placeholder registry and stable audio event map without requiring generated audio.",
            "audio_placeholder",
            "Assets/Config/",
            vec![
                "Assets/Config/audio_event_map.json",
                "Assets/Scripts/Audio/AudioPlaceholderRegistry.cs",
            ],
            vec!["Assets/Config/", "Assets/Scripts/Audio/", "Assets/Audio/"],
            "Audio events are registered and missing real clips fall back to silent playback.",
        ),
        (
            "PLAYABLE-TEST-001",
            "Implement PlayMode acceptance checks for visible, interactive, state-changing demo flow.",
            "playmode_test",
            "Assets/Tests/PlayMode/",
            vec!["Assets/Tests/PlayMode/PlayableDemoFlowTests.cs"],
            vec!["Assets/Tests/PlayMode/"],
            "PlayMode checks cover scene start, visible HUD/content, primary action state change, and demo completion.",
        ),
    ];
    specs
        .into_iter()
        .enumerate()
        .map(|(offset, (requirement_id, title, category, target_path, output_files, allowed_write_paths, acceptance))| {
            let task_id = format!("PLAY-{:03}", start_index + offset);
            json!({
                "task_id": task_id,
                "task_type": category,
                "requirement_id": requirement_id,
                "title": title,
                "phase": "core_playable",
                "category": category,
                "priority": "P0",
                "target_path": target_path,
                "output_files": output_files,
                "allowed_write_paths": allowed_write_paths,
                "verification_commands": verification(category != "audio_placeholder"),
                "package_changes": [],
                "source_refs": [get_str(&bundle["core_playable_contract"], "source").if_empty("playable_contracts".to_string())],
                "acceptance": acceptance,
                "dependencies": [],
                "execution_policy": "ai_edit_declared_files_only",
                "status": "planned",
                "contract_refs": {
                    "core_playable": "stage_02/playable_contracts/core_playable_contract.json",
                    "demo_flow": "stage_02/playable_contracts/demo_flow_contract.json",
                    "runtime_data": "stage_02/playable_contracts/runtime_data_contract.json",
                    "ui_flow": "stage_02/playable_contracts/ui_flow_contract.json",
                    "scene_bootstrap": "stage_02/playable_contracts/scene_bootstrap_contract.json",
                    "audio": "stage_02/playable_contracts/audio_requirements_contract.json",
                    "acceptance_checks": checks,
                },
            })
        })
        .collect()
}

fn build_action_verbs(selections: &[Value]) -> Vec<Value> {
    let action_selections = pick_action_selections(selections);
    if action_selections.is_empty() {
        return vec![default_action()];
    }
    action_selections
        .iter()
        .enumerate()
        .map(|(index, selection)| {
            let action_id = format!("action_{:02}", index + 1);
            json!({
                "action_id": action_id,
                "display_name": selection_label(selection).if_empty(format!("Action {}", index + 1)),
                "input_binding": "PrimaryAction",
                "preconditions": ["demo_scene_loaded"],
                "state_reads": ["demo_state.current_objective"],
                "state_writes": ["demo_state.progress"],
                "success_feedback": ["ui.status_message", "scene.visible_state_change"],
                "failure_feedback": ["ui.error_message"],
                "ui_entry_points": ["game_hud.primary_action"],
                "source_refs": selection_source_refs(selection),
                "acceptance_tests": [format!("playmode_{action_id}_changes_demo_state")],
            })
        })
        .collect()
}

fn runtime_systems(selections: &[Value], actions: &[Value]) -> Vec<Value> {
    let action_ids = actions
        .iter()
        .filter_map(|action| action.get("action_id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let systems = selections
        .iter()
        .filter(|selection| matches_keywords(selection, &["系统", "system", "loop", "循环", "玩法"]))
        .take(8)
        .enumerate()
        .map(|(index, selection)| {
            json!({
                "system_id": format!("runtime_system_{:02}", index + 1),
                "display_name": selection_label(selection).if_empty(format!("Runtime System {}", index + 1)),
                "responsibility": get_str(selection, "purpose").if_empty("Runtime behavior".to_string()),
                "state_reads": ["demo_state"],
                "state_writes": ["demo_state"],
                "actions": action_ids,
                "source_refs": selection_source_refs(selection),
            })
        })
        .collect::<Vec<_>>();
    if systems.is_empty() {
        vec![json!({
            "system_id": "runtime_system_01",
            "display_name": "Demo Runtime System",
            "responsibility": "Own the first playable loop.",
            "state_reads": ["demo_state"],
            "state_writes": ["demo_state"],
            "actions": action_ids,
            "source_refs": [],
        })]
    } else {
        systems
    }
}

fn pick_action_selections(selections: &[Value]) -> Vec<Value> {
    let matched = selections
        .iter()
        .filter(|selection| {
            matches_keywords(
                selection,
                &[
                    "操作", "控制", "输入", "行动", "action", "input", "control", "interact",
                    "build", "use", "move", "attack",
                ],
            )
        })
        .take(5)
        .cloned()
        .collect::<Vec<_>>();
    if !matched.is_empty() {
        return matched;
    }
    selections
        .iter()
        .find(|selection| matches_keywords(selection, &["核心循环", "玩法", "system", "系统"]))
        .cloned()
        .map(|item| vec![item])
        .unwrap_or_else(|| selections.iter().take(1).cloned().collect())
}

fn primary_goal(selections: &[Value]) -> String {
    selections
        .iter()
        .find(|selection| {
            matches_keywords(selection, &["目标", "objective", "goal", "胜利", "完成"])
        })
        .or_else(|| {
            selections.iter().find(|selection| {
                matches_keywords(selection, &["核心循环", "core loop", "玩法", "loop"])
            })
        })
        .map(selection_label)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Complete the first playable demo objective.".to_string())
}

fn structured_selections_from_decisions(decisions: &Value) -> Vec<Value> {
    let source = get_str(decisions, "source").if_empty("decisions.json".to_string());
    iter_decision_records(decisions)
        .into_iter()
        .enumerate()
        .filter_map(|(index, decision)| {
            let state = first_str(&decision, &["decision_state", "state", "status"]).to_ascii_lowercase();
            if !state.is_empty()
                && !["selected", "completed", "confirmed", "approved"].contains(&state.as_str())
            {
                return None;
            }
            let option = decision_option_payload(&decision);
            if option.is_null() && first_str(&decision, &["label", "title"]).is_empty() {
                return None;
            }
            Some(json!({
                "id": first_str(&decision, &["id", "node_id"]).if_empty(format!("DEC-{:03}", index + 1)),
                "label": first_str(&option, &["label", "title", "name"])
                    .if_empty(first_str(&decision, &["label", "title", "name"]))
                    .if_empty(format!("Structured decision {}", index + 1)),
                "item_type": first_str(&decision, &["item_type", "itemType", "domain", "node_type", "nodeType"])
                    .if_empty("structured_decision".to_string()),
                "option": first_str(&option, &["value", "id", "label"]),
                "purpose": first_str(&option, &["purpose", "description"]).if_empty(first_str(&decision, &["purpose", "description"])),
                "source_ref": decision_source_ref(&decision, &format!("{source}:decision:{}", index + 1)),
            }))
        })
        .collect()
}

fn iter_decision_records(decisions: &Value) -> Vec<Value> {
    let mut records = Vec::new();
    for key in [
        "selected_options",
        "selectedOptions",
        "decisions",
        "nodes",
        "items",
    ] {
        match decisions.get(key) {
            Some(Value::Array(items)) => records.extend(items.clone()),
            Some(Value::Object(items)) => records.extend(items.values().cloned()),
            _ => {}
        }
    }
    if records.is_empty() {
        records.extend(
            object_from_value(decisions)
                .into_values()
                .filter(Value::is_object),
        );
    }
    records
}

fn decision_option_payload(decision: &Value) -> Value {
    for key in [
        "selected_option",
        "selectedOption",
        "primary_option",
        "primaryOption",
        "option",
    ] {
        match decision.get(key) {
            Some(Value::Object(_)) => return decision[key].clone(),
            Some(Value::String(text)) if !text.trim().is_empty() => return json!({"label": text}),
            _ => {}
        }
    }
    for key in ["selected_options", "selectedOptions"] {
        if let Some(items) = decision.get(key).and_then(Value::as_array) {
            if let Some(first) = items.first() {
                return if first.is_object() {
                    first.clone()
                } else {
                    json!({"label": first.as_str().unwrap_or("").to_string()})
                };
            }
        }
    }
    Value::Null
}

fn decision_source_ref(decision: &Value, fallback: &str) -> String {
    decision
        .get("source_refs")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| {
            first_str(decision, &["source_ref", "trace_ref", "node_id", "id"])
                .if_empty(fallback.to_string())
        })
}

fn annotate_structured_bundle(
    bundle: &mut Value,
    decisions: &Value,
    profile: &Value,
    archetype_requirements: &Value,
    selections: &Value,
) {
    let source = get_str(decisions, "source").if_empty("decisions.json".to_string());
    let selection_array = selections.as_array().cloned().unwrap_or_default();
    let source_refs = selection_array
        .iter()
        .filter_map(|selection| selection.get("source_ref").and_then(Value::as_str))
        .map(|item| Value::String(item.to_string()))
        .collect::<Vec<_>>();
    let source_refs = if source_refs.is_empty() {
        vec![Value::String(source.clone())]
    } else {
        source_refs
    };
    let contract_refs = json!({
        "decisions": source,
        "profile": get_str(profile, "source").if_empty("profile.json".to_string()),
        "archetype_requirements": get_str(archetype_requirements, "source").if_empty("archetype_requirements.json".to_string()),
    });
    if let Some(object) = bundle.as_object_mut() {
        for (key, payload) in object.iter_mut() {
            if key == "design_completeness_report" || !payload.is_object() {
                continue;
            }
            payload["source_refs"] = Value::Array(source_refs.clone());
            payload["contract_refs"] = contract_refs.clone();
            payload["generation_mode"] = json!("structured_decisions");
        }
    }
    if let Some(core) = bundle.get_mut("core_playable_contract") {
        let project_id = first_str(profile, &["project_id", "id"]);
        if !project_id.is_empty() {
            core["project_id"] = json!(project_id);
        }
        let genre_id = first_str(profile, &["genre_id", "genre"]);
        if !genre_id.is_empty() {
            core["genre_profile"]["genre_id"] = json!(genre_id);
        }
        core["archetype_requirements"] = archetype_requirements.clone();
    }
}

fn contract_source_ref(parsed: &Value) -> String {
    get_str(parsed, "source").if_empty("design_source".to_string())
}

fn project_id(parsed: &Value) -> String {
    let source = contract_source_ref(parsed);
    std::path::Path::new(&source)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "project".to_string())
}

fn demo_step(id: &str, instruction: &str, input: &str, state: &str, ui: &str) -> Value {
    json!({
        "step_id": id,
        "player_instruction": instruction,
        "required_input": input,
        "expected_state_change": state,
        "expected_ui_feedback": ui,
        "required_scene_objects": ["RuntimeRoot", "UIRoot"],
    })
}

fn playmode_check(
    id: &str,
    steps: &[&str],
    visible: &str,
    state_change: &str,
    timeout: u64,
) -> Value {
    json!({
        "check_id": id,
        "start_scene": DEFAULT_START_SCENE,
        "steps": steps,
        "expected_visible_text_or_object": visible,
        "expected_state_change": state_change,
        "timeout_seconds": timeout,
        "blocking": true,
    })
}

fn verification(include_unity: bool) -> Vec<Value> {
    let mut commands = vec![json!({
        "id": "static_csharp_contract",
        "type": "internal",
        "required": true,
        "description": "Check declared C# outputs and public type names where applicable.",
    })];
    if include_unity {
        commands.push(json!({
            "id": "unity_batchmode_compile",
            "type": "unity_batchmode",
            "required": true,
            "description": "Compile the Unity project after playable infrastructure changes.",
        }));
    }
    commands
}

fn require(condition: bool, code: &str, message: &str, blockers: &mut Vec<Value>) {
    if !condition {
        blockers.push(json!({"code": code, "message": message}));
    }
}

fn default_action() -> Value {
    json!({
        "action_id": "action_01",
        "display_name": "Primary demo action",
        "input_binding": "PrimaryAction",
        "preconditions": ["demo_scene_loaded"],
        "state_reads": ["demo_state.current_objective"],
        "state_writes": ["demo_state.progress"],
        "success_feedback": ["ui.status_message", "scene.visible_state_change"],
        "failure_feedback": ["ui.error_message"],
        "ui_entry_points": ["game_hud.primary_action"],
        "source_refs": [],
        "acceptance_tests": ["playmode_action_01_changes_demo_state"],
    })
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
