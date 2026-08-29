use crate::types::{DecisionId, DecisionPoint, ParameterSchema};
use adm4_foundation::{Adm4Error, Adm4Result};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// 决策图：决策点集合 + 索引。
#[derive(Debug, Clone, Default)]
pub struct DecisionGraph {
    points: Vec<DecisionPoint>,
    index: BTreeMap<DecisionId, usize>,
}

impl DecisionGraph {
    pub fn new(points: Vec<DecisionPoint>) -> Adm4Result<Self> {
        let mut index = BTreeMap::new();
        for (position, point) in points.iter().enumerate() {
            if index.insert(point.id.clone(), position).is_some() {
                return Err(Adm4Error::validation(format!(
                    "duplicate decision point id: {}",
                    point.id
                )));
            }
        }
        Ok(Self { points, index })
    }

    pub fn points(&self) -> &[DecisionPoint] {
        &self.points
    }

    pub fn point(&self, id: &str) -> Option<&DecisionPoint> {
        self.index.get(id).map(|position| &self.points[*position])
    }

    pub fn require_point(&self, id: &str) -> Adm4Result<&DecisionPoint> {
        self.point(id)
            .ok_or_else(|| Adm4Error::not_found(format!("unknown decision point: {id}")))
    }

    /// Kahn 拓扑序（unlocks 边：父 → 子）。图有环时返回错误。
    pub fn topological_order(&self) -> Adm4Result<Vec<DecisionId>> {
        let mut in_degree: BTreeMap<&str, usize> = self
            .points
            .iter()
            .map(|point| (point.id.as_str(), 0))
            .collect();
        let mut edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for point in &self.points {
            for option in &point.options {
                for unlocked in &option.unlocks {
                    if let Some(degree) = in_degree.get_mut(unlocked.as_str()) {
                        // 同一父点多个选项 unlock 同一子点只计一条边。
                        let entry = edges.entry(point.id.as_str()).or_default();
                        if !entry.contains(&unlocked.as_str()) {
                            entry.push(unlocked.as_str());
                            *degree += 1;
                        }
                    }
                }
            }
        }
        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut order = Vec::new();
        while let Some(current) = queue.pop_front() {
            order.push(current.to_string());
            if let Some(children) = edges.get(current) {
                for child in children {
                    let degree = in_degree.get_mut(child).ok_or_else(|| {
                        Adm4Error::internal(format!(
                            "拓扑排序内部不一致：子节点 {child} 未登记入度"
                        ))
                    })?;
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push_back(child);
                    }
                }
            }
        }
        if order.len() != self.points.len() {
            return Err(Adm4Error::validation(
                "decision graph contains a cycle in unlocks edges",
            ));
        }
        Ok(order)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphViolation {
    pub code: String,
    pub message: String,
}

/// 表格型决策点：任一选项以表或矩阵承载参数（L5/L6 结构点）。
fn is_tabular(point: &DecisionPoint) -> bool {
    point.options.iter().any(|option| {
        matches!(
            option.parameter_schema,
            ParameterSchema::Table(_) | ParameterSchema::Matrix(_)
        )
    })
}

/// DAG 校验最小规则集（设计 01 号文档 §2.5）。
pub fn validate_graph(graph: &DecisionGraph) -> Vec<GraphViolation> {
    let mut violations = Vec::new();
    let known: BTreeSet<&str> = graph
        .points()
        .iter()
        .map(|point| point.id.as_str())
        .collect();

    for point in graph.points() {
        if point.options.is_empty() {
            violations.push(GraphViolation {
                code: "empty_options".into(),
                message: format!("决策点 {} 没有任何选项", point.id),
            });
        } else if point.options.len() < 2 && !is_tabular(point) {
            // 工程规范 §4：非表格型决策点必须给出真实的二选一，否则"决策"退化为公告。
            // 表/矩阵结构点靠行列数据承载差异，允许单结构选项。
            violations.push(GraphViolation {
                code: "insufficient_options".into(),
                message: format!(
                    "决策点 {} 只有 1 个选项：非表格型决策点必须提供 ≥2 个真实可选项（工程规范 §4 数据规范）",
                    point.id
                ),
            });
        }
        let mut option_ids = BTreeSet::new();
        for option in &point.options {
            if !option_ids.insert(option.id.as_str()) {
                violations.push(GraphViolation {
                    code: "duplicate_option".into(),
                    message: format!("决策点 {} 存在重复选项 {}", point.id, option.id),
                });
            }
            if option.is_custom && matches!(option.parameter_schema, ParameterSchema::None) {
                violations.push(GraphViolation {
                    code: "custom_without_schema".into(),
                    message: format!(
                        "决策点 {} 的 custom 选项 {} 必须提供结构化 parameter_schema",
                        point.id, option.id
                    ),
                });
            }
            for selector in option.requires.iter().chain(option.conflicts.iter()) {
                match graph.point(&selector.decision) {
                    None => violations.push(GraphViolation {
                        code: "dangling_selector".into(),
                        message: format!(
                            "{}/{} 引用了不存在的决策点 {}",
                            point.id, option.id, selector.decision
                        ),
                    }),
                    Some(target) => {
                        if target.option(&selector.option).is_none() {
                            violations.push(GraphViolation {
                                code: "dangling_selector_option".into(),
                                message: format!(
                                    "{}/{} 引用了 {} 中不存在的选项 {}",
                                    point.id, option.id, selector.decision, selector.option
                                ),
                            });
                        }
                    }
                }
            }
            for unlocked in &option.unlocks {
                if !known.contains(unlocked.as_str()) {
                    violations.push(GraphViolation {
                        code: "dangling_unlock".into(),
                        message: format!(
                            "{}/{} unlock 了不存在的决策点 {}",
                            point.id, option.id, unlocked
                        ),
                    });
                }
            }
        }
    }

    // conflicts 双向对称性。
    for point in graph.points() {
        for option in &point.options {
            for conflict in &option.conflicts {
                let Some(target_point) = graph.point(&conflict.decision) else {
                    continue;
                };
                let Some(target_option) = target_point.option(&conflict.option) else {
                    continue;
                };
                let mirrored = target_option
                    .conflicts
                    .iter()
                    .any(|back| back.decision == point.id && back.option == option.id);
                if !mirrored {
                    violations.push(GraphViolation {
                        code: "asymmetric_conflict".into(),
                        message: format!(
                            "冲突不对称：{}/{} 冲突 {}/{}，但反向未声明",
                            point.id, option.id, conflict.decision, conflict.option
                        ),
                    });
                }
            }
        }
    }

    // 无环。
    if let Err(error) = graph.topological_order() {
        violations.push(GraphViolation {
            code: "cycle".into(),
            message: error.message,
        });
    }

    violations
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        DecisionOption, DesignLevel, GenreScope, OptionSelector, PointRequirement, SelectionMode,
    };

