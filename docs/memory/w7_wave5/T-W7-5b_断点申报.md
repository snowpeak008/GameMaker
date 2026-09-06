# T-W7-5b 自走棋薄样板 —— 断点申报

任务：W7 波 5 样板矩阵第 3 件（执行计划 §6）：单样板同时压 G2 对局赛制 / G5 战术棋盘 / G8 编队指挥。
开发子 agent 开工时间：2026-09-05。

## M0 调研（开工申报）

- 基线：`cargo test --workspace` 全绿，退出码 0（5a 后 623）。
- 必读完成：附录自走棋行（第 41 行）/ spire_like pack 形态基准 / spire_sample_e2e 先例 /
  五模块 module.json 接口事实 / 5a 遗留申报（effects_template entity 占位符纪律）。
- 模块档位事实（逐字节核对 module.json）：
  - sys.match_format：重档 = `mf2_elimination_bracket`（M3D2C2P2O2=11，「自走棋 8 人循环」白纸黑字在档位 summary）；
    consumes `sys.objective.win_signal` / `sys.objective.elimination_signal`。
  - sys.tactical_board：中档 = `tb1_grid_movement`（M2D2C1P2O1=8，网格移动+射程+站位槽）；
    consumes `sys.turn_combat.unit_entity` / `sys.turn_combat.move_command`。
  - sys.squad_command：中档 = `s1_formation`（M2D2C1P2O1=8，阵型+协同；人口上限=
    `roster_capacity` 点的 `population_budget` 选项）；consumes `sys.combat.combat_unit` /
    `sys.player_input.command_intent`。
  - sys.run_deckbuild：draft 档待定 db0/db1——db1 有 Induction（组合内须有实例 provides
    `turn_signal`），本组合无回合战斗模块，需查 V1 判定实现后裁量。
  - sys.economy：T1 `recycle_loop`（M2D2C2P2O1=9）无利息决策点——利息/连胜连败按任务卡
    落 pack 薄点，不改模块。
- 1c 预判（任务卡经验预判核对）：站位=tactical_board 网格 ✓、羁绊=squad_command
  `tag_count_synergy`（Attach 效果臂——**注意**：该模板含 `attach` 效果，需核实 attach
  是否为波 1 诚实 Err 臂之一 → 若是，选点须避开或停下申报）、拿牌=DrawFromPool ✓、
  利息=pack 薄点 grant_resource ✓。待源码核实后定论。

## M0 补充调研（第二任子 agent 2026-09-05，前任断线于调研期零落盘，从头核清）

