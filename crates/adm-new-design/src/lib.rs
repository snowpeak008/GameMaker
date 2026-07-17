#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use adm_new_contracts::project::{
    ChecklistOptionGroupState, DecisionState, EntityValidationError, NodeState,
    OptionProvenanceEntry, ProjectState,
};
use adm_new_foundation::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod ai_interview;
pub mod anti_overfit;
pub mod art_pipeline;
pub mod contracts;
pub mod data_loader;
pub mod decision_graph;
pub mod game_spec_projection;
pub mod handoff;
pub mod semantic_pipeline;

pub const CRATE_NAME: &str = "adm-new-design";

pub fn crate_ready() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignNodeSpec {
    pub node_id: String,
    pub domain_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub role_class: String,
    #[serde(default)]
    pub checklist: Vec<DesignChecklistItemSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignChecklistItemSpec {
    pub item_id: String,
    pub label: String,
    #[serde(default)]
    pub option_groups: Vec<DesignOptionGroupSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignOptionGroupSpec {
    pub group_id: String,
    #[serde(default)]
    pub selection_mode: String,
    #[serde(default)]
    pub allow_primary: bool,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignWorkbenchView {
    pub project_name: String,
    pub profile: Vec<ProfileFieldView>,
    pub domains: Vec<DomainSummaryView>,
    pub nodes: Vec<NodeCardView>,
    pub gameplay_systems: Value,
    pub project_coverage: CoverageMetrics,
    pub project_l4_progress: L4Progress,
    pub quality_metrics: QualityMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileFieldView {
    pub key: String,
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainSummaryView {
    pub domain_id: String,
    pub name: String,
    pub description: String,
    pub node_count: usize,
    pub node_percent: u32,
    pub checklist_percent: u32,
    pub l4_done: usize,
    pub l4_total: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeCardView {
    pub node_id: String,
    pub domain_id: String,
    pub name: String,
    pub description: String,
    pub role_class: String,
    pub effective_state: DecisionState,
    pub progress: ProgressMetrics,
    pub l4_progress: L4Progress,
    pub l5_entity_count: usize,
    pub entity_validation_error_count: usize,
    pub design_note: String,
    pub risk_note: String,
    pub not_applicable_reason: String,
    pub checklist_items: Vec<ChecklistItemView>,
    pub design_entities: Vec<Value>,
    pub entity_validation_errors: Vec<EntityValidationError>,
    pub palette: NodePalette,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecklistItemView {
    pub item_id: String,
    pub label: String,
    pub checked: bool,
    pub option_groups: Vec<OptionGroupView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionGroupView {
    pub group_id: String,
    pub selection_mode: String,
    pub allow_primary: bool,
    pub options: Vec<OptionView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionView {
    pub option_id: String,
    pub label: String,
    pub selected: bool,
    pub primary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressMetrics {
    pub done: usize,
    pub total: usize,
    pub percent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L4Progress {
    pub done: usize,
    pub total: usize,
    pub missing_items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageMetrics {
    pub done_nodes: usize,
    pub total_nodes: usize,
    pub node_percent: u32,
    pub done_checklist: usize,
    pub total_checklist: usize,
    pub checklist_percent: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub quality_badge: String,
    pub quality_critical_count: usize,
    pub quality_violations: Vec<QualityViolation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityViolation {
    pub id: String,
    pub violation_type: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodePalette {
    pub bg: String,
    pub border: String,
    pub marker: String,
}

#[derive(Debug, Clone)]
pub struct DesignEngineService {
    specs: Vec<DesignNodeSpec>,
}

impl DesignEngineService {
    pub fn new(specs: Vec<DesignNodeSpec>) -> Self {
        Self { specs }
    }

    pub fn specs(&self) -> &[DesignNodeSpec] {
        &self.specs
    }

    pub fn empty_state(&self) -> ProjectState {
        let mut state = ProjectState::empty();
        for spec in &self.specs {
            state
                .nodes
                .insert(spec.node_id.clone(), self.empty_node_state(spec));
        }
        state
    }

    pub fn normalize_state(&self, mut state: ProjectState) -> ProjectState {
        if state.project_name.trim().is_empty() {
            state.project_name = ProjectState::empty().project_name;
        }
        for spec in &self.specs {
            let default_node = self.empty_node_state(spec);
            let node_state = state
                .nodes
                .entry(spec.node_id.clone())
                .or_insert(default_node);
            normalize_node_state(spec, node_state);
        }
        state
    }

    pub fn effective_node_state(&self, node: &NodeState) -> DecisionState {
        match node.decision_state {
            DecisionState::NotApplicable => DecisionState::NotApplicable,
            DecisionState::Risk => DecisionState::Risk,
            DecisionState::Completed => DecisionState::Completed,
            DecisionState::Selected => DecisionState::Selected,
            DecisionState::NotStarted => {
                if !node.design_note.trim().is_empty()
                    || node.checklist.values().any(|value| *value)
                    || !node.design_entities.is_empty()
                {
                    DecisionState::Selected
                } else {
                    DecisionState::NotStarted
                }
            }
        }
    }

    pub fn refresh_node_state(&self, node: &mut NodeState) {
        if node.decision_state == DecisionState::NotApplicable {
            return;
        }
        let progress = node_progress(node);
        node.decision_state = if progress.total > 0 && progress.done == progress.total {
            DecisionState::Completed
        } else if progress.done > 0 || !node.design_note.trim().is_empty() {
            DecisionState::Selected
        } else {
            DecisionState::NotStarted
        };
    }

    pub fn set_checklist_item(
        &self,
        state: &mut ProjectState,
        node_id: &str,
        item_id: &str,
        checked: bool,
    ) -> AdmResult<()> {
        let node = state
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| AdmError::new(format!("unknown node: {node_id}")))?;
        node.checklist.insert(item_id.to_string(), checked);
        if !checked {
            node.checklist_options.remove(item_id);
            node.option_provenance.remove(item_id);
        }
        self.refresh_node_state(node);
        Ok(())
    }

    pub fn set_option_group_option(
        &self,
        state: &mut ProjectState,
        node_id: &str,
        item_id: &str,
        group_id: &str,
        option_id: &str,
        selected: bool,
    ) -> AdmResult<()> {
        let group_spec = self.find_group(node_id, item_id, group_id)?;
        if !group_spec.options.is_empty()
            && !group_spec.options.iter().any(|item| item == option_id)
        {
            return Err(AdmError::new(format!(
                "unknown option for group {group_id}: {option_id}"
            )));
        }
        let node = state
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| AdmError::new(format!("unknown node: {node_id}")))?;
        let mut should_mark_checked = false;
        {
            let group_state = node
                .checklist_options
                .entry(item_id.to_string())
                .or_default()
                .entry(group_id.to_string())
                .or_default();
            if selected {
                if group_spec.selection_mode == "single" {
                    group_state.selected.clear();
                }
                if !group_state.selected.iter().any(|item| item == option_id) {
                    group_state.selected.push(option_id.to_string());
                }
                should_mark_checked = true;
            } else {
                group_state.selected.retain(|item| item != option_id);
            }
            if !group_state
                .selected
                .iter()
                .any(|item| item == &group_state.primary)
            {
                group_state.primary.clear();
            }
        }
        if should_mark_checked {
            node.checklist.insert(item_id.to_string(), true);
        }
        if selected {
            ensure_user_provenance(node, item_id, group_id, option_id);
        } else {
            remove_option_provenance(node, item_id, group_id, option_id);
        }
        self.refresh_node_state(node);
        Ok(())
    }

    pub fn set_option_group_primary(
        &self,
        state: &mut ProjectState,
        node_id: &str,
        item_id: &str,
        group_id: &str,
        option_id: &str,
    ) -> AdmResult<()> {
        let group_spec = self.find_group(node_id, item_id, group_id)?;
        if !group_spec.allow_primary {
            return Err(AdmError::new(format!(
                "primary is not allowed for {group_id}"
            )));
        }
        let node = state
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| AdmError::new(format!("unknown node: {node_id}")))?;
        {
            let group_state = node
                .checklist_options
                .entry(item_id.to_string())
                .or_default()
                .entry(group_id.to_string())
                .or_default();
            if !group_state.selected.iter().any(|item| item == option_id) {
                return Err(AdmError::new(format!(
                    "primary option is not selected: {option_id}"
                )));
            }
            group_state.primary = option_id.to_string();
        }
        ensure_user_provenance(node, item_id, group_id, option_id);
        Ok(())
    }

    pub fn view_model(&self, state: &ProjectState) -> DesignWorkbenchView {
        let normalized = self.normalize_state(state.clone());
        let nodes = self
            .specs
            .iter()
            .map(|spec| {
                let node = normalized
                    .nodes
                    .get(&spec.node_id)
                    .cloned()
                    .unwrap_or_default();
                let effective = self.effective_node_state(&node);
                NodeCardView {
                    node_id: spec.node_id.clone(),
                    domain_id: spec.domain_id.clone(),
                    name: spec.name.clone(),
                    description: spec.description.clone(),
                    role_class: spec.role_class.clone(),
                    effective_state: effective.clone(),
                    progress: node_progress(&node),
                    l4_progress: node_l4_progress(spec, &node),
                    l5_entity_count: node.design_entities.len(),
                    entity_validation_error_count: node.entity_validation_errors.len(),
                    design_note: node.design_note.clone(),
                    risk_note: node.risk_note.clone(),
                    not_applicable_reason: node.not_applicable_reason.clone(),
                    checklist_items: checklist_item_views(spec, &node),
                    design_entities: node.design_entities.clone(),
                    entity_validation_errors: node.entity_validation_errors.clone(),
                    palette: palette_for_state(&effective),
                }
            })
            .collect::<Vec<_>>();
        DesignWorkbenchView {
            project_name: normalized.project_name,
            profile: profile_field_views(&normalized.profile),
            domains: domain_summary_views(&self.specs, &nodes),
            gameplay_systems: serde_json::to_value(normalized.gameplay_systems)
                .unwrap_or(Value::Null),
            project_coverage: project_coverage(&nodes),
            project_l4_progress: project_l4_progress(&nodes),
            quality_metrics: quality_metrics(&self.specs, &nodes),
            nodes,
        }
    }

    fn empty_node_state(&self, spec: &DesignNodeSpec) -> NodeState {
        let mut node = NodeState::default();
        for item in &spec.checklist {
            node.checklist.insert(item.item_id.clone(), false);
            let mut groups = BTreeMap::new();
            for group in &item.option_groups {
                groups.insert(group.group_id.clone(), ChecklistOptionGroupState::default());
            }
            if !groups.is_empty() {
                node.checklist_options.insert(item.item_id.clone(), groups);
            }
        }
        node
    }

    fn find_group(
        &self,
        node_id: &str,
        item_id: &str,
        group_id: &str,
    ) -> AdmResult<&DesignOptionGroupSpec> {
        self.specs
            .iter()
            .find(|spec| spec.node_id == node_id)
            .and_then(|spec| spec.checklist.iter().find(|item| item.item_id == item_id))
            .and_then(|item| {
                item.option_groups
                    .iter()
                    .find(|group| group.group_id == group_id)
            })
            .ok_or_else(|| {
                AdmError::new(format!(
                    "unknown option group: {node_id}/{item_id}/{group_id}"
                ))
            })
    }
}

fn normalize_node_state(spec: &DesignNodeSpec, node: &mut NodeState) {
    for item in &spec.checklist {
        node.checklist.entry(item.item_id.clone()).or_insert(false);
        for group in &item.option_groups {
            let selected_after = {
                let group_state = node
                    .checklist_options
                    .entry(item.item_id.clone())
                    .or_default()
                    .entry(group.group_id.clone())
                    .or_default();
                if !group.options.is_empty() {
                    group_state
                        .selected
                        .retain(|option| group.options.iter().any(|allowed| allowed == option));
                }
                if group.selection_mode == "single" {
                    group_state.selected.truncate(1);
                }
                if !group_state
                    .selected
                    .iter()
                    .any(|item| item == &group_state.primary)
                {
                    group_state.primary.clear();
                }
                group_state.selected.clone()
            };
            cleanup_group_provenance(node, &item.item_id, &group.group_id, &selected_after);
        }
    }
}

fn profile_field_views(profile: &BTreeMap<String, Value>) -> Vec<ProfileFieldView> {
    profile
        .iter()
        .map(|(key, value)| ProfileFieldView {
            key: key.clone(),
            label: humanize_id(key),
            value: json_value_label(value),
        })
        .collect()
}

fn domain_summary_views(
    specs: &[DesignNodeSpec],
    nodes: &[NodeCardView],
) -> Vec<DomainSummaryView> {
    let mut domain_order = Vec::new();
    for spec in specs {
        if !domain_order.iter().any(|id| id == &spec.domain_id) {
            domain_order.push(spec.domain_id.clone());
        }
    }
    domain_order
        .into_iter()
        .map(|domain_id| {
            let domain_nodes = nodes
                .iter()
                .filter(|node| node.domain_id == domain_id)
                .collect::<Vec<_>>();
            let node_count = domain_nodes.len();
            let done_nodes = domain_nodes
                .iter()
                .filter(|node| node.effective_state == DecisionState::Completed)
                .count();
            let done_checklist = domain_nodes.iter().map(|node| node.progress.done).sum();
            let total_checklist = domain_nodes.iter().map(|node| node.progress.total).sum();
            let l4_done = domain_nodes.iter().map(|node| node.l4_progress.done).sum();
            let l4_total = domain_nodes.iter().map(|node| node.l4_progress.total).sum();
            let name = humanize_id(&domain_id);
            DomainSummaryView {
                domain_id,
                name: name.clone(),
                description: format!("{name} domain contains {node_count} design nodes."),
                node_count,
                node_percent: percent(done_nodes, node_count),
                checklist_percent: percent(done_checklist, total_checklist),
                l4_done,
                l4_total,
            }
        })
        .collect()
}

fn checklist_item_views(spec: &DesignNodeSpec, node: &NodeState) -> Vec<ChecklistItemView> {
    spec.checklist
        .iter()
        .map(|item| ChecklistItemView {
            item_id: item.item_id.clone(),
            label: item.label.clone(),
            checked: *node.checklist.get(&item.item_id).unwrap_or(&false),
            option_groups: item
                .option_groups
                .iter()
                .map(|group| {
                    let group_state = node
                        .checklist_options
                        .get(&item.item_id)
                        .and_then(|groups| groups.get(&group.group_id))
                        .cloned()
                        .unwrap_or_default();
                    OptionGroupView {
                        group_id: group.group_id.clone(),
                        selection_mode: group.selection_mode.clone(),
                        allow_primary: group.allow_primary,
                        options: group
                            .options
                            .iter()
                            .map(|option_id| OptionView {
                                option_id: option_id.clone(),
                                label: humanize_id(option_id),
                                selected: group_state
                                    .selected
                                    .iter()
                                    .any(|selected| selected == option_id),
                                primary: group_state.primary == *option_id,
                            })
                            .collect(),
                    }
                })
                .collect(),
        })
        .collect()
}

fn node_progress(node: &NodeState) -> ProgressMetrics {
    let total = node.checklist.len();
    let done = node.checklist.values().filter(|value| **value).count();
    ProgressMetrics {
        done,
        total,
        percent: percent(done, total),
    }
}

fn node_l4_progress(spec: &DesignNodeSpec, node: &NodeState) -> L4Progress {
    let mut total = 0;
    let mut done = 0;
    let mut missing_items = Vec::new();
    for item in &spec.checklist {
        let mut item_missing = Vec::new();
        for group in &item.option_groups {
            total += 1;
            let selected = node
                .checklist_options
                .get(&item.item_id)
                .and_then(|groups| groups.get(&group.group_id))
                .map(|group_state| !group_state.selected.is_empty())
                .unwrap_or(false);
            if selected {
                done += 1;
            } else {
                item_missing.push(group.group_id.clone());
            }
        }
        if !item_missing.is_empty() {
            missing_items.push(format!("{}:{}", item.item_id, item_missing.join(",")));
        }
    }
    L4Progress {
        done,
        total,
        missing_items,
    }
}

fn project_coverage(nodes: &[NodeCardView]) -> CoverageMetrics {
    let total_nodes = nodes.len();
    let done_nodes = nodes
        .iter()
        .filter(|node| node.effective_state == DecisionState::Completed)
        .count();
    let done_checklist = nodes.iter().map(|node| node.progress.done).sum();
    let total_checklist = nodes.iter().map(|node| node.progress.total).sum();
    CoverageMetrics {
        done_nodes,
        total_nodes,
        node_percent: percent(done_nodes, total_nodes),
        done_checklist,
        total_checklist,
        checklist_percent: percent(done_checklist, total_checklist),
    }
}

fn project_l4_progress(nodes: &[NodeCardView]) -> L4Progress {
    L4Progress {
        done: nodes.iter().map(|node| node.l4_progress.done).sum(),
        total: nodes.iter().map(|node| node.l4_progress.total).sum(),
        missing_items: nodes
            .iter()
            .flat_map(|node| {
                node.l4_progress
                    .missing_items
                    .iter()
                    .map(|item| format!("{}:{item}", node.node_id))
            })
            .collect(),
    }
}

fn quality_metrics(specs: &[DesignNodeSpec], nodes: &[NodeCardView]) -> QualityMetrics {
    let mut violations = Vec::new();
    for (spec, node) in specs.iter().zip(nodes.iter()) {
        if matches!(
            spec.role_class.as_str(),
            "system_concrete" | "content_concrete"
        ) && node.l5_entity_count == 0
        {
            violations.push(QualityViolation {
                id: format!("missing_l5_entity_{}", node.node_id),
                violation_type: "missing_l5_entity".to_string(),
                severity: "CRITICAL".to_string(),
                message: "concrete node is missing L5 designEntities".to_string(),
            });
        }
        if node.entity_validation_error_count > 0 {
            violations.push(QualityViolation {
                id: format!("entity_validation_errors_{}", node.node_id),
                violation_type: "entity_validation_errors".to_string(),
                severity: "WARNING".to_string(),
                message: "node has entity validation errors".to_string(),
            });
        }
    }
    let quality_critical_count = violations
        .iter()
        .filter(|item| item.severity == "CRITICAL")
        .count();
    let any_l5 = nodes.iter().any(|node| node.l5_entity_count > 0);
    let quality_badge = if quality_critical_count == 0 && any_l5 {
        "L5_complete_consistent"
    } else if any_l5 {
        "L5_partial"
    } else {
        "L4_only_filled"
    }
    .to_string();
    QualityMetrics {
        quality_badge,
        quality_critical_count,
        quality_violations: violations,
    }
}

fn palette_for_state(state: &DecisionState) -> NodePalette {
    match state {
        DecisionState::Completed => NodePalette::new("#E7F7EF", "#0F8A5F", "#0F8A5F"),
        DecisionState::Risk => NodePalette::new("#FFF4DE", "#B45309", "#B45309"),
        DecisionState::NotApplicable => NodePalette::new("#F8FAFC", "#A8B7C5", "#A8B7C5"),
        DecisionState::Selected => NodePalette::new("#EAF1FF", "#2563EB", "#2563EB"),
        DecisionState::NotStarted => NodePalette::new("#FFFFFF", "#D7E0E8", "#F8FAFC"),
    }
}

impl NodePalette {
    fn new(bg: &str, border: &str, marker: &str) -> Self {
        Self {
            bg: bg.to_string(),
            border: border.to_string(),
            marker: marker.to_string(),
        }
    }
}

fn percent(done: usize, total: usize) -> u32 {
    if total == 0 {
        0
    } else {
        ((done as f64 / total as f64) * 100.0).round() as u32
    }
}

fn json_value_label(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn humanize_id(id: &str) -> String {
    let words = id
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    let mut label = String::new();
                    label.extend(first.to_uppercase());
                    label.push_str(chars.as_str());
                    label
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>();
    if words.is_empty() {
        id.to_string()
    } else {
        words.join(" ")
    }
}

fn ensure_user_provenance(node: &mut NodeState, item_id: &str, group_id: &str, option_id: &str) {
    node.option_provenance
        .entry(item_id.to_string())
        .or_default()
        .entry(group_id.to_string())
        .or_default()
        .entry(option_id.to_string())
        .or_insert_with(|| OptionProvenanceEntry {
            source: "user_selected".to_string(),
            confidence: Some(1.0),
            confirmed: Some(true),
            updated_at: String::new(),
            extra: BTreeMap::from([(
                "provenanceKind".to_string(),
                Value::String("structured_selection".to_string()),
            )]),
        });
}

fn remove_option_provenance(node: &mut NodeState, item_id: &str, group_id: &str, option_id: &str) {
    if let Some(group) = node
        .option_provenance
        .get_mut(item_id)
        .and_then(|items| items.get_mut(group_id))
    {
        group.remove(option_id);
    }
}

fn cleanup_group_provenance(
    node: &mut NodeState,
    item_id: &str,
    group_id: &str,
    selected: &[String],
) {
    if let Some(group) = node
        .option_provenance
        .get_mut(item_id)
        .and_then(|items| items.get_mut(group_id))
    {
        group.retain(|option_id, _| selected.iter().any(|selected| selected == option_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_reports_ready() {
        assert!(crate_ready());
        assert_eq!(CRATE_NAME, "adm-new-design");
    }

    #[test]
    fn design_empty_state_creates_spec_nodes_with_l4_options() {
        let service = sample_service();
        let state = service.empty_state();
        let node = state.nodes.get("combat_loop").unwrap();
        assert_eq!(node.decision_state, DecisionState::NotStarted);
        assert_eq!(node.checklist.get("core_loop"), Some(&false));
        assert!(
            node.checklist_options
                .get("core_loop")
                .unwrap()
                .contains_key("loop_type")
        );
    }

    #[test]
    fn design_normalize_cleans_primary_and_stale_provenance() {
        let service = sample_service();
        let mut state = service.empty_state();
        let node = state.nodes.get_mut("combat_loop").unwrap();
        let group = node
            .checklist_options
            .get_mut("core_loop")
            .unwrap()
            .get_mut("loop_type")
            .unwrap();
        group.selected = vec!["turn_based".to_string(), "invalid".to_string()];
        group.primary = "invalid".to_string();
        ensure_user_provenance(node, "core_loop", "loop_type", "invalid");

        let normalized = service.normalize_state(state);
        let group = normalized.nodes["combat_loop"].checklist_options["core_loop"]
            .get("loop_type")
            .unwrap();
        assert_eq!(group.selected, vec!["turn_based"]);
        assert_eq!(group.primary, "");
        assert!(
            normalized.nodes["combat_loop"].option_provenance["core_loop"]["loop_type"]
                .get("invalid")
                .is_none()
        );
    }

    #[test]
    fn design_set_checklist_item_clears_options_and_provenance() {
        let service = sample_service();
        let mut state = service.empty_state();
        service
            .set_option_group_option(
                &mut state,
                "combat_loop",
                "core_loop",
                "loop_type",
                "turn_based",
                true,
            )
            .unwrap();
        service
            .set_checklist_item(&mut state, "combat_loop", "core_loop", false)
            .unwrap();
        let node = &state.nodes["combat_loop"];
        assert!(!node.checklist["core_loop"]);
        assert!(node.checklist_options.get("core_loop").is_none());
        assert!(node.option_provenance.get("core_loop").is_none());
    }

    #[test]
    fn design_view_model_reports_coverage_l4_l5_quality_palette() {
        let service = sample_service();
        let mut state = service.empty_state();
        service
            .set_option_group_option(
                &mut state,
                "combat_loop",
                "core_loop",
                "loop_type",
                "turn_based",
                true,
            )
            .unwrap();
        state.nodes.get_mut("combat_loop").unwrap().design_entities =
            vec![serde_json::json!({"schemaVersion": "1.0", "kind": "loop"})];
        let view = service.view_model(&state);
        let node = view
            .nodes
            .iter()
            .find(|node| node.node_id == "combat_loop")
            .unwrap();
        assert_eq!(node.progress.percent, 100);
        assert_eq!(node.l4_progress.done, 1);
        assert_eq!(node.l5_entity_count, 1);
        assert_eq!(node.palette.border, "#0F8A5F");
        assert_eq!(node.description, "Define combat.");
        assert_eq!(node.checklist_items[0].label, "Core Loop");
        assert!(node.checklist_items[0].option_groups[0].options[0].selected);
        assert_eq!(node.design_entities.len(), 1);
        assert_eq!(view.domains[0].domain_id, "mechanics");
        assert_eq!(view.domains[0].checklist_percent, 100);
        assert_eq!(view.project_coverage.checklist_percent, 100);
        assert_eq!(view.quality_metrics.quality_badge, "L5_complete_consistent");
    }

    fn sample_service() -> DesignEngineService {
        DesignEngineService::new(vec![DesignNodeSpec {
            node_id: "combat_loop".to_string(),
            domain_id: "mechanics".to_string(),
            name: "Combat Loop".to_string(),
            description: "Define combat.".to_string(),
            role_class: "system_concrete".to_string(),
            checklist: vec![DesignChecklistItemSpec {
                item_id: "core_loop".to_string(),
                label: "Core Loop".to_string(),
                option_groups: vec![DesignOptionGroupSpec {
                    group_id: "loop_type".to_string(),
                    selection_mode: "single".to_string(),
                    allow_primary: true,
                    options: vec!["turn_based".to_string(), "real_time".to_string()],
                }],
            }],
        }])
    }
}
