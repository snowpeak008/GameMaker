# Step13Unity场景装配原子计划

## 目标

让 Step13 成为实际 Unity 场景装配阶段，确保生成的项目 Play 后至少有可见场景、可见 UI、输入入口和目标追踪。

Step13 不能依赖虚构的 Step08 场景设计文件。它必须消费现有流水线中的结构化任务、开发执行结果和资源生产结果。

## 依赖

- `12_Step08程序计划与场景装配前置任务原子计划.md`
- `20_Step10到Step12执行对齐与生产链路原子计划.md`

## 涉及范围

重点修改：

```text
pipeline/step_13_scene_assembly/plugin.py
core/engines/generation.py
```

可能涉及 Unity 生成模板和脚本输出目录，实际路径以项目为准。

建议新增测试：

```text
core/tests/unit/test_step13_requires_scene_and_ui_contracts.py
core/tests/unit/test_step13_scene_assembly_outputs.py
```

## 输入

Step13 必须读取：

- Stage02 `scene_bootstrap_contract.json`
- Stage02 `ui_flow_contract.json`
- Stage02 `runtime_data_contract.json`
- Stage02 `asset_mount_contract.json`
- Step08 `scene_assembly_task_requirements.json`
- Step08 `ui_runtime_task_requirements.json`
- Step08 `input_runtime_task_requirements.json`
- Step08 `objective_runtime_task_requirements.json`
- Step10 asset alignment report。
- Step11 dev execution report。
- Step12 art production outputs。
- Step12 audio placeholder runtime manifest。

## 必须生成或验证

- Entry Scene。
- RuntimeBootstrap。
- CameraRig。
- EventSystem。
- Canvas/UIRoot。
- 初始 HUD。
- InputRouter。
- ObjectiveTracker。
- GameState。
- 初始可见对象。
- 资源挂载。
- audio placeholder marker 或声明。

## 执行步骤

1. 加载 Stage02 playable contracts。
2. 加载 Step08 场景装配前置任务。
3. 加载 Step10 资产对齐结果。
4. 加载 Step11 开发执行结果，确认 playable 相关 EO 已 verified。
5. 加载 Step12 资源生产结果。
6. 校验 Entry Scene。
7. 生成或更新 Unity 场景文件。
8. 生成 RuntimeBootstrap 脚本或 prefab。
9. 生成 CameraRig。
10. 生成 EventSystem。
11. 生成 Canvas/UIRoot 与初始 UI。
12. 生成 InputRouter。
13. 生成 ObjectiveTracker。
14. 挂载必要资源或占位资源。
15. 输出 scene assembly report。

## 失败条件

以下情况 Step13 必须失败：

- 没有 `scene_bootstrap_contract`。
- 没有 `ui_flow_contract`。
- 没有 Entry Scene。
- 没有 active camera。
- 没有 Canvas/UIRoot。
- 没有初始 UI。
- 没有输入入口。
- 没有目标追踪。
- playable 相关 EO 未 verified。
- 关键资源没有挂载且无 fallback。

## 完成标准

1. Unity 项目中存在入口场景。
2. 场景中存在 active camera。
3. 场景中存在 Canvas/UIRoot。
4. 初始 UI 元素存在。
5. 输入入口存在。
6. 目标追踪对象存在。
7. 资源挂载或 fallback 明确。
8. 输出可供 Step14 验证的报告。

## 不做事项

- 不把某个游戏类型的玩法写死。
- 不用一个固定 Demo 场景替代合同驱动装配。
- 不跳过缺失合同继续 success。
