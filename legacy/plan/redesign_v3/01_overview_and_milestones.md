# v3 实施子计划 · 01 · 总纲与里程碑

> 版本：impl-draft-1（2026-07-26）
> 上位文档：[00_master_design.md](00_master_design.md)（架构真相源，本集不改其结论，只落地）
> 性质：把 00 号总案拆成**可评审、可分阶段实施**的工程计划。本集只做计划，暂不写代码。

---

## 0. 用户已定的四项执行边界（2026-07-26）

| # | 决策 | 对本集的约束 |
|---|------|-------------|
| E1 | 本轮**先出详细子计划再定范围** | 现在不写代码；产出本文档集供评审后再决定实施到哪个里程碑 |
| E2 | 实现载体 = **NEWrust（Rust v2）** | 所有新代码落在 `NEWrust/`，与 SpecStore/ChangeKernel/GameSpec 同栈；不再动 Python 1.2 |
| E3 | **M5 逆向工具链前置** | 里程碑重排：先建逆向工具链，再用它产出品类包 ≥3 款参考答卷数据 |
| E4 | 决策点/选项**设计空间由用户提供清单** | 我不发明 L0–L6 决策点与选项枚举；设**输入门**（文档 03），我只建模型与工具围绕它 |

---

## 1. 载体决策的后果（E2 落地）

00 号 §7-Q4 说「GUI 轻改·复用图结构外壳」。探测确认 NEWrust 现状：

- GUI = **Tauri v2 + 原生 ES-module JS**（无 React/Vue），前端在 `NEWrust/web/src/`，tab 路由 `design/pipeline/patch/package/logs/sdk`。
- 「设计工作台」`web/src/features/design.js`（2130 行）是**按领域组织的表单/列表 UI，不是画布图编辑器**。「图」是数据层的依赖/节点模型。
- 因此「复用图结构外壳」= 复用**领域-节点数据模型 + 列表/表单渲染骨架**，节点内容按新决策模型重做；**不需要**造可视化图编辑器。

crate 分层（低→高，探测所得）：

```
adm-new-game-spec / adm-new-knowledge / adm-new-contracts   （底层）
        ↓
adm-new-change-kernel / adm-new-design                      （中层）
        ↓
adm-new-pipeline → adm-new-application → adm-new-tauri-commands → desktop-tauri  （高层）
```

v3 新增/改动的 crate 归属总表：

| v3 能力 | 落点 crate | 新建 or 扩展 | 依据锚点 |
|---------|-----------|-------------|---------|
| DecisionPoint/Selection 模型 + L0–L6 | `adm-new-design`（+ DTO 进 `adm-new-contracts/project.rs`） | 扩展（现有 `DecisionState`/`OptionProvenanceEntry` 之上） | 文档 02 |
| 设计空间清单 schema + 加载 | `adm-new-design/data_loader/` | 扩展（现 `DesignDataLoader` 旁加载器） | 文档 03 |
| 品类包（通用层 + 塔防/战棋） | `knowledge/design_data/`（数据）+ design 校验 | 新建数据 + 扩展校验 | 文档 03 |
| 逆向工具链（混合 D 检索 + 映射 + 审核入库） | 新 crate `adm-new-reverse`（暂名）+ 联网证据通道进 `adm-new-ai` | **新建** | 文档 04 |
| 双模式作业 + 分层逐条确认 | `adm-new-design/ai_interview/` + Tauri 命令 | 扩展（现 5 命令 + `ai-interview.js`） | 文档 05 |
| 冻结门（五道 + 换皮门） | `adm-new-design`（冻结）+ `adm-new-game-spec`（哈希/canonical） | 扩展 | 文档 05 |
| C0–C6 文档编译流水线 | `adm-new-pipeline`（新 registry）+ 契约进 `adm-new-contracts` | **新建**（与现 "00"–"14" 并存，逐步替换 Phase1） | 文档 06 |
| 7 条红线机器化 | 横切：`adm-new-contracts`（证据指针类型）+ 各步 | 新建约束 + 扩展 | 文档 07 |
| GUI 轻改（节点重做 + 访谈/冻结面板 + C0–C6 面板） | `web/src/` + `adm-new-tauri-commands` | 扩展 | 文档 08 |
| Phase 2（P0–P5，EXE） | 复用 `adm-new-change-kernel`/`adm-new-pipeline` work-unit | 后置（00 号 §5.3，另行立项） | 文档 06 §尾 |

