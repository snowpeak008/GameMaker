[CmdletBinding()]
param(
    [string]$InstructionsPath = "",
    [string]$UnityExe = "",
    [string]$DataRoot = "",
    [string]$ProviderId = "",
    [string]$Model = "",
    [string]$Preset = "",
    [string]$Endpoint = "",
    [string]$SecretRef = "",
    [string]$ArchiveId = "",
    [string]$ReportPath = "",
    [string]$Cargo = "cargo",
    [switch]$RequireAiInvoke,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RustRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Set-Location $RustRoot

$script:ReportLines = [System.Collections.Generic.List[string]]::new()
$script:StepIndex = 0

function Format-Bool {
    param([bool]$Value)

    if ($Value) {
        return "true"
    }
    return "false"
}

function Escape-ReportValue {
    param([string]$Value)

    if ($null -eq $Value) {
        return ""
    }
    return ($Value -replace "`r", "\r" -replace "`n", "\n")
}

function Add-ReportLine {
    param([string]$Line)

    [void]$script:ReportLines.Add($Line)
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

function Remove-ReportLinesStartingWith {
    param([string]$Prefix)

    for ($i = $script:ReportLines.Count - 1; $i -ge 0; $i--) {
        if ($script:ReportLines[$i].StartsWith($Prefix)) {
            $script:ReportLines.RemoveAt($i)
        }
    }
}

function Add-PrefixedAdmOutput {
    param(
        [string]$Prefix,
        [string]$Text
    )

    if ([string]::IsNullOrWhiteSpace($Prefix) -or [string]::IsNullOrWhiteSpace($Text)) {
        return
    }

    foreach ($line in ($Text -split "`r?`n")) {
        $trimmed = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($trimmed) -or $trimmed.StartsWith("#")) {
            continue
        }
        $separator = $trimmed.IndexOf("=")
        if ($separator -lt 0) {
            continue
        }
        $key = $trimmed.Substring(0, $separator)
        $value = $trimmed.Substring($separator + 1)
        Add-ReportLine ("{0}_{1}={2}" -f $Prefix, $key, (Escape-ReportValue -Value $value))
    }
}

function Resolve-ReportPath {
    param([string]$PathValue)

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        $PathValue = Join-Path (Join-Path $RustRoot "dist\AutoDesignMaker-rust") "final-acceptance-run.adm"
    }
    return [System.IO.Path]::GetFullPath($PathValue)
}

function Write-RunReport {
    param([string]$Path)

    $parent = Split-Path -Parent $Path
    if (-not [string]::IsNullOrWhiteSpace($parent)) {
        New-Item -ItemType Directory -Force -Path $parent | Out-Null
    }
    Set-Content -LiteralPath $Path -Encoding UTF8 -Value $script:ReportLines
}

function Get-AdmValues {
    param([string]$Path)

    $values = @{}
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $values
    }
    foreach ($line in Get-Content -LiteralPath $Path -Encoding UTF8) {
        $trimmed = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($trimmed) -or $trimmed.StartsWith("#")) {
            continue
        }
        $separator = $trimmed.IndexOf("=")
        if ($separator -lt 0) {
            continue
        }
        $key = $trimmed.Substring(0, $separator)
        if (-not $values.ContainsKey($key)) {
            $values[$key] = $trimmed.Substring($separator + 1)
        }
    }
    return $values
}

function Get-ValueOrDefault {
    param(
        [hashtable]$Values,
        [string]$Key,
        [string]$DefaultValue
    )

    if ($Values.ContainsKey($Key)) {
        return [string]$Values[$Key]
    }
    return $DefaultValue
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
    if ($DryRun) {
        return $PathValue.Trim()
    }
    $parent = Split-Path -Parent $PathValue
    if ([string]::IsNullOrWhiteSpace($parent)) {
        $parent = "."
    }
    return (Resolve-Path -LiteralPath $parent -ErrorAction Stop).Path |
        ForEach-Object { Join-Path $_ (Split-Path -Leaf $PathValue) }
}

