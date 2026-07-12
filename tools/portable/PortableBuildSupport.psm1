Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:PathComparison = [System.StringComparison]::OrdinalIgnoreCase
$script:EmptyTreeSha256 = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
$script:PortableModes = @('required-read-only', 'seed-read-only')
$script:RequiredPortableGroups = @(
    'knowledge/design_data',
    'knowledge/schemas',
    'knowledge/market_data',
    'knowledge/sdks',
    'knowledge/skills',
    'pipeline/artifact_layer'
)

function ConvertTo-PortableFullPath {
    param([Parameter(Mandatory = $true)][string] $Path)
    $full = [System.IO.Path]::GetFullPath($Path)
    $root = [System.IO.Path]::GetPathRoot($full)
    if ($full.Equals($root, $script:PathComparison)) { return $root }
    $full.TrimEnd('\', '/')
}

function Test-PortablePathWithin {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][string] $Boundary,
        [switch] $AllowEqual
    )
    $candidate = ConvertTo-PortableFullPath $Path
    $root = ConvertTo-PortableFullPath $Boundary
    if ($AllowEqual -and $candidate.Equals($root, $script:PathComparison)) { return $true }
    $prefix = $root + [System.IO.Path]::DirectorySeparatorChar
    $candidate.StartsWith($prefix, $script:PathComparison)
}

function Assert-NoPortableReparseAncestors {
    param([Parameter(Mandatory = $true)][string] $Path)
    $cursor = ConvertTo-PortableFullPath $Path
    while (-not (Test-Path -LiteralPath $cursor)) {
        $parent = [System.IO.Directory]::GetParent($cursor)
        if ($null -eq $parent) { break }
        $cursor = $parent.FullName
    }
    while (-not [string]::IsNullOrWhiteSpace($cursor)) {
        $item = Get-Item -LiteralPath $cursor -Force
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "reparse path is not supported: $($item.FullName)"
        }
        $parent = [System.IO.Directory]::GetParent($item.FullName)
        if ($null -eq $parent -or $parent.FullName.Equals($item.FullName, $script:PathComparison)) {
            break
        }
        $cursor = $parent.FullName
    }
}

