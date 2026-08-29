# SpriteAtlas 归档计划（Step12-Art-B）

> 路径说明：本文是早期拆分计划。最终实现路径以 `artlist/01_标准化美术资产管线开发计划.md` 为准，自动生成内容统一写入 `Assets/AutoDesign/`。本文中旧的 `Assets/Art/...` 示例不得作为实现路径。

## 目标

根据 `unity_import_settings_contract.json` 中的 `packing_tag` 分组，生成 SpriteAtlas 规格，让 Unity 项目中的 UI 资产按功能归类打包，减少 Draw Call。

## 前置依赖

- `art/03_Unity导入设置规格.md` 已完成（`packing_tag` 字段已定义）
- `ui_flow_contract.json` 中的 `screens` 结构

## 新增合同：`sprite_atlas_plan.json`

### 位置
```
outputs/artifacts/stage_12/sprite_atlas_plan.json
```

### Schema（新增 `knowledge/schemas/ai_design/sprite_atlas_plan.schema.json`）

```json
{
  "required": ["schema_version", "source_refs", "atlases"],
  "properties": {
    "schema_version": { "type": "string" },
    "source_refs": { "type": "array", "items": { "type": "string" } },
    "atlases": {
      "type": "array",
      "items": {
        "type": "object",
        "required": [
          "atlas_id",
          "atlas_name",
          "unity_target_path",
          "packing_tag",
          "included_assets",
          "max_atlas_size",
          "allow_rotation",
          "tight_packing"
        ],
        "properties": {
          "atlas_id":           { "type": "string" },
          "atlas_name":         { "type": "string" },
          "unity_target_path":  { "type": "string", "description": "如 Assets/AutoDesign/Art/Atlas/ui_buttons.spriteatlasv2" },
          "packing_tag":        { "type": "string" },
          "included_assets": {
            "type": "array",
            "items": {
              "type": "object",
              "required": ["asset_id", "source_path"],
              "properties": {
                "asset_id":    { "type": "string" },
                "source_path": { "type": "string" }
              }
            }
          },
          "max_atlas_size":   { "type": "integer", "enum": [512, 1024, 2048, 4096] },
          "allow_rotation":   { "type": "boolean" },
          "tight_packing":    { "type": "boolean" },
          "filter_mode":      { "type": "string" },
          "compression":      { "type": "string" }
        }
      }
    }
  }
}
```

## Atlas 分组策略

从 `packing_tag` 字段自动聚合：

| Atlas 名称 | packing_tag | 适用资产 | 推荐最大尺寸 |
|-----------|------------|---------|------------|
| `UIAtlas_Buttons` | `ui_buttons` | 所有按钮状态图 | 1024 |
| `UIAtlas_Panels` | `ui_panels` | 面板背景、弹窗 | 2048 |
| `UIAtlas_Icons` | `ui_icons` | 图标、小图 | 1024 |
| `UIAtlas_HUD` | `ui_hud` | HUD 元素 | 1024 |
| `EnvAtlas_Tiles` | `env_tiles` | 场景地砖 | 2048 |
| `CharAtlas` | `characters` | 角色 Sprite | 2048 |
| `FXAtlas` | `effects` | 特效 Sprite | 1024 |

**例外：** 单独大图（背景图 > 1024×1024）不进 Atlas，保持 `packing_tag: ""`。

## Unity 项目目录结构规范

```
Assets/
  Art/
    UI/
      Source/         ← AI 生成的原始图片（未切片）
        btn_*.png
        bg_*.png
        icon_*.png
      Sliced/         ← 切片后的 Sprite（Step12 处理后）
        Buttons/
        Panels/
        Icons/
        HUD/
      Atlas/          ← SpriteAtlas 文件
        UIAtlas_Buttons.spriteatlasv2
        UIAtlas_Panels.spriteatlasv2
        UIAtlas_Icons.spriteatlasv2
        UIAtlas_HUD.spriteatlasv2
    Environment/
      Tiles/
      Atlas/
        EnvAtlas_Tiles.spriteatlasv2
    Characters/
      Atlas/
        CharAtlas.spriteatlasv2
    Effects/
      Atlas/
        FXAtlas.spriteatlasv2
    Audio/
      .audio_placeholder  ← 现有占位文件
  Prefabs/
    UI/               ← UGUI Prefab（art/05 生成）
```

## 涉及范围

### 新增文件

```text
core/art_pipeline/sprite_atlas_planner.py
knowledge/schemas/ai_design/sprite_atlas_plan.schema.json
```

### 修改文件

```text
pipeline/step_12_art_production/plugin.py   ← 生成 sprite_atlas_plan.json
pipeline/artifact_layer/registry.json       ← 新增 stage_12 条目
```

### 新增 registry 条目

```
| path | schema | consumed_by |
|------|--------|-------------|
| outputs/artifacts/stage_12/sprite_atlas_plan.json | knowledge/schemas/ai_design/sprite_atlas_plan.schema.json | Step13 |
```

## 执行步骤

1. 定义 schema。
2. 在 `core/art_pipeline/sprite_atlas_planner.py` 实现：
   - `build_sprite_atlas_plan(import_settings_contract, asset_registry) -> dict`
   - 按 `packing_tag` 聚合资产，过滤空 tag（不进 Atlas 的资产）
   - 根据资产数量和最大尺寸建议 Atlas 尺寸（超过 64 张图建议 2048）
3. Step12 在 `unity_import_settings_contract.json` 生成后，立即生成 `sprite_atlas_plan.json`。
4. Step13 读取 `sprite_atlas_plan.json`，为每个 Atlas 生成 `.spriteatlasv2` 文件的 JSON 内容（Unity SpriteAtlas v2 格式）。

## SpriteAtlas v2 文件格式参考

```json
{
  "m_MasterAtlas": { "fileID": 0 },
  "m_PackingSettings": {
    "m_EnableRotation": false,
    "m_EnableTightPacking": false,
    "m_Padding": 4
  },
  "m_TextureSettings": {
    "m_FilterMode": 1,
    "m_MaxTextureSize": 1024,
    "m_TextureCompression": 2
  },
  "m_Sprites": []
}
```

## 完成标准

1. Step12 输出 `sprite_atlas_plan.json`。
2. 所有 `packing_tag` 非空的资产被正确分组到对应 Atlas。
3. 每个 Atlas 条目包含 `max_atlas_size`、`allow_rotation`、`tight_packing`。
4. 大图资产（tag 为空）不出现在任何 Atlas 的 `included_assets` 中。
5. 单元测试覆盖：10个 icon + 4个 button 状态图 → 正确分配到 UIAtlas_Icons 和 UIAtlas_Buttons。

## 不做事项

- 不实际执行 Atlas 打包（只生成规格 JSON，Step13 生成文件，Unity Editor 打包）。
- 不处理 3D Texture、RenderTexture。
- 不强制要求 Unity 项目已存在（Step13 负责创建目录）。
