[CmdletBinding()]
param(
    [string]$InstructionsPath = "",
    [string]$UnityExe = "",
    [string]$DataRoot = "",
    [switch]$RequireReady
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RustRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Set-Location $RustRoot

function Get-AdmReport {
    param([string]$Path)

    $result = [pscustomobject]@{
        Values = @{}
        OperatorInputs = @()
    }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return $result
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
        $value = $trimmed.Substring($separator + 1)
        if (-not $result.Values.ContainsKey($key)) {
            $result.Values[$key] = $value
        }
        if ($key -eq "operator_input") {
            $result.OperatorInputs += $value
        }
    }
    return $result
}

function Format-Bool {
    param([bool]$Value)

    if ($Value) {
        return "true"
    }
    return "false"
}

function Test-Placeholder {
    param([string]$Value)

    $trimmed = $Value.Trim()
    return $trimmed.StartsWith("<") -and $trimmed.EndsWith(">")
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

function Test-OperatorInputRequired {
    param(
        [string[]]$Rows,
        [string]$InputId
    )

    foreach ($row in $Rows) {
        $first = ($row -split ";", 2)[0].Trim()
        if ($first -eq $InputId) {
            return $true
        }
    }
    return $false
}

function Resolve-CheckPath {
    param([string]$PathValue)

    if ([string]::IsNullOrWhiteSpace($PathValue) -or (Test-Placeholder -Value $PathValue)) {
        return $PathValue.Trim()
    }
    try {
        return (Resolve-Path -LiteralPath $PathValue -ErrorAction Stop).Path
    } catch {
        return $PathValue.Trim()
    }
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

    $candidates = @()
    if (-not [string]::IsNullOrWhiteSpace($ExplicitUnityExe)) {
        $candidates += (New-UnityCandidate -Source "explicit" -Path $ExplicitUnityExe)
    } else {
        if (-not [string]::IsNullOrWhiteSpace($env:ADM_UNITY_EDITOR)) {
            $candidates += (New-UnityCandidate -Source "env:ADM_UNITY_EDITOR" -Path $env:ADM_UNITY_EDITOR)
        }
        if (-not [string]::IsNullOrWhiteSpace($env:UNITY_EDITOR_PATH)) {
            $candidates += (New-UnityCandidate -Source "env:UNITY_EDITOR_PATH" -Path $env:UNITY_EDITOR_PATH)
        }
        foreach ($candidatePath in Get-DefaultUnityEditorCandidates) {
            $candidates += (New-UnityCandidate -Source "default" -Path $candidatePath)
        }
    }

    $firstCandidate = $null
    foreach ($candidate in $candidates) {
        $resolvedPath = Resolve-CheckPath -PathValue $candidate.Path
        if ($null -eq $firstCandidate) {
            $firstCandidate = New-UnityCandidate -Source $candidate.Source -Path $resolvedPath
        }
        if ((-not [string]::IsNullOrWhiteSpace($resolvedPath)) -and
            (-not (Test-Placeholder -Value $resolvedPath)) -and
            (Test-Path -LiteralPath $resolvedPath -PathType Leaf) -and
            ((Split-Path -Leaf $resolvedPath) -ieq "Unity.exe")) {
            return (New-UnityCandidate -Source $candidate.Source -Path $resolvedPath)
        }
    }

    if ($null -ne $firstCandidate) {
        return $firstCandidate
    }
    return (New-UnityCandidate -Source "none" -Path "")
}

if ([string]::IsNullOrWhiteSpace($InstructionsPath)) {
    $InstructionsPath = Join-Path $RustRoot "dist\AutoDesignMaker-rust\handoff-instructions.adm"
}
$InstructionsPath = Resolve-CheckPath -PathValue $InstructionsPath
$instructionsPresent = Test-Path -LiteralPath $InstructionsPath -PathType Leaf
$report = Get-AdmReport -Path $InstructionsPath
$values = $report.Values
$operatorInputs = [string[]]$report.OperatorInputs

$secretEnvVar = Get-ValueOrDefault -Values $values -Key "suggested_ai_secret_env_var" -DefaultValue "OPENAI_API_KEY"
$aiSecretRequired = Test-OperatorInputRequired -Rows $operatorInputs -InputId "ai_secret"
$unityExeRequired = Test-OperatorInputRequired -Rows $operatorInputs -InputId "unity_exe"

if ([string]::IsNullOrWhiteSpace($DataRoot)) {
    $DataRoot = Get-ValueOrDefault -Values $values -Key "external_acceptance_data_root" -DefaultValue ""
}
if ([string]::IsNullOrWhiteSpace($DataRoot)) {
    $DataRoot = Get-ValueOrDefault -Values $values -Key "ai_acceptance_data_root" -DefaultValue ""
}
$resolvedDataRoot = Resolve-CheckPath -PathValue $DataRoot
$dataRootRequired = (Get-ValueOrDefault -Values $values -Key "strict_gate_requires_matching_data_root" -DefaultValue "true") -ne "false"
$dataRootPresent = -not [string]::IsNullOrWhiteSpace($resolvedDataRoot) -and -not (Test-Placeholder -Value $resolvedDataRoot) -and (Test-Path -LiteralPath $resolvedDataRoot -PathType Container)

$unityCandidate = Resolve-UnityCandidate -ExplicitUnityExe $UnityExe
$resolvedUnityExe = $unityCandidate.Path
$unityExeSource = $unityCandidate.Source
$unityExeSupplied = -not [string]::IsNullOrWhiteSpace($resolvedUnityExe) -and -not (Test-Placeholder -Value $resolvedUnityExe)
$unityExePresent = $unityExeSupplied -and (Test-Path -LiteralPath $resolvedUnityExe -PathType Leaf)
$unityExeLooksLikeEditor = $unityExeSupplied -and ((Split-Path -Leaf $resolvedUnityExe) -ieq "Unity.exe")

$archiveId = Get-ValueOrDefault -Values $values -Key "suggested_unity_archive_id" -DefaultValue ""
$archiveKnown = -not [string]::IsNullOrWhiteSpace($archiveId) -and -not (Test-Placeholder -Value $archiveId)
$archiveManifest = ""
$archiveManifestPresent = $false
if ($archiveKnown -and $dataRootPresent) {
    $archiveManifest = Join-Path (Join-Path (Join-Path $resolvedDataRoot "archives") $archiveId) "manifest.adm"
    $archiveManifestPresent = Test-Path -LiteralPath $archiveManifest -PathType Leaf
}

$scriptNames = @(
    "final_handoff_acceptance.ps1",
    "ai_acceptance_gate.ps1",
    "unity_acceptance_gate.ps1",
    "external_acceptance_doctor.ps1",
    "release_gate.ps1"
)
$scriptRows = @()
$scriptsReady = $true
foreach ($scriptName in $scriptNames) {
    $scriptPath = Join-Path $PSScriptRoot $scriptName
    $scriptPresent = Test-Path -LiteralPath $scriptPath -PathType Leaf
    if (-not $scriptPresent) {
        $scriptsReady = $false
    }
    $scriptRows += "script=$scriptName; present=$(Format-Bool -Value $scriptPresent); path=$scriptPath"
}

$aiSecretPresent = $true
if ($aiSecretRequired -and $secretEnvVar -ne "none") {
    $aiSecretPresent = -not [string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($secretEnvVar, "Process"))
}
$unityReady = -not $unityExeRequired -or ($unityExeSupplied -and $unityExePresent -and $unityExeLooksLikeEditor)
$dataRootReady = -not $dataRootRequired -or $dataRootPresent
$archiveReady = -not $unityExeRequired -or $archiveKnown
$ready = $instructionsPresent -and $scriptsReady -and $aiSecretPresent -and $unityReady -and $dataRootReady -and $archiveReady

$missingInputs = @()
if (-not $instructionsPresent) {
    $missingInputs += "missing_input=handoff_instructions; requirement=dist/AutoDesignMaker-rust/handoff-instructions.adm; fix=run-release-gate-or-pass-InstructionsPath"
}
if (-not $scriptsReady) {
    $missingInputs += "missing_input=handoff_scripts; requirement=source-bundle/scripts; fix=run-from-rust-workspace-root-or-restored-source-bundle"
}
if ($aiSecretRequired -and -not $aiSecretPresent) {
    $missingInputs += "missing_input=ai_secret; requirement=env:$secretEnvVar; fix=`$env:$secretEnvVar='<secret>'"
}
if ($unityExeRequired -and -not $unityReady) {
    $missingInputs += "missing_input=unity_exe; requirement=compatible-unity-editor-path; fix=pass -UnityExe '<path-to-Unity.exe>' or set ADM_UNITY_EDITOR/UNITY_EDITOR_PATH"
}
if ($dataRootRequired -and -not $dataRootPresent) {
    $missingInputs += "missing_input=data_root; requirement=matching-acceptance-data-root; fix=pass -DataRoot '<data_root>'"
}
if ($unityExeRequired -and -not $archiveKnown) {
    $missingInputs += "missing_input=archive_id; requirement=suggested_unity_archive_id; fix=run with a DataRoot containing formal archives"
}

Write-Output "# Handoff Operator Preflight"
Write-Output "ready=$(Format-Bool -Value $ready)"
Write-Output "rust_root=$RustRoot"
Write-Output "instructions_path=$InstructionsPath"
Write-Output "instructions_present=$(Format-Bool -Value $instructionsPresent)"
Write-Output "script_count=$($scriptNames.Count)"
foreach ($scriptRow in $scriptRows) {
    Write-Output $scriptRow
}
Write-Output "operator_input_count=$($operatorInputs.Count)"
Write-Output "ai_secret_required=$(Format-Bool -Value $aiSecretRequired)"
Write-Output "ai_secret_env_var=$secretEnvVar"
Write-Output "ai_secret_present=$(Format-Bool -Value $aiSecretPresent)"
Write-Output "unity_exe_required=$(Format-Bool -Value $unityExeRequired)"
Write-Output "unity_exe_supplied=$(Format-Bool -Value $unityExeSupplied)"
Write-Output "unity_exe_present=$(Format-Bool -Value $unityExePresent)"
Write-Output "unity_exe_looks_like_editor=$(Format-Bool -Value $unityExeLooksLikeEditor)"
Write-Output "unity_exe_source=$unityExeSource"
Write-Output "unity_exe=$resolvedUnityExe"
Write-Output "data_root_required=$(Format-Bool -Value $dataRootRequired)"
Write-Output "data_root=$resolvedDataRoot"
Write-Output "data_root_present=$(Format-Bool -Value $dataRootPresent)"
Write-Output "archive_id=$archiveId"
Write-Output "archive_id_known=$(Format-Bool -Value $archiveKnown)"
Write-Output "archive_manifest=$archiveManifest"
Write-Output "archive_manifest_present=$(Format-Bool -Value $archiveManifestPresent)"
Write-Output "missing_input_count=$($missingInputs.Count)"
foreach ($missingInput in $missingInputs) {
    Write-Output $missingInput
}

if ($RequireReady -and -not $ready) {
    throw "Handoff operator preflight is not ready; missing_input_count=$($missingInputs.Count)"
}
