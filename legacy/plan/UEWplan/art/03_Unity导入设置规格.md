# Unity 导入设置规格（Step12-Art-A）

## 目标

将现有 `_asset_import_settings()` 从只有 3 个字段（texture_type / alpha_is_transparency / pixels_per_unit）扩展为完整的 Unity TextureImporter 配置，并生成机器可读的 `.meta` 片段供 Step13 使用。

## 现有状态（代码位置）

`core/engines/generation.py:2999`：
```python
def _asset_import_settings(asset_type: str) -> dict[str, Any]:
    if normalized in {"ui", "icon", "sprite"}:
        return {
            "texture_type": "Sprite",
            "alpha_is_transparency": True,
            "pixels_per_unit": 100,
        }
```

**缺少：** FilterMode、Compression、MaxSize、SpriteMode、PackingTag、MipMaps、sRGB、WrapMode、PreserveAlpha 等关键字段。

## 新增合同：`unity_import_settings_contract.json`

### 位置
```
outputs/artifacts/stage_12/unity_import_settings_contract.json
```

### Schema（新增 `knowledge/schemas/ai_design/unity_import_settings_contract.schema.json`）

```json
{
  "required": ["schema_version", "source_refs", "import_configs"],
  "properties": {
    "schema_version": { "type": "string" },
    "source_refs": { "type": "array", "items": { "type": "string" } },
    "import_configs": {
      "type": "array",
      "items": {
        "type": "object",
        "required": [
          "asset_id",
          "unity_target_path",
          "texture_type",
          "sprite_mode",
          "filter_mode",
          "compression",
          "max_size",
          "alpha_is_transparency",
          "pixels_per_unit",
          "generate_mip_maps",
          "wrap_mode",
          "s_rgb_texture",
          "packing_tag"
        ],
        "properties": {
          "asset_id":              { "type": "string" },
          "unity_target_path":     { "type": "string" },
          "texture_type":          { "type": "string", "enum": ["Sprite", "GUI", "Default", "NormalMap", "Cursor"] },
          "sprite_mode":           { "type": "string", "enum": ["Single", "Multiple", "Polygon"] },
          "filter_mode":           { "type": "string", "enum": ["Point", "Bilinear", "Trilinear"] },
          "compression":           { "type": "string", "enum": ["None", "Low", "Normal", "High"] },
          "max_size":              { "type": "integer", "enum": [32, 64, 128, 256, 512, 1024, 2048, 4096] },
          "alpha_is_transparency": { "type": "boolean" },
          "pixels_per_unit":       { "type": "number" },
          "generate_mip_maps":     { "type": "boolean" },
          "wrap_mode":             { "type": "string", "enum": ["Repeat", "Clamp", "Mirror", "MirrorOnce"] },
          "s_rgb_texture":         { "type": "boolean" },
          "packing_tag":           { "type": "string", "description": "SpriteAtlas 分组标签，空字符串表示不打包" },
          "readable":              { "type": "boolean", "description": "Read/Write Enabled，一般为 false" },
          "preserve_alpha":        { "type": "boolean" },
          "meta_fragment":         { "type": "string", "description": "对应 Unity .meta 文件的 TextureImporter 片段（YAML）" }
        }
      }
    }
  }
}
```

## 完整导入设置规则表

| 资产类型 | texture_type | sprite_mode | filter_mode | compression | max_size | mip_maps | sRGB | packing_tag |
|---------|-------------|------------|------------|------------|---------|---------|------|------------|
| `button` | Sprite | Multiple（状态图）或 Single | Bilinear | Normal | 512 | false | true | `ui_buttons` |
| `panel_background` | Sprite | Single | Bilinear | Normal | 1024 | false | true | `ui_panels` |
| `icon` | Sprite | Single | Bilinear | None | 256 | false | true | `ui_icons` |
| `hud_element` | Sprite | Single 或 Multiple | Point（像素风）或 Bilinear | Normal | 512 | false | true | `ui_hud` |
| `nine_slice` | Sprite | Single | Bilinear | None | 256 | false | true | `ui_panels` |
| `environment_tile` | Sprite | Single | Point | None | 128 | false | true | `env_tiles` |
| `character_sprite` | Sprite | Multiple | Bilinear | Normal | 512 | false | true | `characters` |
| `effect_sprite` | Sprite | Multiple | Bilinear | Normal | 512 | false | false（Linear） | `effects` |
| `config` / `json` | Default | — | — | None | — | false | false | `` |
| `audio_placeholder` | — | — | — | — | — | — | — | `` |

