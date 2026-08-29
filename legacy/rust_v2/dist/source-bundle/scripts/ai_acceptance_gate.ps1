[CmdletBinding()]
param(
    [string]$Cargo = "cargo",
    [string]$ProviderId = "",
    [string]$Model = "",
    [string]$ReportPath = "",
    [string]$Preset = "",
    [string]$Endpoint = "",
    [string]$SecretRef = "",
    [string]$DataRoot = "",
    [switch]$Invoke,
    [switch]$RequireReady,
    [switch]$RequireInvoke,
    [switch]$DryRun
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$RustRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Set-Location $RustRoot

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

if ([string]::IsNullOrWhiteSpace($ProviderId)) {
    throw "Pass -ProviderId <provider_id>."
}
if ([string]::IsNullOrWhiteSpace($Model)) {
    throw "Pass -Model <model>."
}
if (-not [string]::IsNullOrWhiteSpace($Preset) -and -not [string]::IsNullOrWhiteSpace($Endpoint)) {
    throw "Pass either -Preset <preset_id> or -Endpoint <endpoint_hint>, not both."
}
if ([string]::IsNullOrWhiteSpace($Preset) -and -not [string]::IsNullOrWhiteSpace($Endpoint) -and [string]::IsNullOrWhiteSpace($SecretRef)) {
    throw "Pass -SecretRef <env:NAME|named:NAME|none> when using -Endpoint."
}

$releaseDir = Join-Path $RustRoot "dist\AutoDesignMaker-rust"
if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $ReportPath = Join-Path $releaseDir "ai-acceptance.adm"
} else {
    $ReportPath = Resolve-OptionalPath -PathValue $ReportPath
}
$ResolvedDataRoot = Resolve-OptionalPath -PathValue $DataRoot

if (-not [string]::IsNullOrWhiteSpace($Preset)) {
    $presetSecretRef = $SecretRef
    if ([string]::IsNullOrWhiteSpace($presetSecretRef)) {
        $presetSecretRef = "default"
    }
    $presetArgs = @("run", "-q", "-p", "adm-cli", "--", "ai-provider-preset", $Preset, $ProviderId, $presetSecretRef)
    if (-not [string]::IsNullOrWhiteSpace($ResolvedDataRoot)) {
        $presetArgs += $ResolvedDataRoot
    }
    $null = Invoke-CargoOutput "Configure AI provider preset" $presetArgs
} elseif (-not [string]::IsNullOrWhiteSpace($Endpoint)) {
    $providerArgs = @("run", "-q", "-p", "adm-cli", "--", "ai-provider-set", $ProviderId, $Endpoint, $SecretRef)
    if (-not [string]::IsNullOrWhiteSpace($ResolvedDataRoot)) {
        $providerArgs += $ResolvedDataRoot
    }
    $null = Invoke-CargoOutput "Configure AI provider endpoint" $providerArgs
}

$cargoArgs = @("run", "-q", "-p", "adm-cli", "--", "ai-acceptance")
if ($RequireInvoke) {
    $cargoArgs += "--require-invoke"
} else {
    if ($Invoke) {
        $cargoArgs += "--invoke"
    }
    if ($RequireReady) {
        $cargoArgs += "--require-ready"
    }
}
$cargoArgs += $ProviderId
$cargoArgs += $Model
$cargoArgs += $ReportPath
if (-not [string]::IsNullOrWhiteSpace($ResolvedDataRoot)) {
    $cargoArgs += $ResolvedDataRoot
}

if ($DryRun) {
    Write-Host ""
    Write-Host "==> AI acceptance"
    Write-Host (Format-CommandLine -Tokens ([string[]](@($Cargo) + $cargoArgs)))
    Write-Host ""
    Write-Host "Dry run: commands were printed but not executed."
    Write-Host "rust_root=$RustRoot"
    Write-Host "provider_id=$ProviderId"
    Write-Host "model=$Model"
    if (-not [string]::IsNullOrWhiteSpace($Preset)) {
        Write-Host "preset=$Preset"
    }
    if (-not [string]::IsNullOrWhiteSpace($Endpoint)) {
        Write-Host "endpoint=$Endpoint"
    }
    if (-not [string]::IsNullOrWhiteSpace($SecretRef)) {
        Write-Host "secret_ref=$SecretRef"
    }
    if (-not [string]::IsNullOrWhiteSpace($ResolvedDataRoot)) {
        Write-Host "data_root=$ResolvedDataRoot"
    }
    Write-Host "ai_acceptance_report=$ReportPath"
    Write-Host ""
    Write-Host "Dry run complete."
    return
}

$null = Invoke-CargoOutput "AI acceptance" $cargoArgs
