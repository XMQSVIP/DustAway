$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
$name='DustAway-HomeCleanup-DingTalk-UserRetry-20261001'
if((Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue) -or (Test-Path -LiteralPath (Join-Path $root 'DingTalk.retry.started'))){throw 'Retry already dispatched'}
$action=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $root 'Retry-DingTalk.ps1')+'"') -WorkingDirectory $root
$principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $name -Action $action -Principal $principal | Out-Null
Start-ScheduledTask -TaskName $name
[ordered]@{time=(Get-Date -Format o);task=$name;stage='requested'} | ConvertTo-Json
