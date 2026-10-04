param([Parameter(Mandatory=$true)][ValidateSet('BeforeRestart','AfterRestart')][string]$Stage)
$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
$taskExpected=Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software'
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne $taskExpected -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong acceptance environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Expected ordinary user token'}
$taskPrefix=if($Stage -eq 'BeforeRestart'){'msi-restart-preflight'}else{'msi-after-restart'}
$taskMode=if($Stage -eq 'BeforeRestart'){'restart-preflight'}else{'after-restart'}
$taskBoot=(Get-CimInstance Win32_OperatingSystem).LastBootUpTime
if($Stage -eq 'AfterRestart'){
 $taskBefore=Get-Content -LiteralPath (Join-Path $taskRoot 'msi-restart-preflight-context.json') -Raw -Encoding UTF8 | ConvertFrom-Json
 if($taskBoot -le [datetime]$taskBefore.BootTime){throw 'No actual Windows restart since the preflight; no recheck started'}
}
$taskMarker=[IO.File]::Open((Join-Path $taskRoot ($taskPrefix+'.started')),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskMarker.Dispose()
$taskBinary=Join-Path $taskRoot 'software_verify_recovery.exe'
[pscustomobject]@{Account=[Security.Principal.WindowsIdentity]::GetCurrent().Name;Elevated=$false;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;Started=(Get-Date).ToString('o');BootTime=$taskBoot.ToString('o');Stage=$Stage;ExecutableSha256=(Get-FileHash -LiteralPath $taskBinary -Algorithm SHA256).Hash} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ($taskPrefix+'-context.json')) -Encoding UTF8
$taskExit=99
try {
 $taskRun=Start-Process -FilePath $taskBinary -ArgumentList @($taskMode) -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot ($taskPrefix+'-run.log')) -RedirectStandardError (Join-Path $taskRoot ($taskPrefix+'-error.log'))
 $taskExit=$taskRun.ExitCode
} catch {$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot ($taskPrefix+'-error.log'))}
[pscustomobject]@{ExitCode=$taskExit;Stage=$Stage;Finished=(Get-Date).ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot ($taskPrefix+'-result.json')) -Encoding UTF8
exit $taskExit
