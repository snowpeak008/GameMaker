use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{CapabilityProfile, ContentGeneration, SpaceTopology, SpecId, TimeProgression};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameSpec {
    pub identity: SpecIdentity,
    pub intent: ProjectIntent,
    pub capabilities: CapabilityProfile,
    #[serde(default)]
    pub entities: BTreeMap<SpecId, EntitySpec>,
    #[serde(default)]
    pub components: BTreeMap<SpecId, ComponentSpec>,
    #[serde(default)]
    pub relationships: BTreeMap<SpecId, RelationshipSpec>,
    #[serde(default)]
    pub actions: BTreeMap<SpecId, ActionSpec>,
    #[serde(default)]
    pub state_machines: BTreeMap<SpecId, StateMachineSpec>,
    #[serde(default)]
    pub resources: BTreeMap<SpecId, ResourceSpec>,
    #[serde(default)]
    pub spaces: BTreeMap<SpecId, SpaceSpec>,
    pub time: TimeSpec,
    #[serde(default)]
    pub interactions: BTreeMap<SpecId, InteractionSpec>,
    #[serde(default)]
    pub content: BTreeMap<SpecId, ContentSpec>,
    #[serde(default)]
    pub presentation: BTreeMap<SpecId, PresentationSpec>,
    pub technical: TechnicalConstraints,
    #[serde(default)]
    pub acceptance_scenarios: BTreeMap<SpecId, AcceptanceScenario>,
    #[serde(default)]
    pub trace_links: BTreeMap<SpecId, TraceLink>,
    #[serde(default)]
    pub extensions: BTreeMap<SpecId, ExtensionBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecIdentity {
    pub schema_version: String,
    pub project_id: SpecId,
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectIntent {
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub experience_promises: BTreeMap<SpecId, ExperiencePromise>,
    #[serde(default)]
    pub audiences: Vec<String>,
    #[serde(default)]
    pub target_platforms: Vec<String>,
    #[serde(default)]
    pub success_metrics: Vec<String>,
    #[serde(default)]
    pub scope: ScopeEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExperiencePromise {
    pub statement: String,
    pub priority: PromisePriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromisePriority {
    Primary,
    Supporting,
    Optional,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScopeEnvelope {
    #[serde(default)]
    pub must_have: Vec<String>,
    #[serde(default)]
    pub wont_have: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum_session_minutes: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EntitySpec {
    pub summary: String,
    #[serde(default)]
    pub components: Vec<SpecId>,
    #[serde(default)]
    pub tags: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComponentSpec {
    pub summary: String,
    #[serde(default)]
    pub properties: BTreeMap<SpecId, PropertySpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PropertySpec {
    pub value_kind: ValueKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    #[serde(default)]
    pub constraints: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Boolean,
    Integer,
    Number,
    Text,
    Identifier,
    Vector2,
    Vector3,
    Enumeration,
    Object,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationshipSpec {
    pub summary: String,
    pub source: SpecRef,
    pub target: SpecRef,
    pub relation: RelationshipKind,
    #[serde(default)]
    pub cardinality: RelationshipCardinality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipKind {
    Contains,
    Owns,
    Targets,
    Opposes,
    Supports,
    Produces,
    Consumes,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipCardinality {
    OneToOne,
    #[default]
    OneToMany,
    ManyToOne,
    ManyToMany,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionSpec {
    pub summary: String,
    #[serde(default)]
    pub actors: Vec<SpecRef>,
    #[serde(default)]
    pub targets: Vec<SpecRef>,
    #[serde(default)]
    pub inputs: Vec<InputSpec>,
    #[serde(default)]
    pub preconditions: Vec<ConditionSpec>,
    #[serde(default)]
    pub effects: Vec<EffectSpec>,
    #[serde(default)]
    pub feedback: Vec<FeedbackSpec>,
    #[serde(default)]
    pub timing: ActionTiming,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputSpec {
    pub channel: String,
    pub command: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionTiming {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FeedbackSpec {
    pub channel: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConditionSpec {
    pub description: String,
    #[serde(default)]
    pub reads: Vec<SpecRef>,
    pub expression: ConditionExpr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ConditionExpr {
    Always,
    All {
        items: Vec<ConditionExpr>,
    },
    Any {
        items: Vec<ConditionExpr>,
    },
    Not {
        item: Box<ConditionExpr>,
    },
    Equals {
        source: SpecRef,
        value: Value,
    },
    Compare {
        source: SpecRef,
        operator: CompareOperator,
        value: Value,
    },
    HasTag {
        entity: SpecId,
        tag: String,
    },
    Extension {
        namespace: SpecId,
        payload: Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareOperator {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum EffectSpec {
    SetValue {
        target: SpecRef,
        value: Value,
    },
    ChangeResource {
        resource: SpecId,
        amount: i64,
    },
    TransitionState {
        state_machine: SpecId,
        target_state: SpecId,
    },
    CreateEntity {
        entity: SpecId,
        quantity: u32,
    },
    RemoveEntity {
        entity: SpecId,
        quantity: u32,
    },
    EmitEvent {
        event: SpecId,
    },
    Extension {
        namespace: SpecId,
        payload: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecRef {
    pub kind: SpecKind,
    pub id: SpecId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecKind {
    Intent,
    Capability,
    Entity,
    Component,
    Relationship,
    Action,
    StateMachine,
    State,
    Resource,
    Space,
    Time,
    Interaction,
    Content,
    Presentation,
    TechnicalConstraint,
    Scenario,
    Extension,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateMachineSpec {
    pub summary: String,
    pub initial_state: SpecId,
    pub states: BTreeMap<SpecId, StateSpec>,
    #[serde(default)]
    pub transitions: Vec<TransitionSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StateSpec {
    pub summary: String,
    #[serde(default)]
    pub terminal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransitionSpec {
    pub transition_id: SpecId,
    pub from: SpecId,
    pub to: SpecId,
    pub trigger: TriggerSpec,
    #[serde(default)]
    pub guards: Vec<ConditionSpec>,
    #[serde(default)]
    pub effects: Vec<EffectSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TriggerSpec {
    pub source: TriggerSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<SpecRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSource {
    Action,
    Input,
    Timer,
    System,
    Extension,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceSpec {
    pub summary: String,
    pub unit: String,
    pub initial: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<i64>,
    #[serde(default)]
    pub source_actions: Vec<SpecId>,
    #[serde(default)]
    pub sink_actions: Vec<SpecId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpaceSpec {
    pub summary: String,
    pub topology: SpaceTopology,
    #[serde(default)]
    pub regions: BTreeMap<SpecId, RegionSpec>,
    #[serde(default)]
    pub connections: Vec<RegionConnection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegionSpec {
    pub summary: String,
    #[serde(default)]
    pub tags: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegionConnection {
    pub from: SpecId,
    pub to: SpecId,
    #[serde(default)]
    pub bidirectional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimeSpec {
    pub progression: TimeProgression,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_step_hz: Option<u32>,
    #[serde(default)]
    pub pausable: bool,
    #[serde(default)]
    pub phases: BTreeMap<SpecId, TimePhaseSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimePhaseSpec {
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InteractionSpec {
    pub summary: String,
    pub direction: InteractionDirection,
    pub modality: String,
    #[serde(default)]
    pub source_actions: Vec<SpecId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionDirection {
    Input,
    Output,
    Bidirectional,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentSpec {
    pub summary: String,
    pub generation: ContentGeneration,
    pub item_kind: String,
    #[serde(default)]
    pub source_refs: Vec<SpecRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationSpec {
    pub summary: String,
    pub medium: String,
    #[serde(default)]
    pub constraints: BTreeMap<String, Value>,
    #[serde(default)]
    pub source_refs: Vec<SpecRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TechnicalConstraints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_engine: Option<String>,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub performance_budgets: BTreeMap<String, u64>,
    #[serde(default)]
    pub save_requirements: Vec<String>,
    #[serde(default)]
    pub accessibility_requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcceptanceScenario {
    pub summary: String,
    #[serde(default)]
    pub given: Vec<ConditionSpec>,
    #[serde(default)]
    pub when: Vec<ActionInvocation>,
    #[serde(default)]
    pub then: Vec<ConditionSpec>,
    #[serde(default)]
    pub failure_case: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActionInvocation {
    pub action: SpecId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<SpecRef>,
    #[serde(default)]
    pub targets: Vec<SpecRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TraceLink {
    pub source: SpecRef,
    pub target: SpecRef,
    pub relation: TraceRelation,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceRelation {
    Refines,
    Requires,
    Verifies,
    Implements,
    Presents,
    Mitigates,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionBlock {
    pub namespace: SpecId,
    pub version: String,
    pub payload: Value,
}
