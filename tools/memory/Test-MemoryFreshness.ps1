[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$projectRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$configPath = Join-Path $projectRoot 'knowledge\ai_memory\memory_config.json'
$freshnessPath = Join-Path $projectRoot 'knowledge\ai_memory\project_understanding\freshness.json'
$sessionIndexPath = Join-Path $projectRoot 'knowledge\ai_memory\session_history\index.json'

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

foreach ($required in @($configPath, $freshnessPath, $sessionIndexPath)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "required memory file is missing: $required"
    }
}
$config = Get-Content -LiteralPath $configPath -Raw -Encoding UTF8 | ConvertFrom-Json
$snapshot = Get-Content -LiteralPath $freshnessPath -Raw -Encoding UTF8 | ConvertFrom-Json
$null = Get-Content -LiteralPath $sessionIndexPath -Raw -Encoding UTF8 | ConvertFrom-Json

$fresh = New-Object 'System.Collections.Generic.List[string]'
$stale = New-Object 'System.Collections.Generic.List[string]'
$missing = New-Object 'System.Collections.Generic.List[string]'
foreach ($entry in @($config.key_files)) {
    $relativePath = ([string]$entry).Replace('\', '/')
    $fullPath = Resolve-MemoryKeyFile $relativePath
    if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
        $missing.Add($relativePath)
        continue
    }
    $property = $snapshot.files.PSObject.Properties[$relativePath]
    if ($null -eq $property) {
        $stale.Add($relativePath)
        continue
    }
    $item = Get-Item -LiteralPath $fullPath
    $hash = (Get-FileHash -LiteralPath $fullPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hash -ne [string]$property.Value.sha256 -or [int64]$item.Length -ne [int64]$property.Value.size) {
        $stale.Add($relativePath)
    }
    else {
        $fresh.Add($relativePath)
    }
}

$currentConfigHash = (Get-FileHash -LiteralPath $configPath -Algorithm SHA256).Hash.ToLowerInvariant()
$result = [ordered]@{
    status = if ($stale.Count -eq 0 -and $missing.Count -eq 0 -and
        $currentConfigHash -eq [string]$snapshot.config_sha256) { 'passed' } else { 'failed' }
    project_id = [string]$config.project_id
    generated_at = [string]$snapshot.generated_at
    config_matches = $currentConfigHash -eq [string]$snapshot.config_sha256
    stale = @($stale)
    missing = @($missing)
    fresh_count = $fresh.Count
}
$result | ConvertTo-Json -Depth 6
if ($result.status -ne 'passed') {
    exit 1
}
