# v3 实施子计划 · 04 · 逆向工具链（M5 前置）

> 上位：[00 §4](00_master_design.md) · [01 总纲](01_overview_and_milestones.md)
> 里程碑：R3（工具链）→ R4（用它产品类包数据）
> 落点：新 crate `adm-new-reverse`（暂名，开放问题 O3）+ 联网证据通道进 `adm-new-ai`
> **前置理由**：品类包选项空间/基数区间需 ≥3 款参考游戏校准（文档 03 §3），数据由本工具产出。

---

## 1. 定位与 crate 归属（开放问题 O3）

00 号 §4.2：逆向工具链是「独立于项目流程的维护工具」。因此**建议新建 crate `adm-new-reverse`**，不挂 `adm-new-design`（避免项目运行时引入检索/联网依赖）。它产出的答卷 JSON 落 `design_space/<pack>/references/`（文档 03 §3），被 `adm-new-design` 只读消费。

依赖：`adm-new-game-spec`（复用 ValueKind）、`adm-new-contracts`（证据类型）、`adm-new-ai`（AI 映射 + 新增联网通道）。**不被** `adm-new-pipeline`/`adm-new-application` 依赖（产线独立）。

---

## 2. 混合 D 检索（00 §4.2，开放问题 O2）

现状缺口（探测确认）：`adm-new-ai` 只有 Codex/Claude CLI + OpenAI HTTP + image/VLM，**无 web-search 路径**。混合 D = 自动检索（官方/wiki 广度）+ 人工供证（低置信/L6 数值层）。

### 2.1 联网证据通道（新增，`adm-new-ai`）

定义一个与现有 `CompletionAdapter` 平行的 trait，接口按「返回带 `source_url` 的结构化证据」设计，实现可替换：

```rust
pub trait EvidenceSearchChannel {
    fn search(&self, q: &EvidenceQuery) -> AdmResult<Vec<EvidenceCandidate>>;
}
pub struct EvidenceCandidate {
    pub source_url: String,          // 必填（红线：宁缺勿造）
    pub title: String,
    pub snippet: String,
    pub source_type: SourceType,     // official | wiki | datamine | inference
    pub fetched_hash: String,        // 内容哈希，可缓存/复核
}
```

两个候选实现（O2，落地时二选一，接口不变）：
- **通道 A**：复用带联网的 CLI（若 Codex/Claude CLI 具备联网检索能力，包一层 `CliSearchChannel`）。
- **通道 B**：专用搜索 API / MCP（`HttpSearchChannel`，走 `http_endpoint_policy` 的 HTTPS 强制）。

> R3 先实现**接口 + 一个可跑通的通道**（A 或 B）+ 人工供证兜底；另一个作后续替换。自动档只做「官方/wiki 概览层广度」；低置信与 L6 数值层强制走人工供证。

### 2.2 人工供证通道

审核界面提供「贴来源」入口：人收集 datamine/wiki URL + 引文，填入 `EvidenceCandidate`（`source_type=datamine/wiki`，`confidence=low/med`）。**查不到就空着并进 coverage_report——宁缺勿造**（00 §4.1）。

### 2.3 检索目标：横向 ≥3 款

检索面向**同品类 ≥3 款游戏**以描出选项空间边界（文档 03 §3），不深挖单一游戏。`EvidenceQuery` 带 `game_name` 维度，产线对每款参考游戏各跑一遍。

---

## 3. 产线五步（00 §4.2）

```
检索(混合D) → AI映射(填答) → 交叉核验 → 人工审核 → 认证入库
```

