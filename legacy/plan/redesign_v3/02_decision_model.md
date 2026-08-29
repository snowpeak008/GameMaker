# v3 实施子计划 · 02 · 决策模型 schema

> 上位：[00 §3](00_master_design.md) · [01 总纲](01_overview_and_milestones.md)
> 里程碑：R1（决策模型骨架，无数据）
> 落点 crate：`adm-new-design`（引擎）+ `adm-new-contracts`（DTO）+ `adm-new-game-spec`（哈希/canonical 复用）

---

## 1. 目标与非目标

**目标**：把 00 号 §3.2 的 DecisionPoint/Selection 数据模型落成 Rust 类型 + 加载 + DAG 校验 + 完成度计算的**空骨架**——能加载一张（空的或用户填的）决策图、校验其内部一致性、计算完成度，但**不含任何具体决策点内容**（内容走文档 03 输入门）。

**非目标**：不定义塔防/战棋的具体决策点（那是用户设计空间清单，文档 03）；不做 UI（文档 08）；不做冻结门（文档 05）。

---

## 2. 与现有模型的关系（不从零造）

探测确认 `adm-new-design` 已有两套可借用的结构：

| 现有结构 | 位置 | v3 借用方式 |
|---------|------|-----------|
| `DomainDocument`/`DomainNode`/`ChecklistItem`/`OptionGroup`/`OptionItem`/`OptionRelation` | `data_loader/mod.rs` | 数据加载骨架的**参照**；v3 DecisionPoint 是其超集（加 level/genre_scope/parameter_schema/skin_fields/evidence_slots） |
| `OptionRelation`/`OptionRef` | `data_loader/mod.rs` | requires/conflicts 的现成表达，v3 直接沿用语义 |
| `CapabilityDecisionGraph`/`CapabilityDecisionGraphCompiler`/`DecisionEdge`/`ConstraintKey`/`DecisionCoverage` | `decision_graph/mod.rs` | **DAG 引擎候选**（§4 评估） |
| `ProjectState`/`NodeState`/`DecisionState`/`ChecklistOptionGroupState`/`OptionProvenanceEntry` | `contracts/project.rs` | Selection 的落点；`OptionProvenanceEntry` 已有 provenance 语义 |

**结论**：v3 决策模型是现有 checklist/option 模型的**结构化升级**，不是替换文件格式的推倒重来。新类型放新模块，旧数据文件保留（01 号 §3 对账）。

---

## 3. 类型设计（Rust，落 `adm-new-design/src/decision_model/`）

### 3.1 DecisionPoint

```rust
/// 一个决策点（00 §3.2）。封闭枚举选项 + typed 参数。
pub struct DecisionPoint {
    pub id: DecisionId,
    pub domain: DomainId,
    pub level: DesignLevel,            // L0..=L6
    pub genre_scope: GenreScope,       // Universal | Pack(GenrePackId)
    pub question: String,
    pub options: Vec<DecisionOption>,
    pub skin_fields: Vec<ParamPath>,   // 哪些参数属于"皮"，换皮门只查这些
    pub evidence_slots: bool,          // 模板逆向时此点是否需来源标注
}

pub enum DesignLevel { L0, L1, L2, L3, L4, L5, L6 }

pub enum GenreScope {
    Universal,
    Pack(GenrePackId),
}

pub struct DecisionOption {
    pub id: OptionId,
    pub label: String,
    pub summary: String,
    pub implications: Vec<String>,
    pub requires: Vec<OptionSelector>,   // 前置约束（decision.option）
    pub conflicts: Vec<OptionSelector>,  // 冲突约束
    pub parameter_schema: ParameterSchema, // 选定后要填的参数（typed）
    pub is_custom: bool,                  // 显式 custom 选项（00 §3.2 要点）
}

/// 指向"某决策点的某选项"，requires/conflicts 用。
pub struct OptionSelector { pub decision: DecisionId, pub option: OptionId }
```

### 3.2 参数与表结构（L5/L6 的核心）

00 号 §3.2 要点：**L5/L6 的 parameter_schema 直接定义实体属性表/克制矩阵/波次表的列结构，L6 填行数据**。这是「数值表成为决策的一部分而非流水线的发明」的落地。

