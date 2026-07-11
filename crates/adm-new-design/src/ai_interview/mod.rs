use adm_new_contracts::project::ProjectState;
use serde::{Deserialize, Serialize};

pub mod backend;
pub mod mapping_agent;
pub mod prompt_packer;
pub mod route_planner;
pub mod state;
pub mod summary_agent;
pub mod ucos_bridge;

pub use backend::*;
pub use mapping_agent::*;
pub use prompt_packer::*;
pub use route_planner::*;
pub use state::*;
pub use summary_agent::*;
pub use ucos_bridge::*;

pub const HIGH_CONFIDENCE_THRESHOLD: f64 = 0.75;
pub const CLARIFICATION_CONFIDENCE_THRESHOLD: f64 = 0.45;
pub const QUESTION_GROUP_CHECK_INTERVAL: u32 = 10;
pub const MAX_QUESTION_GROUP_SIZE: usize = 4;
pub const RECENT_MESSAGE_LIMIT_TURN: usize = 6;
pub const RECENT_MESSAGE_LIMIT_FULL: usize = 12;
pub const CANDIDATE_NODE_LIMIT: usize = 5;
pub const CANDIDATE_NODE_MIN_LIMIT: usize = 3;
pub const PROMPT_CHAR_BUDGET_TURN: usize = 16_000;
pub const OUTPUT_PARTITION_PROMPT_BUDGET: usize = 130_000;
pub const OUTPUT_PARTITION_CANDIDATE_COUNTS: &[usize] = &[4, 8, 16];

pub(crate) const CONCRETE_ROLE_CLASSES: &[&str] = &["system_concrete", "content_concrete"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterviewSchemaMode {
    Turn,
    Readiness,
    FullOutput,
    PartialOutput,
    Mapping,
    Summary,
}

impl InterviewSchemaMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Turn => "turn",
            Self::Readiness => "readiness",
            Self::FullOutput => "full_output",
            Self::PartialOutput => "partial_output",
            Self::Mapping => "mapping",
            Self::Summary => "summary",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterviewOutputMode {
    InterviewTurn,
    FullProjectOutput,
    PartialProjectOutput,
    Mapping,
    SummaryCorrection,
}

impl InterviewOutputMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InterviewTurn => "interview_turn",
            Self::FullProjectOutput => "full_project_output",
            Self::PartialProjectOutput => "partial_project_output",
            Self::Mapping => "mapping",
            Self::SummaryCorrection => "summary_correction",
        }
    }
}

