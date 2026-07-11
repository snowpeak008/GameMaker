use adm_new_foundation::{AdmError, AdmResult, paths::relative_display};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

mod entity_schema;
mod normalization;
mod project_templates;
mod validation;

pub use entity_schema::{EntitySchemaRegistry, EntityValidationWarning};
pub use validation::{SharedOptionGroupReuse, TemplateReuseReport};

pub const DESIGN_DATA_ROOT: &str = "knowledge/design_data";
pub const DEFAULT_PROGRAM_ID: &str = "commercial_game_design_decision_tool";
pub const DEFAULT_PROGRAM_NAME: &str = "完整商业游戏设计决策工具";
pub const DEFAULT_PROGRAM_DESCRIPTION: &str = "全领域游戏设计决策、节点补全和框架补全工作台。";
pub const TEMPLATE_INDEX_FILE: &str = "template_index.json";
pub const DEFAULT_TEMPLATE_SCHEMA_VERSION: &str = "0.1.0";
pub const DEFAULT_DOMAIN_SCHEMA_VERSION: &str = "0.1.0";
pub const DEFAULT_ROLE_CLASS: &str = "meta_planning";
pub const ROLE_CLASS_VALUES: [&str; 3] = ["meta_planning", "system_concrete", "content_concrete"];
pub const OPTION_RELATION_TYPES: [&str; 2] = ["soft_conflict", "hard_exclusive"];
pub const SCALE_ORDER: [&str; 5] = ["iaa_hypercasual", "indie", "midcore", "3a", "large_service"];
pub const MDA_LAYER_LABELS: [(&str, &str); 5] = [
    ("aesthetics", "体验目标"),
    ("dynamics", "玩家动态"),
    ("mechanics", "机制抓手"),
    ("constraints", "边界约束"),
    ("evidence", "验收信号"),
];

#[derive(Debug, Clone)]
pub struct DesignDataLoader {
    project_root: PathBuf,
    runtime_root: PathBuf,
    design_data_dir: PathBuf,
}

