# Risk Register

状态：第一轮设计完成。

evidence=

- `python_deconstruction/18_garbage_isolation_draft.md`
- `python_deconstruction/20_parity_gate_test_matrix.md`
- `python_deconstruction/scorecard.md`

classification=NEWrust authoritative design

confidence=high

open_questions=none

next_read_targets=none

## 1. Major Risks

| risk | severity | mitigation |
| --- | --- | --- |
| UI first implementation drifts from backend contracts | high | contract/service/Tauri before UI; command tests required |
| Old `RUST/` structure contaminates NEWrust | high | reference only; no Slint continuation |
| Python garbage treated as product requirement | high | source authority + garbage isolation gate |
| AI interview paths diverge | high | one `AiInterviewService`; embedded and standalone routes share service |
| Web UI duplicates domain logic | high | derived fields computed in Rust view models |
| Save lock corruption | high | atomic lock repository and temp-root tests |
| Pipeline fake success | high | artifact preflight/review/validator required |
| Package without real changed files | high | package validation blocks empty changed_files |
| Screenshot-only UI acceptance | medium | DOM behavior + screenshot + backend evidence |
| Over-scoring self-review | medium | Red Team role remains in every stage |

## 2. Hard Gates

- No code development before design score passes。
- No UI task before command/service contract task exists。
- No package success without Step14 source evidence。
- No AI writeback without schema validation and confidence threshold。
- No save write without lock or explicit unsaved draft mode。

## 3. Deferred Items

- `core/ui/workbench.py` remains reference/defer, not in core scope。
- `tools/build/*` only enters release gate if directly needed。
- Historical Step15-17 remains excluded。

## 4. Drift Response

If drift is detected:

1. Stop current implementation。
2. Re-read `plan/NEWrust/README.md`。
3. Re-read current phase scorecard。
4. Identify violated gate。
5. Patch design/plan first。
6. Resume only after scoring passes。
