# 真 AI 概念访谈实调用记录（补 5a 已知缺口）

> 2026-09-06 主开发执行。用户提供 DeepSeek API 密钥后补跑 5a 验收单登记的
> 「真 AI 访谈未验证=已知缺口」（执行计划二轮必改 4）。
> **结论：真 AI 通道本身已验证可用；但四次尝试全部在概念提案的最后一道校验被拒——
> 暴露一个真实产品缺口（非 AI 质量问题），已登记为波 6 前置修复项。**

## 1. 通道验证（已通过）

| 项 | 结果 |
|---|---|
| 密钥写入 | `ai secret-set deepseek_key` → 值不落日志/不进存档/不进报告（回执只报名字与字符数） |
| 配置 | `.adm4_data/config/app.json`：provider_id=deepseek、base_url=https://api.deepseek.com/v1、model=deepseek-chat、api_key_ref=named:deepseek_key |
| `ai doctor` | [OK] 已配置且密钥可解析（退出码 0） |
| `ai invoke-check` | **[OK] 实调用成功，2 字符应答，耗时 411 ms**（真发请求、真消耗额度） |

即：HTTP 通道、密钥解析、模型可用性三项全部实证通过，`adm4-ai` 的 provider 层无缺陷。

## 2. 四次概念访谈实调用（全部被校验拒收）

每次都是「DeepSeek 返回结构合规的 JSON 提案 → 引擎解析成功 → 落盘前校验拒收」。
**AI 输出格式零问题**（说明 3d 的 purpose 提示词与 schema 约束有效，越界防线也没有误伤）；
拒收原因全部集中在**「提案实例与 pack 现状的接缝」**：

| # | pack | 口述 | 拒收原因 |
|---|---|---|---|
| 1 | spire_like | 爬塔卡牌肉鸽 | 实例 id `combat_main` 与 pack 既有实例**重复** |
| 2 | lane_defense | 爬塔卡牌肉鸽 | 实例 `turn_combat` 的 `sys.player_input.command_intent` 无法绑定（pack 无该核心名词） |
| 3 | lane_defense | 刷宝挂机（装备/掉落/合成/货币） | 实例 `equip_main` 的 `combat_attribute` 无提供方（无战斗系统实例、pack 未声明核心名词） |
| 4 | grid_strategy | 战棋（网格/地形/升级） | 实例 id `grid` 与 pack 既有实例（4b 迁移的战术棋盘实例）**重复** |

## 3. 缺口诊断（两条，均为产品侧非 AI 侧）

### 缺口 A：概念访谈对「pack 已有实例」不可见 → 必然撞 id
提案里 AI 自主命名实例（`combat_main`/`grid`/`equip_main` 都是它按语义取的合理名字），
但**样板 pack 已经把这些名字占了**（spire_like 有 combat_main、grid_strategy 迁移后有 grid）。
AI 拿不到「已占用实例 id 清单」，撞车是必然而非偶然。
- **修复方向（波 6 前置）**：概念访谈的 system prompt 注入当前 pack 的 `system_refs` 实例 id 清单
  与「不得复用」约束；或引擎侧对提案实例 id 做自动去重后缀（须保持确定性，且落盘前告知用户）。
  前者更符合「AI 提案、用户确认」纪律，推荐前者。

### 缺口 B：pack 核心名词声明缺失 → 输入类名词无处落地
`lane_defense` 无 `core_nouns` 键（六包中 towerdef/autochess/rhythm/spire/grid 五包都有）。
`sys.turn_combat`/`sys.beatmap_timeline` 这类模块 consumes `sys.player_input.command_intent`
（玩家输入是**平台事实而非系统**，按设计应由 pack 声明为核心名词），pack 没声明 → V6 悬空必拒。
- 这不是 AI 的错，也不是校验器的错——**校验器行为完全正确**（fail-closed 点名修复方向），
  是 lane_defense 作为 W7 前的老包**没跟上 core_nouns 这个波 3 新增的接缝**。
- **修复方向（波 6 前置，低风险）**：给 lane_defense 补 `core_nouns: ["player_command_intent"]`
  等平台事实名词。**注意**：ld 加 `core_nouns` 不涉及 `system_refs`，不进冻结哈希载荷
  （4b 发现 B 的金样红线只针对 `module_versions`），故**不会破金样**——但落地前须实跑金样比对确认。

## 4. 本次记录的正面结论

