# 导出与交接契约拆解

状态：第一轮确认。

## 用户导出

入口：`core.design.exporter.write_export(engine, project_state, target_dir, export_format, export_scope, include_gameplay_global_view)`。

支持格式：

- `json`
- `markdown`
- `txt` / `text`
- `prompt`

支持 scope：

- `decision`
- `archive`

`build_payload()` 生成统一 payload：

- `schemaVersion = 0.5.0`
- `exportedAt`
- `projectName`
- `documentMetadata`
- `taxonomy`
- `profile`
- `profileDisplay`
- `projectCoverage`
- `coverageMetrics`
- `qualityBadge`
- `structureCoverage`
- `concretenessCoverage`
- `consistencyScore`
- `qualityViolations`
- `crossLayerViolations`
- `gameplaySystems`
- `gameplaySystemGlobalView`
- `domains`

每个 node payload 包含：

- node metadata
- `designEntities`
- `entityValidationErrors`
- `decisionState`
- notes
- `decisionMetadata`
- `selection_state`
- `confidence`
- downstream stage hints
- checklist option groups and active conflicts

`archive` markdown 额外写：

- `<project>.full.json`
- `<project>.profile.json`
- `<project>.coverage.json`

## 流水线交接

入口：`core.design.export_adapter.export_concept_package()`。

状态读取优先级：

1. `DRAFT_DIR/autosave_state.json`
2. current save + execution object store + latest design project
3. `DesignEngine.empty_state()`

输出根目录：`SOURCE_ARTIFACTS_DIR`，除非显式传入 `target_dir`。

输出三类 package：

```text
devflow_Concept_v2/
devflow_GameplayFramework_v2/
devflow_Design_v2/
```

每个 package 包含：

```text
attachments/<primary>.md
package_manifest.json
operator_submission.json
human_approval.json
selected_play_prototype.json
selected_play_prototype.md
human_review.md
stage_input.md
```

package manifest 固定声明：

- `schema_version = 1`
- `project`
- `project_id = devflow`
- `package_id = source:<SourceType>`
- `package_type`
- `source_id`
- `source_ids`
- `stage = 0`
- `stage_slug = idea_intake`
- `generated_by = autodesignmaker.export_adapter`
- `design_summary`

Design package 额外调用 `write_structured_handoff()`，这是 Step02+ 消费结构化设计数据的关键契约。

## 内容分层

- Concept package：Layer 1 项目愿景 + Layer 2 核心体验。
- GameplayFramework package：Layer 1-2 摘要 + Layer 3 系统图。
- Design package：Layer 1-3 摘要 + Layer 4 全量设计决策 + Layer 5 资源图和 L5 实体。

## NEWrust 设计约束

- 用户导出和 pipeline handoff 是两条不同业务链路。
- pipeline handoff 必须保留三包结构，不能合并为单文件。
- package manifest、operator submission、human approval 等文件名和字段需要 contract 测试。
- structured handoff 必须作为单独里程碑继续深读。
