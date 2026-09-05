# migration_map.json 注释侧车（T-W7-4b）

`migration_map.json` 是 `adm4-spec-diff`（tools/spec_diff）的映射表，供
`cargo run -p adm4-spec-diff -- --old <迁移前spec> --new <迁移后spec> --map <本表>` 与
`adm4-app` 集成测试 `grid_migration_equivalence` 共同消费。

**为什么注释放侧车而不是放表内**：`IdMapping` 反序列化声明了
`deny_unknown_fields`，映射表本体加任何注释性键都会被解析拒绝——注释义务由本文件承担，
豁免决定仍与映射表同目录留痕。

## 映射段

- `exact` / `prefix` 均为空 = **恒等映射**。发现 A + 发现 C：实例 id 取旧前缀 `grid`，
  system_loader 的纯前缀重写（`sys.tactical_board.` → `grid.`）让迁移四点在装配后
  逐字节保持旧 id（D4），因此无需任何 id 换算规则。

## ignore_paths 豁免（逐条原因）

| 路径 | 原因 |
| --- | --- |
| `identity.frozen_hash` | 发现 B 论证的预期漂移：迁移后 pack 有 system_refs，`execute_freeze` 把非空 `module_versions`（sys.tactical_board@1.1.0）写进冻结哈希载荷，哈希必然变化。 |
| `identity.project_id` | 与 frozen_hash 同源漂移：project_id 内嵌 content_hash 前 23 字符（C0 编译器拼接），哈希变则它变。 |
| `systems[grid.tier]` | 迁移引入的结构性新增：tier 合成点 `grid.tier`（L3 单选 Unlocked）由加载器合成、C0 按 L3 缺省角色编译为 SystemSpec。它在迁移前不存在，属「引用模块」这一结构变化的固有产物，不属语义漂移。 |
| `source_map[systems/grid.tier|grid.tier]` | 同上：`grid.tier` 的 source_map 锚定条目随该系统元素新增。 |

## 不豁免的部分（等价断言的实际覆盖面）

迁移四点（`grid.battlefield_system` / `grid.move_rule` / `grid.terrain_effect_rule` /
`grid.terrain_table`）与全部未迁移点的 spec 元素、机制效果、实体/表行、design_notes、
source_map 条目——映射后必须逐字节相等，任何漂移都会让
`grid_migration_c0_semantics_are_equivalent` 测试失败。

lane_defense **不建映射表**：发现 B，本卡零迁移。
