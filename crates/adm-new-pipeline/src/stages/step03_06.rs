use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use adm_new_contracts::ArtifactLocale;
use adm_new_design::art_pipeline::stage04::{
    build_asset_usage_binding_seed, build_audio_placeholder_requirements,
    build_image_consumable_spec, build_ui_slice_spec_contract, build_unity_import_policy,
    merge_asset_strategy_into_assets, normalize_asset_targets,
};
use adm_new_design::contracts::build_customization_score_report;
use adm_new_design::semantic_pipeline::{
    build_art_taxonomy_and_strategy, build_program_capability_contract,
    build_program_semantic_coverage_report,
};
use adm_new_foundation::io::{now_iso, write_json};
use adm_new_foundation::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::generation::{
    ParsedDesignSource, STAGE2_REQUIRED_PLAYABLE_CONTRACTS, StageOutputGenerator,
    artifact_locale_from_inputs, localized_text,
};
use crate::source::SourceGroup;
use crate::stages::step00_02::{
    DesignEntity, STEP01, STEP02, StagePluginSpec, extract_l5_entities, preferred_design_entities,
};

pub const STEP03: u32 = 3;
pub const STEP04: u32 = 4;
pub const STEP05: u32 = 5;
pub const STEP06: u32 = 6;

pub const PROGRAM_CONTRACT_SCHEMA: &str =
    "knowledge/schemas/program_requirements_contract.schema.json";
pub const ART_CONTRACT_SCHEMA: &str = "knowledge/schemas/art_requirements_contract.schema.json";
pub const STANDARD_SCHEMA_VERSION: &str = "1.0";
pub const SOURCE_CONTRACT_PROTOCOL: &str = "stage_02_playable_contract_bundle_v1";

pub fn step03_plugin_spec() -> StagePluginSpec {
    StagePluginSpec {
        stage_id: "03",
        source_groups: vec![SourceGroup {
            label: "program_requirements".to_string(),
            patterns: vec!["devflow_ProgReq_*".to_string()],
            mode: "latest".to_string(),
            required: false,
            source_ids: vec!["ProgReq".to_string()],
        }],
        test_mode_status: "success",
        generation_entrypoint: "apply_development_plan_outputs",
    }
}

pub fn step04_plugin_spec() -> StagePluginSpec {
    StagePluginSpec {
        stage_id: "04",
        source_groups: vec![SourceGroup {
            label: "art_requirements".to_string(),
            patterns: vec!["devflow_ArtReq_*".to_string()],
            mode: "latest".to_string(),
            required: false,
            source_ids: vec!["ArtReq".to_string()],
        }],
        test_mode_status: "success",
        generation_entrypoint: "apply_development_plan_outputs",
    }
}

pub fn step05_plugin_spec() -> StagePluginSpec {
    StagePluginSpec {
        stage_id: "05",
        source_groups: vec![SourceGroup {
            label: "program_review".to_string(),
            patterns: vec!["devflow_ProgReview_*".to_string()],
            mode: "latest".to_string(),
            required: false,
            source_ids: vec!["ProgReview".to_string()],
        }],
        test_mode_status: "success",
        generation_entrypoint: "apply_development_plan_outputs",
    }
}

