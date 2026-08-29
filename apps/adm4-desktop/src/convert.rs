//! UI 行模型（字符串格缓冲）与 `ParameterValues::Rows/Cells` 之间的纯格式转换。
//!
//! 边界约定（D14）：这里只做「字符串 ↔ 类型值」的确定性格式转换，不做任何业务校验
//! （基数、必填、枚举合法性、行标识唯一性等一律交给 `AppServices` 后面的校验器）。
//! 任何解析失败都返回带定位上下文的 `Err`，由 UI 状态栏原样展示——禁止吞错兜底。

use adm4_contracts::{MatrixCell, TypedValue, ValueKind};
use adm4_decision::{MatrixSchema, ParameterValues, TableSchema};
use adm4_foundation::{Adm4Error, Adm4Result};
use std::collections::BTreeMap;

/// 值类型的中文短标（表头提示用）。
pub fn kind_label(kind: &ValueKind) -> &'static str {
    match kind {
        ValueKind::Int => "整数",
        ValueKind::Float => "数值",
        ValueKind::Bool => "布尔",
        ValueKind::Text => "文本",
        ValueKind::Enum { .. } => "枚举",
    }
}

/// 单格字符串 → 类型值。Int/Float/Bool 先去首尾空白再解析；
/// Text/Enum 原样保留（枚举取值是否合法属业务校验，交给后端报告）。
pub fn parse_cell(kind: &ValueKind, text: &str) -> Adm4Result<TypedValue> {
    match kind {
        ValueKind::Int => {
            let trimmed = text.trim();
            trimmed
                .parse::<i64>()
                .map(TypedValue::Int)
                .map_err(|_| Adm4Error::invalid_input(format!("「{trimmed}」无法解析为整数")))
        }
        ValueKind::Float => {
            let trimmed = text.trim();
            trimmed
                .parse::<f64>()
                .map(TypedValue::Float)
                .map_err(|_| Adm4Error::invalid_input(format!("「{trimmed}」无法解析为数值")))
        }
        ValueKind::Bool => match text.trim() {
            "true" => Ok(TypedValue::Bool(true)),
            "false" => Ok(TypedValue::Bool(false)),
            other => Err(Adm4Error::invalid_input(format!(
                "「{other}」无法解析为布尔值（只接受 true/false）"
            ))),
        },
        ValueKind::Text | ValueKind::Enum { .. } => Ok(TypedValue::Text(text.to_string())),
    }
}

/// 已存参数 → 表格编辑缓冲（每行按 schema 列序渲染为字符串；缺列渲染为空串）。
/// 返回 (缓冲, 警告清单)：值变体不符或存在 schema 外列时给出警告——这些数据
/// 结构化保存会丢失，须提示用户改用高级 JSON 模式处理，不得静默吞掉。
pub fn table_buffer_from_params(
    schema: &TableSchema,
    params: &ParameterValues,
) -> (Vec<Vec<String>>, Vec<String>) {
    let mut warnings = Vec::new();
    let buffer = match params {
        ParameterValues::Rows { rows } => rows
            .iter()
            .enumerate()
            .map(|(row_index, row)| {
                for key in row.keys() {
                    if !schema.columns.iter().any(|column| &column.key == key) {
                        warnings.push(format!(
                            "第 {} 行含 schema 外列「{key}」：结构化保存会丢弃该列，请改用高级 JSON 模式处理",
                            row_index + 1
                        ));
                    }
                }
                schema
                    .columns
                    .iter()
                    .map(|column| {
                        row.get(&column.key)
                            .map_or_else(String::new, TypedValue::render)
                    })
                    .collect()
            })
            .collect(),
        ParameterValues::None => Vec::new(),
        other => {
            warnings.push(format!(
                "现有参数不是行数据（实际为 {}），结构化编辑从空表开始；原数据请用高级 JSON 模式查看",
                variant_name(other)
            ));
            Vec::new()
        }
    };
    (buffer, warnings)
}

