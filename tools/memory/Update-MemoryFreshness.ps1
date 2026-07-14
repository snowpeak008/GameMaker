[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$projectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$configPath = Join-Path $projectRoot 'knowledge\ai_memory\memory_config.json'
$freshnessPath = Join-Path $projectRoot 'knowledge\ai_memory\project_understanding\freshness.json'

function Resolve-MemoryKeyFile {
    param([Parameter(Mandatory = $true)][string] $RelativePath)

    if ([System.IO.Path]::IsPathRooted($RelativePath)) {
        throw "memory key file must be relative: $RelativePath"
    }
    $fullPath = [System.IO.Path]::GetFullPath((Join-Path $projectRoot $RelativePath))
    $rootPrefix = $projectRoot.TrimEnd('\') + '\'
    if (-not $fullPath.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "memory key file escapes the project root: $RelativePath"
    }
    return $fullPath
}

if (-not (Test-Path -LiteralPath $configPath -PathType Leaf)) {
    throw "memory configuration is missing: $configPath"
}
$config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ([int]$config.schema_version -ne 1 -or [string]::IsNullOrWhiteSpace([string]$config.project_id)) {
    throw 'memory configuration has an unsupported schema or missing project id'
}

$files = [ordered]@{}
$missing = New-Object 'System.Collections.Generic.List[string]'
foreach ($entry in @($config.key_files)) {
    $relativePath = ([string]$entry).Replace('\', '/')
    $fullPath = Resolve-MemoryKeyFile $relativePath
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        $missing.Add($relativePath)
        continue
    }
    $item = Get-Item -LiteralPath $fullPath
    $files[$relativePath] = [ordered]@{
        sha256 = (Get-FileHash -LiteralPath $fullPath -Algorithm SHA256).Hash.ToLowerInvariant()
        size = [int64]$item.Length
    }
}

$output = [ordered]@{
    generated_at = [DateTime]::Now.ToString('s')
    project_id = [string]$config.project_id
    config_sha256 = (Get-FileHash -LiteralPath $configPath -Algorithm SHA256).Hash.ToLowerInvariant()
    files = $files
}
$json = $output | ConvertTo-Json -Depth 8
$encoding = New-Object System.Text.UTF8Encoding($false)
$tempPath = "$freshnessPath.tmp-$PID-$([guid]::NewGuid().ToString('N'))"
[System.IO.File]::WriteAllText($tempPath, $json + [Environment]::NewLine, $encoding)
Move-Item -LiteralPath $tempPath -Destination $freshnessPath -Force

Write-Host "[OK] Updated NEWrust freshness with $($files.Count) files"
if ($missing.Count -gt 0) {
    Write-Error "missing memory key files: $($missing -join ', ')"
}
