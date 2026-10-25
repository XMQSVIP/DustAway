$ErrorActionPreference='Stop'
$root='C:\Users\MSN\AppData\Local\DustAwayAcceptance\weixin-021'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or [IO.File]::ReadAllText((Join-Path $root '.dustaway-test')) -cne 'DustAway.Weixin.0.21'){throw 'Wrong host/marker'}
foreach($name in @('DustAway-Weixin-021-checks','DustAway-Weixin-021-suite')){
  $task=Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
  if($task){
    if($task.State -eq 'Running'){throw 'Checks still running'}
    if($task.Actions.WorkingDirectory -ne $root){throw 'Task ownership changed'}
    Unregister-ScheduledTask -TaskName $task.TaskName -Confirm:$false
  }
}
'Finished; evidence retained, temporary check task removed.'
