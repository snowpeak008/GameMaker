[CmdletBinding()]
param(
    [string]$ProjectRoot,
    [string]$TempParent,
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:SchemaVersion = 2
$script:Producer = 'tools/verify-standalone.ps1/v2'
$script:ProjectId = 'autodesignmaker-rust-v2'
$script:PortableRelativeRoot = 'dist/AutoDesignMaker-NEWrust-release'
$script:EvidenceLifetimeSeconds = 86400
$script:RequiredChecks = @(
    'cargo_fmt_check', 'cargo_check_workspace', 'cargo_test_workspace',
    'web_unit', 'web_i18n', 'web_design_content', 'web_build', 'web_e2e',
    'web_language_gate', 'web_ui_gate', 'web_ui_baseline_gate',
    'package_contract_self_test', 'resource_manifest', 'standalone_boundary_gate',
    'portable_build', 'portable_smoke', 'portable_integrity', 'pe_architecture_crt',
    'clean_clone_relocation', 'anti_fake_scan', 'generated_cleanup'
)
$projectRoot = if ([string]::IsNullOrWhiteSpace($ProjectRoot)) {
    [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
} else { [IO.Path]::GetFullPath($ProjectRoot) }
$evidencePath = Join-Path $projectRoot 'gates\standalone-release-evidence.json'
$securityAllowlist = Join-Path $projectRoot 'tools\security-scan-allowlist.json'
$portableModuleRoot = Join-Path $projectRoot 'tools\portable'
$emptySha256 = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855'
$checks = [ordered]@{}
foreach ($id in $script:RequiredChecks) {
    $checks[$id] = [ordered]@{ status = 'not-run'; command = ''; exitCode = -1; durationMs = 0; outputSha256 = $emptySha256 }
}
$errors = New-Object 'System.Collections.Generic.List[string]'
$lease = $null
$tempParentCreated = $false
$gitCommit = ''
$sourceTreeClean = $false
$portableEvidence = $null
$portableTransactionManifest = ''
$evidenceId = [guid]::NewGuid().ToString('N')
$verificationStartedAtUnix = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$cargoVariables = @('CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS', 'CARGO_PROFILE_TEST_DEBUG', 'CARGO_INCREMENTAL')
$originalCargoEnvironment = @{}
foreach ($name in $cargoVariables) {
    $originalCargoEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
}

function Get-Sha256Text {
    param([AllowEmptyString()][string]$Text)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes($Text)
        ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    } finally { $sha.Dispose() }
}

function Write-Utf8NoBomAtomic {
    param([string]$Path, $Value)
    $full = [IO.Path]::GetFullPath($Path)
    $parent = Split-Path -Parent $full
    New-Item -ItemType Directory -Path $parent -Force | Out-Null
    $temporary = "$full.tmp-$([guid]::NewGuid().ToString('N'))"
    try {
        $json = $Value | ConvertTo-Json -Depth 12
        [IO.File]::WriteAllText($temporary, $json + [Environment]::NewLine, (New-Object Text.UTF8Encoding($false)))
        Move-Item -LiteralPath $temporary -Destination $full -Force
    } finally {
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Force }
    }
}

