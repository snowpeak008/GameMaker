# v3 Full Project Reproduction Scorecard

状态：第二轮评分通过；v3 原子开发计划已生成，开发已从 A00 开始。

## 1. 合格规则

- 单项评分 `>=90`
- 综合加权评分 `>=95`
- 无硬门禁失败
- `confidence != low`
- 未裁决 Python 文件数必须为 0，否则综合分上限为 89

## 2. 第一轮评分

| 角色 | 领域 | 分数 | 权重 | confidence | evidence | issues | required_action |
| --- | --- | ---: | ---: | --- | --- | --- | --- |
| Python Whole-Project Auditor | 是否覆盖全部 Python 文件 | 88 | 15 | high | 已生成并修正为 379 个 Python 文件机器清单 | 仍是初始 disposition，未完成最终裁决 | 完成 `03_file_disposition_matrix.md` |
| Reachability Analyst | 入口/import/动态加载追踪 | 82 | 15 | medium | 旧计划覆盖主入口和 pipeline registry | tools、ucos、tests、动态 import 未全追踪 | 建立 import/call graph |
| Rust Migration Architect | Rust 目标映射 | 86 | 15 | medium | v2 已有部分 crates 和 handoff | 仅 9 个功能域，非文件级 Rust 目标 | 建立 `04_rust_target_mapping.md` |
| Tooling and Build Reviewer | tools/build/test helper 覆盖 | 78 | 10 | medium | 已修正统计 `tools/` 55 个 py | 尚未逐脚本裁决 | tools 逐文件 CLI/gate/drop 裁决 |
| UI Pixel Parity Reviewer | Tk/Web 像素复刻可证性 | 84 | 15 | medium | v2 有 Web screenshot gate | 缺 Python Tk baseline 和差异审查 | 建立 Python/UI baseline gate |
| QA Gate Reviewer | 测试/gate/handoff 文件级证明 | 86 | 15 | high | v2 release/handoff gate 已通过 | handoff 只有 9 个功能域 | final handoff 改为文件级 |
| Red Team Reviewer | 伪全量风险 | 75 | 15 | high | 当前对话已发现范围错误 | v2 容易再次以主链路冒充全项目 | 开发暂停，v3 计划通过前禁止新增业务开发 |

第一轮加权综合分：`82.7`。

第一轮结论：不合格。硬门禁失败：存在未最终裁决 Python 文件，综合分上限为 89。

## 3. 第一轮修正动作

必须新增并完成：

- `03_file_disposition_matrix.md`
- `04_rust_target_mapping.md`
- `06_import_and_reachability_graph.md`
- `07_ui_python_baseline_plan.md`
- `08_tooling_migration_matrix.md`
- `09_test_migration_matrix.md`
- `10_v3_atomic_development_plan.md`
- `11_data_asset_migration_matrix.md`
- `11_data_asset_migration_matrix.md`

## 4. 防偏移记录

```text
plan_reread=done
drift_detected=true
drift_action=first_round_failed_due_to_feature_domain_scope_not_full_project_scope
next=complete_file_level_disposition_matrix
```

## 5. 第一轮修正后状态

已补齐缺失文档入口：

- `03_file_disposition_matrix.md`
- `04_rust_target_mapping.md`
- `06_import_and_reachability_graph.md`
- `06_import_graph_static.md`
- `07_ui_python_baseline_plan.md`
- `08_tooling_migration_matrix.md`
- `09_test_migration_matrix.md`
- `10_v3_atomic_development_plan.md`

修正后仍不合格，因为孤儿可达性裁决与多角色评分尚未完成。当前 `03_file_disposition_matrix.md` 已包含 379 个 Python 文件，已完成最终裁决 379 个，`pending_final_decision=0`。

第二批深读裁决已覆盖核心启动/基础设施层：

- `core\main.py`
- `core\context.py`
- `core\io.py`
- `core\paths.py`
- `core\plugin_manager.py`
- `core\registry.py`
- `core\skill_loader.py`
- `core\stage.py`
- `core\stage_plugin.py`

第三批深读裁决已覆盖 AI adapter 层：

