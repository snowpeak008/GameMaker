# T-W7-5d 断点申报（塔防 G1 建造放置轻档实战 + C3 user_prompt 属性值复核）

开工：2026-09-06。开发子 agent（波 5 样板矩阵最后一件，范围经主开发调整：
ld 重表达对照推迟——发现 B，见 `docs/memory/w7_wave4/4b_迁移方案.md`）。

## 基线

- `cargo test --workspace`：**627 全绿**（passed=627 failed=0，与 5b/5c 交付后基线一致）。

## 调研期事实（≤10 分钟窗口内确认）

1. **G1 轻档事实**（`knowledge/systems/sys.build_placement/module.json`）：
   BP0 `bp0_preset_slots`，rating {m1 d1 c1 p1 o0} → W=4 轻档；activates 三点 =
   slot_legality / occupancy_rule / build_cost_timing；interface_floor=2。
2. **BP0 撞臂预判**：`slot_legality` 两选项 effects 均为 `roll_check`（RollCheck =
   波 1 四未交付臂之一，C4 渲染必 Err）——与 5b squad_main.synergy_bonus 同款
   「正名点被未交付臂挡住」。预案：该点署名 N/A 豁免，放置合法性由干净激活点
   occupancy_rule（claim_exclusive … fail_if_occupied，占用约束语义）+
   build_cost_timing（instant_build，塔防口径）+ pack 薄点承载到 C4。
3. **C3 现状**（`crates/adm4-pipeline/src/c3_content.rs::describe_asset`）：
   user_prompt = `实体：{name}（{id}），属性：{列键名逗号连接}`——**只有属性键名，
   无属性值**。审计 B 速修项欠账属实。
   - EntitySpec.properties 是 PropertySpec（key/kind/constraint）**不含值**；
     行值在 GameSpec.tables（entity.id = `{table_id}.{row_id}`）。修复须回表取行。
   - **金样 C3 产物不含 user_prompt 字段**（tests/golden/lane_defense/C3/contract.json
     只有 assets/ui_entries/non_visual_entities/cardinality；document.md 由 contract
     渲染）。金样固化与比对链（golden_make/golden_diff）走 ScriptedProvider 按
     purpose 出固定应答、不消费 prompt → 修复=纯提示词增强，产物字节不动。
   - 修复判定：**修**（待金样实比对零漂移验证后才落最终结论，风险即挂账）。
4. **金样比对面**：golden_diff 比 document.md 逐字节 + contract.json 结构化；
   均不含 AI 请求面。实比对方式：临时脚本照抄 golden_make §1-§6 建项目到 %TEMP%
   数据根（不跑 §7 固化、不碰金样目录），再 golden_diff -DataRoot。

## 计划

1. `knowledge/design_space/towerdef_thin/pack.json`：system_refs 三实例
   （placement_main=sys.build_placement 声明 BP0 / economy_main=sys.economy 轻-中 /
   score_main=sys.scoring_combo 轻档——任务卡可选项转必用以满足「至少三实例」，
   波次 #7 未入库走 pack 本体薄点）+ 波次生成/敌人表/塔表薄点 ≤6 + reference_games ≥3。
2. e2e `crates/adm4-app/tests/towerdef_sample_e2e.rs`：形态照抄 autochess_sample_e2e
   （装配零悬空 / 组合判定实录 / 全链 C0-C6 / BP0 C4 证据 / C6 放置任务）。
3. C3 修复（判定安全后）+ 金样实比对零漂移 + e2e 内用 ScriptedProvider.calls()
   断言 user_prompt 带属性值。
4. cli_smoke.ps1 追加塔防段（装配+判定最小路径，既有段零改动）。
5. 文档：5d_C3复核结论.md、5d_样板验收单.md。

## 收尾状态（2026-09-06 完工）

全部交付落盘：pack + e2e（2 测试全绿）+ C3 修复（金样实比对干净）+ 冒烟 8m 段 +
两份文档。全门禁：630 全绿 / fmt 0 / clippy 0 / 六包 [OK] / cli_smoke 退出码 0 /
desktop 构建 0 / golden_diff -SelfTest 过 + 实比对干净。详见 5d_样板验收单.md。

## 纪律确认

- 不用 PowerShell 管道改源文件；effects_template entity 引用一律 `{param:xxx_table_id}`
  占位+必填参数；pack 文案零商业游戏名（游戏名只进 reference_games）。
- 不改 ld/grid/spire/autochess/rhythm pack 与既有模块（增点归档除外，逐条申报）；
  不改 tests/golden/**；不动 docs/plan；零 git 操作、零新增依赖。
