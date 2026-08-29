# 09 Step12 导入 Atlas 和可选 Addressables

## 目标

为 processed assets 生成 Unity Importer、SpriteAtlas 和条件 Addressables 计划。

## 修改范围

```text
core/art_pipeline/services/unity_import_planner.py
core/art_pipeline/services/sprite_atlas_planner.py
pipeline/step_12_art_production/plugin.py
core/tests/unit/test_art_pipeline_import_planner.py
core/tests/unit/test_art_pipeline_atlas_planner.py
```

## 输入

- `processed_asset_manifest.json`
- `sprite_slice_result_manifest.json`
- Stage04 `unity_import_policy.json`

## 输出

```text
outputs/artifacts/stage_12/unity_import_settings_manifest.json
outputs/artifacts/stage_12/sprite_atlas_plan.json
outputs/artifacts/stage_12/addressable_asset_plan.json  # 条件输出
```

## Addressables 规则

- 默认不启用。
- 仅当 Unity 项目启用 Addressables 包或配置 `addressables_enabled=true` 时输出并参与门禁。
- 未启用时可不生成，或生成 `status=not_applicable` 非阻断报告。
- 启用时必须生成稳定 key，并验证 handle 生命周期要求。

## 验收标准

- 每个 processed asset 有 TextureImporter 配置。
- 每个 sprite 有 PPU、pivot、sprite mode、compression、platform settings。
- 大图不进 Atlas 时记录原因。
- 未启用 Addressables 时不阻断 Step13。

## 禁止事项

- 不把 Addressables 作为默认要求。
- 不手写 `.meta` 作为最终执行路径；最终由 Step13 Unity Editor 应用。
- 不让 import settings 缺失资产进入 handoff。

