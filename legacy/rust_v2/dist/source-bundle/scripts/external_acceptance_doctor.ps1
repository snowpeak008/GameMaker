[CmdletBinding()]
param(
    [string]$Cargo = "cargo",
    [string]$UnityExe = "",
    [string]$ReportPath = "",
    [string]$DataRoot = "",
    [switch]$RequireReady,
    [switch]$RequireAiInvoke,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RustRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Set-Location $RustRoot

if ($RequireAiInvoke -and -not $RequireReady) {
    throw "-RequireAiInvoke requires -RequireReady so missing invocation evidence fails the external acceptance command."
}

function Resolve-OptionalPath {
    param([string]$PathValue)

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return ""
    }
    $trimmed = $PathValue.Trim()
    if (Test-Placeholder -Value $trimmed) {
        if ($DryRun) {
            return $trimmed
        }
        throw "Replace placeholder path before running: $trimmed"
    }
    $parent = Split-Path -Parent $PathValue
    if ([string]::IsNullOrWhiteSpace($parent)) {
        $parent = "."
    }
    return (Resolve-Path -LiteralPath $parent -ErrorAction Stop).Path |
        ForEach-Object { Join-Path $_ (Split-Path -Leaf $PathValue) }
}

function Test-Placeholder {
    param([string]$Value)

    if ($null -eq $Value) {
        return $false
    }
    $trimmed = $Value.Trim()
    return $trimmed.StartsWith("<") -and $trimmed.EndsWith(">")
}

function Quote-CommandToken {
    param([string]$Value)

    if ($null -eq $Value) {
        return "''"
    }

    $needsQuotes = [string]::IsNullOrEmpty($Value) -or ($Value -match "[\s'`"<>|&()]")
    if (-not $needsQuotes) {
        return $Value
    }

    $escaped = $Value -replace "'", "''"
    return "'$escaped'"
}

function Format-CommandLine {
    param([string[]]$Tokens)

    return (($Tokens | ForEach-Object { Quote-CommandToken -Value $_ }) -join " ")
}

$releaseDir = Join-Path $RustRoot "dist\AutoDesignMaker-rust"
if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $ReportPath = Join-Path $releaseDir "external-acceptance.adm"
} else {
    $ReportPath = Resolve-OptionalPath -PathValue $ReportPath
}
$ResolvedDataRoot = Resolve-OptionalPath -PathValue $DataRoot

$resolvedUnityExe = ""
if (-not [string]::IsNullOrWhiteSpace($UnityExe)) {
    if (Test-Placeholder -Value $UnityExe) {
        if (-not $DryRun) {
            throw "Replace placeholder Unity executable before running: $UnityExe"
        }
        $resolvedUnityExe = $UnityExe.Trim()
    } else {
        if ($DryRun) {
            $resolvedUnityExe = $UnityExe.Trim()
        } else {
            $resolvedUnityExe = (Resolve-Path -LiteralPath $UnityExe -ErrorAction Stop).Path
        }
    }
}

$cargoArgs = @("run", "-q", "-p", "adm-cli", "--", "external-acceptance")
if (-not [string]::IsNullOrWhiteSpace($resolvedUnityExe)) {
    $cargoArgs += "--unity-exe"
    $cargoArgs += $resolvedUnityExe
}
if ($RequireReady) {
    $cargoArgs += "--require-ready"
}
if ($RequireAiInvoke) {
    $cargoArgs += "--require-ai-invoke"
}
$cargoArgs += $releaseDir
$cargoArgs += $ReportPath
if (-not [string]::IsNullOrWhiteSpace($ResolvedDataRoot)) {
    $cargoArgs += $ResolvedDataRoot
}

if ($DryRun) {
    Write-Host "Dry run: command will be printed but not executed."
    Write-Host "rust_root=$RustRoot"
    Write-Host "release_dir=$releaseDir"
    Write-Host "external_acceptance_report=$ReportPath"
    if (-not [string]::IsNullOrWhiteSpace($ResolvedDataRoot)) {
        Write-Host "data_root=$ResolvedDataRoot"
    }
    if (-not [string]::IsNullOrWhiteSpace($resolvedUnityExe)) {
        Write-Host "unity_exe=$resolvedUnityExe"
    }
    if ($RequireAiInvoke) {
        Write-Host "require_ai_invoke=true"
    }
    Write-Host (Format-CommandLine -Tokens ([string[]](@($Cargo) + $cargoArgs)))
    return
}

$previousErrorActionPreference = $ErrorActionPreference
$ErrorActionPreference = "Continue"
try {
    $output = & $Cargo @cargoArgs 2>&1
} finally {
    $ErrorActionPreference = $previousErrorActionPreference
}
Write-Host ($output | Out-String).TrimEnd()
if ($LASTEXITCODE -ne 0) {
    throw "external-acceptance failed with exit code $LASTEXITCODE"
}
