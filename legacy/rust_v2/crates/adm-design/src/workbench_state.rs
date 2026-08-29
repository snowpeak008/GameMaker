use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkbenchState {
    pub project_name: String,
    pub profile: BTreeMap<String, String>,
    pub nodes: BTreeMap<String, NodeState>,
    pub gameplay_systems: GameplaySystemsState,
    pub ai_interview: AiInterviewState,
    pub version: u64,
    pub dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeState {
    pub decision_state: DecisionState,
    pub design_note: String,
    pub risk_note: String,
    pub not_applicable_reason: String,
    pub design_entities: Vec<Value>,
    pub entity_validation_errors: Vec<EntityValidationError>,
    pub checklist: BTreeMap<String, bool>,
    pub checklist_options: BTreeMap<String, BTreeMap<String, OptionGroupState>>,
    pub option_provenance: BTreeMap<String, BTreeMap<String, BTreeMap<String, OptionProvenance>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionGroupState {
    pub selected: Vec<String>,
    pub primary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionProvenance {
    pub source: String,
    pub confirmed: bool,
    pub actor: String,
    pub ai_inference_id: String,
    pub updated_at_millis: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityValidationError {
    pub severity: String,
    pub node_id: String,
    pub path: String,
    pub message: String,
    pub schema_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameplaySystemsState {
    pub selected: Vec<String>,
    pub custom: Vec<CustomGameplaySystem>,
    pub weights: BTreeMap<String, GameplaySystemWeight>,
    pub core_loops: BTreeMap<String, String>,
    pub interview: GameplayInterviewState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomGameplaySystem {
    pub id: String,
    pub name: String,
    pub category: String,
    pub mapping_desc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameplaySystemWeight {
    pub weight: String,
    pub weight_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GameplayInterviewState {
    pub answers: Vec<String>,
    pub parsed_system_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AiInterviewState {
    pub messages: Vec<AiInterviewMessage>,
    pub candidate_node_ids: Vec<String>,
    pub route_overview: Vec<String>,
    pub prompt_meter: BTreeMap<String, u64>,
    pub replay_records: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiInterviewMessage {
    pub role: String,
    pub content: String,
    pub node_id: String,
    pub created_at_millis: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionState {
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

    pub fn label(&self) -> &'static str {
        match self {
            Self::NotStarted => "未选择",
            Self::Selected => "已确认",
            Self::Completed => "已完成",
            Self::Risk => "有风险",
            Self::NotApplicable => "不适用",
        }
    }

    pub fn from_legacy(value: &str) -> Self {
        match value {
            "selected" => Self::Selected,
            "completed" => Self::Completed,
            "risk" => Self::Risk,
            "not_applicable" => Self::NotApplicable,
            _ => Self::NotStarted,
        }
    }
}

impl Default for DecisionState {
    fn default() -> Self {
        Self::NotStarted
    }
}

impl Default for OptionGroupState {
    fn default() -> Self {
        Self {
            selected: Vec::new(),
            primary: String::new(),
        }
    }
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
        }
    }
}

impl Default for GameplaySystemsState {
    fn default() -> Self {
        Self {
            selected: Vec::new(),
            custom: Vec::new(),
            weights: BTreeMap::new(),
            core_loops: BTreeMap::new(),
            interview: GameplayInterviewState::default(),
        }
    }
}
