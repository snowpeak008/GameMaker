# 08 AI 访谈三段式与 Scripted 脚本 schema（T-W7-3d）

本文档是三段访谈（概念 / 组合 / 机制）的 **AI purpose 键与 Scripted 脚本 schema 定稿**。
Scripted 通道（`--scripted-file` / `ScriptedProvider`）按 purpose 回放固定应答，
支持无人值守断言；真实 Provider 与 Scripted 走同一解析与校验路径——
**AI 输出必须结构化可校验，越界（发明模块 id / 档位 id / 决策点 id / 选项种类）即 Err 不吞（R7）**。

红线不变（D11）：AI 只提案，确认 / 执行是用户手势，AI 永不代确认、永不代签。

## 1. purpose 键总表

| purpose | 段落 | 调用点 | 应答 schema |
|---|---|---|---|
| `interview_proposal` | 既有逐点访谈 | `InterviewService::propose_next` | §5（既有，未改动） |
| `interview_concept` | 概念访谈提案 | `AppServices::interview_concept[_with]` | §2.1 |
| `interview_concept_tier` | 逐重核档位理清 | `AppServices::interview_concept_clarify_with` | §2.2 |
| `interview_composition` | 组合访谈（违例解释+修复选项） | `AppServices::interview_compose_fix[_with]` | §3 |
| `interview_mechanism` | 机制访谈（实例内逐点提案） | `AppServices::interview_mechanism_next[_with]` | §5 同形（弹药注入只改 prompt，不改应答 schema） |
| `interview_mechanism_custom` | custom 机制草案起草 | `AppServices::interview_mechanism_draft_custom_with` | §4 |

Scripted 脚本文件格式（CLI `--scripted-file`，与既有约定一致）：

```json
{
  "<purpose>": ["<应答1>", "<应答2>", "…"],
  "<purpose2>": { "可直接内嵌": "JSON 对象（自动序列化为应答文本）" }
}
```

应答队列按序弹出、弹尽复用最后一条。

## 2. 概念访谈

### 2.1 `interview_concept` 应答 schema（→ 解析为 `ConceptProposal`）

```json
{
  "systems": [
    {
      "instance_id": "equip_main",          // 必填；[a-z0-9_]；提案内与既有实例均唯一
      "module_id": "sys.equipment",         // 必填；必须在模块库（库+项目私有）内，发明即 Err
      "suggested_tier": "e3_socket",        // 必填；必须在该模块重度阶梯内，发明即 Err
      "core_link": "strong",                 // 必填；core|strong|weak|meta（κ 建议）
      "rationale": "刷宝主循环的装备承诺",   // 建议理由（落 tier 合成点 rationale）
      "noun_bindings": {                     // 可选；缺省由确定性推导补全（见 §2.3）
        "sys.loot.gem_entity": "loot_main.gem_entity"
      }
    }
  ],
  "library_external": [                      // 可选；模块库覆盖不到的系统如实标注，不发明 module_id
    { "name": "天气系统", "note": "库内暂无对应，后续走系统级 custom" }
  ],
  "core_loop": [                             // 可选；动词→实例绑定，实例必须在清单内（悬空即 Err）
    { "verb": "击杀拾取", "instance_id": "loot_main" }
  ],
  "fusion": {                                // 可选；识别到"X+Y"融合型口述时给出（非融合省略）
    "cores": [                               // ≥2 个核；instance_ids 必须在 systems 内
      { "label": "合成大西瓜", "instance_ids": ["merge_loot"] },
      { "label": "塔防", "instance_ids": ["defense_econ"] }
    ],
    "transition": "合成产物折算为建造资金进入塔防波次"  // 跨核转换说明
  },
  "notes": "整体说明"
}
```

解析后由**引擎侧计算**（AI 不产出、产出也被覆盖）：

- `heavy_core_candidates`：建议档 rating.total() ≥ 9 且 κ∈{core,strong} 的实例（字典序）；
- `per_heavy_core_mode`：候选数 > 4（W7 定稿 §4.2(c) 参考线上限）→ true；
- `hints`：超大玩法提示（**只提示不设阻**；总体形态署名确认走既有 `compose confirm-form`）；
- `tier_clarifications`：清空——理清记录只能由 `interview_concept_tier` 通道产生。

### 2.2 `interview_concept_tier` 应答 schema

输入侧：prompt 携带系统实例、模块阶梯（id/label/W 值/summary）、规范问句
「这个系统你要轻度还是重度？轻重的判断依据（对标哪款游戏的哪个系统）？」与用户回答。

```json
{
  "tier_id": "e3_socket",                          // 必须在该实例模块阶梯内，发明即 Err
  "rationale": "对标 EU4 外交：全谈判栈要重度"      // 必填非空——理清的目的就是理由
}
```

落点：`ConceptProposal.tier_clarifications[instance_id] = { tier_id, rationale, user_answer }`。
确认时理清档**覆盖**建议档；`per_heavy_core_mode` 下全部重核候选必须已理清，否则确认被拒并点名。

### 2.3 noun_bindings 确定性推导（非 AI，解析期执行）

AI 未显式给出绑定时按规则补全（全部不中 → Err 点名名词与修复方向）：

1. 显式给出的绑定先校验：目标必须是 pack 核心名词，或 `<提案内提供方实例>.<名词>` 且提供方模块确实 provides；
2. 带命名空间名词 `sys.X.n`：提案内恰一个模块 `sys.X` 实例 → 绑 `<实例>.n`；多个 → 歧义 Err；没有但 `n` 是核心名词 → 绑核心名词；
3. 裸名词 `n`：本模块自身 provides → 自绑 `<self>.n`；否则核心名词；否则提案内唯一提供方。

