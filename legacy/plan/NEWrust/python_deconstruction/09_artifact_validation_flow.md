# Artifact 与 Validation 流程

状态：第一轮确认。

已确认 `core/main.py` 调用：

```text
emit_dependency_graph()
preflight_stage_contract(step)
run_review_pipeline(...)
run_artifact_validators(...)
```

已确认注册表位置：

```text
pipeline/artifact_layer/
```

## 注册表契约

`pipeline/artifact_layer/registry.json` 是 artifact 层事实源，顶层包含：

- `version`
- `description`
- `default_reviewers`
- `default_validators`
- `artifacts`

每个 artifact 至少包含：

- `id`
- `stage`
- `kind`
- `depends_on`
- `tasks`
- `reviewers`
- `validators`
- `schema_refs`
- `knowledge_refs`

已确认 reviewer 白名单：

- `structure_reviewer`
- `source_trace_reviewer`
- `task_reviewer`
- `dependency_reviewer`

已确认 validator 白名单：

- `validator_first_contract`
- `stage_files_validator`
- `review_report_validator`
- `manifest_validator`
- `schema_contract_validator`
- `knowledge_refs_validator`
- `dependency_status_validator`

## 依赖图

`core.artifact.graph` 负责：

- `topological_artifact_order()`：按 artifact 依赖拓扑排序，检查未知依赖和环。
- `build_dependency_graph()`：生成 nodes、edges、topological_order、errors。
- `topological_step_order(from_step, stop_step)`：从 artifact 拓扑推出实际执行 step 顺序。
- `emit_dependency_graph()`：写 `pipeline/artifact_layer/dependency_graph.json` 和 `OUTPUTS_DIR/dependency_graph.json`。

当前物化图为 Step00 -> Step01 -> Step02 -> Step03，之后分支到 Step04/05/08，Step06 -> Step07，Step08/09 -> Step10，Step10/11/12 -> Step13 -> Step14。

## Preflight

`preflight_stage_contract(step)` 在步骤执行前运行硬门禁：

- stage 必须声明 artifacts。
- artifact 必须声明 tasks、reviewers、validators。
- reviewers/validators 必须在白名单中。
- task id 不可重复。
- `depends_on` 必须能解析到 registry artifact。
- `knowledge_refs` 必须存在。
- 若声明 `schema_contract_validator`，必须提供 `schema_refs` 且 schema 文件存在。
- 上游依赖必须已有 `artifact_validation_layer.json` 且 status 为 `success`。

失败时写 `OUTPUTS_DIR/artifact_layer/preflight_stage_XX.json` 并抛出异常。

## Reviewer

`run_review_pipeline(step)` 在步骤执行后运行 4 reviewer：

- `write_stage_artifact_manifest()` 先写 `stage_XX/artifact_layer_manifest.json`。
- `structure_reviewer` 检查 stage 目录、`artifact_index.json`、`reference_manifest.json`。
- `source_trace_reviewer` 检查 `validation_report.json`，要求成功导入 sources/upstream，或明确记录 missing groups。
- `task_reviewer` 检查 task id 存在且不重复。
- `dependency_reviewer` 检查依赖解析和上游 validation 成功。
- 结果写 `stage_XX/artifact_reviews.json`。

任一 fail 会抛出异常。

## Validator

`run_artifact_validators(step)` 在 reviewer 之后运行 7 validator：

- `validator_first_contract`：artifact 必须声明 validators、reviewers、tasks。
- `stage_files_validator`：`validation_report.json` 成功，且 `artifact_index.json`、`reference_manifest.json` 存在。
- `review_report_validator`：`artifact_reviews.json` status 必须为 success。
- `manifest_validator`：`artifact_layer_manifest.json` 必须含 artifacts 和 tasks。
- `knowledge_refs_validator`：knowledge refs 必须存在。
- `schema_contract_validator`：调用 `tools.validators.contract_validator.validate_contract_file()`。
- `dependency_status_validator`：上游 artifact validation 必须成功。

结果写 `stage_XX/artifact_validation_layer.json`，随后调用 `refresh_reference_manifest_file_inventory(step)`。

## NEWrust 迁移要求

- Rust 后端必须把 artifact registry 作为 contract-first 数据源读取。
- 不能把 Step00-14 顺序硬编码为唯一真相；执行顺序应由拓扑计算校验。
- preflight/reviewer/validator 必须是后端 service，不属于 Web UI。
- schema contract validator 可先按 JSON Schema 实现，再逐步补齐 Python validator 的路径候选解析行为。
- dependency status 必须读取上游 `artifact_validation_layer`，不能只看 step state。

每个 Step output 到 artifact `schema_refs` 的完整映射表已拆到 `15_artifact_schema_refs_map.md`。
