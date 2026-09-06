# T-W7-5a 杀戮尖塔全量样板 —— 断点申报

任务：W7 波 5 垂直样板（定稿 §6.2 / §7.1 波 5 / 指令 7 叠加序验收）。
开发子 agent 开工时间：2026-09-05。

## M0 调研完成（开工申报）

- 基线：`cargo test --workspace` 全绿，**606 测试**（42 个测试二进制），符合 ≥606 要求。
- 关键源码事实（调研结论，全部影响实现路线）：
  1. **C4 叠加序已交付**（`c4_capabilities.rs` 波 1）：`ModifyRule` 渲染
     `…（按 priority=N 结算，同序按机制 id 字典序）`——e2e 断言直接引用该输出。
  2. **接口咬合**：`sys.tactical_board` 已 consumes `sys.turn_combat.unit_entity` 与
     `sys.turn_combat.move_command`。门禁 `dotted_interface_nouns_resolve_to_real_providers`
     要求提供方在库内时裸名词必须真实 provides——**sys.turn_combat 必须 provides 这两个名词**，
     否则既有门禁破。
  3. **ModifyRule 靶机制路线**：C1 复检（`c1_modify_rule_dangling`）要求 `target_rule`
     是 spec 内真实机制 id；加载器命名空间重写**不触碰 effects_template**。因此模块模板内
     `target_rule` 一律写 `{param:target_rule_id}` 占位符（scalar text 参数），由项目作者
     在组合期填真实机制 id（如 `combat_main.damage_formula`）——字段全来自作者填写，合 I1。
  4. **发现（上报不改）**：既有模块（sys.onboarding/sys.class_archetype/sys.economy/
     sys.loot/sys.inventory/sys.squad_command）的 `modify_rule` 模板 `target_rule` 写的是
     裸语义名（如 `feature_gate`、`price_rule_slot`），这些点一旦在真实项目激活并上全链，
     会撞 `c1_modify_rule_dangling`。effects.rs 注释说 target 可为「系统声明的 RuleSlot 名词」，
     但 C1 实现只查机制 id——RuleSlot 支路未实现。属波 1/量产波欠账，本卡不改 src，如实上报。
  5. **组合判定链路**：`ProductGrade` 由 L0 点 `u.target_scale` 映射（`midcore` → 参考线 2）；
     署名确认走 `compose_confirm_form`（h_set 快照失效重签语义已有测试）。
  6. **真 AI 配置**：`.adm4_data/config` 与全仓均无 `app.json`（`ai_doctor` 判定不可用）——
     真 AI 访谈将按「已知缺口」口径落验收单（执行计划二轮必改 4）。
  7. **未交付臂纪律**：三新模块与尖塔 pack 的全部 effects_template 只使用
     旧 7 变体 + Displace/Schedule/ModifyRule/DrawFromPool（不触 AreaApply/Attach/Detach/
     RollCheck 四个诚实 Err 臂）——按定稿 §6.2 尖塔不需要，1c 不触发。

## 设计裁量申报（主开发裁决范围内的落地口径）

- **#2 档位口径**：任务卡「#2 中档」落为四档谱系第 3 档 `tc2_status_stack`
  （M2 D2 C2 P3 O1 = **W10，全局档带=重**）——对齐定稿 §6.2「战斗 W10 重·core」。
  「中档」指模块阶梯内相对位置（tc0 轻/tc1 中/tc2 重/tc3 重），非全局档带中档；
  若按全局中档（W5-8）取 tc1，战斗不入 H，R-C1′ 判定将与定稿 §6.2 矛盾。
- **#26 地图**：按任务卡指示不开模块，pack 层 Graph 决策点表达
  （`acyclic:true, entry:single, directed:true`）。
- **意图预告**：落 pack 层 L4 机制点（信息披露），轻缺口注记写进选项 implications
  与验收单——不放进 sys.turn_combat 谱系（避免与「响应式回合未闭合」缺口纠缠）。
