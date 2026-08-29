# A08f Step14 打包与 R1 停止门

状态：已完成（v2 R1 停止门内核，本地，待验收提交）
依赖：A08e

## 目标

可重现打包、完整性与 smoke 验证，并判定 R1 停止门。

## 变更

1. 复用现有独立性门禁、便携构建与 EXE smoke 基建（`tools/verify-standalone.ps1` 一系）。
2. 发布清单：构建哈希、规格哈希、全部门禁证据、AI 使用证据汇总。
3. 构建/发布分离：本地测试包自动构建（unattended）；**对外正式发布签署永远人工**。
4. R1 停止门判定自动化：读取 A08e 场景结果 + 宪章清单，产出停止门报告。

## 验收（R1 停止门）

**通道防守垂直切片 EXE 满足全部条件——不是"能运行"即可：**

1. R1-C0 宪章定义的验收场景全部通过（A08e 证据）。
2. 玩法闭环成立、内容完整（达到宪章完成度档位）。
3. 可重现构建 + 完整性 + EXE smoke + 跨电脑独立性通过。
4. AI 补全在产物中有明确使用证据；关闭 AI 流程可人工完成。
5. 核心源码无品牌/类型专属分支（三层反过拟合防线通过）。
6. 用户实际游玩并签署。

停止门未过不进入 A09，也不通过扩大 AI 自由度补偿。

## 回滚

打包模块独立；删除不影响流水线其余部分。

## 实际结果

- 新增 `adm-new-pipeline::stages::step14_v2`，读取 A08e `Step13AcceptanceOutput` 与 R1 门禁证据，生成 `r1_release_manifest.json`、`r1_stop_gate_report.json`、`package_integrity_report.json`、`exe_smoke_report.json`、`ai_usage_evidence_summary.json` 和 `step14_r1_packaging_output.json`。
- R1 停止门 fail closed：场景失败、内容未完成、不可重现构建、完整性/独立性失败、EXE smoke 失败、AI 使用证据缺失、AI-off 人工流程缺失、反过拟合门禁失败、用户游玩签署缺失均阻断进入 A09。
- 对外正式发布签署保持人工要求，发布清单固定记录 `manual_required_for_external_release`。
- 已补充 `step14_v2` 集成测试，覆盖全部证据通过、缺少用户签署阻断、A08e 场景失败阻断。

## 已验证

- `cargo fmt --all`
- `cargo test -p adm-new-pipeline --test step14_v2`
- `cargo test -p adm-new-pipeline`

## R1 完成后动作

- 用 R1 真实数据校准 ProductEnvelope 加权复杂度预算（供 A09）。
- 按 R1 暴露的能力缺口选定 A09 的 3 类全生产品类。
- 同步跨会话记忆。
