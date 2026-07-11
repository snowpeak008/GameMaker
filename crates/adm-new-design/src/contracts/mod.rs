pub mod common;
pub mod identity;
pub mod open_questions;
pub mod playable;
pub mod project_dna;

pub use identity::{
    build_customization_score_report, build_customization_score_report_with_locale,
    build_project_identity, build_project_identity_with_locale,
};
pub use open_questions::{
    build_open_questions_contract, build_open_questions_contract_with_locale,
    unresolved_blocking_questions,
};
pub use playable::{
    AUDIO_PLACEHOLDER_PATH, DEFAULT_START_SCENE, PLAYABLE_CONTRACT_DIR, PLAYABLE_CONTRACT_VERSION,
    build_playable_contract_bundle, build_playable_contract_bundle_from_decisions,
    build_playable_contract_bundle_from_decisions_with_locale,
    build_playable_contract_bundle_with_locale, build_playable_development_tasks,
    validate_playable_contract_bundle,
};
pub use project_dna::{
    build_playable_scenario_contract, build_playable_scenario_contract_with_locale,
    build_project_dna_seed, build_project_dna_seed_with_locale, freeze_project_dna,
    freeze_project_dna_with_locale, validate_project_dna_contract,
};

#[cfg(test)]
mod tests {
    use std::path::Path;

    use adm_new_contracts::schema::{load_structured_file, validate_contract};
    use serde_json::json;

    use super::*;

