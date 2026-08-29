# A04 ChangeKernel、SpecStore 与 WorkspaceChangeSet 合同

状态：已完成（2026-07-15，本地实现，按用户要求待总验收后提交）
依赖：A03、R0（消费 R0 失败分类清单）

R0 输入：`../evidence/R0_failure_catalog.md`。A04 的合同与测试必须逐项覆盖该清单的稳定分类和末节约束。

## 目标

建立规格域与代码域共用的单写者变更内核；SpecStore 作为第一个实现；定义（不迁移）代码域变更集合同。

## 变更

1. **ChangeKernel 抽象**：事务（base 修订 + base 哈希 + 陈旧拒绝）、验证挂点、原子提交、不可变审计记录、并发冲突拒绝。具体接口字段在本任务启动时结合 R0 失败分类清单设计（总计划 10.1：原则已冻结，细节按需设计）。
2. **SpecStore**（规格域实现）：
   - `SpecPatch`：base_revision + base_hash、每操作声明路径/旧值预期/新值/来源/理由/证据；
   - 内存副本先行应用 → schema/引用/语义/影响验证 → 唯一提交者原子提交 → 新修订 + 审计；
   - 人工、模板、迁移器、AI 同一入口，无旁路。
3. **WorkspaceChangeSet 合同定义**（只定义接口与合同，不迁移现有三套实现——迁移在 A08c）：
   - 必须覆盖：读取集合、写入集合、删除/重命名、二进制文件、基础树哈希、命令权限、资源预算、**受信测试不可由编码代理修改**；
   - 合同以 R0 暴露的真实失败模式为校验依据。
4. 规格快照只存相对资源引用；机器路径/凭证由本机绑定层管理。

## 验收（停止门）

- 陈旧补丁、越界写入、并发冲突全部拒绝并留审计。
- 同一补丁序列重放得到相同修订链与哈希链。
- 现有三套执行实现（work_unit / adm-new-patch / execution_objects）**未被触碰且测试仍通过**。
- WorkspaceChangeSet 合同文档评审通过，并覆盖 R0 清单的全部失败类别。

## 回滚

删除新 crate/模块；现有运行路径零影响（本任务不迁移任何存量实现）。

## 实际结果

- 新增独立 `NEWrust/crates/adm-new-change-kernel/`，实现通用 `ChangeKernel<C>`、共享失败分类/副作用/证据/审计合同，以及线程安全单写者 `SpecStore`；未把事务逻辑塞入现有 storage、patch 或 application。
- `SpecPatch` 强制携带 base revision/hash、精确声明写路径、旧值预期、新值/删除、来源、理由和证据摘要。补丁先在 JSON 副本执行，再做实际差异范围检查、严格反序列化、A02 语义验证、自定义验证挂点和规范哈希，最后一次性更新权威快照。
- revision 与 parentHash 由内核独占管理；人工、模板、迁移器、AI 只有同一个 `submit` 入口。陈旧基础、旧值冲突、声明范围不符、受管字段写入、结构/语义无效和证据缺失均拒绝且追加不可变审计，权威快照不变。
- 审计记录绑定完整补丁 SHA-256；规格域审计同时保留补丁副本（来源、理由、操作、证据引用）。相同两补丁序列在独立 store 重放得到相同 revision/hash 和 audit record ID 链。
- `WorkspaceChangeSet` v1 Rust 合同覆盖读取、代理写、受信工具派生写、构建输出、文本/二进制、删除/重命名、基础树哈希、命令权限、本机资源预算及受信测试防篡改；三类写集合独立归因，结果必须绑定完整合同哈希并重新哈希每个受信测试。
- R0 的 `input / agent_error / scope_violation / compile / test / timeout / tooling / evidence` 全部成为稳定分类；另加 `conflict` 表达陈旧修订/树。编译或测试可如实记录 `committed_recovery_blocked`，范围/证据失败强制无权威副作用。
- 评审合同写入 `plan/newdesignplan/contracts/WorkspaceChangeSet_v1.md`；其中明确 A08c 前不迁移三套实现，并逐项映射 R0 失败目录、工具派生 `.meta`、本机绑定和可信测试规则。
- `adm-new-change-kernel` 24 项测试通过，包含并发竞争、陈旧/越界拒绝、原子回滚、确定性重放、结构 mutation、验证挂点、二进制/重命名、工具派生写、受信测试篡改和全部 R0 分类；`cargo clippy -D warnings` 通过。
- `adm-new-application` 77 项单元 + 2 项集成测试、`adm-new-patch` 15 项测试全部通过；A04 未迁移或接入 `work_unit.rs`、`adm-new-patch`、`execution_objects.rs` 的现有运行路径。
- `cargo fmt --all -- --check`、`cargo check --workspace --locked`、安全自检（4 条规则、418 个命中、151 个精确例外）通过；A03/A04 新文件补充扫描 0 命中；独立性边界扫描通过（252 个文件、父项目/Python 禁止引用 0）。
- 因“总验收前不同步仓库”的约束，独立提交延后到总验收通过后执行。
