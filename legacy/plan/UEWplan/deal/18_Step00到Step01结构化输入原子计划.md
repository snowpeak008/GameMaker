# Step00到Step01结构化输入原子计划

## 目标

让 Step00 和 Step01 不再只依赖 Markdown 推断项目类型和玩法框架，而是优先读取设计工作台导出的结构化 profile、archetype 和核心玩法选择。

如果 Step00/01 不纳入结构化链路，Step02 冻结的 playable contracts 仍然会建立在不可信的上游推断上。

## 依赖

- `07_D4结构化Handoff导出原子计划.md`
- `08_StructuredDesignContext统一加载原子计划.md`

## 涉及范围

重点修改：

```text
pipeline/step_00_idea_intake/plugin.py
pipeline/step_01_gameplay_framework/plugin.py
pipeline/step_00_idea_intake/helpers.py
core/engines/generation.py
```

建议新增测试：

```text
core/tests/unit/test_step00_structured_profile_input.py
core/tests/unit/test_step01_structured_gameplay_framework.py
```

## Step00 输入

Step00 优先读取：

- D4 `structured/profile.json`
- D4 `structured/archetype_requirements.json`
- D4 `structured/decisions.json`

Markdown 只能作为补充摘要。

## Step00 输出

Step00 应输出：

- `concept_profile.json`
- `intent_interpretation_contract.json`
- profile/archetype traceability。

## Step01 输入

Step01 优先读取：

- Step00 `concept_profile.json`
- D4 playable contract candidates。
- D4 decisions。

## Step01 输出

Step01 应输出：

- gameplay framework。
- core loop summary。
- system graph。
- archetype-specific required contracts。

## 执行步骤

1. Step00 加载 `StructuredDesignContext`。
2. Step00 使用 structured profile 覆盖 Markdown 推断结果。
3. Step00 输出 profile traceability。
4. Step01 加载 Step00 输出和 D4 decisions。
5. Step01 根据 archetype 生成 gameplay framework。
6. Step01 输出合同消费报告。
7. 缺结构化 profile 时允许 fallback，但必须 warning，不能作为完整设计通过依据。

## 完成标准

1. Step00 可以从 structured profile 生成 concept profile。
2. Step01 可以从 structured decisions 生成 gameplay framework。
3. Markdown 中的关键词不能覆盖结构化 profile。
4. Step01 输出能被 Step02 读取和追溯。

## 不做事项

- 不生成 playable contracts 正式 artifacts。
- 不修改 Step02 冻结逻辑。
- 不做类型专用硬编码。
