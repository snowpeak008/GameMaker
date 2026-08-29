# v3 实施子计划 · 06 · C0–C6 文档编译流水线

> 上位：[00 §5](00_master_design.md) · [01 总纲](01_overview_and_milestones.md)
> 里程碑：R6（C0–C2）、R7（C3–C6）、R9（Phase2 衔接，另行立项）
> 落点：`adm-new-pipeline`（新 registry）+ 契约进 `adm-new-contracts` + 复用 `adm-new-game-spec`

---

## 1. 与现有流水线的关系（开放问题 O4）

探测确认现状：`adm-new-pipeline` 用 string stage_ids `"00"`–`"14"`（`default_development_registry()`），legacy（`step08_14.rs`）与 `_v2` 实现并存，`StageKind ∈ Development/HumanGate(07)/Validation(14)`。

**v3 策略**：C0–C6 建**新 registry**（`design_compile_registry()`，独立命名空间 `"C0"`–`"C6"`），**与现 "00"–"14" 并存**。Phase1（C0–C6）全绿并试跑通过后，再下线 legacy（01 号 §3 对账）。不增量修补 legacy。

复用现有 `contracts/pipeline.rs` 的基础设施：`PipelineRegistry`/`StageSpec`/`StageKind`/`StageStatus`/`PipelineRunState`/`PipelineCheckpoint`/`StageExecutor` trait。C0–C6 只是新 `StageExecutor` 实现 + 新 registry。

---

## 2. 总原则（00 §5.1）

- **输入唯一**：Phase1 唯一输入 = 冻结决策集编译出的 GameSpec（文档 05 `FrozenDesign`）。无 source_artifacts 概念包解析；概念文档只服务设计阶段的 AI 访谈。
- **产物双格式**：每步产机器契约（真相源，进哈希）+ 由契约渲染的人读 MD（不可手改，改了下次渲染覆盖）。
- **AI 分工**：确定性派生（编译/校验/渲染骨架）不用 AI；叙述加工与评审必须用 AI，AI 不可用即 `blocked`。AI 产出必须锚定 GameSpec 路径（红线 R4）。
- **步骤自治 + 单一职责**：每步一职责、一组契约、一个门禁。

---

## 3. C0：规格编译（决策集 → GameSpec，确定性，无 AI）

这是 v3 与 `adm-new-game-spec` 的接缝。`FrozenDesign` 的 Selection → GameSpec 各 Spec：

| 决策层 | Selection 内容 | 编译到 GameSpec |
|--------|--------------|----------------|
| L2 品类结构 | 品类/模式 | `ProjectIntent`/`ScopeEnvelope` |
| L3 系统组成 | 系统 + 接口 | `EntitySpec` 骨架 + `RelationshipSpec` |
| L4 机制规则 | 规则/状态机（公式符号级） | `ActionSpec`/`ConditionSpec`/`EffectSpec`/`StateMachineSpec` |
| L5 实体与参数结构 | 属性表/矩阵列结构 | `EntitySpec.components` + `PropertySpec`（ValueKind 已对齐，文档 02 §3.2） |
| L6 数值表 | 具体行数据 | `PropertySpec` 默认值 + `ResourceSpec` + 关卡 `ContentSpec` |

C0 产物：`GameSpec`（`canonicalize_game_spec` + sha2 哈希，绑定 `FrozenDesign.content_hash`）。门禁：schema 校验（`validate_game_spec`）+ 决策集哈希绑定。**无 AI**。

> **schema 演进**（00 §5.5）：GameSpec v2 现为 `2.0.0-alpha.1`。若 L5/L6 表结构无法无损落进现有 Spec，按需演进 schema（加字段走 `ExtensionBlock` 或提 minor 版本），不另造并行 spec。

---

## 4. C1–C6 逐步契约（00 §5.2）

