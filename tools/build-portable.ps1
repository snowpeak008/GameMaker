[CmdletBinding()]
param(
    [ValidatePattern('^[A-Za-z0-9._-]+$')]
    [string] $OutputName = "AutoDesignMaker-NEWrust",

    [switch] $CleanUserData
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$newRustRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $newRustRoot ".."))
$distRoot = Join-Path $newRustRoot "dist"
$stageRoot = Join-Path $distRoot $OutputName
$temporaryStageRoot = Join-Path $distRoot (".{0}.stage-{1}" -f $OutputName, $PID)
$backupStageRoot = Join-Path $distRoot (".{0}.previous-{1}" -f $OutputName, $PID)
$releaseExecutable = Join-Path $newRustRoot "target\release\desktop-tauri.exe"
$webIndex = Join-Path $newRustRoot "web\dist\index.html"
$tauriConfigPath = Join-Path $newRustRoot "apps\desktop-tauri\tauri.conf.json"
$designDataSource = Join-Path $repositoryRoot "knowledge\design_data"
$schemaSource = Join-Path $repositoryRoot "knowledge\schemas"
$artifactLayerSource = Join-Path $repositoryRoot "pipeline\artifact_layer"
$portableFilesSource = Join-Path $PSScriptRoot "portable"

function Invoke-CheckedCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Description,

        [Parameter(Mandatory = $true)]
        [scriptblock] $Command
    )

    Write-Host "==> $Description"
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Description failed with exit code $LASTEXITCODE."
    }
}

function Assert-SafeDistChild {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path
    )

    $resolvedDist = [System.IO.Path]::GetFullPath($distRoot).TrimEnd('\', '/')
    $resolvedPath = [System.IO.Path]::GetFullPath($Path)
    $prefix = $resolvedDist + [System.IO.Path]::DirectorySeparatorChar
    if (-not $resolvedPath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Portable staging path escaped the dist directory: $resolvedPath"
    }
}

