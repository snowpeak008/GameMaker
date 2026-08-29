# Rust UI 下一轮完整开发计划

日期：2026-07-07  
范围：`RUST/` Rust 桌面端、`adm-application` Step00-14 内容生成、Slint 流水线呈现、发行包 staging  
目标：在已完成 `rust_devflow_executor_v1`、Step00-14 materialized artifacts、devflow run state/report、workbench-to-pipeline brief、Step N-M 区间投影的基础上，继续把“开发流水线”从结构可用推进到内容可验收。

## 0. 执行结果（2026-07-07 23:12）

本计划已完成，并额外推进了原本列为后续工作的 Step07-14 内容 parity。

已完成：

- [x] 检查 Step00-05：结构实现正常，Step spec、`contract_kind`、`pipeline/stepXX/stage.adm` 写入链路和 `adm-application` 测试均正常。
- [x] Step00-06 已新增 `## Structured Stage Content`、`## Acceptance Checklist`、`## Downstream Inputs`。
- [x] Step07-14 已从通用摘要深化为专属结构化内容，覆盖风格确认、程序计划、美术计划、资源对齐、程序执行、美术生产、场景组装和集成验证。
- [x] 阶段详情已读取真实 `pipeline/stepXX/stage.adm`，并显示 `contract_kind`、结构化内容、验收清单和下游输入摘要。
- [x] Step N-M 区间运行已关联严格运行日志和 `pipeline/last_range_run.adm` 最近运行摘要。
- [x] 已重新 release 构建并 stage 双击入口：`fnv64:2744f3483f88f0fd`，`23521792` bytes，`2026/7/7 23:09:15`。
- [x] 已完成验证、进度页更新、AI memory 更新和清理。

剩余不再是 Step07-14 内容 parity，而是外部/体验验收：

- [x] Slint 长文本交互检查、本地可重复 UI audit 和 6 视图 PNG 截图审计已完成。
- [ ] 可选真实窗口人工点击复核仍需可视化运行环境。
- [ ] 真实 AI provider 验收，需要用户提供凭证和运行配置。
- [ ] 真实 Unity PlayMode 验收按约束不自动执行，需要用户手动检查。

## 0.1 追加执行结果（2026-07-07 23:27）

本计划完成后继续推进了本地可做的 Slint 长文本交互 polish。

已完成：

- [x] 将设计摘要、AI 访谈输出、流水线阶段列表、流水线详情日志、补充分析、打包验证、运行日志、SDK 审批队列和 SDK 资源列表纳入 `ScrollView`。
- [x] 为关键长文本报告增加 `wrap: word-wrap`，避免长报告只能显示首屏或横向溢出。
- [x] 新增桌面 UI 布局回归测试，校验关键长文本区域和长列表处在 `ScrollView` 中。
- [x] 重新验证并 stage 双击入口：`fnv64:42848c26c3f3093e`，`24535552` bytes，`2026/7/7 23:25:47`。

剩余仍需要外部或真实人工环境：

- [x] 本地可重复截图审计已由 `--ui-audit` 生成 6 张 PNG。
- [ ] 真实窗口人工点击复核仍需可视化运行环境。
- [ ] 真实 AI provider 验收仍需要用户提供凭证和运行配置。
- [ ] 真实 Unity PlayMode 验收仍按约束不自动执行，需要用户手动检查。

## 0.2 追加执行结果（2026-07-07 23:40）

本计划继续推进了本地可自动化的视觉验收证据。

已完成：

