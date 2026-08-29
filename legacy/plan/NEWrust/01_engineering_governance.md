# NEWrust 工程治理规范

## 1. 单一事实源

每一轮开发必须有三个同步事实源：

- 代码事实：`NEWrust/` 当前文件和测试结果。
- 计划事实：`plan/NEWrust/` 当前规范和评分。
- 证据事实：测试报告、release manifest、acceptance report、截图或运行日志。

三者不一致时，以代码和命令输出为准，计划和报告必须更新；不能反向用旧报告证明新代码完成。

## 2. 目录边界

`NEWrust/` 固定结构：

```text
NEWrust/
├── Cargo.toml
├── README.md
├── apps/
│   └── adm-new-cli/
├── crates/
│   ├── adm-new-foundation/
│   ├── adm-new-contracts/
│   └── adm-new-governance/
├── docs/
├── fixtures/
├── gates/
└── target/          # generated, ignored by git when gitignore is added
```

后续新增 crate 必须满足以下条件：

- 有明确职责。
- 有 README 或 crate-level module 文档。
- 有至少一个测试或 gate 解释为什么暂不测试。
- 不与已有 crate 职责重叠。

## 3. 文件体量上限

硬上限：

- Rust 单文件超过 500 行必须拆分，除非是纯数据 fixture。
- UI 单文件超过 700 行必须拆分。
- 桌面入口超过 300 行必须拆分 callback/view model/service binding。
- 单函数超过 80 行必须拆分或写明例外原因。
- 单 crate public API 超过 15 个核心类型时必须建立模块分层。

软预警：

- Rust 单文件超过 300 行触发审查。
- UI 单文件超过 400 行触发审查。
- CLI match 分支超过 20 个触发 command 模块拆分。

## 4. 变更治理

每个非平凡任务必须包含：

- 需求来源。
- 影响范围。
- 数据契约变化。
- UI 影响。
- 测试证据。
- 未完成项和阻断项。

禁止：

- 只更新 UI 文案但声明业务完成。
- 只跑 smoke 但声明内容质量完成。
- 使用 mock/fake/static evidence 声称真实 AI 或 Unity 通过。
- 生成 release 后不刷新 release/handoff/external acceptance 报告。
- 在 `NEWrust/` 外写新 Rust 运行时代码。

## 5. 计划变更规则

计划可以修改，但必须满足：

- 修改原因写入对应计划文件。
- 如果修改影响评分项，必须更新 `07_scorecard_and_optimization.md`。
- 如果降低原目标，必须明确写成 scope decision，不能隐性缩水。
- 如果新增外部依赖，必须增加本地替代 gate 和真实验收 gate。

## 6. 质量责任

每个阶段都有责任边界：

- Foundation：路径、安全、错误、hash、time、report 格式。
- Contracts：typed data、schema、artifact、archive、provenance。
- Content：Step00-14 内容生成、语义覆盖、下游输入。
- Application：服务命令、状态机、事务、日志。
- UI：view model 投影、中文文案、真实数据交互。
- Packaging：构建、发布、handoff、证据绑定。

任何层向下绕过职责直接写文件或拼字符串，都必须重构。

## 7. 证据命名

所有自动证据使用稳定命名：

- `gates/local-check.adm`
- `gates/content-contract-check.adm`
- `gates/ui-audit.adm`
- `gates/release-manifest.adm`
- `gates/external-acceptance.adm`
- `gates/final-handoff-status.adm`

报告必须包含：

- `status`
- `workspace_hash`
- `build_hash`，如果有构建物
- `command`
- `timestamp`
- `data_root`
- `mock_or_real`
- `blocker_count`

## 8. 完成定义

“完成”必须同时满足：

- 当前代码存在。
- 当前测试或 gate 覆盖了该需求。
- 报告与当前构建 hash 一致。
- 文档没有相反的未完成记录。
- 没有使用低层级证据证明高层级要求。

