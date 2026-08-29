# Application Services Design

状态：第一轮设计完成。

evidence=

- `python_deconstruction/08_runtime_save_ai_package_flow.md`
- `python_deconstruction/13_save_and_execution_object_contracts.md`
- `python_deconstruction/16_ai_interview_and_completion_contracts.md`
- `python_deconstruction/17_packaging_contracts.md`
- `python_deconstruction/20_parity_gate_test_matrix.md`

classification=NEWrust authoritative design

confidence=high

open_questions=none

next_read_targets=none

## 1. Service List

| service | responsibility | primary commands |
| --- | --- | --- |
| `ShellService` | main status, active route bootstrap, geometry/settings summary | `get_shell_state` |
| `DesignWorkbenchService` | project state read/write, profile, node edits, export, templates | `load_design`, `update_node`, `export_design` |
| `SaveService` | save manager, autosave, load/delete/rename, lock lifecycle | `list_saves`, `save_project`, `load_save` |
| `AiInterviewService` | interview turn, schema backend, archive, high-confidence writeback | `submit_ai_turn`, `force_ai_output`, `archive_interview` |
| `PipelineService` | run range, stop, status, Step07 confirmation, export to pipeline | `run_pipeline_range`, `stop_pipeline` |
| `ArtifactService` | dependency graph, preflight/review/validation reports | internal and diagnostics commands |
| `PatchService` | patch analyze/list/read | `analyze_patch`, `list_patches` |
| `PackagingService` | package current project after Step14 | `package_current_project` |
| `RunLogService` | latest logs, filter, export | `list_run_logs`, `read_run_log` |
| `SdkKnowledgeService` | SDK add/update/list/approved context | `add_sdk`, `update_sdk_status` |
| `AiConfigService` | AI config categories and validation | `load_ai_config`, `save_ai_config` |

## 2. Transaction Pattern

Every mutating service method:

```text
validate input
load current typed state
acquire lock if required
apply domain service
run validators
write files atomically
write evidence/log
release lock when operation lifecycle ends
return view model
```

UI never performs partial filesystem updates。

## 3. Error Model

Use typed application errors：

| error | use |
| --- | --- |
| `ValidationError` | bad payload, schema mismatch, invalid state |
| `BlockedError` | gate blocked with evidence |
| `LockError` | save or pipeline lock conflict |
| `BackendUnavailable` | AI/Codex/OpenAI/Claude unavailable |
| `IoEvidenceError` | write/read failed after evidence context |
| `NotFound` | save/sdk/patch/stage absent |
| `Cancelled` | user requested stop |

Every error returned to UI includes:

- code
- human message
- evidence refs
- recoverability hint

## 4. Background Jobs

Background jobs:

- AI turn。
- AI full output partitions。
- pipeline run。
- package generation。
- patch analysis。
- SDK extraction。

Job model：

```text
JobId
JobKind
status: queued|running|completed|failed|cancelled
started_at
updated_at
progress
log_refs
result_ref
```

Tauri event streaming may be used for progress, but persisted state remains in Rust repositories。

## 5. Service to View Model

Application services return view models, not raw domain internals where UI needs derived state。

Examples：

- `DesignWorkbenchView`: domains with coverage, current domain, nodes, result tabs, ai panel state。
- `PipelineView`: grouped steps, selected step detail, running state, log tail。
- `PackageView`: step14 readiness, last package result, blockers。
- `SdkView`: rows and approved context。

Derived fields must be computed in Rust services to avoid Web duplicating Python/Rust domain logic。
