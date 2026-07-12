[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string[]]$Target,

    [string]$ProjectRoot,

    [ValidateSet('generated', 'owned-ephemeral-user-data', 'owned-ephemeral-workspace')]
    [string]$Kind = 'generated',

    [string[]]$ProtectedUserData = @(),
    [string]$OwnerManifest,
    [string]$Nonce,
    [switch]$ScanPortableStaging,
    [switch]$Execute,
    [switch]$Json
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$intrinsicProjectRoot = Split-Path -Parent $PSScriptRoot
if ([string]::IsNullOrWhiteSpace($ProjectRoot)) {
    $ProjectRoot = $intrinsicProjectRoot
}
$policyModule = Join-Path $PSScriptRoot 'cleanup\GuardedCleanup.Policy.psm1'
Import-Module $policyModule -Force

try {
    $results = @(Invoke-GuardedCleanup -Target $Target -ProjectRoot $ProjectRoot `
            -IntrinsicProjectRoot $intrinsicProjectRoot -Kind $Kind `
            -ProtectedUserData $ProtectedUserData -OwnerManifest $OwnerManifest `
            -Nonce $Nonce -Execute:$Execute -ScanPortableStaging:$ScanPortableStaging)
}
catch {
    $results = @([pscustomobject]@{
            target = ''
            kind = $Kind
            action = 'refused'
            reason = $_.Exception.Message
            fileCount = 0
            bytes = 0
        })
}

if ($Json) {
    [pscustomobject]@{
        schemaVersion = 1
        mode = if ($Execute) { 'execute' } else { 'dry-run' }
        resultCount = $results.Count
        refusedCount = @($results | Where-Object { $_.action -eq 'refused' }).Count
        deletedCount = @($results | Where-Object { $_.action -eq 'deleted' }).Count
        results = $results
    } | ConvertTo-Json -Depth 8
}
else {
    Write-Host ("Guarded cleanup mode: {0}" -f $(if ($Execute) { 'EXECUTE' } else { 'DRY-RUN' }))
    if ($results.Count -eq 0) { Write-Host 'No allowlisted generated targets were found.' }
    foreach ($result in $results) {
        Write-Host ("[{0}] {1} ({2}, {3} files, {4} bytes) - {5}" -f `
                $result.action.ToUpperInvariant(), $result.target, $result.kind, `
                $result.fileCount, $result.bytes, $result.reason)
    }
}

if (@($results | Where-Object { $_.action -eq 'refused' }).Count -gt 0) { exit 2 }
exit 0
