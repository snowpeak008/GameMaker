use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::paths::{
    UNITY_ART_ATLAS_ROOT, UNITY_GENERATED_ROOT, allowed_parent_path, atlas_path,
    canonical_processed_path, is_autodesign_path, normalize_unity_path, prefab_path,
};
use super::{SCHEMA_VERSION, first_str, is_empty_value, list};

pub const NON_CONSUMABLE_MARKERS: &[&str] = &["_concept", "_reference", "_draft"];

pub fn stage12_art_task_preflight(tasks: &[Value]) -> Vec<Value> {
    if tasks.is_empty() {
        return Vec::new();
    }
    let mut blockers = Vec::new();
    for task in tasks {
        let missing = [
            "asset_id",
            "unity_target_path",
            "dimensions",
            "consumer_system",
            "mount_point",
            "acceptance",
        ]
        .iter()
        .filter(|field| is_empty_value(task.get(**field)))
        .map(|field| json!(field))
        .collect::<Vec<_>>();
        if !missing.is_empty() {
            blockers.push(json!({
                "code": "ART_TASK_NOT_CONTRACT_BOUND",
                "task_id": task.get("task_id").cloned().unwrap_or(Value::Null),
                "missing_fields": missing,
                "message": "Art task is not bound to a consumable asset spec."
            }));
        }
        if is_empty_value(task.get("contract_refs")) {
            blockers.push(json!({
                "code": "ART_TASK_CONTRACT_REFS_MISSING",
                "task_id": task.get("task_id").cloned().unwrap_or(Value::Null),
                "message": "Art task must trace to asset_spec_contract before production."
            }));
        }
        let target_path = first_str(task, &["unity_target_path"]).unwrap_or_default();
        if !target_path.is_empty()
            && !normalize_unity_path(&target_path).starts_with(UNITY_GENERATED_ROOT)
        {
            blockers.push(json!({
                "code": "ART_TASK_TARGET_OUTSIDE_AUTODESIGN_ROOT",
                "task_id": task.get("task_id").cloned().unwrap_or(Value::Null),
                "unity_target_path": target_path,
                "message": "Stage12 requires generated art targets under Assets/AutoDesign/."
            }));
        }
    }
    blockers
}

