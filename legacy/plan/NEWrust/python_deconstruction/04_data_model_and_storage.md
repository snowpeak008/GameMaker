# Python 数据模型与存储

状态：草案。

## 已确认路径模型

`core/paths.py` 定义：

- `PROJECT_ROOT`
- `DRAFTS_DIR`
- `DRAFT_DIR`
- `SOURCE_ARTIFACTS_DIR`
- `OUTPUTS_DIR`
- `ARTIFACTS_DIR`
- `RUNTIME_CONTROL_DIR`
- `RUN_LOGS_DIR`
- `SAVES_DIR`
- `SETTINGS_DIR`
- `KNOWLEDGE_DIR`

## 已确认存档模型

`core/save/manager.py` 定义：

- runtime edits live in current per-session draft。
- formal archives live under `saves/<save_id>/`。
- formal archives contain `manifest.json` plus `workspace/`。
- active dirs include `source_artifacts`、`outputs`、`workspace`、`iteration_specs`、`patches`。

### Save Index

`saves/save_index.json`：

- `schema_version`
- `current_save_id`
- `saves`
- `updated_at`

`save_index()` 会按 `last_worked_at` / `created_at` 倒序排序。

### Formal Manifest

`saves/<save_id>/manifest.json`：

- `schema_version`
- `save_id`
- `display_name`
- `save_type`
- `created_by`
- `reason`
- `created_at`
- `last_worked_at`
- `last_transaction_seq`
- `progress`
- iteration save 额外可有 `change_type`、`requested_version`、`iteration_spec_path`

旧文件名 `save_manifest.json` 只作兼容读取，sync 后会迁移为 `manifest.json`。

### Draft Meta

`draft_meta.json`：

- `schema_version`
- `session_id`
- `pid`
- `project_root`
- `draft_root`
- `updated_at`
- `linked_save_id`
- `linked_archive_path`
- `workspace_state`
- `origin_deleted_save_id`

`workspace_state` 可为：

- `linked_save`
- `unsaved`
- `unsaved_copy_of_deleted_save`

### File Map / Timeline / Snapshot

当前 draft 会维护：

- `draft_file_map.json`
- `timeline.jsonl`
- `snapshots/<seq>_<event>/snapshot_manifest.json`
- `snapshots/<seq>_<event>/snapshot_file_map.json`
- `snapshots/<seq>_<event>/full/`
- `snapshots/<seq>_<event>/delta/added.json`
- `snapshots/<seq>_<event>/delta/modified.json`
- `snapshots/<seq>_<event>/delta/removed.json`

`build_file_map()` 为每个 active file 记录：

- `workspace_path`
- `size_bytes`
- `mtime_ns`
- `sha256`
- `stage`
- `artifact_id`
- `role`
- `source_type`
- `reference_manifest`
- `latest_transaction_seq`

### Lock

`saves/<save_id>/.archive_lock`：

- `pid`
- `session_id`
- `acquired_at`

`acquire_archive_lock()` 使用 `O_CREAT | O_EXCL` 原子创建；如果旧 pid 已不存在会清理旧锁。

### Autosave

设计工作台 autosave 文件：

```text
drafts/<session_id>/autosave_state.json
```

`CommercialDesignApp._do_autosave()` 写入当前 `project_state`。窗口关闭前 `MainWindow.on_close()` 会调用 `_flush_autosave()` 并用 hash 检查未保存变更。

## 已确认设计工作台状态模型

`core.design.engine.DesignEngine.empty_state()` 是设计工作台 `project_state` 的初始 schema：

- `projectName`：默认 `未命名游戏设计项目`。
- `profile`：来自 `PROFILE_DEFAULTS`。
- `nodes`：按 `knowledge/design_data/domains/*` 中所有 node 建立状态。
- `nodes.<node_id>.decisionState`：`not_started`、`selected`、`completed`、`risk`、`not_applicable`。
- `nodes.<node_id>.designNote` / `riskNote` / `notApplicableReason`：用户输入文本。
- `nodes.<node_id>.designEntities`：L5 实体数组。
- `nodes.<node_id>.entityValidationErrors`：实体 schema 验证 warning。
- `nodes.<node_id>.checklist`：`item_id -> bool`。
- `nodes.<node_id>.checklistOptions`：`item_id -> group_id -> {selected, primary}`。
- `nodes.<node_id>.optionProvenance`：记录 `source`、`confirmed`、`updated_at`、`actor`、`ai_inference_id`。
- `gameplaySystems`：玩法系统选择、自定义系统、权重、coreLoops、interview。
- `aiInterview`：AI 访谈状态。

