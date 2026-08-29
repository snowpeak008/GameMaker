# Step10到Step12执行对齐与生产链路原子计划

## 目标

补齐 Step10、Step11、Step12 的结构化合同消费，让程序任务、资源任务、开发执行对象和资源生产结果可以可靠进入 Step13。

该计划必须在 Step08 输出 schema 明确之后执行，因为 Step11 需要读取 Step08 的程序计划和场景装配前置任务。

## 依赖

- `12_Step08程序计划与场景装配前置任务原子计划.md`
- `19_Step05到Step09前置审查与美术链路原子计划.md`

## 涉及范围

重点修改：

```text
pipeline/step_10_asset_alignment/plugin.py
pipeline/step_11_dev_execution/plugin.py
pipeline/step_12_art_production/plugin.py
core/engines/generation.py
pipeline/artifact_layer/registry.json
pipeline/artifact_layer/dependency_graph.json
```

建议新增测试：

```text
core/tests/unit/test_step10_to_step12_structured_contract_chain.py
core/tests/unit/test_step11_playable_execution_objects_verified.py
```

## Step10 资产对齐

输入：

- Step08 `program_plan_contract.json`
- Step08 `scene_assembly_task_requirements.json`
- Step08 `ui_runtime_task_requirements.json`
- Step08 `input_runtime_task_requirements.json`
- Step08 `objective_runtime_task_requirements.json`
- Step09 `art_production_task_contract.json`
- Stage02 playable contracts。

输出：

- 代码任务与资源任务对齐报告。
- 缺失资源 fallback 决策。
- Step13 可消费的 mount readiness summary。

## Step11 开发执行

输入：

- Step08 program plan。
- Step08 scene/UI/input/objective task requirements。
- Step10 asset alignment。
- Stage02 playable contracts。

输出：

- 开发执行对象。
- RuntimeBootstrap、UI、Input、Objective、Scene hooks 的执行状态。

要求：

- 每个与 playable contract 相关的执行对象必须有 traceability。
- EO 状态闭环必须继续保持 verified 门禁。
- 缺 Step08 输出时 Step11 必须 blocked，不能 fallback 到 Markdown。

## Step12 美术生产

输入：

- Step09 art plan。
- Step10 asset alignment。
- `asset_mount_contract`。
- `audio_requirements_contract`。

输出：

- 资源生产结果。
- audio placeholder runtime manifest。
- Unity 可挂载资源路径。

## artifact registry 条目

需要在 `pipeline/artifact_layer/registry.json` 中新增或确认以下 `schema_refs`：

| stage bundle | path | schema | consumed_by |
|---|---|---|---|
| `stage_10.asset_alignment_bundle` | `outputs/artifacts/stage_10/asset_alignment_report.json` | `knowledge/schemas/ai_design/asset_alignment_report.schema.json` | Step11, Step12, Step13 |
| `stage_10.asset_alignment_bundle` | `outputs/artifacts/stage_10/mount_readiness_summary.json` | `knowledge/schemas/ai_design/mount_readiness_summary.schema.json` | Step13 |
| `stage_11.dev_execution_bundle` | `outputs/artifacts/stage_11/dev_execution_report.json` | `knowledge/schemas/ai_design/dev_execution_report.schema.json` | Step13 |
| `stage_11.dev_execution_bundle` | `outputs/execution_objects/execution_objects.json` | `knowledge/schemas/execution_object_workflow.schema.json` | Step13 |
| `stage_12.art_production_bundle` | `outputs/artifacts/stage_12/art_production_report.json` | `knowledge/schemas/ai_design/art_production_report.schema.json` | Step13 |
| `stage_12.art_production_bundle` | `outputs/artifacts/stage_12/audio_placeholder_manifest_runtime.json` | `knowledge/schemas/playable_contracts/audio_placeholder_manifest_runtime.schema.json` | Step13, Step14 |

如果 schema 文件尚不存在，必须在本计划实施时创建最小 schema 或复用现有 schema，不能只声明输出文件。