- **e2e 结构**：概念访谈路径全链用测试内嵌「概念变体 pack」（无 system_refs，访谈落盘
  三实例——与 3d 先例 PACK_EMPTY 同构）；真实 spire_like pack 的装配与 R-C1′ 判定
  用独立轻量测试锁；CLI 冒烟段用真实 pack 走 scripted 全链（互补覆盖，三处都到 §6.2 判定）。
- **传导设计**：db1（运行循环档）起 `NounProvided(turn_signal)`（回合事件必须有源）；
  rm1（规则修补档）起 `NounProvided(combat_rule_slot)`（修饰器必须有真实规则挂点作靶）。
  局限性：Induction 名词析取是单名词，非战斗肉鸽用 #14 可能误 V1——如实申报，
  未来撞例时按定稿 §4.4 扩展，本卡不发明。
- **PromptLibrary 弹药**：`knowledge/prompt_library/seed.json` 不在本卡可写范围。
  e2e 夹具在临时目录自造含 `sys.run_deckbuild` domain 条目的弹药副本以锁「弹药注入」
  断言；仓库 seed.json 的三新模块条目留给波 4 量产（如实申报缺口）。

## 里程碑记录

- [x] M0 调研 + 基线确认（606 绿）
- [x] M1 三模块入库过门禁（前任完成，主开发已核实：14 模块 5 断言全绿）
- [x] M2 spire_like pack + 装配判定测试（接力）
- [x] M3 全链样板 e2e（接力）
- [x] M4 冒烟段 + 全门禁（接力）
- [x] M5 真 AI 验证（不可用→已知缺口）+ 验收单（接力）

## 接力申报（M2 后半-M5，接力子 agent 2026-09-05）

- 接力基线：`cargo test --workspace` 全绿 **621 测试**（前任 M1 后 +15），退出码 0。
- **数据文件修复（就地修，接力纪律允许）**：前任三模块与 pack 的部分 effects_template
  用裸语义名作 `modify_property`/`spawn_entity` 的 entity 字段（`unit_entity`/`run_deck`/
  `card_entity`/`relic_collection`/`enemy_unit`）。C0 编译后 spec 级校验
  `effect_dangling_entity` 要求 entity 是真实实体 id 或实体类（entity_table 决策 id 前缀）
  ——裸语义名在全链必撞墙。**修复口径与前任 M0 调研结论 3（ModifyRule target 占位符路线）
  同一精神**：全部改为 `{param:xxx_table_id}` 占位符 + scalar text 必填参数，由项目作者
  在组合期填真实实体表决策 id（字段全来自作者填写，合 I1）。改动点：
  - sys.turn_combat：status_effect_rule（2 处）/status_timing（2 选项）/combo_chain_rule
    （2 选项）新增 `unit_table_id` 参数；
  - sys.run_deckbuild：draw_discard_cycle（回合末弃牌 entity）/energy_cost_rule（2 选项
    spawn_entity）/card_removal（2 选项）/upgrade_rule（2 选项）新增 `card_table_id`
    或复用 `pool_table_id`；
  - sys.rule_modifier_collect：periodic_trigger 新增 `relic_table_id`；
  - spire_like pack：intent_telegraph 两选项新增 `enemy_table_id`。
  模块门禁（knowledge_modules 5 断言）与标定回归修后重跑全绿。
- **M2 落点**：`crates/adm4-app/tests/spire_sample_e2e.rs::
  spire_like_pack_assembles_and_composition_matches_finalized_ruling`——
  真实 pack.json 原文装配（3 实例/tier 合成点 4+3+3 档/命名空间重写点在图上）；
  中核档 + tc2/db2/rm1 三档位声明后组合判定：H={combat_main,deck_main,relic_main}、
  (a) 连通 ✓、(b) blocks 空（V3b 有则必现）✓、|H|=3>2 → 恰一条 V3cCountAdvice +
  form_confirmation_required=true ✓、B(G)=31.5 ≤ mid_core 42 无预算提示 ✓
  （31.5 = 14×1.0 + 10×1.0 + 10×0.75；标定锚 37.75 含 pack 外地图/局外解锁两件，
  本 pack 三件套组合数字更小，κ 声明与定稿一致无偏高）；署名确认后不再要求、
  数量提示带「已署名确认」字样。
