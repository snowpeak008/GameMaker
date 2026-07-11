use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

use super::paths::{
    UNITY_ART_ATLAS_ROOT, UNITY_AUDIO_PLACEHOLDER_ROOT, UNITY_GENERATED_ROOT, UNITY_UI_PREFAB_ROOT,
    canonical_processed_path, canonical_unity_target_path, is_autodesign_path,
    is_legacy_generated_path, normalize_unity_path, prefab_path, slug,
};
use super::{SCHEMA_VERSION, first_str, list, object_map, unique_strings};

pub fn strategy_category_asset_type(category_id: &str) -> &'static str {
    match category_id {
        "character_unit" => "art_asset",
        "scene_space" => "environment",
        "ui_hud" | "icon_resource" => "ui",
        "feedback_vfx" | "state_variant" => "effect",
        "audio_placeholder" => "audio_placeholder",
        _ => "art_asset",
    }
}

pub fn normalize_asset_targets(assets: &[Value]) -> Vec<Value> {
    assets
        .iter()
        .filter(|asset| asset.is_object())
        .map(|asset| {
            let mut item = object_map(asset);
            let target = normalize_unity_path(
                first_str(asset, &["unity_target_path", "target_path"]).unwrap_or_default(),
            );
            let generated_target = canonical_unity_target_path(asset);
            item.insert(
                "legacy_unity_target_path".to_string(),
                json!(if !target.is_empty() && !is_autodesign_path(&target) {
                    target.clone()
                } else {
                    String::new()
                }),
            );
            item.insert("target_path".to_string(), json!(generated_target));
            item.insert("unity_target_path".to_string(), json!(generated_target));
            item.insert(
                "unity_path_policy".to_string(),
                json!({
                    "root": UNITY_GENERATED_ROOT,
                    "generated_by_pipeline": true,
                    "legacy_input_path_accepted": !target.is_empty() && is_legacy_generated_path(target),
                }),
            );
            Value::Object(item)
        })
        .collect()
}

