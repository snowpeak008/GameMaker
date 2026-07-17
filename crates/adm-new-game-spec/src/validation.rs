use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{
    ActionSpec, ConditionExpr, ConditionSpec, EffectSpec, GAME_SPEC_SCHEMA_VERSION, GameSpec,
    ProductEnvelope, SpecId, SpecKind, SpecRef, StateMachineSpec, TriggerSource,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecValidationIssue {
    pub code: String,
    pub severity: ValidationSeverity,
    pub path: String,
    #[serde(default)]
    pub related_ids: Vec<SpecId>,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpecValidationReport {
    #[serde(default)]
    pub issues: Vec<SpecValidationIssue>,
}

impl SpecValidationReport {
    pub fn is_valid(&self) -> bool {
        !self
            .issues
            .iter()
            .any(|issue| issue.severity == ValidationSeverity::Error)
    }

    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|issue| issue.severity == ValidationSeverity::Error)
            .count()
    }

    pub fn contains_code(&self, code: &str) -> bool {
        self.issues.iter().any(|issue| issue.code == code)
    }
}

pub fn validate_game_spec(spec: &GameSpec) -> SpecValidationReport {
    let mut report = SpecValidationReport::default();
    let index = SpecIndex::new(spec, &mut report);

    validate_identity(spec, &mut report);
    validate_entities(spec, &index, &mut report);
    validate_relationships(spec, &index, &mut report);
    validate_actions(spec, &index, &mut report);
    validate_state_machines(spec, &index, &mut report);
    validate_resources(spec, &mut report);
    validate_spaces(spec, &mut report);
    validate_interactions(spec, &mut report);
    validate_content_and_presentation(spec, &index, &mut report);
    validate_acceptance_scenarios(spec, &index, &mut report);
    validate_trace_links(spec, &index, &mut report);

    report
}

pub fn validate_game_spec_for_envelope(
    spec: &GameSpec,
    supported: &ProductEnvelope,
) -> SpecValidationReport {
    let mut report = validate_game_spec(spec);
    for violation in spec
        .technical
        .product_envelope
        .violations_against(supported)
    {
        push_issue(
            &mut report,
            "SPEC_ENVELOPE_EXCEEDED",
            format!(
                "/technical/productEnvelope/{}",
                violation.dimension.json_field()
            ),
            vec![spec.identity.project_id.clone()],
            format!(
                "requested {:?} exceeds supported {:?}",
                violation.required, violation.supported
            ),
            "Reduce the requested production scale or select a larger supported envelope.",
        );
    }
    report
}

