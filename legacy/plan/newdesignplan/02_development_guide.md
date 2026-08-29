# 开发说明文档

面向：后续执行本计划的开发者 / AI 会话。
目的：解释这套计划"是什么、为什么这样排、从哪里开始、去哪里找东西"。规范性要求见 `01_development_standards.md`，本文只做说明。

---

## 1. 一句话说明

把 NEWrust（Rust/Tauri 版 AutoDesignMaker）重构为一个自动游戏开发系统：**用户意图 → GameSpec → 确定性编译 → 代码/资产生产 → 测试 → 可执行游戏**；先用一个通道防守垂直切片证明"能造出可玩的游戏"（R1），再横向扩展为通用生产系统（R2）。

## 2. 计划文档地图

```
plan/newdesignplan/
├── 00_master_plan.md            总体计划：定位、架构、任务图、停止门（权威）
├── 01_development_standards.md  强制开发标准与红线
├── 02_development_guide.md      本文档
└── dev/                         原子开发计划（执行入口）
    ├── README.md                任务顺序表与状态
    ├── R1C0_content_charter.md  内容宪章（最先启动）
    ├── A02_validation_hashing_envelope.md
    ├── R0_technical_probe.md
    ├── A03 … A10                各原子任务
```

历史文档：v1 计划在 `NEWrust/plan/2026-07-15_universal_game_spec_compiler.md` 及其 `dev/`，保留作参考，不再是执行依据。v1 的 A01 已完成且继续有效（`adm-new-game-spec` crate）。

## 3. 为什么任务图长这样（设计决策摘要）

这份计划经过三方评审（初版计划 → Claude 审查 → Codex 反驳修正 → 收敛），关键决策及理由：

1. **R1-C0 内容宪章放在最前**：前几轮方案都在设计"工厂"而没人定义"产品"。不先回答"通道防守游戏做成什么样才算数"，A08f 的停止门就是主观的。宪章的机器层交付物（AcceptanceScenario 草稿 + ProductEnvelope 档位）让"可玩"从形容词变成可判定条件。
2. **R0 技术探针并行且前置于 A04**：最致命的风险（Step11 工具链、Unity 批处理、打包）不能等到 A08 才第一次接触。R0 用手写最小规格 + 现有内核提前走通全链路；它暴露的失败分类正是 A04 设计统一内核最需要的输入——先探针、后统一。R0 是技术验证，不是内容成果，占位内容用完即弃，但 harness 保留演化为 A08c 的回归床。
3. **A04 只统一接口，A08c 才迁移实现**：Rust 侧已有三套相关实现在工作（见第 5 节），在没有新消费者时全部迁移等于大爆炸重构。A04 定义 ChangeKernel + WorkspaceChangeSet 合同，A08c 在有 R0 harness 回归保护时做实际迁移。
4. **A08 拆为 a–f 六个任务**：v1 用一个任务装下 Step07–14，违反自己定的原子任务原则；其中 A08c（Step11 执行引擎）是全计划最大单体。
5. **Step07 与批量资产生产分离**：Step07 只出规范/锚点/代表资产，批量生产在 Step09 冻结 AssetManifest 后由 Step12 执行——避免架构变化作废大量图像。
6. **A09 分层而非八类全生产**：规格级编译（D1–Step10）已足以暴露核心被单一品类塑形；全生产只选 3 类，且等 R1 结束按实际缺口选。
7. **数量指标不冻结**：旧项目"369 任务 37 小时"只能证明拆分过细，不能证明任何上限合理。R1 用序数档位，R1 后用真实数据校准。

## 4. 从哪里开始

1. 读 `00_master_plan.md`（至少第 1、2、10 节）。
2. 读 `01_development_standards.md` 全部。
3. 打开 `dev/README.md` 看当前状态，找到第一个"待执行"且依赖满足的任务。
4. 当前的两个起点是 **R1-C0**（需要用户参与决策）和 **A02**（纯代码任务），可并行。

## 5. 存量代码导航（截至 2026-07-15，行号会漂移，以符号为准）

工作在 NEWrust 子仓库（独立 git 仓库，父仓库 E:\workwork\CrewAi\AutoDesignMaker 另有自己的 git）。

