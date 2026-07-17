use std::fmt;

use adm_new_game_spec::{
    ConditionExpr, ConnectivityModel, ContentGeneration, ContentMutability, ControlCardinality,
    ControlDirectness, Dimensionality, GameSpec, InformationVisibility, ParticipantAsymmetry,
    ParticipantMode, ProgressionPersistence, ProgressionStructure, SimulationAuthority,
    Simultaneity, SpaceTopology, SpecId, TimeProgression, UncertaintySource,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AntiOverfitInputError {
    EmptyReplacement,
    MutationDidNotChangeCapability,
}

impl fmt::Display for AntiOverfitInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyReplacement => formatter.write_str("replacement label must not be empty"),
            Self::MutationDidNotChangeCapability => {
                formatter.write_str("capability mutation must change the source specification")
            }
        }
    }
}

impl std::error::Error for AntiOverfitInputError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityMutation {
    SpaceTopology(SpaceTopology),
    SpaceDimensionality(Dimensionality),
    TimeProgression(TimeProgression),
    TimeSimultaneity(Simultaneity),
    ControlCardinality(ControlCardinality),
    ControlDirectness(ControlDirectness),
    ParticipantMode(ParticipantMode),
    ParticipantAsymmetry(ParticipantAsymmetry),
    InformationVisibility(InformationVisibility),
    InformationUncertainty(UncertaintySource),
    ProgressionPersistence(ProgressionPersistence),
    ProgressionStructure(ProgressionStructure),
    ContentGeneration(ContentGeneration),
    ContentMutability(ContentMutability),
    ConnectivityModel(ConnectivityModel),
    ConnectivityAuthority(SimulationAuthority),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityMutationCase {
    pub axis: &'static str,
    pub mutation: CapabilityMutation,
}

/// Replaces fields that are permitted to carry discovery or display labels.
/// Structural capability and gameplay fields remain untouched.
pub fn permute_display_labels(
    source: &GameSpec,
    replacement: &str,
) -> Result<GameSpec, AntiOverfitInputError> {
    let replacement = replacement.trim();
    if replacement.is_empty() {
        return Err(AntiOverfitInputError::EmptyReplacement);
    }

    let mut permuted = source.clone();
    permuted.intent.title = replacement.to_string();
    for (index, audience) in permuted.intent.audiences.iter_mut().enumerate() {
        *audience = format!("{replacement}_{index}");
    }
    permute_entity_tags(&mut permuted, replacement);
    permute_region_tags(&mut permuted, replacement);
    Ok(permuted)
}

pub fn capability_mutation_suite(source: &GameSpec) -> Vec<CapabilityMutationCase> {
    vec![
        CapabilityMutationCase {
            axis: "space_topology",
            mutation: CapabilityMutation::SpaceTopology(alternate_space_topology(
                source.capabilities.space.topology,
            )),
        },
        CapabilityMutationCase {
            axis: "space_dimensionality",
            mutation: CapabilityMutation::SpaceDimensionality(alternate_dimensionality(
                source.capabilities.space.dimensionality,
            )),
        },
        CapabilityMutationCase {
            axis: "time_progression",
            mutation: CapabilityMutation::TimeProgression(alternate_time_progression(
                source.capabilities.time.progression,
            )),
        },
        CapabilityMutationCase {
            axis: "time_simultaneity",
            mutation: CapabilityMutation::TimeSimultaneity(alternate_simultaneity(
                source.capabilities.time.simultaneity,
            )),
        },
        CapabilityMutationCase {
            axis: "control_cardinality",
            mutation: CapabilityMutation::ControlCardinality(alternate_control_cardinality(
                source.capabilities.control.cardinality,
            )),
        },
        CapabilityMutationCase {
            axis: "control_directness",
            mutation: CapabilityMutation::ControlDirectness(alternate_control_directness(
                source.capabilities.control.directness,
            )),
        },
        CapabilityMutationCase {
            axis: "participant_mode",
            mutation: CapabilityMutation::ParticipantMode(alternate_participant_mode(
                source.capabilities.participants.mode,
            )),
        },
        CapabilityMutationCase {
            axis: "participant_asymmetry",
            mutation: CapabilityMutation::ParticipantAsymmetry(alternate_participant_asymmetry(
                source.capabilities.participants.asymmetry,
            )),
        },
        CapabilityMutationCase {
            axis: "information_visibility",
            mutation: CapabilityMutation::InformationVisibility(alternate_information_visibility(
                source.capabilities.information.visibility,
            )),
        },
        CapabilityMutationCase {
            axis: "information_uncertainty",
            mutation: CapabilityMutation::InformationUncertainty(alternate_uncertainty_source(
                source.capabilities.information.uncertainty,
            )),
        },
        CapabilityMutationCase {
            axis: "progression_persistence",
            mutation: CapabilityMutation::ProgressionPersistence(
                alternate_progression_persistence(source.capabilities.progression.persistence),
            ),
        },
        CapabilityMutationCase {
            axis: "progression_structure",
            mutation: CapabilityMutation::ProgressionStructure(alternate_progression_structure(
                source.capabilities.progression.structure,
            )),
        },
        CapabilityMutationCase {
            axis: "content_generation",
            mutation: CapabilityMutation::ContentGeneration(alternate_content_generation(
                source.capabilities.content.generation,
            )),
        },
        CapabilityMutationCase {
            axis: "content_mutability",
            mutation: CapabilityMutation::ContentMutability(alternate_content_mutability(
                source.capabilities.content.mutability,
            )),
        },
        CapabilityMutationCase {
            axis: "connectivity_model",
            mutation: CapabilityMutation::ConnectivityModel(alternate_connectivity_model(
                source.capabilities.connectivity.model,
            )),
        },
        CapabilityMutationCase {
            axis: "connectivity_authority",
            mutation: CapabilityMutation::ConnectivityAuthority(alternate_simulation_authority(
                source.capabilities.connectivity.authority,
            )),
        },
    ]
}

pub fn apply_capability_mutation(
    source: &GameSpec,
    mutation: CapabilityMutation,
) -> Result<GameSpec, AntiOverfitInputError> {
    let mut mutated = source.clone();
    match mutation {
        CapabilityMutation::SpaceTopology(value) => mutated.capabilities.space.topology = value,
        CapabilityMutation::SpaceDimensionality(value) => {
            mutated.capabilities.space.dimensionality = value;
        }
        CapabilityMutation::TimeProgression(value) => {
            mutated.capabilities.time.progression = value;
        }
        CapabilityMutation::TimeSimultaneity(value) => {
            mutated.capabilities.time.simultaneity = value;
        }
        CapabilityMutation::ControlCardinality(value) => {
            mutated.capabilities.control.cardinality = value;
        }
        CapabilityMutation::ControlDirectness(value) => {
            mutated.capabilities.control.directness = value;
        }
        CapabilityMutation::ParticipantMode(value) => {
            mutated.capabilities.participants.mode = value;
        }
        CapabilityMutation::ParticipantAsymmetry(value) => {
            mutated.capabilities.participants.asymmetry = value;
        }
        CapabilityMutation::InformationVisibility(value) => {
            mutated.capabilities.information.visibility = value;
        }
        CapabilityMutation::InformationUncertainty(value) => {
            mutated.capabilities.information.uncertainty = value;
        }
        CapabilityMutation::ProgressionPersistence(value) => {
            mutated.capabilities.progression.persistence = value;
        }
        CapabilityMutation::ProgressionStructure(value) => {
            mutated.capabilities.progression.structure = value;
        }
        CapabilityMutation::ContentGeneration(value) => {
            mutated.capabilities.content.generation = value;
        }
        CapabilityMutation::ContentMutability(value) => {
            mutated.capabilities.content.mutability = value;
        }
        CapabilityMutation::ConnectivityModel(value) => {
            mutated.capabilities.connectivity.model = value;
        }
        CapabilityMutation::ConnectivityAuthority(value) => {
            mutated.capabilities.connectivity.authority = value;
        }
    }

    if mutated == *source {
        return Err(AntiOverfitInputError::MutationDidNotChangeCapability);
    }
    Ok(mutated)
}

fn permute_entity_tags(spec: &mut GameSpec, replacement: &str) {
    let mut rewrites = Vec::new();
    let entity_ids = spec.entities.keys().cloned().collect::<Vec<_>>();
    for (entity_index, entity_id) in entity_ids.iter().enumerate() {
        let Some(entity) = spec.entities.get_mut(entity_id) else {
            continue;
        };
        let original_tags = entity.tags.iter().cloned().collect::<Vec<_>>();
        entity.tags.clear();
        for (tag_index, original_tag) in original_tags.iter().enumerate() {
            let replacement_tag = format!("{replacement}_entity_{entity_index}_{tag_index}");
            entity.tags.insert(replacement_tag.clone());
            rewrites.push((entity_id.clone(), original_tag.clone(), replacement_tag));
        }
    }
    for (entity_id, old_tag, new_tag) in rewrites {
        rewrite_entity_tag_references(spec, &entity_id, &old_tag, &new_tag);
    }
}

fn permute_region_tags(spec: &mut GameSpec, replacement: &str) {
    for (space_index, space) in spec.spaces.values_mut().enumerate() {
        for (region_index, region) in space.regions.values_mut().enumerate() {
            let original_count = region.tags.len();
            region.tags.clear();
            for tag_index in 0..original_count {
                region.tags.insert(format!(
                    "{replacement}_region_{space_index}_{region_index}_{tag_index}"
                ));
            }
        }
    }
}

fn rewrite_entity_tag_references(
    spec: &mut GameSpec,
    entity_id: &SpecId,
    old_tag: &str,
    new_tag: &str,
) {
    for action in spec.actions.values_mut() {
        for condition in &mut action.preconditions {
            rewrite_condition_tag(&mut condition.expression, entity_id, old_tag, new_tag);
        }
    }
    for state_machine in spec.state_machines.values_mut() {
        for transition in &mut state_machine.transitions {
            for condition in &mut transition.guards {
                rewrite_condition_tag(&mut condition.expression, entity_id, old_tag, new_tag);
            }
        }
    }
    for scenario in spec.acceptance_scenarios.values_mut() {
        for condition in &mut scenario.given {
            rewrite_condition_tag(&mut condition.expression, entity_id, old_tag, new_tag);
        }
        for condition in &mut scenario.then {
            rewrite_condition_tag(&mut condition.expression, entity_id, old_tag, new_tag);
        }
    }
}

fn rewrite_condition_tag(
    expression: &mut ConditionExpr,
    entity_id: &SpecId,
    old_tag: &str,
    new_tag: &str,
) {
    match expression {
        ConditionExpr::All { items } | ConditionExpr::Any { items } => {
            for item in items {
                rewrite_condition_tag(item, entity_id, old_tag, new_tag);
            }
        }
        ConditionExpr::Not { item } => rewrite_condition_tag(item, entity_id, old_tag, new_tag),
        ConditionExpr::HasTag { entity, tag } if entity == entity_id && tag == old_tag => {
            *tag = new_tag.to_string();
        }
        ConditionExpr::Always
        | ConditionExpr::Equals { .. }
        | ConditionExpr::Compare { .. }
        | ConditionExpr::HasTag { .. }
        | ConditionExpr::Extension { .. } => {}
    }
}

fn alternate_space_topology(value: SpaceTopology) -> SpaceTopology {
    if value == SpaceTopology::Lane {
        SpaceTopology::Graph
    } else {
        SpaceTopology::Lane
    }
}

fn alternate_dimensionality(value: Dimensionality) -> Dimensionality {
    if value == Dimensionality::TwoD {
        Dimensionality::Abstract
    } else {
        Dimensionality::TwoD
    }
}

fn alternate_time_progression(value: TimeProgression) -> TimeProgression {
    if value == TimeProgression::Realtime {
        TimeProgression::TurnBased
    } else {
        TimeProgression::Realtime
    }
}

fn alternate_simultaneity(value: Simultaneity) -> Simultaneity {
    if value == Simultaneity::Simultaneous {
        Simultaneity::Sequential
    } else {
        Simultaneity::Simultaneous
    }
}

fn alternate_control_cardinality(value: ControlCardinality) -> ControlCardinality {
    if value == ControlCardinality::Single {
        ControlCardinality::Party
    } else {
        ControlCardinality::Single
    }
}

fn alternate_control_directness(value: ControlDirectness) -> ControlDirectness {
    if value == ControlDirectness::Direct {
        ControlDirectness::Command
    } else {
        ControlDirectness::Direct
    }
}

fn alternate_participant_mode(value: ParticipantMode) -> ParticipantMode {
    if value == ParticipantMode::Networked {
        ParticipantMode::Solo
    } else {
        ParticipantMode::Networked
    }
}

fn alternate_participant_asymmetry(value: ParticipantAsymmetry) -> ParticipantAsymmetry {
    if value == ParticipantAsymmetry::Symmetric {
        ParticipantAsymmetry::Asymmetric
    } else {
        ParticipantAsymmetry::Symmetric
    }
}

fn alternate_information_visibility(value: InformationVisibility) -> InformationVisibility {
    if value == InformationVisibility::Complete {
        InformationVisibility::Partial
    } else {
        InformationVisibility::Complete
    }
}

fn alternate_uncertainty_source(value: UncertaintySource) -> UncertaintySource {
    if value == UncertaintySource::None {
        UncertaintySource::Randomness
    } else {
        UncertaintySource::None
    }
}

fn alternate_progression_persistence(value: ProgressionPersistence) -> ProgressionPersistence {
    if value == ProgressionPersistence::Persistent {
        ProgressionPersistence::SessionOnly
    } else {
        ProgressionPersistence::Persistent
    }
}

fn alternate_progression_structure(value: ProgressionStructure) -> ProgressionStructure {
    if value == ProgressionStructure::Linear {
        ProgressionStructure::Branching
    } else {
        ProgressionStructure::Linear
    }
}

fn alternate_content_generation(value: ContentGeneration) -> ContentGeneration {
    if value == ContentGeneration::Authored {
        ContentGeneration::Procedural
    } else {
        ContentGeneration::Authored
    }
}

fn alternate_content_mutability(value: ContentMutability) -> ContentMutability {
    if value == ContentMutability::Static {
        ContentMutability::RuntimeMutable
    } else {
        ContentMutability::Static
    }
}

fn alternate_connectivity_model(value: ConnectivityModel) -> ConnectivityModel {
    if value == ConnectivityModel::Offline {
        ConnectivityModel::ClientServer
    } else {
        ConnectivityModel::Offline
    }
}

fn alternate_simulation_authority(value: SimulationAuthority) -> SimulationAuthority {
    if value == SimulationAuthority::Server {
        SimulationAuthority::Local
    } else {
        SimulationAuthority::Server
    }
}
