$c0 = Get-Content 'tests\golden\lane_defense\C0\contract.json' -Raw | ConvertFrom-Json
Write-Host '--- root keys ---'
$c0.PSObject.Properties.Name
Write-Host '--- systems[0] keys ---'
$c0.systems[0].PSObject.Properties.Name
Write-Host '--- mechanics[0] keys ---'
$c0.mechanics[0].PSObject.Properties.Name
Write-Host '--- tables[0] keys ---'
$c0.tables[0].PSObject.Properties.Name
Write-Host '--- content count ---'
$c0.content.Count
Write-Host '--- rationale in C2 source? C2 keys ---'
$c2 = Get-Content 'tests\golden\lane_defense\C2\contract.json' -Raw | ConvertFrom-Json
$c2.PSObject.Properties.Name
Write-Host '--- C2 sections count / first section keys ---'
$c2.sections.Count
$c2.sections[0].PSObject.Properties.Name
