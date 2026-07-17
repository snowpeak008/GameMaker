use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use adm_new_contracts::project::ProjectState;
use adm_new_game_spec::GameSpec;
use serde::{Deserialize, Serialize};

use crate::data_loader::{DomainDocument, DomainNode};

mod policy;

pub const DECISION_GRAPH_SCHEMA_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintKey {
    pub node_id: String,
    pub checklist_id: String,
}

impl ConstraintKey {
    pub fn new(node_id: impl Into<String>, checklist_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            checklist_id: checklist_id.into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionCoverage {
    #[serde(default)]
    pub satisfied_constraints: BTreeSet<ConstraintKey>,
}

impl DecisionCoverage {
    pub fn from_project_state(project_state: &ProjectState) -> Self {
        let satisfied_constraints = project_state
            .nodes
            .iter()
            .flat_map(|(node_id, state)| {
                state
                    .checklist
                    .iter()
                    .filter_map(move |(checklist_id, done)| {
                        done.then(|| ConstraintKey::new(node_id, checklist_id))
                    })
            })
            .collect();
        Self {
            satisfied_constraints,
        }
    }

    pub fn satisfy(&mut self, node_id: impl Into<String>, checklist_id: impl Into<String>) {
        self.satisfied_constraints
            .insert(ConstraintKey::new(node_id, checklist_id));
    }

    pub fn is_satisfied(&self, node_id: &str, checklist_id: &str) -> bool {
        self.satisfied_constraints
            .contains(&ConstraintKey::new(node_id, checklist_id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivationReason {
    pub predicate_id: String,
    pub source_path: String,
    pub operator: String,
    pub expected: Vec<String>,
    pub actual: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnresolvedConstraint {
    pub checklist_id: String,
    pub label: String,
    pub output_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActiveDecisionNode {
    pub order: usize,
    pub node_id: String,
    pub domain_id: String,
    pub name: String,
    pub activation_reasons: Vec<ActivationReason>,
    pub unresolved_constraints: Vec<UnresolvedConstraint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionEdgeKind {
    Requires,
    RequiresAny,
    RecommendedBefore,
    ConflictsWith,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionEdge {
    pub from_node: String,
    pub to_node: String,
    pub kind: DecisionEdgeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionGraphCoverage {
    pub knowledge_domain_count: usize,
    pub knowledge_node_count: usize,
    pub knowledge_checklist_count: usize,
    pub relevant_node_count: usize,
    pub active_node_count: usize,
    pub resolved_node_count: usize,
    pub irrelevant_node_count: usize,
    pub relevant_checklist_count: usize,
    pub satisfied_checklist_count: usize,
    pub unresolved_checklist_count: usize,
    pub completion_percent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityDecisionGraph {
    pub schema_version: String,
    pub coverage: DecisionGraphCoverage,
    pub active_nodes: Vec<ActiveDecisionNode>,
    pub edges: Vec<DecisionEdge>,
}

impl CapabilityDecisionGraph {
    pub fn activation_set(&self) -> BTreeSet<String> {
        self.active_nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect()
    }

    pub fn semantics(&self) -> DecisionGraphSemantics {
        DecisionGraphSemantics {
            active_constraints: self
                .active_nodes
                .iter()
                .flat_map(|node| {
                    node.unresolved_constraints.iter().map(move |constraint| {
                        ConstraintKey::new(&node.node_id, &constraint.checklist_id)
                    })
                })
                .collect(),
            edges: self.edges.clone(),
        }
    }

    pub fn validate_activation_evidence(&self) -> Result<(), DecisionGraphError> {
        for node in &self.active_nodes {
            if node.activation_reasons.is_empty() {
                return Err(DecisionGraphError::new(
                    "decision_graph.missing_activation_reason",
                    format!("/activeNodes/{}/activationReasons", node.node_id),
                    format!("active node '{}' has no activation reason", node.node_id),
                    "attach at least one matched capability predicate",
                ));
            }
            for reason in &node.activation_reasons {
                if !reason.source_path.starts_with("/capabilities/")
                    || reason.predicate_id.is_empty()
                    || reason.operator.is_empty()
                    || reason.expected.is_empty()
                    || reason.actual.is_empty()
                {
                    return Err(DecisionGraphError::new(
                        "decision_graph.incomplete_activation_reason",
                        format!("/activeNodes/{}/activationReasons", node.node_id),
                        format!(
                            "active node '{}' contains incomplete capability evidence",
                            node.node_id
                        ),
                        "record predicate id, capability JSON Pointer, operator, expected, and actual values",
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionGraphSemantics {
    pub active_constraints: BTreeSet<ConstraintKey>,
    pub edges: Vec<DecisionEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionGraphError {
    pub code: String,
    pub path: String,
    pub message: String,
    pub suggestion: String,
}

impl DecisionGraphError {
    pub(crate) fn new(
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            path: path.into(),
            message: message.into(),
            suggestion: suggestion.into(),
        }
    }
}

impl fmt::Display for DecisionGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} at {}: {}",
            self.code, self.path, self.message
        )
    }
}

impl std::error::Error for DecisionGraphError {}

#[derive(Debug, Clone, Copy, Default)]
pub struct CapabilityDecisionGraphCompiler;

impl CapabilityDecisionGraphCompiler {
    pub fn compile(
        &self,
        spec: &GameSpec,
        domains: &[DomainDocument],
        coverage: &DecisionCoverage,
    ) -> Result<CapabilityDecisionGraph, DecisionGraphError> {
        let inventory = KnowledgeInventory::build(domains, coverage)?;
        inventory.validate_policy_coverage()?;

        let mut relevant_nodes = BTreeSet::new();
        let mut active_nodes = BTreeMap::new();
        let mut relevant_checklist_count = 0usize;
        let mut satisfied_checklist_count = 0usize;

        for (node_id, indexed) in &inventory.nodes {
            let rule = policy::effective_rule(&indexed.domain_id, node_id)?;
            let Some(activation_reasons) = rule.evaluate(&spec.capabilities) else {
                continue;
            };

            relevant_nodes.insert(node_id.clone());
            relevant_checklist_count += indexed.node.checklist.len();
            let unresolved_constraints = indexed
                .node
                .checklist
                .iter()
                .filter_map(|item| {
                    if coverage.is_satisfied(node_id, &item.id) {
                        satisfied_checklist_count += 1;
                        None
                    } else {
                        Some(UnresolvedConstraint {
                            checklist_id: item.id.clone(),
                            label: item.label.clone(),
                            output_key: item.output_key.clone(),
                        })
                    }
                })
                .collect::<Vec<_>>();

            if !unresolved_constraints.is_empty() {
                active_nodes.insert(
                    node_id.clone(),
                    ActiveDecisionNode {
                        order: 0,
                        node_id: node_id.clone(),
                        domain_id: indexed.domain_id.clone(),
                        name: indexed.node.name.clone(),
                        activation_reasons,
                        unresolved_constraints,
                    },
                );
            }
        }

        let edges = build_edges(&inventory.nodes, &active_nodes);
        let order = topological_order(&active_nodes, &edges)?;
        for (node_id, index) in order.iter().enumerate() {
            if let Some(node) = active_nodes.get_mut(index) {
                node.order = node_id;
            }
        }
        let mut active_nodes = active_nodes.into_values().collect::<Vec<_>>();
        active_nodes.sort_by_key(|node| node.order);

        let relevant_node_count = relevant_nodes.len();
        let active_node_count = active_nodes.len();
        let unresolved_checklist_count =
            relevant_checklist_count.saturating_sub(satisfied_checklist_count);
        let completion_percent = if relevant_checklist_count == 0 {
            100
        } else {
            ((satisfied_checklist_count * 100) / relevant_checklist_count) as u32
        };
        let graph = CapabilityDecisionGraph {
            schema_version: DECISION_GRAPH_SCHEMA_VERSION.to_string(),
            coverage: DecisionGraphCoverage {
                knowledge_domain_count: inventory.domain_count,
                knowledge_node_count: inventory.nodes.len(),
                knowledge_checklist_count: inventory.known_constraints.len(),
                relevant_node_count,
                active_node_count,
                resolved_node_count: relevant_node_count.saturating_sub(active_node_count),
                irrelevant_node_count: inventory.nodes.len().saturating_sub(relevant_node_count),
                relevant_checklist_count,
                satisfied_checklist_count,
                unresolved_checklist_count,
                completion_percent,
            },
            active_nodes,
            edges,
        };
        graph.validate_activation_evidence()?;
        Ok(graph)
    }

    pub fn compile_from_project_state(
        &self,
        spec: &GameSpec,
        domains: &[DomainDocument],
        project_state: &ProjectState,
    ) -> Result<CapabilityDecisionGraph, DecisionGraphError> {
        self.compile(
            spec,
            domains,
            &DecisionCoverage::from_project_state(project_state),
        )
    }
}

struct IndexedNode<'a> {
    domain_id: String,
    node: &'a DomainNode,
}

struct KnowledgeInventory<'a> {
    domain_count: usize,
    domain_ids: BTreeSet<String>,
    nodes: BTreeMap<String, IndexedNode<'a>>,
    known_constraints: BTreeSet<ConstraintKey>,
}

impl<'a> KnowledgeInventory<'a> {
    fn build(
        domains: &'a [DomainDocument],
        coverage: &DecisionCoverage,
    ) -> Result<Self, DecisionGraphError> {
        let mut domain_ids = BTreeSet::new();
        let mut nodes = BTreeMap::new();
        let mut known_constraints = BTreeSet::new();

        for domain in domains {
            let domain_id = domain.domain.id.trim();
            if domain_id.is_empty() || !domain_ids.insert(domain_id.to_string()) {
                return Err(DecisionGraphError::new(
                    "decision_graph.invalid_domain_id",
                    "/knowledge/domains",
                    format!("domain id '{domain_id}' is empty or duplicated"),
                    "provide one non-empty, unique domain id per knowledge document",
                ));
            }
            for node in &domain.nodes {
                if node.id.trim().is_empty() || nodes.contains_key(&node.id) {
                    return Err(DecisionGraphError::new(
                        "decision_graph.invalid_node_id",
                        format!("/knowledge/domains/{domain_id}/nodes"),
                        format!("node id '{}' is empty or duplicated", node.id),
                        "provide globally unique, non-empty node ids",
                    ));
                }
                if node.domain != domain_id {
                    return Err(DecisionGraphError::new(
                        "decision_graph.node_domain_mismatch",
                        format!("/knowledge/nodes/{}/domain", node.id),
                        format!(
                            "node '{}' declares domain '{}' but is stored under '{domain_id}'",
                            node.id, node.domain
                        ),
                        "make the node domain match its containing knowledge domain",
                    ));
                }
                if node.checklist.is_empty() {
                    return Err(DecisionGraphError::new(
                        "decision_graph.empty_node_constraints",
                        format!("/knowledge/nodes/{}/checklist", node.id),
                        format!("node '{}' has no machine-trackable constraint", node.id),
                        "add at least one checklist constraint before activation",
                    ));
                }
                let mut local_checklist_ids = BTreeSet::new();
                for item in &node.checklist {
                    if item.id.trim().is_empty() || !local_checklist_ids.insert(item.id.clone()) {
                        return Err(DecisionGraphError::new(
                            "decision_graph.invalid_checklist_id",
                            format!("/knowledge/nodes/{}/checklist", node.id),
                            format!(
                                "node '{}' contains an empty or duplicate checklist id '{}'",
                                node.id, item.id
                            ),
                            "provide unique, non-empty checklist ids within each node",
                        ));
                    }
                    known_constraints.insert(ConstraintKey::new(&node.id, &item.id));
                }
                nodes.insert(
                    node.id.clone(),
                    IndexedNode {
                        domain_id: domain_id.to_string(),
                        node,
                    },
                );
            }
        }

        for (node_id, indexed) in &nodes {
            for reference in indexed
                .node
                .requires
                .iter()
                .chain(indexed.node.unlocks.iter())
                .chain(indexed.node.recommended_before.iter())
                .chain(indexed.node.requires_any.iter())
                .chain(indexed.node.conflicts_with.iter())
            {
                if !nodes.contains_key(reference) {
                    return Err(DecisionGraphError::new(
                        "decision_graph.unknown_node_reference",
                        format!("/knowledge/nodes/{node_id}"),
                        format!("node '{node_id}' references unknown node '{reference}'"),
                        "repair the knowledge dependency before graph compilation",
                    ));
                }
            }
        }

        for key in &coverage.satisfied_constraints {
            if !known_constraints.contains(key) {
                return Err(DecisionGraphError::new(
                    "decision_graph.unknown_coverage_constraint",
                    format!("/coverage/{}/{}", key.node_id, key.checklist_id),
                    format!(
                        "coverage references unknown constraint '{}.{}'",
                        key.node_id, key.checklist_id
                    ),
                    "remove stale coverage or migrate it to a current checklist id",
                ));
            }
        }

        Ok(Self {
            domain_count: domains.len(),
            domain_ids,
            nodes,
            known_constraints,
        })
    }

    fn validate_policy_coverage(&self) -> Result<(), DecisionGraphError> {
        let supported = policy::SUPPORTED_DOMAINS
            .iter()
            .map(|domain| (*domain).to_string())
            .collect::<BTreeSet<_>>();
        if self.domain_ids != supported {
            let missing = supported
                .difference(&self.domain_ids)
                .cloned()
                .collect::<Vec<_>>();
            let unsupported = self
                .domain_ids
                .difference(&supported)
                .cloned()
                .collect::<Vec<_>>();
            return Err(DecisionGraphError::new(
                "decision_graph.policy_domain_mismatch",
                "/knowledge/domains",
                format!(
                    "activation policy mismatch; missing={missing:?}, unsupported={unsupported:?}"
                ),
                "update and review the capability policy together with the knowledge taxonomy",
            ));
        }

        let missing_conditional_nodes = policy::CONDITIONAL_NODE_IDS
            .iter()
            .filter(|node_id| !self.nodes.contains_key(**node_id))
            .copied()
            .collect::<Vec<_>>();
        if !missing_conditional_nodes.is_empty() {
            return Err(DecisionGraphError::new(
                "decision_graph.policy_node_mismatch",
                "/knowledge/nodes",
                format!(
                    "conditional policy references missing nodes: {missing_conditional_nodes:?}"
                ),
                "update the policy and taxonomy atomically so conditional rules cannot go stale",
            ));
        }
        Ok(())
    }
}

// Only unresolved, capability-relevant nodes participate in the executable graph.
// Resolved or irrelevant prerequisites therefore cannot block the current design route.
fn build_edges(
    nodes: &BTreeMap<String, IndexedNode<'_>>,
    active_nodes: &BTreeMap<String, ActiveDecisionNode>,
) -> Vec<DecisionEdge> {
    let mut edges = BTreeSet::new();
    for (node_id, active) in active_nodes {
        let node = nodes
            .get(node_id)
            .expect("active nodes are created from the validated inventory")
            .node;
        for required in &node.requires {
            if active_nodes.contains_key(required) {
                edges.insert(DecisionEdge {
                    from_node: required.clone(),
                    to_node: active.node_id.clone(),
                    kind: DecisionEdgeKind::Requires,
                });
            }
        }
        for required in &node.requires_any {
            if active_nodes.contains_key(required) {
                edges.insert(DecisionEdge {
                    from_node: required.clone(),
                    to_node: active.node_id.clone(),
                    kind: DecisionEdgeKind::RequiresAny,
                });
            }
        }
        for target in &node.recommended_before {
            if active_nodes.contains_key(target) {
                edges.insert(DecisionEdge {
                    from_node: active.node_id.clone(),
                    to_node: target.clone(),
                    kind: DecisionEdgeKind::RecommendedBefore,
                });
            }
        }
        for target in &node.conflicts_with {
            if active_nodes.contains_key(target) {
                let (from_node, to_node) = if active.node_id <= *target {
                    (active.node_id.clone(), target.clone())
                } else {
                    (target.clone(), active.node_id.clone())
                };
                edges.insert(DecisionEdge {
                    from_node,
                    to_node,
                    kind: DecisionEdgeKind::ConflictsWith,
                });
            }
        }
    }
    edges.into_iter().collect()
}

fn topological_order(
    active_nodes: &BTreeMap<String, ActiveDecisionNode>,
    edges: &[DecisionEdge],
) -> Result<Vec<String>, DecisionGraphError> {
    let mut indegree = active_nodes
        .keys()
        .map(|node_id| (node_id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<String, BTreeSet<String>>::new();
    for edge in edges.iter().filter(|edge| {
        matches!(
            edge.kind,
            DecisionEdgeKind::Requires | DecisionEdgeKind::RequiresAny
        )
    }) {
        if outgoing
            .entry(edge.from_node.clone())
            .or_default()
            .insert(edge.to_node.clone())
        {
            *indegree
                .get_mut(&edge.to_node)
                .expect("edge targets are active nodes") += 1;
        }
    }

    let mut ready = indegree
        .iter()
        .filter_map(|(node_id, degree)| (*degree == 0).then_some(node_id.clone()))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(active_nodes.len());
    while let Some(node_id) = ready.pop_first() {
        order.push(node_id.clone());
        if let Some(targets) = outgoing.get(&node_id) {
            for target in targets {
                let degree = indegree
                    .get_mut(target)
                    .expect("edge targets are active nodes");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(target.clone());
                }
            }
        }
    }

    if order.len() != active_nodes.len() {
        let blocked = indegree
            .into_iter()
            .filter_map(|(node_id, degree)| (degree > 0).then_some(node_id))
            .collect::<Vec<_>>();
        return Err(DecisionGraphError::new(
            "decision_graph.dependency_cycle",
            "/knowledge/nodes",
            format!("active hard dependencies contain a cycle: {blocked:?}"),
            "remove the hard cycle or downgrade a non-blocking relation to recommended ordering",
        ));
    }
    Ok(order)
}