function ConvertTo-NormalizedPath {
    param([string]$Path)
    $full = [IO.Path]::GetFullPath($Path)
    $root = [IO.Path]::GetPathRoot($full)
    if ($full.Equals($root, [StringComparison]::OrdinalIgnoreCase)) { return $root }
    $full.TrimEnd('\', '/')
}

function Test-PathInside {
    param([string]$Path, [string]$Boundary, [switch]$AllowEqual)
    $candidate = ConvertTo-NormalizedPath $Path
    $root = ConvertTo-NormalizedPath $Boundary
    if ($AllowEqual -and $candidate.Equals($root, [StringComparison]::OrdinalIgnoreCase)) { return $true }
    $candidate.StartsWith($root + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)
}

function Assert-SamePath {
    param([string]$Left, [string]$Right, [string]$Message)
    if (-not (ConvertTo-NormalizedPath $Left).Equals((ConvertTo-NormalizedPath $Right), [StringComparison]::OrdinalIgnoreCase)) {
        throw $Message
    }
}

function Get-SafeProjectRelativePath {
    param([string]$Path, [string]$Root)
    $full = ConvertTo-NormalizedPath $Path
    $boundary = ConvertTo-NormalizedPath $Root
    if (-not (Test-PathInside $full $boundary)) { throw "path is outside project root: $full" }
    $relative = $full.Substring($boundary.Length).TrimStart('\', '/').Replace('\', '/')
    if ([IO.Path]::IsPathRooted($relative) -or $relative -match '(^|/)\.\.(/|$)') {
        throw "unsafe project-relative path: $relative"
    }
    $relative
}

function Assert-NoUnresolvedPortableOutputState {
    param([string]$DistRoot, [string]$OutputName, [string]$CurrentReceipt)
    $escaped = [Regex]::Escape($OutputName)
    $receiptPattern = "^\.$escaped\.swap-[a-fA-F0-9]{32}\.json$"
    $artifactPattern = "^\.$escaped\.(?:stage|previous|backup|failed)-"
    $tombstonePattern = "^\.$escaped\.retired-(?:backup|failed)-"
    $lockName = ".$OutputName.operation.lock"
    [int]$receiptCount = 0
    $currentTransactionId = ''
    foreach ($entry in @(Get-ChildItem -LiteralPath $DistRoot -Force -ErrorAction Stop)) {
        if ($entry.Name -match $artifactPattern -or $entry.Name -match $tombstonePattern) {
            throw "unresolved portable output state remains: $($entry.FullName)"
        }
        if ($entry.Name -notmatch $receiptPattern) { continue }
        if ($entry.PSIsContainer) { throw "portable receipt is not a file: $($entry.FullName)" }
        $record = Get-Content -LiteralPath $entry.FullName -Raw -Encoding UTF8 | ConvertFrom-Json
        if ([int]$record.schema_version -ne 1 -or $record.kind -ne 'portable-swap-transaction' -or
            [string]$record.output_name -ne $OutputName -or
            [string]$record.status -notin @('finalized', 'failure_artifact_finalized')) {
            throw "unresolved or invalid portable receipt remains: $($entry.FullName)"
        }
        if ((ConvertTo-NormalizedPath $entry.FullName).Equals((ConvertTo-NormalizedPath $CurrentReceipt), [StringComparison]::OrdinalIgnoreCase)) {
            if ([string]$record.status -ne 'finalized') { throw 'current release receipt is not finalized' }
            $currentTransactionId = [string]$record.transaction_id
            $receiptCount += 1
        }
    }
    if ($receiptCount -ne 1) { throw 'current finalized portable receipt was not found exactly once' }
    $operationLockPath = Join-Path $DistRoot $lockName
    if (Test-Path -LiteralPath $operationLockPath) {
        if (-not (Test-Path -LiteralPath $operationLockPath -PathType Leaf)) {
            throw 'portable output operation lock is not a regular file'
        }
        # Reading fails while another operation holds the file with FileShare.None.
        $operationLock = Get-Content -LiteralPath $operationLockPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ([int]$operationLock.schema_version -ne 1 -or
            $operationLock.kind -ne 'portable-output-operation-lock' -or
            [string]$operationLock.output_name -ne $OutputName -or
            [string]$operationLock.transaction_id -ne $currentTransactionId) {
            throw 'portable output operation lock identity is stale or invalid'
        }
    }
    "current_receipt=finalized; unresolved_artifacts=0"
}

function Assert-TempParentSafe {
    param([string]$Candidate, [string]$SourceRoot)
    $candidatePath = ConvertTo-NormalizedPath $Candidate
    $sourcePath = ConvertTo-NormalizedPath $SourceRoot
    $volumeRoot = [IO.Path]::GetPathRoot($sourcePath)
    if (-not ([IO.Path]::GetPathRoot($candidatePath)).Equals($volumeRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'temporary parent must be on the same volume as the source root'
    }
    if ($candidatePath.Equals($volumeRoot, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'custom temporary parent may not be the volume root'
    }
    $legacyRoot = Split-Path -Parent $sourcePath
    if ((Test-PathInside $candidatePath $sourcePath -AllowEqual) -or
        (-not $legacyRoot.Equals($volumeRoot, [StringComparison]::OrdinalIgnoreCase) -and
            (Test-PathInside $candidatePath $legacyRoot -AllowEqual))) {
        throw "temporary parent overlaps the source or legacy project tree: $candidatePath"
    }
}

function Assert-AncestorsHaveNoProjectResources {
    param([string]$Path)
    $cursor = [IO.Directory]::GetParent((ConvertTo-NormalizedPath $Path))
    while ($null -ne $cursor) {
        foreach ($relative in @('.project_root', 'Cargo.toml', 'knowledge\resource-manifest.json', 'plan\NEWrust', 'gui_app.py')) {
            if (Test-Path -LiteralPath (Join-Path $cursor.FullName $relative)) {
                throw "clone ancestor contains project-owned resources: $($cursor.FullName) [$relative]"
            }
        }
        $next = $cursor.Parent
        if ($null -eq $next -or $next.FullName -eq $cursor.FullName) { break }
        $cursor = $next
    }
}

function Invoke-CapturedNative {
    param([scriptblock]$Body)
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $previous = $ErrorActionPreference
    $lines = @()
    $exitCode = 1
    try {
        $ErrorActionPreference = 'Continue'
        $global:LASTEXITCODE = 0
        $lines = @(& $Body 2>&1)
        $exitCode = [int]$LASTEXITCODE
    } catch {
        $lines += $_.Exception.Message
        $exitCode = 1
    } finally {
        $ErrorActionPreference = $previous
        $watch.Stop()
    }
    $text = (@($lines | ForEach-Object { [string]$_ }) -join [Environment]::NewLine)
    [pscustomobject]@{ ExitCode = $exitCode; DurationMs = [int64]$watch.ElapsedMilliseconds; Text = $text; Lines = $lines }
}

function Set-CheckEvidence {
    param([string]$Id, [string]$Command, [int]$ExitCode, [int64]$DurationMs, [AllowEmptyString()][string]$Output)
    if ($script:RequiredChecks -notcontains $Id) { throw "unknown release check id: $Id" }
    $checks[$Id] = [ordered]@{
        status = if ($ExitCode -eq 0) { 'passed' } else { 'failed' }
        command = $Command
        exitCode = $ExitCode
        durationMs = [Math]::Max([int64]0, $DurationMs)
        outputSha256 = Get-Sha256Text $Output
    }
}

function Invoke-NativeCheck {
    param([string]$Id, [string]$Command, [scriptblock]$Body)
    Write-Host "==> $Id"
    $capture = Invoke-CapturedNative $Body
    if (-not [string]::IsNullOrWhiteSpace($capture.Text)) { Write-Host $capture.Text }
    Set-CheckEvidence $Id $Command $capture.ExitCode $capture.DurationMs $capture.Text
    if ($capture.ExitCode -ne 0) { throw "$Id failed with exit code $($capture.ExitCode)" }
    $capture
}

function Invoke-InternalCheck {
    param([string]$Id, [string]$Command, [scriptblock]$Body)
    Write-Host "==> $Id"
    $watch = [Diagnostics.Stopwatch]::StartNew()
    try {
        $output = [string](& $Body)
        $watch.Stop()
        Set-CheckEvidence $Id $Command 0 $watch.ElapsedMilliseconds $output
        if (-not [string]::IsNullOrWhiteSpace($output)) { Write-Host $output }
        $output
    } catch {
        $watch.Stop()
        Set-CheckEvidence $Id $Command 1 $watch.ElapsedMilliseconds $_.Exception.Message
        throw
    }
}

function Invoke-RequiredNative {
    param([string]$Description, [scriptblock]$Body)
    Write-Host "==> $Description"
    $capture = Invoke-CapturedNative $Body
    if (-not [string]::IsNullOrWhiteSpace($capture.Text)) { Write-Host $capture.Text }
    if ($capture.ExitCode -ne 0) { throw "$Description failed with exit code $($capture.ExitCode)" }
    $capture
}

function Remove-VerifiedEmptyDirectory {
    param([string]$Path, [string]$Boundary)
    if (-not (Test-Path -LiteralPath $Path -PathType Container)) { return }
    $resolved = ConvertTo-NormalizedPath $Path
    if (-not (Test-PathInside $resolved $Boundary)) { throw "empty-directory cleanup escaped boundary: $resolved" }
    if (@(Get-ChildItem -LiteralPath $resolved -Force).Count -ne 0) { throw "cleanup directory is not empty: $resolved" }
    Remove-Item -LiteralPath $resolved -Force
}

function Invoke-StrictSecurityScan {
    param([string]$Root, [string]$AllowlistPath, [string]$LegacyRoot)
    $policy = Get-Content -LiteralPath $AllowlistPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ([int]$policy.schemaVersion -ne 1 -or $policy.projectId -ne $script:ProjectId) { throw 'security scan allowlist identity is invalid' }
    $legacyBackslash = [Regex]::Escape((ConvertTo-NormalizedPath $LegacyRoot))
    $legacySlash = [Regex]::Escape((ConvertTo-NormalizedPath $LegacyRoot).Replace('\', '/'))
    $rules = [ordered]@{
        windows_absolute_path = '(?<![A-Za-z0-9+.-])[A-Za-z]:[\\/]'
        unc_path = '(?<!\\)\\\\(?![?\.])[^\\\s]+\\'
        parent_traversal = '\.\.[\\/]'
        legacy_project_reference = "(?i)(?:$legacyBackslash|$legacySlash|plan[\\/]+NEWrust|AutoDesignMaker[\\/]+NEWrust)"
    }
    $allowed = @{}
    $fileHashes = @{}
    foreach ($property in @($policy.files.PSObject.Properties)) {
        if ([string]$property.Value -notmatch '^[a-f0-9]{64}$') { throw "invalid allowlist file hash: $($property.Name)" }
        $fileHashes[$property.Name] = [string]$property.Value
    }
    foreach ($ruleProperty in @($policy.allow.PSObject.Properties)) {
        $ruleId = [string]$ruleProperty.Name
        if (-not $rules.Contains($ruleId) -or [string]::IsNullOrWhiteSpace([string]$policy.reasons.$ruleId)) {
            throw "unknown or unexplained security allowlist rule: $ruleId"
        }
        foreach ($path in @($ruleProperty.Value)) {
            if (-not $fileHashes.ContainsKey([string]$path)) { throw "allowlist rule references unhashed file: $path" }
            $key = "$ruleId|$path"
            if ($allowed.ContainsKey($key)) { throw "duplicate security allowlist entry: $key" }
            $allowed[$key] = [pscustomobject]@{ path = [string]$path; fileSha256 = $fileHashes[[string]$path] }
        }
    }
    $seen = @{}
    [int]$hitCount = 0
    foreach ($rule in $rules.GetEnumerator()) {
        $capture = Invoke-CapturedNative { & git -C $Root grep -n -I -P $rule.Value -- ':!tools/security-scan-allowlist.json' }
        if ($capture.ExitCode -notin @(0, 1)) { throw "security scan rule failed: $($rule.Key)" }
        if ($capture.ExitCode -eq 1) { continue }
        foreach ($line in @($capture.Lines)) {
            $match = [Regex]::Match([string]$line, '^(.*?):(\d+):(.*)$')
            if (-not $match.Success) { throw "unparseable security scan result: $line" }
            $relative = $match.Groups[1].Value.Replace('\', '/')
            $key = "$($rule.Key)|$relative"
            if (-not $allowed.ContainsKey($key)) { throw "unallowlisted security finding: $key line=$($match.Groups[2].Value)" }
            $actualHash = (Get-FileHash -LiteralPath (Join-Path $Root $relative) -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($actualHash -ne [string]$allowed[$key].fileSha256) { throw "stale security allowlist hash: $key" }
            $seen[$key] = $true
            $hitCount += 1
        }
    }
    $verifierTracked = Invoke-CapturedNative { & git -C $Root ls-files --error-unmatch -- 'tools/verify-standalone.ps1' }
    if ($verifierTracked.ExitCode -ne 0) {
        $relative = 'tools/verify-standalone.ps1'
        $lines = Get-Content -LiteralPath (Join-Path $Root $relative)
        foreach ($rule in $rules.GetEnumerator()) {
            if (-not @($lines | Where-Object { [Regex]::IsMatch($_, $rule.Value) }).Count) { continue }
            $key = "$($rule.Key)|$relative"
            if (-not $allowed.ContainsKey($key)) { throw "unallowlisted security finding: $key" }
            $actualHash = (Get-FileHash -LiteralPath (Join-Path $Root $relative) -Algorithm SHA256).Hash.ToLowerInvariant()
            if ($actualHash -ne [string]$allowed[$key].fileSha256) { throw "stale security allowlist hash: $key" }
            $seen[$key] = $true
            $hitCount += @($lines | Where-Object { [Regex]::IsMatch($_, $rule.Value) }).Count
        }
    }
    foreach ($key in $allowed.Keys) {
        if (-not $seen.ContainsKey($key)) { throw "stale security allowlist entry has no finding: $key" }
    }
    foreach ($path in $fileHashes.Keys) {
        if (-not @($allowed.Values | Where-Object { $_.path -eq $path }).Count) { throw "unused allowlist file hash: $path" }
    }
    "rules=$($rules.Count); findings=$hitCount; allowlist_entries=$($allowed.Count)"
}

function Invoke-SelfTest {
    $tokens = $null
    $parseErrors = $null
    [Management.Automation.Language.Parser]::ParseFile($PSCommandPath, [ref]$tokens, [ref]$parseErrors) | Out-Null
    if ($parseErrors.Count -ne 0) { throw 'verify-standalone parser self-test failed' }
    if ($script:RequiredChecks.Count -ne 21 -or @($script:RequiredChecks | Sort-Object -Unique).Count -ne 21) {
        throw 'required release check set is invalid'
    }
    if ((New-Object Text.UTF8Encoding($false)).GetPreamble().Length -ne 0) { throw 'UTF-8 no-BOM encoder self-test failed' }
    $sourceText = Get-Content -LiteralPath $PSCommandPath -Raw -Encoding UTF8
    foreach ($marker in @(
        "status = 'running'",
        'swapReceiptSha256',
        'staged_immutable_tree',
        'source guarded dry-run',
        'final_evidence=write-after-cleanup'
    )) {
        if (-not $sourceText.Contains($marker)) { throw "standalone evidence self-test marker missing: $marker" }
    }
    $policy = Get-Content -LiteralPath $securityAllowlist -Raw -Encoding UTF8 | ConvertFrom-Json
    if ([int]$policy.schemaVersion -ne 1 -or $policy.projectId -ne $script:ProjectId) { throw 'security allowlist self-test failed' }
    $security = Invoke-StrictSecurityScan $projectRoot $securityAllowlist (Split-Path -Parent $projectRoot)
    [pscustomobject]@{ status = 'passed'; parser = 'passed'; requiredChecks = 21; evidenceContract = 'schema-v2-running-finalized-receipt-cleanup-last'; securityScan = $security; filesystemMutation = $false } | ConvertTo-Json
}

if ($SelfTest) {
    $evidenceExistedBefore = Test-Path -LiteralPath $evidencePath -PathType Leaf
    $evidenceHashBefore = if ($evidenceExistedBefore) {
        (Get-FileHash -LiteralPath $evidencePath -Algorithm SHA256).Hash
    } else { '' }
    $selfTestResult = Invoke-SelfTest
    $evidenceExistsAfter = Test-Path -LiteralPath $evidencePath -PathType Leaf
    $evidenceHashAfter = if ($evidenceExistsAfter) {
        (Get-FileHash -LiteralPath $evidencePath -Algorithm SHA256).Hash
    } else { '' }
    if ($evidenceExistedBefore -ne $evidenceExistsAfter -or $evidenceHashBefore -ne $evidenceHashAfter) {
        throw 'verify-standalone SelfTest mutated release evidence'
    }
    $selfTestResult
    exit 0
}

# Invalidate any prior passed claim before doing work. A crash may leave this
# record in `running`, which is intentionally rejected by the Rust consumer.
$runningEvidence = [ordered]@{
    schemaVersion = $script:SchemaVersion
    producer = $script:Producer
    evidenceId = $evidenceId
    projectId = $script:ProjectId
    status = 'running'
    gitCommit = ''
    sourceTreeClean = $false
    generatedAtUnix = [int64]$verificationStartedAtUnix
    expiresAtUnix = [int64]($verificationStartedAtUnix + $script:EvidenceLifetimeSeconds)
    checks = $checks
    portable = $null
    errors = @()
}
Write-Utf8NoBomAtomic $evidencePath $runningEvidence

try {
    Import-Module (Join-Path $portableModuleRoot 'PortableBuildSupport.psm1') -Force
    Import-Module (Join-Path $portableModuleRoot 'PortableSwap.psm1') -Force
    $marker = Get-Content -LiteralPath (Join-Path $projectRoot '.project_root') -Raw -Encoding UTF8 | ConvertFrom-Json
    if ([int]$marker.schemaVersion -ne 1 -or $marker.kind -ne 'source-project-root' -or $marker.projectId -ne $script:ProjectId) {
        throw 'source project marker is invalid'
    }
    Assert-SamePath (& git -C $projectRoot rev-parse --show-toplevel) $projectRoot 'Git toplevel is not the standalone root'
    $gitCommit = [string](& git -C $projectRoot rev-parse HEAD)
    if ($LASTEXITCODE -ne 0 -or $gitCommit.Trim() -notmatch '^[a-f0-9]{40,64}$') { throw 'Git HEAD is unavailable' }
    $gitCommit = $gitCommit.Trim()
    $sourceTreeClean = [string]::IsNullOrWhiteSpace((@(& git -C $projectRoot status --porcelain=v1 --untracked-files=all) -join "`n"))
    if (-not $sourceTreeClean) { throw 'formal standalone verification requires a clean source worktree' }

    Invoke-InternalCheck 'resource_manifest' 'internal: verify tracked source resource manifest' {
        $manifestPath = Join-Path $projectRoot 'knowledge\resource-manifest.json'
        $manifest = Get-Content -LiteralPath $manifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ([int]$manifest.schemaVersion -ne 1 -or $manifest.projectId -ne $script:ProjectId) { throw 'resource manifest identity invalid' }
        foreach ($group in @($manifest.groups)) {
            $measure = Get-PortableTreeMeasure (Join-Path $projectRoot ([string]$group.path))
            if ($measure.FileCount -ne [int64]$group.files -or $measure.Bytes -ne [int64]$group.bytes -or
                $measure.Digest -ne [string]$group.treeSha256) { throw "resource manifest mismatch: $($group.path)" }
            $tracked = @(& git -C $projectRoot ls-files -- ([string]$group.path))
            if ($LASTEXITCODE -ne 0 -or $tracked.Count -ne $measure.FileCount) { throw "resource group not fully tracked: $($group.path)" }
        }
        "groups=$(@($manifest.groups).Count)"
    } | Out-Null

    $volumeRoot = [IO.Path]::GetPathRoot($projectRoot)
    if ([string]::IsNullOrWhiteSpace($TempParent)) {
        $TempParent = Join-Path $volumeRoot ("独立化验证 空间-{0}-{1}" -f $PID, [guid]::NewGuid().ToString('N'))
    }
    $TempParent = ConvertTo-NormalizedPath $TempParent
    Assert-TempParentSafe $TempParent $projectRoot
    if (-not (Test-Path -LiteralPath $TempParent)) {
        New-Item -ItemType Directory -Path $TempParent | Out-Null
        $tempParentCreated = $true
    }
    Assert-AncestorsHaveNoProjectResources $TempParent
    $leaseCapture = Invoke-RequiredNative 'Issuing cleanup lease' {
        & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $projectRoot 'tools\new-cleanup-lease.ps1') `
            -Operation Issue -Kind owned-ephemeral-workspace -ProjectRoot $projectRoot -TempParent $TempParent -Json
    }
    $lease = $leaseCapture.Text | ConvertFrom-Json
    if ($lease.operation -ne 'issued') { throw 'cleanup lease was refused' }
    $cloneRoot = [string]$lease.target

    Invoke-InternalCheck 'clean_clone_relocation' 'git clone --no-local <source> <leased-clone>; git fsck --full' {
        $clone = Invoke-CapturedNative { & git clone --no-local --quiet $projectRoot $cloneRoot }
        if ($clone.ExitCode -ne 0) { throw "git clone failed: $($clone.Text)" }
        Assert-AncestorsHaveNoProjectResources $cloneRoot
        Assert-SamePath (& git -C $cloneRoot rev-parse --show-toplevel) $cloneRoot 'clone toplevel mismatch'
        if (Test-Path -LiteralPath (Join-Path $cloneRoot '.git\objects\info\alternates')) { throw 'clone uses shared Git alternates' }
        $fsck = Invoke-CapturedNative { & git -C $cloneRoot fsck --full }
        if ($fsck.ExitCode -ne 0) { throw "clone fsck failed: $($fsck.Text)" }
        $cloneCommit = [string](& git -C $cloneRoot rev-parse HEAD)
        if ($cloneCommit.Trim() -ne $gitCommit) { throw 'clone HEAD differs from source HEAD' }
        if (-not [string]::IsNullOrWhiteSpace((@(& git -C $cloneRoot status --porcelain=v1 --untracked-files=all) -join "`n"))) {
            throw 'clone is dirty before verification'
        }
        "commit=$cloneCommit; shared_alternates=false; path_class=same-volume-root-Chinese-space"
    } | Out-Null

    Invoke-RequiredNative 'Installing locked Web dependencies' { & npm.cmd --prefix (Join-Path $cloneRoot 'web') ci } | Out-Null
    Invoke-NativeCheck 'web_unit' 'npm --prefix <clone>/web test' { & npm.cmd --prefix (Join-Path $cloneRoot 'web') test } | Out-Null
    Invoke-NativeCheck 'web_i18n' 'npm --prefix <clone>/web run i18n-test' { & npm.cmd --prefix (Join-Path $cloneRoot 'web') run i18n-test } | Out-Null
    Invoke-NativeCheck 'web_design_content' 'npm --prefix <clone>/web run design-content-check' { & npm.cmd --prefix (Join-Path $cloneRoot 'web') run design-content-check } | Out-Null
    Invoke-NativeCheck 'web_build' 'npm --prefix <clone>/web run build' { & npm.cmd --prefix (Join-Path $cloneRoot 'web') run build } | Out-Null
    Invoke-NativeCheck 'web_e2e' 'npm --prefix <clone>/web run e2e' { & npm.cmd --prefix (Join-Path $cloneRoot 'web') run e2e } | Out-Null
    Invoke-NativeCheck 'web_language_gate' 'npm --prefix <clone>/web run language-gate' { & npm.cmd --prefix (Join-Path $cloneRoot 'web') run language-gate } | Out-Null
    Invoke-NativeCheck 'web_ui_gate' 'npm --prefix <clone>/web run ui-gate' { & npm.cmd --prefix (Join-Path $cloneRoot 'web') run ui-gate } | Out-Null
    Invoke-NativeCheck 'web_ui_baseline_gate' 'npm --prefix <clone>/web run ui-baseline-gate' { & npm.cmd --prefix (Join-Path $cloneRoot 'web') run ui-baseline-gate } | Out-Null

    [Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', (Join-Path $cloneRoot 'target'), 'Process')
    [Environment]::SetEnvironmentVariable('CARGO_BUILD_JOBS', '1', 'Process')
    [Environment]::SetEnvironmentVariable('CARGO_PROFILE_TEST_DEBUG', '0', 'Process')
    [Environment]::SetEnvironmentVariable('CARGO_INCREMENTAL', '0', 'Process')
    Invoke-NativeCheck 'cargo_fmt_check' 'cargo fmt --manifest-path <clone>/Cargo.toml --all -- --check' { & cargo fmt --manifest-path (Join-Path $cloneRoot 'Cargo.toml') --all -- --check } | Out-Null
    Invoke-NativeCheck 'cargo_check_workspace' 'cargo check --manifest-path <clone>/Cargo.toml --workspace --locked -j 1' { & cargo check --manifest-path (Join-Path $cloneRoot 'Cargo.toml') --workspace --locked -j 1 } | Out-Null
    Invoke-NativeCheck 'cargo_test_workspace' 'cargo test --manifest-path <clone>/Cargo.toml --workspace --locked -j 1' { & cargo test --manifest-path (Join-Path $cloneRoot 'Cargo.toml') --workspace --locked -j 1 } | Out-Null
    Invoke-NativeCheck 'standalone_boundary_gate' 'cargo run --manifest-path <clone>/Cargo.toml --locked -p adm-new-cli -- --project-root <clone> standalone-boundary-gate' {
        & cargo run --manifest-path (Join-Path $cloneRoot 'Cargo.toml') --locked -p adm-new-cli -- --project-root $cloneRoot standalone-boundary-gate
    } | Out-Null
    Invoke-NativeCheck 'package_contract_self_test' 'cargo run --manifest-path <clone>/Cargo.toml --locked -p adm-new-cli -- --project-root <clone> package-gate' {
        & cargo run --manifest-path (Join-Path $cloneRoot 'Cargo.toml') --locked -p adm-new-cli -- --project-root $cloneRoot package-gate
    } | Out-Null
    Invoke-InternalCheck 'anti_fake_scan' 'internal: scan all tracked text with tools/security-scan-allowlist.json' {
        Invoke-StrictSecurityScan $cloneRoot (Join-Path $cloneRoot 'tools\security-scan-allowlist.json') (Split-Path -Parent $projectRoot)
    } | Out-Null

    $sourceDistRoot = Join-Path $projectRoot 'dist'
    $receiptPattern = '.AutoDesignMaker-NEWrust-release.swap-*.json'
    $receiptsBefore = @{}
    if (Test-Path -LiteralPath $sourceDistRoot -PathType Container) {
        foreach ($receipt in @(Get-ChildItem -LiteralPath $sourceDistRoot -File -Force -Filter $receiptPattern)) {
            $receiptsBefore[(ConvertTo-NormalizedPath $receipt.FullName)] = $true
        }
    }
    Invoke-NativeCheck 'portable_build' 'powershell tools/build-portable.ps1 -OutputName AutoDesignMaker-NEWrust-release -CleanUserData' {
        & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $projectRoot 'tools\build-portable.ps1') `
            -OutputName 'AutoDesignMaker-NEWrust-release' -CleanUserData
    } | Out-Null
    $newReceipts = @(Get-ChildItem -LiteralPath $sourceDistRoot -File -Force -Filter $receiptPattern |
            Where-Object { -not $receiptsBefore.ContainsKey((ConvertTo-NormalizedPath $_.FullName)) })
    if ($newReceipts.Count -ne 1) {
        throw "portable build did not create exactly one new swap transaction receipt (found $($newReceipts.Count))"
    }
    $portableTransactionManifest = ConvertTo-NormalizedPath $newReceipts[0].FullName
    $portableRoot = Join-Path $projectRoot $script:PortableRelativeRoot
    $portableExe = Join-Path $portableRoot 'AutoDesignMaker.exe'
    Invoke-NativeCheck 'portable_smoke' 'dist/AutoDesignMaker-NEWrust-release/AutoDesignMaker.exe --smoke' {
        & $portableExe --smoke
    } | Out-Null
    Invoke-InternalCheck 'portable_integrity' 'internal: Assert-PortableStage dist/AutoDesignMaker-NEWrust-release' {
        $report = Assert-PortableStage $portableRoot
        if ($report.UserData.FileCount -ne 0 -or $report.UserData.Bytes -ne 0) { throw 'formal portable user_data is not empty' }
        $validateLive = { param($root) $null = Assert-PortableStage $root }
        $dryFinalize = Invoke-PortableSwapFinalization `
            -TransactionManifest $portableTransactionManifest -ValidateLive $validateLive
        if ($dryFinalize.Status -ne 'ready_to_finalize') {
            throw "portable transaction finalization dry-run was not ready: $($dryFinalize.Status)"
        }
        $finalized = Invoke-PortableSwapFinalization `
            -TransactionManifest $portableTransactionManifest -ValidateLive $validateLive -Execute
        if ($finalized.Status -ne 'finalized' -or
            $finalized.ReceiptRetention.Status -ne 'receipt-retention-complete') {
            throw 'portable transaction was verified but not explicitly finalized'
        }
        "groups=$(@($report.ResourceManifest.groups).Count); user_data_files=0; transaction=finalized; old_receipts_pruned=$($finalized.ReceiptRetention.PrunedCount)"
    } | Out-Null
    Invoke-InternalCheck 'pe_architecture_crt' 'internal: dumpbin x64/static-CRT verification' {
        $inspection = Get-PortablePeInspection $portableExe
        "machine=$($inspection.Machine); dynamic_crt_dependencies=$(@($inspection.DynamicCrtDependencies).Count)"
    } | Out-Null
    $buildManifestPath = Join-Path $portableRoot 'build-manifest.json'
    $resourceManifestPath = Join-Path $portableRoot 'portable-resource-manifest.json'
    $buildManifest = Get-Content -LiteralPath $buildManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($buildManifest.git_commit -ne $gitCommit -or $buildManifest.release_mode -ne 'formal' -or [bool]$buildManifest.development_snapshot -or
        $buildManifest.crt_linkage -ne 'static-msvc' -or $buildManifest.pe_machine -ne 'x86_64' -or
        @($buildManifest.dynamic_crt_dependencies).Count -ne 0 -or $buildManifest.user_data_mode -ne 'clean_release') {
        throw 'portable build manifest is not a formal clean current-HEAD x64/static-CRT release'
    }
    $swapReceipt = Get-Content -LiteralPath $portableTransactionManifest -Raw -Encoding UTF8 | ConvertFrom-Json
    $transactionId = [string]$swapReceipt.transaction_id
    if ([int]$swapReceipt.schema_version -ne 1 -or $swapReceipt.kind -ne 'portable-swap-transaction' -or
        $transactionId -notmatch '^[a-fA-F0-9]{32}$' -or
        [string]$swapReceipt.output_name -ne 'AutoDesignMaker-NEWrust-release' -or
        [string]$swapReceipt.status -ne 'finalized' -or [string]$swapReceipt.smoke_status -ne 'passed') {
        throw 'portable swap receipt is not a finalized release transaction'
    }
    if ([string]$buildManifest.transaction_id -ne $transactionId) {
        throw 'portable build manifest and swap receipt transaction IDs differ'
    }
    $postReceiptReport = Assert-PortableStage -StageRoot $portableRoot `
        -ExpectedTransactionId $transactionId -ExpectedImmutableMeasure $swapReceipt.staged_immutable_tree
    Assert-PortableMeasureEqual -Expected $swapReceipt.staged_user_data -Actual $postReceiptReport.UserData `
        -Description 'final receipt-bound portable user_data'
    if ([bool]$swapReceipt.backup_deleted -ne [bool]$swapReceipt.had_previous_live) {
        throw 'portable receipt backup deletion state is inconsistent with the previous-live state'
    }
    Assert-SamePath ([string]$swapReceipt.dist_root) $sourceDistRoot 'portable receipt dist root mismatch'
    Assert-SamePath ([string]$swapReceipt.live_root) $portableRoot 'portable receipt live root mismatch'
    $swapReceiptRelative = Get-SafeProjectRelativePath $portableTransactionManifest $projectRoot
    $expectedReceiptRelative = "dist/.AutoDesignMaker-NEWrust-release.swap-$transactionId.json"
    if ($swapReceiptRelative -cne $expectedReceiptRelative) {
        throw "portable receipt relative path mismatch: $swapReceiptRelative"
    }
    $transactionScan = Assert-NoUnresolvedPortableOutputState `
        -DistRoot $sourceDistRoot -OutputName 'AutoDesignMaker-NEWrust-release' `
        -CurrentReceipt $portableTransactionManifest
    $portableEvidence = [ordered]@{
        root = $script:PortableRelativeRoot
        executable = 'AutoDesignMaker.exe'
        executableSha256 = (Get-FileHash -LiteralPath $portableExe -Algorithm SHA256).Hash.ToLowerInvariant()
        buildManifestSha256 = (Get-FileHash -LiteralPath $buildManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
        resourceManifestSha256 = (Get-FileHash -LiteralPath $resourceManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
        gitCommit = $gitCommit
        swapReceipt = $swapReceiptRelative
        swapReceiptSha256 = (Get-FileHash -LiteralPath $portableTransactionManifest -Algorithm SHA256).Hash.ToLowerInvariant()
        transactionId = $transactionId
        transactionStatus = [string]$swapReceipt.status
    }
    Write-Host $transactionScan
}
catch {
    $errors.Add($_.Exception.Message)
}
finally {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $cleanupText = New-Object Text.StringBuilder
    $cleanupFailures = New-Object 'System.Collections.Generic.List[string]'
    if ($null -ne $lease -and $lease.operation -eq 'issued') {
        try {
            $seal = Invoke-CapturedNative { & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $projectRoot 'tools\new-cleanup-lease.ps1') `
                    -Operation Seal -Kind owned-ephemeral-workspace -ProjectRoot $projectRoot -Target $lease.target `
                    -OwnerManifest $lease.ownerManifest -Nonce $lease.nonce -Json }
            $null = $cleanupText.AppendLine($seal.Text)
            $sealJson = $seal.Text | ConvertFrom-Json
            if ($seal.ExitCode -ne 0 -or $sealJson.operation -ne 'sealed') { throw 'cleanup lease seal did not return sealed' }
            $dry = Invoke-CapturedNative { & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $projectRoot 'tools\clean-generated.ps1') `
                    -Kind owned-ephemeral-workspace -ProjectRoot $projectRoot -Target $lease.target `
                    -OwnerManifest $lease.ownerManifest -Nonce $lease.nonce -Json }
            $null = $cleanupText.AppendLine($dry.Text)
            $dryJson = $dry.Text | ConvertFrom-Json
            if ($dry.ExitCode -ne 0 -or $dryJson.mode -ne 'dry-run' -or $dryJson.resultCount -ne 1 -or
                $dryJson.results[0].action -ne 'dry-run-delete') { throw 'cleanup dry-run did not prove dry-run-delete' }
            $execute = Invoke-CapturedNative { & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $projectRoot 'tools\clean-generated.ps1') `
                    -Kind owned-ephemeral-workspace -ProjectRoot $projectRoot -Target $lease.target `
                    -OwnerManifest $lease.ownerManifest -Nonce $lease.nonce -Execute -Json }
            $null = $cleanupText.AppendLine($execute.Text)
            $executeJson = $execute.Text | ConvertFrom-Json
            if ($execute.ExitCode -ne 0 -or $executeJson.mode -ne 'execute' -or $executeJson.deletedCount -ne 1 -or
                $executeJson.results[0].action -ne 'deleted' -or (Test-Path -LiteralPath $lease.target)) {
                throw 'cleanup execute did not prove deleted'
            }
            $retire = Invoke-CapturedNative { & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $projectRoot 'tools\new-cleanup-lease.ps1') `
                    -Operation Retire -Kind owned-ephemeral-workspace -ProjectRoot $projectRoot -Target $lease.target `
                    -OwnerManifest $lease.ownerManifest -Nonce $lease.nonce -Json }
            $null = $cleanupText.AppendLine($retire.Text)
            $retireJson = $retire.Text | ConvertFrom-Json
            if ($retire.ExitCode -ne 0 -or $retireJson.operation -ne 'retired' -or -not $retireJson.receiptRemoved -or
                -not $retireJson.boundaryRemoved -or (Test-Path -LiteralPath $lease.ownerManifest)) {
                throw 'cleanup lease receipt was not retired'
            }
            if ($tempParentCreated) { Remove-VerifiedEmptyDirectory $TempParent ([IO.Path]::GetPathRoot($TempParent)) }
        } catch {
            $cleanupFailures.Add($_.Exception.Message)
        }
    }
    try {
        $protectedUserData = @(
            (Join-Path $projectRoot 'dist\AutoDesignMaker-NEWrust\user_data'),
            (Join-Path $projectRoot 'dist\AutoDesignMaker-NEWrust-release\user_data')
        )
        $sourceDry = Invoke-CapturedNative { & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $projectRoot 'tools\clean-generated.ps1') `
                -ProjectRoot $projectRoot -Kind generated -ProtectedUserData $protectedUserData -Json }
        $null = $cleanupText.AppendLine($sourceDry.Text)
        $sourceDryJson = $sourceDry.Text | ConvertFrom-Json
        $sourceDryActions = @($sourceDryJson.results | ForEach-Object { [string]$_.action })
        if ($sourceDry.ExitCode -ne 0 -or $sourceDryJson.mode -ne 'dry-run' -or [int]$sourceDryJson.refusedCount -ne 0 -or
            @($sourceDryActions | Where-Object { $_ -notin @('dry-run-delete', 'skipped') }).Count -ne 0) {
            throw 'source generated cleanup dry-run was not fully guarded'
        }
        $sourceTargets = @($sourceDryJson.results | Where-Object { $_.action -eq 'dry-run-delete' } | ForEach-Object { [string]$_.target })
        $plannedDeletes = $sourceTargets.Count
        $sourceExecute = if ($plannedDeletes -gt 0) {
            Invoke-CapturedNative { & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $projectRoot 'tools\clean-generated.ps1') `
                    -ProjectRoot $projectRoot -Kind generated -Target $sourceTargets `
                    -ProtectedUserData $protectedUserData -Execute -Json }
        } else {
            Invoke-CapturedNative { & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $projectRoot 'tools\clean-generated.ps1') `
                    -ProjectRoot $projectRoot -Kind generated -ProtectedUserData $protectedUserData -Execute -Json }
        }
        $null = $cleanupText.AppendLine($sourceExecute.Text)
        $sourceExecuteJson = $sourceExecute.Text | ConvertFrom-Json
        $sourceExecuteActions = @($sourceExecuteJson.results | ForEach-Object { [string]$_.action })
        if ($sourceExecute.ExitCode -ne 0 -or $sourceExecuteJson.mode -ne 'execute' -or [int]$sourceExecuteJson.refusedCount -ne 0 -or
            [int]$sourceExecuteJson.deletedCount -ne $plannedDeletes -or
            @($sourceExecuteActions | Where-Object { $_ -notin @('deleted', 'skipped') }).Count -ne 0) {
            throw 'source generated cleanup execute did not match its dry-run plan'
        }
        if (-not (Test-Path -LiteralPath $evidencePath -PathType Leaf)) {
            throw 'guarded cleanup removed the protected running release evidence'
        }
        $retainedEvidence = Get-Content -LiteralPath $evidencePath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($retainedEvidence.status -ne 'running' -or $retainedEvidence.evidenceId -ne $evidenceId) {
            throw 'protected running evidence changed before the final evidence write'
        }
        if ($null -ne $portableEvidence) {
            if (-not (Test-Path -LiteralPath $portableRoot -PathType Container) -or
                -not (Test-Path -LiteralPath $portableTransactionManifest -PathType Leaf)) {
                throw 'guarded cleanup removed the formal portable or its finalized receipt'
            }
        }
        $null = $cleanupText.AppendLine("source_planned=$plannedDeletes; source_deleted=$($sourceExecuteJson.deletedCount); formal_portable=preserved; running_evidence=protected; final_evidence=write-after-cleanup")
    } catch {
        $cleanupFailures.Add($_.Exception.Message)
    }
    $watch.Stop()
    if ($cleanupFailures.Count -eq 0) {
        Set-CheckEvidence 'generated_cleanup' 'leased clone cleanup; source guarded dry-run; source guarded execute; preserve portable; write evidence last' 0 $watch.ElapsedMilliseconds $cleanupText.ToString()
    } else {
        $failureText = @($cleanupFailures | ForEach-Object { $_ }) -join '; '
        Set-CheckEvidence 'generated_cleanup' 'leased clone cleanup; source guarded dry-run; source guarded execute; preserve portable; write evidence last' 1 $watch.ElapsedMilliseconds ($cleanupText.ToString() + $failureText)
        foreach ($failure in $cleanupFailures) { $errors.Add($failure) }
    }
    if ($tempParentCreated -and (Test-Path -LiteralPath $TempParent -PathType Container)) {
        try { Remove-VerifiedEmptyDirectory $TempParent ([IO.Path]::GetPathRoot($TempParent)) }
        catch { $errors.Add("temporary parent cleanup failed: $($_.Exception.Message)") }
    }
    foreach ($name in $cargoVariables) {
        [Environment]::SetEnvironmentVariable($name, $originalCargoEnvironment[$name], 'Process')
    }
}

try {
    $currentCommit = [string](& git -C $projectRoot rev-parse HEAD)
    $sourceTreeClean = [string]::IsNullOrWhiteSpace((@(& git -C $projectRoot status --porcelain=v1 --untracked-files=all) -join "`n"))
    if ($LASTEXITCODE -ne 0 -or $currentCommit.Trim() -ne $gitCommit -or -not $sourceTreeClean) {
        $errors.Add('source HEAD or worktree changed while standalone verification was running')
    }
} catch { $errors.Add("final source Git verification failed: $($_.Exception.Message)") }

$allPassed = @($checks.Values | Where-Object { $_.status -ne 'passed' }).Count -eq 0
$now = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
$status = if ($allPassed -and $errors.Count -eq 0 -and $sourceTreeClean -and $null -ne $portableEvidence) { 'passed' } else { 'blocked' }
$evidence = [ordered]@{
    schemaVersion = $script:SchemaVersion
    producer = $script:Producer
    evidenceId = $evidenceId
    projectId = $script:ProjectId
    status = $status
    gitCommit = $gitCommit
    sourceTreeClean = $sourceTreeClean
    generatedAtUnix = [int64]$now
    expiresAtUnix = [int64]($now + $script:EvidenceLifetimeSeconds)
    checks = $checks
    portable = $portableEvidence
    errors = @($errors | ForEach-Object { $_ })
}
Write-Utf8NoBomAtomic $evidencePath $evidence
$evidence | ConvertTo-Json -Depth 12
if ($status -ne 'passed') { exit 1 }
