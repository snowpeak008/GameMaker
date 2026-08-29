//! 跨表一致性规则的机器化实现：目前承载「跨表外键引用」（row_reference）。
//!
//! 单表校验（`validate_parameters`）只看得见一张表，无法约束「A 表某列的取值必须
//! 落在 B 表的行键集合内」。缺了这道检查，悬空行引用（波次行指向不存在的关卡、
//! 数值行指向已删除的实体）可以一路穿过完成度与冻结门进入 FrozenDesign，直到 C0
//! 编译或更晚才暴露。本模块把这类引用关系变成品类包可声明、校验器可执行的规则。

use crate::graph::DecisionGraph;
use crate::types::{DecisionId, ParameterSchema, ParameterValues, ScalarField, Selection};
use adm4_contracts::TypedValue;
use std::collections::{BTreeMap, BTreeSet};

/// 违规信息里最多列出的目标行键数量（超出只报总数，避免大表刷屏）。
const MAX_LISTED_KEYS: usize = 8;

/// 跨表外键引用声明：`source_decision` 表的 `source_column` 列取值，
/// 必须落在 `target_decision` 表 `target_key_column` 列的取值集合内。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowReference {
    /// 品类包里的规则 id，回写进违规信息供定位。
    pub rule_id: String,
    pub source_decision: DecisionId,
    pub source_column: String,
    pub target_decision: DecisionId,
    pub target_key_column: String,
}

/// 一条外键违规；`decision_id` 恒为源决策点，便于并入完成度的待填清单。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowReferenceViolation {
    pub rule_id: String,
    pub decision_id: DecisionId,
    /// 点名「哪张表、哪一行、哪个值」。
    pub detail: String,
}

/// 逐条执行外键规则。
///
/// 适用性判定（避免把「还没答到那儿」误报成引用错误）：
/// 源决策未选、当前选项不是表结构、尚未填行数据、或当前选项的表里根本没有
/// `source_column` 列（例如选了不带关卡列的简版波次表）→ 本规则不适用，静默跳过；
/// 缺答本身由完成度门负责。
///
/// 一旦源表确实产生了引用值，目标侧就必须能给出行键集合：目标未答/不是表/未填行/
/// 缺键列一律判违规（R2 未知即停——引用是否悬空无法判定时不得放行）。
pub fn check_row_references(
    graph: &DecisionGraph,
    selections: &BTreeMap<DecisionId, Selection>,
    rules: &[RowReference],
) -> Vec<RowReferenceViolation> {
    let mut violations = Vec::new();
    for rule in rules {
        check_one(graph, selections, rule, &mut violations);
    }
    violations
}

/// 按源决策点分组的违规明细，供完成度按点归并。
pub fn row_reference_problems_by_decision(
    graph: &DecisionGraph,
    selections: &BTreeMap<DecisionId, Selection>,
    rules: &[RowReference],
) -> BTreeMap<DecisionId, Vec<String>> {
    let mut grouped: BTreeMap<DecisionId, Vec<String>> = BTreeMap::new();
    for violation in check_row_references(graph, selections, rules) {
        grouped
            .entry(violation.decision_id)
            .or_default()
            .push(format!("[{}] {}", violation.rule_id, violation.detail));
    }
    grouped
}

fn check_one(
    graph: &DecisionGraph,
    selections: &BTreeMap<DecisionId, Selection>,
    rule: &RowReference,
    out: &mut Vec<RowReferenceViolation>,
) {
    let Ok(source) = table_view(graph, selections, &rule.source_decision) else {
        return; // 源侧尚未答到可校验的状态：缺答由完成度门负责。
    };
    if !has_column(source.columns, &rule.source_column) {
        return; // 当前选项的表结构不含该列：本规则对这个分支不适用。
    }
    let referencing: Vec<(usize, String)> = source
        .rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            row.get(&rule.source_column)
                .map(|value| (index + 1, value.render()))
        })
        .filter(|(_, value)| !value.trim().is_empty())
        .collect();
    if referencing.is_empty() {
        return;
    }

    let target = match table_view(graph, selections, &rule.target_decision) {
        Ok(target) if has_column(target.columns, &rule.target_key_column) => target,
        Ok(target) => {
            out.push(violation(
                rule,
                format!(
                    "表 {}（选项 {}）的 {} 列引用 {} 的 {} 行键，但 {} 当前选项 {} 的表结构不含该键列，{} 行引用无法判定（R2 未知即停）",
                    rule.source_decision,
                    source.option_id,
                    rule.source_column,
                    rule.target_decision,
                    rule.target_key_column,
                    rule.target_decision,
                    target.option_id,
                    referencing.len()
                ),
            ));
            return;
        }
        Err(reason) => {
            out.push(violation(
                rule,
                format!(
                    "表 {}（选项 {}）的 {} 列引用 {} 的 {} 行键，但{}，{} 行引用无法判定（R2 未知即停）",
                    rule.source_decision,
                    source.option_id,
                    rule.source_column,
                    rule.target_decision,
                    rule.target_key_column,
                    reason,
                    referencing.len()
                ),
            ));
            return;
        }
    };

    let keys: BTreeSet<String> = target
        .rows
        .iter()
        .filter_map(|row| row.get(&rule.target_key_column).map(TypedValue::render))
        .filter(|value| !value.trim().is_empty())
        .collect();
    let key_hint = render_keys(&keys);
    for (row_number, value) in referencing {
        if !keys.contains(&value) {
            out.push(violation(
                rule,
                format!(
                    "表 {} 第 {} 行的 {}=「{}」在 {} 的 {} 行键集合中不存在（{}）",
                    rule.source_decision,
                    row_number,
                    rule.source_column,
                    value,
                    rule.target_decision,
                    rule.target_key_column,
                    key_hint
                ),
            ));
        }
    }
}