    fn assert_schema_valid(contract: &serde_json::Value, schema_path: &str) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let schema = load_structured_file(&root.join(schema_path)).unwrap();
        let errors = validate_contract(contract, &schema);
        assert!(errors.is_empty(), "{schema_path}: {errors:?}");
    }

    #[test]
    fn project_identity_contract_is_stable_and_reports_customization_score() {
        let parsed = json!({
            "source": "source_artifacts/concept.md",
            "source_package": "source_artifacts/game",
            "source_sha256": "abc",
            "selections": [
                {"id": "SEL-001", "item_type": "core_loop", "label": "Dash combat", "purpose": "fast action", "source_ref": "concept.md#loop"}
            ]
        });
        let profile = json!({
            "project_id": "Hades Like",
            "project_name": "Underworld Dash",
            "genre": "roguelike_action",
            "referenceGame": "Hades"
        });
        let identity = build_project_identity(
            &parsed,
            Path::new("drafts/session_a/outputs/artifacts/stage_00"),
            Some(&profile),
            Some("save_001"),
        );
        assert_eq!(identity["draft_session_id"], json!("session_a"));
        assert_eq!(identity["project_id"], json!("Hades_Like"));
        assert_eq!(identity["project_name"], json!("Underworld Dash"));
        assert_eq!(identity["linked_save_id"], json!("save_001"));
        assert!(identity["project_signature"].as_str().unwrap().len() >= 32);

        let report =
            build_customization_score_report("00", Some(&identity), "passed", &[], &[], None);
        assert_eq!(report["status"], json!("passed"));
        assert_eq!(report["project_specificity_score"], json!(1.0));
    }

    #[test]
    fn open_questions_find_unresolved_blockers() {
        let identity = json!({"project_signature": "sig", "draft_session_id": "d1"});
        let archetype = json!({
            "detected_archetype": "action",
            "open_questions": [
                {"id": "oq1", "prompt": "Need target platform?", "blocking": true}
            ]
        });
        let contract = build_open_questions_contract(
            Some(&identity),
            Some(&archetype),
            &[json!("Optional note")],
            "01",
        );
        assert_eq!(contract["blocking_count"], json!(1));
        let blockers = unresolved_blocking_questions(Some(&contract));
        assert_eq!(blockers[0]["code"], json!("BLOCKING_OQ_UNRESOLVED"));
    }

    #[test]
    fn project_dna_freeze_blocks_without_demo_flow_and_passes_with_bundle() {
        let identity = json!({
            "draft_session_id": "d1",
            "project_signature": "sig",
            "project_id": "p1",
            "project_name": "Project",
            "source_refs": ["concept.md"]
        });
        let parsed = json!({
            "source": "concept.md",
            "selections": [{"id": "SEL-001", "label": "Core loop", "purpose": "Loop"}]
        });
        let archetype = json!({
            "detected_archetype": "action",
            "required_systems": [{"system_id": "combat_loop"}],
            "required_entities": [{"entity_id": "player_actor"}],
            "required_player_actions": [{"action_id": "attack"}],
            "required_objectives": [{"objective_id": "win"}],
            "required_assets": [{"asset_role": "hero"}],
            "acceptance_scenarios": [{"scenario_id": "first_flow"}]
        });
        let seed = build_project_dna_seed(&identity, &json!({}), &parsed, &archetype, None);
        let (_blocked, blockers) = freeze_project_dna(&seed, &archetype, &json!({}), &[]);
        assert!(
            blockers
                .iter()
                .any(|item| item["code"] == "PLAYABLE_SCENARIO_MISSING")
        );

        let bundle = build_playable_contract_bundle(&parsed);
        let (frozen, blockers) = freeze_project_dna(&seed, &archetype, &bundle, &[]);
        assert!(blockers.is_empty(), "{blockers:?}");
        assert_eq!(frozen["status"], json!("frozen"));
        assert_eq!(
            frozen["playable_contract_refs"]["demo_flow_contract"],
            json!("stage_02/playable_contracts/demo_flow_contract.json")
        );
        let scenario = build_playable_scenario_contract(&frozen, &bundle);
        assert_eq!(scenario["project_signature"], json!("sig"));
    }

    #[test]
    fn playable_bundle_validator_and_tasks_cover_runtime_surface() {
        let parsed = json!({
            "source": "concept.md",
            "selections": [
                {"id": "SEL-001", "label": "Move and attack", "purpose": "Player action", "source_ref": "concept.md#action"},
                {"id": "SEL-002", "item_type": "system", "label": "Core Loop", "purpose": "Runtime system"}
            ]
        });
        let bundle = build_playable_contract_bundle(&parsed);
        assert_eq!(
            bundle["design_completeness_report"]["status"],
            json!("passed")
        );
        assert_eq!(
            bundle["core_playable_contract"]["action_verbs"][0]["action_id"],
            json!("action_01")
        );
        assert_eq!(
            bundle["demo_flow_contract"]["steps"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        let tasks = build_playable_development_tasks(&bundle, 10);
        assert_eq!(tasks.len(), 5);
        assert_eq!(tasks[0]["task_id"], json!("PLAY-010"));
        for (contract_id, schema_path) in [
            (
                "runtime_data_contract",
                "knowledge/schemas/playable_contracts/runtime_data_contract.schema.json",
            ),
            (
                "ui_flow_contract",
                "knowledge/schemas/playable_contracts/ui_flow_contract.schema.json",
            ),
            (
                "scene_bootstrap_contract",
                "knowledge/schemas/playable_contracts/scene_bootstrap_contract.schema.json",
            ),
        ] {
            assert_schema_valid(&bundle[contract_id], schema_path);
        }

        let mut broken = bundle.clone();
        broken["demo_flow_contract"]["steps"] = json!([]);
        let report = validate_playable_contract_bundle(&broken);
        assert_eq!(report["status"], json!("blocked"));
        assert!(
            report["playable_completeness"]["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == "DEMO_FLOW_TOO_SHORT")
        );
    }

    #[test]
    fn playable_bundle_from_empty_structured_decisions_is_blocked() {
        let bundle = build_playable_contract_bundle_from_decisions(
            &json!({"source": "decisions.json", "decisions": []}),
            &json!({"project_id": "p1"}),
            &json!({}),
        );
        assert_eq!(
            bundle["design_completeness_report"]["status"],
            json!("blocked")
        );
        assert!(
            bundle["design_completeness_report"]["playable_completeness"]["blocking_issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["code"] == "STRUCTURED_DECISIONS_EMPTY")
        );
    }

    #[test]
    fn structured_decisions_do_not_keep_abstract_inference_review_item() {
        let bundle = build_playable_contract_bundle_from_decisions(
            &json!({
                "source": "decisions.json",
                "decisions": [{
                    "node_id": "core_loop_decision",
                    "source_refs": ["core_loop_decision"],
                    "selected_options": [{
                        "label": "移动并攻击",
                        "description": "玩家执行主要动作并得到可见反馈。",
                        "source_refs": ["core_loop_decision/action_loop"]
                    }]
                }]
            }),
            &json!({"project_id": "p1"}),
            &json!({}),
        );

        assert_eq!(
            bundle["core_playable_contract"]["generation_mode"],
            json!("structured_decisions")
        );
        assert!(
            bundle["design_completeness_report"]["playable_completeness"]["review_items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["code"] != "CONTRACT_INFERRED_FROM_ABSTRACT_DESIGN")
        );
    }
}
