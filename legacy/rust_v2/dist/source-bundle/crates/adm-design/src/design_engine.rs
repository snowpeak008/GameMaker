use crate::data_repository::{
    ChecklistItem, DesignDataRepository, DesignDomain, DesignNode, EntitySchema, OptionGroup,
    OptionRef, load_design_data_repository,
};
use crate::workbench_state::{
    AiInterviewState, DecisionState, EntityValidationError, GameplaySystemsState, NodeState,
    OptionGroupState, OptionProvenance, WorkbenchState,
};
use adm_foundation::{AdmError, AdmResult, UtcTimestamp};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressSummary {
    pub done: f32,
    pub total: usize,
    pub percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct L4ProgressSummary {
    pub done: usize,
    pub total: usize,
    pub percent: u8,
    pub missing_groups: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoverageSummary {
    pub node_percent: u8,
    pub checklist_percent: u8,
    pub done_nodes: f32,
    pub total_nodes: usize,
    pub done_checklist: usize,
    pub total_checklist: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchResultTabs {
    pub summary: String,
    pub missing: String,
    pub risk: String,
    pub validation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionConflict {
    pub node_id: String,
    pub node_name: String,
    pub item_id: String,
    pub item_label: String,
    pub source_label: String,
    pub target_label: String,
    pub reason: String,
    pub severity: String,
}

pub struct DesignEngine {
    data: DesignDataRepository,
    node_index: BTreeMap<String, (usize, usize)>,
}

impl DesignEngine {
    pub fn load(design_data_root: &Path) -> AdmResult<Self> {
        Self::new(load_design_data_repository(design_data_root)?)
    }

    pub fn new(data: DesignDataRepository) -> AdmResult<Self> {
        let mut node_index = BTreeMap::new();
        for (domain_index, domain) in data.domains.iter().enumerate() {
            for (node_index_in_domain, node) in domain.nodes.iter().enumerate() {
                node_index.insert(node.id.clone(), (domain_index, node_index_in_domain));
            }
        }
        Ok(Self { data, node_index })
    }

    pub fn data(&self) -> &DesignDataRepository {
        &self.data
    }

    pub fn first_domain_id(&self) -> &str {
        self.data
            .domains
            .first()
            .map(|domain| domain.id.as_str())
            .unwrap_or_default()
    }

    pub fn domains(&self) -> &[DesignDomain] {
        &self.data.domains
    }

    pub fn nodes(&self) -> impl Iterator<Item = &DesignNode> {
        self.data
            .domains
            .iter()
            .flat_map(|domain| domain.nodes.iter())
    }

    pub fn domain_nodes(&self, domain_id: &str) -> Vec<&DesignNode> {
        self.data
            .domains
            .iter()
            .find(|domain| domain.id == domain_id)
            .map(|domain| domain.nodes.iter().collect())
            .unwrap_or_default()
    }

    pub fn empty_state(&self) -> WorkbenchState {
        let nodes = self
            .nodes()
            .map(|node| (node.id.clone(), self.empty_node_state(node)))
            .collect();
        WorkbenchState {
            project_name: "未命名游戏设计项目".to_string(),
            profile: self.profile_defaults(),
            nodes,
            gameplay_systems: GameplaySystemsState::default(),
            ai_interview: AiInterviewState::default(),
            version: 0,
            dirty: false,
        }
    }

    pub fn normalize_state(&self, state: WorkbenchState) -> WorkbenchState {
        let mut normalized = state;
        if normalized.project_name.trim().is_empty() {
            normalized.project_name = "未命名游戏设计项目".to_string();
        }
        let mut profile = self.profile_defaults();
        profile.extend(normalized.profile);
        normalized.profile = profile;
        for node in self.nodes() {
            let mut node_state = normalized
                .nodes
                .remove(&node.id)
                .unwrap_or_else(|| self.empty_node_state(node));
            self.normalize_node_state(node, &mut node_state);
            normalized.nodes.insert(node.id.clone(), node_state);
        }
        normalized
    }

    pub fn set_node_state(
        &self,
        state: &mut WorkbenchState,
        node_id: &str,
        decision_state: DecisionState,
    ) -> AdmResult<()> {
        self.require_node(node_id)?;
        let node_state = state.nodes.entry(node_id.to_string()).or_default();
        node_state.decision_state = decision_state;
        if decision_state != DecisionState::NotApplicable {
            node_state.not_applicable_reason.clear();
        }
        self.mark_changed(state);
        Ok(())
    }

    pub fn set_checklist_item(
        &self,
        state: &mut WorkbenchState,
        node_id: &str,
        item_id: &str,
        checked: bool,
    ) -> AdmResult<()> {
        let node = self.require_node(node_id)?;
        self.require_item(node, item_id)?;
        let node_state = state.nodes.entry(node_id.to_string()).or_default();
        if checked && node_state.decision_state == DecisionState::NotApplicable {
            node_state.decision_state = DecisionState::NotStarted;
            node_state.not_applicable_reason.clear();
        }
        node_state.checklist.insert(item_id.to_string(), checked);
        if !checked {
            if let Some(item_options) = node_state.checklist_options.get_mut(item_id) {
                for group_state in item_options.values_mut() {
                    group_state.selected.clear();
                    group_state.primary.clear();
                }
            }
            node_state.option_provenance.remove(item_id);
        }
        self.refresh_node_state(state, node_id)?;
        self.mark_changed(state);
        Ok(())
    }

    pub fn set_option_group_option(
        &self,
        state: &mut WorkbenchState,
        node_id: &str,
        item_id: &str,
        group_id: &str,
        option_id: &str,
        checked: bool,
    ) -> AdmResult<()> {
        let group = self.require_group(node_id, item_id, group_id)?;
        if !group.options.iter().any(|option| option.id == option_id) {
            return Err(AdmError::validation(format!(
                "unknown option {option_id} in group {group_id}"
            )));
        }
        let single_selection = group.selection_mode == "single";
        let allow_primary = group.allow_primary;
        let node_state = state.nodes.entry(node_id.to_string()).or_default();
        let mut set_provenance = false;
        let mut remove_provenance = false;
        let mut auto_check_item = false;

        {
            let group_state = node_state
                .checklist_options
                .entry(item_id.to_string())
                .or_default()
                .entry(group_id.to_string())
                .or_default();

            if checked {
                if single_selection {
                    group_state.selected.clear();
                }
                if !group_state.selected.iter().any(|item| item == option_id) {
                    group_state.selected.push(option_id.to_string());
                }
                set_provenance = true;
                auto_check_item = true;
            } else {
                group_state.selected.retain(|item| item != option_id);
                remove_provenance = true;
            }

            if !group_state
                .selected
                .iter()
                .any(|item| item == &group_state.primary)
            {
                group_state.primary = if allow_primary && group_state.selected.len() == 1 {
                    group_state.selected[0].clone()
                } else {
                    String::new()
                };
            }
        }

        if set_provenance {
            self.set_option_provenance(node_state, item_id, group_id, option_id);
        }
        if auto_check_item {
            node_state.checklist.insert(item_id.to_string(), true);
        }
        if remove_provenance {
            if let Some(group_provenance) = node_state
                .option_provenance
                .get_mut(item_id)
                .and_then(|item| item.get_mut(group_id))
            {
                group_provenance.remove(option_id);
            }
        }
        self.refresh_node_state(state, node_id)?;
        self.mark_changed(state);
        Ok(())
    }

    pub fn set_option_group_primary(
        &self,
        state: &mut WorkbenchState,
        node_id: &str,
        item_id: &str,
        group_id: &str,
        option_id: &str,
    ) -> AdmResult<()> {
        let group = self.require_group(node_id, item_id, group_id)?;
        if !group.allow_primary {
            return Ok(());
        }
        let node_state = state.nodes.entry(node_id.to_string()).or_default();
        let group_state = node_state
            .checklist_options
            .entry(item_id.to_string())
            .or_default()
            .entry(group_id.to_string())
            .or_default();
        group_state.primary = if group_state.selected.iter().any(|item| item == option_id) {
            option_id.to_string()
        } else {
            String::new()
        };
        if !group_state.primary.is_empty() {
            self.set_option_provenance(node_state, item_id, group_id, option_id);
        }
        self.mark_changed(state);
        Ok(())
    }

    pub fn update_node_text(
        &self,
        state: &mut WorkbenchState,
        node_id: &str,
        field_name: &str,
        value: impl Into<String>,
    ) -> AdmResult<()> {
        self.require_node(node_id)?;
        let node_state = state.nodes.entry(node_id.to_string()).or_default();
        match field_name {
            "designNote" | "design_note" => node_state.design_note = value.into(),
            "riskNote" | "risk_note" => node_state.risk_note = value.into(),
            "notApplicableReason" | "not_applicable_reason" => {
                node_state.not_applicable_reason = value.into();
            }
            _ => {
                return Err(AdmError::validation(format!(
                    "unknown node text field {field_name}"
                )));
            }
        }
        self.refresh_node_state(state, node_id)?;
        self.mark_changed(state);
        Ok(())
    }

    pub fn update_node_design_entities_json(
        &self,
        state: &mut WorkbenchState,
        node_id: &str,
        raw_json: &str,
    ) -> AdmResult<()> {
        self.require_node(node_id)?;
        let value = serde_json::from_str::<Value>(raw_json)
            .map_err(|error| AdmError::validation(format!("L5 JSON 解析失败：{error}")))?;
        let entities = value
            .as_array()
            .cloned()
            .ok_or_else(|| AdmError::validation("L5 designEntities must be an array"))?;
        let (entities, errors) = self.normalize_node_design_entities(node_id, entities);
        let node_state = state.nodes.entry(node_id.to_string()).or_default();
        node_state.design_entities = entities;
        node_state.entity_validation_errors = errors;
        self.refresh_node_state(state, node_id)?;
        self.mark_changed(state);
        Ok(())
    }

    pub fn normalize_node_design_entities(
        &self,
        node_id: &str,
        raw_entities: Vec<Value>,
    ) -> (Vec<Value>, Vec<EntityValidationError>) {
        let mut entities = Vec::new();
        let mut errors = Vec::new();
        for (index, entity) in raw_entities.into_iter().enumerate() {
            let entity_path = format!("designEntities[{index}]");
            if !entity.is_object() {
                errors.push(entity_error(
                    node_id,
                    &entity_path,
                    "entity must be an object",
                    "",
                ));
                continue;
            }
            errors.extend(self.validate_entity(node_id, &entity_path, &entity));
            entities.push(entity);
        }
        (entities, errors)
    }

    pub fn effective_node_state(&self, node: &DesignNode, state: &WorkbenchState) -> DecisionState {
        let Some(node_state) = state.nodes.get(&node.id) else {
            return DecisionState::NotStarted;
        };
        if node_state.decision_state == DecisionState::NotApplicable {
            return DecisionState::NotApplicable;
        }
        let done = node
            .checklist
            .iter()
            .filter(|item| node_state.checklist.get(&item.id).copied().unwrap_or(false))
            .count();
        if !node.checklist.is_empty() && done == node.checklist.len() {
            return DecisionState::Completed;
        }
        if done > 0 || !node_state.design_note.trim().is_empty() {
            return DecisionState::Selected;
        }
        DecisionState::NotStarted
    }

    pub fn node_progress(&self, node: &DesignNode, state: &WorkbenchState) -> ProgressSummary {
        let Some(node_state) = state.nodes.get(&node.id) else {
            return progress(0.0, node.checklist.len());
        };
        if node_state.decision_state == DecisionState::NotApplicable {
            return progress(0.0, 0);
        }
        let done = node
            .checklist
            .iter()
            .filter(|item| node_state.checklist.get(&item.id).copied().unwrap_or(false))
            .count();
        progress(done as f32, node.checklist.len())
    }

    pub fn item_l4_progress(
        &self,
        node: &DesignNode,
        item: &ChecklistItem,
        state: &WorkbenchState,
    ) -> L4ProgressSummary {
        let Some(node_state) = state.nodes.get(&node.id) else {
            return l4_progress(0, 0, Vec::new());
        };
        if node_state.decision_state == DecisionState::NotApplicable
            || !node_state.checklist.get(&item.id).copied().unwrap_or(false)
        {
            return l4_progress(0, 0, Vec::new());
        }
        let required_groups = item
            .option_groups
            .iter()
            .filter(|group| group.required)
            .collect::<Vec<_>>();
        let mut done = 0;
        let mut missing = Vec::new();
        for group in &required_groups {
            let selected = node_state
                .checklist_options
                .get(&item.id)
                .and_then(|item_options| item_options.get(&group.id))
                .map(|group_state| !group_state.selected.is_empty())
                .unwrap_or(false);
            if selected {
                done += 1;
            } else {
                missing.push(group.label.clone());
            }
        }
        l4_progress(done, required_groups.len(), missing)
    }

    pub fn node_l4_progress(&self, node: &DesignNode, state: &WorkbenchState) -> L4ProgressSummary {
        let mut done = 0;
        let mut total = 0;
        let mut missing = Vec::new();
        for item in &node.checklist {
            let item_progress = self.item_l4_progress(node, item, state);
            done += item_progress.done;
            total += item_progress.total;
            missing.extend(item_progress.missing_groups);
        }
        l4_progress(done, total, missing)
    }

    pub fn domain_l4_progress(&self, domain_id: &str, state: &WorkbenchState) -> L4ProgressSummary {
        let mut done = 0;
        let mut total = 0;
        let mut missing = Vec::new();
        for node in self.domain_nodes(domain_id) {
            let progress = self.node_l4_progress(node, state);
            done += progress.done;
            total += progress.total;
            missing.extend(progress.missing_groups);
        }
        l4_progress(done, total, missing)
    }

    pub fn domain_coverage(&self, domain_id: &str, state: &WorkbenchState) -> CoverageSummary {
        let Some(domain) = self
            .data
            .domains
            .iter()
            .find(|domain| domain.id == domain_id)
        else {
            return coverage(0.0, 0, 0, 0);
        };
        let mut done_node_score = 0.0;
        let mut total_nodes = 0;
        let mut done_checklist = 0;
        let mut total_checklist = 0;
        for node_id in &domain.coverage_required_items {
            let Some(node) = self.node_by_id(node_id) else {
                continue;
            };
            let effective = self.effective_node_state(node, state);
            if effective == DecisionState::NotApplicable {
                continue;
            }
            total_nodes += 1;
            done_node_score += state_score(effective);
            let progress = self.node_progress(node, state);
            done_checklist += progress.done as usize;
            total_checklist += progress.total;
        }
        coverage(
            done_node_score,
            total_nodes,
            done_checklist,
            total_checklist,
        )
    }

    pub fn project_coverage(&self, state: &WorkbenchState) -> CoverageSummary {
        let mut done_node_score = 0.0;
        let mut total_nodes = 0;
        let mut done_checklist = 0;
        let mut total_checklist = 0;
        for domain in &self.data.domains {
            let coverage = self.domain_coverage(&domain.id, state);
            done_node_score += coverage.done_nodes;
            total_nodes += coverage.total_nodes;
            done_checklist += coverage.done_checklist;
            total_checklist += coverage.total_checklist;
        }
        coverage(
            done_node_score,
            total_nodes,
            done_checklist,
            total_checklist,
        )
    }

    pub fn missing_items(&self, domain_id: &str, state: &WorkbenchState) -> Vec<String> {
        let mut missing = Vec::new();
        for node in self.domain_nodes(domain_id) {
            let node_state = state.nodes.get(&node.id);
            if self.effective_node_state(node, state) == DecisionState::NotApplicable {
                continue;
            }
            if self.effective_node_state(node, state) == DecisionState::NotStarted {
                missing.push(format!("{}：节点未确认", node.name));
            }
            for item in &node.checklist {
                let checked = node_state
                    .and_then(|node_state| node_state.checklist.get(&item.id))
                    .copied()
                    .unwrap_or(false);
                if !checked {
                    missing.push(format!("{} / {}", node.name, item.label));
                } else {
                    let l4 = self.item_l4_progress(node, item, state);
                    if !l4.missing_groups.is_empty() {
                        missing.push(format!(
                            "{} / {}：L4 未完整（{}）",
                            node.name,
                            item.label,
                            l4.missing_groups.join(", ")
                        ));
                    }
                }
            }
        }
        missing
    }

    pub fn risk_items(&self, state: &WorkbenchState) -> Vec<(String, String)> {
        self.nodes()
            .filter_map(|node| {
                let node_state = state.nodes.get(&node.id)?;
                if node_state.risk_note.trim().is_empty()
                    || self.effective_node_state(node, state) == DecisionState::NotApplicable
                {
                    return None;
                }
                Some((node.name.clone(), node_state.risk_note.clone()))
            })
            .collect()
    }

    pub fn active_domain_option_conflicts(
        &self,
        state: &WorkbenchState,
        domain_id: &str,
    ) -> Vec<OptionConflict> {
        let mut conflicts = Vec::new();
        for node in self.domain_nodes(domain_id) {
            for item in &node.checklist {
                let selected_refs = self.selected_option_refs(state, &node.id, &item.id);
                for relation in &item.option_relations {
                    if relation.relation_type != "soft_conflict"
                        || !selected_refs.contains(&relation.source)
                    {
                        continue;
                    }
                    for target in &relation.targets {
                        if !selected_refs.contains(target) {
                            continue;
                        }
                        conflicts.push(OptionConflict {
                            node_id: node.id.clone(),
                            node_name: node.name.clone(),
                            item_id: item.id.clone(),
                            item_label: item.label.clone(),
                            source_label: self.option_ref_label(item, &relation.source),
                            target_label: self.option_ref_label(item, target),
                            reason: relation.reason.clone(),
                            severity: relation.severity.clone(),
                        });
                    }
                }
            }
        }
        conflicts
    }

    pub fn result_tabs(&self, state: &WorkbenchState, domain_id: &str) -> WorkbenchResultTabs {
        let project_coverage = self.project_coverage(state);
        let project_l4 = self.project_l4_progress(state);
        let summary = self.render_summary(state, &project_coverage, &project_l4);
        let missing = self.missing_items(domain_id, state);
        let risks = self.risk_items(state);
        let conflicts = self.active_domain_option_conflicts(state, domain_id);
        WorkbenchResultTabs {
            summary,
            missing: if missing.is_empty() {
                "当前领域没有缺失项。".to_string()
            } else {
                missing.join("\n")
            },
            risk: self.render_risk_tab(risks, conflicts),
            validation: self.render_validation_tab(state),
        }
    }

    pub fn project_l4_progress(&self, state: &WorkbenchState) -> L4ProgressSummary {
        let mut done = 0;
        let mut total = 0;
        let mut missing = Vec::new();
        for domain in &self.data.domains {
            let progress = self.domain_l4_progress(&domain.id, state);
            done += progress.done;
            total += progress.total;
            missing.extend(progress.missing_groups);
        }
        l4_progress(done, total, missing)
    }

    fn normalize_node_state(&self, node: &DesignNode, state: &mut NodeState) {
        if state.decision_state == DecisionState::NotApplicable {
            state.design_note = state.design_note.trim().to_string();
        }
        for item in &node.checklist {
            state.checklist.entry(item.id.clone()).or_insert(false);
            let item_options = state.checklist_options.entry(item.id.clone()).or_default();
            for group in &item.option_groups {
                let group_state = item_options.entry(group.id.clone()).or_default();
                let allowed = group
                    .options
                    .iter()
                    .map(|option| option.id.as_str())
                    .collect::<BTreeSet<_>>();
                group_state
                    .selected
                    .retain(|option_id| allowed.contains(option_id.as_str()));
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
            }
        }
        let (entities, errors) =
            self.normalize_node_design_entities(&node.id, state.design_entities.clone());
        state.design_entities = entities;
        state.entity_validation_errors = errors;
    }

    fn empty_node_state(&self, node: &DesignNode) -> NodeState {
        let mut state = NodeState::default();
        for item in &node.checklist {
            state.checklist.insert(item.id.clone(), false);
            let item_options = state.checklist_options.entry(item.id.clone()).or_default();
            for group in &item.option_groups {
                item_options.insert(group.id.clone(), OptionGroupState::default());
            }
        }
        state
    }

    fn profile_defaults(&self) -> BTreeMap<String, String> {
        self.data
            .profile_fields
            .iter()
            .map(|field| (field.id.clone(), "unknown".to_string()))
            .collect()
    }

    fn refresh_node_state(&self, state: &mut WorkbenchState, node_id: &str) -> AdmResult<()> {
        let node = self.require_node(node_id)?;
        let effective = self.effective_node_state(node, state);
        if let Some(node_state) = state.nodes.get_mut(node_id) {
            if node_state.decision_state != DecisionState::NotApplicable {
                node_state.decision_state = effective;
            }
        }
        Ok(())
    }

    fn validate_entity(
        &self,
        node_id: &str,
        entity_path: &str,
        entity: &Value,
    ) -> Vec<EntityValidationError> {
        let Some(object) = entity.as_object() else {
            return vec![entity_error(
                node_id,
                entity_path,
                "entity must be an object",
                "",
            )];
        };
        let schema = self.schema_for_entity(entity);
        let Some(schema) = schema else {
            return vec![entity_error(
                node_id,
                entity_path,
                "missing or unknown entity schema",
                "",
            )];
        };
        let mut errors = Vec::new();
        for required in &schema.required {
            if !object.contains_key(required) {
                errors.push(entity_error(
                    node_id,
                    &format!("{entity_path}.{required}"),
                    &format!("missing required field: {required}"),
                    &schema.id,
                ));
            }
        }
        for (key, expected) in &schema.constants {
            let Some(actual) = object.get(key).and_then(Value::as_str) else {
                continue;
            };
            if actual != expected {
                errors.push(entity_error(
                    node_id,
                    &format!("{entity_path}.{key}"),
                    &format!("expected constant {expected:?}"),
                    &schema.id,
                ));
            }
        }
        errors
    }

    fn schema_for_entity(&self, entity: &Value) -> Option<&EntitySchema> {
        let schema_id = entity
            .get("schema")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !schema_id.is_empty() {
            return self.data.entity_schemas.get(schema_id);
        }
        let kind = entity
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let version = normalize_schema_version(
            entity
                .get("schemaVersion")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        );
        self.data.entity_schemas.values().find(|schema| {
            schema.kind == kind && normalize_schema_version(&schema.schema_version) == version
        })
    }

    fn selected_option_refs(
        &self,
        state: &WorkbenchState,
        node_id: &str,
        item_id: &str,
    ) -> BTreeSet<OptionRef> {
        state
            .nodes
            .get(node_id)
            .and_then(|node_state| node_state.checklist_options.get(item_id))
            .into_iter()
            .flat_map(|item_options| item_options.iter())
            .flat_map(|(group_id, group_state)| {
                group_state.selected.iter().map(|option_id| OptionRef {
                    group_id: group_id.clone(),
                    option_id: option_id.clone(),
                })
            })
            .collect()
    }

    fn option_ref_label(&self, item: &ChecklistItem, option_ref: &OptionRef) -> String {
        item.option_groups
            .iter()
            .find(|group| group.id == option_ref.group_id)
            .and_then(|group| {
                group
                    .options
                    .iter()
                    .find(|option| option.id == option_ref.option_id)
                    .map(|option| format!("{} / {}", group.label, option.label))
            })
            .unwrap_or_else(|| format!("{} / {}", option_ref.group_id, option_ref.option_id))
    }

    fn render_summary(
        &self,
        state: &WorkbenchState,
        project_coverage: &CoverageSummary,
        project_l4: &L4ProgressSummary,
    ) -> String {
        let mut lines = vec![
            format!("项目：{}", state.project_name),
            format!("全项目节点覆盖率：{}%", project_coverage.node_percent),
            format!(
                "全项目三级子项覆盖率：{}%",
                project_coverage.checklist_percent
            ),
            format!("全项目 L4 完整度：{}/{}", project_l4.done, project_l4.total),
            String::new(),
            "项目画像：".to_string(),
        ];
        for field in &self.data.profile_fields {
            let value = state
                .profile
                .get(&field.id)
                .map(String::as_str)
                .unwrap_or("unknown");
            let label = field
                .options
                .iter()
                .find(|option| option.value == value)
                .map(|option| option.label.as_str())
                .unwrap_or(value);
            lines.push(format!("- {}：{}", field.label, label));
        }
        lines.push(String::new());
        lines.push("领域覆盖：".to_string());
        for domain in &self.data.domains {
            let coverage = self.domain_coverage(&domain.id, state);
            let l4 = self.domain_l4_progress(&domain.id, state);
            lines.push(format!(
                "- {}：节点 {}%，子项 {}%，L4 {}/{}",
                domain.name, coverage.node_percent, coverage.checklist_percent, l4.done, l4.total
            ));
        }
        lines.join("\n")
    }

    fn render_risk_tab(
        &self,
        risks: Vec<(String, String)>,
        conflicts: Vec<OptionConflict>,
    ) -> String {
        let mut lines = Vec::new();
        for (node_name, risk_note) in risks {
            lines.push(format!("- {node_name}：{risk_note}"));
        }
        if !conflicts.is_empty() {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.push("选项软冲突：".to_string());
            for conflict in conflicts {
                lines.push(format!(
                    "- {} / {}：{} ↔ {}。{}",
                    conflict.node_name,
                    conflict.item_label,
                    conflict.source_label,
                    conflict.target_label,
                    conflict.reason
                ));
            }
        }
        if lines.is_empty() {
            "暂无风险节点。".to_string()
        } else {
            lines.join("\n")
        }
    }

    fn render_validation_tab(&self, state: &WorkbenchState) -> String {
        let mut lines = vec![
            "数据校验：".to_string(),
            if self.data.validation_errors.is_empty() {
                "- 通过".to_string()
            } else {
                self.data
                    .validation_errors
                    .iter()
                    .map(|item| format!("- {item}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            String::new(),
            "L5 实体校验：".to_string(),
        ];
        let mut l5_errors = Vec::new();
        for node in self.nodes() {
            if let Some(node_state) = state.nodes.get(&node.id) {
                for error in &node_state.entity_validation_errors {
                    l5_errors.push(format!(
                        "- {} / {}：{}",
                        node.name, error.path, error.message
                    ));
                }
            }
        }
        if l5_errors.is_empty() {
            lines.push("- 通过".to_string());
        } else {
            lines.extend(l5_errors);
        }
        lines.join("\n")
    }

    fn node_by_id(&self, node_id: &str) -> Option<&DesignNode> {
        let (domain_index, node_index) = self.node_index.get(node_id).copied()?;
        self.data
            .domains
            .get(domain_index)
            .and_then(|domain| domain.nodes.get(node_index))
    }

    fn require_node(&self, node_id: &str) -> AdmResult<&DesignNode> {
        self.node_by_id(node_id)
            .ok_or_else(|| AdmError::validation(format!("unknown design node {node_id}")))
    }

    fn require_item<'a>(
        &self,
        node: &'a DesignNode,
        item_id: &str,
    ) -> AdmResult<&'a ChecklistItem> {
        node.checklist
            .iter()
            .find(|item| item.id == item_id)
            .ok_or_else(|| AdmError::validation(format!("unknown checklist item {item_id}")))
    }

    fn require_group(
        &self,
        node_id: &str,
        item_id: &str,
        group_id: &str,
    ) -> AdmResult<&OptionGroup> {
        let node = self.require_node(node_id)?;
        let item = self.require_item(node, item_id)?;
        item.option_groups
            .iter()
            .find(|group| group.id == group_id)
            .ok_or_else(|| AdmError::validation(format!("unknown option group {group_id}")))
    }

    fn set_option_provenance(
        &self,
        node_state: &mut NodeState,
        item_id: &str,
        group_id: &str,
        option_id: &str,
    ) {
        node_state
            .option_provenance
            .entry(item_id.to_string())
            .or_default()
            .entry(group_id.to_string())
            .or_default()
            .insert(
                option_id.to_string(),
                OptionProvenance {
                    source: "user_selected".to_string(),
                    confirmed: true,
                    actor: "user".to_string(),
                    ai_inference_id: String::new(),
                    updated_at_millis: UtcTimestamp::now().as_millis(),
                },
            );
    }

    fn mark_changed(&self, state: &mut WorkbenchState) {
        state.version = state.version.saturating_add(1);
        state.dirty = true;
    }
}

fn entity_error(
    node_id: &str,
    path: &str,
    message: &str,
    schema_id: &str,
) -> EntityValidationError {
    EntityValidationError {
        severity: "WARNING".to_string(),
        node_id: node_id.to_string(),
        path: path.to_string(),
        message: message.to_string(),
        schema_id: schema_id.to_string(),
    }
}

fn state_score(decision_state: DecisionState) -> f32 {
    match decision_state {
        DecisionState::Completed => 1.0,
        DecisionState::Selected | DecisionState::Risk => 0.5,
        DecisionState::NotStarted | DecisionState::NotApplicable => 0.0,
    }
}

fn progress(done: f32, total: usize) -> ProgressSummary {
    ProgressSummary {
        done,
        total,
        percent: percent(done, total),
    }
}

fn l4_progress(done: usize, total: usize, missing_groups: Vec<String>) -> L4ProgressSummary {
    L4ProgressSummary {
        done,
        total,
        percent: percent(done as f32, total),
        missing_groups,
    }
}

fn coverage(
    done_nodes: f32,
    total_nodes: usize,
    done_checklist: usize,
    total_checklist: usize,
) -> CoverageSummary {
    CoverageSummary {
        node_percent: percent(done_nodes, total_nodes),
        checklist_percent: percent(done_checklist as f32, total_checklist),
        done_nodes,
        total_nodes,
        done_checklist,
        total_checklist,
    }
}

fn percent(done: f32, total: usize) -> u8 {
    if total == 0 {
        0
    } else {
        ((done / total as f32) * 100.0).round().clamp(0.0, 100.0) as u8
    }
}

fn normalize_schema_version(value: &str) -> String {
    if value
        .chars()
        .next()
        .is_some_and(|item| item.is_ascii_digit())
    {
        format!("v{value}")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_design_data_repository;
    use std::path::PathBuf;

    #[test]
    fn repository_loads_real_design_data_and_template_l4_groups() {
        let repository = load_design_data_repository(&fixture_design_data_root()).expect("repo");

        assert!(repository.domains.len() >= 16);
        assert!(repository.gameplay_system_options.len() >= 5);
        assert!(repository.entity_schemas.contains_key("system_card_v1"));
        assert!(
            repository
                .domains
                .iter()
                .flat_map(|domain| &domain.nodes)
                .flat_map(|node| &node.checklist)
                .any(|item| !item.option_groups.is_empty()),
            "templateRef option groups should be materialized"
        );
    }

    #[test]
    fn empty_state_contains_profile_defaults_and_all_nodes() {
        let engine = fixture_engine();
        let state = engine.empty_state();

        assert_eq!(state.project_name, "未命名游戏设计项目");
        assert_eq!(
            state.profile.get("targetScale"),
            Some(&"unknown".to_string())
        );
        assert_eq!(state.nodes.len(), engine.nodes().count());
        assert!(!state.dirty);
    }

    #[test]
    fn l4_selection_checks_item_and_refreshes_results() {
        let engine = fixture_engine();
        let mut state = engine.empty_state();
        let (node_id, item_id, group_id, option_id) = first_required_l4_option(&engine);

        engine
            .set_option_group_option(&mut state, &node_id, &item_id, &group_id, &option_id, true)
            .expect("set option");

        let node_state = state.nodes.get(&node_id).expect("node state");
        assert_eq!(node_state.checklist.get(&item_id), Some(&true));
        assert!(state.dirty);
        assert!(state.version > 0);
        assert!(
            node_state
                .option_provenance
                .get(&item_id)
                .and_then(|item| item.get(&group_id))
                .and_then(|group| group.get(&option_id))
                .is_some()
        );
        let node = engine.require_node(&node_id).expect("node");
        let l4 = engine.node_l4_progress(node, &state);
        assert!(l4.done >= 1);
        let tabs = engine.result_tabs(&state, engine.first_domain_id());
        assert!(tabs.summary.contains("全项目节点覆盖率"));
    }

    #[test]
    fn l5_json_validation_populates_validation_tab() {
        let engine = fixture_engine();
        let mut state = engine.empty_state();
        let node_id = engine
            .nodes()
            .find(|node| {
                matches!(
                    node.role_class.as_str(),
                    "system_concrete" | "content_concrete"
                )
            })
            .expect("concrete node")
            .id
            .clone();

        engine
            .update_node_design_entities_json(
                &mut state,
                &node_id,
                r#"[{"schema":"system_card_v1","kind":"system","schemaVersion":"v1","id":"movement"}]"#,
            )
            .expect("update l5");

        let node_state = state.nodes.get(&node_id).expect("node state");
        assert!(!node_state.entity_validation_errors.is_empty());
        let tabs = engine.result_tabs(&state, engine.first_domain_id());
        assert!(tabs.validation.contains("missing required field"));

        engine
            .update_node_design_entities_json(
                &mut state,
                &node_id,
                r#"[{"schema":"system_card_v1","kind":"system","schemaVersion":"v1","id":"movement","inputs":[],"outputs":["position"],"owners":["program"],"updateTick":"fixed"}]"#,
            )
            .expect("update valid l5");
        assert!(
            state
                .nodes
                .get(&node_id)
                .expect("node state")
                .entity_validation_errors
                .is_empty()
        );
    }

    fn fixture_engine() -> DesignEngine {
        DesignEngine::load(&fixture_design_data_root()).expect("engine")
    }

    fn fixture_design_data_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("knowledge")
            .join("design_data")
    }

    fn first_required_l4_option(engine: &DesignEngine) -> (String, String, String, String) {
        for node in engine.nodes() {
            for item in &node.checklist {
                for group in &item.option_groups {
                    if group.required {
                        if let Some(option) = group.options.first() {
                            return (
                                node.id.clone(),
                                item.id.clone(),
                                group.id.clone(),
                                option.id.clone(),
                            );
                        }
                    }
                }
            }
        }
        panic!("expected at least one required L4 option group");
    }
}
