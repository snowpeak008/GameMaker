use adm_new_game_spec::{
    CapabilityProfile, ConnectivityModel, ContentGeneration, ContentMutability, ControlCardinality,
    ControlDirectness, Dimensionality, InformationVisibility, ParticipantAsymmetry,
    ParticipantMode, ProgressionPersistence, ProgressionStructure, SimulationAuthority,
    Simultaneity, SpaceTopology, TimeProgression, UncertaintySource,
};

use super::{ActivationReason, DecisionGraphError};

pub(super) const SUPPORTED_DOMAINS: &[&str] = &[
    "balance_design",
    "compliance_risk_design",
    "content_design",
    "core_experience_design",
    "data_validation_design",
    "documentation_collaboration_design",
    "economy_monetization_design",
    "gameplay_system_design",
    "launch_readiness_design",
    "liveops_version_design",
    "presentation_feel_design",
    "product_positioning_design",
    "release_growth_design",
    "retention_lifecycle_design",
    "social_community_design",
    "ux_interface_design",
];

pub(super) const CONDITIONAL_NODE_IDS: &[&str] = &[
    "balance_competition_decision",
    "balance_economy_decision",
    "balance_payment_decision",
    "build_system_decision",
    "character_unit_decision",
    "cinematic_presentation_decision",
    "compliance_abuse_fairness_decision",
    "compliance_community_safety_decision",
    "compliance_fairness_risk_decision",
    "compliance_privacy_data_decision",
    "content_consumption_decision",
    "content_supply_structure_decision",
    "economy_security_dispute_decision",
    "item_resource_content_decision",
    "launch_activity_decision",
    "launch_post_launch_followup_decision",
    "meta_structure_decision",
    "narrative_content_decision",
    "payment_fairness_decision",
    "payment_point_decision",
    "pricing_value_decision",
    "product_structure_decision",
    "quest_event_decision",
    "randomness_system_decision",
    "release_community_warmup_decision",
    "release_preregistration_decision",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CapabilityPath {
    SpaceTopology,
    SpaceDimensionality,
    TimeProgression,
    TimeSimultaneity,
    ControlCardinality,
    ControlDirectness,
    ParticipantsMode,
    ParticipantsAsymmetry,
    InformationVisibility,
    InformationUncertainty,
    ProgressionPersistence,
    ProgressionStructure,
    ContentGeneration,
    ContentMutability,
    ConnectivityModel,
    ConnectivityAuthority,
}

impl CapabilityPath {
    fn pointer(self) -> &'static str {
        match self {
            Self::SpaceTopology => "/capabilities/space/topology",
            Self::SpaceDimensionality => "/capabilities/space/dimensionality",
            Self::TimeProgression => "/capabilities/time/progression",
            Self::TimeSimultaneity => "/capabilities/time/simultaneity",
            Self::ControlCardinality => "/capabilities/control/cardinality",
            Self::ControlDirectness => "/capabilities/control/directness",
            Self::ParticipantsMode => "/capabilities/participants/mode",
            Self::ParticipantsAsymmetry => "/capabilities/participants/asymmetry",
            Self::InformationVisibility => "/capabilities/information/visibility",
            Self::InformationUncertainty => "/capabilities/information/uncertainty",
            Self::ProgressionPersistence => "/capabilities/progression/persistence",
            Self::ProgressionStructure => "/capabilities/progression/structure",
            Self::ContentGeneration => "/capabilities/content/generation",
            Self::ContentMutability => "/capabilities/content/mutability",
            Self::ConnectivityModel => "/capabilities/connectivity/model",
            Self::ConnectivityAuthority => "/capabilities/connectivity/authority",
        }
    }

    fn value(self, profile: &CapabilityProfile) -> &'static str {
        match self {
            Self::SpaceTopology => match profile.space.topology {
                SpaceTopology::None => "none",
                SpaceTopology::Graph => "graph",
                SpaceTopology::Grid => "grid",
                SpaceTopology::Lane => "lane",
                SpaceTopology::Continuous => "continuous",
                SpaceTopology::Hybrid => "hybrid",
            },
            Self::SpaceDimensionality => match profile.space.dimensionality {
                Dimensionality::None => "none",
                Dimensionality::OneD => "one_d",
                Dimensionality::TwoD => "two_d",
                Dimensionality::ThreeD => "three_d",
                Dimensionality::Abstract => "abstract",
            },
            Self::TimeProgression => match profile.time.progression {
                TimeProgression::Realtime => "realtime",
                TimeProgression::TurnBased => "turn_based",
                TimeProgression::EventDriven => "event_driven",
                TimeProgression::Hybrid => "hybrid",
            },
            Self::TimeSimultaneity => match profile.time.simultaneity {
                Simultaneity::Sequential => "sequential",
                Simultaneity::Simultaneous => "simultaneous",
                Simultaneity::Mixed => "mixed",
            },
            Self::ControlCardinality => match profile.control.cardinality {
                ControlCardinality::Single => "single",
                ControlCardinality::Party => "party",
                ControlCardinality::Squad => "squad",
                ControlCardinality::Many => "many",
            },
            Self::ControlDirectness => match profile.control.directness {
                ControlDirectness::Direct => "direct",
                ControlDirectness::Command => "command",
                ControlDirectness::Indirect => "indirect",
                ControlDirectness::Mixed => "mixed",
            },
            Self::ParticipantsMode => match profile.participants.mode {
                ParticipantMode::Solo => "solo",
                ParticipantMode::LocalMulti => "local_multi",
                ParticipantMode::Networked => "networked",
                ParticipantMode::Hybrid => "hybrid",
            },
            Self::ParticipantsAsymmetry => match profile.participants.asymmetry {
                ParticipantAsymmetry::Symmetric => "symmetric",
                ParticipantAsymmetry::Asymmetric => "asymmetric",
                ParticipantAsymmetry::NotApplicable => "not_applicable",
            },
            Self::InformationVisibility => match profile.information.visibility {
                InformationVisibility::Complete => "complete",
                InformationVisibility::Partial => "partial",
                InformationVisibility::Hidden => "hidden",
                InformationVisibility::Mixed => "mixed",
            },
            Self::InformationUncertainty => match profile.information.uncertainty {
                UncertaintySource::None => "none",
                UncertaintySource::Randomness => "randomness",
                UncertaintySource::HiddenState => "hidden_state",
                UncertaintySource::Execution => "execution",
                UncertaintySource::Mixed => "mixed",
            },
            Self::ProgressionPersistence => match profile.progression.persistence {
                ProgressionPersistence::SessionOnly => "session_only",
                ProgressionPersistence::Persistent => "persistent",
                ProgressionPersistence::Mixed => "mixed",
            },
            Self::ProgressionStructure => match profile.progression.structure {
                ProgressionStructure::Linear => "linear",
                ProgressionStructure::Branching => "branching",
                ProgressionStructure::Open => "open",
                ProgressionStructure::Cyclical => "cyclical",
                ProgressionStructure::Mixed => "mixed",
            },
            Self::ContentGeneration => match profile.content.generation {
                ContentGeneration::Authored => "authored",
                ContentGeneration::Procedural => "procedural",
                ContentGeneration::Mixed => "mixed",
            },
            Self::ContentMutability => match profile.content.mutability {
                ContentMutability::Static => "static",
                ContentMutability::RuntimeMutable => "runtime_mutable",
                ContentMutability::UserGenerated => "user_generated",
                ContentMutability::Mixed => "mixed",
            },
            Self::ConnectivityModel => match profile.connectivity.model {
                ConnectivityModel::Offline => "offline",
                ConnectivityModel::PeerToPeer => "peer_to_peer",
                ConnectivityModel::ClientServer => "client_server",
                ConnectivityModel::Hybrid => "hybrid",
            },
            Self::ConnectivityAuthority => match profile.connectivity.authority {
                SimulationAuthority::Local => "local",
                SimulationAuthority::Host => "host",
                SimulationAuthority::Server => "server",
                SimulationAuthority::Distributed => "distributed",
                SimulationAuthority::NotApplicable => "not_applicable",
            },
        }
    }
}

