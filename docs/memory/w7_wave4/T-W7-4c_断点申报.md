# T-W7-4c 断点申报：PromptLibrary 全量填充

任务：v2 2575 点降级为访谈提示词库，去重聚类 ≤300 条（含 seed 30 条总账）。

## 开工状态申报（批次 0）

- 基线 `cargo test -p adm4-decision` 全绿（9 个测试：module_library 5 + prompt_library 1 + prompt_library_seed 3）。
- v2 素材结构已勘：2575 点 = 104 节点（103 个满编节点 × 25 点 + gameplay_system_scope_decision 1 点占位）。
  每节点固定「5 子项 × 5 维度」冲压；维度族按 domains.json 领域绑定
  （gameplay_*/core_*/content_*/economy_*/balance_*/social_*/retention_*/liveops_*/ux_*/presentation_*/data_*/compliance_*/documentation_*/release_*/launch_*/positioning_*，
  另 product_vision 与 target_player 两节点为专属维度）。
- source_ref 语义锚点格式确认：`v2:v2.<node>.<subitem>.<dimension>`，与 seed.json 一致。
- 文件结构裁量：systems.json（11 模块域）+ combat/growth/econ/content/social/meta 六大类 + ux/ops 两个补充文件
  （UX/表现、数据/合规/发行/上线里的真取舍不硬塞进六大类，也不因分类不便而丢弃）。
- 总量规划：约 210 条（含 seed 30），远低于 300 上限——按「每条能说出取舍点在哪、两头各是什么收益」的红线挑，不凑数。

## 批次申报

- [x] 批次 1：systems.json（11 系统模块域，54 条：equipment+3/inventory+3/loot+3/economy+3 增补，7 个新模块域各 6 条）
- [x] 批次 2：combat.json 20 条 / growth.json 19 条 / economy.json 17 条（通用域：玩法/成长/商业化真金最厚，按素材分布倾斜）
- [x] 批次 3：content.json 17 条 / social.json 14 条 / meta.json 17 条
- [x] 批次 4：ux.json 14 条 / positioning.json 10 条（原计划的 ops 域并入 meta.json——数据/合规/发行/上线的真金不足以单独立文件）
- [x] 批次 5：prompt_library.rs 升级为全库校验（总量 ≤300 / 全库 id 唯一 / 溯源 / 前 12 字符去重 / 11 模块域 ≥5），
  cargo test -p adm4-decision 89 测试全绿、fmt 无 diff；对账单落盘。
  期间修复 2 处 source_ref 锚点错误（positioning.json 引用了不存在的维度后缀，脚本自查捕获后改锚真实决策点）。

## 完工状态

全库 211 条（seed 30 + 新增 181），分 10 文件；总量距上限余 89——素材真金见底，宁缺勿滥。