- `core\adapters\base.py`
- `core\adapters\registry.py`
- `core\adapters\completion_adapter.py`
- `core\adapters\openai_adapter.py`
- `core\adapters\codex_adapter.py`
- `core\adapters\codex\executor.py`
- `core\adapters\codex\file_guard.py`
- `core\adapters\codex\task_builder.py`
- `core\adapters\codex\result_parser.py`
- `core\adapters\claude_code_model_adapter.py`
- `core\adapters\local_adapter.py`
- `core\adapters\memory\context_builder.py`
- `core\adapters\memory\token_budget.py`
- `core\adapters\__init__.py`
- `core\adapters\codex\__init__.py`
- `core\adapters\memory\__init__.py`
- `core\adapters\claude_code_adapter.py`

第四批深读裁决已覆盖 artifact/config gate 层：

- `core\artifact\graph.py`
- `core\artifact\manifest.py`
- `core\artifact\preflight.py`
- `core\artifact\registry_loader.py`
- `core\artifact\reviewer.py`
- `core\artifact\validator.py`
- `core\artifact\__init__.py`
- `core\config\ai_config.py`
- `core\config\ai_config_schema.py`
- `core\config\integrity.py`
- `core\config\loader.py`
- `core\config\validator.py`
- `core\config\__init__.py`

第四批后架构修正：

- `04_rust_target_mapping.md` 已从骨架更新为 45 个已裁决文件的文件级映射。
- v3 计划新增或明确承载 `adm-new-config`、`adm-new-knowledge`，用于配置系统与知识/UCOS memory bridge，不再把这些行为塞进泛化 runtime。

第五批深读裁决已覆盖运行骨架/源包/打包/补丁/SDK 层：

- `core\runtime\control.py`
- `core\runtime\execution_config.py`
- `core\runtime\execution_planner.py`
- `core\runtime\execution_state.py`
- `core\runtime\guard.py`
- `core\runtime\locks.py`
- `core\runtime\pipeline_state.py`
- `core\runtime\preflight.py`
- `core\runtime\run_context.py`
- `core\runtime\__init__.py`
- `core\packaging\manifest.py`
- `core\packaging\service.py`
- `core\packaging\validation.py`
- `core\packaging\__init__.py`
- `core\patch\analyzer.py`
- `core\patch\codex_runner.py`
- `core\patch\executor.py`
- `core\patch\light_validator.py`
- `core\patch\record.py`
- `core\patch\__init__.py`
- `core\sdk\ai_extractor.py`
- `core\sdk\knowledge_base.py`
- `core\sdk\__init__.py`
- `core\source\finder.py`
- `core\source\folder_manager.py`
- `core\source\groups.py`
- `core\source\importer.py`
- `core\source\snapshot.py`
- `core\source\__init__.py`

第五批后仍不合格；`core\save\manager.py` 是大型核心模块，必须单独深读后再裁决。

第六批深读裁决已覆盖 save 模块：

- `core\save\manager.py`
- `core\save\__init__.py`

`core\save\manager.py` 必须复刻为 `adm-new-save::manager`，覆盖 active draft/formal archive/workspace、save index、项目 ID 迁移、草稿清理、锁、文件地图、快照、执行对象 store 所有权转移、同步重试、创建/加载/删除存档等行为。

第七批深读裁决已覆盖 iteration/utils 层：

- `core\iteration\artifact_inheritor.py`
- `core\iteration\delta_scheduler.py`
- `core\iteration\spec_parser.py`
- `core\iteration\__init__.py`
- `core\utils\base_tool.py`
- `core\utils\md_parser.py`
- `core\utils\process_utils.py`
- `core\utils\structured_md.py`
- `core\utils\text_extractor.py`
- `core\utils\yaml_compat.py`
- `core\utils\__init__.py`

第八批深读裁决已覆盖 AI design contract 层：

- `core\ai_design\__init__.py`
- `core\ai_design\types.py`
- `core\ai_design\completion_service.py`
- `core\ai_design\asset_spec_gate.py`
- `core\ai_design\traceability.py`
- `core\ai_design\prompt_library.py`
- `core\ai_design\contract_gate.py`

`core\ai_design` 不是垃圾内容，必须复刻为 `adm-new-ai` 的设计合约与 gate 子系统，覆盖 completion JSON 抽取/重试、AI 设计提示词模板、playable contract 注册表、资产规格约束和 traceability 任务字段校验。

第九批深读裁决已覆盖 core/design 基础设计契约层：

