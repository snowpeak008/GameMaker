use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use adm_new_contracts::ArtifactLocale;
use adm_new_design::contracts::{
    build_customization_score_report_with_locale, build_open_questions_contract_with_locale,
    build_playable_contract_bundle_from_decisions_with_locale,
    build_playable_contract_bundle_with_locale, build_playable_scenario_contract_with_locale,
    build_project_dna_seed_with_locale, build_project_identity_with_locale,
    freeze_project_dna_with_locale, unresolved_blocking_questions,
    validate_playable_contract_bundle,
};
use adm_new_design::semantic_pipeline::{
    ArchetypeCatalog, build_archetype_requirements_with_locale,
    build_semantic_coverage_seed_with_locale,
};
use adm_new_foundation::io::{now_iso, read_json, write_json, write_text};
use adm_new_foundation::{AdmError, AdmResult, sha256_hex};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::generation::{
    ParsedDesignSource, Selection, StageOutputGenerator, artifact_locale_from_inputs,
    is_l5_entity_item_type, is_l5_node_item_type, localized_text,
};
use crate::source::SourceGroup;

pub const STEP00: u32 = 0;
pub const STEP01: u32 = 1;
pub const STEP02: u32 = 2;

pub const STEP00_CONCEPT_GROUP: &str = "concept";
pub const STEP01_GAMEPLAY_HISTORY_GROUP: &str = "gameplay_framework_history";
pub const STEP02_SUBSYSTEM_DESIGN_GROUP: &str = "2a_subsystem_design";
pub const STEP02_AI_DESIGN_SCRIPT_GROUP: &str = "2b_ai_design_script";
pub const STEP02_DESIGN_PACKAGE_GROUP: &str = "2c_design_package";
pub const STEP02_DEVELOPMENT_DESIGN_GROUP: &str = "2c_development_design";

pub const VALID_ENTITY_KINDS: &[&str] = &[
    "weapon",
    "character",
    "enemy",
    "ability",
    "room",
    "resource",
    "ui",
    "scene",
    "system",
    "loop",
    "numeric_curve",
    "content",
    "encounter",
    "config",
    "audio",
    "design_selection",
];

pub const DEFAULT_TARGET_KINDS: &[&str] = &["weapon", "character", "ability", "room", "enemy"];
pub const MISSING_NODE_FALLBACK_LIMIT: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagePluginSpec {
    pub stage_id: &'static str,
    pub source_groups: Vec<SourceGroup>,
    pub test_mode_status: &'static str,
    pub generation_entrypoint: &'static str,
}

pub fn step00_plugin_spec() -> StagePluginSpec {
    StagePluginSpec {
        stage_id: "00",
        source_groups: vec![SourceGroup {
            label: STEP00_CONCEPT_GROUP.to_string(),
            patterns: vec!["devflow_Concept_*".to_string()],
            mode: "latest".to_string(),
            required: false,
            source_ids: vec!["Concept".to_string()],
        }],
        test_mode_status: "success",
        generation_entrypoint: "apply_development_plan_outputs",
    }
}

pub fn step01_plugin_spec() -> StagePluginSpec {
    StagePluginSpec {
        stage_id: "01",
        source_groups: vec![SourceGroup {
            label: STEP01_GAMEPLAY_HISTORY_GROUP.to_string(),
            patterns: vec!["devflow_GameplayFramework_*".to_string()],
            mode: "all".to_string(),
            required: false,
            source_ids: vec!["GameplayFramework".to_string()],
        }],
        test_mode_status: "success",
        generation_entrypoint: "apply_development_plan_outputs",
    }
}

