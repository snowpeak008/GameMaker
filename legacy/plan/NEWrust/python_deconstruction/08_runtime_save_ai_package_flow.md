# Runtime / Save / AI / Package 流程

状态：草案。

## Runtime

已确认：

- `core/main.py` 写 run state。
- `core/runtime/control.py` 管 stop request。
- `core/runtime/pipeline_state.py` 管 step state。
- `core/runtime/run_context.py` 管 run context。

## Save

已确认：

- draft 是运行时写入点。
- formal save 是显式归档点。
- GUI 启动会尝试恢复上次存档。
- GUI 退出会释放当前 lock。
- `create_save()` / `create_blank_save()` 创建 formal archive 后会调用 sync。
- `_sync_save()` 将 active draft 原子复制到 formal `workspace/`，写 snapshot、file map、timeline、manifest progress。
- `load_save()` 先获取 archive lock，再把 formal workspace 复制回 active draft。
- `delete_save()` 删除 formal archive 后，若删除的是当前存档，会把 draft 标记为 `unsaved_copy_of_deleted_save`。
- 设计项目本体通过 `ExecutionObjectStore` 保存为 `design_project` execution object，manual save 会立即 submit/analyze/approve/execute/verify。

## AI

已确认：

- `MainWindow` 底部状态读取 active profile 和 validator。
- `PipelinePanel` 有 AI 配置入口。
- 设计工作台导入 AI interview window。
- `settings/ai_config.json` schemaVersion 为 v3。
- AI 配置分三类：`dev`、`image`、`completion`。
- active dev entry 兼容生成 active profile。
- `get_pipeline_adapter()` 从 active profile 的 `adapter` 派生具体 adapter。
- 支持 adapter：`none`、`codex`、`claude`、`openai`、`local`。
- `AIConfigValidator` 只做静态验证，默认不做网络调用；可选检查 CLI `--version`。

待补充：AI 访谈 UI 写入链和 ucos bridge。

## Logs

已确认：

- `JsonlLogWriter.for_run("pipeline_run", run_id)` 写到 `RUN_LOGS_DIR / pipeline_run_<run_id>.jsonl`。
- `LogEntry` 字段：`timestamp`、`level`、`context`、`message`、`source`、`metadata`。
- `core/main.py::run_range()` 在 stage start / inherited / success / failed 时写 JSONL。
- `MainWindow._get_log_panel()` 加载最近 5 个 JSONL 到 `LogPanel`。
- `LogPanel` 支持 level filter、clear、export JSONL。

## Design -> Pipeline Handoff

已确认：

- `PipelinePanel._export_to_pipeline()` 调用 `core.design.export_adapter.export_concept_package()`。
- `export_concept_package()` 优先读取 `DRAFT_DIR/autosave_state.json`。
- 若 autosave 不存在，则尝试 `save_manager.ensure_current_save()`、execution object store、`load_latest_design_project()`。
- 若仍无状态，则使用 `DesignEngine.empty_state()`。
- 输出根目录默认为 `SOURCE_ARTIFACTS_DIR`。
- 生成三个 source package：
  - `devflow_Concept_v2/concept.md`
  - `devflow_GameplayFramework_v2/framework.md`
  - `devflow_Design_v2/design.md`
- 每个 package 同时写：
  - `attachments/<name>.md`
  - `package_manifest.json`
  - `operator_submission.json`
  - `human_approval.json`
  - `selected_play_prototype.json`
  - `selected_play_prototype.md`
  - `human_review.md`
  - `stage_input.md`
- Design package 额外调用 `write_structured_handoff()`，作为 Step02+ 的结构化设计交接。

这条链路是设计工作台进入 Step00-02 的关键桥，不可降级为单一 markdown 导出。

## User Export

`core.design.exporter.write_export()` 支持：

- `markdown`
- `json`
- `txt` / `text`
- `prompt`
- `decision` scope
- `archive` scope

`archive` markdown 会额外写 sidecars：

- `<project>.full.json`
- `<project>.profile.json`
- `<project>.coverage.json`

`build_payload()` 的 schemaVersion 当前为 `0.5.0`，包含 taxonomy、profile、coverage、quality、crossLayerViolations、gameplaySystems、domains、nodes、checklist、optionGroups、optionRelations、designEntities。

## Package

已确认：

- `core/ui/package_panel.py` 只有在 Step14 state 为 `success` 时启用打包。
- `PackagePanel.run_package()` 调用 `core.packaging.run_package()`。
- `core.packaging.service.run_package()` 写 `build_report.json`、`package_validation_report.json`、`PACKAGE_NOTES.md`、`package_manifest.json`。

待补充：`core/packaging/validation.py` 和 manifest 字段细节。
