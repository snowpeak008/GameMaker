[CmdletBinding(SupportsShouldProcess = $true, ConfirmImpact = 'High')]
param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string] $TransactionManifest,

    [switch] $Execute
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$portableToolsRoot = Join-Path $PSScriptRoot 'portable'
Import-Module (Join-Path $portableToolsRoot 'PortableBuildSupport.psm1') -Force
Import-Module (Join-Path $portableToolsRoot 'PortableSwap.psm1') -Force

$manifestPath = [System.IO.Path]::GetFullPath($TransactionManifest)
$validation = { param($root) $null = Assert-PortableStage $root }

if ($Execute -and -not $PSCmdlet.ShouldProcess($manifestPath, 'Finalize portable swap and delete only its verified recovery backup')) {
    return
}

$result = Invoke-PortableSwapFinalization -TransactionManifest $manifestPath `
    -ValidateLive $validation -Execute:$Execute
$result | ConvertTo-Json -Depth 6

if (-not $Execute -and $result.Status -in @('ready_to_finalize', 'ready_to_reconcile')) {
    Write-Host 'Dry run only. Re-run with -Execute to reconcile the verified topology and finalize its transaction-bound backup.'
}
