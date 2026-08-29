# A09 跨结构评估

状态：已完成（本地，待验收提交）
依赖：A08f（R1 停止门通过）

## 目标

分层证明系统通用性、质量稳定性和反过拟合能力——规格级全覆盖，少量全生产。

## 变更

1. **Tier 1（已随 R1 完成）**：通道防守全生产；三消/分支叙事/回合战术规格级编译（D1–Step10）。
2. **Tier 2**：样例扩展到八类（+动作 Roguelite、卡组构筑、经营建造、网络协作），全部规格级编译；选 **3 类**全生产——**具体品类此刻按 R1 暴露的能力缺口决定，能力距离最远优先**（计划期不预选）。
3. **Tier 3**：其余品类关键步骤抽查；网络协作只做规格 + 架构验证，不做完整生产。
4. 第三层反过拟合：标签置换、字段 mutation、重复 AI N=20、无 AI 模式、故障注入、核心源码品牌与类型分支扫描。
5. 用 R1 + Tier 2 数据把 ProductEnvelope 从序数档位校准为加权复杂度预算。
6. 核对全部核心新增字段满足三样例晋升规则。

## 验收（停止门）

- 八类样例规格级门禁（schema、引用、不变量、场景、追踪、重复性）全部通过。
- 选定 3 类全生产达到 R1 同等停止门标准。
- 无样例专属核心泄漏；晋升规则核对清单交付。
- 加权预算模型以真实数据为依据并文档化。

## 回滚

评估样例与测试独立于产品路径，可整体删除。

## 实际结果

- 新增 `adm-new-pipeline::cross_genre_evaluation`，交付可复跑 A09 harness：八类规格级矩阵、R1 参考全生产、三类 R2 全生产样例、第三层反过拟合、源码扫描、字段晋升核对与包络权重校准报告。
- 八类规格级样例覆盖：通道防守、三消、分支叙事、回合战术、动作 Roguelite、卡组构筑、经营建造、网络协作；网络协作只跑规格 + 架构验证，不进入全生产。
- 三类全生产按能力距离选择：三消、分支叙事、回合战术；R1-C0 作为参考全生产数据参与包络校准。
- A09 报告输出：`a09_cross_genre_evaluation_report.json`、`a09_spec_level_matrix.json`、`a09_full_production_matrix.json`、`a09_source_scan_report.json`、`a09_envelope_calibration_report.json`、`a09_field_promotion_checklist.json`。
- 修正真实泛化问题：移除 Step07 的 R1 名称条件调色板；将 Step08-10 固定资产 ID `micro_ecodome_style_keyframe` 改为通用 `style_reference_keyframe`。
- 升级早期 GameSpec fixture：为三消、分支叙事、回合战术、通道防守补齐失败场景；为三消补齐状态机，使其满足 Step00-06 冻结门禁。

## 已验证

- `cargo fmt --all`
- `cargo test -p adm-new-pipeline --test a09_cross_genre_evaluation`
- `cargo test -p adm-new-game-spec`
- `cargo test -p adm-new-pipeline`
