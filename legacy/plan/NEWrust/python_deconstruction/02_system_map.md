# Python 系统结构图

状态：草案，依据第一轮 source authority index。

```text
Root GUI
  -> core.ui.gui_app
    -> MainWindow
      -> Design Workbench
      -> Development Pipeline
      -> Supplemental Development
      -> Packaging Stage
      -> Run Log
      -> SDK Knowledge Base

Pipeline Runtime
  -> core.main.run_range
    -> PluginManager
    -> StagePlugin
    -> artifact preflight/review/validator
    -> runtime state
    -> save sync

Design Workbench
  -> core.design.data_loader.load_project_data
    -> knowledge/design_data/domains
    -> knowledge/design_data/templates
    -> knowledge/design_data/entity_schemas
    -> knowledge/design_data/gameplay_system_options.json
  -> core.design.engine.DesignEngine
    -> project_state normalization
    -> checklist / option group decisions
    -> option provenance
    -> L4 progress
    -> L5 designEntities validation
    -> cross-layer / quality violations
  -> core.design.exporter.write_export
    -> user export markdown/json/text/prompt
  -> core.design.export_adapter.export_concept_package
    -> source_artifacts/devflow_Concept_v2
    -> source_artifacts/devflow_GameplayFramework_v2
    -> source_artifacts/devflow_Design_v2
    -> structured handoff

Storage
  -> core.paths
  -> drafts/<session_id>
  -> saves/<save_id>
  -> logs/
  -> settings/

Knowledge
  -> knowledge/design_data
  -> knowledge/schemas
  -> pipeline/artifact_layer

Artifact Governance
  -> pipeline/artifact_layer/registry.json
    -> artifact id / stage / kind / tasks / depends_on / reviewers / validators / schema_refs
  -> core.artifact.graph
    -> dependency_graph.json
    -> topological_step_order()
  -> core.artifact.preflight
    -> pre-step hard gate
  -> core.artifact.reviewer
    -> post-step 4 reviewer pass
  -> core.artifact.validator
    -> post-step 7 validator pass
```

待补充：AI adapters、save manager 函数级 schema、日志面板细节。
