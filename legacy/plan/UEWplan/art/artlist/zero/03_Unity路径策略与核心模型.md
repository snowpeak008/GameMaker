# 03 Unity 路径策略与核心模型

## 目标

建立 `core/art_pipeline/` 的核心模型、状态聚合和 Unity 路径策略。后续所有阶段必须复用同一策略生成 `unity_target_path`、`unity_prefab_path` 和 Atlas path。

## 修改范围

```text
core/art_pipeline/__init__.py
core/art_pipeline/models.py
core/art_pipeline/result.py
core/art_pipeline/reports.py
core/art_pipeline/paths.py
core/tests/unit/test_art_pipeline_models.py
core/tests/unit/test_art_pipeline_unity_paths.py
```

## Unity 路径策略

自动生成内容统一根目录：

```text
Assets/AutoDesign/
```

标准子目录：

```text
Assets/AutoDesign/Art/Source
Assets/AutoDesign/Art/Processed
Assets/AutoDesign/Art/Atlas
Assets/AutoDesign/Prefabs/UI
Assets/AutoDesign/Runtime/Generated
Assets/AutoDesign/Audio/Placeholders
Assets/AutoDesign/Editor
```

兼容边界：

- `Assets/Scenes/DemoScene.unity` 是场景路径。
- `Assets/Scripts/` 是手写或历史脚本路径。
- `Assets/Editor/AutoDesignMaker/` 只作为历史 bootstrap 兼容路径。

## 核心模型

实现：

- `AssetLifecycleRecord`
- `AssetIssue`
- `ArtPipelineResult`
- `ArtifactWriteResult`
- `UnityMaterializationRequest`
- `AcceptanceCheckResult`

状态：

- `passed`
- `completed_with_review`
- `blocked`
- `environment_blocked`
- `failed`

## 验收标准

- P0/P1 blocking issue 能决定 Step12/13/14 状态。
- `environment_blocked` 不能被视为 passed。
- 旧路径 `Assets/Art/...`、`Assets/Prefabs/...` 被识别为 path convention mismatch。
- 路径生成测试覆盖图片、Atlas、Prefab、音频占位和 Editor Script。

## 禁止事项

- 不在各阶段手写路径拼接规则。
- 不在 `generation.py` 中实现路径策略。
- 不静默迁移旧路径。