| 步 | 输入 | 处理 | 输出 | 红线 |
|----|------|------|------|------|
| S1 检索 | 品类 + ≥3 参考游戏名 + 决策点清单 | 自动档拉官方/wiki 候选 + 人工供证低置信/数值 | `EvidenceCandidate[]`（含 source_url） | 宁缺勿造：无源即空 |
| S2 映射 | 证据 + 决策点选项空间 | AI 把证据映射到「选哪个选项 + 填什么参数」，每条挂 evidence | `TemplateAnswer[]`（草稿） | AI 锚定：每条挂 source_url |
| S3 交叉核验 | 低置信条目 | **第二个独立 AI 会话**复核（不同来源相互印证） | 核验标记（一致/冲突） | R3 独立性（评审工作量证明） |
| S4 人工审核 | 全部答卷 | 逐领域过卷，可改可退回 | 审核通过标记 | **未经人工审核不能入库** |
| S5 认证入库 | 通过的答卷 | 登记 game_name+aliases 进换皮词表；产 coverage_report | `Template` + `coverage_report` | — |

### 3.1 答卷数据模型（00 §4.1）

```rust
pub struct TemplateAnswer {
    pub decision_id: DecisionId,
    pub option_id: OptionId,
    pub parameters: ParameterValues,
    pub evidence: Vec<Evidence>,     // 每条挂 source_url
    pub notes: String,               // 逆向者备注
}
pub struct Evidence {
    pub source_url: String,
    pub quote: String,
    pub source_type: SourceType,
    pub confidence: Confidence,      // high | med | low
}
pub struct Template {
    pub game_name: String,
    pub aliases: Vec<String>,        // 换皮门扫描词表来源
    pub genre_pack: GenrePackId,
    pub pack_version: String,
    pub depth_reached: DesignLevel,  // 逆向到哪层（可低于 L6）
    pub answers: Vec<TemplateAnswer>,
    pub coverage_report: CoverageReport,
    pub certification: Certification, // reviewed_by, date, status
}
```

原则（00 §4.1）：只逆向**官方标准版玩法**，不收模组；L6 允许社区测定数据但 source_type/confidence 如实标注；查不到进 coverage_report。

---

## 4. 人工审核界面（GUI）

新增一个独立面板（文档 08）——**不在项目流程 tab 内**，属维护工具。逐领域展示答卷 + 证据链接，支持：改选项/改参数/退回重逆向/标注 notes。审核状态持久化。未审核的 `Template.certification.status != approved` → 不能入库、不能预填项目。

---

## 5. 换皮扫描词表登记（接红线 R5）

S5 认证入库时，把 `game_name + aliases` 写入**全局换皮扫描词表**。现有雏形 `adm-new-pipeline/src/cross_genre_evaluation/forbidden_source_tokens.json` 升级为该词表的存储：

- 词表由「模板库所有 game_name + 别名」构成（00 §3.5 换皮门第 3 条）。
- 供冻结门换皮扫描（文档 05）+ Phase1/2 全程扫描（红线 R5，文档 07）复用。
- 入库即登记，卸载模板即移除对应词条。

---

## 6. R3 / R4 交付清单

**R3（工具链，能跑）**：

| 交付 | 文件 | 验证 |
|------|------|------|
| `adm-new-reverse` crate 骨架 | `crates/adm-new-reverse/` | 加入 workspace 编译 |
| `EvidenceSearchChannel` + 一个通道 | `adm-new-ai/src/evidence_search/`（新模块） | 跑通一次真实检索（A 或 B） |
| 人工供证 + 答卷模型 | `adm-new-reverse/src/{evidence,answer}.rs` | serde + 空源不造假 |
| AI 映射（S2）+ 交叉核验（S3） | `adm-new-reverse/src/{mapping,cross_check}.rs` | 两独立会话哈希不同（R3） |
| 人工审核界面 | `web/src/features/reverse-review.js` + Tauri 命令 | 过卷/退回可用 |
| 换皮词表登记 | 升级 `forbidden_source_tokens.json` 读写 | 入库→词表新增 |

**R4（产数据）**：见文档 03 §5 R4 表（塔防 ≥3 参考答卷 + 交叉验证 + 基数回填）。

**R3 完成定义**：能对一款塔防游戏跑通 S1→S5，产出一份带真实 source_url、经人工审核、已登记换皮词表的 `Template`。抽查证据链接真实可达 = 通过。
