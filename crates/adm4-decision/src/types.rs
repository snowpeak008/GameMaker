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
///
/// 旧清单没有该键 → `serde(default)` → `Unlocked`，与扩展前行为逐字节一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PointRequirement {
    /// 默认：纯激活驱动，父选项 unlock 才需回答。
    #[default]
    Unlocked,
    /// 基线：建议回答，但可给结构化理由码显式 N/A。
    Baseline,
    /// 非必做（二版检查单项 `required=false` 的归宿）：激活判定与 `Unlocked` 相同，
    /// 但**未作答时不进完成度分母、不构成冻结门第 1 道的阻塞项**——设计者可以整域跳过
    /// 而不把完成度拖低。
    ///
    /// 一旦作答就按普通点校验：作答等于把这个点纳入本项目的设计，参数缺格、主选缺失、
    /// 悬空外键仍照常拦（否则 R2 会被绕过——非法答案会一路带进 `FrozenDesign` 与 C0）。
    Optional,
}

impl PointRequirement {
    /// 未作答时是否可以不进完成度分母。
    pub fn is_optional(&self) -> bool {
        matches!(self, Self::Optional)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Unlocked => "激活驱动",
            Self::Baseline => "基线点",
            Self::Optional => "非必做",
        }
    }
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

/// 图入口约束（W7 定稿 §5.4）。
///
/// 决策侧自有枚举而不引用 `adm4-spec::GraphEntry`：decision 是 spec 的上游，
/// 反向依赖会成环。serde 形态与 spec 侧逐字一致（snake_case 单词 tag），
/// C0 做确定性映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GraphEntryConstraint {
    /// 入度 0 的节点恰好 1 个（天赋树根、肉鸽地图起点）。
    Single,
    /// 不限入口（对话 hub 图）。
    #[default]
    Multiple,
}

/// 图结构参数的 schema（W7 定稿 §5.4：节点/边负载复用 ScalarField 含 is_skin）。
///
/// `acyclic` 默认 **false**（定稿指令 10：对话/图类模块默认可回环，
/// 仅天赋树/肉鸽地图类显式声明 true）。图值本身沿 Curve 先例以标量参数
/// `graph` 键装 GraphSpec 形态的 JSON 文本（`ParameterValues` 零改动），
/// C0 以本 schema 为真相覆盖 directed/acyclic/entry。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct GraphSchema {
    /// 节点负载字段（required 字段缺失即校验问题；is_skin 供换皮门比对粒度）。
    pub node_payload: Vec<ScalarField>,
    /// 边负载字段。
    pub edge_payload: Vec<ScalarField>,
    pub directed: bool,
    /// 默认 false（定稿指令 10）。
    pub acyclic: bool,
    pub entry: GraphEntryConstraint,
    /// 节点数期望的基数键（空 = 不检查），对应品类包 cardinality_expectations。
    pub cardinality_key: String,
}

/// 曲线参数的 schema（W7 定稿 §5.4）。
///
/// 曲线值沿波 1 先例以标量参数 `curve` 键装 CurveSpec JSON 文本，
/// C0 编译成两列 Table + 插值注记（不加新 section）。schema 分支的意义是
/// 让「这个选项收曲线」成为清单里的声明事实，而不再只靠 compiler_tags 约定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CurveSchema {
    /// 采样点数期望的基数键（空 = 不检查）。
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
    /// W7 §5.4：图结构参数（旧 tag 零改动，新 tag=graph）。
    Graph(GraphSchema),
    /// W7 §5.4：曲线参数（tag=curve）。
    Curve(CurveSchema),
}

