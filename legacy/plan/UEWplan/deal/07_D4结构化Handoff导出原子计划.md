# D4结构化Handoff导出原子计划

## 目标

把 D4 从 Markdown 交付升级为结构化 handoff 导出。

该计划完成后，D4 必须导出当前项目真实决策、profile、archetype、playable contract candidates 与 traceability。后续 Step02 会把这些候选合同冻结到 `outputs/artifacts/stage_02/playable_contracts/`。

## 依赖

- `02_合同Schema注册表与校验器原子计划.md`
- `03_设计知识库节点元数据原子计划.md`
- `06_D3设计合同门禁原子计划.md`

## 涉及范围

重点修改：

```text
pipeline/step_d4_devflow_handoff/plugin.py
core/design/export_adapter.py
core/design/exporter.py
core/design/playable_contracts.py
```

建议新增：

```text
core/design/structured_handoff.py
```

建议新增测试：

```text
core/tests/unit/test_structured_handoff_export.py
```

## 输出结构

```text
output/devflow_Design_*/
  package_manifest.json
  attachments/
    design.md
  structured/
    handoff_manifest.json
    decisions.json
    profile.json
    archetype_requirements.json
    traceability.json
    playable_contract_candidates/
      core_playable_contract.json
      demo_flow_contract.json
      runtime_data_contract.json
      ui_flow_contract.json
      scene_bootstrap_contract.json
      asset_mount_contract.json
      audio_requirements_contract.json
      playable_acceptance_contract.json
```

## handoff_manifest.json 格式

`handoff_manifest.json` 是新增文件，不替代现有 `package_manifest.json`。

关系：

- `package_manifest.json` 继续用于现有 devflow 包定位，必须保留 `stage`、`source_ids`、`package_type` 等旧字段。
- `handoff_manifest.json` 位于 `structured/` 下，用于描述结构化 handoff 内容。
- `handoff_manifest.json` 必须引用 `package_manifest.json` 的包信息，不能另建一套包发现标准。

建议格式：

```json
{
  "schema_version": "1.0",
  "handoff_type": "structured_design_handoff",
  "generated_at": "ISO-8601",
  "project_id": "string",
  "package_manifest_path": "../package_manifest.json",
  "structured_dir": "structured/",
  "decisions_path": "structured/decisions.json",
  "profile_path": "structured/profile.json",
  "archetype_path": "structured/archetype_requirements.json",
  "traceability_path": "structured/traceability.json",
  "contracts": [
    {
      "contract_id": "core_playable_contract",
      "path": "structured/playable_contract_candidates/core_playable_contract.json",
      "schema": "knowledge/schemas/playable_contracts/core_playable_contract.schema.json",
      "schema_version": "2.0",
      "required": true
    }
  ],
  "validation": {
    "status": "passed",
    "blocking_issues": [],
    "review_items": []
  }
}
```

## traceability 表达规则

系统保留两层 traceability，职责不同：

- 合同内部的 `source_refs` 是机器校验的主要依据，供 `validate_contract()` 和 Step 消费时快速判断该合同是否有来源。
- 合同内部的 `contract_refs` 用于表示该合同依赖的其他合同或 stage artifact。
- `structured/traceability.json` 是全量字段级索引，记录每个合同字段来自哪个设计节点、选项、profile 字段或生成规则，供人类审查、调试和追溯。

一致性规则：

- Step02 冻结时必须保留每个合同内部的 `source_refs`。
- `traceability.json` 可以比 `source_refs` 更细，但不能与 `source_refs` 矛盾。
- 发生不一致时，D4/D2 报告必须标记 `TRACEABILITY_MISMATCH`，Step02 不应冻结该合同为 success。

## 决策导出要求

`decisions.json` 必须包含：

- node_id。
- domain。
- priority。
- requirement_level。
- decision_state。
- checklistOptions。
- optionProvenance。
- selected_options。
- primary_options。
- notes。
- conflicts。
- provenance。

## 合同候选要求

