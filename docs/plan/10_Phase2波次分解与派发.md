# V4 · Phase 2 · 10 · 插件架构与波次分解派发

> 立项分册 10/10。总纲 `06`，治理 `07`，美术线 `08`，程序线+验收 `09`。
> 本册是**架构 + 执行计划**：先定插件架构（你第 3 点：强设计关联/弱代码耦合），再把 Phase 2
> 拆成 G1-G5 波次任务卡（照 `plan/04` 格式）。协作模式沿用 W6（主开发计划/派发/集成/验收，
> 子 agent 并行，统一 Claude Opus 5 Thinking，每波跑统一门禁）。

---

## 1. 插件架构（你第 3 点，采纳 py StagePlugin + artifact_layer）

**原则：强设计关联（制品依赖图显式声明），弱代码耦合（插件自治、接口统一）——谁错谁的问题立即定位。**

| 层 | 职责 | 采纳自 |
|----|------|--------|
| **插件层** | 每个能力一个自治插件（含逻辑 + 契约声明），实现统一 `StageExecutor`/`Producer`/`Backend` trait | py `pipeline/step_*/` StagePlugin |
| **制品注册层** | 每个插件显式声明产出契约与依赖（依赖图，拓扑序）；**这是"强设计关联"** | py `pipeline/artifact_layer/registry.json` |
| **运行骨架层** | `Phase2Runner`（复用 V4 pipeline 框架：断点续跑/区间/人工门），只认 trait 不认具体实现 | V4 `adm4-pipeline` |
| **接缝层** | `EngineBackend` / `AssetProducer` / 校验插件——弱代码耦合的换点 | godogen + D17/D19 |

**弱代码耦合的落法**：
- 插件之间不直接 import，只通过**制品契约**交换数据（JSON，权威顺序表 §07）；
- 加引擎 = 加一个 `EngineBackend` 插件；加资产通道 = 加一个 `AssetProducer` 插件；加校验 = 加一个校验插件——骨架零改；
- **接缝纪律**：`adm4-build` 的 runner/治理/契约模块里禁止出现 `unity` 字样（Unity 锁在 `engine/unity_mcp/`）。

**crate 布局**：
```
crates/adm4-build/
├── runner.rs            Phase2Runner（复用 pipeline 框架）
├── registry.rs          制品依赖声明 + 拓扑序（强设计关联）
├── governance/          program_line / art_line / alignment / asset_registry / asset_genome / authority_order（册 07）
├── art/                 style_anchor（设计阶段门）/ asset_producer(trait) / budget / cache / genome_backfill（册 08）
├── program/             slice / manifest / engine_guide（册 09）
├── engine/              EngineBackend(trait) + unity_mcp/（MCP 驱动 Unity）
├── proof/               bundle / precheck / verdict / repair（册 09）
└── determinism/         哈希缓存 + 生成调用记录
```
`phase2_registry()` 留在 `adm4-pipeline`（版图同处渲染）；执行器实现在 `adm4-build`；`AppServices::build_*` 接线。

---

## 2. 波次总览（G1-G5）

| 波 | 名称 | 交付 | 对应册 | 依赖 |
|----|------|------|--------|------|
| **G1** | 治理骨架 + 插件框架 | 两条线契约、asset_registry、alignment、AssetGenome、权威顺序校验器、`Phase2Runner` + 插件注册（执行器诚实空实现） | 07 | T12 基线 |
| **G2** | 美术风格锚点门（设计阶段，选项 A） | 设计工作台风格门：看图/改词/反复重生成/确认锁定 `style_anchor_set` + `style_application_contract` | 08 §2-3 | G1 |
| **G3** | 资产生产线 | `AssetProducer`(AI 通道) + 预算门 + 哈希缓存 + 资产表 + AssetGenome 回填 + 一致性比对 | 08 §4 | G2 |
| **G4** | 可玩生产（MCP-Unity） | 可玩切片 + runtime manifest + Unity 引擎指南 + `UnityMcpBackend` 现场开发 + durable docs | 09 §2-4 | G3 + **Q1/Q2 定** |
| **G5** | proof 验收 + 端到端 | proof bundle 捕获 + 机器预检 + 用户 verdict + repair 回写 + L6 全链 e2e + 红线审查 | 09 §5-7 | G4 |

