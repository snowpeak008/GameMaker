use serde_json::{Map, Value};

pub mod paths;
pub mod stage04;
pub mod stage09;
pub mod stage12;
pub mod stage13;
pub mod stage14;

pub const SCHEMA_VERSION: &str = "1.0";

pub(crate) fn object_map(value: &Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

pub(crate) fn list(value: &Value, key: &str) -> Vec<Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn first_str(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .map(value_to_string)
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
    })
}

pub(crate) fn value_to_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

pub(crate) fn is_empty_value(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => true,
        Some(Value::String(text)) => text.trim().is_empty(),
        Some(Value::Array(items)) => items.is_empty(),
        Some(Value::Object(map)) => map.is_empty(),
        Some(Value::Bool(value)) => !*value,
        Some(Value::Number(number)) => {
            number.as_i64() == Some(0) || number.as_u64() == Some(0) || number.as_f64() == Some(0.0)
        }
    }
}

pub(crate) fn truthy(value: Option<&Value>) -> bool {
    !is_empty_value(value)
}

pub(crate) fn unique_strings(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    items
        .into_iter()
        .filter(|item| !item.trim().is_empty())
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use adm_new_contracts::schema;
    use serde_json::{Value, json};

    use super::stage04::*;
    use super::stage09::*;
    use super::stage12::*;
    use super::stage13::*;
    use super::stage14::*;

    #[test]
    fn stage04_paths_and_schema_contracts_match_autodesign_policy() {
        let assets = normalize_asset_targets(&[
            json!({
                "asset_id": "ASSET-HUD-001",
                "name": "HUD Power Icon",
                "asset_type": "ui",
                "target_path": "Assets/UI/legacy_icon.png",
                "required_format": "png_with_alpha_or_unity_prefab",
                "required_size": {"width": 128, "height": 128},
                "transparency": "transparent alpha",
                "consumer_system": "game_hud",
                "mount_point": "Canvas/HUD",
                "fallback_policy": "generated production asset",
                "acceptance_check": "hud_icon_visible",
                "source_refs": ["stage_04/art_requirements_contract.json"]
            }),
            json!({
                "asset_id": "AUDIO-PLACEHOLDER-001",
                "asset_type": "audio_placeholder",
                "consumer_system": "audio_router",
                "mount_point": "AudioRoot",
                "source_refs": ["stage_02/playable_contracts/audio_requirements_contract.json"]
            }),
        ]);

        assert_eq!(
            assets[0]["unity_target_path"],
            json!("Assets/AutoDesign/Art/Source/ASSET_HUD_001.png")
        );
        assert_eq!(
            assets[0]["legacy_unity_target_path"],
            json!("Assets/UI/legacy_icon.png")
        );
        assert_eq!(
            assets[1]["unity_target_path"],
            json!("Assets/AutoDesign/Audio/Placeholders/AUDIO_PLACEHOLDER_001.placeholder")
        );

        let image_spec =
            build_image_consumable_spec(&assets, "test", vec![json!({"code": "sample"})]);
        assert_eq!(image_spec["assets"].as_array().unwrap().len(), 1);
        assert_schema(
            &image_spec,
            "knowledge/schemas/ai_design/art_pipeline/image_consumable_spec.schema.json",
        );

        let slice_contract = build_ui_slice_spec_contract(&assets, "test");
        assert_eq!(
            slice_contract["slice_specs"][0]["sprite_id"],
            json!("ASSET-HUD-001")
        );

        let import_policy = build_unity_import_policy(&assets, "test");
        assert_eq!(import_policy["unity_root"], json!("Assets/AutoDesign/"));

        let binding_seed = build_asset_usage_binding_seed(&assets, "test");
        assert_eq!(binding_seed["bindings"].as_array().unwrap().len(), 2);

        let audio_requirements = build_audio_placeholder_requirements(&assets, "test");
        assert_eq!(
            audio_requirements["requirements"].as_array().unwrap().len(),
            1
        );
    }

    #[test]
    fn stage09_enriches_art_tasks_without_overwriting_existing_fields() {
        let task = enrich_art_task(
            &json!({
                "task_id": "ART-001",
                "asset_id": "asset_hud_icon",
                "asset_type": "ui",
                "acceptance": "HUD icon visible"
            }),
            "flat_readable",
        );

        assert!(
            task["generation_prompt"]
                .as_str()
                .unwrap()
                .contains("flat_readable")
        );
        assert_eq!(
            task["slice_spec_ref"],
            json!("stage_04/ui_slice_spec_contract.json")
        );
        assert_eq!(task["acceptance_criteria"][0], json!("HUD icon visible"));
    }

    #[test]
    fn stage12_reports_preflight_quality_semantic_and_handoff_state() {
        let tasks = vec![json!({
            "task_id": "ART-001",
            "asset_id": "asset_hud_icon",
            "asset_type": "ui",
            "unity_target_path": "Assets/AutoDesign/Art/Source/hud_icon.png",
            "dimensions": {"width": 128, "height": 128},
            "consumer_system": "game_hud",
            "mount_point": "Canvas/HUD",
            "acceptance": "HUD icon visible",
            "contract_refs": {"asset_spec_contract": "stage_04/asset_spec_contract.json"},
            "source_refs": ["stage_09/art_production_task_contract.json"]
        })];
        assert_eq!(stage12_art_task_preflight(&tasks), Vec::<Value>::new());

        let blockers = stage12_art_task_preflight(&[json!({
            "task_id": "ART-LEGACY",
            "asset_id": "asset_legacy",
            "unity_target_path": "Assets/UI/legacy.png"
        })]);
        assert!(
            blockers
                .iter()
                .any(|item| { item["code"] == json!("ART_TASK_NOT_CONTRACT_BOUND") })
        );
        assert!(
            blockers
                .iter()
                .any(|item| { item["code"] == json!("ART_TASK_TARGET_OUTSIDE_AUTODESIGN_ROOT") })
        );

        let produced = vec![json!({
            "task_id": "ART-001",
            "asset_id": "asset_hud_icon",
            "target_path": "Assets/AutoDesign/Art/Source/hud_icon.png"
        })];
        let raw = build_raw_generated_asset_manifest(
            &tasks,
            &produced,
            "test",
            Some(&json!({
                "records": [{
                    "task_id": "ART-001",
                    "status": "success",
                    "result": {"output_path": "Assets/AutoDesign/Art/Source/hud_icon.png"}
                }]
            })),
        );
        assert_eq!(raw["raw_assets"][0]["available"], json!(true));

        let quality = build_image_quality_report(&raw, "test");
        assert_eq!(quality["status"], json!("passed"));

        let semantic = build_semantic_review_report(&raw, "test");
        assert_eq!(semantic["status"], json!("passed_with_review"));

        let rework = build_art_rework_queue(&quality, &semantic, "test");
        assert_eq!(rework["review_count"], json!(1));

        let processed = build_processed_asset_manifest(&tasks, &raw, "test");
        let mount = build_asset_mount_manifest(&tasks, "test");
        let binding = build_program_asset_binding_preflight(&mount, "test");
        assert_eq!(binding["ready"], json!(true));
        let handoff = build_art_handoff_manifest(
            "test", &quality, &semantic, &rework, &processed, &mount, &binding,
        );
        assert_eq!(handoff["ready_for_step13"], json!(true));
        assert_schema(
            &handoff,
            "knowledge/schemas/ai_design/art_pipeline/art_handoff_manifest.schema.json",
        );

        let slices = build_sprite_slice_result_manifest(&tasks, "test");
        assert_eq!(slices["slices"].as_array().unwrap().len(), 1);
        let ugui = build_ugui_prefab_contract(&tasks, "test");
        let prefab_request = build_ui_prefab_generation_request(&ugui, "test");
        assert_eq!(
            prefab_request["requests"][0]["status"],
            json!("ready_for_unity_editor")
        );
    }

    #[test]
    fn stage13_materialization_reports_and_stage14_acceptance_are_schema_valid() {
        let handoff = json!({
            "schema_version": "1.0",
            "generated_at": "test",
            "ready_for_step13": true,
            "blockers": [],
            "source_refs": ["stage_12/asset_mount_manifest.json"],
            "asset_count": 2,
            "mount_item_count": 2
        });
        let request = build_unity_editor_request(
            "test",
            "UnityProject",
            "outputs/artifacts/stage_13",
            &handoff,
            "Assets/Scenes/DemoScene.unity",
        );
        assert_eq!(
            request["requires_compile_before_execute_method"],
            json!(true)
        );
        assert_schema(
            &request,
            "knowledge/schemas/ai_design/art_pipeline/unity_editor_request.schema.json",
        );

        let reports = build_unity_materialization_reports(
            "test",
            &handoff,
            &json!({
                "status": "success",
                "scene_path": "Assets/Scenes/DemoScene.unity",
                "changed_files": ["Assets/AutoDesign/Prefabs/UI/Screen_hud.prefab"],
                "visible_content_verified": true,
                "blocking_issues": []
            }),
        );
        assert_eq!(
            reports["unity_art_import_report"]["status"],
            json!("success")
        );
        assert_eq!(
            reports["unity_prefab_generation_report"]["generated_prefabs"][0],
            json!("Assets/AutoDesign/Prefabs/UI/Screen_hud.prefab")
        );
        assert_schema(
            &reports["unity_art_import_report"],
            "knowledge/schemas/ai_design/art_pipeline/unity_art_import_report.schema.json",
        );

        let art_acceptance = build_art_acceptance_report(
            "test",
            &handoff,
            &reports["unity_art_import_report"],
            &reports["unity_prefab_generation_report"],
            &reports["unity_scene_mount_report"],
        );
        assert_eq!(art_acceptance["status"], json!("success"));
        assert_schema(
            &art_acceptance,
            "knowledge/schemas/ai_design/art_pipeline/art_acceptance_report.schema.json",
        );

        let level4 = build_level4_visual_checks(
            &json!({"visible_content_verified": false}),
            &[json!({"id": "UNITY-EDITOR-MISSING"})],
        );
        assert_eq!(level4["status"], json!("skipped_with_warning"));

        let level5 = build_level5_design_coverage_checks(
            &json!({
                "playable_acceptance_contract": {"playmode_checks": [{"check_id": "demo_starts"}]},
                "ui_flow_contract": {"playmode_checks": [{"check_id": "hud_visible"}]}
            }),
            &json!({"status": "success", "contract_summary": {"ui_screen_count": 1}}),
        );
        assert_eq!(level5["status"], json!("passed"));
    }

    fn assert_schema(payload: &Value, schema_relative: &str) {
        let schema_path = project_root().join(schema_relative);
        let schema = schema::load_structured_file(&schema_path).unwrap();
        let errors = schema::validate_contract(payload, &schema);
        assert!(errors.is_empty(), "{schema_relative}: {errors:?}");
    }

    fn project_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .unwrap()
    }
}
