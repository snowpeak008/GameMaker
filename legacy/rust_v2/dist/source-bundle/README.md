# AutoDesignMaker Rust

This directory is the Rust rebuild boundary for AutoDesignMaker.

The Python project outside `RUST/` is reference material only. Runtime code,
new tests, and new product architecture for the Rust version should live here.

Initial development follows `../plan/rustplan/02_框架优先开发计划.md`:

1. Build the Cargo workspace and crate boundaries.
2. Build foundation/config/archive/runtime/AI/pipeline frameworks.
3. Add GUI and business chains only after the base contracts are testable.

## Isolated Rust desktop release

Build the Slint desktop executable first:

```powershell
cargo build -p adm-desktop --release
```

Then stage a Rust-only release bundle under `RUST/dist/AutoDesignMaker-rust/`:

```powershell
cargo run -p adm-cli -- stage-desktop-release .\target\release\adm-desktop.exe
```

The staged executable is named `AutoDesignMaker-rust.exe`. This command writes only under
`RUST/dist/...` and does not modify the legacy root `..\AutoDesignMaker.exe`.

Check the staged release before handing it off:

```powershell
cargo run -p adm-cli -- release-doctor
.\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke
```

`release-doctor` should print `ready=true` and `legacy_root_exe=not_modified`.
The desktop `--smoke` run also starts child `AutoDesignMaker-rust.exe` probe processes to verify
that different formal archives can be opened in parallel while the same archive remains locked to a
single session.

