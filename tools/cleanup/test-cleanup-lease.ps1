[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$cleaner = Convert-Path (Join-Path $PSScriptRoot '..\clean-generated.ps1')
$issuer = Convert-Path (Join-Path $PSScriptRoot '..\new-cleanup-lease.ps1')
$pathsModule = Join-Path $PSScriptRoot 'GuardedCleanup.psm1'
Import-Module $pathsModule -Force

$sandbox = Join-Path ([IO.Path]::GetTempPath()) ("adm-newrust-lease-tests-{0}" -f ([guid]::NewGuid().ToString('N')))
$project = Join-Path $sandbox 'fixture source'
$payloadParent = Join-Path $sandbox 'payloads'
$passed = 0
$projectMarkerHashBefore = $null
$cargoManifestHashBefore = $null

function Write-Utf8Json {
    param([Parameter(Mandatory = $true)]$Value, [Parameter(Mandatory = $true)][string]$Path)
    $Value | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $Path -Encoding UTF8
}

function New-TestFile {
    param([Parameter(Mandatory = $true)][string]$Path, [string]$Content = 'fixture')
    $parent = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    Set-Content -LiteralPath $Path -Value $Content -Encoding UTF8
}

function Assert-True {
    param([Parameter(Mandatory = $true)][bool]$Condition, [Parameter(Mandatory = $true)][string]$Message)
    if (-not $Condition) { throw "assertion failed: $Message" }
}

function Assert-Equal {
    param($Expected, $Actual, [Parameter(Mandatory = $true)][string]$Message)
    if ($Expected -ne $Actual) { throw "assertion failed: $Message (expected '$Expected', actual '$Actual')" }
}

function Assert-MeasureEqual {
    param($Expected, $Actual, [Parameter(Mandatory = $true)][string]$Message)
    Assert-Equal ([int64]$Expected.fileCount) ([int64]$Actual.fileCount) "$Message file count"
    Assert-Equal ([int64]$Expected.bytes) ([int64]$Actual.bytes) "$Message bytes"
    Assert-Equal ([string]$Expected.digest) ([string]$Actual.digest) "$Message digest"
}

function Get-PayloadTombstone {
    param([Parameter(Mandatory = $true)]$Lease)
    $manifest = Get-Content -LiteralPath $Lease.ownerManifest -Raw -Encoding UTF8 | ConvertFrom-Json
    return Get-OwnedCleanupTombstonePath -Boundary ([string]$manifest.boundaryPath) `
        -LeaseId ([string]$manifest.leaseId) -Nonce ([string]$Lease.nonce)
}

function Get-ReceiptTombstone {
    param([Parameter(Mandatory = $true)]$Lease)
    $leaseDirectory = Split-Path -Parent $Lease.ownerManifest
    $leaseRoot = Split-Path -Parent $leaseDirectory
    $leaseId = Split-Path -Leaf $leaseDirectory
    return Get-LeaseRetirementTombstonePath -LeaseRoot $leaseRoot `
        -LeaseId $leaseId -Nonce ([string]$Lease.nonce)
}

function Invoke-JsonProcess {
    param(
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [string]$Failpoint
    )
    $previousFailpoint = [Environment]::GetEnvironmentVariable('AUTODESIGNMAKER_CLEANUP_FAILPOINT', 'Process')
    try {
        if ([string]::IsNullOrWhiteSpace($Failpoint)) {
            [Environment]::SetEnvironmentVariable('AUTODESIGNMAKER_CLEANUP_FAILPOINT', $null, 'Process')
        }
        else {
            [Environment]::SetEnvironmentVariable('AUTODESIGNMAKER_CLEANUP_FAILPOINT', $Failpoint, 'Process')
        }
        $output = @(& powershell.exe @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
        $text = ($output | ForEach-Object { [string]$_ }) -join "`n"
        try { $payload = $text | ConvertFrom-Json }
        catch { throw "child script did not return JSON (exit $exitCode): $text" }
        [pscustomobject]@{ ExitCode = $exitCode; Payload = $payload }
    }
    finally {
        [Environment]::SetEnvironmentVariable(
            'AUTODESIGNMAKER_CLEANUP_FAILPOINT', $previousFailpoint, 'Process')
    }
}

function Issue-Lease {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('owned-ephemeral-user-data', 'owned-ephemeral-workspace')]
        [string]$Kind,
        [string]$SourcePath
    )
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $issuer,
        '-Operation', 'Issue', '-ProjectRoot', $project, '-Kind', $Kind,
        '-TempParent', $payloadParent, '-ValidForMinutes', '60', '-Json')
    if ($SourcePath) { $arguments += @('-SourcePath', $SourcePath) }
    $result = Invoke-JsonProcess -Arguments $arguments
    Assert-Equal 0 $result.ExitCode 'lease issue exit code'
    Assert-Equal 'issued' $result.Payload.operation 'lease issue operation'
    return $result.Payload
}

