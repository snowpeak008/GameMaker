# NEWrust v3 Full Project Reproduction Plan

日期：2026-07-10

状态：A00-A40 已全部完成；2026-07-10 已完成真实 Tauri 可用版闭环与便携发布验证。`final_handoff_v3.md` 与 `final-handoff-v3-gate` 已通过。此目录是新的全项目复刻计划入口，优先级高于既有 v2 `atomic_backlog` 开发计划。

## 0. 可用版交付闭环（2026-07-10）

- `desktop-tauri` 已接入真实 Tauri Builder、Web 前端和命令层，不再是仅编译脚手架。
- 项目可启动、可构建；正式存档与每窗口独立草稿可保存、切换、退出同步和重启恢复。存档提交现使用 OS 锁、项目级有界事务锁、完整 before-image journal 和同盘目录交换；创建、空白新建、同步、加载、重命名、删除均可在失败或崩溃后恢复。
- Step00-14 可完整运行，Step07 支持人工确认；界面可查看每步输出、错误、警告、制品列表和制品内容。
- AI 支持 Codex CLI、Claude CLI 和 OpenAI-compatible completion；长任务不占用桌面全局运行时锁。
- 应用界面已建立 `zh-CN` 纯中文与 `en-US` 纯英文两种可替换语言模式；当前通过 `ADM_NEWRUST_LANGUAGE` 选择，暂不提供可见入口。内置目录以稳定 ID 覆盖 16 个领域、103 个节点、515 条检查项、分组/选项、玩法系统、回退目录、项目画像、L4 缺失项和内建质量错误；只替换显示文本，不修改源 JSON、协议或存档。用户输入、日志、AI 原始响应和产物正文保持原文。
- 项目模板链路已从占位按钮补齐为真实可用功能：浏览器按元数据列出 25 个内置模板与当前草稿自定义模板；服务端按模板 ID 套用并移除访谈记录；自定义模板支持原子保存、明确覆盖、只删自定义、损坏文件隔离告警，以及随正式存档归档、恢复和重启继续使用。内置模板浏览信息按语言模式选择中文或英文显示。
- 打包页从当前 Step11-14 证据重建校验请求；缺少真实 Unity 审计时给出明确 blocker，不伪造通过。
- 最终便携目录：`NEWrust/dist/AutoDesignMaker-NEWrust/`；启动器：`Start-AutoDesignMaker.cmd`。
- 当前模板链路便携版 EXE SHA-256：`0e691d579d47783c85def909378d77c38d52295730daaed89b1325e5c8ba3d75`（21,350,912 bytes）。
- 已通过 Rust 全工作区格式/check/test，其中模板分层测试为 design 37、application 48、Tauri adapter 56、desktop 32，`adm-new-save` 35/35；Python 模板基线 7/7；Web unit/e2e、1655 项设计内容生成检查、2489 个双语对称键、两种语言各 12 个界面纯度门禁、56 个中英文宽窄屏截图及溢出/控件高度门禁、93 项基线门禁。release 构建、资源逐文件校验和真实窗口冒烟在本轮便携构建后刷新。
- 按用户边界，Unity 实际运行、真实 AI 效果和每步产物质量由用户在目标环境验收。

## 1. 目标重定义

用户目标不是“主产品链路高保真复刻”，而是：

```text
将整个 Python 项目复刻为由 Rust 代码开发的 NEWrust 项目。
```

因此，v3 计划不允许再只用功能域覆盖来证明完成。所有 Python 文件、配置、数据资产、测试、工具和运行时样本都必须被审计，并得到最终处置结论。

## 2. 与 v2 的关系

v2 已完成的 `plan-gate`、`parity-gate`、`ui-gate`、`package-gate`、`release-gate`、`handoff-report` 只能作为历史实现证据。

它们不能证明全项目复刻，因为 v2 的最终 handoff 只有 9 个功能域条目，无法覆盖 379 个 Python 文件。

