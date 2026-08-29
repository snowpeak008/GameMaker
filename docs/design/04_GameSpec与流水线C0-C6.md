# V4 设计 · 04 · GameSpec 与流水线 C0-C6

> 落点 crate：`adm4-spec`（规格）+ `adm4-pipeline`（框架与 C0-C6）
> 依据：redesign_v3 文档 00 §5 / 06；第二版验收追溯链经验

---

## 1. GameSpec（全新定义，V4 schema 4.0.0）

由 C0 从 `FrozenDesign` 确定性编译产生，是下游一切派生的唯一输入。

```rust
pub struct GameSpec {
    pub identity: SpecIdentity,           // schema_version, project_id, frozen_hash（绑定冻结集）
    pub intent: ProjectIntent,            // L0-L2：标题/体验承诺/品类结构/平台/商业/规模
    pub systems: Vec<SystemSpec>,         // L3：系统组成 { id, name, purpose, interfaces }
    pub mechanics: Vec<MechanicSpec>,     // L4：机制规则 { id, system_id, rule_text(公式符号级),
                                          //   preconditions: Vec<ConditionSpec>,
                                          //   effects: Vec<EffectSpec>,   // 封闭枚举
                                          //   state_machine: Option<StateMachineSpec> }
    pub entities: Vec<EntitySpec>,        // L5：实体类型 { id, name, visual_form: Option<VisualForm>,
                                          //   properties: Vec<PropertySpec> }
    pub tables: Vec<TableSpec>,           // L5/L6：属性表/矩阵（列结构 + 行数据）
    pub content: Vec<ContentSpec>,        // L6：关卡/波次等内容数据
    pub acceptance: Vec<AcceptanceScenario>, // GWT 验收场景（C4 派生填充）
    pub source_map: Vec<SpecSourceEntry>, // 每个 spec 元素 ← 决策路径（可追溯）
}

pub enum EffectSpec {                     // 封闭枚举——C4 投影的前提
    ModifyProperty { entity: String, property: String, formula: String },
    SpawnEntity { entity: String },
    DespawnEntity { entity: String },
    ChangeState { machine: String, to_state: String },
    GrantResource { resource: String, formula: String },
    ConsumeResource { resource: String, formula: String },
    EmitSignal { signal: String },
}

pub struct SpecRef(pub String);           // 如 "mechanics/counter_damage"，锚定用（R4）
```

- **规范化 + 哈希**：`canonicalize(spec) -> String`（键排序的确定性 JSON）+ sha256；`identity.frozen_hash` 绑定冻结集哈希。
- **校验**：`validate_game_spec`——引用完整性（mechanic→system、table 轴→entity、source_map 全覆盖）、公式字段非空、未知 EffectSpec 不可能存在（封闭枚举）。

---

## 2. 流水线框架（adm4-pipeline）

```rust
pub struct StageSpec { pub id: StageId /* "C0".."C6" */, pub name: String,
                       pub kind: StageKind /* Deterministic | AiRequired | HumanGate */,
                       pub depends_on: Vec<StageId> }

pub enum StageStatus { Pending, Running, Succeeded, Failed, Blocked { reasons: Vec<String> },
                       WaitingHuman { gate: String } }

pub trait StageExecutor {
    fn stage(&self) -> StageId;
    fn execute(&self, ctx: &StageContext) -> Adm4Result<StageOutcome>;
}
```

- **注册表**：`design_compile_registry()` 返回 C0-C6 的 `StageSpec` 拓扑；面板与步骤解耦，UI 数据驱动。
- **双格式产物**：每步产出机器契约（JSON，真相源，进哈希）+ 由契约渲染的人读 Markdown（不可手改，重跑覆盖）。产物写入统一经 `ArtifactWriter`，落盘前过 `SkinScanner`（R5）。
- **运行状态**：`PipelineRunState`（每阶段状态 + 产物哈希 + 人工门记录），支持断点续跑与区间执行。

---

## 3. C0：规格编译（确定性，无 AI）

`FrozenDesign.Selection` → GameSpec 的分层映射：

| 决策层 | Selection 内容 | 编译到 |
|--------|--------------|--------|
| L0-L1 | 档案/体验方向 | `ProjectIntent` |
| L2 | 品类/模式 | `ProjectIntent.genre_structure` |
| L3 | 系统 + 接口 | `SystemSpec` |
| L4 | 机制规则（公式符号级 Scalar 参数） | `MechanicSpec`（preconditions/effects/state_machine） |
| L5 | 表/矩阵列结构 | `EntitySpec.properties` + `TableSpec` 列 |
| L6 | 行数据 | `TableSpec.rows` + `ContentSpec` |