For a full local release gate, run the scripted sequence from the `RUST/` directory:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1
```

The script imports the Visual Studio build environment, runs `cargo fmt --check`,
`cargo check --workspace`, `cargo test --workspace`, `cargo build -p adm-desktop --release`,
`stage-desktop-release`, `release-doctor`, `delivery-doctor`, `release-acceptance`,
`stage-source-bundle`, `stage-handoff-bundle`, and `external-acceptance`, then writes
`handoff-status.adm`, writes `handoff-instructions.adm`, syncs final gate reports into the handoff
bundle evidence directory, and writes a final handoff package manifest with a package hash. It then
refreshes handoff instructions/evidence and finalizes the package again so the packaged
`handoff-instructions.adm` reflects the current final manifest readiness fields. The final package
also includes a top-level `HANDOFF_README.txt` entry file that points to the executable, source
bundle, evidence instructions, final manifest, strict gate command, strict gate working-directory
context, and current readiness state. The handoff bundle root is a package inspection and evidence
entrypoint; strict gate commands must be rerun from a Rust workspace root with the `scripts`
directory, generated delivery artifacts, matching `DataRoot`, Unity editor, and real AI credentials.
The generated `strict_gate_command` and `strict_gate_ai_invoke_command` fields reuse the current
handoff DataRoot when known, leaving only the Unity executable path as an operator-supplied value.
It also writes `source-handoff-policy.adm` so the generated source bundle is an explicit handoff
evidence surface.
Use `-DryRun` to print the planned commands without executing mutating acceptance gates; the final
handoff wrapper still runs the read-only operator preflight and embeds `operator_preflight_*` rows in
the dry-run report so missing Unity, AI secret, DataRoot, or archive inputs are visible in one file.
Wrapper dry-runs preserve angle-bracket placeholders such as `'<data_root>'` in printed commands; a
real non-dry-run command rejects unresolved placeholder paths before any mutating gate executes. The
printed command previews quote placeholders and paths when PowerShell would otherwise parse them as
syntax.
The external acceptance step writes beta-readiness diagnostics without failing the local gate; pass
`-RequireExternalAcceptance` in a real Unity/provider environment to require external acceptance,
handoff status, and the refreshed final package manifest to be ready, including
`final-handoff-manifest.adm delivery_ready=true`. Use `-SkipExternalAcceptance` for a local-only gate.
Add `-RequireAiInvoke` when the final gate must also prove that `ai-acceptance.adm` was produced with
a successful real provider invocation; wrappers require this flag to be paired with the strict
external acceptance flag so missing invocation evidence fails the command. Pass
`-UnityExe '<path-to-Unity.exe>'` when Unity is installed outside the default discovery paths.

Track external beta-readiness requirements separately from the local release gate:

```powershell
cargo run -p adm-cli -- external-acceptance --unity-exe '<path-to-Unity.exe>'
cargo run -p adm-cli -- external-acceptance --require-ready --require-ai-invoke --unity-exe '<path-to-Unity.exe>'
cargo run -p adm-cli -- stage-source-bundle
cargo run -p adm-cli -- stage-handoff-bundle
cargo run -p adm-cli -- write-source-handoff-policy
cargo run -p adm-cli -- handoff-status
cargo run -p adm-cli -- write-handoff-instructions
cargo run -p adm-cli -- sync-handoff-evidence
cargo run -p adm-cli -- finalize-handoff-package
powershell -ExecutionPolicy Bypass -File .\scripts\external_acceptance_doctor.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'
```

The CLI command writes `dist/AutoDesignMaker-rust/external-acceptance.adm` with the current release
acceptance status, Unity editor discovery result, imported Unity runtime execution evidence, the
selected `data_root`, the current AI acceptance report snapshot, and whether a non-mock AI provider
is configured. It also writes `blocker_count` and `blocker=` rows so `--require-ready` failures can
be diagnosed from the external acceptance report itself.
Use `--require-ready` on the CLI, or `-RequireReady` on the PowerShell wrapper, when running in a
real Unity/provider environment and you want missing external acceptance evidence to fail the
command. Add `--require-ai-invoke`, or wrapper `-RequireAiInvoke`, only when the current
`ai-acceptance.adm` must show `invoke_attempted=true` and `invoke_succeeded=true`; wrapper usage
must also include `-RequireReady` or `-RequireExternalAcceptance` depending on the script so the
strict condition controls the exit code. Use the same Unity executable path for
`unity_acceptance_gate.ps1`, `external-acceptance`, and the final strict `release_gate.ps1`. When AI
providers are configured under a non-default data root, pass the same `-DataRoot '<data_root>'` to
`ai_acceptance_gate.ps1`, `external_acceptance_doctor.ps1`, `unity_acceptance_gate.ps1`, and
`release_gate.ps1`.
External acceptance is ready only when Unity is discovered, the generated
`Assets/AutoDesignMaker/Generated/runtime_execution_results.adm` report has `runner=unity_playmode`
and `ready=true`, a non-mock AI provider is ready, and the current `ai-acceptance.adm` report is
ready/configured for one of those real providers. When `--require-ai-invoke` is used, it also
requires a successful AI invocation in the current `ai-acceptance.adm` report.
`handoff-status` reads `release-acceptance.adm`, `external-acceptance.adm`, and
`ai-acceptance.adm`, plus the source manifest from `stage-source-bundle` and the handoff bundle
manifest from `stage-handoff-bundle`, then writes `dist/AutoDesignMaker-rust/handoff-status.adm`
with a single blocker list for final handoff. When both external acceptance and AI acceptance report
a `data_root`, `handoff-status` also records whether the two reports used the same root and blocks on
`ai_acceptance_data_root_mismatch` if they disagree; it also blocks on
`ai_acceptance_provider_not_real_provider` if the accepted provider is not listed as a ready real
provider by external acceptance.
`write-handoff-instructions` reads the same final gate reports and writes
`dist/AutoDesignMaker-rust/handoff-instructions.adm` with machine-readable blocker follow-up steps,
expected evidence files, commands, and rough time estimates for external Unity and real AI provider
acceptance. It also records the current `final-handoff-manifest.adm` presence plus
`final_package_ready`, `final_delivery_ready`, and `final_handoff_ready`, then emits a final
`confirm-final-delivery-package` instruction so the handoff endpoint is explicit:
`final-handoff-manifest.adm` must report `delivery_ready=true`. The same report also declares
`strict_gate_requires_final_delivery=true` and
`strict_gate_final_manifest_requires=package_ready,handoff_ready,delivery_ready` so strict final
delivery requirements are machine-readable. The generated `instruction=` rows use the same
context-filled suggested commands as the blocker resolution rows, so known provider, model, DataRoot,
and archive id values are not replaced by generic placeholders. It also records
`required_instruction_count`, `required_blocked_instruction_count`,
`required_waiting_instruction_count`, `optional_instruction_count`,
`manual_decision_instruction_count`, and `next_required_instruction` so remaining handoff work can be
read without parsing every instruction row. The report also expands that next required action into
`next_required_instruction_status`, `next_required_instruction_estimate`,
`next_required_instruction_command`, `next_required_instruction_evidence`,
`next_required_instruction_done_when`, and `next_required_instruction_note`. It also emits
`remaining_required_execution_step_count` plus ordered `remaining_required_execution_step=` rows for
the still-required, not-yet-ready handoff actions, including each action's command, evidence,
completion condition, and note.
It also emits `external_dependency_count` plus `external_dependency=` rows for handoff prerequisites
that must be supplied by the receiving environment, such as a real AI provider secret/configuration,
an optional real-provider invoke check, and a compatible Unity editor capable of producing
`unity_playmode` runtime evidence. It also emits `operator_input_count` plus `operator_input=` rows
for the concrete values the receiving operator must provide, including the redacted AI secret
placeholder and the Unity executable placeholder with copyable check/set guidance. It also emits
`suggested_operator_preflight_command` and
`suggested_operator_preflight_require_ready_command`, which run
`scripts/handoff_operator_preflight.ps1` to check the current receiving shell for the required
secret, Unity executable path, DataRoot, archive id, final acceptance script, and gate scripts before
attempting AI, Unity, or strict release gates. For packaged handoff inspection, it also emits
`operator_preflight_bundle_root_supported`,
`suggested_operator_preflight_bundle_root_command`, and
`suggested_operator_preflight_bundle_root_require_ready_command` so a receiver can run the same
preflight from the `dist/handoff-bundle` root through
`source-bundle/scripts/handoff_operator_preflight.ps1` while reading
`evidence/handoff-instructions.adm`. `scripts/final_handoff_acceptance.ps1 -DryRun` runs that
preflight without `-RequireReady` and records its diagnostic output as `operator_preflight_*` rows,
while the real final run keeps `-RequireReady` so missing operator inputs still fail before any gate
runs. The operator preflight and final acceptance wrapper resolve Unity from an explicit
`-UnityExe`, `ADM_UNITY_EDITOR`, `UNITY_EDITOR_PATH`, and default Unity discovery paths, and report
the selected source as `unity_exe_source`. Unity acceptance instruction evidence uses the final
package path
`dist/unity-project/Assets/AutoDesignMaker/Generated/runtime_execution_results.adm`, matching the
external dependency and blocker resolution rows.
`stage-source-bundle` copies the Rust source tree to `dist/source-bundle` while excluding generated
`target`, `dist`, `.adm_rust_data`, and `.git` directories. It recreates the source bundle directory
before copying so stale source files are not carried into handoff, and writes
`dist/AutoDesignMaker-rust/source-manifest.adm` with file counts and an aggregate source hash.
`write-source-handoff-policy` reads that source manifest plus the handoff bundle manifest, verifies
that `source-bundle` was copied into `dist/handoff-bundle`, and writes
`dist/AutoDesignMaker-rust/source-handoff-policy.adm` to declare the current handoff policy as
bundled source evidence.
`stage-handoff-bundle` recreates `dist/handoff-bundle`, copies the staged desktop release and source
bundle as required directories, includes generated game, SDK, and Unity delivery directories when
present, and writes `dist/AutoDesignMaker-rust/handoff-bundle-manifest.adm` with a deterministic
bundle hash. It excludes `external-acceptance.adm`, `handoff-status.adm`, its own previous
`handoff-bundle-manifest.adm`, previous `handoff-evidence-manifest.adm`, previous
`handoff-instructions.adm`, previous `source-handoff-policy.adm`, and previous
`final-handoff-manifest.adm` from the copied desktop release directory because those final gate
reports are updated after the bundle manifest is staged.
`sync-handoff-evidence` runs after
`handoff-status` and copies the current final reports into `dist/handoff-bundle/evidence`, alongside
`source-handoff-policy.adm`, `handoff-instructions.adm`, and `handoff-evidence-manifest.adm`, so the
handoff directory contains current evidence and explicit next steps without mixing stale reports into
the nested desktop release copy. The source release keeps the original `DataRoot` evidence for audit,
while the bundled `evidence/handoff-instructions.adm` rewrites command `-DataRoot` arguments to the
portable `'<data_root>'` placeholder and records
`handoff_bundle_command_data_root_mode=portable-placeholder`, leaving the original
`external_acceptance_data_root` and `ai_acceptance_data_root` fields intact. The handoff bundle copy
of `AutoDesignMaker-rust/final-acceptance-run.adm` applies the same command-only DataRoot
placeholder rewrite so receivers do not see original-workspace DataRoot command arguments in the
packaged dry-run report; the report's `data_root=` audit field remains unchanged.
`finalize-handoff-package` then writes
`HANDOFF_README.txt` at the bundle root, then writes `final-handoff-manifest.adm` with a package hash
for the complete `dist/handoff-bundle` directory, including evidence and the README entry file while
excluding only the final manifest itself. The final manifest requires `HANDOFF_README.txt` as a
package file. The generated README records `strict_gate_working_dir=rust-workspace-root-with-scripts-directory`
and `strict_gate_bundle_root_runnable=false` so the package root is not mistaken for the strict gate
execution directory. It also records `handoff_bundle_dist_layout=top-level-delivery-artifacts`,
`handoff_bundle_contains_data_root=false`, `strict_gate_original_data_root`,
`strict_gate_requires_matching_data_root=true`, and
`strict_gate_requires_rehydrated_workspace_when_not_original=true`. Those fields make the restore
boundary explicit: when not running in the original Rust workspace, use `source-bundle` as the Rust
workspace root, place the top-level bundle artifact directories under that workspace's `dist/`, and
provide the same DataRoot or an imported equivalent before rerunning acceptance gates. The README and
handoff instructions also emit `handoff_rehydration_bundle_root_supported`,
`handoff_rehydration_script`, `handoff_rehydration_manifest`, and
`suggested_handoff_rehydration_command`. The suggested command runs
`source-bundle/scripts/rehydrate_handoff_workspace.ps1` from the handoff bundle root, restores
`source-bundle` into a clean Rust workspace destination, copies the bundle artifact directories into
that destination's `dist/`, restores the full handoff bundle under `dist/handoff-bundle`, and writes
`dist/handoff-rehydration-manifest.adm` with the next workspace-root preflight and strict-gate
commands. For the final receiving environment, it emits `final_acceptance_working_dir`,
`final_acceptance_script`, `final_acceptance_sequence`, `final_acceptance_requires`,
`final_acceptance_report`, `suggested_final_acceptance_command`, and
`suggested_final_acceptance_ai_invoke_command`. The final acceptance script runs the operator
preflight, real AI acceptance, Unity acceptance, external acceptance, and strict release gate in order
from a Rust workspace root, using the existing gate scripts rather than duplicating their logic. It
writes `dist/AutoDesignMaker-rust/final-acceptance-run.adm` with the planned command sequence in
dry-run mode and the pass/fail status of each step during a real run. Dry-run command previews and
generated handoff commands quote placeholder or space-containing arguments, and dry-run mode does not
require a supplied Unity path to exist locally. After a successful non-dry-run acceptance using the
default report path, it refreshes the final handoff package again so the package contains the final
`passed` report rather than an earlier planned report. It also forwards the current
`external_dependency=` rows,
`blocker=` rows,
`operator_input=` rows with concrete placeholders/check commands,
`blocker_resolution=` rows with the direct action/command/evidence/done condition for each blocker,
instruction summary counts and the expanded `next_required_instruction_*` fields, and `instruction=`
rows with each manual action's command/evidence/note. It also forwards the ordered
`remaining_required_execution_step=` rows, so a receiving operator can follow the remaining required
actions without filtering optional/manual-decision instructions first. The README includes external
acceptance context such as
`external_acceptance_data_root`, `unity_runtime_runner`, `unity_selected`, `ai_provider_id`,
`ai_configured_ready`, `real_ai_provider_count`, and `ready_provider_count`, so the package entry file
can be used as a machine-readable prerequisite checklist before rerunning the strict gate. It also
forwards `unity_candidate_detail_count` and `unity_candidate=` rows from the Unity discovery output,
so the package entry file records which Unity executable paths were tested and whether each was
present/ready. It also forwards `ai_provider_detail_count` and `ai_provider=` rows from the AI
diagnostics output, so the package entry file records which providers were discovered, their
readiness, capabilities, and redacted readiness notes. The same entry file also forwards
`suggested_ai_acceptance_command`,
`suggested_ai_acceptance_invoke_command`, `suggested_unity_acceptance_command`,
`suggested_external_acceptance_command`, `suggested_strict_release_gate_command`, and
`suggested_strict_release_gate_ai_invoke_command` with the current provider/model/DataRoot already
filled when known; it also forwards the handoff operator preflight commands so receivers can verify
their local shell inputs before the expensive gates. The canonical `strict_gate_command` and
`strict_gate_ai_invoke_command` fields use the same current DataRoot, and it forwards
`strict_gate_requires_final_delivery=true`. It also
records the preset's `suggested_ai_secret_env_var`,
`suggested_ai_secret_requirement`, `suggested_ai_secret_check_command`, and
`suggested_ai_secret_session_set_command`, so real-provider acceptance can verify the required
environment variable and prepare the current PowerShell session before rerunning the gate. The check
command escapes `$env:` as `` `$env:`` so it can be copied from an interactive PowerShell session
without the parent shell expanding the variable before the nested `powershell -Command` receives it.
The session set command contains only a `<secret>` placeholder; the receiving operator supplies the
real value locally before running the AI acceptance wrapper. When the DataRoot contains formal archives,
the generated Unity acceptance command also fills
`suggested_unity_archive_id` from the latest archive id and records
`suggested_unity_archive_source=data_root_latest_archive`; otherwise it keeps `<archive_id>` as the
manual placeholder. The final manifest keeps `ready=true` for package assembly readiness, and
separately records `delivery_ready`, `handoff_ready`, `external_acceptance_ready`, and
`ai_acceptance_ready` so a locally assembled package cannot be mistaken for a fully accepted
delivery. Full completion requires the strict release gate with `-RequireExternalAcceptance` to
refresh the handoff status/evidence and enforce final package manifest `package_ready=true`,
`handoff_ready=true`, and `delivery_ready=true`.
Use
`handoff-status --require-ready` only when local release, external Unity acceptance, explicit AI
provider acceptance, source handoff evidence, and the final handoff bundle are all expected to be
ready.

