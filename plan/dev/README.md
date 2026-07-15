# 原子开发顺序

执行规则：严格按编号推进；每个任务独立提交；未满足本任务停止门时不得启动后续依赖任务。

| 顺序 | 文件 | 状态 | 目标 |
|---|---|---|---|
| A01 | `A01_game_spec_foundation.md` | 已完成 | 建立通用类型核心和四个反过拟合样例 |
| A02 | `A02_validation_and_hashing.md` | 待执行 | 建立确定性验证、规范化和内容哈希 |
| A03 | `A03_capability_decision_graph.md` | 待执行 | 用能力谓词生成动态决策图 |
| A04 | `A04_single_writer_spec_store.md` | 待执行 | 建立补丁、修订、并发控制和审计日志 |
| A05 | `A05_bounded_ai_completion.md` | 待执行 | 把 AI 补全接入候选补丁闭环 |
| A06 | `A06_design_workbench_adapter.md` | 待执行 | D1–D4 双轨接入与兼容投影 |
| A07 | `A07_pipeline_steps_00_06.md` | 待执行 | 重构设计编译和冻结阶段 |
| A08 | `A08_pipeline_steps_07_14.md` | 待执行 | 重构视觉、开发、验证和发布阶段 |
| A09 | `A09_cross_genre_evaluation.md` | 待执行 | 八类样例、重复性和反过拟合门禁 |
| A10 | `A10_migration_and_release.md` | 待执行 | 默认切换、迁移、回滚和发布证明 |

每个原子提交至少执行：`cargo fmt --all -- --check`、受影响 crate 测试、`cargo check --workspace --locked`、安全自检。触及 Web 时追加 Web 单元、i18n、设计内容和 UI 门禁。
