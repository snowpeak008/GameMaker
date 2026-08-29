# Pipeline and Artifact Engine Design

状态：第一轮设计完成。

evidence=

- `python_deconstruction/07_pipeline_step_contracts.md`
- `python_deconstruction/09_artifact_validation_flow.md`
- `python_deconstruction/15_artifact_schema_refs_map.md`
- `python_deconstruction/20_parity_gate_test_matrix.md`

classification=NEWrust authoritative design

confidence=high

open_questions=none

next_read_targets=none

## 1. Pipeline Registry

Rust must load a data registry equivalent to `pipeline/_registry.json`。

Model:

- `StageId`
- `StageKind`
- `StageSpec`
- `StageDependency`
- `StagePluginRef`
- `StageStatus`

Stage order is topological, not hardcoded。

## 2. Stage Execution

```text
run_range(from, to)
  -> ensure_run_context
  -> preflight project/runtime
  -> emit_dependency_graph
  -> topological_order
  -> for each stage:
       check stop signal
       load stage runner
       preflight_stage_contract
       execute stage
       review pipeline
       artifact validators
       update step state
       save sync
       write run state/log
```

Initial implementation may wrap Python-equivalent deterministic generators where no AI is needed, but all outputs must match typed contracts。

## 3. Artifact Layer

`adm-new-artifact` owns:

- registry loader。
- dependency graph。
- preflight。
- reviewer set。
- validator set。
- reference manifest refresh。

Reviewer whitelist:

- structure_reviewer
- source_trace_reviewer
- task_reviewer
- dependency_reviewer

Validators:

- validator_first_contract
- stage_files_validator
- review_report_validator
- manifest_validator
- schema_contract_validator
- knowledge_refs_validator
- dependency_status_validator

## 4. Step07 Human Gate

Step07 has special UI state:

- waiting style options。
- user selects style。
- notes captured。
- `style_confirmation.json` written。
- rerun/regenerate path available。

Pipeline runner must represent:

```text
waiting_confirmation
approved
regenerate_requested
```

## 5. Stop Handling

Stop is cooperative:

- runtime control writes stop request。
- runner checks before and after stage。
- status becomes `stopped`。
- logs/evidence record stop request。

No thread/process kill as primary control path。
