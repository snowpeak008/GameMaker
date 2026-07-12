[CmdletBinding()]
param(
    [ValidatePattern('^[A-Za-z0-9._-]+$')]
    [string] $OutputName = 'AutoDesignMaker-NEWrust',

    [switch] $CleanUserData,

    [switch] $DevelopmentSnapshot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$portableToolsRoot = Join-Path $PSScriptRoot 'portable'
Import-Module (Join-Path $portableToolsRoot 'PortableBuildSupport.psm1') -Force
Import-Module (Join-Path $portableToolsRoot 'PortableSwap.psm1') -Force

$projectRoot = ConvertTo-PortableFullPath (Join-Path $PSScriptRoot '..')
$distRoot = ConvertTo-PortableFullPath (Join-Path $projectRoot 'dist')
$targetTriple = 'x86_64-pc-windows-msvc'
$transactionId = [guid]::NewGuid().ToString('N')
$liveRoot = Join-Path $distRoot $OutputName
$stageRoot = Join-Path $distRoot ('.{0}.stage-{1}' -f $OutputName, $transactionId)
$backupRoot = Join-Path $distRoot ('.{0}.previous-{1}' -f $OutputName, $transactionId)
$failedRoot = Join-Path $distRoot ('.{0}.failed-{1}' -f $OutputName, $transactionId)
$transactionManifest = Join-Path $distRoot ('.{0}.swap-{1}.json' -f $OutputName, $transactionId)
$cargoTargetRoot = if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
    Join-Path $projectRoot 'target'
}
elseif ([System.IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) {
    [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
}
else {
    [System.IO.Path]::GetFullPath((Join-Path $projectRoot $env:CARGO_TARGET_DIR))
}
$releaseExecutable = Join-Path (Join-Path (Join-Path $cargoTargetRoot $targetTriple) 'release') 'desktop-tauri.exe'
$webIndex = Join-Path $projectRoot 'web\dist\index.html'
$tauriConfigPath = Join-Path $projectRoot 'apps\desktop-tauri\tauri.conf.json'
$sourceResourceManifest = Join-Path $projectRoot 'knowledge\resource-manifest.json'

function Invoke-CheckedCommand {
    param(
        [Parameter(Mandatory = $true)][string] $Description,
        [Parameter(Mandatory = $true)][scriptblock] $Command
    )
    Write-Host "==> $Description"
    & $Command
    if ($LASTEXITCODE -ne 0) { throw "$Description failed with exit code $LASTEXITCODE" }
}

function New-PortableFileEvidence {
    param(
        [Parameter(Mandatory = $true)][string] $Root,
        [Parameter(Mandatory = $true)][string] $RelativePath
    )
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "portable support file is missing: $RelativePath"
    }
    [ordered]@{
        path = $RelativePath.Replace('\', '/')
        bytes = [int64](Get-Item -LiteralPath $path).Length
        sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

function ConvertTo-PortableTransactionMeasure {
    param([Parameter(Mandatory = $true)] $Measure)
    [ordered]@{
        Exists = [bool]$Measure.Exists
        FileCount = [int64]$Measure.FileCount
        Bytes = [int64]$Measure.Bytes
        Digest = [string]$Measure.Digest
    }
}

function Assert-NoUnresolvedPortableWork {
    if (-not (Test-Path -LiteralPath $distRoot -PathType Container)) { return }
    $directories = @(Get-ChildItem -LiteralPath $distRoot -Directory -Force | Where-Object {
        $_.Name -like ".$OutputName.stage-*" -or
        $_.Name -like ".$OutputName.previous-*" -or
        $_.Name -like ".$OutputName.backup-*" -or
        $_.Name -like ".$OutputName.failed-*" -or
        $_.Name -like ".$OutputName.retired-backup-*" -or
        $_.Name -like ".$OutputName.retired-failed-*"
    })
    $activeTransactions = New-Object 'System.Collections.Generic.List[string]'
    foreach ($file in @(Get-ChildItem -LiteralPath $distRoot -File -Force -Filter ".$OutputName.swap-*.json")) {
        try {
            $record = Read-PortableJson $file.FullName
            if ([string]$record.status -notin @('finalized', 'failure_artifact_finalized')) {
                $activeTransactions.Add("$($file.FullName) [$($record.status)]")
            }
        }
        catch {
            $activeTransactions.Add("$($file.FullName) [invalid]")
        }
    }
    if ($directories.Count -gt 0 -or $activeTransactions.Count -gt 0) {
        $details = @($directories | ForEach-Object FullName) + @($activeTransactions | ForEach-Object { $_ })
        throw "portable recovery artifacts require review/finalization before another build: $($details -join '; ')"
    }
}

foreach ($program in @('node', 'npm.cmd', 'cargo', 'rustc')) {
    if (-not (Get-Command $program -ErrorAction SilentlyContinue)) {
        throw "$program was not found in PATH"
    }
}
foreach ($requiredFile in @(
    '.project_root',
    'Cargo.toml',
    'Cargo.lock',
    'rust-toolchain.toml',
    '.cargo\config.toml',
    'web\package.json',
    'web\package-lock.json',
    'apps\desktop-tauri\tauri.conf.json',
    'knowledge\resource-manifest.json',
    'tools\Finalize-PortableSwap.ps1',
    'tools\portable\Remove-FailedPortableArtifact.ps1',
    'tools\portable\Start-AutoDesignMaker.cmd',
    'tools\portable\README.txt'
)) {
    if (-not (Test-Path -LiteralPath (Join-Path $projectRoot $requiredFile) -PathType Leaf)) {
        throw "required portable build input is missing: $requiredFile"
    }
}

Assert-PortableCargoTargetPath -ProjectRoot $projectRoot -DistRoot $distRoot -CargoTargetRoot $cargoTargetRoot
Assert-NoPortableReparseAncestors $releaseExecutable
Assert-PortableSwapPaths -DistRoot $distRoot -Paths @(
    $liveRoot, $stageRoot, $backupRoot, $failedRoot, $transactionManifest
)
New-Item -ItemType Directory -Path $distRoot -Force | Out-Null
$buildOperationLock = Enter-PortableOutputOperationLock -DistRoot $distRoot -OutputName $OutputName `
    -TransactionId $transactionId -Purpose 'build-stage-swap'
try {
Assert-NoUnresolvedPortableWork

$nodeTools = Assert-PortableNodeEngines (Join-Path $projectRoot 'web\package.json')
$sourceGroups = @(Read-PortableSourceResourceGroups -ProjectRoot $projectRoot `
    -ManifestPath $sourceResourceManifest)
$requiredTrackedPaths = @(
    '.project_root',
    'Cargo.toml',
    'Cargo.lock',
    'rust-toolchain.toml',
    '.cargo/config.toml',
    'web/package.json',
    'web/package-lock.json',
    'apps/desktop-tauri/tauri.conf.json',
    'knowledge/resource-manifest.json',
    'tools/build-portable.ps1',
    'tools/Finalize-PortableSwap.ps1',
    'tools/portable/PortableBuildSupport.psm1',
    'tools/portable/PortableSwap.psm1',
    'tools/portable/Remove-FailedPortableArtifact.ps1',
    'tools/portable/Start-AutoDesignMaker.cmd',
    'tools/portable/README.txt'
)
$gitState = Get-PortableGitState -ProjectRoot $projectRoot -ResourceGroups $sourceGroups `
    -RequiredTrackedPaths $requiredTrackedPaths -DevelopmentSnapshot:$DevelopmentSnapshot

Push-Location $projectRoot
try {
    Invoke-CheckedCommand 'Building the Web UI' {
        & npm.cmd --prefix web run build
    }
    Invoke-CheckedCommand "Building locked desktop release for $targetTriple" {
        & cargo build --locked --target $targetTriple -p desktop-tauri --release
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $webIndex -PathType Leaf)) {
    throw "Web build did not produce $webIndex"
}
if (-not (Test-Path -LiteralPath $releaseExecutable -PathType Leaf)) {
    throw "Rust build did not produce the explicit target output $releaseExecutable"
}
Assert-NoPortableReparseAncestors $releaseExecutable
$sourcePe = Get-PortablePeInspection -Executable $releaseExecutable
$tauriConfig = Read-PortableJson $tauriConfigPath
$rustVersion = [string](& rustc --version)
if ($LASTEXITCODE -ne 0) { throw 'rustc --version failed' }
$initialLiveExists = Test-Path -LiteralPath $liveRoot -PathType Container
$initialUserData = if ($initialLiveExists) {
    Assert-PortableQuiescent $liveRoot
    Assert-PortableDataStable (Join-Path $liveRoot 'user_data')
}
else {
    Get-PortableTreeMeasure (Join-Path $liveRoot 'user_data')
}

$transaction = [ordered]@{
    schema_version = 1
    kind = 'portable-swap-transaction'
    transaction_id = $transactionId
    output_name = $OutputName
    release_mode = if ($DevelopmentSnapshot) { 'development_snapshot' } else { 'formal' }
    created_at_utc = [DateTime]::UtcNow.ToString('o')
    dist_root = $distRoot
    live_root = $liveRoot
    stage_root = $stageRoot
    backup_root = $backupRoot
    failed_root = $failedRoot
    backup_tombstone_root = Join-Path $distRoot ('.{0}.retired-backup-{1}' -f $OutputName, $transactionId)
    failed_tombstone_root = Join-Path $distRoot ('.{0}.retired-failed-{1}' -f $OutputName, $transactionId)
    status = 'initialized'
    smoke_status = 'pending'
    smoke_completed_at_utc = ''
    pre_user_data = ConvertTo-PortableTransactionMeasure $initialUserData
    staged_user_data = ConvertTo-PortableTransactionMeasure (Get-PortableTreeMeasure (Join-Path $stageRoot 'user_data'))
    staged_immutable_tree = ConvertTo-PortableTransactionMeasure (Get-PortableImmutableTreeMeasure $stageRoot)
    backup_tree = ConvertTo-PortableTransactionMeasure (Get-PortableTreeMeasure $backupRoot)
    backup_tombstone_tree = ConvertTo-PortableTransactionMeasure (Get-PortableTreeMeasure `
        (Join-Path $distRoot ('.{0}.retired-backup-{1}' -f $OutputName, $transactionId)))
    had_previous_live = [bool]$initialLiveExists
    swapped_at_utc = ''
    failure = ''
    failed_artifact_deleted = $false
    failed_artifact_deleted_at_utc = ''
    finalized_at_utc = ''
    backup_deleted = $false
}
$transactionWritten = $false
$stageCreated = $false

try {
    New-Item -ItemType Directory -Path $distRoot -Force | Out-Null
    Assert-NoPortableReparseAncestors $distRoot
    $transaction.status = 'staging'
    Write-PortableSwapTransaction -Path $transactionManifest -Transaction $transaction
    $transactionWritten = $true
    New-Item -ItemType Directory -Path $stageRoot | Out-Null
    $stageCreated = $true

    Copy-Item -LiteralPath $releaseExecutable -Destination (Join-Path $stageRoot 'AutoDesignMaker.exe')
    $stagedGroups = @(Copy-PortableResourceGroups -Groups $sourceGroups -StageRoot $stageRoot)
    Copy-Item -LiteralPath $sourceResourceManifest `
        -Destination (Join-Path $stageRoot 'knowledge\resource-manifest.json')
    Copy-Item -LiteralPath (Join-Path $portableToolsRoot 'Start-AutoDesignMaker.cmd') -Destination $stageRoot
    Copy-Item -LiteralPath (Join-Path $portableToolsRoot 'README.txt') -Destination $stageRoot

    if (Test-Path -LiteralPath $liveRoot -PathType Container) {
        Assert-PortableQuiescent $liveRoot
    }
    $userDataCopy = Copy-PortableUserData -SourceUserData (Join-Path $liveRoot 'user_data') `
        -StageUserData (Join-Path $stageRoot 'user_data') -CleanUserData:$CleanUserData
    $transaction.pre_user_data = ConvertTo-PortableTransactionMeasure $userDataCopy.Source
    $transaction.staged_user_data = ConvertTo-PortableTransactionMeasure $userDataCopy.Stage

    $portableResourceManifestPath = Join-Path $stageRoot 'portable-resource-manifest.json'
    Write-PortableJsonAtomic -Path $portableResourceManifestPath `
        -Value (New-PortableResourceManifestValue -Groups $stagedGroups)

    $manifestGitState = Get-PortableGitState -ProjectRoot $projectRoot -ResourceGroups $sourceGroups `
        -RequiredTrackedPaths $requiredTrackedPaths -DevelopmentSnapshot:$DevelopmentSnapshot
    if ([string]$manifestGitState.Commit -ne [string]$gitState.Commit) {
        throw 'Git HEAD changed while the portable build was running'
    }
    $gitState = $manifestGitState

    $stagedExecutable = Join-Path $stageRoot 'AutoDesignMaker.exe'
    $stagedLauncher = Join-Path $stageRoot 'Start-AutoDesignMaker.cmd'
    $stagedRegistry = Join-Path $stageRoot 'pipeline\artifact_layer\registry.json'
    $supportFiles = @(
        New-PortableFileEvidence -Root $stageRoot -RelativePath 'Start-AutoDesignMaker.cmd'
        New-PortableFileEvidence -Root $stageRoot -RelativePath 'README.txt'
        New-PortableFileEvidence -Root $stageRoot -RelativePath 'knowledge/resource-manifest.json'
    )
    $buildManifest = [ordered]@{
        schema_version = 1
        root_kind = 'portable-build-root'
        product = 'AutoDesignMaker NEWrust'
        version = [string]$tauriConfig.version
        built_at_utc = [DateTime]::UtcNow.ToString('o')
        release_mode = if ($DevelopmentSnapshot) { 'development_snapshot' } else { 'formal' }
        development_snapshot = [bool]$DevelopmentSnapshot
        transaction_id = $transactionId
        git_commit = [string]$gitState.Commit
        git_dirty = [bool]$gitState.Dirty
        git_tracked_resource_files = [int64]$gitState.TrackedResourceFiles
        target_triple = $targetTriple
        minimum_os = 'Windows 10 x64'
        webview2_required = $true
        crt_linkage = 'static-msvc'
        pe_machine = [string]$sourcePe.Machine
        pe_dependencies = @($sourcePe.Dependencies)
        dynamic_crt_dependencies = @($sourcePe.DynamicCrtDependencies)
        rust_version = $rustVersion.Trim()
        node_version = [string]$nodeTools.NodeVersion
        npm_version = [string]$nodeTools.NpmVersion
        cargo_lock_sha256 = (Get-FileHash -LiteralPath (Join-Path $projectRoot 'Cargo.lock') -Algorithm SHA256).Hash.ToLowerInvariant()
        npm_lock_sha256 = (Get-FileHash -LiteralPath (Join-Path $projectRoot 'web\package-lock.json') -Algorithm SHA256).Hash.ToLowerInvariant()
        cargo_config_sha256 = (Get-FileHash -LiteralPath (Join-Path $projectRoot '.cargo\config.toml') -Algorithm SHA256).Hash.ToLowerInvariant()
        executable = 'AutoDesignMaker.exe'
        executable_sha256 = (Get-FileHash -LiteralPath $stagedExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
        executable_bytes = [int64](Get-Item -LiteralPath $stagedExecutable).Length
        launcher = 'Start-AutoDesignMaker.cmd'
        launcher_sha256 = (Get-FileHash -LiteralPath $stagedLauncher -Algorithm SHA256).Hash.ToLowerInvariant()
        source_resource_manifest = 'knowledge/resource-manifest.json'
        source_resource_manifest_sha256 = (Get-FileHash -LiteralPath $sourceResourceManifest -Algorithm SHA256).Hash.ToLowerInvariant()
        resource_manifest = 'portable-resource-manifest.json'
        resource_manifest_sha256 = (Get-FileHash -LiteralPath $portableResourceManifestPath -Algorithm SHA256).Hash.ToLowerInvariant()
        artifact_registry = 'pipeline/artifact_layer/registry.json'
        artifact_registry_sha256 = (Get-FileHash -LiteralPath $stagedRegistry -Algorithm SHA256).Hash.ToLowerInvariant()
        portable_data_root = 'user_data'
        user_data_mode = [string]$userDataCopy.Mode
        user_data_files = [int64]$userDataCopy.Stage.FileCount
        user_data_bytes = [int64]$userDataCopy.Stage.Bytes
        user_data_digest = [string]$userDataCopy.Stage.Digest
        support_files = $supportFiles
    }
    Write-PortableJsonAtomic -Path (Join-Path $stageRoot 'build-manifest.json') -Value $buildManifest -Depth 16

    $stageValidation = Assert-PortableStage -StageRoot $stageRoot -ExpectedTransactionId $transactionId
    foreach ($binary in @(Get-PortableTreeFiles $stageRoot | Where-Object {
        $_.Extension -in @('.exe', '.dll')
    })) {
        $null = Get-PortablePeInspection -Executable $binary.FullName
    }
    $transaction.staged_immutable_tree = ConvertTo-PortableTransactionMeasure $stageValidation.ImmutableTree
    $transaction.status = 'stage_validated'
    Write-PortableSwapTransaction -Path $transactionManifest -Transaction $transaction

    Write-Host '==> Smoke validating the complete stage before swap'
    $smokeOutput = @(& $stagedExecutable --smoke 2>&1)
    if ($LASTEXITCODE -ne 0) {
        $transaction.smoke_status = 'failed'
        throw "portable stage smoke failed: $($smokeOutput -join [Environment]::NewLine)"
    }
    $null = Assert-PortableStage -StageRoot $stageRoot -ExpectedTransactionId $transactionId `
        -ExpectedImmutableMeasure $transaction.staged_immutable_tree
    $transaction.smoke_status = 'passed'
    $transaction.smoke_completed_at_utc = [DateTime]::UtcNow.ToString('o')
    $transaction.status = 'stage_smoke_passed'
    Write-PortableSwapTransaction -Path $transactionManifest -Transaction $transaction

    $preSwapGitState = Get-PortableGitState -ProjectRoot $projectRoot -ResourceGroups $sourceGroups `
        -RequiredTrackedPaths $requiredTrackedPaths -DevelopmentSnapshot:$DevelopmentSnapshot
    if ([string]$preSwapGitState.Commit -ne [string]$buildManifest.git_commit) {
        throw 'Git HEAD changed after portable stage validation'
    }

    $validation = { param($root) $null = Assert-PortableStage $root }
    $swapResult = Invoke-PortableSwapTransaction -DistRoot $distRoot -StageRoot $stageRoot `
        -LiveRoot $liveRoot -BackupRoot $backupRoot -FailedRoot $failedRoot `
        -TransactionManifest $transactionManifest -Transaction $transaction `
        -ValidateStage $validation -ValidateLive $validation -OperationLock $buildOperationLock
    $stageCreated = $false
}
catch {
    $originalError = $_
    try {
        if (Test-Path -LiteralPath $stageRoot -PathType Container) {
            if (Test-Path -LiteralPath $failedRoot) {
                throw "failed-artifact destination already exists: $failedRoot"
            }
            $stageBuildManifest = Join-Path $stageRoot 'build-manifest.json'
            if (Test-Path -LiteralPath $stageBuildManifest -PathType Leaf) {
                try {
                    $stageIdentity = Read-PortableJson $stageBuildManifest
                    if (([string]$stageIdentity.transaction_id).Equals(
                            $transactionId, [StringComparison]::OrdinalIgnoreCase)) {
                        $transaction.staged_immutable_tree = ConvertTo-PortableTransactionMeasure `
                            (Get-PortableImmutableTreeMeasure $stageRoot)
                    }
                }
                catch { Write-Warning "failed stage has no valid transaction-bound immutable credential" }
            }
            Move-Item -LiteralPath $stageRoot -Destination $failedRoot
            $stageCreated = $false
            $transaction.failed_root = $failedRoot
            $transaction.staged_user_data = ConvertTo-PortableTransactionMeasure `
                (Get-PortableTreeMeasure (Join-Path $failedRoot 'user_data'))
        }
        if ([string]$transaction.status -notmatch '^rollback_' -and
            [string]$transaction.status -ne 'swap_failed_before_install') {
            $transaction.status = 'stage_failed'
        }
        if ([string]$transaction.smoke_status -eq 'pending' -and
            $originalError.Exception.Message -match '(?i)smoke') {
            $transaction.smoke_status = 'failed'
        }
        $transaction.failure = $originalError.Exception.Message
        if (Test-Path -LiteralPath $failedRoot -PathType Container) {
            Write-PortableSwapTransaction -Path $transactionManifest -Transaction $transaction
            $transactionWritten = $true
            Write-Warning "failed portable artifact retained for diagnosis: $failedRoot"
            Write-Warning "cleanup dry run: powershell -ExecutionPolicy Bypass -File `"$portableToolsRoot\Remove-FailedPortableArtifact.ps1`" -TransactionManifest `"$transactionManifest`""
        }
        elseif ($transactionWritten -and (Test-Path -LiteralPath $transactionManifest -PathType Leaf)) {
            Write-PortableSwapTransaction -Path $transactionManifest -Transaction $transaction
        }
        if ([string]$transaction.status -ne 'rollback_failed' -and
            (Test-Path -LiteralPath $liveRoot -PathType Container)) {
            Remove-PortableUpdateLock $liveRoot
        }
    }
    catch {
        Write-Warning "portable failure bookkeeping also failed: $($_.Exception.Message)"
    }
    throw $originalError
}

$finalExecutable = Join-Path $liveRoot 'AutoDesignMaker.exe'
$finalHash = (Get-FileHash -LiteralPath $finalExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host ''
Write-Host "Portable build installed and awaiting transaction finalization: $liveRoot"
Write-Host "Executable SHA-256: $finalHash"
Write-Host "Transaction: $transactionManifest"
Write-Host 'Validate cleanup (dry run):'
Write-Host "  powershell -ExecutionPolicy Bypass -File `"$PSScriptRoot\Finalize-PortableSwap.ps1`" -TransactionManifest `"$transactionManifest`""
Write-Host 'Delete the verified backup and finalize the transaction:'
Write-Host "  powershell -ExecutionPolicy Bypass -File `"$PSScriptRoot\Finalize-PortableSwap.ps1`" -TransactionManifest `"$transactionManifest`" -Execute"
}
finally {
    Exit-PortableOutputOperationLock $buildOperationLock
}