function Measure-DirectoryFiles {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        return [pscustomobject]@{ Count = 0; Bytes = [long]0; Digest = "" }
    }
    $resolvedRoot = [System.IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    $files = @(Get-ChildItem -LiteralPath $Path -File -Recurse -Force | Sort-Object FullName)
    [long] $bytes = 0
    foreach ($file in $files) {
        $bytes += [long] $file.Length
    }
    $fingerprintLines = @($files | ForEach-Object {
        $relative = $_.FullName.Substring($resolvedRoot.Length).TrimStart('\', '/')
        $hash = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        "{0}|{1}|{2}" -f $relative.Replace('\', '/'), $_.Length, $hash
    })
    $fingerprintText = $fingerprintLines -join "`n"
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $digestBytes = $sha.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($fingerprintText))
        $digest = ([System.BitConverter]::ToString($digestBytes)).Replace("-", "").ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
    return [pscustomobject]@{ Count = $files.Count; Bytes = [long]$bytes; Digest = $digest }
}

if (-not (Get-Command "npm.cmd" -ErrorAction SilentlyContinue)) {
    throw "npm.cmd was not found in PATH. Install Node.js and npm before building."
}
if (-not (Get-Command "cargo" -ErrorAction SilentlyContinue)) {
    throw "cargo was not found in PATH. Install Rust 1.96 or newer before building."
}
if (-not (Test-Path -LiteralPath $designDataSource -PathType Container)) {
    throw "Required design data was not found: $designDataSource"
}
if (-not (Test-Path -LiteralPath $schemaSource -PathType Container)) {
    throw "Required contract schemas were not found: $schemaSource"
}
if (-not (Test-Path -LiteralPath (Join-Path $artifactLayerSource "registry.json") -PathType Leaf)) {
    throw "Required artifact registry was not found: $artifactLayerSource"
}
if (-not (Test-Path -LiteralPath $portableFilesSource -PathType Container)) {
    throw "Portable support files were not found: $portableFilesSource"
}

Push-Location $newRustRoot
try {
    Invoke-CheckedCommand "Building the Web UI" {
        & npm.cmd --prefix web run build
    }
    Invoke-CheckedCommand "Building the locked desktop release" {
        & cargo build --locked -p desktop-tauri --release
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $webIndex -PathType Leaf)) {
    throw "The Web build did not produce $webIndex"
}
if (-not (Test-Path -LiteralPath $releaseExecutable -PathType Leaf)) {
    throw "The release build did not produce $releaseExecutable"
}

$tauriConfig = Get-Content -LiteralPath $tauriConfigPath -Raw | ConvertFrom-Json

New-Item -ItemType Directory -Path $distRoot -Force | Out-Null
Assert-SafeDistChild -Path $temporaryStageRoot
Assert-SafeDistChild -Path $backupStageRoot
Assert-SafeDistChild -Path $stageRoot
if (Test-Path -LiteralPath $temporaryStageRoot) {
    Remove-Item -LiteralPath $temporaryStageRoot -Recurse -Force
}
if (Test-Path -LiteralPath $backupStageRoot) {
    throw "A previous portable backup still exists and requires review: $backupStageRoot"
}

try {
    New-Item -ItemType Directory -Path $temporaryStageRoot | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $temporaryStageRoot "knowledge") | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $temporaryStageRoot "pipeline") | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $temporaryStageRoot "user_data") | Out-Null

    Copy-Item -LiteralPath $releaseExecutable `
        -Destination (Join-Path $temporaryStageRoot "AutoDesignMaker.exe")
    Copy-Item -LiteralPath $designDataSource `
        -Destination (Join-Path $temporaryStageRoot "knowledge") `
        -Recurse
    Copy-Item -LiteralPath $schemaSource `
        -Destination (Join-Path $temporaryStageRoot "knowledge") `
        -Recurse
    Copy-Item -LiteralPath $artifactLayerSource `
        -Destination (Join-Path $temporaryStageRoot "pipeline") `
        -Recurse
    Copy-Item -LiteralPath (Join-Path $portableFilesSource "Start-AutoDesignMaker.cmd") `
        -Destination $temporaryStageRoot
    Copy-Item -LiteralPath (Join-Path $portableFilesSource "README.txt") `
        -Destination $temporaryStageRoot

    $existingUserData = Join-Path $stageRoot "user_data"
    $stagedUserData = Join-Path $temporaryStageRoot "user_data"
    $existingUserDataMeasure = Measure-DirectoryFiles -Path $existingUserData
    if ($CleanUserData -and $existingUserDataMeasure.Count -gt 0) {
        throw "Clean release refused to overwrite non-empty user_data at $existingUserData"
    }
    if (-not $CleanUserData -and (Test-Path -LiteralPath $existingUserData -PathType Container)) {
        Remove-Item -LiteralPath $stagedUserData -Recurse -Force
        Copy-Item -LiteralPath $existingUserData -Destination $temporaryStageRoot -Recurse -Force
        $stagedUserDataMeasure = Measure-DirectoryFiles -Path $stagedUserData
        if ($stagedUserDataMeasure.Count -ne $existingUserDataMeasure.Count `
            -or $stagedUserDataMeasure.Bytes -ne $existingUserDataMeasure.Bytes `
            -or $stagedUserDataMeasure.Digest -ne $existingUserDataMeasure.Digest) {
            throw "Portable user_data copy verification failed; the existing dist was not changed."
        }
    }

    $stagedExecutable = Join-Path $temporaryStageRoot "AutoDesignMaker.exe"
    $stagedDesignData = Join-Path $temporaryStageRoot "knowledge\design_data"
    $stagedSchemas = Join-Path $temporaryStageRoot "knowledge\schemas"
    $stagedArtifactRegistry = Join-Path $temporaryStageRoot "pipeline\artifact_layer\registry.json"
    $designFiles = @(Get-ChildItem -LiteralPath $stagedDesignData -File -Recurse)
    $schemaFiles = @(Get-ChildItem -LiteralPath $stagedSchemas -File -Recurse)
    $designBytes = ($designFiles | Measure-Object -Property Length -Sum).Sum
    if ($null -eq $designBytes) {
        $designBytes = 0
    }

    $manifest = [ordered]@{
        product = "AutoDesignMaker NEWrust"
        version = [string]$tauriConfig.version
        built_at_utc = [DateTime]::UtcNow.ToString("o")
        executable = "AutoDesignMaker.exe"
        executable_sha256 = (Get-FileHash -LiteralPath $stagedExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
        executable_bytes = (Get-Item -LiteralPath $stagedExecutable).Length
        design_data_root = "knowledge/design_data"
        design_data_file_count = $designFiles.Count
        design_data_bytes = [long]$designBytes
        schema_root = "knowledge/schemas"
        schema_file_count = $schemaFiles.Count
        artifact_registry = "pipeline/artifact_layer/registry.json"
        artifact_registry_sha256 = (Get-FileHash -LiteralPath $stagedArtifactRegistry -Algorithm SHA256).Hash.ToLowerInvariant()
        portable_data_root = "user_data"
        user_data_mode = if ($CleanUserData) { "clean_release" } else { "preserved_local" }
        preserved_user_data_files = if ($CleanUserData) { 0 } else { $existingUserDataMeasure.Count }
        preserved_user_data_bytes = if ($CleanUserData) { 0 } else { $existingUserDataMeasure.Bytes }
        preserved_user_data_digest = if ($CleanUserData) { "" } else { $existingUserDataMeasure.Digest }
    }
    $manifest | ConvertTo-Json | Set-Content `
        -LiteralPath (Join-Path $temporaryStageRoot "build-manifest.json") `
        -Encoding UTF8

    if (Test-Path -LiteralPath $stageRoot) {
        Move-Item -LiteralPath $stageRoot -Destination $backupStageRoot
    }
    try {
        Move-Item -LiteralPath $temporaryStageRoot -Destination $stageRoot
    }
    catch {
        if (-not (Test-Path -LiteralPath $stageRoot) `
            -and (Test-Path -LiteralPath $backupStageRoot)) {
            Move-Item -LiteralPath $backupStageRoot -Destination $stageRoot
        }
        throw
    }
    if (Test-Path -LiteralPath $backupStageRoot) {
        Remove-Item -LiteralPath $backupStageRoot -Recurse -Force
    }
}
catch {
    if (-not (Test-Path -LiteralPath $stageRoot) `
        -and (Test-Path -LiteralPath $backupStageRoot)) {
        Move-Item -LiteralPath $backupStageRoot -Destination $stageRoot
    }
    if (Test-Path -LiteralPath $temporaryStageRoot) {
        Remove-Item -LiteralPath $temporaryStageRoot -Recurse -Force
    }
    throw
}

$finalExecutable = Join-Path $stageRoot "AutoDesignMaker.exe"
$finalHash = (Get-FileHash -LiteralPath $finalExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host ""
Write-Host "Portable trial build ready: $stageRoot"
Write-Host "Executable: $finalExecutable"
Write-Host "SHA-256: $finalHash"
