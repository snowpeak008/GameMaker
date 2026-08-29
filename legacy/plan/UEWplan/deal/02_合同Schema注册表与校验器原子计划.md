# 合同Schema注册表与校验器原子计划

## 目标

扩展现有 AI 设计合同与 playable contract 校验体系，而不是新建第二套合同系统。

该计划完成后，系统应该能基于现有 `core/ai_design` 与 `core/design/playable_contracts.py` 判断一个项目是否具备可开发、可验证、可追溯的 playable contracts。

## 依赖

无。本计划是后续 D2/D3/D4 和 Step00-14 强消费的基础。

## 必须复用的现有代码

```text
core/ai_design/contract_gate.py
core/ai_design/types.py
core/ai_design/traceability.py
core/design/playable_contracts.py
knowledge/schemas/playable_contracts/
pipeline/artifact_layer/registry.json
```

## 禁止新增的平行体系

不要新增：

```text
core/design/contracts/
core/design/contracts/registry.py
core/design/contracts/validator.py
```

原因是当前代码已经存在 `validate_contract()`、`GateResult`、`AiDesignIssue`、playable contract bundle 和 schema 文件。新增第二套会导致 D3、D4、Step02-14 读取不同合同标准。

## 正式合同名称

必须使用现有名称：

- `core_playable_contract`
- `demo_flow_contract`
- `runtime_data_contract`
- `ui_flow_contract`
- `scene_bootstrap_contract`
- `asset_mount_contract`
- `audio_requirements_contract`
- `playable_acceptance_contract`

`camera_view_spec` 不作为第一阶段独立合同，摄像机要求写入 `scene_bootstrap_contract.camera`。

## 执行步骤

1. 在 `core/ai_design/contract_gate.py` 中扩展注册表能力，但保留现有 `validate_contract()` API。
2. 在 `core/ai_design/types.py` 中复用 `AiDesignIssue` 与 `GateResult`，如字段不足，只做向后兼容扩展。
3. 在 `knowledge/schemas/playable_contracts/` 中扩展现有 schema，不换文件名。
4. 在 `core/design/playable_contracts.py` 中新增结构化决策入口：
   ```python
   def build_playable_contract_bundle_from_decisions(
       decisions: dict[str, Any],
       profile: dict[str, Any],
       archetype_requirements: dict[str, Any],
   ) -> dict[str, Any]:
       ...
   ```
   旧 `build_playable_contract_bundle(parsed)` 必须保留，用于 Markdown 兼容和既有测试。
5. 将 `runtime_data_contract.schema.json` 从简单 tables 扩展为通用 runtime 数据模型。
6. 将 `ui_flow_contract.schema.json` 扩展为屏幕图、HUD、数据绑定、输入入口和空状态。第一阶段新增字段必须为 optional，不修改现有 required 列表；旧 `build_playable_contract_bundle(parsed)` 无需为新字段补齐输出，避免破坏既有测试。待 `build_playable_contract_bundle_from_decisions()` 覆盖完整结构后，再评估是否将部分字段升级为 required。
7. 将 `scene_bootstrap_contract.schema.json` 扩展为入口场景、根对象、摄像机、Canvas、EventSystem、输入根、目标追踪。
8. 将 `asset_mount_contract.schema.json` 扩展为 Unity 路径、资源用途、挂载点、fallback。
9. 将 `audio_requirements_contract.schema.json` 明确为 placeholder-first 音频需求。
10. 保持 `pipeline/artifact_layer/registry.json` 中 stage artifact 路径与 schema_refs 一致。

## build_playable_contract_bundle_from_decisions 要求

新函数的输入不是 Markdown parsed selections，而是 D4 导出的结构化数据。

输入：

```text
decisions.json
profile.json
archetype_requirements.json
```

输出：

```text
core_playable_contract
demo_flow_contract
runtime_data_contract
ui_flow_contract
scene_bootstrap_contract
asset_mount_contract
audio_requirements_contract
playable_acceptance_contract
```

要求：

