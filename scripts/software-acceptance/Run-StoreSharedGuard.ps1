param([Parameter(Mandatory=$true)][ValidateSet('shared','restored')][string]$Phase)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or
 [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong dedicated environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'store-shared-guard-manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
foreach($taskFile in $taskManifest.Files.PSObject.Properties){
 if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name) -Algorithm SHA256).Hash -ne $taskFile.Value){throw 'Driver integrity mismatch'}
}
$taskPrefix='store-shared-'+$Phase
$taskOnce=[IO.File]::Open((Join-Path $taskRoot ($taskPrefix+'-20261001.started')),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
$taskStarted=(Get-Date).ToString('o')
$env:DUSTAWAY_STORE_SHARED_PHASE=$Phase
$taskExit=99
try {
 $taskRun=Start-Process -FilePath (Join-Path $taskRoot 'software_store_shared_guard_test.exe') -ArgumentList @('--ignored','--exact','software::store_isolation::tests::real_store_uninstalled_shared_members_are_retained','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot ($taskPrefix+'-run.log')) -RedirectStandardError (Join-Path $taskRoot ($taskPrefix+'-error.log'))
 $taskExit=$taskRun.ExitCode
} catch {$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot ($taskPrefix+'-error.log'))}
[pscustomobject]@{Phase=$Phase;ExitCode=$taskExit;Elevated=$false;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;Started=$taskStarted;Finished=(Get-Date).ToString('o');UninstallExecuted=$false;ResidualDeletionExecuted=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ($taskPrefix+'-result.json')) -Encoding UTF8
exit $taskExit
