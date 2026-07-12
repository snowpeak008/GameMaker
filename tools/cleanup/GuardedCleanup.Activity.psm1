Set-StrictMode -Version Latest

$pathsModule = Join-Path $PSScriptRoot 'GuardedCleanup.psm1'
Import-Module $pathsModule -Force

function Test-CommandLineContainsPath {
    param([string]$CommandLine, [string]$Path)
    if ([string]::IsNullOrWhiteSpace($CommandLine)) { return $false }
    $normalizedPath = (ConvertTo-NormalizedPath $Path).ToLowerInvariant()
    $forwardPath = $normalizedPath.Replace('\', '/')
    $line = $CommandLine.ToLowerInvariant()
    return ($line.Contains($normalizedPath) -or $line.Replace('\', '/').Contains($forwardPath))
}

function Get-RelevantBuildProcesses {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][string]$Kind
    )

    $cargoNames = @('cargo.exe', 'rustc.exe', 'cl.exe', 'link.exe')
    $webNames = @('node.exe', 'npm.exe', 'npx.exe')
    $names = @(if ($Kind -in @('cargo-target', 'local-cargo-target')) {
            $cargoNames
        }
        elseif ($Kind -in @('web-dist', 'web-dependency-cache', 'browser-output', 'browser-profile')) {
            $webNames
        })
    if ($names.Count -eq 0) { return @() }
    try {
        $processes = @(Get-CimInstance Win32_Process -ErrorAction Stop)
    }
    catch {
        throw "unable to inspect active build processes; cleanup refused: $($_.Exception.Message)"
    }
    return @($processes | Where-Object {
            $_.Name -in $names -and
            ((Test-CommandLineContainsPath -CommandLine $_.CommandLine -Path $Target) -or
                (Test-CommandLineContainsPath -CommandLine $_.CommandLine -Path $ProjectRoot))
        })
}

function Test-ExclusiveFileLockAvailable {
    param([Parameter(Mandatory = $true)][string]$Path)

    $stream = $null
    try {
        $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite, [IO.FileShare]::ReadWrite)
        $stream.Lock(0, 1)
        $stream.Unlock(0, 1)
        return $true
    }
    catch [IO.IOException] {
        return $false
    }
    catch [UnauthorizedAccessException] {
        return $false
    }
    finally {
        if ($null -ne $stream) { $stream.Dispose() }
    }
}

function Get-ActiveLockFiles {
    param([Parameter(Mandatory = $true)][string]$Target)

    if (-not (Test-Path -LiteralPath $Target -PathType Container)) { return @() }
    $locks = @(Get-ChildItem -LiteralPath $Target -File -Recurse -Force -ErrorAction Stop |
            Where-Object { $_.Name -in @('.cargo-lock', '.cleanup-active.lock') })
    return @($locks | Where-Object { -not (Test-ExclusiveFileLockAvailable -Path $_.FullName) })
}

function Assert-GeneratedTargetInactive {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$ProjectRoot,
        [Parameter(Mandatory = $true)][string]$Kind
    )

    $processes = @(Get-RelevantBuildProcesses -Target $Target -ProjectRoot $ProjectRoot -Kind $Kind)
    if ($processes.Count -gt 0) {
        $summary = ($processes | ForEach-Object { "$($_.Name):$($_.ProcessId)" }) -join ', '
        throw "active build process refused cleanup: $summary"
    }
    $activeLocks = @(Get-ActiveLockFiles -Target $Target)
    if ($activeLocks.Count -gt 0) {
        throw "active build lock refused cleanup: $($activeLocks[0].FullName)"
    }
}

Export-ModuleMember -Function Assert-GeneratedTargetInactive
