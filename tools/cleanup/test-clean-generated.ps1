[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$cleaner = Convert-Path (Join-Path $PSScriptRoot '..\clean-generated.ps1')
$pathsModule = Join-Path $PSScriptRoot 'GuardedCleanup.psm1'
Import-Module $pathsModule -Force

$sandbox = Join-Path ([IO.Path]::GetTempPath()) ("adm-newrust-cleanup-tests-{0}" -f ([guid]::NewGuid().ToString('N')))
$project = Join-Path $sandbox 'fixture source'
$reparsePaths = New-Object System.Collections.Generic.List[string]
$passed = 0

function Write-Utf8Json {
    param([Parameter(Mandatory = $true)]$Value, [Parameter(Mandatory = $true)][string]$Path)
    $Value | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $Path -Encoding UTF8
}

function Assert-True {
    param([Parameter(Mandatory = $true)][bool]$Condition, [Parameter(Mandatory = $true)][string]$Message)
    if (-not $Condition) { throw "assertion failed: $Message" }
}

function Assert-Equal {
    param($Expected, $Actual, [Parameter(Mandatory = $true)][string]$Message)
    if ($Expected -ne $Actual) { throw "assertion failed: $Message (expected '$Expected', actual '$Actual')" }
}

function Invoke-FixtureCleaner {
    param(
        [string[]]$Targets = @(),
        [ValidateSet('generated', 'owned-ephemeral-user-data', 'owned-ephemeral-workspace')]
        [string]$Kind = 'generated',
        [string[]]$Protected = @(),
        [string]$OwnerManifest,
        [string]$Nonce,
        [switch]$Execute,
        [switch]$ScanPortableStaging
    )

    function ConvertTo-Literal([string]$Value) { return "'" + $Value.Replace("'", "''") + "'" }
    $command = "& $(ConvertTo-Literal $cleaner) -ProjectRoot $(ConvertTo-Literal $project) " +
        "-Kind $(ConvertTo-Literal $Kind) -Json"
    if ($Targets.Count -gt 0) {
        $literalTargets = @($Targets | ForEach-Object { ConvertTo-Literal $_ }) -join ','
        $command += " -Target @($literalTargets)"
    }
    if ($Protected.Count -gt 0) {
        $literalProtected = @($Protected | ForEach-Object { ConvertTo-Literal $_ }) -join ','
        $command += " -ProtectedUserData @($literalProtected)"
    }
    if ($OwnerManifest) { $command += " -OwnerManifest $(ConvertTo-Literal $OwnerManifest)" }
    if ($Nonce) { $command += " -Nonce $(ConvertTo-Literal $Nonce)" }
    if ($Execute) { $command += ' -Execute' }
    if ($ScanPortableStaging) { $command += ' -ScanPortableStaging' }

    $output = @(& powershell.exe -NoProfile -ExecutionPolicy Bypass -Command $command 2>&1)
    $exitCode = $LASTEXITCODE
    $text = ($output | ForEach-Object { [string]$_ }) -join "`n"
    try { $payload = $text | ConvertFrom-Json }
    catch { throw "cleaner did not return JSON (exit $exitCode): $text" }
    [pscustomobject]@{ ExitCode = $exitCode; Payload = $payload }
}

function New-TestFile {
    param([Parameter(Mandatory = $true)][string]$Path, [string]$Content = 'fixture')
    $parent = Split-Path -Parent $Path
    if (-not (Test-Path -LiteralPath $parent)) { New-Item -ItemType Directory -Path $parent -Force | Out-Null }
    Set-Content -LiteralPath $Path -Value $Content -Encoding UTF8
}

function New-StageManifest {
    param([Parameter(Mandatory = $true)][string]$Stage)
    $userData = Join-Path $Stage 'user_data'
    New-Item -ItemType Directory -Path $userData -Force | Out-Null
    $measure = Get-TreeMeasure $userData
    Write-Utf8Json -Path (Join-Path $Stage '.adm-cleanup-stage.json') -Value ([ordered]@{
            schemaVersion = 1
            kind = 'verified-portable-stage'
            verified = $true
            targetPath = (ConvertTo-NormalizedPath $Stage)
            userData = $measure
        })
}

function Run-Test {
    param([Parameter(Mandatory = $true)][string]$Name, [Parameter(Mandatory = $true)][scriptblock]$Body)
    & $Body
    $script:passed += 1
    Write-Host "PASS $Name"
}

try {
    New-Item -ItemType Directory -Path $project -Force | Out-Null
    Write-Utf8Json -Path (Join-Path $project '.project_root') -Value ([ordered]@{
            schemaVersion = 1
            kind = 'source-project-root'
            projectId = 'autodesignmaker-rust-v2'
            workspaceManifest = 'Cargo.toml'
        })
    Set-Content -LiteralPath (Join-Path $project 'Cargo.toml') -Value '[workspace]' -Encoding UTF8

    Run-Test 'default is dry-run and preserves allowlisted target' {
        $target = Join-Path $project 'target'
        New-TestFile (Join-Path $target 'debug\cache.bin')
        $result = Invoke-FixtureCleaner -Targets @($target)
        Assert-Equal 0 $result.ExitCode 'dry-run exit code'
        Assert-Equal 'dry-run' $result.Payload.mode 'default mode'
        Assert-Equal 'dry-run-delete' $result.Payload.results[0].action 'planned action'
        Assert-True (Test-Path -LiteralPath $target) 'dry-run must preserve target'
    }

    Run-Test 'execute is explicit and repeated execution is an idempotent no-op' {
        $target = Join-Path $project 'target'
        $first = Invoke-FixtureCleaner -Targets @($target) -Execute
        Assert-Equal 0 $first.ExitCode 'execute exit code'
        Assert-Equal 'deleted' $first.Payload.results[0].action 'execute action'
        Assert-True (-not (Test-Path -LiteralPath $target)) 'target should be deleted inside fixture'
        $second = Invoke-FixtureCleaner -Targets @($target) -Execute
        Assert-Equal 'skipped' $second.Payload.results[0].action 'repeat action'
    }

    Run-Test 'exact project temp root is removable only when empty' {
        $tempRoot = Join-Path $project '.tmp'
        New-Item -ItemType Directory -Path $tempRoot -Force | Out-Null
        $dryRun = Invoke-FixtureCleaner -Targets @($tempRoot)
        Assert-Equal 'empty-project-temp-root' $dryRun.Payload.results[0].kind 'empty temp kind'
        Assert-Equal 'dry-run-delete' $dryRun.Payload.results[0].action 'empty temp dry run'

        $unknown = Join-Path $tempRoot 'operator-owned.txt'
        New-TestFile $unknown
        $refused = Invoke-FixtureCleaner -Targets @($tempRoot) -Execute
        Assert-True ($refused.ExitCode -ne 0) 'non-empty temp root refusal'
        Assert-True (Test-Path -LiteralPath $unknown) 'unknown temp entry preserved'

        Remove-Item -LiteralPath $unknown -Force
        $deleted = Invoke-FixtureCleaner -Targets @($tempRoot) -Execute
        Assert-Equal 'deleted' $deleted.Payload.results[0].action 'empty temp root deleted'
        Assert-True (-not (Test-Path -LiteralPath $tempRoot)) 'empty temp root removed'
    }

    Run-Test 'default cleanup retains standalone evidence and removes other generated gates' {
        $gates = Join-Path $project 'gates'
        $readme = Join-Path $gates 'README.md'
        $evidence = Join-Path $gates 'standalone-release-evidence.json'
        $generated = Join-Path $gates 'language-gate.json'
        $screenshots = Join-Path $gates 'screenshots\panel.png'
        New-TestFile $readme '# fixture gates'
        New-TestFile $evidence '{"status":"passed"}'
        New-TestFile $generated '{"status":"generated"}'
        New-TestFile $screenshots 'fixture-image'

        $dryRun = Invoke-FixtureCleaner
        $plannedPaths = @($dryRun.Payload.results | ForEach-Object target)
        Assert-True ($plannedPaths -notcontains (ConvertTo-NormalizedPath $evidence)) 'standalone evidence omitted from default cleanup'
        Assert-True ($plannedPaths -contains (ConvertTo-NormalizedPath $generated)) 'other generated gate discovered'
        Assert-True ($plannedPaths -contains (ConvertTo-NormalizedPath (Split-Path -Parent $screenshots))) 'generated gate directory discovered'

        $explicit = Invoke-FixtureCleaner -Targets @($evidence) -Execute
        Assert-Equal 'report-only' $explicit.Payload.results[0].action 'explicit evidence cleanup remains report-only'
        Assert-True (Test-Path -LiteralPath $evidence) 'explicit cleanup preserved evidence'

        $executed = Invoke-FixtureCleaner -Execute
        Assert-Equal 0 $executed.ExitCode 'default generated-gate execute exit code'
        Assert-True (-not (Test-Path -LiteralPath $generated)) 'ordinary generated gate deleted'
        Assert-True (-not (Test-Path -LiteralPath (Split-Path -Parent $screenshots))) 'generated gate directory deleted'
        Assert-True (Test-Path -LiteralPath $evidence) 'standalone evidence retained'
        Assert-True (Test-Path -LiteralPath $readme) 'gate README retained'
    }

    Run-Test 'source roots, protected source folders, and out-of-bound paths are refused' {
        $outside = Join-Path $sandbox 'outside'
        $outsideBackupName = Join-Path $sandbox '.release.previous-outside'
        New-TestFile (Join-Path $outside 'keep.txt')
        New-TestFile (Join-Path $outsideBackupName 'keep.txt')
        New-TestFile (Join-Path $project 'docs\keep.md')
        $rootResult = Invoke-FixtureCleaner -Targets @($project) -Execute
        $docsResult = Invoke-FixtureCleaner -Targets @((Join-Path $project 'docs')) -Execute
        $outsideResult = Invoke-FixtureCleaner -Targets @($outside) -Execute
        $outsideBackupResult = Invoke-FixtureCleaner -Targets @($outsideBackupName) -Execute
        Assert-True ($rootResult.ExitCode -ne 0) 'project root refusal'
        Assert-True ($docsResult.ExitCode -ne 0) 'docs refusal'
        Assert-True ($outsideResult.ExitCode -ne 0) 'boundary refusal'
        Assert-True ($outsideBackupResult.ExitCode -ne 0) 'out-of-bound backup-shaped path refusal'
        Assert-True (Test-Path -LiteralPath (Join-Path $outside 'keep.txt')) 'outside file preserved'
        Assert-True (Test-Path -LiteralPath (Join-Path $project 'docs\keep.md')) 'docs file preserved'
    }

    Run-Test 'ordinary cleanup refuses .git and user_data descendants' {
        $gitTarget = Join-Path $project '.tmp\test-git'
        $gitFileTarget = Join-Path $project '.tmp\test-git-file'
        $dataTarget = Join-Path $project '.tmp\test-data'
        New-TestFile (Join-Path $gitTarget '.git\config')
        New-TestFile (Join-Path $gitFileTarget '.git') 'gitdir: elsewhere'
        New-TestFile (Join-Path $dataTarget 'user_data\save.json')
        $gitResult = Invoke-FixtureCleaner -Targets @($gitTarget) -Execute
        $gitFileResult = Invoke-FixtureCleaner -Targets @($gitFileTarget) -Execute
        $dataResult = Invoke-FixtureCleaner -Targets @($dataTarget) -Execute
        Assert-True ($gitResult.ExitCode -ne 0) '.git refusal'
        Assert-True ($gitFileResult.ExitCode -ne 0) '.git file refusal'
        Assert-True ($dataResult.ExitCode -ne 0) 'user_data refusal'
        Assert-True (Test-Path -LiteralPath $gitTarget) '.git ancestor preserved'
        Assert-True (Test-Path -LiteralPath $dataTarget) 'user_data ancestor preserved'
    }

    Run-Test 'explicit protected-user-data protects itself, ancestors, and descendants' {
        $ancestor = Join-Path $project '.tmp\test-protected'
        $protected = Join-Path $ancestor 'local-state'
        New-TestFile (Join-Path $protected 'save.bin')
        foreach ($candidate in @($ancestor, $protected, (Join-Path $protected 'save.bin'))) {
            $result = Invoke-FixtureCleaner -Targets @($candidate) -Protected @($protected) -Execute
            Assert-True ($result.ExitCode -ne 0) "protected overlap refusal for $candidate"
        }
        Assert-True (Test-Path -LiteralPath (Join-Path $protected 'save.bin')) 'protected data preserved'
    }

    Run-Test 'mixed execute plans are atomic when any target is refused' {
        $valid = Join-Path $project '.tmp\test-atomic'
        $invalid = Join-Path $project 'apps'
        New-TestFile (Join-Path $valid 'cache.tmp')
        New-TestFile (Join-Path $invalid 'keep.rs')
        $result = Invoke-FixtureCleaner -Targets @($valid, $invalid) -Execute
        Assert-True ($result.ExitCode -ne 0) 'mixed plan exit code'
        Assert-True (@($result.Payload.results | Where-Object action -eq 'blocked-by-plan').Count -eq 1) 'valid target blocked'
        Assert-True (Test-Path -LiteralPath $valid) 'valid target was not partially deleted'
        Assert-True (Test-Path -LiteralPath $invalid) 'invalid target preserved'
    }

    Run-Test 'portable backups are report-only even with Execute' {
        $backup = Join-Path $project 'dist\.release.previous-1234'
        New-TestFile (Join-Path $backup 'user_data\save.json')
        $result = Invoke-FixtureCleaner -Targets @($backup) -Execute
        Assert-Equal 0 $result.ExitCode 'backup report exit code'
        Assert-Equal 'report-only' $result.Payload.results[0].action 'backup action'
        Assert-True (Test-Path -LiteralPath $backup) 'backup preserved'
    }

    Run-Test 'only verified empty portable stages are deletable' {
        $verified = Join-Path $project 'dist\.release.stage-1001'
        $unverified = Join-Path $project 'dist\.release.stage-1002'
        $nonEmpty = Join-Path $project 'dist\.release.stage-1003'
        $gitStage = Join-Path $project 'dist\.release.stage-1004'
        New-StageManifest $verified
        New-TestFile (Join-Path $unverified 'artifact.bin')
        New-StageManifest $nonEmpty
        New-TestFile (Join-Path $nonEmpty 'user_data\save.json')
        New-StageManifest $gitStage
        New-TestFile (Join-Path $gitStage '.git') 'gitdir: elsewhere'
        $ok = Invoke-FixtureCleaner -Targets @($verified) -Execute
        $bad = Invoke-FixtureCleaner -Targets @($unverified) -Execute
        $data = Invoke-FixtureCleaner -Targets @($nonEmpty) -Execute
        $git = Invoke-FixtureCleaner -Targets @($gitStage) -Execute
        Assert-Equal 'deleted' $ok.Payload.results[0].action 'verified stage deletion'
        Assert-True ($bad.ExitCode -ne 0) 'unverified stage refused'
        Assert-True ($data.ExitCode -ne 0) 'non-empty stage refused'
        Assert-True ($git.ExitCode -ne 0) 'stage containing .git refused'
        Assert-True (Test-Path -LiteralPath $unverified) 'unverified stage preserved'
        Assert-True (Test-Path -LiteralPath $nonEmpty) 'non-empty stage preserved'
    }

    Run-Test 'portable staging scan finds every stage and backup without deleting' {
        $stage = Join-Path $project 'dist\.scan.stage-a'
        $previousOne = Join-Path $project 'dist\.scan.previous-a'
        $previousTwo = Join-Path $project 'dist\.scan.backup-b'
        New-StageManifest $stage
        New-TestFile (Join-Path $previousOne 'one.bin')
        New-TestFile (Join-Path $previousTwo 'two.bin')
        $result = Invoke-FixtureCleaner -ScanPortableStaging
        $paths = @($result.Payload.results | ForEach-Object target)
        Assert-True ($paths -contains (ConvertTo-NormalizedPath $stage)) 'stage discovered'
        Assert-True ($paths -contains (ConvertTo-NormalizedPath $previousOne)) 'previous backup discovered'
        Assert-True ($paths -contains (ConvertTo-NormalizedPath $previousTwo)) 'backup variant discovered'
        Assert-True (Test-Path -LiteralPath $stage) 'scan is dry-run'
    }

    Run-Test 'local Cargo output is default-allowlisted while node_modules is explicit-only' {
        $localBuild = Join-Path $project '.local-build'
        $nodeModules = Join-Path $project 'web\node_modules'
        New-TestFile (Join-Path $localBuild 'cargo-workspace\cache.bin')
        New-TestFile (Join-Path $nodeModules 'fixture-package\index.js')
        $default = Invoke-FixtureCleaner
        $defaultPaths = @($default.Payload.results | ForEach-Object target)
        Assert-True ($defaultPaths -contains (ConvertTo-NormalizedPath $localBuild)) '.local-build discovered by default'
        Assert-True ($defaultPaths -notcontains (ConvertTo-NormalizedPath $nodeModules)) 'node_modules omitted from default discovery'
        $explicit = Invoke-FixtureCleaner -Targets @($nodeModules)
        Assert-Equal 'web-dependency-cache' $explicit.Payload.results[0].kind 'node_modules exact kind'
        Assert-Equal 'dry-run-delete' $explicit.Payload.results[0].action 'node_modules explicit dry-run'
        Assert-True (Test-Path -LiteralPath $nodeModules) 'explicit dry-run preserves dependency cache'
        $deletedCache = Invoke-FixtureCleaner -Targets @($nodeModules) -Execute
        Assert-Equal 'deleted' $deletedCache.Payload.results[0].action 'explicit dependency cache deleted in fixture'

        $fakeLocal = Join-Path $project '.local-build-copy'
        $fakeModules = Join-Path $project 'web\node_modules-copy'
        New-TestFile (Join-Path $fakeLocal 'keep.bin')
        New-TestFile (Join-Path $fakeModules 'keep.bin')
        foreach ($lookalike in @($fakeLocal, $fakeModules)) {
            $refused = Invoke-FixtureCleaner -Targets @($lookalike) -Execute
            Assert-True ($refused.ExitCode -ne 0) "lookalike allowlist path refused: $lookalike"
            Assert-True (Test-Path -LiteralPath $lookalike) "lookalike path preserved: $lookalike"
        }
    }

    Run-Test 'active Cargo lock refuses cleanup and later allows idempotent fixture deletion' {
        $localBuild = Join-Path $project '.local-build'
        $lockPath = Join-Path $localBuild 'cargo-workspace\debug\.cargo-lock'
        New-TestFile $lockPath 'active'
        $stream = [IO.File]::Open($lockPath, [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite, [IO.FileShare]::ReadWrite)
        try {
            $stream.Lock(0, 1)
            $active = Invoke-FixtureCleaner -Targets @($localBuild)
            Assert-True ($active.ExitCode -ne 0) 'locked Cargo output refused'
            Assert-Equal 'refused' $active.Payload.results[0].action 'active lock action'
            Assert-True (Test-Path -LiteralPath $localBuild) 'active Cargo output preserved'
        }
        finally {
            $stream.Unlock(0, 1)
            $stream.Dispose()
        }
        $deleted = Invoke-FixtureCleaner -Targets @($localBuild) -Execute
        Assert-Equal 'deleted' $deleted.Payload.results[0].action 'inactive local Cargo output deleted'
        $repeat = Invoke-FixtureCleaner -Targets @($localBuild) -Execute
        Assert-Equal 'skipped' $repeat.Payload.results[0].action 'local Cargo cleanup repeat no-op'
    }

    Run-Test 'reparse descendants are refused without touching their destination' {
        $target = Join-Path $project '.tmp\test-reparse'
        $destination = Join-Path $sandbox 'junction-destination'
        New-TestFile (Join-Path $target 'local.bin')
        New-TestFile (Join-Path $destination 'keep.bin')
        $junction = Join-Path $target 'junction'
        New-Item -ItemType Junction -Path $junction -Target $destination | Out-Null
        $reparsePaths.Add($junction)
        $result = Invoke-FixtureCleaner -Targets @($target) -Execute
        Assert-True ($result.ExitCode -ne 0) 'reparse refusal'
        Assert-True (Test-Path -LiteralPath (Join-Path $destination 'keep.bin')) 'junction destination preserved'
        Assert-True (Test-Path -LiteralPath $target) 'reparse ancestor preserved'
    }

    Write-Host "All $passed guarded-cleanup fixture tests passed."
}
finally {
    foreach ($junction in $reparsePaths) {
        if (Test-Path -LiteralPath $junction) {
            $item = Get-Item -LiteralPath $junction -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                [IO.Directory]::Delete($junction)
            }
        }
    }
    $normalizedSandbox = ConvertTo-NormalizedPath $sandbox
    $normalizedTemp = ConvertTo-NormalizedPath ([IO.Path]::GetTempPath())
    $leaf = [IO.Path]::GetFileName($normalizedSandbox)
    if ((Test-PathWithin -Path $normalizedSandbox -Boundary $normalizedTemp) -and
        $leaf -match '^adm-newrust-cleanup-tests-[0-9a-f]{32}$' -and
        (Test-Path -LiteralPath $normalizedSandbox)) {
        Remove-Item -LiteralPath $normalizedSandbox -Recurse -Force -ErrorAction SilentlyContinue
    }
}