- [x] 在桌面入口新增 release 可调用的 `--ui-audit` 模式，使用 Slint software backend 创建 `MainWindow`、设置 `1280x860` 尺寸并验证六个主视图切换。
- [x] `--ui-audit` 对长文本字段注入 80 行探针并确认设计摘要、AI 访谈、流水线服务、阶段详情、补充分析、打包、构建日志、运行日志和 SDK 审核文本可回写。
- [x] `--ui-audit` 复用 Slint 源码契约检查，确认关键长文本在最近 `ScrollView` 内且包含 `wrap: word-wrap`。
- [x] `--ui-audit` 检查阶段列表、包文件列表、SDK 审核队列和 SDK 资源列表均处在 `ScrollView` 中。
- [x] `--ui-audit` 对 `design`、`pipeline`、`patch`、`package`、`logs`、`sdk` 六个主视图调用 `Window::take_snapshot()`，写出 6 张 PNG 截图。
- [x] 每张截图通过尺寸、字节数、非空像素和颜色变化采样检查。
- [x] 发行目录生成审计证据：`RUST/dist/AutoDesignMaker-rust/ui-visual-audit.adm` 和 6 张 `ui-visual-audit-*.png`，内容包含 `status=passed` 与 `screenshot_artifact_count=6`。
- [x] 重新验证并 stage 双击入口：`fnv64:29c5af171923f62d`，`24717312` bytes，`2026/7/7 23:51:18`。

本地验证：

- [x] `cargo fmt`
- [x] `cargo check -p adm-desktop`
- [x] `cargo test -p adm-desktop`
- [x] `cargo run -p adm-desktop -- --ui-audit .\dist\AutoDesignMaker-rust\ui-visual-audit.adm`
- [x] `cargo run -p adm-desktop -- --smoke`
- [x] `cargo test --workspace`
- [x] `cargo build -p adm-desktop --release`
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`
- [x] `dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe --ui-audit .\dist\AutoDesignMaker-rust\ui-visual-audit.adm`
- [x] `dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe --smoke`

仍需外部或真实人工环境：

- [ ] 可选真实窗口人工点击复核仍需可视化运行环境。
- [ ] 真实 AI provider 验收仍需要用户提供凭证和运行配置。
- [ ] 真实 Unity PlayMode 验收仍按约束不自动执行，需要用户手动检查。

## 0.3 收尾复核结果（2026-07-09 22:50）

本轮按“上一阶段先收尾、当前阶段再收尾”的顺序复核并刷新了本地可完成的所有交付证据。

已完成：

- [x] 复核上一阶段 Step00-14 内容 parity、阶段详情读取、Step N-M 日志关联均已有测试覆盖。
- [x] 重跑 `cargo fmt --check`、`cargo test -p adm-application`、`cargo test -p adm-packaging`、`cargo check -p adm-desktop`、`cargo test -p adm-desktop`、`cargo test --workspace`、`cargo check --workspace`。
- [x] 重新 `cargo build -p adm-desktop --release` 并 `stage-desktop-release`。
- [x] 当前双击入口已更新为 `fnv64:6e4290bdd417082b`，`24715776` bytes，`2026/7/9 22:46:11`。
- [x] 重新运行 staged exe `--ui-audit`，`status=passed`，`screenshot_artifact_count=6`。
- [x] 重新生成 `release-acceptance.adm`，本地 release smoke 通过，`release_hash=fnv64:6e4290bdd417082b`。
- [x] 重新生成 `external-acceptance.adm` 和 `handoff-status.adm`，二者均已对齐最新 release hash。
- [x] 重新生成 source bundle、handoff bundle、handoff instructions、handoff evidence 和 final handoff package。
- [x] 当前 source bundle：`source_file_count=68`，`source_bundle_hash=fnv64:c4de2af7f5bd6c7f`。
- [x] 当前 handoff bundle：`handoff_bundle_file_count=131`，`handoff_bundle_hash=fnv64:84e04f59168a7e4c`。
- [x] 当前 final handoff package：`ready=true`，`package_ready=true`，`delivery_ready=false`，`file_count=142`，`package_hash=fnv64:3307a1bdf45ec0b3`。

仍需外部或真实人工环境：

- [ ] `handoff-status.adm` 仍为 `ready=false`，因为真实 AI provider 和 Unity PlayMode 未满足。
- [ ] 真实 AI provider 验收需要 `OPENAI_API_KEY` 或等效真实 provider 配置。
- [ ] Unity PlayMode 验收需要可用 Unity Editor 路径，并且 runtime runner 必须从 `cli_smoke_runner` 升级为 `unity_playmode`。
- [ ] 严格 release gate 和 `delivery_ready=true` 只能在上述外部验收完成后达成。
## 1. 当前基线

已完成并通过验证：

- 六任务区 Slint UI 已存在，双击入口为 `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`。
- 设计工作台已有真实 `WorkbenchState`、节点 checklist、L4/L5 编辑、右侧四页签、模板、导出、存档和 AI 访谈写回。
- 开发流水线已有 Step00-14 metadata、分组、阶段列表、阶段详情、运行/停止、Step07 风格确认、Step N-M 区间投影。
- Step00-14 已通过 Rust `PipelineRunner` 顺序执行并写出：
  - `pipeline/step00/stage.adm` 到 `pipeline/step14/stage.adm`
  - `pipeline/devflow_run_report.adm`
  - `pipeline/devflow_run_state.adm`
- 每个 Step 文档已有 `## Rust Native Contract Output` 和 `contract_kind`。
- 导出包 manifest 已包含 devflow state/report，package support files 为 13。
- 当前 staged exe：
  - hash: `fnv64:6e4290bdd417082b`
  - size: `24715776`
  - timestamp: `2026/7/9 22:46:11`
  - ui_audit: `RUST/dist/AutoDesignMaker-rust/ui-visual-audit.adm`
  - ui_screenshots: `RUST/dist/AutoDesignMaker-rust/ui-visual-audit-*.png`