/// 表格编辑缓冲 → `ParameterValues::Rows`。
/// 留空格 = 未填该列（键不写入行 map，由后端校验器按 schema 报缺）；
/// 任一格解析失败即整体 Err，并给出「第几行第几列」定位。
pub fn table_buffer_to_params(
    schema: &TableSchema,
    buffer: &[Vec<String>],
) -> Adm4Result<ParameterValues> {
    let mut rows = Vec::with_capacity(buffer.len());
    for (row_index, row_buffer) in buffer.iter().enumerate() {
        if row_buffer.len() != schema.columns.len() {
            return Err(Adm4Error::internal(format!(
                "表格缓冲第 {} 行宽 {} 与 schema 列数 {} 不一致（UI 内部错误）",
                row_index + 1,
                row_buffer.len(),
                schema.columns.len()
            )));
        }
        let mut row = BTreeMap::new();
        for (column, cell) in schema.columns.iter().zip(row_buffer) {
            if cell.trim().is_empty() {
                continue;
            }
            let value = parse_cell(&column.kind, cell).map_err(|error| {
                Adm4Error::invalid_input(format!(
                    "第 {} 行列「{}」：{}",
                    row_index + 1,
                    column.key,
                    error.message
                ))
            })?;
            row.insert(column.key.clone(), value);
        }
        rows.push(row);
    }
    Ok(ParameterValues::Rows { rows })
}

/// 已存参数 → 矩阵格编辑缓冲（每格一行：[row, col, value]）。
/// 值变体不符时同表格逻辑：空缓冲 + 警告。
pub fn matrix_buffer_from_params(params: &ParameterValues) -> (Vec<Vec<String>>, Vec<String>) {
    match params {
        ParameterValues::Cells { cells } => (
            cells
                .iter()
                .map(|cell| vec![cell.row.clone(), cell.col.clone(), cell.value.render()])
                .collect(),
            Vec::new(),
        ),
        ParameterValues::None => (Vec::new(), Vec::new()),
        other => (
            Vec::new(),
            vec![format!(
                "现有参数不是矩阵格数据（实际为 {}），结构化编辑从空清单开始；原数据请用高级 JSON 模式查看",
                variant_name(other)
            )],
        ),
    }
}

/// 矩阵格编辑缓冲 → `ParameterValues::Cells`。
/// row/col 为空或值解析失败即整体 Err（缺格与轴合法性由后端校验器判定）。
pub fn matrix_buffer_to_params(
    schema: &MatrixSchema,
    buffer: &[Vec<String>],
) -> Adm4Result<ParameterValues> {
    let mut cells = Vec::with_capacity(buffer.len());
    for (cell_index, entry) in buffer.iter().enumerate() {
        if entry.len() != 3 {
            return Err(Adm4Error::internal(format!(
                "矩阵缓冲第 {} 格宽 {} 不是 3（UI 内部错误）",
                cell_index + 1,
                entry.len()
            )));
        }
        let row = entry[0].trim();
        let col = entry[1].trim();
        if row.is_empty() || col.is_empty() {
            return Err(Adm4Error::invalid_input(format!(
                "第 {} 格：row/col 标识不能为空",
                cell_index + 1
            )));
        }
        let value = parse_cell(&schema.cell.kind, &entry[2]).map_err(|error| {
            Adm4Error::invalid_input(format!(
                "第 {} 格 [{row} × {col}] 的值：{}",
                cell_index + 1,
                error.message
            ))
        })?;
        cells.push(MatrixCell {
            row: row.to_string(),
            col: col.to_string(),
            value,
        });
    }
    Ok(ParameterValues::Cells { cells })
}