门禁：schema 校验 + 冻结哈希绑定 + `source_map` 全覆盖（每个 spec 元素必须能追溯到决策 id）。编译遇无法映射的选择 → `Derive::Blocked`（R2），绝不跳过。

---

## 4. C1-C6 契约与门禁

| 步 | 类型 | 职责 | 关键契约产物 | 门禁 |
|----|------|------|-------------|------|
| C1 | AI 必需 | 静态验证与红队 | `validation_report`、`redteam_findings` | 机器规则零违例；`ReviewProof`（R3）；发现项处置记录 |
| C2 | AI 必需（叙述） | 玩法设计文档 | `gameplay_doc_contract`（章节→SpecRef 锚定表）+ 渲染 MD | 锚定覆盖率 100%；无锚定叙述即 fail（R4） |
| C3 | AI 必需（画面描述） | 内容与资产需求 | `content_inventory`、`asset_spec_set`（图/音/UI，含生图风格字段与画面描述）、`audio_spec`、`ui_spec` | **视觉形态白名单**（只有声明 `visual_form` 的实体产美术资产；未知→Blocked，R2）；**基数门**（对照品类包期望区间，超界→人工确认，R6） |
| C4 | AI 必需（接口命名） | 程序需求与架构 | `capability_contracts`（接口/数据结构/GWT 验收场景）、`engine_architecture`（场景/预制体/模块划分） | 每能力 ≥1 可判定验收场景；L4 机制覆盖率双向核对（见 §5） |
| C5 | 人工门 + AI 生图 | 美术方向与风格锚点 | `style_brief`、`anchor_images`、`style_confirmation` | 人工确认风格锚点 |
| C6 | 人工门 | 开发计划 + Phase1 签收 | `task_graph`（程序/资产/装配任务，依赖与并行分组）、`phase1_signoff` | 任务图与 C3/C4 真实全量对账（非硬编码分数）；人工签收文档集 |

**Phase 1 完成定义**：C0-C6 契约齐全 + 两个人工门通过。文档集独立可交付；L4/L5 深度档到此为止。

### 4.1 C4 机制投影派生器（三代从未建成，V4 核心新建）

确定性投影（枚举投影，无 AI；AI 只做接口命名/叙述且必须锚定）：

```
对每条 MechanicSpec：
  preconditions(ConditionSpec) ──投影──▶ 验收场景 Given
  effects(EffectSpec 枚举)      ──投影──▶ 验收场景 Then
  mechanic + 涉及实体/资源      ──派生──▶ CapabilityContract（接口 + 数据结构）
  CapabilityContract            ──派生──▶ ≥1 个程序任务（带 declared_write_paths）
```

- 无法投影的机制（理论上不可能——EffectSpec 是封闭枚举；但公式解析失败等）→ `Derive::Blocked` + 清单，**绝不用通用 stub 兜底**；
- **反过拟合验收**：机制变异/标签置换必须改变任务图（测试强制）。

### 4.2 一致性双向核对（真核对，非橡皮图章）

1. **正向**：每个能力契约、每个任务带 `source_refs: Vec<SpecRef>`，校验路径真实存在于 GameSpec；
2. **反向**：遍历每条 MechanicSpec，未被任何能力契约 `source_refs` 命中 → `CORE_MECHANIC_NOT_PLANNED` blocker；
3. 覆盖率 = 真实命中数（`MeasuredMetric` 带证据指针，R1）；禁止恒 1.0；
4. 一对多派生（实体→资产、机制→需求）返回 `CardinalityDeclaration`（映射规则+丢弃清单+期望区间对照，R6）。

---

## 5. Phase 2 边界（P0-P5，仅 L6 深度档，另行立项）

| 步 | 职责 |
|----|------|
| P0 | 引擎工程骨架种子（按 `engine_architecture`） |
| P1 | 程序任务执行（并行生成、串行合入；变更内核+受信测试） |
| P2 | 资产批量生产（生产前清单人工门；内容哈希缓存） |
| P3 | 装配与集成（按 spec 执行） |
| P4 | 验收场景执行（C4 的 GWT 真机运行判定） |
| P5 | 打包交付（EXE + manifest + 确定性报告） |

V4 本轮只在 registry 保留 `phase2_registry()` 占位定义与文档边界，不实现。
