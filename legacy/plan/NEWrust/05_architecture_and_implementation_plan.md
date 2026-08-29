# NEWrust 架构与实施计划

## 1. 架构原则

NEWrust 采用 contract-first 架构：

```text
foundation -> contracts -> content engine -> application services -> UI view models -> UI
                                      -> packaging/release gates
```

UI、CLI、测试都只能通过 application service 或 gate API 调用业务能力。

## 2. 初始 workspace

初始 crate：

- `adm-new-foundation`：错误、路径、hash、report、evidence level。
- `adm-new-contracts`：核心 typed contract。
- `adm-new-governance`：计划评分、目录门禁、证据门禁。
- `adm-new-cli`：执行 plan gate、doctor、后续本地检查。

后续 crate：

- `adm-new-design`
- `adm-new-content`
- `adm-new-application`
- `adm-new-ui-model`
- `adm-new-desktop`
- `adm-new-packaging`
- `adm-new-ai`
- `adm-new-unity`

## 3. 禁止继承旧问题

禁止从旧 `RUST/` 复制：

- 8000 行级桌面主入口。
- 1000 行级 UI 单文件。
- callback 内直接读写业务文件。
- 字符串解析替代 typed contract。
- release 报告与 exe hash 不绑定。
- fake/mock/static evidence 没有层级标注。

允许迁移：

- 已证明合理的领域名称。
- Step00-14 信息架构。
- release/handoff 命令思路。
- 测试用例意图。

迁移方式：先写 NEWrust contract，再按 contract 重写实现。

## 4. 实施阶段

### Phase 0：计划与骨架

- 创建计划文档。
- 评分达到高于 95。
- 创建 `NEWrust/` workspace。
- 实现基础 plan gate。

### Phase 1：Foundation

- Error type。
- Safe path。
- Stable hash。
- Report rendering。
- Evidence level。
- File size and boundary gate。

### Phase 2：Contracts

- `ProjectIdentity`
- `StageContract`
- `ArtifactRecord`
- `AcceptanceEvidence`
- validate/render/hash。

### Phase 3：Content Engine

- Step00-14 typed output。
- placeholder rejection。
- semantic coverage。
- downstream contract validation。
- local content gate。

### Phase 4：Application Services

- Workbench service。
- Pipeline service。
- Packaging service。
- Run log service。
- SDK service。
- AI service boundary。

### Phase 5：UI Technology Decision

- 对 Slint/egui/Tauri/iced 评分。
- 写 ADR。
- 实现 UI skeleton。
- 绑定真实 view model。

### Phase 6：Full UI

- 六任务区。
- 空态/加载/错误/真实数据态。
- 真实 command。
- 截图和人工验收。

### Phase 7：Packaging and Handoff

- stage release。
- bind build hash。
- delivery doctor。
- handoff status。
- source bundle。
- final manifest。

### Phase 8：External Acceptance

- real AI provider acceptance。
- real Unity PlayMode acceptance。
- blocker report。
- final handoff ready。

## 5. 开发节奏

每轮开发必须产出：

- 代码变更。
- 测试或 gate。
- 文档状态更新。
- 明确下一步。

不允许只产出“计划更新”并宣称工程推进，除非当前任务就是规划。