## Core CLI workflow

Create a demo project and run the core game design/development/assets/SDK/package pipeline:

```powershell
cargo run -p adm-cli -- demo-core "Demo Game"
cargo run -p adm-cli -- list
cargo run -p adm-cli -- workspace-doctor
cargo run -p adm-cli -- workspace-cleanup
```

`workspace-doctor` reports temporary formal-archive workspaces under `data_root/workspaces`.
`workspace-cleanup` removes stale workspaces that are not referenced by an active archive lock, and
skips workspaces for currently locked archive sessions.
The desktop shell exposes the same operations through the Data Root row's `Workspaces` and
`Clean Workspaces` actions.
For archive locks, the desktop shell keeps current-window and external recovery paths separate:
`Release Lock` only releases the lock held by the current window, while `Clear External Lock`
removes a lock file owned by another or stale session.

Create a project from explicit brief fields instead of the built-in demo brief:

```powershell
cargo run -p adm-cli -- run-core "Custom Game" "tactical puzzle adventure" "Players solve compact tactical routes with readable feedback" "Scout the room | Plan a route | Resolve the encounter with feedback"
```

The Slint desktop shell exposes the same brief fields through Project, Genre, player promise, and
Core Loop inputs before `Create + Run`.
The original brief is persisted in each archive as `project/brief.adm`; resume and rerun operations
load this stored brief so custom projects do not fall back to the built-in demo defaults.

