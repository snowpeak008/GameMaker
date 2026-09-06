# T-W7-1c 断点申报：机动卡「AreaApply/Attach/Detach/RollCheck 四臂真渲染」

- 领卡时间：2026-09-06
- 触发条件：波 5 三个样板场景撞臂（sys.squad_command/synergy_bonus、sys.match_format/round_robin_points、sys.build_placement/slot_legality）
- 基线：`cargo test --workspace` 全绿（退出码 0），未动手前确认。

## 断点记录

### M0 开工（2026-09-06）
- 已读 effects.rs 四变体字段定义、c4_capabilities.rs 现状（四臂走 undelivered Err，锁定测试 643 行起）。
- 待办：读 W7 定稿 §5.3、三模块 JSON、样板 e2e；查 ModifyRule 悬空校验先例位置。

### M0.5 调研结论与关键裁量（2026-09-06）
- **悬空复检落点**：ModifyRule 悬空先例在 c1_validation.rs（本卡禁改）。四臂悬空复检
  （Attach/Detach 的 target 须为 spec 实体 id 或实体类前缀，判据与 effect_dangling_entity
  同款）落 c4_capabilities.rs 新增 `check_effect_references`，execute 逐机制先查后投影，
  Err=validation 点名机制 id/变体/目标名。
- **非法字段**：Attach/Detach 缺 modifier_id、RollCheck 缺 formula → 渲染臂内结构化 Err
  （纯函数内可判，无需 spec）；AreaApply 空 params/空 filter/空 inner 均如实渲染不 Err
  （沿用「（无内层效果）」诚实空渲染先例）。
- **三场景数据修复方案（申报）**：
  1. squad_command/tag_count_synergy：Attach target=`squad` 裸名 + modify_property
     entity=`sys.combat.combat_unit`（自走棋组合内悬空）→ 均改 `{param:roster_table_id}`
     占位 + 必填 scalar text 参数（5a/5b 占位符纪律同款）。bond_pairs 是 Table schema
     （Rows 不走占位符替换）不修，记上报。
  2. match_format/round_robin_points：modify_property entity=`match_participant` 裸名 →
     `{param:participant_table_id}` 占位 + 必填参数（bracket_shape/single_elimination
     5b 已归档同款）。
  3. build_placement/preset_slot_whitelist：**Table schema 选项撞 Rows 不走占位符的基建
     缺口（5d 遗留 3 预言命中）**。c0_compile 禁改，走数据路线：parameter_schema 由
     table 改 scalar（slot_table_id/structure_table_id/occupancy_table_id 三必填），
     槽位表数据本就活在关卡侧表点（towerdef.slot_roster 列结构逐字节同款），机制点
     按 5a 以来的 xxx_table_id 纪律引用表——`preset_slot_rows` 基数键随之移除。
     predicate_rule_check 同为 Table schema 不修，记上报。**此项超出"补字段"字面，
     属 schema 形态修复，重点申报。**
- **5c 翻正裁量：做**——unbroken_chain 的 roll_check 内 entity=`scoring_session` 走实体类
  前缀解析（scoring_session.standard 在场），零数据修复即可翻，工作量小、补 RollCheck
  真实场景第三例。
- **既有豁免注记连带修**：autochess 的 row_effect/range_los/series_length 等 N/A 理由
  引用"未交付臂"字样在翻正后失真，改为纯 genre 理由（豁免本身保留）。

### M1 四臂真渲染 + 悬空复检 + 锁定测试翻转（2026-09-06）
- c4_capabilities.rs：render_effect 补 AreaApply/Attach/Detach/RollCheck 四臂
  （穷尽匹配无 `_` 臂、禁 todo!()、字段全来自作者填写、嵌套走 MAX_EFFECT_DEPTH）；
  Attach 叠加序文字与 ModifyRule 同款；空生效期/空分支/空参数如实渲染不发明。
- 新增 check_effect_references（execute 投影前整体复检）：Attach/Detach target 须
  解析到实体 id 或实体类前缀（判据同 effect_dangling_entity）；Detach modifier_id
  须被 spec 内某 Attach 声明；嵌套递归、深度同款。
- 非法字段负测试：Attach/Detach 缺 modifier_id、RollCheck 缺 formula → Validation Err。
- 锁定测试 remaining_arms_are_honest_undelivered_err 翻转为四臂正断言（4 个正测试）；
  mixed_effects_fail_whole_projection 翻转为 mixed_effects_project_each_effect。
- `cargo test -p adm4-pipeline` 全绿（71+ 单元）。

### M2 模块 JSON 三处数据修复（2026-09-06，申报）
- sys.squad_command/tag_count_synergy：attach target=`squad` 裸名与 modify_property
  entity=`sys.combat.combat_unit`（本组合悬空）→ `{param:roster_table_id}` 占位 +
  必填 text 参数（priority 保作者原值 0，字段是 i32 不能走占位符——占位符替换产字符串）。
- sys.match_format/round_robin_points：modify_property entity=`match_participant`
  裸名 → `{param:participant_table_id}` 占位 + 必填参数。
- sys.build_placement/preset_slot_whitelist：**schema table→scalar 形态修复（重点申报，
  超出"补字段"字面）**——Rows 参数不走 substitute_placeholders（5d 遗留 3 基建缺口，
  c0_compile 禁改），槽位表数据本就活在关卡侧表点（towerdef.slot_roster 列结构同款），
  机制点改为 slot_table_id/structure_table_id/occupancy_table_id 三必填引用表；
  `preset_slot_rows` 基数键无引用随之移除。predicate_rule_check 仍是 Table schema
  未修（本卡未选用，记上报）。
