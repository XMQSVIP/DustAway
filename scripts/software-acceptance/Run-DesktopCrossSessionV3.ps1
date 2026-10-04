$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary token required'}
if([Diagnostics.Process]::GetCurrentProcess().SessionId -eq 0){throw 'Interactive observer required'}
$taskDriver=Join-Path $taskRoot 'desktop-process-driver-v3.exe'
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'desktop-process-driver-v3-manifest.json') -Raw | ConvertFrom-Json
if((Get-FileHash -LiteralPath $taskDriver).Hash -cne $taskManifest.DriverSha256){throw 'Driver digest mismatch'}
$taskOnce=[IO.File]::Open((Join-Path $taskRoot 'desktop-process-cross-session-v3-observer.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
$taskRun=Start-Process -FilePath $taskDriver -ArgumentList @('--ignored','--exact','software::store_processes::acceptance::real_desktop_cross_session_guard','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'desktop-process-cross-session-v3-observer.stdout.log') -RedirectStandardError (Join-Path $taskRoot 'desktop-process-cross-session-v3-observer.stderr.log')
if($null -eq $taskRun.ExitCode){throw 'Native exit code unavailable; do not replay'}
[pscustomobject]@{Time=(Get-Date).ToString('o');ExitCode=$taskRun.ExitCode;OrdinaryUser=$true;Uninstall=$false;Deletion=$false;SecuritySettingsChanged=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'desktop-process-cross-session-v3-observer-result.json') -Encoding UTF8
exit $taskRun.ExitCode
