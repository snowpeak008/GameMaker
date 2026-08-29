# Testing, Gates, and Release Design

状态：第一轮设计完成。

evidence=

- `python_deconstruction/20_parity_gate_test_matrix.md`
- `NEWrust/README.md`
- `plan/NEWrust/00_execution_protocol.md`

classification=NEWrust authoritative design

confidence=high

open_questions=none

next_read_targets=none

## 1. Test Layers

| layer | target |
| --- | --- |
| Rust unit | domain services and validators |
| Rust integration | repositories with temp project roots |
| Tauri command | request/response and error mapping |
| Web component | view model rendering |
| Playwright e2e | six task views and critical workflows |
| Gate scripts | plan/score/artifact/package/release |

## 2. Required Parity Tests

- MainWindow route switching and status bar。
- Design project normalize/edit/export/save。
- AI schema validation and high-confidence writeback。
- Pipeline dependency order and stage status transitions。
- Step07 confirmation states。
- Artifact preflight/review/validator blocked cases。
- Patch analyze success/failure。
- Package success/blocked cases。
- SDK review status transitions。
- Log level filter and JSONL parse。

## 3. Gate Reports

Each gate emits a report:

```text
gate_name
status: success|blocked|failed
generated_at
checks[]
evidence_refs[]
blocking_issues[]
```

Reports live under `NEWrust/gates/reports/` during development and package output during release。

## 4. Release Rule

Release cannot pass unless:

- `cargo fmt --check` passes。
- `cargo test --workspace` passes。
- Web build passes。
- Playwright parity passes。
- plan gate passes。
- package gate passes。
- no hardcoded fake evidence。

## 5. Atomic Task Acceptance

Every atomic task must state:

- evidence file from Python decomposition。
- Rust files changed。
- Web files changed if any。
- test command。
- expected evidence artifact。

No task can close with “manual only” verification unless explicitly marked as visual review and paired with screenshot。
