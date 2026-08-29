# Full Project Reproduction Protocol

状态：v3 第一版。

## 1. 禁止误用“全项目复刻”

以下情况不得称为全项目复刻：

- 只复刻 GUI 主流程。
- 只复刻 Step00-14 pipeline。
- 只复刻 `core/`，忽略 `tools/`。
- 只复刻产品 runtime，忽略测试、配置、schema、build 和维护工具。
- 用 9 个功能域 handoff 覆盖数百个 Python 文件。
- 把 `reference`、`defer`、`quarantine` 当作“无需审计”。

## 2. 文件 disposition 枚举

每个 Python 文件必须收敛为以下之一：

| disposition | 含义 | 是否需要 Rust 对应 |
| --- | --- | --- |
| `implemented` | 独立功能已在 Rust 中实现 | 是 |
| `absorbed` | 行为被合并进更大 Rust 模块 | 是，需写明目标 |
| `cli_tool_port` | Python 工具脚本迁移为 Rust CLI/xtask/gate | 是 |
| `test_port` | Python 测试迁移为 Rust/Web/gate 测试 | 是 |
| `asset_or_schema` | 文件只是加载器或 schema/data 辅助，迁移为资产/typed loader | 是 |
| `external_dev_only` | 仅 AI/开发辅助，不属于产品 runtime，但保留 Rust 侧替代或明确外部依赖 | 条件需要 |
| `drop_with_reason` | 明确废弃，不进入 NEWrust | 否，但必须有理由 |

任何 `pending`、`partial`、`defer`、`unclassified` 都是失败状态。

## 3. 全量硬门禁

| gate | 合格条件 |
| --- | --- |
| inventory gate | 根目录、`core/`、`pipeline/`、`knowledge/ucos/`、`tools/` 的源码 `.py` 全部进入 inventory；垃圾目录 `_trash/`、`sandbox/`、`build/skill/` 不计入 |
| disposition gate | 379 个 Python 文件都有最终 disposition |
| authoritative gate | authoritative runtime 文件 100% 有 Rust 目标 |
| tool gate | `tools/**/*.py` 100% 有 `cli_tool_port`、`implemented`、`absorbed` 或 `drop_with_reason`，且每个保留行为都有 Rust CLI/gate/service 映射 |
| test gate | `core/tests/**/*.py` 100% 有 Rust/Web/gate 测试映射 |
| ucos gate | `knowledge/ucos/**/*.py` 100% 有产品/外部/资产裁决 |
| data gate | JSON/TOML/schema/knowledge assets 有迁移或排除方案 |
| UI design gate | Python Tk baseline 采集策略、Web/Tauri 截图策略、差异审查表和交互状态矩阵已定义 |
| UI completion gate | 实现完成时必须具备 Python Tk 基线截图/人工记录 + Web/Tauri 截图 + 差异审查 |
| handoff gate | final handoff 使用文件级矩阵，不是功能域矩阵 |

## 4. 评分规则

继续采用用户确认后的门槛：

- 单项评分 `>=90`。
- 综合加权评分 `>=95`。
- 无硬门禁失败。
- `confidence != low`。

但 v3 增加一条：

```text
只要存在未裁决 Python 文件，综合评分不得高于 89。
```

## 5. 多角色评分

v3 评分角色调整为：

- `Python Whole-Project Auditor`：检查 379 个 Python 文件是否全部进入矩阵。
- `Reachability Analyst`：检查入口、import、动态加载、测试调用是否追踪。
- `Rust Migration Architect`：检查每个文件的 Rust 目标归属。
- `Tooling and Build Reviewer`：检查 tools/build/validator/memory 脚本是否迁移或裁决。
- `UI Pixel Parity Reviewer`：检查 Tk 基线、Web/Tauri 截图和差异表。
- `QA Gate Reviewer`：检查测试迁移、gate、handoff 是否文件级。
- `Data Asset Reviewer`：检查 JSON/TOML/schema/knowledge assets 是否有迁移、加载或排除方案。
- `Red Team Reviewer`：检查伪全量、功能域冒充文件级、历史垃圾误删。

## 6. 开发暂停规则

在以下文件完成并通过评分前，不继续开发新的 Rust 业务功能：

- `01_full_python_file_inventory.md`
- `02_scope_matrix.md`
- `03_file_disposition_matrix.md`
- `04_rust_target_mapping.md`
- `05_v3_scorecard.md`
- `10_v3_atomic_development_plan.md`
- `11_data_asset_migration_matrix.md`
