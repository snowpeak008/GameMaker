# AutoDesignMaker NEWrust

`NEWrust/` is the Tauri 2 + Rust rebuild of AutoDesignMaker. The desktop application
contains the design workbench, save management, AI configuration/interview surfaces,
the Step00-14 pipeline, safe execution summaries, logs, patches, packaging, and SDK views.

This directory is an independent source repository. All product data, Schema,
protocol resources, test baselines, and release tooling live below this root; no
parent AutoDesignMaker or Python project is required. See
`docs/independence/README.md` for the enforced boundary.

## Requirements

- Windows 10/11 x64 with Microsoft Edge WebView2 Runtime.
- Node.js 22-24 and npm 10-11 (the Web build script itself uses only Node.js; Playwright checks
  require the declared development dependency).
- Rust 1.96 or newer with the `x86_64-pc-windows-msvc` toolchain.
- Visual Studio C++ Build Tools for the MSVC linker.

## Development

Run commands from `NEWrust/`:

```powershell
# Build the static Web UI consumed by Tauri.
npm --prefix web run build

# Check and test the Rust workspace.
cargo fmt --all -- --check
cargo check --workspace --locked
cargo test --workspace --locked

# Build or run the desktop application.
cargo build --locked -p desktop-tauri --release
cargo run -p desktop-tauri
```

The desktop application normally stores runtime data in its Tauri application-data
directory. Set `ADM_NEWRUST_DATA_DIR` to use an explicit data directory and
`ADM_NEWRUST_SOURCE_ROOT` to override where `knowledge/design_data` is loaded from.
The override is for development/testing and must point to a valid standalone source
root. Release builds do not search parent directories or use the compiler's source path.

## Language Modes

Application-owned UI text supports two complete modes:

- `zh-CN`: pure Simplified Chinese UI, the default.
- `en-US`: pure English UI.

There is intentionally no visible language selector yet. Choose the startup mode
with `ADM_NEWRUST_LANGUAGE`; the portable launcher preserves an externally supplied
value and otherwise defaults to `zh-CN`:

```powershell
$env:ADM_NEWRUST_LANGUAGE = "en-US"
.\dist\AutoDesignMaker-NEWrust\Start-AutoDesignMaker.cmd
```

The Web localization API also exports `replaceLanguageMode(language)`, which persists
the choice and reloads the application for a future settings entry to call. The
built-in design catalog is localized at display time through stable IDs: this covers
all 16 domains, 103 nodes, 515 checklist labels, inline/shared option groups and
options, gameplay-system names, the emergency fallback taxonomy, project-profile
system fields, L4 missing paths, and built-in quality violations. The authoritative
design/save schema remains language-neutral. New design exports and Step00-14 runs
capture an immutable `artifact_locale`; user-facing Markdown, messages, reasons,
acceptance text, and generation prompts follow that locale. The first complete
artifact catalog is `zh-CN`, while the shared locale contract keeps `en-US` and future
catalogs as protocol-compatible extensions. A stopped run resumes with the locale
captured by its checkpoint instead of mixing languages after the UI preference changes.

Machine IDs, JSON keys, status/code values, paths, file formats, schema identifiers,
commands, and user-authored text are never translated. Localized Markdown is a view;
structured JSON and stable IDs are the machine-to-machine protocol. Legacy Chinese
and English Markdown readers remain only for older packages.

## Project Templates

The design workbench provides a real template browser rather than a client-side
placeholder. It lists lightweight metadata for the 25 bundled templates and custom
templates in the current draft without transferring every complete project state to
the Web UI. Built-in names, summaries, and analysis presentation follow the selected
application language; custom names and content remain verbatim user data.

Loading a template requires confirmation and sends only its stable template ID. Rust
loads the authoritative `projectState`, removes AI interview history, infers missing
gameplay-system selections, normalizes the design, updates the project name, and then
autosaves. A failed autosave restores the previous in-memory project.

