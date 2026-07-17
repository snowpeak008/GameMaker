use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use adm_new_contracts::project::{DecisionState, ProjectState};
use adm_new_foundation::{AdmResult, io, sha256_hex};
use adm_new_game_spec::{
    AcceptanceScenario, ActionInvocation, ActionSpec, CapabilityProfile, ComponentSpec,
    ConditionExpr, ConditionSpec, ConnectivityCapability, ConnectivityModel, ContentGeneration,
    ContentMutability, ContentSpec, ControlCapability, ControlCardinality, ControlDirectness,
    Dimensionality, EffectSpec, EntitySpec, ExperiencePromise, FeedbackSpec, GameSpec,
    InformationCapability, InformationVisibility, InputSpec, InteractionDirection, InteractionSpec,
    ParticipantAsymmetry, ParticipantCapability, ParticipantMode, PresentationSpec,
    ProductEnvelope, ProductionScale, ProgressionCapability, ProgressionPersistence,
    ProgressionStructure, ProjectIntent, PropertySpec, RelationshipKind, RelationshipSpec,
    ScopeEnvelope, SimulationAuthority, Simultaneity, SpaceCapability, SpaceSpec, SpaceTopology,
    SpecId, SpecIdentity, SpecKind, SpecRef, StateMachineSpec, StateSpec, TechnicalConstraints,
    TimePhaseSpec, TimeProgression, TimeSpec, TraceLink, TraceRelation, TriggerSource, TriggerSpec,
    ValueKind, canonicalize_game_spec, validate_game_spec,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::DesignEngineService;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameSpecV2Switch {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_freeze_policy")]
    pub d4_freeze_policy: String,
}

impl Default for GameSpecV2Switch {
    fn default() -> Self {
        Self {
            enabled: false,
            d4_freeze_policy: default_freeze_policy(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GameSpecProjectionReport {
    pub schema_version: String,
    pub enabled: bool,
    pub semantic_hash: String,
    pub game_spec_hash: String,
    pub validation_error_count: usize,
    pub validation_warning_count: usize,
    pub diff: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameSpecProjection {
    pub spec: GameSpec,
    pub report: GameSpecProjectionReport,
}

pub fn project_state_to_game_spec(
    engine: &DesignEngineService,
    project_state: &ProjectState,
) -> AdmResult<GameSpecProjection> {
    let normalized = engine.normalize_state(project_state.clone());
    let spec = build_game_spec(engine, &normalized);
    let validation = validate_game_spec(&spec);
    let canonical = canonicalize_game_spec(&spec).map_err(|error| {
        adm_new_foundation::AdmError::new(format!("GameSpec projection hash failed: {error}"))
    })?;
    let diff = projection_diff(engine, &normalized, &spec);
    let semantic_hash = sha256_hex(
        serde_json::to_vec(&json!({
            "projectState": normalized,
            "projectionDiff": diff,
        }))
        .unwrap_or_default()
        .as_slice(),
    );
    Ok(GameSpecProjection {
        spec,
        report: GameSpecProjectionReport {
            schema_version: "game_spec_projection_v1".to_string(),
            enabled: true,
            semantic_hash,
            game_spec_hash: canonical.content_hash,
            validation_error_count: validation.error_count(),
            validation_warning_count: validation.issues.len() - validation.error_count(),
            diff,
        },
    })
}

pub fn write_game_spec_shadow_outputs(
    artifact_dir: &Path,
    engine: &DesignEngineService,
    project_state: &ProjectState,
) -> AdmResult<Value> {
    let projection = project_state_to_game_spec(engine, project_state)?;
    let root = artifact_dir.join("game_spec_v2_shadow");
    std::fs::create_dir_all(&root)?;
    let spec_path = io::write_json_serializable(&root.join("game_spec.json"), &projection.spec)?;
    let diff_path = io::write_json(&root.join("diff_report.json"), &projection.report.diff)?;
    let report_path =
        io::write_json_serializable(&root.join("projection_report.json"), &projection.report)?;
    Ok(json!({
        "enabled": true,
        "gameSpecPath": spec_path.to_string_lossy().replace('\\', "/"),
        "diffReportPath": diff_path.to_string_lossy().replace('\\', "/"),
        "projectionReportPath": report_path.to_string_lossy().replace('\\', "/"),
        "semanticHash": projection.report.semantic_hash,
        "gameSpecHash": projection.report.game_spec_hash,
        "validationErrorCount": projection.report.validation_error_count,
        "validationWarningCount": projection.report.validation_warning_count,
    }))
}

fn build_game_spec(_engine: &DesignEngineService, state: &ProjectState) -> GameSpec {
    let project_id = id(&safe_spec_id(&state.project_name, "project"));
    let promise_id = id("core_experience");
    let actor_id = id("player_actor");
    let object_id = id("primary_object");
    let component_id = id("semantic_state");
    let action_id = id("perform_core_loop");
    let resource_id = id("focus");
    let machine_id = id("playable_flow");
    let start_state = id("ready");
    let end_state = id("resolved");
    let scenario_id = id("complete_core_loop");
    let summary = first_non_empty(&[
        state.project_name.as_str(),
        first_design_note(state).as_deref().unwrap_or(""),
        "Projected from the current design workbench state.",
    ]);
    GameSpec {
        identity: SpecIdentity {
            schema_version: adm_new_game_spec::GAME_SPEC_SCHEMA_VERSION.to_string(),
            project_id,
            revision: 1,
            parent_hash: None,
        },
        intent: ProjectIntent {
            title: if state.project_name.trim().is_empty() {
                "Untitled GameSpec Projection".to_string()
            } else {
                state.project_name.clone()
            },
            summary,
            experience_promises: BTreeMap::from([(
                promise_id.clone(),
                ExperiencePromise {
                    statement: first_design_note(state).unwrap_or_else(|| {
                        "Deliver a playable loop from confirmed design choices.".to_string()
                    }),
                    priority: adm_new_game_spec::PromisePriority::Primary,
                },
            )]),
            audiences: vec!["game designers".to_string()],
            target_platforms: vec!["windows".to_string()],
            success_metrics: vec![
                "Projected GameSpec validates without schema errors.".to_string(),
            ],
            scope: ScopeEnvelope {
                must_have: selected_node_ids(state),
                wont_have: vec!["network service dependency".to_string()],
                maximum_session_minutes: Some(30),
            },
        },
        capabilities: infer_capabilities(state),
        entities: BTreeMap::from([
            (
                actor_id.clone(),
                EntitySpec {
                    summary: "Player-controlled decision source projected from the workbench."
                        .to_string(),
                    components: vec![component_id.clone()],
                    tags: BTreeSet::from(["player_controlled".to_string()]),
                },
            ),
            (
                object_id.clone(),
                EntitySpec {
                    summary: "Primary gameplay object derived from confirmed design content."
                        .to_string(),
                    components: vec![component_id.clone()],
                    tags: BTreeSet::from(["projected".to_string()]),
                },
            ),
        ]),
        components: BTreeMap::from([(
            component_id,
            ComponentSpec {
                summary: "Stores projected semantic state.".to_string(),
                properties: BTreeMap::from([(
                    id("state"),
                    PropertySpec {
                        value_kind: ValueKind::Text,
                        required: true,
                        default: Some(json!("ready")),
                        constraints: BTreeMap::new(),
                    },
                )]),
            },
        )]),
        relationships: BTreeMap::from([(
            id("actor_targets_object"),
            RelationshipSpec {
                summary: "The player actor targets the primary projected object.".to_string(),
                source: spec_ref(SpecKind::Entity, &actor_id),
                target: spec_ref(SpecKind::Entity, &object_id),
                relation: RelationshipKind::Targets,
                cardinality: adm_new_game_spec::RelationshipCardinality::OneToOne,
            },
        )]),
        actions: BTreeMap::from([(
            action_id.clone(),
            ActionSpec {
                summary: "Execute the projected core loop once.".to_string(),
                actors: vec![spec_ref(SpecKind::Entity, &actor_id)],
                targets: vec![spec_ref(SpecKind::Entity, &object_id)],
                inputs: vec![InputSpec {
                    channel: "primary".to_string(),
                    command: "confirm".to_string(),
                }],
                preconditions: vec![always("The projected flow is ready.")],
                effects: vec![
                    EffectSpec::ChangeResource {
                        resource: resource_id.clone(),
                        amount: -1,
                    },
                    EffectSpec::TransitionState {
                        state_machine: machine_id.clone(),
                        target_state: end_state.clone(),
                    },
                ],
                feedback: vec![FeedbackSpec {
                    channel: "ui".to_string(),
                    message: "Projected loop feedback is visible.".to_string(),
                }],
                timing: Default::default(),
            },
        )]),
        state_machines: BTreeMap::from([(
            machine_id.clone(),
            StateMachineSpec {
                summary: "Minimal playable flow projected from D1-D4.".to_string(),
                initial_state: start_state.clone(),
                states: BTreeMap::from([
                    (
                        start_state.clone(),
                        StateSpec {
                            summary: "Ready to act.".to_string(),
                            terminal: false,
                        },
                    ),
                    (
                        end_state.clone(),
                        StateSpec {
                            summary: "Projected loop resolved.".to_string(),
                            terminal: true,
                        },
                    ),
                ]),
                transitions: vec![adm_new_game_spec::TransitionSpec {
                    transition_id: id("resolve_projected_loop"),
                    from: start_state,
                    to: end_state,
                    trigger: TriggerSpec {
                        source: TriggerSource::Action,
                        reference: Some(spec_ref(SpecKind::Action, &action_id)),
                    },
                    guards: Vec::new(),
                    effects: Vec::new(),
                }],
            },
        )]),
        resources: BTreeMap::from([(
            resource_id.clone(),
            adm_new_game_spec::ResourceSpec {
                summary: "Small execution budget for the projected loop.".to_string(),
                unit: "points".to_string(),
                initial: 1,
                minimum: Some(0),
                maximum: Some(1),
                source_actions: Vec::new(),
                sink_actions: vec![action_id.clone()],
            },
        )]),
        spaces: BTreeMap::from([(
            id("projected_space"),
            SpaceSpec {
                summary: "Projection workspace for the current design.".to_string(),
                topology: infer_capabilities(state).space.topology,
                regions: BTreeMap::new(),
                connections: Vec::new(),
            },
        )]),
        time: TimeSpec {
            progression: infer_capabilities(state).time.progression,
            fixed_step_hz: Some(30),
            pausable: true,
            phases: BTreeMap::from([
                (
                    id("planning"),
                    TimePhaseSpec {
                        summary: "Review projected choices.".to_string(),
                        duration_ms: None,
                    },
                ),
                (
                    id("resolution"),
                    TimePhaseSpec {
                        summary: "Resolve the loop.".to_string(),
                        duration_ms: None,
                    },
                ),
            ]),
        },
        interactions: BTreeMap::from([(
            id("primary_input"),
            InteractionSpec {
                summary: "Primary user input for the projected loop.".to_string(),
                direction: InteractionDirection::Input,
                modality: "pointer_or_keyboard".to_string(),
                source_actions: vec![action_id.clone()],
            },
        )]),
        content: BTreeMap::from([(
            id("authored_projection"),
            ContentSpec {
                summary: "Authored content projected from workbench decisions.".to_string(),
                generation: ContentGeneration::Authored,
                item_kind: "design_projection".to_string(),
                source_refs: vec![spec_ref(SpecKind::Action, &action_id)],
            },
        )]),
        presentation: BTreeMap::from([(
            id("projection_readability"),
            PresentationSpec {
                summary: "Projection must remain readable to designers and developers.".to_string(),
                medium: "ui".to_string(),
                constraints: BTreeMap::from([("clarity".to_string(), json!("high"))]),
                source_refs: vec![spec_ref(SpecKind::Intent, &promise_id)],
            },
        )]),
        technical: TechnicalConstraints {
            product_envelope: medium_envelope(),
            target_engine: Some("unity".to_string()),
            platforms: vec!["windows".to_string()],
            performance_budgets: BTreeMap::from([("activeEntities".to_string(), 300)]),
            save_requirements: vec![
                "Projection is read-only and must not overwrite ProjectState.".to_string(),
            ],
            accessibility_requirements: vec![
                "Projected UI feedback must not rely on color alone.".to_string(),
            ],
        },
        acceptance_scenarios: BTreeMap::from([(
            scenario_id.clone(),
            AcceptanceScenario {
                summary: "The projected core loop can be executed once.".to_string(),
                given: vec![always("The projected flow starts ready.")],
                when: vec![ActionInvocation {
                    action: action_id.clone(),
                    actor: Some(spec_ref(SpecKind::Entity, &actor_id)),
                    targets: vec![spec_ref(SpecKind::Entity, &object_id)],
                }],
                then: vec![always("The resolved state is observable.")],
                failure_case: false,
                manual_review_required: false,
                performance_budget_refs: Vec::new(),
                asset_validation_required: false,
            },
        )]),
        trace_links: BTreeMap::from([(
            id("promise_to_projected_loop"),
            TraceLink {
                source: spec_ref(SpecKind::Intent, &promise_id),
                target: spec_ref(SpecKind::Scenario, &scenario_id),
                relation: TraceRelation::Verifies,
                rationale: "The scenario verifies the projected core experience promise."
                    .to_string(),
            },
        )]),
        extensions: BTreeMap::new(),
    }
}

fn projection_diff(engine: &DesignEngineService, state: &ProjectState, spec: &GameSpec) -> Value {
    let view = engine.view_model(state);
    json!({
        "mode": "read_only_shadow_projection",
        "backPropagatesToProjectState": false,
        "projectName": state.project_name,
        "workbench": {
            "nodeCount": state.nodes.len(),
            "selectedNodeCount": selected_node_ids(state).len(),
            "selectedGameplaySystemCount": state.gameplay_systems.selected.len(),
            "coverage": {
                "nodePercent": view.project_coverage.node_percent,
                "checklistPercent": view.project_coverage.checklist_percent
            }
        },
        "gameSpec": {
            "projectId": spec.identity.project_id.to_string(),
            "entityCount": spec.entities.len(),
            "actionCount": spec.actions.len(),
            "scenarioCount": spec.acceptance_scenarios.len(),
            "capabilitySummary": spec.capabilities
        }
    })
}

fn infer_capabilities(state: &ProjectState) -> CapabilityProfile {
    let text = flattened_project_text(state);
    let lane = contains_any(
        &text,
        &["lane", "tower", "defense", "塔防", "防线", "波次", "格子"],
    );
    let turn = contains_any(&text, &["turn", "tactic", "回合", "战棋"]);
    CapabilityProfile {
        space: SpaceCapability {
            topology: if lane {
                SpaceTopology::Lane
            } else {
                SpaceTopology::Graph
            },
            dimensionality: Dimensionality::TwoD,
        },
        time: adm_new_game_spec::TimeCapability {
            progression: if turn {
                TimeProgression::TurnBased
            } else {
                TimeProgression::Realtime
            },
            simultaneity: if turn {
                Simultaneity::Sequential
            } else {
                Simultaneity::Simultaneous
            },
        },
        control: ControlCapability {
            cardinality: if lane {
                ControlCardinality::Many
            } else {
                ControlCardinality::Single
            },
            directness: if lane {
                ControlDirectness::Indirect
            } else {
                ControlDirectness::Direct
            },
        },
        participants: ParticipantCapability {
            mode: ParticipantMode::Solo,
            asymmetry: ParticipantAsymmetry::NotApplicable,
        },
        information: InformationCapability {
            visibility: InformationVisibility::Partial,
            uncertainty: adm_new_game_spec::UncertaintySource::Mixed,
        },
        progression: ProgressionCapability {
            persistence: ProgressionPersistence::Persistent,
            structure: ProgressionStructure::Linear,
        },
        content: adm_new_game_spec::ContentCapability {
            generation: ContentGeneration::Authored,
            mutability: ContentMutability::RuntimeMutable,
        },
        connectivity: ConnectivityCapability {
            model: ConnectivityModel::Offline,
            authority: SimulationAuthority::Local,
        },
    }
}

fn first_design_note(state: &ProjectState) -> Option<String> {
    state.nodes.values().find_map(|node| {
        let text = node.design_note.trim();
        (!text.is_empty()).then(|| text.to_string())
    })
}

fn selected_node_ids(state: &ProjectState) -> Vec<String> {
    state
        .nodes
        .iter()
        .filter(|(_, node)| {
            matches!(
                node.decision_state,
                DecisionState::Selected | DecisionState::Completed | DecisionState::Risk
            ) || !node.design_note.trim().is_empty()
        })
        .map(|(id, _)| id.clone())
        .collect()
}

fn flattened_project_text(state: &ProjectState) -> String {
    let mut parts = vec![state.project_name.clone()];
    parts.extend(state.profile.values().map(value_text));
    parts.extend(state.gameplay_systems.selected.iter().cloned());
    parts.extend(state.nodes.values().map(|node| node.design_note.clone()));
    parts.join(" ").to_lowercase()
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles
        .iter()
        .any(|needle| text.contains(&needle.to_lowercase()))
}

fn always(description: &str) -> ConditionSpec {
    ConditionSpec {
        description: description.to_string(),
        reads: Vec::new(),
        expression: ConditionExpr::Always,
    }
}

fn spec_ref(kind: SpecKind, id: &SpecId) -> SpecRef {
    SpecRef {
        kind,
        id: id.clone(),
        path: None,
    }
}

fn safe_spec_id(value: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut last_sep = false;
    for ch in value.to_ascii_lowercase().chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            out.push(ch);
            last_sep = false;
        } else if !last_sep {
            out.push('_');
            last_sep = true;
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        return fallback.to_string();
    }
    let mut out = if out
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_lowercase())
    {
        out
    } else {
        format!("{fallback}_{out}")
    };
    if out.len() > 96 {
        let digest = sha256_hex(value.as_bytes());
        let suffix = format!("_{}", &digest[..12]);
        let max_prefix_len = 96usize.saturating_sub(suffix.len());
        let mut truncated = String::new();
        for ch in out.chars() {
            if truncated.len() + ch.len_utf8() > max_prefix_len {
                break;
            }
            truncated.push(ch);
        }
        while truncated.ends_with(['_', '-', '.']) {
            truncated.pop();
        }
        if truncated.is_empty()
            || !truncated
                .chars()
                .next()
                .is_some_and(|first| first.is_ascii_lowercase())
        {
            truncated = fallback.to_string();
        }
        out = format!("{truncated}{suffix}");
    }
    out
}

fn id(value: &str) -> SpecId {
    SpecId::new(value).expect("projection static id must be valid")
}

fn first_non_empty(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .unwrap_or("Projected GameSpec.")
        .to_string()
}

fn medium_envelope() -> ProductEnvelope {
    ProductEnvelope {
        scene_scale: ProductionScale::Medium,
        system_complexity: ProductionScale::Medium,
        asset_scale: ProductionScale::Medium,
        content_volume: ProductionScale::Medium,
    }
}

fn default_freeze_policy() -> String {
    "attended".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DesignChecklistItemSpec, DesignNodeSpec};

    fn service() -> DesignEngineService {
        DesignEngineService::new(vec![DesignNodeSpec {
            node_id: "core_loop_decision".to_string(),
            domain_id: "core".to_string(),
            name: "Core Loop".to_string(),
            description: String::new(),
            role_class: String::new(),
            checklist: vec![DesignChecklistItemSpec {
                item_id: "loop".to_string(),
                label: "Loop".to_string(),
                option_groups: Vec::new(),
            }],
        }])
    }

    #[test]
    fn projection_generates_valid_stable_game_spec_without_mutating_project_state() {
        let service = service();
        let mut state = service.empty_state();
        state.project_name = "Lane Defense Prototype".to_string();
        state.gameplay_systems.selected = vec!["resource_economy".to_string()];
        state
            .nodes
            .get_mut("core_loop_decision")
            .unwrap()
            .decision_state = DecisionState::Selected;
        state
            .nodes
            .get_mut("core_loop_decision")
            .unwrap()
            .design_note = "Lane defense loop with authored waves.".to_string();
        let before = state.clone();

        let first = project_state_to_game_spec(&service, &state).unwrap();
        let second = project_state_to_game_spec(&service, &state).unwrap();

        assert_eq!(state, before);
        assert_eq!(first.report.semantic_hash, second.report.semantic_hash);
        assert_eq!(first.report.game_spec_hash, second.report.game_spec_hash);
        assert_eq!(first.report.validation_error_count, 0);
        assert_eq!(first.spec.capabilities.space.topology, SpaceTopology::Lane);
        assert_eq!(
            first.report.diff["backPropagatesToProjectState"],
            json!(false)
        );
    }

    #[test]
    fn shadow_projection_writes_spec_diff_and_report() {
        let service = service();
        let state = service.empty_state();
        let root = std::env::temp_dir()
            .join(adm_new_foundation::new_stable_id("game_spec_projection").unwrap());
        std::fs::create_dir_all(&root).unwrap();

        let report = write_game_spec_shadow_outputs(&root, &service, &state).unwrap();

        assert_eq!(report["enabled"], json!(true));
        assert!(root.join("game_spec_v2_shadow/game_spec.json").exists());
        assert!(root.join("game_spec_v2_shadow/diff_report.json").exists());
        assert!(
            root.join("game_spec_v2_shadow/projection_report.json")
                .exists()
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