---

## 2. 里程碑重排（E3 落地）

00 号 §6 原序把 M5 逆向工具链排在 C3–C6 之后。E3 要求 M5 前置。重排如下，并标注**每个里程碑的输入门/人工门**：

| 序 | 里程碑 | 内容 | 前置输入门 | 验证方式 | 对应文档 |
|----|--------|------|-----------|---------|---------|
| **R1** | 决策模型骨架 | DecisionPoint/Selection Rust 类型 + L0–L6 + parameter_schema(表结构) + DAG 校验；**无数据** | — | schema 单元测试；空图能加载/校验 | 02 |
| **R2** | 设计空间清单输入门 | 清单 schema + 空白模板 + 加载/校验器；通用层 + 塔防包骨架（**待用户填**） | ⛳ **用户交通用层+塔防设计空间清单** | 用户填一版塔防清单→校验通过 | 03 |
| **R3** | 逆向工具链（M5 前置） | 混合 D 检索通道（新增联网）+ AI 映射 + 交叉核验 + 人工审核界面 + 换皮词表登记 | R2 的清单 schema | 逆向 ≥3 款塔防→答卷抽查来源真实性 | 04 |
| **R4** | 品类包数据到 L6 | 用 R3 工具产出塔防包 ≥3 参考答卷 + 交叉验证选项空间/基数区间 | R3 工具就绪 | 品类包完整性自检 + 基数区间可用 | 03/04 |
| **R5** | 双模式 + 冻结门 | 手动/AI 访谈分层逐条确认 + 冻结门五道（含换皮门）+ GUI 节点重做 | R4 有可预填模板 | 手动走通一个塔防设计并冻结 | 05/08 |
| **R6** | C0–C2 | 规格编译（→GameSpec）+ 静态验证红队 + 玩法文档 | R5 能产出冻结集 | 冻结集编译 + 红队对注入缺陷检出率 | 06/07 |
| **R7** | C3–C6 | 需求/架构/风格/计划 + 两个人工门 | R6 | 文档集整体人工评审 ≥ 目标分 | 06/07 |
| **R8** | 第二品类包（格子战棋）+ 泛化 | 复跑 R2–R7 于战棋，验证无品类间污染 | R3 工具通用 | 两品类各跑一项目 | 03 |
| **R9** | Phase 2 衔接设计 | P0–P5 细化（复用 v2 ChangeKernel） | Phase1 试跑通过 | 另行立项 | 06 §尾 |

> **重排要点**：R3（逆向工具链）现在排在 R5（决策工具双模式）之前，因为品类包的**选项空间与基数区间需靠 ≥3 款参考游戏横向校准**（00 号 §3.4），而校准数据由逆向工具产出。R1/R2 只建「空模型 + 清单格式」，不依赖参考数据，可先行。

---

## 3. 与三代资产的对账（00 号 §5.5 落到具体文件）