pub fn step06_plugin_spec() -> StagePluginSpec {
    StagePluginSpec {
        stage_id: "06",
        source_groups: vec![SourceGroup {
            label: "art_review".to_string(),
            patterns: vec!["devflow_ArtReview_*".to_string()],
            mode: "latest".to_string(),
            required: false,
            source_ids: vec!["ArtReview".to_string()],
        }],
        test_mode_status: "success",
        generation_entrypoint: "apply_development_plan_outputs",
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgramRequirement {
    pub id: String,
    pub requirement: String,
    pub entity_id: String,
    pub entity_label: String,
    pub entity_kind: String,
    pub entity_schema: String,
    pub selection_id: String,
    pub source_refs: Vec<String>,
    pub phase: String,
    pub system_ids: Vec<String>,
    pub system_binding: Value,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub dependencies: Vec<String>,
    pub acceptance: String,
    pub trace_kind: String,
}

#[derive(Debug, Clone, Default)]
pub struct EntityToRequirementConverter;

impl EntityToRequirementConverter {
    pub fn convert(&self, parsed: &ParsedDesignSource) -> Vec<ProgramRequirement> {
        self.convert_entities_with_locale(&extract_l5_entities(parsed), ArtifactLocale::ZhCn)
    }

    pub fn convert_entities(&self, entities: &[DesignEntity]) -> Vec<ProgramRequirement> {
        self.convert_entities_with_locale(entities, ArtifactLocale::ZhCn)
    }

    pub fn convert_entities_with_locale(
        &self,
        entities: &[DesignEntity],
        locale: ArtifactLocale,
    ) -> Vec<ProgramRequirement> {
        let mut requirements = Vec::new();
        for entity in entities {
            let route = route_for_entity(entity, locale);
            for (suffix, description) in requirement_templates(entity, &route, locale) {
                let id = format!("ENT-REQ-{:03}", requirements.len() + 1);
                let label = non_empty_or(entity.label.clone(), &entity.entity_id);
                requirements.push(ProgramRequirement {
                    id,
                    requirement: if locale == ArtifactLocale::ZhCn {
                        format!("实现 L5 实体“{label}”的{description}。")
                    } else {
                        format!("Implement L5 entity \"{label}\" {description}.")
                    },
                    entity_id: entity.entity_id.clone(),
                    entity_label: entity.label.clone(),
                    entity_kind: entity.kind.clone(),
                    entity_schema: entity.schema.clone(),
                    selection_id: entity.source_selection_id.clone(),
                    source_refs: non_empty_vec([entity.source.clone()]),
                    phase: phase_for_entity(entity),
                    system_ids: Vec::new(),
                    system_binding: json!({}),
                    inputs: non_empty_vec([
                        "entity_definition".to_string(),
                        non_empty_or(entity.node_id.clone(), "design_node"),
                    ]),
                    outputs: vec![output_for_entity(entity, suffix)],
                    dependencies: non_empty_vec([entity.node_id.clone()]),
                    acceptance: if locale == ArtifactLocale::ZhCn {
                        format!("实体“{label}”具备可执行数据、运行时行为，并至少拥有一条验证路径。")
                    } else {
                        format!("Entity \"{label}\" has executable data, runtime behavior, and at least one validation path.")
                    },
                    trace_kind: "design_entity".to_string(),
                });
            }
        }
        requirements
    }
}

#[derive(Debug, Clone, Default)]
pub struct SystemBinder;

impl SystemBinder {
    pub fn bind(
        &self,
        requirements: &mut [ProgramRequirement],
        system_graph: &Value,
    ) -> Vec<ProgramRequirement> {
        let nodes = system_graph
            .get("nodes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for requirement in requirements.iter_mut() {
            let binding = self.best_binding(requirement, &nodes);
            let system_id = string_field(&binding, "system_id");
            requirement.system_binding = binding;
            requirement.system_ids = if system_id.is_empty() {
                Vec::new()
            } else {
                vec![system_id]
            };
        }
        requirements.to_vec()
    }

    fn best_binding(&self, requirement: &ProgramRequirement, nodes: &[Value]) -> Value {
        let dependency_ids = requirement
            .dependencies
            .iter()
            .filter(|value| !value.trim().is_empty())
            .cloned()
            .collect::<BTreeSet<_>>();
        for node in nodes {
            let node_id = string_field(node, "id");
            if !node_id.is_empty() && dependency_ids.contains(&node_id) {
                return json!({"system_id": node_id, "confidence": 1.0, "method": "dependency_id"});
            }
        }
        if let Some(node_id) = dependency_ids.iter().next() {
            return json!({"system_id": node_id, "confidence": 0.85, "method": "design_node_dependency"});
        }
        let req_text = format!("{} {}", requirement.requirement, requirement.entity_label);
        let req_tokens = tokens(&req_text);
        let mut best_id = String::new();
        let mut best_score = 0.0;
        for node in nodes {
            let node_text = format!(
                "{} {} {}",
                string_field(node, "name"),
                string_field(node, "id"),
                string_field(node, "responsibility")
            );
            let node_tokens = tokens(&node_text);
            let score = token_score(&req_tokens, &node_tokens);
            if score > best_score {
                best_score = score;
                best_id = string_field(node, "id");
            }
        }
        if best_score >= 0.4 {
            json!({"system_id": best_id, "confidence": round4(best_score), "method": "fuzzy_name"})
        } else {
            json!({"system_id": "", "confidence": 0.0, "method": "unmatched"})
        }
    }
}

#[derive(Debug, Clone)]
pub struct RequirementBindingEngine {
    systems: Vec<Value>,
    node_to_systems: BTreeMap<String, Vec<String>>,
    entity_to_systems: BTreeMap<String, Vec<String>>,
    source_to_entities: BTreeMap<String, Vec<Value>>,
}

impl RequirementBindingEngine {
    pub fn new(contract: &Value) -> Self {
        let systems = contract
            .get("systems")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let entities = contract
            .get("entities")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut engine = Self {
            systems,
            node_to_systems: BTreeMap::new(),
            entity_to_systems: BTreeMap::new(),
            source_to_entities: BTreeMap::new(),
        };
        engine.build_indexes(&entities);
        engine
    }

    pub fn bind_missing(&self, requirements: &mut [ProgramRequirement]) -> Value {
        let mut auto_bound = 0usize;
        for requirement in requirements.iter_mut() {
            if !requirement.system_ids.is_empty() {
                continue;
            }
            let binding = self.bind_requirement(requirement);
            requirement.system_ids = binding
                .get("system_ids")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            requirement.system_binding = binding;
            auto_bound += 1;
        }
        let mut stats = self.binding_stats(requirements);
        stats["auto_bound"] = json!(auto_bound);
        stats
    }

    pub fn bind_requirement(&self, requirement: &ProgramRequirement) -> Value {
        for (method_name, system_ids) in [
            ("dependency", self.bind_by_dependency(requirement)),
            ("source_entity", self.bind_by_source_entity(requirement)),
            ("semantic_match", self.bind_by_semantic_match(requirement)),
            ("default", self.bind_by_default(requirement)),
        ] {
            if !system_ids.is_empty() {
                return json!({
                    "system_ids": sorted_unique(system_ids),
                    "method": method_name,
                    "confidence": if method_name == "dependency" { 1.0 } else { 0.72 },
                });
            }
        }
        json!({"system_ids": ["SYS-UNKNOWN"], "method": "unknown_fallback", "confidence": 0.1})
    }

    pub fn binding_stats(&self, requirements: &[ProgramRequirement]) -> Value {
        let total = requirements.len();
        let bound = requirements
            .iter()
            .filter(|requirement| !requirement.system_ids.is_empty())
            .count();
        json!({
            "total": total,
            "bound": bound,
            "unbound": total.saturating_sub(bound),
            "binding_rate": ratio(bound, total),
            "system_count": self.systems.len(),
        })
    }

    fn build_indexes(&mut self, entities: &[Value]) {
        for system in &self.systems {
            let system_id = system_id(system);
            if system_id.is_empty() {
                continue;
            }
            for key in ["node_id", "system_id"] {
                let value = string_field(system, key);
                if !value.is_empty() {
                    self.node_to_systems
                        .entry(value)
                        .or_default()
                        .push(system_id.clone());
                }
            }
            for dep in string_array(system.get("dependencies")) {
                self.node_to_systems
                    .entry(dep)
                    .or_default()
                    .push(system_id.clone());
            }
            for entity_id in string_array(system.get("related_entities")) {
                self.entity_to_systems
                    .entry(entity_id)
                    .or_default()
                    .push(system_id.clone());
            }
        }
        for entity in entities {
            let source = string_field(entity, "source");
            if !source.is_empty() {
                self.source_to_entities
                    .entry(source)
                    .or_default()
                    .push(entity.clone());
            }
        }
    }

    fn bind_by_dependency(&self, requirement: &ProgramRequirement) -> Vec<String> {
        let mut matched = Vec::new();
        for dep in &requirement.dependencies {
            matched.extend(self.node_to_systems.get(dep).cloned().unwrap_or_default());
        }
        if !requirement.entity_id.is_empty() {
            matched.extend(
                self.entity_to_systems
                    .get(&requirement.entity_id)
                    .cloned()
                    .unwrap_or_default(),
            );
        }
        matched
    }

    fn bind_by_source_entity(&self, requirement: &ProgramRequirement) -> Vec<String> {
        let mut matched = Vec::new();
        for source_ref in &requirement.source_refs {
            for (entity_source, entities) in &self.source_to_entities {
                if source_ref != entity_source {
                    continue;
                }
                for entity in entities {
                    let entity_id = string_field(entity, "entity_id");
                    matched.extend(
                        self.entity_to_systems
                            .get(&entity_id)
                            .cloned()
                            .unwrap_or_default(),
                    );
                    matched.extend(
                        self.node_to_systems
                            .get(&string_field(entity, "node_id"))
                            .cloned()
                            .unwrap_or_default(),
                    );
                }
            }
        }
        matched
    }

    fn bind_by_semantic_match(&self, requirement: &ProgramRequirement) -> Vec<String> {
        let req_text = format!(
            "{} {} {} {}",
            requirement.requirement,
            requirement.entity_label,
            requirement.entity_kind,
            requirement.phase
        );
        let req_tokens = tokens(&req_text);
        let mut best_id = String::new();
        let mut best_score = 0.0;
        for system in &self.systems {
            let id = system_id(system);
            let system_text = format!(
                "{} {} {} {}",
                string_field(system, "system_name"),
                string_field(system, "system_id"),
                string_field(system, "node_id"),
                string_field(system, "node_type")
            );
            let score = token_score(&req_tokens, &tokens(&system_text));
            if !id.is_empty() && score > best_score {
                best_id = id;
                best_score = score;
            }
        }
        if best_score >= 0.35 && !best_id.is_empty() {
            vec![best_id]
        } else {
            Vec::new()
        }
    }

    fn bind_by_default(&self, requirement: &ProgramRequirement) -> Vec<String> {
        let req_tokens = tokens(&format!(
            "{} {}",
            requirement.requirement, requirement.phase
        ));
        for (domain, keywords) in domain_keywords() {
            if keywords.iter().any(|keyword| req_tokens.contains(*keyword)) {
                let matched = self.find_systems_by_keyword(domain, &keywords);
                if !matched.is_empty() {
                    return matched;
                }
            }
        }
        self.systems
            .first()
            .map(system_id)
            .into_iter()
            .filter(|id| !id.is_empty())
            .collect()
    }

    fn find_systems_by_keyword(&self, domain: &str, keywords: &[&str]) -> Vec<String> {
        self.systems
            .iter()
            .filter_map(|system| {
                let id = system_id(system);
                let text = format!(
                    "{} {}",
                    string_field(system, "system_name"),
                    string_field(system, "node_id")
                )
                .to_lowercase();
                if !id.is_empty()
                    && (text.contains(domain)
                        || keywords.iter().any(|keyword| text.contains(keyword)))
                {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }
}

pub fn build_requirement_quality_report(requirements: &[ProgramRequirement]) -> Value {
    build_requirement_quality_report_with_locale(requirements, ArtifactLocale::ZhCn)
}

fn build_requirement_quality_report_with_locale(
    requirements: &[ProgramRequirement],
    locale: ArtifactLocale,
) -> Value {
    let total = requirements.len();
    let bound = requirements
        .iter()
        .filter(|item| !item.system_ids.is_empty())
        .count();
    let traced = requirements
        .iter()
        .filter(|item| !item.source_refs.is_empty())
        .count();
    let placeholders = requirements
        .iter()
        .filter(|item| contains_placeholder(&item.requirement))
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let mut system_counts = BTreeMap::<String, usize>::new();
    for requirement in requirements {
        for system_id in &requirement.system_ids {
            if !system_id.trim().is_empty() {
                *system_counts.entry(system_id.clone()).or_default() += 1;
            }
        }
    }
    json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "requirement_count": total,
        "system_binding_rate": ratio(bound, total),
        "traceability_rate": ratio(traced, total),
        "placeholder_rate": ratio(placeholders.len(), total),
        "placeholder_requirement_ids": placeholders,
        "bound_system_count": system_counts.len(),
        "average_requirements_per_bound_system": if system_counts.is_empty() { 0.0 } else { round4(total as f64 / system_counts.len() as f64) },
        "requirements_per_system": system_counts,
    })
}

pub fn build_program_requirements_contract(input: ProgramContractInput<'_>) -> Value {
    let source_markdown = input.parsed.source.clone();
    let source_handoff = source_design_handoff(input.parsed, &source_markdown);
    let all_blockers = input
        .preflight_blockers
        .iter()
        .chain(input.contract_blockers)
        .chain(input.trace_blockers)
        .cloned()
        .collect::<Vec<_>>();
    let coverage = program_source_coverage(
        input.requirements,
        input.system_graph,
        input.resource_graph,
        input.required_contracts,
        &source_handoff,
    );
    let valid = all_blockers.is_empty() && !input.requirements.is_empty();
    json!({
        "schema_version": STANDARD_SCHEMA_VERSION,
        "generated_at": input.generated_at,
        "artifact_locale": input.locale,
        "consumer_stage": "step_3_program_requirements",
        "source_contract_protocol": SOURCE_CONTRACT_PROTOCOL,
        "source_design_handoff": source_handoff,
        "source_design_markdown": source_markdown,
        "source_coverage": coverage,
        "derivation_policy": {
            "may_derive": if input.locale == ArtifactLocale::ZhCn {
                json!(["从可玩性契约推导程序系统", "从冻结设计事实推导运行时数据绑定", "从可执行场景推导验收标准"])
            } else {
                json!(["program systems from playable contracts", "runtime data bindings from frozen design facts", "acceptance criteria from executable scenarios"])
            },
            "must_not_invent": if input.locale == ArtifactLocale::ZhCn {
                json!(["步骤 02 契约中不存在的新玩法系统", "源设计中不存在的新商业化或平台需求", "声明输出绑定之外的 Unity 路径"])
            } else {
                json!(["new gameplay systems not present in Step02 contracts", "new monetization or platform requirements not present in source design", "Unity paths outside declared output bindings"])
            },
            "markdown_role": "supporting_explanation_only",
        },
        "systems": program_contract_systems(input.system_graph, input.requirements, input.locale),
        "contracts": program_contract_contracts(input.requirements),
        "entities": program_contract_entities(input.requirements, input.parsed),
        "events": program_contract_events(input.requirements, input.locale),
        "authority": authority(input.required_contracts, input.locale),
        "acceptance": program_contract_acceptance(input.requirements, input.locale),
        "design_fact_bindings": design_fact_bindings(input.requirements, input.locale),
        "path_bindings": program_path_bindings(input.requirements),
        "source_files": {
            "design_markdown": source_markdown,
            "design_handoff": source_handoff,
            "playable_contracts": source_contract_paths(input.required_contracts),
            "program_structure_spec": "stage_03/program_structure_spec.json",
            "program_requirement_trace_report": "stage_03/program_requirement_trace_report.json",
        },
        "quality": {
            "schema": PROGRAM_CONTRACT_SCHEMA,
            "valid": valid,
            "blockers": stringify_blockers(&all_blockers),
            "warnings": coverage.get("coverage_gaps").cloned().unwrap_or_else(|| json!([])),
        },
        "valid": valid,
        "source": source_markdown,
        "source_contracts": source_contract_paths(input.required_contracts),
        "requirements": input.requirements,
        "binding_stats": input.binding_stats,
        "system_count": input.system_graph.get("nodes").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "resource_count": input.resource_graph.get("resources").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "program_structure_spec": "program_structure_spec.json",
        "preflight_blocking_issues": input.preflight_blockers,
        "structure_spec_summary": {
            "project_type": input.structure_spec.get("project_type").and_then(Value::as_str).unwrap_or_default(),
            "allowed_roots": input.structure_spec.get("allowed_roots").cloned().unwrap_or_else(|| json!([])),
            "path_binding_count": input.structure_spec.get("system_path_map").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        },
    })
}

pub struct ProgramContractInput<'a> {
    pub generated_at: &'a str,
    pub parsed: &'a ParsedDesignSource,
    pub requirements: &'a [ProgramRequirement],
    pub structure_spec: &'a Value,
    pub system_graph: &'a Value,
    pub resource_graph: &'a Value,
    pub binding_stats: &'a Value,
    pub required_contracts: &'a [String],
    pub contract_blockers: &'a [Value],
    pub trace_blockers: &'a [Value],
    pub preflight_blockers: &'a [Value],
    pub locale: ArtifactLocale,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtAssetRequirement {
    pub asset_id: String,
    pub name: String,
    pub asset_type: String,
    pub source: String,
    pub source_entity_id: String,
    pub source_node_id: String,
    pub purpose: String,
    pub dependencies: Vec<String>,
    pub unlocks: Vec<String>,
    pub priority: String,
    pub complexity: String,
    pub required_for_phase: String,
    pub status: String,
    pub trace_kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub resolution: String,
}

#[derive(Debug, Clone, Default)]
pub struct EntityToAssetConverter;

impl EntityToAssetConverter {
    pub fn convert(&self, parsed: &ParsedDesignSource) -> Vec<ArtAssetRequirement> {
        self.convert_entities_with_locale(&extract_l5_entities(parsed), ArtifactLocale::ZhCn)
    }

    pub fn convert_entities(&self, entities: &[DesignEntity]) -> Vec<ArtAssetRequirement> {
        self.convert_entities_with_locale(entities, ArtifactLocale::ZhCn)
    }

    pub fn convert_entities_with_locale(
        &self,
        entities: &[DesignEntity],
        locale: ArtifactLocale,
    ) -> Vec<ArtAssetRequirement> {
        let mut assets = Vec::new();
        for entity in entities {
            for spec in asset_specs_for(entity) {
                let asset_type = string_field(&spec, "asset_type");
                let priority =
                    non_empty_or(string_field(&spec, "priority"), &priority_for(&asset_type));
                let mut asset = ArtAssetRequirement {
                    asset_id: format!("ENTITY-ASSET-{:03}", assets.len() + 1),
                    name: asset_name(entity, &string_field(&spec, "suffix")),
                    asset_type: asset_type.clone(),
                    source: entity.source.clone(),
                    source_entity_id: entity.entity_id.clone(),
                    source_node_id: entity.node_id.clone(),
                    purpose: purpose_for_asset(entity, &asset_type, locale),
                    dependencies: non_empty_vec([entity.node_id.clone()]),
                    unlocks: vec![
                        "program_requirements".to_string(),
                        "art_production".to_string(),
                    ],
                    priority: priority.clone(),
                    complexity: non_empty_or(
                        string_field(&spec, "complexity"),
                        &complexity_for(&asset_type),
                    ),
                    required_for_phase: phase_for_entity(entity),
                    status: "requirement_defined".to_string(),
                    trace_kind: "design_entity".to_string(),
                    resolution: String::new(),
                };
                if priority == "P0" {
                    asset.resolution = resolution_for(&asset_type, locale);
                }
                assets.push(asset);
            }
        }
        assets
    }
}

#[derive(Debug, Clone, Default)]
pub struct MarketResearchSkill;

impl MarketResearchSkill {
    pub fn local_fallback(&self, parsed: &ParsedDesignSource) -> Value {
        self.local_fallback_with_locale(parsed, ArtifactLocale::ZhCn)
    }

    pub fn local_fallback_with_locale(
        &self,
        parsed: &ParsedDesignSource,
        locale: ArtifactLocale,
    ) -> Value {
        let raw_text = parsed.raw_text.to_lowercase();
        let (style, references) = if contains_any(
            &raw_text,
            &["hades", "rogue", "roguelike", "roguelite", "肉鸽"],
        ) {
            (
                "stylized_action_readability",
                if locale == ArtifactLocale::ZhCn {
                    vec![
                        "高对比角色剪影",
                        "清晰可读的战斗特效",
                        "神话感环境分层",
                        "明确的奖励图标",
                    ]
                } else {
                    vec![
                        "high-contrast character silhouettes",
                        "readable combat VFX",
                        "mythic environment layering",
                        "clear reward icons",
                    ]
                },
            )
        } else if contains_any(&raw_text, &["fps", "shooter", "射击", "枪"]) {
            (
                "readable_shooter_feedback",
                if locale == ArtifactLocale::ZhCn {
                    vec!["明确的枪口反馈", "可读的目标剪影", "不遮挡视野的命中特效"]
                } else {
                    vec![
                        "clear muzzle feedback",
                        "readable target silhouette",
                        "hit VFX that does not block view",
                    ]
                },
            )
        } else if contains_any(&raw_text, &["puzzle", "解谜", "match", "消除"]) {
            (
                "clean_puzzle_readability",
                if locale == ArtifactLocale::ZhCn {
                    vec!["清晰可读的棋盘", "渐进式提示反馈", "易区分的方块状态"]
                } else {
                    vec![
                        "readable board",
                        "progressive hint feedback",
                        "distinct block states",
                    ]
                },
            )
        } else {
            (
                "functional_game_readability",
                if locale == ArtifactLocale::ZhCn {
                    vec!["核心动作反馈", "清晰的状态层级", "易识别的资源图标"]
                } else {
                    vec![
                        "core action feedback",
                        "clear state hierarchy",
                        "recognizable resource icons",
                    ]
                },
            )
        };
        json!({
            "schema_version": 1,
            "generated_at": now_iso(),
            "artifact_locale": locale,
            "mode": "local_fallback",
            "style_direction": style,
            "reference_principles": references,
            "network_used": false,
        })
    }
}

pub fn build_art_requirements_contract(input: ArtContractInput<'_>) -> Value {
    let source_markdown = non_empty_or(input.parsed.source.clone(), "source_design");
    let source_handoff = source_design_handoff(input.parsed, &source_markdown);
    let standard_assets = standard_assets(input.assets, input.locale);
    let gate_blockers = value_array(input.asset_spec_gate.get("blockers"));
    let gate_warnings = value_array(input.asset_spec_gate.get("warnings"));
    let all_blockers = input
        .contract_blockers
        .iter()
        .cloned()
        .chain(gate_blockers.clone())
        .collect::<Vec<_>>();
    let coverage = art_source_coverage(
        &source_handoff,
        input.assets,
        &standard_assets,
        input.required_contracts,
    );
    let valid = !input.assets.is_empty()
        && all_blockers.is_empty()
        && input
            .asset_spec_gate
            .get("valid")
            .and_then(Value::as_bool)
            .unwrap_or(true);
    let mut warnings = stringify_blockers(&gate_warnings);
    warnings.extend(string_array(coverage.get("coverage_gaps")));
    json!({
        "schema_version": STANDARD_SCHEMA_VERSION,
        "generated_at": input.generated_at,
        "artifact_locale": input.locale,
        "consumer_stage": "step_4_art_requirements",
        "source_contract_protocol": SOURCE_CONTRACT_PROTOCOL,
        "source_design_handoff": source_handoff,
        "source_design_markdown": source_markdown,
        "source_coverage": coverage,
        "derivation_policy": {
            "may_derive": if input.locale == ArtifactLocale::ZhCn {
                json!(["从冻结的资产、界面、场景和音频契约推导视觉需求", "从已声明的资产使用方推导 Unity 导入与挂载需求", "在源契约允许时使用视觉占位回退"])
            } else {
                json!(["visual requirements from frozen asset, UI, scene, and audio contracts", "Unity import and mount requirements from declared asset consumers", "fallback visual placeholders when the source contract allows them"])
            },
            "must_not_invent": if input.locale == ArtifactLocale::ZhCn {
                json!(["步骤 02 契约中不存在的新玩法对象", "不可追踪的界面或场景对象", "音频供应方接入前的最终音频生成"])
            } else {
                json!(["new gameplay objects not present in Step02 contracts", "untraced UI screens or scene objects", "final audio generation before an audio provider is connected"])
            },
            "markdown_role": "supporting_explanation_only",
        },
        "visual_language": {
            "style_tokens": ["source_traced", "readable_runtime_2d"],
            "material_tokens": ["unity_sprite_or_prefab", "transparent_ui_when_needed"],
            "lighting_tokens": ["clear_silhouette", "runtime_visibility_first"],
        },
        "assets": standard_assets,
        "visual_states": visual_states(&standard_assets, input.locale),
        "ux_signal_bindings": ux_signal_bindings(&standard_assets),
        "drift_checks": drift_checks(&standard_assets, input.locale),
        "path_bindings": art_path_bindings(&standard_assets),
        "source_files": {
            "design_markdown": source_markdown,
            "design_handoff": source_handoff,
            "playable_contracts": source_contract_paths(input.required_contracts),
            "asset_spec_contract": "stage_04/asset_spec_contract.json",
            "asset_registry": "stage_04/asset_registry.json",
            "unity_asset_mount_plan": "stage_04/unity_asset_mount_plan.json",
            "audio_placeholder_plan": "stage_04/audio_placeholder_plan.json",
        },
        "quality": {
            "schema": ART_CONTRACT_SCHEMA,
            "valid": valid,
            "blockers": stringify_blockers(&all_blockers),
            "warnings": warnings,
        },
        "valid": valid,
        "source": source_markdown,
        "source_contracts": source_contract_paths(input.required_contracts),
        "raw_assets": input.assets,
        "asset_requirements": input.assets,
        "market_research": input.market_research,
        "rule": localized_text(
            input.locale,
            "美术需求由步骤 02 冻结的可玩性资产、界面、场景和音频契约生成。",
            "Art requirements are generated from Stage02 frozen playable asset, UI, scene, and audio contracts."
        ),
        "blockers": input.contract_blockers,
        "asset_spec_contract": "asset_spec_contract.json",
        "asset_spec_gate": input.asset_spec_gate,
        "audio_placeholders": input.assets.iter().filter(|asset| asset.asset_type == "audio_placeholder").collect::<Vec<_>>(),
    })
}

pub struct ArtContractInput<'a> {
    pub generated_at: &'a str,
    pub parsed: &'a ParsedDesignSource,
    pub assets: &'a [ArtAssetRequirement],
    pub market_research: &'a Value,
    pub asset_spec_gate: &'a Value,
    pub required_contracts: &'a [String],
    pub contract_blockers: &'a [Value],
    pub locale: ArtifactLocale,
}

#[derive(Debug, Clone, Default)]
pub struct PlaceholderDetector;

impl PlaceholderDetector {
    pub fn detect(&self, text: &str) -> Vec<String> {
        let lower = text.to_lowercase();
        placeholder_tokens()
            .iter()
            .filter(|token| lower.contains(&token.to_lowercase()))
            .map(|token| (*token).to_string())
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
pub struct IntelligentReviewer {
    placeholder_detector: PlaceholderDetector,
}

impl IntelligentReviewer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn review_program(&self, requirements: &[ProgramRequirement]) -> Value {
        self.review_program_with_locale(requirements, ArtifactLocale::ZhCn)
    }

    pub fn review_program_with_locale(
        &self,
        requirements: &[ProgramRequirement],
        locale: ArtifactLocale,
    ) -> Value {
        let mut issues = Vec::new();
        let mut placeholder_count = 0usize;
        let mut l4_derived_count = 0usize;
        if requirements.is_empty() {
            issues.push(issue(
                "PROGRAM_REQUIREMENTS_MISSING",
                "BLOCKER",
                "stage_03",
                "program_requirements_contract.json",
                "requirements",
                localized_text(
                    locale,
                    "未生成程序需求。",
                    "No program requirements were produced.",
                ),
                localized_text(
                    locale,
                    "请在设计交接后运行步骤 03。",
                    "Run Step 03 after design handoff.",
                ),
                locale,
            ));
        }
        for requirement in requirements {
            self.check_trace(
                &mut issues,
                "stage_03",
                "program_requirements_contract.json",
                &requirement.id,
                &requirement.source_refs,
                locale,
            );
            if requirement.system_ids.is_empty()
                && !is_project_configuration_requirement(requirement)
            {
                issues.push(issue(
                    "PROGRAM_REQUIREMENT_WITHOUT_SYSTEM",
                    "WARNING",
                    "stage_03",
                    "program_requirements_contract.json",
                    &requirement.id,
                    localized_text(
                        locale,
                        "需求尚未绑定到系统。",
                        "Requirement is not bound to a system.",
                    ),
                    localized_text(
                        locale,
                        "请通过依赖 ID 或系统语义匹配完成绑定。",
                        "Bind by dependency id or fuzzy system match.",
                    ),
                    locale,
                ));
            }
            if self.check_placeholder(
                &mut issues,
                "stage_03",
                "program_requirements_contract.json",
                &requirement.id,
                &requirement.requirement,
                locale,
            ) {
                placeholder_count += 1;
            }
            if is_l4_derived_requirement(requirement) {
                l4_derived_count += 1;
            }
            if requirement.requirement.chars().count() < 15 {
                issues.push(issue(
                    "PROGRAM_REQUIREMENT_TEXT_TOO_SHORT",
                    "WARNING",
                    "stage_03",
                    "program_requirements_contract.json",
                    &requirement.id,
                    localized_text(
                        locale,
                        "需求文本过短，无法描述有效行为。",
                        "Requirement text is too short to describe meaningful behavior.",
                    ),
                    localized_text(
                        locale,
                        "请补充结构、行为和验收内容。",
                        "Expand the requirement with structure, behavior, and acceptance.",
                    ),
                    locale,
                ));
            }
            if requirement.acceptance.trim().is_empty() {
                issues.push(issue(
                    "PROGRAM_REQUIREMENT_ACCEPTANCE_MISSING",
                    "CRITICAL",
                    "stage_03",
                    "program_requirements_contract.json",
                    &requirement.id,
                    localized_text(
                        locale,
                        "需求缺少验收标准。",
                        "Requirement has no acceptance criteria.",
                    ),
                    localized_text(
                        locale,
                        "请添加具体的验收说明。",
                        "Add a concrete acceptance statement.",
                    ),
                    locale,
                ));
            }
        }
        if l4_derived_count > 0 {
            issues.push(issue(
                "PROGRAM_REQUIREMENT_L4_DEPTH",
                "WARNING",
                "stage_03",
                "program_requirements_contract.json",
                "requirement_depth",
                &if locale == ArtifactLocale::ZhCn {
                    format!("有 {l4_derived_count} 条需求似乎来自 L4 设计决策。")
                } else {
                    format!(
                        "{l4_derived_count} requirements appear to come from L4 design decisions."
                    )
                },
                localized_text(
                    locale,
                    "请补全 L5 实体，以生成更接近实现层的需求。",
                    "Fill in L5 entities to generate more implementation-level requirements.",
                ),
                locale,
            ));
        }
        if !requirements.is_empty() && placeholder_count as f64 / requirements.len() as f64 > 0.5 {
            issues.push(issue(
                "PROGRAM_REQUIREMENT_PLACEHOLDER_RATE_HIGH",
                "BLOCKER",
                "stage_03",
                "program_requirements_contract.json",
                "placeholder_rate",
                localized_text(
                    locale,
                    "超过 50% 的需求仍包含占位文本。",
                    "More than 50% of requirements contain placeholder text.",
                ),
                localized_text(
                    locale,
                    "请先依据具体设计实体重新生成步骤 03，再继续规划。",
                    "Regenerate Step 03 from concrete design entities before planning.",
                ),
                locale,
            ));
        }
        review_report("program_requirements", issues, locale)
    }

    pub fn review_art(&self, assets: &[ArtAssetRequirement]) -> Value {
        self.review_art_with_locale(assets, ArtifactLocale::ZhCn)
    }

    pub fn review_art_with_locale(
        &self,
        assets: &[ArtAssetRequirement],
        locale: ArtifactLocale,
    ) -> Value {
        let mut issues = Vec::new();
        if assets.is_empty() {
            issues.push(issue(
                "ART_ASSETS_MISSING",
                "BLOCKER",
                "stage_04",
                "asset_registry.json",
                "assets",
                localized_text(locale, "未生成美术资产。", "No art assets were produced."),
                localized_text(
                    locale,
                    "请在设计交接后运行步骤 04。",
                    "Run Step 04 after design handoff.",
                ),
                locale,
            ));
        }
        for asset in assets {
            self.check_trace(
                &mut issues,
                "stage_04",
                "asset_registry.json",
                &asset.asset_id,
                std::slice::from_ref(&asset.source),
                locale,
            );
            for (field, present) in [
                ("asset_type", !asset.asset_type.is_empty()),
                ("purpose", !asset.purpose.is_empty()),
                ("priority", !asset.priority.is_empty()),
            ] {
                if !present {
                    issues.push(issue(
                        "ART_ASSET_REQUIRED_FIELD_MISSING",
                        "WARNING",
                        "stage_04",
                        "asset_registry.json",
                        &format!("{}.{}", asset.asset_id, field),
                        &if locale == ArtifactLocale::ZhCn {
                            format!("资产缺少 `{field}`。")
                        } else {
                            format!("Asset has no `{field}`.")
                        },
                        &if locale == ArtifactLocale::ZhCn {
                            format!("请补充 `{field}`，用于生产规划。")
                        } else {
                            format!("Populate `{field}` for production planning.")
                        },
                        locale,
                    ));
                }
            }
            if asset.asset_type != "audio_placeholder" {
                self.check_placeholder(
                    &mut issues,
                    "stage_04",
                    "asset_registry.json",
                    &asset.asset_id,
                    &asset.purpose,
                    locale,
                );
            }
        }
        if !assets.is_empty() {
            let present = assets
                .iter()
                .map(|asset| asset.asset_type.clone())
                .collect::<BTreeSet<_>>();
            for missing in ["ui", "effect", "environment"]
                .into_iter()
                .filter(|kind| !present.contains(*kind))
            {
                issues.push(issue(
                    "ART_ASSET_TYPE_MISSING",
                    "WARNING",
                    "stage_04",
                    "asset_registry.json",
                    "asset_types",
                    &if locale == ArtifactLocale::ZhCn {
                        format!("未找到 `{missing}` 类型的资产。")
                    } else {
                        format!("No assets of type `{missing}` found.")
                    },
                    &if locale == ArtifactLocale::ZhCn {
                        format!("请通过补充相关 L5 实体生成 `{missing}` 资产。")
                    } else {
                        format!("Generate {missing} assets by adding relevant L5 entities.")
                    },
                    locale,
                ));
            }
            if !assets.iter().any(|asset| asset.priority == "P0") {
                issues.push(issue(
                    "ART_P0_ASSET_MISSING",
                    "WARNING",
                    "stage_04",
                    "asset_registry.json",
                    "p0_assets",
                    localized_text(locale, "尚未定义 P0 优先级资产。", "No P0 priority assets defined."),
                    localized_text(locale, "请将角色、武器、核心界面或特效等关键路径资产标记为 P0。", "Mark critical-path assets such as character, weapon, core UI, or VFX as P0."),
                    locale,
                ));
            }
        }
        review_report("art_requirements", issues, locale)
    }

    fn check_trace(
        &self,
        issues: &mut Vec<Value>,
        stage: &str,
        artifact: &str,
        item_id: &str,
        sources: &[String],
        locale: ArtifactLocale,
    ) {
        if sources.iter().all(|source| source.trim().is_empty()) {
            issues.push(issue(
                "SOURCE_TRACE_MISSING",
                "CRITICAL",
                stage,
                artifact,
                item_id,
                localized_text(locale, "条目缺少来源追踪。", "Item has no source trace."),
                localized_text(
                    locale,
                    "请保留来自设计选择或 L5 实体的来源引用。",
                    "Keep source refs from design selection or L5 entity.",
                ),
                locale,
            ));
        }
    }

    fn check_placeholder(
        &self,
        issues: &mut Vec<Value>,
        stage: &str,
        artifact: &str,
        item_id: &str,
        text: &str,
        locale: ArtifactLocale,
    ) -> bool {
        let tokens = self.placeholder_detector.detect(text);
        if tokens.is_empty() {
            return false;
        }
        issues.push(issue(
            "PLACEHOLDER_TOKEN_REMAINS",
            "CRITICAL",
            stage,
            artifact,
            item_id,
            &if locale == ArtifactLocale::ZhCn {
                format!("仍存在占位标记：{}。", tokens.join("、"))
            } else {
                format!("Placeholder tokens remain: {}.", tokens.join(", "))
            },
            localized_text(
                locale,
                "请用具体的设计驱动内容替换模板文本。",
                "Replace template text with concrete design-driven content.",
            ),
            locale,
        ));
        true
    }
}

fn load_stage_artifact(out_dir: &Path, stage: u32, file_name: &str) -> Option<Value> {
    let path = out_dir
        .parent()
        .unwrap_or(out_dir)
        .join(format!("stage_{stage:02}"))
        .join(file_name);
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn load_project_dna_contract(
    out_dir: &Path,
    _consumer_stage: &str,
    locale: ArtifactLocale,
) -> (Value, Vec<Value>) {
    let Some(contract) = load_stage_artifact(out_dir, STEP02, "project_dna_contract.json") else {
        return (
            json!({}),
            vec![protocol_issue(
                "PROJECT_DNA_CONTRACT_MISSING",
                "BLOCKER",
                "02",
                "stage_02/project_dna_contract.json",
                "project_dna_contract",
                locale,
                (
                    "缺少冻结的项目 DNA 契约",
                    "步骤 03 及后续阶段无法读取步骤 02 的项目 DNA 契约。",
                    "请返回步骤 02，重新生成并冻结项目 DNA 契约。",
                ),
                (
                    "Frozen Project DNA contract missing",
                    "Step03 and later stages cannot read the Step02 Project DNA contract.",
                    "Return to Step02 and regenerate the frozen Project DNA contract.",
                ),
            )],
        );
    };
    let state = string_field(&contract, "contract_state");
    let status = string_field(&contract, "status");
    let frozen = state == "frozen" && status != "blocked";
    if frozen {
        (contract, Vec::new())
    } else {
        (
            contract,
            vec![protocol_issue(
                "PROJECT_DNA_CONTRACT_NOT_FROZEN",
                "BLOCKER",
                "02",
                "stage_02/project_dna_contract.json",
                "contract_state",
                locale,
                (
                    "项目 DNA 契约尚未冻结",
                    "项目 DNA 契约仍处于未冻结或阻断状态，不能供后续阶段消费。",
                    "请在步骤 02 解决冻结阻断项后重新运行。",
                ),
                (
                    "Project DNA contract is not frozen",
                    "The Project DNA contract is unfrozen or blocked and cannot be consumed downstream.",
                    "Resolve the Step02 freeze blockers and rerun the stage.",
                ),
            )],
        )
    }
}

fn load_archetype_requirements(out_dir: &Path, structured_inputs: &Value) -> Value {
    load_stage_artifact(out_dir, STEP01, "archetype_requirements.json")
        .or_else(|| {
            structured_inputs
                .get("inputs")
                .and_then(|inputs| inputs.get("archetype_requirements"))
                .cloned()
        })
        .unwrap_or_else(|| json!({}))
}

fn load_program_requirements(out_dir: &Path) -> Option<Vec<ProgramRequirement>> {
    let value = load_stage_artifact(out_dir, STEP03, "program_requirements.json")?;
    serde_json::from_value(value).ok()
}

fn load_art_requirements(out_dir: &Path) -> Option<Vec<ArtAssetRequirement>> {
    let registry = load_stage_artifact(out_dir, STEP04, "asset_registry.json")?;
    Some(
        value_array(registry.get("assets"))
            .iter()
            .map(art_requirement_from_value)
            .collect(),
    )
}

fn annotate_artifact_locale(document: &mut Value, locale: ArtifactLocale) {
    if document.is_object() {
        document["artifact_locale"] = json!(locale);
    }
}

fn localize_program_capability_contract(document: &mut Value, locale: ArtifactLocale) {
    annotate_artifact_locale(document, locale);
    for capability in document
        .get_mut("capabilities")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        let semantic_id = non_empty_or(
            string_field(capability, "source_semantic_id"),
            &string_field(capability, "capability_id"),
        );
        let capability_type = string_field(capability, "capability_type");
        for state_change in capability
            .get_mut("state_changes")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            state_change["description"] = json!(match (capability_type.as_str(), locale) {
                ("core_entity", ArtifactLocale::ZhCn) => {
                    format!("{semantic_id} 的实体状态会在可玩流程中发生变化。")
                }
                ("gameplay_system", ArtifactLocale::ZhCn) => {
                    format!("{semantic_id} 的运行时状态会在执行期间发生变化。")
                }
                ("player_action", ArtifactLocale::ZhCn) => {
                    format!("{semantic_id} 必须产生可观察的状态变化。")
                }
                ("resource_model", ArtifactLocale::ZhCn) => {
                    format!("运行时代码必须追踪 {semantic_id} 的数量变化。")
                }
                ("objective", ArtifactLocale::ZhCn) => {
                    format!("必须追踪 {semantic_id} 的完成或失败状态。")
                }
                _ => string_field(state_change, "description"),
            });
        }
    }
    let mut blockers = value_array(document.get("blocking_issues"));
    for blocker in &mut blockers {
        localize_semantic_blocker(blocker, locale);
    }
    document["blocking_issues"] = json!(blockers);
    document["blockers"] = document["blocking_issues"].clone();
    document["source_refs"] = json!(["stage_02/project_dna_contract.json"]);
}

fn localize_program_semantic_coverage_report(document: &mut Value, locale: ArtifactLocale) {
    annotate_artifact_locale(document, locale);
    if let Some(blockers) = document.get_mut("blockers").and_then(Value::as_array_mut) {
        for blocker in blockers {
            localize_semantic_blocker(blocker, locale);
        }
    }
}

fn localize_semantic_blocker(blocker: &mut Value, locale: ArtifactLocale) {
    let code = string_field(blocker, "code");
    blocker["message"] = json!(match (code.as_str(), locale) {
        ("ACTION_HAS_NO_STATE_CHANGE", ArtifactLocale::ZhCn) => {
            "玩家动作必须声明可观察的状态变化。".to_string()
        }
        ("OBJECTIVE_HAS_NO_COMPLETION_CONDITION", ArtifactLocale::ZhCn) => {
            "目标必须声明完成条件或失败条件。".to_string()
        }
        ("PROGRAM_CAPABILITY_NOT_BOUND", ArtifactLocale::ZhCn) => {
            "必需的程序语义覆盖率低于 85%。".to_string()
        }
        _ => string_field(blocker, "message"),
    });
}

fn localize_and_complete_asset_values(assets: &mut [Value], locale: ArtifactLocale) {
    for asset in assets {
        let asset_id = non_empty_or(string_field(asset, "asset_id"), "asset");
        let asset_type = non_empty_or(string_field(asset, "asset_type"), "art_asset");
        let role = non_empty_or(
            string_field(asset, "asset_role"),
            &non_empty_or(string_field(asset, "name"), &asset_id),
        );
        let consumer = non_empty_or(
            string_field(asset, "consumer_system"),
            &non_empty_or(
                string_field(asset, "source_entity_id"),
                &string_field(asset, "source_node_id"),
            ),
        );
        let target = non_empty_or(
            string_field(asset, "unity_target_path"),
            &string_field(asset, "target_path"),
        );
        let mut source_refs = string_array(asset.get("source_refs"));
        if source_refs.is_empty() {
            let source = string_field(asset, "source");
            if !source.is_empty() {
                source_refs.push(source);
            }
        }
        if source_refs.is_empty() {
            source_refs.push("stage_04/asset_strategy_matrix.json".to_string());
        }
        asset["source_refs"] = json!(source_refs);
        asset["source"] = json!(non_empty_or(
            string_field(asset, "source"),
            asset["source_refs"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(Value::as_str)
                .unwrap_or("stage_04/asset_strategy_matrix.json"),
        ));
        asset["name"] = json!(non_empty_or(string_field(asset, "name"), &role));
        if string_field(asset, "trace_kind") == "asset_strategy_matrix"
            || string_field(asset, "purpose").is_empty()
        {
            asset["purpose"] = json!(if locale == ArtifactLocale::ZhCn {
                format!("为 {consumer} 提供可消费的“{role}”{asset_type}资源。")
            } else {
                format!("Provide consumable {asset_type} asset \"{role}\" for {consumer}.")
            });
        }
        asset["consumer_system"] = json!(non_empty_or(consumer, "core_loop_runtime"));
        asset["target_path"] = json!(target);
        asset["unity_target_path"] = asset["target_path"].clone();
        asset["mount_point"] = json!(non_empty_or(
            string_field(asset, "mount_point"),
            asset["unity_target_path"]
                .as_str()
                .and_then(|path| path.rsplit_once('/').map(|(parent, _)| parent))
                .unwrap_or("Assets/AutoDesign"),
        ));
        let fallback = localized_text(
            locale,
            "生产资源不可用时使用明确标记的可见占位资源；音频仅使用静音标记。",
            "Use an explicitly marked visible placeholder when production art is unavailable; audio uses a silent marker only.",
        );
        asset["fallback_strategy"] = json!(fallback);
        asset["fallback_policy"] = json!(fallback);
        asset["acceptance_check"] = json!(non_empty_or(
            string_field(asset, "acceptance_check"),
            &format!("{asset_id}_available_for_runtime"),
        ));
        let dimensions = asset_dimensions(&asset_type);
        asset["dimensions"] = dimensions.clone();
        asset["required_size"] = dimensions;
        asset["background"] = json!(asset_background(&asset_type));
        asset["transparency"] = json!(if matches!(
            asset_type.to_ascii_lowercase().as_str(),
            "ui" | "icon" | "sprite" | "effect" | "vfx"
        ) {
            "transparent_alpha"
        } else {
            "not_required"
        });
        asset["required_format"] = json!(match asset_type.to_ascii_lowercase().as_str() {
            "ui" | "icon" | "sprite" | "effect" | "vfx" => "png_with_alpha",
            "audio_placeholder" => "silent_placeholder_marker",
            _ => "png_or_unity_prefab",
        });
        if asset.get("import_settings").is_none() {
            asset["import_settings"] = default_import_settings(&asset_type);
        }
        if string_field(asset, "linked_contract_field").is_empty() {
            asset["linked_contract_field"] = json!(non_empty_or(
                string_field(asset, "strategy_id"),
                &non_empty_or(
                    string_field(asset, "source_entity_id"),
                    &string_field(asset, "source_node_id"),
                ),
            ));
        }
    }
}

fn default_import_settings(asset_type: &str) -> Value {
    match asset_type.to_ascii_lowercase().as_str() {
        "ui" | "icon" | "sprite" => json!({
            "texture_type": "Sprite",
            "alpha_is_transparency": true,
            "mipmap_enabled": false,
        }),
        "effect" | "vfx" => json!({
            "texture_type": "Sprite",
            "alpha_is_transparency": true,
            "mipmap_enabled": false,
        }),
        "audio_placeholder" => json!({"importer": "silent_marker"}),
        _ => json!({"texture_type": "Default", "mipmap_enabled": true}),
    }
}

fn art_requirement_from_value(asset: &Value) -> ArtAssetRequirement {
    ArtAssetRequirement {
        asset_id: string_field(asset, "asset_id"),
        name: string_field(asset, "name"),
        asset_type: string_field(asset, "asset_type"),
        source: non_empty_or(
            string_field(asset, "source"),
            string_array(asset.get("source_refs"))
                .first()
                .map(String::as_str)
                .unwrap_or("stage_04/asset_strategy_matrix.json"),
        ),
        source_entity_id: non_empty_or(
            string_field(asset, "source_entity_id"),
            &string_field(asset, "consumer_system"),
        ),
        source_node_id: string_field(asset, "source_node_id"),
        purpose: string_field(asset, "purpose"),
        dependencies: string_array(asset.get("dependencies")),
        unlocks: string_array(asset.get("unlocks")),
        priority: non_empty_or(string_field(asset, "priority"), "P0"),
        complexity: non_empty_or(string_field(asset, "complexity"), "m"),
        required_for_phase: non_empty_or(
            string_field(asset, "required_for_phase"),
            "core_playable",
        ),
        status: non_empty_or(string_field(asset, "status"), "requirement_defined"),
        trace_kind: non_empty_or(string_field(asset, "trace_kind"), "asset_strategy_matrix"),
        resolution: string_field(asset, "resolution"),
    }
}

fn build_asset_requirements_resolved(registry: &Value, locale: ArtifactLocale) -> Value {
    let mut resolved = Vec::new();
    let mut unresolved = Vec::new();
    for asset in value_array(registry.get("assets")) {
        let asset_id = string_field(&asset, "asset_id");
        let source_refs = string_array(asset.get("source_refs"));
        let target = string_field(&asset, "unity_target_path");
        let consumer = string_field(&asset, "consumer_system");
        let mount = string_field(&asset, "mount_point");
        let missing = [
            ("asset_id", asset_id.is_empty()),
            ("source_contract_refs", source_refs.is_empty()),
            ("unity_target_path", target.is_empty()),
            ("consumer_system", consumer.is_empty()),
            ("mount_point", mount.is_empty()),
        ]
        .into_iter()
        .filter_map(|(field, missing)| missing.then_some(field))
        .collect::<Vec<_>>();
        if missing.is_empty() {
            let mut item = asset.clone();
            item["source_contract_refs"] = json!(source_refs);
            item["fallback_policy"] = json!(non_empty_or(
                string_field(&item, "fallback_policy"),
                &string_field(&item, "fallback_strategy"),
            ));
            resolved.push(item);
        } else {
            let reason = if locale == ArtifactLocale::ZhCn {
                format!("资产契约缺少字段：{}。", missing.join("、"))
            } else {
                format!("Asset contract is missing fields: {}.", missing.join(", "))
            };
            unresolved.push(json!({
                "asset_id": non_empty_or(asset_id, "UNKNOWN-ASSET"),
                "reason": reason,
                "message": reason,
                "severity": "blocking",
                "code": "ASSET_REQUIREMENT_UNRESOLVED",
                "missing_fields": missing,
            }));
        }
    }
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "source_refs": [
            "stage_02/playable_contracts/asset_mount_contract.json",
            "stage_04/asset_registry.json",
            "stage_04/asset_strategy_matrix.json"
        ],
        "resolved_assets": resolved,
        "unresolved_assets": unresolved,
    })
}

fn build_unity_asset_mount_plan(resolved: &Value, locale: ArtifactLocale) -> Value {
    let mount_items = value_array(resolved.get("resolved_assets"))
        .iter()
        .map(|asset| {
            let asset_type = string_field(asset, "asset_type");
            json!({
                "asset_id": string_field(asset, "asset_id"),
                "asset_type": asset_type,
                "unity_target_path": string_field(asset, "unity_target_path"),
                "mount_point": string_field(asset, "mount_point"),
                "consumer_system": string_field(asset, "consumer_system"),
                "import_settings": asset.get("import_settings").cloned().unwrap_or_else(|| default_import_settings(&asset_type)),
                "fallback_policy": non_empty_or(string_field(asset, "fallback_policy"), &string_field(asset, "fallback_strategy")),
                "acceptance_check": string_field(asset, "acceptance_check"),
                "source_refs": string_array(asset.get("source_contract_refs")),
            })
        })
        .collect::<Vec<_>>();
    let blockers = value_array(resolved.get("unresolved_assets"))
        .into_iter()
        .filter(|item| string_field(item, "severity") == "blocking")
        .collect::<Vec<_>>();
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "mount_items": mount_items,
        "blockers": blockers,
        "source_refs": ["stage_04/asset_requirements_resolved.json"],
    })
}

