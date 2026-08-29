# A05 有界 AI 补全

状态：已完成（本地，待验收提交）
依赖：A04、R1-C0（风险分类依赖 ProductEnvelope 与宪章的人工/自动边界）

## 目标

让 AI 补全成为真实、可观察、不能越权的数据流。

## 变更

1. `PromptPack`：只打包最小相关子图、未决问题、允许写路径和输出 schema。
2. 模型只返回 `CandidateSpecPatch` + 置信度 + 依据 + 未决项；复用现有配置解析与 adapter（`adm-new-ai`/`adm-new-config`），不创建角色代理。
3. Rust 端拒绝：越界路径、未知引用、删除保护字段、schema 不匹配。
4. 确定性门禁计算影响面、冲突和覆盖率；风险分类结合 ProductEnvelope（触碰包络边界/保护字段 = 高风险）。
5. **确认策略数据化**：`attended` / `unattended` / `sample(n)` / `auto_accept` 项目级配置；低风险批量确认是人工模式；`auto_accept` 必须显式配置且留审计。
6. 审计：模型配置 ID、输入/输出摘要哈希、验证结果、确认记录；不含密钥、认证头、未脱敏原始提示。
7. 测试：越界、提示注入、无效 JSON、陈旧补丁、超时、重试、**关闭 AI 模式**（流程可人工完成）。

## 验收（停止门）

- 重复运行 20 次无越权提交。
- 未调用 / 失败 / 拒绝 / 确认 / 提交五种状态在 UI 与产物中可区分。
- 关闭 AI 后同一流程可人工走通。

## 实际结果

- 新增 `adm-new-ai::bounded_completion`，拆分为 `types`、`policy`、`validation`、`service` 与测试模块。
- `PromptPack` 包含最小相关子图、未决问题、允许 JSON Pointer 写路径、输出 schema、模型配置 ID 和 ProductEnvelope。
- 模型输出被限制为 `CandidateSpecPatch`；Rust 端将其转为 `SpecPatch`，先在克隆 `SpecStore` 上做确定性预检，再按确认策略决定是否写入真实 `SpecStore`。
- 项目级 `ConfirmationPolicyConfig` 支持 `attended` / `unattended` / `sample(n)` / `auto_accept`；`auto_accept` 和 unattended 写入都要求路径显式列入 `explicit_auto_accept_paths`。
- `BoundedCompletionRun` 明确序列化五种状态：`not_called` / `failed` / `rejected` / `confirmed` / `committed`。审计只记录模型配置 ID、输入/输出哈希、验证哈希、风险、确认记录和错误摘要，不保存原始提示、密钥或认证头。
- `CompletionAdapter` 增加借用实现，现有真实 adapter 与测试 fake adapter 均可复用现有结构化补全路径。
- 测试覆盖：关闭 AI 后人工提交、低风险候选确认但不提交、显式 auto_accept 提交、未显式 auto_accept 不提交、无效 JSON、schema mismatch、陈旧补丁、越界补丁、提示注入 20 次无越权提交、未知引用确定性拒绝、重试恢复、超时失败。

## 验证

- `cargo test -p adm-new-ai bounded_completion`：9 passed。
- `cargo test -p adm-new-ai`：101 passed。
- 新增 A05 文件行数检查：`service.rs` 328 行、`validation.rs` 254 行、`types.rs` 166 行、`policy.rs` 134 行、`tests.rs` 364 行。

## 回滚

删除补全模块与策略配置；SpecStore 与 adapter 不受影响。
