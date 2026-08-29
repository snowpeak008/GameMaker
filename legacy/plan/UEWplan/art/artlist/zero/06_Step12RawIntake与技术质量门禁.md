# 06 Step12 Raw Intake 与技术质量门禁

## 目标

登记 AI 生成图片或占位资源，并执行不可跳过的技术校验。技术不合格的 P0/P1 资产不能进入 Step13。

## 修改范围

```text
core/art_pipeline/services/image_probe.py
core/art_pipeline/services/consumable_validator.py
pipeline/step_12_art_production/plugin.py
core/tests/unit/test_art_pipeline_consumable_validator.py
core/tests/unit/test_step12_image_quality_gate.py
```

## 输入

- Stage09 `art_production_task_contract.json`
- Stage04 `image_consumable_spec.json`
- AI 生成图片文件
- 音频占位需求

## 输出

```text
outputs/artifacts/stage_12/raw_generated_asset_manifest.json
outputs/artifacts/stage_12/image_quality_report.json
```

## 必检项

- 文件存在。
- 文件可读取。
- 格式白名单。
- 实际尺寸。
- alpha 通道。
- 透明背景要求。
- 文件大小。
- power-of-two 要求。
- 命名规则。
- 禁止 marker。

## Pillow 边界

Pillow 只作为可选图像探测依赖。缺少 Pillow 时：

- 可执行的静态检查仍需执行。
- 需要真实像素读取的检查输出 `environment_blocked` 或对应 review item。
- 不能假成功。

## 验收标准

- P0/P1 文件缺失时 blocked。
- P0/P1 尺寸、格式、alpha、命名失败时不能 `succeeded`。
- 报告包含 passed/failed/blocking/environment_blocked 计数。

## 禁止事项

- 不在 Step12 Raw Intake 中修图。
- 不把概念图当作可消费图放行。
- 不因为缺少 Pillow 而跳过质量门禁。

