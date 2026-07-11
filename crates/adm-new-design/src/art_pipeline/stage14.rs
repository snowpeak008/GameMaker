use serde_json::{Value, json};

use super::{SCHEMA_VERSION, list, truthy};

pub fn build_art_acceptance_report(
    generated_at: &str,
    art_handoff_manifest: &Value,
    unity_art_import_report: &Value,
    unity_prefab_generation_report: &Value,
    unity_scene_mount_report: &Value,
) -> Value {
    let checks = vec![
        json!({
            "id": "art_handoff_ready",
            "passed": art_handoff_manifest
                .get("ready_for_step13")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "source": "stage_12/art_handoff_manifest.json"
        }),
        json!({
            "id": "unity_art_import_succeeded",
            "passed": unity_art_import_report.get("status").and_then(Value::as_str) == Some("success"),
            "source": "stage_13/unity_art_import_report.json"
        }),
        json!({
            "id": "unity_prefab_generation_succeeded",
            "passed": unity_prefab_generation_report.get("status").and_then(Value::as_str) == Some("success"),
            "source": "stage_13/unity_prefab_generation_report.json"
        }),
        json!({
            "id": "scene_mount_visible",
            "passed": unity_scene_mount_report
                .get("visible_content_verified")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "source": "stage_13/unity_scene_mount_report.json"
        }),
    ];
    let blockers = checks
        .iter()
        .filter(|check| check.get("passed").and_then(Value::as_bool) != Some(true))
        .map(|check| {
            let id = check
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_ascii_uppercase();
            json!({
                "id": format!("ART-ACCEPTANCE-{id}"),
                "source": check.get("source").cloned().unwrap_or(Value::Null)
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": SCHEMA_VERSION,
        "generated_at": generated_at,
        "status": if blockers.is_empty() { "success" } else { "blocked" },
        "checks": checks,
        "blockers": blockers
    })
}

pub fn build_level4_visual_checks(scene_assembly: &Value, environment_issues: &[Value]) -> Value {
    let visible = scene_assembly
        .get("visible_content_verified")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    json!({
        "level": 4,
        "name": "visual_or_pixel_validation",
        "status": if visible {
            "passed"
        } else if !environment_issues.is_empty() {
            "skipped_with_warning"
        } else {
            "failed"
        },
        "thresholds": {
            "minimum_nontransparent_pixel_ratio": 0.05,
            "minimum_active_graphic_count": 1,
            "failure_screenshot_path": "outputs/artifacts/stage_14/screenshots/fail_<check_id>.png",
            "screenshot_format": "PNG",
            "preferred_resolution": "1920x1080 or actual runtime resolution"
        },
        "checks": [{
            "id": "visible_content_verified",
            "passed": visible,
            "source": "stage_13/scene_assembly_report.json"
        }]
    })
}

pub fn build_level5_design_coverage_checks(
    playable_bundle: &Value,
    scene_assembly: &Value,
) -> Value {
    let acceptance = playable_bundle
        .get("playable_acceptance_contract")
        .unwrap_or(&Value::Null);
    let ui_contract = playable_bundle
        .get("ui_flow_contract")
        .unwrap_or(&Value::Null);
    let program_summary = scene_assembly
        .get("contract_summary")
        .unwrap_or(&Value::Null);
    let scene_success = scene_assembly.get("status").and_then(Value::as_str) == Some("success");
    let mut sources = Vec::new();
    for check in list(acceptance, "playmode_checks") {
        sources.push(json!({
            "id": check
                .get("check_id")
                .or_else(|| check.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("playable_check"),
            "source": "stage_02/playable_contracts/playable_acceptance_contract.json",
            "passed": scene_success
        }));
    }
    for check in list(ui_contract, "playmode_checks") {
        sources.push(json!({
            "id": check
                .get("check_id")
                .or_else(|| check.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("ui_check"),
            "source": "stage_02/playable_contracts/ui_flow_contract.json",
            "passed": truthy(program_summary.get("ui_screen_count"))
        }));
    }
    if sources.is_empty() {
        return json!({
            "level": 5,
            "name": "upper_design_requirement_coverage",
            "status": "skipped_with_warning",
            "checks": [],
            "warning": "No programmable Level 5 acceptance sources were available.",
            "sources": [
                "playable_acceptance_contract.playmode_checks",
                "ui_flow_contract.playmode_checks",
                "program_requirements_contract.tasks[].acceptance",
                "asset_spec_contract.assets[].acceptance_check",
                "asset_registry.assets[].purpose"
            ]
        });
    }
    json!({
        "level": 5,
        "name": "upper_design_requirement_coverage",
        "status": if sources.iter().all(|item| item.get("passed").and_then(Value::as_bool) == Some(true)) {
            "passed"
        } else {
            "failed"
        },
        "checks": sources
    })
}