The generated archive content includes `validation/acceptance_matrix.adm`, which traces each core
loop mechanic through design scenario, development task, asset feedback task, SDK/build target, and
validation probe readiness. It also includes `validation/scenario_test_plan.adm`, which expands each
playable scenario into a setup, playable-smoke steps, success/failure expectations, telemetry, and
build target row. `validation/runtime_validation_report.adm` turns those rows into deterministic
runtime probes with linked acceptance trace IDs, telemetry start/complete events, expected runtime
state, failure guards, and build targets. After an external runtime runner has executed those probes,
`runtime-validation-record` can import its results into `validation/runtime_execution_results.adm`.
`validation/production_readiness.adm` summarizes design quality, playable scenario coverage,
development task coverage, asset feedback coverage, SDK/build readiness, acceptance trace readiness,
scenario test plan readiness, runtime validation readiness, and validation gate status.

The Slint desktop shell also renders this acceptance matrix as a dedicated table after project
creation, import, resume, or archive inspection.
Each core loop step now receives its own playable scenario and validation probe, so the acceptance
matrix can trace individual mechanics instead of collapsing the whole loop into one smoke scenario.

Export or import project archives:

```powershell
cargo run -p adm-cli -- export <archive_id> <target_file>
cargo run -p adm-cli -- package-doctor <target_file>
cargo run -p adm-cli -- import <package_file>
```