function Get-PortableTreeFiles {
    param([Parameter(Mandatory = $true)][string] $Root)
    $fullRoot = ConvertTo-PortableFullPath $Root
    if (-not (Test-Path -LiteralPath $fullRoot -PathType Container)) { return @() }
    Assert-NoPortableReparseAncestors $fullRoot
    $pending = New-Object 'System.Collections.Generic.Queue[string]'
    $pending.Enqueue($fullRoot)
    $files = New-Object 'System.Collections.Generic.List[System.IO.FileInfo]'
    while ($pending.Count -gt 0) {
        $current = $pending.Dequeue()
        foreach ($entry in @(Get-ChildItem -LiteralPath $current -Force)) {
            if (($entry.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "reparse entry is not supported: $($entry.FullName)"
            }
            if ($entry.PSIsContainer) {
                $pending.Enqueue($entry.FullName)
            }
            else {
                $files.Add([System.IO.FileInfo]$entry)
            }
        }
    }
    @($files | Sort-Object FullName)
}

function Get-PortableTreeMeasure {
    param([Parameter(Mandatory = $true)][string] $Path)
    $fullPath = ConvertTo-PortableFullPath $Path
    $exists = Test-Path -LiteralPath $fullPath -PathType Container
    if (-not $exists) {
        return [pscustomobject]@{
            Exists = $false
            FileCount = 0
            Bytes = [long]0
            Digest = $script:EmptyTreeSha256
        }
    }
    $files = @(Get-PortableTreeFiles $fullPath)
    [long]$bytes = 0
    $fingerprints = New-Object 'System.Collections.Generic.List[string]'
    foreach ($file in $files) {
        $bytes += [long]$file.Length
        $relative = $file.FullName.Substring($fullPath.Length).TrimStart('\', '/').Replace('\', '/')
        $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        $fingerprints.Add(('{0}|{1}|{2}' -f $relative, $file.Length, $hash))
    }
    $fingerprintText = $fingerprints -join "`n"
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $bytesToHash = [System.Text.Encoding]::UTF8.GetBytes($fingerprintText)
        $digestBytes = $sha.ComputeHash($bytesToHash)
        $digest = ([System.BitConverter]::ToString($digestBytes)).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
    [pscustomobject]@{
        Exists = $true
        FileCount = $files.Count
        Bytes = [long]$bytes
        Digest = $digest
    }
}

function Get-PortableImmutableTreeMeasure {
    param([Parameter(Mandatory = $true)][string] $Path)
    $fullPath = ConvertTo-PortableFullPath $Path
    if (-not (Test-Path -LiteralPath $fullPath -PathType Container)) {
        return [pscustomobject]@{
            Exists = $false
            FileCount = 0
            Bytes = [long]0
            Digest = $script:EmptyTreeSha256
        }
    }
    $files = @(Get-PortableTreeFiles $fullPath | Where-Object {
        $relative = $_.FullName.Substring($fullPath.Length).TrimStart('\', '/').Replace('\', '/')
        $relative -ne '.portable-update.lock' -and
            $relative -ne 'user_data' -and
            -not $relative.StartsWith('user_data/', [StringComparison]::OrdinalIgnoreCase)
    })
    [long]$bytes = 0
    $fingerprints = New-Object 'System.Collections.Generic.List[string]'
    foreach ($file in $files) {
        $bytes += [long]$file.Length
        $relative = $file.FullName.Substring($fullPath.Length).TrimStart('\', '/').Replace('\', '/')
        $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        $fingerprints.Add(('{0}|{1}|{2}' -f $relative, $file.Length, $hash))
    }
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $digestBytes = $sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($fingerprints -join "`n")))
        $digest = ([BitConverter]::ToString($digestBytes)).Replace('-', '').ToLowerInvariant()
    }
    finally { $sha.Dispose() }
    [pscustomobject]@{
        Exists = $true
        FileCount = $files.Count
        Bytes = [long]$bytes
        Digest = $digest
    }
}

function Enter-PortableOutputOperationLock {
    param(
        [Parameter(Mandatory = $true)][string] $DistRoot,
        [Parameter(Mandatory = $true)][ValidatePattern('^[A-Za-z0-9._-]+$')][string] $OutputName,
        [Parameter(Mandatory = $true)][ValidatePattern('^[a-fA-F0-9]{32}$')][string] $TransactionId,
        [Parameter(Mandatory = $true)][string] $Purpose
    )
    $dist = ConvertTo-PortableFullPath $DistRoot
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    Assert-NoPortableReparseAncestors $dist
    $lockPath = Join-Path $dist ('.{0}.operation.lock' -f $OutputName)
    $stream = $null
    try {
        $stream = [IO.File]::Open(
            $lockPath,
            [IO.FileMode]::OpenOrCreate,
            [IO.FileAccess]::ReadWrite,
            [IO.FileShare]::None
        )
        $payload = [ordered]@{
            schema_version = 1
            kind = 'portable-output-operation-lock'
            output_name = $OutputName
            transaction_id = $TransactionId
            purpose = $Purpose
            pid = $PID
            acquired_at_utc = [DateTime]::UtcNow.ToString('o')
        } | ConvertTo-Json -Compress
        $bytes = [Text.UTF8Encoding]::new($false).GetBytes($payload + [Environment]::NewLine)
        $stream.SetLength(0)
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
        [pscustomobject]@{
            Path = $lockPath
            DistRoot = $dist
            OutputName = $OutputName
            TransactionId = $TransactionId
            Stream = $stream
        }
    }
    catch {
        if ($null -ne $stream) { $stream.Dispose() }
        throw "portable output is busy or its exclusive lock is unavailable: $lockPath; $($_.Exception.Message)"
    }
}

function Exit-PortableOutputOperationLock {
    param([Parameter(Mandatory = $true)] $Lock)
    if ($null -ne $Lock.Stream) {
        $Lock.Stream.Dispose()
        $Lock.Stream = $null
    }
}

function Test-PortableMeasureEqual {
    param(
        [Parameter(Mandatory = $true)] $Expected,
        [Parameter(Mandatory = $true)] $Actual,
        [switch] $IgnoreExists
    )
    ($IgnoreExists -or [bool]$Expected.Exists -eq [bool]$Actual.Exists) -and
        [int64]$Expected.FileCount -eq [int64]$Actual.FileCount -and
        [int64]$Expected.Bytes -eq [int64]$Actual.Bytes -and
        ([string]$Expected.Digest).Equals([string]$Actual.Digest, [System.StringComparison]::OrdinalIgnoreCase)
}

function Assert-PortableMeasureEqual {
    param(
        [Parameter(Mandatory = $true)] $Expected,
        [Parameter(Mandatory = $true)] $Actual,
        [Parameter(Mandatory = $true)][string] $Description,
        [switch] $IgnoreExists
    )
    if (-not (Test-PortableMeasureEqual -Expected $Expected -Actual $Actual -IgnoreExists:$IgnoreExists)) {
        throw "$Description tree measure mismatch: expected files=$($Expected.FileCount) bytes=$($Expected.Bytes) digest=$($Expected.Digest); actual files=$($Actual.FileCount) bytes=$($Actual.Bytes) digest=$($Actual.Digest)"
    }
}

function Write-PortableJsonAtomic {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)] $Value,
        [int] $Depth = 12
    )
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $parent = [System.IO.Directory]::GetParent($fullPath)
    if ($null -eq $parent) { throw "JSON path has no parent: $fullPath" }
    New-Item -ItemType Directory -Path $parent.FullName -Force | Out-Null
    Assert-NoPortableReparseAncestors $parent.FullName
    $temporary = "$fullPath.tmp-$([guid]::NewGuid().ToString('N'))"
    try {
        $json = $Value | ConvertTo-Json -Depth $Depth
        [System.IO.File]::WriteAllText($temporary, $json + [Environment]::NewLine, [System.Text.UTF8Encoding]::new($false))
        Move-Item -LiteralPath $temporary -Destination $fullPath -Force
    }
    finally {
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Force }
    }
}