依赖基本串行；波内子任务并行（可写文件集互不相交）。**硬前置**：需真实走完设计→冻结→C0-C6 的 L6 项目做夹具（`lane_defense` 全链 e2e 可作基底）。

---

## 3. 波次任务卡

### G1 · 治理骨架 + 插件框架
- **必读**：`docs/plan/06/07`、`docs/plan/03 工程规范`、`crates/adm4-pipeline/src/framework.rs`（ArtifactStore/PipelineRunState）、`knowledge/governance/*`（原始治理协议）。
- **可写范围**：新建 `crates/adm4-build/**`（除 `engine/unity_mcp`、`art/style_anchor`）、`adm4-pipeline`（接线）、`adm4-app`（build_*）。
- **接口契约**：GameSpec（唯一真源）、SpecRef、C3/C4 契约。
- **任务**：`governance/`（program_line/art_line/alignment/asset_registry/asset_genome/authority_order，全 serde + `#[serde(default)]`）；`registry.rs`（制品依赖声明 + 拓扑）；`Phase2Runner`（区间/续跑/人工门框架，执行器先返回 `Blocked("待 G? 实现")`）；权威顺序校验器（JSON 压 Markdown）做成校验插件；对齐三要素核对 + orphan/conflict 判定用**确定性 Rust**（不靠 AI）。
- **验收**：契约 serde 往返；对齐层能出 unresolved_conflicts/orphan；权威顺序校验拦 Markdown-only 事实；插件框架可跑诚实空版图。
- **自验收**：`cargo test -p adm4-build`；`cargo clippy -p adm4-build --all-targets`（零警告）；门禁全绿。

### G2 · 美术风格锚点门（设计阶段）
- **必读**：`06/08`、py `pipeline/step_07_art_style_generation/`、`core/ui/pipeline_panel.py`（风格网格/全屏/重生成）、`core/ui/style_prompt_editor.py`、v3 `A08a_step07_art_direction.md`。
- **可写范围**：`crates/adm4-build/art/style_anchor.rs`、`crates/adm4-authoring`（风格态）、`crates/adm4-app`（风格服务）、`apps/adm4-desktop`（风格面板）。
- **任务**：风格生成（3-5 方向带预览图）+ 风格网格 UI（双击全屏）+ 对话式改提示词（prompt_override 重生成）+ attended 确认（禁 auto_accept，R3 署名）→ 锁定 `style_anchor_set`/`style_application_contract`（+ 锚点不可变历史 `style/anchors/v{N}/`）；风格-原型适配报告。
- **验收**：用户可看图/改词/反复重生成/确认；未确认阻断下游；确认后可重选；产物结构对下游可读。
- **自验收**：`cargo test`；`cargo build -p adm4-desktop`；真机走查看图确认流程。

### G3 · 资产生产线
- **必读**：`06/07/08`、py `core/art_pipeline/`、`AssetGenome.md`、`ART_ASSET_NAMING_CONVENTION.md`。
- **可写范围**：`crates/adm4-build/art/{asset_producer.rs,budget.rs,cache.rs,genome_backfill.rs}`。
- **任务**：`AssetProducer`(AI 通道 + `ExternalToolProducer` NotConfigured 占位)；资产预算门（首次付费确认，R3）；内容哈希缓存；资产表（Name/Purpose/Runtime path/In-game size/Cost/Fallback/Used by）；AssetGenome 回填（path=运行时加载 path）；一致性比对 vs 风格锚点（drift → repair）；视觉白名单/换皮/基数（R2/R5/R6）。
- **验收**：无 visual_form 不产；缓存二次命中；资产名过命名规范+换皮扫描；AssetGenome path 与运行时一致；漂移可检出。
- **自验收**：`cargo test`（白名单/缓存/命名/漂移单测）；`clippy` 零警告。

