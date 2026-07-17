use std::fmt;

use sha2::{Digest, Sha256};

use crate::{ConditionExpr, ConditionSpec, GameSpec, RegionConnection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalGameSpec {
    pub json: String,
    pub content_hash: String,
}

pub fn canonicalize_game_spec(spec: &GameSpec) -> Result<CanonicalGameSpec, CanonicalizationError> {
    let normalized = normalized_clone(spec).map_err(CanonicalizationError::new)?;
    let json = serde_json::to_string(&normalized).map_err(CanonicalizationError::new)?;
    let content_hash = Sha256::digest(json.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(CanonicalGameSpec { json, content_hash })
}

fn normalized_clone(spec: &GameSpec) -> Result<GameSpec, serde_json::Error> {
    let mut normalized = spec.clone();

    normalized.intent.audiences.sort();
    normalized.intent.target_platforms.sort();
    normalized.intent.success_metrics.sort();
    normalized.intent.scope.must_have.sort();
    normalized.intent.scope.wont_have.sort();

    for entity in normalized.entities.values_mut() {
        entity.components.sort();
    }

    for action in normalized.actions.values_mut() {
        for condition in &mut action.preconditions {
            normalize_condition(condition)?;
        }
    }

    for machine in normalized.state_machines.values_mut() {
        for transition in &mut machine.transitions {
            for guard in &mut transition.guards {
                normalize_condition(guard)?;
            }
        }
    }

    for resource in normalized.resources.values_mut() {
        resource.source_actions.sort();
        resource.sink_actions.sort();
    }

    for space in normalized.spaces.values_mut() {
        space.connections.sort_by(compare_connections);
    }

    for interaction in normalized.interactions.values_mut() {
        interaction.source_actions.sort();
    }

    for content in normalized.content.values_mut() {
        content.source_refs.sort();
    }

    for presentation in normalized.presentation.values_mut() {
        presentation.source_refs.sort();
    }

    normalized.technical.platforms.sort();
    normalized.technical.save_requirements.sort();
    normalized.technical.accessibility_requirements.sort();

    for scenario in normalized.acceptance_scenarios.values_mut() {
        for condition in &mut scenario.given {
            normalize_condition(condition)?;
        }
        for condition in &mut scenario.then {
            normalize_condition(condition)?;
        }
    }

    Ok(normalized)
}

fn normalize_condition(condition: &mut ConditionSpec) -> Result<(), serde_json::Error> {
    condition.reads.sort();
    normalize_expression(&mut condition.expression)
}

fn normalize_expression(expression: &mut ConditionExpr) -> Result<(), serde_json::Error> {
    match expression {
        ConditionExpr::All { items } | ConditionExpr::Any { items } => {
            for item in items.iter_mut() {
                normalize_expression(item)?;
            }
            let mut keyed = Vec::with_capacity(items.len());
            for item in std::mem::take(items) {
                keyed.push((serde_json::to_string(&item)?, item));
            }
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            *items = keyed.into_iter().map(|(_, item)| item).collect();
        }
        ConditionExpr::Not { item } => normalize_expression(item)?,
        ConditionExpr::Always
        | ConditionExpr::Equals { .. }
        | ConditionExpr::Compare { .. }
        | ConditionExpr::HasTag { .. }
        | ConditionExpr::Extension { .. } => {}
    }
    Ok(())
}

fn compare_connections(left: &RegionConnection, right: &RegionConnection) -> std::cmp::Ordering {
    (&left.from, &left.to, left.bidirectional).cmp(&(&right.from, &right.to, right.bidirectional))
}

#[derive(Debug)]
pub struct CanonicalizationError {
    message: String,
}

impl CanonicalizationError {
    fn new(error: serde_json::Error) -> Self {
        Self {
            message: format!("failed to serialize canonical GameSpec: {error}"),
        }
    }
}

impl fmt::Display for CanonicalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CanonicalizationError {}
