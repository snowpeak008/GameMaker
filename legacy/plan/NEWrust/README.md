# NEWrust v2 开发计划总览

日期：2026-07-08

状态：v2 主链路复刻计划已完成；2026-07-09 起被 `full_project_reproduction/` v3 全项目复刻计划接管。

重要变更：

- v2 目标是 Python 主运行链路高保真复刻。
- 用户已明确要求整个 Python 项目 Rust 化复刻。
- 因此，v3 计划要求所有 Python 文件、工具、测试、配置、数据资产都有最终处置。
- 在 v3 文件级矩阵和评分通过前，禁止继续按 v2 原子 backlog 开发新业务功能。

## 1. 总目标

NEWrust 是 AutoDesignMaker 的新一轮 Rust 产品线，不继续修补旧 `RUST/`。本轮目标是基于现有 Python 项目做高保真功能复刻，同时隔离垃圾内容、历史残留和旧 Rust 单体化问题。

技术路线固定为：

```text
Tauri + Web UI + Rust 后端
```

职责边界：

- Rust 后端负责数据契约、业务服务、内容流水线、存档、AI、打包、验收和 release gates。
- Web UI 只负责 view model 展示和用户交互。
- Web UI 不直接写业务数据，不直接读写制品文件。
- Tauri command 只作为桥接层，业务逻辑必须进入 Rust application service。

## 2. 强制执行顺序

1. Python 项目可达性解构。
2. Python 系统、数据、UI、pipeline、存档、AI、打包、制品验证拆解。
3. Python 解构多角色评分循环。
4. NEWrust 详细设计。
5. NEWrust 设计多角色评分循环。
6. 原子开发计划。
7. 原子计划评分循环。
8. 满足门禁后才进入开发。

禁止跳过 Python 解构直接开发。

## 3. 每小阶段防偏移规则

每完成一个小阶段，必须重新阅读本文件和当前阶段的 scorecard，确认没有偏离以下约束：

- 当前工作是否仍在阶段顺序内。
- 是否提前进入 UI 或开发。
- 是否把垃圾内容误纳入核心复刻范围。
- 是否把 mock/fake/static 证据当成 real evidence。
- 是否满足当前阶段评分规则。
- 是否需要更新计划或风险记录。

执行记录中必须写明：

```text
plan_reread=done
drift_detected=false|true
drift_action=<none|修正动作>
```

## 4. Python 内容分类

所有 Python 文件先分类：

- `authoritative`：真实入口可达，必须拆解和复刻。
- `reference`：有参考价值，但不直接复刻。
- `quarantine`：垃圾、废弃、重复、临时、历史残留。

`quarantine` 只是设计阶段标记，不删除真实 Python 文件。最终设计完成后，临时垃圾标记必须收敛为正式结论：`drop`、`defer` 或 `reference`。任何真实删除必须另行取得用户确认。

## 5. 评分规则

不再采用“每项都必须大于 95”的规则。

合格条件：

- 单项评分 `>=90`。
- 综合加权评分 `>=95`。
- 无硬门禁失败。
- `confidence` 不能为 `low`。

硬门禁失败时，不看平均分，直接回炉。

## 6. 多角色评分

每轮评分使用多个第三人称角色：

- `Python Archaeologist`：检查 Python 可达性、垃圾隔离、入口证据。
- `Product Parity Reviewer`：检查功能复刻完整性。
- `Data Contract Architect`：检查数据、schema、制品、存档契约。
- `UI Reproduction Reviewer`：检查 UI 信息架构、互动路径、高保真复刻。
- `Rust Architecture Reviewer`：检查 Tauri/Rust 架构和 crate 边界。
- `QA Release Reviewer`：检查测试、gate、release、handoff 证据。
- `Red Team Reviewer`：专门查伪完成、证据矛盾、遗漏和范围漂移。

Red Team 不直接以主观意见否决，但发现以下情况时必须回炉：

- 证据矛盾。
- 低层证据冒充高层验收。
- 没有入口证据却列为必须复刻。
- UI 互动没有追到后端行为。
- 数据写入路径不清楚。

## 7. 像素级复刻定义

采用工程可执行定义：

- 信息架构 100% 复刻。
- 交互路径 100% 复刻。
- 业务状态变化 100% 等价。
- 指定分辨率下布局区域高一致。
- 允许字体抗锯齿、原生控件边框、DPI 渲染差异。
- 使用 Playwright 截图、DOM 状态、交互记录和人工审查组合验收。

## 8. 目录

```text
plan/NEWrust/
├── README.md
├── 00_execution_protocol.md
├── python_deconstruction/
├── newrust_design/
└── atomic_backlog/
```

开发目录：

```text
NEWrust/
├── apps/desktop-tauri/
├── web/
├── crates/
└── gates/
```

当前已有的 `NEWrust` 初始 Rust workspace 只作为治理和 contract-first 骨架。后续会按 v2 计划迁移到 Tauri + Web UI + Rust 后端结构，不继续按旧 Slint 方向扩展。