`normalize_state()` 负责：

- 合并 profile 默认值。
- 兼容旧字段 `design_entities`。
- 剔除不存在的 option group 和 option id。
- 单选 group 截断为一个 option。
- 为迁移推断出的 option provenance 标记 `migration_inferred` 和 `confirmed=false`。
- 对 `designEntities` 调用 entity schema registry 生成 warning。

## 已确认设计静态数据模型

`core.design.data_loader.load_project_data()` 读取：

- `knowledge/design_data/domains/`
- `knowledge/design_data/templates/`
- `knowledge/design_data/entity_schemas/`
- `knowledge/design_data/gameplay_system_options.json`
- `knowledge/design_data/domain_order.json`

加载阶段会执行：

- `templateRef` 展开 optionGroups / optionRelations。
- checklist legacy id 兼容。
- option group 标准化：`selectionMode`、`required`、`allowPrimary`、`mdaLayer`、`progressionStep`。
- option relation 标准化：`soft_conflict`、`hard_exclusive`。
- node roleClass 归一化和 requirement metadata 注入。
- 设计实体 schema 预验证。
- 生成 `_meta.validationErrors`、`_meta.validationWarnings`、`templateReuse`、`roleClassCounts`、`runtimeRoot`、`dataSource`。

注意：`option_mapping.json` 和 `option_mapping.md` 是大体量数据资产，NEWrust 应作为数据文件读取和索引，不应手写进代码。

## 已确认设计质量数据

`DesignEngine` 计算的数据必须迁移到 Rust 后端：

- `domain_coverage()` / `project_coverage()`：节点和 checklist 覆盖率。
- `node_l4_progress()` / `project_l4_progress()`：必选 option group 完整性。
- `concreteness_coverage()`：具体节点是否具备 schema-valid L5 实体。
- `consistency_score()`：跨层规则 critical violation 比例。
- `quality_violations()`：缺失 L5 实体、跨层 critical、模板复用未声明。
- `quality_metrics()`：结构覆盖、具体性、跨层一致性、质量 badge。
- `design_completion_summary()`：contract coverage、P0/P1/P2 完成情况、blocking issues、review items。

这些结果属于业务 contract，不属于 Web UI 层。

## 已确认补充开发数据

`core.patch.record.PatchStore`：

- 根目录：`PATCHES_DIR`。
- 每个补丁：`<patch_id>/patch_manifest.json`。
- `PatchRecord` 字段：`patch_id`、`request`、`status`、`created_at`、`updated_at`、`tasks`、`changed_files`、`validation_summary`、`promoted_iteration_spec`、`errors`。
- `PatchTask` 字段：`task_id`、`title`、`description`、`affected_systems`、`expected_files`、`validation_route`、`requires_iteration`。

## 已确认 SDK 数据

`core.sdk.knowledge_base.SdkKnowledgeBase`：

- 根目录：`SDK_KNOWLEDGE_DIR`，实际为 `knowledge/sdks`。
- Index：`_index.json`。
- Template：`_spec_template.md`。
- Spec：`<sdk_id>/spec.json`。
- 状态：`draft`、`pending_review`、`approved`、`rejected`。
- approved prompt context 只读取 `approved` specs。

## 已确认打包数据

`core.packaging.service.run_package()`：

- 输出目录：`package_output_dir(outputs_dir)`。
- 读取：`ARTIFACTS_DIR` 中的 packaging sources。
- 写入：
  - `build_report.json`
  - `package_validation_report.json`
  - `PACKAGE_NOTES.md`
  - `package_manifest.json`
- Gate：UI 层要求 Step14 state 为 `success`。