### G4 · 可玩生产（MCP-Unity）
- **前置**：**Q1（Unity 版本+License）、Q2（Unity MCP 工具选型）已定**；先打通"最小工程经 MCP 建场景+进 PlayMode+截一帧"验证环境。
- **必读**：`06/09`、godogen `build/01-03`、Q2 选定的 MCP server 文档。
- **可写范围**：`crates/adm4-build/{program/,engine/unity_mcp/}`。
- **任务**：可玩切片抽取 + 风险切片；runtime manifest + Unity 引擎指南（只写坑+构建/运行/捕获命令）；`UnityMcpBackend`（经 MCP 现场建场景/挂脚本/配预制体/跑 PlayMode，agent cwd=隔离工程）；durable docs 生成；每轮记录可停可续。
- **验收**：能经 MCP 现场把可玩切片跑起来；隔离工程与仓库隔离；失败可恢复；环境缺失诚实 Blocked。
- **自验收**：`cargo test`（用 MockEngineBackend 确定性回放）；真机 MCP-Unity 冒烟（视 Q1/Q2 环境）。

### G5 · proof 验收 + 端到端
- **必读**：`06/09`、godogen proof 层、`docs/design/05 红线`。
- **可写范围**：`crates/adm4-build/proof/`、`adm4-app`（build_confirm 收尾）、`adm4-desktop`（proof 预览+verdict）。
- **任务**：proof bundle 捕获（build/run log + screenshots + video + proof_review）；机器预检（非空白/核心动作可见/资产加载，确定性）；用户 verdict 门（pass/warn/repair/blocked_env）；repair_queue 映射回设计/程序/美术/环境层；**L6 全链 e2e**（设计→冻结→C0-C6→风格锚点→资产→MCP 可玩生产→proof→verdict）；**Phase 2 红线合规审查**（R1-R7 生产段默认路径强制生效）。
- **验收**：proof 以画面事实判定、恒真被拦；verdict 与证据一致；缺陷能定位到层；全链绿；红线审查无实质违规。
- **自验收**：`cargo test --workspace`；`cargo build -p adm4-desktop`；`space validate` 双包；Phase 2 冒烟脚本。

---

## 4. 工程规范补充（承接 plan/03）

1. **接缝纪律**：`adm4-build` 的 runner/governance/契约模块禁止 `unity` 字样（Unity 锁在 `engine/unity_mcp/`）——任务卡强制，验收抽查（D17）。
2. **不新造第二真源**：一切派生自 GameSpec；对齐/回填/命名是校验与追溯层（D22）。
3. **确定性优先**：对齐三要素核对、机器预检、命名校验用确定性 Rust，不靠 AI；AI 只产候选、不自判通过（v3 原则）。
4. **id 只增不改 + serde default**：阶段/契约字段只增不改不删；新字段带 `#[serde(default)]`（旧档兼容铁律，D4）。
5. **优化纪律（D25）**：每处对既有设计的改动在册中标「【优化】」；保留既有设计，不过度扩散/简化。
6. **git**：沿用用户规则（推 GameMaker，补丁位 +1；大版本号用户明确才动）。

---

## 5. 未决项收束（派 G1 前）

- Q1 Unity 版本 + License（G4 前）；**Q2 Unity MCP 工具选型（G4 前，决定 proof 捕获可行性）**；Q3 proof 捕获方式；Q4 AI 生产模型/预算。
- 选项 A 的设计阶段风格门要动 `adm4-authoring`/`adm4-desktop`（G2）——确认可接受再派。
- G1 与 G2 文件集基本不相交，理论可并行；但建议 G1 先（治理是地基），G2 紧随。