| 步 | 职责 | 关键产出（契约） | 门禁 | AI |
|----|------|-----------------|------|-----|
| **C1** | 静态验证与红队 | `validation_report`、`redteam_findings` | 机器规则零违例；红队最低工作量证明（R3）；发现项处置记录 | 必需 |
| **C2** | 玩法设计文档 | `gameplay_doc_contract`（章节→spec 路径锚定表）+ 渲染 MD | 锚定覆盖率 100%（每设计声明可追溯到决策）；无锚定叙述即 fail（R4） | 必需（叙述） |
| **C3** | 内容与资产需求 | `content_inventory`、`asset_spec_set`（图/音/UI，含生图风格字段与画面描述）、`audio_spec`、`ui_spec` | **视觉形态白名单**（只有声明视觉形态的实体产美术资产；未知类型→blocked，R2）；**基数门禁**（对照品类包基数期望表，超界→人工确认，R6） | 必需（画面描述） |
| **C4** | 程序需求与架构 | `capability_contracts`（接口/数据结构/验收场景 GWT，从 L4 派生）、`unity_architecture`（场景/Prefab/asmdef 划分） | 每能力≥1 可判定验收场景；机制规则覆盖率核对（L4 每规则≥1 能力映射） | 必需（接口设计） |
| **C5** | 美术方向与风格锚点 | `style_brief`、`anchor_images` + `style_confirmation` | **人工门**：用户确认风格锚点 | 必需（生图） |
| **C6** | 开发计划 + Phase1 签收 | `task_graph`（程序/资产/装配任务，依赖与并行分组）、`phase1_signoff` | 任务图与 C3/C4 **真实**全量对账（§4.4，非硬编码 100 分）；**人工门**：用户签收文档集 | 计划叙述 |

**Phase1 完成定义**：C0–C6 契约齐全 + 两个人工门（C5、C6）通过。此时文档集独立可交付（选 L4/L5 深度档的项目到此为止，L6 才进 Phase2）。

> ⚠️ **C4 是本流水线最高风险步，且三代从未真正建成**（见 §4.3 代码调查结论）。C4 与 C6 的成败决定整个 v3 是否成立——务必先读 §4.3/§4.4。

### 4.1 契约落点

新契约类型进 `adm-new-contracts`（新模块 `compile.rs` 或按步分）。复用现有 `ArtifactContract`/`ArtifactRecord`/`AcceptanceEvidence`/`TraceLink`（game-spec）作锚定基础设施。C5 生图复用 `adm-new-ai` image/VLM + 现有 step07 风格确认经验（`style_confirmation.json`）。

### 4.2 复用现有 C5 相关资产

现有 `step07`（Art Style Generation，HumanGate）+ `adm-new-application/style_image.rs` + `art_pipeline/`（stage04/09/12/13/14）有成熟的生图与风格确认链路。C5 **复用其人工门与生图机制**，改的是输入（从 v3 GameSpec 派生而非 legacy 上游）。

### 4.3 design→plan 派生：净新建，不复用 v2 硬编码模板（本流水线最高风险）

**代码调查结论（2026-07-26）**：现有两套 design→plan 实现都不能直接用作 C3/C4 的内容派生：

| 现有实现 | 真实性质 | 对 C3/C4 的复用裁决 |
|---------|---------|-------------------|
| v2 编译器（`step08_10_v2.rs`） | 调度**框架**真实高质量（Kahn 拓扑分层 `:833`、写冲突并行拆批 `:1009`、workspace 契约、determinism 哈希、`validate_task_graph` `:742`）；但**任务图是写死的 8 条塔防任务**（`:405-483`），`compile_task_graph` **完全不读** `spec.actions/state_machines/effects`，无论输入什么玩法产出恒定；资产靠硬编码 tag（`"guardian"/"enemy"/"objective"` `:326`）过滤 | **框架复用，内容派生全部重写**。C6 复用其拓扑/并行/契约/哈希；C3/C4 的任务/系统/资产派生**必须真读 GameSpec 内容**，绝不沿用硬编码模板 |
| legacy（`step08_14.rs` + `semantic_pipeline.rs`） | `program_tasks_from_contract`（`:2760`）是真 1:1（requirement→task，随 project_dna 变），能力契约从 project_dna 各元素 1:1 派生（`semantic_pipeline.rs:376`，带真实 blocker）；但**扁平**——无依赖、无粒度分层、依赖图写死空、单一 PG-001 组 | **1:1 派生思路可借鉴**（内容真的随设计变），但需补依赖/粒度/并行分层（用 v2 框架补） |