`package-doctor` validates the `.admproj` package without importing it. It reports the package
format, manifest, declared and actual file counts, payload hash, per-file hashes, and `ready=true`
only when the package can be safely imported.

Record runtime validation evidence from an external runner:

```powershell
cargo run -p adm-cli -- runtime-validation-record <archive_id> <results_file>
```

The runner input is a line-oriented document:

```text
# Runtime Validation Execution
runner=unity_playmode
target_id=windows_desktop_playable
- result_id=runtime_scenario_core_loop_step_1; scenario_id=scenario_core_loop_step_1; test_id=test_scenario_core_loop_step_1; acceptance_trace_id=trace_core_loop_step_1; telemetry_start_seen=true; telemetry_complete_seen=true; expected_state_seen=true; failure_guard_triggered=false; status=passed
```

The command validates every reported `result_id` against `validation/runtime_validation_report.adm`,
rejects malformed or duplicate rows, writes `validation/runtime_execution_results.adm`, and updates
the runtime validation check inside `validation/production_readiness.adm`.

AI provider checks and manual invocation:

```powershell
cargo run -p adm-cli -- ai-doctor
cargo run -p adm-cli -- ai-secret-set openai <secret>
cargo run -p adm-cli -- ai-provider-presets
cargo run -p adm-cli -- ai-provider-preset openai openai_main default
cargo run -p adm-cli -- ai-provider-set openai_compatible https://api.openai.com/v1 named:openai
cargo run -p adm-cli -- ai-provider-check <provider_id> <model>
cargo run -p adm-cli -- ai-provider-invoke <provider_id> <model> <prompt>
cargo run -p adm-cli -- ai-acceptance --require-ready <provider_id> <model>
powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId '<provider_id>' -Model '<model>' -DataRoot '<data_root>' -RequireReady
powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId '<provider_id>' -Model '<model>' -DataRoot '<data_root>' -Invoke -RequireReady -RequireInvoke
powershell -ExecutionPolicy Bypass -File .\scripts\ai_acceptance_gate.ps1 -ProviderId openai_main -Model gpt-4.1 -Preset openai -SecretRef default -DataRoot '<data_root>' -RequireReady
```

