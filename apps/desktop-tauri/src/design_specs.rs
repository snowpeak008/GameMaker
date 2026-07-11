use std::path::{Path, PathBuf};

use adm_new_application::{DesignChecklistItemSpec, DesignNodeSpec, DesignOptionGroupSpec};
use adm_new_design::data_loader::DesignDataLoader;

pub struct LoadedDesignSpecs {
    pub specs: Vec<DesignNodeSpec>,
    pub source_root: Option<PathBuf>,
    pub warnings: Vec<String>,
}

pub fn load_design_specs() -> LoadedDesignSpecs {
    let mut warnings = Vec::new();
    for root in project_root_candidates() {
        let loader = DesignDataLoader::new(&root);
        match loader.load_domains() {
            Ok(domains) => {
                let specs = domains
                    .into_iter()
                    .flat_map(|domain| {
                        let domain_id = domain.domain.id;
                        domain.nodes.into_iter().map(move |node| DesignNodeSpec {
                            node_id: node.id,
                            domain_id: if node.domain.trim().is_empty() {
                                domain_id.clone()
                            } else {
                                node.domain
                            },
                            name: node.name,
                            description: node.description,
                            role_class: node.role_class,
                            checklist: node
                                .checklist
                                .into_iter()
                                .map(|item| DesignChecklistItemSpec {
                                    item_id: item.id,
                                    label: item.label,
                                    option_groups: item
                                        .option_groups
                                        .into_iter()
                                        .map(|group| DesignOptionGroupSpec {
                                            group_id: group.id,
                                            selection_mode: group.selection_mode,
                                            allow_primary: group.allow_primary,
                                            options: group
                                                .options
                                                .into_iter()
                                                .map(|option| option.id)
                                                .filter(|id| !id.trim().is_empty())
                                                .collect(),
                                        })
                                        .collect(),
                                })
                                .collect(),
                        })
                    })
                    .collect::<Vec<_>>();
                if !specs.is_empty() {
                    return LoadedDesignSpecs {
                        specs,
                        source_root: Some(root),
                        warnings,
                    };
                }
                warnings.push(format!(
                    "design data contained no nodes: {}",
                    loader.design_data_dir().display()
                ));
            }
            Err(error) => warnings.push(format!(
                "failed to load design data from {}: {error}",
                loader.design_data_dir().display()
            )),
        }
    }

    warnings.push("using embedded fallback design taxonomy".to_string());
    LoadedDesignSpecs {
        specs: fallback_design_specs(),
        source_root: None,
        warnings,
    }
}

fn project_root_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(root) = std::env::var_os("ADM_NEWRUST_SOURCE_ROOT") {
        push_candidate(&mut candidates, PathBuf::from(root));
    }
    if let Ok(current) = std::env::current_dir() {
        push_ancestors(&mut candidates, &current);
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        push_ancestors(&mut candidates, parent);
    }
    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    push_candidate(&mut candidates, manifest_root);
    candidates
        .into_iter()
        .filter(|path| path.join("knowledge/design_data/domains").is_dir())
        .collect()
}

fn push_ancestors(candidates: &mut Vec<PathBuf>, start: &Path) {
    for ancestor in start.ancestors() {
        push_candidate(candidates, ancestor.to_path_buf());
    }
}

fn push_candidate(candidates: &mut Vec<PathBuf>, candidate: PathBuf) {
    let candidate = candidate.canonicalize().unwrap_or(candidate);
    if !candidates.iter().any(|existing| existing == &candidate) {
        candidates.push(candidate);
    }
}

fn fallback_design_specs() -> Vec<DesignNodeSpec> {
    [
        (
            "product_positioning_design",
            "产品定位",
            "明确目标用户、平台与产品承诺",
        ),
        (
            "core_experience_design",
            "核心体验",
            "定义玩家体验目标与核心循环",
        ),
        (
            "gameplay_system_design",
            "玩法系统",
            "建立可执行的玩法与系统规则",
        ),
        (
            "content_design",
            "内容设计",
            "规划关卡、角色、叙事与内容节奏",
        ),
        (
            "economy_monetization_design",
            "经济与商业化",
            "定义资源循环、成长与商业边界",
        ),
        (
            "ux_interface_design",
            "交互界面",
            "定义信息架构、输入与反馈",
        ),
        (
            "presentation_feel_design",
            "表现与手感",
            "定义视听表现和操作手感",
        ),
        ("balance_design", "数值平衡", "定义数值模型与平衡验证"),
        (
            "social_community_design",
            "社交社区",
            "定义社交关系与社区机制",
        ),
        (
            "retention_lifecycle_design",
            "留存生命周期",
            "定义长期目标与回流路径",
        ),
        (
            "liveops_version_design",
            "运营版本",
            "定义版本节奏与活动框架",
        ),
        (
            "data_validation_design",
            "数据验证",
            "定义指标、埋点与验证方式",
        ),
        (
            "compliance_risk_design",
            "合规风险",
            "识别合规、平台与制作风险",
        ),
        (
            "documentation_collaboration_design",
            "文档协作",
            "定义交接、评审与变更规则",
        ),
        (
            "release_growth_design",
            "发布增长",
            "定义发布渠道与增长策略",
        ),
        (
            "launch_readiness_design",
            "上线准备",
            "定义上线门禁与应急预案",
        ),
    ]
    .into_iter()
    .map(|(domain_id, name, description)| DesignNodeSpec {
        node_id: format!("{domain_id}_core"),
        domain_id: domain_id.to_string(),
        name: name.to_string(),
        description: description.to_string(),
        role_class: "system_concrete".to_string(),
        checklist: vec![
            checklist("goal", "目标与边界", Vec::new()),
            checklist(
                "decision",
                "关键方案",
                vec![DesignOptionGroupSpec {
                    group_id: "depth".to_string(),
                    selection_mode: "single".to_string(),
                    allow_primary: true,
                    options: vec![
                        "focused".to_string(),
                        "balanced".to_string(),
                        "deep".to_string(),
                    ],
                }],
            ),
            checklist("acceptance", "验收信号", Vec::new()),
        ],
    })
    .collect()
}

fn checklist(
    item_id: &str,
    label: &str,
    option_groups: Vec<DesignOptionGroupSpec>,
) -> DesignChecklistItemSpec {
    DesignChecklistItemSpec {
        item_id: item_id.to_string(),
        label: label.to_string(),
        option_groups,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn design_specs_load_real_taxonomy_or_complete_fallback() {
        let loaded = load_design_specs();
        assert!(!loaded.specs.is_empty());
        let domain_count = loaded
            .specs
            .iter()
            .map(|spec| spec.domain_id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        assert!(domain_count >= 16);
    }
}
