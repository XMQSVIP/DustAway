$ErrorActionPreference='Stop'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $env:USERNAME -ne 'MSN'){throw 'Wrong machine'}
$taskRoot=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20261001-developer-cleanup'
if([IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-developer')) -ne 'DustAway.Developer.20261001'){throw 'Wrong root'}
$taskFirst=Get-Content -LiteralPath (Join-Path $taskRoot 'evidence/run.json') -Raw | ConvertFrom-Json
if($taskFirst.exit_code -ne 1 -or (Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence') -Filter '*-plan.json') -or (Get-ChildItem -LiteralPath (Join-Path $taskRoot 'evidence') -Filter '*-delete-started.json')){throw 'First UI attempt may have prepared or executed; do not replay'}
if(Test-Path -LiteralPath (Join-Path $taskRoot 'evidence-desktop-v2')){throw 'Second attempt already exists'}
$taskUpload=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance/20260930-software'
Copy-Item -LiteralPath (Join-Path $taskUpload 'desktop.mjs') -Destination (Join-Path $taskRoot 'scripts/developer-cleanup/desktop-v2.mjs')
Copy-Item -LiteralPath (Join-Path $taskUpload 'Run-DesktopV2.ps1') -Destination (Join-Path $taskRoot 'scripts/developer-cleanup/Run-DesktopV2.ps1')
$taskName='DustAway-Developer-DesktopV2-20261001'
if(Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue){throw 'Task already dispatched'}
$taskAction=New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -Argument ('-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File "'+(Join-Path $taskRoot 'scripts/developer-cleanup/Run-DesktopV2.ps1')+'"') -WorkingDirectory $taskRoot
$taskPrincipal=New-ScheduledTaskPrincipal -UserId 'DESKTOP-24NJTT6\MSN' -LogonType Interactive -RunLevel Limited
Register-ScheduledTask -TaskName $taskName -Action $taskAction -Principal $taskPrincipal | Out-Null
Start-ScheduledTask -TaskName $taskName