`ai-provider-presets` and `ai-provider-preset` perform non-network configuration validation for
known OpenAI-compatible endpoints such as `openai`, `openrouter`, `deepseek`, and `local_openai`.
Use `default` for the preset's default secret reference, or `none` for local endpoints that do not
need a secret. `ai-provider-check` is also non-network; only `ai-provider-invoke` calls the provider.
`ai-acceptance` writes `dist/AutoDesignMaker-rust/ai-acceptance.adm` with the selected `data_root`
and without storing raw model output. Add `--invoke`, or use the wrapper's `-Invoke`, only when a
real network call is intended. Use `--require-invoke` or `-RequireInvoke` when the provider
invocation itself must be part of the acceptance gate. The final external gate can enforce that
evidence with `external-acceptance --require-ai-invoke` or `release_gate.ps1 -RequireAiInvoke`.

`scripts/ai_acceptance_gate.ps1` can configure the provider immediately before writing the
acceptance report. Pass `-Preset <preset_id>` with `-SecretRef default` to use a known preset's
default environment variable, or pass `-Endpoint <endpoint_hint> -SecretRef <env:NAME|named:NAME|none>`
for a custom OpenAI-compatible endpoint. The built-in default environment variables are
`OPENAI_API_KEY` for `openai`, `OPENROUTER_API_KEY` for `openrouter`, `DEEPSEEK_API_KEY` for
`deepseek`, and no secret for `local_openai`. Use `-DryRun` to print the exact configuration and
acceptance commands without writing provider config or calling the provider.

