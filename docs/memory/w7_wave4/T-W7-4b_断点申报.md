# T-W7-4b 断点状态申报：存量两包拆解迁移 + 25 逆向模板标 smoke_test

## 里程碑 0：开工申报（基线确认 + 必读完成）

- 基线：`cargo test --workspace` 全绿，**600 passed / 0 failed**（4-0 报告口径 589+11=600 一致；并行卡 4c 若再增只增不减）。
- 必读完成：
  - W7 定稿 §7.2（两包拆解归属：grid 战斗四点→#2、ld 波次/经济→#7+#15）与 §2.5（引用+绑定语义、前缀重写、module_versions 进冻结哈希）；
  - `lane_defense/pack.json`（17 决策点）与 `grid_strategy/pack.json`（20 决策点）全文；
  - `system_loader.rs`：重写=纯字符串前缀替换 `<module_id>.`→`<instance_id>.`；**tier 合成点 `<instance>.tier` 恒被合成、L3 单选 Unlocked、无人 unlock=根点恒 Active 进分母**；实例 id 在 pack 内唯一（重复即 Err）；
  - `tools/spec_diff`（4-0 交付：--old/--new/--map，exact/prefix/ignore_paths，退出码 0/1/2）；
  - `adm4-template/src/model.rs`：Template 无 smoke_test 字段 → 本卡加 `#[serde(default)]` 字段；25 份 builtin 为 BulkMigration 来源，`answers_digest` 只覆盖答卷结构（不含新字段），加字段不破坏迁移登记指纹；
  - 金样：`tests/golden/lane_defense`（C0-C6 × contract.json/document.md，豁免清单 3 条固定）。

## 里程碑 1：结构性发现（迁移可行性裁量，先申报再动手）

### 发现 A：实例 id 命名空间独占 → 每包最多一个模块可做「id 逐字节保持」迁移
重写规则要求实例 id = 旧前缀（`ld`/`grid`）才能装配回旧 id；而 `instantiate_system_refs`
拒绝重复实例 id ——**同一个包内只有一个模块实例能拿到旧前缀**。ld 的 deploy（→sys.build_placement）
与 economy（→sys.economy）两个可承接域不能同时以保 id 方式迁移。

### 发现 B：lane_defense 在本卡约束下不可加 system_refs（金样红线 + cli_smoke 追加纪律双重锁死）
1. `execute_freeze` 把非空 `module_versions` 写进冻结哈希载荷 → 任何 system_refs 都改变
   `content_hash` → C0 契约 `identity.frozen_hash` 与金样不一致（豁免清单无此路径且不许新增）→
   golden_diff 必漂移；
2. tier 合成点 `ld.tier` 恒被合成且为恒 Active 根点 → 进完成度分母 → cli_smoke（只可追加，
   既有断言一条不许改）的访谈回合 5 期待 `ld.counter_matrix`，实际会先提案 L3 `ld.tier` → 冒烟必破；
   冻结完备门也会因 `ld.tier` 未答而 [BLOCK]。
结论：**ld 本卡零数据迁移**，可承接点全部「留在 pack 本体」，逐点原因记入对账单；
拆解须等金样重固化窗口（golden 重新冻结时一并做，归模块量产波）。此为「不可行就停下申报」的申报。

### 发现 C：grid_strategy 可迁（无金样、冒烟 §8b 不冻结该包）
迁移集 = sys.tactical_board 能承接的 4 个网格/地形点：`grid.battlefield_system`（L3）、
`grid.move_rule`（L4）、`grid.terrain_effect_rule`（L4）、`grid.terrain_table`（L5），
实例 id=`grid` 保 id。战斗四点/回合两点→#2、波次关卡→#7 未入库，留 pack；
养成三点语义是养成非货币，sys.economy 不承接，留 pack。

（后续里程碑追加在下方。）

## 里程碑 2：接力开工（基线复核 + 验证式速读完成）

- 接力执行者复核基线：`cargo test --workspace` 全绿，**603 passed / 0 failed**（4c 已并库，較里程碑 0 的 600 只增不减）。
- 验证式速读复核前任三发现，全部成立：
  - 发现 A/B/C 与 `system_loader.rs` 实现逐条对上（前缀重写、tier 合成点恒 L3 单选 Unlocked、
    allowed_tiers 收窄剔除不可达点、`instantiate_system_refs` 拒重复实例 id）；
  - 补充确认执行要点：**tier 合成点须 ≥2 选项**（`validate_graph` 的 insufficient_options 拦
    非表格单选项点）→ grid 的 allowed_tiers 必须恰含 2 个新档；要让 space validate 点数=原值+1，
    模块须在 tb0 之下新增两个「迁移承接档」，4 个迁移点 tier_gate 归档到这两档，
    既有 tb0-tb3 十点全部因收窄被剔除；
  - 机制归属：模块选项 compiler_tags 的 `system` 只有**恰等于 module_id** 时才被重写为
    `<instance>.tier`——迁移点保持字面 `grid.battlefield_system` 标签即可在 instance_id=grid 下
    逐字节保持 C0 归属（跨实例复用留给量产波正式类型化，记入迁移方案文档）；
  - spec_diff 映射表 `IdMapping` 为 `deny_unknown_fields`：**注释性说明字段不能进
    migration_map.json 本体**，注释落同目录 `migration_map.notes.md` 侧车（原因记录在案）；
  - `identity.project_id` 内嵌 frozen_hash 前缀 → 与 frozen_hash 同源漂移，一并豁免；
    tier 合成点选择会新增 `systems[grid.tier]` 与对应 source_map 条目 → added 豁免（逐条留痕）。
- 执行顺序：先在**改动前**用当前代码捕获 grid 固定选择集的 C0 GameSpec 基准（存 adm4-app tests
  夹具），再动 knowledge 数据。
