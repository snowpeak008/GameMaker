# Python 解构评分卡

状态：第二轮评分通过。

合格规则：

- 单项 `>=90`
- 综合 `>=95`
- 无硬门禁失败
- `confidence != low`

## 第一轮评分项

| 角色 | 领域 | 分数 | 权重 | confidence | evidence | issues | required_action |
| --- | --- | ---: | ---: | --- | --- | --- | --- |
| Python Archaeologist | 可达性和垃圾隔离 | 94 | 15 | medium-high | 入口、registry、root 隔离、runtime/generated 分类已成型 | `tools/` 和 legacy UI 仍为 defer，未完全二次审计 | 优化垃圾隔离和工具分类说明 |
| Product Parity Reviewer | 功能复刻完整性 | 93 | 15 | medium-high | 六任务区、pipeline、design/export/save/AI/package 已覆盖 | patch/sdk/log 面板功能已有摘要但验收级细节不足 | 补面板级 UI 行为与功能验收矩阵 |
| Data Contract Architect | 数据流完整性 | 96 | 15 | high | save、execution object、artifact schema refs、AI schema、package manifest 已拆 | 少量 runtime output 样本未形成统一验证矩阵 | 补数据合同验收引用 |
| UI Reproduction Reviewer | UI 信息架构和互动路径 | 91 | 15 | medium | MainWindow、设计、pipeline、AI、package 交互链已追 | 未形成像素/主题/控件级复刻规格，容易低估 Web UI 工作量 | 回读 theme 和主要 panel layout，补 UI 复刻规格 |
| Rust Architecture Reviewer | Rust/Tauri 可实现性 | 95 | 10 | high | 已明确 Tauri + Web UI + Rust 后端职责边界 | 需要把 UI 直写文件改为后端 service 的边界再细化 | NEWrust 设计阶段重点落实 service/command 边界 |
| QA Release Reviewer | 测试与验收可执行性 | 94 | 15 | medium-high | pipeline gates、artifact validators、package validation 已拆 | 未形成 Python parity 到 NEWrust gate 的测试映射 | 补 parity/gate 测试矩阵 |
| Red Team Reviewer | 伪完成和证据矛盾 | 90 | 15 | medium | 已发现 embedded interview 缺 UCOS bridge、旧 Step15-17 不复刻 | 分数刚达单项门槛，仍有“文档覆盖多但验收证据不足”的风险 | 必须优化 UI/QA/garbage 后再重评 |

第一轮加权综合分：`93.2`。

第一轮结论：不合格。单项均 `>=90`，但综合 `<95`，且 UI/QA/Red Team 风险偏高。进入一轮优化。

## 第二轮评分项

优化证据：

- 新增 `19_ui_reproduction_specs.md`：补齐 theme token、字体、主窗体、设计工作台、AI 访谈、Pipeline、Patch、Package、Logs、SDK、AI Config 的布局和控件级复刻要求。
- 新增 `20_parity_gate_test_matrix.md`：补齐面板级功能验收、数据合同验收、gate 映射和 anti-fake completion rules。
- 更新 `18_garbage_isolation_draft.md`：补齐 `tools/` 二级分类，避免维护工具污染产品 UI。

| 角色 | 领域 | 分数 | 权重 | confidence | evidence | issues | required_action |
| --- | --- | ---: | ---: | --- | --- | --- | --- |
| Python Archaeologist | 可达性和垃圾隔离 | 96 | 15 | high | source authority、garbage isolation、tools 二级分类、root runtime/generated 分类 | 仍有 defer 项，但已明确不能进入核心复刻 | NEWrust 设计阶段保留引用链 |
| Product Parity Reviewer | 功能复刻完整性 | 96 | 15 | high | 六任务区、design/pipeline/patch/package/log/sdk/AI config、AI/packaging/save/export 全链路 | 未逐个实现验证，属于下一阶段任务 | 进入 NEWrust 详细设计 |
| Data Contract Architect | 数据流完整性 | 97 | 15 | high | project_state、save、execution object、artifact、AI schema、patch、SDK、log、package 合同矩阵 | 无硬门禁缺失 | 进入 typed Rust model 设计 |
| UI Reproduction Reviewer | UI 信息架构和互动路径 | 96 | 15 | high | UI reproduction specs 已含颜色、字体、布局、控件、状态和 Playwright 验收 | Tk 到 Web 的像素差异需在实现阶段截图校准 | NEWrust UI 设计必须引用 `19_*` |
| Rust Architecture Reviewer | Rust/Tauri 可实现性 | 96 | 10 | high | Rust 后端服务边界、Web UI 禁止直写、Tauri command 只桥接 | crate 边界需在 NEWrust 设计细化 | 进入 crate/service 设计 |
| QA Release Reviewer | 测试与验收可执行性 | 96 | 15 | high | parity/gate matrix、artifact/package/save/AI/UI gate 已明确 | 具体测试文件尚未创建 | 原子计划必须绑定 test command |
| Red Team Reviewer | 伪完成和证据矛盾 | 95 | 15 | high | 已记录 embedded UCOS bridge 差异、旧 Step15-17 排除、runtime data 只作样本 | 后续最大风险是开发阶段跳过 gate | 每阶段继续回读计划和 scorecard |

