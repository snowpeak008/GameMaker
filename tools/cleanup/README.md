# Guarded generated-file cleanup

`tools/clean-generated.ps1` is the only ordinary generated-file cleaner. It is a
two-phase planner/executor and defaults to dry-run. `-Execute` is required before an
allowlisted target can be removed; if any requested target is refused, no requested
target is removed.

## Ordinary generated targets

The allowlist is intentionally narrow:

- `target/` and descendants;
- `.local-build/` and descendants;
- `web/dist/`, `web/test-results/`, `web/playwright-report/`, and `web/.playwright/`;
- exact `web/node_modules/` only when it is passed with `-Target` (never by default);
- generated children of `gates/`, except the retained `gates/README.md` and
  `gates/standalone-release-evidence.json`;
- purpose-named children of `.tmp/` (`adm-newrust-*`, `cargo-*`, `web-*`,
  `gate-*`, `browser-*`, `playwright-*`, or `test-*`);
- the exact `.tmp/` root only after it is completely empty (a second cleanup pass
  removes the root after its allowlisted children are gone);
- verified `dist/.<name>.stage-*` directories.

The project root, its ancestors, `.git`, source/resource/test/documentation folders,
out-of-bound paths, reparse points, and ordinary targets containing `user_data` are
refused. Supply every known real/local data path with `-ProtectedUserData`; the path,
all descendants, and every ancestor that could recursively delete it are refused.
Cargo targets are also refused while a Cargo/rustc/link process references the project
or output path, or while `.cargo-lock`/`.cleanup-active.lock` is held. Web output is
refused while a matching Node/npm process references the project or output path.

Portable `dist/.<name>.previous-*` and `dist/.<name>.backup-*` directories are always
`report-only`, including with `-Execute`. Only the future `Finalize-PortableSwap`
transaction may delete such a recovery backup.

`gates/standalone-release-evidence.json` is release evidence, not disposable gate
output. Default discovery omits it, and even an explicit ordinary-cleanup request is
`report-only`. Other generated gate files and directories remain cleanup candidates.

```powershell
# Default dry-run over discovered generated targets.
powershell -File .\tools\clean-generated.ps1 -ScanPortableStaging

# Explicit dry-run, then execution after reviewing the same target list.
powershell -File .\tools\clean-generated.ps1 -Target .\target, .\web\dist
powershell -File .\tools\clean-generated.ps1 -Target .\target, .\web\dist -Execute
```

A portable stage is deletable only when it contains `.adm-cleanup-stage.json` with
schema version 1, `kind=verified-portable-stage`, its exact canonical `targetPath`,
`verified=true`, and the SHA-256 tree measure of an empty `user_data`. A non-empty or
unverified stage is refused.

`.local-build` participates in default discovery. `web/node_modules` is an explicit
dependency-cache decision and must be named directly:

```powershell
powershell -File .\tools\clean-generated.ps1 -Target .\web\node_modules
```

## Trusted owned-ephemeral leases

Test user-data copies and clean-clone/relocation workspaces are outside the ordinary
allowlist. Do not hand-author their manifest. The issuer first proves a random payload
boundary is absent, creates an empty target, and writes the authoritative manifest and
external root marker below the validated project's controlled lease root:

```text
<project>/.tmp/cleanup-leases/<lease-id>/
|-- owner-manifest.json
|-- root-marker.json
`-- seal.json                 # created only by the seal operation

<chosen-temp-parent>/adm-newrust-cleanup-<lease-id>/
`-- owned-user-data/          # or owned-workspace/
    `-- .adm-cleanup-root.json  # created only by the seal operation
```

Issue a lease, populate its already-created empty target, then seal it. For a Git clone,
clone into the existing empty workspace (for example, run `git clone <source> .` from
inside it) before sealing:

```powershell
$lease = powershell -File .\tools\new-cleanup-lease.ps1 `
  -Operation Issue `
  -Kind owned-ephemeral-workspace `
  -TempParent $env:TEMP `
  -Json | ConvertFrom-Json

