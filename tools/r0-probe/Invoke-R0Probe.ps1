[CmdletBinding()]
param(
    [string]$UnityEditor,
    [ValidateRange(1, 10)]
    [int]$Repeat = 1
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspaceRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$fixture = Join-Path $PSScriptRoot 'minimal_game_spec.json'
$reportPath = Join-Path $workspaceRoot 'target\r0-probe\evidence\r0-probe-report.json'

if ([string]::IsNullOrWhiteSpace($UnityEditor)) {
    $candidates = @()
    if (-not [string]::IsNullOrWhiteSpace($env:UNITY_EDITOR_PATH)) {
        $candidates += $env:UNITY_EDITOR_PATH
    }
    foreach ($programRoot in @($env:ProgramFiles, ${env:ProgramFiles(x86)})) {
        if ([string]::IsNullOrWhiteSpace($programRoot)) { continue }
        $hubRoot = Join-Path $programRoot 'Unity\Hub\Editor'
        if (Test-Path -LiteralPath $hubRoot -PathType Container) {
            $candidates += Get-ChildItem -LiteralPath $hubRoot -Directory |
                ForEach-Object { Join-Path $_.FullName 'Editor\Unity.exe' }
        }
    }
    $UnityEditor = $candidates |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Sort-Object -Descending |
        Select-Object -First 1
}

if ([string]::IsNullOrWhiteSpace($UnityEditor) -or
    -not (Test-Path -LiteralPath $UnityEditor -PathType Leaf)) {
    throw 'A valid Unity editor is required. Pass -UnityEditor or set UNITY_EDITOR_PATH.'
}

$fingerprint = ''
Push-Location $workspaceRoot
try {
    for ($index = 1; $index -le $Repeat; $index++) {
        & cargo run --manifest-path (Join-Path $workspaceRoot 'Cargo.toml') --locked `
            -p adm-new-cli --bin adm-new-r0-probe -- run `
            --workspace-root $workspaceRoot `
            --fixture $fixture `
            --unity-editor $UnityEditor
        if ($LASTEXITCODE -ne 0) {
            throw "R0 probe run $index failed with exit code $LASTEXITCODE."
        }
        $report = Get-Content -LiteralPath $reportPath -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($report.status -ne 'passed') {
            throw "R0 probe run $index did not produce a passed report."
        }
        if ($index -eq 1) {
            $fingerprint = [string]$report.deterministicFingerprint
        } elseif ([string]$report.deterministicFingerprint -ne $fingerprint) {
            throw "R0 probe run $index produced a different deterministic fingerprint."
        }
    }
} finally {
    Pop-Location
}

[pscustomobject]@{
    status = 'passed'
    repeats = $Repeat
    deterministicFingerprint = $fingerprint
    unityEditorVersion = $report.unityEditorVersion
    report = 'target/r0-probe/evidence/r0-probe-report.json'
} | ConvertTo-Json
