# Step05到Step09前置审查与美术链路原子计划

## 目标

补齐 Step05、Step06、Step07、Step09 的结构化合同消费，防止 Step04 之后的审查、风格确认和美术计划重新退回 Markdown 链路。

该计划只覆盖 Step08 之前必须完成的前置链路。Step10/11/12 在 `20_Step10到Step12执行对齐与生产链路原子计划.md` 中单独处理，避免 Step11 依赖尚未定义的 Step08 输出。

## 依赖

- `10_Step03程序需求强消费原子计划.md`
- `11_Step04资源需求强消费原子计划.md`
- `17_generation引擎结构化上下文重构策略.md`

## 涉及范围

重点修改：

```text
pipeline/step_05_program_review/plugin.py
pipeline/step_06_art_review/plugin.py
pipeline/step_07_art_style_generation/plugin.py
pipeline/step_09_art_plan/plugin.py
core/engines/generation.py
pipeline/artifact_layer/registry.json
pipeline/artifact_layer/dependency_graph.json
```

建议新增测试：

```text
core/tests/unit/test_step05_to_step09_structured_contract_chain.py
core/tests/unit/test_step07_style_confirmation_gate.py
```

## Step05 程序审查

输入：

- Step03 `program_requirements_contract.json`
- Stage02 playable contracts。

输出：

- `program_ai_review_report.json`
- 程序需求阻断项。
- playable contract 覆盖检查。

禁止：

- 仅从 Markdown 判断程序完整性。

## Step06 美术审查

输入：

- Step04 `asset_spec_contract.json`
- `asset_mount_contract`
- `ui_flow_contract`
- `scene_bootstrap_contract`

输出：

- `art_ai_review_report.json`
- 可消费资源问题列表。

## Step07 美术风格确认

输入：

- Step06 art review。
- Step04 asset requirements。

输出：

- `style_options.json`
- `style_confirmation.json`
- `style_application_contract.json`

确认规则：

- 正式流程必须保留 `style_confirmation.json` 的人工确认机制。
- 只有 `status == "approved"` 时，才能产出可被 Step09 消费的 `style_application_contract.json`。
- `ctx.test_mode == True` 时允许直接 success，但端到端测试需要显式覆盖这一分支。
- 自动化测试可以使用预置 approved fixture，不允许删除人工确认门禁。
- 如果未来允许结构化美术方向决策自动确认，必须在计划中另行定义确认来源和风险提示。

注意：

- 音频仍为 placeholder-first，不在 Step07 接入真实音频 AI。

## Step09 美术计划

输入：

- Step07 `style_application_contract.json`。
- `asset_mount_contract`。
- Step04 asset plan。

输出：

- `art_production_task_contract.json`
- 资源生产任务。

## artifact registry 条目

需要在 `pipeline/artifact_layer/registry.json` 中新增或确认以下 `schema_refs`：

| stage bundle | path | schema | consumed_by |
|---|---|---|---|
| `stage_05.program_review_bundle` | `outputs/artifacts/stage_05/program_ai_review_report.json` | `knowledge/schemas/ai_design/program_ai_review_report.schema.json` | Step08 |
| `stage_06.art_review_bundle` | `outputs/artifacts/stage_06/art_ai_review_report.json` | `knowledge/schemas/ai_design/art_ai_review_report.schema.json` | Step07, Step09 |
| `stage_07.art_style_generation_confirmation_bundle` | `outputs/artifacts/stage_07/style_application_contract.json` | `knowledge/schemas/ai_design/style_application_contract.schema.json` | Step09, Step12 |
| `stage_09.art_plan_bundle` | `outputs/artifacts/stage_09/art_production_task_contract.json` | `knowledge/schemas/ai_design/art_production_task_contract.schema.json` | Step10, Step12, Step13 |

这些条目目前部分已存在时，实施时应确认路径、schema 和 dependency graph 与新强消费路径一致，而不是重复新增。

## 执行步骤

1. 为 Step05/06/07/09 增加统一 structured context 读取。
2. 明确每个 Step 的必需输入 artifacts。
3. 按上表同步或确认 artifact registry 和 dependency graph。
4. 禁止 P0 逻辑回退 Markdown。
5. 保留 Step07 人工确认门禁。
6. 每个 Step 输出 traceability。
7. 缺上游结构化合同或 artifact 时 blocked。

## 完成标准

1. Step05/06/07/09 都能读取 Stage02 playable contracts 或对应上游 artifacts。
2. Step05/06/07/09 都不从 Markdown 解析 P0 字段。
3. Step07 正式流程保留 approved 确认门禁。
4. Step07 test_mode 行为有测试覆盖。
5. Step09 输出能被 Step10 消费。

## 不做事项

- 不实现 Step10/11/12。
- 不跳过 Step07 人工确认。
- 不接入真实音频生成 AI。
