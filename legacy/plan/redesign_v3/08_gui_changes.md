# v3 实施子计划 · 08 · GUI 轻改边界

> 上位：[00 §7-Q4](00_master_design.md) · [01 总纲](01_overview_and_milestones.md)
> 里程碑：R5（节点重做 + 冻结面板）、R6/R7（C0–C6 面板套入）、R3（逆向审核界面）
> 落点：`NEWrust/web/src/`（前端）+ `adm-new-tauri-commands`（命令）+ `desktop-tauri/src/commands/`（薄封装）

---

## 1. GUI 现状（探测确认）

- Tauri v2 + **原生 ES-module JS**（无框架，devDep 仅 playwright），前端 `NEWrust/web/src/`，自定义 `build.mjs` 打包到 `web/dist`。
- tab 路由（`web/src/main.js` 的 `applyRoute`）：`design / pipeline / patch / package / logs / sdk`。
- feature 模块：`design.js`(2130)、`pipeline.js`(1221)、`ai-config.js`、`ai-interview.js`(422)、`settings-style.js`、`utility-panels.js`(1893)。
- **设计工作台是表单/列表 UI**（按 `selectedDomainId` 过滤节点，节点含 checklist/option-group/decision + L4/L5 进度计数），**不是画布图**。
- 69 个 Tauri 命令（含 design/interview/pipeline/save/ai-config 全套）。

---

## 2. 轻改原则（00 §7-Q4）

> 设计工作台复用图结构外壳、节点内容按新决策模型重做、新增访谈/冻结门面板；**流水线面板保留当前规格**（面板与步骤解耦，新 C0–C6/P0–P5 数据驱动套入）。

三条边界：

1. **复用外壳**：保留 tab 路由 + 按领域过滤的列表/表单渲染骨架（`design.js` 的领域-节点结构）。不造可视化图编辑器。
2. **节点内容重做**：节点从旧 checklist/option-group 换成文档 02 的 DecisionPoint 渲染——含 L0–L6 层标、选项 + implications、参数表单（L5/L6 渲染成表格编辑器）。
3. **面板数据驱动**：pipeline 面板保留现规格，改为消费新 C0–C6 registry 的 stage 数据（`load_pipeline_view` 返回新 registry）。面板不硬编码步骤，套入即可。

---

## 3. 需改/新增的前端模块

| 模块 | 改动 | 里程碑 |
|------|------|-------|
| `design.js` | 节点渲染换新决策模型：L 层标、DecisionPoint 选项、L5/L6 **表格编辑器**（属性表/克制矩阵/波次表）、实时完成度（文档 02 §5）与约束冲突高亮 | R5 |
| `ai-interview.js` | 分层逐条确认：结构层逐条 turn UI + **整表提案** UI（整表确认 + 例外下钻改单元格）；`confirmed_by_user` 显式勾选；杜绝「AI 代提交」入口（R7） | R5 |
| **新 `freeze-panel.js`** | 冻结门五道的状态展示 + 逐门 block 清单（完备度待填清单、冲突、换皮命中位置、红队发现处置）+ 冻结按钮（全绿才可点） | R5 |
| **新 `reverse-review.js`** | 逆向工具链人工审核界面（逐领域过卷 + 证据链接 + 改/退回）；**独立于项目 tab**（维护工具） | R3 |
| `pipeline.js` | 消费新 C0–C6 registry（保留现规格，数据驱动）；C5/C6 人工门 UI 复用现 `confirm_style` 模式 | R6/R7 |
| `settings-style.js` | C5 风格锚点确认复用现有链路（step07 经验） | R7 |

## 4. 需改/新增的 Tauri 命令

现有相关命令（探测）：`load_design_workbench / update_node / export_design / autosave_design / list_templates / select_template`；`load_ai_interview / submit_ai_turn / force_ai_output / mark_ai_inaccurate / save_ai_archive`；`load_pipeline_view / run_pipeline_range / confirm_style`。

| 命令 | 改动 | 落点 |
|------|------|------|
| `load_design_workbench` | 返回新决策模型视图（DecisionPoint + 完成度 + 冲突） | `adm-new-tauri-commands/design.rs` |
| `update_node` | 接受 Selection（选项+参数+rationale+confirmed_by_user） | 同上 |
| `submit_ai_turn` | 支持整表提案 turn + 例外下钻 | `.../ai.rs` |
| `force_ai_output` | **审查/收窄**：不得成为 AI 代提交后门（R7） | 同上 |
| **新 `load_freeze_gate`** | 返回五道门状态 + 各 block 清单 | 新命令 |
| **新 `execute_freeze`** | 全绿则产 `FrozenDesign`（哈希、只读） | 新命令 |
| **新 逆向审核命令组** | `load_reverse_review / update_answer / return_answer / certify_template` | 新 `reverse.rs` |
| `load_pipeline_view` | 返回新 C0–C6 registry | `.../pipeline.rs` |

命令注册在 `desktop-tauri/src/lib.rs` 的 `tauri::generate_handler![...]`（~231 行）追加。

---

## 5. 不做的事（守住"轻改"）

- 不引入前端框架（保持原生 ES-module，与现有一致）。
- 不造可视化图/画布编辑器（现状就不是，00 号也不要求）。
- 不重写 pipeline 面板的运行/恢复/checkpoint 机制（复用）。
- 不动 patch/package/logs/sdk 面板（v3 不涉及）。

---

## 6. R 交付与验证

| 里程碑 | GUI 交付 | 验证 |
|--------|---------|------|
| R3 | `reverse-review.js` + 审核命令组 | 逆向过卷/退回可用 |
| R5 | `design.js` 节点重做 + `ai-interview.js` 分层确认 + `freeze-panel.js` | 手动走通塔防设计→换皮→冻结（配合文档 05） |
| R6/R7 | `pipeline.js` 套 C0–C6 + 人工门 UI | 跑通 C0–C6 双格式产物展示 + 两人工门 |

**验证方式**：以文档 05 的「手动模式走通一个塔防设计并冻结」为 GUI 端到端验收——UI 能完成选项选择、表格填写、完成度显示、五道门通过、冻结，即 R5 GUI 达标。