- `core\design\__init__.py`
- `core\design\ai_llm_backend.py`
- `core\design\ai_schema.py`
- `core\design\node_role.py`
- `core\design\profile_schema.py`
- `core\design\open_questions.py`
- `core\design\project_identity.py`
- `core\design\project_dna.py`
- `core\design\semantic_coverage.py`
- `core\design\semantic_alignment.py`
- `core\design\style_fit.py`
- `core\design\art_taxonomy.py`
- `core\design\program_capabilities.py`
- `core\design\task_semanticizer.py`

该批次确认 `adm-new-design` 必须承载项目身份/Project DNA、AI response schema、开放问题、风格适配、程序能力、资产策略、语义覆盖和任务语义化，不能只用最终导出文件替代这些中间设计契约。

第十批深读裁决已覆盖 core/design 结构化交接、模板、需求与 AI interview 辅助层：

- `core\design\engine_data_loader.py`
- `core\design\entity_schema.py`
- `core\design\structured_context.py`
- `core\design\structured_handoff.py`
- `core\design\cross_layer_lint.py`
- `core\design\project_templates.py`
- `core\design\requirements.py`
- `core\design\ai_mapping_agent.py`
- `core\design\ai_memory_retriever.py`
- `core\design\ai_prompt_packer.py`
- `core\design\ai_route_planner.py`
- `core\design\ai_summary_agent.py`
- `core\design\option_mapping.py`

该批次确认 Rust 侧必须复刻 D4 structured handoff 写入、entity schema 轻量验证、project template 内置/自定义目录策略、archetype requirements 检测、AI interview prompt packing/route/mapping/summary 校验，以及 option mapping 的 JSON/Markdown 生成器。

第十一批深读裁决已覆盖 core/design 大型运行模块：

- `core\design\ai_backend.py`
- `core\design\ai_interview.py`
- `core\design\ai_ucos_bridge.py`
- `core\design\ai_validator.py`
- `core\design\data_loader.py`
- `core\design\engine.py`
- `core\design\export_adapter.py`
- `core\design\exporter.py`
- `core\design\framework_memory.py`
- `core\design\gameplay_systems.py`
- `core\design\playable_contracts.py`
- `core\design\prompt_evaluation.py`
- `core\design\prompt_framework.py`

该批次确认 `core/design` 是全项目复刻的主设计引擎层：Rust 侧必须保留 Codex CLI 只读后端、AI 访谈状态机、Project state 高置信合并、UCOS 写入桥、设计数据加载校验、DesignEngine 状态/质量计算、DevFlow source package 导出、完整导出渲染、可玩合约生成、prompt framework 版本治理、framework memory 聚合/回滚和 prompt evaluation gate。

第十二批深读裁决已覆盖 core/art_pipeline 美术资产管线：

- `core\art_pipeline\__init__.py`
- `core\art_pipeline\paths.py`
- `core\art_pipeline\stage04.py`
- `core\art_pipeline\stage09.py`
- `core\art_pipeline\stage12.py`
- `core\art_pipeline\stage13.py`
- `core\art_pipeline\stage14.py`

该批次确认 `core/art_pipeline` 被 `core\engines\generation.py` 直接使用，必须复刻为 `adm-new-design::art_pipeline`，覆盖 Assets/AutoDesign 路径政策、资产策略到可消费规格、art task prompt enrichment、Stage12 质量/语义 review、Unity import/materialization request、art handoff 和 Stage14 视觉/设计覆盖验收。

第十三批深读裁决已覆盖 core/engines 生成编排与 execution object 层：

- `core\engines\__init__.py`
- `core\engines\source_context.py`
- `core\engines\handoff_loader.py`
- `core\engines\generation.py`
- `core\engines\execution_objects\__init__.py`
- `core\engines\execution_objects\paths.py`
- `core\engines\execution_objects\type_registry.py`
- `core\engines\execution_objects\workflow.py`
- `core\engines\execution_objects\integration.py`
- `core\engines\execution_objects\correction_queue.py`
- `core\engines\execution_objects\unattended_recovery.py`
- `core\engines\execution_objects\design_project.py`
- `core\engines\execution_objects\user_artifact.py`
- `core\engines\execution_objects\workspace_snapshot.py`

