//! 领域 / 节点两级内容组织（二版「16 领域 → 设计节点 → 检查单」的四版归宿）。
//!
//! 四版原本只有 L0-L6 深度维度，左栏无法按领域巡视设计完整度。本模块补上横向维度：
//! **领域**（跨品类通用，固定展示顺序）→ **节点**（决策点的分组单元，带角色分类）
//! → **决策点**（`DecisionPoint::node_id` 挂载）。深度维度（L 层）与组织维度正交，
//! 二者共同构成「领域 × 层级」二维视图。
//!
//! 决策点不声明 `node_id` 时归入保留领域「未分域」/保留节点「未分节点」——保留项
//! 由代码内置（清单不声明也不允许声明），因此既有品类包无需改一个字节就能参与聚合。

use crate::applicability::{ApplicabilityMap, PointApplicability};
use crate::graph::DecisionGraph;
use crate::types::{DecisionId, DomainId, NodeId, Selection};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 保留领域 id：承载未声明 `node_id` 的决策点。
pub const UNASSIGNED_DOMAIN_ID: &str = "unassigned";
/// 保留领域展示名。
pub const UNASSIGNED_DOMAIN_NAME: &str = "未分域";
/// 保留节点 id：与保留领域配对，节点级聚合与节点文本挂载都指向它。
pub const UNASSIGNED_NODE_ID: &str = "unassigned";
/// 保留节点展示名。
pub const UNASSIGNED_NODE_NAME: &str = "未分节点";
/// 保留领域的展示序：恒排在全部声明领域之后。
const UNASSIGNED_ORDER: u32 = u32::MAX;

/// 设计领域（二版 16 个通用领域的四版形态；跨品类通用，声明在通用层）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignDomain {
    pub id: DomainId,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// 左栏领域卡片的固定展示序（二版 `domain_order.json` 的四版形态）；同序按 id 升序。
    #[serde(default)]
    pub order: u32,
}

/// 设计节点：决策点的分组单元（二版的「设计节点」）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignNode {
    pub id: NodeId,
    pub domain_id: DomainId,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// 角色分类：沿用二版 `roleClass` 语义（`strategic` / `system_concrete` /
    /// `content_concrete` 等），取值开放为字符串——新增角色不必改代码。
    #[serde(default)]
    pub role_class: String,
}

/// 装配后的组织维度：声明的领域/节点 + 内置保留项。
#[derive(Debug, Clone)]
pub struct DesignOrganization {
    domains: Vec<DesignDomain>,
    nodes: Vec<DesignNode>,
}

/// 空组织：只有保留领域/节点——所有决策点都归「未分域」。
/// 未迁移的品类包就是这个形态，聚合照常工作。
impl Default for DesignOrganization {
    fn default() -> Self {
        Self::new(Vec::new(), Vec::new())
    }
}

impl DesignOrganization {
    /// 装配：按 `order` 再按 id 排序领域，节点保持声明顺序，最后追加保留领域/节点。
    ///
    /// 与清单里重名的保留项会被丢弃（保留 id 归代码所有），重名本身由
    /// `validate_organization` 报违规——装配不做静默纠错以外的判断。
    pub fn new(domains: Vec<DesignDomain>, nodes: Vec<DesignNode>) -> Self {
        let mut domains: Vec<DesignDomain> = domains
            .into_iter()
            .filter(|domain| domain.id != UNASSIGNED_DOMAIN_ID)
            .collect();
        domains.sort_by(|left, right| {
            left.order
                .cmp(&right.order)
                .then_with(|| left.id.cmp(&right.id))
        });
        domains.push(DesignDomain {
            id: UNASSIGNED_DOMAIN_ID.to_string(),
            name: UNASSIGNED_DOMAIN_NAME.to_string(),
            description: "未声明所属节点的决策点（保留领域，由代码内置）".to_string(),
            order: UNASSIGNED_ORDER,
        });
        let mut nodes: Vec<DesignNode> = nodes
            .into_iter()
            .filter(|node| node.id != UNASSIGNED_NODE_ID)
            .collect();
        nodes.push(DesignNode {
            id: UNASSIGNED_NODE_ID.to_string(),
            domain_id: UNASSIGNED_DOMAIN_ID.to_string(),
            name: UNASSIGNED_NODE_NAME.to_string(),
            role_class: "reserved".to_string(),
            description: "未声明 node_id 的决策点（保留节点，由代码内置）".to_string(),
        });
        Self { domains, nodes }
    }

