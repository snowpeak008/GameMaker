# v3 实施子计划 · 05 · 双模式作业 + 冻结门

> 上位：[00 §3.3 / §3.5 / §4.3](00_master_design.md) · [01 总纲](01_overview_and_milestones.md)
> 里程碑：R5（双模式 + 冻结门 + GUI 节点重做）
> 落点：`adm-new-design`（引擎 + `ai_interview/` 扩展）+ `adm-new-game-spec`（冻结哈希）+ Tauri 命令

---

## 1. 两种作业模式（00 §3.3）

**模式 A · 手动选择**：用户在决策图上逐点选选项、填参数。UI 按领域 × L 层组织，实时显示完成度（文档 02 §5）与约束冲突。AI 不可用时**始终可用**。

**模式 B · AI 访谈（分层逐条确认）**：扩展现有 `adm-new-design/src/ai_interview/`（已有 mod/backend/prompt_packer/state/summary_agent/mapping_agent/route_planner）+ 现有 Tauri 命令（`load_ai_interview`/`submit_ai_turn`/`force_ai_output`/`mark_ai_inaccurate`/`save_ai_archive`）+ `web/src/features/ai-interview.js`。AI 不可用时模式 B 整体不可用（显式提示），不锁死模式 A。

两模式可混用（访谈 80% + 手动补 20%）。

---

## 2. 分层逐条确认（00 §3.3，解决"决策点多 vs 逐条确认"冲突）

确认负担分两层解耦——这是 v3 相对现有 ai_interview 的关键新增：

| 层 | 确认单元 | AI 行为 | 落地 |
|----|---------|--------|------|
| **结构层 L0–L4** | **每决策点逐条** | 提案「建议选 X，理由，参数建议」，用户确认/改选/改参/跳过 | 扩展 `ai_interview/state.rs` 的 turn 逻辑，每 turn = 一决策点 |
| **参数/数值表层 L5–L6** | **整表为一个确认单元** | 提案**整表值**（每格挂 rationale/来源），用户确认「表结构+AI提案」为一决策 | 新增「整表提案」turn 类型 + 例外下钻 |

要点（00 §3.3）：
- 一张 40 行属性表 = **1 次确认**，不是 40 次。
- 用户可**例外下钻**改任意行/格；未下钻的格仍需对「整表提案」点确认（`confirmed_by_user` 覆盖整表）。
- **AI 永不代提交**：只有 `confirmed_by_user=true` 的 Selection 计入完成度（文档 02 §5）。`force_ai_output` 现有命令语义需审查——不得成为「AI 替用户提交」的后门（红线 R7）。

访谈时长由**结构层决策点数量**决定，与 L5/L6 表行数无关（00 §3.3）。决策点总量不设硬上限。

### 2.1 现有 ai_interview 的扩展点

| 现有 | v3 改动 |
|------|---------|
| `state.rs::InterviewTurnStart`/`InterviewApplyReport` | 加 turn 类型：`StructuralPoint`（逐条）vs `TableProposal`（整表） |
| `prompt_packer.rs`（PromptBuildOptions/OutputPartitionPlan） | 整表提案需按表结构分区输出（每格 rationale） |
| `mapping_agent.rs` | 复用为「证据/概念→选项参数」映射，但**项目内访谈的输入是概念文档**（非逆向证据） |
| `force_ai_output`/`mark_ai_inaccurate` 命令 | 审查语义，确保不违 R7（AI 不代提交） |

---

## 3. 模板进入项目（00 §4.3）

- **预填模式**：模板答卷整卷预填进决策工具，每条 `provenance=template:<id>`；用户/AI 访谈在其上改。项目创建时明确警示「冻结前必须完成换皮」。
- **对照模式**：模板不进项目，仅在决策点侧栏显示「该游戏此处的选择与理由」。
- 两模式 UI 均可用（文档 08）。

---

## 4. 冻结门五道（00 §3.5，进入流水线的唯一入口）

按顺序全通过才可冻结。落 `adm-new-design/src/freeze/`：

