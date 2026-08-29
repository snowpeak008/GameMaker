use adm_foundation::{AdmError, AdmResult};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchReference {
    pub source_root: PathBuf,
    pub domains: Vec<WorkbenchDomainOverview>,
    pub nodes: Vec<WorkbenchNodeOverview>,
    pub option_groups: Vec<WorkbenchOptionGroupOverview>,
    pub gameplay_system_count: usize,
    pub profile_fields: Vec<String>,
    pub summary_text: String,
    pub missing_text: String,
    pub risk_text: String,
    pub validation_text: String,
    pub ai_interview_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchDomainOverview {
    pub id: String,
    pub name: String,
    pub description: String,
    pub node_count: usize,
    pub checklist_count: usize,
    pub option_group_count: usize,
    pub l5_candidate_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchNodeOverview {
    pub id: String,
    pub domain_id: String,
    pub name: String,
    pub description: String,
    pub role_class: String,
    pub checklist_count: usize,
    pub option_group_count: usize,
    pub l5_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkbenchOptionGroupOverview {
    pub node_name: String,
    pub checklist_label: String,
    pub label: String,
    pub selection_mode: String,
    pub required: bool,
    pub allow_primary: bool,
    pub mda_layer_label: String,
    pub option_count: usize,
    pub design_question: String,
}

pub fn load_workbench_reference(design_data_root: &Path) -> AdmResult<WorkbenchReference> {
    let domain_order = read_json(&design_data_root.join("domain_order.json"))?;
    let domain_ids = domain_order
        .get("domainOrder")
        .and_then(Value::as_array)
        .ok_or_else(|| AdmError::validation("domain_order.json missing domainOrder array"))?
        .iter()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();

    let mut domains = Vec::new();
    let mut nodes = Vec::new();
    let mut option_groups = Vec::new();
    let mut validation_lines = Vec::new();

    for domain_id in domain_ids {
        let path = design_data_root
            .join("domains")
            .join(format!("{domain_id}.json"));
        let domain_doc = read_json(&path)?;
        let domain = domain_doc.get("domain").unwrap_or(&Value::Null);
        let name = string_at(domain, "name").unwrap_or_else(|| domain_id.clone());
        let description = string_at(domain, "description").unwrap_or_default();
        let node_values = domain_doc
            .get("nodes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut checklist_count = 0_usize;
        let mut option_group_count = 0_usize;
        let mut l5_candidate_count = 0_usize;

        for node_value in &node_values {
            let node_id = string_at(node_value, "id").unwrap_or_default();
            let node_name = string_at(node_value, "name").unwrap_or_else(|| node_id.clone());
            let node_description = string_at(node_value, "description").unwrap_or_default();
            let role_class = string_at(node_value, "roleClass").unwrap_or_default();
            let checklist = node_value
                .get("checklist")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let node_checklist_count = checklist.len();
            let mut node_option_group_count = 0_usize;

            for item in &checklist {
                let item_label = string_at(item, "label").unwrap_or_default();
                let groups = item
                    .get("optionGroups")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                node_option_group_count += groups.len();
                for group in groups {
                    option_groups.push(WorkbenchOptionGroupOverview {
                        node_name: node_name.clone(),
                        checklist_label: item_label.clone(),
                        label: string_at(&group, "label").unwrap_or_default(),
                        selection_mode: string_at(&group, "selectionMode")
                            .unwrap_or_else(|| "single".to_string()),
                        required: group
                            .get("required")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        allow_primary: group
                            .get("allowPrimary")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        mda_layer_label: string_at(&group, "mdaLayerLabel").unwrap_or_default(),
                        option_count: group
                            .get("options")
                            .and_then(Value::as_array)
                            .map(Vec::len)
                            .unwrap_or(0),
                        design_question: string_at(&group, "designQuestion").unwrap_or_default(),
                    });
                }
            }

            let l5_enabled = matches!(role_class.as_str(), "system_concrete" | "content_concrete");
            if l5_enabled {
                l5_candidate_count += 1;
            }
            checklist_count += node_checklist_count;
            option_group_count += node_option_group_count;
            nodes.push(WorkbenchNodeOverview {
                id: node_id,
                domain_id: domain_id.clone(),
                name: node_name,
                description: node_description,
                role_class,
                checklist_count: node_checklist_count,
                option_group_count: node_option_group_count,
                l5_enabled,
            });
        }

        validation_lines.push(format!(
            "{}: nodes={} checklist={} l4={} l5={}",
            name,
            node_values.len(),
            checklist_count,
            option_group_count,
            l5_candidate_count
        ));
        domains.push(WorkbenchDomainOverview {
            id: domain_id,
            name,
            description,
            node_count: node_values.len(),
            checklist_count,
            option_group_count,
            l5_candidate_count,
        });
    }

    let gameplay_system_count =
        gameplay_system_count(&design_data_root.join("gameplay_system_options.json"))?;
    let profile_fields = vec![
        "品类/平台".to_string(),
        "目标用户".to_string(),
        "商业模式".to_string(),
        "美术风格".to_string(),
        "内容规模".to_string(),
        "上线节奏".to_string(),
    ];

    let total_nodes = nodes.len();
    let total_checklist = domains
        .iter()
        .map(|domain| domain.checklist_count)
        .sum::<usize>();
    let total_l4 = domains
        .iter()
        .map(|domain| domain.option_group_count)
        .sum::<usize>();
    let total_l5 = domains
        .iter()
        .map(|domain| domain.l5_candidate_count)
        .sum::<usize>();

    let summary_text = format!(
        "设计数据源={}\n领域={}\n节点={}\n决策项={}\nL4选项组={}\nL5候选节点={}\n玩法系统={}\n项目画像字段={}",
        design_data_root.display(),
        domains.len(),
        total_nodes,
        total_checklist,
        total_l4,
        total_l5,
        gameplay_system_count,
        profile_fields.join(" / ")
    );
    let missing_text = "当前 Rust 工作台已读取旧知识库结构；下一步需要把用户实际勾选、主选项、L5 JSON 编辑、模板存取和 AI 访谈轮次写入结构化工作台状态。".to_string();
    let risk_text = "风险：当前 UI 已替换为六任务区壳，但流水线 runner 仍是 Rust 简化核心阶段；Step00-14 的真实执行模型仍需继续补齐。".to_string();
    let validation_text = format!(
        "knowledge/design_data 读取通过。\n{}",
        validation_lines
            .into_iter()
            .take(12)
            .collect::<Vec<_>>()
            .join("\n")
    );
    let ai_interview_text = "AI 访谈槽位：项目画像补问、节点缺失项追问、L4冲突解释、L5实体补全、摘要/风险/校验建议。后续写入 WorkbenchState.aiInterview。".to_string();

    Ok(WorkbenchReference {
        source_root: design_data_root.to_path_buf(),
        domains,
        nodes,
        option_groups,
        gameplay_system_count,
        profile_fields,
        summary_text,
        missing_text,
        risk_text,
        validation_text,
        ai_interview_text,
    })
}

fn read_json(path: &Path) -> AdmResult<Value> {
    let text = fs::read_to_string(path).map_err(|error| {
        AdmError::invalid_input(format!("failed to read {}: {error}", path.display()))
    })?;
    serde_json::from_str(&text).map_err(|error| {
        AdmError::validation(format!("failed to parse {}: {error}", path.display()))
    })
}

fn gameplay_system_count(path: &Path) -> AdmResult<usize> {
    let doc = read_json(path)?;
    Ok(doc
        .get("options")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0))
}

fn string_at(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
