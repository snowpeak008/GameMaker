use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};

/// 参数/属性的类型系统。决策模型与 GameSpec 共用（避免两套类型体系分叉）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValueKind {
    Int,
    Float,
    Bool,
    Text,
    Enum { variants: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "constraint", rename_all = "snake_case")]
pub enum ValueConstraint {
    Range { min: f64, max: f64 },
    MinLen { min: usize },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TypedValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
}

impl TypedValue {
    pub fn type_matches(&self, kind: &ValueKind) -> bool {
        match (self, kind) {
            (TypedValue::Int(_), ValueKind::Int) => true,
            (TypedValue::Float(_), ValueKind::Float) => true,
            (TypedValue::Int(_), ValueKind::Float) => true,
            (TypedValue::Bool(_), ValueKind::Bool) => true,
            (TypedValue::Text(_), ValueKind::Text) => true,
            (TypedValue::Text(text), ValueKind::Enum { variants }) => {
                variants.iter().any(|variant| variant == text)
            }
            _ => false,
        }
    }

    pub fn check_constraint(&self, constraint: &ValueConstraint) -> Adm4Result<()> {
        match (self, constraint) {
            (value, ValueConstraint::Range { min, max }) => {
                let number = match value {
                    TypedValue::Int(int_value) => *int_value as f64,
                    TypedValue::Float(float_value) => *float_value,
                    _ => return Ok(()),
                };
                if number < *min || number > *max {
                    return Err(Adm4Error::validation(format!(
                        "value {number} out of range [{min}, {max}]"
                    )));
                }
                Ok(())
            }
            (TypedValue::Text(text), ValueConstraint::MinLen { min }) => {
                if text.chars().count() < *min {
                    return Err(Adm4Error::validation(format!(
                        "text length {} below minimum {min}",
                        text.chars().count()
                    )));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn render(&self) -> String {
        match self {
            TypedValue::Bool(value) => value.to_string(),
            TypedValue::Int(value) => value.to_string(),
            TypedValue::Float(value) => value.to_string(),
            TypedValue::Text(value) => value.clone(),
        }
    }
}

/// 矩阵单元格。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatrixCell {
    pub row: String,
    pub col: String,
    pub value: TypedValue,
}

/// GameSpec 路径引用（锚定用，如 `mechanics/counter_damage`）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SpecRef(pub String);

impl SpecRef {
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }
}
