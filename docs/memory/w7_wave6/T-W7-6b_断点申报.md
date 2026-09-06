# T-W7-6b 断点申报：波 6 代码遗留收口

> 2026-09-06 开工。基线：`cargo test --workspace` 全绿（退出码 0）。

## 任务范围

- ① `derive_binding` 命名口径修复：带点号外部名词（`sys.X.n`）在提案内无 `sys.X`
  实例时，核心名词兜底从「裸名 `n` 精确对照」升级为「精确命中优先 + 尾段后缀
  匹配核心名词的全名变体（`command_intent` → `player_command_intent`）」；
  红线：不许静默绑错——后缀候选多于一个即歧义 Err，留给 AI 显式 noun_bindings。
  scripted 正反测试各 ≥1。
- ② spire 撞名真 AI 复验（6pre 遗留 1）：真 DeepSeek ≤3 次，检验「systems 只列
  新增实例」约束；结果追加 5a。若 ① 使 lane_defense 能通，顺手验一次（计入
  总额 ≤4 次）。
- ③ autochess_thin pack.json `trait_synergy_rule` 的「Attach 属未交付臂」历史
  注记更正为与现状一致的中性描述（1c 后 Attach 已交付）；纯文案，id/结构零改动。
- ④ 清理 6pre 复验临时项目 archive_1788659622766_101140_1 /
  archive_1788659821879_88652_1（核实无引用后删除）。
- ⑤ 全门禁：fmt / clippy / test 全绿只增 / space validate 六包 / cli_smoke /
  desktop 构建。

## 调研核实结论（≤10 分钟窗口内完成）

1. 缺口定位确认：`concept.rs::derive_binding` 带点号分支的 `[]`（无提供方实例）
   臂用 `core_nouns.contains(bare)` 精确对照——六包统一命名
   `player_command_intent`，裸段 `command_intent` 永不命中（6pre 5a §6.2 第 2 条
   属实）。修复只动该臂，`[only]`/`multiple` 两臂与裸名词路径不动。
2. 六包 core_nouns 事实：lane_defense/rhythm_micro/spire_like =
   `["player_command_intent"]`；autochess_thin 含 `player_command_intent` 等；
   towerdef_thin = td_* 三词；grid_strategy = grid_* 三词。后缀匹配
   （`cn == bare || cn.ends_with("_" + bare)`）在现有六包数据下
   `command_intent` 恰唯一命中 `player_command_intent`，无歧义。
3. ④ 引用核实：全仓 grep 两个 archive id，仅 docs\memory\w7_wave5 两份记忆文件
   提及（历史记录非活引用）；.adm4_data\config、logs 无引用。可删。

## 边界申报

- 不碰 6a 可写集（docs\design\**、docs\plan\03、6a 断点申报、6_上呈清单）。
- 不改校验拒收逻辑（歧义 Err 是兜底不命中路径的新增诚实报错，不改既有判定）；
  不动其他 knowledge；不做 git 操作；不加依赖；密钥值不进任何文件。
- 不用 PowerShell 管道改源文件。
- 真 AI 调用总数 ≤4 次。

## 断点记录

- [x] 基线全绿确认
- [x] ① derive_binding 后缀匹配兜底 + 正反测试（前任完成，主开发核实）
- [x] ③ autochess 文案更正
- [x] ④ 临时项目清理（前任完成，主开发核实）
- [x] ② 真 AI 复验 + 5a 追加（含调用次数声明）
- [x] ⑤ 全门禁

## 接力收尾记录（2026-09-06，接力子 agent 完成 ②③⑤）

### ② 真 AI 复验（总调用 4 次，未超 ≤4 上限）

- spire_like 1 次：**1 次即通过校验**——systems 只列 2 个新增实例
  （map_progress/reward_choice），零撞名零重列 combat_main（6pre 遗留 1 闭环）。
- lane_defense 3 次：#2/#3 拒收均为语料问题（「爬塔肉鸽」口述超出 lane_defense
  表达域，AI 造了 sys.equipment / board_occupancy 等 pack 没有的接缝），
  **零次挂在 command_intent**；#4（口述收窄）通过校验，
  `sys.player_input.command_intent` 绑定 player_command_intent 成功
  （6pre 遗留 2「3/3 全挂」闭环）。
- 逐次实录已追加 5a_真AI访谈记录.md §7；复验临时项目
  archive_1788672517980_91192_1 / archive_1788672630205_101700_1 跑完即删，
  数据根 grep 零残留。

### ③ 文案更正

- pack.json `autochess.auto_battle_resolution` design_question：
  「归零判定的条件分支需 RollCheck 表达（波 1 未交付臂）」→
  「归零判定的条件分支以 RollCheck 表达（真渲染已交付）」。
  id/结构/断言面零改动（rg 核实无测试断言该文案；全 knowledge「未交付」清零）。
- 补充说明：任务卡与断点申报 ③ 原文指向 `trait_synergy_rule` 的「Attach 属
  未交付臂」注记——rg 核实该点现行文案已是「Attach 模板臂已另行交付」（无需改），
  实际含「未交付」字样的是上述 auto_battle_resolution 点（RollCheck 同为 1c
  交付的四臂之一），按「与现状一致」口径更正之。

### ⑤ 全门禁结果

| 门禁 | 结果 |
|---|---|
| cargo fmt --all -- --check | 绿（先修正前任 ① 测试代码两处 rustfmt 格式偏差，纯格式化零语义改动） |
| cargo clippy --workspace --all-targets -D warnings | 绿（零警告） |
| cargo test --workspace | 全绿 646 通过 0 失败（基线 643 + 前任 ① 新增，满足只增） |
| space validate | 六包全 [OK] |
| cli_smoke.ps1 | 退出码 0 |
| cargo build -p adm4-desktop | 绿 |

### 边界申报（接力段）

- concept.rs 仅被 cargo fmt 触碰（前任落盘代码的格式修正），逻辑零改动。
- 未碰 6a 可写集 / docs\plan / 校验拒收逻辑；无 git 操作；无新增依赖；
  密钥值未进任何文件；未用 PowerShell 管道改源文件（fmt 由 cargo 执行）。
