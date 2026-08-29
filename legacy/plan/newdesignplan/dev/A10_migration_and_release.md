# A10 迁移与 R2 发布

状态：已完成（迁移/发布 readiness 内核，本地，待验收提交）
依赖：A09

## 目标

安全切换默认写路径，完成旧项目迁移、回滚和独立发布证明。

## 变更

1. 版本迁移器：预览、备份、回滚、幂等测试；迁移失败不改变原保存。
2. 稳定期双轨：同时生成旧交接与 V2 投影，比较语义差异。
3. 达标后新项目默认 `game_spec_v2`；旧项目按项目选择迁移，保留兼容读取窗口。
4. 删除角色化 AI 路径与类型硬编码（含 `semantic_pipeline.rs` 的品类数据分支）。
5. D4 自动冻结（预批准签名包络内）等 R2 无人值守扩展按宪章边界启用。
6. 完整门禁：Web、Rust、UI、跨结构、便携包、跨电脑搬迁、EXE smoke。

## 验收（停止门 / R2 完成定义）

- 迁移失败不改原保存；迁移后可重新打开并继续流水线。
- 复制到另一台满足前提的电脑后不访问旧项目内容。
- 八类规格级 + 3 类全生产门禁保持通过（回归）。
- 对外发布签署由用户人工完成。

## 回滚

开关退回默认关闭；兼容读取窗口保证旧项目不受影响。

## 实际结果

- 新增 `adm-new-pipeline::r2_release`，提供 A10 迁移与发布 readiness 内核。
- 旧项目迁移采用 sidecar-only：读取 `ProjectState`，复用现有 `ProjectState -> GameSpec` 投影，写入 `.game_spec_v2_migration/game_spec.json`、`projection_report.json`、`migration_receipt.json` 和哈希命名备份；原项目状态文件不被覆盖。
- 迁移具备预览、幂等、回滚、失败零写入验证；回滚只移除 sidecar，不删除备份，不改原保存。
- R2 发布 readiness fail closed：A09 回归、迁移安全、Rust/Web 门禁、独立性、便携 smoke、跨电脑搬迁证据、人工发布签署任一缺失都会阻断。
- 新项目 `game_spec_v2` 默认策略由 readiness 报告明确给出；旧项目仍必须显式迁移，保留兼容读取窗口。
- 对外发布签署仍是人工阻断，不由自动化代码伪造通过。

## 已验证

- `cargo fmt --all`
- `cargo test -p adm-new-pipeline --test a10_migration_release`
- `cargo test -p adm-new-pipeline`

## 待正式验收补跑

- `cargo check --workspace --locked`
- `cargo test --workspace --locked`
- `tools/verify-standalone.ps1` / release gate：该工具要求 clean committed worktree，本轮按“验收后再同步仓库”未提交，因此正式发布证据留到总验收阶段。