/// 图值（GraphSpec 形态 JSON）对 schema 的结构校验纯函数（W7 定稿 §5.4）。
///
/// 校验项：nodes/edges 形态、节点 id 唯一、边端点已声明、必填负载字段在场、
/// `schema.acyclic=true` 时 Kahn 拓扑检查无环、`entry=Single` 时入度 0 节点恰 1。
/// 返回问题清单（空 = 通过），供确认时（completeness）与 C0 前置拦截共用——
/// I3 校验前置：非法图值在创作期就点名，不等冻结后的编译才爆。
pub fn validate_graph_value(schema: &GraphSchema, value: &serde_json::Value) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(root) = value.as_object() else {
        return vec!["图值必须是 JSON 对象（GraphSpec 形态：nodes/edges）".to_string()];
    };
    let nodes = match root.get("nodes").and_then(|nodes| nodes.as_array()) {
        Some(nodes) => nodes,
        None => {
            problems.push("图值缺少 nodes 数组".to_string());
            return problems;
        }
    };
    let mut node_ids: Vec<&str> = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let Some(id) = node.get("id").and_then(|id| id.as_str()) else {
            problems.push(format!("图节点第 {index} 项缺少字符串 id"));
            continue;
        };
        if node_ids.contains(&id) {
            problems.push(format!("图节点 id 重复：{id}"));
        } else {
            node_ids.push(id);
        }
        for field in &schema.node_payload {
            if field.required
                && node
                    .get("payload")
                    .and_then(|payload| payload.get(&field.key))
                    .is_none()
            {
                problems.push(format!("图节点 {id} 缺少必填负载字段 {}", field.key));
            }
        }
    }
    let empty = Vec::new();
    let edges = root
        .get("edges")
        .and_then(|edges| edges.as_array())
        .unwrap_or(&empty);
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut indegree: BTreeMap<&str, usize> = node_ids.iter().map(|id| (*id, 0usize)).collect();
    for (index, edge) in edges.iter().enumerate() {
        let from = edge.get("from").and_then(|from| from.as_str());
        let to = edge.get("to").and_then(|to| to.as_str());
        let (Some(from), Some(to)) = (from, to) else {
            problems.push(format!("图边第 {index} 项缺少 from/to 字符串端点"));
            continue;
        };
        for endpoint in [from, to] {
            if !node_ids.contains(&endpoint) {
                problems.push(format!(
                    "图边 {from}→{to} 的端点 {endpoint} 未在 nodes 声明"
                ));
            }
        }
        for field in &schema.edge_payload {
            if field.required
                && edge
                    .get("payload")
                    .and_then(|payload| payload.get(&field.key))
                    .is_none()
            {
                problems.push(format!("图边 {from}→{to} 缺少必填负载字段 {}", field.key));
            }
        }
        if node_ids.contains(&from) && node_ids.contains(&to) {
            adjacency.entry(from).or_default().push(to);
            if let Some(degree) = indegree.get_mut(to) {
                *degree += 1;
            }
        }
    }
    if schema.acyclic && has_graph_value_cycle(&node_ids, &adjacency, &indegree) {
        problems.push("schema 声明 acyclic=true 但图值存在环".to_string());
    }
    if schema.entry == GraphEntryConstraint::Single {
        let entry_count = indegree.values().filter(|degree| **degree == 0).count();
        if entry_count != 1 {
            problems.push(format!(
                "schema 要求单入口（entry=single），实际入度 0 节点 {entry_count} 个"
            ));
        }
    }
    problems
}

/// Kahn 拓扑排序成环检测（按 from→to 有向读法；无向图声明 acyclic 同样保守查有向环，
/// 与 adm4-spec 侧 `validate_game_spec` 的口径一致）。
fn has_graph_value_cycle(
    node_ids: &[&str],
    adjacency: &BTreeMap<&str, Vec<&str>>,
    indegree: &BTreeMap<&str, usize>,
) -> bool {
    let mut indegree = indegree.clone();
    let mut queue: Vec<&str> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut visited = 0usize;
    while let Some(current) = queue.pop() {
        visited += 1;
        if let Some(children) = adjacency.get(current) {
            for child in children {
                if let Some(degree) = indegree.get_mut(child) {
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push(child);
                    }
                }
            }
        }
    }
    visited != node_ids.len()
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
    /// 所属重度档 id（W7 系统模块，定稿 §5.1）：项目对该模块选定的档位未达此档时，
    /// 点不激活、不进完成度分母。None = 不受档位门控——旧清单没有该键 →
    /// `serde(default)` → None，行为与扩展前逐字节一致（I2 旧档守恒）。
    #[serde(default)]
    pub tier_gate: Option<String>,
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

