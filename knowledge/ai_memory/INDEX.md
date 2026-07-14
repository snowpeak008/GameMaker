# AI 会话记忆索引

> 最后更新：2026-07-15
> 缓存状态：✓ 有效

---

## 上次会话摘要

**Date**: 2026-07-15
**ID**: 2026-07-15-001
**Summary**: Reframed the NEWrust redesign as a generic, capability-composed game specification compiler with a single canonical state and deterministic gates; PvZ is only one validation fixture and multi-role AI workflows are excluded.

**Confirmed Constraints**:
- [x] Core schemas, defaults, routing, and quality criteria must be project-generic; PvZ cannot define any of them.
- [x] CrewAI-style role conversations, role reviews, AI voting, and AI-directed stage routing are prohibited from the authoritative path.
- [x] AI may return bounded candidate patches, but only a single deterministic compiler/store may validate and commit canonical state.
- [x] Existing 16-domain/103-node/515-check content becomes an activation-aware knowledge library rather than an always-complete project template.

**Research and Diagnosis**:
- [x] Verified all retained GitHub references exceed 5K stars and excluded the 1,093-star `game-ci/unity-actions` candidate.
- [x] Confirmed current overfitting in the built-in `tower_defense` archetype, Unity-specific generic contracts, app-config-based D1 portrait, and question-count-based MDA routing.
- [x] Defined a cross-genre validation matrix and anti-overfitting rule; no production code or current plan was changed in this analysis turn.

---

**Date**: 2026-07-14
**ID**: 2026-07-14-005
**Summary**: Fixed NEWrust pipeline sizing and selected-stage rendering, simplified AI configuration presentation, and repaired Step07 Windows image generation that had silently fallen back to low-resolution placeholders.

**Completed**:
- [x] Increased the pipeline step rail to 330px and step cards to a 124px minimum height while retaining horizontal overflow at constrained widths.
- [x] Limited Step07 images to the selected Step07 view and simplified all AI configuration lists to user-defined display names with compact active actions.
- [x] Rejected extensionless POSIX npm shims on Windows, selected `codex.cmd`, raised provider requests to 1536x1024, and strengthened project-specific prompts.
- [x] Scored the current Plants vs. Zombies Step00-06 artifacts independently: 76, 72, 70, 62, 45, 47, and 38.

**Verification**:
- [x] Rust workspace check/test and the full Web unit/E2E/build/i18n/language/UI gates passed.
- [x] Standalone boundary scanned 224 files with 0 forbidden parent-Python hits; root and portable smoke checks passed.
- [x] Installed development snapshot `42e2f76a...21318` while preserving 1201 local data files and 44,983,881 bytes exactly.

---

**Date**: 2026-07-14
**ID**: 2026-07-14-004
**Summary**: Copied the complete AI memory system, including legacy Python history, into NEWrust while keeping it outside the product runtime and replacing Python maintenance helpers with PowerShell.

**Completed**:
- [x] Copied the full historical memory tree and preserved the legacy Python freshness snapshot separately.
- [x] Added NEWrust-local AI entry rules, a 43-file Rust/Tauri and memory-system freshness configuration, and PowerShell update/test tools.
- [x] Defined legacy Python memories as read-only development context that cannot reintroduce parent-project runtime dependencies.

**Verification**:
- [x] Historical copy comparison: 288 files checked, 0 missing, 0 mismatched.
- [x] NEWrust freshness: 43 fresh, 0 stale, 0 missing; secret-pattern scan: 0 hits.
- [x] Standalone boundary: 224 files, 0 forbidden parent hits; portable products contain no `knowledge/ai_memory` directory.

---

**Date**: 2026-07-14
**ID**: 2026-07-14-003
**Summary**: Confirmed that the current NEWrust portable can be relocated to another Windows computer without consulting or depending on the parent Python project.

**Completed**:
- [x] Verified zero Python runtime files, zero parent-project absolute-path references, and zero Python references in the current portable `user_data`.
- [x] Recorded the required root layout: `.project_root`, root `AutoDesignMaker.exe`, and the complete `dist/AutoDesignMaker-NEWrust` tree must move together.
- [x] Recorded Windows 10/11 x64, WebView2, and writable-directory runtime requirements.

**Migration Caveat**:
- [ ] The preserved local `user_data/settings/project_bindings.json` contains one machine-specific Unity editor/project binding and must be reconfigured on the target computer.
- [ ] A formal clean-transfer package still requires a committed tree and a successful `NEWrust/tools/verify-standalone.ps1` run.

---

**Date**: 2026-07-14
**ID**: 2026-07-14-002
**Summary**: Completed NEWrust quality hardening for explicit load failures, targeted utility refresh, fail-closed UI evidence, non-overlapping Shell refresh, and opt-in safe blank-draft retention.

**Completed**:
- [x] Preserved backend failures across design, AI, patch, package, logs, SDK, and save surfaces instead of rendering false empty states.
- [x] Added per-target refresh versioning, Playwright-only UI evidence, safe draft-retention guards, shared Web helpers, and focused UI consistency fixes.
- [x] Synchronized the quality plan and atomic plans 009-015 with implementation and verification results.

**Verification**:
- [x] Rust workspace checks/tests, Web build/unit/e2e, 2573 bilingual keys, 90 UI screenshots, 93 baselines, standalone boundary, root launcher check, and root/portable smoke passed.
- [x] Parent Python forbidden hits remained 0; the root EXE still opens an independent blank Rust session by default.
- [x] The repaired source was installed as a development-snapshot portable EXE (`4c07249d...e895`) while preserving all 402 local `user_data` files.
- [ ] Formal release evidence must be regenerated from a committed clean tree with `NEWrust/tools/verify-standalone.ps1`.

---

**Date**: 2026-07-14
**ID**: 2026-07-14-001
**Summary**: Replaced the NEWrust source-root CMD entry with a native root `AutoDesignMaker.exe` that directly launches the manifest-bound Rust portable product and defaults to a blank project.

**Completed**:
- [x] Added the tested `adm-new-root-launcher` workspace package and atomic root-launcher build/install script.
- [x] Removed `NEWrust/Start-AutoDesignMaker.cmd`; the root EXE does not invoke the portable compatibility CMD or search the parent Python project.
- [x] Verified root layout/WebView2 checks, Rust-owned data routing, blank startup, x64 static CRT, actual GUI delegation, and synchronized plans/docs.

**Verification**:
- [x] Root EXE self-check, full workspace check/test, launcher Clippy `-D warnings`, and standalone boundary gate passed.
- [x] Actual launch created a fresh 103-node all-`not_started` draft with no notes, gameplay systems, or AI messages; pipeline state was idle with zero stages and shutdown was clean.

---

**Date**: 2026-07-12
**ID**: 2026-07-12-001
**Summary**: Extracted the current AI memory system into a reusable template package under `plan/memries/`, ready to copy into other projects.

**Completed**:
- [x] Created `plan/memries/README.md` with copy and activation instructions.
- [x] Created `plan/memries/COPY_ME/` with `AGENTS.md`, `AI_README.md`, `knowledge/ai_memory/`, and `tools/memory/`.
- [x] Generalized freshness scripts to read `memory_config.json` instead of hardcoded AutoDesignMaker key files.
- [x] Included session note scaffolding while excluding current project history, secrets, local config, and runtime artifacts.

**Verification**:
- [x] `python -m compileall tools\memory` passed inside `plan/memries/COPY_ME`.
- [x] `python tools\memory\update_freshness.py` passed inside `plan/memries/COPY_ME`.
- [x] `python tools\memory\check_staleness.py` passed with no stale or missing files.

---
**Date**: 2026-07-10
**ID**: 2026-07-10-005
**Summary**: Established `plan/fixplan/` as the shared cross-window directory for Rust second-development problems and solutions, with a Markdown recording convention.

**Completed**:
- [x] Added `plan/fixplan/README.md` with the shared recording template.
- [x] Recorded the decision in `knowledge/ai_memory/decisions/architecture.md` and this index.
- [x] Confirmed that future records must not contain secrets or other sensitive configuration.

---
**Date**: 2026-07-10
**ID**: 2026-07-10-004
**Summary**: Recovered and closed out the NEWrust project-template workflow completed after the prior memory write, refreshed the latest release/final-handoff gates, and recorded the current portable build.

**Recovered Stop Node**:
- [x] The previous memory stopped at the save-system closeout, but later source and runtime evidence showed the template workflow had also been completed.
- [x] The missing node was gate/document/session closeout, not a half-written implementation or a stopped pipeline stage.
- [x] The last NEWrust process shut down cleanly; the pipeline is idle and no process, recovery journal, corrupt quarantine, tombstone, or live lock remains.

**Completed**:
- [x] Rechecked Web → Tauri → application/design service → draft/archive persistence contracts for template list/apply/save/overwrite/delete.
- [x] Confirmed built-in protection, corrupt-file warnings, bilingual presentation, keyboard/confirmation/busy states, formal-save restore, and restart persistence.
- [x] Re-ran current Rust, Python-template, Web, language, screenshot, and UI-baseline checks.
- [x] Refreshed `release-gate` and `final-handoff-v3-gate` after the template source changes; both passed with zero blockers.
- [x] Updated active handoff documents to the current portable EXE hash and size.

**Verification**:
- [x] Rust workspace format/check/test: passed.
- [x] Python template baseline: 7/7.
- [x] Web unit/e2e, 1655 design-content values, 2489 bilingual keys, two languages × 12 surfaces, 56 screenshots, and 93 baselines: passed.
- [x] Final handoff: 379/379 inventory, 379/379 disposition, 379/379 mappings, 73/73 tests, 727 assets, and 93 UI records.
- [x] Portable resources: 156/156 exact path/length/SHA-256 matches.
- [x] Real Tauri template smoke and four clean shutdowns are present; the custom template survives restart.

**Current Handoff State**:
- [x] Launcher: `NEWrust/dist/AutoDesignMaker-NEWrust/Start-AutoDesignMaker.cmd`.
- [x] EXE SHA-256: `0e691d579d47783c85def909378d77c38d52295730daaed89b1325e5c8ba3d75` (21,350,912 bytes).
- [x] The template workflow and its closeout are complete; NEWrust is currently not running.
- [ ] Unity execution, live-provider output quality, and Step00-14 artifact quality remain target-environment user acceptance checks.
- [ ] Backup rotation and a dedicated recovery UI remain optional future hardening.
- [ ] Strict Clippy with `-D warnings` still reports one pre-existing `trim_split_whitespace` lint in foundation code.

---
**Date**: 2026-07-10
**ID**: 2026-07-10-003
**Summary**: Compared Python and NEWrust save design end to end, then completed a transaction-safe multi-window Tauri save manager with crash recovery, full UI states, tests, and a verified portable release.

**Completed**:
- [x] Split New Project Save, Save As Copy, and Save Current; added explicit save/discard/cancel switching.
- [x] Added responsive save details for draft, lock, integrity, progress, transaction, files, size, path, busy, corrupt, and localized errors.
- [x] Added OS draft/archive/index locks, bounded project-global transactions, complete before-image journals, commit markers, and cross-session runtime recovery.
- [x] Transactionalized create/blank/sync/load/rename/delete, including rename rollback and delete tombstones.
- [x] Added corrupt index/draft/pipeline quarantine, durable pipeline recovery, Python verified `design_project` fallback, and shutdown retry/lock retention.
- [x] Preserved unknown project/gameplay/AI fields and propagated post-commit warnings through diagnostics, UI, and logs.
- [x] Documented Python/Rust differences and accepted the save transaction ADR.

**Verification**:
- [x] Rust full workspace format/check/test: passed; save-related joint tests 194/194 and `adm-new-save` 35/35.
- [x] Python save baseline: 41/41.
- [x] Web unit/e2e and all content/language/UI gates: passed; 2426 symmetric keys, 48 bilingual screenshots, 93 baselines.
- [x] Portable resources: 156/156 exact path/length/SHA-256 matches.
- [x] Real clean-exit and restart recovery smoke: passed with stable autosave hash and no residual journal/corrupt file.

**Current Handoff State**:
- [x] Trial launcher: `NEWrust/dist/AutoDesignMaker-NEWrust/Start-AutoDesignMaker.cmd`.
- [x] EXE SHA-256: `1720b9eff3e8cbc81cce5a23e11181fad029c6edd441c9dc974cc08c43efbd75` (20,584,960 bytes).
- [x] Trial window is running as PID `35524`.
- [ ] Power-loss/media-damage backup rotation and a dedicated recovery UI remain non-blocking future hardening.
- [ ] Unity execution, live-provider output quality, and per-step artifact quality remain user acceptance checks by agreement.

---
**Date**: 2026-07-10
**ID**: 2026-07-10-002
**Summary**: Added replaceable `zh-CN` pure Chinese and `en-US` pure English application-language modes to NEWrust/Tauri, including a stable-ID overlay for the full built-in design catalog, without a visible selector, and rebuilt the verified portable trial.

**Completed**:
- [x] Added Rust `UiLanguage` startup capture through `ADM_NEWRUST_LANGUAGE` with a `zh-CN` default.
- [x] Added the central Web localization service and 2335 symmetric Chinese/English keys: 680 application UI keys plus 1655 generated design-content values.
- [x] Localized all application-owned shell, workflow, settings, status, validation, and accessibility text.
- [x] Localized the built-in 16-domain/103-node/515-checklist catalog, groups/options, gameplay systems, fallback taxonomy, profile system fields, L4 missing paths, and built-in quality violations by stable ID.
- [x] Kept source JSON, protocol IDs/values, saves, and user/log/existing-AI/artifact content verbatim through explicit content-origin boundaries.
- [x] Made Step00-14 display titles and AI generation language follow the selected mode.
- [x] Added static catalog and two-language browser purity gates without adding a visible selector.
- [x] Rebuilt and verified the portable release.

**Verification**:
- [x] Rust format, full workspace check, and full workspace tests: passed.
- [x] Web unit, end-to-end, 48 bilingual screenshot/responsive-overflow, and 93 baseline checks: passed.
- [x] `design-content-check`: passed 1655 localized values.
- [x] `i18n-test`: passed 2335 matching keys.
- [x] `language-gate`: passed 2 languages across 10 surfaces each.
- [x] Portable data verification: 156 files with zero path/length/SHA-256 mismatches.
- [x] Real Chinese and English Tauri startup/autosave/clean-exit smokes: passed.

**Current Handoff State**:
- [x] Trial launcher: `NEWrust/dist/AutoDesignMaker-NEWrust/Start-AutoDesignMaker.cmd`.
- [x] EXE SHA-256: `9a8e7f649deb3b49a7b9379721d699c22553c35f74f57a50feebeb8bd36ebee2` (19,594,752 bytes).
- [x] Language selection: `ADM_NEWRUST_LANGUAGE=zh-CN|en-US`; default `zh-CN`.
- [x] No visible language selector exists yet, by request.
- [x] The local usable product boundary is complete and ready for user trial.
- [ ] Unity execution, live-provider output quality, and per-step artifact quality remain user acceptance checks by agreement.

---
**Date**: 2026-07-07
**ID**: 2026-07-07-011
**Summary**: Continued the Rust full-project rebuild by adding a release-callable UI audit mode, generating repeatable staged UI audit and six-view PNG screenshot evidence, restaging the double-click exe, and verifying the desktop flow.

**Completed**:
- [x] Re-read current project memory and `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`.
- [x] Confirmed Step00-05 remained structurally healthy and Step00-14 content parity was already completed in session `2026-07-07-009`.
- [x] Confirmed Slint long-text and long-list polish was already completed in session `2026-07-07-010`.
- [x] Added `--ui-audit` to `RUST/apps/adm-desktop/src/main.rs`.
- [x] Added `png` as a direct desktop dependency so `--ui-audit` can write PNG evidence from Slint snapshots.
- [x] `--ui-audit` verifies active-view switching for `design`, `pipeline`, `patch`, `package`, `logs`, and `sdk`.
- [x] `--ui-audit` injects long probe text and verifies key report fields round-trip the long text.
- [x] `--ui-audit` checks key long-text bindings are inside `ScrollView` and use `wrap: word-wrap`.
- [x] `--ui-audit` checks stage/package/SDK long-row lists are inside `ScrollView`.
- [x] `--ui-audit` sets the Slint window size to `1280x860`, calls `Window::take_snapshot()`, and writes screenshots for `design`, `pipeline`, `patch`, `package`, `logs`, and `sdk`.
- [x] Each screenshot is checked for dimensions, expected byte count, non-zero bytes, and sampled color variation.
- [x] Generated staged release evidence at `RUST/dist/AutoDesignMaker-rust/ui-visual-audit.adm` and `RUST/dist/AutoDesignMaker-rust/ui-visual-audit-*.png`.
- [x] Updated `RUST/项目最新进度.html`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`.
- [x] Rebuilt and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-desktop`: passed, 2 UI layout tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -p adm-desktop -- --ui-audit .\dist\AutoDesignMaker-rust\ui-visual-audit.adm`: passed, generated 6 screenshots.
- [x] `cargo run -p adm-desktop -- --smoke`: passed with `mode=rust_devflow_executor_v1`, `completed=15`, `artifacts=26; files=34`, and `support_files=13`.
- [x] `cargo test --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] Staged exe `--ui-audit` wrote `RUST/dist/AutoDesignMaker-rust/ui-visual-audit.adm` with `status=passed` and `screenshot_artifact_count=6`.
- [x] Staged exe smoke through `cmd` stdout/stderr redirection: exit code 0, stdout populated, stderr empty.
- [x] New staged exe timestamp: `2026/7/7 23:51:18`.
- [x] New staged exe hash: `fnv64:29c5af171923f62d`.
- [x] New staged exe size: `24717312` bytes.

**Current Handoff State**:
- [x] Step00-14 remain content-level structured Rust stage documents.
- [x] Desktop stage detail still reads real Step artifact content.
- [x] Step N-M range runs remain linked to strict run log events and pipeline service latest-run summary.
- [x] Slint long-text and long-list surfaces have static regression coverage.
- [x] The staged double-click exe has repeatable local UI audit evidence at `RUST/dist/AutoDesignMaker-rust/ui-visual-audit.adm`.
- [x] The staged double-click exe has six local PNG screenshot artifacts at `RUST/dist/AutoDesignMaker-rust/ui-visual-audit-*.png`.
- [x] The staged double-click exe has been updated to `fnv64:29c5af171923f62d`.
- [ ] Full-project completion is still not achieved because external acceptance remains: real AI provider validation and real Unity PlayMode verification.
- [ ] Optional visible-window manual click review can still be performed by the user in a real GUI session.
- [ ] Real AI provider acceptance still depends on user-provided credentials/runtime configuration.
- [ ] Real Unity PlayMode acceptance is explicitly skipped per user instruction and must be manually checked by the user.
- [x] No external Unity project files were modified.

---

**Date**: 2026-07-09
**ID**: 2026-07-09-001
**Summary**: Closed out the Rust rebuild handoff step by rechecking prior Step00-14/UI-audit work, rebuilding and restaging the desktop release, refreshing local release acceptance, UI audit screenshots, source/handoff bundles, evidence, and final handoff package.

**Completed**:
- [x] Rechecked previous Step00-14 content parity and current UI-audit step before moving forward.
- [x] Rebuilt `adm-desktop` release and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Refreshed `release-acceptance.adm`, `external-acceptance.adm`, and `handoff-status.adm` to the current release hash.
- [x] Re-ran staged exe `--ui-audit`, regenerating `ui-visual-audit.adm` and six PNG screenshots.
- [x] Regenerated source bundle, handoff bundle, handoff instructions, handoff evidence, and final handoff package.
- [x] Fixed the local closeout order by running `sync-handoff-evidence` before finalizing the handoff package, restoring `package_ready=true`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`, `RUST/项目最新进度.html`, and this memory record.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-application`: passed, 40 tests.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed, 2 UI layout tests.
- [x] `cargo test --workspace`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- release-acceptance`: passed with staged smoke exit code 0.
- [x] Staged exe `--ui-audit`: passed with `status=passed` and `screenshot_artifact_count=6`.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- handoff-status`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- write-handoff-instructions`: passed.
- [x] `cargo run -q -p adm-cli -- sync-handoff-evidence`: passed.
- [x] `cargo run -q -p adm-cli -- finalize-handoff-package`: passed with final package ready.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:6e4290bdd417082b`, `release_bytes=24715776`, timestamp `2026/7/9 22:46:11`.
- [x] UI audit evidence is ready: `status=passed`, `screenshot_artifact_count=6`.
- [x] Source bundle is ready: `source_file_count=68`, `source_bundle_hash=fnv64:c4de2af7f5bd6c7f`.
- [x] Handoff bundle is ready: `handoff_bundle_file_count=131`, `handoff_bundle_hash=fnv64:84e04f59168a7e4c`.
- [x] Final handoff package is assembled: `ready=true`, `package_ready=true`, `file_count=142`, `package_hash=fnv64:3307a1bdf45ec0b3`.
- [ ] Full delivery is still not achieved because `delivery_ready=false` until external acceptance passes.
- [ ] Real AI provider acceptance still needs real provider credentials/config, currently `OPENAI_API_KEY` for `openai_main`.
- [ ] Unity PlayMode acceptance still needs a ready Unity Editor path and `unity_playmode` runtime evidence.
- [x] No external Unity project files were modified.

---
**Date**: 2026-07-07
**ID**: 2026-07-07-010
**Summary**: Continued the Rust full-project rebuild by polishing Slint long-text and long-list UI surfaces, adding UI layout regression tests, restaging the double-click exe, and verifying the desktop flow.

**Completed**:
- [x] Re-read current project rules, memory, and `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`.
- [x] Confirmed Step00-14 content parity and stage-detail artifact reading were already completed in session `2026-07-07-009`.
- [x] Updated `RUST/apps/adm-desktop/ui/main.slint` so design summaries, AI interview output, pipeline detail/log text, supplement analysis, package validation, run logs, SDK summaries, and SDK approval/resource lists are scrollable where needed.
- [x] Added `wrap: word-wrap` to key long-text report surfaces.
- [x] Added `ui_layout_tests` in `RUST/apps/adm-desktop/src/main.rs` to assert long-text surfaces and long row lists are backed by `ScrollView`.
- [x] Updated `RUST/项目最新进度.html`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`.
- [x] Rebuilt and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed, 2 UI layout tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed with `mode=rust_devflow_executor_v1`, `completed=15`, and `artifacts=26; files=34`.
- [x] `cargo test --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] Staged exe smoke through `cmd` stdout/stderr redirection: exit code 0, stdout populated, stderr empty.
- [x] New staged exe timestamp: `2026/7/7 23:25:47`.
- [x] New staged exe hash: `fnv64:42848c26c3f3093e`.
- [x] New staged exe size: `24535552` bytes.

**Current Handoff State**:
- [x] Step00-14 remain content-level structured Rust stage documents.
- [x] Desktop stage detail still reads real Step artifact content.
- [x] Step N-M range runs remain linked to strict run log events and pipeline service latest-run summary.
- [x] Slint long-text and long-list surfaces now have scroll/wrap regression coverage.
- [x] The staged double-click exe is `fnv64:42848c26c3f3093e`.
- [ ] Full-project completion is still not achieved because external/visual acceptance remains: real window screenshot review, real AI provider validation, and real Unity PlayMode verification.
- [ ] Real AI provider acceptance still depends on user-provided credentials/runtime configuration.
- [ ] Real Unity PlayMode acceptance is explicitly skipped per user instruction and must be manually checked by the user.
- [x] No external Unity project files were modified.

---

**Date**: 2026-07-09
**ID**: 2026-07-09-001
**Summary**: Closed out the Rust rebuild handoff step by rechecking prior Step00-14/UI-audit work, rebuilding and restaging the desktop release, refreshing local release acceptance, UI audit screenshots, source/handoff bundles, evidence, and final handoff package.

**Completed**:
- [x] Rechecked previous Step00-14 content parity and current UI-audit step before moving forward.
- [x] Rebuilt `adm-desktop` release and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Refreshed `release-acceptance.adm`, `external-acceptance.adm`, and `handoff-status.adm` to the current release hash.
- [x] Re-ran staged exe `--ui-audit`, regenerating `ui-visual-audit.adm` and six PNG screenshots.
- [x] Regenerated source bundle, handoff bundle, handoff instructions, handoff evidence, and final handoff package.
- [x] Fixed the local closeout order by running `sync-handoff-evidence` before finalizing the handoff package, restoring `package_ready=true`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`, `RUST/项目最新进度.html`, and this memory record.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-application`: passed, 40 tests.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed, 2 UI layout tests.
- [x] `cargo test --workspace`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- release-acceptance`: passed with staged smoke exit code 0.
- [x] Staged exe `--ui-audit`: passed with `status=passed` and `screenshot_artifact_count=6`.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- handoff-status`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- write-handoff-instructions`: passed.
- [x] `cargo run -q -p adm-cli -- sync-handoff-evidence`: passed.
- [x] `cargo run -q -p adm-cli -- finalize-handoff-package`: passed with final package ready.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:6e4290bdd417082b`, `release_bytes=24715776`, timestamp `2026/7/9 22:46:11`.
- [x] UI audit evidence is ready: `status=passed`, `screenshot_artifact_count=6`.
- [x] Source bundle is ready: `source_file_count=68`, `source_bundle_hash=fnv64:c4de2af7f5bd6c7f`.
- [x] Handoff bundle is ready: `handoff_bundle_file_count=131`, `handoff_bundle_hash=fnv64:84e04f59168a7e4c`.
- [x] Final handoff package is assembled: `ready=true`, `package_ready=true`, `file_count=142`, `package_hash=fnv64:3307a1bdf45ec0b3`.
- [ ] Full delivery is still not achieved because `delivery_ready=false` until external acceptance passes.
- [ ] Real AI provider acceptance still needs real provider credentials/config, currently `OPENAI_API_KEY` for `openai_main`.
- [ ] Unity PlayMode acceptance still needs a ready Unity Editor path and `unity_playmode` runtime evidence.
- [x] No external Unity project files were modified.

---
**Date**: 2026-07-07
**ID**: 2026-07-07-009
**Summary**: Implemented the Rust pipeline content-parity rounds: Step00-14 structured stage content, stage-detail artifact reading, Step N-M run-log association, release staging, progress page update, and verification.

**Completed**:
- [x] Checked Step00-05 current Rust state and confirmed the existing structural implementation was healthy with passing `adm-application` tests.
- [x] Added stable `## Structured Stage Content`, `## Acceptance Checklist`, and `## Downstream Inputs` sections to Step documents.
- [x] Added Step00-06 structured content parity for idea intake, gameplay framework, design freeze, program requirements, art requirements, program review, and art review.
- [x] Added Step07-14 structured content parity for style confirmation, program plan, art plan, SDK/resource alignment, program execution, art production, scene assembly, and integration validation.
- [x] Updated `RUST/apps/adm-desktop/src/main.rs` so stage detail reads selected `pipeline/stepXX/stage.adm` content and displays `contract_kind`, structured content, checklist, and downstream input summaries.
- [x] Added desktop smoke assertions for Step03, Step04, and Step14 structured stage detail reading.
- [x] Added `PipelineService` last range run summary persistence and strict run log events for `pipeline_range_started`, `pipeline_range_projected`, and `pipeline_range_completed`.
- [x] Updated `RUST/项目最新进度.html`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md` with the execution result, new staged exe hash, and remaining external/UI acceptance work.
- [x] Rebuilt and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-application`: passed, 40 tests.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed with `mode=rust_devflow_executor_v1`, `completed=15`, `artifacts=26; files=34`.
- [x] `cargo test --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe --smoke`: passed.
- [x] New staged exe timestamp: `2026/7/7 23:09:15`.
- [x] New staged exe hash: `fnv64:2744f3483f88f0fd`.
- [x] New staged exe size: `23521792` bytes.
- [x] After cleanup: `RUST/target=false`, `%TEMP%/adm_desktop_smoke_*=0`, and `RUST/dist/AutoDesignMaker-rust/.adm_rust_data=false`.

**Current Handoff State**:
- [x] Step00-14 are now content-level structured Rust stage documents, not only contract summaries.
- [x] Desktop stage detail reads real Step artifact content when `pipeline/stepXX/stage.adm` exists.
- [x] Step N-M range runs are linked to strict run log events and pipeline service latest-run summary.
- [x] The staged double-click exe is `fnv64:2744f3483f88f0fd`.
- [ ] Full-project completion is still not achieved because external acceptance remains: UI screenshot-level polish, real AI provider validation, and real Unity PlayMode verification.
- [ ] UI screenshot-level polish and long-text interaction verification remain.
- [ ] Real AI provider acceptance still depends on user-provided credentials/runtime configuration.
- [ ] Real Unity PlayMode acceptance is explicitly skipped per user instruction and must be manually checked by the user.
- [x] No external Unity project files were modified.

---

**Date**: 2026-07-09
**ID**: 2026-07-09-001
**Summary**: Closed out the Rust rebuild handoff step by rechecking prior Step00-14/UI-audit work, rebuilding and restaging the desktop release, refreshing local release acceptance, UI audit screenshots, source/handoff bundles, evidence, and final handoff package.

**Completed**:
- [x] Rechecked previous Step00-14 content parity and current UI-audit step before moving forward.
- [x] Rebuilt `adm-desktop` release and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Refreshed `release-acceptance.adm`, `external-acceptance.adm`, and `handoff-status.adm` to the current release hash.
- [x] Re-ran staged exe `--ui-audit`, regenerating `ui-visual-audit.adm` and six PNG screenshots.
- [x] Regenerated source bundle, handoff bundle, handoff instructions, handoff evidence, and final handoff package.
- [x] Fixed the local closeout order by running `sync-handoff-evidence` before finalizing the handoff package, restoring `package_ready=true`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`, `RUST/项目最新进度.html`, and this memory record.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-application`: passed, 40 tests.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed, 2 UI layout tests.
- [x] `cargo test --workspace`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- release-acceptance`: passed with staged smoke exit code 0.
- [x] Staged exe `--ui-audit`: passed with `status=passed` and `screenshot_artifact_count=6`.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- handoff-status`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- write-handoff-instructions`: passed.
- [x] `cargo run -q -p adm-cli -- sync-handoff-evidence`: passed.
- [x] `cargo run -q -p adm-cli -- finalize-handoff-package`: passed with final package ready.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:6e4290bdd417082b`, `release_bytes=24715776`, timestamp `2026/7/9 22:46:11`.
- [x] UI audit evidence is ready: `status=passed`, `screenshot_artifact_count=6`.
- [x] Source bundle is ready: `source_file_count=68`, `source_bundle_hash=fnv64:c4de2af7f5bd6c7f`.
- [x] Handoff bundle is ready: `handoff_bundle_file_count=131`, `handoff_bundle_hash=fnv64:84e04f59168a7e4c`.
- [x] Final handoff package is assembled: `ready=true`, `package_ready=true`, `file_count=142`, `package_hash=fnv64:3307a1bdf45ec0b3`.
- [ ] Full delivery is still not achieved because `delivery_ready=false` until external acceptance passes.
- [ ] Real AI provider acceptance still needs real provider credentials/config, currently `OPENAI_API_KEY` for `openai_main`.
- [ ] Unity PlayMode acceptance still needs a ready Unity Editor path and `unity_playmode` runtime evidence.
- [x] No external Unity project files were modified.

---
**Date**: 2026-07-07
**ID**: 2026-07-07-007
**Summary**: Fixed the old Python pipeline Step11 blocker caused by Stage08 emitting an empty `PG-000-parent-reuse` parallel group when no parent tasks were reusable.

**Completed**:
- [x] Diagnosed the Step11 failure in `drafts/20260707_075553_20448` as `PARALLEL_GROUP_INVALID`.
- [x] Wrote `bug/2026-07-07_Step11空父轮复用并行组阻断修复计划.md`.
- [x] Updated `core/engines/generation.py::_merge_parent_program_tasks()` so `PG-000-parent-reuse` is emitted only when `reused_ids` is non-empty.
- [x] Added cleanup for stale parent-reuse group entries in `parallel_groups` and `execution_topology.group_order`.
- [x] Added Stage08 topology self-checking through `_validate_actual_development_plan(plan)`.
- [x] Added regression tests for no-parent merges and Step11 empty parallel group rejection.

**Verification**:
- [x] `python -m pytest core\tests\unit\test_iteration_development.py core\tests\unit\test_stage11_parent_reuse_parallel.py -q`: 16 passed.
- [x] `python -m pytest core\tests\unit\test_step10_to_step12_structured_contract_chain.py -q`: 3 passed.
- [x] `python -m pytest core\tests\unit\test_step05_to_step09_structured_contract_chain.py -q`: 2 passed.
- [x] Combined targeted suite: 21 passed.
- [x] `python -m compileall core\engines\generation.py core\tests\unit\test_iteration_development.py core\tests\unit\test_stage11_parent_reuse_parallel.py`: passed.
- [x] Direct merge reproduction confirmed no empty `PG-000-parent-reuse` is emitted for an empty parent plan.
- [ ] `python -m black --check ...` timed out in this workspace even for small targeted files.

**Current Handoff State**:
- [x] New Stage08 plans without reusable parent tasks should no longer create an empty parent-reuse group.
- [x] Existing stale Stage08 artifacts must be regenerated before rerunning Step11; rerunning Step11 directly against the old draft will still read the old invalid JSON.
- [x] Step11 still rejects empty parallel groups, preserving the safety gate.
- [x] No external Unity project files were modified.

---

**Date**: 2026-07-09
**ID**: 2026-07-09-001
**Summary**: Closed out the Rust rebuild handoff step by rechecking prior Step00-14/UI-audit work, rebuilding and restaging the desktop release, refreshing local release acceptance, UI audit screenshots, source/handoff bundles, evidence, and final handoff package.

**Completed**:
- [x] Rechecked previous Step00-14 content parity and current UI-audit step before moving forward.
- [x] Rebuilt `adm-desktop` release and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Refreshed `release-acceptance.adm`, `external-acceptance.adm`, and `handoff-status.adm` to the current release hash.
- [x] Re-ran staged exe `--ui-audit`, regenerating `ui-visual-audit.adm` and six PNG screenshots.
- [x] Regenerated source bundle, handoff bundle, handoff instructions, handoff evidence, and final handoff package.
- [x] Fixed the local closeout order by running `sync-handoff-evidence` before finalizing the handoff package, restoring `package_ready=true`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`, `RUST/项目最新进度.html`, and this memory record.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-application`: passed, 40 tests.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed, 2 UI layout tests.
- [x] `cargo test --workspace`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- release-acceptance`: passed with staged smoke exit code 0.
- [x] Staged exe `--ui-audit`: passed with `status=passed` and `screenshot_artifact_count=6`.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- handoff-status`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- write-handoff-instructions`: passed.
- [x] `cargo run -q -p adm-cli -- sync-handoff-evidence`: passed.
- [x] `cargo run -q -p adm-cli -- finalize-handoff-package`: passed with final package ready.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:6e4290bdd417082b`, `release_bytes=24715776`, timestamp `2026/7/9 22:46:11`.
- [x] UI audit evidence is ready: `status=passed`, `screenshot_artifact_count=6`.
- [x] Source bundle is ready: `source_file_count=68`, `source_bundle_hash=fnv64:c4de2af7f5bd6c7f`.
- [x] Handoff bundle is ready: `handoff_bundle_file_count=131`, `handoff_bundle_hash=fnv64:84e04f59168a7e4c`.
- [x] Final handoff package is assembled: `ready=true`, `package_ready=true`, `file_count=142`, `package_hash=fnv64:3307a1bdf45ec0b3`.
- [ ] Full delivery is still not achieved because `delivery_ready=false` until external acceptance passes.
- [ ] Real AI provider acceptance still needs real provider credentials/config, currently `OPENAI_API_KEY` for `openai_main`.
- [ ] Unity PlayMode acceptance still needs a ready Unity Editor path and `unity_playmode` runtime evidence.
- [x] No external Unity project files were modified.

---
**Date**: 2026-07-07
**ID**: 2026-07-07-006
**Summary**: Created the next-round Rust full-development plan, focused on moving the development pipeline from structure/status parity toward content-level acceptance.

**Completed**:
- [x] Re-read the active project rules, memory index, and `plan/RUSTUI/README.md`.
- [x] Reviewed the latest Rust staged baseline from session `2026-07-07-005`.
- [x] Created `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`.
- [x] Planned the next implementation around Step00-06 structured stage content parity, stage-detail artifact reading, Step N-M run-log association, release staging, and cleanup.
- [x] Preserved the constraints: Slint remains, no real Unity PlayMode automation, no external Unity project modifications, no Python runtime dependency, no draggable splitters.

**Verification**:
- [x] Read back `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`.
- [x] Confirmed it records current baseline, target files, implementation order, validation commands, and cleanup checks.
- [x] No Rust source code or dist executable was rebuilt in this planning-only turn.

**Current Handoff State**:
- [x] Next implementation target is documented: Step00-06 structured content parity and pipeline stage detail content reading.
- [x] Current staged executable remains `fnv64:74cf721b2e653cb2`.
- [ ] Next coding turn should start in `RUST/crates/adm-application/src/core_pipeline.rs` by adding `## Structured Stage Content`, `## Acceptance Checklist`, and `## Downstream Inputs` for Step00-06.
- [ ] Then update `RUST/apps/adm-desktop/src/main.rs` so stage detail reads selected `pipeline/stepXX/stage.adm` content.
- [x] No external Unity project files were modified.

---

**Date**: 2026-07-09
**ID**: 2026-07-09-001
**Summary**: Closed out the Rust rebuild handoff step by rechecking prior Step00-14/UI-audit work, rebuilding and restaging the desktop release, refreshing local release acceptance, UI audit screenshots, source/handoff bundles, evidence, and final handoff package.

**Completed**:
- [x] Rechecked previous Step00-14 content parity and current UI-audit step before moving forward.
- [x] Rebuilt `adm-desktop` release and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Refreshed `release-acceptance.adm`, `external-acceptance.adm`, and `handoff-status.adm` to the current release hash.
- [x] Re-ran staged exe `--ui-audit`, regenerating `ui-visual-audit.adm` and six PNG screenshots.
- [x] Regenerated source bundle, handoff bundle, handoff instructions, handoff evidence, and final handoff package.
- [x] Fixed the local closeout order by running `sync-handoff-evidence` before finalizing the handoff package, restoring `package_ready=true`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`, `RUST/项目最新进度.html`, and this memory record.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-application`: passed, 40 tests.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed, 2 UI layout tests.
- [x] `cargo test --workspace`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- release-acceptance`: passed with staged smoke exit code 0.
- [x] Staged exe `--ui-audit`: passed with `status=passed` and `screenshot_artifact_count=6`.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- handoff-status`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- write-handoff-instructions`: passed.
- [x] `cargo run -q -p adm-cli -- sync-handoff-evidence`: passed.
- [x] `cargo run -q -p adm-cli -- finalize-handoff-package`: passed with final package ready.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:6e4290bdd417082b`, `release_bytes=24715776`, timestamp `2026/7/9 22:46:11`.
- [x] UI audit evidence is ready: `status=passed`, `screenshot_artifact_count=6`.
- [x] Source bundle is ready: `source_file_count=68`, `source_bundle_hash=fnv64:c4de2af7f5bd6c7f`.
- [x] Handoff bundle is ready: `handoff_bundle_file_count=131`, `handoff_bundle_hash=fnv64:84e04f59168a7e4c`.
- [x] Final handoff package is assembled: `ready=true`, `package_ready=true`, `file_count=142`, `package_hash=fnv64:3307a1bdf45ec0b3`.
- [ ] Full delivery is still not achieved because `delivery_ready=false` until external acceptance passes.
- [ ] Real AI provider acceptance still needs real provider credentials/config, currently `OPENAI_API_KEY` for `openai_main`.
- [ ] Unity PlayMode acceptance still needs a ready Unity Editor path and `unity_playmode` runtime evidence.
- [x] No external Unity project files were modified.

---
**Date**: 2026-07-07
**ID**: 2026-07-07-005
**Summary**: Continued the full-project Rust rebuild by adding strict run logs, SDK approval persistence, pipeline range/stop/style service records, materialized Step00-14 archive artifacts, Rust Step00-14 devflow execution, per-step Rust native contract output, workbench-to-pipeline brief export, devflow-first package inspection, Step N-M range-run projection, and a freshly staged double-click exe.

**Completed**:
- [x] Re-read the active project rules, memory index, and current Rust handoff state.
- [x] Added `adm-application::RunLogService` with JSONL append/filter/render/clear/export support.
- [x] Added `adm-application::SdkKnowledgeService` with pending/approved/rejected review records and approved prompt context generation.
- [x] Added `adm-application::PipelineService` with range run request, stop request, and Step07 style confirmation persistence.
- [x] Wired Slint desktop pages for run-log operations, SDK review queue operations, pipeline range/stop/style controls, and service status display.
- [x] Added materialized Step00-14 documents at `pipeline/stepXX/stage.adm` and registered them as archive artifacts.
- [x] Added `pipeline/devflow_run_report.adm` and `pipeline/devflow_run_state.adm`, then updated desktop stage progress/runtime summary to prefer devflow state/report whenever devflow files exist and only fall back for old archives without devflow files.
- [x] Upgraded Step00-14 from one-shot document projection to `rust_devflow_executor_v1`, a Rust `PipelineRunner` with sequential Step00-14 stages.
- [x] Added `## Rust Native Contract Output` sections to each Step00-14 document with step-specific `contract_kind` values and Rust-derived contract summaries.
- [x] Added `WorkbenchService::pipeline_brief()` and wired desktop “导出到流水线” plus no-archive range run to use current workbench state before falling back to top-level fields.
- [x] Added devflow run report/state to package manifest support files, raising package support files from 11 to 13.
- [x] Added desktop range-run projection for “from Step N to M”: the app reruns from the mapped start core stage and rewrites archive devflow state/report to the selected Step range.
- [x] Increased archive registry records from 11 to 26 and updated export/smoke expectations to 34 files.
- [x] Rebuilt and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Updated `RUST/项目最新进度.html`.
- [x] Ran `cargo clean` and removed smoke temp directories.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-application`: passed, 40 tests.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed, reporting `mode=rust_devflow_executor_v1`, `completed=15`, `artifacts=26; files=34`, and `support_files=13`.
- [x] `cargo test --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe --smoke`: passed.
- [x] Staged exe timestamp: `2026/7/7 20:39:53`.
- [x] Staged exe hash: `fnv64:74cf721b2e653cb2`.
- [x] Staged exe size: `23448064` bytes.
- [x] After cleanup: `RUST/target=false`, `%TEMP%/adm_desktop_smoke_*=0`, and `RUST/dist/AutoDesignMaker-rust/.adm_rust_data=false`.

**Current Handoff State**:
- [x] The staged double-click exe includes strict logs, SDK approval queue, pipeline service records, materialized Step00-14 archive artifacts, Rust devflow execution, per-step Rust native contract output, package-exported devflow state/report files, workbench-to-pipeline brief export, and Step N-M range-run projection.
- [ ] Full-project completion is still not achieved: Step00-14 need deeper per-step Rust contract content parity with the old plugin outputs beyond the current Rust devflow executor, contract summaries, and core-artifact-backed document generation.
- [ ] UI screenshot-level polish and long-text interaction verification remain.
- [ ] Real AI provider acceptance still depends on user-provided credentials/runtime configuration.
- [ ] Real Unity PlayMode acceptance is explicitly skipped per user instruction and must be manually checked by the user.
- [x] No external Unity project files were modified.

---

**Date**: 2026-07-09
**ID**: 2026-07-09-001
**Summary**: Closed out the Rust rebuild handoff step by rechecking prior Step00-14/UI-audit work, rebuilding and restaging the desktop release, refreshing local release acceptance, UI audit screenshots, source/handoff bundles, evidence, and final handoff package.

**Completed**:
- [x] Rechecked previous Step00-14 content parity and current UI-audit step before moving forward.
- [x] Rebuilt `adm-desktop` release and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Refreshed `release-acceptance.adm`, `external-acceptance.adm`, and `handoff-status.adm` to the current release hash.
- [x] Re-ran staged exe `--ui-audit`, regenerating `ui-visual-audit.adm` and six PNG screenshots.
- [x] Regenerated source bundle, handoff bundle, handoff instructions, handoff evidence, and final handoff package.
- [x] Fixed the local closeout order by running `sync-handoff-evidence` before finalizing the handoff package, restoring `package_ready=true`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`, `RUST/项目最新进度.html`, and this memory record.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-application`: passed, 40 tests.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed, 2 UI layout tests.
- [x] `cargo test --workspace`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- release-acceptance`: passed with staged smoke exit code 0.
- [x] Staged exe `--ui-audit`: passed with `status=passed` and `screenshot_artifact_count=6`.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- handoff-status`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- write-handoff-instructions`: passed.
- [x] `cargo run -q -p adm-cli -- sync-handoff-evidence`: passed.
- [x] `cargo run -q -p adm-cli -- finalize-handoff-package`: passed with final package ready.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:6e4290bdd417082b`, `release_bytes=24715776`, timestamp `2026/7/9 22:46:11`.
- [x] UI audit evidence is ready: `status=passed`, `screenshot_artifact_count=6`.
- [x] Source bundle is ready: `source_file_count=68`, `source_bundle_hash=fnv64:c4de2af7f5bd6c7f`.
- [x] Handoff bundle is ready: `handoff_bundle_file_count=131`, `handoff_bundle_hash=fnv64:84e04f59168a7e4c`.
- [x] Final handoff package is assembled: `ready=true`, `package_ready=true`, `file_count=142`, `package_hash=fnv64:3307a1bdf45ec0b3`.
- [ ] Full delivery is still not achieved because `delivery_ready=false` until external acceptance passes.
- [ ] Real AI provider acceptance still needs real provider credentials/config, currently `OPENAI_API_KEY` for `openai_main`.
- [ ] Unity PlayMode acceptance still needs a ready Unity Editor path and `unity_playmode` runtime evidence.
- [x] No external Unity project files were modified.

---
**Date**: 2026-07-07
**ID**: 2026-07-07-004
**Summary**: Continued the Rust UI/data rebuild by finishing workbench export/template/archive operations, wiring AI interview provider write-back, adding supplemental development analysis, and upgrading pipeline presentation to Step00-14 mapped onto the current Rust core stages.

**Completed**:
- [x] Re-read the active project rules, memory index, and `plan/RUSTUI/README.md`.
- [x] Added `WorkbenchService` export support for markdown/json/text/prompt and wired desktop export.
- [x] Added project template list/import/save support and desktop template controls.
- [x] Added AI interview question generation, user reply recording, provider invocation, validation, current-node design note write-back, and replay/provenance records.
- [x] Added formal archive save/load for `design/workbench_state.json` and `design/project.adm`.
- [x] Added Step00-14 metadata and desktop stage mapping to the current Rust `design/development/assets/sdk/packaging` core stages.
- [x] Added StepXX rerun mapping to the executable Rust core stage.
- [x] Added supplemental development request analysis and archive commit to `patch/supplement_requests.adm`.
- [x] Rebuilt and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Updated `RUST/项目最新进度.html`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-application`: passed, 36 tests.
- [x] `cargo test -p adm-desktop`: passed, 0 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe --smoke`: passed.
- [x] New staged exe timestamp: `2026/7/7 1:09:46`.
- [x] New staged exe hash: `fnv64:f73114d5ddd642f1`.

**Current Handoff State**:
- [x] The staged double-click entry now includes workbench export/templates/archive, AI interview write-back, supplemental development analysis, and Step00-14 mapped pipeline presentation.
- [ ] Step00-14 true Rust execution parity remains: each old step is displayed and mapped, but not yet implemented as an independent Rust executor.
- [ ] Run log strict operations and SDK approval queue persistence remain.
- [ ] No real Unity PlayMode acceptance was run, and no external Unity project files were modified.

---

**Date**: 2026-07-09
**ID**: 2026-07-09-001
**Summary**: Closed out the Rust rebuild handoff step by rechecking prior Step00-14/UI-audit work, rebuilding and restaging the desktop release, refreshing local release acceptance, UI audit screenshots, source/handoff bundles, evidence, and final handoff package.

**Completed**:
- [x] Rechecked previous Step00-14 content parity and current UI-audit step before moving forward.
- [x] Rebuilt `adm-desktop` release and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Refreshed `release-acceptance.adm`, `external-acceptance.adm`, and `handoff-status.adm` to the current release hash.
- [x] Re-ran staged exe `--ui-audit`, regenerating `ui-visual-audit.adm` and six PNG screenshots.
- [x] Regenerated source bundle, handoff bundle, handoff instructions, handoff evidence, and final handoff package.
- [x] Fixed the local closeout order by running `sync-handoff-evidence` before finalizing the handoff package, restoring `package_ready=true`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`, `RUST/项目最新进度.html`, and this memory record.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-application`: passed, 40 tests.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed, 2 UI layout tests.
- [x] `cargo test --workspace`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- release-acceptance`: passed with staged smoke exit code 0.
- [x] Staged exe `--ui-audit`: passed with `status=passed` and `screenshot_artifact_count=6`.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- handoff-status`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- write-handoff-instructions`: passed.
- [x] `cargo run -q -p adm-cli -- sync-handoff-evidence`: passed.
- [x] `cargo run -q -p adm-cli -- finalize-handoff-package`: passed with final package ready.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:6e4290bdd417082b`, `release_bytes=24715776`, timestamp `2026/7/9 22:46:11`.
- [x] UI audit evidence is ready: `status=passed`, `screenshot_artifact_count=6`.
- [x] Source bundle is ready: `source_file_count=68`, `source_bundle_hash=fnv64:c4de2af7f5bd6c7f`.
- [x] Handoff bundle is ready: `handoff_bundle_file_count=131`, `handoff_bundle_hash=fnv64:84e04f59168a7e4c`.
- [x] Final handoff package is assembled: `ready=true`, `package_ready=true`, `file_count=142`, `package_hash=fnv64:3307a1bdf45ec0b3`.
- [ ] Full delivery is still not achieved because `delivery_ready=false` until external acceptance passes.
- [ ] Real AI provider acceptance still needs real provider credentials/config, currently `OPENAI_API_KEY` for `openai_main`.
- [ ] Unity PlayMode acceptance still needs a ready Unity Editor path and `unity_playmode` runtime evidence.
- [x] No external Unity project files were modified.

---
**Date**: 2026-07-07
**ID**: 2026-07-07-003
**Summary**: Continued the `plan/RUSTUI` rebuild by wiring node-level design workbench editing and autosave restoration into Rust service snapshots and Slint callbacks.

**Completed**:
- [x] Re-read the active project rules, memory index, and `plan/RUSTUI/README.md`.
- [x] Extended `adm-application::WorkbenchService` with active node detail, checklist rows, and L4 option rows.
- [x] Added Rust service commands and Slint callbacks for node selection, project name apply, checklist toggle, L4 option selection, primary option selection, node text save, L5 JSON save, and L5 JSON clear.
- [x] Reworked the design workbench center/left panels so selected node details, checklist, L4 options, notes, and L5 JSON are visible and write back to `WorkbenchState`.
- [x] Added workbench autosave/load support at `data_root/design_workbench/workbench_state.json`.
- [x] Wired desktop design workbench mutations and reset to autosave after successful service commands.
- [x] Rebuilt and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Updated `RUST/项目最新进度.html`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo check -p adm-application`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-design`: passed, 6 tests.
- [x] `cargo test -p adm-application`: passed, 27 tests.
- [x] `cargo test -p adm-desktop`: passed, 0 tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe --smoke`: passed.
- [x] New staged exe timestamp: `2026/7/7 0:38:45`.
- [x] New staged exe hash: `fnv64:f575ea5d02124f82`.
- [x] `cargo clean`: removed build artifacts; `RUST/target=false` after cleanup.

**Current Handoff State**:
- [x] Rust design workbench now has the first editable node-level UI/data loop: node selection, checklist, L4 option selection, primary option, L5 JSON, node notes, and autosave restoration.
- [ ] Remaining major work: template operations, formal save/archive management, AI interview provider write-back, Step00-14 true Rust pipeline parity, and visual/interaction polish.
- [ ] No real Unity PlayMode acceptance was run, and no external Unity project files were modified.

---

**Date**: 2026-07-09
**ID**: 2026-07-09-001
**Summary**: Closed out the Rust rebuild handoff step by rechecking prior Step00-14/UI-audit work, rebuilding and restaging the desktop release, refreshing local release acceptance, UI audit screenshots, source/handoff bundles, evidence, and final handoff package.

**Completed**:
- [x] Rechecked previous Step00-14 content parity and current UI-audit step before moving forward.
- [x] Rebuilt `adm-desktop` release and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Refreshed `release-acceptance.adm`, `external-acceptance.adm`, and `handoff-status.adm` to the current release hash.
- [x] Re-ran staged exe `--ui-audit`, regenerating `ui-visual-audit.adm` and six PNG screenshots.
- [x] Regenerated source bundle, handoff bundle, handoff instructions, handoff evidence, and final handoff package.
- [x] Fixed the local closeout order by running `sync-handoff-evidence` before finalizing the handoff package, restoring `package_ready=true`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`, `RUST/项目最新进度.html`, and this memory record.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-application`: passed, 40 tests.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed, 2 UI layout tests.
- [x] `cargo test --workspace`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- release-acceptance`: passed with staged smoke exit code 0.
- [x] Staged exe `--ui-audit`: passed with `status=passed` and `screenshot_artifact_count=6`.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- handoff-status`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- write-handoff-instructions`: passed.
- [x] `cargo run -q -p adm-cli -- sync-handoff-evidence`: passed.
- [x] `cargo run -q -p adm-cli -- finalize-handoff-package`: passed with final package ready.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:6e4290bdd417082b`, `release_bytes=24715776`, timestamp `2026/7/9 22:46:11`.
- [x] UI audit evidence is ready: `status=passed`, `screenshot_artifact_count=6`.
- [x] Source bundle is ready: `source_file_count=68`, `source_bundle_hash=fnv64:c4de2af7f5bd6c7f`.
- [x] Handoff bundle is ready: `handoff_bundle_file_count=131`, `handoff_bundle_hash=fnv64:84e04f59168a7e4c`.
- [x] Final handoff package is assembled: `ready=true`, `package_ready=true`, `file_count=142`, `package_hash=fnv64:3307a1bdf45ec0b3`.
- [ ] Full delivery is still not achieved because `delivery_ready=false` until external acceptance passes.
- [ ] Real AI provider acceptance still needs real provider credentials/config, currently `OPENAI_API_KEY` for `openai_main`.
- [ ] Unity PlayMode acceptance still needs a ready Unity Editor path and `unity_playmode` runtime evidence.
- [x] No external Unity project files were modified.

---
**Date**: 2026-07-07
**ID**: 2026-07-07-002
**Summary**: Rebuilt and restaged the current Rust desktop exe after the user reported that the double-click entry still looked like the previous UI.

**Completed**:
- [x] Re-read the active project rules and memory index.
- [x] Confirmed the staged Rust double-click entry is `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Clarified that `cargo run -p adm-desktop` is the source development entry and the `dist` exe is the staged double-click entry.
- [x] Rebuilt `adm-desktop` in release mode and restaged it into `RUST/dist/AutoDesignMaker-rust`.

**Verification**:
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] New staged exe timestamp: `2026/7/7 0:16:39`.
- [x] New staged release hash: `fnv64:56d5d7a5da1259e6`.
- [x] No running `AutoDesignMaker-rust` or `adm-desktop` process was found after staging.

**Current Handoff State**:
- [x] The latest staged double-click app is `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] There is no separate hidden completed UI executable.
- [ ] If the visible UI still does not match the requested final interaction model, the remaining Rust UI/data rebuild work is still unfinished rather than a launch-path issue.

---

**Date**: 2026-07-09
**ID**: 2026-07-09-001
**Summary**: Closed out the Rust rebuild handoff step by rechecking prior Step00-14/UI-audit work, rebuilding and restaging the desktop release, refreshing local release acceptance, UI audit screenshots, source/handoff bundles, evidence, and final handoff package.

**Completed**:
- [x] Rechecked previous Step00-14 content parity and current UI-audit step before moving forward.
- [x] Rebuilt `adm-desktop` release and restaged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Refreshed `release-acceptance.adm`, `external-acceptance.adm`, and `handoff-status.adm` to the current release hash.
- [x] Re-ran staged exe `--ui-audit`, regenerating `ui-visual-audit.adm` and six PNG screenshots.
- [x] Regenerated source bundle, handoff bundle, handoff instructions, handoff evidence, and final handoff package.
- [x] Fixed the local closeout order by running `sync-handoff-evidence` before finalizing the handoff package, restoring `package_ready=true`.
- [x] Updated `plan/RUSTUI/NEXT_ROUND_2026-07-07.md`, `RUST/项目最新进度.html`, and this memory record.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-application`: passed, 40 tests.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed, 2 UI layout tests.
- [x] `cargo test --workspace`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- release-acceptance`: passed with staged smoke exit code 0.
- [x] Staged exe `--ui-audit`: passed with `status=passed` and `screenshot_artifact_count=6`.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- handoff-status`: completed with expected external blockers.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- write-handoff-instructions`: passed.
- [x] `cargo run -q -p adm-cli -- sync-handoff-evidence`: passed.
- [x] `cargo run -q -p adm-cli -- finalize-handoff-package`: passed with final package ready.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:6e4290bdd417082b`, `release_bytes=24715776`, timestamp `2026/7/9 22:46:11`.
- [x] UI audit evidence is ready: `status=passed`, `screenshot_artifact_count=6`.
- [x] Source bundle is ready: `source_file_count=68`, `source_bundle_hash=fnv64:c4de2af7f5bd6c7f`.
- [x] Handoff bundle is ready: `handoff_bundle_file_count=131`, `handoff_bundle_hash=fnv64:84e04f59168a7e4c`.
- [x] Final handoff package is assembled: `ready=true`, `package_ready=true`, `file_count=142`, `package_hash=fnv64:3307a1bdf45ec0b3`.
- [ ] Full delivery is still not achieved because `delivery_ready=false` until external acceptance passes.
- [ ] Real AI provider acceptance still needs real provider credentials/config, currently `OPENAI_API_KEY` for `openai_main`.
- [ ] Unity PlayMode acceptance still needs a ready Unity Editor path and `unity_playmode` runtime evidence.
- [x] No external Unity project files were modified.

---
**Date**: 2026-07-07
**ID**: 2026-07-07-001
**Summary**: Created a double-clickable Rust progress document under `RUST/` for opening the latest project progress directly.

**Completed**:
- [x] Re-read the active project rules and memory index.
- [x] Created `RUST/项目最新进度.html` as a Chinese HTML progress page that opens by double-click in the default browser.
- [x] Included the current Rust rebuild status from session `2026-07-06-058`.
- [x] Added links to the AI memory index, latest session record, Rust UI rebuild plan, and staged Rust exe.

**Verification**:
- [x] Confirmed `RUST/项目最新进度.html` exists.
- [x] Read the generated file back with UTF-8 encoding and confirmed the expected progress content is present.

**Current Handoff State**:
- [x] Requested double-click progress document is available at `RUST/项目最新进度.html`.
- [x] No Rust source code was modified in this session.
- [x] No external Unity files were touched.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-058
**Summary**: Started implementing the `plan/RUSTUI` rebuild. Added the Rust design workbench data kernel, application service layer, and desktop snapshot/field interaction wiring.

**Completed**:
- [x] Added `adm-design` workbench data repository, serializable state, and design engine.
- [x] Implemented Rust-side checklist, L4 selection, L4 primary, node text, L5 JSON validation, coverage, missing/risk/validation/result-tab derivation.
- [x] Added `adm-application::WorkbenchService` as the UI-safe command/snapshot layer.
- [x] Updated `adm-desktop` design refresh to use `WorkbenchService` instead of the previous read-only `WorkbenchReference`.
- [x] Added Slint/Rust callback wiring for design domain selection and workbench reset.
- [x] Rebuilt and copied the release exe to `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Cleaned Cargo build outputs and smoke temp directories after verification.

**Verification**:
- [x] `cargo test -p adm-design`: passed, 6 tests.
- [x] `cargo test -p adm-application`: passed, 25 tests.
- [x] `cargo check -p adm-application`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `RUST/target=false` after cleanup.

**Current Handoff State**:
- [x] Rust now has a real workbench state/data/service foundation instead of only a static UI/reference view.
- [x] Domain selection is a real UI-to-service interaction and refreshes page data.
- [ ] Per-node checklist controls, full L4 option UI, L5 JSON editor UI, template operations, save/autosave, and AI interview provider write-back remain.
- [ ] Step00-14 true Rust pipeline parity remains.
- [ ] No real Unity PlayMode test was run, and no external Unity project was modified.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-057
**Summary**: Re-audited the old Python UI/data interaction contract and current Rust gaps, then wrote a new Rust UI/data rebuild plan under `plan/RUSTUI`.

**Completed**:
- [x] Re-read the active project rules and memory index.
- [x] Re-audited old Python UI/data contracts for the six top-level pages, design workbench, Step00-14 pipeline, patch/package/log/SDK panels, save/config/runtime boundaries, and design engine.
- [x] Re-audited current Rust boundaries in `adm-desktop`, `adm-design`, `adm-ui-model`, `adm-application`, and `adm-pipeline`.
- [x] Confirmed the current Rust UI is still a shell/reference view without full `WorkbenchState`, L4/L5 persistence, AI interview state, template operations, or true Step00-14 parity.
- [x] Created `plan/RUSTUI/README.md` with the full rebuild plan and acceptance criteria.

**Verification**:
- [x] Confirmed `plan/RUSTUI/README.md` exists and contains data contracts, service commands, Slint rebuild phases, tests, risks, and work estimate.
- [x] Confirmed `plan/` is ignored by `.gitignore`, so the requested file exists on disk but does not appear in normal `git status`.

**Current Handoff State**:
- [x] No Rust code was modified in this session.
- [x] No external Unity files were touched.
- [ ] Next implementation should start with `adm-design` data contracts and tests before more Slint layout work.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-056
**Summary**: Reworked the Rust Slint desktop UI away from the rejected single-page form into the six-task-space workbench shell, recorded the Slint/no-draggable-splitter decision, and added a Rust-native design knowledge reference loader.

**Completed**:
- [x] Re-read the active project rules, memory index, and current context.
- [x] Recorded that Rust continues to use Slint and old Python draggable pane boundaries are not required.
- [x] Added/updated glossary and ADR entries for Rust workbench interaction parity.
- [x] Replaced `RUST/apps/adm-desktop/ui/main.slint` with a six-window workbench shell: design workbench, development pipeline, supplemental development, packaging, run log, and SDK knowledge base.
- [x] Added Rust-native loading of `knowledge/design_data` into design workbench domain, node, L4 option group, L5 candidate, and right-tab summaries.
- [x] Preserved existing desktop callbacks/properties so current AI, packaging, archive, and smoke paths still compile.
- [x] Cleaned Cargo build/test artifacts with `cargo clean`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo check -p adm-design`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed, 0 tests in this crate.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `RUST/target=false` after cleanup.

**Current Handoff State**:
- [x] Rust UI no longer uses the rejected single-page long form structure.
- [x] Design knowledge structure is now read by Rust, not Python.
- [ ] Full design workbench editing state, L4/L5 persistence, AI interview interaction, and template operations still need implementation.
- [ ] Development pipeline UI now exposes Step00-14, but Rust runner still needs true Step00-14 execution parity.
- [ ] No real Unity run was performed and no external Unity project was modified.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-055
**Summary**: Localized the Rust desktop GUI to Chinese, rebuilt the staged Windows release exe, verified it renders correctly without an extra cmd window, and cleaned temporary test/build data.

**Completed**:
- [x] Re-read the active project rules and current memory before continuing.
- [x] Translated the Rust desktop Slint UI chrome to Chinese-facing labels.
- [x] Added runtime display localization for status/report strings while preserving `--smoke` English machine-readable assertions.
- [x] Localized safe table/status row fields for stages, artifacts, package files, build records, delivery checks, SDK resources, validation issues, and acceptance traces.
- [x] Preserved technical values as-is: provider ids, model names, environment variables, URLs, paths, target ids, command lines, Unity brand names, and AutoDesignMaker product identifiers.
- [x] Rebuilt and staged `RUST/dist/AutoDesignMaker-rust/AutoDesignMaker-rust.exe`.
- [x] Launched the staged exe for visual verification, confirmed a non-white Chinese UI, closed it, and deleted the temporary screenshot.
- [x] Confirmed the release exe PE subsystem is `Windows GUI` (`Subsystem=2`), so it should not spawn its own cmd console.
- [x] Removed `RUST/.adm_rust_data`, the temporary launch screenshot, and `RUST/target` after verification.
- [x] Did not launch a real Unity Editor and did not modify any external Unity project.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-desktop`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed, including release acceptance and staged exe `--smoke`.
- [x] GUI launch check showed title `自动设计生成器（桌面版）` and rendered Chinese UI.
- [x] Cleanup check: `RUST/target=false`, `RUST/.adm_rust_data=false`, `launch-check-zh.png=false`, staged release exe present.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:fd67eff6865beb38`.
- [x] Source bundle is ready: `source_bundle_hash=fnv64:48cc0840896fbd33`.
- [x] Handoff bundle is ready: `handoff_bundle_hash=fnv64:7d03a60eebcbec36`.
- [x] Handoff evidence is ready: `evidence_hash=fnv64:8dcc156f9c77926f`.
- [x] Final package is assembled: `package_ready=true`, `package_hash=fnv64:c262e365488f3f15`.
- [ ] Final delivery is still not externally accepted: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] Real AI provider acceptance still requires a configured secret such as `OPENAI_API_KEY`.
- [ ] Real Unity PlayMode acceptance remains intentionally outside Codex validation per user instruction.

**Resume Steps**:
- [ ] User manually validates real Unity PlayMode if strict final delivery should become green.
- [ ] Provide a real AI provider secret and rerun AI acceptance with `-RequireReady`; add `-Invoke -RequireInvoke` if real network-call evidence is required.
- [ ] Re-run strict final acceptance after external AI and Unity evidence are available.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-054
**Summary**: Closed out the Rust automated verification side, attempted the real AI provider acceptance path, refreshed handoff artifacts, and cleaned test/build artifacts.

**Completed**:
- [x] Re-read the active project rules, memory index, and current Rust handoff state.
- [x] Confirmed the abandoned manual Unity acceptance implementation direction was not retained; no `manual_unity` / `ManualUnityAcceptance` code paths remain.
- [x] Ran Rust formatter, workspace check, and workspace tests.
- [x] Configured `openai_main` through the OpenAI preset path and regenerated the AI acceptance report.
- [x] Refreshed release, source bundle, handoff bundle, handoff status, handoff instructions, handoff evidence, and final handoff package.
- [x] Cleaned Cargo build/test outputs with `cargo clean`, removing `RUST/target`.
- [x] Removed stale Rust temp test directories under `%TEMP%` after verifying they stayed under the temp root.
- [x] Confirmed no real Unity Editor was launched and no external Unity project was modified.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot .adm_rust_data`: completed and wrote a redacted not-ready report.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed.
- [x] `RUST/target` no longer exists after cleanup.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source bundle is ready: `source_bundle_hash=fnv64:34d5340e07053b34`.
- [x] Handoff bundle is ready: `handoff_bundle_hash=fnv64:05c6989778afd31c`.
- [x] Handoff evidence is ready: `evidence_hash=fnv64:71c239d8e8122d1a`.
- [x] Final package is assembled: `package_ready=true`, `package_hash=fnv64:330fd67d1b721dc4`.
- [ ] Final delivery is not accepted: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] Real AI provider acceptance remains blocked because `OPENAI_API_KEY` is absent: `diagnostic_readiness=MissingSecret`, `network_call=false`, and `invoke_attempted=false`.
- [ ] Unity strict acceptance remains blocked in generated evidence: `unity_ready=false` and `unity_runtime_runner=cli_smoke_runner`; user owns real Unity PlayMode validation.

**Resume Steps**:
- [ ] Set `OPENAI_API_KEY` or configure an equivalent real provider secret.
- [ ] Re-run AI acceptance with `-RequireReady`; add `-Invoke -RequireInvoke` if a real network call must be proven.
- [ ] User performs real Unity PlayMode validation separately and updates Unity acceptance evidence if strict final delivery should become fully green.
- [ ] Re-run strict final acceptance after both external inputs are available.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-053
**Summary**: Completed a blocked-state audit for the Rust handoff: no new local command/path portability defect was found, bundle-root operator preflight was verified, and the remaining delivery blockers are confirmed to require external AI credentials and a compatible Unity Editor.

**Completed**:
- [x] Re-read the active project rules, memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the package remained assembled but externally unaccepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Scanned receiver-facing handoff bundle commands for original workspace absolute paths in `suggested_*`, `command=`, `-DataRoot`, `-File`, and `-InstructionsPath` fields.
- [x] Confirmed suggested receiver commands in `HANDOFF_README.txt` and `evidence/handoff-instructions.adm` use portable placeholders such as `<data_root>` and `<path-to-Unity.exe>`.
- [x] Verified the bundle-root operator preflight command from the handoff bundle can read `evidence/handoff-instructions.adm`.
- [x] Checked the current shell for external inputs without exposing secrets: `OPENAI_API_KEY` is not present.
- [x] Checked default Unity locations and environment variables: no compatible `Unity.exe` path is available.
- [x] Ran final handoff acceptance dry-run and confirmed the operator preflight reports exactly the missing required inputs: `ai_secret` and `unity_exe`.
- [x] Re-ran the release gate after the dry-run to keep handoff bundle, evidence, and final manifest synchronized.
- [x] Confirmed no real Unity Editor was launched and no external Unity installation/project outside the workspace was modified.

**Verification**:
- [x] `rg` scan found no receiver-facing suggested command carrying the original workspace absolute path.
- [x] Bundle-root operator preflight from `RUST/dist/handoff-bundle`: passed and loaded bundle evidence; it reported missing AI secret, Unity executable, and real data root as expected for placeholder mode.
- [x] `[bool]$env:OPENAI_API_KEY`: returned `False`.
- [x] Default Unity checks for `C:\Program Files\Unity\Editor\Unity.exe` and `C:\Program Files\Unity\Hub\Editor`: both absent.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun`: passed; operator preflight reported `missing_input=ai_secret` and `missing_input=unity_exe`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] No external Unity PlayMode validation was executed; current Unity evidence remains local CLI smoke evidence, not `runner=unity_playmode`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:34d5340e07053b34`.
- [x] Unified handoff bundle is ready: `handoff_bundle_file_count=111`, `handoff_bundle_hash=fnv64:1647f473a88182c0`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:a6a8de10d8ab8622`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:57136579ce47d1e2`.
- [x] Final package contains refreshed `AutoDesignMaker-rust/final-acceptance-run.adm`: bytes `5454`, hash `fnv64:f4bfe3e8d152aea5`.
- [x] Final package contains refreshed `HANDOFF_README.txt`: bytes `18698`, hash `fnv64:59788f199b76b830`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: bytes `17866`, hash `fnv64:41148a206f2fd977`.
- [ ] Final delivery is not accepted: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no compatible Unity Editor is available and no real Unity PlayMode run has produced `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because `OPENAI_API_KEY` is absent and no equivalent real provider secret is configured.

**Blocked Audit**:
- [x] The same external blockers have repeated across at least sessions 051, 052, and 053: missing real AI provider acceptance and missing Unity PlayMode acceptance.
- [x] Current local evidence proves the package is assembled and locally smoke-ready, but not deliverable.
- [x] No further meaningful local code or packaging fix was identified after auditing receiver-facing commands and running the portable bundle-root preflight.
- [ ] Full completion requires an external-state change: provide a real AI provider secret and compatible Unity Editor path, then rerun final handoff acceptance.

**Resume Steps**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path or set `ADM_UNITY_EDITOR` / `UNITY_EDITOR_PATH`.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real Unity and real AI provider inputs; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-052
**Summary**: Added first-class rehydrated workspace smoke rerun fields to handoff instructions and `HANDOFF_README.txt`, so receivers can see the exact workspace-root command that session 051 made executable after restoring the bundle.

**Completed**:
- [x] Re-read the active project rules, memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the package remained assembled but externally unaccepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Audited the refreshed handoff bundle and found the rehydrate script already rewrote the restored top-level `release-acceptance.adm`, but the operator-facing README/instructions did not list the rehydrated workspace-root smoke command as first-class evidence.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so handoff instructions emit `rehydrated_release_smoke_report`, `rehydrated_release_smoke_command`, and `rehydrated_release_smoke_working_dir`.
- [x] Updated handoff README rendering to inherit and publish the same three fields.
- [x] Extended `adm-cli` tests to cover both `handoff-instructions.adm` and `HANDOFF_README.txt` output.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final handoff package.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-readme-release-smoke-command-052`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] Confirmed `RUST/dist/AutoDesignMaker-rust/handoff-instructions.adm`, `RUST/dist/handoff-bundle/evidence/handoff-instructions.adm`, and `RUST/dist/handoff-bundle/HANDOFF_README.txt` all contain the new `rehydrated_release_smoke_*` fields.
- [x] Rehydrated from `RUST/dist/handoff-bundle` using `.\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-readme-release-smoke-command-052`; passed.
- [x] Confirmed the rehydration manifest reports `release_smoke_command=.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke` and `release_smoke_command_working_dir=rehydrated-rust-workspace-root`.
- [x] Ran `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke` from the refreshed rehydrated workspace root; passed.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:34d5340e07053b34`.
- [x] Unified handoff bundle is ready: `handoff_bundle_file_count=111`, `handoff_bundle_hash=fnv64:983f943137562cad`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:6cff9e969728cdb0`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:9dadf4491099c51f`.
- [x] Final package contains refreshed `HANDOFF_README.txt`: bytes `18698`, hash `fnv64:6feb4591e193b6a1`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: bytes `17866`, hash `fnv64:41148a206f2fd977`.
- [x] Final package contains refreshed `source-bundle/apps/adm-cli/src/main.rs`: bytes `319753`, hash `fnv64:3d2cabd96f5beb91`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no real Unity PlayMode run has produced `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because no real provider secret has been supplied in the normal shell.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path or set `ADM_UNITY_EDITOR` / `UNITY_EDITOR_PATH`.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real Unity and real AI provider inputs; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-051
**Summary**: Fixed rehydrated handoff workspace smoke rerun evidence by rewriting the restored top-level `dist/AutoDesignMaker-rust/release-acceptance.adm` to use a workspace-root-relative executable path, while preserving the nested handoff bundle report as bundle-root-relative evidence.

**Completed**:
- [x] Re-read the active project rules, memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the package was still assembled but externally unaccepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified a receiver-facing evidence mismatch left after session 050: the nested handoff bundle release report used the correct bundle-root smoke command, but the rehydrated workspace top-level release report also retained that bundle-root command even though its natural working directory is the rehydrated workspace root.
- [x] Updated `RUST/scripts/rehydrate_handoff_workspace.ps1` with report-line rewrite helpers and a rehydrated release acceptance update step after artifacts/source are copied.
- [x] Rewrote restored top-level release report fields to `smoke_executable=.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe` and `smoke_command=.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`.
- [x] Added rehydration manifest fields for the restored release smoke report: `release_acceptance_report_rewrite=workspace-root-relative`, `release_smoke_command`, `release_smoke_command_working_dir`, and `release_smoke_report`.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final handoff package with the refreshed rehydration script.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-rehydrated-release-smoke-command-bundle-051`.

**Verification**:
- [x] Direct source-script rehydrate passed: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\rehydrate_handoff_workspace.ps1 -BundleRoot .\dist\handoff-bundle -DestinationPath .\target\handoff-rehydrated-release-smoke-command-smoke-051`.
- [x] Confirmed the directly rehydrated top-level report uses `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`.
- [x] Confirmed the directly rehydrated nested handoff bundle report still uses `.\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`.
- [x] Ran smoke from the directly rehydrated workspace root: `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`; passed.
- [x] Ran smoke from the directly rehydrated nested handoff bundle root: `.\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`; passed.
- [x] Release gate passed: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] Confirmed packaged source bundle scripts contain `Update-RehydratedReleaseAcceptance` and the new release smoke manifest fields.
- [x] Rehydrated with the refreshed packaged source-bundle script from `RUST/dist/handoff-bundle`: `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-rehydrated-release-smoke-command-bundle-051`; passed.
- [x] Confirmed the refreshed packaged rehydrate result writes top-level workspace-root smoke fields and preserves nested bundle-root smoke fields.
- [x] Ran smoke from the refreshed packaged rehydrated workspace root: `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`; passed.
- [x] Ran smoke from the refreshed packaged rehydrated nested handoff bundle root: `.\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`; passed.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:4ef8958530a09371`.
- [x] Unified handoff bundle is ready: `handoff_bundle_file_count=111`, `handoff_bundle_hash=fnv64:ac52d2d12a917af4`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:a8ace1283a8f9d54`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:f0c912a83e47b219`.
- [x] Final package contains refreshed `source-bundle/scripts/rehydrate_handoff_workspace.ps1`: bytes `8958`, hash `fnv64:af522a5479874b76`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no real Unity PlayMode run has produced `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is available for local acceptance and no real provider secret has been supplied in the normal shell.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path or set `ADM_UNITY_EDITOR` / `UNITY_EDITOR_PATH`.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real Unity and real AI provider inputs; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Stop broad repair after this round per user request; next turn should report the current state first before doing any further fixing.
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-050
**Summary**: Made bundled `release-acceptance.adm` smoke rerun evidence portable by rewriting copied `smoke_executable` and `smoke_command` fields to a handoff-bundle-root relative executable path, while preserving the source release acceptance report as original audit evidence.

**Completed**:
- [x] Re-read the active project rules, memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the final package was still assembled but externally unaccepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Audited receiver-facing command fields in the handoff bundle and confirmed session 049 had removed original workspace script paths from planned final acceptance commands.
- [x] Identified that copied `release-acceptance.adm` files still exposed the original workspace `smoke_executable` and `smoke_command`.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so handoff bundle staging rewrites the release directory copy of `release-acceptance.adm`.
- [x] Updated handoff evidence sync so `evidence/release-acceptance.adm` receives the same portable smoke command transform.
- [x] Added `handoff_bundle_smoke_command_mode=portable-package-root-relative` and `handoff_bundle_smoke_command_working_dir=handoff-bundle-root` evidence fields.
- [x] Extended `adm-cli` tests to cover both bundled release acceptance locations.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final handoff package.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-release-smoke-command-smoke-050`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] Confirmed both `RUST/dist/handoff-bundle/AutoDesignMaker-rust/release-acceptance.adm` and `RUST/dist/handoff-bundle/evidence/release-acceptance.adm` use `.\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`.
- [x] Confirmed those bundled release acceptance copies no longer contain original workspace `smoke_command=E:\workwork\CrewAi\AutoDesignMaker\RUST...` or `smoke_executable=E:\workwork\CrewAi\AutoDesignMaker\RUST...`.
- [x] Ran `.\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke` from `RUST/dist/handoff-bundle`: passed.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-release-smoke-command-smoke-050`: passed from `RUST/dist/handoff-bundle`.
- [x] Confirmed the rehydrated handoff bundle still contains portable smoke command fields.
- [x] Ran `.\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke` from the rehydrated `dist/handoff-bundle`: passed.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:0a8bbd6c0d0aee0e`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:bd71cea6d99877f0`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:e374090d59ef1627`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:f8b54e8034fb0c41`.
- [x] Final package contains portable `AutoDesignMaker-rust/release-acceptance.adm`: bytes `7230`, hash `fnv64:c362171a46f5a5b3`.
- [x] Final package contains portable `AutoDesignMaker-rust/final-acceptance-run.adm`: bytes `5454`, hash `fnv64:6627799b943f5614`.
- [x] Final package contains updated `HANDOFF_README.txt`: bytes `18455`, hash `fnv64:1a37f7279d43a71c`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no real Unity PlayMode run has produced `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no real provider secret is present in the normal shell.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path or set `ADM_UNITY_EDITOR` / `UNITY_EDITOR_PATH`.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real Unity and real AI provider inputs; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-049
**Summary**: Made the handoff bundle copy of `AutoDesignMaker-rust/final-acceptance-run.adm` fully portable for planned command rows by rewriting bundled `-File` script paths and the operator preflight `-InstructionsPath` to rehydrated-workspace relative paths.

**Completed**:
- [x] Re-read the active project rules, memory, and Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the final package was still assembled but externally unaccepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified that session 046 made `-DataRoot` portable in bundled final acceptance commands, but bundled command rows still carried original workspace `RUST\scripts\*.ps1` paths and operator preflight still pointed `-InstructionsPath` at the original workspace.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `portable_final_acceptance_run_for_bundle` rewrites bundled command script paths to `.\scripts\*.ps1`.
- [x] Updated the same bundle-only transform to rewrite `-InstructionsPath` to `.\dist\handoff-bundle\evidence\handoff-instructions.adm`.
- [x] Added bundle evidence fields for command script path and instruction path portability.
- [x] Extended the handoff bundle unit test to use absolute source paths and assert the bundled command rows are portable while audit fields remain unchanged.
- [x] Refreshed the default final acceptance dry-run report, handoff bundle, handoff evidence, and final handoff package.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-final-report-script-path-smoke-049`.
- [x] Stopped further repair work after this current round per user instruction.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun`: passed and refreshed `dist/AutoDesignMaker-rust/final-acceptance-run.adm`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] Confirmed bundled `AutoDesignMaker-rust/final-acceptance-run.adm` command rows use `.\scripts\*.ps1`, `-DataRoot '<data_root>'`, and `.\dist\handoff-bundle\evidence\handoff-instructions.adm`.
- [x] Confirmed bundled command rows no longer contain original workspace `E:\workwork\CrewAi\AutoDesignMaker\RUST\scripts` paths or original workspace `-InstructionsPath`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-final-report-script-path-smoke-049`: passed from `RUST/dist/handoff-bundle`.
- [x] Confirmed the rehydrated `dist/AutoDesignMaker-rust/final-acceptance-run.adm` retains portable command rows and bundle evidence fields.
- [x] Rehydrated final handoff dry-run from `RUST/target/handoff-final-report-script-path-smoke-049`: passed and resolved scripts from the rehydrated workspace root; missing inputs remained the expected external values: AI secret, Unity executable, and real data root.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:441dbb21e94d5abd`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:b5d09448fba69ffc`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:e07815fec382e727`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:2de2e5090d00a3dd`.
- [x] Final package contains portable `AutoDesignMaker-rust/final-acceptance-run.adm`: bytes `5454`, hash `fnv64:6627799b943f5614`.
- [x] Final package contains updated `HANDOFF_README.txt`: bytes `18455`, hash `fnv64:b683c25dce2b3187`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no real Unity PlayMode run has produced `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no real provider secret is present in the normal shell.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path or set `ADM_UNITY_EDITOR` / `UNITY_EDITOR_PATH`.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real Unity and real AI provider inputs; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-048
**Summary**: Refreshed the default `final-acceptance-run.adm` dry-run report so the packaged handoff evidence now matches the current final acceptance wrapper behavior, including `unity_exe_source` and `UNITY_EDITOR_PATH` guidance.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the final package was still assembled but externally unaccepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified that the source-bundle scripts and README had been updated in session 047, but the default `dist/AutoDesignMaker-rust/final-acceptance-run.adm` dry-run report still reflected the older wrapper behavior.
- [x] Ran the default `scripts/final_handoff_acceptance.ps1 -DryRun` to refresh `dist/AutoDesignMaker-rust/final-acceptance-run.adm`.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-final-report-refresh-smoke-048`.

**Verification**:
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun`: passed and refreshed `dist/AutoDesignMaker-rust/final-acceptance-run.adm`.
- [x] The refreshed source report contains `unity_exe_source=placeholder`, `operator_preflight_unity_exe_source=explicit`, and `operator_preflight_missing_input=unity_exe` guidance mentioning `ADM_UNITY_EDITOR/UNITY_EDITOR_PATH`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] Confirmed bundled `AutoDesignMaker-rust/final-acceptance-run.adm` contains `unity_exe_source` and `UNITY_EDITOR_PATH` guidance.
- [x] Confirmed bundled `AutoDesignMaker-rust/final-acceptance-run.adm` contains no original workspace `-DataRoot E:\workwork\CrewAi\AutoDesignMaker\RUST\.adm_rust_data` command arguments.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-final-report-refresh-smoke-048`: passed from `RUST/dist/handoff-bundle`.
- [x] Confirmed the rehydrated `dist/AutoDesignMaker-rust/final-acceptance-run.adm` contains `unity_exe_source` and `UNITY_EDITOR_PATH` guidance.
- [x] Confirmed the rehydrated `final-acceptance-run.adm` contains no original workspace `-DataRoot E:\workwork\CrewAi\AutoDesignMaker\RUST\.adm_rust_data` command arguments.
- [x] Confirmed the rehydration manifest reports `ready=true` and copy-safe final acceptance / strict gate commands.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:f20694c53e976b96`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:4211b6bd82246644`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:5f76d287a3bb3059`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:5e5878a94fbfb1ae`.
- [x] Final package contains refreshed `AutoDesignMaker-rust/final-acceptance-run.adm`: bytes `5371`, hash `fnv64:d310551d37723ce8`.
- [x] Final package contains updated `HANDOFF_README.txt`: bytes `18455`, hash `fnv64:3367883bfed383e6`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no real Unity PlayMode run has produced `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no real provider secret is present in the normal shell.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path or set `ADM_UNITY_EDITOR` / `UNITY_EDITOR_PATH`.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real Unity and real AI provider inputs; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-047
**Summary**: Aligned handoff operator preflight and final handoff acceptance Unity discovery with documented behavior by adding `UNITY_EDITOR_PATH` / default-path fallback and `unity_exe_source` report evidence.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the final package was still assembled but externally unaccepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified a wrapper-level gap: `unity_acceptance_gate.ps1` and Rust `unity-doctor` support `ADM_UNITY_EDITOR`, `UNITY_EDITOR_PATH`, and default Unity discovery paths, but `handoff_operator_preflight.ps1` and `final_handoff_acceptance.ps1` only used explicit `-UnityExe` or `ADM_UNITY_EDITOR`.
- [x] Updated `RUST/scripts/handoff_operator_preflight.ps1` to resolve Unity from explicit `-UnityExe`, `ADM_UNITY_EDITOR`, `UNITY_EDITOR_PATH`, static defaults, and Unity Hub version directories.
- [x] Added `unity_exe_source` to operator preflight output.
- [x] Updated operator preflight missing-input guidance to mention both `ADM_UNITY_EDITOR` and `UNITY_EDITOR_PATH`.
- [x] Updated `RUST/scripts/final_handoff_acceptance.ps1` to use the same Unity candidate order before falling back to the dry-run placeholder.
- [x] Added `unity_exe_source` to final handoff acceptance report and console output.
- [x] Updated `RUST/README.md` to document wrapper-level Unity discovery and `unity_exe_source`.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.
- [x] Stopped further repair work after this current round per user instruction.

**Verification**:
- [x] `handoff_operator_preflight.ps1 -DataRoot .adm_rust_data -RequireReady` with temporary `UNITY_EDITOR_PATH` and `OPENAI_API_KEY`: passed with `ready=true`, `unity_exe_source=env:UNITY_EDITOR_PATH`, and `missing_input_count=0`.
- [x] `final_handoff_acceptance.ps1 -DataRoot .adm_rust_data -DryRun -ReportPath .\target\final-acceptance-unity-env-fallback-smoke-047.adm` with temporary `UNITY_EDITOR_PATH` and `OPENAI_API_KEY`: passed with `unity_exe_source=env:UNITY_EDITOR_PATH` and nested `operator_preflight_ready=true`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] Confirmed `dist/source-bundle/scripts/handoff_operator_preflight.ps1` contains `UNITY_EDITOR_PATH` and `unity_exe_source`.
- [x] Confirmed `dist/source-bundle/scripts/final_handoff_acceptance.ps1` contains `UNITY_EDITOR_PATH` and `unity_exe_source`.
- [x] Confirmed `dist/source-bundle/README.md` documents `UNITY_EDITOR_PATH` and `unity_exe_source`.
- [x] Confirmed the final package manifest includes the updated source-bundle scripts and README.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:f20694c53e976b96`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:5d3cae7466806f82`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:2a0af1523e505fe9`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:8094a2eaa37636da`.
- [x] Final package contains updated `source-bundle/README.md`: bytes `34706`, hash `fnv64:8228d0fc5be88f10`.
- [x] Final package contains updated `source-bundle/scripts/final_handoff_acceptance.ps1`: bytes `18580`, hash `fnv64:9994e9729ea3a3d9`.
- [x] Final package contains updated `source-bundle/scripts/handoff_operator_preflight.ps1`: bytes `11099`, hash `fnv64:9c44b4d18091bd43`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no real Unity PlayMode run has produced `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no real provider secret is present in the normal shell.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path or set `ADM_UNITY_EDITOR` / `UNITY_EDITOR_PATH`.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real Unity and real AI provider inputs; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-046
**Summary**: Made the handoff bundle copy of `AutoDesignMaker-rust/final-acceptance-run.adm` portable by rewriting command DataRoot arguments to `'<data_root>'` while preserving original report DataRoot audit fields.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Checked local external acceptance prerequisites: `OPENAI_API_KEY`, `ADM_UNITY_EDITOR`, and `UNITY_EDITOR_PATH` are still absent.
- [x] Ran `scripts/handoff_operator_preflight.ps1 -DataRoot .adm_rust_data`; it still reports `ready=false` with missing `ai_secret` and `unity_exe`, while DataRoot/archive are present.
- [x] Confirmed the final handoff package remains assembled but externally unaccepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Verified the bundle-root operator preflight command resolves `..\evidence\handoff-instructions.adm` correctly from `dist/handoff-bundle`.
- [x] Identified a remaining handoff portability gap: bundled README/evidence were portable, but nested `AutoDesignMaker-rust/final-acceptance-run.adm` still contained original-workspace `-DataRoot` command arguments.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `stage_handoff_bundle` rewrites only the bundled copy of `AutoDesignMaker-rust/final-acceptance-run.adm`.
- [x] Added `portable_final_acceptance_run_for_bundle()` using command-only `-DataRoot '<data_root>'` conversion.
- [x] Preserved original `data_root=` and `operator_preflight_data_root=` audit fields.
- [x] Added bundled report fields `handoff_bundle_command_data_root_mode=portable-placeholder` and `handoff_bundle_command_data_root_placeholder=<data_root>`.
- [x] Extended `stage_handoff_bundle_copies_required_and_optional_dirs_and_cleans_stale_files` to prove bundled final acceptance report command rows are portable.
- [x] Updated `RUST/README.md` to document the bundled final acceptance report DataRoot behavior.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-final-acceptance-portable-smoke-046`.

**Verification**:
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\handoff_operator_preflight.ps1 -DataRoot .adm_rust_data`: passed as diagnostic and reported missing `ai_secret` and `unity_exe`.
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] Positive grep confirmed source release `final-acceptance-run.adm` keeps original DataRoot audit and command context.
- [x] Positive grep confirmed bundled `AutoDesignMaker-rust/final-acceptance-run.adm` keeps audit fields but rewrites command `-DataRoot` values to `'<data_root>'`.
- [x] Negative grep confirmed bundled `AutoDesignMaker-rust/final-acceptance-run.adm` contains no original workspace `-DataRoot` command arguments.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-final-acceptance-portable-smoke-046`: passed from `RUST/dist/handoff-bundle`.
- [x] Positive grep confirmed rehydrated `dist/AutoDesignMaker-rust/final-acceptance-run.adm` and nested `dist/handoff-bundle/AutoDesignMaker-rust/final-acceptance-run.adm` both carry portable command DataRoot placeholders.
- [x] Negative grep confirmed rehydrated final acceptance reports and rehydration manifest contain no original workspace `-DataRoot` command arguments.
- [x] Rehydrated final dry-run with `-InstructionsPath .\dist\handoff-bundle\evidence\handoff-instructions.adm -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'`: passed and wrote `operator_preflight_missing_input_count=3`.
- [x] Negative grep confirmed the rehydrated dry-run report contains no original workspace `-DataRoot` command arguments.
- [x] Cross-layer check confirmed the same final acceptance report portability rule appears in source, `dist/source-bundle`, `RUST/README.md`, final package manifest, bundled final acceptance report, and rehydrated final acceptance report.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:68b5db090a0ff265`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:901a76418a736e8f`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:097ba1dccb820ca4`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:7515dd13e9bea544`.
- [x] Final package contains updated `source-bundle/apps/adm-cli/src/main.rs`: bytes `306775`, hash `fnv64:5f46cf9773152b98`.
- [x] Final package contains updated `source-bundle/README.md`: bytes `34480`, hash `fnv64:cf482e6aec71884b`.
- [x] Final package contains updated `AutoDesignMaker-rust/final-acceptance-run.adm`: bytes `5279`, hash `fnv64:ab747e543ad7ec1b`.
- [x] Final package contains updated `HANDOFF_README.txt`: bytes `18455`, hash `fnv64:cc1a1256ec2048eb`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path.
- [ ] If running outside the original workspace, rehydrate the package from `dist/handoff-bundle` into a clean Rust workspace.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real `-UnityExe` and `-DataRoot`; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `dist/AutoDesignMaker-rust/final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-045
**Summary**: Made the handoff bundle's machine-readable evidence instructions portable by rewriting copied command DataRoot arguments to the `'<data_root>'` placeholder while preserving original DataRoot audit fields.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the Rust handoff package was still assembled but not externally accepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified a handoff portability gap: source `dist/AutoDesignMaker-rust/handoff-instructions.adm` should keep original DataRoot evidence, but receiver-facing `dist/handoff-bundle/evidence/handoff-instructions.adm` should not instruct receivers to run commands with the original workspace DataRoot.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `sync_handoff_evidence` rewrites only the bundled copy of `handoff-instructions.adm`.
- [x] Added `portable_handoff_instructions_for_bundle()` to reuse the same `-DataRoot '<data_root>'` conversion used by the handoff README.
- [x] Added bundled evidence fields `handoff_bundle_command_data_root_mode=portable-placeholder` and `handoff_bundle_command_data_root_placeholder=<data_root>`.
- [x] Preserved original `external_acceptance_data_root` and `ai_acceptance_data_root` audit fields in the bundled instructions.
- [x] Extended the `sync_handoff_evidence_copies_current_reports_and_cleans_stale_files` test to prove the bundled evidence commands are portable while audit fields remain.
- [x] Updated `RUST/README.md` to document the source-release-vs-bundled-evidence DataRoot behavior.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-portable-evidence-smoke-045`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] Positive grep confirmed source release `handoff-instructions.adm` keeps original DataRoot audit and command context.
- [x] Positive grep confirmed bundled `evidence/handoff-instructions.adm` keeps original `external_acceptance_data_root` / `ai_acceptance_data_root` audit fields but rewrites command `-DataRoot` values to `'<data_root>'`.
- [x] Negative grep confirmed bundled `evidence/handoff-instructions.adm` contains no original workspace `-DataRoot` command arguments.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-portable-evidence-smoke-045`: passed from `RUST/dist/handoff-bundle`.
- [x] Positive grep confirmed the rehydrated bundle evidence still has portable command DataRoot placeholders.
- [x] Negative grep confirmed the rehydrated evidence and rehydration manifest contain no original workspace `-DataRoot` command arguments.
- [x] Rehydrated final dry-run with `-InstructionsPath .\dist\handoff-bundle\evidence\handoff-instructions.adm -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'`: passed and wrote `operator_preflight_missing_input_count=3`.
- [x] Negative grep confirmed the rehydrated final dry-run report contains no original workspace `-DataRoot` command arguments.
- [x] Cross-layer check confirmed the same portable DataRoot rule appears in source, `dist/source-bundle`, `RUST/README.md`, `dist/handoff-bundle/HANDOFF_README.txt`, bundled evidence, and rehydrated evidence.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:3b74d53ee69612c6`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:cea8621727a0ebce`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:1a1f47de22a6ce50`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:bff0920e3dfe2c43`.
- [x] Final package contains updated `source-bundle/apps/adm-cli/src/main.rs`: bytes `304550`, hash `fnv64:a5cfb51fce7bb977`.
- [x] Final package contains updated `source-bundle/README.md`: bytes `34187`, hash `fnv64:1ac88b19f67f3d06`.
- [x] Final package contains updated `evidence/handoff-instructions.adm`: bytes `17623`, hash `fnv64:956632b88ca594a5`.
- [x] Final package contains updated `HANDOFF_README.txt`: bytes `18455`, hash `fnv64:8c49cfe7ebe215ad`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path.
- [ ] If running outside the original workspace, rehydrate the package from `dist/handoff-bundle` into a clean Rust workspace.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real `-UnityExe` and `-DataRoot`; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `dist/AutoDesignMaker-rust/final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-044
**Summary**: Made PowerShell acceptance wrapper dry-runs preserve and quote placeholder arguments while real non-dry-run commands reject unresolved placeholder paths before any mutating gate can execute.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the Rust handoff package was still assembled but not externally accepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified that single-gate dry-runs for `ai_acceptance_gate.ps1`, `external_acceptance_doctor.ps1`, `unity_acceptance_gate.ps1`, and `release_gate.ps1` preserved execution safety but expanded `'<data_root>'` into workspace-local paths in printed command previews.
- [x] Updated the PowerShell wrappers to detect angle-bracket placeholders in path-like arguments.
- [x] Dry-run now preserves placeholders such as `'<data_root>'`, `'<path-to-Unity.exe>'`, and `'<archive_id>'` instead of expanding them into workspace paths.
- [x] Real non-dry-run now rejects unresolved placeholder paths before invoking cargo or mutating acceptance gates.
- [x] Added copy-safe PowerShell command formatting to the single-gate wrapper previews so placeholder/path tokens are quoted in printed commands.
- [x] Updated `handoff_operator_preflight.ps1` missing-input `fix=` guidance to quote placeholder command arguments.
- [x] Updated `RUST/README.md` to document placeholder preservation, placeholder rejection in real runs, and quoted dry-run command previews.
- [x] Refreshed the default `dist/AutoDesignMaker-rust/final-acceptance-run.adm`.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-placeholder-command-safety-smoke-044`.

**Verification**:
- [x] Dry-run checks passed for `ai_acceptance_gate.ps1`, `external_acceptance_doctor.ps1`, `unity_acceptance_gate.ps1`, and `release_gate.ps1` with placeholder `-DataRoot`, `-UnityExe`, and `-ArchiveId`; printed command previews now contain quoted placeholders.
- [x] Non-dry-run placeholder rejection checks passed for `ai_acceptance_gate.ps1`, `external_acceptance_doctor.ps1`, `unity_acceptance_gate.ps1`, `release_gate.ps1`, and `final_handoff_acceptance.ps1`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun`: passed and refreshed the default final acceptance report.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-placeholder-command-safety-smoke-044`: passed from `RUST/dist/handoff-bundle`.
- [x] Rehydrated final dry-run with placeholder `-UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'`: passed and wrote quoted placeholder commands plus `operator_preflight_missing_input_count=3`.
- [x] Cross-layer positive grep confirmed placeholder helpers and command formatting in source scripts, `dist/source-bundle`, and the rehydrated workspace.
- [x] Cross-layer positive grep confirmed quoted placeholder commands in `dist/handoff-bundle/HANDOFF_README.txt`, the rehydration manifest, and the rehydrated final dry-run report.
- [x] Cross-layer negative grep confirmed no unquoted `-DataRoot <data_root>` / `-UnityExe <path-to-Unity.exe>` or workspace-expanded `...\<data_root>` forms in generated handoff command reports.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:6e9fe28eca64b0d8`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:bfbbc5a2c6ac367b`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:a8233e6f617b8e2e`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:50acae25653c86b2`.
- [x] Final package contains updated wrapper scripts:
  - `source-bundle/scripts/ai_acceptance_gate.ps1`: hash `fnv64:d5d89d396a48824b`
  - `source-bundle/scripts/external_acceptance_doctor.ps1`: hash `fnv64:abd0f68127bb9634`
  - `source-bundle/scripts/final_handoff_acceptance.ps1`: hash `fnv64:cfe8a63b776e82e8`
  - `source-bundle/scripts/handoff_operator_preflight.ps1`: hash `fnv64:c19d3a8e58e8ce7b`
  - `source-bundle/scripts/release_gate.ps1`: hash `fnv64:e59c5c5d738ac792`
  - `source-bundle/scripts/unity_acceptance_gate.ps1`: hash `fnv64:9ab242ce2dcf95e5`
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path.
- [ ] If running outside the original workspace, rehydrate the package from `dist/handoff-bundle` into a clean Rust workspace.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real `-UnityExe` and `-DataRoot`; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `dist/AutoDesignMaker-rust/final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-043
**Summary**: Embedded read-only operator preflight diagnostics into the final handoff acceptance dry-run report, then regenerated and rehydration-tested the packaged handoff artifacts so receivers can see missing Unity, AI secret, DataRoot, or archive inputs in one report.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the Rust handoff package was still assembled but not externally accepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified that `scripts/final_handoff_acceptance.ps1 -DryRun` only wrote planned command rows and skipped the read-only operator preflight, forcing receivers to run a separate preflight command to discover missing inputs.
- [x] Updated `RUST/scripts/final_handoff_acceptance.ps1` so dry-run still skips mutating acceptance gates but runs `handoff_operator_preflight.ps1` without `-RequireReady`.
- [x] Added prefixed report capture so the dry-run report now includes `operator_preflight_*` rows such as readiness, script checks, missing input count, and missing input details.
- [x] Preserved real final acceptance semantics: non-dry-run still passes `-RequireReady` to operator preflight and fails before gates if required inputs are missing.
- [x] Updated `RUST/README.md` to document that final handoff dry-run embeds operator preflight diagnostics.
- [x] Refreshed the default `dist/AutoDesignMaker-rust/final-acceptance-run.adm` so the packaged default report contains the new diagnostics.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-dryrun-preflight-smoke-043`.

**Verification**:
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun -ReportPath .\target\final-acceptance-dryrun-preflight-smoke-043.adm`: passed and wrote `operator_preflight_missing_input_count=2` for local missing `ai_secret` and `unity_exe`.
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed after refreshing the default dry-run report; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-dryrun-preflight-smoke-043`: passed from `RUST/dist/handoff-bundle`.
- [x] Rehydrated dry-run with placeholder `-UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'`: passed and wrote `operator_preflight_missing_input_count=3` for `ai_secret`, `unity_exe`, and `data_root`.
- [x] Cross-layer positive grep confirmed `Add-PrefixedAdmOutput`, `RunDuringDryRun`, and `operator_preflight_*` behavior in source, `dist/source-bundle`, the rehydrated workspace, README, default packaged `final-acceptance-run.adm`, and the rehydrated dry-run report.
- [x] Cross-layer negative grep confirmed the operator preflight step is not marked `status=skipped_dry_run` in either the packaged default report or the rehydrated default report.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:25f428d24d1b1b93`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:c7f75dda35662675`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:159d49b83bbc19df`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:30fa9d3beba642d6`.
- [x] Final package contains updated `AutoDesignMaker-rust/final-acceptance-run.adm`: bytes `5425`, hash `fnv64:3a060a0c93424e04`.
- [x] Final package contains updated `source-bundle/scripts/final_handoff_acceptance.ps1`: bytes `15984`, hash `fnv64:a63b3a38f3eefe82`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path.
- [ ] If running outside the original workspace, rehydrate the package from `dist/handoff-bundle` into a clean Rust workspace.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real `-UnityExe` and `-DataRoot`; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `dist/AutoDesignMaker-rust/final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-042
**Summary**: Made the handoff bundle README portable for receivers by replacing original workspace `DataRoot` command arguments with the `'<data_root>'` placeholder while preserving the original data root as an audit field.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the Rust handoff package was still assembled but not externally accepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified that `dist/handoff-bundle/HANDOFF_README.txt` was still using the original workspace absolute `DataRoot` in suggested commands, which made the bundle less portable for receivers.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so only bundle README command text converts original data root arguments to `-DataRoot '<data_root>'`.
- [x] Preserved audit fields such as `strict_gate_original_data_root`, `external_acceptance_data_root`, and `ai_acceptance_data_root`.
- [x] Added explicit bundle README fields: `handoff_bundle_command_data_root_mode=portable-placeholder` and `handoff_bundle_command_data_root_placeholder=<data_root>`.
- [x] Updated `adm-cli` tests to prove bundle README commands are portable while the original data root evidence remains visible.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-portable-data-root-smoke-042`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-portable-data-root-smoke-042`: passed from `RUST/dist/handoff-bundle`.
- [x] Cross-layer positive grep confirmed portable data-root fields and `-DataRoot '<data_root>'` command arguments in `dist/handoff-bundle/HANDOFF_README.txt`, the rehydrated handoff README, the rehydration manifest, and `dist/source-bundle/apps/adm-cli/src/main.rs`.
- [x] Cross-layer negative grep found no old `-DataRoot E:\workwork\CrewAi\AutoDesignMaker\RUST\.adm_rust_data` or `-DataRoot .adm_rust_data` command forms in `HANDOFF_README.txt` or the rehydrated manifest.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:0baae26d45b90d4c`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:00b54ffecde0f12d`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:1bdf964ee3972835`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:67b200444ab65142`.
- [x] Final package still contains `source-bundle/scripts/final_handoff_acceptance.ps1`: hash `fnv64:3d1b4d23049ed102`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path.
- [ ] If running outside the original workspace, rehydrate the package from `dist/handoff-bundle` into a clean Rust workspace.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real `-UnityExe` and `-DataRoot`; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `dist/AutoDesignMaker-rust/final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-041
**Summary**: Made generated handoff, README, and rehydration commands copy-safe by quoting PowerShell placeholder arguments, then regenerated and rehydration-tested the packaged handoff artifacts.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before continuing the full-project completion goal.
- [x] Confirmed the Rust handoff package was assembled but still not externally accepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so generated handoff instruction fallback commands quote placeholder arguments such as `'<path-to-Unity.exe>'`, `'<data_root>'`, `'<archive_id>'`, `'<provider_id>'`, `'<model>'`, `'<preset_id>'`, and `'<path-to-rehydrated-rust-workspace>'`.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `powershell_arg` explicitly treats bracketed placeholder arguments as quoted PowerShell arguments.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` tests to assert the quoted generated command output and to normalize legacy fixture command placeholders before rendering handoff README output.
- [x] Updated `RUST/scripts/rehydrate_handoff_workspace.ps1` so generated rehydration manifest commands quote placeholder `DataRoot` and `UnityExe` arguments.
- [x] Updated `RUST/README.md` so PowerShell command examples quote placeholder arguments.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-command-placeholder-quote-smoke-041`.
- [x] Verified the generated handoff evidence, `HANDOFF_README.txt`, source bundle, and rehydrated manifest all expose copy-safe quoted placeholder commands.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-command-placeholder-quote-smoke-041`: passed from `RUST/dist/handoff-bundle`.
- [x] Cross-layer positive grep confirmed quoted commands in source, `dist/source-bundle`, `dist/handoff-bundle/evidence/handoff-instructions.adm`, `dist/handoff-bundle/HANDOFF_README.txt`, and the rehydrated manifest.
- [x] Cross-layer negative grep found no old raw `-UnityExe <path-to-Unity.exe>`, `-DataRoot <data_root>`, or `-DestinationPath <path-to-rehydrated-rust-workspace>` command forms in generated handoff evidence, `HANDOFF_README.txt`, or the rehydrated manifest.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:a88095866d572735`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:7e0d4a89ccdcf0d9`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:4c59f91d1b0ad42a`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:e4a0352605cea1eb`.
- [x] Final package still contains `source-bundle/scripts/final_handoff_acceptance.ps1`: hash `fnv64:3d1b4d23049ed102`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path.
- [ ] If running outside the original workspace, rehydrate the package from `dist/handoff-bundle` into a clean Rust workspace.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real `-UnityExe` and `-DataRoot`; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `dist/AutoDesignMaker-rust/final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-040
**Summary**: Made final handoff acceptance dry-run reports copy-safe for placeholder and space-containing Unity paths, then regenerated and rehydration-tested the packaged handoff artifacts.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before continuing the goal.
- [x] Confirmed the current Rust package was assembled but still not externally accepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Ran current operator preflight and confirmed the receiving shell still lacks only the expected external inputs: `ai_secret` and `unity_exe`.
- [x] Updated `RUST/scripts/final_handoff_acceptance.ps1` to render planned command lines with PowerShell-safe quoting for empty, placeholder, or space-containing arguments.
- [x] Updated `RUST/scripts/final_handoff_acceptance.ps1` so dry-run mode no longer requires supplied Unity paths to exist locally before generating command previews.
- [x] Updated `RUST/README.md` to document that dry-run command previews quote placeholder/space-containing arguments and do not require a supplied Unity path to exist locally.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-final-acceptance-quote-smoke-040`.
- [x] Ran final acceptance dry-run from the rehydrated package with a representative `C:\Program Files\...\Unity.exe` path and confirmed planned commands quote that path.

**Verification**:
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\handoff_operator_preflight.ps1 -DataRoot .adm_rust_data`: passed and reported the expected `missing_input_count=2`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun`: passed and wrote quoted placeholder commands in `final-acceptance-run.adm`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun -UnityExe 'C:\Program Files\Unity\Hub\Editor\2022.3.60f1\Editor\Unity.exe' -ReportPath .\target\final-acceptance-command-quote-smoke.adm`: passed even though that local Unity path does not exist.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-final-acceptance-quote-smoke-040`: passed from `RUST/dist/handoff-bundle`.
- [x] Rehydrated package dry-run with a space-containing Unity path passed and wrote `RUST/target/handoff-final-acceptance-quote-smoke-040/target/final-acceptance-command-quote-smoke.adm`.
- [x] Cross-layer text check confirmed quoting helpers and README wording are present in source and `dist/source-bundle`, the refreshed final manifest points at the new script hash, and the rehydrated dry-run report contains quoted `UnityExe 'C:\Program Files\...\Unity.exe'` commands.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:6361314b4bdc6043`.
- [x] Unified handoff bundle is ready: `handoff_bundle_file_count=111`, `handoff_bundle_hash=fnv64:aa06d280c8d6e75d`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:f7dc9f4514c8384d`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:2611e1da0b741afd`.
- [x] Final package contains `AutoDesignMaker-rust/final-acceptance-run.adm`: hash `fnv64:fd880e5c131b76c0`.
- [x] Final package contains `source-bundle/README.md`: hash `fnv64:8d4529fa442cd191`.
- [x] Final package contains `source-bundle/scripts/final_handoff_acceptance.ps1`: hash `fnv64:3d1b4d23049ed102`.
- [x] Default final acceptance run report is currently a dry-run plan: `dry_run=true`, `final_package_refresh_after_report=false`, `final_package_refresh_result=not_required`, `result=planned`, and `final_result=planned`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path.
- [ ] If running outside the original workspace, rehydrate the package from `dist/handoff-bundle` into a clean Rust workspace.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real `-UnityExe` and `-DataRoot`; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `dist/AutoDesignMaker-rust/final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-039
**Summary**: Added the final handoff acceptance entrypoint to the operator preflight script check, then regenerated and smoke-tested the packaged handoff artifacts from a rehydrated workspace.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before continuing the goal.
- [x] Confirmed the current Rust package was assembled but still not externally accepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Updated `RUST/scripts/handoff_operator_preflight.ps1` so the operator preflight now checks `final_handoff_acceptance.ps1` in addition to the AI, Unity, external acceptance, and release gate scripts.
- [x] Updated `RUST/README.md` so the operator preflight documentation says it checks the final acceptance script as well as the gate scripts.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.
- [x] Rehydrated the refreshed handoff bundle into `RUST/target/handoff-preflight-script-smoke-039`.
- [x] Ran operator preflight from the rehydrated workspace and confirmed `script_count=5`, including `final_handoff_acceptance.ps1`.
- [x] Confirmed the rehydrated operator preflight still reports only the expected missing operator inputs: `ai_secret` and `unity_exe`.

**Verification**:
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\handoff_operator_preflight.ps1 -DataRoot .adm_rust_data`: passed and reported `script_count=5`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun`: passed before package refresh and wrote a planned final acceptance report.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed; it also ran `cargo fmt --check` and `cargo check --workspace`.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\source-bundle\scripts\rehydrate_handoff_workspace.ps1 -DestinationPath ..\..\target\handoff-preflight-script-smoke-039`: passed.
- [x] Rehydrated preflight command passed from `RUST/target/handoff-preflight-script-smoke-039` and reported `script_count=5`.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] Cross-layer text check confirmed final acceptance command and package-refresh wording are present in source, source bundle, handoff bundle, final manifest, and rehydration manifest.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:d505d9900ab9bffc`.
- [x] Unified handoff bundle is ready: `handoff_bundle_file_count=111`, `handoff_bundle_hash=fnv64:cdafb72e31b17818`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:b5aef39a0730498f`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:a23ed207f362f449`.
- [x] Final package contains `AutoDesignMaker-rust/final-acceptance-run.adm`: hash `fnv64:68b46d8c0daa47bc`.
- [x] Final package contains `source-bundle/scripts/final_handoff_acceptance.ps1`: hash `fnv64:0a04c29ee79d866c`.
- [x] Final package contains `source-bundle/scripts/handoff_operator_preflight.ps1`: hash `fnv64:eaca5a08f5da97c7`.
- [x] Default final acceptance run report is currently a dry-run plan: `dry_run=true`, `final_package_refresh_after_report=false`, `final_package_refresh_result=not_required`, `result=planned`, and `final_result=planned`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path.
- [ ] If running outside the original workspace, rehydrate the package from `dist/handoff-bundle` into a clean Rust workspace.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real `-UnityExe` and `-DataRoot`; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `dist/AutoDesignMaker-rust/final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-038
**Summary**: Fixed the final acceptance report packaging order so a successful real final acceptance run writes the final report before refreshing the final handoff package.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before changing code.
- [x] Confirmed the current package was assembled but still not externally accepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified a real final-delivery evidence issue: `final_handoff_acceptance.ps1` wrote its final `passed` report after strict release gate packaging, so a successful real run could package an earlier planned report.
- [x] Updated `RUST/scripts/final_handoff_acceptance.ps1` so successful non-dry-run executions using the default report path write the final report first, then run `cargo run -q -p adm-cli -- finalize-handoff-package` to refresh the final handoff package with that final report.
- [x] Kept dry-run and custom report path behavior non-refreshing to avoid surprising package writes.
- [x] Added `final_package_refresh_after_report` and `final_package_refresh_result` rows to the final acceptance report.
- [x] Added report-line replacement logic so package-refresh failure reports do not contain contradictory `final_result` rows.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so generated handoff instructions and top-level `HANDOFF_README.txt` expose `final_acceptance_package_refresh=after-successful-default-report-write`.
- [x] Updated `RUST/scripts/rehydrate_handoff_workspace.ps1` so rehydrated workspaces emit the same final acceptance package-refresh policy.
- [x] Updated `RUST/README.md` to document that a successful real final acceptance run refreshes the final package after writing the final report.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun`: passed.
- [x] Dry-run report contains `final_package_refresh_after_report=false`, `final_package_refresh_result=not_required`, `result=planned`, and `final_result=planned`.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] Expected failure smoke passed: a non-dry-run final acceptance command without AI secret or Unity path exited at operator preflight and wrote a failed custom report with no final package refresh.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed package evidence.
- [x] The release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] `cargo test --workspace`: passed.
- [x] Packaged bundle-root rehydration test passed from `RUST/dist/handoff-bundle` into `target/handoff-final-acceptance-refresh-smoke`.
- [x] Final acceptance dry-run passed from the rehydrated packaged workspace and wrote that workspace's default report.
- [x] Cross-layer check confirmed `final_acceptance_package_refresh`, report refresh rows, and updated scripts are present in source, `dist/source-bundle`, generated `handoff-instructions.adm`, evidence copy, top-level `HANDOFF_README.txt`, rehydration manifest, and final manifest.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:1ddb0d2245b1efdb`.
- [x] Unified handoff bundle is ready: `handoff_bundle_file_count=111`, `handoff_bundle_hash=fnv64:9d0d76e5825050c2`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:05b7b25358da1690`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:e83e871db6e824a7`.
- [x] Final package contains `AutoDesignMaker-rust/final-acceptance-run.adm`: hash `fnv64:3bf7505c85c25b17`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:c7ca0c2dab94e635`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:18706171e8c2b64b`.
- [x] Final package contains `source-bundle/scripts/final_handoff_acceptance.ps1`: hash `fnv64:0a04c29ee79d866c`.
- [x] Final package contains `source-bundle/scripts/rehydrate_handoff_workspace.ps1`: hash `fnv64:01653fdd26095be7`.
- [x] Default final acceptance run report is currently a dry-run plan: `dry_run=true`, `result=planned`, `final_result=planned`.
- [x] Current generated operator input checklist still has 2 entries: AI secret and Unity executable path.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path.
- [ ] If running outside the original workspace, rehydrate the package from `dist/handoff-bundle` into a clean Rust workspace.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real `-UnityExe` and `-DataRoot`; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `dist/AutoDesignMaker-rust/final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-037
**Summary**: Added a persistent final acceptance run report so the scripted final handoff acceptance sequence leaves auditable planned, passed, or failed step evidence.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before changing code.
- [x] Confirmed the current package was assembled but still not externally accepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified the next highest-value remaining local gap: `final_handoff_acceptance.ps1` could sequence the final gates, but did not persist per-step run evidence for operators or reviewers.
- [x] Updated `RUST/scripts/final_handoff_acceptance.ps1` to write `dist/AutoDesignMaker-rust/final-acceptance-run.adm` by default.
- [x] The final acceptance report records dry-run mode, resolved inputs, the exact five-step command plan, per-step dry-run/pass/fail status, final result, and failure error when the chain stops early.
- [x] Added `-ReportPath` so operators can redirect the run report without changing the default handoff evidence location.
- [x] Verified an expected real-run failure without AI secret or Unity path writes a failed report before exiting nonzero.
- [x] Updated `RUST/scripts/rehydrate_handoff_workspace.ps1` so rehydrated workspaces emit `final_acceptance_report=dist\AutoDesignMaker-rust\final-acceptance-run.adm`.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so generated handoff instructions and top-level `HANDOFF_README.txt` expose `final_acceptance_report`.
- [x] Added adm-cli regression assertions for `final_acceptance_report` in generated handoff instructions and handoff README fallback forwarding.
- [x] Updated `RUST/README.md` to document the final acceptance run report.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun`: passed and wrote the default planned run report.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun -RequireAiInvoke`: passed and printed the real-invocation-required sequence.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed package evidence.
- [x] The release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] `cargo test --workspace`: passed.
- [x] Packaged bundle-root rehydration test passed from `RUST/dist/handoff-bundle` into `target/handoff-final-acceptance-report-smoke`.
- [x] Final acceptance dry-run passed from the rehydrated packaged workspace and wrote that workspace's default `dist/AutoDesignMaker-rust/final-acceptance-run.adm`.
- [x] Expected failure smoke passed: a non-dry-run final acceptance command without AI secret or Unity path exited at operator preflight and wrote `target/final-acceptance-failure-smoke.adm` with `result=failed`, failed `step=1`, and `error=Operator preflight failed with exit code 1`.
- [x] Cross-layer check confirmed `final_acceptance_report` and the updated script are present in source, `dist/source-bundle`, generated `handoff-instructions.adm`, evidence copy, top-level `HANDOFF_README.txt`, rehydration manifest, and final manifest.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:1e6998a3e6e1f271`.
- [x] Unified handoff bundle is ready: `handoff_bundle_file_count=111`, `handoff_bundle_hash=fnv64:a39c39fb08c0b633`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:505b06d68be8fc99`.
- [x] Final handoff package artifact is assembled: `file_count=122`, `package_ready=true`, `package_hash=fnv64:aa7b718f6569674b`.
- [x] Final package contains `AutoDesignMaker-rust/final-acceptance-run.adm`: hash `fnv64:ca5efed2d7fc2586`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:50aab26eec387419`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:9af9d4acee007ecf`.
- [x] Final package contains `source-bundle/scripts/final_handoff_acceptance.ps1`: hash `fnv64:6bfb5e50dd5e9c4a`.
- [x] Final package contains `source-bundle/scripts/rehydrate_handoff_workspace.ps1`: hash `fnv64:e010ea0845fcceb7`.
- [x] Default final acceptance run report is currently a dry-run plan: `dry_run=true`, `result=planned`, `final_result=planned`.
- [x] Current generated operator input checklist still has 2 entries: AI secret and Unity executable path.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path.
- [ ] If running outside the original workspace, rehydrate the package from `dist/handoff-bundle` into a clean Rust workspace.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with real `-UnityExe` and `-DataRoot`; add `-RequireAiInvoke` when final acceptance must prove a real provider call.
- [ ] Review `dist/AutoDesignMaker-rust/final-acceptance-run.adm`, `handoff-status.adm`, and `final-handoff-manifest.adm` after the real run.
- [ ] Confirm final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-036
**Summary**: Added a final handoff acceptance orchestrator so a receiver can run the remaining strict acceptance sequence from a rehydrated or original Rust workspace with one command.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before changing code.
- [x] Rechecked current generated evidence and confirmed the final package is assembled but not externally accepted: `package_ready=true`, `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified the next highest-value gap: the package exposed all remaining commands, but still required the receiver to manually sequence operator preflight, AI acceptance, Unity acceptance, external acceptance, and the strict release gate.
- [x] Added `RUST/scripts/final_handoff_acceptance.ps1`.
- [x] The final acceptance script resolves the Rust workspace root, auto-selects handoff instructions from the release artifact or rehydrated bundle copy, derives default provider/model/preset/secret/archive inputs from the instructions, and runs the acceptance sequence in order.
- [x] Added `-DryRun` support so the full final command sequence can be audited without a real AI secret or Unity Editor.
- [x] Added `-RequireAiInvoke` support so the final run can require a real AI invocation when credentials are available.
- [x] Updated `RUST/scripts/rehydrate_handoff_workspace.ps1` so rehydrated workspaces emit final acceptance commands.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so generated handoff instructions and top-level handoff README expose final acceptance working dir, script, sequence, requirements, and suggested commands.
- [x] Added adm-cli regression assertions for generated handoff instructions and handoff README fallback forwarding.
- [x] Updated `RUST/README.md` to document the final acceptance script and required external inputs.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun`: passed and printed the full final sequence.
- [x] `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -DryRun -RequireAiInvoke`: passed and printed the real-invocation-required sequence.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed package evidence.
- [x] The release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] `cargo test --workspace`: passed.
- [x] Packaged bundle-root rehydration test passed from `RUST/dist/handoff-bundle` into `target/handoff-final-acceptance-smoke`.
- [x] Final acceptance dry-run passed from the rehydrated packaged workspace and auto-selected `dist\handoff-bundle\evidence\handoff-instructions.adm`.
- [x] Cross-layer check confirmed the new final acceptance script and fields are present in source, `dist/source-bundle`, generated `handoff-instructions.adm`, evidence copy, top-level `HANDOFF_README.txt`, source-bundle README, rehydration manifest, and final manifest.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=58`, `source_bundle_hash=fnv64:fd18b872271f91d1`.
- [x] Unified handoff bundle is ready: `handoff_bundle_file_count=110`, `handoff_bundle_hash=fnv64:9dbded93dba7d9da`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:4a5dc78dc0235fa5`.
- [x] Final handoff package artifact is assembled: `file_count=121`, `package_ready=true`, `package_hash=fnv64:d6dd6334c835a218`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:f328255b9c351739`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:003d3c75f7f061f9`.
- [x] Final package contains `source-bundle/scripts/final_handoff_acceptance.ps1`: hash `fnv64:cc61d8a0ac83b78a`.
- [x] Final package contains `source-bundle/scripts/rehydrate_handoff_workspace.ps1`: hash `fnv64:50b6f2a982a8c7f7`.
- [x] Current generated operator input checklist still has 2 entries: AI secret and Unity executable path.
- [x] Current generated remaining required execution sequence still has 5 steps, now callable through `scripts/final_handoff_acceptance.ps1`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Provide a compatible Unity Editor executable path.
- [ ] If running outside the original workspace, rehydrate the package from `dist/handoff-bundle` into a clean Rust workspace.
- [ ] Run `scripts/final_handoff_acceptance.ps1` with the real `-UnityExe` and `-DataRoot` values; add `-RequireAiInvoke` when the final run must prove real provider invocation.
- [ ] Review the refreshed final manifest and confirm `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 0-1% of the Rust rebuild unless external acceptance uncovers defects.
- [ ] Remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are already ready, expected calendar time is about 1-3 hours because the final sequence is now scripted.
- [ ] If Unity install/import/provider setup is still needed, expected calendar time remains about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-035
**Summary**: Added a handoff bundle rehydration script and generated handoff fields so a receiver can restore a runnable Rust workspace from `dist/handoff-bundle`.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before changing code.
- [x] Rechecked external acceptance prerequisites and confirmed the local shell still has no `OPENAI_API_KEY`, `ADM_UNITY_EDITOR`, or default Unity Editor path.
- [x] Identified a remaining handoff gap: the final package explained that strict gates require a Rust workspace root, but did not provide an executable restore step from the handoff bundle root.
- [x] Added `RUST/scripts/rehydrate_handoff_workspace.ps1`.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `write-handoff-instructions` emits handoff rehydration metadata and `suggested_handoff_rehydration_command`.
- [x] Updated final handoff README rendering to forward those rehydration fields into top-level `HANDOFF_README.txt`.
- [x] Added adm-cli regression assertions for generated handoff instructions and final README fallback forwarding.
- [x] Updated `RUST/README.md` to document the rehydration flow.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] Source script smoke test restored `target/handoff-rehydrate-smoke` with `ready=true`, `source_workspace_restored=true`, `handoff_bundle_restored=true`, and `copied_dist_artifact_count=4`.
- [x] Rehydrated workspace preflight passed structurally from `target/handoff-rehydrate-smoke` and reported only the expected external missing inputs: `ai_secret` and `unity_exe`.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed package evidence.
- [x] The release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Packaged bundle-root rehydration test passed from `RUST/dist/handoff-bundle` into `target/handoff-rehydrate-bundle-smoke`.
- [x] Rehydrated packaged workspace preflight passed structurally and reported only the expected external missing inputs: `ai_secret` and `unity_exe`.
- [x] `cargo test --workspace`: passed.
- [x] Cross-layer check confirmed the new script and fields are present in source, `dist/source-bundle`, generated `handoff-instructions.adm`, evidence copy, top-level `HANDOFF_README.txt`, source-bundle README, and final manifest.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_file_count=57`, `source_bundle_hash=fnv64:5801a4ef05660651`.
- [x] Unified handoff bundle is ready: `handoff_bundle_file_count=109`, `handoff_bundle_hash=fnv64:a2d82269406e0a99`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:6848aeb337a806d3`.
- [x] Final handoff package artifact is assembled: `file_count=120`, `package_ready=true`, `package_hash=fnv64:c37ce99653ca5cde`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:a08db09c13d026a5`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:adb0d8b8318b9391`.
- [x] Final package contains `source-bundle/scripts/rehydrate_handoff_workspace.ps1`: hash `fnv64:1ca0e43190948f2d`.
- [x] Current generated operator input checklist still has 2 entries: AI secret and Unity executable path.
- [x] Current generated remaining required execution sequence still has 5 steps: configure real AI provider, run Unity acceptance, rerun external acceptance, run strict release gate, confirm final delivery package.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Provide `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] If running outside the original workspace, run the new handoff bundle rehydration command into a clean destination, then run the workspace-root operator preflight from that destination.
- [ ] Run the AI acceptance command exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the Unity acceptance command with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-034
**Summary**: Added bundle-root operator preflight commands so a receiver can validate handoff inputs directly from `dist/handoff-bundle`.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before changing code.
- [x] Confirmed the current final package was assembled but not externally accepted: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Identified a handoff usability gap: the existing operator preflight commands assumed a Rust workspace root, while the packaged receiver entrypoint is `dist/handoff-bundle`.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `write-handoff-instructions` emits bundle-root preflight metadata.
- [x] Updated final handoff README rendering to forward the same bundle-root preflight fields into `HANDOFF_README.txt`.
- [x] Added adm-cli regression assertions for generated handoff instructions and final README fallback forwarding.
- [x] Updated `RUST/README.md` to document the bundle-root operator preflight flow.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed package evidence.
- [x] The release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] `cargo test --workspace`: passed.
- [x] Bundle-root preflight ran successfully from `RUST/dist/handoff-bundle` and reported `ready=false` only because `ai_secret` and `unity_exe` are still missing.
- [x] Cross-layer check confirmed the new fields are present in source, `dist/source-bundle`, generated `handoff-instructions.adm`, evidence copy, and top-level `HANDOFF_README.txt`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:b71d2bbdcd76419b`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:1b0113d51ceb4714`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:768bbeaba89c1721`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:15f418cbd176583e`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:8a911f56417a5f4c`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:cbf6b97900535fee`.
- [x] Current generated operator input checklist still has 2 entries: AI secret and Unity executable path.
- [x] Current generated remaining required execution sequence still has 5 steps: configure real AI provider, run Unity acceptance, rerun external acceptance, run strict release gate, confirm final delivery package.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the bundle-root or workspace-root operator preflight with the receiving Unity path and DataRoot, then require readiness before expensive gates.
- [ ] Run the AI acceptance command exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the Unity acceptance command with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-033
**Summary**: Added a handoff operator preflight script and forwarded copyable preflight commands into Rust handoff evidence.

**Completed**:
- [x] Re-read `AI_README.md`, active memory, and current Rust handoff evidence before changing code.
- [x] Rechecked the local environment and confirmed `OPENAI_API_KEY` is still absent, `ADM_UNITY_EDITOR` is unset, and the default Unity paths are unavailable.
- [x] Added `RUST/scripts/handoff_operator_preflight.ps1`.
- [x] The preflight script parses `handoff-instructions.adm`, checks required operator inputs, verifies gate scripts are present, detects the required AI secret environment variable without printing its value, checks a supplied or `ADM_UNITY_EDITOR` Unity path, validates DataRoot presence, and verifies the suggested archive manifest when available.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `write-handoff-instructions` emits `suggested_operator_preflight_command` and `suggested_operator_preflight_require_ready_command`.
- [x] Updated final handoff README rendering to forward the same preflight commands into `HANDOFF_README.txt`.
- [x] Added adm-cli regression assertions for both generated handoff instructions and final README forwarding.
- [x] Updated `RUST/README.md` to document the operator preflight flow.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\handoff_operator_preflight.ps1 -DataRoot .adm_rust_data`: passed and reported `ready=false` with `missing_input_count=2` for `ai_secret` and `unity_exe`.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] The release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Copied source-bundle script verification passed: `dist/source-bundle/scripts/handoff_operator_preflight.ps1` ran with explicit instructions/DataRoot paths and reported the same missing AI secret and Unity executable inputs.
- [x] `cargo test --workspace`: passed after the final script workdir correction.
- [x] Cross-layer check confirmed the preflight script and generated fields are present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, top-level `HANDOFF_README.txt`, source manifest, and final handoff manifest.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:ca4ac8633bc78574`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:d62d7fc4de3a09d0`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:afac7e0138c2c419`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:37cd75531e0a383a`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:dcf484a68efe1e3a`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:ea244cc11dbb3ece`.
- [x] Final package contains `source-bundle/scripts/handoff_operator_preflight.ps1`: hash `fnv64:4178bd34e5465de6`.
- [x] Current generated operator input checklist still has 2 entries: AI secret and Unity executable path.
- [x] Current generated remaining required execution sequence still has 5 steps: configure real AI provider, run Unity acceptance, rerun external acceptance, run strict release gate, confirm final delivery package.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the new operator preflight with the receiving Unity path and DataRoot, then require readiness before expensive gates.
- [ ] Run the AI acceptance command exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the Unity acceptance command with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-032
**Summary**: Added machine-readable operator input rows for AI secret and Unity executable handoff prerequisites.

**Completed**:
- [x] Re-read the active project memory and current Rust handoff evidence before changing code.
- [x] Confirmed no `OPENAI_API_KEY` is present in the current shell and the default Unity paths are unavailable locally.
- [x] Confirmed the final handoff package is still assembled but not externally accepted: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Found that the handoff flow still required `<secret>` and `<path-to-Unity.exe>` values, but did not expose a central machine-readable operator input checklist.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `write-handoff-instructions` emits `operator_input_count` and `operator_input=` rows for the AI secret and Unity executable path.
- [x] Updated final handoff README rendering to forward the same operator input rows.
- [x] Added adm-cli regression assertions for generated handoff instructions and final README forwarding.
- [x] Updated `RUST/README.md` to document the new operator input fields.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed after an escalated rerun because the sandboxed attempt hit access denied on `apps/adm-cli/src/main.rs`.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed `operator_input_count` and `operator_input=` are present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, top-level `HANDOFF_README.txt`, and `RUST/README.md`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:3a10d59c595ca3f0`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:7de7d95fb0a48839`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:d68f5c0318496002`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:a7839e87191aeff2`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:83ce785afee73bea`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:f1ddf66fede024b8`.
- [x] Current generated operator input checklist has 2 entries: AI secret and Unity executable path.
- [x] Current generated remaining required execution sequence has 5 steps: configure real AI provider, run Unity acceptance, rerun external acceptance, run strict release gate, confirm final delivery package.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the AI acceptance command exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the Unity acceptance command with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-031
**Summary**: Forwarded AI provider diagnostic details into Rust handoff instructions and the final handoff README.

**Completed**:
- [x] Re-read the active project memory and current Rust handoff evidence before changing code.
- [x] Confirmed the final handoff package is still assembled but not externally accepted: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Found that `ai-acceptance.adm` and `external-acceptance.adm` already include AI diagnostic provider rows, but `handoff-instructions.adm` and top-level `HANDOFF_README.txt` only exposed aggregate counts such as `ready_provider_count=1`.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `write-handoff-instructions` extracts tab-separated AI diagnostic provider rows and emits `ai_provider_detail_count` plus `ai_provider=` rows.
- [x] Updated final handoff README rendering to forward the same provider detail rows.
- [x] Added adm-cli regression assertions for generated instructions and final README forwarding.
- [x] Updated `RUST/README.md` to document the new AI provider detail fields.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed `ai_provider_detail_count` and `ai_provider=` are present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, top-level `HANDOFF_README.txt`, and `RUST/README.md`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:5f7c2d07fa465f08`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:2793360e3e7f1605`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:0cb20359ca7e6b70`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:9f644fd839c3603f`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:652ba09d8fe6b669`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:d6d1378624df4295`.
- [x] Current generated AI provider details show one ready provider: `mock`, readiness `Ready`, capability `text_generation`, note `provider does not require a secret`.
- [x] Current generated remaining required execution sequence has 5 steps: configure real AI provider, run Unity acceptance, rerun external acceptance, run strict release gate, confirm final delivery package.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the AI acceptance command exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the Unity acceptance command with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-030
**Summary**: Forwarded Unity editor discovery candidate details into Rust handoff instructions and the final handoff README.

**Completed**:
- [x] Re-read the active project memory and current Rust handoff evidence before changing code.
- [x] Confirmed the final handoff package is still assembled but not externally accepted: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Found that `external-acceptance.adm` included detailed Unity editor discovery rows, but `handoff-instructions.adm` and top-level `HANDOFF_README.txt` only exposed the aggregate `unity_candidates=2` count.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `write-handoff-instructions` extracts Unity discovery rows and emits `unity_candidate_detail_count` plus `unity_candidate=` rows.
- [x] Updated final handoff README rendering to forward the same candidate detail rows.
- [x] Added adm-cli regression assertions for generated instructions and final README forwarding.
- [x] Updated `RUST/README.md` to document the new Unity candidate detail fields.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the new Unity candidate detail fields are present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, top-level `HANDOFF_README.txt`, and `RUST/README.md`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:cfe3261a27d3e6b6`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:d3c8e7d61338644f`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:e6033d47f1feeb5c`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:ae5c55765e750a63`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:0543828145c9c9ed`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:60008c069d7fa810`.
- [x] Current generated Unity candidate details show two default paths, both missing/not ready: `C:\Program Files\Unity\Editor\Unity.exe` and `C:\Program Files\Unity Hub\Editor\Unity.exe`.
- [x] Current generated remaining required execution sequence has 5 steps: configure real AI provider, run Unity acceptance, rerun external acceptance, run strict release gate, confirm final delivery package.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the AI acceptance command exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the Unity acceptance command with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-029
**Summary**: Added explicit Rust handoff bundle workspace rehydration and DataRoot guidance to the final handoff README.

**Completed**:
- [x] Confirmed the final handoff package is assembled but not accepted: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Found a receiver-facing gap in the generated final `HANDOFF_README.txt`: the handoff bundle root is not itself a directly runnable Rust workspace and does not include the original acceptance `DataRoot`.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so the generated final README emits explicit restore fields for bundle layout, missing DataRoot, original DataRoot, matching DataRoot requirement, rehydrated workspace requirement, source directory, dist artifact directories, and restore note.
- [x] Added adm-cli regression assertions for the new final README fields.
- [x] Updated `RUST/README.md` to document the restore flow: use `source-bundle` as the Rust workspace root, place artifact directories under that workspace's `dist/`, and provide the same `DataRoot` or imported equivalent before rerunning acceptance gates.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the new rehydration/DataRoot fields are present in source, source bundle, handoff bundle source copy, generated top-level `HANDOFF_README.txt`, and `RUST/README.md`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:07a0c3bbe5fb4865`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:6b2ef9a98230aa0f`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:7dc113f7bc05fff0`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:3bb9f2b96e426880`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:45b62f433b2e666d`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:3290449babd4fb09`.
- [x] Current generated remaining required execution sequence has 5 steps: configure real AI provider, run Unity acceptance, rerun external acceptance, run strict release gate, confirm final delivery package.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the AI acceptance command exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the Unity acceptance command with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-028
**Summary**: Added an ordered remaining-required execution sequence to the Rust handoff instructions and final handoff bundle README.

**Completed**:
- [x] Confirmed the current final package is assembled but not accepted: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, and `ai_acceptance_ready=false`.
- [x] Found that generated `instruction=` rows contained the needed actions, but receivers still had to filter required/not-ready rows and preserve order manually.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `handoff-instructions.adm` emits `remaining_required_execution_step_count` plus ordered `remaining_required_execution_step=` rows.
- [x] Each execution step now includes the step index, instruction id, status, estimate, command, evidence, done condition, and note.
- [x] Updated final handoff README rendering to forward the same ordered remaining-required execution rows.
- [x] Updated adm-cli regression tests for generated instructions, missing-final-manifest behavior, and README forwarding.
- [x] Updated `RUST/README.md` to document the new execution sequence fields.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed `remaining_required_execution_step_count` and `remaining_required_execution_step=` are present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, and top-level `HANDOFF_README.txt`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:5a09482d94f748e6`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:7c6c946f1880c61b`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:3fd794d44e22fdf1`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:80238dacdd5e51ee`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:a408bd6bc7ae4d98`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:3290449babd4fb09`.
- [x] Current generated remaining required execution sequence has 5 steps: configure real AI provider, run Unity acceptance, rerun external acceptance, run strict release gate, confirm final delivery package.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the AI acceptance command exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the Unity acceptance command with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-027
**Summary**: Added a copyable but redacted PowerShell session command to the Rust handoff AI secret guidance.

**Completed**:
- [x] Found that the handoff already exposed the suggested AI secret env var, requirement, and check command, but did not include a direct current-session set command template.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `handoff-instructions.adm` emits `suggested_ai_secret_session_set_command=$env:OPENAI_API_KEY='<secret>'` for the OpenAI preset.
- [x] Forwarded the same field into the top-level final handoff `HANDOFF_README.txt`.
- [x] Added a fallback value for older instruction files when rendering the handoff README.
- [x] Updated adm-cli regression fixtures/assertions for both the generated instructions and final README forwarding path.
- [x] Updated `RUST/README.md` to document that the generated set command is a placeholder-only template and must be filled locally by the receiving operator.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed `suggested_ai_secret_session_set_command=$env:OPENAI_API_KEY='<secret>'` is present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, and top-level `HANDOFF_README.txt`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:62a6e5c2b95eac7b`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:7cf0bd4a032a6eb2`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:fb08f23ec6adf295`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:cf29ef2d379d6d3b`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:306563d906477e82`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:c670f10919cf28b7`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the concrete AI acceptance instruction exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the concrete Unity acceptance instruction with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-026
**Summary**: Unified the Rust handoff Unity acceptance instruction evidence path with the final package evidence path.

**Completed**:
- [x] Found that `instruction=run-unity-acceptance` still reported `evidence=validation/runtime_execution_results.adm`, while the matching external dependency and blocker resolution rows already pointed to the final package path.
- [x] Added `UNITY_PLAYMODE_EVIDENCE_PATH` in `RUST/apps/adm-cli/src/main.rs`.
- [x] Updated Unity acceptance instruction evidence, Unity external dependency evidence, and Unity blocker resolution evidence to use the shared path.
- [x] Updated adm-cli regression fixtures/assertions so final handoff README forwarding expects `dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm`.
- [x] Updated `RUST/README.md` to document that Unity instruction evidence uses the final package path.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the updated Unity instruction evidence path is present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, and top-level `HANDOFF_README.txt`.
- [x] Cross-layer check found no remaining `instruction=run-unity-acceptance` rows with `evidence=validation/runtime_execution_results.adm` in generated handoff instruction/README evidence.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:410038785a25be9f`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:b2bd71961871656c`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:0a3c0e2cd4de310e`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:0f16fe93f1851b49`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:0365a768689cfa1a`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:e5102782cdeb297a`.
- [x] `handoff-instructions.adm` and `HANDOFF_README.txt` now report `instruction=run-unity-acceptance` with `evidence=dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the concrete AI acceptance instruction exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the concrete Unity acceptance instruction with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-025
**Summary**: Fixed the generated Rust handoff AI secret check command so it is safe to copy from an interactive PowerShell session.

**Completed**:
- [x] Found that the previous generated `suggested_ai_secret_check_command=powershell -NoProfile -Command "[bool]$env:OPENAI_API_KEY"` can be expanded by the parent PowerShell before the nested command receives it.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so generated secret check commands escape the environment expression as `` `[bool]`$env:<NAME>``.
- [x] Updated fallback handoff README rendering and adm-cli test fixtures to expect the escaped form.
- [x] Updated `RUST/README.md` to document why the generated command escapes `$env:`.
- [x] Verified the escaped command returns `False` in the current environment instead of printing the Boolean type metadata.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `powershell -NoProfile -Command "[bool]`$env:OPENAI_API_KEY"`: returned `False`.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the escaped secret check command is present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, and top-level `HANDOFF_README.txt`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:a71f195e91669388`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:644fb36f98a8d657`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:87d0784f302b2c19`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:bab5cd3c60e07325`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:df3920bf7da3afe5`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:eee6ab6a10afd545`.
- [x] `handoff-instructions.adm` and `HANDOFF_README.txt` now report `suggested_ai_secret_check_command=powershell -NoProfile -Command "[bool]`$env:OPENAI_API_KEY"`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the concrete AI acceptance instruction exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the concrete Unity acceptance instruction with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-024
**Summary**: Added machine-readable external dependency rows to Rust handoff instructions and the final handoff bundle README.

**Completed**:
- [x] Confirmed the current environment still lacks `OPENAI_API_KEY` and no Unity Editor was found in common Unity Hub paths.
- [x] Added `HandoffExternalDependency` generation in `RUST/apps/adm-cli/src/main.rs`.
- [x] `handoff-instructions.adm` now emits `external_dependency_count` and `external_dependency=` rows for missing receiving-environment prerequisites.
- [x] Current generated dependencies are `real_ai_provider` and `unity_playmode`.
- [x] `HANDOFF_README.txt` now forwards the external dependency count and rows from `handoff-instructions.adm`.
- [x] Added adm-cli regression coverage for generated dependency rows, including the stricter real-provider invoke case when required.
- [x] Updated `RUST/README.md` to document the new dependency fields and final README forwarding.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed `external_dependency_count` and `external_dependency=` rows are present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, and top-level `HANDOFF_README.txt`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:d1223e7830afbd0c`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:909dd9daf0aa9218`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:cecde6c46266f303`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:0499c09a28220a88`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:798c927294a8d04b`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:32cac9b04da9e6af`.
- [x] `handoff-instructions.adm` and `HANDOFF_README.txt` now report `external_dependency_count=2`.
- [x] `external_dependency=real_ai_provider` reports `status=missing_secret_or_provider_config`, `requirement=env:OPENAI_API_KEY`, and unlocks `configure-real-ai-provider`.
- [x] `external_dependency=unity_playmode` reports `status=unity_not_ready`, `requirement=compatible-unity-editor-path`, and unlocks `run-unity-acceptance`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the concrete AI acceptance instruction exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the concrete Unity acceptance instruction with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-023
**Summary**: Expanded Rust handoff next-required-instruction fields and forwarded them into the final handoff bundle README.

**Completed**:
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `handoff-instructions.adm` now emits expanded `next_required_instruction_*` fields for the first required instruction that is not ready.
- [x] Added `next_required_instruction_status`, `next_required_instruction_estimate`, `next_required_instruction_command`, `next_required_instruction_evidence`, `next_required_instruction_done_when`, and `next_required_instruction_note`.
- [x] Updated `render_handoff_bundle_readme()` so top-level `RUST/dist/handoff-bundle/HANDOFF_README.txt` forwards those expanded fields from the handoff instruction evidence.
- [x] Added adm-cli regression coverage for generated handoff instructions and final handoff bundle README forwarding.
- [x] Updated `RUST/README.md` to document the expanded next-required-instruction fields.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the expanded fields are present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, and top-level `HANDOFF_README.txt`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:66f9ffc2d74da630`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:171a0eecb4bafc19`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:9ec0e5ff3efa9e2a`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:2d70959f710f6768`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:8d567cf5ce4a8281`.
- [x] Final package contains refreshed `evidence/handoff-instructions.adm`: hash `fnv64:836ac2e7155134d6`.
- [x] `handoff-instructions.adm` and `HANDOFF_README.txt` now report `required_instruction_count=5`, `required_blocked_instruction_count=4`, `required_waiting_instruction_count=1`, `optional_instruction_count=3`, `manual_decision_instruction_count=1`, and `next_required_instruction=configure-real-ai-provider`.
- [x] `next_required_instruction_command` is the concrete `ai_acceptance_gate.ps1` command with provider, model, preset, secret ref, and current DataRoot.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the concrete AI acceptance instruction now exposed in top-level `HANDOFF_README.txt` and `handoff-instructions.adm`.
- [ ] Run the concrete Unity acceptance instruction with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-022
**Summary**: Added machine-readable Rust handoff instruction summary counts and forwarded them into the final handoff bundle README.

**Completed**:
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `handoff-instructions.adm` now emits `required_instruction_count`, `required_blocked_instruction_count`, `required_waiting_instruction_count`, `optional_instruction_count`, `manual_decision_instruction_count`, and `next_required_instruction`.
- [x] Updated `render_handoff_bundle_readme()` so top-level `RUST/dist/handoff-bundle/HANDOFF_README.txt` forwards those fields from the handoff instruction evidence.
- [x] Added adm-cli regression coverage for both generated `handoff-instructions.adm` and final `HANDOFF_README.txt` forwarding.
- [x] Updated `RUST/README.md` to document the new summary fields.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the new fields are present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, and top-level `HANDOFF_README.txt`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:94af478aee20c030`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:92fba94986336123`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:86734eb094270508`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:f76cd1804ab60b34`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:21bc7d2022f105c7`.
- [x] `handoff-instructions.adm` hash is `fnv64:649969cddc88d1f8`.
- [x] `handoff-instructions.adm` and `HANDOFF_README.txt` now report `required_instruction_count=5`, `required_blocked_instruction_count=4`, `required_waiting_instruction_count=1`, `optional_instruction_count=3`, `manual_decision_instruction_count=1`, and `next_required_instruction=configure-real-ai-provider`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the concrete AI acceptance instruction from `HANDOFF_README.txt` or `handoff-instructions.adm`.
- [ ] Run the concrete Unity acceptance instruction with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-021
**Summary**: Forwarded `strict_gate_requires_final_delivery=true` into the Rust top-level handoff bundle README.

**Completed**:
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `render_handoff_bundle_readme()` forwards `strict_gate_requires_final_delivery` from `handoff-instructions.adm`.
- [x] Added fallback `strict_gate_requires_final_delivery=true` for older handoff instruction evidence.
- [x] Updated final handoff package regression coverage so `HANDOFF_README.txt` must contain `strict_gate_requires_final_delivery=true`.
- [x] Updated `RUST/README.md` to document that the handoff package entry file forwards this strict final delivery requirement.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed `strict_gate_requires_final_delivery=true` is present in `dist/AutoDesignMaker-rust/handoff-instructions.adm`, the handoff evidence copy, top-level `dist/handoff-bundle/HANDOFF_README.txt`, source, source bundle, and handoff bundle source copy.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:c0e44c9a6f348885`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:d44ae5a0b0ff9c1b`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:90d0e4b0ac7cf33b`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:e1ddf02814022765`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:893e134f37d37ce6`.
- [x] `handoff-instructions.adm` hash is `fnv64:2c4d2594512eed90`.
- [x] Top-level `HANDOFF_README.txt` now forwards `strict_gate_requires_final_delivery=true`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the concrete AI acceptance instruction from `HANDOFF_README.txt` or `handoff-instructions.adm`.
- [ ] Run the concrete Unity acceptance instruction with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-020
**Summary**: Made Rust handoff canonical strict gate commands reuse the current DataRoot instead of leaving `<data_root>` placeholders.

**Completed**:
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so generated `strict_gate_command` reuses `suggested_strict_release_gate_command`.
- [x] Updated `strict_gate_ai_invoke_command` to reuse `suggested_strict_release_gate_ai_invoke_command`.
- [x] Added adm-cli regression coverage proving both canonical strict gate fields include the current DataRoot.
- [x] Updated the final handoff package fixture and assertions so `HANDOFF_README.txt` forwards concrete canonical strict gate commands.
- [x] Updated `RUST/README.md` to document that canonical strict gate fields use the current handoff DataRoot when known.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed `strict_gate_command`, `strict_gate_ai_invoke_command`, and the suggested strict gate command fields are consistent in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, and top-level `HANDOFF_README.txt`.
- [x] Search for canonical `strict_gate_command=.*<data_root>` and `strict_gate_ai_invoke_command=.*<data_root>` in generated handoff reports and source found no matches.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:467c4ac2ce87b902`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:a9bffe0a72f99a75`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:8dec86f719f5aadf`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:6189a3baed29b1a5`.
- [x] Final package contains the refreshed `HANDOFF_README.txt`: hash `fnv64:a4aa04ee51934300`.
- [x] `handoff-instructions.adm` hash is `fnv64:2c4d2594512eed90`.
- [x] Generated `strict_gate_command` now uses `E:\workwork\CrewAi\AutoDesignMaker\RUST\.adm_rust_data` instead of `<data_root>`.
- [x] Generated `strict_gate_ai_invoke_command` now uses `E:\workwork\CrewAi\AutoDesignMaker\RUST\.adm_rust_data` instead of `<data_root>`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the concrete AI acceptance instruction from `HANDOFF_README.txt` or `handoff-instructions.adm`.
- [ ] Run the concrete Unity acceptance instruction with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-019
**Summary**: Forwarded Rust handoff `instruction=` rows into the top-level handoff bundle README.

**Completed**:
- [x] Updated `RUST/apps/adm-cli/src/main.rs` so `render_handoff_bundle_readme()` copies all generated `instruction=` rows from `handoff-instructions.adm` into `dist/handoff-bundle/HANDOFF_README.txt`.
- [x] Added adm-cli regression coverage proving `HANDOFF_README.txt` includes `instruction_count=8` and representative concrete `configure-real-ai-provider` / `run-unity-acceptance` instruction rows.
- [x] Updated `RUST/README.md` to document that the handoff package entry file forwards manual action command/evidence/note rows.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package through the release gate.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the `instruction=` rows are present in source, generated source bundles, `dist/AutoDesignMaker-rust/handoff-instructions.adm`, evidence copy, and top-level `dist/handoff-bundle/HANDOFF_README.txt`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:18ad219e8b559d77`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:89b4b829029d86dd`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:06b9de211b0e4c68`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:dbf28f4ff080a595`.
- [x] Final package contains the enriched `HANDOFF_README.txt`: hash `fnv64:fe1f30c7faaa390b`.
- [x] `handoff-instructions.adm` hash remains `fnv64:6692f2aa4298e06a`.
- [x] Top-level `HANDOFF_README.txt` now includes `instruction_count=8` and concrete `instruction=` rows for real AI, Unity PlayMode acceptance, external acceptance, and final strict gate.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the concrete AI acceptance instruction from `HANDOFF_README.txt` or `handoff-instructions.adm`.
- [ ] Run the concrete Unity acceptance instruction with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is approximately 1% of the Rust rebuild; remaining total completion effort is approximately 3-5% if external Unity/AI environment setup and acceptance ownership are counted.
- [ ] If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-018
**Summary**: Made Rust handoff instruction rows reuse context-filled suggested commands instead of generic placeholders.

**Completed**:
- [x] Added `handoff_instruction_command()` in `RUST/apps/adm-cli/src/main.rs`.
- [x] `instruction=configure-real-ai-provider` now renders the concrete suggested AI acceptance command with provider, model, preset, secret ref, and DataRoot.
- [x] `instruction=run-ai-provider-invoke-acceptance` now renders the concrete suggested AI invoke acceptance command.
- [x] `instruction=run-unity-acceptance` now renders the concrete suggested Unity acceptance command, including the inferred latest archive id.
- [x] `instruction=rerun-external-acceptance` now renders the concrete external acceptance command with the current DataRoot.
- [x] `instruction=run-strict-release-gate` now renders the concrete strict release gate command with the current DataRoot.
- [x] Updated adm-cli tests and `RUST/README.md`.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the concrete instruction command behavior is present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, and the evidence copy.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:1e67875955c54834`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:9abbfc3089fc8039`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:0a7598357682243e`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:3e25b90ab66fb26b`.
- [x] Final package contains `HANDOFF_README.txt`: hash `fnv64:1c3e99494ce50994`.
- [x] `handoff-instructions.adm` hash is `fnv64:6692f2aa4298e06a`.
- [x] `instruction=run-unity-acceptance` now uses `archive_1783253820821_24440_1` and `E:\workwork\CrewAi\AutoDesignMaker\RUST\.adm_rust_data`.
- [x] `instruction=configure-real-ai-provider`, `instruction=rerun-external-acceptance`, and `instruction=run-strict-release-gate` now use the current DataRoot.
- [x] Current generated reports list `suggested_ai_secret_env_var=OPENAI_API_KEY`.
- [x] Current generated reports list `blocker_resolution_count=6`, matching `blocker_count=6`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the concrete AI acceptance instruction from `handoff-instructions.adm` or the blocker resolution rows.
- [ ] Run the concrete Unity acceptance instruction with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is still approximately 1-2% of the Rust rebuild. If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-017
**Summary**: Added explicit AI secret environment-variable prerequisites to Rust handoff instructions and final package README.

**Completed**:
- [x] Added `suggested_ai_secret_env_var`, `suggested_ai_secret_requirement`, and `suggested_ai_secret_check_command` to `RUST/apps/adm-cli/src/main.rs`.
- [x] Mapped built-in AI presets to their default environment variables: `openai` -> `OPENAI_API_KEY`, `openrouter` -> `OPENROUTER_API_KEY`, `deepseek` -> `DEEPSEEK_API_KEY`, `local_openai` -> `none`.
- [x] `handoff-instructions.adm` now writes the secret prerequisite fields before the suggested AI acceptance commands.
- [x] `HANDOFF_README.txt` now forwards those fields so the final package entrypoint is a machine-readable prerequisite checklist.
- [x] Updated adm-cli tests and `RUST/README.md`.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the new secret prerequisite fields are present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, evidence copy, and final `HANDOFF_README.txt`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:326f9317aa963d2b`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:242d8cdd0f9539bc`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:3267174d0c84af8c`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:3b52cea6c2ed9f60`.
- [x] Final package contains `HANDOFF_README.txt`: hash `fnv64:8e6c7bb88687979e`.
- [x] `handoff-instructions.adm` hash is `fnv64:77d3c3d6c9496f9c`.
- [x] Current generated reports list `suggested_ai_secret_env_var=OPENAI_API_KEY`, `suggested_ai_secret_requirement=env:OPENAI_API_KEY`, and `suggested_ai_secret_check_command=powershell -NoProfile -Command "[bool]$env:OPENAI_API_KEY"`.
- [x] Current generated reports list `suggested_unity_archive_id=archive_1783253820821_24440_1` and `suggested_unity_archive_source=data_root_latest_archive`.
- [x] Current generated reports list `blocker_resolution_count=6`, matching `blocker_count=6`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the AI blocker resolution command from `HANDOFF_README.txt` or `handoff-instructions.adm`.
- [ ] Run the Unity blocker resolution command with a compatible Unity editor; the archive id is already prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is still approximately 1-2% of the Rust rebuild. If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-016
**Summary**: Made Rust handoff instructions infer and prefill the latest Unity acceptance archive id from DataRoot.

**Completed**:
- [x] Added `latest_archive_id_from_data_root()` in `RUST/apps/adm-cli/src/main.rs`.
- [x] The helper scans `<data_root>/archives`, keeps only directories with valid archive ids and matching `manifest.adm`, sorts archive ids, and returns the latest one.
- [x] `handoff-instructions.adm` now writes `suggested_unity_archive_id` and `suggested_unity_archive_source`.
- [x] `suggested_unity_acceptance_command` and Unity blocker resolution commands now use the inferred archive id when available.
- [x] `HANDOFF_README.txt` now forwards `suggested_unity_archive_id` and `suggested_unity_archive_source`.
- [x] Updated `RUST/README.md` and adm-cli tests.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the new archive fields are present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, and final `HANDOFF_README.txt`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:cd1cf49ea4448073`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:fc284d9fda410bf7`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:b5a61602962e0043`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:db879f7ef02502e0`.
- [x] Final package contains `HANDOFF_README.txt`: hash `fnv64:4819f22178cfce36`.
- [x] `handoff-instructions.adm` hash is `fnv64:79823e32704fd53c`.
- [x] Current generated reports list `suggested_unity_archive_id=archive_1783253820821_24440_1` and `suggested_unity_archive_source=data_root_latest_archive`.
- [x] Current generated reports list `blocker_resolution_count=6`, matching `blocker_count=6`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set a real provider secret such as `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the AI blocker resolution command from `HANDOFF_README.txt` or `handoff-instructions.adm`.
- [ ] Run the Unity blocker resolution command with a compatible Unity editor; the archive id is now prefilled as `archive_1783253820821_24440_1`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is still approximately 1-2% of the Rust rebuild. If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-015
**Summary**: Added machine-readable blocker resolution rows to Rust handoff instructions and final package README.

**Completed**:
- [x] Added `HandoffBlockerResolution` rendering in `RUST/apps/adm-cli/src/main.rs`.
- [x] `handoff-instructions.adm` now writes `blocker_resolution_count` plus one `blocker_resolution=` row for each blocker.
- [x] Each blocker resolution includes `action`, `command`, `evidence`, and `done_when`.
- [x] Covered local release blockers, external acceptance blockers, Unity PlayMode blockers, real AI provider blockers, AI invoke blockers, DataRoot/provider mismatch blockers, source bundle blockers, and handoff bundle blockers.
- [x] `HANDOFF_README.txt` now forwards `blocker_resolution_count` and all `blocker_resolution=` rows.
- [x] Updated `RUST/README.md` and adm-cli tests.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed `blocker_resolution_count` and `blocker_resolution=` are present in source, source bundle, handoff bundle source copy, generated `handoff-instructions.adm`, and final `HANDOFF_README.txt`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:25a50856309e6e78`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:2e09c3a2d7f1f5ff`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:1b6eaa1cdb9e60ac`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:f7d739ee4b8af61c`.
- [x] Final package contains `HANDOFF_README.txt`: hash `fnv64:fc14c98f579188f3`.
- [x] `handoff-instructions.adm` hash is `fnv64:d355821ba21fbd70`.
- [x] Current generated reports list `blocker_resolution_count=6`, matching `blocker_count=6`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Set a real provider secret such as `OPENAI_API_KEY` for the suggested OpenAI preset, or configure an equivalent named secret/provider.
- [ ] Run the AI blocker resolution command from `HANDOFF_README.txt` or `handoff-instructions.adm`.
- [ ] Run the Unity blocker resolution command with a compatible Unity editor until `runner=unity_playmode`.
- [ ] Run external acceptance and then the strict release gate, confirming final manifest `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [ ] Remaining engineering workload is still approximately 1-2% of the Rust rebuild. If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-014
**Summary**: Added concrete suggested external-acceptance commands to Rust handoff instructions and the final package README.

**Completed**:
- [x] Checked local external prerequisites: default Unity editor paths are absent, no real AI API key environment variables were found, `.adm_rust_data` contains only the mock provider, and `ai-doctor` reports only the mock provider as ready.
- [x] Added generated suggested command fields to `handoff-instructions.adm`: `suggested_ai_provider_preset`, `suggested_ai_secret_ref`, `suggested_ai_acceptance_command`, `suggested_ai_acceptance_invoke_command`, `suggested_unity_acceptance_command`, `suggested_external_acceptance_command`, `suggested_strict_release_gate_command`, and `suggested_strict_release_gate_ai_invoke_command`.
- [x] Forwarded those suggested commands into the final package `HANDOFF_README.txt`.
- [x] Added provider preset/secret inference and PowerShell argument rendering for generated handoff commands.
- [x] Updated adm-cli tests and `RUST/README.md`.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the suggested fields are present in source, source bundle, handoff bundle source copy, generated package README, handoff instructions, and source README.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:ade31f61342f58d2`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:882ab7f6718c60d0`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:6c3b3525607e76c5`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:a96e1c3398e34a22`.
- [x] Final package contains `HANDOFF_README.txt`: hash `fnv64:89dd02b0d7614940`.
- [x] `handoff-instructions.adm` hash is `fnv64:672802fba5db45a6`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains blocked in the local environment: `blocker_count=6`.
- [ ] Unity strict acceptance remains blocked because no default Unity editor path is available locally and strict acceptance requires `runner=unity_playmode`.
- [ ] Real AI provider acceptance remains blocked because only the mock provider is configured and no `OPENAI_API_KEY` or equivalent real provider secret is present.

**Remaining Work And Rough Time**:
- [ ] Configure a real provider secret, run the suggested AI acceptance command, and run the invoke variant if strict network-call proof is required.
- [ ] Install or point the gate to a compatible Unity editor and run the suggested Unity acceptance command until `runner=unity_playmode`.
- [ ] Run the suggested external acceptance command and then the suggested strict release gate command, optionally with `-RequireAiInvoke`.
- [ ] Remaining engineering workload is approximately 1-2% of the Rust rebuild. If Unity and AI credentials are ready, expected calendar time is about 2-5 hours. If Unity install/import/provider setup is still needed, expected calendar time is about 0.5-2 days.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves final delivery readiness.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-013
**Summary**: Made the final package `HANDOFF_README.txt` expose concrete blocker rows and external acceptance context.

**Completed**:
- [x] `handoff-instructions.adm` now forwards external acceptance context from existing reports: `external_acceptance_data_root`, `ai_acceptance_data_root`, `ai_provider_id`, `ai_provider_model`, `ai_diagnostic_readiness`, `unity_selected`, `unity_candidates`, `real_ai_provider_count`, and `ready_provider_count`.
- [x] `HANDOFF_README.txt` now forwards repeated `blocker=` rows from `handoff-instructions.adm`.
- [x] `HANDOFF_README.txt` now forwards key prerequisite state including `unity_runtime_runner`, `unity_selected`, `unity_candidates`, `ai_provider_id`, `ai_provider_model`, `ai_diagnostic_readiness`, `ai_configured_ready`, `real_ai_provider_ready`, `real_ai_provider_count`, and `ready_provider_count`.
- [x] Added adm-cli unit test assertions for the new generated README fields.
- [x] Updated `RUST/README.md` to document the top-level package entry file as a machine-readable prerequisite checklist.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package.

**Verification**:
- [x] `cargo fmt`: passed after rerunning outside the managed sandbox.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully.
- [x] Cross-layer check confirmed the new fields are present in source, source bundle, handoff bundle source copy, generated package README, handoff instructions, and source README.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:3e0bded7baa24cfb`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:7371caed0642acc1`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:5911c939fd34d226`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:ffdc905a0b311cf6`.
- [x] Final package contains `HANDOFF_README.txt`: hash `fnv64:7498d1f21dc4abd2`, listed as `required_file=HANDOFF_README.txt; present=true`.
- [x] `HANDOFF_README.txt` now lists all six current blockers: `external_acceptance_not_ready`, `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`, `ai_provider_acceptance_not_ready`, and `ai_provider_not_configured`.
- [x] `HANDOFF_README.txt` now records `unity_runtime_runner=cli_smoke_runner`, `unity_selected=none`, `unity_candidates=2`, `ai_provider_id=openai_main`, `ai_diagnostic_readiness=MissingProvider`, `ai_configured_ready=false`, `real_ai_provider_count=0`, and `ready_provider_count=1`.
- [x] `handoff-instructions.adm` remains ready with `instruction_count=8` and strict final manifest requirements.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains not ready in the local environment: `external_acceptance_ready=false`.
- [ ] Unity strict acceptance remains blocked: `unity_ready=false`, `unity_runtime_runner=cli_smoke_runner`, strict acceptance requires `unity_playmode`.
- [ ] Real AI provider acceptance remains blocked: `real_ai_provider_ready=false`, `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance. If endpoint/model/secret are ready: about 0.25-1 hour. If secrets/provider setup is not ready: about 0.5 day.
- [ ] If strict network-call proof is desired, run `scripts/ai_acceptance_gate.ps1 ... -Invoke -RequireReady -RequireInvoke`, then run strict external/release gate with `-RequireAiInvoke`.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine until `runner=unity_playmode`. If Unity is installed and compatible: about 1-3 hours. If install/import/build fixes are needed: about 0.5-1 day.
- [ ] Run final strict gate after both are ready: `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`, optionally adding `-RequireAiInvoke` for network-call proof. About 0.5-1 hour after prerequisites pass.
- [ ] Remaining engineering workload is still roughly 1-2% of the Rust rebuild. Calendar time is dominated by external Unity and real AI credential availability: about 2-5 hours if both prerequisites are ready, or about 0.5-2 days if setup/import/provider issues appear.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves the final manifest requirements listed in `HANDOFF_README.txt` and `handoff-instructions.adm`.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-012
**Summary**: Made the generated top-level `HANDOFF_README.txt` explicit about strict release gate working-directory context.

**Completed**:
- [x] Added `handoff_bundle_root_mode=package-inspection-and-evidence-entrypoint` to generated `HANDOFF_README.txt`.
- [x] Added `source_bundle_mode=source-audit-snapshot` and `source_bundle_scripts_path=source-bundle/scripts`.
- [x] Added `strict_gate_working_dir=rust-workspace-root-with-scripts-directory`.
- [x] Added `strict_gate_bundle_root_runnable=false`.
- [x] Added a context note warning that the strict gate must not be run from the handoff bundle root.
- [x] Updated `next_steps` to say strict gate reruns must happen from a Rust workspace root with scripts, generated delivery artifacts, matching DataRoot, Unity editor, and real AI credentials.
- [x] Added adm-cli test assertions for the new generated README fields.
- [x] Updated `RUST/README.md` to document the package root vs Rust workspace root distinction.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package.

**Verification**:
- [x] `cargo fmt`: passed after rerunning outside the managed sandbox because Windows denied rustfmt writeback in the sandbox.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate also ran `cargo fmt --check` and `cargo check --workspace` successfully during the final refresh.
- [x] Cross-layer check confirmed the new fields are present in source, source bundle, handoff bundle source copy, generated package README, and source README.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:802b363bf7ed2e56`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:550ad138368ddfb3`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:8490085c82c78c2a`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:40fde1d830569b9e`.
- [x] Final package contains `HANDOFF_README.txt`: hash `fnv64:9d24683ce4ae7a71`, listed as `required_file=HANDOFF_README.txt; present=true`.
- [x] `HANDOFF_README.txt` reports `strict_gate_working_dir=rust-workspace-root-with-scripts-directory`.
- [x] `HANDOFF_README.txt` reports `strict_gate_bundle_root_runnable=false`.
- [x] `HANDOFF_README.txt` reports `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`, `final_package_ready=true`, `final_delivery_ready=false`, `blocker_count=6`, and `instruction_count=8`.
- [x] `handoff-instructions.adm` remains ready with `instruction_count=8` and strict final manifest requirements.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains not ready in the local environment: `external_acceptance_ready=false`.
- [ ] Unity strict acceptance remains blocked: `unity_ready=false`, `unity_runtime_runner=cli_smoke_runner`, strict acceptance requires `unity_playmode`.
- [ ] Real AI provider acceptance remains blocked: `real_ai_provider_ready=false`, `ai_acceptance_ready=false`, `ai_configured_ready=false`.
- [x] `handoff-status.adm` still reports `blocker_count=6`: `external_acceptance_not_ready`, `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`, `ai_provider_acceptance_not_ready`, `ai_provider_not_configured`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance. If endpoint/model/secret are ready: about 0.25-1 hour. If secrets/provider setup is not ready: about 0.5 day.
- [ ] If strict network-call proof is desired, run `scripts/ai_acceptance_gate.ps1 ... -Invoke -RequireReady -RequireInvoke`, then run strict external/release gate with `-RequireAiInvoke`.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine until `runner=unity_playmode`. If Unity is installed and compatible: about 1-3 hours. If install/import/build fixes are needed: about 0.5-1 day.
- [ ] Run final strict gate after both are ready: `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`, optionally adding `-RequireAiInvoke` for network-call proof. About 0.5-1 hour after prerequisites pass.
- [ ] Current remaining engineering workload is roughly 1-2% of the Rust rebuild. Calendar time is dominated by external Unity and real AI credential availability: about 2-5 hours if both prerequisites are ready, or about 0.5-2 days if setup/import/provider issues appear.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves the final manifest requirements listed in `HANDOFF_README.txt` and `handoff-instructions.adm`.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-011
**Summary**: Added a generated top-level `HANDOFF_README.txt` entry file to the Rust final handoff bundle.

**Completed**:
- [x] Added `HANDOFF_README.txt` generation to `finalize-handoff-package`.
- [x] The handoff README points to the executable, bundled source README, evidence instructions, final manifest, strict gate commands, current readiness fields, blocker count, and instruction count.
- [x] Added `required_file=HANDOFF_README.txt; present=true` to generated `final-handoff-manifest.adm`.
- [x] Included `HANDOFF_README.txt` in final package file hashing and final manifest file listing.
- [x] Added adm-cli unit test assertions for the generated README, required file marker, final manifest listing, and readiness summary.
- [x] Updated `RUST/README.md` to document the top-level handoff entry file and final package requirement.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package.

**Verification**:
- [x] `cargo fmt`: passed after rerunning outside the sandbox because Windows denied rustfmt writeback in the managed sandbox.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Cross-layer check confirmed `HANDOFF_README.txt` handling is present in source, source bundle, handoff bundle source copy, generated package README, and final manifest.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:355a30abc22c9f8b`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:cea78c515c81f281`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:f3ef8d66fc2161bb`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:bf3d8780b83f6fd1`.
- [x] Final package now contains `HANDOFF_README.txt`: hash `fnv64:51a5340a6e615055`, listed as `required_file=HANDOFF_README.txt; present=true`.
- [x] `HANDOFF_README.txt` reports `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`, `final_package_ready=true`, `final_delivery_ready=false`, `blocker_count=6`, and `instruction_count=8`.
- [x] `handoff-instructions.adm` remains ready with `instruction_count=8` and strict final manifest requirements.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains not ready in the local environment: `external_acceptance_ready=false`.
- [ ] Unity strict acceptance remains blocked: `unity_ready=false`, `unity_runtime_runner=cli_smoke_runner`, strict acceptance requires `unity_playmode`.
- [ ] Real AI provider acceptance remains blocked: `real_ai_provider_ready=false`, `ai_acceptance_ready=false`, `ai_configured_ready=false`.
- [x] `handoff-status.adm` still reports `blocker_count=6`: `external_acceptance_not_ready`, `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`, `ai_provider_acceptance_not_ready`, `ai_provider_not_configured`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance. If endpoint/model/secret are ready: about 0.25-1 hour. If secrets/provider setup is not ready: about 0.5 day.
- [ ] If strict network-call proof is desired, run `scripts/ai_acceptance_gate.ps1 ... -Invoke -RequireReady -RequireInvoke`, then run strict external/release gate with `-RequireAiInvoke`.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine until `runner=unity_playmode`. If Unity is installed and compatible: about 1-3 hours. If install/import/build fixes are needed: about 0.5-1 day.
- [ ] Run final strict gate after both are ready: `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`, optionally adding `-RequireAiInvoke` for network-call proof. About 0.5-1 hour after prerequisites pass.
- [ ] Estimated remaining engineering workload is still roughly 1-2% of the Rust rebuild, with final risk concentrated in external Unity and real AI credentials.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves the final manifest requirements listed in `HANDOFF_README.txt` and `handoff-instructions.adm`.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-010
**Summary**: Made strict final delivery requirements machine-readable in `handoff-instructions.adm`.

**Completed**:
- [x] Added `strict_gate_requires_final_delivery=true` to generated `handoff-instructions.adm`.
- [x] Added `strict_gate_final_manifest_requires=package_ready,handoff_ready,delivery_ready` to generated `handoff-instructions.adm`.
- [x] Updated the `run-strict-release-gate` instruction note to state that the final gate must produce handoff-ready status and a final manifest whose package/handoff/delivery readiness fields are all ready.
- [x] Added adm-cli test assertions so the strict final delivery invariant stays covered.
- [x] Updated `RUST/README.md` to document the new machine-readable strict final delivery fields.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final package.

**Verification**:
- [x] `cargo fmt`: passed after rerunning outside the sandbox because Windows denied rustfmt writeback in the managed sandbox.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Cross-layer check confirmed the new strict final delivery fields are present in source, source bundle, release dist, and handoff evidence.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:47cf7ffda4cb74cb`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:151ee04f183c1d42`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:f3f5b374ede6a38a`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:1342e9251e61012c`.
- [x] `handoff-instructions.adm` remains ready with `instruction_count=8`.
- [x] `handoff-instructions.adm` now records `strict_gate_requires_final_delivery=true`.
- [x] `handoff-instructions.adm` now records `strict_gate_final_manifest_requires=package_ready,handoff_ready,delivery_ready`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains not ready in the local environment: `external_acceptance_ready=false`.
- [ ] Unity strict acceptance remains blocked: `unity_ready=false`, `unity_runtime_runner=cli_smoke_runner`, strict acceptance requires `unity_playmode`.
- [ ] Real AI provider acceptance remains blocked: `real_ai_provider_ready=false`, `ai_acceptance_ready=false`, `ai_configured_ready=false`.
- [x] `handoff-status.adm` still reports `blocker_count=6`: `external_acceptance_not_ready`, `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`, `ai_provider_acceptance_not_ready`, `ai_provider_not_configured`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance. If endpoint/model/secret are ready: about 0.25-1 hour. If secrets/provider setup is not ready: about 0.5 day.
- [ ] If strict network-call proof is desired, run `scripts/ai_acceptance_gate.ps1 ... -Invoke -RequireReady -RequireInvoke`, then run strict external/release gate with `-RequireAiInvoke`.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine until `runner=unity_playmode`. If Unity is installed and compatible: about 1-3 hours. If install/import/build fixes are needed: about 0.5-1 day.
- [ ] Run final strict gate after both are ready: `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`, optionally adding `-RequireAiInvoke` for network-call proof. About 0.5-1 hour after prerequisites pass.
- [ ] Estimated remaining engineering workload is still roughly 1-2% of the Rust rebuild, with final risk concentrated in external Unity and real AI credentials.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate proves the final manifest requirements listed in `handoff-instructions.adm`.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-009
**Summary**: Made `release_gate.ps1 -RequireExternalAcceptance` explicitly enforce final package delivery readiness.

**Completed**:
- [x] Added strict final handoff manifest assertions to `RUST/scripts/release_gate.ps1` under `-RequireExternalAcceptance`.
- [x] The strict release gate now requires `package_ready=true`, `handoff_ready=true`, and `delivery_ready=true`.
- [x] Kept the default local release gate behavior unchanged: it still prints final readiness fields without failing local-only runs when external acceptance is not ready.
- [x] Updated `RUST/README.md` to document that `-RequireExternalAcceptance` requires external acceptance, handoff status, and the refreshed final package manifest to be ready.
- [x] Regenerated source bundle, handoff bundle, evidence bundle, and final package.

**Verification**:
- [x] Simulated the new strict final expression against the current `final-handoff-manifest.adm`; it failed as expected because the current package has `handoff_ready=false` and `delivery_ready=false`.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Cross-layer check confirmed the strict final assertions are present in source and `dist/source-bundle/scripts/release_gate.ps1`.
- [x] Cross-layer check confirmed the README update is present in source and `dist/source-bundle/README.md`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:5c5acac53520b411`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:975f3d37adafd76c`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:d229f044bafbccfa`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:de9a920af339abae`.
- [x] `handoff-instructions.adm` remains ready with `instruction_count=8`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains not ready in the local environment: `external_acceptance_ready=false`.
- [ ] Unity strict acceptance remains blocked: `unity_ready=false`, `unity_runtime_runner=cli_smoke_runner`, strict acceptance requires `unity_playmode`.
- [ ] Real AI provider acceptance remains blocked: `real_ai_provider_ready=false`, `ai_acceptance_ready=false`, `ai_configured_ready=false`.
- [x] `handoff-status.adm` still reports `blocker_count=6`: `external_acceptance_not_ready`, `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`, `ai_provider_acceptance_not_ready`, `ai_provider_not_configured`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance. If endpoint/model/secret are ready: about 0.25-1 hour. If secrets/provider setup is not ready: about 0.5 day.
- [ ] If strict network-call proof is desired, run `scripts/ai_acceptance_gate.ps1 ... -Invoke -RequireReady -RequireInvoke`, then run strict external/release gate with `-RequireAiInvoke`.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine until `runner=unity_playmode`. If Unity is installed and compatible: about 1-3 hours. If install/import/build fixes are needed: about 0.5-1 day.
- [ ] Run final strict gate after both are ready: `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`, optionally adding `-RequireAiInvoke` for network-call proof. About 0.5-1 hour after prerequisites pass.
- [ ] Estimated remaining engineering workload is still roughly 1-2% of the Rust rebuild, with final risk concentrated in external Unity and real AI credentials.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and the strict release gate enforces `final-handoff-manifest.adm delivery_ready=true`.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-008
**Summary**: Made the final delivery endpoint explicit in Rust handoff instructions.

**Completed**:
- [x] Added final handoff manifest snapshot fields to `HandoffInstructionsReport`.
- [x] `handoff-instructions.adm` now records `final_handoff_manifest_report`, `final_handoff_manifest_present`, `final_package_ready`, `final_delivery_ready`, and `final_handoff_ready`.
- [x] Added machine-readable instruction `confirm-final-delivery-package`.
- [x] Added informational instruction `explain-package-vs-delivery-readiness` when the package is assembled but delivery is not accepted.
- [x] Kept `write-handoff-instructions ready=true` independent from final manifest existence so first clean release gate runs remain valid before `finalize-handoff-package` creates the manifest.
- [x] Updated `release_gate.ps1` to refresh handoff instructions/evidence after the first final package pass, then finalize the package again.
- [x] Updated `RUST/README.md` to document the final delivery endpoint and the two-pass final handoff refresh.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, final handoff manifest, and packaged evidence.

**Verification**:
- [x] `cargo fmt`: passed after rerunning outside the sandbox because Windows denied rustfmt writeback in the managed sandbox.
- [x] `cargo test -p adm-cli`: passed, 17 tests.
- [x] `cargo test --workspace`: passed.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate summary reports `instruction_count=8`, `final_package_ready=true`, `final_delivery_ready=false`, and `final_handoff_ready=false`.
- [x] Cross-layer check confirmed the new final delivery fields and instructions are present in source, source bundle, release dist, and handoff bundle evidence.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:3cdc994532598dbd`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:395e29143932789c`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:5f502c52a08a3a38`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:c36a5d071969f7d0`.
- [x] `handoff-instructions.adm` is ready and now has `instruction_count=8`.
- [x] The packaged evidence copy at `dist/handoff-bundle/evidence/handoff-instructions.adm` also records `final_handoff_manifest_present=true`, `final_package_ready=true`, and `final_delivery_ready=false`.
- [ ] Final delivery is not accepted yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains not ready in the local environment: `external_acceptance_ready=false`.
- [ ] Unity strict acceptance remains blocked: `unity_ready=false`, `unity_runtime_runner=cli_smoke_runner`, strict acceptance requires `unity_playmode`.
- [ ] Real AI provider acceptance remains blocked: `real_ai_provider_ready=false`, `ai_acceptance_ready=false`, `ai_configured_ready=false`.
- [x] `handoff-status.adm` still reports `blocker_count=6`: `external_acceptance_not_ready`, `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`, `ai_provider_acceptance_not_ready`, `ai_provider_not_configured`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance. If endpoint/model/secret are ready: about 0.25-1 hour. If secrets/provider setup is not ready: about 0.5 day.
- [ ] If strict network-call proof is desired, run `scripts/ai_acceptance_gate.ps1 ... -Invoke -RequireReady -RequireInvoke`, then run strict external/release gate with `-RequireAiInvoke`.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine until `runner=unity_playmode`. If Unity is installed and compatible: about 1-3 hours. If install/import/build fixes are needed: about 0.5-1 day.
- [ ] Run final strict gate after both are ready: `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`, optionally adding `-RequireAiInvoke` for network-call proof. About 0.5-1 hour after prerequisites pass.
- [ ] Estimated remaining engineering workload is still roughly 1-2% of the Rust rebuild, with final risk concentrated in external Unity and real AI credentials.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass, and `final-handoff-manifest.adm` reports `delivery_ready=true`.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-007
**Summary**: Clarified final package readiness by separating assembled package readiness from accepted delivery readiness.

**Completed**:
- [x] Added `FinalHandoffPackageReport::package_ready()`, `handoff_ready()`, `external_acceptance_ready()`, `ai_acceptance_ready()`, and `delivery_ready()`.
- [x] Added final manifest fields `package_ready`, `delivery_ready`, `handoff_ready`, `external_acceptance_ready`, and `ai_acceptance_ready`.
- [x] Preserved existing `ready=true` package assembly semantics so default local release gate remains usable.
- [x] Updated `release_gate.ps1` summary output to print `final_package_ready`, `final_delivery_ready`, and `final_handoff_ready`.
- [x] Updated `RUST/README.md` to document the distinction between package assembly readiness and full accepted delivery readiness.
- [x] Regenerated source bundle, handoff bundle, handoff evidence, and final handoff manifest.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 16 tests.
- [x] `cargo test --workspace`: passed.
- [x] `release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Release gate summary now prints `final_package_ready=true`, `final_delivery_ready=false`, and `final_handoff_ready=false`.
- [x] Cross-layer check confirmed the new readiness fields are present in source, source bundle, `dist/AutoDesignMaker-rust/final-handoff-manifest.adm`, and `dist/handoff-bundle/final-handoff-manifest.adm`.

**Current Handoff State**:
- [x] Local release remains ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:16a8cd28047b7832`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:e365589b15098bbb`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:cf374e4e07238add`.
- [x] Final handoff package artifact is assembled: `package_ready=true`, `package_hash=fnv64:eb6c6c25934d7dd4`.
- [ ] Final handoff package is not accepted for delivery yet: `delivery_ready=false`, `handoff_ready=false`, `external_acceptance_ready=false`, `ai_acceptance_ready=false`.
- [ ] External acceptance remains not ready in the local environment: `external_acceptance_ready=false`.
- [ ] Unity strict acceptance remains blocked: `unity_ready=false`, `unity_runtime_runner=cli_smoke_runner`, strict acceptance requires `unity_playmode`.
- [ ] Real AI provider acceptance remains blocked: `real_ai_provider_ready=false`, `ai_acceptance_ready=false`, `ai_configured_ready=false`.
- [x] `handoff-status.adm` still reports `blocker_count=6`: `external_acceptance_not_ready`, `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`, `ai_provider_acceptance_not_ready`, `ai_provider_not_configured`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance. If endpoint/model/secret are ready: about 0.25-1 hour. If secrets/provider setup is not ready: about 0.5 day.
- [ ] If strict network-call proof is desired, run `scripts/ai_acceptance_gate.ps1 ... -Invoke -RequireReady -RequireInvoke`, then run strict external/release gate with `-RequireAiInvoke`.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine until `runner=unity_playmode`. If Unity is installed and compatible: about 1-3 hours. If install/import/build fixes are needed: about 0.5-1 day.
- [ ] Run final strict gate after both are ready: `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`, optionally adding `-RequireAiInvoke` for network-call proof. About 0.5-1 hour after prerequisites pass.
- [ ] Estimated remaining engineering workload is still roughly 1-2% of the Rust rebuild, with final risk concentrated in external Unity and real AI credentials.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-006
**Summary**: Closed the strict AI invoke handoff gap by adding a dedicated real-network AI acceptance instruction and wrapper parameter constraints.

**Completed**:
- [x] Added `ai_invoke_attempted`, `ai_invoke_succeeded`, and `external_acceptance_require_ai_invoke` to `handoff-instructions.adm`.
- [x] Added a new machine-readable handoff instruction: `run-ai-provider-invoke-acceptance`.
- [x] The new instruction uses `ai_acceptance_gate.ps1 ... -Invoke -RequireReady -RequireInvoke` and points to `dist/AutoDesignMaker-rust/ai-acceptance.adm`.
- [x] The new instruction is required only when external acceptance was generated with `require_ai_invoke=true` and no successful invoke evidence exists; otherwise it is an optional `manual-decision` path.
- [x] Added wrapper validation so `-RequireAiInvoke` cannot be used without the matching strict readiness flag in `external_acceptance_doctor.ps1`, `release_gate.ps1`, and `unity_acceptance_gate.ps1`.
- [x] Updated `RUST/README.md` and regenerated source bundle, handoff bundle, evidence bundle, and final package.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 15 tests.
- [x] `cargo test --workspace`: passed.
- [x] Negative dry-runs confirmed `-RequireAiInvoke` misuse fails without `-RequireReady` / `-RequireExternalAcceptance`.
- [x] Positive dry-runs confirmed valid strict usage still prints `--require-ai-invoke`.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed and refreshed final package evidence.
- [x] Cross-layer check confirmed the new invoke instruction and wrapper constraints are present in source, source bundle, `dist/AutoDesignMaker-rust`, and `dist/handoff-bundle/evidence`.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:0595e54b8bbf3b8d`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:525296160046e9a5`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:be4691821d454c72`.
- [x] Final handoff package artifact is ready: `package_hash=fnv64:ce79566cc04855cf`.
- [x] `handoff-instructions.adm` is ready and now has `instruction_count=6`.
- [x] `handoff-instructions.adm` records `ai_invoke_attempted=false`, `ai_invoke_succeeded=false`, and `external_acceptance_require_ai_invoke=false` for the current default local report.
- [x] The new `run-ai-provider-invoke-acceptance` instruction is currently `required=false; status=manual-decision` because strict AI invoke is not enabled in the default external acceptance report.
- [ ] External acceptance is not ready in the default local environment: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Current imported runtime evidence is still CLI smoke evidence: `unity_runtime_runner=cli_smoke_runner`; strict acceptance requires `unity_playmode`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.
- [x] `handoff-status.adm` still reports `blocker_count=6`: external acceptance, Unity, real AI provider, and AI provider configuration blockers.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance: about 0.25-1 hour if endpoint, model, and secret are ready; about 0.5 day if credentials/provider setup is not ready.
- [ ] If strict network-call proof is desired, run `scripts/ai_acceptance_gate.ps1 ... -Invoke -RequireReady -RequireInvoke`, then run strict external/release gate with `-RequireAiInvoke`.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine: about 1-3 hours if Unity is compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run final strict gate `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`, optionally with `-RequireAiInvoke`: about 0.5-1 hour after Unity and real AI provider readiness both pass.
- [ ] Estimated remaining engineering workload is roughly 1-2% of the Rust rebuild, but final delivery still depends on external Unity and real AI credentials.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-005
**Summary**: Added an optional strict AI invocation requirement through Rust external acceptance, handoff status, PowerShell gates, generated handoff instructions, and README.

**Completed**:
- [x] Added `ai_acceptance_invoke_attempted`, `ai_acceptance_invoke_succeeded`, and `require_ai_invoke` to `ExternalAcceptanceReport`.
- [x] Added external blockers `ai_acceptance_invoke_not_attempted` and `ai_acceptance_invoke_not_succeeded` when `require_ai_invoke=true`.
- [x] Added CLI flag `external-acceptance --require-ai-invoke`.
- [x] Added final handoff field `external_acceptance_require_ai_invoke` and final blockers for missing/failed required AI invocation.
- [x] Added `-RequireAiInvoke` to `external_acceptance_doctor.ps1`, `release_gate.ps1`, and `unity_acceptance_gate.ps1`.
- [x] Added generated `strict_gate_ai_invoke_command` to `handoff-instructions.adm`.
- [x] Updated `RUST/README.md` and regenerated source bundle, handoff bundle, evidence bundle, and final package.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-packaging`: passed, 39 tests.
- [x] `cargo test -p adm-cli`: passed, 15 tests.
- [x] `cargo test --workspace`: passed.
- [x] Dry-ran `external_acceptance_doctor.ps1`, `release_gate.ps1`, and `unity_acceptance_gate.ps1` with `-RequireAiInvoke`; all printed `--require-ai-invoke`.
- [x] `scripts/ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -DataRoot .adm_rust_data`: regenerated `ai-acceptance.adm`.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed, including final package refresh.
- [x] Cross-layer check confirmed consistent `require_ai_invoke` semantics across source, scripts, README, generated dist reports, and handoff evidence.

**Current Handoff State**:
- [x] Local release is ready: `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_bundle_hash=fnv64:5a39d8148cc6a273`.
- [x] Unified handoff bundle is ready: `handoff_bundle_hash=fnv64:d77b04548264e4d2`.
- [x] Final evidence sync is ready: `evidence_hash=fnv64:c000b5cfc6a3df05`.
- [x] Final handoff package artifact is ready: `package_hash=fnv64:0ea19f562642a2ff`.
- [x] Default external acceptance records `require_ai_invoke=false`, `ai_acceptance_invoke_attempted=false`, and `ai_acceptance_invoke_succeeded=false`.
- [x] Default handoff status records `external_acceptance_require_ai_invoke=false`.
- [ ] External acceptance is not ready in the default local environment: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Current imported runtime evidence is still CLI smoke evidence: `unity_runtime_runner=cli_smoke_runner`; strict acceptance requires `unity_playmode`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.
- [x] `external-acceptance.adm` reports `blocker_count=5`: `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`, `ai_acceptance_not_ready`, `ai_acceptance_provider_not_configured`.
- [x] `handoff-status.adm` reports `blocker_count=6`: external acceptance, Unity, real AI provider, and AI provider configuration blockers.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance: about 0.25-1 hour if endpoint, model, and secret are ready; about 0.5 day if credentials/provider setup is not ready.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine: about 1-3 hours if Unity is compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run final strict gate `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`, optionally with `-RequireAiInvoke`: about 0.5-1 hour after Unity and real AI provider readiness both pass.
- [ ] Estimated remaining engineering workload is roughly 1-2% of the Rust rebuild, but it remains the final delivery acceptance blocker.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-004
**Summary**: Tightened final Rust external acceptance so it requires a ready/configured AI acceptance report whose provider is one of the real ready providers discovered by AI diagnostics.

**Completed**:
- [x] Added `ExternalAcceptanceReport::ai_acceptance_provider_matches_real_provider()`.
- [x] Added external acceptance blockers for missing AI acceptance report, AI acceptance not ready, AI provider not configured, and ready-provider mismatch.
- [x] Added `ai_acceptance_provider_matches_real_provider` to `external-acceptance.adm`.
- [x] Added `ai_acceptance_provider_matches_real_provider` to `handoff-status.adm`.
- [x] Added handoff blocker `ai_acceptance_provider_not_real_provider` when AI acceptance is ready/configured but the provider is not a real ready provider.
- [x] Updated `RUST/README.md` and regenerated source bundle, handoff bundle, evidence bundle, and final package.

**Verification**:
- [x] `cargo fmt`: passed through the approved Windows Rust/MSVC environment.
- [x] `cargo test -p adm-packaging`: passed, 38 tests.
- [x] `cargo test -p adm-cli`: passed, 14 tests.
- [x] `scripts/ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -DataRoot .adm_rust_data`: regenerated `ai-acceptance.adm` with the resolved data root.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed, including `cargo fmt --check`, `cargo check --workspace`, release acceptance, source bundle, handoff bundle, handoff evidence, and final package refresh.
- [x] Cross-layer check confirmed the new provider-match field and blockers are present in source, source bundle, `dist/AutoDesignMaker-rust`, and `dist/handoff-bundle/evidence`.

**Current Handoff State**:
- [x] Local release is ready: `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`, `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_ready=true`, `source_handoff_mode=bundled`, `source_file_count=55`, `source_bundle_hash=fnv64:6603ce6043d58390`.
- [x] Unified handoff bundle is ready: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:ee3151ddd81ade57`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `file_count=8`, `evidence_hash=fnv64:2666c0075badc714`.
- [x] Final handoff package artifact is ready: `final-handoff-manifest.adm ready=true`, `file_count=117`, `package_hash=fnv64:9b32e6706f52b35c`.
- [x] External and AI acceptance reports agree on `data_root=E:\workwork\CrewAi\AutoDesignMaker\RUST\.adm_rust_data`; `ai_acceptance_data_root_matches_external=true`.
- [ ] External acceptance is not ready in the default local environment: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Current imported runtime evidence is still CLI smoke evidence: `unity_runtime_runner=cli_smoke_runner`; strict acceptance requires `unity_playmode`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.
- [x] `external-acceptance.adm` now reports `blocker_count=5`: `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`, `ai_acceptance_not_ready`, `ai_acceptance_provider_not_configured`.
- [x] `handoff-status.adm` still reports `blocker_count=6`: external acceptance, Unity, real AI provider, and AI provider configuration blockers.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance: about 0.25-1 hour if endpoint, model, and secret are ready.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine: about 1-3 hours if Unity is compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run final strict gate `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`: about 0.5-1 hour after Unity and real AI provider readiness both pass.
- [ ] Estimated remaining engineering workload is still roughly 1-2%, but it remains the final delivery acceptance blocker.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-003
**Summary**: Strengthened the final Rust handoff evidence chain by recording AI/external acceptance data roots and making `handoff-status.adm` detect mismatched external/AI data roots.

**Completed**:
- [x] Added `data_root` to the `adm-cli ai-acceptance` report output.
- [x] Added `data_root` and current AI acceptance snapshot fields to `adm-packaging` external acceptance reports.
- [x] Passed the parsed external acceptance `data_root` from `adm-cli` into `run_external_acceptance`.
- [x] Added `external_acceptance_data_root`, `ai_acceptance_data_root`, and `ai_acceptance_data_root_matches_external` to `handoff-status.adm`.
- [x] Added final handoff blocker `ai_acceptance_data_root_mismatch` when both reports declare data roots and they disagree.
- [x] Updated `RUST/README.md` and regenerated source bundle, handoff bundle, evidence bundle, and final package.

**Verification**:
- [x] `cargo fmt`: passed through the approved Windows Rust/MSVC environment.
- [x] `cargo test -p adm-packaging`: passed, 37 tests.
- [x] `cargo test -p adm-cli`: passed, 13 tests.
- [x] `scripts/ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -DataRoot .adm_rust_data`: regenerated `ai-acceptance.adm` with the resolved data root.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild -DataRoot .adm_rust_data`: passed, including `cargo fmt --check`, `cargo check --workspace`, release acceptance, source bundle, handoff bundle, handoff evidence, and final package refresh.
- [x] Cross-layer check confirmed the new data-root fields are present in source, source bundle, `dist/AutoDesignMaker-rust`, and `dist/handoff-bundle/evidence`.

**Current Handoff State**:
- [x] Local release is ready: `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`, `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_ready=true`, `source_handoff_mode=bundled`, `source_file_count=55`, `source_bundle_hash=fnv64:491fb85ca374f0c6`.
- [x] Unified handoff bundle is ready: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:4c5554cb59e93ae2`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `file_count=8`, `evidence_hash=fnv64:68d6895a27f2c8c8`.
- [x] Final handoff package artifact is ready: `final-handoff-manifest.adm ready=true`, `file_count=117`, `package_hash=fnv64:bc43c1b67c936be2`.
- [x] External and AI acceptance reports agree on `data_root=E:\workwork\CrewAi\AutoDesignMaker\RUST\.adm_rust_data`; `ai_acceptance_data_root_matches_external=true`.
- [ ] External acceptance is not ready in the default local environment: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Current imported runtime evidence is still CLI smoke evidence: `unity_runtime_runner=cli_smoke_runner`; strict acceptance requires `unity_playmode`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.
- [x] `external-acceptance.adm` still reports `blocker_count=3`: `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`.
- [x] `handoff-status.adm` still reports `blocker_count=6`: external acceptance, Unity, real AI provider, and AI provider configuration blockers.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance: about 0.25-1 hour if endpoint, model, and secret are ready.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine: about 1-3 hours if Unity is compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run final strict gate `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`: about 0.5-1 hour after Unity and real AI provider readiness both pass.
- [ ] Estimated remaining engineering workload is still roughly 1-2%, but it remains the final delivery acceptance blocker.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-002
**Summary**: Propagated final Rust acceptance `DataRoot` handling across scripts, generated handoff instructions, source bundle, evidence bundle, and README so non-default AI provider data roots are preserved through strict external acceptance.

**Completed**:
- [x] Added `-DataRoot` support to `RUST/scripts/external_acceptance_doctor.ps1`.
- [x] Added `-DataRoot` support to `RUST/scripts/release_gate.ps1` and passed it as the third positional argument to `adm-cli external-acceptance`.
- [x] Added `-DataRoot` support to `RUST/scripts/unity_acceptance_gate.ps1` and reused it for final external acceptance.
- [x] Updated generated handoff instructions in `RUST/apps/adm-cli/src/main.rs` so AI acceptance, Unity acceptance, external acceptance, and strict release gate commands all include the same data root placeholder.
- [x] Updated `RUST/README.md` and regenerated source bundle, handoff bundle, evidence bundle, and final package.

**Verification**:
- [x] `cargo fmt`: passed through the approved Windows Rust/MSVC environment.
- [x] `cargo test -p adm-cli`: passed, 12 tests.
- [x] `scripts/external_acceptance_doctor.ps1 -DryRun -RequireReady -UnityExe C:\Unity\Editor\Unity.exe -DataRoot .\target\adm-final-data-root`: printed the resolved data root and positional external acceptance data root.
- [x] `scripts/release_gate.ps1 -DryRun -RequireExternalAcceptance -UnityExe C:\Unity\Editor\Unity.exe -DataRoot .\target\adm-final-data-root`: printed the resolved data root and propagated it into external acceptance.
- [x] `scripts/unity_acceptance_gate.ps1 -DryRun -ArchiveId archive_1783253820821_24440_1 -UnityExe C:\Unity\Editor\Unity.exe -DataRoot .\target\adm-final-data-root -RequireExternalAcceptance`: printed the resolved data root and propagated it into final external acceptance.
- [x] `scripts/ai_acceptance_gate.ps1 -DryRun -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot .\target\adm-final-data-root -RequireReady`: confirmed provider preset and AI acceptance share the same data root.
- [x] `scripts/release_gate.ps1 -SkipTests -SkipBuild`: passed and refreshed dist, evidence, source bundle, handoff bundle, and final package.
- [x] Cross-layer check confirmed the `DataRoot` contract is present in source scripts, source bundle scripts, README, generated handoff instructions, and evidence handoff instructions.

**Current Handoff State**:
- [x] Local release is ready: `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`, `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_ready=true`, `source_handoff_mode=bundled`, `source_file_count=55`, `source_bundle_hash=fnv64:836bf4c79ad36323`.
- [x] Unified handoff bundle is ready: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:b7bb0d81e0024a00`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `file_count=8`, `evidence_hash=fnv64:7adf3005b954c396`.
- [x] Final handoff package artifact is ready: `final-handoff-manifest.adm ready=true`, `file_count=117`, `package_hash=fnv64:b833ac5a04a66756`.
- [ ] External acceptance is not ready in the default local environment: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Current imported runtime evidence is still CLI smoke evidence: `unity_runtime_runner=cli_smoke_runner`; strict acceptance requires `unity_playmode`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.
- [x] `external-acceptance.adm` still reports `blocker_count=3`: `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance: about 0.25-1 hour if endpoint, model, and secret are ready.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe> -DataRoot <data_root>` on a real Unity machine: about 1-3 hours if Unity is compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run final strict gate `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe> -DataRoot <data_root>`: about 0.5-1 hour after Unity and real AI provider readiness both pass.
- [ ] Estimated remaining engineering workload is roughly 1-2%, but it remains the final delivery acceptance blocker.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity PlayMode acceptance and real AI provider acceptance both pass.

---

**Date**: 2026-07-06
**ID**: 2026-07-06-001
**Summary**: Added self-explaining blocker output to Rust external acceptance reports so `external-acceptance --require-ready` failures can be diagnosed from `external-acceptance.adm` itself.

**Completed**:
- [x] Added `ExternalAcceptanceReport::blockers()` in `RUST/crates/adm-packaging/src/lib.rs`.
- [x] Changed external acceptance readiness to derive from the blocker list.
- [x] Added `blocker_count` and repeated `blocker=` rows to `external-acceptance.adm`.
- [x] Updated `adm-cli external-acceptance --require-ready` so validation failures include the blocker list in the error message.
- [x] Added packaging test assertions for blocked, ready, and non-Unity-playmode runtime evidence cases.
- [x] Updated `RUST/README.md` to document the new external acceptance blocker rows.
- [x] Regenerated source bundle, handoff bundle, evidence bundle, and final handoff package.

**Verification**:
- [x] `cargo fmt`: passed through the approved Windows Rust/MSVC environment.
- [x] `cargo test -p adm-packaging`: passed, 37 tests.
- [x] `cargo test -p adm-cli`: passed, 12 tests.
- [x] `cargo run -q -p adm-cli -- external-acceptance --require-ready`: expected failure; error included `blockers=unity_not_ready,unity_runtime_runner_not_unity_playmode,real_ai_provider_not_ready`.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed after the expected failure check and refreshed consistent dist/evidence/package state.
- [x] Cross-layer check confirmed blocker output is present in source, source bundle, `dist/AutoDesignMaker-rust/external-acceptance.adm`, and `dist/handoff-bundle/evidence/external-acceptance.adm`.

**Current Handoff State**:
- [x] Local release is ready: `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`, `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_ready=true`, `source_handoff_mode=bundled`, `source_file_count=55`, `source_bundle_hash=fnv64:e22dc0742a9aa615`.
- [x] Unified handoff bundle is ready: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:a924a1ea9d46e0b7`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `file_count=8`, `evidence_hash=fnv64:e6e537565a8cae7f`.
- [x] Final handoff package artifact is ready: `final-handoff-manifest.adm ready=true`, `file_count=117`, `package_hash=fnv64:6bfef18d6f4d55d5`.
- [ ] External acceptance is not ready in the default local environment: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [x] `external-acceptance.adm` now reports `blocker_count=3`: `unity_not_ready`, `unity_runtime_runner_not_unity_playmode`, `real_ai_provider_not_ready`.
- [ ] Current imported runtime evidence is still CLI smoke evidence: `unity_runtime_runner=cli_smoke_runner`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance: about 0.25-1 hour if endpoint, model, and secret are ready.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe>` on a real Unity machine: about 1-3 hours if Unity is compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run final strict gate `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe>`: about 0.5-1 hour after Unity and real AI provider readiness both pass.
- [ ] Estimated remaining engineering workload is roughly 1-2%, but it remains the final delivery acceptance blocker.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity acceptance and real AI provider acceptance pass.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-125
**Summary**: Fixed explicit Unity executable path propagation so Unity acceptance, standalone external acceptance, and the final strict release gate can all use the same `Unity.exe`.

**Completed**:
- [x] Added `--unity-exe <path>` and `--unity-exe=<path>` support to `adm-cli external-acceptance`.
- [x] Added parser tests for explicit Unity executable handling.
- [x] Added `-UnityExe <path>` pass-through to `scripts/release_gate.ps1`.
- [x] Added `-UnityExe <path>` pass-through to `scripts/external_acceptance_doctor.ps1`.
- [x] Updated `scripts/unity_acceptance_gate.ps1` so its resolved Unity path is reused by final `external-acceptance`.
- [x] Updated generated handoff instructions and `RUST/README.md` so strict gate and rerun commands include explicit Unity path placeholders.
- [x] Regenerated source bundle, handoff bundle, evidence, and final handoff package.

**Verification**:
- [x] `cargo fmt`: passed through approved Windows Rust/MSVC environment.
- [x] `cargo test -p adm-cli`: passed, 12 tests.
- [x] `scripts/release_gate.ps1 -DryRun -RequireExternalAcceptance -UnityExe C:\Unity\Editor\Unity.exe`: printed external acceptance with `--unity-exe`.
- [x] `scripts/external_acceptance_doctor.ps1 -DryRun -RequireReady -UnityExe C:\Unity\Editor\Unity.exe`: printed external acceptance with `--unity-exe`.
- [x] `scripts/unity_acceptance_gate.ps1 -DryRun -ArchiveId archive_1783253820821_24440_1 -UnityExe C:\Unity\Editor\Unity.exe -RequireExternalAcceptance`: printed final external acceptance with `--unity-exe`.
- [x] Actual CLI proof with fake Unity path: `unity_ready=true`, `unity_selected=target\adm-test-unity\Editor\Unity.exe`.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed.
- [x] Cross-layer check completed for CLI, scripts, README, generated handoff instructions, source bundle, and evidence bundle.

**Current Handoff State**:
- [x] Local release is ready: `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`, `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_ready=true`, `source_handoff_mode=bundled`, `source_file_count=55`, `source_bundle_hash=fnv64:b36ed98bf835572e`.
- [x] Unified handoff bundle is ready: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:bbd20c7176e4ae2a`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `file_count=8`, `evidence_hash=fnv64:97009fbf5c859d70`.
- [x] Final handoff package artifact is ready: `final-handoff-manifest.adm ready=true`, `file_count=117`, `package_hash=fnv64:1a7a640c2f5556f6`.
- [ ] External acceptance is not ready in the default local environment: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Current imported runtime evidence is still CLI smoke evidence: `unity_runtime_runner=cli_smoke_runner`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance: about 0.25-1 hour if endpoint, model, and secret are ready.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -UnityExe <path-to-Unity.exe>` on a real Unity machine: about 1-3 hours if Unity is compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run final strict gate `scripts/release_gate.ps1 -RequireExternalAcceptance -UnityExe <path-to-Unity.exe>`: about 0.5-1 hour after Unity and real AI provider readiness both pass.
- [ ] Estimated remaining engineering workload is roughly 1-2%, but it remains the final delivery acceptance blocker.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity acceptance and real AI provider acceptance pass.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-124
**Summary**: Fixed the Unity runtime acceptance runner mismatch so real Unity validation can produce `runner=unity_playmode`, matching strict external acceptance.

**Completed**:
- [x] Audited `scripts/unity_acceptance_gate.ps1`, `adm-cli run-unity-runtime-validation`, archive import, Unity restaging, and `external-acceptance`.
- [x] Found the generated Unity validator wrote `runner=unity_editor_runtime_validation` while strict acceptance requires `runner=unity_playmode`.
- [x] Updated `RUST/crates/adm-packaging/src/lib.rs` so generated `AutoDesignMakerRuntimeValidation.cs` writes `runner=unity_playmode`.
- [x] Updated the packaging test expectation.
- [x] Restaged `dist/unity-project` from latest formal archive `archive_1783253820821_24440_1`.
- [x] Regenerated source bundle, handoff bundle, evidence, and final handoff package.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-packaging`: passed, 37 tests.
- [x] `cargo test -p adm-cli`: passed, 10 tests.
- [x] `cargo run -q -p adm-cli -- stage-unity-project archive_1783253820821_24440_1 windows_desktop_playable dist\unity-project`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: confirmed strict gate order.
- [x] Cross-layer check confirmed the updated `runner=unity_playmode` is present in source, `dist/unity-project`, `dist/handoff-bundle/unity-project`, and `dist/handoff-bundle/source-bundle`.

**Current Handoff State**:
- [x] Local release is ready: `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`, `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_ready=true`, `source_handoff_mode=bundled`, `source_file_count=55`, `source_bundle_hash=fnv64:cfe921b299fb11e4`.
- [x] Unified handoff bundle is ready: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:d5e7f8f1c7a3b322`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `file_count=8`, `evidence_hash=fnv64:f5d47a5be5f90f81`.
- [x] Final handoff package artifact is ready: `final-handoff-manifest.adm ready=true`, `file_count=117`, `package_hash=fnv64:acabbc4f84bb3eab`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Current imported runtime evidence is still CLI smoke evidence: `unity_runtime_runner=cli_smoke_runner`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance: about 0.25-1 hour if endpoint, model, and secret are ready.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed: about 1-3 hours if Unity is compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run final strict gate `scripts/release_gate.ps1 -RequireExternalAcceptance`: about 0.5-1 hour after Unity and AI readiness both pass.
- [ ] Estimated remaining engineering workload is roughly 1-3%, but it is still the blocking final delivery acceptance work.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity acceptance and real AI provider acceptance pass.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-123
**Summary**: Tightened Rust external acceptance so final completion requires imported Unity `unity_playmode` runtime execution evidence, not only CLI smoke evidence.

**Completed**:
- [x] Added Unity runtime evidence fields to `RUST/crates/adm-packaging/src/lib.rs` external acceptance reports.
- [x] External acceptance readiness now requires `unity_runtime_present=true`, `unity_runtime_ready=true`, and `unity_runtime_runner=unity_playmode`.
- [x] Updated `RUST/apps/adm-cli/src/main.rs` handoff status and handoff instructions to expose runtime evidence fields and blockers.
- [x] Added blocker `unity_runtime_runner_not_unity_playmode`.
- [x] Updated `RUST/README.md` to document imported Unity playmode runtime evidence as required.
- [x] Regenerated source bundle, handoff bundle, handoff instructions, evidence manifest, and final handoff package.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-packaging`: passed, 37 tests.
- [x] `cargo test -p adm-cli`: passed, 10 tests.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: confirmed strict gate order still blocks at external acceptance before final ready status.
- [x] Cross-layer check completed for packaging report generation, CLI handoff status/instructions, README, generated evidence, and final package manifests.

**Current Handoff State**:
- [x] Local release is ready: `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`, `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready: `source_ready=true`, `source_handoff_mode=bundled`, `source_file_count=55`, `source_bundle_hash=fnv64:12cf412620999dc6`.
- [x] Source handoff policy is ready: `source_handoff_policy=bundled-source-bundle-is-current-delivery-evidence`.
- [x] Unified handoff bundle is ready: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:399aefc68c4ff197`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `file_count=8`, `evidence_hash=fnv64:567813c02f7e17ec`.
- [x] Final handoff package artifact is ready: `final-handoff-manifest.adm ready=true`, `file_count=117`, `package_hash=fnv64:4b4f35d54d050655`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Unity runtime evidence is present but not sufficient for final acceptance: `unity_runtime_present=true`, `unity_runtime_ready=true`, `unity_runtime_runner=cli_smoke_runner`.
- [ ] Explicit AI provider acceptance is not ready on this machine: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Remaining Work And Rough Time**:
- [ ] Configure and run real non-mock AI provider acceptance: about 0.25-1 hour if endpoint, model, and secret are ready.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed and refresh imported `unity_playmode` runtime evidence: about 1-3 hours if Unity is compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run the strict final gate `scripts/release_gate.ps1 -RequireExternalAcceptance`: about 0.5-1 hour after Unity and real AI provider readiness both pass.
- [ ] Estimated remaining workload: roughly 2-4% of the Rust rebuild by engineering scope, but it is the blocking 100% of final delivery acceptance.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity acceptance and real AI provider acceptance pass.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-122
**Summary**: Closed the Rust real-AI acceptance setup gap by extending the AI acceptance gate script so it can configure a non-mock provider before writing the redacted acceptance report.

**Completed**:
- [x] Extended `RUST/scripts/ai_acceptance_gate.ps1` with `-Preset`, `-Endpoint`, `-SecretRef`, and `-DataRoot`.
- [x] The script can now dry-run or execute `ai-provider-preset` before `ai-acceptance`.
- [x] The script can now dry-run or execute `ai-provider-set` for custom OpenAI-compatible endpoints before `ai-acceptance`.
- [x] The script prints command lines and secret references, but not secret values.
- [x] Updated generated handoff instructions so `configure-real-ai-provider` points to the wrapper script instead of a raw `adm-cli ai-acceptance` command.
- [x] Updated `RUST/README.md` with preset and endpoint examples for the AI acceptance wrapper.
- [x] Regenerated source bundle, handoff bundle, handoff instructions, evidence manifest, and final handoff package.

**Verification**:
- [x] AI acceptance wrapper preset dry-run: passed.
- [x] AI acceptance wrapper endpoint dry-run: passed.
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-cli`: passed, 10 tests.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed and regenerated current handoff reports.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: confirmed strict gate order remains intact.
- [x] Cross-layer check completed for script, CLI generated handoff instruction, README, and generated evidence package.

**Current Handoff State**:
- [x] Local release is ready: `local_release_ready=true`, `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`, `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready and clean: `source_ready=true`, `source_handoff_mode=bundled`, `source_file_count=55`, `source_bundle_hash=fnv64:7b65b6d57d81931c`.
- [x] Source handoff policy is ready: `source-handoff-policy.adm ready=true`, `source_handoff_policy=bundled-source-bundle-is-current-delivery-evidence`.
- [x] Unified handoff bundle is ready: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:f3c4d02b5f15fdae`.
- [x] Handoff instructions are ready and include the AI acceptance setup wrapper command: `handoff-instructions.adm ready=true`, `instruction_count=5`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `file_count=8`, `evidence_hash=fnv64:2254a30cd917f06d`.
- [x] Final handoff package is ready: `final-handoff-manifest.adm ready=true`, `file_count=117`, `package_hash=fnv64:5b2dcb25c888f89b`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Explicit AI provider acceptance is not ready on this machine because no non-mock provider secret is configured: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Remaining Work And Rough Time**:
- [ ] Run the AI acceptance wrapper with real credentials: about 0.25-1 hour if the environment variable or named secret is ready.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed and refresh real runtime execution evidence: about 1-3 hours if Unity is already installed and compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run the strict final gate `scripts/release_gate.ps1 -RequireExternalAcceptance`: about 0.5-1 hour after Unity and real AI provider readiness both pass.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity acceptance and real AI provider acceptance pass.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-121
**Summary**: Added a machine-readable Rust source handoff policy so the generated source bundle is explicitly accepted as current package handoff evidence.

**Completed**:
- [x] Added `adm-cli write-source-handoff-policy [release_dir] [bundle_dir] [report_path]`.
- [x] The command reads `source-manifest.adm` and `handoff-bundle-manifest.adm`.
- [x] It verifies that `dist/source-bundle` exists, was copied into `dist/handoff-bundle/source-bundle`, and matches the source bundle hash.
- [x] It writes `dist/AutoDesignMaker-rust/source-handoff-policy.adm` with `source_handoff_policy=bundled-source-bundle-is-current-delivery-evidence`.
- [x] `write-handoff-instructions` now marks `decide-source-handoff-policy` as ready when the policy report is ready.
- [x] `sync-handoff-evidence` now copies `source-handoff-policy.adm` into `dist/handoff-bundle/evidence`.
- [x] `stage-handoff-bundle` excludes stale `source-handoff-policy.adm` from the nested desktop release copy.
- [x] `release_gate.ps1` now runs and validates `write-source-handoff-policy`.
- [x] Updated `RUST/README.md` with the source handoff policy workflow.

**Verification**:
- [x] `cargo fmt`: passed through the approved Windows Rust/MSVC environment.
- [x] `cargo test -p adm-cli`: passed, 10 tests.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed and regenerated current handoff reports.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: confirmed the strict gate order runs `write-source-handoff-policy` before external acceptance and handoff instructions.
- [x] Cross-layer check completed for CLI command, release gate script, README, generated source policy report, handoff instructions, evidence manifest, and final package manifest.
- [x] Confirmed `dist/handoff-bundle/AutoDesignMaker-rust/source-handoff-policy.adm` is absent, while `dist/handoff-bundle/evidence/source-handoff-policy.adm` is present.

**Current Handoff State**:
- [x] Local release is ready: `local_release_ready=true`, `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`, `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready and clean: `source_ready=true`, `source_handoff_mode=bundled`, `source_file_count=55`, `source_bundle_hash=fnv64:242a36cf9cb7d19f`.
- [x] Source handoff policy is ready: `source-handoff-policy.adm ready=true`, `source_handoff_policy=bundled-source-bundle-is-current-delivery-evidence`, `parent_repo_commit_required_for_package_ready=false`.
- [x] Unified handoff bundle is ready and excludes stale final-gate reports, including `source-handoff-policy.adm`: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:8493e5beea902093`.
- [x] Handoff instructions are ready: `handoff-instructions.adm ready=true`, `source_policy_ready=true`, `instruction_count=5`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `file_count=8`, `evidence_hash=fnv64:79af195547950b0b`.
- [x] Final handoff package is ready: `final-handoff-manifest.adm ready=true`, `file_count=117`, `package_hash=fnv64:527485a2b76ab59c`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Remaining Work And Rough Time**:
- [ ] Configure a non-mock AI provider and rerun `ai-acceptance --require-ready`: about 0.5-1 hour if endpoint/model/secret are ready, longer if network or provider config needs debugging.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed and refresh real runtime execution evidence: about 1-3 hours if Unity is already installed and compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run the strict final gate `scripts/release_gate.ps1 -RequireExternalAcceptance`: about 0.5-1 hour after Unity and real AI provider readiness both pass.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity acceptance and real AI provider acceptance pass.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-120
**Summary**: Added machine-readable Rust handoff instructions so the final package carries explicit external acceptance follow-up steps.

**Completed**:
- [x] Added `adm-cli write-handoff-instructions [release_dir] [report_path]`.
- [x] The command reads `handoff-status.adm`, `external-acceptance.adm`, and `ai-acceptance.adm`.
- [x] It writes `dist/AutoDesignMaker-rust/handoff-instructions.adm` with current blockers, strict gate command, required evidence files, command lines, and rough estimates.
- [x] `sync-handoff-evidence` now copies `handoff-instructions.adm` into `dist/handoff-bundle/evidence`.
- [x] `stage-handoff-bundle` now excludes stale `handoff-instructions.adm` from the nested desktop release copy, matching the other final gate reports.
- [x] `release_gate.ps1` now runs and validates `write-handoff-instructions`, then prints `instruction_count`.
- [x] Updated `RUST/README.md` with the handoff instructions workflow.

**Verification**:
- [x] `cargo fmt`: passed through the approved Windows Rust/MSVC environment.
- [x] `cargo test -p adm-cli`: passed, 9 tests.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed; includes `cargo fmt --check`, `cargo check --workspace`, local release acceptance, source bundle, handoff bundle, handoff status, handoff instructions, evidence sync, and final package generation.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: confirmed the strict gate order includes `write-handoff-instructions` before evidence sync.
- [x] Cross-layer check completed for CLI command, release gate script, README, generated `handoff-instructions.adm`, evidence manifest, and final package manifest.
- [x] Confirmed `dist/handoff-bundle/AutoDesignMaker-rust/handoff-instructions.adm` is absent, while `dist/handoff-bundle/evidence/handoff-instructions.adm` is present.

**Current Handoff State**:
- [x] Local release is ready: `local_release_ready=true`, `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`, `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready and clean: `source_ready=true`, `source_file_count=55`, `source_bundle_hash=fnv64:2f2054a4a365fc81`, `stale_cleanup=removed_existing_bundle_dir`.
- [x] Unified handoff bundle is ready and excludes stale final-gate reports, including `handoff-instructions.adm`: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:95145a1866d28b03`.
- [x] Handoff instructions are ready: `handoff-instructions.adm ready=true`, `instruction_count=5`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `file_count=7`, `evidence_hash=fnv64:16ec3bb19926c16b`.
- [x] Final handoff package is ready: `final-handoff-manifest.adm ready=true`, `file_count=116`, `package_hash=fnv64:9ca90c5683c47e6f`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Remaining Work And Rough Time**:
- [ ] Configure a non-mock AI provider and rerun `ai-acceptance --require-ready`: about 0.5-1 hour if endpoint/model/secret are ready, longer if network or provider config needs debugging.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed and refresh real runtime execution evidence: about 1-3 hours if Unity is already installed and compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run the strict final gate `scripts/release_gate.ps1 -RequireExternalAcceptance`: about 0.5-1 hour after Unity and real AI provider readiness both pass.
- [ ] Decide source handoff strategy for the untracked `RUST/` tree versus generated `dist/source-bundle`: about 0.5-2 hours depending on whether parent repository cleanup is required.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity acceptance and real AI provider acceptance pass.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-119
**Summary**: Added the final Rust handoff package manifest and fixed stale final manifest leakage in the staged handoff bundle.

**Completed**:
- [x] Added `adm-cli finalize-handoff-package [bundle_dir] [report_path]`.
- [x] The command validates `handoff-bundle-manifest.adm`, `evidence/handoff-evidence-manifest.adm`, and required `AutoDesignMaker-rust`, `source-bundle`, and `evidence` directories.
- [x] The command writes `final-handoff-manifest.adm` to both `dist/AutoDesignMaker-rust` and the root of `dist/handoff-bundle`.
- [x] The final package manifest computes a deterministic package hash over the complete handoff bundle after evidence sync, excluding only the root `final-handoff-manifest.adm`.
- [x] Updated `release_gate.ps1` to run `finalize-handoff-package`, validate `ready=true`, and print `package_hash`.
- [x] Fixed `stage-handoff-bundle` to exclude stale `final-handoff-manifest.adm` from the nested desktop release copy.
- [x] Updated `RUST/README.md` with the final handoff package workflow and stale final manifest exclusion.

**Verification**:
- [x] `cargo fmt`: passed after rerunning through the approved Windows Rust/MSVC environment because the direct sandboxed rustfmt write was denied by Windows permissions.
- [x] `cargo test -p adm-cli`: passed, 8 tests.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed; includes `cargo fmt --check`, `cargo check --workspace`, local release acceptance, source bundle, handoff bundle, evidence sync, and final package generation.
- [x] Final package is ready: `final-handoff-manifest.adm ready=true`, `package_hash=fnv64:7b524ba82e905e8d`.
- [x] Confirmed nested `dist/handoff-bundle/AutoDesignMaker-rust/final-handoff-manifest.adm` is absent and root `dist/handoff-bundle/final-handoff-manifest.adm` is present.
- [x] `cargo run -q -p adm-cli -- handoff-status --require-ready`: failed as expected because external Unity and real AI provider readiness are not satisfied.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: confirmed final strict gate order still runs external acceptance and handoff status with `--require-ready`, then evidence sync and final package generation.
- [x] Cross-layer check completed for CLI command, release gate script, README, and generated handoff reports.

**Current Handoff State**:
- [x] Local release is ready: `local_release_ready=true`, `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`, `release_hash=fnv64:97814f3263c20abf`.
- [x] Source handoff evidence is ready and clean: `source_ready=true`, `source_file_count=55`, `source_bundle_hash=fnv64:da1d6617a7590161`, `stale_cleanup=removed_existing_bundle_dir`.
- [x] Unified handoff bundle is ready and excludes stale final-gate reports, including `final-handoff-manifest.adm`: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:c912a3a09c693245`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `evidence_hash=fnv64:21f53480bb3a1df7`.
- [x] Final handoff package is ready: `final-handoff-manifest.adm ready=true`, `package_hash=fnv64:7b524ba82e905e8d`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Remaining Work And Rough Time**:
- [ ] Configure a non-mock AI provider and rerun `ai-acceptance --require-ready`: about 0.5-1 hour if endpoint/model/secret are ready, longer if network or provider config needs debugging.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed and refresh real runtime execution evidence: about 1-3 hours if Unity is already installed and compatible; about 0.5-1 day if Unity install/import fixes are needed.
- [ ] Run the strict final gate `scripts/release_gate.ps1 -RequireExternalAcceptance`: about 0.5-1 hour after Unity and real AI provider readiness both pass.
- [ ] Decide source handoff strategy for the untracked `RUST/` tree versus generated `dist/source-bundle`: about 0.5-2 hours depending on whether parent repository cleanup is required.

**Follow-up**:
- [ ] Keep the full project completion goal active until external Unity acceptance and real AI provider acceptance pass.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-118
**Summary**: Added final handoff evidence syncing so the Rust handoff bundle carries current gate reports without copying stale reports into the nested desktop release.

**Completed**:
- [x] Added `adm-cli sync-handoff-evidence [release_dir] [bundle_dir] [report_path]`.
- [x] The command recreates `dist/handoff-bundle/evidence` and copies current final reports from `dist/AutoDesignMaker-rust`.
- [x] Evidence now includes `release-acceptance.adm`, `source-manifest.adm`, `handoff-bundle-manifest.adm`, `external-acceptance.adm`, `ai-acceptance.adm`, and `handoff-status.adm`.
- [x] Added `handoff-evidence-manifest.adm` in both `dist/AutoDesignMaker-rust` and `dist/handoff-bundle/evidence`.
- [x] Updated `stage-handoff-bundle` to also exclude stale `handoff-evidence-manifest.adm` from the nested desktop release copy.
- [x] Updated `release_gate.ps1` to run `sync-handoff-evidence` after `handoff-status`, validate the evidence manifest, and print `evidence_hash`.
- [x] Updated `RUST/README.md` with the final evidence sync workflow.

**Verification**:
- [x] `cargo test -p adm-cli`: passed, including 7 CLI tests.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed and rewrote `source-manifest.adm`.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed and excluded stale final-gate reports.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: passed and reported expected local blockers.
- [x] `cargo run -q -p adm-cli -- handoff-status`: passed and reported `handoff_bundle_ready=true`.
- [x] `cargo run -q -p adm-cli -- sync-handoff-evidence`: passed and wrote evidence reports.
- [x] `cargo run -q -p adm-cli -- handoff-status --require-ready`: failed as expected because external Unity and real AI provider are not ready.
- [x] Confirmed nested `dist/handoff-bundle/AutoDesignMaker-rust` does not contain `external-acceptance.adm`, `handoff-status.adm`, `handoff-bundle-manifest.adm`, or `handoff-evidence-manifest.adm`.
- [x] Confirmed `dist/handoff-bundle/evidence` contains the six final report files plus `handoff-evidence-manifest.adm`.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: listed `sync-handoff-evidence` after `handoff-status`.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed and printed `evidence_hash=fnv64:86ea25bd81a83096`.
- [x] Cross-layer check completed for CLI evidence sync -> release gate script -> README -> `handoff-evidence-manifest.adm` -> `handoff-status.adm`.

**Current Handoff State**:
- [x] Local release is ready: `local_release_ready=true`, `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`.
- [x] Source handoff evidence is ready and clean: `source_ready=true`, `source_file_count=55`, `source_bundle_hash=fnv64:1f48c28cafa51c68`, `stale_cleanup=removed_existing_bundle_dir`.
- [x] Unified handoff bundle is ready and excludes stale final-gate reports: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:a6e7d82efa6d8b11`.
- [x] Final evidence sync is ready: `handoff-evidence-manifest.adm ready=true`, `evidence_hash=fnv64:86ea25bd81a83096`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Follow-up**:
- [ ] Configure a non-mock provider with endpoint/model/secret and rerun `ai-acceptance --require-ready`.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed to produce real `runtime_execution_results.adm` and refreshed delivery reports.
- [ ] Run `scripts/release_gate.ps1 -RequireExternalAcceptance` only after Unity and real AI provider readiness both pass.
- [ ] Decide whether generated bundle handoff evidence is sufficient or whether `RUST/` must also be committed in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-117
**Summary**: Prevented stale final-gate reports from being copied into the Rust handoff bundle.

**Completed**:
- [x] Updated `adm-cli stage-handoff-bundle` to exclude final-gate reports from the copied `AutoDesignMaker-rust` release directory.
- [x] Excluded `external-acceptance.adm`, `handoff-status.adm`, and stale `handoff-bundle-manifest.adm` from the nested release copy because those reports are updated after bundle staging.
- [x] Updated `handoff-bundle-manifest.adm` to list `excluded_file=...` entries for the omitted reports.
- [x] Added unit coverage proving those stale reports are not copied into the handoff bundle.
- [x] Updated `RUST/README.md` to document the exclusion behavior.

**Verification**:
- [x] `cargo test -p adm-cli`: passed, including 6 CLI tests.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed and rewrote `source-manifest.adm`.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed and wrote a clean bundle manifest with excluded report lines.
- [x] `cargo run -q -p adm-cli -- handoff-status`: passed and reported `handoff_bundle_ready=true`.
- [x] `cargo run -q -p adm-cli -- handoff-status --require-ready`: failed as expected because external Unity and real AI provider are not ready.
- [x] Confirmed `dist/handoff-bundle/AutoDesignMaker-rust/external-acceptance.adm`, `handoff-status.adm`, and `handoff-bundle-manifest.adm` are absent.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: listed the expected release gate order.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed and printed `handoff_bundle_hash=fnv64:5c5d67831b43bf86`.
- [x] Cross-layer check completed for CLI exclusion logic -> README -> generated `handoff-bundle-manifest.adm` -> `handoff-status.adm`.

**Current Handoff State**:
- [x] Local release is ready: `local_release_ready=true`, `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`.
- [x] Source handoff evidence is ready and clean: `source_ready=true`, `source_file_count=55`, `source_bundle_hash=fnv64:98efd891ff8305e9`, `stale_cleanup=removed_existing_bundle_dir`.
- [x] Unified handoff bundle is ready and excludes stale final-gate reports: `handoff_bundle_ready=true`, `handoff_bundle_file_count=107`, `handoff_bundle_hash=fnv64:5c5d67831b43bf86`, `stale_cleanup=removed_existing_bundle_dir`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Follow-up**:
- [ ] Configure a non-mock provider with endpoint/model/secret and rerun `ai-acceptance --require-ready`.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed to produce real `runtime_execution_results.adm` and refreshed delivery reports.
- [ ] Run `scripts/release_gate.ps1 -RequireExternalAcceptance` only after Unity and real AI provider readiness both pass.
- [ ] Decide whether generated bundle handoff evidence is sufficient or whether `RUST/` must also be committed in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-116
**Summary**: Added a deterministic Rust handoff bundle that gathers generated delivery outputs into one final local handoff directory.

**Completed**:
- [x] Added `adm-cli stage-handoff-bundle [dist_root] [bundle_dir] [report_path]`.
- [x] The command recreates `RUST/dist/handoff-bundle` before copying current delivery outputs.
- [x] Required handoff directories are `AutoDesignMaker-rust` and `source-bundle`; optional `game-build`, `sdk-bundle`, and `unity-project` directories are included when present.
- [x] Added path guards so the handoff bundle cannot be the dist root, a parent of dist root, or overlap a source delivery directory.
- [x] Added `handoff-bundle-manifest.adm` with file counts, bytes, per-directory hashes, aggregate bundle hash, and stale cleanup state.
- [x] Updated `handoff-status` to read `handoff-bundle-manifest.adm` and report `handoff_bundle_ready`, `handoff_bundle_file_count`, and `handoff_bundle_hash`.
- [x] Updated `release_gate.ps1` to run `stage-handoff-bundle`, validate the manifest, and print `handoff_bundle_hash`.
- [x] Updated `RUST/README.md` with the new handoff bundle workflow.

**Verification**:
- [x] `cargo test -p adm-cli`: passed, including 6 CLI tests.
- [x] `cargo run -q -p adm-cli -- stage-handoff-bundle`: passed and wrote `handoff-bundle-manifest.adm`.
- [x] `cargo run -q -p adm-cli -- handoff-status`: passed and reported `handoff_bundle_ready=true`.
- [x] `cargo run -q -p adm-cli -- handoff-status --require-ready`: failed as expected because external Unity and real AI provider are not ready.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: listed `stage-handoff-bundle` before external acceptance and `handoff-status --require-ready`.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed and printed `handoff_bundle_hash=fnv64:f829bd3b59e95ce4`.
- [x] Cross-layer check completed for CLI command -> release gate script -> README -> `handoff-bundle-manifest.adm` -> `handoff-status.adm`.

**Current Handoff State**:
- [x] Local release is ready: `local_release_ready=true`, `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`.
- [x] Source handoff evidence is ready and clean: `source_ready=true`, `source_file_count=55`, `source_bundle_hash=fnv64:cd1b955b3ef54e3a`, `stale_cleanup=removed_existing_bundle_dir`.
- [x] Unified handoff bundle is ready: `handoff_bundle_ready=true`, `handoff_bundle_file_count=110`, `handoff_bundle_hash=fnv64:f829bd3b59e95ce4`, `stale_cleanup=removed_existing_bundle_dir`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Follow-up**:
- [ ] Configure a non-mock provider with endpoint/model/secret and rerun `ai-acceptance --require-ready`.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed to produce real `runtime_execution_results.adm` and refreshed delivery reports.
- [ ] Run `scripts/release_gate.ps1 -RequireExternalAcceptance` only after Unity and real AI provider readiness both pass.
- [ ] Decide whether generated bundle handoff evidence is sufficient or whether `RUST/` must also be committed in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-115
**Summary**: Made the Rust source bundle deterministic by safely cleaning stale handoff files before copying.

**Completed**:
- [x] Updated `adm-cli stage-source-bundle` to recreate the source bundle directory before copying current Rust source files.
- [x] Added a safety guard so the bundle directory cannot be the source root or a parent of the source root.
- [x] Updated `source-manifest.adm` to report `stale_cleanup=removed_existing_bundle_dir` or `created_clean_bundle_dir`.
- [x] Added unit coverage for stale source cleanup and dangerous bundle path rejection.
- [x] Updated `RUST/README.md` to document stale cleanup behavior.

**Verification**:
- [x] `cargo test -p adm-cli`: passed, including 4 CLI tests.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed and rewrote `source-manifest.adm`.
- [x] `cargo run -q -p adm-cli -- handoff-status`: passed and reported `source_ready=true`.
- [x] `cargo run -q -p adm-cli -- handoff-status --require-ready`: failed as expected because external Unity and real AI provider are not ready.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed and printed `source_bundle_hash=fnv64:7688d71b937d7f8a`.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: listed `stage-source-bundle` and `handoff-status --require-ready`.
- [x] Cross-layer check completed for CLI source cleanup -> README -> release gate -> `source-manifest.adm` -> `handoff-status.adm`.

**Current Handoff State**:
- [x] Local release is ready: `local_release_ready=true`, `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`.
- [x] Source handoff evidence is ready and clean: `source_ready=true`, `source_file_count=55`, `source_bundle_hash=fnv64:7688d71b937d7f8a`, `stale_cleanup=removed_existing_bundle_dir`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Follow-up**:
- [ ] Configure a non-mock provider with endpoint/model/secret and rerun `ai-acceptance --require-ready`.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed to produce real `runtime_execution_results.adm` and refreshed delivery reports.
- [ ] Run `scripts/release_gate.ps1 -RequireExternalAcceptance` only after Unity and real AI provider readiness both pass.
- [ ] Decide whether generated source-bundle handoff evidence is sufficient or whether `RUST/` must also be committed in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-114
**Summary**: Added Rust source bundle handoff evidence and included it in the unified handoff gate.

**Completed**:
- [x] Added `adm-cli stage-source-bundle [source_root] [bundle_dir] [report_path]`.
- [x] The command copies Rust source files into `RUST/dist/source-bundle`.
- [x] The source bundle excludes generated/runtime directories: `target`, `dist`, `.adm_rust_data`, and `.git`.
- [x] The command writes `RUST/dist/AutoDesignMaker-rust/source-manifest.adm` with file count, total bytes, per-file hashes, and aggregate `bundle_hash`.
- [x] `handoff-status` now reads `source-manifest.adm` and reports `source_ready`, `source_handoff_mode`, `source_file_count`, and `source_bundle_hash`.
- [x] `release_gate.ps1` now runs `stage-source-bundle` and verifies `source-manifest.adm` contains `ready=true`.
- [x] Updated `RUST/README.md` with source bundle handoff workflow.

**Verification**:
- [x] `cargo test -p adm-cli`: passed, including 3 CLI tests.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle --help`: passed.
- [x] `cargo run -q -p adm-cli -- stage-source-bundle`: passed and wrote source bundle evidence.
- [x] `cargo run -q -p adm-cli -- handoff-status`: passed and reported `source_ready=true`.
- [x] `cargo run -q -p adm-cli -- handoff-status --require-ready`: failed as expected because external Unity and real AI provider are not ready.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed and printed `source_bundle_hash=fnv64:2e9a520b7ee9fda1`.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: listed `stage-source-bundle` and `handoff-status --require-ready`.
- [x] Cross-layer check completed for CLI -> release gate script -> README -> `source-manifest.adm` -> `handoff-status.adm`.

**Current Handoff State**:
- [x] Local release is ready: `local_release_ready=true`, `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`.
- [x] Source handoff evidence is ready: `source_ready=true`, `source_file_count=55`, `source_bundle_hash=fnv64:2e9a520b7ee9fda1`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Follow-up**:
- [ ] Configure a non-mock provider with endpoint/model/secret and rerun `ai-acceptance --require-ready`.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed to produce real `runtime_execution_results.adm` and refreshed delivery reports.
- [ ] Run `scripts/release_gate.ps1 -RequireExternalAcceptance` only after Unity and real AI provider readiness both pass.
- [ ] Decide whether the generated `RUST/dist/source-bundle` is sufficient for source handoff or whether `RUST/` must also be committed in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-113
**Summary**: Added a unified Rust handoff status report and wired it into the release gate.

**Completed**:
- [x] Added `adm-cli handoff-status [--require-ready] [release_dir] [report_path]`.
- [x] The command reads `release-acceptance.adm`, `external-acceptance.adm`, and `ai-acceptance.adm`.
- [x] The command writes `dist/AutoDesignMaker-rust/handoff-status.adm` with local release readiness, external readiness, explicit AI provider readiness, and blocker lines.
- [x] Added unit coverage for ready and blocked handoff report states.
- [x] Updated `RUST/scripts/release_gate.ps1` to run `handoff-status` after `external-acceptance`.
- [x] Updated `RUST/README.md` with the unified handoff status workflow.

**Verification**:
- [x] `cargo test -p adm-cli`: passed, including 2 handoff status tests.
- [x] `cargo run -q -p adm-cli -- handoff-status --help`: passed.
- [x] `cargo run -q -p adm-cli -- handoff-status`: passed and wrote `handoff-status.adm`.
- [x] `cargo run -q -p adm-cli -- handoff-status --require-ready`: failed as expected on this machine.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: listed `handoff-status --require-ready`.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed and printed `handoff_ready=false`.
- [x] Cross-layer check completed for README -> release gate script -> CLI command -> persisted `handoff-status.adm`.

**Current Handoff State**:
- [x] Local release is ready: `local_release_ready=true`, `release_accepted=true`, `release_delivery_ready=true`, `release_smoke_ready=true`.
- [ ] External acceptance is not ready: `external_acceptance_ready=false`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [ ] Explicit AI provider acceptance is not ready: `ai_acceptance_ready=false`, `ai_configured_ready=false`.

**Follow-up**:
- [ ] Configure a non-mock provider with endpoint/model/secret and rerun `ai-acceptance --require-ready`.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed to produce real `runtime_execution_results.adm` and refreshed delivery reports.
- [ ] Run `scripts/release_gate.ps1 -RequireExternalAcceptance` only after Unity and real AI provider readiness both pass.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-112
**Summary**: Added a redacted AI provider acceptance gate for the Rust delivery chain.

**Completed**:
- [x] Added `adm-cli ai-acceptance [--invoke] [--require-ready] [--require-invoke] <provider_id> <model> [report_path] [data_root]`.
- [x] Added `RUST/scripts/ai_acceptance_gate.ps1` as the PowerShell wrapper.
- [x] The report writes `dist/AutoDesignMaker-rust/ai-acceptance.adm` without raw model output.
- [x] `--require-ready` fails when the selected provider is missing, mock-only, lacks text generation, or invocation fails after `--invoke`.
- [x] Updated `RUST/README.md` with CLI and wrapper usage.

**Verification**:
- [x] `cargo run -q -p adm-cli -- ai-acceptance --help`: passed.
- [x] `cargo run -q -p adm-cli -- ai-acceptance openai_main gpt-4.1`: passed and wrote a redacted not-ready report.
- [x] `cargo run -q -p adm-cli -- ai-acceptance --require-ready openai_main gpt-4.1`: failed as expected because `openai_main` is not configured on this machine.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -DryRun -ProviderId openai_main -Model gpt-4.1 -RequireInvoke`: passed and printed `--require-invoke`.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -RequireReady`: failed as expected.
- [x] `cargo check --workspace`: passed.
- [x] Cross-layer check completed for README -> PowerShell wrapper -> CLI command -> persisted `ai-acceptance.adm`.

**Follow-up**:
- [ ] Configure a non-mock provider with endpoint/model/secret and rerun `ai-acceptance --require-ready`.
- [ ] Run `ai-acceptance --require-invoke` only when a real provider network call is intended.
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed to produce real `runtime_execution_results.adm` and refreshed delivery reports.
- [ ] Run `scripts/release_gate.ps1 -RequireExternalAcceptance` only after Unity and real AI provider readiness both pass.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-111
**Summary**: Added a guarded real-Unity acceptance gate script for the Rust delivery chain.

**Completed**:
- [x] Added `RUST/scripts/unity_acceptance_gate.ps1`.
- [x] Script can stage the Unity project, run Unity build preflight, run guarded local Unity build, run guarded runtime validation, restage game/SDK/Unity delivery outputs, then rerun delivery, release, and external acceptance reports.
- [x] Script supports `-ArchiveId`, `-UnityExe`, `-UnityProjectDir`, `-TargetId`, `-RequireExternalAcceptance`, and `-DryRun`.
- [x] Dry-run mode does not require the target Unity executable to exist and does not run cargo commands.
- [x] Non-dry-run mode discovers Unity via `unity-doctor` when `-UnityExe` is omitted, then fails before staging if no Unity editor is available.
- [x] Updated `RUST/README.md` with real Unity machine usage.

**Verification**:
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\unity_acceptance_gate.ps1 -DryRun -ArchiveId archive_1783253820821_24440_1 -UnityExe C:\Unity\Editor\Unity.exe`: passed and listed the full Unity acceptance command sequence.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\unity_acceptance_gate.ps1 -DryRun -ArchiveId archive_1783253820821_24440_1 -UnityExe C:\Unity\Editor\Unity.exe -RequireExternalAcceptance`: passed and listed final `external-acceptance --require-ready`.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\unity_acceptance_gate.ps1 -DryRun`: passed with `<latest_archive_id>` and `<Unity.exe>` placeholders.
- [x] `powershell -ExecutionPolicy Bypass -Command "& { try { & .\scripts\unity_acceptance_gate.ps1 *> `$null; throw 'expected unity discovery failure' } catch { if (`$_.Exception.Message -like '*Unity editor was not discovered*') { 'unity_missing_failed_as_expected' } else { throw } } }"`: passed.
- [x] Cross-layer check completed for README -> Unity acceptance script -> existing CLI Unity commands -> runtime result path -> delivery/release/external reports.

**Follow-up**:
- [ ] Run `scripts/unity_acceptance_gate.ps1` on a machine with Unity installed to produce real `runtime_execution_results.adm` and refreshed delivery reports.
- [ ] Run `scripts/unity_acceptance_gate.ps1 -RequireExternalAcceptance` only after a non-mock AI provider is configured.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-110
**Summary**: Integrated external acceptance diagnostics into the Rust one-command release gate.

**Completed**:
- [x] Updated `RUST/scripts/release_gate.ps1` to run `adm-cli external-acceptance` after local `release-acceptance`.
- [x] Added `-RequireExternalAcceptance` to make real Unity/provider readiness a hard release-gate requirement.
- [x] Added `-SkipExternalAcceptance` for local-only release gate runs.
- [x] Added an argument conflict check for `-SkipExternalAcceptance` plus `-RequireExternalAcceptance`.
- [x] Made `Invoke-CargoStep` tolerate native Cargo stderr/progress output and rely on `$LASTEXITCODE`.
- [x] Updated `RUST/README.md` with the new release-gate behavior and flags.

**Verification**:
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun`: listed `external-acceptance`.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -RequireExternalAcceptance`: listed `external-acceptance --require-ready`.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun -SkipExternalAcceptance`: listed the external acceptance skip path.
- [x] `powershell -ExecutionPolicy Bypass -Command "& { try { & .\scripts\release_gate.ps1 -DryRun -SkipExternalAcceptance -RequireExternalAcceptance *> `$null; throw 'expected conflict failure' } catch { if (`$_.Exception.Message -like '*cannot be combined*') { 'conflict_failed_as_expected' } else { throw } } }"`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -SkipTests -SkipBuild`: passed; local release accepted and reported `external_ready=false`.
- [x] `powershell -ExecutionPolicy Bypass -Command "& { try { & .\scripts\release_gate.ps1 -SkipTests -SkipBuild -RequireExternalAcceptance *> `$null; throw 'expected release gate external failure' } catch { if (`$_.Exception.Message -like '*External acceptance failed with exit code*') { 'release_gate_require_external_failed_as_expected' } else { throw } } }"`: passed.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: restored default `require_ready=false` report after the forced failure test.
- [x] Cross-layer check completed for README -> release gate script -> external acceptance CLI -> persisted release/external reports.

**Follow-up**:
- [ ] Run `scripts/release_gate.ps1 -RequireExternalAcceptance` on a machine with Unity installed and a non-mock AI provider configured; expect a fully passing gate with `external_ready=true`.
- [ ] Run desktop `Run Unity` and `Run Runtime` against a real Unity installation and confirm actual editor output ingestion.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-109
**Summary**: Moved Rust external acceptance into a tested CLI and packaging report.

**Completed**:
- [x] Added `ExternalAiProviderAcceptance` and `ExternalAcceptanceReport` to `adm-packaging`.
- [x] Added `run_external_acceptance()` to read `release-acceptance.adm`, combine Unity discovery and AI provider diagnostics, and write `external-acceptance.adm`.
- [x] Added `adm-cli external-acceptance [--require-ready] [release_dir] [report_path] [data_root]`.
- [x] Updated `scripts/external_acceptance_doctor.ps1` into a thin wrapper over the Rust CLI.
- [x] Updated `RUST/README.md` so the CLI is the primary external acceptance entry point.
- [x] Added unit tests for blocked and ready external acceptance reports.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-packaging`: passed, 36 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo run -q -p adm-cli -- external-acceptance`: passed and wrote `external-acceptance.adm`.
- [x] `cargo run -q -p adm-cli -- external-acceptance --require-ready`: failed as expected on this machine because Unity and real provider are missing.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\external_acceptance_doctor.ps1 -DryRun -ReportPath external-acceptance-check.adm`: passed and wrote nothing.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\external_acceptance_doctor.ps1`: passed and restored `require_ready=false` in the report.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 135 Rust tests.
- [x] Cross-layer check completed for README -> PowerShell wrapper -> CLI command -> packaging report model -> persisted `external-acceptance.adm`.

**Follow-up**:
- [ ] Run `cargo run -p adm-cli -- external-acceptance --require-ready` on a machine with Unity installed and a non-mock AI provider configured; expect `ready=true`.
- [ ] Run desktop `Run Unity` and `Run Runtime` against a real Unity installation and confirm actual editor output ingestion.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-108
**Summary**: Added an external acceptance doctor for the Rust rebuild.

**Completed**:
- [x] Added `RUST/scripts/external_acceptance_doctor.ps1`.
- [x] Script reads local release acceptance evidence and runs `unity-doctor` plus `ai-doctor`.
- [x] Script writes `dist/AutoDesignMaker-rust/external-acceptance.adm`.
- [x] Script separates local release readiness from external Unity and real-provider readiness.
- [x] `-RequireReady` now fails when external acceptance is missing; default mode writes a diagnostic report.
- [x] Updated `RUST/README.md`.

**Verification**:
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\external_acceptance_doctor.ps1 -DryRun`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\external_acceptance_doctor.ps1`: passed and wrote `external-acceptance.adm`.
- [x] `powershell -ExecutionPolicy Bypass -Command "& { try { & .\scripts\external_acceptance_doctor.ps1 -RequireReady *> `$null; throw 'expected external acceptance failure' } catch { if (`$_.Exception.Message -like 'External acceptance is not ready*') { 'require_ready_failed_as_expected' } else { throw } } }"`: passed.
- [x] Current external acceptance report: `ready=false`, `release_acceptance_accepted=true`, `release_smoke_ready=true`, `unity_ready=false`, `real_ai_provider_ready=false`.
- [x] Cross-layer check completed for README workflow -> PowerShell script -> CLI doctor commands -> persisted external acceptance report.

**Follow-up**:
- [ ] Run the external acceptance doctor on a machine with Unity installed and a non-mock AI provider configured; expect `ready=true`.
- [ ] Run desktop `Run Unity` and `Run Runtime` against a real Unity installation and confirm actual editor output ingestion.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-107
**Summary**: Added a one-command Windows release gate script for the Rust rebuild.

**Completed**:
- [x] Added `RUST/scripts/release_gate.ps1`.
- [x] Script imports Visual Studio build environment through `vcvars64.bat`.
- [x] Script runs `fmt --check`, workspace check/test, desktop release build, stage, release doctor, delivery doctor, and release acceptance.
- [x] Script supports `-DryRun`, `-SkipTests`, and `-SkipBuild`.
- [x] Script verifies final `release-acceptance.adm` contains `accepted=true` and `smoke_ready=true`.
- [x] Updated `RUST/README.md`.

**Verification**:
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -DryRun`: passed.
- [x] `powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1`: passed.
- [x] Scripted gate ran `cargo fmt --check`: passed.
- [x] Scripted gate ran `cargo check --workspace`: passed.
- [x] Scripted gate ran `cargo test --workspace`: passed, 133 Rust unit tests.
- [x] Scripted gate ran `cargo build -p adm-desktop --release`: passed.
- [x] Scripted gate ran `stage-desktop-release`: release hash `fnv64:97814f3263c20abf`, bytes `22492160`.
- [x] Scripted gate ran `release-doctor`: `ready=true`.
- [x] Scripted gate ran `delivery-doctor`: `ready=true`.
- [x] Scripted gate ran `release-acceptance`: `accepted=true`, `delivery_ready=true`, `smoke_ready=true`.
- [x] Cross-layer check completed for README workflow -> PowerShell script -> CLI commands -> `release-acceptance.adm`.

**Follow-up**:
- [ ] Run desktop `Run Unity` and `Run Runtime` against a real Unity installation and confirm actual editor output ingestion.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Decide whether to wire `scripts/release_gate.ps1` into CI once repository tracking is settled.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-106
**Summary**: Added a final release acceptance gate for the Rust rebuild that writes persistent delivery and staged-executable smoke evidence.

**Completed**:
- [x] Added `ReleaseAcceptanceReport` and staged executable smoke evidence to `adm-packaging`.
- [x] Added CLI command `release-acceptance`.
- [x] Made `release-acceptance --help` non-mutating after catching the initial path-handling issue.
- [x] Made desktop release staging remove stale `release-acceptance.adm`.
- [x] Added tests for incomplete acceptance, accepted report rendering, and stale acceptance cleanup.
- [x] Updated `RUST/README.md`.
- [x] Refreshed the default Rust desktop release and generated `release-acceptance.adm`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-packaging`: passed, 34 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 133 Rust unit tests.
- [x] `cargo run -q -p adm-cli -- release-acceptance --help`: printed usage and did not recreate `RUST/--help`.
- [x] Cross-layer check completed for CLI command -> packaging service -> release filesystem report -> README.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `stage-desktop-release`: passed; release hash `fnv64:97814f3263c20abf`, bytes `22492160`.
- [x] `release-doctor`: `ready=true`.
- [x] `delivery-doctor`: `ready=true`, including verified game, SDK, and Unity outputs.
- [x] `release-acceptance`: `accepted=true`, `delivery_ready=true`, `smoke_ready=true`.

**Follow-up**:
- [ ] Run desktop `Run Unity` and `Run Runtime` against a real Unity installation and confirm actual editor output ingestion.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Add CI or a scripted release pipeline that runs `fmt`, `check`, `test`, `stage-desktop-release`, and `release-acceptance` end to end.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-105
**Summary**: Added guarded desktop local Unity process launch for Unity build and runtime validation in the Rust rebuild.

**Completed**:
- [x] Added desktop `Run Unity` and `Run Runtime` buttons and callbacks.
- [x] Added desktop guarded local Unity build execution through `LocalProcessEngineBuildRunner`.
- [x] Added desktop guarded Unity runtime validation execution and archive import of `validation/runtime_execution_results.adm`.
- [x] Reused selected-archive lock release/relock behavior while local Unity processes run.
- [x] Added fake Unity child-process smoke support without requiring a real Unity install.
- [x] Updated desktop smoke coverage and `RUST/README.md`.
- [x] Refreshed the default Rust desktop release.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 131 Rust unit tests.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed, including fake Unity local process build/runtime execution.
- [x] Cross-layer check completed for Slint callbacks -> desktop helpers -> `LocalProcessEngineBuildRunner` -> archive commits -> package inspection -> README.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `stage-desktop-release`: passed; release hash `fnv64:01e099ca8ea41b42`, bytes `22491648`.
- [x] `release-doctor`: `ready=true`.
- [x] `delivery-doctor`: `ready=true`, including verified runtime execution evidence.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.

**Follow-up**:
- [ ] Run desktop `Run Unity` and `Run Runtime` against a real Unity installation and confirm actual editor output ingestion.
- [ ] Continue real Unity Editor build validation on a machine with Unity installed.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-104
**Summary**: Added desktop UI entry points for Unity runtime validation planning, dry-run, and result recording in the Rust rebuild.

**Completed**:
- [x] Added a desktop `Runtime Val` row with a runtime result file path field.
- [x] Added desktop callbacks for runtime validation plan, dry-run, and result recording.
- [x] Added desktop helpers for Unity runtime validation command planning, dry-run execution, and archive result import.
- [x] Reused selected-archive lock release/relock behavior for runtime result recording.
- [x] Added `validation/runtime_execution_results.adm` to desktop package file inspection as optional support content.
- [x] Updated desktop smoke coverage and `RUST/README.md`.
- [x] Refreshed the default Rust desktop release.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 131 Rust unit tests.
- [x] Cross-layer check completed for Slint UI -> desktop helpers -> archive commit -> package inspection -> README.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `stage-desktop-release`: passed; release hash `fnv64:510c28304ca99382`, bytes `22429696`.
- [x] `release-doctor`: `ready=true`.
- [x] `delivery-doctor`: `ready=true`, including verified runtime execution evidence.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.

**Follow-up**:
- [ ] Run guarded `run-unity-runtime-validation` on a machine with Unity installed and confirm real runtime output ingestion.
- [x] Superseded in 2026-07-05-105: desktop now exposes guarded real Unity launch.
- [ ] Continue real Unity Editor build validation on a machine with Unity installed.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-103
**Summary**: Added a generated Unity runtime validation runner and CLI execution path for the Rust rebuild.

**Completed**:
- [x] Added `Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs` to generated Unity scaffolds.
- [x] Added Unity scaffold doctor checks for the runtime validation runner script.
- [x] Added `plan_unity_runtime_validation()` and CLI commands `plan-unity-runtime-validation`, `dry-run-unity-runtime-validation`, and guarded `run-unity-runtime-validation`.
- [x] Wired real Unity runtime validation output import into `validation/runtime_execution_results.adm`.
- [x] Updated desktop smoke coverage and `RUST/README.md`.
- [x] Refreshed the default Rust release and delivery artifacts with the runner script.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-packaging`: passed, 32 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] CLI `plan-unity-runtime-validation`: generated `AutoDesignMaker.RuntimeValidation.RunValidation`.
- [x] CLI `dry-run-unity-runtime-validation`: `status=succeeded`, `launched=false`.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 131 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `stage-desktop-release`: passed; release hash `fnv64:1e760e148b5a15cf`, bytes `22262784`.
- [x] `stage-game-build-bundle`: passed; `staged_files=10`, hash `fnv64:090c0bfa25a85aff`.
- [x] `stage-sdk-bundle`: passed; `staged_files=6`, hash `fnv64:a5f8651659bbb6fc`.
- [x] `stage-unity-project`: passed; `generated_files=21`, hash `fnv64:fece028ce50237b2`.
- [x] `release-doctor`: `ready=true`.
- [x] `delivery-doctor`: `ready=true`, including verified runtime validation runner and runtime execution evidence.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] Cross-layer check completed for Unity scaffold generation -> doctor checks -> CLI -> archive import -> desktop smoke -> generated dist artifacts.

**Follow-up**:
- [ ] Run guarded `run-unity-runtime-validation` on a machine with Unity installed and confirm real runtime output ingestion.
- [ ] Add desktop UI entry points for runtime validation plan/run/import if needed.
- [ ] Continue real Unity Editor build validation on a machine with Unity installed.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-102
**Summary**: Added runtime validation execution result ingestion for the Rust rebuild and carried the evidence through delivery outputs.

**Completed**:
- [x] Added `adm-runtime` parsing and validation for external runtime execution result rows.
- [x] Added application-level archive commit support for `validation/runtime_execution_results.adm`.
- [x] Added CLI command `runtime-validation-record <archive_id> <results_file> [data_root]`.
- [x] Updated production readiness after imported runtime execution results.
- [x] Added optional runtime execution evidence staging and doctor checks for game, SDK, and Unity outputs.
- [x] Updated desktop smoke coverage and `RUST/README.md`.
- [x] Refreshed the default Rust release and delivery artifacts with runtime execution evidence.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-runtime`: passed, 4 tests.
- [x] `cargo test -p adm-application`: passed, 23 tests.
- [x] `cargo test -p adm-packaging`: passed, 31 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] CLI `runtime-validation-record`: `ready=true`, 3/3 rows passed, `written_files=18`.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 130 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `stage-desktop-release`: passed; release hash `fnv64:04884ea11778166a`, bytes `22252032`.
- [x] `stage-game-build-bundle`: passed; `staged_files=10`, hash `fnv64:090c0bfa25a85aff`.
- [x] `stage-sdk-bundle`: passed; `staged_files=6`, hash `fnv64:a5f8651659bbb6fc`.
- [x] `stage-unity-project`: passed; `generated_files=20`, hash `fnv64:84bf73bc979cfc43`.
- [x] `release-doctor`: `ready=true`.
- [x] `delivery-doctor`: `ready=true`, including verified runtime execution evidence.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] Cross-layer check completed for runtime parser -> archive commit -> CLI -> packaging -> desktop smoke -> generated dist artifacts.

**Follow-up**:
- [ ] Replace the imported CLI smoke result with a real Unity/runtime runner output when available.
- [ ] Add desktop UI entry points for recording runtime validation results if needed.
- [ ] Continue real Unity Editor build validation on a machine with Unity installed.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-101
**Summary**: Added a runtime validation report artifact across the Rust core pipeline and delivery chain.

**Completed**:
- [x] Generated `validation/runtime_validation_report.adm` from playable scenarios, acceptance trace rows, telemetry events, failure guards, and build targets.
- [x] Registered and persisted `artifact_runtime_validation_report` through the core pipeline artifact registry and formal archive content.
- [x] Added `runtime_validation_readiness` to the production readiness report.
- [x] Added runtime validation to package support files, game build required artifacts, optional SDK bundle content, Unity generated content, and delivery doctor checks.
- [x] Updated desktop smoke coverage for package counts, stage detail, package doctor, game bundle, SDK bundle, Unity scaffold, delivery doctor, and staged executable smoke.
- [x] Updated `RUST/README.md`.
- [x] Refreshed the default Rust release and delivery artifacts.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-application`: passed, 22 tests.
- [x] `cargo test -p adm-packaging`: passed, 31 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 127 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `stage-desktop-release`: passed; release hash `fnv64:cdc6f748a2823187`, bytes `22240256`.
- [x] `demo-core`: passed; created archive `archive_1783252607580_26972_1`, `written_files=17`.
- [x] `stage-game-build-bundle`: passed; `staged_files=9`, hash `fnv64:d91dff044c68cc33`.
- [x] `stage-sdk-bundle`: passed; `staged_files=5`, hash `fnv64:7d0b0bfbfc18bd6f`.
- [x] `stage-unity-project`: passed; `generated_files=19`, hash `fnv64:afe9b029cbdd94ba`.
- [x] `release-doctor`: `ready=true`, release hash `fnv64:cdc6f748a2823187`.
- [x] `delivery-doctor`: `ready=true`, including runtime validation checks for game, SDK, and Unity outputs.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] Cross-layer check completed for core pipeline -> production readiness -> archive persistence -> packaging -> desktop smoke -> generated dist artifacts.

**Follow-up**:
- [ ] Continue from static runtime probe contracts toward actual runtime execution result ingestion.
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Decide how to track or package the still-untracked `RUST/` tree in the parent repository.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-100
**Summary**: Added a scenario test plan artifact across the Rust core pipeline and delivery surfaces.

**Completed**:
- [x] Generated `validation/scenario_test_plan.adm` from playable scenarios, development task trace, art task feedback, telemetry probes, and package build targets.
- [x] Registered and persisted `artifact_scenario_test_plan` through the core pipeline artifact registry and committed archive content.
- [x] Added `scenario_test_plan_readiness` to the production readiness report.
- [x] Included the scenario test plan in package support files, game build bundles, optional SDK bundle content, Unity generated content, and delivery doctor verification.
- [x] Updated desktop smoke coverage for archive/package/game/SDK/Unity/delivery doctor paths.
- [x] Updated `RUST/README.md` and refreshed the default release/delivery artifacts.

**Verification**:
- [x] `cargo fmt`: passed after rerunning with the approved Visual Studio build environment.
- [x] `cargo test -p adm-application`: passed, 22 tests.
- [x] `cargo test -p adm-packaging`: passed, 31 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 127 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `stage-desktop-release`: passed; release hash `fnv64:784d1f1cb9293b88`, bytes `22195200`.
- [x] `demo-core`: passed; created archive `archive_1783251418896_3976_1`, `written_files=16`.
- [x] `stage-game-build-bundle`: passed; `staged_files=8`, hash `fnv64:2f028a00053a17d9`.
- [x] `stage-sdk-bundle`: passed; `staged_files=4`, hash `fnv64:fd02c03e9377bde7`.
- [x] `stage-unity-project`: passed; `generated_files=18`, hash `fnv64:161ae90817b2113b`.
- [x] `release-doctor`: `ready=true`, release hash `fnv64:784d1f1cb9293b88`.
- [x] `delivery-doctor`: `ready=true`, including scenario test plan checks for game, SDK, and Unity outputs.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] Cross-layer check completed for core pipeline -> production readiness -> packaging -> desktop smoke -> generated dist artifacts.

**Follow-up**:
- [ ] Continue production-depth expansion beyond deterministic scenario plans, especially deeper runtime probes and acceptance validation.
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Continue real-provider acceptance after safe endpoint/model/secret setup.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-099
**Summary**: Split desktop archive lock recovery into current-window release and external stale-lock clearing.

**Completed**:
- [x] Replaced the ambiguous desktop `Clear Lock` action with separate `Release Lock` and `Clear External Lock` actions.
- [x] Added Slint callbacks `release-current-lock` and `clear-external-lock`.
- [x] Added desktop binding for current-window lock release using the selected in-memory lock state.
- [x] Reworked external lock-file clearing so it rejects locks owned by the current window with `use Release Lock instead`.
- [x] Extended desktop smoke to verify current-window locks cannot be cleared through the external recovery path, current-window release works, and external stale-lock clearing still works.
- [x] Updated `RUST/README.md`.
- [x] Rebuilt and re-staged the isolated Rust desktop release.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-application`: passed, 22 tests.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 127 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `stage-desktop-release`: passed; release hash `fnv64:34afa39c530ee2c9`, bytes `22170112`.
- [x] `release-doctor`: `ready=true`, release hash `fnv64:34afa39c530ee2c9`.
- [x] `delivery-doctor`: `ready=true`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] Cross-layer check completed for Slint UI -> desktop callback/helper -> selected lock state -> archive lock file recovery.

**Follow-up**:
- [ ] Continue production-depth pipeline expansion beyond demo-derived content.
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Continue release/import/archive lifecycle polish as needed.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-098
**Summary**: Added desktop-facing workspace doctor and cleanup controls for the Rust formal archive lifecycle.

**Completed**:
- [x] Added `workspace-doctor-text` to the Slint desktop state.
- [x] Added `Workspaces` and `Clean Workspaces` actions in the Data Root row.
- [x] Added desktop callbacks `inspect-workspaces` and `cleanup-workspaces`.
- [x] Added desktop helpers using `AdmApplication::inspect_workspaces()` and `cleanup_stale_workspaces()`.
- [x] Cleanup success refreshes the project list while preserving selected lock state.
- [x] Extended desktop smoke to verify stale workspace detection, cleanup, and `workspace_count=0` after cleanup.
- [x] Updated `RUST/README.md`.
- [x] Rebuilt and re-staged the isolated Rust desktop release.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-application`: passed, 22 tests.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 127 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `stage-desktop-release`: passed; release hash `fnv64:9e52e5a70f84220e`, bytes `22154752`.
- [x] `release-doctor`: `ready=true`, release hash `fnv64:9e52e5a70f84220e`.
- [x] `delivery-doctor`: `ready=true`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] Cross-layer check completed for Slint UI -> desktop helper -> application API -> archive repo.

**Follow-up**:
- [ ] Continue lock recovery UX: distinguish current-window lock release from external stale lock clearing.
- [ ] Continue production-depth pipeline expansion beyond demo-derived content.
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-097
**Summary**: Added stale workspace doctor and cleanup commands for the Rust formal archive lifecycle.

**Completed**:
- [x] Added `WorkspaceInspection`, `ArchiveWorkspaceDoctorReport`, and `ArchiveWorkspaceCleanupReport` in `adm-archive`.
- [x] Added workspace inspection and stale cleanup APIs to `ArchiveRepository`.
- [x] Cleanup deletes only workspaces not referenced by an active `archives/*/.archive_lock` session.
- [x] Added application wrappers `inspect_workspaces()` and `cleanup_stale_workspaces()`.
- [x] Added CLI commands `workspace-doctor [data_root]` and `workspace-cleanup [data_root]`.
- [x] Updated `RUST/README.md`.
- [x] Rebuilt and re-staged the isolated Rust desktop release.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-archive`: passed, 13 tests.
- [x] `cargo test -p adm-application`: passed, 22 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] CLI smoke showed `workspace_count=2`, `stale_count=2`, cleanup `removed_count=2`, then `workspace_count=0`.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 127 Rust unit tests.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `stage-desktop-release`: passed; release hash `fnv64:282bb54c6c935e88`, bytes `22083072`.
- [x] `release-doctor`: `ready=true`, release hash `fnv64:282bb54c6c935e88`.
- [x] `delivery-doctor`: `ready=true`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] Cross-layer check completed for archive repo -> application wrapper -> CLI command -> README workflow.

**Follow-up**:
- [ ] Add desktop-facing workspace doctor/cleanup controls if project operators need it in the GUI.
- [ ] Continue lock recovery UX: distinguish current-window lock release from external stale lock clearing.
- [ ] Continue production-depth pipeline expansion beyond demo-derived content.
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-096
**Summary**: Added desktop-facing `.admproj` package doctor and import preflight coverage to the Rust project lifecycle.

**Completed**:
- [x] Added `package-doctor-text` to the Slint desktop state.
- [x] Added `Check Package` beside Import File and wired `check-import-package`.
- [x] Export success now fills Import File and immediately runs package doctor on the exported `.admproj`.
- [x] Import now runs package doctor preflight before `app.import_project(...)` and blocks when `ready=false`.
- [x] Import success preserves the package doctor report in the desktop UI.
- [x] Extended desktop smoke to verify package doctor output for exported packages.
- [x] Rebuilt and re-staged the isolated Rust desktop release.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo test -p adm-application`: passed, 21 tests.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] Cross-layer check completed for Slint UI -> desktop callback/helper -> application inspector -> archive parser.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 125 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed; release hash `fnv64:6df5d8a1d5ec5cd4`, bytes `22082560`.
- [x] `release-doctor`: `ready=true`, release hash `fnv64:6df5d8a1d5ec5cd4`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `delivery-doctor`: `ready=true`; release/game/SDK/Unity outputs all ready.

**Follow-up**:
- [ ] Continue formal project lifecycle work: archive cleanup, stale workspace cleanup, and stronger lock recovery UX.
- [ ] Continue production-depth pipeline expansion beyond demo-derived content.
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-095
**Summary**: Started the full-project completion goal and added `.admproj` package doctor coverage to the Rust project lifecycle.

**Completed**:
- [x] Created a long-running goal for completing the AutoDesignMaker Rust rebuild.
- [x] Added `ArchivePackageDoctorReport` and `ArchivePackageFileInspection` to `adm-archive`.
- [x] Added `inspect_archive_package(...)` and CLI command `package-doctor <package_file>`.
- [x] Added application-level `inspect_project_package(...)`.
- [x] Changed package import to validate all file hashes before creating an archive directory, preventing partial archives from corrupted legacy packages.
- [x] Updated `RUST/README.md` so export/import workflow includes package inspection.
- [x] Ran cross-layer review for archive -> application -> CLI -> README flow.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-archive`: passed, 12 tests.
- [x] `cargo test -p adm-application`: passed, 21 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] CLI smoke created `archive_1783248430098_3024_1`, exported `RUST/target/package-doctor-smoke.admproj`, and `package-doctor` reported `ready=true`, `file_count_actual=15`, all file hashes matching.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 125 Rust unit tests.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.

**Follow-up**:
- [ ] Add a desktop-facing package doctor action or import preflight panel.
- [ ] Continue formal project lifecycle work: save/export/import UX polish, lock recovery, and archive cleanup tools.
- [ ] Continue production-depth pipeline expansion beyond demo-derived content.
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Keep the full project completion goal active until the project is genuinely complete.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-094
**Summary**: Added deterministic production readiness reporting to the Rust rebuild and refreshed delivery artifacts.

**Completed**:
- [x] Added `validation/production_readiness.adm` generation from design quality, scenario coverage, development tasks, asset feedback, SDK/build readiness, acceptance matrix, and validation gate status.
- [x] Registered `artifact_production_readiness` in the Rust core pipeline and persisted the report into formal archives.
- [x] Added the report to package support files, game build required artifacts, SDK bundle checks, Unity generated content, and delivery doctor verification.
- [x] Updated Slint desktop inspection/smoke to display `production_readiness=ready` and validate the report across delivery outputs.
- [x] Updated `RUST/README.md`.
- [x] Rebuilt and re-staged the isolated Rust desktop release and refreshed default game, SDK, and Unity delivery artifacts.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 123 Rust unit tests.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed; release hash `fnv64:2c58c400596fad76`.
- [x] `cargo run -q -p adm-cli -- demo-core "Rust Production Readiness Demo"`: created `archive_1783247675212_5004_1`, `written_files=15`.
- [x] Inspected `validation/production_readiness.adm`: `overall_status=ready`, 9 checks ready.
- [x] Game bundle staged with `staged_files=7`, hash `fnv64:62150e44e7fa8f74`.
- [x] Unity scaffold staged with `generated_files=17`, hash `fnv64:3a1c1af721c0ccb3`.
- [x] SDK bundle restaged after dry-run engine history with `staged_files=4`, hash `fnv64:dc782981c2ec4b98`.
- [x] Unity dry-run and preflight passed with `ready_for_local_build=true`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `release-doctor`: `ready=true`.
- [x] `delivery-doctor`: `ready=true`, including production readiness in game/SDK/Unity outputs.

**Follow-up**:
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Continue production-depth pipeline expansion beyond demo-derived content.
- [ ] Continue formal project save/import/export UX and lock recovery coverage.
- [ ] Add real provider invocation acceptance only after safe endpoint/model/secret setup is available.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-093
**Summary**: Paused the active Rust rebuild goal and wrote a restart handoff plan.

**Completed**:
- [x] Honored the user's pause request and avoided further feature development.
- [x] Recorded that automatic goal continuation messages are not permission to resume development.
- [x] Wrote the current Rust rebuild state and restart instructions to `knowledge/ai_memory/session_history/2026-07-05-093.md`.
- [x] Wrote the follow-up Rust development plan to `plan/rustplan/05_暂停交接与后续推进计划.md`.
- [x] Checked that CC-Panes shared-memory environment variables were empty, so shared-pool memory was skipped.
- [x] Ran `python tools\memory\update_freshness.py` to refresh the memory hash cache.

**Verification**:
- [x] Read `AI_README.md`.
- [x] Read the latest memory index entry.
- [x] Read `knowledge/ai_memory/session_history/2026-07-05-092.md`.
- [x] Checked `git status --short` and confirmed the workspace is dirty with many unrelated existing changes.
- [x] Parsed `knowledge/ai_memory/session_history/index.json` after adding `2026-07-05-093`.

**Follow-up**:
- [ ] Resume only after the user explicitly says to continue.
- [ ] On resume, re-read `2026-07-05-093` and `2026-07-05-092`, then run a targeted Rust baseline check.
- [ ] Continue production-depth pipeline expansion beyond demo content.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-092
**Summary**: Added non-network OpenAI-compatible AI provider presets to the Rust rebuild.

**Completed**:
- [x] Added `AiProviderPreset` support to `adm-config`.
- [x] Added built-in provider presets: `openai`, `openrouter`, `deepseek`, and `local_openai`.
- [x] Added non-network OpenAI-compatible endpoint validation for preset endpoints.
- [x] Added CLI commands `ai-provider-presets` and `ai-provider-preset`.
- [x] Added a Slint desktop `AI Preset` input and `Apply Preset` action.
- [x] Extended desktop smoke to cover the preset apply path.
- [x] Updated `RUST/README.md` with AI provider preset usage.
- [x] Rebuilt and re-staged the isolated Rust desktop release after the GUI/AI preset changes.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-config`: passed, 14 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] CLI smoke: `cargo run -q -p adm-cli -- ai-provider-presets`: passed and printed `network_call=false`.
- [x] CLI smoke: `cargo run -q -p adm-cli -- ai-provider-preset openai openai_main default .\target\ai-preset-smoke`: passed and reported `openai_main MissingSecret` without network access.
- [x] CLI smoke: `cargo run -q -p adm-cli -- ai-provider-preset local_openai local_llm none .\target\ai-preset-local-smoke`: passed and reported `local_llm Ready` without network access.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 121 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed; release hash `fnv64:c2d49df11f4ff151`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`.

**Follow-up**:
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add real provider invocation acceptance notes once the user supplies non-secret endpoint/model choices and a safe test prompt.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-088
**Summary**: Moved Rust project creation from a fixed demo brief toward configurable brief inputs.

**Completed**:
- [x] Added `design_brief_from_parts(...)` in `adm-application` to build `GameDesignBrief` from explicit title, genre, player promise, and pipe/semicolon-delimited core loop steps.
- [x] Added `run-core <title> <genre> <player_promise> <core_loop_steps> [data_root]` to `adm-cli`.
- [x] Added Project Genre, player promise, and Core Loop inputs to the Slint desktop shell.
- [x] Updated desktop `Create + Run` to use the configured brief fields instead of always calling `default_demo_brief`.
- [x] Kept existing `demo-core` and resume/rerun flows on the previous demo-brief path until original brief persistence is added.
- [x] Extended desktop smoke to create the main smoke project from custom brief fields and assert the generated core artifact summary contains the custom genre.
- [x] Updated `RUST/README.md` with `run-core` and desktop brief input usage.
- [x] Rebuilt and re-staged the isolated Rust desktop release.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-application`: passed, 18 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] CLI smoke: `cargo run -q -p adm-cli -- run-core "Custom Brief Smoke" "tactical puzzle adventure" "Players solve compact tactical routes with readable feedback" "Scout the room | Plan a route | Resolve the encounter with feedback" .\target\custom-brief-smoke`: passed with `pipeline_status=Succeeded`, `validation_status=Passed`, `written_files=13`.
- [x] Inspected generated `design/project.adm`: contains `genre=tactical puzzle adventure`, the custom player promise, and the three custom core loop steps.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed after verifying the custom genre through `core_artifact_items`.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 116 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`.
- [x] Staged release hash: `fnv64:50224afc81986f01`.

**Follow-up**:
- [ ] Persist the original configurable brief into each archive and use it for resume/rerun instead of reconstructing from demo defaults.
- [ ] Add real-provider non-network config validation presets for common OpenAI-compatible endpoints.
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Continue production-depth pipeline expansion beyond demo content.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-087
**Summary**: Added a Slint desktop flow for saving named AI secrets.

**Completed**:
- [x] Added `ai-provider-secret-value` state to the Slint desktop shell.
- [x] Added a `Save Secret` action in the AI configuration area.
- [x] Added `save-ai-secret` callback wiring in `adm-desktop`.
- [x] Added `save_ai_named_secret(...)` so the GUI saves secret material only for `named:...` refs and rejects non-named refs.
- [x] Ensured the GUI status message names the secret ref and output file path without printing the secret value.
- [x] Extended desktop smoke to save a fake named secret through the GUI path, configure a provider with `named:openai`, and verify `remote_named | Ready`.
- [x] Extended desktop smoke to verify `app_config.adm` contains the `named:openai` reference but not the fake secret value.
- [x] Rebuilt and re-staged the isolated Rust desktop release.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed; AI diagnostics showed `ready_provider_count=3`, `provider_count=4`, and `remote_named | Ready`.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 115 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed; release smoke also showed `remote_named | Ready`.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`.
- [x] Staged release hash: `fnv64:31e8e31b260b3d62`.

**Follow-up**:
- [ ] Add real-provider non-network config validation presets for common OpenAI-compatible endpoints.
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add manual GUI acceptance coverage for visible double-clicked Windows processes.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-086
**Summary**: Implemented named secret resolution for Rust AI provider configuration.

**Completed**:
- [x] Added `NamedSecretStore` under `adm-config` for local `config/named_secrets.adm` secret material.
- [x] Added `AppSecretResolver` so `env:...` and `named:...` secret references resolve through one application-level resolver.
- [x] Added `AppConfig::named_secrets_file_path`, `load_named_secrets`, and `upsert_named_secret`.
- [x] Updated `AdmApplication` remote provider construction and AI diagnostics to use `AppSecretResolver`.
- [x] Added `AdmApplication::upsert_named_secret`.
- [x] Added CLI command `ai-secret-set <name> <secret> [data_root]`.
- [x] Kept `app_config.adm` storing only `named:...` references; real named secret values stay in `config/named_secrets.adm`.
- [x] Updated `RUST/README.md` with `ai-secret-set`, `named:...`, and `env:...` AI provider setup notes.
- [x] Rebuilt and re-staged the isolated Rust desktop release.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-config`: passed, 10 tests.
- [x] `cargo test -p adm-application`: passed, 17 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] CLI smoke: `cargo run -q -p adm-cli -- ai-secret-set openai fake_named_secret_for_smoke .\target\named-secret-smoke` saved `config/named_secrets.adm` without printing the secret.
- [x] CLI smoke: `cargo run -q -p adm-cli -- ai-provider-set remote_named https://example.invalid/v1 named:openai .\target\named-secret-smoke` reported `remote_named Ready` and `secret named:openai resolved`.
- [x] Smoke profile inspection: `app_config.adm` contains `named:openai`, does not contain the fake secret value, and `named_secrets.adm` contains the fake secret value.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 115 Rust unit tests.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`.
- [x] Staged release hash: `fnv64:f3907fbf3f4a83d4`.

**Follow-up**:
- [ ] Add GUI affordance for saving named secret values without using CLI.
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add manual GUI acceptance coverage for visible double-clicked Windows processes.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-085
**Summary**: Promoted the Rust acceptance trace matrix into the game build and Unity engine delivery chain.

**Completed**:
- [x] Made `validation/acceptance_matrix.adm` a required artifact for the `windows_desktop_playable` game build target.
- [x] Updated game build bundle staging and doctor checks so the staged bundle must include `content/validation/acceptance_matrix.adm`.
- [x] Updated Unity project scaffold generation to copy the acceptance matrix into `Assets/AutoDesignMaker/Generated/acceptance_matrix.adm`.
- [x] Updated Unity generated content indexing and scaffold doctor checks so the acceptance matrix is content-verified.
- [x] Updated desktop smoke delivery assertions from 27 to 29 doctor rows and added checks for the game-bundle and Unity acceptance matrix entries.
- [x] Fixed ready-state packaging test fixtures to include valid acceptance matrix content.
- [x] Updated `RUST/README.md` with default delivery staging paths, Unity scaffold staging, and acceptance matrix delivery expectations.
- [x] Rebuilt and re-staged the isolated Rust desktop release and refreshed default game, SDK, and Unity delivery artifacts.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-packaging`: passed, 31 tests.
- [x] `cargo test -p adm-application`: passed, 16 tests.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 112 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- demo-core "Rust Acceptance Matrix Delivery Demo"`: created `archive_1783241662655_37636_1` with `written_files=13`.
- [x] Inspected `.adm_rust_data/archives/archive_1783241662655_37636_1/content/package/build_targets.adm`: required artifacts include `validation/acceptance_matrix.adm`.
- [x] Inspected `.adm_rust_data/archives/archive_1783241662655_37636_1/content/validation/acceptance_matrix.adm`: 3 rows, each with distinct scenario/probe and `status=ready`.
- [x] `cargo run -q -p adm-cli -- stage-game-build-bundle archive_1783241662655_37636_1 windows_desktop_playable .\dist\game-build`: passed with `staged_files=5`.
- [x] `cargo run -q -p adm-cli -- stage-game-build-bundle archive_1783241662655_37636_1 windows_desktop_playable .\dist\game-build\windows_desktop_playable`: passed with `staged_files=5` for the default doctor path.
- [x] `cargo run -q -p adm-cli -- stage-sdk-bundle archive_1783241662655_37636_1 .\dist\sdk-bundle`: passed initially with `staged_files=2`.
- [x] `cargo run -q -p adm-cli -- stage-unity-project archive_1783241662655_37636_1 windows_desktop_playable .\dist\unity-project`: passed with `generated_files=15`.
- [x] `cargo run -q -p adm-cli -- dry-run-unity-build archive_1783241662655_37636_1 windows_desktop_playable .\target\fake-unity\Editor\Unity.exe .\dist\unity-project`: passed with `history_commit_files=14`.
- [x] Re-ran `cargo run -q -p adm-cli -- stage-sdk-bundle archive_1783241662655_37636_1 .\dist\sdk-bundle`: passed with `staged_files=3`.
- [x] `cargo run -q -p adm-cli -- unity-build-preflight archive_1783241662655_37636_1 windows_desktop_playable .\target\fake-unity\Editor\Unity.exe .\dist\unity-project ADM_CONFIRM_LOCAL_ENGINE_BUILD`: passed with `ready_for_local_build=true`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`; game bundle acceptance matrix is present and Unity acceptance matrix is verified.
- [x] Staged release hash: `fnv64:7f08d5b45b52ce65`.
- [x] Staged game build bundle hash: `fnv64:1019f48230c171d0`.
- [x] Staged Unity scaffold hash: `fnv64:56876f0b7e2ba449`.
- [x] Staged SDK bundle hash after engine history: `fnv64:98a9f9b0d9419128`.

**Follow-up**:
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add manual GUI acceptance coverage for visible double-clicked Windows processes.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-084
**Summary**: Deepened the Rust core pipeline from one shared smoke scenario to per-mechanic playable scenarios.

**Completed**:
- [x] Changed design generation so every core loop step gets its own playable scenario: `scenario_core_loop_step_1`, `scenario_core_loop_step_2`, and `scenario_core_loop_step_3`.
- [x] Changed design validation probes to per-step probes such as `probe_core_loop_step_1_input_state_feedback`.
- [x] Updated development tasks so each task references its corresponding per-step scenario and scenario-specific path test.
- [x] Updated asset feedback tasks so their validation steps reference the matching per-step scenario.
- [x] Tightened packaging-stage input validation so a development task referencing a missing design scenario produces `design.scenario.missing`.
- [x] Updated desktop acceptance-matrix smoke assertions to verify the rendered scenario id and validation probe.
- [x] Updated Rust tests and packaging fixtures away from the old shared `scenario_core_loop_smoke`.
- [x] Updated `RUST/README.md` to document per-mechanic scenario tracing.
- [x] Rebuilt and re-staged the isolated Rust desktop release and refreshed default game, SDK, and Unity delivery artifacts.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-design -p adm-development -p adm-assets -p adm-application -p adm-packaging`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed; desktop smoke verified the acceptance matrix row contains `scenario_core_loop_step_1` and `probe_core_loop_step_1_input_state_feedback`.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 112 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- demo-core "Rust Per-Mechanic Scenario Demo"`: created `archive_1783240952068_29016_1` with `written_files=13`.
- [x] Inspected `.adm_rust_data/archives/archive_1783240952068_29016_1/content/design/project.adm`: contains `scenario_core_loop_step_1`, `scenario_core_loop_step_2`, `scenario_core_loop_step_3`, and matching probes.
- [x] Inspected `.adm_rust_data/archives/archive_1783240952068_29016_1/content/development/plan.adm`: each development task references its matching per-step scenario.
- [x] Inspected `.adm_rust_data/archives/archive_1783240952068_29016_1/content/validation/acceptance_matrix.adm`: 3 rows, each row has a distinct scenario/probe and `status=ready`.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- stage-game-build-bundle archive_1783240952068_29016_1 windows_desktop_playable .\dist\game-build`: passed with `staged_files=4`.
- [x] `cargo run -q -p adm-cli -- stage-sdk-bundle archive_1783240952068_29016_1 .\dist\sdk-bundle`: passed initially with `staged_files=2`.
- [x] `cargo run -q -p adm-cli -- stage-unity-project archive_1783240952068_29016_1 windows_desktop_playable .\dist\unity-project`: passed with `generated_files=14`.
- [x] `cargo run -q -p adm-cli -- dry-run-unity-build archive_1783240952068_29016_1 windows_desktop_playable .\target\fake-unity\Editor\Unity.exe .\dist\unity-project`: passed with `history_commit_files=14`.
- [x] Re-ran `cargo run -q -p adm-cli -- stage-sdk-bundle archive_1783240952068_29016_1 .\dist\sdk-bundle`: passed with `staged_files=3`.
- [x] `cargo run -q -p adm-cli -- unity-build-preflight archive_1783240952068_29016_1 windows_desktop_playable .\target\fake-unity\Editor\Unity.exe .\dist\unity-project ADM_CONFIRM_LOCAL_ENGINE_BUILD`: passed with `ready_for_local_build=true`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed for tracked memory files; `RUST/` remains untracked in this repository.
- [x] Staged release hash: `fnv64:eb0e3680dd2efe35`.
- [x] Staged Unity scaffold hash: `fnv64:be218a38e8e9e0f3`.
- [x] Staged SDK bundle hash after engine history: `fnv64:1c2658fac04d81ef`.

**Follow-up**:
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add manual GUI acceptance coverage for visible double-clicked Windows processes.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-076
**Summary**: Added expected-output verification to local engine build execution reports.

**Completed**:
- [x] Extended `EngineBuildExecutionReport` with `expected_output_path`, `expected_output_present`, `expected_output_bytes`, and `expected_output_hash`.
- [x] Updated dry-run engine build reports to render the resolved expected output path without requiring a generated file.
- [x] Updated local process engine build execution so success now requires both process success and the expected output file being present.
- [x] Local process build reports now include generated output size and `fnv64` hash when the expected artifact exists.
- [x] Added regression tests for successful local output generation and process-success-with-missing-output failure.
- [x] Updated application engine build history tests for the new report fields.
- [x] Ran a dry-run Unity build against the generated Unity project and wrote the report into `package/engine_build_history.adm`.
- [x] Re-staged the SDK bundle after engine history generation so the default SDK delivery includes `package/engine_build_history.adm`.
- [x] Rebuilt and re-staged the isolated Rust desktop release and default delivery artifacts.

**Verification**:
- [x] `cargo test -p adm-packaging`: passed, 28 tests.
- [x] `cargo test -p adm-application`: passed, 16 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 109 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- demo-core "Rust Build Report Demo"`: created `archive_1783236103885_18620_1`.
- [x] `cargo run -q -p adm-cli -- stage-game-build-bundle archive_1783236103885_18620_1 windows_desktop_playable .\dist\game-build`: passed.
- [x] `cargo run -q -p adm-cli -- stage-sdk-bundle archive_1783236103885_18620_1 .\dist\sdk-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-unity-project archive_1783236103885_18620_1 windows_desktop_playable .\dist\unity-project`: passed with `generated_files=14`.
- [x] `cargo run -q -p adm-cli -- dry-run-unity-build archive_1783236103885_18620_1 windows_desktop_playable .\target\fake-unity\Editor\Unity.exe .\dist\unity-project`: passed and rendered `expected_output_present=false`, `expected_output_bytes=0`, `expected_output_hash=none`.
- [x] Re-ran `cargo run -q -p adm-cli -- stage-sdk-bundle archive_1783236103885_18620_1 .\dist\sdk-bundle`: passed with `staged_files=3`.
- [x] `cargo run -q -p adm-cli -- unity-build-preflight archive_1783236103885_18620_1 windows_desktop_playable .\target\fake-unity\Editor\Unity.exe .\dist\unity-project ADM_CONFIRM_LOCAL_ENGINE_BUILD`: passed with `ready_for_local_build=true`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`, content-verified Unity scripts, and `legacy_root_exe=not_modified`.
- [x] Staged release hash: `fnv64:3b18182a92a17a47`.
- [x] Staged Unity scaffold hash: `fnv64:4e5cc529d664a939`.

**Follow-up**:
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Surface expected output presence/bytes/hash in the Slint engine history table.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add broader multi-window/manual GUI acceptance coverage.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-075
**Summary**: Strengthened Rust delivery doctor from file-presence checks to content-level Unity scaffold verification.

**Completed**:
- [x] Extended `BundleFileCheck` with `content_verified`, `required_fragments`, `ready()`, and `status()`.
- [x] Delivery doctor now distinguishes `present`, `verified`, `missing`, and `content_mismatch` bundle states.
- [x] Unity scaffold inspection now verifies required fragments in the scaffold manifest, bootstrap, generated content index, gameplay model, runtime controller, gameplay controller, scene composer, input router, save data, and editor build script.
- [x] Slint desktop delivery rows now surface verified/content-mismatch status instead of only present/missing for bundle files.
- [x] Desktop smoke now asserts `AutoDesignMakerSceneComposer.cs` is content-verified in the delivery rows.
- [x] Added regression coverage so a present but incomplete `AutoDesignMakerSceneComposer.cs` makes the Unity project doctor not ready.
- [x] Rebuilt and re-staged the isolated Rust desktop release and default game, SDK, and Unity delivery artifacts.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo test -p adm-packaging`: passed, 26 tests.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed, including verified scene composer delivery row.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 107 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- demo-core "Rust Delivery Verified Demo"`: created `archive_1783235652624_2364_1`.
- [x] `cargo run -q -p adm-cli -- stage-game-build-bundle archive_1783235652624_2364_1 windows_desktop_playable .\dist\game-build`: passed.
- [x] `cargo run -q -p adm-cli -- stage-sdk-bundle archive_1783235652624_2364_1 .\dist\sdk-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-unity-project archive_1783235652624_2364_1 windows_desktop_playable .\dist\unity-project`: passed with `generated_files=14`.
- [x] `cargo run -q -p adm-cli -- unity-build-preflight archive_1783235652624_2364_1 windows_desktop_playable .\target\fake-unity\Editor\Unity.exe .\dist\unity-project ADM_CONFIRM_LOCAL_ENGINE_BUILD`: passed with `ready_for_local_build=true`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`; `AutoDesignMakerSceneComposer.cs` reported `status=verified`, `content_verified=true`, `required_fragments=6`.
- [x] Staged release hash: `fnv64:1850416a4aba51f6`.
- [x] Staged Unity scaffold hash: `fnv64:3621c9d7b13db973`.

**Follow-up**:
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Add generated Unity/engine build report capture after real batchmode execution.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add broader multi-window/manual GUI acceptance coverage.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-074
**Summary**: Added a generated Unity scene composer that creates concrete runtime scene objects from the Rust pipeline gameplay model.

**Completed**:
- [x] Added generated `AutoDesignMakerSceneComposer.cs` to the Unity project scaffold.
- [x] Updated `AutoDesignMakerRuntimeController.cs` generation so the runtime mounts `AutoDesignMakerSceneComposer` alongside input and gameplay controllers.
- [x] Scene composer now creates a main camera, directional light, workbench floor, mechanic cubes, loop links, goal marker, scenario board, and `TextMesh` labels.
- [x] Scene labels and object details are sourced from `AutoDesignMakerGameplayModel` mechanics, core loop, scenarios, development tasks, and asset feedback.
- [x] Updated Unity scaffold generation from 13 to 14 generated files.
- [x] Updated Unity project delivery checks to require `Assets/AutoDesignMaker/Runtime/AutoDesignMakerSceneComposer.cs`.
- [x] Updated Slint desktop smoke checks to validate the scene composer and 25 delivery doctor check rows.
- [x] Rebuilt and re-staged the isolated Rust desktop release and default game, SDK, and Unity delivery artifacts.

**Verification**:
- [x] `cargo test -p adm-packaging`: passed, 25 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed, including Unity scene composer and 25 delivery rows.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 106 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- demo-core "Rust Delivery Demo"`: created `archive_1783235003742_38744_1`.
- [x] `cargo run -q -p adm-cli -- stage-game-build-bundle archive_1783235003742_38744_1 windows_desktop_playable .\dist\game-build`: passed.
- [x] `cargo run -q -p adm-cli -- stage-sdk-bundle archive_1783235003742_38744_1 .\dist\sdk-bundle`: passed.
- [x] `cargo run -q -p adm-cli -- stage-unity-project archive_1783235003742_38744_1 windows_desktop_playable .\dist\unity-project`: passed with `generated_files=14`.
- [x] `cargo run -q -p adm-cli -- unity-doctor .\target\fake-unity\Editor\Unity.exe`: passed with selected fake Unity editor.
- [x] `cargo run -q -p adm-cli -- unity-build-preflight archive_1783235003742_38744_1 windows_desktop_playable .\target\fake-unity\Editor\Unity.exe .\dist\unity-project ADM_CONFIRM_LOCAL_ENGINE_BUILD`: passed with `ready_for_local_build=true`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`, `unity_project_ready=true`, and scene composer present.
- [x] Staged release hash: `fnv64:36d720d97eaac890`.
- [x] Staged Unity scaffold hash: `fnv64:cdf39c7c142bf325`.

**Follow-up**:
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Compile and inspect the generated Unity scene in a real Unity import/build pass.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add broader multi-window/manual GUI acceptance coverage.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-073
**Summary**: Added Unity Editor discovery and guarded Unity local build preflight across packaging, CLI, and Slint desktop.

**Completed**:
- [x] Added `UnityEditorCandidate`, `UnityEditorDiscoveryReport`, and `discover_unity_editor(...)` in `adm-packaging`.
- [x] Added Unity discovery sources for explicit path, `ADM_UNITY_EDITOR`, `UNITY_EDITOR_PATH`, and common Unity install locations.
- [x] Added `UnityBuildPreflightReport` and `inspect_unity_build_preflight(...)`.
- [x] Preflight now verifies Unity executable presence, `Unity.exe` naming, Unity project scaffold readiness, confirmation token validity, command line, and expected output.
- [x] Updated CLI with `unity-doctor [unity_exe]`.
- [x] Updated CLI with `unity-build-preflight <archive_id> <target_id> <unity_exe> <unity_project_dir> <confirm_token|none> [data_root]`.
- [x] Updated CLI `run-unity-build` so local Unity launch is blocked unless preflight is ready.
- [x] Added Slint desktop `unity-confirm-token` input and `Build Check` action.
- [x] Updated desktop smoke to create a fake `Unity.exe`, stage the Unity project, and verify `ready_for_local_build=true`.
- [x] Rebuilt and re-staged the isolated Rust desktop release and default delivery artifacts.

**Verification**:
- [x] `cargo test -p adm-packaging`: passed, 25 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed, including Unity build preflight assertions.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 106 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- unity-doctor .\target\fake-unity\Editor\Unity.exe`: passed with selected fake Unity editor.
- [x] `cargo run -q -p adm-cli -- unity-build-preflight archive_1783234297671_9032_1 windows_desktop_playable .\target\fake-unity\Editor\Unity.exe .\dist\unity-project ADM_CONFIRM_LOCAL_ENGINE_BUILD`: passed with `ready_for_local_build=true`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true` and `unity_project_ready=true`.
- [x] Staged release hash: `fnv64:ac3d4c76df89eaa0`.

**Follow-up**:
- [ ] Run guarded real Unity build validation on a machine with Unity installed.
- [ ] Generate concrete Unity scene objects/prefabs beyond OnGUI runtime surfaces.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add broader multi-window/manual GUI acceptance coverage.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-072
**Summary**: Generated Unity gameplay model and gameplay controller scripts from Rust pipeline artifacts.

**Completed**:
- [x] Added a Unity gameplay artifact parser in `adm-packaging` for design core loop, gameplay mechanics, playable scenarios, development tasks, and asset feedback rows.
- [x] Added generated `AutoDesignMakerGameplayModel.cs` with core loop steps, mechanics, scenarios, development tasks, and asset feedback derived from `design/project.adm`, `development/plan.adm`, and `assets/plan.adm`.
- [x] Added generated `AutoDesignMakerGameplayController.cs` that advances generated mechanics on confirm input, resets on cancel input, and renders scenario, development task, and asset feedback state in Unity runtime UI.
- [x] Updated `AutoDesignMakerRuntimeController.cs` to mount `AutoDesignMakerGameplayController` and persist active mechanic state in runtime snapshots.
- [x] Updated Unity scaffold generation from 11 to 13 generated files.
- [x] Updated Unity project delivery checks to require the gameplay model and gameplay controller.
- [x] Updated desktop smoke to validate the generated gameplay model/controller and 24 delivery doctor check rows.
- [x] Rebuilt and re-staged the isolated Rust desktop release and default delivery artifacts.

**Verification**:
- [x] `cargo test -p adm-packaging`: passed, 22 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed, including gameplay model/controller and delivery doctor assertions.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 103 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `cargo run -q -p adm-cli -- stage-unity-project archive_1783233565965_22880_1 windows_desktop_playable .\dist\unity-project`: passed with `generated_files=13`.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true` and `unity_project_ready=true`.
- [x] Staged release hash: `fnv64:8f2bc7bee429ad01`.

**Follow-up**:
- [ ] Add Unity installation discovery and guarded real local Unity build validation.
- [ ] Generate concrete Unity scene objects/prefabs beyond OnGUI runtime surfaces.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add broader multi-window/manual GUI acceptance coverage.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-071
**Summary**: Deepened the Unity scaffold into a runtime-capable generated Unity project and included it in delivery readiness checks.

**Completed**:
- [x] Expanded Unity project scaffold generation from 7 files to 11 generated project files.
- [x] Added generated Unity runtime scripts: `AutoDesignMakerRuntimeController.cs`, `AutoDesignMakerInputRouter.cs`, and `AutoDesignMakerSaveData.cs`.
- [x] Added `AutoDesignMakerGeneratedContent.cs` to expose generated artifact paths and target metadata to Unity runtime code.
- [x] Updated `AutoDesignMakerBootstrap.cs` so the bootstrap scene mounts `AutoDesignMakerRuntimeController`.
- [x] Added runtime snapshot save flow using Unity `Application.persistentDataPath` and `JsonUtility.ToJson`.
- [x] Updated packaging tests and desktop smoke to verify runtime/controller/input/save scaffold output.
- [x] Extended `DeliveryDoctorReport` with `unity_project` readiness.
- [x] Updated CLI `delivery-doctor` to accept optional `[unity_project_dir]` and default to `dist/unity-project`.
- [x] Updated desktop delivery doctor to check Unity project files and show 22 delivery check rows.
- [x] Rebuilt and re-staged the isolated Rust desktop release and default delivery artifacts.

**Verification**:
- [x] `cargo test -p adm-packaging`: passed, 22 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed, including Unity runtime scaffold and delivery doctor assertions.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 103 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- stage-unity-project archive_1783231038338_32204_1 windows_desktop_playable .\dist\unity-project`: passed with `generated_files=11`.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true` and `unity_project_ready=true`.
- [x] Staged release hash: `fnv64:fcb5bd20f080b671`.

**Follow-up**:
- [ ] Generate deeper Unity gameplay systems from design/development artifacts instead of only a runtime scaffold.
- [ ] Add Unity installation discovery and guarded real local Unity build validation.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add broader multi-window/manual GUI acceptance coverage.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-070
**Summary**: Added Unity project scaffold generation from Rust pipeline artifacts and exposed it through CLI and the Slint desktop workbench.

**Completed**:
- [x] Added `UnityProjectScaffold` and `stage_unity_project_scaffold(...)` in `adm-packaging`.
- [x] Generated Unity project files from stored pipeline artifacts: design, development, assets, and SDK snapshots.
- [x] Generated `AutoDesignMakerBootstrap.cs`, `AutoDesignMakerBuild.cs`, `ProjectVersion.txt`, and `adm-unity-scaffold-manifest.adm`.
- [x] Made the generated Unity editor build script create a bootstrap scene at build time before calling `BuildPipeline.BuildPlayer`.
- [x] Added CLI command `stage-unity-project <archive_id> <target_id> <unity_project_dir> [data_root]`.
- [x] Added Slint desktop `Stage Project` action in the Unity build row.
- [x] Updated desktop smoke to generate the Unity scaffold and assert the generated design snapshot and editor build script exist.
- [x] Rebuilt and re-staged the isolated Rust desktop release.
- [x] Re-staged default game build, SDK bundle, and Unity project scaffold under `RUST/dist/`.

**Verification**:
- [x] `cargo test -p adm-packaging`: passed, 22 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed, including Unity scaffold assertions.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 103 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- stage-unity-project archive_1783228378107_28348_1 windows_desktop_playable .\dist\unity-project`: passed with `generated_files=7`.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`.
- [x] Staged release hash: `fnv64:4a83fe9e51c582ae`.

**Follow-up**:
- [ ] Replace scaffold-level Unity output with deeper generated Unity runtime/gameplay systems.
- [ ] Add real Unity executable discovery and guarded local build acceptance when Unity is installed.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add broader multi-window/manual GUI acceptance coverage.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-069
**Summary**: Surfaced the richer Rust SDK metadata in the Slint desktop workbench.

**Completed**:
- [x] Extended Slint `SdkResourceRow` with category, target engines, target platforms, build-required status, and AI explanation.
- [x] Extended desktop `SdkResourceItem` with the same SDK metadata fields.
- [x] Updated the SDK index parser to read `category=`, `target_engines=`, `target_platforms=`, `required_for_build=`, and `ai_explanation=`.
- [x] Updated SDK row application so Slint receives target-aware metadata, not only purpose/risk/validation text.
- [x] Updated desktop smoke to verify the `Unity Build Automation SDK` row exposes `build`, `Unity`, `windows-desktop`, `build_required=true`, and the guarded Unity build explanation.
- [x] Rebuilt and re-staged the isolated Rust desktop release.

**Verification**:
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed, including SDK target metadata assertions.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 101 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`.
- [x] Staged release hash: `fnv64:424bedc4e91ad990`.

**Follow-up**:
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add broader multi-window/manual GUI acceptance coverage.
- [ ] Add richer real-engine project generation and build artifact validation.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-068
**Summary**: Expanded the Rust SDK system from a single default resource into a target-aware multi-resource SDK knowledge base.

**Completed**:
- [x] Reworked `adm-sdk` with SDK categories, target engines, target platforms, build-required flags, and AI explanation summaries.
- [x] Added `SdkKnowledgeBase::for_target(...)` and `recommended_for_target(...)`.
- [x] Expanded the default Unity/windows-desktop SDK knowledge base to 5 resources.
- [x] Added resources for runtime, input/save, build automation, telemetry/diagnostics, and desktop packaging.
- [x] Strengthened SDK validation for target engine coverage, target platform coverage, build script coverage, category, platform, risks, validation, and AI explanation data.
- [x] Connected the core pipeline SDK stage to the target-aware SDK profile.
- [x] Updated application and desktop smoke expectations to `SDK: resources=5`, `risks=10`, and `validation=15`.
- [x] Rebuilt and re-staged the isolated Rust desktop release.
- [x] Re-staged default game build and SDK delivery bundles from a fresh Rust demo archive.

**Verification**:
- [x] `cargo test -p adm-sdk`: passed, 3 tests.
- [x] `cargo test -p adm-application`: passed, 16 tests.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed, showing `SDK: resources=5`.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 101 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed, showing `SDK: resources=5`.
- [x] Default SDK bundle restage smoke verified `Unity Build Automation SDK`, `Windows Desktop Packaging SDK`, and `required_for_build=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] Staged release hash: `fnv64:e8d9140fc3b890e5`.

**Follow-up**:
- [ ] Add richer SDK UI rows for category, target coverage, build-required status, and AI explanation.
- [ ] Continue production-depth pipeline expansion beyond demo content.
- [ ] Add broader multi-window/manual GUI acceptance coverage.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-067
**Summary**: Deepened the Rust core pipeline outputs with playable scenarios, development contracts, and mechanic-level asset feedback tasks.

**Completed**:
- [x] Added `PlayableScenario` to the design domain model.
- [x] Added a rendered `## Playable Scenarios` section to `design/project.adm`.
- [x] Extended development tasks with scenario ids, implementation layers, data contracts, test cases, and telemetry events.
- [x] Added `AssetPlan::for_core_loop(...)` to generate one feedback asset task per core loop mechanic.
- [x] Switched the core pipeline assets stage to use mechanic-level asset feedback tasks.
- [x] Added pipeline validation that each development mechanic has matching asset feedback coverage.
- [x] Updated application and desktop smoke expectations from `asset_tasks=4` to `asset_tasks=6`.
- [x] Rebuilt and re-staged the isolated Rust desktop release.
- [x] Re-staged default game build and SDK delivery bundles from a fresh Rust demo archive.

**Verification**:
- [x] `cargo test -p adm-design`: passed, 2 tests.
- [x] `cargo test -p adm-development`: passed, 1 test.
- [x] `cargo test -p adm-assets`: passed, 2 tests.
- [x] `cargo test -p adm-application`: passed, 16 tests.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed, showing `asset_tasks=6`.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 100 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed, showing `asset_tasks=6`.
- [x] Default bundle restage smoke verified `stage=mechanic_feedback` and `source_mechanic=Core Loop Mechanic 3` in the staged game build asset plan.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] Staged release hash: `fnv64:37c4fc278d2beb49`.

**Follow-up**:
- [ ] Continue replacing demo-depth pipeline content with production-depth design/development/asset logic.
- [ ] Add richer SDK resource migration and SDK selection depth.
- [ ] Add broader multi-window/manual GUI acceptance coverage.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-066
**Summary**: Surfaced the unified Rust delivery doctor in the Slint desktop workbench.

**Completed**:
- [x] Added Slint `DeliveryCheckRow`.
- [x] Added `delivery-doctor-text` and `delivery-check-items` UI state.
- [x] Added a desktop `Delivery Doctor` button next to the release controls.
- [x] Added desktop callback wiring for release, game build bundle, and SDK bundle readiness checks.
- [x] Added structured delivery check rows for release executable, release manifest, README, game bundle files, and SDK bundle files.
- [x] Extended desktop smoke coverage to verify delivery doctor text and 10 structured check rows.
- [x] Rebuilt and re-staged the isolated Rust desktop release after the Slint UI change.

**Verification**:
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -q -p adm-desktop -- --smoke`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 99 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] `cargo run -q -p adm-cli -- delivery-doctor`: passed with `ready=true`.
- [x] Staged release hash: `fnv64:8033181c3f9024ee`.

**Follow-up**:
- [ ] Continue richer Slint grouping for game build bundle, SDK bundle, release, and engine history.
- [ ] Continue replacing demo-depth pipeline stages with production-depth game design/development logic.
- [ ] Add broader multi-window/manual GUI acceptance coverage.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-065
**Summary**: Added a unified Rust delivery doctor for desktop release, game build bundle, and SDK bundle readiness.

**Completed**:
- [x] Added `BundleFileCheck`, `BundleDoctorReport`, and `DeliveryDoctorReport`.
- [x] Added game build bundle inspection for required staged files.
- [x] Added SDK bundle inspection for required staged files.
- [x] Added `inspect_delivery(...)` to combine release, game bundle, and SDK bundle checks.
- [x] Added CLI command `delivery-doctor [release_dir] [game_bundle_dir] [sdk_bundle_dir]`.
- [x] Added packaging tests for ready and missing bundle states.
- [x] Staged default game build and SDK bundles from a Rust demo archive.
- [x] Rebuilt and re-staged the isolated Rust desktop release.

**Verification**:
- [x] Temp CLI delivery-doctor smoke: passed with `ready=true`.
- [x] Default CLI delivery-doctor: passed with `ready=true`.
- [x] `cargo test -p adm-packaging`: passed, 20 tests.
- [x] `cargo check -p adm-cli`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 99 Rust unit tests.
- [x] `cargo build -p adm-desktop --release`: passed.
- [x] `cargo run -q -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe`: passed.
- [x] `.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke`: passed.
- [x] `cargo run -q -p adm-cli -- release-doctor`: passed with `ready=true`.
- [x] Staged release hash: `fnv64:3e24731b9ca3b4c4`.

**Follow-up**:
- [ ] Add a desktop visible delivery doctor panel/button.
- [ ] Add richer Slint grouping for release, SDK bundle, game build bundle, and engine history.
- [ ] Continue replacing demo-depth pipeline stages with production-depth game design/development logic.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-064
**Summary**: Updated Rust README with current CLI, release, SDK, and Unity build workflows.

**Completed**:
- [x] Documented `release-doctor`.
- [x] Documented staged desktop smoke command.
- [x] Documented demo project creation and project listing.
- [x] Documented export/import commands.
- [x] Documented AI doctor/provider check/manual invocation commands.
- [x] Documented game build bundle staging.
- [x] Documented SDK bundle staging.
- [x] Documented Unity build planning, dry-run, and guarded real-run commands.
- [x] Documented that engine build reports persist to `package/engine_build_history.adm`.

**Verification**:
- [x] `git diff --check -- RUST\README.md`: passed.

**Follow-up**:
- [ ] Add richer Slint grouping for release, SDK bundle, game build bundle, and engine history.
- [ ] Continue final delivery checklist hardening.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-031
**Summary**: Upgraded project, AI provider, package, and validation desktop displays to structured row models.

**Completed**:
- [x] Added Slint `ProjectRow`.
- [x] Added Slint `AiProviderRow`.
- [x] Added Slint `PackageFileRow`.
- [x] Added Slint `ValidationIssueRow`.
- [x] Added `project-items`, `ai-provider-items`, `package-file-items`, and `validation-issue-items`.
- [x] Project rows can select archives directly.
- [x] AI config panel now renders provider rows.
- [x] Package and validation panels now render structured rows.
- [x] Desktop smoke verifies project, provider, package, and validation row models.

**Verification**:
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 73 Rust unit tests.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Add a desktop smoke fixture that covers the Resume Failed visual entry.
- [ ] Add an explicit manual real-AI invocation entry while keeping defaults on dry-run/no-network.
- [ ] Continue completing the desktop GUI run, validation, and packaging loop.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-030
**Summary**: Upgraded Slint stage detail to structured fields and an artifact row model while keeping text compatibility.

**Completed**:
- [x] Added Slint `StageArtifactRow` struct.
- [x] Added structured `stage-detail-label/id/status/message` properties.
- [x] Added `stage-artifact-items: [StageArtifactRow]`.
- [x] Added Rust `StageDetailView`.
- [x] `inspect_stage_detail(...)` now returns structured detail.
- [x] Added `apply_stage_detail(...)` and `apply_stage_artifacts(...)`.
- [x] Kept `stage-detail-text` rendering for smoke/log compatibility.
- [x] Smoke verifies structured packaging artifact rows.

**Verification**:
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 73 Rust unit tests.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Upgrade project list, AI diagnostics, package, and validation displays to structured lists or tables.
- [ ] Add an explicit manual real-AI invocation entry while keeping defaults on dry-run/no-network.
- [ ] Add a desktop smoke fixture that covers the Resume Failed visual entry.
- [ ] Continue completing the desktop GUI run, validation, and packaging loop.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-029
**Summary**: Added graph-aware core pipeline stage rerun and failed-stage resume support across application, CLI, and Slint desktop.

**Completed**:
- [x] Added `PipelineGraph::downstream_stage_ids(...)`.
- [x] Added `PipelineRunState::rewind_to_stage(...)`.
- [x] Added `PipelineRunReport::from_report_text(...)` and `last_unsuccessful_stage_id(...)`.
- [x] Added `AdmApplication::rerun_core_pipeline_stage(...)`.
- [x] Added `AdmApplication::resume_failed_core_pipeline(...)`.
- [x] Added CLI `rerun-stage` and `resume-failed`.
- [x] Added Slint stage-row `Rerun` action and archive-level `Resume Failed`.
- [x] Desktop smoke now verifies selected-stage rerun.

**Verification**:
- [x] `cargo test -p adm-pipeline`: passed, 10 tests.
- [x] `cargo test -p adm-application`: passed, 13 tests.
- [x] `cargo check -p adm-cli -p adm-desktop`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] CLI `rerun-stage development`: passed, `rerun_results=3`.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 73 Rust unit tests.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Split stage detail into structured fields and artifact list model.
- [ ] Upgrade project list, AI diagnostics, package, and validation displays to structured lists or tables.
- [ ] Add an explicit manual real-AI invocation entry while keeping defaults on dry-run/no-network.
- [ ] Add a desktop smoke fixture that covers the Resume Failed visual entry.
- [ ] Continue completing the desktop GUI run, validation, and packaging loop.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-028
**Summary**: Upgraded Slint desktop stage progress from fixed buttons to a stage-items list model with per-row Inspect actions.

**Completed**:
- [x] Added Slint `StageRow` struct.
- [x] Added `stage-items: [StageRow]` property.
- [x] Replaced fixed stage buttons with a repeated stage list.
- [x] Added per-row `Inspect` action through unified `inspect-stage(string)`.
- [x] Added Rust conversion from `Vec<StageProgressItem>` to `ModelRc<VecModel<StageRow>>`.
- [x] `DesktopRunSummary` and `ProjectInspection` now carry structured `stage_items`.
- [x] Smoke asserts the Slint stage item model has 5 rows.

**Verification**:
- [x] `cargo check -p adm-desktop`: passed, no warnings.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo test --workspace`: passed, 68 Rust unit tests.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.
- [ ] Split stage detail into structured fields and artifact list model.
- [ ] Upgrade project list, AI diagnostics, package, and validation displays to structured lists or tables.
- [ ] Add an explicit manual real-AI invocation entry while keeping defaults on dry-run/no-network.
- [ ] Continue completing the desktop GUI run, validation, and packaging loop.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-027
**Summary**: Added editable provider capabilities and no-network provider check action to the Slint desktop POC.

**Completed**:
- [x] Added `ai-provider-capabilities` Slint property and input.
- [x] Added `ai-provider-model` Slint property and input.
- [x] Added desktop `check-ai-provider` callback and button.
- [x] Save Provider now parses comma-separated capabilities instead of forcing `text_generation`.
- [x] Empty capabilities input defaults to `text_generation`.
- [x] Desktop provider Check builds the configured chat-completions provider and prints a capability matrix.
- [x] Desktop provider Check emits `network_call=false` and does not call real AI.
- [x] Smoke now covers `text_generation,structured_output` provider config and dry-run check.

**Verification**:
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo test --workspace`: passed, 68 Rust unit tests.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.
- [ ] Continue moving desktop UI from text panels to structured lists, tables, and state controls.
- [ ] Add an explicit manual real-AI invocation entry while keeping defaults on dry-run/no-network.
- [ ] Continue completing the desktop GUI run, validation, and packaging loop.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-026
**Summary**: Added application chat-completions provider factory and CLI no-network provider dry-run check.

**Completed**:
- [x] Added `ConfiguredChatCompletionsProvider` alias.
- [x] Added `AdmApplication::chat_completions_provider_from_config(...)`.
- [x] Factory composes configured provider settings, `ReqwestBlockingHttpJsonClient`, and `ChatCompletionsTransport`.
- [x] Added no-network unit test for chat provider construction and capability checks.
- [x] Added CLI command `ai-provider-check <provider_id> <model> [data_root]`.
- [x] CLI dry-run prints `network_call=false` and capability support matrix.

**Verification**:
- [x] `cargo test -p adm-application`: passed, 10 tests.
- [x] CLI `ai-provider-set` with `env:PATH`: passed.
- [x] CLI `ai-provider-check`: passed, no network call.
- [x] `cargo check --workspace`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo test --workspace`: passed, 68 Rust unit tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Add editable capability selection to the Slint provider configuration.
- [ ] Add a desktop provider check entry that does not make network calls by default.
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.
- [ ] Continue completing the desktop GUI run, validation, and packaging loop.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-025
**Summary**: Added reqwest-backed real HTTP JSON client adapter for AI chat-completions transport with no-network unit tests.

**Completed**:
- [x] Added `reqwest` to `adm-ai` with `blocking`, `json`, and `rustls` features.
- [x] Added `ReqwestBlockingHttpJsonClient`.
- [x] Implemented `AiHttpJsonClient` using bearer JSON POST.
- [x] Added empty endpoint, secret, and request body validation.
- [x] Added non-2xx HTTP status handling with truncated error body.
- [x] Kept bearer secret out of generated error messages.
- [x] Added no-network unit tests for validation and secret non-leakage.

**Verification**:
- [x] `cargo test -p adm-ai`: passed, 19 `adm-ai` tests.
- [x] `cargo check --workspace`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo test --workspace`: passed, 67 Rust unit tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Add an `adm-application` helper to build chat-completions remote providers from configured provider settings.
- [ ] Add CLI or desktop dry-run entry points for real providers without making network calls by default.
- [ ] Add editable capability selection to the Slint provider configuration.
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-024
**Summary**: Added AI provider capabilities to config profile schema and wired runtime provider factory and diagnostics to use them.

**Completed**:
- [x] Added `AiProviderConfig.capabilities`.
- [x] Added `ai.provider.N.capabilities=...` profile rendering and parsing.
- [x] Kept legacy provider profiles compatible when capabilities are missing.
- [x] Defaulted enabled/local providers to `text_generation`.
- [x] Rejected enabled providers with empty or duplicate capabilities.
- [x] Changed `remote_ai_provider_from_config(...)` to use configured capabilities.
- [x] Updated CLI and Slint provider save paths to write default capabilities.
- [x] Exposed provider capabilities in AI diagnostics UI text.

**Verification**:
- [x] `cargo test -p adm-config`: passed, 8 `adm-config` tests.
- [x] `cargo test -p adm-application`: passed, 9 tests.
- [x] `cargo test -p adm-ui-model`: passed, 2 tests.
- [x] `cargo check --workspace`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo test --workspace`: passed, 65 Rust unit tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Implement real HTTP client adapter while keeping tests on fake HTTP.
- [ ] Add editable capability selection to the Slint provider configuration.
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.
- [ ] Continue completing the desktop GUI run, validation, and packaging loop.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-023
**Summary**: Added explicit app config profile schema versioning with legacy profile compatibility and future-version rejection.

**Completed**:
- [x] Added `APP_CONFIG_PROFILE_VERSION`.
- [x] Added `AppConfig.profile_version`.
- [x] New configs render `profile_version=1`.
- [x] Config validation rejects zero and future profile versions.
- [x] Profile parser accepts legacy files without an explicit version.
- [x] Added tests for round trip, legacy compatibility, and future-version rejection.

**Verification**:
- [x] `cargo test -p adm-config`: passed, 7 `adm-config` tests.
- [x] `cargo check --workspace`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo test --workspace`: passed, 64 Rust unit tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Add provider capabilities to config schema while keeping profile migration controlled.
- [ ] Implement real HTTP client adapter while keeping tests on fake HTTP.
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.
- [ ] Continue completing the desktop GUI run, validation, and packaging loop.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-022
**Summary**: Added chat-completions compatible JSON transport with injectable HTTP client and fake-HTTP tests.

**Completed**:
- [x] Added direct `serde_json` dependency to `adm-ai`.
- [x] Added `AiHttpJsonClient`.
- [x] Added `ChatCompletionsTransport<C: AiHttpJsonClient>`.
- [x] Transport builds chat-completions JSON from `AiRemoteRequest`.
- [x] Transport appends `/chat/completions` when endpoint hint is a base URL.
- [x] Transport parses `choices[0].message.content`.
- [x] Transport rejects missing or empty content.
- [x] Added fake HTTP tests with no real network dependency.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-ai`: passed, 17 `adm-ai` tests.
- [x] `cargo check --workspace`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo test --workspace`: passed, 62 Rust unit tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Implement real HTTP client adapter while keeping tests on fake HTTP.
- [ ] Add provider capabilities to config schema or introduce provider presets.
- [ ] Add explicit profile schema versioning and migration strategy for provider configuration.
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-021
**Summary**: Added `adm-application` runtime provider factory from provider config, `EnvSecretResolver`, and remote transport.

**Completed**:
- [x] Added `AdmApplication::remote_ai_provider_from_config(...)`.
- [x] Factory validates configured provider id, enabled state, endpoint hint, and secret ref.
- [x] Factory resolves secret at runtime with `EnvSecretResolver`.
- [x] Factory wraps the resolved secret with `AiSecretMaterial`.
- [x] Factory returns `RemoteAiProvider<T>` using caller-supplied capabilities and transport.
- [x] Added fake-transport test proving the profile stores only `env:PATH`, not the resolved secret value.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-application`: passed, 9 `adm-application` tests.
- [x] `cargo check --workspace`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo test --workspace`: passed, 60 Rust unit tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Implement concrete OpenAI/Claude transports with fake HTTP tests and no real network dependency.
- [ ] Add explicit profile schema versioning and migration strategy for provider configuration.
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.
- [ ] Run visible-window manual verification and full release package end-to-end validation.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-020
**Summary**: Added remote AI provider abstraction with redacted runtime secret material and pluggable transport.

**Completed**:
- [x] Added `AiSecretMaterial` for runtime-only secrets with redacted `Debug`.
- [x] Added `AiRemoteRequest` with provider id, endpoint hint, secret material, capability, prompt, and context summary.
- [x] Added `AiRemoteResponse`.
- [x] Added `AiRemoteTransport` trait for concrete provider transports.
- [x] Added `RemoteAiProvider<T: AiRemoteTransport>` implementing the existing `AiProvider` trait.
- [x] Added tests for transport invocation, secret redaction, empty secret, and empty endpoint rejection.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-ai`: passed, 15 `adm-ai` tests.
- [x] `cargo check --workspace`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo test --workspace`: passed, 59 Rust unit tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Add an `adm-application` factory boundary from `AiProviderConfig + SecretResolver` to runtime providers.
- [ ] Implement concrete OpenAI/Claude transports without relying on real network in tests.
- [ ] Add explicit profile schema versioning and migration strategy for provider configuration.
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-019
**Summary**: Added AI provider configuration upsert/disable services plus CLI and Slint POC write paths.

**Completed**:
- [x] Added `AiSettings::upsert_provider(...)` and `AiSettings::disable_provider(...)`.
- [x] Added `AdmApplication::upsert_ai_provider(...)` and `AdmApplication::disable_ai_provider(...)`.
- [x] Added CLI `ai-provider-set <provider_id> <endpoint_hint|none> <secret_ref|none> [data_root]`.
- [x] Added CLI `ai-provider-disable <provider_id> [data_root]`.
- [x] Added Slint provider id, endpoint, and secret ref inputs.
- [x] Added `Save Provider` and `Disable` callbacks in the desktop POC.
- [x] Desktop smoke now verifies provider MissingSecret and Disabled states.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-config -p adm-application`: passed.
- [x] `cargo check -p adm-cli -p adm-desktop`: passed.
- [x] `cargo run -p adm-cli -- ai-provider-set remote_cli https://example.invalid/v1 env:ADM_CLI_AI_KEY E:\workwork\CrewAi\AutoDesignMaker\RUST\target\adm-cli-provider-smoke`: passed.
- [x] `cargo run -p adm-cli -- ai-provider-disable remote_cli E:\workwork\CrewAi\AutoDesignMaker\RUST\target\adm-cli-provider-smoke`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed, showing `remote_smoke | Disabled | provider is disabled`.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 57 Rust unit tests.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Implement real AI provider adapter trait wrapping without writing API keys into config files.
- [ ] Add explicit profile schema versioning and migration strategy for provider configuration.
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.
- [ ] Run visible-window manual verification and full release package end-to-end validation.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-018
**Summary**: Added desktop AI diagnostics panel and smoke coverage for provider failure summaries.

**Completed**:
- [x] Added `AiProviderDiagnosticsItem` and `AiDiagnosticsView` in `adm-ui-model`.
- [x] Added `ShellState::ai_diagnostics`.
- [x] Added AI diagnostics rendering for budget, retry count, provider counts, readiness, and notes.
- [x] Added `ai-config-text` and `refresh-ai-diagnostics` to the Slint UI.
- [x] Added an `AI Diagnostics` button and visible AI Config panel in the desktop POC.
- [x] Desktop startup/refresh paths now show `AdmApplication::ai_diagnostics()`.
- [x] Desktop smoke now verifies diagnostics and a fixture with `budget_exceeded` plus `provider_unavailable`.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed, showing AI Config and failure summary.
- [x] `cargo check --workspace`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo test --workspace`: passed, 55 Rust unit tests.
- [x] `cargo run -p adm-cli -- ai-doctor`: passed.
- [x] `cargo run -p adm-cli -- ai-journal archive_1783211679400_39896_1 E:\workwork\CrewAi\AutoDesignMaker\RUST\target\adm-cli-smoke-ai`: passed.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Continue provider task persistence and configuration editing before real AI adapters.
- [ ] Add CLI/UI profile write paths for real provider configuration without exposing secret values.
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.
- [ ] Run visible-window manual verification and full release package end-to-end validation.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-017
**Summary**: Added AI task journal summaries and exposed failure counts, failure kinds, and last error through desktop and CLI.

**Completed**:
- [x] Added `AiFailureSummary` and `AiTaskJournalSummary` in `adm-ai`.
- [x] Added `AiTaskJournal::summary()` to count records, accepted, failed, rejected, failure kinds, last failure kind, and last error.
- [x] Extended `AiStatusView` with `failed_count`, `rejected_count`, `failure_summary`, and `last_error`.
- [x] Desktop run/resume and project inspection now render AI summary through one helper.
- [x] Added `adm-cli ai-journal <archive_id> [data_root]`.
- [x] Added unit tests for AI journal summary and AI status failure rendering.

**Verification**:
- [x] `cargo fmt`: passed.
- [x] `cargo test -p adm-ai -p adm-ui-model`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 54 Rust unit tests.
- [x] `cargo run -p adm-cli -- ai-doctor`: passed.
- [x] `cargo run -p adm-cli -- demo-core AiJournalSmoke E:\workwork\CrewAi\AutoDesignMaker\RUST\target\adm-cli-smoke-ai`: passed.
- [x] `cargo run -p adm-cli -- ai-journal archive_1783211679400_39896_1 E:\workwork\CrewAi\AutoDesignMaker\RUST\target\adm-cli-smoke-ai`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed, showing AI failed/rejected fields, 5 completed stages, and Packaging detail.
- [x] `cargo fmt --check`: passed.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Continue provider task persistence and configuration UI before real AI adapters.
- [ ] Add failure-scenario smoke or fixtures for provider unavailable and budget exceeded desktop display.
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.
- [ ] Run visible-window manual verification and full release package end-to-end validation.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-016
**Summary**: Added desktop stage progress and stage detail panels backed by `run_state`, `run_report`, and `artifact_registry`.

**Completed**:
- [x] Added `StageProgressItem` and `render_stage_progress(...)` in `adm-ui-model`.
- [x] Added `stage-progress-text` and `stage-detail-text` Slint properties.
- [x] Added fixed stage buttons for Design, Development, Assets, SDK, and Packaging.
- [x] Added stage inspect callbacks and backend handlers.
- [x] Desktop now parses `pipeline/run_state.adm`, `pipeline/run_report.adm`, and `pipeline/artifact_registry.adm`.
- [x] Stage progress shows status, artifact counts, and messages.
- [x] Stage detail shows stage id, status, artifact count, message, and output paths.
- [x] Desktop smoke validates `Packaging: Completed` and `package/manifest.adm` stage detail.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 53 Rust unit tests.
- [x] `cargo run -p adm-cli -- ai-doctor`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed, showing 5 completed stages and Packaging detail.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Upgrade stage progress from fixed buttons to a proper selectable list model.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.
- [ ] Continue provider task persistence, configuration UI, and provider failure display before real AI adapters.
- [ ] Run visible-window manual verification and full release package end-to-end validation.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-015
**Summary**: Added saved pipeline run-state recovery through `adm-application` and a desktop `Resume` action covered by smoke.

**Completed**:
- [x] `CorePipelineServices` now has `build_with_state(...)`.
- [x] Recovery validates run state against the pipeline graph and prebuilds completed-stage dependencies in graph order.
- [x] `AdmApplication::resume_core_pipeline(...)` reads `pipeline/run_state.adm`, preserves existing `ai/journal.adm`, resumes remaining stages, and commits outputs.
- [x] Shared output persistence moved into `commit_core_pipeline_outputs(...)`.
- [x] Slint UI added a `Resume` button and `resume-project` callback.
- [x] Desktop smoke now exercises create/run -> resume -> export.
- [x] Added recovery tests at core pipeline and application-service levels.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test -p adm-application`: passed, 7 application tests.
- [x] `cargo test --workspace`: passed, 53 Rust unit tests.
- [x] `cargo run -p adm-cli -- ai-doctor`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed, including `Resumed ...` output.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed, with Git line-ending warnings only.

**Follow-up**:
- [ ] Add clickable stage progress and failure reason display to the GUI.
- [ ] Extend recovery so users can choose failed-stage or selected-stage reruns.
- [ ] Continue provider task persistence, configuration UI, and provider failure display before real AI adapters.
- [ ] Run visible-window manual verification and full release package end-to-end validation.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-014
**Summary**: Split `adm-application` core pipeline into a real staged business executor that generates and registers artifacts per stage.

**Completed**:
- [x] Added `RUST/crates/adm-application/src/core_pipeline.rs`.
- [x] Moved core pipeline business logic out of `lib.rs`.
- [x] `CorePipelineServices::build(...)` now runs `PipelineRunner::run_serial_with_state(...)` with a business stage executor.
- [x] `design` stage now generates the design document, AI journal, and `artifact_design_project`.
- [x] `development`, `assets`, `sdk`, and `packaging` stages now generate their own outputs and register artifacts only when executed.
- [x] Final output assembly now merges input validation with package validation after run state is known.
- [x] Added a stage-executor unit test covering 5 stage results, 5 artifacts, packaging artifact, and passed validation.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test -p adm-application`: passed, 5 application tests.
- [x] `cargo test --workspace`: passed, 51 Rust unit tests.
- [x] `cargo run -p adm-cli -- ai-doctor`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST knowledge\ai_memory`: passed.

**Follow-up**:
- [ ] Add failed-stage recovery entry points and GUI resume action.
- [ ] Add clickable stage progress and failure reason display to the GUI.
- [ ] Continue provider task persistence, configuration UI, and provider failure display before real AI adapters.
- [ ] Run visible-window manual verification and full release package end-to-end validation.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-013
**Summary**: Upgraded `.admproj` exports to `ADM_PACKAGE_V3` with package-level payload hash validation while preserving legacy V2 imports.

**Completed**:
- [x] `adm-archive` now exports `ADM_PACKAGE_V3`.
- [x] Added package header fields: `format_version=3`, `payload_hash=fnv64:*`, and `file_count=*`.
- [x] V3 import validates the whole payload hash before parsing file contents.
- [x] V3 import validates header file count against parsed file records.
- [x] Legacy `ADM_PACKAGE_V2` import remains supported.
- [x] Exported file collection is sorted for stable payload generation.
- [x] Added tests for V3 export, V3 payload tamper rejection, and V2 legacy import.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test -p adm-archive`: passed, 10 archive tests.
- [x] `cargo test --workspace`: passed, 50 Rust unit tests.
- [x] `cargo run -p adm-cli -- ai-doctor`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST`: passed.

**Follow-up**:
- [ ] Split core business outputs into true resumable stage executors.
- [ ] Add failed-stage resume action and clickable stage progress to the GUI.
- [ ] Continue provider task persistence, configuration UI, and provider failure display before real AI adapters.
- [ ] Run visible-window manual verification and broader release package end-to-end validation.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-012
**Summary**: Switched application pipeline execution from placeholder artifacts to artifact-registry-backed stage results and added AI failure kind classification/persistence.

**Completed**:
- [x] Added `CoreStageExecutor` in `adm-application`.
- [x] Pipeline stage results now use real artifact ids from `ArtifactRegistry`.
- [x] Stage execution fails if a stage has no registered artifact.
- [x] App test asserts `artifact_package_manifest` appears in the pipeline report.
- [x] Added `AiFailureKind`.
- [x] Added `AiTaskRecord.failure_kind`.
- [x] Persisted `failure_kind` in `ADM_AI_JOURNAL_V1`.
- [x] Router classifies budget, unsupported capability, invalid response, and provider-unavailable failures.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 47 Rust unit tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST`: passed.

**Follow-up**:
- [ ] Split core business outputs into true resumable stage executors.
- [ ] Add failed-stage resume action and clickable stage progress to the GUI.
- [ ] Expose provider failure kinds in AI diagnostics and configuration UI.
- [ ] Add package-level manifest/hash validation for `.admproj` and run visible-window manual verification.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-011
**Summary**: Exposed pipeline, AI, package, and validation status in the Slint desktop GUI and added corresponding `adm-ui-model` view models.

**Completed**:
- [x] Added `AiStatusView`, `PackageStatusView`, and `ValidationStatusView`.
- [x] Extended `ShellState` with AI/package/validation fields.
- [x] Added Slint properties and panels for AI, Package, and Validation.
- [x] Desktop project inspection now reads committed run state, AI journal, package manifest, and validation report.
- [x] Desktop smoke asserts package panel state and prints Pipeline/AI/Package/Validation panels.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 45 Rust unit tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST`: passed.

**Follow-up**:
- [ ] Split deterministic core stages into real resumable business executors.
- [ ] Add AI provider error classification and provider task persistence.
- [ ] Add real list selection, file picker, clickable stage progress, and failed-stage resume to the GUI.
- [ ] Run visible-window manual verification and release package validation.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-010
**Summary**: Added release package validation with `PackageManifest` support files and connected package validation into the application validation report.

**Completed**:
- [x] Added `PackageManifest.support_files`.
- [x] Added `PackageManifest::with_support_files`.
- [x] Added `validate_release_package`.
- [x] Validates target platform, entries, support files, artifact registry coverage, run state, and validation status.
- [x] Application package manifest now separates product entries from audit support files.
- [x] Package validation is merged into the final application `ValidationReport`.
- [x] App tests read `content/package/manifest.adm` and confirm support files.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 45 Rust unit tests.
- [x] `cargo run -p adm-cli -- ai-doctor`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST`: passed.

**Follow-up**:
- [ ] Expose pipeline run state, AI diagnostics/journal, package support files, and validation status in the desktop GUI.
- [ ] Split deterministic core stages into real resumable business executors.
- [ ] Add release package manifest/parser and package-level hash validation for `.admproj`.
- [ ] Add provider task persistence and error classification before real AI provider adapters.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-009
**Summary**: Added state-aware pipeline resume/failure recovery to `PipelineRunner` and switched `adm-application` default pipeline execution to maintain `PipelineRunState` directly.

**Completed**:
- [x] Added `PipelineRunState::cancel`.
- [x] Added `PipelineRunState::is_stage_completed`.
- [x] Added `PipelineRunState::validate_for_graph`.
- [x] Added `PipelineRunner::run_serial_with_state`.
- [x] Resume now skips completed stages and runs only remaining stages.
- [x] Failed stages update run state and can be resumed later.
- [x] `adm-application` default pipeline now uses state-aware execution.
- [x] App tests parse `content/pipeline/run_state.adm` from the committed archive.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 43 Rust unit tests.
- [x] `cargo run -p adm-cli -- ai-doctor`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST`: passed.

**Follow-up**:
- [ ] Continue SDK/package validation so package manifest, artifact registry, run state, and validation report form an inspectable release bundle.
- [ ] Expose pipeline run state, failed-stage resume, and stage progress in the desktop GUI.
- [ ] Split deterministic core stages into real resumable business executors.
- [ ] Add provider task persistence and error classification before real AI provider adapters.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-008
**Summary**: Upgraded the Rust AI task journal to recoverable `ADM_AI_JOURNAL_V1` with hex-encoded prompt/result fields, file save/load, parser tests, and application archive re-parse validation.

**Completed**:
- [x] Added stable `as_str/parse` helpers for AI capability, output state, and task status.
- [x] Changed `AiTaskJournal::render()` to `ADM_AI_JOURNAL_V1`.
- [x] Added `AiTaskJournal::from_text`, `save_to_path`, and `load_from_path`.
- [x] Persisted prompt, context, provider, status, budget, attempts, timestamps, errors, result output, and validation notes.
- [x] Used hex encoding for multi-line/string fields.
- [x] Added app-level test that reads `content/ai/journal.adm` back from a committed archive.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 40 Rust unit tests.
- [x] `cargo run -p adm-cli -- ai-doctor`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST`: passed.

**Follow-up**:
- [ ] Implement pipeline resume and failure recovery using `PipelineRunState` plus AI journal.
- [ ] Expose AI journal and diagnostics in the desktop GUI.
- [ ] Add provider task persistence and error classification before real AI provider adapters.
- [ ] Continue SDK/package validation, visible-window verification, and release package checks.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-007
**Summary**: Added Rust AI configuration persistence, secret references, provider readiness diagnostics, AI intervention decisions, retry policy, and CLI `ai-doctor`.

**Completed**:
- [x] Added `AiSettings` to `AppConfig`.
- [x] Added `config/app_config.adm` save/load/default persistence.
- [x] Added `SecretRef` with `env:` and `named:` references.
- [x] Added `EnvSecretResolver` and provider readiness diagnostics.
- [x] Added AI intervention decisions for quality gaps, missing content, and post-generation review.
- [x] Added retry policy and attempt tracking to AI task routing.
- [x] Connected AI settings to `adm-application` core pipeline.
- [x] Added `adm-cli ai-doctor [data_root]`.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 37 Rust unit tests.
- [x] `cargo run -p adm-cli -- ai-doctor`: passed.
- [x] `cargo run -p adm-desktop -- --smoke`: passed.
- [x] `git diff --check -- RUST`: passed.

**Follow-up**:
- [ ] Implement real AI provider adapters without hard-coded secrets.
- [ ] Add structured AI task journal persistence and recovery.
- [ ] Expose AI diagnostics and config path in the desktop GUI.
- [ ] Continue pipeline resume, SDK/package validation, visible-window verification, and release package checks.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-006
**Summary**: Entered goal mode and upgraded the Slint desktop from inline POC to a maintained UI file with import/export controls, project details, pipeline status, and `adm-ui-model` usage.

**Completed**:
- [x] Created active goal for completing the Rust rebuild.
- [x] Moved Slint UI to `RUST/apps/adm-desktop/ui/main.slint`.
- [x] Added `RUST/apps/adm-desktop/build.rs` and `slint-build`.
- [x] Added GUI controls for archive id, export path, import path, Select, Export, Import.
- [x] Added project detail and pipeline status panels.
- [x] Connected GUI import/export/select callbacks to `adm-application`.
- [x] Started using `adm-ui-model` in the desktop layer.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 34 Rust unit tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed, including `.admproj` export.

**Follow-up**:
- [ ] Continue goal with real AI Provider config, task persistence, retry, and secret-ref handling.
- [ ] Add real file picker, clickable project list, and stage progress view.
- [ ] Reduce GUI string formatting by expanding `adm-ui-model` binding models.
- [ ] Run visible-window manual verification.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-005
**Summary**: Connected the Slint POC desktop shell to `adm-application` with project creation, core pipeline execution, project refresh, and automated smoke validation.

**Completed**:
- [x] Added `slint = "1"` to `adm-desktop`; Cargo resolved `slint 1.17.0`.
- [x] Replaced the console desktop placeholder with a real Slint window.
- [x] Added UI controls for project title, data root, create/run, refresh, status, and project list.
- [x] Connected Slint callbacks to `adm-application`.
- [x] Added `adm-desktop --smoke` for automated non-interactive validation.
- [x] Adjusted only `adm-desktop` unsafe lint from workspace `forbid` to crate `deny` because Slint macros need internal lint flexibility.

**Verification**:
- [x] `cargo check -p adm-desktop`: passed.
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 34 Rust unit tests.
- [x] `cargo run -p adm-desktop -- --smoke`: passed and created/listed a project via application services.

**Follow-up**:
- [ ] Move Slint UI into `.slint` files before the interface grows.
- [ ] Add import/export controls, project detail view, and pipeline progress view.
- [ ] Run visible-window manual verification.
- [ ] Connect `adm-ui-model` as the formal binding layer instead of direct string formatting.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-004
**Summary**: Advanced the Rust rebuild with staged core services, `ADM_PACKAGE_V2` binary-safe import/export, pipeline artifact registry and run state, AI task journal/budget/router, and CLI import.

**Completed**:
- [x] Split core pipeline output construction into `CorePipelineServices`.
- [x] Added `ADM_PACKAGE_V2` export format with file path, content hash, and hex bytes.
- [x] Added package import with hash validation.
- [x] Added binary workspace writes.
- [x] Added pipeline artifact registry and renderable/restorable run state.
- [x] Added AI task records, task journal, budget, and provider router.
- [x] Added `adm-cli import <package_file> [data_root]`.
- [x] Verified CLI create/export/import/list across two temp data roots.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 34 Rust unit tests.
- [x] Real CLI project migration flow passed.

**Follow-up**:
- [ ] Start Slint POC connected to `adm-application` services.
- [ ] Add AI task persistence, retry, provider config, and secret-ref resolution.
- [ ] Implement pipeline resume execution from `PipelineRunState`.
- [ ] Harden `ADM_PACKAGE_V2` with package-level manifest/hash and damaged-package diagnostics.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-003
**Summary**: Expanded the Rust rebuild from framework skeleton to a runnable core product loop with application services, CLI project commands, archive load/list/export, deterministic pipeline outputs, AI validation gates, pipeline gates, and UI state models.

**Completed**:
- [x] Added `adm-application` as the application service layer for CLI/GUI callers.
- [x] Added CLI commands: `demo-core`, `list`, and `export`.
- [x] Extended `adm-archive` with manifest parsing, archive loading/listing, workspace reads, delete synchronization, and `.admproj` export.
- [x] Extended design/development/assets/SDK/packaging/validation crates with minimal business models and renderable outputs.
- [x] Added AI output validation so raw AI output cannot become accepted without validation.
- [x] Added pipeline gate abstractions and gate-blocking tests.
- [x] Added UI shell/project/pipeline state models for future Slint binding.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 27 Rust unit tests.
- [x] CLI create/list/export flow passed against a temp data root.

**Follow-up**:
- [ ] Split `demo-core` into real staged application services.
- [ ] Harden `.admproj` with binary-safe encoding, import flow, and content hash validation.
- [ ] Add AI task persistence, retry, budget, and capability routing before real Provider integration.
- [ ] Add artifact registry, run-state persistence, stage resume, and failure recovery.
- [ ] Connect Slint POC to `adm-application` after service APIs stabilize.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-002
**Summary**: Started the Rust rebuild implementation by creating the `RUST/` Cargo workspace, initial apps, framework crates, archive locking, workspace commit transactions, and tests.

**Completed**:
- [x] Created `RUST/Cargo.toml`, `Cargo.lock`, and `RUST/.gitignore`.
- [x] Created `apps/adm-cli` and `apps/adm-desktop`.
- [x] Created 14 framework crates under `RUST/crates/`.
- [x] Implemented foundation basics: errors, IDs, timestamps, content hash, safe paths, atomic write.
- [x] Implemented config basics: app config, AI Provider config, secret references.
- [x] Implemented archive basics: formal archive manifest, workspace session, archive lock, open archive workspace, workspace writes, commit-to-archive transaction.
- [x] Implemented runtime task logs/cancellation, AI Provider trait/mock provider, and serial pipeline graph/runner.

**Verification**:
- [x] `cargo fmt --check`: passed.
- [x] `cargo check --workspace`: passed.
- [x] `cargo test --workspace`: passed, 17 Rust unit tests.
- [x] `cargo run -p adm-cli -- --version`: `adm-cli 0.1.0`.

**Follow-up**:
- [ ] Extend `adm-archive` with manifest parsing, delete synchronization, export package format, and stronger Windows lock metadata.
- [ ] Extend `adm-ai` with validated/accepted output flow before real Provider integration.
- [ ] Extend `adm-pipeline` with gates, artifact registry, AI intervention hooks, and failure recovery.
- [ ] Add Slint POC only after application services and UI model are stable enough.

---

**Date**: 2026-07-05
**ID**: 2026-07-05-001
**Summary**: Planned the complete Rust rebuild of AutoDesignMaker, including architecture decisions, migration matrix, framework-first phases, engineering cautions, and development guarantees.

**Completed**:
- [x] Confirmed the Rust version is a full rebuild under `RUST/`; existing Python code is reference only.
- [x] Confirmed the retained product scope: game design, game development pipeline, game packaging, SDK knowledge/support, and on-demand AI intervention.
- [x] Confirmed multi-process desktop behavior: different formal archives may open in parallel; the same formal archive must be locked against concurrent editing.
- [x] Confirmed framework-first development, multi-crate Cargo workspace, directory-backed formal archives, serial same-project pipeline first, and pluggable AI Providers.
- [x] Updated `CONTEXT.md` and added Rust rebuild ADRs for the settled decisions.
- [x] Wrote the Rust plan package under `plan/rustplan/`.

**Verification**:
- [x] Confirmed 5 plan files exist under `plan/rustplan/`.
- [x] Checked headings and required terms with `rg`.
- [x] Confirmed `plan/` is ignored by Git and left it unchanged as requested.

**Follow-up**:
- [ ] Begin implementation from `RUST/Cargo.toml` and foundational crates, not business UI first.
- [ ] Build mock AI, task records, validation states, archive locks, and runtime logs before connecting real AI Providers.
- [ ] Treat old Python saves/drafts/schemas/tests as reference only; do not promise compatibility unless a later import tool is explicitly designed.

---

**Date**: 2026-07-04
**ID**: 2026-07-04-009
**Summary**: Investigated GUI multi-open/save-load failures, identified archive-lock conflict with a live background pythonw process, and added minimal guardrails.

**Completed**:
- [x] Confirmed `saves/save_20260704_220446_2f433d/.archive_lock` is owned by live `pythonw.exe` PID `24096`.
- [x] Confirmed draft `drafts/20260704_230912_24096` last recorded Step08-10 success and updated the save to `11/15` progress.
- [x] Identified direct cause: subsequent windows auto-load the global `current_save_id`, but the archive lock blocks the same formal save from being loaded/written by another live process.
- [x] Added close-time guard: when the pipeline is running, the main window now requests a stop and keeps the window open instead of destroying UI state.
- [x] Added GUI exit cleanup to release the current archive lock during normal interpreter shutdown.
- [x] Added save-manager guard so "保存到选中存档" refuses archives open in another live window.

**Verification**:
- [x] `python -m pytest core\tests\unit\test_draft_archive_paths.py -q`: 35 passed.
- [x] `python -m compileall core\ui\gui_app.py core\ui\main_window.py core\ui\save_manager_dialog.py core\tests\unit\test_draft_archive_paths.py`: passed.

**Follow-up**:
- [ ] To immediately recover current save loading, confirm/close live PID `24096`; do not manually delete `.archive_lock` while its owner is alive.
- [ ] Same-save multi-window editing remains intentionally unsupported; design read-only/open-copy mode if parallel inspection is required.
- [ ] CC-Panes plan recall was skipped because required environment variables were empty.

---

**Date**: 2026-07-04
**ID**: 2026-07-04-008
**Summary**: Verified the failing Step05-to-Step09 structured contract chain regression and identified the real Step09 art task enrichment ordering bug.

**Completed**:
- [x] Reproduced `test_step05_to_step09_structured_contract_chain.py` failure at `assert all(task.get("semantic_policy") for task in art_contract["tasks"])`.
- [x] Confirmed the test path does approve Step07 style confirmation before Step09; the failure is not caused by empty tasks or missing style approval.
- [x] Found the actual ordering problem in `_stage9_outputs()`: `enrich_art_tasks()` runs before `ensure_art_tasks_cover_asset_strategy()`, so strategy-generated tasks bypass `semantic_policy/rework_policy` injection.
- [x] Confirmed `enrich_art_tasks_with_semantics()` does not overwrite existing `generation_prompt` and does not inject `semantic_policy`.

**Verification**:
- [x] Printed Step09 task fields: base tasks have `semantic_policy/rework_policy`; `strategy_generated=True` tasks do not.

**Follow-up**:
- [ ] Fix `_stage9_outputs()` ordering: build all tasks, apply asset-strategy completion, run `enrich_art_tasks()` on the complete list, then run semantic enrichment.
- [ ] Re-run `core\tests\unit\test_step05_to_step09_structured_contract_chain.py`.

---

**Date**: 2026-07-04
**ID**: 2026-07-04-007
**Summary**: Cloned `htdt/godogen` into `build/skill/`, extracted its art asset generation skill, and created a Codex-valid AutoDesignMaker-oriented art asset skill.

**Completed**:
- [x] Used the skill-creator workflow for reusable skill creation.
- [x] Cloned `https://github.com/htdt/godogen.git` into `build/skill/_godogen_repo/`.
- [x] Copied Godogen `asset-gen/` into `build/skill/asset-gen/`.
- [x] Normalized the copied `asset-gen/SKILL.md` frontmatter for local Codex skill validation while preserving the original source under `_godogen_repo/asset-gen/`.
- [x] Created `build/skill/autodesign-art-assets/` as an AutoDesignMaker-specific integrated art asset skill.
- [x] Added `build/skill/SKILL_介绍.md` with source commit, folder roles, capabilities, and usage guidance.

**Verification**:
- [x] `quick_validate.py build\skill\asset-gen`: passed with Python UTF-8 mode.
- [x] `quick_validate.py build\skill\autodesign-art-assets`: passed.
- [x] `python -m compileall build\skill\asset-gen\tools`: passed.
- [x] `python -m compileall build\skill\autodesign-art-assets\scripts`: passed.
- [x] Removed generated `__pycache__` directories after compile checks.

**Follow-up**:
- [ ] Install a selected skill into `$CODEX_HOME/skills/` or `~/.codex/skills/` only if auto-discovery is needed.
- [ ] Keep paid asset generation behind explicit user budget approval.
- [ ] Prefer `autodesign-art-assets` for AutoDesignMaker pipeline work; use `asset-gen` for the original Godogen workflow.

---

**Date**: 2026-07-04
**ID**: 2026-07-04-006
**Summary**: Reviewed `htdt/godogen` and generated a Godogen-inspired fast playable validation planning package under `build/`.

**Completed**:
- [x] Reviewed Godogen's README, runtime manifest, publish script, engine guides, asset-gen skill, project overview, setup notes, and changelog.
- [x] Identified the fast-generation mechanism: thin runtime, high-signal engine guides, clean published game repo, executable asset skill, durable project status, and proof-first validation.
- [x] Created `build/01_godogen快速游戏生成_设计总分总计划书.md`.
- [x] Created `build/02_godogen快速游戏生成_代码总分总计划书.md`.
- [x] Created `build/03_godogen快速游戏生成_分层与步骤填充表.md`.

**Verification**:
- [x] Confirmed all three generated `build/` documents exist and are non-empty.
- [x] Checked the first lines of each generated document.
- [x] No runtime code changed, so no test suite was run.

**Follow-up**:
- [ ] If implementing, start with the minimal Babylon.js proof path before expanding to Godot/Unity/Bevy.
- [ ] Keep the fast playable lane separate from the existing Step00-14 documentation pipeline.
- [ ] Require Step14 proof artifacts to include screenshots/video/logs, not text-only completion claims.

---

**Date**: 2026-07-04
**ID**: 2026-07-04-005
**Summary**: Rebuilt the built-in 2D game design template library: archived old historical active templates and generated 25 replacement templates, 5 per existing category.

**Completed**:
- [x] Archived 38 old active `builtin_*.json` templates under `knowledge/design_data/project_templates/_archived_2026_07_04_replaced_builtin/`.
- [x] Generated 25 new active templates across `iaa_hypercasual`, `indie`, `midcore`, `3a`, and `large_service`.
- [x] Preserved the existing category rule of 5 templates per category.
- [x] Updated `template_index.json` and gameplaySystems mappings for the new template IDs.
- [x] Added `tools/design/rebuild_builtin_project_templates.py` for reproducible template replacement.
- [x] Updated template regression tests for the new matrix, archive separation, Chinese labels, 2D-only policy, L5 coverage, and gameplay system validity.

**Verification**:
- [x] `python -m compileall tools\design\rebuild_builtin_project_templates.py tools\design\fill_template_gameplay_systems.py core\tests\unit\test_template_l5_expansion.py core\tests\unit\test_hades_quality_optimization.py`: passed.
- [x] `python tools\design\fill_template_gameplay_systems.py`: updated 25 templates.
- [x] `python -m pytest core\tests\unit\test_template_l5_expansion.py core\tests\unit\test_hades_quality_optimization.py core\tests\unit\test_project_templates.py core\tests\unit\test_pipeline_optimization_helpers.py core\tests\unit\test_l5_supplement.py -q`: 83 passed.
- [x] Active template search found no old game names, `范本反推`, or `????` placeholders.
- [x] Count check confirmed 25 active templates, 5 per category, and 38 archived old templates.
- [x] `git diff --check` on touched template/tool/test paths passed with CRLF warnings only.

**Follow-up**:
- [ ] The new templates are structurally valid L5/B-tier 2D templates; run a later per-template content refinement pass if deeply hand-authored differences are required.
- [ ] Old templates are retained only in the archive directory and are no longer loaded as active built-ins.

---

**Date**: 2026-07-04
**ID**: 2026-07-04-004
**Summary**: Implemented the standardized art asset pipeline atomics: Step04 consumable specs, Step09 enriched art task contracts, Step12 full art handoff, Step13 Unity materialization reports, and Step14 structured art/playable acceptance.

**Completed**:
- [x] Added `core/art_pipeline/` for path policy, contract builders, handoff construction, Unity materialization reports, and acceptance helpers.
- [x] Standardized new generated art paths under `Assets/AutoDesign/`.
- [x] Extended Step04/09/12/13/14 outputs according to the artlist zero atomics.
- [x] Added `knowledge/schemas/ai_design/art_pipeline/` schemas and updated artifact registry references.
- [x] Added direct dependency graph edges `Stage04 -> Stage12` and `Stage09 -> Stage12`.
- [x] Updated tests for the new AutoDesign path policy, art handoff gate, and Step14 acceptance reports.

**Verification**:
- [x] `python -m compileall core/art_pipeline core/engines/generation.py`: passed.
- [x] `python -m pytest core/tests/ --tb=short -q`: 336 passed.
- [x] `git diff --check`: passed with CRLF warnings only.
- [x] Registry schema reference check: all schema files exist.

**Follow-up**:
- [ ] Manual Unity validation is still required on a real project save.
- [ ] If image generation is disabled, Step12 records warnings and relies on Unity fallback materialization rather than claiming final bitmap art exists.
- [ ] Addressables remains optional unless explicitly enabled in the Unity project.

---

**Date**: 2026-07-04
**ID**: 2026-07-04-003
**Summary**: Designed and refined the standard art asset pipeline implementation plan under `plan/UEWplan/art/artlist/`, including review fixes for real plugin paths, dependency graph edges, optional Addressables, and concrete Step14 upper-design acceptance.

**Completed**:
- [x] Created `plan/UEWplan/art/artlist/01_标准化美术资产管线开发计划.md`.
- [x] Defined the complete art pipeline from Step04 asset consumption specs through Step14 art + playable acceptance.
- [x] Required new art business logic to live under `core/art_pipeline/`, with `core/engines/generation.py` compatibility-only.
- [x] Corrected real implementation paths to `pipeline/step_09_art_plan/plugin.py` and `pipeline/step_14_integration_validation/plugin.py`.
- [x] Clarified dependency graph changes: confirm existing Stage04 -> Stage09, Stage12 -> Stage13, Stage13 -> Stage14; add Stage09 -> Stage12 and Stage04 -> Stage12.
- [x] Scoped Addressables as optional P2 capability, not a default blocker.
- [x] Defined Step14 Level 5 sources and `skipped_with_warning` behavior when upper acceptance sources are absent.
- [x] Created `plan/UEWplan/art/artlist/02_开发计划审查问题处理记录.md`.

**Verification**:
- [x] Confirmed real plugin directories exist for Step09 and Step14.
- [x] Confirmed artifact validator uses explicit `schema_refs.schema` paths, allowing `knowledge/schemas/ai_design/art_pipeline/` if Phase 01 tests subdirectory schema refs.
- [x] Text checks confirmed corrected paths, generation.py boundary, Addressables condition, dependency edge reasoning, and Step14 Level 5 source rules.
- [x] No runtime code changes or tests were run for this planning-only session.
- [x] CC-Panes shared memory skipped because all required environment variables were empty.

**Follow-up**:
- [ ] If implementing, start with Phase 00 test baseline and Phase 01 schema/registry/dependency graph work.
- [ ] Do not create duplicate `step_09_art_planning` or `step_14_playable_acceptance` modules.
- [ ] Keep art logic in `core/art_pipeline/`; `generation.py` remains adapter-only.
- [ ] Treat Addressables as optional unless explicitly enabled.

---

## 历史会话摘要

**Date**: 2026-07-04
**ID**: 2026-07-04-002
**Summary**: Implemented the pipeline execution/packaging restructure: the development pipeline now ends at Step14, Step14 belongs to the execution stage, packaging is a top-level stage, and old validation/version/audit surfaces were removed.

**Completed**:
- [x] Moved Step14 Integration Validation into the development pipeline execution group.
- [x] Replaced the old top-level Validation stage with a top-level Package stage.
- [x] Added `core/packaging/` and `core/ui/package_panel.py` for independent package report generation.
- [x] Removed Step15/16/17 from pipeline and artifact registries.
- [x] Removed old validation, version history, and project audit UI surfaces.
- [x] Removed active `version_manifest` maintenance from save and iteration flows.
- [x] Added `SceneAssembly` source package typing and removed old `Build`/`DeltaPatch` source package types.
- [x] Updated project docs and implementation plan records.

**Verification**:
- [x] `python -m compileall core\packaging core\ui\main_window.py core\ui\pipeline_panel.py core\ui\package_panel.py core\ui\workbench.py core\ui\app_window.py core\registry.py core\main.py core\source\groups.py core\source\importer.py core\engines\generation.py core\iteration core\save\manager.py`: passed.
- [x] `python -m pytest core\tests\unit\test_ui_panels_import.py core\tests\unit\test_validation_cli.py core\tests\unit\test_iteration_development.py core\tests\unit\test_draft_archive_paths.py core\tests\integration\test_plugins.py -q`: 40 passed.
- [x] `python -m pytest core\tests -q`: 293 passed.
- [x] `git diff --check`: passed with CRLF warnings only.

**Follow-up**:
- [ ] Keep `core.patch` / "补充开发" separate from the removed Step16 delta patch pipeline stage.
- [ ] Design real platform build automation later as a packaging enhancement.
- [ ] Review commit scope carefully because the worktree still contains unrelated earlier uncommitted changes.

---

**Date**: 2026-06-29
**ID**: 2026-06-29-005
**Summary**: Implemented `newplan/step11_codex_cwd_bug_plan.md`: Step11 Codex tasks now execute from the configured Unity project path instead of the AutoDesignMaker repository root.

**Completed**:
- [x] Added `ModelTask.cwd` with an empty-string default to preserve existing adapter behavior.
- [x] Updated `CodexAdapter.generate()` to use `task.cwd` when present and fall back to the adapter root otherwise.
- [x] Set `model_task.cwd` and `repair_task.cwd` to `project_path` in Step11 main execution and auto-repair paths.
- [x] Added Codex cwd regression tests and isolated a Step11 legacy-resume test from the current formal save workspace.

**Verification**:
- [x] `python -B -m py_compile core\adapters\base.py core\adapters\codex_adapter.py core\engines\generation.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_model_adapters.py -q`: 8 passed.
- [x] `python -B -m pytest core\tests\unit\test_unattended_recovery.py -q`: 21 passed.
- [x] `python -B -m pytest core\tests\integration\test_plugins.py -q`: 4 passed.
- [x] `python -B -m pytest -q`: 221 passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.

**Follow-up**:
- [ ] To recover the failed Factorio draft, clear `drafts/20260629_231411_38176/outputs/artifacts/stage_11/` after stopping any active run, then rerun Step11.
- [ ] Keep Step11 Codex execution and repair tasks rooted at the configured Unity `development_path`.

---

**Date**: 2026-06-29
**ID**: 2026-06-29-004
**Summary**: Patched a Step11 follow-up bug: execution-object fallback now matches `DEV_EXECUTION_STAGE` instead of hardcoded stage `10`, and unused Step11 helpers were removed.

**Completed**:
- [x] Fixed `_stage11_outputs()` exception fallback to find existing stage-11 program task execution objects.
- [x] Removed unused `_previous_stage11_report()` and `_stage11_resume_read_dirs()` helpers.
- [x] Updated the AST-shadowing test start point after removing dead code.
- [x] Added a regression assertion that Step11 fallback uses `DEV_EXECUTION_STAGE` and no longer checks `stage == 10`.

**Verification**:
- [x] `python -B -m pytest core\tests\unit\test_unattended_recovery.py -q`: 21 passed.
- [x] `python -B -m py_compile core\engines\generation.py core\tests\unit\test_unattended_recovery.py`: passed.
- [x] `python -B -m pytest -q`: 218 passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.

**Follow-up**:
- [ ] Keep Step11 execution-object lookups tied to `DEV_EXECUTION_STAGE`.

---

**Date**: 2026-06-29
**ID**: 2026-06-29-003
**Summary**: Implemented `newplan/step11_standardization_bugfix_plan.md`: Step11 Development Execution now uses Step11-owned recovery paths, run-state semantics, save history lookup, and development-execution messages.

**Completed**:
- [x] Removed the Step11 `load_project_settings` local import shadowing issue.
- [x] Renamed Step11 execution helpers from `_stage10_*` to `_stage11_*` while preserving real Step10 asset alignment.
- [x] Moved new Step11 task checkpoints to `outputs/checkpoints/stage_11_resume_records/`.
- [x] Kept read-only compatibility for legacy `stage_12_resume_records/`.
- [x] Reworked previous task record loading into explicit priority merging: active stage, active Step11 checkpoint, current `saves/` workspace, legacy checkpoint, then legacy `save/` workspace.
- [x] Updated Step11 run state to `unit_type="stage11_task"` and stop reports to `stage=11`.
- [x] Corrected Step11 and Step13 development-execution messages that incorrectly referred to Stage12.
- [x] Added regression tests for shadowing, resume paths, save workspace reads, merge priority, run state, stop report, and Step13 text.

**Verification**:
- [x] `python -B -m pytest core\tests\unit\test_unattended_recovery.py -q`: 20 passed.
- [x] `python -B -m pytest core\tests\integration\test_plugins.py -q`: 4 passed.
- [x] `python -B -m pytest core\tests\unit\test_manual_style_confirmation.py -q`: 27 passed.
- [x] `python -B -m pytest -q`: 217 passed.
- [x] `python -B -m compileall -q core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config tools\design tools\save\repair_blank_save_progress.py`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.

**Follow-up**:
- [ ] Keep legacy `stage_12_resume_records/` and legacy `save/` reads compatibility-only.
- [ ] Do not reintroduce Step11 `stage10`/`Stage12` naming in recovery state.

---

**Date**: 2026-06-29
**ID**: 2026-06-29-002
**Summary**: Implemented `newplan/step10_image_generation_removal_plan.md`: Step10 asset alignment no longer performs real image generation, and the failed Clash Royale save/output residue was cleaned.

**Completed**:
- [x] Removed Step10's `write_skill_guidance(out_dir, "imagegen")` and `_write_generated_images_manifest(out_dir, art_tasks, stage=ASSET_ALIGNMENT_STAGE)` calls.
- [x] Confirmed Step12 still owns image generation through `_write_generated_images_manifest(out_dir, tasks, stage=ART_PRODUCTION_STAGE)`.
- [x] Updated `pipeline/step_10_asset_alignment/README.md` to state Step10 does not generate preview or production images.
- [x] Added regression coverage proving Step10 does not call image generation or create image-generation outputs.
- [x] Deleted Clash Royale save `save_20260628_235115_d020fa`, all six linked drafts, and untracked image residue directories `generated_assets/` and `output/`.

**Verification**:
- [x] `python -B -m pytest core\tests\unit\test_hades_quality_optimization.py -q`: 10 passed.
- [x] `python -B -m pytest core\tests\integration\test_plugins.py -q`: 4 passed.
- [x] `python -B -m pytest -q`: 208 passed.
- [x] `python -B -m compileall -q core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config tools\design tools\save\repair_blank_save_progress.py`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.
- [x] Confirmed the Clash Royale save, all linked drafts, `generated_assets/`, and `output/` no longer exist.

**Follow-up**:
- [ ] Keep Step10 deterministic and do not reintroduce image generation there.
- [ ] Use Step12 Art Production for real image generation.
- [ ] `newplan/` remains local planning material and should not be committed unless explicitly requested.

---

**Date**: 2026-06-29
**ID**: 2026-06-29-001
**Summary**: Generated `newplan/11_12_update/step11_12_second_pass_optimization_plan.md`: a second-pass optimization plan for Step11/12 unattended execution.

**Completed**:
- [x] Reviewed current Step11/12 implementation, unattended recovery helpers, Step13 gating, PipelinePanel display, and prior v4 plan.
- [x] Confirmed the v4 main line is structurally correct: `completed_with_review`, correction queues, pause/resume logs, Step13 guard, dependency skip records, and Step11 sync reduction are present.
- [x] Identified remaining optimization areas: transactional repair writes, explicit resume controller, queue-state-based Step13 gating, UI review actions, Step12 real asset task records, and Step11 naming/log cleanup.
- [x] Wrote the complete optimization plan to `newplan/11_12_update/step11_12_second_pass_optimization_plan.md`.

**Verification**:
- [x] Confirmed the plan file exists and is readable.
- [x] No runtime code changes or tests were run for this planning-only task.

**Follow-up**:
- [ ] If implementing the plan, start with P0 items: two-phase repair writes, unresolved queue helper, and Step13 queue-aware gating.
- [ ] Preserve current `completed_with_review` semantics and do not default Step13 to continue past unresolved Step11/12 queues.
- [ ] `newplan/` remains local planning material and should not be committed unless explicitly requested.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-021
**Summary**: Implemented `newplan/blank_new_save_progress_bug_fix.md`: UI "new save" now creates a blank project save instead of cloning the current pipeline workspace, preventing fresh projects from inheriting old `10/17` progress.

**Completed**:
- [x] Added `create_blank_save()` for blank project saves while preserving `create_save()` / `save_current_as()` as current-state clone paths.
- [x] Added `_reset_active_for_blank_save()` to clear old pipeline outputs, workspace files, execution-object stores, snapshots, timeline, and generated `source_artifacts/devflow_*` packages while preserving non-generated source artifacts.
- [x] Extracted `_progress_from_workspace_root()` so save workspaces and repair tools can use the same progress calculation.
- [x] Updated `SaveManagerDialog.on_new_save()` to call `create_blank_save()` and clarified UI comments/status text.
- [x] Added `tools/save/repair_blank_save_progress.py` with dry-run by default and `--apply` for existing abnormal saves.
- [x] Added regression coverage for blank save cleanup, workspace cleanup, generated source cleanup, no execution-object ownership transfer, UI API routing, save-current-as clone semantics, and repair dry-run/apply.

**Verification**:
- [x] `python -B -m compileall core\save\manager.py core\ui\save_manager_dialog.py core\tests\unit\test_draft_archive_paths.py tools\save\repair_blank_save_progress.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_draft_archive_paths.py -q`: 24 passed.
- [x] `python -B -m pytest -q`: 207 passed.
- [x] `python -B -m compileall -q core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config tools\design tools\save\repair_blank_save_progress.py`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.
- [x] `python tools\save\repair_blank_save_progress.py --help`: passed.

**Follow-up**:
- [ ] Existing abnormal save `save_20260628_232416_bb4424` was not automatically modified; use the repair tool only after explicit confirmation.
- [ ] `newplan/` remains local planning material and should not be committed unless explicitly requested.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-020
**Summary**: Implemented `newplan/execution_object_save_id_ownership_fix.md`: restored strict execution-object `save_id` validation and added explicit create/save-as ownership transfer with audit records.

**Completed**:
- [x] Replaced the broad `migrate_save_id()` helper with `transfer_ownership_to_save(new_save_id, source_save_id, reason)`.
- [x] Kept `ExecutionObjectStore.save()` strict: ordinary mismatch between stored `save_id` and expected `save_id` raises `ExecutionObjectError`.
- [x] Added top-level `ownership_migrations` audit records to execution-object stores and updated the schema.
- [x] Added save-manager ownership transfer for active draft execution-object stores before the first sync of a newly created save.
- [x] Wrote active `timeline.jsonl` events for execution-object ownership transfers.
- [x] Added regression coverage for mismatch errors, successful transfer, wrong source rejection, create/save-as active/archive migration, and load-save non-repair behavior.

**Verification**:
- [x] `python -B -m compileall core\save\manager.py core\engines\execution_objects\workflow.py core\tests\unit\test_draft_archive_paths.py core\tests\unit\test_unattended_recovery.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_unattended_recovery.py core\tests\unit\test_draft_archive_paths.py -q`: 31 passed.
- [x] `python -B -m pytest -q`: 203 passed.
- [x] `python -B -m compileall -q core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config tools\design`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.

**Follow-up**:
- [ ] `newplan/` remains local planning material and should not be committed unless explicitly requested.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-019
**Summary**: Patched the Step13 direct-run gap after `completed_with_review` support: Step13 now independently blocks when Step11 or Step12 still has unresolved review items unless the override config allows continuation.

**Completed**:
- [x] Added a Step13 plugin guard that reads pipeline state before import/generation work.
- [x] Blocked direct Step13 execution when Step11 or Step12 has `status=completed_with_review` and `pipeline.unattended_execution.continue_after_completed_with_review=false`.
- [x] Kept the existing override path: when the config is true, Step13 continues normally.
- [x] Added regression coverage for both direct-run blocking and override-enabled continuation.

**Verification**:
- [x] `python -B -m compileall pipeline\step_13_integration_validation\plugin.py core\tests\unit\test_unattended_recovery.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_unattended_recovery.py core\tests\integration\test_plugins.py -q`: 12 passed.
- [x] `python -B -m pytest -q`: 198 passed.

**Follow-up**:
- [ ] Existing unrelated save_id local change in `core/engines/execution_objects/workflow.py` remains intentionally uncommitted.
- [ ] `newplan/` remains local planning material and should not be committed unless explicitly requested.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-018
**Summary**: Implemented `newplan/step11_12_unattended_recovery_v4.md`: Step11/12 unattended recovery with correction queues, pause/resume logs, completed-with-review status, dependency skip records, and sync reduction.

**Completed**:
- [x] Added unattended execution config defaults under `settings/app.toml` / `core.config.loader`.
- [x] Added `unattended_recovery.py` helpers for FailureEvent, queue upsert, reproduction payloads, resume cursors, dependency skip, and pause/resume logs.
- [x] Preserved correction queue extra fields during JSON round trips.
- [x] Added execution-object automated remediation evidence and verification support.
- [x] Added `completed_with_review` across stage status, pipeline state, stage report, and `run_range()`.
- [x] Updated Step11/12 to write correction queues, summaries, pause/resume logs, and review-oriented statuses.
- [x] Added Step13 gating for unresolved Step11/12 review outputs and UI display for "需复核".
- [x] Added regression coverage for unattended recovery contracts.

**Verification**:
- [x] `python -B -m compileall -q core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config tools\design`: passed.
- [x] `python -B -m pytest core\tests\unit\test_unattended_recovery.py -q`: 6 passed.
- [x] `python -B -m pytest core\tests\unit\test_unattended_recovery.py core\tests\unit\test_config_loader.py core\tests\unit\test_model_adapters.py -q`: 26 passed.
- [x] `python -B -m pytest -q`: 196 passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.

**Follow-up**:
- [ ] Manual GUI spot-check remains useful for Step11/12 "需复核" display and pause/resume log visibility.
- [ ] Manual long-running Step11 run remains useful to validate real save-sync reduction and unattended correction queues against Unity.
- [ ] `newplan/` remains local planning material and should not be committed unless explicitly requested.
- [ ] Existing unrelated save_id local change in `core/engines/execution_objects/workflow.py` was preserved but should remain outside this task commit.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-017
**Summary**: Completed `newplan/image_cleanup_and_save_management.md`: cleaned stale Step07 style images, excluded binary files from current-draft snapshots, and added bulk save deletion.

**Completed**:
- [x] Cleared stale top-level PNG files in `stage_07/generated_images/` before regenerating style option images.
- [x] Removed unselected Step07 generated PNGs after manual style confirmation, while preserving selected fallback image paths.
- [x] Added snapshot-only binary suffix exclusion so `snapshot/full` skips PNG and other large binary files while formal save archives remain complete.
- [x] Added `delete_all_saves()` to remove every save and associated linked drafts, then clear `current_save_id`.
- [x] Added a "删除全部存档" action to the save manager dialog with confirmation and status refresh.
- [x] Added regression coverage for Step07 image cleanup, snapshot binary exclusion vs archive preservation, and bulk save deletion.

**Verification**:
- [x] `python -B -m compileall core\engines\generation.py core\ui\pipeline_panel.py core\save\manager.py core\ui\save_manager_dialog.py core\tests\unit\test_manual_style_confirmation.py core\tests\unit\test_draft_archive_paths.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_manual_style_confirmation.py core\tests\unit\test_draft_archive_paths.py -q`: 45 passed.
- [x] `python -B -m pytest -q`: 190 passed.
- [x] `python -B -m compileall -q core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config tools\design`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.

**Follow-up**:
- [ ] Manual GUI spot-check remains useful for confirming the "删除全部存档" button and Step07 confirmed-image cleanup.
- [ ] Manual large-save timing check remains useful to confirm snapshot sync improvement with real generated images.
- [ ] `newplan/image_cleanup_and_save_management.md` remains local planning material and should not be committed unless explicitly requested.
- [ ] Existing unrelated local change in `core/engines/execution_objects/workflow.py` was left untouched and should not be included in this task commit.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-016
**Summary**: Completed `plan/step07_prompt_editor_review.md`: fixed the Step07 prompt editor lifecycle bug where background AI callbacks could touch destroyed Tk widgets after the dialog closed.

**Completed**:
- [x] Added a `_cancelled` lifecycle flag to `StylePromptEditorDialog`.
- [x] Routed worker-thread UI callbacks through `_schedule_ui()`, which drops callbacks after cancel and tolerates destroyed Tk widgets.
- [x] Guarded AI success/error handlers so already-cancelled dialogs do not update conversation widgets.
- [x] Added the initial assistant greeting to `_history` so follow-up AI calls include the visible conversation context.
- [x] Added regression coverage for initial greeting history and cancelled AI callbacks.

**Verification**:
- [x] `python -B -m compileall core\ui\style_prompt_editor.py core\tests\unit\test_manual_style_confirmation.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_manual_style_confirmation.py -q`: 25 passed.
- [x] `python -B -m pytest -q`: 186 passed.
- [x] `python -B -m compileall -q core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config tools\design`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.

**Follow-up**:
- [ ] Manual GUI spot-check remains useful for closing the prompt editor while an actual AI completion request is pending.
- [ ] `plan/step07_prompt_editor_review.md` remains local planning material and should not be committed unless explicitly requested.
- [ ] Existing unrelated local change in `core/engines/execution_objects/workflow.py` was left untouched and should not be included in this task commit.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-015
**Summary**: Completed `plan/step07_prompt_editor_dialog.md`: added a Step07 prompt editor dialog for AI-assisted style prompt refinement and wired prompt overrides into one-shot style-image regeneration.

**Completed**:
- [x] Added `StylePromptEditorDialog` for Step07 "重新生成": users can review current prompts, chat with the configured completion AI, choose image count, and confirm generation.
- [x] Changed the Step07 style grid default selection to blank so users must explicitly select a final style before approval.
- [x] Added prompt-response parsing and `prompt_override.json` writing for refined prompts.
- [x] Added Step07 generation-layer consumption of `prompt_override.json`; the override is deleted after use and regenerated options are written through the same `style_options.json` / manifest path as default generation.
- [x] Preserved the old direct regenerate path for legacy fallback dialog flows.
- [x] Added regression coverage for prompt-block parsing, override file writing, and Step07 override consumption.

**Verification**:
- [x] `python -B -m compileall core\ui\style_prompt_editor.py core\ui\pipeline_panel.py core\engines\generation.py core\tests\unit\test_manual_style_confirmation.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_manual_style_confirmation.py -q`: 23 passed.
- [x] `python -B -m pytest -q`: 184 passed.
- [x] `python -B -m compileall -q core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config tools\design`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.

**Follow-up**:
- [ ] Manual GUI spot-check remains useful for the prompt editor dialog, real AI completion calls, and real image-generation timing.
- [ ] `plan/step07_prompt_editor_dialog.md` remains local planning material and should not be committed unless explicitly requested.
- [ ] Existing unrelated local change in `core/engines/execution_objects/workflow.py` was left untouched and should not be included in this task commit.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-014
**Summary**: Completed `plan/asset_type_fix_and_parallel_image_gen.md`: fixed asset type detection, enabled parallel Step07 style-image generation, enlarged style thumbnails, and added custom-template deletion.

**Completed**:
- [x] Fixed `_asset_type()` so `buildcraft` / `pickup` no longer match the `ui` substring while `ui_element` and `hud` still classify as UI.
- [x] Changed Step07 style image generation to use up to 5 parallel workers with per-style work directories to avoid shared PNG scan races.
- [x] Enlarged Step07 style thumbnails from 220x150 to 330x225 and widened style-card text wrapping to 330.
- [x] Added `delete_custom_template()` and a "删除自定义模板" action in the template viewer; builtin templates remain protected.
- [x] Added regression coverage for asset type boundaries, parallel style-image execution, worker limits, and custom template deletion.

**Verification**:
- [x] `python -B -m compileall core\engines\generation.py core\ui\pipeline_panel.py core\ui\app_window.py core\design\project_templates.py core\tests\unit\test_hades_quality_optimization.py core\tests\unit\test_manual_style_confirmation.py core\tests\unit\test_project_templates.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_hades_quality_optimization.py core\tests\unit\test_manual_style_confirmation.py core\tests\unit\test_project_templates.py -q`: 30 passed.
- [x] `python -B -m pytest -q`: 181 passed.
- [x] `python -B -m compileall -q core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config tools\design`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.

**Follow-up**:
- [ ] Manual GUI spot-check remains useful for Step07 style-card sizing and custom-template deletion dialogs.
- [ ] Manual Step04/Step07 reruns remain useful for real asset distribution and real image-generation timing.
- [ ] `plan/asset_type_fix_and_parallel_image_gen.md` remains local planning material and should not be committed unless explicitly requested.
- [ ] Existing unrelated local change in `core/engines/execution_objects/workflow.py` was left untouched and should not be included in this task commit.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-013
**Summary**: Completed the template gameplay-systems completion plan: all builtin project templates now carry valid gameplay system selections, weights, and core loops, with template-import fallback and regression coverage.

**Completed**:
- [x] Added gameplay-system inference and integer weight normalization helpers for legacy template import fallback.
- [x] Wired the fallback only into the template viewer import path, so blank projects still require manual gameplay-system selection.
- [x] Added a maintenance script to populate builtin template gameplay-system profiles by genre group.
- [x] Updated 38 builtin templates with non-empty `gameplaySystems.selected`, integer percent weights summing to 100, and readable `coreLoops`.
- [x] Added regression tests covering builtin template gameplay completeness, legacy template inference, and empty-project validation.

**Verification**:
- [x] `python -B -m pytest core\tests\unit\test_template_l5_expansion.py -q`: 8 passed.
- [x] `python -B -m pytest -q`: 178 passed.
- [x] `python -B -m compileall core\design\gameplay_systems.py core\ui\app_window.py core\tests\unit\test_template_l5_expansion.py tools\design\fill_template_gameplay_systems.py`: passed.
- [x] `python -B -m compileall -q core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config tools\design`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.
- [x] Builtin template JSON parse check: passed.
- [x] `python tools\design\fill_template_gameplay_systems.py`: updated/validated 38 builtin templates.

**Follow-up**:
- [ ] Manual GUI spot-check remains useful for loading representative templates and confirming "玩法系统校验：通过" in the validation panel.
- [ ] `plan/template_gameplay_systems_completion_plan.md` remains local planning material and should not be committed unless explicitly requested.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-012
**Summary**: Completed `plan/deferred_load_thread_fix.md`: moved startup save auto-load off the Tk main thread and preserved a visible restore status while the background load runs.

**Completed**:
- [x] Changed `_deferred_startup()` so automatic `load_save()` runs in a daemon background thread when a fresh draft needs to restore the current save.
- [x] Added a status-bar override API on `MainWindow` so "系统: 恢复上次存档中..." is not overwritten by the periodic status refresh.
- [x] Preserved startup integrity warning status after the background restore completes instead of blindly resetting to ready.
- [x] Kept automatic restore errors non-fatal and logged, matching the previous startup behavior.

**Verification**:
- [x] `python -B -m compileall core\ui\gui_app.py core\ui\main_window.py`: passed.
- [x] `python -B -m pytest -q`: 175 passed.
- [x] `python -B -m compileall -q core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.

**Follow-up**:
- [ ] Manual GUI startup check remains useful to confirm the window stays responsive while a large save restores and the status returns to normal after completion.
- [ ] `plan/deferred_load_thread_fix.md` remains local planning material and should not be committed unless explicitly requested.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-011
**Summary**: Completed `plan/step07_ui_fix_plan.md`: fixed Step07 style-confirmation UI behavior around fresh-session save loading, confirmed-style display, image preview, and Chinese style option labels.

**Completed**:
- [x] Added deferred startup auto-load of the current save into a fresh draft when no `draft_file_map.json` exists.
- [x] Updated the Step07 pipeline detail panel to show an approved-style summary instead of reopening the selection grid after confirmation.
- [x] Added a "重新选择风格" action that clears confirmation and refreshes the Step07 detail panel.
- [x] Added double-click enlarged preview for available Step07 style thumbnails.
- [x] Localized `STYLE_OPTION_PRESETS` titles and descriptions to Chinese for newly generated style options.
- [x] Added regression coverage for Chinese style option titles and approved style confirmation detection.

**Verification**:
- [x] `python -B -m compileall core\ui\gui_app.py core\ui\pipeline_panel.py core\engines\generation.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_manual_style_confirmation.py -q`: 19 passed.
- [x] `python -B -m pytest -q`: 175 passed.
- [x] `python -B -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.

**Follow-up**:
- [ ] Manual GUI click-through remains useful for fresh-session auto-load, Step07 confirmed-summary display, reselect flow, and double-click image preview.
- [ ] Existing generated `style_options.json` files keep their old titles/descriptions until Step07 is regenerated.
- [ ] `plan/step07_ui_fix_plan.md` remains local planning material and should not be committed unless explicitly requested.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-010
**Summary**: Completed `plan/perf_fix/CODE_REVIEW.md`: fixed follow-up UI state-cache issues in the performance optimization implementation.

**Completed**:
- [x] Added missing `_mark_state_changed()` after loading a design project from execution-object storage.
- [x] Removed duplicate post-try render/reset code from `_open_project_from_file()` and moved the open-status update into the successful load path.
- [x] Collapsed redundant `_mark_state_changed()` calls in risk and not-applicable toggles while preserving cache invalidation after the actual state change.

**Verification**:
- [x] `python -B -m compileall core\ui\app_window.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_draft_archive_paths.py -q`: 16 passed.
- [x] `python -B -m pytest -q`: 174 passed.

**Follow-up**:
- [ ] Manual GUI click-through remains useful for `open_project()` / `_open_project_from_file()` flows.
- [ ] `plan/perf_fix/` remains local planning material and should not be committed unless explicitly requested.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-009
**Summary**: Implemented `plan/perf_fix/FINAL_PLAN.md`: improved GUI startup responsiveness, reduced high-frequency design-workbench render work, and optimized save/load bookkeeping for unchanged workspaces.

**Completed**:
- [x] Deferred startup integrity checks and draft pruning until after `MainWindow` is created, with logged/visible startup-check warnings.
- [x] Added lazy `CommercialDesignApp` creation with an active-view guard.
- [x] Added result-panel cache invalidation and changed frequent checklist/risk/not-applicable interactions to skip `render_domains()`.
- [x] Added `current_save_id_readonly()` for close-time checks without save-system write side effects.
- [x] Made workspace migration report actual changes, added `mtime_ns` SHA256 cache reuse, limited current draft snapshots to 5, and added a `load_save` fast path.
- [x] Added focused save-manager tests for manifest idempotence, SHA256 cache reuse, snapshot trimming, and load fast path.

**Verification**:
- [x] `python -B -m compileall core\ui\gui_app.py core\ui\main_window.py core\ui\app_window.py core\save\manager.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_draft_archive_paths.py -q`: 16 passed.
- [x] `python -B -m pytest -q`: 174 passed.
- [x] `python -B -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.

**Follow-up**:
- [ ] Manual GUI timing/click-through remains useful for lazy design-panel creation and high-frequency design-workbench interactions.
- [ ] `plan/perf_fix/` remains local planning material and should not be committed unless explicitly requested.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-008
**Summary**: Completed `plan/step07_08_merge/BUGFIX_PLAN.md`: fixed Step07/08 merge follow-up issues around output function numbering, legacy confirmation wrapper, UI fallback routing, and legacy skip-gate messaging.

**Completed**:
- [x] Renamed post-merge development output functions so Step08-15 dispatch to matching `_stage8_outputs()` through `_stage15_outputs()`.
- [x] Removed legacy `_stage8_art_style_confirmation_outputs()` to prevent duplicate `style_options.json` writes into `stage_08`.
- [x] Fixed GUI confirmation fallback so only Step07 routes confirmation output to `stage_07`.
- [x] Updated `--skip-gate-08` help text and runtime warning to point users to `--skip-gate-07`.
- [x] Updated direct unit-test calls and added a dispatch regression for Step08-11.

**Verification**:
- [x] `python -B -m compileall core\engines\generation.py core\main.py core\ui\pipeline_panel.py`: passed.
- [x] `python -B -m pytest core\tests\unit\test_manual_style_confirmation.py core\tests\unit\test_hades_quality_optimization.py -q`: 26 passed.
- [x] `python -B -m pytest -q`: 170 passed.

**Follow-up**:
- [ ] `sunny_girl_image2.png` remains untracked local output and should not be committed unless explicitly requested.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-007
**Summary**: Implemented `plan/step07_08_merge`: merged Step07 art style generation and Step08 confirmation into one Step07 gate, renumbered the development pipeline to Step00-16, and verified the full suite.

**Completed**:
- [x] Step07 now writes `style_options.json`, scored/recommended options, `generation_log.json`, and `style_confirmation.json` in the same stage.
- [x] Manual style confirmation now returns `waiting_confirmation` from Step07; confirmed Step07 reruns reuse existing outputs instead of triggering another real image generation.
- [x] Added legacy compatibility for old `stage_08/style_confirmation.json` migration and both `--skip-gate-07` / `--skip-gate-08`.
- [x] Renumbered plugin folders, `core.registry`, `pipeline/_registry.json`, artifact registry, dependency graph, GUI groups, old workbench metadata, docs, and tests from Step08 onward.
- [x] Removed independent `pipeline/step_08_art_style_confirmation/`; Step08 is now Program Plan and Step16 is Migration Audit.

**Verification**:
- [x] `python -B -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.
- [x] `python -B -m pytest core\tests\unit\test_manual_style_confirmation.py core\tests\integration\test_plugins.py -q`: 21 passed.
- [x] `python -B -m pytest -q`: 169 passed.
- [x] Dependency graph check: 17 nodes, no errors, final artifact `stage_16.migration_audit_bundle`.

**Follow-up**:
- [ ] GUI visual click-through for the embedded Step07 style grid was not manually performed in this CLI session.
- [ ] `sunny_girl_image2.png` remains untracked local output and should not be committed unless explicitly requested.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

## 历史会话摘要

**Date**: 2026-06-28
**ID**: 2026-06-28-006
**Summary**: Created complete Step07/08 merge implementation plan based on user feedback that running Step07 leaves users unaware images are generated.

**Completed**:
- [x] Reviewed Factorio project quality: A grade (95/100), all metrics passed except entity coverage 55% vs 75% target.
- [x] Diagnosed Step07 issue: images successfully generated but user didn't know because Step08 confirmation gate requires separate execution.
- [x] Confirmed root cause: Step07/08 separation creates UX gap - users expect immediate confirmation after generation.
- [x] Gathered detailed requirements through 20+ questions covering CLI/GUI behavior, regeneration flow, folder renaming, compatibility.
- [x] Created comprehensive implementation plan: 4 phases (core 2-3h + GUI 3-4h + advanced 2-3h + migration 1-2h), 8-10h total.
- [x] Organized all planning documents in `plan/step07_08_merge/` folder with README index.

**Key Requirements Confirmed**:
- Merge Step07 + Step08 into single "Art Style Generation & Confirmation" step (18 steps → 17 steps)
- Generate 5 style options (currently 3) with AI scoring to mark recommended
- CLI: block waiting for confirmation file; GUI: auto-switch to embedded style panel (3x2 grid)
- Regenerate feature: show current prompt (read-only) + user input modifications + AI-assisted refinement
- Rename step folders: step_09→step_08 through step_17→step_16
- Backward compatibility: auto-migrate old Step08 state, support both --skip-gate-07 and --skip-gate-08

**Verification**:
- [x] Factorio draft located: `drafts/20260628_112840_34072/`
- [x] Images confirmed generated: 2 PNGs (STYLE-01 2.2MB, STYLE-02 2.4MB) at stage_07/generated_images/
- [x] Quality metrics: question_coverage=1.0, binding_rate=1.0, placeholder_rate=0.0, asset_count=319, entity_coverage=0.55
- [x] Implementation plan includes: detailed tasks, testing strategy, risk mitigation, rollback plan, 40+ checklist items

**Follow-up**:
- [ ] User to review `plan/step07_08_merge/IMPLEMENTATION_PLAN.md` before starting implementation
- [ ] Estimated 8-10 hours for full implementation including all phases
- [ ] Success criteria: users immediately see confirmation UI after Step07 generates images
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

## 历史会话摘要

**Date**: 2026-06-28
**ID**: 2026-06-28-005
**Summary**: Fixed BUG-NEW-001: Codex image stdout PNG path extraction now allows backticks inside filenames instead of relying on session/new-file fallbacks.

**Completed**:
- [x] Allowed backticks in `_png_paths_from_text()` path segments while keeping backticks valid as wrapping delimiters.
- [x] Added regression coverage for `saved: \`...\style\`board.png\`` so saved-path parsing succeeds directly.

**Verification**:
- [x] `python -B -m pytest core\tests\unit\test_codex_image_tool.py -q`: 6 passed.
- [x] `python -B -m pytest -q`: 168 passed.
- [x] `python -B -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.

**Follow-up**:
- [ ] `sunny_girl_image2.png` remains untracked local output and should not be committed unless explicitly requested.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

## 历史会话摘要

**Date**: 2026-06-28
**ID**: 2026-06-28-004
**Summary**: Completed `bug_collection_20260628_step07_edge_cases`: handled remaining Step07 edge cases around deleted files, cleanup logging, path parser documentation, and trailing-backtick path tests.

**Completed**:
- [x] Added `OSError` protection in `_new_style_pngs()` so files deleted during scan are skipped.
- [x] Added non-blocking warning logging when temporary style images cannot be removed after successful copy fallback.
- [x] Documented that Codex stdout path regex can match malformed paths, with `path.is_file()` filtering them out.
- [x] Confirmed non-`OSError` exceptions should keep propagating.
- [x] Added regression coverage for deleted files during scan and filenames with a backtick immediately before `.png`.
- [x] Updated local plan status; `plan/` remains ignored.

**Verification**:
- [x] Targeted Step07/Codex image tests: 21 passed.
- [x] `python -B -m pytest -q`: 167 passed.
- [x] `python -B -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.

**Follow-up**:
- [ ] Real Codex CLI Step07 image generation was not rerun.
- [ ] `sunny_girl_image2.png` remains untracked local output and should not be committed unless explicitly requested.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

**Date**: 2026-06-28
**ID**: 2026-06-28-003
**Summary**: Completed `bug_collection_20260628_step07_prompt_fix`: fixed Step07 image-output edge cases, tightened Codex PNG/session parsing, preserved serial image generation, and verified the full suite.

**Completed**:
- [x] Added safe Step07 image placement: `replace()` failure falls back to copy, and still-locked targets fall back to a unique image filename.
- [x] Tightened new PNG detection using path+mtime snapshots and operation-start filtering.
- [x] Tightened Codex CLI PNG path parsing to prefer saved/generated/output lines.
- [x] Restricted Codex session-id matching to UUID-form ids.
- [x] Preserved inner backticks in saved image paths, added fallback asset labels, and kept Step07 image generation serial.
- [x] Updated local plan status; `plan/` remains ignored.

**Verification**:
- [x] Targeted Step07/Codex image tests: 18 passed.
- [x] `python -B -m pytest -q`: 164 passed.
- [x] `python -B -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.

**Follow-up**:
- [ ] Real Codex CLI Step07 image generation was not rerun to avoid re-triggering Windows sandbox helper issues.
- [ ] `sunny_girl_image2.png` remains untracked local output and should not be committed unless explicitly requested.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

**Date**: 2026-06-28
**ID**: 2026-06-28-002
**Summary**: Continued the interrupted Step07/Codex image-generation follow-up, forced Step07 image generation back to safe serial execution, and recorded a hard ban on broad Codex/sandbox process termination.

**Completed**:
- [x] Removed Step07 image-generation concurrency and the unused per-worker Codex home helper from the abandoned parallel approach.
- [x] Added a regression test proving Step07 image generation workers stay at `1`.
- [x] Recorded the process-cleanup anti-pattern: never terminate `codex`, `node`, or sandbox-related processes by broad process-name/PID matching; use read-only diagnostics and user confirmation.

**Verification**:
- [x] Targeted Step07/Codex image tests: 12 passed.
- [x] `python -B -m pytest -q`: 158 passed.
- [x] `python -B -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.

**Follow-up**:
- [ ] If a Codex/sandbox helper popup appears again, inspect only; do not kill processes unless the target is proven outside the active session process tree and the user confirms.
- [ ] `sunny_girl_image2.png` remains untracked local output and should not be committed unless explicitly requested.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

## 历史会话摘要

**Date**: 2026-06-28
**ID**: 2026-06-28-001
**Summary**: Executed `pending_tasks_20260628`: committed Codex CLI subprocess/image fixes, added orphan draft and snapshot pruning, cleaned old drafts, and verified real Codex CLI image generation through Step07.

**Completed**:
- [x] Removed the obsolete `-` stdin argument from Codex CLI subprocess calls and passed `stdin=None` so `input=` can pipe prompts without conflicting with `DEVNULL`.
- [x] Added UTF-8 replacement decoding for Codex subprocess output on Windows.
- [x] Updated `CodexCLIImageGenerator` to recursively find PNGs under `CODEX_HOME/generated_images/**`, matching current `image_gen` output.
- [x] Treated drafts linked to missing save directories as pruneable orphan drafts.
- [x] Added `prune_draft_snapshots()` and GUI startup cleanup for pruneable draft snapshots only.
- [x] Ran one-time cleanup: 34 pruneable drafts deleted; `drafts/` reduced to 16 dirs / about 551.51 MB.
- [x] Verified real Codex CLI image generation with a rigorous Lumen Archive prompt and Step07 end-to-end run.
- [x] Created commits `f1eb62f` and `7090277`.

**Verification**:
- [x] Targeted Codex image/model/draft tests: 18 passed.
- [x] `python -B -m pytest -q`: 154 passed.
- [x] `python -B -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.
- [x] `codex.cmd --version`: `codex-cli 0.142.3`.
- [x] Step07 manifest in `drafts/codex_template_2d_redesign_fixes_validation` shows 5 generated images with status `success`.

**Follow-up**:
- [ ] User should visually review generated images under `drafts/codex_template_2d_redesign_fixes_validation/outputs/artifacts/stage_07/generated_images/`.
- [ ] Future sandboxed Codex CLI image runs need an authenticated `CODEX_HOME` or outside-sandbox execution; `C:\Users\CodexSandboxOffline\.codex` has no credentials.
- [ ] Continue excluding `plan/`, runtime drafts, local AI config files, and `sunny_girl_image2.png` from commits unless explicitly requested.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

**Date**: 2026-06-27
**ID**: 2026-06-27-005
**Summary**: Executed `step07_image_fix_and_ui`: fixed Step07 image generation activation/routing, added Codex CLI image generation support, and embedded the Step07/08 style confirmation UI.

**Completed**:
- [x] Default and empty v3 image categories now activate `codex_cli_image`, so Step07 image generation is not disabled by default.
- [x] Added `CodexCLIImageGenerator` using `codex.cmd`/`codex.exe`/`codex` priority and `codex exec -` to drive the local image_gen-capable Codex CLI.
- [x] Routed image generation by active image entry type: Codex CLI for `codex_cli_image`, HTTP Image2 API otherwise.
- [x] Reused the same image-generator routing for later generated image manifests.
- [x] Embedded Step07/08 style option cards in the pipeline detail panel with image previews, selection, notes, confirm, regenerate, and log-area compression.
- [x] Waiting Step08 style confirmation now selects the embedded detail panel when style options are available; the old dialog remains as fallback.
- [x] Added regression tests for image activation, Codex image tool copying, generator routing, and style option JSON discovery.

**Verification**:
- [x] Targeted Step07/image/UI tests: 34 passed.
- [x] `python -B -m pytest -q`: 151 passed.
- [x] `python -B -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.
- [x] `git diff --check`: passed with only CRLF working-copy warnings.

**Follow-up**:
- [ ] Manually click-test GUI Step07/08 embedded style grid: confirm, regenerate, layout, and log area.
- [ ] Continue excluding `plan/`, runtime drafts, local AI config files, and `sunny_girl_image2.png` from commits unless explicitly requested.
- [ ] CC-Panes shared memory was skipped because required environment variables were absent.

---

**Date**: 2026-06-27
**ID**: 2026-06-27-004
**Summary**: Completed `ai_config_ui_redesign/BUGS.md` follow-up fixes for initialization data loss, label preservation, compatibility image aliasing, and save-side effects.

**Completed**:
- [x] Fixed `AIConfigUnifiedDialog._switch_tab()` so first render does not write empty UI fields into the first dev entry.
- [x] Preserved migrated/custom labels when changing config type, while still updating default labels.
- [x] Removed unused imports from the redesigned AI config dialog.
- [x] Gave each compatibility `AIProfile` an independent `ImageConfig` copy.
- [x] Removed `save_ai_config()` mutation of the caller's `AIConfig.active_profile_id`.
- [x] Added regression tests for independent image configs, save non-mutation, v2 auto-writeback, and label preservation.

**Verification**:
- [x] Targeted AI config tests: 32 passed.
- [x] `python -B -m pytest -q`: 147 passed.
- [x] `PYTHONPYCACHEPREFIX=.cache\pycache python -B -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.
- [x] `git diff --check`: passed with only the existing CRLF working-copy warning.

**Follow-up**:
- [ ] Manually click-test the redesigned AI config dialog: first-open migration preservation, tabs, active highlight, Codex file fields, custom JSON validation, save/reopen.
- [ ] Keep `settings/ai_config.json`, old `settings/api_config.toml`, and `settings/ai_profiles.json` local and ignored.
- [ ] Continue excluding `plan/`, bug documents, and runtime drafts from commits.
- [ ] CC-Panes shared memory was skipped because all required environment variables were absent.

---

**Date**: 2026-06-27
**ID**: 2026-06-27-003
**Summary**: Executed `ai_config_ui_redesign`: upgraded AI config to v3 `dev` / `image` / `completion` API categories and redesigned the unified AI config dialog into three tabs.

**Completed**:
- [x] Added v3 schema primitives in `core/config/ai_config_schema.py` and kept `core/config/ai_config.py` as the load/save/migration facade.
- [x] Migrated v2 Profile data into `dev`, `image`, and `completion` categories while preserving `get_active_profile()` compatibility.
- [x] Updated loader, validator, Step02 supplement selection, image-generation enablement, image API helper, migration tool, and config example for v3.
- [x] Rewrote `AIConfigUnifiedDialog` into `开发API` / `生图API` / `补全API` tabs with active-entry highlight, CLI read-only panels, API fields, custom JSON, and Codex path fields.
- [x] Updated README, AI config guide, AI_README, and AI config tests.

**Verification**:
- [x] Targeted AI config tests: 29 passed.
- [x] `python -B -m pytest -q`: 144 passed.
- [x] `PYTHONPYCACHEPREFIX=.cache\pycache python -B -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production tools\config`: passed.
- [x] Targeted `py_compile`: passed.
- [x] `git diff --check`: passed with only the existing CRLF working-copy warning.

**Follow-up**:
- [ ] Manually click-test the redesigned AI config dialog: tabs, active highlight, local CLI read-only panel, Codex file fields, custom JSON validation, save/reopen.
- [ ] Keep `settings/ai_config.json`, old `settings/api_config.toml`, and `settings/ai_profiles.json` local and ignored.
- [ ] Continue excluding `plan/`, bug documents, and runtime drafts from commits.
- [ ] CC-Panes shared memory was skipped because all required environment variables were absent.

---

**Date**: 2026-06-27
**ID**: 2026-06-27-002
**Summary**: Executed `ui_improvement_v1`: moved AI visibility into the main-window bottom status bar, enhanced the unified AI config dialog, audited UI component usage, updated docs, and isolated tests from local ignored AI config.

**Completed**:
- [x] Added bottom status bar in `core/ui/main_window.py` for active AI Profile/adapter, DevFlow progress, and system state.
- [x] Wired status-bar AI click to unified AI config and progress click to the pipeline panel with first-incomplete-step selection.
- [x] Enhanced `core/ui/ai_config_unified_dialog.py` with active Profile highlight, detail validation status, async CLI checks, `应用`, save toast, and close cleanup.
- [x] Audited UI components: `bottom_panel.py` and `embedded_interview.py` are still referenced; `workbench.py` is retained as a deletion-audit candidate.
- [x] Updated `README.md`, `docs/AI_CONFIG_GUIDE.md`, and `AI_README.md` for the new status-bar entry.
- [x] Added test isolation in `core/tests/conftest.py` so local ignored `settings/ai_config.json` cannot pollute unit tests.

**Verification**:
- [x] `python -B -m py_compile core\ui\main_window.py core\ui\ai_config_unified_dialog.py core\tests\conftest.py`: passed.
- [x] Targeted local-config-polluted regression tests: 3 passed.
- [x] `python -B -m pytest -q`: 142 passed.
- [x] UI module import smoke test: passed.
- [x] `git diff --check`: passed with only the existing CRLF working-copy warning.

**Follow-up**:
- [ ] Manually spot-check the bottom status bar, AI config save/apply flow, and progress jump in the GUI.
- [ ] Keep `settings/ai_config.json`, old `settings/api_config.toml`, and `settings/ai_profiles.json` local and ignored.
- [ ] Continue excluding `plan/`, bug documents, and runtime drafts from commits.
- [ ] CC-Panes shared memory was skipped because all required environment variables were absent.

---

**Date**: 2026-06-27
**ID**: 2026-06-27-001
**Summary**: Executed `ai_config_optimization_v2`: unified AI config into `settings/ai_config.json`, bound Profile to Adapter/LLM/Image, added migration, validation, GUI, status indicator, tests, and docs.

**Completed**:
- [x] Added `core/config/ai_config.py`, `core/config/validator.py`, migration tooling, Profile-bound adapters, loader compatibility, unified AI config GUI, docs, and tests.
- [x] Updated Step02 supplement, Stage12 execution, and image generation to prefer active AI Profile.

**Verification**:
- [x] Targeted AI config/adapter tests: 27 passed.
- [x] Adapter/Step02/image/manual-gate related tests: 42 passed.
- [x] `python -B -m pytest -q`: 142 passed.
- [x] `PYTHONPYCACHEPREFIX=.cache\pycache python -B -m compileall ...`: passed.
- [x] `git diff --check`: passed with only the existing CRLF working-copy warning.

**Follow-up**:
- [ ] Manually spot-check the main-window AI status and unified AI config save/activate flow.
- [ ] Keep `settings/ai_config.json`, old `settings/api_config.toml`, and `settings/ai_profiles.json` local and ignored.
- [ ] Continue excluding `plan/`, bug documents, and runtime drafts from commits.
- [ ] CC-Panes shared memory was skipped because all required environment variables were absent.

---

**Date**: 2026-06-26
**ID**: 2026-06-26-006
**Summary**: Executed `ai_config_manager`: added named AI profiles, GUI configuration dialog, profile-first config loading, and profile-controlled image generation.

**Completed**:
- [x] Added `core/config/ai_profiles.py` for ignored `settings/ai_profiles.json`, active profile management, LLM/image config parsing, and secret masking.
- [x] Updated `core/config/loader.py` so active profiles override `llm` / `image` / `image2`, with fallback to old `api_config.toml` when profile config is absent or incomplete.
- [x] Added `core/ui/ai_config_dialog.py` and wired an `AI 配置` button into `core/ui/pipeline_panel.py`.
- [x] Changed image generation enablement to prefer the active profile image switch while preserving the legacy env-var fallback when no profile file exists.
- [x] Updated image tooling to use the configured image model instead of a hardcoded default.
- [x] Ignored `settings/ai_profiles.json` so local API keys are not committed.
- [x] Added regression tests for profile fallback, override, image2 mapping, image enablement, and default profile file creation.
- [x] Self-check fix: isolated the legacy `image2` fallback test from local `settings/ai_profiles.json` so future personal profiles cannot pollute test results.

**Verification**:
- [x] Targeted config/image tests: 15 passed.
- [x] `python -B -m py_compile ...`: passed.
- [x] `PYTHONPYCACHEPREFIX=.cache\pycache python -B -m compileall ...`: passed.
- [x] `python -B -m pytest -q`: 129 passed.
- [x] `git diff --check`: passed with only the existing CRLF working-copy warning.

**Follow-up**:
- [ ] Manually spot-check the GUI `AI 配置` dialog save/activate flow.
- [ ] Keep `settings/ai_profiles.json` local and ignored because it may contain API keys.
- [ ] This turn requested memory sync only; the code changes are not committed yet.
- [ ] Continue excluding `plan/`, bug documents, and runtime drafts from commits.
- [ ] CC-Panes shared memory was skipped because all required environment variables were absent.

---

**Date**: 2026-06-26
**ID**: 2026-06-26-005
**Summary**: Executed `template_2d_redesign/PLAN_FIXES.md`: fixed template replacement filenames, service count, Axiom Verge replacement, and concrete-node reference rules.

**Completed**:
- [x] Renamed 18 replacement template files so public `fileName` matches `template.id`.
- [x] Deleted `builtin_large_service_splatoon_3.json`; public `large_service` templates now total 5.
- [x] Replaced the duplicate 3A Hollow Knight slot with `builtin_3a_axiom_verge.json`, including metadata, notes, and entity ids.
- [x] Synced `template_index.json` with new file names, Axiom Verge metadata, and large-service ordering.
- [x] Updated `core/tests/unit/test_template_l5_expansion.py` to derive concrete nodes from `builtin_indie_hades_l5_complete.json`.
- [x] Added tests for removed old files, `fileName == id + .json`, and public scale counts: iaa=9 / indie=10 / midcore=3 / 3a=9 / large_service=5.

**Verification**:
- [x] All project template JSON parsed successfully.
- [x] Static audit: 36 public templates, no removed old replacement files, no public file/id mismatch.
- [x] `python -B -m pytest core\tests\unit\test_template_l5_expansion.py -q`: 3 passed.
- [x] D4 export from new `builtin_3a_axiom_verge.json` followed by Step00-08: all success.
- [x] Pipeline quality: 104 entities, requirement binding 1.0, placeholder rate 0, Step05 blocking 0.
- [x] `python -B -m pytest -q`: 122 passed.
- [x] `PYTHONPYCACHEPREFIX=.cache\pycache python -B -m compileall ...`: passed.
- [x] `git diff --check`: passed with only the existing CRLF working-copy warning.

**Follow-up**:
- [ ] This turn requested memory sync only; the code changes are not committed yet.
- [ ] Runtime validation draft `drafts\codex_template_2d_redesign_fixes_validation` should not be committed.
- [ ] `plan/` remains local execution material and should not be committed.
- [ ] CC-Panes shared memory was skipped because all required environment variables were absent.

---

**Date**: 2026-06-26
**ID**: 2026-06-26-004
**Summary**: Executed `template_2d_redesign`: rebuilt all 37 public templates as 2D L5 templates.

**Completed**:
- [x] All 37 public templates now set `profile.dimension = "2D"`; `template_index.json` mirrors `dimension = "2D"`.
- [x] Public template `qualityClaim` values are now `L5_complete_consistent`, with no public `L4_only_filled` entries.
- [x] Planned 3D/non-target replacements were rewritten as 2D reference games while keeping stable file names.
- [x] Every public template covers all 39 concrete nodes: 26 system_concrete nodes with 3 entities each and 13 content_concrete nodes with 2 entities each.
- [x] `builtin_indie_hades.json`, currently part of the public index, was also upgraded to 2D L5.
- [x] `core/tests/unit/test_template_l5_expansion.py` now validates public template count, index sync, 2D dimension, L5 claim, old 3D names, concrete coverage, and entity shape.

**Verification**:
- [x] All project template JSON parsed successfully.
- [x] Static public-template audit: 37 public entries, no `L4_only_filled`, no old 3D public names.
- [x] `python -B -m pytest core\tests\unit\test_template_l5_expansion.py -q`: 3 passed.
- [x] D4 export from the rebuilt Celeste template followed by Step00-08: all success.
- [x] Pipeline quality for the validation draft: 104 entities, requirement binding 1.0, placeholder rate 0, Step05 blocking 0.
- [x] `python -B -m pytest -q`: 122 passed.
- [x] `PYTHONPYCACHEPREFIX=.cache\pycache python -B -m compileall ...`: passed.
- [x] `git diff --check`: passed with only the existing CRLF working-copy warning.

**Follow-up**:
- [ ] Optional GUI spot-check of the template list to confirm renamed public entries display as expected.
- [ ] Validation draft `drafts\codex_template_2d_redesign_validation` is runtime output and should not be committed.
- [ ] Continue excluding `plan/` and bug documents from commits.
- [ ] CC-Panes shared memory was skipped because all required environment variables were absent.

---

**日期**：2026-06-26
**ID**：2026-06-26-003
**摘要**：执行 `directory_cleanup_analysis`：目录清理、存档进度总数修复与 plan 忽略规则统一

**完成内容**：
- ✅ 删除临时验证存档 `save_20260626_080638_065042` 及其关联 draft
- ✅ 剩余正式存档进度显示更新为 `9/18`
- ✅ 删除旧 plan 目录，保留最近 3 个计划目录和本轮分析文件
- ✅ 删除根目录旧 `.pytest_cache/` 和 legacy `sandbox/outputs/`
- ✅ `.gitignore` 改为统一忽略 `plan/`
- ✅ `core/save/manager.py`、`core/ui/save_manager_dialog.py` 的存档进度总数改为动态 `max_step_number()+1`
- ✅ `knowledge/ucos/scripts` 中旧 `pipeline_progress.total=16` 改为动态总步数

**自查修复**：
- ✅ 发现 `.gitignore` 实际未统一忽略 `plan/`，已修复
- ✅ 搜索发现 ucos 初始化/迁移脚本仍会写入旧 16 步进度，已修复
- ✅ 自检生成的 `core/__pycache__` 已路径校验后删除
- ✅ pytest 临时目录未强删：当前 63 个都未超过 7 天，继续依赖现有 7 天自动清理策略

**验证**：
- ✅ 关联回归测试：9 passed
- ✅ `python -B -m pytest -q`：121 passed
- ✅ `PYTHONPYCACHEPREFIX=.cache\pycache python -B -m compileall ...`：通过
- ✅ `python -B -m flake8 ... --select=F`：通过
- ✅ 旧 16 步进度字面量搜索无命中
- ✅ `.pytest_cache`、`sandbox/outputs`、源树 `__pycache__` 均不存在

**后续关注**：
- [ ] pytest `sandbox/pytest_*` 目录仍有 63 个，未超过 7 天，后续等 Windows 锁释放或自动清理
- [ ] drafts 仍有 42 个，当前都在 7 天内；后续可单独做自动清理策略
- [ ] 提交前确认 `plan/directory_cleanup_analysis.md` 和剩余 plan 目录不进入暂存区
- [ ] CC-Panes 共享记忆池本次因环境变量缺失未写入

---

**日期**：2026-06-26
**ID**：2026-06-26-002
**摘要**：执行 `manual_style_confirmation`：Step07/08 美术风格生成与人工确认门禁

**完成内容**：
- ✅ 新增 `pipeline/step_07_art_style_generation/`，输出风格候选、确定性 PNG 预览和生成日志
- ✅ 新增 `pipeline/step_08_art_style_confirmation/`，支持 `waiting_confirmation` 人工门禁、自动跳过门禁和已有确认复用
- ✅ 原 Step07-15 后移为 Step09-17，同步 core registry、pipeline registry、artifact layer registry、dependency graph 和 README
- ✅ CLI 新增 `--skip-all-gates` / `--skip-gate-08`，`run_range()` 使用动态 `max_step_number()`
- ✅ GUI 增加风格确认对话框、跳过人工确认选项和等待确认后的续跑/重新生成流程
- ✅ 修复 Step08 重跑时 `run_import_step()` 重置阶段目录导致 `style_confirmation.json` 被删除的问题
- ✅ 更新 AI 入口文档和 `knowledge/ai_memory/project_understanding` 中旧的 16 阶段/旧阶段号记忆

**验证**：
- ✅ 新增/关联回归测试：9 passed；新增人工确认测试：6 passed
- ✅ `python -B -m pytest -q`：120 passed
- ✅ 真实流水线 Step00-08：全部 success（使用 `--skip-all-gates` 验证自动通过路径）

**后续关注**：
- [ ] GUI 需要人工点选 Step08 对话框做一次视觉/交互验收
- [ ] 提交前确认 `plan/manual_style_confirmation/`、其他 `plan/` 临时目录、bug 文档和 `settings/api_config.toml` 不进入暂存区
- [ ] CC-Panes 共享记忆池本次因环境变量缺失未写入

---

**日期**：2026-06-26
**ID**：2026-06-26-001
**摘要**：执行 `universal_genre_coverage`：通用品类覆盖与 Step02 liveops 元数据过滤

**完成内容**：
- ✅ Step00 `_genre_key()` 改为有序规则推断，避免宽泛 shooter/puzzle/arena 抢先命中具体品类
- ✅ Step00 为计划列出的 17 个市场品类补齐 `GENRE_DEFAULT_EVIDENCE`
- ✅ Step02 governance node 过滤改为项目元数据感知：文档/帮助节点始终排除，liveops-only 节点只在明确买断/离线/单次发布项目中排除
- ✅ Step02 项目分类只读取 profile、project metadata、商业模式/运营模式 selections，避免 raw text 中节点 ID 污染分类
- ✅ 同步保留上一轮未提交修复：documentation 需求过滤、存档管理对话框文案/import 清理及相关回归测试

**验证**：
- ✅ 关联回归测试：3 passed
- ✅ `python -B -m pytest -q`：114 passed
- ✅ Hades 与 Stardew Valley 临时验证存档 Step00-08 全部 success

**后续关注**：
- [ ] Stage05 warning_count 当前为 1，属于非阻断 warning；如需归零可单独治理 L4-derived requirement 启发式
- [ ] 提交前确认 `plan/universal_genre_coverage/`、其他 `plan/` 临时目录、bug 文档和 `settings/api_config.toml` 不进入暂存区
- [ ] CC-Panes 共享记忆池本次因环境变量缺失未写入

---

**日期**：2026-06-25
**ID**：2026-06-25-002
**摘要**：执行 `hades_l5_step0008_opt` v2：Step00-08 质量优化

**完成内容**：
- ✅ PLAN-B：Step04 资产识别补充英文 environment 关键词，支持 `room` / `level` / `chamber` / `dungeon` / `tileset`
- ✅ PLAN-C：Step07 任务分类移除 `schema=...` 元数据干扰，并跳过 `documentation_*` 治理需求
- ✅ PLAN-A：Step02 `missing_entities` 优先输出真实 expected node_id，不再只给 `UNMAPPED-NODE-xxx`
- ✅ PLAN-A 补强：Step02 supplement request 接收 `missing_node_ids`，fallback 能按真实缺失 node_id 生成有限补全实体
- ✅ PLAN-B 补强：Step04 优先消费 Stage02 冻结/补全后的实体，补全 room/scene 能级联生成 environment 资产
- ✅ PLAN-D：Step00 `roguelike_action` 补充 CQ-011 运行时流程 genre evidence
- ✅ 新增回归测试覆盖英文 environment 资产、documentation 过滤、真实缺失节点追踪和 CQ-011 evidence

**自查修复**：
- ✅ black 格式化后重新跑全量测试和静态检查
- ✅ 真实配置重跑 Step00-08：`drafts\20260625_122737_33376`，步骤 00-08 全部 success
- ✅ 质量指标：question coverage 1.0，Step02 entity coverage 0.8447，asset_count 132，environment 4，Stage06 PASS，Step07 documentation 5

**验证**：
- ✅ 关联回归测试：75 passed
- ✅ `python -m pytest -q`：111 passed
- ✅ black / flake8 / `py_compile`：本轮触碰文件通过
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production`：通过
- ✅ `git diff --check`：通过（仅 CRLF 工作区提示）

**后续关注**：
- [ ] 若后续要求 Step02 覆盖率接近 1.0，可继续扩展 missing-node fallback 上限或补齐源模板 L5 实体；本轮计划目标已达成
- [ ] 提交前确认 `plan/hades_l5_step0008_opt/`、其他 `plan/` 临时目录和 `settings/api_config.toml` 不进入暂存区
- [ ] CC-Panes 共享记忆池本次因环境变量缺失未写入

---

**日期**：2026-06-25
**ID**：2026-06-25-001
**摘要**：集中缓存目录与 Step05 绑定质量优化

**完成内容**：
- ✅ `.cache/pycache`、`.cache/pytest`、`.cache/mypy` 成为 Python/pytest/mypy 缓存集中目录
- ✅ `sitecustomize.py`、`core/__init__.py`、GUI 入口和 `conftest.py` 自动设置 pycache 前缀
- ✅ Step00 全量重跑时清理同一 active save 关联 sibling draft 的旧 `outputs/artifacts`
- ✅ Stage02 freeze contract 写入 `entities`、`systems`、`entity_stats`，Stage03 自动补齐需求系统绑定
- ✅ Step02 supplement 记录触发原因，并按未映射节点优先级补齐 `expected_kind`
- ✅ 新增/扩展测试覆盖缓存集中化、draft 清理、Step03 绑定和 supplement 触发诊断

**自查修复**：
- ✅ 清理源树分散 `__pycache__`，保留 `.cache/` 作为唯一缓存落点
- ✅ 修复 `core/ui/workbench.py` 中外部运行前同步/清理逻辑的局部结构问题
- ✅ 确认 `plan/cache_centralization/`、`plan/step05_optimization/` 仍为本地执行材料，不进入暂存区

**验证**：
- ✅ `python -m pytest -q`：105 passed
- ✅ 目标 Step05/L5 回归：64 passed
- ✅ black / flake8 / 目标 mypy / `py_compile`：通过
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production`：通过
- ✅ `git diff --check`：通过（仅 CRLF 工作区提示）

**后续关注**：
- [ ] 直接对 `core/engines/generation.py` 跑全量 mypy 仍有既有历史弱类型问题，后续可独立治理
- [ ] 后续 Python 工具运行继续保持 `PYTHONPYCACHEPREFIX=.cache/pycache`
- [ ] 提交前确认 bug 文档、`plan/` 临时执行目录和 `settings/api_config.toml` 不进入暂存区
- [ ] CC-Panes 共享记忆池本次因环境变量缺失未写入

---

**日期**：2026-06-24
**ID**：2026-06-24-006
**摘要**：修复 Codex sandbox、图片配置与 pytest basetemp 清理

**完成内容**：
- ✅ Step02 Codex sandbox 从非法 `none` 改为 `read-only`，兼容 Codex CLI 0.141.0
- ✅ `image2` / `image` / `llm` API 配置支持继承回退，旧图片工具改用 `core.config.loader`
- ✅ Stage09/Stage11 输出 `generated_images_manifest.json`，真实图片生成改为显式环境变量开启
- ✅ pytest 旧 basetemp 自动清理，只删除超过 7 天的严格时间戳目录
- ✅ `.gitignore` 补充本地报告类文档忽略规则，继续避免 bug 文档和临时计划入库
- ✅ 新增/更新回归测试覆盖 sandbox、图片配置、manifest 和 pytest 清理

**自查修复**：
- ✅ 修正只有 `[llm]` 配置时图片模型误继承文本模型的边界，默认使用 `gpt-image-2`
- ✅ 修复旧 `Image2Generator` 导入不存在的 `tools.config_loader`
- ✅ 确认 `settings/api_config.toml` 仅本地读取，不输出、不提交

**验证**：
- ✅ `python -m pytest -q`：97 passed
- ✅ black / flake8 / mypy / `py_compile`：通过
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py tools\asset_production`：通过
- ✅ `git diff --check`：通过（仅 CRLF 工作区提示）

**后续关注**：
- [ ] 需要实图验收时设置 `AUTODESIGNMAKER_ENABLE_IMAGE_GENERATION=1`
- [ ] 提交前确认 bug 文档、`plan/` 临时执行目录和 `settings/api_config.toml` 不进入暂存区
- [ ] CC-Panes 共享记忆池本次因环境变量缺失未写入

---

**日期**：2026-06-24
**ID**：2026-06-24-005
**摘要**：修复 pytest 临时目录与 draft 生命周期管理

**完成内容**：
- ✅ pytest 默认 basetemp 改为 `sandbox/pytest_<timestamp>`，cache 改为 `sandbox/pytest_cache`，避免 Windows Temp / `.pytest_cache` 权限问题
- ✅ 默认 pytest 收集范围限定为 `core/tests`，避免开发工具脚本误收集
- ✅ Hades 质量测试删除冗余断言，并提取模板节点数常量；任务标题长度 magic number 提取为常量
- ✅ 新增 draft 生命周期策略：启动时保留最近未关联 drafts，删除存档时清关联 draft，step0 重跑清当前 artifacts
- ✅ `draft_meta.json` 写入 `linked_save_id`，同时兼容旧 `linked_archive_path`
- ✅ `.gitignore` 防止 pytest 遗留缓存与临时 plan 入库

**自查修复**：
- ✅ 修复 `conftest.py` docstring 中 Windows 路径说明触发的 `W605 invalid escape sequence`
- ✅ 清理 `core/main.py`、`core/ui/workbench.py` 中 flake8 暴露的未使用 import/变量
- ✅ 历史 `drafts/` 一次性删除未执行，需用户明确确认

**验证**：
- ✅ `python -m pytest -q`：90 passed
- ✅ `python -m pytest core\tests\unit\test_draft_archive_paths.py core\tests\unit\test_core_paths.py -q`：11 passed
- ✅ `python -m pytest core\tests\unit\test_core_paths.py -q --basetemp=sandbox\pytest_tmp_explicit`：4 passed
- ✅ flake8 / mypy / `py_compile -W error::SyntaxWarning`：通过
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py`：通过
- ✅ `git diff --check`：通过（仅 CRLF 工作区提示）

**后续关注**：
- [ ] 历史 `drafts/` 清理属于不可逆用户数据删除，执行前必须再次确认
- [ ] 提交前确认 bug 文档和 `plan/` 临时执行目录不进入暂存区
- [ ] CC-Panes 共享记忆池本次因环境变量缺失未写入

---

**日期**：2026-06-24
**ID**：2026-06-24-004
**摘要**：执行 `hades_quality_optimization`：Hades 质量优化与标准化沉淀

**完成内容**：
- ✅ Hades partial 模板从 39/103 节点扩展到 103/103 节点，超过计划 80+ 覆盖目标
- ✅ Step07 程序任务标题清理，并新增 `category` / `priority`
- ✅ Step08 美术任务透传/生成 `asset_type` / `category` / `priority` / `complexity`
- ✅ 修复 Codex CLI Windows shim：优先 `codex.cmd` / `codex.exe`，避免 PowerShell `.ps1` 执行策略阻断
- ✅ 建立 `knowledge/governance/quality_standards/` 标准体系，共 17 个标准、模板、手册和指标文档

**自查修复**：
- ✅ 发现裸 `codex` 在 PowerShell 下会命中被拦截的 `codex.ps1`，已在 executor 中避开
- ✅ 任务标题清理后只剩泛词时使用 fallback，避免输出“资源”这类弱标题

**验证**：
- ✅ `python -m pytest core\tests -q`：87 passed（仅 `.pytest_cache` 写入权限 warning）
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py`：通过
- ✅ 内置模板 JSON 解析通过，Hades partial 覆盖 103 节点
- ✅ `codex.cmd --version`：`codex-cli 0.141.0`
- ✅ `git diff --check`：通过（仅 CRLF 工作区提示）

**后续关注**：
- [ ] GUI 重新载入/导出 Hades partial 后运行 Step02-09，验证新 draft 质量指标
- [ ] 提交前确认 `plan/hades_quality_optimization/` 和根目录临时评分报告不进入暂存区

---

**日期**：2026-06-24
**ID**：2026-06-24-003
**摘要**：执行 `template_l5_expansion`：内置模板 L5 实体覆盖扩展

**完成内容**：
- ✅ Phase 1 partial 模板补到 39 节点 complete 标准，并对齐 `elden_ring` 既有 complete 实体结构质量
- ✅ Phase 2 的 8 个 Indie 模板补齐 P0 16 核心节点实体
- ✅ Phase 3 的 7 个 3A 模板补齐 P0 16 核心节点实体
- ✅ Phase 4 的服务型、Midcore 和 IAA 超休闲模板补齐 P0 16 核心节点实体
- ✅ 新增 `test_template_l5_expansion.py` 覆盖 complete/P0 覆盖率与实体 schema 结构质量

**自查修复**：
- ✅ 修正旧有 `elden_ring` 曲线采样点不足、循环步骤不足和缺失 `supplement_basis` 的结构问题
- ✅ 临时批处理脚本执行后已删除，未留下开发过程工具

**验证**：
- ✅ `python -m pytest core\tests -q`：82 passed（仅 `.pytest_cache` 写入权限 warning）
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py`：通过
- ✅ 新增测试 black/flake8/mypy：通过
- ✅ 内置模板 JSON 解析通过；排除基础参考 `builtin_indie_hades.json` 后，8 个 complete、30 个 P0，缺失列表为空

**后续关注**：
- [ ] 提交前继续确认 `plan/template_l5_expansion/` 不进入暂存区
- [ ] GUI 载入模板后抽样检查 Step02 实体覆盖报告是否显示预期等级

---

**日期**：2026-06-24
**ID**：2026-06-24-002
**摘要**：提交范围纠正：本地 bug 文档和临时开发计划不入库

**完成内容**：
- ✅ 从上一笔提交中移除 `bug收集文档*.md`、`bug优化文档*.md` 和 `plan/l5_entity_ai_supplement/`
- ✅ `.gitignore` 增加小范围规则，防止本地 bug 文档和临时开发计划再次被误加
- ✅ `knowledge/ai_memory/code_conventions/anti_patterns.md` 补充提交禁令
- ✅ 记住提交前必须检查 `git status --short` 和 `git diff --cached --name-only`

**自查修复**：
- ✅ bug 文档只作为本地审查输入读取和处理，不提交到 git
- ✅ 临时开发执行计划只作为本地任务材料使用，不提交到 git

**验证**：
- ✅ 更新项目记忆并运行 `python tools\memory\update_freshness.py`
- ✅ 使用 amend 修正上一笔提交，不追加无意义修正提交

**后续关注**：
- [ ] 后续提交前确认暂存区不包含本地 bug 文档和临时执行计划
- [ ] 真实 Codex CLI 环境中仍需跑 Step02，确认 stdout JSON 能被 `_parse_response()` 接收

---

**日期**：2026-06-24
**ID**：2026-06-24-001
**摘要**：修复 `bug收集文档7.md` 第七轮 BUG-024/025：AI 补全适配器连通性

**完成内容**：
- ✅ BUG-024：`ModelTask` 增加 `sandbox` 字段，默认仍为 `workspace-write`
- ✅ BUG-024：`run_codex_exec()` 使用 `task.sandbox`，Step02 supplement 显式传 `sandbox="none"`
- ✅ BUG-025：`ClaudeCodeModelAdapter.generate()` 改用 `task.timeout_seconds`，不再硬编码 600 秒
- ✅ 新增适配器回归测试覆盖 Codex sandbox 和 Claude 超时透传

**验证**：
- ✅ `python -m pytest core\tests -q`：80 passed（仅 `.pytest_cache` 写入权限 warning）
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py`：通过
- ✅ 本次触碰文件 black/flake8/mypy：通过

---

**日期**：2026-06-23
**ID**：2026-06-23-012
**摘要**：修复 `bug收集文档6.md` 第六轮 BUG-023：无效 adapter 不再击穿 Step02

**完成内容**：
- ✅ BUG-023：`_call_ai()` 继续让 `ValueError/ImportError` 暴露，保留底层配置错误可见性
- ✅ `supplement()` 捕获 adapter 配置错误并降级到本地 fallback 实体，Step02 不再崩溃
- ✅ `SupplementResult.error` 与 `entity_coverage_report.json.ai_supplement.error` 记录 `unknown adapter` 等原因
- ✅ 补充分层回归测试：底层抛错、业务入口 fallback、`_stage2_outputs()` 无效 adapter 不崩溃

**验证**：
- ✅ `python -m pytest core\tests -q`：76 passed（仅 `.pytest_cache` 写入权限 warning）
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py`：通过
- ✅ 本次触碰文件 black/flake8/mypy：通过

---

**日期**：2026-06-23
**ID**：2026-06-23-011
**摘要**：修复 `bug收集文档5.md` 第五轮 5 个 L5 AI 补全问题

**完成内容**：
- ✅ BUG-018：adapter 配置错误直接抛出，不再被 `_call_ai()` 静默降级
- ✅ BUG-019：Step01 暴露公开 `pick_genre_template_key()`，Step02 不再导入私有 `_pick_template_key`
- ✅ BUG-020：AI adapter 实例化移到重试循环外，避免重复创建
- ✅ BUG-021：缺失 `pipeline_adapter` 时默认 `none`，Step02 AI 补全默认关闭
- ✅ BUG-022：旧缓存缺少 `supplement_basis` 时仍可命中

**验证**：
- ✅ `python -m pytest core\tests -q`：74 passed（仅 `.pytest_cache` 写入权限 warning）
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py`：通过
- ✅ 新增补全模块 black/flake8/mypy：通过

---

**日期**：2026-06-23
**ID**：2026-06-23-010
**摘要**：执行 `plan/l5_entity_ai_supplement`：Step02 L5 实体 AI 补全、缓存、降级与测试

**完成内容**：
- ✅ Step02 支持 `status=approximate` 概略实体解析和 `should_supplement()` 触发判断
- ✅ 新增 `EntitySupplementAdapter`，支持 AI 调用、缓存、失败降级和实体合并
- ✅ 新增补全提示词与多品类降级实体库
- ✅ `generation.py` Step02 接入补全适配器，`pipeline_adapter=none` 可关闭
- ✅ 新增 19 个 L5 supplement 单元/集成测试

**验证**：
- ✅ `python -m pytest core\tests -q`：66 passed（仅 `.pytest_cache` 写入权限 warning）
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py`：通过

---

**日期**：2026-06-23
**ID**：2026-06-23-009
**摘要**：溯源 `范本：Hades` 自动化开发 Step05 阻断：源包仍是 `未命名游戏设计项目` 空白状态

**完成内容**：
- ✅ 确认 Step05 `BLOCKED` 来自占位符质量门禁，不是评审代码异常
- ✅ 追溯到 `source_artifacts/devflow_Concept_v2` / `devflow_Design_v2`，确认导出源包不是 Hades
- ✅ 对比失败 draft 时间和 `范本：Hades` 存档创建时间，确认流水线早于 Hades 存档创建
- ✅ 记录排错经验：Step05 placeholder 阻断应向上检查导出源包和 Stage00/02/03 产物

**验证**：
- ✅ `stage_05/intelligent_review_report.json`：`verdict=BLOCKED`，阻断项为 `placeholder_rate`
- ✅ `stage_03/program_requirements_contract.json`：4 条需求全部包含 `未命名游戏设计项目`
- ✅ `source_artifacts/.../concept.md` / `design.md`：标题仍为 `未命名游戏设计项目`

---

**日期**：2026-06-23
**ID**：2026-06-23-008
**摘要**：修复 `bug收集文档4.md` 第四轮 3 个问题：Step00 新品类 inference、Step04 roguelike_action 键名兼容、CQ-013/014 证据入口

**完成内容**：
- ✅ BUG-015：Step00 `_genre_key` 补充 `strategy/rpg/moba`，并为三类补齐 CQ-005~CQ-012 genre evidence
- ✅ BUG-016：Step04 支持 `roguelike_action` 市场库键名，同时保留 `roguelike.json` 别名兼容
- ✅ BUG-017：CQ-013/CQ-014 增加实际 L4 字段和关键词入口
- ✅ 补充 strategy/rpg/moba、CQ-013/014、roguelike_action 市场库回归测试

**验证**：
- ✅ `python -m pytest core\tests -q`：47 passed（仅 `.pytest_cache` 写入权限 warning）
- ✅ 本次改动范围 `black --check` / `flake8` / `mypy --explicit-package-bases`：通过
- ✅ Step 00-06 端到端通过

---

**日期**：2026-06-23
**ID**：2026-06-23-007
**摘要**：修复 `bug收集文档3.md` 第三轮 5 个问题：阶段误判、模板缓存污染、市场库读取、资产阶段和 warning verdict

**完成内容**：
- ✅ BUG-010：Step02 移除宽泛 `"build"`，避免 `build_system_decision` 被误分到 `launch_ops`
- ✅ BUG-011：Step01 模板缓存按文件签名刷新，并在 pytest fixture 中清理缓存
- ✅ BUG-012：Step04 roguelike/fps/puzzle 都优先读取本地 market_data 库
- ✅ BUG-013：Step04 资产阶段分类补齐 `progression/social/launch_ops`
- ✅ BUG-014：Step05 有 warning 时 `_verdict` 返回 `WARN`

**验证**：
- ✅ `python -m pytest core\tests -q`：44 passed
- ✅ Step 00-06 端到端通过

---

**日期**：2026-06-23
**ID**：2026-06-23-006
**摘要**：执行 `plan/v5_engineering_plan`：Step00-05 能力增强、Phase0 基础设施、多品类模板验证和质量门禁

**完成内容**：
- ✅ Step00 Hades 稀疏 Concept 通过 `genre_inference` 透明补证据，coverage 提升到 0.8667
- ✅ Step01 增加 strategy/rpg/moba 模板、模板缓存、显式 loop 分隔符增强和 system_layer 前缀清洗
- ✅ Step02 L5 连续编号、kind 推断和 launch_ops 分类增强；保留无可信分母 coverage=0 的 BUG-007 修复
- ✅ Step03 多需求生成、扩展 schema routes、中文/英文语义绑定和需求密度统计
- ✅ Step04 多资产生成、P0 resolution、roguelike 本地 market_data 参考库
- ✅ Step05 verdict、placeholder BLOCKER、内容深度聚合告警、资产类型覆盖和 L1 配置绑定豁免

**验证**：
- ✅ `python -m pytest core\tests -q`：39 passed
- ✅ Step 00-06 端到端通过

---

**日期**：2026-06-23
**ID**：2026-06-23-005
**摘要**：修复 `bug优化文档.md` 第二轮 3 个问题：覆盖率最终回退、模板重复读取、requires_action_count 漏计 BLOCKER

**完成内容**：
- ✅ BUG-007：Step02 `_expected_node_count` 最终 fallback 不再返回 covered node count，避免无可信分母时假报 1.0
- ✅ BUG-008：Step01 `LoopExtractor` / `SystemDeducer` 每次只读取一次 `genre_templates.json`
- ✅ BUG-009：Step05 `requires_action_count` 现在统计 BLOCKER + CRITICAL
- ✅ 新增无 expected total 的真实 L5 entity 场景测试，以及 BLOCKER action count 测试

**验证**：
- ✅ `python -m pytest core\tests -q`：34 passed
- ✅ D4 → Step 00-06 端到端通过

---

**日期**：2026-06-23
**ID**：2026-06-23-004
**摘要**：修复 `bug收集文档.md` 中列出的 6 个 pipeline optimization 回归问题，并补充测试

**完成内容**：
- ✅ BUG-001：修复 `export_adapter.py` 中 `coreLoops` 默认值缺失导致的 `None.get()` 崩溃
- ✅ BUG-002：Step02 entity coverage 改用真实分母，优先 `design_summary.node_count`，不再无条件 1.0
- ✅ BUG-003：Step01 `SystemDeducer` 系统数量明确截断到最多 8 个
- ✅ BUG-004：Step03 模糊匹配阈值从 0.18 提升到 0.4，并取消无依据 `phase:*` 伪绑定
- ✅ BUG-005：Step02 环路报告不再重复闭合节点
- ✅ BUG-006：Step05/06 评审报告分离 BLOCKER 与 CRITICAL，`blocking_issue_count` 只统计 BLOCKER

**验证**：
- ✅ `python -m pytest core\tests -q`：32 passed
- ✅ D4 → Step 00-06 端到端通过

---

**日期**：2026-06-23
**ID**：2026-06-23-003
**摘要**：完成 pipeline_optimization 收尾：跨进程 D4 源包发现、无 L5 实体本地补全、资产字段完整性与最终端到端自检

**完成内容**：
- ✅ `core/source/finder.py` 新增 source root 回退：当前 draft → 最新历史 draft → legacy `sandbox/source_artifacts/`
- ✅ `core/engines/generation.py` 与导入器使用一致的源包发现顺序，修复 D4 与 Step 00-06 分进程执行失败
- ✅ Step 02 在无显式 `L5实体` 时合成最多 47 个可追踪本地实体
- ✅ Step 04 selection/entity 资产补齐 `priority` 与 `complexity`
- ✅ 新增跨 draft source fallback、无 L5 实体合成、asset complexity 测试

**验证**：
- ✅ `python -m pytest core\tests -q`：26 passed
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py`：通过
- ✅ D4 → Step 00-06 端到端通过

---

**日期**：2026-06-23
**ID**：2026-06-23-002
**摘要**：继续完成开发计划收尾：质量工具修复、根目录设计蓝图归档、最终自检

**完成内容**：
- ✅ `tools/validators/pipeline_quality.py` 支持 `--artifacts-dir`，并在新进程 draft 为空时回退到最近有 stage 00-06 质量产物的 draft
- ✅ 新增测试覆盖质量指标工具的显式 artifacts 根目录采集
- ✅ 清理 `core/design/export_adapter.py` 中不可达旧代码和未使用导出常量
- ✅ 清理 `core/engines/generation.py` 中已被 `LoopExtractor` 替代的 `_core_loop_steps()`
- ✅ 根目录 `design_plan/` 旧蓝图文件已归档到 `plan/pipeline_optimization/design_plan_archive/`
- ✅ 更新 `plan/pipeline_optimization/README.md` 和 `plan/status_snapshot_2026-06-23.md` 记录实际状态

**验证**：
- ✅ `python -m pytest core\tests -q`：24 passed
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py`：通过
- ✅ Hades 00-06 代码级回归达到目标指标

---

**日期**：2026-06-23
**ID**：2026-06-23-001
**摘要**：完成 D4 designEntities 导出与步骤00-06质量优化基础设施

**完成内容**：
- ✅ D4 `devflow_Design_v2/attachments/design.md` 将 `designEntities` 序列化为可解析的 `L5实体` 条目
- ✅ Step 00-06 新增实体驱动输出、质量报告、资产转换、分级评审
- ✅ 新增 `tools/validators/pipeline_quality.py` 初版质量指标采集工具
- ✅ 新增存档系统二期 ADR：draft 为唯一运行写入点，正式存档为显式归档结果

**验证**：
- ✅ `python -m pytest core\tests -q`：23 passed
- ✅ `python -m compileall core pipeline tools\validators\pipeline_quality.py`：通过
- ✅ Hades 00-06 同进程代码级回归达到目标指标

---

**日期**：2026-06-21
**ID**：2026-06-21-003
**摘要**：按 ADR 0001/0002 完成 per-session drafts 路径与正式存档去快照化

**完成内容**：
- ✅ `core/paths.py` 新增 `drafts/{timestamp}_{pid}/` 会话草稿根，`SANDBOX_DIR` 仅作为兼容别名指向当前 draft
- ✅ `core/save/manager.py` 改为从 draft 同步 active 文件，正式存档 `saves/{save_id}/` 只保留 `manifest.json` 和 `workspace/`
- ✅ 快照、事务 file map、timeline 只写入当前 draft，不再进入正式存档
- ✅ 兼容读取旧 `save_manifest.json`，同步时迁移为 `manifest.json`

**后续关注**：
- [ ] 执行对象存储仍通过正式存档 workspace 读写；本轮已形成 ADR 草案，待评审后实施

---

**日期**：2026-06-21
**ID**：2026-06-21-001  
**摘要**：存档管理完善 + 流水线 AI 适配器可选 + Skill 库集成 + 项目配置 UI 重设计

**完成内容**：
- ✅ 存档管理：新建存档自动保存设计、默认项目名、重命名功能、删除打开按钮
- ✅ AI 适配器可选：Claude Code CLI / Codex CLI / OpenAI，项目配置界面下拉切换
- ✅ Skill 库：从官方拉取 frontend-design + imagegen，注入流水线步骤 04/08/09/11
- ✅ 项目配置 UI 卡片化重设计

**下次优先任务**：
- [ ] 验证 AI 适配器切换（codex vs claude）
- [ ] 验证 skill_guidance.md 写入步骤输出目录

**完成内容**：
- ✅ `ucos/` → `knowledge/ucos/`，`core/paths.py` 加 sys.path 免改导入
- ✅ 新建 `core/design/ai_ucos_bridge.py`：AI 访谈每轮写入 ucos（对话/决策/路由/设计生成）
- ✅ `artifact_layer/` → `pipeline/artifact_layer/`
- ✅ 清理：删除 `_archive/`、根目录 `ai_runtime/`、`.claude/` 残留、`plan/`

**关键发现**：
- ucos 之前完全孤立无入口调用，现已通过 `ai_ucos_bridge` 联通
- `ai_runtime/` 根目录是旧版遗留，实际写入路径为 `sandbox/workspace/ai_runtime/`

**下次优先任务**：
- [ ] 验证 AI 访谈 → ucos 写入是否正常
- [ ] 考虑将 ucos context_builder 读取结果注入 AI 访谈 prompt（闭环）

**完成内容**：
- ✅ 修复执行对象保存全链路（6个 Bug，包括 force_cancel 解决残留冲突）
- ✅ 新增 `core/ui/save_manager_dialog.py`：独立存档管理对话框
- ✅ 存档过滤：只显示含 design_project 的存档槽，屏蔽流水线存档
- ✅ `core/ui/unity_config_dialog.py` 重构为 `ProjectConfigDialog`（多引擎）
- ✅ `core/runtime/preflight.py` 按引擎分支检查
- ✅ 清理垃圾存档，工作区置空

**关键发现**：
- 历史实现中 `runtime_root` = `sandbox/workspace`，存档在 `sandbox/workspace/saves/`，与流水线共享；2026-06-21-003 后运行根已改为 `drafts/{session_id}/`
- `save_20260609_*` 等是流水线存档，不含 design_project 数据，需过滤

**Git commit**：`77be8bd`

**下次优先任务**：
- [ ] 验证保存/加载流程端到端正常
- [ ] 验证引擎切换后 preflight 检查变化符合预期

---

## L1 项目理解缓存状态

| 文件 | 缓存状态 | 上次读取 |
|------|----------|----------|
| architecture.md | ✓ 有效 | 2026-06-19 |
| key_files.md | ✓ 有效 | 2026-06-19 |
| freshness.json | ✓ 有效 | 2026-06-19 |

**架构精要**（详见 project_understanding/architecture.md）：
- 8 层职责分工：步骤插件层 / 运行骨架层 / 知识层 / 配置层 / 工具层 / 认知层 / 注册表层 / 沙盒层
- 核心执行链：main.py::run_range() → plugin.execute() → run_import_step() → apply_development_plan_outputs() → artifact review/validation → save/manager.py::retry_sync()
- 三大引擎：generation.py（16阶段业务逻辑）、DesignEngine（游戏设计决策）、CodexCliBackend（AI后端）

---

## L2 代码惯例速查

**必须遵守的5条规则**（详见 code_conventions/patterns.md）：

1. **路径管理**：所有路径常量在 `core/paths.py` 定义，禁止其他文件硬编码路径字符串
2. **StagePlugin 模式**：stage_id + _source_groups + execute(ctx) → run_import_step() → apply_development_plan_outputs()
3. **错误处理**：返回 StageResult(status="failed")，不用 try/except 包业务逻辑
4. **文件头**：所有 .py 文件开头必须有 `from __future__ import annotations`
5. **主题统一**：GUI 组件颜色从 `core/ui/theme.py::COLORS` 取，字体从 `FONT_*` 取

**禁止事项**（详见 code_conventions/anti_patterns.md）：
- ❌ 在 core/ 以外写运行时核心逻辑
- ❌ 直接 import steps.* 或 design_tool.*（已删除）
- ❌ 在 tools/ 根目录放 .py 文件（必须放子目录）
- ❌ 创建超过 400 行的单一功能文件（必须拆分）

---

## L4 待办决策

**待解决问题**（详见 decisions/open_questions.md）：
- AI 对话面板在"流水线模式"下的系统 prompt 内容
- 记忆系统会话结束时机触发方式（手动 vs 自动检测）

**最新架构决策**（详见 decisions/architecture.md）：
- 2026-06-19：GUI 重构采用标签切换方式，CommercialDesignApp 改为 tk.Frame
- 2026-07-10：Rust 二次开发的问题与解决方案统一记录在 `plan/fixplan/`，供多个窗口共享

---

## 如何使用本记忆系统

1. **会话开始时**：读取本文件 + project_understanding/key_files.md，检查缓存有效性
2. **缓存有效**：直接使用记忆中的理解，不重新读对应文件
3. **缓存失效**：对比 freshness.json，只重读改过的文件，更新对应记忆
4. **会话结束时**：写入新的 session_history/YYYY-MM-DD_NNN.md，更新本索引
