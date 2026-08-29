# Unity 路径、语义审查与 Editor 启动补充计划

## 目标

本文件把第二轮审查中发现的 3 个问题拆成实现时必须遵守的补充计划。它是 `01_标准化美术资产管线开发计划.md` 的执行补充，不替代主计划。

## 1. Unity 路径统一

### 决策

自动生成内容统一使用：

```text
Assets/AutoDesign/
```

### 标准目录

```text
Assets/AutoDesign/Art/Source
Assets/AutoDesign/Art/Processed
Assets/AutoDesign/Art/Atlas
Assets/AutoDesign/Prefabs/UI
Assets/AutoDesign/Runtime/Generated
Assets/AutoDesign/Audio/Placeholders
Assets/AutoDesign/Editor
```

### 兼容说明

- `Assets/Scenes/DemoScene.unity` 保留为场景文件路径。
- `Assets/Scripts/` 保留为手写或历史脚本路径。
- `Assets/Editor/AutoDesignMaker/` 可作为历史 bootstrap 路径。
- 新增自动生成资源、Prefab、runtime 脚本、音频占位、Editor Script 必须进入 `Assets/AutoDesign/`。
- 旧计划中的 `Assets/Art/...`、`Assets/Prefabs/...` 示例不再作为实现路径。

### 开发任务

- 在 `core/art_pipeline/paths.py` 中定义 Unity 路径策略。
- Step04 生成 `unity_target_path` 时从该策略读取。
- Step08/Step12 生成 Atlas、Prefab、mount item 时复用同一策略。
- Step13 写入 Unity 项目前检查路径是否符合策略。
- Step14 验证实际文件路径与 handoff 中路径一致。

### 验收

- 单元测试覆盖 UI 图片、Atlas、Prefab、音频占位、Editor Script 的路径生成。
- 任一 artifact 输出 `Assets/Art/...` 或 `Assets/Prefabs/...` 时触发 path convention warning 或 blocking。

## 2. 语义审查分层

### 决策

语义审查不能假装已经具备视觉识别能力。当前必须分为 deterministic 检查和视觉审查两层。

### 第 1 层：Deterministic 检查

必需实现，无外部依赖。

检查来源：

- 文件名。
- 文件路径。
- metadata。
- `asset_kind`。
- art task prompt / negative prompt。
- non-consumable markers。

阻断 marker：

```text
_concept
_reference
_moodboard
_draft
_wip
copyright
watermark
```

P0/P1 必需资产命中上述 marker 时，Step12 必须 blocking 或 completed_with_review，不能 succeeded。

### 第 2 层：视觉内容审查

需要 Vision AI 或人工审查。

检查项：

- 复杂背景。
- 水印。
- 不可编辑文字。
- UI 状态图缺失或状态含义不清。
- 图标主体不清。
- 按钮/面板不适合九宫格。

当前未接入 Vision AI 时：

- 不能用不可靠启发式自动判定通过。
- 不能把视觉检查写成 passed。
- 应写入 `needs_human_review` 或 `vision_review_unavailable`。
- `semantic_policy.requires_visual_review=true` 的资产进入 `completed_with_review` 并由 Step13 handoff 门禁处理。

### 验收

- deterministic marker 测试必须通过。
- Vision AI 未配置时，视觉检查报告状态必须是 `needs_human_review` / `vision_review_unavailable`，不能是 passed。
- rework queue 必须包含失败原因、建议 prompt 修正和来源 asset_id。

## 3. Unity Editor Script 启动序列

### 问题

首次把 `AutoDesignAssetImporter.cs` 写入 Unity 项目后，Unity 需要先编译该脚本。若直接执行以下错误示例：

```powershell
Unity.exe -batchmode -executeMethod AutoDesignAssetImporter.Run
```

入口方法可能尚不存在，导致失败。

### 标准序列

Step13 必须使用两段式启动：

```powershell
Unity.exe -batchmode -projectPath <generated_unity_project> -quit -logFile compile.log
Unity.exe -batchmode -projectPath <generated_unity_project> -executeMethod AutoDesignAssetImporter.Run
Unity.exe -batchmode -projectPath <generated_unity_project> -executeMethod AutoDesignPrefabBuilder.Run
Unity.exe -batchmode -projectPath <generated_unity_project> -executeMethod AutoDesignSceneBinder.Run
```

### 可选 bootstrap

如果项目已有可编译的 bootstrap，例如 `Assets/Editor/AutoDesignMaker/PlayableSceneBootstrapBuilder.cs`，Step13 可以复用该模式，但必须在 `unity_editor_request.json` 中记录：

```json
{
  "uses_precompiled_bootstrap": true,
  "bootstrap_script_path": "Assets/Editor/AutoDesignMaker/PlayableSceneBootstrapBuilder.cs"
}
```

### `unity_editor_request.json` 必需字段

```json
{
  "schema_version": "1.0",
  "editor_scripts_written": [],
  "requires_compile_pass": true,
  "compile_command": "",
  "compile_log_path": "",
  "execute_methods": [],
  "request_files": [],
  "expected_reports": []
}
```

### 验收

- Step13 生成 Editor Script 后必须先 compile pass，再 execute pass。
- compile pass 失败时输出 `environment_blocked` 或 `failed`。
- executeMethod 找不到入口时不能标记 Unity 资产物化成功。

## 4. 图像处理执行器职责

### 决策

Python 不执行实际像素切割。Unity Editor 是正式 Sprite 切割和导入执行器。

### Python Phase 07 职责

- 复制或重命名源图片到 processed target。
- 生成 `processed_asset_manifest.json`。
- 生成 `sprite_slice_result_manifest.json`。
- 为每个 sprite 生成稳定 `sprite_id`。
- 记录 `rect`、`border`、`pivot`、`pixels_per_unit`、`fallback_sprite_id`、`source_refs`。

禁止：

- 不写实际像素切割代码。
- 不 resize、crop、trim、pad 或重新编码图片。
- 不把 Pillow 作为 Phase 07 必需依赖。

### Unity Phase 11 职责

- 读取 Phase 07 切片元数据。
- 写入 `TextureImporter` 设置。
- 写入 Sprite rect、border、pivot。
- 执行 reimport。
- 创建 Atlas 和 Prefab 引用。

### Pillow 使用边界

Pillow 只允许作为 Phase 05 的可选图像探测依赖，用于格式、尺寸、alpha 检查。若缺少 Pillow，Phase 05 应输出 `environment_blocked` 或降级到可证明的静态检查，不能假成功。