该批次确认 `core/engines` 是全项目复刻的核心运行编排层：Rust 侧必须拆分并复刻 Step00-Step14 stage output contract、structured handoff/source context、程序/美术/场景/集成 execution object 状态机、确认等级、漂移/并发写范围检测、失败记录、自动修复、纠错队列、恢复游标、工作区快照、设计项目版本和用户导出制品。`generation.py` 不能被压缩成简单顺序脚本，必须在 `adm-new-pipeline`、`adm-new-application`、`adm-new-design`、`adm-new-artifact` 多 crate 中保留阶段产物、路径门禁、Unity 执行、场景物化和验收闭环。

第十四批深读裁决已覆盖 core/ui Tk 桌面界面、工作台和 UI 驱动的运行门面：

- `core\ui\__init__.py`
- `core\ui\ai_config_unified_dialog.py`
- `core\ui\ai_interview_window.py`
- `core\ui\app_window.py`
- `core\ui\bottom_panel.py`
- `core\ui\embedded_interview.py`
- `core\ui\log_entry.py`
- `core\ui\log_panel.py`
- `core\ui\main_window.py`
- `core\ui\package_panel.py`
- `core\ui\patch_panel.py`
- `core\ui\pipeline_panel.py`
- `core\ui\pipeline_step_card.py`
- `core\ui\save_manager_dialog.py`
- `core\ui\sdk_panel.py`
- `core\ui\semantic_quality_panel.py`
- `core\ui\style_confirmation_dialog.py`
- `core\ui\style_prompt_editor.py`
- `core\ui\theme.py`
- `core\ui\unity_config_dialog.py`
- `core\ui\workbench.py`

该批次确认 `core/ui` 不是单纯外观层：`main_window.py` 是 Tauri/Web app shell 和关闭/状态/锁释放生命周期；`app_window.py` 是 16 领域设计工作台、L4/L5、玩法系统、模板、导出、存档和自动保存主界面；`ai_interview_window.py` 与 `embedded_interview.py` 必须共用 AI interview controller，覆盖 Codex CLI 状态、分片输出、payload 校验、访谈存档、后台 mapping 和 summary correction；`pipeline_panel.py` 必须复刻 Step00-Step14 状态树、区间运行、停止、语义质量返回路径、D4 导出、Step07 风格确认和提示词覆盖重跑；`workbench.py` 必须拆分到 application facade、source package writer、Tauri workbench commands 和 CLI self-test，不能作为 Tk 辅助文件丢弃。UI 像素复刻的后续 P4 必须以这些视图的 Python Tk baseline 截图为基线，而不是只按 Rust 现有界面自查。

第十五批深读裁决已覆盖 core/tests 质量证明层：

