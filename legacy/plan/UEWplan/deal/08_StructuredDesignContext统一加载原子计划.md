# StructuredDesignContext统一加载原子计划

## 目标

建立开发流水线统一读取结构化设计 handoff 与 stage artifacts 的上下文层，避免每个 Step 自己解析 JSON 或 Markdown。

该计划完成后，Step00-14 都可以通过统一 API 获取 D4 handoff、Stage02 playable contracts 和上游 artifacts。

## 依赖

- `07_D4结构化Handoff导出原子计划.md`
- `17_generation引擎结构化上下文重构策略.md`

## 涉及范围

建议新增：

```text
core/design/structured_context.py
core/design/structured_handoff.py
core/engines/source_context.py
```

建议修改：

```text
core/engines/generation.py
```

建议新增测试：

```text
core/tests/unit/test_structured_design_context.py
```

## API 设计

禁止使用未定义的 `save_id` 参数。建议 API 使用现有路径概念：

```python
context = StructuredDesignContext.from_output_base(output_base_dir)
context = StructuredDesignContext.from_draft_session(draft_session_id, workspace_root)

profile = context.require_handoff("profile")
ui_flow = context.require_playable_contract("ui_flow_contract")
scene = context.require_playable_contract("scene_bootstrap_contract")
audio = context.optional_playable_contract("audio_requirements_contract")
```

## 数据来源优先级

1. 当前 stage 输出目录中的 artifacts。
2. `outputs/artifacts/stage_02/playable_contracts/` 中的 frozen contracts。
3. D4 `structured/playable_contract_candidates/`。
4. Markdown 只能作为非 P0 兼容 fallback，并且必须产生 warning。

## 执行步骤

1. 实现 handoff 查找逻辑。优先扩展现有 `_latest_concept_package()` 或提取其 devflow 包扫描、manifest 读取、版本排序逻辑为公共函数；`StructuredDesignContext` 复用该逻辑查找 `devflow_Design_*` 包下的 `structured/` 目录，不重复实现扫描和排序。
2. 读取 D4 `handoff_manifest.json`。
3. 读取 StageContext/artifact_layer 标准路径。
4. 提供 `require_handoff(contract_id)`。
5. 提供 `require_playable_contract(contract_id)`。
6. 提供 `optional_playable_contract(contract_id)`。
7. 提供 `trace(contract_id, field_path)`。
8. 缺必需合同时返回标准 blocking issue。
9. 阻止 Step 回退解析 `design.md` 获取 P0 合同字段。

## 错误规则

如果 Step 调用 `require_playable_contract("ui_flow_contract")` 但合同缺失，错误必须包含：

- `code`: `REQUIRED_CONTRACT_MISSING`
- `contract_id`: `ui_flow_contract`
- `required_by_step`: 当前 Step
- `artifact_path`
- `repair_hint`

## 完成标准

1. 能加载最新 D4 structured handoff。
2. 能加载 `outputs/artifacts/stage_02/playable_contracts/`。
3. 能按 contract_id 读取合同。
4. 能校验合同文件存在性。
5. 缺合同能生成 blocking issue。
6. Step 测试可以用 fake output_base_dir 运行。

## 不做事项

- 不实现具体 Step 业务逻辑。
- 不导出 D4 handoff。
- 不生成 Unity 内容。
