[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$DestinationPath,
    [string]$BundleRoot = "",
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Format-Bool {
    param([bool]$Value)

    if ($Value) {
        return "true"
    }
    return "false"
}

function Resolve-RequiredDirectory {
    param(
        [string]$PathValue,
        [string]$Name
    )

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        throw "$Name is required."
    }
    return (Resolve-Path -LiteralPath $PathValue -ErrorAction Stop).Path
}

function Resolve-OutputDirectory {
    param([string]$PathValue)

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        throw "DestinationPath is required."
    }

    $parent = Split-Path -Parent $PathValue
    $leaf = Split-Path -Leaf $PathValue
    if ([string]::IsNullOrWhiteSpace($parent)) {
        $parent = "."
    }
    if ([string]::IsNullOrWhiteSpace($leaf)) {
        throw "DestinationPath must include a directory name."
    }

    $resolvedParent = Resolve-Path -LiteralPath $parent -ErrorAction Stop
    return (Join-Path $resolvedParent.Path $leaf)
}

function Test-PathInside {
    param(
        [string]$Child,
        [string]$Parent
    )

    $normalizedChild = $Child.TrimEnd("\", "/")
    $normalizedParent = $Parent.TrimEnd("\", "/")
    if ($normalizedChild.Equals($normalizedParent, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $true
    }
    return $normalizedChild.StartsWith(
        "$normalizedParent\",
        [System.StringComparison]::OrdinalIgnoreCase
    )
}

function Copy-DirectoryContents {
    param(
        [string]$Source,
        [string]$Destination
    )

    if ($DryRun) {
        return
    }
    Get-ChildItem -LiteralPath $Source -Force | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination $Destination -Recurse -Force
    }
}

function Copy-DirectoryTo {
    param(
        [string]$Source,
        [string]$Destination
    )

    if ($DryRun) {
        return
    }
    New-Item -ItemType Directory -Force -Path $Destination | Out-Null
    Copy-DirectoryContents -Source $Source -Destination $Destination
}

function Set-ReportLine {
    param(
        [string[]]$Lines,
        [string]$Key,
        [string]$Value
    )

    $prefix = "$Key="
    $updated = $false
    $rewritten = @()
    foreach ($line in $Lines) {
        if ($line.StartsWith($prefix, [System.StringComparison]::Ordinal)) {
            $rewritten += "$prefix$Value"
            $updated = $true
        } else {
            $rewritten += $line
        }
    }
    if (-not $updated) {
        $rewritten += "$prefix$Value"
    }
    return ,$rewritten
}

function Update-RehydratedReleaseAcceptance {
    param([string]$ReleaseDir)

    $reportPath = Join-Path $ReleaseDir "release-acceptance.adm"
    if ($DryRun) {
        return "release_acceptance_report_rewrite=dry_run"
    }
    if (-not (Test-Path -LiteralPath $reportPath -PathType Leaf)) {
        return "release_acceptance_report_rewrite=missing"
    }

    $smokeExecutable = ".\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe"
    $lines = Get-Content -LiteralPath $reportPath -Encoding UTF8
    $lines = @(
        $lines | Where-Object {
            -not $_.StartsWith("handoff_bundle_smoke_", [System.StringComparison]::Ordinal) -and
            -not $_.StartsWith("rehydrated_smoke_", [System.StringComparison]::Ordinal)
        }
    )
    $lines = Set-ReportLine -Lines $lines -Key "smoke_executable" -Value $smokeExecutable
    $lines = Set-ReportLine -Lines $lines -Key "smoke_command" -Value "$smokeExecutable --smoke"
    $lines += "rehydrated_smoke_command_mode=portable-workspace-root-relative"
    $lines += "rehydrated_smoke_command_working_dir=rehydrated-rust-workspace-root"
    $lines += "rehydrated_smoke_executable_placeholder=$smokeExecutable"
    Set-Content -LiteralPath $reportPath -Encoding UTF8 -Value $lines
    return "release_acceptance_report_rewrite=workspace-root-relative"
}

$scriptSourceBundle = Resolve-RequiredDirectory -PathValue (Join-Path $PSScriptRoot "..") -Name "source-bundle"
if ([string]::IsNullOrWhiteSpace($BundleRoot)) {
    $resolvedBundleRoot = Resolve-RequiredDirectory -PathValue (Join-Path $scriptSourceBundle "..") -Name "bundle-root"
} else {
    $resolvedBundleRoot = Resolve-RequiredDirectory -PathValue $BundleRoot -Name "bundle-root"
}

$sourceBundle = Join-Path $resolvedBundleRoot "source-bundle"
$resolvedSourceBundle = Resolve-RequiredDirectory -PathValue $sourceBundle -Name "source-bundle"
if (-not (Test-Path -LiteralPath (Join-Path $resolvedSourceBundle "Cargo.toml") -PathType Leaf)) {
    throw "source-bundle does not look like a Rust workspace: $resolvedSourceBundle"
}

$destination = Resolve-OutputDirectory -PathValue $DestinationPath
if (Test-PathInside -Child $destination -Parent $resolvedBundleRoot) {
    throw "DestinationPath must be outside the handoff bundle root to avoid recursive copies: $destination"
}
if (Test-PathInside -Child $destination -Parent $resolvedSourceBundle) {
    throw "DestinationPath must be outside source-bundle: $destination"
}

$destinationExists = Test-Path -LiteralPath $destination -PathType Container
if ($destinationExists) {
    $existingItems = Get-ChildItem -LiteralPath $destination -Force
    if ($existingItems.Count -gt 0) {
        throw "DestinationPath must be absent or empty: $destination"
    }
} elseif (-not $DryRun) {
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
}

$distDir = Join-Path $destination "dist"
if (-not $DryRun) {
    New-Item -ItemType Directory -Force -Path $distDir | Out-Null
}

$artifactNames = @(
    "AutoDesignMaker-rust",
    "game-build",
    "sdk-bundle",
    "unity-project"
)
$artifactRows = @()
$copiedArtifactCount = 0
foreach ($artifactName in $artifactNames) {
    $artifactSource = Join-Path $resolvedBundleRoot $artifactName
    $present = Test-Path -LiteralPath $artifactSource -PathType Container
    $required = $artifactName -eq "AutoDesignMaker-rust"
    $artifactDestination = Join-Path $distDir $artifactName
    if ($present) {
        Copy-DirectoryTo -Source $artifactSource -Destination $artifactDestination
        $copiedArtifactCount += 1
    } elseif ($required) {
        throw "Required handoff artifact directory is missing: $artifactSource"
    }
    $artifactRows += "dist_artifact=$artifactName; required=$(Format-Bool -Value $required); present=$(Format-Bool -Value $present); destination=$artifactDestination"
}

$handoffBundleDestination = Join-Path $distDir "handoff-bundle"
Copy-DirectoryTo -Source $resolvedBundleRoot -Destination $handoffBundleDestination
Copy-DirectoryContents -Source $resolvedSourceBundle -Destination $destination

$releaseSmokeCommand = ".\dist\AutoDesignMaker-rust\AutoDesignMaker-rust.exe --smoke"
$releaseAcceptanceRewrite = Update-RehydratedReleaseAcceptance -ReleaseDir (Join-Path $distDir "AutoDesignMaker-rust")

$manifestPath = Join-Path $distDir "handoff-rehydration-manifest.adm"
$manifestLines = @(
    "# Handoff Workspace Rehydration",
    "ready=true",
    "bundle_root=$resolvedBundleRoot",
    "source_bundle=$resolvedSourceBundle",
    "destination=$destination",
    "dist_dir=$distDir",
    "source_workspace_restored=true",
    "handoff_bundle_restored=true",
    "handoff_bundle_destination=$handoffBundleDestination",
    "copied_dist_artifact_count=$copiedArtifactCount",
    $releaseAcceptanceRewrite,
    "release_smoke_command=$releaseSmokeCommand",
    "release_smoke_command_working_dir=rehydrated-rust-workspace-root",
    "release_smoke_report=dist\AutoDesignMaker-rust\release-acceptance.adm"
)
$manifestLines += $artifactRows
$manifestLines += @(
    "operator_preflight_command=powershell -ExecutionPolicy Bypass -File .\scripts\handoff_operator_preflight.ps1 -InstructionsPath .\dist\handoff-bundle\evidence\handoff-instructions.adm -DataRoot '<data_root>'",
    "final_acceptance_command=powershell -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'",
    "final_acceptance_ai_invoke_command=powershell -ExecutionPolicy Bypass -File .\scripts\final_handoff_acceptance.ps1 -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>' -RequireAiInvoke",
    "final_acceptance_report=dist\AutoDesignMaker-rust\final-acceptance-run.adm",
    "final_acceptance_package_refresh=after-successful-default-report-write",
    "strict_gate_command=powershell -ExecutionPolicy Bypass -File .\scripts\release_gate.ps1 -RequireExternalAcceptance -UnityExe '<path-to-Unity.exe>' -DataRoot '<data_root>'",
    "delivery_note=Run commands from this rehydrated Rust workspace root."
)

if (-not $DryRun) {
    Set-Content -LiteralPath $manifestPath -Encoding UTF8 -Value $manifestLines
}

foreach ($line in $manifestLines) {
    Write-Output $line
}
Write-Output "manifest_path=$manifestPath"
