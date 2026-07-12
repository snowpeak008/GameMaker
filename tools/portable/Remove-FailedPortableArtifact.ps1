[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'High')]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string] $TransactionManifest,

    [switch] $Execute
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Import-Module (Join-Path $PSScriptRoot 'PortableBuildSupport.psm1') -Force
Import-Module (Join-Path $PSScriptRoot 'PortableSwap.psm1') -Force

$manifestPath = [System.IO.Path]::GetFullPath($TransactionManifest)
if ($Execute -and -not $PSCmdlet.ShouldProcess($manifestPath, 'Delete only the failed artifact proven by this portable transaction')) {
    return
}

$result = Invoke-PortableFailedArtifactCleanup -TransactionManifest $manifestPath -Execute:$Execute
$result | ConvertTo-Json -Depth 6

if (-not $Execute -and $result.Status -eq 'ready_to_clean_failed_artifact') {
    Write-Host 'Dry run only. Re-run with -Execute to delete the verified failed artifact.'
}
