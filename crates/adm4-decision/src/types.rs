use adm4_contracts::{MatrixCell, TypedValue, ValueConstraint, ValueKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type DecisionId = String;
pub type OptionId = String;
pub type DomainId = String;
pub type GenrePackId = String;
pub type ParamPath = String;

/// L 层梯度（全局唯一语义）。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum DesignLevel {
    L0,
    L1,
    L2,
    L3,
    #[default]
    L4,
    L5,
    L6,
}

impl DesignLevel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::L0 => "L0 项目档案",
            Self::L1 => "L1 体验方向",
            Self::L2 => "L2 品类结构",
            Self::L3 => "L3 系统组成",
            Self::L4 => "L4 机制规则",
            Self::L5 => "L5 实体与参数结构",
            Self::L6 => "L6 数值表",
        }
    }

    pub fn all() -> [Self; 7] {
        [
            Self::L0,
            Self::L1,
            Self::L2,
            Self::L3,
            Self::L4,
            Self::L5,
            Self::L6,
        ]
    }
}

/// 项目深度档：创建时选定，全局统一；最低 L4。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepthProfile {
    pub target: DesignLevel,
}

impl DepthProfile {
    pub fn new(target: DesignLevel) -> Result<Self, String> {
        if target < DesignLevel::L4 {
            return Err(format!(
                "depth profile target must be >= L4, got {target:?}"
            ));
        }
        Ok(Self { target })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenreScope {
    Universal,
    Pack(GenrePackId),
}

/// MDA 展示标签（继承二版，UI 分组与访谈话术用，不参与门禁）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MdaLayer {
    Aesthetics,
    Dynamics,
    Mechanics,
    Constraints,
    Evidence,
}

impl MdaLayer {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Aesthetics => "体验目标",
            Self::Dynamics => "玩家动态",
            Self::Mechanics => "机制抓手",
            Self::Constraints => "边界约束",
            Self::Evidence => "验收信号",
        }
    }
}

/// 决策点适用性要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PointRequirement {
    /// 默认：纯激活驱动，父选项 unlock 才需回答。
    #[default]
    Unlocked,
    /// 基线：建议回答，但可给结构化理由码显式 N/A。
    Baseline,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OptionSelector {
    pub decision: DecisionId,
    pub option: OptionId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScalarField {
    pub key: String,
    pub kind: ValueKind,
    #[serde(default)]
    pub constraint: Option<ValueConstraint>,
    #[serde(default)]
    pub required: bool,
    /// 该字段是否属于「皮」（命名/主题/文案），换皮门比对粒度。
    #[serde(default)]
    pub is_skin: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableSchema {
    pub columns: Vec<ScalarField>,
    pub row_key: String,
    /// 对应品类包 cardinality_expectations 的键。
    pub cardinality_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "axis", rename_all = "snake_case")]
pub enum AxisRef {
    DecisionOptions { decision: DecisionId },
    TableRows { decision: DecisionId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatrixSchema {
    pub row_axis: AxisRef,
    pub col_axis: AxisRef,
    pub cell: ScalarField,
    /// 对应品类包 cardinality_expectations 的键（行数期望）。
    pub cardinality_key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "schema", rename_all = "snake_case")]
pub enum ParameterSchema {
    #[default]
    None,
    Scalar {
        fields: Vec<ScalarField>,
    },
    Table(TableSchema),
    Matrix(MatrixSchema),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DecisionOption {
    pub id: OptionId,
    pub label: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub implications: Vec<String>,
    #[serde(default)]
    pub requires: Vec<OptionSelector>,
    #[serde(default)]
    pub conflicts: Vec<OptionSelector>,
    /// 选中后激活哪些深层决策点（适用性判定驱动）。
    #[serde(default)]
    pub unlocks: Vec<DecisionId>,
    #[serde(default)]
    pub parameter_schema: ParameterSchema,
    #[serde(default)]
    pub is_custom: bool,
    /// C0 编译提示（数据驱动，键值由清单声明）：
    /// `spec_role`（system/mechanic/entity_table/data_table/content/title）、
    /// `system`（机制归属的系统决策 id）、`visual_form`、`content_kind` 等。
    #[serde(default)]
    pub compiler_tags: BTreeMap<String, String>,
    /// L4 机制的效果模板（EffectSpec 的 JSON 形态，支持 `{param:KEY}` 占位符）。
    /// 缺失时 C0 按 R2 阻塞——流水线不发明效果。
    #[serde(default)]
    pub effects_template: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionPoint {
    pub id: DecisionId,
    pub domain: DomainId,
    pub level: DesignLevel,
    pub genre_scope: GenreScope,
    pub question: String,
    #[serde(default)]
    pub mda_layer: Option<MdaLayer>,
    #[serde(default)]
    pub requirement: PointRequirement,
    pub options: Vec<DecisionOption>,
    /// 属于「皮」的参数路径（换皮门只查这些）。
    #[serde(default)]
    pub skin_fields: Vec<ParamPath>,
    /// 模板逆向时此点是否需要来源标注。
    #[serde(default)]
    pub evidence_slots: bool,
}

impl DecisionPoint {
    pub fn option(&self, option_id: &str) -> Option<&DecisionOption> {
        self.options.iter().find(|option| option.id == option_id)
    }
}

// ---------------------------------------------------------------------------
// Selection（项目状态侧）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provenance", rename_all = "snake_case")]
pub enum Provenance {
    UserManual,
    AiInterviewConfirmed,
    Template { template_id: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "values", rename_all = "snake_case")]
pub enum ParameterValues {
    #[default]
    None,
    Scalars {
        entries: BTreeMap<String, TypedValue>,
    },
    Rows {
        rows: Vec<BTreeMap<String, TypedValue>>,
    },
    Cells {
        cells: Vec<MatrixCell>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    pub decision_id: DecisionId,
    pub option_id: OptionId,
    #[serde(default)]
    pub parameters: ParameterValues,
    #[serde(default)]
    pub rationale: String,
    pub provenance: Provenance,
    #[serde(default)]
    pub confirmed_by_user: bool,
    /// 模板预填时记录原参数值，供换皮门逐字段比对。
    #[serde(default)]
    pub template_original: Option<ParameterValues>,
}

/// 显式 N/A 的结构化理由（机器可判定理由码，非散文）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaJustification {
    pub reason_code: String,
    #[serde(default)]
    pub note: String,
}
