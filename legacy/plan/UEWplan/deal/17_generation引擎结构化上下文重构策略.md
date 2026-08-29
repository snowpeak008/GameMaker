# generation引擎结构化上下文重构策略

## 目标

在修改 `core/engines/generation.py` 前，先建立清晰的重构边界，避免多个原子计划反复直接改 8000 行以上的大文件，造成 Step 之间互相破坏。

该计划不是大重构实施计划，而是约束后续 Step 改造的架构策略。

## 依赖

- `07_D4结构化Handoff导出原子计划.md`

## 当前风险

`core/engines/generation.py` 当前承担：

- Step00-14 输出生成。
- Markdown 设计文档解析。
- playable contract bundle 读写。
- Unity 场景和测试生成。
- AI 审查和资源计划生成。

如果每个 Step 计划都直接修改 `_stage{n}_outputs()`，会导致：

- `_parse_design_doc()` 兼容逻辑被误删。
- Stage02 playable contracts 输出路径被破坏。
- Step05-09、Step08、Step10-12 中间链路断裂。
- 旧测试大量失败且原因难以定位。

## 重构原则

### 1. 先加边界，不先搬空文件

第一阶段不做大拆分，只新增独立 helper：

```text
core/engines/source_context.py
core/design/structured_context.py
core/design/structured_handoff.py
```

`generation.py` 只通过这些 helper 获取结构化输入。

### 2. Markdown 只能作为兼容 fallback

`_parse_design_doc()` 可以保留，但不能作为 P0 合同来源。

允许用途：

- 旧项目迁移提示。
- 非阻断性摘要。
- 人类可读报告。

禁止用途：

- 生成 `ui_flow_contract`。
- 生成 `scene_bootstrap_contract`。
- 生成 P0 程序任务。
- 生成 Step13/14 验收依据。

### 3. 每个 Step 只改自己的输入适配层

后续 Step 改造时优先新增：

- `resolve_stage_inputs(stage_id, output_base_dir)`
- `load_playable_contracts(stage2_dir)`
- `load_stage_artifact(stage_id, artifact_name)`

不要在每个 `_stage{n}_outputs()` 中重复写路径解析。

### 4. 保持现有 artifact registry 对齐

所有正式输出必须对齐：

```text
pipeline/artifact_layer/registry.json
pipeline/artifact_layer/dependency_graph.json
```

新增 artifacts 必须同步 registry。

## 执行步骤

1. 统计 `generation.py` 中 `_parse_design_doc()` 和 `_parse_design_text()` 的调用点。
2. 标记哪些调用属于 P0 合同来源。
3. 新增 source/context helper，不改变现有行为。
4. 给 Step02/03/04/08/13/14 添加读取 helper 的最小接入点。
5. 给 Step00/01、Step05-09、Step08、Step10-12 添加兼容接入点。
6. 新增测试确保 Markdown fallback 不会覆盖 structured contract。
7. 后续每个 Step 原子计划只在该策略允许范围内修改。

## 完成标准

1. 有统一 helper 负责 structured handoff 和 artifacts 加载。
2. `generation.py` 不新增重复路径解析。
3. P0 合同字段不再从 Markdown 解析。
4. 旧 Markdown fallback 有 warning。
5. 相关测试能证明 structured input 优先。

## 不做事项

- 不一次性拆分整个 `generation.py`。
- 不删除旧 Markdown 兼容逻辑。
- 不改变所有 Step 的业务输出。