/// 显式 N/A 的结构化理由（机器可判定理由码，非散文）+ 人工豁免的署名（R3）。
///
/// 两条来路共用本结构：
/// - baseline 点的理由码跳过：只有 `reason_code`（+可选 `note`），`actor`/`at` 留空；
/// - 人工豁免适用点：`reason_code`/`note`/`actor` 三者非空，`at` 由引擎盖时间戳。
///
/// `actor`/`at` 原先存在 `AuthoringState.na_signoffs` 并行 map 里（T11 无权改本文件所致）。
/// 并行 map 要求两处键始终同步，任一处漏删就会出现「豁免已解除但署名还在」的幽灵记录；
/// 合并进本结构后这类不一致在类型层面就不可能发生。两个新键都 `serde(default)`，
/// 旧存档（无这两键）照旧可读，其 N/A 视为无署名的历史条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NaJustification {
    pub reason_code: String,
    #[serde(default)]
    pub note: String,
    /// 人工豁免的署名（人名/账号）；baseline 理由码跳过与旧存档为空串。
    #[serde(default)]
    pub actor: String,
    /// 署名时间（ISO8601）；无署名时为空串。
    #[serde(default)]
    pub at: String,
}

impl NaJustification {
    /// baseline 点的理由码跳过（无署名）。
    pub fn reason_code_only(reason_code: impl Into<String>, note: impl Into<String>) -> Self {
        Self {
            reason_code: reason_code.into(),
            note: note.into(),
            actor: String::new(),
            at: String::new(),
        }
    }

    /// 是否带人工署名（R3 可追责的豁免）。
    pub fn is_signed(&self) -> bool {
        !self.actor.trim().is_empty()
    }

