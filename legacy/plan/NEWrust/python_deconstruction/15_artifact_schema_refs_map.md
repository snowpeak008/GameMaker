# Artifact schema_refs 映射表

状态：第一轮确认。

来源：`pipeline/artifact_layer/registry.json`。

## 汇总

| Artifact | Stage | Kind | schema_refs | deps | tasks |
| --- | ---: | --- | ---: | ---: | ---: |
| `stage_00.concept_bundle` | 0 | `design_import_bundle` | 5 | 0 | 2 |
| `stage_01.gameplay_framework_bundle` | 1 | `framework_graph_bundle` | 5 | 1 | 2 |
| `stage_02.design_freeze_bundle` | 2 | `design_freeze_bundle` | 13 | 1 | 4 |
| `stage_03.program_requirements_bundle` | 3 | `program_requirements_bundle` | 6 | 1 | 2 |
| `stage_04.art_requirements_bundle` | 4 | `source_placeholder_or_import` | 14 | 1 | 2 |
| `stage_05.program_review_bundle` | 5 | `source_placeholder_or_import` | 3 | 1 | 2 |
| `stage_06.art_review_bundle` | 6 | `source_placeholder_or_import` | 3 | 1 | 2 |
| `stage_07.art_style_generation_confirmation_bundle` | 7 | `manual_gate` | 4 | 1 | 5 |
| `stage_08.program_plan_bundle` | 8 | `source_placeholder_or_import` | 10 | 3 | 2 |
| `stage_09.art_plan_bundle` | 9 | `source_placeholder_or_import` | 4 | 2 | 2 |
| `stage_10.asset_alignment_bundle` | 10 | `source_placeholder_or_import` | 5 | 2 | 2 |
| `stage_11.dev_execution_bundle` | 11 | `source_placeholder_or_import` | 2 | 3 | 2 |
| `stage_12.art_production_bundle` | 12 | `source_placeholder_or_import` | 17 | 1 | 2 |
| `stage_13.scene_assembly_bundle` | 13 | `source_placeholder_or_import` | 12 | 2 | 2 |
| `stage_14.integration_validation_bundle` | 14 | `source_placeholder_or_import` | 13 | 1 | 2 |

## schema_refs