#[derive(Debug, Clone)]
enum Matcher {
    Exists,
    OneOf(&'static [&'static str]),
}

#[derive(Debug, Clone)]
pub(super) struct Predicate {
    id: &'static str,
    path: CapabilityPath,
    matcher: Matcher,
}

#[derive(Debug, Clone)]
pub(super) enum Rule {
    Predicate(Predicate),
    All(Vec<Rule>),
    Any(Vec<Rule>),
}

impl Rule {
    pub(super) fn evaluate(&self, profile: &CapabilityProfile) -> Option<Vec<ActivationReason>> {
        match self {
            Self::Predicate(predicate) => predicate.evaluate(profile).map(|reason| vec![reason]),
            Self::All(rules) => {
                let mut reasons = Vec::new();
                for rule in rules {
                    reasons.extend(rule.evaluate(profile)?);
                }
                normalize_reasons(&mut reasons);
                Some(reasons)
            }
            Self::Any(rules) => {
                let mut matched = false;
                let mut reasons = Vec::new();
                for rule in rules {
                    if let Some(mut rule_reasons) = rule.evaluate(profile) {
                        matched = true;
                        reasons.append(&mut rule_reasons);
                    }
                }
                matched.then(|| {
                    normalize_reasons(&mut reasons);
                    reasons
                })
            }
        }
    }
}