function Read-PortableJson {
    param([Parameter(Mandatory = $true)][string] $Path)
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { throw "required JSON file is missing: $Path" }
    try {
        Get-Content -LiteralPath $Path -Raw -Encoding UTF8 | ConvertFrom-Json
    }
    catch {
        throw "invalid JSON file $Path`: $($_.Exception.Message)"
    }
}

function Read-PortableSourceResourceGroups {
    param(
        [Parameter(Mandatory = $true)][string] $ProjectRoot,
        [Parameter(Mandatory = $true)][string] $ManifestPath
    )
    $project = ConvertTo-PortableFullPath $ProjectRoot
    $manifest = Read-PortableJson $ManifestPath
    if ([int]$manifest.schemaVersion -ne 1 -or $manifest.projectId -ne 'autodesignmaker-rust-v2') {
        throw 'source resource manifest identity is invalid'
    }
    $groups = New-Object 'System.Collections.Generic.List[object]'
    $seen = @{}
    foreach ($group in @($manifest.groups)) {
        $mode = [string]$group.mode
        if ($script:PortableModes -notcontains $mode) { continue }
        $relative = ([string]$group.path).Replace('\', '/').Trim('/')
        if ([string]::IsNullOrWhiteSpace($relative) -or
            [System.IO.Path]::IsPathRooted($relative) -or
            $relative -match '(^|/)\.\.($|/)') {
            throw "invalid resource group path: $relative"
        }
        if ($seen.ContainsKey($relative)) { throw "duplicate resource group path: $relative" }
        $seen[$relative] = $true
        $source = [System.IO.Path]::GetFullPath((Join-Path $project $relative))
        if (-not (Test-PortablePathWithin -Path $source -Boundary $project)) {
            throw "resource group escaped project root: $relative"
        }
        if (-not (Test-Path -LiteralPath $source -PathType Container)) {
            throw "resource group is missing: $relative"
        }
        $measure = Get-PortableTreeMeasure $source
        $declared = [pscustomobject]@{
            Exists = $true
            FileCount = [int64]$group.files
            Bytes = [int64]$group.bytes
            Digest = [string]$group.treeSha256
        }
        Assert-PortableMeasureEqual -Expected $declared -Actual $measure -Description "source resource group $relative"
        $groups.Add([pscustomobject]@{
            Path = $relative
            Mode = $mode
            SourcePath = $source
            Measure = $measure
        })
    }
    foreach ($required in $script:RequiredPortableGroups) {
        if (-not $seen.ContainsKey($required)) { throw "required portable resource group is absent from source manifest: $required" }
    }
    @($groups | Sort-Object Path)
}

function Copy-PortableResourceGroups {
    param(
        [Parameter(Mandatory = $true)] [object[]] $Groups,
        [Parameter(Mandatory = $true)][string] $StageRoot
    )
    $stage = ConvertTo-PortableFullPath $StageRoot
    $stagedGroups = New-Object 'System.Collections.Generic.List[object]'
    foreach ($group in $Groups) {
        $target = [System.IO.Path]::GetFullPath((Join-Path $stage ([string]$group.Path)))
        if (-not (Test-PortablePathWithin -Path $target -Boundary $stage)) {
            throw "staged resource path escaped stage root: $($group.Path)"
        }
        if (Test-Path -LiteralPath $target) { throw "staged resource target already exists: $target" }
        $parent = [System.IO.Directory]::GetParent($target)
        New-Item -ItemType Directory -Path $parent.FullName -Force | Out-Null
        Copy-Item -LiteralPath $group.SourcePath -Destination $target -Recurse -Force
        $actual = Get-PortableTreeMeasure $target
        Assert-PortableMeasureEqual -Expected $group.Measure -Actual $actual -Description "staged resource group $($group.Path)"
        $stagedGroups.Add([pscustomobject]@{
            Path = [string]$group.Path
            Mode = [string]$group.Mode
            Measure = $actual
        })
    }
    @($stagedGroups | Sort-Object Path)
}

function New-PortableResourceManifestValue {
    param([Parameter(Mandatory = $true)][object[]] $Groups)
    [ordered]@{
        schema_version = 1
        root_kind = 'portable-resource-root'
        groups = @($Groups | Sort-Object Path | ForEach-Object {
            [ordered]@{
                path = [string]$_.Path
                mode = [string]$_.Mode
                files = [int64]$_.Measure.FileCount
                bytes = [int64]$_.Measure.Bytes
                tree_sha256 = [string]$_.Measure.Digest
            }
        })
    }
}

function Test-PortableMajorVersionRange {
    param(
        [Parameter(Mandatory = $true)][string] $Version,
        [Parameter(Mandatory = $true)][string] $Range
    )
    if ($Version -notmatch '(?i)^v?(\d+)') { return $false }
    $major = [int]$Matches[1]
    foreach ($token in @($Range -split '\s+' | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })) {
        if ($token -notmatch '^(>=|>|<=|<|=|\^|~)?(\d+)') { throw "unsupported engines token: $token" }
        $operator = [string]$Matches[1]
        $required = [int]$Matches[2]
        switch ($operator) {
            '>=' { if ($major -lt $required) { return $false } }
            '>'  { if ($major -le $required) { return $false } }
            '<=' { if ($major -gt $required) { return $false } }
            '<'  { if ($major -ge $required) { return $false } }
            default { if ($major -ne $required) { return $false } }
        }
    }
    $true
}