从本计划启动后：

- 暂停继续按 v2 原子 backlog 开发新功能。
- 先完成全项目文件级解构。
- 文件级设计、评分、修改、再评分通过后，重新生成 v3 原子开发计划。
- 只有 v3 原子计划通过门禁后，才允许继续开发。

## 3. 强制完成定义

全项目复刻完成必须同时满足：

- Python 文件盘点覆盖率 = 100%。
- 每个 Python 文件有最终 disposition。
- `pending` / `partial` / `defer` / `unclassified` 数量 = 0。
- 每个 authoritative runtime 文件映射到 Rust crate、Web UI、CLI 或数据资产。
- 每个 tool 脚本映射到 Rust CLI / xtask / gate，或有明确 drop 理由。
- 每个 Python 测试映射到 Rust/Web/gate 测试，或有明确替代证明。
- 每个配置和 JSON/schema 数据资产有迁移、嵌入、加载或排除方案。
- Python Tk UI 与 Web/Tauri UI 有基线截图和差异审查。
- 最终 handoff 必须是文件级矩阵，不允许只有功能域矩阵。

## 4. 当前事实

初始机器盘点结果：

| 分类 | 数量 |
| --- | ---: |
| Python 文件总数 | 379 |
| `core/` | 245 |
| `tools/` | 55 |
| `pipeline/` | 50 |
| `knowledge/` Python | 26 |
| 根 Python 文件 | 3 |

完整清单见：

- `01_full_python_file_inventory.md`
- `11_data_asset_migration_matrix.md`

第二轮可达性索引结果：

| 指标 | 数量 |
| --- | ---: |
| Python 文件总数 | 379 |
| 入口候选 | 161 |
| 静态孤立候选 | 71 |
| registry 动态 stage module | 19 |
| `__main__` 命令入口文件 | 42 |
| `subprocess` / `Popen` 命中 | 98 |
| AST 解析错误 | 0 |

当前开闸状态：

