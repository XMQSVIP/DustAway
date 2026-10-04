$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$taskSeed=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-seed-v3-result.json') -Raw | ConvertFrom-Json
if($taskSeed.ExitCode -ne 0){throw 'Real browser data not verified'}
if(Get-Process -Name brave -ErrorAction SilentlyContinue){throw 'Browser still running'}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-scope-preflight-manifest.json') -Raw | ConvertFrom-Json
foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Preflight integrity mismatch'}}
$taskOnce=[IO.File]::Open((Join-Path $taskRoot 'brave-scope-preflight.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
$taskRun=Start-Process -FilePath (Join-Path $taskRoot 'brave-scope-preflight-driver.exe') -ArgumentList @('--ignored','--exact','software::brave_acceptance::real_brave_scope_preflight','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'brave-scope-preflight.stdout.log') -RedirectStandardError (Join-Path $taskRoot 'brave-scope-preflight.stderr.log')
if($null -eq $taskRun.ExitCode){throw 'Missing original native exit code'}
[pscustomobject]@{Time=(Get-Date).ToString('o');ExitCode=$taskRun.ExitCode;OrdinaryUser=$true;Uninstall=$false;ResidualDeletion=$false;ProductionApprovalAdded=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'brave-scope-preflight-result.json') -Encoding UTF8
exit $taskRun.ExitCode
