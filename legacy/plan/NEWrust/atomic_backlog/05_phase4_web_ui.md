# Phase 4: Web UI

状态：开发中。

## ATOM-050 Web Shell and Tokens

| 字段 | 内容 |
| --- | --- |
| 目标 | 初始化 Web UI tokens、AppShell、TopTaskBar、BottomStatusBar、API client。 |
| 依赖 | ATOM-040, ATOM-041 |
| 输入设计文档 | `python_deconstruction/19_ui_reproduction_specs.md`, `newrust_design/06_web_ui_design.md` |
| 涉及 crate/file | `web/src/*` |
| Rust service/command | `get_shell_state` |
| 数据契约 | ShellState |
| UI 影响 | 主壳可见 |
| 验收命令 | `npm run build`; `npm run test`; `npm run e2e -- shell` |
| 完成定义 | 六任务区、状态栏、颜色 token、route switch verified。 |
| 禁止事项 | 不做 fake panel data。 |

开发状态：完成。

完成记录：

- 2026-07-08：将 `web/src/index.html` 从 ATOM-040 placeholder 侧栏改为固定 `TopTaskBar`、`RouteOutlet`、`BottomStatusBar` 的 AppShell，包含六任务区：设计工作台、开发流水线、补充开发、打包阶段、运行日志、SDK 知识库。
- 2026-07-08：在 `web/src/styles.css` 落地 Python UI token：`bg/surface/border/text/primary/success/warning/danger/dark` 等，并实现桌面三栏、两栏和窄屏堆叠规则；未使用营销 hero、装饰渐变或 fake panel data。
- 2026-07-08：在 `web/src/main.js` 实现 `TASKS`、`createShellModel`、`formatProgress`、`invokeCommand`、`getShellState`、`applyRoute`、`applyShellState`、`initApp`，作为后续 API client 和 Tauri command 调用基础。
- 2026-07-08：新增零依赖 `npm run test` 和 `npm run e2e -- shell`，校验六任务区、token、placeholder 移除、底部状态栏、route switch 和 unknown route guard。
- 2026-07-08：回读计划发现 Rust command `get_shell_state` 缺口，新增 `adm-new-tauri-commands/src/shell.rs`，提供 `ShellState` DTO、`DefaultShellCommandService`、`get_shell_state` helper 和序列化/mock/error 测试。
- 验收通过：`cargo test -p adm-new-tauri-commands shell`、`npm run build`、`npm run test`、`npm run e2e -- shell`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- plan-gate`。

## ATOM-051 Design Workbench UI

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 DesignTopbar、DomainSidebar、NodePanel、ResultTabs。 |
| 依赖 | ATOM-042, ATOM-050 |
| 输入设计文档 | `python_deconstruction/19_ui_reproduction_specs.md` |
| 涉及 crate/file | `web/src/features/design/*` |
| Rust service/command | design/save commands |
| 数据契约 | DesignWorkbenchView |
| UI 影响 | 设计工作台 |
| 验收命令 | `npm run test -- design`; `npm run e2e -- design` |
| 完成定义 | profile/domain/node/L4/L5/result tabs render and edit flows work。 |
| 禁止事项 | 不在 TS 重算 coverage/quality。 |

开发状态：完成。

完成记录：

- 2026-07-08：扩展 Rust `DesignWorkbenchView`，由后端提供 `profile`、`domains`、节点描述、role class、checklist items、option groups、设计备注、风险备注、不适用原因、L5 design entities 和实体校验错误，避免 Web UI 硬编码业务结构。
- 2026-07-08：将设计工作台 HTML 改为 `DesignTopbar`、`DomainSidebar`、`NodePanel`、`ResultTabs`、`DesignStatusBar` 结构，保留导出格式、存档/模板/重置动作入口和三栏桌面布局。
- 2026-07-08：新增 `web/src/features/design.js`，实现 DesignWorkbenchView 规范化、领域/搜索/状态/L4 缺失过滤、节点 checklist/option/text/L5 编辑请求构造、结果 tabs 渲染和 design command API client。
- 2026-07-08：新增 design 单测/e2e fixture，只在测试脚本中使用样本数据；运行时 Tauri command 不可用时只显示等待状态，不注入 fake panel data。
- 验收通过：`npm run build`、`npm run test`、`npm run test -- design`、`npm run e2e -- design`、`npm run e2e -- shell`、`cargo test -p adm-new-design`、`cargo test -p adm-new-application design`、`cargo test -p adm-new-tauri-commands design`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`。

## ATOM-052 AI Interview UI

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现内嵌 AI 访谈面板。 |
| 依赖 | ATOM-043, ATOM-051 |
| 输入设计文档 | `python_deconstruction/16_ai_interview_and_completion_contracts.md`, `19_ui_reproduction_specs.md` |
| 涉及 crate/file | `web/src/features/ai-interview/*` |
| Rust service/command | ai commands |
| 数据契约 | AiInterviewState |
| UI 影响 | 设计中间栏底部 AI panel |
| 验收命令 | `npm run test -- ai-interview`; `npm run e2e -- ai-interview` |
| 完成定义 | send/force/mark/archive/running disabled/chat colors verified。 |
| 禁止事项 | 不让 UI merge AI output。 |

开发状态：完成。

完成记录：

- 2026-07-08：新增 `load_ai_interview` 与 `save_ai_archive` command helper；`save_ai_archive` 只更新 `AiInterviewState` 的手动存档路径/时间与 memory marker，不把 AI 输出合并放到 Web。
- 2026-07-08：将设计工作台内嵌 AI 区升级为当前提问、聊天记录、输入框、节点 ID、状态文本和四个动作按钮：`发送回答`、`生成输出`、`标记不准`、`保存访谈存档`。
- 2026-07-08：新增 `web/src/features/ai-interview.js`，实现 `AiInterviewState` 规范化、running/queued 禁用、Ctrl+Enter 提交、消息角色颜色、submit/force/mark/archive 请求构造和 ai command API client。
- 2026-07-08：force 输出只验证 UI 请求路径和 command payload 形状，不伪造 AI provider 结果；真实 full output 仍必须由 Rust 后端 schema/validator/high-confidence 写回链处理。
- 验收通过：`npm run build`、`npm run test`、`npm run test -- ai-interview`、`npm run e2e -- ai-interview`、`npm run e2e -- shell`、`npm run e2e -- design`、`cargo test -p adm-new-application ai_interview`、`cargo test -p adm-new-tauri-commands ai`、`cargo test -p adm-new-ai interview`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`。

## ATOM-053 Pipeline UI and Step07

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 PipelinePanel、StepSidebar、config bar、detail、runtime log、Step07 style grid。 |
| 依赖 | ATOM-044, ATOM-050 |
| 输入设计文档 | `python_deconstruction/19_ui_reproduction_specs.md`, `newrust_design/07_pipeline_artifact_engine_design.md` |
| 涉及 crate/file | `web/src/features/pipeline/*` |
| Rust service/command | pipeline commands |
| 数据契约 | PipelineView |
| UI 影响 | 开发流水线 |
| 验收命令 | `npm run test -- pipeline`; `npm run e2e -- pipeline` |
| 完成定义 | from/to, skip gate, run/stop, selected step, Step07 confirm/regenerate verified。 |
| 禁止事项 | 不在 UI 伪造 Step14 success。 |

开发状态：完成。

完成记录：

- 2026-07-08：扩展 Rust `PipelineView`，由 command view 输出 `PipelineStageView` stage summaries 与 Step07 `StyleOptionView`，Web 不再硬拼 stage status/title/style option。
- 2026-07-08：将 Pipeline HTML 改为 Step sidebar、config bar、detail panel、Step07 style grid、runtime log pane；保留 from/to、skip gate、run/stop、project/AI config/export action 入口。
- 2026-07-08：新增 `web/src/features/pipeline.js`，实现 `PipelineView` 规范化、selected step、runtime lines、run range/stop/confirm style 请求构造和 pipeline command API client。
- 2026-07-08：Step07 UI 渲染后端 style options、确认备注和确认/重新生成动作；不伪造 Step14 success，也不在 UI 绕过 waiting_confirmation。
- 验收通过：`npm run build`、`npm run test`、`npm run test -- pipeline`、`npm run e2e -- pipeline`、`npm run e2e -- shell`、`npm run e2e -- design`、`npm run e2e -- ai-interview`、`cargo test -p adm-new-tauri-commands pipeline`、`cargo test -p adm-new-application pipeline`、`cargo test -p adm-new-pipeline`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`。

## ATOM-054 Utility Panels

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 Patch、Package、Logs、SDK 四个面板。 |
| 依赖 | ATOM-044, ATOM-045, ATOM-050 |
| 输入设计文档 | `python_deconstruction/19_ui_reproduction_specs.md`, `20_parity_gate_test_matrix.md` |
| 涉及 crate/file | `web/src/features/patch`, `package`, `logs`, `sdk` |
| Rust service/command | patch/package/log/sdk commands |
| 数据契约 | PatchRecord, PackageView, LogEntry, SdkSpec |
| UI 影响 | 四个任务区 |
| 验收命令 | `npm run test -- utility-panels`; `npm run e2e -- utility-panels` |
| 完成定义 | empty validation, blocked package, log filter, sdk status flows verified。 |
| 禁止事项 | 不绕过 backend validation。 |

开发状态：完成。

完成记录：

- 2026-07-08：新增 `web/src/features/utility-panels.js`，实现 Patch、Package、Logs、SDK 四个面板的 view normalization、请求构造、DOM 渲染和 command API client；运行时 command 不可用时只显示等待/不可用状态，不注入 fake panel data。
- 2026-07-08：Patch 面板支持空请求提示、`analyze_patch_request`、`list_patches` 和 PatchRecord 表格；非空请求必须提交后端分析，不在 Web 侧生成 PatchRecord。
- 2026-07-08：Package 面板支持 `PackageView` 阻断展示和 `can_package=false` 禁用生成按钮；没有后端源数据时不会合成 package success，也不绕过 Step14/package validation。
- 2026-07-08：Logs 面板支持 `read_log_entries` 级别过滤、`export_log_jsonl` 和 `clear_logs`；补齐 Rust `clear_logs` command helper 与回归测试。
- 2026-07-08：SDK 面板支持新增、选择、批准/待复核/拒绝状态流和 approved context；同时扩展 Rust SDK service/command，使 `AddSdkRequest.source_url` 由后端写入 `SdkSpec`，避免 URL 只停留在 Web UI。
- 验收通过：`npm run build`、`npm run test`、`npm run test -- utility-panels`、`npm run e2e -- utility-panels`、`npm run e2e -- shell`、`npm run e2e -- design`、`npm run e2e -- ai-interview`、`npm run e2e -- pipeline`、`cargo test -p adm-new-tauri-commands patch`、`cargo test -p adm-new-tauri-commands package`、`cargo test -p adm-new-tauri-commands logs`、`cargo test -p adm-new-tauri-commands sdk`、`cargo test -p adm-new-sdk`、`cargo test -p adm-new-application sdk`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`。

## ATOM-055 AI Config Dialog

| 字段 | 内容 |
| --- | --- |
| 目标 | 实现 AI 配置三分类 modal。 |
| 依赖 | ATOM-043, ATOM-050 |
| 输入设计文档 | `python_deconstruction/14_ai_config_adapter_log_contracts.md`, `19_ui_reproduction_specs.md` |
| 涉及 crate/file | `web/src/features/ai-config/*` |
| Rust service/command | config commands |
| 数据契约 | AIConfig |
| UI 影响 | status bar modal |
| 验收命令 | `npm run test -- ai-config`; `npm run e2e -- ai-config` |
| 完成定义 | dev/image/completion tabs, active entry, local/API/file conditional fields verified。 |
| 禁止事项 | 不显示完整 API key after save。 |

开发状态：完成。

完成记录：

- 2026-07-08：新增 AI 配置 modal HTML，入口绑定底部 AI 状态按钮和 Pipeline `AI 配置` 按钮，保留开发API、生图API、补全API 三分类 tab、entry 列表、新建/删除、应用/保存/取消动作。
- 2026-07-08：新增 `web/src/features/ai-config.js`，实现 `AIConfig`/`ApiCategory`/`ApiEntry` 规范化、active entry 切换、条件字段渲染、extra_json object 校验、`load_ai_config`、`validate_ai_config`、`save_ai_config` 和 `completion_adapter_spec` API client。
- 2026-07-08：API 类型显示 API URL/API Key 字段；Codex CLI/file 类型显示 TOML/JSON path；local CLI 类型显示等待后端验证状态；custom 类型显示 extra JSON；不在 Web 侧构造 adapter。
- 2026-07-08：API key 输入框只显示 `********` 掩码；用户不修改时保存 payload 保留原 key，但 DOM/测试用 display config 不包含完整 key，满足“不显示完整 API key after save”约束。
- 2026-07-08：新增 AI Config fixture、unit 和 e2e 覆盖 dev/image/completion tab、active entry、条件字段、extra JSON、API key 掩码和保存 payload。
- 验收通过：`npm run build`、`npm run test`、`npm run test -- ai-config`、`npm run e2e -- ai-config`、`npm run e2e -- shell`、`npm run e2e -- design`、`npm run e2e -- ai-interview`、`npm run e2e -- pipeline`、`npm run e2e -- utility-panels`、`cargo test -p adm-new-tauri-commands config`、`cargo test -p adm-new-application ai_config`、`cargo test -p adm-new-ai ai_config`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`。