## 新增 schema 最低结构

本计划新增或确认的 schema 至少要定义到 Step13 可以可靠消费的字段层级。字段可继续扩展，但不能低于以下 required 集合。

### `asset_alignment_report.schema.json`

```json
{
  "required": ["schema_version", "alignment_items", "gaps"],
  "properties": {
    "schema_version": { "type": "string" },
    "alignment_items": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["program_task_id", "asset_id", "status", "source_refs"],
        "properties": {
          "program_task_id": { "type": "string" },
          "asset_id": { "type": "string" },
          "status": {
            "type": "string",
            "enum": ["aligned", "fallback_required", "missing", "blocked"]
          },
          "source_refs": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "gaps": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["gap_id", "severity", "message"],
        "properties": {
          "gap_id": { "type": "string" },
          "severity": { "type": "string", "enum": ["info", "warning", "blocking"] },
          "message": { "type": "string" }
        }
      }
    }
  }
}
```

### `mount_readiness_summary.schema.json`

```json
{
  "required": ["schema_version", "ready", "mount_items"],
  "properties": {
    "schema_version": { "type": "string" },
    "ready": { "type": "boolean" },
    "mount_items": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["target_path", "asset_id", "fallback_policy"],
        "properties": {
          "target_path": { "type": "string" },
          "asset_id": { "type": "string" },
          "fallback_policy": {
            "type": "string",
            "enum": ["none", "placeholder_allowed", "block_on_missing"]
          },
          "source_refs": { "type": "array", "items": { "type": "string" } }
        }
      }
    }
  }
}
```

### `dev_execution_report.schema.json`

```json
{
  "required": ["schema_version", "execution_records", "verified_execution_objects"],
  "properties": {
    "schema_version": { "type": "string" },
    "execution_records": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["task_id", "execution_object_id", "state", "source_refs"],
        "properties": {
          "task_id": { "type": "string" },
          "execution_object_id": { "type": "string" },
          "state": {
            "type": "string",
            "enum": ["planned", "approved", "executing", "execution_failed", "verified"]
          },
          "source_refs": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "verified_execution_objects": {
      "type": "array",
      "items": { "type": "string" }
    }
  }
}
```

### `art_production_report.schema.json`

```json
{
  "required": ["schema_version", "produced_assets", "missing_assets"],
  "properties": {
    "schema_version": { "type": "string" },
    "produced_assets": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["asset_id", "path", "mount_point", "source_refs"],
        "properties": {
          "asset_id": { "type": "string" },
          "path": { "type": "string" },
          "mount_point": { "type": "string" },
          "source_refs": { "type": "array", "items": { "type": "string" } }
        }
      }
    },
    "missing_assets": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["asset_id", "reason", "fallback_policy"],
        "properties": {
          "asset_id": { "type": "string" },
          "reason": { "type": "string" },
          "fallback_policy": {
            "type": "string",
            "enum": ["placeholder_allowed", "block_on_missing"]
          }
        }
      }
    }
  }
}
```

## 执行步骤

1. 为 Step10/11/12 增加统一 structured context 读取。
2. 明确每个 Step 的必需输入 artifacts。
3. 按上表同步或确认 artifact registry 和 dependency graph。
4. 禁止 P0 逻辑回退 Markdown。
5. Step10 对齐 Step08 程序任务和 Step09 资源任务。
6. Step11 生成并验证 playable 相关 EO。
7. Step12 输出 Step13 可挂载资源。

## 完成标准

1. Step10 能证明程序任务和资源任务已对齐。
2. Step11 读取 Step08 输出 schema，不依赖未定义格式。
3. Step11 的 playable 相关 EO 必须 verified。
4. Step12 输出能被 Step13 消费。
5. Step13 不再接收不可信中间输入。

## 不做事项

- 不实现 Step05/06/07/09。
- 不跳过中间步骤直接生成场景。
- 不把资源生产和场景装配合并。