fn build_audio_placeholder_plan(registry: &Value, locale: ArtifactLocale) -> Value {
    let audio_assets = value_array(registry.get("assets"))
        .into_iter()
        .filter(|asset| string_field(asset, "asset_type") == "audio_placeholder")
        .collect::<Vec<_>>();
    let placeholder_files = audio_assets
        .iter()
        .map(|asset| {
            json!({
                "placeholder_id": string_field(asset, "asset_id"),
                "path": string_field(asset, "unity_target_path"),
                "consumer_system": string_field(asset, "consumer_system"),
                "replace_later": true,
                "acceptance_check": string_field(asset, "acceptance_check"),
            })
        })
        .collect::<Vec<_>>();
    let replacement_requirements = audio_assets
        .iter()
        .map(|asset| {
            json!({
                "placeholder_id": string_field(asset, "asset_id"),
                "audio_role": non_empty_or(string_field(asset, "asset_role"), &string_field(asset, "name")),
                "future_provider_requirement": localized_text(
                    locale,
                    "接入音频生成供应方后，用可追溯的最终音频替换静音标记。",
                    "Replace the silent marker with traceable final audio after an audio provider is connected.",
                ),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "source_refs": [
            "stage_02/playable_contracts/audio_requirements_contract.json",
            "stage_04/asset_registry.json"
        ],
        "placeholder_files": placeholder_files,
        "replacement_requirements": replacement_requirements,
    })
}

fn align_art_contract_with_registry(contract: &mut Value, registry: &Value) {
    let registry_assets = value_array(registry.get("assets"));
    let path_bindings =
        if let Some(contract_assets) = contract.get_mut("assets").and_then(Value::as_array_mut) {
            for contract_asset in contract_assets.iter_mut() {
                let asset_id = string_field(contract_asset, "asset_id");
                let Some(registry_asset) = registry_assets
                    .iter()
                    .find(|asset| string_field(asset, "asset_id") == asset_id)
                else {
                    continue;
                };
                let target = string_field(registry_asset, "unity_target_path");
                let consumer = string_field(registry_asset, "consumer_system");
                let mount_point = string_field(registry_asset, "mount_point");
                let acceptance_check = string_field(registry_asset, "acceptance_check");
                let source_refs = string_array(registry_asset.get("source_refs"));
                contract_asset["source_system_ids"] = json!([consumer]);
                contract_asset["production_specs"]["target_path"] = json!(target);
                contract_asset["production_specs"]["unity_target_path"] = json!(target);
                contract_asset["production_specs"]["consumer_system"] = json!(consumer);
                contract_asset["production_specs"]["mount_point"] = json!(mount_point);
                contract_asset["production_specs"]["fallback_policy"] = json!(non_empty_or(
                    string_field(registry_asset, "fallback_policy"),
                    &string_field(registry_asset, "fallback_strategy"),
                ));
                contract_asset["production_specs"]["acceptance_check"] = json!(acceptance_check);
                contract_asset["production_specs"]["source_refs"] = json!(source_refs);
                contract_asset["acceptance_checks"] = json!([acceptance_check]);
            }
            Some(art_path_bindings(contract_assets))
        } else {
            None
        };
    if let Some(path_bindings) = path_bindings {
        contract["path_bindings"] = json!(path_bindings);
    }
}

fn localize_art_pipeline_document(document: &mut Value, locale: ArtifactLocale) {
    annotate_artifact_locale(document, locale);
    if locale != ArtifactLocale::ZhCn {
        return;
    }
    if document.get("policy").is_some() {
        document["policy"] =
            json!("本程序仅生成切片元数据；Unity TextureImporter 在重新导入时执行切片。");
    }
    for requirement in document
        .get_mut("requirements")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if requirement.get("future_audio_requirement").is_some() {
            requirement["future_audio_requirement"] =
                json!("后续接入音频生成供应方后，用正式音频替换此静音标记。");
        }
    }
}

fn build_program_semantic_review_report(
    capability_contract: &Value,
    semantic_coverage: &Value,
    locale: ArtifactLocale,
) -> Value {
    let capabilities = value_array(capability_contract.get("capabilities"));
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    if capabilities.is_empty() {
        blockers.push(protocol_issue(
            "PROGRAM_CAPABILITY_CONTRACT_MISSING",
            "BLOCKER",
            "03",
            "stage_03/program_capability_contract.json",
            "capabilities",
            locale,
            (
                "缺少程序能力契约",
                "步骤 03 未提供可评审的程序能力。",
                "请重新运行步骤 03，生成程序能力契约。",
            ),
            (
                "Program capability contract missing",
                "Step03 did not provide program capabilities for review.",
                "Rerun Step03 to generate the program capability contract.",
            ),
        ));
    }
    for raw in value_array(semantic_coverage.get("blockers")) {
        blockers.push(protocol_issue_from_semantic(
            &raw,
            "03",
            "stage_03/program_semantic_coverage_report.json",
            locale,
        ));
    }
    let coverage = semantic_coverage
        .get("coverage")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    if !capabilities.is_empty() && coverage < 1.0 && blockers.is_empty() {
        warnings.push(protocol_issue(
            "PROGRAM_SEMANTIC_COVERAGE_INCOMPLETE",
            "WARNING",
            "03",
            "stage_03/program_semantic_coverage_report.json",
            "coverage",
            locale,
            (
                "程序语义覆盖尚未完整",
                "部分项目 DNA 语义尚未绑定到程序能力。",
                "请检查缺失能力，并在步骤 03 补齐绑定。",
            ),
            (
                "Program semantic coverage incomplete",
                "Some Project DNA semantics are not bound to program capabilities.",
                "Inspect missing capabilities and complete the Step03 bindings.",
            ),
        ));
    }
    let reviews = capabilities
        .iter()
        .map(|capability| {
            json!({
                "capability_id": string_field(capability, "capability_id"),
                "program_class": string_field(capability, "program_class"),
                "status": "covered",
                "message": localized_text(
                    locale,
                    "程序能力已绑定来源语义和验收引用。",
                    "Program capability is bound to source semantics and acceptance refs.",
                ),
            })
        })
        .collect::<Vec<_>>();
    let status = if blockers.is_empty() {
        if warnings.is_empty() {
            "passed"
        } else {
            "passed_with_review"
        }
    } else {
        "blocked"
    };
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "status": status,
        "project_signature": string_field(capability_contract, "project_signature"),
        "source_refs": [
            "stage_02/project_dna_contract.json",
            "stage_03/program_capability_contract.json",
            "stage_03/program_semantic_coverage_report.json"
        ],
        "reviews": reviews,
        "blockers": blockers,
        "warnings": warnings,
        "rework_items": blockers.iter().chain(warnings.iter()).cloned().collect::<Vec<_>>(),
        "coverage": coverage,
    })
}

fn build_art_semantic_review_report(
    asset_strategy: &Value,
    resolved_assets: &Value,
    mount_plan: &Value,
    locale: ArtifactLocale,
) -> Value {
    let strategies = value_array(asset_strategy.get("assets"));
    let resolved = value_array(resolved_assets.get("resolved_assets"));
    let mounts = value_array(mount_plan.get("mount_items"));
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    if strategies.is_empty() {
        blockers.push(protocol_issue(
            "ASSET_STRATEGY_MATRIX_MISSING",
            "BLOCKER",
            "04",
            "stage_04/asset_strategy_matrix.json",
            "assets",
            locale,
            (
                "缺少资产策略矩阵",
                "步骤 04 未提供可评审的项目资产策略。",
                "请重新运行步骤 04，生成资产策略矩阵。",
            ),
            (
                "Asset strategy matrix missing",
                "Step04 did not provide a project asset strategy for review.",
                "Rerun Step04 to generate the asset strategy matrix.",
            ),
        ));
    }
    for unresolved in value_array(resolved_assets.get("unresolved_assets")) {
        let severity = if string_field(&unresolved, "severity") == "blocking" {
            "BLOCKER"
        } else {
            "WARNING"
        };
        let item = protocol_issue(
            "ASSET_REQUIREMENT_UNRESOLVED",
            severity,
            "04",
            "stage_04/asset_requirements_resolved.json",
            &string_field(&unresolved, "asset_id"),
            locale,
            (
                "资产需求尚未解析",
                "资产需求缺少可消费路径、使用方或挂载信息。",
                "请在步骤 04 补齐资产目标和挂载契约。",
            ),
            (
                "Asset requirement unresolved",
                "An asset requirement lacks a consumable path, consumer, or mount information.",
                "Complete the asset target and mount contract in Step04.",
            ),
        );
        if severity == "BLOCKER" {
            blockers.push(item);
        } else {
            warnings.push(item);
        }
    }
    for raw in value_array(mount_plan.get("blockers")) {
        let asset_id = string_field(&raw, "asset_id");
        if blockers.iter().any(|item| {
            string_field(item, "code") == "ASSET_REQUIREMENT_UNRESOLVED"
                && string_field(item, "field") == asset_id
        }) {
            continue;
        }
        blockers.push(protocol_issue(
            "UNITY_ASSET_MOUNT_UNRESOLVED",
            "BLOCKER",
            "04",
            "stage_04/unity_asset_mount_plan.json",
            &asset_id,
            locale,
            (
                "Unity 资产挂载尚未解析",
                "资产尚无可执行的 Unity 挂载计划。",
                "请在步骤 04 修复目标路径、使用方和挂载点。",
            ),
            (
                "Unity asset mount unresolved",
                "An asset has no executable Unity mount plan.",
                "Fix the target path, consumer, and mount point in Step04.",
            ),
        ));
    }
    let reviews = strategies
        .iter()
        .map(|strategy| {
            let strategy_id = string_field(strategy, "strategy_id");
            let target = string_field(strategy, "unity_target_path");
            let asset = resolved.iter().find(|asset| {
                string_field(asset, "strategy_id") == strategy_id
                    || (!target.is_empty() && string_field(asset, "unity_target_path") == target)
            });
            let mounted = asset.is_some_and(|asset| {
                let id = string_field(asset, "asset_id");
                mounts
                    .iter()
                    .any(|mount| string_field(mount, "asset_id") == id)
            });
            if !mounted {
                warnings.push(protocol_issue(
                    "ASSET_STRATEGY_NOT_MOUNT_READY",
                    "WARNING",
                    "04",
                    "stage_04/unity_asset_mount_plan.json",
                    &strategy_id,
                    locale,
                    (
                        "资产策略尚未准备挂载",
                        "资产策略尚未关联到完整的 Unity 挂载项。",
                        "请检查资产解析结果和 Unity 挂载计划。",
                    ),
                    (
                        "Asset strategy is not mount-ready",
                        "An asset strategy is not linked to a complete Unity mount item.",
                        "Inspect the resolved assets and Unity mount plan.",
                    ),
                ));
            }
            json!({
                "strategy_id": strategy_id,
                "asset_role": string_field(strategy, "asset_role"),
                "status": if mounted { "mount_ready" } else { "review_required" },
                "message": if mounted {
                    localized_text(locale, "资产角色、目标路径和挂载点已对齐。", "Asset role, target path, and mount point are aligned.")
                } else {
                    localized_text(locale, "资产策略尚未形成完整的挂载链路。", "Asset strategy does not yet have a complete mount chain.")
                }
            })
        })
        .collect::<Vec<_>>();
    let status = if blockers.is_empty() {
        if warnings.is_empty() {
            "passed"
        } else {
            "passed_with_review"
        }
    } else {
        "blocked"
    };
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "status": status,
        "source_refs": [
            "stage_04/asset_strategy_matrix.json",
            "stage_04/asset_requirements_resolved.json",
            "stage_04/unity_asset_mount_plan.json"
        ],
        "reviews": reviews,
        "blockers": blockers,
        "warnings": warnings,
        "rework_items": blockers.iter().chain(warnings.iter()).cloned().collect::<Vec<_>>(),
    })
}