function Assert-PortableNodeEngines {
    param([Parameter(Mandatory = $true)][string] $PackageJsonPath)
    $package = Read-PortableJson $PackageJsonPath
    $nodeVersion = [string](& node --version)
    if ($LASTEXITCODE -ne 0) { throw 'node --version failed' }
    $npmProgram = if ($env:OS -eq 'Windows_NT') { 'npm.cmd' } else { 'npm' }
    $npmVersion = [string](& $npmProgram --version)
    if ($LASTEXITCODE -ne 0) { throw "$npmProgram --version failed" }
    if (-not (Test-PortableMajorVersionRange -Version $nodeVersion.Trim() -Range ([string]$package.engines.node))) {
        throw "Node.js $nodeVersion does not satisfy engines.node $($package.engines.node)"
    }
    if (-not (Test-PortableMajorVersionRange -Version $npmVersion.Trim() -Range ([string]$package.engines.npm))) {
        throw "npm $npmVersion does not satisfy engines.npm $($package.engines.npm)"
    }
    [pscustomobject]@{ NodeVersion = $nodeVersion.Trim(); NpmVersion = $npmVersion.Trim() }
}

function Assert-PortableCargoTargetPath {
    param(
        [Parameter(Mandatory = $true)][string] $ProjectRoot,
        [Parameter(Mandatory = $true)][string] $DistRoot,
        [Parameter(Mandatory = $true)][string] $CargoTargetRoot
    )
    $project = ConvertTo-PortableFullPath $ProjectRoot
    $dist = ConvertTo-PortableFullPath $DistRoot
    $target = ConvertTo-PortableFullPath $CargoTargetRoot
    if ($target.Equals($project, $script:PathComparison) -or
        (Test-PortablePathWithin -Path $project -Boundary $target -AllowEqual)) {
        throw "CARGO_TARGET_DIR may not contain or equal the project root: $target"
    }
    if ((Test-PortablePathWithin -Path $target -Boundary $dist -AllowEqual) -or
        (Test-PortablePathWithin -Path $dist -Boundary $target -AllowEqual)) {
        throw "CARGO_TARGET_DIR may not overlap dist: $target"
    }
    if (Test-PortablePathWithin -Path $target -Boundary $project) {
        $expected = Join-Path $project 'target'
        if (-not $target.Equals((ConvertTo-PortableFullPath $expected), $script:PathComparison)) {
            throw "in-project CARGO_TARGET_DIR must be the dedicated target directory: $target"
        }
    }
    Assert-NoPortableReparseAncestors $target
}

