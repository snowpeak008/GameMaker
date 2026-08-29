# NEWrust 数据规范

## 1. 数据原则

数据规范优先于 UI 和文本生成。

硬规则：

- 所有跨层数据必须 typed。
- 所有持久化数据必须有 schema_version。
- 所有生成内容必须有 provenance。
- 所有报告必须标注 evidence_level。
- 所有运行写入必须进入 data_root，不污染源码目录。

## 2. 核心模型

基础模型：

- `ProjectIdentity`
- `WorkbenchState`
- `StageContract`
- `ArtifactRecord`
- `ArchiveManifest`
- `RunEvent`
- `AcceptanceEvidence`
- `ProviderInvocationRecord`
- `UiViewModelSnapshot`

每个模型必须支持：

- validate
- render report
- parse report
- hash stable content
- schema version migration

## 3. 制品规范

每个制品记录包含：

- artifact_id
- kind
- producer
- consumers
- path
- content_hash
- schema_version
- evidence_level
- created_at
- source_inputs

制品不可只靠路径存在判断完成；必须检查内容、hash 和 schema。

## 4. Evidence level

统一证据层级：

| Level | 含义 | 可证明 |
| --- | --- | --- |
| static | 静态结构或源码检查 | 结构存在 |
| mock | mock provider 或 fake runner | 调用链可运行 |
| local | 本地真实程序但无外部依赖 | 本地集成健康 |
| real | 真实外部 provider/Unity/用户确认 | 外部验收通过 |

低层证据不能证明高层需求。

## 5. 数据根

默认数据根：

- 开发：`NEWrust/.adm_newrust_data`
- 测试：临时目录
- 发行：release bundle 内显式 data root 或用户指定 data root

禁止：

- 测试污染发行 data root。
- release gate 使用旧 data root 生成新报告。
- handoff 报告引用不存在或旧 hash 的构建物。

## 6. 日志规范

运行日志使用 append-only event：

- event_id
- event_type
- level
- scope
- message
- context
- artifact_hashes
- timestamp

重要事件：

- plan_gate_passed
- data_contract_validated
- stage_started
- stage_completed
- ui_command_invoked
- release_staged
- external_acceptance_checked
- blocker_recorded

## 7. 迁移规范

任何 schema 变化必须提供：

- old_version
- new_version
- migration function
- migration test
- incompatible field list

不能用 ad hoc 字符串替换迁移正式数据。

