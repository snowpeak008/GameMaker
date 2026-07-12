Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'PortableBuildSupport.psm1')

$script:UpdateLockName = '.portable-update.lock'
$script:PathComparison = [System.StringComparison]::OrdinalIgnoreCase
$script:FinalizedReceiptRetentionCount = 5

function Assert-PortableDataStable {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [int] $DelayMilliseconds = 120
    )
    $first = Get-PortableTreeMeasure $Path
    Start-Sleep -Milliseconds $DelayMilliseconds
    $second = Get-PortableTreeMeasure $Path
    Assert-PortableMeasureEqual -Expected $first -Actual $second -Description "unstable data at $Path"
    $second
}

function Get-PortableProcesses {
    $results = New-Object 'System.Collections.Generic.List[object]'
    $cim = Get-Command Get-CimInstance -ErrorAction SilentlyContinue
    if ($null -ne $cim) {
        try {
            foreach ($process in @(Get-CimInstance Win32_Process -Filter "Name='AutoDesignMaker.exe'" -ErrorAction Stop)) {
                $results.Add([pscustomobject]@{ Id = $process.ProcessId; Path = [string]$process.ExecutablePath })
            }
            return $results.ToArray()
        }
        catch {
            # Fall through to the conservative process-name check.
        }
    }
    foreach ($process in @(Get-Process -Name AutoDesignMaker -ErrorAction SilentlyContinue)) {
        $path = ''
        try { $path = [string]$process.Path } catch { $path = '' }
        $results.Add([pscustomobject]@{ Id = $process.Id; Path = $path })
    }
    $results.ToArray()
}

function Assert-PortableDataLocksAvailable {
    param([Parameter(Mandatory = $true)][string] $UserDataRoot)
    if (-not (Test-Path -LiteralPath $UserDataRoot -PathType Container)) { return }
    foreach ($file in @(Get-PortableTreeFiles $UserDataRoot | Where-Object {
        $_.Name -like '*.lock' -or $_.Name -eq '.archive_lock'
    })) {
        $stream = $null
        try {
            $stream = [System.IO.File]::Open(
                $file.FullName,
                [System.IO.FileMode]::Open,
                [System.IO.FileAccess]::ReadWrite,
                [System.IO.FileShare]::None
            )
        }
        catch {
            throw "portable user data lock is held or inaccessible: $($file.FullName)"
        }
        finally {
            if ($null -ne $stream) { $stream.Dispose() }
        }
    }
}

function Assert-PortableQuiescent {
    param([Parameter(Mandatory = $true)][string] $LiveRoot)
    $running = @(Get-PortableProcesses)
    if ($running.Count -gt 0) {
        $details = @($running | ForEach-Object { "pid=$($_.Id) path=$($_.Path)" }) -join '; '
        throw "AutoDesignMaker must be fully stopped before portable update: $details"
    }
    Assert-PortableDataLocksAvailable (Join-Path $LiveRoot 'user_data')
}

function New-PortableUpdateLock {
    param(
        [Parameter(Mandatory = $true)][string] $Root,
        [Parameter(Mandatory = $true)][string] $TransactionId
    )
    if (-not (Test-Path -LiteralPath $Root -PathType Container)) { return }
    Write-PortableJsonAtomic -Path (Join-Path $Root $script:UpdateLockName) -Value ([ordered]@{
        schema_version = 1
        kind = 'portable-update-lock'
        transaction_id = $TransactionId
        created_at_utc = [DateTime]::UtcNow.ToString('o')
    })
}

function Remove-PortableUpdateLock {
    param([Parameter(Mandatory = $true)][string] $Root)
    $path = Join-Path $Root $script:UpdateLockName
    if (Test-Path -LiteralPath $path -PathType Leaf) { Remove-Item -LiteralPath $path -Force }
}