- 清理状态：
  - `RUST/target=false`
  - `%TEMP%/adm_desktop_smoke_*=0`
  - `RUST/dist/AutoDesignMaker-rust/.adm_rust_data=false`

## 2. 下一轮核心判断

当前最大缺口不是按钮是否存在，而是 Step00-14 的阶段内容仍然偏“摘要型合同输出”。下一轮应优先做内容 parity，而不是继续扩 UI 外壳。

下一轮不做：

- 不执行真实 Unity PlayMode 自动验收。
- 不修改外部 Unity 项目文件。
- 不重写成非 Slint UI。
- 不恢复旧 Python UI 或 Python 运行时依赖。
- 不引入可拖拽 pane/splitter。

## 3. 下一轮交付目标

### 目标 A：Step00-06 内容 parity 第一批

把设计阶段 Step00-06 从“摘要合同输出”推进到“结构化阶段内容输出”。

实现内容：

- Step00 创意收集：
  - 输出项目画像、创意输入摘要、类型/平台/商业模式、核心体验承诺。
  - 从 `GameDesignBrief` 和 `WorkbenchService::pipeline_brief()` 派生字段。
- Step01 玩法框架：
  - 输出核心循环、玩法系统、玩家行动、反馈结构、系统边界。
- Step02 设计冻结：
  - 输出冻结决策、未决问题、风险列表、下游开发输入。
- Step03 程序需求：
  - 输出程序能力、系统需求、任务列表、验收探针、依赖源。
- Step04 美术需求：
  - 输出资产类别、视觉需求、生产风险、验收口径。
- Step05 程序评审：
  - 输出程序需求覆盖评审、阻塞项、警告项、修正建议。
- Step06 美术评审：
  - 输出美术需求覆盖评审、风格一致性风险、缺失资产项。

验收标准：

- 每个 Step 文档中有稳定 section：
  - `## Step Contract`
  - `## Rust Native Contract Output`
  - `## Structured Stage Content`
  - `## Acceptance Checklist`
  - `## Downstream Inputs`
- `cargo test -p adm-application` 覆盖 Step00-06 的关键 section 和字段。
- smoke 输出仍为 `mode=rust_devflow_executor_v1`、`completed=15`、`artifacts=26; files=34`、`support_files=13`。

### 目标 B：流水线阶段详情读取真实 Step 内容

当前阶段列表可显示状态，但阶段详情仍偏状态摘要。下一轮要让右侧阶段详情读取对应 `pipeline/stepXX/stage.adm` 的结构化内容。

实现内容：

