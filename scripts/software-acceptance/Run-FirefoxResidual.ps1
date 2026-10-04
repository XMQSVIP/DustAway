$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Expected ordinary token'}
$taskReady=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-real-ready.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$taskSeed=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-seed-verification.json') -Raw -Encoding UTF8 | ConvertFrom-Json
if(-not $taskReady.Prepared -or -not $taskSeed.passed){throw 'Firefox preparation not complete'}
$taskRun=Start-Process -FilePath (Join-Path $taskRoot 'software_firefox_acceptance_test.exe') -ArgumentList @('--ignored','--exact','software::firefox_acceptance::real_firefox_uninstall_residual_protection','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -PassThru -Wait -RedirectStandardOutput (Join-Path $taskRoot 'firefox-residual-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-residual-error.log')
[pscustomobject]@{ExitCode=$taskRun.ExitCode;Finished=(Get-Date).ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-residual-result.json') -Encoding UTF8
