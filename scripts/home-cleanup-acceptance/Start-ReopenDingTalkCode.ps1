$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6'){throw 'Wrong host'}
$root=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-home-cleanup'
if([IO.File]::ReadAllText((Join-Path $root '.dustaway-test-only')) -ne 'DustAway.HomeCleanup.20261001'){throw 'Wrong root'}
if(@(Get-ScheduledTask | Where-Object {$_.TaskName -like 'DustAway-HomeCleanup-*' -and $_.State -eq 'Running'}).Count){throw 'Another test operation is running'}
$name='DustAway-HomeCleanup-Reopen-DingTalk-Code'
if((Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue) -or (Test-Path -LiteralPath (Join-Path $root 'reopen-dingtalk-code.started'))){throw 'Already requested; inspect original receipt'}
$action=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $root 'Run-ReopenDingTalkCode.ps1')+'"') -WorkingDirectory $root
$principal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $name -Action $action -Principal $principal | Out-Null
Start-ScheduledTask -TaskName $name
@{time=(Get-Date -Format o);task=$name;stage='requested'} | ConvertTo-Json
