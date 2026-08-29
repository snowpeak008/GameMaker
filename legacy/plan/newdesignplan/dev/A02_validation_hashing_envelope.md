# A02 验证、哈希与生产包络

状态：已完成（2026-07-15，本地实现，按用户要求待总验收后提交）
依赖：A01

## 目标

让 `GameSpec` 在进入任何编译阶段前可确定性判定有效性、身份和生产可行性。

## 变更

1. **确定性验证**：ID 唯一性、引用存在性、状态迁移闭合、动作条件/效果完整、资源约束、场景引用、追踪闭合。
2. **规范化与哈希**：稳定排序（无序集合）+ 保序（有序语义序列）、规范 JSON、SHA-256 内容哈希；哈希包含 schema 版本、修订号、父修订哈希、全部语义内容，不含自身哈希字段。
3. **ProductEnvelope**（新增到 `spec.rs` 的 `TechnicalConstraints` 一系）：
   - 序数档位字段（场景数档位、系统复杂度档位、资产规模档位、内容量档位），**不用精确数量上限**；
   - 进入内容哈希；
   - 提供确定性判定 API：`spec 是否在给定包络内`（Step06 冻结门禁将调用）。
   - 档位取值来自 R1-C0 宪章；若宪章未签署，机制先落地，用样例档位测试。
4. **ExecutionBudget**（独立类型，不放入 GameSpec）：费用/超时/重试/并发的本机策略骨架，归入现有 `UnattendedExecutionConfig` 一系管理；序列化验证其不出现在规格快照与哈希输入中。
5. **错误模型**：稳定代码、严重级别、规范路径、相关 ID、修复提示。
6. **Mutation tests**：删除引用、重复 ID、无效迁移、空效果、断裂追踪、超包络规格，全部必须失败。

## 验收（停止门）

- 同一语义输入在字段/集合合法重排后得到同一规范哈希；有序序列重排必须改变哈希。
- 全部无效样例 fail closed；错误含稳定代码与路径。
- ProductEnvelope 参与哈希；ExecutionBudget 被证明不参与。
- 四个 A01 样例 + 超包络负样例通过验证测试。

## 回滚

删除新增验证/哈希模块与 envelope 字段（`deny_unknown_fields` 下旧样例补充 envelope 默认值需同步回退）；不触及现有运行路径。

## 实际结果

- 在 `NEWrust/crates/adm-new-game-spec/src/` 新增 `validation.rs`、`canonical.rs`、`parse.rs`、`envelope.rs`，并由 `lib.rs` 导出严格解析、确定性验证、规范化哈希及包络判定 API。
- `TechnicalConstraints.productEnvelope` 已进入四个 A01 样例和 SHA-256 规范哈希；本机 `ExecutionBudget` 位于 `NEWrust/crates/adm-new-application/src/execution_objects.rs`，未进入 `GameSpec` 序列化与哈希输入。
- Mutation 测试覆盖重复对象 ID、删除引用、无效状态转移、空动作效果、资源范围、断裂追踪和超包络；四个样例均通过。验证同时发现并修复了三份旧样例的终局状态不可达问题。
- `cargo fmt --all -- --check`、`cargo check --workspace --locked` 通过；`adm-new-game-spec` 与 `adm-new-application` 共 89 项测试通过。
- 安全自检通过（4 条规则、415 个命中、151 个精确豁免）；6 个未跟踪新增源码/测试文件经同规则补充扫描为 0 命中，未新增豁免。
- 本任务未接入或替换现有运行路径。因“总验收前不同步仓库”的约束，独立提交延后到总验收通过后执行。