/// 值变体名（警告文案用）。
fn variant_name(params: &ParameterValues) -> &'static str {
    match params {
        ParameterValues::None => "none",
        ParameterValues::Scalars { .. } => "scalars",
        ParameterValues::Rows { .. } => "rows",
        ParameterValues::Cells { .. } => "cells",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use adm4_decision::{AxisRef, ScalarField};

    fn table_schema() -> TableSchema {
        TableSchema {
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
                ScalarField {
                    key: "speed".into(),
                    kind: ValueKind::Float,
                    constraint: None,
                    required: false,
                    is_skin: false,
                },
            ],
            row_key: "id".into(),
            cardinality_key: "guard_types".into(),
        }
    }

    fn matrix_schema() -> MatrixSchema {
        MatrixSchema {
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
        }
    }

    #[test]
    fn table_round_trips_through_buffer() {
        let schema = table_schema();
        let params = table_buffer_to_params(
            &schema,
            &[
                vec!["archer".into(), "100".into(), "1.5".into()],
                vec!["mage".into(), "150".into(), String::new()],
            ],
        )
        .unwrap();
        let ParameterValues::Rows { rows } = &params else {
            panic!("expected rows");
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["cost"], TypedValue::Int(100));
        assert_eq!(rows[0]["speed"], TypedValue::Float(1.5));
        // 留空格不写键，由后端按 schema 报缺。
        assert!(!rows[1].contains_key("speed"));

        let (buffer, warnings) = table_buffer_from_params(&schema, &params);
        assert!(warnings.is_empty());
        assert_eq!(buffer[0], vec!["archer", "100", "1.5"]);
        assert_eq!(buffer[1], vec!["mage", "150", ""]);
    }

    #[test]
    fn table_parse_failure_reports_row_and_column() {
        let schema = table_schema();
        let error = table_buffer_to_params(
            &schema,
            &[vec!["archer".into(), "abc".into(), String::new()]],
        )
        .unwrap_err();
        assert!(error.message.contains("第 1 行"), "{}", error.message);
        assert!(error.message.contains("cost"), "{}", error.message);
        assert!(error.message.contains("整数"), "{}", error.message);
    }

    #[test]
    fn table_buffer_warns_on_extra_keys_and_wrong_variant() {
        let schema = table_schema();
        let mut row: BTreeMap<String, TypedValue> = BTreeMap::new();
        row.insert("id".into(), TypedValue::Text("archer".into()));
        row.insert("ghost".into(), TypedValue::Int(1));
        let (_, warnings) =
            table_buffer_from_params(&schema, &ParameterValues::Rows { rows: vec![row] });
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("ghost"), "{}", warnings[0]);

        let (buffer, warnings) = table_buffer_from_params(
            &schema,
            &ParameterValues::Scalars {
                entries: BTreeMap::new(),
            },
        );
        assert!(buffer.is_empty());
        assert!(warnings[0].contains("scalars"), "{}", warnings[0]);
    }

    #[test]
    fn matrix_round_trips_and_rejects_blank_axis() {
        let schema = matrix_schema();
        let params = matrix_buffer_to_params(
            &schema,
            &[vec!["archer".into(), "walker".into(), "1.5".into()]],
        )
        .unwrap();
        let ParameterValues::Cells { cells } = &params else {
            panic!("expected cells");
        };
        assert_eq!(cells[0].value, TypedValue::Float(1.5));

        let (buffer, warnings) = matrix_buffer_from_params(&params);
        assert!(warnings.is_empty());
        assert_eq!(buffer, vec![vec!["archer", "walker", "1.5"]]);

        let error = matrix_buffer_to_params(
            &schema,
            &[vec![String::new(), "walker".into(), "1.5".into()]],
        )
        .unwrap_err();
        assert!(error.message.contains("不能为空"), "{}", error.message);

        let error = matrix_buffer_to_params(
            &schema,
            &[vec!["archer".into(), "walker".into(), "快".into()]],
        )
        .unwrap_err();
        assert!(error.message.contains("数值"), "{}", error.message);
    }

    #[test]
    fn bool_and_text_cells_parse_deterministically() {
        assert_eq!(
            parse_cell(&ValueKind::Bool, " true ").unwrap(),
            TypedValue::Bool(true)
        );
        assert!(parse_cell(&ValueKind::Bool, "yes").is_err());
        // Text/Enum 原样保留，合法性交给后端校验。
        assert_eq!(
            parse_cell(
                &ValueKind::Enum {
                    variants: vec!["a".into()]
                },
                "not_a_variant"
            )
            .unwrap(),
            TypedValue::Text("not_a_variant".into())
        );
    }
}
