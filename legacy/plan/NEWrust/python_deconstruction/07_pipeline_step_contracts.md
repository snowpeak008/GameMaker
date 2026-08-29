# Python Pipeline Step 契约

状态：草案。

## 注册表

`pipeline/_registry.json` 注册：

- D1-D4：设计前置。
- Step00-Step14：开发流水线。

## Step metadata

`core/registry.py` 定义：

- Step number。
- slug。
- title。
- requires。

## 依赖摘要

```text
00 -> 01 -> 02 -> 03
03 -> 04
03 -> 05
04 -> 06 -> 07
05 -> 08
07 -> 09
08 + 09 -> 10
10 -> 11 + 12
11 + 12 -> 13 -> 14
```

待补充：每个 plugin 的 source groups、outputs、artifact contracts、manual gates。

## 统一 StagePlugin 生命周期

`core.stage_plugin.StagePlugin.run()`：

```text
validate_inputs(context)
  -> execute(context)
  -> context.outputs.update(result.outputs)
  -> validate_outputs(context)
```

`StageContext`：

- `stage_id`
- `project_root`
- `inputs`
- `outputs`
- `metadata`
- `knowledge`
- `skills`
- `test_mode`
- `artifact_dir = get_stage_artifact_dir(stage_id)`

`StageResult.status` 可为：

- `success`
- `failed`
- `skipped`
- `blocked`
- `stopped`
- `waiting_confirmation`
- `completed_with_review`

## Step00-14 SourceGroup 初表

| Step | SourceGroup label | Pattern | Mode | Source type |
| --- | --- | --- | --- | --- |
| 00 | concept | `devflow_Concept_*` | latest | Concept |
| 01 | gameplay_framework_history | `devflow_GameplayFramework_*` | all | GameplayFramework |
| 02 | 2a_subsystem_design | `devflow_SubsystemDesign_*` | latest | SubsystemDesign |
| 02 | 2b_ai_design_script | `devflow_AIDesignScript_*` | latest | AIDesignScript |
| 02 | 2c_design_package | `devflow_Design_*` | latest | Design |
| 02 | 2c_development_design | `devflow_DevelopmentDesign_*` | latest | DevelopmentDesign |
| 03 | program_requirements | `devflow_ProgReq_*` | latest | ProgReq |
| 04 | art_requirements | `devflow_ArtReq_*` | latest | ArtReq |
| 05 | program_review | `devflow_ProgReview_*` | latest | ProgReview |
| 06 | art_review | `devflow_ArtReview_*` | latest | ArtReview |
| 07 | none | none | special | style confirmation |
| 08 | program_plans | `devflow_Plans_*` | latest | Plans |
| 09 | art_plans | `devflow_ArtPlans_*` | latest | ArtPlans |
| 10 | asset_alignment | `devflow_Alignment_*` | latest | Alignment |
| 11 | dev_execution | `devflow_DevExecution_*` | latest | DevExecution |
| 12 | art_production | `devflow_ArtProduction_*` | latest | ArtProduction |
| 13 | scene_assembly | `devflow_SceneAssembly_*` | latest | SceneAssembly |
| 14 | integration_validation | `devflow_Integration_*` | latest | Integration |

Most Step00-14 plugins follow:

```text
run_import_step(stage_id, source_groups)
  -> apply_development_plan_outputs(stage_id, report)
  -> StageResult(status=result.status, outputs=result)
```

## 特殊阶段契约

### D1-D3

Inherit `DesignStagePlugin`:

- load `knowledge/design_data` through `load_project_data()`.
- summarize domains, nodes, checklist, option groups, options, validation warnings/errors.
- write `design_stage_summary.json`.

D2/D3 include additional custom validation/decision behavior and require deeper read.

### D4 DevFlow Handoff

`pipeline/step_d4_devflow_handoff/plugin.py`:

- calls `export_concept_package()`.
- writes `conceptPackage` into outputs.
- reads `structured_handoff.validation.status`.
- returns `blocked` when validation blocked unless in test mode.

NEWrust implication：D4 is not a generic content step; it is a bridge from design workbench to development pipeline source artifacts.

### Step07 Art Style Generation

Special behavior:

- No source groups.
- Reads current `style_confirmation.json`.
- Can reuse legacy confirmation from `LEGACY_ART_STYLE_CONFIRMATION_STAGE`.
- If approved confirmation and `style_options.json` exist, uses `confirmation_resume`.
- Otherwise runs generation and preserves approved confirmation.

NEWrust implication：Step07 requires a human confirmation state machine, not only content generation.

### Step13 Scene Assembly

Special behavior:

- Normal import/generation plus optional standalone metadata from `ctx.metadata`.
- Adds `validation_scope=standalone` and artifact source version when present.

### Step14 Integration Validation

Special behavior:

- Before import/generation, blocks if Step11/12/13 are `completed_with_review`、`blocked`、`failed`, unless config `pipeline.unattended_execution.continue_after_completed_with_review` allows continuation.
- Supports standalone mode metadata.
- In `standalone_partial`, converts blocked result into `completed_with_review` and preserves blockers as warnings.

NEWrust implication：Step14 must model review-blocker semantics explicitly; local/static evidence cannot be treated as final real integration acceptance.