pub fn build_raw_generated_asset_manifest(
    tasks: &[Value],
    produced: &[Value],
    generated_at: &str,
    image_generation_manifest: Option<&Value>,
) -> Value {
    let mut by_task = BTreeMap::new();
    if let Some(manifest) = image_generation_manifest {
        for record in list(manifest, "records") {
            if let Some(task_id) = first_str(&record, &["task_id"]) {
                by_task.insert(task_id, record);
            }
        }
    }
    let records = tasks
        .iter()
        .map(|task| {
            let task_id = first_str(task, &["task_id"]).unwrap_or_default();
            let image_record = by_task.get(&task_id);
            let produced_record = produced.iter().find(|item| {
                first_str(item, &["task_id"]).as_deref() == Some(task_id.as_str())
                    || first_str(item, &["asset_id"]) == first_str(task, &["asset_id"])
            });
            let raw_path = image_record
                .and_then(|record| record.get("result"))
                .and_then(|result| first_str(result, &["output_path", "path"]))
                .unwrap_or_default();
            json!({
                "task_id": task_id,
                "asset_id": task.get("asset_id").cloned().unwrap_or(Value::Null),
                "asset_type": task.get("asset_type").cloned().unwrap_or(Value::Null),
                "raw_source_path": normalize_unity_path(&raw_path),
                "declared_target_path": normalize_unity_path(
                    first_str(task, &["unity_target_path"])
                        .or_else(|| produced_record.and_then(|item| first_str(item, &["target_path"])))
                        .unwrap_or_default()
                ),
                "image_generation_status": image_record
                    .and_then(|record| first_str(record, &["status"]))
                    .unwrap_or_else(|| "not_available".to_string()),
                "available": !raw_path.is_empty(),
                "fallback_policy": first_str(task, &["fallback_policy"])
                    .or_else(|| produced_record.and_then(|item| first_str(item, &["fallback_policy"])))
                    .unwrap_or_else(|| "generated_unity_placeholder_allowed".to_string()),
                "source_refs": task.get("source_refs").cloned().unwrap_or_else(|| json!([]))
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_refs": [
            "stage_09/art_production_task_contract.json",
            "stage_12/generated_images_manifest.json"
        ],
        "raw_assets": records
    })
}

pub fn build_image_quality_report(raw_manifest: &Value, generated_at: &str) -> Value {
    let mut checks = Vec::new();
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    for asset in list(raw_manifest, "raw_assets") {
        let available = asset
            .get("available")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let fallback_policy = first_str(&asset, &["fallback_policy"]).unwrap_or_default();
        let mut status = if available {
            "passed"
        } else {
            "skipped_with_warning"
        };
        let issue = json!({
            "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
            "task_id": asset.get("task_id").cloned().unwrap_or(Value::Null),
            "code": "RAW_IMAGE_NOT_AVAILABLE",
            "message": "No generated bitmap was available; Unity fallback materialization must provide visible content.",
            "fallback_policy": fallback_policy
        });
        if !available && fallback_policy == "block_on_missing" {
            status = "failed";
            blockers.push(issue);
        } else if !available {
            warnings.push(issue);
        }
        checks.push(json!({
            "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
            "task_id": asset.get("task_id").cloned().unwrap_or(Value::Null),
            "status": status,
            "raw_source_path": asset.get("raw_source_path").cloned().unwrap_or_else(|| json!("")),
            "checks": {
                "file_exists": available,
                "format_probe": if available { "passed" } else { "not_run_without_file" },
                "alpha_probe": if available { "passed" } else { "not_run_without_file" }
            }
        }));
    }
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "status": if !blockers.is_empty() {
            "failed"
        } else if !warnings.is_empty() {
            "passed_with_warnings"
        } else {
            "passed"
        },
        "checks": checks,
        "blockers": blockers,
        "warnings": warnings
    })
}

pub fn build_semantic_review_report(raw_manifest: &Value, generated_at: &str) -> Value {
    let mut reviews = Vec::new();
    let mut blockers = Vec::new();
    let mut rework_items = Vec::new();
    for asset in list(raw_manifest, "raw_assets") {
        let path =
            first_str(&asset, &["raw_source_path", "declared_target_path"]).unwrap_or_default();
        let lowered = path.to_ascii_lowercase();
        let markers = NON_CONSUMABLE_MARKERS
            .iter()
            .filter(|marker| lowered.contains(**marker))
            .map(|marker| json!(marker))
            .collect::<Vec<_>>();
        let mut state = "passed";
        if !markers.is_empty() {
            state = "needs_rework";
            let issue = json!({
                "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
                "task_id": asset.get("task_id").cloned().unwrap_or(Value::Null),
                "code": "NON_CONSUMABLE_FILENAME_MARKER",
                "markers": markers,
                "message": "Filename/path marks this asset as concept/reference/draft rather than consumable production art."
            });
            blockers.push(issue.clone());
            rework_items.push(issue);
        } else if asset
            .get("available")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            state = "needs_human_review";
            rework_items.push(json!({
                "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
                "task_id": asset.get("task_id").cloned().unwrap_or(Value::Null),
                "code": "VISION_AI_NOT_CONNECTED",
                "message": "Visual watermark/background/editability checks require a future Vision AI pass.",
                "blocking": false
            }));
        }
        reviews.push(json!({
            "asset_id": asset.get("asset_id").cloned().unwrap_or(Value::Null),
            "task_id": asset.get("task_id").cloned().unwrap_or(Value::Null),
            "status": state,
            "visual_detector": "not_connected",
            "automatic_blocking_checks": ["filename_path_markers"]
        }));
    }
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "status": if !blockers.is_empty() {
            "failed"
        } else if !rework_items.is_empty() {
            "passed_with_review"
        } else {
            "passed"
        },
        "reviews": reviews,
        "blockers": blockers,
        "rework_items": rework_items
    })
}