```text
stage_00.concept_bundle
  outputs/artifacts/stage_00/intent_interpretation_contract.json -> knowledge/schemas/ai_design/intent_interpretation_contract.schema.json
  outputs/artifacts/stage_00/project_identity_contract.json -> knowledge/schemas/ai_design/project_identity_contract.schema.json
  outputs/artifacts/stage_00/project_dna_seed.json -> knowledge/schemas/ai_design/project_dna_contract.schema.json
  outputs/artifacts/stage_00/open_questions_contract.json -> knowledge/schemas/ai_design/open_questions_contract.schema.json
  outputs/artifacts/stage_00/customization_score_report.json -> knowledge/schemas/ai_design/customization_score_report.schema.json

stage_01.gameplay_framework_bundle
  outputs/artifacts/stage_01/gameplay_concretization_contract.json -> knowledge/schemas/ai_design/gameplay_concretization_contract.schema.json
  outputs/artifacts/stage_01/archetype_requirements.json -> knowledge/schemas/ai_design/archetype_requirements.schema.json
  outputs/artifacts/stage_01/open_questions_contract.json -> knowledge/schemas/ai_design/open_questions_contract.schema.json
  outputs/artifacts/stage_01/archetype_detection_report.json -> knowledge/schemas/ai_design/archetype_detection_report.schema.json
  outputs/artifacts/stage_01/customization_score_report.json -> knowledge/schemas/ai_design/customization_score_report.schema.json

stage_02.design_freeze_bundle
  outputs/artifacts/stage_02/playable_contracts/core_playable_contract.json -> knowledge/schemas/playable_contracts/core_playable_contract.schema.json
  outputs/artifacts/stage_02/playable_contracts/demo_flow_contract.json -> knowledge/schemas/playable_contracts/demo_flow_contract.schema.json
  outputs/artifacts/stage_02/playable_contracts/runtime_data_contract.json -> knowledge/schemas/playable_contracts/runtime_data_contract.schema.json
  outputs/artifacts/stage_02/playable_contracts/ui_flow_contract.json -> knowledge/schemas/playable_contracts/ui_flow_contract.schema.json
  outputs/artifacts/stage_02/playable_contracts/scene_bootstrap_contract.json -> knowledge/schemas/playable_contracts/scene_bootstrap_contract.schema.json
  outputs/artifacts/stage_02/playable_contracts/asset_mount_contract.json -> knowledge/schemas/playable_contracts/asset_mount_contract.schema.json
  outputs/artifacts/stage_02/playable_contracts/audio_requirements_contract.json -> knowledge/schemas/playable_contracts/audio_requirements_contract.schema.json
  outputs/artifacts/stage_02/playable_contracts/playable_acceptance_contract.json -> knowledge/schemas/playable_contracts/playable_acceptance_contract.schema.json
  outputs/artifacts/stage_02/design_ai_review_report.json -> knowledge/schemas/ai_design/design_ai_review_report.schema.json
  outputs/artifacts/stage_02/project_dna_contract.json -> knowledge/schemas/ai_design/project_dna_contract.schema.json
  outputs/artifacts/stage_02/playable_scenario_contract.json -> knowledge/schemas/ai_design/playable_scenario_contract.schema.json
  outputs/artifacts/stage_02/semantic_coverage_seed.json -> knowledge/schemas/ai_design/semantic_coverage_matrix.schema.json
  outputs/artifacts/stage_02/customization_score_report.json -> knowledge/schemas/ai_design/customization_score_report.schema.json

stage_03.program_requirements_bundle
  outputs/artifacts/stage_03/program_requirements_contract.json -> knowledge/schemas/program_requirements_contract.schema.json
  outputs/artifacts/stage_03/program_requirement_trace_report.json -> knowledge/schemas/ai_design/program_requirement_trace_report.schema.json
  outputs/artifacts/stage_03/program_structure_spec.json -> knowledge/schemas/ai_design/program_structure_spec.schema.json
  outputs/artifacts/stage_03/program_capability_contract.json -> knowledge/schemas/ai_design/program_capability_contract.schema.json
  outputs/artifacts/stage_03/program_semantic_coverage_report.json -> knowledge/schemas/ai_design/program_semantic_coverage_report.schema.json
  outputs/artifacts/stage_03/customization_score_report.json -> knowledge/schemas/ai_design/customization_score_report.schema.json

stage_04.art_requirements_bundle
  outputs/artifacts/stage_04/asset_spec_contract.json -> knowledge/schemas/ai_design/asset_spec_contract.schema.json
  outputs/artifacts/stage_04/art_requirements_contract.json -> knowledge/schemas/art_requirements_contract.schema.json
  outputs/artifacts/stage_04/asset_registry.json -> knowledge/schemas/ai_design/asset_registry.schema.json
  outputs/artifacts/stage_04/asset_requirements_resolved.json -> knowledge/schemas/ai_design/asset_requirements_resolved.schema.json
  outputs/artifacts/stage_04/unity_asset_mount_plan.json -> knowledge/schemas/ai_design/unity_asset_mount_plan.schema.json
  outputs/artifacts/stage_04/audio_placeholder_plan.json -> knowledge/schemas/ai_design/audio_placeholder_plan.schema.json
  outputs/artifacts/stage_04/image_consumable_spec.json -> knowledge/schemas/ai_design/art_pipeline/image_consumable_spec.schema.json
  outputs/artifacts/stage_04/ui_slice_spec_contract.json -> knowledge/schemas/ai_design/art_pipeline/ui_slice_spec_contract.schema.json
  outputs/artifacts/stage_04/unity_import_policy.json -> knowledge/schemas/ai_design/art_pipeline/unity_import_policy.schema.json
  outputs/artifacts/stage_04/asset_usage_binding_seed.json -> knowledge/schemas/ai_design/art_pipeline/asset_usage_binding_seed.schema.json
  outputs/artifacts/stage_04/audio_placeholder_requirements.json -> knowledge/schemas/ai_design/art_pipeline/audio_placeholder_requirements.schema.json
  outputs/artifacts/stage_04/art_taxonomy_contract.json -> knowledge/schemas/ai_design/art_taxonomy_contract.schema.json
  outputs/artifacts/stage_04/asset_strategy_matrix.json -> knowledge/schemas/ai_design/asset_strategy_matrix.schema.json
  outputs/artifacts/stage_04/customization_score_report.json -> knowledge/schemas/ai_design/customization_score_report.schema.json

stage_05.program_review_bundle
  outputs/artifacts/stage_05/program_ai_review_report.json -> knowledge/schemas/ai_design/program_ai_review_report.schema.json
  outputs/artifacts/stage_05/program_semantic_review_report.json -> knowledge/schemas/ai_design/program_semantic_review_report.schema.json
  outputs/artifacts/stage_05/customization_score_report.json -> knowledge/schemas/ai_design/customization_score_report.schema.json

stage_06.art_review_bundle
  outputs/artifacts/stage_06/art_ai_review_report.json -> knowledge/schemas/ai_design/art_ai_review_report.schema.json
  outputs/artifacts/stage_06/art_semantic_review_report.json -> knowledge/schemas/ai_design/art_pipeline/art_semantic_review_report.schema.json
  outputs/artifacts/stage_06/customization_score_report.json -> knowledge/schemas/ai_design/customization_score_report.schema.json

stage_07.art_style_generation_confirmation_bundle
  outputs/artifacts/stage_07/style_application_contract.json -> knowledge/schemas/ai_design/style_application_contract.schema.json
  outputs/artifacts/stage_07/style_fit_report.json -> knowledge/schemas/ai_design/style_fit_report.schema.json
  outputs/artifacts/stage_07/style_risk_acknowledgement.json -> knowledge/schemas/ai_design/style_risk_acknowledgement.schema.json
  outputs/artifacts/stage_07/customization_score_report.json -> knowledge/schemas/ai_design/customization_score_report.schema.json

stage_08.program_plan_bundle
  outputs/artifacts/stage_08/program_plan_contract.json -> knowledge/schemas/ai_design/program_plan_contract.schema.json
  outputs/artifacts/stage_08/playable_contract_plan_summary.json -> knowledge/schemas/playable_contracts/playable_contract_plan_summary.schema.json
  outputs/artifacts/stage_08/ai_task_synthesis_report.json -> knowledge/schemas/ai_design/ai_task_synthesis_report.schema.json
  outputs/artifacts/stage_08/scene_assembly_task_requirements.json -> knowledge/schemas/ai_design/scene_assembly_task_requirements.schema.json
  outputs/artifacts/stage_08/ui_runtime_task_requirements.json -> knowledge/schemas/ai_design/ui_runtime_task_requirements.schema.json
  outputs/artifacts/stage_08/input_runtime_task_requirements.json -> knowledge/schemas/ai_design/input_runtime_task_requirements.schema.json
  outputs/artifacts/stage_08/objective_runtime_task_requirements.json -> knowledge/schemas/ai_design/objective_runtime_task_requirements.schema.json
  outputs/artifacts/stage_08/program_task_breakdown.json -> knowledge/schemas/ai_design/program_task_breakdown.schema.json
  outputs/artifacts/stage_08/program_semantic_coverage_matrix.json -> knowledge/schemas/ai_design/program_semantic_coverage_matrix.schema.json
  outputs/artifacts/stage_08/customization_score_report.json -> knowledge/schemas/ai_design/customization_score_report.schema.json

stage_09.art_plan_bundle
  outputs/artifacts/stage_09/art_production_task_contract.json -> knowledge/schemas/ai_design/art_production_task_contract.schema.json
  outputs/artifacts/stage_09/art_task_breakdown.json -> knowledge/schemas/ai_design/art_task_breakdown.schema.json
  outputs/artifacts/stage_09/art_semantic_coverage_matrix.json -> knowledge/schemas/ai_design/art_semantic_coverage_matrix.schema.json
  outputs/artifacts/stage_09/customization_score_report.json -> knowledge/schemas/ai_design/customization_score_report.schema.json

stage_10.asset_alignment_bundle
  outputs/artifacts/stage_10/asset_alignment_report.json -> knowledge/schemas/ai_design/asset_alignment_report.schema.json
  outputs/artifacts/stage_10/mount_readiness_summary.json -> knowledge/schemas/ai_design/mount_readiness_summary.schema.json
  outputs/artifacts/stage_10/semantic_alignment_report.json -> knowledge/schemas/ai_design/semantic_alignment_report.schema.json
  outputs/artifacts/stage_10/semantic_coverage_matrix.json -> knowledge/schemas/ai_design/semantic_coverage_matrix.schema.json
  outputs/artifacts/stage_10/customization_score_report.json -> knowledge/schemas/ai_design/customization_score_report.schema.json

stage_11.dev_execution_bundle
  outputs/artifacts/stage_11/dev_execution_report.json -> knowledge/schemas/ai_design/dev_execution_report.schema.json
  outputs/execution_objects/execution_objects.json -> knowledge/schemas/execution_object_workflow.schema.json

stage_12.art_production_bundle
  outputs/artifacts/stage_12/art_production_report.json -> knowledge/schemas/ai_design/art_production_report.schema.json
  outputs/artifacts/stage_12/audio_placeholder_manifest_runtime.json -> knowledge/schemas/playable_contracts/audio_placeholder_manifest_runtime.schema.json
  outputs/artifacts/stage_12/raw_generated_asset_manifest.json -> knowledge/schemas/ai_design/art_pipeline/raw_generated_asset_manifest.schema.json
  outputs/artifacts/stage_12/image_quality_report.json -> knowledge/schemas/ai_design/art_pipeline/image_quality_report.schema.json
  outputs/artifacts/stage_12/art_semantic_review_report.json -> knowledge/schemas/ai_design/art_pipeline/art_semantic_review_report.schema.json
  outputs/artifacts/stage_12/art_rework_queue.json -> knowledge/schemas/ai_design/art_pipeline/art_rework_queue.schema.json
  outputs/artifacts/stage_12/processed_asset_manifest.json -> knowledge/schemas/ai_design/art_pipeline/processed_asset_manifest.schema.json
  outputs/artifacts/stage_12/sprite_slice_result_manifest.json -> knowledge/schemas/ai_design/art_pipeline/sprite_slice_result_manifest.schema.json
  outputs/artifacts/stage_12/unity_import_settings_manifest.json -> knowledge/schemas/ai_design/art_pipeline/unity_import_settings_manifest.schema.json
  outputs/artifacts/stage_12/sprite_atlas_plan.json -> knowledge/schemas/ai_design/art_pipeline/sprite_atlas_plan.schema.json
  outputs/artifacts/stage_12/addressable_asset_plan.json -> knowledge/schemas/ai_design/art_pipeline/addressable_asset_plan.schema.json
  outputs/artifacts/stage_12/ugui_prefab_contract.json -> knowledge/schemas/ai_design/art_pipeline/ugui_prefab_contract.schema.json
  outputs/artifacts/stage_12/ui_prefab_generation_request.json -> knowledge/schemas/ai_design/art_pipeline/ui_prefab_generation_request.schema.json
  outputs/artifacts/stage_12/asset_mount_manifest.json -> knowledge/schemas/ai_design/art_pipeline/asset_mount_manifest.schema.json
  outputs/artifacts/stage_12/program_asset_binding_preflight.json -> knowledge/schemas/ai_design/art_pipeline/program_asset_binding_preflight.schema.json
  outputs/artifacts/stage_12/art_handoff_manifest.json -> knowledge/schemas/ai_design/art_pipeline/art_handoff_manifest.schema.json
  outputs/execution_objects/execution_objects.json -> knowledge/schemas/execution_object_workflow.schema.json

stage_13.scene_assembly_bundle
  outputs/artifacts/stage_13/scene_assembly_report.json -> knowledge/schemas/ai_design/scene_assembly_report.schema.json
  outputs/artifacts/stage_11/dev_execution_report.json -> knowledge/schemas/ai_design/dev_execution_report.schema.json
  outputs/artifacts/stage_12/art_production_report.json -> knowledge/schemas/ai_design/art_production_report.schema.json
  outputs/artifacts/stage_12/audio_placeholder_manifest_runtime.json -> knowledge/schemas/playable_contracts/audio_placeholder_manifest_runtime.schema.json
  outputs/artifacts/stage_12/art_handoff_manifest.json -> knowledge/schemas/ai_design/art_pipeline/art_handoff_manifest.schema.json
  outputs/artifacts/stage_12/asset_mount_manifest.json -> knowledge/schemas/ai_design/art_pipeline/asset_mount_manifest.schema.json
  outputs/artifacts/stage_13/unity_editor_request.json -> knowledge/schemas/ai_design/art_pipeline/unity_editor_request.schema.json
  outputs/artifacts/stage_13/unity_art_import_report.json -> knowledge/schemas/ai_design/art_pipeline/unity_art_import_report.schema.json
  outputs/artifacts/stage_13/unity_prefab_generation_report.json -> knowledge/schemas/ai_design/art_pipeline/unity_prefab_generation_report.schema.json
  outputs/artifacts/stage_13/program_asset_binding_contract.json -> knowledge/schemas/ai_design/art_pipeline/program_asset_binding_contract.schema.json
  outputs/artifacts/stage_13/unity_scene_mount_report.json -> knowledge/schemas/ai_design/art_pipeline/unity_scene_mount_report.schema.json
  outputs/execution_objects/execution_objects.json -> knowledge/schemas/execution_object_workflow.schema.json

stage_14.integration_validation_bundle
  outputs/artifacts/stage_14/integration_validation_report.json -> knowledge/schemas/ai_design/integration_validation_report.schema.json
  outputs/artifacts/stage_14/playmode_test_results.json -> knowledge/schemas/ai_design/playmode_test_results.schema.json
  outputs/artifacts/stage_14/art_acceptance_report.json -> knowledge/schemas/ai_design/art_pipeline/art_acceptance_report.schema.json
  outputs/artifacts/stage_14/playable_acceptance_report.json -> knowledge/schemas/ai_design/art_pipeline/playable_acceptance_report.schema.json
  outputs/artifacts/stage_13/scene_assembly_report.json -> knowledge/schemas/ai_design/scene_assembly_report.schema.json
  outputs/artifacts/stage_13/unity_art_import_report.json -> knowledge/schemas/ai_design/art_pipeline/unity_art_import_report.schema.json
  outputs/artifacts/stage_13/unity_prefab_generation_report.json -> knowledge/schemas/ai_design/art_pipeline/unity_prefab_generation_report.schema.json
  outputs/artifacts/stage_13/unity_scene_mount_report.json -> knowledge/schemas/ai_design/art_pipeline/unity_scene_mount_report.schema.json
  outputs/artifacts/stage_12/art_handoff_manifest.json -> knowledge/schemas/ai_design/art_pipeline/art_handoff_manifest.schema.json
  outputs/artifacts/stage_02/playable_contracts/playable_acceptance_contract.json -> knowledge/schemas/playable_contracts/playable_acceptance_contract.schema.json
  outputs/artifacts/stage_12/art_production_report.json -> knowledge/schemas/ai_design/art_production_report.schema.json
  outputs/artifacts/stage_12/audio_placeholder_manifest_runtime.json -> knowledge/schemas/playable_contracts/audio_placeholder_manifest_runtime.schema.json
  outputs/execution_objects/execution_objects.json -> knowledge/schemas/execution_object_workflow.schema.json
```

## NEWrust 迁移要求

- 上表中的每个 path/schema pair 都需要 contract test。
- validator 解析路径时需要按 Python 行为支持 draft、current save workspace 和 project root 候选路径。
- Stage13/14 会验证上游 stage 文件，不是只验证本 stage 输出。