function Seal-Lease {
    param([Parameter(Mandatory = $true)]$Lease)
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $issuer,
        '-Operation', 'Seal', '-ProjectRoot', $project, '-Kind', $Lease.kind,
        '-Target', $Lease.target, '-OwnerManifest', $Lease.ownerManifest,
        '-Nonce', $Lease.nonce, '-Json')
    $result = Invoke-JsonProcess -Arguments $arguments
    Assert-Equal 0 $result.ExitCode 'lease seal exit code'
    Assert-Equal 'sealed' $result.Payload.operation 'lease seal operation'
    return $result.Payload
}

function Retire-Lease {
    param([Parameter(Mandatory = $true)]$Lease)
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $issuer,
        '-Operation', 'Retire', '-ProjectRoot', $project, '-Kind', $Lease.kind,
        '-Target', $Lease.target, '-OwnerManifest', $Lease.ownerManifest,
        '-Nonce', $Lease.nonce, '-Json')
    $result = Invoke-JsonProcess -Arguments $arguments
    Assert-Equal 0 $result.ExitCode 'lease retirement exit code'
    Assert-Equal 'retired' $result.Payload.operation 'lease retirement operation'
    return $result.Payload
}

function Invoke-RetirementProcess {
    param([Parameter(Mandatory = $true)]$Lease, [string]$Failpoint)
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $issuer,
        '-Operation', 'Retire', '-ProjectRoot', $project, '-Kind', $Lease.kind,
        '-Target', $Lease.target, '-OwnerManifest', $Lease.ownerManifest,
        '-Nonce', $Lease.nonce, '-Json')
    return Invoke-JsonProcess -Arguments $arguments -Failpoint $Failpoint
}

function Invoke-LeaseCleaner {
    param(
        [Parameter(Mandatory = $true)]$Lease,
        [string]$Manifest,
        [string]$Nonce,
        [string]$Failpoint,
        [switch]$Execute
    )
    if (-not $Manifest) { $Manifest = $Lease.ownerManifest }
    if (-not $Nonce) { $Nonce = $Lease.nonce }
    $arguments = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $cleaner,
        '-ProjectRoot', $project, '-Kind', $Lease.kind, '-Target', $Lease.target,
        '-OwnerManifest', $Manifest, '-Nonce', $Nonce, '-Json')
    if ($Execute) { $arguments += '-Execute' }
    return Invoke-JsonProcess -Arguments $arguments -Failpoint $Failpoint
}

function Run-Test {
    param([Parameter(Mandatory = $true)][string]$Name, [Parameter(Mandatory = $true)][scriptblock]$Body)
    & $Body
    $script:passed += 1
    Write-Host "PASS $Name"
}

