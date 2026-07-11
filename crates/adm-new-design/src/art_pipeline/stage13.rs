use serde_json::{Value, json};

use super::paths::UNITY_EDITOR_ROOT;
use super::{SCHEMA_VERSION, first_str, list};

pub fn build_unity_editor_request(
    generated_at: &str,
    project_path: &str,
    stage_output_path: &str,
    handoff_manifest: &Value,
    scene_path: &str,
) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "project_path": project_path,
        "stage_output_path": stage_output_path,
        "scene_path": scene_path,
        "editor_script_root": UNITY_EDITOR_ROOT,
        "compile_step": {
            "method": "",
            "arguments": ["-batchmode", "-quit", "-projectPath", project_path],
            "purpose": "First Unity launch compiles generated Editor scripts before executeMethod is used."
        },
        "execute_steps": [
            {
                "method": "AutoDesignMaker.Editor.AutoDesignAssetImporter.Run",
                "arguments": ["-artHandoffManifest", "stage_12/art_handoff_manifest.json"]
            },
            {
                "method": "AutoDesignMaker.Editor.AutoDesignPrefabBuilder.Run",
                "arguments": ["-uiPrefabRequest", "stage_12/ui_prefab_generation_request.json"]
            },
            {
                "method": "AutoDesignMaker.Editor.SceneAssemblyBuilder.BuildDemoScene",
                "arguments": ["-sceneAssemblyConfig", "stage_13/scene_assembly_config.json"]
            }
        ],
        "requires_compile_before_execute_method": true,
        "ready_for_execute": handoff_manifest
            .get("ready_for_step13")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    })
}

pub fn build_unity_materialization_reports(
    generated_at: &str,
    handoff_manifest: &Value,
    scene_report: &Value,
) -> serde_json::Map<String, Value> {
    let ready = handoff_manifest
        .get("ready_for_step13")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let scene_status = first_str(scene_report, &["status"]).unwrap_or_default();
    let status = if ready && scene_status == "success" {
        "success"
    } else {
        "blocked"
    };
    let changed_files = list(scene_report, "changed_files");
    let generated_prefabs = changed_files
        .iter()
        .filter_map(Value::as_str)
        .filter(|path| path.ends_with(".prefab"))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let binding_blockers = list(handoff_manifest, "blockers")
        .into_iter()
        .chain(list(scene_report, "blocking_issues"))
        .collect::<Vec<_>>();
    serde_json::Map::from_iter([
        (
            "unity_art_import_report".to_string(),
            json!({
                "schema_version": SCHEMA_VERSION,
                "generated_at": generated_at,
                "status": status,
                "ready_for_step13": ready,
                "imported_assets": handoff_manifest
                    .get("asset_count")
                    .cloned()
                    .unwrap_or_else(|| json!(0)),
                "source_refs": ["stage_12/art_handoff_manifest.json"],
                "blockers": handoff_manifest
                    .get("blockers")
                    .cloned()
                    .unwrap_or_else(|| json!([]))
            }),
        ),
        (
            "unity_prefab_generation_report".to_string(),
            json!({
                "schema_version": SCHEMA_VERSION,
                "generated_at": generated_at,
                "status": status,
                "generated_prefabs": generated_prefabs,
                "source_refs": ["stage_12/ui_prefab_generation_request.json"],
                "blockers": scene_report
                    .get("blocking_issues")
                    .cloned()
                    .unwrap_or_else(|| json!([]))
            }),
        ),
        (
            "program_asset_binding_contract".to_string(),
            json!({
                "schema_version": SCHEMA_VERSION,
                "generated_at": generated_at,
                "status": status,
                "bindings_verified": ready && scene_status == "success",
                "source_refs": [
                    "stage_12/asset_mount_manifest.json",
                    "stage_12/program_asset_binding_preflight.json",
                    "stage_13/scene_assembly_report.json"
                ],
                "blockers": binding_blockers
            }),
        ),
        (
            "unity_scene_mount_report".to_string(),
            json!({
                "schema_version": SCHEMA_VERSION,
                "generated_at": generated_at,
                "status": status,
                "scene_path": scene_report
                    .get("scene_path")
                    .cloned()
                    .unwrap_or_else(|| json!("")),
                "visible_content_verified": scene_report
                    .get("visible_content_verified")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                "mount_item_count": handoff_manifest
                    .get("mount_item_count")
                    .cloned()
                    .unwrap_or_else(|| json!(0)),
                "source_refs": [
                    "stage_12/art_handoff_manifest.json",
                    "stage_13/scene_assembly_report.json"
                ],
                "blockers": scene_report
                    .get("blocking_issues")
                    .cloned()
                    .unwrap_or_else(|| json!([]))
            }),
        ),
    ])
}
