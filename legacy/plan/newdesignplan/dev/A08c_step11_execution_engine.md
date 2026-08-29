# A08c Step11 执行引擎

状态：已完成（v2 权威执行内核，本地，待验收提交；旧入口 legacy 保留）
依赖：A08b、A04、R0（harness 作回归床）
规模警示：全计划最大单体任务；若执行中需要再拆分，允许，但 R1 停止门位置和验收标准不因拆分改变。

## 目标

Step11 成为 SpecPatch 模型在代码域的镜像：现有三套实现统一迁移至 WorkspaceChangeSet，实现隔离生成、串行合入、有界修复与修正队列。

## 变更

1. **迁移边界**：新增 `adm-new-pipeline::stages::step11_v2` 作为 GameSpec v2 的权威 Step11 执行内核，直接消费 A08b 的 `WorkspaceChangeSet` 合同。`work_unit.rs`、`adm-new-patch::CodexPatchRunner`、`execution_objects.rs::UnattendedExecutionConfig`、`AiDevelopmentWorkUnitExecutor` 已标记为 legacy 兼容入口，保留给当前桌面流水线、Step07 图片任务和 R0 回归，后续调用面迁移必须通过 v2 合同执行器。
2. **执行循环**：
   - 消费 A08b 任务合同（写入范围 + 受信测试 + 依赖）；
   - 每任务在隔离工作副本中由编码代理执行（复用 adapter 层），产出变更集；
   - 确定性验证：越界写入直接拒绝；受信测试树哈希校验防篡改；
   - 有界修复：失败证据（编译/测试输出、越界清单）结构化回喂，重试上限来自 ExecutionBudget；失败分类显式（compile/test/scope_violation/timeout/agent_error），scope_violation 不重试同一提示；
   - 修正队列：预算耗尽任务入队，流水线在 DAG 无关分支继续；
   - **并行生成、串行合入**：并发模型第一天存在，R0/R1 默认 `max_workers=1`；单写者串行合入，每次集成失败有唯一责任任务；
   - 每次合入后构建 + smoke；软停止 + 断点续跑。
3. 并发数、重试数、调度策略等参数在本任务启动时结合 R0/R1 数据设计（属 ExecutionBudget，不进规格）。
4. Step11 成功判定 = 全部任务合入且修正队列为空或已全部人工解决。

## 验收（停止门）

- R0 harness 在迁移后全部通过（回归零破坏）。
- 越界、篡改受信测试、陈旧基线三类攻击样例全部拒绝。
- 故障注入（代理超时/输出无效/编译失败）走完分类 → 修复 → 队列全流程，可软停止并续跑。
- 通道防守 fixture 的任务图端到端执行至全部合入（允许人工处置队列残留）。
- 三套旧实现的调用方全部切换到 v2 权威执行模型，或按 `contracts/W1_5_legacy_execution_boundary.md` 写明保留原因、影响面、禁止用途，并用代码级边界测试防止误接回 v2 产品路径。

## 实际结果

- 新增 `stages::step11_v2` facade，并拆分 `types.rs`、`engine.rs`、`support.rs`，避免 A08c 成为大文件。
- 新增 `WorkspaceTaskAgent`、`Step11ExecutionEngine`、`Step11ExecutionState`、`Step11CorrectionQueueItem`、`Step11StopToken` 和可序列化执行报告。
- 执行内核消费 A08b `TrustedTaskGraph`，逐任务校验 `WorkspaceChangeSet`，拒绝无效合同、越界写、受信测试篡改、陈旧基线和证据不完整结果。
- 实现有界重试：`compile/test/timeout/agent_error` 可按预算重试，`scope_violation/conflict/evidence/input` 不重试同一提示。
- 实现修正队列与 DAG 继续调度：失败任务入队后，无关分支继续执行，依赖失败任务的后继标记为依赖阻塞。
- 实现软停止与续跑：已合入任务记录在 `Step11ExecutionState`，下一次运行跳过已合入任务继续剩余 DAG。
- 旧 `WorkUnitExecutor`、`CodexPatchRunner`、`UnattendedExecutionConfig`、`AiDevelopmentWorkUnitExecutor` 增加 legacy/bridge 文档标记；未强行切换当前桌面旧 Step08-14 调用，以免破坏 `game_spec_v2=false` 的渐进替换路径。
- W1.5 修订：保留策略已冻结到 `contracts/W1_5_legacy_execution_boundary.md`。v2 产品 Step11 已要求真实 `WorkspaceTaskAgent`，旧入口只保留给 legacy Step08-14、Step07 图片任务、CLI patch 与 R0 harness；禁止作为 GameSpec v2 产品 Step11 权威执行面。

## 验证

- `cargo fmt --all`：通过。
- `cargo test -p adm-new-pipeline work_unit_executor_retention_boundary_excludes_v2_product_step11 --locked`：通过。
- `cargo test -p adm-new-application development_executor_boundary_allows_v2_bridge_but_not_direct_v2_commit --locked`：通过。
- `cargo test -p adm-new-patch codex_patch_runner_retention_boundary_excludes_gamespec_v2_products --locked`：通过。
- `cargo test -p adm-new-pipeline --test step11_v2`：4 passed。
- `cargo test -p adm-new-pipeline --locked`：173 passed；集成测试 3 + 6 + 10 + 4 + 4 + 3 + 3 passed；doc tests 0。
- `cargo test -p adm-new-application --locked`：82 passed；集成测试 2 passed；doc tests 0。
- `cargo test -p adm-new-patch --locked`：16 passed；doc tests 0。
- `cargo check --workspace --locked`：通过。
- `cargo test -p adm-new-cli --bin adm-new-r0-probe`：3 passed。
- 未运行实际 `adm-new-r0-probe run`，因为该命令需要真实 Unity Editor 绑定；本轮只完成 harness 单元回归。

## 回滚

保留迁移前分支点；harness 双跑（迁移前后）验证等价性，不等价即回退。
