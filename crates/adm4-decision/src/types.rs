use adm4_contracts::{MatrixCell, TypedValue, ValueConstraint, ValueKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type DecisionId = String;
pub type OptionId = String;
pub type DomainId = String;
pub type NodeId = String;
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

/// 决策点的选择基数：单选（默认）或多选。
///
/// 二版的 L4 选项组支持「多选 + 主选」，四版原本只有单选。多选点的已选集合落在
/// `Selection`（`option_id` + `additional_options`），主选落 `Selection::primary_option`。
/// 旧清单没有该键 → `serde(default)` → `Single`，行为与扩展前逐字节一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum SelectionMode {
    /// 单选：一个决策点最多一个已选选项。
    #[default]
    Single,
    /// 多选：至少选 1 个；`allow_primary=true` 时必须从已选集合里指定一个主选。
    Multi {
        #[serde(default)]
        allow_primary: bool,
    },
}

impl SelectionMode {
    pub fn is_multi(&self) -> bool {
        matches!(self, Self::Multi { .. })
    }

    /// 是否要求标记主选（单选点恒 false）。
    pub fn requires_primary(&self) -> bool {
        matches!(
            self,
            Self::Multi {
                allow_primary: true
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionPoint {
    pub id: DecisionId,
    /// 编译期分组标签（C0 用它把 L4 机制归属到同域 L3 系统）。
    /// 注意：这**不是**左栏「设计领域」——领域归属走 `node_id` → 节点 → 领域。
    pub domain: DomainId,
    pub level: DesignLevel,
    pub genre_scope: GenreScope,
    pub question: String,
    #[serde(default)]
    pub mda_layer: Option<MdaLayer>,
    /// 二版「设计提问」：比 `question` 更钩子化的追问话术，UI 与访谈提示词展示用。
    #[serde(default)]
    pub design_question: Option<String>,
    /// 所属设计节点（领域/节点两级组织的挂载点）。
    /// 缺省 → 归入保留节点/保留领域「未分域」（见 `organization` 模块常量），
    /// 因此旧清单无需声明该键即可参与领域/节点聚合。
    #[serde(default)]
    pub node_id: Option<NodeId>,
    #[serde(default)]
    pub selection_mode: SelectionMode,
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

    pub fn is_multi(&self) -> bool {
        self.selection_mode.is_multi()
    }

    pub fn requires_primary(&self) -> bool {
        self.selection_mode.requires_primary()
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

/// 多选点上的一个附加已选选项（首个已选选项仍平铺在 `Selection` 上，保持存档兼容）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SelectedOption {
    pub option_id: OptionId,
    #[serde(default)]
    pub parameters: ParameterValues,
    #[serde(default)]
    pub rationale: String,
    /// 模板预填时记录原参数值，供换皮门逐字段比对（与 `Selection::template_original` 同义）。
    #[serde(default)]
    pub template_original: Option<ParameterValues>,
}

/// 一个已选选项的只读视图：把 `Selection` 平铺的首选项与 `additional_options`
/// 统一成一串可遍历的条目，让完成度/一致性/换皮/C0 各处「按已选选项逐个处理」，
/// 不必各自解构多选结构（避免遗漏多选点）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectedOptionRef<'a> {
    pub option_id: &'a str,
    pub parameters: &'a ParameterValues,
    pub rationale: &'a str,
    pub template_original: Option<&'a ParameterValues>,
    /// 多选点的主选标记（单选点恒 false——单选无主选概念）。
    pub is_primary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    pub decision_id: DecisionId,
    /// 首个已选选项；单选点即唯一选项（存档兼容锚点，语义不变）。
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
    /// 多选点的其余已选选项（单选点恒空）。
    #[serde(default)]
    pub additional_options: Vec<SelectedOption>,
    /// 多选点的主选选项 id；必须落在已选集合内（完成度门校验）。
    #[serde(default)]
    pub primary_option: Option<OptionId>,
}

impl Selection {
    /// 全部已选选项，**主选排在最前**，其余保持声明顺序。
    /// 单选点返回 1 条，且 `is_primary=false`。
    pub fn selected_options(&self) -> Vec<SelectedOptionRef<'_>> {
        let is_primary = |option_id: &str| self.primary_option.as_deref() == Some(option_id);
        let mut refs = Vec::with_capacity(1 + self.additional_options.len());
        refs.push(SelectedOptionRef {
            option_id: self.option_id.as_str(),
            parameters: &self.parameters,
            rationale: self.rationale.as_str(),
            template_original: self.template_original.as_ref(),
            is_primary: is_primary(&self.option_id),
        });
        for extra in &self.additional_options {
            refs.push(SelectedOptionRef {
                option_id: extra.option_id.as_str(),
                parameters: &extra.parameters,
                rationale: extra.rationale.as_str(),
                template_original: extra.template_original.as_ref(),
                is_primary: is_primary(&extra.option_id),
            });
        }
        // 稳定排序：主选提前，其余次序不变。
        refs.sort_by_key(|item| !item.is_primary);
        refs
    }

    /// 已选选项 id（顺序同 `selected_options`）。
    pub fn selected_option_ids(&self) -> Vec<&str> {
        self.selected_options()
            .into_iter()
            .map(|item| item.option_id)
            .collect()
    }

    pub fn contains_option(&self, option_id: &str) -> bool {
        self.option_id == option_id
            || self
                .additional_options
                .iter()
                .any(|extra| extra.option_id == option_id)
    }

    /// 已选选项数量（≥1）。
    pub fn selected_count(&self) -> usize {
        1 + self.additional_options.len()
    }

    /// 规范化副本：把主选搬到 `option_id` 位（其余保持相对顺序）。
    ///
    /// 冻结时用它落 `FrozenDesign`，让「主选排序在前」在产物里是字面事实，
    /// 而不是只能靠 `selected_options()` 推导——下游读 JSON 的工具也能直接看出主次。
    pub fn with_primary_first(&self) -> Self {
        let Some(primary) = self.primary_option.as_deref() else {
            return self.clone();
        };
        if self.option_id == primary || !self.contains_option(primary) {
            return self.clone();
        }
        let mut head: Option<SelectedOption> = None;
        let mut rest: Vec<SelectedOption> = Vec::with_capacity(self.additional_options.len());
        for extra in &self.additional_options {
            if extra.option_id == primary && head.is_none() {
                head = Some(extra.clone());
            } else {
                rest.push(extra.clone());
            }
        }
        let Some(head) = head else {
            return self.clone();
        };
        // 原首选项降级为附加选项，插到最前保持「除主选外次序不变」。
        rest.insert(
            0,
            SelectedOption {
                option_id: self.option_id.clone(),
                parameters: self.parameters.clone(),
                rationale: self.rationale.clone(),
                template_original: self.template_original.clone(),
            },
        );
        Self {
            decision_id: self.decision_id.clone(),
            option_id: head.option_id,
            parameters: head.parameters,
            rationale: head.rationale,
            provenance: self.provenance.clone(),
            confirmed_by_user: self.confirmed_by_user,
            template_original: head.template_original,
            additional_options: rest,
            primary_option: self.primary_option.clone(),
        }
    }
}

/// 显式 N/A 的结构化理由（机器可判定理由码，非散文）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaJustification {
    pub reason_code: String,
    #[serde(default)]
    pub note: String,
}
