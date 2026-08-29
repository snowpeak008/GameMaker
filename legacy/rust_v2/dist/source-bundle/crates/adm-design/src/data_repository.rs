use adm_foundation::{AdmError, AdmResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignDataRepository {
    pub source_root: PathBuf,
    pub domains: Vec<DesignDomain>,
    pub gameplay_system_options: Vec<GameplaySystemOption>,
    pub profile_fields: Vec<ProfileField>,
    pub entity_schemas: BTreeMap<String, EntitySchema>,
    pub validation_errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignDomain {
    pub id: String,
    pub name: String,
    pub description: String,
    pub nodes: Vec<DesignNode>,
    pub coverage_required_items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignNode {
    pub id: String,
    pub domain_id: String,
    pub name: String,
    pub description: String,
    pub role_class: String,
    pub priority: String,
    pub requires: Vec<String>,
    pub unlocks: Vec<String>,
    pub checklist: Vec<ChecklistItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub progression_step: u32,
    pub relation: String,
    pub design_question: String,
    pub options: Vec<OptionItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionItem {
    pub id: String,
    pub label: String,
    pub description: String,
    pub output_key: String,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OptionRef {
    pub group_id: String,
    pub option_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameplaySystemOption {
    pub id: String,
    pub name: String,
    pub category: String,
    pub mapping_desc: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileField {
    pub id: String,
    pub label: String,
    pub options: Vec<ProfileOption>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntitySchema {
    pub id: String,
    pub kind: String,
    pub schema_version: String,
    pub required: Vec<String>,
    pub constants: BTreeMap<String, String>,
}

pub fn load_design_data_repository(design_data_root: &Path) -> AdmResult<DesignDataRepository> {
    let template_values = load_templates(&design_data_root.join("templates"))?;
    let entity_schemas = load_entity_schemas(&design_data_root.join("entity_schemas"))?;
    let domains = load_domains(design_data_root, &template_values)?;
    let gameplay_system_options =
        load_gameplay_system_options(&design_data_root.join("gameplay_system_options.json"))?;
    let profile_fields = default_profile_fields();
    let validation_errors = validate_repository(&domains, &gameplay_system_options);

    Ok(DesignDataRepository {
        source_root: design_data_root.to_path_buf(),
        domains,
        gameplay_system_options,
        profile_fields,
        entity_schemas,
        validation_errors,
    })
}

fn load_domains(
    design_data_root: &Path,
    template_values: &BTreeMap<String, Value>,
) -> AdmResult<Vec<DesignDomain>> {
    let order = load_domain_order(&design_data_root.join("domain_order.json"))?;
    let domains_root = design_data_root.join("domains");
    let mut by_id = BTreeMap::new();
    for entry in fs::read_dir(&domains_root).map_err(|error| {
        AdmError::invalid_input(format!(
            "failed to read {}: {error}",
            domains_root.display()
        ))
    })? {
        let path = entry
            .map_err(|error| AdmError::invalid_input(format!("failed to read domain: {error}")))?
            .path();
        if path.extension().and_then(|item| item.to_str()) != Some("json") {
            continue;
        }
        let value = read_json(&path)?;
        let domain = parse_domain(value, template_values)?;
        by_id.insert(domain.id.clone(), domain);
    }

    let mut domains = Vec::new();
    for domain_id in order {
        if let Some(domain) = by_id.remove(&domain_id) {
            domains.push(domain);
        }
    }
    domains.extend(by_id.into_values());
    Ok(domains)
}

fn parse_domain(
    value: Value,
    template_values: &BTreeMap<String, Value>,
) -> AdmResult<DesignDomain> {
    let domain_value = value.get("domain").unwrap_or(&Value::Null);
    let id = string_at(domain_value, "id")
        .ok_or_else(|| AdmError::validation("domain document is missing domain.id".to_string()))?;
    let name = string_at(domain_value, "name").unwrap_or_else(|| id.clone());
    let description = string_at(domain_value, "description").unwrap_or_default();
    let nodes = value
        .get("nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, node_value)| parse_node(&id, index, node_value, template_values))
        .collect::<AdmResult<Vec<_>>>()?;
    let coverage_required_items = value
        .get("coverageStandard")
        .and_then(|item| item.get("requiredItems"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
        .unwrap_or_else(|| nodes.iter().map(|node| node.id.clone()).collect());
    Ok(DesignDomain {
        id,
        name,
        description,
        nodes,
        coverage_required_items,
    })
}

fn parse_node(
    domain_id: &str,
    index: usize,
    value: &Value,
    template_values: &BTreeMap<String, Value>,
) -> AdmResult<DesignNode> {
    let id = string_at(value, "id").unwrap_or_else(|| format!("{domain_id}_node_{index}"));
    let checklist = value
        .get("checklist")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(item_index, item_value)| {
            parse_checklist_item(&id, item_index, item_value, template_values)
        })
        .collect::<AdmResult<Vec<_>>>()?;
    Ok(DesignNode {
        id,
        domain_id: string_at(value, "domain").unwrap_or_else(|| domain_id.to_string()),
        name: string_at(value, "name").unwrap_or_else(|| format!("{domain_id} 节点")),
        description: string_at(value, "description").unwrap_or_default(),
        role_class: normalize_role_class(&string_at(value, "roleClass").unwrap_or_default()),
        priority: string_at(value, "priority").unwrap_or_default(),
        requires: string_array_at(value, "requires"),
        unlocks: string_array_at(value, "unlocks"),
        checklist,
    })
}

fn parse_checklist_item(
    node_id: &str,
    index: usize,
    value: &Value,
    template_values: &BTreeMap<String, Value>,
) -> AdmResult<ChecklistItem> {
    let legacy_id = format!("{}_item_{}", node_id, index + 1);
    let id = string_at(value, "id").unwrap_or_else(|| legacy_id.clone());
    let mut legacy_ids = string_array_at(value, "legacyIds");
    if id != legacy_id && !legacy_ids.iter().any(|item| item == &legacy_id) {
        legacy_ids.push(legacy_id);
    }
    let template_ref = string_at(value, "templateRef")
        .or_else(|| string_at(value, "template_ref"))
        .unwrap_or_default();
    let option_group_values = option_group_values(value, &template_ref, template_values);
    Ok(ChecklistItem {
        id: id.clone(),
        label: string_at(value, "label").unwrap_or_else(|| id.clone()),
        description: string_at(value, "description").unwrap_or_default(),
        output_key: string_at(value, "outputKey").unwrap_or_else(|| camel_case(&id)),
        legacy_ids,
        template_ref,
        option_groups: option_group_values
            .iter()
            .map(parse_option_group)
            .collect::<Vec<_>>(),
        option_relations: option_relation_values(value, template_values)
            .iter()
            .filter_map(parse_option_relation)
            .collect(),
    })
}

fn option_group_values(
    item: &Value,
    template_ref: &str,
    template_values: &BTreeMap<String, Value>,
) -> Vec<Value> {
    if let Some(groups) = item.get("optionGroups").and_then(Value::as_array) {
        if !groups.is_empty() {
            return groups.clone();
        }
    }
    template_values
        .get(template_ref)
        .and_then(|template| template.get("optionGroups"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn option_relation_values(item: &Value, template_values: &BTreeMap<String, Value>) -> Vec<Value> {
    if let Some(relations) = item.get("optionRelations").and_then(Value::as_array) {
        if !relations.is_empty() {
            return relations.clone();
        }
    }
    let template_ref = string_at(item, "templateRef")
        .or_else(|| string_at(item, "template_ref"))
        .unwrap_or_default();
    template_values
        .get(&template_ref)
        .and_then(|template| template.get("optionRelations"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn parse_option_group(value: &Value) -> OptionGroup {
    let id = string_at(value, "id").unwrap_or_else(|| "option_group".to_string());
    let mda_layer = string_at(value, "mdaLayer").unwrap_or_default();
    let selection_mode = string_at(value, "selectionMode").unwrap_or_else(|| "multi".to_string());
    OptionGroup {
        id: id.clone(),
        label: string_at(value, "label").unwrap_or_else(|| id.clone()),
        description: string_at(value, "description").unwrap_or_default(),
        output_key: string_at(value, "outputKey").unwrap_or_else(|| camel_case(&id)),
        selection_mode: if selection_mode == "single" {
            "single".to_string()
        } else {
            "multi".to_string()
        },
        required: bool_at(value, "required"),
        allow_primary: bool_at(value, "allowPrimary"),
        mda_layer_label: string_at(value, "mdaLayerLabel")
            .unwrap_or_else(|| mda_layer_label(&mda_layer).to_string()),
        mda_layer,
        progression_step: value
            .get("progressionStep")
            .and_then(Value::as_u64)
            .unwrap_or_default() as u32,
        relation: string_at(value, "relation").unwrap_or_default(),
        design_question: string_at(value, "designQuestion").unwrap_or_default(),
        options: value
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(parse_option_item)
            .collect(),
    }
}

fn parse_option_item(value: &Value) -> OptionItem {
    if let Some(text) = value.as_str() {
        return OptionItem {
            id: text.to_string(),
            label: text.to_string(),
            description: String::new(),
            output_key: camel_case(text),
        };
    }
    let id = string_at(value, "id").unwrap_or_else(|| "option".to_string());
    OptionItem {
        id: id.clone(),
        label: string_at(value, "label").unwrap_or_else(|| id.clone()),
        description: string_at(value, "description").unwrap_or_default(),
        output_key: string_at(value, "outputKey").unwrap_or_else(|| camel_case(&id)),
    }
}

fn parse_option_relation(value: &Value) -> Option<OptionRelation> {
    let relation_type = string_at(value, "type").unwrap_or_else(|| "soft_conflict".to_string());
    if !matches!(relation_type.as_str(), "soft_conflict" | "hard_exclusive") {
        return None;
    }
    let source = normalize_option_ref(value.get("source")?)?;
    let targets = value
        .get("targets")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(normalize_option_ref)
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return None;
    }
    let id = string_at(value, "id")
        .unwrap_or_else(|| format!("{}_{}_{}", relation_type, source.group_id, source.option_id));
    Some(OptionRelation {
        id,
        relation_type,
        source,
        targets,
        reason: string_at(value, "reason").unwrap_or_default(),
        severity: string_at(value, "severity").unwrap_or_else(|| "warning".to_string()),
    })
}

fn normalize_option_ref(value: &Value) -> Option<OptionRef> {
    if let Some(text) = value.as_str() {
        let (group_id, option_id) = text.split_once('.')?;
        return Some(OptionRef {
            group_id: group_id.trim().to_string(),
            option_id: option_id.trim().to_string(),
        });
    }
    let group_id = string_at(value, "groupId")
        .or_else(|| string_at(value, "group"))
        .or_else(|| string_at(value, "group_id"))?;
    let option_id = string_at(value, "optionId")
        .or_else(|| string_at(value, "option"))
        .or_else(|| string_at(value, "option_id"))?;
    if group_id.is_empty() || option_id.is_empty() {
        return None;
    }
    Some(OptionRef {
        group_id,
        option_id,
    })
}

fn load_domain_order(path: &Path) -> AdmResult<Vec<String>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(read_json(path)?
        .get("domainOrder")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect())
}

fn load_templates(path: &Path) -> AdmResult<BTreeMap<String, Value>> {
    let mut templates = BTreeMap::new();
    if !path.exists() {
        return Ok(templates);
    }
    for entry in fs::read_dir(path).map_err(|error| {
        AdmError::invalid_input(format!("failed to read {}: {error}", path.display()))
    })? {
        let path = entry
            .map_err(|error| AdmError::invalid_input(format!("failed to read template: {error}")))?
            .path();
        if path.extension().and_then(|item| item.to_str()) != Some("json") {
            continue;
        }
        let value = read_json(&path)?;
        let id = string_at(&value, "id").unwrap_or_else(|| {
            path.file_stem()
                .and_then(|item| item.to_str())
                .unwrap_or_default()
                .to_string()
        });
        templates.insert(id, value);
    }
    Ok(templates)
}

fn load_gameplay_system_options(path: &Path) -> AdmResult<Vec<GameplaySystemOption>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let value = read_json(path)?;
    let mut seen = BTreeSet::new();
    Ok(value
        .get("options")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let id = string_at(item, "id")?;
            if id.is_empty() || !seen.insert(id.clone()) {
                return None;
            }
            Some(GameplaySystemOption {
                name: string_at(item, "name").unwrap_or_else(|| id.clone()),
                category: string_at(item, "category").unwrap_or_else(|| "preset".to_string()),
                mapping_desc: string_at(item, "mapping_desc")
                    .or_else(|| string_at(item, "mappingDesc"))
                    .unwrap_or_default(),
                id,
            })
        })
        .collect())
}

fn load_entity_schemas(path: &Path) -> AdmResult<BTreeMap<String, EntitySchema>> {
    let mut schemas = BTreeMap::new();
    if !path.exists() {
        return Ok(schemas);
    }
    for entry in fs::read_dir(path).map_err(|error| {
        AdmError::invalid_input(format!("failed to read {}: {error}", path.display()))
    })? {
        let path = entry
            .map_err(|error| AdmError::invalid_input(format!("failed to read schema: {error}")))?
            .path();
        if path.extension().and_then(|item| item.to_str()) != Some("json") {
            continue;
        }
        let value = read_json(&path)?;
        let id = string_at(&value, "id").unwrap_or_else(|| {
            path.file_stem()
                .and_then(|item| item.to_str())
                .unwrap_or_default()
                .to_string()
        });
        let properties = value
            .get("properties")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let constants = properties
            .iter()
            .filter_map(|(key, property)| {
                property
                    .get("const")
                    .and_then(Value::as_str)
                    .map(|constant| (key.clone(), constant.to_string()))
            })
            .collect();
        let kind = string_at(&value, "kind")
            .or_else(|| {
                properties
                    .get("kind")
                    .and_then(|item| string_at(item, "const"))
            })
            .unwrap_or_default();
        let schema_version = string_at(&value, "schemaVersion")
            .or_else(|| {
                properties
                    .get("schemaVersion")
                    .and_then(|item| string_at(item, "const"))
            })
            .unwrap_or_default();
        schemas.insert(
            id.clone(),
            EntitySchema {
                id,
                kind,
                schema_version,
                required: string_array_at(&value, "required"),
                constants,
            },
        );
    }
    Ok(schemas)
}

fn validate_repository(
    domains: &[DesignDomain],
    gameplay_system_options: &[GameplaySystemOption],
) -> Vec<String> {
    let mut errors = Vec::new();
    let mut domain_ids = BTreeSet::new();
    let mut node_ids = BTreeSet::new();
    for domain in domains {
        if domain.id.is_empty() {
            errors.push("存在缺少 id 的领域。".to_string());
        }
        if !domain_ids.insert(domain.id.clone()) {
            errors.push(format!("重复领域 id：{}", domain.id));
        }
        for node in &domain.nodes {
            if node.id.is_empty() {
                errors.push(format!("领域 {} 存在空节点 id。", domain.id));
            }
            if !node_ids.insert(node.id.clone()) {
                errors.push(format!("重复节点 id：{}", node.id));
            }
            for item in &node.checklist {
                if item.label.trim().is_empty() {
                    errors.push(format!(
                        "节点 {} 的 checklist {} 缺少 label。",
                        node.id, item.id
                    ));
                }
                let mut group_ids = BTreeSet::new();
                for group in &item.option_groups {
                    if !group_ids.insert(group.id.clone()) {
                        errors.push(format!(
                            "节点 {} 的 checklist {} 存在重复 L4 组：{}",
                            node.id, item.id, group.id
                        ));
                    }
                }
            }
        }
    }
    let mut gameplay_ids = BTreeSet::new();
    for option in gameplay_system_options {
        if !gameplay_ids.insert(option.id.clone()) {
            errors.push(format!("玩法系统预设 id 重复：{}", option.id));
        }
    }
    errors
}

pub fn default_profile_fields() -> Vec<ProfileField> {
    vec![
        profile_field(
            "businessModel",
            "商业模式",
            &[
                ("unknown", "未确定"),
                ("buyout", "买断制"),
                ("free_to_play", "免费游玩"),
                ("subscription", "订阅制"),
                ("premium_with_dlc", "买断制 + DLC"),
            ],
        ),
        profile_field(
            "operationModel",
            "运营模式",
            &[
                ("unknown", "未确定"),
                ("offline_single_release", "离线单次发布"),
                ("content_updates", "持续内容更新"),
                ("live_service", "长线服务运营"),
            ],
        ),
        profile_field(
            "socialModel",
            "社交结构",
            &[
                ("unknown", "未确定"),
                ("none", "无社交"),
                ("async_light", "轻量异步社交"),
                ("multiplayer", "多人在线"),
                ("community_driven", "社区驱动"),
            ],
        ),
        profile_field(
            "platformScope",
            "平台范围",
            &[
                ("unknown", "未确定"),
                ("single_platform", "单平台"),
                ("multi_platform", "多平台"),
            ],
        ),
        profile_field(
            "primaryPlatform",
            "主平台",
            &[
                ("unknown", "未确定"),
                ("mobile", "手机"),
                ("pc_console", "PC / 主机"),
                ("web", "Web"),
                ("cross_platform", "跨平台同等优先"),
            ],
        ),
        profile_field(
            "regionScope",
            "发行区域",
            &[
                ("unknown", "未确定"),
                ("single_region", "单一区域"),
                ("multi_region", "多区域"),
                ("global", "全球发行"),
            ],
        ),
        profile_field(
            "targetScale",
            "项目规模",
            &[
                ("unknown", "未确定"),
                ("iaa_hypercasual", "IAA 超休闲小游戏"),
                ("indie", "独立游戏"),
                ("midcore", "中度商业游戏"),
                ("3a", "3A 游戏"),
                ("large_service", "大型长线服务游戏"),
            ],
        ),
        profile_field(
            "contentRating",
            "内容分级",
            &[
                ("unknown", "未确定"),
                ("all_ages", "全年龄"),
                ("teen", "青少年"),
                ("mature_17_plus", "M / 17+"),
            ],
        ),
        profile_field(
            "targetSessionBand",
            "目标单次时长",
            &[
                ("unknown", "未确定"),
                ("session_1_3", "1-3 分钟"),
                ("session_3_10", "3-10 分钟"),
                ("session_10_20", "10-20 分钟"),
                ("session_20_40", "20-40 分钟"),
                ("session_40_plus", "40 分钟以上"),
            ],
        ),
    ]
}

fn profile_field(id: &str, label: &str, options: &[(&str, &str)]) -> ProfileField {
    ProfileField {
        id: id.to_string(),
        label: label.to_string(),
        options: options
            .iter()
            .map(|(value, label)| ProfileOption {
                value: (*value).to_string(),
                label: (*label).to_string(),
            })
            .collect(),
    }
}

fn read_json(path: &Path) -> AdmResult<Value> {
    let text = fs::read_to_string(path).map_err(|error| {
        AdmError::invalid_input(format!("failed to read {}: {error}", path.display()))
    })?;
    serde_json::from_str(&text).map_err(|error| {
        AdmError::validation(format!("failed to parse {}: {error}", path.display()))
    })
}

fn string_at(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
}

fn bool_at(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn string_array_at(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn camel_case(value: &str) -> String {
    let mut parts = value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty());
    let Some(head) = parts.next() else {
        return String::new();
    };
    let mut output = head.to_ascii_lowercase();
    for part in parts {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            output.push(first.to_ascii_uppercase());
            output.extend(chars);
        }
    }
    output
}

fn normalize_role_class(value: &str) -> String {
    match value {
        "system_concrete" | "content_concrete" | "system_abstract" | "content_abstract" => {
            value.to_string()
        }
        _ => String::new(),
    }
}

fn mda_layer_label(value: &str) -> &'static str {
    match value {
        "aesthetics" => "体验目标",
        "dynamics" => "玩家动态",
        "mechanics" => "机制抓手",
        "constraints" => "边界约束",
        "evidence" => "验收信号",
        _ => "",
    }
}