    /// 全部领域（含保留领域，保留领域恒最后）。
    pub fn domains(&self) -> &[DesignDomain] {
        &self.domains
    }

    /// 全部节点（含保留节点，保留节点恒最后）。
    pub fn nodes(&self) -> &[DesignNode] {
        &self.nodes
    }

    pub fn domain(&self, id: &str) -> Option<&DesignDomain> {
        self.domains.iter().find(|domain| domain.id == id)
    }

    pub fn node(&self, id: &str) -> Option<&DesignNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// 决策点的有效节点 id：未声明 `node_id`、或声明了但节点不存在时归保留节点。
    /// （节点不存在本身是 `validate_organization` 的违规项，聚合侧仍需给出确定归属。）
    pub fn effective_node_id<'a>(&'a self, node_id: Option<&'a str>) -> &'a str {
        match node_id {
            Some(id) if self.node(id).is_some() => id,
            _ => UNASSIGNED_NODE_ID,
        }
    }

    /// 节点所属领域 id（节点未知 → 保留领域）。
    pub fn domain_of_node(&self, node_id: &str) -> &str {
        match self.node(node_id) {
            Some(node) if self.domain(&node.domain_id).is_some() => node.domain_id.as_str(),
            _ => UNASSIGNED_DOMAIN_ID,
        }
    }

    /// 某领域下的节点（声明顺序；含保留节点）。
    pub fn nodes_of_domain(&self, domain_id: &str) -> Vec<&DesignNode> {
        self.nodes
            .iter()
            .filter(|node| node.domain_id == domain_id)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrganizationViolation {
    pub code: String,
    pub message: String,
}

/// 组织维度校验（进 `space validate` 违规清单）：
/// 1. 领域/节点 id 不得重复，不得占用保留 id；
/// 2. 节点的 `domain_id` 必须是已声明领域；
/// 3. 决策点若声明 `node_id`，该节点必须存在。
pub fn validate_organization(
    declared_domains: &[DesignDomain],
    declared_nodes: &[DesignNode],
    graph: &DecisionGraph,
) -> Vec<OrganizationViolation> {
    let mut violations = Vec::new();

    let mut domain_ids: BTreeSet<&str> = BTreeSet::new();
    for domain in declared_domains {
        if domain.id == UNASSIGNED_DOMAIN_ID {
            violations.push(OrganizationViolation {
                code: "domain.reserved_id".into(),
                message: format!(
                    "领域 id {UNASSIGNED_DOMAIN_ID} 是保留领域「{UNASSIGNED_DOMAIN_NAME}」，清单不得声明"
                ),
            });
            continue;
        }
        if domain.name.trim().is_empty() {
            violations.push(OrganizationViolation {
                code: "domain.empty_name".into(),
                message: format!("领域 {} 缺少展示名", domain.id),
            });
        }
        if !domain_ids.insert(domain.id.as_str()) {
            violations.push(OrganizationViolation {
                code: "domain.duplicate_id".into(),
                message: format!("领域 id {} 重复声明", domain.id),
            });
        }
    }

    let mut node_ids: BTreeSet<&str> = BTreeSet::new();
    for node in declared_nodes {
        if node.id == UNASSIGNED_NODE_ID {
            violations.push(OrganizationViolation {
                code: "node.reserved_id".into(),
                message: format!(
                    "节点 id {UNASSIGNED_NODE_ID} 是保留节点「{UNASSIGNED_NODE_NAME}」，清单不得声明"
                ),
            });
            continue;
        }
        if node.name.trim().is_empty() {
            violations.push(OrganizationViolation {
                code: "node.empty_name".into(),
                message: format!("节点 {} 缺少展示名", node.id),
            });
        }
        if !node_ids.insert(node.id.as_str()) {
            violations.push(OrganizationViolation {
                code: "node.duplicate_id".into(),
                message: format!("节点 id {} 重复声明", node.id),
            });
        }
        if !domain_ids.contains(node.domain_id.as_str()) {
            violations.push(OrganizationViolation {
                code: "node.dangling_domain".into(),
                message: format!(
                    "节点 {} 归属的领域 {} 未声明（领域必须先在通用层声明）",
                    node.id, node.domain_id
                ),
            });
        }
    }

    for point in graph.points() {
        if let Some(node_id) = &point.node_id
            && !node_ids.contains(node_id.as_str())
        {
            violations.push(OrganizationViolation {
                code: "point.dangling_node".into(),
                message: format!(
                    "决策点 {} 的 node_id {} 不是任何已声明节点（留空则归入保留领域「{UNASSIGNED_DOMAIN_NAME}」）",
                    point.id, node_id
                ),
            });
        }
    }

    violations
}

// ---------------------------------------------------------------------------
// 聚合查询（UI DTO）
// ---------------------------------------------------------------------------

/// 一个聚合单元的进度计数：`applicable` 为分母（不含 N/A 与未激活点）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProgressCounts {
    /// 已确认（`confirmed_by_user`）且适用的决策点数。
    pub confirmed: usize,
    /// 适用（Active）决策点数 = 完成度分母。
    pub applicable: usize,
    /// 显式 N/A 的决策点数（不进分母，但要在案可见）。
    pub not_applicable: usize,
    /// 该单元下的决策点总数（含未激活/超深度档，供「这个领域一共多少事」展示）。
    pub total_points: usize,
}

impl ProgressCounts {
    pub fn percent(&self) -> u8 {
        if self.applicable == 0 {
            100
        } else {
            ((self.confirmed as f32 / self.applicable as f32) * 100.0).round() as u8
        }
    }