**已完成的 V2 地基：**
- `crates/adm-new-game-spec/`：A01 产物。`src/spec.rs`（核心聚合，含 `TechnicalConstraints`——A02 要在此系加 ProductEnvelope）、`src/capability.rs`（能力轴）、`src/id.rs`（受验证 ID）、`tests/anti_overfit.rs`（第一层防线：禁止词 + 依赖方向）、`tests/fixtures.rs`（四样例往返）。
- `testdata/game_spec/`：lane_guard / match_grid / branching_story / turn_tactics 四个跨结构样例。

**A08c 要统一的三套存量实现（迁移前保持可用）：**
- `crates/adm-new-application/src/work_unit.rs`：隔离工作单元执行器——声明 output_files/allowed_write_paths、路径安全检查、前后哈希、Unity 批处理 preflight。
- `crates/adm-new-patch/src/lib.rs`：`CodexPatchRunner`——独立补丁执行（CompletionAdapter + 超时）。
- `crates/adm-new-application/src/execution_objects.rs`：`UnattendedExecutionConfig`——修复次数上限、失败分类、独立任务继续、per-group 同步、软停止恢复。

**将被重建的旧路径（当前仍在运行，勿提前删除）：**
- `crates/adm-new-pipeline/src/stages/step08_14.rs`：现有 Step08–14 执行器。已知问题：Step08 任务 `dependencies` 为空数组、Step11 串行提交且未消费并行组、任务合同弱。
- `crates/adm-new-design/src/art_pipeline/stage12.rs`：现有图像质量报告。已知问题：`available=true` 即判 passed，Vision AI 未接入——A08a/A08d 彻底重建。
- `crates/adm-new-design/src/semantic_pipeline.rs` 等：含类型硬编码，A10 阶段删除。

**复用而不重写：**
- AI 配置解析与 adapter 层（`adm-new-ai` / `adm-new-config`）：A05/A08c 复用，不创建角色代理。
- 保存/便携/独立性门禁（`adm-new-save` / `tools/verify-standalone.ps1`）：Step14 与 A10 复用。

## 6. 关键术语表

| 术语 | 含义 |
|---|---|
| GameSpec | 类型化、引擎无关、可版本迁移的游戏规格，唯一权威状态 |
| SpecPatch / CandidateSpecPatch | 进入权威状态的唯一途径；AI 只能产出后者（候选） |
| SpecStore | 规格域单写者，原子提交 + 修订 + 审计 |
| ChangeKernel | 规格域与代码域共享的事务/审计/并发抽象（A04 定义） |
| WorkspaceChangeSet | 代码域变更集合同：读取集/写入集/删除重命名/树哈希/命令权限/受信测试保护 |
| ProductEnvelope | 产品规模包络，进 GameSpec 与哈希，Step06 冻结时判定生产可行性 |
| ExecutionBudget | 本机执行策略（费用/超时/重试/并发），不进规格 |
| 冻结规格 fixture | A07 交付的通道防守完整 GameSpec，贯穿 A08a–f 的统一测试输入 |
| 风格锚点 | Step07 人工确认的参考图像集，后续资产一致性比对基准 |
| 受信测试 | 任务合同中的验收检查，编码代理不可修改，以树哈希防篡改 |
| 修正队列（停车场） | Step11 预算耗尽任务的容器；流水线在无关分支继续 |
| 规格级编译 | 只跑 D1–Step10 不执行生产（Step11–14）的验证方式 |
| R1 停止门 | 通道防守垂直切片：宪章验收场景全通过、玩法闭环成立的 EXE |

## 7. 常见误区（评审中真实出现过的）

1. "生成了 EXE 就算 R1 完成" —— 不算。停止门是内容完整 + 验收场景全通过。
2. "VLM 打分 + 阈值 = 确定性门禁" —— 不是。VLM 是评审证据；硬门禁必须是解码/尺寸/几何等机器可判项。
3. "反过拟合测试已经在 A01 做完了" —— 没有。A01 只是黑名单；第二层测试随每个编译器任务持续执行。
4. "先把三套执行实现统一了再说" —— 顺序反了。先 R0 探针拿真实失败数据，A04 定合同，A08c 才迁移。
5. "任务上限 400、图片上限 200" —— 未冻结。等 R1 数据。
6. "D4 永远人工确认" —— R1 默认人工；R2 允许预批准包络内自动冻结；只有对外发布签署永远人工。
7. "文件存在 = 资产可用" —— 集成的定义是真实引擎加载 + 被场景引用。