```rust
pub enum ParameterSchema {
    None,
    /// L0–L4 的标量/枚举参数
    Scalar(Vec<ScalarField>),
    /// L5：表结构（列定义），L6 在此结构上填行
    Table(TableSchema),
    /// L5：矩阵（如克制矩阵，行=X 种类、列=Y 种类）
    Matrix(MatrixSchema),
}

pub struct ScalarField {
    pub key: String,
    pub kind: ValueKind,       // 复用 game-spec::ValueKind（int/float/enum/bool/string…）
    pub constraint: Option<ValueConstraint>, // 范围/枚举集/正则
    pub required: bool,
}

pub struct TableSchema {
    pub columns: Vec<ScalarField>,
    pub row_key: String,           // 行标识列（如实体 id）
    pub cardinality: Cardinality,  // 期望行数区间（接品类包基数期望，R6/07）
}

pub struct MatrixSchema {
    pub row_axis: AxisRef,   // 行维度（引用某决策的选项集，如"守卫种类"）
    pub col_axis: AxisRef,   // 列维度
    pub cell: ScalarField,   // 每格类型（如克制系数 float）
}
```

> **设计约束**：`ValueKind`/`ValueConstraint` 尽量复用 `adm-new-game-spec::spec::{ValueKind}`，避免两套类型体系分叉——C0 编译时（文档 06）参数值要直接落进 GameSpec 的 `PropertySpec`。

### 3.3 Selection（一次选择的记录）

```rust
pub struct Selection {
    pub decision_id: DecisionId,
    pub option_id: OptionId,
    pub parameters: ParameterValues,   // 按 parameter_schema 填的值（含表行数据）
    pub rationale: String,             // 为什么选（用户写或 AI 访谈生成）
    pub provenance: Provenance,
    pub confirmed_by_user: bool,       // AI 访谈必须 true 才计入完成度
}

pub enum Provenance {
    UserManual,
    AiInterviewConfirmed,
    Template(TemplateId),
}

pub enum ParameterValues {
    Scalars(BTreeMap<String, TypedValue>),
    Rows(Vec<BTreeMap<String, TypedValue>>),   // 表/矩阵的行
}
```

> `Provenance` 与现有 `contracts/project.rs::OptionProvenanceEntry` 对齐；`Selection` 作为 DTO 进 `contracts`，引擎逻辑留 `design`。

### 3.4 项目深度档（Depth Profile，00 §3.1）

```rust
pub struct DepthProfile { pub target: DesignLevel } // 最低 L4，可选 L5/L6
```

冻结门（文档 05）按 `target` 检查完整性：选 L6 而某表空 = 不能冻结。

### 3.5 适用性判定（解决"简单玩法内容天然少 → 会不会卡完备度门"）

> 背景：纯 IAA 超休闲玩法系统极少，品类包里大量决策点对它并不适用。若完备度门要求"全部决策点都填"，简单项目永远无法全绿。此机制保证**该填的一个不落、用不上的不强求**。

一个决策点对某项目的状态是三选一：

```rust
pub enum PointApplicability {
    Active,                       // 被 DAG 激活：必须 confirmed 才算完成
    Inactive,                     // 未被任何已选选项 unlock：不进分母，天然跳过
    NotApplicable(NaJustification), // 品类包基线点，但本项目合理不需要：显式跳过
}

pub struct NaJustification {
    pub reason_code: String,      // 结构化枚举（如 "no_meta_progression"），非散文
    pub note: String,
}
```

三条规则：
1. **激活式跳过（主力机制）**：决策点靠父选项的 `unlocks` 激活。L0–L2 通用根点恒 `Active`；深层 L4–L6 只有父选项选中才激活。超休闲在 L3 只选极少系统 → 复杂系统的深层点从未激活 = `Inactive` → **不进分母、不卡门**。这是"简单玩法少填"的天然出口，无需额外声明。
2. **显式 N/A（补激活机制的缺口）**：品类包可标记少数决策点为 `baseline`（无论激活与否都建议回答，如"是否有存档")。若某项目合理地不需要，用户/AI 给 `NotApplicable(reason_code)`——**机器可判定的理由码**（枚举，非散文，类比 custom 处理），完备度门接受它而非判空。`reason_code` 进冻结报告计数，比例过高反馈品类包设计。
3. **深度档兜底**：超休闲选 `DepthProfile { target: L4 }` 即可冻结，L5/L6 数值层显式标"留待开发期"，不强求。

品类包在设计空间清单（文档 03）里为决策点声明 `requirement: unlocked | baseline`，默认 `unlocked`（纯激活驱动）。只有极少数真正"基线"的点标 `baseline`。

---

## 4. DAG 引擎选型（开放问题 O5 —— 已定：新写轻量校验器）

**结论（2026-07-26 代码审查后定）**：**新写轻量校验器**，不复用 `decision_graph::CapabilityDecisionGraph` 引擎。

