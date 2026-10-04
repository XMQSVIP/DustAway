$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $env:USERNAME -ne 'MSN'){throw 'Wrong host/user'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-developer-mainstream'
$taskZip=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software\developer-mainstream-bundle.zip'
if(Test-Path -LiteralPath $taskRoot){throw 'Test root already exists'}
Expand-Archive -LiteralPath $taskZip -DestinationPath $taskRoot
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-developer')) -ne 'DustAway.Mainstream.20261001'){throw 'Missing marker'}
$taskDriver=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-cleanup-completion\artifacts\webdriver\msedgedriver.exe'
if((Get-Item -LiteralPath $taskDriver).VersionInfo.FileVersion -ne '154.0.4258.37'){throw 'Unexpected driver'}
Copy-Item -LiteralPath $taskDriver -Destination (Join-Path $taskRoot 'artifacts/webdriver/msedgedriver.exe')
$taskName='DustAway-Mainstream-20261001'
if(Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue){throw 'Task already requested'}
$taskAction=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $taskRoot 'scripts/developer-mainstream/Run-Remote.ps1')+'"') -WorkingDirectory $taskRoot
$taskPrincipal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $taskName -Action $taskAction -Principal $taskPrincipal | Out-Null
Start-ScheduledTask -TaskName $taskName
[ordered]@{task=$taskName;exe_sha256=(Get-FileHash -LiteralPath (Join-Path $taskRoot 'dustaway.exe')).Hash} | ConvertTo-Json