fn protocol_issue_from_semantic(
    raw: &Value,
    return_target: &str,
    artifact: &str,
    locale: ArtifactLocale,
) -> Value {
    let code = non_empty_or(string_field(raw, "code"), "SEMANTIC_COVERAGE_BLOCKED");
    let (zh_cn, en_us) = match code.as_str() {
        "ACTION_HAS_NO_STATE_CHANGE" => (
            (
                "玩家动作缺少状态变化",
                "玩家动作没有声明可观察的状态变化。",
                "请在步骤 02 的项目 DNA 中补齐动作状态变化。",
            ),
            (
                "Player action has no state change",
                "A player action does not declare an observable state change.",
                "Add the action state change to the Step02 Project DNA.",
            ),
        ),
        "OBJECTIVE_HAS_NO_COMPLETION_CONDITION" => (
            (
                "目标缺少完成条件",
                "目标没有声明完成条件或失败条件。",
                "请在步骤 02 的项目 DNA 中补齐目标条件。",
            ),
            (
                "Objective has no completion condition",
                "An objective declares neither a completion nor a failure condition.",
                "Add objective conditions to the Step02 Project DNA.",
            ),
        ),
        _ => (
            (
                "程序语义覆盖不足",
                "必需的项目 DNA 语义尚未完全绑定到程序能力。",
                "请返回步骤 03，补齐程序能力和来源绑定。",
            ),
            (
                "Program semantic coverage is insufficient",
                "Required Project DNA semantics are not fully bound to program capabilities.",
                "Return to Step03 and complete program capabilities and source bindings.",
            ),
        ),
    };
    protocol_issue(
        &code,
        "BLOCKER",
        return_target,
        artifact,
        &non_empty_or(
            string_field(raw, "capability_id"),
            &non_empty_or(
                string_field(raw, "action_id"),
                &string_field(raw, "objective_id"),
            ),
        ),
        locale,
        zh_cn,
        en_us,
    )
}

#[allow(clippy::too_many_arguments)]
fn protocol_issue(
    code: &str,
    severity: &str,
    return_target: &str,
    artifact: &str,
    field: &str,
    locale: ArtifactLocale,
    zh_cn: (&str, &str, &str),
    en_us: (&str, &str, &str),
) -> Value {
    let (title, reason, suggestion) = if locale == ArtifactLocale::ZhCn {
        zh_cn
    } else {
        en_us
    };
    json!({
        "code": code,
        "severity": severity,
        "title": title,
        "message": reason,
        "reason": reason,
        "suggestion": suggestion,
        "artifact": artifact,
        "field": field,
        "return_target": return_target,
    })
}

#[derive(Debug, Clone, Default)]
pub struct Step03OutputGenerator;

impl StageOutputGenerator for Step03OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP03)?;
        let locale = artifact_locale_from_inputs(structured_inputs);
        let entities = preferred_design_entities(parsed, structured_inputs);
        let mut requirements =
            EntityToRequirementConverter.convert_entities_with_locale(&entities, locale);
        let system_graph = system_graph_from_parsed(parsed, locale);
        SystemBinder.bind(&mut requirements, &system_graph);
        let binding_stats = build_requirement_quality_report_with_locale(&requirements, locale);
        let structure_spec = program_structure_spec(&requirements, locale);
        let trace_report = program_requirement_trace_report(&requirements, &binding_stats, locale);
        let resource_graph = resource_graph_from_requirements(&requirements);
        let required_contracts = required_contracts();
        let (project_dna, mut upstream_blockers) = load_project_dna_contract(out_dir, "03", locale);
        let mut capability_contract = build_program_capability_contract(&project_dna);
        localize_program_capability_contract(&mut capability_contract, locale);
        let mut semantic_coverage =
            build_program_semantic_coverage_report(&project_dna, &capability_contract);
        localize_program_semantic_coverage_report(&mut semantic_coverage, locale);
        upstream_blockers.extend(value_array(semantic_coverage.get("blockers")));
        upstream_blockers.extend(value_array(structure_spec.get("preflight_blocking_issues")));
        let trace_blockers = value_array(trace_report.get("blockers"));
        let mut contract = build_program_requirements_contract(ProgramContractInput {
            generated_at: &now_iso(),
            parsed,
            requirements: &requirements,
            structure_spec: &structure_spec,
            system_graph: &system_graph,
            resource_graph: &resource_graph,
            binding_stats: &binding_stats,
            required_contracts: &required_contracts,
            contract_blockers: &[],
            trace_blockers: &trace_blockers,
            preflight_blockers: &upstream_blockers,
            locale,
        });
        contract["program_capability_contract"] =
            json!("stage_03/program_capability_contract.json");
        contract["program_semantic_coverage_report"] =
            json!("stage_03/program_semantic_coverage_report.json");
        contract["capability_summary"] = json!({
            "capability_count": value_array(capability_contract.get("capabilities")).len(),
            "semantic_status": string_field(&semantic_coverage, "status"),
            "semantic_coverage": semantic_coverage.get("coverage").cloned().unwrap_or_else(|| json!(0.0)),
        });
        let contract_valid = contract
            .get("valid")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut issues = contract_review_items(&contract, "03", locale);
        if !contract_valid && !has_blocking_review_item(&issues) {
            issues.push(contract_validity_blocker(
                "PROGRAM_REQUIREMENTS_MISSING",
                "03",
                locale,
            ));
        }
        let blocking_issues = blocking_review_items(&issues);
        let review_items = non_blocking_review_items(&issues);
        let warnings = warning_items(&review_items);
        let ready = contract_valid && blocking_issues.is_empty() && review_items.is_empty();
        let status = if !blocking_issues.is_empty() {
            "blocked"
        } else if ready {
            "success"
        } else {
            "completed_with_review"
        };
        let return_targets = blocking_issues
            .iter()
            .chain(review_items.iter())
            .cloned()
            .collect::<Vec<_>>();
        let mut customization_report = build_customization_score_report(
            "03",
            Some(&project_dna),
            if status == "blocked" {
                "blocked"
            } else {
                "passed"
            },
            &blocking_issues,
            &warnings,
            Some(json!({
                "requirement_traceability": binding_stats.get("traceability_rate").cloned().unwrap_or_else(|| json!(0.0)),
                "system_binding": binding_stats.get("system_binding_rate").cloned().unwrap_or_else(|| json!(0.0)),
                "semantic_coverage": semantic_coverage.get("coverage").cloned().unwrap_or_else(|| json!(0.0)),
            })),
        );
        customization_report["artifact_locale"] = json!(locale);
        write_json(
            &out_dir.join("program_requirements.json"),
            &to_json_value(&requirements)?,
        )?;
        write_json(
            &out_dir.join("program_structure_spec.json"),
            &structure_spec,
        )?;
        write_json(
            &out_dir.join("program_requirement_trace_report.json"),
            &trace_report,
        )?;
        write_json(
            &out_dir.join("program_capability_contract.json"),
            &capability_contract,
        )?;
        write_json(
            &out_dir.join("program_semantic_coverage_report.json"),
            &semantic_coverage,
        )?;
        write_json(
            &out_dir.join("customization_score_report.json"),
            &customization_report,
        )?;
        write_json(
            &out_dir.join("program_requirements_contract.json"),
            &contract,
        )?;
        Ok(json!({
            "status": status,
            "artifact_locale": locale,
            "content_exists": true,
            "traceability_valid": requirements.iter().all(|item| !item.source_refs.is_empty()),
            "program_requirements_contract": "program_requirements_contract.json",
            "program_structure_spec": "program_structure_spec.json",
            "requirement_count": requirements.len(),
            "system_binding_rate": binding_stats.get("system_binding_rate").cloned().unwrap_or(Value::Null),
            "structured_input_status": structured_inputs.get("status").and_then(Value::as_str).unwrap_or("fallback_to_markdown"),
            "review_items_count": review_items.len(),
            "review_items": review_items,
            "blocking_issues": blocking_issues,
            "warnings": warnings,
            "message": if ready {
                localized_text(locale, "步骤 03 已成功生成程序需求。", "Step03 generated program requirements successfully.").to_string()
            } else if status == "blocked" && locale == ArtifactLocale::ZhCn {
                "步骤 03 的程序需求存在阻断项。".to_string()
            } else if status == "blocked" {
                "Step03 program requirements contain blocking issues.".to_string()
            } else if locale == ArtifactLocale::ZhCn {
                format!("步骤 03 已生成程序需求，但有 {} 个项目需要复核。", review_items.len())
            } else {
                format!("Step03 generated program requirements with {} review item(s).", review_items.len())
            },
            "semantic_quality": {
                "status": if status == "blocked" { "blocked" } else if ready { "success" } else { "warning" },
                "return_targets": return_targets,
            },
        }))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Step04OutputGenerator;

impl StageOutputGenerator for Step04OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP04)?;
        let locale = artifact_locale_from_inputs(structured_inputs);
        let entities = preferred_design_entities(parsed, structured_inputs);
        let base_assets = EntityToAssetConverter.convert_entities_with_locale(&entities, locale);
        let market = MarketResearchSkill.local_fallback_with_locale(parsed, locale);
        let base_asset_spec = build_asset_spec_contract_with_locale(&base_assets, locale);
        let (project_dna, mut contract_blockers) = load_project_dna_contract(out_dir, "04", locale);
        let archetype_requirements = load_archetype_requirements(out_dir, structured_inputs);
        let (mut art_taxonomy, mut asset_strategy) = build_art_taxonomy_and_strategy(
            &project_dna,
            &archetype_requirements,
            Some(&base_asset_spec),
        );
        annotate_artifact_locale(&mut art_taxonomy, locale);
        annotate_artifact_locale(&mut asset_strategy, locale);
        let base_registry = asset_registry_document(&base_assets, &base_asset_spec, locale);
        let base_registry_assets = value_array(base_registry.get("assets"));
        let merged_assets =
            merge_asset_strategy_into_assets(&base_registry_assets, &asset_strategy);
        let mut normalized_assets = normalize_asset_targets(&merged_assets);
        localize_and_complete_asset_values(&mut normalized_assets, locale);
        let registry = json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "artifact_locale": locale,
            "assets": normalized_assets,
        });
        let resolved_assets = build_asset_requirements_resolved(&registry, locale);
        contract_blockers.extend(
            value_array(resolved_assets.get("unresolved_assets"))
                .into_iter()
                .filter(|item| string_field(item, "severity") == "blocking"),
        );
        let mount_plan = build_unity_asset_mount_plan(&resolved_assets, locale);
        let audio_plan = build_audio_placeholder_plan(&registry, locale);
        let assets = normalized_assets
            .iter()
            .map(art_requirement_from_value)
            .collect::<Vec<_>>();
        let asset_spec = asset_spec_contract_from_values(&normalized_assets, locale);
        let asset_spec_gate = validate_asset_spec_contract_with_locale(&asset_spec, locale);
        let required_contracts = required_contracts();
        let mut contract = build_art_requirements_contract(ArtContractInput {
            generated_at: &now_iso(),
            parsed,
            assets: &assets,
            market_research: &market,
            asset_spec_gate: &asset_spec_gate,
            required_contracts: &required_contracts,
            contract_blockers: &contract_blockers,
            locale,
        });
        align_art_contract_with_registry(&mut contract, &registry);
        contract["art_taxonomy_contract"] = json!("stage_04/art_taxonomy_contract.json");
        contract["asset_strategy_matrix"] = json!("stage_04/asset_strategy_matrix.json");
        contract["asset_requirements_resolved"] =
            json!("stage_04/asset_requirements_resolved.json");
        contract["unity_import_policy"] = json!("stage_04/unity_import_policy.json");
        contract["source_files"]["art_taxonomy_contract"] =
            json!("stage_04/art_taxonomy_contract.json");
        contract["source_files"]["asset_strategy_matrix"] =
            json!("stage_04/asset_strategy_matrix.json");
        contract["source_files"]["asset_requirements_resolved"] =
            json!("stage_04/asset_requirements_resolved.json");
        contract["source_files"]["unity_import_policy"] =
            json!("stage_04/unity_import_policy.json");
        let contract_valid = contract
            .get("valid")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let mut issues = contract_review_items(&contract, "04", locale);
        if !contract_valid && !has_blocking_review_item(&issues) {
            issues.push(contract_validity_blocker(
                "ART_ASSETS_MISSING",
                "04",
                locale,
            ));
        }
        let blocking_issues = blocking_review_items(&issues);
        let review_items = non_blocking_review_items(&issues);
        let warnings = warning_items(&review_items);
        let ready = contract_valid && blocking_issues.is_empty() && review_items.is_empty();
        let status = if !blocking_issues.is_empty() {
            "blocked"
        } else if ready {
            "success"
        } else {
            "completed_with_review"
        };
        let return_targets = blocking_issues
            .iter()
            .chain(review_items.iter())
            .cloned()
            .collect::<Vec<_>>();
        let generated_at = now_iso();
        let mut image_spec = build_image_consumable_spec(
            registry
                .get("assets")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            &generated_at,
            value_array(resolved_assets.get("unresolved_assets"))
                .into_iter()
                .filter(|item| string_field(item, "severity") == "blocking")
                .collect(),
        );
        let mut ui_slice_spec = build_ui_slice_spec_contract(
            registry
                .get("assets")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            &generated_at,
        );
        let mut unity_import_policy = build_unity_import_policy(
            registry
                .get("assets")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            &generated_at,
        );
        let mut asset_usage_binding = build_asset_usage_binding_seed(
            registry
                .get("assets")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            &generated_at,
        );
        let mut audio_requirements = build_audio_placeholder_requirements(
            registry
                .get("assets")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
            &generated_at,
        );
        for document in [
            &mut image_spec,
            &mut ui_slice_spec,
            &mut unity_import_policy,
            &mut asset_usage_binding,
            &mut audio_requirements,
        ] {
            localize_art_pipeline_document(document, locale);
        }
        let mut customization_report = build_customization_score_report(
            "04",
            Some(&project_dna),
            if status == "blocked" {
                "blocked"
            } else {
                "passed"
            },
            &blocking_issues,
            &warnings,
            Some(json!({
                "asset_traceability": if assets.is_empty() { 0.0 } else { ratio(assets.iter().filter(|asset| !asset.source.is_empty()).count(), assets.len()) },
                "mount_readiness": if assets.is_empty() { 0.0 } else { ratio(value_array(resolved_assets.get("resolved_assets")).len(), assets.len()) },
                "taxonomy_category_count": value_array(art_taxonomy.get("asset_categories")).len(),
            })),
        );
        customization_report["artifact_locale"] = json!(locale);
        write_json(&out_dir.join("asset_registry.json"), &registry)?;
        write_json(&out_dir.join("market_research.json"), &market)?;
        write_json(&out_dir.join("asset_spec_contract.json"), &asset_spec)?;
        write_json(
            &out_dir.join("asset_spec_gate_report.json"),
            &asset_spec_gate,
        )?;
        write_json(&out_dir.join("art_requirements_contract.json"), &contract)?;
        write_json(
            &out_dir.join("asset_requirements_resolved.json"),
            &resolved_assets,
        )?;
        write_json(&out_dir.join("unity_asset_mount_plan.json"), &mount_plan)?;
        write_json(&out_dir.join("audio_placeholder_plan.json"), &audio_plan)?;
        write_json(&out_dir.join("image_consumable_spec.json"), &image_spec)?;
        write_json(&out_dir.join("ui_slice_spec_contract.json"), &ui_slice_spec)?;
        write_json(
            &out_dir.join("unity_import_policy.json"),
            &unity_import_policy,
        )?;
        write_json(
            &out_dir.join("asset_usage_binding_seed.json"),
            &asset_usage_binding,
        )?;
        write_json(
            &out_dir.join("audio_placeholder_requirements.json"),
            &audio_requirements,
        )?;
        write_json(&out_dir.join("art_taxonomy_contract.json"), &art_taxonomy)?;
        write_json(&out_dir.join("asset_strategy_matrix.json"), &asset_strategy)?;
        write_json(
            &out_dir.join("customization_score_report.json"),
            &customization_report,
        )?;
        Ok(json!({
            "status": status,
            "artifact_locale": locale,
            "content_exists": true,
            "traceability_valid": assets.iter().all(|item| !item.source.is_empty()),
            "art_requirements_contract": "art_requirements_contract.json",
            "asset_spec_contract": "asset_spec_contract.json",
            "asset_count": assets.len(),
            "asset_spec_valid": asset_spec_gate.get("valid").and_then(Value::as_bool).unwrap_or(false),
            "structured_input_status": structured_inputs.get("status").and_then(Value::as_str).unwrap_or("fallback_to_markdown"),
            "review_items_count": review_items.len(),
            "review_items": review_items,
            "blocking_issues": blocking_issues,
            "warnings": warnings,
            "message": if ready {
                localized_text(locale, "步骤 04 已成功生成美术需求。", "Step04 generated art requirements successfully.").to_string()
            } else if status == "blocked" && locale == ArtifactLocale::ZhCn {
                "步骤 04 的美术需求存在阻断项。".to_string()
            } else if status == "blocked" {
                "Step04 art requirements contain blocking issues.".to_string()
            } else if locale == ArtifactLocale::ZhCn {
                format!("步骤 04 已生成美术需求，但有 {} 个项目需要复核。", review_items.len())
            } else {
                format!("Step04 generated art requirements with {} review item(s).", review_items.len())
            },
            "semantic_quality": {
                "status": if status == "blocked" { "blocked" } else if ready { "success" } else { "warning" },
                "return_targets": return_targets,
            },
        }))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Step05OutputGenerator;

impl StageOutputGenerator for Step05OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        _parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP05)?;
        let locale = artifact_locale_from_inputs(structured_inputs);
        let requirements = load_program_requirements(out_dir).unwrap_or_default();
        let capability_contract =
            load_stage_artifact(out_dir, STEP03, "program_capability_contract.json")
                .unwrap_or_else(|| json!({}));
        let semantic_coverage =
            load_stage_artifact(out_dir, STEP03, "program_semantic_coverage_report.json")
                .unwrap_or_else(|| json!({}));
        let semantic_review =
            build_program_semantic_review_report(&capability_contract, &semantic_coverage, locale);
        let mut issues = IntelligentReviewer::new()
            .review_program_with_locale(&requirements, locale)
            .get("issues")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        issues.extend(value_array(semantic_review.get("blockers")));
        issues.extend(value_array(semantic_review.get("warnings")));
        let review = review_report("program_requirements", issues, locale);
        let (project_dna, _) = load_project_dna_contract(out_dir, "05", locale);
        let blockers = blocking_review_items(&value_array(review.get("issues")));
        let warnings = warning_items(&non_blocking_review_items(&value_array(
            review.get("issues"),
        )));
        let mut customization_report = build_customization_score_report(
            "05",
            Some(&project_dna),
            if blockers.is_empty() {
                "passed"
            } else {
                "blocked"
            },
            &blockers,
            &warnings,
            Some(json!({
                "requirement_review_coverage": if requirements.is_empty() { 0.0 } else { 1.0 },
                "semantic_coverage": semantic_coverage.get("coverage").cloned().unwrap_or_else(|| json!(0.0)),
            })),
        );
        customization_report["artifact_locale"] = json!(locale);
        write_json(&out_dir.join("program_ai_review_report.json"), &review)?;
        write_json(
            &out_dir.join("program_semantic_review_report.json"),
            &semantic_review,
        )?;
        write_json(
            &out_dir.join("customization_score_report.json"),
            &customization_report,
        )?;
        write_json(&out_dir.join("program_review_report.json"), &review)?;
        write_json(&out_dir.join("ProgReview_report.json"), &review)?;
        Ok(review_result(
            "program_ai_review_report.json",
            &review,
            structured_inputs,
        ))
    }
}

#[derive(Debug, Clone, Default)]
pub struct Step06OutputGenerator;

impl StageOutputGenerator for Step06OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        _parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP06)?;
        let locale = artifact_locale_from_inputs(structured_inputs);
        let assets = load_art_requirements(out_dir).unwrap_or_default();
        let asset_strategy = load_stage_artifact(out_dir, STEP04, "asset_strategy_matrix.json")
            .unwrap_or_else(|| json!({}));
        let resolved_assets =
            load_stage_artifact(out_dir, STEP04, "asset_requirements_resolved.json")
                .unwrap_or_else(|| json!({}));
        let mount_plan = load_stage_artifact(out_dir, STEP04, "unity_asset_mount_plan.json")
            .unwrap_or_else(|| json!({}));
        let semantic_review = build_art_semantic_review_report(
            &asset_strategy,
            &resolved_assets,
            &mount_plan,
            locale,
        );
        let mut issues = IntelligentReviewer::new()
            .review_art_with_locale(&assets, locale)
            .get("issues")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        issues.extend(value_array(semantic_review.get("blockers")));
        issues.extend(value_array(semantic_review.get("warnings")));
        let review = review_report("art_requirements", issues, locale);
        let (project_dna, _) = load_project_dna_contract(out_dir, "06", locale);
        let blockers = blocking_review_items(&value_array(review.get("issues")));
        let warnings = warning_items(&non_blocking_review_items(&value_array(
            review.get("issues"),
        )));
        let mut customization_report = build_customization_score_report(
            "06",
            Some(&project_dna),
            if blockers.is_empty() {
                "passed"
            } else {
                "blocked"
            },
            &blockers,
            &warnings,
            Some(json!({
                "asset_review_coverage": if assets.is_empty() { 0.0 } else { 1.0 },
                "mount_readiness": if assets.is_empty() { 0.0 } else { ratio(value_array(mount_plan.get("mount_items")).len(), assets.len()) },
            })),
        );
        customization_report["artifact_locale"] = json!(locale);
        write_json(&out_dir.join("art_ai_review_report.json"), &review)?;
        write_json(
            &out_dir.join("art_semantic_review_report.json"),
            &semantic_review,
        )?;
        write_json(
            &out_dir.join("customization_score_report.json"),
            &customization_report,
        )?;
        write_json(&out_dir.join("art_review_report.json"), &review)?;
        write_json(&out_dir.join("ArtReview_report.json"), &review)?;
        Ok(review_result(
            "art_ai_review_report.json",
            &review,
            structured_inputs,
        ))
    }
}

pub fn generator_for_step(step_number: u32) -> AdmResult<Box<dyn StageOutputGenerator>> {
    match step_number {
        STEP03 => Ok(Box::new(Step03OutputGenerator)),
        STEP04 => Ok(Box::new(Step04OutputGenerator)),
        STEP05 => Ok(Box::new(Step05OutputGenerator)),
        STEP06 => Ok(Box::new(Step06OutputGenerator)),
        other => Err(AdmError::new(format!(
            "Step03-06 generator cannot handle stage {other:02}"
        ))),
    }
}