- `03_file_disposition_matrix.md` 已完成 379/379 个 Python 文件最终裁决，`pending_final_decision=0`。
- `06_orphan_file_decision.md` 已完成 71/71 个静态孤立候选复核，`pending_final_decision=0`。
- Rust target mapping 已同步 379 个已裁决文件，`pending_file_mappings=0`。
- v3 深读已发现需新增或明确承载的 Rust 目标：`adm-new-config`、`adm-new-knowledge`。
- `.gitignore` 漏扫修正已纳入 `tools/build/*.py`，并排除 `_trash/`、`sandbox/`、`build/skill/` 等垃圾或构建产物目录。
- UI、tests、knowledge/ucos、pipeline、tools 与 `tools/build` 已完成逐文件裁决；第二轮评分已通过，`10_v3_atomic_development_plan.md` 已生成。
- A00 workspace/crate 对齐已完成：`adm-new-config`、`adm-new-knowledge` 已加入 `NEWrust` workspace，治理清单已同步，`cargo check --workspace` 已通过。
- A01 foundation 层复刻已完成：`sitecustomize.py`、`core/__init__.py`、`core/paths.py`、`core/io.py`、`core/utils/*` 的路径、pycache、session/draft、JSON/YAML/Markdown、文本提取、process env、BaseTool 兼容与 SHA-256 manifest 已落到 `adm-new-foundation`。
- A02 schema/contracts 层复刻已完成：`knowledge/schemas/**` 的 93 个 JSON schema 已由 `adm-new-contracts::schema` 注册，`tools/validators/contract_validator.py` 的轻量 schema 子集校验与 validation report writer 已迁移。
- A03 settings/config 层复刻已完成：`settings/**`、`core/config/*`、`tools/config/migrate_ai_config.py` 的 Python 兼容 snake_case AI config、legacy migration、app/project settings loader、OpenAI endpoint normalization、secret masking、integrity checks 已落到 `adm-new-config`，`adm-new-ai` 已改为通过 config facade 读写配置。
- A04 design data loader 层复刻已完成：`knowledge/design_data/**`、`core/design/data_loader.py`、`engine_data_loader.py` 和项目模板加载策略已落到 `adm-new-design::data_loader`，覆盖真实资产枚举、shared template 展开、entity schema 轻量验证、domain/gameplay 校验、template registry 排序与归档隔离。
- A05 knowledge/UCOS memory 层复刻已完成：`knowledge/ucos/**`、`core/adapters/memory/*`、`tools/memory/*` 已落到 `adm-new-knowledge` 与 `adm-new-cli memory/ucos`，覆盖 UCOS 库存、memory tiers、identity 约束、skill registry/discovery、context builder/token budget/formatters、freshness 快照、decision/planning/reflection/world model 诊断。
- A06 AI adapter 与 image 工具层复刻已完成：`core/adapters/*`、`core/ai_design/completion_service.py`、`tools/asset_production/image_api_config.py`、`image_api_probe.py`、`codex_image_tool.py`、`image_metadata_checker.py` 的 adapter registry、Codex/Claude CLI 命令规格、本地禁用适配器、OpenAI 兼容 completion transport、文件生成 task builder、结果摘要、结构化 JSON completion、image API settings/probe request、Responses/Images b64 解析、Codex image 命令和 PNG metadata gate 已落到 `adm-new-ai::adapters` 与 `adm-new-ai::image`。
- A07 design contract 层复刻已完成：`core/design/ai_schema.py`、`project_identity.py`、`project_dna.py`、`open_questions.py`、`playable_contracts.py` 已落到 `adm-new-ai::design_contracts` 与 `adm-new-design::contracts`，覆盖 AI response schema profile/required field/mode validation、project identity/customization score、open question normalization/blocker、project DNA seed/freeze/scenario contract、playable bundle generation/completeness validation、structured decisions bundle annotation 和 playable development task projection。
- A08 AI interview 状态机与 UCOS bridge 层复刻已完成：`core/design/ai_interview.py`、`ai_backend.py`、`ai_mapping_agent.py`、`ai_prompt_packer.py`、`ai_route_planner.py`、`ai_summary_agent.py`、`ai_ucos_bridge.py` 已落到 `adm-new-design::ai_interview`，覆盖访谈状态归一化、用户/助手消息、backend stage、MDA 进度、候选节点路由、prompt packing/预算降级/replay/meter、output partition prompt、mapping/summary payload 校验、Codex 后端任务规格/JSON event 解析和 UCOS episodic/short-term/semantic/design-generation 事件模型。
- A09 semantic pipeline 层复刻已完成：`core/design/requirements.py`、`program_capabilities.py`、`art_taxonomy.py`、`semantic_coverage.py`、`semantic_alignment.py`、`style_fit.py`、`task_semanticizer.py` 已落到 `adm-new-design::semantic_pipeline`，覆盖 archetype detection/catalog、archetype requirement nodes、requirement metadata、program capability contract/coverage report、art taxonomy/asset strategy matrix、semantic coverage seed、semantic alignment report/matrix、style fit report/acknowledgement、program/art task semantic enrichment 和 coverage matrix。
- A10 handoff/export/lint 层复刻已完成：`core/design/exporter.py`、`export_adapter.py`、`structured_context.py`、`structured_handoff.py`、`cross_layer_lint.py` 已落到 `adm-new-design::handoff`，覆盖 export payload/preview/render/sidecar、D4 Concept/GameplayFramework/Design source package、structured decisions/profile/archetype/traceability、playable contract candidates、handoff manifest validation、Stage02 优先/D4 候选兜底 structured context 和 profile-to-option cross-layer lint。
- A11 save manager 层复刻并加固已完成：`core/save/manager.py`、`tools/save/repair_blank_save_progress.py`、`audit_parallel_isolation.py`、`repair_parallel_save_contamination.py` 已落到 `adm-new-save` 与 `adm-new-application`；覆盖 blank/iteration save、Python verified `design_project` 只读回退、OS archive/index lock、每窗口 draft lease、项目级有界事务串行、完整 before-image journal、跨会话启动/运行期恢复、rename rollback、delete tombstone、file map SHA-256、full/delta snapshot、blank progress repair 和 parallel isolation audit/repair。
- A12 runtime 层复刻已完成：`core/runtime/control.py`、`execution_config.py`、`execution_planner.py`、`execution_state.py`、`guard.py`、`locks.py`、`pipeline_state.py`、`preflight.py`、`run_context.py` 已落到 `adm-new-application::runtime`，覆盖 stop/run state 控制文件、runtime locks、run context/settings snapshot、execution config/planner/state、actual development preflight、pipeline_state.md 和 forbidden runtime guard。
- A13 source 层复刻已完成：`core/source/finder.py`、`folder_manager.py`、`groups.py`、`importer.py`、`snapshot.py`、`__init__.py` 已落到 `adm-new-pipeline::source`，覆盖 source roots/run-context precedence、source package metadata/markers、latest/all discovery、source group import、stage artifact reset、upstream reference import、reference_manifest、temporary correction folder merge、snapshot manifest/restore。
- A14 artifact 层复刻已完成：`core/artifact/graph.py`、`manifest.py`、`preflight.py`、`registry_loader.py`、`reviewer.py`、`validator.py`、`__init__.py` 已落到 `adm-new-artifact`，覆盖 registry loader、dependency graph/topological step order、versioned artifact manifest helpers、file-backed preflight/review/validation layer outputs、schema contract validation、artifact_layer_manifest/artifact_reviews/artifact_validation_layer 写出和 reference_manifest inventory refresh。
- A15 art pipeline 层复刻已完成：`core/art_pipeline/paths.py`、`stage04.py`、`stage09.py`、`stage12.py`、`stage13.py`、`stage14.py` 已落到 `adm-new-design::art_pipeline`，覆盖 Assets/AutoDesign 路径政策、Stage04 可消费资产规格、Stage09 art task prompt enrichment、Stage12 raw/processed asset manifests、quality/semantic review/rework queue/art handoff、Stage13 Unity editor/materialization reports 和 Stage14 art/visual/design coverage acceptance helpers；art pipeline 输出已用真实 schema 做单元校验。
- A16 execution objects 层复刻已完成：`core/engines/execution_objects/*` 已落到 `adm-new-application::execution_objects`，覆盖 EO store/type registry/workflow 状态机、确认等级门禁、漂移/冲突检查、失败重试/人工与自动修复、program/art/user/workspace/design object helpers、correction queue 和 unattended recovery。
- A17 generation 层复刻已完成：`core/engines/generation.py` 的通用生成合同、设计源解析、Concept/Design source loading、structured handoff fallback、stage report/index refresh 和可插拔 stage output writer 已落到 `adm-new-pipeline::generation`；`source_context.py` 与 `handoff_loader.py` 的 source package/latest handoff/read-write/load helpers 已同步迁移。
- A18 pipeline Step00-02 层复刻已完成：`pipeline/step_00_idea_intake/*`、`step_01_gameplay_framework/*`、`step_02_design_review_freeze/*` 已落到 `adm-new-pipeline::stages::step00_02`，覆盖 Step00/01/02 plugin source group 规格、核心问题覆盖、genre/template fallback、玩法循环与系统定义、L5 实体抽取、expected node coverage、AI supplement cache/fallback、entity graph、phase classification 和 frozen design 输出。
- A19 pipeline Step03-06 层复刻已完成：`pipeline/step_03_program_requirements/*`、`step_04_art_requirements/*`、`step_05_program_review/*`、`step_06_art_review/*` 已落到 `adm-new-pipeline::stages::step03_06`，覆盖 Step03/04/05/06 plugin source group 规格、L5 entity 到 program requirement 多模板转换、system binding、standard program requirements contract、entity 到 art asset 多资产展开、market local fallback、asset spec gate、standard art requirements contract、program/art review severity code 与 PASS/WARN/FAIL/BLOCKED verdict。
- A20 pipeline Step07 style generation 层复刻已完成：`pipeline/step_07_art_style_generation/*`、`core/ui/style_confirmation_dialog.py`、`core/ui/style_prompt_editor.py` 和 pipeline panel style confirmation 合约已落到 `adm-new-pipeline::stages::step07`、`adm-new-application`、`adm-new-tauri-commands` 与 Web pipeline request builder，覆盖 Step07 无 source group wrapper、风格选项生成、确定性 PNG placeholder、manual confirmation waiting/resume、approved style application contract、prompt override rerun、style prompt response parsing、未选图片清理和 app/Web style selection 状态回写。
- A21 pipeline Step08-14 层复刻已完成：`pipeline/step_08_design_to_plan/*` through `pipeline/step_14_integration_validation/*` 已落到 `adm-new-pipeline::stages::step08_14`，覆盖 Step08 program plan、Step09 art plan、Step10 alignment、Step11 dev execution、Step12 art production handoff、Step13 scene assembly、Step14 integration validation wrapper/output contracts，以及 standalone metadata、completed_with_review blocker、structured input 缺失阻断和 acceptance report 兼容逻辑。
- A22 design_flow D1-D4 层复刻已完成：`pipeline/_design_base.py`、`pipeline/step_d1_project_portrait/*` through `pipeline/step_d4_devflow_handoff/*` 已落到 `adm-new-pipeline::design_flow`，覆盖 D1 project portrait、D2 design domains/decision report、D3 design gate/profile/contract blockers、D4 concept package export/structured handoff blocker 传播、test_mode 成功语义、autosave/project state fallback 和 D1-D4 stage/plugin specs。
- A23 patch 层复刻已完成：`core/patch/*`、`tools/patch/manager.py` 已落到 `adm-new-patch`、`adm-new-contracts::patch` 与 `adm-new-cli patch`，覆盖 patch record/store manifest、deterministic analyzer、route/stage validation、light validator、executor apply/validate/promote 生命周期、Codex runner expected-file sandbox 边界、approved context 和 CLI analyze/list/show/validate/apply/promote 命令。
- A24 SDK 层复刻已完成：`core/sdk/*`、`tools/sdk/manager.py`、`knowledge/sdks/**` 已落到 `adm-new-sdk` 与 `adm-new-cli sdk`，覆盖 file-backed SDK index/spec/template store、review status、approved-only prompt context、HTML readable extraction、AI completion spec extraction pending-review 语义、Tauri/application 兼容 service 和 CLI init/list/show/add/review/context/sync 命令。
- A25 packaging/build 层复刻已完成：`core/packaging/*`、`tools/build/build.py`、`tools/build/verify_build.py` 与 build spec 资产已落到 `adm-new-packaging` 与 `adm-new-cli package/dist`，覆盖 Step14 source 文件加载、package validation/build report/notes/manifest 写出、package CLI 成功/阻断退出码、Rust release build plan 和 bundle executable/required-item verifier。
- A26 asset/image 工具层复刻已完成：`tools/asset_production/*` 已落到 `adm-new-artifact::asset_tools`、`adm-new-ai::image` 与 `adm-new-cli asset/image`，覆盖 audio/SFX 静音 WAV 占位、image API probe request/Codex image command/PNG metadata check、sprite sheet slice、sprite atlas metadata、localization manager 生成与 C# 中文硬编码注入。
- A27 dev/Unity/codegen/validator 工具层复刻已完成：`tools/dev/*` 与 Unity compile/environment validators 已落到 `adm-new-application::dev_tools` 与 `adm-new-cli dev/project/pipeline/unity/validate`，覆盖 CSV/schema 配置编译、ErrorLogger/PerfMonitor/PerfHUD/UIManager 生成、生命周期方法注入、项目 scaffold、pipeline step scaffold、Unity NUnit 占位测试生成、git 本地命令白名单、Unity compile plan/result evaluation 和 environment check。
- A28 scripts/migration/governance 工具层复刻已完成：`tools/scripts/*` 已落到 `adm-new-application::migration_tools` 与 `adm-new-cli migrate/schema/governance/pipeline/design`，覆盖 hardcoded legacy path scan、legacy copy dry-run/apply report、execution-object save_id backfill、design project migration to execution objects、pipeline validation report inspection、schema migration helper 和 D4-compatible concept package export/mirror。
- A29 validators/semantic/pipeline quality 工具层复刻已完成：`tools/validators/*` 已落到 `adm-new-application::validation_tools`、`adm-new-cli validate` 与 `adm-new-governance validation-gate`，覆盖 config/context/contract/output validators、pipeline quality PLAN-002 gate、design semantic quality metrics/report 写出和治理 marker gate。
- A30 iteration 工具层复刻已完成：`core/iteration/*` 已落到 `adm-new-application::iteration`、`adm-new-cli iteration` 与 `adm-new-governance iteration-gate`，覆盖 iteration spec parser/discovery、conservative delta scheduler、skipped artifact inheritance、prepare dry-run/formal iteration save、resume summary 和 marker gate。
- A31 Tauri desktop shell + Web theme 复刻已完成：`core/ui/theme.py`、`main_window.py`、`gui_app.py` 的 shell/theme/startup 契约已落到 `desktop-tauri`、`adm-new-tauri-commands::shell`、Web `theme.js` 与 `ui-shell-gate`，覆盖窗口尺寸/居中策略、theme tokens、startup/auto-restore/exit cleanup 状态、Web shell marker、build 和截图 gate。
- A32 Web design workbench + Tauri design commands 复刻已完成：`core/ui/app_window.py` 的 16-domain workbench、project state、模板选择/另存、L4/L5、gameplay systems、autosave/export/reset 契约已落到 `adm-new-design::DesignWorkbenchView`、`adm-new-application::DesignWorkbenchService`、`adm-new-tauri-commands::design`、Web `features/design.js` 与 `ui-workbench-gate`；2026-07-10 复核发现原“查看模板”仍是占位处理，现已补齐轻量列表、服务端按 ID 套用、原子自定义保存/删除、草稿与正式存档恢复、双语浏览器和真实点击门禁。
- A33 Web AI interview components 复刻已完成：`core/ui/ai_interview_window.py`、`embedded_interview.py`、`bottom_panel.py` 的 AI 访谈窗口、内嵌访谈面板、stream/background 状态和 bottom log/AI tab 契约已落到 `adm-new-tauri-commands::ai`、Web `features/ai-interview.js`、`index.html`、`styles.css` 与 `ui-ai-gate`；覆盖 shared `AiInterviewController`、stream/background normalizers、Tauri command view、Web AI runtime markers、bottom AI tab、build 和截图 gate。
- A34 Web pipeline page 复刻已完成：`core/ui/pipeline_panel.py`、`pipeline_step_card.py`、`semantic_quality_panel.py` 的 Step00-14 树、区间运行/停止、Step07 风格确认、blocked/completed_with_review 状态和语义质量返回路径契约已落到 `adm-new-tauri-commands::pipeline`、Web `features/pipeline.js`、pipeline fixture/test/CSS 与 `ui-pipeline-gate`；覆盖 `PipelineSemanticQualityView`、`PipelineIssueReturnView`、Web `normalizeSemanticQuality`、semantic-quality panel、full Step00-14 tree、skip manual gate request、build 和截图 gate。
- A35 Web utility panels 复刻已完成：`core/ui/patch_panel.py`、`package_panel.py`、`sdk_panel.py`、`save_manager_dialog.py` 的 patch/package/SDK/save workflow 与存档管理对话框契约已落到 Web `features/utility-panels.js`、`index.html`、`styles.css`、`adm-new-tauri-commands::save` 适配、`ui-utility-gate` 与截图 gate；存档 UI 现明确区分新建项目/另存副本/保存当前，加载提供保存/放弃/取消三选，显示草稿、锁、完整性、设计/流水线进度、事务、文件数/大小和路径，支持 corrupt/locked/error/busy 状态、键盘选择、打开目录和中英文宽窄屏无溢出门禁。
- A36 Web settings/style modals 复刻已完成：`core/ui/ai_config_unified_dialog.py`、`unity_config_dialog.py`、`style_confirmation_dialog.py`、`style_prompt_editor.py` 的 AI 配置、项目引擎/路径 preflight、Step07 风格确认与提示词编辑契约已落到 Web `features/settings-style.js`、pipeline prompt editor action、`index.html`、`styles.css`、`adm-new-tauri-commands::config` 项目配置/preflight 适配、`ui-settings-style-gate` 与截图 gate；覆盖 project_config/style_prompt_editor 截图证据、style prompt parser/override request、ProjectRuntimeSettings request builders、build 和 workspace gate。
- A37 UI parity v3 baseline records 复刻已完成：`07_ui_python_baseline_plan.md` 的 required surfaces 已落到 `ui-baseline-gate`、`ui-parity-v3-gate` 与 `plan/NEWrust/full_project_reproduction/ui_baselines/`；覆盖 93 个 required state record、92 个 screenshot-required state 的 desktop/narrow Web 截图、93 个 Python baseline/manual note、interaction trace、command contract、difference table，且无未批准 P0/P1 delta。
- A38 `core\tests\unit\*` Python unit assertions migration 已完成：新增 `unit-test-migration-gate`，从 `09_test_migration_matrix.md` 与真实 `core/tests/unit` 目录核对 68/68 个 Python unit test 文件，检查 decided 状态、test gate、target/evidence、目标域、56 个 Rust/Web/CLI/gate 迁移 marker 和 ignored-test 禁止项；CLI gate 报告已通过并写出。
- A39 `core\tests\integration\*` / `conftest.py` integration gate migration 已完成：新增 `integration-test-migration-gate`，从 `09_test_migration_matrix.md` 与真实 `core/tests/integration`、`core/tests/conftest.py` 核对 5/5 个 Python integration/fixture test 文件，检查 decided 状态、test gate、target/evidence、目标域、18 个 adapter/semantic pipeline/parallel isolation/plugin/fixture/gate 迁移 marker 和 ignored-test 禁止项；CLI gate 报告已通过并写出。
- A40 final handoff evidence 已完成：新增 `final-handoff-v3-gate` 与 `plan/NEWrust/full_project_reproduction/final_handoff_v3.md`，核对 379/379 Python inventory、379/379 final disposition、379/379 Rust target mapping、73/73 test migration、727 data assets、93 UI baseline records、A00-A39 completed atoms 和 16 个关键 gate log；`release-gate` 与 `final-handoff-v3-gate` 均已刷新通过。
- v3 原子开发计划 A00-A40 已全部完成。
- `11_data_asset_migration_matrix.md` 已补充 Python 文件矩阵之外的 727 个 knowledge/settings 资产处理规则。

## 5. 阶段顺序

1. P0 全量文件盘点。
2. P1 import/入口/调用图解构。
3. P2 文件级 disposition 裁决。
4. P3 Rust 目标架构映射。
5. P4 UI 基线和像素级复刻方案。
6. P5 测试、工具、数据资产迁移方案。
7. P6 多角色评分。
8. P7 修改计划并重新评分，直到满足门禁。
9. P8 生成 v3 原子开发计划。
10. P9 按 v3 原子计划进入开发。

## 6. 防偏移记录

```text
plan_reread=done
drift_detected=false
drift_action=none_after_A40_final_file_level_handoff_gate
```
