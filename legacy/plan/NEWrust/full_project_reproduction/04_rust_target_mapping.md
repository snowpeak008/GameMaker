# Rust Target Mapping

状态：v3 文件级 Rust 目标映射。已同步 379 个已裁决 Python 文件，`pending_file_mappings=0`。本文件仍需与孤儿可达性裁决和多角色评分一起使用，不能单独作为开发开闸证据。

## 1. 目标原则

每个 Python 文件必须映射到 Rust 侧目标之一：

- `NEWrust/crates/adm-new-foundation`
- `NEWrust/crates/adm-new-config` (v3 required new crate if not already present)
- `NEWrust/crates/adm-new-contracts`
- `NEWrust/crates/adm-new-storage`
- `NEWrust/crates/adm-new-design`
- `NEWrust/crates/adm-new-knowledge` (v3 required new crate if not already present)
- `NEWrust/crates/adm-new-ai`
- `NEWrust/crates/adm-new-pipeline`
- `NEWrust/crates/adm-new-artifact`
- `NEWrust/crates/adm-new-save`
- `NEWrust/crates/adm-new-packaging`
- `NEWrust/crates/adm-new-patch`
- `NEWrust/crates/adm-new-sdk`
- `NEWrust/crates/adm-new-application`
- `NEWrust/crates/adm-new-tauri-commands`
- `NEWrust/apps/adm-new-cli`
- `NEWrust/apps/desktop-tauri`
- `NEWrust/web`
- `NEWrust/gates`
- `data asset`
- `test/gate`
- `drop_with_reason`

## 2. 当前缺口

v2 映射粒度是功能域，不足以支撑全项目复刻。v3 必须补齐：

- `tools/**/*.py` 到 CLI/gate/xtask 的映射。
- `pipeline/**/*.py` 已完成每个 stage/helper/contract builder 的 Rust stage service 归属，后续开发必须按本矩阵落地。
- `knowledge/ucos/**/*.py` 已完成产品 AI memory、外部开发工具或 drop 的裁决，后续开发必须按本矩阵落地。
- `core/tests/**/*.py` 已完成 Rust/Web/gate 测试映射，后续开发必须按本矩阵落地。
- `sitecustomize.py`、`conftest.py` 等环境文件已完成替代策略裁决，后续只允许按矩阵实现。

## 3. 目标矩阵模板

