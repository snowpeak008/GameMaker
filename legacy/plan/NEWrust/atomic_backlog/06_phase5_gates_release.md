# Phase 5: Gates and Release

状态：开发完成。

## ATOM-060 Rust Parity Test Gate

| 字段 | 内容 |
| --- | --- |
| 目标 | 聚合 contract/storage/domain/application tests 为 parity gate。 |
| 依赖 | ATOM-030 至 ATOM-037 |
| 输入设计文档 | `newrust_design/09_testing_gates_release_design.md` |
| 涉及 crate/file | `crates/adm-new-governance`, `gates/` |
| Rust service/command | `adm-new-cli parity-gate` |
| 数据契约 | GateReport |
| UI 影响 | 无 |
| 验收命令 | `cargo run -p adm-new-cli -- parity-gate` |
| 完成定义 | gate report includes all required Rust checks。 |
| 禁止事项 | 不把 skipped test 当 success。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-governance` 新增 `parity_gate_report`，聚合 workspace member 清单、contract/storage/domain/application/command 必需测试标记、ignored test marker 扫描和 source hash。
- 2026-07-08：在 `adm-new-cli` 新增 `parity-gate` 子命令；该命令不只检查静态清单，还实际执行 `cargo test --workspace --quiet`，失败时写入 blocker，避免 skipped/missing test 被当成 success。
- 2026-07-08：新增 `NEWrust/gates/README.md`，说明 `plan-gate.adm` 与 `parity-gate.adm` 为 CLI 生成的 gate evidence；实际运行已生成 `NEWrust/gates/parity-gate.adm`。
- 2026-07-08：自查中发现 gate 源码字面量会触发自身 `#[ignore]` 扫描误判，已改为运行时拼接检测 marker，防止 gate 自污染。
- 验收通过：`cargo test -p adm-new-governance parity`、`cargo check -p adm-new-cli`、`cargo run -p adm-new-cli -- parity-gate`、`cargo fmt --check`、`cargo check --workspace`、`cargo test -p adm-new-governance`。

## ATOM-061 UI Parity Gate

