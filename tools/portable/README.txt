AutoDesignMaker NEWrust - Portable Trial
========================================

Start-AutoDesignMaker.cmd
  Recommended. Starts the application with saves, logs, configuration, pipeline
  state, and generated artifacts stored under this directory's user_data folder.
  It first checks for Microsoft Edge WebView2 Runtime and prints the official
  installer link instead of launching a guaranteed-broken desktop window.
  The UI defaults to pure Simplified Chinese. To start the pure English UI, set
  ADM_NEWRUST_LANGUAGE=en-US before running this launcher. Supported values are
  zh-CN and en-US; there is no visible language selector in this version.
  New design exports and Step00-14 artifacts follow the selected language. Each
  pipeline run records its artifact locale so a resumed run cannot mix languages.

AutoDesignMaker.exe
  The executable discovers this portable root only from the files beside it. No
  source-project path is embedded or required. Keep the launcher and manifests
  beside the executable when moving the directory to another writable location.

knowledge\ and pipeline\artifact_layer
  Required design taxonomy, Schema, read-only SDK/Skill seeds, resource manifest,
  and pipeline protocol data. Keep these directories beside the executable. Move
  or distribute the complete portable directory, never the executable by itself.

build-manifest.json
  Records the executable SHA-256, Git revision, target architecture, static CRT
  mode, toolchain/lockfile versions, and staged resource inventory.

portable-resource-manifest.json
  Identifies this directory as a portable resource root and records the digest of
  every bundled resource group. It is separate from the source-project manifest.

Save Manager
  New Project Save keeps the current design but clears pipeline/generated state.
  Save As Copy keeps the complete persistent workspace. Save Current only writes
  the bound save. Loading another save asks whether to save, discard, or cancel.
  Locked or corrupt saves remain visible with inspection and recovery actions.

Project Templates
  View Templates opens the bundled and current-draft template browser. Loading a
  template replaces the current design only after confirmation. Save as Template
  writes an atomic custom template into the current draft; formal saves and Save As
  Copy preserve it. Built-in templates cannot be overwritten or deleted.

The application requires Windows 10/11 x64 and Microsoft Edge WebView2 Runtime.
The portable executable statically links the MSVC CRT and does not require Rust,
Node.js, npm, Git, or the source project on the target computer. The destination
directory must be writable because portable user data is stored beside the app.
Unity validation and live AI-provider quality must be performed in the target
environment; failures and blockers are shown by the application.

Portable update and cleanup
  Stop every AutoDesignMaker process before replacing or finalizing this directory.
  Build, swap, finalization, recovery, and failed-artifact cleanup take the same
  output-level exclusive lock. An unresolved transaction receipt blocks a later
  build; each later operation acquires a fresh lock, so no process must keep a file
  handle open while waiting for human finalization.
  A successful update retains the prior directory as a transaction backup. The
  receipt binds the exact transaction_id in build-manifest.json, the immutable
  candidate-tree digest, and the separately measured user_data tree. The immutable
  digest excludes only user_data and the ephemeral update marker. The build never
  deletes its recovery backup. From the source checkout, run
  tools\Finalize-PortableSwap.ps1 first without -Execute to validate the transaction,
  then with -Execute to remove only its proven backup. A failed candidate is isolated
  as .failed-* and may be removed only through the source checkout's
  tools\portable\Remove-FailedPortableArtifact.ps1 using the recorded transaction;
  that command is also a dry run unless -Execute is supplied. Smoke execution uses an
  explicit redirected process handle, waits for the GUI-subsystem
  executable to exit, records its real exit code/output, and kills then reaps it on timeout.
  This avoids PowerShell treating a still-running GUI executable as a completed command.
  Successful explicit finalization retains the globally newest five receipts for the output
  name, preventing receipt files from accumulating forever. Backup and failed
  directories are first atomically renamed to transaction-bound .retired-* tombstones;
  interrupted or partial deletion is resumed idempotently by the same command. A
  receipt left at stage_smoke_passed or backup_created is reconciled by the finalizer;
  a validated stage with no recorded smoke pass is discarded only by explicit failed
  cleanup. Invalid topology or a wrong transaction_id stops without deleting either
  the legal live directory or its recovery backup.
