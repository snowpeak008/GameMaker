use adm4_contracts::CardinalityRange;
use adm4_decision::{
    DecisionGraph, DecisionId, DecisionPoint, DesignDomain, DesignNode, DesignOrganization,
    GenrePackId, RowReference,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 品类包跨决策一致性规则（冻结门第 2 道消费）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsistencyRule {
    pub id: String,
    #[serde(flatten)]
    pub kind: ConsistencyRuleKind,
}

impl ConsistencyRule {
    /// 跨表外键规则的机器化形态；其它规则类型返回 None。
    pub fn as_row_reference(&self) -> Option<RowReference> {
        match &self.kind {
            ConsistencyRuleKind::RowReference {
                source_decision,
                source_column,
                target_decision,
                target_key_column,
            } => Some(RowReference {
                rule_id: self.id.clone(),
                source_decision: source_decision.clone(),
                source_column: source_column.clone(),
                target_decision: target_decision.clone(),
                target_key_column: target_key_column.clone(),
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsistencyRuleKind {
    /// 矩阵行轴集合必须与某表的行集合一致。
    MatrixAxisMatchesTableRows {
        matrix_decision: DecisionId,
        table_decision: DecisionId,
    },
    /// 两个决策点必须同时被回答或同时不适用。
    AnsweredTogether {
        first: DecisionId,
        second: DecisionId,
    },
    /// 跨表外键：源表某列的取值必须落在目标表行键列的取值集合内。
    /// 旧包没有这种规则，新增枚举分支不影响既有清单的反序列化。
    RowReference {
        source_decision: DecisionId,
        source_column: String,
        target_decision: DecisionId,
        target_key_column: String,
    },
}

/// 通用层清单（L0-L2 全部 + 跨品类决策点 + 跨品类的领域/节点组织维度）。
///
/// 通用层目录下的每个 `*.json` 都是一份本结构，按文件名排序合并：
/// `space_version` 必须一致，`decision_points` / `domains` / `nodes` 三者各自累加。
/// 因此 T10 可以把 16 个领域与其节点单独放一个 `domains.json`（只带 domains/nodes），
/// 也可以并进 `core.json`——加载结果相同。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UniversalLayer {
    pub space_version: String,
    /// 只声明组织维度的文件可以省略该键。
    #[serde(default)]
    pub decision_points: Vec<DecisionPoint>,
    /// 设计领域：跨品类通用，只在通用层声明（品类包不得声明领域）。
    #[serde(default)]
    pub domains: Vec<DesignDomain>,
    /// 通用设计节点。
    #[serde(default)]
    pub nodes: Vec<DesignNode>,
}

/// 品类包。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenrePack {
    pub pack_id: GenrePackId,
    pub pack_version: String,
    pub display_name: String,
    /// ≥3 参考游戏名（硬要求；同时登记进换皮词表）。
    pub reference_games: Vec<String>,
    #[serde(default)]
    pub cardinality_expectations: BTreeMap<String, CardinalityRange>,
    #[serde(default)]
    pub consistency_rules: Vec<ConsistencyRule>,
    /// 品类专属设计节点：`domain_id` 必须指向通用层已声明的领域
    /// （领域跨品类通用，节点可以是本品类独有的）。
    #[serde(default)]
    pub nodes: Vec<DesignNode>,
    pub decision_points: Vec<DecisionPoint>,
}

impl GenrePack {
    /// 本包声明的全部跨表外键规则（喂完成度与冻结门第 2 道）。
    pub fn row_references(&self) -> Vec<RowReference> {
        self.consistency_rules
            .iter()
            .filter_map(ConsistencyRule::as_row_reference)
            .collect()
    }
}

/// 装配后的设计空间：通用层 + 一个品类包 → 一张决策图 + 一套领域/节点组织。
#[derive(Debug, Clone)]
pub struct DesignSpace {
    pub universal_version: String,
    pub pack: GenrePack,
    pub graph: DecisionGraph,
    /// 横向组织维度（领域 → 节点 → 决策点），含内置保留领域「未分域」。
    pub organization: DesignOrganization,
}

impl DesignSpace {
    pub fn cardinality(&self) -> &BTreeMap<String, CardinalityRange> {
        &self.pack.cardinality_expectations
    }

    /// 品类包声明的跨表外键规则。
    pub fn row_references(&self) -> Vec<RowReference> {
        self.pack.row_references()
    }

    /// 换皮词表贡献（品类包参考游戏名）。
    pub fn skin_words(&self) -> Vec<String> {
        self.pack.reference_games.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_decision::{DesignLevel, SelectionMode};

    /// 组织维度的清单写法（T10 迁移格式基准）：
    /// 通用层可以有一个只带 `domains`/`nodes` 的文件，不必声明决策点。
    const DOMAINS_JSON: &str = r#"{
      "space_version": "0.1.0",
      "domains": [
        { "id": "product_positioning_design", "name": "立项与产品定位设计",
          "description": "确认项目愿景、目标用户、市场定位。", "order": 1 },
        { "id": "gameplay_system_design", "name": "玩法系统设计", "order": 3 }
      ],
      "nodes": [
        { "id": "vision_decision", "domain_id": "product_positioning_design",
          "name": "项目愿景决策", "description": "明确项目要兑现什么。",
          "role_class": "strategic" },
        { "id": "input_control_decision", "domain_id": "gameplay_system_design",
          "name": "输入与控制决策", "role_class": "system_concrete" }
      ]
    }"#;

    /// 决策点上的新键：`node_id` / `selection_mode` / `design_question` / `mda_layer`。
    const MULTI_POINT_JSON: &str = r#"{
      "space_version": "0.1.0",
      "decision_points": [
        {
          "id": "pp.core_feeling_type",
          "domain": "profile",
          "level": "L1",
          "genre_scope": "universal",
          "node_id": "vision_decision",
          "question": "项目必须兑现哪些核心感受？",
          "design_question": "玩家最终会反复获得哪种核心感受？",
          "mda_layer": "aesthetics",
          "selection_mode": { "mode": "multi", "allow_primary": true },
          "options": [
            { "id": "tense_choice", "label": "紧张抉择" },
            { "id": "growth_accumulation", "label": "成长积累" }
          ]
        }
      ]
    }"#;

    #[test]
    fn organization_only_universal_file_parses() {
        let layer: UniversalLayer = serde_json::from_str(DOMAINS_JSON).expect("领域清单应可解析");
        assert!(layer.decision_points.is_empty());
        assert_eq!(layer.domains.len(), 2);
        assert_eq!(layer.domains[1].order, 3);
        // description 可省略 → 空串（展示层自行处理）。
        assert!(layer.domains[1].description.is_empty());
        assert_eq!(layer.nodes.len(), 2);
        assert_eq!(layer.nodes[0].domain_id, "product_positioning_design");
        assert_eq!(layer.nodes[1].role_class, "system_concrete");
    }

    #[test]
    fn multi_selection_point_keys_parse() {
        let layer: UniversalLayer =
            serde_json::from_str(MULTI_POINT_JSON).expect("多选决策点应可解析");
        let point = &layer.decision_points[0];
        assert_eq!(point.node_id.as_deref(), Some("vision_decision"));
        assert_eq!(
            point.design_question.as_deref(),
            Some("玩家最终会反复获得哪种核心感受？")
        );
        assert_eq!(
            point.selection_mode,
            SelectionMode::Multi {
                allow_primary: true
            }
        );
        assert!(point.requires_primary());
    }

    /// 既有清单（无任何新键）必须原样解析，且默认值等价于扩展前的行为。
    #[test]
    fn legacy_point_without_new_keys_defaults_to_single_and_unassigned() {
        let legacy = r#"{
          "space_version": "0.1.0",
          "decision_points": [
            { "id": "u.platform", "domain": "profile", "level": "L0",
              "genre_scope": "universal", "question": "主平台是什么？",
              "options": [ { "id": "pc_single", "label": "PC 单机" },
                           { "id": "mobile", "label": "移动端" } ] }
          ]
        }"#;
        let layer: UniversalLayer = serde_json::from_str(legacy).expect("旧清单应原样解析");
        let point = &layer.decision_points[0];
        assert_eq!(point.level, DesignLevel::L0);
        assert!(point.node_id.is_none());
        assert!(point.design_question.is_none());
        assert_eq!(point.selection_mode, SelectionMode::Single);
        assert!(!point.is_multi());
        assert!(layer.domains.is_empty());
        assert!(layer.nodes.is_empty());
    }

    /// 品类包可以声明品类专属节点（领域仍只在通用层声明）。
    #[test]
    fn pack_may_declare_nodes() {
        let pack_json = r#"{
          "pack_id": "lane_defense",
          "pack_version": "0.1.0",
          "display_name": "通道塔防",
          "reference_games": ["虚构甲", "虚构乙", "虚构丙"],
          "nodes": [
            { "id": "ld_wave_decision", "domain_id": "gameplay_system_design",
              "name": "波次编排决策", "role_class": "content_concrete" }
          ],
          "decision_points": []
        }"#;
        let pack: GenrePack = serde_json::from_str(pack_json).expect("品类包应可解析");
        assert_eq!(pack.nodes.len(), 1);
        assert_eq!(pack.nodes[0].domain_id, "gameplay_system_design");
    }
}