- `core\tests\integration\test_adapter_configuration.py`
- `core\tests\integration\test_design_semantic_pipeline.py`
- `core\tests\integration\test_parallel_project_semantic_isolation.py`
- `core\tests\integration\test_plugins.py`
- `core\tests\unit\test_ai_config.py`
- `core\tests\unit\test_ai_design_asset_spec.py`
- `core\tests\unit\test_ai_design_completion_service.py`
- `core\tests\unit\test_ai_design_contracts.py`
- `core\tests\unit\test_art_task_semanticization.py`
- `core\tests\unit\test_art_taxonomy_builder.py`
- `core\tests\unit\test_artifact_registry_playable_chain.py`
- `core\tests\unit\test_artifact_validator_paths.py`
- `core\tests\unit\test_codex_image_tool.py`
- `core\tests\unit\test_config_loader.py`
- `core\tests\unit\test_config_validator.py`
- `core\tests\unit\test_core_paths.py`
- `core\tests\unit\test_customization_scorer.py`
- `core\tests\unit\test_d2_real_decision_report.py`
- `core\tests\unit\test_d3_design_gate.py`
- `core\tests\unit\test_design_node_requirement_metadata.py`
- `core\tests\unit\test_design_requirements_archetype_detector.py`
- `core\tests\unit\test_design_requirements_archetype_subtypes.py`
- `core\tests\unit\test_design_semantic_quality.py`
- `core\tests\unit\test_design_semantic_schema_registry.py`
- `core\tests\unit\test_draft_archive_paths.py`
- `core\tests\unit\test_execution_planner.py`
- `core\tests\unit\test_hades_quality_optimization.py`
- `core\tests\unit\test_iteration_cli.py`
- `core\tests\unit\test_iteration_development.py`
- `core\tests\unit\test_l5_supplement.py`
- `core\tests\unit\test_manual_style_confirmation.py`
- `core\tests\unit\test_model_adapters.py`
- `core\tests\unit\test_open_questions_contract.py`
- `core\tests\unit\test_parallel_runtime_isolation.py`
- `core\tests\unit\test_patch_channel.py`
- `core\tests\unit\test_pipeline_optimization_helpers.py`
- `core\tests\unit\test_pipeline_registry_schema_generation_contracts.py`
- `core\tests\unit\test_playable_contracts.py`
- `core\tests\unit\test_program_capability_builder.py`
- `core\tests\unit\test_project_dna_builder.py`
- `core\tests\unit\test_project_templates.py`
- `core\tests\unit\test_pytest_config.py`
- `core\tests\unit\test_reference_manifest_refresh.py`
- `core\tests\unit\test_run_state_failure.py`
- `core\tests\unit\test_sdk_knowledge_base.py`
- `core\tests\unit\test_semantic_alignment.py`
- `core\tests\unit\test_stage11_parent_reuse_parallel.py`
- `core\tests\unit\test_step00_project_identity.py`
- `core\tests\unit\test_step00_structured_profile_input.py`
- `core\tests\unit\test_step01_structured_gameplay_framework.py`
- `core\tests\unit\test_step02_freezes_playable_contracts.py`
- `core\tests\unit\test_step02_project_dna_freeze.py`
- `core\tests\unit\test_step03_program_requirements_contract_schema.py`
- `core\tests\unit\test_step03_program_requirements_from_contracts.py`
- `core\tests\unit\test_step04_asset_requirements_from_contracts.py`
- `core\tests\unit\test_step05_optimization.py`
- `core\tests\unit\test_step05_to_step09_structured_contract_chain.py`
- `core\tests\unit\test_step08_program_plan_from_playable_contracts.py`
- `core\tests\unit\test_step10_to_step12_structured_contract_chain.py`
- `core\tests\unit\test_step11_eo_state_closure.py`
- `core\tests\unit\test_step13_requires_scene_and_ui_contracts.py`
- `core\tests\unit\test_step14_playable_acceptance_contract.py`
- `core\tests\unit\test_structured_design_context.py`
- `core\tests\unit\test_structured_handoff_export.py`
- `core\tests\unit\test_structured_logging.py`
- `core\tests\unit\test_style_fit_validator.py`
- `core\tests\unit\test_task_semanticizer.py`
- `core\tests\unit\test_template_l5_expansion.py`
- `core\tests\unit\test_ui_panels_import.py`
- `core\tests\unit\test_ui_semantic_reports.py`
- `core\tests\unit\test_unattended_recovery.py`
- `core\tests\unit\test_validation_cli.py`

该批次确认 Python 测试本身是 Rust 复刻验收标准的一部分，不允许在 NEWrust 中仅以“已有 Python 测试”作为证明。所有 `core/tests` pending 文件均裁决为 `test_port`，需要迁移到对应 crate unit/integration tests、Web component tests、Tauri command tests、CLI tests 或 gates。测试覆盖面包括 AI 配置与 provider、设计契约、asset spec、Stage00-Step14 结构化管线、save/draft/archive/lock、parallel runtime isolation、execution object/unattended recovery、style confirmation、semantic quality UI、SDK/patch/iteration CLI、schema registry 和全链路语义质量 gate。

第十六批深读裁决已覆盖 `knowledge/ucos` 统一认知操作系统层：

- `knowledge\ucos\__init__.py`
- `knowledge\ucos\adapters\__init__.py`
- `knowledge\ucos\adapters\api_adapter.py`
- `knowledge\ucos\adapters\base.py`
- `knowledge\ucos\adapters\claude_code_adapter.py`
- `knowledge\ucos\engines\__init__.py`
- `knowledge\ucos\engines\decision_engine.py`
- `knowledge\ucos\engines\identity_engine.py`
- `knowledge\ucos\engines\memory_engine.py`
- `knowledge\ucos\engines\planning_engine.py`
- `knowledge\ucos\engines\reflection_engine.py`
- `knowledge\ucos\engines\skill_engine.py`
- `knowledge\ucos\engines\world_model_engine.py`
- `knowledge\ucos\output\__init__.py`
- `knowledge\ucos\output\context_builder.py`
- `knowledge\ucos\output\formatters\__init__.py`
- `knowledge\ucos\output\formatters\agents_md.py`
- `knowledge\ucos\output\formatters\json_format.py`
- `knowledge\ucos\output\formatters\summary.py`
- `knowledge\ucos\output\token_budget.py`
- `knowledge\ucos\scripts\__init__.py`
- `knowledge\ucos\scripts\ucos_init.py`
- `knowledge\ucos\scripts\ucos_migrate.py`
- `knowledge\ucos\scripts\ucos_query.py`
- `knowledge\ucos\scripts\ucos_sync.py`
- `knowledge\ucos\scripts\ucos_validate.py`

