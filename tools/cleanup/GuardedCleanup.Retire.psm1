Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$pathsModule = Join-Path $PSScriptRoot 'GuardedCleanup.psm1'
Import-Module $pathsModule -Force
$leaseModule = Join-Path $PSScriptRoot 'GuardedCleanup.Lease.psm1'
Import-Module $leaseModule -Force
$script:Comparison = [StringComparison]::OrdinalIgnoreCase
$script:ProjectId = 'autodesignmaker-rust-v2'
$script:ReceiptNames = @('owner-manifest.json', 'root-marker.json', 'seal.json')

function Remove-EmptyDirectoryExact {
    param([Parameter(Mandatory = $true)][string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) { return }
    Assert-NoReparsePath -Path $Path
    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        throw "lease retirement expected a directory: $Path"
    }
    if (@(Get-ChildItem -LiteralPath $Path -Force).Count -ne 0) {
        throw "lease retirement refuses a non-empty directory: $Path"
    }
    Remove-Item -LiteralPath $Path -Force
}

function Get-RetirementIdentity {
    param(
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$Kind,
        [Parameter(Mandatory = $true)][string]$OwnerManifest,
        [Parameter(Mandatory = $true)][string]$Nonce
    )

    $root = Assert-SourceProjectRoot $ProjectRoot
    $leaseRoot = Get-ControlledLeaseRoot $root
    $manifestPath = ConvertTo-NormalizedPath $OwnerManifest
    if (-not (Test-PathWithin -Path $manifestPath -Boundary $leaseRoot) -or
        -not ([IO.Path]::GetFileName($manifestPath)).Equals('owner-manifest.json', $script:Comparison)) {
        throw 'lease retirement owner manifest path is outside the controlled layout'
    }
    $leaseDirectory = ConvertTo-NormalizedPath ([IO.Directory]::GetParent($manifestPath).FullName)
    if (-not (ConvertTo-NormalizedPath ([IO.Directory]::GetParent($leaseDirectory).FullName).Equals(
            $leaseRoot, $script:Comparison))) {
        throw 'lease retirement receipt is not a direct child of the controlled lease root'
    }
    $leaseId = [IO.Path]::GetFileName($leaseDirectory)
    if ($leaseId -notmatch '^[0-9a-f]{32}$' -or $Nonce -notmatch '^[0-9a-fA-F]{64}$') {
        throw 'lease retirement identity is invalid'
    }
    $targetPath = ConvertTo-NormalizedPath $Target
    $expectedTargetName = if ($Kind -eq 'owned-ephemeral-user-data') { 'owned-user-data' } else { 'owned-workspace' }
    if (-not ([IO.Path]::GetFileName($targetPath)).Equals($expectedTargetName, $script:Comparison)) {
        throw 'lease retirement target name is invalid for its cleanup kind'
    }
    $boundary = ConvertTo-NormalizedPath ([IO.Directory]::GetParent($targetPath).FullName)
    if (-not ([IO.Path]::GetFileName($boundary)).Equals("adm-newrust-cleanup-$leaseId", $script:Comparison) -or
        (Test-PathsOverlap -Left $boundary -Right $root)) {
        throw 'lease retirement payload boundary identity is invalid'
    }
    $payloadTombstone = Get-OwnedCleanupTombstonePath -Boundary $boundary -LeaseId $leaseId -Nonce $Nonce
    $retirementTombstone = Get-LeaseRetirementTombstonePath -LeaseRoot $leaseRoot `
        -LeaseId $leaseId -Nonce $Nonce
    [pscustomobject]@{
        Root = $root
        LeaseRoot = $leaseRoot
        LeaseId = $leaseId
        LeaseDirectory = $leaseDirectory
        ManifestPath = $manifestPath
        TargetPath = $targetPath
        Boundary = $boundary
        PayloadTombstone = $payloadTombstone
        RetirementTombstone = $retirementTombstone
    }
}

function Assert-RetirementTombstoneContents {
    param(
        [Parameter(Mandatory = $true)]$Identity,
        [Parameter(Mandatory = $true)][string]$Kind,
        [Parameter(Mandatory = $true)][string]$Nonce
    )

    $path = [string]$Identity.RetirementTombstone
    if (-not (Test-Path -LiteralPath $path -PathType Container)) {
        throw 'lease retirement tombstone is missing or not a directory'
    }
    Assert-NoReparsePath -Path $path -InspectDescendants
    $entries = @(Get-ChildItem -LiteralPath $path -Force -ErrorAction Stop)
    foreach ($entry in $entries) {
        if ($entry.PSIsContainer -or $script:ReceiptNames -notcontains $entry.Name) {
            throw "lease retirement tombstone contains an unexpected entry: $($entry.Name)"
        }
    }
    $currentProjectMarkerHash = (Get-FileHash -LiteralPath (Join-Path $Identity.Root '.project_root') `
            -Algorithm SHA256).Hash.ToLowerInvariant()

    $manifest = $null
    $manifestHash = $null
    $manifestPath = Join-Path $path 'owner-manifest.json'
    if (Test-Path -LiteralPath $manifestPath -PathType Leaf) {
        $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ([int]$manifest.schemaVersion -ne 2 -or
            $manifest.projectId -ne $script:ProjectId -or
            $manifest.leaseId -ne $Identity.LeaseId -or
            $manifest.kind -ne $Kind -or
            -not ([string]$manifest.nonce).Equals($Nonce, [StringComparison]::Ordinal) -or
            -not (ConvertTo-NormalizedPath ([string]$manifest.projectRoot)).Equals($Identity.Root, $script:Comparison) -or
            -not (ConvertTo-NormalizedPath ([string]$manifest.sourceRoot)).Equals($Identity.Root, $script:Comparison) -or
            -not (ConvertTo-NormalizedPath ([string]$manifest.targetPath)).Equals($Identity.TargetPath, $script:Comparison) -or
            -not (ConvertTo-NormalizedPath ([string]$manifest.boundaryPath)).Equals($Identity.Boundary, $script:Comparison)) {
            throw 'lease retirement tombstone manifest identity is invalid'
        }
        if (-not ([string]$manifest.projectRootManifestSha256).Equals(
                $currentProjectMarkerHash, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'source project marker changed before lease retirement completed'
        }
        if ($Kind -eq 'owned-ephemeral-user-data') {
            $sourceMeasure = Get-TreeMeasure (ConvertTo-NormalizedPath ([string]$manifest.sourcePath))
            if ([int64]$manifest.sourceSnapshot.fileCount -ne $sourceMeasure.fileCount -or
                [int64]$manifest.sourceSnapshot.bytes -ne $sourceMeasure.bytes -or
                -not ([string]$manifest.sourceSnapshot.digest).Equals(
                    [string]$sourceMeasure.digest, [StringComparison]::OrdinalIgnoreCase)) {
                throw 'protected source data changed before lease retirement completed'
            }
        }
        $manifestHash = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }

    $markerHash = $null
    $markerPath = Join-Path $path 'root-marker.json'
    if (Test-Path -LiteralPath $markerPath -PathType Leaf) {
        $marker = Get-Content -LiteralPath $markerPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ([int]$marker.schemaVersion -ne 2 -or
            $marker.markerKind -ne 'adm-newrust-cleanup-root' -or
            $marker.projectId -ne $script:ProjectId -or
            $marker.leaseId -ne $Identity.LeaseId -or
            $marker.cleanupKind -ne $Kind -or
            -not ([string]$marker.nonce).Equals($Nonce, [StringComparison]::Ordinal) -or
            -not (ConvertTo-NormalizedPath ([string]$marker.projectRoot)).Equals($Identity.Root, $script:Comparison) -or
            -not (ConvertTo-NormalizedPath ([string]$marker.targetPath)).Equals($Identity.TargetPath, $script:Comparison) -or
            -not (ConvertTo-NormalizedPath ([string]$marker.ownerManifest)).Equals($Identity.ManifestPath, $script:Comparison) -or
            -not ([string]$marker.projectRootManifestSha256).Equals(
                $currentProjectMarkerHash, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'lease retirement tombstone root marker identity is invalid'
        }
        $markerHash = (Get-FileHash -LiteralPath $markerPath -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($null -ne $manifest -and
            -not ([string]$manifest.rootMarkerSha256).Equals($markerHash, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'lease retirement tombstone root marker digest is invalid'
        }
    }

    $sealPath = Join-Path $path 'seal.json'
    if (Test-Path -LiteralPath $sealPath -PathType Leaf) {
        $seal = Get-Content -LiteralPath $sealPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ([int]$seal.schemaVersion -ne 2 -or
            $seal.sealKind -ne 'adm-newrust-cleanup-seal' -or
            $seal.projectId -ne $script:ProjectId -or
            $seal.leaseId -ne $Identity.LeaseId -or
            $seal.cleanupKind -ne $Kind -or
            -not ([string]$seal.nonce).Equals($Nonce, [StringComparison]::Ordinal) -or
            -not (ConvertTo-NormalizedPath ([string]$seal.targetPath)).Equals($Identity.TargetPath, $script:Comparison) -or
            ($null -ne $manifestHash -and -not ([string]$seal.manifestSha256).Equals(
                    $manifestHash, [StringComparison]::OrdinalIgnoreCase)) -or
            ($null -ne $markerHash -and -not ([string]$seal.rootMarkerSha256).Equals(
                    $markerHash, [StringComparison]::OrdinalIgnoreCase))) {
            throw 'lease retirement tombstone seal identity is invalid'
        }
    }
}

function Remove-GuardedCleanupLeaseReceipt {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)]
        [ValidateSet('owned-ephemeral-user-data', 'owned-ephemeral-workspace')]
        [string]$Kind,
        [Parameter(Mandatory = $true)][string]$OwnerManifest,
        [Parameter(Mandatory = $true)][string]$Nonce
    )

    $identity = Get-RetirementIdentity -ProjectRoot $ProjectRoot -Target $Target `
        -Kind $Kind -OwnerManifest $OwnerManifest -Nonce $Nonce
    $leaseDirectoryExists = Test-Path -LiteralPath $identity.LeaseDirectory
    $retirementExists = Test-Path -LiteralPath $identity.RetirementTombstone
    if ($leaseDirectoryExists -and $retirementExists) {
        throw 'active receipt and retirement tombstone both exist; ambiguous state refused'
    }
    if ((Test-Path -LiteralPath $identity.TargetPath) -or
        (Test-Path -LiteralPath $identity.PayloadTombstone)) {
        throw 'lease retirement requires the owned target and payload tombstone to be deleted first'
    }

    if ($leaseDirectoryExists) {
        if (-not (Test-Path -LiteralPath $identity.ManifestPath -PathType Leaf)) {
            throw 'active lease receipt lost its owner manifest before retirement transition'
        }
        $lease = Get-TrustedCleanupLease -ManifestPath $identity.ManifestPath `
            -Target $identity.TargetPath -Kind $Kind -Nonce $Nonce -ProjectRoot $identity.Root
        if ($lease.TargetExists) {
            throw 'lease retirement requires the owned target to be deleted first'
        }
        Assert-NoReparsePath -Path $identity.LeaseDirectory -InspectDescendants
        $actual = @(Get-ChildItem -LiteralPath $identity.LeaseDirectory -Force |
                ForEach-Object Name | Sort-Object)
        if (($actual -join '|') -ne (($script:ReceiptNames | Sort-Object) -join '|')) {
            throw "lease receipt contains unexpected entries: $($actual -join ', ')"
        }
        if (-not (ConvertTo-NormalizedPath ([string]$lease.Manifest.boundaryPath)).Equals(
                $identity.Boundary, $script:Comparison)) {
            throw 'lease receipt payload boundary does not match the retirement request'
        }
        Remove-EmptyDirectoryExact -Path $identity.Boundary
        [IO.Directory]::Move($identity.LeaseDirectory, $identity.RetirementTombstone)
        if ((Test-Path -LiteralPath $identity.LeaseDirectory) -or
            -not (Test-Path -LiteralPath $identity.RetirementTombstone -PathType Container)) {
            throw 'lease receipt atomic retirement rename did not reach the expected state'
        }
        Invoke-CleanupFailpoint -Name 'after-receipt-tombstone-rename'
        $retirementExists = $true
    }

    if ($retirementExists) {
        Assert-RetirementTombstoneContents -Identity $identity -Kind $Kind -Nonce $Nonce
        Remove-EmptyDirectoryExact -Path $identity.Boundary
        Remove-Item -LiteralPath $identity.RetirementTombstone -Recurse -Force -ErrorAction Stop
        if (Test-Path -LiteralPath $identity.RetirementTombstone) {
            throw 'lease retirement tombstone still exists after deletion'
        }
    }
    else {
        # A fully absent receipt/tombstone is the terminal idempotent state. The
        # deterministic controlled path, nonce-shaped name, absent payload, and
        # source-root checks above prove that this no-op cannot delete source data.
        Remove-EmptyDirectoryExact -Path $identity.Boundary
    }

    if ((Test-Path -LiteralPath $identity.LeaseRoot -PathType Container) -and
        (@(Get-ChildItem -LiteralPath $identity.LeaseRoot -Force)).Count -eq 0) {
        Remove-EmptyDirectoryExact -Path $identity.LeaseRoot
    }
    $projectTemp = ConvertTo-NormalizedPath (Join-Path $identity.Root '.tmp')
    if ((Test-Path -LiteralPath $projectTemp -PathType Container) -and
        (@(Get-ChildItem -LiteralPath $projectTemp -Force)).Count -eq 0) {
        Remove-EmptyDirectoryExact -Path $projectTemp
    }
    [pscustomobject]@{
        schemaVersion = 2
        operation = 'retired'
        projectId = $script:ProjectId
        leaseId = $identity.LeaseId
        target = $identity.TargetPath
        receiptRemoved = (-not (Test-Path -LiteralPath $identity.LeaseDirectory) -and
            -not (Test-Path -LiteralPath $identity.RetirementTombstone))
        boundaryRemoved = (-not (Test-Path -LiteralPath $identity.Boundary))
    }
}

Export-ModuleMember -Function Remove-GuardedCleanupLeaseReceipt
