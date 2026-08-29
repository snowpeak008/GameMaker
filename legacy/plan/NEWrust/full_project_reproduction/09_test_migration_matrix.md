# Test Migration Matrix

????? `04_rust_target_mapping.md` ?? 73/73 ? Python ????????? v3 ???????????????? `04_rust_target_mapping.md` ???

## 1. ??

`core/tests` ? 73 ? Python ????????? Rust/Web/gate ??????????????

## 2. ??????

| Python test type | Rust/Web target |
| --- | --- |
| contract/schema tests | `adm-new-contracts` unit tests |
| storage/save tests | `adm-new-storage` / `adm-new-save` tests |
| design engine tests | `adm-new-design` tests |
| pipeline tests | `adm-new-pipeline` / `adm-new-artifact` tests |
| UI import/panel tests | Web e2e + Tauri command tests |
| adapter tests | `adm-new-ai` tests |
| tooling tests | `adm-new-cli` gate tests |

## 3. File-Level Mapping

| Python test file | Rust/Web/gate target | required evidence | gate | status |
| --- | --- | --- | --- | --- |
| `core\tests\conftest.py` | `Rust/Web test fixtures and isolated config temp roots` | fixture isolation and cache reset coverage | test gate | `decided` |
| `core\tests\integration\test_adapter_configuration.py` | `adm-new-ai integration tests` | adapter profile construction and active pipeline adapter tests | test gate | `decided` |
| `core\tests\integration\test_design_semantic_pipeline.py` | `NEWrust/gates::design_semantic_pipeline + adm-new-pipeline integration tests` | Step00-Step10 semantic output and low quality failure tests | test gate | `decided` |
| `core\tests\integration\test_parallel_project_semantic_isolation.py` | `adm-new-save + adm-new-design integration isolation tests` | parallel draft context and project signature isolation tests | test gate | `decided` |
| `core\tests\integration\test_plugins.py` | `adm-new-pipeline::registry integration tests` | plugin manifest, design stage and development import tests | test gate | `decided` |
| `core\tests\unit\test_ai_config.py` | `adm-new-config::ai_config tests + adm-new-web::dialogs::ai_config tests` | v3 config/default/migration/dialog type tests | test gate | `decided` |
| `core\tests\unit\test_ai_design_asset_spec.py` | `adm-new-ai::asset_spec_gate + adm-new-design::art_pipeline tests` | consumable asset spec, schema, Stage04 builder and Stage12 preflight tests | test gate | `decided` |
| `core\tests\unit\test_ai_design_completion_service.py` | `adm-new-ai::completion_json_service tests` | plain/fenced JSON parse and retry/error tests | test gate | `decided` |
| `core\tests\unit\test_ai_design_contracts.py` | `adm-new-ai::contract_gate + traceability + prompt_library tests` | contract validation, prompt coverage and task traceability tests | test gate | `decided` |
| `core\tests\unit\test_art_task_semanticization.py` | `adm-new-design::task_semanticizer tests` | art task usage prompt and semantic roles tests | test gate | `decided` |
| `core\tests\unit\test_art_taxonomy_builder.py` | `adm-new-design::art_taxonomy tests` | required category coverage tests | test gate | `decided` |
| `core\tests\unit\test_artifact_registry_playable_chain.py` | `adm-new-artifact::registry tests` | playable chain registration/schema visibility tests | test gate | `decided` |
| `core\tests\unit\test_artifact_validator_paths.py` | `adm-new-artifact::validator path resolution tests` | draft/current-save contract path and missing checked path tests | test gate | `decided` |
| `core\tests\unit\test_codex_image_tool.py` | `adm-new-ai::image_generation codex CLI tests` | PNG detection and stdout saved/session path parser tests | test gate | `decided` |
| `core\tests\unit\test_config_loader.py` | `adm-new-config::loader tests` | app/project settings, endpoint normalization and AI image routing tests | test gate | `decided` |
| `core\tests\unit\test_config_validator.py` | `adm-new-config::validator tests` | OpenAI/CLI/image profile validation tests | test gate | `decided` |
| `core\tests\unit\test_core_paths.py` | `adm-new-foundation::paths tests` | root marker/design data/session draft path tests | test gate | `decided` |
| `core\tests\unit\test_customization_scorer.py` | `adm-new-design::semantic_quality tests` | blocker score report tests | test gate | `decided` |
| `core\tests\unit\test_d2_real_decision_report.py` | `adm-new-pipeline::step_d2_design_decisions tests` | incomplete/unstructured decision report tests | test gate | `decided` |
| `core\tests\unit\test_d3_design_gate.py` | `adm-new-pipeline::step_d3_design_validation tests` | D3 blockers and test-mode report tests | test gate | `decided` |
| `core\tests\unit\test_design_node_requirement_metadata.py` | `adm-new-design::engine metadata tests` | requirement metadata/provenance/completion blocker tests | test gate | `decided` |
| `core\tests\unit\test_design_requirements_archetype_detector.py` | `adm-new-design::requirements archetype detector tests` | subtype/generic fallback and stable contract tests | test gate | `decided` |
| `core\tests\unit\test_design_requirements_archetype_subtypes.py` | `adm-new-design::requirements subtype tests` | subtype data/parent requirement/signal combination tests | test gate | `decided` |
| `core\tests\unit\test_design_semantic_quality.py` | `adm-new-application::semantic_quality tests` | placeholder/generic/signature/report serialization tests | test gate | `decided` |
| `core\tests\unit\test_design_semantic_schema_registry.py` | `adm-new-artifact::schema_registry tests` | semantic schemas, refs and Stage06 registration tests | test gate | `decided` |
| `core\tests\unit\test_draft_archive_paths.py` | `adm-new-save::manager + app shell lifecycle tests` | draft/archive/workspace/lock/snapshot/EO/save UI tests | test gate | `decided` |
| `core\tests\unit\test_execution_planner.py` | `adm-new-application::execution_planner tests` | write-set conflicts, readiness and execution state tests | test gate | `decided` |
| `core\tests\unit\test_hades_quality_optimization.py` | `adm-new-pipeline::optimization_helpers + adm-new-design::art_pipeline tests` | template coverage, task cleanup, Stage04/07/09 and image skip tests | test gate | `decided` |
| `core\tests\unit\test_iteration_cli.py` | `adm-new-cli iteration tests` | iteration CLI coverage | test gate | `decided` |
| `core\tests\unit\test_iteration_development.py` | `adm-new-application::iteration tests` | spec parser, delta scheduler, artifact inheritance and plan merge tests | test gate | `decided` |
| `core\tests\unit\test_l5_supplement.py` | `adm-new-design::entity_supplement + adm-new-ai tests` | L5 supplement trigger/cache/fallback/merge and Stage02 metadata tests | test gate | `decided` |
| `core\tests\unit\test_manual_style_confirmation.py` | `adm-new-pipeline::stage07_style + adm-new-web::style dialogs tests` | style options, prompt override, confirmation, image generation and dispatch tests | test gate | `decided` |
| `core\tests\unit\test_model_adapters.py` | `adm-new-ai::providers tests` | Codex/Claude/completion/OpenAI provider behavior tests | test gate | `decided` |
| `core\tests\unit\test_open_questions_contract.py` | `adm-new-design::open_questions tests` | archetype questions and unresolved blocker tests | test gate | `decided` |
| `core\tests\unit\test_parallel_runtime_isolation.py` | `adm-new-save + adm-new-application::run_context isolation tests` | bound save/source/EO/project settings/Unity lock isolation tests | test gate | `decided` |
| `core\tests\unit\test_patch_channel.py` | `adm-new-patch + adm-new-cli patch tests` | patch analyzer/validator/executor/runner/CLI promote tests | test gate | `decided` |
| `core\tests\unit\test_pipeline_optimization_helpers.py` | `adm-new-pipeline::optimization_helpers tests` | Step00-04 inference/binding/assets/reviewer/quality/source tests | test gate | `decided` |
| `core\tests\unit\test_pipeline_registry_schema_generation_contracts.py` | `adm-new-pipeline::registry + adm-new-artifact::schema_registry tests` | Stage03-09 schema generation contract tests | test gate | `decided` |
| `core\tests\unit\test_playable_contracts.py` | `adm-new-design::playable_contracts + pipeline stage tests` | playable bundle/gate/Stage02/08/12 tests | test gate | `decided` |
| `core\tests\unit\test_program_capability_builder.py` | `adm-new-design::program_capabilities tests` | semantic binding and action state-change blocker tests | test gate | `decided` |
| `core\tests\unit\test_project_dna_builder.py` | `adm-new-design::project_dna tests` | DNA freeze/open-question/playable coverage tests | test gate | `decided` |
| `core\tests\unit\test_project_templates.py` | `adm-new-design::project_templates tests` | custom template delete behavior tests | test gate | `decided` |
| `core\tests\unit\test_pytest_config.py` | `NEWrust/gates::test_environment tests` | temp cleanup/cache/pycache policy tests | test gate | `decided` |
| `core\tests\unit\test_reference_manifest_refresh.py` | `adm-new-pipeline::source::importer tests` | reference manifest refresh tests | test gate | `decided` |
| `core\tests\unit\test_run_state_failure.py` | `adm-new-application::runtime_control + adm-new-pipeline::orchestrator tests` | run state replace/clear and failure mark tests | test gate | `decided` |
| `core\tests\unit\test_sdk_knowledge_base.py` | `adm-new-sdk tests + adm-new-cli sdk tests` | SDK index/context/CLI and readable extraction tests | test gate | `decided` |
| `core\tests\unit\test_semantic_alignment.py` | `adm-new-design::semantic_alignment tests` | program/art coverage and placeholder blocker tests | test gate | `decided` |
| `core\tests\unit\test_stage11_parent_reuse_parallel.py` | `adm-new-application::stage11_executor + execution planner tests` | parent reuse, scoped snapshots, audit and parallel write tests | test gate | `decided` |
| `core\tests\unit\test_step00_project_identity.py` | `adm-new-pipeline::step00 tests` | project identity/DNA seed and draft signature isolation tests | test gate | `decided` |
| `core\tests\unit\test_step00_structured_profile_input.py` | `adm-new-pipeline::step00 tests` | structured profile override tests | test gate | `decided` |
| `core\tests\unit\test_step01_structured_gameplay_framework.py` | `adm-new-pipeline::step01 tests` | structured framework and contract consumption tests | test gate | `decided` |
| `core\tests\unit\test_step02_freezes_playable_contracts.py` | `adm-new-pipeline::step02 tests` | playable candidate freeze and UI flow blocker tests | test gate | `decided` |
| `core\tests\unit\test_step02_project_dna_freeze.py` | `adm-new-pipeline::step02 + adm-new-design::project_dna tests` | frozen Project DNA contract write tests | test gate | `decided` |
| `core\tests\unit\test_step03_program_requirements_contract_schema.py` | `adm-new-pipeline::step03 contract builder tests` | schema-valid passed/blocked program requirement contracts | test gate | `decided` |
| `core\tests\unit\test_step03_program_requirements_from_contracts.py` | `adm-new-pipeline::step03 tests` | requirements from contracts and missing contract blocker tests | test gate | `decided` |
| `core\tests\unit\test_step04_asset_requirements_from_contracts.py` | `adm-new-pipeline::step04 tests` | asset requirements and mount contract blocker tests | test gate | `decided` |
| `core\tests\unit\test_step05_optimization.py` | `adm-new-pipeline::step05 + requirement binding tests` | freeze contents, binding fallback and supplement trigger tests | test gate | `decided` |
| `core\tests\unit\test_step05_to_step09_structured_contract_chain.py` | `adm-new-pipeline::steps05_09 contract chain tests` | structured contract chain and Step07 test mode confirmation tests | test gate | `decided` |
| `core\tests\unit\test_step08_program_plan_from_playable_contracts.py` | `adm-new-pipeline::step08 tests` | program plan outputs and missing UI flow blocker tests | test gate | `decided` |
| `core\tests\unit\test_step10_to_step12_structured_contract_chain.py` | `adm-new-pipeline::steps10_12 contract chain tests` | alignment/mount readiness/Stage11 missing inputs/Stage12 report tests | test gate | `decided` |
| `core\tests\unit\test_step11_eo_state_closure.py` | `adm-new-application::execution_objects + adm-new-cli repair tests` | reuse guard, reverify path and repair report tests | test gate | `decided` |
| `core\tests\unit\test_step13_requires_scene_and_ui_contracts.py` | `adm-new-pipeline::step13_scene_assembly tests` | UI contract blocker and static playable structure report tests | test gate | `decided` |
| `core\tests\unit\test_step14_playable_acceptance_contract.py` | `adm-new-pipeline::step14_acceptance tests` | UI root failure, Unity unavailable blocker and acceptance reports | test gate | `decided` |
| `core\tests\unit\test_structured_design_context.py` | `adm-new-design::structured_context tests` | Stage02-first/D4-fallback/missing issue/traceability tests | test gate | `decided` |
| `core\tests\unit\test_structured_handoff_export.py` | `adm-new-design::structured_handoff tests` | manifest decisions and candidate export tests | test gate | `decided` |
| `core\tests\unit\test_structured_logging.py` | `adm-new-application::logging tests` | context source and JSONL roundtrip tests | test gate | `decided` |
| `core\tests\unit\test_style_fit_validator.py` | `adm-new-design::style_fit tests` | style risk and override reason tests | test gate | `decided` |
| `core\tests\unit\test_task_semanticizer.py` | `adm-new-design::task_semanticizer tests` | program task semantic refs tests | test gate | `decided` |
| `core\tests\unit\test_template_l5_expansion.py` | `adm-new-design::project_templates + gameplay systems tests` | template count/L5 sync/archive/gameplay inference tests | test gate | `decided` |
| `core\tests\unit\test_ui_panels_import.py` | `adm-new-web app shell smoke tests` | panel import, app views and step grouping smoke tests | test gate | `decided` |
| `core\tests\unit\test_ui_semantic_reports.py` | `adm-new-web::semantic_quality_panel tests` | semantic report status/return target/classification/dedupe tests | test gate | `decided` |
| `core\tests\unit\test_unattended_recovery.py` | `adm-new-application::unattended_recovery + execution_objects tests` | queue/cursor/resume/dependency/reproduction/remediation/EO ownership tests | test gate | `decided` |
| `core\tests\unit\test_validation_cli.py` | `adm-new-cli package validation tests` | package CLI success and blocked failure tests | test gate | `decided` |

## 4. ???

- ??????Rust ????????? Python ??????? Rust/Web/gate ???????
- ????? gate ???? CI ???????????
- Python ?????????????? NEWrust ?????
