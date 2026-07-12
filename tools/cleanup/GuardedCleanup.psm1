Set-StrictMode -Version Latest

$script:PathComparison = [StringComparison]::OrdinalIgnoreCase
$script:StandaloneEvidenceRelativePath = 'gates\standalone-release-evidence.json'
$script:ProtectedProjectEntries = @(
    '.git',
    '.cargo',
    'apps',
    'crates',
    'docs',
    'knowledge',
    'pipeline',
    'testdata',
    'tools',
    'web\src'
)

function ConvertTo-NormalizedPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [string]$BasePath = (Get-Location).Path
    )

    $candidate = $Path
    if (-not [IO.Path]::IsPathRooted($candidate)) {
        $candidate = Join-Path $BasePath $candidate
    }

    $fullPath = [IO.Path]::GetFullPath($candidate)
    $root = [IO.Path]::GetPathRoot($fullPath)
    if (-not $fullPath.Equals($root, $script:PathComparison)) {
        $fullPath = $fullPath.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    }
    return $fullPath
}

function Test-PathWithin {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Boundary,
        [switch]$AllowEqual
    )

    $normalizedPath = ConvertTo-NormalizedPath $Path
    $normalizedBoundary = ConvertTo-NormalizedPath $Boundary
    if ($normalizedPath.Equals($normalizedBoundary, $script:PathComparison)) {
        return [bool]$AllowEqual
    }
    $prefix = $normalizedBoundary + [IO.Path]::DirectorySeparatorChar
    return $normalizedPath.StartsWith($prefix, $script:PathComparison)
}

function Test-PathsOverlap {
    param(
        [Parameter(Mandatory = $true)][string]$Left,
        [Parameter(Mandatory = $true)][string]$Right
    )

    return ((Test-PathWithin -Path $Left -Boundary $Right -AllowEqual) -or
        (Test-PathWithin -Path $Right -Boundary $Left -AllowEqual))
}

function Get-PathChain {
    param([Parameter(Mandatory = $true)][string]$Path)

    $current = ConvertTo-NormalizedPath $Path
    $parts = New-Object System.Collections.Generic.List[string]
    while ($current) {
        $parts.Add($current)
        $parent = [IO.Directory]::GetParent($current)
        if ($null -eq $parent) { break }
        $next = ConvertTo-NormalizedPath $parent.FullName
        if ($next.Equals($current, $script:PathComparison)) { break }
        $current = $next
    }
    $result = $parts.ToArray()
    [array]::Reverse($result)
    return $result
}