| 字段 | 内容 |
| --- | --- |
| 目标 | 建立 Playwright screenshot + DOM assertions gate。 |
| 依赖 | ATOM-050 至 ATOM-055 |
| 输入设计文档 | `python_deconstruction/19_ui_reproduction_specs.md` |
| 涉及 crate/file | `web/tests/e2e`, `gates/` |
| Rust service/command | optional `adm-new-cli ui-gate` wrapper |
| 数据契约 | UI evidence manifest |
| UI 影响 | screenshots |
| 验收命令 | `npm run e2e`; `cargo run -p adm-new-cli -- ui-gate` |
| 完成定义 | screenshots for shell/design/pipeline/Step07/patch/package/logs/sdk/config saved。 |
| 禁止事项 | 不接受 blank screenshots。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `web/scripts/ui-gate.mjs` 建立真实 Playwright Chromium 截图门禁，通过本地 HTTP server 加载 `web/dist`，避免 `file://` ES module CORS 干扰。
- 2026-07-08：UI gate 覆盖 `shell`、`design`、`pipeline`、`step07`、`patch`、`package`、`logs`、`sdk`、`config` 九类界面证据；每张截图均执行 PNG signature、尺寸、文件大小和像素变化度检查，防止 blank screenshot 被接受。
- 2026-07-08：在 Web shell 中新增 URL 初始状态入口：`route` 控制初始面板，`step07=1` 只显示等待后端生成风格选项的真实空态，不注入 fake backend data，`aiConfig=1` 打开 AI 配置弹窗。
- 2026-07-08：在 `adm-new-cli` 新增 `ui-gate` 子命令，串联 `npm.cmd run build`、`npm.cmd run e2e`、`npm.cmd run ui-gate`，并输出 `gates/ui-gate.adm`。
- 2026-07-08：Playwright browser 已安装并用于真实截图；本地 Chrome/Edge 直连在当前环境下存在 headless GPU 进程异常，因此 gate 固定优先使用 Playwright 管理的 Chromium，不使用伪截图。
- 验收通过：`npm.cmd run build`、`npm.cmd run test`、`npm.cmd run e2e`、`npm.cmd run ui-gate`、`cargo run -p adm-new-cli -- ui-gate`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`。

## ATOM-062 Package Gate

| 字段 | 内容 |
| --- | --- |
| 目标 | 建立 package success/blocked/release manifest gate。 |
| 依赖 | ATOM-036, ATOM-044, ATOM-054 |
| 输入设计文档 | `python_deconstruction/17_packaging_contracts.md` |
| 涉及 crate/file | `crates/adm-new-packaging`, `crates/adm-new-governance`, `gates/` |
| Rust service/command | `adm-new-cli package-gate` |
| 数据契约 | PackageManifest, PackageValidationReport |
| UI 影响 | 无 |
| 验收命令 | `cargo run -p adm-new-cli -- package-gate` |
| 完成定义 | missing changed_files and missing unity summary block correctly。 |
| 禁止事项 | 不 accept UI-only readiness。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-governance` 新增 `package_gate_report`，通过 `adm-new-packaging::PackagingService` 真实生成 success、missing changed_files、missing unity summary 三类 package evidence，不使用 UI 状态推断打包 readiness。
- 2026-07-08：package gate 校验 `PackageValidationReport`、`PackageBuildReport`、`PackageManifest` 三层状态一致性，固定 `outputs/package/current` 输出契约，并要求 9 个 `REQUIRED_INTEGRATION_CHECKS` 全部出现在报告中。
- 2026-07-08：blocked 场景明确验证 `PACKAGE-NO-ACTUAL-PROJECT-CHANGES` 与 `PACKAGE-UNITY-VALIDATION-MISSING`，同时要求 `PACKAGE_NOTES.md` 文本包含对应 blocker，避免只在结构体里标记但交付说明缺失。
- 2026-07-08：在 `adm-new-cli` 新增 `package-gate` 子命令，运行后输出 `gates/package-gate.adm`，可被后续 release gate 聚合。
- 验收通过：`cargo test -p adm-new-governance package_gate`、`cargo test -p adm-new-packaging`、`cargo test -p adm-new-tauri-commands package`、`cargo run -p adm-new-cli -- package-gate`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`。

## ATOM-063 Release Gate

| 字段 | 内容 |
| --- | --- |
| 目标 | 聚合 fmt/check/test/web/e2e/plan/parity/package gates。 |
| 依赖 | ATOM-060, ATOM-061, ATOM-062 |
| 输入设计文档 | `newrust_design/09_testing_gates_release_design.md`, `10_risk_register.md` |
| 涉及 crate/file | `crates/adm-new-governance`, `apps/adm-new-cli`, `gates/` |
| Rust service/command | `adm-new-cli release-gate` |
| 数据契约 | ReleaseGateReport |
| UI 影响 | 无 |
| 验收命令 | `cargo run -p adm-new-cli -- release-gate` |
| 完成定义 | release blocked unless every required gate passes。 |
| 禁止事项 | 不允许 manual unchecked release。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-governance` 新增 `release_gate_report`，声明并执行 10 项 release 必需检查：`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、Web build、Web e2e、Web UI screenshot gate、plan gate、parity gate、package gate、anti-fake scan。
- 2026-07-08：release gate 直接执行真实 Rust/Web 命令，并内联调用 `plan_gate_report`、`parity_gate_report`、`package_gate_report` 聚合状态；任一失败都会写入 blocker，不能手工绕过。
- 2026-07-08：新增反伪证据扫描，排除 `target`、`node_modules`、`dist`、`gates` 生成目录后检查源码/脚本/文档中是否出现伪证据通过类 marker，防止 fake/static/blank evidence 被当作 release 依据。
- 2026-07-08：在 `adm-new-cli` 新增 `release-gate` 子命令，运行后输出 `gates/release-gate.adm`，作为 ATOM-064 handoff 的上游证据。
- 验收通过：`cargo test -p adm-new-governance release_gate_declares`、`cargo run -p adm-new-cli -- release-gate`。`release-gate` 内部已通过 Rust fmt/check/test、Web build/e2e/ui-gate、plan/parity/package gate 和 anti-fake scan。

## ATOM-064 Development Handoff Report

| 字段 | 内容 |
| --- | --- |
| 目标 | 生成最终开发证据索引，映射 Python evidence -> NEWrust files -> tests -> gates。 |
| 依赖 | ATOM-063 |
| 输入设计文档 | 全部 `plan/NEWrust` |
| 涉及 crate/file | `gates/reports/final_handoff_manifest.*` |
| Rust service/command | `adm-new-cli handoff-report` |
| 数据契约 | HandoffManifest |
| UI 影响 | 无 |
| 验收命令 | `cargo run -p adm-new-cli -- handoff-report` |
| 完成定义 | 每个功能有 evidence、implementation、test、gate reference。 |
| 禁止事项 | 不把未实现计划列为已完成。 |

开发状态：完成。

完成记录：

- 2026-07-08：在 `adm-new-governance` 新增 `HandoffManifest` / `HandoffEntry` 数据结构和 `handoff_report`，生成 `gates/reports/final_handoff_manifest.json` 与 `gates/reports/final_handoff_manifest.md`。
- 2026-07-08：handoff report 覆盖 9 个功能证据映射：source authority、data contracts、design workbench、AI config/interview、pipeline/artifacts、save runtime、utility panels、UI parity、release governance。
- 2026-07-08：每个映射条目都强制包含 Python evidence、NEWrust implementation files、test commands、gate refs；命令会校验文件存在，`.adm` gate 引用必须包含 `status=passed`，缺项即 blocked。
- 2026-07-08：在 `adm-new-cli` 新增 `handoff-report` 子命令，运行后输出 `gates/handoff-gate.adm`，并同步刷新 `NEWrust/gates/README.md` 的 gate 说明。
- 验收通过：`cargo test -p adm-new-governance handoff`、`cargo fmt --check`、`cargo check --workspace`、`cargo test --workspace --quiet`、`cargo run -p adm-new-cli -- release-gate`、`cargo run -p adm-new-cli -- handoff-report`。
