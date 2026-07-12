[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'PortableBuildSupport.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'PortableSwap.psm1') -Force

$fixtureRoot = Join-Path ([System.IO.Path]::GetTempPath()) `
    ('.adm-portable-fixture-{0}' -f [guid]::NewGuid().ToString('N'))
$script:Passed = 0

function Write-FixtureText {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string] $Value
    )
    $parent = [System.IO.Directory]::GetParent([System.IO.Path]::GetFullPath($Path))
    New-Item -ItemType Directory -Path $parent.FullName -Force | Out-Null
    [System.IO.File]::WriteAllText($Path, $Value, [System.Text.UTF8Encoding]::new($false))
}

function Assert-Fixture {
    param(
        [Parameter(Mandatory = $true)][bool] $Condition,
        [Parameter(Mandatory = $true)][string] $Message
    )
    if (-not $Condition) { throw $Message }
}

function Assert-FixtureThrows {
    param(
        [Parameter(Mandatory = $true)][scriptblock] $Body,
        [Parameter(Mandatory = $true)][string] $Message
    )
    $didThrow = $false
    try { & $Body } catch { $didThrow = $true }
    if (-not $didThrow) { throw $Message }
}

function Run-FixtureTest {
    param(
        [Parameter(Mandatory = $true)][string] $Name,
        [Parameter(Mandatory = $true)][scriptblock] $Body
    )
    Write-Host "TEST $Name"
    & $Body
    $script:Passed += 1
    Write-Host "PASS $Name"
}

function New-FixtureResourceSource {
    param([Parameter(Mandatory = $true)][string] $Root)
    $paths = @(
        'knowledge/design_data',
        'knowledge/schemas',
        'knowledge/market_data',
        'knowledge/sdks',
        'knowledge/skills',
        'pipeline/artifact_layer'
    )
    $groups = @()
    foreach ($relative in $paths) {
        $fileName = if ($relative -eq 'pipeline/artifact_layer') { 'registry.json' } else { 'seed.txt' }
        Write-FixtureText -Path (Join-Path (Join-Path $Root $relative) $fileName) -Value "fixture:$relative"
        $measure = Get-PortableTreeMeasure (Join-Path $Root $relative)
        $groups += [ordered]@{
            path = $relative
            files = [int64]$measure.FileCount
            bytes = [int64]$measure.Bytes
            treeSha256 = [string]$measure.Digest
            mode = if ($relative -in @('knowledge/market_data', 'knowledge/sdks', 'knowledge/skills')) {
                'seed-read-only'
            }
            else {
                'required-read-only'
            }
        }
    }
    $manifestPath = Join-Path $Root 'knowledge\resource-manifest.json'
    Write-PortableJsonAtomic -Path $manifestPath -Value ([ordered]@{
        schemaVersion = 1
        projectId = 'autodesignmaker-rust-v2'
        generatedFrom = 'portable fixture'
        groups = @($groups)
    })
    @(Read-PortableSourceResourceGroups -ProjectRoot $Root -ManifestPath $manifestPath)
}

function Get-FixtureFileEvidence {
    param(
        [Parameter(Mandatory = $true)][string] $Root,
        [Parameter(Mandatory = $true)][string] $RelativePath
    )
    $path = Join-Path $Root $RelativePath
    [ordered]@{
        path = $RelativePath.Replace('\', '/')
        bytes = [int64](Get-Item -LiteralPath $path).Length
        sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

function New-FixturePortableStage {
    param(
        [Parameter(Mandatory = $true)][string] $StageRoot,
        [Parameter(Mandatory = $true)][string] $SourceRoot,
        [Parameter(Mandatory = $true)][object[]] $SourceGroups
    )
    New-Item -ItemType Directory -Path $StageRoot -Force | Out-Null
    Write-FixtureText -Path (Join-Path $StageRoot 'AutoDesignMaker.exe') -Value 'MZ-fixture'
    $stagedGroups = @(Copy-PortableResourceGroups -Groups $SourceGroups -StageRoot $StageRoot)
    Copy-Item -LiteralPath (Join-Path $SourceRoot 'knowledge\resource-manifest.json') `
        -Destination (Join-Path $StageRoot 'knowledge\resource-manifest.json')
    Write-FixtureText -Path (Join-Path $StageRoot 'Start-AutoDesignMaker.cmd') -Value '@echo fixture'
    Write-FixtureText -Path (Join-Path $StageRoot 'README.txt') -Value 'fixture portable readme'
    New-Item -ItemType Directory -Path (Join-Path $StageRoot 'user_data') -Force | Out-Null
    $portableResourcePath = Join-Path $StageRoot 'portable-resource-manifest.json'
    Write-PortableJsonAtomic -Path $portableResourcePath `
        -Value (New-PortableResourceManifestValue -Groups $stagedGroups)
    $exe = Join-Path $StageRoot 'AutoDesignMaker.exe'
    $launcher = Join-Path $StageRoot 'Start-AutoDesignMaker.cmd'
    $registry = Join-Path $StageRoot 'pipeline\artifact_layer\registry.json'
    $userData = Get-PortableTreeMeasure (Join-Path $StageRoot 'user_data')
    $support = @(
        Get-FixtureFileEvidence -Root $StageRoot -RelativePath 'Start-AutoDesignMaker.cmd'
        Get-FixtureFileEvidence -Root $StageRoot -RelativePath 'README.txt'
        Get-FixtureFileEvidence -Root $StageRoot -RelativePath 'knowledge/resource-manifest.json'
    )
    Write-PortableJsonAtomic -Path (Join-Path $StageRoot 'build-manifest.json') -Depth 12 -Value ([ordered]@{
        schema_version = 1
        root_kind = 'portable-build-root'
        executable = 'AutoDesignMaker.exe'
        executable_sha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
        executable_bytes = [int64](Get-Item -LiteralPath $exe).Length
        launcher = 'Start-AutoDesignMaker.cmd'
        launcher_sha256 = (Get-FileHash -LiteralPath $launcher -Algorithm SHA256).Hash.ToLowerInvariant()
        source_resource_manifest_sha256 = (Get-FileHash -LiteralPath (Join-Path $SourceRoot 'knowledge\resource-manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant()
        resource_manifest_sha256 = (Get-FileHash -LiteralPath $portableResourcePath -Algorithm SHA256).Hash.ToLowerInvariant()
        artifact_registry = 'pipeline/artifact_layer/registry.json'
        artifact_registry_sha256 = (Get-FileHash -LiteralPath $registry -Algorithm SHA256).Hash.ToLowerInvariant()
        user_data_files = [int64]$userData.FileCount
        user_data_bytes = [int64]$userData.Bytes
        user_data_digest = [string]$userData.Digest
        support_files = $support
    })
    $null = Assert-PortableStage $StageRoot
}

function New-FixtureTransaction {
    param(
        [Parameter(Mandatory = $true)][string] $Id,
        [Parameter(Mandatory = $true)][string] $DistRoot,
        [Parameter(Mandatory = $true)][string] $LiveRoot,
        [Parameter(Mandatory = $true)][string] $StageRoot,
        [Parameter(Mandatory = $true)][string] $BackupRoot,
        [Parameter(Mandatory = $true)][string] $FailedRoot
    )
    if (Test-Path -LiteralPath $StageRoot -PathType Container) {
        $buildManifestPath = Join-Path $StageRoot 'build-manifest.json'
        $buildManifest = if (Test-Path -LiteralPath $buildManifestPath -PathType Leaf) {
            Read-PortableJson $buildManifestPath
        }
        else { [pscustomobject]@{} }
        $buildManifest | Add-Member -NotePropertyName transaction_id -NotePropertyValue $Id -Force
        Write-PortableJsonAtomic -Path $buildManifestPath -Value $buildManifest
    }
    $pre = Get-PortableTreeMeasure (Join-Path $LiveRoot 'user_data')
    $staged = Get-PortableTreeMeasure (Join-Path $StageRoot 'user_data')
    $immutable = Get-PortableImmutableTreeMeasure $StageRoot
    [ordered]@{
        schema_version = 1
        kind = 'portable-swap-transaction'
        transaction_id = $Id
        output_name = 'fixture'
        release_mode = 'fixture'
        created_at_utc = [DateTime]::UtcNow.ToString('o')
        dist_root = $DistRoot
        live_root = $LiveRoot
        stage_root = $StageRoot
        backup_root = $BackupRoot
        failed_root = $FailedRoot
        backup_tombstone_root = Join-Path $DistRoot ".fixture.retired-backup-$Id"
        failed_tombstone_root = Join-Path $DistRoot ".fixture.retired-failed-$Id"
        status = 'stage_smoke_passed'
        smoke_status = 'passed'
        smoke_completed_at_utc = [DateTime]::UtcNow.ToString('o')
        pre_user_data = [ordered]@{ Exists = $pre.Exists; FileCount = $pre.FileCount; Bytes = $pre.Bytes; Digest = $pre.Digest }
        staged_user_data = [ordered]@{ Exists = $staged.Exists; FileCount = $staged.FileCount; Bytes = $staged.Bytes; Digest = $staged.Digest }
        staged_immutable_tree = [ordered]@{ Exists = $immutable.Exists; FileCount = $immutable.FileCount; Bytes = $immutable.Bytes; Digest = $immutable.Digest }
        backup_tree = [ordered]@{ Exists = $false; FileCount = 0; Bytes = 0; Digest = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855' }
        backup_tombstone_tree = [ordered]@{ Exists = $false; FileCount = 0; Bytes = 0; Digest = 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855' }
        had_previous_live = [bool](Test-Path -LiteralPath $LiveRoot -PathType Container)
        swapped_at_utc = ''
        failure = ''
        failed_artifact_deleted = $false
        failed_artifact_deleted_at_utc = ''
        finalized_at_utc = ''
        backup_deleted = $false
    }
}

function Invoke-FixtureGit {
    param(
        [Parameter(Mandatory = $true)][string] $Root,
        [Parameter(Mandatory = $true)][string[]] $Arguments
    )
    & git -C $Root @Arguments | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "fixture git command failed: git $($Arguments -join ' ')" }
}

try {
    New-Item -ItemType Directory -Path $fixtureRoot | Out-Null

    Run-FixtureTest 'missing dist and absent user_data normalize to the empty-tree digest' {
        $root = Join-Path $fixtureRoot 'no-dist'
        $dist = Join-Path $root 'dist'
        Assert-Fixture (-not (Test-Path -LiteralPath $dist)) 'fixture unexpectedly started with dist'
        $copy = Copy-PortableUserData -SourceUserData (Join-Path $dist 'live\user_data') `
            -StageUserData (Join-Path $dist 'stage\user_data')
        Assert-Fixture (-not $copy.Source.Exists) 'missing source must remain distinguishable as absent'
        Assert-Fixture $copy.Stage.Exists 'stage user_data must always exist'
        Assert-Fixture ($copy.Source.Digest -eq 'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855') `
            'absent data did not use the canonical empty-tree digest'
        Assert-Fixture ($copy.Source.Digest -eq $copy.Stage.Digest) 'absent and staged-empty digest mismatch'
    }

    Run-FixtureTest 'empty user_data preserves exact zero-file evidence' {
        $root = Join-Path $fixtureRoot 'empty'
        New-Item -ItemType Directory -Path (Join-Path $root 'source') -Force | Out-Null
        $copy = Copy-PortableUserData -SourceUserData (Join-Path $root 'source') `
            -StageUserData (Join-Path $root 'stage\user_data')
        Assert-Fixture $copy.Source.Exists 'empty source must be recorded as existing'
        Assert-Fixture ($copy.Stage.FileCount -eq 0) 'empty stage contains files'
        Assert-Fixture ($copy.Source.Digest -eq $copy.Stage.Digest) 'empty-tree evidence changed during copy'
    }

    Run-FixtureTest 'non-empty user_data preserves nested files exactly' {
        $root = Join-Path $fixtureRoot 'nonempty'
        Write-FixtureText -Path (Join-Path $root 'source\nested\save.json') -Value '{"save":1}'
        Write-FixtureText -Path (Join-Path $root 'source\settings.txt') -Value 'settings'
        $copy = Copy-PortableUserData -SourceUserData (Join-Path $root 'source') `
            -StageUserData (Join-Path $root 'stage\user_data')
        Assert-Fixture ($copy.Source.FileCount -eq 2) 'non-empty source file count is wrong'
        Assert-Fixture (Test-PortableMeasureEqual $copy.Source $copy.Stage) 'non-empty data evidence changed'
    }

    Run-FixtureTest 'Clean refuses non-empty user_data without modifying it' {
        $root = Join-Path $fixtureRoot 'clean-refusal'
        $source = Join-Path $root 'source'
        Write-FixtureText -Path (Join-Path $source 'important.save') -Value 'do-not-delete'
        $before = Get-PortableTreeMeasure $source
        Assert-FixtureThrows {
            Copy-PortableUserData -SourceUserData $source -StageUserData (Join-Path $root 'stage\user_data') `
                -CleanUserData
        } 'Clean accepted non-empty user_data'
        $after = Get-PortableTreeMeasure $source
        Assert-Fixture (Test-PortableMeasureEqual $before $after) 'Clean refusal modified source data'
    }

    Run-FixtureTest 'resource staging validates exact source groups and rejects tampering' {
        $source = Join-Path $fixtureRoot 'resource-source'
        $groups = @(New-FixtureResourceSource $source)
        $stage = Join-Path $fixtureRoot 'resource-stage'
        New-FixturePortableStage -StageRoot $stage -SourceRoot $source -SourceGroups $groups
        $relocated = Join-Path $fixtureRoot 'relocated portable folder'
        Move-Item -LiteralPath $stage -Destination $relocated
        Move-Item -LiteralPath $source -Destination (Join-Path $fixtureRoot 'source-moved-away')
        $stage = $relocated
        $null = Assert-PortableStage $stage
        Write-FixtureText -Path (Join-Path $stage 'knowledge\schemas\seed.txt') -Value 'tampered'
        Assert-FixtureThrows { $null = Assert-PortableStage $stage } `
            'tampered resource passed portable stage validation'
    }

    Run-FixtureTest 'formal Git gate rejects dirty and untracked state' {
        if (-not (Get-Command git -ErrorAction SilentlyContinue)) { throw 'git is required for this fixture' }
        $root = Join-Path $fixtureRoot 'git-gate'
        New-Item -ItemType Directory -Path $root -Force | Out-Null
        Invoke-FixtureGit $root @('init', '--quiet')
        Invoke-FixtureGit $root @('config', 'user.email', 'portable-fixture@example.invalid')
        Invoke-FixtureGit $root @('config', 'user.name', 'Portable Fixture')
        Write-FixtureText -Path (Join-Path $root 'required.txt') -Value 'required'
        Write-FixtureText -Path (Join-Path $root 'resources\tracked.txt') -Value 'tracked'
        Invoke-FixtureGit $root @('add', '--', 'required.txt', 'resources/tracked.txt')
        Invoke-FixtureGit $root @('commit', '--quiet', '-m', 'fixture')
        $measure = Get-PortableTreeMeasure (Join-Path $root 'resources')
        $group = [pscustomobject]@{ Path = 'resources'; Measure = $measure }
        $clean = Get-PortableGitState -ProjectRoot $root -ResourceGroups @($group) `
            -RequiredTrackedPaths @('required.txt')
        Assert-Fixture (-not $clean.Dirty) 'clean fixture Git state was reported dirty'
        Write-FixtureText -Path (Join-Path $root 'untracked.txt') -Value 'untracked'
        Assert-FixtureThrows {
            $null = Get-PortableGitState -ProjectRoot $root -ResourceGroups @($group) `
                -RequiredTrackedPaths @('required.txt')
        } 'formal Git gate accepted untracked content'
        $development = Get-PortableGitState -ProjectRoot $root -ResourceGroups @($group) `
            -RequiredTrackedPaths @('required.txt') -DevelopmentSnapshot
        Assert-Fixture $development.Dirty 'development snapshot did not record dirty Git state'
        $nonRepository = Join-Path $fixtureRoot 'git-development-snapshot-without-repository'
        Write-FixtureText -Path (Join-Path $nonRepository 'resources\seed.txt') -Value 'seed'
        $nonRepoGroup = [pscustomobject]@{
            Path = 'resources'
            Measure = Get-PortableTreeMeasure (Join-Path $nonRepository 'resources')
        }
        Assert-FixtureThrows {
            $null = Get-PortableGitState -ProjectRoot $nonRepository -ResourceGroups @($nonRepoGroup) `
                -RequiredTrackedPaths @('required.txt')
        } 'formal Git gate accepted a non-repository source'
        $nonRepoDevelopment = Get-PortableGitState -ProjectRoot $nonRepository `
            -ResourceGroups @($nonRepoGroup) -RequiredTrackedPaths @('required.txt') -DevelopmentSnapshot
        Assert-Fixture $nonRepoDevelopment.Dirty 'development snapshot did not opt out of unavailable Git state'
    }

    Run-FixtureTest 'PE checks require x64 and reject dynamic CRT imports' {
        $root = Join-Path $fixtureRoot 'pe'
        $exe = Join-Path $root 'fixture.exe'
        Write-FixtureText -Path $exe -Value 'MZ'
        $goodDumpbin = Join-Path $root 'dumpbin-good.cmd'
        Write-FixtureText -Path $goodDumpbin -Value "@echo off`r`nif /I `"%2`"==`"/headers`" goto headers`r`necho     KERNEL32.dll`r`ngoto end`r`n:headers`r`necho 8664 machine ^(x64^)`r`n:end`r`n"
        $inspection = Get-PortablePeInspection -Executable $exe -DumpbinPath $goodDumpbin
        Assert-Fixture ($inspection.Machine -eq 'x86_64') 'x64 fixture was not accepted'
        $badDumpbin = Join-Path $root 'dumpbin-bad.cmd'
        Write-FixtureText -Path $badDumpbin -Value "@echo off`r`nif /I `"%2`"==`"/headers`" goto headers`r`necho     VCRUNTIME140.dll`r`ngoto end`r`n:headers`r`necho 8664 machine ^(x64^)`r`n:end`r`n"
        Assert-FixtureThrows { $null = Get-PortablePeInspection -Executable $exe -DumpbinPath $badDumpbin } `
            'dynamic CRT import passed PE inspection'
        Assert-Fixture (Test-PortableDynamicCrtDependency 'ucrtbase.dll') 'UCRT detector missed ucrtbase.dll'
        Assert-Fixture (-not (Test-PortableDynamicCrtDependency 'KERNEL32.dll')) 'CRT detector rejected a system DLL'
    }

    Run-FixtureTest 'target overlap and Node engine range gates are enforced' {
        $root = Join-Path $fixtureRoot 'path-gates'
        $project = Join-Path $root 'project'
        $dist = Join-Path $project 'dist'
        New-Item -ItemType Directory -Path $dist -Force | Out-Null
        Assert-FixtureThrows {
            Assert-PortableCargoTargetPath -ProjectRoot $project -DistRoot $dist `
                -CargoTargetRoot (Join-Path $dist 'target')
        } 'CARGO_TARGET_DIR inside dist was accepted'
        Assert-PortableCargoTargetPath -ProjectRoot $project -DistRoot $dist `
            -CargoTargetRoot (Join-Path $project 'target')
        Assert-Fixture (Test-PortableMajorVersionRange -Version 'v22.5.0' -Range '>=22 <25') `
            'valid Node engine range was rejected'
        Assert-Fixture (-not (Test-PortableMajorVersionRange -Version '25.0.0' -Range '>=22 <25')) `
            'invalid Node engine range was accepted'
    }

    Run-FixtureTest 'immutable candidate digest excludes only runtime data and binds transaction id' {
        $source = Join-Path $fixtureRoot 'immutable-source'
        $stage = Join-Path $fixtureRoot 'immutable-stage'
        $groups = @(New-FixtureResourceSource $source)
        New-FixturePortableStage -StageRoot $stage -SourceRoot $source -SourceGroups $groups
        $id = [guid]::NewGuid().ToString('N')
        $manifestPath = Join-Path $stage 'build-manifest.json'
        $manifest = Read-PortableJson $manifestPath
        $manifest | Add-Member -NotePropertyName transaction_id -NotePropertyValue $id -Force
        Write-PortableJsonAtomic -Path $manifestPath -Value $manifest
        $null = Assert-PortableStage -StageRoot $stage -ExpectedTransactionId $id
        $immutable = Get-PortableImmutableTreeMeasure $stage

        Write-FixtureText -Path (Join-Path $stage 'user_data\mutable.txt') -Value 'runtime data'
        Write-FixtureText -Path (Join-Path $stage '.portable-update.lock') -Value 'ephemeral lock'
        Assert-PortableMeasureEqual -Expected $immutable -Actual (Get-PortableImmutableTreeMeasure $stage) `
            -Description 'mutable candidate exclusions'
        Write-FixtureText -Path (Join-Path $stage 'README.txt') -Value 'immutable tamper'
        Assert-Fixture (-not (Test-PortableMeasureEqual -Expected $immutable `
            -Actual (Get-PortableImmutableTreeMeasure $stage))) 'immutable candidate tampering was accepted'
        Assert-FixtureThrows {
            $null = Assert-PortableStage -StageRoot $stage `
                -ExpectedTransactionId ([guid]::NewGuid().ToString('N'))
        } 'wrong build-manifest transaction id was accepted'
    }

    Run-FixtureTest 'output operation lock is exclusive and reusable across operations' {
        $dist = Join-Path $fixtureRoot 'exclusive-lock\dist'
        $id = [guid]::NewGuid().ToString('N')
        $first = Enter-PortableOutputOperationLock -DistRoot $dist -OutputName 'fixture' `
            -TransactionId $id -Purpose 'first'
        try {
            Assert-FixtureThrows {
                $null = Enter-PortableOutputOperationLock -DistRoot $dist -OutputName 'fixture' `
                    -TransactionId ([guid]::NewGuid().ToString('N')) -Purpose 'concurrent'
            } 'concurrent output operation acquired the same lock'
        }
        finally { Exit-PortableOutputOperationLock $first }
        $second = Enter-PortableOutputOperationLock -DistRoot $dist -OutputName 'fixture' `
            -TransactionId ([guid]::NewGuid().ToString('N')) -Purpose 'later-finalizer'
        Exit-PortableOutputOperationLock $second
    }

    Run-FixtureTest 'finalizer reconciles a crash after backup creation and completes the install' {
        $dist = Join-Path $fixtureRoot 'backup-created-recovery\dist'
        $live = Join-Path $dist 'fixture'
        $id = [guid]::NewGuid().ToString('N')
        $stage = Join-Path $dist ".fixture.stage-$id"
        $backup = Join-Path $dist ".fixture.previous-$id"
        $failed = Join-Path $dist ".fixture.failed-$id"
        $manifest = Join-Path $dist ".fixture.swap-$id.json"
        Write-FixtureText -Path (Join-Path $live 'version.txt') -Value 'old'
        Write-FixtureText -Path (Join-Path $live 'user_data\save.txt') -Value 'preserved'
        Write-FixtureText -Path (Join-Path $stage 'version.txt') -Value 'new'
        Write-FixtureText -Path (Join-Path $stage 'user_data\save.txt') -Value 'preserved'
        $transaction = New-FixtureTransaction -Id $id -DistRoot $dist -LiveRoot $live `
            -StageRoot $stage -BackupRoot $backup -FailedRoot $failed
        Move-Item -LiteralPath $live -Destination $backup
        $transaction.backup_tree = Get-PortableTreeMeasure $backup
        $transaction.status = 'backup_created'
        Write-PortableSwapTransaction -Path $manifest -Transaction $transaction
        $validateNew = {
            param($root)
            if ((Get-Content -LiteralPath (Join-Path $root 'version.txt') -Raw) -ne 'new') {
                throw 'candidate marker is not new'
            }
        }

        $reconciled = Invoke-PortableSwapFinalization -TransactionManifest $manifest `
            -ValidateLive $validateNew -QuiescenceCheck { param($root) }

        Assert-Fixture ($reconciled.Status -eq 'ready_to_reconcile') `
            'backup-created topology dry run did not identify the recovery action'
        Assert-Fixture (-not (Test-Path -LiteralPath $live)) `
            'backup-created topology dry run changed the live directory'
        Assert-Fixture (Test-Path -LiteralPath $stage -PathType Container) `
            'backup-created topology dry run consumed the stage'
        $completed = Invoke-PortableSwapFinalization -TransactionManifest $manifest `
            -ValidateLive $validateNew -QuiescenceCheck { param($root) } -Execute
        Assert-Fixture ($completed.Status -eq 'finalized') `
            'backup-created topology did not reconcile and finalize'
        Assert-Fixture ((Read-PortableJson $manifest).status -eq 'finalized') `
            'reconciled transaction did not persist the final state'
        Assert-Fixture ((Get-Content -LiteralPath (Join-Path $live 'version.txt') -Raw) -eq 'new') `
            'reconciliation did not install the staged candidate'
        Assert-Fixture (-not (Test-Path -LiteralPath $backup)) `
            'completed reconciliation retained its retired recovery backup'
    }

    Run-FixtureTest 'wrong transaction candidate at the legal live path is rejected without rollback loss' {
        $dist = Join-Path $fixtureRoot 'wrong-live-transaction\dist'
        $live = Join-Path $dist 'fixture'
        $id = [guid]::NewGuid().ToString('N')
        $stage = Join-Path $dist ".fixture.stage-$id"
        $backup = Join-Path $dist ".fixture.previous-$id"
        $failed = Join-Path $dist ".fixture.failed-$id"
        $manifest = Join-Path $dist ".fixture.swap-$id.json"
        Write-FixtureText -Path (Join-Path $live 'version.txt') -Value 'old'
        Write-FixtureText -Path (Join-Path $live 'user_data\save.txt') -Value 'preserved'
        Write-FixtureText -Path (Join-Path $stage 'version.txt') -Value 'new'
        Write-FixtureText -Path (Join-Path $stage 'user_data\save.txt') -Value 'preserved'
        $transaction = New-FixtureTransaction -Id $id -DistRoot $dist -LiveRoot $live `
            -StageRoot $stage -BackupRoot $backup -FailedRoot $failed
        Move-Item -LiteralPath $live -Destination $backup
        Move-Item -LiteralPath $stage -Destination $live
        $wrong = Read-PortableJson (Join-Path $live 'build-manifest.json')
        $wrong.transaction_id = [guid]::NewGuid().ToString('N')
        Write-PortableJsonAtomic -Path (Join-Path $live 'build-manifest.json') -Value $wrong
        $transaction.backup_tree = Get-PortableTreeMeasure $backup
        $transaction.status = 'backup_created'
        Write-PortableSwapTransaction -Path $manifest -Transaction $transaction

        Assert-FixtureThrows {
            $null = Invoke-PortableSwapFinalization -TransactionManifest $manifest `
                -ValidateLive { param($root) } -QuiescenceCheck { param($root) }
        } 'wrong-transaction live candidate was accepted'
        Assert-Fixture (Test-Path -LiteralPath $live -PathType Container) `
            'blocked wrong live reconciliation deleted the candidate before review'
        Assert-Fixture (Test-Path -LiteralPath $backup -PathType Container) `
            'blocked wrong live reconciliation lost the recovery backup'
        Assert-Fixture ((Read-PortableJson $manifest).status -eq 'backup_created') `
            'blocked wrong live reconciliation changed the receipt state'
    }

    Run-FixtureTest 'validated pre-smoke crash is discardable only through explicit failed cleanup' {
        $dist = Join-Path $fixtureRoot 'pre-smoke-cleanup\dist'
        $live = Join-Path $dist 'fixture'
        $id = [guid]::NewGuid().ToString('N')
        $stage = Join-Path $dist ".fixture.stage-$id"
        $backup = Join-Path $dist ".fixture.previous-$id"
        $failed = Join-Path $dist ".fixture.failed-$id"
        $manifest = Join-Path $dist ".fixture.swap-$id.json"
        Write-FixtureText -Path (Join-Path $live 'version.txt') -Value 'old'
        Write-FixtureText -Path (Join-Path $live 'user_data\save.txt') -Value 'preserved'
        Write-FixtureText -Path (Join-Path $stage 'version.txt') -Value 'candidate'
        Write-FixtureText -Path (Join-Path $stage 'user_data\save.txt') -Value 'preserved'
        $transaction = New-FixtureTransaction -Id $id -DistRoot $dist -LiveRoot $live `
            -StageRoot $stage -BackupRoot $backup -FailedRoot $failed
        $transaction.status = 'stage_validated'
        $transaction.smoke_status = 'pending'
        Write-PortableSwapTransaction -Path $manifest -Transaction $transaction

        $dry = Invoke-PortableFailedArtifactCleanup -TransactionManifest $manifest `
            -QuiescenceCheck { param($root) }
        Assert-Fixture ($dry.Action -eq 'dry-run-retire-validated-pre-smoke-stage') `
            'pre-smoke cleanup dry run did not identify the exact discard action'
        Assert-Fixture (Test-Path -LiteralPath $stage -PathType Container) `
            'pre-smoke cleanup dry run mutated the stage'
        $cleaned = Invoke-PortableFailedArtifactCleanup -TransactionManifest $manifest `
            -QuiescenceCheck { param($root) } -Execute
        Assert-Fixture ($cleaned.Status -eq 'failure_artifact_finalized') `
            'pre-smoke failed cleanup did not finalize'
        Assert-Fixture (-not (Test-Path -LiteralPath $stage) -and -not (Test-Path -LiteralPath $failed)) `
            'pre-smoke cleanup retained generated candidate directories'
        Assert-Fixture ((Get-Content -LiteralPath (Join-Path $live 'version.txt') -Raw) -eq 'old') `
            'pre-smoke cleanup modified the legal live installation'
    }

    Run-FixtureTest 'staging receipt created before its directory is recoverable without deletion' {
        $dist = Join-Path $fixtureRoot 'empty-staging-receipt\dist'
        $live = Join-Path $dist 'fixture'
        $id = [guid]::NewGuid().ToString('N')
        $stage = Join-Path $dist ".fixture.stage-$id"
        $backup = Join-Path $dist ".fixture.previous-$id"
        $failed = Join-Path $dist ".fixture.failed-$id"
        $manifest = Join-Path $dist ".fixture.swap-$id.json"
        Write-FixtureText -Path (Join-Path $live 'version.txt') -Value 'old'
        Write-FixtureText -Path (Join-Path $stage 'placeholder.txt') -Value 'not committed'
        $transaction = New-FixtureTransaction -Id $id -DistRoot $dist -LiveRoot $live `
            -StageRoot $stage -BackupRoot $backup -FailedRoot $failed
        Remove-Item -LiteralPath $stage -Recurse -Force
        $transaction.status = 'staging'
        Write-PortableSwapTransaction -Path $manifest -Transaction $transaction
        $dry = Invoke-PortableFailedArtifactCleanup -TransactionManifest $manifest `
            -QuiescenceCheck { param($root) }
        Assert-Fixture ($dry.Action -eq 'dry-run-finalize-empty-staging-receipt') `
            'empty staging receipt dry run did not identify a no-delete recovery'
        $done = Invoke-PortableFailedArtifactCleanup -TransactionManifest $manifest `
            -QuiescenceCheck { param($root) } -Execute
        Assert-Fixture ($done.Action -eq 'empty-staging-receipt-finalized') `
            'empty staging receipt did not finalize safely'
        Assert-Fixture ((Get-Content -LiteralPath (Join-Path $live 'version.txt') -Raw) -eq 'old') `
            'empty staging recovery changed the legal live directory'
    }

    Run-FixtureTest 'failed installed candidate is isolated and previous live is restored' {
        $dist = Join-Path $fixtureRoot 'failure-swap\dist'
        $live = Join-Path $dist 'fixture'
        $id = [guid]::NewGuid().ToString('N')
        $stage = Join-Path $dist ".fixture.stage-$id"
        $backup = Join-Path $dist ".fixture.previous-$id"
        $failed = Join-Path $dist ".fixture.failed-$id"
        $manifest = Join-Path $dist ".fixture.swap-$id.json"
        Write-FixtureText -Path (Join-Path $live 'version.txt') -Value 'old'
        Write-FixtureText -Path (Join-Path $live 'user_data\save.txt') -Value 'preserved'
        Write-FixtureText -Path (Join-Path $stage 'version.txt') -Value 'new'
        Write-FixtureText -Path (Join-Path $stage 'user_data\save.txt') -Value 'preserved'
        $transaction = New-FixtureTransaction -Id $id -DistRoot $dist -LiveRoot $live `
            -StageRoot $stage -BackupRoot $backup -FailedRoot $failed
        Write-PortableSwapTransaction -Path $manifest -Transaction $transaction
        Assert-FixtureThrows {
            Invoke-PortableSwapTransaction -DistRoot $dist -StageRoot $stage -LiveRoot $live `
                -BackupRoot $backup -FailedRoot $failed -TransactionManifest $manifest `
                -Transaction $transaction -ValidateStage { param($root) } `
                -ValidateLive { param($root) throw 'fixture live validation failure' } `
                -QuiescenceCheck { param($root) }
        } 'injected post-install validation failure did not fail the swap'
        Assert-Fixture ((Get-Content -LiteralPath (Join-Path $live 'version.txt') -Raw) -eq 'old') `
            'previous live directory was not restored'
        Assert-Fixture (Test-Path -LiteralPath $failed -PathType Container) 'bad live was not isolated'
        Assert-Fixture (-not (Test-Path -LiteralPath $backup)) 'backup remained after rollback restoration'
        $record = Read-PortableJson $manifest
        Assert-Fixture ($record.status -eq 'rollback_restored') 'rollback status was not recorded'
        $dryRun = Invoke-PortableFailedArtifactCleanup -TransactionManifest $manifest `
            -QuiescenceCheck { param($root) }
        Assert-Fixture ($dryRun.Status -eq 'ready_to_clean_failed_artifact') 'failed cleanup dry run was not ready'
        Assert-Fixture (Test-Path -LiteralPath $failed) 'failed cleanup dry run deleted data'
        Assert-FixtureThrows {
            $null = Invoke-PortableFailedArtifactCleanup -TransactionManifest $manifest `
                -QuiescenceCheck { param($root) } -Execute `
                -AfterFailedRetired { param($root, $receipt) throw 'injected retired-failed crash' }
        } 'failed cleanup retirement fault did not interrupt deletion'
        $retired = Read-PortableJson $manifest
        Assert-Fixture ($retired.status -eq 'cleaning_failed_retired') `
            'failed cleanup did not persist its retired tombstone state'
        Assert-Fixture (-not (Test-Path -LiteralPath $failed)) `
            'failed cleanup retained both source and retired tombstone'
        Assert-Fixture (Test-Path -LiteralPath ([string]$retired.failed_tombstone_root) -PathType Container) `
            'failed cleanup retirement did not create its transaction-bound tombstone'
        $cleaned = Invoke-PortableFailedArtifactCleanup -TransactionManifest $manifest `
            -QuiescenceCheck { param($root) } -Execute
        Assert-Fixture ($cleaned.Status -eq 'failure_artifact_finalized') 'failed artifact cleanup did not finalize'
        Assert-Fixture (-not (Test-Path -LiteralPath $failed)) 'failed artifact cleanup did not delete its exact target'
        Assert-Fixture (-not (Test-Path -LiteralPath ([string]$retired.failed_tombstone_root))) `
            'failed artifact cleanup retained a partial tombstone'
    }

    Run-FixtureTest 'successful swap retains backup until explicit finalization' {
        $dist = Join-Path $fixtureRoot 'success-swap\dist'
        $live = Join-Path $dist 'fixture'
        $id = [guid]::NewGuid().ToString('N')
        $stage = Join-Path $dist ".fixture.stage-$id"
        $backup = Join-Path $dist ".fixture.previous-$id"
        $failed = Join-Path $dist ".fixture.failed-$id"
        $manifest = Join-Path $dist ".fixture.swap-$id.json"
        Write-FixtureText -Path (Join-Path $live 'version.txt') -Value 'old'
        Write-FixtureText -Path (Join-Path $live 'user_data\save.txt') -Value 'preserved'
        Write-FixtureText -Path (Join-Path $stage 'version.txt') -Value 'new'
        Write-FixtureText -Path (Join-Path $stage 'user_data\save.txt') -Value 'preserved'
        $transaction = New-FixtureTransaction -Id $id -DistRoot $dist -LiveRoot $live `
            -StageRoot $stage -BackupRoot $backup -FailedRoot $failed
        Write-PortableSwapTransaction -Path $manifest -Transaction $transaction
        $validateNew = {
            param($root)
            if ((Get-Content -LiteralPath (Join-Path $root 'version.txt') -Raw) -ne 'new') {
                throw 'fixture did not install new live marker'
            }
        }
        $result = Invoke-PortableSwapTransaction -DistRoot $dist -StageRoot $stage -LiveRoot $live `
            -BackupRoot $backup -FailedRoot $failed -TransactionManifest $manifest `
            -Transaction $transaction -ValidateStage $validateNew -ValidateLive $validateNew `
            -QuiescenceCheck { param($root) }
        Assert-Fixture ($result.Status -eq 'swapped_pending_finalize') 'successful swap status is wrong'
        Assert-Fixture (Test-Path -LiteralPath $backup -PathType Container) 'build transaction deleted its backup'
        $dryRun = Invoke-PortableSwapFinalization -TransactionManifest $manifest -ValidateLive $validateNew `
            -QuiescenceCheck { param($root) }
        Assert-Fixture ($dryRun.Status -eq 'ready_to_finalize') 'backup finalization dry run was not ready'
        Assert-Fixture (Test-Path -LiteralPath $backup) 'finalization dry run deleted the backup'
        $final = Invoke-PortableSwapFinalization -TransactionManifest $manifest -ValidateLive $validateNew `
            -QuiescenceCheck { param($root) } -Execute
        Assert-Fixture ($final.Status -eq 'finalized') 'explicit finalization did not finish'
        Assert-Fixture (-not (Test-Path -LiteralPath $backup)) 'explicit finalization did not remove backup'
        $again = Invoke-PortableSwapFinalization -TransactionManifest $manifest -ValidateLive $validateNew `
            -QuiescenceCheck { param($root) } -Execute
        Assert-Fixture ($again.Status -eq 'finalized' -and $again.Action -eq 'no-op') `
            'repeated finalization was not an idempotent no-op'
    }

    Run-FixtureTest 'finalizing recovers after backup deletion and blocks invalid live data' {
        $dist = Join-Path $fixtureRoot 'interrupted-finalize\dist'
        $live = Join-Path $dist 'fixture'
        $id = [guid]::NewGuid().ToString('N')
        $stage = Join-Path $dist ".fixture.stage-$id"
        $backup = Join-Path $dist ".fixture.previous-$id"
        $failed = Join-Path $dist ".fixture.failed-$id"
        $manifest = Join-Path $dist ".fixture.swap-$id.json"
        Write-FixtureText -Path (Join-Path $live 'version.txt') -Value 'old'
        Write-FixtureText -Path (Join-Path $live 'user_data\save.txt') -Value 'preserved'
        Write-FixtureText -Path (Join-Path $stage 'version.txt') -Value 'new'
        Write-FixtureText -Path (Join-Path $stage 'user_data\save.txt') -Value 'preserved'
        $transaction = New-FixtureTransaction -Id $id -DistRoot $dist -LiveRoot $live `
            -StageRoot $stage -BackupRoot $backup -FailedRoot $failed
        Write-PortableSwapTransaction -Path $manifest -Transaction $transaction
        $validateNew = {
            param($root)
            if ((Get-Content -LiteralPath (Join-Path $root 'version.txt') -Raw) -ne 'new') {
                throw 'fixture did not retain the installed live marker'
            }
        }
        $null = Invoke-PortableSwapTransaction -DistRoot $dist -StageRoot $stage -LiveRoot $live `
            -BackupRoot $backup -FailedRoot $failed -TransactionManifest $manifest `
            -Transaction $transaction -ValidateStage $validateNew -ValidateLive $validateNew `
            -QuiescenceCheck { param($root) }

        Assert-FixtureThrows {
            $null = Invoke-PortableSwapFinalization -TransactionManifest $manifest `
                -ValidateLive $validateNew -QuiescenceCheck { param($root) } -Execute `
                -AfterBackupRemoved { param($backupRoot, $transactionPath) throw 'injected crash after backup removal' }
        } 'injected post-backup-removal crash did not interrupt finalization'
        Assert-Fixture (-not (Test-Path -LiteralPath $backup)) `
            'fault injection did not occur after backup deletion'
        $interrupted = Read-PortableJson $manifest
        Assert-Fixture ($interrupted.status -eq 'finalizing_backup_retired') `
            'interrupted finalization did not retain the recoverable retired-backup state'
        Assert-Fixture (-not [bool]$interrupted.backup_deleted) `
            'interrupted finalization incorrectly claimed its backup deletion was committed'

        Write-FixtureText -Path (Join-Path $live 'user_data\save.txt') -Value 'tampered-after-crash'
        Assert-FixtureThrows {
            $null = Invoke-PortableSwapFinalization -TransactionManifest $manifest `
                -ValidateLive $validateNew -QuiescenceCheck { param($root) } -Execute
        } 'finalizing recovery accepted changed live user_data'
        Assert-Fixture ((Read-PortableJson $manifest).status -eq 'finalizing_backup_retired') `
            'blocked recovery changed the transaction state'
        Assert-Fixture ((Get-Content -LiteralPath (Join-Path $live 'user_data\save.txt') -Raw) -eq 'tampered-after-crash') `
            'blocked recovery modified live user_data'

        Write-FixtureText -Path (Join-Path $live 'user_data\save.txt') -Value 'preserved'
        $dryRecovery = Invoke-PortableSwapFinalization -TransactionManifest $manifest `
            -ValidateLive $validateNew -QuiescenceCheck { param($root) }
        Assert-Fixture ($dryRecovery.Status -eq 'ready_to_finalize' -and
            $dryRecovery.Action -eq 'dry-run-complete-interrupted-finalization') `
            'interrupted finalization dry run did not identify the recoverable state'
        $recovered = Invoke-PortableSwapFinalization -TransactionManifest $manifest `
            -ValidateLive $validateNew -QuiescenceCheck { param($root) } -Execute
        Assert-Fixture ($recovered.Status -eq 'finalized' -and
            $recovered.Action -eq 'completed-interrupted-finalization') `
            'interrupted finalization did not complete atomically'
        $completed = Read-PortableJson $manifest
        Assert-Fixture ($completed.status -eq 'finalized' -and [bool]$completed.backup_deleted) `
            'recovered transaction did not commit finalized backup evidence'
        Assert-Fixture (-not [string]::IsNullOrWhiteSpace([string]$completed.finalized_at_utc)) `
            'recovered transaction omitted finalized_at_utc'
        $again = Invoke-PortableSwapFinalization -TransactionManifest $manifest `
            -ValidateLive $validateNew -QuiescenceCheck { param($root) } -Execute
        Assert-Fixture ($again.Status -eq 'finalized' -and $again.Action -eq 'no-op') `
            'recovered finalization was not idempotent on repetition'
    }

    Run-FixtureTest 'finalizing with an intact backup revalidates and resumes deletion' {
        $dist = Join-Path $fixtureRoot 'resume-finalize-with-backup\dist'
        $live = Join-Path $dist 'fixture'
        $id = [guid]::NewGuid().ToString('N')
        $stage = Join-Path $dist ".fixture.stage-$id"
        $backup = Join-Path $dist ".fixture.previous-$id"
        $failed = Join-Path $dist ".fixture.failed-$id"
        $manifest = Join-Path $dist ".fixture.swap-$id.json"
        Write-FixtureText -Path (Join-Path $live 'version.txt') -Value 'old'
        Write-FixtureText -Path (Join-Path $live 'user_data\save.txt') -Value 'preserved'
        Write-FixtureText -Path (Join-Path $stage 'version.txt') -Value 'new'
        Write-FixtureText -Path (Join-Path $stage 'user_data\save.txt') -Value 'preserved'
        $transaction = New-FixtureTransaction -Id $id -DistRoot $dist -LiveRoot $live `
            -StageRoot $stage -BackupRoot $backup -FailedRoot $failed
        Write-PortableSwapTransaction -Path $manifest -Transaction $transaction
        $validateNew = {
            param($root)
            if ((Get-Content -LiteralPath (Join-Path $root 'version.txt') -Raw) -ne 'new') {
                throw 'fixture did not retain the installed live marker'
            }
        }
        $null = Invoke-PortableSwapTransaction -DistRoot $dist -StageRoot $stage -LiveRoot $live `
            -BackupRoot $backup -FailedRoot $failed -TransactionManifest $manifest `
            -Transaction $transaction -ValidateStage $validateNew -ValidateLive $validateNew `
            -QuiescenceCheck { param($root) }
        $interrupted = Read-PortableJson $manifest
        $interrupted.status = 'finalizing'
        Write-PortableSwapTransaction -Path $manifest -Transaction $interrupted

        $dryRecovery = Invoke-PortableSwapFinalization -TransactionManifest $manifest `
            -ValidateLive $validateNew -QuiescenceCheck { param($root) }
        Assert-Fixture ($dryRecovery.Status -eq 'ready_to_finalize' -and
            $dryRecovery.Action -eq 'dry-run-resume-delete-backup') `
            'intact-backup finalizing state was not resumable after revalidation'
        Assert-Fixture (Test-Path -LiteralPath $backup -PathType Container) `
            'intact-backup recovery dry run removed the backup'
        $recovered = Invoke-PortableSwapFinalization -TransactionManifest $manifest `
            -ValidateLive $validateNew -QuiescenceCheck { param($root) } -Execute
        Assert-Fixture ($recovered.Status -eq 'finalized' -and $recovered.Action -eq 'backup-deleted') `
            'intact-backup finalizing state did not resume deletion'
        Assert-Fixture (-not (Test-Path -LiteralPath $backup)) `
            'intact-backup resumed finalization retained its backup'
    }

    Run-FixtureTest 'finalized transaction receipts are retained with a bounded policy' {
        $dist = Join-Path $fixtureRoot 'receipt-retention\dist'
        New-Item -ItemType Directory -Path $dist -Force | Out-Null
        $currentManifest = ''
        $newestManifest = ''
        $pendingManifest = ''
        for ($index = 0; $index -lt 8; $index += 1) {
            $id = ([guid]::NewGuid().ToString('N'))
            $live = Join-Path $dist 'fixture'
            $stage = Join-Path $dist ".fixture.stage-$id"
            $backup = Join-Path $dist ".fixture.previous-$id"
            $failed = Join-Path $dist ".fixture.failed-$id"
            $manifest = Join-Path $dist ".fixture.swap-$id.json"
            $transaction = New-FixtureTransaction -Id $id -DistRoot $dist -LiveRoot $live `
                -StageRoot $stage -BackupRoot $backup -FailedRoot $failed
            if ($index -eq 7) {
                $transaction.status = 'swapped_no_backup'
                $pendingManifest = $manifest
            }
            else {
                $transaction.status = 'finalized'
                $transaction.finalized_at_utc = [DateTime]::UtcNow.AddMinutes(-20 + $index).ToString('o')
                if ($index -eq 0) { $currentManifest = $manifest }
                if ($index -eq 6) { $newestManifest = $manifest }
            }
            Write-PortableSwapTransaction -Path $manifest -Transaction $transaction
        }

        $dryRun = Invoke-PortableTransactionReceiptRetention -TransactionManifest $currentManifest
        Assert-Fixture ($dryRun.PlannedPruneCount -eq 2 -and $dryRun.PrunedCount -eq 0) `
            'receipt retention dry run did not plan exactly the two oldest receipts'
        Assert-Fixture (@(Get-ChildItem -LiteralPath $dist -File -Filter '.fixture.swap-*.json').Count -eq 8) `
            'receipt retention dry run changed the fixture'

        $executed = Invoke-PortableTransactionReceiptRetention -TransactionManifest $currentManifest -Execute
        Assert-Fixture ($executed.PrunedCount -eq 2 -and $executed.RetainedCount -eq 5) `
            'receipt retention did not keep exactly five finalized receipts'
        Assert-Fixture (-not (Test-Path -LiteralPath $currentManifest)) `
            'an old current receipt displaced one of the globally newest five receipts'
        Assert-Fixture (Test-Path -LiteralPath $newestManifest -PathType Leaf) `
            'receipt retention removed a globally newer receipt'
        Assert-Fixture (Test-Path -LiteralPath $pendingManifest -PathType Leaf) `
            'receipt retention removed a pending recovery transaction'

        $repeat = Invoke-PortableTransactionReceiptRetention -TransactionManifest $newestManifest -Execute
        Assert-Fixture ($repeat.Action -eq 'no-op' -and $repeat.PrunedCount -eq 0) `
            'receipt retention repeat was not an idempotent no-op'

        $futureId = [guid]::NewGuid().ToString('N')
        $futureManifest = Join-Path $dist ".fixture.swap-$futureId.json"
        $future = New-FixtureTransaction -Id $futureId -DistRoot $dist -LiveRoot (Join-Path $dist 'fixture') `
            -StageRoot (Join-Path $dist ".fixture.stage-$futureId") `
            -BackupRoot (Join-Path $dist ".fixture.previous-$futureId") `
            -FailedRoot (Join-Path $dist ".fixture.failed-$futureId")
        $future.status = 'finalized'
        $future.finalized_at_utc = [DateTime]::UtcNow.AddHours(2).ToString('o')
        Write-PortableSwapTransaction -Path $futureManifest -Transaction $future
        Assert-FixtureThrows {
            $null = Invoke-PortableTransactionReceiptRetention -TransactionManifest $newestManifest
        } 'receipt retention accepted an unreasonable future completion timestamp'
    }

    Write-Host "ALL PORTABLE FIXTURES PASSED: $script:Passed"
}
finally {
    if (Test-Path -LiteralPath $fixtureRoot) {
        Remove-Item -LiteralPath $fixtureRoot -Recurse -Force
    }
    Write-Host "Fixture cleanup complete: $fixtureRoot"
}
