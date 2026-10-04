$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or
 [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong dedicated environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'store-process-check-manifest.json') -Raw -Encoding UTF8 | ConvertFrom-Json
foreach($taskFile in $taskManifest.Files.PSObject.Properties){
 if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name) -Algorithm SHA256).Hash -ne $taskFile.Value){throw 'Driver integrity mismatch'}
}
$taskOnce=[IO.File]::Open((Join-Path $taskRoot 'store-process-check-20261001.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
[pscustomobject]@{Elevated=$false;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;Started=(Get-Date).ToString('o');UninstallExecuted=$false;ResidualDeletionExecuted=$false;ProcessTermination=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'store-process-check-context.json') -Encoding UTF8
$taskExit=99
try {
 $taskRun=Start-Process -FilePath (Join-Path $taskRoot 'software_store_process_test.exe') -ArgumentList @('--ignored','--exact','software::store_processes::acceptance::real_store_process_queries_and_lifetime','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'store-process-check-run.log') -RedirectStandardError (Join-Path $taskRoot 'store-process-check-error.log')
 $taskExit=$taskRun.ExitCode
} catch {$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'store-process-check-error.log')}
[pscustomobject]@{ExitCode=$taskExit;Finished=(Get-Date).ToString('o');UninstallExecuted=$false;ResidualDeletionExecuted=$false;ProcessTermination=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'store-process-check-result.json') -Encoding UTF8
exit $taskExit