## `.meta` 片段生成规则

Step12 应为每个资产生成 `meta_fragment` 字段，内容为 Unity `.meta` 文件中 `TextureImporter` 段的 YAML 片段，便于 Step13 直接写入 `.meta` 文件：

```yaml
TextureImporter:
  serializedVersion: 13
  textureType: 8          # 8 = Sprite
  spriteMode: 1           # 1 = Single, 2 = Multiple
  filterMode: 1           # 0 = Point, 1 = Bilinear
  m_Alignment: 0
  alphaIsTransparency: 1
  sRGBTexture: 1
  mipmaps:
    enableMipMap: 0
  platformSettings:
    - buildTarget: DefaultTexturePlatform
      maxTextureSize: 512
      textureCompression: 2  # 2 = Normal
  spriteSheet:
    m_PixelsToUnits: 100
    m_SpriteBorder: {x: 20, y: 12, z: 20, w: 12}  # 九宫格 border
```

## 涉及范围

### 新增文件

```text
core/art_pipeline/unity_import_settings_builder.py
knowledge/schemas/ai_design/unity_import_settings_contract.schema.json
```

### 修改文件

```text
core/engines/generation.py:2999              ← 扩展 _asset_import_settings() 或用新函数替代
pipeline/step_12_art_production/plugin.py   ← 生成 unity_import_settings_contract.json
pipeline/artifact_layer/registry.json       ← 新增 stage_12 条目
```

### 新增 registry 条目

```
| path | schema | consumed_by |
|------|--------|-------------|
| outputs/artifacts/stage_12/unity_import_settings_contract.json | knowledge/schemas/ai_design/unity_import_settings_contract.schema.json | Step13, Step14 |
```

## 执行步骤

1. 定义 schema。
2. 在 `core/art_pipeline/unity_import_settings_builder.py` 实现：
   - `build_import_config(asset: dict, slice_spec: dict | None) -> dict`
   - 参照规则表按 `asset_type` 生成全量字段
   - 如果 `ui_slice_spec_contract.json` 中该资产有 `nine_slice_borders`，写入 `meta_fragment` 的 `m_SpriteBorder`
   - 如果是 `individual_sprites`，设置 `sprite_mode = Multiple`
3. 扩展现有 `_asset_import_settings()`：保留其接口签名，内部调用新函数（向后兼容）。
4. Step12 遍历 `asset_registry.json`，为每个非占位资产生成完整配置，汇总到 `unity_import_settings_contract.json`。
5. Step13 读取 `unity_import_settings_contract.json`，为每个资产生成对应的 `.meta` 文件内容（写入 Unity 项目目录）。

## 完成标准

1. 每个 UI 类型资产配置包含 11+ 字段（非占位资产）。
2. `packing_tag` 字段非空（按资产类型归入对应 Atlas 组）。
3. `meta_fragment` 字段包含有效 YAML 片段（TextureImporter 段）。
4. 九宫格资产的 `meta_fragment` 含非零 `m_SpriteBorder`。
5. 旧 `_asset_import_settings()` 接口调用不报错（向后兼容）。
6. 单元测试：`test_unity_import_settings_builder.py` 覆盖 button/panel/icon/tile 四类。

## 不做事项

- 不实际写入 Unity 工程的 `.meta` 文件（只生成内容，Step13 负责写入）。
- 不处理音频、字体、ShaderGraph 等非 Texture 资产的 import settings。
- 不修改现有测试中对 `_asset_import_settings()` 的期望返回值（向后兼容）。
