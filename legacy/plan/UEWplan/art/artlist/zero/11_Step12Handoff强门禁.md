# 11 Step12 Handoff 强门禁

## 目标

生成 Step13 唯一强消费入口 `art_handoff_manifest.json`，阻止问题资产进入 Unity 场景装配。

## 修改范围

```text
core/art_pipeline/contracts/handoff_manifest.py
core/art_pipeline/orchestrator.py
pipeline/step_12_art_production/plugin.py
core/tests/unit/test_art_pipeline_handoff_gate.py
```

## 输入

- Step12 所有报告和 manifest。
- `art_rework_queue.json`
- `program_asset_binding_preflight.json`

## 输出

```text
outputs/artifacts/stage_12/art_handoff_manifest.json
outputs/artifacts/stage_12/art_production_report.json
```

## Handoff 必需字段

```text
schema_version
ready_for_step13
mount_items
blocking_issues
review_items
source_refs
```

每个 mount item：

```text
asset_id
unity_target_path
usage
required_by
source_refs
```

## 状态规则

- 有 P0/P1 blocking issue 时 `ready_for_step13=false`。
- 有 unresolved `requires_visual_review=true` 时 `completed_with_review`。
- 有 `path_convention_mismatch` 时 blocked。

## 验收标准

- Step13 缺 handoff 时 blocked。
- `ready_for_step13=false` 时 Step13 blocked。
- blockers 与 rework queue 可追踪对应。

## 禁止事项

- 不让 Step13 直接扫目录猜资源。
- 不让 completed_with_review 自动等于 succeeded。

