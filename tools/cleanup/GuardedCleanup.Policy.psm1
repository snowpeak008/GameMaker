Set-StrictMode -Version Latest

$pathsModule = Join-Path $PSScriptRoot 'GuardedCleanup.psm1'
Import-Module $pathsModule -Force
$activityModule = Join-Path $PSScriptRoot 'GuardedCleanup.Activity.psm1'
Import-Module $activityModule -Force
$leaseModule = Join-Path $PSScriptRoot 'GuardedCleanup.Lease.psm1'
Import-Module $leaseModule -Force
$script:Comparison = [StringComparison]::OrdinalIgnoreCase

function New-CleanupResult {
    param([string]$Target, [string]$Kind, [string]$Action, [string]$Reason, $Measure)
    [pscustomobject]@{
        target = $Target
        kind = $Kind
        action = $Action
        reason = $Reason
        fileCount = if ($null -eq $Measure) { 0 } else { [int64]$Measure.fileCount }
        bytes = if ($null -eq $Measure) { 0 } else { [int64]$Measure.bytes }
    }
}

function Get-ProtectedProjectPaths {
    param([Parameter(Mandatory = $true)][string]$ProjectRoot)
    @('.git', '.cargo', 'apps', 'crates', 'docs', 'knowledge', 'pipeline', 'testdata', 'tools', 'web\src') |
        ForEach-Object { ConvertTo-NormalizedPath (Join-Path $ProjectRoot $_) }
}

function Find-NamedEntry {
    param([Parameter(Mandatory = $true)][string]$Root, [Parameter(Mandatory = $true)][string]$Name)

    if (-not (Test-Path -LiteralPath $Root -PathType Container)) { return @() }
    $matches = New-Object System.Collections.Generic.List[string]
    $pending = New-Object System.Collections.Generic.Stack[string]
    $pending.Push((ConvertTo-NormalizedPath $Root))
    while ($pending.Count -gt 0) {
        $directory = $pending.Pop()
        foreach ($entry in @(Get-ChildItem -LiteralPath $directory -Force -ErrorAction Stop)) {
            if (($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "reparse point refused: $($entry.FullName)"
            }
            if ($entry.Name.Equals($Name, $script:Comparison)) {
                $matches.Add($entry.FullName)
            }
            elseif ($entry.PSIsContainer) {
                $pending.Push($entry.FullName)
            }
        }
    }
    return $matches.ToArray()
}

function Assert-TargetDoesNotOverlap {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string[]]$ProtectedPaths,
        [Parameter(Mandatory = $true)][string]$Label
    )
    foreach ($protected in $ProtectedPaths) {
        if (Test-PathsOverlap -Left $Target -Right $protected) {
            throw "$Label overlap refused: $protected"
        }
    }
}

function Assert-VerifiedPortableStage {
    param([Parameter(Mandatory = $true)][string]$StageRoot)

    $manifestPath = Join-Path $StageRoot '.adm-cleanup-stage.json'
    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
        throw "portable stage is unverified; missing .adm-cleanup-stage.json"
    }
    Assert-NoReparsePath -Path $manifestPath
    $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $stage = ConvertTo-NormalizedPath $StageRoot
    if ([int]$manifest.schemaVersion -ne 1 -or $manifest.kind -ne 'verified-portable-stage' -or
        $manifest.verified -ne $true -or
        -not (ConvertTo-NormalizedPath ([string]$manifest.targetPath)).Equals($stage, $script:Comparison)) {
        throw "portable stage verification manifest is invalid"
    }

    $emptyDigest = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
    if ([int64]$manifest.userData.fileCount -ne 0 -or [int64]$manifest.userData.bytes -ne 0 -or
        -not ([string]$manifest.userData.digest).Equals($emptyDigest, [StringComparison]::OrdinalIgnoreCase)) {
        throw "portable stage manifest does not prove empty user_data"
    }

    $userData = Join-Path $stage 'user_data'
    if (Test-Path -LiteralPath $userData) {
        $measure = Get-TreeMeasure $userData
        if ($measure.fileCount -ne 0 -or $measure.bytes -ne 0 -or
            [string]$manifest.userData.digest -ne [string]$measure.digest) {
            throw "portable stage contains user_data; ordinary cleanup refused"
        }
    }
}

