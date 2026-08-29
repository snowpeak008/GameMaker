# UGUI Prefab 生成规范（Step12-Art-C）

> 路径说明：本文是早期拆分计划。最终实现路径以 `artlist/01_标准化美术资产管线开发计划.md` 为准，自动生成 Prefab 统一写入 `Assets/AutoDesign/Prefabs/UI/`。本文中旧的 `Assets/Prefabs/UI/...` 示例不得作为实现路径。

## 目标

从 `ui_flow_contract.json`（screens 结构）+ `ui_slice_spec_contract.json`（切片规格）生成 UGUI Prefab 层级规范，让 Step13 能直接按规范组装 Canvas 下的 UI 对象树，而不是靠硬编码或手动创建。

## 前置依赖

- `ui_flow_contract.json`（screens, controls, data_bindings, input_entry_points）
- `ui_slice_spec_contract.json`（每个资产的切片规格）
- `unity_import_settings_contract.json`（每个资产的 Unity 路径）

## 新增合同：`ugui_prefab_spec_contract.json`

### 位置
```
outputs/artifacts/stage_12/ugui_prefab_spec_contract.json
```

### Schema（新增 `knowledge/schemas/ai_design/ugui_prefab_spec_contract.schema.json`）

```json
{
  "required": ["schema_version", "source_refs", "canvas_root", "screens"],
  "properties": {
    "schema_version": { "type": "string" },
    "source_refs": { "type": "array", "items": { "type": "string" } },
    "canvas_root": {
      "type": "object",
      "required": ["name", "canvas_mode", "sort_order", "render_camera"],
      "properties": {
        "name":          { "type": "string", "default": "UIRoot" },
        "canvas_mode":   { "type": "string", "enum": ["ScreenSpaceOverlay", "ScreenSpaceCamera", "WorldSpace"] },
        "sort_order":    { "type": "integer" },
        "render_camera": { "type": "string", "description": "Camera 对象路径（ScreenSpaceCamera 时用）" },
        "event_system":  { "type": "string", "description": "EventSystem 对象路径" }
      }
    },
    "screens": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["screen_id", "prefab_name", "unity_prefab_path", "visible_on_start", "root_object"],
        "properties": {
          "screen_id":          { "type": "string" },
          "prefab_name":        { "type": "string" },
          "unity_prefab_path":  { "type": "string", "description": "如 Assets/AutoDesign/Prefabs/UI/Screen_GameHUD.prefab" },
          "visible_on_start":   { "type": "boolean" },
          "root_object": {
            "type": "object",
            "description": "Prefab 根节点描述",
            "required": ["name", "rect_transform", "children"],
            "properties": {
              "name":           { "type": "string" },
              "rect_transform": { "$ref": "#/$defs/rect_transform" },
              "children": {
                "type": "array",
                "items": { "$ref": "#/$defs/ui_node" }
              }
            }
          }
        }
      }
    },
    "$defs": {
      "rect_transform": {
        "type": "object",
        "properties": {
          "anchor_min":    { "type": "array", "items": { "type": "number" } },
          "anchor_max":    { "type": "array", "items": { "type": "number" } },
          "pivot":         { "type": "array", "items": { "type": "number" } },
          "offset_min":    { "type": "array", "items": { "type": "number" } },
          "offset_max":    { "type": "array", "items": { "type": "number" } }
        }
      },
      "ui_node": {
        "type": "object",
        "required": ["name", "component_type"],
        "properties": {
          "name":           { "type": "string" },
          "component_type": {
            "type": "string",
            "enum": [
              "Panel",
              "Image",
              "Button",
              "Text",
              "TextMeshPro",
              "Slider",
              "ScrollView",
              "InputField",
              "Toggle",
              "LayoutGroup",
              "Empty"
            ]
          },
          "sprite_asset_id":  { "type": "string", "description": "对应 asset_registry 中的 asset_id" },
          "image_type":       { "type": "string", "enum": ["Simple", "Sliced", "Tiled", "Filled"] },
          "raycast_target":   { "type": "boolean" },
          "interactable":     { "type": "boolean" },
          "text_content":     { "type": "string", "description": "占位文字，实际运行时由本地化表替换" },
          "font_size":        { "type": "integer" },
          "data_binding_key": { "type": "string", "description": "与 ui_flow_contract.data_bindings 中的 key 对应" },
          "action_binding":   { "type": "string", "description": "Button/Toggle 触发的 action，与 input_entry_points.action_binding 对应" },
          "rect_transform":   { "$ref": "#/$defs/rect_transform" },
          "children": {
            "type": "array",
            "items": { "$ref": "#/$defs/ui_node" }
          }
        }
      }
    }
  }
}
```

