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
    /// `requirement=Optional` 且未作答，因此被移出分母的适用点数（非必做点，在案可见）。
    /// 作答过的 Optional 点不计在此——它们已按普通点进 done/total。
    pub optional_skipped: usize,
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
///
/// 多选点逐个已选选项分别校验（主选在前），明细前缀「选项 X：」以便定位；
/// 单选点的明细文本与扩展前完全一致。
pub fn validate_parameters(
    graph: &DecisionGraph,
    selections: &BTreeMap<DecisionId, Selection>,
    point: &DecisionPoint,
    selection: &Selection,
    cardinality: &BTreeMap<String, CardinalityRange>,
) -> Vec<String> {
    let selected = selection.selected_options();
    let label_each = selected.len() > 1;
    let mut problems = Vec::new();
    for item in selected {
        let per_option = validate_option_parameters(
            graph,
            selections,
            point,
            item.option_id,
            item.parameters,
            cardinality,
        );
        if label_each {
            problems.extend(
                per_option
                    .into_iter()
                    .map(|problem| format!("选项 {}：{problem}", item.option_id)),
            );
        } else {
            problems.extend(per_option);
        }
    }
    problems
}

/// 单个已选选项的参数校验（多选点按选项逐个调用）。
pub fn validate_option_parameters(
    graph: &DecisionGraph,
    selections: &BTreeMap<DecisionId, Selection>,
    point: &DecisionPoint,
    option_id: &str,
    parameters: &ParameterValues,
    cardinality: &BTreeMap<String, CardinalityRange>,
) -> Vec<String> {
    let Some(option) = point.option(option_id) else {
        return vec![format!("选项 {option_id} 不存在")];
    };
    let mut problems = Vec::new();
    match (&option.parameter_schema, parameters) {
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
        // W7 §5.4 机械新增臂（T-W7-3a 编译豁免申报）：Graph/Curve 值沿 Curve 先例
        // 以标量 `graph`/`curve` 键装 JSON 文本（ParameterValues 现有形态零改动），
        // 结构校验（端点/环/入口）由 C0 编译前置拦截（I3）——本函数零行为改动。
        (ParameterSchema::Graph(_), _) | (ParameterSchema::Curve(_), _) => {}
    }
    problems
}

/// 校验已选集合与选择基数是否相符（单选点不许多选、多选点主选必须落在已选集合内）。
///
/// 多选点「至少选 1」由 `Selection` 的结构本身保证（`option_id` 恒有值，没选就没有
/// Selection，缺答走「未选择」分支）；这里补的是四版原本没有的另外两条：
/// 单选点被写入多个选项、以及 `allow_primary` 点缺主选/主选不在已选集合内。
pub fn validate_selection_mode(point: &DecisionPoint, selection: &Selection) -> Vec<String> {
    let mut problems = Vec::new();
    let selected: Vec<&str> = selection.selected_option_ids();
    if !point.is_multi() {
        if selected.len() > 1 {
            problems.push(format!(
                "单选点被写入 {} 个已选选项（{}）：single 点最多一个选项",
                selected.len(),
                selected.join("、")
            ));
        }
        if let Some(primary) = &selection.primary_option {
            problems.push(format!(
                "单选点标记了主选 {primary}：主选只对 multi + allow_primary 的点有意义"
            ));
        }
        return problems;
    }
    let mut seen: Vec<&str> = Vec::new();
    for option_id in &selected {
        if seen.contains(option_id) {
            problems.push(format!("已选选项 {option_id} 重复"));
        } else {
            seen.push(option_id);
        }
    }
    match &selection.primary_option {
        Some(primary) if !selection.contains_option(primary) => problems.push(format!(
            "主选 {primary} 不在已选集合（{}）内",
            selected.join("、")
        )),
        None if point.requires_primary() => problems.push(format!(
            "多选点要求标记主选（allow_primary），当前已选 {} 项但未指定主选",
            selected.len()
        )),
        _ => {}
    }
    if selection.primary_option.is_some() && !point.requires_primary() {
        problems.push("该多选点未开启 allow_primary，不接受主选标记".to_string());
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
        // 多选点：全部已选选项的行键并集（主选在前），少算一个选项会让矩阵轴缺行。
        AxisRef::TableRows { decision } => {
            let Some(selection) = selections.get(decision) else {
                return Vec::new();
            };
            let Some(point) = graph.point(decision) else {
                return Vec::new();
            };
            let mut values = Vec::new();
            for item in selection.selected_options() {
                let Some(option) = point.option(item.option_id) else {
                    continue;
                };
                let ParameterSchema::Table(table) = &option.parameter_schema else {
                    continue;
                };
                let ParameterValues::Rows { rows } = item.parameters else {
                    continue;
                };
                for row in rows {
                    let Some(rendered) = row.get(&table.row_key).map(|value| value.render()) else {
                        continue;
                    };
                    if !rendered.is_empty() && !values.contains(&rendered) {
                        values.push(rendered);
                    }
                }
            }
            values
        }
    }
}