- 必须保留 selected option、primary option 和 optionProvenance 的 traceability。
- 不允许把空 decisions 转换成全 fallback success。
- fallback 字段必须标记 `source: fallback`，并在 completeness report 中产生 review item。
- D4 必须调用该新函数，旧函数只作为兼容入口。

## runtime_data_contract 字段结构

`runtime_data_contract` 顶层字段建议：

```json
{
  "schema_version": "2.1",
  "tables": [
    {
      "table_id": "objectives",
      "purpose": "Runtime objective data.",
      "consumer_systems": ["ObjectiveTracker"],
      "records": []
    }
  ],
  "entities": [
    {
      "entity_id": "objective",
      "entity_type": "runtime_entity",
      "source_table": "objectives",
      "fields": [
        {"field": "objective_id", "type": "string", "required": true},
        {"field": "progress", "type": "integer", "default": 0}
      ]
    }
  ],
  "relations": [
    {
      "relation_id": "objective_updates_ui",
      "from": "objective.progress",
      "to": "ui_flow_contract.data_bindings.status_panel.progress",
      "relation_type": "data_binding"
    }
  ],
  "state_models": [
    {
      "model_id": "demo_state",
      "owner_system": "RuntimeBootstrap",
      "fields": [
        {"field": "progress", "type": "integer", "default": 0},
        {"field": "completed", "type": "boolean", "default": false}
      ],
      "initial_values": {"progress": 0, "completed": false}
    }
  ],
  "consumer_systems": ["RuntimeBootstrap", "ObjectiveTracker", "UIController"]
}
```

说明：

- `tables` 描述配置数据。
- `entities` 描述运行时对象或配置对象。
- `state_models` 描述运行时状态，不替代 `core_playable_contract.state_model`，而是给 Step03 生成代码模型使用。
- `relations` 描述数据、状态、UI、目标之间的引用关系。

## scene_bootstrap_contract 字段结构

`scene_bootstrap_contract.schema.json` 必须显式定义 required 中已有字段的结构，不能只写在 required 里。

建议字段：

```json
{
  "camera": {
    "role": "MainCamera",
    "projection": "Orthographic",
    "position": [0, 0, -10],
    "rotation": [0, 0, 0],
    "orthographic_size": 5.5,
    "field_of_view": 60,
    "clear_flags": "SolidColor",
    "background_color": "#111318",
    "required": true
  },
  "event_system": {
    "name": "EventSystem",
    "required": true
  },
  "input_roots": [
    {
      "name": "InputRoot",
      "actions": ["action_01"],
      "required": true
    }
  ],
  "objective_tracker": {
    "name": "ObjectiveTracker",
    "state_model": "demo_state",
    "required": true
  },
  "ui_roots": [
    {
      "name": "UIRoot",
      "screen": "game_hud",
      "canvas_mode": "ScreenSpaceOverlay",
      "required": true
    }
  ]
}
```

最低要求：

- `camera.projection`
- `camera.position`
- `runtime_roots`
- `ui_roots`
- `input_roots`
- `event_system`
- `objective_tracker`

## 错误输出要求

所有校验错误必须包含：

- `code`
- `message`
- `severity`
- `path`
- `source_refs`
- `required_by_steps`

如果现有 `AiDesignIssue` 不含 `required_by_steps`，可以先放入 `source_refs` 或向后兼容扩展，不能破坏旧测试。

## 完成标准

1. 不存在第二套 `core/design/contracts/` validator。
2. 现有 `test_ai_design_contracts.py` 仍然通过。
3. 现有 `test_playable_contracts.py` 仍然通过或按新 schema 明确更新。
4. 所有 playable contract schema 仍位于 `knowledge/schemas/playable_contracts/`。
5. `build_playable_contract_bundle_from_decisions()` 被明确为 D4 的结构化入口。
6. `runtime_data_contract` 有明确通用 schema，不再只是模糊引用。
7. `scene_bootstrap_contract` 中 camera、input、event_system、objective_tracker 字段结构明确。
8. schema registry 能回答每个合同被哪些 Step 消费。

## 不做事项

- 不改 D2/D3/D4 执行逻辑。
- 不生成 Unity 场景。
- 不删除现有 playable contract 文件名。