# Populate $lease.target, then seal the immutable root identity.
powershell -File .\tools\new-cleanup-lease.ps1 `
  -Operation Seal `
  -Kind $lease.kind `
  -Target $lease.target `
  -OwnerManifest $lease.ownerManifest `
  -Nonce $lease.nonce
```

The schema-v2 manifest, external marker, target marker, and seal cross-check project
ID, project-root marker hash, exact canonical target, kind, cryptographic nonce,
lease ID, issue/expiry/seal times, and marker/manifest SHA-256. The finalizer accepts
only a sealed issuer record under the current project's controlled lease root. For
`owned-ephemeral-user-data`, it also recomputes the protected source tree's file count,
bytes, and digest and refuses cleanup if the source changed.

`owned-ephemeral-workspace` is the sole mode that may remove the target clone's root
`.git`. It refuses the real source root, source/target overlap, nested repositories,
unsealed/relocated/tampered/expired leases, bad markers, and reparse points.

An executed owned cleanup never recursively deletes the live target name. After the
final lease recheck it first atomically renames the target, within the same controlled
payload boundary and therefore on the same volume, to a deterministic tombstone bound
to the lease ID and 256-bit nonce:

```text
adm-newrust-cleanup-<lease-id>/
`-- .adm-cleanup-tombstone-<lease-id>-<nonce>/
```

Only that tombstone is recursively removed. If the process stops after the rename or
during recursive removal, the active receipt remains intact. A later dry-run reports
`dry-run-resume-delete`; a later `-Execute` revalidates the receipt, source proof,
deterministic tombstone path, reparse guards, and then resumes idempotently. The live
target and its tombstone existing at the same time is an ambiguous state and is
refused.

This lease is an engineering guard against operator mistakes and stale or relocated
manifests. It is not a security boundary against malicious code running as the same
OS user: that user can modify both the project-controlled lease store and payload.

After an executed cleanup has removed the owned target, retire the validated lease to
remove its now-empty payload boundary and controlled receipt. Retirement first removes
the proven-empty external boundary while the receipt is still active, then atomically
renames the entire receipt directory to a lease-ID/nonce-bound retirement tombstone
under the same controlled lease root. Only the tombstone is recursively removed. A
retry can safely finish an intact or partially removed retirement tombstone, and a
retry after completion is a no-op. Retirement refuses a live target or payload
tombstone, an unexpected receipt entry, a reparse point, or any path outside the
controlled lease root:

```text
<project>/.tmp/cleanup-leases/
`-- .adm-cleanup-retirement-<lease-id>-<nonce>/
```

```powershell
powershell -File .\tools\new-cleanup-lease.ps1 `
  -Operation Retire `
  -Kind $lease.kind `
  -Target $lease.target `
  -OwnerManifest $lease.ownerManifest `
  -Nonce $lease.nonce
```

```powershell
powershell -File .\tools\clean-generated.ps1 `
  -Kind owned-ephemeral-workspace `
  -Target $lease.target `
  -OwnerManifest $lease.ownerManifest `
  -Nonce $lease.nonce

# Add -Execute only after reviewing the dry-run result.
```

## Fixture verification and cleanup node

The self-contained test script creates a unique operating-system temp fixture, tests
dry-run and explicitly executed deletions only inside that fixture, and removes the
fixture in a guarded `finally` cleanup node:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass `
  -File .\tools\cleanup\test-clean-generated.ps1

powershell -NoProfile -ExecutionPolicy Bypass `
  -File .\tools\cleanup\test-cleanup-lease.ps1
```

The fixtures also inject interruptions immediately after the owned-target rename,
during partial tombstone deletion, and immediately after the receipt-retirement
rename. Each case must recover idempotently while source and protected-data tree
digests remain unchanged. The exact empty `.tmp` root is still removable only after
all controlled state has been retired and no unknown entry remains.
