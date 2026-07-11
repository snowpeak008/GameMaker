AutoDesignMaker NEWrust - Portable Trial
========================================

Start-AutoDesignMaker.cmd
  Recommended. Starts the application with saves, logs, configuration, pipeline
  state, and generated artifacts stored under this directory's user_data folder.
  The UI defaults to pure Simplified Chinese. To start the pure English UI, set
  ADM_NEWRUST_LANGUAGE=en-US before running this launcher. Supported values are
  zh-CN and en-US; there is no visible language selector in this version.
  New design exports and Step00-14 artifacts follow the selected language. Each
  pipeline run records its artifact locale so a resumed run cannot mix languages.

AutoDesignMaker.exe
  Starts the same application using the normal Windows application-data directory.

knowledge\design_data
  Required design taxonomy and pipeline data. Keep this directory beside the
  executable. Do not distribute or move the executable by itself.

build-manifest.json
  Records the executable SHA-256 and staged resource inventory.

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
Unity validation and live AI-provider quality must be performed in the target
environment; failures and blockers are shown by the application.