`ai-provider-set` accepts `env:ENV_VAR_NAME` or `named:secret_name` references. Named secret values are
stored in `config/named_secrets.adm` under the selected data root, while `config/app_config.adm`
keeps only the `named:...` reference so provider profiles and diagnostics do not print the secret.

## Game build and SDK delivery

Stage required game build content for an engine project:

```powershell
cargo run -p adm-cli -- stage-game-build-bundle <archive_id> windows_desktop_playable <target_dir>
```

For the repository default delivery layout, stage this target to
`.\dist\game-build\windows_desktop_playable`. The staged bundle must include
`content/project/brief.adm`, `content/validation/acceptance_matrix.adm`,
`content/validation/scenario_test_plan.adm`,
`content/validation/runtime_validation_report.adm`, and
`content/validation/production_readiness.adm` next to design, development, assets, and SDK content.
If `validation/runtime_execution_results.adm` exists in the archive, the game build bundle includes
it as optional runtime evidence.

Stage the SDK delivery bundle:

```powershell
cargo run -p adm-cli -- stage-sdk-bundle <archive_id> <target_dir>
```

The SDK bundle always includes `sdk/index.adm` and carries optional delivery evidence such as
`package/build_targets.adm`, runtime/scenario validation files, runtime execution results,
production readiness, and engine build history when those files exist in the archive.

Stage the generated Unity project scaffold:

```powershell
cargo run -p adm-cli -- stage-unity-project <archive_id> windows_desktop_playable <unity_project_dir>
```

The Unity scaffold carries the original project brief as
`Assets/AutoDesignMaker/Generated/project_brief.adm`, the same trace data as
`Assets/AutoDesignMaker/Generated/acceptance_matrix.adm`, scenario test plan data as
`Assets/AutoDesignMaker/Generated/scenario_test_plan.adm`, runtime validation data as
`Assets/AutoDesignMaker/Generated/runtime_validation_report.adm`, and the production readiness
report as `Assets/AutoDesignMaker/Generated/production_readiness.adm`. The generated content index
references these files so engine-side review can inspect the project intent, scenario-to-build-target
contract, playable-smoke plan, runtime probe contract, and production readiness status. When runtime
execution results have been recorded, the Unity scaffold also emits
`Assets/AutoDesignMaker/Generated/runtime_execution_results.adm` and lists it in the generated
content index.

Plan or dry-run a Unity build without starting Unity:

```powershell
cargo run -p adm-cli -- plan-unity-build <archive_id> windows_desktop_playable <unity_exe> <unity_project_dir>
cargo run -p adm-cli -- dry-run-unity-build <archive_id> windows_desktop_playable <unity_exe> <unity_project_dir>
```