| Python file | final disposition | Rust target | required tests | gate | status |
| --- | --- | --- | --- | --- | --- |
| `conftest.py` | `test_port` | `NEWrust/gates + cargo/web test harness` | isolated temp/cache test harness coverage | test gate | decided |
| `core\__init__.py` | `absorbed` | `adm-new-foundation::runtime_cache_policy` | path/cache policy unit test | foundation gate | decided |
| `core\adapters\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\adapters\base.py` | `implemented` | `adm-new-ai::adapter` | ModelTask/ModelResult serialization and trait contract tests | AI adapter gate | decided |
| `core\adapters\claude_code_adapter.py` | `drop_with_reason` | none | no references; superseded by model adapter | disposition review | decided |
| `core\adapters\claude_code_model_adapter.py` | `implemented` | `adm-new-ai::providers::claude_cli` | CLI resolution, timeout, stderr mapping tests with mocked process | AI adapter gate | decided |
| `core\adapters\codex\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\adapters\codex\executor.py` | `implemented` | `adm-new-ai::providers::codex_cli::exec` | codex command resolution, sandbox/cwd args, allowlist failure tests | AI adapter gate | decided |
| `core\adapters\codex\file_guard.py` | `implemented` | `adm-new-ai::providers::codex_cli::output_allowlist_guard` | allowed path exact/prefix/outside cases | AI adapter gate | decided |
| `core\adapters\codex\result_parser.py` | `absorbed` | `adm-new-ai::providers::codex_cli::result_summary` | truncation and no-truncation tests | AI adapter gate | decided |
| `core\adapters\codex\task_builder.py` | `implemented` | `adm-new-ai::task_builder` | declared input/output/allowed path prompt construction tests | AI adapter gate | decided |
| `core\adapters\codex_adapter.py` | `implemented` | `adm-new-ai::providers::codex_cli::CodexAdapter` | profile config and cwd resolution tests | AI adapter gate | decided |
| `core\adapters\completion_adapter.py` | `implemented` | `adm-new-ai::completion_adapter` | dev/image/completion config factory tests | AI config/adapter gate | decided |
| `core\adapters\local_adapter.py` | `implemented` | `adm-new-ai::providers::local_disabled` | disabled failure result test | AI adapter gate | decided |
| `core\adapters\memory\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\adapters\memory\context_builder.py` | `absorbed` | `adm-new-knowledge::ucos_memory_bridge::context_builder` | UCOS context builder facade parity test | UCOS bridge gate | decided |
| `core\adapters\memory\token_budget.py` | `absorbed` | `adm-new-knowledge::ucos_memory_bridge::token_budget` | estimate/enforce/MAX_TOKENS parity tests | UCOS bridge gate | decided |
| `core\adapters\openai_adapter.py` | `implemented` | `adm-new-ai::providers::openai_compatible` | config coercion, input-file embedding, error mapping tests with mocked caller | AI adapter gate | decided |
| `core\adapters\registry.py` | `implemented` | `adm-new-ai::adapter_registry` | provider lookup and unsupported adapter tests | AI adapter gate | decided |
| `core\ai_design\__init__.py` | `absorbed` | `adm-new-ai::design_contracts::public_api` | public API export coverage for design contract helpers | AI design gate | decided |
| `core\ai_design\asset_spec_gate.py` | `implemented` | `adm-new-ai::asset_spec_gate` | required field, non-consumable marker, prompt phrase and transparent background warning tests | AI design gate | decided |
| `core\ai_design\completion_service.py` | `implemented` | `adm-new-ai::completion_json_service` | fenced/raw JSON extraction, retry hint, missing config and adapter error tests with mocked completion adapter | AI design gate | decided |
| `core\ai_design\contract_gate.py` | `implemented` | `adm-new-ai::contract_gate` | registered/generic contract validation, missing refs, blocker/warning aggregation and schema path tests | AI design gate | decided |
| `core\ai_design\prompt_library.py` | `implemented` | `adm-new-ai::design_prompt_library` | Step00-09 template lookup, fallback and placeholder integrity tests | AI design gate | decided |
| `core\ai_design\traceability.py` | `implemented` | `adm-new-ai::traceability_gate` | missing output/allowed/source/contract/acceptance fields and accepted happy-path tests | AI design gate | decided |
| `core\ai_design\types.py` | `implemented` | `adm-new-ai::design_contracts::types` | issue/gate/completion result serialization and blocker/warning helper tests | AI design gate | decided |
| `core\art_pipeline\__init__.py` | `absorbed` | `adm-new-design::art_pipeline::public_api` | public API export coverage | art pipeline gate | decided |
| `core\art_pipeline\paths.py` | `implemented` | `adm-new-design::art_pipeline::paths` | canonical Unity path, slug, extension, legacy detection and allowed parent path tests | art pipeline gate | decided |
| `core\art_pipeline\stage04.py` | `implemented` | `adm-new-design::art_pipeline::stage04_specs` | strategy merge, target normalization, consumable specs, import policy and audio placeholder tests | art pipeline gate | decided |
| `core\art_pipeline\stage09.py` | `implemented` | `adm-new-design::art_pipeline::stage09_task_enrichment` | task prompt defaults, semantic policy, slice ref and rework policy tests | art pipeline gate | decided |
| `core\art_pipeline\stage12.py` | `implemented` | `adm-new-design::art_pipeline::stage12_processing` | raw/processed manifests, quality/semantic review, rework queue, sprite/import/atlas/prefab/mount/preflight/handoff tests | art pipeline gate | decided |
| `core\art_pipeline\stage13.py` | `implemented` | `adm-new-design::art_pipeline::stage13_unity_materialization` | editor request sequencing and materialization report status/blocker tests | art pipeline gate | decided |
| `core\art_pipeline\stage14.py` | `implemented` | `adm-new-design::art_pipeline::stage14_acceptance` | art acceptance, level4 visual threshold and level5 coverage tests | art pipeline gate | decided |
| `core\artifact\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\artifact\graph.py` | `implemented` | `adm-new-artifact::dependency_graph` | topological order, unknown dependency, cycle detection tests | artifact gate | decided |
| `core\artifact\manifest.py` | `implemented` | `adm-new-artifact::manifest` | versioned dir parse, sha256, file entry, manifest writer tests | artifact gate | decided |
| `core\artifact\preflight.py` | `implemented` | `adm-new-artifact::preflight` | reviewer/validator registry, schema refs, dependency status preflight tests | artifact gate | decided |
| `core\artifact\registry_loader.py` | `implemented` | `adm-new-artifact::registry` | missing/empty/duplicate artifact registry tests | artifact gate | decided |
| `core\artifact\reviewer.py` | `implemented` | `adm-new-artifact::review_pipeline` | structure/source/task/dependency reviewer tests | artifact gate | decided |
| `core\artifact\validator.py` | `implemented` | `adm-new-artifact::validation_pipeline` | stage/schema/manifest/knowledge/dependency validator tests | artifact gate | decided |
| `core\config\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\config\ai_config.py` | `implemented` | `adm-new-config::ai_config` | default config, legacy migration, active entry/profile tests | config gate | decided |
| `core\config\ai_config_schema.py` | `implemented` | `adm-new-config::ai_config_schema` | category defaults, entry conversion, compatibility profile tests | config gate | decided |
| `core\config\integrity.py` | `implemented` | `adm-new-config::startup_integrity` | missing schema/plugin/data error aggregation tests | startup gate | decided |
| `core\config\loader.py` | `implemented` | `adm-new-config::loader` | TOML/JSON merge, env fallback, base URL normalization, caller config tests | config gate | decided |
| `core\config\validator.py` | `implemented` | `adm-new-config::ai_config_validator` | offline entry/profile/config validation and mocked CLI probe tests | config gate | decided |
| `core\context.py` | `absorbed` | `adm-new-pipeline::stage_context` | StageContext/StageResult/StageStatus typed contract tests | pipeline gate | decided |
| `core\design\__init__.py` | `drop_with_reason` | none | no behavior; package docstring only | disposition review | decided |
| `core\design\ai_backend.py` | `implemented` | `adm-new-design::codex_cli_backend` | API config parse, CODEX_HOME/auth, command args, read-only sandbox, timeout/cancel, output JSON extraction and timing tests | design AI gate | decided |
| `core\design\ai_interview.py` | `implemented` | `adm-new-design::ai_interview` | state normalization, summary updates, route overview, prompt budget degradation, meter/replay write and partition prompt tests | design AI gate | decided |
| `core\design\ai_mapping_agent.py` | `implemented` | `adm-new-design::ai_mapping_agent` | explicit signal, mapping schedule, prompt payload and payload validator tests | design AI gate | decided |
| `core\design\ai_memory_retriever.py` | `implemented` | `adm-new-design::ai_memory_retriever` | staged signal scoring, dedupe, top-k and hidden injection log tests | design AI gate | decided |
| `core\design\ai_llm_backend.py` | `implemented` | `adm-new-design::ai_llm_backend` | capability defaults, JSON task trait and turn trait contract tests | design AI gate | decided |
| `core\design\ai_prompt_packer.py` | `implemented` | `adm-new-design::ai_prompt_packer` | compact JSON stability, SHA fields, preview/full prompt env behavior and prompt prefix tests | design AI gate | decided |
| `core\design\ai_route_planner.py` | `implemented` | `adm-new-design::ai_route_planner` | CJK tokenization, recent target suppression, focus-domain fallback and scoring tests | design AI gate | decided |
| `core\design\ai_schema.py` | `implemented` | `adm-new-design::ai_schema` | schema required fields, enum modes and backward alias tests | design AI gate | decided |
| `core\design\ai_summary_agent.py` | `implemented` | `adm-new-design::ai_summary_agent` | summary prompt payload and invalid list/object field validator tests | design AI gate | decided |
| `core\design\ai_ucos_bridge.py` | `implemented` | `adm-new-design::ai_ucos_bridge + adm-new-knowledge::ucos_writer` | episodic turn, router short-term, semantic staging and design episode write tests | UCOS/design memory gate | decided |
| `core\design\ai_validator.py` | `implemented` | `adm-new-design::ai_output_validator` | full/partial payload validation, confidence merge, diff generation and high-confidence apply tests | design AI gate | decided |
| `core\design\art_taxonomy.py` | `implemented` | `adm-new-design::art_taxonomy` | required category coverage, asset strategy generation, fallback and blocker tests | design contract gate | decided |
| `core\design\cross_layer_lint.py` | `implemented` | `adm-new-design::cross_layer_lint` | rule loading, profile match, selected option context and violation tests | design consistency gate | decided |
| `core\design\data_loader.py` | `implemented` | `adm-new-design::data_loader` | path fallback, templateRef resolution, option relation normalization, domain validation and project data meta tests | design data gate | decided |
| `core\design\engine.py` | `implemented` | `adm-new-design::engine` | state normalization, selection/provenance, conflicts, coverage, completion summary, quality metrics and focus-domain tests | design engine gate | decided |
| `core\design\engine_data_loader.py` | `absorbed` | `adm-new-design::data_loader::public_api` | public facade coverage for load_all/load_domains/load_project_data | design data gate | decided |
| `core\design\entity_schema.py` | `implemented` | `adm-new-design::entity_schema_registry` | schema lookup by id/kind/version, type/const/enum/required/list/anyOf/oneOf validation tests | design data gate | decided |
| `core\design\export_adapter.py` | `implemented` | `adm-new-design::devflow_export_adapter` | Concept/GameplayFramework/Design package writers, manifest/approval sidecars and structured handoff tests | design export gate | decided |
| `core\design\exporter.py` | `implemented` | `adm-new-design::exporter` | payload/taxonomy/metadata, markdown/text/prompt/archive rendering, sidecar JSON and write_export tests | design export gate | decided |
| `core\design\framework_memory.py` | `implemented` | `adm-new-design::framework_memory` | evidence/log append, review chain qualification, regression extraction, aggregation, promotion gate and rollback tests | design memory gate | decided |
| `core\design\gameplay_systems.py` | `implemented` | `adm-new-design::gameplay_systems` | custom/preset normalization, inferred systems, weight normalization, validation and parsed answer tests | design data gate | decided |
| `core\design\node_role.py` | `implemented` | `adm-new-design::node_role` | role normalization/defaulting and domain role count tests | design data gate | decided |
| `core\design\open_questions.py` | `implemented` | `adm-new-design::open_questions` | normalize dict/string question, blocking/resolved counters and unresolved blocker tests | design contract gate | decided |
| `core\design\option_mapping.py` | `implemented` | `adm-new-design::option_mapping + adm-new-cli::generate_option_mapping` | domain/template mapping JSON, Markdown escaping/rendering and writer entrypoint tests | design CLI gate | decided |
| `core\design\playable_contracts.py` | `implemented` | `adm-new-design::playable_contracts` | structured/legacy bundle generation, contract validation, file map read/write and PLAY task synthesis tests | playable contract gate | decided |
| `core\design\profile_schema.py` | `implemented` | `adm-new-design::profile_schema` | defaults, option label/value conversion and display profile tests | design data gate | decided |
| `core\design\program_capabilities.py` | `implemented` | `adm-new-design::program_capabilities` | Project DNA to capability mapping, missing state/completion blockers and coverage report tests | design contract gate | decided |
| `core\design\project_dna.py` | `implemented` | `adm-new-design::project_dna` | seed defaults, freeze, playable refs, null field and open question blocker tests | design contract gate | decided |
| `core\design\project_identity.py` | `implemented` | `adm-new-design::project_identity` | output-base detection, stable hash/signature, slug and customization score tests | design contract gate | decided |
| `core\design\project_templates.py` | `implemented` | `adm-new-design::project_templates` | builtin/custom precedence, payload normalization, save/delete custom template and scale option tests | design data gate | decided |
| `core\design\prompt_evaluation.py` | `implemented` | `adm-new-design::prompt_evaluation` | sample set validation, offline/synthetic/Codex smoke scoring, reports, gate policy and anonymized sample extraction tests | prompt governance gate | decided |
| `core\design\prompt_framework.py` | `implemented` | `adm-new-design::prompt_framework` | default framework creation, manifest/hash validation, boundary checks, diff candidate, promote and rollback tests | prompt governance gate | decided |
| `core\design\requirements.py` | `implemented` | `adm-new-design::archetype_requirements` | archetype data loading, keyword/rule detection, metadata defaults and fallback generic tests | design contract gate | decided |
| `core\design\semantic_alignment.py` | `implemented` | `adm-new-design::semantic_alignment` | program/art gap handling, placeholder-only code and 85 percent threshold tests | semantic gate | decided |
| `core\design\semantic_coverage.py` | `implemented` | `adm-new-design::semantic_coverage` | coverage seed ID generation from Project DNA fields tests | semantic gate | decided |
| `core\design\style_fit.py` | `implemented` | `adm-new-design::style_fit` | archetype readability blocker, override acknowledgement and risk tests | design contract gate | decided |
| `core\design\structured_context.py` | `implemented` | `adm-new-design::structured_context` | artifacts dir resolution, latest D4 package lookup, required/optional contract and trace tests | design contract gate | decided |
| `core\design\structured_handoff.py` | `implemented` | `adm-new-design::structured_handoff` | structured decisions, traceability mismatch, manifest contract registry and blocked validation tests | design export gate | decided |
| `core\design\task_semanticizer.py` | `implemented` | `adm-new-design::task_semanticizer` | art task synthesis, program/art semantic enrichment, generic ratio and coverage matrix tests | semantic gate | decided |
| `core\engines\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\engines\execution_objects\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\engines\execution_objects\correction_queue.py` | `implemented` | `adm-new-application::execution_objects::correction_queue` | conflict classification, target-stage routing, Markdown/JSON load/write and known-gap tests | execution object gate | decided |
| `core\engines\execution_objects\design_project.py` | `implemented` | `adm-new-application::execution_objects::design_project` | design project save/autosave/latest/version/restore metadata tests | execution object gate | decided |
| `core\engines\execution_objects\integration.py` | `implemented` | `adm-new-application::execution_objects::integration` | program/art/scene EO begin/verify/failure/retry/remediation/reference-audit tests | execution object gate | decided |
| `core\engines\execution_objects\paths.py` | `implemented` | `adm-new-application::execution_objects::paths` | save-bound execution object store path and run-context override tests | execution object gate | decided |
| `core\engines\execution_objects\type_registry.py` | `implemented` | `adm-new-application::execution_objects::type_registry` | type metadata, confirmation level, write scope prefix and category lookup tests | execution object gate | decided |
| `core\engines\execution_objects\unattended_recovery.py` | `implemented` | `adm-new-application::execution_objects::unattended_recovery` | failure classification, reproduction payload, queue upsert, resume cursor and dependency skip tests | recovery gate | decided |
| `core\engines\execution_objects\user_artifact.py` | `implemented` | `adm-new-application::execution_objects::user_artifact` | export artifact lifecycle, filter/list/get and soft delete tests | execution object gate | decided |
| `core\engines\execution_objects\workflow.py` | `implemented` | `adm-new-application::execution_objects::workflow` | state transitions, confirmation gates, drift/conflict checks, failure/retry/remediation/verify and cleanup tests | execution object gate | decided |
| `core\engines\execution_objects\workspace_snapshot.py` | `implemented` | `adm-new-application::execution_objects::workspace_snapshot` | workspace manifest hashing, snapshot compare and file history tests | execution object gate | decided |
| `core\engines\generation.py` | `implemented` | `adm-new-pipeline::generation_engine + adm-new-application::stage11_executor + adm-new-design::stage_output_contracts + adm-new-artifact::stage_reports` | Step00-14 stage output parity, task topology, Unity execution, art production, scene assembly, acceptance and resume/recovery tests | pipeline parity gate | decided |
| `core\engines\handoff_loader.py` | `implemented` | `adm-new-design::handoff_loader` | structured handoff package discovery, JSON read/write and design source load tests | design handoff gate | decided |
| `core\engines\source_context.py` | `implemented` | `adm-new-pipeline::source_context` | source package iteration/type match/latest package/latest structured handoff tests | source gate | decided |
| `core\io.py` | `absorbed` | `adm-new-foundation::io` | read/write JSON/text, rel path, file manifest tests | foundation gate | decided |
| `core\main.py` | `implemented` | `adm-new-cli + adm-new-pipeline::orchestrator + adm-new-tauri-commands` | CLI args, run_range, iterate, package, preflight-only, stop/review states | pipeline/CLI gate | decided |
| `core\paths.py` | `absorbed` | `adm-new-foundation::paths` | root discovery, session id, draft/workspace/save path tests | foundation gate | decided |
| `core\plugin_manager.py` | `absorbed` | `adm-new-pipeline::stage_registry` | registry load/list/validate and stage dispatch tests | pipeline gate | decided |
| `core\registry.py` | `implemented` | `adm-new-pipeline::registry` | D1-D4/Step00-14 metadata and dependency satisfaction tests | pipeline gate | decided |
| `core\skill_loader.py` | `absorbed` | `adm-new-knowledge::skill_guidance` | SKILL.md lookup and guidance artifact writer tests | knowledge gate | decided |
| `core\stage.py` | `absorbed` | `adm-new-pipeline::stage_fs` | safe reset, stage file classifier, gate log, reference manifest tests | pipeline gate | decided |
| `core\stage_plugin.py` | `absorbed` | `adm-new-pipeline::StagePlugin` | lifecycle validation/execute/output contract tests | pipeline gate | decided |
| `core\tests\conftest.py` | `test_port` | `Rust/Web test fixtures and isolated config temp roots` | fixture isolation and cache reset coverage | test gate | decided |
| `core\tests\integration\test_adapter_configuration.py` | `test_port` | `adm-new-ai integration tests` | adapter profile construction and active pipeline adapter tests | test gate | decided |
| `core\tests\integration\test_design_semantic_pipeline.py` | `test_port` | `NEWrust/gates::design_semantic_pipeline + adm-new-pipeline integration tests` | Step00-Step10 semantic output and low quality failure tests | test gate | decided |
| `core\tests\integration\test_parallel_project_semantic_isolation.py` | `test_port` | `adm-new-save + adm-new-design integration isolation tests` | parallel draft context and project signature isolation tests | test gate | decided |
| `core\tests\integration\test_plugins.py` | `test_port` | `adm-new-pipeline::registry integration tests` | plugin manifest, design stage and development import tests | test gate | decided |
| `core\tests\unit\test_ai_config.py` | `test_port` | `adm-new-config::ai_config tests + adm-new-web::dialogs::ai_config tests` | v3 config/default/migration/dialog type tests | test gate | decided |
| `core\tests\unit\test_ai_design_asset_spec.py` | `test_port` | `adm-new-ai::asset_spec_gate + adm-new-design::art_pipeline tests` | consumable asset spec, schema, Stage04 builder and Stage12 preflight tests | test gate | decided |
| `core\tests\unit\test_ai_design_completion_service.py` | `test_port` | `adm-new-ai::completion_json_service tests` | plain/fenced JSON parse and retry/error tests | test gate | decided |
| `core\tests\unit\test_ai_design_contracts.py` | `test_port` | `adm-new-ai::contract_gate + traceability + prompt_library tests` | contract validation, prompt coverage and task traceability tests | test gate | decided |
| `core\tests\unit\test_art_task_semanticization.py` | `test_port` | `adm-new-design::task_semanticizer tests` | art task usage prompt and semantic roles tests | test gate | decided |
| `core\tests\unit\test_art_taxonomy_builder.py` | `test_port` | `adm-new-design::art_taxonomy tests` | required category coverage tests | test gate | decided |
| `core\tests\unit\test_artifact_registry_playable_chain.py` | `test_port` | `adm-new-artifact::registry tests` | playable chain registration/schema visibility tests | test gate | decided |
| `core\tests\unit\test_artifact_validator_paths.py` | `test_port` | `adm-new-artifact::validator path resolution tests` | draft/current-save contract path and missing checked path tests | test gate | decided |
| `core\tests\unit\test_codex_image_tool.py` | `test_port` | `adm-new-ai::image_generation codex CLI tests` | PNG detection and stdout saved/session path parser tests | test gate | decided |
| `core\tests\unit\test_config_loader.py` | `test_port` | `adm-new-config::loader tests` | app/project settings, endpoint normalization and AI image routing tests | test gate | decided |
| `core\tests\unit\test_config_validator.py` | `test_port` | `adm-new-config::validator tests` | OpenAI/CLI/image profile validation tests | test gate | decided |
| `core\tests\unit\test_core_paths.py` | `test_port` | `adm-new-foundation::paths tests` | root marker/design data/session draft path tests | test gate | decided |
| `core\tests\unit\test_customization_scorer.py` | `test_port` | `adm-new-design::semantic_quality tests` | blocker score report tests | test gate | decided |
| `core\tests\unit\test_d2_real_decision_report.py` | `test_port` | `adm-new-pipeline::step_d2_design_decisions tests` | incomplete/unstructured decision report tests | test gate | decided |
| `core\tests\unit\test_d3_design_gate.py` | `test_port` | `adm-new-pipeline::step_d3_design_validation tests` | D3 blockers and test-mode report tests | test gate | decided |
| `core\tests\unit\test_design_node_requirement_metadata.py` | `test_port` | `adm-new-design::engine metadata tests` | requirement metadata/provenance/completion blocker tests | test gate | decided |
| `core\tests\unit\test_design_requirements_archetype_detector.py` | `test_port` | `adm-new-design::requirements archetype detector tests` | subtype/generic fallback and stable contract tests | test gate | decided |
| `core\tests\unit\test_design_requirements_archetype_subtypes.py` | `test_port` | `adm-new-design::requirements subtype tests` | subtype data/parent requirement/signal combination tests | test gate | decided |
| `core\tests\unit\test_design_semantic_quality.py` | `test_port` | `adm-new-application::semantic_quality tests` | placeholder/generic/signature/report serialization tests | test gate | decided |
| `core\tests\unit\test_design_semantic_schema_registry.py` | `test_port` | `adm-new-artifact::schema_registry tests` | semantic schemas, refs and Stage06 registration tests | test gate | decided |
| `core\tests\unit\test_draft_archive_paths.py` | `test_port` | `adm-new-save::manager + app shell lifecycle tests` | draft/archive/workspace/lock/snapshot/EO/save UI tests | test gate | decided |
| `core\tests\unit\test_execution_planner.py` | `test_port` | `adm-new-application::execution_planner tests` | write-set conflicts, readiness and execution state tests | test gate | decided |
| `core\tests\unit\test_hades_quality_optimization.py` | `test_port` | `adm-new-pipeline::optimization_helpers + adm-new-design::art_pipeline tests` | template coverage, task cleanup, Stage04/07/09 and image skip tests | test gate | decided |
| `core\tests\unit\test_iteration_cli.py` | `test_port` | `adm-new-cli iteration tests` | iteration CLI coverage | test gate | decided |
| `core\tests\unit\test_iteration_development.py` | `test_port` | `adm-new-application::iteration tests` | spec parser, delta scheduler, artifact inheritance and plan merge tests | test gate | decided |
| `core\tests\unit\test_l5_supplement.py` | `test_port` | `adm-new-design::entity_supplement + adm-new-ai tests` | L5 supplement trigger/cache/fallback/merge and Stage02 metadata tests | test gate | decided |
| `core\tests\unit\test_manual_style_confirmation.py` | `test_port` | `adm-new-pipeline::stage07_style + adm-new-web::style dialogs tests` | style options, prompt override, confirmation, image generation and dispatch tests | test gate | decided |
| `core\tests\unit\test_model_adapters.py` | `test_port` | `adm-new-ai::providers tests` | Codex/Claude/completion/OpenAI provider behavior tests | test gate | decided |
| `core\tests\unit\test_open_questions_contract.py` | `test_port` | `adm-new-design::open_questions tests` | archetype questions and unresolved blocker tests | test gate | decided |
| `core\tests\unit\test_parallel_runtime_isolation.py` | `test_port` | `adm-new-save + adm-new-application::run_context isolation tests` | bound save/source/EO/project settings/Unity lock isolation tests | test gate | decided |
| `core\tests\unit\test_patch_channel.py` | `test_port` | `adm-new-patch + adm-new-cli patch tests` | patch analyzer/validator/executor/runner/CLI promote tests | test gate | decided |
| `core\tests\unit\test_pipeline_optimization_helpers.py` | `test_port` | `adm-new-pipeline::optimization_helpers tests` | Step00-04 inference/binding/assets/reviewer/quality/source tests | test gate | decided |
| `core\tests\unit\test_pipeline_registry_schema_generation_contracts.py` | `test_port` | `adm-new-pipeline::registry + adm-new-artifact::schema_registry tests` | Stage03-09 schema generation contract tests | test gate | decided |
| `core\tests\unit\test_playable_contracts.py` | `test_port` | `adm-new-design::playable_contracts + pipeline stage tests` | playable bundle/gate/Stage02/08/12 tests | test gate | decided |
| `core\tests\unit\test_program_capability_builder.py` | `test_port` | `adm-new-design::program_capabilities tests` | semantic binding and action state-change blocker tests | test gate | decided |
| `core\tests\unit\test_project_dna_builder.py` | `test_port` | `adm-new-design::project_dna tests` | DNA freeze/open-question/playable coverage tests | test gate | decided |
| `core\tests\unit\test_project_templates.py` | `test_port` | `adm-new-design::project_templates tests` | custom template delete behavior tests | test gate | decided |
| `core\tests\unit\test_pytest_config.py` | `test_port` | `NEWrust/gates::test_environment tests` | temp cleanup/cache/pycache policy tests | test gate | decided |
| `core\tests\unit\test_reference_manifest_refresh.py` | `test_port` | `adm-new-pipeline::source::importer tests` | reference manifest refresh tests | test gate | decided |
| `core\tests\unit\test_run_state_failure.py` | `test_port` | `adm-new-application::runtime_control + adm-new-pipeline::orchestrator tests` | run state replace/clear and failure mark tests | test gate | decided |
| `core\tests\unit\test_sdk_knowledge_base.py` | `test_port` | `adm-new-sdk tests + adm-new-cli sdk tests` | SDK index/context/CLI and readable extraction tests | test gate | decided |
| `core\tests\unit\test_semantic_alignment.py` | `test_port` | `adm-new-design::semantic_alignment tests` | program/art coverage and placeholder blocker tests | test gate | decided |
| `core\tests\unit\test_stage11_parent_reuse_parallel.py` | `test_port` | `adm-new-application::stage11_executor + execution planner tests` | parent reuse, scoped snapshots, audit and parallel write tests | test gate | decided |
| `core\tests\unit\test_step00_project_identity.py` | `test_port` | `adm-new-pipeline::step00 tests` | project identity/DNA seed and draft signature isolation tests | test gate | decided |
| `core\tests\unit\test_step00_structured_profile_input.py` | `test_port` | `adm-new-pipeline::step00 tests` | structured profile override tests | test gate | decided |
| `core\tests\unit\test_step01_structured_gameplay_framework.py` | `test_port` | `adm-new-pipeline::step01 tests` | structured framework and contract consumption tests | test gate | decided |
| `core\tests\unit\test_step02_freezes_playable_contracts.py` | `test_port` | `adm-new-pipeline::step02 tests` | playable candidate freeze and UI flow blocker tests | test gate | decided |
| `core\tests\unit\test_step02_project_dna_freeze.py` | `test_port` | `adm-new-pipeline::step02 + adm-new-design::project_dna tests` | frozen Project DNA contract write tests | test gate | decided |
| `core\tests\unit\test_step03_program_requirements_contract_schema.py` | `test_port` | `adm-new-pipeline::step03 contract builder tests` | schema-valid passed/blocked program requirement contracts | test gate | decided |
| `core\tests\unit\test_step03_program_requirements_from_contracts.py` | `test_port` | `adm-new-pipeline::step03 tests` | requirements from contracts and missing contract blocker tests | test gate | decided |
| `core\tests\unit\test_step04_asset_requirements_from_contracts.py` | `test_port` | `adm-new-pipeline::step04 tests` | asset requirements and mount contract blocker tests | test gate | decided |
| `core\tests\unit\test_step05_optimization.py` | `test_port` | `adm-new-pipeline::step05 + requirement binding tests` | freeze contents, binding fallback and supplement trigger tests | test gate | decided |
| `core\tests\unit\test_step05_to_step09_structured_contract_chain.py` | `test_port` | `adm-new-pipeline::steps05_09 contract chain tests` | structured contract chain and Step07 test mode confirmation tests | test gate | decided |
| `core\tests\unit\test_step08_program_plan_from_playable_contracts.py` | `test_port` | `adm-new-pipeline::step08 tests` | program plan outputs and missing UI flow blocker tests | test gate | decided |
| `core\tests\unit\test_step10_to_step12_structured_contract_chain.py` | `test_port` | `adm-new-pipeline::steps10_12 contract chain tests` | alignment/mount readiness/Stage11 missing inputs/Stage12 report tests | test gate | decided |
| `core\tests\unit\test_step11_eo_state_closure.py` | `test_port` | `adm-new-application::execution_objects + adm-new-cli repair tests` | reuse guard, reverify path and repair report tests | test gate | decided |
| `core\tests\unit\test_step13_requires_scene_and_ui_contracts.py` | `test_port` | `adm-new-pipeline::step13_scene_assembly tests` | UI contract blocker and static playable structure report tests | test gate | decided |
| `core\tests\unit\test_step14_playable_acceptance_contract.py` | `test_port` | `adm-new-pipeline::step14_acceptance tests` | UI root failure, Unity unavailable blocker and acceptance reports | test gate | decided |
| `core\tests\unit\test_structured_design_context.py` | `test_port` | `adm-new-design::structured_context tests` | Stage02-first/D4-fallback/missing issue/traceability tests | test gate | decided |
| `core\tests\unit\test_structured_handoff_export.py` | `test_port` | `adm-new-design::structured_handoff tests` | manifest decisions and candidate export tests | test gate | decided |
| `core\tests\unit\test_structured_logging.py` | `test_port` | `adm-new-application::logging tests` | context source and JSONL roundtrip tests | test gate | decided |
| `core\tests\unit\test_style_fit_validator.py` | `test_port` | `adm-new-design::style_fit tests` | style risk and override reason tests | test gate | decided |
| `core\tests\unit\test_task_semanticizer.py` | `test_port` | `adm-new-design::task_semanticizer tests` | program task semantic refs tests | test gate | decided |
| `core\tests\unit\test_template_l5_expansion.py` | `test_port` | `adm-new-design::project_templates + gameplay systems tests` | template count/L5 sync/archive/gameplay inference tests | test gate | decided |
| `core\tests\unit\test_ui_panels_import.py` | `test_port` | `adm-new-web app shell smoke tests` | panel import, app views and step grouping smoke tests | test gate | decided |
| `core\tests\unit\test_ui_semantic_reports.py` | `test_port` | `adm-new-web::semantic_quality_panel tests` | semantic report status/return target/classification/dedupe tests | test gate | decided |
| `core\tests\unit\test_unattended_recovery.py` | `test_port` | `adm-new-application::unattended_recovery + execution_objects tests` | queue/cursor/resume/dependency/reproduction/remediation/EO ownership tests | test gate | decided |
| `core\tests\unit\test_validation_cli.py` | `test_port` | `adm-new-cli package validation tests` | package CLI success and blocked failure tests | test gate | decided |
| `core\ui\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\ui\ai_config_unified_dialog.py` | `implemented` | `adm-new-web::dialogs::ai_config + adm-new-tauri-commands::ai_config + adm-new-config` | API config CRUD, active profile, JSON validation and local CLI detection tests | UI/config gate | decided |
| `core\ui\ai_interview_window.py` | `implemented` | `adm-new-web::views::ai_interview_window + adm-new-application::ai_interview_controller` | AI turn state, Codex unavailable/error paths, partitioned output merge, archive and mapping/summary job tests | AI interview gate | decided |
| `core\ui\app_window.py` | `implemented` | `adm-new-web::views::design_workbench + adm-new-application::design_workbench_controller + adm-new-tauri-commands::design_workspace` | domain/node/checklist/L4/L5 interactions, gameplay systems, template, export, save and autosave parity tests | design UI gate | decided |
| `core\ui\bottom_panel.py` | `implemented` | `adm-new-web::components::bottom_panel` | log/AI tab switch and event stream polling component tests | UI gate | decided |
| `core\ui\embedded_interview.py` | `implemented` | `adm-new-web::components::embedded_interview_panel + adm-new-application::ai_interview_controller` | compact interview view parity against shared controller states and archive actions | AI interview gate | decided |
| `core\ui\gui_app.py` | `implemented` | `desktop-tauri startup + adm-new-application startup lifecycle service` | startup lifecycle, auto-restore, pruning, lock release and UI smoke tests | UI gate | decided |
| `core\ui\log_entry.py` | `implemented` | `adm-new-application::logging::ui_log_entry` | LogEntry serialization, source context and JSONL writer/read tests | logging gate | decided |
| `core\ui\log_panel.py` | `implemented` | `adm-new-web::views::log_panel` | level filter, clear and JSONL export UI tests | UI/logging gate | decided |
| `core\ui\main_window.py` | `implemented` | `adm-new-web::app_shell + desktop-tauri window lifecycle + adm-new-tauri-commands::app_status` | lazy navigation, geometry persistence, status bar, running pipeline close handling and lock release tests | app shell gate | decided |
| `core\ui\package_panel.py` | `implemented` | `adm-new-web::views::package_panel + adm-new-tauri-commands::packaging` | Step14 readiness and async package command tests | packaging UI gate | decided |
| `core\ui\patch_panel.py` | `implemented` | `adm-new-web::views::patch_panel + adm-new-tauri-commands::patch` | patch analyzer request/status/table refresh tests | patch UI gate | decided |
| `core\ui\pipeline_panel.py` | `implemented` | `adm-new-web::views::pipeline_panel + adm-new-tauri-commands::pipeline + adm-new-application::pipeline_ui_controller` | run range, stop, style confirmation/prompt override, semantic quality, D4 export and log event stream tests | pipeline UI gate | decided |
| `core\ui\pipeline_step_card.py` | `implemented` | `adm-new-web::components::pipeline_step_card` | status mapping and selected card rendering tests | UI gate | decided |
| `core\ui\save_manager_dialog.py` | `implemented` | `adm-new-web::dialogs::save_manager + adm-new-tauri-commands::save_manager + adm-new-save::manager` | save slot CRUD, design_project restore, project_config copy and lock conflict tests | save UI gate | decided |
| `core\ui\sdk_panel.py` | `implemented` | `adm-new-web::views::sdk_panel + adm-new-tauri-commands::sdk_knowledge` | SDK add/review status/update prompt context tests | SDK UI gate | decided |
| `core\ui\semantic_quality_panel.py` | `implemented` | `adm-new-web::components::semantic_quality_panel + adm-new-application::semantic_quality_summary` | report scan, metric extraction, issue normalization and return-target rendering tests | semantic quality gate | decided |
| `core\ui\style_confirmation_dialog.py` | `implemented` | `adm-new-web::dialogs::style_confirmation + adm-new-application::style_confirmation` | style option selection, confirmation JSON and regenerate/cancel tests | style gate | decided |
| `core\ui\style_prompt_editor.py` | `implemented` | `adm-new-web::dialogs::style_prompt_editor + adm-new-ai::style_prompt_completion + adm-new-tauri-commands::style_generation` | response parsing, prompt override writing, AI error and Stage07 rerun tests | style gate | decided |
| `core\ui\theme.py` | `absorbed` | `adm-new-web::theme_tokens + desktop-tauri window positioning` | token snapshot and window centering/platform positioning tests | UI baseline gate | decided |
| `core\ui\unity_config_dialog.py` | `implemented` | `adm-new-web::dialogs::project_config + adm-new-tauri-commands::project_settings` | engine/custom path settings save and preflight validation tests | project config gate | decided |
| `core\ui\workbench.py` | `implemented` | `adm-new-application::workbench_facade + adm-new-pipeline::source_package_writer + adm-new-tauri-commands::workbench + adm-new-cli::workbench-self-test` | StageInteraction catalog, source package writers, save sync, command runner, orchestrator range, acceptance and self-test coverage | workbench gate | decided |
| `gui_app.py` | `implemented` | `NEWrust/apps/desktop-tauri + adm-new-cli launcher path policy` | launcher compatibility and project-root path policy tests | app launch gate | decided |
| `knowledge\ucos\__init__.py` | `drop_with_reason` | none | no behavior; package marker/docstring only | disposition review | decided |
| `knowledge\ucos\adapters\__init__.py` | `drop_with_reason` | none | no behavior; package marker/docstring only | disposition review | decided |
| `knowledge\ucos\adapters\api_adapter.py` | `implemented` | `adm-new-knowledge::runtime_adapter::api` | session_start context assembly and token estimate tests | knowledge adapter gate | decided |
| `knowledge\ucos\adapters\base.py` | `implemented` | `adm-new-knowledge::runtime_adapter::traits` | UCOSContext serialization and runtime lifecycle trait contract tests | knowledge adapter gate | decided |
| `knowledge\ucos\adapters\claude_code_adapter.py` | `implemented` | `adm-new-knowledge::runtime_adapter::claude_code` | session/tool hook sync invocation tests with mocked CLI | knowledge adapter gate | decided |
| `knowledge\ucos\engines\__init__.py` | `drop_with_reason` | none | no behavior; package marker/docstring only | disposition review | decided |
| `knowledge\ucos\engines\decision_engine.py` | `implemented` | `adm-new-knowledge::decision_engine` | identity constraint filtering and best-score selection tests | knowledge engine gate | decided |
| `knowledge\ucos\engines\identity_engine.py` | `implemented` | `adm-new-knowledge::identity_engine` | profile/principle/policy load and forbidden pattern validation tests | knowledge engine gate | decided |
| `knowledge\ucos\engines\memory_engine.py` | `implemented` | `adm-new-knowledge::memory_engine` | tier CRUD, FileLock-style write safety, query, decay, consolidation and index tests | knowledge engine gate | decided |
| `knowledge\ucos\engines\planning_engine.py` | `implemented` | `adm-new-knowledge::planning_engine` | goal-to-stage plan, world dependency injection and fact snapshot hash tests | knowledge engine gate | decided |
| `knowledge\ucos\engines\reflection_engine.py` | `implemented` | `adm-new-knowledge::reflection_engine` | episode reflection, pattern/failure creation and batch abstraction tests | knowledge engine gate | decided |
| `knowledge\ucos\engines\skill_engine.py` | `implemented` | `adm-new-knowledge::skill_engine` | skill loading, trigger discovery, handler execution and dependency cycle tests | knowledge engine gate | decided |
| `knowledge\ucos\engines\world_model_engine.py` | `implemented` | `adm-new-knowledge::world_model_engine` | dependency map, causal graph and domain model loading tests | knowledge engine gate | decided |
| `knowledge\ucos\output\__init__.py` | `drop_with_reason` | none | no behavior; package marker/docstring only | disposition review | decided |
| `knowledge\ucos\output\context_builder.py` | `implemented` | `adm-new-knowledge::context_builder` | working/identity/skills/memory context assembly and budget enforcement tests | knowledge output gate | decided |
| `knowledge\ucos\output\formatters\__init__.py` | `drop_with_reason` | none | no behavior; package marker/docstring only | disposition review | decided |
| `knowledge\ucos\output\formatters\agents_md.py` | `implemented` | `adm-new-knowledge::context_formatters::agents_md` | AGENTS.md-style markdown output snapshot tests | knowledge output gate | decided |
| `knowledge\ucos\output\formatters\json_format.py` | `implemented` | `adm-new-knowledge::context_formatters::json` | JSON context output shape tests | knowledge output gate | decided |
| `knowledge\ucos\output\formatters\summary.py` | `implemented` | `adm-new-knowledge::context_formatters::summary` | session summary formatter field and fallback tests | knowledge output gate | decided |
| `knowledge\ucos\output\token_budget.py` | `implemented` | `adm-new-knowledge::token_budget` | token estimate, priority trim and recursive list/dict/string budget tests | knowledge output gate | decided |
| `knowledge\ucos\scripts\__init__.py` | `drop_with_reason` | none | no behavior; package marker/docstring only | disposition review | decided |
| `knowledge\ucos\scripts\ucos_init.py` | `cli_tool_port` | `adm-new-cli ucos init + adm-new-knowledge::bootstrap` | bootstrap tree/schema/identity/plugin/hook creation tests | CLI knowledge gate | decided |
| `knowledge\ucos\scripts\ucos_migrate.py` | `cli_tool_port` | `adm-new-cli ucos migrate + adm-new-knowledge::legacy_migration` | dry-run and migrated working/fact/episode/failure output tests | CLI knowledge gate | decided |
| `knowledge\ucos\scripts\ucos_query.py` | `cli_tool_port` | `adm-new-cli ucos query` | tier keyword query, top-k and JSON output tests | CLI knowledge gate | decided |
| `knowledge\ucos\scripts\ucos_sync.py` | `cli_tool_port` | `adm-new-cli ucos sync + adm-new-knowledge::sync_hooks` | session/tool/code_changed sync, checkpoint rotation and decay tests | CLI knowledge gate | decided |
| `knowledge\ucos\scripts\ucos_validate.py` | `cli_tool_port` | `adm-new-cli ucos validate + adm-new-knowledge::schema_validation` | schema validation, profile shape and semantic facts validation tests | CLI knowledge gate | decided |
| `pipeline\_design_base.py` | `implemented` | `adm-new-pipeline::design_stage_base` | D-stage id/title, design data summary and stage summary writer tests | pipeline design gate | decided |
| `pipeline\step_00_idea_intake\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_00_idea_intake\helpers.py` | `implemented` | `adm-new-pipeline::stages::step00::concept_profile_and_question_coverage` | genre fallback evidence, concept profile and core question coverage tests | pipeline stage00 gate | decided |
| `pipeline\step_00_idea_intake\plugin.py` | `implemented` | `adm-new-pipeline::stages::step00::plugin` | Concept source group import, test mode and output application tests | pipeline stage00 gate | decided |
| `pipeline\step_01_gameplay_framework\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_01_gameplay_framework\helpers.py` | `implemented` | `adm-new-pipeline::stages::step01::gameplay_framework` | template cache refresh, loop extraction and system deduction tests | pipeline stage01 gate | decided |
| `pipeline\step_01_gameplay_framework\plugin.py` | `implemented` | `adm-new-pipeline::stages::step01::plugin` | GameplayFramework all-history import and output application tests | pipeline stage01 gate | decided |
| `pipeline\step_02_design_review_freeze\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_02_design_review_freeze\helpers.py` | `implemented` | `adm-new-pipeline::stages::step02::entity_validation_graph_phase` | L5 extraction, synthetic fallback, coverage, graph cycle and phase classification tests | pipeline stage02 gate | decided |
| `pipeline\step_02_design_review_freeze\plugin.py` | `implemented` | `adm-new-pipeline::stages::step02::plugin` | multi-source group import and output application tests | pipeline stage02 gate | decided |
| `pipeline\step_02_design_review_freeze\supplement.py` | `implemented` | `adm-new-pipeline::stages::step02::l5_entity_supplement + adm-new-ai::model_task_bridge` | request hash/cache/model call/fallback/merge tests with mocked adapter | pipeline stage02 gate | decided |
| `pipeline\step_02_design_review_freeze\supplement_contracts.py` | `implemented` | `adm-new-pipeline::stages::step02::supplement_contracts` | SupplementRequest/Result serde, valid kind and node mapping tests | pipeline stage02 gate | decided |
| `pipeline\step_02_design_review_freeze\supplement_entities.py` | `implemented` | `adm-new-pipeline::stages::step02::supplement_entities` | validate/normalize/parse/merge/dedupe tests | pipeline stage02 gate | decided |
| `pipeline\step_03_program_requirements\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_03_program_requirements\binding.py` | `implemented` | `adm-new-pipeline::stages::step03::requirement_binding` | dependency/source/semantic/default binding tests | pipeline stage03 gate | decided |
| `pipeline\step_03_program_requirements\contract_builder.py` | `implemented` | `adm-new-pipeline::stages::step03::program_requirements_contract` | standard contract schema/source coverage/path binding/quality tests | pipeline stage03 gate | decided |
| `pipeline\step_03_program_requirements\helpers.py` | `implemented` | `adm-new-pipeline::stages::step03::entity_to_requirement` | multi-requirement templates, system binding and quality report tests | pipeline stage03 gate | decided |
| `pipeline\step_03_program_requirements\plugin.py` | `implemented` | `adm-new-pipeline::stages::step03::plugin` | ProgReq source import and output application tests | pipeline stage03 gate | decided |
| `pipeline\step_04_art_requirements\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_04_art_requirements\contract_builder.py` | `implemented` | `adm-new-pipeline::stages::step04::art_requirements_contract` | visual contract, UX binding, drift check, path binding and quality tests | pipeline stage04 gate | decided |
| `pipeline\step_04_art_requirements\helpers.py` | `implemented` | `adm-new-pipeline::stages::step04::entity_to_asset_and_market_reference` | multi-asset conversion, P0 specs and local market reference tests | pipeline stage04 gate | decided |
| `pipeline\step_04_art_requirements\plugin.py` | `implemented` | `adm-new-pipeline::stages::step04::plugin` | ArtReq source import and output application tests | pipeline stage04 gate | decided |
| `pipeline\step_05_program_review\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_05_program_review\helpers.py` | `implemented` | `adm-new-pipeline::stages::step05::intelligent_reviewer` | placeholder, trace, binding, acceptance, severity and verdict tests | pipeline stage05 gate | decided |
| `pipeline\step_05_program_review\plugin.py` | `implemented` | `adm-new-pipeline::stages::step05::plugin` | ProgReview source import and output application tests | pipeline stage05 gate | decided |
| `pipeline\step_06_art_review\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_06_art_review\plugin.py` | `implemented` | `adm-new-pipeline::stages::step06::plugin` | ArtReview source import and output application tests | pipeline stage06 gate | decided |
| `pipeline\step_07_art_style_generation\__init__.py` | `drop_with_reason` | none | docstring-only package marker | disposition review | decided |
| `pipeline\step_07_art_style_generation\plugin.py` | `implemented` | `adm-new-pipeline::stages::step07::style_generation_plugin + adm-new-application::style_confirmation_resume` | confirmation resume, legacy copy and style option guard tests | pipeline stage07 gate | decided |
| `pipeline\step_08_design_to_plan\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_08_design_to_plan\plugin.py` | `implemented` | `adm-new-pipeline::stages::step08::plugin` | Plans source import and output application tests | pipeline stage08 gate | decided |
| `pipeline\step_09_art_plan\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_09_art_plan\plugin.py` | `implemented` | `adm-new-pipeline::stages::step09::plugin` | ArtPlans source import and output application tests | pipeline stage09 gate | decided |
| `pipeline\step_10_asset_alignment\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_10_asset_alignment\plugin.py` | `implemented` | `adm-new-pipeline::stages::step10::plugin` | Alignment source import and output application tests | pipeline stage10 gate | decided |
| `pipeline\step_11_dev_execution\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_11_dev_execution\plugin.py` | `implemented` | `adm-new-pipeline::stages::step11::plugin` | DevExecution source import and output application tests | pipeline stage11 gate | decided |
| `pipeline\step_12_art_production\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_12_art_production\plugin.py` | `implemented` | `adm-new-pipeline::stages::step12::plugin` | ArtProduction source import and output application tests | pipeline stage12 gate | decided |
| `pipeline\step_13_scene_assembly\plugin.py` | `implemented` | `adm-new-pipeline::stages::step13::scene_assembly_plugin` | SceneAssembly import, standalone metadata and output application tests | pipeline stage13 gate | decided |
| `pipeline\step_14_integration_validation\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_14_integration_validation\plugin.py` | `implemented` | `adm-new-pipeline::stages::step14::integration_validation_plugin` | completed_with_review blocker, config override and standalone partial tests | pipeline stage14 gate | decided |
| `pipeline\step_d1_project_portrait\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_d1_project_portrait\plugin.py` | `implemented` | `adm-new-pipeline::design_flow::d1_project_portrait` | config/project setting portrait and design summary tests | pipeline D-flow gate | decided |
| `pipeline\step_d2_design_decisions\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_d2_design_decisions\plugin.py` | `implemented` | `adm-new-pipeline::design_flow::d2_design_decisions` | autosave state, completion summary and decision markdown/report tests | pipeline D-flow gate | decided |
| `pipeline\step_d3_design_validation\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_d3_design_validation\plugin.py` | `implemented` | `adm-new-pipeline::design_flow::d3_design_validation` | profile blocker, archetype warning, contract blocker and gate report tests | pipeline D-flow gate | decided |
| `pipeline\step_d4_devflow_handoff\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `pipeline\step_d4_devflow_handoff\plugin.py` | `implemented` | `adm-new-pipeline::design_flow::d4_devflow_handoff + adm-new-design::concept_package_export` | structured handoff validation blocked/passed propagation tests | pipeline D-flow gate | decided |
| `tools\build\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `tools\build\build.py` | `cli_tool_port` | `adm-new-cli dist build + NEWrust xtask::dist_build` | release build command and Tauri/Rust dist orchestration tests | release build gate | decided |
| `tools\build\verify_build.py` | `cli_tool_port` | `adm-new-cli dist verify-bundle + NEWrust/gates::release_bundle` | bundle existence, size and required resource inclusion tests | release build gate | decided |
| `tools\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `tools\asset_production\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `tools\asset_production\audio_placeholder.py` | `cli_tool_port` | `adm-new-cli asset audio-placeholder + adm-new-artifact::asset_tools::audio_placeholder` | WAV duration/sample-rate/channel/size and explicit placeholder-report tests | asset tool gate | decided |
| `tools\asset_production\codex_image_tool.py` | `cli_tool_port` | `adm-new-cli asset codex-image + adm-new-ai::codex_image_generator` | Codex command discovery, generated image snapshot, output/session path parsing, PNG copy collision and failure tests | AI asset gate | decided |
| `tools\asset_production\image_api_config.py` | `implemented` | `adm-new-ai::image_provider_config` | active provider, legacy relay fallback, env fallback, base URL normalization and secret masking tests | AI config gate | decided |
| `tools\asset_production\image_api_probe.py` | `cli_tool_port` | `adm-new-cli image probe + adm-new-ai::image_probe` | responses stream, images_generations JSON, masked report, image write and exit-code tests | AI image gate | decided |
| `tools\asset_production\image_metadata_checker.py` | `cli_tool_port` | `adm-new-cli asset image-metadata-check + adm-new-artifact::asset_tools::image_metadata` | missing file, width/height/format mismatch and PASS tests | asset validation gate | decided |
| `tools\asset_production\image_tool.py` | `cli_tool_port` | `adm-new-cli image generate + adm-new-ai::responses_image_generation` | streamed image_generation_call extraction, base64 decode and file save tests | AI image gate | decided |
| `tools\asset_production\localization_injector.py` | `cli_tool_port` | `adm-new-cli unity localization-inject + adm-new-application::unity_codegen::localization` | LocalizationManager generation, Chinese string extraction, idempotence and parser limitation tests | Unity codegen gate | decided |
| `tools\asset_production\sfx_tool.py` | `cli_tool_port` | `adm-new-cli asset sfx-placeholder + adm-new-artifact::asset_tools::sfx_placeholder` | placeholder WAV duration/sample-rate tests and report marking non-real generation | asset tool gate | decided |
| `tools\asset_production\sprite_atlas_packer.py` | `cli_tool_port` | `adm-new-cli asset pack-sprite-atlas + adm-new-artifact::asset_tools::sprite_atlas` | frame generation adapter mock, atlas layout, metadata schema and broken legacy import regression tests | asset tool gate | decided |
| `tools\asset_production\sprite_slicer.py` | `cli_tool_port` | `adm-new-cli asset slice-sprite + adm-new-artifact::asset_tools::sprite_slicer` | grid/cell/gap crop count and output naming tests | asset tool gate | decided |
| `tools\config\migrate_ai_config.py` | `cli_tool_port` | `adm-new-cli config migrate-ai + adm-new-config::legacy_ai_migration` | legacy profile/api/app/project settings migration, backup, no-op and log tests | config migration gate | decided |
| `tools\design\fill_template_gameplay_systems.py` | `cli_tool_port` | `adm-new-cli design fill-template-gameplay-systems + adm-new-design::template_gameplay_systems` | option id validation, normalized weights, core loop formatting and template update tests | design data gate | decided |
| `tools\design\rebuild_builtin_project_templates.py` | `cli_tool_port` | `adm-new-cli design rebuild-builtin-templates + adm-new-design::builtin_template_builder` | base template discovery, 25 template generation, archive manifest, index and profile consistency tests | design data gate | decided |
| `tools\dev\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `tools\dev\config_compiler.py` | `cli_tool_port` | `adm-new-cli unity compile-config + adm-new-contracts::config_table_compiler` | CSV coercion, JSON output and C# struct generation tests | config/codegen gate | decided |
| `tools\dev\error_logger_generator.py` | `cli_tool_port` | `adm-new-cli unity inject-error-logger + adm-new-application::unity_codegen::error_logger` | ErrorLogger.cs output, lifecycle method injection/idempotence and limitation-report tests | Unity codegen gate | decided |
| `tools\dev\git_tool.py` | `cli_tool_port` | `adm-new-cli dev git-safe + adm-new-foundation::safe_git` | allowed verb list, push rejection, work_dir binding and timeout tests | dev tool gate | decided |
| `tools\dev\perf_profiler_generator.py` | `cli_tool_port` | `adm-new-cli unity inject-perf + adm-new-application::unity_codegen::perf_profiler` | PerfMonitor/PerfHUD generation, probe injection/idempotence and limitation-report tests | Unity codegen gate | decided |
| `tools\dev\scaffold.py` | `cli_tool_port` | `adm-new-cli project scaffold + adm-new-application::project_scaffold` | scaffold directory/template creation, non-empty warning and NEWrust template replacement tests | project scaffold gate | decided |
| `tools\dev\scaffold_step.py` | `cli_tool_port` | `adm-new-cli pipeline scaffold-step + adm-new-pipeline::step_scaffold` | step folder, plugin/helper/prompt/data generation and force behavior tests | pipeline scaffold gate | decided |
| `tools\dev\test_generator.py` | `cli_tool_port` | `adm-new-cli unity generate-tests + adm-new-governance::unity_test_generator` | plan module discovery, NUnit stub output and SKIPPED-not-PASS report tests | test tooling gate | decided |
| `tools\dev\ui_state_generator.py` | `cli_tool_port` | `adm-new-cli unity generate-ui-state + adm-new-application::unity_codegen::ui_state` | UI graph parser, UIManager.cs output and state markdown tests | Unity codegen gate | decided |
| `tools\memory\check_staleness.py` | `cli_tool_port` | `adm-new-cli memory check-staleness + adm-new-knowledge::freshness_index` | fresh/stale/missing/hash mismatch and stale exit-code tests | knowledge freshness gate | decided |
| `tools\memory\update_freshness.py` | `cli_tool_port` | `adm-new-cli memory update-freshness + adm-new-knowledge::freshness_index` | key-file hash snapshot, missing-file warning and timestamp tests | knowledge freshness gate | decided |
| `tools\patch\__init__.py` | `drop_with_reason` | none | no behavior; CLI package marker only | disposition review | decided |
| `tools\patch\manager.py` | `cli_tool_port` | `adm-new-cli patch analyze/list/show/validate/apply/promote + adm-new-patch` | every subcommand, unknown patch, runner failure and promoted metadata tests | patch gate | decided |
| `tools\repair_scene_assembly.py` | `cli_tool_port` | `adm-new-cli maintenance repair-scene-assembly + adm-new-pipeline::repairs::scene_assembly` | draft/save resolution, current-save guard, Step13 rebuild and downstream invalidation tests | maintenance gate | decided |
| `tools\repair_step11_eo_states.py` | `cli_tool_port` | `adm-new-cli maintenance repair-step11-eo-states + adm-new-application::execution_object_repairs` | repairable states, output-file verification, report refresh and snapshot sync tests | maintenance gate | decided |
| `tools\save\audit_parallel_isolation.py` | `cli_tool_port` | `adm-new-cli save audit-parallel-isolation + adm-new-save::parallel_isolation_audit` | draft/save/source/artifact mismatch detection and report status exit-code tests | save gate | decided |
| `tools\save\repair_blank_save_progress.py` | `cli_tool_port` | `adm-new-cli save repair-blank-progress + adm-new-save::blank_save_repair` | dry-run/apply, safe workspace cleanup, progress reset and repair log tests | save gate | decided |
| `tools\save\repair_parallel_save_contamination.py` | `cli_tool_port` | `adm-new-cli save repair-parallel-contamination + adm-new-save::parallel_contamination_repair` | run_context-proven dry-run/apply, missing save skip and no artifact movement tests | save gate | decided |
| `tools\scripts\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `tools\scripts\check_hardcoded_paths.py` | `cli_tool_port` | `adm-new-cli governance check-hardcoded-paths + adm-new-governance::legacy_path_scan` | pattern hit, skip directory, suffix filter and docs allowlist tests | governance gate | decided |
| `tools\scripts\export_concept_package.py` | `cli_tool_port` | `adm-new-cli design export-concept-package + adm-new-design::concept_package_export` | target-dir, workspace mirror toggle and stale import replacement tests | design export gate | decided |
| `tools\scripts\inspect_reports.py` | `cli_tool_port` | `adm-new-cli pipeline inspect-reports + adm-new-pipeline::report_inspector` | missing/invalid report, artifact layer and all/one step output tests | pipeline diagnostic gate | decided |
| `tools\scripts\migrate_design_projects_to_execution_objects.py` | `cli_tool_port` | `adm-new-cli migrate design-projects-to-execution-objects + adm-new-application::execution_object_migrations` | project discovery, dry-run, backup/delete options and error aggregation tests | migration gate | decided |
| `tools\scripts\migrate_execution_objects_add_save_id.py` | `cli_tool_port` | `adm-new-cli migrate execution-objects-add-save-id + adm-new-save::execution_object_store_migration` | missing/consistent/mismatch/missing_save_id, backup, dry-run/apply and markdown report tests | migration gate | decided |
| `tools\scripts\migrate_legacy.py` | `cli_tool_port` | `adm-new-cli migrate legacy-copy + adm-new-application::legacy_migration` | dry-run default, exclusions, copy apply and migration report tests | migration gate | decided |
| `tools\scripts\schema_migrator.py` | `cli_tool_port` | `adm-new-cli schema migrate + adm-new-contracts::schema_migrator` | structured file load, wrap_to_object rule and version update tests | contract migration gate | decided |
| `tools\sdk\__init__.py` | `drop_with_reason` | none | no behavior; CLI package marker only | disposition review | decided |
| `tools\sdk\manager.py` | `cli_tool_port` | `adm-new-cli sdk init/list/show/add/review/context/sync + adm-new-sdk` | all SDK subcommands, missing spec and AI extraction sync failure tests | SDK gate | decided |
| `tools\validators\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `tools\validators\compile_checker.py` | `cli_tool_port` | `adm-new-cli validate compile + adm-new-governance::compile_checker` | default Unity command/custom command, timeout and stdout/stderr PASS/FAIL tests | validation gate | decided |
| `tools\validators\config_validator.py` | `cli_tool_port` | `adm-new-cli validate config-table + adm-new-contracts::config_table_validator` | required/deprecated/type/unique/range and missing schema/table tests | validation gate | decided |
| `tools\validators\context_lint.py` | `cli_tool_port` | `adm-new-cli validate context-lint + adm-new-governance::context_lint` | required sections, term outside section, missing ADR and coverage threshold tests | validation gate | decided |
| `tools\validators\contract_validator.py` | `cli_tool_port` | `adm-new-cli validate contract + adm-new-contracts::schema_validator` | type/required/properties/items/enum/anyOf and report writer tests | validation gate | decided |
| `tools\validators\design_semantic_quality.py` | `cli_tool_port` | `adm-new-cli validate design-semantic-quality + adm-new-governance::design_semantic_quality` | nullish/generic/placeholder/template leakage/signature metrics and report tests | validation gate | decided |
| `tools\validators\environment_checker.py` | `cli_tool_port` | `adm-new-cli validate environment + adm-new-application::environment_checker` | Unity/.NET/Python/tool checks, parser fallback and optional install permission boundary tests | validation gate | decided |
| `tools\validators\output_validator.py` | `cli_tool_port` | `adm-new-cli validate output + adm-new-foundation::agent_output_validator` | rejection phrase, fenced JSON, required keys, Crew output and exit-code tests | validation gate | decided |
| `tools\validators\pipeline_quality.py` | `cli_tool_port` | `adm-new-cli validate pipeline-quality + adm-new-governance::pipeline_quality` | artifacts discovery, metric collection and PLAN-002 pass/fail tests | validation gate | decided |
| `sitecustomize.py` | `absorbed` | `adm-new-foundation::runtime_cache_policy` | cache redirect policy test | foundation gate | decided |
| `core\packaging\__init__.py` | `absorbed` | `adm-new-packaging::public_api` | public API export coverage | packaging gate | decided |
| `core\packaging\manifest.py` | `implemented` | `adm-new-packaging::manifest` | package output dir and manifest field tests | packaging gate | decided |
| `core\packaging\service.py` | `implemented` | `adm-new-packaging::service` | package report/notes/manifest writer tests | packaging gate | decided |
| `core\packaging\validation.py` | `implemented` | `adm-new-packaging::validation` | Step14 source loading and readiness blocker tests | packaging gate | decided |
| `core\patch\__init__.py` | `absorbed` | `adm-new-patch::public_api` | public API export coverage | patch gate | decided |
| `core\patch\analyzer.py` | `implemented` | `adm-new-patch::analyzer` | completion JSON analysis and route selection tests | patch gate | decided |
| `core\patch\codex_runner.py` | `implemented` | `adm-new-patch::codex_runner` | expected-file allowlist and changed hash detection tests | patch gate | decided |
| `core\patch\executor.py` | `implemented` | `adm-new-patch::executor` | apply/validate success and failure lifecycle tests | patch gate | decided |
| `core\patch\light_validator.py` | `implemented` | `adm-new-patch::light_validator` | missing file, bracket balance, risky keyword warning tests | patch gate | decided |
| `core\patch\record.py` | `implemented` | `adm-new-patch::record_store` | PatchTask/PatchRecord serialization and PatchStore list/read/write tests | patch gate | decided |
| `core\runtime\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\runtime\control.py` | `implemented` | `adm-new-application::runtime_control` | stop request, stale clear, run state, mark stopped tests | runtime gate | decided |
| `core\runtime\execution_config.py` | `implemented` | `adm-new-application::execution_config` | bounded config defaults and override tests | runtime gate | decided |
| `core\runtime\execution_planner.py` | `implemented` | `adm-new-application::execution_planner` | write set conflict batch and readiness report tests | runtime gate | decided |
| `core\runtime\execution_state.py` | `implemented` | `adm-new-application::execution_state` | snapshot cloning and concurrent append tests | runtime gate | decided |
| `core\runtime\guard.py` | `cli_tool_port` | `adm-new-cli guard-forbidden-runtime-refs` | forbidden runtime scan CLI test | CLI gate | decided |
| `core\runtime\locks.py` | `implemented` | `adm-new-application::runtime_locks` | exclusive lock, owner mismatch release, path hashing tests | runtime gate | decided |
| `core\runtime\pipeline_state.py` | `implemented` | `adm-new-pipeline::pipeline_state` | structured state read/write and invalid status tests | pipeline gate | decided |
| `core\runtime\preflight.py` | `implemented` | `adm-new-application::development_preflight` | Unity/custom/unreal/godot preflight blocker/warning tests | runtime gate | decided |
| `core\runtime\run_context.py` | `implemented` | `adm-new-application::run_context` | run context creation/load/env binding/settings snapshot/mismatch tests | runtime gate | decided |
| `core\sdk\__init__.py` | `absorbed` | `adm-new-sdk::public_api` | public API export coverage | SDK gate | decided |
| `core\sdk\ai_extractor.py` | `implemented` | `adm-new-sdk::ai_extractor` | HTML readable extraction and completion result mapping tests | SDK gate | decided |
| `core\sdk\knowledge_base.py` | `implemented` | `adm-new-sdk::knowledge_base` | index/spec store, review status, approved prompt context tests | SDK gate | decided |
| `core\source\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\source\finder.py` | `implemented` | `adm-new-pipeline::source::finder` | source root precedence, id inference, latest/all selection tests | source gate | decided |
| `core\source\folder_manager.py` | `implemented` | `adm-new-pipeline::source::folder_manager` | versioned folder creation, correction merge, cleanup, design path tests | source gate | decided |
| `core\source\groups.py` | `implemented` | `adm-new-pipeline::source::groups` | marker/source type mapping tests | source gate | decided |
| `core\source\importer.py` | `implemented` | `adm-new-pipeline::source::importer` | import reports, upstream refs, reference manifest, forbidden runtime scan tests | source gate | decided |
| `core\source\snapshot.py` | `implemented` | `adm-new-pipeline::source::snapshot` | snapshot manifest/hash/list/restore dry-run tests | source gate | decided |
| `core\save\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\save\manager.py` | `implemented` | `adm-new-save::manager` | active draft/formal archive/workspace lifecycle, save index, locks, file map, snapshot, sync, load/delete tests | save gate | decided |
| `core\iteration\__init__.py` | `absorbed` | `adm-new-application::iteration::public_api` | public API export coverage | iteration gate | decided |
| `core\iteration\artifact_inheritor.py` | `implemented` | `adm-new-application::iteration::artifact_inheritor` | skipped stage copy/hash/sidecar tests | iteration gate | decided |
| `core\iteration\delta_scheduler.py` | `implemented` | `adm-new-application::iteration::delta_scheduler` | conservative rerun/skip and dependency promotion tests | iteration gate | decided |
| `core\iteration\spec_parser.py` | `implemented` | `adm-new-application::iteration::spec_parser` | standard/simplified spec parsing and validation tests | iteration gate | decided |
| `core\utils\__init__.py` | `drop_with_reason` | none | no behavior; package marker only | disposition review | decided |
| `core\utils\base_tool.py` | `absorbed` | `adm-new-foundation::tool_trait` | run/call dispatch tests where compatibility remains needed | foundation gate | decided |
| `core\utils\md_parser.py` | `implemented` | `adm-new-foundation::markdown_parser` | headings/lists/tables/JSON/YAML/required-key parser tests | foundation gate | decided |
| `core\utils\process_utils.py` | `absorbed` | `adm-new-foundation::process_env` | child env and Windows hidden process option tests | foundation gate | decided |
| `core\utils\structured_md.py` | `implemented` | `adm-new-foundation::structured_markdown` | fenced JSON/YAML load/write and fallback parser tests | foundation gate | decided |
| `core\utils\text_extractor.py` | `cli_tool_port` | `adm-new-cli extract-translation-strings` | UI graph/config schema extraction tests | CLI gate | decided |
| `core\utils\yaml_compat.py` | `implemented` | `adm-new-foundation::yaml_compat` | YAML/JSON fallback load/dump tests | foundation gate | decided |

## 4. 当前同步状态

```text
decided_file_mappings=379
pending_file_mappings=0
status=file_mapping_synced_but_not_final_passed
reason=orphan_reachability_and_multirole_score_gates_remain_before_v3_atomic_development
```
