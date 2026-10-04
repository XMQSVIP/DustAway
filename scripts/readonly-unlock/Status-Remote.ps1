$ErrorActionPreference='Stop'
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20261002-readonly-unlock-v1'
if($env:COMPUTERNAME-ne'DESKTOP-24NJTT6'-or[IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-readonly-unlock'))-ne'DustAway.ReadonlyUnlock.20261002.V1'){throw 'Wrong environment'}
$taskStatus=@(foreach($taskPhase in @('fixture','matrix','real','uv','performance')){
 $taskDirectory=Join-Path $taskRoot ('evidence/'+$taskPhase)
 if(!(Test-Path -LiteralPath $taskDirectory)){continue}
 $taskRow=@{phase=$taskPhase}
 foreach($taskFile in @('run.json','error.txt','stdout.log','stderr.log','comparison/summary.json')){
  $taskPath=Join-Path $taskDirectory $taskFile
  if(Test-Path -LiteralPath $taskPath){try{$taskRow[$taskFile]=Get-Content -LiteralPath $taskPath -Tail 25 -ErrorAction Stop | Out-String}catch{$taskRow[$taskFile]='Still being written'}}
 }
 $taskRow
})
$taskStatus | ConvertTo-Json -Depth 5