impl Predicate {
    fn evaluate(&self, profile: &CapabilityProfile) -> Option<ActivationReason> {
        let actual = self.path.value(profile);
        let matched = match self.matcher {
            Matcher::Exists => true,
            Matcher::OneOf(values) => values.contains(&actual),
        };
        matched.then(|| ActivationReason {
            predicate_id: self.id.to_string(),
            source_path: self.path.pointer().to_string(),
            operator: match self.matcher {
                Matcher::Exists => "exists",
                Matcher::OneOf(_) => "one_of",
            }
            .to_string(),
            expected: match self.matcher {
                Matcher::Exists => vec!["present".to_string()],
                Matcher::OneOf(values) => values.iter().map(|value| (*value).to_string()).collect(),
            },
            actual: actual.to_string(),
        })
    }
}

fn normalize_reasons(reasons: &mut Vec<ActivationReason>) {
    reasons.sort();
    reasons.dedup();
}

fn exists(id: &'static str, path: CapabilityPath) -> Rule {
    Rule::Predicate(Predicate {
        id,
        path,
        matcher: Matcher::Exists,
    })
}

fn one_of(id: &'static str, path: CapabilityPath, values: &'static [&'static str]) -> Rule {
    Rule::Predicate(Predicate {
        id,
        path,
        matcher: Matcher::OneOf(values),
    })
}

fn any(rules: Vec<Rule>) -> Rule {
    Rule::Any(rules)
}

fn all(rules: Vec<Rule>) -> Rule {
    Rule::All(rules)
}

fn social_rule() -> Rule {
    any(vec![
        one_of(
            "multiple_participants",
            CapabilityPath::ParticipantsMode,
            &["local_multi", "networked", "hybrid"],
        ),
        one_of(
            "remote_connectivity",
            CapabilityPath::ConnectivityModel,
            &["peer_to_peer", "client_server", "hybrid"],
        ),
    ])
}

fn persistent_rule() -> Rule {
    one_of(
        "persistent_progression",
        CapabilityPath::ProgressionPersistence,
        &["persistent", "mixed"],
    )
}

fn service_rule() -> Rule {
    any(vec![
        all(vec![
            persistent_rule(),
            one_of(
                "mutable_content",
                CapabilityPath::ContentMutability,
                &["runtime_mutable", "user_generated", "mixed"],
            ),
        ]),
        one_of(
            "remote_service",
            CapabilityPath::ConnectivityModel,
            &["peer_to_peer", "client_server", "hybrid"],
        ),
    ])
}

fn domain_rule(domain_id: &str) -> Option<Rule> {
    match domain_id {
        "product_positioning_design" => Some(exists(
            "spatial_capability_declared",
            CapabilityPath::SpaceTopology,
        )),
        "core_experience_design" => Some(exists(
            "time_capability_declared",
            CapabilityPath::TimeProgression,
        )),
        "gameplay_system_design" => Some(exists(
            "control_capability_declared",
            CapabilityPath::ControlDirectness,
        )),
        "content_design" => Some(exists(
            "content_capability_declared",
            CapabilityPath::ContentGeneration,
        )),
        "economy_monetization_design" => Some(persistent_rule()),
        "ux_interface_design" => Some(exists(
            "control_cardinality_declared",
            CapabilityPath::ControlCardinality,
        )),
        "presentation_feel_design" => Some(exists(
            "dimensionality_declared",
            CapabilityPath::SpaceDimensionality,
        )),
        "balance_design" => Some(exists(
            "uncertainty_declared",
            CapabilityPath::InformationUncertainty,
        )),
        "social_community_design" => Some(social_rule()),
        "retention_lifecycle_design" => Some(persistent_rule()),
        "liveops_version_design" => Some(service_rule()),
        "data_validation_design" => Some(exists(
            "information_visibility_declared",
            CapabilityPath::InformationVisibility,
        )),
        "compliance_risk_design" => Some(exists(
            "participant_capability_declared",
            CapabilityPath::ParticipantsAsymmetry,
        )),
        "documentation_collaboration_design" => Some(exists(
            "content_mutability_declared",
            CapabilityPath::ContentMutability,
        )),
        "release_growth_design" => Some(exists(
            "connectivity_capability_declared",
            CapabilityPath::ConnectivityAuthority,
        )),
        "launch_readiness_design" => Some(exists(
            "simultaneity_declared",
            CapabilityPath::TimeSimultaneity,
        )),
        _ => None,
    }
}

fn node_rule(node_id: &str) -> Option<Rule> {
    match node_id {
        "build_system_decision" | "character_unit_decision" => Some(any(vec![
            one_of(
                "multi_entity_control",
                CapabilityPath::ControlCardinality,
                &["party", "squad", "many"],
            ),
            one_of(
                "delegated_control",
                CapabilityPath::ControlDirectness,
                &["command", "indirect", "mixed"],
            ),
        ])),
        "randomness_system_decision" => Some(any(vec![
            one_of(
                "stochastic_uncertainty",
                CapabilityPath::InformationUncertainty,
                &["randomness", "mixed"],
            ),
            one_of(
                "generated_content",
                CapabilityPath::ContentGeneration,
                &["procedural", "mixed"],
            ),
        ])),
        "meta_structure_decision" | "content_consumption_decision" => Some(persistent_rule()),
        "quest_event_decision" => Some(any(vec![
            one_of(
                "event_driven_time",
                CapabilityPath::TimeProgression,
                &["event_driven", "hybrid"],
            ),
            one_of(
                "nonlinear_progression",
                CapabilityPath::ProgressionStructure,
                &["branching", "open", "cyclical", "mixed"],
            ),
        ])),
        "item_resource_content_decision" | "content_supply_structure_decision" => Some(any(vec![
            one_of(
                "generated_content",
                CapabilityPath::ContentGeneration,
                &["procedural", "mixed"],
            ),
            one_of(
                "mutable_content",
                CapabilityPath::ContentMutability,
                &["runtime_mutable", "user_generated", "mixed"],
            ),
        ])),
        "narrative_content_decision" | "cinematic_presentation_decision" => Some(one_of(
            "nonlinear_progression",
            CapabilityPath::ProgressionStructure,
            &["branching", "open", "mixed"],
        )),
        "balance_economy_decision" => Some(persistent_rule()),
        "balance_competition_decision"
        | "compliance_abuse_fairness_decision"
        | "compliance_community_safety_decision"
        | "release_community_warmup_decision" => Some(social_rule()),
        "balance_payment_decision"
        | "economy_security_dispute_decision"
        | "launch_activity_decision"
        | "launch_post_launch_followup_decision"
        | "payment_fairness_decision"
        | "payment_point_decision"
        | "pricing_value_decision"
        | "product_structure_decision"
        | "release_preregistration_decision" => Some(service_rule()),
        "compliance_fairness_risk_decision" => Some(any(vec![
            social_rule(),
            one_of(
                "stochastic_or_execution_uncertainty",
                CapabilityPath::InformationUncertainty,
                &["randomness", "execution", "mixed"],
            ),
        ])),
        "compliance_privacy_data_decision" => Some(one_of(
            "remote_data_processing",
            CapabilityPath::ConnectivityModel,
            &["peer_to_peer", "client_server", "hybrid"],
        )),
        _ => None,
    }
}

pub(super) fn effective_rule(domain_id: &str, node_id: &str) -> Result<Rule, DecisionGraphError> {
    if let Some(rule) = node_rule(node_id) {
        return Ok(rule);
    }
    domain_rule(domain_id).ok_or_else(|| {
        DecisionGraphError::new(
            "decision_graph.unsupported_domain",
            format!("/knowledge/domains/{domain_id}"),
            format!("no capability activation policy exists for domain '{domain_id}'"),
            "add a capability-based domain policy before compiling this knowledge domain",
        )
    })
}