    fn option(id: &str) -> DecisionOption {
        DecisionOption {
            id: id.into(),
            label: id.into(),
            ..Default::default()
        }
    }

    fn point(id: &str, options: Vec<DecisionOption>) -> DecisionPoint {
        DecisionPoint {
            id: id.into(),
            domain: "d".into(),
            level: DesignLevel::L3,
            genre_scope: GenreScope::Universal,
            question: "q".into(),
            mda_layer: None,
            design_question: None,
            node_id: None,
            selection_mode: SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            options,
            skin_fields: Vec::new(),
            evidence_slots: false,
        }
    }

    #[test]
    fn detects_cycle() {
        let mut a_option = option("a1");
        a_option.unlocks = vec!["b".into()];
        let mut b_option = option("b1");
        b_option.unlocks = vec!["a".into()];
        let graph =
            DecisionGraph::new(vec![point("a", vec![a_option]), point("b", vec![b_option])])
                .unwrap();
        assert!(
            validate_graph(&graph)
                .iter()
                .any(|violation| violation.code == "cycle")
        );
    }

    #[test]
    fn detects_asymmetric_conflict_and_dangling_refs() {
        let mut a_option = option("a1");
        a_option.conflicts = vec![OptionSelector {
            decision: "b".into(),
            option: "b1".into(),
        }];
        a_option.unlocks = vec!["ghost".into()];
        let graph = DecisionGraph::new(vec![
            point("a", vec![a_option]),
            point("b", vec![option("b1")]),
        ])
        .unwrap();
        let violations = validate_graph(&graph);
        assert!(violations.iter().any(|v| v.code == "asymmetric_conflict"));
        assert!(violations.iter().any(|v| v.code == "dangling_unlock"));
    }

    #[test]
    fn custom_option_requires_schema() {
        let mut custom = option("custom");
        custom.is_custom = true;
        let graph = DecisionGraph::new(vec![point("a", vec![custom])]).unwrap();
        assert!(
            validate_graph(&graph)
                .iter()
                .any(|violation| violation.code == "custom_without_schema")
        );
    }

    #[test]
    fn single_option_non_tabular_point_is_rejected() {
        let graph = DecisionGraph::new(vec![point("solo", vec![option("only")])]).unwrap();
        let violations = validate_graph(&graph);
        let insufficient = violations
            .iter()
            .find(|violation| violation.code == "insufficient_options")
            .expect("单选项非表格点必须被判违规");
        assert!(insufficient.message.contains("solo"), "{insufficient:?}");

        let graph =
            DecisionGraph::new(vec![point("pair", vec![option("a"), option("b")])]).unwrap();
        assert!(
            !validate_graph(&graph)
                .iter()
                .any(|violation| violation.code == "insufficient_options")
        );
    }

    #[test]
    fn single_option_table_point_is_allowed() {
        use crate::types::{ScalarField, TableSchema};
        use adm4_contracts::ValueKind;

        let mut table_option = option("rows");
        table_option.parameter_schema = ParameterSchema::Table(TableSchema {
            columns: vec![ScalarField {
                key: "id".into(),
                kind: ValueKind::Text,
                constraint: None,
                required: true,
                is_skin: false,
            }],
            row_key: "id".into(),
            cardinality_key: "k".into(),
        });
        let graph = DecisionGraph::new(vec![point("roster", vec![table_option])]).unwrap();
        assert!(
            !validate_graph(&graph)
                .iter()
                .any(|violation| violation.code == "insufficient_options")
        );
    }

    #[test]
    fn topological_order_covers_all_points() {
        let mut root = option("r1");
        root.unlocks = vec!["child".into()];
        let graph = DecisionGraph::new(vec![
            point("root", vec![root]),
            point("child", vec![option("c1")]),
        ])
        .unwrap();
        let order = graph.topological_order().unwrap();
        assert_eq!(order.len(), 2);
        assert!(
            order.iter().position(|id| id == "root").unwrap()
                < order.iter().position(|id| id == "child").unwrap()
        );
    }
}