| # | 门 | 机器判定规则 | 依赖 |
|---|----|------------|------|
| 1 | **完整性 / 拆分就绪度**（完备度门） | **适用**决策点（激活 or baseline 未标 N/A，见文档 02 §3.5）均 confirmed；L5/L6 表结构/行数据完整。选 L6→每属性表填满、每矩阵格有值、每关波次齐；缺格→`blocked` + 待填清单（**不给默认值**，R2）。**Inactive/NotApplicable 的点不要求填**——简单玩法（纯 IAA 超休闲）因深层点大量 Inactive，分母天然小，L4 深度档即可全绿，不卡门 | 文档 02 §3.5/§5 |
| 2 | **一致性** | requires/conflicts 图零违例；跨决策机器校验（品类包提供规则，如「克制矩阵行数=守卫种类数」） | 文档 02 §4 DAG + 品类包校验规则 |
| 3 | **换皮门** | 所有 `provenance=template:*` 的 `skin_fields` 参数必须已改（与模板原值逐字段比对）；全部产出文本过**参考名扫描**（词表=文档 04 §5）；命中即 block，列位置 | 文档 04 §5 词表 |
| 4 | **AI 红队评审** | AI 以对抗姿态找设计矛盾/不可实现/体验断裂；**最低工作量证明**（红线 R3）；发现项用户逐条处置（修改/接受风险）后过 | 红线 R3（文档 07） |
| 5 | **冻结** | 决策集 + 品类包版本 + 深度档 → 规范化 JSON → 内容哈希 → 只读。此后走版本化变更（新冻结版本），不可原地改 | `game-spec::canonicalize` + sha2 |

> **门 1 是整个 v3 的枢纽**（代码调查印证，文档 06 §4.3/§4.4）：三代 design→plan 失败的根因是"设计太薄→下游被迫发明/硬编码"。门 1 的完备度检查"下游能否不发明就拆分"——即 L4 达公式符号级、L5 是真表结构——**正是 C4 能'派生而非发明'的准入前提**。门 1 松，C4 必然退回硬编码模板（如现状 `step08_10_v2.rs:405`）。所以这道门不是流程官僚，是可派生性的硬前提。

### 4.1 冻结产物

```rust
pub struct FrozenDesign {
    pub decisions: Vec<Selection>,
    pub genre_pack: GenrePackId,
    pub pack_version: String,
    pub depth_profile: DepthProfile,
    pub content_hash: String,       // sha2，复用 game-spec canonical 风格
    pub frozen_at: String,          // 由调用方注入时间戳（Rust 侧不取系统时间）
}
```

**冻结集是唯一内容真相源**（00 §2 核心不变式）。C0（文档 06）以它为唯一输入。修改走新冻结版本，旧版只读。

### 4.2 换皮门实现要点

- **skin_fields 比对**：每个预填 Selection 记录模板原值；冻结门逐字段比对，未改的 skin_field → block（00 §3.5 第 3 条）。
- **参考名扫描**：对**所有产出文本**（label/rationale/参数中的字符串/文案）扫词表；命中列出「哪个决策点/哪个字段/命中词」。
- 这道门是 00 号「参考与项目混同」病灶的核心防线（§1）。

---

## 5. R5 交付清单

| 交付 | 文件 | 验证 |
|------|------|------|
| 分层确认 turn 扩展 | `adm-new-design/src/ai_interview/state.rs`+`prompt_packer.rs` | 结构层逐条 + 整表提案 + 例外下钻的测试 |
| `force_ai_output` R7 审查 | 现命令 | 确认无「AI 代提交」路径 |
| 模板预填/对照 | `adm-new-design/src/template_apply.rs` | 预填后 provenance 正确 + 换皮警示 |
| 冻结门五道 | `adm-new-design/src/freeze/{completeness,consistency,skin_gate,redteam,freeze}.rs` | 每道正/负例测试 |
| 换皮门（比对+扫描） | `.../freeze/skin_gate.rs` | 未换皮/残留参考名→block |
| GUI 节点重做 + 冻结面板 | `web/src/features/design.js` + 新 `freeze-panel.js` | 文档 08 |

**R5 完成定义**：手动模式走通一个塔防设计（用 R4 预填模板）→ 完成换皮 → 五道门全绿 → 产出 `FrozenDesign`（含内容哈希、只读）。这是 00 号 M2 的验证标准。
