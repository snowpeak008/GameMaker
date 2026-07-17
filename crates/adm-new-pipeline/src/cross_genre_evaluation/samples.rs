use adm_new_foundation::{AdmError, AdmResult};
use adm_new_game_spec::parse_game_spec;
use serde_json::{Value, json};

use crate::cross_genre_evaluation::types::{A09ProductionScope, A09Sample};

pub(super) fn a09_samples() -> AdmResult<Vec<A09Sample>> {
    Ok(vec![
        fixture_sample(
            "r1c0_micro_ecodome_lane_guard",
            "R1-C0 Micro Ecodome Lane Guard",
            "channel_defense",
            A09ProductionScope::R1Reference,
            include_str!(
                "../../../../testdata/game_spec/r1c0_micro_ecodome_lane_guard_frozen.json"
            ),
        )?,
        fixture_sample(
            "match_grid_sample",
            "Match Grid Sample",
            "match_grid",
            A09ProductionScope::FullProduction,
            include_str!("../../../../testdata/game_spec/match_grid.json"),
        )?,
        fixture_sample(
            "branching_story_sample",
            "Branching Story Sample",
            "branching_story",
            A09ProductionScope::FullProduction,
            include_str!("../../../../testdata/game_spec/branching_story.json"),
        )?,
        fixture_sample(
            "turn_tactics_sample",
            "Turn Tactics Sample",
            "turn_tactics",
            A09ProductionScope::FullProduction,
            include_str!("../../../../testdata/game_spec/turn_tactics.json"),
        )?,
        generated_sample(GeneratedSample {
            sample_id: "action_roguelite_sample",
            display_name: "Action Roguelite Sample",
            structure_family: "action_roguelite",
            production_scope: A09ProductionScope::SpecLevelOnly,
            promise_id: "readable_risk_reward",
            promise: "Short realtime choices expose risk, reward, and recovery windows.",
            action_id: "dash_strike",
            actor_id: "runner",
            target_id: "hazard",
            resource_id: "stamina",
            resource_unit: "points",
            topology: "continuous",
            dimensionality: "two_d",
            time_progression: "realtime",
            simultaneity: "simultaneous",
            control_cardinality: "single",
            control_directness: "direct",
            participant_mode: "solo",
            participant_asymmetry: "not_applicable",
            information_visibility: "partial",
            uncertainty: "execution",
            persistence: "session_only",
            progression_structure: "cyclical",
            content_generation: "procedural",
            content_mutability: "runtime_mutable",
            connectivity_model: "offline",
            connectivity_authority: "local",
            platform: "windows",
            item_kind: "room",
            medium: "visual_audio",
        })?,
        generated_sample(GeneratedSample {
            sample_id: "deck_builder_sample",
            display_name: "Deck Builder Sample",
            structure_family: "deck_builder",
            production_scope: A09ProductionScope::SpecLevelOnly,
            promise_id: "transparent_card_synergy",
            promise: "Every played card explains cost, target, and follow-up synergy.",
            action_id: "play_card",
            actor_id: "player",
            target_id: "card",
            resource_id: "energy",
            resource_unit: "energy",
            topology: "none",
            dimensionality: "abstract",
            time_progression: "turn_based",
            simultaneity: "sequential",
            control_cardinality: "single",
            control_directness: "command",
            participant_mode: "solo",
            participant_asymmetry: "not_applicable",
            information_visibility: "complete",
            uncertainty: "randomness",
            persistence: "persistent",
            progression_structure: "branching",
            content_generation: "mixed",
            content_mutability: "runtime_mutable",
            connectivity_model: "offline",
            connectivity_authority: "local",
            platform: "desktop",
            item_kind: "card",
            medium: "ui_audio",
        })?,
        generated_sample(GeneratedSample {
            sample_id: "management_builder_sample",
            display_name: "Management Builder Sample",
            structure_family: "management_builder",
            production_scope: A09ProductionScope::SpecLevelOnly,
            promise_id: "legible_system_pressure",
            promise: "Facilities, budgets, and demand pressure remain readable before commitment.",
            action_id: "build_facility",
            actor_id: "planner",
            target_id: "facility",
            resource_id: "budget",
            resource_unit: "credits",
            topology: "grid",
            dimensionality: "two_d",
            time_progression: "event_driven",
            simultaneity: "mixed",
            control_cardinality: "many",
            control_directness: "indirect",
            participant_mode: "solo",
            participant_asymmetry: "not_applicable",
            information_visibility: "mixed",
            uncertainty: "hidden_state",
            persistence: "persistent",
            progression_structure: "open",
            content_generation: "authored",
            content_mutability: "runtime_mutable",
            connectivity_model: "offline",
            connectivity_authority: "local",
            platform: "windows",
            item_kind: "facility_plan",
            medium: "visual_ui",
        })?,
        generated_sample(GeneratedSample {
            sample_id: "network_coop_sample",
            display_name: "Network Coop Sample",
            structure_family: "network_coop",
            production_scope: A09ProductionScope::ArchitectureOnly,
            promise_id: "shared_state_consistency",
            promise: "Players receive consistent shared state feedback after each synchronized action.",
            action_id: "sync_action",
            actor_id: "host_player",
            target_id: "peer_player",
            resource_id: "team_tokens",
            resource_unit: "tokens",
            topology: "graph",
            dimensionality: "abstract",
            time_progression: "hybrid",
            simultaneity: "simultaneous",
            control_cardinality: "party",
            control_directness: "mixed",
            participant_mode: "networked",
            participant_asymmetry: "symmetric",
            information_visibility: "mixed",
            uncertainty: "mixed",
            persistence: "mixed",
            progression_structure: "linear",
            content_generation: "authored",
            content_mutability: "runtime_mutable",
            connectivity_model: "client_server",
            connectivity_authority: "server",
            platform: "windows",
            item_kind: "shared_objective",
            medium: "network_ui_audio",
        })?,
    ])
}

