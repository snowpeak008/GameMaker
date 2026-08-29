# A08e Step13 可执行验收场景

状态：已完成（v2 内核，本地，待验收提交）
依赖：A08c、A08d

## 目标

把 R1-C0 宪章定义、D4 冻结的 AcceptanceScenario 变成可执行验证，覆盖玩法、性能、可访问性和回归。

## 变更

1. 场景执行器：Given/When/Then 映射到引擎内自动化（启动状态构造、输入注入、断言）；无法自动化的场景显式标记人工复核项，不静默跳过。
2. 性能预算来自 `TechnicalConstraints.performance_budgets`；可访问性检查项来自规格。
3. 回归：Step11/12 修正后重跑受影响场景。
4. 证据链：每场景结果关联规格哈希、构建哈希、日志/截图。
5. 测试执行本身可无人值守（unattended），结果判定确定性。

## 验收（停止门）

- 通道防守 fixture 的全部验收场景可执行或已标记人工复核并完成复核。
- 玩法闭环场景（宪章核心承诺）全部通过。
- 注入已知坏构建（删一个系统/一批资产）时对应场景必须失败。

## 实际结果

- 新增 `adm-new-pipeline::stages::step13_v2`，拆分为 facade、`types.rs`、`validation.rs`。
- `run_step13_acceptance_validation` 消费 `GameSpec`、A08c `Step11ExecutionReport`、A08d `Step12AssetProductionOutput` 和 `Step13ValidationPolicy`。
- 默认实现为确定性 headless 验收 runner：场景结果绑定规格哈希、构建哈希、日志哈希、动作列表、性能检查、可访问性检查和失败原因。
- fail closed：
  - Step11 或 Step12 有未解决队列时，相关场景失败；
  - 人工复核场景未完成时返回 `WaitingManualReview`，不静默跳过；
  - 性能场景缺少观测值或超预算时失败；
  - 可访问性复核未通过时失败；
  - 注入 disabled action 或 missing asset 时，对应场景失败。
- 输出 `scenario_execution_results.json`、`performance_validation_report.json`、`manual_review_report.json`、`regression_report.json`、`step13_acceptance_output.json`。
- 当前 runner 不直接启动 Unity；真实 Unity 自动化 runner 后续可在同一验证模型下替换。

## 验证

- `cargo fmt --all`：通过。
- `cargo test -p adm-new-pipeline --test step13_v2`：3 passed。
- `cargo test -p adm-new-pipeline`：168 passed；集成测试 1 + 4 + 3 + 3 passed；doc tests 0。
- 覆盖样例：R1-C0 全验收场景通过、人工复核缺失等待、不跳过、禁用动作坏构建失败、缺失资产坏构建失败。

## 回滚

删除场景执行器；不影响 Step11/12 产物。
