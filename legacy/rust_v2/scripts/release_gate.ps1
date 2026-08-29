[CmdletBinding()]
param(
    [string]$VcVars64 = "",
    [string]$UnityExe = "",
    [string]$DataRoot = "",
    [string]$Cargo = "cargo",
    [switch]$SkipTests,
    [switch]$SkipBuild,
    [switch]$SkipExternalAcceptance,
    [switch]$RequireExternalAcceptance,
    [switch]$RequireAiInvoke,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RustRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Set-Location $RustRoot

if ($SkipExternalAcceptance -and $RequireExternalAcceptance) {
    throw "-SkipExternalAcceptance cannot be combined with -RequireExternalAcceptance."
}
if ($SkipExternalAcceptance -and $RequireAiInvoke) {
    throw "-SkipExternalAcceptance cannot be combined with -RequireAiInvoke."
}
if ($RequireAiInvoke -and -not $RequireExternalAcceptance) {
    throw "-RequireAiInvoke requires -RequireExternalAcceptance so missing invocation evidence fails the final gate."
}

function Resolve-VcVars64Path {
    param([string]$ExplicitPath)

    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        $resolved = Resolve-Path -LiteralPath $ExplicitPath -ErrorAction Stop
        return $resolved.Path
    }

    $candidates = @(
        "C:\Program Files (x86)\Microsoft Visual Studio\18\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
        "C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat",
        "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
    )

    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return $candidate
        }
    }

    throw "Could not find vcvars64.bat. Pass -VcVars64 <path>."
}