**C4 核心新建：L4 机制 → 能力契约 → GWT 派生器（三代都不存在）**

现状：`ActionSpec{preconditions, effects}`、`StateMachineSpec{guards, effects}`、`AcceptanceScenario`（=GWT）在数据模型里**并列独立、人工各自编写**，代码只做交叉引用不悬空校验（`validation.rs:575`），**没有任何派生逻辑**。`CapabilityProfile` 8 轴是独立画像，与 ActionSpec 无派生关系。

C4 必须新建这个派生器（数据模型已具前提：`EffectSpec` 是枚举 `spec.rs:282`、GWT 即 `AcceptanceScenario`）：

```
对每条 L4 机制规则（决策集 → GameSpec.actions / state_machines）：
  precondition(ConditionSpec)  ──投影──▶  验收场景 given
  effect(EffectSpec 枚举)      ──投影──▶  验收场景 then
  action + 涉及实体/资源       ──派生──▶  能力契约（接口 + 数据结构）
  能力契约                     ──派生──▶  ≥1 个程序任务（带 declared_write_paths）
```

- 派生是**确定性**的（枚举投影，无 AI）；AI 只在"接口命名/叙述"层加工且必须锚定（红线 R4）。
- 每个能力契约至少一个可判定验收场景（C4 门禁）；每条 L4 规则至少映射到一个能力（覆盖率核对，§4.4）。
- 遇 GameSpec 里无法投影的机制（未知 EffectSpec 变体等）→ `blocked` + 清单（红线 R2），**绝不用通用 stub 兜底**（现状 `machine_checks` 是固定桩 `step08_10_v2.rs:635`，v3 禁止）。

### 4.4 一致性核对：把"设计↔计划"从橡皮图章改成真核对

**代码调查确认的病灶**（legacy 活跃路径）：coverage 恒 = 1.0（`unbound_semantic_items` 硬编码 `[]`，`semantic_pipeline.rs:512/517`）、score 恒 100/60（`step08_14.rs:3548`）、blockers 恒 `[]`、review 恒 `passed`。真正的反向覆盖核对（`build_program_semantic_coverage_matrix` `:1025`）**只在测试里被调用**，且用 `index % len` 取模轮询假覆盖（`:998`）。

v3 一致性核对必须（接红线 R1/R4/R6）：

1. **双向真核对，接进活跃路径**（非 test-only）：
   - **正向**（每个任务/需求 → 追溯回设计决策）：每个 C4 能力契约、C6 任务必须带 `source_refs` 指向 GameSpec 路径，且**校验该路径真实存在**（现状 legacy `source_refs` 无存在性校验，`step08_14.rs:2821`）。
   - **反向**（每条 L4 设计规则 → 至少映射到一个能力/任务）：遍历 GameSpec 每条机制规则，未被任何能力契约覆盖 → `CORE_MECHANIC_NOT_PLANNED` blocker。**用语义匹配（source_ref 指向）而非取模轮询**。
2. **覆盖率是真测量**（红线 R1）：`covered = 实际被 source_ref 命中的规则数`，带证据指针列表；**禁止** `unbound = []` 式恒零。
3. **基数申报 + 丢弃清单**（红线 R6，现状完全不存在）：实体→资产、机制→需求 的一对多派生必须声明映射规则 + 丢弃清单；`unbound_semantic_items` 必须是**真实计算的残差**，非硬编码空。
4. **anti-overfit 扩到任务图**：现状 anti-overfit 只验 `runtime_systems`，**不验任务图**（因任务图硬编码，`step08_10_v2.rs:1191/1219`）。v3 任务图真随设计变后，label 置换/capability 变异必须能改变任务图，纳入 anti-overfit。

