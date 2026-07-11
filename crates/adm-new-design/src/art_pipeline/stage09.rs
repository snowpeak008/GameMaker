use serde_json::{Map, Value, json};

use super::first_str;

pub fn enrich_art_task(task: &Value, selected_style_id: &str) -> Value {
    let mut item = task.as_object().cloned().unwrap_or_else(Map::new);
    let asset_type = first_str(task, &["asset_type"])
        .unwrap_or_else(|| "art_asset".to_string())
        .to_ascii_lowercase();
    let title =
        first_str(task, &["title", "asset_id", "task_id"]).unwrap_or_else(|| "asset".to_string());
    let task_id = first_str(task, &["task_id"]).unwrap_or_default();
    let asset_id = first_str(task, &["asset_id"]).unwrap_or_default();
    item.entry("generation_prompt".to_string()).or_insert_with(|| {
        json!([
            "Create a production-ready Unity game asset.".to_string(),
            format!("Task: {task_id}"),
            format!("Asset: {asset_id}"),
            format!("Title: {title}"),
            format!("Asset type: {asset_type}"),
            format!(
                "Style id: {}",
                if selected_style_id.is_empty() {
                    "confirmed_style"
                } else {
                    selected_style_id
                }
            ),
            "Output must be inspectable, editable, and directly consumable by the Unity import pipeline.".to_string(),
        ].join("\n"))
    });
    item.entry("negative_prompt".to_string()).or_insert_with(|| {
        json!("watermark, embedded UI text, stock-photo collage, complex unusable background, unreadable silhouette, merged layers")
    });
    item.entry("semantic_policy".to_string())
        .or_insert_with(|| {
            json!({
                "non_consumable_markers": ["_concept", "_reference", "_draft"],
                "vision_ai_status": "not_connected",
                "vision_ai_behavior": "mark_needs_human_review_not_auto_block"
            })
        });
    item.entry("slice_spec_ref".to_string()).or_insert_with(|| {
        json!(if matches!(asset_type.as_str(), "ui" | "icon" | "sprite") {
            "stage_04/ui_slice_spec_contract.json"
        } else {
            ""
        })
    });
    item.entry("acceptance_criteria".to_string())
        .or_insert_with(|| {
            json!([
                first_str(task, &["acceptance"])
                    .unwrap_or_else(|| "asset_mounted_and_visible".to_string()),
                "unity_target_path_under_Assets_AutoDesign".to_string(),
                "source_refs_preserved".to_string()
            ])
        });
    item.entry("rework_policy".to_string()).or_insert_with(|| {
        json!({
            "blocking_for_p0": true,
            "queue_artifact": "stage_12/art_rework_queue.json",
            "human_review_state": "needs_human_review"
        })
    });
    Value::Object(item)
}

pub fn enrich_art_tasks(tasks: &[Value], selected_style_id: &str) -> Vec<Value> {
    tasks
        .iter()
        .filter(|task| task.is_object())
        .map(|task| enrich_art_task(task, selected_style_id))
        .collect()
}
