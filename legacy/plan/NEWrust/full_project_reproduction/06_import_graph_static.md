# Static Import and Reachability Index

Generated: 2026-07-09

Status: refreshed static index using direct v3 source-scope scan, not `rg --files`; includes `tools/build/*.py` despite `.gitignore`.

Python file count: 379
Parse error count: 0
Entry candidate count: 161
Static orphan candidate count: 71

## Entry Candidates

| path | reasons |
| --- | --- |
| `conftest.py` | `root_special_entry, pytest_entry` |
| `core/design/option_mapping.py` | `__main__` |
| `core/main.py` | `__main__` |
| `core/plugin_manager.py` | `__main__` |
| `core/runtime/guard.py` | `__main__` |
| `core/tests/conftest.py` | `pytest_entry` |
| `core/tests/integration/test_adapter_configuration.py` | `pytest_entry` |
| `core/tests/integration/test_design_semantic_pipeline.py` | `pytest_entry` |
| `core/tests/integration/test_parallel_project_semantic_isolation.py` | `pytest_entry` |
| `core/tests/integration/test_plugins.py` | `pytest_entry` |
| `core/tests/unit/test_ai_config.py` | `pytest_entry` |
| `core/tests/unit/test_ai_design_asset_spec.py` | `pytest_entry` |
| `core/tests/unit/test_ai_design_completion_service.py` | `pytest_entry` |
| `core/tests/unit/test_ai_design_contracts.py` | `pytest_entry` |
| `core/tests/unit/test_art_task_semanticization.py` | `pytest_entry` |
| `core/tests/unit/test_art_taxonomy_builder.py` | `pytest_entry` |
| `core/tests/unit/test_artifact_registry_playable_chain.py` | `pytest_entry` |
| `core/tests/unit/test_artifact_validator_paths.py` | `pytest_entry` |
| `core/tests/unit/test_codex_image_tool.py` | `pytest_entry` |
| `core/tests/unit/test_config_loader.py` | `pytest_entry` |
| `core/tests/unit/test_config_validator.py` | `pytest_entry` |
| `core/tests/unit/test_core_paths.py` | `pytest_entry` |
| `core/tests/unit/test_customization_scorer.py` | `pytest_entry` |
| `core/tests/unit/test_d2_real_decision_report.py` | `pytest_entry` |
| `core/tests/unit/test_d3_design_gate.py` | `pytest_entry` |
| `core/tests/unit/test_design_node_requirement_metadata.py` | `pytest_entry` |
| `core/tests/unit/test_design_requirements_archetype_detector.py` | `pytest_entry` |
| `core/tests/unit/test_design_requirements_archetype_subtypes.py` | `pytest_entry` |
| `core/tests/unit/test_design_semantic_quality.py` | `pytest_entry` |
| `core/tests/unit/test_design_semantic_schema_registry.py` | `pytest_entry` |
| `core/tests/unit/test_draft_archive_paths.py` | `pytest_entry` |
| `core/tests/unit/test_execution_planner.py` | `pytest_entry` |
| `core/tests/unit/test_hades_quality_optimization.py` | `pytest_entry` |
| `core/tests/unit/test_iteration_cli.py` | `pytest_entry` |
| `core/tests/unit/test_iteration_development.py` | `pytest_entry` |
| `core/tests/unit/test_l5_supplement.py` | `pytest_entry` |
| `core/tests/unit/test_manual_style_confirmation.py` | `pytest_entry` |
| `core/tests/unit/test_model_adapters.py` | `pytest_entry` |
| `core/tests/unit/test_open_questions_contract.py` | `pytest_entry` |
| `core/tests/unit/test_parallel_runtime_isolation.py` | `pytest_entry` |
| `core/tests/unit/test_patch_channel.py` | `pytest_entry` |
| `core/tests/unit/test_pipeline_optimization_helpers.py` | `pytest_entry` |
| `core/tests/unit/test_pipeline_registry_schema_generation_contracts.py` | `pytest_entry` |
| `core/tests/unit/test_playable_contracts.py` | `pytest_entry` |
| `core/tests/unit/test_program_capability_builder.py` | `pytest_entry` |
| `core/tests/unit/test_project_dna_builder.py` | `pytest_entry` |
| `core/tests/unit/test_project_templates.py` | `pytest_entry` |
| `core/tests/unit/test_pytest_config.py` | `pytest_entry` |
| `core/tests/unit/test_reference_manifest_refresh.py` | `pytest_entry` |
| `core/tests/unit/test_run_state_failure.py` | `pytest_entry` |
| `core/tests/unit/test_sdk_knowledge_base.py` | `pytest_entry` |
| `core/tests/unit/test_semantic_alignment.py` | `pytest_entry` |
| `core/tests/unit/test_stage11_parent_reuse_parallel.py` | `pytest_entry` |
| `core/tests/unit/test_step00_project_identity.py` | `pytest_entry` |
| `core/tests/unit/test_step00_structured_profile_input.py` | `pytest_entry` |
| `core/tests/unit/test_step01_structured_gameplay_framework.py` | `pytest_entry` |
| `core/tests/unit/test_step02_freezes_playable_contracts.py` | `pytest_entry` |
| `core/tests/unit/test_step02_project_dna_freeze.py` | `pytest_entry` |
| `core/tests/unit/test_step03_program_requirements_contract_schema.py` | `pytest_entry` |
| `core/tests/unit/test_step03_program_requirements_from_contracts.py` | `pytest_entry` |
| `core/tests/unit/test_step04_asset_requirements_from_contracts.py` | `pytest_entry` |
| `core/tests/unit/test_step05_optimization.py` | `pytest_entry` |
| `core/tests/unit/test_step05_to_step09_structured_contract_chain.py` | `pytest_entry` |
| `core/tests/unit/test_step08_program_plan_from_playable_contracts.py` | `pytest_entry` |
| `core/tests/unit/test_step10_to_step12_structured_contract_chain.py` | `pytest_entry` |
| `core/tests/unit/test_step11_eo_state_closure.py` | `__main__, pytest_entry` |
| `core/tests/unit/test_step13_requires_scene_and_ui_contracts.py` | `pytest_entry` |
| `core/tests/unit/test_step14_playable_acceptance_contract.py` | `pytest_entry` |
| `core/tests/unit/test_structured_design_context.py` | `pytest_entry` |
| `core/tests/unit/test_structured_handoff_export.py` | `pytest_entry` |
| `core/tests/unit/test_structured_logging.py` | `pytest_entry` |
| `core/tests/unit/test_style_fit_validator.py` | `pytest_entry` |
| `core/tests/unit/test_task_semanticizer.py` | `pytest_entry` |
| `core/tests/unit/test_template_l5_expansion.py` | `pytest_entry` |
| `core/tests/unit/test_ui_panels_import.py` | `pytest_entry` |
| `core/tests/unit/test_ui_semantic_reports.py` | `pytest_entry` |
| `core/tests/unit/test_unattended_recovery.py` | `pytest_entry` |
| `core/tests/unit/test_validation_cli.py` | `pytest_entry` |
| `core/ui/gui_app.py` | `__main__` |
| `gui_app.py` | `__main__, root_special_entry` |
| `knowledge/ucos/scripts/__init__.py` | `ucos_script_candidate` |
| `knowledge/ucos/scripts/ucos_init.py` | `__main__, ucos_script_candidate` |
| `knowledge/ucos/scripts/ucos_migrate.py` | `__main__, ucos_script_candidate` |
| `knowledge/ucos/scripts/ucos_query.py` | `__main__, ucos_script_candidate` |
| `knowledge/ucos/scripts/ucos_sync.py` | `__main__, ucos_script_candidate` |
| `knowledge/ucos/scripts/ucos_validate.py` | `__main__, ucos_script_candidate` |
| `pipeline/step_00_idea_intake/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_01_gameplay_framework/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_02_design_review_freeze/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_03_program_requirements/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_04_art_requirements/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_05_program_review/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_06_art_review/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_07_art_style_generation/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_08_design_to_plan/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_09_art_plan/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_10_asset_alignment/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_11_dev_execution/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_12_art_production/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_13_scene_assembly/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_14_integration_validation/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_d1_project_portrait/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_d2_design_decisions/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_d3_design_validation/plugin.py` | `pipeline_registry_dynamic_entry` |
| `pipeline/step_d4_devflow_handoff/plugin.py` | `pipeline_registry_dynamic_entry` |
| `sitecustomize.py` | `root_special_entry` |
| `tools/__init__.py` | `tool_script_candidate` |
| `tools/asset_production/__init__.py` | `tool_script_candidate` |
| `tools/asset_production/audio_placeholder.py` | `tool_script_candidate` |
| `tools/asset_production/codex_image_tool.py` | `tool_script_candidate` |
| `tools/asset_production/image_api_config.py` | `tool_script_candidate` |
| `tools/asset_production/image_api_probe.py` | `__main__, tool_script_candidate` |
| `tools/asset_production/image_metadata_checker.py` | `tool_script_candidate` |
| `tools/asset_production/image_tool.py` | `tool_script_candidate` |
| `tools/asset_production/localization_injector.py` | `tool_script_candidate` |
| `tools/asset_production/sfx_tool.py` | `tool_script_candidate` |
| `tools/asset_production/sprite_atlas_packer.py` | `tool_script_candidate` |
| `tools/asset_production/sprite_slicer.py` | `tool_script_candidate` |
| `tools/build/__init__.py` | `tool_script_candidate` |
| `tools/build/build.py` | `__main__, tool_script_candidate` |
| `tools/build/verify_build.py` | `__main__, tool_script_candidate` |
| `tools/config/migrate_ai_config.py` | `__main__, tool_script_candidate` |
| `tools/design/fill_template_gameplay_systems.py` | `__main__, tool_script_candidate` |
| `tools/design/rebuild_builtin_project_templates.py` | `__main__, tool_script_candidate` |
| `tools/dev/__init__.py` | `tool_script_candidate` |
| `tools/dev/config_compiler.py` | `__main__, tool_script_candidate` |
| `tools/dev/error_logger_generator.py` | `tool_script_candidate` |
| `tools/dev/git_tool.py` | `tool_script_candidate` |
| `tools/dev/perf_profiler_generator.py` | `tool_script_candidate` |
| `tools/dev/scaffold.py` | `__main__, tool_script_candidate` |
| `tools/dev/scaffold_step.py` | `__main__, tool_script_candidate` |
| `tools/dev/test_generator.py` | `tool_script_candidate` |
| `tools/dev/ui_state_generator.py` | `__main__, tool_script_candidate` |
| `tools/memory/check_staleness.py` | `__main__, tool_script_candidate` |
| `tools/memory/update_freshness.py` | `__main__, tool_script_candidate` |
| `tools/patch/__init__.py` | `tool_script_candidate` |
| `tools/patch/manager.py` | `__main__, tool_script_candidate` |
| `tools/repair_scene_assembly.py` | `__main__, tool_script_candidate` |
| `tools/repair_step11_eo_states.py` | `__main__, tool_script_candidate` |
| `tools/save/audit_parallel_isolation.py` | `__main__, tool_script_candidate` |
| `tools/save/repair_blank_save_progress.py` | `__main__, tool_script_candidate` |
| `tools/save/repair_parallel_save_contamination.py` | `__main__, tool_script_candidate` |
| `tools/scripts/__init__.py` | `tool_script_candidate` |
| `tools/scripts/check_hardcoded_paths.py` | `__main__, tool_script_candidate` |
| `tools/scripts/export_concept_package.py` | `__main__, tool_script_candidate` |
| `tools/scripts/inspect_reports.py` | `__main__, tool_script_candidate` |
| `tools/scripts/migrate_design_projects_to_execution_objects.py` | `__main__, tool_script_candidate` |
| `tools/scripts/migrate_execution_objects_add_save_id.py` | `__main__, tool_script_candidate` |
| `tools/scripts/migrate_legacy.py` | `__main__, tool_script_candidate` |
| `tools/scripts/schema_migrator.py` | `__main__, tool_script_candidate` |
| `tools/sdk/__init__.py` | `tool_script_candidate` |
| `tools/sdk/manager.py` | `__main__, tool_script_candidate` |
| `tools/validators/__init__.py` | `tool_script_candidate` |
| `tools/validators/compile_checker.py` | `tool_script_candidate` |
| `tools/validators/config_validator.py` | `__main__, tool_script_candidate` |
| `tools/validators/context_lint.py` | `__main__, tool_script_candidate` |
| `tools/validators/contract_validator.py` | `tool_script_candidate` |
| `tools/validators/design_semantic_quality.py` | `__main__, tool_script_candidate` |
| `tools/validators/environment_checker.py` | `tool_script_candidate` |
| `tools/validators/output_validator.py` | `tool_script_candidate` |
| `tools/validators/pipeline_quality.py` | `__main__, tool_script_candidate` |