> **与冻结门第 1 道的闭环**：C4 能否**派生而非发明**，前提是 L4 达"公式符号级"、L5 是真表结构（文档 02 §3.2）。若决策集这些层太薄，C4 无从投影 → 被迫发明 → 退回三代失败。因此冻结门第 1 道"拆分就绪度/完备度门"（文档 05 门1）不是官僚流程，**它是 C4 可派生的准入前提**——完备度门检查的正是"下游能不发明就拆分"。

---

## 5. Phase 2 边界（P0–P5，仅 L6，R9 另行立项）

本集只定边界与复用关系（00 §5.3 明确 Phase2 细化后置）：

| 步 | 职责 | 复用锚点 |
|----|------|---------|
| P0 | 工作区种子 | 现有 workspace_seed 思路 + `unity_architecture` |
| P1 | 程序任务执行 | **`adm-new-change-kernel`**（SpecStore/ChangeKernel/受信测试）+ `adm-new-pipeline` work-unit（`WorkUnitExecutor`/`execute_work_unit_batch`）；并行生成串行合入 |
| P2 | 资产批量生产 | `adm-new-ai` image + 内容哈希缓存；**清单人工门** |
| P3 | 装配集成 | 按 spec 执行 |
| P4 | 验收场景执行 | C4 的 GWT + 现有 acceptance harness 方向 |
| P5 | 打包交付 | `adm-new-packaging`（`PackageManifest`/`package_current_project`） |

Phase2 在 Phase1 落地并试跑通过后展开（R9）。

---

## 6. R6 / R7 交付清单

**R6（C0–C2）**：

| 交付 | 文件 | 验证 |
|------|------|------|
| 新 registry | `adm-new-pipeline/src/design_compile_registry.rs` | 拓扑排序正确 |
| C0 编译器 | `.../stages/c0_compile.rs` | FrozenDesign→GameSpec，哈希绑定，validate 通过 |
| C1 验证+红队 | `.../stages/c1_validation.rs` | 对注入缺陷的检出率（M3 验证） |
| C2 玩法文档 | `.../stages/c2_gameplay_doc.rs` | 锚定覆盖率 100%，无锚定→fail |
| 契约类型 | `adm-new-contracts/src/compile.rs` | serde + schema |

**R7（C3–C6）**：

| 交付 | 文件 | 验证 |
|------|------|------|
| C3 内容资产需求 | `.../stages/c3_content.rs` | 视觉白名单 + 基数门（未知→blocked）；资产随设计变（非硬编码 tag） |
| **C4 L4→能力→GWT 派生器**（净新建，§4.3） | `.../stages/c4_program.rs` + `.../derive/mechanic_projection.rs` | **反测：label 置换/机制变异→任务图必须变**（现状 v2 恒定）；每能力≥1 GWT；无法投影的机制→blocked（非 stub 兜底） |
| C4/C6 一致性核对（§4.4） | `.../derive/coverage.rs` | **接活跃路径**：正向 source_ref 存在性 + 反向机制覆盖（语义匹配非取模）；覆盖率真测量带证据；**反测：漏派生一条机制→coverage<1.0 且报 blocker**（现状恒 1.0） |
| C5 风格锚点 | `.../stages/c5_style.rs`（复用 step07） | 人工门通过 |
| C6 计划+签收 | `.../stages/c6_plan.rs`（复用 v2 拓扑/并行框架） | 真实对账（非硬编码 100 分）+ 人工签收 |

**R6/R7 完成定义**：一个塔防冻结集跑通 C0–C6，产出双格式文档集，两人工门通过，文档集整体人工评审 ≥ 目标分（M4）。