pub fn merge_asset_strategy_into_assets(
    assets: &[Value],
    asset_strategy_matrix: &Value,
) -> Vec<Value> {
    let mut merged = assets
        .iter()
        .filter(|asset| asset.is_object())
        .cloned()
        .collect::<Vec<_>>();
    let mut seen_ids = merged
        .iter()
        .filter_map(|asset| first_str(asset, &["asset_id"]))
        .collect::<BTreeSet<_>>();
    let mut seen_targets = merged
        .iter()
        .filter_map(|asset| first_str(asset, &["unity_target_path", "target_path"]))
        .map(normalize_unity_path)
        .filter(|target| !target.is_empty())
        .collect::<BTreeSet<_>>();
    let mut seen_strategy_ids = merged
        .iter()
        .filter_map(|asset| first_str(asset, &["strategy_id"]))
        .collect::<BTreeSet<_>>();

    for (index, strategy) in list(asset_strategy_matrix, "assets").iter().enumerate() {
        if !strategy.is_object() {
            continue;
        }
        let role = first_str(strategy, &["asset_role", "strategy_id"]).unwrap_or_default();
        if role.is_empty() {
            continue;
        }
        let strategy_id = first_str(strategy, &["strategy_id"]).unwrap_or_else(|| role.clone());
        let target =
            normalize_unity_path(first_str(strategy, &["unity_target_path"]).unwrap_or_default());
        let asset_id = slug(&role, &format!("strategy_asset_{:03}", index + 1));
        if seen_ids.contains(&asset_id)
            || seen_strategy_ids.contains(&strategy_id)
            || (!target.is_empty() && seen_targets.contains(&target))
        {
            continue;
        }
        let category_id = first_str(strategy, &["category_id"]).unwrap_or_default();
        let asset_type = strategy_category_asset_type(&category_id);
        let source_ref = first_str(strategy, &["source_ref"])
            .unwrap_or_else(|| "stage_04/asset_strategy_matrix.json".to_string());
        let source_refs = unique_strings([
            source_ref.clone(),
            "stage_04/asset_strategy_matrix.json".to_string(),
        ]);
        let final_required = strategy
            .get("final_asset_required")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let consumer_system = first_str(strategy, &["consumer_system"])
            .unwrap_or_else(|| "core_loop_runtime".to_string());
        let mount_point = first_str(strategy, &["mount_point"]).unwrap_or_else(|| {
            target
                .rsplit_once('/')
                .map(|(parent, _)| parent.to_string())
                .filter(|parent| !parent.is_empty())
                .unwrap_or_else(|| "Assets/AutoDesign".to_string())
        });
        let fallback_strategy = if asset_type == "audio_placeholder" {
            "silent marker; replace after audio provider is connected"
        } else {
            "generated production asset until final art pass replaces it"
        };
        let mut item = Map::new();
        item.insert("asset_id".to_string(), json!(asset_id));
        item.insert("name".to_string(), json!(role.replace('_', " ")));
        item.insert("asset_type".to_string(), json!(asset_type));
        item.insert("category_id".to_string(), json!(category_id));
        item.insert("strategy_id".to_string(), json!(strategy_id));
        item.insert("source".to_string(), json!(source_ref));
        item.insert("source_refs".to_string(), json!(source_refs));
        item.insert(
            "purpose".to_string(),
            json!(format!(
                "Consumable {} asset for `{}` used by `{}`.",
                if category_id.is_empty() {
                    asset_type
                } else {
                    &category_id
                },
                role,
                consumer_system
            )),
        );
        item.insert(
            "usage_context".to_string(),
            json!(format!(
                "{role} is mounted for {consumer_system} from the Step04 asset strategy matrix."
            )),
        );
        item.insert("target_path".to_string(), json!(target));
        item.insert("unity_target_path".to_string(), json!(target));
        item.insert("consumer_system".to_string(), json!(consumer_system));
        item.insert("mount_point".to_string(), json!(mount_point));
        item.insert("fallback_strategy".to_string(), json!(fallback_strategy));
        item.insert("fallback_policy".to_string(), json!(fallback_strategy));
        item.insert(
            "acceptance_check".to_string(),
            json!(format!("{asset_id}_available_for_runtime")),
        );
        item.insert(
            "linked_contract_field".to_string(),
            json!(format!("asset_strategy_matrix.assets[{}]", index)),
        );
        item.insert(
            "priority".to_string(),
            json!(if final_required { "P0" } else { "P2" }),
        );
        item.insert(
            "complexity".to_string(),
            json!(
                if matches!(asset_type, "ui" | "effect" | "audio_placeholder") {
                    "s"
                } else {
                    "m"
                }
            ),
        );
        item.insert("required_for_phase".to_string(), json!("core_playable"));
        item.insert("status".to_string(), json!("requirement_defined"));
        item.insert("trace_kind".to_string(), json!("asset_strategy_matrix"));
        item.insert("source_semantics".to_string(), json!([role]));
        item.insert(
            "design_requirement_refs".to_string(),
            item["source_refs"].clone(),
        );
        item.insert(
            "placeholder_allowed".to_string(),
            json!(
                strategy
                    .get("placeholder_allowed")
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
            ),
        );
        item.insert("final_asset_required".to_string(), json!(final_required));
        item.insert(
            "ui_slice_required".to_string(),
            json!(
                strategy
                    .get("ui_slice_required")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            ),
        );
        let value = Value::Object(item);
        if let Some(id) = first_str(&value, &["asset_id"]) {
            seen_ids.insert(id);
        }
        seen_strategy_ids.insert(strategy_id);
        if !target.is_empty() {
            seen_targets.insert(target);
        }
        merged.push(value);
    }
    merged
}

pub fn build_image_consumable_spec(
    assets: &[Value],
    generated_at: &str,
    blockers: Vec<Value>,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "unity_root": UNITY_GENERATED_ROOT,
        "source_refs": ["stage_04/asset_spec_contract.json"],
        "assets": assets.iter().filter(|asset| {
            first_str(asset, &["asset_type"]).unwrap_or_default() != "audio_placeholder"
        }).map(|asset| json!({
            "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
            "asset_type": asset.get("asset_type").cloned().unwrap_or(Value::Null),
            "unity_target_path": asset.get("unity_target_path").cloned().unwrap_or(Value::Null),
            "processed_path": canonical_processed_path(asset),
            "required_format": asset.get("required_format").cloned().unwrap_or(Value::Null),
            "required_size": asset.get("required_size").cloned().unwrap_or(Value::Null),
            "transparency": asset.get("transparency").cloned().unwrap_or(Value::Null),
            "consumer_system": asset.get("consumer_system").cloned().unwrap_or(Value::Null),
            "mount_point": asset.get("mount_point").cloned().unwrap_or(Value::Null),
            "fallback_policy": asset.get("fallback_policy").cloned().unwrap_or(Value::Null),
            "source_refs": asset.get("source_refs").cloned().unwrap_or_else(|| json!([])),
            "consumable_contract_defined": true
        })).collect::<Vec<_>>(),
        "blockers": blockers
    })
}

