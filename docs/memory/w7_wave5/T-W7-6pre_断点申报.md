# T-W7-6pre 断点申报：真 AI 访谈两缺口修复（波 6 前置）

> 2026-09-06 开工。基线：`cargo test --workspace` 全绿（630 通过，48 套件，退出码 0）。

## 任务范围

- 缺口 A：概念访谈 system/user prompt 注入已占用实例 id 清单 + 硬约束（提示词层）；
  校验拒收逻辑不动，仅拒收文案增补「已占用清单：…」；scripted 正反测试。
- 缺口 B：lane_defense 补 `core_nouns`；金样备份 → golden_make → 比对 → 恢复实证。
- 真 AI 复验：spire_like 撞名场景 + lane_defense 输入名词场景，每场景 ≤3 次，
  结果人读追加进 5a_真AI访谈记录.md。
- 全门禁：fmt / clippy / test / space validate 六包 / desktop 构建 / golden_diff -SelfTest。

## 调研核实结论（≤10 分钟窗口内完成）

1. **core_nouns 不进冻结哈希载荷（代码级核实）**：`adm4-authoring/src/freeze.rs`
   `execute_freeze` 的 payload = project_name / decisions / not_applicable / genre_pack /
   pack_version / depth_profile + 条件键 custom_points（非空才进）+ module_versions
   （非空才进）。`core_nouns` 完全不在 payload 构造里；且 ld 金样项目零 system_refs →
   module_versions 恒空不进键。理论金样零漂移，按纪律仍实跑比对确认。
2. **缺口 A 无需门面穿参**：`AppServices::project_space_at` 走
   `load_design_space_customized`，loader 在装配前已把项目私有引用（概念确认落盘的
   system_refs.json + 私有模块实例）extend 进 `pack.system_refs`——概念访谈拿到的
   `engine.space().pack.system_refs` 天然含项目私有实例，提示词层取该清单即全覆盖，
   `services.rs` 不需要改。
3. **拒收文案主战场**：`concept.rs::parse_concept_proposal` 的 seen.insert 分支
   （「实例 id 重复（或与既有实例冲突）」）——只增文案（已占用清单），不改判定。
4. **core_nouns 取词裁量**：只加 `player_command_intent`（与 spire_like/rhythm_micro/
   autochess_thin 同款，玩家输入=平台事实）。诊断场景 3 的 `combat_attribute` 属
   sys.* 系统间名词（应由战斗系统实例 provides 供给），**不加**——宁少勿滥，
   系统间名词走 provides 不进 core_nouns。

## 边界申报

- 不碰 1c 可写集：c4_capabilities.rs、knowledge\systems\{squad_command,match_format,
  build_placement,scoring_combo}、三个样板 e2e、cli_smoke.ps1。
- 不改校验拒收逻辑（只增文案）；不动 tests\golden\**（除缺口 B 流程的临时覆盖+恢复）；
  不做 git 操作；不加依赖；密钥值不进任何文件。

## 断点记录

- [x] 基线全绿确认（630 通过）
- [x] 缺口 A 落地 + scripted 正反测试（concept.rs 提示词注入 occupied_instance_ids +
      拒收文案增补清单；4 个新单测：注入断言/撞名仍拒且文案带清单/避开清单通过/
      提案内部重复的诚实文案）
- [x] 缺口 B 落地 + 金样实证（pack.json 仅加 core_nouns 键；备份 → golden_make →
      行尾归一化逐字节比对：14 产物文件中 12 个逐字节一致，C0/C1 contract 差异
      仅为既有 3 条豁免键（$.graphs 空数组 / *.design_notes / $.custom_review_targets
      空数组——金样是豁免生效前固化的，重生成产物带豁免键属预期）→ 恢复备份并
      核对逐字节一致 → 删除备份临时目录。frozen_hash 新旧一致
      sha256:64bda8cc…c864，实证 core_nouns 不进冻结哈希载荷）
- [x] 真 AI 复验 + 5a 追加（两场景各 3 次共 6 次实调用，逐次实录 + 结论 +
      新缺口候选登记见 5a §6；复验中按语料反馈两次增补 system/user prompt——
      「systems 只列新增实例」约束与核心名词绑定示例，均为提示词层）
- [x] 全门禁：fmt --check 0 / clippy 全工作区 0 警告 / test 636 通过 0 失败
      （基线 630 只增 6：concept 4 单测 + ld e2e 2）/ space validate 六包 OK
      （ld 2604 分母不变）/ desktop 构建过 / golden_diff -SelfTest 三场景过

## 遗留登记（波 6）

1. spire 侧「AI 把既有系统重列进 systems」的重列约束（本卡已在 system prompt
   增补）未在 spire 场景实测通过（每场景 3 次费用上限用尽）——波 6 复验。
2. 新小接缝候选：`derive_binding` 对 `sys.player_input.command_intent` 的核心名词
   兜底用裸名词 `command_intent` 对照，与六包统一命名 `player_command_intent`
   对不上 → 兜底永不命中，绑定义务全落 AI 显式输出。修复属推导逻辑改动
   （超出本卡授权），详见 5a §6.2 第 2 条。
3. 复验临时项目 archive_1788659622766_101140_1 / archive_1788659821879_88652_1
   留在 .adm4_data 备查（CLI 无删除命令）。