pub(crate) fn project_state_ai_mut(
    state: &mut ProjectState,
) -> &mut adm_new_contracts::ai::AiInterviewState {
    &mut state.ai_interview
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DesignChecklistItemSpec, DesignEngineService, DesignNodeSpec, DesignOptionGroupSpec,
    };
    use adm_new_contracts::ai::{AiResponseMode, AiResponsePayload};
    use serde_json::{Value, json};
    use std::collections::BTreeMap;

    fn sample_service() -> DesignEngineService {
        DesignEngineService::new(vec![
            DesignNodeSpec {
                node_id: "combat_loop".to_string(),
                domain_id: "mechanics".to_string(),
                name: "Combat Loop".to_string(),
                description: "Define combat.".to_string(),
                role_class: "system_concrete".to_string(),
                checklist: vec![DesignChecklistItemSpec {
                    item_id: "core_loop".to_string(),
                    label: "Core Loop".to_string(),
                    option_groups: vec![DesignOptionGroupSpec {
                        group_id: "loop_type".to_string(),
                        selection_mode: "single".to_string(),
                        allow_primary: true,
                        options: vec!["turn_based".to_string(), "real_time".to_string()],
                    }],
                }],
            },
            DesignNodeSpec {
                node_id: "economy".to_string(),
                domain_id: "systems".to_string(),
                name: "Economy".to_string(),
                description: "Define economy.".to_string(),
                role_class: "system_abstract".to_string(),
                checklist: vec![DesignChecklistItemSpec {
                    item_id: "sink_source".to_string(),
                    label: "Sink Source".to_string(),
                    option_groups: vec![DesignOptionGroupSpec {
                        group_id: "economy_shape".to_string(),
                        selection_mode: "multi".to_string(),
                        allow_primary: false,
                        options: vec!["soft_currency".to_string(), "crafting".to_string()],
                    }],
                }],
            },
        ])
    }

    #[test]
    fn state_machine_records_user_backend_and_question_payload() {
        let mut project = sample_service().empty_state();
        normalize_interview_state(&mut project.ai_interview);
        let turn = start_user_turn(&mut project, "我要回合制战斗", "turn_001", false);
        assert_eq!(turn.turn_id, "turn_001");
        assert_eq!(project.ai_interview.status, "running");
        assert_eq!(project.ai_interview.messages[0]["role"], "user");

        mark_backend_started(&mut project.ai_interview, "codex_cli");
        let payload = AiResponsePayload {
            schema_version: "1.0".to_string(),
            mode: AiResponseMode::QuestionGroup,
            assistant_message: "请确认战斗节奏。".to_string(),
            question_group: Some(json!({
                "purpose": "澄清战斗",
                "questions": [{"text": "偏回合还是即时？", "targetNodeIds": ["combat_loop"]}]
            })),
            inferences: vec![json!({
                "nodeId": "combat_loop",
                "itemId": "core_loop",
                "groupId": "loop_type",
                "optionIds": ["turn_based"],
                "confidence": 0.88,
                "reason": "用户明确提到回合制"
            })],
            ..minimal_payload(AiResponseMode::QuestionGroup)
        };
        let report = apply_payload_to_interview_state(&mut project, &payload, Some("turn_001"));
        assert_eq!(report.mode, AiResponseMode::QuestionGroup);
        assert!(project.ai_interview.awaiting_user_answer);
        assert_eq!(project.ai_interview.question_group_count, 1);
        assert_eq!(project.ai_interview.summary.v1.confirmed_intent.len(), 1);
        assert!(
            project
                .ai_interview
                .applicability_scores
                .contains_key("combat_loop")
        );
    }

    #[test]
    fn prompt_packer_builds_budgeted_turn_prompt_and_replay_fields() {
        let service = sample_service();
        let mut project = service.empty_state();
        project.project_name = "Project Orion".to_string();
        let result = build_interview_prompt(
            &service,
            &mut project,
            "我想要回合制战斗，也要经济系统",
            PromptBuildOptions {
                turn_id: "turn_prompt".to_string(),
                framework_version: "pf-1".to_string(),
                manifest_hash: "hash".to_string(),
                memory_signals: vec![json!({"summary":"old combat preference"})],
                ..PromptBuildOptions::default()
            },
        );
        assert!(
            result
                .prompt_text
                .contains("commercial_game_design_ai_interview")
        );
        assert!(result.prompt_text.contains("Combat Loop"));
        assert_eq!(result.schema_mode, InterviewSchemaMode::Turn);
        assert_eq!(result.meter.turn_id, "turn_prompt");
        assert_eq!(
            result.replay["packedPromptChars"],
            json!(result.prompt_text.chars().count())
        );
    }

    #[test]
    fn route_planner_prefers_matching_and_unfinished_nodes() {
        let service = sample_service();
        let mut project = service.empty_state();
        let candidates = candidate_node_ids(
            &service,
            &project,
            "我想做 turn_based 战斗",
            CANDIDATE_NODE_LIMIT,
        );
        assert_eq!(candidates.first().map(String::as_str), Some("combat_loop"));
        let overview = update_route_overview(&service, &mut project);
        assert_eq!(overview.current_mda_stage, "体验目标");
        assert!(
            overview
                .expected_domains
                .iter()
                .any(|item| item == "Mechanics")
        );
    }

    #[test]
    fn mapping_validator_rejects_unknown_options_and_accepts_valid_payload() {
        let service = sample_service();
        let valid = json!({
            "mode": "mapping",
            "inferences": [{
                "nodeId": "combat_loop",
                "itemId": "core_loop",
                "groupId": "loop_type",
                "optionIds": ["turn_based"],
                "confidence": 0.83
            }]
        });
        assert!(validate_mapping_payload(&service, &valid).is_empty());

        let invalid = json!({
            "mode": "mapping",
            "inferences": [{
                "nodeId": "combat_loop",
                "itemId": "core_loop",
                "groupId": "loop_type",
                "optionIds": ["unknown"],
                "confidence": 1.2
            }]
        });
        let errors = validate_mapping_payload(&service, &invalid);
        assert!(errors.iter().any(|item| item.contains("unknown option")));
        assert!(errors.iter().any(|item| item.contains("confidence")));
    }

    #[test]
    fn summary_validator_and_prompt_keep_summary_only_contract() {
        let service = sample_service();
        let mut project = service.empty_state();
        add_message(
            &mut project.ai_interview,
            "user",
            "修正：不是硬核 Roguelike",
            None,
        );
        let prompt = build_summary_correction_prompt(
            &service,
            &mut project,
            SummaryPromptOptions {
                turn_id: "summary_1".to_string(),
                framework_version: "pf".to_string(),
                manifest_hash: "mh".to_string(),
            },
        );
        assert!(prompt.prompt_text.contains("summaryRequirements"));
        assert!(
            validate_summary_payload(&json!({
                "mode": "summary_correction",
                "summary": {
                    "confirmedIntent": [],
                    "openQuestions": [],
                    "rejectedAssumptions": [],
                    "lastUserCorrections": [],
                    "nodeNotes": {},
                    "mdaProgress": {}
                }
            }))
            .is_empty()
        );
    }

    #[test]
    fn ucos_bridge_emits_turn_router_semantic_and_generation_events() {
        let payload = json!({
            "mode": "full_project_output",
            "assistantMessage": "完成",
            "routerDecision": {
                "candidateNodes": [{"id": "combat_loop", "name": "Combat Loop"}]
            },
            "inferences": [{
                "nodeId": "combat_loop",
                "confidence": 0.91,
                "reason": "明确确认"
            }],
            "fullProjectOutput": {
                "projectStateJson": "{\"projectName\":\"Project Orion\"}"
            }
        });
        let events = record_interview_turn_events(
            "turn_abc",
            "生成完整方案",
            &payload,
            "project_mem",
            "batch_1",
        );
        assert_eq!(events.len(), 4);
        assert!(events.iter().any(|event| {
            event
                .relative_path
                .contains("episodic/turns/project_mem/batch_1/turn_abc.json")
        }));
        assert!(
            events
                .iter()
                .any(|event| event.relative_path.contains("semantic/staging"))
        );
        assert!(
            events
                .iter()
                .any(|event| event.payload["title"] == json!("设计内容生成：Project Orion"))
        );
    }

    #[test]
    fn backend_specs_parse_events_json_and_resume_args() {
        let args = build_codex_interview_args(&CodexInterviewCommandSpec {
            prompt_path: "prompt.txt".to_string(),
            output_path: "last_message.json".to_string(),
            schema_path: "schema.json".to_string(),
            workdir: "work".to_string(),
            session_id: "session-1".to_string(),
            config_args: vec!["--profile".to_string(), "dev".to_string()],
            use_schema: true,
        });
        assert_eq!(args[0], "exec");
        assert!(args.iter().any(|item| item == "resume"));
        let events =
            parse_json_lines("{\"session_id\":\"550e8400-e29b-41d4-a716-446655440000\"}\nnoise");
        assert_eq!(
            extract_session_id(&events).as_deref(),
            Some("550e8400-e29b-41d4-a716-446655440000")
        );
        let object = extract_json_object("```json\n{\"mode\":\"mapping\"}\n```").unwrap();
        assert_eq!(object["mode"], json!("mapping"));
    }

    #[test]
    fn output_partition_prompt_contains_only_requested_domains() {
        let service = sample_service();
        let mut project = service.empty_state();
        let result = build_output_partition_prompt(
            &service,
            &mut project,
            "生成",
            &["mechanics".to_string()],
            1,
            2,
            PromptBuildOptions {
                turn_id: "part_1".to_string(),
                framework_version: "pf".to_string(),
                manifest_hash: "mh".to_string(),
                ..PromptBuildOptions::default()
            },
        );
        assert_eq!(result.schema_mode, InterviewSchemaMode::PartialOutput);
        assert!(result.prompt_text.contains("\"domainIds\":[\"mechanics\"]"));
        assert!(!result.prompt_text.contains("\"systems\""));
    }

    fn minimal_payload(mode: AiResponseMode) -> AiResponsePayload {
        AiResponsePayload {
            schema_version: "1.0".to_string(),
            mode,
            assistant_message: String::new(),
            route_overview: None,
            question_group: None,
            readiness_check: None,
            full_project_output: None,
            partial_project_output: None,
            inferences: Vec::new(),
            option_differences: Vec::new(),
            summary: None,
            errors: Vec::new(),
            extra: BTreeMap::<String, Value>::new(),
        }
    }
}
