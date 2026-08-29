# 02 Schema 注册表与产物契约

## 目标

为设计语义闭环新增和扩展必要 schema，并同步 `pipeline/artifact_layer/registry.json`，确保新增产物都有结构门禁。

---

## 修改范围

新增或扩展：

```text
knowledge/schemas/ai_design/
pipeline/artifact_layer/registry.json
pipeline/artifact_layer/dependency_graph.json
core/tests/unit/test_design_semantic_schema_registry.py
```

---

## Schema 分类

### 已存在，只扩展

| Schema | 处理 |
|---|---|
| `program_plan_contract.schema.json` | 扩展项目语义字段 |
| `art_production_task_contract.schema.json` | 扩展项目语义字段 |
| `program_ai_review_report.schema.json` | 保留 AI review 职责 |
| `art_ai_review_report.schema.json` | 保留 AI review 职责 |
| `art_pipeline/art_semantic_review_report.schema.json` | Stage06 registry 新增引用 |

### 新增

```text
project_identity_contract.schema.json
project_dna_contract.schema.json
open_questions_contract.schema.json
program_capability_contract.schema.json
art_taxonomy_contract.schema.json
asset_strategy_matrix.schema.json
customization_score_report.schema.json
style_fit_report.schema.json
semantic_alignment_report.schema.json
semantic_coverage_matrix.schema.json
archetype_detection_report.schema.json
playable_scenario_contract.schema.json
program_semantic_coverage_report.schema.json
program_semantic_coverage_matrix.schema.json
art_semantic_coverage_matrix.schema.json
program_semantic_review_report.schema.json
style_risk_acknowledgement.schema.json
program_task_breakdown.schema.json
art_task_breakdown.schema.json
```

### 不单独建 schema

| 产物 | 处理 |
|---|---|
| `project_dna_seed.json` | 复用 `project_dna_contract.schema.json`，`contract_state="seed"` |
| `semantic_coverage_seed.json` | 复用 `semantic_coverage_matrix.schema.json`，`matrix_state="seed"` |

---

## Registry 要求

必须新增映射：

```text
stage_06/art_semantic_review_report.json
  -> knowledge/schemas/ai_design/art_pipeline/art_semantic_review_report.schema.json
```

各阶段新增产物必须进入对应 `schema_refs`，不得只写文件不注册。

---

## 验收标准

1. 所有 schema 文件存在且 JSON 可解析。
2. registry 中所有新增 schema refs 指向真实文件。
3. `archetype_requirements.schema.json` 不重复定义冲突版本，只扩展 deal 既有结构。
4. 测试能发现 registry 指向不存在 schema 的情况。
5. 不改变现有已通过 schema 的必填字段兼容性，除非对应阶段同步修改测试。