审查证据（`adm-new-design/src/decision_graph/`）：
- 该引擎是**能力轴驱动激活**（`compile(spec: &GameSpec.capabilities, domains, coverage)`，`mod.rs:242`），激活由 16 条固定能力轴谓词决定（`policy.rs` ~500 行硬编码规则表）。**"选项/Selection"概念在引擎里根本不存在**（grep 零命中）。
- 边是**节点级**不是选项级；`build_edges` 只在两端已激活时连边，**边不激活任何节点**（`mod.rs:536/545`）。`unlocks` 字段被解析但 `build_edges` **完全不消费**（`mod.rs:441`，无实现）。`ConflictsWith` 只生成无向标注、**不拦截不报错**（`mod.rs:562-575`）。
- **无深度档概念**；被 16 域硬门禁焊死（`validate_policy_coverage` 要求域集精确等于 `SUPPORTED_DOMAINS`，`mod.rs:479-520`）——无法喂任意选项图。
- 线上调用传 `DecisionCoverage::default()`（空覆盖，`game_spec_v2_steps.rs:433`），完成度恒 0，覆盖度路径只有测试在走。

**因此**：v3 在 `decision_model/` 内新写校验器，只**抄用**其两段通用算法（约 120 行）：Kahn 拓扑排序+环检测（`mod.rs:580-639`）、覆盖计数（`mod.rs:307-330`）。**保留** `CapabilityDecisionGraph` 引擎当"能力相关性过滤器"（它擅长的），与 v3 选项图并行、互不侵入。

**数据层复用**：现有 `OptionGroup`/`OptionItem`/`OptionRelation`/`OptionRef`（`data_loader/mod.rs:706-815`）是真正的选项级关系，v3 复用其结构——但注意 `OPTION_RELATION_TYPES` 现只有 `["soft_conflict","hard_exclusive"]`（`mod.rs:24`），**只有冲突、缺 requires/unlocks**，v3 需扩展关系类型。

DAG 校验最小规则集（R1 必须实现，全新写）：
1. 无环（拓扑可排序，抄 Kahn 内核）。
2. 每个 `requires` 的目标 `OptionSelector` 存在且可达。
3. `conflicts` 双向对称性检查（A 冲突 B 则 B 也应冲突 A）——**且冲突要真拦截**（补现引擎"只标注不拦截"的缺陷）。
4. `unlocks` 真实驱动激活（补现引擎未实现的语义）——这是文档 02 §3.5 适用性判定的基础。
5. 每个决策点至少一个选项；`custom` 选项的 `parameter_schema` 必须提供结构化字段（00 §3.2）。

---

## 5. 完成度计算（供 UI 实时显示 + 冻结门第 1 道）

```
分母 = 对所有 level ≤ depth_profile.target 且 genre_scope 适用当前品类包的 DecisionPoint，
       其 applicability == Active（激活）或 baseline 且未标 NotApplicable 的点。
       （Inactive 与 NotApplicable 的点不进分母 —— 见 §3.5）
完成 = 分母内每点：有 Selection 且 (provenance≠AiInterview 或 confirmed_by_user=true)
       且 参数按 parameter_schema 填齐（L6：表行数达 cardinality 下限、矩阵无空格）
```

- 结构层（L0–L4）：以决策点为单位计。
- 参数/表层（L5–L6）：以**整表**为确认单元（00 §3.3、文档 05）；表内缺格进「待填清单」而非默认值（红线 R2）。
- **简单玩法（如纯 IAA 超休闲）**：因大量深层点 `Inactive`，分母天然小，`target=L4` 即可全绿——不会因"内容少"卡门（§3.5）。

输出 `CompletenessReport { done, total, blocking: Vec<MissingItem> }`，`MissingItem` 精确到「哪个决策点/哪张表/哪一格」——直接喂冻结门与 UI。

---

## 6. R1 交付清单与验证

| 交付 | 文件（拟） | 验证 |
|------|-----------|------|
| 决策模型类型 | `adm-new-design/src/decision_model/types.rs` | 编译 + serde round-trip 测试 |
| 加载器（读设计空间清单 JSON） | `.../decision_model/loader.rs` | 加载空图 + 加载文档 03 样例清单 |
| DAG 校验器（全新写，抄 Kahn+计数内核） | `.../decision_model/graph.rs` | 环/悬空 requires/冲突不对称/unlocks 激活 的负例测试 |
| 完成度计算（含 §3.5 适用性） | `.../decision_model/completeness.rs` | 部分填充 → 正确 blocking 清单；Inactive/NA 不进分母 |
| ~~CapabilityDecisionGraph 复用评估~~（已定：不复用引擎，见 §4） | — | — |
| DTO（Selection/Provenance/DepthProfile） | `adm-new-contracts/src/decision.rs`（新模块） | 与 `project.rs` 不冲突 |

**R1 完成定义**：能加载文档 03 交付的一份塔防设计空间清单（哪怕选项稀疏），跑通 DAG 校验与完成度计算，全部单元测试绿。**此时无真实决策内容，内容在 R2/R4 到位。**