function Invoke-PortableGitCapture {
    param(
        [Parameter(Mandatory = $true)][string] $ProjectRoot,
        [Parameter(Mandatory = $true)][string[]] $Arguments
    )
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'SilentlyContinue'
        $output = @(& git -C $ProjectRoot @Arguments 2>$null)
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    [pscustomobject]@{ Output = $output; ExitCode = $exitCode }
}

function Get-PortableGitState {
    param(
        [Parameter(Mandatory = $true)][string] $ProjectRoot,
        [Parameter(Mandatory = $true)][object[]] $ResourceGroups,
        [Parameter(Mandatory = $true)][string[]] $RequiredTrackedPaths,
        [switch] $DevelopmentSnapshot
    )
    $git = Get-Command git -ErrorAction SilentlyContinue
    if ($null -eq $git) {
        if (-not $DevelopmentSnapshot) { throw 'git is required for a formal portable release' }
        return [pscustomobject]@{ Commit = 'unavailable'; Dirty = $true; DevelopmentSnapshot = $true; TrackedResourceFiles = 0 }
    }
    $commitResult = Invoke-PortableGitCapture -ProjectRoot $ProjectRoot -Arguments @('rev-parse', 'HEAD')
    $commit = [string]($commitResult.Output -join '')
    if ($commitResult.ExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($commit)) {
        if (-not $DevelopmentSnapshot) { throw 'formal portable release requires a Git HEAD' }
        $commit = 'unavailable'
    }
    $statusResult = Invoke-PortableGitCapture -ProjectRoot $ProjectRoot `
        -Arguments @('status', '--porcelain=v1', '--untracked-files=all')
    $status = @($statusResult.Output)
    if ($statusResult.ExitCode -ne 0) {
        if ($DevelopmentSnapshot) {
            return [pscustomobject]@{ Commit = $commit; Dirty = $true; DevelopmentSnapshot = $true; TrackedResourceFiles = 0 }
        }
        throw 'git status failed'
    }
    $dirty = $status.Count -gt 0
    if ($dirty -and -not $DevelopmentSnapshot) {
        throw "formal portable release requires a clean Git worktree; use -DevelopmentSnapshot only for a non-release build"
    }
    [int64]$trackedResourceFiles = 0
    foreach ($group in $ResourceGroups) {
        $trackedResult = Invoke-PortableGitCapture -ProjectRoot $ProjectRoot `
            -Arguments @('ls-files', '--', [string]$group.Path)
        $tracked = @($trackedResult.Output)
        if ($trackedResult.ExitCode -ne 0) {
            if ($DevelopmentSnapshot) { continue }
            throw "git ls-files failed for $($group.Path)"
        }
        $trackedResourceFiles += $tracked.Count
        if (-not $DevelopmentSnapshot -and $tracked.Count -ne [int64]$group.Measure.FileCount) {
            throw "resource group is not fully tracked by Git: $($group.Path) expected=$($group.Measure.FileCount) tracked=$($tracked.Count)"
        }
    }
    foreach ($relative in $RequiredTrackedPaths) {
        $requiredResult = Invoke-PortableGitCapture -ProjectRoot $ProjectRoot `
            -Arguments @('ls-files', '--error-unmatch', '--', $relative)
        if ($requiredResult.ExitCode -ne 0 -and -not $DevelopmentSnapshot) {
            throw "required release input is not tracked by Git: $relative"
        }
    }
    [pscustomobject]@{
        Commit = $commit.Trim()
        Dirty = $dirty
        DevelopmentSnapshot = [bool]$DevelopmentSnapshot
        TrackedResourceFiles = $trackedResourceFiles
    }
}

function Get-PortableDumpbinPath {
    $command = Get-Command dumpbin.exe -ErrorAction SilentlyContinue
    if ($null -ne $command) { return $command.Source }
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path -LiteralPath $vswhere -PathType Leaf) {
        $candidate = & $vswhere -latest -products * -find 'VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe' | Select-Object -First 1
        if (-not [string]::IsNullOrWhiteSpace($candidate)) { return [System.IO.Path]::GetFullPath($candidate) }
    }
    throw 'dumpbin.exe was not found; PE architecture and static CRT cannot be verified'
}

function Test-PortableDynamicCrtDependency {
    param([Parameter(Mandatory = $true)][string] $Name)
    $Name -match '(?i)^(vcruntime|msvcp|concrt).*\.dll$' -or
        $Name -match '(?i)^ucrtbase\.dll$' -or
        $Name -match '(?i)^api-ms-win-crt-.*\.dll$'
}

function Get-PortablePeInspection {
    param(
        [Parameter(Mandatory = $true)][string] $Executable,
        [string] $DumpbinPath
    )
    if ([string]::IsNullOrWhiteSpace($DumpbinPath)) { $DumpbinPath = Get-PortableDumpbinPath }
    $headers = @(& $DumpbinPath /nologo /headers $Executable)
    if ($LASTEXITCODE -ne 0) { throw "dumpbin /headers failed for $Executable" }
    $headerText = $headers -join "`n"
    if ($headerText -notmatch '(?im)^\s*8664\s+machine\s+\(x64\)') {
        throw "PE machine is not x86-64 for $Executable"
    }
    $dependentOutput = @(& $DumpbinPath /nologo /dependents $Executable)
    if ($LASTEXITCODE -ne 0) { throw "dumpbin /dependents failed for $Executable" }
    $dependencies = @($dependentOutput | ForEach-Object {
        if ($_ -match '^\s+([A-Za-z0-9._-]+\.dll)\s*$') { $Matches[1] }
    } | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Sort-Object -Unique)
    $dynamic = @($dependencies | Where-Object { Test-PortableDynamicCrtDependency $_ })
    if ($dynamic.Count -gt 0) { throw "dynamic MSVC/UCRT dependency detected in $Executable`: $($dynamic -join ', ')" }
    [pscustomobject]@{
        Path = (ConvertTo-PortableFullPath $Executable)
        Machine = 'x86_64'
        Dependencies = $dependencies
        DynamicCrtDependencies = $dynamic
    }
}

