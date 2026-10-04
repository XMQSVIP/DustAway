$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$taskWorkspace=Join-Path $taskRoot 'firefox-desktop-delete-20261001-v2'
if([IO.File]::ReadAllText((Join-Path $taskWorkspace '.dustaway-real-ui-test')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong bundle'}
$taskStatus=Get-Content -LiteralPath (Join-Path $taskWorkspace 'evidence\status.json') -Raw | ConvertFrom-Json
if($taskStatus.phase -ne 'failed' -or $taskStatus.taskId -ne '8451a694-498b-42db-b7b1-1145428c91c9' -or $taskStatus.session -ne 'b573a9a035519d7fbb4b4cc2e9519bb5'){throw 'Unexpected live session'}
if(Test-Path -LiteralPath (Join-Path $taskWorkspace 'evidence\continuation.started')){throw 'Continuation already started'}
$taskRun=Start-Process -FilePath 'D:\nodejs\node.exe' -ArgumentList @('scripts/software-acceptance/firefox-desktop-delete-continue.mjs') -WorkingDirectory $taskWorkspace -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-desktop-delete-v2-continue-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-desktop-delete-v2-continue-error.log')
[pscustomobject]@{ExitCode=$taskRun.ExitCode;Finished=(Get-Date).ToString('o');OriginalTaskPreserved=$true;NoUninstallReplay=$true;OrdinaryUser=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-desktop-delete-v2-continue-result.json') -Encoding UTF8
exit $taskRun.ExitCode