fn route_for_entity(entity: &DesignEntity, locale: ArtifactLocale) -> String {
    let text = format!("{} {} {}", entity.kind, entity.schema, entity.label).to_lowercase();
    for (token, zh_cn, en_us) in [
        (
            "character",
            "角色行为、状态与交互",
            "character behavior, state, and interaction",
        ),
        (
            "enemy",
            "敌人行为、攻击模式与生成条件",
            "enemy behavior, attack pattern, and spawn condition",
        ),
        (
            "weapon",
            "武器输入、命中结算、伤害与反馈",
            "weapon input, hit resolution, damage, and feedback",
        ),
        (
            "ability",
            "能力触发、效果、冷却与组合规则",
            "ability trigger, effect, cooldown, and composition rules",
        ),
        (
            "room",
            "房间生成、遭遇配置与出口规则",
            "room generation, encounter config, and exit rules",
        ),
        (
            "resource",
            "资源产出、消耗、存储与显示",
            "resource production, spending, storage, and display",
        ),
        (
            "ui",
            "界面状态、输入反馈与信息层级",
            "UI state, input feedback, and information hierarchy",
        ),
        (
            "scene",
            "场景视觉、环境交互与氛围",
            "scene visuals, environment interaction, and mood",
        ),
        (
            "config",
            "配置参数、数据表与平衡接口",
            "configuration parameters, data tables, and balance interfaces",
        ),
        (
            "audio",
            "音频触发条件、音量控制与混音规则",
            "audio trigger conditions, volume control, and mix rules",
        ),
        (
            "system",
            "系统初始化、事件与模块通信",
            "system initialization, events, and module communication",
        ),
        (
            "narrative",
            "叙事触发条件、对话树与剧情状态",
            "narrative trigger conditions, dialogue tree, and story state",
        ),
    ] {
        if text.contains(token) {
            return localized_text(locale, zh_cn, en_us).to_string();
        }
    }
    localized_text(
        locale,
        "通用数据、行为与验收规则",
        "generic data, behavior, and acceptance rules",
    )
    .to_string()
}

fn requirement_templates(
    entity: &DesignEntity,
    route: &str,
    locale: ArtifactLocale,
) -> Vec<(&'static str, String)> {
    let text = |zh_cn: &str, en_us: &str| localized_text(locale, zh_cn, en_us).to_string();
    match entity.kind.to_lowercase().as_str() {
        "weapon" => vec![
            (
                "input_response",
                text(
                    "输入响应、攻击触发与操作手感反馈",
                    "input response, attack trigger, and feel feedback",
                ),
            ),
            (
                "hit_resolution",
                text(
                    "命中检测、伤害计算与击退效果",
                    "hit detection, damage calculation, and knockback effect",
                ),
            ),
            (
                "visual_audio",
                text(
                    "攻击动画、命中特效与音频触发",
                    "attack animation, hit VFX, and audio trigger",
                ),
            ),
        ],
        "ability" => vec![
            (
                "trigger_cooldown",
                text(
                    "施放条件、冷却管理与资源消耗",
                    "cast conditions, cooldown management, and resource cost",
                ),
            ),
            (
                "effect_execution",
                text(
                    "目标选择、效果计算与状态施加",
                    "targeting, effect calculation, and status application",
                ),
            ),
            (
                "visual_feedback",
                text(
                    "施放动画、命中特效与界面图标更新",
                    "cast animation, hit VFX, and UI icon update",
                ),
            ),
        ],
        "character" => vec![
            (
                "attribute_init",
                text(
                    "属性数据结构、基础数值与成长曲线",
                    "attribute data structure, base values, and growth curve",
                ),
            ),
            (
                "state_behavior",
                text(
                    "状态机、行为驱动与决策逻辑",
                    "state machine, behavior driver, and decision logic",
                ),
            ),
            (
                "damage_lifecycle",
                text(
                    "受击反馈、死亡处理与重生或复活流程",
                    "hit reaction, death handling, and respawn or revive flow",
                ),
            ),
        ],
        "room" => vec![
            (
                "generation_rules",
                text(
                    "生成规则、遭遇配置与出口逻辑",
                    "generation rules, encounter configuration, and exit logic",
                ),
            ),
            (
                "path_choice",
                text(
                    "房间路线选择、状态记录与奖励交接",
                    "room path choice, state recording, and reward handoff",
                ),
            ),
        ],
        _ => vec![("core_behavior", route.to_string())],
    }
}

fn output_for_entity(entity: &DesignEntity, suffix: &str) -> String {
    let schema = if entity.schema.is_empty() {
        "entity".to_string()
    } else {
        entity.schema.replace('.', "_")
    };
    let label = non_empty_or(entity.label.replace(' ', "_"), &entity.entity_id);
    format!("{schema}/{label}.{suffix}.asset")
}

fn phase_for_entity(entity: &DesignEntity) -> String {
    let text = format!(
        "{} {} {} {}",
        entity.kind, entity.schema, entity.label, entity.node_id
    )
    .to_lowercase();
    if contains_any(
        &text,
        &[
            "release",
            "launch",
            "analytics",
            "telemetry",
            "发布",
            "上线",
            "埋点",
        ],
    ) {
        "launch_ops"
    } else if contains_any(
        &text,
        &["social", "guild", "friend", "社交", "好友", "公会"],
    ) {
        "social"
    } else if contains_any(&text, &["resource", "currency", "economy", "资源", "货币"]) {
        "economy"
    } else if contains_any(
        &text,
        &["upgrade", "progress", "unlock", "升级", "成长", "解锁"],
    ) {
        "progression"
    } else if contains_any(&text, &["room", "enemy", "encounter", "房间", "敌人"]) {
        "content_ops"
    } else {
        "core_playable"
    }
    .to_string()
}

fn system_graph_from_parsed(parsed: &ParsedDesignSource, locale: ArtifactLocale) -> Value {
    let mut nodes = Vec::new();
    let mut seen = BTreeSet::new();
    for selection in &parsed.selections {
        let haystack = format!(
            "{} {} {}",
            selection.item_type, selection.option, selection.layer_title
        )
        .to_lowercase();
        if !haystack.contains("system") && !haystack.contains("系统") && !haystack.contains("玩法")
        {
            continue;
        }
        let id = selection
            .dependencies
            .first()
            .cloned()
            .unwrap_or_else(|| format!("SYS-{:03}", nodes.len() + 1));
        if seen.insert(id.clone()) {
            nodes.push(json!({
                "id": id,
                "system_id": id,
                "node_id": id,
                "name": non_empty_or(selection.option.clone(), &selection.item_type),
                "system_name": non_empty_or(selection.option.clone(), &selection.item_type),
                "responsibility": selection.purpose,
                "source": selection.source_ref,
            }));
        }
    }
    if nodes.is_empty() {
        nodes.push(json!({
            "id": "SYS-CORE",
            "system_id": "SYS-CORE",
            "node_id": "core_system",
            "name": localized_text(locale, "核心玩法系统", "Core Gameplay System"),
            "system_name": localized_text(locale, "核心玩法系统", "Core Gameplay System"),
            "responsibility": localized_text(
                locale,
                "实现可追溯到来源的核心玩法行为。",
                "Implement source-traced core gameplay behavior.",
            ),
            "source": parsed.source,
        }));
    }
    json!({"artifact_locale": locale, "nodes": nodes, "edges": []})
}

fn resource_graph_from_requirements(requirements: &[ProgramRequirement]) -> Value {
    json!({
        "resources": requirements
            .iter()
            .filter(|requirement| requirement.entity_kind == "resource")
            .map(|requirement| json!({
                "id": requirement.entity_id,
                "name": requirement.entity_label,
                "source": requirement.source_refs.first().cloned().unwrap_or_default(),
            }))
            .collect::<Vec<_>>()
    })
}