function Copy-PortableUserData {
    param(
        [Parameter(Mandatory = $true)][string] $SourceUserData,
        [Parameter(Mandatory = $true)][string] $StageUserData,
        [switch] $CleanUserData
    )
    $source = [System.IO.Path]::GetFullPath($SourceUserData)
    $target = [System.IO.Path]::GetFullPath($StageUserData)
    $sourceMeasure = Assert-PortableDataStable $source
    if ($CleanUserData -and $sourceMeasure.FileCount -gt 0) {
        throw "clean portable release refused to replace non-empty user_data: $source"
    }
    if (Test-Path -LiteralPath $target) {
        $existingTarget = Get-PortableTreeMeasure $target
        if ($existingTarget.FileCount -gt 0) { throw "stage user_data must be empty before copy: $target" }
        Remove-Item -LiteralPath $target -Recurse -Force
    }
    $parent = [System.IO.Directory]::GetParent($target)
    New-Item -ItemType Directory -Path $parent.FullName -Force | Out-Null
    if (-not $CleanUserData -and $sourceMeasure.Exists) {
        Copy-Item -LiteralPath $source -Destination $target -Recurse -Force
    }
    else {
        New-Item -ItemType Directory -Path $target -Force | Out-Null
    }
    $stageMeasure = Get-PortableTreeMeasure $target
    if ($CleanUserData) {
        if ($stageMeasure.FileCount -ne 0 -or $stageMeasure.Bytes -ne 0) {
            throw 'clean portable stage unexpectedly contains user_data'
        }
    }
    else {
        Assert-PortableMeasureEqual -Expected $sourceMeasure -Actual $stageMeasure `
            -Description 'portable user_data copy' -IgnoreExists
    }
    [pscustomobject]@{
        Mode = if ($CleanUserData) { 'clean_release' } else { 'preserved_local' }
        Source = $sourceMeasure
        Stage = $stageMeasure
    }
}

function Assert-PortableSwapPaths {
    param(
        [Parameter(Mandatory = $true)][string] $DistRoot,
        [Parameter(Mandatory = $true)][string[]] $Paths
    )
    $dist = ConvertTo-PortableFullPath $DistRoot
    Assert-NoPortableReparseAncestors $dist
    $seen = @{}
    foreach ($path in $Paths) {
        $full = ConvertTo-PortableFullPath $path
        if (-not (Test-PortablePathWithin -Path $full -Boundary $dist)) {
            throw "portable swap path escaped dist: $full"
        }
        if ($seen.ContainsKey($full.ToLowerInvariant())) { throw "duplicate portable swap path: $full" }
        $seen[$full.ToLowerInvariant()] = $true
        Assert-NoPortableReparseAncestors $full
    }
}

function Write-PortableSwapTransaction {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)] $Transaction
    )
    Write-PortableJsonAtomic -Path $Path -Value $Transaction -Depth 16
}

function Get-PortableTransactionPropertyValue {
    param(
        [Parameter(Mandatory = $true)] $Transaction,
        [Parameter(Mandatory = $true)][string] $Property
    )
    if ($Transaction -is [System.Collections.IDictionary]) { return $Transaction[$Property] }
    $member = $Transaction.PSObject.Properties[$Property]
    if ($null -eq $member) { throw "portable transaction is missing $Property" }
    $member.Value
}

function Set-PortableTransactionPropertyValue {
    param(
        [Parameter(Mandatory = $true)] $Transaction,
        [Parameter(Mandatory = $true)][string] $Property,
        [Parameter(Mandatory = $true)] $Value
    )
    if ($Transaction -is [System.Collections.IDictionary]) {
        $Transaction[$Property] = $Value
        return
    }
    $member = $Transaction.PSObject.Properties[$Property]
    if ($null -eq $member) {
        $Transaction | Add-Member -NotePropertyName $Property -NotePropertyValue $Value
    }
    else { $member.Value = $Value }
}

function Assert-PortableTransactionPathContract {
    param(
        [Parameter(Mandatory = $true)] $Transaction,
        [Parameter(Mandatory = $true)][string] $TransactionManifest
    )
    $id = [string]$Transaction.transaction_id
    $outputName = [string]$Transaction.output_name
    if ($id -notmatch '^[a-fA-F0-9]{32}$' -or $outputName -notmatch '^[A-Za-z0-9._-]+$') {
        throw 'portable transaction identity is invalid'
    }
    $dist = ConvertTo-PortableFullPath ([string]$Transaction.dist_root)
    $expected = [ordered]@{
        live_root = Join-Path $dist $outputName
        stage_root = Join-Path $dist ('.{0}.stage-{1}' -f $outputName, $id)
        backup_root = Join-Path $dist ('.{0}.previous-{1}' -f $outputName, $id)
        failed_root = Join-Path $dist ('.{0}.failed-{1}' -f $outputName, $id)
        backup_tombstone_root = Join-Path $dist ('.{0}.retired-backup-{1}' -f $outputName, $id)
        failed_tombstone_root = Join-Path $dist ('.{0}.retired-failed-{1}' -f $outputName, $id)
    }
    foreach ($property in @('backup_tombstone_root', 'failed_tombstone_root')) {
        if ($null -eq $Transaction.PSObject.Properties[$property] -and
            -not ($Transaction -is [System.Collections.IDictionary] -and $Transaction.Contains($property))) {
            Set-PortableTransactionPropertyValue -Transaction $Transaction -Property $property `
                -Value ([string]$expected[$property])
        }
    }
    foreach ($property in @($expected.Keys)) {
        $value = Get-PortableTransactionPropertyValue -Transaction $Transaction -Property $property
        $actual = ConvertTo-PortableFullPath ([string]$value)
        $wanted = ConvertTo-PortableFullPath ([string]$expected[$property])
        if (-not $actual.Equals($wanted, $script:PathComparison)) {
            throw "portable transaction $property does not match its identity: $actual"
        }
    }
    $wantedManifest = ConvertTo-PortableFullPath (Join-Path $dist ('.{0}.swap-{1}.json' -f $outputName, $id))
    $actualManifest = ConvertTo-PortableFullPath $TransactionManifest
    if (-not $actualManifest.Equals($wantedManifest, $script:PathComparison)) {
        throw "portable transaction manifest name does not match its identity: $actualManifest"
    }
}

function Get-PortableTransactionCompletionTime {
    param([Parameter(Mandatory = $true)] $Transaction)

    $status = [string]$Transaction.status
    $text = if ($status -eq 'finalized') {
        [string]$Transaction.finalized_at_utc
    }
    elseif ($status -eq 'failure_artifact_finalized') {
        [string]$Transaction.failed_artifact_deleted_at_utc
    }
    else {
        throw "portable transaction is not a finalized receipt: $status"
    }
    $parsed = [DateTimeOffset]::MinValue
    if (-not [DateTimeOffset]::TryParse(
            $text,
            [Globalization.CultureInfo]::InvariantCulture,
            [Globalization.DateTimeStyles]::RoundtripKind,
            [ref]$parsed)) {
        throw 'portable finalized receipt has an invalid completion timestamp'
    }
    if ($parsed -gt [DateTimeOffset]::UtcNow.AddMinutes(5)) {
        throw 'portable finalized receipt completion timestamp is unreasonably far in the future'
    }
    $parsed
}

function Assert-PortableOperationLockContract {
    param(
        [Parameter(Mandatory = $true)] $OperationLock,
        [Parameter(Mandatory = $true)] $Transaction
    )
    if ($null -eq $OperationLock.Stream -or -not $OperationLock.Stream.CanWrite -or
        -not ([string]$OperationLock.OutputName).Equals([string]$Transaction.output_name, $script:PathComparison) -or
        -not ([string]$OperationLock.TransactionId).Equals([string]$Transaction.transaction_id, $script:PathComparison)) {
        throw 'portable operation does not hold the matching exclusive output lock'
    }
}