pub fn step02_plugin_spec() -> StagePluginSpec {
    StagePluginSpec {
        stage_id: "02",
        source_groups: vec![
            SourceGroup {
                label: STEP02_SUBSYSTEM_DESIGN_GROUP.to_string(),
                patterns: vec!["devflow_SubsystemDesign_*".to_string()],
                mode: "latest".to_string(),
                required: false,
                source_ids: vec!["SubsystemDesign".to_string()],
            },
            SourceGroup {
                label: STEP02_AI_DESIGN_SCRIPT_GROUP.to_string(),
                patterns: vec!["devflow_AIDesignScript_*".to_string()],
                mode: "latest".to_string(),
                required: false,
                source_ids: vec!["AIDesignScript".to_string()],
            },
            SourceGroup {
                label: STEP02_DESIGN_PACKAGE_GROUP.to_string(),
                patterns: vec!["devflow_Design_*".to_string()],
                mode: "latest".to_string(),
                required: false,
                source_ids: vec!["Design".to_string()],
            },
            SourceGroup {
                label: STEP02_DEVELOPMENT_DESIGN_GROUP.to_string(),
                patterns: vec!["devflow_DevelopmentDesign_*".to_string()],
                mode: "latest".to_string(),
                required: false,
                source_ids: vec!["DevelopmentDesign".to_string()],
            },
        ],
        test_mode_status: "success",
        generation_entrypoint: "apply_development_plan_outputs",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreQuestion {
    pub id: String,
    pub domain: String,
    pub question: String,
    #[serde(default)]
    pub item_types: Vec<String>,
    #[serde(default)]
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuestionCoverageReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub source: String,
    pub total_questions: usize,
    pub answered_questions: usize,
    pub unanswered_questions: usize,
    pub coverage_rate: f64,
    pub target_coverage_rate: f64,
    pub needs_ai_supplement: bool,
    pub questions: Vec<QuestionEvaluation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionEvaluation {
    pub id: String,
    pub domain: String,
    pub question: String,
    pub answered: bool,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub label: String,
    pub source: String,
    #[serde(rename = "match")]
    pub match_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConceptProfile {
    pub schema_version: u32,
    pub generated_at: String,
    pub source: String,
    pub project_positioning: ProfileItem,
    pub core_loop: ProfileItem,
    pub key_constraints: Vec<ProfileItem>,
    pub key_systems: Vec<ProfileItem>,
    pub selected_item_count: usize,
    pub fallback_used: bool,
    pub genre_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProfileItem {
    pub label: String,
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Default)]
pub struct ConceptProcessor;

impl ConceptProcessor {
    pub fn build_profile(&self, parsed: &ParsedDesignSource) -> ConceptProfile {
        self.build_profile_with_locale(parsed, ArtifactLocale::ZhCn)
    }

    pub fn build_profile_with_locale(
        &self,
        parsed: &ParsedDesignSource,
        locale: ArtifactLocale,
    ) -> ConceptProfile {
        let selections = &parsed.selections;
        let mut project_positioning = first_matching(
            selections,
            &[
                "项目定位",
                "游戏类型",
                "玩法想法",
                "project positioning",
                "game type",
                "game concept",
            ],
        );
        let mut core_loop = first_matching(selections, &["核心循环", "core loop"]);
        let fallback_source = if parsed.source.is_empty() {
            "fallback".to_string()
        } else {
            parsed.source.clone()
        };

        if project_positioning.label.is_empty() {
            project_positioning = ProfileItem {
                label: first_chars(&parsed.raw_text, 80)
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| {
                        localized_text(locale, "待补充项目定位", "Project positioning required")
                            .to_string()
                    }),
                source: fallback_source.clone(),
                confidence: "fallback".to_string(),
            };
        }
        if core_loop.label.is_empty() {
            core_loop = ProfileItem {
                label: fallback_loop_with_locale(&parsed.raw_text, locale),
                source: fallback_source,
                confidence: "fallback".to_string(),
            };
        }

        ConceptProfile {
            schema_version: 1,
            generated_at: now_iso(),
            source: parsed.source.clone(),
            project_positioning,
            core_loop,
            key_constraints: matching_items(
                selections,
                &[
                    "平台",
                    "商业模式",
                    "技术",
                    "约束",
                    "资源",
                    "platform",
                    "business model",
                    "technology",
                    "constraint",
                    "resource",
                ],
                8,
            ),
            key_systems: matching_items(
                selections,
                &[
                    "system_layer",
                    "玩法系统",
                    "系统图",
                    "游戏系统",
                    "system map",
                    "gameplay system",
                ],
                8,
            ),
            selected_item_count: selections.len(),
            fallback_used: selections.is_empty(),
            genre_key: genre_key(&parsed.raw_text, selections),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuestionEngine {
    questions: Vec<CoreQuestion>,
}

impl Default for QuestionEngine {
    fn default() -> Self {
        Self {
            questions: default_core_questions(),
        }
    }
}

impl QuestionEngine {
    pub fn new(questions: Vec<CoreQuestion>) -> Self {
        Self { questions }
    }

    pub fn from_path(path: &Path) -> Self {
        Self {
            questions: load_core_questions(path),
        }
    }

    pub fn evaluate(&self, parsed: &ParsedDesignSource) -> QuestionCoverageReport {
        self.evaluate_with_locale(parsed, ArtifactLocale::ZhCn)
    }

    pub fn evaluate_with_locale(
        &self,
        parsed: &ParsedDesignSource,
        locale: ArtifactLocale,
    ) -> QuestionCoverageReport {
        let questions = if self.questions.is_empty() {
            default_core_questions()
        } else {
            self.questions.clone()
        };
        let evaluations = questions
            .iter()
            .map(|question| {
                let mut evidence = evidence_for(question, &parsed.selections, &parsed.raw_text);
                if locale == ArtifactLocale::ZhCn {
                    for item in &mut evidence {
                        if item.match_kind == "genre_inference" {
                            item.label =
                                localized_genre_evidence(&question.id, &item.label, locale);
                        }
                    }
                }
                QuestionEvaluation {
                    id: question.id.clone(),
                    domain: question.domain.clone(),
                    question: localized_core_question(&question.id, &question.question, locale),
                    answered: !evidence.is_empty(),
                    evidence: evidence.into_iter().take(5).collect(),
                }
            })
            .collect::<Vec<_>>();
        let answered = evaluations.iter().filter(|item| item.answered).count();
        let total = evaluations.len();
        let coverage_rate = ratio(answered, total);
        QuestionCoverageReport {
            schema_version: 1,
            generated_at: now_iso(),
            source: parsed.source.clone(),
            total_questions: total,
            answered_questions: answered,
            unanswered_questions: total.saturating_sub(answered),
            coverage_rate,
            target_coverage_rate: 0.55,
            needs_ai_supplement: total > 0 && coverage_rate < 0.4,
            questions: evaluations,
        }
    }
}

pub fn load_core_questions(path: &Path) -> Vec<CoreQuestion> {
    let value = read_json(path, json!([]));
    let questions = serde_json::from_value::<Vec<CoreQuestion>>(value).unwrap_or_default();
    if questions.is_empty() {
        default_core_questions()
    } else {
        questions
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenreTemplate {
    #[serde(default)]
    pub core_loop: Vec<String>,
    #[serde(default)]
    pub systems: Vec<SystemDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemDefinition {
    pub id: String,
    pub name: String,
    pub responsibility: String,
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreLoopReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub source: String,
    pub template_key: String,
    pub source_kind: String,
    pub loop_nodes: Vec<String>,
    pub output_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemDefinitionsReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub source: String,
    pub template_key: String,
    pub systems: Vec<SystemDefinition>,
    pub system_count: usize,
    pub definition_rate: f64,
}

#[derive(Debug, Clone, Default)]
pub struct TemplateLibrary {
    templates: BTreeMap<String, GenreTemplate>,
}

impl TemplateLibrary {
    pub fn default_templates() -> Self {
        Self {
            templates: default_genre_templates(),
        }
    }

    pub fn from_path(path: &Path) -> Self {
        let value = read_json(path, json!({}));
        let templates =
            serde_json::from_value::<BTreeMap<String, GenreTemplate>>(value).unwrap_or_default();
        if templates.is_empty() {
            Self::default_templates()
        } else {
            Self { templates }
        }
    }

    pub fn get(&self, key: &str) -> GenreTemplate {
        self.templates
            .get(key)
            .or_else(|| self.templates.get("generic"))
            .cloned()
            .unwrap_or_else(generic_template)
    }
}

#[derive(Debug, Clone)]
pub struct LoopExtractor {
    templates: TemplateLibrary,
}

impl Default for LoopExtractor {
    fn default() -> Self {
        Self {
            templates: TemplateLibrary::default_templates(),
        }
    }
}

impl LoopExtractor {
    pub fn new(templates: TemplateLibrary) -> Self {
        Self { templates }
    }

    pub fn extract(&self, parsed: &ParsedDesignSource) -> CoreLoopReport {
        self.extract_with_locale(parsed, ArtifactLocale::ZhCn)
    }

    pub fn extract_with_locale(
        &self,
        parsed: &ParsedDesignSource,
        locale: ArtifactLocale,
    ) -> CoreLoopReport {
        let explicit_loop = explicit_core_loop(&parsed.selections);
        let template_key = pick_genre_template_key(&parsed.raw_text, &parsed.selections);
        let template = if locale == ArtifactLocale::EnUs {
            english_genre_template(&template_key)
        } else {
            self.templates.get(&template_key)
        };
        let mut loop_nodes = if explicit_loop.is_empty() {
            template.core_loop
        } else {
            explicit_loop
        };
        if loop_nodes.is_empty() {
            loop_nodes = localized_loop_fallback(locale);
        }
        CoreLoopReport {
            schema_version: 1,
            generated_at: now_iso(),
            source: parsed.source.clone(),
            template_key,
            source_kind: if explicit_core_loop(&parsed.selections).is_empty() {
                "template_fallback".to_string()
            } else {
                "explicit".to_string()
            },
            output_rate: if loop_nodes.is_empty() { 0.0 } else { 1.0 },
            loop_nodes,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SystemDeducer {
    templates: TemplateLibrary,
}

impl Default for SystemDeducer {
    fn default() -> Self {
        Self {
            templates: TemplateLibrary::default_templates(),
        }
    }
}

impl SystemDeducer {
    pub fn new(templates: TemplateLibrary) -> Self {
        Self { templates }
    }

    pub fn deduce(
        &self,
        parsed: &ParsedDesignSource,
        system_graph: &Value,
    ) -> SystemDefinitionsReport {
        self.deduce_with_locale(parsed, system_graph, ArtifactLocale::ZhCn)
    }

    pub fn deduce_with_locale(
        &self,
        parsed: &ParsedDesignSource,
        system_graph: &Value,
        locale: ArtifactLocale,
    ) -> SystemDefinitionsReport {
        let mut systems = systems_from_graph(system_graph, locale);
        let template_key = pick_genre_template_key(&parsed.raw_text, &parsed.selections);
        let template = if locale == ArtifactLocale::EnUs {
            english_genre_template(&template_key)
        } else {
            self.templates.get(&template_key)
        };
        let mut existing_names = systems
            .iter()
            .map(|item| item.name.clone())
            .collect::<BTreeSet<_>>();
        for system in template.systems {
            if system.name.is_empty() || !existing_names.insert(system.name.clone()) {
                continue;
            }
            systems.push(SystemDefinition {
                id: if system.id.is_empty() {
                    format!("SYS-FALLBACK-{:03}", systems.len() + 1)
                } else {
                    system.id
                },
                name: system.name,
                responsibility: if system.responsibility.is_empty() {
                    localized_text(
                        locale,
                        "提供玩法系统能力。",
                        "Provide gameplay system capability.",
                    )
                    .to_string()
                } else {
                    system.responsibility
                },
                source: "genre_template".to_string(),
                confidence: "fallback".to_string(),
            });
        }
        systems.truncate(8);
        let system_count = systems.len();
        SystemDefinitionsReport {
            schema_version: 1,
            generated_at: now_iso(),
            source: parsed.source.clone(),
            template_key,
            systems,
            system_count,
            definition_rate: if system_count >= 5 {
                1.0
            } else {
                ratio(system_count, 5)
            },
        }
    }
}

pub fn pick_genre_template_key(raw_text: &str, selections: &[Selection]) -> String {
    let haystack = selection_haystack(raw_text, selections);
    for (genre, tokens) in [
        (
            "roguelike_action",
            &["rogue", "肉鸽", "roguelite", "roguelike", "hades"][..],
        ),
        ("fps", &["fps", "射击", "枪", "shooter"]),
        ("puzzle", &["puzzle", "解谜", "match", "消除"]),
        ("strategy", &["strategy", "rts", "4x", "策略", "战棋"]),
        ("rpg", &["rpg", "jrpg", "arpg", "role-playing", "角色扮演"]),
        ("moba", &["moba", "推塔", "对线"]),
    ] {
        if tokens.iter().any(|token| haystack.contains(token)) {
            return genre.to_string();
        }
    }
    "generic".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignEntity {
    #[serde(default)]
    pub entity_id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub schema: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub source_selection_id: String,
    #[serde(default)]
    pub node_id: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub purpose: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supplement_basis: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_from: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StructuredDesignEntities {
    entities: Vec<DesignEntity>,
    node_ids: Vec<String>,
}

/// Returns the canonical entity set for downstream stages. The structured D4
/// contract is authoritative; localized Markdown remains a compatibility path.
pub fn preferred_design_entities(
    parsed: &ParsedDesignSource,
    structured_inputs: &Value,
) -> Vec<DesignEntity> {
    structured_design_entities(structured_inputs)
        .filter(|value| !value.entities.is_empty())
        .map(|value| value.entities)
        .unwrap_or_else(|| extract_l5_entities(parsed))
}

fn structured_design_entities(structured_inputs: &Value) -> Option<StructuredDesignEntities> {
    let contract = structured_inputs
        .get("inputs")
        .and_then(|inputs| inputs.get("design_entities"))?;
    let nodes = contract.get("nodes").and_then(Value::as_array)?;
    let contract_source = non_empty_or(
        string_field(contract, "source"),
        "structured/design_entities.json",
    );
    let mut node_ids = Vec::new();
    let mut entities = Vec::new();
    for node in nodes {
        let node_id = string_field(node, "node_id");
        if node_id.is_empty() {
            continue;
        }
        node_ids.push(node_id.clone());
        let Some(raw_entities) = node.get("entities").and_then(Value::as_array) else {
            continue;
        };
        for raw in raw_entities {
            if !raw.is_object() {
                continue;
            }
            let index = entities.len() + 1;
            let entity_id = non_empty_or(
                non_empty_or(string_field(raw, "id"), &string_field(raw, "entity_id")),
                &format!("ENT-{index:03}"),
            );
            let kind = string_field(raw, "kind");
            let schema = structured_entity_schema(raw, &kind);
            let dependencies = {
                let values = string_array(raw.get("dependencies"));
                if values.is_empty() {
                    vec![node_id.clone()]
                } else {
                    values
                }
            };
            entities.push(DesignEntity {
                entity_id,
                label: first_non_empty_field(raw, &["label", "name", "id", "entity_id", "kind"]),
                kind,
                schema: non_empty_or(schema, "unknown"),
                status: non_empty_or(string_field(raw, "status"), "precise"),
                source: format!("{contract_source}#{node_id}"),
                source_selection_id: node_id.clone(),
                node_id: node_id.clone(),
                dependencies,
                purpose: first_non_empty_field(
                    raw,
                    &[
                        "purpose",
                        "design_rationale",
                        "supplement_basis",
                        "description",
                    ],
                ),
                inference: None,
                supplement_basis: optional_non_empty_field(raw, "supplement_basis"),
                completed_from: optional_non_empty_field(raw, "completed_from"),
            });
        }
    }
    node_ids.sort();
    node_ids.dedup();
    Some(StructuredDesignEntities { entities, node_ids })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntityCoverageReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub source: String,
    pub entities: Vec<DesignEntity>,
    pub entity_count: usize,
    pub concrete_node_count: usize,
    pub covered_concrete_nodes: usize,
    pub entity_coverage_rate: f64,
    pub target_coverage_rate: f64,
    pub coverage_basis: String,
    pub missing_entities: Vec<MissingEntity>,
    pub invalid_entities: Vec<InvalidEntity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_supplement: Option<SupplementMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingEntity {
    pub node_id: String,
    pub reason: String,
    pub expected_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidEntity {
    pub entity_id: String,
    pub label: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplementMeta {
    pub triggered: bool,
    pub trigger_reason: String,
    pub mode: String,
    pub entities_added: usize,
    pub entities_completed: usize,
    pub cache_hit: bool,
    pub adapter: String,
    pub fallback_used: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub error: String,
    pub supplement_basis_samples: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplementRequest {
    pub project_name: String,
    pub genre: String,
    pub core_loop: Vec<String>,
    pub systems: Vec<SystemDefinition>,
    pub existing_entities: Vec<DesignEntity>,
    pub l4_decisions: BTreeMap<String, String>,
    pub target_kinds: Vec<String>,
    pub min_per_kind: BTreeMap<String, usize>,
    pub missing_node_ids: Vec<String>,
    pub known_node_ids: BTreeMap<String, String>,
    pub request_hash: String,
    pub schema_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupplementResult {
    pub entities: Vec<DesignEntity>,
    pub added_count: usize,
    pub completed_count: usize,
    pub cache_hit: bool,
    pub adapter_used: String,
    pub fallback_used: bool,
    pub mode: String,
    pub error: String,
    pub supplement_basis_samples: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EntityValidator {
    supplement_adapter: Option<EntitySupplementAdapter>,
}

impl EntityValidator {
    pub fn new(supplement_adapter: Option<EntitySupplementAdapter>) -> Self {
        Self { supplement_adapter }
    }

    pub fn validate(&self, parsed: &ParsedDesignSource) -> EntityCoverageReport {
        self.validate_with_inputs(parsed, &Value::Null, ArtifactLocale::ZhCn)
    }

    pub fn validate_with_inputs(
        &self,
        parsed: &ParsedDesignSource,
        structured_inputs: &Value,
        locale: ArtifactLocale,
    ) -> EntityCoverageReport {
        let structured = structured_design_entities(structured_inputs);
        let structured_used = structured
            .as_ref()
            .is_some_and(|value| !value.entities.is_empty());
        let mut entities = structured
            .as_ref()
            .filter(|value| !value.entities.is_empty())
            .map(|value| value.entities.clone())
            .unwrap_or_else(|| extract_l5_entities(parsed));
        localize_inferred_entities(&mut entities, locale);
        let (expected_node_ids, has_explicit_l5_nodes) = if let Some(structured) = structured
            .as_ref()
            .filter(|value| !value.entities.is_empty())
        {
            (structured.node_ids.clone(), true)
        } else {
            expected_node_ids(parsed)
        };
        let mut expected_node_ids = if has_explicit_l5_nodes {
            expected_node_ids
        } else {
            filter_governance_nodes(expected_node_ids, parsed)
        };
        expected_node_ids.sort();
        expected_node_ids.dedup();
        let pre_concrete_nodes = entity_node_ids(&entities);
        let mut expected_total = expected_node_count(parsed, &entities);
        let coverage_basis = if structured_used {
            "structured_design_entities"
        } else if has_explicit_l5_nodes {
            "explicit_l5_nodes"
        } else if !expected_node_ids.is_empty() {
            "legacy_expected_node_ids"
        } else if summary_positive_count(parsed, "design_entity_node_count").is_some() {
            "design_entity_node_count"
        } else if summary_positive_count(parsed, "node_count").is_some() {
            "legacy_node_count"
        } else if entities.iter().any(|entity| entity.inference.is_some()) {
            "selection_fallback"
        } else {
            "detected_entity_nodes"
        }
        .to_string();
        if !expected_node_ids.is_empty() {
            expected_total = expected_node_ids.len();
        }
        let pre_covered_nodes = if expected_node_ids.is_empty() {
            pre_concrete_nodes.len().min(expected_total)
        } else {
            expected_node_ids
                .iter()
                .filter(|node_id| pre_concrete_nodes.contains(*node_id))
                .count()
        };
        let pre_coverage_rate = ratio(pre_covered_nodes, expected_total);
        let mut supplement_meta = None;

        if let Some(adapter) = &self.supplement_adapter {
            let (should_run, trigger_reason) = supplement_trigger_reason_with_locale(
                &entities,
                pre_coverage_rate,
                &adapter.adapter_name,
                locale,
            );
            if should_run {
                let mut result = adapter.supplement(
                    &entities,
                    parsed,
                    &expected_node_ids
                        .iter()
                        .filter(|node_id| !pre_concrete_nodes.contains(*node_id))
                        .cloned()
                        .collect::<Vec<_>>(),
                );
                localize_supplement_result(&mut result, locale);
                entities = result.entities;
                supplement_meta = Some(SupplementMeta {
                    triggered: true,
                    trigger_reason,
                    mode: result.mode,
                    entities_added: result.added_count,
                    entities_completed: result.completed_count,
                    cache_hit: result.cache_hit,
                    adapter: result.adapter_used,
                    fallback_used: result.fallback_used,
                    error: result.error,
                    supplement_basis_samples: result.supplement_basis_samples,
                });
            } else {
                supplement_meta = Some(SupplementMeta {
                    triggered: false,
                    trigger_reason,
                    mode: "skipped".to_string(),
                    entities_added: 0,
                    entities_completed: 0,
                    cache_hit: false,
                    adapter: adapter.adapter_name.clone(),
                    fallback_used: false,
                    error: String::new(),
                    supplement_basis_samples: Vec::new(),
                });
            }
        }

        let concrete_nodes = entity_node_ids(&entities);
        let (total_nodes, covered_nodes, missing_seed) = if expected_node_ids.is_empty() {
            let total_nodes = expected_node_count(parsed, &entities);
            (
                total_nodes,
                concrete_nodes.len().min(total_nodes),
                Vec::new(),
            )
        } else {
            let covered = expected_node_ids
                .iter()
                .filter(|node_id| concrete_nodes.contains(*node_id))
                .count();
            let missing = expected_node_ids
                .iter()
                .filter(|node_id| !concrete_nodes.contains(*node_id))
                .map(|node_id| MissingEntity {
                    node_id: node_id.clone(),
                    reason: localized_text(
                        locale,
                        "该预期设计节点尚未映射 L5 实体。",
                        "No L5 entity mapped to this expected design node.",
                    )
                    .to_string(),
                    expected_kind: infer_kind_from_node_id(node_id),
                })
                .collect::<Vec<_>>();
            (expected_node_ids.len(), covered, missing)
        };
        let missing_count = total_nodes.saturating_sub(covered_nodes);
        let mut missing_entities = missing_seed;
        for index in 1..=missing_count.saturating_sub(missing_entities.len()) {
            let node_id = format!("UNMAPPED-NODE-{index:03}");
            missing_entities.push(MissingEntity {
                expected_kind: infer_kind_from_node_id(&node_id),
                node_id,
                reason: localized_text(
                    locale,
                    "该预期设计节点尚未映射 L5 实体。",
                    "No L5 entity mapped to this expected design node.",
                )
                .to_string(),
            });
        }
        missing_entities.sort_by_key(|item| {
            (
                supplement_priority_rank(&item.expected_kind),
                item.node_id.clone(),
            )
        });
        let invalid_entities = entities
            .iter()
            .filter(|entity| {
                entity.label.is_empty() || entity.kind.is_empty() || entity.schema == "unknown"
            })
            .map(|entity| InvalidEntity {
                entity_id: entity.entity_id.clone(),
                label: entity.label.clone(),
                reason: localized_text(
                    locale,
                    "缺少标签、类型或 schema",
                    "missing label, kind, or schema",
                )
                .to_string(),
            })
            .collect::<Vec<_>>();
        let entity_count = entities.len();

        EntityCoverageReport {
            schema_version: 1,
            generated_at: now_iso(),
            source: parsed.source.clone(),
            entities,
            entity_count,
            concrete_node_count: total_nodes,
            covered_concrete_nodes: covered_nodes,
            entity_coverage_rate: ratio(covered_nodes, total_nodes),
            target_coverage_rate: 0.8,
            coverage_basis,
            missing_entities,
            invalid_entities,
            ai_supplement: supplement_meta,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EntitySupplementAdapter {
    pub adapter_name: String,
    pub cache_dir: Option<PathBuf>,
    fallback_library: BTreeMap<String, Vec<DesignEntity>>,
}

impl EntitySupplementAdapter {
    pub fn new(adapter_name: impl Into<String>) -> Self {
        Self {
            adapter_name: adapter_name.into(),
            cache_dir: None,
            fallback_library: fallback_entity_library(),
        }
    }

    pub fn with_cache_dir(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(cache_dir.into());
        self
    }

    pub fn with_fallback_library(mut self, library: BTreeMap<String, Vec<DesignEntity>>) -> Self {
        self.fallback_library = library;
        self
    }

    pub fn from_fallback_path(adapter_name: impl Into<String>, path: &Path) -> Self {
        let value = read_json(path, json!({}));
        let library = serde_json::from_value::<BTreeMap<String, Vec<DesignEntity>>>(value)
            .unwrap_or_default();
        if library.is_empty() {
            Self::new(adapter_name)
        } else {
            Self {
                adapter_name: adapter_name.into(),
                cache_dir: None,
                fallback_library: library,
            }
        }
    }

    pub fn should_supplement(&self, coverage_report: &EntityCoverageReport) -> (bool, String) {
        if self.adapter_name == "none" || self.adapter_name.is_empty() {
            return (false, "adapter disabled".to_string());
        }
        if coverage_report.entity_coverage_rate < 0.50 {
            return (
                true,
                format!(
                    "coverage_rate={:.2} < 0.50",
                    coverage_report.entity_coverage_rate
                ),
            );
        }
        if coverage_report.missing_entities.len() > 30 {
            return (
                true,
                format!(
                    "unmapped_nodes={} > 30",
                    coverage_report.missing_entities.len()
                ),
            );
        }
        let system_count = coverage_report
            .entities
            .iter()
            .filter(|entity| entity.kind == "system")
            .count();
        if system_count < 5 {
            return (true, format!("system_entities={system_count} < 5"));
        }
        (false, "coverage sufficient".to_string())
    }

    pub fn supplement(
        &self,
        entities: &[DesignEntity],
        parsed: &ParsedDesignSource,
        missing_node_ids: &[String],
    ) -> SupplementResult {
        let request = self.build_request(entities, parsed, missing_node_ids);
        let cache = self
            .cache_path()
            .and_then(|path| load_supplement_cache(&path, &request.request_hash));
        let (cached, cache_error) = cache.unwrap_or((None, None));
        let (supplemented, cache_hit, fallback_used, error) = if let Some(cached) = cached {
            (cached, true, false, cache_error.unwrap_or_default())
        } else {
            let supplemented = self.fallback(entities, parsed, &request);
            if let Some(path) = self.cache_path() {
                let _ = save_supplement_cache(
                    &path,
                    &request.request_hash,
                    &self.adapter_name,
                    true,
                    &supplemented,
                );
            }
            (supplemented, false, true, cache_error.unwrap_or_default())
        };
        let samples = supplemented
            .iter()
            .filter_map(|entity| entity.supplement_basis.clone())
            .filter(|value| !value.is_empty())
            .take(3)
            .collect::<Vec<_>>();
        let (merged, added_count, completed_count) = merge_entities(entities, &supplemented);
        SupplementResult {
            entities: merged,
            added_count,
            completed_count,
            cache_hit,
            adapter_used: self.adapter_name.clone(),
            fallback_used,
            mode: "complete_approximate".to_string(),
            error,
            supplement_basis_samples: samples,
        }
    }

    pub fn build_request(
        &self,
        entities: &[DesignEntity],
        parsed: &ParsedDesignSource,
        missing_node_ids: &[String],
    ) -> SupplementRequest {
        let genre = pick_genre_template_key(&parsed.raw_text, &parsed.selections);
        let core_loop = LoopExtractor::default().extract(parsed).loop_nodes;
        let systems = SystemDeducer::default()
            .deduce(parsed, &json!({"nodes": [], "edges": []}))
            .systems;
        let request = SupplementRequest {
            project_name: project_name(parsed),
            genre,
            core_loop,
            systems: systems.clone(),
            existing_entities: entities.iter().take(20).cloned().collect(),
            l4_decisions: l4_decisions(&parsed.selections),
            target_kinds: DEFAULT_TARGET_KINDS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            min_per_kind: BTreeMap::from([
                ("weapon".to_string(), 3),
                ("character".to_string(), 1),
                ("ability".to_string(), 5),
                ("room".to_string(), 3),
                ("enemy".to_string(), 3),
            ]),
            missing_node_ids: missing_node_ids.to_vec(),
            known_node_ids: known_node_ids(entities, &systems, missing_node_ids),
            request_hash: String::new(),
            schema_version: 1,
        };
        let hash = compute_request_hash(&request, parsed);
        SupplementRequest {
            request_hash: hash,
            ..request
        }
    }

    fn fallback(
        &self,
        _entities: &[DesignEntity],
        _parsed: &ParsedDesignSource,
        request: &SupplementRequest,
    ) -> Vec<DesignEntity> {
        let mut supplemented = request
            .missing_node_ids
            .iter()
            .take(MISSING_NODE_FALLBACK_LIMIT)
            .enumerate()
            .map(|(index, node_id)| {
                let kind = kind_for_missing_node(node_id);
                normalize_entity(DesignEntity {
                    entity_id: format!("SUP-MISS-{:03}", index + 1),
                    label: label_for_missing_node(node_id, &kind),
                    kind: kind.clone(),
                    schema: format!("{kind}.v1"),
                    status: String::new(),
                    source: "ai_supplement_missing_node_fallback".to_string(),
                    source_selection_id: String::new(),
                    node_id: node_id.clone(),
                    dependencies: vec![node_id.clone()],
                    purpose: String::new(),
                    inference: None,
                    supplement_basis: Some(format!(
                        "{} missing-node fallback #{}: {}",
                        if request.genre.is_empty() {
                            "generic"
                        } else {
                            &request.genre
                        },
                        index + 1,
                        node_id
                    )),
                    completed_from: None,
                })
            })
            .collect::<Vec<_>>();
        let library_entities = self
            .fallback_library
            .get(&request.genre)
            .or_else(|| self.fallback_library.get("generic"))
            .cloned()
            .unwrap_or_default();
        for (index, mut entity) in library_entities.into_iter().enumerate() {
            entity.entity_id = format!("SUP-LIB-{:03}", index + 1);
            entity.source = if entity.source.is_empty() {
                "ai_supplement_fallback".to_string()
            } else {
                entity.source
            };
            if entity.supplement_basis.is_none() {
                entity.supplement_basis = Some(format!(
                    "{} genre fallback entity #{}",
                    request.genre,
                    index + 1
                ));
            }
            supplemented.push(normalize_entity(entity));
        }
        supplemented
    }

    fn cache_path(&self) -> Option<PathBuf> {
        self.cache_dir
            .as_ref()
            .map(|dir| dir.join("ai_supplement_cache.json"))
    }
}

pub fn supplement_trigger_reason(
    entities: &[DesignEntity],
    entity_coverage_rate: f64,
    adapter_name: &str,
) -> (bool, String) {
    if adapter_name.is_empty() || adapter_name == "none" {
        return (false, "adapter disabled".to_string());
    }
    if entities.iter().any(|entity| entity.status == "approximate") {
        return (true, "approximate entities present".to_string());
    }
    let real_l5 = entities
        .iter()
        .filter(|entity| entity.inference.is_none())
        .count();
    if entity_coverage_rate < 0.50 {
        return (
            true,
            format!("coverage_rate={entity_coverage_rate:.2} < 0.50"),
        );
    }
    if entity_coverage_rate < 0.60 && real_l5 < 10 {
        return (
            true,
            format!("coverage_rate={entity_coverage_rate:.2} < 0.60 and real_l5={real_l5} < 10"),
        );
    }
    let system_count = entities
        .iter()
        .filter(|entity| entity.kind == "system")
        .count();
    if system_count < 5 {
        return (true, format!("system_entities={system_count} < 5"));
    }
    (false, "coverage sufficient".to_string())
}

fn supplement_trigger_reason_with_locale(
    entities: &[DesignEntity],
    entity_coverage_rate: f64,
    adapter_name: &str,
    locale: ArtifactLocale,
) -> (bool, String) {
    if locale == ArtifactLocale::EnUs {
        return supplement_trigger_reason(entities, entity_coverage_rate, adapter_name);
    }
    if adapter_name.is_empty() || adapter_name == "none" {
        return (false, "适配器已禁用".to_string());
    }
    if entities.iter().any(|entity| entity.status == "approximate") {
        return (true, "存在概略实体".to_string());
    }
    let real_l5 = entities
        .iter()
        .filter(|entity| entity.inference.is_none())
        .count();
    if entity_coverage_rate < 0.50 {
        return (
            true,
            format!("实体覆盖率 {entity_coverage_rate:.2} 低于 0.50"),
        );
    }
    if entity_coverage_rate < 0.60 && real_l5 < 10 {
        return (
            true,
            format!(
                "实体覆盖率 {entity_coverage_rate:.2} 低于 0.60，且真实 L5 实体数 {real_l5} 少于 10"
            ),
        );
    }
    let system_count = entities
        .iter()
        .filter(|entity| entity.kind == "system")
        .count();
    if system_count < 5 {
        return (true, format!("系统实体数 {system_count} 少于 5"));
    }
    (false, "覆盖率充足".to_string())
}

pub fn extract_l5_entities(parsed: &ParsedDesignSource) -> Vec<DesignEntity> {
    let mut entities = Vec::new();
    for selection in &parsed.selections {
        if !is_l5_entity_item_type(&selection.item_type) {
            continue;
        }
        let index = entities.len() + 1;
        let mut schema = String::new();
        let mut kind = String::new();
        let mut status = String::new();
        for part in selection.purpose.replace(';', "；").split('；') {
            let cleaned = part.trim();
            if let Some(value) = cleaned.strip_prefix("schema=") {
                schema = value.trim().to_string();
            } else if let Some(value) = cleaned.strip_prefix("kind=") {
                kind = value.trim().to_string();
            } else if let Some(value) = cleaned.strip_prefix("status=") {
                status = value.trim().to_string();
            }
        }
        entities.push(DesignEntity {
            entity_id: format!("ENT-{index:03}"),
            label: if selection.option.trim().is_empty() {
                selection.label()
            } else {
                selection.option.clone()
            },
            kind: if kind.is_empty() {
                entity_kind_for(selection)
            } else {
                kind
            },
            schema: if schema.is_empty() {
                "unknown".to_string()
            } else {
                schema
            },
            status: if status.is_empty() {
                "precise".to_string()
            } else {
                status
            },
            source: selection.source_ref.clone(),
            source_selection_id: selection.id(),
            node_id: selection.dependencies.first().cloned().unwrap_or_default(),
            dependencies: selection.dependencies.clone(),
            purpose: selection.purpose.clone(),
            inference: None,
            supplement_basis: None,
            completed_from: None,
        });
    }
    if entities.is_empty() {
        synthetic_entities(parsed, 47)
    } else {
        entities
    }
}

pub fn validate_entity(entity: &DesignEntity) -> bool {
    !entity.label.is_empty()
        && !entity.kind.is_empty()
        && !entity.schema.is_empty()
        && !entity.node_id.is_empty()
        && VALID_ENTITY_KINDS.contains(&entity.kind.as_str())
}

pub fn normalize_entity(mut entity: DesignEntity) -> DesignEntity {
    if entity.kind.is_empty() {
        entity.kind = "resource".to_string();
    }
    if entity.schema.is_empty() {
        entity.schema = format!("{}.v1", entity.kind);
    }
    if entity.node_id.is_empty() {
        entity.node_id = node_by_kind(&entity.kind).to_string();
    }
    entity.dependencies = vec![entity.node_id.clone()];
    if entity.status.is_empty() {
        entity.status = "precise".to_string();
    }
    if entity.source.is_empty() {
        entity.source = "ai_supplement".to_string();
    }
    if entity.purpose.is_empty() {
        entity.purpose = entity.supplement_basis.clone().unwrap_or_default();
    }
    entity
}

pub fn merge_entities(
    original: &[DesignEntity],
    supplemented: &[DesignEntity],
) -> (Vec<DesignEntity>, usize, usize) {
    let normalized = supplemented
        .iter()
        .map(|entity| normalize_entity(entity.clone()))
        .filter(validate_entity)
        .collect::<Vec<_>>();
    let mut used_indexes = BTreeSet::new();
    let mut merged = Vec::new();
    let mut completed_count = 0;

    for entity in original {
        if entity.status == "approximate" {
            if let Some(index) = matching_supplement_index(entity, &normalized, &used_indexes) {
                let mut replacement = normalized[index].clone();
                replacement.source_selection_id = entity.source_selection_id.clone();
                replacement.completed_from = Some(entity.entity_id.clone());
                merged.push(replacement);
                used_indexes.insert(index);
                completed_count += 1;
                continue;
            }
        }
        merged.push(entity.clone());
    }

    let mut seen = merged.iter().map(dedupe_key).collect::<BTreeSet<_>>();
    let mut added_count = 0;
    for (index, entity) in normalized.into_iter().enumerate() {
        if used_indexes.contains(&index) {
            continue;
        }
        let key = dedupe_key(&entity);
        if seen.insert(key) {
            merged.push(entity);
            added_count += 1;
        }
    }
    for (index, entity) in merged.iter_mut().enumerate() {
        entity.entity_id = format!("ENT-{:03}", index + 1);
    }
    (merged, added_count, completed_count)
}

pub fn parse_response_entities(output_text: &str) -> Vec<DesignEntity> {
    let payload = extract_json(output_text);
    payload
        .get("supplemented_entities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| serde_json::from_value::<DesignEntity>(item.clone()).ok())
        .map(normalize_entity)
        .filter(validate_entity)
        .collect()
}

pub fn extract_json(output_text: &str) -> Value {
    if let Ok(payload) = serde_json::from_str::<Value>(output_text) {
        return payload;
    }
    if let (Some(start), Some(end)) = (output_text.find('{'), output_text.rfind('}')) {
        if end > start {
            if let Ok(payload) = serde_json::from_str::<Value>(&output_text[start..=end]) {
                return payload;
            }
        }
    }
    json!({})
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityGraphReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub cycles: Vec<Vec<String>>,
    pub cycle_free: bool,
}

#[derive(Debug, Clone, Default)]
pub struct GraphGenerator;

impl GraphGenerator {
    pub fn generate(
        &self,
        system_graph: &Value,
        entity_report: &EntityCoverageReport,
    ) -> EntityGraphReport {
        let mut nodes = system_graph
            .get("nodes")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|node| {
                let id = string_field(node, "id");
                if id.is_empty() {
                    None
                } else {
                    Some(GraphNode {
                        id,
                        name: string_field(node, "name"),
                        node_type: "system".to_string(),
                        source: string_field(node, "source"),
                    })
                }
            })
            .collect::<Vec<_>>();
        let mut node_ids = nodes
            .iter()
            .map(|node| node.id.clone())
            .collect::<BTreeSet<_>>();
        let mut edges = system_graph
            .get("edges")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|edge| {
                let from = string_field(edge, "from");
                let to = string_field(edge, "to");
                if from.is_empty() || to.is_empty() {
                    None
                } else {
                    Some(GraphEdge {
                        from,
                        to,
                        relation: non_empty_or(string_field(edge, "relation"), "depends_on"),
                        source: string_field(edge, "source"),
                    })
                }
            })
            .collect::<Vec<_>>();
        for entity in &entity_report.entities {
            if entity.entity_id.is_empty() {
                continue;
            }
            nodes.push(GraphNode {
                id: entity.entity_id.clone(),
                name: entity.label.clone(),
                node_type: "entity".to_string(),
                source: entity.source.clone(),
            });
            if !entity.node_id.is_empty() {
                if node_ids.insert(entity.node_id.clone()) {
                    nodes.push(GraphNode {
                        id: entity.node_id.clone(),
                        name: entity.node_id.clone(),
                        node_type: "design_node".to_string(),
                        source: "L5 entity dependency".to_string(),
                    });
                }
                edges.push(GraphEdge {
                    from: entity.node_id.clone(),
                    to: entity.entity_id.clone(),
                    relation: "defines_entity".to_string(),
                    source: entity.source.clone(),
                });
            }
        }
        let cycles = detect_cycles(
            &nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>(),
            &edges,
        );
        EntityGraphReport {
            schema_version: 1,
            generated_at: now_iso(),
            nodes,
            edges,
            cycle_free: cycles.is_empty(),
            cycles,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseClassificationReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub phases: BTreeMap<String, Vec<PhaseEntity>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseEntity {
    pub entity_id: String,
    pub label: String,
    pub kind: String,
    pub node_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct PhaseClassifier;

impl PhaseClassifier {
    pub fn classify(&self, entity_report: &EntityCoverageReport) -> PhaseClassificationReport {
        self.classify_with_locale(entity_report, ArtifactLocale::ZhCn)
    }

    pub fn classify_with_locale(
        &self,
        entity_report: &EntityCoverageReport,
        locale: ArtifactLocale,
    ) -> PhaseClassificationReport {
        let mut phases = BTreeMap::from([
            ("core_playable".to_string(), Vec::new()),
            ("progression".to_string(), Vec::new()),
            ("economy".to_string(), Vec::new()),
            ("content_ops".to_string(), Vec::new()),
            ("social".to_string(), Vec::new()),
            ("launch_ops".to_string(), Vec::new()),
        ]);
        for entity in &entity_report.entities {
            phases
                .entry(phase_for(entity))
                .or_default()
                .push(PhaseEntity {
                    entity_id: entity.entity_id.clone(),
                    label: entity.label.clone(),
                    kind: entity.kind.clone(),
                    node_id: entity.node_id.clone(),
                    reason: localized_text(
                        locale,
                        "基于实体关键词的确定性阶段分类",
                        "deterministic entity keyword classification",
                    )
                    .to_string(),
                });
        }
        PhaseClassificationReport {
            schema_version: 1,
            generated_at: now_iso(),
            phases,
        }
    }
}

fn structured_input_review_items(
    structured_inputs: &Value,
    return_target: &str,
    locale: ArtifactLocale,
) -> Vec<Value> {
    structured_inputs
        .get("warnings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|warning| {
            let code = non_empty_or(
                string_field(warning, "code"),
                "STRUCTURED_INPUT_WARNING",
            );
            let message = non_empty_or(
                string_field(warning, "message"),
                localized_text(
                    locale,
                    "结构化设计交接输入存在兼容性问题，已回退到 Markdown 输入。",
                    "Structured design handoff input has a compatibility issue; Markdown fallback was used.",
                ),
            );
            json!({
                "code": code,
                "severity": "warning",
                "return_target": return_target,
                "message": message,
                "affected_count": 1,
                "source": "structured_handoff",
            })
        })
        .collect()
}

fn review_item_count(items: &[Value]) -> usize {
    items
        .iter()
        .map(|item| {
            item.get("affected_count")
                .and_then(Value::as_u64)
                .and_then(|count| usize::try_from(count).ok())
                .unwrap_or(1)
                .max(1)
        })
        .sum()
}

fn review_messages(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| item.get("message").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn stage_completion_message(
    stage_id: &str,
    ready: bool,
    warnings: &[String],
    review_items_count: usize,
    locale: ArtifactLocale,
) -> String {
    if ready {
        return if locale == ArtifactLocale::ZhCn {
            format!("步骤 {stage_id} 已成功完成")
        } else {
            format!("Step{stage_id} completed successfully")
        };
    }
    let detail = warnings.first().cloned().unwrap_or_else(|| {
        if locale == ArtifactLocale::ZhCn {
            format!("{review_items_count} 个复核项")
        } else {
            format!("{review_items_count} review item(s)")
        }
    });
    if locale == ArtifactLocale::ZhCn {
        format!("步骤 {stage_id} 已完成，但需要复核：{detail}")
    } else {
        format!("Step{stage_id} completed with review: {detail}")
    }
}

fn parsed_contract_value(parsed: &ParsedDesignSource) -> AdmResult<Value> {
    let mut value = to_json_value(parsed)?;
    value["selections"] = Value::Array(parsed.selection_dicts());
    Ok(value)
}

fn add_contract_metadata(
    mut contract: Value,
    locale: ArtifactLocale,
    zh_cn_name: &str,
    en_us_name: &str,
) -> Value {
    if let Some(object) = contract.as_object_mut() {
        object.insert("artifact_locale".to_string(), json!(locale));
        object.insert(
            "contract_display_name".to_string(),
            json!(localized_text(locale, zh_cn_name, en_us_name)),
        );
    }
    contract
}

fn structured_contract<'a>(structured_inputs: &'a Value, name: &str) -> Option<&'a Value> {
    structured_inputs
        .get("inputs")
        .and_then(|inputs| inputs.get(name))
        .filter(|value| value.as_object().is_some_and(|object| !object.is_empty()))
}

fn contract_matches_locale(contract: &Value, locale: ArtifactLocale) -> bool {
    contract
        .get("artifact_locale")
        .and_then(Value::as_str)
        .map(|value| ArtifactLocale::normalize(Some(value)) == locale)
        .unwrap_or(locale == ArtifactLocale::ZhCn)
}

fn semantic_profile(
    profile: &ConceptProfile,
    structured_inputs: &Value,
    locale: ArtifactLocale,
) -> AdmResult<Value> {
    if let Some(profile) = structured_contract(structured_inputs, "profile")
        .filter(|profile| contract_matches_locale(profile, locale))
    {
        return Ok(add_contract_metadata(
            profile.clone(),
            locale,
            "结构化项目画像",
            "Structured Project Profile",
        ));
    }
    to_artifact_json_value(profile, locale)
}

fn confirmed_option_text(parsed: &ParsedDesignSource, structured_inputs: &Value) -> Vec<String> {
    let confirmed = structured_contract(structured_inputs, "decisions")
        .and_then(|decisions| decisions.get("confirmed_option_text"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !confirmed.is_empty() {
        confirmed
    } else {
        parsed
            .selections
            .iter()
            .map(|selection| selection.option.trim().to_string())
            .filter(|text| !text.is_empty())
            .collect()
    }
}

fn archetype_requirements_for(
    profile: &Value,
    parsed: &ParsedDesignSource,
    structured_inputs: &Value,
    locale: ArtifactLocale,
) -> Value {
    if let Some(requirements) = structured_contract(structured_inputs, "archetype_requirements")
        .filter(|requirements| contract_matches_locale(requirements, locale))
    {
        return add_contract_metadata(
            requirements.clone(),
            locale,
            "原型需求契约",
            "Archetype Requirements Contract",
        );
    }
    let requirements = build_archetype_requirements_with_locale(
        profile,
        &confirmed_option_text(parsed, structured_inputs),
        &ArchetypeCatalog::builtin(),
        locale,
    );
    add_contract_metadata(
        requirements,
        locale,
        "原型需求契约",
        "Archetype Requirements Contract",
    )
}

fn unanswered_question_values(coverage: &QuestionCoverageReport) -> Vec<Value> {
    coverage
        .questions
        .iter()
        .filter(|question| !question.answered)
        .map(|question| {
            json!({
                "question_id": question.id,
                "prompt": question.question,
                "blocking": false,
                "priority": "P1",
                "status": "open",
                "source_refs": question.evidence.iter().map(|evidence| evidence.source.clone()).collect::<Vec<_>>(),
            })
        })
        .collect()
}

fn build_intent_interpretation_contract(
    parsed: &ParsedDesignSource,
    profile: &ConceptProfile,
    coverage: &QuestionCoverageReport,
    locale: ArtifactLocale,
) -> Value {
    let resource_loop = first_matching(
        &parsed.selections,
        &[
            "资源循环",
            "资源",
            "奖励节奏",
            "经济",
            "resource loop",
            "resource",
            "reward cadence",
            "economy",
        ],
    );
    let perspective = first_matching(
        &parsed.selections,
        &["视角", "镜头", "perspective", "camera"],
    );
    let platforms = matching_items(
        &parsed.selections,
        &["平台", "platform", "target platform"],
        8,
    )
    .into_iter()
    .map(|item| item.label)
    .filter(|label| !label.is_empty())
    .collect::<Vec<_>>();
    let warnings = if coverage.coverage_rate < coverage.target_coverage_rate {
        vec![json!({
            "code": "CORE_QUESTION_COVERAGE_BELOW_TARGET",
            "message": if locale == ArtifactLocale::ZhCn {
                format!("仍有 {} 个核心意图问题需要补充。", coverage.unanswered_questions)
            } else {
                format!("{} core intent question(s) still require input.", coverage.unanswered_questions)
            },
        })]
    } else {
        Vec::new()
    };
    add_contract_metadata(
        json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "source_refs": [parsed.source],
            "genre": profile.genre_key,
            "genre_display_name": profile.project_positioning.label,
            "perspective": non_empty_or(
                perspective.label,
                localized_text(locale, "由首个可玩场景确定", "Determined by the first playable scene"),
            ),
            "platform_assumptions": platforms,
            "core_loop": profile.core_loop.label,
            "resource_loop": non_empty_or(
                resource_loop.label,
                localized_text(locale, "核心动作产生反馈并推进目标。", "Core actions produce feedback and advance the objective."),
            ),
            "blockers": [],
            "warnings": warnings,
        }),
        locale,
        "意图解释契约",
        "Intent Interpretation Contract",
    )
}

fn build_gameplay_concretization_contract(
    parsed: &ParsedDesignSource,
    playable_preview: &Value,
    locale: ArtifactLocale,
) -> Value {
    let core = playable_preview
        .get("core_playable_contract")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let demo = playable_preview
        .get("demo_flow_contract")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let runtime = playable_preview
        .get("runtime_data_contract")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let scene = playable_preview
        .get("scene_bootstrap_contract")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let completeness = playable_preview
        .get("design_completeness_report")
        .and_then(|report| report.get("playable_completeness"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    add_contract_metadata(
        json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "source_refs": [parsed.source, "stage_00/intent_interpretation_contract.json"],
            "player_actions": core.get("action_verbs").cloned().unwrap_or_else(|| json!([])),
            "feedback_loop": {
                "summary": localized_text(locale, "玩家动作会产生可见反馈并改变可玩状态。", "Player actions produce visible feedback and change playable state."),
                "feedback_requirements": core.get("feedback_requirements").cloned().unwrap_or_else(|| json!([])),
            },
            "goal_progression": {
                "flow_steps": demo.get("steps").cloned().unwrap_or_else(|| json!([])),
                "completion_conditions": demo.get("completion_conditions").cloned().unwrap_or_else(|| json!([])),
                "failure_conditions": demo.get("failure_conditions").cloned().unwrap_or_else(|| json!([])),
            },
            "scene_requirements": scene.get("initial_objects").cloned().unwrap_or_else(|| json!([])),
            "runtime_data_needs": runtime.get("tables").cloned().unwrap_or_else(|| json!([])),
            "blockers": completeness.get("blocking_issues").cloned().unwrap_or_else(|| json!([])),
            "warnings": completeness.get("review_items").cloned().unwrap_or_else(|| json!([])),
        }),
        locale,
        "玩法具体化契约",
        "Gameplay Concretization Contract",
    )
}

fn build_archetype_detection_report(requirements: &Value, locale: ArtifactLocale) -> Value {
    let confidence_label = requirements
        .get("detection_confidence")
        .and_then(Value::as_str)
        .unwrap_or("fallback");
    let confidence = match confidence_label {
        "high" => 1.0,
        "medium" => 0.7,
        "low" => 0.4,
        _ => 0.0,
    };
    let signals = requirements
        .get("detection_source")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|source| {
            json!({
                "source": source,
                "display_name": if locale == ArtifactLocale::ZhCn {
                    format!("原型识别信号：{source}")
                } else {
                    format!("Archetype signal: {source}")
                },
            })
        })
        .collect::<Vec<_>>();
    add_contract_metadata(
        json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "detected_archetype": requirements.get("detected_archetype").cloned().unwrap_or_else(|| json!("generic_playable")),
            "parent_archetypes": requirements.get("parent_archetypes").cloned().unwrap_or_else(|| json!([])),
            "confidence": confidence,
            "confidence_label": confidence_label,
            "signals": signals,
            "fallback_used": confidence_label == "fallback",
            "warnings": requirements.get("warnings").cloned().unwrap_or_else(|| json!([])),
        }),
        locale,
        "原型识别报告",
        "Archetype Detection Report",
    )
}

fn upstream_contract(
    out_dir: &Path,
    stage: u32,
    file_name: &str,
    locale: ArtifactLocale,
) -> Option<Value> {
    let parent = out_dir.parent()?;
    for stage_name in [format!("stage_{stage:02}"), format!("{stage:02}")] {
        let path = parent.join(stage_name).join(file_name);
        if !path.is_file() {
            continue;
        }
        let value = read_json(&path, Value::Null);
        if value.is_object() && contract_matches_locale(&value, locale) {
            return Some(value);
        }
    }
    None
}

fn structured_stage_issue(
    item: &Value,
    return_target: &str,
    severity: &str,
    locale: ArtifactLocale,
) -> Value {
    let code = item
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("CONTRACT_BLOCKED");
    let message = item
        .get("message")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            localized_text(
                locale,
                "契约包含尚未解决的阻断项。",
                "The contract contains an unresolved blocking item.",
            )
            .to_string()
        });
    json!({
        "code": code,
        "severity": severity,
        "return_target": return_target,
        "message": message,
        "affected_count": 1,
        "details": item,
    })
}

fn build_design_ai_review_report(
    entity_report: &EntityCoverageReport,
    project_dna: &Value,
    open_questions: &Value,
    blockers: &[Value],
    review_items: &[Value],
    locale: ArtifactLocale,
) -> Value {
    let inferred_items = entity_report
        .entities
        .iter()
        .filter_map(|entity| {
            entity.inference.as_ref().map(|inference| {
                json!({
                    "entity_id": entity.entity_id,
                    "label": entity.label,
                    "inference": inference,
                    "source": entity.source,
                })
            })
        })
        .collect::<Vec<_>>();
    add_contract_metadata(
        json!({
            "schema_version": "1.0",
            "generated_at": now_iso(),
            "source_refs": [
                "stage_02/project_dna_contract.json",
                "stage_02/entity_validation_report.json",
                "stage_02/playable_contracts/design_completeness_report.json",
            ],
            "project_signature": project_dna.get("project_signature").cloned().unwrap_or_else(|| json!("")),
            "review_status": if !blockers.is_empty() { "blocked" } else if review_items.is_empty() { "passed" } else { "review" },
            "blockers": blockers,
            "warnings": review_items,
            "inferred_items": inferred_items,
            "user_questions": open_questions.get("questions").cloned().unwrap_or_else(|| json!([])),
            "coverage": {
                "covered_entity_nodes": entity_report.covered_concrete_nodes,
                "expected_entity_nodes": entity_report.concrete_node_count,
                "entity_coverage_rate": entity_report.entity_coverage_rate,
                "target_coverage_rate": entity_report.target_coverage_rate,
            },
        }),
        locale,
        "设计 AI 复核报告",
        "Design AI Review Report",
    )
}

#[derive(Debug, Clone, Default)]
pub struct Step00OutputGenerator {
    question_engine: QuestionEngine,
    concept_processor: ConceptProcessor,
}

impl Step00OutputGenerator {
    pub fn new(question_engine: QuestionEngine) -> Self {
        Self {
            question_engine,
            concept_processor: ConceptProcessor,
        }
    }
}

impl StageOutputGenerator for Step00OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP00)?;
        let locale = artifact_locale_from_inputs(structured_inputs);
        let profile = self
            .concept_processor
            .build_profile_with_locale(parsed, locale);
        let coverage = self.question_engine.evaluate_with_locale(parsed, locale);
        let parsed_contract = parsed_contract_value(parsed)?;
        let profile_contract = semantic_profile(&profile, structured_inputs, locale)?;
        let archetype_requirements =
            archetype_requirements_for(&profile_contract, parsed, structured_inputs, locale);
        let linked_save_id = profile_contract
            .get("linked_save_id")
            .or_else(|| profile_contract.get("save_id"))
            .and_then(Value::as_str);
        let project_identity = build_project_identity_with_locale(
            &parsed_contract,
            out_dir,
            Some(&profile_contract),
            linked_save_id,
            locale,
        );
        let open_questions = build_open_questions_contract_with_locale(
            Some(&project_identity),
            Some(&archetype_requirements),
            &unanswered_question_values(&coverage),
            "00",
            locale,
        );
        let project_dna_seed = build_project_dna_seed_with_locale(
            &project_identity,
            &profile_contract,
            &parsed_contract,
            &archetype_requirements,
            Some(&open_questions),
            locale,
        );
        let intent_interpretation =
            build_intent_interpretation_contract(parsed, &profile, &coverage, locale);
        let source_manifest = source_manifest(parsed, "concept_source", locale);
        write_json(
            &out_dir.join("design_source_manifest.json"),
            &source_manifest,
        )?;
        write_json(
            &out_dir.join("concept_profile.json"),
            &to_artifact_json_value(&profile, locale)?,
        )?;
        write_json(
            &out_dir.join("core_question_coverage_report.json"),
            &to_artifact_json_value(&coverage, locale)?,
        )?;
        write_json(
            &out_dir.join("intent_interpretation_contract.json"),
            &intent_interpretation,
        )?;
        write_json(
            &out_dir.join("project_identity_contract.json"),
            &project_identity,
        )?;
        write_json(&out_dir.join("project_dna_seed.json"), &project_dna_seed)?;
        write_json(
            &out_dir.join("open_questions_contract.json"),
            &open_questions,
        )?;
        write_json(
            &out_dir.join("option_taxonomy.json"),
            &json!({
                "schema_version": 1,
                "generated_at": now_iso(),
                "artifact_locale": locale,
                "source": parsed.source,
                "selection_count": parsed.selections.len(),
                "layers": parsed.layers,
            }),
        )?;
        write_text(
            &out_dir.join("main_design_source.md"),
            &render_idea_intake(parsed, &profile, &coverage, locale),
        )?;
        let mut review_items = structured_input_review_items(structured_inputs, "00", locale);
        if coverage.coverage_rate < coverage.target_coverage_rate {
            review_items.push(json!({
                "code": "CORE_QUESTION_COVERAGE_BELOW_TARGET",
                "severity": "warning",
                "return_target": "00",
                "message": if locale == ArtifactLocale::ZhCn {
                    format!(
                        "核心问题覆盖率为 {:.2}%，低于 {:.2}% 的目标；仍有 {} 个问题需要补充。",
                        coverage.coverage_rate * 100.0,
                        coverage.target_coverage_rate * 100.0,
                        coverage.unanswered_questions,
                    )
                } else {
                    format!(
                        "Core-question coverage is {:.2}%, below the {:.2}% target; {} question(s) still require input.",
                        coverage.coverage_rate * 100.0,
                        coverage.target_coverage_rate * 100.0,
                        coverage.unanswered_questions,
                    )
                },
                "affected_count": coverage.unanswered_questions,
            }));
        }
        let review_items_count = review_item_count(&review_items);
        let warnings = review_messages(&review_items);
        let identity_blockers = project_identity
            .get("blockers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let customization = build_customization_score_report_with_locale(
            "00",
            Some(&project_identity),
            if identity_blockers.is_empty() {
                if review_items.is_empty() {
                    "passed"
                } else {
                    "review"
                }
            } else {
                "blocked"
            },
            &identity_blockers,
            &review_items,
            Some(json!({
                "project_signature_present": if project_identity.get("project_signature").and_then(Value::as_str).is_some_and(|value| !value.is_empty()) { 1.0 } else { 0.0 },
                "project_name_present": if project_identity.get("project_name").and_then(Value::as_str).is_some_and(|value| !value.is_empty()) { 1.0 } else { 0.0 },
                "core_question_coverage": coverage.coverage_rate,
                "template_leakage_count": if profile.fallback_used { 1 } else { 0 },
            })),
            locale,
        );
        write_json(
            &out_dir.join("customization_score_report.json"),
            &customization,
        )?;
        let blocked = !identity_blockers.is_empty();
        let ready = review_items.is_empty() && !blocked;
        Ok(json!({
            "status": if blocked { "blocked" } else if ready { "success" } else { "completed_with_review" },
            "artifact_locale": locale,
            "content_exists": true,
            "traceability_valid": !blocked,
            "blocking_issues": identity_blockers.len(),
            "source": parsed.source,
            "selection_count": parsed.selections.len(),
            "concept_profile": "concept_profile.json",
            "core_question_coverage_report": "core_question_coverage_report.json",
            "intent_interpretation_contract": "intent_interpretation_contract.json",
            "project_identity_contract": "project_identity_contract.json",
            "project_dna_seed": "project_dna_seed.json",
            "open_questions_contract": "open_questions_contract.json",
            "customization_score_report": "customization_score_report.json",
            "coverage_rate": coverage.coverage_rate,
            "answered_questions": coverage.answered_questions,
            "needs_ai_supplement": coverage.needs_ai_supplement,
            "structured_input_status": structured_inputs.get("status").and_then(Value::as_str).unwrap_or("fallback_to_markdown"),
            "review_items_count": review_items_count,
            "review_items": review_items,
            "warnings": warnings,
            "message": stage_completion_message("00", ready, &warnings, review_items_count, locale),
            "semantic_quality": {
                "status": if blocked { "blocked" } else if ready { "success" } else { "warning" },
                "project_specificity_score": if profile.fallback_used { 0.0 } else { 1.0 },
                "required_semantic_coverage": coverage.coverage_rate,
                "generic_template_ratio": if profile.fallback_used { 1.0 } else { 0.0 },
                "placeholder_ratio": if profile.project_positioning.label == localized_text(locale, "待补充项目定位", "Project positioning required") { 1.0 } else { 0.0 },
                "return_targets": review_items,
            },
        }))
    }
}

#[derive(Debug, Clone)]
pub struct Step01OutputGenerator {
    loop_extractor: LoopExtractor,
    system_deducer: SystemDeducer,
}

impl Default for Step01OutputGenerator {
    fn default() -> Self {
        Self {
            loop_extractor: LoopExtractor::default(),
            system_deducer: SystemDeducer::default(),
        }
    }
}

impl Step01OutputGenerator {
    pub fn new(templates: TemplateLibrary) -> Self {
        Self {
            loop_extractor: LoopExtractor::new(templates.clone()),
            system_deducer: SystemDeducer::new(templates),
        }
    }
}

impl StageOutputGenerator for Step01OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP01)?;
        let locale = artifact_locale_from_inputs(structured_inputs);
        let core_loop = self.loop_extractor.extract_with_locale(parsed, locale);
        let system_graph = add_contract_metadata(
            system_graph_from_selections(parsed),
            locale,
            "玩法系统关系图",
            "Gameplay System Relationship Graph",
        );
        let systems = self
            .system_deducer
            .deduce_with_locale(parsed, &system_graph, locale);
        let concept_profile = ConceptProcessor.build_profile_with_locale(parsed, locale);
        let parsed_contract = parsed_contract_value(parsed)?;
        let profile_contract = semantic_profile(&concept_profile, structured_inputs, locale)?;
        let project_identity =
            upstream_contract(out_dir, STEP00, "project_identity_contract.json", locale)
                .unwrap_or_else(|| {
                    build_project_identity_with_locale(
                        &parsed_contract,
                        out_dir,
                        Some(&profile_contract),
                        None,
                        locale,
                    )
                });
        let archetype_requirements =
            archetype_requirements_for(&profile_contract, parsed, structured_inputs, locale);
        let prior_questions =
            upstream_contract(out_dir, STEP00, "open_questions_contract.json", locale)
                .and_then(|contract| contract.get("questions").and_then(Value::as_array).cloned())
                .unwrap_or_default();
        let open_questions = build_open_questions_contract_with_locale(
            Some(&project_identity),
            Some(&archetype_requirements),
            &prior_questions,
            "01",
            locale,
        );
        let (playable_preview, _) = playable_contract_bundle(parsed, structured_inputs, locale);
        let gameplay_concretization =
            build_gameplay_concretization_contract(parsed, &playable_preview, locale);
        let archetype_detection = build_archetype_detection_report(&archetype_requirements, locale);
        write_json(
            &out_dir.join("core_loop.json"),
            &to_artifact_json_value(&core_loop, locale)?,
        )?;
        write_json(&out_dir.join("system_graph.json"), &system_graph)?;
        write_json(
            &out_dir.join("system_definitions.json"),
            &to_artifact_json_value(&systems, locale)?,
        )?;
        write_json(
            &out_dir.join("gameplay_framework.json"),
            &json!({
                "schema_version": 1,
                "generated_at": now_iso(),
                "artifact_locale": locale,
                "source": parsed.source,
                "core_loop": core_loop.loop_nodes,
                "systems": systems.systems,
                "template_key": core_loop.template_key,
                "structured_input_status": structured_inputs.get("status").and_then(Value::as_str).unwrap_or("fallback_to_markdown"),
            }),
        )?;
        write_json(
            &out_dir.join("gameplay_concretization_contract.json"),
            &gameplay_concretization,
        )?;
        write_json(
            &out_dir.join("archetype_requirements.json"),
            &archetype_requirements,
        )?;
        write_json(
            &out_dir.join("open_questions_contract.json"),
            &open_questions,
        )?;
        write_json(
            &out_dir.join("archetype_detection_report.json"),
            &archetype_detection,
        )?;
        write_text(
            &out_dir.join("gameplay_framework.md"),
            &render_gameplay_framework(parsed, &core_loop, &systems, locale),
        )?;
        let mut review_items = structured_input_review_items(structured_inputs, "01", locale);
        for warning in archetype_requirements
            .get("warnings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            review_items.push(structured_stage_issue(warning, "01", "warning", locale));
        }
        if systems.system_count < 5 {
            review_items.push(json!({
                "code": "GAMEPLAY_SYSTEM_COUNT_BELOW_TARGET",
                "severity": "warning",
                "return_target": "01",
                "message": if locale == ArtifactLocale::ZhCn {
                    format!("当前仅定义 {} 个玩法系统，建议至少补充到 5 个。", systems.system_count)
                } else {
                    format!("Only {} gameplay system(s) are defined; at least 5 are recommended.", systems.system_count)
                },
                "affected_count": 5usize.saturating_sub(systems.system_count),
            }));
        }
        let review_items_count = review_item_count(&review_items);
        let warnings = review_messages(&review_items);
        let identity_blockers = project_identity
            .get("blockers")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let customization = build_customization_score_report_with_locale(
            "01",
            Some(&project_identity),
            if identity_blockers.is_empty() {
                if review_items.is_empty() {
                    "passed"
                } else {
                    "review"
                }
            } else {
                "blocked"
            },
            &identity_blockers,
            &review_items,
            Some(json!({
                "project_signature_present": if project_identity.get("project_signature").and_then(Value::as_str).is_some_and(|value| !value.is_empty()) { 1.0 } else { 0.0 },
                "gameplay_system_definition_rate": systems.definition_rate,
                "archetype_confidence": archetype_detection.get("confidence").cloned().unwrap_or_else(|| json!(0.0)),
                "template_leakage_count": systems.systems.iter().filter(|system| system.confidence != "explicit").count(),
            })),
            locale,
        );
        write_json(
            &out_dir.join("customization_score_report.json"),
            &customization,
        )?;
        let blocked = !identity_blockers.is_empty();
        let ready = review_items.is_empty() && !blocked;
        let explicit_systems = systems
            .systems
            .iter()
            .filter(|system| system.confidence == "explicit")
            .count();
        Ok(json!({
            "status": if blocked { "blocked" } else if ready { "success" } else { "completed_with_review" },
            "artifact_locale": locale,
            "content_exists": true,
            "traceability_valid": !blocked,
            "blocking_issues": identity_blockers.len(),
            "source": parsed.source,
            "template_key": core_loop.template_key,
            "core_loop_output_rate": core_loop.output_rate,
            "system_count": systems.system_count,
            "definition_rate": systems.definition_rate,
            "core_loop": "core_loop.json",
            "system_definitions": "system_definitions.json",
            "gameplay_framework": "gameplay_framework.md",
            "gameplay_concretization_contract": "gameplay_concretization_contract.json",
            "archetype_requirements": "archetype_requirements.json",
            "open_questions_contract": "open_questions_contract.json",
            "archetype_detection_report": "archetype_detection_report.json",
            "customization_score_report": "customization_score_report.json",
            "structured_input_status": structured_inputs.get("status").and_then(Value::as_str).unwrap_or("fallback_to_markdown"),
            "review_items_count": review_items_count,
            "review_items": review_items,
            "warnings": warnings,
            "message": stage_completion_message("01", ready, &warnings, review_items_count, locale),
            "semantic_quality": {
                "status": if blocked { "blocked" } else if ready { "success" } else { "warning" },
                "project_specificity_score": ratio(explicit_systems, systems.system_count),
                "required_semantic_coverage": systems.definition_rate,
                "generic_template_ratio": ratio(systems.system_count.saturating_sub(explicit_systems), systems.system_count),
                "placeholder_ratio": 0.0,
                "return_targets": review_items,
            },
        }))
    }
}

#[derive(Debug, Clone)]
pub struct Step02OutputGenerator {
    entity_validator: EntityValidator,
    graph_generator: GraphGenerator,
    phase_classifier: PhaseClassifier,
}

impl Default for Step02OutputGenerator {
    fn default() -> Self {
        Self {
            entity_validator: EntityValidator::new(Some(EntitySupplementAdapter::new(
                "local_fallback",
            ))),
            graph_generator: GraphGenerator,
            phase_classifier: PhaseClassifier,
        }
    }
}

impl Step02OutputGenerator {
    pub fn new(entity_validator: EntityValidator) -> Self {
        Self {
            entity_validator,
            graph_generator: GraphGenerator,
            phase_classifier: PhaseClassifier,
        }
    }
}

impl StageOutputGenerator for Step02OutputGenerator {
    fn generate(
        &self,
        step_number: u32,
        parsed: &ParsedDesignSource,
        out_dir: &Path,
        structured_inputs: &Value,
    ) -> AdmResult<Value> {
        ensure_stage(step_number, STEP02)?;
        let locale = artifact_locale_from_inputs(structured_inputs);
        let system_graph = add_contract_metadata(
            system_graph_from_selections(parsed),
            locale,
            "玩法系统关系图",
            "Gameplay System Relationship Graph",
        );
        let entity_report =
            self.entity_validator
                .validate_with_inputs(parsed, structured_inputs, locale);
        let graph = self.graph_generator.generate(&system_graph, &entity_report);
        let phases = self
            .phase_classifier
            .classify_with_locale(&entity_report, locale);
        let frozen = render_frozen_game_design(parsed, &entity_report, &phases, locale);
        let (playable_bundle, playable_contract_source) =
            playable_contract_bundle(parsed, structured_inputs, locale);
        let playable_contracts = playable_bundle.as_object().ok_or_else(|| {
            AdmError::new("playable contract builder returned a non-object payload")
        })?;
        for (contract_id, payload) in playable_contracts {
            if contract_id == "artifact_locale" || !payload.is_object() {
                continue;
            }
            write_json(
                &out_dir
                    .join("playable_contracts")
                    .join(format!("{contract_id}.json")),
                payload,
            )?;
        }
        let playable_contract_count = playable_contracts
            .iter()
            .filter(|(contract_id, payload)| {
                contract_id.as_str() != "artifact_locale" && payload.is_object()
            })
            .count();
        let playable_completeness = playable_bundle
            .get("design_completeness_report")
            .and_then(|report| report.get("playable_completeness"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let playable_valid = playable_completeness
            .get("valid")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let playable_blocker_count = playable_completeness
            .get("blocking_issues")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        let playable_review_count = playable_completeness
            .get("review_items")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        let playable_blockers = playable_completeness
            .get("blocking_issues")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let concept_profile = ConceptProcessor.build_profile_with_locale(parsed, locale);
        let parsed_contract = parsed_contract_value(parsed)?;
        let profile_contract = semantic_profile(&concept_profile, structured_inputs, locale)?;
        let project_identity =
            upstream_contract(out_dir, STEP00, "project_identity_contract.json", locale)
                .unwrap_or_else(|| {
                    build_project_identity_with_locale(
                        &parsed_contract,
                        out_dir,
                        Some(&profile_contract),
                        None,
                        locale,
                    )
                });
        let archetype_requirements =
            upstream_contract(out_dir, STEP01, "archetype_requirements.json", locale)
                .unwrap_or_else(|| {
                    archetype_requirements_for(&profile_contract, parsed, structured_inputs, locale)
                });
        let open_questions =
            upstream_contract(out_dir, STEP01, "open_questions_contract.json", locale)
                .or_else(|| {
                    upstream_contract(out_dir, STEP00, "open_questions_contract.json", locale)
                })
                .unwrap_or_else(|| {
                    build_open_questions_contract_with_locale(
                        Some(&project_identity),
                        Some(&archetype_requirements),
                        &[],
                        "01",
                        locale,
                    )
                });
        let project_dna_seed = upstream_contract(out_dir, STEP00, "project_dna_seed.json", locale)
            .unwrap_or_else(|| {
                build_project_dna_seed_with_locale(
                    &project_identity,
                    &profile_contract,
                    &parsed_contract,
                    &archetype_requirements,
                    Some(&open_questions),
                    locale,
                )
            });
        let open_question_blockers = unresolved_blocking_questions(Some(&open_questions));
        let (project_dna, freeze_blockers) = freeze_project_dna_with_locale(
            &project_dna_seed,
            &archetype_requirements,
            &playable_bundle,
            &open_question_blockers,
            locale,
        );
        let playable_scenario =
            build_playable_scenario_contract_with_locale(&project_dna, &playable_bundle, locale);
        let semantic_coverage_seed = build_semantic_coverage_seed_with_locale(&project_dna, locale);
        let mut review_items = step02_review_items(
            &entity_report,
            &graph,
            playable_valid,
            playable_blocker_count,
            playable_review_count,
            locale,
        );
        review_items.extend(structured_input_review_items(
            structured_inputs,
            "02",
            locale,
        ));
        for blocker in &freeze_blockers {
            review_items.push(structured_stage_issue(blocker, "02", "blocker", locale));
        }
        let review_items_count = review_item_count(&review_items);
        let warnings = review_messages(&review_items);
        let inferred_entities = entity_report
            .entities
            .iter()
            .filter(|entity| entity.inference.is_some())
            .count();
        let explicit_entities = entity_report.entity_count.saturating_sub(inferred_entities);
        let mut contract_blockers = playable_blockers;
        contract_blockers.extend(freeze_blockers.clone());
        let blocking_issue_count = contract_blockers.len();
        let nonblocking_review_items = review_items
            .iter()
            .filter(|item| item.get("severity").and_then(Value::as_str) != Some("blocker"))
            .cloned()
            .collect::<Vec<_>>();
        let design_ai_review = build_design_ai_review_report(
            &entity_report,
            &project_dna,
            &open_questions,
            &contract_blockers,
            &nonblocking_review_items,
            locale,
        );
        let customization = build_customization_score_report_with_locale(
            "02",
            Some(&project_identity),
            if blocking_issue_count > 0 {
                "blocked"
            } else if review_items.is_empty() {
                "passed"
            } else {
                "review"
            },
            &contract_blockers,
            &nonblocking_review_items,
            Some(json!({
                "project_signature_present": if project_dna.get("project_signature").and_then(Value::as_str).is_some_and(|value| !value.is_empty()) { 1.0 } else { 0.0 },
                "entity_coverage_rate": entity_report.entity_coverage_rate,
                "explicit_entity_ratio": ratio(explicit_entities, entity_report.entity_count),
                "template_leakage_count": inferred_entities,
            })),
            locale,
        );
        let step02_blocked = blocking_issue_count > 0;
        let step02_ready = review_items.is_empty() && !step02_blocked;
        write_json(
            &out_dir.join("l5_entities.json"),
            &to_artifact_json_array_value(&entity_report.entities, locale)?,
        )?;
        write_json(
            &out_dir.join("entity_validation_report.json"),
            &to_artifact_json_value(&entity_report, locale)?,
        )?;
        write_json(
            &out_dir.join("entity_graph.json"),
            &to_artifact_json_value(&graph, locale)?,
        )?;
        write_json(
            &out_dir.join("phase_classification.json"),
            &to_artifact_json_value(&phases, locale)?,
        )?;
        write_json(&out_dir.join("project_dna_contract.json"), &project_dna)?;
        write_json(
            &out_dir.join("playable_scenario_contract.json"),
            &playable_scenario,
        )?;
        write_json(
            &out_dir.join("semantic_coverage_seed.json"),
            &semantic_coverage_seed,
        )?;
        write_json(
            &out_dir.join("design_ai_review_report.json"),
            &design_ai_review,
        )?;
        write_json(
            &out_dir.join("customization_score_report.json"),
            &customization,
        )?;
        write_text(&out_dir.join("frozen_game_design.md"), &frozen)?;
        write_json(
            &out_dir.join("design_freeze_report.json"),
            &json!({
                "schema_version": 1,
                "generated_at": now_iso(),
                "artifact_locale": locale,
                "source": parsed.source,
                "entity_count": entity_report.entity_count,
                "covered_entity_node_count": entity_report.covered_concrete_nodes,
                "expected_entity_node_count": entity_report.concrete_node_count,
                "entity_coverage_rate": entity_report.entity_coverage_rate,
                "entity_coverage_target": entity_report.target_coverage_rate,
                "entity_coverage_basis": entity_report.coverage_basis,
                "invalid_entity_count": entity_report.invalid_entities.len(),
                "missing_entity_count": entity_report.missing_entities.len(),
                "cycle_free": graph.cycle_free,
                "playable_contract_valid": playable_valid,
                "playable_contract_count": playable_contract_count,
                "playable_contract_blocker_count": playable_blocker_count,
                "project_dna_status": project_dna.get("status").cloned().unwrap_or_else(|| json!("blocked")),
                "blocking_issue_count": blocking_issue_count,
                "status": if step02_blocked {
                    "blocked"
                } else if step02_ready {
                    "success"
                } else {
                    "completed_with_review"
                },
            }),
        )?;
        let result_status = if step02_blocked {
            "blocked"
        } else if step02_ready {
            "success"
        } else {
            "completed_with_review"
        };
        let ai_review_status = if step02_blocked {
            "blocked"
        } else if step02_ready {
            "passed"
        } else {
            "review"
        };
        let completion_message = if step02_blocked {
            localized_text(
                locale,
                "步骤 02 因契约阻断项而停止。",
                "Step02 is blocked by contract issues.",
            )
            .to_string()
        } else {
            stage_completion_message("02", step02_ready, &warnings, review_items_count, locale)
        };
        let semantic_quality = json!({
            "status": if step02_blocked { "blocked" } else if step02_ready { "success" } else { "warning" },
            "project_specificity_score": ratio(explicit_entities, entity_report.entity_count),
            "required_semantic_coverage": entity_report.entity_coverage_rate,
            "generic_template_ratio": ratio(inferred_entities, entity_report.entity_count),
            "placeholder_ratio": ratio(entity_report.invalid_entities.len(), entity_report.entity_count),
            "return_targets": review_items.clone(),
        });
        Ok(json!({
            "status": result_status,
            "artifact_locale": locale,
            "content_exists": true,
            "traceability_valid": entity_report.invalid_entities.is_empty() && playable_valid && !step02_blocked,
            "blocking_issues": blocking_issue_count,
            "ai_review_status": ai_review_status,
            "source": parsed.source,
            "entity_count": entity_report.entity_count,
            "covered_entity_node_count": entity_report.covered_concrete_nodes,
            "expected_entity_node_count": entity_report.concrete_node_count,
            "entity_coverage_rate": entity_report.entity_coverage_rate,
            "entity_coverage_target": entity_report.target_coverage_rate,
            "entity_coverage_basis": entity_report.coverage_basis,
            "missing_entity_count": entity_report.missing_entities.len(),
            "invalid_entity_count": entity_report.invalid_entities.len(),
            "cycle_free": graph.cycle_free,
            "ai_supplement": entity_report.ai_supplement,
            "l5_entities": "l5_entities.json",
            "entity_validation_report": "entity_validation_report.json",
            "entity_graph": "entity_graph.json",
            "phase_classification": "phase_classification.json",
            "frozen_game_design": "frozen_game_design.md",
            "project_dna_contract": "project_dna_contract.json",
            "playable_scenario_contract": "playable_scenario_contract.json",
            "semantic_coverage_seed": "semantic_coverage_seed.json",
            "design_ai_review_report": "design_ai_review_report.json",
            "customization_score_report": "customization_score_report.json",
            "playable_contract_dir": "playable_contracts",
            "playable_contract_count": playable_contract_count,
            "playable_contract_valid": playable_valid,
            "playable_contract_blocker_count": playable_blocker_count,
            "playable_contract_source": playable_contract_source,
            "structured_input_status": structured_inputs.get("status").and_then(Value::as_str).unwrap_or("fallback_to_markdown"),
            "review_items_count": review_items_count,
            "review_items": review_items,
            "warnings": warnings,
            "message": completion_message,
            "semantic_quality": semantic_quality,
        }))
    }
}

fn step02_review_items(
    entity_report: &EntityCoverageReport,
    graph: &EntityGraphReport,
    playable_valid: bool,
    playable_blocker_count: usize,
    playable_review_count: usize,
    locale: ArtifactLocale,
) -> Vec<Value> {
    let mut items = Vec::new();
    if entity_report.entity_coverage_rate < entity_report.target_coverage_rate {
        items.push(json!({
            "code": "L5_ENTITY_COVERAGE_BELOW_TARGET",
            "severity": "warning",
            "return_target": "02",
            "message": if locale == ArtifactLocale::ZhCn {
                format!(
                    "L5 实体节点覆盖率为 {}/{}（{:.2}%），低于 {:.2}% 的目标；仍有 {} 个预期节点未映射（依据：{}）。",
                    entity_report.covered_concrete_nodes,
                    entity_report.concrete_node_count,
                    entity_report.entity_coverage_rate * 100.0,
                    entity_report.target_coverage_rate * 100.0,
                    entity_report.missing_entities.len(),
                    localized_coverage_basis(&entity_report.coverage_basis, locale),
                )
            } else {
                format!(
                    "L5 entity-node coverage {}/{} ({:.2}%) is below the {:.2}% target; {} expected node(s) are unmapped (basis: {}).",
                    entity_report.covered_concrete_nodes,
                    entity_report.concrete_node_count,
                    entity_report.entity_coverage_rate * 100.0,
                    entity_report.target_coverage_rate * 100.0,
                    entity_report.missing_entities.len(),
                    entity_report.coverage_basis,
                )
            },
            "affected_count": entity_report.missing_entities.len(),
        }));
    }
    if !entity_report.invalid_entities.is_empty() {
        items.push(json!({
            "code": "INVALID_L5_ENTITY",
            "severity": "warning",
            "return_target": "02",
            "message": if locale == ArtifactLocale::ZhCn {
                format!("{} 条 L5 实体记录缺少标签、类型或 schema。", entity_report.invalid_entities.len())
            } else {
                format!("{} L5 entity record(s) are missing a label, kind, or schema.", entity_report.invalid_entities.len())
            },
            "affected_count": entity_report.invalid_entities.len(),
        }));
    }
    if !graph.cycles.is_empty() {
        items.push(json!({
            "code": "ENTITY_GRAPH_CYCLE",
            "severity": "warning",
            "return_target": "02",
            "message": if locale == ArtifactLocale::ZhCn {
                format!("实体图中有 {} 个环路需要复核。", graph.cycles.len())
            } else {
                format!("{} entity graph cycle(s) require review.", graph.cycles.len())
            },
            "affected_count": graph.cycles.len(),
        }));
    }
    if !playable_valid || playable_blocker_count > 0 || playable_review_count > 0 {
        items.push(json!({
            "code": "PLAYABLE_CONTRACT_INCOMPLETE",
            "severity": if playable_blocker_count > 0 { "blocker" } else { "warning" },
            "return_target": "02",
            "message": if locale == ArtifactLocale::ZhCn {
                format!("可玩性契约需要复核：有效={playable_valid}，阻断项={playable_blocker_count}，复核项={playable_review_count}。")
            } else {
                format!("Playable contracts require review: valid={playable_valid}, blockers={playable_blocker_count}, review_items={playable_review_count}.")
            },
            "affected_count": playable_blocker_count + playable_review_count,
        }));
    }
    items
}

fn localized_coverage_basis<'a>(basis: &'a str, locale: ArtifactLocale) -> &'a str {
    if locale == ArtifactLocale::EnUs {
        return basis;
    }
    match basis {
        "structured_design_entities" => "D4 结构化设计实体",
        "explicit_l5_nodes" => "显式 L5 节点",
        "legacy_expected_node_ids" => "旧版预期节点标识",
        "design_entity_node_count" => "设计实体节点总数",
        "legacy_node_count" => "旧版节点总数",
        "selection_fallback" => "设计选择兼容回退",
        "detected_entity_nodes" => "已检测实体节点",
        _ => basis,
    }
}

fn playable_contract_bundle(
    parsed: &ParsedDesignSource,
    structured_inputs: &Value,
    locale: ArtifactLocale,
) -> (Value, &'static str) {
    let inputs = structured_inputs.get("inputs").unwrap_or(&Value::Null);
    if let Some(candidates) = structured_inputs
        .get("inputs")
        .and_then(|inputs| inputs.get("playable_contract_candidates"))
        .and_then(Value::as_object)
        .filter(|candidates| !candidates.is_empty())
    {
        let mut bundle = Value::Object(candidates.clone());
        let mut validation_input = bundle.clone();
        validation_input["artifact_locale"] = json!(locale);
        let validation = validate_playable_contract_bundle(&validation_input);
        let rebuildable = playable_validation_is_rebuildable(&validation);
        bundle["design_completeness_report"] = validation;
        if rebuildable
            && let (Some(decisions), Some(profile), Some(archetype)) = (
                inputs.get("decisions"),
                inputs.get("profile"),
                inputs.get("archetype_requirements"),
            )
        {
            let rebuilt = build_playable_contract_bundle_from_decisions_with_locale(
                decisions, profile, archetype, locale,
            );
            let rebuilt_valid = rebuilt
                .get("design_completeness_report")
                .and_then(|report| report.get("playable_completeness"))
                .and_then(|completeness| completeness.get("valid"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if rebuilt_valid {
                return (rebuilt, "structured_candidate_rebuild");
            }
        }
        return (bundle, "structured_candidate_freeze");
    }

    if let (Some(decisions), Some(profile), Some(archetype)) = (
        inputs.get("decisions"),
        inputs.get("profile"),
        inputs.get("archetype_requirements"),
    ) {
        return (
            build_playable_contract_bundle_from_decisions_with_locale(
                decisions, profile, archetype, locale,
            ),
            "structured_rebuild",
        );
    }

    (
        build_playable_contract_bundle_with_locale(
            &json!({
                "source": parsed.source,
                "selections": parsed.selection_dicts(),
            }),
            locale,
        ),
        "parsed_design_fallback",
    )
}

fn playable_validation_is_rebuildable(validation: &Value) -> bool {
    let blockers = validation
        .get("playable_completeness")
        .and_then(|completeness| completeness.get("blocking_issues"))
        .and_then(Value::as_array);
    blockers.is_some_and(|blockers| {
        !blockers.is_empty()
            && blockers.iter().all(|blocker| {
                matches!(
                    blocker.get("code").and_then(Value::as_str),
                    Some("NO_DESIGN_SELECTIONS" | "CONTRACT_INFERRED_FROM_ABSTRACT_DESIGN")
                )
            })
    })
}

pub fn generator_for_step(step_number: u32) -> AdmResult<Box<dyn StageOutputGenerator>> {
    match step_number {
        STEP00 => Ok(Box::new(Step00OutputGenerator::default())),
        STEP01 => Ok(Box::new(Step01OutputGenerator::default())),
        STEP02 => Ok(Box::new(Step02OutputGenerator::default())),
        other => Err(AdmError::new(format!(
            "Step00-02 generator cannot handle stage {other:02}"
        ))),
    }
}

fn source_manifest(
    parsed: &ParsedDesignSource,
    role: &str,
    artifact_locale: ArtifactLocale,
) -> Value {
    json!({
        "schema_version": 1,
        "generated_at": now_iso(),
        "artifact_locale": artifact_locale,
        "sources": [{
            "path": parsed.source,
            "sha256": parsed.source_sha256,
            "size_bytes": parsed.source_size_bytes,
            "line_count": parsed.source_line_count,
            "role": role,
            "source_package": parsed.source_package,
            "source_input_type": parsed.source_input_type,
        }],
    })
}

fn to_json_value<T: Serialize + ?Sized>(value: &T) -> AdmResult<Value> {
    serde_json::to_value(value)
        .map_err(|error| AdmError::new(format!("failed to serialize stage00-02 JSON: {error}")))
}

fn to_artifact_json_value<T: Serialize + ?Sized>(
    value: &T,
    locale: ArtifactLocale,
) -> AdmResult<Value> {
    let mut artifact = to_json_value(value)?;
    if let Some(object) = artifact.as_object_mut() {
        object.insert("artifact_locale".to_string(), json!(locale));
    }
    Ok(artifact)
}

fn to_artifact_json_array_value<T: Serialize>(
    values: &[T],
    locale: ArtifactLocale,
) -> AdmResult<Value> {
    let mut artifact = to_json_value(values)?;
    for item in artifact.as_array_mut().into_iter().flatten() {
        if let Some(object) = item.as_object_mut() {
            object.insert("artifact_locale".to_string(), json!(locale));
        }
    }
    Ok(artifact)
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

fn default_core_questions() -> Vec<CoreQuestion> {
    vec![
        question(
            "CQ-001",
            "project",
            "What is the product positioning?",
            &["项目定位", "游戏类型", "玩法想法"],
            &["定位", "类型", "愿景", "承诺"],
        ),
        question(
            "CQ-002",
            "project",
            "Who are the target players?",
            &["目标玩家", "玩家"],
            &["玩家", "受众", "人群", "session"],
        ),
        question(
            "CQ-003",
            "project",
            "Which platform and input model are targeted?",
            &["平台"],
            &["平台", "pc", "console", "mobile", "触屏", "手柄"],
        ),
        question(
            "CQ-004",
            "business",
            "What is the business or operation model?",
            &["商业模式"],
            &["商业", "付费", "买断", "内购", "广告", "运营"],
        ),
        question(
            "CQ-005",
            "core",
            "Is the core loop explicit?",
            &[
                "核心循环",
                "core_loop",
                "主循环",
                "Loop",
                "玩法循环",
                "游戏循环",
            ],
            &[
                "循环", "loop", "->", "→", "进入", "战斗", "奖励", "升级", "挑战", "成长",
            ],
        ),
        question(
            "CQ-006",
            "core",
            "What are the main pressure sources?",
            &[
                "压力来源",
                "challenge_profile",
                "risk_pressure",
                "失败条件",
                "挑战",
            ],
            &[
                "压力",
                "挑战",
                "失败",
                "风险",
                "敌人",
                "首领",
                "时间",
                "资源不足",
                "死亡",
            ],
        ),
        question(
            "CQ-007",
            "core",
            "How does reward cadence drive replay?",
            &["奖励节奏", "reward_loop", "奖励系统", "成长节奏", "掉落"],
            &[
                "奖励",
                "掉落",
                "解锁",
                "成长",
                "反馈",
                "构筑",
                "祝福",
                "永久成长",
                "重玩",
            ],
        ),
        question(
            "CQ-008",
            "systems",
            "Are top-level systems clear?",
            &["system_layer", "玩法系统", "系统图", "Layer 3", "游戏系统"],
            &[
                "系统",
                "system",
                "模块",
                "战斗系统",
                "经济系统",
                "成长系统",
                "房间系统",
            ],
        ),
        question(
            "CQ-009",
            "content",
            "Which core content objects exist?",
            &[
                "内容",
                "L5实体",
                "content_type_decision",
                "character_unit_decision",
                "item_resource",
            ],
            &[
                "角色", "敌人", "武器", "房间", "关卡", "实体", "weapon", "enemy", "ability",
                "技能", "物品",
            ],
        ),
        question(
            "CQ-010",
            "resources",
            "What are the resource and cost relations?",
            &["资源", "resource_flow", "经济系统", "货币", "item_resource"],
            &[
                "资源", "货币", "消耗", "产出", "经济", "升级", "解锁", "商店", "cost",
            ],
        ),
        question(
            "CQ-011",
            "runtime",
            "Are runtime flows traceable?",
            &["运行时", "runtime_flow", "状态机", "流程", "系统图"],
            &[
                "流程",
                "运行时",
                "状态",
                "存档",
                "事件",
                "触发",
                "输入",
                "输出",
                "结算",
            ],
        ),
        question(
            "CQ-012",
            "presentation",
            "Are feedback, UI, and presentation needs present?",
            &["表现", "资源", "ui_feedback", "反馈", "美术资源", "音效"],
            &[
                "UI", "界面", "反馈", "特效", "表现", "音效", "动画", "图标", "提示",
            ],
        ),
        question(
            "CQ-013",
            "technology",
            "Are technical constraints explicit?",
            &["技术", "平台范围", "运营模式", "商业模式", "项目规模"],
            &[
                "技术",
                "引擎",
                "配置",
                "平台",
                "性能",
                "indie",
                "买断",
                "离线",
                "offline",
                "multi_platform",
            ],
        ),
        question(
            "CQ-014",
            "production",
            "Can development phase and validation be inferred?",
            &["生产", "QA", "项目规模", "社交模式"],
            &[
                "阶段",
                "验收",
                "测试",
                "QA",
                "里程碑",
                "成长",
                "解锁",
                "成就",
                "indie",
                "community",
            ],
        ),
        question(
            "CQ-015",
            "risk",
            "Are major risks and boundaries recorded?",
            &["风险", "约束"],
            &["风险", "边界", "约束", "不做", "限制"],
        ),
    ]
}

fn localized_core_question(id: &str, fallback: &str, locale: ArtifactLocale) -> String {
    if locale == ArtifactLocale::EnUs {
        return fallback.to_string();
    }
    match id {
        "CQ-001" => "产品定位是什么？",
        "CQ-002" => "目标玩家是谁？",
        "CQ-003" => "目标平台和输入方式是什么？",
        "CQ-004" => "商业或运营模式是什么？",
        "CQ-005" => "核心循环是否明确？",
        "CQ-006" => "主要压力来源是什么？",
        "CQ-007" => "奖励节奏如何推动重复游玩？",
        "CQ-008" => "顶层系统是否清晰？",
        "CQ-009" => "存在哪些核心内容对象？",
        "CQ-010" => "资源与消耗之间是什么关系？",
        "CQ-011" => "运行时流程是否可追踪？",
        "CQ-012" => "是否包含反馈、界面和表现需求？",
        "CQ-013" => "技术约束是否明确？",
        "CQ-014" => "能否推导开发阶段和验收方式？",
        "CQ-015" => "是否记录了主要风险与边界？",
        _ => fallback,
    }
    .to_string()
}

fn localized_genre_evidence(id: &str, fallback: &str, locale: ArtifactLocale) -> String {
    if locale == ArtifactLocale::EnUs {
        return fallback.to_string();
    }
    match id {
        "CQ-005" => "已根据游戏类型推导核心循环。",
        "CQ-006" => "已根据游戏类型推导主要压力来源。",
        "CQ-007" => "已根据游戏类型推导奖励节奏。",
        "CQ-008" => "已根据游戏类型推导顶层系统。",
        "CQ-009" => "已根据游戏类型推导核心内容对象。",
        "CQ-010" => "已根据游戏类型推导资源关系。",
        "CQ-011" => "已根据游戏类型推导运行时流程。",
        "CQ-012" => "已根据游戏类型推导表现与反馈需求。",
        _ => fallback,
    }
    .to_string()
}

fn question(
    id: &str,
    domain: &str,
    question: &str,
    item_types: &[&str],
    keywords: &[&str],
) -> CoreQuestion {
    CoreQuestion {
        id: id.to_string(),
        domain: domain.to_string(),
        question: question.to_string(),
        item_types: item_types
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        keywords: keywords.iter().map(|value| (*value).to_string()).collect(),
    }
}

fn evidence_for(
    question: &CoreQuestion,
    selections: &[Selection],
    raw_text: &str,
) -> Vec<Evidence> {
    let item_types = question
        .item_types
        .iter()
        .map(|value| value.to_lowercase())
        .collect::<BTreeSet<_>>();
    let keywords = question
        .keywords
        .iter()
        .map(|value| value.to_lowercase())
        .collect::<Vec<_>>();
    let mut evidence = Vec::new();
    for selection in selections {
        let item_type = selection.item_type.to_lowercase();
        let haystack = selection_text(selection).to_lowercase();
        if item_types.contains(&item_type)
            || keywords.iter().any(|keyword| haystack.contains(keyword))
        {
            evidence.push(Evidence {
                label: selection.label(),
                source: selection.source_ref.clone(),
                match_kind: "selection".to_string(),
            });
        }
    }
    let raw_lower = raw_text.to_lowercase();
    let raw_matches = keywords
        .iter()
        .filter(|keyword| raw_lower.contains(keyword.as_str()))
        .count();
    let minimum_matches = if keywords.len() >= 3 { 2 } else { 1 };
    if evidence.is_empty() && raw_matches >= minimum_matches {
        evidence.push(Evidence {
            label: first_chars(raw_text, 120).unwrap_or_default(),
            source: "raw_text".to_string(),
            match_kind: if raw_matches > 1 {
                "raw_text_multi_keyword".to_string()
            } else {
                "raw_text".to_string()
            },
        });
    }
    if evidence.is_empty() {
        let genre = genre_key(raw_text, selections);
        if let Some(label) = genre_default_evidence(&genre, &question.id) {
            evidence.push(Evidence {
                label: label.to_string(),
                source: format!("genre_template:{genre}"),
                match_kind: "genre_inference".to_string(),
            });
        }
    }
    evidence
}

fn first_matching(selections: &[Selection], tokens: &[&str]) -> ProfileItem {
    matching_items(selections, tokens, 1)
        .into_iter()
        .next()
        .unwrap_or_default()
}

fn matching_items(selections: &[Selection], tokens: &[&str], limit: usize) -> Vec<ProfileItem> {
    let mut result = Vec::new();
    for selection in selections {
        let haystack = selection_text(selection).to_lowercase();
        if !tokens
            .iter()
            .any(|token| haystack.contains(&token.to_lowercase()))
        {
            continue;
        }
        result.push(ProfileItem {
            label: selection.label(),
            source: selection.source_ref.clone(),
            confidence: "explicit".to_string(),
        });
        if result.len() >= limit {
            break;
        }
    }
    result
}

fn genre_key(raw_text: &str, selections: &[Selection]) -> String {
    let haystack = selection_haystack(raw_text, selections);
    for (genre, tokens) in genre_detection_rules() {
        if tokens.iter().any(|token| haystack.contains(token)) {
            return genre.to_string();
        }
    }
    String::new()
}

fn genre_detection_rules() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        (
            "roguelike_action",
            vec!["hades", "rogue", "roguelike", "roguelite", "肉鸽"],
        ),
        (
            "farming_sim",
            vec![
                "stardew",
                "farming",
                "farm",
                "harvest",
                "种田",
                "农场",
                "生活模拟",
                "life sim",
            ],
        ),
        (
            "card_game",
            vec![
                "deck",
                "card",
                "卡牌",
                "构筑卡组",
                "slay the spire",
                "clash royale",
                "marvel snap",
                "hearthstone",
                "炉石",
            ],
        ),
        (
            "bullet_heaven",
            vec![
                "vampire survivors",
                "survivor",
                "bullet heaven",
                "幸存者",
                "弹幕生存",
                "auto battle",
                "自动战斗",
            ],
        ),
        (
            "match3",
            vec![
                "match-3",
                "match3",
                "match 3",
                "消除",
                "royal match",
                "bejeweled",
            ],
        ),
        (
            "hypercasual",
            vec![
                "hypercasual",
                "hyper casual",
                "超休闲",
                "flappy",
                "helix",
                "stack",
                "runner",
                "跑酷",
                "subway surfers",
                "crossy",
            ],
        ),
        (
            "idle",
            vec![
                "idle",
                "clicker",
                "incremental",
                "放置",
                "挂机",
                "coin master",
            ],
        ),
        (
            "souls_like",
            vec![
                "souls",
                "soulslike",
                "souls-like",
                "elden ring",
                "sekiro",
                "dark souls",
                "魂系",
                "高难度动作",
            ],
        ),
        (
            "action_adventure",
            vec![
                "god of war",
                "last of us",
                "death stranding",
                "action adventure",
                "动作冒险",
                "叙事动作",
            ],
        ),
        (
            "survival_horror",
            vec!["survival horror", "resident evil", "生存恐怖", "恐怖射击"],
        ),
        (
            "looter_shooter",
            vec![
                "looter shooter",
                "looter-shooter",
                "borderlands",
                "掠夺射击",
                "战利品射击",
            ],
        ),
        (
            "battle_royale",
            vec!["battle royale", "battle-royale", "apex", "吃鸡", "大逃杀"],
        ),
        (
            "hero_shooter",
            vec![
                "hero shooter",
                "valorant",
                "splatoon",
                "overwatch",
                "英雄射击",
                "团队射击",
            ],
        ),
        (
            "mmorpg",
            vec![
                "mmorpg",
                "mmo",
                "world of warcraft",
                "wow",
                "final fantasy xiv",
                "ff14",
                "runescape",
                "大型多人在线",
                "网络游戏",
                "网游",
            ],
        ),
        (
            "factory_sim",
            vec![
                "factory",
                "factorio",
                "automation",
                "工厂",
                "自动化",
                "生产线",
            ],
        ),
        (
            "exploration",
            vec![
                "exploration",
                "open world",
                "sandbox",
                "a short hike",
                "探索",
                "开放世界",
                "沙盒",
            ],
        ),
        (
            "metroidvania",
            vec![
                "metroidvania",
                "metroid",
                "castlevania",
                "dead cells",
                "hollow knight",
                "银河城",
                "动作平台",
            ],
        ),
        ("moba", vec!["moba", "推塔", "对线"]),
        (
            "brawler",
            vec!["brawler", "brawl stars", "arena", "乱斗", "竞技场"],
        ),
        ("fps", vec!["fps", "shooter", "射击", "枪"]),
        ("puzzle", vec!["puzzle", "match", "解谜", "消除"]),
        ("strategy", vec!["strategy", "rts", "4x", "策略", "战棋"]),
        (
            "rpg",
            vec!["rpg", "jrpg", "arpg", "role-playing", "角色扮演"],
        ),
    ]
}

fn genre_default_evidence(genre: &str, question_id: &str) -> Option<&'static str> {
    match (genre, question_id) {
        ("roguelike_action", "CQ-005") => Some(
            "Core loop: room entry -> combat clear -> reward choice -> build growth -> boss challenge.",
        ),
        ("roguelike_action", "CQ-006") => {
            Some("Main pressure: room combat, enemy mix, death restart, and resource tradeoffs.")
        }
        ("roguelike_action", "CQ-007") => Some(
            "Reward cadence: clear rewards, build choices, run upgrades, and meta progression.",
        ),
        ("roguelike_action", "CQ-008") => Some(
            "Top systems: action combat, room progression, reward choice, build growth, and meta progression.",
        ),
        ("roguelike_action", "CQ-009") => {
            Some("Core content objects: weapons, enemies, abilities, rooms, resources, and bosses.")
        }
        ("roguelike_action", "CQ-010") => Some(
            "Resource relation: clear rewards, currency spend, upgrade unlocks, and build gains.",
        ),
        ("roguelike_action", "CQ-011") => {
            Some("Runtime flow: room load -> combat clear -> reward settlement -> next room.")
        }
        ("roguelike_action", "CQ-012") => Some(
            "Presentation: attack feedback, hit effects, reward icons, room readability, and combat UI.",
        ),
        ("fps", "CQ-005") => Some(
            "Core loop: discover target -> move for position -> shoot -> acquire equipment -> complete objective.",
        ),
        ("fps", "CQ-006") => {
            Some("Main pressure: enemy fire, exposed position, ammo drain, and objective timing.")
        }
        ("fps", "CQ-008") => {
            Some("Top systems: shooting, movement, equipment, objective, and spawning systems.")
        }
        ("puzzle", "CQ-005") => Some(
            "Core loop: observe board -> attempt move -> receive feedback -> unlock next puzzle.",
        ),
        ("puzzle", "CQ-006") => Some(
            "Main pressure: rule understanding, move limits, error feedback, and progressive difficulty.",
        ),
        ("strategy", "CQ-005") => Some(
            "Core loop: plan deployment -> execute operations -> observe result -> adjust strategy.",
        ),
        ("rpg", "CQ-005") => Some(
            "Core loop: accept goal -> explore encounter -> fight or solve -> gain equipment -> grow character.",
        ),
        ("moba", "CQ-005") => Some(
            "Core loop: choose hero -> lane and grow -> contest resources -> team fight -> push objective.",
        ),
        _ => None,
    }
}

fn fallback_loop_with_locale(raw_text: &str, locale: ArtifactLocale) -> String {
    if locale == ArtifactLocale::EnUs {
        let lower = raw_text.to_lowercase();
        if lower.contains("rogue") {
            return "Enter combat -> gain reward -> grow build -> challenge next room".to_string();
        }
        if ["strategy", "rts", "4x"]
            .iter()
            .any(|token| lower.contains(token))
        {
            return "Plan deployment -> execute orders -> observe result -> adjust strategy -> contest victory".to_string();
        }
        if ["rpg", "jrpg", "arpg"]
            .iter()
            .any(|token| lower.contains(token))
        {
            return "Accept objective -> explore and fight -> gain equipment -> grow character -> advance story".to_string();
        }
        if ["moba", "tower defense", "tower_defense"]
            .iter()
            .any(|token| lower.contains(token))
        {
            return "Deploy or choose route -> oppose enemy -> gain resources -> upgrade build -> advance objective".to_string();
        }
        if lower.contains("puzzle") {
            return "Observe state -> attempt action -> receive feedback -> unlock next puzzle"
                .to_string();
        }
        if lower.contains("fps") || lower.contains("shooter") {
            return "Find target -> move and shoot -> control space -> gain equipment".to_string();
        }
        return "Understand objective -> perform core action -> receive feedback -> advance to next objective".to_string();
    }
    let lower = raw_text.to_lowercase();
    if lower.contains("rogue") || raw_text.contains("肉鸽") || raw_text.contains("Roguelike") {
        return "进入战斗 -> 获得奖励 -> 构筑成长 -> 挑战下一房间".to_string();
    }
    if ["strategy", "rts", "4x"]
        .iter()
        .any(|token| lower.contains(token))
        || ["策略", "战棋"]
            .iter()
            .any(|token| raw_text.contains(token))
    {
        return "规划部署 -> 执行操作 -> 观察结果 -> 调整策略 -> 争夺胜利".to_string();
    }
    if ["rpg", "jrpg", "arpg"]
        .iter()
        .any(|token| lower.contains(token))
        || raw_text.contains("角色扮演")
    {
        return "接取目标 -> 探索战斗 -> 获得装备 -> 角色成长 -> 推进剧情".to_string();
    }
    if ["moba", "tower defense", "tower_defense"]
        .iter()
        .any(|token| lower.contains(token))
        || ["塔防", "推塔"]
            .iter()
            .any(|token| raw_text.contains(token))
    {
        return "部署/选路 -> 对抗敌方 -> 获取资源 -> 升级构筑 -> 推进目标".to_string();
    }
    if lower.contains("puzzle") || raw_text.contains("解谜") {
        return "观察局面 -> 尝试操作 -> 获得反馈 -> 解锁下一谜题".to_string();
    }
    if lower.contains("fps") || raw_text.contains("射击") {
        return "发现目标 -> 移动射击 -> 占领空间 -> 获取装备".to_string();
    }
    "理解目标 -> 执行核心动作 -> 获得反馈 -> 推进下一目标".to_string()
}

fn default_genre_templates() -> BTreeMap<String, GenreTemplate> {
    BTreeMap::from([
        (
            "roguelike_action".to_string(),
            GenreTemplate {
                core_loop: strings(&[
                    "选择武器",
                    "进入房间",
                    "战斗清场",
                    "选择奖励",
                    "升级构筑",
                    "挑战首领",
                    "死亡后永久成长",
                ]),
                systems: vec![
                    system(
                        "SYS-COMBAT",
                        "即时战斗系统",
                        "处理攻击、受击、移动、敌人行为与战斗反馈。",
                    ),
                    system(
                        "SYS-ROOM",
                        "房间推进系统",
                        "组织房间节点、遭遇类型、出口和下一步选择。",
                    ),
                    system(
                        "SYS-REWARD",
                        "奖励选择系统",
                        "在清场后提供祝福、资源、武器成长或事件奖励。",
                    ),
                    system(
                        "SYS-BUILD",
                        "构筑成长系统",
                        "累计武器、技能、祝福和被动能力的组合效果。",
                    ),
                    system(
                        "SYS-META",
                        "局外成长系统",
                        "在失败后沉淀永久资源、解锁和叙事推进。",
                    ),
                    system("SYS-BOSS", "首领挑战系统", "提供阶段性高压战斗和进度检查。"),
                ],
            },
        ),
        (
            "fps".to_string(),
            GenreTemplate {
                core_loop: strings(&["侦察空间", "交火压制", "移动换位", "拾取补给", "完成目标"]),
                systems: vec![
                    system(
                        "SYS-GUNPLAY",
                        "枪械手感系统",
                        "处理武器射击、后坐力、命中和反馈。",
                    ),
                    system("SYS-MOVEMENT", "移动系统", "处理奔跑、跳跃、掩体和换位。"),
                    system("SYS-ENCOUNTER", "遭遇系统", "组织敌人、空间和目标压力。"),
                    system("SYS-LOADOUT", "装备系统", "管理武器、配件、道具和切换。"),
                    system(
                        "SYS-OBJECTIVE",
                        "目标系统",
                        "提供任务目标、进度和胜负判定。",
                    ),
                ],
            },
        ),
        (
            "puzzle".to_string(),
            GenreTemplate {
                core_loop: strings(&["观察谜面", "形成假设", "执行操作", "获得反馈", "解锁新规则"]),
                systems: vec![
                    system("SYS-RULE", "规则表达系统", "展示谜题规则、约束和变化。"),
                    system("SYS-INPUT", "操作系统", "处理拖拽、点击、移动或组合输入。"),
                    system("SYS-FEEDBACK", "反馈系统", "让玩家理解尝试结果和错误原因。"),
                    system(
                        "SYS-PROGRESSION",
                        "关卡推进系统",
                        "组织难度、章节和新机制解锁。",
                    ),
                    system("SYS-HINT", "提示系统", "在卡关时提供渐进式帮助。"),
                ],
            },
        ),
        (
            "strategy".to_string(),
            GenreTemplate {
                core_loop: strings(&["规划部署", "执行操作", "观察结果", "调整策略", "争夺胜利"]),
                systems: vec![
                    system("SYS-UNIT", "单位系统", "管理单位属性、行为、生产和升级。"),
                    system("SYS-MAP", "地图系统", "管理地形、资源点、区域控制和视野。"),
                    system(
                        "SYS-ECONOMY",
                        "经济系统",
                        "处理资源采集、消耗、科技和建造节奏。",
                    ),
                    system("SYS-AI", "AI系统", "驱动敌方决策、战术响应和难度调整。"),
                    system(
                        "SYS-COMBAT",
                        "战斗解算系统",
                        "处理单位交战、伤害计算和结果判定。",
                    ),
                ],
            },
        ),
        (
            "rpg".to_string(),
            GenreTemplate {
                core_loop: strings(&[
                    "接取目标",
                    "探索遭遇",
                    "战斗解谜",
                    "获得装备",
                    "角色成长",
                    "推进剧情",
                ]),
                systems: vec![
                    system("SYS-QUEST", "任务系统", "组织目标、分支、奖励和任务状态。"),
                    system(
                        "SYS-CHARACTER",
                        "角色成长系统",
                        "管理属性、技能、装备和等级成长。",
                    ),
                    system(
                        "SYS-COMBAT",
                        "战斗系统",
                        "处理战斗规则、技能释放和受击反馈。",
                    ),
                    system(
                        "SYS-INVENTORY",
                        "背包装备系统",
                        "管理物品、装备、掉落和消耗品。",
                    ),
                    system(
                        "SYS-NARRATIVE",
                        "叙事系统",
                        "管理对话、剧情状态和角色关系。",
                    ),
                ],
            },
        ),
        ("generic".to_string(), generic_template()),
    ])
}

fn generic_template() -> GenreTemplate {
    GenreTemplate {
        core_loop: strings(&[
            "设定目标",
            "执行核心动作",
            "获得反馈",
            "获取奖励",
            "推进下一挑战",
        ]),
        systems: vec![
            system("SYS-CORE", "核心操作系统", "承载玩家每轮最常执行的动作。"),
            system("SYS-GOAL", "目标系统", "定义阶段目标、完成条件和失败条件。"),
            system(
                "SYS-FEEDBACK",
                "反馈系统",
                "提供即时反馈、奖励反馈和错误反馈。",
            ),
            system(
                "SYS-PROGRESSION",
                "进度系统",
                "组织关卡、章节、解锁和成长。",
            ),
            system(
                "SYS-CONTENT",
                "内容系统",
                "管理可游玩对象、规则变化和内容扩展。",
            ),
        ],
    }
}

fn english_genre_template(key: &str) -> GenreTemplate {
    let (loop_nodes, systems): (&[&str], Vec<SystemDefinition>) = match key {
        "roguelike_action" => (
            &[
                "Choose weapon",
                "Enter room",
                "Clear combat",
                "Choose reward",
                "Upgrade build",
                "Challenge boss",
                "Keep meta progression",
            ] as &[&str],
            vec![
                system(
                    "SYS-COMBAT",
                    "Real-time Combat System",
                    "Handle attacks, damage, movement, enemy behavior, and combat feedback.",
                ),
                system(
                    "SYS-ROOM",
                    "Room Progression System",
                    "Organize room nodes, encounters, exits, and route choices.",
                ),
                system(
                    "SYS-REWARD",
                    "Reward Choice System",
                    "Offer blessings, resources, weapon growth, or event rewards after combat.",
                ),
                system(
                    "SYS-BUILD",
                    "Build Growth System",
                    "Combine weapons, abilities, blessings, and passive effects.",
                ),
                system(
                    "SYS-META",
                    "Meta Progression System",
                    "Persist resources, unlocks, and narrative progress after failure.",
                ),
            ],
        ),
        "fps" => (
            &[
                "Survey space",
                "Exchange fire",
                "Reposition",
                "Collect supplies",
                "Complete objective",
            ],
            vec![
                system(
                    "SYS-GUNPLAY",
                    "Gunplay System",
                    "Handle firing, recoil, hits, and weapon feedback.",
                ),
                system(
                    "SYS-MOVEMENT",
                    "Movement System",
                    "Handle running, jumping, cover, and repositioning.",
                ),
                system(
                    "SYS-ENCOUNTER",
                    "Encounter System",
                    "Organize enemies, spaces, and objective pressure.",
                ),
                system(
                    "SYS-LOADOUT",
                    "Loadout System",
                    "Manage weapons, attachments, items, and switching.",
                ),
                system(
                    "SYS-OBJECTIVE",
                    "Objective System",
                    "Track mission objectives, progress, and outcomes.",
                ),
            ],
        ),
        "puzzle" => (
            &[
                "Observe puzzle",
                "Form hypothesis",
                "Take action",
                "Receive feedback",
                "Unlock new rule",
            ],
            vec![
                system(
                    "SYS-RULE",
                    "Rule Presentation System",
                    "Present puzzle rules, constraints, and changes.",
                ),
                system(
                    "SYS-INPUT",
                    "Interaction System",
                    "Handle drag, click, movement, or combination input.",
                ),
                system(
                    "SYS-FEEDBACK",
                    "Feedback System",
                    "Explain attempt results and failure causes.",
                ),
                system(
                    "SYS-PROGRESSION",
                    "Level Progression System",
                    "Organize difficulty, chapters, and mechanic unlocks.",
                ),
                system(
                    "SYS-HINT",
                    "Hint System",
                    "Provide progressive assistance when the player is stuck.",
                ),
            ],
        ),
        "strategy" => (
            &[
                "Plan deployment",
                "Execute orders",
                "Observe results",
                "Adjust strategy",
                "Contest victory",
            ],
            vec![
                system(
                    "SYS-UNIT",
                    "Unit System",
                    "Manage unit attributes, behavior, production, and upgrades.",
                ),
                system(
                    "SYS-MAP",
                    "Map System",
                    "Manage terrain, resources, area control, and visibility.",
                ),
                system(
                    "SYS-ECONOMY",
                    "Economy System",
                    "Handle gathering, spending, technology, and construction pace.",
                ),
                system(
                    "SYS-AI",
                    "AI System",
                    "Drive opponent decisions, tactical response, and difficulty.",
                ),
                system(
                    "SYS-COMBAT",
                    "Combat Resolution System",
                    "Resolve engagements, damage, and outcomes.",
                ),
            ],
        ),
        "rpg" => (
            &[
                "Accept objective",
                "Explore encounter",
                "Fight or solve",
                "Gain equipment",
                "Grow character",
                "Advance story",
            ],
            vec![
                system(
                    "SYS-QUEST",
                    "Quest System",
                    "Organize objectives, branches, rewards, and quest states.",
                ),
                system(
                    "SYS-CHARACTER",
                    "Character Progression System",
                    "Manage attributes, skills, equipment, and levels.",
                ),
                system(
                    "SYS-COMBAT",
                    "Combat System",
                    "Handle combat rules, ability use, and damage feedback.",
                ),
                system(
                    "SYS-INVENTORY",
                    "Inventory System",
                    "Manage items, equipment, drops, and consumables.",
                ),
                system(
                    "SYS-NARRATIVE",
                    "Narrative System",
                    "Manage dialogue, story state, and character relationships.",
                ),
            ],
        ),
        _ => (
            &[
                "Set objective",
                "Perform core action",
                "Receive feedback",
                "Gain reward",
                "Advance to next challenge",
            ],
            vec![
                system(
                    "SYS-CORE",
                    "Core Interaction System",
                    "Own the action players perform most often.",
                ),
                system(
                    "SYS-GOAL",
                    "Goal System",
                    "Define objectives, completion conditions, and failure conditions.",
                ),
                system(
                    "SYS-FEEDBACK",
                    "Feedback System",
                    "Provide immediate, reward, and error feedback.",
                ),
                system(
                    "SYS-PROGRESSION",
                    "Progression System",
                    "Organize levels, chapters, unlocks, and growth.",
                ),
                system(
                    "SYS-CONTENT",
                    "Content System",
                    "Manage playable objects, rule variation, and content expansion.",
                ),
            ],
        ),
    };
    GenreTemplate {
        core_loop: strings(loop_nodes),
        systems,
    }
}

fn localized_loop_fallback(locale: ArtifactLocale) -> Vec<String> {
    if locale == ArtifactLocale::ZhCn {
        strings(&["理解目标", "执行核心动作", "获得反馈", "推进下一目标"])
    } else {
        strings(&[
            "Understand objective",
            "Perform core action",
            "Receive feedback",
            "Advance to next objective",
        ])
    }
}

fn system(id: &str, name: &str, responsibility: &str) -> SystemDefinition {
    SystemDefinition {
        id: id.to_string(),
        name: name.to_string(),
        responsibility: responsibility.to_string(),
        source: "genre_template".to_string(),
        confidence: "fallback".to_string(),
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn explicit_core_loop(selections: &[Selection]) -> Vec<String> {
    for selection in selections {
        if !["核心循环", "core loop"]
            .iter()
            .any(|item_type| selection.item_type.eq_ignore_ascii_case(item_type))
        {
            continue;
        }
        let normalized = selection
            .option
            .replace('→', "->")
            .replace("=>", "->")
            .replace('⇒', "->")
            .replace(['/', '、', ',', '，'], "->");
        let parts = normalized
            .split("->")
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        return if parts.is_empty() && !selection.option.trim().is_empty() {
            vec![selection.option.clone()]
        } else {
            parts
        };
    }
    Vec::new()
}

fn systems_from_graph(system_graph: &Value, locale: ArtifactLocale) -> Vec<SystemDefinition> {
    system_graph
        .get("nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .filter_map(|(index, node)| {
            let mut name = string_field(node, "name");
            for prefix in ["system_layer:", "system_layer：", "system:", "system："] {
                if let Some(rest) = name.to_lowercase().strip_prefix(prefix) {
                    name = rest.trim().to_string();
                }
            }
            if name.is_empty() {
                None
            } else {
                Some(SystemDefinition {
                    id: non_empty_or(string_field(node, "id"), &format!("SYS-{:03}", index + 1)),
                    responsibility: if locale == ArtifactLocale::ZhCn {
                        format!("承载{name}相关的核心玩法职责。")
                    } else {
                        format!("Own the core gameplay responsibilities related to {name}.")
                    },
                    name,
                    source: string_field(node, "source"),
                    confidence: "explicit".to_string(),
                })
            }
        })
        .collect()
}

fn system_graph_from_selections(parsed: &ParsedDesignSource) -> Value {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for selection in &parsed.selections {
        if !selection_text(selection).to_lowercase().contains("system")
            && !selection.item_type.contains("系统")
            && !selection.layer_title.contains("系统")
        {
            continue;
        }
        let id = selection
            .dependencies
            .first()
            .cloned()
            .unwrap_or_else(|| format!("SYS-{:03}", nodes.len() + 1));
        nodes.push(json!({
            "id": id,
            "name": if selection.option.is_empty() { selection.item_type.clone() } else { selection.option.clone() },
            "source": selection.source_ref,
        }));
        for dep in &selection.dependencies {
            if dep != &id {
                edges.push(json!({
                    "from": dep,
                    "to": id,
                    "relation": "depends_on",
                    "source": selection.source_ref,
                }));
            }
        }
    }
    json!({"nodes": nodes, "edges": edges})
}

fn render_idea_intake(
    parsed: &ParsedDesignSource,
    profile: &ConceptProfile,
    coverage: &QuestionCoverageReport,
    locale: ArtifactLocale,
) -> String {
    if locale == ArtifactLocale::ZhCn {
        format!(
            "# 步骤 00：创意收集\n\n- 来源：{}\n- 项目定位：{}\n- 核心循环：{}\n- 问题覆盖率：{:.2}\n",
            parsed.source,
            profile.project_positioning.label,
            profile.core_loop.label,
            coverage.coverage_rate
        )
    } else {
        format!(
            "# Step00 Idea Intake\n\n- Source: {}\n- Project positioning: {}\n- Core loop: {}\n- Question coverage: {:.2}\n",
            parsed.source,
            profile.project_positioning.label,
            profile.core_loop.label,
            coverage.coverage_rate
        )
    }
}

fn render_gameplay_framework(
    parsed: &ParsedDesignSource,
    core_loop: &CoreLoopReport,
    systems: &SystemDefinitionsReport,
    locale: ArtifactLocale,
) -> String {
    let loop_text = core_loop
        .loop_nodes
        .iter()
        .enumerate()
        .map(|(index, item)| format!("{}. {item}", index + 1))
        .collect::<Vec<_>>()
        .join("\n");
    let mut text = if locale == ArtifactLocale::ZhCn {
        format!(
            "# 玩法框架\n\n- 来源：{}\n- 模板：{}\n\n## 核心循环\n\n{}\n\n## 系统\n\n",
            parsed.source, core_loop.template_key, loop_text
        )
    } else {
        format!(
            "# Gameplay Framework\n\n- Source: {}\n- Template: {}\n\n## Core Loop\n\n{}\n\n## Systems\n\n",
            parsed.source, core_loop.template_key, loop_text
        )
    };
    for system in &systems.systems {
        text.push_str(&format!(
            "- {} ({}) - {}\n",
            system.name, system.id, system.responsibility
        ));
    }
    text
}

fn synthetic_entities(parsed: &ParsedDesignSource, limit: usize) -> Vec<DesignEntity> {
    parsed
        .selections
        .iter()
        .filter(|selection| {
            !is_l5_entity_item_type(&selection.item_type) && !selection.label().is_empty()
        })
        .take(limit)
        .enumerate()
        .map(|(index, selection)| {
            let kind = entity_kind_for(selection);
            let node_id = selection.id();
            DesignEntity {
                entity_id: format!("ENT-{:03}", index + 1),
                label: selection.label(),
                kind: kind.clone(),
                schema: format!("inferred.{kind}.v1"),
                status: String::new(),
                source: selection.source_ref.clone(),
                source_selection_id: node_id.clone(),
                node_id,
                dependencies: selection.dependencies.clone(),
                purpose: if selection.purpose.is_empty() {
                    "Generated from current design selection fallback.".to_string()
                } else {
                    selection.purpose.clone()
                },
                inference: Some(json!({
                    "mode": "local_selection_fallback",
                    "reason": "No explicit L5 entity selections were found.",
                })),
                supplement_basis: None,
                completed_from: None,
            }
        })
        .collect()
}

fn localize_inferred_entities(entities: &mut [DesignEntity], locale: ArtifactLocale) {
    if locale == ArtifactLocale::EnUs {
        return;
    }
    for entity in entities {
        let local_fallback = entity
            .inference
            .as_ref()
            .and_then(|value| value.get("mode"))
            .and_then(Value::as_str)
            == Some("local_selection_fallback");
        if !local_fallback {
            continue;
        }
        if entity.purpose == "Generated from current design selection fallback." {
            entity.purpose = "根据当前设计选择生成的兼容回退实体。".to_string();
        }
        if let Some(inference) = entity.inference.as_mut()
            && inference.get("reason").and_then(Value::as_str)
                == Some("No explicit L5 entity selections were found.")
        {
            inference["reason"] = json!("未找到显式 L5 实体选择。");
        }
    }
}

fn localize_supplement_result(result: &mut SupplementResult, locale: ArtifactLocale) {
    for entity in &mut result.entities {
        localize_fallback_entity(entity, locale);
    }
    result.supplement_basis_samples = result
        .entities
        .iter()
        .filter_map(|entity| entity.supplement_basis.clone())
        .filter(|value| !value.is_empty())
        .take(3)
        .collect();
}

fn localize_fallback_entity(entity: &mut DesignEntity, locale: ArtifactLocale) {
    if !entity.source.starts_with("ai_supplement_") {
        return;
    }
    if locale == ArtifactLocale::EnUs {
        entity.label = match entity.label.as_str() {
            "核心角色" => "Core Player Character",
            "核心能力" => "Core Ability",
            "核心资源" => "Core Resource",
            "标准场景" => "Standard Scene",
            "主界面信息层" => "Main HUD Information Layer",
            "冥界近战武器" => "Underworld Melee Weapon",
            "冲刺斩击" => "Dash Slash",
            "冥府斗士" => "Underworld Fighter",
            "标准战斗房间" => "Standard Combat Room",
            "骸骨敌兵" => "Skeleton Soldier",
            "关底守卫" => "Stage Guardian",
            "暗影货币" => "Shadow Currency",
            "战斗状态 HUD" => "Combat Status HUD",
            _ => return,
        }
        .to_string();
        return;
    }

    let localized_basis = if entity.source == "ai_supplement_missing_node_fallback" {
        format!("针对缺失设计节点 `{}` 生成的本地补全依据。", entity.node_id)
    } else {
        match entity.kind.as_str() {
            "character" => "用于描述玩家可控制角色的本地补全依据。".to_string(),
            "ability" => "用于描述核心交互能力的本地补全依据。".to_string(),
            "resource" => "用于描述奖励与消耗对象的本地补全依据。".to_string(),
            "scene" | "room" => "用于描述承载玩法内容场景的本地补全依据。".to_string(),
            "ui" => "用于描述运行时状态界面的本地补全依据。".to_string(),
            "weapon" => "用于描述核心战斗武器的本地补全依据。".to_string(),
            "enemy" => "用于描述战斗遭遇敌人的本地补全依据。".to_string(),
            _ => "根据当前设计生成的本地补全依据。".to_string(),
        }
    };
    let previous_basis = entity.supplement_basis.clone().unwrap_or_default();
    if entity.purpose.is_empty() || entity.purpose == previous_basis {
        entity.purpose = localized_basis.clone();
    }
    entity.supplement_basis = Some(localized_basis);
    if entity.source == "ai_supplement_missing_node_fallback" {
        entity.label = format!(
            "{} 对应的{}实体",
            entity.node_id,
            localized_kind(&entity.kind)
        );
    }
}

fn localized_kind(kind: &str) -> &'static str {
    match kind {
        "weapon" => "武器",
        "character" => "角色",
        "enemy" => "敌人",
        "ability" => "能力",
        "room" => "房间",
        "resource" => "资源",
        "ui" => "界面",
        "scene" => "场景",
        "system" => "系统",
        "loop" => "循环",
        "numeric_curve" => "数值曲线",
        "content" => "内容",
        "encounter" => "遭遇",
        "config" => "配置",
        "audio" => "音频",
        _ => "设计",
    }
}

fn entity_kind_for(selection: &Selection) -> String {
    let text = format!("{} {}", selection.label(), selection.purpose).to_lowercase();
    for (kind, tokens) in [
        ("ui", &["ui", "hud", "menu", "界面", "图标"][..]),
        (
            "weapon",
            &[
                "weapon", "sword", "blade", "bow", "gun", "武器", "剑", "刀", "弓", "枪",
            ],
        ),
        (
            "enemy",
            &["enemy", "boss", "monster", "敌人", "首领", "怪物"],
        ),
        (
            "resource",
            &[
                "resource", "currency", "economy", "资源", "货币", "经济", "item", "loot", "drop",
                "物品", "道具", "掉落",
            ],
        ),
        (
            "ability",
            &["skill", "ability", "attack", "技能", "攻击", "特效"],
        ),
        ("room", &["room", "level", "房间", "关卡"]),
        ("scene", &["scene", "environment", "场景", "环境"]),
        ("character", &["character", "avatar", "角色", "主角", "npc"]),
        (
            "audio",
            &["audio", "sound", "music", "音频", "音效", "音乐"],
        ),
        ("config", &["config", "setting", "配置", "参数"]),
        ("system", &["system", "loop", "玩法", "系统", "循环"]),
    ] {
        if tokens.iter().any(|token| text.contains(token)) {
            return kind.to_string();
        }
    }
    "design_selection".to_string()
}

fn expected_node_ids(parsed: &ParsedDesignSource) -> (Vec<String>, bool) {
    let mut explicit_node_ids = Vec::new();
    let mut explicit_seen = BTreeSet::new();
    for selection in &parsed.selections {
        if is_l5_node_item_type(&selection.item_type) {
            push_node_id(
                &mut explicit_node_ids,
                &mut explicit_seen,
                &selection.option,
            );
        }
    }
    for line in parsed.raw_text.lines() {
        let trimmed = line.trim();
        for prefix in [
            "- L5节点:",
            "- L5节点：",
            "- L5 节点:",
            "- L5 节点：",
            "- L5 node:",
            "- L5 node：",
        ] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                push_node_id(&mut explicit_node_ids, &mut explicit_seen, rest);
            }
        }
    }
    if !explicit_node_ids.is_empty() {
        return (explicit_node_ids, true);
    }

    let mut node_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for key in [
        "design_nodes",
        "template_nodes",
        "expected_nodes",
        "node_ids",
    ] {
        if let Some(items) = parsed.design_summary.get(key).and_then(Value::as_array) {
            for item in items {
                collect_node_payload(item, &mut node_ids, &mut seen);
            }
        }
    }
    for selection in &parsed.selections {
        if ["设计节点", "Design Node"].contains(&selection.item_type.as_str()) {
            push_node_id(&mut node_ids, &mut seen, &selection.option);
        } else if selection.layer_title == "设计决策"
            && !is_l5_entity_item_type(&selection.item_type)
            && !["资源", "表现"].contains(&selection.item_type.as_str())
        {
            push_node_id(&mut node_ids, &mut seen, &selection.item_type);
        }
    }
    (node_ids, false)
}

fn collect_node_payload(payload: &Value, node_ids: &mut Vec<String>, seen: &mut BTreeSet<String>) {
    if let Some(text) = payload.as_str() {
        push_node_id(node_ids, seen, text);
        return;
    }
    for key in ["node_id", "id", "key"] {
        if let Some(text) = payload.get(key).and_then(Value::as_str) {
            push_node_id(node_ids, seen, text);
            return;
        }
    }
}

fn push_node_id(node_ids: &mut Vec<String>, seen: &mut BTreeSet<String>, value: &str) {
    let node_id = value
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(['：', ':', ',', '，', ';', '；'])
        .to_string();
    if !node_id.is_empty() && seen.insert(node_id.clone()) {
        node_ids.push(node_id);
    }
}

fn filter_governance_nodes(node_ids: Vec<String>, parsed: &ParsedDesignSource) -> Vec<String> {
    let is_liveops = is_liveops_project(parsed);
    node_ids
        .into_iter()
        .filter(|node_id| {
            !["documentation_", "help_support_"]
                .iter()
                .any(|prefix| node_id.starts_with(prefix))
                && (is_liveops
                    || ![
                        "liveops_",
                        "data_",
                        "retention_",
                        "launch_",
                        "release_",
                        "compliance_",
                    ]
                    .iter()
                    .any(|prefix| node_id.starts_with(prefix)))
        })
        .collect()
}

fn is_liveops_project(parsed: &ParsedDesignSource) -> bool {
    let text = format!(
        "{} {}",
        parsed.raw_text.to_lowercase(),
        parsed
            .selections
            .iter()
            .map(selection_text)
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    );
    if [
        "live_service",
        "free_to_play",
        "subscription",
        "season_pass",
        "liveops",
        "large_service",
        "长线服务",
        "长线运营",
        "服务型",
        "赛季",
    ]
    .iter()
    .any(|token| text.contains(token))
    {
        return true;
    }
    if [
        "buyout",
        "buy_to_play",
        "offline_single_release",
        "offline",
        "single_release",
        "买断",
        "离线",
        "单机",
        "单次发布",
    ]
    .iter()
    .any(|token| text.contains(token))
    {
        return false;
    }
    true
}

fn expected_node_count(parsed: &ParsedDesignSource, entities: &[DesignEntity]) -> usize {
    if let Some(count) = summary_positive_count(parsed, "design_entity_node_count") {
        return count;
    }
    if let Some(count) = summary_positive_count(parsed, "node_count") {
        return count;
    }
    if entities.iter().any(|entity| entity.inference.is_some()) {
        return parsed
            .selections
            .iter()
            .filter(|selection| {
                !is_l5_entity_item_type(&selection.item_type) && !selection.label().is_empty()
            })
            .count();
    }
    0
}

fn summary_positive_count(parsed: &ParsedDesignSource, key: &str) -> Option<usize> {
    parsed
        .design_summary
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value > 0)
}

fn entity_node_ids(entities: &[DesignEntity]) -> BTreeSet<String> {
    entities
        .iter()
        .filter_map(|entity| {
            if entity.node_id.is_empty() {
                None
            } else {
                Some(entity.node_id.clone())
            }
        })
        .collect()
}

fn infer_kind_from_node_id(node_id: &str) -> String {
    let text = node_id.to_lowercase();
    for (kind, tokens) in [
        ("system", &["system", "build", "runtime", "系统"][..]),
        ("operation", &["operation", "input", "action", "操作"]),
        ("loop", &["loop", "cycle", "循环"]),
        ("ability", &["ability", "skill", "attack", "技能", "攻击"]),
        ("numeric_curve", &["curve", "numeric", "balance", "数值"]),
        ("content", &["content", "room", "level", "内容", "房间"]),
    ] {
        if tokens.iter().any(|token| text.contains(token)) {
            return kind.to_string();
        }
    }
    "unknown".to_string()
}

fn supplement_priority_rank(kind: &str) -> usize {
    [
        "system",
        "operation",
        "loop",
        "ability",
        "numeric_curve",
        "content",
    ]
    .iter()
    .position(|value| *value == kind)
    .unwrap_or(usize::MAX)
}

fn fallback_entity_library() -> BTreeMap<String, Vec<DesignEntity>> {
    BTreeMap::from([
        (
            "generic".to_string(),
            vec![
                fallback_entity(
                    "核心角色",
                    "character",
                    "character.v1",
                    "character_node",
                    "Generic player-controlled object",
                ),
                fallback_entity(
                    "核心能力",
                    "ability",
                    "ability.v1",
                    "ability_node",
                    "Generic main interaction ability",
                ),
                fallback_entity(
                    "核心资源",
                    "resource",
                    "resource.v1",
                    "resource_node",
                    "Generic reward and cost object",
                ),
                fallback_entity(
                    "标准场景",
                    "scene",
                    "scene.v1",
                    "room_node",
                    "Generic content-bearing scene",
                ),
                fallback_entity(
                    "主界面信息层",
                    "ui",
                    "ui.v1",
                    "ui_node",
                    "Generic runtime status UI object",
                ),
            ],
        ),
        (
            "roguelike_action".to_string(),
            vec![
                fallback_entity(
                    "冥界近战武器",
                    "weapon",
                    "weapon.v1",
                    "combat_node",
                    "Roguelike action base combat weapon",
                ),
                fallback_entity(
                    "冲刺斩击",
                    "ability",
                    "ability.v1",
                    "ability_node",
                    "Roguelike movement attack ability",
                ),
                fallback_entity(
                    "冥府斗士",
                    "character",
                    "character.v1",
                    "character_node",
                    "Roguelike player character",
                ),
                fallback_entity(
                    "标准战斗房间",
                    "room",
                    "room.v1",
                    "room_node",
                    "Roguelike room clear node",
                ),
                fallback_entity(
                    "骸骨敌兵",
                    "enemy",
                    "enemy.v1",
                    "combat_node",
                    "Roguelike base encounter enemy",
                ),
                fallback_entity(
                    "关底守卫",
                    "enemy",
                    "enemy.v1",
                    "boss_node",
                    "Roguelike boss entity",
                ),
                fallback_entity(
                    "暗影货币",
                    "resource",
                    "resource.v1",
                    "resource_node",
                    "Roguelike run reward resource",
                ),
                fallback_entity(
                    "战斗状态 HUD",
                    "ui",
                    "ui.v1",
                    "ui_node",
                    "Roguelike combat feedback UI",
                ),
            ],
        ),
    ])
}

fn fallback_entity(
    label: &str,
    kind: &str,
    schema: &str,
    node_id: &str,
    supplement_basis: &str,
) -> DesignEntity {
    DesignEntity {
        entity_id: String::new(),
        label: label.to_string(),
        kind: kind.to_string(),
        schema: schema.to_string(),
        status: "precise".to_string(),
        source: "ai_supplement_fallback".to_string(),
        source_selection_id: String::new(),
        node_id: node_id.to_string(),
        dependencies: vec![node_id.to_string()],
        purpose: supplement_basis.to_string(),
        inference: None,
        supplement_basis: Some(supplement_basis.to_string()),
        completed_from: None,
    }
}

fn node_by_kind(kind: &str) -> &'static str {
    match kind {
        "weapon" | "enemy" => "combat_node",
        "ability" => "ability_node",
        "room" | "scene" => "room_node",
        "character" => "character_node",
        "resource" => "resource_node",
        "ui" | "audio" => "ui_node",
        "system" => "build_node",
        "config" => "meta_node",
        _ => "design_node",
    }
}

fn kind_for_missing_node(node_id: &str) -> String {
    let text = node_id.to_lowercase();
    for (kind, tokens) in [
        ("weapon", &["weapon", "attack"][..]),
        ("enemy", &["enemy", "boss"]),
        ("ability", &["ability", "skill", "action", "input"]),
        ("room", &["room", "level", "encounter", "biome"]),
        ("resource", &["resource", "currency", "economy"]),
        ("ui", &["ui", "hud", "interface"]),
        ("audio", &["audio", "sound", "music"]),
        ("scene", &["scene", "environment", "art", "visual"]),
        ("character", &["character", "avatar", "npc"]),
        ("system", &["system", "loop", "runtime"]),
    ] {
        if tokens.iter().any(|token| text.contains(token)) {
            return kind.to_string();
        }
    }
    "config".to_string()
}

fn label_for_missing_node(node_id: &str, kind: &str) -> String {
    let base = node_id
        .strip_suffix("_decision")
        .unwrap_or(node_id)
        .replace('_', " ");
    let label = base
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{} {kind}",
        if label.is_empty() {
            "Design Node".to_string()
        } else {
            label
        }
    )
}

fn l4_decisions(selections: &[Selection]) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for selection in selections {
        if is_l5_entity_item_type(&selection.item_type) {
            continue;
        }
        let key = if selection.item_type.is_empty() {
            format!("decision_{}", result.len() + 1)
        } else {
            selection.item_type.clone()
        };
        let value = if selection.option.is_empty() {
            selection.label()
        } else {
            selection.option.clone()
        };
        if !key.is_empty() && !value.is_empty() {
            result.insert(key, value);
        }
        if result.len() >= 8 {
            break;
        }
    }
    result
}

fn known_node_ids(
    entities: &[DesignEntity],
    systems: &[SystemDefinition],
    missing_node_ids: &[String],
) -> BTreeMap<String, String> {
    let mut known = BTreeMap::from([
        ("weapon".to_string(), "combat_node".to_string()),
        ("enemy".to_string(), "combat_node".to_string()),
        ("ability".to_string(), "ability_node".to_string()),
        ("room".to_string(), "room_node".to_string()),
        ("character".to_string(), "character_node".to_string()),
        ("resource".to_string(), "resource_node".to_string()),
        ("ui".to_string(), "ui_node".to_string()),
        ("scene".to_string(), "room_node".to_string()),
        ("system".to_string(), "build_node".to_string()),
        ("config".to_string(), "meta_node".to_string()),
        ("audio".to_string(), "ui_node".to_string()),
    ]);
    for node_id in missing_node_ids {
        known
            .entry(node_id.clone())
            .or_insert_with(|| node_id.clone());
    }
    for system in systems {
        if !system.id.is_empty() {
            known.insert(
                system.id.clone(),
                non_empty_or(system.name.clone(), &system.id),
            );
        }
    }
    for entity in entities {
        if !entity.node_id.is_empty() {
            known.insert(
                entity.node_id.clone(),
                non_empty_or(entity.label.clone(), &entity.node_id),
            );
        }
    }
    known
}

fn compute_request_hash(request: &SupplementRequest, parsed: &ParsedDesignSource) -> String {
    let key_data = json!({
        "source_sha256": parsed.source_sha256,
        "genre": request.genre,
        "core_loop": sorted_strings(&request.core_loop),
        "systems": sorted_strings(&request.systems.iter().map(|system| system.name.clone()).collect::<Vec<_>>()),
        "existing_entities": sorted_strings(&request.existing_entities.iter().map(|entity| format!("{}::{}::{}", entity.kind, entity.label, entity.status)).collect::<Vec<_>>()),
        "missing_node_ids": sorted_strings(&request.missing_node_ids),
        "min_per_kind": request.min_per_kind,
    });
    let bytes = serde_json::to_vec(&key_data).unwrap_or_default();
    sha256_hex(&bytes).chars().take(16).collect()
}

fn sorted_strings(values: &[String]) -> Vec<String> {
    let mut values = values.to_vec();
    values.sort();
    values
}

fn load_supplement_cache(
    path: &Path,
    request_hash: &str,
) -> Option<(Option<Vec<DesignEntity>>, Option<String>)> {
    let payload = read_json(path, json!({}));
    if payload.get("request_hash").and_then(Value::as_str) != Some(request_hash) {
        return None;
    }
    let entities = payload
        .get("entities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| serde_json::from_value::<DesignEntity>(item.clone()).ok())
        .map(normalize_entity)
        .filter(validate_entity)
        .collect::<Vec<_>>();
    let error = payload
        .get("error")
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(((!entities.is_empty()).then_some(entities), error))
}

fn save_supplement_cache(
    path: &Path,
    request_hash: &str,
    adapter: &str,
    fallback_used: bool,
    entities: &[DesignEntity],
) -> AdmResult<()> {
    write_json(
        path,
        &json!({
            "schema_version": 1,
            "generated_at": now_iso(),
            "request_hash": request_hash,
            "adapter": adapter,
            "fallback_used": fallback_used,
            "entities": entities,
        }),
    )?;
    Ok(())
}

fn matching_supplement_index(
    approximate: &DesignEntity,
    supplemented: &[DesignEntity],
    used_indexes: &BTreeSet<usize>,
) -> Option<usize> {
    let approximate_label = approximate.label.to_lowercase();
    for (index, entity) in supplemented.iter().enumerate() {
        if used_indexes.contains(&index) || entity.kind != approximate.kind {
            continue;
        }
        let label = entity.label.to_lowercase();
        if approximate_label.is_empty()
            || label.contains(&approximate_label)
            || approximate_label.contains(&label)
        {
            return Some(index);
        }
    }
    supplemented
        .iter()
        .enumerate()
        .find(|(index, entity)| !used_indexes.contains(index) && entity.kind == approximate.kind)
        .map(|(index, _)| index)
}

fn dedupe_key(entity: &DesignEntity) -> String {
    format!(
        "{}::{}",
        entity.kind.to_lowercase(),
        entity.label.to_lowercase()
    )
}

fn detect_cycles(node_ids: &[String], edges: &[GraphEdge]) -> Vec<Vec<String>> {
    let mut graph = node_ids
        .iter()
        .map(|node_id| (node_id.clone(), Vec::<String>::new()))
        .collect::<BTreeMap<_, _>>();
    for edge in edges {
        graph
            .entry(edge.from.clone())
            .or_default()
            .push(edge.to.clone());
    }
    let mut visiting = BTreeSet::<String>::new();
    let mut visited = BTreeSet::<String>::new();
    let mut cycles = Vec::new();
    for node_id in node_ids {
        visit_cycle(
            node_id,
            &graph,
            &mut visiting,
            &mut visited,
            &mut Vec::new(),
            &mut cycles,
        );
    }
    cycles.sort();
    cycles.dedup();
    cycles
}

fn visit_cycle(
    node_id: &str,
    graph: &BTreeMap<String, Vec<String>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    path: &mut Vec<String>,
    cycles: &mut Vec<Vec<String>>,
) {
    if visiting.contains(node_id) {
        if let Some(index) = path.iter().position(|item| item == node_id) {
            cycles.push(path[index..].to_vec());
        }
        return;
    }
    if visited.contains(node_id) {
        return;
    }
    visiting.insert(node_id.to_string());
    path.push(node_id.to_string());
    for target in graph.get(node_id).into_iter().flatten() {
        visit_cycle(target, graph, visiting, visited, path, cycles);
    }
    path.pop();
    visiting.remove(node_id);
    visited.insert(node_id.to_string());
}

fn phase_for(entity: &DesignEntity) -> String {
    let text = format!(
        "{} {} {} {}",
        entity.label, entity.kind, entity.schema, entity.node_id
    )
    .to_lowercase();
    if [
        "release",
        "launch",
        "analytics",
        "telemetry",
        "release_build",
        "build_pipeline",
        "运营",
        "发布",
        "上线",
        "埋点",
        "数据分析",
    ]
    .iter()
    .any(|token| text.contains(token))
    {
        return "launch_ops".to_string();
    }
    if ["currency", "economy", "resource", "货币", "资源"]
        .iter()
        .any(|token| text.contains(token))
    {
        return "economy".to_string();
    }
    if ["progress", "upgrade", "unlock", "成长", "升级", "解锁"]
        .iter()
        .any(|token| text.contains(token))
    {
        return "progression".to_string();
    }
    if ["room", "enemy", "encounter", "content", "房间", "敌人"]
        .iter()
        .any(|token| text.contains(token))
    {
        return "content_ops".to_string();
    }
    if ["social", "guild", "friend", "社交"]
        .iter()
        .any(|token| text.contains(token))
    {
        return "social".to_string();
    }
    "core_playable".to_string()
}

fn render_frozen_game_design(
    parsed: &ParsedDesignSource,
    entity_report: &EntityCoverageReport,
    phases: &PhaseClassificationReport,
    locale: ArtifactLocale,
) -> String {
    let mut text = if locale == ArtifactLocale::ZhCn {
        format!(
            "# 已冻结游戏设计\n\n- 来源：{}\n- 实体覆盖率：{:.2}\n- 实体数量：{}\n\n## 实体\n\n",
            parsed.source, entity_report.entity_coverage_rate, entity_report.entity_count
        )
    } else {
        format!(
            "# Frozen Game Design\n\n- Source: {}\n- Entity coverage: {:.2}\n- Entity count: {}\n\n## Entities\n\n",
            parsed.source, entity_report.entity_coverage_rate, entity_report.entity_count
        )
    };
    for entity in &entity_report.entities {
        text.push_str(&format!(
            "- {} [{}] node={} schema={}\n",
            entity.label, entity.kind, entity.node_id, entity.schema
        ));
    }
    text.push_str(localized_text(locale, "\n## 阶段\n\n", "\n## Phases\n\n"));
    for (phase, entities) in &phases.phases {
        text.push_str(&format!("- {phase}: {}\n", entities.len()));
    }
    text
}

fn project_name(parsed: &ParsedDesignSource) -> String {
    for line in parsed.raw_text.lines() {
        if let Some(title) = line.trim().strip_prefix("# ") {
            let title = title
                .split(" - ")
                .next()
                .unwrap_or(title)
                .split(" — ")
                .next()
                .unwrap_or(title)
                .trim();
            if !title.is_empty() {
                return title.to_string();
            }
        }
    }
    Path::new(&parsed.source)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Unnamed Game Project")
        .to_string()
}

fn selection_text(selection: &Selection) -> String {
    [
        selection.item_type.as_str(),
        selection.option.as_str(),
        selection.purpose.as_str(),
        selection.layer_title.as_str(),
        selection.source_ref.as_str(),
    ]
    .into_iter()
    .filter(|value| !value.trim().is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

fn selection_haystack(raw_text: &str, selections: &[Selection]) -> String {
    format!(
        "{} {}",
        raw_text,
        selections
            .iter()
            .map(selection_text)
            .collect::<Vec<_>>()
            .join(" ")
    )
    .to_lowercase()
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

fn first_non_empty_field(value: &Value, fields: &[&str]) -> String {
    fields
        .iter()
        .map(|field| string_field(value, field))
        .find(|item| !item.is_empty())
        .unwrap_or_default()
}

fn structured_entity_schema(value: &Value, kind: &str) -> String {
    let schema = first_non_empty_field(value, &["schema", "schemaId", "schema_id"]);
    if !schema.is_empty() {
        return schema;
    }
    let version = first_non_empty_field(value, &["schemaVersion", "schema_version"]);
    if version.is_empty() || kind.is_empty() {
        return String::new();
    }
    match kind {
        "system" => "system_card_v1".to_string(),
        "numeric_curve" => "numeric_curve_v1".to_string(),
        "loop" => "loop_card_v1".to_string(),
        "content" => "content_set_v1".to_string(),
        "encounter" => "encounter_pattern_v1".to_string(),
        _ => format!("{kind}.{}", version.trim_start_matches('v')),
    }
}

fn optional_non_empty_field(value: &Value, field: &str) -> Option<String> {
    let text = string_field(value, field);
    (!text.is_empty()).then_some(text)
}

fn non_empty_or(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

fn first_chars(value: &str, limit: usize) -> Option<String> {
    let text = value.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.chars().take(limit).collect())
    }
}

fn ratio(left: usize, right: usize) -> f64 {
    if right == 0 {
        0.0
    } else {
        let value = left as f64 / right as f64;
        (value * 10_000.0).round() / 10_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::parse_design_text;
    use adm_new_contracts::schema::{load_structured_file, validate_contract};
    use adm_new_foundation::{new_stable_id, paths::SourceProjectRoot};
    use std::fs;

    #[test]
    fn step00_builds_concept_profile_and_core_question_coverage() {
        let parsed = sample_design();

        let profile = ConceptProcessor.build_profile(&parsed);
        let coverage = QuestionEngine::default().evaluate(&parsed);

        assert_eq!(profile.genre_key, "roguelike_action");
        assert!(profile.core_loop.label.contains("进入房间"));
        assert!(coverage.answered_questions >= 10);
        assert!(!coverage.needs_ai_supplement);
    }

    #[test]
    fn question_engine_loads_fixture_or_default_questions() {
        let root = temp_root("questions");
        let path = root.join("core_questions.json");
        write_json(
            &path,
            &json!([{
                "id": "CQ-X",
                "domain": "core",
                "question": "loop?",
                "item_types": ["核心循环"],
                "keywords": ["循环"]
            }]),
        )
        .unwrap();

        let coverage = QuestionEngine::from_path(&path).evaluate(&sample_design());

        assert_eq!(coverage.total_questions, 1);
        assert_eq!(coverage.answered_questions, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step01_prefers_explicit_loop_and_falls_back_to_template_systems() {
        let parsed = sample_design();
        let loop_report = LoopExtractor::default().extract(&parsed);
        let systems = SystemDeducer::default().deduce(&parsed, &json!({"nodes": [], "edges": []}));

        assert_eq!(loop_report.source_kind, "explicit");
        assert_eq!(loop_report.loop_nodes[0], "进入房间");
        assert_eq!(systems.template_key, "roguelike_action");
        assert!(systems.system_count >= 5);
    }

    #[test]
    fn step00_and_step01_accept_d4_english_concept_protocol() {
        let parsed = parse_design_text(
            r#"## Layer 1 Project Vision
Submitted / accepted
- Game type: lane defense strategy
  Purpose: concrete project positioning
- Platform scope: PC
## Layer 2 Core Experience
Submitted / accepted
- Core loop: collect sun -> place plants -> stop enemy waves -> earn rewards
  Purpose: primary gameplay loop
"#,
            "concept.md",
            "",
            None,
            None,
        );

        let profile = ConceptProcessor.build_profile(&parsed);
        let loop_report = LoopExtractor::default().extract(&parsed);

        assert_eq!(profile.project_positioning.confidence, "explicit");
        assert!(profile.project_positioning.label.contains("lane defense"));
        assert_eq!(profile.core_loop.confidence, "explicit");
        assert_eq!(loop_report.source_kind, "explicit");
        assert_eq!(
            loop_report.loop_nodes,
            vec![
                "collect sun",
                "place plants",
                "stop enemy waves",
                "earn rewards"
            ]
        );
    }

    #[test]
    fn step00_02_artifacts_default_to_chinese_and_support_english() {
        let parsed = sample_design();
        let root = temp_root("localized_artifacts");
        let zh00 = root.join("zh_00");
        let en00 = root.join("en_00");
        let zh01 = root.join("zh_01");
        let en01 = root.join("en_01");
        let zh02 = root.join("zh_02");
        let en02 = root.join("en_02");
        let en_inputs = json!({"artifact_locale": "en-US"});

        let zh00_result = Step00OutputGenerator::default()
            .generate(STEP00, &parsed, &zh00, &json!({}))
            .unwrap();
        let en00_result = Step00OutputGenerator::default()
            .generate(STEP00, &parsed, &en00, &en_inputs)
            .unwrap();
        let zh01_result = Step01OutputGenerator::default()
            .generate(STEP01, &parsed, &zh01, &json!({}))
            .unwrap();
        let en01_result = Step01OutputGenerator::default()
            .generate(STEP01, &parsed, &en01, &en_inputs)
            .unwrap();
        Step02OutputGenerator::new(EntityValidator::new(None))
            .generate(STEP02, &parsed, &zh02, &json!({}))
            .unwrap();
        Step02OutputGenerator::new(EntityValidator::new(None))
            .generate(STEP02, &parsed, &en02, &en_inputs)
            .unwrap();

        assert_eq!(zh00_result["artifact_locale"], json!("zh-CN"));
        assert_eq!(en00_result["artifact_locale"], json!("en-US"));
        assert!(
            fs::read_to_string(zh00.join("main_design_source.md"))
                .unwrap()
                .starts_with("# 步骤 00：创意收集")
        );
        assert!(
            fs::read_to_string(en00.join("main_design_source.md"))
                .unwrap()
                .starts_with("# Step00 Idea Intake")
        );
        assert!(
            fs::read_to_string(zh01.join("gameplay_framework.md"))
                .unwrap()
                .starts_with("# 玩法框架")
        );
        assert!(
            fs::read_to_string(en01.join("gameplay_framework.md"))
                .unwrap()
                .starts_with("# Gameplay Framework")
        );
        assert!(zh01_result["message"].as_str().unwrap().contains("步骤 01"));
        assert!(en01_result["message"].as_str().unwrap().contains("Step01"));

        let zh_contract = read_json(
            &zh02.join("playable_contracts/design_completeness_report.json"),
            json!({}),
        );
        let en_contract = read_json(
            &en02.join("playable_contracts/design_completeness_report.json"),
            json!({}),
        );
        assert_eq!(zh_contract["artifact_locale"], json!("zh-CN"));
        assert_eq!(en_contract["artifact_locale"], json!("en-US"));
        assert!(
            zh_contract["playable_completeness"]["review_items"][0]["message"]
                .as_str()
                .unwrap()
                .contains("建议进行人工确认")
        );
        assert!(
            en_contract["playable_completeness"]["review_items"][0]["message"]
                .as_str()
                .unwrap()
                .contains("human confirmation")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step00_and_step01_propagate_structured_input_review_details() {
        let parsed = sample_design();
        let root = temp_root("structured_reviews_00_01");
        let inputs = json!({
            "artifact_locale": "zh-CN",
            "status": "fallback_to_markdown",
            "warnings": [{
                "code": "STRUCTURED_INPUT_FALLBACK_USED",
                "message": "结构化输入不完整，已使用 Markdown 兼容输入。"
            }]
        });

        let result00 = Step00OutputGenerator::default()
            .generate(STEP00, &parsed, &root.join("00"), &inputs)
            .unwrap();
        let result01 = Step01OutputGenerator::default()
            .generate(STEP01, &parsed, &root.join("01"), &inputs)
            .unwrap();

        for result in [&result00, &result01] {
            assert_eq!(result["status"], json!("completed_with_review"));
            assert!(result["review_items_count"].as_u64().unwrap() > 0);
            assert_eq!(
                result["review_items"][0]["code"],
                json!("STRUCTURED_INPUT_FALLBACK_USED")
            );
            assert!(!result["warnings"].as_array().unwrap().is_empty());
            assert!(
                !result["semantic_quality"]["return_targets"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step02_prefers_structured_d4_entities_and_keeps_markdown_fallback() {
        let parsed = parse_design_text(
            r#"## 第 5 层 L5 实体
- L5 节点：markdown_node
- L5 实体：Markdown 实体
  目的：kind=ability；schema=ability.v1
  依赖：markdown_node
"#,
            "design.md",
            "",
            None,
            None,
        );
        let structured_inputs = json!({
            "artifact_locale": "zh-CN",
            "status": "structured",
            "inputs": {
                "design_entities": {
                    "source": "structured/design_entities.json",
                    "nodes": [{
                        "node_id": "structured_node",
                        "entities": [{
                            "id": "structured_system",
                            "label": "结构化系统实体",
                            "kind": "system",
                            "schemaVersion": "1.0"
                        }]
                    }]
                }
            }
        });

        let preferred = preferred_design_entities(&parsed, &structured_inputs);
        let structured_report = EntityValidator::new(None).validate_with_inputs(
            &parsed,
            &structured_inputs,
            ArtifactLocale::ZhCn,
        );
        let markdown_report = EntityValidator::new(None).validate(&parsed);

        assert_eq!(preferred.len(), 1);
        assert_eq!(preferred[0].label, "结构化系统实体");
        assert_eq!(preferred[0].schema, "system_card_v1");
        assert_eq!(structured_report.entity_count, 1);
        assert_eq!(structured_report.entities[0].node_id, "structured_node");
        assert_eq!(
            structured_report.coverage_basis,
            "structured_design_entities"
        );
        assert_eq!(structured_report.entity_coverage_rate, 1.0);
        assert_eq!(markdown_report.entity_count, 1);
        assert_eq!(markdown_report.entities[0].label, "Markdown 实体");
        assert_eq!(markdown_report.coverage_basis, "explicit_l5_nodes");
    }

    #[test]
    fn step02_localizes_generated_supplement_prose() {
        let parsed = sample_design();
        let validator = EntityValidator::new(Some(EntitySupplementAdapter::new("local_fallback")));

        let zh = validator.validate_with_inputs(&parsed, &json!({}), ArtifactLocale::ZhCn);
        let en = validator.validate_with_inputs(
            &parsed,
            &json!({"artifact_locale": "en-US"}),
            ArtifactLocale::EnUs,
        );
        let zh_generated = zh
            .entities
            .iter()
            .find(|entity| entity.source.starts_with("ai_supplement_"))
            .unwrap();
        let en_generated = en
            .entities
            .iter()
            .find(|entity| entity.source.starts_with("ai_supplement_"))
            .unwrap();

        assert!(
            zh_generated
                .supplement_basis
                .as_deref()
                .unwrap()
                .contains("补全依据")
        );
        assert!(
            en_generated
                .supplement_basis
                .as_deref()
                .unwrap()
                .contains("Roguelike")
        );
        assert!(en_generated.label.is_ascii());
        assert!(
            zh.ai_supplement.as_ref().unwrap().supplement_basis_samples[0].contains("补全依据")
        );
    }

    #[test]
    fn step02_extracts_l5_entities_and_supplements_missing_nodes() {
        let mut parsed = sample_design();
        parsed.design_summary = json!({
            "node_count": 4,
            "node_ids": ["combat_node", "ability_node", "room_node", "resource_node"]
        });
        let validator = EntityValidator::new(Some(EntitySupplementAdapter::new("local_fallback")));

        let report = validator.validate(&parsed);

        assert!(report.entity_count >= 4);
        assert_eq!(report.entity_coverage_rate, 1.0);
        assert_eq!(
            report.ai_supplement.as_ref().unwrap().trigger_reason,
            "系统实体数 0 少于 5"
        );
    }

    #[test]
    fn step02_recognizes_exported_english_l5_protocol() {
        let mut parsed = parse_design_text(
            r#"## Layer 5 L5 Entities
- L5 node: input_control_decision
  Purpose: this concrete design node contains traceable L5 entities.
- L5 entity: Input system (input_system)
  Purpose: kind=system; schema=system_card_v1
  Depends: input_control_decision
  Unlocks: program_requirements, art_requirements
- L5 entity: Input curve (input_curve)
  Purpose: kind=numeric_curve; schema=numeric_curve_v1
  Depends: input_control_decision
- L5 entity: Input loop (input_loop)
  Purpose: kind=loop; schema=loop_card_v1
  Depends: input_control_decision
- L5 node: help_support_experience_decision
  Purpose: this explicit L5 node must remain part of the coverage contract.
- L5 entity: Help system (help_system)
  Purpose: kind=system; schema=system_card_v1
  Depends: help_support_experience_decision
- L5 entity: Help curve (help_curve)
  Purpose: kind=numeric_curve; schema=numeric_curve_v1
  Depends: help_support_experience_decision
- L5 entity: Help loop (help_loop)
  Purpose: kind=loop; schema=loop_card_v1
  Depends: help_support_experience_decision
"#,
            "design.md",
            "",
            None,
            None,
        );
        parsed.design_summary = json!({
            "node_count": 103,
            "design_entity_node_count": 2,
            "design_entity_count": 6
        });

        let report = EntityValidator::new(None).validate(&parsed);

        assert_eq!(report.entity_count, 6);
        assert_eq!(report.concrete_node_count, 2);
        assert_eq!(report.covered_concrete_nodes, 2);
        assert_eq!(report.entity_coverage_rate, 1.0);
        assert_eq!(report.coverage_basis, "explicit_l5_nodes");
        assert!(report.missing_entities.is_empty());
        assert!(report.invalid_entities.is_empty());
        assert!(
            report
                .entities
                .iter()
                .all(|entity| entity.inference.is_none())
        );

        let root = temp_root("english_l5_protocol");
        let result = Step02OutputGenerator::new(EntityValidator::new(None))
            .generate(STEP02, &parsed, &root, &json!({}))
            .unwrap();
        assert_eq!(result["covered_entity_node_count"], json!(2));
        assert_eq!(result["expected_entity_node_count"], json!(2));
        assert_eq!(result["entity_coverage_rate"], json!(1.0));
        assert!(
            result["review_items"]
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["code"] != json!("L5_ENTITY_COVERAGE_BELOW_TARGET"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step02_prefers_entity_node_summary_over_legacy_node_count() {
        let mut parsed = parse_design_text(
            r#"## Layer 5 L5 Entities
- L5 entity: Combat system (combat_system)
  Purpose: kind=system; schema=system_card_v1
  Depends: combat_node
- L5 entity: Reward system (reward_system)
  Purpose: kind=system; schema=system_card_v1
  Depends: reward_node
"#,
            "design.md",
            "",
            None,
            None,
        );
        parsed.design_summary = json!({
            "node_count": 103,
            "design_entity_node_count": 2,
            "design_entity_count": 2
        });

        let report = EntityValidator::new(None).validate(&parsed);

        assert_eq!(report.concrete_node_count, 2);
        assert_eq!(report.covered_concrete_nodes, 2);
        assert_eq!(report.entity_coverage_rate, 1.0);
        assert_eq!(report.coverage_basis, "design_entity_node_count");
    }

    #[test]
    fn step02_summary_coverage_never_exceeds_one() {
        let mut parsed = parse_design_text(
            r#"## Layer 5 L5 Entities
- L5 entity: Combat system (combat_system)
  Purpose: kind=system; schema=system_card_v1
  Depends: combat_node
- L5 entity: Reward system (reward_system)
  Purpose: kind=system; schema=system_card_v1
  Depends: reward_node
"#,
            "design.md",
            "",
            None,
            None,
        );
        parsed.design_summary = json!({
            "design_entity_node_count": 1,
            "design_entity_count": 2
        });

        let report = EntityValidator::new(None).validate(&parsed);

        assert_eq!(report.concrete_node_count, 1);
        assert_eq!(report.covered_concrete_nodes, 1);
        assert_eq!(report.entity_coverage_rate, 1.0);
    }

    #[test]
    fn step02_review_status_always_has_structured_details() {
        let mut parsed = parse_design_text(
            r#"## Layer 5 L5 Entities
- L5 node: combat_node
- L5 node: reward_node
- L5 node: progression_node
- L5 entity: Combat system (combat_system)
  Purpose: kind=system; schema=system_card_v1
  Depends: combat_node
"#,
            "design.md",
            "",
            None,
            None,
        );
        parsed.design_summary = json!({
            "design_entity_node_count": 3,
            "design_entity_count": 1
        });
        let root = temp_root("structured_step02_review");

        let result = Step02OutputGenerator::new(EntityValidator::new(None))
            .generate(STEP02, &parsed, &root, &json!({}))
            .unwrap();

        assert_eq!(result["status"], json!("completed_with_review"));
        assert!(result["review_items_count"].as_u64().unwrap() > 0);
        assert_eq!(
            result["review_items"][0]["code"],
            json!("L5_ENTITY_COVERAGE_BELOW_TARGET")
        );
        assert!(!result["warnings"].as_array().unwrap().is_empty());
        assert!(
            !result["semantic_quality"]["return_targets"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn supplement_helpers_parse_merge_and_cache_entities() {
        let parsed = sample_design();
        let root = temp_root("supplement");
        let adapter = EntitySupplementAdapter::new("local_fallback").with_cache_dir(&root);
        let original = vec![DesignEntity {
            entity_id: "ENT-001".to_string(),
            label: "Approx Slash".to_string(),
            kind: "ability".to_string(),
            schema: "ability.v1".to_string(),
            status: "approximate".to_string(),
            source: "source".to_string(),
            source_selection_id: "SEL-001".to_string(),
            node_id: "ability_node".to_string(),
            dependencies: vec!["ability_node".to_string()],
            purpose: String::new(),
            inference: None,
            supplement_basis: None,
            completed_from: None,
        }];

        let parsed_entities = parse_response_entities(
            r#"model says {"supplemented_entities":[{"label":"Slash","kind":"ability","schema":"ability.v1","node_id":"ability_node"}]}"#,
        );
        let (merged, added, completed) = merge_entities(&original, &parsed_entities);
        let result = adapter.supplement(&merged, &parsed, &["weapon_node".to_string()]);
        let cached = adapter.supplement(&merged, &parsed, &["weapon_node".to_string()]);

        assert_eq!(added, 0);
        assert_eq!(completed, 1);
        assert_eq!(merged[0].completed_from.as_deref(), Some("ENT-001"));
        assert!(result.fallback_used);
        assert!(cached.cache_hit);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn response_entity_parser_accepts_exporter_entity_kinds() {
        let entities = parse_response_entities(
            r#"{"supplemented_entities":[
                {"label":"Loop","kind":"loop","schema":"loop_card_v1","node_id":"loop_node"},
                {"label":"Curve","kind":"numeric_curve","schema":"numeric_curve_v1","node_id":"curve_node"},
                {"label":"Content","kind":"content","schema":"content_set_v1","node_id":"content_node"},
                {"label":"Encounter","kind":"encounter","schema":"encounter_pattern_v1","node_id":"encounter_node"}
            ]}"#,
        );

        assert_eq!(entities.len(), 4);
        assert_eq!(
            entities
                .iter()
                .map(|entity| entity.kind.as_str())
                .collect::<Vec<_>>(),
            vec!["loop", "numeric_curve", "content", "encounter"]
        );
    }

    #[test]
    fn graph_generator_detects_cycles_and_phase_classifier_buckets_entities() {
        let parsed = sample_design();
        let report = EntityValidator::new(None).validate(&parsed);
        let graph = GraphGenerator.generate(
            &json!({
                "nodes": [
                    {"id": "A", "name": "System A"},
                    {"id": "B", "name": "System B"}
                ],
                "edges": [
                    {"from": "A", "to": "B"},
                    {"from": "B", "to": "A"}
                ]
            }),
            &report,
        );
        let phases = PhaseClassifier.classify(&report);

        assert!(!graph.cycle_free);
        assert!(!phases.phases["economy"].is_empty());
    }

    #[test]
    fn stage_generators_write_step00_02_outputs() {
        let parsed = sample_design();
        let root = temp_root("generators");
        let step00 = root.join("stage_00");
        let step01 = root.join("stage_01");
        let step02 = root.join("stage_02");

        let result00 = Step00OutputGenerator::default()
            .generate(
                STEP00,
                &parsed,
                &step00,
                &json!({"status": "fallback_to_markdown"}),
            )
            .unwrap();
        let result01 = Step01OutputGenerator::default()
            .generate(STEP01, &parsed, &step01, &json!({}))
            .unwrap();
        let result02 = Step02OutputGenerator::default()
            .generate(STEP02, &parsed, &step02, &json!({}))
            .unwrap();

        assert_eq!(result00["content_exists"], json!(true));
        assert!(step00.join("concept_profile.json").exists());
        assert!(step01.join("gameplay_framework.md").exists());
        assert!(result01["system_count"].as_u64().unwrap() >= 5);
        assert!(step02.join("frozen_game_design.md").exists());
        for contract_id in [
            "core_playable_contract",
            "demo_flow_contract",
            "runtime_data_contract",
            "ui_flow_contract",
            "scene_bootstrap_contract",
            "asset_mount_contract",
            "audio_requirements_contract",
            "audio_event_map",
            "audio_placeholder_manifest",
            "playable_acceptance_contract",
            "design_completeness_report",
        ] {
            assert!(
                step02
                    .join("playable_contracts")
                    .join(format!("{contract_id}.json"))
                    .exists(),
                "missing playable contract {contract_id}"
            );
        }
        assert_eq!(result02["playable_contract_valid"], json!(true));
        assert_eq!(result02["traceability_valid"], json!(true));
        if result02["status"] == json!("success") {
            assert_eq!(result02["review_items_count"], json!(0));
            assert!(result02["review_items"].as_array().unwrap().is_empty());
            assert!(result02["warnings"].as_array().unwrap().is_empty());
            assert!(
                result02["semantic_quality"]["return_targets"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step00_02_registry_schema_contracts_are_materialized_and_valid() {
        let root = temp_root("registry_schema_contracts");
        generate_registered_stages(&root, ArtifactLocale::ZhCn);
        let repository_root = SourceProjectRoot::discover(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
            .into_path();
        let registry =
            load_structured_file(&repository_root.join("pipeline/artifact_layer/registry.json"))
                .unwrap();
        let mut validated_paths = Vec::new();
        for stage in registry
            .get("artifacts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter(|stage| {
                stage
                    .get("stage")
                    .and_then(Value::as_u64)
                    .is_some_and(|id| id <= 2)
            })
        {
            for schema_ref in stage
                .get("schema_refs")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let relative_path = schema_ref.get("path").and_then(Value::as_str).unwrap();
                let schema_path = schema_ref.get("schema").and_then(Value::as_str).unwrap();
                let artifact_path = root.join(relative_path);
                assert!(
                    artifact_path.is_file(),
                    "registry artifact was not materialized: {relative_path}"
                );
                let artifact = load_structured_file(&artifact_path).unwrap();
                assert_eq!(
                    artifact.get("artifact_locale"),
                    Some(&json!("zh-CN")),
                    "artifact locale missing from {relative_path}"
                );
                let schema = load_structured_file(&repository_root.join(schema_path)).unwrap();
                let errors = validate_contract(&artifact, &schema);
                assert!(
                    errors.is_empty(),
                    "{relative_path} does not satisfy {schema_path}: {errors:?}"
                );
                validated_paths.push(relative_path.to_string());
            }
        }

        assert_eq!(validated_paths.len(), 23, "registry coverage changed");
        let project_dna = load_structured_file(
            &root.join("outputs/artifacts/stage_02/project_dna_contract.json"),
        )
        .unwrap();
        assert_eq!(project_dna["contract_state"], json!("frozen"));
        assert_eq!(project_dna["status"], json!("frozen"));
        assert_eq!(
            project_dna["contract_display_name"],
            json!("已冻结项目 DNA 契约")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn step00_02_registry_contracts_keep_machine_fields_stable_in_english() {
        let zh_root = temp_root("registry_zh_machine_fields");
        let en_root = temp_root("registry_en_machine_fields");
        generate_registered_stages(&zh_root, ArtifactLocale::ZhCn);
        generate_registered_stages(&en_root, ArtifactLocale::EnUs);

        let zh_archetype = load_structured_file(
            &zh_root.join("outputs/artifacts/stage_01/archetype_requirements.json"),
        )
        .unwrap();
        let en_archetype = load_structured_file(
            &en_root.join("outputs/artifacts/stage_01/archetype_requirements.json"),
        )
        .unwrap();
        let zh_seed = load_structured_file(
            &zh_root.join("outputs/artifacts/stage_02/semantic_coverage_seed.json"),
        )
        .unwrap();
        let en_seed = load_structured_file(
            &en_root.join("outputs/artifacts/stage_02/semantic_coverage_seed.json"),
        )
        .unwrap();

        assert_eq!(zh_archetype["artifact_locale"], json!("zh-CN"));
        assert_eq!(en_archetype["artifact_locale"], json!("en-US"));
        assert_eq!(
            zh_archetype["detected_archetype"],
            en_archetype["detected_archetype"]
        );
        assert_eq!(zh_seed["matrix_state"], en_seed["matrix_state"]);
        assert_eq!(
            zh_seed["coverage_items"][0]["coverage_id"],
            en_seed["coverage_items"][0]["coverage_id"]
        );
        assert_eq!(zh_seed["matrix_display_name"], json!("语义覆盖种子矩阵"));
        assert_eq!(
            en_seed["matrix_display_name"],
            json!("Semantic Coverage Seed Matrix")
        );
        let _ = fs::remove_dir_all(zh_root);
        let _ = fs::remove_dir_all(en_root);
    }

    #[test]
    fn step02_rebuilds_legacy_empty_candidates_from_nonempty_structured_decisions() {
        let empty = build_playable_contract_bundle_with_locale(
            &json!({"source": "legacy", "selections": []}),
            ArtifactLocale::ZhCn,
        );
        let mut candidates = empty.as_object().unwrap().clone();
        candidates.remove("design_completeness_report");
        candidates.remove("artifact_locale");
        let inputs = json!({
            "inputs": {
                "playable_contract_candidates": candidates,
                "decisions": {
                    "source": "structured/decisions.json",
                    "decisions": [{
                        "node_id": "combat_loop",
                        "decision_state": "completed",
                        "selected_options": [{
                            "option_id": "real_time_combat",
                            "label": "实时战斗",
                            "source_refs": ["combat_loop.core_loop.real_time_combat"]
                        }],
                        "source_refs": ["combat_loop"]
                    }]
                },
                "profile": {"project_id": "structured-project", "genre": "action_rpg"},
                "archetype_requirements": {"detected_archetype": "action_rpg"}
            }
        });

        let (bundle, mode) =
            playable_contract_bundle(&sample_design(), &inputs, ArtifactLocale::ZhCn);

        assert_eq!(mode, "structured_candidate_rebuild");
        assert_eq!(
            bundle["design_completeness_report"]["playable_completeness"]["valid"],
            true
        );
        assert_eq!(
            bundle["core_playable_contract"]["generation_mode"],
            "structured_decisions"
        );
    }

    #[test]
    fn plugin_specs_match_python_stage_wrappers() {
        let step00 = step00_plugin_spec();
        let step01 = step01_plugin_spec();
        let step02 = step02_plugin_spec();

        assert_eq!(step00.source_groups[0].label, "concept");
        assert_eq!(step00.source_groups[0].mode, "latest");
        assert_eq!(step01.source_groups[0].mode, "all");
        assert_eq!(step02.source_groups.len(), 4);
        assert_eq!(step02.source_groups[2].source_ids, vec!["Design"]);
    }

    fn sample_design() -> ParsedDesignSource {
        parse_design_text(
            r#"# Hades-like Prototype

## Layer 1 Project
Draft / Selected
- 项目定位: Hades-style roguelike action game for PC
  目的: fast run-based combat positioning
- 目标玩家: action roguelike players
- 平台: PC offline buyout
- 商业模式: buyout offline single_release

## Layer 2 Core
Selected / Stable
- 核心循环: 进入房间 -> 战斗清场 -> 选择奖励 -> 升级构筑 -> 挑战首领
  目的: loop and reward clarity
- 压力来源: 敌人组合、死亡重开、资源不足
- 奖励节奏: 清场奖励、祝福构筑、局外成长
- 玩法系统: 即时战斗系统
  依赖: combat_node
- system_layer: 房间推进系统
  依赖: room_node

## Layer 5 Entities
Frozen / Traceable
- L5实体: Shadow Sword
  目的: schema=weapon.v1；kind=weapon；status=precise
  依赖: combat_node
- L5实体: Dash Slash
  目的: schema=ability.v1；kind=ability；status=precise
  依赖: ability_node
- L5实体: Shadow Coin
  目的: schema=resource.v1；kind=resource；status=precise
  依赖: resource_node
"#,
            "sample.md",
            "",
            None,
            None,
        )
    }

    fn generate_registered_stages(root: &Path, locale: ArtifactLocale) {
        let parsed = sample_design();
        let artifacts = root.join("outputs/artifacts");
        let inputs = json!({
            "artifact_locale": locale,
            "status": "structured",
            "inputs": {},
            "warnings": [],
        });
        Step00OutputGenerator::default()
            .generate(STEP00, &parsed, &artifacts.join("stage_00"), &inputs)
            .unwrap();
        Step01OutputGenerator::default()
            .generate(STEP01, &parsed, &artifacts.join("stage_01"), &inputs)
            .unwrap();
        Step02OutputGenerator::new(EntityValidator::new(None))
            .generate(STEP02, &parsed, &artifacts.join("stage_02"), &inputs)
            .unwrap();
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir()
            .join("adm_new_pipeline_step00_02")
            .join(label)
            .join(new_stable_id("root").unwrap())
    }
}
