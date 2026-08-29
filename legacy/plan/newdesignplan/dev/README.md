# 原子开发计划 — 执行入口

执行规则：严格按依赖推进；每个任务独立提交；未满足本任务停止门不得启动依赖它的后续任务。强制标准见 `../01_development_standards.md`。

∥ 标记的任务是并行轨道，允许与其声明区间内的任务同时进行。

| 顺序 | 文件 | 状态 | 依赖 | 目标 |
|---|---|---|---|---|
| R1-C0 ∥ | `R1C0_content_charter.md` | 已完成（用户已签署 A+M+P1+V2） | 无 | 通道防守内容宪章：产品定义 + 验收场景草稿 + 包络取值 |
| A01 | （v1 计划，已完成） | 已完成 | 无 | GameSpec 类型核心与四个反过拟合样例 |
| A02 | `A02_validation_hashing_envelope.md` | 已完成（本地，待验收提交） | A01 | 确定性验证、规范哈希、ProductEnvelope/ExecutionBudget 机制 |
| R0 ∥ | `R0_technical_probe.md` | 已完成（本地，待验收提交） | A01（与 A02/A03 并行） | 最小规格 → Unity → EXE 技术探针 + 可重复 harness |
| A03 | `A03_capability_decision_graph.md` | 已完成（本地，待验收提交） | A02 | 能力决策图 + 第二层反过拟合门禁（此后每个编译器任务自带） |
| A04 | `A04_change_kernel_and_spec_store.md` | 已完成（本地，待验收提交） | A03、R0 | ChangeKernel 内核 + SpecStore + WorkspaceChangeSet 合同定义 |
| A05 | `A05_bounded_ai_completion.md` | 已完成（本地，待验收提交） | A04、R1-C0 | 有界 AI 补全、风险分类、确认策略（含 auto_accept）、审计 |
| A06 | `A06_design_workbench_adapter.md` | 已完成（本地，待验收提交） | A05 | D1–D4 双轨接入与兼容投影 |
| A07 | `A07_pipeline_steps_00_06.md` | 已完成（本地，待验收提交） | A06、R1-C0 | Step00–06 冻结规格编译链 + 通道防守冻结规格 fixture |
| A08a | `A08a_step07_art_direction.md` | 已完成（本地，待验收提交） | A07 | Step07 美术规范、风格锚点、代表资产、资产硬门禁 |
| A08b | `A08b_step08_10_architecture_tasks.md` | 已完成（本地，待验收提交） | A07 | Step08–10 架构编译、AssetManifest 冻结、可信任务合同 |
| A08c | `A08c_step11_execution_engine.md` | 已完成（v2 内核，本地，待验收提交；旧入口 legacy 保留） | A08b、A04、R0 | Step11 执行引擎：WorkspaceChangeSet 验证、并行调度/串行合入、修正队列 |
| A08d | `A08d_step12_asset_production.md` | 已完成（v2 内核，本地，待验收提交） | A08a、A08b | Step12 批量资产生产、导入、headless 绑定加载探针 |
| A08e | `A08e_step13_acceptance_validation.md` | 已完成（v2 内核，本地，待验收提交） | A08c、A08d | Step13 可执行验收场景、性能、回归 |
| A08f | `A08f_step14_packaging_r1_gate.md` | 已完成（v2 R1 停止门内核，本地，待验收提交） | A08e | Step14 打包 + smoke +【R1 停止门】 |
| A09 | `A09_cross_genre_evaluation.md` | 已完成（本地，待验收提交） | A08f | 八类规格级全覆盖 + 选 3 类全生产 + 包络权重校准 |
| A10 | `A10_migration_and_release.md` | 已完成（迁移/发布 readiness 内核，本地，待验收提交） | A09 | 迁移、默认切换、构建/发布分离、R2 发布 |

## 每个原子提交最低执行

```
cargo fmt --all -- --check
cargo check --workspace --locked
受影响 crate: cargo test --locked
安全自检
```

触及 Web 追加：Web 单元、i18n、设计内容和 UI 门禁。触及执行器/打包追加：独立性边界扫描、EXE smoke。

## 分工与验收流程（2026-07-15 确定）

- **开发**：Codex 独立完成全部原子任务（R1-C0 → A10），按本表顺序推进，自行判定各任务停止门后进入下一任务。开发期间自行维护本表状态和各任务文档的"状态 + 实际结果"。
- **总验收**：全部开发完成后，由 Claude 做一次性总验收。开发期间 Claude 不介入、不逐任务把关。
- 总验收内容：
  1. 实际运行全部门禁（fmt/check/test、安全自检、独立性扫描、EXE smoke），不采信文档声明；
  2. 逐任务核对停止门与"实际结果"是否与代码一致；
  3. 架构红线扫描（`01_development_standards.md` 第 3 节全部条目）；
  4. 反过拟合三层防线测试的存在性与真实性；
  5. R1 停止门实测：通道防守 EXE 实际运行 + 宪章验收场景核对。
- 验收结论写成总验收报告（通过项 / 问题清单及严重级别）；问题由 Codex 修复后复验。
- 验收通过后由 Claude 同步跨会话记忆。

**给开发方的提示**：正因为没有过程验收，`01_development_standards.md` 的红线（尤其"禁止占位实现冒充完成"、fail closed、受信测试防篡改）在总验收时会被逐条实测，任何一条不过都会进入修复循环。