Custom templates are stored under the active desktop session at
`drafts/<session>/workspace/projects/templates`. Saving is atomic, built-in templates
cannot be overwritten or deleted, duplicate custom templates require explicit
overwrite confirmation, and corrupt JSON files are skipped with visible warnings
instead of making the complete browser unusable. Because templates live inside the
draft workspace, Save As Copy and formal-save restore preserve them; a new blank
project save intentionally starts with an empty workspace.

## Save Semantics And Recovery

Each desktop window owns an independent autosaved draft. A formal save is an explicit,
versioned commit of that draft; another window cannot edit the same formal save while
its operating-system lock is held.

- **New Project Save** keeps the current design decisions but starts with an empty
  pipeline/generated workspace.
- **Save As Copy** preserves the complete persistent workspace and binds the window to
  the new save.
- **Save Current** only writes the currently bound save. It never overwrites an
  arbitrary selected save.
- **Load** requires an explicit choice to save the current draft, discard it, or cancel.
  A detached draft must first be saved as a copy unless discard is chosen.

Formal commits use same-volume staging, before-image transaction journals, and durable
archive/index locks. Interrupted transactions are recovered before a later save
operation proceeds. Corrupt draft/runtime state is quarantined; corrupt formal saves
remain listed so their directory can be inspected or deleted. Cleanup and recovery
warnings are returned to the UI and written to Runtime Logs.

Automatic draft pruning is disabled by default (`pruneDraftsKeepCount = 0`) so recovery
data is not deleted without an explicit retention policy. The loader also supports
Python saves that have no Rust autosave file by reading the latest verified
`design_project` execution object without modifying the legacy archive.

## Portable Trial Build

The release script builds the Web UI, compiles the locked Rust release, and stages a
self-contained trial directory with the executable and the required design taxonomy:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\build-portable.ps1
```

The default local update preserves and verifies any existing `user_data` before the
portable directory is replaced. To create a distributable build with an empty data
directory, use a separate output name; the script refuses to erase non-empty data:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\build-portable.ps1 `
  -OutputName AutoDesignMaker-NEWrust-release -CleanUserData
```

`adm-new-cli dist build --execute` delegates to this same locked portable script; it
does not maintain a second release layout.

Output:

```text
dist/AutoDesignMaker-NEWrust/
|-- AutoDesignMaker.exe
|-- Start-AutoDesignMaker.cmd
|-- README.txt
|-- build-manifest.json
|-- portable-resource-manifest.json
|-- knowledge/{design_data,schemas,market_data,sdks,skills}/
|-- pipeline/artifact_layer/
`-- user_data/
```

Use `Start-AutoDesignMaker.cmd` for a portable trial: it pins the runtime data directory
to the staged `user_data` folder. Release resource discovery accepts only the complete
manifest-verified directory beside the executable; it does not honor a source override,
search the current directory, or fall back to an embedded taxonomy. Moving only the
`.exe`, changing a tracked resource, or changing either manifest makes startup/smoke
fail closed.

`build-manifest.json` binds the executable, launcher, artifact registry, resource
manifest, Git commit, lockfiles, toolchain, target architecture, CRT result, and
preserved/clean user-data digest. `portable-resource-manifest.json` binds every shipped
resource tree. A staged release can be checked without opening the GUI:

```powershell
.\dist\AutoDesignMaker-NEWrust-release\AutoDesignMaker.exe --smoke `
  --smoke-report "$env:TEMP\adm-newrust-smoke.json"
```

An update that replaces an existing local portable keeps its recovery backup until
`tools/Finalize-PortableSwap.ps1` validates the transaction, live smoke result, locks,
and user-data digests. The finalizer defaults to dry-run; only its explicit `-Execute`
path may remove that backup.

## Standalone release verification and cleanup

Formal release evidence comes from a `--no-local` clone under a same-volume root path
that is outside both the source and legacy project trees and contains Chinese characters
and spaces. The verifier has no development/skip mode. It runs the complete Web/Rust
checks, builds the clean portable at `dist/AutoDesignMaker-NEWrust-release`, verifies
its manifests/hash/current HEAD/x64 static-CRT evidence, then closes the leased clone
through dry-run, delete, and receipt retirement:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File .\tools\verify-standalone.ps1

cargo run --locked -p adm-new-cli -- release-gate
```

