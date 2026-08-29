# 05 Open Questions 与 Project DNA 冻结

## 目标

Step01 生成 archetype 专属开放问题，Step02 合并 decisions、OQ、playable contracts，冻结正式 `project_dna_contract.json`。

---

## 修改范围

修改：

```text
pipeline/step_01_gameplay_framework/plugin.py
pipeline/step_02_design_review_freeze/plugin.py
core/engines/generation.py
```

新增或扩展：

```text
core/design/open_questions.py
core/design/project_dna.py
core/design/semantic_coverage.py
core/tests/unit/test_open_questions_contract.py
core/tests/unit/test_project_dna_builder.py
core/tests/unit/test_step02_project_dna_freeze.py
```

---

## 输出产物

Step01：

```text
stage_01/archetype_requirements.json
stage_01/open_questions_contract.json
stage_01/archetype_detection_report.json
stage_01/customization_score_report.json
```

Step02：

```text
stage_02/project_dna_contract.json
stage_02/playable_scenario_contract.json
stage_02/semantic_coverage_seed.json
stage_02/customization_score_report.json
```

---

## Project DNA 冻结规则

1. `project_dna_contract.json` 必须设置 `contract_state="frozen"`。
2. Step03+ 只能消费 frozen contract。
3. blocking OQ 未解决时，Step02 blocked。
4. `systems`、`entities`、`assets` 等关键对象不允许全空字段通过。
5. playable contracts 必须引用 Project DNA 的真实语义。

---

## 门禁

| Code | 中文描述 | 阻断 |
|---|---|---|
| `BLOCKING_OQ_UNRESOLVED` | 阻断级开放问题未解决 | 是 |
| `NULL_CONTRACT_FIELD` | 冻结合约存在关键空字段 | 是 |
| `PLAYABLE_SCENARIO_MISSING` | 第一可玩流程缺失 | 是 |
| `CORE_ENTITY_UNFROZEN` | 核心实体未冻结 | 是 |

---

## 验收标准

1. Step01 的 OQ 对不同 archetype 内容不同。
2. blocking OQ 未答时 Step02 blocked。
3. Step02 输出 frozen Project DNA。
4. `semantic_coverage_seed.json` 只定义 required semantic items，不判断覆盖。
5. 测试覆盖 seed 到 frozen 的转换。