| 现有资产 | 探测确认的真实位置 | v3 处置 |
|---------|-------------------|---------|
| 16 domains / 103 nodes | `knowledge/design_data/domains/*.json`（16 文件）+ `domain_order.json`；`DesignDataLoader` 载入 | 拓扑与领域划分**参考保留**；节点按文档 02 模型重做（新数据文件，旧文件不原地改） |
| `decision_graph/`（能力驱动图） | `adm-new-design/src/decision_graph/`（`CapabilityDecisionGraph`） | **评估复用**：其 requires/conflicts 边 + coverage 机制可支撑 DAG 校验（文档 02 §4） |
| 26+ 模板（含 lane_defense_v1） | `knowledge/design_data/project_templates/*.json`（80 文件） | 降级为「逆向素材」；按文档 04 产线重产，不直接迁移 |
| legacy 流水线 step00–14 | `adm-new-pipeline/src/stages/`（legacy + _v2 并存） | Phase1 由 C0–C6 **并存替换**（新 registry，旧的逐步下线，不增量修补 legacy） |
| GameSpec v2 | `adm-new-game-spec`（schema 2.0.0-alpha.1，canonical+sha2） | **直接复用**为 C0 编译目标；按需演进 schema（文档 06 §2） |
| SpecStore/ChangeKernel/受信测试 | `adm-new-change-kernel` | Phase 2 复用（文档 06 §尾） |
| 换皮/禁词雏形 | `adm-new-pipeline/src/cross_genre_evaluation/forbidden_source_tokens.json` | 升级为换皮扫描词表来源之一（文档 04 §5、07 R5） |
| AI 访谈 | `adm-new-design/src/ai_interview/` + 5 Tauri 命令 + `ai-interview.js` | 扩展为分层逐条确认（文档 05） |

---

## 4. 文档集导航

| 文档 | 主题 | 关键交付 |
|------|------|---------|
| 01（本文） | 总纲与里程碑 | 载体决策、里程碑重排、对账、导航 |
| [02](02_decision_model.md) | 决策模型 schema | DecisionPoint/Selection Rust 类型、L0–L6、表结构参数、DAG |
| [03](03_design_space_intake.md) | 设计空间清单输入门 | 清单 schema + 空白模板、品类包架构、≥3 参考交叉验证 |
| [04](04_reverse_toolchain.md) | 逆向工具链（M5 前置） | 混合 D 检索、AI 映射、交叉核验、人工审核入库、换皮词表 |
| [05](05_authoring_and_freeze.md) | 双模式作业 + 冻结门 | 分层逐条确认、完成度、冻结门五道、换皮门 |
| [06](06_pipeline_c0_c6.md) | C0–C6 文档编译流水线 | 逐步契约、GameSpec 编译复用、双格式产物、Phase2 边界 |
| [07](07_red_lines.md) | 7 条红线机器化 | 证据指针类型、未知即停、评审工作量证明、AI 锚定等 |
| [08](08_gui_changes.md) | GUI 轻改边界 | 节点重做、访谈/冻结面板、C0–C6 面板、命令增改 |

---

## 5. 本集遗留的开放问题（待用户拍板，见各文档细化）

| # | 问题 | 缺省建议 | 出处 |
|---|------|---------|------|
| O1 | 设计空间清单**先给塔防实例** vs **我先定空白模板格式** | ✅ **已定（用户同意）**：模板格式先行 | 文档 03 §1 |
| O2 | 混合 D 自动档具体实现：复用带联网 CLI（A）vs 专用搜索 API/MCP（B） | ✅ **已定（用户同意）**：接口先行（返回带 source_url 证据），A/B 落地时选 | 文档 04 §2 |
| O3 | 逆向工具链是**新建 crate** `adm-new-reverse` vs 挂在 `adm-new-design` 下 | ✅ **已定（用户同意）**：新建 crate | 文档 04 §1 |
| O4 | C0–C6 新 registry 与现 "00"–"14" 的**并存期长度** | 建议新 registry 独立命名空间，Phase1 全绿后再下线 legacy | 文档 06 §1 |
| O5 | 决策点是否复用 `decision_graph/` 的 `CapabilityDecisionGraph` 作 DAG 引擎 | ✅ **已定（代码审查后）**：不复用引擎（它是能力轴驱动、无选项/unlocks/深度档概念），只抄 ~120 行拓扑+计数；新写轻量校验器 | 文档 02 §4 |

新增设计问题（用户 #4 提出，已落地）：
- **完备度门 vs 简单玩法**：纯 IAA 超休闲会否因内容天然少而卡门？→ 已加**适用性判定**（激活式分母 + 显式 N/A + L4 深度档兜底），见文档 02 §3.5、03、05 门1。