fn program_structure_spec(requirements: &[ProgramRequirement], locale: ArtifactLocale) -> Value {
    let preflight_blocking_issues = requirements
        .iter()
        .filter(|requirement| requirement.source_refs.is_empty())
        .map(|requirement| {
            issue(
                "SOURCE_TRACE_MISSING",
                "BLOCKER",
                "stage_03",
                "program_structure_spec.json",
                &requirement.id,
                localized_text(
                    locale,
                    "无法为缺少来源引用的程序需求分配写入路径。",
                    "A write path cannot be assigned to a program requirement without source refs.",
                ),
                localized_text(
                    locale,
                    "请先在步骤 03 的程序需求中补齐步骤 02 来源引用。",
                    "Add Step02 source refs to the Step03 program requirement first.",
                ),
                locale,
            )
        })
        .collect::<Vec<_>>();
    let system_path_map = requirements
        .iter()
        .map(|requirement| {
            let module = requirement
                .system_ids
                .first()
                .cloned()
                .unwrap_or_else(|| "SYS-UNBOUND".to_string());
            let module_slug = safe_name(&module);
            let requirement_slug = safe_name(&requirement.id);
            let target_path =
                format!("Assets/Scripts/AutoDesign/{module_slug}/{requirement_slug}.cs");
            let test_path =
                format!("Assets/Tests/AutoDesign/{module_slug}/{requirement_slug}Tests.cs");
            json!({
                "requirement_id": requirement.id,
                "selection_id": requirement.selection_id,
                "phase": requirement.phase,
                "module": module,
                "target_path": target_path,
                "test_path": test_path,
                "allowed_write_paths": [target_path, test_path],
                "source_refs": requirement.source_refs,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "valid": !requirements.is_empty() && preflight_blocking_issues.is_empty(),
        "project_type": "unity_game",
        "allowed_roots": ["Assets/AutoDesign", "Assets/Scripts/AutoDesign"],
        "preflight_blocking_issues": preflight_blocking_issues,
        "system_path_map": system_path_map,
        "path_binding_contract": {
            "required_task_fields": [
                "requirement_id",
                "phase",
                "module",
                "target_path",
                "test_path",
                "source_refs"
            ],
            "required_topology_fields": [
                "allowed_roots",
                "system_path_map",
                "path_binding_contract"
            ]
        },
        "output_file_rules": [
            "all_generated_program_files_must_stay_under_allowed_roots",
            "each_requirement_must_define_a_test_path"
        ]
    })
}

fn program_requirement_trace_report(
    requirements: &[ProgramRequirement],
    binding_stats: &Value,
    locale: ArtifactLocale,
) -> Value {
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    let requirement_reviews = requirements
        .iter()
        .map(|requirement| {
            let source_traced = !requirement.source_refs.is_empty();
            let acceptance_present = !requirement.acceptance.trim().is_empty();
            let system_bound = !requirement.system_ids.is_empty();
            if !source_traced {
                blockers.push(issue(
                    "SOURCE_TRACE_MISSING",
                    "BLOCKER",
                    "stage_03",
                    "program_requirement_trace_report.json",
                    &requirement.id,
                    localized_text(
                        locale,
                        "程序需求缺少步骤 02 来源引用。",
                        "The program requirement has no Step02 source reference.",
                    ),
                    localized_text(
                        locale,
                        "请将需求绑定到冻结设计实体或可玩性契约。",
                        "Bind the requirement to a frozen design entity or playable contract.",
                    ),
                    locale,
                ));
            }
            if !acceptance_present {
                blockers.push(issue(
                    "PROGRAM_REQUIREMENT_ACCEPTANCE_MISSING",
                    "BLOCKER",
                    "stage_03",
                    "program_requirement_trace_report.json",
                    &requirement.id,
                    localized_text(
                        locale,
                        "程序需求缺少可验证的验收标准。",
                        "The program requirement has no verifiable acceptance criterion.",
                    ),
                    localized_text(
                        locale,
                        "请补充可由测试或运行时观察验证的验收标准。",
                        "Add acceptance criteria verifiable by tests or runtime observation.",
                    ),
                    locale,
                ));
            }
            if !system_bound {
                warnings.push(issue(
                    "PROGRAM_REQUIREMENT_WITHOUT_SYSTEM",
                    "WARNING",
                    "stage_03",
                    "program_requirement_trace_report.json",
                    &requirement.id,
                    localized_text(
                        locale,
                        "程序需求尚未绑定运行时系统。",
                        "The program requirement is not bound to a runtime system.",
                    ),
                    localized_text(
                        locale,
                        "请依据设计节点依赖补齐系统绑定。",
                        "Add a system binding from the design-node dependency.",
                    ),
                    locale,
                ));
            }
            json!({
                "requirement_id": requirement.id,
                "source_refs": requirement.source_refs,
                "system_ids": requirement.system_ids,
                "acceptance_present": acceptance_present,
                "status": if source_traced && acceptance_present { "traced" } else { "blocked" },
                "message": if source_traced && acceptance_present {
                    localized_text(locale, "需求来源与验收链路完整。", "Requirement source and acceptance trace are complete.")
                } else {
                    localized_text(locale, "需求来源或验收链路不完整。", "Requirement source or acceptance trace is incomplete.")
                }
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "source_refs": [
            "stage_02/project_dna_contract.json",
            "stage_02/design_entities.json",
            "stage_03/program_requirements_contract.json"
        ],
        "requirement_reviews": requirement_reviews,
        "blockers": blockers,
        "warnings": warnings,
        "coverage": binding_stats,
    })
}

fn program_source_coverage(
    requirements: &[ProgramRequirement],
    system_graph: &Value,
    resource_graph: &Value,
    required_contracts: &[String],
    source_handoff: &str,
) -> Value {
    let mut gaps = Vec::new();
    if source_handoff.is_empty() {
        gaps.push("source_design_handoff_missing".to_string());
    }
    if requirements.is_empty() {
        gaps.push("program_requirements_missing".to_string());
    }
    let traced = requirements
        .iter()
        .filter(|req| !req.source_refs.is_empty())
        .count();
    let executable = requirements
        .iter()
        .filter(|req| !req.acceptance.is_empty())
        .count();
    json!({
        "design_handoff_schema_version": STANDARD_SCHEMA_VERSION,
        "consumer_view": "program_requirements",
        "consumer_view_present": !requirements.is_empty(),
        "systems": system_graph.get("nodes").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "resources": resource_graph.get("resources").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
        "visual_objects": 0,
        "ux_signals": 0,
        "knowledge_units": requirements.len(),
        "knowledge_relations": traced,
        "authority_domains": required_contracts.len(),
        "executable_scenarios": executable,
        "coverage_gaps": gaps,
    })
}

fn program_contract_systems(
    system_graph: &Value,
    requirements: &[ProgramRequirement],
    locale: ArtifactLocale,
) -> Vec<Value> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for node in system_graph
        .get("nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let system_id_value =
            non_empty_or(string_field(node, "id"), &string_field(node, "system_id"));
        if system_id_value.is_empty() || !seen.insert(system_id_value.clone()) {
            continue;
        }
        result.push(json!({
            "system_id": system_id_value,
            "source_design_system_id": non_empty_or(string_field(node, "source"), "design_system"),
            "name": non_empty_or(string_field(node, "name"), &system_id_value),
            "responsibility": non_empty_or(
                string_field(node, "responsibility"),
                localized_text(locale, "实现可追溯到来源的程序行为。", "Implement source-traced program behavior."),
            ),
            "description": non_empty_or(
                string_field(node, "description"),
                &if locale == ArtifactLocale::ZhCn {
                    format!("{system_id_value} 由步骤 03 的程序需求推导。")
                } else {
                    format!("{system_id_value} is derived from Step03 program requirements.")
                },
            ),
        }));
    }
    for req in requirements {
        let fallback = non_empty_or(req.system_ids.first().cloned().unwrap_or_default(), &req.id);
        for system_id_value in req.system_ids.iter().cloned().chain([fallback]) {
            if system_id_value.is_empty() || !seen.insert(system_id_value.clone()) {
                continue;
            }
            result.push(json!({
                "system_id": system_id_value,
                "source_design_system_id": non_empty_or(string_field(&req.system_binding, "source_contract"), "program_requirement"),
                "name": system_id_value,
                "responsibility": req.requirement,
                "description": req.acceptance,
            }));
        }
    }
    result
}

fn program_contract_contracts(requirements: &[ProgramRequirement]) -> Vec<Value> {
    requirements
        .iter()
        .map(|req| {
            let target = non_empty_or(req.system_ids.first().cloned().unwrap_or_default(), &req.id);
            json!({
                "contract_id": format!("CONTRACT-{}", req.id),
                "source_system": non_empty_or(string_field(&req.system_binding, "source_contract"), &req.trace_kind),
                "target_system": target,
                "method": "command",
                "inputs": if req.inputs.is_empty() { req.source_refs.clone() } else { req.inputs.clone() },
                "outputs": req.outputs,
                "errors": [],
            })
        })
        .collect()
}

fn program_contract_entities(
    requirements: &[ProgramRequirement],
    parsed: &ParsedDesignSource,
) -> Vec<Value> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for req in requirements {
        let entity_id = non_empty_or(req.entity_id.clone(), &format!("ENTITY-{}", req.id));
        if seen.insert(entity_id.clone()) {
            result.push(json!({
                "entity_id": entity_id,
                "entity_name": non_empty_or(req.entity_label.clone(), &entity_id),
                "owner_system": non_empty_or(req.system_ids.first().cloned().unwrap_or_default(), "ProgramRequirements"),
                "source_text": req.requirement,
            }));
        }
    }
    for selection in &parsed.selections {
        let id = selection.id();
        if seen.insert(id.clone()) {
            result.push(json!({
                "entity_id": id,
                "entity_name": selection.label(),
                "owner_system": "SourceDesign",
                "source_text": selection.source_ref,
            }));
        }
    }
    result
}

fn program_contract_events(
    requirements: &[ProgramRequirement],
    locale: ArtifactLocale,
) -> Vec<Value> {
    requirements
        .iter()
        .map(|req| {
            json!({
                "event_id": format!("EVENT-{}", req.id),
                "event": non_empty_or(
                    req.acceptance.clone(),
                    localized_text(locale, "必须通过验收标准。", "Acceptance criteria must pass."),
                ),
                "source_text": non_empty_or(req.requirement.clone(), &req.id),
            })
        })
        .collect()
}

fn authority(required_contracts: &[String], locale: ArtifactLocale) -> Vec<Value> {
    required_contracts
        .iter()
        .map(|contract_id| {
            json!({
                "authority_id": format!("AUTH-{contract_id}"),
                "authority": format!("stage_02/playable_contracts/{contract_id}.json"),
                "source_text": if locale == ArtifactLocale::ZhCn {
                    format!("{contract_id} 是冻结的可玩性契约，也是步骤 03 程序需求的权威来源。")
                } else {
                    format!("{contract_id} is a frozen playable contract and source of authority for Step03 program requirements.")
                },
            })
        })
        .collect()
}

fn program_contract_acceptance(
    requirements: &[ProgramRequirement],
    locale: ArtifactLocale,
) -> Vec<Value> {
    requirements
        .iter()
        .map(|req| {
            json!({
                "acceptance_id": format!("ACCEPT-{}", req.id),
                "acceptance": non_empty_or(
                    req.acceptance.clone(),
                    localized_text(locale, "必须通过需求验收。", "Requirement acceptance must pass."),
                ),
                "source_text": req.id,
            })
        })
        .collect()
}

fn design_fact_bindings(requirements: &[ProgramRequirement], locale: ArtifactLocale) -> Vec<Value> {
    requirements
        .iter()
        .map(|req| {
            json!({
                "binding_id": format!("BIND-{}", req.id),
                "target_requirement_id": req.id,
                "source_fact_ids": req.source_refs,
                "derivation": localized_text(
                    locale,
                    "需求由冻结的可玩性契约和源设计事实推导。",
                    "Requirement derived from frozen playable contracts and source design facts.",
                ),
            })
        })
        .collect()
}

fn program_path_bindings(requirements: &[ProgramRequirement]) -> Vec<Value> {
    requirements
        .iter()
        .flat_map(|req| {
            req.outputs.iter().enumerate().map(|(index, path)| {
                json!({
                    "binding_id": format!("PATH-{}-{:02}", req.id, index + 1),
                    "owner_id": req.id,
                    "target_path": path,
                    "allowed_outputs": req.outputs,
                    "source": req.id,
                })
            })
        })
        .collect()
}

fn asset_specs_for(entity: &DesignEntity) -> Vec<Value> {
    match entity.kind.to_lowercase().as_str() {
        "character" => vec![
            json!({"suffix": "concept", "asset_type": "art_asset", "priority": "P0", "complexity": "l"}),
            json!({"suffix": "animation_set", "asset_type": "animation", "priority": "P0", "complexity": "xl"}),
            json!({"suffix": "portrait", "asset_type": "ui", "priority": "P1", "complexity": "s"}),
        ],
        "weapon" => vec![
            json!({"suffix": "weapon_concept", "asset_type": "art_asset", "priority": "P0", "complexity": "m"}),
            json!({"suffix": "attack_vfx", "asset_type": "effect", "priority": "P0", "complexity": "m"}),
            json!({"suffix": "icon", "asset_type": "ui", "priority": "P1", "complexity": "s"}),
        ],
        "ability" => vec![
            json!({"suffix": "cast_vfx", "asset_type": "effect", "priority": "P0", "complexity": "l"}),
            json!({"suffix": "hit_vfx", "asset_type": "effect", "priority": "P0", "complexity": "m"}),
            json!({"suffix": "ability_icon", "asset_type": "ui", "priority": "P1", "complexity": "s"}),
        ],
        "room" => vec![
            json!({"suffix": "environment_concept", "asset_type": "environment", "priority": "P0", "complexity": "xl"}),
            json!({"suffix": "tile_set", "asset_type": "environment", "priority": "P0", "complexity": "l"}),
            json!({"suffix": "ambience_audio", "asset_type": "audio", "priority": "P1", "complexity": "m"}),
        ],
        "enemy" => vec![
            json!({"suffix": "enemy_concept", "asset_type": "art_asset", "priority": "P0", "complexity": "l"}),
            json!({"suffix": "attack_vfx", "asset_type": "effect", "priority": "P0", "complexity": "m"}),
            json!({"suffix": "death_vfx", "asset_type": "effect", "priority": "P1", "complexity": "m"}),
        ],
        _ => {
            let asset_type = asset_type_for(entity);
            vec![json!({
                "suffix": "",
                "asset_type": asset_type,
                "priority": priority_for(&asset_type),
                "complexity": complexity_for(&asset_type),
            })]
        }
    }
}

fn asset_type_for(entity: &DesignEntity) -> String {
    let text = format!("{} {} {}", entity.kind, entity.schema, entity.label).to_lowercase();
    if contains_any(&text, &["ui", "hud", "menu", "界面"]) {
        "ui"
    } else if contains_any(
        &text,
        &["ability", "effect", "attack", "技能", "攻击", "特效"],
    ) {
        "effect"
    } else if contains_any(&text, &["room", "level", "environment", "房间", "场景"]) {
        "environment"
    } else if contains_any(&text, &["audio", "sound", "音乐", "音效"]) {
        "audio"
    } else if contains_any(&text, &["config", "resource", "currency", "配置", "资源"]) {
        "config"
    } else {
        "art_asset"
    }
    .to_string()
}

fn asset_name(entity: &DesignEntity, suffix: &str) -> String {
    let label = non_empty_or(entity.label.clone(), &entity.entity_id);
    if suffix.is_empty() {
        label
    } else {
        format!("{label}_{suffix}")
    }
}

fn purpose_for_asset(entity: &DesignEntity, asset_type: &str, locale: ArtifactLocale) -> String {
    let label = non_empty_or(entity.label.clone(), &entity.entity_id);
    match asset_type {
        "ui" if locale == ArtifactLocale::ZhCn => {
            format!("为实体“{label}”提供清晰可读的界面呈现和状态反馈。")
        }
        "ui" => {
            format!("Provide readable UI presentation and state feedback for entity \"{label}\".")
        }
        "effect" if locale == ArtifactLocale::ZhCn => {
            format!("为实体“{label}”提供动作、命中、奖励或状态变化特效。")
        }
        "effect" => {
            format!("Provide action, hit, reward, or state-change VFX for entity \"{label}\".")
        }
        "environment" if locale == ArtifactLocale::ZhCn => {
            format!("为实体“{label}”提供场景、房间或关卡视觉资源。")
        }
        "environment" => format!("Provide scene, room, or level visuals for entity \"{label}\"."),
        "config" if locale == ArtifactLocale::ZhCn => {
            format!("为实体“{label}”提供可配置图标、数据呈现或资源标记。")
        }
        "config" => format!(
            "Provide configurable icon, data presentation, or resource marker for entity \"{label}\"."
        ),
        _ if locale == ArtifactLocale::ZhCn => {
            format!("为 L5 实体“{label}”提供可追溯到生产环节的美术资源。")
        }
        _ => format!("Provide production-traceable art asset for L5 entity \"{label}\"."),
    }
}

fn priority_for(asset_type: &str) -> String {
    if ["ui", "effect", "art_asset"].contains(&asset_type) {
        "P0"
    } else {
        "P1"
    }
    .to_string()
}

fn complexity_for(asset_type: &str) -> String {
    match asset_type {
        "ui" | "config" => "s",
        "effect" | "environment" => "m",
        "animation" => "xl",
        _ => "xs",
    }
    .to_string()
}

fn resolution_for(asset_type: &str, locale: ArtifactLocale) -> String {
    match (asset_type, locale) {
        ("ui", ArtifactLocale::ZhCn) => "1024x1024 源文件，可缩放导出为界面图集",
        ("ui", ArtifactLocale::EnUs) => "1024x1024 source, scalable UI atlas export",
        ("effect", ArtifactLocale::ZhCn) => "2048x2048 精灵表或引擎特效预制体",
        ("effect", ArtifactLocale::EnUs) => "2048x2048 sprite sheet or engine VFX prefab",
        ("environment", ArtifactLocale::ZhCn) => "3840x2160 概念源文件及生产切片",
        ("environment", ArtifactLocale::EnUs) => "3840x2160 concept source plus production slices",
        ("animation", ArtifactLocale::ZhCn) => "包含源帧、可直接用于引擎的动画集",
        ("animation", ArtifactLocale::EnUs) => "engine-ready animation set with source frames",
        (_, ArtifactLocale::ZhCn) => "2048x2048 分层源文件",
        (_, ArtifactLocale::EnUs) => "2048x2048 layered source",
    }
    .to_string()
}

pub fn build_asset_spec_contract(assets: &[ArtAssetRequirement]) -> Value {
    build_asset_spec_contract_with_locale(assets, ArtifactLocale::ZhCn)
}

fn build_asset_spec_contract_with_locale(
    assets: &[ArtAssetRequirement],
    locale: ArtifactLocale,
) -> Value {
    let items = assets
        .iter()
        .map(|asset| {
            let target = format!(
                "Assets/AutoDesign/{}/{}.asset",
                asset.asset_type,
                safe_name(&asset.name)
            );
            json!({
                "asset_id": asset.asset_id,
                "name": asset.name,
                "asset_type": asset.asset_type,
                "linked_contract_field": non_empty_or(asset.source_entity_id.clone(), &asset.source_node_id),
                "source_refs": non_empty_vec([asset.source.clone()]),
                "target_path": target,
                "unity_target_path": target,
                "consumer_system": non_empty_or(asset.source_entity_id.clone(), &asset.source_node_id),
                "mount_point": format!("Assets/AutoDesign/{}", asset.asset_type),
                "dimensions": asset_dimensions(&asset.asset_type),
                "background": asset_background(&asset.asset_type),
                "fallback_policy": localized_text(
                    locale,
                    "生产资源不可用时使用明确标记的可见占位资源。",
                    "Use an explicitly marked visible placeholder when the production asset is unavailable.",
                ),
                "acceptance_check": format!("{}_visible_or_mounted", asset.asset_id),
                "priority": asset.priority,
                "complexity": asset.complexity,
                "required_for_phase": asset.required_for_phase,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "source_refs": [
            "stage_02/playable_contracts/asset_mount_contract.json",
            "stage_02/design_entities.json"
        ],
        "assets": items,
        "blockers": [],
        "warnings": [],
        "production_summary": {
            "asset_count": assets.len(),
            "mount_ready_count": assets.len(),
        },
    })
}

fn asset_spec_contract_from_values(assets: &[Value], locale: ArtifactLocale) -> Value {
    let specs = assets
        .iter()
        .map(|asset| {
            let asset_id = string_field(asset, "asset_id");
            let asset_type = non_empty_or(string_field(asset, "asset_type"), "art_asset");
            let target_path = non_empty_or(
                string_field(asset, "unity_target_path"),
                &string_field(asset, "target_path"),
            );
            let consumer_system = non_empty_or(
                string_field(asset, "consumer_system"),
                &non_empty_or(
                    string_field(asset, "source_entity_id"),
                    &string_field(asset, "source_node_id"),
                ),
            );
            let mount_point = non_empty_or(
                string_field(asset, "mount_point"),
                &format!("Assets/AutoDesign/{asset_type}"),
            );
            json!({
                "asset_id": asset_id,
                "name": non_empty_or(string_field(asset, "name"), &asset_id),
                "asset_type": asset_type,
                "linked_contract_field": non_empty_or(
                    string_field(asset, "linked_contract_field"),
                    &non_empty_or(string_field(asset, "source_entity_id"), &string_field(asset, "source_node_id"))
                ),
                "source_refs": string_array(asset.get("source_refs")),
                "target_path": target_path,
                "unity_target_path": target_path,
                "consumer_system": consumer_system,
                "mount_point": mount_point,
                "dimensions": asset_dimensions(&asset_type),
                "background": asset_background(&asset_type),
                "fallback_policy": localized_text(
                    locale,
                    "生产资源不可用时使用明确标记的可见占位资源。",
                    "Use an explicitly marked visible placeholder when the production asset is unavailable.",
                ),
                "acceptance_check": non_empty_or(
                    string_field(asset, "acceptance_check"),
                    &format!("{asset_id}_visible_or_mounted")
                ),
                "priority": non_empty_or(string_field(asset, "priority"), "P0"),
                "complexity": non_empty_or(string_field(asset, "complexity"), "m"),
                "required_for_phase": non_empty_or(string_field(asset, "required_for_phase"), "core_playable"),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "source_refs": [
            "stage_02/playable_contracts/asset_mount_contract.json",
            "stage_02/design_entities.json",
            "stage_04/asset_strategy_matrix.json"
        ],
        "assets": specs,
        "blockers": [],
        "warnings": [],
        "production_summary": {
            "asset_count": assets.len(),
            "mount_ready_count": assets.iter().filter(|asset| !string_field(asset, "unity_target_path").is_empty()).count(),
        },
    })
}

fn asset_dimensions(asset_type: &str) -> Value {
    match asset_type.to_ascii_lowercase().as_str() {
        "ui" | "icon" | "sprite" => json!({"width": 1024, "height": 1024}),
        "effect" | "vfx" => json!({"width": 2048, "height": 2048}),
        "environment" => json!({"width": 3840, "height": 2160}),
        "audio_placeholder" => json!({"width": 0, "height": 0}),
        _ => json!({"width": 2048, "height": 2048}),
    }
}

fn asset_background(asset_type: &str) -> &'static str {
    match asset_type.to_ascii_lowercase().as_str() {
        "ui" | "icon" | "sprite" | "effect" | "vfx" => "transparent",
        "audio_placeholder" => "not_applicable",
        _ => "opaque_or_scene_context",
    }
}

fn asset_registry_document(
    assets: &[ArtAssetRequirement],
    asset_spec: &Value,
    locale: ArtifactLocale,
) -> Value {
    let specs = value_array(asset_spec.get("assets"));
    let entries = assets
        .iter()
        .map(|asset| {
            let spec = specs
                .iter()
                .find(|spec| string_field(spec, "asset_id") == asset.asset_id);
            let target_path = spec
                .map(|spec| string_field(spec, "target_path"))
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| {
                    format!(
                        "Assets/AutoDesign/{}/{}.asset",
                        asset.asset_type,
                        safe_name(&asset.name)
                    )
                });
            let acceptance_check = spec
                .map(|spec| string_field(spec, "acceptance_check"))
                .filter(|check| !check.is_empty())
                .unwrap_or_else(|| format!("{}_visible_or_mounted", asset.asset_id));
            json!({
                "asset_id": asset.asset_id,
                "name": asset.name,
                "asset_type": asset.asset_type,
                "source": asset.source,
                "source_refs": non_empty_vec([asset.source.clone()]),
                "purpose": asset.purpose,
                "target_path": target_path,
                "unity_target_path": target_path,
                "consumer_system": non_empty_or(asset.source_entity_id.clone(), &asset.source_node_id),
                "mount_point": format!("Assets/AutoDesign/{}", asset.asset_type),
                "fallback_strategy": localized_text(
                    locale,
                    "生产资源不可用时使用明确标记的可见占位资源。",
                    "Use an explicitly marked visible placeholder when the production asset is unavailable.",
                ),
                "acceptance_check": acceptance_check,
                "priority": asset.priority,
                "complexity": asset.complexity,
                "required_for_phase": asset.required_for_phase,
                "source_entity_id": asset.source_entity_id,
                "source_node_id": asset.source_node_id,
                "status": asset.status,
                "trace_kind": asset.trace_kind,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "assets": entries,
    })
}

pub fn validate_asset_spec_contract(contract: &Value) -> Value {
    validate_asset_spec_contract_with_locale(contract, ArtifactLocale::ZhCn)
}

fn validate_asset_spec_contract_with_locale(contract: &Value, locale: ArtifactLocale) -> Value {
    let assets = contract
        .get("assets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    for asset in &assets {
        let id = string_field(asset, "asset_id");
        if string_field(asset, "linked_contract_field").is_empty() {
            blockers.push(json!({
                "code": "ASSET_SOURCE_FIELD_MISSING",
                "message": if locale == ArtifactLocale::ZhCn {
                    format!("{id} 缺少关联契约字段。")
                } else {
                    format!("{id} has no linked contract field")
                },
            }));
        }
        if string_field(asset, "unity_target_path").is_empty() {
            blockers.push(json!({
                "code": "ASSET_TARGET_PATH_MISSING",
                "message": if locale == ArtifactLocale::ZhCn {
                    format!("{id} 缺少 Unity 目标路径。")
                } else {
                    format!("{id} has no Unity target path")
                },
            }));
        }
        if string_array(asset.get("source_refs")).is_empty() {
            warnings.push(json!({
                "code": "ASSET_SOURCE_REF_MISSING",
                "message": if locale == ArtifactLocale::ZhCn {
                    format!("{id} 缺少来源引用。")
                } else {
                    format!("{id} has no source refs")
                },
            }));
        }
    }
    json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "artifact_locale": locale,
        "valid": blockers.is_empty(),
        "blockers": blockers,
        "warnings": warnings,
        "asset_count": assets.len(),
    })
}

fn standard_assets(assets: &[ArtAssetRequirement], locale: ArtifactLocale) -> Vec<Value> {
    assets
        .iter()
        .enumerate()
        .filter_map(|(index, asset)| standard_asset(asset, index + 1, locale))
        .collect()
}

fn standard_asset(
    asset: &ArtAssetRequirement,
    index: usize,
    locale: ArtifactLocale,
) -> Option<Value> {
    let category = asset_category(&asset.asset_type)?;
    let target = format!(
        "Assets/AutoDesign/{}/{}.asset",
        asset.asset_type,
        safe_name(&asset.name)
    );
    Some(json!({
        "asset_id": non_empty_or(asset.asset_id.clone(), &format!("ASSET-{index:03}")),
        "name": non_empty_or(asset.name.clone(), &asset.asset_id),
        "category": category,
        "source_visual_object_id": non_empty_or(non_empty_or(asset.source_entity_id.clone(), &asset.source_node_id), &asset.asset_id),
        "source_system_ids": non_empty_vec([asset.source_entity_id.clone(), asset.source_node_id.clone(), asset.trace_kind.clone()]),
        "purpose": non_empty_or(
            asset.purpose.clone(),
            localized_text(
                locale,
                "可玩性契约所需、可追溯到来源的视觉资源。",
                "Source-traced visual asset required by the playable contract.",
            ),
        ),
        "required_readability": ["readable_at_runtime", "runtime_mount"],
        "production_specs": {
            "asset_type": asset.asset_type,
            "target_path": target,
            "unity_target_path": target,
            "required_format": "unity_consumable_asset",
            "required_size": {},
            "transparency": "unspecified",
            "import_settings": {},
            "priority": non_empty_or(asset.priority.clone(), "P0"),
            "complexity": non_empty_or(asset.complexity.clone(), "m"),
            "required_for_phase": non_empty_or(asset.required_for_phase.clone(), "core_playable"),
            "source_refs": non_empty_vec([asset.source.clone()]),
        },
        "forbidden_visuals": if locale == ArtifactLocale::ZhCn {
            json!(["水印", "不可编辑的烘焙界面文字", "独立界面精灵中的复杂背景"])
        } else {
            json!(["watermark", "non-editable baked UI text", "complex background in isolated UI sprites"])
        },
        "acceptance_checks": [format!("{}_visible_or_mounted", asset.asset_id)],
    }))
}

fn asset_category(asset_type: &str) -> Option<&'static str> {
    match asset_type.to_lowercase().as_str() {
        "ui" | "icon" | "sprite" | "config" => Some("ui"),
        "effect" | "vfx" => Some("vfx"),
        "audio_placeholder" => None,
        _ => Some("illustration"),
    }
}

fn visual_states(assets: &[Value], locale: ArtifactLocale) -> Vec<Value> {
    assets
        .iter()
        .map(|asset| {
            let id = string_field(asset, "asset_id");
            json!({
                "visual_state_id": format!("STATE-{id}-default"),
                "asset_id": id,
                "source_state_id": "default",
                "state_name": "default",
                "required_difference": localized_text(
                    locale,
                    "资源在运行时挂载状态下必须清晰可读。",
                    "Asset must be visually readable in its runtime mount state.",
                ),
            })
        })
        .collect()
}

fn ux_signal_bindings(assets: &[Value]) -> Vec<Value> {
    let mut bindings = Vec::new();
    for asset in assets {
        for check in string_array(asset.get("acceptance_checks")) {
            bindings.push(json!({
                "binding_id": format!("UX-{}-{:03}", string_field(asset, "asset_id"), bindings.len() + 1),
                "ux_signal_id": check,
                "asset_id": string_field(asset, "asset_id"),
                "required_feedback": check,
                "timing": "runtime_visible_or_interactive",
            }));
        }
    }
    bindings
}

fn drift_checks(assets: &[Value], locale: ArtifactLocale) -> Vec<Value> {
    assets
        .iter()
        .map(|asset| {
            let id = string_field(asset, "asset_id");
            json!({
                "check_id": format!("DRIFT-{id}"),
                "asset_id": id,
                "rule": localized_text(
                    locale,
                    "资源必须始终可追溯到步骤 02 的可玩性契约，且不得引入无关的视觉范围。",
                    "Asset must remain traceable to Step02 playable contracts and must not introduce unrelated visual scope.",
                ),
                "severity": "OK",
            })
        })
        .collect()
}

fn art_path_bindings(assets: &[Value]) -> Vec<Value> {
    assets
        .iter()
        .enumerate()
        .filter_map(|(index, asset)| {
            let specs = asset.get("production_specs").cloned().unwrap_or_else(|| json!({}));
            let target = non_empty_or(string_field(&specs, "unity_target_path"), &string_field(&specs, "target_path"));
            if target.is_empty() {
                None
            } else {
                Some(json!({
                    "binding_id": format!("ART-PATH-{:03}", index + 1),
                    "asset_id": string_field(asset, "asset_id"),
                    "source_path": string_array(specs.get("source_refs")).first().cloned().unwrap_or_else(|| "source_design".to_string()),
                    "target_path": target,
                    "output_files": [target],
                }))
            }
        })
        .collect()
}

fn art_source_coverage(
    source_handoff: &str,
    raw_assets: &[ArtAssetRequirement],
    standard_assets: &[Value],
    required_contracts: &[String],
) -> Value {
    let mut gaps = Vec::new();
    if source_handoff.is_empty() {
        gaps.push("source_design_handoff_missing".to_string());
    }
    if raw_assets.is_empty() {
        gaps.push("art_assets_missing".to_string());
    }
    if !raw_assets.is_empty() && standard_assets.is_empty() {
        gaps.push("visual_assets_missing".to_string());
    }
    let traced = raw_assets
        .iter()
        .filter(|asset| !asset.source.is_empty())
        .count();
    let executable = raw_assets
        .iter()
        .filter(|asset| !asset.asset_id.is_empty())
        .count();
    let ux_signals = standard_assets
        .iter()
        .map(|asset| {
            asset
                .get("acceptance_checks")
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0)
        })
        .sum::<usize>()
        .max(executable);
    json!({
        "design_handoff_schema_version": STANDARD_SCHEMA_VERSION,
        "consumer_view": "art_requirements",
        "consumer_view_present": !raw_assets.is_empty(),
        "systems": raw_assets.iter().filter(|asset| !asset.source_entity_id.is_empty()).map(|asset| asset.source_entity_id.clone()).collect::<BTreeSet<_>>().len(),
        "resources": raw_assets.len(),
        "visual_objects": standard_assets.len(),
        "ux_signals": ux_signals,
        "knowledge_units": raw_assets.len(),
        "knowledge_relations": traced,
        "authority_domains": required_contracts.len(),
        "executable_scenarios": executable,
        "coverage_gaps": gaps,
    })
}

fn contract_review_items(contract: &Value, stage: &str, locale: ArtifactLocale) -> Vec<Value> {
    let quality = contract.get("quality").unwrap_or(&Value::Null);
    let mut items = Vec::new();
    for (field, severity) in [("blockers", "BLOCKER"), ("warnings", "WARNING")] {
        for raw in value_array(quality.get(field)) {
            let raw_code = raw
                .get("code")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| raw.as_str().map(explicit_code_prefix))
                .unwrap_or_default();
            let code = canonical_contract_issue_code(&raw_code, stage);
            let (title, default_reason, suggestion) = contract_issue_text(&code, stage, locale);
            let reason = raw
                .get("message")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| default_reason.to_string());
            items.push(json!({
                "code": code,
                "severity": severity,
                "title": title,
                "message": reason,
                "reason": reason,
                "suggestion": suggestion,
                "return_target": contract_issue_return_target(&code, stage),
            }));
        }
    }
    items
}

fn is_blocking_review_item(item: &Value) -> bool {
    matches!(
        string_field(item, "severity").as_str(),
        "BLOCKER" | "CRITICAL"
    )
}

fn has_blocking_review_item(items: &[Value]) -> bool {
    items.iter().any(is_blocking_review_item)
}

fn blocking_review_items(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .filter(|item| is_blocking_review_item(item))
        .cloned()
        .collect()
}

fn non_blocking_review_items(items: &[Value]) -> Vec<Value> {
    items
        .iter()
        .filter(|item| !is_blocking_review_item(item))
        .cloned()
        .collect()
}

fn contract_validity_blocker(code: &str, stage: &str, locale: ArtifactLocale) -> Value {
    let (title, reason, suggestion) = contract_issue_text(code, stage, locale);
    json!({
        "code": code,
        "severity": "BLOCKER",
        "title": title,
        "message": reason,
        "reason": reason,
        "suggestion": suggestion,
        "return_target": stage,
    })
}

fn warning_items(review_items: &[Value]) -> Vec<Value> {
    review_items
        .iter()
        .filter(|item| string_field(item, "severity") == "WARNING")
        .cloned()
        .collect()
}

fn explicit_code_prefix(value: &str) -> String {
    value
        .split_once(':')
        .map(|(code, _)| code)
        .unwrap_or(value)
        .trim()
        .to_string()
}

fn canonical_contract_issue_code(raw_code: &str, stage: &str) -> String {
    match raw_code.trim() {
        "source_design_handoff_missing" => "SOURCE_DESIGN_HANDOFF_MISSING".to_string(),
        "program_requirements_missing" => "PROGRAM_REQUIREMENTS_MISSING".to_string(),
        "art_assets_missing" => "ART_ASSETS_MISSING".to_string(),
        "visual_assets_missing" => "VISUAL_ASSETS_MISSING".to_string(),
        code if !code.is_empty() => code.to_string(),
        _ => format!("STEP{stage}_REVIEW_ITEM"),
    }
}

fn contract_issue_return_target(code: &str, stage: &str) -> String {
    match code {
        "PROJECT_DNA_CONTRACT_MISSING"
        | "PROJECT_DNA_CONTRACT_NOT_FROZEN"
        | "ACTION_HAS_NO_STATE_CHANGE"
        | "OBJECTIVE_HAS_NO_COMPLETION_CONDITION" => "02".to_string(),
        _ => stage.to_string(),
    }
}