该批次确认 `knowledge/ucos` 不是可忽略的资料夹，而是 Python 项目的记忆、身份、技能、决策、规划、反思、上下文输出和开发工具同步系统。Rust 侧必须新增或落地 `adm-new-knowledge` 承载 memory tier、identity policy、skill registry、decision/planning/reflection/world model、context builder、token budget 和 formatters；脚本层迁移到 `adm-new-cli ucos init/migrate/query/sync/validate`。仅包标记文件可 `drop_with_reason`，其余均需以产品知识子系统或 CLI 工具复刻。

第十七批深读裁决已覆盖 `pipeline/` Stage00-Step14 与 D1-D4 生产链路：

- `pipeline\_design_base.py`
- `pipeline\step_00_idea_intake\__init__.py`
- `pipeline\step_00_idea_intake\helpers.py`
- `pipeline\step_00_idea_intake\plugin.py`
- `pipeline\step_01_gameplay_framework\__init__.py`
- `pipeline\step_01_gameplay_framework\helpers.py`
- `pipeline\step_01_gameplay_framework\plugin.py`
- `pipeline\step_02_design_review_freeze\__init__.py`
- `pipeline\step_02_design_review_freeze\helpers.py`
- `pipeline\step_02_design_review_freeze\plugin.py`
- `pipeline\step_02_design_review_freeze\supplement.py`
- `pipeline\step_02_design_review_freeze\supplement_contracts.py`
- `pipeline\step_02_design_review_freeze\supplement_entities.py`
- `pipeline\step_03_program_requirements\__init__.py`
- `pipeline\step_03_program_requirements\binding.py`
- `pipeline\step_03_program_requirements\contract_builder.py`
- `pipeline\step_03_program_requirements\helpers.py`
- `pipeline\step_03_program_requirements\plugin.py`
- `pipeline\step_04_art_requirements\__init__.py`
- `pipeline\step_04_art_requirements\contract_builder.py`
- `pipeline\step_04_art_requirements\helpers.py`
- `pipeline\step_04_art_requirements\plugin.py`
- `pipeline\step_05_program_review\__init__.py`
- `pipeline\step_05_program_review\helpers.py`
- `pipeline\step_05_program_review\plugin.py`
- `pipeline\step_06_art_review\__init__.py`
- `pipeline\step_06_art_review\plugin.py`
- `pipeline\step_07_art_style_generation\__init__.py`
- `pipeline\step_07_art_style_generation\plugin.py`
- `pipeline\step_08_design_to_plan\__init__.py`
- `pipeline\step_08_design_to_plan\plugin.py`
- `pipeline\step_09_art_plan\__init__.py`
- `pipeline\step_09_art_plan\plugin.py`
- `pipeline\step_10_asset_alignment\__init__.py`
- `pipeline\step_10_asset_alignment\plugin.py`
- `pipeline\step_11_dev_execution\__init__.py`
- `pipeline\step_11_dev_execution\plugin.py`
- `pipeline\step_12_art_production\__init__.py`
- `pipeline\step_12_art_production\plugin.py`
- `pipeline\step_13_scene_assembly\plugin.py`
- `pipeline\step_14_integration_validation\__init__.py`
- `pipeline\step_14_integration_validation\plugin.py`
- `pipeline\step_d1_project_portrait\__init__.py`
- `pipeline\step_d1_project_portrait\plugin.py`
- `pipeline\step_d2_design_decisions\__init__.py`
- `pipeline\step_d2_design_decisions\plugin.py`
- `pipeline\step_d3_design_validation\__init__.py`
- `pipeline\step_d3_design_validation\plugin.py`
- `pipeline\step_d4_devflow_handoff\__init__.py`
- `pipeline\step_d4_devflow_handoff\plugin.py`