function Resolve-OptionalUnityExePath {
    param([string]$ExplicitPath)

    if ([string]::IsNullOrWhiteSpace($ExplicitPath)) {
        return ""
    }
    if (Test-Placeholder -Value $ExplicitPath) {
        if ($DryRun) {
            return $ExplicitPath.Trim()
        }
        throw "Replace placeholder Unity executable before running: $ExplicitPath"
    }
    if ($DryRun) {
        return $ExplicitPath.Trim()
    }
    $resolved = Resolve-Path -LiteralPath $ExplicitPath -ErrorAction Stop
    return $resolved.Path
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

function Import-VcVars64 {
    param([string]$Path)

    Write-Host "==> Importing Visual Studio build environment"
    Write-Host "vcvars64=$Path"
    $environmentLines = & cmd.exe /d /s /c "call `"$Path`" >nul && set"
    if ($LASTEXITCODE -ne 0) {
        throw "vcvars64.bat failed with exit code $LASTEXITCODE"
    }

    foreach ($line in $environmentLines) {
        $separator = $line.IndexOf("=")
        if ($separator -le 0) {
            continue
        }
        $name = $line.Substring(0, $separator)
        $value = $line.Substring($separator + 1)
        [Environment]::SetEnvironmentVariable($name, $value, "Process")
    }
}

function Invoke-CargoStep {
    param(
        [string]$Name,
        [string[]]$CargoArgs
    )

    Write-Host ""
    Write-Host "==> $Name"
    Write-Host (Format-CommandLine -Tokens ([string[]](@($Cargo) + $CargoArgs)))
    if ($DryRun) {
        return
    }

    $previousErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & $Cargo @CargoArgs
    } finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    if ($LASTEXITCODE -ne 0) {
        throw "$Name failed with exit code $LASTEXITCODE"
    }
}

$vcvarsPath = Resolve-VcVars64Path -ExplicitPath $VcVars64
$resolvedUnityExe = Resolve-OptionalUnityExePath -ExplicitPath $UnityExe
$resolvedDataRoot = Resolve-OptionalPath -PathValue $DataRoot
$releaseDir = Join-Path $RustRoot "dist\AutoDesignMaker-rust"
$externalAcceptancePath = Join-Path $releaseDir "external-acceptance.adm"
if ($DryRun) {
    Write-Host "Dry run: commands will be printed but not executed."
    Write-Host "rust_root=$RustRoot"
    Write-Host "vcvars64=$vcvarsPath"
    if (-not [string]::IsNullOrWhiteSpace($resolvedUnityExe)) {
        Write-Host "unity_exe=$resolvedUnityExe"
    }
    if (-not [string]::IsNullOrWhiteSpace($resolvedDataRoot)) {
        Write-Host "data_root=$resolvedDataRoot"
    }
    if ($RequireAiInvoke) {
        Write-Host "require_ai_invoke=true"
    }
} else {
    Import-VcVars64 -Path $vcvarsPath
}

Invoke-CargoStep "Format check" @("fmt", "--check")
Invoke-CargoStep "Workspace check" @("check", "--workspace")

if (-not $SkipTests) {
    Invoke-CargoStep "Workspace tests" @("test", "--workspace")
} else {
    Write-Host ""
    Write-Host "==> Workspace tests skipped by -SkipTests"
}

if (-not $SkipBuild) {
    Invoke-CargoStep "Desktop release build" @("build", "-p", "adm-desktop", "--release")
} else {
    Write-Host ""
    Write-Host "==> Desktop release build skipped by -SkipBuild"
}

$desktopExe = ".\target\release\adm-desktop.exe"
if (-not $DryRun -and -not (Test-Path -LiteralPath $desktopExe -PathType Leaf)) {
    throw "Expected desktop executable missing: $desktopExe"
}

Invoke-CargoStep "Stage desktop release" @(
    "run", "-q", "-p", "adm-cli", "--",
    "stage-desktop-release", $desktopExe
)
Invoke-CargoStep "Release doctor" @("run", "-q", "-p", "adm-cli", "--", "release-doctor")
Invoke-CargoStep "Delivery doctor" @("run", "-q", "-p", "adm-cli", "--", "delivery-doctor")
Invoke-CargoStep "Release acceptance" @("run", "-q", "-p", "adm-cli", "--", "release-acceptance")
Invoke-CargoStep "Source bundle" @("run", "-q", "-p", "adm-cli", "--", "stage-source-bundle")
Invoke-CargoStep "Handoff bundle" @("run", "-q", "-p", "adm-cli", "--", "stage-handoff-bundle")
Invoke-CargoStep "Source handoff policy" @("run", "-q", "-p", "adm-cli", "--", "write-source-handoff-policy")

if (-not $SkipExternalAcceptance) {
    $externalAcceptanceArgs = @("run", "-q", "-p", "adm-cli", "--", "external-acceptance")
    if (-not [string]::IsNullOrWhiteSpace($resolvedUnityExe)) {
        $externalAcceptanceArgs += "--unity-exe"
        $externalAcceptanceArgs += $resolvedUnityExe
    }
    if ($RequireExternalAcceptance) {
        $externalAcceptanceArgs += "--require-ready"
    }
    if ($RequireAiInvoke) {
        $externalAcceptanceArgs += "--require-ai-invoke"
    }
    if (-not [string]::IsNullOrWhiteSpace($resolvedDataRoot)) {
        $externalAcceptanceArgs += $releaseDir
        $externalAcceptanceArgs += $externalAcceptancePath
        $externalAcceptanceArgs += $resolvedDataRoot
    }
    Invoke-CargoStep "External acceptance" $externalAcceptanceArgs

    $handoffStatusArgs = @("run", "-q", "-p", "adm-cli", "--", "handoff-status")
    if ($RequireExternalAcceptance) {
        $handoffStatusArgs += "--require-ready"
    }
    Invoke-CargoStep "Handoff status" $handoffStatusArgs
    Invoke-CargoStep "Handoff instructions" @("run", "-q", "-p", "adm-cli", "--", "write-handoff-instructions")
    Invoke-CargoStep "Handoff evidence" @("run", "-q", "-p", "adm-cli", "--", "sync-handoff-evidence")
    Invoke-CargoStep "Final handoff package" @("run", "-q", "-p", "adm-cli", "--", "finalize-handoff-package")
    Invoke-CargoStep "Refresh handoff instructions after final package" @("run", "-q", "-p", "adm-cli", "--", "write-handoff-instructions")
    Invoke-CargoStep "Refresh handoff evidence after final package" @("run", "-q", "-p", "adm-cli", "--", "sync-handoff-evidence")
    Invoke-CargoStep "Refresh final handoff package" @("run", "-q", "-p", "adm-cli", "--", "finalize-handoff-package")
} else {
    Write-Host ""
    Write-Host "==> External acceptance skipped by -SkipExternalAcceptance"
}

if (-not $DryRun) {
    $acceptancePath = Join-Path $RustRoot "dist\AutoDesignMaker-rust\release-acceptance.adm"
    if (-not (Test-Path -LiteralPath $acceptancePath -PathType Leaf)) {
        throw "Release acceptance report was not written: $acceptancePath"
    }

    $acceptanceText = Get-Content -LiteralPath $acceptancePath -Raw -Encoding UTF8
    if ($acceptanceText -notmatch "(?m)^accepted=true$") {
        throw "Release acceptance report did not contain accepted=true: $acceptancePath"
    }
    if ($acceptanceText -notmatch "(?m)^smoke_ready=true$") {
        throw "Release acceptance report did not contain smoke_ready=true: $acceptancePath"
    }

    $hashLine = ($acceptanceText -split "`r?`n" | Where-Object { $_ -like "release_hash=*" } | Select-Object -First 1)
    $sourceManifestPath = Join-Path $RustRoot "dist\AutoDesignMaker-rust\source-manifest.adm"
    $handoffBundleManifestPath = Join-Path $RustRoot "dist\AutoDesignMaker-rust\handoff-bundle-manifest.adm"
    $sourceHandoffPolicyPath = Join-Path $RustRoot "dist\AutoDesignMaker-rust\source-handoff-policy.adm"
    $handoffEvidenceManifestPath = Join-Path $RustRoot "dist\AutoDesignMaker-rust\handoff-evidence-manifest.adm"
    $handoffInstructionsPath = Join-Path $RustRoot "dist\AutoDesignMaker-rust\handoff-instructions.adm"
    $finalHandoffManifestPath = Join-Path $RustRoot "dist\AutoDesignMaker-rust\final-handoff-manifest.adm"
    $externalAcceptancePath = Join-Path $RustRoot "dist\AutoDesignMaker-rust\external-acceptance.adm"
    $handoffStatusPath = Join-Path $RustRoot "dist\AutoDesignMaker-rust\handoff-status.adm"
    $externalReadyLine = $null
    $handoffReadyLine = $null
    $sourceHashLine = $null
    $sourceHandoffPolicyLine = $null
    $handoffBundleHashLine = $null
    $handoffEvidenceHashLine = $null
    $handoffInstructionCountLine = $null
    $finalPackageHashLine = $null
    $finalPackageReadyLine = $null
    $finalDeliveryReadyLine = $null
    $finalHandoffReadyLine = $null
    if (-not (Test-Path -LiteralPath $sourceManifestPath -PathType Leaf)) {
        throw "Source manifest report was not written: $sourceManifestPath"
    }
    $sourceManifestText = Get-Content -LiteralPath $sourceManifestPath -Raw -Encoding UTF8
    if ($sourceManifestText -notmatch "(?m)^ready=true$") {
        throw "Source manifest report did not contain ready=true: $sourceManifestPath"
    }
    $sourceHashLine = ($sourceManifestText -split "`r?`n" | Where-Object { $_ -like "bundle_hash=*" } | Select-Object -First 1)
    if (-not (Test-Path -LiteralPath $handoffBundleManifestPath -PathType Leaf)) {
        throw "Handoff bundle manifest was not written: $handoffBundleManifestPath"
    }
    $handoffBundleManifestText = Get-Content -LiteralPath $handoffBundleManifestPath -Raw -Encoding UTF8
    if ($handoffBundleManifestText -notmatch "(?m)^ready=true$") {
        throw "Handoff bundle manifest did not contain ready=true: $handoffBundleManifestPath"
    }
    $handoffBundleHashLine = ($handoffBundleManifestText -split "`r?`n" | Where-Object { $_ -like "bundle_hash=*" } | Select-Object -First 1)
    if (-not (Test-Path -LiteralPath $sourceHandoffPolicyPath -PathType Leaf)) {
        throw "Source handoff policy report was not written: $sourceHandoffPolicyPath"
    }
    $sourceHandoffPolicyText = Get-Content -LiteralPath $sourceHandoffPolicyPath -Raw -Encoding UTF8
    if ($sourceHandoffPolicyText -notmatch "(?m)^ready=true$") {
        throw "Source handoff policy report did not contain ready=true: $sourceHandoffPolicyPath"
    }
    $sourceHandoffPolicyLine = ($sourceHandoffPolicyText -split "`r?`n" | Where-Object { $_ -like "source_handoff_policy=*" } | Select-Object -First 1)
    if (-not $SkipExternalAcceptance) {
        if (-not (Test-Path -LiteralPath $externalAcceptancePath -PathType Leaf)) {
            throw "External acceptance report was not written: $externalAcceptancePath"
        }
        $externalAcceptanceText = Get-Content -LiteralPath $externalAcceptancePath -Raw -Encoding UTF8
        $externalReadyLine = ($externalAcceptanceText -split "`r?`n" | Where-Object { $_ -like "ready=*" } | Select-Object -First 1)
        if ($RequireExternalAcceptance -and $externalAcceptanceText -notmatch "(?m)^ready=true$") {
            throw "External acceptance report did not contain ready=true: $externalAcceptancePath"
        }
        if (-not (Test-Path -LiteralPath $handoffStatusPath -PathType Leaf)) {
            throw "Handoff status report was not written: $handoffStatusPath"
        }
        $handoffStatusText = Get-Content -LiteralPath $handoffStatusPath -Raw -Encoding UTF8
        $handoffReadyLine = ($handoffStatusText -split "`r?`n" | Where-Object { $_ -like "ready=*" } | Select-Object -First 1)
        if ($RequireExternalAcceptance -and $handoffStatusText -notmatch "(?m)^ready=true$") {
            throw "Handoff status report did not contain ready=true: $handoffStatusPath"
        }
        if (-not (Test-Path -LiteralPath $handoffInstructionsPath -PathType Leaf)) {
            throw "Handoff instructions report was not written: $handoffInstructionsPath"
        }
        $handoffInstructionsText = Get-Content -LiteralPath $handoffInstructionsPath -Raw -Encoding UTF8
        if ($handoffInstructionsText -notmatch "(?m)^ready=true$") {
            throw "Handoff instructions report did not contain ready=true: $handoffInstructionsPath"
        }
        $handoffInstructionCountLine = ($handoffInstructionsText -split "`r?`n" | Where-Object { $_ -like "instruction_count=*" } | Select-Object -First 1)
        if (-not (Test-Path -LiteralPath $handoffEvidenceManifestPath -PathType Leaf)) {
            throw "Handoff evidence manifest was not written: $handoffEvidenceManifestPath"
        }
        $handoffEvidenceManifestText = Get-Content -LiteralPath $handoffEvidenceManifestPath -Raw -Encoding UTF8
        if ($handoffEvidenceManifestText -notmatch "(?m)^ready=true$") {
            throw "Handoff evidence manifest did not contain ready=true: $handoffEvidenceManifestPath"
        }
        $handoffEvidenceHashLine = ($handoffEvidenceManifestText -split "`r?`n" | Where-Object { $_ -like "evidence_hash=*" } | Select-Object -First 1)
        if (-not (Test-Path -LiteralPath $finalHandoffManifestPath -PathType Leaf)) {
            throw "Final handoff manifest was not written: $finalHandoffManifestPath"
        }
        $finalHandoffManifestText = Get-Content -LiteralPath $finalHandoffManifestPath -Raw -Encoding UTF8
        if ($finalHandoffManifestText -notmatch "(?m)^ready=true$") {
            throw "Final handoff manifest did not contain ready=true: $finalHandoffManifestPath"
        }
        if ($RequireExternalAcceptance -and $finalHandoffManifestText -notmatch "(?m)^package_ready=true$") {
            throw "Final handoff manifest did not contain package_ready=true: $finalHandoffManifestPath"
        }
        if ($RequireExternalAcceptance -and $finalHandoffManifestText -notmatch "(?m)^handoff_ready=true$") {
            throw "Final handoff manifest did not contain handoff_ready=true: $finalHandoffManifestPath"
        }
        if ($RequireExternalAcceptance -and $finalHandoffManifestText -notmatch "(?m)^delivery_ready=true$") {
            throw "Final handoff manifest did not contain delivery_ready=true: $finalHandoffManifestPath"
        }
        $finalPackageHashLine = ($finalHandoffManifestText -split "`r?`n" | Where-Object { $_ -like "package_hash=*" } | Select-Object -First 1)
        $finalPackageReadyLine = ($finalHandoffManifestText -split "`r?`n" | Where-Object { $_ -like "package_ready=*" } | Select-Object -First 1)
        $finalDeliveryReadyLine = ($finalHandoffManifestText -split "`r?`n" | Where-Object { $_ -like "delivery_ready=*" } | Select-Object -First 1)
        $finalHandoffReadyLine = ($finalHandoffManifestText -split "`r?`n" | Where-Object { $_ -like "handoff_ready=*" } | Select-Object -First 1)
    }

    Write-Host ""
    Write-Host "Release gate passed."
    Write-Host "acceptance_report=$acceptancePath"
    if ($hashLine) {
        Write-Host $hashLine
    }
    Write-Host "source_manifest_report=$sourceManifestPath"
    if ($sourceHashLine) {
        Write-Host "source_$sourceHashLine"
    }
    Write-Host "handoff_bundle_manifest=$handoffBundleManifestPath"
    if ($handoffBundleHashLine) {
        Write-Host ($handoffBundleHashLine -replace "^bundle_hash=", "handoff_bundle_hash=")
    }
    Write-Host "source_handoff_policy_report=$sourceHandoffPolicyPath"
    if ($sourceHandoffPolicyLine) {
        Write-Host $sourceHandoffPolicyLine
    }
    if (-not $SkipExternalAcceptance) {
        Write-Host "external_acceptance_report=$externalAcceptancePath"
        if ($externalReadyLine) {
            Write-Host "external_$externalReadyLine"
        }
        Write-Host "handoff_status_report=$handoffStatusPath"
        if ($handoffReadyLine) {
            Write-Host "handoff_$handoffReadyLine"
        }
        Write-Host "handoff_instructions_report=$handoffInstructionsPath"
        if ($handoffInstructionCountLine) {
            Write-Host $handoffInstructionCountLine
        }
        Write-Host "handoff_evidence_manifest=$handoffEvidenceManifestPath"
        if ($handoffEvidenceHashLine) {
            Write-Host $handoffEvidenceHashLine
        }
        Write-Host "final_handoff_manifest=$finalHandoffManifestPath"
        if ($finalPackageReadyLine) {
            Write-Host "final_$finalPackageReadyLine"
        }
        if ($finalDeliveryReadyLine) {
            Write-Host "final_$finalDeliveryReadyLine"
        }
        if ($finalHandoffReadyLine) {
            Write-Host "final_$finalHandoffReadyLine"
        }
        if ($finalPackageHashLine) {
            Write-Host $finalPackageHashLine
        }
    }
} else {
    Write-Host ""
    Write-Host "Dry run complete."
}