Run a real local Unity build only when intentionally launching the local engine process:

```powershell
cargo run -p adm-cli -- run-unity-build <archive_id> windows_desktop_playable <unity_exe> <unity_project_dir> ADM_CONFIRM_LOCAL_ENGINE_BUILD
```

Dry-run and real-run build reports are appended to
`package/engine_build_history.adm` inside the formal project archive.

Plan or dry-run the generated Unity runtime validation runner:

```powershell
cargo run -p adm-cli -- plan-unity-runtime-validation <archive_id> windows_desktop_playable <unity_exe> <unity_project_dir>
cargo run -p adm-cli -- dry-run-unity-runtime-validation <archive_id> windows_desktop_playable <unity_exe> <unity_project_dir>
```

Run the local Unity runtime validation runner only when intentionally launching Unity:

```powershell
cargo run -p adm-cli -- run-unity-runtime-validation <archive_id> windows_desktop_playable <unity_exe> <unity_project_dir> ADM_CONFIRM_LOCAL_ENGINE_BUILD
```

The generated Unity scaffold includes
`Assets/AutoDesignMaker/Editor/AutoDesignMakerRuntimeValidation.cs`. The runner reads
`Assets/AutoDesignMaker/Generated/runtime_validation_report.adm`, mounts the generated runtime
components, writes `Library/AutoDesignMaker/runtime_execution_results.adm`, and then the CLI imports
that file into the formal archive as `validation/runtime_execution_results.adm`.
The desktop shell exposes the same Unity build and runtime validation workflow. The `Unity Build`
row can stage the project, inspect preflight readiness, plan the command, dry-run without starting
Unity, and launch the guarded local Unity build through `Run Unity` when the confirmation token is
set to `ADM_CONFIRM_LOCAL_ENGINE_BUILD`. `Run Unity` appends the execution report to
`package/engine_build_history.adm`. The `Runtime Val` row can plan, dry-run, launch the guarded
runtime validation through `Run Runtime`, and record a runtime result file into the selected archive.
`Run Runtime` imports Unity's generated `validation/runtime_execution_results.adm` result after the
local process succeeds.

On a real Unity machine, use the guarded Unity acceptance script to run the full editor build and
runtime validation chain, then refresh delivery artifacts and acceptance reports:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\unity_acceptance_gate.ps1 -ArchiveId '<archive_id>' -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'
powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'
powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -RequireExternalAcceptance -RequireAiInvoke -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'
```

Omit `-ArchiveId` to use the latest formal archive. Omit `-UnityExe` to use `ADM_UNITY_EDITOR`,
`UNITY_EDITOR_PATH`, or the default Unity discovery paths. Omit `-DataRoot` only when the AI provider
was configured under the default data root. Use `-DryRun` to inspect the exact command sequence
without launching Unity. Add `-RequireExternalAcceptance` only when a real non-mock AI provider is
also configured and the final external acceptance report must be `ready=true`; add
`-RequireAiInvoke` only when that report must also prove a successful real AI network invocation.
`-RequireAiInvoke` must be paired with `-RequireExternalAcceptance` in the Unity and release gate
wrappers.
The final external acceptance gate requires imported Unity playmode evidence; smoke or CLI-generated
runtime evidence remains useful local delivery evidence, but it does not satisfy the real Unity
acceptance gate.

Run the full delivery check after staging release, game build, SDK, and Unity outputs:

```powershell
cargo run -p adm-cli -- delivery-doctor
```

`delivery-doctor` should print `ready=true` with `game_build_bundle_ready=true`,
`sdk_bundle_ready=true`, and `unity_project_ready=true`.

Write a final release acceptance gate after the release, game bundle, SDK bundle, and Unity scaffold
have been staged:

```powershell
cargo run -p adm-cli -- release-acceptance
```

`release-acceptance` runs the delivery doctor, launches the staged
`AutoDesignMaker-rust.exe --smoke` when delivery is ready, writes
`dist/AutoDesignMaker-rust/release-acceptance.adm`, and exits non-zero if either the delivery checks
or smoke evidence fail.
