[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$portableToolsRoot = Join-Path $PSScriptRoot 'portable'
Import-Module (Join-Path $portableToolsRoot 'PortableBuildSupport.psm1') -Force

$projectRoot = ConvertTo-PortableFullPath (Join-Path $PSScriptRoot '..')
$targetTriple = 'x86_64-pc-windows-msvc'
$cargoTargetRoot = if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
    Join-Path $projectRoot 'target'
}
elseif ([System.IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) {
    [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
}
else {
    [System.IO.Path]::GetFullPath((Join-Path $projectRoot $env:CARGO_TARGET_DIR))
}
$builtLauncher = Join-Path (Join-Path (Join-Path $cargoTargetRoot $targetTriple) 'release') 'AutoDesignMaker.exe'
$installedLauncher = Join-Path $projectRoot 'AutoDesignMaker.exe'
$portableRoot = Join-Path $projectRoot 'dist\AutoDesignMaker-NEWrust'
$candidateLauncher = Join-Path $projectRoot ('.AutoDesignMaker.exe.stage-{0}' -f [guid]::NewGuid().ToString('N'))
$backupLauncher = Join-Path $projectRoot ('.AutoDesignMaker.exe.backup-{0}' -f [guid]::NewGuid().ToString('N'))
$installationVerified = $false

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo was not found in PATH'
}
foreach ($relative in @(
    '.project_root',
    'Cargo.toml',
    'Cargo.lock',
    'apps\root-launcher\Cargo.toml',
    'apps\root-launcher\src\main.rs',
    'apps\root-launcher\src\lib.rs',
    'dist\AutoDesignMaker-NEWrust\AutoDesignMaker.exe',
    'dist\AutoDesignMaker-NEWrust\build-manifest.json',
    'dist\AutoDesignMaker-NEWrust\portable-resource-manifest.json',
    'dist\AutoDesignMaker-NEWrust\knowledge\resource-manifest.json',
    'dist\AutoDesignMaker-NEWrust\pipeline\artifact_layer\registry.json'
)) {
    if (-not (Test-Path -LiteralPath (Join-Path $projectRoot $relative) -PathType Leaf)) {
        throw "required root-launcher input is missing: $relative"
    }
}
if (-not (Test-Path -LiteralPath (Join-Path $portableRoot 'user_data') -PathType Container)) {
    New-Item -ItemType Directory -Path (Join-Path $portableRoot 'user_data') -Force | Out-Null
}

Assert-PortableCargoTargetPath -ProjectRoot $projectRoot `
    -DistRoot (Join-Path $projectRoot 'dist') -CargoTargetRoot $cargoTargetRoot
Assert-NoPortableReparseAncestors $installedLauncher

Push-Location $projectRoot
try {
    Write-Host '==> Building the locked native root launcher'
    & cargo build --locked --target $targetTriple -p adm-new-root-launcher --release
    if ($LASTEXITCODE -ne 0) {
        throw "root launcher build failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $builtLauncher -PathType Leaf)) {
    throw "Rust build did not produce $builtLauncher"
}
$null = Get-PortablePeInspection -Executable $builtLauncher

try {
    Copy-Item -LiteralPath $builtLauncher -Destination $candidateLauncher
    $candidateHash = (Get-FileHash -LiteralPath $candidateLauncher -Algorithm SHA256).Hash.ToLowerInvariant()
    $check = Invoke-PortableSmokeProcess -Executable $candidateLauncher `
        -ArgumentLine '--check-launcher' -TimeoutMilliseconds 30000
    if ($check.ExitCode -ne 0) {
        throw "root launcher self-check failed with exit code $($check.ExitCode): $($check.Output)"
    }

    if (Test-Path -LiteralPath $installedLauncher -PathType Leaf) {
        [System.IO.File]::Replace($candidateLauncher, $installedLauncher, $backupLauncher, $true)
    }
    else {
        Move-Item -LiteralPath $candidateLauncher -Destination $installedLauncher
    }

    $installedHash = (Get-FileHash -LiteralPath $installedLauncher -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $installedHash.Equals($candidateHash, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'installed root launcher hash does not match the verified candidate'
    }
    $installationVerified = $true
    if (Test-Path -LiteralPath $backupLauncher -PathType Leaf) {
        Remove-Item -LiteralPath $backupLauncher -Force
    }
}
finally {
    if (Test-Path -LiteralPath $candidateLauncher -PathType Leaf) {
        Remove-Item -LiteralPath $candidateLauncher -Force
    }
    if ($installationVerified -and (Test-Path -LiteralPath $backupLauncher -PathType Leaf)) {
        Remove-Item -LiteralPath $backupLauncher -Force
    }
}

$item = Get-Item -LiteralPath $installedLauncher
[pscustomobject]@{
    Path = $item.FullName
    Bytes = [int64]$item.Length
    Sha256 = (Get-FileHash -LiteralPath $item.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    PortableRoot = $portableRoot
    StartupProjectDefault = 'blank'
}
