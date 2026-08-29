# 04 Project Identity 与 DNA Seed

## 目标

在 Step00 输出当前项目身份、上下文签名、初始 Project DNA seed 和开放问题草稿，避免后续步骤无法区分并行项目。

---

## 修改范围

修改：

```text
pipeline/step_00_idea_intake/plugin.py
core/engines/generation.py
```

新增：

```text
core/design/project_identity.py
core/design/project_dna.py
core/design/open_questions.py
core/tests/unit/test_step00_project_identity.py
```

---

## 输出产物

```text
stage_00/project_identity_contract.json
stage_00/project_dna_seed.json
stage_00/open_questions_contract.json
stage_00/customization_score_report.json
```

`project_dna_seed.json` 必须使用：

```json
{
  "contract_state": "seed"
}
```

Step03+ 不得直接消费 seed。

---

## Project Signature

最低字段：

```json
{
  "draft_session_id": "...",
  "output_base_dir": "...",
  "linked_save_id": "... optional",
  "project_id": "...",
  "project_name": "...",
  "project_signature": "hash(draft_session_id + output_base_dir + source_artifacts_path + decisions_hash + template_hash + project_name)"
}
```

`linked_save_id` 只作归属信息，不作结构化上下文主键。

---

## 门禁

| Code | 中文描述 | 阻断 |
|---|---|---|
| `PROJECT_IDENTITY_INCOMPLETE` | 项目身份字段不完整 | 是 |
| `ARCHETYPE_GENERIC_WITH_STRONG_SIGNALS` | 有强信号却退化为 generic | 是 |
| `PROJECT_SIGNATURE_MISSING` | 缺少项目签名 | 是 |
| `SOURCE_REF_CROSSES_DRAFT` | 引用其他 draft/source | 是 |

---

## 验收标准

1. Step00 成功输出四个新增产物。
2. 两个并行项目生成不同 `project_signature`。
3. seed 中包含项目名、初步 archetype、已知核心循环、显式设计片段。
4. 无强信号时允许 fallback，但必须写 warning。
5. 测试覆盖并行两个项目的签名隔离。