fn fixture_sample(
    sample_id: &str,
    display_name: &str,
    structure_family: &str,
    production_scope: A09ProductionScope,
    input: &str,
) -> AdmResult<A09Sample> {
    let spec = parse_game_spec(input).map_err(|error| AdmError::new(error.to_string()))?;
    Ok(A09Sample {
        sample_id: sample_id.to_string(),
        display_name: display_name.to_string(),
        structure_family: structure_family.to_string(),
        production_scope,
        spec,
    })
}

struct GeneratedSample {
    sample_id: &'static str,
    display_name: &'static str,
    structure_family: &'static str,
    production_scope: A09ProductionScope,
    promise_id: &'static str,
    promise: &'static str,
    action_id: &'static str,
    actor_id: &'static str,
    target_id: &'static str,
    resource_id: &'static str,
    resource_unit: &'static str,
    topology: &'static str,
    dimensionality: &'static str,
    time_progression: &'static str,
    simultaneity: &'static str,
    control_cardinality: &'static str,
    control_directness: &'static str,
    participant_mode: &'static str,
    participant_asymmetry: &'static str,
    information_visibility: &'static str,
    uncertainty: &'static str,
    persistence: &'static str,
    progression_structure: &'static str,
    content_generation: &'static str,
    content_mutability: &'static str,
    connectivity_model: &'static str,
    connectivity_authority: &'static str,
    platform: &'static str,
    item_kind: &'static str,
    medium: &'static str,
}

fn generated_sample(input: GeneratedSample) -> AdmResult<A09Sample> {
    let value = sample_value(&input);
    let spec =
        parse_game_spec(&value.to_string()).map_err(|error| AdmError::new(error.to_string()))?;
    Ok(A09Sample {
        sample_id: input.sample_id.to_string(),
        display_name: input.display_name.to_string(),
        structure_family: input.structure_family.to_string(),
        production_scope: input.production_scope,
        spec,
    })
}