impl DesignDataLoader {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        let project_root = project_root.into();
        let runtime_root = project_root.join("drafts");
        let design_data_dir = project_root.join(DESIGN_DATA_ROOT);
        Self {
            project_root,
            runtime_root,
            design_data_dir,
        }
    }

    pub fn from_design_data_dir(
        project_root: impl Into<PathBuf>,
        design_data_dir: impl Into<PathBuf>,
    ) -> Self {
        let project_root = project_root.into();
        let runtime_root = project_root.join("drafts");
        Self {
            project_root,
            runtime_root,
            design_data_dir: design_data_dir.into(),
        }
    }

    pub fn with_runtime_root(mut self, runtime_root: impl Into<PathBuf>) -> Self {
        self.runtime_root = runtime_root.into();
        self
    }

    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    pub fn runtime_root(&self) -> &Path {
        &self.runtime_root
    }

    pub fn design_data_dir(&self) -> &Path {
        &self.design_data_dir
    }

    pub fn domains_dir(&self) -> PathBuf {
        self.design_data_dir.join("domains")
    }

    pub fn shared_templates_dir(&self) -> PathBuf {
        self.design_data_dir.join("templates")
    }

    pub fn entity_schemas_dir(&self) -> PathBuf {
        self.design_data_dir.join("entity_schemas")
    }

    pub fn archetypes_dir(&self) -> PathBuf {
        self.design_data_dir.join("archetypes")
    }

    pub fn prompt_framework_dir(&self) -> PathBuf {
        self.design_data_dir.join("prompt_framework")
    }

    pub fn project_templates_dir(&self) -> PathBuf {
        self.design_data_dir.join("project_templates")
    }

    pub fn custom_project_templates_dir(&self) -> PathBuf {
        self.runtime_root
            .join("workspace")
            .join("projects")
            .join("templates")
    }

    pub fn gameplay_system_options_path(&self) -> PathBuf {
        self.design_data_dir.join("gameplay_system_options.json")
    }

    pub fn load_project_data(&self) -> AdmResult<DesignProjectData> {
        let domains = self.load_domains()?;
        let gameplay_system_options = self.load_gameplay_system_options()?;
        let mut validation_errors = validation::validate_domains(&domains);
        validation_errors.extend(validation::validate_gameplay_system_options(
            &gameplay_system_options,
        ));
        let entity_validation_warnings = domains
            .iter()
            .flat_map(|domain| domain.entity_validation_warnings.clone())
            .collect::<Vec<_>>();
        let template_warnings = domains
            .iter()
            .flat_map(|domain| domain.template_warnings.clone())
            .collect::<Vec<_>>();
        let role_class_warnings = domains
            .iter()
            .flat_map(|domain| domain.role_class_warnings.clone())
            .collect::<Vec<_>>();
        let mut validation_warnings = role_class_warnings.clone();
        validation_warnings.extend(entity_validation_warnings.clone());
        validation_warnings.extend(template_warnings.clone());

        Ok(DesignProjectData {
            program: ProgramMetadata {
                id: DEFAULT_PROGRAM_ID.to_string(),
                name: DEFAULT_PROGRAM_NAME.to_string(),
                description: DEFAULT_PROGRAM_DESCRIPTION.to_string(),
            },
            domains: domains.clone(),
            gameplay_system_options,
            meta: DesignProjectMeta {
                validation_errors,
                validation_warnings,
                entity_validation_warnings,
                template_warnings,
                template_reuse: validation::scan_template_reuse(&domains),
                role_class_counts: validation::count_role_classes(&domains),
                runtime_root: self.runtime_root.display().to_string(),
                data_source: self.design_data_dir.display().to_string(),
            },
        })
    }

    pub fn load_domains(&self) -> AdmResult<Vec<DomainDocument>> {
        let mut paths = collect_json_files(&self.domains_dir(), false)?;
        paths.sort();
        let shared_templates = self.shared_templates_by_id()?;
        let entity_registry = EntitySchemaRegistry::load(&self.entity_schemas_dir())?;
        let mut domains = Vec::new();
        for path in paths {
            let value = load_json_value(&path)?;
            let normalized =
                normalization::normalize_domain(value, &shared_templates, &entity_registry);
            domains.push(DomainDocument::from_value(
                normalized,
                relative_display(&path, &self.project_root),
            ));
        }
        let order = self.load_domain_order()?;
        if !order.is_empty() {
            let rank = order
                .into_iter()
                .enumerate()
                .map(|(index, id)| (id, index))
                .collect::<BTreeMap<_, _>>();
            domains
                .sort_by_key(|domain| rank.get(&domain.domain.id).copied().unwrap_or(usize::MAX));
        }
        Ok(domains)
    }

    pub fn load_domain_order(&self) -> AdmResult<Vec<String>> {
        let path = self.design_data_dir.join("domain_order.json");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let payload = load_json_value(&path)?;
        Ok(payload
            .get("domainOrder")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default())
    }

    pub fn load_shared_templates(&self) -> AdmResult<Vec<SharedTemplate>> {
        let mut paths = collect_json_files(&self.shared_templates_dir(), false)?;
        paths.sort();
        let mut templates = Vec::new();
        for path in paths {
            let mut raw = load_json_value(&path)?;
            let id = raw
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| file_stem(&path));
            if let Some(object) = raw.as_object_mut() {
                object
                    .entry("id")
                    .or_insert_with(|| Value::String(id.clone()));
            }
            templates.push(SharedTemplate::from_value(
                raw,
                relative_display(&path, &self.project_root),
            ));
        }
        Ok(templates)
    }

    pub fn load_gameplay_system_options(&self) -> AdmResult<Vec<GameplaySystemOption>> {
        let path = self.gameplay_system_options_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let payload = load_json_value(&path)?;
        let mut seen = BTreeSet::new();
        let mut options = Vec::new();
        for value in payload
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(object) = value.as_object() else {
                continue;
            };
            let option_id = string_from_value(object.get("id")).trim().to_string();
            if option_id.is_empty() || !seen.insert(option_id.clone()) {
                continue;
            }
            let name = string_from_value(object.get("name"));
            let category = string_from_value(object.get("category"));
            options.push(GameplaySystemOption {
                id: option_id.clone(),
                name: if name.trim().is_empty() {
                    option_id
                } else {
                    name.trim().to_string()
                },
                category: if category.trim().is_empty() {
                    "preset".to_string()
                } else {
                    category.trim().to_string()
                },
                mapping_desc: string_from_value(
                    object
                        .get("mapping_desc")
                        .or_else(|| object.get("mappingDesc")),
                )
                .trim()
                .to_string(),
            });
        }
        Ok(options)
    }

    pub fn load_archetype_index(&self) -> AdmResult<ArchetypeIndex> {
        let path = self.archetypes_dir().join("archetype_index.json");
        let payload = load_json_value(&path)?;
        Ok(ArchetypeIndex {
            schema_version: string_field(&payload, "schema_version"),
            archetypes: payload
                .get("archetypes")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(ArchetypeEntry::from_value)
                        .collect()
                })
                .unwrap_or_default(),
            raw: payload,
        })
    }

    pub fn load_prompt_framework_manifest(&self) -> AdmResult<PromptFrameworkManifest> {
        let path = self.prompt_framework_dir().join("manifest.json");
        let payload = load_json_value(&path)?;
        Ok(PromptFrameworkManifest {
            schema_version: string_field(&payload, "schemaVersion"),
            framework_version: string_field(&payload, "frameworkVersion"),
            module_order: string_array_field(&payload, "moduleOrder"),
            modules: payload
                .get("modules")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(PromptFrameworkModule::from_value)
                        .collect()
                })
                .unwrap_or_default(),
            raw: payload,
        })
    }

    pub fn load_prompt_modules(&self) -> AdmResult<Vec<PromptModuleDocument>> {
        let modules_dir = self.prompt_framework_dir().join("modules");
        let mut paths = collect_json_files(&modules_dir, false)?;
        paths.sort();
        let mut modules = Vec::new();
        for path in paths {
            let raw = load_json_value(&path)?;
            modules.push(PromptModuleDocument {
                module_id: file_stem(&path),
                relative_path: relative_display(&path, &self.project_root),
                raw,
            });
        }
        Ok(modules)
    }

    pub fn load_project_templates(
        &self,
        include_internal: bool,
    ) -> AdmResult<Vec<ProjectTemplatePayload>> {
        project_templates::list_project_templates(self, include_internal)
    }

    pub fn load_project_templates_report(
        &self,
        include_internal: bool,
    ) -> AdmResult<ProjectTemplateLoadReport> {
        project_templates::list_project_templates_report(self, include_internal)
    }

    pub fn find_project_template(&self, template_id: &str) -> AdmResult<ProjectTemplatePayload> {
        project_templates::find_project_template(self, template_id)
    }

    pub fn save_custom_project_template(
        &self,
        template_name: &str,
        target_scale: &str,
        project_state: Value,
        overwrite: bool,
    ) -> AdmResult<ProjectTemplateWriteResult> {
        project_templates::save_custom_project_template(
            self,
            template_name,
            target_scale,
            project_state,
            overwrite,
        )
    }

    pub fn delete_custom_project_template(
        &self,
        template_id: &str,
    ) -> AdmResult<ProjectTemplateDeleteResult> {
        project_templates::delete_custom_project_template(self, template_id)
    }

    pub fn template_registry_report(&self) -> AdmResult<TemplateRegistryReport> {
        let active_templates = self.load_project_templates(false)?;
        let all_templates = self.load_project_templates(true)?;
        let mut active_ids = Vec::new();
        let mut seen = BTreeSet::new();
        let mut duplicate_ids = Vec::new();
        let mut scale_counts = BTreeMap::new();
        let mut public_count = 0;
        let mut internal_count = 0;
        for template in &active_templates {
            let id = template.meta.id.clone();
            if !seen.insert(id.clone()) {
                duplicate_ids.push(id.clone());
            }
            active_ids.push(id);
            *scale_counts
                .entry(template.meta.target_scale.clone())
                .or_insert(0) += 1;
            if template.meta.visibility == "internal" {
                internal_count += 1;
            } else {
                public_count += 1;
            }
        }
        let index = self.load_template_index().unwrap_or_else(|_| json!({}));
        let mut missing_index_files = Vec::new();
        for file_name in index
            .get("templates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| item.get("fileName").and_then(Value::as_str))
        {
            if !self.project_templates_dir().join(file_name).exists() {
                missing_index_files.push(file_name.to_string());
            }
        }

        Ok(TemplateRegistryReport {
            active_template_count: active_templates.len(),
            all_visible_or_internal_template_count: all_templates.len(),
            active_template_json_count: collect_json_files(&self.project_templates_dir(), false)?
                .len(),
            archived_template_json_count: self.archived_project_template_json_count()?,
            shared_template_count: self.load_shared_templates()?.len(),
            public_template_count: public_count,
            internal_template_count: internal_count,
            scale_counts,
            active_template_ids: active_ids,
            duplicate_template_ids: duplicate_ids,
            missing_index_files,
        })
    }

    pub fn asset_inventory(&self) -> AdmResult<DataAssetInventory> {
        let archetype_index = self.load_archetype_index()?;
        let prompt_manifest = self.load_prompt_framework_manifest()?;
        Ok(DataAssetInventory {
            design_data_root: relative_display(&self.design_data_dir, &self.project_root),
            domain_count: collect_json_files(&self.domains_dir(), false)?.len(),
            domain_order_count: self.load_domain_order()?.len(),
            shared_template_count: self.load_shared_templates()?.len(),
            entity_schema_count: collect_json_files(&self.entity_schemas_dir(), false)?.len(),
            archetype_count: archetype_index.archetypes.len(),
            prompt_framework_module_count: prompt_manifest.modules.len(),
            prompt_module_file_count: self.load_prompt_modules()?.len(),
            prompt_evaluation_json_count: collect_json_files(
                &self.design_data_dir.join("prompt_evaluation"),
                true,
            )?
            .len(),
            active_project_template_json_count: collect_json_files(
                &self.project_templates_dir(),
                false,
            )?
            .len(),
            active_project_template_count: self.load_project_templates(false)?.len(),
            archived_project_template_json_count: self.archived_project_template_json_count()?,
            gameplay_system_option_count: self.load_gameplay_system_options()?.len(),
            option_mapping_json_present: self.design_data_dir.join("option_mapping.json").exists(),
            option_mapping_markdown_present: self
                .design_data_dir
                .join("option_mapping.md")
                .exists(),
            roleclass_review_present: self.design_data_dir.join("roleclass_review.csv").exists(),
            cross_layer_rules_present: self.design_data_dir.join("cross_layer_rules.json").exists(),
            framework_memory_file_count: count_files(
                &self.design_data_dir.join("framework_memory"),
            )?,
        })
    }

    fn shared_templates_by_id(&self) -> AdmResult<BTreeMap<String, SharedTemplate>> {
        Ok(self
            .load_shared_templates()?
            .into_iter()
            .map(|template| (template.id.clone(), template))
            .collect())
    }

    fn load_template_index(&self) -> AdmResult<Value> {
        load_json_value(&self.project_templates_dir().join(TEMPLATE_INDEX_FILE))
    }

    fn archived_project_template_json_count(&self) -> AdmResult<usize> {
        let mut count = 0;
        if !self.project_templates_dir().exists() {
            return Ok(0);
        }
        for entry in std::fs::read_dir(self.project_templates_dir())? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir()
                && entry.file_name().to_string_lossy().starts_with("_archived")
            {
                count += collect_json_files(&path, true)?.len();
            }
        }
        Ok(count)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignProjectData {
    pub program: ProgramMetadata,
    pub domains: Vec<DomainDocument>,
    pub gameplay_system_options: Vec<GameplaySystemOption>,
    pub meta: DesignProjectMeta,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesignProjectMeta {
    pub validation_errors: Vec<String>,
    pub validation_warnings: Vec<String>,
    pub entity_validation_warnings: Vec<String>,
    pub template_warnings: Vec<String>,
    pub template_reuse: TemplateReuseReport,
    pub role_class_counts: BTreeMap<String, usize>,
    pub runtime_root: String,
    pub data_source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainDocument {
    pub relative_path: String,
    pub schema_version: String,
    pub domain: DomainMetadata,
    pub nodes: Vec<DomainNode>,
    pub coverage_standard: CoverageStandard,
    pub role_class_warnings: Vec<String>,
    pub entity_validation_warnings: Vec<String>,
    pub template_warnings: Vec<String>,
    pub raw: Value,
}

impl DomainDocument {
    fn from_value(raw: Value, relative_path: String) -> Self {
        let domain = DomainMetadata::from_value(raw.get("domain").unwrap_or(&Value::Null));
        let nodes = raw
            .get("nodes")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(DomainNode::from_value).collect())
            .unwrap_or_default();
        Self {
            relative_path,
            schema_version: string_field(&raw, "schemaVersion"),
            domain,
            nodes,
            coverage_standard: CoverageStandard::from_value(
                raw.get("coverageStandard").unwrap_or(&Value::Null),
            ),
            role_class_warnings: string_array_field(&raw, "_roleClassWarnings"),
            entity_validation_warnings: string_array_field(&raw, "_entityValidationWarnings"),
            template_warnings: string_array_field(&raw, "_templateWarnings"),
            raw,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub priority: String,
    pub activation: String,
}

impl DomainMetadata {
    fn from_value(value: &Value) -> Self {
        Self {
            id: string_field(value, "id"),
            name: string_field(value, "name"),
            description: string_field(value, "description"),
            priority: string_field_or(value, "priority", "P0"),
            activation: string_field_or(value, "activation", "always"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainNode {
    pub id: String,
    pub domain: String,
    pub name: String,
    pub description: String,
    pub role_class: String,
    pub requires: Vec<String>,
    pub unlocks: Vec<String>,
    pub recommended_before: Vec<String>,
    pub requires_any: Vec<String>,
    pub conflicts_with: Vec<String>,
    pub checklist: Vec<ChecklistItem>,
    pub design_entities: Vec<Value>,
    pub entity_validation_errors: Vec<EntityValidationWarning>,
    pub contract_targets: Vec<String>,
    pub consumed_by_steps: Vec<String>,
    pub contract_fields: Vec<String>,
    pub priority: String,
    pub requirement_level: String,
    pub required_for_archetypes: Vec<String>,
    pub optional_for_archetypes: Vec<String>,
    pub not_applicable_allowed: bool,
    pub not_applicable_requires_reason: bool,
}

impl DomainNode {
    fn from_value(value: &Value) -> Self {
        Self {
            id: string_field(value, "id"),
            domain: string_field(value, "domain"),
            name: string_field(value, "name"),
            description: string_field(value, "description"),
            role_class: string_field_or(value, "roleClass", DEFAULT_ROLE_CLASS),
            requires: string_array_field(value, "requires"),
            unlocks: string_array_field(value, "unlocks"),
            recommended_before: string_array_field(value, "recommendedBefore"),
            requires_any: string_array_field(value, "requiresAny"),
            conflicts_with: string_array_field(value, "conflictsWith"),
            checklist: value
                .get("checklist")
                .and_then(Value::as_array)
                .map(|items| items.iter().map(ChecklistItem::from_value).collect())
                .unwrap_or_default(),
            design_entities: value
                .get("designEntities")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            entity_validation_errors: value
                .get("entityValidationErrors")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| serde_json::from_value(item.clone()).ok())
                        .collect()
                })
                .unwrap_or_default(),
            contract_targets: string_array_field(value, "contract_targets")
                .into_iter()
                .chain(string_array_field(value, "contractTargets"))
                .collect(),
            consumed_by_steps: string_array_field(value, "consumed_by_steps")
                .into_iter()
                .chain(string_array_field(value, "consumedBySteps"))
                .collect(),
            contract_fields: string_array_field(value, "contract_fields")
                .into_iter()
                .chain(string_array_field(value, "contractFields"))
                .collect(),
            priority: string_field(value, "priority"),
            requirement_level: string_field(value, "requirement_level"),
            required_for_archetypes: string_array_field(value, "required_for_archetypes"),
            optional_for_archetypes: string_array_field(value, "optional_for_archetypes"),
            not_applicable_allowed: value
                .get("not_applicable_allowed")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            not_applicable_requires_reason: value
                .get("not_applicable_requires_reason")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: String,
    pub label: String,
    pub description: String,
    pub output_key: String,
    pub legacy_ids: Vec<String>,
    pub template_ref: String,
    pub option_groups: Vec<OptionGroup>,
    pub option_relations: Vec<OptionRelation>,
}

impl ChecklistItem {
    fn from_value(value: &Value) -> Self {
        Self {
            id: string_field(value, "id"),
            label: string_field(value, "label"),
            description: string_field(value, "description"),
            output_key: string_field(value, "outputKey"),
            legacy_ids: string_array_field(value, "legacyIds"),
            template_ref: string_field(value, "templateRef"),
            option_groups: value
                .get("optionGroups")
                .and_then(Value::as_array)
                .map(|items| items.iter().map(OptionGroup::from_value).collect())
                .unwrap_or_default(),
            option_relations: value
                .get("optionRelations")
                .and_then(Value::as_array)
                .map(|items| items.iter().map(OptionRelation::from_value).collect())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionGroup {
    pub id: String,
    pub label: String,
    pub description: String,
    pub output_key: String,
    pub selection_mode: String,
    pub required: bool,
    pub allow_primary: bool,
    pub mda_layer: String,
    pub mda_layer_label: String,
    pub progression_step: i64,
    pub relation: String,
    pub design_question: String,
    pub options: Vec<OptionItem>,
}

impl OptionGroup {
    fn from_value(value: &Value) -> Self {
        Self {
            id: string_field(value, "id"),
            label: string_field(value, "label"),
            description: string_field(value, "description"),
            output_key: string_field(value, "outputKey"),
            selection_mode: string_field_or(value, "selectionMode", "multi"),
            required: value
                .get("required")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            allow_primary: value
                .get("allowPrimary")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            mda_layer: string_field(value, "mdaLayer"),
            mda_layer_label: string_field(value, "mdaLayerLabel"),
            progression_step: value
                .get("progressionStep")
                .and_then(Value::as_i64)
                .unwrap_or(0),
            relation: string_field(value, "relation"),
            design_question: string_field(value, "designQuestion"),
            options: value
                .get("options")
                .and_then(Value::as_array)
                .map(|items| items.iter().map(OptionItem::from_value).collect())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionItem {
    pub id: String,
    pub label: String,
    pub description: String,
    pub output_key: String,
}

impl OptionItem {
    fn from_value(value: &Value) -> Self {
        Self {
            id: string_field(value, "id"),
            label: string_field(value, "label"),
            description: string_field(value, "description"),
            output_key: string_field(value, "outputKey"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionRelation {
    pub id: String,
    pub relation_type: String,
    pub source: OptionRef,
    pub targets: Vec<OptionRef>,
    pub reason: String,
    pub severity: String,
}

impl OptionRelation {
    fn from_value(value: &Value) -> Self {
        Self {
            id: string_field(value, "id"),
            relation_type: string_field(value, "type"),
            source: OptionRef::from_value(value.get("source").unwrap_or(&Value::Null)),
            targets: value
                .get("targets")
                .and_then(Value::as_array)
                .map(|items| items.iter().map(OptionRef::from_value).collect())
                .unwrap_or_default(),
            reason: string_field(value, "reason"),
            severity: string_field_or(value, "severity", "warning"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionRef {
    pub group_id: String,
    pub option_id: String,
}

impl OptionRef {
    fn from_value(value: &Value) -> Self {
        Self {
            group_id: string_field(value, "groupId"),
            option_id: string_field(value, "optionId"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageStandard {
    pub domain: String,
    pub unit: String,
    pub required_items: Vec<String>,
    pub expected: usize,
    pub formula: String,
}

impl CoverageStandard {
    fn from_value(value: &Value) -> Self {
        Self {
            domain: string_field(value, "domain"),
            unit: string_field_or(value, "unit", "nodes_and_checklist"),
            required_items: string_array_field(value, "requiredItems"),
            expected: value.get("expected").and_then(Value::as_u64).unwrap_or(0) as usize,
            formula: string_field_or(
                value,
                "formula",
                "completed_or_partial_nodes / applicable_required_items",
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameplaySystemOption {
    pub id: String,
    pub name: String,
    pub category: String,
    pub mapping_desc: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SharedTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub option_groups: Vec<OptionGroup>,
    pub option_relations: Vec<OptionRelation>,
    pub relative_path: String,
    pub raw: Value,
}

impl SharedTemplate {
    fn from_value(raw: Value, relative_path: String) -> Self {
        Self {
            id: string_field(&raw, "id"),
            name: string_field(&raw, "name"),
            description: string_field(&raw, "description"),
            option_groups: raw
                .get("optionGroups")
                .and_then(Value::as_array)
                .map(|items| items.iter().map(OptionGroup::from_value).collect())
                .unwrap_or_default(),
            option_relations: raw
                .get("optionRelations")
                .and_then(Value::as_array)
                .map(|items| items.iter().map(OptionRelation::from_value).collect())
                .unwrap_or_default(),
            relative_path,
            raw,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectTemplatePayload {
    pub schema_version: String,
    pub meta: ProjectTemplateMeta,
    pub project_state: Value,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectTemplateLoadReport {
    pub templates: Vec<ProjectTemplatePayload>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectTemplateWriteResult {
    pub template: ProjectTemplatePayload,
    pub overwritten: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectTemplateDeleteResult {
    pub template_id: String,
    pub template_name: String,
    pub target_scale: String,
    pub file_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectTemplateMeta {
    pub id: String,
    pub source: String,
    pub source_label: String,
    pub name: String,
    pub game_name: String,
    pub target_scale: String,
    pub scale_label: String,
    pub quality_tier: String,
    pub summary: String,
    pub visibility: String,
    pub file_name: String,
    pub path: String,
    pub order: Option<i64>,
    pub analysis: Vec<Value>,
    pub verification: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArchetypeIndex {
    pub schema_version: String,
    pub archetypes: Vec<ArchetypeEntry>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchetypeEntry {
    pub archetype_id: String,
    pub file: String,
    pub parent_archetypes: Vec<String>,
}

impl ArchetypeEntry {
    fn from_value(value: &Value) -> Option<Self> {
        if !value.is_object() {
            return None;
        }
        Some(Self {
            archetype_id: string_field(value, "archetype_id"),
            file: string_field(value, "file"),
            parent_archetypes: string_array_field(value, "parent_archetypes"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptFrameworkManifest {
    pub schema_version: String,
    pub framework_version: String,
    pub module_order: Vec<String>,
    pub modules: Vec<PromptFrameworkModule>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptFrameworkModule {
    pub module_id: String,
    pub path: String,
    pub module_version: String,
    pub hash: String,
    pub dependencies: Vec<String>,
}

impl PromptFrameworkModule {
    fn from_value(value: &Value) -> Option<Self> {
        if !value.is_object() {
            return None;
        }
        Some(Self {
            module_id: string_field(value, "moduleId"),
            path: string_field(value, "path"),
            module_version: string_field(value, "moduleVersion"),
            hash: string_field(value, "hash"),
            dependencies: string_array_field(value, "dependencies"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptModuleDocument {
    pub module_id: String,
    pub relative_path: String,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataAssetInventory {
    pub design_data_root: String,
    pub domain_count: usize,
    pub domain_order_count: usize,
    pub shared_template_count: usize,
    pub entity_schema_count: usize,
    pub archetype_count: usize,
    pub prompt_framework_module_count: usize,
    pub prompt_module_file_count: usize,
    pub prompt_evaluation_json_count: usize,
    pub active_project_template_json_count: usize,
    pub active_project_template_count: usize,
    pub archived_project_template_json_count: usize,
    pub gameplay_system_option_count: usize,
    pub option_mapping_json_present: bool,
    pub option_mapping_markdown_present: bool,
    pub roleclass_review_present: bool,
    pub cross_layer_rules_present: bool,
    pub framework_memory_file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateRegistryReport {
    pub active_template_count: usize,
    pub all_visible_or_internal_template_count: usize,
    pub active_template_json_count: usize,
    pub archived_template_json_count: usize,
    pub shared_template_count: usize,
    pub public_template_count: usize,
    pub internal_template_count: usize,
    pub scale_counts: BTreeMap<String, usize>,
    pub active_template_ids: Vec<String>,
    pub duplicate_template_ids: Vec<String>,
    pub missing_index_files: Vec<String>,
}

pub(crate) fn load_json_value(path: &Path) -> AdmResult<Value> {
    let text = std::fs::read_to_string(path)?;
    serde_json::from_str(&text)
        .map_err(|error| AdmError::new(format!("failed to parse json {}: {error}", path.display())))
}

pub(crate) fn collect_json_files(dir: &Path, recursive: bool) -> AdmResult<Vec<PathBuf>> {
    let mut paths = Vec::new();
    if !dir.exists() {
        return Ok(paths);
    }
    collect_json_files_inner(dir, recursive, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_json_files_inner(
    dir: &Path,
    recursive: bool,
    paths: &mut Vec<PathBuf>,
) -> AdmResult<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() && recursive {
            collect_json_files_inner(&path, recursive, paths)?;
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("json")
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn count_files(dir: &Path) -> AdmResult<usize> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in std::fs::read_dir(dir)? {
        if entry?.file_type()?.is_file() {
            count += 1;
        }
    }
    Ok(count)
}

pub(crate) fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn string_field(value: &Value, field: &str) -> String {
    value
        .get(field)
        .map(|value| string_from_value(Some(value)))
        .unwrap_or_default()
}

pub(crate) fn string_field_or(value: &Value, field: &str, fallback: &str) -> String {
    let value = string_field(value, field);
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

pub(crate) fn string_array_field(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| string_from_value(Some(item)))
                .filter(|item| !item.trim().is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn string_from_value(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.to_string(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

pub(crate) fn mda_layer_label(layer: &str) -> String {
    MDA_LAYER_LABELS
        .iter()
        .find_map(|(key, label)| (*key == layer).then(|| (*label).to_string()))
        .unwrap_or_default()
}

pub(crate) fn valid_mda_layer(layer: &str) -> bool {
    MDA_LAYER_LABELS.iter().any(|(key, _)| *key == layer)
}

pub(crate) fn valid_relation_type(relation_type: &str) -> bool {
    OPTION_RELATION_TYPES.contains(&relation_type)
}

pub(crate) fn scale_label(value: &str) -> String {
    match value {
        "iaa_hypercasual" => "IAA 超休闲小游戏",
        "indie" => "独立游戏",
        "midcore" => "中度商业游戏",
        "3a" => "3A / 高制作规格游戏",
        "large_service" => "大型长线服务游戏",
        other => other,
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm_new_foundation::paths::locate_project_root;

    fn loader() -> DesignDataLoader {
        let root = locate_project_root(env!("CARGO_MANIFEST_DIR")).unwrap();
        DesignDataLoader::new(root)
    }

    #[test]
    fn data_asset_inventory_v3_counts_real_design_data() {
        let inventory = loader().asset_inventory().unwrap();

        assert_eq!(inventory.domain_count, 16);
        assert_eq!(inventory.domain_order_count, 16);
        assert_eq!(inventory.shared_template_count, 16);
        assert_eq!(inventory.entity_schema_count, 7);
        assert_eq!(inventory.archetype_count, 2);
        assert_eq!(inventory.prompt_framework_module_count, 9);
        assert_eq!(inventory.prompt_module_file_count, 9);
        assert_eq!(inventory.active_project_template_json_count, 26);
        assert_eq!(inventory.active_project_template_count, 25);
        assert_eq!(inventory.archived_project_template_json_count, 53);
        assert_eq!(inventory.gameplay_system_option_count, 12);
        assert!(inventory.option_mapping_json_present);
        assert!(inventory.option_mapping_markdown_present);
        assert!(inventory.cross_layer_rules_present);
        assert!(inventory.roleclass_review_present);
        assert!(inventory.framework_memory_file_count >= 2);
    }

    #[test]
    fn real_domain_loader_normalizes_templates_and_validation_meta() {
        let project_data = loader().load_project_data().unwrap();

        assert_eq!(project_data.domains.len(), 16);
        assert_eq!(
            project_data.domains.first().unwrap().domain.id,
            "product_positioning_design"
        );
        assert_eq!(
            project_data.domains.last().unwrap().domain.id,
            "launch_readiness_design"
        );
        assert_eq!(project_data.gameplay_system_options.len(), 12);
        assert!(
            project_data.meta.validation_errors.is_empty(),
            "validation errors: {:?}",
            project_data.meta.validation_errors
        );
        assert!(
            project_data.meta.template_warnings.is_empty(),
            "template warnings: {:?}",
            project_data.meta.template_warnings
        );
        for role_class in ROLE_CLASS_VALUES {
            assert!(project_data.meta.role_class_counts.contains_key(role_class));
        }
        let templated_item = project_data
            .domains
            .iter()
            .flat_map(|domain| &domain.nodes)
            .flat_map(|node| &node.checklist)
            .find(|item| !item.template_ref.is_empty())
            .expect("expected at least one shared templateRef");
        assert!(!templated_item.option_groups.is_empty());
        assert!(
            templated_item
                .option_groups
                .iter()
                .all(|group| !group.output_key.is_empty())
        );
    }

    #[test]
    fn template_registry_v3_skips_index_and_archives() {
        let report = loader().template_registry_report().unwrap();

        assert_eq!(report.active_template_json_count, 26);
        assert_eq!(report.active_template_count, 25);
        assert_eq!(report.archived_template_json_count, 53);
        assert_eq!(report.shared_template_count, 16);
        assert!(report.duplicate_template_ids.is_empty());
        assert!(report.missing_index_files.is_empty());
        for scale in SCALE_ORDER {
            assert_eq!(report.scale_counts.get(scale), Some(&5));
        }
        assert!(
            report
                .active_template_ids
                .iter()
                .all(|id| !id.contains("hades_l5_partial"))
        );
    }
}
