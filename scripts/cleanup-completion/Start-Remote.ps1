param([ValidateSet('native','chrome')][string]$Phase='native', [ValidatePattern('^[a-z0-9-]+$')][string]$Run='first')
$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-cleanup-completion'
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-completion')) -ne 'DustAway.CleanupCompletion.20261001'){throw 'Wrong root'}
$taskName="DustAway-CleanupCompletion-$Phase-$Run"
if(Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue){throw 'Task already requested'}
$taskAction=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $taskRoot 'scripts/cleanup-completion/Run-Remote.ps1')+'" -Phase '+$Phase+' -Run '+$Run) -WorkingDirectory $taskRoot
$taskPrincipal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $taskName -Action $taskAction -Principal $taskPrincipal | Out-Null
Start-ScheduledTask -TaskName $taskName
[ordered]@{task=$taskName;requested=(Get-Date -Format o)} | ConvertTo-Json
