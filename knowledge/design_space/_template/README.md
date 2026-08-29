# 设计空间清单填写指南（E4 输入门）

决策点与选项的**内容**由用户提供，工程只负责 schema、加载与校验。

## 文件布局

```
design_space/
├── universal/*.json      通用层（L0-L2 全部 + 跨品类决策点，genre_scope=universal）
├── <pack_id>/pack.json   品类包（L3-L6，genre_scope={"pack":"<pack_id>"}）
├── <pack_id>/references/*.json   逆向答卷（逆向产线产出，认证后可预填）
└── skin_wordlist.json    换皮词表（认证入库自动登记）
```

## 硬性校验规则（违例即 blocked）

1. 决策图无环；requires/conflicts/unlocks 引用必须存在；conflicts 双向对称；
2. 品类包 `reference_games` ≥ 3 款；
3. 品类包决策点层级 ∈ L3-L6；通用层必须覆盖 L0/L1/L2；
4. Table/Matrix 参数的 `cardinality_key` 必须在 `cardinality_expectations` 有区间；
5. 矩阵轴引用的决策点必须存在且可枚举；
6. custom 选项必须提供结构化 `parameter_schema`；
7. 非表格型决策点 `options` ≥ 2（表/矩阵结构点允许单结构选项，靠行列数据承载差异）；
8. `consistency_rules` 引用的决策点必须存在，`row_reference` 规则的列名必须真实存在于对应决策点的某个表结构选项。

## 跨表外键：`row_reference` 规则

一张表的某列引用另一张表的行键时（波次行指向关卡、数值行指向实体），必须显式声明，
否则悬空引用会一路穿过完成度与冻结门进 FrozenDesign：

```jsonc
{ "id": "wave_rows_reference_enemies",
  "kind": "row_reference",
  "source_decision": "ld.wave_table",   // 引用方决策点
  "source_column": "enemy_id",          // 引用方列
  "target_decision": "ld.enemy_roster", // 被引用的表
  "target_key_column": "id" }           // 被引用的行键列
```

判定语义：源表当前选项不含 `source_column`（例如选了不带关卡列的简版波次表）时规则不适用；
一旦源表产生了引用值，目标表未答/未填/缺键列一律判违规（R2 未知即停），
悬空值的报错点名「哪张表、第几行、哪个值」。

## L4 机制选项必填 `effects_template`

C0 编译不发明效果（红线 R2）。每个 L4 机制选项须声明效果模板（支持 `{param:KEY}` 占位符）：

```jsonc
"effects_template": [
  { "effect": "modify_property", "entity": "<实体表决策id>", "property": "hp",
    "formula": "hp - attack * {param:base_multiplier}" }
]
```

可用效果：`modify_property` / `spawn_entity` / `despawn_entity` / `change_state` /
`grant_resource` / `consume_resource` / `emit_signal`（封闭枚举）。

## compiler_tags 速查

| 键 | 取值 | 含义 |
|----|------|------|
| spec_role | profile/promise/genre/system/mechanic/entity_table/data_table/content/title | 覆盖按层级的默认编译角色 |
| visual_form | sprite2d/model3d/ui_only/invisible | 实体表的视觉形态（C3 白名单依据） |
| system | 决策 id | 机制显式归属的系统（默认同域 L3） |
| content_kind | 任意 | content 角色的内容类型标签 |

空白模板见 `pack.blank.json`；最小可运行样例见 `_example/mini.json`；
完整示例见 `lane_defense/pack.json`（示例数值，正式内容需逆向产线 ≥3 款参考校准）。