- 为 `inspect_stage_detail()` 增加 Step artifact 内容摘要读取。
- 阶段详情显示：
  - Step 标题
  - Step 状态
  - 当前 Step 的 `contract_kind`
  - `Structured Stage Content` 摘要
  - `Acceptance Checklist` 摘要
  - 下游输入摘要
- 保留旧归档回退：没有 `pipeline/stepXX/stage.adm` 时仍显示状态和 artifacts。

验收标准：

- 点击 Step03 能看到程序需求结构化内容，不只看到“Step 已完成”。
- 点击 Step07 仍保留风格确认信息路径。
- smoke 覆盖至少 Step03/Step04/Step14 的详情内容读取。

### 目标 C：开发流水线运行日志和请求记录关联

当前运行日志和 pipeline service 已接入，但 Step 运行记录仍是并列信息。下一轮把它们关联起来。

实现内容：

- Step N-M 运行时写入 run log：
  - `pipeline_range_started`
  - `pipeline_range_projected`
  - `pipeline_range_completed`
- 日志 context 包含：
  - `archive_id`
  - `start_step_id`
  - `end_step_id`
  - `mapped_core_stage_ids`
  - `devflow_completed_count`
- pipeline service status 显示最近一次运行摘要。

验收标准：

- 点击运行 N-M 后，运行日志页可以按 `pipeline` 过滤看到该次范围运行。
- smoke 校验日志中存在 `pipeline_range_completed`。

### 目标 D：发行包更新与清理

实现完成后必须更新正式双击入口。

验收命令：

- `cargo fmt`
- `cargo test -p adm-application`
- `cargo test -p adm-packaging`
- `cargo check -p adm-desktop`
- `cargo run -p adm-desktop -- --smoke`
- `cargo test --workspace`
- `cargo build -p adm-desktop --release`
- `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`
- `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`

清理要求：

- `cargo clean`
- 删除 `%TEMP%/adm_desktop_smoke_*`
- 确认：
  - `RUST/target=false`
  - `%TEMP%/adm_desktop_smoke_*=0`
  - `RUST/dist/AutoDesignMaker-rust/.adm_rust_data=false`

## 4. 实施顺序

1. 在 `RUST/crates/adm-application/src/core_pipeline.rs` 内抽出 Step 内容渲染 helper。
2. 为 Step00-06 增加 `render_*_structured_content()` 和 `render_*_acceptance_checklist()`。
3. 增加 Step00-06 单元测试，校验 section、contract_kind、downstream inputs。
4. 修改 `RUST/apps/adm-desktop/src/main.rs` 的阶段详情读取逻辑，让它读取 Step artifact 正文摘要。
5. 增加 smoke 断言：Step03/Step04/Step14 详情不是空壳。
6. 增加 Step N-M 日志关联。
7. 全量验证、release staging、更新进度页和记忆、清理产物。

## 5. 预计工作量

下一轮预计 0.8 到 1.5 个工程日。

工作量占剩余全项目约 12% 到 18%：

- Step00-06 内容 parity：约 7%
- 阶段详情读取真实 Step 内容：约 3%
- 运行日志关联和 smoke 覆盖：约 2%
- release staging、文档、清理：约 1% 到 2%

完成后，开发流水线将从“结构与状态可用”推进到“Step00-14 内容可验收”。当前 Step07-14 深层内容 parity 已在执行中补齐，Slint 长文本和 6 视图截图审计也已完成；剩余工作主要集中在可选真实窗口人工点击复核、真实 AI provider 凭证验收，以及用户手动 Unity PlayMode 验收。

## 6. 风险与边界

- Step00-06 内容 parity 不能直接复制 Python 代码，只能按旧插件的信息架构和输出意图在 Rust 中重建。
- 当前 `adm-desktop/src/main.rs` 已经很大；本轮只做必要改动，后续需要拆分 callbacks/view_model，但不在本轮阻断功能完成。
- 如果 Step 内容摘要提取逻辑过度依赖字符串格式，应优先补稳定 section 标记，而不是写脆弱解析。
- 不因为真实 AI provider 缺少凭证而阻断本轮 mock/provider-router 验证。