function Resolve-PortableStagePath {
    param(
        [Parameter(Mandatory = $true)][string] $StageRoot,
        [Parameter(Mandatory = $true)][string] $RelativePath,
        [Parameter(Mandatory = $true)][string] $Description
    )
    $relative = $RelativePath.Replace('\', '/').Trim('/')
    if ([string]::IsNullOrWhiteSpace($relative) -or
        [System.IO.Path]::IsPathRooted($relative) -or
        $relative -match '(^|/)\.\.($|/)') {
        throw "$Description is not a safe relative path: $RelativePath"
    }
    $resolved = [System.IO.Path]::GetFullPath((Join-Path $StageRoot $relative))
    if (-not (Test-PortablePathWithin -Path $resolved -Boundary $StageRoot)) {
        throw "$Description escaped the portable root: $RelativePath"
    }
    $resolved
}

function Assert-PortableStage {
    param(
        [Parameter(Mandatory = $true)][string] $StageRoot,
        [string] $ExpectedTransactionId = '',
        $ExpectedImmutableMeasure = $null
    )
    $stage = ConvertTo-PortableFullPath $StageRoot
    Assert-NoPortableReparseAncestors $stage
    $required = @(
        'AutoDesignMaker.exe',
        'Start-AutoDesignMaker.cmd',
        'README.txt',
        'build-manifest.json',
        'portable-resource-manifest.json',
        'knowledge/resource-manifest.json',
        'pipeline/artifact_layer/registry.json',
        'user_data'
    )
    foreach ($relative in $required) {
        $path = Join-Path $stage $relative
        if (-not (Test-Path -LiteralPath $path)) { throw "portable stage is incomplete: $relative" }
    }
    $resourceManifestPath = Join-Path $stage 'portable-resource-manifest.json'
    $resourceManifest = Read-PortableJson $resourceManifestPath
    if ([int]$resourceManifest.schema_version -ne 1 -or $resourceManifest.root_kind -ne 'portable-resource-root') {
        throw 'portable resource manifest identity is invalid'
    }
    $seenGroups = @{}
    foreach ($group in @($resourceManifest.groups)) {
        $relative = ([string]$group.path).Replace('\', '/').Trim('/')
        if ($script:PortableModes -notcontains ([string]$group.mode)) {
            throw "portable group has an invalid mode: $relative"
        }
        if ($seenGroups.ContainsKey($relative)) { throw "portable resource group is duplicated: $relative" }
        $seenGroups[$relative] = $true
        $groupPath = Resolve-PortableStagePath -StageRoot $stage -RelativePath $relative `
            -Description 'portable resource group path'
        $actual = Get-PortableTreeMeasure $groupPath
        $expected = [pscustomobject]@{
            Exists = $true
            FileCount = [int64]$group.files
            Bytes = [int64]$group.bytes
            Digest = [string]$group.tree_sha256
        }
        Assert-PortableMeasureEqual -Expected $expected -Actual $actual -Description "portable group $relative"
    }
    foreach ($requiredGroup in $script:RequiredPortableGroups) {
        if (-not $seenGroups.ContainsKey($requiredGroup)) {
            throw "portable resource manifest omitted required group: $requiredGroup"
        }
    }
    $buildManifest = Read-PortableJson (Join-Path $stage 'build-manifest.json')
    if ([int]$buildManifest.schema_version -ne 1 -or $buildManifest.root_kind -ne 'portable-build-root') {
        throw 'portable build manifest identity is invalid'
    }
    if (-not [string]::IsNullOrWhiteSpace($ExpectedTransactionId) -and
        -not ([string]$buildManifest.transaction_id).Equals($ExpectedTransactionId, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'portable build manifest transaction_id does not match its transaction receipt'
    }
    $resourceHash = (Get-FileHash -LiteralPath $resourceManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $resourceHash.Equals([string]$buildManifest.resource_manifest_sha256, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'portable resource manifest hash mismatch'
    }
    if ([string]$buildManifest.executable -ne 'AutoDesignMaker.exe') {
        throw 'portable executable path is invalid'
    }
    $exePath = Resolve-PortableStagePath -StageRoot $stage -RelativePath ([string]$buildManifest.executable) `
        -Description 'portable executable path'
    $exeHash = (Get-FileHash -LiteralPath $exePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $exeHash.Equals([string]$buildManifest.executable_sha256, [System.StringComparison]::OrdinalIgnoreCase) -or
        (Get-Item -LiteralPath $exePath).Length -ne [int64]$buildManifest.executable_bytes) {
        throw 'portable executable hash or size mismatch'
    }
    if ([string]$buildManifest.launcher -ne 'Start-AutoDesignMaker.cmd') {
        throw 'portable launcher path is invalid'
    }
    $launcherPath = Resolve-PortableStagePath -StageRoot $stage -RelativePath ([string]$buildManifest.launcher) `
        -Description 'portable launcher path'
    $launcherHash = (Get-FileHash -LiteralPath $launcherPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $launcherHash.Equals([string]$buildManifest.launcher_sha256, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'portable launcher hash mismatch'
    }
    if ([string]$buildManifest.artifact_registry -ne 'pipeline/artifact_layer/registry.json') {
        throw 'portable artifact registry path is invalid'
    }
    $registryPath = Resolve-PortableStagePath -StageRoot $stage -RelativePath ([string]$buildManifest.artifact_registry) `
        -Description 'portable artifact registry path'
    $registryHash = (Get-FileHash -LiteralPath $registryPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $registryHash.Equals([string]$buildManifest.artifact_registry_sha256, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'portable artifact registry hash mismatch'
    }
    $sourceManifestPath = Join-Path $stage 'knowledge\resource-manifest.json'
    $sourceManifestHash = (Get-FileHash -LiteralPath $sourceManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $sourceManifestHash.Equals([string]$buildManifest.source_resource_manifest_sha256, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw 'source resource manifest hash mismatch'
    }
    foreach ($support in @($buildManifest.support_files)) {
        $supportPath = Resolve-PortableStagePath -StageRoot $stage -RelativePath ([string]$support.path) `
            -Description 'portable support file path'
        if (-not (Test-Path -LiteralPath $supportPath -PathType Leaf)) { throw "portable support file missing: $($support.path)" }
        $hash = (Get-FileHash -LiteralPath $supportPath -Algorithm SHA256).Hash.ToLowerInvariant()
        if (-not $hash.Equals([string]$support.sha256, [System.StringComparison]::OrdinalIgnoreCase) -or
            (Get-Item -LiteralPath $supportPath).Length -ne [int64]$support.bytes) {
            throw "portable support file integrity mismatch: $($support.path)"
        }
    }
    $userDataMeasure = Get-PortableTreeMeasure (Join-Path $stage 'user_data')
    $expectedUserData = [pscustomobject]@{
        Exists = $true
        FileCount = [int64]$buildManifest.user_data_files
        Bytes = [int64]$buildManifest.user_data_bytes
        Digest = [string]$buildManifest.user_data_digest
    }
    Assert-PortableMeasureEqual -Expected $expectedUserData -Actual $userDataMeasure `
        -Description 'portable build-manifest user_data'
    $immutableMeasure = Get-PortableImmutableTreeMeasure $stage
    if ($null -ne $ExpectedImmutableMeasure) {
        Assert-PortableMeasureEqual -Expected $ExpectedImmutableMeasure -Actual $immutableMeasure `
            -Description 'portable immutable candidate tree'
    }
    [pscustomobject]@{
        StageRoot = $stage
        BuildManifest = $buildManifest
        ResourceManifest = $resourceManifest
        UserData = $userDataMeasure
        ImmutableTree = $immutableMeasure
    }
}

Export-ModuleMember -Function @(
    'ConvertTo-PortableFullPath',
    'Test-PortablePathWithin',
    'Assert-NoPortableReparseAncestors',
    'Get-PortableTreeFiles',
    'Get-PortableTreeMeasure',
    'Get-PortableImmutableTreeMeasure',
    'Test-PortableMeasureEqual',
    'Assert-PortableMeasureEqual',
    'Write-PortableJsonAtomic',
    'Read-PortableJson',
    'Enter-PortableOutputOperationLock',
    'Exit-PortableOutputOperationLock',
    'Read-PortableSourceResourceGroups',
    'Copy-PortableResourceGroups',
    'New-PortableResourceManifestValue',
    'Test-PortableMajorVersionRange',
    'Assert-PortableNodeEngines',
    'Assert-PortableCargoTargetPath',
    'Get-PortableGitState',
    'Test-PortableDynamicCrtDependency',
    'Get-PortablePeInspection',
    'Assert-PortableStage'
)
