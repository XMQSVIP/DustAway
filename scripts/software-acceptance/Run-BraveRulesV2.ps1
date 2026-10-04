$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
$taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
if(Get-Process -Name brave,setup,dustaway,brave-rules-v2-driver -ErrorAction SilentlyContinue){throw 'Prior process remains; do not replay'}
$taskSeed=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-seed-v4-result.json') -Raw|ConvertFrom-Json
if($taskSeed.ExitCode -ne 0){throw 'Real data generation incomplete'}
$taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'brave-rules-v2-manifest.json') -Raw|ConvertFrom-Json
foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Integrity mismatch'}}
$taskOnce=[IO.File]::Open((Join-Path $taskRoot 'brave-rules-v2-wrapper.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
$taskOnce.Dispose()
[pscustomobject]@{Time=(Get-Date).ToString('o');OrdinaryUser=$true;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;InteractiveVendorUninstall=$true;TestOnlyResidualEvidence=$true;ProductionApprovalAdded=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRoot 'brave-rules-v2-wrapper-context.json') -Encoding UTF8
$taskExit=99
try{
 $taskRun=Start-Process -FilePath (Join-Path $taskRoot 'brave-rules-v2-driver.exe') -ArgumentList @('--ignored','--exact','software::brave_acceptance::independent_rules::real_brave_interactive_rules','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'brave-rules-v2.stdout.log') -RedirectStandardError (Join-Path $taskRoot 'brave-rules-v2.stderr.log')
 if($null -eq $taskRun.ExitCode){throw 'Missing native exit code'}
 $taskExit=$taskRun.ExitCode
}catch{$_|Out-String|Add-Content -LiteralPath (Join-Path $taskRoot 'brave-rules-v2-wrapper-error.log') -Encoding UTF8}
[pscustomobject]@{Time=(Get-Date).ToString('o');ExitCode=$taskExit;OrdinaryUser=$true;InteractiveVendorUninstall=$true;ProductionApprovalAdded=$false}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $taskRoot 'brave-rules-v2-wrapper-result.json') -Encoding UTF8
exit $taskExit