function Assert-PortableTransactionCandidate {
    param(
        [Parameter(Mandatory = $true)][string] $Root,
        [Parameter(Mandatory = $true)] $Transaction,
        [Parameter(Mandatory = $true)][string] $Description
    )
    $expected = Get-PortableTransactionPropertyValue -Transaction $Transaction -Property 'staged_immutable_tree'
    if ([string]$expected.Digest -notmatch '^[a-fA-F0-9]{64}$') {
        throw 'portable transaction has no immutable candidate cleanup credential'
    }
    try {
        $buildManifest = Read-PortableJson (Join-Path $Root 'build-manifest.json')
        if (-not ([string]$buildManifest.transaction_id).Equals(
                [string]$Transaction.transaction_id, $script:PathComparison)) {
            throw 'build-manifest transaction_id mismatch'
        }
        $actual = Get-PortableImmutableTreeMeasure $Root
        Assert-PortableMeasureEqual -Expected $expected -Actual $actual -Description $Description
    }
    catch {
        throw "$Description failed transaction-bound candidate validation: $($_.Exception.Message)"
    }
}

function Invoke-PortableTransactionReceiptRetention {
    param(
        [Parameter(Mandatory = $true)][string] $TransactionManifest,
        [ValidateRange(1, 100)][int] $Retain = $script:FinalizedReceiptRetentionCount,
        [switch] $Execute
    )

    $currentPath = ConvertTo-PortableFullPath $TransactionManifest
    Assert-NoPortableReparseAncestors $currentPath
    $current = Read-PortableJson $currentPath
    if ([int]$current.schema_version -ne 1 -or $current.kind -ne 'portable-swap-transaction') {
        throw 'portable receipt retention requires a valid transaction manifest'
    }
    Assert-PortableTransactionPathContract -Transaction $current -TransactionManifest $currentPath
    $null = Get-PortableTransactionCompletionTime $current

    $dist = ConvertTo-PortableFullPath ([string]$current.dist_root)
    $outputName = [string]$current.output_name
    Assert-PortableSwapPaths -DistRoot $dist -Paths @($currentPath)
    $eligible = New-Object System.Collections.Generic.List[object]
    foreach ($file in @(Get-ChildItem -LiteralPath $dist -File -Force `
            -Filter ('.{0}.swap-*.json' -f $outputName) -ErrorAction Stop)) {
        $path = ConvertTo-PortableFullPath $file.FullName
        Assert-NoPortableReparseAncestors $path
        $record = Read-PortableJson $path
        if ([int]$record.schema_version -ne 1 -or $record.kind -ne 'portable-swap-transaction') {
            throw "portable receipt retention found an invalid transaction: $path"
        }
        Assert-PortableTransactionPathContract -Transaction $record -TransactionManifest $path
        if ([string]$record.output_name -ne $outputName) {
            throw "portable receipt retention found a mismatched output name: $path"
        }
        if ([string]$record.status -in @('finalized', 'failure_artifact_finalized')) {
            $eligible.Add([pscustomobject]@{
                    Path = $path
                    CompletedAt = Get-PortableTransactionCompletionTime $record
                })
        }
    }

    $ordered = @($eligible | Sort-Object -Property `
            @{ Expression = 'CompletedAt'; Descending = $true },
            @{ Expression = 'Path'; Descending = $true })
    $kept = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
    foreach ($entry in @($ordered | Select-Object -First $Retain)) {
        $null = $kept.Add([string]$entry.Path)
    }
    $prunable = @($ordered | Where-Object { -not $kept.Contains([string]$_.Path) })

    if ($Execute) {
        foreach ($entry in $prunable) {
            $path = [string]$entry.Path
            $recheck = Read-PortableJson $path
            Assert-PortableTransactionPathContract -Transaction $recheck -TransactionManifest $path
            $null = Get-PortableTransactionCompletionTime $recheck
            Remove-Item -LiteralPath $path -Force -ErrorAction Stop
        }
    }

    [pscustomobject]@{
        Status = if ($Execute) { 'receipt-retention-complete' } else { 'receipt-retention-dry-run' }
        Action = if ($prunable.Count -eq 0) { 'no-op' } elseif ($Execute) { 'old-receipts-pruned' } else { 'dry-run-prune-old-receipts' }
        EligibleCount = $ordered.Count
        RetainedCount = $ordered.Count - $prunable.Count
        PrunedCount = if ($Execute) { $prunable.Count } else { 0 }
        PlannedPruneCount = $prunable.Count
    }
}

function Invoke-PortableSwapTransactionCore {
    param(
        [Parameter(Mandatory = $true)][string] $DistRoot,
        [Parameter(Mandatory = $true)][string] $StageRoot,
        [Parameter(Mandatory = $true)][string] $LiveRoot,
        [Parameter(Mandatory = $true)][string] $BackupRoot,
        [Parameter(Mandatory = $true)][string] $FailedRoot,
        [Parameter(Mandatory = $true)][string] $TransactionManifest,
        [Parameter(Mandatory = $true)] $Transaction,
        [Parameter(Mandatory = $true)] $OperationLock,
        [Parameter(Mandatory = $true)][scriptblock] $ValidateStage,
        [Parameter(Mandatory = $true)][scriptblock] $ValidateLive,
        [scriptblock] $QuiescenceCheck = { param($root) Assert-PortableQuiescent $root }
    )
    if ([int]$Transaction.schema_version -ne 1 -or $Transaction.kind -ne 'portable-swap-transaction') {
        throw 'portable swap transaction is invalid'
    }
    Assert-PortableOperationLockContract -OperationLock $OperationLock -Transaction $Transaction
    $argumentPaths = [ordered]@{
        dist_root = $DistRoot
        stage_root = $StageRoot
        live_root = $LiveRoot
        backup_root = $BackupRoot
        failed_root = $FailedRoot
    }
    foreach ($property in @($argumentPaths.Keys)) {
        $argumentPath = ConvertTo-PortableFullPath ([string]$argumentPaths[$property])
        $transactionPath = ConvertTo-PortableFullPath ([string](Get-PortableTransactionPropertyValue `
            -Transaction $Transaction -Property $property))
        if (-not $argumentPath.Equals($transactionPath, $script:PathComparison)) {
            throw "portable swap argument $property differs from the transaction: $argumentPath"
        }
    }
    Assert-PortableTransactionPathContract -Transaction $Transaction -TransactionManifest $TransactionManifest
    Assert-PortableSwapPaths -DistRoot $DistRoot -Paths @(
        $StageRoot, $LiveRoot, $BackupRoot, $FailedRoot, $TransactionManifest
    )
    if (Test-Path -LiteralPath $BackupRoot -PathType Leaf) { throw "portable backup is not a directory: $BackupRoot" }
    if (Test-Path -LiteralPath $FailedRoot) { throw "portable failed replacement already exists: $FailedRoot" }
    $stageExists = Test-Path -LiteralPath $StageRoot -PathType Container
    $liveExists = Test-Path -LiteralPath $LiveRoot -PathType Container
    $backupExists = Test-Path -LiteralPath $BackupRoot -PathType Container
    if (-not $stageExists -and -not $liveExists) {
        throw 'portable topology has neither a staged nor installed candidate'
    }
    if ($stageExists) {
        & $ValidateStage $StageRoot
        Assert-PortableTransactionCandidate -Root $StageRoot -Transaction $Transaction -Description 'staged portable'
    }
    $hadPreviousLive = [bool]$Transaction.had_previous_live
    $oldMoved = $backupExists
    $newInstalled = -not $stageExists -and $liveExists
    if ($oldMoved) {
        Remove-PortableUpdateLock $BackupRoot
        if ($liveExists) {
            if (-not $newInstalled) { throw 'portable topology contains both old live and recovery backup' }
        }
        else {
            $backupData = Assert-PortableDataStable (Join-Path $BackupRoot 'user_data')
            Assert-PortableMeasureEqual -Expected $Transaction.pre_user_data -Actual $backupData `
                -Description 'reconciled recovery backup user_data'
        }
        if (-not [bool]$Transaction.backup_tree.Exists) {
            $Transaction.backup_tree = Assert-PortableDataStable $BackupRoot
        }
        $Transaction.status = 'backup_created'
        Write-PortableSwapTransaction -Path $TransactionManifest -Transaction $Transaction
    }
    elseif ($newInstalled) {
        Assert-PortableTransactionCandidate -Root $LiveRoot -Transaction $Transaction -Description 'installed portable'
    }
    else {
        $Transaction.status = 'stage_validated'
        Write-PortableSwapTransaction -Path $TransactionManifest -Transaction $Transaction
    }
    try {
        if ($liveExists) { & $QuiescenceCheck $LiveRoot }
        if ($hadPreviousLive -and -not $oldMoved) {
            if (-not $liveExists) { throw 'previous live portable is missing before backup creation' }
            New-PortableUpdateLock -Root $LiveRoot -TransactionId ([string]$Transaction.transaction_id)
            & $QuiescenceCheck $LiveRoot
            $current = Assert-PortableDataStable (Join-Path $LiveRoot 'user_data')
            Assert-PortableMeasureEqual -Expected $Transaction.pre_user_data -Actual $current `
                -Description 'pre-swap live user_data'
        }
        if (-not $newInstalled) {
            New-PortableUpdateLock -Root $StageRoot -TransactionId ([string]$Transaction.transaction_id)
        }
        if ($hadPreviousLive -and -not $oldMoved) {
            Move-Item -LiteralPath $LiveRoot -Destination $BackupRoot
            $oldMoved = $true
            Remove-PortableUpdateLock $BackupRoot
            $Transaction.backup_tree = Assert-PortableDataStable $BackupRoot
            $Transaction.status = 'backup_created'
            Write-PortableSwapTransaction -Path $TransactionManifest -Transaction $Transaction
        }
        if (-not $newInstalled) {
            Move-Item -LiteralPath $StageRoot -Destination $LiveRoot
            $newInstalled = $true
        }
        & $ValidateLive $LiveRoot
        Assert-PortableTransactionCandidate -Root $LiveRoot -Transaction $Transaction -Description 'installed portable'
        $liveMeasure = Assert-PortableDataStable (Join-Path $LiveRoot 'user_data')
        Assert-PortableMeasureEqual -Expected $Transaction.staged_user_data -Actual $liveMeasure `
            -Description 'installed portable user_data'
        Remove-PortableUpdateLock $LiveRoot
        $Transaction.status = if ($hadPreviousLive) { 'swapped_pending_finalize' } else { 'swapped_no_backup' }
        $Transaction.swapped_at_utc = [DateTime]::UtcNow.ToString('o')
        Write-PortableSwapTransaction -Path $TransactionManifest -Transaction $Transaction
        return [pscustomobject]@{
            Status = [string]$Transaction.status
            LiveRoot = ConvertTo-PortableFullPath $LiveRoot
            BackupRoot = if ($hadPreviousLive) { ConvertTo-PortableFullPath $BackupRoot } else { '' }
            TransactionManifest = ConvertTo-PortableFullPath $TransactionManifest
        }
    }
    catch {
        $failure = $_.Exception.Message
        $rollbackErrors = New-Object 'System.Collections.Generic.List[string]'
        $restored = $false
        if ($newInstalled -and (Test-Path -LiteralPath $LiveRoot -PathType Container)) {
            try {
                Move-Item -LiteralPath $LiveRoot -Destination $FailedRoot
            }
            catch {
                $rollbackErrors.Add("failed candidate could not be isolated: $($_.Exception.Message)")
            }
        }
        if ($oldMoved -and (Test-Path -LiteralPath $BackupRoot -PathType Container) -and
            -not (Test-Path -LiteralPath $LiveRoot)) {
            try {
                Move-Item -LiteralPath $BackupRoot -Destination $LiveRoot
                Remove-PortableUpdateLock $LiveRoot
                $restored = $true
            }
            catch {
                $rollbackErrors.Add("previous live could not be restored: $($_.Exception.Message)")
            }
        }
        elseif ($hadPreviousLive -and (Test-Path -LiteralPath $LiveRoot -PathType Container)) {
            try { Remove-PortableUpdateLock $LiveRoot } catch {
                $rollbackErrors.Add("previous live update lock could not be removed: $($_.Exception.Message)")
            }
        }
        $Transaction.status = if ($oldMoved) {
            if ($restored) { 'rollback_restored' } else { 'rollback_failed' }
        }
        else {
            'swap_failed_before_install'
        }
        if (Test-Path -LiteralPath $FailedRoot) {
            $Transaction.failed_root = ConvertTo-PortableFullPath $FailedRoot
        }
        $rollbackDetail = if ($rollbackErrors.Count -gt 0) { '; rollback=' + ($rollbackErrors -join '; ') } else { '' }
        $Transaction.failure = $failure + $rollbackDetail
        try {
            Write-PortableSwapTransaction -Path $TransactionManifest -Transaction $Transaction
        }
        catch {
            $rollbackDetail += "; transaction write failed: $($_.Exception.Message)"
        }
        throw "portable swap failed; status=$($Transaction.status); transaction=$TransactionManifest; failure=$failure$rollbackDetail"
    }
}

function Invoke-PortableSwapFinalizationCore {
    param(
        [Parameter(Mandatory = $true)][string] $TransactionManifest,
        [Parameter(Mandatory = $true)][scriptblock] $ValidateLive,
        [Parameter(Mandatory = $true)] $OperationLock,
        [scriptblock] $QuiescenceCheck = { param($root) Assert-PortableQuiescent $root },
        [scriptblock] $AfterBackupRetired,
        [scriptblock] $AfterBackupRemoved,
        [switch] $Execute
    )
    $manifestPath = ConvertTo-PortableFullPath $TransactionManifest
    Assert-NoPortableReparseAncestors $manifestPath
    $transaction = Read-PortableJson $manifestPath
    if ([int]$transaction.schema_version -ne 1 -or $transaction.kind -ne 'portable-swap-transaction') {
        throw 'portable swap transaction manifest is invalid'
    }
    Assert-PortableOperationLockContract -OperationLock $OperationLock -Transaction $transaction
    Assert-PortableTransactionPathContract -Transaction $transaction -TransactionManifest $manifestPath
    if ([string]$transaction.status -in @('stage_smoke_passed', 'backup_created')) {
        if ([string]$transaction.smoke_status -ne 'passed') {
            throw 'portable pre-swap recovery requires a recorded successful smoke result'
        }
        if (-not $Execute) {
            $candidate = if (Test-Path -LiteralPath ([string]$transaction.stage_root) -PathType Container) {
                [string]$transaction.stage_root
            }
            elseif (Test-Path -LiteralPath ([string]$transaction.live_root) -PathType Container) {
                [string]$transaction.live_root
            }
            else { throw 'portable pre-swap recovery has no candidate directory' }
            & $ValidateLive $candidate
            Assert-PortableTransactionCandidate -Root $candidate -Transaction $transaction `
                -Description 'pre-swap recovery candidate'
            return [pscustomobject]@{
                Status = 'ready_to_reconcile'
                Action = 'dry-run-resume-swap-then-finalize'
                BackupRoot = [string]$transaction.backup_root
            }
        }
        $null = Invoke-PortableSwapTransactionCore `
            -DistRoot ([string]$transaction.dist_root) -StageRoot ([string]$transaction.stage_root) `
            -LiveRoot ([string]$transaction.live_root) -BackupRoot ([string]$transaction.backup_root) `
            -FailedRoot ([string]$transaction.failed_root) -TransactionManifest $manifestPath `
            -Transaction $transaction -OperationLock $OperationLock -ValidateStage $ValidateLive `
            -ValidateLive $ValidateLive -QuiescenceCheck $QuiescenceCheck
        $transaction = Read-PortableJson $manifestPath
        Assert-PortableTransactionPathContract -Transaction $transaction -TransactionManifest $manifestPath
    }
    if ([string]$transaction.status -eq 'finalized') {
        $retention = Invoke-PortableTransactionReceiptRetention -TransactionManifest $manifestPath -Execute:$Execute
        return [pscustomobject]@{
            Status = 'finalized'
            Action = 'no-op'
            BackupRoot = [string]$transaction.backup_root
            ReceiptRetention = $retention
        }
    }
    if ([string]$transaction.smoke_status -ne 'passed') { throw 'portable swap was not smoke validated' }
    $startingStatus = [string]$transaction.status
    if ($startingStatus -notin @(
        'swapped_pending_finalize', 'swapped_no_backup', 'finalizing', 'finalizing_backup_retired'
    )) {
        throw "portable swap is not finalizable from status $($transaction.status)"
    }
    $dist = ConvertTo-PortableFullPath ([string]$transaction.dist_root)
    $live = ConvertTo-PortableFullPath ([string]$transaction.live_root)
    $backup = ConvertTo-PortableFullPath ([string]$transaction.backup_root)
    $backupTombstone = ConvertTo-PortableFullPath ([string]$transaction.backup_tombstone_root)
    Assert-PortableSwapPaths -DistRoot $dist -Paths @($live, $backup, $backupTombstone, $manifestPath)
    & $QuiescenceCheck $live
    & $ValidateLive $live
    Assert-PortableTransactionCandidate -Root $live -Transaction $transaction -Description 'final live portable'
    if (Test-Path -LiteralPath (Join-Path $live $script:UpdateLockName)) {
        throw 'live portable still has an update lock'
    }
    $liveMeasure = Assert-PortableDataStable (Join-Path $live 'user_data')
    Assert-PortableMeasureEqual -Expected $transaction.staged_user_data -Actual $liveMeasure `
        -Description 'final live user_data'
    $hasBackup = [bool]$transaction.had_previous_live
    if ($startingStatus -in @('finalizing', 'finalizing_backup_retired')) {
        if (-not $hasBackup -or -not [bool]$transaction.backup_tree.Exists) {
            throw 'portable finalizing transaction does not prove a previous recovery backup'
        }
        if ([bool]$transaction.backup_deleted) {
            throw 'portable finalizing transaction already claims its backup was deleted'
        }
    }
    $backupExists = Test-Path -LiteralPath $backup
    $backupExistedAtStart = $backupExists
    $tombstoneExists = Test-Path -LiteralPath $backupTombstone
    if ($backupExists -and $tombstoneExists) {
        throw 'portable topology contains both backup and its retired tombstone'
    }
    if ($backupExists -and -not (Test-Path -LiteralPath $backup -PathType Container)) {
        throw "portable backup is not a directory: $backup"
    }
    if ($hasBackup -and $backupExists) {
        $null = Get-PortableTreeFiles $backup
        $backupTree = Assert-PortableDataStable $backup
        Assert-PortableMeasureEqual -Expected $transaction.backup_tree -Actual $backupTree `
            -Description 'recovery backup tree'
        $backupMeasure = Assert-PortableDataStable (Join-Path $backup 'user_data')
        Assert-PortableMeasureEqual -Expected $transaction.pre_user_data -Actual $backupMeasure `
            -Description 'recovery backup user_data'
    }
    elseif ($hasBackup -and -not $tombstoneExists -and
        $startingStatus -notin @('finalizing', 'finalizing_backup_retired')) {
        throw "portable backup is missing before finalization began: $backup"
    }
    if ($hasBackup -and $tombstoneExists -and $startingStatus -eq 'finalizing') {
        $tombstoneTree = Assert-PortableDataStable $backupTombstone
        Assert-PortableMeasureEqual -Expected $transaction.backup_tree -Actual $tombstoneTree `
            -Description 'reconciled retired recovery backup'
        $transaction.backup_tombstone_tree = $tombstoneTree
        $transaction.status = 'finalizing_backup_retired'
        Write-PortableSwapTransaction -Path $manifestPath -Transaction $transaction
        $startingStatus = 'finalizing_backup_retired'
    }
    if (-not $Execute) {
        return [pscustomobject]@{
            Status = 'ready_to_finalize'
            Action = if ($hasBackup -and $backupExists) {
                if ($startingStatus -eq 'finalizing') { 'dry-run-resume-delete-backup' } else { 'dry-run-delete-backup' }
            }
            elseif ($hasBackup -and $tombstoneExists) { 'dry-run-resume-retired-backup-deletion' }
            elseif ($hasBackup) { 'dry-run-complete-interrupted-finalization' }
            else { 'dry-run-complete' }
            BackupRoot = if ($hasBackup) { $backup } else { '' }
        }
    }
    & $QuiescenceCheck $live
    & $ValidateLive $live
    Assert-PortableTransactionCandidate -Root $live -Transaction $transaction -Description 'rechecked final live portable'
    $liveMeasure = Assert-PortableDataStable (Join-Path $live 'user_data')
    Assert-PortableMeasureEqual -Expected $transaction.staged_user_data -Actual $liveMeasure `
        -Description 'rechecked final live user_data'
    if ($hasBackup -and $backupExists) {
        $backupTree = Assert-PortableDataStable $backup
        Assert-PortableMeasureEqual -Expected $transaction.backup_tree -Actual $backupTree `
            -Description 'rechecked recovery backup tree'
        $backupMeasure = Assert-PortableDataStable (Join-Path $backup 'user_data')
        Assert-PortableMeasureEqual -Expected $transaction.pre_user_data -Actual $backupMeasure `
            -Description 'rechecked recovery backup user_data'
        $transaction.status = 'finalizing'
        Write-PortableSwapTransaction -Path $manifestPath -Transaction $transaction
        Move-Item -LiteralPath $backup -Destination $backupTombstone
        $backupExists = $false
        $tombstoneExists = $true
        $transaction.backup_tombstone_tree = $backupTree
        $transaction.status = 'finalizing_backup_retired'
        Write-PortableSwapTransaction -Path $manifestPath -Transaction $transaction
        if ($null -ne $AfterBackupRetired) {
            & $AfterBackupRetired $backupTombstone $manifestPath
        }
    }
    if ($hasBackup -and $tombstoneExists) {
        Remove-Item -LiteralPath $backupTombstone -Recurse -Force
        if ($null -ne $AfterBackupRemoved) {
            & $AfterBackupRemoved $backupTombstone $manifestPath
        }
    }
    $transaction.status = 'finalized'
    $transaction.finalized_at_utc = [DateTime]::UtcNow.ToString('o')
    $transaction.backup_deleted = $hasBackup
    Write-PortableSwapTransaction -Path $manifestPath -Transaction $transaction
    $retention = Invoke-PortableTransactionReceiptRetention -TransactionManifest $manifestPath -Execute
    [pscustomobject]@{
        Status = 'finalized'
        Action = if ($hasBackup -and -not $backupExistedAtStart -and
            $startingStatus -in @('finalizing', 'finalizing_backup_retired')) {
            'completed-interrupted-finalization'
        }
        elseif ($hasBackup) { 'backup-deleted' }
        else { 'completed-no-backup' }
        BackupRoot = if ($hasBackup) { $backup } else { '' }
        ReceiptRetention = $retention
    }
}

function Invoke-PortableFailedArtifactCleanupCore {
    param(
        [Parameter(Mandatory = $true)][string] $TransactionManifest,
        [Parameter(Mandatory = $true)] $OperationLock,
        [scriptblock] $QuiescenceCheck = { param($root) Assert-PortableQuiescent $root },
        [scriptblock] $AfterFailedRetired,
        [switch] $Execute
    )
    $manifestPath = ConvertTo-PortableFullPath $TransactionManifest
    Assert-NoPortableReparseAncestors $manifestPath
    $transaction = Read-PortableJson $manifestPath
    if ([int]$transaction.schema_version -ne 1 -or $transaction.kind -ne 'portable-swap-transaction') {
        throw 'portable swap transaction manifest is invalid'
    }
    Assert-PortableOperationLockContract -OperationLock $OperationLock -Transaction $transaction
    Assert-PortableTransactionPathContract -Transaction $transaction -TransactionManifest $manifestPath
    if ([string]$transaction.status -eq 'failure_artifact_finalized') {
        $retention = Invoke-PortableTransactionReceiptRetention -TransactionManifest $manifestPath -Execute:$Execute
        return [pscustomobject]@{
            Status = 'failure_artifact_finalized'
            Action = 'no-op'
            FailedRoot = [string]$transaction.failed_root
            ReceiptRetention = $retention
        }
    }
    if ([string]$transaction.status -notin @(
        'staging', 'stage_validated', 'stage_failed', 'swap_failed_before_install',
        'rollback_restored', 'cleaning_failed_retired'
    )) {
        throw "portable failed artifact is not cleanable from status $($transaction.status)"
    }
    $dist = ConvertTo-PortableFullPath ([string]$transaction.dist_root)
    $live = ConvertTo-PortableFullPath ([string]$transaction.live_root)
    $backup = ConvertTo-PortableFullPath ([string]$transaction.backup_root)
    $stage = ConvertTo-PortableFullPath ([string]$transaction.stage_root)
    $failed = ConvertTo-PortableFullPath ([string]$transaction.failed_root)
    $failedTombstone = ConvertTo-PortableFullPath ([string]$transaction.failed_tombstone_root)
    Assert-PortableSwapPaths -DistRoot $dist -Paths @(
        $live, $stage, $backup, $failed, $failedTombstone, $manifestPath
    )
    if ([string]$transaction.status -eq 'staging') {
        if (-not (Test-Path -LiteralPath $stage -PathType Container)) {
            if (-not $Execute) {
                return [pscustomobject]@{
                    Status = 'ready_to_clean_failed_artifact'
                    Action = 'dry-run-finalize-empty-staging-receipt'
                    FailedRoot = $failed
                }
            }
            $transaction.status = 'failure_artifact_finalized'
            $transaction.failure = 'build stopped before a stage directory was created'
            $transaction.failed_artifact_deleted = $false
            $transaction.failed_artifact_deleted_at_utc = [DateTime]::UtcNow.ToString('o')
            Write-PortableSwapTransaction -Path $manifestPath -Transaction $transaction
            $retention = Invoke-PortableTransactionReceiptRetention `
                -TransactionManifest $manifestPath -Execute
            return [pscustomobject]@{
                Status = 'failure_artifact_finalized'
                Action = 'empty-staging-receipt-finalized'
                FailedRoot = $failed
                ReceiptRetention = $retention
            }
        }
        Assert-PortableTransactionCandidate -Root $stage -Transaction $transaction `
            -Description 'interrupted staging candidate'
        if (-not $Execute) {
            return [pscustomobject]@{
                Status = 'ready_to_clean_failed_artifact'
                Action = 'dry-run-retire-hash-bound-staging-candidate'
                FailedRoot = $failed
            }
        }
        Move-Item -LiteralPath $stage -Destination $failed
        $transaction.status = 'stage_failed'
        $transaction.failure = 'hash-bound staging candidate discarded after interruption'
        Write-PortableSwapTransaction -Path $manifestPath -Transaction $transaction
    }
    if ([string]$transaction.status -eq 'stage_validated') {
        if ([string]$transaction.smoke_status -eq 'passed') {
            throw 'smoke-passed stage must be recovered through finalization, not failed cleanup'
        }
        if (-not (Test-Path -LiteralPath $stage -PathType Container)) {
            throw 'validated pre-smoke stage is missing and cannot be safely reconciled'
        }
        Assert-PortableTransactionCandidate -Root $stage -Transaction $transaction `
            -Description 'validated pre-smoke stage'
        if (-not $Execute) {
            return [pscustomobject]@{
                Status = 'ready_to_clean_failed_artifact'
                Action = 'dry-run-retire-validated-pre-smoke-stage'
                FailedRoot = $failed
            }
        }
        if (Test-Path -LiteralPath $failed) { throw 'failed artifact destination already exists' }
        Move-Item -LiteralPath $stage -Destination $failed
        $transaction.status = 'stage_failed'
        $transaction.failure = 'validated candidate discarded before a recorded successful smoke result'
        Write-PortableSwapTransaction -Path $manifestPath -Transaction $transaction
    }
    $failedExists = Test-Path -LiteralPath $failed -PathType Container
    $tombstoneExists = Test-Path -LiteralPath $failedTombstone -PathType Container
    if ($failedExists -and $tombstoneExists) {
        throw 'portable topology contains both failed artifact and its retired tombstone'
    }
    if (-not $failedExists -and -not $tombstoneExists -and
        [string]$transaction.status -ne 'cleaning_failed_retired') {
        throw "portable failed artifact is missing: $failed"
    }
    if (Test-Path -LiteralPath $backup) {
        throw "recovery backup still exists and cannot be handled by failed-artifact cleanup: $backup"
    }
    & $QuiescenceCheck $live
    if ($failedExists) {
        Assert-PortableTransactionCandidate -Root $failed -Transaction $transaction -Description 'failed portable artifact'
        $failedData = Assert-PortableDataStable (Join-Path $failed 'user_data')
        Assert-PortableMeasureEqual -Expected $transaction.staged_user_data -Actual $failedData `
            -Description 'failed portable artifact user_data'
    }
    elseif ($tombstoneExists -and [string]$transaction.status -ne 'cleaning_failed_retired') {
        Assert-PortableTransactionCandidate -Root $failedTombstone -Transaction $transaction `
            -Description 'retired failed portable artifact'
        $transaction.status = 'cleaning_failed_retired'
        Write-PortableSwapTransaction -Path $manifestPath -Transaction $transaction
    }
    if ([bool]$transaction.had_previous_live) {
        if (-not (Test-Path -LiteralPath $live -PathType Container)) {
            throw "previous portable live root was not restored: $live"
        }
        $liveData = Assert-PortableDataStable (Join-Path $live 'user_data')
        Assert-PortableMeasureEqual -Expected $transaction.pre_user_data -Actual $liveData `
            -Description 'restored live user_data'
    }
    elseif (Test-Path -LiteralPath $live) {
        throw "unexpected live root exists for a failed first installation: $live"
    }
    if (-not $Execute) {
        return [pscustomobject]@{
            Status = 'ready_to_clean_failed_artifact'
            Action = 'dry-run-delete-failed-artifact'
            FailedRoot = $failed
        }
    }
    & $QuiescenceCheck $live
    if ($failedExists) {
        Assert-PortableTransactionCandidate -Root $failed -Transaction $transaction `
            -Description 'rechecked failed portable artifact'
        $failedData = Assert-PortableDataStable (Join-Path $failed 'user_data')
        Assert-PortableMeasureEqual -Expected $transaction.staged_user_data -Actual $failedData `
            -Description 'rechecked failed portable artifact user_data'
        Move-Item -LiteralPath $failed -Destination $failedTombstone
        $failedExists = $false
        $tombstoneExists = $true
        $transaction.status = 'cleaning_failed_retired'
        Write-PortableSwapTransaction -Path $manifestPath -Transaction $transaction
        if ($null -ne $AfterFailedRetired) { & $AfterFailedRetired $failedTombstone $manifestPath }
    }
    if ($tombstoneExists) {
        Remove-Item -LiteralPath $failedTombstone -Recurse -Force
    }
    $transaction.status = 'failure_artifact_finalized'
    $transaction.failed_artifact_deleted = $true
    $transaction.failed_artifact_deleted_at_utc = [DateTime]::UtcNow.ToString('o')
    Write-PortableSwapTransaction -Path $manifestPath -Transaction $transaction
    $retention = Invoke-PortableTransactionReceiptRetention -TransactionManifest $manifestPath -Execute
    [pscustomobject]@{
        Status = 'failure_artifact_finalized'
        Action = 'failed-artifact-deleted'
        FailedRoot = $failed
        ReceiptRetention = $retention
    }
}

function Invoke-PortableSwapTransaction {
    param(
        [Parameter(Mandatory = $true)][string] $DistRoot,
        [Parameter(Mandatory = $true)][string] $StageRoot,
        [Parameter(Mandatory = $true)][string] $LiveRoot,
        [Parameter(Mandatory = $true)][string] $BackupRoot,
        [Parameter(Mandatory = $true)][string] $FailedRoot,
        [Parameter(Mandatory = $true)][string] $TransactionManifest,
        [Parameter(Mandatory = $true)] $Transaction,
        [Parameter(Mandatory = $true)][scriptblock] $ValidateStage,
        [Parameter(Mandatory = $true)][scriptblock] $ValidateLive,
        [scriptblock] $QuiescenceCheck = { param($root) Assert-PortableQuiescent $root },
        $OperationLock = $null
    )
    $ownedLock = $null
    if ($null -eq $OperationLock) {
        $ownedLock = Enter-PortableOutputOperationLock -DistRoot $DistRoot `
            -OutputName ([string]$Transaction.output_name) `
            -TransactionId ([string]$Transaction.transaction_id) -Purpose 'swap'
        $OperationLock = $ownedLock
    }
    try {
        Invoke-PortableSwapTransactionCore -DistRoot $DistRoot -StageRoot $StageRoot `
            -LiveRoot $LiveRoot -BackupRoot $BackupRoot -FailedRoot $FailedRoot `
            -TransactionManifest $TransactionManifest -Transaction $Transaction `
            -OperationLock $OperationLock -ValidateStage $ValidateStage -ValidateLive $ValidateLive `
            -QuiescenceCheck $QuiescenceCheck
    }
    finally {
        if ($null -ne $ownedLock) { Exit-PortableOutputOperationLock $ownedLock }
    }
}

function Invoke-PortableSwapFinalization {
    param(
        [Parameter(Mandatory = $true)][string] $TransactionManifest,
        [Parameter(Mandatory = $true)][scriptblock] $ValidateLive,
        [scriptblock] $QuiescenceCheck = { param($root) Assert-PortableQuiescent $root },
        [scriptblock] $AfterBackupRetired,
        [scriptblock] $AfterBackupRemoved,
        [switch] $Execute
    )
    $manifestPath = ConvertTo-PortableFullPath $TransactionManifest
    $transaction = Read-PortableJson $manifestPath
    $operationLock = Enter-PortableOutputOperationLock -DistRoot ([string]$transaction.dist_root) `
        -OutputName ([string]$transaction.output_name) `
        -TransactionId ([string]$transaction.transaction_id) -Purpose 'finalize'
    try {
        Invoke-PortableSwapFinalizationCore -TransactionManifest $manifestPath `
            -ValidateLive $ValidateLive -OperationLock $operationLock `
            -QuiescenceCheck $QuiescenceCheck -AfterBackupRetired $AfterBackupRetired `
            -AfterBackupRemoved $AfterBackupRemoved -Execute:$Execute
    }
    finally { Exit-PortableOutputOperationLock $operationLock }
}

function Invoke-PortableFailedArtifactCleanup {
    param(
        [Parameter(Mandatory = $true)][string] $TransactionManifest,
        [scriptblock] $QuiescenceCheck = { param($root) Assert-PortableQuiescent $root },
        [scriptblock] $AfterFailedRetired,
        [switch] $Execute
    )
    $manifestPath = ConvertTo-PortableFullPath $TransactionManifest
    $transaction = Read-PortableJson $manifestPath
    $operationLock = Enter-PortableOutputOperationLock -DistRoot ([string]$transaction.dist_root) `
        -OutputName ([string]$transaction.output_name) `
        -TransactionId ([string]$transaction.transaction_id) -Purpose 'failed-cleanup'
    try {
        Invoke-PortableFailedArtifactCleanupCore -TransactionManifest $manifestPath `
            -OperationLock $operationLock -QuiescenceCheck $QuiescenceCheck `
            -AfterFailedRetired $AfterFailedRetired -Execute:$Execute
    }
    finally { Exit-PortableOutputOperationLock $operationLock }
}

Export-ModuleMember -Function @(
    'Assert-PortableDataStable',
    'Assert-PortableDataLocksAvailable',
    'Assert-PortableQuiescent',
    'New-PortableUpdateLock',
    'Remove-PortableUpdateLock',
    'Copy-PortableUserData',
    'Assert-PortableSwapPaths',
    'Write-PortableSwapTransaction',
    'Invoke-PortableTransactionReceiptRetention',
    'Invoke-PortableSwapTransaction',
    'Invoke-PortableSwapFinalization',
    'Invoke-PortableFailedArtifactCleanup'
)