1. **真 AI 通道可用**（doctor + invoke-check 双实证），5a 验收单的「未验证」缺口**已消除**；
2. **AI 输出质量达标**：四次调用产出的提案都是结构合规 JSON、系统清单与档位语义合理、
   零越界（没有发明不存在的模块 id 或档位 id）——3d 的提示词工程与越界防线经真模型验证有效；
3. **校验器诚实性经真实场景验证**：四次拒收全部点名了具体实例、具体名词与修复方向，
   无一次静默放行或含糊报错——这正是 R2「宁缺勿造」与 fail-closed 纪律的预期表现；
4. **暴露的两个缺口都是「新接缝没铺到位」而非模型能力问题**，修复都是数据/提示词层面，
   零类型改动、零校验器改动。

## 5. 待办登记

| 项 | 归属 | 风险 |
|---|---|---|
| 缺口 A：概念访谈注入已占用实例 id 清单 | 波 6 前置（3d 提示词层） | 低（不改判定逻辑） |
| 缺口 B：lane_defense 补 core_nouns | 波 6 前置（数据层） | 低（不进冻结哈希，但须实跑金样确认） |
| 修复后重跑本记录第 2 节四个场景 | 波 6 验收项 | — |

> 密钥安全声明：本次全程未把密钥值写入任何文档、日志或提交；密钥存于
> `.adm4_data/config/secrets.json`（数据根，不进存档/导出包/内容指纹/git）。

---

## 6. 追加：T-W7-6pre 修复后真 AI 复验（2026-09-06）

> 执行卡 T-W7-6pre。缺口 A（提示词注入已占用实例 id 清单 + 硬约束 + 拒收文案增补清单）
> 与缺口 B（lane_defense 补 `core_nouns: ["player_command_intent"]`）落地后，
> 用真 DeepSeek 重跑 §2 两个场景。**费用意识声明：总实调用 6 次（两场景各 3 次，
> 卡内上限每场景 ≤3 次，未超）**；复验用的两个临时项目
> archive_1788659622766_101140_1（spire_like）/ archive_1788659821879_88652_1（lane_defense）
> 留在数据根备查（CLI 无 project delete 命令）。

### 6.1 逐次调用实录（每次一行）

| # | 场景 | 提示词版本 | 结果 |
|---|---|---|---|
| 1 | spire_like「爬塔卡牌肉鸽」 | 注入已占用清单+硬约束 | 拒收：**实例 id 零撞名**（tower_layout 等新名，缺口 A 注入见效），但 `tower_layout` 的 `board_occupancy` 无提供方（提案没带棋盘供给系统）——合法拒收，点名修复方向 |
| 2 | spire_like（口述细化：回合卡牌/构筑/遗物） | 同上 | 拒收：AI 把既有系统 `combat_main` 重新列进 systems → 撞名拒收，**新文案带已占用清单**（combat_main、deck_main、relic_main），防线+文案双生效 |
| 3 | spire_like（口述追加取名提醒） | 同上 | 拒收：同 #2（combat_main 重列），文案同样携带清单 |
| 4 | lane_defense「爬塔卡牌肉鸽」 | 注入版 + systems 只列新增实例约束 | 拒收：**零撞名**，`card_combat_tower` 的 `sys.player_input.command_intent` 未显式绑定 → 推导悬空（合法拒收） |
| 5 | lane_defense（同口述） | + system prompt 增补名词绑定指引 | 拒收：同 #4（deck_combat_engine 未给 noun_bindings） |
| 6 | lane_defense（同口述） | + user prompt 注入带 pack 实际核心名词的绑定示例 | 拒收：同 #4（turn_combat 未给 noun_bindings）——deepseek-chat 持续忽略显式绑定示例 |

### 6.2 复验结论与判断

1. **缺口 A 注入部分生效、防线与新文案全程如实**：两场景的第 1 次调用 AI 均自主
   取了不撞名的新实例 id（修复前撞名是 4/4 必然）；撞名重现的 #2/#3 是「AI 把
   既有系统**重列**进 systems」（口述描述整个游戏、而 pack 已装配同套系统的场景
   特有行为），拒收文案已按本卡增补携带已占用清单，人读可直接改名/删重列项。
   针对重列行为已在 system prompt 增补「systems 只列新增实例；引用既有实例走
   core_loop 绑定」约束（调用 #4 起生效，lane_defense 侧零撞名佐证），spire 侧
   因每场景 3 次费用上限未再实测——**留波 6 复验**。
