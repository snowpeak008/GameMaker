[CmdletBinding()]
param(
    [string]$Cargo = "cargo",
    [string]$ArchiveId = "",
    [string]$UnityExe = "",
    [string]$UnityProjectDir = "",
    [string]$DataRoot = "",
    [string]$TargetId = "windows_desktop_playable",
    [switch]$RequireExternalAcceptance,
    [switch]$RequireAiInvoke,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RustRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Set-Location $RustRoot

if ($RequireAiInvoke -and -not $RequireExternalAcceptance) {
    throw "-RequireAiInvoke requires -RequireExternalAcceptance so missing invocation evidence fails the Unity acceptance gate."
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

if ([string]::IsNullOrWhiteSpace($UnityProjectDir)) {
    $UnityProjectDir = Join-Path $RustRoot "dist\unity-project"
} else {
    $UnityProjectDir = Resolve-OptionalPath -PathValue $UnityProjectDir
}

$GameBundleDir = Join-Path $RustRoot "dist\game-build\windows_desktop_playable"
$SdkBundleDir = Join-Path $RustRoot "dist\sdk-bundle"
$ReleaseDir = Join-Path $RustRoot "dist\AutoDesignMaker-rust"
$ExternalAcceptancePath = Join-Path $ReleaseDir "external-acceptance.adm"
$ConfirmationToken = "ADM_CONFIRM_LOCAL_ENGINE_BUILD"
$ResolvedDataRoot = Resolve-OptionalPath -PathValue $DataRoot

function Invoke-CargoOutput {
    param(
        [string]$Name,
        [string[]]$CargoArgs
    )

    Write-Host ""
    Write-Host "==> $Name"
    Write-Host (Format-CommandLine -Tokens ([string[]](@($Cargo) + $CargoArgs)))
    if ($DryRun) {
        return $null
    }

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $Cargo @CargoArgs 2>&1
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    $text = ($output | Out-String)
    if (-not [string]::IsNullOrWhiteSpace($text)) {
        Write-Host $text.TrimEnd()
    }
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
    return $text
}

function Resolve-ArchiveId {
    param([string]$ExplicitArchiveId)

    if (-not [string]::IsNullOrWhiteSpace($ExplicitArchiveId)) {
        $trimmed = $ExplicitArchiveId.Trim()
        if ((Test-Placeholder -Value $trimmed) -and -not $DryRun) {
            throw "Replace placeholder archive id before running: $trimmed"
        }
        return $trimmed
    }
    if ($DryRun) {
        return "<latest_archive_id>"
    }

    $listOutput = Invoke-CargoOutput "List archives" @("run", "-q", "-p", "adm-cli", "--", "list")
    $archiveIds = @()
    foreach ($line in ($listOutput -split "`r?`n")) {
        $trimmed = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($trimmed)) {
            continue
        }
        $archiveIds += (($trimmed -split "`t| +")[0])
    }
    $archiveIds = @($archiveIds | Where-Object { $_ -like "archive_*" } | Sort-Object)
    if ($archiveIds.Count -eq 0) {
        throw "No formal archives found. Pass -ArchiveId <archive_id>."
    }
    return $archiveIds[-1]
}

function Resolve-UnityExe {
    param([string]$ExplicitUnityExe)

    if (-not [string]::IsNullOrWhiteSpace($ExplicitUnityExe)) {
        if ((Test-Placeholder -Value $ExplicitUnityExe) -and -not $DryRun) {
            throw "Replace placeholder Unity executable before running: $ExplicitUnityExe"
        }
        if ($DryRun) {
            return $ExplicitUnityExe.Trim()
        }
        return (Resolve-Path -LiteralPath $ExplicitUnityExe -ErrorAction Stop).Path
    }
    if ($DryRun) {
        return "<Unity.exe>"
    }

    $doctorOutput = Invoke-CargoOutput "Unity doctor" @("run", "-q", "-p", "adm-cli", "--", "unity-doctor")
    $selected = "none"
    foreach ($line in ($doctorOutput -split "`r?`n")) {
        if ($line -like "selected=*") {
            $selected = $line.Substring("selected=".Length).Trim()
            break
        }
    }
    if ([string]::IsNullOrWhiteSpace($selected) -or $selected -eq "none") {
        throw "Unity editor was not discovered. Pass -UnityExe <path-to-Unity.exe> or set ADM_UNITY_EDITOR."
    }
    return $selected
}

$resolvedArchiveId = Resolve-ArchiveId -ExplicitArchiveId $ArchiveId
$resolvedUnityExe = Resolve-UnityExe -ExplicitUnityExe $UnityExe

if ($DryRun) {
    Write-Host "Dry run: commands will be printed but not executed."
    Write-Host "rust_root=$RustRoot"
    Write-Host "archive_id=$resolvedArchiveId"
    Write-Host "target_id=$TargetId"
    Write-Host "unity_exe=$resolvedUnityExe"
    Write-Host "unity_project_dir=$UnityProjectDir"
    if (-not [string]::IsNullOrWhiteSpace($ResolvedDataRoot)) {
        Write-Host "data_root=$ResolvedDataRoot"
    }
    if ($RequireAiInvoke) {
        Write-Host "require_ai_invoke=true"
    }
}

$null = Invoke-CargoOutput "Stage Unity project" @(
    "run", "-q", "-p", "adm-cli", "--",
    "stage-unity-project", $resolvedArchiveId, $TargetId, $UnityProjectDir
)
$null = Invoke-CargoOutput "Unity build preflight" @(
    "run", "-q", "-p", "adm-cli", "--",
    "unity-build-preflight", $resolvedArchiveId, $TargetId, $resolvedUnityExe, $UnityProjectDir, $ConfirmationToken
)
$null = Invoke-CargoOutput "Run Unity build" @(
    "run", "-q", "-p", "adm-cli", "--",
    "run-unity-build", $resolvedArchiveId, $TargetId, $resolvedUnityExe, $UnityProjectDir, $ConfirmationToken
)
$null = Invoke-CargoOutput "Run Unity runtime validation" @(
    "run", "-q", "-p", "adm-cli", "--",
    "run-unity-runtime-validation", $resolvedArchiveId, $TargetId, $resolvedUnityExe, $UnityProjectDir, $ConfirmationToken
)
$null = Invoke-CargoOutput "Restage game build bundle" @(
    "run", "-q", "-p", "adm-cli", "--",
    "stage-game-build-bundle", $resolvedArchiveId, $TargetId, $GameBundleDir
)
$null = Invoke-CargoOutput "Restage SDK bundle" @(
    "run", "-q", "-p", "adm-cli", "--",
    "stage-sdk-bundle", $resolvedArchiveId, $SdkBundleDir
)
$null = Invoke-CargoOutput "Restage Unity project" @(
    "run", "-q", "-p", "adm-cli", "--",
    "stage-unity-project", $resolvedArchiveId, $TargetId, $UnityProjectDir
)
$null = Invoke-CargoOutput "Delivery doctor" @("run", "-q", "-p", "adm-cli", "--", "delivery-doctor")
$null = Invoke-CargoOutput "Release acceptance" @("run", "-q", "-p", "adm-cli", "--", "release-acceptance")

$externalArgs = @("run", "-q", "-p", "adm-cli", "--", "external-acceptance")
$externalArgs += "--unity-exe"
$externalArgs += $resolvedUnityExe
if ($RequireExternalAcceptance) {
    $externalArgs += "--require-ready"
}
if ($RequireAiInvoke) {
    $externalArgs += "--require-ai-invoke"
}
if (-not [string]::IsNullOrWhiteSpace($ResolvedDataRoot)) {
    $externalArgs += $ReleaseDir
    $externalArgs += $ExternalAcceptancePath
    $externalArgs += $ResolvedDataRoot
}
$null = Invoke-CargoOutput "External acceptance" $externalArgs

if (-not $DryRun) {
    $runtimeReport = Join-Path $UnityProjectDir "Library\AutoDesignMaker\runtime_execution_results.adm"
    $releaseAcceptance = Join-Path $ReleaseDir "release-acceptance.adm"
    $externalAcceptance = Join-Path $ReleaseDir "external-acceptance.adm"
    Write-Host ""
    Write-Host "Unity acceptance gate completed."
    Write-Host "archive_id=$resolvedArchiveId"
    Write-Host "runtime_report=$runtimeReport"
    Write-Host "release_acceptance_report=$releaseAcceptance"
    Write-Host "external_acceptance_report=$externalAcceptance"
} else {
    Write-Host ""
    Write-Host "Dry run complete."
}