    pub fn is_complete(&self) -> bool {
        self.confirmed == self.applicable
    }
}

/// 领域进度卡片（左栏 16 领域总览的数据源）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainProgress {
    pub domain_id: DomainId,
    pub name: String,
    pub description: String,
    pub order: u32,
    /// 该域下有决策点的节点数。
    pub node_count: usize,
    pub counts: ProgressCounts,
    pub percent: u8,
}

/// 节点进度（中栏节点列表的数据源）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeProgress {
    pub node_id: NodeId,
    pub domain_id: DomainId,
    pub name: String,
    pub description: String,
    pub role_class: String,
    pub counts: ProgressCounts,
    pub percent: u8,
    /// 该节点下的决策点 id（清单声明顺序）。
    pub decision_ids: Vec<DecisionId>,
}

/// 组织维度聚合结果：领域卡片 + 节点列表（都只列出「有决策点」的单元）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrganizationProgress {
    pub domains: Vec<DomainProgress>,
    pub nodes: Vec<NodeProgress>,
    /// 全局计数（等于各领域求和），与 `CompletenessReport` 的 done/total 同源同义。
    pub total: ProgressCounts,
}

impl OrganizationProgress {
    pub fn domain(&self, domain_id: &str) -> Option<&DomainProgress> {
        self.domains.iter().find(|item| item.domain_id == domain_id)
    }

    pub fn node(&self, node_id: &str) -> Option<&NodeProgress> {
        self.nodes.iter().find(|item| item.node_id == node_id)
    }

    /// 某领域下的节点进度（顺序同 `nodes`）。
    pub fn nodes_of_domain(&self, domain_id: &str) -> Vec<&NodeProgress> {
        self.nodes
            .iter()
            .filter(|item| item.domain_id == domain_id)
            .collect()
    }
}

