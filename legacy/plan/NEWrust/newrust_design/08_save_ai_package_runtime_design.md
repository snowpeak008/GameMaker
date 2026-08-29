# Save, AI, Package, and Runtime Design

状态：第一轮设计完成。

evidence=

- `python_deconstruction/08_runtime_save_ai_package_flow.md`
- `python_deconstruction/13_save_and_execution_object_contracts.md`
- `python_deconstruction/14_ai_config_adapter_log_contracts.md`
- `python_deconstruction/16_ai_interview_and_completion_contracts.md`
- `python_deconstruction/17_packaging_contracts.md`

classification=NEWrust authoritative design

confidence=high

open_questions=none

next_read_targets=none

## 1. Save Service

Save service owns:

- current draft workspace。
- formal archive。
- save index。
- archive locks。
- snapshot full/delta。
- timeline。
- autosave。

Critical parity:

- closing window releases lock。
- deleted current save becomes unsaved copy state。
- manual save uses execution object flow。
- load save copies formal workspace to active draft and acquires lock。

## 2. AI Service

AI service owns:

- AI config categories。
- schema-mode backend calls。
- prompt construction。
- prompt meter/replay。
- payload validation。
- high-confidence writeback。
- archive。
- framework memory。
- UCOS bridge or equivalent memory event。

Design split:

| service | role |
| --- | --- |
| `AiInterviewService` | project AI interview state machine |
| `StructuredCompletionService` | generic JSON completion for patch/sdk |
| `AiConfigService` | settings/ai_config typed load/save/validate |
| `AiMemoryService` | framework memory and UCOS event writing |

Embedded and standalone interview paths must call the same backend service。

## 3. Package Service

Packaging service repeats validation regardless of UI readiness:

```text
load stage14 integration
load actual_project_file_audit
load unity_validation_summary
evaluate required checks
write validation report
write build report
write notes
write manifest
```

Blocked package generation still writes evidence files。

## 4. Runtime Logs

Run logs are structured JSONL:

- timestamp
- level
- context
- message
- source
- metadata

Pipeline and long jobs must write both:

- persisted JSONL。
- optional Tauri progress events。

## 5. Runtime Data Policy

Generated runtime data may be read as samples during migration, but NEWrust tests must create fresh temp roots。

No test may depend on current user `drafts/` or `saves/` content。
