# WorkspaceChangeSet v1 合同

状态：A04 冻结合同（2026-07-15）  
权威类型：`NEWrust/crates/adm-new-change-kernel/src/workspace_change_set.rs`  
失败分类输入：`../evidence/R0_failure_catalog.md`

## 1. 边界

`WorkspaceChangeSet` 是代码域提交给 `ChangeKernel` 的候选变更合同，不是执行结果，也不是允许代理直接写权威工作区的凭证。A04 只冻结类型、验证规则和结果证据；现有 `work_unit.rs`、`adm-new-patch`、`execution_objects.rs` 到 A08c 才迁移。

固定流程：

```
冻结基础树 → 隔离执行 → 候选变更集 → 合同验证 → 受信命令验证 → 串行合入 → 新树哈希 + 审计
```

只有串行合入者能写权威工作区。代理、工具和构建器只能在各自声明的集合内产生候选或派生文件。

## 2. 必填合同

| 字段 | 含义 | 硬约束 |
|---|---|---|
| `schemaVersion` / `changeSetId` | 合同版本与稳定 ID | 严格 schema；未知字段失败 |
| `baseTreeHash` | 隔离执行所基于的完整树 | 小写 SHA-256；合入前必须重新比对 |
| `readPaths` | 可读取的相对路径集合 | 规范 `/` 分隔；禁止绝对路径、盘符、UNC、`.`、父级穿越 |
| `agentWritePaths` | 编码代理写集合 | 所有写入、删除、重命名源和目标均须落在集合内 |
| `trustedToolWritePaths` | 受信工具派生写集合 | 与代理、构建集合不重叠；例如引擎导入产生的 `.meta` |
| `buildOutputPaths` | 编译/打包输出集合 | 与代理、工具集合不重叠，独立归因 |
| `operations` | 文本/二进制写入、删除、重命名 | 携带旧文件预期哈希或缺失预期；payload 自带并校验哈希 |
| `commandPermissions` | 允许执行的命令合同 | 只存本机绑定 ID；不存可执行文件绝对路径或凭证；逐命令超时 |
| `trustedTests` | 受信验收测试 | 基线哈希 + test 命令；不与任何可写集合重叠；结果必须重新哈希 |
| `resourceBudget` | 时间、进程、写入字节、文件数、重试上限 | 全部显式、有限；重试由 ExecutionBudget 决策 |
| `evidence` | 合同来源证据 | 只存稳定 ID、阶段、状态与内容摘要哈希，不存密钥或原始提示 |

合同整体使用确定性序列化生成 `contractSha256`；`WorkspaceTransactionResult` 必须同时绑定 `changeSetId`、`contractSha256` 和 `baseTreeHash`，不能只凭任务 ID 声称完成。

## 3. 写集合与受信测试

1. 代理写、受信工具派生写、构建输出三套集合必须两两不相交，路径父子重叠同样视为冲突。
2. 实际观测路径必须落在对应归因集合；任一越界均为 `scope_violation`，整个隔离候选不得串行合入。
3. 受信测试必须在 `readPaths` 内，但不得落在上述任何写集合内。
4. 执行后逐个重新计算受信测试哈希；缺失、变化或出现未声明测试均拒绝。
5. 工具派生副作用不冒充代理写入。代理声明 `.cs`、受信工具声明 `.meta`、构建器声明 Player 输出，各自单独留证。

## 4. 事务结果

`WorkspaceTransactionResult` 同时记录：结果、失败分类、阶段、副作用状态、结果树哈希、三类实际变更路径、受信测试终态哈希和验证证据。

副作用状态固定为：

- `none`：没有进入权威工作区；
- `staged_only`：只存在隔离/暂存副本；
- `committed`：已串行合入并有结果树哈希；
- `committed_recovery_blocked`：代码已安全提交，但后续编译/测试失败，必须进入修正队列。

成功结果只能是 `committed`。`scope_violation` 和 `evidence` 失败必须为 `none`；`compile` / `test` 可在已有安全提交时记录 `committed_recovery_blocked`，不得伪装成无副作用失败。

## 5. R0 失败分类覆盖

| R0 类别 | v1 合同判据 | 默认处置 |
|---|---|---|
| `input` | schema、路径、哈希、操作或预算无效 | 不重试，修正合同 |
| `agent_error` | 代理未交付声明候选或适配器失败 | ExecutionBudget 内有界重试 |
| `scope_violation` | 实际路径越过归因写集合，或受信测试可写 | 直接拒绝，副作用必须为 `none` |
| `compile` | 受信编译命令失败并留摘要证据 | 有界修正；保留真实副作用状态 |
| `test` | 受信测试/构建/smoke 失败或测试哈希变化 | 测试被修改则直接拒绝；实现失败可修正 |
| `timeout` | 命令超过合同超时 | 终止进程树，按预算处置 |
| `tooling` | 本机绑定缺失、工具不可启动或参数协议不兼容 | 修复本机绑定；机器路径不进入合同 |
| `evidence` | 合同/结果哈希、阶段、日志摘要或受信测试终态缺失 | fail closed，不提交、不升级状态 |

`conflict` 是 A04 新增的内核分类：基础树/修订已变化时拒绝陈旧候选，不自动重放到新基础。

## 6. A08c 前禁止事项

- 不让现有三套执行实现依赖或调用本合同；
- 不把机器安装路径写进 change set；
- 不允许代理创建、修改或替换受信测试；
- 不把“文件存在”“代理自述成功”当成验证证据；
- 不在并行 worker 中直接合入权威工作区；
- 不因外部工具路径兼容需求降低内部相对路径和树哈希约束。

## 7. A08c 迁移入口

A08c 以 R0 harness 为回归床，将 `work_unit`、`CodexPatchRunner` 和执行对象逐个适配为本合同的生产者/消费者。迁移只允许替换适配层，合同字段、失败分类、受信测试保护和串行合入原则若需变化，必须先升 schema 并补兼容测试。