pub fn build_art_rework_queue(
    quality_report: &Value,
    semantic_report: &Value,
    generated_at: &str,
) -> Value {
    let mut items = Vec::new();
    for (source, report) in [
        ("image_quality_report", quality_report),
        ("art_semantic_review_report", semantic_report),
    ] {
        for issue in list(report, "blockers") {
            let mut item = issue.as_object().cloned().unwrap_or_default();
            item.insert(
                "source".to_string(),
                json!(format!("stage_12/{source}.json")),
            );
            item.insert("blocking".to_string(), json!(true));
            items.push(Value::Object(item));
        }
        for issue in list(report, "warnings") {
            let mut item = issue.as_object().cloned().unwrap_or_default();
            item.insert(
                "source".to_string(),
                json!(format!("stage_12/{source}.json")),
            );
            item.insert("blocking".to_string(), json!(false));
            items.push(Value::Object(item));
        }
        for issue in list(report, "rework_items") {
            let mut item = issue.as_object().cloned().unwrap_or_default();
            item.insert(
                "source".to_string(),
                json!(format!("stage_12/{source}.json")),
            );
            item.entry("blocking".to_string()).or_insert(json!(false));
            items.push(Value::Object(item));
        }
    }
    let blocking_count = items
        .iter()
        .filter(|item| {
            item.get("blocking")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
        .count();
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "items": items,
        "blocking_count": blocking_count,
        "review_count": items.len().saturating_sub(blocking_count)
    })
}

pub fn build_processed_asset_manifest(
    tasks: &[Value],
    raw_manifest: &Value,
    generated_at: &str,
) -> Value {
    let raw_by_asset = list(raw_manifest, "raw_assets")
        .into_iter()
        .filter_map(|item| first_str(&item, &["asset_id"]).map(|id| (id, item)))
        .collect::<BTreeMap<_, _>>();
    let processed = tasks
        .iter()
        .map(|task| {
            let raw = first_str(task, &["asset_id"])
                .and_then(|id| raw_by_asset.get(&id).cloned())
                .unwrap_or(Value::Null);
            let available = raw
                .get("available")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            json!({
                "asset_id": task.get("asset_id").cloned().unwrap_or(Value::Null),
                "task_id": task.get("task_id").cloned().unwrap_or(Value::Null),
                "asset_type": task.get("asset_type").cloned().unwrap_or(Value::Null),
                "raw_source_path": raw.get("raw_source_path").cloned().unwrap_or_else(|| json!("")),
                "processed_path": canonical_processed_path(task),
                "unity_target_path": task.get("unity_target_path").cloned().unwrap_or(Value::Null),
                "status": if available {
                    "processed_metadata_ready"
                } else {
                    "unity_fallback_materialization_ready"
                },
                "python_pixel_operation_performed": false,
                "source_refs": task.get("source_refs").cloned().unwrap_or_else(|| json!([]))
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "processed_assets": processed
    })
}

pub fn build_sprite_slice_result_manifest(tasks: &[Value], generated_at: &str) -> Value {
    let slices = tasks
        .iter()
        .filter(|task| {
            matches!(
                first_str(task, &["asset_type"])
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .as_str(),
                "ui" | "icon" | "sprite"
            )
        })
        .map(|task| {
            let asset_id = task.get("asset_id").cloned().unwrap_or(Value::Null);
            json!({
                "asset_id": asset_id,
                "task_id": task.get("task_id").cloned().unwrap_or(Value::Null),
                "processed_path": canonical_processed_path(task),
                "sprite_id": task.get("asset_id").cloned().unwrap_or(Value::Null),
                "rect": {"x": 0, "y": 0, "width": 1, "height": 1},
                "pivot": {"x": 0.5, "y": 0.5},
                "border": {"left": 0, "right": 0, "top": 0, "bottom": 0},
                "executor": "unity_texture_importer",
                "python_pixel_operation_performed": false
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "slices": slices,
        "policy": "No Python pixel slicing. Unity Editor injects SpriteMetaData into TextureImporter."
    })
}

pub fn build_unity_import_settings_manifest(tasks: &[Value], generated_at: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "unity_root": UNITY_GENERATED_ROOT,
        "import_items": tasks.iter().map(|task| json!({
            "asset_id": task.get("asset_id").cloned().unwrap_or(Value::Null),
            "task_id": task.get("task_id").cloned().unwrap_or(Value::Null),
            "unity_target_path": task.get("unity_target_path").cloned().unwrap_or(Value::Null),
            "processed_path": canonical_processed_path(task),
            "import_settings": task.get("import_settings").cloned().unwrap_or_else(|| json!({
                "texture_type": "Sprite",
                "alpha_is_transparency": true,
                "pixels_per_unit": 100
            })),
            "allowed_write_path": allowed_parent_path(first_str(task, &["unity_target_path"]).unwrap_or_default())
        })).collect::<Vec<_>>()
    })
}

pub fn build_sprite_atlas_plan(tasks: &[Value], generated_at: &str) -> Value {
    let sprite_asset_ids = tasks
        .iter()
        .filter(|task| {
            matches!(
                first_str(task, &["asset_type"])
                    .unwrap_or_default()
                    .to_ascii_lowercase()
                    .as_str(),
                "ui" | "icon" | "sprite"
            )
        })
        .filter_map(|task| task.get("asset_id").cloned())
        .collect::<Vec<_>>();
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "atlas_root": UNITY_ART_ATLAS_ROOT,
        "atlases": if sprite_asset_ids.is_empty() {
            json!([])
        } else {
            json!([{
                "atlas_id": "UIAtlas_Main",
                "path": atlas_path("UIAtlas_Main"),
                "asset_ids": sprite_asset_ids,
                "optional": false
            }])
        }
    })
}

