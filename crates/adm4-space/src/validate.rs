use crate::model::{ConsistencyRuleKind, DesignSpace};
use adm4_decision::{
    AxisRef, DecisionPoint, DesignLevel, GenreScope, ParameterSchema, validate_graph,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceViolation {
    pub code: String,
    pub message: String,
}

/// 设计空间校验规则（设计 01 号文档 §3.3）。
pub fn validate_design_space(
    space: &DesignSpace,
    universal_points: &[DecisionPoint],
) -> Vec<SpaceViolation> {
    let mut violations = Vec::new();

    // 1. DAG 合法。
    for graph_violation in validate_graph(&space.graph) {
        violations.push(SpaceViolation {
            code: format!("graph.{}", graph_violation.code),
            message: graph_violation.message,
        });
    }

    // 2. 参考游戏 ≥3。
    if space.pack.reference_games.len() < 3 {
        violations.push(SpaceViolation {
            code: "pack.too_few_references".into(),
            message: format!(
                "品类包 {} 只有 {} 款参考游戏，硬要求 ≥3",
                space.pack.pack_id,
                space.pack.reference_games.len()
            ),
        });
    }

    // 3. 品类包决策点层级 ∈ L3..=L6，genre_scope 必须指向本包。
    for point in &space.pack.decision_points {
        if point.level < DesignLevel::L3 {
            violations.push(SpaceViolation {
                code: "pack.level_out_of_range".into(),
                message: format!(
                    "品类包决策点 {} 层级 {:?} 低于 L3（L0-L2 属通用层）",
                    point.id, point.level
                ),
            });
        }
        match &point.genre_scope {
            GenreScope::Pack(pack_id) if pack_id == &space.pack.pack_id => {}
            other => violations.push(SpaceViolation {
                code: "pack.wrong_scope".into(),
                message: format!(
                    "品类包决策点 {} 的 genre_scope {:?} 未指向本包 {}",
                    point.id, other, space.pack.pack_id
                ),
            }),
        }
    }

    // 4. 通用层必须覆盖 L0-L2 三层。
    for level in [DesignLevel::L0, DesignLevel::L1, DesignLevel::L2] {
        if !universal_points.iter().any(|point| point.level == level) {
            violations.push(SpaceViolation {
                code: "universal.missing_level".into(),
                message: format!("通用层缺少 {} 决策点", level.label()),
            });
        }
    }
    for point in universal_points {
        if !matches!(point.genre_scope, GenreScope::Universal) {
            violations.push(SpaceViolation {
                code: "universal.wrong_scope".into(),
                message: format!("通用层决策点 {} 的 genre_scope 必须是 universal", point.id),
            });
        }
    }

    // 5. Table/Matrix 的 cardinality_key 必须有期望区间；轴引用合法。
    for point in space.graph.points() {
        for option in &point.options {
            match &option.parameter_schema {
                ParameterSchema::Table(table) => {
                    if !space
                        .pack
                        .cardinality_expectations
                        .contains_key(&table.cardinality_key)
                        && matches!(point.genre_scope, GenreScope::Pack(_))
                    {
                        violations.push(SpaceViolation {
                            code: "cardinality.missing_expectation".into(),
                            message: format!(
                                "{}/{} 的表基数键 {} 在 cardinality_expectations 无对应区间",
                                point.id, option.id, table.cardinality_key
                            ),
                        });
                    }
                }
                ParameterSchema::Matrix(matrix) => {
                    for axis in [&matrix.row_axis, &matrix.col_axis] {
                        let target = match axis {
                            AxisRef::DecisionOptions { decision } => decision,
                            AxisRef::TableRows { decision } => decision,
                        };
                        if space.graph.point(target).is_none() {
                            violations.push(SpaceViolation {
                                code: "matrix.dangling_axis".into(),
                                message: format!(
                                    "{}/{} 的矩阵轴引用了不存在的决策点 {target}",
                                    point.id, option.id
                                ),
                            });
                        } else if let AxisRef::TableRows { decision } = axis {
                            let has_table = space.graph.point(decision).is_some_and(|axis_point| {
                                axis_point.options.iter().any(|axis_option| {
                                    matches!(
                                        axis_option.parameter_schema,
                                        ParameterSchema::Table(_)
                                    )
                                })
                            });
                            if !has_table {
                                violations.push(SpaceViolation {
                                    code: "matrix.axis_not_table".into(),
                                    message: format!(
                                        "{}/{} 的矩阵行/列轴 {decision} 不含表结构选项",
                                        point.id, option.id
                                    ),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // 6. 一致性规则引用的决策点存在；跨表外键规则的列引用也必须落在某个表结构选项上。
    for rule in &space.pack.consistency_rules {
        let referenced: Vec<&str> = match &rule.kind {
            ConsistencyRuleKind::MatrixAxisMatchesTableRows {
                matrix_decision,
                table_decision,
            } => vec![matrix_decision, table_decision],
            ConsistencyRuleKind::AnsweredTogether { first, second } => vec![first, second],
            ConsistencyRuleKind::RowReference {
                source_decision,
                target_decision,
                ..
            } => vec![source_decision, target_decision],
        };
        let mut dangling_point = false;
        for id in referenced {
            if space.graph.point(id).is_none() {
                dangling_point = true;
                violations.push(SpaceViolation {
                    code: "rule.dangling_reference".into(),
                    message: format!("一致性规则 {} 引用了不存在的决策点 {id}", rule.id),
                });
            }
        }
        if dangling_point {
            continue; // 决策点都不存在，列检查无从谈起。
        }
        if let ConsistencyRuleKind::RowReference {
            source_decision,
            source_column,
            target_decision,
            target_key_column,
        } = &rule.kind
        {
            for (decision, column, role, code) in [
                (
                    source_decision,
                    source_column,
                    "源列",
                    "rule.unknown_source_column",
                ),
                (
                    target_decision,
                    target_key_column,
                    "目标键列",
                    "rule.unknown_target_key_column",
                ),
            ] {
                if !decision_has_table_column(space, decision, column) {
                    violations.push(SpaceViolation {
                        code: code.into(),
                        message: format!(
                            "跨表外键规则 {} 的{} {column} 在决策点 {decision} 的任何表结构选项中都不存在",
                            rule.id, role
                        ),
                    });
                }
            }
        }
    }

    violations
}

/// 决策点是否存在「含指定列的表结构选项」（外键规则的列引用合法性依据）。
fn decision_has_table_column(space: &DesignSpace, decision: &str, column: &str) -> bool {
    space.graph.point(decision).is_some_and(|point| {
        point
            .options
            .iter()
            .any(|option| match &option.parameter_schema {
                ParameterSchema::Table(table) => {
                    table.columns.iter().any(|field| field.key == column)
                }
                _ => false,
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ConsistencyRule, GenrePack};
    use adm4_contracts::{CardinalityRange, ValueKind};
    use adm4_decision::{
        DecisionGraph, DecisionOption, PointRequirement, ScalarField, TableSchema,
    };

    fn universal_point(id: &str, level: DesignLevel) -> DecisionPoint {
        DecisionPoint {
            id: id.into(),
            domain: "core".into(),
            level,
            genre_scope: GenreScope::Universal,
            question: "q".into(),
            mda_layer: None,
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

    fn table_point(id: &str, columns: &[&str], row_key: &str) -> DecisionPoint {
        DecisionPoint {
            id: id.into(),
            domain: "core".into(),
            level: DesignLevel::L5,
            genre_scope: GenreScope::Pack("demo".into()),
            question: "q".into(),
            mda_layer: None,
            requirement: PointRequirement::Unlocked,
            options: vec![DecisionOption {
                id: "rows".into(),
                label: "表".into(),
                parameter_schema: ParameterSchema::Table(TableSchema {
                    columns: columns
                        .iter()
                        .map(|key| ScalarField {
                            key: (*key).to_string(),
                            kind: ValueKind::Text,
                            constraint: None,
                            required: true,
                            is_skin: false,
                        })
                        .collect(),
                    row_key: row_key.into(),
                    cardinality_key: "rows".into(),
                }),
                ..Default::default()
            }],
            skin_fields: Vec::new(),
            evidence_slots: false,
        }
    }

    /// 组装一份最小设计空间：通用层三点 + 两张表 + 给定的一致性规则。
    fn space_with_rules(rules: Vec<ConsistencyRule>) -> (DesignSpace, Vec<DecisionPoint>) {
        let universal = vec![
            universal_point("u.a", DesignLevel::L0),
            universal_point("u.b", DesignLevel::L1),
            universal_point("u.c", DesignLevel::L2),
        ];
        let pack_points = vec![
            table_point("demo.stages", &["stage_id"], "stage_id"),
            table_point("demo.waves", &["row_id", "stage_id"], "row_id"),
        ];
        let mut all = universal.clone();
        all.extend(pack_points.clone());
        let graph = match DecisionGraph::new(all) {
            Ok(graph) => graph,
            Err(error) => panic!("测试图构造失败：{}", error.message),
        };
        let pack = GenrePack {
            pack_id: "demo".into(),
            pack_version: "0.1.0".into(),
            display_name: "演示包".into(),
            reference_games: vec!["甲".into(), "乙".into(), "丙".into()],
            cardinality_expectations: [("rows".to_string(), CardinalityRange { min: 1, max: 9 })]
                .into_iter()
                .collect(),
            consistency_rules: rules,
            decision_points: pack_points,
        };
        (
            DesignSpace {
                universal_version: "0.1.0".into(),
                pack,
                graph,
            },
            universal,
        )
    }

    fn row_reference_rule(
        id: &str,
        source_column: &str,
        target_key_column: &str,
    ) -> ConsistencyRule {
        ConsistencyRule {
            id: id.into(),
            kind: ConsistencyRuleKind::RowReference {
                source_decision: "demo.waves".into(),
                source_column: source_column.into(),
                target_decision: "demo.stages".into(),
                target_key_column: target_key_column.into(),
            },
        }
    }

    #[test]
    fn well_formed_row_reference_rule_passes() {
        let (space, universal) = space_with_rules(vec![row_reference_rule(
            "waves_reference_stages",
            "stage_id",
            "stage_id",
        )]);
        let violations = validate_design_space(&space, &universal);
        assert!(violations.is_empty(), "{violations:?}");
    }

    #[test]
    fn row_reference_rule_with_unknown_columns_is_rejected() {
        let (space, universal) = space_with_rules(vec![
            row_reference_rule("bad_source", "ghost_column", "stage_id"),
            row_reference_rule("bad_target", "stage_id", "ghost_key"),
        ]);
        let violations = validate_design_space(&space, &universal);
        let source_violation = violations
            .iter()
            .find(|violation| violation.code == "rule.unknown_source_column")
            .expect("源列不存在必须报违规");
        assert!(
            source_violation.message.contains("ghost_column"),
            "{source_violation:?}"
        );
        let target_violation = violations
            .iter()
            .find(|violation| violation.code == "rule.unknown_target_key_column")
            .expect("目标键列不存在必须报违规");
        assert!(
            target_violation.message.contains("ghost_key"),
            "{target_violation:?}"
        );
    }

    #[test]
    fn row_reference_rule_with_unknown_decision_is_rejected() {
        let (space, universal) = space_with_rules(vec![ConsistencyRule {
            id: "ghost_rule".into(),
            kind: ConsistencyRuleKind::RowReference {
                source_decision: "demo.ghost".into(),
                source_column: "stage_id".into(),
                target_decision: "demo.stages".into(),
                target_key_column: "stage_id".into(),
            },
        }]);
        let violations = validate_design_space(&space, &universal);
        assert!(
            violations
                .iter()
                .any(|violation| violation.code == "rule.dangling_reference"
                    && violation.message.contains("demo.ghost")),
            "{violations:?}"
        );
    }
}
