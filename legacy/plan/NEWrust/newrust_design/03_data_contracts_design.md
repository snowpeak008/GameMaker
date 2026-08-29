# Data Contracts Design

状态：第一轮设计完成。

evidence=

- `python_deconstruction/04_data_model_and_storage.md`
- `python_deconstruction/13_save_and_execution_object_contracts.md`
- `python_deconstruction/15_artifact_schema_refs_map.md`
- `python_deconstruction/16_ai_interview_and_completion_contracts.md`
- `python_deconstruction/17_packaging_contracts.md`
- `python_deconstruction/20_parity_gate_test_matrix.md`

classification=NEWrust authoritative design

confidence=high

open_questions=none

next_read_targets=

- Python sample JSON files under `drafts/`, `saves/`, `outputs/` only as generated-runtime-data samples.

## 1. Contract Families

| family | Rust module | Python evidence |
| --- | --- | --- |
| Project state | `contracts::project` | `DesignEngine.empty_state/normalize_state` |
| Design taxonomy | `contracts::design_data` | `knowledge/design_data` loader |
| Save/archive | `contracts::save` | `core/save/manager.py` |
| Execution object | `contracts::execution_object` | `core/engines/execution_objects` |
| Source package | `contracts::source_package` | `core/design/export_adapter.py` |
| Structured handoff | `contracts::handoff` | `core/design/structured_handoff.py` |
| Pipeline stage | `contracts::pipeline` | `core/context.py`, `core/stage_plugin.py` |
| Artifact registry | `contracts::artifact` | `pipeline/artifact_layer/registry.json` |
| AI interview | `contracts::ai_interview` | `core/design/ai_schema.py`, `ai_interview.py` |
| AI config | `contracts::ai_config` | `settings/ai_config.json`, `core/config` |
| Patch | `contracts::patch` | `core/patch/record.py` |
| SDK | `contracts::sdk` | `core/sdk/knowledge_base.py` |
| Log | `contracts::log` | `core/ui/log_entry.py` |
| Package | `contracts::package` | `core/packaging/*` |

## 2. Schema Version Policy

Every persisted struct must include:

- `schema_version` or `schemaVersion`, matching Python source when compatibility matters。
- `created_at` / `updated_at` when Python has it。
- explicit enum string values for persisted states。
- unknown-field rejection in validation path, even if serde can ignore unknown fields in migration path。

Rust pattern：

```text
Raw JSON -> migration adapter -> typed struct -> validator -> repository write
```

## 3. Project State

Core structs：

- `ProjectState`
- `ProjectProfile`
- `NodeState`
- `ChecklistState`
- `ChecklistOptionState`
- `GameplaySystemsState`
- `AiInterviewState`

Required behavior：

- `ProjectState::empty()` mirrors Python defaults。
- `normalize()` fills missing nodes and aiInterview fields。
- `effective_node_state()`、coverage、L4 progress、quality metrics are Rust domain functions。
- `option_provenance` must preserve source semantics：user_selected、user_confirmed_ai、ai_inferred、migration_inferred。

## 4. Save and Workspace Contracts

Rust repositories：

- `DraftWorkspaceRepository`
- `SaveArchiveRepository`
- `SaveIndexRepository`
- `ArchiveLockRepository`
- `SnapshotRepository`

Important contracts：

| file | model |
| --- | --- |
| `saves/save_index.json` | `SaveIndex` |
| `saves/<save_id>/manifest.json` | `SaveManifest` |
| `drafts/<session>/draft_meta.json` | `DraftMeta` |
| `drafts/<session>/autosave_state.json` | `AutosaveState` |
| `.archive_lock` | `ArchiveLock` |
| `snapshot_manifest.json` | `SnapshotManifest` |
| `snapshot_file_map.json` | `SnapshotFileMap` |

Lock semantics：

- atomic create。
- stale process detection。
- release on close。
- no cross-save overwrite without lock。

## 5. Artifact and Pipeline Contracts

Registry must be data-driven：

- no hardcoded Step00-14 UI order beyond registry/dependency model。
- topological order derives from `depends_on`。
- preflight must validate tasks/reviewers/validators/schema_refs/knowledge_refs。

Stage result enum：

```text
success
failed
skipped
blocked
stopped
waiting_confirmation
completed_with_review
```

## 6. AI Contracts

Schema modes：

- `turn`
- `readiness`
- `full_output`
- `partial_output`
- `mapping`
- `summary`

Rust must validate：

- top-level mode enum。
- assistantMessage presence。
- questionGroup max 4。
- partial output domain containment。
- full project state normalize success。
- confidence map parse and threshold logic。

High-confidence threshold fixed：`0.75`。

## 7. Package Contracts

Persisted outputs：

- `build_report.json`
- `package_validation_report.json`
- `PACKAGE_NOTES.md`
- `package_manifest.json`

Rust typed models：

- `PackageValidationReport`
- `PackageBuildReport`
- `PackageManifest`
- `PackageBlockingIssue`
- `PackageRequiredCheck`

`changed_files` empty is blocked even if Step14 status is success。
