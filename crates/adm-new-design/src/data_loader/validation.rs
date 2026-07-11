use crate::data_loader::{
    DEFAULT_ROLE_CLASS, DomainDocument, GameplaySystemOption, ROLE_CLASS_VALUES, valid_mda_layer,
    valid_relation_type,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateReuseReport {
    pub template_refs: BTreeMap<String, usize>,
    pub shared_option_groups: Vec<SharedOptionGroupReuse>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedOptionGroupReuse {
    pub group_id: String,
    pub count: usize,
    pub declared_template_refs: usize,
    pub undeclared_refs: usize,
}

pub fn validate_gameplay_system_options(options: &[GameplaySystemOption]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    for option in options {
        if option.id.trim().is_empty() {
            errors.push("玩法系统预设存在空 id。".to_string());
            continue;
        }
        if !seen.insert(option.id.clone()) {
            errors.push(format!("玩法系统预设 id 重复：{}", option.id));
        }
        if option.name.trim().is_empty() {
            errors.push(format!("玩法系统预设 {} 缺少 name。", option.id));
        }
        if option.category.trim().is_empty() {
            errors.push(format!("玩法系统预设 {} 缺少 category。", option.id));
        }
        if option.mapping_desc.trim().is_empty() {
            errors.push(format!("玩法系统预设 {} 缺少 mapping_desc。", option.id));
        }
    }
    errors
}

pub fn validate_domains(domains: &[DomainDocument]) -> Vec<String> {
    let mut errors = Vec::new();
    let mut domain_ids = BTreeSet::new();
    let mut node_ids = BTreeSet::new();

    for domain_doc in domains {
        let domain_id = &domain_doc.domain.id;
        if domain_id.trim().is_empty() {
            errors.push("存在缺少 domain.id 的领域文件。".to_string());
            continue;
        }
        if !domain_ids.insert(domain_id.clone()) {
            errors.push(format!("重复 domain id：{domain_id}"));
        }
        for node in &domain_doc.nodes {
            if node.id.trim().is_empty() {
                errors.push(format!("领域 {domain_id} 存在缺少 id 的节点。"));
                continue;
            }
            if !node_ids.insert(node.id.clone()) {
                errors.push(format!("重复节点 id：{}", node.id));
            }
            if node.domain != *domain_id {
                errors.push(format!("节点 {} 的 domain 与文件 domain 不一致。", node.id));
            }
        }
    }

    for domain_doc in domains {
        let domain_id = &domain_doc.domain.id;
        for required_id in &domain_doc.coverage_standard.required_items {
            if !node_ids.contains(required_id) {
                errors.push(format!(
                    "领域 {domain_id} 的 coverage requiredItems 引用了不存在节点：{required_id}"
                ));
            }
        }
        for node in &domain_doc.nodes {
            for (field_name, relation_ids) in [
                ("requires", &node.requires),
                ("unlocks", &node.unlocks),
                ("recommendedBefore", &node.recommended_before),
                ("requiresAny", &node.requires_any),
                ("conflictsWith", &node.conflicts_with),
            ] {
                for relation_id in relation_ids {
                    if !node_ids.contains(relation_id) {
                        errors.push(format!(
                            "节点 {} 的 {field_name} 引用了不存在节点：{relation_id}",
                            node.id
                        ));
                    }
                }
            }

            let mut checklist_ids = BTreeSet::new();
            let mut item_output_keys = BTreeSet::new();
            for item in &node.checklist {
                if !checklist_ids.insert(item.id.clone()) {
                    errors.push(format!(
                        "节点 {} 存在重复 checklist id：{}",
                        node.id, item.id
                    ));
                }
                if item.label.trim().is_empty() {
                    errors.push(format!(
                        "节点 {} 的 checklist {} 缺少 label。",
                        node.id, item.id
                    ));
                }
                if item.description.trim().is_empty() {
                    errors.push(format!(
                        "节点 {} 的 checklist {} 缺少 description。",
                        node.id, item.id
                    ));
                }
                if item.output_key.trim().is_empty() {
                    errors.push(format!(
                        "节点 {} 的 checklist {} 缺少 outputKey。",
                        node.id, item.id
                    ));
                } else if !item_output_keys.insert(item.output_key.clone()) {
                    errors.push(format!(
                        "节点 {} 存在重复 checklist outputKey：{}",
                        node.id, item.output_key
                    ));
                }

                let mut group_ids = BTreeSet::new();
                let mut group_output_keys = BTreeSet::new();
                let mut option_refs = BTreeSet::new();
                let mut relation_ids = BTreeSet::new();
                for group in &item.option_groups {
                    if group.id.trim().is_empty() {
                        errors.push(format!(
                            "节点 {} 的 checklist {} 存在空 optionGroup id。",
                            node.id, item.id
                        ));
                    } else if !group_ids.insert(group.id.clone()) {
                        errors.push(format!(
                            "节点 {} 的 checklist {} 存在重复 optionGroup id：{}",
                            node.id, item.id, group.id
                        ));
                    }
                    if group.label.trim().is_empty() {
                        errors.push(format!(
                            "节点 {} 的 checklist {} / optionGroup {} 缺少 label。",
                            node.id, item.id, group.id
                        ));
                    }
                    if group.mda_layer.trim().is_empty() {
                        errors.push(format!(
                            "节点 {} 的 checklist {} / optionGroup {} 缺少 mdaLayer。",
                            node.id, item.id, group.id
                        ));
                    } else if !valid_mda_layer(&group.mda_layer) {
                        errors.push(format!(
                            "节点 {} 的 checklist {} / optionGroup {} 的 mdaLayer 非法：{}",
                            node.id, item.id, group.id, group.mda_layer
                        ));
                    }
                    if group.progression_step == 0 {
                        errors.push(format!(
                            "节点 {} 的 checklist {} / optionGroup {} 缺少 progressionStep。",
                            node.id, item.id, group.id
                        ));
                    }
                    if group.relation.trim().is_empty() {
                        errors.push(format!(
                            "节点 {} 的 checklist {} / optionGroup {} 缺少 relation。",
                            node.id, item.id, group.id
                        ));
                    }
                    if group.design_question.trim().is_empty() {
                        errors.push(format!(
                            "节点 {} 的 checklist {} / optionGroup {} 缺少 designQuestion。",
                            node.id, item.id, group.id
                        ));
                    }
                    if group.output_key.trim().is_empty() {
                        errors.push(format!(
                            "节点 {} 的 checklist {} / optionGroup {} 缺少 outputKey。",
                            node.id, item.id, group.id
                        ));
                    } else if !group_output_keys.insert(group.output_key.clone()) {
                        errors.push(format!(
                            "节点 {} 的 checklist {} 存在重复 optionGroup outputKey：{}",
                            node.id, item.id, group.output_key
                        ));
                    }

                    let mut option_ids = BTreeSet::new();
                    let mut option_output_keys = BTreeSet::new();
                    for option in &group.options {
                        if option.id.trim().is_empty() {
                            errors.push(format!(
                                "节点 {} 的 checklist {} / optionGroup {} 存在空 option id。",
                                node.id, item.id, group.id
                            ));
                        } else if !option_ids.insert(option.id.clone()) {
                            errors.push(format!(
                                "节点 {} 的 checklist {} / optionGroup {} 存在重复 option id：{}",
                                node.id, item.id, group.id, option.id
                            ));
                        }
                        if option.label.trim().is_empty() {
                            errors.push(format!(
                                "节点 {} 的 checklist {} / optionGroup {} / option {} 缺少 label。",
                                node.id, item.id, group.id, option.id
                            ));
                        }
                        if option.output_key.trim().is_empty() {
                            errors.push(format!(
                                "节点 {} 的 checklist {} / optionGroup {} / option {} 缺少 outputKey。",
                                node.id, item.id, group.id, option.id
                            ));
                        } else if !option_output_keys.insert(option.output_key.clone()) {
                            errors.push(format!(
                                "节点 {} 的 checklist {} / optionGroup {} 存在重复 option outputKey：{}",
                                node.id, item.id, group.id, option.output_key
                            ));
                        }
                        option_refs.insert((group.id.clone(), option.id.clone()));
                    }
                }

                for relation in &item.option_relations {
                    if relation.id.trim().is_empty() {
                        errors.push(format!(
                            "节点 {} 的 checklist {} 存在空 optionRelation id。",
                            node.id, item.id
                        ));
                    } else if !relation_ids.insert(relation.id.clone()) {
                        errors.push(format!(
                            "节点 {} 的 checklist {} 存在重复 optionRelation id：{}",
                            node.id, item.id, relation.id
                        ));
                    }
                    if !valid_relation_type(&relation.relation_type) {
                        errors.push(format!(
                            "节点 {} 的 checklist {} / optionRelation {} 类型非法：{}",
                            node.id, item.id, relation.id, relation.relation_type
                        ));
                    }
                    let source_ref = (
                        relation.source.group_id.clone(),
                        relation.source.option_id.clone(),
                    );
                    if !option_refs.contains(&source_ref) {
                        errors.push(format!(
                            "节点 {} 的 checklist {} / optionRelation {} source 引用了不存在选项：{:?}",
                            node.id, item.id, relation.id, source_ref
                        ));
                    }
                    if relation.reason.trim().is_empty() {
                        errors.push(format!(
                            "节点 {} 的 checklist {} / optionRelation {} 缺少 reason。",
                            node.id, item.id, relation.id
                        ));
                    }
                    for target in &relation.targets {
                        let target_ref = (target.group_id.clone(), target.option_id.clone());
                        if !option_refs.contains(&target_ref) {
                            errors.push(format!(
                                "节点 {} 的 checklist {} / optionRelation {} target 引用了不存在选项：{:?}",
                                node.id, item.id, relation.id, target_ref
                            ));
                        }
                    }
                }
            }
        }
    }
    errors
}

pub fn count_role_classes(domains: &[DomainDocument]) -> BTreeMap<String, usize> {
    let mut counts = ROLE_CLASS_VALUES
        .iter()
        .map(|role| ((*role).to_string(), 0))
        .collect::<BTreeMap<_, _>>();
    for domain in domains {
        for node in &domain.nodes {
            let role = if ROLE_CLASS_VALUES.contains(&node.role_class.as_str()) {
                node.role_class.as_str()
            } else {
                DEFAULT_ROLE_CLASS
            };
            *counts.entry(role.to_string()).or_insert(0) += 1;
        }
    }
    counts
}

pub fn scan_template_reuse(domains: &[DomainDocument]) -> TemplateReuseReport {
    let mut group_refs = BTreeMap::<String, Vec<(String, String)>>::new();
    let mut template_refs = BTreeMap::<String, Vec<String>>::new();
    for domain in domains {
        for node in &domain.nodes {
            for item in &node.checklist {
                let item_path = format!("{}.{}.{}", domain.domain.id, node.id, item.id);
                if !item.template_ref.trim().is_empty() {
                    template_refs
                        .entry(item.template_ref.clone())
                        .or_default()
                        .push(item_path.clone());
                }
                for group in &item.option_groups {
                    group_refs
                        .entry(group.id.clone())
                        .or_default()
                        .push((item_path.clone(), item.template_ref.clone()));
                }
            }
        }
    }

    let shared_option_groups = group_refs
        .into_iter()
        .filter_map(|(group_id, refs)| {
            if refs.len() < 2 {
                return None;
            }
            let declared_template_refs = refs
                .iter()
                .filter(|(_, template_ref)| !template_ref.trim().is_empty())
                .count();
            Some(SharedOptionGroupReuse {
                group_id,
                count: refs.len(),
                declared_template_refs,
                undeclared_refs: refs.len() - declared_template_refs,
            })
        })
        .collect();

    TemplateReuseReport {
        template_refs: template_refs
            .into_iter()
            .map(|(key, refs)| (key, refs.len()))
            .collect(),
        shared_option_groups,
    }
}
