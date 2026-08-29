use adm4_contracts::CardinalityRange;
use adm4_decision::{DecisionGraph, DecisionId, DecisionPoint, GenrePackId, RowReference};
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

/// 通用层清单（L0-L2 全部 + 跨品类决策点）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UniversalLayer {
    pub space_version: String,
    pub decision_points: Vec<DecisionPoint>,
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

/// 装配后的设计空间：通用层 + 一个品类包 → 一张决策图。
#[derive(Debug, Clone)]
pub struct DesignSpace {
    pub universal_version: String,
    pub pack: GenrePack,
    pub graph: DecisionGraph,
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
