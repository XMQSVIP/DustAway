$ErrorActionPreference='Stop'
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'){throw 'Wrong host'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261002-progress-visible-v2'
$taskState=@{task=(Get-ScheduledTask -TaskName 'DustAway-ProgressVisible-20261002-v2' | Select-Object TaskName,State);evidence=@()}
foreach($taskName in @('run.json','results.json','error.txt','stderr.log')){
 $taskPath=Join-Path $taskRoot ('evidence/'+$taskName)
 if(Test-Path -LiteralPath $taskPath){$taskState.evidence+=@{name=$taskName;text=[IO.File]::ReadAllText($taskPath)}}
}
$taskState | ConvertTo-Json -Depth 5