pub fn build_ui_slice_spec_contract(assets: &[Value], generated_at: &str) -> Value {
    let slice_specs = assets
        .iter()
        .filter(|asset| {
            matches!(
                first_str(asset, &["asset_type"])
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .as_str(),
                "ui" | "icon" | "sprite"
            )
        })
        .map(|asset| {
            let asset_id = first_str(asset, &["asset_id"]).unwrap_or_default();
            json!({
                "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
                "source_image_path": asset.get("unity_target_path").cloned().unwrap_or(Value::Null),
                "processed_image_path": canonical_processed_path(asset),
                "sprite_id": asset_id,
                "rect": {"x": 0, "y": 0, "width": 1, "height": 1},
                "pivot": {"x": 0.5, "y": 0.5},
                "border": {"left": 0, "right": 0, "top": 0, "bottom": 0},
                "target_prefab_path": prefab_path(first_str(asset, &["consumer_system"]).unwrap_or(asset_id)),
                "slice_executor": "unity_texture_importer",
                "python_pixel_operation_allowed": false,
                "source_refs": asset.get("source_refs").cloned().unwrap_or_else(|| json!([]))
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_refs": ["stage_04/asset_spec_contract.json"],
        "slice_specs": slice_specs,
        "policy": "Python emits metadata only; Unity TextureImporter applies slicing during reimport."
    })
}

pub fn build_unity_import_policy(assets: &[Value], generated_at: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "unity_root": UNITY_GENERATED_ROOT,
        "allowed_roots": [
            UNITY_GENERATED_ROOT,
            "Assets/AutoDesign/Art/",
            "Assets/AutoDesign/Prefabs/",
            "Assets/AutoDesign/Runtime/",
            UNITY_AUDIO_PLACEHOLDER_ROOT
        ],
        "atlas_root": UNITY_ART_ATLAS_ROOT,
        "ui_prefab_root": UNITY_UI_PREFAB_ROOT,
        "import_items": assets.iter().map(|asset| json!({
            "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
            "asset_type": asset.get("asset_type").cloned().unwrap_or(Value::Null),
            "unity_target_path": asset.get("unity_target_path").cloned().unwrap_or(Value::Null),
            "import_settings": asset.get("import_settings").cloned().unwrap_or_else(|| json!({})),
            "addressable": false,
            "source_refs": asset.get("source_refs").cloned().unwrap_or_else(|| json!([]))
        })).collect::<Vec<_>>(),
        "addressables_policy": {
            "required": false,
            "blocking_when_package_missing": false,
            "apply_only_when_project_has_addressables": true
        }
    })
}

pub fn build_asset_usage_binding_seed(assets: &[Value], generated_at: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_refs": ["stage_04/asset_spec_contract.json"],
        "bindings": assets.iter().map(|asset| json!({
            "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
            "asset_type": asset.get("asset_type").cloned().unwrap_or(Value::Null),
            "unity_target_path": asset.get("unity_target_path").cloned().unwrap_or(Value::Null),
            "consumer_system": asset.get("consumer_system").cloned().unwrap_or(Value::Null),
            "mount_point": asset.get("mount_point").cloned().unwrap_or(Value::Null),
            "acceptance_check": asset.get("acceptance_check").cloned().unwrap_or(Value::Null),
            "source_refs": asset.get("source_refs").cloned().unwrap_or_else(|| json!([]))
        })).collect::<Vec<_>>()
    })
}

pub fn build_audio_placeholder_requirements(assets: &[Value], generated_at: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_refs": ["stage_04/asset_spec_contract.json"],
        "placeholder_root": UNITY_AUDIO_PLACEHOLDER_ROOT,
        "requirements": assets.iter().filter(|asset| {
            first_str(asset, &["asset_type"]).unwrap_or_default() == "audio_placeholder"
        }).map(|asset| json!({
            "placeholder_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
            "path": asset.get("unity_target_path").cloned().unwrap_or(Value::Null),
            "consumer_system": asset.get("consumer_system").cloned().unwrap_or(Value::Null),
            "mount_point": asset.get("mount_point").cloned().unwrap_or(Value::Null),
            "silent_until_replaced": true,
            "future_audio_requirement": "Replace this marker with generated audio in a later audio provider pass.",
            "source_refs": asset.get("source_refs").cloned().unwrap_or_else(|| json!([]))
        })).collect::<Vec<_>>()
    })
}
