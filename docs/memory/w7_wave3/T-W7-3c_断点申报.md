# T-W7-3c 断点申报（首批 4 个系统模块 JSON：装备/背包/掉落/货币）

## 2026-09-03 21:04
- 已完成：领取任务，创建本申报文件。
- 下一步：跑 `cargo test -p adm4-decision` 基线；速读 system_module.rs（已读）、定稿 §2.2/§3.4/§4.4/§5.1、lane_defense/pack.json、types.rs L170-220。

## 2026-09-03 21:15
- 已完成：调研完毕。基线 `cargo test -p adm4-decision` 70 passed 全绿。锚点确认：
  - 装备档带（夹具+定稿 §3.4）：E0=3轻/E1=7中/E2=11重/E3=12重/E4=15极重/E5=15/E6=15；13 个点 id 骨架。
  - 传导（§4.4）：E1→Module(sys.loot)≥quality_affix_weights；E2→NounProvided(skill_effect_def)；E3→NounProvided(gem_entity)+Module(sys.inventory)≥classify；E4→NounProvided(material_entity)+Module(sys.inventory)≥批量档+Module(sys.economy)≥recycle_loop。注意：0a 夹具 E4 只有 2 条传导（无 inventory 批量档），定稿 §4.4 与本任务卡均要求 3 条——module.json 按定稿口径写 3 条（夹具是骨架非全集，任务卡明示 E4 传导含 sys.inventory 批量档）。
  - 咬合方案：sys.inventory 档 id = basic_capacity/classify/batch_ops（classify、batch_ops 与装备 E3/E4 min_tier 逐字节一致）；sys.loot 档含 quality_affix_weights；sys.economy 档含 recycle_loop。skill_effect_def/gem_entity/material_entity 由 sys.loot provides（技能石按 PoE 口径走掉落——正式技能系统入库后成为第一提供方，届时为析取第二源，遗留声明）。
  - EffectSpec tag：snake_case（modify_property/grant_resource/consume_resource/draw_from_pool/modify_rule/roll_check/attach/emit_signal/despawn_entity/spawn_entity/change_state/schedule…）。
- 下一步：写 sys.equipment/module.json（13 点真内容扩写）。

## 2026-09-04（接力，前任断线后续跑）
- 已完成：接力调研。复核前任产物 sys.equipment/module.json（53KB、13 决策点、7 档、5 名词、semver 1.0.0）：
  - 基线 `cargo test -p adm4-decision` 70+1+3 全绿（接力时点）。
  - E4 传导已按定稿 §4.4 写 3 条（material_entity + sys.inventory≥batch_ops + sys.economy≥recycle_loop），前任已自行完成口径修正，无需补。
  - 装备 JSON 验证结果：**零修改**。后续由门禁测试机检确认（反序列化 SystemModule + validate() + §3.4 逐档档带断言全过）。
- 已完成：sys.inventory/module.json 落盘。3 档 basic_capacity/classify/batch_ops（与装备 E3/E4 传导 min_tier 逐字节一致）；7 决策点（capacity_model/overflow_rule/storage_expand/stack_rule/tab_structure/sort_rule/batch_discard）；5 名词；batch_ops 档带 1 条 NounProvided(currency_main) 传导（批量出售折现需货币承接，析取语义）。

## 2026-09-04（续）
- 已完成：sys.loot/module.json 落盘。3 档 basic_table/quality_affix_weights/pity_directed；7 决策点（table_structure/roll_timing/rate_model/quality_weights/affix_weight_link/pity_rule/smart_bias）；7 名词。provides 含装备点名的全部四个外部名词：drop_table（装备 consumes sys.loot.drop_table 的点号后段）、gem_entity、material_entity、skill_effect_def（技能石按 PoE 口径走掉落；遗留声明：正式技能系统入库后成为析取第二源）。
- 已完成：sys.economy/module.json 落盘。3 档 basic_income/recycle_loop/exchange_reservoir；7 决策点（income_model/income_curve/sink_structure/recycle_pricing/currency_split/exchange_rule/inflation_guard）；5 名词；provides 含 currency_main（承接 inventory batch_ops 传导与装备 consumes sys.economy.currency_main）。

## 2026-09-04（收尾）
- 已完成：永久门禁测试 tests/knowledge_modules.rs 落盘，5 项断言全过（首跑即绿）：
  1. every_module_deserializes_and_validates：遍历 knowledge/systems/*/module.json，反序列化+validate+目录名与 module_id 一致；
  2. equipment_ladder_bands_match_finalized_calibration：装备 E0-E6 总分/档带逐档对 §3.4（3轻/7中/11重/12重/15极重/15/15），并断言 E4 传导 3 条；
  3. inductions_interlock_across_module_library：全库传导咬合（Module 目标在库且 min_tier 为真实档 id；NounProvided 有人 provides）；
  4. dotted_interface_nouns_resolve_to_real_providers：带点号外部名词的提供方在库时裸名词必须真实 provides（拦拼写漂移）；
  5. every_l4_option_carries_effects_template：每个 L4 点选项 ≥2、summary 非空、effects_template 非空。
- 已完成：全量 `cargo test -p adm4-decision` 79 项（70 单元 + 5 门禁 + 1 prompt_library + 3 seed）全绿；`cargo fmt` 修正新测试文件 3 处格式后 `--check` 通过。
- **并行冲突记录（按纪律不阻塞）**：fmt 后复跑时 adm4-decision 编译失败——并行 3a 正在改 types.rs（ParameterSchema 新增 Graph/Curve 变体），src/completeness.rs:94 匹配臂未跟上（E0004）。该文件属本卡禁改范围（src/**），且缺臂修复是 3a 自己的收尾职责。本卡全部产物（3 个 module.json + 门禁测试 + 文档）在破坏发生前已全绿验证；等待 30 秒重试一次仍失败，按任务卡纪律记录并继续。JSON 与测试文件本身不依赖该编译错误的修复——3a 补上匹配臂后本门禁自动恢复可跑。
