$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-developer-cleanup'
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-developer')) -ne 'DustAway.Developer.20261001'){throw 'Wrong root'}
Get-ScheduledTask -TaskName 'DustAway-Developer-20261001' | Select-Object TaskName,State | ConvertTo-Json
Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence') -File | Select-Object Name,Length | ConvertTo-Json
foreach($taskName in @('run.json','stderr.log','error.txt','stdout.log')){
 $taskPath=Join-Path $taskRoot "evidence/$taskName"
 if(Test-Path -LiteralPath $taskPath){Get-Content -LiteralPath $taskPath -Tail 20}
}
