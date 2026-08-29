# 10 Step12 Prefab 与绑定预检

## 目标

从 UI flow、sprite metadata 和程序需求生成 UGUI Prefab 合同，并提前检查程序绑定完整性。

## 修改范围

```text
core/art_pipeline/services/ugui_prefab_planner.py
core/art_pipeline/services/binding_preflight.py
pipeline/step_12_art_production/plugin.py
core/tests/unit/test_art_pipeline_prefab_planner.py
core/tests/unit/test_art_pipeline_binding_preflight.py
```

## 输入

- Stage02 `ui_flow_contract`
- Stage02 `input/runtime/objective contracts`
- Stage03 `program_requirements_contract`
- `sprite_slice_result_manifest.json`
- `unity_import_settings_manifest.json`

## 输出

```text
outputs/artifacts/stage_12/ugui_prefab_contract.json
outputs/artifacts/stage_12/ui_prefab_generation_request.json
outputs/artifacts/stage_12/asset_mount_manifest.json
outputs/artifacts/stage_12/program_asset_binding_preflight.json
```

## 实现要点

- 每个 screen 有 Prefab。
- 每个 node 有稳定 path。
- Button 有 sprite、action binding、interaction state。
- Data binding 映射到 UI 节点或有明确豁免理由。
- Prefab 路径统一 `Assets/AutoDesign/Prefabs/UI`。

## 验收标准

- screen 缺 Prefab 时 blocking。
- Button 缺 action binding 时 blocking。
- 必需显示数据无 UI consumer 时 blocking 或 completed_with_review。
- 输出可由 Step13 Editor Script 执行的 generation request。

## 禁止事项

- 不生成空 Canvas 假完成。
- 不使用 runtime `Find()` 作为正式绑定方案。
- 不硬编码具体游戏 UI。