2. **缺口 B 数据层已通、语料层仍卡**：ld 已声明 `player_command_intent`，scripted
   显式绑定路径全绿（e2e `lane_defense_core_nouns_e2e` 锁定）；但真 AI 三次都
   没按示例给出显式 noun_bindings。诊断为**语料/模型行为问题 + 一个新的小接缝**：
   确定性推导 `derive_binding` 对 `sys.player_input.command_intent` 的核心名词兜底
   用裸名词 `command_intent` 对照 `core_nouns`，而六包统一命名 `player_command_intent`
   ——名字不一致使兜底永不命中，绑定义务全落在 AI 显式输出上。**登记为波 6 候选**：
   或推导层允许 pack 声明输入名词映射（如 `command_intent → player_command_intent`
   的别名），或统一核心名词命名口径；属推导逻辑改动，超出本卡「提示词层+数据层」
   授权，不在本卡动。
3. **零静默放行**：6 次拒收全部点名实例/名词/修复方向，fail-closed 纪律在
   修复后依旧成立；AI 输出结构合规率 6/6（零 JSON 解析失败、零发明模块/档位 id）。

> 密钥安全声明（追加节）：本次 6 次调用全程未把密钥值写入任何文档、日志或提交。

---

## 7. 追加：T-W7-6b 修复后真 AI 复验（2026-09-06）

> 执行卡 T-W7-6b。①「systems 只列新增实例」约束（6pre 已注入，spire 侧当时因费用
> 上限未实测）与 ② derive_binding 尾段后缀兜底（本卡 ① 落地：`command_intent` →
> 唯一命中 `player_command_intent`；多候选歧义 Err 不静默绑错）修复后复验。
> **费用意识声明：本次真实调用总数 4 次（spire_like 1 次 + lane_defense 3 次，
> 卡内上限 ≤4 次，未超）**；复验临时项目 archive_1788672517980_91192_1（spire_like）
> / archive_1788672630205_101700_1（lane_defense）跑完即删，数据根零残留（已 grep 核实）。

### 7.1 逐次调用实录（每次一行）

| # | 场景 | 口述 | 结果 |
|---|---|---|---|
| 1 | spire_like「爬塔卡牌肉鸽（回合卡牌/选路/遗物/奖励三选一）」 | **通过校验**（提案 JSON 落回）：systems 只列 2 个新增实例（map_progress/reward_choice），**零撞名零重列**——既有 combat_main/deck_main/relic_main 只出现在 core_loop 引用里（约束语义完全命中）；AI 还自发给了 `sys.player_input.command_intent → player_command_intent` 显式绑定 |
| 2 | lane_defense（同口述） | 拒收：deck_capacity 的 `sys.equipment.equipment_entity` 悬空（AI 把「遗物构筑」映射成装备系统名词却没把 sys.equipment 列进 systems）——合法拒收点名修复方向；**与 command_intent 无关** |
| 3 | lane_defense（口述去掉装备语义） | 拒收：map_path_roguetrail 的 `board_occupancy` 无提供方（AI 为「爬塔选路」造了棋盘占位需求）——合法拒收；**与 command_intent 无关** |
| 4 | lane_defense（口述收窄为极简回合卡牌战斗） | **通过校验**（提案 JSON 落回）：turn_combat + run_deckbuild 双实例，`sys.player_input.command_intent` 绑到 `player_command_intent`（本次 AI 显式给了 noun_bindings），零撞名零悬空 |

### 7.2 复验结论

1. **6pre 遗留 1（spire 撞名重列）复验通过**：1 次即过，「systems 只列新增实例」
   约束在 spire_like 侧实证生效（6pre 时 #2/#3 连续重列 combat_main）。
2. **6pre 遗留 2（lane_defense command_intent 3/3 全挂）已消除**：本次 3 次调用
   **零次**挂在 command_intent 绑定上——#4 通过（AI 显式绑定，且即便 AI 不给，
   scripted 测试 namespaced_noun_falls_back_to_core_noun_suffix_variant 已锁定
   兜底推导路径同样能通）；#2/#3 拒收均为**语料问题**（「爬塔肉鸽」口述超出
   lane_defense 表达域，AI 造了 pack 没有的装备/棋盘名词），属合法拒收非新缺口，
   fail-closed 点名修复方向文案正常。
3. **零静默放行、零 JSON 解析失败**：4 次输出全部结构合规，两次通过均为合法提案，
   两次拒收均点名实例/名词/修复方向。6pre 登记的两条遗留至此**全部闭环**。

> 密钥安全声明（追加节）：本次 4 次调用全程未把密钥值写入任何文档、日志或提交。
