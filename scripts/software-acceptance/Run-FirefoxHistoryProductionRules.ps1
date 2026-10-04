$ErrorActionPreference='Stop'
$taskRoot=[IO.Path]::GetFullPath($PSScriptRoot)
if($env:COMPUTERNAME -ne 'DESKTOP-24NJTT6' -or $taskRoot -ne (Join-Path $env:LOCALAPPDATA 'DustAwayAcceptance\20260930-software') -or [IO.File]::ReadAllText((Join-Path $taskRoot '.dustaway-isolated-acceptance')) -ne 'DustAway.Acceptance.20260930.7c7e25e8'){throw 'Wrong environment'}
try{
 $taskPrincipal=New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
 if($taskPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Ordinary user required'}
 if(Get-Process -Name firefox,geckodriver,Un_A,Au_,FirefoxSetup,dustaway,software_firefox_history_production_test -ErrorAction SilentlyContinue){throw 'Prior process remains'}
 if(Get-NetTCPConnection -State Listen -LocalPort 4446,19838,2829 -ErrorAction SilentlyContinue){throw 'Prior test driver remains'}
 $taskSeed=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-production-seed-verification.json') -Raw | ConvertFrom-Json
 $taskSeedResult=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-production-prepare-result.json') -Raw | ConvertFrom-Json
 if($taskSeedResult.ExitCode -ne 0 -or -not $taskSeed.passed -or -not $taskSeed.browserBackupDataDecodedAndVerified -or -not $taskSeed.twoGracefulBrowserExits -or $taskSeed.files.Count -ne 2){throw 'Real browser history data not verified'}
 $taskManifest=Get-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-production-manifest.json') -Raw | ConvertFrom-Json
 foreach($taskFile in $taskManifest.Files.PSObject.Properties){if($taskFile.Name -notmatch '^[A-Za-z0-9_.-]+$' -or (Get-FileHash -LiteralPath (Join-Path $taskRoot $taskFile.Name)).Hash -ne $taskFile.Value){throw 'Driver integrity mismatch'}}
 $taskOnce=[IO.File]::Open((Join-Path $taskRoot 'firefox-history-production-run.started'),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
 $taskOnce.Dispose()
 [pscustomobject]@{Started=(Get-Date).ToString('o');OrdinaryUser=$true;Session=[Diagnostics.Process]::GetCurrentProcess().SessionId;TestOnlyPredicate=$false;ProductionPermissionAdded=$true;VendorWizardTest=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-production-context.json') -Encoding UTF8
 $taskRun=Start-Process -FilePath (Join-Path $taskRoot 'software_firefox_history_production_test.exe') -ArgumentList @('--ignored','--exact','software::firefox_rules_acceptance::real_firefox_history_production_rules','--nocapture') -WorkingDirectory $taskRoot -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput (Join-Path $taskRoot 'firefox-history-production-run.log') -RedirectStandardError (Join-Path $taskRoot 'firefox-history-production-error.log')
 [pscustomobject]@{ExitCode=$taskRun.ExitCode;Finished=(Get-Date).ToString('o');OrdinaryUser=$true;ProductionPermissionAdded=$true} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-production-result.json') -Encoding UTF8
 exit $taskRun.ExitCode
}catch{$_ | Out-String | Add-Content -LiteralPath (Join-Path $taskRoot 'firefox-history-production-error.log') -Encoding UTF8;exit 99}