try {
    New-Item -ItemType Directory -Path $project, $payloadParent -Force | Out-Null
    Write-Utf8Json -Path (Join-Path $project '.project_root') -Value ([ordered]@{
            schemaVersion = 1
            kind = 'source-project-root'
            projectId = 'autodesignmaker-rust-v2'
            workspaceManifest = 'Cargo.toml'
        })
    Set-Content -LiteralPath (Join-Path $project 'Cargo.toml') -Value '[workspace]' -Encoding UTF8
    $projectMarkerHashBefore = (Get-FileHash -LiteralPath (Join-Path $project '.project_root') -Algorithm SHA256).Hash
    $cargoManifestHashBefore = (Get-FileHash -LiteralPath (Join-Path $project 'Cargo.toml') -Algorithm SHA256).Hash

    Run-Test 'issuer creates controlled lease and unsealed targets are refused' {
        $source = Join-Path $sandbox 'protected-one'
        New-TestFile (Join-Path $source 'save.json') 'source-one'
        $lease = Issue-Lease -Kind 'owned-ephemeral-user-data' -SourcePath $source
        $controlledRoot = ConvertTo-NormalizedPath (Join-Path $project '.tmp\cleanup-leases')
        Assert-True (Test-PathWithin -Path $lease.ownerManifest -Boundary $controlledRoot) 'manifest is in controlled lease root'
        Assert-True (Test-Path -LiteralPath $lease.target -PathType Container) 'issuer created empty target root'
        New-TestFile (Join-Path $lease.target 'copy\save.json') 'copy-one'
        $unsealed = Invoke-LeaseCleaner -Lease $lease
        Assert-True ($unsealed.ExitCode -ne 0) 'unsealed target refusal'
        Assert-True (Test-Path -LiteralPath $lease.target) 'unsealed target preserved'
    }

    Run-Test 'sealed user-data lease supports dry-run, execute, and idempotent repeat' {
        $source = Join-Path $sandbox 'protected-two'
        New-TestFile (Join-Path $source 'save.json') 'source-two'
        $lease = Issue-Lease -Kind 'owned-ephemeral-user-data' -SourcePath $source
        New-TestFile (Join-Path $lease.target 'copy\save.json') 'copy-two'
        $null = Seal-Lease $lease
        $dry = Invoke-LeaseCleaner -Lease $lease
        Assert-Equal 0 $dry.ExitCode 'lease dry-run exit code'
        Assert-Equal 'dry-run-delete' $dry.Payload.results[0].action 'lease dry-run action'
        Assert-True (Test-Path -LiteralPath $lease.target) 'dry-run preserved leased target'
        $deleted = Invoke-LeaseCleaner -Lease $lease -Execute
        Assert-Equal 'deleted' $deleted.Payload.results[0].action 'leased target deleted'
        Assert-True (Test-Path -LiteralPath $source) 'protected source preserved'
        Assert-True (Test-Path -LiteralPath $lease.ownerManifest) 'controlled audit manifest preserved'
        $repeat = Invoke-LeaseCleaner -Lease $lease -Execute
        Assert-Equal 'skipped' $repeat.Payload.results[0].action 'leased cleanup repeat no-op'
        $boundary = [string](Get-Content -LiteralPath $lease.ownerManifest -Raw -Encoding UTF8 |
                ConvertFrom-Json).boundaryPath
        $null = Retire-Lease $lease
        Assert-True (-not (Test-Path -LiteralPath $lease.ownerManifest)) 'lease receipt retired'
        Assert-True (-not (Test-Path -LiteralPath $boundary)) 'empty payload boundary retired'
    }

    Run-Test 'owned cleanup resumes after an interruption immediately after atomic rename' {
        $source = Join-Path $sandbox 'protected-rename-crash'
        New-TestFile (Join-Path $source 'save.json') 'rename-crash-source'
        $sourceBefore = Get-TreeMeasure $source
        $lease = Issue-Lease -Kind 'owned-ephemeral-user-data' -SourcePath $source
        New-TestFile (Join-Path $lease.target 'copy\save.json') 'rename-crash-copy'
        $null = Seal-Lease $lease
        $tombstone = Get-PayloadTombstone $lease

        $interrupted = Invoke-LeaseCleaner -Lease $lease -Execute `
            -Failpoint 'after-owned-target-rename'
        Assert-True ($interrupted.ExitCode -ne 0) 'rename interruption reported failure'
        Assert-Equal 'refused' $interrupted.Payload.results[0].action 'rename interruption action'
        Assert-True (-not (Test-Path -LiteralPath $lease.target)) 'original target absent after atomic rename'
        Assert-True (Test-Path -LiteralPath $tombstone -PathType Container) 'lease-bound payload tombstone retained'

        $recoveryPlan = Invoke-LeaseCleaner -Lease $lease
        Assert-Equal 0 $recoveryPlan.ExitCode 'rename recovery dry-run exit code'
        Assert-Equal 'dry-run-resume-delete' $recoveryPlan.Payload.results[0].action 'rename recovery recognized'
        $recovered = Invoke-LeaseCleaner -Lease $lease -Execute
        Assert-Equal 'deleted' $recovered.Payload.results[0].action 'rename recovery completed'
        Assert-True (-not (Test-Path -LiteralPath $tombstone)) 'payload tombstone removed after recovery'
        Assert-MeasureEqual $sourceBefore (Get-TreeMeasure $source) 'protected source after rename recovery'
        $null = Retire-Lease $lease
    }

    Run-Test 'owned cleanup resumes after a partial tombstone deletion' {
        $source = Join-Path $sandbox 'protected-partial-crash'
        New-TestFile (Join-Path $source 'save.json') 'partial-crash-source'
        $sourceBefore = Get-TreeMeasure $source
        $lease = Issue-Lease -Kind 'owned-ephemeral-user-data' -SourcePath $source
        New-TestFile (Join-Path $lease.target 'copy\one.bin') 'one'
        New-TestFile (Join-Path $lease.target 'copy\two.bin') 'two'
        $null = Seal-Lease $lease
        $targetBefore = Get-TreeMeasure $lease.target
        $tombstone = Get-PayloadTombstone $lease

        $interrupted = Invoke-LeaseCleaner -Lease $lease -Execute `
            -Failpoint 'during-owned-tombstone-delete'
        Assert-True ($interrupted.ExitCode -ne 0) 'partial deletion interruption reported failure'
        Assert-True (Test-Path -LiteralPath $tombstone -PathType Container) 'partially deleted tombstone retained'
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $tombstone '.adm-cleanup-root.json'))) 'partial deletion may remove the moved marker'
        $remaining = Get-TreeMeasure $tombstone
        Assert-True ($remaining.fileCount -lt $targetBefore.fileCount) 'failpoint removed part of the owned payload'
        Assert-MeasureEqual $sourceBefore (Get-TreeMeasure $source) 'protected source during partial deletion'

        $recoveryPlan = Invoke-LeaseCleaner -Lease $lease
        Assert-Equal 'dry-run-resume-delete' $recoveryPlan.Payload.results[0].action 'partial deletion recovery recognized'
        $recovered = Invoke-LeaseCleaner -Lease $lease -Execute
        Assert-Equal 'deleted' $recovered.Payload.results[0].action 'partial deletion recovery completed'
        Assert-True (-not (Test-Path -LiteralPath $tombstone)) 'partial tombstone removed after recovery'
        Assert-MeasureEqual $sourceBefore (Get-TreeMeasure $source) 'protected source after partial recovery'
        $null = Retire-Lease $lease
    }

    Run-Test 'lease retirement resumes from its nonce-bound receipt tombstone' {
        $source = Join-Path $sandbox 'protected-retirement-crash'
        New-TestFile (Join-Path $source 'save.json') 'retirement-crash-source'
        $sourceBefore = Get-TreeMeasure $source
        $lease = Issue-Lease -Kind 'owned-ephemeral-user-data' -SourcePath $source
        New-TestFile (Join-Path $lease.target 'copy\save.json') 'retirement-copy'
        $null = Seal-Lease $lease
        $deleted = Invoke-LeaseCleaner -Lease $lease -Execute
        Assert-Equal 'deleted' $deleted.Payload.results[0].action 'retirement fixture target deleted'
        $receiptTombstone = Get-ReceiptTombstone $lease

        $interrupted = Invoke-RetirementProcess -Lease $lease `
            -Failpoint 'after-receipt-tombstone-rename'
        Assert-True ($interrupted.ExitCode -ne 0) 'receipt tombstone interruption reported failure'
        Assert-Equal 'refused' $interrupted.Payload.status 'receipt tombstone interruption status'
        Assert-True (-not (Test-Path -LiteralPath $lease.ownerManifest)) 'active owner path absent after retirement rename'
        Assert-True (Test-Path -LiteralPath $receiptTombstone -PathType Container) 'retirement tombstone retained for recovery'
        Assert-MeasureEqual $sourceBefore (Get-TreeMeasure $source) 'protected source during receipt recovery'

        # Simulate an operating-system interruption after recursive retirement
        # deletion had already removed the former owner file. The deterministic
        # controlled tombstone must remain sufficient to finish safely.
        Remove-Item -LiteralPath (Join-Path $receiptTombstone 'owner-manifest.json') -Force
        Assert-True (-not (Test-Path -LiteralPath (Join-Path $receiptTombstone 'owner-manifest.json'))) 'partially removed retirement receipt prepared'

        $recovered = Invoke-RetirementProcess -Lease $lease
        Assert-Equal 0 $recovered.ExitCode 'receipt tombstone recovery exit code'
        Assert-Equal 'retired' $recovered.Payload.operation 'receipt tombstone recovery operation'
        Assert-True (-not (Test-Path -LiteralPath $receiptTombstone)) 'receipt tombstone removed after recovery'
        $repeat = Invoke-RetirementProcess -Lease $lease
        Assert-Equal 0 $repeat.ExitCode 'retirement terminal repeat exit code'
        Assert-Equal 'retired' $repeat.Payload.operation 'retirement terminal repeat operation'
        Assert-MeasureEqual $sourceBefore (Get-TreeMeasure $source) 'protected source after receipt recovery'
    }

    Run-Test 'source digest changes and target marker tampering are refused' {
        $source = Join-Path $sandbox 'protected-three'
        New-TestFile (Join-Path $source 'save.json') 'source-three'
        $changed = Issue-Lease -Kind 'owned-ephemeral-user-data' -SourcePath $source
        New-TestFile (Join-Path $changed.target 'copy.bin')
        $null = Seal-Lease $changed
        New-TestFile (Join-Path $source 'save.json') 'source-three-mutated'
        $changedResult = Invoke-LeaseCleaner -Lease $changed -Execute
        Assert-True ($changedResult.ExitCode -ne 0) 'changed source refused'
        Assert-True (Test-Path -LiteralPath $changed.target) 'changed-source target preserved'

        $sourceTwo = Join-Path $sandbox 'protected-four'
        New-TestFile (Join-Path $sourceTwo 'save.json')
        $tampered = Issue-Lease -Kind 'owned-ephemeral-user-data' -SourcePath $sourceTwo
        New-TestFile (Join-Path $tampered.target 'copy.bin')
        $null = Seal-Lease $tampered
        New-TestFile (Join-Path $tampered.target '.adm-cleanup-root.json') 'tampered-marker'
        $tamperedResult = Invoke-LeaseCleaner -Lease $tampered -Execute
        Assert-True ($tamperedResult.ExitCode -ne 0) 'tampered marker refused'
        Assert-True (Test-Path -LiteralPath $tampered.target) 'tampered-marker target preserved'
    }

    Run-Test 'relocated trusted manifest and wrong nonce are refused' {
        $source = Join-Path $sandbox 'protected-five'
        New-TestFile (Join-Path $source 'save.json')
        $lease = Issue-Lease -Kind 'owned-ephemeral-user-data' -SourcePath $source
        New-TestFile (Join-Path $lease.target 'copy.bin')
        $null = Seal-Lease $lease
        $forgedDir = Join-Path $sandbox 'self-consistent-external-forgery'
        New-Item -ItemType Directory -Path $forgedDir | Out-Null
        $forgedManifest = Join-Path $forgedDir 'owner-manifest.json'
        Copy-Item -LiteralPath $lease.ownerManifest -Destination $forgedManifest
        $external = Invoke-LeaseCleaner -Lease $lease -Manifest $forgedManifest -Execute
        Assert-True ($external.ExitCode -ne 0) 'external manifest copy refused'
        $wrongNonce = Invoke-LeaseCleaner -Lease $lease -Nonce ('0' * 64) -Execute
        Assert-True ($wrongNonce.ExitCode -ne 0) 'wrong nonce refused'
        Assert-True (Test-Path -LiteralPath $lease.target) 'forgery target preserved'
    }

    Run-Test 'sealed workspace may remove only its owned root .git and never the source root' {
        $lease = Issue-Lease -Kind 'owned-ephemeral-workspace'
        New-TestFile (Join-Path $lease.target '.git\config')
        New-TestFile (Join-Path $lease.target '.project_root')
        New-TestFile (Join-Path $lease.target 'user_data\test-save.json')
        $null = Seal-Lease $lease
        $deleted = Invoke-LeaseCleaner -Lease $lease -Execute
        Assert-Equal 'deleted' $deleted.Payload.results[0].action 'owned workspace deleted'

        $realAttempt = Issue-Lease -Kind 'owned-ephemeral-workspace'
        New-TestFile (Join-Path $realAttempt.target '.git\config')
        $null = Seal-Lease $realAttempt
        $realAttempt.target = ConvertTo-NormalizedPath $project
        $refused = Invoke-LeaseCleaner -Lease $realAttempt -Execute
        Assert-True ($refused.ExitCode -ne 0) 'source root refused'
        Assert-True (Test-Path -LiteralPath (Join-Path $project '.project_root')) 'source root preserved'
    }

    Run-Test 'cleanup crash recovery never mutates source-root identity files' {
        Assert-Equal $projectMarkerHashBefore `
            (Get-FileHash -LiteralPath (Join-Path $project '.project_root') -Algorithm SHA256).Hash `
            'source root marker digest'
        Assert-Equal $cargoManifestHashBefore `
            (Get-FileHash -LiteralPath (Join-Path $project 'Cargo.toml') -Algorithm SHA256).Hash `
            'Cargo workspace manifest digest'
    }

    Write-Host "All $passed trusted-cleanup-lease fixture tests passed."
}
finally {
    $normalizedSandbox = ConvertTo-NormalizedPath $sandbox
    $normalizedTemp = ConvertTo-NormalizedPath ([IO.Path]::GetTempPath())
    $leaf = [IO.Path]::GetFileName($normalizedSandbox)
    if ((Test-PathWithin -Path $normalizedSandbox -Boundary $normalizedTemp) -and
        $leaf -match '^adm-newrust-lease-tests-[0-9a-f]{32}$' -and
        (Test-Path -LiteralPath $normalizedSandbox)) {
        Remove-Item -LiteralPath $normalizedSandbox -Recurse -Force -ErrorAction SilentlyContinue
    }
}