D4 只导出 candidates，不直接替代 Stage02 artifacts。

原因：

- D4 属于设计 handoff。
- Step02 属于开发流水线冻结点。
- 正式开发消费路径必须仍是 `outputs/artifacts/stage_02/playable_contracts/`。

## playable contract 生成 API

D4 不能直接调用旧的 Markdown 入口：

```python
build_playable_contract_bundle(parsed)
```

必须新增并使用结构化入口：

```python
build_playable_contract_bundle_from_decisions(
    decisions: dict[str, Any],
    profile: dict[str, Any],
    archetype_requirements: dict[str, Any],
) -> dict[str, Any]
```

要求：

- 从 `decisions.json` 读取 selected、primary 和 optionProvenance。
- 从 `profile.json` 读取项目画像。
- 从 `archetype_requirements.json` 读取 required contracts 和 P0 节点规则。
- 旧 `build_playable_contract_bundle(parsed)` 保留兼容，不作为 D4 结构化 handoff 的主入口。
- 空 decisions 不允许生成假完整 candidates。

## archetype_requirements.json 要求

必须导出：

```json
{
  "schema_version": "1.0",
  "detected_archetype": "generic_playable",
  "detection_confidence": "fallback",
  "detection_source": [],
  "required_contracts": [],
  "optional_contracts": [],
  "archetype_p0_nodes": [],
  "archetype_p1_nodes": [],
  "not_applicable_rules": [],
  "warnings": []
}
```

如果 archetype 是 fallback，必须写入 `warnings`。

## audio_requirements_contract 要求

当前不强制生成真实音频，但必须导出：

- 音乐需求。
- 音效需求。
- UI 音效需求。
- 占位资源路径。
- 后续 AI 音频接入说明。

占位路径第一阶段沿用现有：

```text
Assets/Audio/.audio_placeholder
```

## playable_acceptance_contract 要求

`playable_acceptance_contract` 不能只依赖默认 fallback。

生成来源优先级：

1. `data_test_design_decision` 节点中已确认的体验测试、验收信号、操作步骤、预期反馈选项。
2. `demo_flow_contract.steps` 中的具体流程。
3. `core_playable_contract.action_verbs` 中的玩家动作。
4. 最后才允许 fallback 生成基础检查，但必须标记 `source: generated_from_core_loop` 并产生 review item。

D4 输出的 `playable_acceptance_contract.playmode_checks` 必须能追溯到上述来源之一。

## 执行步骤

1. D4 继续导出人类可读 `design.md`。
2. 新增 `structured/` 目录。
3. 从真实项目状态导出 `decisions.json`。
4. 导出 `profile.json` 与 `archetype_requirements.json`。
5. 调用 `build_playable_contract_bundle_from_decisions()` 生成 candidates。
6. 从 `data_test_design_decision` 或基础玩法合同生成 `playable_acceptance_contract`。
7. 生成 `traceability.json`，记录合同字段来自哪个节点、选项或人工输入。
8. 生成 `handoff_manifest.json`。
9. 导出完成后运行 schema 校验。
10. 校验失败时 D4 返回 blocked 或 completed_with_review，不能假 success。

## 完成标准

1. D4 输出 `handoff_manifest.json`。
2. D4 输出 `decisions.json` 且包含 selected、primary、optionProvenance。
3. D4 输出 playable contract candidates，名称与现有 `playable_contracts.py` 一致。
4. D4 输出 `scene_bootstrap_contract.camera`，不导出独立 `camera_view_spec`。
5. D4 使用 `build_playable_contract_bundle_from_decisions()`，不是旧 Markdown parsed 入口。
6. `archetype_requirements.json` 结构明确。
7. `playable_acceptance_contract` 有明确来源或明确 fallback review item。
8. 缺必需合同字段时 D4 不允许 success。
9. `design.md` 不再是后续 Step 的关键数据源。

## 不做事项

- 不改 Step02/03/04/08/13/14。
- 不生成 Unity 场景。
- 不把工厂建造字段写进通用核心合同。