/// 非必做点且尚未作答——「不进完成度分母」的唯一新增情形。
///
/// 单独抽成函数是为了让完成度、领域/节点聚合、访谈进度与访谈待办四处共用同一口径：
/// 口径分叉会让「完成度 100% 但访谈永远停在某层」这类矛盾状态出现。
pub fn optional_unanswered(
    point: &DecisionPoint,
    selections: &BTreeMap<DecisionId, Selection>,
) -> bool {
    point.requirement.is_optional() && !selections.contains_key(&point.id)
}

/// 该决策点是否计入完成度分母：适用（Active）且不是「未作答的非必做点」。
pub fn counts_toward_completeness(
    point: &DecisionPoint,
    applicability: &ApplicabilityMap,
    selections: &BTreeMap<DecisionId, Selection>,
) -> bool {
    matches!(
        applicability.get(&point.id),
        Some(PointApplicability::Active)
    ) && !optional_unanswered(point, selections)
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
    let mut optional_skipped = 0;
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
        // 非必做点未作答：移出分母（二版「非必做」检查单项的归宿），计数在案。
        // 已作答的非必做点走下面的普通路径——作答即纳入设计，校验一视同仁。
        if optional_unanswered(point, selections) {
            optional_skipped += 1;
            continue;
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
        let mut problems = validate_selection_mode(point, selection);
        problems.extend(validate_parameters(
            graph,
            selections,
            point,
            selection,
            cardinality,
        ));
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
        optional_skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::applicability::compute_applicability;
    use crate::types::{
        DecisionOption, DepthProfile, DesignLevel, GenreScope, MatrixSchema, PointRequirement,
        Provenance, ScalarField, SelectedOption, SelectionMode, TableSchema,
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
            design_question: None,
            node_id: None,
            selection_mode: SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            tier_gate: None,
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
            additional_options: Vec::new(),
            primary_option: None,
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
            design_question: None,
            node_id: None,
            selection_mode: SelectionMode::Single,
            requirement: PointRequirement::Unlocked,
            tier_gate: None,
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
                additional_options: Vec::new(),
                primary_option: None,
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

    /// 多选 + 主选的完成度正反例：缺主选/主选越界拦截，标对了才算完成。
    #[test]
    fn multi_point_primary_rules_gate_completeness() {
        let mut point = table_point();
        point.id = "core_feelings".into();
        point.level = DesignLevel::L1;
        point.selection_mode = SelectionMode::Multi {
            allow_primary: true,
        };
        point.options = vec![
            DecisionOption {
                id: "tense_choice".into(),
                label: "紧张抉择".into(),
                ..Default::default()
            },
            DecisionOption {
                id: "growth_accumulation".into(),
                label: "成长积累".into(),
                ..Default::default()
            },
            DecisionOption {
                id: "creative_expression".into(),
                label: "表达创造".into(),
                ..Default::default()
            },
        ];
        let graph = DecisionGraph::new(vec![point]).unwrap();
        let depth = DepthProfile::new(DesignLevel::L4).unwrap();

        let base = Selection {
            decision_id: "core_feelings".into(),
            option_id: "tense_choice".into(),
            parameters: ParameterValues::None,
            rationale: String::new(),
            provenance: Provenance::UserManual,
            confirmed_by_user: true,
            template_original: None,
            additional_options: vec![SelectedOption {
                option_id: "growth_accumulation".into(),
                ..Default::default()
            }],
            primary_option: None,
        };
        let report = |selection: Selection| {
            let selections: BTreeMap<_, _> = [("core_feelings".to_string(), selection)]
                .into_iter()
                .collect();
            let applicability = compute_applicability(&graph, &selections, &BTreeMap::new(), depth);
            compute_completeness(
                &graph,
                &selections,
                &BTreeMap::new(),
                &applicability,
                &BTreeMap::new(),
                &[],
            )
        };

        // 反例 1：allow_primary 但未标主选。
        let missing_primary = report(base.clone());
        assert_eq!(missing_primary.done, 0);
        assert!(
            missing_primary
                .blocking
                .iter()
                .any(|item| item.detail.contains("未指定主选")),
            "{:?}",
            missing_primary.blocking
        );

        // 反例 2：主选不在已选集合内。
        let mut outside = base.clone();
        outside.primary_option = Some("creative_expression".into());
        let outside_report = report(outside);
        assert_eq!(outside_report.done, 0);
        assert!(
            outside_report
                .blocking
                .iter()
                .any(|item| item.detail.contains("不在已选集合")),
            "{:?}",
            outside_report.blocking
        );

        // 正例：主选落在已选集合内 → 该点完成，且主选排序在前。
        let mut good = base;
        good.primary_option = Some("growth_accumulation".into());
        assert_eq!(
            good.selected_option_ids(),
            vec!["growth_accumulation", "tense_choice"]
        );
        let good_report = report(good);
        assert!(good_report.is_complete(), "{:?}", good_report.blocking);
        assert_eq!(good_report.done, 1);
    }

    /// 非必做点（`requirement=Optional`）的完成度口径：
    /// 未作答 → 不进分母、不进阻塞清单，只在 `optional_skipped` 计数；
    /// 一旦作答 → 与普通点一视同仁（进分母、参数缺漏照常拦）。
    #[test]
    fn optional_point_leaves_denominator_until_answered() {
        let mut required_point = table_point();
        required_point.id = "u.platform".into();
        required_point.level = DesignLevel::L0;
        required_point.options = vec![DecisionOption {
            id: "pc_single".into(),
            label: "PC 单机".into(),
            ..Default::default()
        }];
        let mut optional_point = required_point.clone();
        optional_point.id = "u.dimension".into();
        optional_point.requirement = PointRequirement::Optional;
        optional_point.options = vec![DecisionOption {
            id: "two_d".into(),
            label: "2D".into(),
            ..Default::default()
        }];
        let graph = DecisionGraph::new(vec![required_point, optional_point]).unwrap();
        let depth = DepthProfile::new(DesignLevel::L4).unwrap();
        let report = |selections: BTreeMap<String, Selection>| {
            let applicability = compute_applicability(&graph, &selections, &BTreeMap::new(), depth);
            compute_completeness(
                &graph,
                &selections,
                &BTreeMap::new(),
                &applicability,
                &BTreeMap::new(),
                &[],
            )
        };
        let answer = |decision: &str, option: &str| Selection {
            decision_id: decision.into(),
            option_id: option.into(),
            parameters: ParameterValues::None,
            rationale: String::new(),
            provenance: Provenance::UserManual,
            confirmed_by_user: true,
            template_original: None,
            additional_options: Vec::new(),
            primary_option: None,
        };

        // 只答必做点：分母只有 1（非必做点不在其中），完成度即 100%。
        let only_required: BTreeMap<_, _> =
            [("u.platform".to_string(), answer("u.platform", "pc_single"))]
                .into_iter()
                .collect();
        let skipped = report(only_required.clone());
        assert_eq!((skipped.done, skipped.total), (1, 1));
        assert_eq!(skipped.optional_skipped, 1);
        assert!(skipped.is_complete(), "{:?}", skipped.blocking);
        assert!(
            !skipped
                .blocking
                .iter()
                .any(|item| item.decision_id == "u.dimension"),
            "非必做点未作答不得进阻塞清单：{:?}",
            skipped.blocking
        );

        // 补答非必做点：进分母并计入完成。
        let mut both = only_required.clone();
        both.insert("u.dimension".into(), answer("u.dimension", "two_d"));
        let answered = report(both.clone());
        assert_eq!((answered.done, answered.total), (2, 2));
        assert_eq!(answered.optional_skipped, 0);

        // 作答但未确认：与普通点一样拦（作答即纳入设计，AI/模板预填仍需用户确认）。
        let mut unconfirmed = both;
        if let Some(selection) = unconfirmed.get_mut("u.dimension") {
            selection.confirmed_by_user = false;
        }
        let pending = report(unconfirmed);
        assert_eq!((pending.done, pending.total), (1, 2));
        assert_eq!(pending.optional_skipped, 0);
        assert!(
            pending
                .blocking
                .iter()
                .any(|item| item.decision_id == "u.dimension"),
            "{:?}",
            pending.blocking
        );
    }

    /// 单选点被写入多个选项 / 被标主选 → 完成度拦截（不静默接受）。
    #[test]
    fn single_point_rejects_multi_payload() {
        let mut point = table_point();
        point.id = "genre".into();
        point.level = DesignLevel::L2;
        point.options = vec![
            DecisionOption {
                id: "lane".into(),
                label: "通道".into(),
                ..Default::default()
            },
            DecisionOption {
                id: "grid".into(),
                label: "网格".into(),
                ..Default::default()
            },
        ];
        let graph = DecisionGraph::new(vec![point]).unwrap();
        let single = graph.point("genre").expect("点存在");
        let selection = Selection {
            decision_id: "genre".into(),
            option_id: "lane".into(),
            parameters: ParameterValues::None,
            rationale: String::new(),
            provenance: Provenance::UserManual,
            confirmed_by_user: true,
            template_original: None,
            additional_options: vec![SelectedOption {
                option_id: "grid".into(),
                ..Default::default()
            }],
            primary_option: Some("lane".into()),
        };
        let problems = validate_selection_mode(single, &selection);
        assert!(
            problems.iter().any(|item| item.contains("single 点最多")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|item| item.contains("主选只对")),
            "{problems:?}"
        );
    }
}
