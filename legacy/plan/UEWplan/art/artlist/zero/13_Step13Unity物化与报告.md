# 13 Step13 Unity 物化与报告

## 目标

把 Step12 handoff 真实写入 Unity 项目，完成 Importer、Sprite、Atlas、Prefab、程序绑定和场景挂载。

## 修改范围

```text
core/art_pipeline/unity/unity_project_writer.py
core/art_pipeline/unity/unity_report_reader.py
core/art_pipeline/unity/generated_scripts/AutoDesignAssetImporter.cs.template
core/art_pipeline/unity/generated_scripts/AutoDesignPrefabBuilder.cs.template
core/art_pipeline/unity/generated_scripts/AutoDesignSceneBinder.cs.template
pipeline/step_13_scene_assembly/plugin.py
core/tests/unit/test_step13_art_handoff_required.py
```

## 输入

- `unity_editor_request.json`
- `art_handoff_manifest.json`
- `sprite_slice_result_manifest.json`
- `ugui_prefab_contract.json`
- `program_asset_binding_preflight.json`

## 输出

```text
outputs/artifacts/stage_13/unity_art_import_report.json
outputs/artifacts/stage_13/unity_prefab_generation_report.json
outputs/artifacts/stage_13/program_asset_binding_contract.json
outputs/artifacts/stage_13/unity_scene_mount_report.json
```

## 执行动作

- 写入 `Assets/AutoDesign/` 标准目录。
- 应用 `TextureImporter`。
- 应用 Sprite rect/border/pivot。
- reimport。
- 创建 SpriteAtlas。
- 创建 UGUI Prefab。
- 注入 serialized field。
- 保存 Scene。

## 验收标准

- 缺 handoff 时 blocked。
- handoff not ready 时 blocked。
- 旧路径出现时 path convention mismatch。
- Unity 不可用时 environment_blocked，静态报告保留。
- 报告中缺关键 Prefab/Sprite/SerializedField 时 failed。

## 禁止事项

- 不只生成 JSON 后宣称挂载完成。
- 不静默跳过 Unity Editor 失败。
- 不使用 runtime `Find()` 作为正式绑定路径。

