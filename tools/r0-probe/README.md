# R0 technical probe

This disposable harness verifies the code-production path before the v2 compiler is built:

1. Parse and validate the fixed minimal `GameSpec`.
2. Generate Unity project files through `CodexPatchRunner`.
3. Prove that an undeclared adapter write is rejected without a project mutation.
4. Generate one runtime script through the current `WorkUnit` executor and compile it in Unity batch mode.
5. Build a Windows player and run its deterministic smoke marker.

Run from the repository root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\r0-probe\Invoke-R0Probe.ps1 `
  -UnityEditor $env:UNITY_EDITOR_PATH `
  -Repeat 2
```

Generated project, player, logs, journal and report stay under `target/r0-probe/`. The directory is reset only when its probe ownership marker is present. It is not an R1 content source.