`gates/standalone-release-evidence.json` is fixed-path, UTF-8 without BOM, expires
after 24 hours, and records structured command/exit/duration/output-hash evidence for
all required checks. `tools/security-scan-allowlist.json` is fail-closed: every reviewed
path exception is pinned to its complete file SHA-256. The release evidence is a
same-user guard against stale or accidental claims, not cryptographic attestation.

The verifier atomically replaces any previous claim with a new schema-v2 `running`
record before the first check. Only the final write can set `status=passed`; an
interrupted run therefore leaves either `running`, `blocked`, expired, or missing
evidence, all of which the Rust release gate rejects. The portable evidence also pins
the finalized swap-receipt path and SHA-256, transaction ID, output root, build
manifest, and current Git HEAD. Any unresolved receipt, stage, backup, failed
candidate, retirement tombstone, or active/stale operation lock blocks release; the
resolved persistent lock file is retained for the next exclusive operation.

The `generated_cleanup` check covers both the leased clone and source-owned Cargo/Web/
gate outputs. It must observe guarded dry-run and execute results, preserve the formal
portable plus finalized swap receipt, and only then write the final evidence file.

Generated Cargo/Web/gate output is cleaned with a guarded dry-run followed by an
explicit execution. Always pass every protected local data path:

```powershell
powershell -File .\tools\clean-generated.ps1 `
  -ProtectedUserData .\dist\AutoDesignMaker-NEWrust\user_data

powershell -File .\tools\clean-generated.ps1 `
  -ProtectedUserData .\dist\AutoDesignMaker-NEWrust\user_data `
  -Execute
```

## Trial Workflow

1. Start `dist/AutoDesignMaker-NEWrust/Start-AutoDesignMaker.cmd` so all trial data
   stays under the staged `user_data/` directory.
2. Build or edit a project in the design workbench. The save manager creates formal
   saves, switches between them, and restores design, pipeline, logs, patches, and
   generated outputs after restart.
3. Configure a Codex CLI, Claude CLI, or OpenAI-compatible completion provider in
   AI settings. API providers support direct keys, environment references, and
   explicitly configured no-auth local services.
4. Run Step00-14 from the pipeline page. Step07 pauses for style confirmation; every
   step exposes its status, warnings, errors, and semantic quality. Internal outputs,
   artifact lists, file paths, and raw Base64 content remain hidden. Step07 alone
   presents validated image previews with generated/fallback/failure status.
5. Open the package page after Step14. Package validation consumes the current
   Step11-14 outputs and, when available, external Unity evidence from
   `stage_14/actual_project_file_audit.json` and
   `stage_14/unity_validation_summary.json`. Missing real Unity evidence is reported
   as a blocker instead of being treated as a successful validation.

Real Unity execution, live-provider output quality, and generated-artifact quality
remain target-environment acceptance checks; the application preserves their internal
evidence while presenting only safe summaries and actionable errors.

## Web Checks

```powershell
npm --prefix web ci
npm --prefix web test
npm --prefix web run e2e
npm --prefix web run design-content-check
npm --prefix web run i18n-test
npm --prefix web run language-gate
npm --prefix web run ui-gate
npm --prefix web run ui-baseline-gate
```

When the authoritative design taxonomy changes, run
`npm --prefix web run design-content` to regenerate the stable-ID display catalog,
then rerun the checks above. The UI gate captures both languages at desktop, compact,
and narrow viewports (90 screenshots) and rejects clipping plus invalid
template-control heights in critical toolbars and dialogs. The separate baseline gate
currently verifies 93 records.

The generated Web output is `web/dist/`. It is embedded into the desktop executable
during the Rust release build.

## Workspace Layout

- `apps/desktop-tauri`: Tauri desktop shell and command adapters.
- `apps/adm-new-cli`: command-line gates and diagnostics.
- `crates/adm-new-*`: foundation, contracts, design, save, AI, pipeline, artifact,
  packaging, patch, SDK, application, and Tauri-command layers.
- `web`: desktop Web UI and its test/gate scripts.
- `gates`: generated local gate evidence.
- `tools`: reproducible release and staging tools.