pub fn build_addressable_asset_plan(generated_at: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "status": "not_applicable",
        "required": false,
        "reason": "Addressables are optional and only enabled when the Unity project already includes the package.",
        "groups": []
    })
}

pub fn build_ugui_prefab_contract(tasks: &[Value], generated_at: &str) -> Value {
    let prefabs = tasks
        .iter()
        .filter(|task| {
            let asset_type = first_str(task, &["asset_type"])
                .unwrap_or_default()
                .to_ascii_lowercase();
            let mount_point = first_str(task, &["mount_point"])
                .unwrap_or_default()
                .to_ascii_lowercase();
            let consumer = first_str(task, &["consumer_system"])
                .unwrap_or_default()
                .to_ascii_lowercase();
            matches!(asset_type.as_str(), "ui" | "icon" | "sprite")
                || mount_point.contains("canvas")
                || consumer.contains("hud")
                || consumer.contains("ui")
        })
        .map(|task| {
            let asset_id = first_str(task, &["asset_id"]).unwrap_or_default();
            json!({
                "prefab_id": format!("prefab_{asset_id}"),
                "asset_id": task.get("asset_id").cloned().unwrap_or(Value::Null),
                "task_id": task.get("task_id").cloned().unwrap_or(Value::Null),
                "path": prefab_path(first_str(task, &["consumer_system", "asset_id"]).unwrap_or_default()),
                "mount_point": first_str(task, &["mount_point"]).unwrap_or_else(|| "Canvas/UIRoot".to_string()),
                "required_components": ["CanvasRenderer", "Image"],
                "sprite_asset_id": task.get("asset_id").cloned().unwrap_or(Value::Null),
                "source_refs": task.get("source_refs").cloned().unwrap_or_else(|| json!([]))
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_refs": ["stage_09/art_production_task_contract.json"],
        "prefabs": prefabs
    })
}

pub fn build_ui_prefab_generation_request(ugui_contract: &Value, generated_at: &str) -> Value {
    let requests = list(ugui_contract, "prefabs")
        .iter()
        .enumerate()
        .filter(|(_, prefab)| prefab.is_object())
        .map(|(index, prefab)| {
            json!({
                "request_id": format!("REQ-{:03}", index + 1),
                "prefab_id": prefab.get("prefab_id").cloned().unwrap_or(Value::Null),
                "path": prefab.get("path").cloned().unwrap_or(Value::Null),
                "mount_point": prefab.get("mount_point").cloned().unwrap_or(Value::Null),
                "status": "ready_for_unity_editor"
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "source_refs": ["stage_12/ugui_prefab_contract.json"],
        "requests": requests
    })
}

pub fn build_asset_mount_manifest(tasks: &[Value], generated_at: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "mount_items": tasks.iter().map(|task| json!({
            "asset_id": task.get("asset_id").cloned().unwrap_or(Value::Null),
            "task_id": task.get("task_id").cloned().unwrap_or(Value::Null),
            "asset_type": task.get("asset_type").cloned().unwrap_or(Value::Null),
            "unity_target_path": task.get("unity_target_path").cloned().unwrap_or(Value::Null),
            "processed_path": canonical_processed_path(task),
            "consumer_system": task.get("consumer_system").cloned().unwrap_or(Value::Null),
            "mount_point": task.get("mount_point").cloned().unwrap_or(Value::Null),
            "acceptance": task.get("acceptance").cloned().unwrap_or(Value::Null),
            "source_refs": task.get("source_refs").cloned().unwrap_or_else(|| json!([]))
        })).collect::<Vec<_>>()
    })
}

pub fn build_program_asset_binding_preflight(
    asset_mount_manifest: &Value,
    generated_at: &str,
) -> Value {
    let mut blockers = Vec::new();
    for item in list(asset_mount_manifest, "mount_items") {
        let missing = [
            "asset_id",
            "unity_target_path",
            "consumer_system",
            "mount_point",
        ]
        .iter()
        .filter(|field| is_empty_value(item.get(**field)))
        .map(|field| json!(field))
        .collect::<Vec<_>>();
        if !missing.is_empty() {
            blockers.push(json!({
                "asset_id": item.get("asset_id").cloned().unwrap_or(Value::Null),
                "task_id": item.get("task_id").cloned().unwrap_or(Value::Null),
                "code": "ASSET_BINDING_FIELD_MISSING",
                "missing_fields": missing
            }));
        }
        let target = first_str(&item, &["unity_target_path"]).unwrap_or_default();
        if !target.is_empty() && !is_autodesign_path(&target) {
            blockers.push(json!({
                "asset_id": item.get("asset_id").cloned().unwrap_or(Value::Null),
                "task_id": item.get("task_id").cloned().unwrap_or(Value::Null),
                "code": "ASSET_TARGET_OUTSIDE_AUTODESIGN_ROOT",
                "unity_target_path": normalize_unity_path(target)
            }));
        }
    }
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "ready": blockers.is_empty(),
        "blockers": blockers,
        "checked_mount_items": list(asset_mount_manifest, "mount_items").len()
    })
}