- 基线复核：`cargo test --workspace` 全绿 **623 测试**，退出码 0。
- **1c 核实结论（源码事实）**：c4_capabilities.rs 的四个诚实 Err 臂 = AreaApply/**Attach**/Detach/**RollCheck**
  （`remaining_arms_are_honest_undelivered_err` 锁定；混入即整体 Err）。前任预判的「attach 是否 Err 臂」
  已确认：**是**。
- **中档雷区盘点（逐选项核对 effects_template）**：
  - sys.tactical_board tb0/tb1 的 `row_effect`（attach / roll_check）与 tb1 的 `range_los`
    （roll_check ×2）**全选项含未交付臂**；mg0/mg1 迁移点（move_rule/terrain_effect_rule）的
    entity **硬编码 `grid.unit_roster`**（grid_strategy 保 id 产物，本 pack 用必撞
    effect_dangling_entity，且改模板会破 grid_strategy——禁区）。
  - sys.squad_command s1 的 `synergy_bonus`（attach ×2）全选项脏；`formation_structure` 的
    preset_formation_table 脏（attach）、free_slot_grid 干净。
  - sys.match_format mf1 的 `series_length`（roll_check ×2）/`side_swap` 半脏/`tiebreak_rule` 半脏；
    mf2 的 `bracket_shape` 中「自走棋 8 人循环」正名选项 round_robin_points 脏（roll_check），
    single_elimination 干净；`seeding_rule`/open_random_draw 与 `reward_mapping`/smooth_curve_payout 干净。
- **1c 裁决：不触发改码上报**——任务卡预判路线（站位=网格干净点、羁绊=pack 薄点 ModifyRule、
  拿牌=DrawFromPool、利息=pack 薄点 grant_resource）可完整绕开四臂；撞臂点走**诚实 N/A 豁免**
  （每条豁免有真实设计理由：自走棋无 BO 局分/无地形/无排位系数/羁绊由 pack 薄点替位表达），
  未交付臂挡住的正名口径（round_robin_points/tag_count_synergy/range_only）逐条记验收单上报主开发。
- **裸名 entity 修复（增点归档申报，5a 接力同款口径）**：三新模块全链要用的干净选项
  entity 为裸语义名（match_participant/standing_entity/board_unit/tactical_board/squad/
  squad_assignment），修为 `{param:xxx_table_id}` 占位 + 必填 scalar text 参数。
  只修全链选用的干净选项；table-schema 参数选项（score_formula_rank/terrain_cost_pathfind）
  占位符不可用（Rows 参数不走 substitute_placeholders），不修记上报。
- **档位裁量（申报）**：
  - format_main = mf2_elimination_bracket（重档，任务卡原样，W11 重）；
  - board_main = tb1_grid_movement（中档，任务卡原样，W8）；撞臂点 N/A；
  - squad_main = s1_formation（中档，任务卡原样，W8）；synergy_bonus N/A + pack 薄点替位；
  - shop_main = db0_simple_draft（W4）：db1 有 Induction(NounProvided turn_signal)，本组合无
    回合战斗模块必 V1 block——draft 语义（自走棋商店选牌）在 db0 完整成立；
  - economy_main = basic_income T0（W5 全局中档带）：T1 recycle_loop W9 入重核但与赛制
    结构上零接口边（match_format consumes 的胜负事件无实例可供），必产 V3a/V3b 假 block；
    T0 结构自洽且「中档」按全局档带口径成立（5a 裁量先例：档位口径取可自洽解释）。
- **R-C1′ 预演**：H={format_main}（唯一 W≥9 且 κ core）；|H|=1 ≤ 中核参考线 2 →
  零 block 零 advice、无署名确认要求；B(G)=11+8+8+4+5×0.75=**34.75** ≤ mid_core 42。
  与尖塔（|H|=3 超线）形成对照——薄样板判定路径的实证价值。
- **名词绑定织网**：棋子实体=shop_main.card_entity（board/squad/economy 三处消费）、
  买牌花费=economy_main.currency_main、移动命令=squad_main.command_signal；
  胜负/淘汰/回合信号/占用表/供给水位/晋级三名词 = pack 核心名词（自动战斗为 pack 薄点产出）。
- **CLI 面缺口（上报）**：CLI 仅有 baseline `na` 通道，无 set_not_applicable 人工豁免命令——
  冒烟段无法豁免非 baseline 点，故冒烟走「装配+R-C1′ 判定」最小路径，全链 C0-C6 由 e2e 覆盖
  （engine 级豁免可用）。

## 里程碑记录

- [x] M0 调研 + 基线确认（623 绿）+ 1c 核实与档位裁量申报（第二任）
- [x] M1 增点归档：三新模块 + economy 干净选项裸名 entity → `{param:xxx_table_id}`
      占位 + 必填参数（match_format 2 点 / tactical_board 3 点 / squad_command 3 点 /
      economy 1 点；只修全链选用面 + bracket_shape/single_elimination 顺手归档；
      knowledge_modules 4 断言与 calibration_regression 13 修后全绿）
- [x] M2 autochess_thin pack 落盘（五实例 + 名词绑定织网 + 6 本体点：4 机制薄点
      + 2 实体表；R5 教训——选项 summary 里的参考游戏名会撞换皮词表，全部改写为
      「自走棋原祖/联盟系/主流口径」措辞；reference_games 字段本身是词表源不触发扫描）
- [x] M3 autochess_sample_e2e.rs 两线全绿：装配+R-C1′ 薄判定（|H|=1 零提示零确认 +
      B(G)=34.75 + V1 传导正反例）/ 全链 C0-C6（撞臂点 set_not_applicable 署名豁免
      10 条 → 冻结五门绿锁五模块版本 → 三模块 C4 契约 + 羁绊叠加序 + DrawFromPool +
      C6 跨机制边与任务零重复）
- [x] M4 cli_smoke.ps1 追加 8l 自走棋段（装配已由 space validate 段覆盖 +
      薄判定字面断言 + V1 负例非零退出与回落恢复；CLI 无人工豁免通道故全链留 e2e——
      缺口记验收单 §8.4）
- [x] M5 验收单落盘 `docs/memory/w7_wave5/5b_样板验收单.md` + 全门禁
      （627 全绿只增 / fmt 0 / clippy 0 / space validate 五包 OK / desktop 构建 0 /
      cli_smoke 见验收单）