function Get-StageRootFromTarget {
    param([Parameter(Mandatory = $true)][string]$Target)
    $current = Get-Item -LiteralPath $Target -Force
    if (-not $current.PSIsContainer) { $current = $current.Directory }
    while ($null -ne $current) {
        if ($current.Name -match '^\..+\.stage-[^\\/]+$') { return $current.FullName }
        $current = $current.Parent
    }
    return $null
}

function Test-GeneratedTarget {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [string[]]$ProtectedUserData,
        [Parameter(Mandatory = $true)][string]$IntrinsicProjectRoot
    )

    $path = ConvertTo-NormalizedPath $Target -BasePath $ProjectRoot
    $kind = Get-GeneratedKind -Target $path -ProjectRoot $ProjectRoot
    if ($kind -eq 'portable-backup') {
        Assert-NoReparsePath -Path $path
        return New-CleanupResult $path $kind 'report-only' 'portable backup is owned by Finalize-PortableSwap and is never deleted here' $null
    }
    if ($kind -eq 'protected-release-evidence') {
        Assert-NoReparsePath -Path $path
        return New-CleanupResult $path $kind 'report-only' 'standalone release evidence is retained by ordinary cleanup' (Get-ItemMeasure $path)
    }
    if (-not $kind) { throw "target is outside the generated-data allowlist" }
    if (-not (Test-PathWithin -Path $path -Boundary $ProjectRoot)) {
        throw "generated target escaped the source project boundary"
    }
    if ($path.Equals((ConvertTo-NormalizedPath $ProjectRoot), $script:Comparison) -or
        (Test-PathWithin -Path $ProjectRoot -Boundary $path -AllowEqual)) {
        throw "project root or its ancestor cannot be cleaned"
    }
    if (Test-PathsOverlap -Left $path -Right $IntrinsicProjectRoot) {
        $validatedRootIsIntrinsic = (ConvertTo-NormalizedPath $ProjectRoot).Equals(
            (ConvertTo-NormalizedPath $IntrinsicProjectRoot), $script:Comparison)
        if (-not $validatedRootIsIntrinsic -or
            -not (Test-PathWithin -Path $path -Boundary $IntrinsicProjectRoot)) {
            throw "real source project root or its ancestor cannot be cleaned"
        }
    }

    Assert-TargetDoesNotOverlap -Target $path -ProtectedPaths (Get-ProtectedProjectPaths $ProjectRoot) -Label 'source/resource'
    if ($ProtectedUserData.Count -gt 0) {
        Assert-TargetDoesNotOverlap -Target $path -ProtectedPaths $ProtectedUserData -Label 'protected-user-data'
    }
    if (-not (Test-Path -LiteralPath $path)) {
        return New-CleanupResult $path $kind 'skipped' 'target does not exist (idempotent no-op)' $null
    }
    Assert-NoReparsePath -Path $path -InspectDescendants
    if ($kind -eq 'empty-project-temp-root' -and
        @(Get-ChildItem -LiteralPath $path -Force -ErrorAction Stop).Count -ne 0) {
        throw 'project .tmp root is not empty; only its allowlisted children may be cleaned'
    }
    Assert-GeneratedTargetInactive -Target $path -ProjectRoot $ProjectRoot -Kind $kind

    $gitPaths = @(Find-NamedEntry -Root $path -Name '.git')
    if ($gitPaths.Count -gt 0) { throw "ordinary cleanup cannot remove .git: $($gitPaths[0])" }
    if ($kind -eq 'portable-stage') {
        $stageRoot = Get-StageRootFromTarget $path
        Assert-VerifiedPortableStage $stageRoot
    }
    else {
        $dataPaths = @(Find-NamedEntry -Root $path -Name 'user_data')
        if ($dataPaths.Count -gt 0) { throw "ordinary cleanup cannot remove user_data or its ancestor: $($dataPaths[0])" }
    }
    return New-CleanupResult $path $kind 'dry-run-delete' 'allowlisted generated target passed all guards' (Get-ItemMeasure $path)
}