### 2.4 确认落盘（用户手势，`interview concept-confirm`）

1. 实例引用写入项目存档 `content/system_refs.json`（3a 私有模块同款纪律：进内容指纹、随导出包走、装配时并进 extra_refs）；
2. 重装生效空间（命名空间重写 / V6 绑定 / 版本要求走加载器同一套代码，非法即整体回滚）；
3. 逐实例在 `<instance>.tier` 合成点 select + confirm（`Provenance::AiInterviewConfirmed`，rationale = 理清理由或建议理由）；
4. `core_loop` 落 `AuthoringState.core_loop`（组合校验 `CompositionInput.core_loop_verbs` 的数据源，κ 推导可用）；
5. 提案与确认进 interview transcript（R3 留痕，理清记录含用户原话）。

## 3. 组合访谈 `interview_composition`

输入侧：prompt 携带当前组合的 `[BLOCK]/[ADVICE]/[缺档]/[CONFIRM-REQUIRED]` 全部明细
与组合内每实例的档位阶梯。零违例零提示时**拒绝调用**（无可访谈内容）。

应答 schema（→ 解析为 `CompositionFixProposal`）：

```json
{
  "explanation": "人话解释：传导链哪里断了、哪个重核游离、为什么是结构缺陷",   // 必填非空
  "options": [
    {
      "option_id": "upgrade_bag",       // 提案内唯一非空（执行入口按它定位）
      "kind": "tier_change",            // 闭集：tier_change|confirm_form|replace_system|add_binding，发明即 Err
      "instance_id": "bag_main",        // tier_change/replace_system/add_binding 必填且必须在组合内
      "to_tier": "classify",            // tier_change 必填且必须在该实例阶梯内
      "binding_noun": "",               // add_binding 呈现用
      "binding_target": "",
      "detail": "做什么、为什么能消除违例、代价是什么"   // 必填非空
    }
  ]
}
```

执行（用户手势，`compose fix-apply <id> <option_id> --proposal-file <文件>`）：

- `tier_change`：改 `<instance>.tier` 合成点选择（既有 select 链路 + confirm + rationale + 留痕）；
- `confirm_form`：必须带 `--signer`（**AI 不能代签**），转发既有 `compose_confirm_form`；
- `replace_system` / `add_binding`：**不自动执行**（改变装配结构），Err 指路系统清单变更通道
  （概念访谈重新确认 / `system_module_add` / 手改引用后重装校验）——结构化建议已在提案里呈现。

## 4. custom 草案起草 `interview_mechanism_custom`

应答 schema（→ 解析为既有 `CustomMechanicDraft`，`host_system_id` 以调用方给定覆盖）：

```json
{
  "host_system_id": "（被调用方覆盖）",
  "slug": "chain_strike",
  "label_zh": "连锁打击",
  "rule_text": "连续命中同一目标叠层",                       // 必填非空
  "effects": [                                              // ≥1 条
    { "effect": "custom", "verb": "stack_bonus",
      "given": "同一目标被连续命中",                        // custom 变体 GWT 三段必填非空
      "when": "第三次命中结算",
      "then": "追加一次伤害" }
  ],
  "new_nouns": [],
  "rationale": "奖励专注输出"                               // 必填非空
}
```

产出草案**不登记**——登记仍走 `custom add --draft <文件>`（用户确认手势；
EffectSpec 全集校验由登记入口的 `EffectTemplateValidator` 做，此处只做形状与非空防线）。

## 5. 机制访谈 `interview_mechanism`（应答与既有逐点访谈同形）

```json
{ "option_id": "slot_grid", "rationale": "…", "parameters": { "slot_count": 40 } }
```

与 `interview_proposal` 的差异只在输入侧：

- 待办过滤到 `<instance_id>.` 命名空间前缀（tier 合成点与模块决策点都在其下）；
- **追问弹药注入**：PromptLibrary（`knowledge/prompt_library/seed.json`）按当前实例的
  `module_id` 作为 domain 过滤，问句与 follow_ups 追加进 user_prompt 的「追问弹药」段
  ——是 AI 的追问素材，**不是决策点**，不进决策图不进分母；
- 弹药库文件缺失 → 空弹药照常访谈（弹药是增强不是前提）；文件存在但坏 → Err。

确认 / 拒绝复用既有 `interview confirm` / `interview reject`（同一提交纪律）。

## 6. 越界防线一览（负测试锚点）

| 越界形态 | 防线位置 | 结果 |
|---|---|---|
| 发明模块 id | `concept::parse_concept_proposal` | Err「不在模块库内」 |
| 发明档位 id | concept 解析 / clarify / compose_fix 解析 | Err「不在阶梯内」 |
| 发明实例 id（修复选项） | `compose_fix::parse_fix_proposal` | Err「不在组合内」 |
| 发明选项种类 | serde 闭集反序列化 | Err「kind 只接受…」 |
| core_loop / fusion 悬空实例 | concept 解析 | Err 点名 |
| custom 草案 GWT 缺段 / rule_text 留白 | `draft_custom_mechanic` | Err 点名段位 |
| 发明决策点选项 / 参数字段 | 既有 `parse_proposal` / `check_parameter_shape` | Err（3d 未改动） |

任何越界都不留 transcript 痕、不落任何状态（提案在解析期被拒即整体丢弃）。
