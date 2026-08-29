# 005 验证与交付

## 目标

用最小但有代表性的验证覆盖本轮治理工具开发，避免只写报告不验证。

## 原子工作

1. `cargo fmt --check`
2. `cargo check --workspace --locked`
3. `cargo test -p adm-new-governance`
4. `cargo test -p adm-new-cli`
5. `cargo test -p adm-new-tauri-commands`
6. `cargo test -p desktop-tauri`
7. `npm test`（前端 shell 启动策略字段）
8. `npm run build`（为 desktop-tauri 测试提供嵌入式 `web/dist`）
9. `cargo test --workspace --locked`
10. `cargo run -p adm-new-cli -- standalone-boundary-gate`
11. `cargo run -p adm-new-cli -- design-sync-audit`
12. `tools/build-portable.ps1 -DevelopmentSnapshot`、stage smoke、finalizer dry-run 与显式 finalization
13. `cargo test -p adm-new-root-launcher --locked`、`tools/build-root-launcher.ps1`、根 `AutoDesignMaker.exe --check-launcher` 与真实 GUI 启动验证

## 验收

- 所有新增测试通过。
- 两个 CLI 命令能在当前仓库产生可解释报告。
- 设计同步审计必须把换行/EOF 差异归类为 `format_only_files`，不能计入语义 `difference_count`。
- 默认桌面启动空项目，显式恢复路径仍有单独回归测试。
- 本轮实测 `cargo test -p adm-new-governance`：50 passed。
- 本轮实测 `cargo test -p adm-new-cli`：16 passed。
- 本轮实测 `cargo test -p adm-new-tauri-commands`：64 passed。
- 本轮实测 `cargo test -p desktop-tauri`：56 passed。
- 本轮实测 `npm test`、`npm run build`：通过。
- 本轮实测 `cargo test --workspace`：通过。
- 本轮实测 `standalone-boundary-gate`：`status=passed`、`boundary_scan:forbidden_hit_count=0`。
- 本轮最终实测 `design-sync-audit`：`status=passed`、`difference_count=0`、`format_difference_count=144`。
- 本轮便携开发快照构建、stage smoke 与事务 finalizer：通过；receipt 为 `finalized`，原有 382 个本地试用数据文件及摘要完整保留。
- 根启动入口必须实测：原生 EXE 自检退出码为 `0`；GUI 子进程来自 `dist/AutoDesignMaker-NEWrust/AutoDesignMaker.exe`，窗口句柄有效，并创建 103 个节点全部为 `not_started` 的空白草稿。
- 本轮原生入口最终实测：根 EXE 为 398,336 字节，`--check-launcher=0`；真实子进程 PID 21204、窗口句柄 8915078；新草稿 103/103 节点为 `not_started`，备注/玩法系统/AI 消息均为 0，流水线为 `idle` 且阶段数为 0；运行日志确认随后干净退出。
- 本轮入口变更后的 `cargo check --workspace --locked`、`cargo test --workspace --locked`、`cargo clippy -p adm-new-root-launcher --all-targets --locked -- -D warnings` 均通过。
- 本轮入口变更后的独立性门禁：`status=passed`、扫描 218 个文件、`forbidden_hit_count=0`。
- 附加 `cargo clippy --workspace --all-targets -- -D warnings` 未作为本轮验收项：仓库既有 `structured_md.rs`、`adm-new-contracts` 等文件存在 `trim_split_whitespace`、`derivable_impls`、`needless_borrow` 基线告警；本轮不扩展为无关重构。

最终全 workspace 测试、格式检查和报告复跑完成后，以本文件最后一次更新的数字为交付基线。