该批次确认 `pipeline/` 不是可由 `core/engines/generation.py` 一笔带过的目录。Rust 侧必须复刻 stage wrapper 的 source group/test-mode/result 状态协议，也必须复刻 Step00/01/02/03/04/05 的具体结构化行为：品类模板与核心问题覆盖、玩法循环和系统推断、L5 实体抽取与 AI/降级补全、program requirement 绑定与标准契约、art requirement 与视觉 contract、占位符/溯源/验收审查。Step07 的人工风格确认恢复、Step13 standalone metadata、Step14 completed_with_review 阻断与 standalone partial 降级、D1-D4 设计画像/决策/验证/交付报告均是全项目复刻的必需行为。18 个 package/docstring marker 可 `drop_with_reason`，其余 32 个均需 Rust 产品代码或 pipeline gate 承载。

第十八批深读裁决已覆盖 `tools/` 工具、迁移、验证、资源生产、维护脚本层：

- `tools\__init__.py`
- `tools\asset_production\__init__.py`
- `tools\asset_production\audio_placeholder.py`
- `tools\asset_production\codex_image_tool.py`
- `tools\asset_production\image_api_config.py`
- `tools\asset_production\image_api_probe.py`
- `tools\asset_production\image_metadata_checker.py`
- `tools\asset_production\image_tool.py`
- `tools\asset_production\localization_injector.py`
- `tools\asset_production\sfx_tool.py`
- `tools\asset_production\sprite_atlas_packer.py`
- `tools\asset_production\sprite_slicer.py`
- `tools\build\__init__.py`
- `tools\build\build.py`
- `tools\build\verify_build.py`
- `tools\config\migrate_ai_config.py`
- `tools\design\fill_template_gameplay_systems.py`
- `tools\design\rebuild_builtin_project_templates.py`
- `tools\dev\__init__.py`
- `tools\dev\config_compiler.py`
- `tools\dev\error_logger_generator.py`
- `tools\dev\git_tool.py`
- `tools\dev\perf_profiler_generator.py`
- `tools\dev\scaffold.py`
- `tools\dev\scaffold_step.py`
- `tools\dev\test_generator.py`
- `tools\dev\ui_state_generator.py`
- `tools\memory\check_staleness.py`
- `tools\memory\update_freshness.py`
- `tools\patch\__init__.py`
- `tools\patch\manager.py`
- `tools\repair_scene_assembly.py`
- `tools\repair_step11_eo_states.py`
- `tools\save\audit_parallel_isolation.py`
- `tools\save\repair_blank_save_progress.py`
- `tools\save\repair_parallel_save_contamination.py`
- `tools\scripts\__init__.py`
- `tools\scripts\check_hardcoded_paths.py`
- `tools\scripts\export_concept_package.py`
- `tools\scripts\inspect_reports.py`
- `tools\scripts\migrate_design_projects_to_execution_objects.py`
- `tools\scripts\migrate_execution_objects_add_save_id.py`
- `tools\scripts\migrate_legacy.py`
- `tools\scripts\schema_migrator.py`
- `tools\sdk\__init__.py`
- `tools\sdk\manager.py`
- `tools\validators\__init__.py`
- `tools\validators\compile_checker.py`
- `tools\validators\config_validator.py`
- `tools\validators\context_lint.py`
- `tools\validators\contract_validator.py`
- `tools\validators\design_semantic_quality.py`
- `tools\validators\environment_checker.py`
- `tools\validators\output_validator.py`
- `tools\validators\pipeline_quality.py`

该批次确认 `tools/` 不是垃圾目录：资源生产、图像 API 探测、Unity 代码生成、配置迁移、内置模板重建、SDK/patch 管理、save/EO 修复、硬编码路径审计、schema migration、语义质量与 pipeline quality 验证都必须进入 Rust CLI、gate 或对应 service。仅空包标记可 `drop_with_reason`。占位音频/SFX 和占位测试生成必须保留兼容语义但在报告中明确标记为 placeholder/SKIPPED，不能作为真实生成或真实测试通过证明。部分旧实现质量较差，例如 regex C# 注入、过时 `src.plugins` 导入、`sprite_atlas_packer.py` 的旧 import 名称；Rust 迁移时必须保留功能意图并修正实现方式。

