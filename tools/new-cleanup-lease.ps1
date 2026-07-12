[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Issue', 'Seal', 'Retire')]
    [string]$Operation,

    [ValidateSet('owned-ephemeral-user-data', 'owned-ephemeral-workspace')]
    [string]$Kind,

    [string]$ProjectRoot,
    [string]$TempParent,
    [string]$SourcePath,
    [string]$Target,
    [string]$OwnerManifest,
    [string]$Nonce,
    [ValidateRange(5, 10080)]
    [int]$ValidForMinutes = 1440,
    [switch]$Json
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ([string]::IsNullOrWhiteSpace($ProjectRoot)) { $ProjectRoot = Split-Path -Parent $PSScriptRoot }
$retireModule = Join-Path $PSScriptRoot 'cleanup\GuardedCleanup.Retire.psm1'
Import-Module $retireModule -Force
$leaseModule = Join-Path $PSScriptRoot 'cleanup\GuardedCleanup.Lease.psm1'
Import-Module $leaseModule -Force

try {
    if ([string]::IsNullOrWhiteSpace($Kind)) { throw 'Kind is required' }
    if ($Operation -eq 'Issue') {
        if ([string]::IsNullOrWhiteSpace($TempParent)) { throw 'Issue requires TempParent' }
        $result = New-GuardedCleanupLease -ProjectRoot $ProjectRoot -Kind $Kind `
            -TempParent $TempParent -SourcePath $SourcePath -ValidForMinutes $ValidForMinutes
    }
    elseif ($Operation -eq 'Seal') {
        foreach ($required in @(
                @{ Name = 'Target'; Value = $Target },
                @{ Name = 'OwnerManifest'; Value = $OwnerManifest },
                @{ Name = 'Nonce'; Value = $Nonce })) {
            if ([string]::IsNullOrWhiteSpace([string]$required.Value)) { throw "Seal requires $($required.Name)" }
        }
        $result = Complete-GuardedCleanupLease -ProjectRoot $ProjectRoot -Kind $Kind `
            -Target $Target -OwnerManifest $OwnerManifest -Nonce $Nonce
    }
    else {
        foreach ($required in @(
                @{ Name = 'Target'; Value = $Target },
                @{ Name = 'OwnerManifest'; Value = $OwnerManifest },
                @{ Name = 'Nonce'; Value = $Nonce })) {
            if ([string]::IsNullOrWhiteSpace([string]$required.Value)) { throw "Retire requires $($required.Name)" }
        }
        $result = Remove-GuardedCleanupLeaseReceipt -ProjectRoot $ProjectRoot -Kind $Kind `
            -Target $Target -OwnerManifest $OwnerManifest -Nonce $Nonce
    }
    if ($Json) { $result | ConvertTo-Json -Depth 10 } else { $result | Format-List }
}
catch {
    if ($Json) {
        [pscustomobject]@{ schemaVersion = 2; operation = $Operation.ToLowerInvariant(); status = 'refused'; reason = $_.Exception.Message } |
            ConvertTo-Json -Depth 5
    }
    else { Write-Error $_ }
    exit 2
}