## 从 `ui_flow_contract.screens` 生成 Prefab 层级的映射规则

### Screen → Prefab 根节点

```
screen.screen_id: "game_hud"
→ prefab_name: "Screen_GameHUD"
→ unity_prefab_path: "Assets/AutoDesign/Prefabs/UI/Screen_GameHUD.prefab"
→ root_object.name: "Screen_GameHUD"
→ root_object.rect_transform: 全屏拉伸 (anchor_min=[0,0], anchor_max=[1,1], offset=[0,0,0,0])
```

### Panel → Image 节点

```
screen.panels: ["ResourcePanel", "ObjectivePanel"]
→ children[0]: { name: "ResourcePanel", component_type: "Panel", image_type: "Sliced", sprite_asset_id: "bg_resource_panel" }
→ children[1]: { name: "ObjectivePanel", ... }
```

### Control（按钮）→ Button 节点

```
screen.controls[0]: { type: "button", id: "btn_build", label: "建造", action: "open_build_menu" }
→ node: {
    name: "Btn_Build",
    component_type: "Button",
    sprite_asset_id: "btn_build",  ← 来自 asset_registry
    image_type: "Sliced",           ← 因为 btn_ 是 nine_slice 或 individual_sprites
    interactable: true,
    action_binding: "open_build_menu",
    children: [
      { name: "Label", component_type: "TextMeshPro", text_content: "建造", data_binding_key: "btn_build.label" }
    ]
  }
```

### data_bindings → data_binding_key

```
ui_flow_contract.data_bindings[0]: { binding_id: "resource_count", source: "ResourceInventory.ironOreCount", target_screen: "game_hud", target_element: "ResourcePanel.IronOreLabel" }
→ 在对应节点加 data_binding_key: "resource_count"
```

## 涉及范围

### 新增文件

```text
core/art_pipeline/ugui_prefab_spec_builder.py
knowledge/schemas/ai_design/ugui_prefab_spec_contract.schema.json
```

### 修改文件

```text
pipeline/step_12_art_production/plugin.py   ← 生成 ugui_prefab_spec_contract.json
pipeline/artifact_layer/registry.json       ← 新增 stage_12 条目
```

### 新增 registry 条目

```
| path | schema | consumed_by |
|------|--------|-------------|
| outputs/artifacts/stage_12/ugui_prefab_spec_contract.json | knowledge/schemas/ai_design/ugui_prefab_spec_contract.schema.json | Step13, Step14 |
```

## 执行步骤

1. 定义 schema。
2. 在 `core/art_pipeline/ugui_prefab_spec_builder.py` 实现：
   - `build_ugui_prefab_spec(ui_flow_contract, slice_spec, import_settings_contract, asset_registry) -> dict`
   - 每个 `screen` 生成一个 Prefab 规范
   - 按 panels/controls/data_bindings 递归生成子节点
   - `image_type` 由 slice_spec 决定（nine_slice → Sliced，whole → Simple）
3. Step12 在 `unity_import_settings_contract.json` 和 `sprite_atlas_plan.json` 后，生成 `ugui_prefab_spec_contract.json`。
4. Step13 读取此合同，生成 Unity Prefab 的 JSON 描述（或直接写 Unity YAML 格式的 .prefab 文件内容）。

## 完成标准

1. Step12 输出 `ugui_prefab_spec_contract.json`。
2. 每个 `ui_flow_contract.screens` 条目都有对应 Prefab 规范。
3. 所有 `controls` 条目的 `action_binding` 有值（来自 `input_entry_points`）。
4. 所有 `data_binding_key` 非空的节点在 `ui_flow_contract.data_bindings` 中有对应记录。
5. 不存在没有 `sprite_asset_id` 的 Image/Button/Panel 节点（除非 component_type=Empty 或 LayoutGroup）。
6. 单元测试覆盖：带 2 个 button + 1 个 panel 的 screen → 正确生成 3 层节点结构。

## 不做事项

- 不实际写 Unity `.prefab` 文件（只生成规范，Step13 负责写入）。
- 不处理动画（Animator、Animation Clip）。
- 不处理 3D UI（WorldSpace Canvas 的具体位置由 scene_bootstrap_contract 管）。
- 不硬编码任何具体游戏的 UI 布局值。