function Assert-NoReparsePath {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [switch]$InspectDescendants
    )

    foreach ($entryPath in (Get-PathChain $Path)) {
        if (-not (Test-Path -LiteralPath $entryPath)) { continue }
        $entry = Get-Item -LiteralPath $entryPath -Force -ErrorAction Stop
        if (($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "reparse point refused: $($entry.FullName)"
        }
    }

    if (-not $InspectDescendants -or -not (Test-Path -LiteralPath $Path -PathType Container)) {
        return
    }

    $pending = New-Object System.Collections.Generic.Stack[string]
    $pending.Push((ConvertTo-NormalizedPath $Path))
    while ($pending.Count -gt 0) {
        $directory = $pending.Pop()
        foreach ($entry in @(Get-ChildItem -LiteralPath $directory -Force -ErrorAction Stop)) {
            if (($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "reparse point refused: $($entry.FullName)"
            }
            if ($entry.PSIsContainer) {
                $pending.Push($entry.FullName)
            }
        }
    }
}

function Get-TreeMeasure {
    param([Parameter(Mandatory = $true)][string]$Path)

    $root = ConvertTo-NormalizedPath $Path
    Assert-NoReparsePath -Path $root -InspectDescendants
    $records = New-Object System.Collections.Generic.List[string]
    [int64]$bytes = 0
    [int64]$count = 0

    if (Test-Path -LiteralPath $root -PathType Leaf) {
        $file = Get-Item -LiteralPath $root -Force
        $hash = (Get-FileHash -LiteralPath $root -Algorithm SHA256).Hash.ToLowerInvariant()
        $records.Add(".|$($file.Length)|$hash")
        $bytes = $file.Length
        $count = 1
    }
    elseif (Test-Path -LiteralPath $root -PathType Container) {
        foreach ($file in @(Get-ChildItem -LiteralPath $root -File -Recurse -Force -ErrorAction Stop |
                Sort-Object { $_.FullName.Substring($root.Length).ToLowerInvariant() })) {
            $relative = $file.FullName.Substring($root.Length).TrimStart('\', '/').Replace('\', '/')
            $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            $records.Add("$relative|$($file.Length)|$hash")
            $bytes += $file.Length
            $count += 1
        }
    }
    else {
        throw "measure source does not exist: $root"
    }

    $payload = [Text.Encoding]::UTF8.GetBytes(($records -join "`n"))
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $digest = ([BitConverter]::ToString($sha.ComputeHash($payload))).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
    [pscustomobject]@{ fileCount = $count; bytes = $bytes; digest = $digest }
}

function Get-ItemMeasure {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return [pscustomobject]@{ fileCount = 0; bytes = 0 }
    }
    $item = Get-Item -LiteralPath $Path -Force
    if (-not $item.PSIsContainer) {
        return [pscustomobject]@{ fileCount = 1; bytes = [int64]$item.Length }
    }
    [int64]$bytes = 0
    [int64]$count = 0
    foreach ($file in @(Get-ChildItem -LiteralPath $Path -File -Recurse -Force -ErrorAction Stop)) {
        $bytes += $file.Length
        $count += 1
    }
    [pscustomobject]@{ fileCount = $count; bytes = $bytes }
}

function Get-PortablePathKind {
    param([Parameter(Mandatory = $true)][string]$Path)

    foreach ($segment in ((ConvertTo-NormalizedPath $Path) -split '[\\/]')) {
        if ($segment -match '^\..+\.(previous|backup)-[^\\/]+$') { return 'portable-backup' }
        if ($segment -match '^\..+\.stage-[^\\/]+$') { return 'portable-stage' }
    }
    return $null
}

function Get-GeneratedKind {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$ProjectRoot
    )

    $targetPath = ConvertTo-NormalizedPath $Target
    $root = ConvertTo-NormalizedPath $ProjectRoot
    $portableKind = Get-PortablePathKind $targetPath
    if ($portableKind) {
        $distRoot = ConvertTo-NormalizedPath (Join-Path $root 'dist')
        if (Test-PathWithin -Path $targetPath -Boundary $distRoot) {
            $relative = $targetPath.Substring($distRoot.Length).TrimStart('\', '/')
            $first = ($relative -split '[\\/]')[0]
            if ($first -match '^\..+\.(stage|previous|backup)-[^\\/]+$') {
                return $portableKind
            }
        }
        return $null
    }

    $exactRoots = @(
        @{ Path = (Join-Path $root 'target'); Kind = 'cargo-target' },
        @{ Path = (Join-Path $root '.local-build'); Kind = 'local-cargo-target' },
        @{ Path = (Join-Path $root 'web\dist'); Kind = 'web-dist' },
        @{ Path = (Join-Path $root 'web\node_modules'); Kind = 'web-dependency-cache' },
        @{ Path = (Join-Path $root 'web\test-results'); Kind = 'browser-output' },
        @{ Path = (Join-Path $root 'web\playwright-report'); Kind = 'browser-output' },
        @{ Path = (Join-Path $root 'web\.playwright'); Kind = 'browser-profile' }
    )
    foreach ($entry in $exactRoots) {
        if (Test-PathWithin -Path $targetPath -Boundary $entry.Path -AllowEqual) { return $entry.Kind }
    }

    $gatesRoot = Join-Path $root 'gates'
    $standaloneEvidence = ConvertTo-NormalizedPath (Join-Path $root $script:StandaloneEvidenceRelativePath)
    if ($targetPath.Equals($standaloneEvidence, $script:PathComparison)) {
        return 'protected-release-evidence'
    }
    if ((Test-PathWithin -Path $targetPath -Boundary $gatesRoot) -and
        -not $targetPath.Equals((Join-Path $gatesRoot 'README.md'), $script:PathComparison)) {
        return 'generated-gate'
    }

    $tempRoot = Join-Path $root '.tmp'
    if ($targetPath.Equals((ConvertTo-NormalizedPath $tempRoot), $script:PathComparison)) {
        return 'empty-project-temp-root'
    }
    if (Test-PathWithin -Path $targetPath -Boundary $tempRoot) {
        $relative = $targetPath.Substring((ConvertTo-NormalizedPath $tempRoot).Length).TrimStart('\', '/')
        $first = ($relative -split '[\\/]')[0]
        if ($first -match '^(adm-newrust|cargo|web|gate|browser|playwright|test)[-_.]') {
            return 'dedicated-temp'
        }
    }
    return $null
}

function Get-DefaultGeneratedTargets {
    param(
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [switch]$ScanPortableStaging
    )

    $root = ConvertTo-NormalizedPath $ProjectRoot
    $targets = New-Object System.Collections.Generic.List[string]
    # node_modules is intentionally explicit-only: dependency cache retention is a caller decision.
    foreach ($relative in @('target', '.local-build', 'web\dist', 'web\test-results', 'web\playwright-report', 'web\.playwright')) {
        $path = Join-Path $root $relative
        if (Test-Path -LiteralPath $path) { $targets.Add($path) }
    }

    $gates = Join-Path $root 'gates'
    if (Test-Path -LiteralPath $gates -PathType Container) {
        foreach ($entry in @(Get-ChildItem -LiteralPath $gates -Force -ErrorAction Stop)) {
            if (-not $entry.Name.Equals('README.md', $script:PathComparison) -and
                -not $entry.FullName.Equals(
                    (ConvertTo-NormalizedPath (Join-Path $root $script:StandaloneEvidenceRelativePath)),
                    $script:PathComparison)) {
                $targets.Add($entry.FullName)
            }
        }
    }

    $temp = Join-Path $root '.tmp'
    if (Test-Path -LiteralPath $temp -PathType Container) {
        $tempEntries = @(Get-ChildItem -LiteralPath $temp -Force -ErrorAction Stop)
        foreach ($entry in $tempEntries) {
            if ($entry.Name -match '^(adm-newrust|cargo|web|gate|browser|playwright|test)[-_.]') {
                $targets.Add($entry.FullName)
            }
        }
        if ($tempEntries.Count -eq 0) { $targets.Add($temp) }
    }

    if ($ScanPortableStaging) {
        $dist = Join-Path $root 'dist'
        if (Test-Path -LiteralPath $dist -PathType Container) {
            foreach ($entry in @(Get-ChildItem -LiteralPath $dist -Force -ErrorAction Stop)) {
                if ($entry.Name -match '^\..+\.(stage|previous|backup)-[^\\/]+$') {
                    $targets.Add($entry.FullName)
                }
            }
        }
    }
    return @($targets | Sort-Object -Unique)
}

function Assert-SourceProjectRoot {
    param([Parameter(Mandatory = $true)][string]$ProjectRoot)

    $root = ConvertTo-NormalizedPath $ProjectRoot
    if (-not (Test-Path -LiteralPath $root -PathType Container)) { throw "project root missing: $root" }
    Assert-NoReparsePath -Path $root
    foreach ($required in @('.project_root', 'Cargo.toml')) {
        if (-not (Test-Path -LiteralPath (Join-Path $root $required) -PathType Leaf)) {
            throw "invalid source project root; missing $required at $root"
        }
    }
    $manifestPath = Join-Path $root '.project_root'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ([int]$manifest.schemaVersion -ne 1 -or $manifest.kind -ne 'source-project-root' -or
        $manifest.projectId -ne 'autodesignmaker-rust-v2' -or
        $manifest.workspaceManifest -ne 'Cargo.toml') {
        throw "invalid .project_root source identity at $root"
    }
    return $root
}

function Get-ControlledLeaseRoot {
    param([Parameter(Mandatory = $true)][string]$ProjectRoot)
    return ConvertTo-NormalizedPath (Join-Path $ProjectRoot '.tmp\cleanup-leases')
}

function Get-OwnedCleanupTombstonePath {
    param(
        [Parameter(Mandatory = $true)][string]$Boundary,
        [Parameter(Mandatory = $true)][string]$LeaseId,
        [Parameter(Mandatory = $true)][string]$Nonce
    )
    if ($LeaseId -notmatch '^[0-9a-f]{32}$' -or $Nonce -notmatch '^[0-9a-fA-F]{64}$') {
        throw 'cleanup tombstone identity is invalid'
    }
    $normalizedBoundary = ConvertTo-NormalizedPath $Boundary
    return ConvertTo-NormalizedPath (Join-Path $normalizedBoundary `
            ".adm-cleanup-tombstone-$LeaseId-$($Nonce.ToLowerInvariant())")
}

function Get-LeaseRetirementTombstonePath {
    param(
        [Parameter(Mandatory = $true)][string]$LeaseRoot,
        [Parameter(Mandatory = $true)][string]$LeaseId,
        [Parameter(Mandatory = $true)][string]$Nonce
    )
    if ($LeaseId -notmatch '^[0-9a-f]{32}$' -or $Nonce -notmatch '^[0-9a-fA-F]{64}$') {
        throw 'lease retirement tombstone identity is invalid'
    }
    $normalizedLeaseRoot = ConvertTo-NormalizedPath $LeaseRoot
    return ConvertTo-NormalizedPath (Join-Path $normalizedLeaseRoot `
            ".adm-cleanup-retirement-$LeaseId-$($Nonce.ToLowerInvariant())")
}

function Invoke-CleanupFailpoint {
    param([Parameter(Mandatory = $true)][string]$Name)
    if ([string]$env:AUTODESIGNMAKER_CLEANUP_FAILPOINT -eq $Name) {
        throw "injected cleanup interruption at $Name"
    }
}

Export-ModuleMember -Function ConvertTo-NormalizedPath, Test-PathWithin, Test-PathsOverlap,
    Assert-NoReparsePath, Get-TreeMeasure, Get-ItemMeasure, Get-PortablePathKind,
    Get-GeneratedKind, Get-DefaultGeneratedTargets, Assert-SourceProjectRoot,
    Get-ControlledLeaseRoot, Get-OwnedCleanupTombstonePath,
    Get-LeaseRetirementTombstonePath, Invoke-CleanupFailpoint