function Test-OwnedEphemeralTarget {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][string]$IntrinsicProjectRoot,
        [Parameter(Mandatory = $true)][string]$Kind,
        [Parameter(Mandatory = $true)][string]$OwnerManifest,
        [Parameter(Mandatory = $true)][string]$Nonce,
        [string[]]$ProtectedUserData
    )

    $path = ConvertTo-NormalizedPath $Target
    if (Test-PathsOverlap -Left $path -Right $IntrinsicProjectRoot) {
        throw "real source project root or its ancestor cannot be finalized"
    }
    if ($ProtectedUserData.Count -gt 0) {
        Assert-TargetDoesNotOverlap -Target $path -ProtectedPaths $ProtectedUserData -Label 'protected-user-data'
    }
    $lease = Get-TrustedCleanupLease -ManifestPath $OwnerManifest -Target $path -Kind $Kind -Nonce $Nonce -ProjectRoot $ProjectRoot
    $boundary = ConvertTo-NormalizedPath ([string]$lease.Manifest.boundaryPath)
    $tombstone = Get-OwnedCleanupTombstonePath -Boundary $boundary `
        -LeaseId ([string]$lease.Manifest.leaseId) -Nonce $Nonce
    $targetExists = [bool]$lease.TargetExists
    $tombstoneExists = Test-Path -LiteralPath $tombstone
    if ($targetExists -and $tombstoneExists) {
        throw 'owned target and its cleanup tombstone both exist; ambiguous state refused'
    }
    if (-not $targetExists -and -not $tombstoneExists) {
        return New-CleanupResult $path $Kind 'skipped' 'owned target does not exist (idempotent no-op)' $null
    }
    if ($tombstoneExists -and -not (Test-Path -LiteralPath $tombstone -PathType Container)) {
        throw 'owned cleanup tombstone exists but is not a directory'
    }
    $payload = if ($targetExists) { $path } else { $tombstone }
    if ($ProtectedUserData.Count -gt 0) {
        Assert-TargetDoesNotOverlap -Target $payload -ProtectedPaths $ProtectedUserData -Label 'protected-user-data'
    }
    Assert-NoReparsePath -Path $payload -InspectDescendants

    $targetMarker = Join-Path $payload '.adm-cleanup-root.json'
    if (Test-Path -LiteralPath $targetMarker -PathType Leaf) {
        if (-not (Get-FileHash -LiteralPath $targetMarker -Algorithm SHA256).Hash.Equals(
                [string]$lease.MarkerSha256, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'owned payload root marker is inconsistent'
        }
    }
    elseif ($targetExists) {
        throw 'owned target root marker is missing'
    }

    $gitPaths = @(Find-NamedEntry -Root $payload -Name '.git')
    if ($Kind -eq 'owned-ephemeral-user-data' -and $gitPaths.Count -gt 0) {
        throw "owned ephemeral user data cannot contain .git"
    }
    if ($Kind -eq 'owned-ephemeral-workspace') {
        $expectedGit = ConvertTo-NormalizedPath (Join-Path $payload '.git')
        foreach ($gitPath in $gitPaths) {
            if (-not (ConvertTo-NormalizedPath $gitPath).Equals($expectedGit, $script:Comparison)) {
                throw "nested .git outside the owned workspace root is refused: $gitPath"
            }
        }
    }
    if ($targetExists) {
        return New-CleanupResult $path $Kind 'dry-run-delete' 'issuer proof passed; execute will atomically rename the owned target before deletion' (Get-ItemMeasure $payload)
    }
    return New-CleanupResult $path $Kind 'dry-run-resume-delete' 'validated lease-bound tombstone found; interrupted deletion can resume' (Get-ItemMeasure $payload)
}

function Remove-OwnedEphemeralTargetTransactional {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][string]$Kind,
        [Parameter(Mandatory = $true)][string]$OwnerManifest,
        [Parameter(Mandatory = $true)][string]$Nonce
    )

    $path = ConvertTo-NormalizedPath $Target
    $lease = Get-TrustedCleanupLease -ManifestPath $OwnerManifest -Target $path `
        -Kind $Kind -Nonce $Nonce -ProjectRoot $ProjectRoot
    $boundary = ConvertTo-NormalizedPath ([string]$lease.Manifest.boundaryPath)
    $tombstone = Get-OwnedCleanupTombstonePath -Boundary $boundary `
        -LeaseId ([string]$lease.Manifest.leaseId) -Nonce $Nonce
    $targetExists = Test-Path -LiteralPath $path -PathType Container
    $tombstoneExists = Test-Path -LiteralPath $tombstone
    if ($targetExists -and $tombstoneExists) {
        throw 'owned target and its cleanup tombstone both exist during execution'
    }
    if ($targetExists) {
        $targetParent = ConvertTo-NormalizedPath ([IO.Directory]::GetParent($path).FullName)
        $tombstoneParent = ConvertTo-NormalizedPath ([IO.Directory]::GetParent($tombstone).FullName)
        if (-not $targetParent.Equals($boundary, $script:Comparison) -or
            -not $tombstoneParent.Equals($boundary, $script:Comparison)) {
            throw 'owned cleanup rename escaped its same-parent boundary'
        }
        [IO.Directory]::Move($path, $tombstone)
        if ((Test-Path -LiteralPath $path) -or
            -not (Test-Path -LiteralPath $tombstone -PathType Container)) {
            throw 'owned cleanup atomic rename did not reach the expected tombstone state'
        }
        Invoke-CleanupFailpoint -Name 'after-owned-target-rename'
    }
    elseif (-not $tombstoneExists) {
        return $false
    }

    Assert-NoReparsePath -Path $tombstone -InspectDescendants
    if ([string]$env:AUTODESIGNMAKER_CLEANUP_FAILPOINT -eq 'during-owned-tombstone-delete') {
        $victim = Get-ChildItem -LiteralPath $tombstone -File -Recurse -Force -ErrorAction Stop |
            Where-Object { -not $_.Name.Equals('.adm-cleanup-root.json', $script:Comparison) } |
            Select-Object -First 1
        if ($null -ne $victim) {
            Remove-Item -LiteralPath $victim.FullName -Force -ErrorAction Stop
        }
        $movedMarker = Join-Path $tombstone '.adm-cleanup-root.json'
        if (Test-Path -LiteralPath $movedMarker -PathType Leaf) {
            Remove-Item -LiteralPath $movedMarker -Force -ErrorAction Stop
        }
        Invoke-CleanupFailpoint -Name 'during-owned-tombstone-delete'
    }
    Remove-Item -LiteralPath $tombstone -Recurse -Force -ErrorAction Stop
    if (Test-Path -LiteralPath $tombstone) {
        throw 'owned cleanup tombstone still exists after recursive deletion'
    }
    return $true
}

function Invoke-GuardedCleanup {
    [CmdletBinding()]
    param(
        [string[]]$Target,
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][string]$IntrinsicProjectRoot,
        [ValidateSet('generated', 'owned-ephemeral-user-data', 'owned-ephemeral-workspace')]
        [string]$Kind = 'generated',
        [string[]]$ProtectedUserData = @(),
        [string]$OwnerManifest,
        [string]$Nonce,
        [switch]$Execute,
        [switch]$ScanPortableStaging
    )

    $root = Assert-SourceProjectRoot $ProjectRoot
    $intrinsic = ConvertTo-NormalizedPath $IntrinsicProjectRoot
    $protected = @($ProtectedUserData | ForEach-Object { ConvertTo-NormalizedPath $_ })
    if ($Kind -eq 'generated' -and (!$Target -or $Target.Count -eq 0)) {
        $Target = @(Get-DefaultGeneratedTargets -ProjectRoot $root -ScanPortableStaging:$ScanPortableStaging)
    }
    if ($Kind -ne 'generated' -and (!$Target -or $Target.Count -ne 1)) {
        throw "owned cleanup requires exactly one target"
    }

    $results = New-Object System.Collections.Generic.List[object]
    foreach ($candidate in @($Target | Sort-Object -Unique)) {
        try {
            if ($Kind -eq 'generated') {
                $results.Add((Test-GeneratedTarget -Target $candidate -ProjectRoot $root -ProtectedUserData $protected -IntrinsicProjectRoot $intrinsic))
            }
            else {
                $results.Add((Test-OwnedEphemeralTarget -Target $candidate -ProjectRoot $root -IntrinsicProjectRoot $intrinsic -Kind $Kind -OwnerManifest $OwnerManifest -Nonce $Nonce -ProtectedUserData $protected))
            }
        }
        catch {
            $results.Add((New-CleanupResult (ConvertTo-NormalizedPath $candidate -BasePath $root) $Kind 'refused' $_.Exception.Message $null))
        }
    }

    $hasRefusal = @($results | Where-Object { $_.action -eq 'refused' }).Count -gt 0
    if ($Execute -and -not $hasRefusal) {
        foreach ($result in @($results | Where-Object { $_.action -in @('dry-run-delete', 'dry-run-resume-delete') })) {
            try {
                # Re-evaluate immediately before the only destructive operation.
                if ($Kind -eq 'generated') {
                    $recheck = Test-GeneratedTarget -Target $result.target -ProjectRoot $root -ProtectedUserData $protected -IntrinsicProjectRoot $intrinsic
                }
                else {
                    $recheck = Test-OwnedEphemeralTarget -Target $result.target -ProjectRoot $root -IntrinsicProjectRoot $intrinsic -Kind $Kind -OwnerManifest $OwnerManifest -Nonce $Nonce -ProtectedUserData $protected
                }
                if ($recheck.action -notin @('dry-run-delete', 'dry-run-resume-delete')) {
                    throw "target was no longer deletable during final recheck"
                }
                if ($Kind -eq 'generated') {
                    Remove-Item -LiteralPath $result.target -Recurse -Force -ErrorAction Stop
                }
                else {
                    $removed = Remove-OwnedEphemeralTargetTransactional -Target $result.target `
                        -ProjectRoot $root -Kind $Kind -OwnerManifest $OwnerManifest -Nonce $Nonce
                    if (-not $removed) { throw 'owned target disappeared before tombstone transition' }
                }
                $result.action = 'deleted'
                $result.reason = if ($Kind -eq 'generated') {
                    'explicit -Execute completed after final guard recheck'
                }
                else {
                    'explicit -Execute completed the lease-bound tombstone deletion state machine'
                }
            }
            catch {
                $result.action = 'refused'
                $result.reason = "execution refused: $($_.Exception.Message)"
                break
            }
        }
    }
    elseif ($Execute -and $hasRefusal) {
        foreach ($result in @($results | Where-Object { $_.action -in @('dry-run-delete', 'dry-run-resume-delete') })) {
            $result.action = 'blocked-by-plan'
            $result.reason = 'nothing was deleted because another target was refused'
        }
    }
    return $results.ToArray()
}

Export-ModuleMember -Function Invoke-GuardedCleanup