fn validate_identity(spec: &GameSpec, report: &mut SpecValidationReport) {
    if spec.identity.schema_version != GAME_SPEC_SCHEMA_VERSION {
        push_issue(
            report,
            "SPEC_SCHEMA_VERSION_UNSUPPORTED",
            "/identity/schemaVersion",
            vec![spec.identity.project_id.clone()],
            format!(
                "schema version {:?} is not supported by this compiler",
                spec.identity.schema_version
            ),
            "Migrate the document to the active GameSpec schema version.",
        );
    }
    if spec.identity.revision == 0 {
        push_issue(
            report,
            "SPEC_REVISION_INVALID",
            "/identity/revision",
            vec![spec.identity.project_id.clone()],
            "revision must be greater than zero",
            "Start a new specification at revision 1.",
        );
    }
    if spec.identity.revision > 1 && spec.identity.parent_hash.is_none() {
        push_issue(
            report,
            "SPEC_PARENT_HASH_REQUIRED",
            "/identity/parentHash",
            vec![spec.identity.project_id.clone()],
            "a revision after the first must identify its parent content hash",
            "Set parentHash to the canonical hash of the preceding revision.",
        );
    }
    if let Some(parent_hash) = &spec.identity.parent_hash
        && (parent_hash.len() != 64
            || !parent_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
    {
        push_issue(
            report,
            "SPEC_PARENT_HASH_INVALID",
            "/identity/parentHash",
            vec![spec.identity.project_id.clone()],
            "parentHash must be a lowercase 64-character SHA-256 digest",
            "Use the canonical content hash from the parent revision.",
        );
    }
}

fn validate_entities(spec: &GameSpec, index: &SpecIndex, report: &mut SpecValidationReport) {
    for (entity_id, entity) in &spec.entities {
        for (position, component_id) in entity.components.iter().enumerate() {
            if !index.components.contains(component_id) {
                push_issue(
                    report,
                    "SPEC_COMPONENT_REFERENCE_MISSING",
                    format!("/entities/{entity_id}/components/{position}"),
                    vec![entity_id.clone(), component_id.clone()],
                    format!("entity references unknown component {component_id}"),
                    "Declare the component or remove it from the entity.",
                );
            }
        }
    }
}

fn validate_relationships(spec: &GameSpec, index: &SpecIndex, report: &mut SpecValidationReport) {
    for (relationship_id, relationship) in &spec.relationships {
        validate_ref(
            spec,
            index,
            &relationship.source,
            format!("/relationships/{relationship_id}/source"),
            report,
        );
        validate_ref(
            spec,
            index,
            &relationship.target,
            format!("/relationships/{relationship_id}/target"),
            report,
        );
    }
}

fn validate_actions(spec: &GameSpec, index: &SpecIndex, report: &mut SpecValidationReport) {
    for (action_id, action) in &spec.actions {
        let base = format!("/actions/{action_id}");
        if action.effects.is_empty() {
            push_issue(
                report,
                "SPEC_ACTION_EFFECTS_EMPTY",
                format!("{base}/effects"),
                vec![action_id.clone()],
                "an action must declare at least one observable effect",
                "Add an effect or remove the incomplete action.",
            );
        }
        validate_action_refs(spec, index, action_id, action, &base, report);
    }
}

fn validate_action_refs(
    spec: &GameSpec,
    index: &SpecIndex,
    action_id: &SpecId,
    action: &ActionSpec,
    base: &str,
    report: &mut SpecValidationReport,
) {
    for (position, reference) in action.actors.iter().enumerate() {
        validate_ref(
            spec,
            index,
            reference,
            format!("{base}/actors/{position}"),
            report,
        );
    }
    for (position, reference) in action.targets.iter().enumerate() {
        validate_ref(
            spec,
            index,
            reference,
            format!("{base}/targets/{position}"),
            report,
        );
    }
    for (position, condition) in action.preconditions.iter().enumerate() {
        validate_condition(
            spec,
            index,
            condition,
            &format!("{base}/preconditions/{position}"),
            report,
        );
    }
    for (position, effect) in action.effects.iter().enumerate() {
        validate_effect(
            spec,
            index,
            effect,
            &format!("{base}/effects/{position}"),
            Some(action_id),
            report,
        );
    }
}

fn validate_state_machines(spec: &GameSpec, index: &SpecIndex, report: &mut SpecValidationReport) {
    for (machine_id, machine) in &spec.state_machines {
        let base = format!("/stateMachines/{machine_id}");
        if !machine.states.contains_key(&machine.initial_state) {
            push_issue(
                report,
                "SPEC_STATE_INITIAL_MISSING",
                format!("{base}/initialState"),
                vec![machine_id.clone(), machine.initial_state.clone()],
                format!(
                    "initial state {} does not exist in state machine {machine_id}",
                    machine.initial_state
                ),
                "Select a declared state as the initial state.",
            );
        }

        let mut transition_ids = BTreeSet::new();
        for (position, transition) in machine.transitions.iter().enumerate() {
            let path = format!("{base}/transitions/{position}");
            if !transition_ids.insert(transition.transition_id.clone()) {
                push_issue(
                    report,
                    "SPEC_TRANSITION_ID_DUPLICATE",
                    format!("{path}/transitionId"),
                    vec![machine_id.clone(), transition.transition_id.clone()],
                    format!(
                        "transition ID {} is duplicated in state machine {machine_id}",
                        transition.transition_id
                    ),
                    "Give every transition in the state machine a unique ID.",
                );
            }
            validate_state_endpoint(
                machine_id,
                machine,
                &transition.from,
                format!("{path}/from"),
                report,
            );
            validate_state_endpoint(
                machine_id,
                machine,
                &transition.to,
                format!("{path}/to"),
                report,
            );
            if transition.trigger.source == TriggerSource::Action {
                match &transition.trigger.reference {
                    Some(reference) if reference.kind == SpecKind::Action => validate_ref(
                        spec,
                        index,
                        reference,
                        format!("{path}/trigger/reference"),
                        report,
                    ),
                    Some(reference) => push_issue(
                        report,
                        "SPEC_TRIGGER_REFERENCE_KIND_INVALID",
                        format!("{path}/trigger/reference"),
                        vec![machine_id.clone(), reference.id.clone()],
                        "an action trigger must reference an action",
                        "Change the reference kind to action or choose another trigger source.",
                    ),
                    None => push_issue(
                        report,
                        "SPEC_TRIGGER_REFERENCE_REQUIRED",
                        format!("{path}/trigger/reference"),
                        vec![machine_id.clone()],
                        "an action trigger must identify its source action",
                        "Add the action reference that causes this transition.",
                    ),
                }
            } else if let Some(reference) = &transition.trigger.reference {
                validate_ref(
                    spec,
                    index,
                    reference,
                    format!("{path}/trigger/reference"),
                    report,
                );
            }
            for (guard_position, guard) in transition.guards.iter().enumerate() {
                validate_condition(
                    spec,
                    index,
                    guard,
                    &format!("{path}/guards/{guard_position}"),
                    report,
                );
            }
            for (effect_position, effect) in transition.effects.iter().enumerate() {
                validate_effect(
                    spec,
                    index,
                    effect,
                    &format!("{path}/effects/{effect_position}"),
                    None,
                    report,
                );
            }
        }
        validate_state_reachability(machine_id, machine, report);
    }
}

fn validate_state_endpoint(
    machine_id: &SpecId,
    machine: &StateMachineSpec,
    state_id: &SpecId,
    path: String,
    report: &mut SpecValidationReport,
) {
    if !machine.states.contains_key(state_id) {
        push_issue(
            report,
            "SPEC_STATE_REFERENCE_MISSING",
            path,
            vec![machine_id.clone(), state_id.clone()],
            format!("state machine {machine_id} references unknown state {state_id}"),
            "Declare the state or correct the transition endpoint.",
        );
    }
}

fn validate_state_reachability(
    machine_id: &SpecId,
    machine: &StateMachineSpec,
    report: &mut SpecValidationReport,
) {
    if !machine.states.contains_key(&machine.initial_state) {
        return;
    }
    let mut reached = BTreeSet::from([machine.initial_state.clone()]);
    let mut queue = VecDeque::from([machine.initial_state.clone()]);
    while let Some(state) = queue.pop_front() {
        for target in machine
            .transitions
            .iter()
            .filter(|transition| transition.from == state)
            .map(|transition| &transition.to)
        {
            if machine.states.contains_key(target) && reached.insert(target.clone()) {
                queue.push_back(target.clone());
            }
        }
    }
    for state_id in machine.states.keys().filter(|id| !reached.contains(*id)) {
        push_issue(
            report,
            "SPEC_STATE_UNREACHABLE",
            format!("/stateMachines/{machine_id}/states/{state_id}"),
            vec![machine_id.clone(), state_id.clone()],
            format!("state {state_id} cannot be reached from the initial state"),
            "Add a valid transition path or remove the unreachable state.",
        );
    }
}

fn validate_resources(spec: &GameSpec, report: &mut SpecValidationReport) {
    for (resource_id, resource) in &spec.resources {
        let base = format!("/resources/{resource_id}");
        if let (Some(minimum), Some(maximum)) = (resource.minimum, resource.maximum)
            && minimum > maximum
        {
            push_issue(
                report,
                "SPEC_RESOURCE_RANGE_INVALID",
                base.clone(),
                vec![resource_id.clone()],
                "resource minimum exceeds its maximum",
                "Make minimum less than or equal to maximum.",
            );
        }
        if resource
            .minimum
            .is_some_and(|minimum| resource.initial < minimum)
            || resource
                .maximum
                .is_some_and(|maximum| resource.initial > maximum)
        {
            push_issue(
                report,
                "SPEC_RESOURCE_INITIAL_OUT_OF_RANGE",
                format!("{base}/initial"),
                vec![resource_id.clone()],
                "resource initial value is outside its declared range",
                "Move the initial value inside the minimum and maximum bounds.",
            );
        }
        validate_resource_action_list(
            spec,
            resource_id,
            &resource.source_actions,
            true,
            &base,
            report,
        );
        validate_resource_action_list(
            spec,
            resource_id,
            &resource.sink_actions,
            false,
            &base,
            report,
        );
    }
}

fn validate_resource_action_list(
    spec: &GameSpec,
    resource_id: &SpecId,
    action_ids: &[SpecId],
    source: bool,
    base: &str,
    report: &mut SpecValidationReport,
) {
    let field = if source {
        "sourceActions"
    } else {
        "sinkActions"
    };
    for (position, action_id) in action_ids.iter().enumerate() {
        let Some(action) = spec.actions.get(action_id) else {
            push_issue(
                report,
                "SPEC_RESOURCE_ACTION_MISSING",
                format!("{base}/{field}/{position}"),
                vec![resource_id.clone(), action_id.clone()],
                format!("resource references unknown action {action_id}"),
                "Declare the action or remove it from the resource flow.",
            );
            continue;
        };
        let matching_flow = action.effects.iter().any(|effect| match effect {
            EffectSpec::ChangeResource { resource, amount } => {
                resource == resource_id && if source { *amount > 0 } else { *amount < 0 }
            }
            _ => false,
        });
        if !matching_flow {
            push_issue(
                report,
                "SPEC_RESOURCE_FLOW_MISMATCH",
                format!("{base}/{field}/{position}"),
                vec![resource_id.clone(), action_id.clone()],
                format!(
                    "action {action_id} does not contain the declared {} flow for resource {resource_id}",
                    if source { "positive" } else { "negative" }
                ),
                "Align the action effect amount with the resource source/sink declaration.",
            );
        }
    }
}

fn validate_spaces(spec: &GameSpec, report: &mut SpecValidationReport) {
    for (space_id, space) in &spec.spaces {
        for (position, connection) in space.connections.iter().enumerate() {
            for (field, region_id) in [("from", &connection.from), ("to", &connection.to)] {
                if !space.regions.contains_key(region_id) {
                    push_issue(
                        report,
                        "SPEC_REGION_REFERENCE_MISSING",
                        format!("/spaces/{space_id}/connections/{position}/{field}"),
                        vec![space_id.clone(), region_id.clone()],
                        format!("space connection references unknown region {region_id}"),
                        "Declare the region or correct the connection endpoint.",
                    );
                }
            }
        }
    }
}

fn validate_interactions(spec: &GameSpec, report: &mut SpecValidationReport) {
    for (interaction_id, interaction) in &spec.interactions {
        for (position, action_id) in interaction.source_actions.iter().enumerate() {
            if !spec.actions.contains_key(action_id) {
                push_issue(
                    report,
                    "SPEC_INTERACTION_ACTION_MISSING",
                    format!("/interactions/{interaction_id}/sourceActions/{position}"),
                    vec![interaction_id.clone(), action_id.clone()],
                    format!("interaction references unknown action {action_id}"),
                    "Declare the action or remove it from the interaction.",
                );
            }
        }
    }
}

fn validate_content_and_presentation(
    spec: &GameSpec,
    index: &SpecIndex,
    report: &mut SpecValidationReport,
) {
    for (content_id, content) in &spec.content {
        for (position, reference) in content.source_refs.iter().enumerate() {
            validate_ref(
                spec,
                index,
                reference,
                format!("/content/{content_id}/sourceRefs/{position}"),
                report,
            );
        }
    }
    for (presentation_id, presentation) in &spec.presentation {
        for (position, reference) in presentation.source_refs.iter().enumerate() {
            validate_ref(
                spec,
                index,
                reference,
                format!("/presentation/{presentation_id}/sourceRefs/{position}"),
                report,
            );
        }
    }
}

fn validate_acceptance_scenarios(
    spec: &GameSpec,
    index: &SpecIndex,
    report: &mut SpecValidationReport,
) {
    for (scenario_id, scenario) in &spec.acceptance_scenarios {
        let base = format!("/acceptanceScenarios/{scenario_id}");
        if scenario.then.is_empty() {
            push_issue(
                report,
                "SPEC_SCENARIO_OUTCOME_EMPTY",
                format!("{base}/then"),
                vec![scenario_id.clone()],
                "an acceptance scenario must declare at least one expected outcome",
                "Add a machine-executable or manually reviewable expected condition.",
            );
        }
        for (position, condition) in scenario.given.iter().enumerate() {
            validate_condition(
                spec,
                index,
                condition,
                &format!("{base}/given/{position}"),
                report,
            );
        }
        for (position, invocation) in scenario.when.iter().enumerate() {
            if !spec.actions.contains_key(&invocation.action) {
                push_issue(
                    report,
                    "SPEC_SCENARIO_ACTION_MISSING",
                    format!("{base}/when/{position}/action"),
                    vec![scenario_id.clone(), invocation.action.clone()],
                    format!("scenario invokes unknown action {}", invocation.action),
                    "Declare the action or correct the scenario invocation.",
                );
            }
            if let Some(actor) = &invocation.actor {
                validate_ref(
                    spec,
                    index,
                    actor,
                    format!("{base}/when/{position}/actor"),
                    report,
                );
            }
            for (target_position, target) in invocation.targets.iter().enumerate() {
                validate_ref(
                    spec,
                    index,
                    target,
                    format!("{base}/when/{position}/targets/{target_position}"),
                    report,
                );
            }
        }
        for (position, condition) in scenario.then.iter().enumerate() {
            validate_condition(
                spec,
                index,
                condition,
                &format!("{base}/then/{position}"),
                report,
            );
        }
    }
}

fn validate_trace_links(spec: &GameSpec, index: &SpecIndex, report: &mut SpecValidationReport) {
    let mut targets_by_promise: BTreeMap<SpecId, Vec<SpecRef>> = BTreeMap::new();
    for (trace_id, trace) in &spec.trace_links {
        validate_ref(
            spec,
            index,
            &trace.source,
            format!("/traceLinks/{trace_id}/source"),
            report,
        );
        validate_ref(
            spec,
            index,
            &trace.target,
            format!("/traceLinks/{trace_id}/target"),
            report,
        );
        if trace.rationale.trim().is_empty() {
            push_issue(
                report,
                "SPEC_TRACE_RATIONALE_EMPTY",
                format!("/traceLinks/{trace_id}/rationale"),
                vec![trace_id.clone()],
                "a trace link must explain why the relationship exists",
                "Add a concise, evidence-based rationale.",
            );
        }
        if trace.source.kind == SpecKind::Intent {
            targets_by_promise
                .entry(trace.source.id.clone())
                .or_default()
                .push(trace.target.clone());
        }
    }

    for promise_id in spec.intent.experience_promises.keys() {
        let targets = targets_by_promise
            .get(promise_id)
            .cloned()
            .unwrap_or_default();
        if targets.is_empty() {
            push_issue(
                report,
                "SPEC_INTENT_TRACE_MISSING",
                format!("/intent/experiencePromises/{promise_id}"),
                vec![promise_id.clone()],
                "experience promise has no outgoing trace link",
                "Trace the promise to an action or acceptance scenario that demonstrates it.",
            );
            continue;
        }
        let verified = targets.iter().any(|target| match target.kind {
            SpecKind::Scenario => spec.acceptance_scenarios.contains_key(&target.id),
            SpecKind::Action => spec.acceptance_scenarios.values().any(|scenario| {
                scenario
                    .when
                    .iter()
                    .any(|invocation| invocation.action == target.id)
            }),
            _ => false,
        });
        if !verified {
            push_issue(
                report,
                "SPEC_INTENT_SCENARIO_MISSING",
                format!("/intent/experiencePromises/{promise_id}"),
                vec![promise_id.clone()],
                "experience promise does not reach an executable acceptance scenario",
                "Trace the promise to a scenario or to an action invoked by a scenario.",
            );
        }
    }
}

fn validate_condition(
    spec: &GameSpec,
    index: &SpecIndex,
    condition: &ConditionSpec,
    base: &str,
    report: &mut SpecValidationReport,
) {
    for (position, reference) in condition.reads.iter().enumerate() {
        validate_ref(
            spec,
            index,
            reference,
            format!("{base}/reads/{position}"),
            report,
        );
    }
    validate_condition_expression(
        spec,
        index,
        &condition.expression,
        &format!("{base}/expression"),
        report,
    );
}

fn validate_condition_expression(
    spec: &GameSpec,
    index: &SpecIndex,
    expression: &ConditionExpr,
    base: &str,
    report: &mut SpecValidationReport,
) {
    match expression {
        ConditionExpr::Always => {}
        ConditionExpr::All { items } | ConditionExpr::Any { items } => {
            if items.is_empty() {
                push_issue(
                    report,
                    "SPEC_CONDITION_GROUP_EMPTY",
                    format!("{base}/items"),
                    Vec::new(),
                    "a logical condition group cannot be empty",
                    "Add a condition or replace the group with an explicit always condition.",
                );
            }
            for (position, item) in items.iter().enumerate() {
                validate_condition_expression(
                    spec,
                    index,
                    item,
                    &format!("{base}/items/{position}"),
                    report,
                );
            }
        }
        ConditionExpr::Not { item } => {
            validate_condition_expression(spec, index, item, &format!("{base}/item"), report)
        }
        ConditionExpr::Equals { source, .. } | ConditionExpr::Compare { source, .. } => {
            validate_ref(spec, index, source, format!("{base}/source"), report)
        }
        ConditionExpr::HasTag { entity, tag } => match spec.entities.get(entity) {
            Some(entity_spec) if entity_spec.tags.contains(tag) => {}
            Some(_) => push_issue(
                report,
                "SPEC_ENTITY_TAG_MISSING",
                format!("{base}/tag"),
                vec![entity.clone()],
                format!("entity {entity} does not declare tag {tag:?}"),
                "Declare the tag on the entity or correct the condition.",
            ),
            None => push_issue(
                report,
                "SPEC_ENTITY_REFERENCE_MISSING",
                format!("{base}/entity"),
                vec![entity.clone()],
                format!("condition references unknown entity {entity}"),
                "Declare the entity or correct the condition.",
            ),
        },
        ConditionExpr::Extension { namespace, .. } => {
            validate_extension_namespace(spec, namespace, format!("{base}/namespace"), report)
        }
    }
}

fn validate_effect(
    spec: &GameSpec,
    index: &SpecIndex,
    effect: &EffectSpec,
    base: &str,
    owner_action: Option<&SpecId>,
    report: &mut SpecValidationReport,
) {
    match effect {
        EffectSpec::SetValue { target, .. } => {
            validate_ref(spec, index, target, format!("{base}/target"), report)
        }
        EffectSpec::ChangeResource { resource, .. } => {
            if !spec.resources.contains_key(resource) {
                push_issue(
                    report,
                    "SPEC_RESOURCE_REFERENCE_MISSING",
                    format!("{base}/resource"),
                    related(owner_action, resource),
                    format!("effect references unknown resource {resource}"),
                    "Declare the resource or correct the effect.",
                );
            }
        }
        EffectSpec::TransitionState {
            state_machine,
            target_state,
        } => match spec.state_machines.get(state_machine) {
            Some(machine) if machine.states.contains_key(target_state) => {}
            Some(_) => push_issue(
                report,
                "SPEC_STATE_REFERENCE_MISSING",
                format!("{base}/targetState"),
                vec![state_machine.clone(), target_state.clone()],
                format!("effect targets unknown state {target_state}"),
                "Choose a state declared by the referenced state machine.",
            ),
            None => push_issue(
                report,
                "SPEC_STATE_MACHINE_REFERENCE_MISSING",
                format!("{base}/stateMachine"),
                vec![state_machine.clone(), target_state.clone()],
                format!("effect references unknown state machine {state_machine}"),
                "Declare the state machine or correct the effect.",
            ),
        },
        EffectSpec::CreateEntity { entity, quantity }
        | EffectSpec::RemoveEntity { entity, quantity } => {
            if !spec.entities.contains_key(entity) {
                push_issue(
                    report,
                    "SPEC_ENTITY_REFERENCE_MISSING",
                    format!("{base}/entity"),
                    related(owner_action, entity),
                    format!("effect references unknown entity {entity}"),
                    "Declare the entity or correct the effect.",
                );
            }
            if *quantity == 0 {
                push_issue(
                    report,
                    "SPEC_ENTITY_EFFECT_QUANTITY_INVALID",
                    format!("{base}/quantity"),
                    related(owner_action, entity),
                    "entity create/remove quantity must be greater than zero",
                    "Use a positive quantity or remove the no-op effect.",
                );
            }
        }
        EffectSpec::EmitEvent { .. } => {}
        EffectSpec::Extension { namespace, .. } => {
            validate_extension_namespace(spec, namespace, format!("{base}/namespace"), report)
        }
    }
}

fn validate_extension_namespace(
    spec: &GameSpec,
    namespace: &SpecId,
    path: String,
    report: &mut SpecValidationReport,
) {
    if !spec
        .extensions
        .values()
        .any(|extension| &extension.namespace == namespace)
    {
        push_issue(
            report,
            "SPEC_EXTENSION_NAMESPACE_MISSING",
            path,
            vec![namespace.clone()],
            format!("extension namespace {namespace} is not declared"),
            "Declare a versioned ExtensionBlock for this namespace.",
        );
    }
}

fn validate_ref(
    spec: &GameSpec,
    index: &SpecIndex,
    reference: &SpecRef,
    path: String,
    report: &mut SpecValidationReport,
) {
    if !index.contains(reference.kind, &reference.id) {
        push_issue(
            report,
            "SPEC_REFERENCE_MISSING",
            path.clone(),
            vec![reference.id.clone()],
            format!(
                "reference {:?}/{} does not resolve",
                reference.kind, reference.id
            ),
            "Declare the referenced object or correct its kind and ID.",
        );
        return;
    }

    let Some(member_path) = reference.path.as_deref() else {
        return;
    };
    if member_path.trim().is_empty() {
        push_issue(
            report,
            "SPEC_REFERENCE_PATH_EMPTY",
            format!("{path}/path"),
            vec![reference.id.clone()],
            "reference path cannot be empty",
            "Remove the path or identify a concrete member.",
        );
        return;
    }
    let member_exists = match reference.kind {
        SpecKind::Component => spec.components.get(&reference.id).is_some_and(|component| {
            component
                .properties
                .keys()
                .any(|property| property.as_str() == member_path)
        }),
        SpecKind::StateMachine => spec
            .state_machines
            .get(&reference.id)
            .is_some_and(|machine| {
                machine
                    .states
                    .keys()
                    .any(|state| state.as_str() == member_path)
            }),
        SpecKind::Time => spec
            .time
            .phases
            .keys()
            .any(|phase| phase.as_str() == member_path),
        _ => true,
    };
    if !member_exists {
        push_issue(
            report,
            "SPEC_REFERENCE_PATH_MISSING",
            format!("{path}/path"),
            vec![reference.id.clone()],
            format!("reference member path {member_path:?} does not resolve"),
            "Correct the member path or declare the referenced member.",
        );
    }
}

fn related(owner: Option<&SpecId>, target: &SpecId) -> Vec<SpecId> {
    owner
        .into_iter()
        .cloned()
        .chain(std::iter::once(target.clone()))
        .collect()
}

fn push_issue(
    report: &mut SpecValidationReport,
    code: &str,
    path: impl Into<String>,
    related_ids: Vec<SpecId>,
    message: impl Into<String>,
    suggestion: impl Into<String>,
) {
    report.issues.push(SpecValidationIssue {
        code: code.to_string(),
        severity: ValidationSeverity::Error,
        path: path.into(),
        related_ids,
        message: message.into(),
        suggestion: suggestion.into(),
    });
}

struct SpecIndex {
    intents: BTreeSet<SpecId>,
    capabilities: BTreeSet<&'static str>,
    entities: BTreeSet<SpecId>,
    components: BTreeSet<SpecId>,
    relationships: BTreeSet<SpecId>,
    actions: BTreeSet<SpecId>,
    state_machines: BTreeSet<SpecId>,
    states: BTreeSet<SpecId>,
    resources: BTreeSet<SpecId>,
    spaces: BTreeSet<SpecId>,
    time: BTreeSet<SpecId>,
    interactions: BTreeSet<SpecId>,
    content: BTreeSet<SpecId>,
    presentation: BTreeSet<SpecId>,
    technical: BTreeSet<SpecId>,
    scenarios: BTreeSet<SpecId>,
    extensions: BTreeSet<SpecId>,
}

impl SpecIndex {
    fn new(spec: &GameSpec, report: &mut SpecValidationReport) -> Self {
        let mut states = BTreeSet::new();
        for (machine_id, machine) in &spec.state_machines {
            for state_id in machine.states.keys() {
                if !states.insert(state_id.clone()) {
                    push_issue(
                        report,
                        "SPEC_STATE_ID_AMBIGUOUS",
                        format!("/stateMachines/{machine_id}/states/{state_id}"),
                        vec![machine_id.clone(), state_id.clone()],
                        "state IDs must be globally unique because typed state references do not carry a parent machine",
                        "Rename the state so it is unique across the specification.",
                    );
                }
            }
        }

        let mut extension_namespaces = BTreeSet::new();
        for (extension_id, extension) in &spec.extensions {
            if !extension_namespaces.insert(extension.namespace.clone()) {
                push_issue(
                    report,
                    "SPEC_EXTENSION_NAMESPACE_DUPLICATE",
                    format!("/extensions/{extension_id}/namespace"),
                    vec![extension.namespace.clone()],
                    "extension namespaces must be unique",
                    "Merge the extension blocks or use a distinct versioned namespace.",
                );
            }
        }

        Self {
            intents: spec.intent.experience_promises.keys().cloned().collect(),
            capabilities: BTreeSet::from([
                "space",
                "time",
                "control",
                "participants",
                "information",
                "progression",
                "content",
                "connectivity",
            ]),
            entities: spec.entities.keys().cloned().collect(),
            components: spec.components.keys().cloned().collect(),
            relationships: spec.relationships.keys().cloned().collect(),
            actions: spec.actions.keys().cloned().collect(),
            state_machines: spec.state_machines.keys().cloned().collect(),
            states,
            resources: spec.resources.keys().cloned().collect(),
            spaces: spec.spaces.keys().cloned().collect(),
            time: std::iter::once(SpecId::new("time").expect("static time ID"))
                .chain(spec.time.phases.keys().cloned())
                .collect(),
            interactions: spec.interactions.keys().cloned().collect(),
            content: spec.content.keys().cloned().collect(),
            presentation: spec.presentation.keys().cloned().collect(),
            technical: BTreeSet::from([SpecId::new("technical").expect("static technical ID")]),
            scenarios: spec.acceptance_scenarios.keys().cloned().collect(),
            extensions: extension_namespaces,
        }
    }

    fn contains(&self, kind: SpecKind, id: &SpecId) -> bool {
        match kind {
            SpecKind::Intent => self.intents.contains(id),
            SpecKind::Capability => self.capabilities.contains(id.as_str()),
            SpecKind::Entity => self.entities.contains(id),
            SpecKind::Component => self.components.contains(id),
            SpecKind::Relationship => self.relationships.contains(id),
            SpecKind::Action => self.actions.contains(id),
            SpecKind::StateMachine => self.state_machines.contains(id),
            SpecKind::State => self.states.contains(id),
            SpecKind::Resource => self.resources.contains(id),
            SpecKind::Space => self.spaces.contains(id),
            SpecKind::Time => self.time.contains(id),
            SpecKind::Interaction => self.interactions.contains(id),
            SpecKind::Content => self.content.contains(id),
            SpecKind::Presentation => self.presentation.contains(id),
            SpecKind::TechnicalConstraint => self.technical.contains(id),
            SpecKind::Scenario => self.scenarios.contains(id),
            SpecKind::Extension => self.extensions.contains(id),
        }
    }
}