fn violation(rule: &RowReference, detail: String) -> RowReferenceViolation {
    RowReferenceViolation {
        rule_id: rule.rule_id.clone(),
        decision_id: rule.source_decision.clone(),
        detail,
    }
}

fn render_keys(keys: &BTreeSet<String>) -> String {
    if keys.is_empty() {
        return "目标表当前无任何有效行键".to_string();
    }
    let listed: Vec<&str> = keys
        .iter()
        .take(MAX_LISTED_KEYS)
        .map(String::as_str)
        .collect();
    if keys.len() > MAX_LISTED_KEYS {
        format!(
            "现有 {} 个行键，前 {} 个：{}",
            keys.len(),
            MAX_LISTED_KEYS,
            listed.join("、")
        )
    } else {
        format!("现有行键：{}", listed.join("、"))
    }
}

fn has_column(columns: &[ScalarField], key: &str) -> bool {
    columns.iter().any(|column| column.key == key)
}

/// 一个决策点当前「已选表结构选项 + 已填行数据」的只读视图。
struct TableView<'a> {
    option_id: &'a str,
    columns: &'a [ScalarField],
    rows: &'a [BTreeMap<String, TypedValue>],
}

/// 取表视图；取不到时返回中文原因（可直接拼进违规信息）。
fn table_view<'a>(
    graph: &'a DecisionGraph,
    selections: &'a BTreeMap<DecisionId, Selection>,
    decision: &str,
) -> Result<TableView<'a>, String> {
    let Some(selection) = selections.get(decision) else {
        return Err(format!("{decision} 尚未回答"));
    };
    let Some(point) = graph.point(decision) else {
        return Err(format!("{decision} 不在当前决策图内"));
    };
    let Some(option) = point.option(&selection.option_id) else {
        return Err(format!(
            "{decision} 选择了清单外的选项 {}",
            selection.option_id
        ));
    };
    let ParameterSchema::Table(table) = &option.parameter_schema else {
        return Err(format!("{decision}/{} 的参数结构不是表", option.id));
    };
    let ParameterValues::Rows { rows } = &selection.parameters else {
        return Err(format!("{decision}/{} 尚未填写行数据", option.id));
    };
    Ok(TableView {
        option_id: &option.id,
        columns: &table.columns,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        DecisionOption, DecisionPoint, DesignLevel, GenreScope, PointRequirement, Provenance,
        TableSchema,
    };
    use adm4_contracts::ValueKind;

    fn text_column(key: &str) -> ScalarField {
        ScalarField {
            key: key.into(),
            kind: ValueKind::Text,
            constraint: None,
            required: true,
            is_skin: false,
        }
    }

    fn table_point(
        id: &str,
        option_id: &str,
        columns: Vec<ScalarField>,
        row_key: &str,
    ) -> DecisionPoint {
        DecisionPoint {
            id: id.into(),
            domain: "d".into(),
            level: DesignLevel::L5,
            genre_scope: GenreScope::Universal,
            question: "q".into(),
            mda_layer: None,
            requirement: PointRequirement::Unlocked,
            options: vec![DecisionOption {
                id: option_id.into(),
                label: option_id.into(),
                parameter_schema: ParameterSchema::Table(TableSchema {
                    columns,
                    row_key: row_key.into(),
                    cardinality_key: "k".into(),
                }),
                ..Default::default()
            }],
            skin_fields: Vec::new(),
            evidence_slots: false,
        }
    }

    fn graph() -> DecisionGraph {
        let stages = table_point(
            "stages",
            "stage_table",
            vec![text_column("stage_id")],
            "stage_id",
        );
        let waves = table_point(
            "waves",
            "wave_rows",
            vec![text_column("row_id"), text_column("stage_id")],
            "row_id",
        );
        let simple_waves = table_point(
            "simple_waves",
            "plain_rows",
            vec![text_column("row_id")],
            "row_id",
        );
        match DecisionGraph::new(vec![stages, waves, simple_waves]) {
            Ok(graph) => graph,
            Err(error) => panic!("测试图构造失败：{}", error.message),
        }
    }

    fn rows(entries: &[&[(&str, &str)]]) -> ParameterValues {
        ParameterValues::Rows {
            rows: entries
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|(key, value)| {
                            ((*key).to_string(), TypedValue::Text((*value).to_string()))
                        })
                        .collect()
                })
                .collect(),
        }
    }

    fn selection(decision: &str, option: &str, parameters: ParameterValues) -> Selection {
        Selection {
            decision_id: decision.into(),
            option_id: option.into(),
            parameters,
            rationale: String::new(),
            provenance: Provenance::UserManual,
            confirmed_by_user: true,
            template_original: None,
        }
    }

    fn rule() -> RowReference {
        RowReference {
            rule_id: "waves_reference_stages".into(),
            source_decision: "waves".into(),
            source_column: "stage_id".into(),
            target_decision: "stages".into(),
            target_key_column: "stage_id".into(),
        }
    }

    #[test]
    fn valid_reference_passes() {
        let selections: BTreeMap<_, _> = [
            (
                "stages".to_string(),
                selection(
                    "stages",
                    "stage_table",
                    rows(&[&[("stage_id", "dawn_ridge")], &[("stage_id", "salt_flats")]]),
                ),
            ),
            (
                "waves".to_string(),
                selection(
                    "waves",
                    "wave_rows",
                    rows(&[
                        &[("row_id", "w1"), ("stage_id", "dawn_ridge")],
                        &[("row_id", "w2"), ("stage_id", "salt_flats")],
                    ]),
                ),
            ),
        ]
        .into_iter()
        .collect();
        assert!(check_row_references(&graph(), &selections, &[rule()]).is_empty());
    }

    #[test]
    fn dangling_reference_names_table_row_and_value() {
        let selections: BTreeMap<_, _> = [
            (
                "stages".to_string(),
                selection(
                    "stages",
                    "stage_table",
                    rows(&[&[("stage_id", "dawn_ridge")]]),
                ),
            ),
            (
                "waves".to_string(),
                selection(
                    "waves",
                    "wave_rows",
                    rows(&[
                        &[("row_id", "w1"), ("stage_id", "dawn_ridge")],
                        &[("row_id", "w2"), ("stage_id", "ghost_stage")],
                    ]),
                ),
            ),
        ]
        .into_iter()
        .collect();
        let violations = check_row_references(&graph(), &selections, &[rule()]);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert_eq!(violations[0].decision_id, "waves");
        assert_eq!(violations[0].rule_id, "waves_reference_stages");
        let detail = &violations[0].detail;
        assert!(detail.contains("waves"), "{detail}");
        assert!(detail.contains("第 2 行"), "{detail}");
        assert!(detail.contains("ghost_stage"), "{detail}");
        assert!(detail.contains("dawn_ridge"), "{detail}");
    }

    #[test]
    fn unanswered_target_blocks_when_source_answered() {
        let selections: BTreeMap<_, _> = [(
            "waves".to_string(),
            selection(
                "waves",
                "wave_rows",
                rows(&[&[("row_id", "w1"), ("stage_id", "dawn_ridge")]]),
            ),
        )]
        .into_iter()
        .collect();
        let violations = check_row_references(&graph(), &selections, &[rule()]);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].detail.contains("尚未回答"),
            "{}",
            violations[0].detail
        );
        assert!(
            violations[0].detail.contains("R2"),
            "{}",
            violations[0].detail
        );
    }

    #[test]
    fn unanswered_source_is_not_a_violation() {
        let selections: BTreeMap<_, _> = [(
            "stages".to_string(),
            selection(
                "stages",
                "stage_table",
                rows(&[&[("stage_id", "dawn_ridge")]]),
            ),
        )]
        .into_iter()
        .collect();
        assert!(check_row_references(&graph(), &selections, &[rule()]).is_empty());
    }

    #[test]
    fn option_without_source_column_is_not_applicable() {
        let inapplicable = RowReference {
            rule_id: "simple_waves_reference_stages".into(),
            source_decision: "simple_waves".into(),
            source_column: "stage_id".into(),
            target_decision: "stages".into(),
            target_key_column: "stage_id".into(),
        };
        let selections: BTreeMap<_, _> = [(
            "simple_waves".to_string(),
            selection("simple_waves", "plain_rows", rows(&[&[("row_id", "w1")]])),
        )]
        .into_iter()
        .collect();
        assert!(check_row_references(&graph(), &selections, &[inapplicable]).is_empty());
    }

    #[test]
    fn problems_are_grouped_by_source_decision() {
        let selections: BTreeMap<_, _> = [(
            "waves".to_string(),
            selection(
                "waves",
                "wave_rows",
                rows(&[&[("row_id", "w1"), ("stage_id", "ghost_stage")]]),
            ),
        )]
        .into_iter()
        .collect();
        let grouped = row_reference_problems_by_decision(&graph(), &selections, &[rule()]);
        assert_eq!(grouped.len(), 1);
        let problems = match grouped.get("waves") {
            Some(problems) => problems,
            None => panic!("违规应归到源决策点 waves：{grouped:?}"),
        };
        assert!(
            problems[0].starts_with("[waves_reference_stages]"),
            "{problems:?}"
        );
    }
}