pub fn build_art_handoff_manifest(
    generated_at: &str,
    quality_report: &Value,
    semantic_report: &Value,
    rework_queue: &Value,
    processed_asset_manifest: &Value,
    asset_mount_manifest: &Value,
    binding_preflight: &Value,
) -> Value {
    let mut blockers = Vec::new();
    blockers.extend(list(quality_report, "blockers"));
    blockers.extend(list(semantic_report, "blockers"));
    blockers.extend(list(binding_preflight, "blockers"));
    if rework_queue
        .get("blocking_count")
        .and_then(Value::as_i64)
        .unwrap_or_default()
        > 0
    {
        blockers.push(json!({
            "code": "ART_REWORK_QUEUE_HAS_BLOCKING_ITEMS",
            "artifact": "stage_12/art_rework_queue.json"
        }));
    }
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "ready_for_step13": blockers.is_empty(),
        "blockers": blockers,
        "review_items_count": rework_queue
            .get("review_count")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        "source_refs": [
            "stage_12/image_quality_report.json",
            "stage_12/art_semantic_review_report.json",
            "stage_12/processed_asset_manifest.json",
            "stage_12/asset_mount_manifest.json",
            "stage_12/program_asset_binding_preflight.json"
        ],
        "asset_count": list(processed_asset_manifest, "processed_assets").len(),
        "mount_item_count": list(asset_mount_manifest, "mount_items").len()
    })
}