fn contract_issue_text(
    code: &str,
    stage: &str,
    locale: ArtifactLocale,
) -> (&'static str, &'static str, &'static str) {
    let (zh_cn, en_us) = match code {
        "SOURCE_DESIGN_HANDOFF_MISSING" => (
            (
                "缺少设计交接",
                "缺少源设计交接。",
                "请返回步骤 02，重新生成结构化设计交接。",
            ),
            (
                "Design handoff missing",
                "The source design handoff is missing.",
                "Return to Step02 and regenerate the structured design handoff.",
            ),
        ),
        "PROGRAM_REQUIREMENTS_MISSING" => (
            (
                "缺少程序需求",
                "未生成程序需求。",
                "请检查步骤 02 的结构化实体后重新运行步骤 03。",
            ),
            (
                "Program requirements missing",
                "No program requirements were generated.",
                "Check Step02 structured entities and rerun Step03.",
            ),
        ),
        "ART_ASSETS_MISSING" => (
            (
                "缺少美术资产需求",
                "未生成美术资产需求。",
                "请检查步骤 02 的结构化实体后重新运行步骤 04。",
            ),
            (
                "Art asset requirements missing",
                "No art asset requirements were generated.",
                "Check Step02 structured entities and rerun Step04.",
            ),
        ),
        "VISUAL_ASSETS_MISSING" => (
            (
                "缺少可生产视觉资产",
                "没有可转换为生产规格的视觉资产。",
                "请补充可追溯的视觉实体并重新运行步骤 04。",
            ),
            (
                "Production visual assets missing",
                "No visual assets can be converted into production specifications.",
                "Add traceable visual entities and rerun Step04.",
            ),
        ),
        "PROJECT_DNA_CONTRACT_MISSING" => (
            (
                "缺少冻结的项目 DNA 契约",
                "步骤 02 未提供冻结的项目 DNA 契约。",
                "请返回步骤 02，重新生成并冻结项目 DNA 契约。",
            ),
            (
                "Frozen Project DNA contract missing",
                "Step02 did not provide a frozen Project DNA contract.",
                "Return to Step02 and regenerate the frozen Project DNA contract.",
            ),
        ),
        "PROJECT_DNA_CONTRACT_NOT_FROZEN" => (
            (
                "项目 DNA 契约尚未冻结",
                "项目 DNA 契约仍处于未冻结或阻断状态。",
                "请在步骤 02 解决冻结阻断项后重新运行。",
            ),
            (
                "Project DNA contract is not frozen",
                "The Project DNA contract remains unfrozen or blocked.",
                "Resolve the Step02 freeze blockers and rerun the stage.",
            ),
        ),
        "ACTION_HAS_NO_STATE_CHANGE" => (
            (
                "玩家动作缺少状态变化",
                "项目 DNA 中的玩家动作没有声明可观察的状态变化。",
                "请返回步骤 02，为玩家动作补齐状态变化。",
            ),
            (
                "Player action has no state change",
                "A Project DNA player action has no observable state change.",
                "Return to Step02 and add the player action state change.",
            ),
        ),
        "OBJECTIVE_HAS_NO_COMPLETION_CONDITION" => (
            (
                "目标缺少完成条件",
                "项目 DNA 中的目标没有完成条件或失败条件。",
                "请返回步骤 02，补齐目标条件。",
            ),
            (
                "Objective has no completion condition",
                "A Project DNA objective has neither completion nor failure conditions.",
                "Return to Step02 and add objective conditions.",
            ),
        ),
        "PROGRAM_CAPABILITY_NOT_BOUND" => (
            (
                "程序语义覆盖不足",
                "必需的项目 DNA 语义未完整绑定到程序能力。",
                "请在步骤 03 补齐程序能力和语义绑定。",
            ),
            (
                "Program semantic coverage is insufficient",
                "Required Project DNA semantics are not fully bound to program capabilities.",
                "Complete program capabilities and semantic bindings in Step03.",
            ),
        ),
        "ASSET_REQUIREMENT_UNRESOLVED" => (
            (
                "资产需求尚未解析",
                "资产需求缺少可消费路径、使用方或挂载信息。",
                "请在步骤 04 补齐资产目标和挂载契约。",
            ),
            (
                "Asset requirement unresolved",
                "An asset requirement lacks a consumable path, consumer, or mount information.",
                "Complete the asset target and mount contract in Step04.",
            ),
        ),
        "ASSET_SOURCE_REF_MISSING" => (
            (
                "美术资产缺少来源引用",
                "美术资产缺少来源引用。",
                "请补充步骤 02 实体或设计选择的来源引用。",
            ),
            (
                "Art asset source reference missing",
                "An art asset has no source reference.",
                "Add a source reference from a Step02 entity or design selection.",
            ),
        ),
        "ASSET_SOURCE_FIELD_MISSING" => (
            (
                "美术资产缺少契约字段",
                "美术资产未关联到契约字段。",
                "请将资产关联到步骤 02 的实体或节点。",
            ),
            (
                "Art asset contract field missing",
                "An art asset is not linked to a contract field.",
                "Link the asset to a Step02 entity or node.",
            ),
        ),
        "ASSET_TARGET_PATH_MISSING" => (
            (
                "美术资产缺少目标路径",
                "美术资产缺少 Unity 目标路径。",
                "请补充明确且允许写入的 Unity 目标路径。",
            ),
            (
                "Art asset target path missing",
                "An art asset has no Unity target path.",
                "Provide an explicit permitted Unity target path.",
            ),
        ),
        _ if stage == "03" => (
            (
                "程序需求需要复核",
                "程序需求契约包含需要复核的项目。",
                "请检查步骤 03 的程序需求契约。",
            ),
            (
                "Program requirements need review",
                "The program requirements contract contains a review item.",
                "Inspect the Step03 program requirements contract.",
            ),
        ),
        _ => (
            (
                "美术需求需要复核",
                "美术需求契约包含需要复核的项目。",
                "请检查步骤 04 的美术需求契约。",
            ),
            (
                "Art requirements need review",
                "The art requirements contract contains a review item.",
                "Inspect the Step04 art requirements contract.",
            ),
        ),
    };
    match locale {
        ArtifactLocale::ZhCn => zh_cn,
        ArtifactLocale::EnUs => en_us,
    }
}

fn review_result(path: &str, review: &Value, structured_inputs: &Value) -> Value {
    let locale = artifact_locale_from_inputs(structured_inputs);
    let verdict = review
        .get("verdict")
        .and_then(Value::as_str)
        .unwrap_or("FAIL");
    let issues = value_array(review.get("issues"));
    let blocking_issues = blocking_review_items(&issues);
    let review_items = non_blocking_review_items(&issues);
    let warnings = review_items
        .iter()
        .filter(|item| string_field(item, "severity") == "WARNING")
        .cloned()
        .collect::<Vec<_>>();
    json!({
        "status": match verdict {
            "PASS" => "success",
            "WARN" => "completed_with_review",
            "FAIL" | "BLOCKED" => "blocked",
            _ if !blocking_issues.is_empty() => "blocked",
            _ => "completed_with_review",
        },
        "content_exists": true,
        "traceability_valid": blocking_issues.is_empty()
            && review.get("critical_count").and_then(Value::as_u64).unwrap_or(0) == 0,
        "review_report": path,
        "verdict": verdict,
        "artifact_locale": locale,
        "blocking_issue_count": blocking_issues.len(),
        "blocking_issues": blocking_issues,
        "requires_action_count": review.get("requires_action_count").cloned().unwrap_or_else(|| json!(0)),
        "review_items_count": review_items.len(),
        "review_items": review_items,
        "warnings": warnings,
        "message": review.get("message").and_then(Value::as_str).unwrap_or_else(|| {
            localized_text(locale, "评审已完成。", "Review completed.")
        }),
        "semantic_quality": {
            "status": if matches!(verdict, "FAIL" | "BLOCKED") { "blocked" } else if verdict == "PASS" { "success" } else { "warning" },
            "return_targets": issues,
        },
        "structured_input_status": structured_inputs.get("status").and_then(Value::as_str).unwrap_or("fallback_to_markdown"),
    })
}

fn review_report(scope: &str, issues: Vec<Value>, locale: ArtifactLocale) -> Value {
    let mut counts = BTreeMap::from([
        ("BLOCKER".to_string(), 0usize),
        ("CRITICAL".to_string(), 0usize),
        ("WARNING".to_string(), 0usize),
        ("INFO".to_string(), 0usize),
    ]);
    for issue_value in &issues {
        let severity = string_field(issue_value, "severity");
        *counts.entry(non_empty_or(severity, "INFO")).or_default() += 1;
    }
    let blocker = *counts.get("BLOCKER").unwrap_or(&0);
    let critical = *counts.get("CRITICAL").unwrap_or(&0);
    let warning = *counts.get("WARNING").unwrap_or(&0);
    let total = issues.len();
    let report_verdict = verdict(blocker, critical, warning);
    let blockers = blocking_review_items(&issues);
    let warnings = issues
        .iter()
        .filter(|item| string_field(item, "severity") == "WARNING")
        .cloned()
        .collect::<Vec<_>>();
    let source_refs = if scope == "program_requirements" {
        json!([
            "stage_03/program_requirements_contract.json",
            "stage_03/program_capability_contract.json",
            "stage_03/program_semantic_coverage_report.json"
        ])
    } else {
        json!([
            "stage_04/art_requirements_contract.json",
            "stage_04/asset_registry.json",
            "stage_04/asset_strategy_matrix.json"
        ])
    };
    let review_status = match report_verdict {
        "PASS" => "passed",
        "WARN" => "passed_with_review",
        "FAIL" | "BLOCKED" => "blocked",
        _ => "review_required",
    };
    json!({
        "schema_version": "1.0",
        "generated_at": now_iso(),
        "scope": scope,
        "artifact_locale": locale,
        "source_refs": source_refs,
        "review_status": review_status,
        "blockers": blockers,
        "warnings": warnings,
        "coverage": {
            "reviewed_item_count": total,
            "blocking_item_count": blocker + critical,
            "warning_item_count": warning,
        },
        "asset_reviews": if scope == "art_requirements" { json!(issues) } else { json!([]) },
        "title": match (scope, locale) {
            ("program_requirements", ArtifactLocale::ZhCn) => "程序需求智能评审",
            ("program_requirements", ArtifactLocale::EnUs) => "Program Requirements Review",
            ("art_requirements", ArtifactLocale::ZhCn) => "美术需求智能评审",
            ("art_requirements", ArtifactLocale::EnUs) => "Art Requirements Review",
            (_, ArtifactLocale::ZhCn) => "需求智能评审",
            (_, ArtifactLocale::EnUs) => "Requirements Review",
        },
        "message": if total == 0 {
            localized_text(locale, "评审通过，未发现需要处理的问题。", "Review passed with no actionable issues.").to_string()
        } else if locale == ArtifactLocale::ZhCn {
            format!("评审完成，共发现 {total} 个需要复核的问题。")
        } else {
            format!("Review completed with {total} item(s) requiring attention.")
        },
        "verdict": report_verdict,
        "issues": issues,
        "severity_counts": counts,
        "blocker_count": blocker,
        "critical_count": critical,
        "requires_action_count": blocker + critical,
        "blocking_issue_count": blocker,
        "warning_count": warning,
    })
}

fn issue(
    code: &str,
    severity: &str,
    stage: &str,
    artifact: &str,
    field: &str,
    reason: &str,
    suggestion: &str,
    locale: ArtifactLocale,
) -> Value {
    json!({
        "severity": severity,
        "code": code,
        "title": issue_title(code, locale),
        "stage": stage,
        "artifact": artifact,
        "field": field,
        "message": reason,
        "reason": reason,
        "suggestion": suggestion,
        "return_target": issue_return_target(code),
    })
}

fn issue_title(code: &str, locale: ArtifactLocale) -> &'static str {
    let (zh_cn, en_us) = match code {
        "PROGRAM_REQUIREMENTS_MISSING" => ("缺少程序需求", "Program requirements missing"),
        "PROGRAM_REQUIREMENT_WITHOUT_SYSTEM" => (
            "程序需求未绑定系统",
            "Program requirement has no system binding",
        ),
        "PROGRAM_REQUIREMENT_TEXT_TOO_SHORT" => {
            ("程序需求描述过短", "Program requirement text too short")
        }
        "PROGRAM_REQUIREMENT_ACCEPTANCE_MISSING" => (
            "程序需求缺少验收标准",
            "Program requirement acceptance missing",
        ),
        "PROGRAM_REQUIREMENT_L4_DEPTH" => (
            "程序需求深度不足",
            "Program requirement depth is insufficient",
        ),
        "PROGRAM_REQUIREMENT_PLACEHOLDER_RATE_HIGH" => (
            "程序需求占位内容过多",
            "Program requirement placeholder rate is high",
        ),
        "ART_ASSETS_MISSING" => ("缺少美术资产需求", "Art asset requirements missing"),
        "ART_ASSET_REQUIRED_FIELD_MISSING" => {
            ("美术资产缺少必填字段", "Art asset required field missing")
        }
        "ART_ASSET_TYPE_MISSING" => ("美术资产类型覆盖不足", "Art asset type coverage missing"),
        "ART_P0_ASSET_MISSING" => ("缺少 P0 美术资产", "P0 art asset missing"),
        "SOURCE_TRACE_MISSING" => ("缺少来源追踪", "Source trace missing"),
        "PLACEHOLDER_TOKEN_REMAINS" => ("仍存在占位标记", "Placeholder token remains"),
        _ => ("需要复核的问题", "Review issue"),
    };
    localized_text(locale, zh_cn, en_us)
}

fn issue_return_target(code: &str) -> &'static str {
    match code {
        "ART_ASSETS_MISSING"
        | "ART_ASSET_REQUIRED_FIELD_MISSING"
        | "ART_ASSET_TYPE_MISSING"
        | "ART_P0_ASSET_MISSING" => "04",
        "SOURCE_TRACE_MISSING" | "PLACEHOLDER_TOKEN_REMAINS" => "02",
        _ => "03",
    }
}

fn verdict(blocker: usize, critical: usize, warning: usize) -> &'static str {
    if blocker > 0 {
        "BLOCKED"
    } else if critical > 0 {
        "FAIL"
    } else if warning > 0 {
        "WARN"
    } else {
        "PASS"
    }
}

fn is_project_configuration_requirement(requirement: &ProgramRequirement) -> bool {
    let combined = format!(
        "{} {} {} {} {}",
        requirement.requirement,
        requirement.entity_label,
        requirement.entity_kind,
        requirement.entity_schema,
        requirement.phase
    );
    [
        "项目规模",
        "商业模式",
        "平台范围",
        "地区范围",
        "项目定位",
        "社交模式",
    ]
    .iter()
    .any(|token| combined.contains(token))
}

fn is_l4_derived_requirement(requirement: &ProgramRequirement) -> bool {
    ["范本反推", "项目配置", "设计决策节点"]
        .iter()
        .any(|token| requirement.requirement.contains(token))
}

fn contains_placeholder(text: &str) -> bool {
    let detector = PlaceholderDetector;
    !detector.detect(text).is_empty()
}

fn placeholder_tokens() -> &'static [&'static str] {
    &[
        "待定义",
        "待完善",
        "placeholder",
        "TODO",
        "{{",
        "}}",
        "<待",
        "未命名",
    ]
}

fn source_design_handoff(parsed: &ParsedDesignSource, source_markdown: &str) -> String {
    parsed
        .design_summary
        .get("source_design_handoff")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            if source_markdown.is_empty() {
                "source_design".to_string()
            } else {
                source_markdown.to_string()
            }
        })
}

fn source_contract_paths(required_contracts: &[String]) -> Vec<String> {
    required_contracts
        .iter()
        .map(|contract_id| format!("stage_02/playable_contracts/{contract_id}.json"))
        .collect()
}

fn required_contracts() -> Vec<String> {
    STAGE2_REQUIRED_PLAYABLE_CONTRACTS
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn stringify_blockers(blockers: &[Value]) -> Vec<String> {
    blockers
        .iter()
        .filter_map(|blocker| {
            if blocker.is_object() {
                let code = non_empty_or(string_field(blocker, "code"), "BLOCKER");
                let message = non_empty_or(string_field(blocker, "message"), &code);
                Some(format!("{code}: {message}"))
            } else {
                blocker.as_str().map(str::to_string)
            }
        })
        .filter(|value| !value.is_empty())
        .collect()
}

fn ensure_stage(actual: u32, expected: u32) -> AdmResult<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(AdmError::new(format!(
            "generator expected stage {expected:02}, got {actual:02}"
        )))
    }
}

fn to_json_value<T: Serialize>(value: &T) -> AdmResult<Value> {
    serde_json::to_value(value)
        .map_err(|error| AdmError::new(format!("failed to serialize step03-06 JSON: {error}")))
}

fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .and_then(|item| match item {
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| match item {
            Value::String(text) => Some(text.trim().to_string()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .filter(|item| !item.is_empty())
        .collect()
}

fn value_array(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

fn non_empty_or(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn non_empty_vec<const N: usize>(values: [String; N]) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect()
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn tokens(value: &str) -> BTreeSet<String> {
    let lower = value.to_lowercase();
    let mut tokens = lower
        .split(|ch: char| {
            !(ch.is_alphanumeric() || ch == '_' || ('\u{4e00}'..='\u{9fff}').contains(&ch))
        })
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    for (_, keywords) in domain_keywords() {
        for keyword in keywords {
            if lower.contains(keyword) {
                tokens.insert(keyword.to_string());
            }
        }
    }
    tokens
}

fn token_score(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f64 {
    let overlap =
        left.intersection(right).count() as f64 / left.len().min(right.len()).max(1) as f64;
    overlap.max(domain_group_score(left, right))
}

fn domain_group_score(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f64 {
    for (_, keywords) in domain_keywords() {
        if keywords.iter().any(|keyword| left.contains(*keyword))
            && keywords.iter().any(|keyword| right.contains(*keyword))
        {
            return 0.6;
        }
    }
    0.0
}

fn domain_keywords() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "combat",
            vec![
                "combat", "attack", "damage", "weapon", "fight", "hit", "战斗", "攻击", "伤害",
                "武器",
            ],
        ),
        (
            "progression",
            vec![
                "progression",
                "unlock",
                "upgrade",
                "level",
                "talent",
                "成长",
                "解锁",
                "升级",
            ],
        ),
        (
            "ui",
            vec![
                "ui",
                "hud",
                "display",
                "menu",
                "interface",
                "button",
                "界面",
                "菜单",
                "显示",
            ],
        ),
        (
            "objective",
            vec![
                "objective",
                "goal",
                "win",
                "lose",
                "escape",
                "目标",
                "胜利",
                "失败",
            ],
        ),
        (
            "settlement",
            vec![
                "settlement",
                "reward",
                "loot",
                "drop",
                "currency",
                "奖励",
                "掉落",
                "货币",
            ],
        ),
    ]
}

fn system_id(system: &Value) -> String {
    non_empty_or(
        string_field(system, "system_id"),
        &string_field(system, "node_id"),
    )
}

fn contains_any(text: &str, tokens: &[&str]) -> bool {
    tokens.iter().any(|token| text.contains(token))
}

fn ratio(left: usize, right: usize) -> f64 {
    if right == 0 {
        0.0
    } else {
        round4(left as f64 / right as f64)
    }
}

fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

fn safe_name(value: &str) -> String {
    let name = value
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if name.is_empty() {
        "asset".to_string()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::parse_design_text;
    use crate::stages::step00_02::parse_response_entities;
    use adm_new_contracts::schema::{load_structured_file, validate_contract};
    use adm_new_foundation::{new_stable_id, paths::SourceProjectRoot};
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn plugin_specs_match_python_stage03_06_wrappers() {
        assert_eq!(
            step03_plugin_spec().source_groups[0].label,
            "program_requirements"
        );
        assert_eq!(
            step04_plugin_spec().source_groups[0].source_ids,
            vec!["ArtReq"]
        );
        assert_eq!(step05_plugin_spec().source_groups[0].mode, "latest");
        assert_eq!(step06_plugin_spec().stage_id, "06");
    }

    #[test]
    fn step03_converts_entities_binds_systems_and_builds_schema_valid_contract() {
        let parsed = sample_design();
        let mut requirements = EntityToRequirementConverter.convert(&parsed);
        let system_graph = system_graph_from_parsed(&parsed, ArtifactLocale::ZhCn);
        SystemBinder.bind(&mut requirements, &system_graph);
        let binding_stats = build_requirement_quality_report(&requirements);
        let structure = program_structure_spec(&requirements, ArtifactLocale::ZhCn);
        let resource_graph = resource_graph_from_requirements(&requirements);
        let required = required_contracts();
        let contract = build_program_requirements_contract(ProgramContractInput {
            generated_at: &now_iso(),
            parsed: &parsed,
            requirements: &requirements,
            structure_spec: &structure,
            system_graph: &system_graph,
            resource_graph: &resource_graph,
            binding_stats: &binding_stats,
            required_contracts: &required,
            contract_blockers: &[],
            trace_blockers: &[],
            preflight_blockers: &[],
            locale: ArtifactLocale::ZhCn,
        });

        assert!(requirements.len() >= 6);
        assert!(requirements.iter().any(|req| req.id == "ENT-REQ-001"));
        assert_eq!(
            contract["consumer_stage"],
            json!("step_3_program_requirements")
        );
        assert!(contract["valid"].as_bool().unwrap());
        assert_schema_valid(
            &contract,
            "knowledge/schemas/program_requirements_contract.schema.json",
        );
    }

    #[test]
    fn requirement_binding_engine_fills_missing_bindings() {
        let contract = json!({
            "systems": [
                {"system_id": "SYS-COMBAT", "node_id": "combat_node", "system_name": "combat weapon system", "related_entities": ["ENT-001"]},
                {"system_id": "SYS-UI", "node_id": "ui_node", "system_name": "ui display system"}
            ],
            "entities": [
                {"entity_id": "ENT-001", "node_id": "combat_node", "source": "source:1"}
            ]
        });
        let mut requirements = vec![ProgramRequirement {
            id: "REQ-001".to_string(),
            requirement: "Implement weapon attack damage".to_string(),
            entity_id: "ENT-001".to_string(),
            entity_label: "Sword".to_string(),
            entity_kind: "weapon".to_string(),
            entity_schema: "weapon.v1".to_string(),
            selection_id: "SEL-001".to_string(),
            source_refs: vec!["source:1".to_string()],
            phase: "core_playable".to_string(),
            system_ids: Vec::new(),
            system_binding: json!({}),
            inputs: vec!["entity_definition".to_string()],
            outputs: vec!["weapon/Sword.asset".to_string()],
            dependencies: vec!["combat_node".to_string()],
            acceptance: "passes".to_string(),
            trace_kind: "design_entity".to_string(),
        }];

        let stats = RequirementBindingEngine::new(&contract).bind_missing(&mut requirements);

        assert_eq!(requirements[0].system_ids, vec!["SYS-COMBAT"]);
        assert_eq!(stats["binding_rate"], json!(1.0));
    }

    #[test]
    fn step04_converts_assets_and_builds_schema_valid_contract() {
        let parsed = sample_design();
        let assets = EntityToAssetConverter.convert(&parsed);
        let spec = build_asset_spec_contract(&assets);
        let gate = validate_asset_spec_contract(&spec);
        let market = MarketResearchSkill.local_fallback(&parsed);
        let required = required_contracts();
        let contract = build_art_requirements_contract(ArtContractInput {
            generated_at: &now_iso(),
            parsed: &parsed,
            assets: &assets,
            market_research: &market,
            asset_spec_gate: &gate,
            required_contracts: &required,
            contract_blockers: &[],
            locale: ArtifactLocale::ZhCn,
        });

        assert!(assets.iter().any(|asset| asset.asset_type == "effect"));
        assert!(assets.iter().any(|asset| asset.asset_type == "ui"));
        assert_eq!(gate["valid"], json!(true));
        assert_eq!(contract["consumer_stage"], json!("step_4_art_requirements"));
        assert_schema_valid(
            &contract,
            "knowledge/schemas/art_requirements_contract.schema.json",
        );
        assert_schema_valid(
            &asset_registry_document(&assets, &spec, ArtifactLocale::ZhCn),
            "knowledge/schemas/ai_design/asset_registry.schema.json",
        );
    }

    #[test]
    fn reviewer_reports_verdict_and_stable_issue_codes() {
        let reviewer = IntelligentReviewer::new();
        let blocked = reviewer.review_program(&[]);
        let warn = reviewer.review_art(&[ArtAssetRequirement {
            asset_id: "A-1".to_string(),
            name: "Only Concept".to_string(),
            asset_type: "art_asset".to_string(),
            source: "source".to_string(),
            source_entity_id: "ENT-1".to_string(),
            source_node_id: "node".to_string(),
            purpose: "Concrete purpose".to_string(),
            dependencies: vec!["node".to_string()],
            unlocks: Vec::new(),
            priority: "P0".to_string(),
            complexity: "m".to_string(),
            required_for_phase: "core_playable".to_string(),
            status: "requirement_defined".to_string(),
            trace_kind: "design_entity".to_string(),
            resolution: String::new(),
        }]);

        assert_eq!(blocked["verdict"], json!("BLOCKED"));
        assert_eq!(
            blocked["issues"][0]["code"],
            json!("PROGRAM_REQUIREMENTS_MISSING")
        );
        assert_eq!(blocked["issues"][0]["title"], json!("缺少程序需求"));
        assert_eq!(blocked["issues"][0]["return_target"], json!("03"));
        let blocked_result = review_result(
            "program_ai_review_report.json",
            &blocked,
            &json!({"artifact_locale": "zh-CN"}),
        );
        assert_eq!(blocked_result["status"], json!("blocked"));
        assert_eq!(
            blocked_result["blocking_issues"][0]["code"],
            json!("PROGRAM_REQUIREMENTS_MISSING")
        );
        assert_eq!(warn["verdict"], json!("WARN"));
        assert!(warn["warning_count"].as_u64().unwrap() > 0);
    }

    #[test]
    fn step03_06_localizes_user_facing_text_and_keeps_protocol_fields_stable() {
        let parsed = sample_design();
        let entities = extract_l5_entities(&parsed);
        let zh_requirements = EntityToRequirementConverter
            .convert_entities_with_locale(&entities, ArtifactLocale::ZhCn);
        let en_requirements = EntityToRequirementConverter
            .convert_entities_with_locale(&entities, ArtifactLocale::EnUs);
        let zh_assets =
            EntityToAssetConverter.convert_entities_with_locale(&entities, ArtifactLocale::ZhCn);
        let en_assets =
            EntityToAssetConverter.convert_entities_with_locale(&entities, ArtifactLocale::EnUs);

        assert!(zh_requirements[0].requirement.starts_with("实现 L5 实体"));
        assert!(zh_requirements[0].acceptance.contains("具备可执行数据"));
        assert!(
            en_requirements[0]
                .requirement
                .starts_with("Implement L5 entity")
        );
        assert!(en_requirements[0].acceptance.contains("executable data"));
        assert!(zh_assets[0].purpose.contains("提供"));
        assert!(en_assets[0].purpose.starts_with("Provide"));
        assert_eq!(zh_requirements[0].id, en_requirements[0].id);
        assert_eq!(
            zh_requirements[0].entity_schema,
            en_requirements[0].entity_schema
        );
        assert_eq!(zh_assets[0].asset_id, en_assets[0].asset_id);
        assert_eq!(zh_assets[0].asset_type, en_assets[0].asset_type);
    }

    #[test]
    fn step03_06_prefers_structured_entities_and_emits_explicit_review_protocol() {
        let parsed = sample_design();
        let structured_inputs = json!({
            "artifact_locale": "zh-CN",
            "status": "structured",
            "inputs": {
                "design_entities": {
                    "source": "stage_02/design_entities.json",
                    "nodes": [{
                        "node_id": "structured_guard_node",
                        "entities": [{
                            "id": "ENT-STRUCTURED-GUARD",
                            "label": "结构化守卫",
                            "kind": "character",
                            "schema": "character.v1",
                            "purpose": "负责阻挡玩家并响应战斗状态。"
                        }]
                    }]
                }
            }
        });
        let root = temp_root("structured_locale");
        seed_semantic_upstream(&root, ArtifactLocale::ZhCn);
        let step03 = root.join("stage_03");
        let step04 = root.join("stage_04");
        let step05 = root.join("stage_05");
        let step06 = root.join("stage_06");

        let result03 = Step03OutputGenerator
            .generate(STEP03, &parsed, &step03, &structured_inputs)
            .unwrap();
        let result04 = Step04OutputGenerator
            .generate(STEP04, &parsed, &step04, &structured_inputs)
            .unwrap();
        let result05 = Step05OutputGenerator
            .generate(STEP05, &parsed, &step05, &structured_inputs)
            .unwrap();
        let result06 = Step06OutputGenerator
            .generate(STEP06, &parsed, &step06, &structured_inputs)
            .unwrap();

        let requirements: Vec<ProgramRequirement> = serde_json::from_str(
            &fs::read_to_string(step03.join("program_requirements.json")).unwrap(),
        )
        .unwrap();
        let asset_registry: Value =
            serde_json::from_str(&fs::read_to_string(step04.join("asset_registry.json")).unwrap())
                .unwrap();
        let assets = asset_registry["assets"].as_array().unwrap();
        assert!(requirements.iter().all(|item| {
            item.entity_id == "ENT-STRUCTURED-GUARD" && item.requirement.contains("结构化守卫")
        }));
        assert!(assets.iter().any(|item| {
            item["source_entity_id"] == "ENT-STRUCTURED-GUARD"
                && item["purpose"]
                    .as_str()
                    .unwrap_or_default()
                    .contains("结构化守卫")
        }));
        assert!(assets.iter().any(|item| {
            item["trace_kind"] == "asset_strategy_matrix"
                && !item["unity_target_path"]
                    .as_str()
                    .unwrap_or_default()
                    .is_empty()
        }));
        for result in [&result03, &result04, &result05, &result06] {
            assert_eq!(result["artifact_locale"], json!("zh-CN"));
            assert!(result["review_items"].is_array());
            assert!(result["warnings"].is_array());
            assert!(result["semantic_quality"]["return_targets"].is_array());
        }
        for item in result06["review_items"].as_array().unwrap() {
            assert!(item["code"].is_string());
            assert!(item["return_target"].is_string());
            assert!(contains_han(item["title"].as_str().unwrap_or_default()));
            assert!(contains_han(item["message"].as_str().unwrap_or_default()));
            assert!(contains_han(item["reason"].as_str().unwrap_or_default()));
            assert!(contains_han(
                item["suggestion"].as_str().unwrap_or_default()
            ));
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stage_generators_write_step03_06_outputs() {
        let parsed = sample_design();
        let root = temp_root("generators");
        seed_semantic_upstream(&root, ArtifactLocale::ZhCn);

        let step03 = root.join("stage_03");
        let step04 = root.join("stage_04");
        let step05 = root.join("stage_05");
        let step06 = root.join("stage_06");
        let result03 = Step03OutputGenerator
            .generate(STEP03, &parsed, &step03, &json!({}))
            .unwrap();
        let result04 = Step04OutputGenerator
            .generate(STEP04, &parsed, &step04, &json!({}))
            .unwrap();
        let result05 = Step05OutputGenerator
            .generate(STEP05, &parsed, &step05, &json!({}))
            .unwrap();
        let result06 = Step06OutputGenerator
            .generate(STEP06, &parsed, &step06, &json!({}))
            .unwrap();

        assert!(step03.join("program_requirements_contract.json").exists());
        assert!(step04.join("art_requirements_contract.json").exists());
        assert!(step05.join("program_ai_review_report.json").exists());
        assert!(step06.join("art_ai_review_report.json").exists());
        assert!(step05.join("program_review_report.json").exists());
        assert!(step06.join("art_review_report.json").exists());
        assert_eq!(result03["content_exists"], json!(true));
        assert_eq!(result04["asset_spec_valid"], json!(true));
        assert_eq!(result05["verdict"], json!("PASS"));
        assert!(["PASS", "WARN"].contains(&result06["verdict"].as_str().unwrap()));
        assert_eq!(
            result05["review_report"],
            json!("program_ai_review_report.json")
        );
        assert_eq!(
            result06["review_report"],
            json!("art_ai_review_report.json")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step03_06_registered_artifacts_exist_and_match_declared_schemas() {
        let parsed = sample_design();
        let root = temp_root("registered_schema_contracts");
        seed_semantic_upstream(&root, ArtifactLocale::ZhCn);
        let inputs = json!({"artifact_locale": "zh-CN", "status": "structured"});
        for (stage, generator) in [
            (STEP03, generator_for_step(STEP03).unwrap()),
            (STEP04, generator_for_step(STEP04).unwrap()),
            (STEP05, generator_for_step(STEP05).unwrap()),
            (STEP06, generator_for_step(STEP06).unwrap()),
        ] {
            generator
                .generate(
                    stage,
                    &parsed,
                    &root.join(format!("stage_{stage:02}")),
                    &inputs,
                )
                .unwrap();
        }

        let repository_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
            .into_path();
        let registry =
            load_structured_file(&repository_root.join("pipeline/artifact_layer/registry.json"))
                .unwrap();
        let mut validated_count = 0usize;
        for stage_bundle in registry
            .get("artifacts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|bundle| {
                bundle
                    .get("stage")
                    .and_then(Value::as_u64)
                    .is_some_and(|stage| (3..=6).contains(&stage))
            })
        {
            let stage = stage_bundle["stage"].as_u64().unwrap() as u32;
            for schema_ref in stage_bundle
                .get("schema_refs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let registered_path = schema_ref["path"].as_str().unwrap();
                let file_name = Path::new(registered_path).file_name().unwrap();
                let schema_path = schema_ref["schema"].as_str().unwrap();
                let artifact_path = root.join(format!("stage_{stage:02}")).join(file_name);
                assert!(
                    artifact_path.exists(),
                    "registered artifact missing: {}",
                    artifact_path.display()
                );
                let payload: Value =
                    serde_json::from_str(&fs::read_to_string(&artifact_path).unwrap()).unwrap();
                assert_eq!(payload["artifact_locale"], json!("zh-CN"));
                assert_schema_valid(&payload, schema_path);
                validated_count += 1;
            }
        }
        assert_eq!(validated_count, registered_schema_contracts().len());
        let art_contract = read_test_json(&root.join("stage_04/art_requirements_contract.json"));
        let asset_registry = read_test_json(&root.join("stage_04/asset_registry.json"));
        let program_contract =
            read_test_json(&root.join("stage_03/program_requirements_contract.json"));
        assert_eq!(
            program_contract["program_capability_contract"],
            json!("stage_03/program_capability_contract.json")
        );
        assert_eq!(
            art_contract["source_files"]["asset_strategy_matrix"],
            json!("stage_04/asset_strategy_matrix.json")
        );
        for contract_asset in art_contract["assets"].as_array().unwrap() {
            let asset_id = contract_asset["asset_id"].as_str().unwrap();
            let registry_asset = asset_registry["assets"]
                .as_array()
                .unwrap()
                .iter()
                .find(|asset| asset["asset_id"] == asset_id)
                .unwrap();
            assert_eq!(
                contract_asset["production_specs"]["unity_target_path"],
                registry_asset["unity_target_path"],
                "asset target path drifted for {asset_id}"
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step03_04_block_when_frozen_project_dna_is_missing() {
        let parsed = sample_design();
        let root = temp_root("missing_project_dna");
        for (stage, generator) in [
            (STEP03, generator_for_step(STEP03).unwrap()),
            (STEP04, generator_for_step(STEP04).unwrap()),
        ] {
            let result = generator
                .generate(
                    stage,
                    &parsed,
                    &root.join(format!("stage_{stage:02}")),
                    &json!({"artifact_locale": "zh-CN"}),
                )
                .unwrap();
            assert_eq!(result["status"], json!("blocked"));
            assert!(
                result["blocking_issues"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|item| {
                        item["code"] == "PROJECT_DNA_CONTRACT_MISSING"
                            && item["return_target"] == "02"
                    })
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step03_06_keeps_en_us_as_an_explicit_artifact_language_extension() {
        let parsed = sample_design();
        let root = temp_root("english_artifacts");
        seed_semantic_upstream(&root, ArtifactLocale::EnUs);
        let inputs = json!({"artifact_locale": "en-US", "status": "structured"});
        for stage in STEP03..=STEP06 {
            generator_for_step(stage)
                .unwrap()
                .generate(
                    stage,
                    &parsed,
                    &root.join(format!("stage_{stage:02}")),
                    &inputs,
                )
                .unwrap();
        }
        let capability = read_test_json(&root.join("stage_03/program_capability_contract.json"));
        let registry = read_test_json(&root.join("stage_04/asset_registry.json"));
        let slice_spec = read_test_json(&root.join("stage_04/ui_slice_spec_contract.json"));
        let program_review = read_test_json(&root.join("stage_05/program_ai_review_report.json"));
        let art_review = read_test_json(&root.join("stage_06/art_ai_review_report.json"));

        assert_eq!(capability["artifact_locale"], json!("en-US"));
        assert!(
            capability["capabilities"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| {
                    item["state_changes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .all(|change| {
                            !contains_han(change["description"].as_str().unwrap_or_default())
                        })
                })
        );
        assert!(registry["assets"].as_array().unwrap().iter().any(|asset| {
            asset["trace_kind"] == "asset_strategy_matrix"
                && asset["purpose"]
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("Provide")
        }));
        assert!(
            slice_spec["policy"]
                .as_str()
                .unwrap_or_default()
                .starts_with("Python emits metadata")
        );
        assert_eq!(
            program_review["title"],
            json!("Program Requirements Review")
        );
        assert_eq!(art_review["title"], json!("Art Requirements Review"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn response_entity_parser_keeps_step02_ai_output_compatible_for_a19() {
        let entities = parse_response_entities(
            r#"{"supplemented_entities":[{"label":"Combat HUD","kind":"ui","schema":"ui.v1","node_id":"ui_node"}]}"#,
        );
        let assets = EntityToAssetConverter.convert_entities(&entities);

        assert_eq!(entities.len(), 1);
        assert_eq!(assets[0].asset_type, "ui");
    }

    fn sample_design() -> ParsedDesignSource {
        parse_design_text(
            r#"# Action Prototype

## Layer 2 Systems
Selected / Stable
- 玩法系统: Combat Weapon System
  目的: weapon attack damage and hit response
  依赖: combat_node
- 玩法系统: Room Flow System
  目的: room encounter and reward routing
  依赖: room_node

## Layer 5 Entities
Frozen / Traceable
- L5实体: Shadow Sword
  目的: schema=weapon.v1；kind=weapon；status=precise
  依赖: combat_node
- L5实体: Dash Slash
  目的: schema=ability.v1；kind=ability；status=precise
  依赖: combat_node
- L5实体: Arena Room
  目的: schema=room.v1；kind=room；status=precise
  依赖: room_node
"#,
            "sample.md",
            "",
            None,
            None,
        )
    }

    fn seed_semantic_upstream(root: &Path, locale: ArtifactLocale) {
        let stage01 = root.join("stage_01");
        let stage02 = root.join("stage_02");
        fs::create_dir_all(&stage01).unwrap();
        fs::create_dir_all(&stage02).unwrap();
        write_json(
            &stage01.join("archetype_requirements.json"),
            &json!({
                "schema_version": "1.0",
                "artifact_locale": locale,
                "detected_archetype": "action_roguelite",
                "project_signature": "sample-project-signature",
                "required_assets": [
                    {"asset_role": "combat_feedback", "consumer": "CombatRuntime"},
                    {"asset_role": "hud_status", "consumer": "HudRuntime"}
                ],
            }),
        )
        .unwrap();
        write_json(
            &stage02.join("project_dna_contract.json"),
            &json!({
                "schema_version": "1.0",
                "generated_at": now_iso(),
                "artifact_locale": locale,
                "contract_state": "frozen",
                "status": "frozen",
                "project_id": "sample-project",
                "project_name": if locale == ArtifactLocale::ZhCn { "示例动作项目" } else { "Sample Action Project" },
                "project_signature": "sample-project-signature",
                "core_entities": [
                    {"entity_id": "shadow_sword", "role": "core_weapon"},
                    {"entity_id": "arena_room", "role": "playable_space"}
                ],
                "runtime_systems": [
                    {"system_id": "CombatRuntime"},
                    {"system_id": "RoomFlowRuntime"}
                ],
                "player_actions": [
                    {"action_id": "dash_slash", "state_change": "enemy_health_changes"}
                ],
                "resources": [
                    {"resource_id": "player_health"}
                ],
                "objectives": [
                    {"objective_id": "clear_arena", "completion_condition": "all_enemies_defeated", "failure_condition": "player_health_zero"}
                ],
                "ui_surfaces": [
                    {"surface_id": "combat_hud", "purpose": "health and objective feedback"}
                ],
                "asset_needs": [
                    {"asset_role": "combat_feedback", "consumer": "CombatRuntime"},
                    {"asset_role": "hud_status", "consumer": "HudRuntime"}
                ],
                "acceptance_scenarios": [
                    {"scenario_id": "first_arena", "expected": "player clears the arena and sees completion feedback"}
                ],
                "source_refs": ["stage_02/playable_contracts/core_playable_contract.json"],
                "blockers": []
            }),
        )
        .unwrap();
    }

    fn registered_schema_contracts() -> Vec<(u32, &'static str, &'static str)> {
        vec![
            (
                STEP03,
                "program_requirements_contract.json",
                "knowledge/schemas/program_requirements_contract.schema.json",
            ),
            (
                STEP03,
                "program_requirement_trace_report.json",
                "knowledge/schemas/ai_design/program_requirement_trace_report.schema.json",
            ),
            (
                STEP03,
                "program_structure_spec.json",
                "knowledge/schemas/ai_design/program_structure_spec.schema.json",
            ),
            (
                STEP03,
                "program_capability_contract.json",
                "knowledge/schemas/ai_design/program_capability_contract.schema.json",
            ),
            (
                STEP03,
                "program_semantic_coverage_report.json",
                "knowledge/schemas/ai_design/program_semantic_coverage_report.schema.json",
            ),
            (
                STEP03,
                "customization_score_report.json",
                "knowledge/schemas/ai_design/customization_score_report.schema.json",
            ),
            (
                STEP04,
                "asset_spec_contract.json",
                "knowledge/schemas/ai_design/asset_spec_contract.schema.json",
            ),
            (
                STEP04,
                "art_requirements_contract.json",
                "knowledge/schemas/art_requirements_contract.schema.json",
            ),
            (
                STEP04,
                "asset_registry.json",
                "knowledge/schemas/ai_design/asset_registry.schema.json",
            ),
            (
                STEP04,
                "asset_requirements_resolved.json",
                "knowledge/schemas/ai_design/asset_requirements_resolved.schema.json",
            ),
            (
                STEP04,
                "unity_asset_mount_plan.json",
                "knowledge/schemas/ai_design/unity_asset_mount_plan.schema.json",
            ),
            (
                STEP04,
                "audio_placeholder_plan.json",
                "knowledge/schemas/ai_design/audio_placeholder_plan.schema.json",
            ),
            (
                STEP04,
                "image_consumable_spec.json",
                "knowledge/schemas/ai_design/art_pipeline/image_consumable_spec.schema.json",
            ),
            (
                STEP04,
                "ui_slice_spec_contract.json",
                "knowledge/schemas/ai_design/art_pipeline/ui_slice_spec_contract.schema.json",
            ),
            (
                STEP04,
                "unity_import_policy.json",
                "knowledge/schemas/ai_design/art_pipeline/unity_import_policy.schema.json",
            ),
            (
                STEP04,
                "asset_usage_binding_seed.json",
                "knowledge/schemas/ai_design/art_pipeline/asset_usage_binding_seed.schema.json",
            ),
            (
                STEP04,
                "audio_placeholder_requirements.json",
                "knowledge/schemas/ai_design/art_pipeline/audio_placeholder_requirements.schema.json",
            ),
            (
                STEP04,
                "art_taxonomy_contract.json",
                "knowledge/schemas/ai_design/art_taxonomy_contract.schema.json",
            ),
            (
                STEP04,
                "asset_strategy_matrix.json",
                "knowledge/schemas/ai_design/asset_strategy_matrix.schema.json",
            ),
            (
                STEP04,
                "customization_score_report.json",
                "knowledge/schemas/ai_design/customization_score_report.schema.json",
            ),
            (
                STEP05,
                "program_ai_review_report.json",
                "knowledge/schemas/ai_design/program_ai_review_report.schema.json",
            ),
            (
                STEP05,
                "program_semantic_review_report.json",
                "knowledge/schemas/ai_design/program_semantic_review_report.schema.json",
            ),
            (
                STEP05,
                "customization_score_report.json",
                "knowledge/schemas/ai_design/customization_score_report.schema.json",
            ),
            (
                STEP06,
                "art_ai_review_report.json",
                "knowledge/schemas/ai_design/art_ai_review_report.schema.json",
            ),
            (
                STEP06,
                "art_semantic_review_report.json",
                "knowledge/schemas/ai_design/art_pipeline/art_semantic_review_report.schema.json",
            ),
            (
                STEP06,
                "customization_score_report.json",
                "knowledge/schemas/ai_design/customization_score_report.schema.json",
            ),
        ]
    }

    fn assert_schema_valid(contract: &Value, schema_path: &str) {
        let root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        let schema = load_structured_file(&root.join(schema_path).unwrap()).unwrap();
        let errors = validate_contract(contract, &schema);
        assert!(errors.is_empty(), "{errors:?}");
    }

    fn read_test_json(path: &Path) -> Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir()
            .join("adm_new_pipeline_step03_06")
            .join(label)
            .join(new_stable_id("root").unwrap())
    }

    fn contains_han(value: &str) -> bool {
        value
            .chars()
            .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
    }
}