- `cargo test -p adm4-decision` 全绿（knowledge_modules 门禁过）。

### M3 三场景 + 5c 翻正（2026-09-06）
- autochess e2e：squad_main.synergy_bonus→tag_count_synergy（Attach 叠加序 GWT 断言）
  + format_main.bracket_shape→round_robin_points（RollCheck 晋级/淘汰两分支断言）；
  两条 undelivered_effect_arm 豁免解除，row_effect/range_los/series_length 豁免措辞
  去"未交付臂"字样（豁免保留，理由改纯 genre）。2 测试绿。
- towerdef e2e：placement_main.slot_legality→preset_slot_whitelist 真作答
  （slot_table_id/structure_table_id/occupancy_table_id 三参数）；GWT 断言判定条件
  slot_open + 成功落成三拍 + 失败 placement_rejected 拒绝分支；N/A 豁免删除。2 测试绿。
- rhythm e2e（裁量翻正）：scoring_main.combo_window timed_window→unbroken_chain
  （全连正名口径）；GWT 断言 is_miss_event 判定 + 断连归零 + 无失误空分支如实渲染。
  2 测试绿。
- 注：autochess pack 的 trait_synergy_rule design_question 内"Attach 模板属波 1
  未交付臂"历史注记在 pack.json（本卡禁改范围）——如实上报不改。

### M4 金样实比对（2026-09-06）
- 流程：备份 tests\golden → golden_backup_1c（16 文件）→ 跑 golden_make.ps1
  （C0-C6 全绿，恒写 tests\golden）→ 与备份比对 → 恢复备份原字节（16 文件
  SHA256 逐一相同，git status 金样目录零改动）。
- 比对结论：**差异只命中既有 3 条豁免，零未豁免漂移**——
  - 14 产物文件中 12 个行尾归一（CRLF→LF）后逐字符一致（备份自 repo 检出为
    CRLF、golden_make 直写产物为 LF，git text 归一化行为，非内容漂移；5d 先例
    同样以结构化比对为准）；
  - C0/contract.json 结构化差异仅 ADDED：systems[*].design_notes ×4、
    mechanics[*].design_notes ×3、tables[*].design_notes ×3、
    content[*].design_notes ×1（命中豁免 `*.design_notes` any）、
    $.graphs ×1（命中豁免 empty_array）；
  - C1/contract.json 仅 ADDED $.custom_review_targets ×1（命中豁免 empty_array）；
  - 既有键值零漂移、零 REMOVED、零 VALUE 差异——**C4 四臂渲染改动不影响金样**
    （lane_defense 不含四臂效果）实证成立。

### M5 全门禁收官（2026-09-06）

| 门禁 | 结果 |
|---|---|
| cargo test --workspace | **643 全绿 0 failed**（基线 630 → 643 只增 13；1 ignored=4b 既有裁决项） |
| cargo fmt --all -- --check | 退出码 0 |
| cargo clippy --workspace --all-targets | 零警告 |
| space validate | 六包全 [OK]（autochess 2642 / grid 2611 / ld 2604 / rhythm 2605 / spire 2610 / towerdef 2617——分母与 5d 基线逐字相同） |
| cli_smoke.ps1 | 退出码 0（8k/8l/8m 段全过；两处失真注释更新为 1c 翻正事实） |
| cargo build -p adm4-desktop | 退出码 0 |
| golden_diff -SelfTest | 三场景全部通过 |
| 金样实比对 | 差异只命中既有 3 条豁免（M4），tests\golden 已恢复原字节（git status 零改动） |

### 遗留与上报
1. **（上报）Rows/Matrix 参数不走 substitute_placeholders 未根治**：本卡按禁改范围
   以数据形态绕开（preset_slot_whitelist schema table→scalar）；同病灶的
   predicate_rule_check（build_placement）、bond_pairs（squad_command）、
   sys.tactical_board 各 Table schema 选项内 attach/roll_check 的裸名仍在。
   基建修复（c0_compile 对 Rows 逐行替换或行内引用语法）宜独立开卡。
2. **（上报）autochess pack 的 trait_synergy_rule design_question 内“Attach 模板属
   波 1 未交付臂”历史注记失真**——pack.json 属本卡禁改范围，未动；下个可写窗口
   顺手更正为“1c 已交付”即可（纯文案）。
3. **（申报）preset_slot_whitelist 的 slot_tag/enabled_from_wave 列语义**并未丢失：
   towerdef_thin 的 towerdef.slot_roster 表点列结构逐字节同款（关卡数据归关卡侧表点，
   与模块点解耦是 5d 验收单既有结论）；`preset_slot_rows` 基数键随 schema 改 scalar
   移除，towerdef pack 自带 `slot_roster_rows` 基数键不受影响。
4. **真 AI 访谈未验证**：与 5a-5d 同状态（数据根无 ai_provider，全链 ScriptedProvider）。
5. **并行卡边界**：未改 effects.rs / composition / system_module / system_loader /
   c0/c1/c6 / docs\plan / tests\golden（除 M4 临时覆盖+恢复）；零 git 操作、零新增依赖。