## Static Orphan Candidates

These files are not reached by project-local static imports and are not entry candidates. Every row is resolved in `06_orphan_file_decision.md` and remains subject to the final score gate.

| path | required_action |
| --- | --- |
| `core/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/adapters/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/adapters/claude_code_adapter.py` | `resolved_in_06_orphan_file_decision` |
| `core/adapters/codex/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/adapters/codex/result_parser.py` | `resolved_in_06_orphan_file_decision` |
| `core/adapters/memory/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/adapters/memory/context_builder.py` | `resolved_in_06_orphan_file_decision` |
| `core/adapters/memory/token_budget.py` | `resolved_in_06_orphan_file_decision` |
| `core/ai_design/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/art_pipeline/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/artifact/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/artifact/manifest.py` | `resolved_in_06_orphan_file_decision` |
| `core/config/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/design/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/design/engine_data_loader.py` | `resolved_in_06_orphan_file_decision` |
| `core/engines/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/engines/execution_objects/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/engines/execution_objects/user_artifact.py` | `resolved_in_06_orphan_file_decision` |
| `core/engines/execution_objects/workspace_snapshot.py` | `resolved_in_06_orphan_file_decision` |
| `core/engines/handoff_loader.py` | `resolved_in_06_orphan_file_decision` |
| `core/iteration/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/packaging/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/patch/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/runtime/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/save/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/sdk/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/source/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/source/snapshot.py` | `resolved_in_06_orphan_file_decision` |
| `core/ui/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/ui/bottom_panel.py` | `resolved_in_06_orphan_file_decision` |
| `core/ui/workbench.py` | `resolved_in_06_orphan_file_decision` |
| `core/utils/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `core/utils/text_extractor.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/adapters/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/adapters/api_adapter.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/adapters/base.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/adapters/claude_code_adapter.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/engines/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/engines/decision_engine.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/engines/identity_engine.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/engines/memory_engine.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/engines/planning_engine.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/engines/reflection_engine.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/engines/skill_engine.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/engines/world_model_engine.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/output/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/output/context_builder.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/output/formatters/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/output/formatters/agents_md.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/output/formatters/json_format.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/output/formatters/summary.py` | `resolved_in_06_orphan_file_decision` |
| `knowledge/ucos/output/token_budget.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_00_idea_intake/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_01_gameplay_framework/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_02_design_review_freeze/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_03_program_requirements/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_04_art_requirements/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_05_program_review/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_06_art_review/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_07_art_style_generation/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_08_design_to_plan/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_09_art_plan/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_10_asset_alignment/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_11_dev_execution/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_12_art_production/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_14_integration_validation/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_d1_project_portrait/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_d2_design_decisions/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_d3_design_validation/__init__.py` | `resolved_in_06_orphan_file_decision` |
| `pipeline/step_d4_devflow_handoff/__init__.py` | `resolved_in_06_orphan_file_decision` |

## Parse Errors

| path | error |
| --- | --- |
| none | none |