第一轮静态 import 索引已生成，并在 2026-07-09 经 `.gitignore` 漏扫复核修正源码基线：

- Python file count: 379
- Parse error count: 0
- Entry candidate count: 161
- Static orphan candidate count: 71

第二轮动态索引已生成：

- `pipeline/_registry.json` module entries: 19
- `__main__` entry files: 42
- `importlib` hits: 7
- `subprocess` / `Popen` hits: 98

第二轮孤立候选矩阵已生成：

- Static orphan candidates: 71
- Final decisions completed: 71

下一轮必须进行多角色评分。若任一单项低于 90、综合低于 95，或评分发现设计缺口，则回到对应源码/文档继续修正，不能直接进入开发。

## 6. 第二轮评分

评分对象：v3 全项目复刻设计计划是否足以进入原子开发计划生成。该评分不代表 Rust 实现已经完成；实现完成仍需后续测试、截图、构建和 handoff gate。

| 角色 | 领域 | 分数 | 权重 | confidence | evidence | residual risk | decision |
| --- | --- | ---: | ---: | --- | --- | --- | --- |
| Python Whole-Project Auditor | 379 个 Python 文件全量覆盖 | 98 | 12 | high | `01_full_python_file_inventory.md`、`03_file_disposition_matrix.md` 379/379，pending 行 0 | 后续新增 Python 文件必须重新跑 inventory | pass |
| Reachability Analyst | 入口/import/动态加载/孤儿候选 | 96 | 12 | high | 第二轮索引：入口候选 161、orphan 71/71 completed、registry 19 | 静态索引不能证明 UI callback 全路径，需开发阶段用 UI interaction gate 补证 | pass |
| Rust Migration Architect | 文件级 Rust/Web/CLI 目标 | 96 | 14 | high | `04_rust_target_mapping.md` 379/379，`adm-new-config`、`adm-new-knowledge` 等新增目标已显式列出 | 新增 crate 需要在原子计划中排序，避免实现时漏建 workspace member | pass |
| Tooling and Build Reviewer | tools/build/validator/memory/repair 脚本 | 96 | 10 | high | `08_tooling_migration_matrix.md` 覆盖 55/55，release/build/save/SDK/patch/image/dev validators 均有 CLI/gate 目标 | 低质量旧脚本必须按新实现约束重做，不能原样正则搬运 | pass |
| UI Pixel Parity Reviewer | Tk baseline 到 Web/Tauri 复刻方案 | 95 | 12 | medium | `07_ui_python_baseline_plan.md` 覆盖 22 个 UI 文件、状态矩阵、截图/交互/command contract gate | 尚未实际采集截图；这是实现阶段 UI completion gate | pass |
| QA Gate Reviewer | Python 测试迁移和文件级 gate | 96 | 14 | high | `09_test_migration_matrix.md` 覆盖 73/73 Python test files，全部映射到 Rust/Web/gate 目标 | 原子开发计划必须把测试与实现同批次或紧邻批次排序 | pass |
| Data Asset Reviewer | JSON/TOML/schema/knowledge assets | 95 | 12 | medium | `11_data_asset_migration_matrix.md` 覆盖 knowledge/settings 727 个非 Python 资产分组 | 数据矩阵是分组级，开发阶段 loader gate 必须验证实际文件枚举 | pass |
| Red Team Reviewer | 伪全量、垃圾误删、历史 v2 误判风险 | 95 | 14 | high | `.gitignore` 漏扫已修正，垃圾目录与 archive/runtime sample 有明确规则，v2 handoff 降级为 evidence | 若直接跳到开发而不生成原子计划会复发，需保持 P8 gate | pass |

第二轮加权综合分：`95.86`。

硬门禁：

- inventory gate: pass
- disposition gate: pass
- authoritative gate: pass
- tool gate: pass
- test gate: pass
- ucos gate: pass
- data design gate: pass
- UI design gate: pass
- handoff design gate: pass

第二轮结论：通过设计评分。`10_v3_atomic_development_plan.md` 已生成并回读，开发按原子顺序从 A00 开始；该评分不代表实现已经完成，后续仍必须逐 atom 产出测试、截图、构建和文件级 handoff 证据。