- **M3 落点**：同文件 `spire_full_chain_from_concept_interview_to_phase1_artifacts`——
  概念变体 pack（spire_concept，无 system_refs，与 3d 先例 PACK_EMPTY 同构，pack 层
  三决策点与真实 pack 同 id 同结构）：scripted 概念访谈（口述「爬塔卡牌肉鸽」→ 提案
  三新模块 → heavy_core_candidates 三个 → 确认落盘 tier+rationale+core_loop）→
  组合报告（|H|=3 超线 → 署名确认）→ 手动补齐激活点（留 draft_pick_rule 单点）→
  机制访谈逐点（自造 sys.run_deckbuild 弹药注入断言 + 逐点确认）→ 冻结五门全绿
  （gate2 组合段绿 + composition_form_confirmed 留痕可见 + module_versions 锁三模块）
  → C0-C6 全绿（C5/C6 人工门）→ 四伤疤断言全过：
  1. C4 `cap_deck_main.draft_pick_rule` 含「从池表 deck_main.card_pool 按规则
     weighted_by_rarity_no_duplicate 抽取 3 个到 draft_offer」（DrawFromPool）；
  2. 加法遗物（priority=10）与乘算遗物（priority=100）同靶 combat_main.damage_formula，
     GWT Then 各含「（按 priority=10/100 结算，同序按机制 id 字典序）」（指令 7 验收）；
  3. GameSpec.graphs 恰一图 spire.map_graph：directed=true/acyclic=true/entry=single/3 节点；
  4. C6 两件遗物程序任务 depends_on 均含 task_cap_combat_main.damage_formula
     （ModifyRule 跨机制依赖边），被引用方零反向边。
- 1c 检查：全链只用旧 7 变体 + Schedule/ModifyRule/DrawFromPool，C0-C6 全绿即证明
  未触 AreaApply/Attach/Detach/RollCheck 四个诚实 Err 臂——与定稿 §6.2 预期一致，未触发上报。
- **M4 落点**：`scripts/cli_smoke.ps1` 追加 8k 尖塔段——真实 spire_like pack 走 scripted
  全链最小路径（建项→通用点+画像→tier 三档→compose report 断言 [ADVICE]/[CONFIRM-REQUIRED]/
  零 [BLOCK]/B(G)=31.50/无 v5→署名确认→[CONFIRMED]→激活点补齐→冻结 gate2 绿含
  composition_form_confirmed→C0-C6→四伤疤产物断言）。附带修一处脚本基建坑：
  PowerShell 5.1 向原生 exe 传 JSON 实参会吞内嵌引号，新增 ConvertTo-NativeJsonArg
  （CommandLineToArgvW 转义），只被新段使用、既有段零改动。u.genre 无爬塔选项
  （通用层不在可写范围）选最近似 puzzle_casual，缺口记验收单。
  全门禁：fmt --check 0 / clippy 零警告 / cargo test --workspace 623 全绿（621+2 只增）/
  space validate 三包 OK（spire_like 2610 点）/ cli_smoke 退出码 0 / desktop 构建 0 /
  golden_diff -SelfTest 三场景过 / knowledge_modules 5 断言绿 / calibration_regression 13 绿。
- **M5 落点**：真 AI 探测——数据根 config/app.json 不存在，`ai doctor` [BLOCKED]
  未配置 Provider（与 M0 调研结论 6 一致）→ 按任务卡口径记「真 AI 访谈未验证=已知缺口」
  不阻塞，验收单 §6 写明补跑路径。验收单落
  `docs/memory/w7_wave5/5a_样板验收单.md`（全门禁表 / |H| 确认流实证 / 四伤疤逐项证据 /
  R-C1′ 对照定稿 §6.2 逐条表 / 意图披露轻缺口注记 / 三模块边界三测试自检 / 遗留申报）。
