# A06 设计工作台双轨接入

状态：已完成（本地，待验收提交）
依赖：A05

## 目标

D1–D4 外壳保留、内部重解释为 GameSpec 驱动，双轨运行不破坏旧项目。

## 变更

1. D1→`ProjectIntent`、D2→`CapabilityProfile`+决策图、D3→玩法与内容规格合成、D4→验收场景+追踪+冻结（详见总计划 3.1）。
2. `game_spec_v2` 项目级持久开关（默认关闭）；开启后影子生成 GameSpec，不接管 UI 与保存。
3. 从现有 `ProjectState` 生成只读 GameSpec 候选与差异报告，不反向覆盖。
4. D4 冻结 R1 默认人工（策略字段可配置，为 R2 预批准自动冻结留位）。
5. 编译器部分执行标签置换 + 能力扰动测试（第二层门禁）。

## 验收（停止门）

- 旧项目在开关关闭时零行为变化（现有测试与 UI 门禁全通过）。
- 开关开启后 D1–D4 双轨输出稳定：同一设计状态重复投影语义哈希一致。
- 差异报告作为产物落盘。

## 回滚

关闭开关即回到旧行为；删除投影模块不影响旧路径。

## 实际结果

- 新增 `adm-new-design::game_spec_projection`，可从现有 `ProjectState` 生成只读 GameSpec 候选、语义哈希、规范哈希和差异报告。
- D4 增加 `game_spec_v2` 显式开关；默认关闭时不暴露 `gameSpecV2Shadow`，开启后写出 `game_spec_v2_shadow/game_spec.json`、`diff_report.json`、`projection_report.json`。
- D4 冻结策略作为 `d4FreezePolicy` 输出，默认 `attended`，可由 stage metadata 或 settings 覆盖。
- 投影路径不反写 `ProjectState`，旧 concept package / structured handoff 行为保持原样。

## 验证

- `cargo fmt --all`
- `cargo test -p adm-new-design game_spec_projection`
- `cargo test -p adm-new-pipeline design_flow`