function Get-DefaultUnityEditorCandidates {
    $candidates = @(
        "C:\Program Files\Unity\Editor\Unity.exe",
        "C:\Program Files\Unity Hub\Editor\Unity.exe"
    )
    foreach ($hubRoot in @("C:\Program Files\Unity\Hub\Editor", "C:\Program Files (x86)\Unity\Hub\Editor")) {
        if (Test-Path -LiteralPath $hubRoot -PathType Container) {
            Get-ChildItem -LiteralPath $hubRoot -Directory | ForEach-Object {
                $candidates += (Join-Path (Join-Path $_.FullName "Editor") "Unity.exe")
            }
        }
    }
    return $candidates
}

function New-UnityCandidate {
    param(
        [string]$Source,
        [string]$Path
    )

    [pscustomobject]@{
        Source = $Source
        Path = $Path
    }
}

function Resolve-UnityCandidate {
    param([string]$ExplicitUnityExe)

    if (-not [string]::IsNullOrWhiteSpace($ExplicitUnityExe)) {
        return (New-UnityCandidate -Source "explicit" -Path (Resolve-OptionalPath -PathValue $ExplicitUnityExe))
    }

    $candidates = @()
    if (-not [string]::IsNullOrWhiteSpace($env:ADM_UNITY_EDITOR)) {
        $candidates += (New-UnityCandidate -Source "env:ADM_UNITY_EDITOR" -Path $env:ADM_UNITY_EDITOR)
    }
    if (-not [string]::IsNullOrWhiteSpace($env:UNITY_EDITOR_PATH)) {
        $candidates += (New-UnityCandidate -Source "env:UNITY_EDITOR_PATH" -Path $env:UNITY_EDITOR_PATH)
    }
    foreach ($candidatePath in Get-DefaultUnityEditorCandidates) {
        $candidates += (New-UnityCandidate -Source "default" -Path $candidatePath)
    }

    foreach ($candidate in $candidates) {
        $candidatePath = $candidate.Path.Trim()
        if ([string]::IsNullOrWhiteSpace($candidatePath) -or (Test-Placeholder -Value $candidatePath)) {
            continue
        }
        if ((Test-Path -LiteralPath $candidatePath -PathType Leaf) -and
            ((Split-Path -Leaf $candidatePath) -ieq "Unity.exe")) {
            return (New-UnityCandidate -Source $candidate.Source -Path (Resolve-OptionalPath -PathValue $candidatePath))
        }
    }

    return (New-UnityCandidate -Source "none" -Path "")
}

function Test-Placeholder {
    param([string]$Value)

    if ($null -eq $Value) {
        return $false
    }
    $trimmed = $Value.Trim()
    return $trimmed.StartsWith("<") -and $trimmed.EndsWith(">")
}

function Resolve-InstructionsPath {
    param([string]$ExplicitPath)

    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        return Resolve-OptionalPath -PathValue $ExplicitPath
    }

    $candidates = @(
        (Join-Path $RustRoot "dist\AutoDesignMaker-rust\handoff-instructions.adm"),
        (Join-Path $RustRoot "dist\handoff-bundle\evidence\handoff-instructions.adm")
    )
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }
    return $candidates[0]
}

function Add-ArgIfValue {
    param(
        [System.Collections.ArrayList]$ArgumentList,
        [string]$Name,
        [string]$Value
    )

    if (-not [string]::IsNullOrWhiteSpace($Value)) {
        [void]$ArgumentList.Add($Name)
        [void]$ArgumentList.Add($Value)
    }
}

