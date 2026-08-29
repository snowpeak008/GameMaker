use crate::applicability::{ApplicabilityMap, PointApplicability};
use crate::consistency::{RowReference, row_reference_problems_by_decision};
use crate::graph::DecisionGraph;
use crate::types::{
    AxisRef, DecisionId, DecisionPoint, NaJustification, ParameterSchema, ParameterValues,
    Selection,
};
use adm4_contracts::CardinalityRange;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingItem {
    pub decision_id: DecisionId,
    /// 精确到「哪张表/哪一格/哪个字段」。
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletenessReport {
    pub done: usize,
    pub total: usize,
    pub blocking: Vec<MissingItem>,
    /// baseline 点显式 N/A 的理由码计数（比例过高反馈品类包设计）。
    pub na_reason_counts: BTreeMap<String, usize>,
}

impl CompletenessReport {
    pub fn is_complete(&self) -> bool {
        self.done == self.total && self.blocking.is_empty()
    }

    pub fn percent(&self) -> u8 {
        if self.total == 0 {
            100
        } else {
            ((self.done as f32 / self.total as f32) * 100.0).round() as u8
        }
    }
}

/// 校验一次 Selection 的参数是否按 schema 填齐、类型与约束合法。
/// 返回缺失/违例明细（空 = 通过）。表/矩阵按整表校验，缺格列清单而非默认值（R2）。
pub fn validate_parameters(
    graph: &DecisionGraph,
    selections: &BTreeMap<DecisionId, Selection>,
    point: &DecisionPoint,
    selection: &Selection,
    cardinality: &BTreeMap<String, CardinalityRange>,
) -> Vec<String> {
    let Some(option) = point.option(&selection.option_id) else {
        return vec![format!("选项 {} 不存在", selection.option_id)];
    };
    let mut problems = Vec::new();
    match (&option.parameter_schema, &selection.parameters) {
        (ParameterSchema::None, _) => {}
        (ParameterSchema::Scalar { fields }, ParameterValues::Scalars { entries }) => {
            for field in fields {
                match entries.get(&field.key) {
                    None if field.required => {
                        problems.push(format!("缺少必填参数 {}", field.key));
                    }
                    None => {}
                    Some(value) => {
                        if !value.type_matches(&field.kind) {
                            problems.push(format!("参数 {} 类型不符", field.key));
                        }
                        if let Some(constraint) = &field.constraint
                            && let Err(error) = value.check_constraint(constraint)
                        {
                            problems.push(format!("参数 {}：{}", field.key, error.message));
                        }
                    }
                }
            }
        }
        (ParameterSchema::Scalar { fields }, _) => {
            if fields.iter().any(|field| field.required) {
                problems.push("必填标量参数未填写".to_string());
            }
        }
        (ParameterSchema::Table(table), ParameterValues::Rows { rows }) => {
            if let Some(range) = cardinality.get(&table.cardinality_key)
                && rows.len() < range.min
            {
                problems.push(format!(
                    "表 {} 行数 {} 低于基数下限 {}",
                    table.cardinality_key,
                    rows.len(),
                    range.min
                ));
            }
            let mut seen_keys = BTreeMap::new();
            for (row_index, row) in rows.iter().enumerate() {
                let row_id = row
                    .get(&table.row_key)
                    .map(|value| value.render())
                    .unwrap_or_default();
                if row_id.is_empty() {
                    problems.push(format!(
                        "第 {} 行缺少行标识列 {}",
                        row_index + 1,
                        table.row_key
                    ));
                } else if let Some(previous) = seen_keys.insert(row_id.clone(), row_index) {
                    problems.push(format!(
                        "行标识 {row_id} 重复（第 {} 行与第 {} 行）",
                        previous + 1,
                        row_index + 1
                    ));
                }
                for column in &table.columns {
                    match row.get(&column.key) {
                        None if column.required => {
                            problems.push(format!("第 {} 行缺少列 {}", row_index + 1, column.key));
                        }
                        None => {}
                        Some(value) => {
                            if !value.type_matches(&column.kind) {
                                problems.push(format!(
                                    "第 {} 行列 {} 类型不符",
                                    row_index + 1,
                                    column.key
                                ));
                            }
                            if let Some(constraint) = &column.constraint
                                && let Err(error) = value.check_constraint(constraint)
                            {
                                problems.push(format!(
                                    "第 {} 行列 {}：{}",
                                    row_index + 1,
                                    column.key,
                                    error.message
                                ));
                            }
                        }
                    }
                }
            }
        }
        (ParameterSchema::Table(_), _) => {
            problems.push("表结构参数未填写行数据".to_string());
        }
        (ParameterSchema::Matrix(matrix), ParameterValues::Cells { cells }) => {
            let rows = enumerate_axis(graph, selections, &matrix.row_axis);
            let cols = enumerate_axis(graph, selections, &matrix.col_axis);
            if rows.is_empty() {
                problems.push("矩阵行轴无法枚举（上游决策未完成）".to_string());
            }
            if cols.is_empty() {
                problems.push("矩阵列轴无法枚举（上游决策未完成）".to_string());
            }
            for row in &rows {
                for col in &cols {
                    let cell = cells
                        .iter()
                        .find(|cell| &cell.row == row && &cell.col == col);
                    match cell {
                        None => problems.push(format!("矩阵缺格：[{row} × {col}]")),
                        Some(cell) => {
                            if !cell.value.type_matches(&matrix.cell.kind) {
                                problems.push(format!("矩阵格 [{row} × {col}] 类型不符"));
                            }
                            if let Some(constraint) = &matrix.cell.constraint
                                && let Err(error) = cell.value.check_constraint(constraint)
                            {
                                problems.push(format!("矩阵格 [{row} × {col}]：{}", error.message));
                            }
                        }
                    }
                }
            }
        }
        (ParameterSchema::Matrix(_), _) => {
            problems.push("矩阵参数未填写格数据".to_string());
        }
    }
    problems
}

/// 枚举矩阵轴的取值集合。
pub fn enumerate_axis(
    graph: &DecisionGraph,
    selections: &BTreeMap<DecisionId, Selection>,
    axis: &AxisRef,
) -> Vec<String> {
    match axis {
        AxisRef::DecisionOptions { decision } => graph
            .point(decision)
            .map(|point| {
                point
                    .options
                    .iter()
                    .map(|option| option.id.clone())
                    .collect()
            })
            .unwrap_or_default(),
        AxisRef::TableRows { decision } => {
            let Some(selection) = selections.get(decision) else {
                return Vec::new();
            };
            let Some(point) = graph.point(decision) else {
                return Vec::new();
            };
            let Some(option) = point.option(&selection.option_id) else {
                return Vec::new();
            };
            let ParameterSchema::Table(table) = &option.parameter_schema else {
                return Vec::new();
            };
            let ParameterValues::Rows { rows } = &selection.parameters else {
                return Vec::new();
            };
            rows.iter()
                .filter_map(|row| row.get(&table.row_key).map(|value| value.render()))
                .filter(|value| !value.is_empty())
                .collect()
        }
    }
}

/// 完成度计算（设计 01 号文档 §2.6）。
///
/// `row_references` 为品类包声明的跨表外键规则：悬空引用与单表校验一样进待填清单，
/// 使「表都填满了但行引用指向不存在的键」不能算完成（否则会一路带进 FrozenDesign）。
pub fn compute_completeness(
    graph: &DecisionGraph,
    selections: &BTreeMap<DecisionId, Selection>,
    not_applicable: &BTreeMap<DecisionId, NaJustification>,
    applicability: &ApplicabilityMap,
    cardinality: &BTreeMap<String, CardinalityRange>,
    row_references: &[RowReference],
) -> CompletenessReport {
    let mut done = 0;
    let mut total = 0;
    let mut blocking = Vec::new();
    let mut na_reason_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut reference_problems =
        row_reference_problems_by_decision(graph, selections, row_references);

    for point in graph.points() {
        match applicability.get(&point.id) {
            Some(PointApplicability::Active) => {}
            Some(PointApplicability::NotApplicable(justification)) => {
                *na_reason_counts
                    .entry(justification.reason_code.clone())
                    .or_default() += 1;
                continue;
            }
            _ => continue,
        }
        total += 1;
        let Some(selection) = selections.get(&point.id) else {
            blocking.push(MissingItem {
                decision_id: point.id.clone(),
                detail: "未选择".into(),
            });
            continue;
        };
        if !selection.confirmed_by_user {
            blocking.push(MissingItem {
                decision_id: point.id.clone(),
                detail: "未经用户确认（AI 提案/模板预填需确认）".into(),
            });
            continue;
        }
        let mut problems = validate_parameters(graph, selections, point, selection, cardinality);
        if let Some(dangling) = reference_problems.remove(&point.id) {
            problems.extend(dangling);
        }
        if problems.is_empty() {
            done += 1;
        } else {
            for problem in problems {
                blocking.push(MissingItem {
                    decision_id: point.id.clone(),
                    detail: problem,
                });
            }
        }
    }
    let _ = not_applicable; // N/A 已通过 applicability 反映；保留参数以固定调用契约。

    CompletenessReport {
        done,
        total,
        blocking,
        na_reason_counts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::applicability::compute_applicability;
    use crate::types::{
        DecisionOption, DepthProfile, DesignLevel, GenreScope, MatrixSchema, PointRequirement,
        Provenance, ScalarField, TableSchema,
    };
    use adm4_contracts::{MatrixCell, TypedValue, ValueKind};

    fn table_point() -> DecisionPoint {
        DecisionPoint {
            id: "guards".into(),
            domain: "gameplay".into(),
            level: DesignLevel::L5,
            genre_scope: GenreScope::Universal,
            question: "守卫有哪些？".into(),
            mda_layer: None,
            requirement: PointRequirement::Unlocked,
            options: vec![DecisionOption {
                id: "roster".into(),
                label: "守卫名单".into(),
                parameter_schema: ParameterSchema::Table(TableSchema {
                    columns: vec![
                        ScalarField {
                            key: "id".into(),
                            kind: ValueKind::Text,
                            constraint: None,
                            required: true,
                            is_skin: false,
                        },
                        ScalarField {
                            key: "cost".into(),
                            kind: ValueKind::Int,
                            constraint: None,
                            required: true,
                            is_skin: false,
                        },
                    ],
                    row_key: "id".into(),
                    cardinality_key: "guard_types".into(),
                }),
                ..Default::default()
            }],
            skin_fields: Vec::new(),
            evidence_slots: false,
        }
    }

    fn row(id: &str, cost: i64) -> BTreeMap<String, TypedValue> {
        [
            ("id".to_string(), TypedValue::Text(id.into())),
            ("cost".to_string(), TypedValue::Int(cost)),
        ]
        .into_iter()
        .collect()
    }

    fn selection_with_rows(rows: Vec<BTreeMap<String, TypedValue>>) -> Selection {
        Selection {
            decision_id: "guards".into(),
            option_id: "roster".into(),
            parameters: ParameterValues::Rows { rows },
            rationale: "test".into(),
            provenance: Provenance::UserManual,
            confirmed_by_user: true,
            template_original: None,
        }
    }

    #[test]
    fn table_cardinality_and_missing_columns_block() {
        let graph = DecisionGraph::new(vec![table_point()]).unwrap();
        let cardinality: BTreeMap<_, _> = [(
            "guard_types".to_string(),
            CardinalityRange { min: 2, max: 8 },
        )]
        .into_iter()
        .collect();
        let selections: BTreeMap<_, _> = [(
            "guards".to_string(),
            selection_with_rows(vec![row("archer", 100)]),
        )]
        .into_iter()
        .collect();
        let applicability = compute_applicability(
            &graph,
            &selections,
            &BTreeMap::new(),
            DepthProfile::new(DesignLevel::L6).unwrap(),
        );
        let report = compute_completeness(
            &graph,
            &selections,
            &BTreeMap::new(),
            &applicability,
            &cardinality,
            &[],
        );
        assert_eq!(report.done, 0);
        assert!(
            report
                .blocking
                .iter()
                .any(|item| item.detail.contains("基数下限"))
        );

        let selections: BTreeMap<_, _> = [(
            "guards".to_string(),
            selection_with_rows(vec![row("archer", 100), row("mage", 150)]),
        )]
        .into_iter()
        .collect();
        let report = compute_completeness(
            &graph,
            &selections,
            &BTreeMap::new(),
            &applicability,
            &cardinality,
            &[],
        );
        assert!(report.is_complete());
    }

    #[test]
    fn dangling_row_reference_keeps_completeness_incomplete() {
        let mut wave_point = table_point();
        wave_point.id = "waves".into();
        wave_point.options = vec![DecisionOption {
            id: "wave_rows".into(),
            label: "波次行".into(),
            parameter_schema: ParameterSchema::Table(TableSchema {
                columns: vec![
                    ScalarField {
                        key: "id".into(),
                        kind: ValueKind::Text,
                        constraint: None,
                        required: true,
                        is_skin: false,
                    },
                    ScalarField {
                        key: "cost".into(),
                        kind: ValueKind::Int,
                        constraint: None,
                        required: true,
                        is_skin: false,
                    },
                ],
                row_key: "id".into(),
                cardinality_key: "guard_types".into(),
            }),
            ..Default::default()
        }];
        let graph = DecisionGraph::new(vec![table_point(), wave_point]).unwrap();
        let cardinality: BTreeMap<_, _> = [(
            "guard_types".to_string(),
            CardinalityRange { min: 1, max: 8 },
        )]
        .into_iter()
        .collect();
        let mut waves = selection_with_rows(vec![row("ghost_guard", 10)]);
        waves.decision_id = "waves".into();
        waves.option_id = "wave_rows".into();
        let selections: BTreeMap<_, _> = [
            (
                "guards".to_string(),
                selection_with_rows(vec![row("archer", 100)]),
            ),
            ("waves".to_string(), waves),
        ]
        .into_iter()
        .collect();
        let applicability = compute_applicability(
            &graph,
            &selections,
            &BTreeMap::new(),
            DepthProfile::new(DesignLevel::L6).unwrap(),
        );
        let references = [RowReference {
            rule_id: "waves_reference_guards".into(),
            source_decision: "waves".into(),
            source_column: "id".into(),
            target_decision: "guards".into(),
            target_key_column: "id".into(),
        }];

        // 无外键规则时两张表各自合法 → 完成。
        let report = compute_completeness(
            &graph,
            &selections,
            &BTreeMap::new(),
            &applicability,
            &cardinality,
            &[],
        );
        assert!(report.is_complete(), "blocking: {:?}", report.blocking);

        // 挂上外键规则后，悬空引用进待填清单且该点不计入完成。
        let report = compute_completeness(
            &graph,
            &selections,
            &BTreeMap::new(),
            &applicability,
            &cardinality,
            &references,
        );
        assert!(!report.is_complete());
        assert_eq!(report.done, 1);
        assert!(
            report
                .blocking
                .iter()
                .any(|item| item.decision_id == "waves"
                    && item.detail.contains("ghost_guard")
                    && item.detail.contains("waves_reference_guards")),
            "{:?}",
            report.blocking
        );
    }

    #[test]
    fn matrix_missing_cells_are_listed_individually() {
        let mut matrix_point = table_point();
        matrix_point.id = "counter".into();
        matrix_point.options = vec![DecisionOption {
            id: "matrix_full".into(),
            label: "全克制矩阵".into(),
            parameter_schema: ParameterSchema::Matrix(MatrixSchema {
                row_axis: AxisRef::TableRows {
                    decision: "guards".into(),
                },
                col_axis: AxisRef::DecisionOptions {
                    decision: "enemy_kind".into(),
                },
                cell: ScalarField {
                    key: "coeff".into(),
                    kind: ValueKind::Float,
                    constraint: None,
                    required: true,
                    is_skin: false,
                },
                cardinality_key: "counter_cells".into(),
            }),
            ..Default::default()
        }];
        let enemy_point = DecisionPoint {
            id: "enemy_kind".into(),
            domain: "gameplay".into(),
            level: DesignLevel::L3,
            genre_scope: GenreScope::Universal,
            question: "敌人种类？".into(),
            mda_layer: None,
            requirement: PointRequirement::Unlocked,
            options: vec![
                DecisionOption {
                    id: "walker".into(),
                    label: "步行".into(),
                    ..Default::default()
                },
                DecisionOption {
                    id: "flyer".into(),
                    label: "飞行".into(),
                    ..Default::default()
                },
            ],
            skin_fields: Vec::new(),
            evidence_slots: false,
        };
        let graph = DecisionGraph::new(vec![table_point(), matrix_point, enemy_point]).unwrap();
        let mut selections: BTreeMap<String, Selection> = BTreeMap::new();
        selections.insert(
            "guards".into(),
            selection_with_rows(vec![row("archer", 100), row("mage", 150)]),
        );
        selections.insert(
            "counter".into(),
            Selection {
                decision_id: "counter".into(),
                option_id: "matrix_full".into(),
                parameters: ParameterValues::Cells {
                    cells: vec![MatrixCell {
                        row: "archer".into(),
                        col: "walker".into(),
                        value: TypedValue::Float(1.5),
                    }],
                },
                rationale: String::new(),
                provenance: Provenance::UserManual,
                confirmed_by_user: true,
                template_original: None,
            },
        );
        let point = graph.point("counter").unwrap();
        let problems = validate_parameters(
            &graph,
            &selections,
            point,
            &selections["counter"],
            &BTreeMap::new(),
        );
        // 2×2 矩阵只填 1 格 → 缺 3 格，逐格列出。
        assert_eq!(
            problems
                .iter()
                .filter(|problem| problem.contains("矩阵缺格"))
                .count(),
            3
        );
    }
}
