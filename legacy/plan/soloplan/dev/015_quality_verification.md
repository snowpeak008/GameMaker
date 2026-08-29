# 015 全局质量验证与收尾

## 目标

验证 009-014 的跨层一致性，确认不改变产品内容、独立性或现有启动/存档行为。

## 原子工作

1. 运行 `cargo fmt --all -- --check`、`cargo check --workspace --locked`、`cargo test --workspace --locked`。
2. 运行 Clippy 并确保本轮新增代码没有新增警告；记录既有警告基线。
3. 运行 Web build、unit、e2e、design-content、i18n、language、UI 和 UI baseline 门禁。
4. 检查 90 张截图中的设计错误、工具错误、AI 配置桌面/窄屏和任务导航状态。
5. 运行 `standalone-boundary-gate`，确认父级 Python 禁止命中仍为 0。
6. 运行根 `AutoDesignMaker.exe --check-launcher` 与便携产品 `--smoke`。
7. 检查 Git diff，不纳入 `node_modules`、截图、运行时数据或发布缓存。
8. 同步总体计划实施结果、AI session history、INDEX 和 freshness。

## 发布说明

正式 `release-gate` 要求干净 Git 树和 24 小时内的 clean-clone 证据。本轮可完成全部源码和开发态验证，但在用户提交当前累计改动并重新运行 `tools/verify-standalone.ps1` 前，发布门禁继续拒绝是正确行为，不以绕过门禁作为修复手段。

## 验收

- 所有开发态验证通过且无残留测试进程。
- 独立性、空白启动、显式恢复、存档保护和根 EXE 行为无回归。
- 计划、原子计划、代码和最终验证结果保持同步。

## 实施结果（2026-07-14）

状态：**已完成开发态验证**。

- `cargo fmt --all -- --check`、`cargo check --workspace --locked`、`cargo test --workspace --locked` 通过；新增草稿模块 60 个 desktop-tauri 测试通过。
- 普通 workspace Clippy 退出码为 0；严格 `-D warnings` 暴露 contracts/save/application 等既有风格警告，本轮修复了 foundation 的一处 `trim_split_whitespace`，未把无关警告清理扩大到本计划。
- Web build、unit、e2e、i18n（2573 keys）、language gate、90 张 UI 截图和 93 条 UI baseline 通过。
- `standalone-boundary-gate` 报告 221 个文件、父级 Python 禁止命中 0；资源清单 7 组通过，试运行 `user_data` 保护通过。
- 根 `AutoDesignMaker.exe --check-launcher`、根入口 smoke 与便携产品 smoke 均通过，默认入口继续创建独立空白会话。便携开发快照已原子替换并 finalize：29,105,664 字节，SHA-256 `4c07249dcf8362524d68312bd069755fdba9280a72ede0d1a4f4741360b7e895`；原有 `user_data` 402 个文件、19,210,560 字节按 `preserved_local` 保留。
- 正式 release gate 未宣称通过：当前累计改动尚未提交，必须在干净工作树中重新生成 clean-clone 证据。