    /// 署名展示文本（无署名时说明它是理由码跳过或旧存档条目）。
    pub fn signature_label(&self) -> String {
        if self.is_signed() {
            format!("署名 {}（{}）", self.actor, self.at)
        } else {
            "无署名（baseline 理由码跳过或旧存档条目）".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---------------- ParameterSchema Graph/Curve tag（W7 §5.4） ----------------

    #[test]
    fn parameter_schema_graph_curve_serde_shapes_and_roundtrip() {
        let graph = ParameterSchema::Graph(GraphSchema {
            node_payload: vec![ScalarField {
                key: "cost".into(),
                kind: adm4_contracts::ValueKind::Int,
                constraint: None,
                required: true,
                is_skin: false,
            }],
            edge_payload: Vec::new(),
            directed: true,
            acyclic: true,
            entry: GraphEntryConstraint::Single,
            cardinality_key: "talent_nodes".into(),
        });
        let value = serde_json::to_value(&graph).expect("序列化应成功");
        assert_eq!(value.get("schema"), Some(&json!("graph")));
        let back: ParameterSchema = serde_json::from_value(value).expect("反序列化应成功");
        assert_eq!(back, graph);

        let curve = ParameterSchema::Curve(CurveSchema {
            cardinality_key: "xp_points".into(),
        });
        let value = serde_json::to_value(&curve).expect("序列化应成功");
        assert_eq!(value.get("schema"), Some(&json!("curve")));
        let back: ParameterSchema = serde_json::from_value(value).expect("反序列化应成功");
        assert_eq!(back, curve);

        // 旧 tag 零改动：table/matrix/scalar/none 的 serde 形态不受新增分支影响。
        let legacy: ParameterSchema =
            serde_json::from_str(r#"{"schema":"none"}"#).expect("旧 tag 应可读");
        assert_eq!(legacy, ParameterSchema::None);
    }

    #[test]
    fn graph_schema_defaults_match_w7_directive_10() {
        // 缺键 JSON：acyclic 默认 false（对话类可回环）、entry 默认 multiple。
        let schema: GraphSchema = serde_json::from_str("{}").expect("空对象应可读");
        assert!(!schema.acyclic, "acyclic 默认必须是 false（定稿指令 10）");
        assert!(!schema.directed);
        assert_eq!(schema.entry, GraphEntryConstraint::Multiple);
        assert!(schema.cardinality_key.is_empty());
    }

    // ---------------- validate_graph_value 校验纯函数 ----------------

    fn talent_schema() -> GraphSchema {
        GraphSchema {
            node_payload: vec![ScalarField {
                key: "cost".into(),
                kind: adm4_contracts::ValueKind::Int,
                constraint: None,
                required: true,
                is_skin: false,
            }],
            edge_payload: Vec::new(),
            directed: true,
            acyclic: true,
            entry: GraphEntryConstraint::Single,
            cardinality_key: String::new(),
        }
    }

    fn talent_value() -> serde_json::Value {
        json!({
            "nodes": [
                { "id": "root", "payload": { "cost": 1 } },
                { "id": "left", "payload": { "cost": 2 } },
                { "id": "right", "payload": { "cost": 2 } }
            ],
            "edges": [
                { "from": "root", "to": "left" },
                { "from": "root", "to": "right" }
            ]
        })
    }

    #[test]
    fn graph_value_wellformed_passes() {
        assert!(validate_graph_value(&talent_schema(), &talent_value()).is_empty());
    }

    #[test]
    fn graph_value_dangling_edge_endpoint_named() {
        let mut value = talent_value();
        value["edges"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "from": "left", "to": "ghost" }));
        let problems = validate_graph_value(&talent_schema(), &value);
        assert!(
            problems.iter().any(|p| p.contains("ghost")),
            "应点名悬空端点，实际：{problems:?}"
        );
    }

    #[test]
    fn graph_value_cycle_rejected_when_schema_acyclic() {
        let mut value = talent_value();
        value["edges"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "from": "left", "to": "root" }));
        let problems = validate_graph_value(&talent_schema(), &value);
        assert!(
            problems.iter().any(|p| p.contains("环")),
            "acyclic 声明下的环必须被拒，实际：{problems:?}"
        );
        // 同构图在 acyclic=false 的 schema 下放行（对话 hub 回环合法）。
        let mut hub = talent_schema();
        hub.acyclic = false;
        hub.entry = GraphEntryConstraint::Multiple;
        assert!(validate_graph_value(&hub, &value).is_empty());
    }

    #[test]
    fn graph_value_single_entry_enforced() {
        let mut value = talent_value();
        value["nodes"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "id": "orphan", "payload": { "cost": 3 } }));
        let problems = validate_graph_value(&talent_schema(), &value);
        assert!(
            problems.iter().any(|p| p.contains("入度 0 节点 2")),
            "单入口约束必须点名入口数，实际：{problems:?}"
        );
    }

    #[test]
    fn graph_value_missing_required_payload_and_duplicate_node() {
        let value = json!({
            "nodes": [
                { "id": "a" },
                { "id": "a", "payload": { "cost": 1 } }
            ],
            "edges": []
        });
        let problems = validate_graph_value(&talent_schema(), &value);
        assert!(
            problems.iter().any(|p| p.contains("缺少必填负载字段 cost")),
            "必填负载缺失必须点名，实际：{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("重复")),
            "节点 id 重复必须点名，实际：{problems:?}"
        );
        // entry=Single 在缺 nodes 数组时不误报：形态错误先行返回。
        let malformed = json!("not an object");
        let problems = validate_graph_value(&talent_schema(), &malformed);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("JSON 对象"));
    }
}