第二轮加权综合分：`96.0`。

第二轮结论：合格。单项均 `>=90`，综合 `>=95`，无硬门禁失败，confidence 均非 low。Python 解构阶段可进入 NEWrust 详细设计阶段。

## 防偏移记录

- 2026-07-08 stage=plan_v2_written; plan_reread=done; drift_detected=false; drift_action=none; next=source_authority_index.
- 2026-07-08 stage=source_authority_index_round1; plan_reread=done; drift_detected=false; drift_action=none; next=design_and_pipeline_deep_read.
- 2026-07-08 stage=ui_interaction_matrix_round1; plan_reread=done; drift_detected=false; drift_action=none; next=pipeline_plugin_contracts.
- 2026-07-08 stage=pipeline_plugin_contracts_round1; plan_reread=done; drift_detected=false; drift_action=none; next=design_engine_and_artifact_contracts.
- 2026-07-08 stage=design_engine_and_artifact_contracts_round1; plan_reread=done; drift_detected=false; drift_action=none; next=save_ai_config_log_panel_deep_read.
- 2026-07-08 stage=save_ai_log_structured_schema_refs_round1; plan_reread=done; drift_detected=false; drift_action=none; next=ai_interview_and_packaging_validation_deep_read.
- 2026-07-08 stage=ai_interview_packaging_garbage_round1; plan_reread=done; drift_detected=false; drift_action=none; next=first_multirole_score.
- 2026-07-08 stage=ui_qa_garbage_optimization_round1; plan_reread=done; drift_detected=false; drift_action=none; next=newrust_detailed_design.

## 评分准备判断

第二轮已通过，进入 NEWrust 详细设计。

上一轮缺口已补：

- `core/save/manager.py` 已拆到 manifest、index、lock、autosave、draft/workspace sync、snapshot、timeline、load/delete。
- `core/config` 与 `core/adapters` 已拆到 AI config v3、active profile、validator、adapter registry、Codex/OpenAI/Claude/completion adapter。
- `core/ui/log_panel.py` 已追到 `run_range()` JSONL 写入和最近 5 个 run log 读取。
- `core.design.structured_handoff` 已拆到 decisions/profile/archetype/contracts/traceability/manifest。
- artifact registry 的 `schema_refs` 已形成 `15_artifact_schema_refs_map.md`。

本轮已补充：

- `16_ai_interview_and_completion_contracts.md` 已拆到嵌入式访谈、独立访谈窗口、schema modes、Codex JSON 后端、高置信写回、mapping/summary 后台任务、framework memory、UCOS bridge、CompletionJsonService。
- `17_packaging_contracts.md` 已拆到 PackagePanel、Step14 UI 启禁、service 二次门禁、validation report、build report、PACKAGE_NOTES、package_manifest。
- `18_garbage_isolation_draft.md` 已把 `RUST/`、`NEWrust/`、`_trash/`、`_archive/`、`bug/`、runtime data、legacy workbench 等内容分为 authoritative/reference/defer/drop-candidate/generated-runtime-data/target-workspace。

后续阶段仍需确认：

- NEWrust 详细设计必须引用 Python 解构证据，不得凭空新增功能。
- 若设计评分发现任一角色 `<90` 或综合 `<95`，必须回读 Python 解构和 NEWrust 设计后优化。

结论：Python 解构评分门禁通过。