function Invoke-ScriptStep {
    param(
        [string]$Name,
        [string]$ScriptName,
        [string[]]$ScriptArgs,
        [switch]$RunDuringDryRun,
        [string]$ReportOutputPrefix = ""
    )

    $script:StepIndex += 1
    $stepId = $script:StepIndex
    $scriptPath = Join-Path $PSScriptRoot $ScriptName
    $commandTokens = [string[]](@("powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $scriptPath) + $ScriptArgs)
    $commandLine = Format-CommandLine -Tokens $commandTokens
    Add-ReportLine "step=$stepId; name=$(Escape-ReportValue -Value $Name); script=$ScriptName; status=planned; command=$(Escape-ReportValue -Value $commandLine)"

    if (-not (Test-Path -LiteralPath $scriptPath -PathType Leaf)) {
        Add-ReportLine "step=$stepId; name=$(Escape-ReportValue -Value $Name); script=$ScriptName; status=missing_script; script_path=$(Escape-ReportValue -Value $scriptPath)"
        throw "Missing script for $Name`: $scriptPath"
    }

    Write-Host ""
    Write-Host "==> $Name"
    Write-Host $commandLine
    if ($DryRun -and -not $RunDuringDryRun) {
        Add-ReportLine "step=$stepId; name=$(Escape-ReportValue -Value $Name); script=$ScriptName; status=skipped_dry_run; exit_code=not_run"
        return
    }

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & powershell -NoProfile -ExecutionPolicy Bypass -File $scriptPath @ScriptArgs 2>&1
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    $text = ($output | Out-String)
    if (-not [string]::IsNullOrWhiteSpace($text)) {
        Write-Host $text.TrimEnd()
        Add-PrefixedAdmOutput -Prefix $ReportOutputPrefix -Text $text
    }
    if ($LASTEXITCODE -ne 0) {
        Add-ReportLine "step=$stepId; name=$(Escape-ReportValue -Value $Name); script=$ScriptName; status=failed; exit_code=$LASTEXITCODE"
        throw "$Name failed with exit code $LASTEXITCODE"
    }
    $passedStatus = if ($DryRun) { "passed_dry_run_diagnostic" } else { "passed" }
    Add-ReportLine "step=$stepId; name=$(Escape-ReportValue -Value $Name); script=$ScriptName; status=$passedStatus; exit_code=0"
}

function Invoke-FinalPackageRefresh {
    $cargoArgs = @("run", "-q", "-p", "adm-cli", "--", "finalize-handoff-package")
    Write-Host ""
    Write-Host "==> Refresh final handoff package with final acceptance report"
    $cargoCommand = Format-CommandLine -Tokens ([string[]](@($Cargo) + $cargoArgs))
    Write-Host $cargoCommand

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & $Cargo @cargoArgs 2>&1
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    $text = ($output | Out-String)
    if (-not [string]::IsNullOrWhiteSpace($text)) {
        Write-Host $text.TrimEnd()
    }
    if ($LASTEXITCODE -ne 0) {
        throw "Final handoff package refresh failed with exit code $LASTEXITCODE"
    }
}

$resolvedInstructionsPath = Resolve-InstructionsPath -ExplicitPath $InstructionsPath
$instructions = Get-AdmValues -Path $resolvedInstructionsPath
if (-not (Test-Path -LiteralPath $resolvedInstructionsPath -PathType Leaf) -and -not $DryRun) {
    throw "Handoff instructions not found: $resolvedInstructionsPath"
}

if ([string]::IsNullOrWhiteSpace($DataRoot)) {
    $DataRoot = Get-ValueOrDefault -Values $instructions -Key "external_acceptance_data_root" -DefaultValue ""
}
if ([string]::IsNullOrWhiteSpace($DataRoot)) {
    $DataRoot = Get-ValueOrDefault -Values $instructions -Key "ai_acceptance_data_root" -DefaultValue ""
}
if ([string]::IsNullOrWhiteSpace($ProviderId)) {
    $ProviderId = Get-ValueOrDefault -Values $instructions -Key "ai_provider_id" -DefaultValue "openai_main"
}
if ([string]::IsNullOrWhiteSpace($Model)) {
    $Model = Get-ValueOrDefault -Values $instructions -Key "ai_provider_model" -DefaultValue "gpt-4.1"
}
if ([string]::IsNullOrWhiteSpace($Preset) -and [string]::IsNullOrWhiteSpace($Endpoint)) {
    $Preset = Get-ValueOrDefault -Values $instructions -Key "suggested_ai_provider_preset" -DefaultValue "openai"
}
if ([string]::IsNullOrWhiteSpace($SecretRef)) {
    $SecretRef = Get-ValueOrDefault -Values $instructions -Key "suggested_ai_secret_ref" -DefaultValue "default"
}
if ([string]::IsNullOrWhiteSpace($ArchiveId)) {
    $ArchiveId = Get-ValueOrDefault -Values $instructions -Key "suggested_unity_archive_id" -DefaultValue ""
}

$defaultReportPath = Resolve-ReportPath -PathValue ""
$ReportPath = Resolve-ReportPath -PathValue $ReportPath
$refreshFinalPackageAfterReport = (-not $DryRun.IsPresent) -and ($ReportPath -ieq $defaultReportPath)
$resolvedDataRoot = Resolve-OptionalPath -PathValue $DataRoot
$unityCandidate = Resolve-UnityCandidate -ExplicitUnityExe $UnityExe
$resolvedUnityExe = $unityCandidate.Path
$unityExeSource = $unityCandidate.Source
if ($DryRun -and [string]::IsNullOrWhiteSpace($resolvedUnityExe)) {
    $resolvedUnityExe = "<path-to-Unity.exe>"
    $unityExeSource = "placeholder"
}
if ($DryRun -and [string]::IsNullOrWhiteSpace($ArchiveId)) {
    $ArchiveId = "<archive_id>"
}

Add-ReportLine "# Final Handoff Acceptance Run"
Add-ReportLine "dry_run=$(Format-Bool -Value $DryRun.IsPresent)"
Add-ReportLine "started_at=$(Get-Date -Format o)"
Add-ReportLine "rust_root=$RustRoot"
Add-ReportLine "instructions_path=$resolvedInstructionsPath"
Add-ReportLine "provider_id=$ProviderId"
Add-ReportLine "model=$Model"
Add-ReportLine "preset=$Preset"
Add-ReportLine "endpoint=$Endpoint"
Add-ReportLine "secret_ref=$SecretRef"
Add-ReportLine "archive_id=$ArchiveId"
Add-ReportLine "unity_exe=$resolvedUnityExe"
Add-ReportLine "unity_exe_source=$unityExeSource"
Add-ReportLine "data_root=$resolvedDataRoot"
Add-ReportLine "require_ai_invoke=$(Format-Bool -Value $RequireAiInvoke.IsPresent)"
Add-ReportLine "report_path=$ReportPath"
Add-ReportLine "final_package_refresh_after_report=$(Format-Bool -Value $refreshFinalPackageAfterReport)"
if ($refreshFinalPackageAfterReport) {
    Add-ReportLine "final_package_refresh_result=will_run_after_report_write"
} else {
    Add-ReportLine "final_package_refresh_result=not_required"
}

$finalError = $null
$result = "not_started"

try {
    Write-Host "# Final Handoff Acceptance"
    Write-Host "dry_run=$(if ($DryRun) { 'true' } else { 'false' })"
    Write-Host "rust_root=$RustRoot"
    Write-Host "instructions_path=$resolvedInstructionsPath"
    Write-Host "provider_id=$ProviderId"
    Write-Host "model=$Model"
    if (-not [string]::IsNullOrWhiteSpace($Preset)) {
        Write-Host "preset=$Preset"
    }
    if (-not [string]::IsNullOrWhiteSpace($Endpoint)) {
        Write-Host "endpoint=$Endpoint"
    }
    Write-Host "secret_ref=$SecretRef"
    Write-Host "archive_id=$ArchiveId"
    Write-Host "unity_exe=$resolvedUnityExe"
    Write-Host "unity_exe_source=$unityExeSource"
    Write-Host "data_root=$resolvedDataRoot"
    Write-Host "require_ai_invoke=$(if ($RequireAiInvoke) { 'true' } else { 'false' })"
    Write-Host "report_path=$ReportPath"

    $preflightArgs = [System.Collections.ArrayList]@("-InstructionsPath", $resolvedInstructionsPath, "-DataRoot", $resolvedDataRoot)
    if (-not $DryRun) {
        [void]$preflightArgs.Add("-RequireReady")
    }
    Add-ArgIfValue -ArgumentList $preflightArgs -Name "-UnityExe" -Value $resolvedUnityExe
    Invoke-ScriptStep "Operator preflight" "handoff_operator_preflight.ps1" ([string[]]$preflightArgs) -RunDuringDryRun:$DryRun.IsPresent -ReportOutputPrefix "operator_preflight"

    $aiArgs = [System.Collections.ArrayList]@("-Cargo", $Cargo, "-ProviderId", $ProviderId, "-Model", $Model, "-SecretRef", $SecretRef, "-DataRoot", $resolvedDataRoot, "-RequireReady")
    if (-not [string]::IsNullOrWhiteSpace($Preset)) {
        [void]$aiArgs.Add("-Preset")
        [void]$aiArgs.Add($Preset)
    }
    if (-not [string]::IsNullOrWhiteSpace($Endpoint)) {
        [void]$aiArgs.Add("-Endpoint")
        [void]$aiArgs.Add($Endpoint)
    }
    if ($RequireAiInvoke) {
        [void]$aiArgs.Add("-Invoke")
        [void]$aiArgs.Add("-RequireInvoke")
    }
    Invoke-ScriptStep "AI acceptance" "ai_acceptance_gate.ps1" ([string[]]$aiArgs)

    $unityArgs = [System.Collections.ArrayList]@("-Cargo", $Cargo, "-ArchiveId", $ArchiveId, "-DataRoot", $resolvedDataRoot)
    Add-ArgIfValue -ArgumentList $unityArgs -Name "-UnityExe" -Value $resolvedUnityExe
    Invoke-ScriptStep "Unity acceptance" "unity_acceptance_gate.ps1" ([string[]]$unityArgs)

    $externalArgs = [System.Collections.ArrayList]@("-Cargo", $Cargo, "-DataRoot", $resolvedDataRoot, "-RequireReady")
    Add-ArgIfValue -ArgumentList $externalArgs -Name "-UnityExe" -Value $resolvedUnityExe
    if ($RequireAiInvoke) {
        [void]$externalArgs.Add("-RequireAiInvoke")
    }
    Invoke-ScriptStep "External acceptance" "external_acceptance_doctor.ps1" ([string[]]$externalArgs)

    $releaseArgs = [System.Collections.ArrayList]@("-Cargo", $Cargo, "-DataRoot", $resolvedDataRoot, "-RequireExternalAcceptance")
    Add-ArgIfValue -ArgumentList $releaseArgs -Name "-UnityExe" -Value $resolvedUnityExe
    if ($RequireAiInvoke) {
        [void]$releaseArgs.Add("-RequireAiInvoke")
    }
    Invoke-ScriptStep "Strict release gate" "release_gate.ps1" ([string[]]$releaseArgs)

    $result = if ($DryRun) { "planned" } else { "passed" }
    Add-ReportLine "result=$result"
    Add-ReportLine "completed_at=$(Get-Date -Format o)"

    Write-Host ""
    Write-Host "Final handoff acceptance completed."
    Write-Host "done_when=final-handoff-manifest-has-package_ready=true-handoff_ready=true-delivery_ready=true"
} catch {
    $finalError = $_
    $result = "failed"
    Add-ReportLine "result=failed"
    Add-ReportLine "failed_at=$(Get-Date -Format o)"
    Add-ReportLine "error=$(Escape-ReportValue -Value $_.Exception.Message)"
} finally {
    Add-ReportLine "final_result=$result"
    Write-RunReport -Path $ReportPath
    Write-Host "final_acceptance_run_report=$ReportPath"
}

if ($null -ne $finalError) {
    throw $finalError
}

if ($refreshFinalPackageAfterReport) {
    try {
        Invoke-FinalPackageRefresh
    } catch {
        Remove-ReportLinesStartingWith -Prefix "result="
        Remove-ReportLinesStartingWith -Prefix "final_result="
        Remove-ReportLinesStartingWith -Prefix "final_package_refresh_result="
        Add-ReportLine "result=failed"
        Add-ReportLine "final_package_refresh_result=failed"
        Add-ReportLine "failed_at=$(Get-Date -Format o)"
        Add-ReportLine "error=$(Escape-ReportValue -Value $_.Exception.Message)"
        Add-ReportLine "final_result=failed"
        Write-RunReport -Path $ReportPath
        throw
    }
}
