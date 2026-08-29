# 02 Schema Registry Dependency Graph

## 目标

建立美术资产管线所需 schema、artifact registry 和 dependency graph 基础，同时区分“新增 schema”和“确认既有 schema”。

## 修改范围

```text
knowledge/schemas/ai_design/art_pipeline/
knowledge/schemas/ai_design/art_production_task_contract.schema.json
pipeline/artifact_layer/registry.json
pipeline/artifact_layer/dependency_graph.json
core/tests/unit/test_artifact_registry_playable_chain.py
```

## 既有 schema 保护

`knowledge/schemas/ai_design/art_production_task_contract.schema.json` 已存在，Stage09 registry 已注册：

```text
outputs/artifacts/stage_09/art_production_task_contract.json
```

本计划只确认该 schema 和 registry 存在，不新建同名 schema，不移动路径，不覆盖既有文件。

既有 required 字段必须保留：

```text
task_id
asset_id
unity_target_path
dimensions
consumer_system
mount_point
acceptance
```

## 新增 schema

新增 schema 放入：

```text
knowledge/schemas/ai_design/art_pipeline/
```

至少包含：

- `image_consumable_spec.schema.json`
- `ui_slice_spec_contract.schema.json`
- `unity_import_policy.schema.json`
- `asset_usage_binding_seed.schema.json`
- `audio_placeholder_requirements.schema.json`
- `raw_generated_asset_manifest.schema.json`
- `image_quality_report.schema.json`
- `art_semantic_review_report.schema.json`
- `art_rework_queue.schema.json`
- `processed_asset_manifest.schema.json`
- `sprite_slice_result_manifest.schema.json`
- `unity_import_settings_manifest.schema.json`
- `sprite_atlas_plan.schema.json`
- `ugui_prefab_contract.schema.json`
- `ui_prefab_generation_request.schema.json`
- `asset_mount_manifest.schema.json`
- `program_asset_binding_preflight.schema.json`
- `art_handoff_manifest.schema.json`
- `unity_editor_request.schema.json`
- `unity_art_import_report.schema.json`
- `unity_prefab_generation_report.schema.json`
- `program_asset_binding_contract.schema.json`
- `unity_scene_mount_report.schema.json`
- `art_acceptance_report.schema.json`
- `playable_acceptance_report.schema.json`

## 既有 schema 复用

以下 schema 已由现有计划或代码库管理，本计划只确认其存在和 registry 引用，不新建、不移动、不覆盖：

| artifact | schema | 管理来源 |
|---|---|---|
| `outputs/artifacts/stage_09/art_production_task_contract.json` | `knowledge/schemas/ai_design/art_production_task_contract.schema.json` | 既有 Stage09 schema，Phase 05 只做向后兼容扩展 |
| `outputs/artifacts/stage_12/art_production_report.json` | `knowledge/schemas/ai_design/art_production_report.schema.json` | deal/Plan 20 已定义并注册 |
| `outputs/artifacts/stage_02/playable_contracts/playable_acceptance_contract.json` | `knowledge/schemas/playable_contracts/playable_acceptance_contract.schema.json` | playable contracts 既有 schema |

说明：

- `program_asset_binding_contract.schema.json` 当前代码库未见实际 schema 文件，虽然早期 `art/06` 计划提到过；本轮实现必须创建 schema。
- `playable_acceptance_report.schema.json` 当前代码库未见实际 schema 文件；若 Step14 输出 JSON report，则本轮实现必须创建 schema。若最终保留 markdown 报告，则必须同步修改 artifact 清单和 registry，不得让 JSON 输出无 schema。

## Dependency Graph

确认已有边：

- Stage04 -> Stage09
- Stage07 -> Stage09
- Stage12 -> Stage13
- Stage13 -> Stage14

新增边：

- Stage04 -> Stage12：Step12 直接消费 `image_consumable_spec`、`ui_slice_spec_contract`、`unity_import_policy`。
- Stage09 -> Stage12：Step12 直接消费 `art_production_task_contract`。

## 验收标准

- registry 中引用的 schema 文件都存在。
- schema 子目录引用通过 artifact validator 显式路径检查。
- `art_production_task_contract.schema.json` 没有被覆盖。
- dependency graph 不重复添加已有边。
- 测试覆盖子目录 schema refs。

## 禁止事项

- 不把 art pipeline schema 散落到多个目录。
- 不删除既有 Stage09 required 字段。
- 不把 Addressables schema 作为默认阻断依赖。