fn sample_value(input: &GeneratedSample) -> Value {
    let success_id = format!("{}_success", input.action_id);
    let blocked_id = format!("{}_blocked", input.action_id);
    json!({
        "identity": {
            "schemaVersion": "2.0.0-alpha.1",
            "projectId": input.sample_id,
            "revision": 1
        },
        "intent": {
            "title": input.display_name,
            "summary": format!("A compact {} validation sample.", input.structure_family),
            "experiencePromises": {
                input.promise_id: { "statement": input.promise, "priority": "primary" }
            },
            "audiences": ["validation players"],
            "targetPlatforms": [input.platform],
            "scope": {
                "mustHave": ["one complete core action", "positive and rejection scenarios"],
                "wontHave": ["unbounded content"],
                "maximumSessionMinutes": 20
            }
        },
        "capabilities": {
            "space": { "topology": input.topology, "dimensionality": input.dimensionality },
            "time": { "progression": input.time_progression, "simultaneity": input.simultaneity },
            "control": { "cardinality": input.control_cardinality, "directness": input.control_directness },
            "participants": { "mode": input.participant_mode, "asymmetry": input.participant_asymmetry },
            "information": { "visibility": input.information_visibility, "uncertainty": input.uncertainty },
            "progression": { "persistence": input.persistence, "structure": input.progression_structure },
            "content": { "generation": input.content_generation, "mutability": input.content_mutability },
            "connectivity": { "model": input.connectivity_model, "authority": input.connectivity_authority }
        },
        "entities": {
            input.actor_id: {
                "summary": "The entity that initiates the core action.",
                "components": ["runtime_status"],
                "tags": ["player_controlled"]
            },
            input.target_id: {
                "summary": "The entity affected by the core action.",
                "components": ["runtime_status"],
                "tags": ["objective"]
            }
        },
        "components": {
            "runtime_status": {
                "summary": "Shared status used by the validation scenarios.",
                "properties": {
                    "ready": { "valueKind": "boolean", "required": true, "default": true }
                }
            }
        },
        "relationships": {
            "actor_targets_object": {
                "summary": "The actor can target the object through the core action.",
                "source": { "kind": "entity", "id": input.actor_id },
                "target": { "kind": "entity", "id": input.target_id },
                "relation": "targets",
                "cardinality": "many_to_many"
            }
        },
        "actions": {
            input.action_id: {
                "summary": "Commit the sample's core action when its budget is available.",
                "actors": [{ "kind": "entity", "id": input.actor_id }],
                "targets": [{ "kind": "entity", "id": input.target_id }],
                "inputs": [{ "channel": "primary_input", "command": input.action_id }],
                "preconditions": [{
                    "description": "The action budget is available.",
                    "reads": [{ "kind": "resource", "id": input.resource_id }],
                    "expression": {
                        "kind": "compare",
                        "source": { "kind": "resource", "id": input.resource_id },
                        "operator": "greater_or_equal",
                        "value": 1
                    }
                }],
                "effects": [
                    { "kind": "change_resource", "resource": input.resource_id, "amount": -1 },
                    { "kind": "transition_state", "stateMachine": "sample_flow", "targetState": "resolved" }
                ],
                "feedback": [{ "channel": "ui", "message": "Show cost, target, and result." }],
                "timing": { "durationMs": 250 }
            }
        },
        "stateMachines": {
            "sample_flow": {
                "summary": "A minimal flow for the sample action.",
                "initialState": "active",
                "states": {
                    "active": { "summary": "The action may be attempted." },
                    "resolved": { "summary": "The action outcome is visible.", "terminal": true }
                },
                "transitions": [{
                    "transitionId": "commit_action",
                    "from": "active",
                    "to": "resolved",
                    "trigger": { "source": "action", "reference": { "kind": "action", "id": input.action_id } }
                }]
            }
        },
        "resources": {
            input.resource_id: {
                "summary": "The bounded budget for the sample action.",
                "unit": input.resource_unit,
                "initial": 3,
                "minimum": 0,
                "maximum": 3,
                "sinkActions": [input.action_id]
            }
        },
        "spaces": {
            "sample_space": {
                "summary": "The authored play surface for this structure family.",
                "topology": input.topology,
                "regions": {
                    "start": { "summary": "Initial region.", "tags": ["start"] },
                    "goal": { "summary": "Outcome region.", "tags": ["goal"] }
                },
                "connections": [{ "from": "start", "to": "goal", "bidirectional": true }]
            }
        },
        "time": {
            "progression": input.time_progression,
            "pausable": true,
            "phases": {
                "ready": { "summary": "Await the action." },
                "resolve": { "summary": "Apply the visible outcome." }
            }
        },
        "interactions": {
            "primary_interaction": {
                "summary": "Invoke the sample action.",
                "direction": "input",
                "modality": "primary_input",
                "sourceActions": [input.action_id]
            }
        },
        "content": {
            "sample_content": {
                "summary": "Bounded content for the sample.",
                "generation": input.content_generation,
                "itemKind": input.item_kind,
                "sourceRefs": [{ "kind": "space", "id": "sample_space" }]
            }
        },
        "presentation": {
            "sample_feedback": {
                "summary": "Expose action cost, target, and outcome clearly.",
                "medium": input.medium,
                "constraints": { "readability": "high" },
                "sourceRefs": [{ "kind": "intent", "id": input.promise_id }]
            }
        },
        "technical": {
            "productEnvelope": {
                "sceneScale": "medium",
                "systemComplexity": "medium",
                "assetScale": "medium",
                "contentVolume": "medium"
            },
            "platforms": [input.platform],
            "performanceBudgets": { "frameTimeMicros": 16667 },
            "saveRequirements": ["Persist the minimum sample state."],
            "accessibilityRequirements": ["Important feedback must be visible without color alone."]
        },
        "acceptanceScenarios": {
            success_id: {
                "summary": "The core action succeeds when its budget is available.",
                "given": [{
                    "description": "The budget is available.",
                    "reads": [{ "kind": "resource", "id": input.resource_id }],
                    "expression": {
                        "kind": "compare",
                        "source": { "kind": "resource", "id": input.resource_id },
                        "operator": "greater_or_equal",
                        "value": 1
                    }
                }],
                "when": [{
                    "action": input.action_id,
                    "actor": { "kind": "entity", "id": input.actor_id },
                    "targets": [{ "kind": "entity", "id": input.target_id }]
                }],
                "then": [{ "description": "The resolved state and feedback are visible.", "expression": { "kind": "always" } }]
            },
            blocked_id: {
                "summary": "The core action is rejected when its budget is unavailable.",
                "given": [{
                    "description": "The budget is unavailable.",
                    "reads": [{ "kind": "resource", "id": input.resource_id }],
                    "expression": {
                        "kind": "compare",
                        "source": { "kind": "resource", "id": input.resource_id },
                        "operator": "less_or_equal",
                        "value": 0
                    }
                }],
                "when": [{
                    "action": input.action_id,
                    "actor": { "kind": "entity", "id": input.actor_id },
                    "targets": [{ "kind": "entity", "id": input.target_id }]
                }],
                "then": [{ "description": "No cost is spent and the rejection reason is visible.", "expression": { "kind": "always" } }],
                "failureCase": true
            }
        },
        "traceLinks": {
            "promise_to_action": {
                "source": { "kind": "intent", "id": input.promise_id },
                "target": { "kind": "action", "id": input.action_id },
                "relation": "refines",
                "rationale": "Both positive and failure scenarios invoke the traced action."
            }
        },
        "extensions": {}
    })
}
