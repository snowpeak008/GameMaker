#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai::AiInterviewState;

pub const CONTRACT_FAMILY: &str = "project";
pub const DEFAULT_PROJECT_NAME: &str = "未命名游戏设计项目";

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionState {
    #[default]
    NotStarted,
    Selected,
    Completed,
    Risk,
    NotApplicable,
}

impl DecisionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::Selected => "selected",
            Self::Completed => "completed",
            Self::Risk => "risk",
            Self::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectState {
    #[serde(default = "default_project_name")]
    pub project_name: String,
    #[serde(default)]
    pub profile: BTreeMap<String, Value>,
    #[serde(default)]
    pub nodes: BTreeMap<String, NodeState>,
    #[serde(default)]
    pub gameplay_systems: GameplaySystemsState,
    #[serde(default)]
    pub ai_interview: AiInterviewState,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self::empty()
    }
}

impl ProjectState {
    pub fn empty() -> Self {
        Self {
            project_name: default_project_name(),
            profile: BTreeMap::new(),
            nodes: BTreeMap::new(),
            gameplay_systems: GameplaySystemsState::default(),
            ai_interview: AiInterviewState::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeState {
    #[serde(default)]
    pub decision_state: DecisionState,
    #[serde(default)]
    pub design_note: String,
    #[serde(default)]
    pub risk_note: String,
    #[serde(default)]
    pub not_applicable_reason: String,
    #[serde(default)]
    pub design_entities: Vec<Value>,
    #[serde(default)]
    pub entity_validation_errors: Vec<EntityValidationError>,
    #[serde(default)]
    pub checklist: BTreeMap<String, bool>,
    #[serde(default)]
    pub checklist_options: BTreeMap<String, BTreeMap<String, ChecklistOptionGroupState>>,
    #[serde(default)]
    pub option_provenance: OptionProvenance,
    #[serde(default)]
    pub l4_progress: Option<Value>,
    #[serde(default)]
    pub l5_progress: Option<Value>,
    #[serde(default)]
    pub quality_signals: Option<Value>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            decision_state: DecisionState::NotStarted,
            design_note: String::new(),
            risk_note: String::new(),
            not_applicable_reason: String::new(),
            design_entities: Vec::new(),
            entity_validation_errors: Vec::new(),
            checklist: BTreeMap::new(),
            checklist_options: BTreeMap::new(),
            option_provenance: BTreeMap::new(),
            l4_progress: None,
            l5_progress: None,
            quality_signals: None,
            extra: BTreeMap::new(),
        }
    }
}

pub type OptionProvenance =
    BTreeMap<String, BTreeMap<String, BTreeMap<String, OptionProvenanceEntry>>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistOptionGroupState {
    #[serde(default)]
    pub selected: Vec<String>,
    #[serde(default)]
    pub primary: String,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for ChecklistOptionGroupState {
    fn default() -> Self {
        Self {
            selected: Vec::new(),
            primary: String::new(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionProvenanceEntry {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub confirmed: Option<bool>,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for OptionProvenanceEntry {
    fn default() -> Self {
        Self {
            source: String::new(),
            confidence: None,
            confirmed: None,
            updated_at: String::new(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityValidationError {
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub node_id: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub schema_id: String,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplaySystemsState {
    #[serde(default = "default_gameplay_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub selected: Vec<String>,
    #[serde(default)]
    pub custom: Vec<GameplaySystemOption>,
    #[serde(default)]
    pub weights: BTreeMap<String, GameplaySystemWeight>,
    #[serde(default)]
    pub core_loops: BTreeMap<String, String>,
    #[serde(default)]
    pub interview: GameplaySystemsInterview,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for GameplaySystemsState {
    fn default() -> Self {
        Self {
            schema_version: default_gameplay_schema_version(),
            selected: Vec::new(),
            custom: Vec::new(),
            weights: BTreeMap::new(),
            core_loops: BTreeMap::new(),
            interview: GameplaySystemsInterview::default(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameplaySystemOption {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub category: String,
    #[serde(default, rename = "mapping_desc")]
    pub mapping_desc: String,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameplaySystemWeight {
    #[serde(default)]
    pub weight: Value,
    #[serde(default = "default_percent_weight_type", rename = "weight_type")]
    pub weight_type: String,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for GameplaySystemWeight {
    fn default() -> Self {
        Self {
            weight: Value::String(String::new()),
            weight_type: default_percent_weight_type(),
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplaySystemsInterview {
    #[serde(default)]
    pub questions: Vec<String>,
    #[serde(default)]
    pub answers: Vec<String>,
    #[serde(default)]
    pub parsed_system_ids: Vec<String>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for GameplaySystemsInterview {
    fn default() -> Self {
        Self {
            questions: Vec::new(),
            answers: Vec::new(),
            parsed_system_ids: Vec::new(),
            extra: BTreeMap::new(),
        }
    }
}

fn default_project_name() -> String {
    DEFAULT_PROJECT_NAME.to_string()
}

fn default_gameplay_schema_version() -> String {
    "1.0".to_string()
}

fn default_percent_weight_type() -> String {
    "percent".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::AI_INTERVIEW_SCHEMA_VERSION;

    #[test]
    fn project_state_default_contains_required_roots() {
        let state = ProjectState::empty();
        assert_eq!(state.project_name, DEFAULT_PROJECT_NAME);
        assert_eq!(state.gameplay_systems.schema_version, "1.0");
        assert_eq!(
            state.ai_interview.schema_version,
            AI_INTERVIEW_SCHEMA_VERSION
        );
        assert!(state.nodes.is_empty());
    }

    #[test]
    fn project_state_serde_roundtrip_preserves_option_provenance_and_l4_l5() {
        let mut state = ProjectState::empty();
        let mut node = NodeState {
            decision_state: DecisionState::Selected,
            design_note: "Use readable tactical loops.".to_string(),
            ..NodeState::default()
        };
        node.checklist.insert("core_loop".to_string(), true);
        node.checklist_options.insert(
            "core_loop".to_string(),
            BTreeMap::from([(
                "loop_type".to_string(),
                ChecklistOptionGroupState {
                    selected: vec!["turn_based".to_string()],
                    primary: "turn_based".to_string(),
                    extra: BTreeMap::new(),
                },
            )]),
        );
        node.option_provenance.insert(
            "core_loop".to_string(),
            BTreeMap::from([(
                "loop_type".to_string(),
                BTreeMap::from([(
                    "turn_based".to_string(),
                    OptionProvenanceEntry {
                        source: "user_selected".to_string(),
                        confidence: Some(1.0),
                        confirmed: Some(true),
                        updated_at: "2026-07-08T00:00:00".to_string(),
                        extra: BTreeMap::from([(
                            "turnId".to_string(),
                            Value::String("manual".to_string()),
                        )]),
                    },
                )]),
            )]),
        );
        node.l4_progress = Some(serde_json::json!({"done": 1, "total": 1}));
        node.l5_progress = Some(serde_json::json!({"entities": 1}));
        node.quality_signals = Some(serde_json::json!({"qualityBadge": "L5_partial"}));
        state.nodes.insert("combat_loop".to_string(), node);

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("optionProvenance"));
        assert!(json.contains("l4Progress"));
        assert!(json.contains("l5Progress"));

        let restored: ProjectState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, state);
    }

    #[test]
    fn project_state_roundtrip_preserves_unknown_project_node_and_gameplay_fields() {
        let input = serde_json::json!({
            "projectName": "Future Project",
            "futureRoot": {"revision": 2},
            "nodes": {
                "mechanics": {
                    "decisionState": "selected",
                    "futureNode": {"score": 9},
                    "entityValidationErrors": [{
                        "severity": "warning",
                        "futureDiagnostic": "kept"
                    }],
                    "checklistOptions": {
                        "core_loop": {
                            "loop_type": {
                                "selected": ["turn_based"],
                                "primary": "turn_based",
                                "futureGroup": "kept"
                            }
                        }
                    }
                }
            },
            "gameplaySystems": {
                "schemaVersion": "2.0",
                "selected": ["combat"],
                "custom": [{
                    "id": "combat_mod",
                    "name": "Combat Mod",
                    "category": "custom",
                    "mapping_desc": "future mapping",
                    "futureOption": [1, 2]
                }],
                "weights": {
                    "combat": {
                        "weight": 60,
                        "weight_type": "percent",
                        "futureWeight": {"source": "adaptive"}
                    }
                },
                "coreLoops": {"combat": "fight"},
                "interview": {
                    "questions": ["What matters?"],
                    "answers": ["Combat"],
                    "parsedSystemIds": ["combat"],
                    "futureInterview": true
                },
                "futureGameplay": {"mode": "hybrid"}
            }
        });

        let state: ProjectState = serde_json::from_value(input).unwrap();
        assert_eq!(
            state.extra["futureRoot"],
            serde_json::json!({"revision": 2})
        );
        assert_eq!(
            state.nodes["mechanics"].extra["futureNode"],
            serde_json::json!({"score": 9})
        );
        assert_eq!(
            state.gameplay_systems.extra["futureGameplay"],
            serde_json::json!({"mode": "hybrid"})
        );

        let restored = serde_json::to_value(state).unwrap();
        assert_eq!(
            restored.pointer("/futureRoot/revision"),
            Some(&serde_json::json!(2))
        );
        assert_eq!(
            restored.pointer("/nodes/mechanics/futureNode/score"),
            Some(&serde_json::json!(9))
        );
        assert_eq!(
            restored.pointer("/nodes/mechanics/entityValidationErrors/0/futureDiagnostic"),
            Some(&serde_json::json!("kept"))
        );
        assert_eq!(
            restored.pointer("/nodes/mechanics/checklistOptions/core_loop/loop_type/futureGroup"),
            Some(&serde_json::json!("kept"))
        );
        assert_eq!(
            restored.pointer("/gameplaySystems/futureGameplay/mode"),
            Some(&serde_json::json!("hybrid"))
        );
        assert_eq!(
            restored.pointer("/gameplaySystems/custom/0/futureOption"),
            Some(&serde_json::json!([1, 2]))
        );
        assert_eq!(
            restored.pointer("/gameplaySystems/weights/combat/futureWeight/source"),
            Some(&serde_json::json!("adaptive"))
        );
        assert_eq!(
            restored.pointer("/gameplaySystems/interview/futureInterview"),
            Some(&serde_json::json!(true))
        );
    }

    #[test]
    fn project_state_rejects_invalid_decision_state_enum() {
        let invalid = r#"{"decisionState":"doneish"}"#;
        assert!(serde_json::from_str::<NodeState>(invalid).is_err());
    }
}
