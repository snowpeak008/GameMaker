# T-W7-3f PromptLibrary 种子化（20-30 条真取舍问句）——断点申报

## 2026-09-03 21:04 开工
- 基线 `cargo test -p adm4-decision`：70 passed，全绿。
- 数据形状确认：`PromptEntry { id, domain, question_zh, follow_ups, source_ref }`，`PromptLibrary { entries }`，均 `#[serde(default)]`。
- 素材库定位：`knowledge\design_space\universal\v2_checklist.json`（4.7MB，含 decision_points）+ `core.json`（12KB，也含 decision_points）。domains.json 存领域 id。
- 下一步：脚本提取 v2_checklist 决策点 id/问题文本，按域挑选真金改写。

## 2026-09-03 21:12 素材打捞完成
- 正则抽取 v2_checklist.json 全部 2575 点（id/domain/level/question/design_question）到临时文件，域分布 103 个 decision 组。
- v2 原句确认为模板冲压（同一动词模板 x 5 维度 x 5 子项），真金在「子项名 x 维度」的组合语义里，改写为取舍问句。
- 选点范围：randomness/item_resource/economy_loop/currency/reward_distribution/balance_economy（四模块弹药）+ action_rule/settlement/build/progression/pressure/reward_experience/balance_difficulty/content_supply（通用域）。
- 背包域 v2 无独立决策组，容量/囤积取舍从 chu_bei_shang_xian（储备上限）、zi_yuan_chu_bei（资源储备）、xiao_hao_gui_ze（消耗规则）打捞，如实申报。
- 通用域 domain 用 domains.json 既有 id：gameplay_system_design / core_experience_design / balance_design / content_design。
- 下一步：写 seed.json（目标 28 条）+ prompt_library.rs 测试。

## 2026-09-03 21:20 完工
- seed.json 落地：30 条 PromptEntry，覆盖 8 个域（sys.equipment/inventory/loot/economy 各 4-5 条 + gameplay_system_design/core_experience_design/balance_design/content_design）。
- 全部 30 条 source_ref 脚本核对 + 测试机器断言双重确认可溯源到 v2_checklist.json 真实决策点。
- 永久测试 tests\prompt_library.rs 落地：条数 >=15、id 唯一、三字段非空、source_ref 剥前缀后存在于 universal\*.json decision_points（serde_json 裸读，不依赖 adm4-space）。
- cargo test -p adm4-decision：71 passed 全绿；cargo fmt --check 无 diff。
- 可写范围遵守：仅新增 seed.json、prompt_library.rs、本申报文件；零 src/既有测试改动（git 层面 V4/ 在 .gitignore 中为仓库既有配置，非本卡行为）。
- 备注：背包域在 v2 无独立决策组，sys.inventory 的 4 条从 chu_bei_shang_xian / zi_yuan_chu_bei / xiao_hao_gui_ze 打捞改写，溯源成立。