/// 按领域/节点聚合进度。
///
/// 计数口径与 `compute_completeness` 一致（Active 进分母、`confirmed_by_user` 才算完成、
/// N/A 单列），因此领域进度求和恒等于全局完成度分子分母，不引入第二套算法。
/// 注意：领域进度只看「点是否已确认」，不看参数是否填齐——参数级缺项由完成度的
/// `blocking` 清单负责（右栏「缺失项」页签），两者互补不重复。
pub fn aggregate_progress(
    graph: &DecisionGraph,
    organization: &DesignOrganization,
    selections: &BTreeMap<DecisionId, Selection>,
    applicability: &ApplicabilityMap,
) -> OrganizationProgress {
    let mut per_node: BTreeMap<&str, (ProgressCounts, Vec<DecisionId>)> = BTreeMap::new();

    for point in graph.points() {
        let node_id = organization.effective_node_id(point.node_id.as_deref());
        let entry = per_node.entry(node_id).or_default();
        entry.0.total_points += 1;
        entry.1.push(point.id.clone());
        match applicability.get(&point.id) {
            Some(PointApplicability::Active) => {
                entry.0.applicable += 1;
                if selections
                    .get(&point.id)
                    .is_some_and(|selection| selection.confirmed_by_user)
                {
                    entry.0.confirmed += 1;
                }
            }
            Some(PointApplicability::NotApplicable(_)) => entry.0.not_applicable += 1,
            _ => {}
        }
    }

    let mut nodes = Vec::new();
    let mut per_domain: BTreeMap<&str, ProgressCounts> = BTreeMap::new();
    let mut domain_node_counts: BTreeMap<&str, usize> = BTreeMap::new();
    // 节点顺序 = 组织装配顺序（清单声明序，保留节点最后），只列出有决策点的节点。
    for node in organization.nodes() {
        let Some((counts, decision_ids)) = per_node.get(node.id.as_str()) else {
            continue;
        };
        let domain_id = organization.domain_of_node(&node.id);
        let domain_counts = per_domain.entry(domain_id).or_default();
        domain_counts.confirmed += counts.confirmed;
        domain_counts.applicable += counts.applicable;
        domain_counts.not_applicable += counts.not_applicable;
        domain_counts.total_points += counts.total_points;
        *domain_node_counts.entry(domain_id).or_default() += 1;
        nodes.push(NodeProgress {
            node_id: node.id.clone(),
            domain_id: domain_id.to_string(),
            name: node.name.clone(),
            description: node.description.clone(),
            role_class: node.role_class.clone(),
            counts: *counts,
            percent: counts.percent(),
            decision_ids: decision_ids.clone(),
        });
    }

    let mut total = ProgressCounts::default();
    let mut domains = Vec::new();
    for domain in organization.domains() {
        let Some(counts) = per_domain.get(domain.id.as_str()) else {
            continue;
        };
        total.confirmed += counts.confirmed;
        total.applicable += counts.applicable;
        total.not_applicable += counts.not_applicable;
        total.total_points += counts.total_points;
        domains.push(DomainProgress {
            domain_id: domain.id.clone(),
            name: domain.name.clone(),
            description: domain.description.clone(),
            order: domain.order,
            node_count: domain_node_counts
                .get(domain.id.as_str())
                .copied()
                .unwrap_or_default(),
            counts: *counts,
            percent: counts.percent(),
        });
    }

    OrganizationProgress {
        domains,
        nodes,
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::applicability::compute_applicability;
    use crate::types::{
        DecisionOption, DecisionPoint, DepthProfile, DesignLevel, GenreScope, ParameterValues,
        PointRequirement, Provenance, SelectionMode,
    };

    fn point(id: &str, node_id: Option<&str>, level: DesignLevel) -> DecisionPoint {
        DecisionPoint {
            id: id.into(),
            domain: "legacy".into(),
            level,
            genre_scope: GenreScope::Universal,
            question: "q".into(),
            mda_layer: None,
            design_question: None,
            node_id: node_id.map(Into::into),
            selection_mode: SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            options: vec![
                DecisionOption {
                    id: "a".into(),
                    label: "甲".into(),
                    ..Default::default()
                },
                DecisionOption {
                    id: "b".into(),
                    label: "乙".into(),
                    ..Default::default()
                },
            ],
            skin_fields: Vec::new(),
            evidence_slots: false,
        }
    }

    fn confirmed(decision: &str) -> (DecisionId, Selection) {
        (
            decision.into(),
            Selection {
                decision_id: decision.into(),
                option_id: "a".into(),
                parameters: ParameterValues::None,
                rationale: String::new(),
                provenance: Provenance::UserManual,
                confirmed_by_user: true,
                template_original: None,
                additional_options: Vec::new(),
                primary_option: None,
            },
        )
    }

    fn organization() -> DesignOrganization {
        DesignOrganization::new(
            vec![
                DesignDomain {
                    id: "gameplay".into(),
                    name: "玩法系统设计".into(),
                    description: String::new(),
                    order: 3,
                },
                DesignDomain {
                    id: "positioning".into(),
                    name: "立项与产品定位设计".into(),
                    description: String::new(),
                    order: 1,
                },
            ],
            vec![
                DesignNode {
                    id: "vision".into(),
                    domain_id: "positioning".into(),
                    name: "项目愿景".into(),
                    description: String::new(),
                    role_class: "strategic".into(),
                },
                DesignNode {
                    id: "input_control".into(),
                    domain_id: "gameplay".into(),
                    name: "输入与控制".into(),
                    description: String::new(),
                    role_class: "system_concrete".into(),
                },
            ],
        )
    }

    #[test]
    fn domains_are_ordered_and_reserved_domain_is_appended() {
        let organization = organization();
        let ids: Vec<&str> = organization
            .domains()
            .iter()
            .map(|domain| domain.id.as_str())
            .collect();
        assert_eq!(ids, vec!["positioning", "gameplay", UNASSIGNED_DOMAIN_ID]);
        assert_eq!(organization.domain_of_node("input_control"), "gameplay");
        assert_eq!(
            organization.domain_of_node(UNASSIGNED_NODE_ID),
            UNASSIGNED_DOMAIN_ID
        );
    }

    #[test]
    fn points_without_node_fall_into_reserved_domain() {
        let organization = organization();
        let graph = DecisionGraph::new(vec![
            point("u.vision", Some("vision"), DesignLevel::L0),
            point("u.legacy", None, DesignLevel::L1),
        ])
        .unwrap();
        let selections: BTreeMap<_, _> = [confirmed("u.legacy")].into_iter().collect();
        let applicability = compute_applicability(
            &graph,
            &selections,
            &BTreeMap::new(),
            DepthProfile::new(DesignLevel::L4).unwrap(),
        );
        let progress = aggregate_progress(&graph, &organization, &selections, &applicability);

        let reserved = progress
            .domain(UNASSIGNED_DOMAIN_ID)
            .expect("保留领域应出现");
        assert_eq!(reserved.counts.applicable, 1);
        assert_eq!(reserved.counts.confirmed, 1);
        assert_eq!(reserved.percent, 100);
        let positioning = progress.domain("positioning").expect("声明领域应出现");
        assert_eq!(positioning.counts.applicable, 1);
        assert_eq!(positioning.counts.confirmed, 0);
        assert_eq!(positioning.percent, 0);
        // 空领域（无决策点）不进卡片列表。
        assert!(progress.domain("gameplay").is_none());
        // 全局计数 = 各领域求和。
        assert_eq!(progress.total.applicable, 2);
        assert_eq!(progress.total.confirmed, 1);
        let reserved_node = progress.node(UNASSIGNED_NODE_ID).expect("保留节点应出现");
        assert_eq!(reserved_node.decision_ids, vec!["u.legacy".to_string()]);
    }

    #[test]
    fn dangling_node_and_domain_references_are_violations() {
        let graph = DecisionGraph::new(vec![point("u.x", Some("ghost_node"), DesignLevel::L0)])
            .expect("图构造");
        let violations = validate_organization(
            &[DesignDomain {
                id: "positioning".into(),
                name: "立项".into(),
                description: String::new(),
                order: 1,
            }],
            &[DesignNode {
                id: "vision".into(),
                domain_id: "ghost_domain".into(),
                name: "愿景".into(),
                description: String::new(),
                role_class: String::new(),
            }],
            &graph,
        );
        assert!(
            violations
                .iter()
                .any(|item| item.code == "node.dangling_domain"
                    && item.message.contains("ghost_domain")),
            "{violations:?}"
        );
        assert!(
            violations
                .iter()
                .any(|item| item.code == "point.dangling_node"
                    && item.message.contains("ghost_node")),
            "{violations:?}"
        );
    }

    #[test]
    fn reserved_ids_cannot_be_declared() {
        let graph = DecisionGraph::new(vec![point("u.x", None, DesignLevel::L0)]).expect("图构造");
        let violations = validate_organization(
            &[DesignDomain {
                id: UNASSIGNED_DOMAIN_ID.into(),
                name: "冒名".into(),
                description: String::new(),
                order: 0,
            }],
            &[DesignNode {
                id: UNASSIGNED_NODE_ID.into(),
                domain_id: UNASSIGNED_DOMAIN_ID.into(),
                name: "冒名".into(),
                description: String::new(),
                role_class: String::new(),
            }],
            &graph,
        );
        assert!(
            violations
                .iter()
                .any(|item| item.code == "domain.reserved_id")
        );
        assert!(
            violations
                .iter()
                .any(|item| item.code == "node.reserved_id")
        );
    }
}
