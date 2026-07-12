Set-StrictMode -Version Latest
$pathsModule = Join-Path $PSScriptRoot 'GuardedCleanup.psm1'
Import-Module $pathsModule -Force
$script:Comparison = [StringComparison]::OrdinalIgnoreCase
$script:ProjectId = 'autodesignmaker-rust-v2'
$script:LeaseSchema = 2

function ConvertTo-Utf8JsonBytes {
    param([Parameter(Mandatory = $true)]$Value)
    $json = $Value | ConvertTo-Json -Depth 12 -Compress
    return [Text.Encoding]::UTF8.GetBytes($json)
}
function Write-BytesCreateNew {
    param([Parameter(Mandatory = $true)][string]$Path, [Parameter(Mandatory = $true)][byte[]]$Bytes)
    $stream = [IO.File]::Open($Path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try {
        $stream.Write($Bytes, 0, $Bytes.Length)
        $stream.Flush($true)
    }
    finally {
        $stream.Dispose()
    }
}
function Get-BytesSha256 {
    param([Parameter(Mandatory = $true)][byte[]]$Bytes)
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}
function New-CryptoNonce {
    $bytes = New-Object byte[] 32
    $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
    try { $rng.GetBytes($bytes) }
    finally { $rng.Dispose() }
    return ([BitConverter]::ToString($bytes)).Replace('-', '').ToLowerInvariant()
}
function ConvertTo-LeaseTime {
    param([Parameter(Mandatory = $true)][string]$Value, [Parameter(Mandatory = $true)][string]$Field)
    try {
        return [DateTimeOffset]::ParseExact(
            $Value,
            'o',
            [Globalization.CultureInfo]::InvariantCulture,
            [Globalization.DateTimeStyles]::RoundtripKind)
    }
    catch { throw "invalid lease $Field timestamp" }
}
function New-RootMarkerValue {
    param(
        [string]$LeaseId,
        [string]$Kind,
        [string]$Nonce,
        [string]$IssuedAtUtc,
        [string]$ExpiresAtUtc,
        [string]$ProjectRoot,
        [string]$TargetPath,
        [string]$ManifestPath,
        [string]$ProjectRootManifestSha256
    )
    [ordered]@{
        schemaVersion = $script:LeaseSchema
        markerKind = 'adm-newrust-cleanup-root'
        projectId = $script:ProjectId
        leaseId = $LeaseId
        cleanupKind = $Kind
        nonce = $Nonce
        issuedAtUtc = $IssuedAtUtc
        expiresAtUtc = $ExpiresAtUtc
        projectRoot = $ProjectRoot
        targetPath = $TargetPath
        ownerManifest = $ManifestPath
        projectRootManifestSha256 = $ProjectRootManifestSha256
    }
}
function Assert-TimeNearIssue {
    param([datetime]$CreationTimeUtc, [DateTimeOffset]$IssuedAt, [string]$Label)
    $difference = [Math]::Abs(($CreationTimeUtc - $IssuedAt.UtcDateTime).TotalMinutes)
    if ($difference -gt 5) { throw "$Label creation time does not match lease issue time" }
}
function Read-ControlledLeaseCore {
    param(
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$Kind,
        [Parameter(Mandatory = $true)][string]$Nonce,
        [Parameter(Mandatory = $true)][string]$ProjectRoot
    )

    $root = Assert-SourceProjectRoot $ProjectRoot
    $manifestFile = ConvertTo-NormalizedPath $ManifestPath
    $targetPath = ConvertTo-NormalizedPath $Target
    $leaseRoot = Get-ControlledLeaseRoot $root
    if (-not (Test-PathWithin -Path $manifestFile -Boundary $leaseRoot)) {
        throw "owner manifest is outside the project-controlled lease root"
    }
    if (-not (Test-Path -LiteralPath $manifestFile -PathType Leaf)) { throw "owner manifest missing" }
    Assert-NoReparsePath -Path $manifestFile
    $leaseDir = ConvertTo-NormalizedPath ([IO.Directory]::GetParent($manifestFile).FullName)
    if (-not (ConvertTo-NormalizedPath ([IO.Directory]::GetParent($leaseDir).FullName)).Equals($leaseRoot, $script:Comparison) -or
        -not ([IO.Path]::GetFileName($manifestFile)).Equals('owner-manifest.json', $script:Comparison)) {
        throw "owner manifest does not use the controlled lease layout"
    }

    $manifest = Get-Content -LiteralPath $manifestFile -Raw -Encoding UTF8 | ConvertFrom-Json
    $leaseId = [string]$manifest.leaseId
    $expectedTargetName = if ($Kind -eq 'owned-ephemeral-user-data') { 'owned-user-data' } else { 'owned-workspace' }
    $boundary = ConvertTo-NormalizedPath ([string]$manifest.boundaryPath)
    $expectedBoundaryName = "adm-newrust-cleanup-$leaseId"
    if ([int]$manifest.schemaVersion -ne $script:LeaseSchema -or
        $manifest.issuer -ne 'new-cleanup-lease.ps1/v1' -or
        $manifest.projectId -ne $script:ProjectId -or
        $leaseId -notmatch '^[0-9a-f]{32}$' -or
        -not ([IO.Path]::GetFileName($leaseDir)).Equals($leaseId, $script:Comparison) -or
        $manifest.kind -ne $Kind -or
        $manifest.createdRootWasAbsent -ne $true -or
        $Nonce -notmatch '^[0-9a-fA-F]{64}$' -or
        -not ([string]$manifest.nonce).Equals($Nonce, [StringComparison]::Ordinal) -or
        -not (ConvertTo-NormalizedPath ([string]$manifest.projectRoot)).Equals($root, $script:Comparison) -or
        -not (ConvertTo-NormalizedPath ([string]$manifest.sourceRoot)).Equals($root, $script:Comparison) -or
        -not (ConvertTo-NormalizedPath ([string]$manifest.targetPath)).Equals($targetPath, $script:Comparison) -or
        -not ([IO.Path]::GetFileName($targetPath)).Equals($expectedTargetName, $script:Comparison) -or
        -not ([IO.Path]::GetFileName($boundary)).Equals($expectedBoundaryName, $script:Comparison) -or
        -not (ConvertTo-NormalizedPath ([IO.Directory]::GetParent($targetPath).FullName).Equals($boundary, $script:Comparison))) {
        throw "controlled lease identity proof is invalid"
    }
    if (Test-PathsOverlap -Left $targetPath -Right $root) { throw "lease target overlaps the source project" }
    Assert-NoReparsePath -Path $leaseDir -InspectDescendants
    Assert-NoReparsePath -Path $boundary

    $issuedAt = ConvertTo-LeaseTime ([string]$manifest.issuedAtUtc) 'issuedAtUtc'
    $expiresAt = ConvertTo-LeaseTime ([string]$manifest.expiresAtUtc) 'expiresAtUtc'
    $now = [DateTimeOffset]::UtcNow
    $targetExistsAsContainer = Test-Path -LiteralPath $targetPath -PathType Container
    if ((Test-Path -LiteralPath $targetPath) -and -not $targetExistsAsContainer) {
        throw "lease target exists but is not a directory"
    }
    if ($expiresAt -le $issuedAt -or $issuedAt -gt $now.AddMinutes(5) -or
        ($now -gt $expiresAt -and $targetExistsAsContainer)) {
        throw "lease time window is invalid or expired"
    }
    Assert-TimeNearIssue (Get-Item -LiteralPath $manifestFile -Force).CreationTimeUtc $issuedAt 'manifest'
    Assert-TimeNearIssue (Get-Item -LiteralPath $leaseDir -Force).CreationTimeUtc $issuedAt 'lease directory'

    $projectMarker = Join-Path $root '.project_root'
    $projectMarkerHash = (Get-FileHash -LiteralPath $projectMarker -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not ([string]$manifest.projectRootManifestSha256).Equals($projectMarkerHash, [StringComparison]::OrdinalIgnoreCase)) {
        throw "source project root marker changed after lease issue"
    }
    $expectedMarker = New-RootMarkerValue -LeaseId $leaseId -Kind $Kind -Nonce $Nonce `
        -IssuedAtUtc ([string]$manifest.issuedAtUtc) -ExpiresAtUtc ([string]$manifest.expiresAtUtc) `
        -ProjectRoot $root -TargetPath $targetPath -ManifestPath $manifestFile `
        -ProjectRootManifestSha256 $projectMarkerHash
    $markerBytes = ConvertTo-Utf8JsonBytes $expectedMarker
    $markerHash = Get-BytesSha256 $markerBytes
    if (-not ([string]$manifest.rootMarkerSha256).Equals($markerHash, [StringComparison]::OrdinalIgnoreCase)) {
        throw "lease root marker digest is invalid"
    }
    $externalMarker = Join-Path $leaseDir 'root-marker.json'
    if (-not (Test-Path -LiteralPath $externalMarker -PathType Leaf) -or
        -not (Get-FileHash -LiteralPath $externalMarker -Algorithm SHA256).Hash.Equals($markerHash, [StringComparison]::OrdinalIgnoreCase)) {
        throw "external lease root marker is missing or inconsistent"
    }

    if ($Kind -eq 'owned-ephemeral-user-data') {
        $sourcePath = ConvertTo-NormalizedPath ([string]$manifest.sourcePath)
        if (Test-PathsOverlap -Left $targetPath -Right $sourcePath) { throw "lease target overlaps protected source data" }
        $sourceMeasure = Get-TreeMeasure $sourcePath
        if ([int64]$manifest.sourceSnapshot.fileCount -ne $sourceMeasure.fileCount -or
            [int64]$manifest.sourceSnapshot.bytes -ne $sourceMeasure.bytes -or
            -not ([string]$manifest.sourceSnapshot.digest).Equals([string]$sourceMeasure.digest, [StringComparison]::OrdinalIgnoreCase)) {
            throw "protected source data digest changed; cleanup refused"
        }
    }
    [pscustomobject]@{
        Manifest = $manifest
        ManifestPath = $manifestFile
        ManifestSha256 = (Get-FileHash -LiteralPath $manifestFile -Algorithm SHA256).Hash.ToLowerInvariant()
        LeaseDirectory = $leaseDir
        TargetPath = $targetPath
        MarkerBytes = $markerBytes
        MarkerSha256 = $markerHash
        IssuedAt = $issuedAt
        ExpiresAt = $expiresAt
    }
}
function New-GuardedCleanupLease {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][ValidateSet('owned-ephemeral-user-data', 'owned-ephemeral-workspace')][string]$Kind,
        [Parameter(Mandatory = $true)][string]$TempParent,
        [string]$SourcePath,
        [ValidateRange(5, 10080)][int]$ValidForMinutes = 1440
    )

    $root = Assert-SourceProjectRoot $ProjectRoot
    $temp = ConvertTo-NormalizedPath $TempParent
    if (-not (Test-Path -LiteralPath $temp -PathType Container)) { throw "temporary parent does not exist" }
    Assert-NoReparsePath -Path $temp
    if (Test-PathsOverlap -Left $temp -Right $root) { throw "temporary parent must not overlap the source project" }
    if ($Kind -eq 'owned-ephemeral-user-data' -and [string]::IsNullOrWhiteSpace($SourcePath)) {
        throw "owned ephemeral user data requires SourcePath"
    }
    $source = $null
    $sourceMeasure = $null
    if ($Kind -eq 'owned-ephemeral-user-data') {
        $source = ConvertTo-NormalizedPath $SourcePath
        $sourceMeasure = Get-TreeMeasure $source
    }

    $leaseId = [guid]::NewGuid().ToString('N')
    $nonce = New-CryptoNonce
    $issuedAt = [DateTimeOffset]::UtcNow
    $expiresAt = $issuedAt.AddMinutes($ValidForMinutes)
    $issuedText = $issuedAt.ToString('o')
    $expiresText = $expiresAt.ToString('o')
    $boundary = Join-Path $temp "adm-newrust-cleanup-$leaseId"
    $targetName = if ($Kind -eq 'owned-ephemeral-user-data') { 'owned-user-data' } else { 'owned-workspace' }
    $target = Join-Path $boundary $targetName
    $leaseRoot = Get-ControlledLeaseRoot $root
    $leaseDir = Join-Path $leaseRoot $leaseId
    $manifestPath = Join-Path $leaseDir 'owner-manifest.json'
    if ($null -ne $source -and (Test-PathsOverlap -Left $target -Right $source)) {
        throw "lease target overlaps protected source data"
    }
    if ((Test-Path -LiteralPath $boundary) -or (Test-Path -LiteralPath $leaseDir)) {
        throw "random cleanup lease boundary collision"
    }

    $projectTempRoot = ConvertTo-NormalizedPath (Join-Path $root '.tmp')
    if (Test-Path -LiteralPath $projectTempRoot) {
        Assert-NoReparsePath -Path $projectTempRoot
    }
    else {
        New-Item -ItemType Directory -Path $projectTempRoot | Out-Null
    }
    if (Test-Path -LiteralPath $leaseRoot) {
        Assert-NoReparsePath -Path $leaseRoot
    }
    else {
        New-Item -ItemType Directory -Path $leaseRoot | Out-Null
    }
    Assert-NoReparsePath -Path $leaseRoot
    New-Item -ItemType Directory -Path $leaseDir | Out-Null
    New-Item -ItemType Directory -Path $boundary | Out-Null
    New-Item -ItemType Directory -Path $target | Out-Null

    $projectHash = (Get-FileHash -LiteralPath (Join-Path $root '.project_root') -Algorithm SHA256).Hash.ToLowerInvariant()
    $marker = New-RootMarkerValue -LeaseId $leaseId -Kind $Kind -Nonce $nonce `
        -IssuedAtUtc $issuedText -ExpiresAtUtc $expiresText -ProjectRoot $root `
        -TargetPath (ConvertTo-NormalizedPath $target) -ManifestPath (ConvertTo-NormalizedPath $manifestPath) `
        -ProjectRootManifestSha256 $projectHash
    $markerBytes = ConvertTo-Utf8JsonBytes $marker
    $markerHash = Get-BytesSha256 $markerBytes
    Write-BytesCreateNew -Path (Join-Path $leaseDir 'root-marker.json') -Bytes $markerBytes

    $manifest = [ordered]@{
        schemaVersion = $script:LeaseSchema
        issuer = 'new-cleanup-lease.ps1/v1'
        projectId = $script:ProjectId
        leaseId = $leaseId
        kind = $Kind
        nonce = $nonce
        issuedAtUtc = $issuedText
        expiresAtUtc = $expiresText
        createdRootWasAbsent = $true
        projectRoot = $root
        sourceRoot = $root
        projectRootManifestSha256 = $projectHash
        boundaryPath = (ConvertTo-NormalizedPath $boundary)
        targetPath = (ConvertTo-NormalizedPath $target)
        rootMarkerSha256 = $markerHash
    }
    if ($Kind -eq 'owned-ephemeral-user-data') {
        $manifest.sourcePath = $source
        $manifest.sourceSnapshot = $sourceMeasure
    }
    Write-BytesCreateNew -Path $manifestPath -Bytes (ConvertTo-Utf8JsonBytes $manifest)
    [pscustomobject]@{
        schemaVersion = $script:LeaseSchema
        operation = 'issued'
        projectId = $script:ProjectId
        leaseId = $leaseId
        kind = $Kind
        nonce = $nonce
        issuedAtUtc = $issuedText
        expiresAtUtc = $expiresText
        target = (ConvertTo-NormalizedPath $target)
        ownerManifest = (ConvertTo-NormalizedPath $manifestPath)
        nextStep = 'populate the empty target, then run new-cleanup-lease.ps1 -Operation Seal'
    }
}
function Complete-GuardedCleanupLease {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][ValidateSet('owned-ephemeral-user-data', 'owned-ephemeral-workspace')][string]$Kind,
        [Parameter(Mandatory = $true)][string]$OwnerManifest,
        [Parameter(Mandatory = $true)][string]$Nonce
    )

    $lease = Read-ControlledLeaseCore -ManifestPath $OwnerManifest -Target $Target -Kind $Kind -Nonce $Nonce -ProjectRoot $ProjectRoot
    if (-not (Test-Path -LiteralPath $lease.TargetPath -PathType Container)) { throw "lease target is missing before seal" }
    Assert-NoReparsePath -Path $lease.TargetPath -InspectDescendants
    Assert-TimeNearIssue (Get-Item -LiteralPath $lease.TargetPath -Force).CreationTimeUtc $lease.IssuedAt 'target root'
    $targetMarker = Join-Path $lease.TargetPath '.adm-cleanup-root.json'
    if (Test-Path -LiteralPath $targetMarker) {
        if (-not (Get-FileHash -LiteralPath $targetMarker -Algorithm SHA256).Hash.Equals($lease.MarkerSha256, [StringComparison]::OrdinalIgnoreCase)) {
            throw "existing target root marker does not match the lease"
        }
    }
    else {
        Write-BytesCreateNew -Path $targetMarker -Bytes $lease.MarkerBytes
    }

    $sealPath = Join-Path $lease.LeaseDirectory 'seal.json'
    if (Test-Path -LiteralPath $sealPath) {
        $existing = Get-Content -LiteralPath $sealPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($existing.leaseId -ne $lease.Manifest.leaseId -or
            -not ([string]$existing.manifestSha256).Equals($lease.ManifestSha256, [StringComparison]::OrdinalIgnoreCase) -or
            -not ([string]$existing.rootMarkerSha256).Equals($lease.MarkerSha256, [StringComparison]::OrdinalIgnoreCase)) {
            throw "existing lease seal is inconsistent"
        }
    }
    else {
        $seal = [ordered]@{
            schemaVersion = $script:LeaseSchema
            sealKind = 'adm-newrust-cleanup-seal'
            projectId = $script:ProjectId
            leaseId = [string]$lease.Manifest.leaseId
            cleanupKind = $Kind
            nonce = $Nonce
            issuedAtUtc = [string]$lease.Manifest.issuedAtUtc
            sealedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
            targetPath = $lease.TargetPath
            manifestSha256 = $lease.ManifestSha256
            rootMarkerSha256 = $lease.MarkerSha256
        }
        Write-BytesCreateNew -Path $sealPath -Bytes (ConvertTo-Utf8JsonBytes $seal)
    }
    $null = Get-TrustedCleanupLease -ManifestPath $lease.ManifestPath -Target $lease.TargetPath `
        -Kind $Kind -Nonce $Nonce -ProjectRoot $ProjectRoot
    [pscustomobject]@{
        schemaVersion = $script:LeaseSchema
        operation = 'sealed'
        projectId = $script:ProjectId
        leaseId = [string]$lease.Manifest.leaseId
        kind = $Kind
        nonce = $Nonce
        target = $lease.TargetPath
        ownerManifest = $lease.ManifestPath
    }
}
function Get-TrustedCleanupLease {
    param(
        [Parameter(Mandatory = $true)][string]$ManifestPath,
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$Kind,
        [Parameter(Mandatory = $true)][string]$Nonce,
        [Parameter(Mandatory = $true)][string]$ProjectRoot
    )

    $lease = Read-ControlledLeaseCore -ManifestPath $ManifestPath -Target $Target -Kind $Kind -Nonce $Nonce -ProjectRoot $ProjectRoot
    $sealPath = Join-Path $lease.LeaseDirectory 'seal.json'
    if (-not (Test-Path -LiteralPath $sealPath -PathType Leaf)) { throw "cleanup lease was not sealed by the issuer" }
    Assert-NoReparsePath -Path $sealPath
    $seal = Get-Content -LiteralPath $sealPath -Raw -Encoding UTF8 | ConvertFrom-Json
    $sealedAt = ConvertTo-LeaseTime ([string]$seal.sealedAtUtc) 'sealedAtUtc'
    if ([int]$seal.schemaVersion -ne $script:LeaseSchema -or
        $seal.sealKind -ne 'adm-newrust-cleanup-seal' -or
        $seal.projectId -ne $script:ProjectId -or
        $seal.leaseId -ne $lease.Manifest.leaseId -or
        $seal.cleanupKind -ne $Kind -or
        -not ([string]$seal.nonce).Equals($Nonce, [StringComparison]::Ordinal) -or
        -not (ConvertTo-NormalizedPath ([string]$seal.targetPath)).Equals($lease.TargetPath, $script:Comparison) -or
        -not ([string]$seal.manifestSha256).Equals($lease.ManifestSha256, [StringComparison]::OrdinalIgnoreCase) -or
        -not ([string]$seal.rootMarkerSha256).Equals($lease.MarkerSha256, [StringComparison]::OrdinalIgnoreCase) -or
        $sealedAt -lt $lease.IssuedAt -or $sealedAt -gt $lease.ExpiresAt) {
        throw "cleanup lease seal is invalid"
    }

    $targetExists = Test-Path -LiteralPath $lease.TargetPath -PathType Container
    if ($targetExists) {
        Assert-NoReparsePath -Path $lease.TargetPath -InspectDescendants
        Assert-TimeNearIssue (Get-Item -LiteralPath $lease.TargetPath -Force).CreationTimeUtc $lease.IssuedAt 'target root'
        $targetMarker = Join-Path $lease.TargetPath '.adm-cleanup-root.json'
        if (-not (Test-Path -LiteralPath $targetMarker -PathType Leaf) -or
            -not (Get-FileHash -LiteralPath $targetMarker -Algorithm SHA256).Hash.Equals($lease.MarkerSha256, [StringComparison]::OrdinalIgnoreCase)) {
            throw "target root marker is missing or inconsistent"
        }
    }
    [pscustomobject]@{
        Manifest = $lease.Manifest
        ManifestPath = $lease.ManifestPath
        ManifestSha256 = $lease.ManifestSha256
        TargetExists = $targetExists
        TargetPath = $lease.TargetPath
        LeaseDirectory = $lease.LeaseDirectory
        MarkerSha256 = $lease.MarkerSha256
        IssuedAt = $lease.IssuedAt
        ExpiresAt = $lease.ExpiresAt
    }
}

Export-ModuleMember -Function New-GuardedCleanupLease, Complete-GuardedCleanupLease, Get-TrustedCleanupLease
